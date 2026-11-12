//! One triangle of a sweep, as Survarium 0.26g's executable casts it: a port of its statically linked Bullet
//! `btTriangleConvexcastCallback::processTriangle` (0xB8670) over `btContinuousConvexCollision::calcTimeOfImpact`
//! (0xB9320) and `computeClosestPoints` (0xB8900), on the closest points of [`super::exe_gjk`], bit for bit for a shape
//! that only translates (the controller's sweeps: every recorded cast has identity bases, `docs/notes/exactness_rig.md`).
//!
//! Derived from Bullet Continuous Collision Detection and Physics Library,
//! Copyright (c) 2003-2006 Erwin Coumans http://continuousphysics.com/Bullet/
//!
//! This software is provided 'as-is', without any express or implied warranty. In no event will the authors
//! be held liable for any damages arising from the use of this software. Permission is granted to anyone to
//! use this software for any purpose, including commercial applications, and to alter it and redistribute it
//! freely, subject to the following restrictions:
//!
//! 1. The origin of this software must not be misrepresented; you must not claim that you wrote the original
//!    software. If you use this software in a product, an acknowledgment in the product documentation would
//!    be appreciated but is not required.
//! 2. Altered source versions must be plainly marked as such, and must not be misrepresented as being the
//!    original software.
//! 3. This notice may not be removed or altered from any source distribution.
//!
//! This file is an altered version: a Rust port following the order of operations of one compiled build
//! (the 0.26g executable; addresses are its virtual addresses, image base 0x10000).
//!
//! # What a translation leaves of the exe's function
//!
//! `calcTimeOfImpact` takes the two shapes' transforms at the start and the end. `calculateVelocity` (0x72EB0) gives the
//! linear velocity `(to - from) * (1 / 1)` and an angular velocity that is zero between equal bases, so the angular bound
//! (`maxAngularProjectedVelocity`) is 0 and stays in the sums as `+ 0`. `integrateTransform` (0x8B2D0) gives the origin
//! `from + velocity * lambda` and, from an identity basis and no angular velocity, the identity basis again (its
//! quaternion round trip is exact there: 16,770 recorded calls after a step all have the identity's bits). The triangle
//! (shape B) stands still at the identity.
// The exe's comparisons are ported as written: `!(a > b)` and `a > b` differ on NaN, as `comiss` branches do.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
use super::exe_gjk::{self, CastShape, ClosestPointInput, Detector, Margins, Outcome, Penetration, Transform, Triangle, Vec3};

/// 0xA2DA0C: `SIMD_EPSILON` 2^-23.
const SIMD_EPSILON: f32 = f32::from_bits(0x3400_0000);
/// 0x91A8F0: 0.001, the distance at which the cast counts as touching (`radius` in Bullet's source).
const TOUCHING: f32 = f32::from_bits(0x3A83_126F);
/// 0xA2D990: `BT_LARGE_FLOAT` 1e18, the detector's largest distance squared.
const BT_LARGE_FLOAT: f32 = f32::from_bits(0x5D5E_0B6B);
/// 0xA2D840: 1e-4, the least squared length of a hit's normal `processTriangle` reports.
const SHORT_NORMAL: f32 = f32::from_bits(0x38D1_B717);
/// `MAX_ITERATIONS`: the immediate 0x40 at 0xB97AD.
const MAX_ITERATIONS: u32 = 64;

const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A shape cast along a straight line, as the exe's sweep hands it to Bullet: the shape, its margin as the detector
/// takes it (`getMargin`, vtable +28h: the collision margin of a capsule or a cylinder, the radius of a sphere), the
/// origin of its transform at the start, and the way: the end's origin minus the start's, subtracted in `f32` as
/// `calculateVelocity` 0x72EB0 does (a caller that has the two origins subtracts them; one that has the start and a
/// displacement makes the end first and subtracts again, as the exe's callers do). Identity bases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cast {
    pub shape: CastShape,
    pub margin: f32,
    pub from: Vec3,
    pub by: Vec3,
}

/// What the cast of one triangle ends in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CastEnd {
    /// `processTriangle` reports the hit (`reportHit`, vtable +8): the share of the way, the unit normal on the triangle
    /// toward the shape, and the point on the triangle.
    Hit { fraction: f32, normal: Vec3, point: Vec3 },
    /// No report: no approach along the contact normal, the way ends first, the iteration ran out, the normal is too
    /// short, or the hit is not under the fraction the callback began with.
    Miss,
    /// The closest points at `lambda` of the way need Bullet's penetration-depth solver (the shape's core is in or within
    /// 0.01 of the triangle on a degenerate simplex), which is not ported: 24 of 57,641 recorded calls.
    NeedsPenetration { lambda: f32 },
}

/// `computeClosestPoints` 0xB8900 for the shape at `origin`: the detector is made with both shapes' margins, the
/// simplex solver and the penetration solver, `m_catchDegeneracies` 1, and the largest distance squared 1e18.
fn closest(cast: &Cast, origin: Vec3, triangle: &Triangle) -> Outcome {
    let detector = Detector {
        margins: Margins::Applied { a: cast.margin, b: 0.0 },
        penetration: Penetration::Solver { catch_degeneracies: true },
    };
    let input = ClosestPointInput {
        transform_a: Transform { basis: IDENTITY, origin },
        transform_b: Transform { basis: IDENTITY, origin: [0.0, 0.0, 0.0] },
        maximum_distance_squared: BT_LARGE_FLOAT,
    };
    exe_gjk::get_closest_points(&detector, &cast.shape, triangle, &input).outcome
}

/// 0xB8670 `btTriangleConvexcastCallback::processTriangle` for one triangle: the cast (0xB9320), then the report of a
/// hit under `hit_fraction` (the fraction the callback holds, `m_hitFraction` +D0h) with its normal made unit.
pub fn cast_triangle(cast: &Cast, triangle: &Triangle, hit_fraction: f32) -> CastEnd {
    let from = cast.from;
    // 0x72EB0: linVelA = (to - from) * (1 / timeStep), timeStep 1; linVelB = 0.
    let lin_a = [cast.by[0] * 1.0, cast.by[1] * 1.0, cast.by[2] * 1.0];
    // 0xB942D..0xB945E: relLinVel = linVelB - linVelA.
    let rel = [0.0 - lin_a[0], 0.0 - lin_a[1], 0.0 - lin_a[2]];
    let max_angular = 0.0f32;
    // 0xB946B..0xB94B7: (relLinVel's length + the angular bound) == 0: nothing moves. ORDER (asm): (x*x + z*z) + y*y.
    if ((rel[0] * rel[0] + rel[2] * rel[2]) + rel[1] * rel[1]).sqrt() + max_angular == 0.0 {
        return CastEnd::Miss;
    }
    // ORDER (asm 0xB952F..0xB957E, 0xB9615..0xB9641): relLinVel . n is (n.y*r.y + n.x*r.x) + n.z*r.z, then the bound.
    let projected = |n: Vec3| ((n[1] * rel[1] + n[0] * rel[0]) + n[2] * rel[2]) + max_angular;

    let mut lambda = 0.0f32;
    let mut last_lambda = lambda;
    let mut iterations = 0u32;
    let (mut n, mut c, mut dist) = match closest(cast, from, triangle) {
        // 0xB9541: dist = the allowed penetration (0: `convexSweepTest` 0xA34E8) + the distance.
        Outcome::Contact { normal_on_b, point_on_b, distance } => (normal_on_b, point_on_b, 0.0 + distance),
        Outcome::NoContact => return CastEnd::Miss,
        Outcome::NeedsPenetration(_) => return CastEnd::NeedsPenetration { lambda },
    };
    // 0xB9584: no report for a move away from the contact normal.
    if SIMD_EPSILON >= projected(n) {
        return CastEnd::Miss;
    }
    while dist > TOUCHING {
        let p = projected(n);
        if SIMD_EPSILON >= p {
            return CastEnd::Miss;
        }
        lambda += dist / p;
        if lambda > 1.0 || 0.0 > lambda || last_lambda >= lambda {
            return CastEnd::Miss;
        }
        last_lambda = lambda;
        // 0x8B2D0: the origin at lambda; the basis stays the identity.
        let at = [from[0] + lin_a[0] * lambda, from[1] + lin_a[1] * lambda, from[2] + lin_a[2] * lambda];
        match closest(cast, at, triangle) {
            Outcome::Contact { normal_on_b, point_on_b, distance } => {
                dist = 0.0 + distance;
                c = point_on_b;
                n = normal_on_b;
            }
            // 0xB9846: `reportFailure(-1)`.
            Outcome::NoContact => return CastEnd::Miss,
            Outcome::NeedsPenetration(_) => return CastEnd::NeedsPenetration { lambda },
        }
        iterations += 1;
        // 0xB97AD: `reportFailure(-2)`.
        if iterations > MAX_ITERATIONS {
            return CastEnd::Miss;
        }
    }
    // 0xB877A..0xB87D2: the normal's squared length over 1e-4 (ORDER (asm): (y*y + z*z) + x*x) and the fraction under
    // the callback's.
    let len2 = (n[1] * n[1] + n[2] * n[2]) + n[0] * n[0];
    if !(len2 > SHORT_NORMAL) || !(hit_fraction > lambda) {
        return CastEnd::Miss;
    }
    // 0xB87D4..0xB882B: x87 at 24-bit precision, 1 / sqrt, then each component times it.
    let rlen = 1.0 / len2.sqrt();
    CastEnd::Hit { fraction: lambda, normal: [n[0] * rlen, n[1] * rlen, n[2] * rlen], point: c }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::exe_gjk::Axis;

    fn capsule(from: Vec3, to: Vec3) -> Cast {
        let by = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        Cast { shape: CastShape::capsule([0.38, 0.52, 0.38], [1.0; 3], 0.04, Axis::Y), margin: 0.04, from, by }
    }

    /// A standing capsule dropped onto a floor stops with its bottom the touching distance over it.
    #[test]
    fn a_capsule_dropped_on_a_floor_stops_at_the_touching_distance() {
        let floor = Triangle([[-5.0, 0.0, -5.0], [5.0, 0.0, -5.0], [0.0, 0.0, 5.0]]);
        let end = cast_triangle(&capsule([0.0, 2.0, 0.0], [0.0, 0.0, 0.0]), &floor, 1.0);
        let CastEnd::Hit { fraction, normal, point } = end else { panic!("{end:?}") };
        // The centre starts 2 m up and the bottom is 0.9 under it: 1.1 m of 2 m to the floor.
        assert!((fraction - 0.55).abs() < 1e-3, "{fraction}");
        assert!((normal[1] - 1.0).abs() < 1e-6 && normal[0].abs() < 1e-3 && normal[2].abs() < 1e-3, "{normal:?}");
        assert!(point[1].abs() < 1e-6, "{point:?}");
    }

    /// A move along the floor, or away from it, reports nothing; neither does a hit at or past the callback's fraction.
    #[test]
    fn no_approach_and_a_later_hit_are_misses() {
        let floor = Triangle([[-5.0, 0.0, -5.0], [5.0, 0.0, -5.0], [0.0, 0.0, 5.0]]);
        assert_eq!(cast_triangle(&capsule([0.0, 2.0, 0.0], [1.0, 2.0, 0.0]), &floor, 1.0), CastEnd::Miss);
        assert_eq!(cast_triangle(&capsule([0.0, 2.0, 0.0], [0.0, 3.0, 0.0]), &floor, 1.0), CastEnd::Miss);
        assert_eq!(cast_triangle(&capsule([0.0, 2.0, 0.0], [0.0, 2.0, 0.0]), &floor, 1.0), CastEnd::Miss);
        assert_eq!(cast_triangle(&capsule([0.0, 2.0, 0.0], [0.0, 0.0, 0.0]), &floor, 0.5), CastEnd::Miss);
    }
}
