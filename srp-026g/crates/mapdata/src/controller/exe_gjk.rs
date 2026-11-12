//! The closest points of a cast shape and a world shape (a mesh's triangle, a box, a sphere or a cylinder), as
//! Survarium 0.26g's executable computes them: a port of its statically linked Bullet
//! `btGjkPairDetector::getClosestPointsNonVirtual` (0xD6CD0) with its Voronoi simplex solver and its support
//! function, bit for bit.
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
//! The executable does this arithmetic in scalar SSE `f32` (`addss`, `subss`, `mulss`, `divss`), so Rust `f32`
//! operators in the same order give the same bits. Float addition is not associative: every sum of three or
//! more terms here is grouped as the executable groups it, and the places where that grouping differs from
//! the order of Bullet's source are marked `ORDER (asm)`. Square roots go through the x87 (`call 0x38960`,
//! `fsqrt`) and reciprocals through `fld1; fdivrp`, with the game's x87 control word 0x007F (24-bit
//! precision): each such result is rounded to `f32`, which `f32::sqrt` and `1.0 / x` reproduce. Two tests are
//! done in double (`cvtps2pd; comisd`) and are written so. `mul_add` is never used. The game's MXCSR (0x9FC0)
//! flushes denormals to zero; this port does not emulate that (no recorded call needed it).

/// A vector of three `f32` (x, y, z). The executable's vectors carry a fourth component, which is zero on
/// every path ported here.
pub type Vec3 = [f32; 3];

// Constants of the executable (read from its data; bit patterns, so nothing depends on literal parsing).

/// 0x927A44: 0.5, the weight of the position offset.
const HALF: f32 = 0.5;
/// 0xA70884: 1.0.
const ONE: f32 = 1.0;
/// 0xA2D990: `BT_LARGE_FLOAT` 1e18, the starting squared distance (and the simplex solver's first `lastW`).
const BT_LARGE_FLOAT: f32 = f32::from_bits(0x5D5E_0B6B);
/// 0xA1EE5C: `REL_ERROR2` 1e-6.
const REL_ERROR2: f32 = f32::from_bits(0x3586_37BD);
/// 0xA2DA0C: `SIMD_EPSILON` 2^-23.
const SIMD_EPSILON: f32 = f32::from_bits(0x3400_0000);
/// 0xA2DA9C: `SIMD_EPSILON * SIMD_EPSILON` 2^-46.
const SIMD_EPSILON_SQUARED: f32 = f32::from_bits(0x2880_0000);
/// 0xA2DB70: the double 0.0001 the separating axis' squared length is compared with (degenerate 5).
const SHORT_AXIS: f64 = f64::from_bits(0x3F1A_36E2_EB1C_432D);
/// 0xA2DB68: the double 0.01 that `distance + margins` is compared with when degeneracies are caught.
const CATCH_DEGENERACY_DISTANCE: f64 = f64::from_bits(0x3F84_7AE1_47AE_147B);
/// `gGjkMaxIter`: the immediate 0x3E8 at 0xD73F3.
const GJK_MAX_ITER: u32 = 1000;
/// The simplex solver's `m_equalVertexThreshold` (+140h): `VORONOI_DEFAULT_EQUAL_VERTEX_THRESHOLD` 1e-4 (0xA2D840),
/// stored by the solver's maker in `btTriangleConvexcastCallback::processTriangle` (0xB8704..0xB8711).
const EQUAL_VERTEX_THRESHOLD: f32 = f32::from_bits(0x38D1_B717);
/// 0xA2DA10: 1e-8, `pointOutsideOfPlane`'s bound on `signd * signd` (Bullet's `1e-4 * 1e-4`, folded).
const PLANE_DEGENERACY: f32 = f32::from_bits(0x322B_CC77);
/// 0xA2D7B8: `FLT_MAX`, the tetrahedron search's starting best squared distance.
const FLT_MAX: f32 = f32::from_bits(0x7F7F_FFFF);
/// 0xA2D840: 1e-4, below which the capsule's support direction counts as zero.
const CAPSULE_SHORT_DIRECTION: f32 = f32::from_bits(0x38D1_B717);
/// 0xA2D7D8: -1e18 (`-BT_LARGE_FLOAT`), the capsule support's starting best dot product.
const CAPSULE_FIRST_MAX_DOT: f32 = f32::from_bits(0xDD5E_0B6B);

/// The origin, as the simplex solver's closest-point routines get it (`p`, all zero).
const ORIGIN: Vec3 = [0.0, 0.0, 0.0];

/// A coordinate axis (a shape's up axis).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// The axis of the executable's index (0, 1, 2); anything else is not an axis.
    pub fn from_index(index: i32) -> Option<Axis> {
        match index {
            0 => Some(Axis::X),
            1 => Some(Axis::Y),
            2 => Some(Axis::Z),
            _ => None,
        }
    }

    /// The component index of the axis.
    pub fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }
}

/// The cast shape (shape A), holding exactly what 0x8D680
/// `btConvexShape::localGetSupportVertexWithoutMarginNonVirtual` reads for its type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CastShape {
    /// Type 8 (`SPHERE_SHAPE_PROXYTYPE`): the support without margin is the centre; the sphere's radius is all
    /// margin (the detector's `margin_a`). Nothing of the shape is read.
    Sphere,
    /// Type 10 (`CAPSULE_SHAPE_PROXYTYPE`), case 0x8D9BB: the implicit dimension on axis `(up + 2) % 3` (the
    /// radius) and on `up` (the half height), the local scaling (+10h), the collision margin (+30h) and the up
    /// axis (+40h).
    Capsule {
        radius: f32,
        half_height: f32,
        scaling: Vec3,
        margin: f32,
        up: Axis,
    },
    /// Type 13 (`CYLINDER_SHAPE_PROXYTYPE`), case 0x8D816: the implicit dimensions (+20h, the half extents
    /// without margin) on the radial axis of `up` (the radius) and on `up` (the half height), and the up axis
    /// (+40h). Neither the margin nor the scaling is read.
    Cylinder { radius: f32, half_height: f32, up: Axis },
}

impl CastShape {
    /// A capsule from its fields as the executable keeps them: the implicit dimensions (+20h), the local
    /// scaling (+10h), the collision margin (+30h) and the up axis (+40h). The radius is the dimension on
    /// `(up + 2) % 3` (0x8D9CF..0x8D9D8: `lea eax,[edi+2]; cdq; idiv 3`), the half height the one on `up`.
    pub fn capsule(implicit_dimensions: Vec3, scaling: Vec3, margin: f32, up: Axis) -> CastShape {
        CastShape::Capsule {
            radius: implicit_dimensions[(up.index() + 2) % 3],
            half_height: implicit_dimensions[up.index()],
            scaling,
            margin,
            up,
        }
    }

    /// A cylinder from its implicit dimensions (+20h) and up axis (+40h): the radius is the half extent on the
    /// first radial axis (`XX` of Bullet's `CylinderLocalSupport{X,Y,Z}`: y for an x-up cylinder, x otherwise),
    /// the half height the one on `up`.
    pub fn cylinder(implicit_dimensions: Vec3, up: Axis) -> CastShape {
        let axes = CylinderAxes::of(up);
        CastShape::Cylinder {
            radius: implicit_dimensions[axes.radial_1],
            half_height: implicit_dimensions[up.index()],
            up,
        }
    }
}

/// A triangle (shape B, type 1 `TRIANGLE_SHAPE_PROXYTYPE`): its corners (+50h, +60h, +70h of the shape).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle(pub [Vec3; 3]);

/// A collision object's convex shape as `objectQuerySingle` hands it to the cast (shape B): a triangle of a
/// mesh (`btTriangleShape` made by `processTriangle`), or a convex primitive (the object's own shape, or a
/// compound's child with the child's transform composed into transform B), holding exactly what 0x8D680
/// `btConvexShape::localGetSupportVertexWithoutMarginNonVirtual` reads for its type. Each primitive's margin is
/// the detector's `margin_b` and is not read here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldShape {
    /// Type 1 (`TRIANGLE_SHAPE_PROXYTYPE`), case 0x8D72C: the corners.
    Triangle(Triangle),
    /// Type 0 (`BOX_SHAPE_PROXYTYPE`), case 0x8D6CA: the implicit dimensions (+20h, +24h, +28h: the half extents
    /// without the margin). Neither the scaling nor the margin is read.
    Box { half_extents: Vec3 },
    /// Type 8 (`SPHERE_SHAPE_PROXYTYPE`), case 0x8D6A8: the support without margin is the centre; the sphere's
    /// radius is all margin. Nothing of the shape is read.
    Sphere,
    /// Type 13 (`CYLINDER_SHAPE_PROXYTYPE`), case 0x8D816: as [`CastShape::Cylinder`] (the implicit dimensions on
    /// the radial axis and on `up`, the up axis +40h; neither the margin nor the scaling is read).
    Cylinder { radius: f32, half_height: f32, up: Axis },
}

impl WorldShape {
    /// A cylinder from its implicit dimensions (+20h) and up axis (+40h), as [`CastShape::cylinder`].
    pub fn cylinder(implicit_dimensions: Vec3, up: Axis) -> WorldShape {
        let axes = CylinderAxes::of(up);
        WorldShape::Cylinder {
            radius: implicit_dimensions[axes.radial_1],
            half_height: implicit_dimensions[up.index()],
            up,
        }
    }
}

impl From<Triangle> for WorldShape {
    fn from(triangle: Triangle) -> WorldShape {
        WorldShape::Triangle(triangle)
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Triangle {}
    impl Sealed for super::WorldShape {}
}

/// A shape B [`get_closest_points`] takes: a [`Triangle`] or a [`WorldShape`]. Sealed: its support is the
/// executable's 0x8D680 for the shape's type, nothing else.
pub trait ShapeB: sealed::Sealed {
    /// 0x8D680 `localGetSupportVertexWithoutMarginNonVirtual` for this shape: its point without margin farthest
    /// along `dir`, in its own frame.
    fn support(&self, dir: Vec3) -> Vec3;
}

impl ShapeB for Triangle {
    fn support(&self, dir: Vec3) -> Vec3 {
        support_triangle(self, dir)
    }
}

impl ShapeB for WorldShape {
    fn support(&self, dir: Vec3) -> Vec3 {
        support_world_shape(self, dir)
    }
}

/// A rigid transform: `basis` rows and the origin, as `btTransform` keeps them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub basis: [[f32; 3]; 3],
    pub origin: Vec3,
}

impl Transform {
    /// `btTransform::operator()(p)`, `basis * p + origin`, in the executable's grouping.
    fn apply(&self, p: Vec3) -> Vec3 {
        let m = &self.basis;
        let o = &self.origin;
        // ORDER (asm 0xD70E0..0xD71C6 for A, 0xD71C0..0xD725B for B, the same grouping): each row is
        // ((m[r][1]*y + m[r][2]*z) + m[r][0]*x) + origin, where the source's dot product is
        // (m[r][0]*x + m[r][1]*y) + m[r][2]*z.
        [
            ((m[0][1] * p[1] + m[0][2] * p[2]) + m[0][0] * p[0]) + o[0],
            ((m[1][1] * p[1] + m[1][2] * p[2]) + m[1][0] * p[0]) + o[1],
            ((m[2][1] * p[1] + m[2][2] * p[2]) + m[2][0] * p[0]) + o[2],
        ]
    }
}

/// How the detector applies the shapes' margins (`m_ignoreMargin`, +40h).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Margins {
    /// The margins are added: `m_marginA` (+38h) and `m_marginB` (+3Ch).
    Applied { a: f32, b: f32 },
    /// `m_ignoreMargin` set: both margins count as zero.
    Ignored,
}

impl Margins {
    /// (margin A, margin B) as the function uses them (0xD6EF4..0xD6F10).
    fn values(self) -> (f32, f32) {
        match self {
            Margins::Applied { a, b } => (a, b),
            Margins::Ignored => (0.0, 0.0),
        }
    }
}

/// Whether the detector has a penetration-depth solver (`m_penetrationDepthSolver`, +20h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Penetration {
    /// No solver: an invalid GJK answer is no contact.
    NoSolver,
    /// A solver, which an invalid GJK answer goes to; with `catch_degeneracies` (`m_catchDegeneracies`, +54h)
    /// a degenerate one closer than 0.01 does too.
    Solver { catch_degeneracies: bool },
}

/// The detector's settings for one call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detector {
    pub margins: Margins,
    pub penetration: Penetration,
}

/// The call's input (`btDiscreteCollisionDetectorInterface::ClosestPointInput`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestPointInput {
    /// `m_transformA` (+0).
    pub transform_a: Transform,
    /// `m_transformB` (+40h).
    pub transform_b: Transform,
    /// `m_maximumDistanceSquared` (+80h).
    pub maximum_distance_squared: f32,
}

/// `m_degenerateSimplex` (+50h): why the GJK loop stopped, or 5 when the separating axis came out shorter
/// than 0.01 (which overrides the reason). The executable's integer is [`DegenerateSimplex::code`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DegenerateSimplex {
    /// 0: the loop stopped without a degeneracy (the iteration limit), or never had one.
    None,
    /// 1: the new support point is already in the simplex.
    VertexInSimplex,
    /// 2: no progress, `squaredDistance - delta <= 0`.
    NoProgressNonPositive,
    /// 3: the simplex solver found no valid closest point.
    ClosestInvalid,
    /// 5: after the loop, the separating axis' squared length is under 0.0001.
    ShortAxis,
    /// 6: the new separating axis' squared length is under `REL_ERROR2`.
    AxisVanished,
    /// 10: the shapes are separated by more than the maximum distance along the axis.
    Separated,
    /// 11: no progress, `0 < squaredDistance - delta <= squaredDistance * REL_ERROR2`.
    NoProgress,
    /// 12: the squared distance did not shrink by more than `SIMD_EPSILON` of itself.
    DistanceStalled,
    /// 13: the simplex is a full tetrahedron.
    FullSimplex,
}

impl DegenerateSimplex {
    /// The executable's integer.
    pub fn code(self) -> i32 {
        match self {
            DegenerateSimplex::None => 0,
            DegenerateSimplex::VertexInSimplex => 1,
            DegenerateSimplex::NoProgressNonPositive => 2,
            DegenerateSimplex::ClosestInvalid => 3,
            DegenerateSimplex::ShortAxis => 5,
            DegenerateSimplex::AxisVanished => 6,
            DegenerateSimplex::Separated => 10,
            DegenerateSimplex::NoProgress => 11,
            DegenerateSimplex::DistanceStalled => 12,
            DegenerateSimplex::FullSimplex => 13,
        }
    }
}

/// `m_lastUsedMethod` (+48h) as the GJK part leaves it. The executable's integer is [`LastUsedMethod::code`];
/// the penetration branch (not ported here) sets 3, 5, 8 or 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LastUsedMethod {
    /// -1: the GJK result was never taken (no check of the simplex).
    Unset,
    /// 1: the GJK result is valid.
    Gjk,
    /// 2: the separating axis is too short (squared length at most 2^-46) for a GJK result.
    GjkAxisTooShort,
}

impl LastUsedMethod {
    /// The executable's integer.
    pub fn code(self) -> i32 {
        match self {
            LastUsedMethod::Unset => -1,
            LastUsedMethod::Gjk => 1,
            LastUsedMethod::GjkAxisTooShort => 2,
        }
    }
}

/// The detector's fields the call leaves behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetectorState {
    /// `m_lastUsedMethod` (+48h).
    pub last_used_method: LastUsedMethod,
    /// `m_curIter` (+4Ch).
    pub cur_iter: u32,
    /// `m_degenerateSimplex` (+50h).
    pub degenerate_simplex: DegenerateSimplex,
    /// `m_cachedSeparatingAxis` (+10h).
    pub cached_separating_axis: Vec3,
    /// `m_cachedSeparatingDistance` (+44h).
    pub cached_separating_distance: f32,
}

/// A valid GJK answer in the call's shifted frame (both origins minus the position offset).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GjkClosest {
    /// `normalInB`: the separating axis normalised.
    pub normal_on_b: Vec3,
    /// `pointOnB`, before the position offset is added back.
    pub point_on_b: Vec3,
    /// The distance between the shapes with their margins.
    pub distance: f32,
}

/// Where the executable enters its penetration branch (0xD762B, the indirect `calcPenDepth` call at
/// 0xD7684): everything a port of that branch continues from.
#[derive(Clone, Debug, PartialEq)]
pub struct PenetrationStart {
    /// The simplex solver as GJK left it (`calcPenDepth`'s first argument).
    pub solver: SimplexSolver,
    /// The GJK answer when it was valid (`isValid`); `None` is the executable's `isValid == false` with
    /// distance 0.
    pub gjk: Option<GjkClosest>,
    /// Transform A with its origin minus the position offset.
    pub local_a: Transform,
    /// Transform B with its origin minus the position offset.
    pub local_b: Transform,
    /// `(origin A + origin B) * 0.5`, added back to a reported point.
    pub position_offset: Vec3,
    /// `marginA + marginB` (after `m_ignoreMargin`).
    pub margin_sum: f32,
}

/// What the call does with the result.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The executable calls the result's `addContactPoint(normal_on_b, point_on_b, distance)` (vtable +0Ch).
    Contact { normal_on_b: Vec3, point_on_b: Vec3, distance: f32 },
    /// No call of the result: GJK found no valid answer and there is no penetration solver, or the answer is
    /// farther than the maximum distance.
    NoContact,
    /// The executable would call the penetration-depth solver, which is not ported: the state to go on from.
    /// The branch starts by zeroing `m_cachedSeparatingAxis` (0xD762B), which `DetectorState` does not yet show.
    NeedsPenetration(Box<PenetrationStart>),
}

/// One call's answer and the detector's state after it.
#[derive(Clone, Debug, PartialEq)]
pub struct ClosestPoints {
    pub outcome: Outcome,
    pub state: DetectorState,
}

/// Which simplex vertices a sub-simplex closest point uses (`btUsageBitfield`: bit 0 A .. bit 3 D).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UsedVertices(u8);

impl UsedVertices {
    const A: u8 = 1;
    const B: u8 = 2;
    const C: u8 = 4;
    const D: u8 = 8;
    const ALL: UsedVertices = UsedVertices(0xF);

    fn uses(self, bit: u8) -> bool {
        self.0 & bit != 0
    }

    fn with(self, bit: u8) -> UsedVertices {
        UsedVertices(self.0 | bit)
    }
}

/// `btSubSimplexClosestResult` (the solver's `m_cachedBC` at +150h).
#[derive(Clone, Copy, Debug, PartialEq)]
struct SubSimplexClosest {
    /// `m_closestPointOnSimplex` (+0).
    point: Vec3,
    /// `m_usedVertices` (+10h).
    used: UsedVertices,
    /// `m_barycentricCoords` (+14h).
    barycentric: [f32; 4],
    /// `m_degenerate` (+24h): the tetrahedron was flat.
    degenerate: bool,
}

impl SubSimplexClosest {
    const EMPTY: SubSimplexClosest = SubSimplexClosest {
        point: ORIGIN,
        used: UsedVertices(0),
        barycentric: [0.0; 4],
        degenerate: false,
    };

    /// `reset` (inlined at 0xD129C): not degenerate, no coordinates, no vertex used; the point is kept.
    fn reset(&mut self) {
        self.degenerate = false;
        self.barycentric = [0.0; 4];
        self.used = UsedVertices(0);
    }

    /// `isValid`: every barycentric coordinate at least 0 (`comiss c, 0; jc` invalid: NaN is invalid).
    fn is_valid(&self) -> bool {
        self.barycentric.iter().all(|&c| c >= 0.0)
    }
}

/// A closest point on a triangle: the point, which corners it uses (bits A, B, C) and its barycentric
/// coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TriangleClosest {
    point: Vec3,
    used: UsedVertices,
    barycentric: [f32; 3],
}

/// One simplex vertex: `w = p - q` and its support points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SimplexVertex {
    /// `m_simplexVectorW[i]` (+10h + 10h*i).
    w: Vec3,
    /// `m_simplexPointsP[i]` (+60h + 10h*i).
    p: Vec3,
    /// `m_simplexPointsQ[i]` (+B0h + 10h*i).
    q: Vec3,
}

/// Whether the cached closest point is up to date (`m_needsUpdate` +180h with `m_cachedValidClosest` +144h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cached {
    /// A vertex was added (or the solver reset) since the last update.
    Stale,
    /// Up to date; whether the closest point is valid.
    Current { valid: bool },
}

/// Bullet's `btVoronoiSimplexSolver` as the executable lays it out (+0 `m_numVertices`, vertices, +100h
/// `m_cachedP1`, +110h `m_cachedP2`, +120h `m_cachedV`, +130h `m_lastW`, +150h `m_cachedBC`).
#[derive(Clone, Debug, PartialEq)]
pub struct SimplexSolver {
    vertices: [SimplexVertex; 5],
    len: usize,
    cached_p1: Vec3,
    cached_p2: Vec3,
    cached_v: Vec3,
    last_w: Vec3,
    cached_bc: SubSimplexClosest,
    cached: Cached,
}

impl Default for SimplexSolver {
    fn default() -> Self {
        SimplexSolver::new()
    }
}

impl SimplexSolver {
    /// A reset solver (0xD1170 `reset`). The cached points start at zero here; the executable leaves them as
    /// they were, and no path reads them before the first update writes them.
    pub fn new() -> SimplexSolver {
        SimplexSolver {
            vertices: [SimplexVertex::default(); 5],
            len: 0,
            cached_p1: ORIGIN,
            cached_p2: ORIGIN,
            cached_v: ORIGIN,
            last_w: [BT_LARGE_FLOAT; 3],
            cached_bc: SubSimplexClosest::EMPTY,
            cached: Cached::Stale,
        }
    }

    /// 0xD1170 `reset`: no vertices, `lastW` (1e18, 1e18, 1e18), the closest point stale.
    pub fn reset(&mut self) {
        self.len = 0;
        self.cached = Cached::Stale;
        self.last_w = [BT_LARGE_FLOAT; 3];
        self.cached_bc.reset();
    }

    /// The number of vertices (`m_numVertices`, +0).
    pub fn num_vertices(&self) -> usize {
        self.len
    }

    /// `fullSimplex`: four vertices.
    pub fn full_simplex(&self) -> bool {
        self.len == 4
    }

    /// The cached closest vector `v` (+120h).
    pub fn cached_v(&self) -> Vec3 {
        self.cached_v
    }

    /// The cached closest point on A (+100h).
    pub fn cached_p1(&self) -> Vec3 {
        self.cached_p1
    }

    /// The cached closest point on B (+110h).
    pub fn cached_p2(&self) -> Vec3 {
        self.cached_p2
    }

    /// The simplex vertices: (w, p, q) each.
    pub fn vertices(&self) -> impl Iterator<Item = (Vec3, Vec3, Vec3)> + '_ {
        self.vertices[..self.len].iter().map(|v| (v.w, v.p, v.q))
    }

    /// 0xD1200 `addVertex` (EAX the solver, EDX `&w`, EDI `&p`, ESI `&q`): `lastW = w`, the closest point
    /// stale, the vertex appended.
    pub fn add_vertex(&mut self, w: Vec3, p: Vec3, q: Vec3) {
        self.last_w = w;
        self.cached = Cached::Stale;
        self.vertices[self.len] = SimplexVertex { w, p, q };
        self.len += 1;
    }

    /// 0xD10A0 `removeVertex` (EAX the solver, ESI the index): the last vertex moves into the slot.
    fn remove_vertex(&mut self, index: usize) {
        self.len -= 1;
        self.vertices[index] = self.vertices[self.len];
    }

    /// 0xD1120 `reduceVertices` (EAX the solver, EDI `&usedVertices`): drops the unused vertices, D first.
    fn reduce_vertices(&mut self, used: UsedVertices) {
        if self.len >= 4 && !used.uses(UsedVertices::D) {
            self.remove_vertex(3);
        }
        if self.len >= 3 && !used.uses(UsedVertices::C) {
            self.remove_vertex(2);
        }
        if self.len >= 2 && !used.uses(UsedVertices::B) {
            self.remove_vertex(1);
        }
        if self.len >= 1 && !used.uses(UsedVertices::A) {
            self.remove_vertex(0);
        }
    }

    /// 0xD1C70 `inSimplex` (EDX the solver, ESI `&w`): whether `w` is within the equal-vertex threshold of a
    /// vertex, or is `lastW`.
    pub fn in_simplex(&self, w: Vec3) -> bool {
        let mut found = false;
        for v in &self.vertices[..self.len] {
            let dx = w[0] - v.w[0];
            let dy = w[1] - v.w[1];
            let dz = w[2] - v.w[2];
            // ORDER (asm 0xD1CC0..0xD1CF0): (dx*dx + dz*dz) + dy*dy; the source's length2 is x, y, z.
            let distance2 = (dx * dx + dz * dz) + dy * dy;
            if distance2 <= EQUAL_VERTEX_THRESHOLD {
                found = true;
            }
        }
        // `w == m_lastW` compares all four components (0xD1E27..0xD1E6E); the fourth is zero in both.
        w == self.last_w || found
    }

    /// `closest(v)` as the GJK loop uses it: 0xD1280, then `m_cachedV` (+120h) read directly.
    fn closest(&mut self) -> (bool, Vec3) {
        let valid = self.update_closest_vector_and_points();
        (valid, self.cached_v)
    }

    /// `compute_points` (0xD1E80) as the GJK uses it: 0xD1280, then `m_cachedP2` (only point B is kept).
    fn compute_point_b(&mut self) -> Vec3 {
        self.update_closest_vector_and_points();
        self.cached_p2
    }

    /// 0xD1280 `updateClosestVectorAndPoints` (stack argument the solver, AL the result): the closest point
    /// of the simplex to the origin, `m_cachedP1`, `m_cachedP2`, `m_cachedV = P1 - P2`, the unused vertices
    /// dropped; whether the closest point is valid. A current answer is returned as it is.
    pub fn update_closest_vector_and_points(&mut self) -> bool {
        if let Cached::Current { valid } = self.cached {
            return valid;
        }
        self.cached_bc.reset();
        let valid = match self.len {
            // Jump table at 0xD1C58: 0 -> 0xD1C40, 1 -> 0xD12DF, 2 -> 0xD13F3, 3 -> 0xD1673, 4 -> 0xD1910.
            1 => self.closest_point(),
            2 => self.closest_on_segment(),
            3 => self.closest_on_triangle(),
            4 => self.closest_on_tetrahedron(),
            _ => false,
        };
        self.cached = Cached::Current { valid };
        valid
    }

    /// Case 1 (0xD12DF): the single vertex.
    fn closest_point(&mut self) -> bool {
        let v = self.vertices[0];
        self.cached_p1 = v.p;
        self.cached_p2 = v.q;
        self.cached_v = sub(self.cached_p1, self.cached_p2);
        self.cached_bc.reset();
        self.cached_bc.barycentric = [ONE, 0.0, 0.0, 0.0];
        self.cached_bc.is_valid()
    }

    /// Case 2 (0xD13F3): the closest point of a segment.
    fn closest_on_segment(&mut self) -> bool {
        let from = self.vertices[0].w;
        let to = self.vertices[1].w;
        // Bullet's `diff = p - from` with p the origin is compiled as a sign flip (`xorps` with the sign mask
        // 0xA2D7C0), which differs from 0 - x for x = +0.
        let diff = [-from[0], -from[1], -from[2]];
        let v = sub(to, from);
        // ORDER (asm 0xD1437..0xD1450): (v.x*d.x + v.z*d.z) + v.y*d.y; the source's dot is x, y, z.
        let mut t = (v[0] * diff[0] + v[2] * diff[2]) + v[1] * diff[1];
        let mut used = self.cached_bc.used;
        if t > 0.0 {
            // ORDER (asm 0xD1467..0xD147D): (v.x*v.x + v.z*v.z) + v.y*v.y.
            let dot_vv = (v[0] * v[0] + v[2] * v[2]) + v[1] * v[1];
            if t < dot_vv {
                t /= dot_vv;
                used = used.with(UsedVertices::A).with(UsedVertices::B);
            } else {
                t = ONE;
                used = used.with(UsedVertices::B);
            }
        } else {
            t = 0.0;
            used = used.with(UsedVertices::A);
        }
        self.cached_bc.used = used;
        self.cached_bc.barycentric = [ONE - t, t, 0.0, 0.0];
        let a = self.vertices[0];
        let b = self.vertices[1];
        // `P0 + t * (P1 - P0)`: (P1 - P0) * t + P0 per component (0xD14B8..0xD1501).
        self.cached_p1 = lerp(a.p, b.p, t);
        self.cached_p2 = lerp(a.q, b.q, t);
        self.cached_v = sub(self.cached_p1, self.cached_p2);
        self.reduce_vertices(self.cached_bc.used);
        self.cached_bc.is_valid()
    }

    /// Case 3 (0xD1673): the closest point of a triangle.
    fn closest_on_triangle(&mut self) -> bool {
        let [a, b, c] = [self.vertices[0], self.vertices[1], self.vertices[2]];
        let closest = closest_pt_point_triangle(ORIGIN, a.w, b.w, c.w);
        self.cached_bc.point = closest.point;
        self.cached_bc.used = closest.used;
        let [b0, b1, b2] = closest.barycentric;
        self.cached_bc.barycentric = [b0, b1, b2, 0.0];
        // (P0*b0 + P1*b1) + P2*b2 per component (0xD169E..0xD1832), the source's order.
        self.cached_p1 = weighted3(a.p, b.p, c.p, b0, b1, b2);
        self.cached_p2 = weighted3(a.q, b.q, c.q, b0, b1, b2);
        self.cached_v = sub(self.cached_p1, self.cached_p2);
        self.reduce_vertices(self.cached_bc.used);
        self.cached_bc.is_valid()
    }

    /// Case 4 (0xD1910): the closest point of a tetrahedron, or the origin inside it.
    fn closest_on_tetrahedron(&mut self) -> bool {
        let [a, b, c, d] = [self.vertices[0], self.vertices[1], self.vertices[2], self.vertices[3]];
        match closest_pt_point_tetrahedron(ORIGIN, a.w, b.w, c.w, d.w) {
            Tetrahedron::Outside(closest) => {
                self.cached_bc.point = closest.point;
                self.cached_bc.used = closest.used;
                self.cached_bc.barycentric = closest.barycentric;
                // ((P0*b0 + P1*b1) + P2*b2) + P3*b3 per component (0xD194A..0xD1B95), the source's order.
                self.cached_p1 = weighted4(a.p, b.p, c.p, d.p, closest.barycentric);
                self.cached_p2 = weighted4(a.q, b.q, c.q, d.q, closest.barycentric);
                self.cached_v = sub(self.cached_p1, self.cached_p2);
                self.reduce_vertices(self.cached_bc.used);
                self.cached_bc.is_valid()
            }
            Tetrahedron::Inside => {
                // 0xD1C07: valid, v zero; P1 and P2 keep their last values.
                self.cached_bc.point = ORIGIN;
                self.cached_bc.used = UsedVertices::ALL;
                self.cached_v = ORIGIN;
                true
            }
            Tetrahedron::Degenerate => {
                // 0xD1C40 via 0xD2B5F: m_degenerate set, invalid.
                self.cached_bc.point = ORIGIN;
                self.cached_bc.used = UsedVertices::ALL;
                self.cached_bc.degenerate = true;
                false
            }
        }
    }
}

/// `a - b` per component.
fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a + t * (b - a)`: (b - a) * t + a per component.
fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    [(b[0] - a[0]) * t + a[0], (b[1] - a[1]) * t + a[1], (b[2] - a[2]) * t + a[2]]
}

/// `(a*w0 + b*w1) + c*w2` per component.
fn weighted3(a: Vec3, b: Vec3, c: Vec3, w0: f32, w1: f32, w2: f32) -> Vec3 {
    [
        (a[0] * w0 + b[0] * w1) + c[0] * w2,
        (a[1] * w0 + b[1] * w1) + c[1] * w2,
        (a[2] * w0 + b[2] * w1) + c[2] * w2,
    ]
}

/// `((a*w0 + b*w1) + c*w2) + d*w3` per component.
fn weighted4(a: Vec3, b: Vec3, c: Vec3, d: Vec3, w: [f32; 4]) -> Vec3 {
    [
        ((a[0] * w[0] + b[0] * w[1]) + c[0] * w[2]) + d[0] * w[3],
        ((a[1] * w[0] + b[1] * w[1]) + c[1] * w[2]) + d[1] * w[3],
        ((a[2] * w[0] + b[2] * w[1]) + c[2] * w[2]) + d[2] * w[3],
    ]
}

/// 0xD1ED0 `closestPtPointTriangle` (stack argument `a`, EDX `b`, ESI `c`, EDI `p`, EAX the result): the
/// point of triangle abc closest to `p` (Ericson's Voronoi-region test).
fn closest_pt_point_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> TriangleClosest {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    // ORDER (asm 0xD1F85..0xD1FC4): the dot products here are (z + y) + x; the source's are x, y, z.
    let d1 = (ap[2] * ab[2] + ap[1] * ab[1]) + ap[0] * ab[0];
    let d2 = (ap[2] * ac[2] + ap[1] * ac[1]) + ap[0] * ac[0];
    if d1 <= 0.0 && d2 <= 0.0 {
        return TriangleClosest { point: a, used: UsedVertices(UsedVertices::A), barycentric: [ONE, 0.0, 0.0] };
    }

    let bp = sub(p, b);
    // ORDER (asm 0xD2056..0xD20A6): (z + y) + x.
    let d3 = (bp[2] * ab[2] + bp[1] * ab[1]) + bp[0] * ab[0];
    let d4 = (bp[2] * ac[2] + bp[1] * ac[1]) + bp[0] * ac[0];
    if d3 >= 0.0 && d4 <= d3 {
        return TriangleClosest { point: b, used: UsedVertices(UsedVertices::B), barycentric: [0.0, ONE, 0.0] };
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return TriangleClosest {
            point: [a[0] + ab[0] * v, a[1] + ab[1] * v, a[2] + ab[2] * v],
            used: UsedVertices(UsedVertices::A | UsedVertices::B),
            barycentric: [ONE - v, v, 0.0],
        };
    }

    let cp = sub(p, c);
    // ORDER (asm 0xD2206..0xD2256): (z + y) + x.
    let d5 = (cp[2] * ab[2] + cp[1] * ab[1]) + cp[0] * ab[0];
    let d6 = (cp[2] * ac[2] + cp[1] * ac[1]) + cp[0] * ac[0];
    if d6 >= 0.0 && d5 <= d6 {
        return TriangleClosest { point: c, used: UsedVertices(UsedVertices::C), barycentric: [0.0, 0.0, ONE] };
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return TriangleClosest {
            point: [a[0] + ac[0] * w, a[1] + ac[1] * w, a[2] + ac[2] * w],
            used: UsedVertices(UsedVertices::A | UsedVertices::C),
            barycentric: [ONE - w, 0.0, w],
        };
    }

    let va = d3 * d6 - d5 * d4;
    let d4_d3 = d4 - d3;
    let d5_d6 = d5 - d6;
    if va <= 0.0 && d4_d3 >= 0.0 && d5_d6 >= 0.0 {
        // (d4 - d3) / ((d5 - d6) + (d4 - d3)) at 0xD2423: the sum's operands swapped, which is exact.
        let w = d4_d3 / (d5_d6 + d4_d3);
        return TriangleClosest {
            point: [b[0] + (c[0] - b[0]) * w, b[1] + (c[1] - b[1]) * w, b[2] + (c[2] - b[2]) * w],
            used: UsedVertices(UsedVertices::B | UsedVertices::C),
            barycentric: [0.0, ONE - w, w],
        };
    }

    let denom = ONE / ((va + vb) + vc);
    let v = vb * denom;
    let w = vc * denom;
    TriangleClosest {
        point: [
            (a[0] + ab[0] * v) + ac[0] * w,
            (a[1] + ab[1] * v) + ac[1] * w,
            (a[2] + ab[2] * v) + ac[2] * w,
        ],
        used: UsedVertices(UsedVertices::A | UsedVertices::B | UsedVertices::C),
        barycentric: [(ONE - v) - w, v, w],
    }
}

/// Which side of plane abc `p` is on, compared with `d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaneSide {
    /// `d` lies (almost) in the plane: the tetrahedron is flat (-1).
    Degenerate,
    /// `p` and `d` are on the same side, or `p` is on the plane (0).
    Inside,
    /// `p` and `d` are on opposite sides (1).
    Outside,
}

/// 0xD25B0 `pointOutsideOfPlane` (stack argument `a`, ESI `b`, EDX `c`, ECX `d`, EDI `p`; EAX -1, 0 or 1).
fn point_outside_of_plane(p: Vec3, a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> PlaneSide {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let normal = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let ap = sub(p, a);
    let ad = sub(d, a);
    // ORDER (asm 0xD267D..0xD2699, 0xD26BE..0xD26D0): both dots are (z + x) + y; the source's are x, y, z.
    let sign_d = (ad[2] * normal[2] + ad[0] * normal[0]) + ad[1] * normal[1];
    if sign_d * sign_d < PLANE_DEGENERACY {
        return PlaneSide::Degenerate;
    }
    let sign_p = (ap[2] * normal[2] + ap[0] * normal[0]) + ap[1] * normal[1];
    if sign_p * sign_d < 0.0 {
        PlaneSide::Outside
    } else {
        PlaneSide::Inside
    }
}

/// The answer of 0xD2700 `closestPtPointTetrahedron`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tetrahedron {
    /// The tetrahedron is flat (a plane test gave -1): the result's `m_degenerate` set, AL 0.
    Degenerate,
    /// The point is inside (no face has it outside): AL 0, the result left at the point with all vertices.
    Inside,
    /// The closest point on the nearest face it is outside of: AL 1.
    Outside(TetraClosest),
}

/// A closest point on a tetrahedron: the point, its vertices (bits A..D) and coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TetraClosest {
    point: Vec3,
    used: UsedVertices,
    barycentric: [f32; 4],
}

/// 0xD2700 `closestPtPointTetrahedron` (EAX `p`, stack `a`, `b`, `c`, `d`, the result).
fn closest_pt_point_tetrahedron(p: Vec3, a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Tetrahedron {
    let sides = [
        point_outside_of_plane(p, a, b, c, d),
        point_outside_of_plane(p, a, c, d, b),
        point_outside_of_plane(p, a, d, b, c),
        point_outside_of_plane(p, b, d, c, a),
    ];
    if sides.contains(&PlaneSide::Degenerate) {
        return Tetrahedron::Degenerate;
    }
    if !sides.contains(&PlaneSide::Outside) {
        return Tetrahedron::Inside;
    }
    let mut best_sq_dist = FLT_MAX;
    let mut best = TetraClosest { point: p, used: UsedVertices::ALL, barycentric: [0.0; 4] };
    // Each face: its corners as closestPtPointTriangle gets them, and the tetrahedron vertex (bit and
    // coordinate slot) each of those corners is (0xD2805..0xD2B4B).
    struct Face {
        side: PlaneSide,
        corners: [Vec3; 3],
        bits: [u8; 3],
        slots: [usize; 3],
    }
    let (ua, ub, uc, ud) = (UsedVertices::A, UsedVertices::B, UsedVertices::C, UsedVertices::D);
    let faces = [
        Face { side: sides[0], corners: [a, b, c], bits: [ua, ub, uc], slots: [0, 1, 2] },
        Face { side: sides[1], corners: [a, c, d], bits: [ua, uc, ud], slots: [0, 2, 3] },
        Face { side: sides[2], corners: [a, d, b], bits: [ua, ud, ub], slots: [0, 3, 1] },
        Face { side: sides[3], corners: [b, d, c], bits: [ub, ud, uc], slots: [1, 3, 2] },
    ];
    for Face { side, corners: [fa, fb, fc], bits, slots } in faces {
        if side != PlaneSide::Outside {
            continue;
        }
        let t = closest_pt_point_triangle(p, fa, fb, fc);
        let diff = sub(t.point, p);
        // ORDER (asm 0xD2818..0xD2851): (z*z + y*y) + x*x; the source's length2 is x, y, z.
        let sq_dist = (diff[2] * diff[2] + diff[1] * diff[1]) + diff[0] * diff[0];
        if sq_dist < best_sq_dist {
            best_sq_dist = sq_dist;
            let mut used = UsedVertices(0);
            let mut barycentric = [0.0; 4];
            let corners = [UsedVertices::A, UsedVertices::B, UsedVertices::C];
            for (((&corner, &bit), &slot), &coordinate) in
                corners.iter().zip(bits.iter()).zip(slots.iter()).zip(t.barycentric.iter())
            {
                if t.used.uses(corner) {
                    used = used.with(bit);
                }
                barycentric[slot] = coordinate;
            }
            best = TetraClosest { point: t.point, used, barycentric };
        }
    }
    Tetrahedron::Outside(best)
}

/// The axis roles of a cylinder (`XX`, `YY` = up, `ZZ` of Bullet's `CylinderLocalSupport{X,Y,Z}`), as the
/// switch at 0x8D84C..0x8D883 picks them.
#[derive(Clone, Copy, Debug)]
struct CylinderAxes {
    radial_1: usize,
    up: usize,
    radial_2: usize,
}

impl CylinderAxes {
    fn of(up: Axis) -> CylinderAxes {
        match up {
            Axis::X => CylinderAxes { radial_1: 1, up: 0, radial_2: 2 },
            Axis::Y => CylinderAxes { radial_1: 0, up: 1, radial_2: 2 },
            Axis::Z => CylinderAxes { radial_1: 0, up: 2, radial_2: 1 },
        }
    }
}

/// 0x8D680 `btConvexShape::localGetSupportVertexWithoutMarginNonVirtual` (ECX the shape; stack: the out
/// vector, the direction), for the cast shapes: the point of the shape without its margin farthest along
/// `dir`.
pub fn support_cast_shape(shape: &CastShape, dir: Vec3) -> Vec3 {
    match *shape {
        // Case 0x8D6A8: zero.
        CastShape::Sphere => ORIGIN,
        CastShape::Capsule { radius, half_height, scaling, margin, up } => {
            support_capsule(radius, half_height, scaling, margin, up, dir)
        }
        CastShape::Cylinder { radius, half_height, up } => support_cylinder(radius, half_height, up, dir),
    }
}

/// Case 0x8D9BB (type 10): `btCapsuleShape::localGetSupportingVertexWithoutMargin`.
fn support_capsule(radius: f32, half_height: f32, scaling: Vec3, margin: f32, up: Axis, dir: Vec3) -> Vec3 {
    // ORDER (asm 0x8DA15..0x8DA33): (x*x + z*z) + y*y; the source's length2 is x, y, z.
    let len_sqr = (dir[0] * dir[0] + dir[2] * dir[2]) + dir[1] * dir[1];
    let vec = if len_sqr < CAPSULE_SHORT_DIRECTION {
        [ONE, 0.0, 0.0]
    } else {
        // x87: 1/sqrt, then each component times it, each rounded to f32 (0x8DA76..0x8DAB7).
        let rlen = ONE / len_sqr.sqrt();
        [dir[0] * rlen, dir[1] * rlen, dir[2] * rlen]
    };
    let mut max_dot = CAPSULE_FIRST_MAX_DOT;
    let mut sup_vec = ORIGIN;
    for end in [half_height, -half_height] {
        let mut pos = ORIGIN;
        pos[up.index()] = end;
        // pos + vec*scaling*radius - vec*margin: ((s*v)*r + pos) - v*m per component (0x8DAE4..0x8DB59).
        let vtx = [
            ((scaling[0] * vec[0]) * radius + pos[0]) - vec[0] * margin,
            ((scaling[1] * vec[1]) * radius + pos[1]) - vec[1] * margin,
            ((scaling[2] * vec[2]) * radius + pos[2]) - vec[2] * margin,
        ];
        let new_dot = (vtx[0] * vec[0] + vtx[1] * vec[1]) + vtx[2] * vec[2];
        if new_dot > max_dot {
            max_dot = new_dot;
            sup_vec = vtx;
        }
    }
    sup_vec
}

/// Case 0x8D816 (type 13): Bullet's `CylinderLocalSupport{X,Y,Z}` on the half extents.
fn support_cylinder(radius: f32, half_height: f32, up: Axis, v: Vec3) -> Vec3 {
    let axes = CylinderAxes::of(up);
    let mut tmp = ORIGIN;
    let s = (v[axes.radial_1] * v[axes.radial_1] + v[axes.radial_2] * v[axes.radial_2]).sqrt();
    // btFsels(v[up], halfHeight, -halfHeight): `comiss 0, v[up]; jna` keeps the plus sign unless 0 > v[up].
    let along = if 0.0 > v[axes.up] { -half_height } else { half_height };
    if s != 0.0 {
        let d = radius / s;
        tmp[axes.radial_1] = v[axes.radial_1] * d;
        tmp[axes.up] = along;
        tmp[axes.radial_2] = v[axes.radial_2] * d;
    } else {
        tmp[axes.radial_1] = radius;
        tmp[axes.up] = along;
        tmp[axes.radial_2] = 0.0;
    }
    tmp
}

/// 0x8D680 `btConvexShape::localGetSupportVertexWithoutMarginNonVirtual` (ECX the shape; stack: the out
/// vector, the direction), for the world shapes: the point of the shape without its margin farthest along
/// `dir`.
pub fn support_world_shape(shape: &WorldShape, dir: Vec3) -> Vec3 {
    match *shape {
        WorldShape::Triangle(ref triangle) => support_triangle(triangle, dir),
        WorldShape::Box { half_extents } => support_box(half_extents, dir),
        // Case 0x8D6A8: zero.
        WorldShape::Sphere => ORIGIN,
        WorldShape::Cylinder { radius, half_height, up } => support_cylinder(radius, half_height, up, dir),
    }
}

/// Case 0x8D6CA (type 0, `btBoxShape`): each half extent with the sign of the direction's component
/// (`btFsels(dir, h, -h)`). `comiss dir, 0; jnc` keeps `+h` unless `dir < 0` or NaN; `-h` is a sign flip
/// (`xorps` with the sign mask 0xA2D7C0). The components are done z, y, x, which nothing observes.
fn support_box(half_extents: Vec3, dir: Vec3) -> Vec3 {
    let pick = |d: f32, h: f32| if d >= 0.0 { h } else { -h };
    [pick(dir[0], half_extents[0]), pick(dir[1], half_extents[1]), pick(dir[2], half_extents[2])]
}

/// 0x8D680, case 0x8D72C (type 1, `btTriangleShape::localGetSupportingVertexWithoutMargin`): the corner with
/// the largest dot product with `dir` (`dots.maxAxis()`).
pub fn support_triangle(triangle: &Triangle, dir: Vec3) -> Vec3 {
    let v = &triangle.0;
    // ORDER (asm 0x8D74F..0x8D7A7): each dot is (y + z) + x; the source's is x, y, z.
    let dot = |c: &Vec3| (c[1] * dir[1] + c[2] * dir[2]) + c[0] * dir[0];
    let (d0, d1, d2) = (dot(&v[0]), dot(&v[1]), dot(&v[2]));
    let index = if d0 < d1 {
        if d1 < d2 {
            2
        } else {
            1
        }
    } else if d0 < d2 {
        2
    } else {
        0
    };
    v[index]
}

/// How the GJK loop ended.
enum LoopEnd {
    /// `checkSimplex`: the closest points are taken from the simplex.
    CheckSimplex,
    /// The full simplex (13) or the iteration limit: no closest points from GJK.
    Done,
}

/// 0xD6CD0 `btGjkPairDetector::getClosestPointsNonVirtual` (stack: the detector, the input, the result, the
/// debug drawer) for a cast shape A and a shape B (a [`Triangle`] or a [`WorldShape`]), up to the
/// penetration-depth solver. Both supports are the one function 0x8D680 (calls at 0xD70C6 for A, 0xD70DB for
/// B).
pub fn get_closest_points<B: ShapeB>(
    detector: &Detector,
    shape_a: &CastShape,
    shape_b: &B,
    input: &ClosestPointInput,
) -> ClosestPoints {
    let mut state = DetectorState {
        last_used_method: LastUsedMethod::Unset,
        cur_iter: 0,
        degenerate_simplex: DegenerateSimplex::None,
        cached_separating_axis: [0.0, ONE, 0.0],
        cached_separating_distance: 0.0,
    };

    // 0xD6DF2..0xD6EA4: positionOffset = (B.origin + A.origin) * 0.5, both origins minus it.
    let oa = input.transform_a.origin;
    let ob = input.transform_b.origin;
    let offset = [(ob[0] + oa[0]) * HALF, (ob[1] + oa[1]) * HALF, (ob[2] + oa[2]) * HALF];
    let local_a = Transform { basis: input.transform_a.basis, origin: sub(oa, offset) };
    let local_b = Transform { basis: input.transform_b.basis, origin: sub(ob, offset) };
    // `check2d` (0xD6EC8..0xD6EE9, applied at 0xD728B) needs both shapes 2D (types 17, 18); no cast shape is,
    // so it never applies.

    let (margin_a, margin_b) = detector.margins.values();
    let margin_sum = margin_b + margin_a;
    let mut squared_distance = BT_LARGE_FLOAT;
    let mut solver = SimplexSolver::new();

    let end = loop {
        let axis = state.cached_separating_axis;
        let n = [-axis[0], -axis[1], -axis[2]];
        let ma = &input.transform_a.basis;
        let mb = &input.transform_b.basis;
        // seperatingAxisInA = -axis * basisA (a column of the basis each).
        // ORDER (asm 0xD6F9A..0xD7019): x (n.z*m20 + n.y*m10) + m00*n.x, y (n.z*m21 + n.x*m01) + n.y*m11,
        // z (m12*n.y + n.z*m22) + m02*n.x; the source's are x, y, z.
        let axis_in_a = [
            (n[2] * ma[2][0] + n[1] * ma[1][0]) + ma[0][0] * n[0],
            (n[2] * ma[2][1] + n[0] * ma[0][1]) + n[1] * ma[1][1],
            (ma[1][2] * n[1] + n[2] * ma[2][2]) + ma[0][2] * n[0],
        ];
        // seperatingAxisInB = axis * basisB.
        // ORDER (asm 0xD702B..0xD70A6): x (m20*a.z + m00*a.x) + m10*a.y, y (a.y*m11 + a.z*m21) + m01*a.x,
        // z (m12*a.y + m22*a.z) + a.x*m02.
        let axis_in_b = [
            (mb[2][0] * axis[2] + mb[0][0] * axis[0]) + mb[1][0] * axis[1],
            (axis[1] * mb[1][1] + axis[2] * mb[2][1]) + mb[0][1] * axis[0],
            (mb[1][2] * axis[1] + mb[2][2] * axis[2]) + axis[0] * mb[0][2],
        ];
        let p_in_a = support_cast_shape(shape_a, axis_in_a);
        let q_in_b = shape_b.support(axis_in_b);
        let p_world = local_a.apply(p_in_a);
        let q_world = local_b.apply(q_in_b);
        let w = sub(p_world, q_world);
        // ORDER (asm 0xD729F..0xD72D2): delta = (a.z*w.z + a.y*w.y) + w.x*a.x; the source's dot is x, y, z.
        let delta = (axis[2] * w[2] + axis[1] * w[1]) + w[0] * axis[0];

        if delta > 0.0 && delta * delta > input.maximum_distance_squared * squared_distance {
            state.degenerate_simplex = DegenerateSimplex::Separated;
            break LoopEnd::CheckSimplex;
        }
        if solver.in_simplex(w) {
            state.degenerate_simplex = DegenerateSimplex::VertexInSimplex;
            break LoopEnd::CheckSimplex;
        }
        let f0 = squared_distance - delta;
        let f1 = squared_distance * REL_ERROR2;
        if f0 <= f1 {
            state.degenerate_simplex =
                if f0 <= 0.0 { DegenerateSimplex::NoProgressNonPositive } else { DegenerateSimplex::NoProgress };
            break LoopEnd::CheckSimplex;
        }

        solver.add_vertex(w, p_world, q_world);
        let (valid, new_axis) = solver.closest();
        if !valid {
            state.degenerate_simplex = DegenerateSimplex::ClosestInvalid;
            break LoopEnd::CheckSimplex;
        }
        let len_sqr = (new_axis[0] * new_axis[0] + new_axis[1] * new_axis[1]) + new_axis[2] * new_axis[2];
        if len_sqr < REL_ERROR2 {
            state.cached_separating_axis = new_axis;
            state.degenerate_simplex = DegenerateSimplex::AxisVanished;
            break LoopEnd::CheckSimplex;
        }
        let previous_squared_distance = squared_distance;
        squared_distance = len_sqr;
        // This build has no `backup_closest` here (0xD73EA -> 0xD75BE), and the squared distance is already
        // the new one.
        if previous_squared_distance - squared_distance <= previous_squared_distance * SIMD_EPSILON {
            state.degenerate_simplex = DegenerateSimplex::DistanceStalled;
            break LoopEnd::CheckSimplex;
        }
        state.cached_separating_axis = new_axis;
        let iteration = state.cur_iter;
        state.cur_iter += 1;
        if iteration > GJK_MAX_ITER {
            break LoopEnd::Done;
        }
        if solver.full_simplex() {
            state.degenerate_simplex = DegenerateSimplex::FullSimplex;
            break LoopEnd::Done;
        }
    };

    let mut gjk = None;
    if let LoopEnd::CheckSimplex = end {
        // 0xD7459: compute_points keeps only point B; normalInB is the cached axis.
        let point_on_b = solver.compute_point_b();
        let axis = state.cached_separating_axis;
        let len_sqr = (axis[0] * axis[0] + axis[1] * axis[1]) + axis[2] * axis[2];
        if (len_sqr as f64) < SHORT_AXIS {
            state.degenerate_simplex = DegenerateSimplex::ShortAxis;
        }
        if len_sqr > SIMD_EPSILON_SQUARED {
            // x87 (0xD74E8..0xD7547): rlen = 1/sqrt(lenSqr); s = marginB / sqrt(squaredDistance).
            let rlen = ONE / len_sqr.sqrt();
            let s = margin_b / squared_distance.sqrt();
            gjk = Some(GjkClosest {
                normal_on_b: [axis[0] * rlen, axis[1] * rlen, axis[2] * rlen],
                point_on_b: [
                    axis[0] * s + point_on_b[0],
                    axis[1] * s + point_on_b[1],
                    axis[2] * s + point_on_b[2],
                ],
                distance: ONE / rlen - margin_sum,
            });
            state.last_used_method = LastUsedMethod::Gjk;
        } else {
            state.last_used_method = LastUsedMethod::GjkAxisTooShort;
        }
    }

    // 0xD75DA..0xD7625: an invalid answer, or a caught degeneracy, goes to the penetration solver.
    let needs_penetration = match detector.penetration {
        Penetration::NoSolver => false,
        Penetration::Solver { catch_degeneracies } => match gjk {
            None => true,
            Some(g) => {
                catch_degeneracies
                    && state.degenerate_simplex != DegenerateSimplex::None
                    && ((margin_sum + g.distance) as f64) < CATCH_DEGENERACY_DISTANCE
            }
        },
    };
    if needs_penetration {
        let start = PenetrationStart { solver, gjk, local_a, local_b, position_offset: offset, margin_sum };
        return ClosestPoints { outcome: Outcome::NeedsPenetration(Box::new(start)), state };
    }

    // 0xD77EE: report a valid answer closer than the maximum distance.
    let outcome = match gjk {
        Some(g) if g.distance < 0.0 || g.distance * g.distance < input.maximum_distance_squared => {
            state.cached_separating_distance = g.distance;
            state.cached_separating_axis = g.normal_on_b;
            Outcome::Contact {
                normal_on_b: g.normal_on_b,
                point_on_b: [
                    g.point_on_b[0] + offset[0],
                    g.point_on_b[1] + offset[1],
                    g.point_on_b[2] + offset[2],
                ],
                distance: g.distance,
            }
        }
        _ => Outcome::NoContact,
    };
    ClosestPoints { outcome, state }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn sphere_above_a_triangle_is_its_height_minus_the_radius() {
        let detector = Detector {
            margins: Margins::Applied { a: 0.5, b: 0.0 },
            penetration: Penetration::Solver { catch_degeneracies: true },
        };
        let triangle = Triangle([[-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 0.0, 1.0]]);
        let input = ClosestPointInput {
            transform_a: Transform { basis: IDENTITY, origin: [0.0, 2.0, 0.0] },
            transform_b: Transform { basis: IDENTITY, origin: ORIGIN },
            maximum_distance_squared: BT_LARGE_FLOAT,
        };
        let answer = get_closest_points(&detector, &CastShape::Sphere, &triangle, &input);
        match answer.outcome {
            Outcome::Contact { normal_on_b, point_on_b, distance } => {
                assert_eq!(normal_on_b, [0.0, 1.0, 0.0]);
                assert_eq!(distance, 1.5);
                assert_eq!(point_on_b, [0.0, 0.0, 0.0]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(answer.state.last_used_method, LastUsedMethod::Gjk);
    }

    #[test]
    fn capsule_radius_is_on_the_axis_after_up_plus_two() {
        let shape = CastShape::capsule([0.38, 0.12, 0.4], [1.0; 3], 0.04, Axis::Y);
        assert_eq!(
            shape,
            CastShape::Capsule { radius: 0.38, half_height: 0.12, scaling: [1.0; 3], margin: 0.04, up: Axis::Y }
        );
    }

    #[test]
    fn box_support_keeps_the_plus_sign_on_zero_and_takes_the_minus_on_nan() {
        let half = [1.0, 2.0, 3.0];
        assert_eq!(support_box(half, [-0.0, 0.5, -0.5]), [1.0, 2.0, -3.0]);
        assert_eq!(support_box(half, [f32::NAN, 0.0, 1.0]), [-1.0, 2.0, 3.0]);
    }

    #[test]
    fn sphere_above_a_box_is_its_height_minus_the_half_extent_and_radius() {
        let detector = Detector {
            margins: Margins::Applied { a: 0.5, b: 0.0 },
            penetration: Penetration::Solver { catch_degeneracies: true },
        };
        let input = ClosestPointInput {
            transform_a: Transform { basis: IDENTITY, origin: [0.0, 2.0, 0.0] },
            transform_b: Transform { basis: IDENTITY, origin: ORIGIN },
            maximum_distance_squared: BT_LARGE_FLOAT,
        };
        let shape_b = WorldShape::Box { half_extents: [1.0, 0.5, 1.0] };
        let answer = get_closest_points(&detector, &CastShape::Sphere, &shape_b, &input);
        match answer.outcome {
            Outcome::Contact { normal_on_b, distance, .. } => {
                assert_eq!(normal_on_b, [0.0, 1.0, 0.0]);
                assert_eq!(distance, 1.0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_triangle_gives_the_same_answer_alone_or_as_a_world_shape() {
        let detector = Detector {
            margins: Margins::Applied { a: 0.04, b: 0.0 },
            penetration: Penetration::Solver { catch_degeneracies: true },
        };
        let triangle = Triangle([[-1.0, 0.1, -1.0], [1.0, -0.2, -1.0], [0.0, 0.0, 1.0]]);
        let input = ClosestPointInput {
            transform_a: Transform { basis: IDENTITY, origin: [0.1, 1.0, 0.2] },
            transform_b: Transform { basis: IDENTITY, origin: ORIGIN },
            maximum_distance_squared: BT_LARGE_FLOAT,
        };
        let shape_a = CastShape::capsule([0.38, 0.52, 0.38], [1.0; 3], 0.04, Axis::Y);
        assert_eq!(
            get_closest_points(&detector, &shape_a, &triangle, &input),
            get_closest_points(&detector, &shape_a, &WorldShape::from(triangle), &input)
        );
    }

    #[test]
    fn triangle_support_takes_the_first_of_equal_corners() {
        let triangle = Triangle([[0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]]);
        assert_eq!(support_triangle(&triangle, [0.0, 1.0, 0.0]), [0.0, 1.0, 0.0]);
    }
}
