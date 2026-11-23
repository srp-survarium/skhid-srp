//! [body physics S2] The stand-in's body is the shared model of the client's body (`client_body::ClientBody`,
//! `SRP_BENCH_STAND_IN_BODY=client`). These are the named cases where the stand-in before it parted from the exe, each run here on the
//! map's own shapes (`Collision::build_exe`, the exe's cast) with the case's own keys and yaws, against the exe's answer (rig_real_v3,
//! the game's controller in Unicorn on the game's objects):
//!
//! - S092 (level_05/day_2): a bot backed off as stuck at a lip with the forward key held. The stand-in held it at the top (lowest feet
//!   8.77, 0.44 m from the take-off); the exe walks off on tick 19 and falls to y 4.33 (`exe_takeoff_v1.py`,
//!   `spot-suite/final-ea94ed45a/exe-takeoff-sample12.tsv`).
//! - School day_2's 0.96 m roof-beam riser: from its foot point, a walk from rest toward the beam's top. The stand-in held it at the
//!   face; the exe is on the top (y 22.45) by its second tick (fix-lip `roof_ramp_v1_exe.tsv`, `climb_ramp_exe_v1.py`).
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> cargo test -p match-server --lib standin_client_body -- --ignored
//! ```

use mapdata::Vec3;
use mapdata::controller::Collision;

use client_body::{Client, ClientBody, Input, Kin, keys};

fn shapes(project: &str) -> Option<Collision> {
    let dir = super::real_maps::dir()?;
    let file = dir.join(mapdata::format::file_name(project));
    let map = mapdata::MapData::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    Some(Collision::build_exe(&map).unwrap_or_else(|| panic!("{}: no game shapes (format 6 or later needed)", file.display())))
}

/// S092's window: the 3 s before the stuck note (16,000-19,000 ms of the suite's run, start 0), forward held, the bot's yaw each tick
/// (the suite's trace, `results.tsv.trace.tsv`).
const S092_YAWS: [f32; 61] = [
    3.0935, 3.0931, 3.0925, 3.0917, 3.0908, 3.0897, 3.0885, 3.0872, 3.0857, 3.0841, 3.0824, 3.0806, 3.0787, 3.0767, 3.0747, 3.0727,
    3.0706, 3.0685, 3.0665, 3.0644, 3.0624, 3.0604, 3.0585, 3.0567, 3.0550, 3.0533, 3.0518, 3.0504, 3.0492, 3.0481, 3.0471, 3.0463,
    3.0457, 3.0453, 3.0450, 3.0450, 3.0450, 3.0452, 3.0456, 3.0462, 3.0469, 3.0479, 3.0489, 3.0501, 3.0515, 3.0530, 3.0546, 3.0563,
    3.0581, 3.0600, 3.0619, 3.0639, 3.0660, 3.0681, 3.0701, 3.0722, 3.0743, 3.0763, 3.0782, 3.0801, 3.0820,
];
/// S092's feet at the window's start (the trace), and the take-off of the stuck note's next link.
const S092_FEET: Vec3 = Vec3::new(-53.3403, 8.7683, 14.9593);
const S092_TAKE_OFF: Vec3 = Vec3::new(-53.4, 8.1, 15.4);
/// The bot's walk in the trace, 0.18878 m a tick forward: the speed factor of its kit (0.18878 / 0.05 / 3.5).
const S092_LEVEL: f32 = 1.0787;

#[test]
#[ignore = "a named case on the real mapdata; needs SRP_MAPDATA_DIR and SRP_GAME_DATA"]
fn s092_the_body_walks_off_the_lip_as_the_exe_does() {
    let Some(world) = shapes("level_05/day_2") else { return };
    let world = Client { world: &world };
    // Put on its feet as the exe's run puts it (0.02 m over them, then settled by its own steps).
    let mut body = ClientBody::placed(S092_FEET + Vec3::new(0.0, 0.02, 0.0), S092_YAWS[0]);
    let kin = Kin { level: S092_LEVEL, legs_broken: false };
    let mut lowest = f32::INFINITY;
    let mut off_at = None;
    for (n, &yaw) in S092_YAWS.iter().enumerate() {
        let t = body.tick(&world, kin, Input { keys: keys::FORWARD, yaw });
        lowest = lowest.min(t.feet.y);
        if off_at.is_none() && t.feet.y < S092_TAKE_OFF.y - 0.8 {
            off_at = Some(n);
        }
    }
    eprintln!("S092: walks off at {off_at:?}, lowest {lowest:.2} (the exe: tick 19, 4.33)");
    assert!(off_at.is_some(), "the body stayed at the top (lowest {lowest:.2}); the exe walks off on tick 19 and falls to 4.33");
    assert!(lowest < 5.0, "it fell only to {lowest:.2}; the exe falls to 4.33");
}

/// The roof riser's foot point and the beam's top (fix-lip `roof_ramp_v1.tsv`, `foot point, walk from rest`).
const RISER_FOOT: Vec3 = Vec3::new(36.704, 21.491, 113.223);
const RISER_TOP: Vec3 = Vec3::new(36.454, 22.448, 112.973);

#[test]
#[ignore = "a named case on the real mapdata; needs SRP_MAPDATA_DIR and SRP_GAME_DATA"]
fn the_roof_riser_is_climbed_from_its_foot_as_the_exe_climbs_it() {
    let Some(world) = shapes("level_02/day_2") else { return };
    let world = Client { world: &world };
    let yaw = crate::v026g::bots::kin::yaw_of([RISER_TOP.x - RISER_FOOT.x, RISER_TOP.z - RISER_FOOT.z]);
    let mut body = ClientBody::placed(RISER_FOOT + Vec3::new(0.0, 0.02, 0.0), yaw);
    // The exe's body is walked on a fixed heading from rest; its keys go down on the first input (they move the step after it).
    let mut top_at = None;
    for n in 0..12 {
        let t = body.tick(&world, Kin::CLIENT, Input { keys: keys::FORWARD, yaw });
        if top_at.is_none() && t.feet.y > RISER_TOP.y - 0.05 {
            top_at = Some(n);
        }
    }
    eprintln!("roof riser: on the top at {top_at:?} (the exe: its tick 2)");
    let at = top_at.expect("the body never got onto the beam's top; the exe is on it by its second tick");
    assert!(at <= 4, "on the top only at tick {at}; the exe is on it by its second tick (one more for the input's lag)");
}
