//! The game's own collision objects, for the exe's cast ([`super::Collision::build_exe`]).
//!
//! The 0.26g client does not sweep its capsule against a soup of placed triangles. Its Bullet world holds one object
//! for each placement ([`crate::bullet`]), and `btCollisionWorld::objectQuerySingle` (0xA14E0) casts against each by
//! its shape (PROVEN, `docs/notes/exactness_rig.md` section 3d):
//! - a mesh (type 21): each triangle in the mesh's own frame, the object's transform as the cast's transform B
//!   (`processTriangle` 0xB8670, [`exe_cast::cast_triangle`]); so the cast's arithmetic runs on the mesh's local
//!   corners and the transform, not on placed corners, also for a mesh that is only moved;
//! - a box, a sphere, a cylinder or a capsule (type under 20): the convex shape itself with its own margin
//!   ([`exe_cast::cast_convex`]), not its drawn triangles: a box is rounded by its margin, a round shape is round;
//! - a compound (type 31): each child as above under `object * child` ([`exe_cast::compose`], 0xA2C30).
//!
//! Everything is kept in **Bullet's frame** (the map frame with z negated), as the game has it; a sweep comes in and
//! its contacts go out in the map frame (a sign flip of z, exact).
//!
//! What a sweep is cast against is every part whose box the swept shape's box meets; of a mesh, the triangles of the
//! tree's leaves that box meets, but for those wholly outside it by [`CLEAR_M`] (their casts are misses). The exe
//! narrows by Bullet's broadphase and each mesh's tree with the cast shape's box (a capsule's is wider than the
//! capsule by its margin, 0.04 m); a hit needs the shapes within 0.001 m, so both give the same hits. Their ORDER is
//! not the exe's (its broadphase's and its quantized tree's order is not ported): hits at equal fractions can be
//! taken in another order.
use std::collections::HashMap;

use super::exe_cast::{self, CastEnd};
use super::exe_gjk::{self, Axis, Transform, WorldShape};
use super::exe_ray::{self, ClosestRayCallback, ConvexRay, ConvexShape, NormalSpace, RayFlags, TriangleRay};
use super::{BULLET_TOLERANCE_M, Contact, Face, Shape, SmallRound, SolidId, Surface, exe_cast_of};
use crate::bullet::{self, ConvexKind};
use crate::math::Vec3;
use crate::{CollisionFilter, MapData, WalkerBehaviour};

/// 0xA7AB04: a round shape under this radius is a small one to the down sweep's second look (0x84887).
const SMALL_ROUND_M: f32 = f32::from_bits(0x3F4C_CCCD);
/// Parts or triangles a leaf of a tree holds at most.
const LEAF: usize = 4;
/// What a box worked out in `f32` is widened by, m.
const BOX_PAD_M: f32 = 1e-3;
/// A mesh's triangle wholly outside the swept shape's box grown by this much (m) is not cast: its cast is a miss.
///
/// The exe's cast ([`exe_cast::time_of_impact`]) reports a hit only where its detector's distance is at most 0.001 m,
/// and goes on to the penetration solver only where the shapes' cores are under 0.01 m apart. The detector's distance
/// is never under the true one but by rounding: its closest vector is a point of its simplex, so of the difference of
/// the two shapes. The shape is inside the swept box at every point of the way, so a triangle outside the grown box
/// is over 0.02 m from it there, and every round of the cast leaves by a miss. (Rounding in the test and in the
/// transform the cast steps the mesh by is under a millimetre for a map's coordinates.)
const CLEAR_M: f32 = 0.02;

type V3 = [f32; 3];
type Box3 = (V3, V3);

/// A point of the map frame in Bullet's, or back.
fn flip(v: V3) -> V3 {
    [v[0], v[1], -v[2]]
}

fn overlaps(a: &Box3, b: &Box3) -> bool {
    (0..3).all(|i| a.0[i] <= b.1[i] && a.1[i] >= b.0[i])
}

fn union(a: &Box3, b: &Box3) -> Box3 {
    ([0, 1, 2].map(|i| a.0[i].min(b.0[i])), [0, 1, 2].map(|i| a.1[i].max(b.1[i])))
}

/// A box grown by `by` on every side.
fn grown(b: &Box3, by: f32) -> Box3 {
    ([0, 1, 2].map(|i| b.0[i] - by), [0, 1, 2].map(|i| b.1[i] + by))
}

/// Whether a triangle of finite corners is wholly outside a box (both in one frame): one of the box's three axes, the
/// triangle's normal or the nine cross products of a box axis and an edge separates them (Akenine-Moller's
/// triangle-box test).
fn clear_of(v: &[V3; 3], b: &Box3) -> bool {
    let centre = [0, 1, 2].map(|i| (b.0[i] + b.1[i]) * 0.5);
    let half = [0, 1, 2].map(|i| (b.1[i] - b.0[i]) * 0.5);
    let p = v.map(|p| [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]]);
    for i in 0..3 {
        if p[0][i].min(p[1][i]).min(p[2][i]) > half[i] || p[0][i].max(p[1][i]).max(p[2][i]) < -half[i] {
            return true;
        }
    }
    let edge = |a: usize, b: usize| [p[b][0] - p[a][0], p[b][1] - p[a][1], p[b][2] - p[a][2]];
    let edges = [edge(0, 1), edge(1, 2), edge(2, 0)];
    let dot = |a: &V3, b: &V3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let reach = |a: &V3| half[0] * a[0].abs() + half[1] * a[1].abs() + half[2] * a[2].abs();
    let (e, f) = (&edges[0], &edges[1]);
    let normal = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
    if dot(&normal, &p[0]).abs() > reach(&normal) {
        return true;
    }
    for f in &edges {
        // The box's x, y and z axis crossed with the edge.
        for axis in [[0.0, -f[2], f[1]], [f[2], 0.0, -f[0]], [-f[1], f[0], 0.0]] {
            let along = [dot(&axis, &p[0]), dot(&axis, &p[1]), dot(&axis, &p[2])];
            let r = reach(&axis);
            if along[0].min(along[1]).min(along[2]) > r || along[0].max(along[1]).max(along[2]) < -r {
                return true;
            }
        }
    }
    false
}

#[derive(Clone, Copy, Debug)]
struct Node {
    bounds: Box3,
    /// A leaf: `first..first + count` of the order; else `first` is the right child (the left is the next node).
    first: u32,
    count: u32,
}

/// A tree of boxes: `order` lists the boxes' indices, each leaf a run of it.
#[derive(Debug, Default)]
struct Tree {
    nodes: Vec<Node>,
    order: Vec<u32>,
}

impl Tree {
    fn build(boxes: &[Box3]) -> Tree {
        let mut tree = Tree { nodes: Vec::with_capacity(2 * boxes.len() / LEAF + 1), order: (0..boxes.len() as u32).collect() };
        if !boxes.is_empty() {
            tree.split(boxes, 0, boxes.len());
        }
        tree
    }

    fn split(&mut self, boxes: &[Box3], first: usize, end: usize) -> usize {
        let bounds = self.order[first..end].iter().map(|&i| boxes[i as usize]).reduce(|a, b| union(&a, &b)).expect("a run of boxes");
        let me = self.nodes.len();
        self.nodes.push(Node { bounds, first: first as u32, count: (end - first) as u32 });
        if end - first <= LEAF {
            return me;
        }
        let size = [0, 1, 2].map(|i| bounds.1[i] - bounds.0[i]);
        let axis = if size[0] >= size[1] && size[0] >= size[2] { 0 } else if size[1] >= size[2] { 1 } else { 2 };
        let centre = |i: &u32| boxes[*i as usize].0[axis] + boxes[*i as usize].1[axis];
        self.order[first..end].sort_by(|a, b| centre(a).total_cmp(&centre(b)));
        let mid = first + (end - first) / 2;
        self.split(boxes, first, mid);
        let right = self.split(boxes, mid, end);
        self.nodes[me].first = right as u32;
        self.nodes[me].count = 0;
        me
    }

    /// Calls `each` with the index of every box in a leaf whose bounds meet `query`.
    fn each_in(&self, query: &Box3, mut each: impl FnMut(usize)) {
        if self.nodes.is_empty() {
            return;
        }
        // A node's two halves differ by one item at most, so the tree of 2^32 items is under 40 deep, and the walk
        // holds one node a level and one more.
        let mut stack = [0u32; 64];
        let mut held = 1;
        while held > 0 {
            held -= 1;
            let i = stack[held] as usize;
            let node = &self.nodes[i];
            if !overlaps(&node.bounds, query) {
                continue;
            }
            if node.count > 0 {
                for &item in &self.order[node.first as usize..(node.first + node.count) as usize] {
                    each(item as usize);
                }
            } else {
                stack[held] = i as u32 + 1;
                stack[held + 1] = node.first;
                held += 2;
            }
        }
    }

    fn bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<Node>() + self.order.capacity() * std::mem::size_of::<u32>()
    }
}

/// A mesh's triangle in the mesh's own frame (Bullet's: the library's corner with z negated, the bits the game's
/// cast takes, [`crate::bullet`]). Its corners are finite numbers ([`MeshTriangle::new`]).
#[derive(Clone, Copy, Debug)]
struct MeshTriangle {
    v: [V3; 3],
    walker: WalkerBehaviour,
}

impl MeshTriangle {
    /// The triangle of these corners; none with a corner that is not a finite number.
    fn new(v: [V3; 3], walker: WalkerBehaviour) -> Option<MeshTriangle> {
        v.iter().flatten().all(|c| c.is_finite()).then_some(MeshTriangle { v, walker })
    }
}

/// A library's mesh, shared by its placements.
#[derive(Debug)]
struct Mesh {
    triangles: Vec<MeshTriangle>,
    tree: Tree,
}

/// A library's convex primitive, shared by its placements.
#[derive(Debug)]
struct Primitive {
    shape: WorldShape,
    /// The margin the detector takes for it (`getMargin`: a sphere's is its radius times its scaling).
    margin: f32,
    /// The half sides of a box around it in its own frame, margin in.
    half: V3,
    /// The shape as the ray's cast reads it (its class's own support and box); none for a cylinder that is not y-up
    /// (the game makes none: `create_bt_primitive` 0x7A1F0), which a ray does not meet here.
    ray: Option<ConvexShape>,
}

#[derive(Clone, Copy, Debug)]
enum Part {
    Mesh(u32),
    Primitive {
        shape: u32,
        /// What a contact with it is on ([`Surface::Primitive`]).
        on: Surface,
    },
}

/// One thing a sweep is cast against: a part under its world transform in Bullet's frame (the object's, or the
/// object's times a compound's child's).
#[derive(Clone, Copy, Debug)]
struct Leaf {
    transform: Transform,
    /// The same transform as the ray test composes it (`RayTester::Process` 0xA10B0 groups a child's origin x
    /// differently from the sweep's 0xA2C30).
    ray_transform: Transform,
    part: Part,
    /// The object it is a part of (an index into the objects).
    object: u32,
}

/// The game's objects a walking body meets.
#[derive(Debug, Default)]
pub(super) struct Solids {
    leaves: Vec<Leaf>,
    /// Each leaf's box in the world (Bullet's frame).
    boxes: Vec<Box3>,
    tree: Tree,
    meshes: Vec<Mesh>,
    primitives: Vec<Primitive>,
    /// Each object's leaves (a run of the leaves, a compound's children in the compound's order).
    objects: Vec<std::ops::Range<u32>>,
}

/// The box of a box of half sides `half` about `centre` (in a part's frame) under `transform`.
fn world_box(transform: &Transform, centre: V3, half: V3) -> Box3 {
    let b = &transform.basis;
    let c = [0, 1, 2].map(|r| (b[r][0] * centre[0] + b[r][1] * centre[1]) + b[r][2] * centre[2] + transform.origin[r]);
    let e = [0, 1, 2].map(|r| (b[r][0].abs() * half[0] + b[r][1].abs() * half[1]) + b[r][2].abs() * half[2] + BOX_PAD_M);
    ([0, 1, 2].map(|i| c[i] - e[i]), [0, 1, 2].map(|i| c[i] + e[i]))
}

/// The box, in the frame of `transform`, of a box of the world.
fn local_box(transform: &Transform, world: &Box3) -> Box3 {
    let b = &transform.basis;
    let centre = [0, 1, 2].map(|i| (world.0[i] + world.1[i]) * 0.5 - transform.origin[i]);
    let half = [0, 1, 2].map(|i| (world.1[i] - world.0[i]) * 0.5);
    // The inverse of a rotation is its transpose.
    let c = [0, 1, 2].map(|k| (b[0][k] * centre[0] + b[1][k] * centre[1]) + b[2][k] * centre[2]);
    let e = [0, 1, 2].map(|k| (b[0][k].abs() * half[0] + b[1][k].abs() * half[1]) + b[2][k].abs() * half[2] + BOX_PAD_M);
    ([0, 1, 2].map(|i| c[i] - e[i]), [0, 1, 2].map(|i| c[i] + e[i]))
}

/// Whether a mesh's triangle (`v`, in the mesh's frame) under `transform` is wholly outside the box `near` of the
/// world; `near_local` is a box of the mesh's frame that holds `near`.
fn out_of_reach(transform: &Transform, triangle: &MeshTriangle, near: &Box3, near_local: &Box3) -> bool {
    let v = &triangle.v;
    if (0..3).any(|i| {
        v[0][i].min(v[1][i]).min(v[2][i]) > near_local.1[i] || v[0][i].max(v[1][i]).max(v[2][i]) < near_local.0[i]
    }) {
        return true;
    }
    let b = &transform.basis;
    let placed = v.map(|p| [0, 1, 2].map(|r| (b[r][0] * p[0] + b[r][1] * p[1]) + b[r][2] * p[2] + transform.origin[r]));
    clear_of(&placed, near)
}

/// A triangle of a mesh placed in the map frame, as a face (its normal zero for a degenerate one).
fn placed(transform: &Transform, triangle: &MeshTriangle) -> Face {
    let b = &transform.basis;
    let v = triangle.v.map(|p| {
        let w = [0, 1, 2].map(|r| (b[r][0] * p[0] + b[r][1] * p[1]) + b[r][2] * p[2] + transform.origin[r]);
        Vec3::from_array(flip(w))
    });
    face_of(v, triangle.walker)
}

fn face_of(v: [Vec3; 3], walker: WalkerBehaviour) -> Face {
    let n = (v[1] - v[0]).cross(v[2] - v[0]);
    let l = n.length();
    Face { v, normal: if l > 1e-12 { n * (1.0 / l) } else { Vec3::ZERO }, walker }
}

impl Primitive {
    fn new(convex: &bullet::Convex) -> Primitive {
        let (dims, m) = (convex.dims, convex.margin);
        let (shape, margin, half, ray) = match convex.kind {
            ConvexKind::Box => {
                (WorldShape::Box { half_extents: dims }, m, dims.map(|d| d + m), Some(ConvexShape::Box { implicit: dims, margin: m }))
            }
            ConvexKind::Sphere => {
                // `btSphereShape::getMargin`: the radius (+20h) times the scaling (+10h).
                let ray = ConvexShape::sphere(dims[0], convex.scaling[0]);
                let radius = dims[0] * convex.scaling[0];
                (WorldShape::Sphere, radius, [radius; 3], Some(ray))
            }
            ConvexKind::Cylinder { up } => {
                let ray = (up == Axis::Y).then_some(ConvexShape::CylinderY { implicit: dims, margin: m });
                (WorldShape::cylinder(dims, up), m, dims.map(|d| d + m), ray)
            }
            ConvexKind::Capsule { up } => {
                // The support reaches the radius times the scaling from the axis' ends (0x8D9BB).
                let radius = dims[(up.index() + 2) % 3];
                let reach = radius * convex.scaling.iter().fold(1.0f32, |a, &s| a.max(s));
                let mut half = [reach; 3];
                half[up.index()] = dims[up.index()] + reach;
                let ray = ConvexShape::Capsule { implicit: dims, scaling: convex.scaling, margin: m, up };
                (WorldShape::capsule(dims, convex.scaling, m, up), m, half, Some(ray))
            }
        };
        Primitive { shape, margin, half, ray }
    }
}

/// What 0x84810 reads of a round primitive under `rotation`, the matrix 0x84190 and `setRotation` 0x72A50 make of the
/// hit (for a whole shape [`exe_cast::through_quaternion`] of the object's basis, for a compound's child
/// [`exe_cast::child_hit_rotation`]): [`SmallRound`].
fn small_round(convex: &bullet::Convex, rotation: &[[f32; 3]; 3]) -> Option<SmallRound> {
    let radius = match convex.kind {
        ConvexKind::Box => return None,
        // 0x84875: the radius (+20h) times the scaling's x (+10h).
        ConvexKind::Sphere => convex.dims[0] * convex.scaling[0],
        // 0x8485F: the implicit dimension on (up + 2) % 3.
        ConvexKind::Capsule { up } => convex.dims[(up.index() + 2) % 3],
        // 0x8484C: `getRadius` (vtable +54h, 0xA9BC0): the half extents with the margin, x.
        ConvexKind::Cylinder { .. } => convex.dims[0] + convex.margin,
    };
    // 0x8487F..0x8488A (`comiss 0.8, radius; jna`: small only when 0.8 is over it).
    let small = SMALL_ROUND_M > radius;
    if !small {
        return None;
    }
    // 0x848C7..0x8496C with the tick's up (0, 1, 0): up . (M up) is M's middle element. For a compound's child M is
    // `R_child R_object` (0x84190), not the child's world rotation: on the eight maps the two decide a lying or
    // standing child differently for 317 of the 4,933 small round children the walker meets (312 on Mamayev Kurgan).
    let upright = || rotation[1][1].abs();
    Some(match convex.kind {
        ConvexKind::Box => return None,
        ConvexKind::Sphere => SmallRound::Sphere,
        ConvexKind::Capsule { .. } => SmallRound::Capsule { upright: upright() },
        ConvexKind::Cylinder { .. } => SmallRound::Cylinder { upright: upright() },
    })
}

/// The parts of a world as they are put together.
#[derive(Default)]
struct Builder {
    solids: Solids,
}

impl Builder {
    fn mesh(&mut self, triangles: Vec<MeshTriangle>) -> u32 {
        let boxes: Vec<Box3> = triangles
            .iter()
            .map(|t| {
                let lo = [0, 1, 2].map(|i| t.v[0][i].min(t.v[1][i]).min(t.v[2][i]));
                let hi = [0, 1, 2].map(|i| t.v[0][i].max(t.v[1][i]).max(t.v[2][i]));
                (lo, hi)
            })
            .collect();
        self.solids.meshes.push(Mesh { tree: Tree::build(&boxes), triangles });
        (self.solids.meshes.len() - 1) as u32
    }

    fn primitive(&mut self, convex: &bullet::Convex) -> u32 {
        self.solids.primitives.push(Primitive::new(convex));
        (self.solids.primitives.len() - 1) as u32
    }

    /// A new object: the parts placed until the next one are its.
    fn object(&mut self) {
        let at = self.solids.leaves.len() as u32;
        self.solids.objects.push(at..at);
    }

    fn leaf(&mut self, bounds: Box3, transform: Transform, ray_transform: Transform, part: Part) {
        let object = self.solids.objects.last_mut().expect("an object begun before its parts");
        object.end += 1;
        let object = (self.solids.objects.len() - 1) as u32;
        self.solids.boxes.push(bounds);
        self.solids.leaves.push(Leaf { transform, ray_transform, part, object });
    }

    /// A mesh under `transform` (the sweep's) and `ray_transform` (the ray test's: the same but for a compound's child).
    fn place_mesh(&mut self, transform: Transform, ray_transform: Transform, mesh: u32) {
        let m = &self.solids.meshes[mesh as usize];
        let Some(root) = m.tree.nodes.first() else { return };
        let (lo, hi) = root.bounds;
        let centre = [0, 1, 2].map(|i| (lo[i] + hi[i]) * 0.5);
        let half = [0, 1, 2].map(|i| (hi[i] - lo[i]) * 0.5);
        self.leaf(world_box(&transform, centre, half), transform, ray_transform, Part::Mesh(mesh));
    }

    fn place_primitive(&mut self, transform: Transform, ray_transform: Transform, shape: u32, on: Surface) {
        let half = self.solids.primitives[shape as usize].half;
        self.leaf(world_box(&transform, [0.0; 3], half), transform, ray_transform, Part::Primitive { shape, on });
    }

    fn finish(mut self) -> Solids {
        self.solids.tree = Tree::build(&self.solids.boxes);
        self.solids
    }
}

impl Solids {
    /// The objects a walking body meets in `map` (its placements the walker's filter meets, each with the library
    /// [`MapData::libraries_met`] names); `None` for a map without the game's shapes.
    pub(super) fn build(map: &MapData) -> Option<Solids> {
        let world = map.bullet.as_ref()?;
        let filter = CollisionFilter::WALKER;
        let met = map.libraries_met(filter);
        let mut builder = Builder::default();
        // A library's parts, made once: (library, part) to the mesh or the primitive.
        let mut meshes: HashMap<(u32, usize), u32> = HashMap::new();
        let mut primitives: HashMap<(u32, usize), u32> = HashMap::new();
        for (index, placement) in map.placements.iter().enumerate() {
            if !filter.meets(placement.cgroup, placement.cmask) {
                continue;
            }
            let library_index = met[placement.library as usize];
            let library = &map.libraries[library_index as usize];
            let corner = |i: u32| library.vertices[i as usize];
            let mut mesh_of = |builder: &mut Builder, part: usize, mesh: &bullet::Mesh| {
                *meshes.entry((library_index, part)).or_insert_with(|| {
                    let triangles = library.faces[mesh.faces.start as usize..mesh.faces.end as usize]
                        .iter()
                        .filter_map(|face| MeshTriangle::new(face.corners.map(|i| flip(corner(i).to_array())), face.surface.walker))
                        .collect();
                    builder.mesh(triangles)
                })
            };
            let mut primitive_of = |builder: &mut Builder, part: usize, convex: &bullet::Convex| {
                *primitives.entry((library_index, part)).or_insert_with(|| {
                    builder.primitive(convex)
                })
            };
            let object = *world.object(index);
            builder.object();
            match world.shape(library_index) {
                bullet::Shape::Empty => {}
                bullet::Shape::Mesh(mesh) => {
                    let id = mesh_of(&mut builder, 0, mesh);
                    builder.place_mesh(object, object, id);
                }
                // An object's whole shape: the callback gets no shape info and takes the walker behaviour 0.
                bullet::Shape::Convex(convex) => {
                    let id = primitive_of(&mut builder, 0, convex);
                    let on = Surface::Primitive { walker: WalkerBehaviour::BySlope, round: small_round(convex, &exe_cast::through_quaternion(&object.basis)) };
                    builder.place_primitive(object, object, id, on);
                }
                bullet::Shape::Compound { children, .. } => {
                    for (part, child) in children.iter().enumerate() {
                        let transform = exe_cast::compose(&object, &child.transform);
                        let ray_transform = exe_ray::compose_child(&object, &child.transform);
                        match &child.part {
                            bullet::Part::Mesh(mesh) => {
                                let id = mesh_of(&mut builder, part, mesh);
                                builder.place_mesh(transform, ray_transform, id);
                            }
                            bullet::Part::Convex(convex) => {
                                let id = primitive_of(&mut builder, part, convex);
                                let on = Surface::Primitive { walker: convex.walker, round: small_round(convex, &exe_cast::child_hit_rotation(&object.basis, &child.transform.basis)) };
                                builder.place_primitive(transform, ray_transform, id, on);
                            }
                        }
                    }
                }
            }
        }
        Some(builder.finish())
    }

    pub(super) fn bytes(&self) -> usize {
        self.leaves.capacity() * std::mem::size_of::<Leaf>()
            + self.boxes.capacity() * std::mem::size_of::<Box3>()
            + self.tree.bytes()
            + self.meshes.iter().map(|m| m.triangles.capacity() * std::mem::size_of::<MeshTriangle>() + m.tree.bytes()).sum::<usize>()
            + self.primitives.len() * std::mem::size_of::<Primitive>()
    }

    /// Every part's first contact with `shape` moved by `by` (both in the map frame), as `objectQuerySingle` 0xA14E0
    /// reports it to a callback that takes every hit (the sweep's allowed penetration 0, `convexSweepTest` 0xA34E8).
    pub(super) fn sweep_each(&self, shape: Shape, by: Vec3, mut each: impl FnMut(Contact)) {
        let mut cast = exe_cast_of(shape, by);
        cast.from = flip(cast.from);
        cast.by = flip(cast.by);
        let (lo0, hi0) = shape.aabb();
        let (lo1, hi1) = shape.moved(by).aabb();
        let pad = Vec3::new(BULLET_TOLERANCE_M, BULLET_TOLERANCE_M, BULLET_TOLERANCE_M);
        let (lo, hi) = (lo0.min(lo1) - pad, hi0.max(hi1) + pad);
        let swept: Box3 = ([lo.x, lo.y, -hi.z], [hi.x, hi.y, -lo.z]);
        let near = grown(&swept, CLEAR_M);
        self.tree.each_in(&swept, |i| {
            if !overlaps(&self.boxes[i], &swept) {
                return;
            }
            let leaf = &self.leaves[i];
            let object = Some(SolidId(leaf.object));
            let out = |fraction: f32, normal: V3, point: V3, on: Surface| Contact {
                t: fraction,
                point: Vec3::from_array(flip(point)),
                normal: Vec3::from_array(flip(normal)),
                on,
                object,
            };
            match leaf.part {
                Part::Primitive { shape: id, on } => {
                    let primitive = &self.primitives[id as usize];
                    match exe_cast::cast_convex(&cast, &primitive.shape, primitive.margin, &leaf.transform, 0.0, 1.0) {
                        CastEnd::Hit { fraction, normal, point } => each(out(fraction, normal, point, on)),
                        CastEnd::Miss => {}
                    }
                }
                Part::Mesh(id) => {
                    let mesh = &self.meshes[id as usize];
                    let query = local_box(&leaf.transform, &swept);
                    let near_local = grown(&query, CLEAR_M);
                    mesh.tree.each_in(&query, |k| {
                        let triangle = &mesh.triangles[k];
                        if out_of_reach(&leaf.transform, triangle, &near, &near_local) {
                            return;
                        }
                        // A mesh's margin is 0 (the game's `btBvhTriangleMeshShape`), the callback's fraction 1.
                        match exe_cast::cast_triangle(&cast, &exe_gjk::Triangle(triangle.v), 0.0, &leaf.transform, 0.0, 1.0) {
                            CastEnd::Hit { fraction, normal, point } => {
                                let face = placed(&leaf.transform, triangle);
                                each(out(fraction, normal, point, Surface::Face { walker: face.walker, normal: face.normal }));
                            }
                            CastEnd::Miss => {}
                        }
                    });
                }
            }
        });
    }
}

impl Solids {
    /// `btCollisionWorld::rayTestSingle` 0xA02D0 on one object, as the walk tick's second looks call it: a ray from `a`
    /// to `b` (the map frame; identity bases) against the object's whole shape under its transform, into a fresh
    /// closest-ray callback (0x83FD0, its flags 0). The normal the callback holds after, in the map frame.
    ///
    /// A compound's children and a mesh's triangles are offered where their boxes meet the ray's box, in this
    /// world's order (the exe's own trees' walks, 0xA54A0 and 0xC39B0, are not ported): a hit needs the ray through
    /// the box, so the hits are the exe's; which of two at exactly one fraction comes first can differ.
    pub(super) fn look_ray(&self, object: u32, a: Vec3, b: Vec3) -> Option<Vec3> {
        let end = |p: Vec3| Transform { basis: exe_cast::IDENTITY.basis, origin: flip(p.to_array()) };
        let (from, to) = (end(a), end(b));
        let bounds: Box3 = (
            [0, 1, 2].map(|i| from.origin[i].min(to.origin[i]) - BOX_PAD_M),
            [0, 1, 2].map(|i| from.origin[i].max(to.origin[i]) + BOX_PAD_M),
        );
        let mut callback = ClosestRayCallback::new(from.origin, to.origin);
        for i in self.objects[object as usize].clone() {
            if !overlaps(&self.boxes[i as usize], &bounds) {
                continue;
            }
            let leaf = &self.leaves[i as usize];
            let world = &leaf.ray_transform;
            match leaf.part {
                Part::Primitive { shape, .. } => {
                    let Some(convex) = self.primitives[shape as usize].ray else { continue };
                    if let ConvexRay::Hit { fraction, normal } = exe_ray::ray_convex(&from, &to, &convex, world, callback.closest).answer {
                        callback.add_single_result(object, normal, fraction, NormalSpace::World);
                    }
                }
                Part::Mesh(id) => {
                    let mesh = &self.meshes[id as usize];
                    let (local_from, local_to) = exe_ray::mesh_ray(from.origin, to.origin, world);
                    let local: Box3 = (
                        [0, 1, 2].map(|k| local_from[k].min(local_to[k]) - BOX_PAD_M),
                        [0, 1, 2].map(|k| local_from[k].max(local_to[k]) + BOX_PAD_M),
                    );
                    // `rcb.m_hitFraction = resultCallback.m_closestHitFraction` (0xA09A4), then each report's answer.
                    let mut hit_fraction = callback.closest;
                    mesh.tree.each_in(&local, |k| {
                        let triangle = &mesh.triangles[k];
                        if let TriangleRay::Hit { normal, distance } = exe_ray::ray_triangle(local_from, local_to, RayFlags(0), hit_fraction, &triangle.v) {
                            let n = exe_ray::bridge_normal(&world.basis, normal);
                            hit_fraction = callback.add_single_result(object, n, distance, NormalSpace::World);
                        }
                    });
                }
            }
        }
        callback.hit.map(|hit| Vec3::from_array(flip(hit.normal)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::exe_cast::IDENTITY;
    use crate::controller::{Caster, Collision, Sweeps};
    use crate::project::Triangle;

    fn world(solids: Solids) -> Collision {
        Collision { sweeps: Sweeps::Solids(Box::new(solids)), ..Collision::build(&[]) }
    }

    fn capsule(x: f32, y: f32, z: f32) -> Shape {
        Shape::CapsuleAt { centre: Vec3::new(x, y, z), radius: 0.38, half_height: 0.52 }
    }

    fn at(x: f32, y: f32, z: f32) -> Transform {
        Transform { basis: IDENTITY.basis, origin: [x, y, z] }
    }

    fn hits(world: &Collision, shape: Shape, by: Vec3) -> Vec<Contact> {
        let mut out = Vec::new();
        world.sweep_each(shape, by, |c| out.push(c));
        out.sort_by(|a, b| a.t.total_cmp(&b.t));
        out
    }

    /// A floor of two triangles in the map frame, off the axes so that a flipped z would show.
    fn floor() -> Vec<Triangle> {
        let p = |x: f32, z: f32| Vec3::new(x, 0.25, z);
        let tri = |a, b, c| Triangle { v: [a, b, c], walker: WalkerBehaviour::Walk };
        vec![tri(p(-4.0, 1.0), p(6.0, 1.0), p(6.0, 9.0)), tri(p(-4.0, 1.0), p(6.0, 9.0), p(-4.0, 9.0))]
    }

    fn mesh_of(triangles: &[Triangle]) -> Vec<MeshTriangle> {
        triangles.iter().map(|t| MeshTriangle::new(t.v.map(|p| flip(p.to_array())), t.walker).expect("finite corners")).collect()
    }

    /// A mesh under the identity is met with the numbers the same triangles get as placed faces by the exe's caster:
    /// the frames' sign flip is exact both ways.
    #[test]
    fn a_mesh_under_the_identity_is_met_as_its_placed_triangles_are() {
        let mut builder = Builder::default();
        let mesh = builder.mesh(mesh_of(&floor()));
        builder.object();
        builder.place_mesh(IDENTITY, IDENTITY, mesh);
        let shapes = world(builder.finish());
        let soup = Collision::build(&floor()).with_caster(Caster::Exe);
        for (shape, by) in [
            (capsule(1.0, 3.0, 4.0), Vec3::new(0.25, -3.0, 0.5)),
            (capsule(5.5, 2.0, 8.5), Vec3::new(0.0, -2.0, 0.0)),
            (Shape::Sphere { centre: Vec3::new(0.0, 1.0, 2.0), radius: 0.3 }, Vec3::new(1.0, -1.0, 1.0)),
        ] {
            let (a, b) = (hits(&shapes, shape, by), hits(&soup, shape, by));
            assert!(!a.is_empty(), "{shape:?}");
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(&b) {
                // Equal as numbers: a zero's sign is all the flip can change.
                assert_eq!(a.t.to_bits(), b.t.to_bits());
                assert_eq!(a.point.to_array(), b.point.to_array());
                assert_eq!(a.normal.to_array(), b.normal.to_array());
                assert_eq!(a.on.walker(), WalkerBehaviour::Walk);
            }
        }
    }

    /// The same floor as a mesh about its own origin, moved and turned a quarter about up by its object's transform,
    /// is met where the placed floor is (not to the bit: the cast runs on the local corners).
    #[test]
    fn a_moved_and_turned_mesh_is_met_where_it_stands() {
        // Local (Bullet's frame) to world: x' = z + 1, z' = -x - 5; the floor's corners worked back.
        let quarter = Transform { basis: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]], origin: [1.0, 0.0, -5.0] };
        let local: Vec<MeshTriangle> = mesh_of(&floor())
            .iter()
            .map(|t| MeshTriangle::new(t.v.map(|w| [-(w[2] + 5.0), w[1], w[0] - 1.0]), t.walker).expect("finite corners"))
            .collect();
        let mut builder = Builder::default();
        let mesh = builder.mesh(local);
        builder.object();
        builder.place_mesh(quarter, quarter, mesh);
        let shapes = world(builder.finish());
        let soup = Collision::build(&floor()).with_caster(Caster::Exe);
        let (shape, by) = (capsule(1.0, 3.0, 4.0), Vec3::new(0.25, -3.0, 0.5));
        let (a, b) = (hits(&shapes, shape, by), hits(&soup, shape, by));
        assert!(!a.is_empty() && a.len() == b.len(), "{a:?} {b:?}");
        assert!((a[0].t - b[0].t).abs() < 1e-5, "{} {}", a[0].t, b[0].t);
        assert!((a[0].normal - b[0].normal).length() < 1e-4 && a[0].normal.y > 0.99, "{:?}", a[0].normal);
        // Outside the floor nothing is met.
        assert!(hits(&shapes, capsule(-8.0, 3.0, 4.0), Vec3::new(0.0, -3.0, 0.0)).is_empty());
    }

    fn block() -> bullet::Convex {
        bullet::Convex::create(1, Vec3::new(1.0, 0.5, 1.0), WalkerBehaviour::DontWalk, 0..0).expect("a box")
    }

    /// A box is met as the box: a capsule dropped on it stops the touching distance over its top, and one dropped
    /// past its edge by less than the box's margin still lands on its rounded edge lower down. What it is on is the
    /// primitive, with the walker behaviour its placement gives.
    #[test]
    fn a_box_is_met_as_a_rounded_box() {
        let mut builder = Builder::default();
        let id = builder.primitive(&block());
        let on = Surface::Primitive { walker: WalkerBehaviour::BySlope, round: None };
        // The box's centre at map (2, 0.5, 3): Bullet's z is -3.
        builder.object();
        builder.place_primitive(at(2.0, 0.5, -3.0), at(2.0, 0.5, -3.0), id, on);
        let shapes = world(builder.finish());
        let top = hits(&shapes, capsule(2.0, 3.0, 3.0), Vec3::new(0.0, -3.0, 0.0));
        assert_eq!(top.len(), 1);
        // The capsule's bottom is 0.9 under its centre: from 3 down to 1 + 0.9 is 1.1 of 3.
        assert!((top[0].t - 1.1 / 3.0).abs() < 1e-3, "{}", top[0].t);
        assert!(top[0].normal.y > 0.999 && (top[0].point.y - 1.0).abs() < 2e-3 && (top[0].point.z - 3.0).abs() < 1e-3, "{:?}", top[0]);
        assert_eq!(top[0].on, on);
        // The second looks' ray meets the box it names, with the face's normal: down onto the top, and from the side.
        let top_normal = shapes.look_ray(top[0].object, Vec3::new(2.0, 2.0, 3.0), Vec3::new(2.0, 0.5, 3.0)).expect("the top");
        assert!(top_normal.y > 0.999, "{top_normal:?}");
        let side = shapes.look_ray(top[0].object, Vec3::new(2.0, 0.5, 6.0), Vec3::new(2.0, 0.5, 3.0)).expect("the side");
        assert!(side.z > 0.999, "{side:?}");
        assert_eq!(shapes.look_ray(top[0].object, Vec3::new(9.0, 2.0, 3.0), Vec3::new(9.0, 0.0, 3.0)), None);
        // 0.37 m past the edge along z: the capsule's side (radius 0.38) still meets the edge, rounded by the margin.
        let edge = hits(&shapes, capsule(2.0, 3.0, 4.37), Vec3::new(0.0, -3.0, 0.0));
        assert_eq!(edge.len(), 1);
        assert!(edge[0].t > top[0].t + 0.05 && edge[0].normal.z > 0.9, "{:?}", edge[0]);
        // And 0.39 m past it nothing.
        assert!(hits(&shapes, capsule(2.0, 3.0, 4.39), Vec3::new(0.0, -3.0, 0.0)).is_empty());
    }

    /// What the down sweep's second look reads of a round shape: small ones only, with how upright they stand.
    #[test]
    fn small_round_shapes_are_told_from_the_rest() {
        let make = |kind: i32, dim: Vec3| bullet::Convex::create(kind, dim, WalkerBehaviour::BySlope, 0..0).expect("a primitive");
        let lying = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(small_round(&block(), &IDENTITY.basis), None);
        assert_eq!(small_round(&make(0, Vec3::new(0.5, 0.0, 0.0)), &IDENTITY.basis), Some(SmallRound::Sphere));
        assert_eq!(small_round(&make(0, Vec3::new(0.8, 0.0, 0.0)), &IDENTITY.basis), None);
        assert_eq!(small_round(&make(2, Vec3::new(0.3, 1.0, 0.3)), &IDENTITY.basis), Some(SmallRound::Cylinder { upright: 1.0 }));
        // A cylinder's radius is its half extent (the implicit dimension and the margin).
        assert_eq!(small_round(&make(2, Vec3::new(0.85, 1.0, 0.85)), &IDENTITY.basis), None);
        let Some(SmallRound::Capsule { upright }) = small_round(&make(3, Vec3::new(0.2, 1.0, 0.2)), &lying) else { panic!() };
        assert!(upright < 1e-6, "{upright}");
    }

    /// Numbers in [0, 1) that are the same every run.
    struct Dice(u64);

    impl Dice {
        fn roll(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 40) as f32 / (1u64 << 24) as f32
        }

        fn within(&mut self, lo: f32, hi: f32) -> f32 {
            lo + (hi - lo) * self.roll()
        }

        fn point(&mut self, reach: f32) -> V3 {
            [self.within(-reach, reach), self.within(-reach, reach), self.within(-reach, reach)]
        }
    }

    /// A triangle the test calls clear of a box has no point in it; and the test tells a triangle that passes a
    /// corner of the box from one that cuts it, though the boxes of both meet the box.
    #[test]
    fn a_triangle_clear_of_a_box_has_no_point_in_it() {
        let mut dice = Dice(7);
        let (mut clear, mut near) = (0, 0);
        for round in 0..4000 {
            let centre = dice.point(1.0);
            let half = [dice.within(0.05, 1.0), dice.within(0.05, 1.0), dice.within(0.05, 1.0)];
            let b: Box3 = ([0, 1, 2].map(|i| centre[i] - half[i]), [0, 1, 2].map(|i| centre[i] + half[i]));
            // Small triangles about the box, and large ones that pass it.
            let size = if round % 2 == 0 { 0.8 } else { 6.0 };
            let about = dice.point(1.2);
            let v = [0, 1, 2].map(|_| {
                let p = dice.point(size);
                [about[0] + p[0], about[1] + p[1], about[2] + p[2]]
            });
            if !clear_of(&v, &b) {
                near += 1;
                continue;
            }
            clear += 1;
            const N: usize = 48;
            for i in 0..=N {
                for j in 0..=N - i {
                    let (s, t) = (i as f32 / N as f32, j as f32 / N as f32);
                    let p = [0, 1, 2].map(|k| v[0][k] + (v[1][k] - v[0][k]) * s + (v[2][k] - v[0][k]) * t);
                    assert!(!(0..3).all(|k| p[k] >= b.0[k] && p[k] <= b.1[k]), "{v:?} is called clear of {b:?}, and {p:?} is in it");
                }
            }
        }
        assert!(clear > 500 && near > 500, "{clear} clear, {near} near");

        let cube: Box3 = ([-1.0; 3], [1.0; 3]);
        // The plane x + y + z = d cut to a triangle: the cube's corner (1, 1, 1) is at 3.
        let across = |d: f32| [[d, 0.0, 0.0], [0.0, d, 0.0], [0.0, 0.0, d]];
        assert!(clear_of(&across(3.5), &cube));
        assert!(!clear_of(&across(2.5), &cube));
        let far = [[5.0, 0.0, 0.0], [6.0, 0.0, 0.0], [5.0, 1.0, 0.0]];
        assert!(clear_of(&far, &cube));
        // A mesh holds no triangle with a corner that is not a finite number (the test would drop a NaN).
        assert!(MeshTriangle::new(far, WalkerBehaviour::Walk).is_some());
        for bad in [f32::NAN, f32::INFINITY] {
            assert!(MeshTriangle::new([[bad, 0.0, 0.0], far[1], far[2]], WalkerBehaviour::Walk).is_none());
            assert!(MeshTriangle::new([far[0], far[1], [5.0, 1.0, bad]], WalkerBehaviour::Walk).is_none());
        }
    }

    /// A sweep over a turned and moved mesh reports the hits that casting every one of its triangles gives, to the
    /// bit: the triangles left out of a sweep's casts are misses.
    #[test]
    fn a_sweep_reports_the_hits_of_casting_every_triangle() {
        let mut dice = Dice(11);
        let triangles: Vec<MeshTriangle> = (0..400)
            .map(|k| {
                // Mostly small faces, every tenth a large one.
                let (about, size) = (dice.point(6.0), if k % 10 == 0 { 8.0 } else { 1.2 });
                let v = [0, 1, 2].map(|_| {
                    let p = dice.point(size);
                    [about[0] + p[0], about[1] + p[1], about[2] + p[2]]
                });
                MeshTriangle::new(v, WalkerBehaviour::Walk).expect("finite corners")
            })
            .collect();
        let (sin, cos) = 0.6f32.sin_cos();
        let transform = Transform { basis: [[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]], origin: [1.5, -0.5, 2.0] };
        let mut builder = Builder::default();
        let mesh = builder.mesh(triangles.clone());
        builder.object();
        builder.place_mesh(transform, transform, mesh);
        let shapes = world(builder.finish());

        let key = |t: f32, p: [f32; 3], n: [f32; 3]| (t.to_bits(), p.map(f32::to_bits), n.map(f32::to_bits));
        let (mut sweeps, mut found) = (0, 0);
        for round in 0..1500 {
            let centre = Vec3::from_array(dice.point(7.0));
            let by = Vec3::from_array(dice.point(if round % 3 == 0 { 2.5 } else { 0.4 }));
            let shape = match round % 4 {
                0 => Shape::Sphere { centre, radius: 0.3 },
                1 => Shape::Cylinder { centre, half: 0.5, radius: 0.3, margin: 0.04 },
                _ => Shape::CapsuleAt { centre, radius: 0.38, half_height: 0.52 },
            };
            let mut cast = exe_cast_of(shape, by);
            cast.from = flip(cast.from);
            cast.by = flip(cast.by);
            let ends: Vec<CastEnd> =
                triangles.iter().map(|t| exe_cast::cast_triangle(&cast, &exe_gjk::Triangle(t.v), 0.0, &transform, 0.0, 1.0)).collect();
            let mut every: Vec<_> = ends
                .iter()
                .filter_map(|end| match *end {
                    CastEnd::Hit { fraction, normal, point } => Some(key(fraction, flip(point), flip(normal))),
                    _ => None,
                })
                .collect();
            let mut swept = Vec::new();
            shapes.sweep_each(shape, by, |c| swept.push(key(c.t, c.point.to_array(), c.normal.to_array())));
            every.sort();
            swept.sort();
            assert_eq!(swept, every, "{shape:?} by {by:?}");
            sweeps += 1;
            found += every.len();
        }
        assert!(sweeps > 200 && found > 200, "{sweeps} sweeps, {found} hits");
    }
}
