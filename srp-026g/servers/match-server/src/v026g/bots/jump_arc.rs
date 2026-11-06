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
//! The reckoning has no capsule sweeps at run time: a wall is a ray of the map's walker mesh (`body::walker_walls`, as the stand-in
//! casts; the bullet mesh misses walls the body bumps into) at [`AIR_RAYS`] within the move and the body's radius, or a floor
//! within the radius above the arc's feet by more than [`CATCH_M`] and under its head; the arc lands on the highest floor under
//! its feet on the way down.
//! LIKELY: the sprint jump's leg (the right leg's clip rises 1.350 m, the left's 1.282 m: the left's is taken), the short jump's
//! time scale 1, and the 0.1 m catch (the capsule's lower sphere on a lip). Also LIKELY: the clip's root alone from the press;
//! the jump's animation lexeme is blended in over 0.3 s (the builder at 0x343CCF), so the client's first ~6 ticks mix the clip's
//! root with the walk it leaves, and the landing blends back.
//!
//! The root tables are generated, not hand-made: `srp-026g/scripts/jump-arc/make_jump_arc.py` evaluates the stock clips
//! (`animations/single/weapons/ak/player/stand/3rd_view/jump/*.b-spline` of the stock `resources.db`; no row of
//! `release/content.tsv` names those clips) as clamped cubic B-splines and writes this file; its README names how to get them.

use mapdata::{NodeId, RayTree, Vec3, WalkGrid};

use super::kin::{Local, TICK_S, to_world};
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
/// How far over the arc's feet a floor's top may be and neither hold the body as a wall nor be missed coming down (the capsule's
/// lower sphere on a lip; the plain jump's measured reach 1.09 m over its clip's 0.998 m apex), m.
pub const CATCH_M: f32 = 0.1;
/// The walk cap, m/s: the speed at a press for the sprint jump, and the fastest a body on the floor in a jump moves (the
/// sprint-jump RE 1.2, 3.2).
const WALK_CAP_MS: f32 = 3.5;
/// The sprint jump's prepare phase, its mean (the run's step left at the press: 0-150 ms, 0-1.64 m, FINDINGS 3.2): LIKELY.
const PREPARE_MS: f32 = 75.0;
const PREPARE_M: f32 = 0.82;
/// A body falling after its clip, m/s^2 (the bench stand-in's).
const GRAVITY: f32 = 9.81;
/// The heights over the feet whose rays a wall holds a body in the air at, m (the stand-in's `AIR_RAYS`: the feet a little over,
/// the middle, the head). A thin horizontal bar between them (Vector Laboratory's railing: bars at 0.43 / 0.70 m, the top rail
/// 7 cm thick at 1.02 m over the platform) is caught by the column test ([`column_blocked`]).
const AIR_RAYS: [f32; 3] = [0.1, 0.9, 1.7];
/// How deep under the arc's feet a floor still makes a cell open air rather than a wall (a drop off a ledge), m.
const OPEN_BELOW_M: f32 = 10.0;
/// How many cells round a target the floorless test looks (a ledge lip's eroded band is crossed, a wall's is not): GUESS 1.5.
const FLOORLESS_CELLS: f32 = 1.5;
/// Where round the feet, as a share of the body's radius, the rays down for a top to come down on are cast (the capsule's lower
/// sphere rests on an edge within its radius): GUESS 0.7.
const PERCH_RING: f32 = 0.7;
/// How far over the grid's floor a top of the walker mesh must be to land on it rather than on the floor (a rail, a sill, a lip
/// too narrow for the grid), m.
const PERCH_OVER_M: f32 = 0.1;
/// How far apart along a tick's move the tops to come down on are looked for (the catch-up after a wall carries the body a metre
/// or more in a tick), m.
const PERCH_STEP_M: f32 = 0.1;

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

/// A jump in the reckoning, as the bench's honest stand-in flies one (`tests/standin_jump.rs` `Flight`, `fly`): the clip's root
/// from the feet at the press, at the press's facing (the keys pick the clip there; after it only the air control steers); on the
/// floor before the take-off at the walk cap at most, in the air after it, a step a wall holds caught up once clear (the chase
/// rule, FINDINGS 5); down on the highest floor under the feet on the way down or at the clip's end, else a fall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpArc {
    pressed: u32,
    clip: Clip,
    /// The feet and the yaw at the press: the clip's root is counted from them.
    from: Vec3,
    yaw: f32,
    /// The phase: on the floor, in the air over the floor it left, or falling after the clip.
    phase: Phase,
    /// How far the air control has moved the body off the clip's way so far (map frame, m).
    air: [f32; 2],
}

/// Where a jump is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Before the take-off.
    Floor,
    /// In the air, the root's rise counted from `base` (the floor's height when the body left it).
    Air { base: f32 },
    /// After the clip with no floor under the feet: falling at `speed` m/s.
    Fall { speed: f32 },
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
            arc.phase = Phase::Air { base: feet.y - rise };
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

    /// The body at `t1` (ms), from `feet` at the tick before, its yaw `yaw` and keys `keys` now, over `grid` (its body's radius
    /// and height stand in for the capsule).
    pub fn step(&mut self, grid: &WalkGrid, walls: Option<&RayTree>, feet: Vec3, yaw: f32, keys: u32, t1: u32) -> ArcStep {
        let dt = TICK_S;
        let body = grid.body();
        let (radius, height) = (body.radius, body.stand_height);
        if let Phase::Fall { speed } = self.phase {
            let speed = speed + GRAVITY * dt;
            let y = feet.y - speed * dt;
            if let Some(top) = perch_over(grid, walls, Vec3::new(feet.x, y, feet.z), feet.y, radius) {
                return ArcStep::Landed(Vec3::new(feet.x, top, feet.z), None);
            }
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
        let mut target = (self.from.x + travel[0], self.from.z + travel[1]);
        if frame <= TAKEOFF_FRAME {
            // On the floor: toward the root at the walk cap at most.
            let (dx, dz) = (target.0 - feet.x, target.1 - feet.z);
            let len = dx.hypot(dz);
            let k = if len > 1e-6 { (WALK_CAP_MS * dt / len).min(1.0) } else { 0.0 };
            return ArcStep::Ground([dx * k, dz * k]);
        }
        let base = match self.phase {
            Phase::Air { base } => base,
            _ => feet.y,
        };
        self.phase = Phase::Air { base };
        // Off the floor the keys held now steer it.
        let [ca, cl] = self.clip.air_control(keys);
        let steer = to_world([ca * dt, cl * dt], yaw);
        self.air = [self.air[0] + steer[0], self.air[1] + steer[1]];
        target = (target.0 + self.air[0], target.1 + self.air[1]);
        let r = self.clip.root(frame);
        let y = base + r[0];
        // A wall: a ray at one of the heights meets the map within the move and the radius, or a floor top within the radius above
        // the feet by more than the catch and under the head.
        let blocked = |x: f32, z: f32| {
            let (dx, dz) = (x - feet.x, z - feet.z);
            let len = dx.hypot(dz);
            let rays = walls.is_some_and(|walls| {
                len > 1e-6
                    && AIR_RAYS.iter().any(|&h| {
                        let dir = Vec3::new(dx / len, 0.0, dz / len);
                        walls.ray_cast(Vec3::new(feet.x, y + h, feet.z), dir, len + radius).is_some()
                    })
            });
            // The walker mesh's walls are the grid's floorless ground (the erosion round a wall): ground with no floor from the feet
            // (and the catch) down to `OPEN_BELOW_M` in any cell within `FLOORLESS_CELLS` of the target is a wall too. A ledge's lip
            // erodes a band of a cell or so on both levels, which the box reaches across; a wall's band (its thickness and the erosion
            // on both sides) is wider; a jump off a ledge has the lower floor under it.
            let reach = grid.cell_size() * FLOORLESS_CELLS;
            let floorless = !grid.floors_in([x - reach, z - reach, x + reach, z + reach]).any(|n| {
                let top = grid.position(n).y;
                top <= y + CATCH_M && top >= y - OPEN_BELOW_M
            });
            // A surface of the walker mesh in the body's column along the move (from the catch over the feet to the head): a rail
            // too thin for any of the horizontal rays' heights.
            let column = walls.is_some_and(|walls| column_blocked(walls, feet, (x, z), y, radius, height));
            rays || column || floorless
                || grid.floors_in([x - radius, z - radius, x + radius, z + radius]).any(|n| {
                    let top = grid.position(n).y;
                    top > y + CATCH_M && top < y + height
                })
        };
        // `step_jump_forward`: all the way, else along one axis, else held (and caught up once clear).
        let (x, z) = [target, (target.0, feet.z), (feet.x, target.1)]
            .into_iter()
            .find(|&(x, z)| (x, z) != (feet.x, feet.z) && !blocked(x, z))
            .unwrap_or((feet.x, feet.z));
        let at = Vec3::new(x, y, z);
        let over = frame >= END_FRAME;
        if frame >= self.clip.apex_frame() || over {
            // Down on a top of the walker mesh the grid has no floor for (the client stands on a railing's top rail), else on the
            // grid's floor.
            if let Some(on) = perch_along(grid, walls, feet, at, radius) {
                return ArcStep::Landed(on, None);
            }
            if let Some((node, on)) = air_floor(grid, at, feet.y.max(y) + CATCH_M)
                && on.y >= y - 1e-4
            {
                return ArcStep::Landed(on, Some(node));
            }
        }
        if over {
            self.phase = Phase::Fall { speed: 0.0 };
        }
        ArcStep::Air(at)
    }
}

/// The highest top of the walker mesh `walls` that feet coming down from `from_y` to `at` meet within the body's radius, m: rays
/// straight down from the catch over `from_y` at the centre and at [`PERCH_RING`] of the radius on four sides.
fn perch(walls: &RayTree, at: Vec3, from_y: f32, radius: f32) -> Option<f32> {
    let top = from_y + CATCH_M;
    let reach = top - at.y;
    if reach <= 0.0 {
        return None;
    }
    let k = radius * PERCH_RING;
    [(0.0, 0.0), (k, 0.0), (-k, 0.0), (0.0, k), (0.0, -k)]
        .into_iter()
        .filter_map(|(dx, dz)| {
            let from = Vec3::new(at.x + dx, top, at.z + dz);
            walls.ray_cast(from, Vec3::new(0.0, -1.0, 0.0), reach).map(|t| top - t)
        })
        .reduce(f32::max)
}

/// [`perch`]'s top where the grid has no floor that high under `at` (by more than [`PERCH_OVER_M`]): a rail, a sill or a lip the
/// grid's cells are too coarse for, which the client's capsule comes down on.
fn perch_over(grid: &WalkGrid, walls: Option<&RayTree>, at: Vec3, from_y: f32, radius: f32) -> Option<f32> {
    let top = perch(walls?, at, from_y, radius)?;
    let floor = air_floor(grid, at, top + CATCH_M).map(|(_, on)| on.y);
    floor.is_none_or(|y| top > y + PERCH_OVER_M).then_some(top)
}

/// Whether a surface of `walls` lies in the body's column (over the catch above `y`, under the head) along the move from `feet` to
/// `to` and the radius past it: rays straight down from the head every [`PERCH_STEP_M`] on the move's line and half the radius to
/// either side.
fn column_blocked(walls: &RayTree, feet: Vec3, to: (f32, f32), y: f32, radius: f32, height: f32) -> bool {
    let (dx, dz) = (to.0 - feet.x, to.1 - feet.z);
    let len = dx.hypot(dz);
    if len <= 1e-6 {
        return false;
    }
    let (ux, uz) = (dx / len, dz / len);
    let reach = len + radius;
    let steps = (reach / PERCH_STEP_M).ceil() as u32;
    let down = Vec3::new(0.0, -1.0, 0.0);
    let span = height - CATCH_M;
    (1..=steps).any(|i| {
        let along = reach * i as f32 / steps as f32;
        [0.0, radius * 0.5, -radius * 0.5].into_iter().any(|side| {
            let from = Vec3::new(feet.x + ux * along - uz * side, y + height, feet.z + uz * along + ux * side);
            walls.ray_cast(from, down, span).is_some()
        })
    })
}

/// The first top of [`perch_over`] along the move from `feet` to `at` (every [`PERCH_STEP_M`]), as the client's capsule sweeps a
/// tick's move: where the feet stand on it.
fn perch_along(grid: &WalkGrid, walls: Option<&RayTree>, feet: Vec3, at: Vec3, radius: f32) -> Option<Vec3> {
    let (dx, dz) = (at.x - feet.x, at.z - feet.z);
    let steps = (dx.hypot(dz) / PERCH_STEP_M).ceil().max(1.0) as u32;
    (1..=steps).find_map(|i| {
        let f = i as f32 / steps as f32;
        let p = Vec3::new(feet.x + dx * f, at.y, feet.z + dz * f);
        perch_over(grid, walls, p, feet.y.max(at.y), radius).map(|top| Vec3::new(p.x, top, p.z))
    })
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

    /// A railing's top rail (3 cm wide, 1.05 m up) under one side of the feet on the way down is landed on; with the rail out of
    /// reach, or the feet still above the catch over it, nothing is.
    #[test]
    fn the_feet_come_down_on_a_rail_the_grid_has_no_floor_for() {
        let rail = |y: f32| {
            let (a, b) = (Vec3::new(-5.0, y, 0.0), Vec3::new(5.0, y, 0.0));
            let (c, d) = (Vec3::new(5.0, y, 0.03), Vec3::new(-5.0, y, 0.03));
            RayTree::from_corners([[a, b, c], [a, c, d]])
        };
        let walls = rail(1.05);
        let r = 0.35;
        let on = perch(&walls, Vec3::new(0.0, 1.0, 0.25), 1.06, r).expect("the rail under the ring");
        assert!((on - 1.05).abs() < 1e-3, "{on}");
        assert_eq!(perch(&walls, Vec3::new(0.0, 1.0, 0.6), 1.06, r), None, "the rail out of reach");
        assert_eq!(perch(&walls, Vec3::new(0.0, 1.2, 0.25), 1.25, r), None, "still above it");
    }

    /// A rail at 1.05 m between two of the horizontal rays' heights still holds a body whose feet are under its top by more than the
    /// catch, and lets it over once the feet are within the catch of the top.
    #[test]
    fn a_thin_rail_in_the_body_s_column_holds_it() {
        let (a, b) = (Vec3::new(-5.0, 1.05, 0.0), Vec3::new(5.0, 1.05, 0.0));
        let (c, d) = (Vec3::new(5.0, 1.05, 0.03), Vec3::new(-5.0, 1.05, 0.03));
        let walls = RayTree::from_corners([[a, b, c], [a, c, d]]);
        let from = Vec3::new(0.0, 0.0, -0.8);
        assert!(column_blocked(&walls, from, (0.0, 0.9), 0.85, 0.38, 1.8), "feet 0.2 m under the top");
        assert!(!column_blocked(&walls, from, (0.0, 0.9), 0.98, 0.38, 1.8), "feet within the catch");
        assert!(!column_blocked(&walls, from, (2.0, -0.8), 0.85, 0.38, 1.8), "moving along it, clear of it");
    }

    /// A tick's move that carries the body from one side of a rail to the other (the catch-up after a wall) comes down on the rail,
    /// as the client's capsule sweeping it does, not past it.
    #[test]
    fn a_move_across_a_rail_comes_down_on_it() {
        let (a, b) = (Vec3::new(-5.0, 1.05, 0.0), Vec3::new(5.0, 1.05, 0.0));
        let (c, d) = (Vec3::new(5.0, 1.05, 0.03), Vec3::new(-5.0, 1.05, 0.03));
        let walls = RayTree::from_corners([[a, b, c], [a, c, d]]);
        let grid = WalkGrid::build(&crate::v026g::bots::body::tests::floor(-3.0, -3.0, 3.0, 3.0, 0.0), [-3.0, -3.0, 3.0, 3.0], 0.25, mapdata::Body::client());
        let r = grid.body().radius;
        let on = perch_along(&grid, Some(&walls), Vec3::new(0.0, 1.0, -0.8), Vec3::new(0.0, 0.99, 0.9), r).expect("on the rail");
        assert!((on.y - 1.05).abs() < 1e-3 && on.z < 0.3, "{on:?}");
        assert_eq!(perch_over(&grid, Some(&walls), Vec3::new(0.0, 0.99, 0.9), 1.0, r), None, "the end alone misses it");
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

    fn step_all(mut arc: JumpArc, grid: &WalkGrid, mut feet: Vec3, keys: u32) -> (Vec3, Option<NodeId>, u32) {
        let mut t = 0;
        loop {
            match arc.step(grid, None, feet, 0.0, keys, t + 50) {
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
        let (at, node, t) = step_all(arc, &grid, Vec3::ZERO, key::FORWARD);
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
            let arc = JumpArc::pressed(0, key::FORWARD, 3.0, Vec3::ZERO, 0.0);
            let (at, _, _) = step_all(arc, &grid, Vec3::ZERO, key::FORWARD);
            assert_eq!(at.y > rise - 0.05, up, "rise {rise}: {at:?}");
            if !up {
                assert!(at.z < 1.5, "the face stopped it: {at:?}");
            }
        }
    }
}
