//! Where a body can use a container's box: the **stands** (batteries
//! WP-B, F7). The game decides a use by one ray from the eye along the view,
//! [`USE_RAY_M`] long, that must meet the container's use shape with nothing
//! of the bullet mesh short of it (`base_player::detect_usable_object`
//! 0x2D84C0, PROVEN; `client_patch.md` section 3.23). A stand is a floor of
//! the walk grid near the box from which some aim within the view's pitch
//! gives such a ray, at the eye heights a body has there.
//!
//! One list per container, made once per map from the game's own test
//! ([`press`]) and shared by the put-in, a theft (`raid.rs`), the holder's
//! sight (`guard_lead.rs`) and the bench stand-in, so none of them judges a
//! use by a proxy of its own.
//!
//! The test is a little stricter than the game's scan (which stops once the
//! walkable and hittable groups were both hit), and a stand holds only when
//! the ray passes at **both** eye heights of a pose ([`EYE_STANDING`],
//! [`EYE_CROUCHED`]). The ray starts at the player's head matrix, an
//! animated bone (PROVEN, 0x2D84C0); its height over the feet was measured
//! on live bullets, which start at the same matrix: standing 1.47 m (sd
//! 0.03, 1.42-1.51 for the 10th to 90th percentile), crouched 1.04 m still
//! and 1.14 m (sd 0.06) on the move, and it moves 0.16-0.22 m with the sine
//! of the view's pitch. The two heights of a pose bound that, so a stand
//! has a margin (`bots::view::eye_height` gives the bots' own).

use crate::{RayTree, Rotation, Shape, Vec3, World, ZoneKind};

/// The use ray's length the stands were first made for, metres: the client's
/// compiled default of `usable_objects_detection_distance` (0xA710D8, PROVEN).
pub const USE_RAY_M: f32 = 2.0;
/// The use ray's length the client runs with, metres: the stock `startup.cfg`
/// sets `usable_objects_detection_distance` to 1.80 (PROVEN; the cvar is the
/// float `detect_usable_object` 0x2D84C0 loads as the ray's length).
pub const USE_RAY_CLIENT_M: f32 = 1.8;
/// The view's pitch limit, radians (LIKELY; `kin.rs`).
pub const PITCH_MAX_RAD: f32 = 1.0;
/// The two eye heights over the feet a standing stand must pass at, metres:
/// the bullet's 1.46 m with the view's pitch within a quarter radian
/// (1.42-1.50) and the spread of live bullets (sd 0.03) round it.
pub const EYE_STANDING: [f32; 2] = [1.40, 1.52];
/// The two eye heights over the feet a crouched stand must pass at, metres:
/// 1.04 m still and 1.14 m moving, the same pitch round them (0.99-1.19)
/// and the spread (sd 0.06).
pub const EYE_CROUCHED: [f32; 2] = [0.97, 1.25];
/// The ranges before the measurement (the eye's height was not read from the
/// client), for [`UseStands::build_with`]: the bench's off switch of the
/// fix.
pub const GUESSED_EYE_STANDING: [f32; 2] = [1.5, 1.7];
pub const GUESSED_EYE_CROUCHED: [f32; 2] = [0.9, 1.0];
/// How far from the box's bounds a floor is looked at, metres.
pub const SEARCH_M: f32 = 3.5;
/// The floors looked at lie from this far under the box's middle...
const BELOW_M: f32 = 3.0;
/// ... to this far over it.
const ABOVE_M: f32 = 1.0;
/// Sample points per axis of the box (the grid of aims tried).
const SAMPLES: usize = 6;

/// How a body stands to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsePose {
    Standing,
    Crouched,
}

/// A floor a body can use a container's box from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseStand {
    /// The floor.
    pub feet: Vec3,
    /// Standing, the ray passes at both of [`EYE_STANDING`].
    pub standing: bool,
    /// Crouched, the ray passes at both of [`EYE_CROUCHED`].
    pub crouched: bool,
    /// A point of the box the press ray meets (from the lower standing
    /// eye, else the lower crouched one).
    pub aim: Vec3,
    /// The view's pitch to `aim`, radians.
    pub pitch: f32,
}

impl UseStand {
    /// Whether a body in `pose` can use from here.
    pub fn fits(&self, pose: UsePose) -> bool {
        match pose {
            UsePose::Standing => self.standing,
            UsePose::Crouched => self.crouched,
        }
    }
}

/// The stands of every container of a map, by the container's index in
/// `MapData::containers`, nearest the box's middle (across) first; and of
/// every resupply zone's box, by the zone's index in `MapData::zones` (the
/// zone is a `usable_object` of the same kind, `resupply_zone_core`, found by
/// the same ray: 0x2D84C0, 0x2F8080).
#[derive(Clone, Debug, Default)]
pub struct UseStands {
    lists: Vec<Vec<UseStand>>,
    ids: Vec<i32>,
    zones: Vec<Vec<UseStand>>,
}

impl UseStands {
    /// Works out every container's stands: hundreds of floors a box, each
    /// asked [`SAMPLES`]^3 aims at four eye heights; a few milliseconds a
    /// map, once.
    pub fn build(world: &World) -> Self {
        Self::build_with(world, EYE_STANDING, EYE_CROUCHED)
    }

    /// [`Self::build`] with other pairs of eye heights.
    pub fn build_with(world: &World, standing: [f32; 2], crouched: [f32; 2]) -> Self {
        Self::build_ray(world, standing, crouched, USE_RAY_M)
    }

    /// [`Self::build_with`] with a use ray of `ray_m` metres.
    pub fn build_ray(world: &World, standing: [f32; 2], crouched: [f32; 2], ray_m: f32) -> Self {
        let lists = world
            .map
            .containers
            .iter()
            .map(|c| stands_of(world, c.use_shapes.first(), standing, crouched, ray_m))
            .collect();
        let ids = world.map.containers.iter().map(|c| c.id).collect();
        let zones = world
            .map
            .zones
            .iter()
            .map(|z| match z.kind {
                ZoneKind::Resupply => stands_of(world, z.shapes.first(), standing, crouched, ray_m),
                _ => Vec::new(),
            })
            .collect();
        Self { lists, ids, zones }
    }

    /// The stands of the resupply zone at `index` of `MapData::zones` (empty
    /// for any other zone or an unknown one).
    pub fn of_zone(&self, index: usize) -> &[UseStand] {
        self.zones.get(index).map_or(&[], |list| list)
    }

    /// The stands of the container with this `id` (empty for an unknown
    /// one).
    pub fn of(&self, id: impl Into<i32>) -> &[UseStand] {
        let id = id.into();
        self.ids
            .iter()
            .position(|&i| i == id)
            .map_or(&[], |k| &self.lists[k])
    }
}

fn stands_of(
    world: &World,
    shape: Option<&Shape>,
    eye_standing: [f32; 2],
    eye_crouched: [f32; 2],
    ray_m: f32,
) -> Vec<UseStand> {
    let Some(shape) = shape else {
        return Vec::new();
    };
    let grid = &world.grid;
    let (lo, hi) = shape.bounds();
    let area = [
        lo.x - SEARCH_M,
        lo.z - SEARCH_M,
        hi.x + SEARCH_M,
        hi.z + SEARCH_M,
    ];
    let mut out: Vec<UseStand> = Vec::new();
    for node in grid.floors_in(area) {
        if !grid.reached(node) {
            continue;
        }
        let feet = grid.position(node);
        if feet.y < shape.center.y - BELOW_M || feet.y > shape.center.y + ABOVE_M {
            continue;
        }
        let at = |heights: [f32; 2]| -> Option<(Vec3, f32)> {
            let first = press(&world.sight, feet + Vec3::new(0.0, heights[0], 0.0), shape, ray_m)?;
            press(&world.sight, feet + Vec3::new(0.0, heights[1], 0.0), shape, ray_m)?;
            Some(first)
        };
        let standing = at(eye_standing);
        let crouched = at(eye_crouched);
        if let Some((aim, pitch)) = standing.or(crouched) {
            out.push(UseStand {
                feet,
                standing: standing.is_some(),
                crouched: crouched.is_some(),
                aim,
                pitch,
            });
        }
    }
    out.sort_by(|a, b| {
        let key = |s: &UseStand| s.feet.distance_xz(shape.center);
        key(a)
            .total_cmp(&key(b))
            .then(a.feet.x.total_cmp(&b.feet.x))
            .then(a.feet.z.total_cmp(&b.feet.z))
            .then(a.feet.y.total_cmp(&b.feet.y))
    });
    out
}

/// The game's use test from `eye` against `shape`: some aim, within the
/// view's pitch, whose `ray_m` ray meets the shape with nothing of
/// the bullet mesh short of it. The aim (a point of the box) and its pitch.
pub fn press(sight: &RayTree, eye: Vec3, shape: &Shape, ray_m: f32) -> Option<(Vec3, f32)> {
    let n = SAMPLES;
    for &(ix, iy, iz) in aim_order() {
        let local = Vec3::new(
            shape.half.x * (ix as f32 / n as f32 * 2.0 - 1.0),
            shape.half.y * (iy as f32 / n as f32 * 2.0 - 1.0),
            shape.half.z * (iz as f32 / n as f32 * 2.0 - 1.0),
        );
        let aim = shape.center + shape.rotation.apply(local);
        let v = aim - eye;
        let len = v.length();
        if len < 1e-3 {
            return Some((aim, 0.0));
        }
        let dir = v * (1.0 / len);
        let pitch = dir.y.atan2(dir.x.hypot(dir.z));
        if pitch.abs() > PITCH_MAX_RAD {
            continue;
        }
        let Some(t) = ray_box(eye, dir, shape.center, &shape.rotation, shape.half) else {
            continue;
        };
        if t > ray_m {
            continue;
        }
        if sight.ray_cast(eye, dir, (t - 0.02).max(0.0)).is_none() {
            return Some((aim, pitch));
        }
    }
    None
}

/// The aim samples' grid indices, the middle of the box first and then
/// outward: the aim a press looks at is the most central one that works.
fn aim_order() -> &'static [(usize, usize, usize)] {
    static ORDER: std::sync::OnceLock<Vec<(usize, usize, usize)>> = std::sync::OnceLock::new();
    ORDER.get_or_init(|| {
        let n = SAMPLES;
        let mut order: Vec<(usize, usize, usize)> = (0..=n)
            .flat_map(|ix| (0..=n).flat_map(move |iy| (0..=n).map(move |iz| (ix, iy, iz))))
            .collect();
        let off = |i: usize| (i as i32 * 2 - n as i32).pow(2);
        order.sort_by_key(|&(ix, iy, iz)| off(ix) + off(iy) + off(iz));
        order
    })
}

/// Whether a use ray from `eye` along `dir` (unit) meets `shape` by the
/// game's rule: within `ray_m`, nothing of the bullet mesh short of
/// it.
pub fn ray_meets(sight: &RayTree, eye: Vec3, dir: Vec3, shape: &Shape, ray_m: f32) -> bool {
    ray_box(eye, dir, shape.center, &shape.rotation, shape.half)
        .is_some_and(|t| t <= ray_m && sight.ray_cast(eye, dir, (t - 0.02).max(0.0)).is_none())
}

/// How far along a ray from `eye` along `dir` (unit) the oriented `shape`
/// is entered, if the ray meets it at all (whatever its length): the length
/// of the use ray that reaches the box, to set against the client's
/// `usable_objects_detection_distance`.
pub fn ray_distance(eye: Vec3, dir: Vec3, shape: &Shape) -> Option<f32> {
    ray_box(eye, dir, shape.center, &shape.rotation, shape.half)
}

/// Where a ray from `origin` along `dir` (unit) enters the oriented box;
/// `None` if it misses.
fn ray_box(origin: Vec3, dir: Vec3, centre: Vec3, rotation: &Rotation, half: Vec3) -> Option<f32> {
    let t = rotation.transposed();
    let lo = t.apply(origin - centre);
    let ld = t.apply(dir);
    let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
    for (o, d, h) in [
        (lo.x, ld.x, half.x),
        (lo.y, ld.y, half.y),
        (lo.z, ld.z, half.z),
    ] {
        if d.abs() < 1e-9 {
            if o.abs() > h {
                return None;
            }
        } else {
            let (a, b) = ((-h - o) / d, (h - o) / d);
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            t0 = t0.max(a);
            t1 = t1.min(b);
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(t0)
}

/// Whether the floor `node` is one of `stands` (to the cell).
pub fn is_stand(stands: &[UseStand], feet: Vec3) -> bool {
    stands
        .iter()
        .any(|s| s.feet.distance_xz(feet) < 0.01 && (s.feet.y - feet.y).abs() < 0.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, Container, MapData, ShapeKind, Triangle, WalkGrid, WalkerBehaviour, Zone};

    fn quad(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> [Triangle; 2] {
        let v = |x, z| Vec3::new(x, y, z);
        [
            Triangle {
                v: [v(x0, z0), v(x1, z0), v(x1, z1)],
                walker: WalkerBehaviour::BySlope,
            },
            Triangle {
                v: [v(x0, z0), v(x1, z1), v(x0, z1)],
                walker: WalkerBehaviour::BySlope,
            },
        ]
    }

    /// A box on a flat floor, solid in the sight tree (12 triangles).
    fn solid_box(center: Vec3, half: Vec3) -> Vec<Triangle> {
        let corner = |i: usize| {
            let pick = |bit: usize, h: f32| if i >> bit & 1 == 1 { h } else { -h };
            center + Vec3::new(pick(0, half.x), pick(1, half.y), pick(2, half.z))
        };
        let faces = [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ];
        faces
            .iter()
            .flat_map(|&[a, b, c, d]| {
                [[a, b, c], [a, c, d]].map(|v| Triangle {
                    v: v.map(corner),
                    walker: WalkerBehaviour::BySlope,
                })
            })
            .collect()
    }

    fn world(wall: bool) -> (World, Shape) {
        let floor = quad(-10.0, -10.0, 10.0, 10.0, 0.0);
        let centre = Vec3::new(0.0, 1.0, 0.0);
        let half = Vec3::new(1.0, 0.85, 0.65);
        let shape = Shape {
            kind: ShapeKind::Box,
            center: centre,
            rotation: Rotation::IDENTITY,
            half,
        };
        let mut sight_tris = floor.to_vec();
        sight_tris.extend(solid_box(centre, half));
        if wall {
            // A wall across x = 2.5, between the box and the east side.
            sight_tris.extend(solid_box(
                Vec3::new(2.5, 1.5, 0.0),
                Vec3::new(0.1, 1.5, 6.0),
            ));
        }
        let grid = WalkGrid::build(&floor, [-10.0, -10.0, 10.0, 10.0], 0.25, Body::default());
        let sight = RayTree::build(&sight_tris);
        let map = MapData {
            containers: vec![Container {
                position: Vec3::new(0.0, 0.0, 0.0),
                team: 0,
                id: 0,
                use_shapes: vec![shape],
            }],
            ..MapData::default()
        };
        (World::assemble(map, grid.into(), sight.into()), shape)
    }

    #[test]
    fn a_shorter_use_ray_keeps_only_stands_the_longer_one_has() {
        let (world, _) = world(false);
        let eyes = (EYE_STANDING, EYE_CROUCHED);
        let long = UseStands::build_ray(&world, eyes.0, eyes.1, USE_RAY_M);
        let short = UseStands::build_ray(&world, eyes.0, eyes.1, USE_RAY_CLIENT_M);
        assert!(!short.of(0u8).is_empty(), "1.8 m still has stands");
        assert!(short.of(0u8).len() < long.of(0u8).len(), "and fewer than 2.0 m");
        assert!(short.of(0u8).iter().all(|s| is_stand(long.of(0u8), s.feet)));
        // build_with is the 2.0 m list.
        assert_eq!(UseStands::build_with(&world, eyes.0, eyes.1).of(0u8), long.of(0u8));
    }

    #[test]
    fn stands_are_floors_whose_press_ray_hits_the_box() {
        let (world, shape) = world(false);
        let stands = UseStands::build(&world);
        let list = stands.of(0u8);
        assert!(!list.is_empty(), "a box on a flat floor has stands");
        for s in list {
            let eye = s.feet + Vec3::new(0.0, EYE_STANDING[0], 0.0);
            let (aim, _) = press(&world.sight, eye, &shape, USE_RAY_M).expect("a listed stand presses");
            assert!((aim - s.aim).length() < 1e-4, "{s:?}");
            // Within the ray's reach of the box's faces.
            assert!(
                s.feet.distance_xz(shape.center) < 1.0 + USE_RAY_M + 0.5,
                "{s:?}"
            );
        }
        // Nearest first.
        let d: Vec<f32> = list
            .iter()
            .map(|s| s.feet.distance_xz(shape.center))
            .collect();
        assert!(d.windows(2).all(|w| w[0] <= w[1]));
        // Nothing far away.
        assert!(!is_stand(list, Vec3::new(8.0, 0.0, 8.0)));
        assert!(is_stand(list, list[0].feet));
    }

    #[test]
    fn a_wall_between_takes_the_stands_behind_it_away() {
        let (open, _) = world(false);
        let (walled, _) = world(true);
        let behind = |w: &World| {
            UseStands::build(w)
                .of(0u8)
                .iter()
                .filter(|s| s.feet.x > 2.7)
                .count()
        };
        assert!(behind(&open) > 0, "stands east of x 2.7 on an open floor");
        assert_eq!(behind(&walled), 0, "none behind the wall");
    }

    #[test]
    fn a_resupply_zones_box_has_stands_by_the_zones_index_and_other_zones_none() {
        let (mut world, shape) = world(false);
        let zone = |kind| Zone {
            kind,
            name: String::new(),
            team: 0,
            position: shape.center,
            damage_type: String::new(),
            damage: None,
            shapes: vec![shape],
        };
        world.map.zones = vec![zone(ZoneKind::Damage), zone(ZoneKind::Resupply)];
        let stands = UseStands::build(&world);
        assert!(stands.of_zone(0).is_empty(), "a damage zone is not used");
        assert_eq!(stands.of_zone(1), stands.of(0u8), "the same box, the same list");
        assert!(!stands.of_zone(1).is_empty());
        assert!(stands.of_zone(5).is_empty(), "no such zone");
    }

    #[test]
    fn building_twice_gives_the_same_list() {
        let (w, _) = world(false);
        assert_eq!(UseStands::build(&w).of(0u8), UseStands::build(&w).of(0u8));
        assert!(UseStands::build(&w).of(9u8).is_empty());
    }
}
