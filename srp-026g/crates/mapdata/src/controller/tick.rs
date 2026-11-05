//! The client's walk tick, ported from the 0.26g exe (A313; the readings are in `private/grid/controller/dis/notes-*.md`).
//!
//! `player_step` 0x7BDA0's walk branch: the capsule is cut from below by the step (`step_up` 0x7C900), swept along the tick's
//! move and slid along what it meets (`step_forward` 0x7D590 / `step_forward_impl` 0x7D2A0, the wall slide 0x7AC50), then swept
//! down onto the ground (`step_down` 0x7E1B0 with `step_down_sweep_test_impl` 0x7EA80), where ground steeper than the limit
//! that lifted the feet is refused unless one of the two slides takes the body (0x7D9C0, 0x7DA50). The origin then moves by
//! the step's share of the tick. Float operations keep the exe's order where the notes give it; the x87 square roots are
//! taken in f32 (the precision control word of the game is not read yet). The air branch (jumps) is not ported here.
//!
//! What is LIKELY rather than read: the via-ray wall normal (0x89B80, taken as the face's own normal made horizontal), the
//! down sweep's ray re-look of a steep by-slope face (0x84810, the face's normal kept), and the stand-up and slide sweeps
//! (0x86BF0, 0x88CB0, read in outline: capsule sweeps).
// The exe's comparisons are ported as written: `!(a > b)` and `a > b` differ on NaN, as `comiss` branches do.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
use super::{Capsule, Collision, Contact, Shape};
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
    };
}

/// One body's controller state (the fields of `bullet_character_controller` the walk tick reads and writes).
#[derive(Clone, Copy, Debug)]
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
}

/// What one step did, for the reader.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepNotes {
    pub skipped: bool,
    pub forward_hit: bool,
    pub landed: bool,
    pub airborne: bool,
    pub refused: bool,
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
        }
    }

    /// The feet (the capsule's bottom) at the origin.
    pub fn feet(&self) -> Vec3 {
        self.origin - UP * (self.dims.0 + self.dims.1)
    }

    fn d(&self) -> f32 {
        self.dims.0 + self.dims.1
    }

    fn capsule_at(&self, centre: Vec3) -> Capsule {
        let (r, hh) = self.dims;
        Capsule { low: centre - UP * hh, high: centre + UP * hh, radius: r }
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
        let v = self.velocity;
        let vy = (v.y * UP.y + v.z * UP.z) + v.x * UP.x;
        let slow = s.on_ground_max_vertical_speed > vy.abs();
        let start = self.origin;
        self.pos = start;
        let foot = start - UP * self.d();
        // can_skip_player_step 0x7C7E0.
        if len2(self.walk).abs() < TINY && len2(v) == 0.0 && !self.need_slide(s) && self.slide == Vec3::ZERO && !self.crouched {
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
        let mm = |x: f32| (x * 1000.0).round() / 1000.0;
        self.pos = Vec3::new(mm(self.pos.x), mm(self.pos.y), mm(self.pos.z));
        self.contact_normal = Vec3::new(mm(self.contact_normal.x), mm(self.contact_normal.y), mm(self.contact_normal.z));
        self.walk = Vec3::ZERO;
        notes
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
            let face = world.face(c.face);
            let k = (UP.z * c.normal.z + UP.y * c.normal.y) + UP.x * c.normal.x;
            let h = Vec3::new(c.normal.x - UP.x * k, c.normal.y - UP.y * k, c.normal.z - UP.z * k);
            let mut n2 = normalized_safe(h, -dir);
            let cc = (dir.z * -n2.z + dir.y * -n2.y) + dir.x * -n2.x;
            if cc > s.wall_full_slide_dot {
                // LIKELY (0x89B80 unread): the face's own normal, made horizontal, toward the body.
                let fnrm = if face.normal.dot(c.normal) < 0.0 { -face.normal } else { face.normal };
                let kf = fnrm.y;
                n2 = normalized_safe(Vec3::new(fnrm.x, fnrm.y - kf, fnrm.z), n2);
            }
            Some((c.t, n2, c.point))
        };
        let complex = if self.crouched { s.crouch_move_complex_test } else { s.stand_move_complex_test };
        if !complex {
            return callback(Shape::Capsule(self.capsule_at(from)), from).map(|(t, n, _)| (t, n));
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
        let by = UP * dist;
        let floor = ((UP.y * from.y + UP.z * from.z) + UP.x * from.x) - hh;
        let mut hits: Vec<Contact> = Vec::new();
        world.sweep_each(Shape::Capsule(self.capsule_at(from)), by, |c| hits.push(c));
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
            let face = world.face(c.face);
            let n = match face.walker {
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
                    if (UP.y * n.y + UP.z * n.z) + UP.x * n.x < s.max_slope_cos { self.ground_relook(world, c.point, n, s) } else { n }
                }
            };
            return Some((c.t, c.point, n));
        }
        None
    }

    /// 0x84810 for a map face (not a small round shape): a steep hit looked at again by a ray along `-n`, from 0.5 m out along
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
        let d = self.d();
        let p = self.pos;
        let h = (((p.z - d * UP.z) - sb.z) * UP.z + ((p.x - d * UP.x) - sb.x) * UP.x) + UP.y * ((p.y - d * UP.y) - sb.y);
        if !(h > 0.0) {
            return true;
        }
        world.sweep(self.capsule_at(p), UP * step, |_| true).is_none()
    }

    /// `slide_in_impassable_case_impl` 0x7D660 (the side slide). `n` in/out.
    #[allow(clippy::too_many_arguments)]
    fn side_slide(&mut self, world: &Collision, w: Vec3, p1: Vec3, sb: Vec3, dist: f32, step: f32, n: &mut Vec3, s: &Settings) -> bool {
        if !s.impassable_slope_side_slide || ((w.x * w.x + w.y * w.y) + w.z * w.z).abs() < TINY {
            return false;
        }
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
        let Some(c2) = world.sweep(self.capsule_at(bs), de - bs, |_| true) else { return false };
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
                if !self.can_straighten(world, sb, step) {
                    // side_slide_on_low_ceiling 0x7DF20 is not ported: the refusal.
                    notes.refused = true;
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
            let mut log = String::new();
            let mut d = Controller::standing_at(Vec3::ZERO);
            for _ in 0..14 {
                d.walk = Vec3::new(3.5 * 0.05, 0.0, 0.0);
                let n = d.step(&world, 0.05, &Settings::LIVE);
                log += &format!("\n{:.3},{:.3} {:?}", d.feet().x, d.feet().y, n);
            }
            if climbed {
                assert!(c.feet().x > 4.0 && (c.feet().y - rise).abs() < 0.01, "rise {rise}: {:?}{log}", c.feet());
            } else {
                assert!(c.feet().x < 2.0 && c.feet().y.abs() < 0.01, "rise {rise}: {:?}", c.feet());
            }
        }
    }

    /// A wall stops the body with its side 0.42 m off it (the cylinder 0.38 m plus the 0.04 m margin), LIKELY.
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
