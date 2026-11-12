//! [A364 P9] What a kept controller's step (`Controller::tick`) costs on a map's worlds, for the reckoning's `ClientStep::Kept`: the
//! triangle world with the grid's caster today (`Caster::Bullet`, face witness), the triangle world with the exe's own cast
//! (`Caster::Exe`), and the game's shapes (`Collision::build_exe`, a format-6 file). Bodies walk straight at the walk speed from a
//! fixed sample of floor triangles, turning every second, each world the same walks; one thread.
//!
//! ```text
//! SRP_KEPT_MAPS=<a.mapdata>,<b.mapdata> cargo test -p mapdata --release --test kept_cost -- --ignored --nocapture
//! ```

use mapdata::controller::tick::{Controller, Settings, TickInput, WallNormal};
use mapdata::controller::{Caster, Collision, StartOverlap};
use mapdata::{MapData, Vec3};

/// Walks of this many steps (50 ms) from each start.
const STEPS: usize = 60;
/// Starts taken from the map's floor triangles (every n-th).
const STARTS: usize = 300;

fn walk(world: &Collision, s: &Settings, starts: &[Vec3]) -> (u64, usize) {
    let mut ns = 0u64;
    let mut steps = 0usize;
    for (i, &feet) in starts.iter().enumerate() {
        let mut body = Controller::standing_at(feet + Vec3::new(0.0, 0.05, 0.0));
        for k in 0..STEPS {
            let heading = i as f32 * 0.7 + (k / 20) as f32 * 1.3;
            let walk = Vec3::new(heading.cos() * 3.5 * 0.05, 0.0, heading.sin() * 3.5 * 0.05);
            let input = TickInput { walk, air_control: Vec3::ZERO, crouch: mapdata::controller::tick::CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
            let started = std::time::Instant::now();
            body.tick(world, 0.05, s, input);
            ns += started.elapsed().as_nanos() as u64;
            steps += 1;
        }
    }
    (ns, steps)
}

#[test]
#[ignore = "needs SRP_KEPT_MAPS"]
fn a_kept_controllers_step_costs_this_on_each_world() {
    let Some(maps) = std::env::var_os("SRP_KEPT_MAPS") else {
        eprintln!("skipping: SRP_KEPT_MAPS is not set");
        return;
    };
    let s = Settings { wall_normal: WallNormal::Rays, ..Settings::LIVE };
    for path in maps.to_string_lossy().split(',') {
        let map = MapData::read(std::path::Path::new(path)).unwrap();
        let triangles = map.walker_triangles();
        let floors: Vec<Vec3> = triangles
            .iter()
            .filter_map(|t| {
                let (a, b, c) = (t.v[0], t.v[1], t.v[2]);
                let (u, v) = (b - a, c - a);
                let n = Vec3::new(u.y * v.z - u.z * v.y, u.z * v.x - u.x * v.z, u.x * v.y - u.y * v.x);
                let len = n.length();
                (len > 1e-6 && n.y.abs() / len > 0.9 && len > 0.5).then(|| Vec3::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0, (a.z + b.z + c.z) / 3.0))
            })
            .collect();
        let every = (floors.len() / STARTS).max(1);
        let starts: Vec<Vec3> = floors.iter().step_by(every).take(STARTS).copied().collect();
        let built = |name: &str, make: &dyn Fn() -> Option<Collision>| {
            let started = std::time::Instant::now();
            let world = make();
            let seconds = started.elapsed().as_secs_f32();
            if let Some(world) = world {
                let (ns, steps) = walk(&world, &s, &starts);
                eprintln!(
                    "{path} {name}: built {seconds:.2} s, {:.1} MB; {steps} steps, {:.1} us a step",
                    world.bytes() as f32 / 1e6,
                    ns as f32 / steps.max(1) as f32 / 1e3
                );
            } else {
                eprintln!("{path} {name}: no world (no shapes in the file)");
            }
        };
        built("triangles Bullet(face witness)", &|| Some(Collision::build(&triangles).with_caster(Caster::Bullet(StartOverlap::FaceWitness))));
        built("triangles Exe(face witness)", &|| Some(Collision::build(&triangles).with_caster(Caster::Exe(StartOverlap::FaceWitness))));
        built("shapes (build_exe)", &|| Collision::build_exe(&map, StartOverlap::FaceWitness));
    }
}
