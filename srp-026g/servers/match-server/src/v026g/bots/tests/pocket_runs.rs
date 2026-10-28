use mapdata::{MapData, RayTree, Spawn, Triangle, WalkGrid, WalkerBehaviour};

use super::*;
use crate::v026g::bots::body::tests::floor;

/// The yard and its pit, `depth` m deep with walls. The grid links no
/// jump over 0.6 m, so a 0.9 m rim is no link but a standing exit, as
/// pit A's (0.90 m up, 1.0 m across: too far for a link).
fn pit(depth: f32) -> Arc<World> {
    let y = -depth;
    let mut tris: Vec<Triangle> = Vec::new();
    tris.extend(floor(0.0, 0.0, 30.0, 40.0, 0.0));
    tris.extend(floor(34.0, 0.0, 80.0, 40.0, 0.0));
    tris.extend(floor(30.0, 0.0, 34.0, 18.0, 0.0));
    tris.extend(floor(30.0, 22.0, 34.0, 40.0, 0.0));
    tris.extend(floor(30.0, 18.0, 34.0, 22.0, y));
    let walker = WalkerBehaviour::BySlope;
    let mut wall = |a: (f32, f32), b: (f32, f32)| {
        let (p, q) = (Vec3::new(a.0, y, a.1), Vec3::new(b.0, y, b.1));
        let (r, s) = (Vec3::new(b.0, 0.0, b.1), Vec3::new(a.0, 0.0, a.1));
        tris.push(Triangle {
            v: [p, q, r],
            walker,
        });
        tris.push(Triangle {
            v: [p, r, s],
            walker,
        });
    };
    wall((30.0, 18.0), (34.0, 18.0));
    wall((30.0, 22.0), (34.0, 22.0));
    wall((30.0, 18.0), (30.0, 22.0));
    wall((34.0, 18.0), (34.0, 22.0));
    let spawns = [Vec3::new(5.0, 0.0, 10.0), Vec3::new(75.0, 0.0, 10.0)];
    let grid = WalkGrid::build_kept(
        &tris,
        [0.0, 0.0, 80.0, 40.0],
        0.25,
        mapdata::Body {
            max_jump: 0.6,
            ..mapdata::Body::default()
        },
        &spawns,
        &spawns,
    );
    let spawn = |team: i32, at: Vec3| Spawn {
        position: at,
        rotation: Vec3::ZERO,
        team,
        priority: 0,
        point_id: 0,
        layer: "default".into(),
    };
    let map = MapData {
        spawns: vec![spawn(0, spawns[0]), spawn(1, spawns[1])],
        ..MapData::default()
    };
    let sight = RayTree::build(&tris);
    Arc::new(World::assemble(map, grid.into(), sight.into()))
}

#[derive(Debug, Default)]
struct Seen {
    /// When its floor was first out of the pocket, and when it got
    /// within 1.5 m of its goal.
    out_ms: Option<u32>,
    arrived_ms: Option<u32>,
    /// What the plays saw last (`Ctx::trapped`), and where it ended.
    trapped: Option<escape::Trapped>,
    end: Vec3,
}

/// The bot put at `start` (in the pit) with `goal`, run to `until` ms.
fn drive(world: Arc<World>, start: Vec3, goal: Vec3, until: u32) -> Seen {
    drive_on(world, 0, start, goal, until)
}

/// [`drive`] on map `level`'s world.
fn drive_on(world: Arc<World>, level: u8, start: Vec3, goal: Vec3, until: u32) -> Seen {
    let teams = [0u8];
    let mut bots = Bots::new(
        0xB17,
        survarium::v026g::maps::mode::BATTERIES,
        level,
        &teams,
        &[true],
        &[None],
        Arc::new(Tuning::all_off()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    bots.entered(0, true, 0);
    let mut seen = Seen::default();
    let (mut clock, mut next_world) = (0u32, 1_000u32);
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) =
                game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let now = Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        let view = bots.tracker.view(&now, &bots.tuning.kin, Some(&world));
        bots.features.searching.left = SEARCHES_PER_POLL;
        bots.features.searching.floors = bots.tuning.think.search_floors_per_poll as usize;
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent::to(goal, Pace::Run).within(1.0);
        }
        let mut made = Vec::new();
        bots.ticks(0, &now, &view, &mut made);
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        let feet = game.bodies[0].feet;
        let in_pocket = world
            .grid
            .locate(feet, 1.0)
            .is_some_and(|node| world.grid.in_pocket(node));
        // Out on the ground: a jump's top beside the rim (the honest
        // stand-in's body in the air) is near a rim floor, not out.
        if seen.out_ms.is_none() && !in_pocket && game.bodies[0].flight.is_none() {
            seen.out_ms = Some(clock);
        }
        if seen.arrived_ms.is_none() && feet.distance_xz(goal) <= 1.5 {
            seen.arrived_ms = Some(clock);
        }
        seen.trapped = bots.tactics.as_ref().unwrap().escapes.trapped(0);
        seen.end = feet;
    }
    eprintln!("{seen:?}");
    seen
}

/// A pit 0.9 m deep (a standing jump out): the bot in it jumps out
/// onto its rim and walks on to its goal. Before, its searches found no
/// way and it stood there.
/// While it may not jump (a broken leg) a try's clock stands still: a
/// try begun, then 20 s hurt, is not failed; 12 s after the leg mends
/// it is.
#[test]
fn a_hurt_bot_spends_no_try_on_a_pit() {
    let world = pit(0.9);
    let grid = &world.grid;
    let board = escape::Board::default();
    let (feet, goal) = (Vec3::new(32.0, -0.9, 20.0), Vec3::new(55.0, 0.0, 32.0));
    let (mut state, mut route) = (None, None);
    let mut tick = |time: u32, may_jump: bool| {
        escape::tick(
            &mut state, &mut route, grid, &board, 0, 0, feet, goal, time, may_jump,
        )
        .events
    };
    let first = tick(0, true);
    assert!(matches!(first[..], [escape::Event::Try(_, 1)]), "{first:?}");
    for time in (500..=20_000).step_by(500) {
        let events = tick(time, false);
        assert!(events.is_empty(), "at {time}: {events:?}");
    }
    assert!(tick(20_000 + escape::TRY_MS - 500, true).is_empty());
    let failed = tick(20_000 + escape::TRY_MS, true);
    assert!(
        matches!(failed.first(), Some(escape::Event::Failed(_))),
        "{failed:?}"
    );
}

/// A bot given up in a pit whose goal moves into the pit and out again
/// stays given up: it gives up once (the bench's level_06 bots gave up
/// hundreds of times, each handing their slot on).
#[test]
fn a_bot_given_up_in_a_pit_gives_up_once_as_its_goal_moves() {
    let world = pit(1.6);
    let grid = &world.grid;
    let board = escape::Board::default();
    let feet = Vec3::new(32.0, -1.6, 20.0);
    let (outside, inside) = (Vec3::new(55.0, 0.0, 32.0), Vec3::new(31.0, -1.6, 19.0));
    let (mut state, mut route) = (None, None);
    let mut gave_up = 0;
    for k in 0..40u32 {
        let goal = if k % 4 < 2 { outside } else { inside };
        let out = escape::tick(
            &mut state,
            &mut route,
            grid,
            &board,
            0,
            0,
            feet,
            goal,
            k * 500,
            true,
        );
        gave_up += out
            .events
            .iter()
            .filter(|e| **e == escape::Event::GaveUp)
            .count();
        assert!(
            board.trapped(0).is_some_and(|t| t.gave_up),
            "at {k}: {:?}",
            board.trapped(0)
        );
    }
    assert_eq!(gave_up, 1);
}

/// A way out is known once the team got out by one (`Board::learn`):
/// the 1.2 m pit has only sprint exits (no bot sprints), so none is
/// known until a bot of the team gets out by one, and then for that
/// team only, and not for its carriers.
#[test]
fn a_way_out_is_known_once_the_team_got_out_by_it() {
    let world = pit(1.2);
    let grid = &world.grid;
    let board = escape::Board::default();
    let bottom = grid.locate(Vec3::new(32.0, -1.2, 20.0), 0.3).unwrap();
    let pocket = grid.pocket_of(bottom).unwrap();
    let exits = grid.pocket_exits(bottom);
    assert!(!exits.is_empty() && exits.iter().all(|e| e.jump != mapdata::ExitJump::Standing));
    assert!(!escape::way_out(grid, &board, 0, pocket, false));
    board.learn_for_test(0, pocket, exits[0]);
    assert!(escape::way_out(grid, &board, 0, pocket, false));
    assert!(!escape::way_out(grid, &board, 1, pocket, false));
    // Not for a carrier: it cannot sprint, and a sprint rim is no way
    // out it is known to make.
    assert!(!escape::way_out(grid, &board, 0, pocket, true));
}

/// A pocket with a steep climb out (a step up onto steep ground) has a
/// way out a bot's own jump takes, but not a carrier's: a jump onto
/// steep ground is not proven to hold (#406 keeps carriers to standing
/// exits).
#[test]
fn a_steep_exit_is_a_way_out_but_not_a_carriers() {
    let world = pit(1.6);
    let grid = &world.grid;
    let bottom = grid.locate(Vec3::new(32.0, -1.6, 20.0), 0.3).unwrap();
    let pocket = grid.pocket_of(bottom).unwrap();
    let board = escape::Board::default();
    assert!(!escape::way_out(grid, &board, 0, pocket, false));
    board.set_jump_exits_for_test(pocket, false, true);
    assert!(escape::standing_exit(grid, &board, pocket));
    assert!(escape::way_out(grid, &board, 0, pocket, false));
    assert!(!escape::way_out(grid, &board, 0, pocket, true));
}

#[test]
fn a_bot_in_a_pit_with_a_standing_exit_jumps_out_and_goes_on() {
    let world = pit(0.9);
    let bottom = world.grid.locate(Vec3::new(32.0, -0.9, 20.0), 0.3).unwrap();
    assert!(world.grid.in_pocket(bottom));
    let seen = drive(
        world,
        Vec3::new(32.0, -0.9, 20.0),
        Vec3::new(55.0, 0.0, 32.0),
        40_000,
    );
    assert!(seen.out_ms.is_some_and(|at| at <= 15_000), "{seen:?}");
    assert!(seen.arrived_ms.is_some(), "{seen:?}");
    assert_eq!(seen.trapped, None, "{seen:?}");
}

/// A pit 1.6 m deep (no way out within the sprint jump): the bot gives
/// up at once and the plays see it ([`escape::Trapped::gave_up`]).
#[test]
fn a_bot_in_a_pit_with_no_exit_gives_up() {
    let seen = drive(
        pit(1.6),
        Vec3::new(32.0, -1.6, 20.0),
        Vec3::new(55.0, 0.0, 32.0),
        5_000,
    );
    assert_eq!(seen.out_ms, None, "{seen:?}");
    assert!(seen.trapped.is_some_and(|t| t.gave_up), "{seen:?}");
}

/// What a bot shut in a pit with no exit did in a run: when it died and came
/// back, what it said, and where it ended.
struct PitRun {
    died: Vec<u32>,
    back: Vec<u32>,
    said: Vec<String>,
    end: Vec3,
    world: Arc<World>,
    /// The route searches the bot made.
    asked: u32,
    /// Whether the bot ended the run counted as shut in (`think.shut_in_gate`).
    shut_in: bool,
}

/// The bot of `pit(1.6)`, shut in it with a goal outside, run to `until` ms
/// on `tuning`: its think (every 100 ms) and ticks, and the stand-in game.
fn run_pit(tuning: Tuning, until: u32) -> PitRun {
    let world = pit(1.6);
    let (start, goal) = (Vec3::new(32.0, -1.6, 20.0), Vec3::new(55.0, 0.0, 32.0));
    let teams = [0u8];
    let mut bots = Bots::new(
        0xB17,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true],
        &[None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut next_think) = (0u32, 1_000u32, 0u32);
    let (mut died, mut back, mut notes) = (Vec::new(), Vec::new(), Vec::new());
    let mut was_alive = true;
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) =
                game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let now = Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        let view = bots.tracker.view(&now, &bots.tuning.kin, Some(&world));
        bots.features.searching.left = SEARCHES_PER_POLL;
        bots.features.searching.floors = bots.tuning.think.search_floors_per_poll as usize;
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent::to(goal, Pace::Run).within(1.0);
        }
        if clock >= next_think {
            next_think += 100;
            let mut rays = RayBudget::new(bots.tuning.think.rays_per_poll);
            bots.think(0, &view, &mut rays, &mut notes);
            // The test's goal stands in for the plays': think's own intent is put back.
            if let Some(bot) = bots.bots[0].as_mut() {
                bot.intent = Intent::to(goal, Pace::Run).within(1.0);
            }
        }
        let mut made = Vec::new();
        bots.ticks(0, &now, &view, &mut made);
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        let alive = game.bodies[0].alive;
        if was_alive && !alive {
            died.push(clock);
        }
        if !was_alive && alive {
            back.push(clock);
        }
        was_alive = alive;
    }
    PitRun {
        died,
        back,
        said: notes
            .iter()
            .map(|n| n.text.clone())
            .filter(|t| t.starts_with("self_kill"))
            .collect(),
        end: game.bodies[0].feet,
        world,
        asked: bots.bots[0].as_ref().map_or(0, |bot| bot.bench_asked.total),
        shut_in: bots.bots[0].as_ref().is_some_and(|bot| bot.bench_shut_in),
    }
}

/// A bot given up in a pit stops asking for goals outside it (`shut_in_gate`): in 100 s the one that may not
/// searches every 0.5 s (`NO_WAY_RETRY_MS`), the gated one only until it gave up.
#[test]
fn a_bot_given_up_in_a_pit_stops_asking_for_goals_outside_it() {
    let asks = |gate: bool| {
        let mut tuning = Tuning::all_off();
        tuning.think.shut_in_gate = gate;
        tuning.unstick.suicide_after_ms = None;
        run_pit(tuning, 100_000)
    };
    let (gated, ungated) = (asks(true), asks(false));
    eprintln!("gated {} ungated {}", gated.asked, ungated.asked);
    assert!(ungated.asked >= 100, "it asks every 0.5 s: {}", ungated.asked);
    assert!(gated.asked <= 10, "it gave up and asks no more: {}", gated.asked);
    // Its skipped ticks carry their own no-route reason, and the episode is the bot's, once.
    assert!(gated.shut_in && !ungated.shut_in, "{} {}", gated.shut_in, ungated.shut_in);
    assert_eq!(gated.end.y, ungated.end.y, "both stay in the pit");
}

/// The last resort, whole: a bot shut in a pit with no exit gives up its
/// tries at once and stands, aiming; 60 s after it found itself in the pit it
/// kills itself with the suicide bit (not a second before, and not twice), the
/// stand-in's body dies like the game's (a death, no one's kill), it respawns
/// at its side's spawn after the mode's delay and goes on to its goal.
#[test]
fn a_bot_shut_in_a_pit_kills_itself_after_the_minute_and_comes_back() {
    let mut tuning = Tuning::all_off();
    tuning.unstick.suicide_after_ms = Some(60_000);
    let PitRun { died, back, said, end, world, .. } = run_pit(tuning, 100_000);
    eprintln!("died {died:?} back {back:?} said {said:?}");
    assert_eq!(died.len(), 1, "once: {died:?}");
    assert!(died[0] >= 60_000, "not before the minute: {died:?}");
    assert!(died[0] <= 66_000, "soon after it: {died:?}");
    assert_eq!(back.len(), 1, "it came back: {back:?}");
    assert!(
        (died[0] + 5_000..=died[0] + 12_000).contains(&back[0]),
        "after the mode's respawn delay: {died:?} {back:?}"
    );
    assert_eq!(said.len(), 1, "{said:?}");
    // Back at its spawn side, out of the pit.
    assert!(
        world.grid.locate(end, 1.0).is_some_and(|n| !world.grid.in_pocket(n)),
        "{end:?}"
    );
}

/// With the setting at 0 (the default, the switch off) the same bot stands in the pit to the end.
#[test]
fn with_the_setting_at_zero_a_bot_in_a_pit_stays_there() {
    let PitRun { died, said, end, world, .. } = run_pit(Tuning::all_off(), 75_000);
    assert!(died.is_empty() && said.is_empty(), "{died:?} {said:?}");
    assert!(world.grid.locate(end, 1.0).is_some_and(|n| world.grid.in_pocket(n)), "{end:?}");
}

/// A pit with no way out is shut: no route is planned into it from the
/// yard (its rim's drops stay links), while a bot put in it is still in
/// its pocket and gives up as before.
#[test]
fn no_route_is_planned_into_a_shut_pit_and_a_bot_in_it_still_gives_up() {
    let world = pit(1.6);
    let grid = &world.grid;
    let bottom = grid.locate(Vec3::new(32.0, -1.6, 20.0), 0.3).unwrap();
    let yard = grid.locate(Vec3::new(20.0, 0.0, 20.0), 0.3).unwrap();
    assert!(grid.in_pocket(bottom) && grid.in_shut_pocket(bottom));
    let pocket = grid.pocket_of(bottom).unwrap();
    assert!(
        grid.pocket_entries(pocket)
            .iter()
            .any(|e| e.kind == mapdata::LinkKind::Drop)
    );
    assert!(crate::v026g::bots::path::plan(grid, yard, bottom).is_none());
    let seen = drive(
        world.clone(),
        Vec3::new(32.0, -1.6, 20.0),
        Vec3::new(55.0, 0.0, 32.0),
        5_000,
    );
    assert_eq!(seen.out_ms, None, "{seen:?}");
    assert!(seen.trapped.is_some_and(|t| t.gave_up), "{seen:?}");
}

/// A route's point over a shut pit under a walkway is put on the
/// walkway, not on the pit's floor below it (level_05's point at
/// (-7.8, -21.3), `HumanRoute::on_floors`).
#[test]
fn a_route_point_over_a_shut_pit_lands_on_the_walkway_over_it() {
    let y = -3.0;
    let mut tris: Vec<Triangle> = Vec::new();
    tris.extend(floor(0.0, 0.0, 30.0, 40.0, 0.0));
    tris.extend(floor(36.0, 0.0, 80.0, 40.0, 0.0));
    tris.extend(floor(30.0, 0.0, 36.0, 14.0, 0.0));
    tris.extend(floor(30.0, 26.0, 36.0, 40.0, 0.0));
    tris.extend(floor(30.0, 14.0, 36.0, 26.0, y));
    tris.extend(floor(30.0, 18.0, 36.0, 22.0, 0.0));
    let walker = WalkerBehaviour::BySlope;
    let mut wall = |a: (f32, f32), b: (f32, f32)| {
        let (p, q) = (Vec3::new(a.0, y, a.1), Vec3::new(b.0, y, b.1));
        let (r, s) = (Vec3::new(b.0, 0.0, b.1), Vec3::new(a.0, 0.0, a.1));
        tris.push(Triangle {
            v: [p, q, r],
            walker,
        });
        tris.push(Triangle {
            v: [p, r, s],
            walker,
        });
    };
    wall((30.0, 14.0), (36.0, 14.0));
    wall((30.0, 26.0), (36.0, 26.0));
    wall((30.0, 14.0), (30.0, 26.0));
    wall((36.0, 14.0), (36.0, 26.0));
    let spawns = [Vec3::new(5.0, 0.0, 10.0), Vec3::new(75.0, 0.0, 10.0)];
    let grid = WalkGrid::build_kept(
        &tris,
        [0.0, 0.0, 80.0, 40.0],
        0.25,
        mapdata::Body::default(),
        &spawns,
        &spawns,
    );
    let at = Vec3::new(33.0, 0.0, 17.0);
    let below = grid.locate(Vec3::new(33.0, y, 17.0), 0.3).unwrap();
    assert!(grid.in_shut_pocket(below));
    let route = crate::v026g::bots::routes::HumanRoute {
        team: 0,
        lives: 1,
        reward: 0.0,
        fight: 0.0,
        battery: 0.0,
        died: 0.0,
        points: vec![at],
    }
    .on_floors(&grid);
    let [point] = route.points[..] else {
        panic!("{:?}", route.points)
    };
    assert!(point.y.abs() < 0.1 && point.distance_xz(at) < 2.0, "{point:?}");
    let node = grid.locate(point, 0.3).unwrap();
    assert!(!grid.in_shut_pocket(node));
}

/// A route's point with the players' height over a walkway 4 m up,
/// open both ways (a ramp to it) over an open floor, is put on the
/// level they stood at: the walkway for 4 m, the floor under it for
/// 0 m. Without a height, or with one no floor near has (12 m), it
/// takes the lowest floor, as before (level_07's middle deck at 20.6 m,
/// `HumanRoute::on_floors`).
#[test]
fn a_route_point_with_a_height_lands_on_the_walkway_over_an_open_floor() {
    let deck = 4.0;
    let mut tris: Vec<Triangle> = Vec::new();
    tris.extend(floor(0.0, 0.0, 80.0, 40.0, 0.0));
    tris.extend(floor(30.0, 10.0, 36.0, 30.0, deck));
    let walker = WalkerBehaviour::BySlope;
    let (a, b, c, d) = (
        Vec3::new(36.0, deck, 18.0),
        Vec3::new(52.0, 0.0, 18.0),
        Vec3::new(52.0, 0.0, 22.0),
        Vec3::new(36.0, deck, 22.0),
    );
    tris.push(Triangle {
        v: [a, b, c],
        walker,
    });
    tris.push(Triangle {
        v: [a, c, d],
        walker,
    });
    let spawns = [Vec3::new(5.0, 0.0, 10.0), Vec3::new(75.0, 0.0, 10.0)];
    let grid = WalkGrid::build_kept(
        &tris,
        [0.0, 0.0, 80.0, 40.0],
        0.25,
        mapdata::Body::default(),
        &spawns,
        &spawns,
    );
    let put = |height: f32| {
        let route = crate::v026g::bots::routes::HumanRoute {
            team: 0,
            lives: 1,
            points: vec![Vec3::new(33.0, height, 20.0)],
            ..Default::default()
        }
        .on_floors(&grid);
        let [point] = route.points[..] else {
            panic!("{height}: {:?}", route.points)
        };
        assert!(
            point.distance_xz(Vec3::new(33.0, 0.0, 20.0)) < 2.0,
            "{point:?}"
        );
        let node = grid.locate(point, 0.3).unwrap();
        assert!(!grid.in_shut_pocket(node), "{point:?}");
        point.y
    };
    assert!((put(deck) - deck).abs() < 0.1, "{}", put(deck));
    assert!((put(deck + 0.4) - deck).abs() < 0.1);
    assert!(put(0.0).abs() < 0.1, "{}", put(0.0));
    assert!(put(f32::NAN).abs() < 0.1, "{}", put(f32::NAN));
    assert!(put(12.0).abs() < 0.1, "{}", put(12.0));
}

/// A pit 1.2 m deep (sprint-only ways out; no bot sprints yet): one
/// standing try at each of its two lowest rims, then it gives up.
#[test]
fn a_bot_in_a_pit_with_sprint_exits_tries_two_then_gives_up() {
    let seen = drive(
        pit(1.2),
        Vec3::new(32.0, -1.2, 20.0),
        Vec3::new(55.0, 0.0, 32.0),
        35_000,
    );
    assert_eq!(seen.out_ms, None, "{seen:?}");
    assert!(seen.trapped.is_some_and(|t| t.gave_up), "{seen:?}");
}

/// The fight's side step (`Tactics::clear_step_for`) does not step off
/// a rim into a pit with no standing jump out, but into one with; in
/// its own pit it steps as it likes.
#[test]
fn the_side_step_keeps_out_of_a_pit_it_cannot_jump_out_of() {
    for (depth, into) in [(1.6, false), (0.9, true)] {
        let tactics = Tactics::new(pit(depth), 1.0);
        let rim = Vec3::new(29.5, 0.0, 20.0);
        let inside = Vec3::new(31.0, -depth, 20.0);
        assert!(tactics.clear_line_for(rim, Vec3::new(28.0, 0.0, 20.0), 0, Some(0), false));
        assert_eq!(
            tactics.clear_step_for(rim, inside, 0, Some(0), false),
            into && tactics.clear_line_for(rim, inside, 0, Some(0), false),
            "depth {depth}"
        );
        let (a, b) = (Vec3::new(31.0, -depth, 19.0), Vec3::new(31.0, -depth, 21.0));
        assert!(
            tactics.clear_step_for(a, b, 0, Some(0), false),
            "depth {depth}: inside"
        );
    }
}

/// The pocket sweep: on each map, a bot put on each pocket's lowest
/// floor with its side's spawn as the goal, 60 s, with the escape off
/// and on: how many pockets it got out of, by their surest way out.
///
/// SRP_MAPDATA_DIR=<dir> SRP_BENCH_MAPS=1/2,… cargo test -p match-server --release pocket_sweep -- --ignored --nocapture --exact
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn pocket_sweep() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let maps: Vec<(u8, u8)> = bench_settings().var("SRP_BENCH_MAPS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|m| {
            let (level, mode) = m.split_once('/')?;
            Some((level.parse().ok()?, mode.parse().ok()?))
        })
        .collect();
    for (level, mode) in maps {
        let project = format!("level_{level:02}/day_{mode}");
        let map =
            mapdata::MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
        let world = Arc::new(World::build(map, mapdata::CELL, mapdata::Body::default()));
        let grid = &world.grid;
        let Some(goal) = world
            .map
            .spawns
            .iter()
            .find(|s| s.team == 0)
            .map(|s| s.position)
        else {
            continue;
        };
        let mut pockets: Vec<NodeId> = (0..grid.len() as NodeId)
            .filter_map(|n| grid.pocket_of(n))
            .collect();
        pockets.sort_unstable();
        pockets.dedup();
        // By the surest way out: standing, sprint, steep, none; out
        // with the escape off and on, and the seconds it took on.
        let mut by: std::collections::BTreeMap<String, (u32, u32, u32, f32)> =
            Default::default();
        for &pocket in &pockets {
            // Only the exits whose take-off the bot walks to inside the
            // pocket: the others' floors are reached by drops from
            // elsewhere (level_06's (63.85, 83.9) listed "standing" exits
            // it could not get to).
            let inside = |n: NodeId| grid.pocket_of(n) == grid.pocket_of(pocket);
            let reachable: Vec<mapdata::PocketExit> = grid
                .pocket_exits(pocket)
                .into_iter()
                .filter(|e| path::plan_where(grid, pocket, e.from, inside).is_some())
                .collect();
            let kind = reachable
                .iter()
                .map(|e| e.jump)
                .min()
                .map_or("none".to_owned(), |j| format!("{j:?}").to_lowercase());
            let start = grid.position(pocket);
            let mut outs = [None, None];
            for (k, off) in [true, false].into_iter().enumerate() {
                escape::counts::OFF.store(off, std::sync::atomic::Ordering::Relaxed);
                outs[k] = drive_on(world.clone(), level, start, goal, 60_000).out_ms;
            }
            escape::counts::OFF.store(false, std::sync::atomic::Ordering::Relaxed);
            if kind == "standing" && outs[1].is_none() {
                let exits: Vec<(f32, f32)> = reachable
                    .iter()
                    .filter(|e| e.jump == mapdata::ExitJump::Standing)
                    .take(4)
                    .map(|e| (e.rise, e.across))
                    .collect();
                eprintln!(
                    "  stayed {project} pocket at {start:?} ({} floors): standing (rise, across) {exits:?}",
                    grid.pocket_floors(pocket).count()
                );
            }
            let row = by.entry(kind).or_default();
            row.0 += 1;
            row.1 += u32::from(outs[0].is_some());
            row.2 += u32::from(outs[1].is_some());
            row.3 += outs[1].map_or(0.0, |ms| ms as f32 / 1000.0);
        }
        for (kind, (n, off, on, secs)) in by {
            eprintln!(
                "pockets {project} {kind}: {n} pockets, out with the escape off {off}, on {on} (mean {:.1} s)",
                secs / on.max(1) as f32
            );
        }
    }
}
