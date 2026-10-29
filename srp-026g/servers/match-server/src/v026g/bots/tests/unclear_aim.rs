//! A walk with no clear point ahead (`Feel::unclear_aim`): School's spawn-room doorway (J0 match 12a83438, bot 3).

use super::*;

use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

/// What a walk from School's team-0 spawn at (22.355, 179.641) to the J0 try start (2.2, 105.72) came to.
struct Walk {
    /// "is stuck" notes: (the clock, where).
    stuck: Vec<(u32, Vec3)>,
    /// The clock at which the body first came within 2 m of the start, if it did.
    reached: Option<u32>,
    /// The motor's counts of the walk (`motor::counts`): look-ahead ended unclear, steered back, off the route.
    unclear: [u64; 3],
}

/// The J0 runner's walk (`Go::To` the start, exact arrive) on the stand-in, the bot's inputs reaching the body `delay` ms late
/// (a live body's lag: its reckoning and the body disagree), for `until` ms.
fn school_walk(world: &Arc<World>, delay: u32, unclear_aim: bool, until: u32) -> Walk {
    let start = Vec3::new(22.355, 16.114, 179.641);
    let goal = Vec3::new(2.2, 16.32, 105.72);
    let mut tuning = Tuning::default();
    tuning.feel.unclear_aim = unclear_aim;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x6E07,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    bots.entered(0, true, 0);
    let before = motor::counts::counts();
    let (mut clock, mut next_world) = (0, 1_000);
    let mut walk = Walk { stuck: Vec::new(), reached: None, unclear: [0; 3] };
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        let mut notes = Vec::new();
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    notes.extend(bots.fixed(place, fix, Some(health)));
                }
                bots.facts(facts);
            }
        }
        let clock_now = Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true };
        let view = bots.tracker.view(&clock_now, &bots.match_wide_tuning().kin, Some(world));
        bots.features.searching.left = SEARCHES_PER_POLL;
        bots.features.searching.floors = bots.match_wide_tuning().think.search_floors_per_poll as usize;
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent {
                go: Go::To { at: goal, within_m: 0.0 },
                pace: Pace::Run,
                fire: false,
                exact_arrive_m: Some(0.1),
                ..Intent::stay()
            };
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        for made in made {
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time + delay, made.input.actions, turn, None, gun));
        }
        for note in notes.iter().filter(|n| n.text.contains("is stuck at")) {
            walk.stuck.push((clock, game.bodies[0].feet));
            let _ = note;
        }
        if walk.reached.is_none() && game.bodies[0].feet.distance_xz(goal) < 2.0 {
            walk.reached = Some(clock);
        }
    }
    let after = motor::counts::counts();
    walk.unclear = [after[8] - before[8], after[9] - before[9], after[10] - before[10]];
    walk
}

/// The walk out of School's spawn room, with the bot's inputs 0 / 150 / 300 / 500 ms late, with the rule off and on: the
/// stuck notes of the walk, when the body came to the start, and the unclear look-aheads. A measurement (it prints), run with
/// `SRP_MAPDATA_DIR=<mapdata folder> cargo test -p match-server --release school_walk -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn school_walk_with_late_inputs() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_02/day_4"))).unwrap();
    let links = Tuning::default().grid.off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let mut stuck = [0usize; 2];
    for delay in [0, 150, 300, 500] {
        for unclear_aim in [false, true] {
            let walk = school_walk(&world, delay, unclear_aim, 60_000);
            println!(
                "delay {delay:3} ms, rule {}: stuck {} {:?}; at the start {:?}; unclear {:?}",
                if unclear_aim { "on " } else { "off" },
                walk.stuck.len(),
                walk.stuck.iter().map(|(t, at)| (*t, at.x.round(), at.z.round())).collect::<Vec<_>>(),
                walk.reached,
                walk.unclear
            );
            stuck[usize::from(unclear_aim)] += walk.stuck.len();
        }
    }
    println!("stuck notes over the four delays: rule off {}, on {}", stuck[0], stuck[1]);
}
