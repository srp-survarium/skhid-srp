//! The game's own Bullet shapes for the placed collision: what the 0.26g executable builds from a project's
//! `collision_objects` and their libraries, field for field with the bits it has in memory, so the walk's casts
//! can meet the shapes the game casts against (a placed mesh in its own frame under the object's transform, a
//! box, a cylinder, a sphere or a capsule as the convex shape itself) and not only their triangles.
//!
//! Everything here is in **Bullet's frame**: the map frame with z negated (`from_vostok` 0x74470). A mesh's
//! vertices stay in the map frame and its scaling turns them into Bullet's (`create_btBvhTriangleMeshShape`
//! 0x7A6E0: scaling `(sx, sy, -sz)`); the library's vertices here have the scale applied already
//! ([`crate::Library`]: `vertex × (sx, sy, sz)` in `f32`), so a corner as the game's cast takes it, `vertex ×
//! scaling`, is the library's vertex with z negated, the same bits.
//!
//! Read in the exe (image base 0x10000; `docs/notes/collision_world.md` §1, `tools/re-kit/v026g`), all PROVEN:
//! - one shape per `lib_name` (the cook, `on_collision_sources_loaded` 0x829E0): its primitives (0x83600) are
//!   one primitive by itself when there is exactly one and its position is within 1e-5 of the origin on every
//!   axis (0x83638..0x836BF; the primitive's rotation and position are then not used, `create_primitive_shape`
//!   0x7A390), else a `btCompoundShape` (`create_compound_shape` 0x7A460). A mesh is a
//!   `btBvhTriangleMeshShape` by itself, or with primitives the compound's last child, added after the
//!   compound's scaling, under the identity (0x83486..0x834B1);
//! - a primitive (`create_bt_primitive` 0x7A1F0): type 0 `btSphereShape(dim.x)` (0x74210), 1 `btBoxShape(dim)`
//!   (0xA96D0), 2 `btCylinderShape(dim.x, dim.y, dim.x)` (0xA9D40), 3 `btCapsuleShape(dim.x, dim.y)` (0xA5E20),
//!   `dim` the primitive's `scale`; then `setLocalScaling(the library's scale)` on the lone primitive or on the
//!   compound;
//! - a compound's child transform is `from_vostok(create_rotation(rotation) * create_translation(position))`
//!   (0x725E0, 0x573C0, `multiply_affine` 0xEE700, 0x74470); the compound's `setLocalScaling` (Bullet 2.78)
//!   scales each child's origin and passes `child scaling × scale / compound scaling` to the child;
//! - an object's transform is the placement's: the loader (0x22C3D0..0x22C458) makes
//!   `create_scale(scale) * create_rotation(rotation) * create_translation(position)` (0x2F8770), then sets the
//!   scale of its axes to (1, 1, 1) (0x727B0): each basis row made unit, so the scale is gone but its rounding
//!   stays; the body takes it through `from_vostok` (`set_transform` 0x356CC0).

use std::ops::Range;

use crate::controller::exe_gjk::{Axis, Transform};
use crate::library::WalkerBehaviour;
use crate::math::Vec3;

/// 0xA2D7DC: Bullet's default collision margin, 0.04 (`btConvexInternalShape`'s, set by every constructor).
const DEFAULT_MARGIN: f32 = f32::from_bits(0x3D23_D70A);
/// 0x921948: `setSafeMargin`'s multiplier, 0.1 of the smallest half extent (0xA8CA0).
const SAFE_MARGIN_MULTIPLIER: f32 = f32::from_bits(0x3DCC_CCCD);
/// 0x927A44: the capsule's half of its height (0xA5E30).
const HALF: f32 = 0.5;
/// 0x9CBA48: how near the origin, on every axis, a lone primitive must be to be built by itself (0x83663).
const LONE_AT_ORIGIN: f32 = f32::from_bits(0x3727_C5AC);

/// A convex primitive's kind, with the up axis (+40h) of the kinds that have one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvexKind {
    /// `BOX_SHAPE_PROXYTYPE` (0).
    Box,
    /// `SPHERE_SHAPE_PROXYTYPE` (8).
    Sphere,
    /// `CAPSULE_SHAPE_PROXYTYPE` (10).
    Capsule { up: Axis },
    /// `CYLINDER_SHAPE_PROXYTYPE` (13).
    Cylinder { up: Axis },
}

impl ConvexKind {
    /// Bullet's shape type (+04h of the shape).
    pub fn bullet_type(self) -> u8 {
        match self {
            ConvexKind::Box => 0,
            ConvexKind::Sphere => 8,
            ConvexKind::Capsule { .. } => 10,
            ConvexKind::Cylinder { .. } => 13,
        }
    }

    /// The kind of Bullet's shape type and up axis (the up axis of a kind without one is not read).
    pub fn from_bullet(bullet_type: u8, up: Axis) -> Option<ConvexKind> {
        match bullet_type {
            0 => Some(ConvexKind::Box),
            8 => Some(ConvexKind::Sphere),
            10 => Some(ConvexKind::Capsule { up }),
            13 => Some(ConvexKind::Cylinder { up }),
            _ => None,
        }
    }

    /// The up axis, for a capsule and a cylinder.
    pub fn up(self) -> Option<Axis> {
        match self {
            ConvexKind::Capsule { up } | ConvexKind::Cylinder { up } => Some(up),
            ConvexKind::Box | ConvexKind::Sphere => None,
        }
    }
}

/// A convex primitive as the game holds it (`btConvexInternalShape`).
#[derive(Clone, Debug, PartialEq)]
pub struct Convex {
    pub kind: ConvexKind,
    /// +10h, the local scaling (its absolute value, as `setLocalScaling` stores it).
    pub scaling: [f32; 3],
    /// +20h, the implicit dimensions: a box's and a cylinder's half extents less the margin, a capsule's radius,
    /// half height and radius, a sphere's radius (then 0, 0). Scaled.
    pub dims: [f32; 3],
    /// +30h, the collision margin (a sphere's is its radius, unscaled: its `getMargin` is radius × scaling).
    pub margin: f32,
    /// The walker behaviour of the primitive's material (its word in the shape's face data, >> 14).
    pub walker: WalkerBehaviour,
    /// Its triangles in the library ([`crate::Library::faces`]): the stand-in for rays, the grid and sight.
    pub faces: Range<u32>,
}

impl Convex {
    /// `create_bt_primitive` 0x7A1F0: the primitive of the exported `type` with the exported `scale` as its
    /// dimensions; `None` for a type it does not make (above 3).
    pub fn create(
        kind: i32,
        dim: Vec3,
        walker: WalkerBehaviour,
        faces: Range<u32>,
    ) -> Option<Convex> {
        let unscaled = |kind, dims, margin| Convex {
            kind,
            scaling: [1.0; 3],
            dims,
            margin,
            walker,
            faces: faces.clone(),
        };
        match kind {
            // btSphereShape (0x74210): the radius is +20h and the margin.
            0 => Some(unscaled(ConvexKind::Sphere, [dim.x, 0.0, 0.0], dim.x)),
            // btBoxShape (0xA96D0) and btCylinderShape (0xA9D40): setSafeMargin(half extents), then
            // `half × scaling - margin` (the scaling is 1 here).
            1 => Some(boxlike(
                ConvexKind::Box,
                [dim.x, dim.y, dim.z],
                walker,
                faces,
            )),
            2 => Some(boxlike(
                ConvexKind::Cylinder { up: Axis::Y },
                [dim.x, dim.y, dim.x],
                walker,
                faces,
            )),
            // btCapsuleShape (0xA5E20): radius, height × 0.5, radius; the default margin.
            3 => Some(unscaled(
                ConvexKind::Capsule { up: Axis::Y },
                [dim.x, dim.y * HALF, dim.x],
                DEFAULT_MARGIN,
            )),
            _ => None,
        }
    }

    /// `setLocalScaling`: a sphere's (0x8DEF0) only stores the scaling's absolute value; a box's, a cylinder's
    /// and a capsule's (vtable slot 5, 0xA9BE0) keep the margin: `scaling' × ((dims + margin) / scaling) - margin`.
    pub fn set_scaling(&mut self, scaling: [f32; 3]) {
        let new = scaling.map(f32::abs);
        if self.kind != ConvexKind::Sphere {
            let m = self.margin;
            for i in 0..3 {
                let unscaled = (self.dims[i] + m) / self.scaling[i];
                self.dims[i] = new[i] * unscaled - m;
            }
        }
        self.scaling = new;
    }
}

/// A box's or a cylinder's constructor (0xA96D0, 0xA9D40): the default margin lowered to a tenth of the
/// smallest half extent when that is less (`setSafeMargin` 0xA8CA0), then the implicit dimensions.
fn boxlike(kind: ConvexKind, half: [f32; 3], walker: WalkerBehaviour, faces: Range<u32>) -> Convex {
    let margin = boxlike_margin(half);
    Convex {
        kind,
        scaling: [1.0; 3],
        dims: half.map(|h| h * 1.0 - margin),
        margin,
        walker,
        faces,
    }
}

/// The margin a box's or a cylinder's constructor (0xA96D0, 0xA9D40) gives a shape of half extents `half` (a cylinder's
/// `[radius, half height, radius]`): the default 0.04 (0xA2D7DC) lowered to a tenth (0x921948) of the smallest half
/// extent when that is less (`setSafeMargin` 0xA8CA0). The shape is its core, `half - margin`, grown by the margin: its
/// faces where the data puts them, its edges rounded by the margin.
pub fn boxlike_margin(half: [f32; 3]) -> f32 {
    // `minAxis` (0xA8CA0..0xA8CD1): on a tie the later axis.
    let [x, y, z] = half;
    let smallest = if y > x {
        if z > x { 0 } else { 2 }
    } else if z > y {
        1
    } else {
        2
    };
    let safe = half[smallest] * SAFE_MARGIN_MULTIPLIER;
    if DEFAULT_MARGIN > safe {
        safe
    } else {
        DEFAULT_MARGIN
    }
}

/// A triangle mesh (`btBvhTriangleMeshShape`): the library's faces in `faces`, in the file's order, under
/// `scaling` (the mesh interface's, +10h: `(sx, sy, -sz)`). The game's margin of a mesh is 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub scaling: [f32; 3],
    pub faces: Range<u32>,
}

/// A compound's child shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Mesh(Mesh),
    Convex(Convex),
}

/// A compound's child: its transform in the compound (scaled with it) and its shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub transform: Transform,
    pub part: Part,
}

/// A library's shape, as the game makes it for every placement of it.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// No files: the game makes no shape.
    Empty,
    Mesh(Mesh),
    /// A lone primitive at the origin.
    Convex(Convex),
    /// `btCompoundShape` (31): its local scaling (+50h) and children (+18h) in the game's order.
    Compound {
        scaling: [f32; 3],
        children: Vec<Child>,
    },
}

/// What a library's primitives become before its mesh is known (the cook's 0x83600).
#[derive(Clone, Debug, PartialEq)]
pub enum Primitives {
    None,
    Lone(Convex),
    Compound {
        scaling: [f32; 3],
        children: Vec<Child>,
    },
}

/// One exported primitive, read: its kind, dims and placement in the library and its triangles.
pub struct PrimitiveRow {
    pub kind: i32,
    pub dim: Vec3,
    pub position: Vec3,
    pub rotation: Vec3,
    pub walker: WalkerBehaviour,
    pub faces: Range<u32>,
}

/// Whether the cook builds a library's primitives as one primitive by itself (0x83600: exactly one, its
/// position within [`LONE_AT_ORIGIN`] of the origin on every axis).
pub fn is_lone(count: usize, first_position: Vec3) -> bool {
    count == 1
        && first_position
            .to_array()
            .iter()
            .all(|c| LONE_AT_ORIGIN > c.abs())
}

impl Primitives {
    /// The cook's primitives (0x83600) under the library's `scale`. A row of a type the game does not make
    /// is an error: the game would add an empty child.
    pub fn build(rows: Vec<PrimitiveRow>, scale: Vec3) -> Result<Primitives, String> {
        let scale = scale.to_array();
        let convex = |row: &PrimitiveRow| {
            Convex::create(row.kind, row.dim, row.walker, row.faces.clone())
                .ok_or_else(|| format!("primitive type {} is none the game makes", row.kind))
        };
        match rows.as_slice() {
            [] => Ok(Primitives::None),
            [row] if is_lone(1, row.position) => {
                let mut lone = convex(row)?;
                lone.set_scaling(scale);
                Ok(Primitives::Lone(lone))
            }
            rows => {
                let mut children = Vec::with_capacity(rows.len());
                for row in rows {
                    let transform = from_vostok(&create_rotation(row.rotation), row.position);
                    children.push(Child {
                        transform,
                        part: Part::Convex(convex(row)?),
                    });
                }
                // btCompoundShape::setLocalScaling (2.78), from a scaling of 1.
                let old = [1.0f32; 3];
                for child in &mut children {
                    if let Part::Convex(c) = &mut child.part {
                        let s = [0, 1, 2].map(|i| c.scaling[i] * scale[i] / old[i]);
                        c.set_scaling(s);
                    }
                    for i in 0..3 {
                        child.transform.origin[i] *= scale[i];
                    }
                }
                Ok(Primitives::Compound {
                    scaling: scale,
                    children,
                })
            }
        }
    }

    /// The library's shape with its mesh, if it has one (the cook, 0x832D8..0x834B1). A mesh beside a lone
    /// primitive is an error: the game adds the mesh to it as to a compound.
    pub fn with_mesh(self, mesh: Option<Mesh>) -> Result<Shape, String> {
        match (self, mesh) {
            (Primitives::None, None) => Ok(Shape::Empty),
            (Primitives::None, Some(mesh)) => Ok(Shape::Mesh(mesh)),
            (Primitives::Lone(convex), None) => Ok(Shape::Convex(convex)),
            (Primitives::Lone(_), Some(_)) => Err("a mesh beside a lone primitive".into()),
            (Primitives::Compound { scaling, children }, None) => {
                Ok(Shape::Compound { scaling, children })
            }
            (
                Primitives::Compound {
                    scaling,
                    mut children,
                },
                Some(mesh),
            ) => {
                children.push(Child {
                    transform: from_vostok(&IDENTITY_ROTATION, Vec3::ZERO),
                    part: Part::Mesh(mesh),
                });
                Ok(Shape::Compound { scaling, children })
            }
        }
    }
}

/// A mesh's scaling (0x7A6E0): the library's scale with z negated.
pub fn mesh_scaling(scale: Vec3) -> [f32; 3] {
    [scale.x, scale.y, -scale.z]
}

/// The vostok rotation rows (row vectors, `v × M`).
type Rows = [[f32; 3]; 3];

const IDENTITY_ROTATION: Rows = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `vostok::math::create_rotation` (0x725E0): rows `Rx(-x) Ry(-y) Rz(-z)` from x87 `fsin`/`fcos` of each angle
/// (0x38FF0, 0x39000), each stored as `f32`, except cos(z), kept at the x87's precision for its one product
/// with cos(y) (0x72643..0x7266F). The rest in scalar SSE, in the exe's grouping (0x72648..0x72759).
pub fn create_rotation(angles: Vec3) -> Rows {
    let fsin = |a: f32| (a as f64).sin();
    let fcos = |a: f32| (a as f64).cos();
    let (sx, cx) = (fsin(angles.x) as f32, fcos(angles.x) as f32);
    let (sy, cy) = (fsin(angles.y) as f32, fcos(angles.y) as f32);
    let sz = fsin(angles.z) as f32;
    let cz_wide = fcos(angles.z);
    let cz = cz_wide as f32;
    let czsx = cz * sx;
    let sysz = sy * sz;
    let sycz = sy * cz;
    [
        [((cy as f64) * cz_wide) as f32, -(cy * sz), sy],
        [sz * cx + sy * czsx, cz * cx - sysz * sx, -(cy * sx)],
        [sz * sx - sycz * cx, sysz * cx + czsx, cx * cy],
    ]
}

/// `from_vostok` (0x74470): the rows of the vostok matrix (rotation `m`, translation `t`) made a `btTransform`:
/// the basis is the transpose with the sign of every element in exactly one z row or column flipped, the origin
/// `(tx, ty, -tz)`; an element equal to zero becomes +0.
pub fn from_vostok(m: &Rows, t: Vec3) -> Transform {
    let zero_plus = |v: f32| if v == 0.0 { 0.0 } else { v };
    let mut basis = [[0.0; 3]; 3];
    for (r, row) in basis.iter_mut().enumerate() {
        for (c, value) in row.iter_mut().enumerate() {
            let flip = (r == 2) != (c == 2);
            let v = m[c][r];
            *value = zero_plus(if flip { -v } else { v });
        }
    }
    Transform {
        basis,
        origin: [zero_plus(t.x), zero_plus(t.y), zero_plus(-t.z)],
    }
}

/// A placed object's world transform (the loader, 0x22C3D0..0x22C458): `scale * create_rotation(rotation) *
/// create_translation(position)` (0x2F8770; each row of the rotation times its scale, the zeros of the scale matrix
/// adding nothing), its axes then made unit ([`unit_axes`]), through `from_vostok`.
pub fn object_transform(rotation: Vec3, position: Vec3, scale: Vec3) -> Transform {
    let r = create_rotation(rotation);
    let s = scale.to_array();
    let scaled: Rows = [0, 1, 2].map(|row| r[row].map(|v| s[row] * v));
    from_vostok(&unit_axes(scaled), position)
}

/// 0x727B0 with the scale (1, 1, 1): each basis row times `1 / sqrt(its length squared)`. The x87 runs at 24-bit
/// precision there (PROVEN: the bits of all 2,182 objects of a capture follow from it, not from a wider one), so
/// the square root and the quotient are `f32`'s. The length of rows 0 and 1 is summed `(x*x + y*y) + z*z`, that of
/// row 2 `(y*y + z*z) + x*x` (0x7287B..0x728A3).
fn unit_axes(m: Rows) -> Rows {
    let mut out = m;
    for (row, axis) in out.iter_mut().enumerate() {
        let [x, y, z] = *axis;
        let length2 = if row < 2 {
            (x * x + y * y) + z * z
        } else {
            (y * y + z * z) + x * x
        };
        let k = 1.0 / length2.sqrt();
        *axis = [x * k, y * k, z * k];
    }
    out
}

/// The game's shapes of a map's placed collision: one [`Shape`] for each library and one object transform
/// for each placement, in the map's order (the game's world order). Made whole or not at all: an export
/// older than the format's version 6 has none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BulletWorld {
    shapes: Vec<Shape>,
    objects: Vec<Transform>,
}

impl BulletWorld {
    /// `shapes` for `libraries` libraries (their face counts) and `objects` for the placements: an error when
    /// a count differs or a shape names faces its library does not have.
    pub fn new(
        shapes: Vec<Shape>,
        objects: Vec<Transform>,
        face_counts: &[u32],
        placements: usize,
    ) -> Result<Self, String> {
        if shapes.len() != face_counts.len() {
            return Err(format!(
                "{} shapes for {} libraries",
                shapes.len(),
                face_counts.len()
            ));
        }
        if objects.len() != placements {
            return Err(format!(
                "{} object transforms for {placements} placements",
                objects.len()
            ));
        }
        for (index, (shape, &faces)) in shapes.iter().zip(face_counts).enumerate() {
            let fits = |range: &Range<u32>| range.start <= range.end && range.end <= faces;
            let ok = match shape {
                Shape::Empty => true,
                Shape::Mesh(mesh) => fits(&mesh.faces),
                Shape::Convex(convex) => fits(&convex.faces),
                Shape::Compound { children, .. } => {
                    children.iter().all(|child| match &child.part {
                        Part::Mesh(mesh) => fits(&mesh.faces),
                        Part::Convex(convex) => fits(&convex.faces),
                    })
                }
            };
            if !ok {
                return Err(format!(
                    "library {index}: a shape names faces past its {faces}"
                ));
            }
        }
        Ok(Self { shapes, objects })
    }

    /// The shape of library `index`.
    pub fn shape(&self, library: u32) -> &Shape {
        &self.shapes[library as usize]
    }

    /// The world transform of placement `index`.
    pub fn object(&self, placement: usize) -> &Transform {
        &self.objects[placement]
    }

    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    pub fn objects(&self) -> &[Transform] {
        &self.objects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_keeps_its_safe_margin_through_scaling() {
        // A thin box: the margin is a tenth of its smallest half extent.
        let mut b =
            Convex::create(1, Vec3::new(0.5, 0.02, 0.3), WalkerBehaviour::Walk, 0..12).unwrap();
        assert_eq!(b.margin, 0.02 * SAFE_MARGIN_MULTIPLIER);
        assert_eq!(b.dims[1], 0.02 - b.margin);
        b.set_scaling([2.0, 2.0, -2.0]);
        assert_eq!(b.scaling, [2.0, 2.0, 2.0]);
        assert_eq!(b.dims[0], 2.0 * (0.5 - b.margin + b.margin) - b.margin);
        // A large one keeps the default.
        let c =
            Convex::create(2, Vec3::new(1.0, 2.0, 7.0), WalkerBehaviour::BySlope, 0..0).unwrap();
        assert_eq!(c.margin, DEFAULT_MARGIN);
        assert_eq!(
            c.dims,
            [
                1.0 - DEFAULT_MARGIN,
                2.0 - DEFAULT_MARGIN,
                1.0 - DEFAULT_MARGIN
            ]
        );
        assert_eq!(c.kind, ConvexKind::Cylinder { up: Axis::Y });
    }

    #[test]
    fn a_sphere_and_a_capsule() {
        let mut s =
            Convex::create(0, Vec3::new(0.3, 9.0, 9.0), WalkerBehaviour::BySlope, 0..0).unwrap();
        s.set_scaling([0.5, 1.0, 1.0]);
        assert_eq!(
            (s.dims, s.margin, s.scaling),
            ([0.3, 0.0, 0.0], 0.3, [0.5, 1.0, 1.0])
        );
        let c =
            Convex::create(3, Vec3::new(0.2, 1.0, 0.0), WalkerBehaviour::BySlope, 0..0).unwrap();
        assert_eq!((c.dims, c.margin), ([0.2, 0.5, 0.2], DEFAULT_MARGIN));
    }

    #[test]
    fn from_vostok_flips_z_and_drops_negative_zeros() {
        let t = from_vostok(&create_rotation(Vec3::ZERO), Vec3::new(1.0, -0.0, 2.0));
        assert_eq!(t.basis, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        assert!(
            t.basis
                .iter()
                .flatten()
                .all(|v| v.to_bits() != (-0.0f32).to_bits())
        );
        assert_eq!(
            t.origin.map(f32::to_bits),
            [1.0f32, 0.0, -2.0].map(f32::to_bits)
        );
        // A turn about y: Bullet's basis is the map's turn seen with z negated.
        let r = object_transform(Vec3::new(0.0, 0.5, 0.0), Vec3::ZERO, Vec3::ONE);
        let m = unit_axes(create_rotation(Vec3::new(0.0, 0.5, 0.0)));
        assert_eq!(r.basis[0][2], -m[2][0]);
        assert_eq!(r.basis[2][0], -m[0][2]);
        assert_eq!(r.basis[1][1], m[1][1]);
        // An object's scale leaves unit axes (and only its rounding).
        let scaled = object_transform(
            Vec3::new(0.3, 0.5, -1.0),
            Vec3::ZERO,
            Vec3::new(1.3, 0.7, 2.0),
        );
        for c in 0..3 {
            let length = (0..3)
                .map(|r| scaled.basis[r][c] * scaled.basis[r][c])
                .sum::<f32>()
                .sqrt();
            assert!((length - 1.0).abs() < 1e-6, "{length}");
        }
    }

    #[test]
    fn the_cook_builds_lone_compound_and_mesh_shapes() {
        let row = |position: Vec3, faces: Range<u32>| PrimitiveRow {
            kind: 1,
            dim: Vec3::new(1.0, 1.0, 1.0),
            position,
            rotation: Vec3::new(0.0, 1.0, 0.0),
            walker: WalkerBehaviour::DontWalk,
            faces,
        };
        let scale = Vec3::new(2.0, 1.0, 0.5);
        let lone = Primitives::build(vec![row(Vec3::new(0.0, 5e-6, 0.0), 0..12)], scale).unwrap();
        let Primitives::Lone(convex) = &lone else {
            panic!("{lone:?}")
        };
        assert_eq!(convex.scaling, [2.0, 1.0, 0.5]);
        assert!(
            lone.with_mesh(Some(Mesh {
                scaling: mesh_scaling(scale),
                faces: 0..1
            }))
            .is_err()
        );

        let two = Primitives::build(
            vec![
                row(Vec3::new(1.0, 2.0, 3.0), 0..12),
                row(Vec3::ZERO, 12..24),
            ],
            scale,
        )
        .unwrap();
        let shape = two
            .with_mesh(Some(Mesh {
                scaling: mesh_scaling(scale),
                faces: 24..30,
            }))
            .unwrap();
        let Shape::Compound { scaling, children } = shape else {
            panic!()
        };
        assert_eq!(scaling, [2.0, 1.0, 0.5]);
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].transform.origin, [2.0, 2.0, -1.5]);
        let Part::Mesh(mesh) = &children[2].part else {
            panic!()
        };
        assert_eq!(mesh.scaling, [2.0, 1.0, -0.5]);
        assert_eq!(children[2].transform.basis, IDENTITY_ROTATION);
        assert!(matches!(
            Primitives::build(vec![], scale).unwrap().with_mesh(None),
            Ok(Shape::Empty)
        ));
    }
}
