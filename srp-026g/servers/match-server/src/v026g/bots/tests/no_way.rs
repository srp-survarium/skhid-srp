use super::*;

/// What one bot's search proved no way to, its team leaves for
/// `TEAM_NO_WAY_KEEP_MS` from the proof (in the owner's match of
/// 2026-10-16 six bots learnt one battery each on its own, 183 takes); a
/// second proof keeps it longer; one goal near another is the same.
#[test]
pub(crate) fn a_goal_one_bot_found_no_way_to_is_left_by_its_team() {
    let goal = Vec3::new(30.0, 0.0, 10.0);
    let mut team = Vec::new();
    // Proved at 10 s: the bot's own memory until 30 s.
    learn_no_way(&mut team, &[(goal, 10_000 + NO_WAY_KEEP_MS)], 12_000);
    assert_eq!(team, vec![(goal, 10_000 + TEAM_NO_WAY_KEEP_MS)]);
    // Learnt again at the next tick: the same.
    learn_no_way(&mut team, &[(goal, 10_000 + NO_WAY_KEEP_MS)], 12_500);
    assert_eq!(team.len(), 1);
    // Another bot proves it again at 40 s, a metre off.
    let near = goal + Vec3::new(1.0, 0.0, 0.0);
    learn_no_way(&mut team, &[(near, 40_000 + NO_WAY_KEEP_MS)], 41_000);
    assert_eq!(team, vec![(goal, 40_000 + TEAM_NO_WAY_KEEP_MS)]);
    const { assert!(TEAM_NO_WAY_KEEP_MS >= 3 * NO_WAY_KEEP_MS) };
}

/// A play's goal its bot proved no way to is shut while the proof holds
/// (`NO_WAY_KEEP_MS`), and open again after: a zone gone off, the way
/// searched again. Not for a carrier (its way home is Collect's), not at a
/// goal it stands within, not at another goal past the replan distance.
#[test]
pub(crate) fn a_goal_proved_no_way_to_is_shut_while_the_proof_holds_but_not_for_a_carrier() {
    let goal = Vec3::new(30.0, 0.0, 10.0);
    let intent = Intent::to(goal, Pace::Walk).within(1.0);
    let mut me = PlaceView {
        place: 0,
        team: 0,
        bot: true,
        present: true,
        alive: true,
        feet: Vec3::new(10.0, 0.0, 10.0),
        yaw: 0.0,
        pitch: 0.0,
        velocity: [0.0, 0.0],
        keys: 0,
        health: 1.0,
        spawned: 0,
        kills: 0,
        deaths: 0,
        streak: 0,
        carrying: None,
        seen_at: 0,
        known_at: 0,
    };
    // Proved at 10 s.
    let proved = [(goal + Vec3::new(0.5, 0.0, 0.0), 10_000 + NO_WAY_KEEP_MS)];
    assert!(goal_shut(&intent, &me, &proved, 12_000));
    let last = 10_000 + NO_WAY_KEEP_MS - 1;
    assert!(goal_shut(&intent, &me, &proved, last));
    assert!(!goal_shut(&intent, &me, &proved, 10_000 + NO_WAY_KEEP_MS));
    assert!(!goal_shut(&intent, &me, &[], 12_000));
    let elsewhere = Intent::to(goal + Vec3::new(REPLAN_MOVED_M + 1.0, 0.0, 0.0), Pace::Walk);
    assert!(!goal_shut(&elsewhere, &me, &proved, 12_000));
    assert!(!goal_shut(&Intent::stay(), &me, &proved, 12_000));
    me.feet = goal + Vec3::new(0.8, 0.0, 0.0);
    assert!(!goal_shut(&intent, &me, &proved, 12_000), "within reach");
    me.feet = Vec3::new(10.0, 0.0, 10.0);
    me.carrying = Some(0);
    assert!(!goal_shut(&intent, &me, &proved, 12_000), "a carrier");
}

/// A bot whose play's goal it proved no way to leaves the play at its next
/// think and takes another at once (the commander is due at once), the one
/// it left set aside for it while the proof holds; before, it stood there
/// to the play's end (up to 70 s of Crossfire, 90 s of Buddy), searching
/// again every 500 ms. A play with other spots takes another of them
/// (`hold_exit`, `crossfire`, `cut`, `buddy`), so the proof here shuts all
/// the floor within 40 m of the goal: no spot is left, and the think is the
/// last resort. A lone bot in a team deathmatch; the commander is held off
/// meanwhile, so the leaving is the think's (Collect leaves a stand proved
/// no way to itself, and its commander a battery).
#[test]
pub(crate) fn a_bot_leaves_a_play_whose_goal_it_proved_no_way_to_and_takes_another() {
    let world = soak_world();
    let tuning = Tuning::default();
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x51A7,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    bots.entered(0, true, 0);
    let (mut clock, mut next_world) = (0, 1_000);
    // (proved at, the play, its goal), and the play taken after it.
    let mut proved: Option<(u32, PlayKind, plays::TargetRef, Vec3)> = None;
    let mut left_at = None;
    let mut next = None;
    let mut said = Vec::new();
    while clock < 20_000 && next.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, notes) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        said.extend(
            notes
                .iter()
                .filter(|n| n.place == 0)
                .map(|n| format!("{} {}", n.time, n.text)),
        );
        if notes.iter().any(|n| n.text.contains("no way to its goal")) {
            left_at = Some(clock);
        }
        let bot = bots.bots[0].as_mut().expect("bot 0");
        match (proved, bot.assignment.as_ref()) {
            // Walking to its first play's goal: proved no way to.
            (None, Some(a)) if bot.route.is_some() => {
                if let Go::To { at, .. } = bot.intent.go {
                    // Points 2 m apart: every floor within 40 m is within
                    // `REPLAN_MOVED_M` of one.
                    for i in -20..=20 {
                        for j in -20..=20 {
                            let p = Vec3::new(at.x + 2.0 * i as f32, at.y, at.z + 2.0 * j as f32);
                            bot.no_way_to.push((p, clock + NO_WAY_KEEP_MS));
                        }
                    }
                    proved = Some((clock, a.kind, a.target, at));
                    bots.teams[0].next_commander = clock + 10_000;
                }
            }
            (Some((_, kind, target, _)), Some(a)) if (a.kind, a.target) != (kind, target) => {
                next = Some((clock, a.kind, a.target));
            }
            _ => {}
        }
    }
    let (at, kind, target, goal) = proved.expect("a play with a goal");
    let left_at =
        left_at.unwrap_or_else(|| panic!("{kind:?} {target:?} to {goal:?} not left: {said:#?}"));
    let (taken_at, next_kind, next_target) = next.expect("another play taken");
    let (leaving, taking) = (left_at - at, taken_at - left_at);
    assert!(leaving <= 1_000, "left {leaving} ms after the proof");
    assert!(taking <= 500, "the next play {taking} ms after");
    let bot = bots.bots[0].as_ref().expect("bot 0");
    assert!(
        bot.set_aside
            .iter()
            .any(|&(k, t, until)| (k, t) == (kind, target) && until == left_at + NO_WAY_KEEP_MS),
        "{:?}",
        bot.set_aside
    );
    assert_ne!((next_kind, next_target), (kind, target));
}
