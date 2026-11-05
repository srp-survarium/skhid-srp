//! Levels as the game answers the ragdoll: a ray (the arena's `ray_test`,
//! from the end along the query, `|to - from| + radius` long), the first
//! surface it meets, over planes and boxes (a floor, a slope, stairs, a
//! wall, a battery). And the scenes the rest tests and the renders use.

use super::*;
extern crate std;
use std::vec::Vec;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Solid {
    /// The half-space below a plane through `point` facing `normal`.
    Plane { point: Vec3, normal: Vec3 },
    /// An axis-aligned box.
    Box { lo: Vec3, hi: Vec3 },
}

pub(crate) struct Scene {
    pub name: &'static str,
    pub solids: Vec<Solid>,
    pub queries: usize,
}

impl Solid {
    /// The ray's first entry into the solid: (t, normal); a ray that starts
    /// inside meets it at 0, the normal of the nearest face.
    fn ray(&self, o: Vec3, d: Vec3, length: f32) -> Option<(f32, Vec3)> {
        match *self {
            Solid::Plane { point, normal } => {
                let h = (o - point).dot(normal);
                if h <= 0.0 {
                    return Some((0.0, normal));
                }
                let dn = d.dot(normal);
                if dn >= 0.0 {
                    return None;
                }
                let t = -h / dn;
                (t <= length).then_some((t, normal))
            }
            Solid::Box { lo, hi } => {
                let mut enter = f32::NEG_INFINITY;
                let mut leave = f32::INFINITY;
                let mut normal = Vec3::ZERO;
                let axes = [Vec3::X, Vec3::Y, Vec3::Z];
                for (k, axis) in axes.iter().enumerate() {
                    let (oo, dd, l, h) = match k {
                        0 => (o.x, d.x, lo.x, hi.x),
                        1 => (o.y, d.y, lo.y, hi.y),
                        _ => (o.z, d.z, lo.z, hi.z),
                    };
                    if dd.abs() < 1e-9 {
                        if oo < l || oo > h {
                            return None;
                        }
                        continue;
                    }
                    let (t1, t2) = ((l - oo) / dd, (h - oo) / dd);
                    let (near, far, n) = if t1 < t2 {
                        (t1, t2, -*axis)
                    } else {
                        (t2, t1, *axis)
                    };
                    if near > enter {
                        enter = near;
                        normal = n;
                    }
                    leave = leave.min(far);
                }
                if enter > leave || leave < 0.0 || enter > length {
                    return None;
                }
                if enter < 0.0 {
                    // inside: the nearest face
                    let faces = [
                        (o.x - lo.x, -Vec3::X),
                        (hi.x - o.x, Vec3::X),
                        (o.y - lo.y, -Vec3::Y),
                        (hi.y - o.y, Vec3::Y),
                        (o.z - lo.z, -Vec3::Z),
                        (hi.z - o.z, Vec3::Z),
                    ];
                    let (_, n) = faces
                        .iter()
                        .fold((f32::MAX, Vec3::Y), |b, f| if f.0 < b.0 { *f } else { b });
                    return Some((0.0, n));
                }
                Some((enter, normal))
            }
        }
    }
}

impl Level for Scene {
    fn sweep(&mut self, from: Vec3, to: Vec3, radius: f32) -> Sweep {
        self.queries += 1;
        let along = to - from;
        let Some(d) = along.direction() else {
            return Sweep::Clear;
        };
        let length = along.length() + radius;
        let best = self
            .solids
            .iter()
            .filter_map(|s| s.ray(from, d, length))
            .fold(None, |b: Option<(f32, Vec3)>, h| match b {
                Some(b) if b.0 <= h.0 => Some(b),
                _ => Some(h),
            });
        match best {
            Some((t, normal)) => Sweep::Hit {
                point: from + d * t,
                normal,
            },
            None => Sweep::Clear,
        }
    }
}

const FLOOR: Solid = Solid::Plane {
    point: Vec3::ZERO,
    normal: Vec3::Y,
};

/// A body standing at the origin on each surface, and where it falls.
pub(crate) fn scenes() -> Vec<Scene> {
    let scene = |name, solids: Vec<Solid>| Scene {
        name,
        solids,
        queries: 0,
    };
    let slope = {
        let (s, c) = crate::math::sin_cos(degrees(20.0));
        Solid::Plane {
            point: Vec3::ZERO,
            normal: Vec3::new(0.0, c, s),
        }
    };
    // stairs going down along +z from the feet: 0.18 m steps, 0.3 m deep
    let mut stairs = Vec::new();
    for k in 0..8 {
        let top = -0.18 * k as f32;
        stairs.push(Solid::Box {
            lo: Vec3::new(-3.0, -3.0, -3.0 + 0.0),
            hi: Vec3::new(3.0, top, 0.15 + 0.3 * k as f32),
        });
    }
    stairs.push(Solid::Plane {
        point: Vec3::new(0.0, -1.44, 0.0),
        normal: Vec3::Y,
    });
    std::vec![
        scene("flat", std::vec![FLOOR]),
        scene("slope", std::vec![slope]),
        scene("stairs", stairs),
        scene(
            "wall",
            std::vec![
                FLOOR,
                Solid::Box {
                    lo: Vec3::new(-3.0, 0.0, 0.45),
                    hi: Vec3::new(3.0, 3.0, 0.75),
                },
            ],
        ),
        scene(
            "battery",
            std::vec![
                FLOOR,
                Solid::Box {
                    lo: Vec3::new(-0.25, 0.0, 0.35),
                    hi: Vec3::new(0.2, 0.3, 0.75),
                },
            ],
        ),
    ]
}

/// The bones' largest move between two frames' parts.
pub(crate) fn largest_move(a: &PartPoses, b: &PartPoses, shapes: &[Body; PARTS]) -> f32 {
    let mut most = 0.0f32;
    for i in 0..PARTS {
        let (a0, a1) = shapes[i].shape.ends(a[i]);
        let (b0, b1) = shapes[i].shape.ends(b[i]);
        most = most.max((a0 - b0).length()).max((a1 - b1).length());
    }
    most
}
