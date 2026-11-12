//! The time of impact of a translating shape and a standing one, as Survarium 0.26g's executable casts it: a port of
//! its statically linked Bullet `btContinuousConvexCollision::calcTimeOfImpact` (0xB9320) over `computeClosestPoints`
//! (0xB8900), `btTransformUtil::integrateTransform` (0x8B2D0) and the closest points of [`super::exe_gjk`], bit for bit
//! for every shape B the controller's sweeps meet (a mesh's triangle, a box, a sphere, a cylinder) under its own
//! transform, rotated or not; and `btTriangleConvexcastCallback::processTriangle` (0xB8670) on top of it for a mesh's
//! triangle.
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
//! # What a translating A and a standing B leave of the exe's function
//!
//! `calcTimeOfImpact` takes both shapes' transforms at the start and the end. Every call of the controller's sweeps
//! (22,727 recorded, `toi_corpus_live.py`) has shape A's two bases the identity's bits and shape B's two transforms
//! equal (B stands still), often with a rotated basis (8,056 calls: placed meshes, boxes, cylinders).
//!
//! * `calculateVelocity` (0x72EB0) gives each shape's linear velocity `(to - from) * (1 / 1)`: A's way, B's +0. Its
//!   angular velocity is `axis * angle * (1 / 1)` from `calculateDiffAxisAngle` (0x72FB0): the rotation of
//!   `to.basis * inverse(from.basis)` as a quaternion (`getRotation` 0x72B90), normalised, `angle = 2 acos(w)` with
//!   `w` clamped to [-1, 1] (0x733B9..0x73408). For equal bases the product is the identity up to rounding, so the
//!   quaternion's `x, y, z` are rounding noise and `w` is within a few ulps of 1; its normalisation
//!   `w * (1 / sqrt(((x*x + y*y) + z*z) + w*w))` comes back 1 or over (and is clamped to 1) whenever the noise's
//!   squares are under half an ulp of `w*w` (checked in f32 for every `w` within 64 ulps of 1 and noise up to 1e-4,
//!   258,000 cases each), so the angle is `2 acos(1) = +0`. The axis is then (1, 0, 0) when its squared length is
//!   under 2^-46, and the angular velocity `axis * 0` is zeros (+0's unless an axis component is negative). Every
//!   recorded call has both angular velocities and the angular bound `maxAngularProjectedVelocity` (0xB9467) as +0's
//!   bits (22,727 of 22,727; the checker requires it of each record). So the angular velocities are taken as +0,
//!   the angular bound is +0 and stays in the sums as `+ 0`, and the shapes' angular motion discs
//!   (`getAngularMotionDisc`, vtable +0Ch) do not matter.
//! * `integrateTransform` (0x8B2D0) with no angular velocity still sends the basis through a quaternion: it takes
//!   the Taylor branch (`fAngle` 0 < 0.001), so the rotation step is the quaternion (+0, +0, +0, cos 0 = 1), which is
//!   multiplied onto `getRotation(basis)`, normalised and turned back into a basis (`setRotation` 0x72A50). The
//!   identity comes back as itself; a rotated basis comes back with other bits (2,394 of the 2,434 recorded loops on
//!   a rotated B). So shape B's transform in the loop is not `fromB`: this is ported ([`integrate_transform`]).
//! * `result.m_debugDrawer` (+B4h) is null in every recorded call (its drawing does not change the answer), and the
//!   result's `DebugDraw` and `reportFailure` (vtable +0, +8) are the base class's empty functions.
// The exe's comparisons are ported as written: `!(a > b)` and `a > b` differ on NaN, as `comiss` branches do.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
use super::exe_epa;
use super::exe_gjk::{
    self as gjk, CastShape, ClosestPointInput, Detector, Margins, Outcome, Penetration, ShapeB, Transform, Triangle, Vec3,
};

/// 0xA2DA0C: `SIMD_EPSILON` 2^-23.
const SIMD_EPSILON: f32 = f32::from_bits(0x3400_0000);
/// 0x91A8F0: 0.001, the distance at which the cast counts as touching (`radius` in Bullet's source).
const TOUCHING: f32 = f32::from_bits(0x3A83_126F);
/// 0xA2D990: `BT_LARGE_FLOAT` 1e18: the detector's largest distance squared, and the empty point collector's distance.
const BT_LARGE_FLOAT: f32 = f32::from_bits(0x5D5E_0B6B);
/// 0xA2D840: 1e-4, the least squared length of a hit's normal `processTriangle` reports.
const SHORT_NORMAL: f32 = f32::from_bits(0x38D1_B717);
/// `MAX_ITERATIONS`: the immediate 0x40 at 0xB97AD.
const MAX_ITERATIONS: u32 = 64;
/// 0xA70884: 1.0.
const ONE: f32 = 1.0;
/// 0x927A44: 0.5.
const HALF: f32 = 0.5;
/// 0x9CC87C: 2.0, `setRotation`'s numerator.
const TWO: f32 = 2.0;
/// 0xA0D984: 0.0, `getRotation`'s bound on the trace.
const ZERO: f32 = 0.0;

/// The identity transform: a world-space shape B (the level mesh's triangles).
pub const IDENTITY: Transform =
    Transform { basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], origin: [0.0, 0.0, 0.0] };

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

/// Why `calcTimeOfImpact` returns false (AL 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Miss {
    /// 0xB94B7: the relative velocity's length plus the angular bound is 0: nothing moves.
    NoMotion,
    /// 0xB9520: the first closest-point call left the collector empty.
    NoClosestPoints,
    /// 0xB9595 (at the start) or 0xB9652 (on the way): the relative velocity along the normal, plus the angular bound,
    /// is at most `SIMD_EPSILON`: a move away from the contact normal.
    Receding,
    /// 0xB966B: lambda went over 1: the way ends first.
    PastTheEnd,
    /// 0xB9677: lambda went under 0.
    BeforeTheStart,
    /// 0xB9686: lambda is not past the last one.
    NoProgress,
    /// 0xB978C: a later closest-point call left the collector empty; the result's `reportFailure(-1, numIter)`
    /// (0xB9846), and the function leaves at 0xB985A.
    LostContact,
    /// 0xB97CF: more than 64 rounds; the result's `reportFailure(-2, numIter)` (0xB982F).
    IterationLimit,
}

impl Miss {
    /// The `ret 14h` the exe leaves by: 0xB985A after `reportFailure(-1)`, 0xB9843 for every other false.
    pub fn exit(self) -> u32 {
        match self {
            Miss::LostContact => 0xB985A,
            _ => 0xB9843,
        }
    }
}

/// What `calcTimeOfImpact` leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Toi {
    /// AL 1 (0xB982C): the result's `m_fraction` (+B0h), `m_normal` (+90h: the closest points' normal on B, as the
    /// collector holds it, not yet made unit by any caller) and `m_hitPoint` (+A0h: the point on B), in world space.
    Hit { fraction: f32, normal: Vec3, point: Vec3 },
    /// AL 0, and why; the result is not written.
    Miss(Miss),
}

/// What the cast of one triangle ends in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CastEnd {
    /// `processTriangle` reports the hit (`reportHit`, vtable +8): the share of the way, the unit normal on the triangle
    /// toward the shape, and the point on the triangle, in world space.
    Hit { fraction: f32, normal: Vec3, point: Vec3 },
    /// No report: no approach along the contact normal, the way ends first, the iteration ran out, the normal is too
    /// short, or the hit is not under the fraction the callback began with.
    Miss,
}

/// A quaternion (x, y, z, w), as `btQuaternion` keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Quaternion {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

/// 0x72B90 `btMatrix3x3::getRotation` (ESI the matrix, stack: the quaternion out; `ret 4`).
fn get_rotation(m: &[[f32; 3]; 3]) -> Quaternion {
    // 0x72BA2..0x72BAE: (m00 + m11) + m22, the source's order.
    let trace = (m[0][0] + m[1][1]) + m[2][2];
    if trace > ZERO {
        // 0x72BB8..0x72C0D: x87 sqrt (24-bit), stored as f32 (0x72BCB), times 0.5; 0.5 / s in SSE.
        let s = (trace + ONE).sqrt();
        let w = s * HALF;
        let r = HALF / s;
        return Quaternion {
            x: (m[2][1] - m[1][2]) * r,
            y: (m[0][2] - m[2][0]) * r,
            z: (m[1][0] - m[0][1]) * r,
            w,
        };
    }
    // 0x72C16..0x72C33: the largest diagonal element (the first of equal ones, as Bullet's `<` chain picks it).
    let i = if m[1][1] > m[0][0] {
        if m[2][2] > m[1][1] {
            2
        } else {
            1
        }
    } else if m[2][2] > m[0][0] {
        2
    } else {
        0
    };
    let j = (i + 1) % 3;
    let k = (i + 2) % 3;
    // 0x72C50..0x72C88: ((m[i][i] - m[j][j]) - m[k][k]) + 1, the source's order.
    let s = (((m[i][i] - m[j][j]) - m[k][k]) + ONE).sqrt();
    let r = HALF / s;
    let mut t = [0.0f32; 3];
    t[i] = s * HALF;
    t[j] = (m[j][i] + m[i][j]) * r;
    t[k] = (m[k][i] + m[i][k]) * r;
    Quaternion { x: t[0], y: t[1], z: t[2], w: (m[k][j] - m[j][k]) * r }
}

/// 0x72A50 `btMatrix3x3::setRotation` (ECX the quaternion, EAX the matrix out).
fn set_rotation(q: Quaternion) -> [[f32; 3]; 3] {
    // 0x72A66..0x72A8A: ((x*x + y*y) + z*z) + w*w, the source's order.
    let d = ((q.x * q.x + q.y * q.y) + q.z * q.z) + q.w * q.w;
    let s = TWO / d;
    let xs = q.x * s;
    let ys = q.y * s;
    let zs = q.z * s;
    let wx = q.w * xs;
    let wy = q.w * ys;
    let wz = q.w * zs;
    let xx = q.x * xs;
    let xy = q.x * ys;
    let xz = q.x * zs;
    let yy = q.y * ys;
    let yz = q.y * zs;
    let zz = q.z * zs;
    [
        [ONE - (yy + zz), xy - wz, xz + wy],
        [xy + wz, ONE - (xx + zz), yz - wx],
        [xz - wy, yz + wx, ONE - (xx + yy)],
    ]
}

/// The quaternion product `d * o` (Bullet's `operator*`), inlined in 0x8B2D0 (0x8B4BA..0x8B595).
fn multiply(d: Quaternion, o: Quaternion) -> Quaternion {
    Quaternion {
        // ORDER (asm 0x8B4D8..0x8B506): ((o.z*d.y + o.w*d.x) + d.w*o.x) - o.y*d.z; the source's is
        // ((d.w*o.x + d.x*o.w) + d.y*o.z) - d.z*o.y.
        x: ((o.z * d.y + o.w * d.x) + d.w * o.x) - o.y * d.z,
        // ORDER (asm 0x8B4F7..0x8B536): ((o.y*d.w + o.w*d.y) + d.z*o.x) - o.z*d.x; the source's is
        // ((d.w*o.y + d.y*o.w) + d.z*o.x) - d.x*o.z.
        y: ((o.y * d.w + o.w * d.y) + d.z * o.x) - o.z * d.x,
        // ORDER (asm 0x8B53A..0x8B595): ((o.z*d.w + o.y*d.x) + o.w*d.z) - d.y*o.x; the source's is
        // ((d.w*o.z + d.z*o.w) + d.x*o.y) - d.y*o.x.
        z: ((o.z * d.w + o.y * d.x) + o.w * d.z) - d.y * o.x,
        // 0x8B555..0x8B586: ((o.w*d.w - d.x*o.x) - o.y*d.y) - o.z*d.z, the source's order.
        w: ((o.w * d.w - d.x * o.x) - o.y * d.y) - o.z * d.z,
    }
}

/// `btQuaternion::normalize`, inlined in 0x8B2D0 (0x8B58A..0x8B62E).
fn normalize(q: Quaternion) -> Quaternion {
    // ORDER (asm 0x8B58A..0x8B5C9): ((w*w + z*z) + y*y) + x*x; the source's is ((x*x + y*y) + z*z) + w*w.
    let len2 = ((q.w * q.w + q.z * q.z) + q.y * q.y) + q.x * q.x;
    // x87 at 24-bit precision: 1 / sqrt, stored as f32 (0x8B5EA), then each component times it in SSE.
    let r = ONE / len2.sqrt();
    Quaternion { x: q.x * r, y: q.y * r, z: q.z * r, w: q.w * r }
}

/// 0x8B2D0 `btTransformUtil::integrateTransform` (ECX the transform, EAX the linear velocity, EDI the angular
/// velocity, stack: the step, the transform out) for an angular velocity of +0 (the only one a translating shape
/// and a standing one have: see the module's notes) and a step over 0 (every lambda the cast integrates to).
///
/// The origin is `origin + velocity * step` (0x8B2D9..0x8B346). With no angular velocity, `fAngle` is
/// `sqrt(0) = 0`, under 0.001: the Taylor branch gives the rotation step's axis `angvel * (step * 0.5 - ...)`, +0's
/// for a step over 0, and its `w` `cos(0 * step * 0.5)` = 1 (0x39000, `fcos`). That quaternion is multiplied onto
/// the basis's own (`getRotation`), normalised, and made a basis again (`setRotation`).
pub fn integrate_transform(transform: &Transform, linear_velocity: Vec3, step: f32) -> Transform {
    let o = transform.origin;
    let v = linear_velocity;
    let origin = [o[0] + v[0] * step, o[1] + v[1] * step, o[2] + v[2] * step];
    let rotation_step = Quaternion { x: 0.0, y: 0.0, z: 0.0, w: ONE };
    let rotation = normalize(multiply(rotation_step, get_rotation(&transform.basis)));
    Transform { basis: set_rotation(rotation), origin }
}

/// The way from one origin to another, `to - from` (0x72EBC..0x72EE2).
fn way(from: Vec3, to: Vec3) -> Vec3 {
    [to[0] - from[0], to[1] - from[1], to[2] - from[2]]
}

/// 0x72EB0 `btTransformUtil::calculateVelocity`'s linear velocity: the way times `1 / timeStep` (0x72EC6..0x72F27).
fn linear_velocity(way: Vec3, time_step: f32) -> Vec3 {
    let inverse = ONE / time_step;
    [way[0] * inverse, way[1] * inverse, way[2] * inverse]
}

/// 0xB9320 / 0xB9352: `fld1`, the time step `calcTimeOfImpact` gives `calculateVelocity`.
const TIME_STEP: f32 = 1.0;

/// 0xB8900 `btContinuousConvexCollision::computeClosestPoints` (EDI transform A, ESI transform B, stack: the caster,
/// the point collector): the simplex solver reset, a detector of the two shapes with each one's own `getMargin`
/// (vtable +28h), the caster's penetration solver and `m_catchDegeneracies` 1, the largest distance squared 1e18;
/// then the collector's `addContactPoint` (0xB88B0), which keeps a point closer than what it holds (1e18 when new).
/// Where the detector goes on to the penetration-depth solver (the shapes' cores overlap, or are within 0.01 m on a
/// degenerate simplex) the contact is the solver's ([`exe_epa::detector_tail`]: the exe's bits on 6,146 recorded calls).
fn closest<B: ShapeB>(
    cast: &Cast,
    transform_a: Transform,
    other: &B,
    margin_b: f32,
    transform_b: Transform,
) -> Collector {
    let detector = Detector {
        margins: Margins::Applied { a: cast.margin, b: margin_b },
        penetration: Penetration::Solver { catch_degeneracies: true },
    };
    let input = ClosestPointInput { transform_a, transform_b, maximum_distance_squared: BT_LARGE_FLOAT };
    let contact = match gjk::get_closest_points(&detector, &cast.shape, other, &input).outcome {
        Outcome::Contact { normal_on_b, point_on_b, distance } => Some((normal_on_b, point_on_b, distance)),
        Outcome::NoContact => None,
        Outcome::NeedsPenetration(start) => {
            exe_epa::detector_tail(
                exe_epa::Convex { shape: &cast.shape, margin: cast.margin },
                &start.local_a,
                exe_epa::Convex { shape: &exe_epa::B(other), margin: margin_b },
                &start.local_b,
                start.gjk,
                start.margin_sum,
                margin_b,
                start.position_offset,
                BT_LARGE_FLOAT,
            )
            .contact
        }
    };
    match contact {
        // 0xB88BB: `comiss m_distance, depth; jna`: kept when 1e18 > depth.
        Some((normal, point, distance)) if BT_LARGE_FLOAT > distance => Collector::Point { normal, point, distance },
        _ => Collector::Empty,
    }
}

/// The point collector (`btPointCollector`) after one closest-point call.
enum Collector {
    /// `m_hasResult` (+34h) set: `m_normalOnBInWorld` (+10h), `m_pointInWorld` (+20h), `m_distance` (+30h).
    Point { normal: Vec3, point: Vec3, distance: f32 },
    /// `m_hasResult` clear.
    Empty,
}

/// 0xB9320 `btContinuousConvexCollision::calcTimeOfImpact(fromA, toA, fromB, toB, result)` for shape A cast along a
/// straight line with identity bases and shape B standing still (`fromB == toB == transform_b`): `other` with its
/// detector margin `margin_b` (its `getMargin`, vtable +28h: 0 for the controller's meshes' triangles, the collision
/// margin +30h of a box or a cylinder, the radius of a sphere), and the result's `m_allowedPenetration` (+B8h).
pub fn time_of_impact<B: ShapeB>(
    cast: &Cast,
    other: &B,
    margin_b: f32,
    transform_b: &Transform,
    allowed_penetration: f32,
) -> Toi {
    // 0x72EB0 (0xB934D for A, 0xB9370 for B), timeStep 1; B's from and to are both `transform_b`.
    let lin_a = linear_velocity(cast.by, TIME_STEP);
    let lin_b = linear_velocity(way(transform_b.origin, transform_b.origin), TIME_STEP);
    // 0xB93A5..0xB9467: the angular bound, angVelB.length() * radiusB + angVelA.length() * radiusA: +0 (module notes).
    let max_angular = 0.0f32;
    // 0xB942D..0xB945E: relLinVel = linVelB - linVelA.
    let rel = [lin_b[0] - lin_a[0], lin_b[1] - lin_a[1], lin_b[2] - lin_a[2]];
    // 0xB946B..0xB94B7: (relLinVel's length + the angular bound) == 0: nothing moves. ORDER (asm): (x*x + z*z) + y*y.
    if ((rel[0] * rel[0] + rel[2] * rel[2]) + rel[1] * rel[1]).sqrt() + max_angular == 0.0 {
        return Toi::Miss(Miss::NoMotion);
    }
    // ORDER (asm 0xB952F..0xB957E, 0xB9615..0xB9641): relLinVel . n is (n.y*r.y + n.x*r.x) + n.z*r.z, then the bound.
    let projected = |n: Vec3| ((n[1] * rel[1] + n[0] * rel[0]) + n[2] * rel[2]) + max_angular;
    let from_a = Transform { basis: IDENTITY.basis, origin: cast.from };

    let mut lambda = 0.0f32;
    let mut last_lambda = lambda;
    let mut iterations = 0u32;
    // 0xB9504: the closest points at the start, of fromA and fromB as the caller passed them.
    let (mut n, mut c, mut dist) = match closest(cast, from_a, other, margin_b, *transform_b) {
        // 0xB9541: dist = the allowed penetration + the distance.
        Collector::Point { normal, point, distance } => (normal, point, allowed_penetration + distance),
        Collector::Empty => return Toi::Miss(Miss::NoClosestPoints),
    };
    // 0xB9584: no report for a move away from the contact normal.
    if SIMD_EPSILON >= projected(n) {
        return Toi::Miss(Miss::Receding);
    }
    while dist > TOUCHING {
        let p = projected(n);
        if SIMD_EPSILON >= p {
            return Toi::Miss(Miss::Receding);
        }
        lambda += dist / p;
        if lambda > ONE {
            return Toi::Miss(Miss::PastTheEnd);
        }
        if 0.0 > lambda {
            return Toi::Miss(Miss::BeforeTheStart);
        }
        if last_lambda >= lambda {
            return Toi::Miss(Miss::NoProgress);
        }
        last_lambda = lambda;
        // 0xB96B0, 0xB96D9: both shapes at lambda; B's basis goes through the quaternion round trip.
        let at_a = integrate_transform(&from_a, lin_a, lambda);
        let at_b = integrate_transform(transform_b, lin_b, lambda);
        match closest(cast, at_a, other, margin_b, at_b) {
            Collector::Point { normal, point, distance } => {
                dist = allowed_penetration + distance;
                c = point;
                n = normal;
            }
            Collector::Empty => return Toi::Miss(Miss::LostContact),
        }
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            return Toi::Miss(Miss::IterationLimit);
        }
    }
    // 0xB97DE..0xB981C: the result's fraction, normal and hit point.
    Toi::Hit { fraction: lambda, normal: n, point: c }
}

/// 0xB8670 `btTriangleConvexcastCallback::processTriangle` for one triangle: the cast (0xB9320) of the callback's
/// shape against the triangle (its corners in the mesh's frame, `m_triangleCollisionMargin` +D4h its margin, the
/// mesh's `m_triangleToWorld` +90h its transform, `m_allowedPenetration` +D8h the result's), then the report of a hit
/// under `hit_fraction` (the fraction the callback holds, `m_hitFraction` +D0h) with its normal made unit.
pub fn cast_triangle(
    cast: &Cast,
    triangle: &Triangle,
    margin: f32,
    transform: &Transform,
    allowed_penetration: f32,
    hit_fraction: f32,
) -> CastEnd {
    let (fraction, n, point) = match time_of_impact(cast, triangle, margin, transform, allowed_penetration) {
        Toi::Hit { fraction, normal, point } => (fraction, normal, point),
        Toi::Miss(_) => return CastEnd::Miss,
    };
    // 0xB877A..0xB87D2: the normal's squared length over 1e-4 (ORDER (asm): (y*y + z*z) + x*x) and the fraction under
    // the callback's.
    let len2 = (n[1] * n[1] + n[2] * n[2]) + n[0] * n[0];
    if !(len2 > SHORT_NORMAL) || !(hit_fraction > fraction) {
        return CastEnd::Miss;
    }
    // 0xB87D4..0xB882B: x87 at 24-bit precision, 1 / sqrt, then each component times it.
    let rlen = ONE / len2.sqrt();
    CastEnd::Hit { fraction, normal: [n[0] * rlen, n[1] * rlen, n[2] * rlen], point }
}

/// 0xA14E0 `btCollisionWorld::objectQuerySingle`'s convex branch (shape type under 20; after the cast's return at
/// 0xA15D8) for one convex shape B: a box, a sphere, a cylinder or a capsule, the object's own shape or a compound's
/// child under [`compose`]d transforms. The cast (0xB9320) with the shape's own margin, then the report of a hit under
/// `closest_fraction` (the callback's `m_closestHitFraction`) with its normal made unit: the callback's
/// `addSingleResult` gets a `LocalConvexResult` without shape info (0x9FC20 with a null `m_localShapeInfo`),
/// `normalInWorldSpace` true.
pub fn cast_convex<B: ShapeB>(
    cast: &Cast,
    other: &B,
    margin: f32,
    transform: &Transform,
    allowed_penetration: f32,
    closest_fraction: f32,
) -> CastEnd {
    let (fraction, n, point) = match time_of_impact(cast, other, margin, transform, allowed_penetration) {
        Toi::Hit { fraction, normal, point } => (fraction, normal, point),
        Toi::Miss(_) => return CastEnd::Miss,
    };
    // 0xA15E0..0xA1622: the normal's squared length over 1e-4, in the source's order here ((x*x + y*y) + z*z);
    // 0xA1628..0xA1635: the callback's fraction over the hit's.
    let len2 = (n[0] * n[0] + n[1] * n[1]) + n[2] * n[2];
    if !(len2 > SHORT_NORMAL) || !(closest_fraction > fraction) {
        return CastEnd::Miss;
    }
    // 0xA163B..0xA1684: x87 at 24-bit precision, 1 / sqrt, then each component times it.
    let rlen = ONE / len2.sqrt();
    CastEnd::Hit { fraction, normal: [n[0] * rlen, n[1] * rlen, n[2] * rlen], point }
}

/// 0xA2C30 (`btCompoundLeafCallback::Process`): a compound's child placed in the world, `world * child`, in the
/// order of operations of its inlined `btTransform::operator*` (row 0 and the origin's x in the source's order; rows
/// 1 and 2 and the origin's y and z with the first column's product added last). Bit for bit the transform B of the
/// 260 distinct children the recorded casts met (`docs/notes/exactness_rig.md`).
pub fn compose(world: &Transform, child: &Transform) -> Transform {
    let (w, c) = (&world.basis, &child.basis);
    let (wo, co) = (&world.origin, &child.origin);
    let mut basis = [[0.0f32; 3]; 3];
    for (r, row) in basis.iter_mut().enumerate() {
        for (col, value) in row.iter_mut().enumerate() {
            *value = if r == 0 {
                // 0xA2E65..0xA2F11.
                (w[0][0] * c[0][col] + w[0][1] * c[1][col]) + w[0][2] * c[2][col]
            } else {
                // 0xA2D82..0xA2E5B (row 1), 0xA2CFD..0xA2D87 (row 2).
                (w[r][1] * c[1][col] + w[r][2] * c[2][col]) + c[0][col] * w[r][0]
            };
        }
    }
    let origin = [
        // 0xA2C70..0xA2C8D.
        ((w[0][0] * co[0] + w[0][1] * co[1]) + w[0][2] * co[2]) + wo[0],
        // 0xA2C9C..0xA2CBF.
        ((w[1][1] * co[1] + w[1][2] * co[2]) + w[1][0] * co[0]) + wo[1],
        // 0xA2CCA..0xA2CE9.
        ((w[2][1] * co[1] + w[2][2] * co[2]) + w[2][0] * co[0]) + wo[2],
    ];
    Transform { basis, origin }
}

/// A basis sent through a quaternion and back (`getRotation` 0x72B90, `setRotation` 0x72A50), as the down sweep's
/// second look at a round shape reads its rotation (0x84190, 0x848C7).
pub fn through_quaternion(basis: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    set_rotation(get_rotation(basis))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::exe_gjk::{Axis, WorldShape};

    fn capsule(from: Vec3, to: Vec3) -> Cast {
        let by = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        Cast { shape: CastShape::capsule([0.38, 0.52, 0.38], [1.0; 3], 0.04, Axis::Y), margin: 0.04, from, by }
    }

    fn floor() -> Triangle {
        Triangle([[-5.0, 0.0, -5.0], [5.0, 0.0, -5.0], [0.0, 0.0, 5.0]])
    }

    /// A standing capsule dropped onto a floor stops with its bottom the touching distance over it.
    #[test]
    fn a_capsule_dropped_on_a_floor_stops_at_the_touching_distance() {
        let end = cast_triangle(&capsule([0.0, 2.0, 0.0], [0.0, 0.0, 0.0]), &floor(), 0.0, &IDENTITY, 0.0, 1.0);
        let CastEnd::Hit { fraction, normal, point } = end else { panic!("{end:?}") };
        // The centre starts 2 m up and the bottom is 0.9 under it: 1.1 m of 2 m to the floor.
        assert!((fraction - 0.55).abs() < 1e-3, "{fraction}");
        assert!((normal[1] - 1.0).abs() < 1e-6 && normal[0].abs() < 1e-3 && normal[2].abs() < 1e-3, "{normal:?}");
        assert!(point[1].abs() < 1e-6, "{point:?}");
    }

    /// A move along the floor, or away from it, reports nothing; neither does a hit at or past the callback's fraction.
    #[test]
    fn no_approach_and_a_later_hit_are_misses() {
        let f = floor();
        let miss = |from, to, hit_fraction| cast_triangle(&capsule(from, to), &f, 0.0, &IDENTITY, 0.0, hit_fraction);
        assert_eq!(miss([0.0, 2.0, 0.0], [1.0, 2.0, 0.0], 1.0), CastEnd::Miss);
        assert_eq!(miss([0.0, 2.0, 0.0], [0.0, 3.0, 0.0], 1.0), CastEnd::Miss);
        assert_eq!(miss([0.0, 2.0, 0.0], [0.0, 2.0, 0.0], 1.0), CastEnd::Miss);
        assert_eq!(miss([0.0, 2.0, 0.0], [0.0, 0.0, 0.0], 0.5), CastEnd::Miss);
        assert_eq!(
            time_of_impact(&capsule([0.0, 2.0, 0.0], [0.0, 2.0, 0.0]), &f, 0.0, &IDENTITY, 0.0),
            Toi::Miss(Miss::NoMotion)
        );
    }

    /// The identity goes through the quaternion round trip with its own bits, at any step.
    #[test]
    fn the_identity_comes_back_from_integrate_transform() {
        let t = Transform { basis: IDENTITY.basis, origin: [1.0, 2.0, 3.0] };
        let at = integrate_transform(&t, [0.5, 0.0, -1.0], 0.25);
        assert_eq!(at.basis.map(|r| r.map(f32::to_bits)), IDENTITY.basis.map(|r| r.map(f32::to_bits)));
        assert_eq!(at.origin, [1.125, 2.0, 2.75]);
    }

    /// A floor rotated a quarter turn about y is the same floor: the hit is where the world-space one is.
    #[test]
    fn a_rotated_standing_floor_is_met_where_the_world_one_is() {
        let quarter = Transform { basis: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]], origin: [0.0; 3] };
        let cast = capsule([0.0, 2.0, 0.0], [0.0, 0.0, 0.0]);
        let world = time_of_impact(&cast, &floor(), 0.0, &IDENTITY, 0.0);
        let turned = time_of_impact(&cast, &floor(), 0.0, &quarter, 0.0);
        let (Toi::Hit { fraction: f0, .. }, Toi::Hit { fraction: f1, .. }) = (world, turned) else {
            panic!("{world:?} {turned:?}")
        };
        assert!((f0 - f1).abs() < 1e-5, "{f0} {f1}");
    }

    /// A sphere dropped onto a box stands on its top face with its margin.
    #[test]
    fn a_sphere_dropped_on_a_box_stops_on_its_top() {
        let block = WorldShape::Box { half_extents: [1.0, 0.5, 1.0] };
        let at = Transform { basis: IDENTITY.basis, origin: [0.0, 1.0, 0.0] };
        let cast = Cast { shape: CastShape::Sphere, margin: 0.25, from: [0.0, 3.0, 0.0], by: [0.0, -2.0, 0.0] };
        let Toi::Hit { fraction, normal, .. } = time_of_impact(&cast, &block, 0.04, &at, 0.0) else { panic!() };
        // From 3 down to 1 + 0.5 + 0.04 + 0.25 = 1.79: 1.21 of 2.
        assert!((fraction - 0.605).abs() < 1e-3, "{fraction}");
        assert!(normal[1] > 0.99, "{normal:?}");
    }
}
