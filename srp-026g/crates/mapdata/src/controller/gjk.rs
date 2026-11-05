//! GJK distance between two convex shapes given by their support functions (Gilbert, Johnson and Keerthi; the sub-simplex
//! steps as Ericson, Real-Time Collision Detection 5.1 and 9.5): the shapes' cores without their margins, so a body with a
//! margin (Bullet's rounded shapes) is its core's distance less the margin, as Bullet's convex casts count it.
use crate::math::Vec3;

/// A convex core: the farthest point along a direction.
pub trait Support {
    fn support(&self, d: Vec3) -> Vec3;
}

/// A triangle.
impl Support for [Vec3; 3] {
    fn support(&self, d: Vec3) -> Vec3 {
        let (a, b, c) = (self[0].dot(d), self[1].dot(d), self[2].dot(d));
        if a >= b && a >= c { self[0] } else if b >= c { self[1] } else { self[2] }
    }
}

/// A segment (a capsule's core).
#[derive(Clone, Copy, Debug)]
pub struct Segment(pub Vec3, pub Vec3);

impl Support for Segment {
    fn support(&self, d: Vec3) -> Vec3 {
        if self.0.dot(d) >= self.1.dot(d) { self.0 } else { self.1 }
    }
}

/// An upright cylinder: its centre, half height and radius (a Bullet cylinder's core, up axis y).
#[derive(Clone, Copy, Debug)]
pub struct Cylinder {
    pub centre: Vec3,
    pub half: f32,
    pub radius: f32,
}

impl Support for Cylinder {
    fn support(&self, d: Vec3) -> Vec3 {
        let h = (d.x * d.x + d.z * d.z).sqrt();
        let (x, z) = if h > 1e-12 { (d.x / h * self.radius, d.z / h * self.radius) } else { (0.0, 0.0) };
        let y = if d.y >= 0.0 { self.half } else { -self.half };
        self.centre + Vec3::new(x, y, z)
    }
}

/// A point of the Minkowski difference with the two points it came from.
#[derive(Clone, Copy, Debug)]
struct Vertex {
    w: Vec3,
    a: Vec3,
    b: Vec3,
}

/// The distance between cores `a` and `b`, with the closest point on each. Zero (with the points equal or meaningless) when
/// they overlap.
pub fn distance(a: &impl Support, b: &impl Support, guess: Vec3) -> (f32, Vec3, Vec3) {
    let mut d = if guess.length() > 1e-9 { guess } else { Vec3::new(1.0, 0.0, 0.0) };
    let vertex = |d: Vec3| {
        let (pa, pb) = (a.support(-d), b.support(d));
        Vertex { w: pa - pb, a: pa, b: pb }
    };
    let mut simplex: Vec<Vertex> = vec![vertex(d)];
    let mut v = simplex[0].w;
    let (mut ca, mut cb) = (simplex[0].a, simplex[0].b);
    for _ in 0..64 {
        let len2 = v.dot(v);
        if len2 < 1e-14 {
            return (0.0, ca, cb);
        }
        d = v;
        let s = vertex(d);
        // No progress toward the origin: v is the closest point.
        if len2 - v.dot(s.w) <= 1e-6 * len2.max(1e-6) || simplex.iter().any(|p| (p.w - s.w).length() < 1e-9) {
            break;
        }
        simplex.push(s);
        let (nv, kept, pa, pb) = closest(&simplex);
        simplex = kept;
        if nv.dot(nv) >= len2 {
            break;
        }
        v = nv;
        ca = pa;
        cb = pb;
        if simplex.len() == 4 {
            return (0.0, ca, cb);
        }
    }
    (v.length(), ca, cb)
}

/// The point of the simplex nearest the origin, the vertices that span it, and the matching points on the two shapes.
fn closest(s: &[Vertex]) -> (Vec3, Vec<Vertex>, Vec3, Vec3) {
    match s.len() {
        1 => (s[0].w, s.to_vec(), s[0].a, s[0].b),
        2 => segment(s[0], s[1]),
        3 => triangle(s[0], s[1], s[2]),
        _ => tetrahedron(s[0], s[1], s[2], s[3]),
    }
}

fn mix(vs: &[(Vertex, f32)]) -> (Vec3, Vec3, Vec3) {
    let mut w = Vec3::ZERO;
    let mut a = Vec3::ZERO;
    let mut b = Vec3::ZERO;
    for &(v, t) in vs {
        w = w + v.w * t;
        a = a + v.a * t;
        b = b + v.b * t;
    }
    (w, a, b)
}

fn segment(p: Vertex, q: Vertex) -> (Vec3, Vec<Vertex>, Vec3, Vec3) {
    let ab = q.w - p.w;
    let t = (-p.w).dot(ab);
    let len2 = ab.dot(ab);
    if t <= 0.0 || len2 < 1e-18 {
        return (p.w, vec![p], p.a, p.b);
    }
    if t >= len2 {
        return (q.w, vec![q], q.a, q.b);
    }
    let t = t / len2;
    let (w, a, b) = mix(&[(p, 1.0 - t), (q, t)]);
    (w, vec![p, q], a, b)
}

fn triangle(p: Vertex, q: Vertex, r: Vertex) -> (Vec3, Vec<Vertex>, Vec3, Vec3) {
    // Ericson 5.1.5 with the origin as the point, keeping the feature's vertices.
    let (a, b, c) = (p.w, q.w, r.w);
    let (ab, ac, ap) = (b - a, c - a, -a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, vec![p], p.a, p.b);
    }
    let bp = -b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (b, vec![q], q.a, q.b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let t = d1 / (d1 - d3);
        let (w, x, y) = mix(&[(p, 1.0 - t), (q, t)]);
        return (w, vec![p, q], x, y);
    }
    let cp = -c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c, vec![r], r.a, r.b);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let t = d2 / (d2 - d6);
        let (w, x, y) = mix(&[(p, 1.0 - t), (r, t)]);
        return (w, vec![p, r], x, y);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let t = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let (w, x, y) = mix(&[(q, 1.0 - t), (r, t)]);
        return (w, vec![q, r], x, y);
    }
    let sum = va + vb + vc;
    if sum.abs() < 1e-30 {
        return segment(p, q);
    }
    let (v, w) = (vb / sum, vc / sum);
    let (pt, x, y) = mix(&[(p, 1.0 - v - w), (q, v), (r, w)]);
    (pt, vec![p, q, r], x, y)
}

fn tetrahedron(p: Vertex, q: Vertex, r: Vertex, s: Vertex) -> (Vec3, Vec<Vertex>, Vec3, Vec3) {
    // The origin outside a face (on the other side from the fourth vertex) is nearest that face's region; inside all, 0.
    let faces = [(p, q, r, s), (p, r, s, q), (p, s, q, r), (q, s, r, p)];
    let volume = (q.w - p.w).cross(r.w - p.w).dot(s.w - p.w);
    let scale = [q.w - p.w, r.w - p.w, s.w - p.w].iter().map(|e| e.length()).fold(0.0f32, f32::max);
    if volume.abs() <= 1e-7 * scale * scale * scale {
        // Flat: no inside; the nearest of its faces.
        return faces
            .iter()
            .map(|&(a, b, c, _)| triangle(a, b, c))
            .min_by(|x, y| x.0.dot(x.0).total_cmp(&y.0.dot(y.0)))
            .unwrap_or_else(|| triangle(p, q, r));
    }
    let mut best: Option<(Vec3, Vec<Vertex>, Vec3, Vec3)> = None;
    let mut inside = true;
    for (a, b, c, d) in faces {
        let n = (b.w - a.w).cross(c.w - a.w);
        let (o, dd) = (n.dot(-a.w), n.dot(d.w - a.w));
        if o * dd < 0.0 {
            inside = false;
            let cand = triangle(a, b, c);
            if best.as_ref().is_none_or(|bst| cand.0.dot(cand.0) < bst.0.dot(bst.0)) {
                best = Some(cand);
            }
        }
    }
    if inside {
        return (Vec3::ZERO, vec![p, q, r, s], Vec3::ZERO, Vec3::ZERO);
    }
    best.unwrap_or_else(|| triangle(p, q, r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gjk_matches_known_distances() {
        let tri = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        // A segment above the triangle: the height.
        let (d, _, _) = distance(&Segment(Vec3::new(0.2, 0.5, 0.2), Vec3::new(0.2, 2.0, 0.2)), &tri, Vec3::Y);
        assert!((d - 0.5).abs() < 1e-4, "{d}");
        // A cylinder of radius 0.3 whose axis is 1 m from a wall's plane: 0.7.
        let wall = [Vec3::new(1.0, -5.0, -5.0), Vec3::new(1.0, 5.0, -5.0), Vec3::new(1.0, 0.0, 5.0)];
        let cyl = Cylinder { centre: Vec3::ZERO, half: 0.5, radius: 0.3 };
        let (d, pa, pb) = distance(&cyl, &wall, Vec3::new(-1.0, 0.0, 0.0));
        assert!((d - 0.7).abs() < 1e-4, "{d} {pa:?} {pb:?}");
        // The cylinder's bottom rim over a floor 0.2 under it.
        let floor = [Vec3::new(-5.0, -0.7, -5.0), Vec3::new(5.0, -0.7, -5.0), Vec3::new(0.0, -0.7, 5.0)];
        let (d, _, _) = distance(&cyl, &floor, Vec3::Y);
        assert!((d - 0.2).abs() < 1e-4, "{d}");
        // A riser face under the cylinder, its top edge 0.3 under the cylinder's bottom and 0.1 beside its side: the corner gap.
        let cyl = Cylinder { centre: Vec3::new(1.52, 1.3, 0.0), half: 0.5, radius: 0.38 };
        let riser = [Vec3::new(2.0, 0.0, -10.0), Vec3::new(2.0, 0.5, -10.0), Vec3::new(2.0, 0.5, 10.0)];
        let (d, pa, pb) = distance(&cyl, &riser, cyl.centre - Vec3::new(2.0, 0.3, 0.0));
        let want = (0.1f32 * 0.1 + 0.3 * 0.3).sqrt();
        assert!((d - want).abs() < 1e-3, "{d} vs {want}: {pa:?} {pb:?}");
        // Overlapping: 0.
        let (d, _, _) = distance(&Segment(Vec3::new(0.2, -1.0, 0.2), Vec3::new(0.2, 1.0, 0.2)), &tri, Vec3::Y);
        assert!(d < 1e-4, "{d}");
    }
}
