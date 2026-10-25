use super::*;

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
        if bot.entered.is_none() || now < bot.next_think {
            return;
        }
        bot.next_think = now
            + bot.rng.range(
                tuning.think.think_ms.0 as f32,
                tuning.think.think_ms.1 as f32,
            ) as u32;
        let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
            return;
        };
        let team = usize::from(bot.team & 1);
        // Senses: sight (the order turns each think, so no enemy waits
        // long for a ray), hearing, the cheats, the team's callouts.
        let mut order: Vec<Place> = view.enemies_of(bot.team).map(|p| p.place).collect();
        if !order.is_empty() {
            let k = (now / 100) as usize % order.len();
            order.rotate_left(k);
        }
        let mut percepts = sense::sense(
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
        let quiet = percepts.is_empty() && bot.memory.all_seen(now).is_empty();
        let state = &mut self.teams[team];
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
            && self
                .tracker
                .body(bot.place)
                .and_then(|body| body.last_error())
                .is_some_and(|error| error > REPLAN_ERROR_M)
        {
            state.knowledge.fell_near(bot.place, at, now);
        }
        percepts.extend(state.knowledge.heard_by(bot.place, now));
        // With the battery cheat off, the batteries and spawners' spots it
        // sees become its team's knowledge.
        if !tuning.cheats.batteries && view.mode.has_batteries() {
            // Spawners only while some battery is missing.
            let missing = plays::batteries_unaccounted(view, bot.team, &state.knowledge) > 0;
            let worth: Vec<bool> = (0..tactics.world.map.battery_spawners.len())
                .map(|index| {
                    missing
                        && plays::spawner_worth_checking(
                            view,
                            &state.knowledge,
                            tactics,
                            &tuning,
                            index,
                        )
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
        let skill = bot.skill;
        let notice_factor = signals::mix(tuning.aim.notice, skill);
        let feet = me.feet;
        bot.memory.update(
            &percepts,
            now,
            |p: &Percept| {
                sense::notice_ms(&tuning.sense, p.at.distance_xz(feet), p.busy, notice_factor)
            },
            tuning.cheats.forget_ms.max(tuning.sense.forget_ms),
            tuning.sense.drop_ms,
        );
        let sit = reflex::Situation {
            view,
            tactics: Some(tactics),
            team: Some(&self.teams[team].knowledge),
            side: bot.side,
            partner: bot.partner,
            weapon: reflex::WeaponKind::of_role(&bot.personality(&tuning).name),
            weapons: &self.weapons,
        };
        bot.reflex
            .think_with(me, &sit, &bot.memory, now, &tuning.aim, skill, &mut bot.rng);
        for text in bot.reflex.take_throw_notes() {
            bot.note(notes, now, text);
        }
        // Hit from a direction alone (the HUD's marker): its callout.
        if let Some((at, bearing)) = bot.reflex.take_hit_note() {
            self.teams[team].knowledge.shots_from.push(know::ShotFrom {
                by: bot.place,
                at,
                bearing,
                time: now + tuning.sense.callout_ms,
            });
        }
        if now < bot.pause_until {
            bot.intent = Intent::stay();
            return;
        }
        // The bench's stand-in camper: to its spot and held there,
        // standing, whatever its play; the reflex fires on sight.
        #[cfg(test)]
        if let Some((camper, spot)) = self.bench_camper.as_mut()
            && *camper == bot.place
        {
            let side = bot.side;
            let found = *spot.get_or_insert_with(|| side.and_then(|side| tests::camper_spot(tactics, side)));
            if let Some((at, _)) = found {
                bot.bench_camper = true;
                bot.intent = Intent {
                    go: Go::To {
                        at,
                        within_m: tests::CAMPER_WITHIN_M,
                    },
                    pace: Pace::Run,
                    ..Intent::stay()
                };
                return;
            }
        }
        // The play's step.
        let state = &self.teams[team];
        let Some(mut running) = bot.running.take() else {
            bot.intent = Intent::stay();
            return;
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
                personality: bot.personality(&tuning),
                side: bot.side,
                current: bot.assignment.as_ref().map(|a| (a.kind, a.target, a.since)),
                last_ran: &bot.last_ran,
                partner: bot.partner,
                paired: bot.paired,
                medkits_used: bot.medkits_used,
                no_way_to: &bot.no_way_to,
                set_aside: &bot.set_aside,
                failed_stands: &bot.failed_stands,
                dry: bot.reflex.dry(),
                crossing: walks_crossing(bot),
            },
            memory: &bot.memory,
            team: &state.knowledge,
            sig: &state.signals,
            mods: &state.mods,
            tactics,
            tuning: &tuning,
            memo: &self.memo,
        };
        let mut rng = bot.rng.clone();
        let step = plays::step(&mut running, target, &cx, &mut rng);
        bot.rng = rng;
        let step_name = running.step_name();
        bot.running = Some(running);
        match step {
            // Its goal one its own search proved no way to lately: the play
            // is left, set aside while the proof holds, for the commander to
            // choose again (not stood at the goal, searched every 500 ms).
            Step::Continue(intent) if goal_shut(&intent, me, &bot.no_way_to, now) => {
                if let Some(a) = bot.assignment.as_ref() {
                    bot.set_aside.retain(|&(_, _, until)| until > now);
                    bot.set_aside.push((a.kind, a.target, now + NO_WAY_KEEP_MS));
                }
                bot.end_play(now, "no way to its goal", notes);
                self.teams[team].next_commander = now;
            }
            Step::Continue(mut intent) => {
                bot.press |= intent.press;
                if let Some(failed) = intent.failed_stand.take() {
                    bot.failed_stands.retain(|f| f.until > now);
                    bot.failed_stands.push(failed);
                    bot.note(notes, now, failed.note());
                }
                bot.intent = intent;
            }
            Step::Done(why) => {
                bot.end_play(now, why, notes);
                self.teams[team].next_commander = now;
            }
            Step::Failed(why) => {
                if let Some(a) = bot.assignment.as_ref()
                    && now < a.since + plays::QUICK_FAIL_MS
                {
                    bot.set_aside.retain(|&(_, _, until)| until > now);
                    bot.set_aside
                        .push((a.kind, a.target, now + plays::SET_ASIDE_MS));
                }
                bot.end_play(now, why, notes);
                self.teams[team].next_commander = now;
            }
        }
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
    pub(super) fn ticks(&mut self, index: usize, clock: &Clock, view: &MatchView, made: &mut Vec<Made>) {
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
        let searching = &mut self.searching;
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
            let decision = match (estimate, me, self.tactics.as_ref()) {
                (Some(at), Some(me), Some(tactics)) if clock.playing => {
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
