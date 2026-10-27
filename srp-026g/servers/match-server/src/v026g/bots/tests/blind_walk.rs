//! A body with no world for a long walk (J0 0146be4a bot 3: the last world it was given was taken mid-fall at 28.65 s; the bot sprinted on
//! for 72 s). What its reckoning does with that world and a long run of the same keys.

use super::*;

use crate::v026g::bots::body::BodyTrack;
use crate::v026g::bots::kin::Kinematics;

/// The estimate of a body given the fix of 28.65 s (feet (-50.24, 4.17, 31.32) above the floor of Chemical Plant, falling at 4.9 m/s, sprint and
/// forward held, yaw -1.37) and then 72 s of the same keys, printed every 5 s. A measurement, run with
/// `SRP_MAPDATA_DIR=<mapdata folder> cargo test -p match-server --release blind_walk -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn blind_walk_after_a_mid_air_world() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_04/day_4"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let kin = Kinematics::default();
    let mut track = BodyTrack::default();
    let feet = Vec3::new(-50.24, 4.17, 31.32);
    let fix = Fix { time: 28_650, alive: true, inserted: 1, feet: Some(feet), yaw: -1.37, pitch: 0.0, keys: 513, speed: Some(5.08) };
    track.fixed(fix, &kin, Some(&world.grid));
    let goal = Vec3::new(-38.38, 1.96, 33.67);
    let at = track.estimate().copied().unwrap();
    println!("fix: node {:?}, floor under {:?}", at.node, world.grid.locate(feet, 3.0).map(|n| world.grid.position(n)));
    for t in (28_675..=100_750).step_by(25) {
        track.decided(t, 513, (0.0, 0.0), 0.0, &kin, Some(&world.grid));
        if t % 5_000 == 0 {
            let e = track.estimate().unwrap();
            println!("t {t}: ({:.2}, {:.2}, {:.2}) node {:?}, {:.1} m from the goal", e.feet.x, e.feet.y, e.feet.z, e.node, e.feet.distance_xz(goal));
        }
    }
}
