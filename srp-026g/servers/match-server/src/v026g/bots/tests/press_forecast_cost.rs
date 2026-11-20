//! [press forecast] What a press forecast (`body::press_forecast`) costs on a real map: the reckoned body of a bot walking up to
//! Mamayev Kurgan's rise (map 91, the gap jump (-22.60, 24.27, 57.88) -> (-21.85, 25.81, 57.88)) on the server's grid with its client
//! walker, its presses after 0-11 ground ticks of forward forecast 20 times, twice; the clock around the forecasts alone, and the
//! press clock's steps (`kept_clock::Of::Press`). A measurement, decision-neutral.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release press_forecast_cost -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::body::{AfterLanding, BodyTrack, PressOutcome, PressPlan, PressWorld, kept_clock, press_forecast};

const PROJECT: &str = "level_06/day_2";
const TAKE_OFF: Vec3 = Vec3::new(-22.601, 24.266, 57.875);
const LANDING: Vec3 = Vec3::new(-21.851, 25.805, 57.875);

#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn press_forecast_cost_on_a_real_map() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let t = Tuning::default();
    let bytes = std::fs::read(dir.join(mapdata::format::file_name(PROJECT))).unwrap();
    let map_file = {
        let mut digest = mapdata::digest::Digest::new();
        digest.bytes(&bytes);
        digest.finish()
    };
    let map = mapdata::MapData::from_bytes(&bytes).unwrap();
    let extras = mapdata::GridExtras {
        wall_clearance: t.needs_wall_clearance(),
        open_sides: t.needs_open_sides(),
        pull_hold: t.grid.wall_clear_pull.then_some(crate::v026g::bots::tuning::PULL_HOLD_M),
        pull_walk: t.grid.pull_walk,
        walker_shapes: t.grid.walker_shapes,
    };
    let world = Arc::new(super::grid_cache::world(map, map_file, mapdata::CELL, t.grid_body(), t.off_mesh_links(), extras));
    let grid = &world.grid;
    assert!(grid.has_client_walker(), "the map's client walker");
    let kin = t.kin.clone();
    // A body 1.5 m behind the take-off on its floor's line, facing the way (+x: the game's yaw -pi/2), walking up to it.
    let start = Vec3::new(TAKE_OFF.x - 1.5, TAKE_OFF.y, TAKE_OFF.z);
    let start = grid.locate(start, 1.0).map_or(start, |node| grid.position(node));
    let yaw = -std::f32::consts::FRAC_PI_2;
    let mut track = BodyTrack::default();
    let fix = Fix {
        fall: None,
        controller: None,
        gates: None,
        jump: None,
        time: 0,
        alive: true,
        inserted: 0,
        feet: Some(start),
        yaw,
        pitch: 0.0,
        keys: 0,
        speed: Some(0.0),
    };
    track.fixed(fix, &kin, Some(grid));
    for time in (50..=100).step_by(50) {
        track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, Some(grid));
    }
    let at = *track.estimate().expect("reckoned");
    let world_of = PressWorld { kin: &kin, level: 1.0, grid };
    let plan = |press_in: u32| PressPlan { walk: key::FORWARD, press_in, take_off: TAKE_OFF, landing: LANDING, after: AfterLanding::Hold };
    let outcomes: Vec<PressOutcome> = (0..12).map(|k| press_forecast(&at, &plan(k), &world_of)).collect();
    eprintln!("PRESS REAL from {:?}: {outcomes:?}", at.feet);
    let _ = kept_clock::take(kept_clock::Of::Press);
    for round in 0..2 {
        let started = std::time::Instant::now();
        let mut forecasts = 0u32;
        for _ in 0..20 {
            for k in 0..12 {
                std::hint::black_box(press_forecast(&at, &plan(k), &world_of));
                forecasts += 1;
            }
        }
        let ns = started.elapsed().as_nanos() as f64;
        let (steps, step_ns) = kept_clock::take(kept_clock::Of::Press);
        eprintln!(
            "PRESS REAL COST round {round}: {forecasts} forecasts, {:.3} ms a forecast (the loop), {steps} controller steps, {:.1} a forecast, {} ns a step (the press clock)",
            ns / 1e6 / f64::from(forecasts),
            steps as f64 / f64::from(forecasts),
            step_ns.checked_div(steps).unwrap_or(0)
        );
    }
    // The reckoning's own steps of the same ticks (`BodyTrack::decided`): the ground ticks, the press, forward through 19
    // flight ticks, 7 let go; its clock (`kept_clock::Of::Reckoning`) against the press clock's ns a step on this map.
    let _ = kept_clock::take(kept_clock::Of::Reckoning);
    let last = at.time;
    for round in 0..2 {
        for _ in 0..20 {
            for k in 0..12u32 {
                let mut t = track.clone();
                let mut time = last;
                let mut tick = |keys: u32| {
                    time += 50;
                    t.decided(time, keys, (0.0, 0.0), 0.0, &kin, Some(grid));
                };
                for _ in 0..k {
                    tick(key::FORWARD);
                }
                tick(key::FORWARD | key::JUMP);
                for _ in 0..19 {
                    tick(key::FORWARD);
                }
                for _ in 0..7 {
                    tick(0);
                }
            }
        }
        let (steps, step_ns) = kept_clock::take(kept_clock::Of::Reckoning);
        eprintln!(
            "PRESS REAL RECKONING round {round}: {steps} controller steps, {} ns a step (the reckoning's clock, same ticks)",
            step_ns.checked_div(steps).unwrap_or(0)
        );
    }
}
