//! The stand-in's body against walls as the client's capsule is (A191): a circle of the capsule's radius in the walls' horizontal
//! cross-sections, moved by the controller's own rule, where the lattice slid along every wall at full speed and through every
//! gap the grid has (the grid keeps its floors 0.3175 m from a wall, the capsule 0.38 m).
//!
//! The rule is `bullet_character_controller::step_forward_impl` 0x7D2A0 with `get_wall_slide_vector` 0x7AC50 (PROVEN, read from
//! the exe and run with Unicorn: `a191` notes in `player_bots_stuck.md`), with the numbers the live game sets in `startup.cfg`
//! (`cc_wall_full_slide_dot 0.50`, `cc_wall_slide_bouncing_factor 0.080`, `cc_wall_slide on`):
//! - the move is swept as a capsule; at the first contact the body stops there, and what is left of the move, `(1 - t)`, slides
//!   along the wall: the unit vector of `m - n (m.n) + 0.08 n` times `|m|` times `f(c)`, `c = |m.n|` for the unit move `m` and the
//!   wall's horizontal normal `n`; `f` is 1 for `c < 0.5` and `(1 - c) / 0.5` above it, so a wall met more than 60 degrees off its
//!   plane takes the speed (head on, all of it);
//! - a slide that points back against the walk's first direction loses that part; a slide shorter than 0.003 m ends the move;
//!   there are three sweeps in a move (`depth` 0 to 2);
//! - the contact normal is the capsule's (round at a corner); with `cc_wall_normal_via_ray` the exe takes a face's normal instead
//!   of it when the move is more head-on than `cc_wall_full_slide_dot` (`character_move_sweep_callback::addSingleResult` 0x895F0
//!   with 0x89B80, LIKELY: a ray along the move from the centre, 0.03 m past the contact, meeting a face).
//!
//! The walls are the walker mesh's faces that are not ground (steeper than the client's slope, or `dont_walk`), cut at four
//! heights above the step height (`cc_step_height 0.80`: the capsule is lifted by it for the horizontal sweep, so a face lower than
//! that is a step), and the body holds off them by `RADIUS_M`. `SRP_BENCH_STAND_IN_CORNER=off` leaves the lattice's slide.
//! Tests-only: the bots' reckoning does not know it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use mapdata::{Triangle, Vec3, WalkerBehaviour};

use super::bench_settings;

/// Whether the stand-in body meets walls as the client's capsule does (`SRP_BENCH_STAND_IN_CORNER`, on unless `off`).
pub(crate) fn stand_in_corner() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_CORNER").map_or(true, |v| v != "off"))
}

/// The capsule's radius, half of `stand_capsule_width` (0.76 m, PROVEN).
pub(crate) const RADIUS_M: f32 = 0.38;
/// `cc_wall_full_slide_dot`: a wall met with `|m.n|` over this takes speed; under it the slide keeps all of it.
const FULL_SLIDE_DOT: f32 = 0.5;
/// `cc_wall_slide_bouncing_factor`: the slide is pushed off the wall by this much of its length.
const BOUNCE: f32 = 0.08;
/// `cc_wall_normal_via_ray_offset`.
const RAY_OFFSET_M: f32 = 0.03;
/// The sweeps in a move: `step_forward_impl` recurses to depth 2.
const SWEEPS: usize = 3;
/// A slide shorter than this squared ends the move (`0x9CBA48`, 1e-5).
const MIN_SLIDE2: f32 = 1e-5;
/// The heights over the feet the walls are cut at, m (the step height is 0.80).
const HEIGHTS: [f32; 4] = [0.9, 1.2, 1.5, 1.75];
/// The most a body overlapping a wall is pushed out in a tick, m.
const PUSH_M: f32 = 0.15;
/// The size of the buckets of the index, m.
const BUCKET_M: f32 = 2.0;

/// A wall's cross-section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Seg {
    pub(crate) a: (f32, f32),
    pub(crate) b: (f32, f32),
}

#[derive(Clone, Copy)]
struct Wall {
    v: [Vec3; 3],
    y: (f32, f32),
}

/// The walker mesh's walls, by bucket.
pub(crate) struct Sections {
    walls: Vec<Wall>,
    buckets: HashMap<(i32, i32), Vec<u32>>,
}

impl Sections {
    /// The walls of `triangles`: not ground for a body whose ground is `min_ground_normal_y` and up.
    pub(crate) fn new(triangles: &[Triangle], min_ground_normal_y: f32) -> Self {
        let mut walls = Vec::new();
        let mut buckets: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for t in triangles {
            let (e1, e2) = (sub(t.v[1], t.v[0]), sub(t.v[2], t.v[0]));
            let n = [e1.y * e2.z - e1.z * e2.y, e1.z * e2.x - e1.x * e2.z, e1.x * e2.y - e1.y * e2.x];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len < 1e-9 {
                continue;
            }
            let ny = n[1] / len;
            let ground = match t.walker {
                WalkerBehaviour::Walk => true,
                WalkerBehaviour::DontWalk => false,
                WalkerBehaviour::BySlope => ny >= min_ground_normal_y,
            };
            // A face a body is below (a ceiling) cuts the horizontal plane too, only when it is not flat.
            if ground || ny.abs() > 0.999 {
                continue;
            }
            let y = (t.v[0].y.min(t.v[1].y).min(t.v[2].y), t.v[0].y.max(t.v[1].y).max(t.v[2].y));
            let id = walls.len() as u32;
            walls.push(Wall { v: t.v, y });
            let (x0, x1) = (t.v[0].x.min(t.v[1].x).min(t.v[2].x), t.v[0].x.max(t.v[1].x).max(t.v[2].x));
            let (z0, z1) = (t.v[0].z.min(t.v[1].z).min(t.v[2].z), t.v[0].z.max(t.v[1].z).max(t.v[2].z));
            for bx in bucket(x0)..=bucket(x1) {
                for bz in bucket(z0)..=bucket(z1) {
                    buckets.entry((bx, bz)).or_default().push(id);
                }
            }
        }
        Self { walls, buckets }
    }

    /// The cross-sections within `reach` m of `at` of the walls a body standing on `y` meets.
    pub(crate) fn near(&self, at: (f32, f32), y: f32, reach: f32, out: &mut Vec<Seg>) {
        out.clear();
        let mut seen: Vec<u32> = Vec::new();
        for bx in bucket(at.0 - reach)..=bucket(at.0 + reach) {
            for bz in bucket(at.1 - reach)..=bucket(at.1 + reach) {
                if let Some(ids) = self.buckets.get(&(bx, bz)) {
                    seen.extend_from_slice(ids);
                }
            }
        }
        seen.sort_unstable();
        seen.dedup();
        for id in seen {
            let w = &self.walls[id as usize];
            for h in HEIGHTS {
                let plane = y + h;
                if plane < w.y.0 || plane > w.y.1 {
                    continue;
                }
                if let Some(seg) = cut(&w.v, plane)
                    && seg_distance(&seg, at) <= reach
                {
                    out.push(seg);
                }
            }
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.walls.len()
    }
}

fn bucket(v: f32) -> i32 {
    (v / BUCKET_M).floor() as i32
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

/// A triangle's cut by the plane `y = plane`: the segment, when two edges cross it.
fn cut(v: &[Vec3; 3], plane: f32) -> Option<Seg> {
    let mut points = [(0.0, 0.0); 2];
    let mut n = 0;
    for i in 0..3 {
        let (p, q) = (v[i], v[(i + 1) % 3]);
        let (dp, dq) = (p.y - plane, q.y - plane);
        if (dp < 0.0) != (dq < 0.0) {
            let t = dp / (dp - dq);
            if n < 2 {
                points[n] = (p.x + (q.x - p.x) * t, p.z + (q.z - p.z) * t);
                n += 1;
            }
        }
    }
    (n == 2 && (points[0].0 - points[1].0).hypot(points[0].1 - points[1].1) > 1e-4).then_some(Seg { a: points[0], b: points[1] })
}

/// The distance from `p` to a segment, and the point on it nearest `p`.
pub(crate) fn nearest(seg: &Seg, p: (f32, f32)) -> ((f32, f32), f32) {
    let (ex, ez) = (seg.b.0 - seg.a.0, seg.b.1 - seg.a.1);
    let len2 = ex * ex + ez * ez;
    let t = (((p.0 - seg.a.0) * ex + (p.1 - seg.a.1) * ez) / len2).clamp(0.0, 1.0);
    let q = (seg.a.0 + ex * t, seg.a.1 + ez * t);
    (q, (p.0 - q.0).hypot(p.1 - q.1))
}

fn seg_distance(seg: &Seg, p: (f32, f32)) -> f32 {
    nearest(seg, p).1
}

/// The first contact of a circle of radius `r` at `p` moved by `m` with the segments: how far along the move (0 to 1) and the
/// normal of the wall there (from the wall to the circle's centre, unit). Only a circle closing on a wall touches it.
pub(crate) fn sweep(segs: &[Seg], p: (f32, f32), m: (f32, f32), r: f32) -> Option<(f32, (f32, f32))> {
    let mut best: Option<(f32, (f32, f32), bool)> = None;
    let mut consider = |t: f32, n: (f32, f32), line: bool| {
        if best.is_none_or(|(bt, _, bline)| t < bt - 1e-6 || (t < bt + 1e-6 && line && !bline)) {
            best = Some((t, n, line));
        }
    };
    for seg in segs {
        let (ex, ez) = (seg.b.0 - seg.a.0, seg.b.1 - seg.a.1);
        let len = ex.hypot(ez);
        if len < 1e-5 {
            continue;
        }
        let (ux, uz) = (ex / len, ez / len);
        let nrm = (-uz, ux);
        let d0 = (p.0 - seg.a.0) * nrm.0 + (p.1 - seg.a.1) * nrm.1;
        let vn = m.0 * nrm.0 + m.1 * nrm.1;
        for side in [1.0f32, -1.0] {
            // On this side of the wall's line and closing on it.
            if side * d0 < r - 1e-4 || side * vn >= -1e-9 {
                continue;
            }
            let t = (side * r - d0) / vn;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let (cx, cz) = (p.0 + m.0 * t, p.1 + m.1 * t);
            let s = (cx - seg.a.0) * ux + (cz - seg.a.1) * uz;
            if (0.0..=len).contains(&s) {
                consider(t, (nrm.0 * side, nrm.1 * side), true);
            }
        }
        for q in [seg.a, seg.b] {
            let (px, pz) = (p.0 - q.0, p.1 - q.1);
            let (a, b, c) = (m.0 * m.0 + m.1 * m.1, 2.0 * (m.0 * px + m.1 * pz), px * px + pz * pz - r * r);
            if c < -1e-4 || b >= 0.0 || a < 1e-12 {
                continue;
            }
            let disc = b * b - 4.0 * a * c;
            if disc < 0.0 {
                continue;
            }
            let t = (-b - disc.sqrt()) / (2.0 * a);
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let (cx, cz) = (p.0 + m.0 * t - q.0, p.1 + m.1 * t - q.1);
            let l = cx.hypot(cz).max(1e-9);
            consider(t, (cx / l, cz / l), false);
        }
    }
    best.map(|(t, n, _)| (t, n))
}

/// The normal of the face a ray along `dir` from `p` meets within `reach`, from the wall to the ray's start.
fn face_ahead(segs: &[Seg], p: (f32, f32), dir: (f32, f32), reach: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, (f32, f32))> = None;
    for seg in segs {
        let (ex, ez) = (seg.b.0 - seg.a.0, seg.b.1 - seg.a.1);
        let denom = dir.0 * ez - dir.1 * ex;
        if denom.abs() < 1e-9 {
            continue;
        }
        let (ax, az) = (seg.a.0 - p.0, seg.a.1 - p.1);
        let t = (ax * ez - az * ex) / denom;
        let s = (ax * dir.1 - az * dir.0) / denom;
        if !(0.0..=reach).contains(&t) || !(0.0..=1.0).contains(&s) || best.is_some_and(|(bt, _)| bt <= t) {
            continue;
        }
        let len = ex.hypot(ez);
        let mut n = (-ez / len, ex / len);
        if n.0 * dir.0 + n.1 * dir.1 > 0.0 {
            n = (-n.0, -n.1);
        }
        best = Some((t, n));
    }
    best.map(|(_, n)| n)
}

/// How a move of the capsule ended.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Outcome {
    /// The centre it comes to.
    pub(crate) to: (f32, f32),
    /// The body overlapped a wall at the start and was pushed off it.
    pub(crate) pushed: bool,
    /// Walls met on the move (the sweeps that touched).
    pub(crate) contacts: u8,
    /// How much of the move's length was made: 1 when nothing was met.
    pub(crate) made: f32,
}

/// The move of a capsule at `from` by `m` among `segs` ([`Sections::near`] with a reach over the move and the radius).
pub(crate) fn client_move(segs: &[Seg], from: (f32, f32), m: (f32, f32), r: f32) -> Outcome {
    let mut p = from;
    let mut pushed = false;
    // Overlapping a wall at the start: out of it, along the line from its nearest point.
    for _ in 0..3 {
        let hit = segs.iter().map(|s| nearest(s, p)).filter(|&(_, d)| d < r - 1e-4).min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((q, d)) = hit else { break };
        let (dx, dz) = if d > 1e-6 { ((p.0 - q.0) / d, (p.1 - q.1) / d) } else { (0.0, 0.0) };
        let push = (r - d).min(PUSH_M);
        p = (p.0 + dx * push, p.1 + dz * push);
        pushed = true;
    }
    let full = m.0.hypot(m.1);
    if full < 1e-5 {
        return Outcome { to: p, pushed, contacts: 0, made: 1.0 };
    }
    let dir = (m.0 / full, m.1 / full);
    let mut cur = m;
    let mut contacts = 0u8;
    for _ in 0..SWEEPS {
        let len = cur.0.hypot(cur.1);
        let Some((t, mut n)) = sweep(segs, p, cur, r) else {
            p = (p.0 + cur.0, p.1 + cur.1);
            break;
        };
        contacts += 1;
        p = (p.0 + cur.0 * t, p.1 + cur.1 * t);
        let unit = (cur.0 / len, cur.1 / len);
        let mut c = -(unit.0 * n.0 + unit.1 * n.1);
        if c > FULL_SLIDE_DOT
            && let Some(face) = face_ahead(segs, p, unit, r + RAY_OFFSET_M)
        {
            n = face;
            c = -(unit.0 * n.0 + unit.1 * n.1);
        }
        let c = c.abs();
        let keep = if c < FULL_SLIDE_DOT { 1.0 } else { (1.0 - c) / (1.0 - FULL_SLIDE_DOT) };
        let dn = unit.0 * n.0 + unit.1 * n.1;
        let (tx, tz) = (unit.0 - n.0 * dn + n.0 * BOUNCE, unit.1 - n.1 * dn + n.1 * BOUNCE);
        let tl = tx.hypot(tz).max(1e-9);
        let scale = len * keep * (1.0 - t) / tl;
        let mut v = (tx * scale, tz * scale);
        let back = dir.0 * v.0 + dir.1 * v.1;
        if back < 0.0 {
            v = (v.0 - dir.0 * back, v.1 - dir.1 * back);
        }
        if v.0 * v.0 + v.1 * v.1 < MIN_SLIDE2 {
            break;
        }
        cur = v;
    }
    Outcome { to: p, pushed, contacts, made: ((p.0 - from.0) * dir.0 + (p.1 - from.1) * dir.1).max(0.0) / full }
}

/// The ticks the capsule rule moved bodies, for the tally: moves, with a wall met, stopped (under a tenth of the move made),
/// slowed (under nine tenths), and bodies pushed off a wall.
pub(crate) mod counts {
    use super::*;

    static MOVES: AtomicU64 = AtomicU64::new(0);
    static MET: AtomicU64 = AtomicU64::new(0);
    static STOPPED: AtomicU64 = AtomicU64::new(0);
    static SLOWED: AtomicU64 = AtomicU64::new(0);
    static PUSHED: AtomicU64 = AtomicU64::new(0);

    pub(crate) fn note(o: &Outcome, moving: bool) {
        if !moving {
            return;
        }
        MOVES.fetch_add(1, Ordering::Relaxed);
        if o.contacts > 0 {
            MET.fetch_add(1, Ordering::Relaxed);
            if o.made < 0.1 {
                STOPPED.fetch_add(1, Ordering::Relaxed);
            } else if o.made < 0.9 {
                SLOWED.fetch_add(1, Ordering::Relaxed);
            }
        }
        if o.pushed {
            PUSHED.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn counts() -> [u64; 5] {
        [MOVES, MET, STOPPED, SLOWED, PUSHED].map(|c| c.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall along x = 0 from z `z0` to `z1`.
    fn wall(z0: f32, z1: f32) -> Seg {
        Seg { a: (0.0, z0), b: (0.0, z1) }
    }

    #[test]
    fn a_wall_met_head_on_stops_the_body_at_the_radius() {
        let out = client_move(&[wall(-10.0, 10.0)], (-1.0, 0.0), (0.5, 0.0), RADIUS_M);
        assert!((out.to.0 + RADIUS_M).abs() < 1e-3, "{out:?}");
        assert_eq!(out.contacts, 1);
        assert!(out.made < 0.3, "{out:?}");
    }

    /// The client's wall slide (0x7AC50 run with Unicorn at the live 0.5 and 0.08): what is left of the move after the contact goes
    /// along the wall at full length when the wall is met within 30 degrees of its plane, and at `(1 - c) / 0.5` of it over that.
    #[test]
    fn a_glancing_wall_is_slid_along_at_full_speed() {
        // From x -0.5 by (0.3, 1.0): the contact at t 0.4, the rest (0.6 of 1.044 m) along the wall.
        let out = client_move(&[wall(-10.0, 10.0)], (-0.5, 0.0), (0.3, 1.0), RADIUS_M);
        assert!(out.contacts >= 1);
        assert!(out.to.1 > 0.97 && out.to.1 < 1.02, "{out:?}");
        assert!(out.to.0 < -RADIUS_M + 0.01 && out.to.0 > -RADIUS_M - 0.1, "{out:?}");
    }

    #[test]
    fn a_wall_met_30_degrees_off_its_normal_keeps_a_quarter_of_the_slide() {
        let a = 30f32.to_radians();
        let out = client_move(&[wall(-10.0, 10.0)], (-0.5, 0.0), (0.4 * a.cos(), 0.4 * a.sin()), RADIUS_M);
        // z at the contact 0.0693, the rest 0.2614 m times (1 - 0.866) / 0.5 = 0.0701 m, along z by 0.987.
        assert!((out.to.1 - 0.1385).abs() < 0.004, "{out:?}");
    }

    #[test]
    fn a_body_in_a_wall_is_pushed_out_to_its_radius() {
        let out = client_move(&[wall(-10.0, 10.0)], (-0.2, 0.0), (0.0, 0.1), RADIUS_M);
        assert!(out.pushed);
        assert!((out.to.0 + RADIUS_M).abs() < 2e-3, "{out:?}");
        assert!((out.to.1 - 0.1).abs() < 2e-3, "{out:?}");
    }

    #[test]
    fn a_body_passing_a_jamb_beyond_a_radius_is_not_held() {
        // The face ends at z 0; walking +x at z 0.45: not within the radius of the corner.
        let jamb = [Seg { a: (1.0, -5.0), b: (1.0, 0.0) }];
        let out = client_move(&jamb, (0.0, 0.45), (1.5, 0.0), RADIUS_M);
        assert_eq!(out.contacts, 0);
        assert!((out.to.0 - 1.5).abs() < 1e-4);
    }

    #[test]
    fn a_body_walking_at_a_jamb_head_on_is_stopped_and_one_off_the_face_slides_round_the_corner() {
        let face = [Seg { a: (1.0, -5.0), b: (1.0, 0.0) }];
        // Straight at the face: stopped at the radius.
        let stopped = client_move(&face, (0.0, -0.1), (1.0, 0.0), RADIUS_M);
        assert!(stopped.made < 0.65 && stopped.contacts >= 1, "{stopped:?}");
        // 0.25 m beside its edge: the contact is the corner's, c 0.75, and half the rest of the move slides round it.
        let round = client_move(&face, (0.0, 0.25), (1.0, 0.0), RADIUS_M);
        assert!(round.contacts >= 1 && (round.to.1 - 0.367).abs() < 0.01 && round.to.0 > 0.75, "{round:?}");
    }

    #[test]
    fn the_cross_section_of_a_triangle_is_cut_by_the_plane() {
        let v = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 2.0, 0.0), Vec3::new(2.0, 0.0, 0.0)];
        let seg = cut(&v, 1.0).unwrap();
        assert!((seg.a.0 - seg.b.0).abs() <= 1.0 && seg.a != seg.b);
        assert!(cut(&v, 3.0).is_none());
    }
}
