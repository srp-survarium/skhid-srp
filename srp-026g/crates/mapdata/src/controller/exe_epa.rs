//! The penetration-depth solver of the 0.26g executable and the detector's use of its answer.
//!
//! Scalar SSE `f32` as the asm does it; every sum of three or more terms is grouped as the asm groups it (marked
//! `ORDER (asm ...)` where that is not Bullet's source order). Square roots go through the x87 (`call 0x38960`) at
//! 24-bit precision, so `f32::sqrt`; `fld1; fdivrp` reciprocals are `1.0 / x` in `f32`; an x87 product of a 24-bit
//! value and an `f32` is an `f32` product. Where the asm multiplies by a reciprocal Bullet's source divides; the asm
//! is followed.
//!
//! Proven against the exe run in Unicorn: `calcPenDepth`'s answer bit for bit on 10,666 recorded calls (the game's
//! own on traced steps, the exe's on the port's inputs of a long replay, and made cases reaching the `Distance` path
//! and the EPA's fallbacks), the detector's whole call on 6,146 (`docs/notes/exactness_rig.md`). Reached by no record
//! and ported from the asm alone: the EPA running out of faces or vertices, GJK's 128-round failure, the detector's
//! method 9.
// The exe's comparisons are ported as written: `!(a > b)` and `a > b` differ on NaN, as `comiss` branches do.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
use super::exe_gjk::{self as gjk, CastShape, GjkClosest, ShapeB, Transform, Vec3};

// --- constants (bit patterns from the exe's data) --------------------------------------------------------------

/// 0xA2D840: 1e-4: GJK_MIN_DISTANCE, GJK_DUPLICATED_EPS, GJK_ACCURARY, EPA_ACCURACY (all four read from it).
const EPS_1E4: f32 = f32::from_bits(0x38D1_B717);
/// 0xA2DA94: -1e-5, `-EPA_PLANE_EPS`.
const MINUS_EPA_PLANE_EPS: f32 = f32::from_bits(0xB727_C5AC);
/// 0xA2DA98: -0.01, `-EPA_INSIDE_EPS`.
const MINUS_EPA_INSIDE_EPS: f32 = f32::from_bits(0xBC23_D70A);
/// 0xA2DA9C: 2^-46 (`SIMD_EPSILON * SIMD_EPSILON`).
const SIMD_EPSILON_SQUARED: f32 = f32::from_bits(0x2880_0000);
/// 0xA2D778: -1.
const MINUS_ONE: f32 = -1.0;
/// 0xA70884: 1.
const ONE: f32 = 1.0;
/// `GJK_MAX_ITERATIONS`: the immediate 0x80 at 0xD3668.
const GJK_MAX_ITERATIONS: u32 = 128;
/// `EPA_MAX_VERTICES`: the immediate 0x40 at 0xD4FCF.
const EPA_MAX_VERTICES: usize = 64;
/// `EPA_MAX_FACES`: the stock's 128 faces (0xD4C46: 32 rounds of 4).
const EPA_MAX_FACES: usize = 128;
/// `EPA_MAX_ITERATIONS`: the immediate 0xFF at 0xD5150.
const EPA_MAX_ITERATIONS: u32 = 255;
/// 0x9E5130 `imd3` / `i1m3` {1, 2, 0}; 0x9E513C `i2m3` {2, 0, 1}.
const I1M3: [usize; 3] = [1, 2, 0];
const I2M3: [usize; 3] = [2, 0, 1];

// --- vector helpers --------------------------------------------------------------------------------------------

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A sign flip per component (`xorps` with the sign mask 0xA2D7C0): -(+0) is -0.
fn neg(a: Vec3) -> Vec3 {
    [-a[0], -a[1], -a[2]]
}

/// `btCross(u, v)`: every product is commutative, so any operand order the asm takes gives these bits.
fn cross(u: Vec3, v: Vec3) -> Vec3 {
    [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
}

/// (x*x + y*y) + z*z.
fn len2_xyz(v: Vec3) -> f32 {
    (v[0] * v[0] + v[1] * v[1]) + v[2] * v[2]
}

/// (z*z + y*y) + x*x.
fn len2_zyx(v: Vec3) -> f32 {
    (v[2] * v[2] + v[1] * v[1]) + v[0] * v[0]
}

/// (v.z*c.z + v.y*c.y) + v.x*c.x.
fn dot_zyx(v: Vec3, c: Vec3) -> f32 {
    (v[2] * c[2] + v[1] * c[1]) + v[0] * c[0]
}

// --- the shapes as the solver reads them ------------------------------------------------------------------------

/// How the solver's supports read a shape (`tShape::Ls`, set by `Initialize` 0xD5B80 at 0xD6256..0xD6271).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ls {
    /// `Penetration`: 0x8DD40 `localGetSupportVertexNonVirtual`, the margin added along the unit direction.
    WithMargin,
    /// `Distance`: 0x8D680 `localGetSupportVertexWithoutMarginNonVirtual`.
    WithoutMargin,
}

/// A shape and the margin 0x8DD40 adds for it (its switch 0x8DE17: types 0, 1, 4, 5, 10, 13 read +30h
/// `m_collisionMargin`, a sphere (8) its radius +20h times its scaling's x +10h).
pub struct Convex<'a, S> {
    pub shape: &'a S,
    pub margin: f32,
}

/// What a shape's support without margin is (0x8D680).
pub trait Support {
    fn support(&self, dir: Vec3) -> Vec3;
}

impl Support for CastShape {
    fn support(&self, dir: Vec3) -> Vec3 {
        gjk::support_cast_shape(self, dir)
    }
}

/// Shape B through the detector's own trait.
pub struct B<'a, T: ShapeB>(pub &'a T);

impl<T: ShapeB> Support for B<'_, T> {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.0.support(dir)
    }
}

/// 0x8DD40 `btConvexShape::localGetSupportVertexNonVirtual`.
fn support_with_margin<S: Support>(shape: &S, margin: f32, dir: Vec3) -> Vec3 {
    // ORDER (asm 0x8DD6D..0x8DD8C): (y*y + z*z) + x*x.
    let len2 = (dir[1] * dir[1] + dir[2] * dir[2]) + dir[0] * dir[0];
    // 0x8DD98: `comiss 2^-46, len2; jna`: the direction (-1, -1, -1) when 2^-46 is over the length.
    let v = if SIMD_EPSILON_SQUARED > len2 { [MINUS_ONE; 3] } else { dir };
    let len2 = (v[1] * v[1] + v[2] * v[2]) + v[0] * v[0];
    let rlen = ONE / len2.sqrt();
    let n = [v[0] * rlen, v[1] * rlen, v[2] * rlen];
    let s = shape.support(n);
    // 0x8DE50..0x8DEBF: support + n * margin.
    [s[0] + n[0] * margin, s[1] + n[1] * margin, s[2] + n[2] * margin]
}

fn ls<S: Support>(mode: Ls, c: &Convex<S>, dir: Vec3) -> Vec3 {
    match mode {
        Ls::WithMargin => support_with_margin(c.shape, c.margin, dir),
        Ls::WithoutMargin => c.shape.support(dir),
    }
}

/// `btGjkEpaSolver2`'s `MinkowskiDiff` (`tShape`), as `Initialize` 0xD5B80 fills it.
struct Minkowski<'a, SA, SB> {
    a: Convex<'a, SA>,
    b: Convex<'a, SB>,
    /// `m_toshape1` (+10h): `wtrs1.basis.transposeTimes(wtrs0.basis)`.
    to1: [[f32; 3]; 3],
    /// `m_toshape0` (+40h): `wtrs0.inverseTimes(wtrs1)`.
    to0: Transform,
    ls: Ls,
}

impl<SA: Support, SB: Support> Minkowski<'_, SA, SB> {
    fn new<'a>(a: Convex<'a, SA>, wtrs0: &Transform, b: Convex<'a, SB>, wtrs1: &Transform, ls: Ls) -> Minkowski<'a, SA, SB> {
        let (b0, b1) = (&wtrs0.basis, &wtrs1.basis);
        // 0xD5BD1..0xD5E42: entry (i, j) = (b1[0][i]*b0[0][j] + b1[1][i]*b0[1][j]) + b1[2][i]*b0[2][j].
        let to1 = [0, 1, 2].map(|i| [0, 1, 2].map(|j| (b1[0][i] * b0[0][j] + b1[1][i] * b0[1][j]) + b1[2][i] * b0[2][j]));
        // 0xD5F3F..0xD618A: entry (i, j) = (b0[0][i]*b1[0][j] + b0[1][i]*b1[1][j]) + b0[2][i]*b1[2][j].
        let basis = [0, 1, 2].map(|i| [0, 1, 2].map(|j| (b0[0][i] * b1[0][j] + b0[1][i] * b1[1][j]) + b0[2][i] * b1[2][j]));
        // 0xD5EB2..0xD5F57: v = origin1 - origin0; origin = v * basis0, (b0[1][i]*v.y + b0[2][i]*v.z) + b0[0][i]*v.x.
        let v = sub(wtrs1.origin, wtrs0.origin);
        let origin = [0, 1, 2].map(|i| (b0[1][i] * v[1] + b0[2][i] * v[2]) + b0[0][i] * v[0]);
        Minkowski { a, b, to1, to0: Transform { basis, origin }, ls }
    }

    /// `Support0`: shape A's support in its own frame.
    fn support0(&self, d: Vec3) -> Vec3 {
        ls(self.ls, &self.a, d)
    }

    /// 0xD2B70 `Support1`: `m_toshape0 * Ls(shape B, m_toshape1 * d)`.
    fn support1(&self, d: Vec3) -> Vec3 {
        let m = &self.to1;
        // ORDER (asm 0xD2B87..0xD2C05): each row (m[r][1]*y + m[r][2]*z) + m[r][0]*x.
        let v = [0, 1, 2].map(|r| (m[r][1] * d[1] + m[r][2] * d[2]) + m[r][0] * d[0]);
        let s = ls(self.ls, &self.b, v);
        let t = &self.to0;
        // ORDER (asm 0xD2C2C..0xD2CA3): each row ((t[r][1]*y + t[r][2]*z) + t[r][0]*x) + origin.
        [0, 1, 2].map(|r| ((t.basis[r][1] * s[1] + t.basis[r][2] * s[2]) + t.basis[r][0] * s[0]) + t.origin[r])
    }

    /// 0xD2CC0 `Support`: `Support0(d) - Support1(-d)`.
    fn support(&self, d: Vec3) -> Vec3 {
        let s0 = self.support0(d);
        let s1 = self.support1(neg(d));
        sub(s0, s1)
    }
}

// --- GJK (btGjkEpa2.cpp's, not the detector's) -------------------------------------------------------------------

/// `sSV`: a support direction and the Minkowski difference's point along it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sv {
    pub d: Vec3,
    pub w: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
struct Simplex {
    /// Indices into the vertex store (GJK's four first, then EPA's).
    c: [usize; 4],
    p: [f32; 4],
    rank: usize,
}

/// `GJK::eStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GjkStatus {
    Valid = 0,
    Inside = 1,
    Failed = 2,
}

/// The vertices: GJK's `m_store[4]` (0..4) and EPA's `m_sv_store[64]` (4..68).
struct Store {
    sv: Vec<Sv>,
}

struct Gjk {
    free: [usize; 4],
    nfree: usize,
    simplices: [Simplex; 2],
    current: usize,
    ray: Vec3,
    distance: f32,
    status: GjkStatus,
    rounds: u32,
}

/// 0xD3D70 `getsupport(d, sv)`: `sv.d = d / |d|`, `sv.w = Support(sv.d)`.
fn getsupport<SA: Support, SB: Support>(shape: &Minkowski<SA, SB>, d: Vec3) -> Sv {
    let rlen = ONE / len2_xyz(d).sqrt();
    let n = [d[0] * rlen, d[1] * rlen, d[2] * rlen];
    Sv { d: n, w: shape.support(n) }
}

/// 0xD3E90 `projectorigin(a, b, w, m)`: the closest point of segment ab to the origin.
fn project_segment(a: Vec3, b: Vec3, w: &mut [f32], m: &mut u32) -> f32 {
    let d = sub(b, a);
    // ORDER (asm 0xD3EC0..0xD3EDD): (x*x + z*z) + y*y.
    let l = (d[0] * d[0] + d[2] * d[2]) + d[1] * d[1];
    // 0xD3EE1: GJK_SIMPLEX2_EPS (0xA0D984) 0.
    if !(l > 0.0) {
        return MINUS_ONE;
    }
    // ORDER (asm 0xD3EF4..0xD3F17): -(((a.z*d.z + a.x*d.x) + a.y*d.y) / l).
    let t = -(((a[2] * d[2] + a[0] * d[0]) + a[1] * d[1]) / l);
    if t >= ONE {
        w[0] = 0.0;
        w[1] = ONE;
        *m = 2;
        // ORDER (asm 0xD3F2E..0xD3F64): (y*y + z*z) + x*x.
        (b[1] * b[1] + b[2] * b[2]) + b[0] * b[0]
    } else if 0.0 >= t {
        w[0] = ONE;
        w[1] = 0.0;
        *m = 1;
        (a[1] * a[1] + a[2] * a[2]) + a[0] * a[0]
    } else {
        w[1] = t;
        w[0] = ONE - t;
        *m = 3;
        let p = [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t];
        // ORDER (asm 0xD3FD3..0xD4002): (z*z + x*x) + y*y.
        (p[2] * p[2] + p[0] * p[0]) + p[1] * p[1]
    }
}

/// 0xD4020 `projectorigin(a, b, c, w, m)`: the closest point of triangle abc to the origin.
fn project_triangle(a: Vec3, b: Vec3, c: Vec3, w: &mut [f32], m: &mut u32) -> f32 {
    let vt = [a, b, c];
    let dl = [sub(a, b), sub(b, c), sub(c, a)];
    let n = cross(dl[0], dl[1]);
    // ORDER (asm 0xD411C..0xD4135): (z*z + y*y) + x*x.
    let l = len2_zyx(n);
    if !(l > 0.0) {
        return MINUS_ONE;
    }
    let mut mindist = MINUS_ONE;
    // The sub-result lives across the loop (a segment that returns -1 leaves it as it was).
    let mut subw = [0.0f32; 2];
    let mut subm = 0u32;
    for i in 0..3 {
        // ORDER (asm 0xD41FF..0xD4222): (v.z*c.z + v.y*c.y) + v.x*c.x.
        if dot_zyx(vt[i], cross(dl[i], n)) > 0.0 {
            let j = I1M3[i];
            let subd = project_segment(vt[i], vt[j], &mut subw, &mut subm);
            if 0.0 > mindist || mindist > subd {
                mindist = subd;
                *m = (if subm & 1 != 0 { 1 << i } else { 0 }) + (if subm & 2 != 0 { 1 << j } else { 0 });
                w[i] = subw[0];
                w[j] = subw[1];
                w[I1M3[j]] = 0.0;
            }
        }
    }
    if 0.0 > mindist {
        // x87 (0xD4304..0xD431F): 1 / sqrt(l), stored as f32 (0xD43C9).
        let rs = ONE / l.sqrt();
        // ORDER (asm 0xD4321..0xD4352): (a.x*n.x + a.y*n.y) + n.z*a.z, divided by l.
        let t = ((a[0] * n[0] + a[1] * n[1]) + n[2] * a[2]) / l;
        let p = [n[0] * t, n[1] * t, n[2] * t];
        // ORDER (asm 0xD4368..0xD4381): (z*z + y*y) + x*x.
        mindist = len2_zyx(p);
        *m = 7;
        // ORDER (asm 0xD43AD..0xD440C, 0xD4450..0xD44A4): each cross's length2 is (z*z + y*y) + x*x, its root times
        // 1 / sqrt(l) (Bullet's source divides by sqrt(l)).
        let w0 = len2_zyx(cross(dl[1], sub(b, p))).sqrt() * rs;
        let w1 = len2_zyx(cross(dl[2], sub(c, p))).sqrt() * rs;
        w[0] = w0;
        w[1] = w1;
        // 0xD44BF..0xD44CA: 1 - (w1 + w0).
        w[2] = ONE - (w1 + w0);
    }
    mindist
}

/// One of the inline determinants: ((((t1 + t2) - t3) - t4) + t5) - t6 of `a.y*b.z*c.x + a.z*b.x*c.y -
/// a.x*b.z*c.y - a.y*b.x*c.z + a.x*b.y*c.z - a.z*b.y*c.x`, each product associated as given.
struct Det6 {
    t: [f32; 6],
}

impl Det6 {
    fn sum(&self) -> f32 {
        ((((self.t[0] + self.t[1]) - self.t[2]) - self.t[3]) + self.t[4]) - self.t[5]
    }
}

/// The determinant of `projectorigin`'s tetrahedron (0xD45BB..0xD4648, `det(dl0, dl1, dl2)`).
fn det_project(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    Det6 {
        t: [
            (b[2] * a[1]) * c[0],
            (c[1] * a[2]) * b[0],
            (c[1] * b[2]) * a[0],
            (c[2] * a[1]) * b[0],
            (c[2] * b[1]) * a[0],
            (a[2] * b[1]) * c[0],
        ],
    }
    .sum()
}

/// The determinant `EncloseOrigin` (0xD3CD3..0xD3D31) and `EPA::Evaluate` (0xD4E53..0xD4EB0) take of
/// `(c0 - c3, c1 - c3, c2 - c3)`: (t2 + t1) - t3 - t4 + t5 - t6, t1 (a.y*b.z)*c.x, t2 (a.z*c.y)*b.x, t3 (b.z*c.y)*a.x,
/// t4 (a.y*c.z)*b.x, t5 (b.y*c.z)*a.x, t6 (a.z*b.y)*c.x.
fn det_simplex(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let t1 = (a[1] * b[2]) * c[0];
    let t2 = (a[2] * c[1]) * b[0];
    let t3 = (b[2] * c[1]) * a[0];
    let t4 = (a[1] * c[2]) * b[0];
    let t5 = (b[1] * c[2]) * a[0];
    let t6 = (a[2] * b[1]) * c[0];
    ((((t2 + t1) - t3) - t4) + t5) - t6
}

/// 0xD44F0 `projectorigin(a, b, c, d, w, m)`: the closest point of tetrahedron abcd to the origin.
fn project_tetra(a: Vec3, b: Vec3, c: Vec3, d: Vec3, w: &mut [f32; 4], m: &mut u32) -> f32 {
    let vt = [a, b, c, d];
    let dl = [sub(a, d), sub(b, d), sub(c, d)];
    let vl = det_project(dl[0], dl[1], dl[2]);
    // ORDER (asm 0xD46F5..0xD4709): ((a.y*cy + a.x*cx) + a.z*cz) * vl.
    let cr = cross(sub(b, c), sub(a, b));
    let ng = ((a[1] * cr[1] + a[0] * cr[0]) + a[2] * cr[2]) * vl;
    // 0xD4710: `comiss 0, x; jc`: on when 0 is not under it; 0xD4719..0xD4731: |vl| over GJK_SIMPLEX4_EPS 0.
    if !(0.0 >= ng) || !(vl.abs() > 0.0) {
        return MINUS_ONE;
    }
    let mut mindist = MINUS_ONE;
    let mut subw = [0.0f32; 3];
    let mut subm = 0u32;
    for i in 0..3 {
        let j = I1M3[i];
        // ORDER (asm 0xD47BD..0xD47EE): ((d.x*cx + cy*d.y) + d.z*cz) * vl.
        let cr = cross(dl[i], dl[j]);
        let s = ((d[0] * cr[0] + cr[1] * d[1]) + d[2] * cr[2]) * vl;
        if s > 0.0 {
            let subd = project_triangle(vt[i], vt[j], d, &mut subw, &mut subm);
            if 0.0 > mindist || mindist > subd {
                mindist = subd;
                *m = (if subm & 1 != 0 { 1 << i } else { 0 })
                    + (if subm & 2 != 0 { 1 << j } else { 0 })
                    + (if subm & 4 != 0 { 8 } else { 0 });
                w[i] = subw[0];
                w[j] = subw[1];
                w[I1M3[j]] = 0.0;
                w[3] = subw[2];
            }
        }
    }
    if 0.0 > mindist {
        mindist = 0.0;
        *m = 15;
        // 0xD4930: 1 / vl in SSE; each weight is a determinant times it (Bullet's source divides).
        let r = ONE / vl;
        // det(c, b, d): T1 (c.y*b.z)*d.x, T2 (d.y*c.z)*b.x, T3 (d.y*b.z)*c.x, T4 (d.z*c.y)*b.x, T5 (b.y*d.z)*c.x,
        // T6 (b.y*c.z)*d.x (0xD48DC..0xD49B8).
        let w0 = Det6 {
            t: [
                (c[1] * b[2]) * d[0],
                (d[1] * c[2]) * b[0],
                (d[1] * b[2]) * c[0],
                (d[2] * c[1]) * b[0],
                (b[1] * d[2]) * c[0],
                (b[1] * c[2]) * d[0],
            ],
        }
        .sum()
            * r;
        // det(a, c, d): T1 (a.y*c.z)*d.x, T2 (a.z*d.y)*c.x, T3 (d.y*c.z)*a.x, T4 (a.y*d.z)*c.x, T5 (d.z*c.y)*a.x,
        // T6 (a.z*c.y)*d.x (0xD49AC..0xD4A3E).
        let w1 = Det6 {
            t: [
                (a[1] * c[2]) * d[0],
                (a[2] * d[1]) * c[0],
                (d[1] * c[2]) * a[0],
                (a[1] * d[2]) * c[0],
                (d[2] * c[1]) * a[0],
                (a[2] * c[1]) * d[0],
            ],
        }
        .sum()
            * r;
        // det(b, a, d): T1 (a.z*b.y)*d.x, T2 (d.y*b.z)*a.x, T3 (a.z*d.y)*b.x, T4 (b.y*d.z)*a.x, T5 (a.y*d.z)*b.x,
        // T6 (a.y*b.z)*d.x (0xD4A4C..0xD4AA2).
        let w2 = Det6 {
            t: [
                (a[2] * b[1]) * d[0],
                (d[1] * b[2]) * a[0],
                (a[2] * d[1]) * b[0],
                (b[1] * d[2]) * a[0],
                (a[1] * d[2]) * b[0],
                (a[1] * b[2]) * d[0],
            ],
        }
        .sum()
            * r;
        w[0] = w0;
        w[1] = w1;
        w[2] = w2;
        // ORDER (asm 0xD4ABB..0xD4AC3): 1 - ((w2 + w1) + w0); the source's is 1 - ((w0 + w1) + w2).
        w[3] = ONE - ((w2 + w1) + w0);
    }
    mindist
}

impl Gjk {
    /// 0xD3E50 `appendvertice(simplex, v)` (inlined the same at 0xD2EBF and in `EncloseOrigin`).
    fn append<SA: Support, SB: Support>(&mut self, s: usize, store: &mut Store, shape: &Minkowski<SA, SB>, v: Vec3) {
        let rank = self.simplices[s].rank;
        self.simplices[s].p[rank] = 0.0;
        self.nfree -= 1;
        let id = self.free[self.nfree];
        self.simplices[s].c[rank] = id;
        self.simplices[s].rank += 1;
        store.sv[id] = getsupport(shape, v);
    }

    /// 0xD3E30 `removevertice(simplex)`.
    fn remove(&mut self, s: usize) {
        self.simplices[s].rank -= 1;
        let id = self.simplices[s].c[self.simplices[s].rank];
        self.free[self.nfree] = id;
        self.nfree += 1;
    }

    /// 0xD2D80 `GJK::Evaluate(shape, guess)`.
    fn evaluate<SA: Support, SB: Support>(store: &mut Store, shape: &Minkowski<SA, SB>, guess: Vec3) -> Gjk {
        let mut g = Gjk {
            free: [0, 1, 2, 3],
            nfree: 4,
            simplices: [Simplex::default(); 2],
            current: 0,
            ray: guess,
            distance: 0.0,
            status: GjkStatus::Valid,
            rounds: 0,
        };
        let mut iterations = 0u32;
        let mut alpha = 0.0f32;
        // ORDER (asm 0xD2E21..0xD2E4F): (x*x + y*y) + z*z.
        let sqrl = len2_xyz(g.ray);
        let first = if sqrl > 0.0 { neg(g.ray) } else { [ONE, 0.0, 0.0] };
        g.append(0, store, shape, first);
        g.simplices[0].p[0] = ONE;
        g.ray = store.sv[g.simplices[0].c[0]].w;
        let mut lastw = [g.ray; 4];
        let mut clastw = 0usize;
        loop {
            g.rounds += 1;
            let next = 1 - g.current;
            let cs = g.current;
            let rl = len2_xyz(g.ray).sqrt();
            // 0xD3021..0xD302E: `fld 1e-4; fcomip`: inside when 1e-4 is over |ray|.
            if EPS_1E4 > rl {
                g.status = GjkStatus::Inside;
                break;
            }
            g.append(cs, store, shape, neg(g.ray));
            let w = store.sv[g.simplices[cs].c[g.simplices[cs].rank - 1]].w;
            // ORDER (asm 0xD30E0..0xD3110): (dz*dz + dy*dy) + dx*dx; found when 1e-4 is over it.
            let found = lastw.iter().any(|l| EPS_1E4 > len2_zyx(sub(w, *l)));
            if found {
                g.remove(g.current);
                break;
            }
            clastw = (clastw + 1) & 3;
            lastw[clastw] = w;
            // ORDER (asm 0xD3155..0xD317F): ((ray.z*w.z + ray.y*w.y) + ray.x*w.x) / rl.
            let omega = dot_zyx(g.ray, w) / rl;
            alpha = if omega > alpha { omega } else { alpha };
            // 0xD319F..0xD31B7: (rl - alpha) - rl * 1e-4, at most 0 stops.
            if 0.0 >= (rl - alpha) - rl * EPS_1E4 {
                g.remove(g.current);
                break;
            }
            let mut weights = [0.0f32; 4];
            let mut mask = 0u32;
            let c = g.simplices[cs].c;
            let rank = g.simplices[cs].rank;
            let sqdist = match rank {
                2 => project_segment(store.sv[c[0]].w, store.sv[c[1]].w, &mut weights, &mut mask),
                3 => project_triangle(store.sv[c[0]].w, store.sv[c[1]].w, store.sv[c[2]].w, &mut weights, &mut mask),
                4 => project_tetra(
                    store.sv[c[0]].w,
                    store.sv[c[1]].w,
                    store.sv[c[2]].w,
                    store.sv[c[3]].w,
                    &mut weights,
                    &mut mask,
                ),
                _ => unreachable!("a simplex of rank {rank} after an append"),
            };
            // 0xD3255: `comiss sqdist, 0; jc`: negative or NaN returns the old simplex.
            if !(sqdist >= 0.0) {
                g.remove(g.current);
                break;
            }
            g.simplices[next].rank = 0;
            g.ray = [0.0; 3];
            g.current = next;
            for i in 0..rank {
                if mask & (1 << i) != 0 {
                    let nr = g.simplices[next].rank;
                    g.simplices[next].c[nr] = c[i];
                    g.simplices[next].p[nr] = weights[i];
                    g.simplices[next].rank += 1;
                    let wi = store.sv[c[i]].w;
                    g.ray = [g.ray[0] + wi[0] * weights[i], g.ray[1] + wi[1] * weights[i], g.ray[2] + wi[2] * weights[i]];
                } else {
                    g.free[g.nfree] = c[i];
                    g.nfree += 1;
                }
            }
            if mask == 15 {
                g.status = GjkStatus::Inside;
            }
            iterations += 1;
            if iterations >= GJK_MAX_ITERATIONS {
                g.status = GjkStatus::Failed;
            }
            if g.status != GjkStatus::Valid {
                break;
            }
        }
        match g.status {
            GjkStatus::Valid => g.distance = len2_xyz(g.ray).sqrt(),
            GjkStatus::Inside => g.distance = 0.0,
            GjkStatus::Failed => {}
        }
        g
    }

    /// 0xD3770 `EncloseOrigin` on `m_simplex` (the current simplex).
    fn enclose_origin<SA: Support, SB: Support>(&mut self, store: &mut Store, shape: &Minkowski<SA, SB>) -> bool {
        let s = self.current;
        let c = self.simplices[s].c;
        match self.simplices[s].rank {
            1 => {
                for i in 0..3 {
                    let mut axis = [0.0f32; 3];
                    axis[i] = ONE;
                    self.append(s, store, shape, axis);
                    if self.enclose_origin(store, shape) {
                        return true;
                    }
                    self.remove(s);
                    self.append(s, store, shape, neg(axis));
                    if self.enclose_origin(store, shape) {
                        return true;
                    }
                    self.remove(s);
                }
                false
            }
            2 => {
                let d = sub(store.sv[c[1]].w, store.sv[c[0]].w);
                for i in 0..3 {
                    let mut axis = [0.0f32; 3];
                    axis[i] = ONE;
                    let p = cross(d, axis);
                    // ORDER (asm 0xD39B1..0xD39D6): (z*z + y*y) + x*x.
                    if len2_zyx(p) > 0.0 {
                        self.append(s, store, shape, p);
                        if self.enclose_origin(store, shape) {
                            return true;
                        }
                        self.remove(s);
                        self.append(s, store, shape, neg(p));
                        if self.enclose_origin(store, shape) {
                            return true;
                        }
                        self.remove(s);
                    }
                }
                false
            }
            3 => {
                let n = cross(sub(store.sv[c[1]].w, store.sv[c[0]].w), sub(store.sv[c[2]].w, store.sv[c[0]].w));
                // ORDER (asm 0xD3B69..0xD3B9F): (z*z + y*y) + x*x.
                if len2_zyx(n) > 0.0 {
                    self.append(s, store, shape, n);
                    if self.enclose_origin(store, shape) {
                        return true;
                    }
                    self.remove(s);
                    self.append(s, store, shape, neg(n));
                    if self.enclose_origin(store, shape) {
                        return true;
                    }
                    self.remove(s);
                }
                false
            }
            4 => {
                let w3 = store.sv[c[3]].w;
                let det = det_simplex(sub(store.sv[c[0]].w, w3), sub(store.sv[c[1]].w, w3), sub(store.sv[c[2]].w, w3));
                det.abs() > 0.0
            }
            _ => false,
        }
    }
}

// --- EPA ------------------------------------------------------------------------------------------------------------

/// `EPA::eStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpaStatus {
    Valid = 0,
    Touching = 1,
    Degenerated = 2,
    NonConvex = 3,
    InvalidHull = 4,
    OutOfFaces = 5,
    OutOfVertices = 6,
    AccuraryReached = 7,
    FallBack = 8,
    Failed = 9,
}

/// `sFace` (0x40 bytes: n +0, d +10h, p +14h, c +18h, f +24h, l +30h, e +38h, pass +3Bh).
#[derive(Clone, Copy, Debug, Default)]
struct Face {
    n: Vec3,
    d: f32,
    p: f32,
    c: [usize; 3],
    f: [usize; 3],
    l: [Option<usize>; 2],
    e: [u8; 3],
    pass: u8,
}

#[derive(Clone, Copy, Debug, Default)]
struct List {
    root: Option<usize>,
    count: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Horizon {
    cf: Option<usize>,
    ff: Option<usize>,
    nf: u32,
}

struct Epa {
    status: EpaStatus,
    /// `m_result`: rank, vertices, weights.
    rank: usize,
    c: [usize; 3],
    p: [f32; 3],
    normal: Vec3,
    depth: f32,
    faces: Vec<Face>,
    nextsv: usize,
    hull: List,
    stock: List,
    rounds: u32,
}

/// 0xD4BA0 `append(list, face)`.
fn list_append(list: &mut List, faces: &mut [Face], i: usize) {
    faces[i].l[0] = None;
    faces[i].l[1] = list.root;
    if let Some(r) = list.root {
        faces[r].l[0] = Some(i);
    }
    list.root = Some(i);
    list.count += 1;
}

/// 0xD4BC0 `remove(list, face)`.
fn list_remove(list: &mut List, faces: &mut [Face], i: usize) {
    if let Some(n) = faces[i].l[1] {
        faces[n].l[0] = faces[i].l[0];
    }
    if let Some(p) = faces[i].l[0] {
        faces[p].l[1] = faces[i].l[1];
    }
    if list.root == Some(i) {
        list.root = faces[i].l[1];
    }
    list.count -= 1;
}

/// `bind(fa, ea, fb, eb)`.
fn bind(faces: &mut [Face], fa: usize, ea: usize, fb: usize, eb: usize) {
    faces[fa].e[ea] = eb as u8;
    faces[fa].f[ea] = fb;
    faces[fb].e[eb] = ea as u8;
    faces[fb].f[eb] = fa;
}

impl Epa {
    /// 0xD4BF0, the constructor: status Failed, the 128 faces appended to the stock from the last.
    fn new() -> Epa {
        let mut e = Epa {
            status: EpaStatus::Failed,
            rank: 0,
            c: [0; 3],
            p: [0.0; 3],
            normal: [0.0; 3],
            depth: 0.0,
            faces: vec![Face::default(); EPA_MAX_FACES],
            nextsv: 0,
            hull: List::default(),
            stock: List::default(),
            rounds: 0,
        };
        for i in 0..EPA_MAX_FACES {
            list_append(&mut e.stock, &mut e.faces, EPA_MAX_FACES - i - 1);
        }
        e
    }

    /// 0xD5550 `newface(a, b, c, forced)` (EDI a, stack: the EPA, b, c, forced).
    fn newface(&mut self, store: &Store, a: usize, b: usize, c: usize, forced: bool) -> Option<usize> {
        let Some(face) = self.stock.root else {
            // 0xD59DB: no face in the stock.
            self.status = EpaStatus::OutOfFaces;
            return None;
        };
        list_remove(&mut self.stock, &mut self.faces, face);
        list_append(&mut self.hull, &mut self.faces, face);
        let (wa, wb, wc) = (store.sv[a].w, store.sv[b].w, store.sv[c].w);
        let n = cross(sub(wb, wa), sub(wc, wa));
        {
            let f = &mut self.faces[face];
            f.pass = 0;
            f.c = [a, b, c];
            f.n = n;
        }
        // ORDER (asm 0xD567D..0xD56A3): (x*x + y*y) + z*z.
        let l = len2_xyz(n).sqrt();
        let v = l > EPS_1E4;
        // 0xD56D2..0xD58A5: the three edge terms, computed c, b, a, each (v.z*cz + v.y*cy) + v.x*cx.
        let tc = dot_zyx(wc, cross(n, sub(wc, wa)));
        let tb = dot_zyx(wb, cross(n, sub(wb, wc)));
        let ta = dot_zyx(wa, cross(n, sub(wa, wb)));
        // 0xD58C3..0xD58DF: min(min(Ta, Tb), Tc) as `comiss` picks (the second of equal ones).
        let m1 = if tb > ta { ta } else { tb };
        let m = if tc > m1 { m1 } else { tc };
        let mut p = m / if v { l } else { ONE };
        // 0xD58E7: `comiss p, -0.01; jc`: kept when under -0.01 (or NaN), else 0.
        if p >= MINUS_EPA_INSIDE_EPS {
            p = 0.0;
        }
        self.faces[face].p = p;
        if v {
            // 0xD590A: 1 / l in SSE; d = ((a.y*n.y + a.z*n.z) + n.x*a.x) * (1 / l); n times it.
            let r = ONE / l;
            let d = ((wa[1] * n[1] + wa[2] * n[2]) + n[0] * wa[0]) * r;
            let f = &mut self.faces[face];
            f.d = d;
            f.n = [n[0] * r, n[1] * r, n[2] * r];
            if forced || d >= MINUS_EPA_PLANE_EPS {
                return Some(face);
            }
            self.status = EpaStatus::NonConvex;
        } else {
            self.status = EpaStatus::Degenerated;
        }
        list_remove(&mut self.hull, &mut self.faces, face);
        list_append(&mut self.stock, &mut self.faces, face);
        None
    }

    /// 0xD59F0 `findbest`.
    fn findbest(&self) -> usize {
        let mut minf = self.hull.root.expect("a hull with faces");
        let mut mind = self.faces[minf].d * self.faces[minf].d;
        let mut maxp = self.faces[minf].p;
        let mut f = self.faces[minf].l[1];
        while let Some(i) = f {
            let sqd = self.faces[i].d * self.faces[i].d;
            // 0xD5A15..0xD5A26: `comiss p, maxp; jc` then `comiss mind, sqd; jna`.
            if self.faces[i].p >= maxp && mind > sqd {
                minf = i;
                mind = sqd;
                maxp = self.faces[i].p;
            }
            f = self.faces[i].l[1];
        }
        minf
    }

    /// 0xD5A40 `expand(pass, w, f, e, horizon)`.
    fn expand(&mut self, store: &Store, pass: u32, w: usize, f: usize, e: usize, horizon: &mut Horizon) -> bool {
        if self.faces[f].pass as u32 == pass {
            return false;
        }
        let e1 = I1M3[e];
        let ww = store.sv[w].w;
        let n = self.faces[f].n;
        // ORDER (asm 0xD5A5E..0xD5A8E): ((w.z*n.z + w.y*n.y) + n.x*w.x) - f.d; `comiss -1e-5, x; jna`.
        let x = dot_zyx(ww, n) - self.faces[f].d;
        if MINUS_EPA_PLANE_EPS > x {
            let (c_e1, c_e) = (self.faces[f].c[e1], self.faces[f].c[e]);
            let Some(nf) = self.newface(store, c_e1, c_e, w, false) else { return false };
            bind(&mut self.faces, nf, 0, f, e);
            match horizon.cf {
                Some(cf) => bind(&mut self.faces, cf, 1, nf, 2),
                None => horizon.ff = Some(nf),
            }
            horizon.cf = Some(nf);
            horizon.nf += 1;
            true
        } else {
            let e2 = I2M3[e];
            self.faces[f].pass = pass as u8;
            let (f1, ee1) = (self.faces[f].f[e1], self.faces[f].e[e1] as usize);
            if !self.expand(store, pass, w, f1, ee1, horizon) {
                return false;
            }
            let (f2, ee2) = (self.faces[f].f[e2], self.faces[f].e[e2] as usize);
            if !self.expand(store, pass, w, f2, ee2, horizon) {
                return false;
            }
            list_remove(&mut self.hull, &mut self.faces, f);
            list_append(&mut self.stock, &mut self.faces, f);
            true
        }
    }

    /// 0xD4D30 `EPA::Evaluate(gjk, guess)`.
    fn evaluate<SA: Support, SB: Support>(
        &mut self,
        g: &mut Gjk,
        store: &mut Store,
        shape: &Minkowski<SA, SB>,
        guess: Vec3,
    ) -> EpaStatus {
        let s = g.current;
        if g.simplices[s].rank > 1 && g.enclose_origin(store, shape) {
            while let Some(f) = self.hull.root {
                list_remove(&mut self.hull, &mut self.faces, f);
                list_append(&mut self.stock, &mut self.faces, f);
            }
            self.status = EpaStatus::Valid;
            self.nextsv = 0;
            let simplex = &mut g.simplices[s];
            let w3 = store.sv[simplex.c[3]].w;
            let det = det_simplex(
                sub(store.sv[simplex.c[0]].w, w3),
                sub(store.sv[simplex.c[1]].w, w3),
                sub(store.sv[simplex.c[2]].w, w3),
            );
            // 0xD4EB4: `comiss 0, det; jna`: swapped when 0 is over it.
            if 0.0 > det {
                simplex.c.swap(0, 1);
                simplex.p.swap(0, 1);
            }
            let c = simplex.c;
            let t0 = self.newface(store, c[0], c[1], c[2], true);
            let t1 = self.newface(store, c[1], c[0], c[3], true);
            let t2 = self.newface(store, c[2], c[1], c[3], true);
            let t3 = self.newface(store, c[0], c[2], c[3], true);
            if self.hull.count == 4 {
                let (t0, t1, t2, t3) = (t0.unwrap(), t1.unwrap(), t2.unwrap(), t3.unwrap());
                let mut best = self.findbest();
                let mut outer = self.faces[best];
                let mut pass = 0u32;
                let mut iterations = 0u32;
                bind(&mut self.faces, t0, 0, t1, 0);
                bind(&mut self.faces, t0, 1, t2, 0);
                bind(&mut self.faces, t0, 2, t3, 0);
                bind(&mut self.faces, t1, 1, t3, 2);
                bind(&mut self.faces, t1, 2, t2, 1);
                bind(&mut self.faces, t2, 2, t3, 1);
                self.status = EpaStatus::Valid;
                while iterations < EPA_MAX_ITERATIONS {
                    self.rounds += 1;
                    if self.nextsv >= EPA_MAX_VERTICES {
                        self.status = EpaStatus::OutOfVertices;
                        break;
                    }
                    let mut horizon = Horizon::default();
                    let w = 4 + self.nextsv;
                    self.nextsv += 1;
                    let mut valid = true;
                    pass += 1;
                    self.faces[best].pass = pass as u8;
                    store.sv[w] = getsupport(shape, self.faces[best].n);
                    let ww = store.sv[w].w;
                    // ORDER (asm 0xD501A..0xD503F): ((w.z*n.z + w.y*n.y) + w.x*n.x) - d.
                    let wdist = dot_zyx(ww, self.faces[best].n) - self.faces[best].d;
                    if !(wdist > EPS_1E4) {
                        self.status = EpaStatus::AccuraryReached;
                        break;
                    }
                    for j in 0..3 {
                        if !valid {
                            break;
                        }
                        let (fj, ej) = (self.faces[best].f[j], self.faces[best].e[j] as usize);
                        valid &= self.expand(store, pass, w, fj, ej, &mut horizon);
                    }
                    if !(valid && horizon.nf >= 3) {
                        self.status = EpaStatus::InvalidHull;
                        break;
                    }
                    bind(&mut self.faces, horizon.cf.unwrap(), 1, horizon.ff.unwrap(), 2);
                    list_remove(&mut self.hull, &mut self.faces, best);
                    list_append(&mut self.stock, &mut self.faces, best);
                    best = self.findbest();
                    // 0xD512A..0xD5145: the outer face follows the best only when its p is not under the outer's.
                    if self.faces[best].p >= outer.p {
                        outer = self.faces[best];
                    }
                    iterations += 1;
                }
                let projection = [outer.n[0] * outer.d, outer.n[1] * outer.d, outer.n[2] * outer.d];
                self.normal = outer.n;
                self.depth = outer.d;
                self.rank = 3;
                self.c = outer.c;
                let w = |i: usize| sub(store.sv[outer.c[i]].w, projection);
                // ORDER (asm 0xD51CE..0xD53A7): each length2 is (z*z + y*y) + x*x.
                let p0 = len2_zyx(cross(w(1), w(2))).sqrt();
                let p1 = len2_zyx(cross(w(2), w(0))).sqrt();
                let p2 = len2_zyx(cross(w(0), w(1))).sqrt();
                // ORDER (asm 0xD53CC..0xD53D7): 1 / ((p2 + p1) + p0), each weight times it.
                let r = ONE / ((p2 + p1) + p0);
                self.p = [r * p0, r * p1, r * p2];
                return self.status;
            }
        }
        // 0xD5408: the fallback.
        self.status = EpaStatus::FallBack;
        let n = neg(guess);
        let nl = len2_xyz(n).sqrt();
        self.normal = if nl > 0.0 {
            let r = ONE / nl;
            [n[0] * r, n[1] * r, n[2] * r]
        } else {
            [ONE, 0.0, 0.0]
        };
        self.depth = 0.0;
        self.rank = 1;
        self.c[0] = g.simplices[g.current].c[0];
        self.p[0] = ONE;
        self.status
    }
}

// --- Penetration, Distance, calcPenDepth ---------------------------------------------------------------------------

/// `sResults::eStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultStatus {
    Separated = 0,
    Penetrating = 1,
    GjkFailed = 2,
    EpaFailed = 3,
}

/// `sResults` and what was seen on the way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Results {
    pub status: ResultStatus,
    pub witnesses: [Vec3; 2],
    pub normal: Vec3,
    pub distance: f32,
    pub penetration_gjk: GjkStatus,
    pub epa: Option<EpaStatus>,
    pub distance_gjk: Option<GjkStatus>,
    pub epa_rounds: u32,
    pub gjk_rounds: u32,
}

/// `wtrs * v` with each row's sum grouped as given: `order[r]` lists the columns in the order they are added.
fn transform_point(t: &Transform, v: Vec3, order: [[usize; 3]; 3]) -> Vec3 {
    [0, 1, 2].map(|r| {
        let [i, j, k] = order[r];
        ((t.basis[r][i] * v[i] + t.basis[r][j] * v[j]) + t.basis[r][k] * v[k]) + t.origin[r]
    })
}

/// 0xD6810 `btGjkEpaSolver2::Penetration` (usemargins true) and, when it fails, 0xD6280 `Distance`, as 0xB7EE0
/// `calcPenDepth` calls them (guess `transA.origin - transB.origin`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenDepth {
    /// AL: Penetration's answer.
    pub valid: bool,
    /// What the solver wrote into the detector's `m_cachedSeparatingAxis` (`v`) and the witnesses; `None` when
    /// neither function had an answer (nothing written).
    pub written: Option<(Vec3, Vec3, Vec3)>,
    pub results: Results,
}

pub fn calc_pen_depth<SA: Support, SB: Support>(
    a: Convex<SA>,
    wtrs0: &Transform,
    b: Convex<SB>,
    wtrs1: &Transform,
) -> PenDepth {
    // 0xB7EF4..0xB7F36: guess = A's origin - B's.
    let guess = sub(wtrs0.origin, wtrs1.origin);
    let (a2, b2) = (Convex { shape: a.shape, margin: a.margin }, Convex { shape: b.shape, margin: b.margin });
    let pen = penetration(a, wtrs0, b, wtrs1, guess);
    if pen.status == ResultStatus::Penetrating {
        return PenDepth { valid: true, written: Some((pen.normal, pen.witnesses[0], pen.witnesses[1])), results: pen };
    }
    let dist = distance(a2, wtrs0, b2, wtrs1, guess, pen);
    let written = (dist.status == ResultStatus::Separated && dist.distance_gjk == Some(GjkStatus::Valid))
        .then_some((dist.normal, dist.witnesses[0], dist.witnesses[1]));
    PenDepth { valid: false, written, results: dist }
}

fn penetration<SA: Support, SB: Support>(
    a: Convex<SA>,
    wtrs0: &Transform,
    b: Convex<SB>,
    wtrs1: &Transform,
    guess: Vec3,
) -> Results {
    let shape = Minkowski::new(a, wtrs0, b, wtrs1, Ls::WithMargin);
    let mut store = Store { sv: vec![Sv::default(); 4 + EPA_MAX_VERTICES] };
    let mut results = Results {
        status: ResultStatus::Separated,
        witnesses: [[0.0; 3]; 2],
        normal: [0.0; 3],
        distance: 0.0,
        penetration_gjk: GjkStatus::Failed,
        epa: None,
        distance_gjk: None,
        epa_rounds: 0,
        gjk_rounds: 0,
    };
    let mut g = Gjk::evaluate(&mut store, &shape, neg(guess));
    results.penetration_gjk = g.status;
    results.gjk_rounds = g.rounds;
    match g.status {
        GjkStatus::Inside => {
            let mut epa = Epa::new();
            let status = epa.evaluate(&mut g, &mut store, &shape, neg(guess));
            results.epa = Some(status);
            results.epa_rounds = epa.rounds;
            if status == EpaStatus::Failed {
                results.status = ResultStatus::EpaFailed;
                return results;
            }
            // 0xD69B0..0xD6A2A: w0 += Support0(c[i].d) * p[i], per component p*s + w0.
            let mut w0 = [0.0f32; 3];
            for i in 0..epa.rank {
                let s = shape.support0(store.sv[epa.c[i]].d);
                let p = epa.p[i];
                w0 = [p * s[0] + w0[0], p * s[1] + w0[1], p * s[2] + w0[2]];
            }
            results.status = ResultStatus::Penetrating;
            // ORDER (asm 0xD6A35..0xD6ABA): rows (x, z, y), (z, y, x), (z, y, x).
            results.witnesses[0] = transform_point(wtrs0, w0, [[0, 2, 1], [2, 1, 0], [2, 1, 0]]);
            let n = epa.normal;
            let w1 = [w0[0] - n[0] * epa.depth, w0[1] - n[1] * epa.depth, w0[2] - n[2] * epa.depth];
            // ORDER (asm 0xD6B22..0xD6BA1): rows (z, y, x) each.
            results.witnesses[1] = transform_point(wtrs0, w1, [[2, 1, 0], [2, 1, 0], [2, 1, 0]]);
            results.normal = neg(n);
            results.distance = -epa.depth;
        }
        GjkStatus::Failed => results.status = ResultStatus::GjkFailed,
        GjkStatus::Valid => {}
    }
    results
}

fn distance<SA: Support, SB: Support>(
    a: Convex<SA>,
    wtrs0: &Transform,
    b: Convex<SB>,
    wtrs1: &Transform,
    guess: Vec3,
    before: Results,
) -> Results {
    let shape = Minkowski::new(a, wtrs0, b, wtrs1, Ls::WithoutMargin);
    let mut store = Store { sv: vec![Sv::default(); 4] };
    let mut results = before;
    let g = Gjk::evaluate(&mut store, &shape, guess);
    results.distance_gjk = Some(g.status);
    results.gjk_rounds += g.rounds;
    if g.status != GjkStatus::Valid {
        // 0xD67FC: Penetrating when GJK found the origin inside, else GJK_Failed.
        results.status = if g.status == GjkStatus::Inside { ResultStatus::Penetrating } else { ResultStatus::GjkFailed };
        return results;
    }
    let s = &g.simplices[g.current];
    let mut w0 = [0.0f32; 3];
    let mut w1 = [0.0f32; 3];
    for i in 0..s.rank {
        let p = s.p[i];
        let d = store.sv[s.c[i]].d;
        let s0 = shape.support0(d);
        w0 = [s0[0] * p + w0[0], s0[1] * p + w0[1], s0[2] * p + w0[2]];
        // 0xD63DF..0xD6588: Support1(-d) inline: m_toshape1 rows (x, y, z); m_toshape0 rows (y, z, x), (x, y, z),
        // (x, y, z), then the origin.
        let nd = neg(d);
        let m = &shape.to1;
        let v = [0, 1, 2].map(|r| (m[r][0] * nd[0] + m[r][1] * nd[1]) + m[r][2] * nd[2]);
        let s1 = ls(Ls::WithoutMargin, &shape.b, v);
        let t = &shape.to0;
        let s1 = [
            ((s1[1] * t.basis[0][1] + s1[2] * t.basis[0][2]) + s1[0] * t.basis[0][0]) + t.origin[0],
            ((s1[0] * t.basis[1][0] + s1[1] * t.basis[1][1]) + s1[2] * t.basis[1][2]) + t.origin[1],
            ((s1[0] * t.basis[2][0] + s1[1] * t.basis[2][1]) + s1[2] * t.basis[2][2]) + t.origin[2],
        ];
        w1 = [s1[0] * p + w1[0], s1[1] * p + w1[1], s1[2] * p + w1[2]];
    }
    // ORDER (asm 0xD65D4..0xD6658): rows (z, y, x) each; (0xD667F..0xD66FD): rows (x, z, y), (z, y, x), (z, y, x).
    results.witnesses[0] = transform_point(wtrs0, w0, [[2, 1, 0], [2, 1, 0], [2, 1, 0]]);
    results.witnesses[1] = transform_point(wtrs0, w1, [[0, 2, 1], [2, 1, 0], [2, 1, 0]]);
    let n = sub(w0, w1);
    let dist = len2_xyz(n).sqrt();
    results.distance = dist;
    // 0xD67AA..0xD67EF: n * (1 / (dist over 1e-4 ? dist : 1)).
    let r = ONE / if dist > EPS_1E4 { dist } else { ONE };
    results.normal = [n[0] * r, n[1] * r, n[2] * r];
    results.status = ResultStatus::Separated;
    results
}

// --- the detector's tail ---------------------------------------------------------------------------------------------

/// `m_lastUsedMethod` as the penetration branch leaves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenMethod {
    /// 3: the solver's answer is taken.
    Taken = 3,
    /// 5: the solver had no answer of its own, Distance's is not under GJK's.
    DistanceNotCloser = 5,
    /// 6: Distance's answer is taken.
    DistanceTaken = 6,
    /// 8: the solver's answer is not deeper than GJK's.
    NotDeeper = 8,
    /// 9: the solver's normal is too short.
    ShortNormal = 9,
}

/// What the detector does after the solver (0xD768E..0xD7A91, 0xD77EE..0xD78A8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailAnswer {
    /// `m_lastUsedMethod` when the branch set it (`None`: GJK's own value stays).
    pub method: Option<PenMethod>,
    /// The detector's `addContactPoint(normal, point + offset, distance)`, when made.
    pub contact: Option<(Vec3, Vec3, f32)>,
    /// `m_cachedSeparatingAxis` after the call (16 bytes, the fourth 0).
    pub cached_axis: Vec3,
    /// `m_cachedSeparatingDistance` when the contact is made.
    pub cached_distance: Option<f32>,
    pub pen: PenDepth,
}

/// The detector's penetration branch from 0xD762B: `gjk` its answer (`isValid`, distance, point B, normal),
/// `margin_sum` `marginA + marginB`, `margin_b`, `offset` the position offset, the largest distance squared.
#[allow(clippy::too_many_arguments)]
pub fn detector_tail<SA: Support, SB: Support>(
    a: Convex<SA>,
    local_a: &Transform,
    b: Convex<SB>,
    local_b: &Transform,
    gjk: Option<GjkClosest>,
    margin_sum: f32,
    margin_b: f32,
    offset: Vec3,
    maximum_distance_squared: f32,
) -> TailAnswer {
    let mut valid = gjk.is_some();
    let (mut distance, mut point_b, mut normal_b) = match gjk {
        Some(g) => (g.distance, g.point_on_b, g.normal_on_b),
        None => (0.0, [0.0; 3], [0.0; 3]),
    };
    // 0xD762B: m_cachedSeparatingAxis zeroed; calcPenDepth writes it when it answers.
    let pen = calc_pen_depth(a, local_a, b, local_b);
    let axis = pen.written.map_or([0.0; 3], |w| w.0);
    let mut method = None;
    if pen.valid {
        let (_, wa, wb) = pen.written.expect("a valid answer is written");
        let mut n = sub(wb, wa);
        // ORDER (asm 0xD76BB..0xD76E5): (z*z + y*y) + x*x.
        let mut len2 = len2_zyx(n);
        // 0xD76E9: `comiss 2^-46, len2; jc`: the axis when 2^-46 is not under the length.
        if !(SIMD_EPSILON_SQUARED < len2) {
            n = axis;
            // ORDER (asm 0xD7719..0xD7733): (x*x + y*y) + z*z.
            len2 = len2_xyz(axis);
        }
        if len2 > SIMD_EPSILON_SQUARED {
            let r = ONE / len2.sqrt();
            // ORDER (asm 0xD775B..0xD7794): (z*z + y*y) + x*x of A - B.
            let d2 = -len2_zyx(sub(wa, wb)).sqrt();
            let n = [n[0] * r, n[1] * r, n[2] * r];
            // 0xD77C3..0xD77E1: taken when GJK had no answer, or its distance is over this one.
            if !valid || distance > d2 {
                distance = d2;
                point_b = wb;
                normal_b = n;
                valid = true;
                method = Some(PenMethod::Taken);
            } else {
                method = Some(PenMethod::NotDeeper);
            }
        } else {
            method = Some(PenMethod::ShortNormal);
        }
    } else if len2_xyz(axis) > 0.0 {
        let (_, wa, wb) = pen.written.expect("a written axis");
        // 0xD7939..0xD7985: |A - B| ((z*z + y*y) + x*x) less the margins (x87).
        let d2 = len2_zyx(sub(wa, wb)).sqrt() - margin_sum;
        if !valid || distance > d2 {
            distance = d2;
            // 0xD79E5..0xD7A1B: B + axis * marginB, (marginB*axis + B).
            point_b = [margin_b * axis[0] + wb[0], axis[1] * margin_b + wb[1], axis[2] * margin_b + wb[2]];
            // ORDER (asm 0xD7A27..0xD7A59): (x*x + z*z) + y*y.
            let r = ONE / ((axis[0] * axis[0] + axis[2] * axis[2]) + axis[1] * axis[1]).sqrt();
            normal_b = [axis[0] * r, axis[1] * r, axis[2] * r];
            valid = true;
            method = Some(PenMethod::DistanceTaken);
        } else {
            method = Some(PenMethod::DistanceNotCloser);
        }
    }
    let mut answer = TailAnswer { method, contact: None, cached_axis: axis, cached_distance: None, pen };
    // 0xD77EE..0xD78A8: a valid answer under the largest distance is reported.
    if valid && (0.0 > distance || distance * distance < maximum_distance_squared) {
        answer.cached_distance = Some(distance);
        answer.cached_axis = normal_b;
        let point = [point_b[0] + offset[0], point_b[1] + offset[1], point_b[2] + offset[2]];
        answer.contact = Some((normal_b, point, distance));
    }
    answer
}
