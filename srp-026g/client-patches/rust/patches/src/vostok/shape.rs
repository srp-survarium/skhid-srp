//! A collision object's shape as the game makes them (0.10b's
//! create_bt_primitive, create_btBvhTriangleMeshShape and
//! create_compound_shape): a box, a sphere, a capsule, a Y cylinder, a
//! triangle mesh (btBvhTriangleMeshShape) or a compound of those, every
//! size in the shape's own space, Bullet's frame (z the game's -z).
//!
//! A mesh's triangles near a point come from Bullet's own BVH query
//! (btBvhTriangleMeshShape::processAllTriangles 0xA8660) through a callback
//! of ours: a btTriangleCallback whose vtable is a patch's copy of
//! btConvexTriangleCallback's, its processTriangle the patch's handler
//! (`triangle_callback.process`), which hands each triangle to
//! [`TriangleSink::deliver`].

use super::geometry::Pose;
use super::hook::{Handle, Thread};
use super::project::NativeVtable;
use super::world::{floats, read};
use crate::raw;

/// A patch's copy of the triangle callback's vtable.
pub type TriangleMethods = NativeVtable<raw::TriangleCallbackMethods>;

const BOX: u32 = 0;
const SPHERE: u32 = 8;
const CAPSULE: u32 = 10;
const CYLINDER: u32 = 13;
const TRIANGLE_MESH: u32 = 21;
const COMPOUND: u32 = 31;

/// A shape, its sizes with the margin Bullet collides with.
#[derive(Clone, Copy)]
pub enum Shape<'t> {
    Box { half: [f32; 3] },
    Sphere { radius: f32 },
    /// Its segment along `up` (0 to 2), `half_height` each way, swept by `radius`.
    Capsule { up: usize, radius: f32, half_height: f32 },
    /// Along y.
    Cylinder { radius: f32, half_height: f32 },
    Mesh(Mesh<'t>),
    Compound(Compound<'t>),
    /// A type the game does not make for its objects: its number.
    Other(u32),
}

impl<'t> Shape<'t> {
    /// # Safety
    /// `shape` is a live shape of the world a hook on `thread` reads.
    pub(in crate::vostok) unsafe fn of(shape: raw::BtCollisionShape, thread: &'t Thread) -> Shape<'t> {
        // SAFETY: the caller's; each kind's fields read only for its type
        // (the convex ones' for 0, 8, 10 and 13, a compound's for 31).
        unsafe {
            let kind = read(shape.shape_type());
            let convex = shape.convex();
            let dimensions = || -> [f32; 3] { floats(convex.dimensions()) };
            let with_margin = || dimensions().map(|d| d + read(convex.margin()));
            match kind {
                BOX => Shape::Box { half: with_margin() },
                SPHERE => Shape::Sphere { radius: dimensions()[0] * read(convex.scaling()) },
                CAPSULE => match read(convex.up_axis()) as usize {
                    up @ 0..=2 => {
                        Shape::Capsule { up, radius: dimensions()[(up + 2) % 3], half_height: dimensions()[up] }
                    }
                    _ => Shape::Other(kind),
                },
                CYLINDER if read(shape.vtable()) == raw::image::CYLINDER_VTABLE => {
                    let [radius, half_height, _] = with_margin();
                    Shape::Cylinder { radius, half_height }
                }
                TRIANGLE_MESH => Shape::Mesh(Mesh { shape, _thread: thread }),
                COMPOUND => Shape::Compound(Compound { this: shape.compound(), thread }),
                _ => Shape::Other(kind),
            }
        }
    }
}

/// A btTransform as a pose.
///
/// # Safety
/// `transform` is a live btTransform.
pub(in crate::vostok) unsafe fn pose(transform: raw::BtTransform) -> Pose {
    // SAFETY: the caller's: its 16 floats, three basis rows of four and the
    // origin.
    let f: [f32; 16] = unsafe { floats(transform.basis()) };
    Pose { rows: [[f[0], f[1], f[2]], [f[4], f[5], f[6]], [f[8], f[9], f[10]]], origin: [f[12], f[13], f[14]] }
}

/// A btBvhTriangleMeshShape.
#[derive(Clone, Copy)]
pub struct Mesh<'t> {
    shape: raw::BtCollisionShape,
    _thread: &'t Thread,
}

/// Bullet's vectors as its functions read them.
#[repr(C, align(16))]
struct Aligned([f32; 4]);

impl Mesh<'_> {
    /// Each triangle the shape's BVH finds in the box `low`..`high` (its own
    /// space) to `visit`, its three corners in that space, before this
    /// returns. `methods` is the patch's copy of the callback's vtable.
    pub fn triangles_in(&self, methods: TriangleMethods, low: [f32; 3], high: [f32; 3],
                        visit: &mut dyn FnMut([[f32; 3]; 3])) {
        let mut room = raw::TriangleCallbackStorage::zeroed();
        let callback = room.handle();
        let mut visitor: &mut dyn FnMut([[f32; 3]; 3]) = visit;
        let (low, high) = (Aligned([low[0], low[1], low[2], 0.0]), Aligned([high[0], high[1], high[2], 0.0]));
        // SAFETY: the shape, alive for the hook; the callback is ours, its
        // vtable the patch's copy whose processTriangle delivers to the
        // visitor, which lives past the call; the query only reads the mesh
        // and its BVH and calls the callback before it returns.
        unsafe {
            *callback.vtable() = methods.address();
            *callback.visitor() = (&raw mut visitor) as usize as u32;
            raw::engine_process_all_triangles(Some(self.shape), Some(callback), low.0.as_ptr(), high.0.as_ptr());
        }
    }
}

/// A btCompoundShape.
#[derive(Clone, Copy)]
pub struct Compound<'t> {
    this: raw::BtCompoundShape,
    thread: &'t Thread,
}

impl<'t> Compound<'t> {
    /// Each child: its pose in the compound and its shape.
    pub fn for_each_child(&self, mut visit: impl FnMut(Pose, Shape<'t>)) {
        // SAFETY: the compound, alive for the hook; m_children holds
        // child_count children, each its transform and a live shape.
        unsafe {
            let Some(first) = self.this.children() else { return };
            for i in 0..read(self.this.child_count()) as usize {
                let child = first.at(i);
                let Some(shape) = child.shape() else { continue };
                visit(pose(child.transform()), Shape::of(shape, self.thread));
            }
        }
    }
}

/// One of our triangle callbacks, as its processTriangle is given it.
pub struct TriangleSink<'t> {
    native: raw::TriangleCallback,
    _thread: &'t Thread,
}

impl<'t> Handle<'t> for TriangleSink<'t> {
    type Raw = raw::TriangleCallback;

    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::TriangleCallback>) -> Option<Self> {
        Some(TriangleSink { native: native?, _thread: thread })
    }
}

impl TriangleSink<'_> {
    pub(in crate::vostok) fn raw(&self) -> raw::TriangleCallback {
        self.native
    }

    /// The triangle to the visitor of the query that made this callback.
    pub fn deliver(&mut self, triangle: &Triangle<'_>) {
        // SAFETY: only Mesh::triangles_in makes a callback with this vtable,
        // its visitor the address of a live `&mut dyn FnMut`, and the game
        // calls its processTriangle only inside that query.
        unsafe {
            let visitor = read(self.native.visitor()) as usize as *mut &mut dyn FnMut([[f32; 3]; 3]);
            (*visitor)(triangle.points());
        }
    }
}

/// The triangle processTriangle is given: three btVector3.
pub struct Triangle<'t> {
    at: *const f32,
    _thread: core::marker::PhantomData<&'t Thread>,
}

impl<'t> Triangle<'t> {
    /// # Safety
    /// `at` is 12 floats alive while the hook runs, on its thread.
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, at: *const f32) -> Triangle<'t> {
        Triangle { at, _thread: core::marker::PhantomData }
    }

    pub(in crate::vostok) fn raw(&self) -> *const f32 {
        self.at
    }

    fn points(&self) -> [[f32; 3]; 3] {
        // SAFETY: from_raw's.
        let f: [f32; 12] = unsafe { floats(self.at.cast_mut()) };
        [[f[0], f[1], f[2]], [f[4], f[5], f[6]], [f[8], f[9], f[10]]]
    }
}
