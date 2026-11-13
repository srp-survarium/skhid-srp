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
}

impl Driver {
    fn new(press: Press) -> Self {
        match press {
            Press::Walk { speed } => {
                let w = 1.0 - f64::from((speed.max(1.0) / FORWARD_LANDING_S).min(0.99));
                Self { press, a: &jump_clips::ON_SITE, b: &jump_clips::ON_SITE_FWD, w, air_forward: true }
            }
            Press::Sprint { leg, .. } => {
                let clip = match leg {
                    Leg::Left => &jump_clips::SPRINT_FROM_LEFT_LEG,
                    Leg::Right => &jump_clips::SPRINT_FROM_RIGHT_LEG,
                };
                Self { press, a: clip, b: clip, w: 0.0, air_forward: true }
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
                    let c = f64::from(WALK_CAP_MS * DT);
                    ((r[1] / l * c) as f32, (r[2] / l * c) as f32, 0.0)
                }
            }
        }
    }

    /// Step n's input, the body heading `(fx, fz)` (unit, flat) with the forward key held.
    fn input(&self, n: u32, heading: (f32, f32)) -> TickInput {
        let (ahead, side, rise) = self.motion(n);
        // (ahead, right) in the body's frame to the map's (x, z): right = (fz, -fx).
        let (fx, fz) = heading;
        let (mut wx, mut wz) = (ahead * fx + side * fz, ahead * fz - side * fx);
        let air = self.in_air(n);
        if !air {
            // 0x2D9060: the horizontal walk cut to `max_speed` x the step when the controller is not in a jump.
            let cap = WALK_CAP_MS * DT;
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
}

/// One walk-branch step with the crouch tested and the key up (the rig's `tick_walk`).
fn walk_step(c: &mut Controller, world: &Collision, s: &Settings, walk: Vec3, sprinting: bool) {
    let input = TickInput { walk, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting };
    c.tick(world, DT, s, input);
}

/// A body put down: three steps with no walk and a small downward speed (so no step is skipped, `can_skip_player_step`), as
/// the rig's `settle` does.
fn settle(c: &mut Controller, world: &Collision, s: &Settings) {
    for _ in 0..3 {
        c.velocity = Vec3::new(0.0, -0.01, 0.0);
        walk_step(c, world, s, Vec3::ZERO, false);
    }
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
    let mut driver = Driver::new(press);
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
        steps.push(FlightStep { n, feet: c.feet(), in_jump: c.jumping });
    }
    let way = Vec3::new(heading.0, 0.0, heading.1);
    for m in 1..=ground_steps {
        let ramp = m.min(6) as f32 / 6.0;
        let speed = match press {
            _ if !keys.held_after => last_travel * (1.0 - ramp),
            Press::Sprint { .. } => WALK_CAP_MS,
            Press::Walk { .. } => last_travel + (WALK_CAP_MS - last_travel) * ramp,
        };
        walk_step(c, world, s, way * (speed * DT), false);
        steps.push(FlightStep { n: landing + m, feet: c.feet(), in_jump: c.jumping });
    }
    steps
}

/// How a run-up ended ([`run_up`]).
pub struct RunUp {
    /// The body at the press.
    pub body: Controller,
    /// Its last step's horizontal speed, m/s (the press's `m_linear_horizontal_speed`).
    pub speed: f32,
}

/// The run-up ending at `press_at` (feet) along `heading` at `speed` m/s (sprinting or not), as the rig's `run_up` makes it: a
/// body put down 3.5 + 4 steps' travel back, walked up to speed over 6 steps (the 0.3 s blend) then 4 steady steps; else
/// with no steady steps; else two steps at speed from 2 steps' travel back. None when no run-up ends within 0.1 m across and
/// 0.35 m in height of `press_at`, on the ground.
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
        let mut c = body_at(world, WalkStart::Clear, Stance::Standing, Vec3::new(start.x, press_at.y, start.z));
        settle(&mut c, world, s);
        c
    };
    for steady in [4u32, 0] {
        let mut c = put(speed * DT * (3.5 + steady as f32));
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
    let mut c = put(2.0 * speed * DT);
    let mut v = 0.0;
    for _ in 0..2 {
        let before = c.origin;
        walk_step(&mut c, world, s, way * (speed * DT), sprinting);
        v = flat(c.origin - before).length() / DT;
    }
    ok(&c).then_some(RunUp { body: c, speed: v })
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
        let world = Collision::build_exe(&map).expect("a format 6 map");
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
