use super::*;
use know::{LookOutcome, Miss};

impl Bots {
    /// A bot's think, when due: senses, memory, reflex, the play's step.
    pub(super) fn think(
        &mut self,
        index: usize,
        view: &MatchView,
        rays: &mut RayBudget,
        notes: &mut Vec<Note>,
    ) {
        let Some(tactics) = self.tactics.as_ref() else {
            return;
        };
        let Some(team) = self.bots[index].as_ref().map(|bot| bot.team) else {
            return;
        };
        let tuning = self.tuning_of(team);
        let now = view.clock;
        let Some(bot) = self.bots[index].as_mut() else {
            return;
        };
        // TEST ONLY: a bot in the jump test does not play (`jump_test.rs`).
        #[cfg(feature = "jump-test")]
        if bot.entered.is_some()
            && let Some(test) = self.jump_test.as_mut()
            && test.think(bot, view, &tactics.world)
        {
            return;
        }
        if !schedule(bot, &tuning, now) {
            return;
        }
        // The sights it tests now are held to this think's next.
        bot.memory.set_next_think(bot.next_think);
        bot.memory
            .set_sight_window(know::SightWindow::of(&tuning.sense, &tuning.cheats));
        let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
            return;
        };
        let team = usize::from(bot.team & 1);
        let due = Due {
            now,
            view,
            me,
            tactics,
            tuning: &tuning,
        };
        // With the sight refresh: a poll that binds (a late one has six or seven thinks due) shares
        // its rays among them, not by bot index (`sense::fair_share`).
        if tuning.sense.sight_refresh {
            let thinks_left = rays.thinks_left;
            rays.thinks_left = thinks_left.saturating_sub(1);
            rays.allow(sense::fair_share(
                rays.left,
                thinks_left,
                sense::think_most(tuning.sense.battery_rays),
            ));
        }
        let sensed = sense(&due, bot, rays);
        let state = &mut self.teams[team];
        let percepts = share(&due, bot, state, &self.tracker, sensed);
        sense_batteries(&due, bot, state, rays, notes);
        rays.allow(None);
        remember(&due, bot, &percepts);
        react(&due, bot, state, &self.features.weapons, notes);
        self_kill(&due, bot, state, notes);
        if now < bot.pause_until {
            bot.intent = Intent::stay();
            return;
        }
        #[cfg(test)]
        if bench_camper(&due, bot, &mut self.bench_camper) {
            return;
        }
        let Some(step_name) = decide(&due, bot, state, &self.memo, &self.features.weapons, notes) else {
            return;
        };
        // [traps] A bot that carries traps may start to put one down.
        traps::think(bot, &due, state);
        // [trap_react] A trap that is not the team's: a chance to notice it.
        trap_react::think(bot, &due, state);
        if self.trace.enabled() && now >= bot.last_trace + tuning.think.trace_every_ms {
            bot.last_trace = now;
            let line = serde_json::json!({
                "t": now, "bot": bot.place, "pos": me.feet.to_array(), "health": me.health,
                "play": format!("{:?}", bot.assignment.as_ref().map(|a| a.kind)),
                "step": step_name,
                "aim_at": bot.reflex.target(),
                "known": bot.memory.tracks().iter()
                    .map(|t| serde_json::json!([t.who, format!("{:?}", t.source), t.error_m]))
                    .collect::<Vec<_>>(),
            });
            self.trace.line(line);
        }
    }

    /// A bot's ticks due: the motor drives the intent (the reflex
    /// overriding the look and the trigger), the encoder makes inputs.
    pub(super) fn ticks(
        &mut self,
        index: usize,
        clock: &Clock,
        view: &MatchView,
        made: &mut Vec<Made>,
    ) {
        let Some(team) = self.bots[index].as_ref().map(|bot| bot.team) else {
            return;
        };
        let tuning = self.tuning_of(team);
        let Some(bot) = self.bots[index].as_mut() else {
            return;
        };
        let Some(entered) = bot.entered else {
            return;
        };
        bot.tracing = self.trace.enabled();
        bot.reflex.tracing = bot.tracing;
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        let searching = &mut self.features.searching;
        for time in bot.encoder.ticks(clock.clock, entered + START_AFTER_MS) {
            #[cfg(test)]
            {
                bot.bench_driven = clock.clock;
            }
            let estimate = self
                .tracker
                .body(bot.place)
                .and_then(|body| body.estimate().copied());
            let me = view.place(bot.place).filter(|p| p.alive);
            // TEST ONLY: the jump test's keys while it stands, runs and records.
            #[cfg(feature = "jump-test")]
            let scripted = self
                .jump_test
                .as_mut()
                .and_then(|test| test.drive(bot, time, view));
            #[cfg(not(feature = "jump-test"))]
            let scripted: Option<Decision> = None;
            #[cfg(test)]
            {
                bot.bench_go = None;
            }
            #[cfg_attr(not(feature = "jump-test"), allow(unused_mut))]
            let mut decision = match (scripted, estimate, me, self.tactics.as_ref()) {
                (Some(scripted), ..) => {
                    bot.motor.rest();
                    scripted
                }
                (None, Some(at), Some(me), Some(tactics)) if clock.playing => {
                    // The base pitch: `drive_tick` adds the kick.
                    let me = PlaceView {
                        feet: at.feet,
                        yaw: at.yaw,
                        pitch: at.pitch,
                        velocity: at.velocity,
                        known_at: at.time,
                        ..me.clone()
                    };
                    drive_tick(bot, &me, view, tactics, &tuning, time, searching)
                }
                _ => bot.motor.rest(),
            };
            #[cfg(feature = "jump-test")]
            if let Some(test) = self.jump_test.as_mut() {
                test.quiet(bot.place, &mut decision);
                test.motor_seen(bot, time, decision.keys);
            }
            // Blind: the last world is older than the tuned limit; the reckoning alone does not walk it on.
            if let Some(limit) = tuning.unstick.blind_stop_ms
                && self.tracker.body(bot.place).and_then(|body| body.fix()).is_some_and(|fix| time.saturating_sub(fix.time) >= limit)
                && decision.keys & stuck::BLIND_KEYS != 0
            {
                decision.keys &= !stuck::BLIND_KEYS;
                bot.blind_since.get_or_insert(time);
                #[cfg(test)]
                stuck::blind::count(0);
            }
            if let Some(line) = bot.reflex.take_friend_note() {
                bot.traced.push(line);
            }
            bot.traced.extend(bot.reflex.take_gate_note());
            let (input, turn) = bot.encoder.encode(time, decision);
            let keys = bot.encoder.keys();
            // [recoil] The part of the pitch turn the kick takes, and the
            // tick's shots: the base pitch moves by the rest.
            let taken = bot.reflex.sent(time, keys, turn.1);
            self.tracker.input(
                bot.place,
                time,
                keys,
                (turn.0, turn.1 + taken),
                bot.reflex.kick(),
                &tuning.kin,
                world.as_deref(),
            );
            if let Some(input) = input {
                made.push(Made {
                    place: bot.place,
                    time,
                    input,
                });
            }
        }
        for line in bot.traced.drain(..) {
            self.trace.line(line);
        }
    }
}

/// What a think of one bot works on, once it is due: the clock, the view, the bot's own place, the map's tactics and the tuning of its team.
#[derive(Clone, Copy)]
pub(super) struct Due<'a> {
    pub(super) now: u32,
    pub(super) view: &'a MatchView,
    pub(super) me: &'a PlaceView,
    pub(super) tactics: &'a Tactics,
    pub(super) tuning: &'a Tuning,
}

/// What the senses of one think found: the percepts, and whether the bot sensed and remembered nothing.
pub(super) struct Sensed {
    pub(super) percepts: Vec<Percept>,
    pub(super) quiet: bool,
}

/// The senses: sight (the order turns each think, so no enemy waits long for a ray), hearing.
pub(super) fn sense(due: &Due, bot: &mut Bot, rays: &mut RayBudget) -> Sensed {
    let Due {
        now,
        view,
        me,
        tactics,
        tuning,
    } = *due;
    let enemies: Vec<Place> = view.enemies_of(bot.team).map(|p| p.place).collect();
    let live: Vec<(Place, u32)> = bot.memory.live_sights().collect();
    let order = if tuning.sense.sight_refresh {
        // The sights it has are looked at again first, then the enemies it has
        // not looked at the longest (research B5).
        sense::look_order(
            &enemies,
            |who| live.iter().any(|&(live, _)| live == who),
            |who| bot.tested.get(who).copied().unwrap_or(0),
        )
    } else {
        // As before the refresh: the order turns each think, so no enemy waits long for a ray.
        let mut order = enemies;
        if !order.is_empty() {
            let k = (now / 100) as usize % order.len();
            order.rotate_left(k);
        }
        order
    };
    let (percepts, looks) = sense::sense_looks(
        me,
        view,
        &tactics.world.sight,
        &tuning.sense,
        &tuning.cheats,
        bot.heard_until,
        &order,
        rays,
        &mut bot.rng,
    );
    bot.heard_until = now;
    for &(who, outcome) in &looks {
        if matches!(outcome, LookOutcome::Seen | LookOutcome::Failed(Miss::Blocked)) {
            if bot.tested.len() <= who {
                bot.tested.resize(who + 1, 0);
            }
            bot.tested[who] = now;
        }
    }
    if tuning.sense.sight_refresh {
        tell_looks(&mut bot.memory, &live, &looks, view, me.feet);
    }
    let quiet = percepts.is_empty() && bot.memory.all_seen(now, view.clock).is_empty();
    Sensed { percepts, quiet }
}

/// What a think's looks tell the memory of the sights it had (`live`: who,
/// since when): one a look found gone ends after its grace (at once when he
/// died or is another life), one the rays ran out on keeps its state to the
/// bound, and one found is the percept's, not told here. A sight with no
/// look this think (not an enemy in the order) is left as it is.
pub(super) fn tell_looks(
    memory: &mut Memory,
    live: &[(Place, u32)],
    looks: &[(Place, LookOutcome)],
    view: &MatchView,
    feet: Vec3,
) {
    for &(who, since) in live {
        let outcome = match view.place(who) {
            None => LookOutcome::Failed(Miss::Dead),
            Some(enemy) if enemy.spawned > since => LookOutcome::Failed(Miss::Dead),
            Some(_) => match looks.iter().find(|look| look.0 == who) {
                Some(&(_, outcome)) => outcome,
                None => continue,
            },
        };
        if outcome != LookOutcome::Seen {
            let at = view.place(who).map_or(feet, |enemy| enemy.feet);
            memory.looked(who, at, outcome, view.clock);
        }
    }
}

/// Whether the bot's think is due: it has entered the match and its next think has come. If so, the next one is drawn
/// (`ThinkTuning::think_ms`) before anything else in this think can draw from the bot's generator.
pub(super) fn schedule(bot: &mut Bot, tuning: &Tuning, now: u32) -> bool {
    if bot.entered.is_none() || now < bot.next_think {
        return false;
    }
    bot.next_think = now
        + bot.rng.range(
            tuning.think.think_ms.0 as f32,
            tuning.think.think_ms.1 as f32,
        ) as u32;
    true
}

/// What the bot's team learns from what it sensed and what the team tells the bot: the callouts, an enemy carrying a battery, the
/// cheats' percepts, a fall by a battery, the teammates' callouts. Gives all the percepts of this think.
pub(super) fn share(
    due: &Due,
    bot: &mut Bot,
    state: &mut TeamState,
    tracker: &Tracker,
    sensed: Sensed,
) -> Vec<Percept> {
    let Due {
        now,
        view,
        me,
        tuning,
        ..
    } = *due;
    let Sensed {
        mut percepts,
        quiet,
    } = sensed;
    for percept in &percepts {
        state
            .knowledge
            .call_out(bot.place, percept, tuning.sense.callout_ms);
        // An enemy seen carrying a battery (it is in his hands).
        if percept.source == Source::Seen
            && let Some(battery) = view.place(percept.who).and_then(|p| p.carrying)
        {
            state
                .knowledge
                .saw_carrying(percept.who, battery, percept.at, percept.time);
        }
    }
    percepts.extend(cheats::cheat_percepts(
        bot.team,
        view,
        &tuning.cheats,
        &mut state.cheats,
        me.feet,
        quiet,
        &mut bot.rng,
    ));
    // An enemy carrier's place, first-hand or the thief cheat's (a
    // callout is a teammate's own): whether he stays put (Intercept's
    // camping carrier).
    let carrying = |p: Place| view.batteries.contains(&BatteryState::Carried(Some(p)));
    for percept in &percepts {
        if matches!(
            percept.source,
            Source::Seen | Source::Heard | Source::Cheat(know::CheatKind::Thief)
        ) && carrying(percept.who)
        {
            state.knowledge.carrier_at(
                percept.who,
                percept.at,
                percept.error_m,
                percept.time,
                carrying,
            );
        }
    }
    // Going for a battery, far off its reckoning and below it near it:
    // it fell there (a ledge by the battery), which the team counts against
    // offering it again (`plays::collect`, `fallen_at`).
    if let Some(a) = bot.assignment.as_ref()
        && a.kind == PlayKind::Collect
        && let plays::TargetRef::Battery(battery) = a.target
        && let Some(at) = state.knowledge.battery(battery).map(|k| k.at).or_else(|| {
            match view.batteries.get(battery) {
                Some(BatteryState::Free(at)) if tuning.cheats.batteries => Some(*at),
                _ => None,
            }
        })
        && me.feet.distance_xz(at) <= tuning.number(PlayKind::Collect, "fall_near_m")
        && me.feet.y < at.y - plays::collect::BELOW_STAND_M
        && tracker
            .body(bot.place)
            .and_then(|body| body.last_error())
            .is_some_and(|error| error > REPLAN_ERROR_M)
    {
        state.knowledge.fell_near(bot.place, at, now);
    }
    percepts.extend(state.knowledge.heard_by(bot.place, now));
    percepts
}

/// With the battery cheat off, the batteries and spawners' spots the bot sees become its team's knowledge.
pub(super) fn sense_batteries(
    due: &Due,
    bot: &mut Bot,
    state: &mut TeamState,
    rays: &mut RayBudget,
    notes: &mut Vec<Note>,
) {
    let Due {
        now,
        view,
        me,
        tactics,
        tuning,
    } = *due;
    // With the battery cheat off, the batteries and spawners' spots it
    // sees become its team's knowledge.
    if !tuning.cheats.batteries && view.mode.has_batteries() {
        // Spawners only while some battery is missing.
        let missing = plays::batteries_unaccounted(view, bot.team, &state.knowledge) > 0;
        let worth: Vec<bool> = (0..tactics.world.map.battery_spawners.len())
            .map(|index| {
                missing
                    && plays::spawner_worth_checking(view, &state.knowledge, tactics, tuning, index)
            })
            .collect();
        sense::sense_batteries(
            me,
            view,
            &tactics.world.sight,
            &tactics.world.map.battery_spawners,
            &tuning.sense,
            &mut state.knowledge,
            |index| worth[index],
            rays,
        );
        for text in state.knowledge.battery_notes.drain(..) {
            bot.note(notes, now, text);
        }
    }
}

/// The memory takes the percepts, each after its notice time.
pub(super) fn remember(due: &Due, bot: &mut Bot, percepts: &[Percept]) {
    let Due {
        now,
        view,
        me,
        tuning,
        tactics,
    } = *due;
    let skill = bot.skill;
    let notice_factor = signals::mix(tuning.aim.notice, skill);
    let feet = me.feet;
    bot.memory.update(
        percepts,
        now,
        |p: &Percept| {
            let base = sense::notice_ms(&tuning.sense, p.at.distance_xz(feet), p.busy, notice_factor);
            if p.source != Source::Seen {
                return base;
            }
            // A sight that begins through bushes (`foliage.rs`) is noticed later.
            let metres = tactics
                .foliage
                .metres(me.eye(), p.at + Vec3::new(0.0, super::foliage::CHEST_M, 0.0));
            let delayed = super::foliage::noticing_ms(base, metres, tuning.sense.bush_notice, tuning.sense.foliage_cap);
            #[cfg(test)]
            super::foliage::through::count(metres, delayed - base);
            if tuning.sense.grass_notice <= 0.0 {
                return delayed;
            }
            // And through grass (`grass.rs`): the chest of the body as he is, standing or crouched.
            let chest = match view.place(p.who) {
                Some(body) if body.crouched() => super::grass::CROUCHED_CHEST_M,
                _ => super::foliage::CHEST_M,
            };
            let hidden = tactics.grass.metres(me.eye(), p.at + Vec3::new(0.0, chest, 0.0));
            let delayed_in_grass = super::foliage::noticing_ms_hidden(
                base,
                tuning.sense.bush_notice * metres + tuning.sense.grass_notice * hidden,
                tuning.sense.foliage_cap,
            );
            #[cfg(test)]
            super::grass::through::count(hidden, delayed_in_grass - delayed);
            delayed_in_grass
        },
        tuning.cheats.forget_ms.max(tuning.sense.forget_ms),
        tuning.sense.drop_ms,
    );
}

/// The reflex thinks (`Reflex::think_with`) on what the bot remembers; its throw notes and the hit it felt from a direction alone.
pub(super) fn react(
    due: &Due,
    bot: &mut Bot,
    state: &mut TeamState,
    weapons: &[Option<reflex::WeaponKind>],
    notes: &mut Vec<Note>,
) {
    let Due {
        now,
        view,
        me,
        tactics,
        tuning,
    } = *due;
    let skill = bot.skill;
    bot.memory.set_sight_target(bot.reflex.target());
    let sit = reflex::Situation {
        view,
        tactics: Some(tactics),
        team: Some(&state.knowledge),
        side: bot.side,
        partner: bot.partner,
        weapon: bot.weapon(weapons, tuning),
        weapons,
        retreat: tuning.avoid.grenade_avoid.then_some(reflex::Retreat {
            margin_m: tuning.avoid.margin_m,
            told_ms: tuning.avoid.told_ms.1,
        }),
    };
    bot.reflex
        .think_with(me, &sit, &bot.memory, now, &tuning.aim, skill, &mut bot.rng);
    for text in bot.reflex.take_throw_notes() {
        bot.note(notes, now, text);
    }
    // A grenade decided on: its place is the team's to skip for a while.
    if let Some(at) = bot.reflex.take_throw_claim() {
        state.knowledge.claim_throw(bot.place, at, now);
    }
    // Hit from a direction alone (the HUD's marker): its callout.
    if let Some((at, bearing)) = bot.reflex.take_hit_note() {
        state.knowledge.shots_from.push(know::ShotFrom {
            by: bot.place,
            at,
            bearing,
            time: now + tuning.sense.callout_ms,
        });
    }
}

/// The bench's stand-in camper: to its spot and held there, standing, whatever its play; the reflex fires on sight.
/// True when the bot is the camper and found its spot (its intent is set).
#[cfg(test)]
pub(super) fn bench_camper(
    due: &Due,
    bot: &mut Bot,
    slot: &mut Option<(Place, Option<tests::CamperSpot>)>,
) -> bool {
    let Due { tactics, me, .. } = *due;
    if let Some((camper, spot)) = slot.as_mut()
        && *camper == bot.place
    {
        let side = bot.side;
        let options = tests::CamperOptions::from_env();
        let found = *spot.get_or_insert_with(|| {
            side.and_then(|side| tests::camper_spot_by(tactics, side, options.rule, options.crouch))
        });
        if let Some((at, _)) = found {
            bot.bench_camper = true;
            bot.intent = Intent {
                go: Go::To {
                    at,
                    within_m: tests::CAMPER_WITHIN_M,
                },
                pace: Pace::Run,
                // Crouched once there, if it is to be.
                crouch: options.crouch && me.feet.distance_xz(at) <= tests::CAMPER_WITHIN_M,
                ..Intent::stay()
            };
            return true;
        }
    }
    false
}

/// The last resort ([`stuck::SelfKill`]): a bot with nothing left to try, 60 s on, in no fight and carrying nothing, kills itself
/// (`Encoder::self_kill`), as the owner decided; the facts are read here once, the rules are `stuck::self_kill_step`'s.
pub(super) fn self_kill(due: &Due, bot: &mut Bot, state: &mut TeamState, notes: &mut Vec<Note>) {
    let Due {
        now,
        view,
        me,
        tuning,
        ..
    } = *due;
    if tuning.unstick.suicide_after_ms.is_none() && bot.self_kill == stuck::SelfKill::default() {
        return;
    }
    state.features.stuck.self_kills.retain(|&at| now < at.saturating_add(60_000));
    // The last time an enemy was sensed by the bot's own senses (a cheat's place does not count), or a bullet hit it.
    let sensed = bot
        .memory
        .tracks()
        .iter()
        .filter_map(|track| track.own.map(|(_, at)| at))
        .chain(bot.hurt.last_hit_ms)
        .max();
    let facts = stuck::Facts {
        now,
        hopeless: bot.stuck.hopeless(now, me.spawned),
        quiet_ms: sensed.map_or(u32::MAX, |at| now.saturating_sub(at)),
        life_ms: now.saturating_sub(me.spawned),
        match_ms: ((view.total_s - view.time_left_s).max(0.0) * 1_000.0) as u32,
        carrying: me.carrying.is_some(),
        tickets: view.mode.is_deathmatch().then(|| view.points[usize::from(bot.team & 1)]),
        since_last_ms: bot.self_killed_at.map(|at| now.saturating_sub(at)),
        team_last_minute: state.features.stuck.self_kills.len() as u32,
    };
    let (next, act) = stuck::self_kill_step(bot.self_kill, &facts, &tuning.unstick);
    #[cfg(test)]
    {
        if matches!((bot.self_kill, next), (stuck::SelfKill::Sent { .. }, stuck::SelfKill::Barred)) {
            stuck::self_kills::count(2);
        }
    }
    bot.self_kill = next;
    let Some(stuck::SelfKillAct::Send { again }) = act else {
        return;
    };
    bot.encoder.self_kill(tuning.unstick.suicide_ticks);
    if !again {
        bot.self_killed_at = Some(now);
        state.features.stuck.self_kills.push(now);
    }
    #[cfg(test)]
    stuck::self_kills::count(usize::from(again));
    let (since, why) = facts.hopeless.map_or((now, String::new()), |h| (h.since, format!("{:?}", h.why)));
    bot.note(
        notes,
        now,
        format!(
            "self_kill{}: nothing left to try for {} s ({why}) at {:?}, life {} s, match {} s, no enemy or hit for {}",
            if again { " again" } else { "" },
            now.saturating_sub(since) / 1_000,
            me.feet.to_array(),
            facts.life_ms / 1_000,
            facts.match_ms / 1_000,
            if facts.quiet_ms == u32::MAX { "the life".to_owned() } else { format!("{} s", facts.quiet_ms / 1_000) },
        ),
    );
}

/// The play's step: its intent, or the end of the play (a goal its own search proved no way to, done, failed) for the commander to
/// choose again. Gives the step's name for the trace; none when the bot has no play running.
pub(super) fn decide(
    due: &Due,
    bot: &mut Bot,
    state: &mut TeamState,
    memo: &plays::memo::Memo,
    weapons: &[Option<reflex::WeaponKind>],
    notes: &mut Vec<Note>,
) -> Option<String> {
    let Due {
        now,
        view,
        me,
        tactics,
        tuning,
    } = *due;
    // The play's step.
    let Some(mut running) = bot.running.take() else {
        bot.intent = Intent::stay();
        return None;
    };
    let target = bot
        .assignment
        .as_ref()
        .map_or(plays::TargetRef::None, |a| a.target);
    let cx = Ctx {
        now,
        view,
        me,
        bot: BotFacts {
            personality: bot.personality(tuning),
            weapon: bot.weapon(weapons, tuning),
            side: bot.side,
            current: bot.assignment.as_ref().map(|a| (a.kind, a.target, a.since)),
            last_ran: &bot.last_ran,
            partner: bot.partner,
            paired: bot.paired,
            medkits_used: bot.features.stock.medkits_used,
            pantry: &bot.features.stock.pantry,
            pools: bot.features.stock.pools,
            zone_armour: &bot.features.crossing.armour,
            no_way_to: &bot.no_way_to,
            set_aside: &bot.features.memory.set_aside,
            failed_stands: &bot.features.memory.failed_stands,
            dry: bot.reflex.dry(),
            ammo: bot.ammo(),
            crossing: walks_crossing(bot),
            held_choke: bot.held_choke,
        },
        memory: &bot.memory,
        team: &state.knowledge,
        sig: &state.signals,
        mods: &state.mods,
        tactics,
        tuning,
        memo,
    };
    let mut rng = bot.rng.clone();
    #[cfg(test)]
    super::hold_trace::at(me.feet);
    let step = plays::step(&mut running, target, &cx, &mut rng);
    // A hold on a spot a teammate of a lower place has claimed gives way
    // (S1); the point it means to be at is its claim.
    let (step, claim) = match step {
        Step::Continue(intent) => {
            let (intent, claim) = plays::goals::keep_apart(&cx, &mut bot.features.memory.apart, running.kind(), intent);
            (Step::Continue(intent), Some(claim))
        }
        other => {
            bot.features.memory.apart.reset();
            (other, None)
        }
    };
    bot.rng = rng;
    let step_name = running.step_name();
    bot.running = Some(running);
    if let Some(at) = claim {
        state.knowledge.claim(me.place, at, now);
    }
    match step {
        // Its goal one its own search proved no way to lately: the play
        // is left, set aside while the proof holds, for the commander to
        // choose again (not stood at the goal, searched every 500 ms).
        Step::Continue(intent) if goal_shut(&intent, me, &bot.no_way_to, now) => {
            if let Some(a) = bot.assignment.as_ref() {
                bot.features.memory.set_aside.retain(|&(_, _, until)| until > now);
                bot.features.memory.set_aside.push((a.kind, a.target, now + NO_WAY_KEEP_MS));
                // A goal on a floor of a zone is the zone's no way, the same for every bot of the team.
                if tuning.hazard.team_zone_aside && goal_in_zone(&intent, tactics) {
                    state.knowledge.set_aside(a.kind, a.target, now + NO_WAY_KEEP_MS);
                }
            }
            bot.end_play(now, "no way to its goal", notes);
            state.next_commander = now;
        }
        Step::Continue(mut intent) => {
            bot.press |= intent.press;
            if let Some(zone) = intent.resupplied.take() {
                bot.resupplies += 1;
                let rounds = bot.top_up();
                bot.note(notes, now, format!("resupplied at zone {zone}: +{rounds} rounds"));
            }
            if let Some(failed) = intent.failed_stand.take() {
                bot.features.memory.failed_stands.retain(|f| f.until > now);
                bot.features.memory.failed_stands.push(failed);
                bot.note(notes, now, failed.note());
            }
            bot.intent = intent;
        }
        Step::Done(why) => {
            bot.end_play(now, why, notes);
            state.next_commander = now;
        }
        Step::Failed(why) => {
            if let Some(a) = bot.assignment.as_ref()
                && (now < a.since + plays::QUICK_FAIL_MS || plays::collect::fails_aside(why))
            {
                bot.features.memory.set_aside.retain(|&(_, _, until)| until > now);
                // A battery proved out of reach is left as long as a goal no
                // way leads to.
                let keep = if plays::collect::fails_aside(why) {
                    NO_WAY_KEEP_MS
                } else {
                    plays::SET_ASIDE_MS
                };
                bot.features.memory.set_aside.push((a.kind, a.target, now + keep));
            }
            bot.end_play(now, why, notes);
            state.next_commander = now;
        }
    }
    Some(step_name)
}

/// Whether the intent's goal lies on a floor a damage zone marks (baked or live): where routes keep off, so no bot's
/// search finds a way.
fn goal_in_zone(intent: &Intent, tactics: &Tactics) -> bool {
    let Go::To { at, .. } = intent.go else {
        return false;
    };
    tactics
        .world
        .grid
        .locate(at, 1.5)
        .is_some_and(|node| tactics.hazards.marked_at(node))
}

#[cfg(test)]
mod phase_tests {
    use super::*;

    use crate::v026g::bots::know::{Percept, SIGHT_GRACE_MS};
    use crate::v026g::bots::view::Mode;

    fn enemy(place: Place, spawned: u32) -> PlaceView {
        PlaceView {
            place,
            team: 1,
            bot: true,
            present: true,
            alive: true,
            feet: Vec3::new(0.0, 0.0, 20.0),
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health: 1.0,
            spawned,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        }
    }

    /// The bot itself, place 0 (the view is indexed by place).
    fn us() -> PlaceView {
        PlaceView {
            team: 0,
            ..enemy(0, 0)
        }
    }

    fn view_at(clock: u32, places: Vec<PlaceView>) -> MatchView {
        MatchView {
            clock,
            mode: Mode::TeamDeathmatch,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places,
            batteries: vec![],
            marks: vec![],
            containers: vec![],
            sounds: vec![],
            deaths: vec![],
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
            grenades: vec![],
        }
    }

    /// Two enemies seen at 1 s, neither the target.
    fn two_sights() -> Memory {
        let mut memory = Memory::default();
        memory.set_sight_window(know::SightWindow::refreshed());
        let seen = |who| Percept {
            who,
            at: Vec3::new(0.0, 0.0, 20.0),
            error_m: 0.0,
            time: 1_000,
            source: Source::Seen,
            busy: false,
        };
        memory.update(&[seen(1), seen(2)], 1_000, |_| 0, 3_000, 20_000);
        memory
    }

    /// A think whose look found a non-target gone ends his sight after the
    /// grace, not at once; one the rays ran out on keeps it to the hold; one
    /// found is left to the percept; and the one that died, or is another
    /// life, ends at once.
    #[test]
    fn a_thinks_failed_look_ends_a_sight_after_its_grace_and_an_untested_one_does_not() {
        let mut memory = two_sights();
        let live: Vec<_> = memory.live_sights().collect();
        assert_eq!(live, vec![(1, 1_000), (2, 1_000)]);
        let view = view_at(1_100, vec![us(), enemy(1, 0), enemy(2, 0)]);
        let looks = [
            (1, LookOutcome::Failed(Miss::Blocked)),
            (2, LookOutcome::Untested),
        ];
        tell_looks(&mut memory, &live, &looks, &view, Vec3::ZERO);
        let missed = |memory: &Memory, who| {
            memory.tracks().iter().find(|t| t.who == who).unwrap().missed
        };
        assert_eq!(missed(&memory, 1), Some((1_100, Miss::Blocked)));
        assert_eq!(missed(&memory, 2), None, "untested is not a negative");
        // (The grace shows on the target: a non-target is held to the hold.)
        memory.set_sight_target(Some(1));
        let last = 1_100 + SIGHT_GRACE_MS;
        assert!(memory.seen(1, last, last).is_some());
        assert!(memory.seen(1, last + 1, last + 1).is_none());
        // The untested one is held from his last test, no more than the
        // hold (the bot's next think is not told here).
        let hold = 1_000 + crate::v026g::bots::know::SIGHT_HOLD_MS;
        assert!(memory.seen(2, hold, hold).is_some());
        assert!(memory.seen(2, hold + 1, hold + 1).is_none());
        // A look that found him is the percept's.
        let before = memory.tracks().to_vec();
        tell_looks(&mut memory, &live, &[(2, LookOutcome::Seen)], &view, Vec3::ZERO);
        assert_eq!(memory.tracks()[1], before[1]);
        // One that died or lives another life ends at once; one the order
        // did not hold is left as it is.
        let mut memory = two_sights();
        let gone = view_at(1_100, vec![us(), enemy(1, 1_050)]);
        tell_looks(&mut memory, &live, &[(1, LookOutcome::Seen)], &gone, Vec3::ZERO);
        assert_eq!(memory.live_sights().count(), 0, "another life, and absent");
        let mut memory = two_sights();
        tell_looks(&mut memory, &live, &[], &view, Vec3::ZERO);
        assert_eq!(memory.live_sights().count(), 2);
    }

    fn bot(entered: Option<u32>, next_think: u32) -> (Bot, Tuning) {
        let tuning = Tuning::default();
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        bot.entered = entered;
        bot.next_think = next_think;
        (bot, tuning)
    }

    /// The kind of a bot's weapon 1 is the one its place was given (the kit it
    /// wears), not its role's name: an SMG rusher plays the SMG band
    /// ([`reflex::Situation::weapon`] feeds `preferred_band`); a place with
    /// none known falls back to the role's.
    #[test]
    fn an_smg_rusher_is_an_smg_whatever_its_role_is_called() {
        let tuning = Tuning::all_off();
        let rusher = Bot::new(1, 3, 0, Some("rusher"), &tuning);
        assert_eq!(tuning.personalities[rusher.personality].name, "rusher");
        let mut weapons = vec![None; 4];
        assert_eq!(
            rusher.weapon(&weapons, &tuning),
            Some(reflex::WeaponKind::Shotgun),
            "unknown: the role's"
        );
        weapons[3] = Some(reflex::WeaponKind::Smg);
        let kind = rusher.weapon(&weapons, &tuning);
        assert_eq!(kind, Some(reflex::WeaponKind::Smg));
        let c = &tuning.aim.combat;
        assert_eq!(c.band(kind.unwrap()), c.band(reflex::WeaponKind::Smg));
        assert_ne!(c.band(kind.unwrap()), c.band(reflex::WeaponKind::Shotgun));
    }

    #[test]
    fn a_bot_that_has_not_entered_or_whose_time_has_not_come_does_not_think_and_draws_nothing() {
        for (entered, next_think, now) in [(None, 0, 5_000), (Some(1_000), 6_000, 5_999)] {
            let (mut bot, tuning) = bot(entered, next_think);
            let before = format!("{:?}", bot.rng);
            assert!(!schedule(&mut bot, &tuning, now));
            assert_eq!(bot.next_think, next_think);
            assert_eq!(format!("{:?}", bot.rng), before, "no draw");
        }
    }

    #[test]
    fn a_due_think_draws_the_next_one_within_the_think_rate_with_one_draw() {
        let (mut bot, tuning) = bot(Some(1_000), 6_000);
        let mut expected = bot.rng.clone();
        let (low, high) = tuning.think.think_ms;
        assert!(schedule(&mut bot, &tuning, 6_000));
        assert_eq!(
            bot.next_think,
            6_000 + expected.range(low as f32, high as f32) as u32
        );
        assert!(
            (6_000 + low..=6_000 + high).contains(&bot.next_think),
            "{}",
            bot.next_think
        );
        assert_eq!(
            format!("{:?}", bot.rng),
            format!("{:?}", expected),
            "exactly one draw"
        );
    }
}
