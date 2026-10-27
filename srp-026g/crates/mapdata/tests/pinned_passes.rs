//! The switched-off passes of the walk grid, pinned: a small world built with each pass on, its grid digest fixed, and the file key of
//! the default build fixed. A change that alters what a pass builds fails here until the pin is moved, and moving the pin is the
//! moment to raise the pass's constant (`GAP_LINKS_ALGO`, `EXACT_EROSION_ALGO`, `HONEST_DROPS_ALGO`, `CLIENT_BODY_ALGO`): the stored
//! grids are found by the file key, so a pass that changes without its constant raised lets a stale file load. The default build is
//! pinned by `grid.rs` (`PINNED_TWO_ROOMS`) and by the key here.

use mapdata::{
    Body, CLIENT_BODY_ALGO, EXACT_EROSION_ALGO, Erosion, GAP_LINKS_ALGO, GRID_ALGO, HONEST_DROPS_ALGO, OffMeshLinks, Triangle, Vec3,
    WalkGrid, WalkerBehaviour, grid_key_with_links,
};

fn quad(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> Vec<Triangle> {
    let (a, b, c, d) = (Vec3::new(x0, y, z0), Vec3::new(x1, y, z0), Vec3::new(x1, y, z1), Vec3::new(x0, y, z1));
    let walker = WalkerBehaviour::BySlope;
    vec![Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
}

/// A face across x at `x`, from z0 to z1, from y0 to y1.
fn face_x(x: f32, z0: f32, z1: f32, y0: f32, y1: f32) -> Vec<Triangle> {
    let (a, b, c, d) = (Vec3::new(x, y0, z0), Vec3::new(x, y0, z1), Vec3::new(x, y1, z1), Vec3::new(x, y1, z0));
    let walker = WalkerBehaviour::BySlope;
    vec![Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
}

/// A block from x0 to x1 (z 0 to 10), its top at `y`, standing on the ground at 0.
fn block(x0: f32, x1: f32, y: f32) -> Vec<Triangle> {
    let mut t = quad(x0, 0.0, x1, 10.0, y);
    t.extend(face_x(x0, 0.0, 10.0, 0.0, y));
    t.extend(face_x(x1, 0.0, 10.0, 0.0, y));
    t
}

/// Ground, a block 2 m high with a 1 m gap to a second, and a ramp from the second block's side back to the first: a gap jump
/// with a way back, a 3 m ledge (a drop) and a post in an opening for the erosion.
fn world() -> (Vec<Triangle>, [f32; 4]) {
    let mut t = quad(0.0, 0.0, 20.0, 20.0, 0.0);
    t.extend(block(2.0, 8.0, 2.0));
    t.extend(block(9.0, 14.0, 2.0));
    // A ledge in two falls, 1.5 m then 4.5 m more, for the honest drops (the second block's far side).
    t.extend(block(15.0, 16.0, 1.5));
    // A post 0.4 m wide in a 0.65 m opening of a wall across z at x 17..19.
    let wall = |z: f32, x0: f32, x1: f32| {
        let mut w = quad(x0, z, x1, z + 0.2, 3.0);
        w.extend(quad(x0, z, x1, z + 0.2, 0.0));
        w
    };
    t.extend(wall(10.0, 16.5, 18.0));
    t.extend(wall(10.0, 18.65, 20.0));
    // A box a sprint jump (1.2 m) up, with a run-up in front of it.
    t.extend(block(10.0, 11.0, 1.2).into_iter().map(|mut tr| {
        for v in &mut tr.v {
            v.z += 12.0;
        }
        tr
    }));
    (t, [0.0, 0.0, 20.0, 24.0])
}

fn digest_of(body: Body, links: OffMeshLinks) -> u64 {
    let (t, area) = world();
    WalkGrid::build_with_links(&t, area, 0.25, body, links).digest()
}

fn print(name: &str, value: u64) -> u64 {
    eprintln!("PIN {name} = {value}");
    value
}

/// Gap links: the walking and sprint jumps, the drops walked off, the two-way filter.
#[test]
fn the_gap_links_pass_is_pinned() {
    assert_eq!(GAP_LINKS_ALGO, 3, "raise GAP_LINKS_ALGO with the pin");
    let d = print("gap", digest_of(Body::default(), OffMeshLinks::Gaps));
    assert_ne!(d, digest_of(Body::default(), OffMeshLinks::None), "the world makes gap links");
    assert_eq!(d, PIN_GAP);
}

/// Exact erosion: the floors a wall comes nearer to than the radius less a quarter cell.
#[test]
fn the_exact_erosion_pass_is_pinned() {
    assert_eq!(EXACT_EROSION_ALGO, 2, "raise EXACT_EROSION_ALGO with the pin");
    let body = Body { erosion: Erosion::Exact, ..Body::default() };
    let d = print("exact", digest_of(body, OffMeshLinks::None));
    assert_ne!(d, digest_of(Body::default(), OffMeshLinks::None), "the world has an opening exact erosion changes");
    assert_eq!(d, PIN_EXACT);
}

/// Honest drops: a Drop is kept only if the body that walks off comes down within the harmless fall.
#[test]
fn the_honest_drops_pass_is_pinned() {
    assert_eq!(HONEST_DROPS_ALGO, 1, "raise HONEST_DROPS_ALGO with the pin");
    let body = Body { drop_flight_mps: 2.45, ..Body::default() };
    let d = print("honest", digest_of(body, OffMeshLinks::None));
    assert_ne!(d, digest_of(Body::default(), OffMeshLinks::None), "the world has a ledge in two falls the honest drops take away");
    assert_eq!(d, PIN_HONEST);
}

/// The clients' body (step 0.8, ground up to 55 degrees): the sprint jumps over the standing jump, the steep landings cut.
#[test]
fn the_client_body_pass_is_pinned() {
    assert_eq!(CLIENT_BODY_ALGO, 1, "raise CLIENT_BODY_ALGO with the pin");
    let d = print("client", digest_of(Body::client(), OffMeshLinks::None));
    assert_ne!(d, digest_of(Body::default(), OffMeshLinks::None), "the world differs on the clients' body");
    assert_eq!(d, PIN_CLIENT);
}

/// The file key of the default build is the one there was before the passes had constants: a default grid stored earlier is found.
#[test]
fn the_default_file_key_is_the_one_there_was() {
    assert_eq!(GRID_ALGO, 11);
    let (t, area) = world();
    let seeds = [Vec3::new(1.0, 0.0, 1.0)];
    let key = grid_key_with_links(&t, area, 0.25, Body::default(), &seeds, OffMeshLinks::None);
    eprintln!("PIN key = {key}");
    assert_eq!(key, PIN_DEFAULT_KEY);
    // And each pass is in its key.
    let k = |body: Body, links| grid_key_with_links(&t, area, 0.25, body, &seeds, links);
    let keys = [
        key,
        k(Body::default(), OffMeshLinks::Gaps),
        k(Body { erosion: Erosion::Exact, ..Body::default() }, OffMeshLinks::None),
        k(Body { drop_flight_mps: 2.45, ..Body::default() }, OffMeshLinks::None),
        k(Body::client(), OffMeshLinks::None),
    ];
    for (i, a) in keys.iter().enumerate() {
        for b in &keys[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

const PIN_GAP: u64 = 218_410_819_143_687_843;
const PIN_EXACT: u64 = 5_559_966_362_604_391_412;
const PIN_HONEST: u64 = 9_697_411_571_619_617_025;
const PIN_CLIENT: u64 = 10_680_625_577_665_009_494;
/// The default build's key before the passes had constants (taken from the code of c48d5455 for this world).
const PIN_DEFAULT_KEY: u64 = 3_673_131_261_731_810_027;
