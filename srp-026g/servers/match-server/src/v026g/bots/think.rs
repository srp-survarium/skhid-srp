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
        let tuning = self.tuning_for(index, team);
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
        // TEST ONLY: a bot a client test scripts plays no play (`client_tests`).
        #[cfg(any(test, feature = "client-test"))]
        if client_tests::script_bot(self.client_tests.as_ref(), bot, me) {
            return;
        }
        #[cfg(test)]
        if self.bench_camper.as_mut().is_some_and(|camper| camper.drive(&due, bot)) {
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
        let tuning = self.tuning_for(index, team);
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
                    {
                        bot.pace.learn_spots(&self.teams[usize::from(team)].knowledge, time, tuning.pace.ring_death_spots);
                        drive_tick(bot, &me, view, tactics, &tuning, time, searching)
                    }
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
            #[cfg(test)]
            {
                if bot.bench_mirror.len() == 8 {
                    bot.bench_mirror.pop_front();
                }
                bot.bench_mirror.push_back((time, bot.reflex.seen()));
            }
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
    // Smoke hides what is behind it, as the team's tuning reads it (`smoke.rs`).
    let veil = view.smoke.veil(view.clock, &tuning.smoke);
    let (percepts, looks) = sense::sense_looks_through(
        me,
        view,
        &tactics.world.sight,
        veil,
        &tuning.sense,
        &tuning.cheats,
        bot.heard_until,
        &order,
        rays,
        &mut bot.rng,
    );
    bot.heard_until = now;
    // A sight it had, lost to smoke: one note (the reflex's notes, taken at its think).
    for &(who, outcome) in &looks {
        if outcome == LookOutcome::Failed(Miss::Smoke) {
            let had = live.iter().any(|&(live, _)| live == who);
            if had {
                bot.reflex.note_smoke(format!("lost player {who} to smoke (a look)"));
            }
            #[cfg(test)]
            super::smoke::tally::add(
                bot.team,
                if had { super::smoke::tally::Event::SightLost } else { super::smoke::tally::Event::NotSeen },
            );
        }
    }
    for &(who, outcome) in &looks {
        if matches!(outcome, LookOutcome::Seen | LookOutcome::Failed(Miss::Blocked | Miss::Smoke)) {
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
            .call_out(bot.place, percept, tuning.sense.callout_ms, tuning.spacing.callouts);
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
    if !tuning.cheats.batteries && view.objective().has_homes() {
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
            view.smoke.veil(view.clock, &tuning.smoke),
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

/// [smoke] The clouds the bot sees join what it knows (`smoke::KnownClouds`); with `smoke.react_watch`, an enemy
/// cloud's near edge is glanced at while nothing else holds its look (one note a cloud).
fn smoke_react(due: &Due, bot: &mut Bot, notes: &mut Vec<Note>) {
    let Due {
        now,
        view,
        me,
        tactics,
        tuning,
    } = *due;
    let t = &tuning.smoke;
    // No reader of what the bot knows of clouds is on: nothing learnt, no ray cast.
    if !t.react_watch || view.smoke.is_empty() {
        return;
    }
    let (eye, feet, yaw, view_deg) = (me.eye(), me.feet, me.yaw, tuning.sense.view_deg);
    let who = super::smoke::Watcher {
        me: bot.place,
        team: bot.team,
        eye,
    };
    // A cloud is seen in the view, within `see_m`, along a clear line on the map no other cloud hides.
    let see_m = t.see_m.get();
    let veil = view.smoke.veil(view.clock, t);
    let _learnt = bot.clouds.update(who, &view.smoke, veil, view.clock, t, |middle| {
        eye.distance_xz(middle) <= see_m
            && super::sense::in_view(feet, yaw, middle, view_deg)
            && tactics.sees(eye, middle)
    });
    #[cfg(test)]
    for (source, &n) in super::smoke::CloudSource::ALL.iter().zip(&_learnt) {
        for _ in 0..n {
            super::smoke::tally::add(bot.team, super::smoke::tally::Event::learnt(*source));
        }
    }
    let Some(watch) = bot.clouds.edge_to_watch(who, feet, &view.smoke, view.clock, t) else {
        return;
    };
    let shown = bot.reflex.watch_smoke(watch.at, now, now + t.watch_ms);
    #[cfg(test)]
    super::smoke::tally::add(
        bot.team,
        if shown { super::smoke::tally::Event::EdgeWatch } else { super::smoke::tally::Event::WatchBusy },
    );
    if shown && watch.first {
        let at = watch.at;
        bot.note(
            notes,
            now,
            format!(
                "watches the edge of player {}'s smoke at ({:.1}, {:.1}, {:.1})",
                watch.key.by, at.x, at.y, at.z
            ),
        );
    }
}

/// [smoke] How far past a lane's crossing the open stretch is looked for, metres (`smoke.lane_open_m`'s test).
const LANE_OPEN_LOOK_M: f32 = 20.0;

/// [smoke use] A smoke from the kit for one of its uses, each its own switch (`SmokeTuning`): against the sniper that
/// outranges it (`anti_sniper`), or across a held lane its route enters ahead (`cross_lane`). Only against an enemy
/// the team sensed within [`super::smoke::FAIR_SENSED_MS`], with a smoke in the kit and the hands free; one note
/// per throw decided.
fn smoke_uses(due: &Due, bot: &mut Bot, knowledge: &mut know::TeamKnowledge, notes: &mut Vec<Note>) {
    use super::smoke::{self, SmokeUse};
    let Due {
        now,
        view,
        me,
        tactics,
        tuning,
    } = *due;
    let t = &tuning.smoke;
    // [diagnosis] Its anti-sniper plans whose crossing its route no longer runs by (anywhere along it, walked or ahead, within
    // the throw classes' 2.5 m): the plan changed after the throw.
    #[cfg(test)]
    {
        let why = match bot.last_route_end() {
            Some((super::route_end::RouteEnd::Replanned, _)) => bot.bench_replan,
            Some((why, _)) => why.name(),
            None => "none",
        };
        smoke::tally::sniper_routes(bot.place, now, why, |crossing| {
            bot.route.as_ref().is_some_and(|(route, _)| {
                let heights = bot.motor.heights();
                (0..=route.length() as usize).any(|k| route.at(k as f32, heights).distance_xz(crossing) <= 2.5)
            })
        });
    }
    // [diagnosis] Outranged by a sniper it saw within 15 s, whatever the kit and the hands.
    #[cfg(test)]
    if t.anti_sniper
        && bot
            .reflex
            .outranged_by()
            .is_some_and(|(_, _, seen)| view.clock.saturating_sub(seen) <= smoke::FAIR_SENSED_MS)
    {
        smoke::tally::add(bot.team, smoke::tally::Event::OutrangedAny);
    }
    let grenade = smoke::SmokeGrenade::RDG_M;
    let eye_over = Vec3::new(0.0, 1.6, 0.0);
    let chest_over = Vec3::new(0.0, 1.3, 0.0);
    // The first point of its route within `lane_ahead_m` that `exposed` holds for (2 m steps), and how far on.
    let crossing_of = |exposed: &dyn Fn(Vec3) -> bool| -> Option<(f32, Vec3)> {
        let (route, _) = bot.route.as_ref()?;
        let heights = bot.motor.heights();
        let start = route.walked();
        let end = (start + t.lane_ahead_m.get()).min(route.length());
        (1..).map(|k| start + k as f32 * 2.0).take_while(|&s| s <= end).map(|s| (s, route.at(s, heights))).find(|&(_, p)| exposed(p))
    };
    // [A208] Where a bot holds for a cloud short of the crossing `crossing_s` metres along its route: the last of
    // `crossing_of`'s samples before it that `seen` does not hold for (2 m steps back), its feet if none is ahead.
    let hold_short = |crossing_s: f32, seen: &dyn Fn(Vec3) -> bool| -> Vec3 {
        let Some((route, _)) = bot.route.as_ref() else {
            return me.feet;
        };
        let heights = bot.motor.heights();
        let start = route.walked();
        (1..)
            .map(|k| crossing_s - k as f32 * 2.0)
            .take_while(|&s| s > start)
            .map(|s| route.at(s, heights))
            .find(|&p| !seen(p))
            .unwrap_or(me.feet)
    };
    // [A208] Under a mate's cloud (`CloudWaiters::Crossers`): a bot, a carrier too, whose route enters a held lane ahead
    // that a team cloud in the air or forming will hide but does not hide yet holds short of it until it does.
    if t.wait_for_cloud.is_some_and(|w| w.who == super::tuning::CloudWaiters::Crossers)
        && !bot.reflex.holds_for_cloud()
        && !bot.smoke_threat.is_empty()
        && !bot.lanes.is_empty()
        && view.smoke.all().iter().any(|c| {
            let at = smoke::MatchMs(now);
            c.team == bot.team && at < c.ends() && at < c.pours.plus(smoke::Forming::of(t).ms(&c.shape))
        })
    {
        let grid = &tactics.world.grid;
        let exposed = |p: Vec3| bot.smoke_threat.exposed_at(grid, p);
        if !exposed(me.feet)
            && let Some((crossing_s, crossing)) = crossing_of(&exposed)
        {
            let chest = crossing + chest_over;
            let now_there = |sensed: u32| view.clock.saturating_sub(sensed) <= t.lane_sensed_ms;
            let planning = view.smoke.planning(view.clock, bot.team, smoke::Forming::of(t));
            let veil = view.smoke.veil(view.clock, t);
            let held = bot
                .lanes
                .iter()
                .filter(|l| l.sensed.is_some_and(now_there) && l.at.distance_xz(crossing) <= l.reach_m)
                .filter(|l| tactics.sees(l.at + eye_over, chest))
                .min_by(|a, b| a.at.distance_xz(crossing).total_cmp(&b.at.distance_xz(crossing)));
            if let Some(held) = held
                && planning.covers(view.clock, t, held.at + eye_over, chest)
                && !veil.hides(held.at + eye_over, chest)
            {
                let line = smoke::SmokeLine { eye: held.at + eye_over, target: chest, crossing };
                let hold = hold_short(crossing_s, &exposed);
                if let Some(w) = smoke::CloudWait::of(t, &grenade, line, Some(held.shooter), smoke::WaitsFor::Mates, hold)
                    && bot.reflex.hold_for_cloud(w, me, now)
                {
                    bot.note(
                        notes,
                        now,
                        format!(
                            "holds for a mate's cloud short of the lane of player {} at ({:.1}, {:.1}, {:.1}), at ({:.1}, {:.1}, {:.1})",
                            held.shooter, held.at.x, held.at.y, held.at.z, hold.x, hold.y, hold.z
                        ),
                    );
                    return;
                }
            }
        }
    }
    // A carrier throws nothing: the battery is in its hands, and the client ignores the quick keys then
    // (`process_quick_slots` 0x2D8BB0: the hands object's byte +0Eh, 0 for a battery, stops the quick pockets and the
    // grenade key; observed, LIKELY for the battery, as for the heal habit).
    if !(t.cross_lane || t.anti_sniper) || !bot.reflex.can_smoke() || me.carrying.is_some() {
        return;
    }
    let reach = smoke::Reach {
        mesh: &tactics.world.sight,
        grenade,
        thrower_eye: me.eye(),
    };
    let thin = t.thin_m.get();
    // Smoke there already: with `spot_rules` every cloud live or forming and the team's own in the air, at full size.
    let planning = view.smoke.planning(view.clock, bot.team, smoke::Forming::of(t));
    let smoke_there = |eye: Vec3, point: Vec3| {
        let there = if t.spot_rules { planning.covers(view.clock, t, eye, point) } else { view.smoke.covers(view.clock, t, eye, point) };
        #[cfg(test)]
        if there && t.spot_rules {
            smoke::tally::add(bot.team, smoke::tally::Event::CloudThere);
        }
        there
    };
    // Not running a gap already: a bot holding a sprint is across before the cloud forms (0.7 s), as a player sprinting
    // a gap does not stop to smoke it.
    let sprinting = bot.sent_keys & (key::FORWARD | key::SPRINT) == key::FORWARD | key::SPRINT;
    // A short gap is sprinted, not smoked: the route a shooter at `eye` sees from the crossing on, 1 m steps, must run at
    // least `lane_open_m` (what a sprint covers in a shooter's answer).
    let open_from = |eyes: &[Vec3], crossing_s: f32| -> bool {
        let Some((route, _)) = bot.route.as_ref() else {
            return false;
        };
        let heights = bot.motor.heights();
        let open_end = (route.length() - crossing_s).min(LANE_OPEN_LOOK_M);
        let open_m = (1..)
            .map(|k| k as f32)
            .take_while(|&d| d <= open_end)
            .take_while(|&d| {
                let p = route.at(crossing_s + d, heights) + chest_over;
                eyes.iter().all(|&eye| tactics.sees(eye, p))
            })
            .last()
            .unwrap_or(0.0);
        open_m >= t.lane_open_m.get()
    };
    // Against a sniper that outranges its weapon (play 11's matchup), sensed where he is within `lane_sensed_ms`: before
    // its route enters ground he sees, the line from him to the crossing. Not from where he already sees it.
    // Not a sniper the kill feed showed dead since (or just before) it sensed him: a human reads the feed and smokes no dead
    // man's line. Without this, 2 of 23 anti-sniper smokes went against a sniper already dead (`h2h-sniper-final-20261030T0807`,
    // class d); a death before the window is an earlier life of his.
    let outranged = bot.reflex.outranged_by().filter(|&(who, _, seen)| {
        view.clock.saturating_sub(seen) <= t.lane_sensed_ms && !smoke::died_since(&view.deaths, who, seen, t.lane_sensed_ms)
    });
    let sniper_crossing = outranged.filter(|_| t.anti_sniper || cfg!(test)).and_then(|(who, at, _)| {
        // His eye: where he was sensed, at the eye height of the posture the bot saw him in (`view::eye_height`), not a
        // standing guess. With a flat 1.6 m (over the game's 1.46 m standing, 1.04 m crouched) 13 of 40 smokes were thrown onto
        // crossings he had no line on (`h2h-sniper-vs-off-20261030T0220`, the throw classes).
        let seen_crouched = view.place(who).is_some_and(|him| him.crouched());
        let up = view.place(who).map_or(eye_over.y, |_| super::view::eye_height(seen_crouched, 0.0, 0.0));
        let eye = at + Vec3::new(0.0, up, 0.0);
        // Where he is is known to a few decimetres (the sensing's lag and the view's), and a crossing seen through a sliver
        // from one point is not his: it is his when he sees it from every eye of that spread (`sniper_eye_spread_m` to either
        // side across the line to the bot), and the bot is in his sight already when any of them sees it. From one eye, 11 of 56
        // smokes went onto crossings his eye in the game had no line on, all 11 lines open from the sensed eye and shut from
        // his, 0.1-1.0 m aside (`h2h-sniper-fresh-20261030T0339`).
        // And what he can do within the sensing's window: step that far nearer or farther along the line, or crouch (the
        // three cases left with the side spread alone, `h2h-sniper-fresh-20261030T0415`: 0.9 m along the line, a crouch since
        // seen, and a 1.5 m move no spread covers).
        let (dx, dz) = (me.feet.x - at.x, me.feet.z - at.z);
        let length = dx.hypot(dz).max(1e-3);
        let spread = t.sniper_eye_spread_m.get();
        let side = Vec3::new(-dz / length, 0.0, dx / length) * spread;
        let along = Vec3::new(dx / length, 0.0, dz / length) * spread;
        let low = Vec3::new(0.0, super::view::eye_height(true, 0.0, 0.0) - up, 0.0);
        let crouch = if seen_crouched { Vec3::ZERO } else { low };
        let eyes = [eye, eye + side, eye - side, eye + along, eye - along, eye + crouch];
        if eyes.iter().any(|&e| tactics.sees(e, me.feet + chest_over)) {
            return None;
        }
        if sprinting {
            #[cfg(test)]
            smoke::tally::add(bot.team, smoke::tally::Event::SprintSkip);
            return None;
        }
        crossing_of(&|p| eyes.iter().all(|&e| tactics.sees(e, p + chest_over))).map(|(s, crossing)| (who, eyes, s, crossing))
    });
    #[cfg(test)]
    if let Some((who, ..)) = sniper_crossing {
        smoke::tally::sniper_crossing(bot.place, who, view.clock);
    }
    let sniper = || {
        if !t.anti_sniper || bot.reflex.sniper_smokes() >= t.snipers_a_life {
            return None;
        }
        let (who, eyes, crossing_s, crossing) = sniper_crossing?;
        let eye = eyes[0];
        let chest = crossing + chest_over;
        if smoke_there(eye, chest) || !open_from(&eyes, crossing_s) {
            return None;
        }
        #[cfg(test)]
        smoke::tally::sniper_candidate(bot.place, who, eye, crossing);
        smoke::smoke_between(&reach, eye, crossing, thin, SmokeUse::AntiSniper).map(|plan| {
            let why = format!(
                "player {who}, a sniper at ({:.1}, {:.1}, {:.1}) out of its reach, crossing at ({:.1}, {:.1}, {:.1})",
                eye.x, eye.y - eye_over.y, eye.z, crossing.x, crossing.y, crossing.z
            );
            // [A208] Short of it where none of his eyes sees the chest.
            let hold = t.wait_for_cloud.map(|_| hold_short(crossing_s, &|p| eyes.iter().any(|&e| tactics.sees(e, p + chest_over))));
            (plan, why, eye, hold, Some(who))
        })
    };
    let sniper = sniper();
    #[cfg(test)]
    if t.anti_sniper && outranged.is_some() {
        smoke::tally::add(
            bot.team,
            if sniper.is_some() { smoke::tally::Event::Outranged } else { smoke::tally::Event::OutrangedNoPlan },
        );
    }
    // Across the first held lane its route enters ahead, before it steps in.
    let lane = || {
        if !t.cross_lane || bot.smoke_threat.is_empty() || bot.lanes.is_empty() || bot.reflex.lane_smokes() >= t.lanes_a_life {
            return None;
        }
        let grid = &tactics.world.grid;
        // Not in the lane already, and not running it already.
        if bot.smoke_threat.exposed_at(grid, me.feet) {
            return None;
        }
        if sprinting {
            #[cfg(test)]
            smoke::tally::add(bot.team, smoke::tally::Event::SprintSkip);
            return None;
        }
        let (crossing_s, crossing) = crossing_of(&|p| bot.smoke_threat.exposed_at(grid, p))?;
        // The shooter known to be there now, with a line onto the crossing.
        let chest = crossing + chest_over;
        let now_there = |sensed: u32| view.clock.saturating_sub(sensed) <= t.lane_sensed_ms;
        let held = bot
            .lanes
            .iter()
            .filter(|l| l.sensed.is_some_and(now_there) && l.at.distance_xz(crossing) <= l.reach_m)
            .filter(|l| tactics.sees(l.at + eye_over, chest))
            .min_by(|a, b| a.at.distance_xz(crossing).total_cmp(&b.at.distance_xz(crossing)))?;
        // A lane a cloud already covers (a mate's smoke, in plain sight) needs no second one.
        if smoke_there(held.at + eye_over, chest) {
            return None;
        }
        if !open_from(&[held.at + eye_over], crossing_s) {
            return None;
        }
        smoke::smoke_between(&reach, held.at + eye_over, crossing, thin, SmokeUse::CrossLane).map(|plan| {
            let why = format!(
                "the lane of player {} at ({:.1}, {:.1}, {:.1}), crossing at ({:.1}, {:.1}, {:.1})",
                held.shooter, held.at.x, held.at.y, held.at.z, crossing.x, crossing.y, crossing.z
            );
            // [A208] Short of it, out of the held lanes.
            let hold = t.wait_for_cloud.map(|_| hold_short(crossing_s, &|p| bot.smoke_threat.exposed_at(grid, p)));
            (plan, why, held.at + eye_over, hold, Some(held.shooter))
        })
    };
    let Some((plan, why, against, hold, threat)) = sniper.or_else(lane) else {
        return;
    };
    // A mate's live plan near its landing or against the same shooter: none of its own.
    if t.spot_rules && knowledge.smoke_claimed(bot.place, plan.at, against, now, t.claim_m.get(), t.claim_ms) {
        #[cfg(test)]
        smoke::tally::add(bot.team, smoke::tally::Event::ClaimRefused);
        return;
    }
    // [A208] Its cloud first: the hold after the launch (`smoke.wait_for_cloud`).
    let wait = hold.and_then(|hold| smoke::CloudWait::of(t, &reach.grenade, plan.line, threat, smoke::WaitsFor::Own(plan.for_use), hold));
    if bot.reflex.start_smoke(plan, now, wait) {
        if t.spot_rules {
            knowledge.claim_smoke(know::SmokeClaim { who: bot.place, at: plan.at, against, time: now });
        }
        #[cfg(test)]
        {
            smoke::tally::set_role(bot.role.map_or(0, |role| role as usize));
            smoke::tally::begun(bot.team, plan.for_use);
            if plan.for_use == SmokeUse::AntiSniper {
                smoke::tally::sniper_smoked(bot.place);
                smoke::tally::sniper_begun(bot.place, bot.team, now);
            }
        }
        let at = plan.at;
        bot.note(
            notes,
            now,
            format!(
                "decides a smoke for {:?}: {why}; to land at ({:.1}, {:.1}, {:.1}), covering {:.1} m of the line",
                plan.for_use, at.x, at.y, at.z, plan.covers_m
            ),
        );
    }
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
        cover: tuning.cover.rule(),
    };
    // A bullet that hit it, a limb's too (the hit marker and the stagger), as the world last showed.
    bot.reflex.bullet_hit(bot.hurt.last_hit_ms);
    bot.reflex
        .think_with(me, &sit, &bot.memory, now, &tuning.aim, skill, &mut bot.rng);
    for text in bot.reflex.take_throw_notes() {
        bot.note(notes, now, text);
    }
    for text in bot.reflex.take_smoke_notes() {
        bot.note(notes, now, text);
    }
    smoke_react(due, bot, notes);
    for text in bot.reflex.take_worth_notes() {
        bot.note(notes, now, text);
    }
    // A push on a hurt enemy: the team's cap on it counts this bot while it goes on, and not after.
    match bot.reflex.pushing() {
        Some((victim, until)) => state.knowledge.wounds.push_claim(victim, bot.place, until),
        None => state.knowledge.wounds.push_release(bot.place),
    }
    smoke_uses(due, bot, &mut state.knowledge, notes);
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
        tickets: view.objective().tickets(view, bot.team),
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
    #[cfg(test)]
    bot.stuck.drop_watch_self_kill();
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
            weapons,
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
            hit: bot.reflex.last_hit(),
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
    // [A192] The bench's Crossfire arrivals (a note as its walk ends at the spot).
    #[cfg(test)]
    let crossfire_going = running.kind() == PlayKind::Crossfire && running.step_name() == "Going";
    let step = plays::step(&mut running, target, &cx, &mut rng);
    // A hold on a spot a teammate of a lower place has claimed gives way
    // (S1); the point it means to be at is its claim.
    let (step, claim) = match step {
        Step::Continue(intent) => {
            let (intent, claim) = plays::goals::keep_apart(&cx, &mut bot.features.memory.apart, running.kind(), intent);
            // [threat.walk_watch] A229: a walk with a known threat ahead looks at it.
            let intent = plays::watch::walk_watch(&cx, intent);
            (Step::Continue(intent), Some(claim))
        }
        other => {
            bot.features.memory.apart.reset();
            (other, None)
        }
    };
    bot.rng = rng;
    let step_name = running.step_name();
    #[cfg(test)]
    if crossfire_going && step_name == "Hold" {
        plays::crossfire::bench::arrived(bot.place, now);
    }
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
            smoke: Default::default(),
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
