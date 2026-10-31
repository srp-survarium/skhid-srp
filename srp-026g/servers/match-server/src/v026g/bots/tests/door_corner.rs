//! Chemical Plant's 717 door (A66), walked with the stand-in's body as the lattice has it and as the client's capsule is (A191,
//! `SRP_BENCH_STAND_IN_CORNER`, read once: run the measurement under each setting). From the A66 door walk (fx-struct): a walk on the stand-in over the real mapdata through the doorway at x 24 (a 1.27 m
//! opening in z 80.3-81.6, cells at x 23.6-24.1 only in the rows z 80.6-81.4, `wsl-out/grid/a96_door717.txt`), entered after
//! a ramp and a turn, with the distance to the nearest wall read every tick.

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

/// The doorway's centre on the floor (x, z); the height is read from the grid.
const DOOR_XZ: [f32; 2] = [24.0, 81.0];
/// The doorway's jambs: the wall face at x 23.92 and the opening between z 80.32 and 81.59 (`a96_door717.txt`, 1.27 m clear).
const WALL_X: f32 = 23.92;
const JAMB_Z: [f32; 2] = [80.32, 81.59];
/// The body's radius (half of the 0.76 m capsule) and the distance the client's controller keeps from a wall (A66 grid note).
const BODY_RADIUS_M: f32 = 0.38;
const CONTROLLER_HOLD_M: f32 = 0.41;

/// The distance from the feet to the nearest jamb of the doorway: to the corner while east of the wall face, to the opening's
/// side once in it.
fn jamb_distance(feet: Vec3) -> f32 {
    JAMB_Z
        .iter()
        .map(|&z| if feet.x > WALL_X { ((feet.x - WALL_X).powi(2) + (feet.z - z).powi(2)).sqrt() } else { (feet.z - z).abs() })
        .fold(f32::MAX, f32::min)
}

/// How far the walls are looked for, m, and the lattice step of the search.
const WALL_REACH_M: f32 = 0.9;
const WALL_STEP_M: f32 = 0.125;

/// The distance from `at` to the nearest point of the lattice around it with no floor within 1 m of its height: the nearest
/// wall or drop edge, read to `WALL_REACH_M`.
pub(super) fn wall_distance(grid: &mapdata::grid::WalkGrid, at: Vec3) -> f32 {
    let mut best = WALL_REACH_M;
    let n = (WALL_REACH_M / WALL_STEP_M) as i32;
    for i in -n..=n {
        for k in -n..=n {
            let (dx, dz) = (i as f32 * WALL_STEP_M, k as f32 * WALL_STEP_M);
            let d = (dx * dx + dz * dz).sqrt();
            if d >= best {
                continue;
            }
            let (x, z) = (at.x + dx, at.z + dz);
            if !grid.floors_in([x, z, x, z]).any(|f| (grid.position(f).y - at.y).abs() <= 1.0) {
                best = d;
            }
        }
    }
    best
}

/// What a walk through the door came to.
struct Pass {
    /// The ticks (ms) the body spent within 4 m of the door centre, and the first and last clock there.
    near_ms: u32,
    /// The least distance to a wall on the walk within 4 m of the door, m, and where.
    closest: (f32, Vec3),
    /// The ticks the ground speed was under 1 m/s within 4 m of the door.
    slow_ms: u32,
    /// The least distance to a jamb within 3 m of the door, m, where, and the ms spent nearer than the controller's hold
    /// ([`CONTROLLER_HOLD_M`]) and than the body's radius.
    jamb: (f32, Vec3),
    under_hold_ms: u32,
    under_radius_ms: u32,
    /// "is stuck" notes within 6 m of the door.
    stuck: usize,
    /// The clock the body first came within 1.5 m of the goal.
    reached: Option<u32>,
    last_speed: f32,
}

/// A walk of the J0 runner's kind (`Go::To` the goal) from `start` to `goal`, for `until` ms.
fn walk(world: &Arc<World>, start: Vec3, goal: Vec3, door: Vec3, until: u32) -> Pass {
    let tuning = Tuning::default();
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x717,
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
    let (mut clock, mut next_world) = (0, 1_000);
    let mut pass = Pass { near_ms: 0, closest: (f32::MAX, Vec3::ZERO), slow_ms: 0, jamb: (f32::MAX, Vec3::ZERO), under_hold_ms: 0, under_radius_ms: 0, stuck: 0, reached: None, last_speed: 0.0 };
    let mut last = start;
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
        bots.features.searching.new_poll(
            bots.match_wide_tuning().think.search_queue,
            bots.match_wide_tuning().think.search_floors_per_poll as usize,
            crate::v026g::bots::search::living(&bots.bots, &view),
        );
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
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        let feet = game.bodies[0].feet;
        // The stand-in moves every TICK_MS: the speed is read over a tick, on the clock steps that end one.
        let ticked = clock % TICK_MS == 0;
        let speed = if ticked { feet.distance_xz(last) / (TICK_MS as f32 / 1000.0) } else { pass.last_speed };
        pass.last_speed = speed;
        if ticked {
            last = feet;
        }
        let to_door = feet.distance_xz(door);
        if to_door <= 4.0 && (feet.y - door.y).abs() <= 2.0 {
            pass.near_ms += 25;
            if ticked && speed < 1.0 {
                pass.slow_ms += TICK_MS;
            }
            if to_door <= 3.0 {
                let j = jamb_distance(feet);
                if j < pass.jamb.0 {
                    pass.jamb = (j, feet);
                }
                if ticked && j < CONTROLLER_HOLD_M {
                    pass.under_hold_ms += TICK_MS;
                }
                if ticked && j < BODY_RADIUS_M {
                    pass.under_radius_ms += TICK_MS;
                }
            }
            let wall = wall_distance(&world.grid, feet);
            if wall < pass.closest.0 {
                pass.closest = (wall, feet);
            }
            pass.stuck += notes.iter().filter(|n| n.text.contains("is stuck at")).count();
        }
        if pass.reached.is_none() && feet.distance_xz(goal) < 1.5 {
            pass.reached = Some(clock);
            break;
        }
    }
    pass
}

/// The starts, within 40 m of the door, whose route to the west side of the door goes through it (within 1.2 m of its
/// centre), climbs at least 1 m in the 12 m before it and turns at least 60 degrees within 8 m of it: the ramp and the turn
/// the J0 bots stalled at.
fn ramp_turn_starts(grid: &mapdata::grid::WalkGrid, door: Vec3, goal_node: u32) -> Vec<(Vec3, f32)> {
    let mut out = Vec::new();
    let nodes: Vec<u32> = grid.floors_in([door.x - 40.0, door.z - 40.0, door.x + 40.0, door.z + 40.0]).collect();
    for &node in nodes.iter().step_by(7) {
        let from = grid.position(node);
        if from.distance_xz(door) < 12.0 {
            continue;
        }
        let Some(route) = path::plan(grid, node, goal_node) else {
            continue;
        };
        let mut through = None;
        let mut s = 0.0;
        while s < route.length() {
            if route.floor_point(s).distance_xz(door) <= 1.2 {
                through = Some(s);
                break;
            }
            s += 0.25;
        }
        let Some(s_door) = through else {
            continue;
        };
        let (before, after) = (route.floor_point((s_door - 12.0).max(0.0)), route.floor_point(s_door));
        let climb = after.y - before.y;
        let heading = |a: f32, b: f32| {
            let (p, q) = (route.floor_point(a), route.floor_point(b));
            (q.z - p.z).atan2(q.x - p.x)
        };
        let turn = {
            let (h0, h1) = (heading((s_door - 8.0).max(0.0), (s_door - 4.0).max(0.0)), heading((s_door - 2.0).max(0.0), s_door + 1.0));
            let mut d = (h1 - h0).abs();
            if d > std::f32::consts::PI {
                d = 2.0 * std::f32::consts::PI - d;
            }
            d.to_degrees()
        };
        if climb >= 1.0 && turn >= 60.0 {
            out.push((from, turn));
        }
    }
    out
}

/// Walks through the 717 door from the starts that climb a ramp and turn before it; prints, per walk, the least distance to a
/// wall within 4 m of the door and where, the milliseconds there and under 1 m/s, the stuck notes. A measurement:
/// `SRP_MAPDATA_DIR=<mapdata folder> cargo test -p match-server --release door_717_walk -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn door_717_walk() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    for project in ["level_04/day_2", "level_04/day_4"] {
        let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(project))).unwrap();
        let links = Tuning::default().grid.off_mesh_links();
        let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
        let grid = &world.grid;
        let Some(door_node) = grid.floors_in([24.38, 81.0, 24.38, 81.0]).next() else {
            println!("{project}: no floor at the door");
            continue;
        };
        let door = Vec3::new(DOOR_XZ[0], grid.position(door_node).y, DOOR_XZ[1]);
        let Some(goal_node) = path::goal_floor(grid, Vec3::new(21.0, door.y, 81.0)) else {
            println!("{project}: no floor west of the door");
            continue;
        };
        let goal = grid.position(goal_node);
        let starts = ramp_turn_starts(grid, door, goal_node);
        println!("{project}: door at ({:.2}, {:.2}, {:.2}), goal ({:.2}, {:.2}, {:.2}); {} starts climb a ramp and turn before it", door.x, door.y, door.z, goal.x, goal.y, goal.z, starts.len());
        let (mut reached, mut walks) = (0, 0);
        let (mut near, mut slow, mut under_hold, mut under_radius, mut stuck) = (0u32, 0u32, 0u32, 0u32, 0usize);
        let mut closest = f32::MAX;
        for (start, turn) in starts.iter().take(12) {
            let pass = walk(&world, *start, goal, door, 90_000);
            walks += 1;
            reached += usize::from(pass.reached.is_some());
            near += pass.near_ms;
            slow += pass.slow_ms;
            under_hold += pass.under_hold_ms;
            under_radius += pass.under_radius_ms;
            stuck += pass.stuck;
            closest = closest.min(pass.jamb.0);
            println!(
                "  corner {}: from ({:.1}, {:.1}, {:.1}) turn {:.0} deg: near the door {} ms (under 1 m/s {} ms), closest jamb {:.2} m ({} ms under the controller's {CONTROLLER_HOLD_M} m, {} ms under the body's radius {BODY_RADIUS_M} m), stuck notes {}, reached the goal {:?}",
                stand_in_corner(), start.x, start.y, start.z, turn, pass.near_ms, pass.slow_ms, pass.jamb.0, pass.under_hold_ms, pass.under_radius_ms, pass.stuck, pass.reached
            );
        }
        println!(
            "{project} DOOR-717 corner {}: walks {walks} reached {reached} near_ms {near} slow_ms {slow} under_hold_ms {under_hold} under_radius_ms {under_radius} stuck_notes {stuck} closest_jamb {closest:.2}",
            stand_in_corner()
        );
    }
}

/// A picture of a place the capsule holds bodies at (`SRP_SPOTS="<level_NN/weather_n> <x> <z> <y>;..."`, `SRP_MAPDATA_DIR`, and
/// `SRP_HUMAN_DIR` for the live samples): 5 m square at 0.1 m: `.` where a floor is and the capsule fits (0.38 m from every wall), `x`
/// where a floor is and it does not, a blank where there is no floor; the live players' samples within 1.5 m of it and how near
/// the walls they stood. A measurement.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_SPOTS"]
fn capsule_spot_report() {
    let (Some(dir), Ok(spots)) = (std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from), std::env::var("SRP_SPOTS")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_SPOTS is not set");
        return;
    };
    for spot in spots.split(';').filter(|s| !s.trim().is_empty()) {
        let f: Vec<&str> = spot.split_whitespace().collect();
        let (project, x, z, y) = (f[0], f[1].parse::<f32>().unwrap(), f[2].parse::<f32>().unwrap(), f[3].parse::<f32>().unwrap());
        let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(project))).unwrap();
        let sections = standin_capsule::Sections::new(&map.walker_triangles(), mapdata::Body::client().min_ground_normal_y);
        let links = Tuning::default().grid.off_mesh_links();
        let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
        let grid = &world.grid;
        let clearance = |px: f32, pz: f32, py: f32| {
            let mut segs = Vec::new();
            sections.near((px, pz), py, 1.0, &mut segs);
            segs.iter().map(|s| standin_capsule::nearest(s, (px, pz)).1).fold(1.0f32, f32::min)
        };
        println!("{project} spot ({x}, {y}, {z}): rows are z {:.1}..{:.1} (down), columns x {:.1}..{:.1}", z - 2.5, z + 2.5, x - 2.5, x + 2.5);
        for k in 0..=50 {
            let pz = z - 2.5 + k as f32 * 0.1;
            let mut row = String::new();
            for i in 0..=50 {
                let px = x - 2.5 + i as f32 * 0.1;
                let floor = grid.floors_in([px, pz, px, pz]).map(|n| grid.position(n).y).find(|fy| (fy - y).abs() <= 2.0);
                row.push(match floor {
                    None => ' ',
                    Some(fy) if clearance(px, pz, fy) >= standin_capsule::RADIUS_M => '.',
                    Some(_) => 'x',
                });
            }
            if !row.trim().is_empty() {
                println!("{pz:7.1} {row}");
            }
        }
        if let Ok(humans) = std::env::var("SRP_HUMAN_DIR") {
            let stem = project.replace('/', "_");
            let path = std::path::Path::new(&humans).join(format!("{stem}.tsv"));
            let (mut n, mut moving, mut least) = (0, 0, f32::MAX);
            if let Ok(text) = std::fs::read_to_string(&path) {
                for line in text.lines().skip(1) {
                    let c: Vec<&str> = line.split('\t').collect();
                    if c.len() < 10 {
                        continue;
                    }
                    let (Ok(sx), Ok(sy), Ok(sz), Ok(speed)) = (c[3].parse::<f32>(), c[4].parse::<f32>(), c[5].parse::<f32>(), c[9].parse::<f32>()) else { continue };
                    if (sx - x).hypot(sz - z) <= 1.5 && (sy - y).abs() <= 1.5 {
                        n += 1;
                        moving += usize::from(speed > 1.0);
                        least = least.min(clearance(sx, sz, sy));
                    }
                }
            }
            println!("  live samples within 1.5 m: {n} ({moving} over 1 m/s), the least wall clearance among them {least:.2} m");
        }
    }
}
