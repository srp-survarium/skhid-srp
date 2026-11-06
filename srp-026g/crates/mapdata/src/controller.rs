//! The client's character controller against the map's collision (A313): the capsule and its sweeps.
//!
//! The 0.26g client moves a body with `vostok::physics::bullet_character_controller` (a Bullet kinematic controller): each walk
//! tick lifts the capsule by the step (`step_up` 0x7C900), sweeps it along the move and slides it along what it meets
//! (`step_forward_impl` 0x7D2A0), and sweeps it down onto the ground (`step_down` 0x7E1B0), refusing ground steeper than the
//! slope limit. This module is the geometry those steps stand on: a capsule (a vertical segment between its two sphere centres
//! and a radius) swept by a translation against the map's triangles, giving the first contact, its point and its normal.
//!
//! How: every triangle near the swept volume (an AABB tree over the triangles) is advanced on conservatively: the exact distance
//! from the capsule's segment to the triangle, less the radius, is how far the capsule can move with no contact, so the sweep
//! steps that far along the move and measures again, until the gap is under [`CONTACT_M`] or the move ends. A capsule that only
//! translates gets no closer than its speed allows, so no contact is stepped over.
pub mod gjk;
pub mod speed;
pub mod tick;

use crate::{Triangle, WalkerBehaviour};
use crate::math::Vec3;

/// The gap at which a sweep counts as touching, m.
pub const CONTACT_M: f32 = 1e-4;
/// A touching capsule's move goes into the face when its part against the normal is over this share of it (float noise in a
/// normal along the face is not a move into it).
const GOING_IN: f32 = 1e-4;
/// Steps of the conservative advancement against one triangle at most.
const ADVANCE_STEPS: usize = 64;
/// Triangles a leaf of the tree holds at most.
const LEAF: usize = 4;

/// One triangle of the collision: its corners, unit normal (zero for a degenerate one) and walker behaviour (the down sweep
/// reads it: a `walk` face counts as flat, a `dont_walk` one as steep).
#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub v: [Vec3; 3],
    pub normal: Vec3,
    pub walker: WalkerBehaviour,
}

/// A body swept against the collision: a convex core and the margin around it (Bullet's rounded shapes).
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// A capsule: the segment between its sphere centres, the radius as the margin.
    Capsule(Capsule),
    /// An upright cylinder's core (centre, half height, radius) and its margin.
    Cylinder { centre: Vec3, half: f32, radius: f32, margin: f32 },
    /// A sphere: its centre and radius.
    Sphere { centre: Vec3, radius: f32 },
}

impl Shape {
    pub fn moved(self, by: Vec3) -> Self {
        match self {
            Shape::Capsule(c) => Shape::Capsule(c.moved(by)),
            Shape::Cylinder { centre, half, radius, margin } => Shape::Cylinder { centre: centre + by, half, radius, margin },
            Shape::Sphere { centre, radius } => Shape::Sphere { centre: centre + by, radius },
        }
    }

    /// The distance from the shape's surface to the triangle (negative inside the margin), the closest points on the core and
    /// on the triangle, and the direction from the triangle to the shape.
    pub fn gap(&self, v: &[Vec3; 3], normal: Vec3) -> (f32, Vec3, Vec3, Vec3) {
        let (d, on_core, on_face, margin) = match *self {
            Shape::Capsule(c) => {
                let (d, s, f) = segment_triangle(c.low, c.high, v);
                (d, s, f, c.radius)
            }
            Shape::Sphere { centre, radius } => {
                let f = point_triangle(centre, v);
                ((centre - f).length(), centre, f, radius)
            }
            Shape::Cylinder { centre, half, radius, margin } => {
                let core = gjk::Cylinder { centre, half, radius };
                let guess = centre - (v[0] + v[1] + v[2]) * (1.0 / 3.0);
                let (d, a, b) = gjk::distance(&core, v, guess);
                (d, a, b, margin)
            }
        };
        let dir = if d > 1e-6 { (on_core - on_face) * (1.0 / d) } else { normal };
        (d - margin, on_core, on_face, dir)
    }

    /// The shape's box.
    fn aabb(&self) -> (Vec3, Vec3) {
        match *self {
            Shape::Capsule(c) => {
                let r = Vec3::new(c.radius, c.radius, c.radius);
                (c.low.min(c.high) - r, c.low.max(c.high) + r)
            }
            Shape::Cylinder { centre, half, radius, margin } => {
                let e = Vec3::new(radius + margin, half + margin, radius + margin);
                (centre - e, centre + e)
            }
            Shape::Sphere { centre, radius } => {
                let e = Vec3::new(radius, radius, radius);
                (centre - e, centre + e)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    /// A leaf: `first..first + count` of the faces; else `first` is the right child (the left is the next node).
    first: u32,
    count: u32,
}

/// The map's collision faces in an AABB tree, for capsule sweeps.
pub struct Collision {
    faces: Vec<Face>,
    nodes: Vec<Node>,
}

/// The first contact of a sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The share of the move done before the contact, 0..=1.
    pub t: f32,
    /// The contact's point on the face.
    pub point: Vec3,
    /// From the face to the capsule at the contact, unit.
    pub normal: Vec3,
    /// The face met (an index into [`Collision::face`]).
    pub face: usize,
}

/// A capsule standing upright: the centres of its lower and upper spheres and its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub low: Vec3,
    pub high: Vec3,
    pub radius: f32,
}

impl Capsule {
    /// The capsule whose bottom (the lowest point of its lower sphere) is at `feet`, `height` tall in all.
    pub fn standing(feet: Vec3, height: f32, radius: f32) -> Self {
        let low = feet + Vec3::new(0.0, radius, 0.0);
        let high = feet + Vec3::new(0.0, (height - radius).max(radius), 0.0);
        Self { low, high, radius }
    }

    /// The capsule moved by `by`.
    pub fn moved(self, by: Vec3) -> Self {
        Self { low: self.low + by, high: self.high + by, ..self }
    }
}

fn aabb(points: impl IntoIterator<Item = Vec3>) -> (Vec3, Vec3) {
    let mut lo = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut hi = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in points {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (lo, hi)
}

fn overlaps(lo: Vec3, hi: Vec3, qlo: Vec3, qhi: Vec3) -> bool {
    lo.x <= qhi.x && hi.x >= qlo.x && lo.y <= qhi.y && hi.y >= qlo.y && lo.z <= qhi.z && hi.z >= qlo.z
}

impl Collision {
    /// The collision of these triangles (every walker behaviour: a `dont_walk` face stops a body as any face does).
    pub fn build(triangles: &[Triangle]) -> Self {
        let mut faces: Vec<Face> = triangles
            .iter()
            .filter(|t| t.v.iter().all(|p| p.is_finite()))
            .map(|t| {
                let n = (t.v[1] - t.v[0]).cross(t.v[2] - t.v[0]);
                let l = n.length();
                Face { v: t.v, normal: if l > 1e-12 { n * (1.0 / l) } else { Vec3::ZERO }, walker: t.walker }
            })
            .collect();
        let mut nodes = Vec::with_capacity(2 * faces.len() / LEAF + 1);
        if !faces.is_empty() {
            let n = faces.len();
            Self::split(&mut faces, 0, n, &mut nodes);
        }
        Self { faces, nodes }
    }

    fn split(faces: &mut [Face], first: usize, end: usize, nodes: &mut Vec<Node>) -> usize {
        let (lo, hi) = aabb(faces[first..end].iter().flat_map(|f| f.v));
        let me = nodes.len();
        nodes.push(Node { lo, hi, first: first as u32, count: (end - first) as u32 });
        if end - first <= LEAF {
            return me;
        }
        let size = hi - lo;
        let axis = if size.x >= size.y && size.x >= size.z { 0 } else if size.y >= size.z { 1 } else { 2 };
        let centre = |f: &Face| (f.v[0].axis(axis) + f.v[1].axis(axis) + f.v[2].axis(axis)) / 3.0;
        faces[first..end].sort_by(|a, b| centre(a).total_cmp(&centre(b)));
        let mid = first + (end - first) / 2;
        Self::split(faces, first, mid, nodes);
        let right = Self::split(faces, mid, end, nodes);
        nodes[me].first = right as u32;
        nodes[me].count = 0;
        me
    }

    pub fn len(&self) -> usize {
        self.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    pub fn face(&self, index: usize) -> &Face {
        &self.faces[index]
    }

    /// Calls `each` with the index of every face whose box meets the box `qlo..qhi`.
    pub fn faces_in(&self, qlo: Vec3, qhi: Vec3, mut each: impl FnMut(usize)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let node = self.nodes[i];
            if !overlaps(node.lo, node.hi, qlo, qhi) {
                continue;
            }
            if node.count > 0 {
                for f in node.first as usize..(node.first + node.count) as usize {
                    let (lo, hi) = aabb(self.faces[f].v);
                    if overlaps(lo, hi, qlo, qhi) {
                        each(f);
                    }
                }
            } else {
                stack.push(i + 1);
                stack.push(node.first as usize);
            }
        }
    }

    /// The first contact of `capsule` moved by `by`, against faces `keep` accepts. A face the capsule already overlaps at the
    /// start is met only when the move goes into it (against its normal at the start).
    pub fn sweep(&self, capsule: Capsule, by: Vec3, keep: impl Fn(&Face) -> bool) -> Option<Contact> {
        let mut best: Option<Contact> = None;
        self.sweep_each(Shape::Capsule(capsule), by, |c| {
            if keep(&self.faces[c.face]) && best.is_none_or(|b| c.t < b.t) {
                best = Some(c);
            }
        });
        best
    }

    /// Every face's first contact with `shape` moved by `by` (Bullet's per-triangle convex cast: what the client's sweep
    /// callbacks are handed, in no order). A face the shape already touches at the start is reported at 0 only when the move
    /// goes into it.
    pub fn sweep_each(&self, shape: Shape, by: Vec3, mut each: impl FnMut(Contact)) {
        let (lo0, hi0) = shape.aabb();
        let (lo1, hi1) = shape.moved(by).aabb();
        let pad = Vec3::new(CONTACT_M, CONTACT_M, CONTACT_M);
        self.faces_in(lo0.min(lo1) - pad, hi0.max(hi1) + pad, |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO {
                return;
            }
            if let Some(c) = advance(shape, by, face) {
                each(Contact { face: f, ..c });
            }
        });
    }

    /// The nearest face a ray from `a` to `b` crosses: the share of the way and the face's normal turned toward `a` (Bullet's
    /// triangle ray normal, LIKELY).
    pub fn ray(&self, a: Vec3, b: Vec3) -> Option<(f32, Vec3)> {
        let d = b - a;
        let mut best: Option<(f32, Vec3)> = None;
        self.faces_in(a.min(b), a.max(b), |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO {
                return;
            }
            // Moller-Trumbore.
            let (e1, e2) = (face.v[1] - face.v[0], face.v[2] - face.v[0]);
            let p = d.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-12 {
                return;
            }
            let inv = 1.0 / det;
            let s = a - face.v[0];
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                return;
            }
            let q = s.cross(e1);
            let v = d.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                return;
            }
            let t = e2.dot(q) * inv;
            if (0.0..=1.0).contains(&t) && best.is_none_or(|b| t < b.0) {
                let n = if face.normal.dot(d) > 0.0 { -face.normal } else { face.normal };
                best = Some((t, n));
            }
        });
        best
    }

    /// The face nearest `shape`'s surface that `keep` accepts within `reach` (a negative gap: overlapping): the gap, the face's
    /// closest point and the direction from it to the shape.
    pub fn nearest_to(&self, shape: Shape, reach: f32, keep: impl Fn(&Face) -> bool) -> Option<(f32, Vec3, Vec3)> {
        let (lo, hi) = shape.aabb();
        let pad = Vec3::new(reach, reach, reach);
        let mut best: Option<(f32, Vec3, Vec3)> = None;
        self.faces_in(lo - pad, hi + pad, |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO || !keep(face) {
                return;
            }
            let (gap, _, on_face, dir) = shape.gap(&face.v, face.normal);
            if gap <= reach && best.is_none_or(|b| gap < b.0) {
                best = Some((gap, on_face, dir));
            }
        });
        best
    }

    /// The smallest distance from the capsule's surface to a face `keep` accepts within `reach` (negative when it overlaps
    /// one), with that face's closest point and the direction from it to the capsule's segment.
    pub fn nearest(&self, capsule: Capsule, reach: f32, keep: impl Fn(&Face) -> bool) -> Option<(f32, Vec3, Vec3, usize)> {
        let pad = Vec3::new(capsule.radius + reach, capsule.radius + reach, capsule.radius + reach);
        let (lo, hi) = aabb([capsule.low, capsule.high]);
        let mut best: Option<(f32, Vec3, Vec3, usize)> = None;
        self.faces_in(lo - pad, hi + pad, |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO || !keep(face) {
                return;
            }
            let (gap, _, on_face, dir) = Shape::Capsule(capsule).gap(&face.v, face.normal);
            if gap <= reach && best.is_none_or(|b| gap < b.0) {
                best = Some((gap, on_face, dir, f));
            }
        });
        best
    }
}

/// The conservative advancement of `shape` moved by `by` against one face.
fn advance(shape: Shape, by: Vec3, face: &Face) -> Option<Contact> {
    if std::env::var_os("SRP_PROBE_BULLET_CAST").is_some() {
        return advance_bullet(shape, by, face);
    }
    let speed = by.length();
    let (g0, _, f0, n0) = shape.gap(&face.v, face.normal);
    if g0 <= CONTACT_M {
        // Touching or overlapping at the start: a contact only when the move goes in.
        return (by.dot(n0) < -GOING_IN * speed).then_some(Contact { t: 0.0, point: f0, normal: n0, face: 0 });
    }
    if speed <= 0.0 {
        return None;
    }
    let mut t = 0.0f32;
    for _ in 0..ADVANCE_STEPS {
        let (gap, _, f, n) = shape.moved(by * t).gap(&face.v, face.normal);
        if gap <= CONTACT_M {
            return Some(Contact { t, point: f, normal: n, face: 0 });
        }
        t += gap / speed;
        if t > 1.0 {
            return None;
        }
    }
    None
}

/// PROBE (A325 P1): the exe's caster 0xB9320, stock btContinuousConvexCollision::calcTimeOfImpact: no hit when the move's
/// part along the normal (toward the face) is <= FLT_EPSILON; advance by gap / that part while the gap is over 0.001 m; at
/// most 64 steps, then no hit.
fn advance_bullet(shape: Shape, by: Vec3, face: &Face) -> Option<Contact> {
    const TOL: f32 = 0.001;
    const VEL_EPS: f32 = f32::EPSILON;
    let (mut gap, _, mut f, mut n) = shape.gap(&face.v, face.normal);
    let mut proj = -by.dot(n);
    if proj <= VEL_EPS {
        return None;
    }
    let mut t = 0.0f32;
    let mut steps = 0;
    while gap > TOL {
        steps += 1;
        if steps > ADVANCE_STEPS {
            return None;
        }
        t += gap / proj;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        (gap, _, f, n) = shape.moved(by * t).gap(&face.v, face.normal);
        proj = -by.dot(n);
        if proj <= VEL_EPS {
            return None;
        }
    }
    Some(Contact { t, point: f, normal: n, face: 0 })
}

/// The closest points of point `p` on triangle `v` (Ericson, Real-Time Collision Detection 5.1.5).
pub fn point_triangle(p: Vec3, v: &[Vec3; 3]) -> Vec3 {
    let (a, b, c) = (v[0], v[1], v[2]);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// The closest points of segments `p1..q1` and `p2..q2` (Ericson 5.1.9).
fn segment_segment(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let eps = 1e-12;
    let (s, t);
    if a <= eps && e <= eps {
        return (p1, p2);
    }
    if a <= eps {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= eps {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > eps { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (p1 + d1 * s, p2 + d2 * t)
}

/// The distance from segment `p..q` to triangle `v`, with the closest point on each.
pub fn segment_triangle(p: Vec3, q: Vec3, v: &[Vec3; 3]) -> (f32, Vec3, Vec3) {
    // The segment through the triangle's inside: distance 0 there.
    let n = (v[1] - v[0]).cross(v[2] - v[0]);
    let (dp, dq) = (n.dot(p - v[0]), n.dot(q - v[0]));
    if (dp > 0.0) != (dq > 0.0) && dp != dq {
        let x = p + (q - p) * (dp / (dp - dq));
        let on = point_triangle(x, v);
        if (on - x).length() <= 1e-6 {
            return (0.0, x, on);
        }
    }
    let mut best = {
        let on = point_triangle(p, v);
        ((p - on).length(), p, on)
    };
    let mut take = |s: Vec3, f: Vec3| {
        let d = (s - f).length();
        if d < best.0 {
            best = (d, s, f);
        }
    };
    let on = point_triangle(q, v);
    take(q, on);
    for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
        let (s, f) = segment_segment(p, q, a, b);
        take(s, f);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(a: Vec3, b: Vec3, c: Vec3) -> Triangle {
        Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }
    }

    /// A wall in the plane x = `x`, from z -5 to 5 and y 0 to 3.
    fn wall(x: f32) -> Vec<Triangle> {
        let p = |y, z| Vec3::new(x, y, z);
        vec![tri(p(0.0, -5.0), p(0.0, 5.0), p(3.0, 5.0)), tri(p(0.0, -5.0), p(3.0, 5.0), p(3.0, -5.0))]
    }

    fn ground() -> Vec<Triangle> {
        let p = |x, z| Vec3::new(x, 0.0, z);
        vec![tri(p(-10.0, -10.0), p(10.0, -10.0), p(10.0, 10.0)), tri(p(-10.0, -10.0), p(10.0, 10.0), p(-10.0, 10.0))]
    }

    /// A capsule walked into a wall stops a radius off it, with the wall's normal.
    #[test]
    fn a_capsule_swept_into_a_wall_stops_a_radius_off_it() {
        let world = Collision::build(&wall(2.0));
        let c = Capsule::standing(Vec3::new(0.0, 0.5, 0.0), 1.8, 0.38);
        let hit = world.sweep(c, Vec3::new(4.0, 0.0, 0.0), |_| true).expect("a contact");
        let x = 4.0 * hit.t;
        assert!((x - (2.0 - 0.38)).abs() < 2e-3, "{x}");
        assert!((hit.normal.x + 1.0).abs() < 1e-3, "{:?}", hit.normal);
    }

    /// Moving along a wall at more than the radius meets nothing; down onto the ground the bottom stops on it.
    #[test]
    fn a_sweep_along_a_wall_misses_and_a_fall_lands_on_the_ground() {
        let mut tris = wall(2.0);
        tris.extend(ground());
        let world = Collision::build(&tris);
        let c = Capsule::standing(Vec3::new(0.0, 0.5, 0.0), 1.8, 0.38);
        assert!(world.sweep(c, Vec3::new(0.0, 0.0, 4.0), |f| f.normal.y.abs() < 0.5).is_none());
        let hit = world.sweep(c, Vec3::new(0.0, -2.0, 0.0), |_| true).expect("the ground");
        assert!((2.0 * hit.t - 0.5).abs() < 2e-3, "{}", hit.t);
        assert!((hit.normal.y - 1.0).abs() < 1e-3);
    }

    /// A capsule already touching a wall slides along it (no contact) and is stopped going into it.
    #[test]
    fn a_touching_capsule_is_stopped_only_going_in() {
        let world = Collision::build(&wall(2.0));
        let c = Capsule::standing(Vec3::new(2.0 - 0.38, 0.5, 0.0), 1.8, 0.38);
        let along = world.sweep(c, Vec3::new(0.0, 0.0, 1.0), |_| true);
        assert!(along.is_none(), "{along:?}");
        let hit = world.sweep(c, Vec3::new(1.0, 0.0, 0.0), |_| true).expect("into the wall");
        assert_eq!(hit.t, 0.0);
    }

    /// The segment-triangle distance: through the face 0, over it the height, beside an edge the edge's distance.
    #[test]
    fn segment_triangle_distances() {
        let v = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        assert_eq!(segment_triangle(Vec3::new(0.2, -1.0, 0.2), Vec3::new(0.2, 1.0, 0.2), &v).0, 0.0);
        assert!((segment_triangle(Vec3::new(0.2, 0.5, 0.2), Vec3::new(0.2, 2.0, 0.2), &v).0 - 0.5).abs() < 1e-6);
        assert!((segment_triangle(Vec3::new(0.5, -1.0, -0.3), Vec3::new(0.5, 1.0, -0.3), &v).0 - 0.3).abs() < 1e-6);
    }
}
