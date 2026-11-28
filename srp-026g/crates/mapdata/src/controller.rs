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
pub mod jump;
pub(crate) mod jump_clips;
mod compound_dbvt;
mod mesh_bvh;
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
    /// [bot CPU] This world's own number in the process (a new one for every world built, [`next_world_id`]): a remembered
    /// sweep is one of this world's only ([`sweep_memo`]). Nothing changes a world once built (no method takes it `&mut`).
    id: u64,
}

/// [bot CPU] A number no other [`Collision`] of this process holds.
fn next_world_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// [bot CPU] The walk tick's closest sweeps remembered on this thread ([`tick`]): a sweep asked again with every bit its
/// callback reads the same, of the same world, answers with the contact it kept before. A world never changes once built,
/// and a sweep's contacts are a function of the world, the shape and the way; its callback's answer a function of the
/// contacts and of what the key holds besides. Off unless a [`sweep_memo::On`] is held on the thread (the bots' lip guard
/// forecasts, whose bodies stand again and again where the last poll's stood); the grid build and the reckoning sweep without it.
pub mod sweep_memo {
    use super::Contact;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// The entries of one generation. The table keeps two (the one filling and the one before), so at most twice this many.
    pub const GENERATION: usize = 16_384;

    /// The walk tick's sweep that remembers: each reads a different set of values besides its shape and way.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub(crate) enum Site {
        /// `air_sweep`: the start, the end, the radius, whether it sweeps along the axis.
        Air,
        /// `move_sweep`'s callback: the shape, its end, the main start, the shape's own start, the move's end.
        Move,
        /// `down_sweep`: the start, the end, the floor limit, the radius.
        Down,
        /// `up_sweep`: the start, the end, the floor limit, the radius.
        Up,
        /// The side slide's down sweep: the shape and its end (its callback takes every contact).
        SlideDown,
    }

    /// Every bit a sweep's answer reads: its world, its site and the values the site names, as f32 bits in a fixed order.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub(crate) struct Key {
        world: u64,
        site: Site,
        bits: [u32; Key::WORDS],
    }

    impl Key {
        /// The words a key holds: up to 23 values (the move sweep's 20: its shape's 8, then 4 points) and the count of them.
        const WORDS: usize = 24;

        /// A key of `site` in `world` with no values yet ([`Self::f`], [`Self::v3`], [`Self::shape`]).
        pub(crate) fn of(world: &super::Collision, site: Site) -> Key {
            Key { world: world.id, site, bits: [0; Key::WORDS] }
        }

        /// The key with the bits of `x` next.
        pub(crate) fn f(mut self, x: f32) -> Key {
            let n = self.bits[Key::WORDS - 1] as usize;
            // A site holds a fixed list of values: more than a key's words is a site's key made wrong.
            assert!(n < Key::WORDS - 1, "a sweep key over {} words", Key::WORDS - 1);
            self.bits[n] = x.to_bits();
            self.bits[Key::WORDS - 1] = n as u32 + 1;
            self
        }

        /// The key with the bits of `p` next (x, y, z).
        pub(crate) fn v3(self, p: crate::math::Vec3) -> Key {
            self.f(p.x).f(p.y).f(p.z)
        }

        /// The key with `shape` next: its kind, then each of its numbers (8 words whatever the kind).
        pub(crate) fn shape(self, shape: &super::Shape) -> Key {
            use super::Shape;
            let kind = |k: u32| f32::from_bits(k);
            match *shape {
                Shape::Capsule(c) => self.f(kind(0)).v3(c.low).v3(c.high).f(c.radius),
                Shape::Cylinder { centre, half, radius, margin } => self.f(kind(1)).v3(centre).f(half).f(radius).f(margin).f(0.0),
                Shape::Sphere { centre, radius } => self.f(kind(2)).v3(centre).f(radius).f(0.0).f(0.0).f(0.0),
                Shape::CapsuleAt { centre, radius, half_height } => self.f(kind(3)).v3(centre).f(radius).f(half_height).f(0.0).f(0.0),
            }
        }
    }

    /// A key's hash: each word folded in by a rotate, xor and multiply (the `FxHash` step). A key is whole words of a fixed
    /// length, so the table needs no keyed hash; equal keys are told apart by `Eq`, never by the hash.
    #[derive(Default)]
    struct FoldHasher(u64);

    impl std::hash::Hasher for FoldHasher {
        fn finish(&self) -> u64 {
            self.0
        }

        fn write(&mut self, bytes: &[u8]) {
            let mut words = bytes.chunks_exact(8);
            for w in &mut words {
                self.write_u64(u64::from_le_bytes(w.try_into().expect("8 bytes")));
            }
            for b in words.remainder() {
                self.write_u64(u64::from(*b));
            }
        }

        fn write_u8(&mut self, i: u8) {
            self.write_u64(u64::from(i));
        }

        fn write_u32(&mut self, i: u32) {
            self.write_u64(u64::from(i));
        }

        fn write_u64(&mut self, i: u64) {
            self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }

        fn write_usize(&mut self, i: usize) {
            self.write_u64(i as u64);
        }
    }

    type Map = HashMap<Key, Option<Contact>, std::hash::BuildHasherDefault<FoldHasher>>;

    #[derive(Default)]
    struct Table {
        on: u32,
        new: Map,
        old: Map,
        counts: Counts,
    }

    /// What the memo did since the last [`take_counts`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Counts {
        /// Sweeps asked while on.
        pub asked: u64,
        /// Of them, answered from the table.
        pub remembered: u64,
        /// Generations turned over (the older dropped).
        pub turned: u64,
        /// The most entries held at once.
        pub most: u64,
    }

    thread_local! {
        static TABLE: RefCell<Table> = RefCell::new(Table::default());
    }

    /// Holds the memo on for this thread while it lives (nested holds count).
    pub struct On(());

    impl On {
        pub fn new() -> On {
            TABLE.with(|t| t.borrow_mut().on += 1);
            On(())
        }
    }

    impl Default for On {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for On {
        fn drop(&mut self) {
            TABLE.with(|t| t.borrow_mut().on -= 1);
        }
    }

    /// The answer of the sweep `key` names: remembered, else `sweep`'s (then remembered). Without an [`On`] held, `sweep`'s.
    pub(crate) fn closest(key: impl FnOnce() -> Key, sweep: impl FnOnce() -> Option<Contact>) -> Option<Contact> {
        let key = match TABLE.with(|t| t.borrow().on > 0) {
            true => key(),
            false => return sweep(),
        };
        let found = TABLE.with(|t| {
            let mut t = t.borrow_mut();
            t.counts.asked += 1;
            let found = t.new.get(&key).or_else(|| t.old.get(&key)).copied();
            if found.is_some() {
                t.counts.remembered += 1;
            }
            found
        });
        if let Some(answer) = found {
            return answer;
        }
        let answer = sweep();
        TABLE.with(|t| {
            let mut t = t.borrow_mut();
            if t.new.len() >= GENERATION {
                t.old = std::mem::take(&mut t.new);
                t.counts.turned += 1;
            }
            t.new.insert(key, answer);
            let held = (t.new.len() + t.old.len()) as u64;
            t.counts.most = t.counts.most.max(held);
        });
        answer
    }

    /// The counts since the last take, the table kept.
    pub fn counts() -> Counts {
        TABLE.with(|t| t.borrow().counts)
    }

    /// The counts since the last take; the table is emptied too (a match's memo ends with it).
    pub fn take_counts() -> Counts {
        TABLE.with(|t| {
            let mut t = t.borrow_mut();
            t.new = Map::default();
            t.old = Map::default();
            std::mem::take(&mut t.counts)
        })
    }

    /// The heap one entry takes in the table, bytes: its key and answer, and the map's control byte (at the map's 7/8 load,
    /// up to 8/7 of that).
    pub fn entry_bytes() -> usize {
        std::mem::size_of::<Key>() + std::mem::size_of::<Option<Contact>>() + 1
    }
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
    /// [L36] The part of that object met (its one shape, or a compound's child: one query of the exe's each), for the tie
    /// counts ([`sweep_ties`]); `None` where `object` is.
    pub part: Option<u32>,
    /// [L36] A mesh triangle's place in the order the game's tree of its mesh offers the mesh's triangles
    /// ([`mesh_bvh`]): at an equal fraction the exe keeps the later one inside one mesh query ([`Contact::replaces`]). None for
    /// anything else (a primitive, a placed face, a mesh the port has no order for).
    pub rank: Option<u32>,
    /// [L36] A compound's child's place in the order the compound's tree offers its children ([`compound_dbvt`]): at an equal
    /// fraction the exe keeps the child it offered first ([`Contact::replaces`]). None for an object's whole shape, a placed
    /// face, or a compound the port has no order for.
    pub child: Option<u32>,
}

impl Contact {
    /// [L36] Whether this contact takes the place of `kept` as a sweep's closest: it is nearer, or at the same fraction the exe
    /// would have kept it (exactness `grid/rig/s7-20261114/L36-tie-order.md` §1):
    /// - inside one mesh query (the same part of the same object) the LAST triangle the mesh's tree offers (`processTriangle`
    ///   0xB87CA strict against the closest when the mesh's query began, `reportHit` 0xA2A6C `<=` against the current one);
    /// - between two children of one compound the FIRST its child tree offers (0xA162D strict, and a mesh child's query
    ///   starts from the closest the earlier children left);
    /// - across objects the earlier stays (their order is the broadphase's, not portable: the port's own).
    ///
    /// By the places, not by the order the port hands the contacts over in.
    pub fn replaces(&self, kept: &Contact) -> bool {
        self.t < kept.t || (self.t == kept.t && self.place().wins_tie(&kept.place()))
    }

    /// [L36] What of the contact the tie rule of [`Self::replaces`] reads.
    pub fn place(&self) -> Place {
        Place { object: self.object, part: self.part, rank: self.rank, child: self.child }
    }
}

/// [L36] Where a contact was met, as the tie rule of [`Contact::replaces`] reads it: known before the cast that would make the
/// contact, so a cast whose contact could not win a tie need not run ([`Bound`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// [`Contact::object`].
    pub object: Option<SolidId>,
    /// [`Contact::part`].
    pub part: Option<u32>,
    /// [`Contact::rank`].
    pub rank: Option<u32>,
    /// [`Contact::child`].
    pub child: Option<u32>,
}

impl Place {
    /// Whether a contact here keeps the place of one at `kept` at an equal fraction ([`Contact::replaces`]).
    pub fn wins_tie(&self, kept: &Place) -> bool {
        match (self.object, kept.object) {
            (Some(a), Some(b)) if a == b => {}
            _ => return false,
        }
        if self.part == kept.part {
            matches!((self.rank, kept.rank), (Some(r), Some(s)) if r > s)
        } else {
            matches!((self.child, kept.child), (Some(r), Some(s)) if r < s)
        }
    }
}

/// [bot CPU] What a sweep's callback answers each contact with ([`Collision::sweep_each_to_within`]): no later contact past the
/// fraction `t` matters to it, and at exactly `t`, where it names the contact it keeps there, only one that would take that
/// one's place ([`Contact::replaces`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bound {
    t: f32,
    kept: Option<Place>,
}

impl Bound {
    /// Every contact matters.
    pub const ALL: Bound = Bound { t: 1.0, kept: None };

    /// The callback keeps `kept` as its closest so far: a later contact matters only where it would replace it.
    pub fn kept(kept: &Contact) -> Bound {
        Bound { t: kept.t, kept: Some(kept.place()) }
    }

    /// The fraction past which no contact matters.
    pub fn t(&self) -> f32 {
        self.t
    }

    /// Whether a contact at `place`, not cast yet, may matter. A cast's fraction is 0 or more ([`exe_cast::time_of_impact`]
    /// refuses a negative lambda), so under a bound at 0 a contact can only tie the kept one, and it matters only where it
    /// wins that tie. Over 0 the cast must run (its fraction may come under the bound).
    pub(crate) fn may_matter(&self, place: &Place) -> bool {
        self.t > 0.0 || self.kept.is_none_or(|kept| place.wins_tie(&kept))
    }
}

/// [bot CPU] Casts not run because the callback kept a contact at fraction 0 that their contact could not replace
/// ([`Bound::may_matter`]): triangles, then primitives. Totals of this process, for the logs: no decision reads them.
static ZERO_TIE_SKIPS: [std::sync::atomic::AtomicU64; 2] = [const { std::sync::atomic::AtomicU64::new(0) }; 2];

/// [bot CPU] The casts skipped under a kept contact at fraction 0 ([`Bound::may_matter`]) since the last take: (triangles,
/// primitives), every sweep of this process.
pub fn take_zero_tie_skips() -> (u64, u64) {
    let n = |i: usize| ZERO_TIE_SKIPS[i].swap(0, std::sync::atomic::Ordering::Relaxed);
    (n(0), n(1))
}

/// [bot CPU] Counts one cast skipped ([`take_zero_tie_skips`]).
fn count_zero_tie_skip(primitive: bool) {
    ZERO_TIE_SKIPS[usize::from(primitive)].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// [L36] How many contacts of the sweeps so far ([`Collision::sweep_each`], every sweep of the walk tick and the build's flights)
/// met something at exactly the sweep's closest fraction besides the first contact there, by what the two are: where the
/// client's pick among them depends on an order (exactness `grid/rig/s7-20261114/L36-tie-order.md`: inside one mesh the exe
/// keeps the LAST triangle its BVH offers, across objects and a compound's children the FIRST). The port keeps the exe's pick
/// inside one mesh by the mesh tree's order ([`mesh_bvh`]) and between a compound's children by its child tree's order
/// ([`compound_dbvt`], from a version 8 map data file): [`Contact::replaces`]. Across objects (not portable) it keeps its own
/// first. Totals of this process, for the logs: no decision reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SweepTies {
    /// Sweeps with at least one tie at the closest fraction.
    pub sweeps: u64,
    /// Ties between two triangles of one mesh: one part of one object, one query (the exe: keep-last in its BVH's leaf order,
    /// 0xB87CA strict against the closest when the mesh's query began, 0xA2A6C `<=` in `reportHit`), decided by the ported
    /// order where the mesh has one.
    pub one_mesh: u64,
    /// Ties between two parts of one object (a compound's children: two primitives, a primitive and a mesh, or two meshes, each
    /// its own query; the exe: keep-first in the dbvt's order), decided by the ported child order.
    pub same_object_parts: u64,
    /// [L36] The same with no child order for the compound (a map data file older than version 8, or a child the port has no
    /// box for): left in the port's own order.
    pub same_object_unordered: u64,
    /// Ties between two objects (the exe: keep-first in the broadphase's order, not portable).
    pub across_objects: u64,
    /// Ties in a world of placed triangles with no objects ([`Collision::build`]).
    pub faces: u64,
}

static TIE_COUNTS: [std::sync::atomic::AtomicU64; 6] = [const { std::sync::atomic::AtomicU64::new(0) }; 6];

/// The process's [`SweepTies`] so far.
pub fn sweep_ties() -> SweepTies {
    let n = |i: usize| TIE_COUNTS[i].load(std::sync::atomic::Ordering::Relaxed);
    SweepTies { sweeps: n(0), one_mesh: n(1), same_object_parts: n(2), across_objects: n(3), faces: n(4), same_object_unordered: n(5) }
}

/// [L36] Where the sweeps' compound-child cull (`Solids::sweep_within`: a child offered only where its box in the compound's
/// child tree meets the cast's volume, as the exe's `collideTV`) parted from the test it replaced (the child's world box
/// against the padded swept box). Totals of this process, for the logs: no decision reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SweepCull {
    /// Children the old test offered and the tree does not.
    pub skipped: u64,
    /// Children the tree offers and the old test did not.
    pub added: u64,
    /// Sweeps given their end ([`Way::To`]) whose end the start plus the way would not rebuild to the same bits (the volume read
    /// `from + by` before).
    pub end_rebuilt_differs: u64,
}

static CULL_COUNTS: [std::sync::atomic::AtomicU64; 3] = [const { std::sync::atomic::AtomicU64::new(0) }; 3];

/// The process's [`SweepCull`] so far.
pub fn sweep_cull() -> SweepCull {
    let n = |i: usize| CULL_COUNTS[i].load(std::sync::atomic::Ordering::Relaxed);
    SweepCull { skipped: n(0), added: n(1), end_rebuilt_differs: n(2) }
}

/// [L36] Counts one child where the cull and the old test part ([`SweepCull`]).
fn count_cull_parting(offered_now: bool) {
    CULL_COUNTS[usize::from(offered_now)].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// [L36] Where the move test's top check (0x85F50), whose callbacks take the main start, would have answered otherwise with each
/// shape's own start (the port's before). Totals of this process, for the logs: no decision reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopCheckStart {
    /// Contacts at fraction 0 the fraction-0 test (0x896EA..0x8972A) keeps or drops otherwise.
    pub fraction_zero: u64,
    /// Wall rays (0x89B80) whose normal comes out otherwise.
    pub wall_ray: u64,
}

/// What [`TopCheckStart`] counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TopCheckParting {
    FractionZero,
    WallRay,
}

static TOP_CHECK_COUNTS: [std::sync::atomic::AtomicU64; 2] = [const { std::sync::atomic::AtomicU64::new(0) }; 2];

/// The process's [`TopCheckStart`] so far.
pub fn top_check_start() -> TopCheckStart {
    let n = |i: usize| TOP_CHECK_COUNTS[i].load(std::sync::atomic::Ordering::Relaxed);
    TopCheckStart { fraction_zero: n(0), wall_ray: n(1) }
}

/// [L36] Counts one parting ([`TopCheckStart`]).
fn count_top_check_parting(what: TopCheckParting) {
    let i = match what {
        TopCheckParting::FractionZero => 0,
        TopCheckParting::WallRay => 1,
    };
    TOP_CHECK_COUNTS[i].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// One sweep's ties at its closest fraction so far: the closest, what the first contact there was, and the counts.
struct Ties {
    closest: f32,
    first: Option<Contact>,
    counts: [u64; 5],
}

impl Ties {
    fn new() -> Self {
        Self { closest: f32::INFINITY, first: None, counts: [0; 5] }
    }

    fn offer(&mut self, c: &Contact) {
        let mesh = matches!(c.on, Surface::Face { .. });
        if c.t < self.closest {
            self.closest = c.t;
            self.first = Some(*c);
            self.counts = [0; 5];
        } else if c.t == self.closest
            && let Some(first) = self.first
        {
            let (object, part, child) = (first.object, first.part, first.child);
            let first_mesh = matches!(first.on, Surface::Face { .. });
            let k = match (object, c.object) {
                (Some(a), Some(b)) if a == b && part == c.part && first_mesh && mesh => 0,
                (Some(a), Some(b)) if a == b && part != c.part && (child.is_none() || c.child.is_none()) => 4,
                (Some(a), Some(b)) if a == b => 1,
                (Some(_), Some(_)) => 2,
                _ => 3,
            };
            self.counts[k] += 1;
        }
    }

    fn flush(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        if self.counts.iter().any(|&n| n > 0) {
            TIE_COUNTS[0].fetch_add(1, Relaxed);
            for (k, &n) in self.counts.iter().enumerate() {
                TIE_COUNTS[k + 1].fetch_add(n, Relaxed);
            }
        }
    }
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
        Self { faces, nodes, sweeps: Sweeps::Faces(Caster::default()), id: next_world_id() }
    }

    /// The same faces, its sweeps against each face by `caster`.
    pub fn with_caster(self, caster: Caster) -> Self {
        Self { sweeps: Sweeps::Faces(caster), id: next_world_id(), ..self }
    }

    /// The walking body's collision of `map` as the game holds it, for sweeps by the exe's own cast: the game's
    /// objects ([`crate::bullet`]: a placed mesh's triangles in the mesh's frame under the object's transform, a box,
    /// a sphere, a cylinder or a capsule as the convex shape with its margin, a compound's children under the
    /// transforms the exe composes), met as `objectQuerySingle` 0xA14E0 meets them. Rays and the nearest-face
    /// questions still read the placed triangles ([`MapData::walker_triangles`]; a primitive's are its drawn
    /// stand-in). `None` for a map without the game's shapes (a file older than format 6).
    pub fn build_exe(map: &MapData) -> Option<Self> {
        // [srchash oracle] The shapes' code, for the coverage check of the memo's `world` digest (`crate::coverage`).
        crate::coverage::window("world", || {
            let solids = solids::Solids::build(map)?;
            Some(Self { sweeps: Sweeps::Solids(Box::new(solids)), ..Self::build(&map.walker_triangles()) })
        })
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
        self.sweep_closest(shape, Way::By(by), |c| keep_surface(&keep, *c))
    }

    /// [`Self::sweep_shape`] to the end's origin `to` as the exe's caller gives it ([`Way::To`]).
    pub fn sweep_shape_to(&self, shape: Shape, to: Vec3, keep: impl Fn(&Face) -> bool) -> Option<Contact> {
        self.sweep_closest(shape, Way::To(to), |c| keep_surface(&keep, *c))
    }

    /// The closest contact `accept` takes of [`Self::sweep_each_to`]'s ([`Contact::replaces`] over them in their order).
    /// `accept` is asked of the contacts at or under the closest one taken so far (no later one past it could replace it).
    pub fn sweep_closest_to(&self, shape: Shape, to: Vec3, accept: impl FnMut(&Contact) -> bool) -> Option<Contact> {
        self.sweep_closest(shape, Way::To(to), accept)
    }

    /// [bot CPU] [`Self::sweep_each_to`] whose casts give up past a bound: `each` answers each contact with the [`Bound`] past
    /// which no later contact matters to it ([`Solids::sweep_within`]); every contact that may matter under it is handed on,
    /// in order.
    pub fn sweep_each_to_within(&self, shape: Shape, to: Vec3, each: impl FnMut(Contact) -> Bound) {
        self.sweep_way_within(shape, Way::To(to), each);
    }

    /// [bot CPU] The closest accepted contact. A cast past the closest accepted so far gives up there ([`Solids::sweep_within`]):
    /// a contact past it would not replace it ([`Contact::replaces`] needs a fraction at or under the kept one's), so the
    /// answer is the full sweep's; `accept` is not asked of the contacts past it. Under a kept contact at 0, a cast whose
    /// contact could not replace it does not run ([`Bound::may_matter`]), nor is `accept` asked of it.
    fn sweep_closest(&self, shape: Shape, way: Way, mut accept: impl FnMut(&Contact) -> bool) -> Option<Contact> {
        let mut best: Option<Contact> = None;
        self.sweep_way_within(shape, way, |c| {
            if accept(&c) && best.is_none_or(|b| c.replaces(&b)) {
                best = Some(c);
            }
            best.as_ref().map_or(Bound::ALL, Bound::kept)
        });
        best
    }

    /// Every face's first contact with `shape` moved by `by` (Bullet's per-triangle convex cast: what the client's sweep
    /// callbacks are handed, in no order). A face the shape already touches at the start is reported at 0 only when the move
    /// goes into it.
    pub fn sweep_each(&self, shape: Shape, by: Vec3, each: impl FnMut(Contact)) {
        self.sweep_way(shape, Way::By(by), each);
    }

    /// [`Self::sweep_each`] to the end's origin `to` as the exe's caller gives it ([`Way::To`]).
    pub fn sweep_each_to(&self, shape: Shape, to: Vec3, each: impl FnMut(Contact)) {
        self.sweep_way(shape, Way::To(to), each);
    }

    fn sweep_way(&self, shape: Shape, way: Way, mut each: impl FnMut(Contact)) {
        self.sweep_way_within(shape, way, |c| {
            each(c);
            Bound::ALL
        });
    }

    /// [`Self::sweep_way`] with the bound of [`Solids::sweep_within`] (`each` answers the [`Bound`] past which no later
    /// contact matters to it). Faces are cast whole.
    fn sweep_way_within(&self, shape: Shape, way: Way, mut each: impl FnMut(Contact) -> Bound) {
        let by = way.by(shape);
        if let Way::To(to) = way
            && cast_origin(shape) + by != to
        {
            CULL_COUNTS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        // [L36] Ties at the closest fraction, counted for the logs ([`sweep_ties`]); the contacts go on unchanged.
        let mut ties = Ties::new();
        let mut each = |c: Contact| {
            ties.offer(&c);
            each(c)
        };
        let caster = match &self.sweeps {
            Sweeps::Faces(caster) => *caster,
            Sweeps::Solids(solids) => {
                solids.sweep_within(shape, way, &mut each);
                ties.flush();
                return;
            }
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
                let _ = each(Contact { t, point, normal, on: Surface::Face { walker: face.walker, normal: face.normal }, object: None, part: None, rank: None, child: None });
            }
        });
        ties.flush();
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

    /// The nearest face a ray from `a` to `b` crosses: the share of the way and the face's normal turned toward `a` (the exe's
    /// triangle ray's normal, PROVEN: `btTriangleRaycastCallback::processTriangle` 0xB8010 hands `reportHit` the unit normal
    /// turned over when the start is not in front; the crossing test here is Moller-Trumbore, not the exe's, which
    /// [`exe_ray`] ports for a world of the game's objects).
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
/// face's margin 0, PROVEN: the map mesh's ctor 0xA7950 sets 0, and every map mesh in two captures of the game's memory has
/// 0 there: 321 of 321 on Vector Laboratory live1, 168 of 168 on Chemical Plant case15).
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

/// How a sweep moves its shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Way {
    /// By this much: the end's origin is the start's plus it.
    By(Vec3),
    /// [L36] To this end origin, as the exe's caller gives the cast its end transform: the cast's way is the end less the
    /// start (0x72EB0), and a compound's query volume reads the end itself (`objectQuerySingle` 0xA2421..0xA2507).
    To(Vec3),
}

impl Way {
    /// The cast's way for `shape`.
    fn by(self, shape: Shape) -> Vec3 {
        match self {
            Way::By(by) => by,
            Way::To(to) => to - cast_origin(shape),
        }
    }

    /// The end's origin for `shape`.
    fn to(self, shape: Shape) -> Vec3 {
        match self {
            Way::By(by) => cast_origin(shape) + by,
            Way::To(to) => to,
        }
    }
}

/// The origin of `shape`'s cast transform ([`exe_cast_of`]'s start).
fn cast_origin(shape: Shape) -> Vec3 {
    match shape {
        Shape::CapsuleAt { centre, .. } | Shape::Cylinder { centre, .. } | Shape::Sphere { centre, .. } => centre,
        Shape::Capsule(c) => (c.low + c.high) * 0.5,
    }
}

/// `shape` moved by `by` as the exe's sweep hands it to Bullet: `by` is the cast's way as the exe subtracts it (the
/// end's origin minus the start's).
fn exe_cast_of(shape: Shape, by: Vec3) -> exe_cast::Cast {
    use exe_gjk::{Axis, CastShape};
    let capsule = |radius: f32, half_height: f32| CastShape::capsule([radius, half_height, radius], [1.0; 3], EXE_CAPSULE_MARGIN_M, Axis::Y);
    let (cast_shape, margin, from) = match shape {
        Shape::CapsuleAt { centre, radius, half_height } => (capsule(radius, half_height), EXE_CAPSULE_MARGIN_M, centre),
        // A capsule given by its sphere centres: the centre and the half height are worked back (not the exe's bits).
        Shape::Capsule(c) => (capsule(c.radius, (c.high.y - c.low.y) * 0.5), EXE_CAPSULE_MARGIN_M, cast_origin(shape)),
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

    /// [L36] At an equal fraction a later triangle of the same mesh query takes the place (keep-last); one of another part or
    /// object, an unranked one, or a farther one does not (keep-first).
    #[test]
    fn a_tie_inside_one_mesh_keeps_the_later_triangle() {
        let c = |t: f32, object: u32, part: u32, rank: Option<u32>| Contact {
            t,
            point: Vec3::ZERO,
            normal: Vec3::new(0.0, 1.0, 0.0),
            on: Surface::Face { walker: WalkerBehaviour::Walk, normal: Vec3::new(0.0, 1.0, 0.0) },
            object: Some(SolidId(object)),
            part: Some(part),
            rank,
            child: None,
        };
        let kept = c(0.0, 1, 2, Some(10));
        assert!(c(0.0, 1, 2, Some(11)).replaces(&kept), "later in the same mesh");
        assert!(!c(0.0, 1, 2, Some(9)).replaces(&kept), "earlier in the same mesh");
        assert!(!c(0.0, 1, 3, Some(11)).replaces(&kept), "another part of the object");
        assert!(!c(0.0, 4, 2, Some(11)).replaces(&kept), "another object");
        assert!(!c(0.0, 1, 2, None).replaces(&kept), "no rank");
        assert!(c(-0.0, 1, 3, None).t == kept.t && !c(-0.0, 1, 3, None).replaces(&kept));
        assert!(c(0.0, 4, 2, None).replaces(&c(0.5, 1, 2, Some(1))), "nearer");
    }

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

/// [bot CPU] Sweeps recorded on a real map by the controller before [`Bound::may_matter`] and the mesh's last-offered-first
/// order (cast-record-patch-v1.py: each sweep of the walk tick's closest kind and its down sweep that took 0.5 ms or more, with the
/// FULL sweep's contacts, the callback's verdict on each, and the answer), cast again here: the same contacts to the bit, and
/// the same answer. `SRP_CAST_RECORDS` the file, `SRP_CAST_MAP` its map's `.mapdata`.
#[cfg(test)]
mod recorded_casts {
    use super::*;

    fn bits(s: &str) -> f32 {
        f32::from_bits(u32::from_str_radix(s, 16).expect("hex bits"))
    }

    fn opt(s: &str) -> Option<u32> {
        (s != "-").then(|| s.parse().expect("a number or -"))
    }

    fn vec3(t: &[&str]) -> Vec3 {
        Vec3::new(bits(t[0]), bits(t[1]), bits(t[2]))
    }

    /// A contact's recorded fields: fraction, point, normal (bits), object, part, rank, child.
    type Key = (u32, [u32; 3], [u32; 3], Option<u32>, Option<u32>, Option<u32>, Option<u32>);

    fn key_of(c: &Contact) -> Key {
        (c.t.to_bits(), c.point.to_array().map(f32::to_bits), c.normal.to_array().map(f32::to_bits), c.object.map(|s| s.0), c.part, c.rank, c.child)
    }

    fn parse_key(t: &[&str]) -> Key {
        let v = |i: usize| [0, 1, 2].map(|k| u32::from_str_radix(t[i + k], 16).expect("hex bits"));
        (u32::from_str_radix(t[0], 16).expect("hex bits"), v(1), v(4), opt(t[7]), opt(t[8]), opt(t[9]), opt(t[10]))
    }

    fn parse_shape(t: &[&str]) -> Shape {
        match t[0] {
            "cap" => Shape::Capsule(Capsule { low: vec3(&t[1..]), high: vec3(&t[4..]), radius: bits(t[7]) }),
            "cyl" => Shape::Cylinder { centre: vec3(&t[1..]), half: bits(t[4]), radius: bits(t[5]), margin: bits(t[6]) },
            "sph" => Shape::Sphere { centre: vec3(&t[1..]), radius: bits(t[4]) },
            "capat" => Shape::CapsuleAt { centre: vec3(&t[1..]), radius: bits(t[4]), half_height: bits(t[5]) },
            other => panic!("shape {other}"),
        }
    }

    #[test]
    #[ignore = "needs SRP_CAST_RECORDS and SRP_CAST_MAP"]
    fn recorded_sweeps_keep_their_contacts_and_answers() {
        let (Some(records), Some(map)) = (std::env::var_os("SRP_CAST_RECORDS"), std::env::var_os("SRP_CAST_MAP")) else {
            return;
        };
        let map = crate::MapData::read(std::path::Path::new(&map)).expect("the map");
        let world = Collision::build_exe(&map).expect("a map of the game's shapes");
        let text = std::fs::read_to_string(records).expect("the records");
        let _ = take_zero_tie_skips();
        let (mut sweeps, mut contacts, mut at_zero, mut old_us, mut new_us) = (0usize, 0usize, 0usize, 0u128, 0u128);
        let mut full_us = 0u128;
        for line in text.lines() {
            let parts: Vec<&str> = line.split(" | ").collect();
            assert_eq!(parts.len(), 5, "a record of 5 parts: {}", &line[..line.len().min(120)]);
            let head: Vec<&str> = parts[0].split(' ').collect();
            let shape = parse_shape(&parts[1].split(' ').collect::<Vec<_>>());
            let way: Vec<&str> = parts[2].split(' ').collect();
            let way = match way[0] {
                "by" => Way::By(vec3(&way[1..])),
                "to" => Way::To(vec3(&way[1..])),
                other => panic!("way {other}"),
            };
            let mut full: Vec<&str> = parts[3].split(" ; ").collect();
            let n: usize = full.remove(0).parse().expect("the count");
            assert_eq!(full.len(), n);
            let verdicts: Vec<(Key, bool)> = full
                .iter()
                .map(|c| {
                    let t: Vec<&str> = c.split(' ').collect();
                    (parse_key(&t), t[11] == "1")
                })
                .collect();
            let answer = (parts[4] != "none").then(|| parse_key(&parts[4].split(' ').collect::<Vec<_>>()));
            // The full sweep's contacts, as a set: the casts' results are the old ones.
            let mut now: Vec<Key> = Vec::new();
            let full_started = std::time::Instant::now();
            world.sweep_way(shape, way, |c| now.push(key_of(&c)));
            full_us += full_started.elapsed().as_micros();
            let mut was: Vec<Key> = verdicts.iter().map(|(k, _)| *k).collect();
            now.sort_unstable();
            was.sort_unstable();
            assert_eq!(now, was, "the full sweep's contacts: {}", &line[..line.len().min(160)]);
            let by_key: std::collections::HashMap<Key, bool> = verdicts.iter().copied().collect();
            assert_eq!(by_key.len(), n, "two recorded contacts of one key");
            let verdict = |c: &Contact| {
                let k = key_of(c);
                *by_key.get(&k).unwrap_or_else(|| panic!("a contact the full sweep did not make: {k:?}"))
            };
            let started = std::time::Instant::now();
            let got = match head[0] {
                "closest" => world.sweep_closest(shape, way, verdict),
                "down" => {
                    // The down sweep's way of keeping (tick.rs `down_sweep`).
                    let mut hits: Vec<Contact> = Vec::new();
                    let mut best: Option<Contact> = None;
                    world.sweep_way_within(shape, way, |c| {
                        if verdict(&c) && best.is_none_or(|b| c.replaces(&b)) {
                            best = Some(c);
                        }
                        hits.push(c);
                        best.as_ref().map_or(Bound::ALL, Bound::kept)
                    });
                    hits.sort_by(|a, b| a.t.total_cmp(&b.t));
                    let mut kept: Option<Contact> = None;
                    for c in hits {
                        match kept {
                            Some(k) if c.t > k.t => break,
                            _ if !verdict(&c) => {}
                            Some(k) if !c.replaces(&k) => {}
                            _ => kept = Some(c),
                        }
                    }
                    kept
                }
                other => panic!("kind {other}"),
            };
            new_us += started.elapsed().as_micros();
            old_us += head[1].parse::<u128>().expect("the old us");
            assert_eq!(got.as_ref().map(key_of), answer, "the answer: {}", &line[..line.len().min(160)]);
            sweeps += 1;
            contacts += n;
            at_zero += usize::from(answer.is_some_and(|a| a.0 == 0));
        }
        let (tri, prim) = take_zero_tie_skips();
        eprintln!(
            "recorded sweeps {sweeps}: answers and full contacts equal; {contacts} contacts, {at_zero} answers at 0; casts skipped: triangles {tri}, primitives {prim}; recorded {old_us} us (the old sweep in its match), now {new_us} us, the full unbounded sweep here {full_us} us"
        );
        assert!(sweeps > 0, "no records");
    }
}
