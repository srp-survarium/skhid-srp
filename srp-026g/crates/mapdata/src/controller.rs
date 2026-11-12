//! The client's character controller against the map's collision (A313): the capsule and its sweeps.
//!
//! The 0.26g client moves a body with `vostok::physics::bullet_character_controller` (a Bullet kinematic controller): each walk
//! tick lifts the capsule by the step (`step_up` 0x7C900), sweeps it along the move and slides it along what it meets
//! (`step_forward_impl` 0x7D2A0), and sweeps it down onto the ground (`step_down` 0x7E1B0), refusing ground steeper than the
//! slope limit. This module is the geometry those steps stand on: a capsule (a vertical segment between its two sphere centres
//! and a radius) swept by a translation against the map's triangles, giving the first contact, its point and its normal.
//!
//! How: every triangle near the swept volume (an AABB tree over the triangles) is advanced on conservatively: the exact distance
//! from the capsule's segment to the triangle, less the radius, is how far the capsule can move with no contact, so the sweep
//! steps that far along the move and measures again, until the gap is under [`CONTACT_M`] or the move ends. A capsule that only
//! translates gets no closer than its speed allows, so no contact is stepped over.
pub mod exe_cast;
pub mod exe_epa;
pub mod exe_gjk;
pub mod exe_ray;
pub mod gjk;
mod solids;
pub mod speed;
pub mod tick;

use crate::{MapData, Triangle, WalkerBehaviour};
use crate::math::Vec3;

/// The gap at which a sweep counts as touching, m.
pub const CONTACT_M: f32 = 1e-4;
/// A touching capsule's move goes into the face when its part against the normal is over this share of it (float noise in a
/// normal along the face is not a move into it).
const GOING_IN: f32 = 1e-4;
/// Steps of the conservative advancement against one triangle at most.
const ADVANCE_STEPS: usize = 64;
/// Triangles a leaf of the tree holds at most.
const LEAF: usize = 4;

/// One triangle of the collision: its corners, unit normal (zero for a degenerate one) and walker behaviour (the down sweep
/// reads it: a `walk` face counts as flat, a `dont_walk` one as steep).
#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub v: [Vec3; 3],
    pub normal: Vec3,
    pub walker: WalkerBehaviour,
}

/// A body swept against the collision: a convex core and the margin around it (Bullet's rounded shapes).
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// A capsule: the segment between its sphere centres, the radius as the margin.
    Capsule(Capsule),
    /// An upright cylinder's core (centre, half height, radius) and its margin.
    Cylinder { centre: Vec3, half: f32, radius: f32, margin: f32 },
    /// A sphere: its centre and radius.
    Sphere { centre: Vec3, radius: f32 },
    /// An upright capsule as the exe's controller holds it (Bullet's `btCapsuleShape`, up axis y): the centre (the cast's
    /// origin), the radius and the half height of the straight part, kept as given so [`Caster::Exe`] casts the exe's own
    /// numbers. The other casters read it as the [`Shape::Capsule`] between its two sphere centres.
    CapsuleAt { centre: Vec3, radius: f32, half_height: f32 },
}

impl Shape {
    /// A point of `v` (a face) where the shape overlaps it: for a cylinder whose core is inside the face (GJK gives no
    /// points there), the face's point nearest the core's axis; else `point` (the gap's own).
    pub fn witness_on(&self, v: &[Vec3; 3], point: Vec3) -> Vec3 {
        match *self {
            Shape::Cylinder { centre, half, .. } => {
                let up = Vec3::new(0.0, half, 0.0);
                segment_triangle(centre - up, centre + up, v).2
            }
            _ => point,
        }
    }

    /// A [`Shape::CapsuleAt`] as the segment between its sphere centres and its radius.
    fn capsule_of(centre: Vec3, radius: f32, half_height: f32) -> Capsule {
        let up = Vec3::new(0.0, half_height, 0.0);
        Capsule { low: centre - up, high: centre + up, radius }
    }

    pub fn moved(self, by: Vec3) -> Self {
        match self {
            Shape::CapsuleAt { centre, radius, half_height } => Shape::CapsuleAt { centre: centre + by, radius, half_height },
            Shape::Capsule(c) => Shape::Capsule(c.moved(by)),
            Shape::Cylinder { centre, half, radius, margin } => Shape::Cylinder { centre: centre + by, half, radius, margin },
            Shape::Sphere { centre, radius } => Shape::Sphere { centre: centre + by, radius },
        }
    }

    /// The distance from the shape's surface to the triangle (negative inside the margin), the closest points on the core and
    /// on the triangle, and the direction from the triangle to the shape.
    pub fn gap(&self, v: &[Vec3; 3], normal: Vec3) -> (f32, Vec3, Vec3, Vec3) {
        let (d, on_core, on_face, margin) = match *self {
            Shape::CapsuleAt { centre, radius, half_height } => {
                return Shape::Capsule(Self::capsule_of(centre, radius, half_height)).gap(v, normal);
            }
            Shape::Capsule(c) => {
                let (d, s, f) = segment_triangle(c.low, c.high, v);
                (d, s, f, c.radius)
            }
            Shape::Sphere { centre, radius } => {
                let f = point_triangle(centre, v);
                ((centre - f).length(), centre, f, radius)
            }
            Shape::Cylinder { centre, half, radius, margin } => {
                let core = gjk::Cylinder { centre, half, radius };
                let guess = centre - (v[0] + v[1] + v[2]) * (1.0 / 3.0);
                let (d, a, b) = gjk::distance(&core, v, guess);
                (d, a, b, margin)
            }
        };
        let dir = if d > 1e-6 { (on_core - on_face) * (1.0 / d) } else { normal };
        (d - margin, on_core, on_face, dir)
    }

    /// The shape's box.
    fn aabb(&self) -> (Vec3, Vec3) {
        match *self {
            Shape::CapsuleAt { centre, radius, half_height } => Shape::Capsule(Self::capsule_of(centre, radius, half_height)).aabb(),
            Shape::Capsule(c) => {
                let r = Vec3::new(c.radius, c.radius, c.radius);
                (c.low.min(c.high) - r, c.low.max(c.high) + r)
            }
            Shape::Cylinder { centre, half, radius, margin } => {
                let e = Vec3::new(radius + margin, half + margin, radius + margin);
                (centre - e, centre + e)
            }
            Shape::Sphere { centre, radius } => {
                let e = Vec3::new(radius, radius, radius);
                (centre - e, centre + e)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    /// A leaf: `first..first + count` of the faces; else `first` is the right child (the left is the next node).
    first: u32,
    count: u32,
}

/// How a sweep finds its first contact with one face.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Caster {
    /// Conservative advancement by the gap over the move's length, touching at [`CONTACT_M`]; a start contact counts when the
    /// move goes in past [`GOING_IN`] of its length. The port's first rule (A313).
    #[default]
    Conservative,
    /// The exe's own: its caster 0xB9320 is stock `btContinuousConvexCollision::calcTimeOfImpact` (A325 P1, read and run in
    /// Unicorn): touching at [`BULLET_TOLERANCE_M`], each step the gap over the move's part along the contact normal, no
    /// contact when that part is at most `f32::EPSILON` (at the start or on the way), and no contact after
    /// [`ADVANCE_STEPS`] steps. A grazing approach (a sphere sliding toward a seam between two flat faces) converges here and
    /// meets the face as the exe does; [`Caster::Conservative`] runs out of steps and misses it.
    Bullet(StartOverlap),
    /// The exe's own arithmetic ([`exe_cast`] over [`exe_gjk`]): Bullet's GJK with its Voronoi simplex solver and the
    /// time-of-impact loop in the exe's order of operations, bit for bit on 57,617 recorded closest-point calls of the
    /// game (`docs/notes/exactness_rig.md`), and where the exe goes on to its penetration-depth solver (the shape's
    /// core in the face or within 0.01 m of it on a degenerate simplex) that solver's own answer ([`exe_epa`]: the
    /// exe's bits on 10,666 recorded calls).
    Exe,
}

/// Where [`Caster::Bullet`] puts a contact met already overlapping at the start (the cylinder's core inside the face, where
/// GJK gives no points: its `distance` returns zeros).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StartOverlap {
    /// At the zero point GJK returns: the port's first rule (A325).
    #[default]
    ZeroPoint,
    /// [A329] At the face's point nearest the core's axis: Bullet's caster takes the penetration solver's witness on the face,
    /// a point of the overlap, and the move test's top check (0x85F50) reads its height (level_08 Cologne corner: the zero
    /// point read as a hit at the feet blocked a walk under an overhang the exe walks under).
    FaceWitness,
}

/// [`Caster::Bullet`]'s touching distance (the exe's 0x91A8F0 = 0.001), m.
pub const BULLET_TOLERANCE_M: f32 = 0.001;

/// The map's collision faces in an AABB tree, for capsule sweeps.
pub struct Collision {
    faces: Vec<Face>,
    nodes: Vec<Node>,
    sweeps: Sweeps,
}

/// What a sweep is cast against.
enum Sweeps {
    /// Every face on its own, placed in the map frame, by this caster.
    Faces(Caster),
    /// The game's own objects by the exe's cast ([`Collision::build_exe`]): each mesh in its own frame under its
    /// object's transform, each primitive as the convex shape it is, the penetration solver's answer included.
    Solids(Box<solids::Solids>),
}

/// The first contact of a sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The share of the move done before the contact, 0..=1.
    pub t: f32,
    /// The contact's point on what is met.
    pub point: Vec3,
    /// From what is met to the capsule at the contact, unit.
    pub normal: Vec3,
    /// What is met.
    pub on: Surface,
    /// The game's object met, in a world that holds the game's objects ([`Collision::build_exe`]).
    pub object: Option<SolidId>,
}

/// One of the game's collision objects in a [`Collision::build_exe`] world (a placement the walking body meets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolidId(u32);

/// What a contact is on, as the walk tick's callbacks read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    /// A triangle of a mesh: its walker behaviour and its own unit normal (either side).
    Face { walker: WalkerBehaviour, normal: Vec3 },
    /// One of the game's convex primitives, met as the shape itself ([`Collision::build_exe`]). Its walker behaviour
    /// is the one the down sweep's callback 0x84420 finds: a compound's child's own (the result's shape info names the
    /// child, 0xA2BE0), by slope for a primitive that is an object's whole shape whatever its material (no shape
    /// info: 0x84561 takes 0). `round` for a small round one.
    Primitive { walker: WalkerBehaviour, round: Option<SmallRound> },
}

impl Surface {
    pub fn walker(self) -> WalkerBehaviour {
        match self {
            Surface::Face { walker, .. } | Surface::Primitive { walker, .. } => walker,
        }
    }
}

/// A sphere, a capsule or a cylinder under 0.8 m of radius (0xA7AB04; a sphere's radius times its scaling, a capsule's
/// radius, a cylinder's with its margin, `getRadius` 0xA9BC0), which the down sweep's second look at steep ground
/// (0x84810) treats on its own. `upright`: how far the shape's own y axis is along the world's up, the absolute value
/// of `up . (M up)` with `M` the shape's world rotation sent through a quaternion (0x84190, 0x848C7..0x8496C).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SmallRound {
    Sphere,
    Capsule { upright: f32 },
    Cylinder { upright: f32 },
}

/// A sweep's meeting with one face or shape, before it is said what it is on.
#[derive(Clone, Copy, Debug)]
struct Touch {
    t: f32,
    point: Vec3,
    normal: Vec3,
}

/// A capsule standing upright: the centres of its lower and upper spheres and its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub low: Vec3,
    pub high: Vec3,
    pub radius: f32,
}

impl Capsule {
    /// The capsule whose bottom (the lowest point of its lower sphere) is at `feet`, `height` tall in all.
    pub fn standing(feet: Vec3, height: f32, radius: f32) -> Self {
        let low = feet + Vec3::new(0.0, radius, 0.0);
        let high = feet + Vec3::new(0.0, (height - radius).max(radius), 0.0);
        Self { low, high, radius }
    }

    /// The capsule moved by `by`.
    pub fn moved(self, by: Vec3) -> Self {
        Self { low: self.low + by, high: self.high + by, ..self }
    }
}

fn aabb(points: impl IntoIterator<Item = Vec3>) -> (Vec3, Vec3) {
    let mut lo = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut hi = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in points {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (lo, hi)
}

fn overlaps(lo: Vec3, hi: Vec3, qlo: Vec3, qhi: Vec3) -> bool {
    lo.x <= qhi.x && hi.x >= qlo.x && lo.y <= qhi.y && hi.y >= qlo.y && lo.z <= qhi.z && hi.z >= qlo.z
}

impl Collision {
    /// The collision of these triangles (every walker behaviour: a `dont_walk` face stops a body as any face does).
    pub fn build(triangles: &[Triangle]) -> Self {
        let mut faces: Vec<Face> = triangles
            .iter()
            .filter(|t| t.v.iter().all(|p| p.is_finite()))
            .map(|t| {
                let n = (t.v[1] - t.v[0]).cross(t.v[2] - t.v[0]);
                let l = n.length();
                Face { v: t.v, normal: if l > 1e-12 { n * (1.0 / l) } else { Vec3::ZERO }, walker: t.walker }
            })
            .collect();
        let mut nodes = Vec::with_capacity(2 * faces.len() / LEAF + 1);
        if !faces.is_empty() {
            let n = faces.len();
            Self::split(&mut faces, 0, n, &mut nodes);
        }
        Self { faces, nodes, sweeps: Sweeps::Faces(Caster::default()) }
    }

    /// The same faces, its sweeps against each face by `caster`.
    pub fn with_caster(self, caster: Caster) -> Self {
        Self { sweeps: Sweeps::Faces(caster), ..self }
    }

    /// The walking body's collision of `map` as the game holds it, for sweeps by the exe's own cast: the game's
    /// objects ([`crate::bullet`]: a placed mesh's triangles in the mesh's frame under the object's transform, a box,
    /// a sphere, a cylinder or a capsule as the convex shape with its margin, a compound's children under the
    /// transforms the exe composes), met as `objectQuerySingle` 0xA14E0 meets them. Rays and the nearest-face
    /// questions still read the placed triangles ([`MapData::walker_triangles`]; a primitive's are its drawn
    /// stand-in). `None` for a map without the game's shapes (a file older than format 6).
    pub fn build_exe(map: &MapData) -> Option<Self> {
        let solids = solids::Solids::build(map)?;
        Some(Self { sweeps: Sweeps::Solids(Box::new(solids)), ..Self::build(&map.walker_triangles()) })
    }

    fn split(faces: &mut [Face], first: usize, end: usize, nodes: &mut Vec<Node>) -> usize {
        let (lo, hi) = aabb(faces[first..end].iter().flat_map(|f| f.v));
        let me = nodes.len();
        nodes.push(Node { lo, hi, first: first as u32, count: (end - first) as u32 });
        if end - first <= LEAF {
            return me;
        }
        let size = hi - lo;
        let axis = if size.x >= size.y && size.x >= size.z { 0 } else if size.y >= size.z { 1 } else { 2 };
        let centre = |f: &Face| (f.v[0].axis(axis) + f.v[1].axis(axis) + f.v[2].axis(axis)) / 3.0;
        faces[first..end].sort_by(|a, b| centre(a).total_cmp(&centre(b)));
        let mid = first + (end - first) / 2;
        Self::split(faces, first, mid, nodes);
        let right = Self::split(faces, mid, end, nodes);
        nodes[me].first = right as u32;
        nodes[me].count = 0;
        me
    }

    /// The heap it holds, bytes (its faces and its tree).
    pub fn bytes(&self) -> usize {
        let solids = match &self.sweeps {
            Sweeps::Faces(_) => 0,
            Sweeps::Solids(solids) => solids.bytes(),
        };
        self.faces.capacity() * std::mem::size_of::<Face>() + self.nodes.capacity() * std::mem::size_of::<Node>() + solids
    }

    pub fn len(&self) -> usize {
        self.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    pub fn face(&self, index: usize) -> &Face {
        &self.faces[index]
    }

    /// Calls `each` with the index of every face whose box meets the box `qlo..qhi`.
    pub fn faces_in(&self, qlo: Vec3, qhi: Vec3, mut each: impl FnMut(usize)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let node = self.nodes[i];
            if !overlaps(node.lo, node.hi, qlo, qhi) {
                continue;
            }
            if node.count > 0 {
                for f in node.first as usize..(node.first + node.count) as usize {
                    let (lo, hi) = aabb(self.faces[f].v);
                    if overlaps(lo, hi, qlo, qhi) {
                        each(f);
                    }
                }
            } else {
                stack.push(i + 1);
                stack.push(node.first as usize);
            }
        }
    }

    /// The first contact of `capsule` moved by `by`, against faces `keep` accepts. A face the capsule already overlaps at the
    /// start is met only when the move goes into it (against its normal at the start).
    pub fn sweep(&self, capsule: Capsule, by: Vec3, keep: impl Fn(&Face) -> bool) -> Option<Contact> {
        self.sweep_shape(Shape::Capsule(capsule), by, keep)
    }

    /// [`Self::sweep`] for any shape.
    pub fn sweep_shape(&self, shape: Shape, by: Vec3, keep: impl Fn(&Face) -> bool) -> Option<Contact> {
        let mut best: Option<Contact> = None;
        self.sweep_each(shape, by, |c| {
            if keep_surface(&keep, c) && best.is_none_or(|b| c.t < b.t) {
                best = Some(c);
            }
        });
        best
    }

    /// Every face's first contact with `shape` moved by `by` (Bullet's per-triangle convex cast: what the client's sweep
    /// callbacks are handed, in no order). A face the shape already touches at the start is reported at 0 only when the move
    /// goes into it.
    pub fn sweep_each(&self, shape: Shape, by: Vec3, mut each: impl FnMut(Contact)) {
        let caster = match &self.sweeps {
            Sweeps::Faces(caster) => *caster,
            Sweeps::Solids(solids) => return solids.sweep_each(shape, by, each),
        };
        let (lo0, hi0) = shape.aabb();
        let (lo1, hi1) = shape.moved(by).aabb();
        let touch = match caster {
            Caster::Conservative => CONTACT_M,
            Caster::Bullet(_) | Caster::Exe => BULLET_TOLERANCE_M,
        };
        let pad = Vec3::new(touch, touch, touch);
        self.faces_in(lo0.min(lo1) - pad, hi0.max(hi1) + pad, |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO {
                return;
            }
            let touch = match caster {
                Caster::Conservative => advance(shape, by, face),
                Caster::Bullet(start) => advance_bullet(shape, by, face, start),
                Caster::Exe => advance_exe(shape, by, face),
            };
            if let Some(Touch { t, point, normal }) = touch {
                each(Contact { t, point, normal, on: Surface::Face { walker: face.walker, normal: face.normal }, object: None });
            }
        });
    }

    /// The ray of the walk tick's second looks (`ray_pick` 0x898C0, the steep-ground look 0x849BB) from `a` to `b`: the
    /// normal of what it meets. The exe casts it with `btCollisionWorld::rayTestSingle` 0xA02D0 against the one object
    /// the sweep hit (its root shape under its transform), never the world: with `object` (a contact of a
    /// [`Self::build_exe`] world) that is done here, by the exe's own arithmetic ([`exe_ray`]). Without one (the placed
    /// triangles know no objects) the ray meets the nearest placed face of any object ([`Self::ray`]).
    pub fn look_ray(&self, object: Option<SolidId>, a: Vec3, b: Vec3) -> Option<Vec3> {
        match (&self.sweeps, object) {
            (Sweeps::Solids(solids), Some(SolidId(id))) => solids.look_ray(id, a, b),
            _ => self.ray(a, b).map(|(_, normal)| normal),
        }
    }

    /// The nearest face a ray from `a` to `b` crosses: the share of the way and the face's normal turned toward `a` (Bullet's
    /// triangle ray normal, LIKELY).
    pub fn ray(&self, a: Vec3, b: Vec3) -> Option<(f32, Vec3)> {
        let d = b - a;
        let mut best: Option<(f32, Vec3)> = None;
        self.faces_in(a.min(b), a.max(b), |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO {
                return;
            }
            // Moller-Trumbore.
            let (e1, e2) = (face.v[1] - face.v[0], face.v[2] - face.v[0]);
            let p = d.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-12 {
                return;
            }
            let inv = 1.0 / det;
            let s = a - face.v[0];
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                return;
            }
            let q = s.cross(e1);
            let v = d.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                return;
            }
            let t = e2.dot(q) * inv;
            if (0.0..=1.0).contains(&t) && best.is_none_or(|b| t < b.0) {
                let n = if face.normal.dot(d) > 0.0 { -face.normal } else { face.normal };
                best = Some((t, n));
            }
        });
        best
    }

    /// The face nearest `shape`'s surface that `keep` accepts within `reach` (a negative gap: overlapping): the gap, the face's
    /// closest point and the direction from it to the shape.
    pub fn nearest_to(&self, shape: Shape, reach: f32, keep: impl Fn(&Face) -> bool) -> Option<(f32, Vec3, Vec3)> {
        let (lo, hi) = shape.aabb();
        let pad = Vec3::new(reach, reach, reach);
        let mut best: Option<(f32, Vec3, Vec3)> = None;
        self.faces_in(lo - pad, hi + pad, |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO || !keep(face) {
                return;
            }
            let (gap, _, on_face, dir) = shape.gap(&face.v, face.normal);
            if gap <= reach && best.is_none_or(|b| gap < b.0) {
                best = Some((gap, on_face, dir));
            }
        });
        best
    }

    /// The smallest distance from the capsule's surface to a face `keep` accepts within `reach` (negative when it overlaps
    /// one), with that face's closest point and the direction from it to the capsule's segment.
    pub fn nearest(&self, capsule: Capsule, reach: f32, keep: impl Fn(&Face) -> bool) -> Option<(f32, Vec3, Vec3, usize)> {
        let pad = Vec3::new(capsule.radius + reach, capsule.radius + reach, capsule.radius + reach);
        let (lo, hi) = aabb([capsule.low, capsule.high]);
        let mut best: Option<(f32, Vec3, Vec3, usize)> = None;
        self.faces_in(lo - pad, hi + pad, |f| {
            let face = &self.faces[f];
            if face.normal == Vec3::ZERO || !keep(face) {
                return;
            }
            let (gap, _, on_face, dir) = Shape::Capsule(capsule).gap(&face.v, face.normal);
            if gap <= reach && best.is_none_or(|b| gap < b.0) {
                best = Some((gap, on_face, dir, f));
            }
        });
        best
    }
}

/// The conservative advancement of `shape` moved by `by` against one face.
fn advance(shape: Shape, by: Vec3, face: &Face) -> Option<Touch> {
    let speed = by.length();
    let (g0, _, f0, n0) = shape.gap(&face.v, face.normal);
    if g0 <= CONTACT_M {
        // Touching or overlapping at the start: a contact only when the move goes in.
        return (by.dot(n0) < -GOING_IN * speed).then_some(Touch { t: 0.0, point: f0, normal: n0 });
    }
    if speed <= 0.0 {
        return None;
    }
    let mut t = 0.0f32;
    for _ in 0..ADVANCE_STEPS {
        let (gap, _, f, n) = shape.moved(by * t).gap(&face.v, face.normal);
        if gap <= CONTACT_M {
            return Some(Touch { t, point: f, normal: n });
        }
        t += gap / speed;
        if t > 1.0 {
            return None;
        }
    }
    None
}

/// [`Caster::Bullet`] against one face: `btContinuousConvexCollision::calcTimeOfImpact` as the exe runs it (0xB9320; the
/// face's margin 0, LIKELY: the map mesh's ctor 0xA7950 sets 0 and no setMargin call was found).
fn advance_bullet(shape: Shape, by: Vec3, face: &Face, start: StartOverlap) -> Option<Touch> {
    let (mut gap, _, mut f, mut n) = shape.gap(&face.v, face.normal);
    // The move's part toward the face (`relLinVel . normalOnB`: the face is B, the normal points from it to the shape).
    let mut toward = -by.dot(n);
    if toward <= f32::EPSILON {
        return None;
    }
    let mut t = 0.0f32;
    let mut steps = 0;
    while gap > BULLET_TOLERANCE_M {
        steps += 1;
        if steps > ADVANCE_STEPS {
            return None;
        }
        t += gap / toward;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        (gap, _, f, n) = shape.moved(by * t).gap(&face.v, face.normal);
        toward = -by.dot(n);
        if toward <= f32::EPSILON {
            return None;
        }
    }
    // A contact met already overlapping at the start ([`StartOverlap`]).
    if start == StartOverlap::FaceWitness && t == 0.0 && gap <= 0.0 {
        f = shape.witness_on(&face.v, f);
    }
    Some(Touch { t, point: f, normal: n })
}

/// `shape` moved by `by` as the exe's sweep hands it to Bullet: `by` is the cast's way as the exe subtracts it (the
/// end's origin minus the start's).
fn exe_cast_of(shape: Shape, by: Vec3) -> exe_cast::Cast {
    use exe_gjk::{Axis, CastShape};
    let capsule = |radius: f32, half_height: f32| CastShape::capsule([radius, half_height, radius], [1.0; 3], EXE_CAPSULE_MARGIN_M, Axis::Y);
    let (cast_shape, margin, from) = match shape {
        Shape::CapsuleAt { centre, radius, half_height } => (capsule(radius, half_height), EXE_CAPSULE_MARGIN_M, centre),
        // A capsule given by its sphere centres: the centre and the half height are worked back (not the exe's bits).
        Shape::Capsule(c) => (capsule(c.radius, (c.high.y - c.low.y) * 0.5), EXE_CAPSULE_MARGIN_M, (c.low + c.high) * 0.5),
        Shape::Cylinder { centre, half, radius, margin } => (CastShape::cylinder([radius, half, radius], Axis::Y), margin, centre),
        Shape::Sphere { centre, radius } => (CastShape::Sphere, radius, centre),
    };
    exe_cast::Cast { shape: cast_shape, margin, from: from.to_array(), by: by.to_array() }
}

/// [`Caster::Exe`] against one face: the exe's cast of its own shape ([`exe_cast::cast_triangle`]).
fn advance_exe(shape: Shape, by: Vec3, face: &Face) -> Option<Touch> {
    let cast = exe_cast_of(shape, by);
    let triangle = exe_gjk::Triangle([face.v[0].to_array(), face.v[1].to_array(), face.v[2].to_array()]);
    // The face is in world coordinates (transform B the identity), its margin 0, the sweep's allowed penetration 0
    // (`convexSweepTest` 0xA34E8), and the callback's fraction 1 (every face's hit is handed on).
    match exe_cast::cast_triangle(&cast, &triangle, 0.0, &exe_cast::IDENTITY, 0.0, 1.0) {
        exe_cast::CastEnd::Hit { fraction, normal, point } => {
            Some(Touch { t: fraction, point: Vec3::from_array(point), normal: Vec3::from_array(normal) })
        }
        exe_cast::CastEnd::Miss => None,
    }
}

/// Whether `keep` accepts what `contact` is on: a face by its walker behaviour and normal; a primitive (no face) is
/// asked as a face of its walker behaviour whose normal is the contact's.
fn keep_surface(keep: &impl Fn(&Face) -> bool, contact: Contact) -> bool {
    let (walker, normal) = match contact.on {
        Surface::Face { walker, normal } => (walker, normal),
        Surface::Primitive { walker, .. } => (walker, contact.normal),
    };
    keep(&Face { v: [contact.point; 3], normal, walker })
}

/// The collision margin of the controller's capsule (+1E0h of its shape, 0.04: Bullet's default), which the exe's
/// detector takes as the capsule's margin.
const EXE_CAPSULE_MARGIN_M: f32 = 0.04;

/// The closest points of point `p` on triangle `v` (Ericson, Real-Time Collision Detection 5.1.5).
pub fn point_triangle(p: Vec3, v: &[Vec3; 3]) -> Vec3 {
    let (a, b, c) = (v[0], v[1], v[2]);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// The closest points of segments `p1..q1` and `p2..q2` (Ericson 5.1.9).
fn segment_segment(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let eps = 1e-12;
    let (s, t);
    if a <= eps && e <= eps {
        return (p1, p2);
    }
    if a <= eps {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= eps {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > eps { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (p1 + d1 * s, p2 + d2 * t)
}

/// The distance from segment `p..q` to triangle `v`, with the closest point on each.
pub fn segment_triangle(p: Vec3, q: Vec3, v: &[Vec3; 3]) -> (f32, Vec3, Vec3) {
    // The segment through the triangle's inside: distance 0 there.
    let n = (v[1] - v[0]).cross(v[2] - v[0]);
    let (dp, dq) = (n.dot(p - v[0]), n.dot(q - v[0]));
    if (dp > 0.0) != (dq > 0.0) && dp != dq {
        let x = p + (q - p) * (dp / (dp - dq));
        let on = point_triangle(x, v);
        if (on - x).length() <= 1e-6 {
            return (0.0, x, on);
        }
    }
    let mut best = {
        let on = point_triangle(p, v);
        ((p - on).length(), p, on)
    };
    let mut take = |s: Vec3, f: Vec3| {
        let d = (s - f).length();
        if d < best.0 {
            best = (d, s, f);
        }
    };
    let on = point_triangle(q, v);
    take(q, on);
    for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
        let (s, f) = segment_segment(p, q, a, b);
        take(s, f);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(a: Vec3, b: Vec3, c: Vec3) -> Triangle {
        Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }
    }

    /// A wall in the plane x = `x`, from z -5 to 5 and y 0 to 3.
    fn wall(x: f32) -> Vec<Triangle> {
        let p = |y, z| Vec3::new(x, y, z);
        vec![tri(p(0.0, -5.0), p(0.0, 5.0), p(3.0, 5.0)), tri(p(0.0, -5.0), p(3.0, 5.0), p(3.0, -5.0))]
    }

    /// A325 P1, the Vector rail case's floor (level_07 day_2, rev-48: two nearly flat triangles 2e-6 m apart along their
    /// diagonal) and the jump-forward move of a capsule standing on the first, toward the second (the exe's own state at that
    /// tick): the exe's own cast (its player_step and Bullet run in Unicorn) meets the second's edge (at t 0.658); conservative
    /// advancement runs out of steps short of touching and misses it.
    #[test]
    fn the_bullet_caster_meets_a_floor_seam_the_conservative_one_misses() {
        let b = |x: u32, y: u32, z: u32| Vec3::new(f32::from_bits(x), f32::from_bits(y), f32::from_bits(z));
        let floor = vec![
            tri(b(0x429251df, 0x41c81ad5, 0xc1b313e6), b(0x429251df, 0x41c81ad5, 0xc1c6471a), b(0x4265877b, 0x41c81ad4, 0xc1c6471a)),
            tri(b(0x429251df, 0x41c81ad5, 0xc1b313e6), b(0x4265877b, 0x41c81ad4, 0xc1c6471a), b(0x4265877b, 0x41c81ad4, 0xc1b313e6)),
        ];
        let low = b(0x428ae2c6, 0x41cb250a, 0xc1b8236c);
        let capsule = Capsule { low, high: Vec3::new(low.x, f32::from_bits(0x41d376f6), low.z), radius: f32::from_bits(0x3ec28f5c) };
        let by = Vec3::new(f32::from_bits(0x3c5e1fdb), 0.0, f32::from_bits(0x3dc8d9f9));
        let first = |caster: Caster| {
            let world = Collision::build(&floor).with_caster(caster);
            let mut hits = Vec::new();
            world.sweep_each(Shape::Capsule(capsule), by, |c| hits.push(c));
            hits.into_iter().map(|c| c.t).reduce(f32::min)
        };
        assert_eq!(first(Caster::Conservative), None);
        // The fraction is not the exe's (0.524 here: Bullet's GJK normal near touching steers its steps); the jump-forward
        // branch slides from the move's start on a hit, so the body ends within 4e-5 m of the exe's (A325 trust table).
        let t = first(Caster::Bullet(StartOverlap::FaceWitness)).expect("the exe meets the seam");
        assert!((0.0..1.0).contains(&t), "t {t}");
    }

    /// Sliding along a flat floor goes into no face: no contact with either caster (Bullet's velocity floor).
    #[test]
    fn a_slide_along_a_flat_floor_meets_nothing() {
        let capsule = Capsule { low: Vec3::new(0.0, 0.38, 0.0), high: Vec3::new(0.0, 1.42, 0.0), radius: 0.38 };
        for caster in [Caster::Conservative, Caster::Bullet(StartOverlap::ZeroPoint), Caster::Bullet(StartOverlap::FaceWitness)] {
            let world = Collision::build(&ground()).with_caster(caster);
            let mut hits = 0;
            world.sweep_each(Shape::Capsule(capsule), Vec3::new(0.3, 0.0, 0.1), |_| hits += 1);
            assert_eq!(hits, 0, "{caster:?}");
        }
    }

    fn ground() -> Vec<Triangle> {
        let p = |x, z| Vec3::new(x, 0.0, z);
        vec![tri(p(-10.0, -10.0), p(10.0, -10.0), p(10.0, 10.0)), tri(p(-10.0, -10.0), p(10.0, 10.0), p(-10.0, 10.0))]
    }

    /// A capsule walked into a wall stops a radius off it, with the wall's normal.
    #[test]
    fn a_capsule_swept_into_a_wall_stops_a_radius_off_it() {
        let world = Collision::build(&wall(2.0));
        let c = Capsule::standing(Vec3::new(0.0, 0.5, 0.0), 1.8, 0.38);
        let hit = world.sweep(c, Vec3::new(4.0, 0.0, 0.0), |_| true).expect("a contact");
        let x = 4.0 * hit.t;
        assert!((x - (2.0 - 0.38)).abs() < 2e-3, "{x}");
        assert!((hit.normal.x + 1.0).abs() < 1e-3, "{:?}", hit.normal);
    }

    /// Moving along a wall at more than the radius meets nothing; down onto the ground the bottom stops on it.
    #[test]
    fn a_sweep_along_a_wall_misses_and_a_fall_lands_on_the_ground() {
        let mut tris = wall(2.0);
        tris.extend(ground());
        let world = Collision::build(&tris);
        let c = Capsule::standing(Vec3::new(0.0, 0.5, 0.0), 1.8, 0.38);
        assert!(world.sweep(c, Vec3::new(0.0, 0.0, 4.0), |f| f.normal.y.abs() < 0.5).is_none());
        let hit = world.sweep(c, Vec3::new(0.0, -2.0, 0.0), |_| true).expect("the ground");
        assert!((2.0 * hit.t - 0.5).abs() < 2e-3, "{}", hit.t);
        assert!((hit.normal.y - 1.0).abs() < 1e-3);
    }

    /// A capsule already touching a wall slides along it (no contact) and is stopped going into it.
    #[test]
    fn a_touching_capsule_is_stopped_only_going_in() {
        let world = Collision::build(&wall(2.0));
        let c = Capsule::standing(Vec3::new(2.0 - 0.38, 0.5, 0.0), 1.8, 0.38);
        let along = world.sweep(c, Vec3::new(0.0, 0.0, 1.0), |_| true);
        assert!(along.is_none(), "{along:?}");
        let hit = world.sweep(c, Vec3::new(1.0, 0.0, 0.0), |_| true).expect("into the wall");
        assert_eq!(hit.t, 0.0);
    }

    /// The segment-triangle distance: through the face 0, over it the height, beside an edge the edge's distance.
    #[test]
    fn segment_triangle_distances() {
        let v = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        assert_eq!(segment_triangle(Vec3::new(0.2, -1.0, 0.2), Vec3::new(0.2, 1.0, 0.2), &v).0, 0.0);
        assert!((segment_triangle(Vec3::new(0.2, 0.5, 0.2), Vec3::new(0.2, 2.0, 0.2), &v).0 - 0.5).abs() < 1e-6);
        assert!((segment_triangle(Vec3::new(0.5, -1.0, -0.3), Vec3::new(0.5, 1.0, -0.3), &v).0 - 0.3).abs() < 1e-6);
    }
}
