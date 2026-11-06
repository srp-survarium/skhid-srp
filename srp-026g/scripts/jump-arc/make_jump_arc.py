"""make_jump_arc.py <anim dir> <out jump_arc.rs>: writes servers/match-server/src/v026g/bots/jump_arc.rs (A318) with the jump clips'
root tables evaluated from the stock clips (clip_table.py's B-spline code). LF line ends. See README.md beside it for the clips."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import clip_table  # noqa: E402

TABLES = [
    ('SHORT_NONE', 'on_site'), ('SHORT_F', 'on_site_fwd'), ('SHORT_FR', 'on_site_fwd_right'), ('SHORT_R', 'on_site_right'),
    ('SHORT_BR', 'on_site_back_right'), ('SHORT_B', 'on_site_back'), ('SHORT_BL', 'on_site_back_left'), ('SHORT_L', 'on_site_left'),
    ('SHORT_FL', 'on_site_fwd_left'), ('SPRINT_F', 'sprint__sprint_from_left_leg'), ('SPRINT_FR', 'sprint__sprint_right_from_left_leg'),
    ('SPRINT_FL', 'sprint__sprint_left_from_left_leg'),
]


def num(v):
    s = '%.4f' % v
    return '0.0' if s in ('0.0000', '-0.0000') else s


def table(anim, clip):
    p = os.path.join(anim, 'animations__single__weapons__ak__player__stand__3rd_view__jump__%s.b-spline' % clip)
    t = clip_table.tracks(p)
    X, end = clip_table.curve(t[0])
    Y, _ = clip_table.curve(t[1])
    Z, _ = clip_table.curve(t[2])
    assert int(end) == 24, (clip, end)
    rows = ['[%s, %s, %s]' % (num(Y(f)), num(Z(f)), num(X(f))) for f in range(25)]
    return rows


CODE = r'''//! [A318] The client's jump for the reckoned body ([`super::kin::Kinematics::jump_arc`]): the arc the jump clip's root draws,
//! instead of the old snap onto a grid leap link the step crosses within 0.8 s (`body::jumped_to`).
//!
//! The client (PROVEN unless marked; `private/sprint-jump-re/FINDINGS.md`, `private/grid/controller/dis/notes-tick.md`):
//! - The jump code from the keys at the press (`player_logic_jump_state::initialize` 0x337040): forward, forward-right or
//!   forward-left with sprint and forward held (no back, fire, crouch, aim) and the body at the walk cap give the sprint jump; else
//!   the short jump of the keys' direction (`get_move_direction` 0x2D6510: forward and back cancel, left and right cancel).
//! - The clip (24 frames, 30 a second; the sprint jumps at the sprint time scale 0.909) moves the body by its root: before the
//!   clip's `jump` event (frame 6) on the ground, after it in the air (`base_player::jump` 0x2D7920 sets the jump mode;
//!   `player_step`'s AIR branch 0x7BF2C: no gravity, the root's rise and travel plus the air control).
//! - The air control (`get_air_control_vector` 0x2D7990, re-kit inferred-high): the keys' direction times `air_control_speed`
//!   (1.0 m/s in `default.player`); none for forward alone in a sprint jump.
//! - The capsule sweeps stop the travel at a wall (`step_jump_forward` 0x7CB90) and set the body down on what it meets coming down
//!   (`step_jump_down` 0x7D0F0).
//! - The jump ends at the clip's end (`jump_logic_state_landing::initialize` 0x3440E0 calls `end_jump` 0x7F050).
//!
//! In the air the reckoning runs the client's own air branch (`mapdata::controller::tick`, `player_step` 0x7BF2C, proven in
//! Unicorn) against the map's walker collision (`body::walker_collision`, one a match): each tick the walk vector is the clip's root
//! target less the body's feet (`set_physics_controller_walk_vector` 0x2D9060 takes the whole difference while jumping, so a body a
//! face held catches up once clear), the air control is the keys' 1 m/s, and the capsule (0.76 x 1.8 m) sweeps up, across (sliding
//! along what it meets) and down. A railing's top rail, a sill or a ledge's lip the body comes down on holds it as it holds the
//! client. At the clip's end `end_jump`; standing (`on_ground`) it has landed, else it falls. With no collision (no map) the root
//! is flown over the grid's floors alone.
//! LIKELY: the sprint jump's leg (the right leg's clip rises 1.350 m, the left's 1.282 m: the left's is taken), the short jump's
//! time scale 1, and the 0.1 m catch (the capsule's lower sphere on a lip). Also LIKELY: the clip's root alone from the press;
//! the jump's animation lexeme is blended in over 0.3 s (the builder at 0x343CCF), so the client's first ~6 ticks mix the clip's
//! root with the walk it leaves, and the landing blends back.
//!
//! The root tables are generated, not hand-made: `srp-026g/scripts/jump-arc/make_jump_arc.py` evaluates the stock clips
//! (`animations/single/weapons/ak/player/stand/3rd_view/jump/*.b-spline` of the stock `resources.db`; no row of
//! `release/content.tsv` names those clips) as clamped cubic B-splines and writes this file; its README names how to get them.

use mapdata::controller::Collision;
use mapdata::controller::tick::{Controller, Settings};
use mapdata::{NodeId, Vec3, WalkGrid};

use super::kin::{Local, TICK_S, to_world};
use crate::v026g::drive::key;

/// How far from a landing's feet its grid floor may be, m (`body`'s locate reach).
const LOCATE_M: f32 = 0.6;
/// The clip's frames a second.
const FPS: f32 = 30.0;
/// The clips' last frame (24 frames, 0.80 s at time scale 1).
pub const END_FRAME: f32 = 24.0;
/// The clip's `jump` event: the body leaves the ground (rifles; pistols at frame 3, whose root does not rise before frame 6 either).
pub const TAKEOFF_FRAME: f32 = 6.0;
/// The sprint jump clip's time scale (FINDINGS 3.3).
const SPRINT_TIME_SCALE: f32 = 0.909;
/// `air_control_speed` (player +0x304, `default.player`), m/s.
const AIR_CONTROL_MS: f32 = 1.0;
/// With no collision (no map): how far over the arc's feet a floor's top may be and still take it coming down, m.
pub const CATCH_M: f32 = 0.1;
/// The walk cap, m/s: the speed at a press for the sprint jump, and the fastest a body on the floor in a jump moves (the
/// sprint-jump RE 1.2, 3.2).
const WALK_CAP_MS: f32 = 3.5;
/// The sprint jump's prepare phase, its mean (the run's step left at the press: 0-150 ms, 0-1.64 m, FINDINGS 3.2): LIKELY.
const PREPARE_MS: f32 = 75.0;
const PREPARE_M: f32 = 0.82;
/// A body falling after its clip, m/s^2 (the bench stand-in's).
const GRAVITY: f32 = 9.81;

/// `get_move_direction` 0x2D6510: the keys' direction, forward and back cancelling, left and right cancelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    None,
    Forward,
    ForwardRight,
    Right,
    BackRight,
    Back,
    BackLeft,
    Left,
    ForwardLeft,
}

impl Direction {
    pub fn of(keys: u32) -> Self {
        let f = i8::from(keys & key::FORWARD != 0) - i8::from(keys & key::BACK != 0);
        let r = i8::from(keys & key::RIGHT != 0) - i8::from(keys & key::LEFT != 0);
        match (f, r) {
            (1, 0) => Self::Forward,
            (1, 1) => Self::ForwardRight,
            (0, 1) => Self::Right,
            (-1, 1) => Self::BackRight,
            (-1, 0) => Self::Back,
            (-1, -1) => Self::BackLeft,
            (0, -1) => Self::Left,
            (1, -1) => Self::ForwardLeft,
            _ => Self::None,
        }
    }

    /// The unit direction in the body's frame (ahead, to the left); zero for none.
    fn local(self) -> Local {
        let d = std::f32::consts::FRAC_1_SQRT_2;
        match self {
            Self::None => [0.0, 0.0],
            Self::Forward => [1.0, 0.0],
            Self::ForwardRight => [d, -d],
            Self::Right => [0.0, -1.0],
            Self::BackRight => [-d, -d],
            Self::Back => [-1.0, 0.0],
            Self::BackLeft => [-d, d],
            Self::Left => [0.0, 1.0],
            Self::ForwardLeft => [d, d],
        }
    }
}

/// The jump the client plays for a press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clip {
    /// A short jump (codes 1, 5-12) in the keys' direction.
    Short(Direction),
    /// A sprint jump (codes 2-4): forward, forward-right or forward-left.
    Sprint(Direction),
}

impl Clip {
    /// The jump of a press with `keys`, the body moving at `speed` m/s.
    pub fn of(keys: u32, speed: f32) -> Self {
        let direction = Direction::of(keys);
        let sprint = keys & (key::FORWARD | key::SPRINT) == (key::FORWARD | key::SPRINT)
            && keys & (key::BACK | key::FIRE | key::CROUCH | key::AIM) == 0
            && speed >= WALK_CAP_MS
            && matches!(direction, Direction::Forward | Direction::ForwardRight | Direction::ForwardLeft);
        if sprint { Self::Sprint(direction) } else { Self::Short(direction) }
    }

    fn table(self) -> &'static Root {
        match self {
            Self::Short(Direction::None) => &SHORT_NONE,
            Self::Short(Direction::Forward) => &SHORT_F,
            Self::Short(Direction::ForwardRight) => &SHORT_FR,
            Self::Short(Direction::Right) => &SHORT_R,
            Self::Short(Direction::BackRight) => &SHORT_BR,
            Self::Short(Direction::Back) => &SHORT_B,
            Self::Short(Direction::BackLeft) => &SHORT_BL,
            Self::Short(Direction::Left) => &SHORT_L,
            Self::Short(Direction::ForwardLeft) => &SHORT_FL,
            Self::Sprint(Direction::ForwardRight) => &SPRINT_FR,
            Self::Sprint(Direction::ForwardLeft) => &SPRINT_FL,
            Self::Sprint(_) => &SPRINT_F,
        }
    }

    fn time_scale(self) -> f32 {
        match self {
            Self::Short(_) => 1.0,
            Self::Sprint(_) => SPRINT_TIME_SCALE,
        }
    }

    /// The clip's root at `frame` (clamped to the clip): rise, ahead, to the right, m.
    fn root(self, frame: f32) -> [f32; 3] {
        let table = self.table();
        let f = frame.clamp(0.0, END_FRAME);
        let i = (f.floor() as usize).min(table.len() - 2);
        let t = f - i as f32;
        let (a, b) = (table[i], table[i + 1]);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    }

    /// The frame of the root's apex (16 for the short jumps, 14 for the sprint jumps): from it on, the body comes down.
    fn apex_frame(self) -> f32 {
        let table = self.table();
        (0..table.len()).max_by(|&a, &b| table[a][0].total_cmp(&table[b][0])).unwrap_or(0) as f32
    }

    /// The air control in the body's frame, m/s: the keys' direction at 1 m/s, none for forward alone in a sprint jump.
    fn air_control(self, keys: u32) -> Local {
        let direction = Direction::of(keys);
        if matches!(self, Self::Sprint(_)) && direction == Direction::Forward {
            return [0.0, 0.0];
        }
        let [a, l] = direction.local();
        [a * AIR_CONTROL_MS, l * AIR_CONTROL_MS]
    }
}

/// A jump in the reckoning: the clip's root from the feet at the press, at the press's facing (the keys pick the clip there;
/// after it only the air control steers); on the floor before the take-off at the walk cap at most; in the air after it the
/// client's air branch moves the body toward the root ([`Air`]); at the clip's end landed, or a fall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpArc {
    pressed: u32,
    clip: Clip,
    /// The feet and the yaw at the press: the clip's root is counted from them.
    from: Vec3,
    yaw: f32,
    /// The phase: on the floor, in the air over the floor it left, or falling after the clip.
    phase: Phase,
    /// With no collision: how far the air control has moved the body off the clip's way so far (map frame, m).
    air: [f32; 2],
}

/// Where a jump is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Before the take-off.
    Floor,
    /// In the air, the root's rise counted from `base` (the floor's height when the body left it), and the client's controller
    /// moving the body (none until the first tick in the air, or with no collision).
    Air { base: f32, body: Option<Air> },
    /// After the clip with no floor under the feet: falling at `speed` m/s.
    Fall { speed: f32 },
}

/// The client's controller of a body in a jump (`bullet_character_controller` in jump mode).
#[derive(Clone, Copy, Debug)]
pub struct Air(Controller);

/// Two controllers are the same body when their capsules stand in the same place (the estimate's equality).
impl PartialEq for Air {
    fn eq(&self, other: &Self) -> bool {
        self.0.origin == other.0.origin && self.0.velocity == other.0.velocity
    }
}

/// What a step of the arc did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArcStep {
    /// Before the take-off: the body walks this far on the ground (map frame, m).
    Ground([f32; 2]),
    /// In the air at the feet given.
    Air(Vec3),
    /// The jump is over: the body stands at the feet given, on the floor given (none: no floor anywhere under it).
    Landed(Vec3, Option<NodeId>),
}

impl JumpArc {
    /// A jump pressed at `pressed` with `keys`, the body moving at `speed`, its feet at `from` and its yaw `yaw`.
    pub fn pressed(pressed: u32, keys: u32, speed: f32, from: Vec3, yaw: f32) -> Self {
        Self { pressed, clip: Clip::of(keys, speed), from, yaw, phase: Phase::Floor, air: [0.0, 0.0] }
    }

    /// The jump a world at `time` finds under way, its feet at `feet` and its yaw `yaw`: the press's feet are the clip's root
    /// back from them, the floor it left the rise under them. None once the clip is over (a fall is the world's).
    pub fn seen(pressed: u32, keys: u32, speed: f32, time: u32, feet: Vec3, yaw: f32) -> Option<Self> {
        let mut arc = Self::pressed(pressed, keys, speed, feet, yaw);
        let ms = time.saturating_sub(pressed) as f32;
        let (frame, travel) = arc.root(ms);
        if frame >= END_FRAME {
            return None;
        }
        let rise = arc.clip.root(frame)[0];
        arc.from = Vec3::new(feet.x - travel[0], feet.y, feet.z - travel[1]);
        if frame > TAKEOFF_FRAME {
            arc.phase = Phase::Air { base: feet.y - rise, body: None };
        }
        Some(arc)
    }

    pub fn clip(&self) -> Clip {
        self.clip
    }

    /// The clip's frame `ms` after the press and the root's travel by then (map frame, from the press's feet): the sprint jump's
    /// prepare phase first (its mean, LIKELY: the client's is the run's step left at the press, 0-0.15 s and 0-1.64 m).
    fn root(&self, ms: f32) -> (f32, [f32; 2]) {
        let (pm, pd) = match self.clip {
            Clip::Sprint(_) => (PREPARE_MS, PREPARE_M),
            Clip::Short(_) => (0.0, 0.0),
        };
        let end = self.clip.root(END_FRAME);
        let len = end[1].hypot(end[2]);
        let unit = if len > 1e-6 { [end[1] / len, -end[2] / len] } else { [0.0, 0.0] };
        if ms < pm {
            let d = pd * ms / pm;
            return (0.0, to_world([unit[0] * d, unit[1] * d], self.yaw));
        }
        let frame = (ms - pm) / 1000.0 * FPS * self.clip.time_scale();
        let r = self.clip.root(frame);
        (frame, to_world([r[1] + unit[0] * pd, -r[2] + unit[1] * pd], self.yaw))
    }

    /// The body at `t1` (ms), from `feet` at the tick before, its yaw `yaw` and keys `keys` now: against the walker collision
    /// `world` (none: the grid's floors alone, its body's radius standing in for the capsule).
    pub fn step(&mut self, grid: &WalkGrid, world: Option<&Collision>, feet: Vec3, yaw: f32, keys: u32, t1: u32) -> ArcStep {
        let dt = TICK_S;
        if let Phase::Fall { speed } = self.phase {
            let speed = speed + GRAVITY * dt;
            let y = feet.y - speed * dt;
            return match air_floor(grid, feet, feet.y + CATCH_M) {
                Some((node, on)) if on.y >= y => ArcStep::Landed(on, Some(node)),
                Some(_) => {
                    self.phase = Phase::Fall { speed };
                    ArcStep::Air(Vec3::new(feet.x, y, feet.z))
                }
                None => ArcStep::Landed(feet, None),
            };
        }
        let ms = t1.saturating_sub(self.pressed) as f32;
        let (frame, travel) = self.root(ms);
        let target = (self.from.x + travel[0], self.from.z + travel[1]);
        if frame <= TAKEOFF_FRAME {
            // On the floor: toward the root at the walk cap at most.
            let (dx, dz) = (target.0 - feet.x, target.1 - feet.z);
            let len = dx.hypot(dz);
            let k = if len > 1e-6 { (WALK_CAP_MS * dt / len).min(1.0) } else { 0.0 };
            return ArcStep::Ground([dx * k, dz * k]);
        }
        let (base, body) = match self.phase {
            Phase::Air { base, body } => (base, body),
            _ => (feet.y, None),
        };
        let y = base + self.clip.root(frame.min(END_FRAME))[0];
        let over = frame >= END_FRAME;
        let [ca, cl] = self.clip.air_control(keys);
        let Some(world) = world else {
            return self.fly_over_floors(grid, feet, target, y, frame, [ca, cl], yaw, base);
        };
        // The client's air branch: the walk vector is the root target less the feet (all three axes), the air control the keys'.
        let mut air = body.unwrap_or_else(|| {
            let mut c = Controller::standing_at(feet);
            c.jumping = true;
            Air(c)
        });
        let at = air.0.feet();
        air.0.walk = Vec3::new(target.0 - at.x, y - at.y, target.1 - at.z);
        let [sx, sz] = to_world([ca, cl], yaw);
        air.0.air_control = Vec3::new(sx, 0.0, sz);
        air.0.step(world, dt, &Settings::LIVE);
        let at = air.0.feet();
        if over {
            // `end_jump`: standing, the jump is over; else the body falls.
            air.0.end_jump();
            if air.0.on_ground(&Settings::LIVE) {
                return ArcStep::Landed(at, grid.locate(at, LOCATE_M));
            }
            self.phase = Phase::Fall { speed: 0.0 };
            return ArcStep::Air(at);
        }
        self.phase = Phase::Air { base, body: Some(air) };
        ArcStep::Air(at)
    }

    /// With no collision: the root over the grid's floors, the air control added, down on the highest floor under the feet from the
    /// apex on (`tests/standin_jump.rs` `air_floor`), else a fall after the clip.
    #[allow(clippy::too_many_arguments)]
    fn fly_over_floors(
        &mut self,
        grid: &WalkGrid,
        feet: Vec3,
        target: (f32, f32),
        y: f32,
        frame: f32,
        control: Local,
        yaw: f32,
        base: f32,
    ) -> ArcStep {
        let steer = to_world([control[0] * TICK_S, control[1] * TICK_S], yaw);
        self.air = [self.air[0] + steer[0], self.air[1] + steer[1]];
        let at = Vec3::new(target.0 + self.air[0], y, target.1 + self.air[1]);
        let over = frame >= END_FRAME;
        if (frame >= self.clip.apex_frame() || over)
            && let Some((node, on)) = air_floor(grid, at, feet.y.max(y) + CATCH_M)
            && on.y >= y - 1e-4
        {
            return ArcStep::Landed(on, Some(node));
        }
        self.phase = if over { Phase::Fall { speed: 0.0 } } else { Phase::Air { base, body: None } };
        ArcStep::Air(at)
    }
}

/// The floor a body in the air at `at` comes down on (`tests/standin_jump.rs` `air_floor`): the highest whose cell holds it at or
/// below `top`, else the highest within the body's radius (the nearest of them); its floor and where the feet stand.
fn air_floor(grid: &WalkGrid, at: Vec3, top: f32) -> Option<(NodeId, Vec3)> {
    let half = grid.cell_size() * 0.5 + 1e-4;
    let box_of = |r: f32| [at.x - r, at.z - r, at.x + r, at.z + r];
    let under = grid
        .floors_in(box_of(half))
        .filter(|&n| {
            let p = grid.position(n);
            (p.x - at.x).abs() <= half && (p.z - at.z).abs() <= half && p.y <= top
        })
        .max_by(|&a, &b| grid.position(a).y.total_cmp(&grid.position(b).y));
    if let Some(n) = under {
        return Some((n, Vec3::new(at.x, grid.position(n).y, at.z)));
    }
    let r = grid.body().radius;
    grid.floors_in(box_of(r))
        .filter(|&n| {
            let p = grid.position(n);
            p.distance_xz(at) <= r && p.y <= top
        })
        .max_by(|&a, &b| {
            let (pa, pb) = (grid.position(a), grid.position(b));
            pa.y.total_cmp(&pb.y).then(pb.distance_xz(at).total_cmp(&pa.distance_xz(at)))
        })
        .map(|n| (n, grid.position(n)))
}

/// A clip's root per frame (0..=24): rise, ahead, to the right (m), from the stock clips (`animations/single/weapons/ak/player/
/// stand/3rd_view/jump/`, evaluated as clamped cubic B-splines by `srp-026g/scripts/jump-arc/make_jump_arc.py`).
type Root = [[f32; 3]; 25];

'''

TESTS = r'''
#[cfg(test)]
mod tests {
    use super::*;

    /// A box (corners `lo`, `hi`) as the walker collision's triangles: its top and four sides.
    fn block(lo: Vec3, hi: Vec3) -> Vec<mapdata::Triangle> {
        let q = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
            [a, b, c, a, c, d].chunks(3).map(|t| mapdata::Triangle { v: [t[0], t[1], t[2]], walker: Default::default() }).collect::<Vec<_>>()
        };
        let (x0, y0, z0, x1, y1, z1) = (lo.x, lo.y, lo.z, hi.x, hi.y, hi.z);
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        [
            q(v(x0, y1, z0), v(x1, y1, z0), v(x1, y1, z1), v(x0, y1, z1)),
            q(v(x0, y0, z0), v(x1, y0, z0), v(x1, y1, z0), v(x0, y1, z0)),
            q(v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)),
            q(v(x0, y0, z0), v(x0, y0, z1), v(x0, y1, z1), v(x0, y1, z0)),
            q(v(x1, y0, z0), v(x1, y0, z1), v(x1, y1, z1), v(x1, y1, z0)),
        ]
        .concat()
    }

    /// A railing's top rail (4 cm wide, 7 cm thick, its top 1.05 m over the floor) 0.6 m ahead of a body standing still: the
    /// forward jump's capsule comes down on it and stands there at the clip's end, as the client perches on Vector Laboratory's.
    #[test]
    fn a_short_jump_from_against_a_rail_ends_on_the_rail() {
        use crate::v026g::bots::body::tests::floor;
        let mut tris = floor(-10.0, -10.0, 10.0, 10.0, 0.0).to_vec();
        tris.extend(block(Vec3::new(-5.0, 0.98, 0.6), Vec3::new(5.0, 1.05, 0.64)));
        let world = Collision::build(&tris);
        let grid = WalkGrid::build(&tris, [-10.0, -10.0, 10.0, 10.0], 0.25, mapdata::Body::client());
        let arc = JumpArc::pressed(0, key::FORWARD, 0.0, Vec3::ZERO, 0.0);
        let (at, _, t) = step_all(arc, &grid, Some(&world), Vec3::ZERO, key::FORWARD);
        assert!(at.y > 0.85 && at.y < 1.1, "on the rail: {at:?} at {t} ms");
        assert!((at.z - 0.62).abs() < 0.4, "beside its line: {at:?}");
    }

    /// The keys pick the client's jump: forward with sprint at the walk cap is the sprint jump; slower, or with fire, the short one.
    #[test]
    fn the_keys_and_the_speed_pick_the_jump() {
        let run = key::FORWARD | key::SPRINT;
        assert_eq!(Clip::of(run, 4.9), Clip::Sprint(Direction::Forward));
        assert_eq!(Clip::of(run, 3.0), Clip::Short(Direction::Forward));
        assert_eq!(Clip::of(run | key::FIRE, 4.9), Clip::Short(Direction::Forward));
        assert_eq!(Clip::of(run | key::RIGHT, 4.9), Clip::Sprint(Direction::ForwardRight));
        assert_eq!(Clip::of(key::BACK | key::LEFT, 0.0), Clip::Short(Direction::BackLeft));
        assert_eq!(Clip::of(key::FORWARD | key::BACK, 0.0), Clip::Short(Direction::None));
    }

    /// The short forward jump's root: up from frame 6, 0.998 m at frame 16, down at the end, 2.67 m ahead; the sprint jump's 1.282 m
    /// and 4.42 m over 0.88 s.
    #[test]
    fn the_clips_rise_and_travel_as_the_stock_clips() {
        let short = Clip::Short(Direction::Forward);
        assert!(short.root(6.0)[0].abs() < 1e-3);
        assert!((short.root(16.0)[0] - 0.998).abs() < 1e-3);
        assert!(short.root(24.0)[0].abs() < 1e-3);
        assert!((short.root(24.0)[1] - 2.67).abs() < 0.01);
        let sprint = Clip::Sprint(Direction::Forward);
        assert!((sprint.root(14.0)[0] - 1.282).abs() < 1e-3);
        assert!((sprint.root(24.0)[1] - 4.42).abs() < 0.01);
        let arc = JumpArc::pressed(0, key::FORWARD | key::SPRINT, 5.0, Vec3::ZERO, 0.0);
        let (frame, travel) = arc.root(880.0 + PREPARE_MS);
        assert!((frame - 24.0).abs() < 0.05, "{frame}");
        assert!((travel[1] - (4.42 + PREPARE_M)).abs() < 0.02, "{travel:?}");
        assert_eq!((short.apex_frame(), sprint.apex_frame()), (16.0, 14.0));
        assert!(Clip::Short(Direction::Right).root(24.0)[2] > 1.9, "right is +x");
    }

    /// The air control: the keys at 1 m/s, none for forward alone in a sprint jump.
    #[test]
    fn the_air_control_is_the_keys_at_one_metre_a_second() {
        assert_eq!(Clip::Short(Direction::Forward).air_control(key::FORWARD), [1.0, 0.0]);
        assert_eq!(Clip::Sprint(Direction::Forward).air_control(key::FORWARD), [0.0, 0.0]);
        assert_eq!(Clip::Sprint(Direction::Forward).air_control(key::LEFT), [0.0, 1.0]);
    }

    fn step_all(mut arc: JumpArc, grid: &WalkGrid, world: Option<&Collision>, mut feet: Vec3, keys: u32) -> (Vec3, Option<NodeId>, u32) {
        let mut t = 0;
        loop {
            match arc.step(grid, world, feet, 0.0, keys, t + 50) {
                ArcStep::Ground([dx, dz]) => feet = Vec3::new(feet.x + dx, feet.y, feet.z + dz),
                ArcStep::Air(at) => feet = at,
                ArcStep::Landed(at, node) => return (at, node, t + 50),
            }
            t += 50;
            assert!(t < 2000, "the jump never ends");
        }
    }

    /// On flat ground the short forward jump goes 2.67 m and the 1 m/s control on top over its 0.6 s in the air, and lands at 0.8 s.
    #[test]
    fn a_short_jump_on_flat_ground_lands_ahead_at_the_clips_end() {
        let grid = WalkGrid::build(&crate::v026g::bots::body::tests::floor(-10.0, -10.0, 10.0, 10.0, 0.0), [-10.0, -10.0, 10.0, 10.0], 0.25, mapdata::Body::client());
        let arc = JumpArc::pressed(0, key::FORWARD, 3.0, Vec3::ZERO, 0.0);
        let (at, node, t) = step_all(arc, &grid, None, Vec3::ZERO, key::FORWARD);
        assert!(node.is_some() && at.y.abs() < 1e-3, "{at:?}");
        assert!((800..=850).contains(&t), "{t}");
        let ahead = at.x.hypot(at.z);
        assert!((ahead - (2.67 + 0.6)).abs() < 0.15, "{ahead}");
    }

    /// A ledge 0.9 m up 1.5 m ahead takes the short jump's body (its root is 0.9 m+ up from 1.17 m to 2.2 m); one 1.3 m up stops it
    /// at its face: the client's capsule against the ledge as a block.
    #[test]
    fn a_short_jump_lands_on_a_ledge_under_its_rise_and_is_stopped_by_a_higher_one() {
        use crate::v026g::bots::body::tests::floor;
        // Yaw 0 faces +z (`kin::forward`): the ledge's face is across z at 1.5 m.
        assert!(to_world([1.0, 0.0], 0.0)[1] > 0.99);
        for (rise, up) in [(0.9, true), (1.3, false)] {
            let mut tris = floor(-10.0, -10.0, 10.0, 1.5, 0.0).to_vec();
            tris.extend(block(Vec3::new(-10.0, 0.0, 1.5), Vec3::new(10.0, rise, 10.0)));
            let world = Collision::build(&tris);
            let grid = WalkGrid::build(&tris, [-10.0, -10.0, 10.0, 10.0], 0.25, mapdata::Body::client());
            let arc = JumpArc::pressed(0, key::FORWARD, 3.0, Vec3::ZERO, 0.0);
            let (at, _, _) = step_all(arc, &grid, Some(&world), Vec3::ZERO, key::FORWARD);
            assert_eq!(at.y > rise - 0.05, up, "rise {rise}: {at:?}");
            if !up {
                assert!(at.z < 1.5, "the face stopped it: {at:?}");
            }
        }
    }
}
'''


def main():
    anim, out = sys.argv[1], sys.argv[2]
    parts = [CODE]
    for const, clip in TABLES:
        rows = table(anim, clip)
        parts.append('/// `%s`.\n' % clip.replace('sprint__', 'sprint/'))
        parts.append("#[allow(clippy::approx_constant)] // the clip's numbers, not the constants\n")
        parts.append('const %s: Root = [\n' % const)
        for r in rows:
            parts.append('    %s,\n' % r)
        parts.append('];\n\n')
    text = ''.join(parts).rstrip('\n') + '\n' + TESTS
    with open(out, 'w', encoding='utf-8', newline='') as f:
        f.write(text)
    print('wrote', out, len(text))


if __name__ == '__main__':
    main()
