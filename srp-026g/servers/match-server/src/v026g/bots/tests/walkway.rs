use super::*;

/// The field with a walkway 3 m up along z 31-34 from x 4 to x 30, a ramp
/// down from its east end to the ground at x 38, team 0's container on it
/// at x 7, and a wall under it across x = 14 (z 28-34): the shape of
/// 0x4da3b51d's stuck carrier (`player_bots_stuck.md` §8).
pub(crate) fn walkway_world() -> Arc<World> {
    use super::body::tests::{floor, wall_x};
    let walker = mapdata::WalkerBehaviour::BySlope;
    let tri = |a: Vec3, b: Vec3, c: Vec3| mapdata::Triangle {
        v: [a, b, c],
        walker,
    };
    let mut solids = floor(4.0, 31.0, 30.0, 34.0, 3.0).to_vec();
    let (a, b) = (Vec3::new(30.0, 3.0, 31.0), Vec3::new(38.0, 0.0, 31.0));
    let (c, d) = (Vec3::new(38.0, 0.0, 34.0), Vec3::new(30.0, 3.0, 34.0));
    solids.push(tri(a, b, c));
    solids.push(tri(a, c, d));
    // The ramp solid under (no way beneath it).
    solids.extend(wall_x(30.0, 31.0, 34.0));
    for z in [31.0, 34.0] {
        solids.push(tri(
            Vec3::new(30.0, 0.0, z),
            Vec3::new(38.0, 0.0, z),
            Vec3::new(30.0, 3.0, z),
        ));
    }
    solids.extend(wall_x(14.0, 28.0, 34.0));
    super::tactics::tests::field_with_solids(&solids, |map| {
        let container = |team: i32, id: i32, at: Vec3| Container {
            position: at,
            team,
            id,
            use_shapes: vec![super::standin::use_box(at)],
        };
        map.containers = vec![
            container(0, 0, Vec3::new(7.0, 3.0, 32.5)),
            container(1, 1, Vec3::new(68.0, 0.0, 30.0)),
        ];
        map.battery_spawners = vec![BatterySpawner {
            position: Vec3::new(15.0, 0.0, 32.5),
            item: Vec3::new(15.0, 0.3, 32.5),
            layer: "default".into(),
        }];
    })
}

/// Issue 1 of the audit (0x4da3b51d bot 13, 273 s): a carrier fallen off a
/// walkway stands under it against a wall, its route above it. Its route
/// was found on the way (the ground under a route is on it across) and its
/// reckoning was as blocked as it (no stuck sign): it pushed into the wall
/// until the end. It must see it is off its route (the floors' heights),
/// or stuck by its walk keys held with its body still, and go round by the
/// ramp: home within 45 s of play, the take included (before: never).
#[test]
pub(crate) fn a_carrier_fallen_under_its_walkway_route_gets_home() {
    let world = walkway_world();
    let teams = [0u8, 1];
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let mut bots = Bots::new(
        0x57C4,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 11, None, None);
    // Under the walkway, east of the wall, by the battery.
    let under = Vec3::new(16.0, 0.0, 32.5);
    game.bodies[0].node = world.grid.locate(under, 0.5);
    game.bodies[0].feet = under;
    let grid = &world.grid;
    assert!(grid.position(game.bodies[0].node.unwrap()).y < 0.5);
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut planted) = (0, 1_000, false);
    while clock < 60_000 && game.delivered[0] == 0 {
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
        // Once it took it and heads home: the route it had on the walkway,
        // from above where it stands on to its container (as it had before
        // it fell).
        if !planted
            && let Some(bot) = bots.bots[0].as_mut()
            && let Some((_, goal)) = bot.route
            && goal.distance_xz(Vec3::new(7.0, 0.0, 32.5)) < 3.0
        {
            let from = grid.locate(Vec3::new(21.0, 3.0, 32.5), 0.5).unwrap();
            let to = grid.locate(goal, 3.0).unwrap();
            assert!(grid.position(to).y > 2.5, "home is on the walkway: {goal:?}");
            bot.route = Some((path::plan(grid, from, to).unwrap(), goal));
            planted = true;
        }
        let (made, _) = bots.poll(Clock {
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
    }
    assert!(planted, "never headed home");
    assert!(
        game.delivered[0] == 1 && clock <= 45_000,
        "not home by {clock} ms: at {:?}",
        game.bodies[0].feet
    );
}

/// Bot 0 of [`soak_world`] (no play: its goal set each poll) walked about
/// by the stand-in, with `floors` floors searched a poll; `goal` gives its
/// goal at each clock. Calls `each` after every poll with the clock, the
/// bot, its body and the notes; runs to `until` ms.
pub(crate) fn walk_to_goals(
    floors: u32,
    until: u32,
    goal: impl Fn(u32) -> Vec3,
    mut each: impl FnMut(u32, &Bot, &Body, &[Note]),
) {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.think.search_floors_per_poll = floors;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x6E07,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    let start = Vec3::new(5.0, 0.0, 10.0);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        let mut notes = Vec::new();
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    notes.extend(bots.fixed(place, fix, Some(health)));
                }
                bots.facts(facts);
            }
        }
        // `Bots::poll` with the play's step left out: the goal is given.
        let clock_now = Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        let view = bots
            .tracker
            .view(&clock_now, &bots.match_wide_tuning().kin, Some(&world));
        bots.features.searching.left = SEARCHES_PER_POLL;
        bots.features.searching.floors = bots.match_wide_tuning().think.search_floors_per_poll as usize;
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent::to(goal(clock), Pace::Run).within(1.0);
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        if let Some(bot) = bots.bots[0].as_mut()
            && let Some(text) = bot.dropped.take()
        {
            bot.note(&mut notes, clock, text);
        }
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        each(clock, bots.bots[0].as_ref().unwrap(), &game.bodies[0], &notes);
    }
}

/// Issue 5 of the audit: a bot whose goal moves on keeps walking its
/// route while the new search goes on over several polls, never standing
/// for it; and one whose goal moves on every poll (its searches dropped
/// before they end) keeps walking too, and notes the dropped searches at
/// most once in [`path::DROPPED_NOTE_MS`].
#[test]
pub(crate) fn a_bot_keeps_walking_its_route_while_its_next_search_goes_on() {
    // Round the wall's end (x 40, z 0-25) to x 48 (out of the other
    // side's spawn area), then 4.5 m on; 150 floors a poll: the search to
    // the new goal takes several.
    let (first, moved) = (Vec3::new(48.0, 0.0, 36.0), Vec3::new(48.0, 0.0, 31.5));
    let changed = 6_000;
    let sliced = path::searches().sliced;
    let (mut had, mut routeless, mut to_moved) = (false, 0, None);
    let mut walked = (None, 0.0f32);
    walk_to_goals(
        150,
        12_000,
        |clock| if clock < changed { first } else { moved },
        |clock, bot, body, _| {
            had |= bot.route.is_some();
            if had && bot.route.is_none() {
                routeless += 1;
            }
            if to_moved.is_none()
                && clock > changed
                && bot.route.as_ref().is_some_and(|(_, goal)| goal.distance_xz(moved) < 0.5)
            {
                to_moved = Some(clock);
            }
            if clock == changed {
                walked.0 = Some(body.feet);
            }
            if clock == changed + 3_000 {
                walked.1 = walked.0.map_or(0.0, |from| from.distance_xz(body.feet));
            }
        },
    );
    // Both searches went on over several polls (their first poll's way so
    // far walked meanwhile).
    assert!(path::searches().sliced >= sliced + 2);
    assert!(to_moved.is_some_and(|at| at <= changed + 100), "{to_moved:?}");
    assert_eq!(routeless, 0, "stood {routeless} polls with no route");
    assert!(walked.1 > 8.0, "walked {} m in the 3 s after", walked.1);
    // Every poll a new goal 3 m on (its search never ends): still a route
    // at every poll once it had one, and a note of the dropped searches
    // at most every 5 s.
    let (mut had, mut routeless, mut notes, mut polls) = (false, 0, Vec::new(), 0);
    let mut moved_m = (None, 0.0f32);
    walk_to_goals(
        150,
        16_000,
        |clock| Vec3::new(48.0, 0.0, 10.0 + 3.0 * ((clock / 25) % 8) as f32),
        |clock, bot, body, polled| {
            had |= bot.route.is_some();
            polls += 1;
            if had && bot.route.is_none() {
                routeless += 1;
            }
            for note in polled {
                if note.text.starts_with("dropped its route search") {
                    notes.push(clock);
                }
            }
            if clock == 4_000 {
                moved_m.0 = Some(body.feet);
            }
            if clock == 16_000 {
                moved_m.1 = moved_m.0.map_or(0.0, |from| from.distance_xz(body.feet));
            }
        },
    );
    assert!(had);
    // A partial route ends at the wall's face when the bot gets there (the
    // goal behind the wall never has its search end): it has arrived, and
    // stands till the next poll's route. The keys that point where the way
    // goes (not along a facing that lags the turn) bring the body to the
    // face sooner, so a few polls of it, not none: under 1 in 16.
    assert!(routeless * 16 <= polls, "stood {routeless} of {polls} polls with no route");
    assert!(!notes.is_empty() && notes.len() <= 4, "notes at {notes:?}");
    assert!(notes.windows(2).all(|w| w[1] >= w[0] + path::DROPPED_NOTE_MS), "{notes:?}");
    assert!(moved_m.1 > 10.0, "walked {} m in 12 s", moved_m.1);
}

/// Issue 5's stand (0x5d506a22 bot 14, 116-131 s): a goal computed at
/// another body's height (Buddy's, short of a partner on a walkway) hangs
/// 4 m over the floor under it, where `WalkGrid::locate` finds none. The
/// bot walks to the floor under it ([`path::goal_floor`]) instead of
/// standing with no way.
#[test]
pub(crate) fn a_goal_in_the_air_is_walked_to_the_floor_under_it() {
    let goal = Vec3::new(20.0, 4.0, 20.0);
    let (mut had, mut end) = (false, Vec3::ZERO);
    walk_to_goals(
        10_000,
        12_000,
        |_| goal,
        |_, bot, body, _| {
            had |= bot.route.is_some();
            end = body.feet;
        },
    );
    assert!(had, "never had a route");
    assert!(end.distance_xz(goal) < 2.0, "at {end:?}");
}
