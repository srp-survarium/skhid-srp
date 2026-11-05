//! [A318] The client's jump for the reckoned body ([`super::kin::Kinematics::jump_arc`]): the arc the jump clip's root draws,
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
//! The reckoning has no collision world at run time, only the walk grid, so the grid's floors stand in for the sweeps: a floor
//! within the body's radius above the arc's feet by more than [`CATCH_M`] and under its head is a wall; the arc lands on the
//! highest floor within the radius that its feet come down through, or, at the clip's end, on the highest one under its feet.
//! LIKELY: the sprint jump's leg (the right leg's clip rises 1.350 m, the left's 1.282 m: the left's is taken), the short jump's
//! time scale 1, and the 0.1 m catch (the capsule's lower sphere on a lip).

use mapdata::{NodeId, Vec3, WalkGrid};

use super::kin::{Local, to_world};
use crate::v026g::drive::key;

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
/// How far over the arc's feet a floor's top may be and still take the body (the capsule's lower sphere on a lip), m.
pub const CATCH_M: f32 = 0.1;
/// The walk cap, m/s: the speed at a press for the sprint jump (the sprint-jump RE 1.2).
const WALK_CAP_MS: f32 = 3.5;

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

/// A jump in the reckoning: pressed at `pressed` (ms), the clip, and the height of the feet the root's rise is counted from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpArc {
    pressed: u32,
    clip: Clip,
    base_y: f32,
}

/// What a step of the arc did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArcStep {
    /// Before the take-off: the body walks the root's travel on the ground (map frame, m).
    Ground([f32; 2]),
    /// In the air at `feet`.
    Air(Vec3),
    /// The jump is over: the body stands at `feet` on `node` (none: no floor under it, a fall the next world puts right).
    Landed(Vec3, Option<NodeId>),
}

impl JumpArc {
    /// A jump pressed at `pressed` with `keys`, the body moving at `speed`, its feet at height `y`.
    pub fn pressed(pressed: u32, keys: u32, speed: f32, y: f32) -> Self {
        Self { pressed, clip: Clip::of(keys, speed), base_y: y }
    }

    /// The jump a world finds in the air at `time` with its feet at `y`: the rise the clip has by then is under them.
    pub fn seen(pressed: u32, keys: u32, speed: f32, time: u32, y: f32) -> Option<Self> {
        let mut arc = Self::pressed(pressed, keys, speed, y);
        let frame = arc.frame(time);
        if frame >= END_FRAME {
            return None;
        }
        arc.base_y = y - arc.clip.root(frame)[0];
        Some(arc)
    }

    pub fn clip(&self) -> Clip {
        self.clip
    }

    /// The clip's frame at `time`.
    pub fn frame(&self, time: u32) -> f32 {
        time.saturating_sub(self.pressed) as f32 / 1000.0 * FPS * self.clip.time_scale()
    }

    /// The body from `t0` to `t1` (ms) of the jump, its feet at `feet` and its yaw `yaw`, holding `keys`, over `grid` (its body's
    /// radius and height).
    pub fn step(&self, grid: &WalkGrid, feet: Vec3, yaw: f32, keys: u32, t0: u32, t1: u32) -> ArcStep {
        let (f0, f1) = (self.frame(t0), self.frame(t1));
        let (r0, r1) = (self.clip.root(f0), self.clip.root(f1));
        let travel = to_world([r1[1] - r0[1], -(r1[2] - r0[2])], yaw);
        if f1 <= TAKEOFF_FRAME {
            return ArcStep::Ground(travel);
        }
        let dt = t1.saturating_sub(t0) as f32 / 1000.0;
        let [ca, cl] = self.clip.air_control(keys);
        let control = to_world([ca * dt, cl * dt], yaw);
        let y = self.base_y + r1[0];
        let body = grid.body();
        let (radius, height) = (body.radius, body.stand_height);
        // Over the body's ground box at `at`: a floor top above the feet by more than the catch and under the head is a wall.
        let blocked = |x: f32, z: f32| {
            grid.floors_in([x - radius, z - radius, x + radius, z + radius]).any(|n| {
                let top = grid.position(n).y;
                top > y + CATCH_M && top < y + height
            })
        };
        let (dx, dz) = (travel[0] + control[0], travel[1] + control[1]);
        // `step_jump_forward`: the whole move, else the wall's side (each axis alone), else none.
        let (x, z) = [(feet.x + dx, feet.z + dz), (feet.x + dx, feet.z), (feet.x, feet.z + dz)]
            .into_iter()
            .find(|&(x, z)| !blocked(x, z))
            .unwrap_or((feet.x, feet.z));
        let under = |below: f32, above: f32| {
            grid.floors_in([x - radius, z - radius, x + radius, z + radius])
                .map(|n| (n, grid.position(n).y))
                .filter(|&(_, top)| top >= below && top <= above)
                .max_by(|a, b| a.1.total_cmp(&b.1))
        };
        if f1 >= END_FRAME {
            // `end_jump`: the clip is over; the body stands on the highest floor under it.
            return match under(f32::NEG_INFINITY, y + CATCH_M) {
                Some((node, top)) => ArcStep::Landed(Vec3::new(x, top, z), Some(node)),
                None => ArcStep::Landed(Vec3::new(x, y, z), None),
            };
        }
        // `step_jump_down`: coming down through a floor's top sets the body on it.
        if r1[0] < r0[0]
            && let Some((node, top)) = under(y - 1e-3, feet.y + CATCH_M)
        {
            return ArcStep::Landed(Vec3::new(x, top, z), Some(node));
        }
        ArcStep::Air(Vec3::new(x, y, z))
    }
}

/// A clip's root per frame (0..=24): rise, ahead, to the right (m), from the stock clips (`animations/single/weapons/ak/player/
/// stand/3rd_view/jump/`, evaluated as clamped cubic B-splines by `traces-20261028/fx-team-a318/scripts/make_jump_arc.py`).
type Root = [[f32; 3]; 25];

/// `on_site`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_NONE: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.1881, 0.0, 0.0],
    [0.3569, 0.0, 0.0],
    [0.5063, 0.0, 0.0],
    [0.6360, 0.0, 0.0],
    [0.7462, 0.0, 0.0],
    [0.8365, 0.0, 0.0],
    [0.9070, 0.0, 0.0],
    [0.9575, 0.0, 0.0],
    [0.9879, 0.0, 0.0],
    [0.9980, 0.0, 0.0],
    [0.9845, 0.0, 0.0],
    [0.9429, 0.0, 0.0],
    [0.8712, 0.0, 0.0],
    [0.7678, 0.0, 0.0],
    [0.6308, 0.0, 0.0],
    [0.4583, 0.0, 0.0],
    [0.2487, 0.0, 0.0],
    [0.0, 0.0, 0.0],
];

/// `on_site_fwd`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_F: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.1111, 0.0],
    [0.0, 0.2222, 0.0],
    [0.0, 0.3333, 0.0],
    [0.0, 0.4444, 0.0],
    [0.0, 0.5556, 0.0],
    [0.0, 0.6667, 0.0],
    [0.1881, 0.7778, 0.0],
    [0.3569, 0.8889, 0.0],
    [0.5063, 1.0000, 0.0],
    [0.6360, 1.1111, 0.0],
    [0.7462, 1.2222, 0.0],
    [0.8365, 1.3333, 0.0],
    [0.9070, 1.4444, 0.0],
    [0.9575, 1.5556, 0.0],
    [0.9879, 1.6667, 0.0],
    [0.9980, 1.7778, 0.0],
    [0.9845, 1.8889, 0.0],
    [0.9429, 2.0000, 0.0],
    [0.8712, 2.1111, 0.0],
    [0.7678, 2.2222, 0.0],
    [0.6308, 2.3333, 0.0],
    [0.4583, 2.4444, 0.0],
    [0.2487, 2.5556, 0.0],
    [0.0, 2.6667, 0.0],
];

/// `on_site_fwd_right`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_FR: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0786, 0.0786],
    [0.0, 0.1571, 0.1571],
    [0.0, 0.2357, 0.2357],
    [0.0, 0.3143, 0.3143],
    [0.0, 0.3928, 0.3928],
    [0.0, 0.4714, 0.4714],
    [0.1881, 0.5500, 0.5500],
    [0.3569, 0.6285, 0.6285],
    [0.5063, 0.7071, 0.7071],
    [0.6360, 0.7857, 0.7857],
    [0.7462, 0.8642, 0.8642],
    [0.8365, 0.9428, 0.9428],
    [0.9070, 1.0214, 1.0214],
    [0.9575, 1.0999, 1.0999],
    [0.9879, 1.1785, 1.1785],
    [0.9980, 1.2571, 1.2571],
    [0.9845, 1.3356, 1.3356],
    [0.9429, 1.4142, 1.4142],
    [0.8712, 1.4928, 1.4928],
    [0.7678, 1.5713, 1.5713],
    [0.6308, 1.6499, 1.6499],
    [0.4583, 1.7285, 1.7285],
    [0.2487, 1.8071, 1.8071],
    [0.0, 1.8856, 1.8856],
];

/// `on_site_right`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_R: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0833],
    [0.0, 0.0, 0.1667],
    [0.0, 0.0, 0.2500],
    [0.0, 0.0, 0.3333],
    [0.0, 0.0, 0.4167],
    [0.0, 0.0, 0.5000],
    [0.1881, 0.0, 0.5833],
    [0.3569, 0.0, 0.6667],
    [0.5063, 0.0, 0.7500],
    [0.6360, 0.0, 0.8333],
    [0.7462, 0.0, 0.9167],
    [0.8365, 0.0, 1.0000],
    [0.9070, 0.0, 1.0833],
    [0.9575, 0.0, 1.1667],
    [0.9879, 0.0, 1.2500],
    [0.9980, 0.0, 1.3333],
    [0.9845, 0.0, 1.4167],
    [0.9429, 0.0, 1.5000],
    [0.8712, 0.0, 1.5833],
    [0.7678, 0.0, 1.6667],
    [0.6308, 0.0, 1.7500],
    [0.4583, 0.0, 1.8333],
    [0.2487, 0.0, 1.9167],
    [0.0, 0.0, 2.0000],
];

/// `on_site_back_right`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_BR: Root = [
    [0.0, 0.0, 0.0],
    [0.0, -0.0786, 0.0786],
    [0.0, -0.1571, 0.1571],
    [0.0, -0.2357, 0.2357],
    [0.0, -0.3143, 0.3143],
    [0.0, -0.3928, 0.3928],
    [0.0, -0.4714, 0.4714],
    [0.1881, -0.5500, 0.5500],
    [0.3569, -0.6285, 0.6285],
    [0.5063, -0.7071, 0.7071],
    [0.6360, -0.7857, 0.7857],
    [0.7462, -0.8642, 0.8642],
    [0.8365, -0.9428, 0.9428],
    [0.9070, -1.0214, 1.0214],
    [0.9575, -1.0999, 1.0999],
    [0.9879, -1.1785, 1.1785],
    [0.9980, -1.2571, 1.2571],
    [0.9845, -1.3356, 1.3356],
    [0.9429, -1.4142, 1.4142],
    [0.8712, -1.4928, 1.4928],
    [0.7678, -1.5713, 1.5713],
    [0.6308, -1.6499, 1.6499],
    [0.4583, -1.7285, 1.7285],
    [0.2487, -1.8071, 1.8071],
    [0.0, -1.8856, 1.8856],
];

/// `on_site_back`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_B: Root = [
    [0.0, 0.0, 0.0],
    [0.0, -0.1111, 0.0],
    [0.0, -0.2222, 0.0],
    [0.0, -0.3333, 0.0],
    [0.0, -0.4444, 0.0],
    [0.0, -0.5556, 0.0],
    [0.0, -0.6667, 0.0],
    [0.1881, -0.7778, 0.0],
    [0.3569, -0.8889, 0.0],
    [0.5063, -1.0000, 0.0],
    [0.6360, -1.1111, 0.0],
    [0.7462, -1.2222, 0.0],
    [0.8365, -1.3333, 0.0],
    [0.9070, -1.4444, 0.0],
    [0.9575, -1.5556, 0.0],
    [0.9879, -1.6667, 0.0],
    [0.9980, -1.7778, 0.0],
    [0.9845, -1.8889, 0.0],
    [0.9429, -2.0000, 0.0],
    [0.8712, -2.1111, 0.0],
    [0.7678, -2.2222, 0.0],
    [0.6308, -2.3333, 0.0],
    [0.4583, -2.4444, 0.0],
    [0.2487, -2.5556, 0.0],
    [0.0, -2.6667, 0.0],
];

/// `on_site_back_left`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_BL: Root = [
    [0.0, 0.0, 0.0],
    [0.0, -0.0786, -0.0786],
    [0.0, -0.1571, -0.1571],
    [0.0, -0.2357, -0.2357],
    [0.0, -0.3143, -0.3143],
    [0.0, -0.3928, -0.3928],
    [0.0, -0.4714, -0.4714],
    [0.1881, -0.5500, -0.5500],
    [0.3569, -0.6285, -0.6285],
    [0.5063, -0.7071, -0.7071],
    [0.6360, -0.7857, -0.7857],
    [0.7462, -0.8642, -0.8642],
    [0.8365, -0.9428, -0.9428],
    [0.9070, -1.0214, -1.0214],
    [0.9575, -1.0999, -1.0999],
    [0.9879, -1.1785, -1.1785],
    [0.9980, -1.2571, -1.2571],
    [0.9845, -1.3356, -1.3356],
    [0.9429, -1.4142, -1.4142],
    [0.8712, -1.4928, -1.4928],
    [0.7678, -1.5713, -1.5713],
    [0.6308, -1.6499, -1.6499],
    [0.4583, -1.7285, -1.7285],
    [0.2487, -1.8071, -1.8071],
    [0.0, -1.8856, -1.8856],
];

/// `on_site_left`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_L: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0, -0.0068],
    [0.0, 0.0, -0.0267],
    [0.0, 0.0, -0.0590],
    [0.0, 0.0, -0.1029],
    [0.0, 0.0, -0.1575],
    [0.0, 0.0, -0.2222],
    [0.1881, 0.0, -0.2962],
    [0.3569, 0.0, -0.3786],
    [0.5063, 0.0, -0.4688],
    [0.6360, 0.0, -0.5658],
    [0.7462, 0.0, -0.6691],
    [0.8365, 0.0, -0.7778],
    [0.9070, 0.0, -0.8911],
    [0.9575, 0.0, -1.0082],
    [0.9879, 0.0, -1.1285],
    [0.9980, 0.0, -1.2510],
    [0.9845, 0.0, -1.3751],
    [0.9429, 0.0, -1.5000],
    [0.8712, 0.0, -1.6249],
    [0.7678, 0.0, -1.7490],
    [0.6308, 0.0, -1.8715],
    [0.4583, 0.0, -1.9918],
    [0.2487, 0.0, -2.1089],
    [0.0, 0.0, -2.2222],
];

/// `on_site_fwd_left`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SHORT_FL: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0786, -0.0786],
    [0.0, 0.1571, -0.1571],
    [0.0, 0.2357, -0.2357],
    [0.0, 0.3143, -0.3143],
    [0.0, 0.3928, -0.3928],
    [0.0, 0.4714, -0.4714],
    [0.1881, 0.5500, -0.5500],
    [0.3569, 0.6285, -0.6285],
    [0.5063, 0.7071, -0.7071],
    [0.6360, 0.7857, -0.7857],
    [0.7462, 0.8642, -0.8642],
    [0.8365, 0.9428, -0.9428],
    [0.9070, 1.0214, -1.0214],
    [0.9575, 1.0999, -1.0999],
    [0.9879, 1.1785, -1.1785],
    [0.9980, 1.2571, -1.2571],
    [0.9845, 1.3356, -1.3356],
    [0.9429, 1.4142, -1.4142],
    [0.8712, 1.4928, -1.4928],
    [0.7678, 1.5713, -1.5713],
    [0.6308, 1.6499, -1.6499],
    [0.4583, 1.7285, -1.7285],
    [0.2487, 1.8071, -1.8071],
    [0.0, 1.8856, -1.8856],
];

/// `sprint/sprint_from_left_leg`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SPRINT_F: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.1909, 0.0],
    [0.0, 0.3817, 0.0],
    [0.0, 0.5725, 0.0],
    [0.0, 0.7634, 0.0],
    [0.0, 0.9546, 0.0],
    [0.0, 1.1462, 0.0],
    [0.2701, 1.3402, 0.0],
    [0.5716, 1.5364, 0.0],
    [0.8574, 1.7323, 0.0],
    [1.0805, 1.9246, 0.0],
    [1.1938, 2.1105, 0.0],
    [1.2350, 2.2917, 0.0],
    [1.2672, 2.4717, 0.0],
    [1.2820, 2.6508, 0.0],
    [1.2709, 2.8290, 0.0],
    [1.2254, 3.0065, 0.0],
    [1.1370, 3.1835, 0.0],
    [1.0112, 3.3600, 0.0],
    [0.8630, 3.5363, 0.0],
    [0.6985, 3.7124, 0.0],
    [0.5239, 3.8886, 0.0],
    [0.3452, 4.0650, 0.0],
    [0.1685, 4.2416, 0.0],
    [0.0, 4.4187, 0.0],
];

/// `sprint/sprint_right_from_left_leg`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SPRINT_FR: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.1350, 0.1350],
    [0.0, 0.2699, 0.2699],
    [0.0, 0.4048, 0.4048],
    [0.0, 0.5398, 0.5398],
    [0.0, 0.6750, 0.6750],
    [0.0, 0.8105, 0.8105],
    [0.2701, 0.9476, 0.9476],
    [0.5716, 1.0864, 1.0864],
    [0.8574, 1.2249, 1.2249],
    [1.0805, 1.3609, 1.3609],
    [1.1938, 1.4924, 1.4924],
    [1.2350, 1.6205, 1.6205],
    [1.2672, 1.7478, 1.7478],
    [1.2820, 1.8744, 1.8744],
    [1.2709, 2.0004, 2.0004],
    [1.2254, 2.1259, 2.1259],
    [1.1370, 2.2510, 2.2510],
    [1.0112, 2.3759, 2.3759],
    [0.8630, 2.5005, 2.5005],
    [0.6985, 2.6251, 2.6251],
    [0.5239, 2.7497, 2.7497],
    [0.3452, 2.8744, 2.8744],
    [0.1685, 2.9993, 2.9993],
    [0.0, 3.1245, 3.1245],
];

/// `sprint/sprint_left_from_left_leg`.
#[allow(clippy::approx_constant)] // the clip's numbers, not the constants
const SPRINT_FL: Root = [
    [0.0, 0.0, 0.0],
    [0.0, 0.1350, -0.1350],
    [0.0, 0.2699, -0.2699],
    [0.0, 0.4048, -0.4048],
    [0.0, 0.5398, -0.5398],
    [0.0, 0.6750, -0.6750],
    [0.0, 0.8105, -0.8105],
    [0.2701, 0.9476, -0.9476],
    [0.5716, 1.0864, -1.0864],
    [0.8574, 1.2249, -1.2249],
    [1.0805, 1.3609, -1.3609],
    [1.1938, 1.4924, -1.4924],
    [1.2350, 1.6205, -1.6205],
    [1.2672, 1.7478, -1.7478],
    [1.2820, 1.8744, -1.8744],
    [1.2709, 2.0004, -2.0004],
    [1.2254, 2.1259, -2.1259],
    [1.1370, 2.2510, -2.2510],
    [1.0112, 2.3759, -2.3759],
    [0.8630, 2.5005, -2.5005],
    [0.6985, 2.6251, -2.6251],
    [0.5239, 2.7497, -2.7497],
    [0.3452, 2.8744, -2.8744],
    [0.1685, 2.9993, -2.9993],
    [0.0, 3.1245, -3.1245],
];

#[cfg(test)]
mod tests {
    use super::*;

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
        let arc = JumpArc::pressed(0, key::FORWARD | key::SPRINT, 5.0, 0.0);
        assert!((arc.frame(880) - 24.0).abs() < 0.05, "{}", arc.frame(880));
        assert!(Clip::Short(Direction::Right).root(24.0)[2] > 1.9, "right is +x");
    }

    /// The air control: the keys at 1 m/s, none for forward alone in a sprint jump.
    #[test]
    fn the_air_control_is_the_keys_at_one_metre_a_second() {
        assert_eq!(Clip::Short(Direction::Forward).air_control(key::FORWARD), [1.0, 0.0]);
        assert_eq!(Clip::Sprint(Direction::Forward).air_control(key::FORWARD), [0.0, 0.0]);
        assert_eq!(Clip::Sprint(Direction::Forward).air_control(key::LEFT), [0.0, 1.0]);
    }

    fn step_all(arc: &JumpArc, grid: &WalkGrid, mut feet: Vec3, keys: u32) -> (Vec3, Option<NodeId>, u32) {
        let mut t = 0;
        loop {
            match arc.step(grid, feet, 0.0, keys, t, t + 50) {
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
        let arc = JumpArc::pressed(0, key::FORWARD, 3.0, 0.0);
        let (at, node, t) = step_all(&arc, &grid, Vec3::ZERO, key::FORWARD);
        assert!(node.is_some() && at.y.abs() < 1e-3, "{at:?}");
        assert!((800..=850).contains(&t), "{t}");
        let ahead = at.x.hypot(at.z);
        assert!((ahead - (2.67 + 0.6)).abs() < 0.15, "{ahead}");
    }

    /// A ledge 0.9 m up 1.5 m ahead takes the short jump's body (its root is 0.9 m+ up from 1.17 m to 2.2 m); one 1.3 m up stops it.
    #[test]
    fn a_short_jump_lands_on_a_ledge_under_its_rise_and_is_stopped_by_a_higher_one() {
        use crate::v026g::bots::body::tests::floor;
        // Yaw 0 faces +z (`kin::forward`): the ledge's face is across z at 1.5 m.
        assert!(to_world([1.0, 0.0], 0.0)[1] > 0.99);
        for (rise, up) in [(0.9, true), (1.3, false)] {
            let mut tris = floor(-10.0, -10.0, 10.0, 1.5, 0.0).to_vec();
            tris.extend(floor(-10.0, 1.5, 10.0, 10.0, rise));
            let grid = WalkGrid::build(&tris, [-10.0, -10.0, 10.0, 10.0], 0.25, mapdata::Body::client());
            let arc = JumpArc::pressed(0, key::FORWARD, 3.0, 0.0);
            let (at, _, _) = step_all(&arc, &grid, Vec3::ZERO, key::FORWARD);
            assert_eq!(at.y > rise - 0.05, up, "rise {rise}: {at:?}");
            if !up {
                assert!(at.z < 1.5, "the face stopped it: {at:?}");
            }
        }
    }
}
