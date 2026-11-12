//! A ray against one collision object, as Survarium 0.26g's executable tests it: a port of its statically linked
//! Bullet `btCollisionWorld::rayTestSingle` (0xA02D0) with what it calls, bit for bit: the convex branch's
//! `btSubsimplexConvexCast::calcTimeOfImpact` (0xB72A0) of a point (`btSphereShape(0)`) against the shape, with the
//! shapes' own virtual supports (vtable +38h) and boxes (vtable +4) and this build's reversed second cast; the
//! triangle mesh branch's ray in the mesh's frame and `btTriangleRaycastCallback::processTriangle` (0xB8010) with
//! the bridge's `reportHit` (0xA0F60); the compound branch's child transform (`RayTester::Process` 0xA10B0); and
//! `ClosestRayResultCallback::addSingleResult` (0x84040). The walk tick's two second looks (`ray_pick` 0x898C0,
//! the steep-ground look in 0x84810) read the callback's object (+8) and its normal (+40h) after the call.
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
//! # Exactness
//!
//! As in [`super::exe_gjk`]: scalar SSE `f32` in the executable's grouping (`ORDER (asm ...)` where it is not the
//! source's), x87 square roots and reciprocals at 24-bit precision (each result an `f32`), x87 sums and products of
//! two `f32` operands (the boxes' `fadd`, a sphere's `fld; fmul` margin) rounded to `f32` the same way. A sum of two
//! terms is exact in either order, so only groupings of three or more are marked. Unary minus is the executable's
//! `xorps` with the sign mask 0xA2D7C0.
// The exe's comparisons are ported as written: `!(a > b)` and `a > b` differ on NaN, as `comiss` branches do.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
use super::exe_gjk::{Axis, SimplexSolver, Transform, Vec3};

/// 0xA70884: 1.0.
const ONE: f32 = 1.0;
/// 0xA2D778: -1.0, a too-short support direction's components (`vecnorm.setValue(-1, -1, -1)`).
const MINUS_ONE: f32 = -1.0;
/// 0xA2D840: 1e-4: the subsimplex cast's `epsilon` (0xB76CE, 0xB7D59), the ray's normal's squared length bound
/// (0xA0456, 0xA0652) and the capsule support's short direction (0xA5EEF).
const EPSILON: f32 = f32::from_bits(0x38D1_B717);
/// 0xA2DA9C: `SIMD_EPSILON * SIMD_EPSILON` 2^-46.
const SIMD_EPSILON_SQUARED: f32 = f32::from_bits(0x2880_0000);
/// 0xA2DB38: `-(SIMD_EPSILON * SIMD_EPSILON)`, the subsimplex cast's bound on `v.dot(r)` (0xB7B67).
const MINUS_SIMD_EPSILON_SQUARED: f32 = f32::from_bits(0xA880_0000);
/// 0xA2DAE8: -1e-4, the triangle ray's edge tolerance factor (0xB8202).
const EDGE_TOLERANCE_FACTOR: f32 = f32::from_bits(0xB8D1_B717);
/// 0xA2D7D8: -1e18, the capsule support's starting best dot product (0xA5E9E).
const CAPSULE_FIRST_MAX_DOT: f32 = f32::from_bits(0xDD5E_0B6B);
/// The subsimplex cast's `MAX_ITERATIONS`: the immediate 0x20 at 0xB76D5 (Bullet's source has 64).
const MAX_ITERATIONS: u32 = 32;

type Basis = [[f32; 3]; 3];

// ---------------------------------------------------------------------------------------------------------------
// The shapes

/// A convex world shape as the ray's cast reads it: through its virtual `localGetSupportingVertex` (vtable +38h,
/// margin included) and its `getAabb` (vtable +4). Each variant is one class of the executable (its vtable).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConvexShape {
    /// `btBoxShape` (vtable 0x9E4AFC, type 0): the implicit dimensions (+20h, the half extents less the margin) and
    /// the collision margin (+30h, `getMargin` 0x74190).
    Box { implicit: Vec3, margin: f32 },
    /// `btSphereShape` (vtable 0x9E2FCC, type 8): its margin is its radius, `implicit.x * scaling.x` (`getMargin`
    /// 0x74280: `fld [+20h]; fmul [+10h]`), kept here as computed once (the same bits every time).
    Sphere { margin: f32 },
    /// `btCylinderShape` (vtable 0x9E4B9C, type 13, up axis Y: its support is `CylinderLocalSupportY` 0xA9F10): the
    /// implicit dimensions (+20h) and the margin (+30h).
    CylinderY { implicit: Vec3, margin: f32 },
    /// `btCapsuleShape` (vtable 0x9E48A4, type 10): the implicit dimensions (+20h; the radius on `(up + 2) % 3`, the
    /// half height on `up`), the local scaling (+10h), the margin (+30h) and the up axis (+40h).
    Capsule { implicit: Vec3, scaling: Vec3, margin: f32, up: Axis },
}

impl ConvexShape {
    /// The ray's cast shape: `btSphereShape pointShape(0)` made on 0xA02D0's stack (0xA02F5..0xA035A: vtable
    /// 0x9E2FCC, scaling (1, 1, 1), implicit dimensions and margin +0), so its radius is `0 * 1`.
    pub const POINT: ConvexShape = ConvexShape::Sphere { margin: 0.0 };

    /// A sphere from its implicit x (+20h) and scaling x (+10h): `getMargin` 0x74280 (x87 product, 24-bit).
    pub fn sphere(implicit_x: f32, scaling_x: f32) -> ConvexShape {
        ConvexShape::Sphere { margin: implicit_x * scaling_x }
    }

    /// `localGetSupportingVertex` (vtable +38h): the shape's farthest point along `dir`, margin included.
    pub fn support(&self, dir: Vec3) -> Vec3 {
        match *self {
            // 0xA8D10: `halfExtents = implicit + margin` (x87 `fadd`: margin + h), then `btFsels(dir, h, -h)`:
            // `comiss dir, 0; jnc` keeps +h unless dir < 0 or NaN.
            ConvexShape::Box { implicit, margin } => {
                let h = [margin + implicit[0], margin + implicit[1], margin + implicit[2]];
                let pick = |d: f32, h: f32| if d >= 0.0 { h } else { -h };
                [pick(dir[0], h[0]), pick(dir[1], h[1]), pick(dir[2], h[2])]
            }
            // 0xA69E0: `btSphereShape::localGetSupportingVertex`: the centre (0xA6990: zeros) plus the margin along
            // the direction made unit, with no test of the margin.
            ConvexShape::Sphere { margin } => add_margin([0.0, 0.0, 0.0], dir, margin),
            // 0xA9A30 (`btConvexInternalShape::localGetSupportingVertex`) over 0xA9FE0 -> 0xA9F10.
            ConvexShape::CylinderY { implicit, margin } => {
                let support = cylinder_local_support_y(implicit, dir);
                if margin != 0.0 {
                    add_margin(support, dir, margin)
                } else {
                    support
                }
            }
            // 0x8E290 (the same as 0xA9A30) over 0xA5E90.
            ConvexShape::Capsule { implicit, scaling, margin, up } => {
                let support = capsule_support_without_margin(implicit, scaling, margin, up, dir);
                if margin != 0.0 {
                    add_margin(support, dir, margin)
                } else {
                    support
                }
            }
        }
    }

    /// `getAabb` (vtable +4) under `t`: (min, max).
    pub fn aabb(&self, t: &Transform) -> (Vec3, Vec3) {
        let m = &t.basis;
        let o = t.origin;
        match *self {
            // 0xA97B0 (`btTransformAabb` of `implicit + margin`, boxes and cylinders alike).
            ConvexShape::Box { implicit, margin } | ConvexShape::CylinderY { implicit, margin } => {
                let h = [implicit[0] + margin, implicit[1] + margin, implicit[2] + margin];
                let a = abs_basis(m);
                // ORDER (asm 0xA9894..0xA98F5): x (|m01|hy + |m02|hz) + |m00|hx, y (|m12|hz + |m10|hx) + |m11|hy,
                // z (|m21|hy + |m22|hz) + |m20|hx; the source's dots are x, y, z.
                let e = [
                    (a[0][1] * h[1] + a[0][2] * h[2]) + a[0][0] * h[0],
                    (a[1][2] * h[2] + a[1][0] * h[0]) + a[1][1] * h[1],
                    (a[2][1] * h[1] + a[2][2] * h[2]) + a[2][0] * h[0],
                ];
                ([o[0] - e[0], o[1] - e[1], o[2] - e[2]], [o[0] + e[0], o[1] + e[1], o[2] + e[2]])
            }
            // 0xA6B50: centre less and plus the margin (x87, three calls of getMargin: the same value).
            ConvexShape::Sphere { margin } => {
                ([o[0] - margin, o[1] - margin, o[2] - margin], [o[0] + margin, o[1] + margin, o[2] + margin])
            }
            // 0xA5B10 (`btCapsuleShape::getAabb`).
            ConvexShape::Capsule { implicit, margin, up, .. } => {
                let radius = implicit[(up.index() + 2) % 3];
                let mut h = [radius; 3];
                h[up.index()] = implicit[up.index()] + radius;
                let h = [margin + h[0], h[1] + margin, h[2] + margin];
                let a = abs_basis(m);
                // ORDER (asm 0xA5C68..0xA5CD0): x (|m01|hy + |m02|hz) + |m00|hx, y (|m11|hy + |m12|hz) + |m10|hx,
                // z (|m22|hz + |m21|hy) + |m20|hx.
                let e = [
                    (a[0][1] * h[1] + a[0][2] * h[2]) + a[0][0] * h[0],
                    (a[1][1] * h[1] + a[1][2] * h[2]) + a[1][0] * h[0],
                    (a[2][2] * h[2] + a[2][1] * h[1]) + a[2][0] * h[0],
                ];
                ([o[0] - e[0], o[1] - e[1], o[2] - e[2]], [o[0] + e[0], o[1] + e[1], o[2] + e[2]])
            }
        }
    }
}

/// The basis with each element's absolute value (0x550B0: `fld; fabs`).
fn abs_basis(m: &Basis) -> Basis {
    let mut a = *m;
    for row in a.iter_mut() {
        for v in row.iter_mut() {
            *v = v.abs();
        }
    }
    a
}

/// The margin part of `localGetSupportingVertex` (0xA6A15..0xA6B3E, 0x8E2C8..0x8E3EB, 0xA9A80..0xA9BA8): the
/// direction made unit (`(-1, -1, -1)` first when its squared length is under 2^-46), times the margin, added.
fn add_margin(support: Vec3, dir: Vec3, margin: f32) -> Vec3 {
    let mut n = dir;
    // (x*x + y*y) + z*z, the source's order (0xA6A36..0xA6A55).
    let len2 = (n[0] * n[0] + n[1] * n[1]) + n[2] * n[2];
    if SIMD_EPSILON_SQUARED > len2 {
        n = [MINUS_ONE, MINUS_ONE, MINUS_ONE];
    }
    let len2 = (n[0] * n[0] + n[1] * n[1]) + n[2] * n[2];
    // x87: 1/sqrt, stored as f32 (`fstp [esp+0Ch]`), then each component times it in SSE.
    let r = ONE / len2.sqrt();
    let n = [r * n[0], r * n[1], r * n[2]];
    [support[0] + n[0] * margin, support[1] + n[1] * margin, support[2] + n[2] * margin]
}

/// 0xA9F10 `CylinderLocalSupportY` (EAX the half extents: +20h; EDI the direction; ESI the out): radius `h.x`, half
/// height `h.y`.
fn cylinder_local_support_y(h: Vec3, v: Vec3) -> Vec3 {
    let radius = h[0];
    let half_height = h[1];
    let s = (v[0] * v[0] + v[2] * v[2]).sqrt();
    // `comiss 0, v.y; jna` keeps the plus sign unless 0 > v.y.
    let along = if 0.0 > v[1] { -half_height } else { half_height };
    // `fucomip` against 0 with `test ah,44h; jpo`: the zero branch only when s == 0 (NaN takes the division).
    if s != 0.0 {
        let d = radius / s;
        [v[0] * d, along, v[2] * d]
    } else {
        [radius, along, 0.0]
    }
}

/// 0xA5E90 `btCapsuleShape::localGetSupportingVertexWithoutMargin` (the virtual one; 0x8D680's case 0x8D9BB has
/// other groupings).
fn capsule_support_without_margin(implicit: Vec3, scaling: Vec3, margin: f32, up: Axis, dir: Vec3) -> Vec3 {
    // ORDER (asm 0xA5ED3..0xA5EFA): (z*z + y*y) + x*x; the source's length2 is x, y, z.
    let len_sqr = (dir[2] * dir[2] + dir[1] * dir[1]) + dir[0] * dir[0];
    let vec = if EPSILON > len_sqr {
        [ONE, 0.0, 0.0]
    } else {
        // x87: 1/sqrt, each component times it (0xA5F38..0xA5F67).
        let rlen = ONE / len_sqr.sqrt();
        [dir[0] * rlen, dir[1] * rlen, dir[2] * rlen]
    };
    let radius = implicit[(up.index() + 2) % 3];
    let half_height = implicit[up.index()];
    let mut max_dot = CAPSULE_FIRST_MAX_DOT;
    let mut sup = [0.0, 0.0, 0.0];
    for end in [half_height, -half_height] {
        let mut pos = [0.0, 0.0, 0.0];
        pos[up.index()] = end;
        // ((s*v)*r + pos) - v*margin per component, `v*margin` in x87 (0xA5FA8..0xA6021, 0xA60B1..0xA612A).
        let vtx = [
            ((scaling[0] * vec[0]) * radius + pos[0]) - vec[0] * margin,
            ((scaling[1] * vec[1]) * radius + pos[1]) - vec[1] * margin,
            ((scaling[2] * vec[2]) * radius + pos[2]) - vec[2] * margin,
        ];
        // ORDER (asm 0xA603C..0xA6052, 0xA6145..0xA615B): (z + y) + x; the source's dot is x, y, z.
        let new_dot = (vtx[2] * vec[2] + vtx[1] * vec[1]) + vtx[0] * vec[0];
        if new_dot > max_dot {
            max_dot = new_dot;
            sup = vtx;
        }
    }
    sup
}

// ---------------------------------------------------------------------------------------------------------------
// The subsimplex cast 0xB72A0

/// Why `calcTimeOfImpact` 0xB72A0 returned false before writing the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `lambda > 1` at a round (0xB7B2F).
    LambdaOverOne,
    /// `v.dot(r) >= -SIMD_EPSILON^2` when `v.dot(w) > 0` (0xB7B6E).
    MovingApart,
}

/// How 0xB72A0 ended, with exactly what it wrote into the CastResult (+90h normal, +A0h hit point, +B0h fraction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SubsimplexEnd {
    /// AL 0, nothing written (0xB7D6B from inside the loop).
    Refused(Refusal),
    /// AL 0 after the fraction and the normal were written: the normal's dot with `r` is not under
    /// `-allowedPenetration` (0xB7E8A).
    Away { fraction: f32, normal: Vec3 },
    /// AL 1: the fraction, the normal (unit, or zeros) and the hit point (the simplex's point on B, 0xD1E80).
    Hit { fraction: f32, normal: Vec3, point: Vec3 },
}

/// 0xB72A0's answer and its loop's rounds (passes of 0xB7729).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Subsimplex {
    pub end: SubsimplexEnd,
    pub rounds: u32,
}

/// 0xB72A0 `btSubsimplexConvexCast::calcTimeOfImpact(fromA, toA, fromB, toB, result)` with a fresh Voronoi simplex
/// solver (reset at 0xB72B8; `m_equalVertexThreshold` 1e-4, set by 0xA02D0 at 0xA03FC).
#[allow(clippy::too_many_arguments)]
pub fn subsimplex_cast(
    a: &ConvexShape,
    b: &ConvexShape,
    from_a: &Transform,
    to_a: &Transform,
    from_b: &Transform,
    to_b: &Transform,
    allowed_penetration: f32,
) -> Subsimplex {
    let mut solver = SimplexSolver::new();
    let fa = from_a.origin;
    let ta = to_a.origin;
    let fb = from_b.origin;
    let tb = to_b.origin;
    // r = (toA - fromA) - (toB - fromB) per component (0xB72CA..0xB73DE).
    let r = [(ta[0] - fa[0]) - (tb[0] - fb[0]), (ta[1] - fa[1]) - (tb[1] - fb[1]), (ta[2] - fa[2]) - (tb[2] - fb[2])];
    let mut interp_a = *from_a;
    let mut interp_b = *from_b;
    let neg_r = [-r[0], -r[1], -r[2]];
    let m = &from_a.basis;
    // ORDER (asm 0xB73E7..0xB7490): `-r * fromA.basis` as (m[1][c]*-ry + m[2][c]*-rz) + m[0][c]*-rx; the source's
    // tdot is (m[0][c]*x + m[1][c]*y) + m[2][c]*z.
    let dir = [
        (m[1][0] * neg_r[1] + m[2][0] * neg_r[2]) + m[0][0] * neg_r[0],
        (m[1][1] * neg_r[1] + m[2][1] * neg_r[2]) + m[0][1] * neg_r[0],
        (m[1][2] * neg_r[1] + m[2][2] * neg_r[2]) + m[0][2] * neg_r[0],
    ];
    let s = a.support(dir);
    // ORDER (asm 0xB74B0..0xB754A): x ((m00 sx + m01 sy) + m02 sz) + ox (the source's), y ((m11 sy + m12 sz) + m10 sx)
    // + oy, z ((m21 sy + m22 sz) + m20 sx) + oz.
    let mut sup_a = [
        ((m[0][0] * s[0] + m[0][1] * s[1]) + m[0][2] * s[2]) + fa[0],
        ((m[1][1] * s[1] + m[1][2] * s[2]) + m[1][0] * s[0]) + fa[1],
        ((m[2][1] * s[1] + m[2][2] * s[2]) + m[2][0] * s[0]) + fa[2],
    ];
    let m = &from_b.basis;
    // ORDER (asm 0xB754F..0xB75C6): `r * fromB.basis` as x (m00 rx + m20 rz) + m10 ry, y (m21 rz + m11 ry) + m01 rx,
    // z (m22 rz + m12 ry) + m02 rx.
    let dir = [
        (m[0][0] * r[0] + m[2][0] * r[2]) + m[1][0] * r[1],
        (m[2][1] * r[2] + m[1][1] * r[1]) + m[0][1] * r[0],
        (m[2][2] * r[2] + m[1][2] * r[1]) + m[0][2] * r[0],
    ];
    let s = b.support(dir);
    // ORDER (asm 0xB75ED..0xB766A): x ((m02 sz + m00 sx) + m01 sy) + ox, y ((m11 sy + m12 sz) + m10 sx) + oy,
    // z ((m21 sy + m22 sz) + m20 sx) + oz.
    let mut sup_b = [
        ((m[0][2] * s[2] + m[0][0] * s[0]) + m[0][1] * s[1]) + fb[0],
        ((m[1][1] * s[1] + m[1][2] * s[2]) + m[1][0] * s[0]) + fb[1],
        ((m[2][1] * s[1] + m[2][2] * s[2]) + m[2][0] * s[0]) + fb[2],
    ];
    let mut v = [sup_a[0] - sup_b[0], sup_a[1] - sup_b[1], sup_a[2] - sup_b[2]];
    // ORDER (asm 0xB76B1..0xB76CA): (x*x + z*z) + y*y.
    let dist2 = (v[0] * v[0] + v[2] * v[2]) + v[1] * v[1];
    let mut n = [0.0f32; 3];
    let mut lambda = 0.0f32;
    let mut rounds = 0u32;
    if dist2 > EPSILON {
        let mut max_iter = MAX_ITERATIONS;
        loop {
            // `while (dist2 > epsilon && maxIter--)`: the count is tested here (0xB7717..0xB7723).
            if max_iter == 0 {
                break;
            }
            max_iter -= 1;
            rounds += 1;
            let neg_v = [-v[0], -v[1], -v[2]];
            let m = &interp_a.basis;
            // ORDER (asm 0xB7743..0xB77DF): x (m00 -vx + m20 -vz) + m10 -vy, y (m21 -vz + m11 -vy) + m01 -vx,
            // z (m22 -vz + m12 -vy) + m02 -vx.
            let dir = [
                (m[0][0] * neg_v[0] + m[2][0] * neg_v[2]) + m[1][0] * neg_v[1],
                (m[2][1] * neg_v[2] + m[1][1] * neg_v[1]) + m[0][1] * neg_v[0],
                (m[2][2] * neg_v[2] + m[1][2] * neg_v[1]) + m[0][2] * neg_v[0],
            ];
            let s = a.support(dir);
            sup_a = loop_apply(&interp_a, s);
            let m = &interp_b.basis;
            // ORDER (asm 0xB78F3..0xB7998): x (m00 vx + m10 vy) + m20 vz (the source's), y (m11 vy + m21 vz) +
            // m01 vx, z (m12 vy + m22 vz) + m02 vx.
            let dir = [
                (m[0][0] * v[0] + m[1][0] * v[1]) + m[2][0] * v[2],
                (m[1][1] * v[1] + m[2][1] * v[2]) + m[0][1] * v[0],
                (m[1][2] * v[1] + m[2][2] * v[2]) + m[0][2] * v[0],
            ];
            let s = b.support(dir);
            sup_b = loop_apply(&interp_b, s);
            let w = [sup_a[0] - sup_b[0], sup_a[1] - sup_b[1], sup_a[2] - sup_b[2]];
            // (wx vx + wy vy) + wz vz, the source's order (0xB7AB3..0xB7B03).
            let v_dot_w = (w[0] * v[0] + w[1] * v[1]) + w[2] * v[2];
            if lambda > ONE {
                return Subsimplex { end: SubsimplexEnd::Refused(Refusal::LambdaOverOne), rounds };
            }
            if v_dot_w > 0.0 {
                // ORDER (asm 0xB7B3E..0xB7B60): (vx rx + vz rz) + vy ry.
                let v_dot_r = (v[0] * r[0] + v[2] * r[2]) + v[1] * r[1];
                if v_dot_r >= MINUS_SIMD_EPSILON_SQUARED {
                    return Subsimplex { end: SubsimplexEnd::Refused(Refusal::MovingApart), rounds };
                }
                lambda -= v_dot_w / v_dot_r;
                // setInterpolate3 (0xB7B80..0xB7C73): to*lambda + from*(1 - lambda) per component.
                let s = ONE - lambda;
                interp_a.origin = [ta[0] * lambda + fa[0] * s, ta[1] * lambda + fa[1] * s, ta[2] * lambda + fa[2] * s];
                interp_b.origin = [tb[0] * lambda + fb[0] * s, tb[1] * lambda + fb[1] * s, tb[2] * lambda + fb[2] * s];
                n = v;
            }
            if !solver.in_simplex(w) {
                solver.add_vertex(w, sup_a, sup_b);
            }
            let valid = solver.update_closest_vector_and_points();
            v = solver.cached_v();
            if !valid {
                break;
            }
            // (x*x + y*y) + z*z, the source's order (0xB7D2A..0xB7D55).
            let dist2 = (v[0] * v[0] + v[1] * v[1]) + v[2] * v[2];
            if !(dist2 > EPSILON) {
                break;
            }
        }
    }
    let fraction = lambda;
    // ORDER (asm 0xB7D82..0xB7DBB): (nx*nx + nz*nz) + ny*ny.
    let n_len2 = (n[0] * n[0] + n[2] * n[2]) + n[1] * n[1];
    // `comiss len2, 2^-46; jc`: zeros when under it or NaN.
    let normal = if n_len2 >= SIMD_EPSILON_SQUARED {
        let rlen = ONE / n_len2.sqrt();
        [n[0] * rlen, n[1] * rlen, n[2] * rlen]
    } else {
        [0.0, 0.0, 0.0]
    };
    // ORDER (asm 0xB7E46..0xB7E74): (Nz rz + Ny ry) + Nx rx.
    let dot = (normal[2] * r[2] + normal[1] * r[1]) + normal[0] * r[0];
    if dot >= -allowed_penetration {
        return Subsimplex { end: SubsimplexEnd::Away { fraction, normal }, rounds };
    }
    solver.update_closest_vector_and_points();
    let point = solver.cached_p2();
    Subsimplex { end: SubsimplexEnd::Hit { fraction, normal, point }, rounds }
}

/// The loop's `interpolatedTrans(support)` (0xB7805..0xB78C9 for A, 0xB79BE..0xB7A74 for B, the same grouping).
fn loop_apply(t: &Transform, s: Vec3) -> Vec3 {
    let m = &t.basis;
    let o = t.origin;
    // ORDER (asm): x ((m01 sy + m02 sz) + m00 sx) + ox, y ((m11 sy + m10 sx) + m12 sz) + oy,
    // z ((m21 sy + m20 sx) + m22 sz) + oz.
    [
        ((m[0][1] * s[1] + m[0][2] * s[2]) + m[0][0] * s[0]) + o[0],
        ((m[1][1] * s[1] + m[1][0] * s[0]) + m[1][2] * s[2]) + o[1],
        ((m[2][1] * s[1] + m[2][0] * s[0]) + m[2][2] * s[2]) + o[2],
    ]
}

// ---------------------------------------------------------------------------------------------------------------
// The convex branch of 0xA02D0

/// Why the convex branch reported nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvexMiss {
    /// The forward cast hit, its normal's squared length is not over 1e-4 (0xA045D).
    ShortNormal,
    /// The forward cast hit, not closer than the callback's fraction (0xA0473).
    NotCloser,
    /// The forward cast missed and the ray's start is outside the shape's box (0xA058D..0xA05DA).
    StartOutsideBox,
    /// The reversed cast (from the ray's end to its start) missed (0xA05FA).
    ReversedMissed,
    /// The reversed cast's fraction is over 1 (0xA0614).
    ReversedBeyond,
    /// The reversed cast's normal's squared length is not over 1e-4 (0xA0659).
    ReversedShortNormal,
    /// `1 - fraction` of the reversed cast is not closer than the callback's fraction (0xA0677).
    ReversedNotCloser,
}

/// What the convex branch hands `addSingleResult`: the fraction and the unit normal in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConvexRay {
    Miss(ConvexMiss),
    Hit { fraction: f32, normal: Vec3 },
}

/// The convex branch step by step: each cast and the box, as 0xA02D0 made them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvexRayTrace {
    pub forward: Subsimplex,
    /// Taken only after the forward cast's miss (0xA0544).
    pub aabb: Option<(Vec3, Vec3)>,
    /// Made only when the ray's start is in that box (0xA05F3).
    pub reversed: Option<Subsimplex>,
    pub answer: ConvexRay,
}

/// 0xA02D0's convex branch (shape type under 20): the point cast from the ray's start to its end against the
/// shape standing at `world` (allowed penetration 0, 0xA03F3); when it misses and the ray starts inside the shape's
/// box, cast back from the end to the start (this build's addition) and take `1 - fraction`. A hit's normal is
/// rotated by the ray's start's basis and made unit.
pub fn ray_convex(from: &Transform, to: &Transform, shape: &ConvexShape, world: &Transform, closest: f32) -> ConvexRayTrace {
    let point = ConvexShape::POINT;
    let forward = subsimplex_cast(&point, shape, from, to, world, world, 0.0);
    if let SubsimplexEnd::Hit { fraction, normal, .. } = forward.end {
        // (x*x + y*y) + z*z, the source's order (0xA0439..0xA0452).
        let len2 = (normal[0] * normal[0] + normal[1] * normal[1]) + normal[2] * normal[2];
        let answer = if !(len2 > EPSILON) {
            ConvexRay::Miss(ConvexMiss::ShortNormal)
        } else if !(closest > fraction) {
            ConvexRay::Miss(ConvexMiss::NotCloser)
        } else {
            ConvexRay::Hit { fraction, normal: rotate_and_normalize(&from.basis, normal) }
        };
        return ConvexRayTrace { forward, aabb: None, reversed: None, answer };
    }
    let (lo, hi) = shape.aabb(world);
    let f = from.origin;
    // `comiss from, min; jc` then `comiss max, from; jc` per axis: outside (or NaN) refuses.
    let inside = (0..3).all(|k| f[k] >= lo[k]) && (0..3).all(|k| hi[k] >= f[k]);
    if !inside {
        let answer = ConvexRay::Miss(ConvexMiss::StartOutsideBox);
        return ConvexRayTrace { forward, aabb: Some((lo, hi)), reversed: None, answer };
    }
    let reversed = subsimplex_cast(&point, shape, to, from, world, world, 0.0);
    let answer = match reversed.end {
        SubsimplexEnd::Hit { fraction, normal, .. } => {
            let len2 = (normal[0] * normal[0] + normal[1] * normal[1]) + normal[2] * normal[2];
            if !(ONE >= fraction) {
                ConvexRay::Miss(ConvexMiss::ReversedBeyond)
            } else if !(len2 > EPSILON) {
                ConvexRay::Miss(ConvexMiss::ReversedShortNormal)
            } else {
                let fraction = ONE - fraction;
                if !(closest > fraction) {
                    ConvexRay::Miss(ConvexMiss::ReversedNotCloser)
                } else {
                    // The reversed cast's normal is not turned over: it is the start's side seen from the end.
                    ConvexRay::Hit { fraction, normal: rotate_and_normalize(&from.basis, normal) }
                }
            }
        }
        _ => ConvexRay::Miss(ConvexMiss::ReversedMissed),
    };
    ConvexRayTrace { forward, aabb: Some((lo, hi)), reversed: Some(reversed), answer }
}

/// `castResult.m_normal = rayFromTrans.getBasis() * castResult.m_normal; normalize()` (0xA0479..0xA0784).
fn rotate_and_normalize(m: &Basis, n: Vec3) -> Vec3 {
    // ORDER (asm 0xA0479..0xA04DF): each row (m[r][1]*y + m[r][2]*z) + m[r][0]*x; the source's dot is x, y, z.
    let v = [
        (m[0][1] * n[1] + m[0][2] * n[2]) + m[0][0] * n[0],
        (m[1][1] * n[1] + m[1][2] * n[2]) + m[1][0] * n[0],
        (m[2][1] * n[1] + m[2][2] * n[2]) + m[2][0] * n[0],
    ];
    // ORDER (asm 0xA050A..0xA0524): (x*x + z*z) + y*y.
    let len2 = (v[0] * v[0] + v[2] * v[2]) + v[1] * v[1];
    // x87 1/sqrt; x in SSE times the f32 stored (`fst [esp+14h]`), y and z in x87: each an f32 product.
    let r = ONE / len2.sqrt();
    [v[0] * r, v[1] * r, r * v[2]]
}

// ---------------------------------------------------------------------------------------------------------------
// The triangle mesh branch

/// `btTransform::inverse` (0x72D20, ECX the transform, EAX the out): the transposed basis and `inv * -origin`.
pub fn inverse(t: &Transform) -> Transform {
    let m = &t.basis;
    let inv = [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]];
    let n = [-t.origin[0], -t.origin[1], -t.origin[2]];
    // ORDER (asm 0x72D82..0x72E17): x (i00 nx + i02 nz) + i01 ny, y (i11 ny + i12 nz) + i10 nx,
    // z (i21 ny + i22 nz) + i20 nx; the source's dot is x, y, z.
    let origin = [
        (inv[0][0] * n[0] + inv[0][2] * n[2]) + inv[0][1] * n[1],
        (inv[1][1] * n[1] + inv[1][2] * n[2]) + inv[1][0] * n[0],
        (inv[2][1] * n[1] + inv[2][2] * n[2]) + inv[2][0] * n[0],
    ];
    Transform { basis: inv, origin }
}

/// The ray's ends in the mesh's frame: `worldTocollisionObject * rayFromTrans.getOrigin()` and the same for the end
/// (0xA07D9..0xA096F): the inverse (0x72D20), then `((i[r][0]*x + i[r][1]*y) + i[r][2]*z) + o[r]`, the source's
/// order for both ends.
pub fn mesh_ray(from: Vec3, to: Vec3, mesh_world: &Transform) -> (Vec3, Vec3) {
    let inv = inverse(mesh_world);
    let apply = |p: Vec3| {
        let i = &inv.basis;
        let o = inv.origin;
        [
            ((i[0][0] * p[0] + i[0][1] * p[1]) + i[0][2] * p[2]) + o[0],
            ((i[1][0] * p[0] + i[1][1] * p[1]) + i[1][2] * p[2]) + o[1],
            ((i[2][0] * p[0] + i[2][1] * p[1]) + i[2][2] * p[2]) + o[2],
        ]
    };
    (apply(from), apply(to))
}

/// A ray callback's `m_flags` (+10h), copied into the triangle callback (+30h) by 0xA0DE0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayFlags(pub u32);

impl RayFlags {
    /// `kF_FilterBackfaces` (bit 0, 0xB81A5).
    fn filter_backfaces(self) -> bool {
        self.0 & 1 != 0
    }

    /// `kF_KeepUnflippedNormal` (bit 1, 0xB8482).
    fn keep_unflipped_normal(self) -> bool {
        self.0 & 2 != 0
    }
}

/// Which of the triangle's three edge tests refused the plane's point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// `(v0 - p) x (v1 - p)` (0xB8338).
    V0V1,
    /// `(v1 - p) x (v2 - p)` (0xB83E1).
    V1V2,
    /// `(v2 - p) x (v0 - p)` (0xB8449).
    V2V0,
}

/// What 0xB8010 does with one triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TriangleRay {
    /// Both ends on one side of the plane: `dist_a * dist_b >= 0` (0xB819C).
    SameSide,
    /// `kF_FilterBackfaces` and the start in front (0xB81AD).
    Backface,
    /// The plane's crossing not under `m_hitFraction` (0xB81D1).
    NotCloser,
    /// The crossing outside the triangle by an edge's test (with the edge tolerance `-1e-4 * |n|^2`).
    Outside(Edge),
    /// `reportHit(normal, distance, part, index)` (0xB84FD): the unit face normal, turned over when
    /// `kF_KeepUnflippedNormal` is set or the start is not in front (`dist_a <= 0`).
    Hit { normal: Vec3, distance: f32 },
}

/// 0xB8010 `btTriangleRaycastCallback::processTriangle` on one triangle (corners as it reads them: in the mesh's
/// frame, scaled), with the callback's `m_from` (+10h), `m_to` (+20h), `m_flags` (+30h), `m_hitFraction` (+34h).
pub fn ray_triangle(from: Vec3, to: Vec3, flags: RayFlags, hit_fraction: f32, tri: &[Vec3; 3]) -> TriangleRay {
    let [v0, v1, v2] = *tri;
    let v10 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let v20 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let n = cross(v10, v20);
    // ORDER (asm 0xB80F2..0xB812F): (z0 nz + y0 ny) + x0 nx; the source's dot is x, y, z.
    let dist = (v0[2] * n[2] + v0[1] * n[1]) + v0[0] * n[0];
    // ORDER (asm 0xB811F..0xB8152, 0xB814D..0xB817A): (z nz + y ny) + x nx, then less dist.
    let dist_a = ((from[2] * n[2] + from[1] * n[1]) + from[0] * n[0]) - dist;
    let dist_b = ((to[2] * n[2] + to[1] * n[1]) + to[0] * n[0]) - dist;
    if dist_b * dist_a >= 0.0 {
        return TriangleRay::SameSide;
    }
    if flags.filter_backfaces() && dist_a > 0.0 {
        return TriangleRay::Backface;
    }
    let distance = dist_a / (dist_a - dist_b);
    if !(hit_fraction > distance) {
        return TriangleRay::NotCloser;
    }
    // ORDER (asm 0xB81D7..0xB81F0): (nz*nz + ny*ny) + nx*nx.
    let len2 = (n[2] * n[2] + n[1] * n[1]) + n[0] * n[0];
    let edge_tolerance = len2 * EDGE_TOLERANCE_FACTOR;
    let s = ONE - distance;
    let p = [to[0] * distance + from[0] * s, from[1] * s + to[1] * distance, from[2] * s + to[2] * distance];
    let v0p = [v0[0] - p[0], v0[1] - p[1], v0[2] - p[2]];
    let v1p = [v1[0] - p[0], v1[1] - p[1], v1[2] - p[2]];
    // ORDER (asm 0xB831F..0xB832F, 0xB83C8..0xB83D8, 0xB8430..0xB8440): each test (cz nz + cy ny) + cx nx.
    let dot_n = |c: Vec3| (c[2] * n[2] + c[1] * n[1]) + c[0] * n[0];
    // `comiss dot, tolerance; jc`: under it (or NaN) refuses.
    if !(dot_n(cross(v0p, v1p)) >= edge_tolerance) {
        return TriangleRay::Outside(Edge::V0V1);
    }
    let v2p = [v2[0] - p[0], v2[1] - p[1], v2[2] - p[2]];
    if !(dot_n(cross(v1p, v2p)) >= edge_tolerance) {
        return TriangleRay::Outside(Edge::V1V2);
    }
    if !(dot_n(cross(v2p, v0p)) >= edge_tolerance) {
        return TriangleRay::Outside(Edge::V2V0);
    }
    // x87 1/sqrt of the same len2, each component times it (0xB844F..0xB847E).
    let r = ONE / len2.sqrt();
    let unit = [n[0] * r, n[1] * r, r * n[2]];
    let turned = flags.keep_unflipped_normal() || 0.0 >= dist_a;
    let normal = if turned { [-unit[0], -unit[1], -unit[2]] } else { unit };
    TriangleRay::Hit { normal, distance }
}

/// `btVector3::cross` as 0xB8010 does it (each component `a*b - c*d`, the source's order: two products, one
/// difference, exact in grouping).
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The ray bridge's `reportHit` (0xA0F60): `hitNormalWorld = m_colObjWorldTransform.getBasis() * hitNormalLocal`.
pub fn bridge_normal(world: &Basis, n: Vec3) -> Vec3 {
    // ORDER (asm 0xA0F6C..0xA0FFA): each row (m[r][1]*y + m[r][2]*z) + m[r][0]*x.
    [
        (world[0][1] * n[1] + world[0][2] * n[2]) + world[0][0] * n[0],
        (world[1][1] * n[1] + world[1][2] * n[2]) + world[1][0] * n[0],
        (world[2][1] * n[1] + world[2][2] * n[2]) + world[2][0] * n[0],
    ]
}

// ---------------------------------------------------------------------------------------------------------------
// The compound branch

/// `childWorldTrans = m_colObjWorldTransform * childTrans` as `RayTester::Process` (0xA10B0) inlines it. Its basis
/// and its origin's y and z are grouped as `objectQuerySingle`'s 0xA2C30 groups them; its origin's x is not.
pub fn compose_child(w: &Transform, c: &Transform) -> Transform {
    let wm = &w.basis;
    let cm = &c.basis;
    let co = c.origin;
    // ORDER (asm 0xA10EF..0xA110C): x ((W02 Cz + W00 Cx) + W01 Cy) + Wox (0xA2C30: ((W00 Cx + W01 Cy) + W02 Cz));
    // ORDER (asm 0xA111A..0xA113D): y ((W11 Cy + W12 Cz) + W10 Cx) + Woy;
    // ORDER (asm 0xA1148..0xA1167): z ((W21 Cy + W22 Cz) + W20 Cx) + Woz.
    let origin = [
        ((wm[0][2] * co[2] + wm[0][0] * co[0]) + wm[0][1] * co[1]) + w.origin[0],
        ((wm[1][1] * co[1] + wm[1][2] * co[2]) + wm[1][0] * co[0]) + w.origin[1],
        ((wm[2][1] * co[1] + wm[2][2] * co[2]) + wm[2][0] * co[0]) + w.origin[2],
    ];
    let mut basis = [[0.0f32; 3]; 3];
    for col in 0..3 {
        // ORDER (asm 0xA12D3..0xA1383): R[0][c] (W00 C0c + W01 C1c) + W02 C2c (the source's tdot);
        // ORDER (asm 0xA1206..0xA12CD): R[1][c] (W11 C1c + W12 C2c) + W10 C0c;
        // ORDER (asm 0xA117B..0xA1205): R[2][c] (W21 C1c + W22 C2c) + W20 C0c.
        basis[0][col] = (wm[0][0] * cm[0][col] + wm[0][1] * cm[1][col]) + wm[0][2] * cm[2][col];
        basis[1][col] = (wm[1][1] * cm[1][col] + wm[1][2] * cm[2][col]) + wm[1][0] * cm[0][col];
        basis[2][col] = (wm[2][1] * cm[1][col] + wm[2][2] * cm[2][col]) + wm[2][0] * cm[0][col];
    }
    Transform { basis, origin }
}

// ---------------------------------------------------------------------------------------------------------------
// The callback

/// How `addSingleResult` gets the normal (its second argument).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NormalSpace {
    /// `normalInWorldSpace` true: copied as it is (every call 0xA02D0 makes).
    World,
    /// False: rotated by the collision object's basis (its world transform, +10h).
    Object(Basis),
}

/// The callback's hit: what `addSingleResult` 0x84040 stores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// `m_collisionObject` (+8): the object's address, as an opaque id.
    pub object: u32,
    /// `m_hitNormalWorld` (+40h, its fourth component +0).
    pub normal: Vec3,
    /// `m_hitPointWorld` (+50h; its fourth component is not written).
    pub point: Vec3,
}

/// `ClosestRayResultCallback` (vtable 0x9E3F20, made by 0x83FD0): the ray's ends, the closest fraction so far, the
/// hit kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestRayCallback {
    /// `m_rayFromWorld` (+20h), `m_rayToWorld` (+30h).
    pub from: Vec3,
    pub to: Vec3,
    /// `m_closestHitFraction` (+4): 1 when made.
    pub closest: f32,
    pub hit: Option<RayHit>,
}

impl ClosestRayCallback {
    /// 0x83FD0's callback: fraction 1, no hit.
    pub fn new(from: Vec3, to: Vec3) -> ClosestRayCallback {
        ClosestRayCallback { from, to, closest: ONE, hit: None }
    }

    /// 0x84040 `addSingleResult(rayResult, normalInWorldSpace)`: stores whatever it is given (no test of the
    /// fraction; Bullet's assert is not compiled), returns the fraction.
    pub fn add_single_result(&mut self, object: u32, normal: Vec3, fraction: f32, space: NormalSpace) -> f32 {
        let normal = match space {
            NormalSpace::World => normal,
            // ORDER (asm 0x84071..0x840F0): each row (b[r][1]*y + b[r][2]*z) + b[r][0]*x.
            NormalSpace::Object(b) => [
                (b[0][1] * normal[1] + b[0][2] * normal[2]) + b[0][0] * normal[0],
                (b[1][1] * normal[1] + b[1][2] * normal[2]) + b[1][0] * normal[0],
                (b[2][1] * normal[1] + b[2][2] * normal[2]) + b[2][0] * normal[0],
            ],
        };
        // setInterpolate3(from, to, f): from*(1 - f) + f*to per component (0x84118..0x84174).
        let s = ONE - fraction;
        let f = self.from;
        let t = self.to;
        let point = [f[0] * s + fraction * t[0], f[1] * s + fraction * t[1], f[2] * s + fraction * t[2]];
        self.closest = fraction;
        self.hit = Some(RayHit { object, normal, point });
        fraction
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The whole call

/// A collision shape as 0xA02D0 branches on it.
#[derive(Clone, Debug, PartialEq)]
pub enum RayShape {
    /// Type under 20.
    Convex(ConvexShape),
    /// Type 21 (`btBvhTriangleMeshShape`): the triangles its BVH hands `processTriangle` for this ray, in that
    /// order (the BVH walk 0xC39B0 is not ported: the caller gives what it offers).
    Mesh(Vec<[Vec3; 3]>),
    /// Type 31: the children the compound's tree offers for this ray, in that order (the tree's walk 0xA54A0 is not
    /// ported), each with its own transform in the compound.
    Compound(Vec<CompoundChild>),
}

/// A compound's child: its transform in the compound (the child array's +0) and its shape (+40h).
#[derive(Clone, Debug, PartialEq)]
pub struct CompoundChild {
    pub transform: Transform,
    pub shape: RayShape,
}

/// 0xA02D0 `rayTestSingle(rayFromTrans, rayToTrans, object, shape, colObjWorldTransform, callback)`. `flags` is the
/// callback's `m_flags` (+10h): 0 for the closest callback 0x83FD0 makes and for the compound's `LocalInfoAdder`
/// (0xA1457), which also starts at the user callback's fraction and copies it back after each child; so a
/// compound's children report into the same callback here, one after the other.
#[allow(clippy::too_many_arguments)]
pub fn ray_test_single(
    from: &Transform,
    to: &Transform,
    object: u32,
    shape: &RayShape,
    world: &Transform,
    flags: RayFlags,
    callback: &mut ClosestRayCallback,
) {
    match shape {
        RayShape::Convex(convex) => {
            let trace = ray_convex(from, to, convex, world, callback.closest);
            if let ConvexRay::Hit { fraction, normal } = trace.answer {
                callback.add_single_result(object, normal, fraction, NormalSpace::World);
            }
        }
        RayShape::Mesh(triangles) => {
            let (from_local, to_local) = mesh_ray(from.origin, to.origin, world);
            // `rcb.m_hitFraction = resultCallback.m_closestHitFraction` (0xA09A4).
            let mut hit_fraction = callback.closest;
            for tri in triangles {
                if let TriangleRay::Hit { normal, distance } = ray_triangle(from_local, to_local, flags, hit_fraction, tri) {
                    let n = bridge_normal(&world.basis, normal);
                    // `m_hitFraction = reportHit(...)`: addSingleResult's answer, the distance.
                    hit_fraction = callback.add_single_result(object, n, distance, NormalSpace::World);
                }
            }
        }
        RayShape::Compound(children) => {
            for child in children {
                let child_world = compose_child(world, &child.transform);
                ray_test_single(from, to, object, &child.shape, &child_world, RayFlags(0), callback);
            }
        }
    }
}
