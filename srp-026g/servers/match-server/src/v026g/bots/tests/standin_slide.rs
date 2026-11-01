//! The client's slide off steep ground, for the stand-in's bodies (C14).
//!
//! The client slides a body by its ground's contact (`need_slide_on_supporting_surface` 0x7bc20, PROVEN): steeper than
//! `cc_max_slope_angle_cos` (55 degrees at run time) moving or not, steeper than `cc_idle_max_slope_angle_cos` (`startup.cfg`
//! 0.70711: 45 degrees) when its speed is under 1e-5 squared. The face's game material decides first: the down sweep's callback
//! (`character_step_down_sweep_test_callback::addSingleResult` 0x84420) takes a `walk` face's normal as straight up (never slid)
//! and never takes a `dont_walk` face as ground (always slid); the cooked face carries the material's `physic.walker_behaviour`
//! (the collision cook 0x82bf0, PROVEN). The push is gravity along the face (`get_slide_acceleration` 0x7ba30); no friction
//! (`update_slide_velocity` 0x7fea0, LIKELY).
//!
//! The stand-in has no slide: a body holding, reloading or camping on such ground stands there. With `SRP_BENCH_STAND_IN_SLIDE=on`
//! it slides down the lattice, its lowest walk or drop neighbour at a time, gaining `GRAVITY x sin(slope)` a second (GUESS for the
//! scale: the end point matters more than the speed), until its ground no longer slides it.

use std::collections::HashMap;
use std::sync::Arc;

use mapdata::{LinkKind, NodeId, RayTree, Triangle, Vec3, WalkGrid, WalkerBehaviour};

use super::settings::bench_settings;
use survarium::v026g::match_msgs::TICK_MS;

/// Whether the stand-in's bodies slide off ground the client slides them off (`SRP_BENCH_STAND_IN_SLIDE=on`).
pub(crate) fn stand_in_slide() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_SLIDE").is_ok_and(|v| v == "on"))
}

/// The client's slope limit, degrees (`cc_max_slope_angle_cos 0.57358`): steeper slides a body moving or not.
pub(crate) const SLOPE_LIMIT_DEG: f32 = 55.0;
/// The client's slope a body standing still slides on, degrees (`cc_idle_max_slope_angle_cos 0.70711`).
pub(crate) const IDLE_LIMIT_DEG: f32 = 45.0;
/// Gravity, m/s² (`fall_damage.md`, PROVEN: 0.49 m/s a 0.05 s tick).
const GRAVITY: f32 = 9.8;
/// A slide's next floor is a neighbour, not a drop's far landing: within this, m (xz).
const NEXT_M: f32 = 0.6;

/// The ground under a body, by the face a ray down meets first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Ground {
    /// A `walk` face: ground at any slope, never slid; its slope, degrees.
    Walk(f32),
    /// A face walked by its slope, degrees.
    Slope(f32),
    /// A `dont_walk` face: never ground, always slid.
    DontWalk,
}

impl Ground {
    /// Whether the client slides a body on it; `still`: the body stands still.
    pub(crate) fn slides(self, still: bool) -> bool {
        match self {
            Ground::Walk(_) => false,
            Ground::Slope(deg) => deg >= SLOPE_LIMIT_DEG || (still && deg >= IDLE_LIMIT_DEG),
            Ground::DontWalk => true,
        }
    }
}

/// The ground of the floors bodies stand on, once a floor: the faces by their material, each class in its own ray tree; the class of
/// the face a ray down meets first in the whole mesh is the one whose tree meets it at the same distance.
pub(crate) struct Grounds {
    all: Arc<RayTree>,
    slope: RayTree,
    dont: RayTree,
    of: HashMap<NodeId, Option<Ground>>,
}

impl Grounds {
    /// `all`: the tree of every walker face (`Game::walls`).
    pub(crate) fn new(triangles: &[Triangle], all: Arc<RayTree>) -> Self {
        let class = |walker: WalkerBehaviour| RayTree::from_corners(triangles.iter().filter(|t| t.walker == walker).map(|t| t.v));
        Self {
            all,
            slope: class(WalkerBehaviour::BySlope),
            dont: class(WalkerBehaviour::DontWalk),
            of: HashMap::new(),
        }
    }

    /// The ground of `node`, whose body's feet are at `feet` (none: no face under it).
    pub(crate) fn at(&mut self, node: NodeId, feet: Vec3) -> Option<Ground> {
        let (all, slope, dont) = (&self.all, &self.slope, &self.dont);
        *self.of.entry(node).or_insert_with(|| ground_under(all, slope, dont, feet))
    }
}

/// The first face a ray down from 0.5 m over the feet meets within 1.5 m (a floor of the lattice on a slope stands over its face, by
/// the capsule's rest), and its class.
fn ground_under(all: &RayTree, slope: &RayTree, dont: &RayTree, feet: Vec3) -> Option<Ground> {
    let (from, down) = (feet + Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, -1.0, 0.0));
    let first = all.ray_hit(from, down, 1.5)?;
    let same = |tree: &RayTree| tree.ray_hit(from, down, 1.5).filter(|hit| (hit.t - first.t).abs() < 1e-4);
    let deg = |normal: Vec3| normal.y.abs().clamp(0.0, 1.0).acos().to_degrees();
    Some(if same(slope).is_some() {
        Ground::Slope(deg(first.normal))
    } else if same(dont).is_some() {
        Ground::DontWalk
    } else {
        Ground::Walk(deg(first.normal))
    })
}

/// The speed a slide gains in a tick on ground of `ground`, m/s (a `dont_walk` face: as on a wall).
pub(crate) fn gain_mps(ground: Ground) -> f32 {
    let deg = match ground {
        Ground::Slope(deg) => deg,
        Ground::DontWalk => 90.0,
        Ground::Walk(_) => 0.0,
    };
    GRAVITY * deg.to_radians().sin() * TICK_MS as f32 / 1000.0
}

/// What the bodies of one game slid ([`stand_in_slide`]): counting only.
#[derive(Default)]
pub(crate) struct Slides {
    /// Slides begun, and the ticks and metres (xz) slid.
    pub(crate) started: u32,
    pub(crate) ticks: u64,
    pub(crate) metres: f32,
    /// The ticks a body stood on ground that slides it with no floor below to slide to (it stands there, as without the switch).
    pub(crate) no_floor: u32,
}

impl Slides {
    /// One tick of a body on ground that slides it: `from` its feet before, `to` after, `began` its first tick; `to` none: no floor below.
    pub(crate) fn note(&mut self, began: bool, from: Vec3, to: Option<Vec3>) {
        match to {
            Some(to) => {
                self.started += u32::from(began);
                self.ticks += 1;
                self.metres += from.distance_xz(to);
            }
            None => self.no_floor += 1,
        }
    }

    /// The counts for a cell's log.
    pub(crate) fn line(&self) -> String {
        format!(
            "slides: started {} ticks {} metres {:.1} no_floor {}",
            self.started, self.ticks, self.metres, self.no_floor
        )
    }
}

/// A slide's move this tick from `node` (the body's feet at `feet`) at `speed` m/s: toward its lowest walk or drop neighbour below it,
/// onto it once reached. None: no floor below to slide to.
pub(crate) fn slide_step(grid: &WalkGrid, node: NodeId, feet: Vec3, speed: f32) -> Option<(NodeId, Vec3)> {
    let here = grid.position(node);
    let next = grid
        .links(node)
        .filter(|&(_, kind)| matches!(kind, LinkKind::Walk | LinkKind::Drop))
        .map(|(to, _)| (to, grid.position(to)))
        .filter(|(_, at)| at.y < here.y - 0.02 && at.distance_xz(here) <= NEXT_M)
        .min_by(|a, b| a.1.y.total_cmp(&b.1.y))?;
    let step = speed * TICK_MS as f32 / 1000.0;
    let left = feet.distance_xz(next.1);
    Some(if left <= step {
        next
    } else {
        let s = step / left;
        (node, Vec3::new(feet.x + (next.1.x - feet.x) * s, feet.y, feet.z + (next.1.z - feet.z) * s))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_material_decides_first_then_the_slope_and_whether_the_body_stands_still() {
        assert!(!Ground::Walk(70.0).slides(true));
        assert!(Ground::DontWalk.slides(false));
        assert!(!Ground::Slope(44.0).slides(true));
        assert!(Ground::Slope(46.0).slides(true));
        assert!(!Ground::Slope(46.0).slides(false));
        assert!(Ground::Slope(56.0).slides(false));
        // 0.05 s of gravity along a 30 degree face: 0.245 m/s.
        assert!((gain_mps(Ground::Slope(30.0)) - 0.245).abs() < 1e-3);
    }

    #[test]
    fn slides_count_their_ticks_metres_and_ends_with_no_floor() {
        let mut s = Slides::default();
        let at = |x: f32| Vec3::new(x, 0.0, 0.0);
        s.note(true, at(0.0), Some(at(0.1)));
        s.note(false, at(0.1), Some(at(0.3)));
        s.note(false, at(0.3), None);
        assert_eq!((s.started, s.ticks, s.no_floor), (1, 2, 1));
        assert!(s.line().starts_with("slides: started 1 ticks 2 metres 0.3 no_floor 1"), "{}", s.line());
    }

    #[test]
    fn a_ray_down_tells_the_faces_class() {
        // A walk floor over x 0..10, a by-slope 50 degree plane over x 20..30, a dont_walk floor over x 40..50.
        let quad = |x0: f32, tan: f32, walker: WalkerBehaviour| {
            let p = |x: f32, z: f32| Vec3::new(x, (x - x0) * tan, z);
            [
                Triangle { v: [p(x0, 0.0), p(x0 + 10.0, 0.0), p(x0 + 10.0, 10.0)], walker },
                Triangle { v: [p(x0, 0.0), p(x0 + 10.0, 10.0), p(x0, 10.0)], walker },
            ]
        };
        let tris: Vec<Triangle> = [
            quad(0.0, 0.0, WalkerBehaviour::Walk),
            quad(20.0, 50f32.to_radians().tan(), WalkerBehaviour::BySlope),
            quad(40.0, 0.0, WalkerBehaviour::DontWalk),
        ]
        .into_iter()
        .flatten()
        .collect();
        let all = Arc::new(RayTree::build(&tris));
        let mut g = Grounds::new(&tris, all);
        assert_eq!(g.at(1, Vec3::new(5.0, 0.0, 5.0)), Some(Ground::Walk(0.0)));
        let Some(Ground::Slope(deg)) = g.at(2, Vec3::new(25.0, 5.0 * 50f32.to_radians().tan(), 5.0)) else { panic!("a slope") };
        assert!((deg - 50.0).abs() < 0.1, "{deg}");
        assert_eq!(g.at(3, Vec3::new(45.0, 0.0, 5.0)), Some(Ground::DontWalk));
        assert_eq!(g.at(4, Vec3::new(65.0, 0.0, 5.0)), None);
    }
}
