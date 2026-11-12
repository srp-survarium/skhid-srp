//! The client's walk tick, ported from the 0.26g exe (A313; the readings are in `private/grid/controller/dis/notes-*.md`).
//!
//! `player_step` 0x7BDA0's walk branch: the capsule is cut from below by the step (`step_up` 0x7C900), swept along the tick's
//! move and slid along what it meets (`step_forward` 0x7D590 / `step_forward_impl` 0x7D2A0, the wall slide 0x7AC50), then swept
//! down onto the ground (`step_down` 0x7E1B0 with `step_down_sweep_test_impl` 0x7EA80), where ground steeper than the limit
//! that lifted the feet is refused unless one of the two slides takes the body (0x7D9C0, 0x7DA50). The origin then moves by
//! the step's share of the tick. Float operations keep the exe's order where the notes give it; the x87 square roots are
//! taken in f32 (the game's x87 control word is 0x007F, 24-bit precision: `docs/notes/collision_world.md` section 6). The
//! air branch (a body in a jump, `step_jump_up` / `forward` / `down`) and the low-ceiling slide 0x7DF20 are ported too.
//!
//! The sweeps cast [`Shape::CapsuleAt`] (the controller's own centre and dims) by the way from the start's origin to the
//! end's, so that [`super::Caster::Exe`] (the exe's own GJK and time of impact, bit for bit) gets the exe's numbers: with
//! it the step is the exe's bits in 991 of 1,000 traced steps of the game on equal triangles
//! (`docs/notes/exactness_rig.md` section 3b). The crouch flags are the exe's four (section 3a).
//!
//! The via-ray wall normal (0x89B80) is read and ported ([`WallNormal::Rays`]); [`Settings::LIVE`] still takes the first
//! stand-in ([`WallNormal::FaceMadeHorizontal`]) until the nav gate. What is LIKELY rather than read: the stand-up and slide
//! sweeps (0x86BF0, 0x88CB0, read in outline: capsule sweeps).
// The exe's comparisons are ported as written: `!(a > b)` and `a > b` differ on NaN, as `comiss` branches do.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
use super::{Collision, Contact, Shape, SmallRound, Surface};
use crate::WalkerBehaviour;
use crate::math::Vec3;

const UP: Vec3 = Vec3::Y;
/// `length² too small` (0x9CBA48).
const TINY: f32 = 1e-5;

/// The live console variables (startup.cfg of 0.26g; the exe's defaults differ for several).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub step_height: f32,
    pub max_slope_cos: f32,
    pub idle_max_slope_cos: f32,
    pub on_ground_max_vertical_speed: f32,
    pub additional_down_speed: f32,
    pub min_walk_speed: f32,
    pub wall_slide: bool,
    pub wall_slide_bouncing_factor: f32,
    pub wall_full_slide_dot: f32,
    pub move_step_margin: f32,
    pub stand_move_complex_test: bool,
    pub crouch_move_complex_test: bool,
    pub prevent_slope_acceleration: bool,
    pub impassable_slope_side_slide: bool,
    pub impassable_slope_down_slide: bool,
    pub impassable_slope_down_slide_offset: f32,
    pub slope_slide_bouncing_factor: f32,
    pub slide_coefficient: f32,
    pub already_sliding_on_slope_dp: f32,
    pub ground_normal_via_ray_offset: f32,
    pub max_vertical_speed_to_can_jump: f32,
    pub low_ceiling_slide: bool,
    pub low_ceiling_slide_bouncing_factor: f32,
    /// How the move sweep's callback finds a wall's normal when the move meets it nearly head-on (`cc_wall_normal_via_ray`).
    pub wall_normal: WallNormal,
}

/// The move sweep's wall normal for a nearly head-on hit (over `wall_full_slide_dot`), callback 0x895F0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WallNormal {
    /// The face's own normal, made horizontal, toward the body: the port's first stand-in (LIKELY, A313).
    #[default]
    FaceMadeHorizontal,
    /// The exe's 0x89B80 with `ray_pick` 0x898C0 (dis/notes-raynormal.md, read instruction by instruction; A325 P1: walk
    /// wall-slide divergences from the exe over 5 mm 11 -> 5 of 235 sampled ticks): rays along the move beside the hit point
    /// (`cc_wall_normal_via_ray_offset` 0.03 m), the less head-on horizontal normal of the faces they meet.
    Rays,
}

impl Settings {
    /// The live game's (`startup.cfg` in `resources.db`, extracted 2026-11-05): the file's numbers as written.
    #[allow(clippy::approx_constant)]
    pub const LIVE: Self = Self {
        step_height: 0.80,
        max_slope_cos: 0.57358,
        idle_max_slope_cos: 0.70711,
        on_ground_max_vertical_speed: 2.0,
        additional_down_speed: 0.05,
        min_walk_speed: 3.0,
        wall_slide: true,
        wall_slide_bouncing_factor: 0.080,
        wall_full_slide_dot: 0.50,
        move_step_margin: 0.040,
        stand_move_complex_test: true,
        crouch_move_complex_test: false,
        prevent_slope_acceleration: true,
        impassable_slope_side_slide: true,
        impassable_slope_down_slide: true,
        impassable_slope_down_slide_offset: 0.0,
        slope_slide_bouncing_factor: 0.50,
        slide_coefficient: 1.0,
        already_sliding_on_slope_dp: 0.20,
        ground_normal_via_ray_offset: 0.050,
        max_vertical_speed_to_can_jump: 4.0,
        low_ceiling_slide: true,
        low_ceiling_slide_bouncing_factor: 0.40,
        wall_normal: WallNormal::FaceMadeHorizontal,
    };
}

/// One body's controller state (the fields of `bullet_character_controller` the walk tick reads and writes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controller {
    /// The collision object's origin (+0xB0): the capsule's centre between steps.
    pub origin: Vec3,
    /// The working position (+0x40): the capsule's centre during a step.
    pub pos: Vec3,
    /// +0x2C770.
    pub velocity: Vec3,
    /// +0x2C710: the ground normal of the last landing.
    pub contact_normal: Vec3,
    /// +0x20: the walk vector the game set for this update (a displacement over `dt`).
    pub walk: Vec3,
    /// +0x2C730: the ceiling-slide velocity.
    pub slide: Vec3,
    /// (radius, half height of the straight part) now (+0x1D0).
    pub dims: (f32, f32),
    pub crouched: bool,
    /// (diameter, height) standing (+0x60) and crouched (+0x68).
    pub standing: (f32, f32),
    pub crouching: (f32, f32),
    /// +0x2C704, +0x2C708.
    pub tick: f32,
    pub gravity: f32,
    /// +0x2C70C: in a jump (the game's `base_player::jump` sets it, `end_jump` clears it): the air branch moves the body.
    pub jumping: bool,
    /// +0x200 `m_input_is_in_crouch`: the crouch key as the game's input has it (`base_player::apply_new_input` 0x2D86E0
    /// writes it before the step). While it is held the step's tail makes no stand test and no ceiling push, and a body at
    /// rest skips its step only when the capsule agrees with the key. Set it with [`Controller::crouch_key`].
    pub input_crouch: bool,
    /// +0x201 `m_logic_is_in_crouch`: the game's logic asks for a crouch (player_step crouches, or stands when it may).
    pub want_crouch: bool,
    /// +0x202 `m_can_stand`: a crouched body may stand (the tail's stand test, made only with the key released).
    pub can_stand: bool,
    /// +0x2C720: the ceiling normal of the last stand test.
    pub ceiling_normal: Vec3,
    /// +0x30: the air control vector the game sets in a jump (a speed, m/s).
    pub air_control: Vec3,
}

/// What one step did, for the reader.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepNotes {
    pub skipped: bool,
    pub forward_hit: bool,
    pub landed: bool,
    pub airborne: bool,
    pub refused: bool,
    /// Refused because the body could not straighten under something (the low-ceiling slide 0x7DF20 is not ported).
    pub ceiling: bool,
    pub side_slide: bool,
    pub down_slide: bool,
}

fn dims_of((w, h): (f32, f32)) -> (f32, f32) {
    (w * 0.5, (h - w) * 0.5)
}

fn len2(v: Vec3) -> f32 {
    (v.y * v.y + v.z * v.z) + v.x * v.x
}

/// `normalized_safe` 0x7AA50.
fn normalized_safe(v: Vec3, fallback: Vec3) -> Vec3 {
    let l2 = len2(v);
    if l2.abs() < TINY {
        return fallback;
    }
    let inv = 1.0 / l2.sqrt();
    Vec3::new(v.x * inv, inv * v.y, inv * v.z)
}

/// `cc_wall_normal_via_ray_offset` (0xA7AAF8, live 0.03).
const WALL_RAY_OFFSET: f32 = 0.03;

/// 0x89B80 for a map face (the root shape no sphere, capsule or multisphere): `n2` the horizontal unit wall normal, `dir` the
/// move's unit direction, `from` the sweep's start, `hp` the hit point. Not head-on: one ray shifted 0.03 toward where the move
/// meets the hit point's height; head-on: two rays shifted 0.03 either way along the wall, the second against the first's
/// result. Each ray starts 0.5 `dir` before its point and is `dir` long.
fn wall_normal_via_ray(world: &Collision, n2: Vec3, dir: Vec3, from: Vec3, hp: Vec3) -> Vec3 {
    let off = WALL_RAY_OFFSET;
    let c = (dir.z * -n2.z + dir.y * -n2.y) + dir.x * -n2.x;
    let start = |u: Vec3, sign: f32| {
        Vec3::new((hp.x + u.x * off * sign) - dir.x * 0.5, (hp.y + u.y * off * sign) - dir.y * 0.5, (hp.z + u.z * off * sign) - dir.z * 0.5)
    };
    let end = |s: Vec3| Vec3::new(dir.x + s.x, dir.y + s.y, dir.z + s.z);
    if !(0.001 > (c - 1.0).abs()) {
        let (dx, dy, dz) = (hp.x - from.x, hp.y - from.y, hp.z - from.z);
        let k = (UP.z * dz + UP.y * dy) + UP.x * dx;
        let p = Vec3::new(UP.x * k + from.x, from.y + UP.y * k, from.z + UP.z * k);
        let e = Vec3::new(hp.x - p.x, hp.y - p.y, hp.z - p.z);
        let l = ((e.z * e.z + e.y * e.y) + e.x * e.x).sqrt();
        let si = 1.0 / c;
        let v = Vec3::new(((l * dir.x) * si + p.x) - hp.x, ((dir.y * l) * si + p.y) - hp.y, ((dir.z * l) * si + p.z) - hp.z);
        let s = start(normalized_safe(v, Vec3::ZERO), 1.0);
        ray_pick(world, s, end(s), n2, dir)
    } else {
        let w = Vec3::new(n2.z * UP.y - n2.y * UP.z, UP.z * n2.x - UP.x * n2.z, UP.x * n2.y - UP.y * n2.x);
        let u = normalized_safe(w, Vec3::ZERO);
        let s1 = start(u, 1.0);
        let r1 = ray_pick(world, s1, end(s1), n2, dir);
        let s2 = start(u, -1.0);
        ray_pick(world, s2, end(s2), r1, dir)
    }
}

/// `ray_pick` 0x898C0: the ray's face normal made horizontal when it is less head-on than `prev`; a miss keeps `prev`.
fn ray_pick(world: &Collision, a: Vec3, b: Vec3, prev: Vec3, dir: Vec3) -> Vec3 {
    let Some((_, rn)) = world.ray(a, b) else { return prev };
    let k = (UP.z * rn.z + UP.y * rn.y) + UP.x * rn.x;
    let m = normalized_safe(Vec3::new(rn.x - UP.x * k, rn.y - UP.y * k, rn.z - UP.z * k), -dir);
    let before = (dir.z * prev.z + dir.y * prev.y) + prev.x * dir.x;
    let now = (dir.y * m.y + dir.z * m.z) + dir.x * m.x;
    if now > before { m } else { prev }
}

/// 0x84E20: `n` tilted about the horizontal to just walkable (`cos_max + 0.001`); a normal along up as it is.
fn limit_walkable(n: Vec3, s: &Settings) -> Vec3 {
    let h = Vec3::new(n.x, 0.0, n.z);
    let hl2 = h.dot(h);
    if hl2 < 1e-12 {
        return n;
    }
    let c = s.max_slope_cos + 0.001;
    UP * c + h * ((1.0 - c * c) / hl2).sqrt()
}

/// `get_wall_slide_vector` 0x7AC50 with the live factors.
fn wall_slide(m: Vec3, n: Vec3, s: &Settings) -> Vec3 {
    let l2 = (m.y * m.y + m.z * m.z) + m.x * m.x;
    let inv = 1.0 / l2.sqrt();
    let u = Vec3::new(m.x * inv, inv * m.y, inv * m.z);
    let bf = s.wall_slide_bouncing_factor;
    let c = (n.z * u.z + n.y * u.y) + n.x * u.x;
    let r = Vec3::new((u.x - n.x * c) + n.x * bf, (u.y - n.y * c) + n.y * bf, (u.z - n.z * c) + n.z * bf);
    let inv2 = 1.0 / ((r.z * r.z + r.y * r.y) + r.x * r.x).sqrt();
    let a = c.abs();
    let t = s.wall_full_slide_dot;
    let k = if t > a { 1.0 } else { (1.0 - a) / (1.0 - t) };
    let l = l2.sqrt();
    Vec3::new(((r.x * inv2) * l) * k, ((r.y * inv2) * l) * k, ((r.z * inv2) * l) * k)
}

impl Controller {
    /// A standing body whose feet are at `feet`, with the 0.26g player's capsule (0.76 m wide, 1.8 m tall; crouched 1.1 m).
    pub fn standing_at(feet: Vec3) -> Self {
        let standing = (0.76, 1.8);
        let dims = dims_of(standing);
        let origin = feet + UP * (dims.0 + dims.1);
        Self {
            origin,
            pos: origin,
            velocity: Vec3::ZERO,
            contact_normal: UP,
            walk: Vec3::ZERO,
            slide: Vec3::ZERO,
            dims,
            crouched: false,
            standing,
            crouching: (0.76, 1.1),
            tick: 0.05,
            gravity: 9.8,
            jumping: false,
            input_crouch: false,
            want_crouch: false,
            can_stand: true,
            // The exe's constructor starts it at -up (Unicorn).
            ceiling_normal: Vec3::new(0.0, -1.0, 0.0),
            air_control: Vec3::ZERO,
        }
    }

    /// The feet (the capsule's bottom) at the origin.
    pub fn feet(&self) -> Vec3 {
        self.origin - UP * (self.dims.0 + self.dims.1)
    }

    fn d(&self) -> f32 {
        self.dims.0 + self.dims.1
    }

    /// The capsule at `centre` as the exe's sweeps cast it: its own dims (+0x1D0), the cast's origin.
    fn capsule_at(&self, centre: Vec3) -> Shape {
        let (radius, half_height) = self.dims;
        Shape::CapsuleAt { centre, radius, half_height }
    }

    /// `need_slide_on_supporting_surface` 0x7BC20.
    fn need_slide(&self, s: &Settings) -> bool {
        let c = (self.contact_normal.y * UP.y + self.contact_normal.z * UP.z) + self.contact_normal.x * UP.x;
        s.max_slope_cos > c || (s.idle_max_slope_cos > c && len2(self.walk).abs() < TINY)
    }

    /// `get_slide_acceleration` 0x7BA30 with `get_slope_slide_vector` 0x7AF00.
    fn slide_acceleration(&self, s: &Settings) -> Vec3 {
        if !self.need_slide(s) {
            return Vec3::ZERO;
        }
        let n = self.contact_normal;
        let dn = -UP;
        let e = (n.y * dn.y + n.z * dn.z) + n.x * dn.x;
        let dh = normalized_safe(Vec3::new(dn.x - n.x * e, dn.y - n.y * e, dn.z - n.z * e), dn);
        let inv = 1.0 / ((dh.z * UP.z + dh.y * UP.y) + dh.x * UP.x).abs();
        let c = (n.y * UP.y + n.z * UP.z) + n.x * UP.x;
        let eh = normalized_safe(Vec3::new(n.x - UP.x * c, n.y - UP.y * c, n.z - UP.z * c), Vec3::ZERO);
        let (g, bf) = (self.gravity, s.slope_slide_bouncing_factor);
        Vec3::new((dh.x * inv + eh.x * bf) * g, (dh.y * inv + eh.y * bf) * g, (dh.z * inv + eh.z * bf) * g) * s.slide_coefficient
    }

    /// `get_corrected_walk_vector` 0x7B840.
    fn corrected_walk(&self, s: &Settings) -> Vec3 {
        let w = self.walk;
        if len2(w).abs() < TINY {
            return w;
        }
        let n = self.contact_normal;
        if !(s.max_slope_cos > (n.y * UP.y + n.z * UP.z) + n.x * UP.x) {
            return w;
        }
        let dn = ((n.x * UP.x) + (n.z * UP.z)) + n.y * UP.y;
        let nh = Vec3::new(n.x - UP.x * dn, n.y - UP.y * dn, n.z - UP.z * dn);
        let inv = 1.0 / ((nh.x * nh.x + nh.z * nh.z) + nh.y * nh.y).sqrt();
        let hn = nh * inv;
        if 0.0 > ((w.z * hn.z) + (w.y * hn.y)) + (hn.x * w.x) { wall_slide(w, hn, s) } else { w }
    }

    /// One `player_step` of `dt` seconds (the walk branch; a body in a jump is not handled). Returns what it did.
    pub fn step(&mut self, world: &Collision, dt: f32, s: &Settings) -> StepNotes {
        let mut notes = StepNotes::default();
        if self.want_crouch {
            if !self.crouched {
                self.set_crouched(true);
            }
        } else if self.crouched && self.can_stand {
            self.set_crouched(false);
        }
        let v = self.velocity;
        let vy = (v.y * UP.y + v.z * UP.z) + v.x * UP.x;
        let slow = s.on_ground_max_vertical_speed > vy.abs();
        let start = self.origin;
        self.pos = start;
        let foot = start - UP * self.d();
        if self.jumping {
            self.air_step(world, dt, s);
            notes.airborne = true;
            return notes;
        }
        // can_skip_player_step 0x7C7E0.
        if len2(self.walk).abs() < TINY
            && len2(v) == 0.0
            && !self.need_slide(s)
            && !self.ceiling_push_due()
            && self.crouched == self.input_crouch
            && self.slide == Vec3::ZERO
        {
            notes.skipped = true;
            self.walk = Vec3::ZERO;
            return notes;
        }
        let vv = ((v.x * UP.x) + (v.z * UP.z)) + v.y * UP.y;
        let vh = Vec3::new(v.x - vv * UP.x, v.y - vv * UP.y, v.z - vv * UP.z);
        let w = self.walk;
        let wv = ((w.x * UP.x) + (w.z * UP.z)) + w.y * UP.y;
        self.walk = Vec3::new(w.x - wv * UP.x, w.y - wv * UP.y, w.z - wv * UP.z);
        let w = self.walk;
        let s2 = ((w.x * w.x + w.y * w.y) + w.z * w.z) / (dt * dt);
        let mut k = 1.0f32;
        if !(s2.abs() < TINY) && (s.min_walk_speed * s.min_walk_speed > s2) {
            let kk = s.min_walk_speed / s2.sqrt();
            k = kk;
            self.walk = Vec3::new(w.x * kk, w.y * kk, w.z * kk);
        }
        let cw = self.corrected_walk(s);
        let acc = self.slide_acceleration(s);
        let tick = self.tick;
        let a1 = Vec3::new(acc.x * tick, acc.y * tick, acc.z * tick);
        let av = ((a1.y * UP.y) + (a1.z * UP.z)) + a1.x * UP.x;
        let ah = Vec3::new(a1.x - av * UP.x, a1.y - av * UP.y, a1.z - av * UP.z);
        let iv = 1.0 / dt;
        let sl = self.slide;
        let m = Vec3::new(
            tick * ((((cw.x * iv) + vh.x) + ah.x) + sl.x),
            (sl.y + (((cw.y * iv) + vh.y) + ah.y)) * tick,
            (sl.z + (((cw.z * iv) + vh.z) + ah.z)) * tick,
        );
        let rise = self.step_up(s);
        let p0 = self.pos;
        notes.forward_hit = self.step_forward(world, m, s);
        self.step_down(world, rise, slow, foot, m, p0, a1, dt, s, &mut notes);
        // The origin from the step's share of the tick.
        let al = dt / tick;
        let p = self.pos;
        if k == 1.0 {
            let be = 1.0 - al;
            self.origin = Vec3::new((al * p.x) + (be * start.x), (p.y * al) + (start.y * be), (p.z * al) + (start.z * be));
        } else {
            let pv = ((p.x * UP.x) + (p.z * UP.z)) + p.y * UP.y;
            let ph = p - UP * pv;
            let sv = ((start.y * UP.y) + (start.z * UP.z)) + start.x * UP.x;
            let sh = start - UP * sv;
            let ahk = al / k;
            let bh = 1.0 - ahk;
            let h = Vec3::new(sh.x * bh + ph.x * ahk, sh.y * bh + ph.y * ahk, sh.z * bh + ph.z * ahk);
            let pv2 = ((p.y * UP.y) + (p.z * UP.z)) + UP.x * p.x;
            let be = 1.0 - al;
            self.origin = Vec3::new(
                (((sv * UP.x) * be) + ((pv2 * UP.x) * al)) + h.x,
                (((sv * UP.y) * be) + ((pv2 * UP.y) * al)) + h.y,
                (((sv * UP.z) * be) + ((pv2 * UP.z) * al)) + h.z,
            );
        }
        // 0x7B7F0: floor(x * 1000 + 0.5) * 0.001 (proven against the exe in Unicorn).
        let mm = |x: f32| (x * 1000.0 + 0.5).floor() * 0.001;
        self.pos = Vec3::new(mm(self.pos.x), mm(self.pos.y), mm(self.pos.z));
        self.contact_normal = Vec3::new(mm(self.contact_normal.x), mm(self.contact_normal.y), mm(self.contact_normal.z));
        self.walk = Vec3::ZERO;
        notes
    }

    /// What the game's logic does with the crouch key between two steps (PROVEN: read in the exe, and counted on 27,790
    /// traced steps of the game's own bodies with 113 presses and 112 releases, `docs/notes/exactness_rig.md`):
    /// `base_player::apply_new_input` 0x2D86E0 writes the key into +0x200; `player_logic_crouch_state` asks for the crouch
    /// (0x337810: +0x201 = 1) and gives it up only once the body may stand (0x337830: +0x201 = 0 when +0x202 is set). The
    /// stand test is made by a step with the key released, so a body stands on the SECOND step after the release, 50 ms
    /// after the key, and stays down under a ceiling.
    ///
    /// Call it before every step with the key's state. Not here (the logic's side, not read): the game refusing a crouch
    /// with the key held (36 of the traced steps: +0x200 set, +0x201 clear) and `base_player::jump` 0x2D7920 clearing +0x201.
    pub fn crouch_key(&mut self, held: bool) {
        self.input_crouch = held;
        if held {
            self.want_crouch = true;
        } else if self.can_stand {
            self.want_crouch = false;
        }
    }

    /// 0x7BD10 (and the same test inline in `step_down`'s tail 0x7E88A and at the head of the push 0x7BAD0): a crouched
    /// body with the key released that may not stand, under a ceiling whose normal is over 60 degrees off straight down,
    /// is pushed out from under it. The three places add the products in different orders; with up = (0, 1, 0) each sum
    /// is the same bits.
    fn ceiling_push_due(&self) -> bool {
        let c = self.ceiling_normal;
        self.crouched && !self.input_crouch && !self.can_stand && 0.5 > (-c.y * UP.y + -c.z * UP.z) + -c.x * UP.x
    }

    /// `setup_crouch_state` 0x7F1A0: the crouched or standing capsule, its feet kept (the origin moves).
    fn set_crouched(&mut self, crouched: bool) {
        let feet = self.origin - UP * self.d();
        self.crouched = crouched;
        self.dims = dims_of(if crouched { self.crouching } else { self.standing });
        self.can_stand = !crouched;
        self.origin = feet + UP * self.d();
    }

    /// The tail's stand test `can_straighten` 0x7F7B0 (read whole: a crouched capsule is tested by the stand tester at
    /// +0x144A0, sweep 0x873C0, from the collision object's origin +0xB0; the sweep itself is LIKELY, read in outline): the
    /// crouched capsule swept up by what standing adds; a face met is the ceiling, its normal kept.
    fn stand_test(&mut self, world: &Collision) -> bool {
        // From the origin (where the body was at the tick's start), proven in Unicorn; a miss leaves -up.
        let rise = self.standing.1 - self.crouching.1;
        match self.up_sweep(world, self.origin, rise) {
            Some(n) => {
                self.ceiling_normal = n;
                false
            }
            None => {
                self.ceiling_normal = -UP;
                true
            }
        }
    }

    /// `side_slide_on_low_ceiling` 0x7DF20 (PROVEN in the reading): the walk move turned off the ceiling or the ground, the
    /// less vertical of the two, and slid as on an impassable slope. `n` in/out.
    #[allow(clippy::too_many_arguments)]
    fn low_ceiling_slide(&mut self, world: &Collision, w: Vec3, p1: Vec3, sb: Vec3, dist: f32, step: f32, n: &mut Vec3, cn: Vec3, s: &Settings) -> bool {
        if !s.low_ceiling_slide || ((w.y * w.y + w.z * w.z) + w.x * w.x).abs() < TINY {
            return false;
        }
        let a = ((cn.y * UP.y + cn.z * UP.z) + cn.x * UP.x).abs();
        let b = ((n.y * UP.y + n.z * UP.z) + n.x * UP.x).abs();
        if (a - 1.0).abs() < 1e-5 && (b - 1.0).abs() < 1e-5 {
            return false;
        }
        let m = if b > a { cn } else { *n };
        let inv = 1.0 / ((w.y * w.y + w.z * w.z) + w.x * w.x).sqrt();
        let bf = s.low_ceiling_slide_bouncing_factor;
        let q = Vec3::new(m.x - (w.x * inv) * bf, m.y - (w.y * inv) * bf, m.z - (w.z * inv) * bf);
        let mut n2 = normalized_safe(q, m);
        let r = self.side_slide_core(world, w, p1, sb, dist, step, &mut n2, s);
        *n = n2;
        r
    }

    /// `end_jump` 0x7F050: out of jump mode, keeping only a downward vertical speed.
    pub fn end_jump(&mut self) {
        self.jumping = false;
        let v = self.velocity;
        let mut vv = (v.y * UP.y + v.z * UP.z) + v.x * UP.x;
        if !(0.0 > vv) {
            vv = 0.0;
        }
        self.velocity = UP * vv;
    }

    /// `on_ground` 0x7EF90 (the game's "can jump"): standing, under 4 m/s vertical, on a walkable contact.
    pub fn on_ground(&self, s: &Settings) -> bool {
        if self.crouched {
            return false;
        }
        let v = self.velocity;
        let vv = (v.y * UP.y + v.z * UP.z) + v.x * UP.x;
        if !(s.max_vertical_speed_to_can_jump > vv.abs()) {
            return false;
        }
        let n = self.contact_normal;
        let d = (n.y * UP.y + n.z * UP.z) + n.x * UP.x;
        !(s.max_slope_cos > d)
    }

    /// The closest contact of the full capsule at `from` moved by `by`: the air sweepers (proven in Unicorn, 2026-11-05). Up
    /// and down (0x885C0, callback 0x88380) skip a rim or wall-side hit (its point not inside the capsule's disc and its normal
    /// across the axis, as the down sweep's 0x846E0); across (0x87B60, callback 0x87B40) keeps every contact.
    fn air_sweep(&self, world: &Collision, from: Vec3, by: Vec3, axis: bool) -> Option<(f32, Vec3)> {
        // The cast's way is the end's origin less the start's (0x72EB0), not the displacement the end was made with.
        let by = (from + by) - from;
        let r = self.dims.0;
        let mut best: Option<Contact> = None;
        world.sweep_each(self.capsule_at(from), by, |c| {
            if axis {
                let at = from + by * c.t;
                let (hx, hz) = (c.point.x - at.x, c.point.z - at.z);
                if !(hx * hx + hz * hz < r * r - 1e-5) && c.normal.y.abs() < 0.001 {
                    return;
                }
            }
            if best.is_none_or(|b| c.t < b.t) {
                best = Some(c);
            }
        });
        best.map(|c| (c.t, c.normal))
    }

    /// The AIR branch of `player_step` (0x7BF2C): no gravity; the vertical part of the walk vector (the jump clip's root
    /// motion) and the air control move the body: up, then across (sliding on walls), then down.
    fn air_step(&mut self, world: &Collision, dt: f32, s: &Settings) {
        let start = self.origin;
        let tick = self.tick;
        let inv = 1.0 / dt;
        let w = self.walk;
        let a = self.air_control;
        let sv = ((w.y * UP.y + w.z * UP.z) + w.x * UP.x) * inv;
        self.velocity = UP * sv;
        let m = Vec3::new(tick * ((inv * w.x) + a.x), (a.y + w.y * inv) * tick, (a.z + w.z * inv) * tick);
        let vert = (m.y * UP.y + m.z * UP.z) + m.x * UP.x;
        let mut hm = Vec3::new(m.x - vert * UP.x, m.y - vert * UP.y, m.z - vert * UP.z);
        if vert > 0.0 && !(vert.abs() < TINY) {
            // step_jump_up 0x7CFC0.
            let pos = self.pos;
            let t = pos + UP * vert;
            self.pos = match self.air_sweep(world, pos, UP * vert, true) {
                Some((f, _)) => Vec3::new(pos.x * (1.0 - f) + t.x * f, pos.y * (1.0 - f) + t.y * f, pos.z * (1.0 - f) + t.z * f),
                None => t,
            };
        }
        if !(((hm.x * hm.x + hm.z * hm.z) + hm.y * hm.y).abs() < TINY) {
            // step_jump_forward 0x7CB90.
            let pos = self.pos;
            let t = Vec3::new(hm.x + pos.x, pos.y + hm.y, pos.z + hm.z);
            match self.air_sweep(world, pos, hm, false) {
                None => self.pos = t,
                Some((f, n)) => {
                    let dn = (n.y * UP.y + n.z * UP.z) + UP.x * n.x;
                    let h = Vec3::new(n.x - UP.x * dn, n.y - UP.y * dn, n.z - UP.z * dn);
                    let h = normalized_safe(h, Vec3::new(1.0, 0.0, 0.0));
                    if !s.wall_slide {
                        self.pos = Vec3::new(pos.x * (1.0 - f) + t.x * f, pos.y * (1.0 - f) + t.y * f, pos.z * (1.0 - f) + t.z * f);
                    } else {
                        hm = wall_slide(hm, h, s);
                        if !(((hm.z * hm.z + hm.y * hm.y) + hm.x * hm.x).abs() < TINY) {
                            let t2 = pos + hm;
                            self.pos = match self.air_sweep(world, pos, hm, false) {
                                Some((f2, _)) => Vec3::new(pos.x * (1.0 - f2) + t2.x * f2, pos.y * (1.0 - f2) + t2.y * f2, pos.z * (1.0 - f2) + t2.z * f2),
                                None => t2,
                            };
                        }
                    }
                }
            }
        }
        if 0.0 > vert && !(vert.abs() < TINY) {
            // step_jump_down 0x7D0F0.
            let pos = self.pos;
            let t = pos + UP * vert;
            match self.air_sweep(world, pos, UP * vert, true) {
                Some((f, n)) => {
                    self.pos = Vec3::new((t.x * f) + ((1.0 - f) * pos.x), (t.y * f) + (pos.y * (1.0 - f)), (t.z * f) + (pos.z * (1.0 - f)));
                    self.contact_normal = n;
                }
                None => {
                    self.pos = t;
                    self.contact_normal = -UP;
                }
            }
        }
        let al = dt / tick;
        let be = 1.0 - al;
        let p = self.pos;
        self.origin = Vec3::new((be * start.x) + (p.x * al), (p.y * al) + (start.y * be), (p.z * al) + (start.z * be));
        let mm = |x: f32| (x * 1000.0 + 0.5).floor() * 0.001;
        self.pos = Vec3::new(mm(self.pos.x), mm(self.pos.y), mm(self.pos.z));
        self.contact_normal = Vec3::new(mm(self.contact_normal.x), mm(self.contact_normal.y), mm(self.contact_normal.z));
        self.walk = Vec3::ZERO;
        self.air_control = Vec3::ZERO;
    }

    /// `step_up` 0x7C900: the capsule cut from below by the step, its top kept; the rise of its bottom.
    fn step_up(&mut self, s: &Settings) -> f32 {
        let c = self.origin;
        let sum = self.d();
        let top = Vec3::new(UP.x * sum + c.x, c.y + UP.y * sum, c.z + UP.z * sum);
        let half = s.step_height * 0.5;
        let bot = Vec3::new(c.x - UP.x * sum, c.y - UP.y * sum, c.z - UP.z * sum);
        let nh = (self.dims.1 - half).max(0.0);
        let (r, _) = self.dims;
        self.dims = dims_of((r * 2.0, (r + nh) * 2.0));
        let s2 = self.d();
        self.pos = Vec3::new(top.x - s2 * UP.x, top.y - UP.y * s2, top.z - UP.z * s2);
        let b = self.pos - UP * s2 - bot;
        (b.y * UP.y + b.z * UP.z) + b.x * UP.x
    }

    /// The move sweep (the complex test 0x85A90 standing, the simple one 0x86490 crouched) from `from` to `to`, through the
    /// callback 0x895F0: contacts behind the move at 0 are ignored, the normal is made horizontal. The fraction and normal.
    fn move_sweep(&self, world: &Collision, from: Vec3, to: Vec3, s: &Settings) -> Option<(f32, Vec3)> {
        let d = to - from;
        let l2 = (d.z * d.z + d.x * d.x) + d.y * d.y;
        let inv = 1.0 / l2.sqrt();
        let dir = Vec3::new(d.x * inv, inv * d.y, inv * d.z);
        let (r, hh) = self.dims;
        let callback = |shape: Shape, from: Vec3| -> Option<(f32, Vec3, Vec3)> {
            let mut best: Option<Contact> = None;
            world.sweep_each(shape, d, |c| {
                if c.t == 0.0 {
                    let q = c.point - from;
                    if !((dir.z * q.z + dir.y * q.y) + q.x * dir.x > 0.0) {
                        return;
                    }
                }
                if best.is_none_or(|b| c.t < b.t) {
                    best = Some(c);
                }
            });
            let c = best?;
            let k = (UP.z * c.normal.z + UP.y * c.normal.y) + UP.x * c.normal.x;
            let h = Vec3::new(c.normal.x - UP.x * k, c.normal.y - UP.y * k, c.normal.z - UP.z * k);
            let mut n2 = normalized_safe(h, -dir);
            let cc = (dir.z * -n2.z + dir.y * -n2.y) + dir.x * -n2.x;
            if cc > s.wall_full_slide_dot {
                n2 = match s.wall_normal {
                    WallNormal::FaceMadeHorizontal => match c.on {
                        Surface::Face { normal, .. } => {
                            let fnrm = if normal.dot(c.normal) < 0.0 { -normal } else { normal };
                            let kf = fnrm.y;
                            normalized_safe(Vec3::new(fnrm.x, fnrm.y - kf, fnrm.z), n2)
                        }
                        // A primitive has no face: the contact's own normal stays.
                        Surface::Primitive { .. } => n2,
                    },
                    WallNormal::Rays => wall_normal_via_ray(world, n2, dir, from, c.point),
                };
            }
            Some((c.t, n2, c.point))
        };
        let complex = if self.crouched { s.crouch_move_complex_test } else { s.stand_move_complex_test };
        if !complex {
            return callback(self.capsule_at(from), from).map(|(t, n, _)| (t, n));
        }
        let m = s.move_step_margin;
        let (fmain, nmain, pmain) = callback(Shape::Cylinder { centre: from, half: r + hh, radius: r, margin: m }, from)?;
        let hgt = (UP.z * (pmain.z - from.z) + UP.y * (pmain.y - from.y)) + UP.x * (pmain.x - from.x);
        if !(hgt > hh) {
            return Some((fmain, nmain));
        }
        // The top check 0x85F50: the top sphere, then a lower cylinder.
        let top = callback(Shape::Sphere { centre: from + UP * hh, radius: r }, from + UP * hh);
        if let Some((f1, _, _)) = top
            && f1 < 1.0
            && !(f1 > fmain)
        {
            return Some((fmain, nmain));
        }
        let lo = from - (UP * r) * 0.5;
        let low = callback(Shape::Cylinder { centre: lo, half: r * 0.5 + hh, radius: r, margin: m }, lo);
        let best = match (top, low) {
            (Some(a), Some(b)) => Some(if b.0 > a.0 { a } else { b }),
            (a, b) => a.or(b),
        };
        match best {
            Some((f, n, _)) if f < 1.0 && f > fmain => Some((f, n)),
            Some((f, _, _)) if f < 1.0 => Some((fmain, nmain)),
            _ => None,
        }
    }

    /// `step_forward` 0x7D590 and `step_forward_impl` 0x7D2A0. Whether anything was met.
    fn step_forward(&mut self, world: &Collision, m: Vec3, s: &Settings) -> bool {
        let l2 = (m.y * m.y + m.z * m.z) + m.x * m.x;
        if l2.abs() < TINY {
            return false;
        }
        let inv = 1.0 / l2.sqrt();
        let dir0 = Vec3::new(m.x * inv, inv * m.y, inv * m.z);
        let mut m = m;
        let mut met = false;
        for depth in 0..3 {
            let pos = self.pos;
            let t = Vec3::new(pos.x + m.x, pos.y + m.y, pos.z + m.z);
            let d = t - pos;
            if ((d.z * d.z + d.y * d.y) + d.x * d.x).abs() < TINY {
                return met;
            }
            let Some((f, n)) = self.move_sweep(world, pos, t, s) else {
                self.pos = t;
                return met;
            };
            met = true;
            let g = 1.0 - f;
            self.pos = Vec3::new(pos.x * g + t.x * f, pos.y * g + t.y * f, pos.z * g + t.z * f);
            let sv = wall_slide(m, n, s);
            let mut r = Vec3::new(sv.x * g, sv.y * g, sv.z * g);
            let dd = (dir0.y * r.y + dir0.z * r.z) + dir0.x * r.x;
            if 0.0 > dd {
                r = Vec3::new(r.x - dir0.x * dd, r.y - dir0.y * dd, r.z - dir0.z * dd);
            }
            if !s.wall_slide || depth == 2 || ((r.z * r.z + r.y * r.y) + r.x * r.x).abs() < TINY {
                return met;
            }
            m = r;
        }
        met
    }

    /// The down sweep 0x84FE0 with its callback 0x84420: the cut capsule from `from` along up by `dist` (negative: down).
    /// The fraction, point and normal of the hit the callback keeps.
    fn down_sweep(&self, world: &Collision, from: Vec3, dist: f32, s: &Settings) -> Option<(f32, Vec3, Vec3)> {
        let (r, hh) = self.dims;
        // The cast's way is the end's origin less the start's (0x72EB0), not `up * dist` itself.
        let by = Vec3::new(UP.x * dist + from.x, UP.y * dist + from.y, UP.z * dist + from.z) - from;
        let floor = ((UP.y * from.y + UP.z * from.z) + UP.x * from.x) - hh;
        let mut hits: Vec<Contact> = Vec::new();
        world.sweep_each(self.capsule_at(from), by, |c| hits.push(c));
        hits.sort_by(|a, b| a.t.total_cmp(&b.t));
        for c in hits {
            let at = from + by * c.t;
            // On the lower hemisphere only.
            if (UP.y * c.point.y + UP.z * c.point.z) + UP.x * c.point.x > floor + by.y * c.t {
                continue;
            }
            // A rim or wall hit: not under the capsule and a normal across up.
            let hx = c.point.x - at.x;
            let hz = c.point.z - at.z;
            if !(hx * hx + hz * hz < r * r - 1e-5) && c.normal.y.abs() < 0.001 {
                continue;
            }
            let n = match c.on.walker() {
                WalkerBehaviour::Walk => UP,
                WalkerBehaviour::DontWalk => {
                    let cm = s.max_slope_cos - 0.001;
                    let h = Vec3::new(c.normal.x, 0.0, c.normal.z);
                    let hl2 = h.dot(h);
                    if (c.normal.y - 1.0).abs() < 1e-5 || hl2 < 1e-12 {
                        c.normal
                    } else {
                        UP * cm + h * ((1.0 - cm * cm) / hl2).sqrt()
                    }
                }
                _ => {
                    let n = c.normal;
                    if (UP.y * n.y + UP.z * n.z) + UP.x * n.x < s.max_slope_cos { self.steep_ground(world, c, s) } else { n }
                }
            };
            return Some((c.t, c.point, n));
        }
        None
    }

    /// 0x84810: the normal of a steep by-slope hit, looked at again. On a small round primitive (0x8482F..0x8499A: a sphere,
    /// a capsule or a cylinder under 0.8 m of radius, [`SmallRound`]) it is tilted to just walkable (0x84E20) for a sphere and
    /// for a capsule or a cylinder that lies (`1 - upright` over the callback's +94h, which 0x85176 hands the slope limit's
    /// cosine); a capsule that stands keeps it (steep), a cylinder that stands gets the ray's look. Anything else (a face, a
    /// box, a larger round shape) gets the ray's look ([`Self::ground_relook`]).
    fn steep_ground(&self, world: &Collision, c: Contact, s: &Settings) -> Vec3 {
        let n = c.normal;
        match c.on {
            Surface::Primitive { round: Some(round), .. } => match round {
                SmallRound::Sphere => limit_walkable(n, s),
                SmallRound::Capsule { upright } | SmallRound::Cylinder { upright } if 1.0 - upright > s.max_slope_cos => limit_walkable(n, s),
                SmallRound::Capsule { .. } => n,
                SmallRound::Cylinder { .. } => self.ground_relook(world, c.point, n, s),
            },
            Surface::Face { .. } | Surface::Primitive { round: None, .. } => self.ground_relook(world, c.point, n, s),
        }
    }

    /// 0x849BB (0x84810's ray look): a steep hit looked at again by a ray along `-n`, from 0.5 m out along
    /// `n` and the ground offset over the hit point, 1 m long. No face met: the ground counts as flat (up). A face flatter than
    /// `n` and walkable: `n` tilted to just walkable (0x84E20). Else `n` as it is (steep).
    fn ground_relook(&self, world: &Collision, hp: Vec3, n: Vec3, s: &Settings) -> Vec3 {
        let start = (hp + UP * s.ground_normal_via_ray_offset) + n * 0.5;
        let Some((_, rn)) = world.ray(start, start - n) else { return UP };
        let (ry, ny) = (rn.dot(UP), n.dot(UP));
        if ry > ny && ry > s.max_slope_cos { limit_walkable(n, s) } else { n }
    }

    /// `step_down_sweep_test_impl` 0x7EA80: moves `pos`; the hit's point and normal.
    fn step_down_test(&mut self, world: &Collision, dist: f32, sb: Vec3, w2: f32, p1: Vec3, s: &Settings) -> Option<(Vec3, Vec3)> {
        let from = self.pos;
        let Some((f, hp, n)) = self.down_sweep(world, from, dist, s) else {
            self.pos = Vec3::new(UP.x * dist + from.x, UP.y * dist + from.y, UP.z * dist + from.z);
            return None;
        };
        let to = from + UP * dist;
        let g = 1.0 - f;
        let h = Vec3::new(g * from.x + to.x * f, (from.y * g) + to.y * f, (from.z * g) + to.z * f);
        let d = self.d();
        let bh = h - UP * d - sb;
        let rose = (h.y - d * UP.y) > sb.y && ((bh.z * bh.z + bh.y * bh.y) + bh.x * bh.x) > w2;
        if s.prevent_slope_acceleration && rose {
            let k = (w2 / ((bh.z * bh.z + bh.y * bh.y) + bh.x * bh.x)).sqrt();
            let f2 = Vec3::new(p1.x * (1.0 - k) + from.x * k, p1.y * (1.0 - k) + from.y * k, p1.z * (1.0 - k) + from.z * k);
            return match self.down_sweep(world, f2, dist, s) {
                Some((f, hp, n)) => {
                    let g = 1.0 - f;
                    self.pos = Vec3::new(
                        g * f2.x + (f2.x + UP.x * dist) * f,
                        f2.y * g + (UP.y * dist + f2.y) * f,
                        f2.z * g + (UP.z * dist + f2.z) * f,
                    );
                    Some((hp, n))
                }
                None => {
                    self.pos = Vec3::new(UP.x * dist + f2.x, UP.y * dist + f2.y, UP.z * dist + f2.z);
                    None
                }
            };
        }
        self.pos = Vec3::new(g * from.x + (UP.x * dist + from.x) * f, from.y * g + (UP.y * dist + from.y) * f, from.z * g + (UP.z * dist + from.z) * f);
        Some((hp, n))
    }

    /// `impassable_slope` 0x7DE10: steeper than the limit and the feet ended higher than at the start.
    fn impassable(&self, n: Vec3, sb: Vec3, s: &Settings) -> bool {
        let c = (n.y * UP.y + n.z * UP.z) + n.x * UP.x;
        if !(s.max_slope_cos > c) {
            return false;
        }
        let d = self.d();
        let p = self.pos;
        let h = (((p.y - d * UP.y) - sb.y) * UP.y + ((p.z - d * UP.z) - sb.z) * UP.z) + ((p.x - d * UP.x) - sb.x) * UP.x;
        if h.abs() < 1e-5 {
            return false;
        }
        h > 0.0
    }

    /// `can_straighten` 0x7EEB0 (the stand-up sweep 0x86BF0 taken as the cut capsule swept up by the step, LIKELY).
    fn can_straighten(&self, world: &Collision, sb: Vec3, step: f32) -> bool {
        self.straighten(world, sb, step).is_none()
    }

    /// [`Self::can_straighten`]'s ceiling: the normal of what the rise meets, if anything.
    fn straighten(&self, world: &Collision, sb: Vec3, step: f32) -> Option<Vec3> {
        let d = self.d();
        let p = self.pos;
        let h = (((p.z - d * UP.z) - sb.z) * UP.z + ((p.x - d * UP.x) - sb.x) * UP.x) + UP.y * ((p.y - d * UP.y) - sb.y);
        if !(h > 0.0) {
            return None;
        }
        self.up_sweep(world, p, step)
    }

    /// The up sweeps 0x86BF0 / 0x873C0 with their callback 0x869D0 (proven in Unicorn): the capsule at `from` swept up by
    /// `rise`; a hit counts only on the top half of the top sphere and strictly inside the radius from the axis. Its normal.
    fn up_sweep(&self, world: &Collision, from: Vec3, rise: f32) -> Option<Vec3> {
        let (r, hh) = self.dims;
        // The cast's way is the end's origin less the start's (0x72EB0).
        let by = (from + UP * rise) - from;
        let mut best: Option<Contact> = None;
        world.sweep_each(self.capsule_at(from), by, |c| {
            let top = from + by * c.t + UP * hh;
            let (hx, hz) = (c.point.x - top.x, c.point.z - top.z);
            if c.point.y < top.y || !(hx * hx + hz * hz < r * r) {
                return;
            }
            if best.is_none_or(|b| c.t < b.t) {
                best = Some(c);
            }
        });
        best.map(|c| c.normal)
    }

    /// `slide_in_impassable_case_impl` 0x7D660 (the side slide). `n` in/out.
    #[allow(clippy::too_many_arguments)]
    fn side_slide(&mut self, world: &Collision, w: Vec3, p1: Vec3, sb: Vec3, dist: f32, step: f32, n: &mut Vec3, s: &Settings) -> bool {
        if !s.impassable_slope_side_slide || ((w.x * w.x + w.y * w.y) + w.z * w.z).abs() < TINY {
            return false;
        }
        self.side_slide_core(world, w, p1, sb, dist, step, n, s)
    }

    /// `slide_in_impassable_case_impl` 0x7D660.
    #[allow(clippy::too_many_arguments)]
    fn side_slide_core(&mut self, world: &Collision, w: Vec3, p1: Vec3, sb: Vec3, dist: f32, step: f32, n: &mut Vec3, s: &Settings) -> bool {
        let c = (n.x * UP.x + n.z * UP.z) + n.y * UP.y;
        let t = Vec3::new(n.x - UP.x * c, n.y - UP.y * c, n.z - UP.z * c);
        let inv = 1.0 / ((t.z * t.z + t.y * t.y) + t.x * t.x).sqrt();
        let nh = t * inv;
        let sv = wall_slide(w, nh, s);
        let s2 = (sv.x * sv.x + sv.y * sv.y) + sv.z * sv.z;
        if s2.abs() < TINY {
            return false;
        }
        let q = p1 + sv;
        if len2(q - p1).abs() < TINY {
            self.pos = p1;
            return false;
        }
        match self.move_sweep(world, p1, q, s) {
            Some((f, _)) => self.pos = Vec3::new(p1.x * (1.0 - f) + q.x * f, p1.y * (1.0 - f) + q.y * f, p1.z * (1.0 - f) + q.z * f),
            None => self.pos = q,
        }
        let Some((_, n2)) = self.step_down_test(world, dist, sb, s2, p1, s) else { return true };
        *n = n2;
        if self.impassable(*n, sb, s) {
            return false;
        }
        self.can_straighten(world, sb, step)
    }

    /// `impassable_slope_down_slide` 0x7DA50. `n` in/out.
    #[allow(clippy::too_many_arguments)]
    fn down_slide(&mut self, world: &Collision, w: Vec3, sb: Vec3, dist: f32, n: &mut Vec3, p1: Vec3, s: &Settings) -> bool {
        if !s.impassable_slope_down_slide {
            return false;
        }
        let c = (n.x * UP.x + n.z * UP.z) + n.y * UP.y;
        let sn = (1.0 - c * c).sqrt();
        if sn.abs() < 1e-5 {
            return false;
        }
        let p = self.pos;
        let h = (((p1.y - p.y) * UP.y + (p1.z - p.z) * UP.z) + (p1.x - p.x) * UP.x) + dist;
        let off = s.impassable_slope_down_slide_offset;
        let l = (off - h) / sn;
        let bs = p + *n * off;
        let dn = -UP;
        let e = (n.z * dn.z + n.y * dn.y) + n.x * dn.x;
        let g = normalized_safe(Vec3::new(dn.x - n.x * e, dn.y - n.y * e, dn.z - n.z * e), dn);
        let de = Vec3::new(g.x * l + bs.x, g.y * l + bs.y, g.z * l + bs.z);
        let Some(c2) = world.sweep_shape(self.capsule_at(bs), de - bs, |_| true) else { return false };
        let n2 = c2.normal;
        if self.impassable(n2, sb, s) {
            return false;
        }
        let f = c2.t;
        let e = Vec3::new((1.0 - f) * bs.x + de.x * f, bs.y * (1.0 - f) + de.y * f, bs.z * (1.0 - f) + de.z * f);
        let eb = e - UP * self.d();
        if !(((w.z * (eb.z - sb.z) + w.y * (eb.y - sb.y)) + w.x * (eb.x - sb.x)) > 0.0) {
            return false;
        }
        self.pos = e;
        *n = n2;
        true
    }

    /// `step_down` 0x7E1B0 (the live exe's airborne gravity: patch 72, `g * min(dt, tick)`).
    #[allow(clippy::too_many_arguments)]
    fn step_down(&mut self, world: &Collision, step: f32, slow: bool, sb: Vec3, w: Vec3, p1: Vec3, adt: Vec3, dt: f32, s: &Settings, notes: &mut StepNotes) {
        let v = self.velocity;
        let tick = self.tick;
        let d0 = if slow {
            -s.additional_down_speed
        } else {
            (((((v.y * UP.y + v.z * UP.z) + adt.y * UP.y) + adt.z * UP.z) + v.x * UP.x) + adt.x * UP.x) * tick
        };
        let dist = d0 - step;
        let w2 = (w.x * w.x + w.y * w.y) + w.z * w.z;
        match self.step_down_test(world, dist, sb, w2, p1, s) {
            None => {
                notes.airborne = true;
                let g = self.gravity;
                let t = dt.min(tick);
                self.velocity = Vec3::new(v.x - (g * UP.x) * t, v.y - (g * UP.y) * t, v.z - (g * UP.z) * t);
            }
            Some((_, mut n)) => {
                let ceiling = self.straighten(world, sb, step);
                let slid = match ceiling {
                    Some(cn) => self.low_ceiling_slide(world, w, p1, sb, dist, step, &mut n, cn, s),
                    None => true,
                };
                if !slid {
                    notes.refused = true;
                    notes.ceiling = true;
                    self.pos = sb + UP * self.d();
                } else if self.impassable(n, sb, s)
                    && !{
                        let side = self.side_slide(world, w, p1, sb, dist, step, &mut n, s);
                        notes.side_slide = side;
                        side
                    }
                    && !{
                        let down = self.down_slide(world, w, sb, dist, &mut n, p1, s);
                        notes.down_slide = down;
                        down
                    }
                {
                    notes.refused = true;
                    self.pos = sb + UP * self.d();
                    self.velocity = Vec3::ZERO;
                } else {
                    self.contact_normal = n;
                    let c = (n.y * UP.y + n.z * UP.z) + n.x * UP.x;
                    if c > s.max_slope_cos {
                        notes.landed = true;
                        self.velocity = Vec3::ZERO;
                    } else {
                        let vh = normalized_safe(v, -UP);
                        if -((n.y * vh.y + n.z * vh.z) + vh.x * n.x) < s.already_sliding_on_slope_dp || len2(v).abs() < 1e-5 {
                            self.update_slide_velocity(adt, sb);
                        } else {
                            let k = (v.x * n.x + v.y * n.y) + v.z * n.z;
                            self.velocity = Vec3::new(v.x - k * n.x, v.y - k * n.y, v.z - k * n.z);
                        }
                    }
                }
            }
        }
        // The tail: the full capsule again, its bottom kept.
        let b = self.pos - UP * self.d();
        self.dims = dims_of(if self.crouched { self.crouching } else { self.standing });
        let d2 = self.d();
        self.pos = Vec3::new(d2 * UP.x + b.x, d2 * UP.y + b.y, d2 * UP.z + b.z);
        if self.crouched {
            // 0x7E871: the stand test only with the key released; with it held +0x202 stays as the crouch left it (0).
            if !self.input_crouch {
                self.can_stand = self.stand_test(world);
            }
            if self.ceiling_push_due() {
                // 0x7BAD0: the horizontal part of the ceiling's normal, times 0.1; no decay this tick.
                let c = self.ceiling_normal;
                let k = (c.x * UP.x + c.z * UP.z) + c.y * UP.y;
                self.slide = self.slide + Vec3::new(c.x - UP.x * k, c.y - UP.y * k, c.z - UP.z * k) * 0.1;
                return;
            }
        }
        let sl = self.slide;
        if sl != Vec3::ZERO {
            let len = ((sl.x * sl.x + sl.y * sl.y) + sl.z * sl.z).sqrt();
            let k = if len - 0.1 > 0.0 { (len - 0.1) / len } else { 0.0 };
            self.slide = sl * k;
        }
    }

    /// `update_slide_velocity` 0x7FEA0.
    fn update_slide_velocity(&mut self, a: Vec3, sb: Vec3) {
        let v = self.velocity;
        let a2 = (a.y * a.y + a.z * a.z) + a.x * a.x;
        if !(a2 > 1e-5) && !(len2(v) > 1e-5) {
            return;
        }
        let inv_dt = 1.0 / self.tick;
        let d = self.d();
        let p = self.pos;
        let m = Vec3::new(((p.x - d * UP.x) - sb.x) * inv_dt, ((p.y - d * UP.y) - sb.y) * inv_dt, ((p.z - d * UP.z) - sb.z) * inv_dt);
        let vh = normalized_safe(v, -UP);
        let k = (vh.y * m.y + vh.x * m.x) + vh.z * m.z;
        let pp = vh * k;
        let mh = normalized_safe(m, Vec3::ZERO);
        let n = self.contact_normal;
        if len2(v).abs() >= 1e-5 && !(len2(pp) > len2(v)) {
            if !(0.0 < (mh.x * vh.x + mh.y * vh.y) + mh.z * vh.z) {
                self.velocity = Vec3::ZERO;
            } else {
                let kk = (n.y * pp.y + n.z * pp.z) + n.x * pp.x;
                self.velocity = Vec3::new(pp.x - n.x * kk, pp.y - n.y * kk, pp.z - n.z * kk);
            }
        } else if a2 > 1e-5 {
            let dh = normalized_safe(m - v, Vec3::ZERO);
            let sdot = (a.x * dh.x + dh.y * a.y) + dh.z * a.z;
            if sdot > 0.0 {
                let ah = normalized_safe(a, Vec3::ZERO);
                self.velocity = Vec3::new(ah.x * sdot + v.x, ah.y * sdot + v.y, ah.z * sdot + v.z);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Triangle;

    fn tri(a: Vec3, b: Vec3, c: Vec3, walker: WalkerBehaviour) -> Triangle {
        Triangle { v: [a, b, c], walker }
    }

    fn quad(a: Vec3, b: Vec3, c: Vec3, d: Vec3, out: &mut Vec<Triangle>) {
        out.push(tri(a, b, c, WalkerBehaviour::BySlope));
        out.push(tri(a, c, d, WalkerBehaviour::BySlope));
    }

    fn ground(y: f32, x0: f32, x1: f32, out: &mut Vec<Triangle>) {
        quad(Vec3::new(x0, y, -10.0), Vec3::new(x0, y, 10.0), Vec3::new(x1, y, 10.0), Vec3::new(x1, y, -10.0), out);
    }

    /// Walks a body east at 3.5 m/s in whole ticks for `ticks`; returns it.
    fn walk(world: &Collision, feet: Vec3, ticks: usize) -> Controller {
        let mut c = Controller::standing_at(feet);
        for _ in 0..ticks {
            c.walk = Vec3::new(3.5 * 0.05, 0.0, 0.0);
            c.step(world, 0.05, &Settings::LIVE);
        }
        c
    }

    /// On flat ground the body walks at its speed and stays on the ground.
    #[test]
    fn a_body_walks_on_flat_ground() {
        let mut tris = Vec::new();
        ground(0.0, -10.0, 10.0, &mut tris);
        let world = Collision::build(&tris);
        let c = walk(&world, Vec3::ZERO, 20);
        assert!((c.feet().x - 3.5).abs() < 0.02, "{:?}", c.feet());
        assert!(c.feet().y.abs() < 0.01, "{:?}", c.feet());
    }

    /// A 0.5 m step is walked up (the client steps 0.80); a 1.0 m ledge stops the body.
    #[test]
    fn a_step_under_the_step_height_is_climbed_and_a_ledge_over_it_is_not() {
        for (rise, climbed) in [(0.5f32, true), (1.0, false)] {
            let mut tris = Vec::new();
            ground(0.0, -10.0, 2.0, &mut tris);
            ground(rise, 2.0, 10.0, &mut tris);
            quad(Vec3::new(2.0, 0.0, -10.0), Vec3::new(2.0, rise, -10.0), Vec3::new(2.0, rise, 10.0), Vec3::new(2.0, 0.0, 10.0), &mut tris);
            let world = Collision::build(&tris);
            let c = walk(&world, Vec3::ZERO, 30);
            if climbed {
                assert!(c.feet().x > 4.0 && (c.feet().y - rise).abs() < 0.01, "rise {rise}: {:?}", c.feet());
            } else {
                assert!(c.feet().x < 2.0 && c.feet().y.abs() < 0.01, "rise {rise}: {:?}", c.feet());
            }
        }
    }

    /// The exe's own player_step run in Unicorn on this world (private/grid/controller/unicorn, 2026-11-05) climbs the 0.5 m step
    /// through these feet at ticks 10, 11, 12 and 13; the port agrees within its contact tolerance.
    #[test]
    fn the_step_climb_is_the_exes() {
        let mut tris = Vec::new();
        ground(0.0, -10.0, 2.0, &mut tris);
        ground(0.5, 2.0, 10.0, &mut tris);
        quad(Vec3::new(2.0, 0.0, -10.0), Vec3::new(2.0, 0.5, -10.0), Vec3::new(2.0, 0.5, 10.0), Vec3::new(2.0, 0.0, 10.0), &mut tris);
        let world = Collision::build(&tris);
        let exe = [(1.644244f32, 0.253557f32), (1.759254, 0.414009), (1.918323, 0.491118), (2.093098, 0.5)];
        let mut c = Controller::standing_at(Vec3::ZERO);
        for tick in 1..=13 {
            c.walk = Vec3::new(3.5 * 0.05, 0.0, 0.0);
            c.step(&world, 0.05, &Settings::LIVE);
            if tick >= 10 {
                let (x, y) = exe[tick - 10];
                let f = c.feet();
                assert!((f.x - x).abs() < 5e-4 && (f.y - y).abs() < 5e-4, "tick {tick}: {f:?} vs exe ({x}, {y})");
            }
        }
    }

    fn crouch_flags(c: &Controller) -> (bool, bool, bool, bool) {
        (c.input_crouch, c.want_crouch, c.can_stand, c.crouched)
    }

    /// The crouch key through the game's logic ([`Controller::crouch_key`]): the four flags before and after each step are
    /// the game's own bodies' (99 of 114 traced releases: held `il-c`; released `-l-c` > `-lsc`; then `--sc` > `--s-`). The
    /// body stands on the second step after the release, and no stand test is made while the key is held.
    #[test]
    fn a_released_crouch_stands_a_step_after_its_stand_test() {
        let mut tris = Vec::new();
        ground(0.0, -10.0, 10.0, &mut tris);
        let world = Collision::build(&tris);
        let mut c = Controller::standing_at(Vec3::ZERO);
        c.crouch_key(true);
        assert_eq!(crouch_flags(&c), (true, true, true, false));
        c.step(&world, 0.05, &Settings::LIVE);
        assert_eq!(crouch_flags(&c), (true, true, false, true));
        c.crouch_key(true);
        c.step(&world, 0.05, &Settings::LIVE);
        assert_eq!(crouch_flags(&c), (true, true, false, true), "held: +0x202 stays 0");
        c.crouch_key(false);
        assert_eq!(crouch_flags(&c), (false, true, false, true), "the logic keeps the crouch until the body may stand");
        c.step(&world, 0.05, &Settings::LIVE);
        assert_eq!(crouch_flags(&c), (false, true, true, true), "the stand test passes, the capsule is still down");
        c.crouch_key(false);
        assert_eq!(crouch_flags(&c), (false, false, true, true));
        c.step(&world, 0.05, &Settings::LIVE);
        assert_eq!(crouch_flags(&c), (false, false, true, false));
        assert!(c.feet().y.abs() < 0.01, "{:?}", c.feet());
    }

    /// Under a ceiling between the crouched and the standing height the released body stays down (18 traced steps
    /// `-l-c` > `-l-c`).
    #[test]
    fn a_crouch_under_a_low_ceiling_is_kept_after_the_release() {
        let mut tris = Vec::new();
        ground(0.0, -10.0, 10.0, &mut tris);
        ground(1.4, -10.0, 10.0, &mut tris);
        let world = Collision::build(&tris);
        let mut c = Controller::standing_at(Vec3::ZERO);
        // Put down as the game would have it there: crouched with the key held.
        c.crouch_key(true);
        c.set_crouched(true);
        for _ in 0..4 {
            c.crouch_key(false);
            c.step(&world, 0.05, &Settings::LIVE);
            assert_eq!(crouch_flags(&c), (false, true, false, true));
        }
    }

    /// A crouched body at rest skips its step while the key is held (`can_skip_player_step` 0x7C7E0 asks that the capsule
    /// agree with the key), and not on the step after the release, which makes the stand test.
    #[test]
    fn a_crouched_body_at_rest_skips_its_step_only_while_the_key_is_held() {
        let mut tris = Vec::new();
        ground(0.0, -10.0, 10.0, &mut tris);
        let world = Collision::build(&tris);
        let mut c = Controller::standing_at(Vec3::ZERO);
        c.crouch_key(true);
        c.step(&world, 0.05, &Settings::LIVE);
        c.crouch_key(true);
        assert!(c.step(&world, 0.05, &Settings::LIVE).skipped);
        c.crouch_key(false);
        assert!(!c.step(&world, 0.05, &Settings::LIVE).skipped);
        assert!(c.can_stand);
    }

    /// A wall stops the body with its side 0.42 m off it (the cylinder 0.38 m plus the 0.04 m margin): the exe's too (Unicorn).
    #[test]
    fn a_wall_stops_the_body_at_the_cylinder_and_margin() {
        let mut tris = Vec::new();
        ground(0.0, -10.0, 10.0, &mut tris);
        quad(Vec3::new(3.0, 0.0, -10.0), Vec3::new(3.0, 3.0, -10.0), Vec3::new(3.0, 3.0, 10.0), Vec3::new(3.0, 0.0, 10.0), &mut tris);
        let world = Collision::build(&tris);
        let c = walk(&world, Vec3::ZERO, 40);
        assert!((3.0 - c.feet().x - 0.42).abs() < 0.01, "{:?}", c.feet());
    }
}
