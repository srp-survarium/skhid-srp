//! [A364 C2 rows 11/12] A forward jump as the 0.26g client drives the controller, step by step from the press to the ground
//! steps after the landing: the walk vector (the jump clip's root motion), the air control and the calls `base_player::jump`
//! 0x2D7920 / `end_jump` 0x7F050 make on [`Controller`]. A port of the exactness rig's jump driver
//! (`private/grid/rig/mapflights/src/driver.rs`; each rule's source is in its `notes-link-walk.md`), for the forward jumps a
//! bot presses: the short jump (`on_site` and `on_site_fwd` blended) and the sprint jump (`sprint/sprint_from_left_leg`,
//! `sprint/sprint_from_right_leg`). The one flyer of the grid's jump checks.
//!
//! In short (step n = the step ending n x 50 ms after the press's input, n = 0 the press's own step):
//!
//! - n = 0: the walk is the gait the body had, cut to the walk cap (the selector is in the jump state already).
//! - Short jump: the clips start at the press; the 0.1 s transition (`short_jump_transition_time`) blends the jump in from
//!   the gait, weight 0.5 at n = 1 and 1 from n = 2. The jump is `on_site` at w and `on_site_fwd` at 1 - w,
//!   w = 1 - min(max(1, v) / 4.0, 0.99) (0x337280; 4.0 the forward row of `jump_on_site_landing`). The clip frame at step
//!   n's end is 1.5 n. `jump()` before n = 4; air n = 4..15; `end_jump()` before n = 16, the first ground step.
//! - Sprint jump: n = 1..5 on the floor at the walk cap along the clip's way; `jump()` before n = 6, whose step ends at the
//!   clip frame `first_air_frame` (6.57-7.44 by the prepare's phase), 1.5 frames a step on; air n = 6..17; `end_jump()`
//!   before n = 18.
//! - In the air the walk is the clip's, uncapped, with its rise; on the floor the horizontal walk is cut to the cap
//!   (0x2D9060). The air control is the keys' direction at `air_control_speed` 1.0 m/s, none for forward alone in a
//!   sprint jump. The crouch is held as it was (the selector's jump state); the sprint flag is down from the press.
//! - The air branch never steps up (0x7BF26: the jump branch has no `step_up`; 0.0000 m over the clip in 2,245 flights):
//!   a top higher than the clip's apex is got onto only when the capsule's round bottom comes to rest on its edge as the
//!   clip comes down and the ground steps after `end_jump` climb the rest (`notes-mapflights.md` §2 C).
use super::jump_clips::{self, ClipKeys};
use super::tick::{Controller, CrouchInput, Settings, TickInput};
use super::Collision;
use crate::math::Vec3;

/// A step, seconds (the game's physics step).
pub const DT: f32 = 0.05;
/// `air_control_speed` (player+304h, `default.player`: 1.0), m/s.
const AIR_CONTROL_MS: f32 = 1.0;
/// The short jump's first air step and its landing step (PROVEN, `notes-link-walk.md` §2).
const SHORT_TAKEOFF_N: u32 = 4;
const SHORT_LANDING_N: u32 = 16;
/// The sprint jump's (the same).
const SPRINT_TAKEOFF_N: u32 = 6;
const SPRINT_LANDING_N: u32 = 18;
/// Clip frames a step (30 a second, 50 ms).
const FRAMES_PER_STEP: f64 = 1.5;
/// `jump_on_site_landing` for the forward short jump (the loader's table order, read by index: 0x337280).
const FORWARD_LANDING_S: f32 = 4.0;
/// The walk cap of a body that does not sprint, m/s (`max_speed` 3.5 x the kit's factor 1).
pub const WALK_CAP_MS: f32 = 3.5;

/// [A385] A flight's body and world: the body's own walk cap (`base_player::max_speed` 0x2DC010: the stance's base speed through
/// the kit's movement modifiers, x `long_jump_speed_factor` 1.0, exactness 2026-11-14) and the world's height off the map's, as a
/// lift of the body at its first air step (`dy` m: the top `dy` lower against the arc; a model of a world 1-2 cm off, not a
/// client rule).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    /// The walk cap, m/s.
    pub cap: f32,
    /// The lift at the first air step, m.
    pub dy: f32,
}

impl Flight {
    /// The grid's body: the kit factor 1, the map's world.
    pub const CLIENT: Self = Self { cap: WALK_CAP_MS, dy: 0.0 };

    /// The sprint at this walk cap: [`SPRINT_MS`] scaled by the same kit factor.
    pub fn sprint_ms(self) -> f32 {
        SPRINT_MS * self.cap / WALK_CAP_MS
    }
}

/// The run cycle's leg a sprint jump starts from (the selector's +50h): its clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    Left,
    Right,
}

/// The forward jump the client plays for a press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Press {
    /// The short jump from a walk at `speed` m/s (the body's horizontal speed at the press, `m_linear_horizontal_speed`).
    Walk { speed: f32 },
    /// The sprint jump from a sprint at `speed` m/s, off `leg`, its first air step ending at the clip frame
    /// `first_air_frame` (the prepare's phase, 6.57-7.44).
    Sprint { speed: f32, leg: Leg, first_air_frame: f32 },
}

impl Press {
    fn speed(self) -> f32 {
        match self {
            Self::Walk { speed } | Self::Sprint { speed, .. } => speed,
        }
    }

    fn takeoff_n(self) -> u32 {
        match self {
            Self::Walk { .. } => SHORT_TAKEOFF_N,
            Self::Sprint { .. } => SPRINT_TAKEOFF_N,
        }
    }

    /// The step before which `end_jump` is called: the first ground step.
    pub fn landing_n(self) -> u32 {
        match self {
            Self::Walk { .. } => SHORT_LANDING_N,
            Self::Sprint { .. } => SPRINT_LANDING_N,
        }
    }
}

/// One channel of a clip, evaluated as a clamped cubic B-spline over its knots (`clip_table.py`'s `curve`, the exe's loader's
/// layout): the keys with a value are control points, the last knot repeated four times closes it.
fn channel(keys: &[(u32, u32)], t: f64) -> f64 {
    let end = f64::from(f32::from_bits(keys[keys.len() - 1].0));
    let points: Vec<(f64, f64)> = keys
        .iter()
        .map(|&(k, v)| (f64::from(f32::from_bits(k)), f64::from(f32::from_bits(v))))
        .filter(|&(_, v)| v > -1e8)
        .collect();
    let n = points.len();
    let mut knots: Vec<f64> = points.iter().map(|&(k, _)| k).collect();
    knots.extend([end; 4]);
    let t = t.clamp(0.0, end - 1e-6);
    let k = (3..n).filter(|&i| knots[i] <= t).max().unwrap_or(3);
    let mut d: [f64; 4] = std::array::from_fn(|j| points[j + k - 3].1);
    for r in 1..4 {
        for j in (r..4).rev() {
            let i = j + k - 3;
            let den = knots[i + 4 - r] - knots[i];
            let a = if den == 0.0 { 0.0 } else { (t - knots[i]) / den };
            d[j] = (1.0 - a) * d[j - 1] + a * d[j];
        }
    }
    d[3]
}

/// A clip's root at `frame`: [rise, ahead, side] from the clip's start, m.
fn root(clip: &ClipKeys, frame: f64) -> [f64; 3] {
    [channel(clip.rise, frame), channel(clip.ahead, frame), channel(clip.side, frame)]
}

/// The jump's clips and their weights for a press.
struct Driver {
    press: Press,
    /// Short: `on_site` at `w`, `on_site_fwd` at 1 - w. Sprint: the leg's clip alone (`w` 0).
    a: &'static ClipKeys,
    b: &'static ClipKeys,
    w: f64,
    /// The forward key held in the air ([`Keys::air_forward`]).
    air_forward: bool,
    flight: Flight,
}

impl Driver {
    fn new(press: Press, flight: Flight) -> Self {
        match press {
            Press::Walk { speed } => {
                let w = 1.0 - f64::from((speed.max(1.0) / FORWARD_LANDING_S).min(0.99));
                Self { press, a: &jump_clips::ON_SITE, b: &jump_clips::ON_SITE_FWD, w, air_forward: true, flight }
            }
            Press::Sprint { leg, .. } => {
                let clip = match leg {
                    Leg::Left => &jump_clips::SPRINT_FROM_LEFT_LEG,
                    Leg::Right => &jump_clips::SPRINT_FROM_RIGHT_LEG,
                };
                Self { press, a: clip, b: clip, w: 0.0, air_forward: true, flight }
            }
        }
    }

    fn mix(&self, frame: f64) -> [f64; 3] {
        let (a, b) = (root(self.a, frame), root(self.b, frame));
        let w = self.w;
        std::array::from_fn(|i| w * a[i] + (1.0 - w) * b[i])
    }

    fn delta(&self, f0: f64, f1: f64) -> [f64; 3] {
        let (a, b) = (self.mix(f0), self.mix(f1));
        std::array::from_fn(|i| b[i] - a[i])
    }

    fn in_air(&self, n: u32) -> bool {
        (self.press.takeoff_n()..self.press.landing_n()).contains(&n)
    }

    /// Step n's root motion in the body's frame: (ahead, side, rise), m. The gait is a steady walk ahead at the press's
    /// speed, cut to its own size or the walk cap's, whichever is larger (the rig's `gait` with no ramp).
    fn motion(&self, n: u32) -> (f32, f32, f32) {
        let fr = FRAMES_PER_STEP;
        let gait = self.press.speed() * DT;
        match self.press {
            _ if n == 0 => (gait, 0.0, 0.0),
            Press::Walk { .. } => {
                let u = (f64::from(n) * 0.5).min(1.0);
                let d = self.delta(fr * f64::from(n - 1), fr * f64::from(n));
                ((u * d[1] + (1.0 - u) * f64::from(gait)) as f32, (u * d[2]) as f32, (u * d[0]) as f32)
            }
            Press::Sprint { first_air_frame, .. } => {
                if self.in_air(n) {
                    let f1 = f64::from(first_air_frame) + fr * f64::from(n - SPRINT_TAKEOFF_N);
                    let d = self.delta(f1 - fr, f1);
                    (d[1] as f32, d[2] as f32, d[0] as f32)
                } else {
                    // On the floor the prepare's and the clip's travel are over the cap: the cap along the clip's way.
                    let r = self.mix(24.0);
                    let l = (r[1] * r[1] + r[2] * r[2]).sqrt();
                    let c = f64::from(self.flight.cap * DT);
                    ((r[1] / l * c) as f32, (r[2] / l * c) as f32, 0.0)
                }
            }
        }
    }

    /// Step n's input, the body heading `(fx, fz)` (unit, flat) with the forward key held.
    fn input(&self, n: u32, heading: (f32, f32)) -> TickInput {
        let (ahead, side, mut rise) = self.motion(n);
        if n == self.press.takeoff_n() {
            rise += self.flight.dy;
        }
        // (ahead, right) in the body's frame to the map's (x, z): right = (fz, -fx).
        let (fx, fz) = heading;
        let (mut wx, mut wz) = (ahead * fx + side * fz, ahead * fz - side * fx);
        let air = self.in_air(n);
        if !air {
            // 0x2D9060: the horizontal walk cut to `max_speed` x the step when the controller is not in a jump.
            let cap = self.flight.cap * DT;
            let l = (wx * wx + wz * wz).sqrt();
            if l >= cap && l > 0.0 {
                wx *= cap / l;
                wz *= cap / l;
            }
        }
        // The forward key: the air control ahead at 1 m/s in a short jump, none in a sprint jump.
        let air_control = match self.press {
            Press::Walk { .. } if air && self.air_forward => Vec3::new(fx * AIR_CONTROL_MS, 0.0, fz * AIR_CONTROL_MS),
            _ => Vec3::ZERO,
        };
        TickInput { walk: Vec3::new(wx, rise, wz), air_control, crouch: CrouchInput::Held { key: false }, sprinting: false }
    }
}

/// One step of a flight, for the reader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightStep {
    /// The jump's step (0 the press's), or the landing step + m for the m-th ground step after it.
    pub n: u32,
    pub feet: Vec3,
    pub in_jump: bool,
    /// The client's own "on the ground" after the step (`on_ground` 0x7EF90, the game's "can jump": standing, under 4 m/s
    /// vertical, on a walkable contact; [`Controller::on_ground`]).
    pub on_ground: bool,
}

/// One walk-branch step with the crouch tested and the key up (the rig's `tick_walk`).
fn walk_step(c: &mut Controller, world: &Collision, s: &Settings, walk: Vec3, sprinting: bool) {
    let input = TickInput { walk, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting };
    c.tick(world, DT, s, input);
}

/// [A385] How high over the press point's height a run-up's body is put before it comes down ([`settle`]): the ground at the
/// run-up's start may be lower than the take-off (a sloped approach), and a body put at the take-off's height there would run
/// in the air (owner-5 `a385-lost-links-client-truth.md` §2.1: Mamayev Kurgan R94, a walking press at 1.0 m pressed with the
/// feet at 28.34 over ground at 28.01). The same height as owner-5's `start_real`.
const RUN_UP_DROP_M: f32 = 0.6;

/// [A385] The steps a body put down is given to come to rest ([`settle`]): 2 s, the 0.6 m fall takes 7.
const SETTLE_STEPS_MAX: u32 = 40;

/// A body put down comes to rest: steps with no walk (a small downward speed at the first, so no step is skipped,
/// `can_skip_player_step`) until it is on the ground and its feet move under a millimetre in a step. False when it has not
/// in [`SETTLE_STEPS_MAX`] steps.
fn settle(c: &mut Controller, world: &Collision, s: &Settings) -> bool {
    c.velocity = Vec3::new(0.0, -0.01, 0.0);
    for _ in 0..SETTLE_STEPS_MAX {
        let before = c.feet();
        walk_step(c, world, s, Vec3::ZERO, false);
        if c.on_ground(s) && (c.feet() - before).length() < 1e-3 {
            return true;
        }
    }
    false
}

/// The keys of a flight ([`fly`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keys {
    /// The forward key held in the air (the air control); else no direction key.
    pub air_forward: bool,
    /// The forward key held after the landing; else let go.
    pub held_after: bool,
}

impl Keys {
    /// A bot's: the forward key held throughout (the motor walks the route on).
    pub const BOT: Self = Self { air_forward: true, held_after: true };
}

/// A body `c` on the floor jumps forward along `heading` (flat, unit) with `press` and `keys`: the jump's steps to the landing
/// step, then `ground_steps` walk steps ahead (the rig's `jump_run`): with the key held a short jump's walk goes back up from
/// the clip's last travel to the walk cap over 6 steps and a sprint jump's is at the cap; let go, the last travel runs down
/// to nothing over 6 steps. Each step's feet, the press's step first. The body is left where the flight left it.
pub fn fly(world: &Collision, s: &Settings, c: &mut Controller, heading: (f32, f32), press: Press, keys: Keys, ground_steps: u32) -> Vec<FlightStep> {
    fly_as(world, s, c, heading, press, keys, ground_steps, Flight::CLIENT)
}

/// [`fly`] for the body and world `flight` ([`Flight`]).
#[allow(clippy::too_many_arguments)]
pub fn fly_as(
    world: &Collision,
    s: &Settings,
    c: &mut Controller,
    heading: (f32, f32),
    press: Press,
    keys: Keys,
    ground_steps: u32,
    flight: Flight,
) -> Vec<FlightStep> {
    let cap = flight.cap;
    let mut driver = Driver::new(press, flight);
    driver.air_forward = keys.air_forward;
    let landing = press.landing_n();
    let mut steps = Vec::with_capacity((landing + ground_steps) as usize + 1);
    let mut last_travel = 0.0f32;
    for n in 0..=landing {
        if n == press.takeoff_n() {
            c.jump();
        } else if n == landing {
            c.end_jump();
        }
        let input = driver.input(n, heading);
        c.tick(world, DT, s, input);
        last_travel = Vec3::new(input.walk.x, 0.0, input.walk.z).length() / DT;
        steps.push(FlightStep { n, feet: c.feet(), in_jump: c.jumping, on_ground: c.on_ground(s) });
    }
    let way = Vec3::new(heading.0, 0.0, heading.1);
    for m in 1..=ground_steps {
        let ramp = m.min(6) as f32 / 6.0;
        let speed = match press {
            _ if !keys.held_after => last_travel * (1.0 - ramp),
            Press::Sprint { .. } => cap,
            Press::Walk { .. } => last_travel + (cap - last_travel) * ramp,
        };
        walk_step(c, world, s, way * (speed * DT), false);
        steps.push(FlightStep { n: landing + m, feet: c.feet(), in_jump: c.jumping, on_ground: c.on_ground(s) });
    }
    steps
}

/// [A385] The floor a run-up's walk-up to `speed` takes ([`run_up`]: 6 steps ramping to the speed, k / 6 of it at step k), m.
pub fn blend_m(speed: f32) -> f32 {
    blend_steps_m(speed, 6)
}

/// [A385] The floor the first `steps` steps of a walk-up to `speed` take (k / 6 of the speed at step k), m.
pub fn blend_steps_m(speed: f32, steps: u8) -> f32 {
    (1..=steps).map(|k| speed * DT * f32::from(k) / 6.0).sum()
}

/// [A385] The most walk-up steps a slow press is pressed after ([`Kind::Slow`]): at 6 the walk-up is at the cap ([`Kind::Walk`]).
pub const SLOW_STEPS_MAX: u8 = 5;

/// [A385] The slow press's run-up ([`Kind::Slow`]): a body put down `steps` steps' walk-up back from `press_at` (as [`run_up`]
/// puts it, [`RUN_UP_DROP_M`] over and come to rest) and walked up `steps` steps toward `cap` (k / 6 of it at step k); 0 steps
/// is a standing jump. None as [`run_up`].
pub fn run_up_slow(world: &Collision, s: &Settings, press_at: Vec3, heading: (f32, f32), cap: f32, steps: u8) -> Option<RunUp> {
    use crate::grid::controlled::{body_at, Stance, WalkStart};
    let way = Vec3::new(heading.0, 0.0, heading.1);
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let start = press_at - way * blend_steps_m(cap, steps);
    let mut c = body_at(world, WalkStart::Clear, Stance::Standing, Vec3::new(start.x, press_at.y + RUN_UP_DROP_M, start.z));
    if !settle(&mut c, world, s) {
        return None;
    }
    let mut v = 0.0;
    for k in 1..=steps {
        let before = c.origin;
        walk_step(&mut c, world, s, way * (cap * DT * f32::from(k) / 6.0), false);
        v = flat(c.origin - before).length() / DT;
    }
    let f = c.feet();
    (flat(f - press_at).length() <= 0.1 && (f.y - press_at.y).abs() <= 0.35 && !c.jumping && c.on_ground(s))
        .then_some(RunUp { body: c, speed: v })
}

/// How a run-up ended ([`run_up`]).
pub struct RunUp {
    /// The body at the press.
    pub body: Controller,
    /// Its last step's horizontal speed, m/s (the press's `m_linear_horizontal_speed`).
    pub speed: f32,
}

/// The run-up ending at `press_at` (feet) along `heading` at `speed` m/s (sprinting or not): a body put down 3.5 + 4 steps'
/// travel back, [`RUN_UP_DROP_M`] over `press_at`'s height, come to rest on the ground there ([`settle`]), walked up to speed
/// over 6 steps (the 0.3 s blend) then 4 steady steps; else with no steady steps. None when no run-up ends within 0.1 m across
/// and 0.35 m in height of `press_at`, on the ground. (A385: no longer two steps at speed from 2 steps' travel back, which no
/// client's body does, owner-5 §2.1; nor from a body still coming down.)
pub fn run_up(world: &Collision, s: &Settings, press_at: Vec3, heading: (f32, f32), speed: f32, sprinting: bool) -> Option<RunUp> {
    use crate::grid::controlled::{body_at, Stance, WalkStart};
    let way = Vec3::new(heading.0, 0.0, heading.1);
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let ok = |c: &Controller| {
        let f = c.feet();
        flat(f - press_at).length() <= 0.1 && (f.y - press_at.y).abs() <= 0.35 && !c.jumping && c.on_ground(s)
    };
    let put = |back: f32| {
        let start = press_at - way * back;
        let mut c = body_at(world, WalkStart::Clear, Stance::Standing, Vec3::new(start.x, press_at.y + RUN_UP_DROP_M, start.z));
        settle(&mut c, world, s).then_some(c)
    };
    for steady in [4u32, 0] {
        let Some(mut c) = put(speed * DT * (3.5 + steady as f32)) else { continue };
        let mut v = 0.0;
        for k in 1..=(6 + steady) {
            let before = c.origin;
            walk_step(&mut c, world, s, way * (speed * DT * (k.min(6) as f32) / 6.0), sprinting);
            v = flat(c.origin - before).length() / DT;
        }
        if ok(&c) {
            return Some(RunUp { body: c, speed: v });
        }
    }
    None
}

/// The gait of a run-up jump: a walk (the short jump) or a sprint (the sprint jump off `leg`, its first air step ending at
/// the clip frame `first_air_frame`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gait {
    Walk,
    Sprint { leg: Leg, first_air_frame: f32 },
}

/// The body's steady sprint at the movement factor 1, m/s (`bots::kin`, exactness F5: the config's desired speed x the
/// clip's root speed over its default, 10,909 of 11,214 traced steady sprint steps).
pub const SPRINT_MS: f32 = 4.9587;

/// A run-up ([`run_up`]) to `press_at` along `heading` at the gait's speed, then the jump ([`fly`]) with `ground_steps` steps
/// after the landing: None when there is no run-up. A sprint run-up under the walk cap presses the short jump (the client's
/// speed test, 0x33705A), as the rig does.
pub fn run_and_fly(world: &Collision, s: &Settings, press_at: Vec3, heading: (f32, f32), gait: Gait, ground_steps: u32) -> Option<Vec<FlightStep>> {
    let (speed, sprinting) = match gait {
        Gait::Walk => (WALK_CAP_MS, false),
        Gait::Sprint { .. } => (SPRINT_MS, true),
    };
    let RunUp { body: mut c, speed: v } = run_up(world, s, press_at, heading, speed, sprinting)?;
    let press = match gait {
        Gait::Sprint { leg, first_air_frame } if v >= WALK_CAP_MS => Press::Sprint { speed: v, leg, first_air_frame },
        _ => Press::Walk { speed: v },
    };
    Some(fly(world, s, &mut c, heading, press, Keys::BOT, ground_steps))
}

/// [A385] The sprint jump's first air frames flown: the prepare's phase is a continuum (0x3375A0: the time scale (B + A / C) / 2,
/// A the time left in the run clip's interval, exactness 2026-11-14); 6.57-7.44 is the range COUNTED over 16 traced sprint jumps
/// (not the client's bounds), flown at its two ends and its middle.
pub const SPRINT_FIRST_AIR_FRAMES: [f32; 3] = [6.57, 7.005, 7.44];

/// [A385] The sprint jump's variants, none of them the bot's to choose (the leg is the selector's +50h, 0x3371F3; the phase the
/// prepare's): each leg at each of [`SPRINT_FIRST_AIR_FRAMES`]. A sprint press lands only when every variant lands.
pub fn sprint_variants() -> impl Iterator<Item = (Leg, f32)> {
    [Leg::Left, Leg::Right].into_iter().flat_map(|leg| SPRINT_FIRST_AIR_FRAMES.into_iter().map(move |f| (leg, f)))
}

/// [A385] A press's gait, its sprint variants not chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The short jump from the walk at the body's walk cap.
    Walk,
    /// The short jump pressed this many steps into the walk-up ([`run_up_slow`], 0 to [`SLOW_STEPS_MAX`]; 0 a standing jump):
    /// a client press (the short jump blends by the speed at the press), and the bots' recorded ups were pressed at 0-2.8 m/s
    /// (owner-5 `a385-lost-links-client-truth.md` §2.2: Chemical Plant R10, 613 slow press points land against 133 at the cap;
    /// Vector Laboratory R57's top is reached in the exe's tick only by slow presses).
    Slow(u8),
    /// The sprint jump from the sprint, all its variants ([`sprint_variants`]).
    Sprint,
}

/// [A385] The neighbourhood's offsets (A385-FORMAT §2; the motor's measured spread, owner-5 §2.2): the lead earlier / later.
pub const NEIGHBOUR_LEADS_M: [f32; 4] = [-0.2, -0.1, 0.1, 0.2];
/// … the line moved to each side, m.
pub const NEIGHBOUR_SIDE_M: f32 = 0.15;
/// … the heading turned each way about the press point, rad.
pub const NEIGHBOUR_TURN_RAD: f32 = 0.1;
/// … the world off in height, each way, m ([`Flight::dy`]; Mamayev R2's one landing press turns on 1 cm).
pub const NEIGHBOUR_DY_M: [f32; 2] = [0.01, 0.02];
/// The flights of a neighbourhood ([`neighbourhood`]).
pub const NEIGHBOURS: usize = 14;

/// [A385] One flight of a press's neighbourhood: the press moved by these offsets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbour {
    /// Pressed this much earlier (more lead), m.
    pub lead: f32,
    /// The press point moved to the way's right, m.
    pub side: f32,
    /// The heading turned (counter-clockwise seen from above), rad.
    pub turn: f32,
    /// The world's height offset ([`Flight::dy`]), m.
    pub dy: f32,
    /// At the bots' fastest kit's walk cap ([`Kits::fastest`]) instead of the grid body's.
    pub fastest: bool,
}

impl Neighbour {
    const AT: Self = Self { lead: 0.0, side: 0.0, turn: 0.0, dy: 0.0, fastest: false };
}

/// [A385] A press's neighbourhood (A385-FORMAT §2): the press itself, the leads, the sides, the turns, the world's heights and
/// the fastest kit, [`NEIGHBOURS`] flights.
pub fn neighbourhood() -> [Neighbour; NEIGHBOURS] {
    let mut out = [Neighbour::AT; NEIGHBOURS];
    let mut i = 1;
    for lead in NEIGHBOUR_LEADS_M {
        out[i].lead = lead;
        i += 1;
    }
    for side in [-NEIGHBOUR_SIDE_M, NEIGHBOUR_SIDE_M] {
        out[i].side = side;
        i += 1;
    }
    for turn in [-NEIGHBOUR_TURN_RAD, NEIGHBOUR_TURN_RAD] {
        out[i].turn = turn;
        i += 1;
    }
    for dy in NEIGHBOUR_DY_M {
        for sign in [-1.0, 1.0] {
            out[i].dy = sign * dy;
            i += 1;
        }
    }
    out[i].fastest = true;
    out
}

/// [A385] A walk-off's neighbourhood (nav-builder's A389): [`neighbourhood`] without its lead row (a walk-off has no press).
pub fn neighbourhood_walk_off() -> Vec<Neighbour> {
    neighbourhood().into_iter().filter(|n| n.lead == 0.0).collect()
}

/// [A385] The walk caps a link is flown at: the grid body's and the bots' fastest kit's, m/s (both the body's
/// `base_player::max_speed` 0x2DC010 with its kit's movement modifiers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kits {
    pub body: f32,
    pub fastest: f32,
}

/// [A385] The ground steps a scoring flight runs after the jump: the release's run-down to rest over 6 steps ([`fly`] with the
/// key let go) and one step at rest.
pub const STAY_STEPS: u32 = 7;

/// [A385] The keys of a scoring flight: forward held to the landing, let go there (A385-FORMAT §1 "stays").
pub const STAY_KEYS: Keys = Keys { air_forward: true, held_after: false };

/// [A385] Where the neighbour `n` of a press `lead` metres before `takeoff` along `way` is pressed: `lead + n.lead` back along
/// the way and `n.side` to its right, at the take-off's height (the run-up ends there, [`neighbour_flights`]).
pub fn press_point(takeoff: Vec3, way: (f32, f32), lead: f32, n: Neighbour) -> Vec3 {
    let right = (way.1, -way.0);
    let back = lead + n.lead;
    Vec3::new(takeoff.x - way.0 * back + right.0 * n.side, takeoff.y, takeoff.z - way.1 * back + right.1 * n.side)
}

/// [A385] The flights of one neighbour `n` of the press of `kind` at `lead` m before `takeoff` (the take-off floor's centre)
/// along `way` (flat, unit): one for a walk, one per sprint variant for a sprint, each from its run-up ([`run_up`], shared by
/// the variants) with the keys let go at the landing ([`STAY_KEYS`], [`STAY_STEPS`]). A sprint run-up under the cap presses the
/// client's short jump. None when the run-up does not fit.
#[allow(clippy::too_many_arguments)]
pub fn neighbour_flights(
    world: &Collision,
    s: &Settings,
    takeoff: Vec3,
    way: (f32, f32),
    kind: Kind,
    lead: f32,
    n: Neighbour,
    kits: Kits,
) -> Option<Vec<Vec<FlightStep>>> {
    let flight = Flight { cap: if n.fastest { kits.fastest } else { kits.body }, dy: n.dy };
    let (c, sn) = (n.turn.cos(), n.turn.sin());
    let heading = (way.0 * c - way.1 * sn, way.0 * sn + way.1 * c);
    let at = press_point(takeoff, way, lead, n);
    match kind {
        Kind::Walk => {
            let RunUp { body: mut b, speed } = run_up(world, s, at, heading, flight.cap, false)?;
            Some(vec![fly_as(world, s, &mut b, heading, Press::Walk { speed }, STAY_KEYS, STAY_STEPS, flight)])
        }
        Kind::Slow(steps) => {
            let RunUp { body: mut b, speed } = run_up_slow(world, s, at, heading, flight.cap, steps)?;
            Some(vec![fly_as(world, s, &mut b, heading, Press::Walk { speed }, STAY_KEYS, STAY_STEPS, flight)])
        }
        Kind::Sprint => {
            let run = run_up(world, s, at, heading, flight.sprint_ms(), true)?;
            Some(
                sprint_variants()
                    .map(|(leg, first_air_frame)| {
                        let press = if run.speed >= flight.cap {
                            Press::Sprint { speed: run.speed, leg, first_air_frame }
                        } else {
                            Press::Walk { speed: run.speed }
                        };
                        let mut b = run.body;
                        fly_as(world, s, &mut b, heading, press, STAY_KEYS, STAY_STEPS, flight)
                    })
                    .collect(),
            )
        }
    }
}

/// [A385] A flight's landing and its rest: after the jump's first air step, the first step out of the jump on which the client
/// has the body on the ground (`on_ground` 0x7EF90) and `on` holds (the caller's floor: a rim catch is on the ground on the
/// top's edge before the ground steps climb the rest), and the flight's last step, when the body is on the ground on every step
/// from the landing on (it stays: A385-FORMAT §1); None when it never lands so or leaves the ground after (it skims).
pub fn landing_and_rest(steps: &[FlightStep], on: impl Fn(&FlightStep) -> bool) -> Option<(FlightStep, FlightStep)> {
    let first_air = steps.iter().position(|f| f.in_jump)?;
    let landing = first_air + steps[first_air..].iter().position(|f| !f.in_jump && f.on_ground && on(f))?;
    steps[landing..].iter().all(|f| f.on_ground).then(|| (steps[landing], steps[steps.len() - 1]))
}

/// [A385] A press's share (A385-FORMAT §2): of its [`NEIGHBOURS`] neighbours, how many land (`lands` holds for each of the
/// neighbour's flights: every sprint variant); a neighbour whose run-up does not fit does not land.
#[allow(clippy::too_many_arguments)]
pub fn score(
    world: &Collision,
    s: &Settings,
    takeoff: Vec3,
    way: (f32, f32),
    kind: Kind,
    lead: f32,
    kits: Kits,
    lands: impl Fn(&[FlightStep]) -> bool,
) -> u8 {
    neighbourhood()
        .into_iter()
        .filter(|&n| neighbour_flights(world, s, takeoff, way, kind, lead, n, kits).is_some_and(|f| f.iter().all(|st| lands(st))))
        .count() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Triangle, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    /// Ground at 0 for x < 5 and a block from x 5 to 12 whose top is `top` high (its face at x 5), 6 m wide.
    fn ledge(top: f32) -> Collision {
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-5.0, 0.0, -3.0), Vec3::new(-5.0, 0.0, 3.0), Vec3::new(5.0, 0.0, 3.0), Vec3::new(5.0, 0.0, -3.0));
        quad(&mut tris, Vec3::new(5.0, top, -3.0), Vec3::new(5.0, top, 3.0), Vec3::new(12.0, top, 3.0), Vec3::new(12.0, top, -3.0));
        quad(&mut tris, Vec3::new(5.0, 0.0, -3.0), Vec3::new(5.0, top, -3.0), Vec3::new(5.0, top, 3.0), Vec3::new(5.0, 0.0, 3.0));
        let (caster, _) = crate::Body { exe_collision: true, start_witness: true, ..crate::Body::client() }.controller();
        Collision::build(&tris).with_caster(caster)
    }

    /// [A385] A run-up whose start is over a lower level (a floor at 0 from x `edge` to 6, a level 3 m lower before it): the
    /// body put there comes to rest on the lower level and does not reach the press point on the take-off's floor, so no press
    /// is flown from a level the take-off is not on (owner-5 `mamayev-1808-lost-floors.md` §5.3: a sweep tool with no test at the
    /// press flew 754 such). With the take-off's floor long enough for the run-up, it is made.
    #[test]
    fn a_run_up_starts_on_the_take_offs_own_level() {
        let world = |edge: f32| {
            let mut tris = Vec::new();
            quad(&mut tris, Vec3::new(-5.0, -3.0, -3.0), Vec3::new(-5.0, -3.0, 3.0), Vec3::new(edge, -3.0, 3.0), Vec3::new(edge, -3.0, -3.0));
            quad(&mut tris, Vec3::new(edge, 0.0, -3.0), Vec3::new(edge, 0.0, 3.0), Vec3::new(6.0, 0.0, 3.0), Vec3::new(6.0, 0.0, -3.0));
            quad(&mut tris, Vec3::new(edge, -3.0, -3.0), Vec3::new(edge, 0.0, -3.0), Vec3::new(edge, 0.0, 3.0), Vec3::new(edge, -3.0, 3.0));
            let (caster, _) = crate::Body { exe_collision: true, start_witness: true, ..crate::Body::client() }.controller();
            Collision::build(&tris).with_caster(caster)
        };
        let press = Vec3::new(4.9, 0.0, 0.0);
        let s = settings();
        assert!(run_up(&world(4.75), &s, press, (1.0, 0.0), WALK_CAP_MS, false).is_none(), "a start over the lower level");
        assert!(run_up(&world(1.0), &s, press, (1.0, 0.0), WALK_CAP_MS, false).is_some(), "a start on the take-off's floor");
    }

    /// The grid's controller settings (the exe caster's: the wall normal by rays).
    fn settings() -> Settings {
        crate::Body { exe_collision: true, start_witness: true, ..crate::Body::client() }.controller().1
    }

    /// Whether a jump pressed `d` m before the face of a `top` ledge with `press`, run up 2 m, stands on the top 4 ground steps
    /// after the landing step.
    fn lands(top: f32, d: f32, gait: Option<Gait>) -> bool {
        use crate::grid::controlled::{body_at, Stance, WalkStart};
        let world = ledge(top);
        let s = settings();
        let at = Vec3::new(5.0 - d, 0.0, 0.0);
        let steps = match gait {
            Some(g) => run_and_fly(&world, &s, at, (1.0, 0.0), g, 10),
            None => {
                let mut c = body_at(&world, WalkStart::Clear, Stance::Standing, at);
                settle(&mut c, &world, &s);
                Some(fly(&world, &s, &mut c, (1.0, 0.0), Press::Walk { speed: 0.0 }, Keys::BOT, 10))
            }
        };
        steps.is_some_and(|steps| {
            let end = steps.last().unwrap().feet;
            end.x > 5.0 && (end.y - top).abs() < 0.05
        })
    }

    /// The table of the rim rule on made ledges (printed; `cargo test -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_ledge_table() {
        for top in [1.0f32, 1.2, 1.3, 1.4, 1.5, 1.6, 1.65, 1.7, 1.8] {
            let mut row = format!("top {top:.2}:");
            for (name, gait) in [
                ("walk", Some(Gait::Walk)),
                ("stand", None),
                ("sprintL", Some(Gait::Sprint { leg: Leg::Left, first_air_frame: 7.0 })),
                ("sprintR", Some(Gait::Sprint { leg: Leg::Right, first_air_frame: 7.0 })),
            ] {
                let ok: Vec<String> = (0..40).map(|i| 0.05 * i as f32).filter(|&d| lands(top, d, gait)).map(|d| format!("{d:.2}")).collect();
                row += &format!(" | {name} {}", ok.join(","));
            }
            println!("{row}");
        }
    }

    /// The forward short jump from a 3.5 m/s walk on flat ground: the feet top out at the clip's apex less its rise before
    /// the take-off, +0.988 at step 11 (exactness's trace `Crec_walk_fwd`, `notes-mapflights.md` §2 C).
    #[test]
    fn a_walking_jump_on_flat_ground_tops_out_where_the_trace_does() {
        let world = ledge(5.0);
        let steps = run_and_fly(&world, &settings(), Vec3::new(0.0, 0.0, 0.0), (1.0, 0.0), Gait::Walk, 0).expect("a run-up");
        let top = steps.iter().max_by(|a, b| a.feet.y.total_cmp(&b.feet.y)).unwrap();
        assert_eq!(top.n, 11, "{steps:?}");
        assert!((top.feet.y - 0.988).abs() < 0.01, "{top:?}");
    }

    /// The clips' apexes as `clip_table.py` gives them: the short jump 0.998 at frame 16, the sprint 1.282 (left leg) and
    /// 1.350 (right leg) at frame 14.
    #[test]
    fn the_clips_rise_to_their_known_apexes() {
        let apex = |c: &ClipKeys| (0..=2400).map(|i| root(c, f64::from(i) / 100.0)[0]).fold(f64::MIN, f64::max);
        assert!((apex(&jump_clips::ON_SITE) - 0.998).abs() < 5e-4);
        assert!((apex(&jump_clips::ON_SITE_FWD) - 0.998).abs() < 5e-4);
        assert!((apex(&jump_clips::SPRINT_FROM_LEFT_LEG) - 1.282).abs() < 5e-4);
        assert!((apex(&jump_clips::SPRINT_FROM_RIGHT_LEG) - 1.350).abs() < 5e-4);
        // The forward clip's travel at its end: 2.67 m (`SHORT_JUMP_AHEAD`'s last), the sprint 4.42 / 4.40.
        assert!((root(&jump_clips::ON_SITE_FWD, 24.0)[1] - 2.67).abs() < 5e-3);
        assert!((root(&jump_clips::SPRINT_FROM_LEFT_LEG, 24.0)[1] - 4.42).abs() < 5e-3);
    }

    /// This flyer against the exactness rig's own flights (`private/grid/rig/mapflights`, `mapflights batch`): its spec rows
    /// flown on the same map world (`Collision::build_exe`, the game's shapes), each row's end after 20 steps compared with the
    /// rig's. `RIG_MAP` (a format 6 map file), `RIG_SPEC` (`out/spec-*.txt`), `RIG_TSV` (its `out/flights-*.tsv`),
    /// `RIG_TOP` (`ylo,yhi`: the target's heights for the on-top count).
    #[test]
    #[ignore]
    fn compare_with_the_rig() {
        use crate::grid::controlled::{body_at, Stance, WalkStart};
        let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let map = crate::MapData::from_bytes(&std::fs::read(var("RIG_MAP")).unwrap()).unwrap();
        // `RIG_WORLD=walker`: the map's walker triangles with the grid's caster (the world the grid's checks run on).
        let world = if std::env::var("RIG_WORLD").as_deref() == Ok("walker") {
            let (caster, _) = crate::Body { exe_collision: true, start_witness: true, ..crate::Body::client() }.controller();
            Collision::build(&map.walker_triangles()).with_caster(caster)
        } else {
            Collision::build_exe(&map).expect("a format 6 map")
        };
        let s = Settings { wall_normal: crate::controller::tick::WallNormal::Rays, ..Settings::LIVE };
        let top: Vec<f32> = var("RIG_TOP").split(',').map(|v| v.parse().unwrap()).collect();
        let tsv = std::fs::read_to_string(var("RIG_TSV")).unwrap();
        let header: Vec<&str> = tsv.lines().find(|l| l.starts_with("# mode\tlabel\tgait")).unwrap()[2..].split('\t').collect();
        let col = |name: &str| header.iter().position(|&h| h == name).unwrap();
        let rig: std::collections::HashMap<String, Vec<String>> = tsv
            .lines()
            .filter(|l| l.starts_with("jump\t"))
            .map(|l| {
                let w: Vec<String> = l.split('\t').map(str::to_string).collect();
                (w[1].clone(), w)
            })
            .collect();
        let (mut rows, mut same_mm, mut same_cm, mut mine_on, mut rig_on, mut both_on) = (0, 0, 0, 0, 0, 0);
        for line in std::fs::read_to_string(var("RIG_SPEC")).unwrap().lines() {
            let w: Vec<&str> = line.split('#').next().unwrap().split_whitespace().collect();
            if w.first() != Some(&"jump") {
                continue;
            }
            let f = |i: usize| w[i].parse::<f32>().unwrap();
            let feet = Vec3::new(f(2), f(3), f(4));
            let l = f(5).hypot(f(6));
            let heading = (f(5) / l, f(6) / l);
            let leg = if w[8] == "l" { Leg::Left } else { Leg::Right };
            let keys = Keys { air_forward: w[10] == "fwd", held_after: w[11] == "held" };
            let start = match w[7] {
                "stand" => {
                    let mut c = body_at(&world, WalkStart::Clear, Stance::Standing, feet);
                    settle(&mut c, &world, &s);
                    Some(RunUp { body: c, speed: 0.0 })
                }
                "walk" => run_up(&world, &s, feet, heading, WALK_CAP_MS, false),
                _ => run_up(&world, &s, feet, heading, 5.0, true),
            };
            let Some(r) = rig.get(w[1]) else { continue };
            rows += 1;
            let Some(RunUp { body: mut c, speed: v }) = start else {
                if r[6] == "no_run_up" {
                    same_mm += 1;
                    same_cm += 1;
                }
                continue;
            };
            if r[6] == "no_run_up" {
                continue;
            }
            let press = if w[7] == "sprint" && v >= WALK_CAP_MS { Press::Sprint { speed: v, leg, first_air_frame: f(9) } } else { Press::Walk { speed: v } };
            let end = fly(&world, &s, &mut c, heading, press, keys, 20).last().unwrap().feet;
            let theirs = Vec3::new(r[col("end_x")].parse().unwrap(), r[col("end_y")].parse().unwrap(), r[col("end_z")].parse().unwrap());
            let d = (end - theirs).length();
            same_mm += usize::from(d < 2e-3);
            same_cm += usize::from(d < 1e-2);
            let on = |p: Vec3| (top[0]..=top[1]).contains(&p.y);
            mine_on += usize::from(on(end));
            rig_on += usize::from(on(theirs));
            both_on += usize::from(on(end) && on(theirs));
            if d >= 1e-2 {
                println!("differs {} by {d:.3}: mine ({:.3} {:.3} {:.3}) rig ({:.3} {:.3} {:.3})", w[1], end.x, end.y, end.z, theirs.x, theirs.y, theirs.z);
            }
        }
        println!("rows {rows}: within 2 mm {same_mm}, within 1 cm {same_cm}; on the top: mine {mine_on}, rig {rig_on}, both {both_on}");
    }
}
