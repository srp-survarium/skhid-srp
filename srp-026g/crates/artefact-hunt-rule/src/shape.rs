//! What the use key reaches: the view's segment against the artefact's
//! sphere and a base's shapes.
//!
//! The game decides a take and a delivery in the simulation, in every game
//! from the same numbers (the eye's matrix, the rule's artefact, the map's
//! config): single precision, no square root, no division but the box's
//! slabs, each operation in this order, so every game gets the same answer
//! from the same code.

/// A config's shape kind, in the form Research's `capture_zones` use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// `half[0]` the radius.
    Sphere,
    Box,
    /// `half[0]` the radius, `half[1]` the half height along its local y.
    Cylinder,
}

impl ShapeKind {
    pub const fn from_raw(raw: u32) -> Option<ShapeKind> {
        match raw {
            0 => Some(ShapeKind::Sphere),
            1 => Some(ShapeKind::Box),
            2 => Some(ShapeKind::Cylinder),
            _ => None,
        }
    }
}

/// A shape in world space: its centre, its three local axes (local = axes x
/// (p - centre)) and half extents, each a finite number and the halves not
/// negative (`Shape::new`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    kind: ShapeKind,
    centre: [f32; 3],
    axes: [[f32; 3]; 3],
    half: [f32; 3],
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// A view: the eye and its forward axis (the eye matrix's third row, unit
/// length), and how far the use key reaches along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: [f32; 3],
    pub forward: [f32; 3],
}

impl Shape {
    /// `None` for a number that is no number or a negative half.
    pub fn new(
        kind: ShapeKind,
        centre: [f32; 3],
        axes: [[f32; 3]; 3],
        half: [f32; 3],
    ) -> Option<Shape> {
        let ok = finite(&centre)
            && axes.iter().all(|axis| finite(axis))
            && finite(&half)
            && half.iter().all(|&h| h >= 0.0);
        ok.then_some(Shape {
            kind,
            centre,
            axes,
            half,
        })
    }

    /// A sphere about `centre`.
    pub fn sphere(centre: [f32; 3], radius: f32) -> Option<Shape> {
        Shape::new(
            ShapeKind::Sphere,
            centre,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [radius, 0.0, 0.0],
        )
    }

    /// Whether the view's segment from the eye, `length` long, enters the
    /// shape (or starts in it).
    pub fn reached(&self, view: &View, length: f32) -> bool {
        if length.is_nan() || length < 0.0 {
            return false;
        }
        // The segment in the shape's frame: from o, along d, t in 0..=length.
        let rel = [
            view.eye[0] - self.centre[0],
            view.eye[1] - self.centre[1],
            view.eye[2] - self.centre[2],
        ];
        let mut o = [0.0f32; 3];
        let mut d = [0.0f32; 3];
        for axis in 0..3 {
            let a = &self.axes[axis];
            o[axis] = a[0] * rel[0] + a[1] * rel[1] + a[2] * rel[2];
            d[axis] = a[0] * view.forward[0] + a[1] * view.forward[1] + a[2] * view.forward[2];
        }
        match self.kind {
            ShapeKind::Sphere => {
                // |o + t d|^2 <= r^2 for some t of the segment.
                let a = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                let b = o[0] * d[0] + o[1] * d[1] + o[2] * d[2];
                let c = o[0] * o[0] + o[1] * o[1] + o[2] * o[2] - self.half[0] * self.half[0];
                quadratic_reaches(a, b, c, 0.0, length)
            }
            ShapeKind::Box => slabs(&o, &d, &self.half, [true; 3], length).is_some(),
            ShapeKind::Cylinder => {
                // Along y the slab; across, the circle in x, z.
                let Some((from, to)) = slabs(&o, &d, &self.half, [false, true, false], length)
                else {
                    return false;
                };
                let a = d[0] * d[0] + d[2] * d[2];
                let b = o[0] * d[0] + o[2] * d[2];
                let c = o[0] * o[0] + o[2] * o[2] - self.half[0] * self.half[0];
                quadratic_reaches(a, b, c, from, to)
            }
        }
    }
}

/// Whether `a t^2 + 2 b t + c <= 0` for some t of `from..=to` (`a` not
/// negative): its least there, at the vertex or an end.
fn quadratic_reaches(a: f32, b: f32, c: f32, from: f32, to: f32) -> bool {
    let at = |t: f32| (a * t + 2.0 * b) * t + c;
    if at(from) <= 0.0 || at(to) <= 0.0 {
        return true;
    }
    // The vertex -b / a inside the range: a t + b = 0 there, the least
    // value c - b^2 / a, so it is reached when a c <= b^2.
    a > 0.0 && -b >= a * from && -b <= a * to && a * c <= b * b
}

/// The part of `0..=length` inside the box's slabs on the axes `on`, as
/// Kay and Kajiya's slab test; `None` when it is empty.
fn slabs(
    o: &[f32; 3],
    d: &[f32; 3],
    half: &[f32; 3],
    on: [bool; 3],
    length: f32,
) -> Option<(f32, f32)> {
    let mut from = 0.0f32;
    let mut to = length;
    for axis in 0..3 {
        if !on[axis] {
            continue;
        }
        let (lo, hi) = (-half[axis], half[axis]);
        if d[axis] == 0.0 {
            if o[axis] < lo || o[axis] > hi {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((lo - o[axis]) / d[axis], (hi - o[axis]) / d[axis]);
        if t0 > t1 {
            core::mem::swap(&mut t0, &mut t1);
        }
        if t0 > from {
            from = t0;
        }
        if t1 < to {
            to = t1;
        }
        if from.is_nan() || to.is_nan() || from > to {
            return None;
        }
    }
    Some((from, to))
}

/// How far a ray hit is along the view (the forward axis is unit length:
/// the hit point's part along it).
pub fn along(view: &View, point: [f32; 3]) -> f32 {
    (point[0] - view.eye[0]) * view.forward[0]
        + (point[1] - view.eye[1]) * view.forward[1]
        + (point[2] - view.eye[2]) * view.forward[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(eye: [f32; 3], forward: [f32; 3]) -> View {
        View { eye, forward }
    }

    #[test]
    fn a_sphere_is_reached_within_the_segment_only() {
        let s = Shape::sphere([0.0, 0.0, 2.0], 0.3).unwrap();
        let v = view([0.0; 3], [0.0, 0.0, 1.0]);
        assert!(s.reached(&v, 2.0));
        assert!(s.reached(&v, 1.71));
        assert!(!s.reached(&v, 1.69));
        // Beside the view by more than the radius.
        let off = view([0.31, 0.0, 0.0], [0.0, 0.0, 1.0]);
        assert!(!s.reached(&off, 3.0));
        let near = view([0.29, 0.0, 0.0], [0.0, 0.0, 1.0]);
        assert!(s.reached(&near, 3.0));
        // Behind the eye.
        assert!(!s.reached(&view([0.0; 3], [0.0, 0.0, -1.0]), 5.0));
        // The eye inside it.
        assert!(s.reached(&view([0.0, 0.0, 2.1], [1.0, 0.0, 0.0]), 0.0));
        assert!(!s.reached(&v, f32::NAN));
    }

    #[test]
    fn a_box_is_reached_by_its_slabs_and_turned_by_its_axes() {
        let b = Shape::new(
            ShapeKind::Box,
            [5.0, 0.0, 0.0],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.5, 0.5, 0.5],
        )
        .unwrap();
        let v = view([3.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        assert!(b.reached(&v, 2.0) && b.reached(&v, 1.5) && !b.reached(&v, 1.49));
        assert!(!b.reached(&view([3.0, 0.6, 0.0], [1.0, 0.0, 0.0]), 9.0));
        // A box turned 45 degrees about y: its corner points at the eye.
        let h = core::f32::consts::FRAC_1_SQRT_2;
        let turned = Shape::new(
            ShapeKind::Box,
            [5.0, 0.0, 0.0],
            [[h, 0.0, -h], [0.0, 1.0, 0.0], [h, 0.0, h]],
            [0.5, 0.5, 0.5],
        )
        .unwrap();
        assert!(turned.reached(&v, 1.30) && !turned.reached(&v, 1.29));
        assert_eq!(
            Shape::new(ShapeKind::Box, [0.0; 3], [[0.0; 3]; 3], [-1.0, 0.0, 0.0]),
            None
        );
        assert_eq!(
            Shape::new(
                ShapeKind::Box,
                [f32::INFINITY, 0.0, 0.0],
                [[0.0; 3]; 3],
                [0.0; 3]
            ),
            None
        );
    }

    #[test]
    fn a_cylinder_is_its_circle_and_its_height() {
        let c = Shape::new(
            ShapeKind::Cylinder,
            [0.0, 1.0, 3.0],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 0.5, 0.0],
        )
        .unwrap();
        let v = view([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        assert!(c.reached(&v, 2.0) && !c.reached(&v, 1.99));
        // Over its top.
        assert!(!c.reached(&view([0.0, 1.6, 0.0], [0.0, 0.0, 1.0]), 9.0));
        // Down through its top, from above.
        assert!(c.reached(&view([0.0, 3.0, 3.0], [0.0, -1.0, 0.0]), 1.5));
        assert!(!c.reached(&view([0.0, 3.0, 3.0], [0.0, -1.0, 0.0]), 1.49));
        // Past its side.
        assert!(!c.reached(&view([1.01, 1.0, 0.0], [0.0, 0.0, 1.0]), 9.0));
    }

    #[test]
    fn a_hit_is_measured_along_the_view() {
        let v = view([1.0, 2.0, 3.0], [0.0, 1.0, 0.0]);
        assert_eq!(along(&v, [5.0, 3.5, 3.0]), 1.5);
    }
}
