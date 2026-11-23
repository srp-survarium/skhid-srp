//! [body physics S1] `client-body`: one model of the 0.26g client's body between its keys and its feet, for every layer that predicts how a body
//! moves: the grid builder's flights, the motor's press forecast, the server's reckoning of a bot and the bench's stand-in. Today each
//! of them wraps the ported walk tick ([`crate::controller::tick::Controller`], the exe's `player_step`) in rules of its own (how keys
//! become the walk vector, which state a body starts from, who owns the feet's height, how a fall goes, when a body has landed); the
//! wrappers drift apart and from the exe. This crate is the one wrapper. Nothing calls it yet (S2-S5 move the layers onto it).
//!
//! A crate of its own, on top of `mapdata`, until the builder uses it (S5): inside `mapdata` its plain item names (`tick`, `now`,
//! `Clip`, ...) were reached by the grid's source digest (`mapdata-srchash` reaches by name), which renamed every built grid. The
//! builder's step moves it into `mapdata` (the grid key moves there anyway).
//!
//! - [`ClientBody`]: the client's body. Its [`ClientBody::tick`] is the client's order in a frame (`docs/notes/walk_blend.md`,
//!   "The order in a frame", PROVEN): the step ending at t moves by the blend of the inputs up to t - 50 ms, capped by the keys of
//!   input t (`max_speed` 0x2DC010 asks whether the body sprints after input t), turned by the yaw the body had before the step
//!   before (counted: the walk-truth tables, below); then the controller's step with the crouch key and the sprint flag of input t.
//!   The controller owns the feet, their height included: no lattice, no floor of a grid, no fall of its own.
//! - [`WalkBlend`]: the animation mixer's walk, as far as it is read: a new key set lays a new movement layer on at weight 0 that
//!   rises linearly to 1 over 0.3 s (6 steps, weights at the END of the tick: k / 6 on step k; 0x3B0CD0, 0x395360, 0x921804); a
//!   second change nests over the layers still ramping (0x3A4580 / 0x3A4490); the move is the plain weighted sum (0x3B1B60); and
//!   the sprint start's group rate blend (below).
//! - [`ClientBody::at_rest`], [`ClientBody::from_saved`]: the two ways a body starts (a settled rest on a floor point; a world's
//!   saved controller, `serialize` 0x7F7F0).
//! - [`RunIn`]: a run-in to a jump press as a key script, played through the same body.
//! - [`verdict`]: one rule each for "rests" and "lands on".
//!
//! Measured (`bots/tests/motion_fidelity.rs` `walk_truth`, 248,784 traced steps of four game traces; the body-physics
//! BASELINE.md R3): the reckoning's mixer (`kin::Blend` with `Mixer::Layers`, the same layers without the rate blend) is the game's
//! walk within 0.1 mm on 11 % of the steps after a forward-to-sprint change; with the rate blend here, 62 %, and no other class of
//! change moves (prototype `body-physics/scripts/walkmix_v1.py`, the same rules).
//!
//! Not read yet, so not here (each is a step of the exe's mixer that leaves the walk up to a few mm off on the steps it touches):
//! - the mid-frame bakes at an animation event (`need_new_transform` 0x3A0260): an interval's end of any clip of the body bakes
//!   the blend so far; one step of a blend up to about 4 % off. Needs each clip's phase.
//! - the group's rate blend for other members than forward-to-sprint (side clips' intervals are 11 and 13 frames: the ratio
//!   depends on the phase);
//! - the aim change's two nested rate ramps (they need the weapon's aim time);
//! - side and back jumps (their clips are not ported: [`Unported`]).

use mapdata::controller::Collision;
use mapdata::controller::jump::{self, Flight, FlightStep, Flying, Leg, Press};
use mapdata::controller::speed;
use mapdata::controller::tick::{Controller, CrouchInput, LandingGround, Saved, Settings, StepNotes, TickInput, WallNormal};
use mapdata::Vec3;

/// A step, seconds (the game's physics step).
pub const DT: f32 = jump::DT;

/// The client's walk-tick settings: the live console variables (`startup.cfg` of 0.26g, [`Settings::LIVE`]) with the wall normal
/// by rays (`cc_wall_normal_via_ray`, 0x89B80 read instruction by instruction, A325 P1), as the grid's controller passes use them
/// with `Body::exe_collision` (`grid.rs` `Body::controller`).
pub const SETTINGS: Settings = Settings { wall_normal: WallNormal::Rays, ..Settings::LIVE };

/// The world a body walks in: one walk tick of its controller ([`Controller::tick`], 50 ms, the client's settings) and one step of a
/// forward jump ([`Flying::step`]). [`Client`] is the game's collision with [`SETTINGS`]; a layer that holds its world elsewhere (the
/// walk grid's client walker) gives its own, with the same settings.
pub trait Walker {
    fn step(&self, c: &mut Controller, input: TickInput) -> StepNotes;
    fn fly(&self, c: &mut Controller, flying: &mut Flying, heading: (f32, f32), air: (f32, f32)) -> FlightStep;
}

/// The game's collision walked with the client's settings ([`SETTINGS`]).
#[derive(Clone, Copy)]
pub struct Client<'w> {
    pub world: &'w Collision,
}

impl Walker for Client<'_> {
    fn step(&self, c: &mut Controller, input: TickInput) -> StepNotes {
        c.tick(self.world, DT, &SETTINGS, input)
    }

    fn fly(&self, c: &mut Controller, flying: &mut Flying, heading: (f32, f32), air: (f32, f32)) -> FlightStep {
        flying.step(self.world, &SETTINGS, c, heading, air)
    }
}

/// The input's action bits the body reads (`apply_new_input` 0x2D86E0's key word; the server's `drive::key`).
pub mod keys {
    pub const FORWARD: u32 = 0x1;
    pub const BACK: u32 = 0x2;
    pub const LEFT: u32 = 0x4;
    pub const RIGHT: u32 = 0x8;
    pub const JUMP: u32 = 0x10;
    pub const FIRE: u32 = 0x20;
    pub const AIM: u32 = 0x80;
    pub const CROUCH: u32 = 0x100;
    pub const SPRINT: u32 = 0x200;
}

/// A velocity in the body's frame: (ahead, to the left), m/s.
pub type Local = [f32; 2];

/// Which way the keys move the body against its facing (a forward diagonal counts as forward, a back diagonal as back).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    Forward,
    Side,
    Back,
}

/// The movement clip the keys select: none (on site), or a gait and heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clip {
    OnSite,
    Run(Heading),
    Sprint,
    Crouch(Heading),
    Aimed(Heading),
    CrouchAimed(Heading),
}

/// Whether `keys` hold the client's sprint state: SPRINT and FORWARD, none of BACK, FIRE, CROUCH, AIM (`sprint_predicate` 0x2FBC90,
/// `is_trying_to_sprint` 0x2FC1F0; AIM ends it, 0x2FBD65; A364 P10, `docs/notes/sprint_state.md`). The sprint's other gates
/// (`can_sprint` 0x2DBF40: a reload, the aim's way out, a stagger, the legs, stamina) are the caller's: it hands keys without
/// SPRINT.
pub fn sprints(k: u32) -> bool {
    k & keys::SPRINT != 0 && k & keys::FORWARD != 0 && k & (keys::BACK | keys::FIRE | keys::CROUCH | keys::AIM) == 0
}

/// The clip `k` selects.
pub fn clip_of(k: u32) -> Clip {
    let ahead = i8::from(k & keys::FORWARD != 0) - i8::from(k & keys::BACK != 0);
    let left = i8::from(k & keys::LEFT != 0) - i8::from(k & keys::RIGHT != 0);
    if ahead == 0 && left == 0 {
        return Clip::OnSite;
    }
    if sprints(k) {
        return Clip::Sprint;
    }
    let heading = match ahead {
        1 => Heading::Forward,
        -1 => Heading::Back,
        _ => Heading::Side,
    };
    match (k & keys::CROUCH != 0, k & keys::AIM != 0) {
        (false, false) => Clip::Run(heading),
        (false, true) => Clip::Aimed(heading),
        (true, false) => Clip::Crouch(heading),
        (true, true) => Clip::CrouchAimed(heading),
    }
}

/// The body's speed in a clip at the speed factor 1, m/s: the config's desired speed times the clip's root speed over its default
/// (`docs/notes/walk_blend.md`, "The movement clips' own speeds"; exactness F5, PROVEN on traced steady steps). Side: the left
/// strafe's (a crouched strafe to the right is 2.0837: not told apart here, as in `bots::kin`).
pub fn clip_speed(clip: Clip) -> f32 {
    let of = |s: [f32; 3], h: Heading| match h {
        Heading::Forward => s[0],
        Heading::Side => s[1],
        Heading::Back => s[2],
    };
    match clip {
        Clip::OnSite => 0.0,
        Clip::Sprint => SPRINT_MS,
        Clip::Run(h) => of([3.5, 2.4933, 3.0], h),
        Clip::Aimed(h) => of([3.0, 1.9947, 2.5], h),
        Clip::Crouch(h) => of([2.5, 2.0143, 1.95], h),
        Clip::CrouchAimed(h) => of([2.0, 1.5107, 1.4625], h),
    }
}

/// The steady sprint at the factor 1, m/s (5.0 x 5.4545 / 5.5: [`clip_speed`]).
pub const SPRINT_MS: f32 = jump::SPRINT_MS;

/// The velocity `k` asks for in the body's frame at the speed factor `level`.
pub fn target_local(k: u32, level: f32) -> Local {
    let ahead = f32::from(i8::from(k & keys::FORWARD != 0) - i8::from(k & keys::BACK != 0));
    let left = f32::from(i8::from(k & keys::LEFT != 0) - i8::from(k & keys::RIGHT != 0));
    if ahead == 0.0 && left == 0.0 {
        return [0.0, 0.0];
    }
    let speed = clip_speed(clip_of(k));
    let n = ahead.hypot(left);
    [ahead / n * speed * level, left / n * speed * level]
}

/// `set_physics_controller_walk_vector` 0x2D9060: the flat walk cut at `max_speed` 0x2DC010 (the sprint's 5.0 x F while input
/// `cap_keys` sprints, else the walk's 3.5 x F), once, on the blended sum (PROVEN, `walk_blend.md`).
pub fn capped(local: Local, cap_keys: u32, level: f32) -> Local {
    use speed::{Gait, Heading as H, Stance};
    let gait = if sprints(cap_keys) { Gait::Sprint } else { Gait::Move { stance: Stance::Stand, heading: H::Forward, aiming: false } };
    let cap = speed::cap(gait, level);
    let len = local[0].hypot(local[1]);
    if len > cap { [local[0] * cap / len, local[1] * cap / len] } else { local }
}

/// The ground direction a body faces at `yaw` (map frame, x and z): `(-sin, cos)`, as `bots::kin::forward`.
pub fn forward(yaw: f32) -> [f32; 2] {
    let (s, c) = yaw.sin_cos();
    [-s, c]
}

/// The world velocity (x, z) of `local` for a body at `yaw`.
pub fn to_world(local: Local, yaw: f32) -> [f32; 2] {
    let [fx, fz] = forward(yaw);
    let [lx, lz] = [-fz, fx];
    [local[0] * fx + local[1] * lx, local[0] * fz + local[1] * lz]
}

/// The steps a movement transition takes (0.3 s, 0x921804).
pub const BLEND_STEPS: u32 = 6;
/// Layers ramping at once at most (a seventh change collapses the stack into its blend so far).
const RAMPING: usize = 7;

/// One ramping layer: its velocity, its clip and the steps since it was laid on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layer {
    local: Local,
    clip: Clip,
    step: u32,
}

/// The time-synchronised group's rate blend after a forward-to-sprint change: the run and sprint movements are one group
/// (+4Ch = 0, 0x336093, 0x33670A); a member joining starts in phase with the old leader and the group's time scale goes from the old
/// rate to the new over the transition's interpolator, advanced in 10 ms pieces and read at each piece's end (0x39FD70, 0x3B1450):
/// u = (k - 0.4) / 6 over 7 steps (`new_time_scale` 0x3A28D0). A member's scale is its play rate over its interval (frames): the
/// run's 12 at 3.5 / 3.5, the sprint's 9 at 5.0 / 5.5, a ratio of 1.2121; each member plays at its own rate times s(u) / its own
/// scale. PROVEN for the sprint start: the shares 0.1198 0.2784 0.4417 0.6098 0.7827 0.9603 of the change over 210 traced starts
/// (`walk_blend.md`, "Counted"), which [`WalkBlend`] reproduces ([`tests`]).
#[derive(Clone, Copy, Debug, PartialEq)]
struct SprintRamp {
    k: u32,
}

/// The run's and the sprint's group scales (play rate over interval frames).
/// The run's play rate is 1 (desired 3.5 over default 3.5).
const RUN_SCALE: f32 = 1.0 / 12.0;
const SPRINT_SCALE: f32 = (5.0 / 5.5) / 9.0;

impl SprintRamp {
    /// The group's scale now.
    fn scale(self) -> f32 {
        let u = if self.k == 0 { 0.0 } else { ((self.k as f32 - 0.4) / 6.0).clamp(0.0, 1.0) };
        RUN_SCALE + (SPRINT_SCALE - RUN_SCALE) * u
    }

    /// The factor a layer of `clip` plays at.
    fn factor(self, clip: Clip) -> f32 {
        match clip {
            Clip::Run(Heading::Forward) => self.scale() / RUN_SCALE,
            Clip::Sprint => self.scale() / SPRINT_SCALE,
            _ => 1.0,
        }
    }

    fn done(self) -> bool {
        self.k > BLEND_STEPS
    }
}

/// The mixer's walk velocity (the body's frame): a base at full weight under up to seven ramping layers, the newest on top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkBlend {
    base: (Local, Clip),
    ramping: [Layer; RAMPING],
    len: usize,
    sprint: Option<SprintRamp>,
}

impl WalkBlend {
    /// Already at `local` in `clip`, nothing ramping.
    pub fn at(local: Local, clip: Clip) -> Self {
        Self { base: (local, clip), ramping: [Layer { local: [0.0, 0.0], clip: Clip::OnSite, step: 0 }; RAMPING], len: 0, sprint: None }
    }

    /// At rest.
    pub fn still() -> Self {
        Self::at([0.0, 0.0], Clip::OnSite)
    }

    fn top(&self) -> (Local, Clip) {
        if self.len == 0 { self.base } else { (self.ramping[self.len - 1].local, self.ramping[self.len - 1].clip) }
    }

    /// The walk now: each layer from the top takes its ramp's share (step / 6) of what the layers above leave, the base the rest;
    /// a plain weighted sum (0x3B1B60).
    pub fn now(&self) -> Local {
        let mut v = [0.0f32; 2];
        let mut rest = 1.0f32;
        let factor = |clip: Clip| self.sprint.map_or(1.0, |r| r.factor(clip));
        for layer in self.ramping[..self.len].iter().rev() {
            let w = rest * (layer.step as f32 / BLEND_STEPS as f32).min(1.0);
            let f = factor(layer.clip);
            v[0] += w * layer.local[0] * f;
            v[1] += w * layer.local[1] * f;
            rest -= w;
        }
        let f = factor(self.base.1);
        v[0] += rest * self.base.0[0] * f;
        v[1] += rest * self.base.0[1] * f;
        v
    }

    /// One step toward `target` in `clip`: a new target lays a new layer on (at weight 0 before this step's advance); then every
    /// layer advances a step and a layer at full weight becomes the base. The walk at the step's end.
    pub fn tick(&mut self, target: Local, clip: Clip) -> Local {
        let (top_local, top_clip) = self.top();
        if target != top_local || clip != top_clip {
            self.sprint = (clip == Clip::Sprint && top_clip == Clip::Run(Heading::Forward)).then_some(SprintRamp { k: 0 });
            if self.len == RAMPING {
                *self = Self::at(self.now(), top_clip);
            }
            self.ramping[self.len] = Layer { local: target, clip, step: 0 };
            self.len += 1;
        }
        for layer in &mut self.ramping[..self.len] {
            layer.step = layer.step.saturating_add(1);
        }
        if let Some(r) = self.sprint.as_mut() {
            r.k += 1;
        }
        let out = self.now();
        if let Some(full) = (0..self.len).rev().find(|&i| self.ramping[i].step >= BLEND_STEPS) {
            let l = self.ramping[full];
            self.base = (l.local, l.clip);
            self.ramping.copy_within(full + 1..self.len, 0);
            self.len -= full + 1;
        }
        if self.sprint.is_some_and(SprintRamp::done) {
            self.sprint = None;
        }
        out
    }
}

/// What the game's logic hands a body each 50 ms input: the action bits and the body's yaw before this step (the trace's pre-step
/// basis: a world's `yaw`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub keys: u32,
    pub yaw: f32,
}

/// The body's own terms: its speed factor `F` (`max(0, 1 + S4)`, [`speed`]) and whether both legs are broken (the damage model's
/// +388h == 2: the logic asks for the crouch whatever the keys, 0x2FBDF0 / 0x2FBE50).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kin {
    pub level: f32,
    pub legs_broken: bool,
}

impl Kin {
    /// The kit factor 1, sound legs.
    pub const CLIENT: Self = Self { level: 1.0, legs_broken: false };
}

/// One step of a body, for the reader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    pub feet: Vec3,
    /// The client's own "on the ground" after the step (`on_ground` 0x7EF90).
    pub on_ground: bool,
    /// The step's fall found no ground under the capsule (`step_down` 0x7E1B0).
    pub airborne: bool,
    /// In a jump (+2C70Ch) after the step.
    pub in_jump: bool,
    /// The ground the step's fall stopped on, if it was a landing.
    pub landing: Option<LandingGround>,
    /// The flat walk the step asked for (m over the step, map frame).
    pub walk: [f32; 2],
}

/// A press the body cannot fly: its clip is not ported (a side or back jump).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unported;

/// The client's body: its kept controller (the only owner of its feet), the mixer's walk, the input the next step moves by and the
/// yaw it turns by, and a forward jump under way.
#[derive(Clone, Copy, PartialEq)]
pub struct ClientBody {
    controller: Controller,
    blend: WalkBlend,
    /// Input t - 50: its keys move the next step (`walk_blend.md`, "The order in a frame").
    moving: Input,
    /// The yaw the step before began at: the next step's walk is turned by it (walk-truth: keys t - 50 and this yaw are the game's
    /// walk on 63-95 % of steady steps within 0.1 mm, the step's own yaw on 2-4 %).
    walk_yaw: f32,
    /// On the ground, in a forward jump, or on the step after one's landing.
    phase: Phase,
}

/// Where a [`ClientBody`] is in a forward jump.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// No jump: the walk is the keys' blend.
    Ground,
    /// A forward jump under way.
    Flying(Flying),
    /// The step after a flight's landing (the landing substate, still in the jump state): it walks at the flight's last travel
    /// straight ahead in `clip`, whatever its input; the keys' blend runs from it on the step after (pr-review-opus, #2561: the first
    /// ground step after 17 of 17 recorded short landings moves within 0.06 m/s of the landing step's travel, forward held or let go).
    Landing { travel: f32, clip: Clip },
}

impl std::fmt::Debug for ClientBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBody")
            .field("controller", &self.controller)
            .field("blend", &self.blend)
            .field("moving", &self.moving)
            .field("walk_yaw", &self.walk_yaw)
            .field("phase", &self.phase)
            .finish()
    }
}

impl ClientBody {
    /// A body whose controller is `controller`, at rest in its blend, with no input yet.
    pub fn of(controller: Controller, yaw: f32) -> Self {
        Self { controller, blend: WalkBlend::still(), moving: Input { keys: 0, yaw }, walk_yaw: yaw, phase: Phase::Ground }
    }

    /// A world's body: the controller it saved (`Controller::deserialized`, the client's own state: crouched where it is), the
    /// mixer replayed from the inputs before it (oldest first; the last is the world's own input, which moves the step after it).
    /// A jump the world shows under way is not replayed: its flight is the caller's.
    pub fn from_saved(saved: &Saved, before: &[Input], kin: Kin) -> Self {
        let yaw = before.last().map_or(0.0, |i| i.yaw);
        let mut body = Self::of(Controller::deserialized(saved), yaw);
        for input in before {
            body.blend.tick(target_local(body.moving.keys, kin.level), clip_of(body.moving.keys));
            body.walk_yaw = body.moving.yaw;
            body.moving = *input;
        }
        body
    }

    /// A body put on the floor point `feet` as [`Self::at_rest`] puts it (a standing controller [`PUT_OVER_M`] over it, one small
    /// downward speed), not yet come to rest: its own steps settle it.
    pub fn placed(feet: Vec3, yaw: f32) -> Self {
        let mut c = Controller::standing_at(feet + Vec3::new(0.0, PUT_OVER_M, 0.0));
        c.velocity = Vec3::new(0.0, -0.01, 0.0);
        Self::of(c, yaw)
    }

    /// A body come to rest on the floor point `feet` (`rest_floors::settle`'s rule, 52 of 53 against the exe rig): a standing
    /// controller 5 mm over it, one small downward speed (so `can_skip_player_step` 0x7C7E0 does not skip the first step), then
    /// steps with no walk until it has been on the ground for [`verdict::STILL_STEPS`] steps running, each moving under
    /// [`verdict::STILL_M`]. None when it has not in [`REST_STEPS_MAX`] steps.
    pub fn at_rest(world: &impl Walker, feet: Vec3, yaw: f32) -> Option<Self> {
        let mut c = Self::placed(feet, yaw).controller;
        let mut still = 0;
        for _ in 0..REST_STEPS_MAX {
            let before = c.feet();
            let input = TickInput { walk: Vec3::ZERO, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
            world.step(&mut c, input);
            still = if c.on_ground(&SETTINGS) && (c.feet() - before).length() < verdict::STILL_M { still + 1 } else { 0 };
            if still >= verdict::STILL_STEPS {
                return Some(Self::of(c, yaw));
            }
        }
        None
    }

    /// The feet.
    pub fn feet(&self) -> Vec3 {
        self.controller.feet()
    }

    /// The kept controller (read only: its state is this body's).
    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    /// The controller block a world would save (`serialize` 0x7F7F0).
    pub fn save(&self) -> Saved {
        let c = &self.controller;
        Saved {
            feet: c.feet(),
            jumping: c.jumping,
            input_crouch: c.input_crouch,
            logic_crouch: c.want_crouch,
            capsule_crouch: c.crouched,
            can_stand: c.can_stand,
            colliding_with_player: c.colliding_with_player,
            velocity: c.velocity,
            support_normal: c.contact_normal,
            ceiling_normal: c.ceiling_normal,
            ceiling_slide: c.slide,
        }
    }

    /// The walk the next step would take with input `now` (m over the step, flat, map frame), and the blend after it: the keys of
    /// the input before, blended, capped by `now`'s keys (skipped in a jump, 0x2D915E), turned by the yaw before the step before.
    fn walk_of(&self, now: Input, kin: Kin) -> ([f32; 2], WalkBlend) {
        let mut blend = self.blend;
        let (target, clip) = match self.phase {
            Phase::Landing { travel, clip } => ([travel, 0.0], clip),
            _ => (target_local(self.moving.keys, kin.level), clip_of(self.moving.keys)),
        };
        let local = blend.tick(target, clip);
        let local = if self.controller.jumping { local } else { capped(local, now.keys, kin.level) };
        let v = to_world(local, self.walk_yaw);
        ([v[0] * DT, v[1] * DT], blend)
    }

    /// One 50 ms step of the body with the input stamped at its end (`now`): the walk ([`Self::walk_of`]), then the controller's
    /// step with `now`'s crouch key and sprint flag (`apply_new_input` writes them before the step; `step_forward_impl` 0x7D33D
    /// reads +204h). A forward jump under way flies its own step instead ([`Flying::step`]: the clip's root motion and the air
    /// control of `now`'s keys, heading by the input before).
    pub fn tick(&mut self, world: &impl Walker, kin: Kin, now: Input) -> Tick {
        let (walk, blend) = self.walk_of(now, kin);
        let (notes, walk) = match &mut self.phase {
            Phase::Flying(flying) => {
                let sign = |a: u32, b: u32| i8::from(now.keys & a != 0) - i8::from(now.keys & b != 0);
                let air = Flying::air_of(flying.press(), sign(keys::FORWARD, keys::BACK), sign(keys::RIGHT, keys::LEFT));
                // The step's heading is the yaw after the input before it (`apply_rotation` 0x2D8AD0 turns the body by an input
                // only after the step it ends: rig rule 7, 1,392 of 1,392 air steps, as `bots::body::FlownJump::step` takes it).
                let f = forward(self.moving.yaw);
                let step: FlightStep = world.fly(&mut self.controller, flying, (f[0], f[1]), air);
                let landed = flying.landed();
                if landed {
                    // The ground walk goes on from the clip's last travel (`jump::fly_as`'s ground steps), first a step at it
                    // whatever the keys ([`Phase::Landing`]).
                    let travel = flying.last_travel();
                    let clip = clip_of(now.keys);
                    self.blend = WalkBlend::at([travel, 0.0], clip);
                    self.phase = Phase::Landing { travel, clip };
                }
                let notes = StepNotes { airborne: !step.on_ground, ..StepNotes::default() };
                (notes, [0.0, 0.0])
            }
            Phase::Ground | Phase::Landing { .. } => {
                self.blend = blend;
                self.phase = Phase::Ground;
                let input = TickInput {
                    walk: Vec3::new(walk[0], 0.0, walk[1]),
                    air_control: Vec3::ZERO,
                    crouch: CrouchInput::Tested { key: now.keys & keys::CROUCH != 0, legs_broken: kin.legs_broken },
                    sprinting: sprints(now.keys),
                };
                (world.step(&mut self.controller, input), walk)
            }
        };
        self.walk_yaw = self.moving.yaw;
        self.moving = now;
        Tick {
            feet: self.controller.feet(),
            on_ground: self.controller.on_ground(&SETTINGS),
            airborne: notes.airborne,
            in_jump: self.controller.jumping,
            landing: notes.landing,
            walk,
        }
    }

    /// A forward jump pressed now, flown from the next step: the short jump from a walk at `speed` (the body's last flat travel,
    /// player+334h), or the sprint jump off `leg` at the first air frame `first_air_frame` when `keys` sprint and `speed` is at
    /// the walk cap or over (the client's speed test, 0x33705A). The flight's walk cap is `kin`'s. A side or back press is
    /// [`Unported`].
    pub fn press_forward(&mut self, keys: u32, speed: f32, kin: Kin, leg: Leg, first_air_frame: f32) -> Result<(), Unported> {
        if keys & keys::FORWARD == 0 || keys & (keys::BACK | keys::LEFT | keys::RIGHT) != 0 {
            return Err(Unported);
        }
        let cap = jump::WALK_CAP_MS * kin.level;
        let press = if sprints(keys) && speed >= cap { Press::Sprint { speed, leg, first_air_frame } } else { Press::Walk { speed } };
        self.phase = Phase::Flying(Flying::new(press, Flight { cap, dy: 0.0 }));
        Ok(())
    }

    /// A forward jump pressed `steps_done` steps ago and still under way, for a body made from a world inside it
    /// ([`Self::from_saved`] keeps the world's controller, mid-flight as the client has it): the press as [`Self::press_forward`]
    /// takes it, its flight already flown `steps_done` steps. A flight's step input is its press's and its step number's alone (the
    /// clip's root motion, `jump()` and `end_jump()` at fixed steps), so those steps are flown on a copy of the controller, which is
    /// dropped; the body's own controller is not stepped. A side or back press is [`Unported`].
    pub fn resume_jump(
        &mut self,
        world: &impl Walker,
        press: (u32, f32),
        kin: Kin,
        (leg, first_air_frame): (Leg, f32),
        steps_done: u32,
    ) -> Result<(), Unported> {
        self.press_forward(press.0, press.1, kin, leg, first_air_frame)?;
        if let Phase::Flying(flying) = &mut self.phase {
            let mut scratch = self.controller;
            let f = forward(self.moving.yaw);
            for _ in 0..steps_done {
                world.fly(&mut scratch, flying, (f[0], f[1]), (0.0, 0.0));
            }
            if flying.landed() {
                let travel = flying.last_travel();
                self.phase = Phase::Landing { travel, clip: clip_of(self.moving.keys) };
            }
        }
        Ok(())
    }

    /// Whether a forward jump is under way.
    pub fn flying(&self) -> bool {
        matches!(self.phase, Phase::Flying(_))
    }
}

/// How far over a floor point [`ClientBody::at_rest`] puts the body, m.
pub const PUT_OVER_M: f32 = 0.005;
/// The steps [`ClientBody::at_rest`] gives a body to come to rest (2 s).
pub const REST_STEPS_MAX: u32 = 40;

/// The gait of a run-in to a press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunGait {
    /// Walking forward: `ramp` steps from rest (0 a standing jump; 6 or more at the walk cap), then the short jump.
    Walk { ramp: u32 },
    /// Sprinting forward for `steps` steps from rest, then the sprint jump off `leg` at `first_air_frame` (a short jump when the
    /// body is under the walk cap at the press, 0x33705A).
    Sprint { steps: u32, leg: Leg, first_air_frame: f32 },
}

/// A run-in to a forward jump press as the client plays it: a body at rest at a floor point, the forward key (and the sprint) held
/// along `yaw` for the gait's steps, the jump pressed, the forward key held in the air, let go on the landing, then
/// [`verdict::STAY_STEPS`] steps. The one run-in definition for the builder's stored presses and the motor's walk-in (fix-builder's
/// R2, DESIGN §3.3a).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunIn {
    pub gait: RunGait,
    pub yaw: f32,
}

impl RunIn {
    /// The run-in played from `body` (at rest): each step's [`Tick`], the run-in's first. Err when the press is unported.
    pub fn play(self, world: &impl Walker, mut body: ClientBody, kin: Kin) -> Result<Vec<Tick>, Unported> {
        let (held, steps) = match self.gait {
            RunGait::Walk { ramp } => (keys::FORWARD, ramp),
            RunGait::Sprint { steps, .. } => (keys::FORWARD | keys::SPRINT, steps),
        };
        let mut out = Vec::new();
        let input = |k| Input { keys: k, yaw: self.yaw };
        let mut last = body.feet();
        let mut speed = 0.0;
        // The keys go down on the first input; the step after it is the first that moves (the order in a frame).
        out.push(body.tick(world, kin, input(held)));
        for _ in 0..steps {
            let t = body.tick(world, kin, input(held));
            let f = t.feet;
            speed = Vec3::new(f.x - last.x, 0.0, f.z - last.z).length() / DT;
            last = f;
            out.push(t);
        }
        let (leg, frame) = match self.gait {
            RunGait::Sprint { leg, first_air_frame, .. } => (leg, first_air_frame),
            RunGait::Walk { .. } => (Leg::Left, jump::SPRINT_FIRST_AIR_FRAMES[0]),
        };
        body.press_forward(held, speed, kin, leg, frame)?;
        while body.flying() {
            out.push(body.tick(world, kin, input(keys::FORWARD)));
        }
        for _ in 0..verdict::STAY_STEPS {
            out.push(body.tick(world, kin, input(0)));
        }
        Ok(out)
    }
}

/// The one rule each for what a body did. These are not client rules: they are this project's tolerances, held in one place, set
/// from the S0 counts (BASELINE.md R8: of the census's 91 exe rests, the motor's band agrees with the census verdict on 78, a 1.0 m
/// radius on 54-58; the exe's rests after a stored press lie 1.0-1.5 m from the stored landing point in 24).
pub mod verdict {
    use super::Tick;
    use mapdata::Vec3;

    /// A body is still when its feet move under this in a step, m.
    pub const STILL_M: f32 = 1e-3;
    /// ... for this many steps running, on the ground.
    pub const STILL_STEPS: u32 = 4;
    /// The steps after a landing a flight is followed for (the builder's `jump::STAY_STEPS`).
    pub const STAY_STEPS: u32 = mapdata::controller::jump::STAY_STEPS;
    /// A landing's rest lies within this of the landing point (flat), m.
    pub const LANDS_FLAT_M: f32 = 1.5;

    /// Whether the body rests at the end of `ticks`: on the ground, moving under [`STILL_M`] a step, for the last
    /// [`STILL_STEPS`] steps.
    pub fn rests(ticks: &[Tick]) -> bool {
        let n = STILL_STEPS as usize;
        ticks.len() > n
            && ticks.windows(2).rev().take(n).all(|w| w[1].on_ground && (w[1].feet - w[0].feet).length() < STILL_M)
    }

    /// A landing's rest may lie this far under the landing point, at most, m (and at least [`UP_MIN_M`], half the rise between).
    pub const UP_UNDER_M: f32 = 0.35;
    /// ... at least this far, m.
    pub const UP_MIN_M: f32 = 0.1;

    /// Whether feet `feet` are up on a link from `take_off` to `landing`: past half the way, within [`LANDS_FLAT_M`] flat of the
    /// landing and no lower than `landing.y - min(UP_UNDER_M, max(rise / 2, UP_MIN_M))`. The one verdict of a press's rest: the
    /// motor's `is_up`, the press forecast's and [`lands_on`]'s end.
    pub fn up(feet: Vec3, take_off: Vec3, landing: Vec3) -> bool {
        let (dx, dz) = (landing.x - take_off.x, landing.z - take_off.z);
        let past = (feet.x - take_off.x) * dx + (feet.z - take_off.z) * dz >= 0.5 * (dx * dx + dz * dz);
        let rise = landing.y - take_off.y;
        past && Vec3::new(feet.x - landing.x, 0.0, feet.z - landing.z).length() <= LANDS_FLAT_M
            && feet.y >= landing.y - UP_UNDER_M.min((0.5 * rise).max(UP_MIN_M))
    }

    /// Whether a flight from `take_off` lands on `landing`: every step from the first on the ground after its first jump step is on
    /// the ground (it does not skim), and its last step is [`up`] on the link.
    pub fn lands_on(ticks: &[Tick], take_off: Vec3, landing: Vec3) -> bool {
        let Some(first_air) = ticks.iter().position(|t| t.in_jump) else { return false };
        let Some(down) = ticks[first_air..].iter().position(|t| !t.in_jump && t.on_ground).map(|i| i + first_air) else { return false };
        if !ticks[down..].iter().all(|t| t.on_ground) {
            return false;
        }
        up(ticks[ticks.len() - 1].feet, take_off, landing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{Triangle, WalkerBehaviour};

    fn floor() -> Collision {
        let p = |x, z| Vec3::new(x, 0.0, z);
        let t = |a, b, c| Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope };
        Collision::build(&[t(p(-30.0, -30.0), p(30.0, -30.0), p(30.0, 30.0)), t(p(-30.0, -30.0), p(30.0, 30.0), p(-30.0, 30.0))])
    }

    /// Clean starts: k / 6 of the speed on step k (`walk_blend.md`: 28 traced starts, median error 0.0000 m/s).
    #[test]
    fn a_start_from_rest_ramps_k_sixths() {
        let mut b = WalkBlend::still();
        let target = target_local(keys::FORWARD, 1.0);
        for k in 1..=8 {
            let v = b.tick(target, clip_of(keys::FORWARD))[0];
            let want = 3.5 * (k.min(6) as f32) / 6.0;
            assert!((v - want).abs() < 1e-5, "step {k}: {v} against {want}");
        }
    }

    /// Clean stops: 1 - k / 6.
    #[test]
    fn a_stop_ramps_down_k_sixths() {
        let mut b = WalkBlend::at(target_local(keys::FORWARD, 1.0), clip_of(keys::FORWARD));
        for k in 1..=6 {
            let v = b.tick([0.0, 0.0], Clip::OnSite)[0];
            assert!((v - 3.5 * (1.0 - k as f32 / 6.0)).abs() < 1e-5, "step {k}: {v}");
        }
    }

    /// The sprint start, the group's rate blend: the shares of the change the traced starts give (`walk_blend.md`, "Counted": 210
    /// starts, 0.1198 0.2784 0.4417 0.6098 0.7827 0.9603), within their own rounding; and the sprint's own speed from step 7.
    #[test]
    fn a_sprint_start_follows_the_groups_rate_blend() {
        let run = target_local(keys::FORWARD, 1.0);
        let sprint = target_local(keys::FORWARD | keys::SPRINT, 1.0);
        let mut b = WalkBlend::at(run, clip_of(keys::FORWARD));
        let shares = [0.1198, 0.2784, 0.4417, 0.6098, 0.7827, 0.9603];
        for (k, want) in shares.iter().enumerate() {
            let v = b.tick(sprint, Clip::Sprint)[0];
            let share = (v - run[0]) / (sprint[0] - run[0]);
            assert!((share - want).abs() < 5e-4, "step {}: share {share} against {want}", k + 1);
        }
        let v = b.tick(sprint, Clip::Sprint)[0];
        assert!((v - SPRINT_MS).abs() < 1e-4, "{v}");
        // Without the group (a plain layer), step 1 would be 1/6 of the change: the rate blend is what the traces show.
        assert!((shares[0] - 1.0 / 6.0f32).abs() > 0.04);
    }

    /// A second change during a blend nests: the forward layer keeps its own ramp under the left one.
    #[test]
    fn a_second_change_nests_over_the_first() {
        let (fwd, left) = (target_local(keys::FORWARD, 1.0), target_local(keys::LEFT, 1.0));
        let mut b = WalkBlend::still();
        b.tick(fwd, clip_of(keys::FORWARD));
        b.tick(fwd, clip_of(keys::FORWARD));
        let v = b.tick(left, clip_of(keys::LEFT));
        // The left layer at 1/6; the forward layer at 3/6 of the rest.
        assert!((v[0] - 3.5 * 3.0 / 6.0 * 5.0 / 6.0).abs() < 1e-5 && (v[1] - left[1] / 6.0).abs() < 1e-5, "{v:?}");
    }

    /// The cap is input t's: on the step that ends a sprint's release the walk is cut to the walk cap at once while the blend still
    /// carries the sprint (`walk_blend.md`: 192 of 201 releases at the walk cap).
    #[test]
    fn a_sprint_release_is_capped_by_the_newest_input() {
        let sprint = target_local(keys::FORWARD | keys::SPRINT, 1.0);
        let v = capped(sprint, keys::FORWARD, 1.0);
        assert!((v[0] - 3.5).abs() < 1e-5, "{v:?}");
        assert_eq!(capped(sprint, keys::FORWARD | keys::SPRINT, 1.0), sprint);
    }

    /// A body put on a flat floor comes to rest on it, and a forward walk from rest moves by the blend: nothing on its first step
    /// (its keys move the step after), then k / 6 of the walk cap a step, along its yaw.
    #[test]
    fn a_body_at_rest_walks_off_by_the_blend() {
        let floor = floor();
        let world = Client { world: &floor };
        let mut body = ClientBody::at_rest(&world, Vec3::new(0.0, 0.0, 0.0), 0.0).expect("it rests on a flat floor");
        assert!(body.feet().y.abs() < 1e-3, "{:?}", body.feet());
        let input = Input { keys: keys::FORWARD, yaw: 0.0 };
        let first = body.tick(&world, Kin::CLIENT, input);
        assert_eq!(first.walk, [0.0, 0.0], "the input's keys move the step after it");
        let mut z = body.feet().z;
        for k in 1..=8u32 {
            let t = body.tick(&world, Kin::CLIENT, input);
            let moved = t.feet.z - z;
            z = t.feet.z;
            let want = 3.5 * (k.min(6) as f32) / 6.0 * DT;
            assert!((moved - want).abs() < 2e-3, "step {k}: {moved} against {want}");
            assert!(t.on_ground && !t.airborne);
        }
    }

    /// A body saved and made again from its block stands where it stood, with the same flags.
    #[test]
    fn a_saved_body_comes_back_where_it_was() {
        let floor = floor();
        let world = Client { world: &floor };
        let body = ClientBody::at_rest(&world, Vec3::new(1.0, 0.0, 2.0), 0.3).expect("rests");
        let saved = body.save();
        let back = ClientBody::from_saved(&saved, &[Input { keys: 0, yaw: 0.3 }], Kin::CLIENT);
        assert!((back.feet() - body.feet()).length() < 1e-5);
        assert_eq!(back.save(), saved);
    }

    /// A short jump run in on a flat floor lands, stays and rests: the flight's own verdicts.
    #[test]
    fn a_run_in_on_a_flat_floor_lands_and_rests() {
        let floor = floor();
        let world = Client { world: &floor };
        let body = ClientBody::at_rest(&world, Vec3::new(0.0, 0.0, -10.0), 0.0).expect("rests");
        let start = body.feet();
        let ticks = RunIn { gait: RunGait::Walk { ramp: 6 }, yaw: 0.0 }.play(&world, body, Kin::CLIENT).expect("forward");
        assert!(ticks.iter().any(|t| t.in_jump), "it jumped");
        let end = ticks[ticks.len() - 1].feet;
        assert!(end.z > start.z + 2.0, "{end:?}");
        assert!(verdict::lands_on(&ticks, start, Vec3::new(end.x, 0.0, end.z)), "lands where it ended");
        assert!(!verdict::lands_on(&ticks, start, Vec3::new(end.x, 2.0, end.z + 5.0)), "not on a top 2 m up and 5 m on");
    }

    /// A flight resumed from a world inside it ([`ClientBody::resume_jump`]) flies on exactly as the flight it was cut from: the same
    /// feet to the bit on every step to the landing and after, for a short and a sprint jump cut at each step.
    #[test]
    fn a_resumed_flight_flies_on_as_the_unbroken_one() {
        let floor = floor();
        let world = Client { world: &floor };
        for (keys, speed, landing) in [(keys::FORWARD, 2.0, 17u32), (keys::FORWARD | keys::SPRINT, 5.5, 19)] {
            let start = ClientBody::at_rest(&world, Vec3::new(0.0, 0.0, -10.0), 0.0).expect("rests");
            let input = Input { keys, yaw: 0.0 };
            for cut in 1..landing {
                let mut whole = start;
                whole.press_forward(keys, speed, Kin::CLIENT, Leg::Left, 6.57).expect("forward");
                for _ in 0..cut {
                    whole.tick(&world, Kin::CLIENT, input);
                }
                // A world at the cut: its controller block and its inputs before, the flight resumed.
                let saved = whole.save();
                let mut resumed = ClientBody::from_saved(&saved, &[input, input], Kin::CLIENT);
                resumed.blend = whole.blend;
                resumed.resume_jump(&world, (keys, speed), Kin::CLIENT, (Leg::Left, 6.57), cut).expect("forward");
                assert_eq!(resumed.flying(), whole.flying(), "keys {keys:#x} cut {cut}");
                for step in 0..24 {
                    let (a, b) = (whole.tick(&world, Kin::CLIENT, input), resumed.tick(&world, Kin::CLIENT, input));
                    assert_eq!((a.feet, a.in_jump, a.on_ground), (b.feet, b.in_jump, b.on_ground), "keys {keys:#x} cut {cut} step {step}");
                }
            }
        }
    }

    /// [#2561] The step after a flight's landing walks at the flight's last travel whatever its keys; the keys' blend runs from it
    /// on the step after: forward held, a sixth of the way to the walk cap.
    #[test]
    fn the_step_after_a_landing_walks_at_the_flights_travel() {
        let floor = floor();
        let world = Client { world: &floor };
        let mut body = ClientBody::at_rest(&world, Vec3::new(0.0, 0.0, -10.0), 0.0).expect("rests");
        let input = Input { keys: keys::FORWARD, yaw: 0.0 };
        body.press_forward(keys::FORWARD, 1.0, Kin::CLIENT, Leg::Left, 6.57).expect("forward");
        let mut travel = None;
        while body.flying() {
            body.tick(&world, Kin::CLIENT, input);
            if let Phase::Landing { travel: t, .. } = body.phase {
                travel = Some(t);
            }
        }
        let travel = travel.expect("landed");
        let m1 = body.tick(&world, Kin::CLIENT, input).walk;
        assert!((m1[1] / DT - travel).abs() < 1e-4 && m1[0].abs() < 1e-5, "m1 {m1:?} against the travel {travel}");
        let m2 = body.tick(&world, Kin::CLIENT, input).walk;
        let want = travel + (target_local(keys::FORWARD, 1.0)[0] - travel) / 6.0;
        assert!((m2[1] / DT - want).abs() < 1e-4, "m2 {m2:?} against {want}");
    }

    /// A side press is not flown (its clip is not ported).
    #[test]
    fn a_side_press_is_unported() {
        let floor = floor();
        let world = Client { world: &floor };
        let mut body = ClientBody::at_rest(&world, Vec3::ZERO, 0.0).expect("rests");
        assert_eq!(body.press_forward(keys::LEFT, 2.0, Kin::CLIENT, Leg::Left, 6.57), Err(Unported));
        assert!(!body.flying());
    }
}
