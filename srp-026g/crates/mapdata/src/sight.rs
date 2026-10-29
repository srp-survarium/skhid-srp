//! Line of sight: a ray tree (bounding volume hierarchy) over triangles,
//! normally the bullet mesh ([`crate::MapData::bullet_triangles`]).
//!
//! Built by binned surface-area-heuristic splits (16 bins on each axis of the
//! triangles' centres, the cheapest plane), four triangles a leaf at most; a
//! subtree deeper than [`SAH_DEPTH`] falls back to median splits, which
//! bound the depth. A test visits the child the ray enters first and stops at
//! the first hit. The answers do not depend on the tree: a boolean "any
//! triangle" and a minimum of t over the same triangles, each from the same
//! triangle test; the boxes are padded by [`PAD`] so that no ray that meets a
//! triangle is turned away by rounding at the edge of a box.

use std::path::Path;

use crate::built;
use crate::digest::Digest;
use crate::math::Vec3;
use crate::pierce::{Armor, Crossing, DamageShare, FaceArmor, Load, Pierce};
use crate::project::Triangle;
use crate::resources::Error;

/// Triangles a leaf holds at most.
const LEAF_SIZE: usize = 4;
/// Traversal stack: the depth of a tree stays under it (see [`SAH_DEPTH`]).
const STACK: usize = 64;
/// Bins a split plane is chosen among on each axis.
const BINS: usize = 16;
/// Levels that split by the surface area heuristic. Under them the median
/// splits keep the depth near log2(n / 4), so a tree is at most this and 20
/// more deep for two million triangles, inside [`STACK`].
const SAH_DEPTH: usize = 32;
/// Metres every box is grown by on every side.
const PAD: f32 = 0.001;
/// Front faces a line is followed through at most ([`RayTree::fly`]); back faces and free armor are skipped inside
/// the walk and cost none of them. More than this is not a line a bullet crosses: it stops.
const MAX_CROSSINGS: usize = 64;
/// Metres past a counted front face before the next hit is looked for: the two triangles that share the edge a line
/// crosses are one surface. Faces skipped as back faces or free armor leave no gap.
const CROSSING_GAP: f32 = 0.002;
/// The metres at both ends of a bots' line that are not tested, as [`RayTree::visible`] leaves them.
const LINE_MARGIN_M: f32 = 0.01;
/// The side of a triangle's normal `e1 x e2` the shooter is on for a front face: `+1` when the ray comes from the side
/// the normal points to (the ray goes against it). LIKELY: the exe takes the hit's raw normal and branches on
/// `dot(normal, dir) > 0` for a back face (0x32D69B, `kF_KeepUnflippedNormal`: Bullet then hands over the triangle's
/// own normal); 1,367 of 1,541 big collision libraries on five maps are wound with `e1 x e2` out of the body
/// (`tests/pierce_maps.rs`); whether the client's physics frame (z negated) turns that round is not read. For a
/// closed body the sign does not matter (the near or the far face is crossed once either way); it decides which side
/// of a one-sided sheet counts. The client test T-PEN-2 settles it.
const FRONT_NORMAL_SIGN: f32 = 1.0;
/// A branch's `first_or_right`: its right child in the low 30 bits, the axis
/// it was split on in the top two.
const RIGHT_MASK: u32 = (1 << 30) - 1;

#[derive(Clone, Copy, Debug)]
struct Node {
    min: Vec3,
    max: Vec3,
    /// A leaf's first triangle, or a branch's right child (the left child
    /// follows the branch) and its split axis in the top two bits
    /// ([`RIGHT_MASK`]).
    first_or_right: u32,
    /// Triangles in a leaf; 0 for a branch.
    count: u32,
}

impl Node {
    /// A branch's right child.
    fn right(&self) -> usize {
        (self.first_or_right & RIGHT_MASK) as usize
    }

    /// The axis a branch was split on (0 for a tree built by medians).
    fn axis(&self) -> usize {
        (self.first_or_right >> 30) as usize
    }
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
    /// What a bullet meets in each of `triangles` (the same order).
    armor: Vec<FaceArmor>,
}

/// A line followed by [`RayTree::fly`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Followed {
    /// The share of the damage that reaches the end (against what an empty line would give).
    pub share: DamageShare,
    /// What the bullet carries at the end (nothing if it was stopped or given up at the `floor`).
    pub load: Load,
    /// What stopped the bullet (none: it was not stopped, or the share fell to the `floor` asked).
    pub stopped_by: Option<FaceArmor>,
    /// The front faces it went through.
    pub crossed: u32,
}

/// What the faces of a bullet mesh are: armor 0 (free), a material with armor, or no material (unknown: stops every
/// bullet, as a file before format 5 reads every face).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmorCensus {
    pub total: usize,
    pub free: usize,
    pub armored: usize,
    pub unknown: usize,
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

    /// A tree whose every triangle is of unknown material (it stops every bullet from either side): line of sight alone.
    pub fn from_corners(corners: impl IntoIterator<Item = [Vec3; 3]>) -> Self {
        Self::assemble(corners.into_iter().map(|c| (c, FaceArmor::Unknown)), true)
    }

    /// A tree of triangles with the armor of the material of each.
    pub fn from_armored_corners(corners: impl IntoIterator<Item = ([Vec3; 3], FaceArmor)>) -> Self {
        Self::assemble(corners, true)
    }

    /// The tree as it was built before [`crate::SIGHT_ALGO`] 2 (median splits, tight boxes), kept for the tests that
    /// prove the surface-area tree answers as it did.
    #[doc(hidden)]
    pub fn from_corners_median(corners: impl IntoIterator<Item = [Vec3; 3]>) -> Self {
        Self::assemble(corners.into_iter().map(|c| (c, FaceArmor::Unknown)), false)
    }

    fn assemble(corners: impl IntoIterator<Item = ([Vec3; 3], FaceArmor)>, sah: bool) -> Self {
        let (corners, armor): (Vec<[Vec3; 3]>, Vec<FaceArmor>) = corners
            .into_iter()
            .filter(|(t, _)| t.iter().all(|v| v.is_finite()))
            .unzip();
        let centres: Vec<Vec3> = corners
            .iter()
            .map(|t| (t[0] + t[1] + t[2]) * (1.0 / 3.0))
            .collect();
        let mut order: Vec<u32> = (0..corners.len() as u32).collect();
        let mut nodes = Vec::with_capacity(corners.len() / 2 + 1);
        if !order.is_empty() {
            build_node(&mut nodes, &corners, &centres, &mut order, 0, 0, sah);
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
        let armor = order.iter().map(|&i| armor[i as usize]).collect();
        Self { nodes, triangles, armor }
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Bytes the tree holds.
    pub fn memory_bytes(&self) -> usize {
        self.nodes.len() * std::mem::size_of::<Node>()
            + self.triangles.len() * std::mem::size_of::<Prepared>()
            + self.armor.len() * std::mem::size_of::<FaceArmor>()
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
        for armor in &self.armor {
            digest.f32(armor.to_wire());
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
            })?;
            out.plain(&self.armor, |armor| armor.to_wire().to_bits().to_le_bytes())
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
                let bad = std::cell::Cell::new(None);
                let armor = input.plain(|bytes: [u8; 4]| {
                    let value = f32::from_bits(u32::from_le_bytes(bytes));
                    FaceArmor::from_wire(value).unwrap_or_else(|| {
                        bad.set(Some(value));
                        FaceArmor::Unknown
                    })
                })?;
                if let Some(value) = bad.get() {
                    return Err(format!("armor {value} is no armor"));
                }
                let tree = Self { nodes, triangles, armor };
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
        if self.armor.len() != triangles {
            return Err(format!("{} armors for {triangles} triangles", self.armor.len()));
        }
        for (index, node) in self.nodes.iter().enumerate() {
            let fine = if node.count == 0 {
                let right = node.right();
                index + 1 < nodes && right > index + 1 && right < nodes && node.axis() < 3
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
        self.first_hit(origin, dir, 0.0, max_t).map(|(t, _)| t)
    }

    /// The first hit along `dir` (unit) within `max_t`, with its triangle's
    /// normal: unit, on the side the ray came from.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<RayHit> {
        let (t, index) = self.first_hit(origin, dir, 0.0, max_t)?;
        let tri = &self.triangles[index];
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

    /// The share of a bullet's damage that reaches `b` from `a`: the client's penetration
    /// ([`crate::pierce`], `bullet::try_pierce`). The line is followed surface by surface; each
    /// front face of armor `R` lets `k = 1 - (1 - clamp((P - R) / R, 0, 1))^2` of the damage and of the
    /// pierce `P` through, and a face with `P <= R` stops the bullet. Back faces and armor 0 cost nothing, so
    /// the share is [`DamageShare::ALL`] exactly when no front face of a known material and no face of unknown
    /// material ([`FaceArmor::Unknown`]: stops from either side) meets the line. A tree built without materials
    /// ([`Self::from_corners`]) stops at its first face, as [`Self::visible`] does.
    ///
    /// Both ends are excluded by 1 cm, as in [`Self::visible`].
    pub fn damage_share(&self, a: Vec3, b: Vec3, pierce: Pierce) -> DamageShare {
        self.follow(a, b, pierce, 0.0).share
    }

    /// Whether more than `over` of a bullet's damage reaches `b` from `a` ([`Self::damage_share`] over `over`), the
    /// cover tests' question. The same answer, sooner: the line is followed only while the share can still be over.
    pub fn reaches_over(&self, a: Vec3, b: Vec3, pierce: Pierce, over: f32) -> bool {
        self.follow(a, b, pierce, over).share.value() > over
    }

    /// [`Self::damage_share`], given up when the share is `floor` or less (it only falls): then a share at or under
    /// `floor`, not necessarily the one at the end; with what stopped the bullet and the faces it crossed (for the
    /// counts). A pierce that does not change along the line.
    pub fn follow(&self, a: Vec3, b: Vec3, pierce: Pierce, floor: f32) -> Followed {
        let load = Load { damage: 1.0, pierce: pierce.value() };
        self.fly(a, b, &|_| load, LINE_MARGIN_M, floor)
    }

    /// A bullet's flight from `a` to `b`, the way the client flies it: `flight(metres)` is what the bullet carries
    /// that far from `a` if nothing took any of it (`damage::bullet_at`); at every counted front face its damage and
    /// pierce become the lesser of what they were and `flight` there (`update_bullet_position`), and then `k` of that
    /// (`try_pierce`). `margin` metres at both ends are not tested. Given up when the share falls to `floor`.
    pub fn fly(&self, a: Vec3, b: Vec3, flight: &dyn Fn(f32) -> Load, margin: f32, floor: f32) -> Followed {
        let d = b - a;
        let length = d.length();
        let gone = Load { damage: 0.0, pierce: 0.0 };
        if !length.is_finite() {
            return Followed { share: DamageShare::NONE, load: gone, stopped_by: None, crossed: 0 };
        }
        if length <= (2.0 * margin).max(1e-4) {
            return Followed { share: DamageShare::ALL, load: flight(length), stopped_by: None, crossed: 0 };
        }
        let dir = d * (1.0 / length);
        let end = length - margin;
        let start = flight(0.0);
        let (mut damage, mut pierce, mut from, mut crossed) = (start.damage, start.pierce, margin, 0);
        // A face counts when its material is unknown (from either side), or it has armor and faces the shooter.
        let counts = |index: usize| match self.armor[index] {
            FaceArmor::Unknown => true,
            FaceArmor::Material(armor) => armor != Armor::FREE && self.faces_shooter(index, dir),
        };
        let mut too_many = true;
        for _ in 0..MAX_CROSSINGS {
            let Some((t, index)) = self.first_hit_where(a, dir, from, end, &counts) else {
                too_many = false;
                break;
            };
            from = t + CROSSING_GAP;
            let here = flight(t);
            damage = damage.min(here.damage);
            pierce = pierce.min(here.pierce);
            let stopped = |face: FaceArmor| Followed {
                share: DamageShare::NONE,
                load: gone,
                stopped_by: Some(face),
                crossed,
            };
            match self.armor[index] {
                FaceArmor::Unknown => return stopped(FaceArmor::Unknown),
                FaceArmor::Material(armor) => match armor.crossed_by(pierce) {
                    Crossing::Stopped => return stopped(self.armor[index]),
                    Crossing::Through { k } => {
                        damage *= k;
                        pierce *= k;
                        crossed += 1;
                        let share = DamageShare::of(damage, here.damage);
                        if share.value() <= floor {
                            return Followed { share, load: Load { damage, pierce }, stopped_by: None, crossed };
                        }
                    }
                },
            }
        }
        if too_many {
            // This many surfaces is not a line a bullet crosses.
            return Followed { share: DamageShare::NONE, load: gone, stopped_by: None, crossed };
        }
        let at_end = flight(length);
        let load = Load { damage: damage.min(at_end.damage), pierce: pierce.min(at_end.pierce) };
        Followed { share: DamageShare::of(load.damage, at_end.damage), load, stopped_by: None, crossed }
    }

    /// The armor of the bullet mesh by kind, for the logs ([`ArmorCensus`]).
    pub fn armor_census(&self) -> ArmorCensus {
        let mut census = ArmorCensus { total: self.armor.len(), ..ArmorCensus::default() };
        for armor in &self.armor {
            match armor {
                FaceArmor::Unknown => census.unknown += 1,
                FaceArmor::Material(armor) if *armor == Armor::FREE => census.free += 1,
                FaceArmor::Material(_) => census.armored += 1,
            }
        }
        census
    }

    /// Whether triangle `index` is a front face for a ray along `dir`: the ray comes from the side
    /// its normal ([`FRONT_NORMAL_SIGN`]) points to.
    fn faces_shooter(&self, index: usize, dir: Vec3) -> bool {
        let tri = &self.triangles[index];
        FRONT_NORMAL_SIGN * tri.e1.cross(tri.e2).dot(dir) < 0.0
    }

    fn first_hit(&self, origin: Vec3, dir: Vec3, min_t: f32, max_t: f32) -> Option<(f32, usize)> {
        self.first_hit_where(origin, dir, min_t, max_t, &|_| true)
    }

    /// The nearest hit among the triangles `accept` takes (judged before a hit narrows the search, so a triangle
    /// that is skipped never hides one behind it at the same place).
    fn first_hit_where(
        &self,
        origin: Vec3,
        dir: Vec3,
        min_t: f32,
        max_t: f32,
        accept: &dyn Fn(usize) -> bool,
    ) -> Option<(f32, usize)> {
        let mut best = None;
        self.walk(origin, dir, min_t, max_t, accept, &mut |t, index| {
            best = Some((t, index));
            false
        });
        best
    }

    fn any_hit(&self, origin: Vec3, dir: Vec3, min_t: f32, max_t: f32) -> bool {
        let mut hit = false;
        self.walk(origin, dir, min_t, max_t, &|_| true, &mut |_, _| {
            hit = true;
            true
        });
        hit
    }

    /// [`Self::ray_cast`] by testing every triangle: what the tree must give (for tests).
    #[doc(hidden)]
    pub fn ray_cast_every_triangle(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        let mut best = None;
        let mut limit = max_t;
        for tri in &self.triangles {
            if let Some(t) = ray_triangle(origin, dir, tri)
                && t > 0.0
                && t < limit
            {
                limit = t;
                best = Some(t);
            }
        }
        best
    }

    /// [`Self::visible`] by testing every triangle (for tests).
    #[doc(hidden)]
    pub fn visible_every_triangle(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let length = d.length();
        if !length.is_finite() {
            return false;
        }
        if length <= 0.02 {
            return true;
        }
        let dir = d * (1.0 / length);
        !self
            .triangles
            .iter()
            .any(|tri| ray_triangle(a, dir, tri).is_some_and(|t| t > 0.01 && t < length - 0.01))
    }

    /// Visits hits nearer than the best so far; `found(t, triangle's index)`
    /// returns whether to stop.
    fn walk(
        &self,
        origin: Vec3,
        dir: Vec3,
        min_t: f32,
        mut max_t: f32,
        accept: &dyn Fn(usize) -> bool,
        found: &mut dyn FnMut(f32, usize) -> bool,
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
            if !slab(origin, inv, node.min, node.max, min_t, max_t) {
                continue;
            }
            if node.count > 0 {
                let first = node.first_or_right as usize;
                for (offset, tri) in self.triangles[first..first + node.count as usize].iter().enumerate() {
                    if let Some(t) = ray_triangle(origin, dir, tri)
                        && t > min_t
                        && t < max_t
                        && accept(first + offset)
                    {
                        max_t = t;
                        if found(t, first + offset) {
                            return;
                        }
                    }
                }
            } else if top + 2 <= STACK {
                // The child the ray enters first is popped first.
                let (left, right) = (index as u32 + 1, node.right() as u32);
                let (near, far) = if dir.axis(node.axis()) < 0.0 { (right, left) } else { (left, right) };
                stack[top] = far;
                stack[top + 1] = near;
                top += 2;
            }
        }
    }
}

/// Builds the node over `order` (triangle indexes, rearranged in place so that a node's triangles are the run from
/// `first`) and its subtrees; returns its index. `sah`: split by the surface area heuristic down to [`SAH_DEPTH`],
/// grow the boxes by [`PAD`] and note the axis; else (the tree before `SIGHT_ALGO` 2) medians and tight boxes.
fn build_node(
    nodes: &mut Vec<Node>,
    corners: &[[Vec3; 3]],
    centres: &[Vec3],
    order: &mut [u32],
    first: usize,
    depth: usize,
    sah: bool,
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
    let grow = if sah { Vec3::new(PAD, PAD, PAD) } else { Vec3::ZERO };
    let index = nodes.len();
    nodes.push(Node {
        min: min - grow,
        max: max + grow,
        first_or_right: first as u32,
        count: order.len() as u32,
    });
    if order.len() <= LEAF_SIZE {
        return index;
    }
    let extent = centre_max - centre_min;
    let widest = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let (axis, middle) = match sah.then(|| sah_split(corners, centres, order, centre_min, centre_max, depth)).flatten() {
        Some(split) => split,
        None => {
            let middle = order.len() / 2;
            order.select_nth_unstable_by(middle, |&a, &b| {
                centres[a as usize]
                    .axis(widest)
                    .total_cmp(&centres[b as usize].axis(widest))
            });
            (widest, middle)
        }
    };
    let (lower, upper) = order.split_at_mut(middle);
    build_node(nodes, corners, centres, lower, first, depth + 1, sah);
    let right = build_node(nodes, corners, centres, upper, first + middle, depth + 1, sah);
    debug_assert!(right <= RIGHT_MASK as usize);
    nodes[index].first_or_right = right as u32 | if sah { (axis as u32) << 30 } else { 0 };
    nodes[index].count = 0;
    index
}

/// The surface area heuristic's split of `order` (more than [`LEAF_SIZE`] triangles): the axis and plane, of the 16 bins
/// on each axis of the centres' box, with the least area of the two sides' boxes times the triangles in them. The
/// triangles are partitioned in place, the lower side first; gives the axis and the lower side's count. `None` below
/// [`SAH_DEPTH`] levels, or when the centres are all one point (the median split does it then).
fn sah_split(
    corners: &[[Vec3; 3]],
    centres: &[Vec3],
    order: &mut [u32],
    centre_min: Vec3,
    centre_max: Vec3,
    depth: usize,
) -> Option<(usize, usize)> {
    if depth >= SAH_DEPTH {
        return None;
    }
    let area = |min: Vec3, max: Vec3| {
        let d = max - min;
        d.x * d.y + d.y * d.z + d.z * d.x
    };
    let bin_of = |centre: f32, origin: f32, scale: f32| (((centre - origin) * scale) as usize).min(BINS - 1);
    let empty = (Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY), Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY));
    let mut best: Option<(f32, usize, usize)> = None;
    for axis in 0..3 {
        let width = centre_max.axis(axis) - centre_min.axis(axis);
        if width.is_nan() || width <= 0.0 {
            continue;
        }
        let (origin, scale) = (centre_min.axis(axis), BINS as f32 / width);
        let mut count = [0usize; BINS];
        let mut boxes = [empty; BINS];
        for &i in order.iter() {
            let bin = bin_of(centres[i as usize].axis(axis), origin, scale);
            count[bin] += 1;
            for v in corners[i as usize] {
                boxes[bin] = (boxes[bin].0.min(v), boxes[bin].1.max(v));
            }
        }
        // The lower side up to each plane, then the upper from each.
        let mut lower = [(0.0f32, 0usize); BINS - 1];
        let (mut sum, mut n) = (empty, 0usize);
        for plane in 0..BINS - 1 {
            sum = (sum.0.min(boxes[plane].0), sum.1.max(boxes[plane].1));
            n += count[plane];
            lower[plane] = (if n > 0 { area(sum.0, sum.1) } else { 0.0 }, n);
        }
        let (mut sum, mut n) = (empty, 0usize);
        for plane in (0..BINS - 1).rev() {
            sum = (sum.0.min(boxes[plane + 1].0), sum.1.max(boxes[plane + 1].1));
            n += count[plane + 1];
            let (lower_area, lower_n) = lower[plane];
            if lower_n == 0 || n == 0 {
                continue;
            }
            let cost = lower_area * lower_n as f32 + area(sum.0, sum.1) * n as f32;
            if best.is_none_or(|(least, _, _)| cost < least) {
                best = Some((cost, axis, plane));
            }
        }
    }
    let (_, axis, plane) = best?;
    let (origin, scale) = (centre_min.axis(axis), BINS as f32 / (centre_max.axis(axis) - centre_min.axis(axis)));
    // Partition: the triangles whose centre is in a bin up to the plane first.
    let mut lower = 0;
    for at in 0..order.len() {
        if bin_of(centres[order[at] as usize].axis(axis), origin, scale) <= plane {
            order.swap(lower, at);
            lower += 1;
        }
    }
    (lower > 0 && lower < order.len()).then_some((axis, lower))
}

/// Whether the ray meets the box before `max_t`.
fn slab(origin: Vec3, inv: Vec3, min: Vec3, max: Vec3, min_t: f32, max_t: f32) -> bool {
    let mut near = min_t.max(0.0);
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
        // The kept median build is the tree of SIGHT_ALGO 1 to the bit.
        assert_eq!(RayTree::from_corners_median(corners.clone()).digest(), PINNED_MEDIAN_FIELD);
        let mut moved = RayTree::from_corners(corners);
        moved.triangles[7].e1.y += 0.001;
        assert_ne!(moved.digest(), tree.digest());
    }

    const PINNED_FIELD: u64 = 5_027_165_012_823_869_721;
    /// The same field as the tree of SIGHT_ALGO 1 was.
    const PINNED_MEDIAN_FIELD: u64 = 5_143_660_430_816_924_144;

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

    /// A soup of small triangles, rays of every shape, and the three ways to answer: the tree, the tree as it was (medians,
    /// tight boxes), and testing every triangle. The surface-area tree must be the third to the bit; the old one is
    /// compared and its differences, if any, counted (a ray that grazes a triangle where a tight box turned it away).
    #[test]
    fn the_surface_area_tree_answers_as_the_median_tree_and_as_every_triangle() {
        let mut seed = 0x0123_4567_89ab_cdefu64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let (mut rays, mut hits, mut old_differs) = (0, 0, 0);
        for scene in 0..6 {
            let mut corners: Vec<[Vec3; 3]> = Vec::new();
            for k in 0..1500 {
                let c = Vec3::new(next() * 80.0, next() * 20.0, next() * 80.0);
                // Walls, floors and slivers as well as small bits; some on a grid so that rays graze their edges.
                let [dx, dy, dz] = match k % 4 {
                    0 => [next() * 6.0, next() * 6.0, 0.0],
                    1 => [next() * 6.0, 0.0, next() * 6.0],
                    2 => [(next() * 40.0).round(), (next() * 4.0).round(), (next() * 40.0).round() * 0.0],
                    _ => [next() * 0.5, next() * 0.5, next() * 0.5],
                };
                let c = if scene % 2 == 1 { Vec3::new(c.x.round(), c.y.round(), c.z.round()) } else { c };
                corners.push([c, c + Vec3::new(dx, 0.0, 0.0), c + Vec3::new(0.0, dy, dz + next())]);
            }
            let new = RayTree::from_corners(corners.clone());
            let old = RayTree::from_corners_median(corners.clone());
            for r in 0..6000 {
                let a = Vec3::new(next() * 80.0, next() * 20.0, next() * 80.0);
                let a = if r % 3 == 0 { Vec3::new(a.x.round(), a.y.round(), a.z.round()) } else { a };
                // Random, axis-aligned, along the grid, and through a vertex.
                let dir = match r % 5 {
                    0 => Vec3::new(next() - 0.5, next() - 0.5, next() - 0.5),
                    1 => [Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::Y, -Vec3::Y, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0)][(next() * 6.0) as usize % 6],
                    2 => Vec3::new((next() * 3.0).floor() - 1.0, (next() * 3.0).floor() - 1.0, (next() * 3.0).floor() - 1.0),
                    3 => corners[(next() * 1500.0) as usize % 1500][0] - a,
                    _ => Vec3::new(next() - 0.5, 0.0, next() - 0.5),
                };
                if dir.length() < 1e-3 {
                    continue;
                }
                let dir = dir * (1.0 / dir.length());
                let b = a + dir * (next() * 60.0);
                rays += 1;
                let want = new.visible_every_triangle(a, b);
                assert_eq!(new.visible(a, b), want, "visible {a:?} {b:?}");
                old_differs += usize::from(old.visible(a, b) != want);
                let cast = new.ray_cast_every_triangle(a, dir, 60.0);
                assert_eq!(new.ray_cast(a, dir, 60.0).map(f32::to_bits), cast.map(f32::to_bits), "ray_cast {a:?} {dir:?}");
                old_differs += usize::from(old.ray_cast(a, dir, 60.0).map(f32::to_bits) != cast.map(f32::to_bits));
                hits += usize::from(cast.is_some());
            }
        }
        assert!(rays > 30_000 && hits > 5_000, "{rays} {hits}");
        // Where the old tree turned a ray away that met a triangle (a tight box flat in the ray's plane: this scene is made of
        // grid-aligned flat triangles and axis-aligned rays on purpose) the new one does not: it is the one that is right.
        eprintln!("{old_differs} of {} answers of the median tree differ from testing every triangle", 2 * rays);
        assert!(old_differs * 20 < rays, "{old_differs} of {rays}");
    }

    /// Triangles spread so that surface-area splits are lopsided (sizes doubling along a line): the tree stays shallower than
    /// the traversal stack and its answers are those of testing every triangle.
    #[test]
    fn a_lopsided_scene_builds_a_tree_the_traversal_stack_holds() {
        let mut corners = Vec::new();
        for k in 0..20_000u32 {
            let x = (k as f32).powf(2.6) * 1e-6;
            let y = (k % 7) as f32;
            corners.push([Vec3::new(x, y, 0.0), Vec3::new(x + 0.1, y, 0.0), Vec3::new(x, y + 0.1, 0.1)]);
        }
        for k in 0..2000 {
            let v = Vec3::new(5.0, 5.0, 5.0) + Vec3::new(k as f32 * 1e-4, 0.0, 0.0);
            corners.push([v, v + Vec3::Y * 1e-3, v + Vec3::new(0.0, 0.0, 1e-3)]);
        }
        let tree = RayTree::from_corners(corners);
        let mut depth = vec![0usize; tree.nodes.len()];
        let mut deepest = 0;
        for (index, node) in tree.nodes.iter().enumerate() {
            if node.count == 0 {
                depth[index + 1] = depth[index] + 1;
                depth[node.right()] = depth[index] + 1;
            }
            deepest = deepest.max(depth[index]);
        }
        assert!(deepest + 1 < STACK, "depth {deepest}");
        let a = Vec3::new(-1.0, 0.05, 0.05);
        for target in [Vec3::new(50.0, 0.05, 0.05), Vec3::new(5.0, 5.0, 5.0), Vec3::new(5.0, 0.0, 0.0)] {
            assert_eq!(tree.visible(a, target), tree.visible_every_triangle(a, target));
        }
    }

    /// A closed box, its half sizes `half`, centred at `centre`, as triangles with armor `armor`.
    fn solid_box(centre: Vec3, half: Vec3, armor: f32) -> Vec<([Vec3; 3], FaceArmor)> {
        let (points, triangles) = crate::library::PrimitiveShape::Box { half }.tessellate();
        triangles
            .iter()
            .map(|t| (t.map(|i| points[i as usize] + centre), material(armor)))
            .collect()
    }

    fn material(armor: f32) -> FaceArmor {
        FaceArmor::Material(Armor::new(armor).unwrap())
    }

    fn share(tree: &RayTree, a: Vec3, b: Vec3, pierce: f32) -> f32 {
        let share = tree.damage_share(a, b, Pierce::new(pierce).unwrap()).value();
        // The cover tests' question agrees at every cut.
        for over in [0.0, 0.3, 0.5, 0.9, 1.0] {
            assert_eq!(
                tree.reaches_over(a, b, Pierce::new(pierce).unwrap(), over),
                share > over,
                "over {over}: share {share}"
            );
        }
        share
    }

    /// The tables of `an-sight.md` §2.2 through a body: a crate crossed once, not once a face.
    #[test]
    fn a_closed_body_is_crossed_once() {
        let (from, to) = (Vec3::new(0.0, 1.0, -5.0), Vec3::new(0.0, 1.0, 5.0));
        let wood = RayTree::from_armored_corners(solid_box(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.3), 0.1));
        assert_eq!(share(&wood, from, to, 0.40), 1.0, "an AK through wood");
        let brick = RayTree::from_armored_corners(solid_box(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.3), 0.2));
        assert!((share(&brick, from, to, 0.275) - 0.609).abs() < 0.006, "an SR-2M through brick");
        assert!((share(&brick, from, to, 0.33) - 0.8775).abs() < 0.006, "a VSK through brick");
        // From the other side, the same body.
        assert!((share(&brick, to, from, 0.275) - 0.609).abs() < 0.006);
        // A line that misses it, and one that stops short of it.
        assert_eq!(share(&brick, Vec3::new(3.0, 1.0, -5.0), Vec3::new(3.0, 1.0, 5.0), 0.275), 1.0);
        assert_eq!(share(&brick, from, Vec3::new(0.0, 1.0, -0.5), 0.275), 1.0);
        // A line that ends inside it has crossed the one face.
        assert!((share(&brick, from, Vec3::new(0.0, 1.0, 0.0), 0.275) - 0.609).abs() < 0.006);
        // Stone stops a rifle.
        let stone = RayTree::from_armored_corners(solid_box(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.3), 0.7));
        assert!(share(&stone, from, to, 0.40) == 0.0);
        assert!(tree_stops(&stone, from, to));
    }

    fn tree_stops(tree: &RayTree, a: Vec3, b: Vec3) -> bool {
        tree.damage_share(a, b, Pierce::new(0.40).unwrap()).is_stopped()
    }

    /// The pierce a bullet keeps falls with each surface: two bricks stop what one lets through.
    #[test]
    fn each_surface_takes_pierce_from_the_next() {
        let mut bricks = solid_box(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.1), 0.2);
        bricks.extend(solid_box(Vec3::new(0.0, 1.0, 2.0), Vec3::new(1.0, 1.0, 0.1), 0.2));
        let tree = RayTree::from_armored_corners(bricks);
        let (from, to) = (Vec3::new(0.0, 1.0, -5.0), Vec3::new(0.0, 1.0, 5.0));
        // SR-2M: 0.275 x 0.609 = 0.167 < 0.2: stopped at the second.
        assert_eq!(share(&tree, from, to, 0.275), 0.0);
        // AK-12: 0.40 goes through both at full damage (k = 1, pierce unchanged).
        assert_eq!(share(&tree, from, to, 0.40), 1.0);
        // A VSK keeps 0.8775 of 0.33 = 0.2896 for the second brick: f = 0.448, k = 0.695; 0.8775 x 0.695.
        let want = 0.8775 * 0.695;
        assert!((share(&tree, from, to, 0.33) - want).abs() < 0.01, "{}", share(&tree, from, to, 0.33));
    }

    /// Back faces cost nothing, free armor costs nothing, and the two triangles of a quad that a
    /// line crosses on their shared edge are one surface.
    #[test]
    fn faces_edges_and_free_armor() {
        let quad = |armor: f32, flip: bool| {
            let (a, b, c, d) = (
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(-1.0, 1.0, 0.0),
            );
            let armor = material(armor);
            let tris = if flip { [[a, c, b], [a, d, c]] } else { [[a, b, c], [a, c, d]] };
            RayTree::from_armored_corners(tris.map(|t| (t, armor)))
        };
        let (near, far) = (Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 0.0, 3.0));
        let (front, back) = (quad(0.2, false), quad(0.2, true));
        // Only one of the two orientations faces the near side; the other faces the far side.
        let (seen_from_near, seen_from_far) = (share(&front, near, far, 0.275), share(&front, far, near, 0.275));
        assert!(
            (seen_from_near == 1.0) != (seen_from_far == 1.0),
            "one side only: {seen_from_near} {seen_from_far}"
        );
        let counted = seen_from_near.min(seen_from_far);
        // The origin is on the diagonal (the shared edge) of the quad: crossed once, not twice.
        assert!((counted - 0.609).abs() < 0.006, "{counted}");
        let flipped = (share(&back, near, far, 0.275), share(&back, far, near, 0.275));
        assert_eq!(flipped, (seen_from_far, seen_from_near), "the other winding faces the other way");
        // Free armor.
        let glass = quad(0.0, false);
        assert_eq!(share(&glass, near, far, 0.05), 1.0);
        // A tree without materials stops at its first face, from either side.
        let solid = RayTree::from_corners([[Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0)]]);
        let (a, b) = (Vec3::new(0.5, -0.5, -3.0), Vec3::new(0.5, -0.5, 3.0));
        assert!(share(&solid, a, b, 1.0) == 0.0 && share(&solid, b, a, 1.0) == 0.0);
    }

    /// The share is 1 where `visible` sees through, whatever the armor.
    #[test]
    fn a_clear_line_keeps_everything() {
        let tree = RayTree::from_armored_corners(solid_box(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 1.0), 0.2));
        let a = Vec3::new(5.0, 1.0, 5.0);
        for b in [Vec3::new(-5.0, 1.0, 5.0), Vec3::new(5.0, 9.0, -5.0), a] {
            assert!(tree.visible(a, b));
            assert_eq!(share(&tree, a, b, 0.275), 1.0);
        }
        assert_eq!(tree.damage_share(a, Vec3::new(f32::NAN, 0.0, 0.0), Pierce::new(0.3).unwrap()), DamageShare::NONE);
    }

    /// A quad in the plane z = `z` (corners wound so that its normal `e1 x e2` points to -z when `toward_minus_z`).
    fn sheet(z: f32, armor: FaceArmor, toward_minus_z: bool) -> Vec<([Vec3; 3], FaceArmor)> {
        let (a, b, c, d) =
            (Vec3::new(-1.0, -1.0, z), Vec3::new(1.0, -1.0, z), Vec3::new(1.0, 1.0, z), Vec3::new(-1.0, 1.0, z));
        let tris = if toward_minus_z { [[a, c, b], [a, d, c]] } else { [[a, b, c], [a, c, d]] };
        tris.map(|t| (t, armor)).to_vec()
    }

    /// Two coincident sheets wound opposite ways are a double-sided wall: a bullet from either side meets the one that
    /// faces it, whatever order the walk finds them in (the back face at the same place must not hide the front).
    #[test]
    fn a_double_sided_sheet_stops_from_both_sides() {
        let mut both = sheet(0.0, material(0.2), true);
        both.extend(sheet(0.0, material(0.2), false));
        let tree = RayTree::from_armored_corners(both);
        let (near, far) = (Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 0.0, 3.0));
        for (from, to) in [(near, far), (far, near)] {
            let got = share(&tree, from, to, 0.275);
            assert!((got - 0.609).abs() < 0.006, "{from:?} -> {to:?}: {got}");
        }
    }

    /// Bodies that touch along the line are two bodies: the exit face of the first is skipped as a back face and the
    /// entry face of the second, at the same place, is counted (a stack of crates, a slab against a wall).
    #[test]
    fn touching_bodies_are_crossed_one_after_the_other() {
        let mut stack = solid_box(Vec3::new(0.0, 1.0, -0.1), Vec3::new(1.0, 1.0, 0.1), 0.2);
        stack.extend(solid_box(Vec3::new(0.0, 1.0, 0.1), Vec3::new(1.0, 1.0, 0.1), 0.2));
        let tree = RayTree::from_armored_corners(stack);
        let apart = {
            let mut apart = solid_box(Vec3::new(0.0, 1.0, -1.0), Vec3::new(1.0, 1.0, 0.1), 0.2);
            apart.extend(solid_box(Vec3::new(0.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 0.1), 0.2));
            RayTree::from_armored_corners(apart)
        };
        let (from, to) = (Vec3::new(0.0, 1.0, -5.0), Vec3::new(0.0, 1.0, 5.0));
        for pierce in [0.275, 0.33, 0.40] {
            assert_eq!(share(&tree, from, to, pierce), share(&apart, from, to, pierce), "pierce {pierce}");
            assert_eq!(share(&tree, to, from, pierce), share(&apart, to, from, pierce), "pierce {pierce}, the other way");
        }
        // And it is two, not one: a VSK keeps 0.8775 x 0.695 of it, an SR-2M none.
        assert!((share(&tree, from, to, 0.33) - 0.8775 * 0.695).abs() < 0.01);
        assert_eq!(share(&tree, from, to, 0.275), 0.0);
    }

    /// Free armor and back faces take no step of the follow's budget: a hundred leaves in front of a wall cost nothing.
    #[test]
    fn foliage_does_not_use_up_the_crossings() {
        let mut scene = solid_box(Vec3::new(0.0, 1.0, 30.0), Vec3::new(1.0, 1.0, 0.3), 0.2);
        for n in 0..100 {
            scene.extend(sheet(1.0 + n as f32 * 0.2, FaceArmor::FREE, n % 2 == 0));
        }
        let tree = RayTree::from_armored_corners(scene);
        let (from, to) = (Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 60.0));
        let got = share(&tree, from, to, 0.275);
        assert!((got - 0.609).abs() < 0.006, "{got}");
    }

    /// A wall meets the pierce the bullet has where the wall is: the same brick lets an SMG's bullet through near the
    /// muzzle and stops it at range, and what a crossing took off stays off.
    #[test]
    fn a_wall_meets_the_pierce_at_its_distance() {
        let at = |z: f32| {
            let tree = RayTree::from_armored_corners(solid_box(Vec3::new(0.0, 1.0, z), Vec3::new(1.0, 1.0, 0.1), 0.2));
            let flight = |metres: f32| Load { damage: 1.0 - metres / 200.0, pierce: 0.30 * (1.0 - metres / 100.0) };
            tree.fly(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, z + 5.0), &flight, 0.0, 0.0)
        };
        let near = at(5.0);
        assert!(near.share.value() > 0.0 && near.stopped_by.is_none(), "{near:?}");
        // 0.30 x (1 - 0.6) = 0.12 under the brick's 0.2 at 60 m: stopped.
        let far = at(60.0);
        assert!(far.share.is_stopped() && far.stopped_by == Some(material(0.2)), "{far:?}");
        // The bullet ends with the lesser of what it kept and what the flight gives there.
        assert!(near.load.damage <= 1.0 - 10.0 / 200.0 + 1e-6 && near.load.pierce <= 0.30 * (1.0 - 10.0 / 100.0) + 1e-6);
    }

    /// The bots' lines leave the 1 cm at each end untested (as `visible`); a bullet's flight leaves none.
    #[test]
    fn the_margin_at_the_ends_is_the_callers() {
        let tree = RayTree::from_armored_corners(sheet(5.0, material(20.0), true));
        let (from, to) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 5.004));
        assert_eq!(tree.damage_share(from, to, Pierce::new(0.3).unwrap()), DamageShare::ALL, "the sheet is 4 mm from the end");
        let whole = tree.fly(from, to, &|_| Load { damage: 1.0, pierce: 0.3 }, 0.0, 0.0);
        assert!(whole.share.is_stopped(), "{whole:?}");
    }

    /// The tree on disk keeps the armor: a tree read back answers as the tree built.
    #[test]
    fn the_armor_goes_to_the_file_and_back() {
        let tree = RayTree::from_armored_corners(solid_box(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.3), 0.2));
        let dir = std::env::temp_dir().join(format!("sight-armor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tree.bin");
        tree.write(&path, 77).unwrap();
        let read = RayTree::read(&path, 77).unwrap();
        assert_eq!(read.digest(), tree.digest());
        assert_eq!(read.armor_census(), tree.armor_census());
        let (from, to) = (Vec3::new(0.0, 1.0, -5.0), Vec3::new(0.0, 1.0, 5.0));
        assert_eq!(share(&read, from, to, 0.275), share(&tree, from, to, 0.275));
        // The same triangles with other armor are another tree.
        let other = RayTree::from_armored_corners(solid_box(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.3), 0.1));
        assert_ne!(other.digest(), tree.digest());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
