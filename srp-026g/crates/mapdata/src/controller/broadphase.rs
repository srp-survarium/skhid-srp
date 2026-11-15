//! [L36] The order the game's broadphase offers its objects to a sweep: `btCollisionWorld::convexSweepTest` 0xA36E0 asks the
//! world's broadphase (`btAxisSweep3`, whose `rayTest` 0x94C70 hands the work to its raycast accelerator, a
//! `btDbvtBroadphase` with one tree: `rayTest` 0xB4FE0 -> `rayTestInternal` 0xB5CA0, `childs[1]` before `childs[0]`).
//! That tree is only ever inserted into and updated, never optimised, so the static objects' order in it follows from
//! how they came in:
//!
//! - each placement is a world object in the map's placement order (the object's index is the placement's), and its proxy
//!   goes in with its shape's `getAabb` at the object's transform (`btDbvtBroadphase::createProxy` 0xB4E00: `btDbvt::insert`
//!   0xAA690 of that box as it is);
//! - the world's first step updates every object's box (`updateAabbs` 0xA01E0 -> `updateSingleAabb` 0x9FF40: the
//!   `getAabb` widened by 0.02, 0xA7AA8C, on each side; a static object straight to the broadphase's `setAabb`, 0xA01BF),
//!   which the accelerator takes as a move (`setAabb` 0xB50E0: the leaf meets the box, the prediction +64h is 0) and, as
//!   the leaf no longer holds the box, reinserts the leaf with the box widened by 0.05 more (`update` 0xAA780, then
//!   0xAA710: out, and in again from the root), each object in turn;
//! - a later update of a static object finds its box held and changes nothing, and the moving bodies' own removals and
//!   insertions leave every other leaf's place in the walk as it was (`removeleaf` puts the sibling where the parent was,
//!   `insertleaf` a new node where a leaf was).
//!
//! Every value is `f32` in the exe's order of operations, in Bullet's frame.
use super::compound_dbvt::{Dbvt, Volume, contains};
use super::exe_gjk::Transform;
use super::{mesh_bvh, solids};
use crate::MapData;
use crate::bullet::Shape;

type V3 = [f32; 3];

/// `updateSingleAabb`'s widening, `gContactBreakingThreshold` (0xA7AA8C).
const CONTACT_THRESHOLD: f32 = 0.02;
/// The margin the accelerator's update adds (`btDbvt::update` with margin, 0xAA780; the tree's leaves hold their proxy's
/// box widened by exactly this, exactness `accel-tree-jumps2.tsv`, 1,955 leaves).
const DBVT_MARGIN: f32 = 0.05;

/// The box of a local box `(lo, hi)` under `t`, as `btTriangleMeshShape::getAabb` 0xA79C0 and `btCompoundShape::getAabb`
/// 0x8A900 make it (each with its margin, 0): the half extents `(hi - lo) * 0.5`, the centre `(lo + hi) * 0.5` placed
/// `((m01 cy + m02 cz) + m00 cx) + ox` (rows 1 and 2 as `(m12 cz + m11 cy) + m10 cx`, `(m22 cz + m21 cy) + m20 cx`), the
/// extents `(|m02| hz + |m01| hy) + |m00| hx` (rows 1 and 2 `(|m11| hy + |m12| hz) + |m10| hx`, `(|m21| hy + |m22| hz) +
/// |m20| hx`; each class sums the two first terms in its own order, which gives the same bits), then the centre less and
/// plus them.
fn placed_box(lo: V3, hi: V3, t: &Transform) -> (V3, V3) {
    let h = [0, 1, 2].map(|k| (hi[k] - lo[k]) * 0.5);
    let c = [0, 1, 2].map(|k| (lo[k] + hi[k]) * 0.5);
    let m = &t.basis;
    let o = t.origin;
    let wc = [
        ((m[0][1] * c[1] + m[0][2] * c[2]) + m[0][0] * c[0]) + o[0],
        ((m[1][2] * c[2] + m[1][1] * c[1]) + m[1][0] * c[0]) + o[1],
        ((m[2][2] * c[2] + m[2][1] * c[1]) + m[2][0] * c[0]) + o[2],
    ];
    let a = |r: usize, k: usize| m[r][k].abs();
    let e = [
        (a(0, 2) * h[2] + a(0, 1) * h[1]) + a(0, 0) * h[0],
        (a(1, 1) * h[1] + a(1, 2) * h[2]) + a(1, 0) * h[0],
        (a(2, 1) * h[1] + a(2, 2) * h[2]) + a(2, 0) * h[0],
    ];
    ([0, 1, 2].map(|k| wc[k] - e[k]), [0, 1, 2].map(|k| e[k] + wc[k]))
}

/// Each placement's proxy box when it is created: its shape's `getAabb` at the object's transform (Bullet's frame).
/// `None` for a placement whose box the port cannot make the exe's way (a compound with a child it has no box for).
pub fn creation_boxes(map: &MapData) -> Vec<Option<(V3, V3)>> {
    let Some(world) = map.bullet.as_ref() else { return Vec::new() };
    map.placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let t = world.object(index);
            let library = &map.libraries[placement.library as usize];
            match world.shape(placement.library) {
                Shape::Empty => None,
                Shape::Convex(convex) => Some(solids::class_shape(convex)?.aabb(t)),
                Shape::Mesh(mesh) => {
                    let corners: Vec<[V3; 3]> = library.faces[mesh.faces.start as usize..mesh.faces.end as usize]
                        .iter()
                        .map(|face| face.corners.map(|i| flip(library.vertices[i as usize].to_array())))
                        .collect();
                    let (lo, hi) = mesh_bvh::local_box(&corners)?;
                    Some(placed_box(lo, hi, t))
                }
                Shape::Compound { children, .. } => {
                    // The compound's local box (+20h/+30h): its children's boxes merged (`recalculateLocalAabb` after the
                    // scaling, then `addChildShape`'s merge for the mesh), each the child's box in its tree; none for no
                    // child (0x8A914: zeros).
                    let (_, boxes) = solids::compound_ranks(library, children)?;
                    let (lo, hi) = boxes.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), b| {
                        (
                            [0, 1, 2].map(|k| if lo[k] > b.0[k] { b.0[k] } else { lo[k] }),
                            [0, 1, 2].map(|k| if hi[k] < b.1[k] { b.1[k] } else { hi[k] }),
                        )
                    });
                    if boxes.is_empty() { Some(placed_box([0.0; 3], [0.0; 3], t)) } else { Some(placed_box(lo, hi, t)) }
                }
            }
        })
        .collect()
}

/// The placements in the order the accelerator's walk offers them after the world's first step (module doc), or `None`
/// where a placement's box is not known ([`creation_boxes`]).
pub fn walk_order(map: &MapData) -> Option<Vec<u32>> {
    let boxes: Vec<(V3, V3)> = creation_boxes(map).into_iter().collect::<Option<_>>()?;
    let mut tree = Dbvt::default();
    let leaves: Vec<usize> = boxes.iter().enumerate().map(|(i, b)| tree.insert(Volume::of(b.0, b.1), i as u32)).collect();
    for (b, &leaf) in boxes.iter().zip(&leaves) {
        let proxy = Volume::of(b.0.map(|v| v - CONTACT_THRESHOLD), b.1.map(|v| v + CONTACT_THRESHOLD));
        let held = tree.volume(leaf);
        // 0xB5183..0xB51B0: a box that no longer meets the leaf is a teleport, reinserted as it is (0xAA710).
        let meets = (0..3).all(|k| !(held.mx[k] < proxy.mi[k] || proxy.mx[k] < held.mi[k]));
        if !meets {
            tree.update(leaf, proxy);
        } else if !contains(&held, &proxy) {
            let widened = Volume::of(
                [0, 1, 2].map(|k| proxy.mi[k] - DBVT_MARGIN),
                [0, 1, 2].map(|k| proxy.mx[k] + DBVT_MARGIN),
            );
            tree.update(leaf, widened);
        }
    }
    Some(tree.offering_order())
}

fn flip(v: V3) -> V3 {
    [v[0], v[1], -v[2]]
}
