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

use crate::Triangle;
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

/// One triangle of the collision: its corners and unit normal (zero for a degenerate one).
#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub v: [Vec3; 3],
    pub normal: Vec3,
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
                Face { v: t.v, normal: if l > 1e-12 { n * (1.0 / l) } else { Vec3::ZERO } }
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
        let r = capsule.radius;
        let pad = Vec3::new(r, r, r);
        let (lo, hi) = aabb([capsule.low, capsule.high, capsule.low + by, capsule.high + by]);
        let mut best: Option<Contact> = None;
        self.faces_in(lo - pad, hi + pad, |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO || !keep(face) {
                return;
            }
            let limit = best.map_or(1.0, |b| b.t);
            if let Some(c) = advance(capsule, by, face, limit) {
                best = Some(Contact { face: f, ..c });
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
            let (d, on_seg, on_face) = segment_triangle(capsule.low, capsule.high, &face.v);
            let gap = d - capsule.radius;
            if gap <= reach && best.is_none_or(|b| gap < b.0) {
                let dir = if d > 1e-6 { (on_seg - on_face) * (1.0 / d) } else { face.normal };
                best = Some((gap, on_face, dir, f));
            }
        });
        best
    }
}

/// The conservative advancement of `capsule` moved by `by` against one face, up to share `limit` of the move.
fn advance(capsule: Capsule, by: Vec3, face: &Face, limit: f32) -> Option<Contact> {
    let speed = by.length();
    let r = capsule.radius;
    let (d0, s0, f0) = segment_triangle(capsule.low, capsule.high, &face.v);
    if d0 - r <= CONTACT_M {
        // Touching or overlapping at the start: a contact only when the move goes in.
        let n = if d0 > 1e-6 { (s0 - f0) * (1.0 / d0) } else { face.normal };
        return (by.dot(n) < -GOING_IN * speed).then_some(Contact { t: 0.0, point: f0, normal: n, face: 0 });
    }
    if speed <= 0.0 {
        return None;
    }
    let mut t = 0.0f32;
    for _ in 0..ADVANCE_STEPS {
        let c = capsule.moved(by * t);
        let (d, s, f) = segment_triangle(c.low, c.high, &face.v);
        let gap = d - r;
        if gap <= CONTACT_M {
            let n = if d > 1e-6 { (s - f) * (1.0 / d) } else { face.normal };
            return Some(Contact { t, point: f, normal: n, face: 0 });
        }
        t += gap / speed;
        if t > limit {
            return None;
        }
    }
    None
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
    use crate::WalkerBehaviour;

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
