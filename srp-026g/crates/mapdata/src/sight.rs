//! Line of sight: a ray tree (bounding volume hierarchy) over triangles,
//! normally the bullet mesh ([`crate::MapData::bullet_triangles`]).
//!
//! Built by median splits on the widest axis of the triangles' centres, four
//! triangles a leaf. A test stops at the first hit.

use std::path::Path;

use crate::built;
use crate::digest::Digest;
use crate::math::Vec3;
use crate::project::Triangle;
use crate::resources::Error;

/// Triangles a leaf holds at most.
const LEAF_SIZE: usize = 4;
/// Traversal stack: median splits keep the depth near log2(n / 4).
const STACK: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Node {
    min: Vec3,
    max: Vec3,
    /// A leaf's first triangle, or a branch's right child (the left child
    /// follows the branch).
    first_or_right: u32,
    /// Triangles in a leaf; 0 for a branch.
    count: u32,
}

/// A triangle kept as its corner and two edges, ready for the ray test.
#[derive(Clone, Copy, Debug)]
struct Prepared {
    a: Vec3,
    e1: Vec3,
    e2: Vec3,
}

pub struct RayTree {
    nodes: Vec<Node>,
    triangles: Vec<Prepared>,
}

/// The first hit of a ray ([`RayTree::ray_hit`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// The distance along the ray.
    pub t: f32,
    /// The triangle's unit normal, on the side the ray came from.
    pub normal: Vec3,
}

impl RayTree {
    pub fn build(triangles: &[Triangle]) -> Self {
        Self::from_corners(triangles.iter().map(|t| t.v))
    }

    pub fn from_corners(corners: impl IntoIterator<Item = [Vec3; 3]>) -> Self {
        let corners: Vec<[Vec3; 3]> = corners
            .into_iter()
            .filter(|t| t.iter().all(|v| v.is_finite()))
            .collect();
        let centres: Vec<Vec3> = corners
            .iter()
            .map(|t| (t[0] + t[1] + t[2]) * (1.0 / 3.0))
            .collect();
        let mut order: Vec<u32> = (0..corners.len() as u32).collect();
        let mut nodes = Vec::with_capacity(corners.len() / 2 + 1);
        if !order.is_empty() {
            build_node(&mut nodes, &corners, &centres, &mut order, 0);
        }
        let triangles = order
            .iter()
            .map(|&i| {
                let [a, b, c] = corners[i as usize];
                Prepared {
                    a,
                    e1: b - a,
                    e2: c - a,
                }
            })
            .collect();
        Self { nodes, triangles }
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Bytes the tree holds.
    pub fn memory_bytes(&self) -> usize {
        self.nodes.len() * std::mem::size_of::<Node>()
            + self.triangles.len() * std::mem::size_of::<Prepared>()
    }

    /// What the tree answers, as one number ([`crate::digest`]): every
    /// node's box and children, every triangle as the ray test sees it.
    pub fn digest(&self) -> u64 {
        fn vec3(digest: &mut Digest, v: Vec3) {
            digest.f32(v.x);
            digest.f32(v.y);
            digest.f32(v.z);
        }
        let mut digest = Digest::new();
        digest.len(self.nodes.len());
        for node in &self.nodes {
            vec3(&mut digest, node.min);
            vec3(&mut digest, node.max);
            digest.u32(node.first_or_right);
            digest.u32(node.count);
        }
        digest.len(self.triangles.len());
        for triangle in &self.triangles {
            vec3(&mut digest, triangle.a);
            vec3(&mut digest, triangle.e1);
            vec3(&mut digest, triangle.e2);
        }
        digest.finish()
    }

    /// Writes the tree to `path` as the tree of `key` ([`crate::built`],
    /// [`crate::sight_key`]).
    pub fn write(&self, path: &Path, key: u64) -> Result<(), Error> {
        built::write(path, built::SIGHT_MAGIC, key, self.digest(), |out| {
            out.plain(&self.nodes, |node| {
                let mut bytes = [0u8; 32];
                let words = [
                    node.min.x.to_bits(),
                    node.min.y.to_bits(),
                    node.min.z.to_bits(),
                    node.max.x.to_bits(),
                    node.max.y.to_bits(),
                    node.max.z.to_bits(),
                    node.first_or_right,
                    node.count,
                ];
                for (chunk, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(words) {
                    *chunk = word.to_le_bytes();
                }
                bytes
            })?;
            out.plain(&self.triangles, |triangle| {
                let mut bytes = [0u8; 36];
                let vs = [triangle.a, triangle.e1, triangle.e2];
                let words = vs.iter().flat_map(|v| [v.x, v.y, v.z]).map(f32::to_bits);
                for (chunk, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(words) {
                    *chunk = word.to_le_bytes();
                }
                bytes
            })
        })
    }

    /// Reads the tree of `key` that [`Self::write`] wrote: checked node by
    /// node, then by its digest.
    pub fn read(path: &Path, key: u64) -> Result<Self, Error> {
        fn words<const N: usize, const W: usize>(bytes: [u8; N]) -> [u32; W] {
            let mut words = [0u32; W];
            for (word, chunk) in words.iter_mut().zip(bytes.as_chunks::<4>().0) {
                *word = u32::from_le_bytes(*chunk);
            }
            words
        }
        let vec3 = |w: &[u32]| {
            Vec3::new(
                f32::from_bits(w[0]),
                f32::from_bits(w[1]),
                f32::from_bits(w[2]),
            )
        };
        built::read(
            path,
            built::SIGHT_MAGIC,
            key,
            |input| {
                let nodes = input.plain(|bytes: [u8; 32]| {
                    let w: [u32; 8] = words(bytes);
                    Node {
                        min: vec3(&w[0..3]),
                        max: vec3(&w[3..6]),
                        first_or_right: w[6],
                        count: w[7],
                    }
                })?;
                let triangles = input.plain(|bytes: [u8; 36]| {
                    let w: [u32; 9] = words(bytes);
                    Prepared {
                        a: vec3(&w[0..3]),
                        e1: vec3(&w[3..6]),
                        e2: vec3(&w[6..9]),
                    }
                })?;
                let tree = Self { nodes, triangles };
                tree.check()?;
                Ok(tree)
            },
            Self::digest,
        )
    }

    /// Whether every index of a tree read from a file is in range: a
    /// branch's children after it, a leaf's triangles in the list.
    fn check(&self) -> Result<(), String> {
        let (nodes, triangles) = (self.nodes.len(), self.triangles.len());
        for (index, node) in self.nodes.iter().enumerate() {
            let fine = if node.count == 0 {
                let right = node.first_or_right as usize;
                index + 1 < nodes && right > index + 1 && right < nodes
            } else {
                (node.first_or_right as usize)
                    .checked_add(node.count as usize)
                    .is_some_and(|end| end <= triangles)
            };
            if !fine {
                return Err(format!("node {index} is out of range"));
            }
        }
        Ok(())
    }

    /// Whether nothing lies between `a` and `b` (both excluded by 1 cm).
    pub fn visible(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let length = d.length();
        if !length.is_finite() {
            return false;
        }
        if length <= 0.02 {
            return true;
        }
        !self.any_hit(a, d * (1.0 / length), 0.01, length - 0.01)
    }

    /// The distance to the first hit along `dir` (unit) within `max_t`.
    pub fn ray_cast(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        self.first_hit(origin, dir, max_t).map(|(t, _)| t)
    }

    /// The first hit along `dir` (unit) within `max_t`, with its triangle's
    /// normal: unit, on the side the ray came from.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<RayHit> {
        let (t, tri) = self.first_hit(origin, dir, max_t)?;
        let normal = tri.e1.cross(tri.e2);
        let normal = normal * (1.0 / normal.length());
        Some(RayHit {
            t,
            normal: if normal.dot(dir) > 0.0 {
                -normal
            } else {
                normal
            },
        })
    }

    fn first_hit(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<(f32, Prepared)> {
        let mut best = None;
        self.walk(origin, dir, 0.0, max_t, &mut |t, tri| {
            best = Some((t, *tri));
            false
        });
        best
    }

    fn any_hit(&self, origin: Vec3, dir: Vec3, min_t: f32, max_t: f32) -> bool {
        let mut hit = false;
        self.walk(origin, dir, min_t, max_t, &mut |_, _| {
            hit = true;
            true
        });
        hit
    }

    /// Visits hits nearer than the best so far; `found(t, triangle)` returns
    /// whether to stop.
    fn walk(
        &self,
        origin: Vec3,
        dir: Vec3,
        min_t: f32,
        mut max_t: f32,
        found: &mut dyn FnMut(f32, &Prepared) -> bool,
    ) {
        if self.nodes.is_empty() || !origin.is_finite() || !dir.is_finite() {
            return;
        }
        let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);
        let mut stack = [0u32; STACK];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let index = stack[top] as usize;
            let node = &self.nodes[index];
            if !slab(origin, inv, node.min, node.max, max_t) {
                continue;
            }
            if node.count > 0 {
                let first = node.first_or_right as usize;
                for tri in &self.triangles[first..first + node.count as usize] {
                    if let Some(t) = ray_triangle(origin, dir, tri)
                        && t > min_t
                        && t < max_t
                    {
                        max_t = t;
                        if found(t, tri) {
                            return;
                        }
                    }
                }
            } else if top + 2 <= STACK {
                stack[top] = node.first_or_right;
                stack[top + 1] = index as u32 + 1;
                top += 2;
            }
        }
    }
}

fn build_node(
    nodes: &mut Vec<Node>,
    corners: &[[Vec3; 3]],
    centres: &[Vec3],
    order: &mut [u32],
    first: usize,
) -> usize {
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = -min;
    let mut centre_min = min;
    let mut centre_max = max;
    for &i in order.iter() {
        for v in corners[i as usize] {
            min = min.min(v);
            max = max.max(v);
        }
        centre_min = centre_min.min(centres[i as usize]);
        centre_max = centre_max.max(centres[i as usize]);
    }
    let index = nodes.len();
    nodes.push(Node {
        min,
        max,
        first_or_right: first as u32,
        count: order.len() as u32,
    });
    if order.len() <= LEAF_SIZE {
        return index;
    }
    let extent = centre_max - centre_min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let middle = order.len() / 2;
    order.select_nth_unstable_by(middle, |&a, &b| {
        centres[a as usize]
            .axis(axis)
            .total_cmp(&centres[b as usize].axis(axis))
    });
    let (lower, upper) = order.split_at_mut(middle);
    build_node(nodes, corners, centres, lower, first);
    let right = build_node(nodes, corners, centres, upper, first + middle);
    nodes[index].first_or_right = right as u32;
    nodes[index].count = 0;
    index
}

/// Whether the ray meets the box before `max_t`.
fn slab(origin: Vec3, inv: Vec3, min: Vec3, max: Vec3, max_t: f32) -> bool {
    let mut near = 0.0f32;
    let mut far = max_t;
    for axis in 0..3 {
        let (o, i) = (origin.axis(axis), inv.axis(axis));
        let t1 = (min.axis(axis) - o) * i;
        let t2 = (max.axis(axis) - o) * i;
        // NaN (a flat box along a zero direction) leaves the bounds as they are.
        near = near.max(t1.min(t2));
        far = far.min(t1.max(t2));
    }
    near <= far
}

/// Möller-Trumbore; both faces count.
fn ray_triangle(origin: Vec3, dir: Vec3, tri: &Prepared) -> Option<f32> {
    let p = dir.cross(tri.e2);
    let det = tri.e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - tri.a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(tri.e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(tri.e2.dot(q) * inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall 2 m high in the plane z = 0, from x = -1 to 1.
    fn wall() -> [[Vec3; 3]; 2] {
        let (a, b) = (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let (c, d) = (Vec3::new(1.0, 2.0, 0.0), Vec3::new(-1.0, 2.0, 0.0));
        [[a, b, c], [a, c, d]]
    }

    #[test]
    fn a_wall_blocks_sight_through_it_only() {
        let tree = RayTree::from_corners(wall());
        let eye = Vec3::new(0.0, 1.0, -5.0);
        assert!(!tree.visible(eye, Vec3::new(0.0, 1.0, 5.0)));
        assert!(tree.visible(eye, Vec3::new(0.0, 3.5, 5.0)));
        assert!(tree.visible(eye, Vec3::new(3.0, 1.0, 5.0)));
        // Short of the wall, and from the other side.
        assert!(tree.visible(eye, Vec3::new(0.0, 1.0, -0.5)));
        assert!(!tree.visible(Vec3::new(0.0, 1.0, 5.0), eye));
        let t = tree.ray_cast(eye, Vec3::new(0.0, 0.0, 1.0), 100.0).unwrap();
        assert!((t - 5.0).abs() < 1e-5);
        assert_eq!(tree.ray_cast(eye, Vec3::new(0.0, 0.0, 1.0), 4.0), None);
    }

    /// The build gives the same tree every time, and the digest is pinned
    /// ([`crate::grid`]'s test says why).
    #[test]
    fn the_build_and_its_digest_are_fixed() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let corners: Vec<[Vec3; 3]> = (0..500)
            .map(|_| {
                let c = Vec3::new(next() * 50.0, next() * 10.0, next() * 50.0);
                [
                    c,
                    c + Vec3::new(next(), next(), next()),
                    c + Vec3::new(next(), next(), next()),
                ]
            })
            .collect();
        let tree = RayTree::from_corners(corners.clone());
        assert_eq!(
            tree.digest(),
            RayTree::from_corners(corners.clone()).digest()
        );
        assert_eq!(tree.digest(), PINNED_FIELD);
        let mut moved = RayTree::from_corners(corners);
        moved.triangles[7].e1.y += 0.001;
        assert_ne!(moved.digest(), tree.digest());
    }

    const PINNED_FIELD: u64 = 8_340_484_153_156_595_831;

    #[test]
    fn a_written_tree_reads_back_the_same_and_a_damaged_one_is_refused() {
        let dir = std::env::temp_dir().join(format!("srp-sight-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(crate::built::sight_file_name(3));
        let mut corners = wall().to_vec();
        for k in 0..20 {
            let shift = Vec3::new(k as f32 * 3.0, 0.0, k as f32);
            corners.extend(wall().map(|t| t.map(|v| v + shift)));
        }
        let tree = RayTree::from_corners(corners);
        tree.write(&path, 3).unwrap();
        let read = RayTree::read(&path, 3).unwrap();
        assert_eq!(read.digest(), tree.digest());
        let eye = Vec3::new(0.0, 1.0, -5.0);
        assert_eq!(
            read.visible(eye, Vec3::new(0.0, 1.0, 5.0)),
            tree.visible(eye, Vec3::new(0.0, 1.0, 5.0))
        );
        assert!(RayTree::read(&path, 4).is_err());
        let bytes = std::fs::read(&path).unwrap();
        for at in (0..bytes.len()).step_by(13).chain([bytes.len() - 1]) {
            let mut damaged = bytes.clone();
            damaged[at] ^= 0x41;
            std::fs::write(&path, &damaged).unwrap();
            assert!(RayTree::read(&path, 3).is_err(), "byte {at}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_tree_agrees_with_testing_every_triangle() {
        // A field of small random triangles; a few hundred rays.
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut corners = Vec::new();
        for _ in 0..2000 {
            let c = Vec3::new(next() * 100.0, next() * 20.0, next() * 100.0);
            corners.push([
                c,
                c + Vec3::new(next() * 3.0, next() * 3.0, next() * 3.0),
                c + Vec3::new(next() * 3.0, next() * 3.0, next() * 3.0),
            ]);
        }
        let tree = RayTree::from_corners(corners.clone());
        assert_eq!(tree.triangle_count(), 2000);
        let prepared: Vec<Prepared> = corners
            .iter()
            .map(|&[a, b, c]| Prepared {
                a,
                e1: b - a,
                e2: c - a,
            })
            .collect();
        let mut blocked = 0;
        for _ in 0..500 {
            let a = Vec3::new(next() * 100.0, next() * 20.0, next() * 100.0);
            let b = Vec3::new(next() * 100.0, next() * 20.0, next() * 100.0);
            let d = b - a;
            let length = d.length();
            let dir = d * (1.0 / length);
            let brute = prepared
                .iter()
                .any(|t| ray_triangle(a, dir, t).is_some_and(|t| t > 0.01 && t < length - 0.01));
            assert_eq!(tree.visible(a, b), !brute);
            blocked += brute as usize;
        }
        assert!(blocked > 50 && blocked < 450, "{blocked}");
    }

    #[test]
    fn a_hit_names_the_triangles_normal_on_the_rays_side() {
        let tree = RayTree::from_corners(wall());
        let ahead = Vec3::new(0.0, 0.0, 1.0);
        let from_front = tree
            .ray_hit(Vec3::new(0.2, 1.0, -5.0), ahead, 100.0)
            .unwrap();
        assert!((from_front.t - 5.0).abs() < 1e-5);
        assert_eq!(from_front.normal, Vec3::new(0.0, 0.0, -1.0));
        let back = Vec3::new(0.0, 0.0, -1.0);
        let from_behind = tree.ray_hit(Vec3::new(0.2, 1.0, 5.0), back, 100.0).unwrap();
        assert_eq!(from_behind.normal, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(tree.ray_hit(Vec3::new(0.2, 1.0, -5.0), ahead, 4.0), None);
    }

    #[test]
    fn the_first_hit_is_the_nearest_triangle_and_its_normal() {
        // A field of small random triangles, the nearest of every ray found by
        // testing them all.
        let mut seed = 0x1234_5678_9abc_def1u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut corners = Vec::new();
        for _ in 0..2000 {
            let c = Vec3::new(next() * 100.0, next() * 20.0, next() * 100.0);
            corners.push([
                c,
                c + Vec3::new(next() * 3.0, next() * 3.0, next() * 3.0),
                c + Vec3::new(next() * 3.0, next() * 3.0, next() * 3.0),
            ]);
        }
        let tree = RayTree::from_corners(corners.clone());
        let prepared: Vec<Prepared> = corners
            .iter()
            .map(|&[a, b, c]| Prepared {
                a,
                e1: b - a,
                e2: c - a,
            })
            .collect();
        let mut hits = 0;
        for _ in 0..500 {
            let a = Vec3::new(next() * 100.0, next() * 20.0, next() * 100.0);
            let b = Vec3::new(next() * 100.0, next() * 20.0, next() * 100.0);
            let dir = (b - a) * (1.0 / (b - a).length());
            let nearest = prepared
                .iter()
                .filter_map(|tri| ray_triangle(a, dir, tri).map(|t| (t, tri)))
                .filter(|(t, _)| *t > 0.0 && *t < 200.0)
                .min_by(|x, y| x.0.total_cmp(&y.0));
            let hit = tree.ray_hit(a, dir, 200.0);
            assert_eq!(hit.map(|h| h.t), nearest.map(|n| n.0));
            assert_eq!(tree.ray_cast(a, dir, 200.0), nearest.map(|n| n.0));
            if let (Some(hit), Some((_, tri))) = (hit, nearest) {
                let n = tri.e1.cross(tri.e2);
                let n = n * (1.0 / n.length());
                let n = if n.dot(dir) > 0.0 { -n } else { n };
                assert_eq!(hit.normal, n);
                assert!((hit.normal.length() - 1.0).abs() < 1e-5);
                assert!(hit.normal.dot(dir) <= 0.0);
                hits += 1;
            }
        }
        assert!(hits > 100, "{hits}");
    }

    #[test]
    fn an_empty_tree_sees_everything() {
        let tree = RayTree::from_corners([]);
        assert!(tree.visible(Vec3::ZERO, Vec3::Y));
        assert_eq!(tree.ray_cast(Vec3::ZERO, Vec3::Y, 10.0), None);
    }
}
