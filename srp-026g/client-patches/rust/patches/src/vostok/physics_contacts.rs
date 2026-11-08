//! The physics world's contact manifolds as patch 121 saves them with every
//! world and puts them back at every load (`docs/notes/physics_contacts.md`):
//! Bullet 2.79's btCollisionDispatcher (its manifolds +0Ch/+14h,
//! getNewManifold 0x8F400), its btPersistentManifold (four points of 120h at
//! +10h, the bodies +490h/+494h, the count +498h), the broadphase's pairs
//! (btHashedOverlappingPairCache::findPair 0xB41D0, cleanOverlappingPair
//! 0xB40E0) and the near callback's order (0x8F860, the pair's +0Ch).
//!
//! Every handle here borrows the hook's thread; what the patch keeps between
//! hooks is ids (addresses as values) and the points' bytes.

use super::context::Context;
use super::world::read;
use crate::raw;
use srp_validate::world::{BodyKey, POINT_BYTES, fnv1a, key_bits};

/// A point's bytes as the world holds them.
pub type PointBytes = [u8; POINT_BYTES];

/// The fields of a btManifoldPoint (the constructor 0x8F1B0, Bullet 2.79)
/// that a world holds, in its order: (offset, bytes). Left out: each
/// vector's w (+0Ch .. +4Ch, +0ACh, +0BCh, no number of the step's), the
/// user pointer +6Ch (the destroy callback's, never set: 0x1F1D3AC is
/// never written), the padding after the friction flag and after the life
/// time, and the three constraint rows +0C0h..+120h (btContactConstraint's,
/// which the sequential impulse solver never uses).
pub const POINT_FIELDS: [(usize, usize); 12] = [
    (0x00, 12), // m_localPointA
    (0x10, 12), // m_localPointB
    (0x20, 12), // m_positionWorldOnB
    (0x30, 12), // m_positionWorldOnA
    (0x40, 12), // m_normalWorldOnB
    (0x50, 28), // m_distance1, m_combinedFriction, m_combinedRestitution, m_partId0/1, m_index0/1
    (0x70, 4),  // m_appliedImpulse
    (0x74, 1),  // m_lateralFrictionInitialized
    (0x78, 24), // m_appliedImpulseLateral1/2, m_contactMotion1/2, m_contactCFM1/2
    (0x90, 4),  // m_lifeTime
    (0xA0, 12), // m_lateralFrictionDir1
    (0xB0, 12), // m_lateralFrictionDir2
];

const POINT_SIZE: usize = 0x120;
/// The group bit of the bodies that move (a grenade 818h, a battery 0A18h;
/// bullet_bounds.md); a body without it is the level's or a hit model.
const MOVING_GROUP: u16 = 0x800;
/// m_collisionFlags: CF_STATIC_OBJECT.
const STATIC: u32 = 1;
/// m_activationState1: ACTIVE_TAG.
const ACTIVE: u32 = 1;

/// A dispatcher's identity: its address as a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatcherId(u32);

/// A body of the physics world.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Body(raw::BtCollisionObject);

impl Body {
    /// Its address as a value (an id; never read through).
    pub fn id(self) -> u32 {
        self.0.address()
    }

    /// Whether it is a battery's or a grenade's (its filter group has 800h).
    pub fn moving(self) -> bool {
        // SAFETY: a live body of the hook's physics world; its proxy, when it
        // is in the world.
        unsafe { self.0.broadphase().is_some_and(|proxy| read(proxy.group()) & MOVING_GROUP != 0) }
    }

    /// What names it in a world (srp_validate::world::BodyKey).
    pub fn key(self) -> BodyKey {
        // SAFETY: a live body; its transform's origin (3 f32 of the float4)
        // and its proxy's box, both its own fields.
        unsafe {
            let origin = self.0.transform().origin().cast::<u32>();
            let origin = [read(origin), read(origin.add(1)), read(origin.add(2))].map(key_bits);
            let proxy = self.0.broadphase();
            if read(self.0.flags()) & STATIC != 0
                && let Some(proxy) = proxy
            {
                let aabb = proxy.aabb().cast::<u32>();
                let words = [0, 1, 2, 4, 5, 6].map(|i| key_bits(read(aabb.add(i))));
                let hash = fnv1a(&words);
                let filter = u32::from(read(proxy.group())) | u32::from(read(proxy.mask())) << 16;
                return BodyKey::Fixed([hash as u32, (hash >> 32) as u32, filter]);
            }
            if proxy.is_some_and(|proxy| read(proxy.group()) & MOVING_GROUP != 0) {
                BodyKey::Moving(origin)
            } else {
                BodyKey::Other(origin)
            }
        }
    }

    fn proxy(self) -> Option<raw::BtBroadphaseProxy> {
        // SAFETY: a live body's proxy field.
        unsafe { self.0.broadphase() }
    }
}

/// A persistent manifold of the dispatcher.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Manifold(raw::BtPersistentManifold);

impl Manifold {
    pub fn bodies(self) -> Option<(Body, Body)> {
        // SAFETY: a live manifold's bodies, set by its constructor.
        unsafe { Some((Body(self.0.body0()?), Body(self.0.body1()?))) }
    }

    /// Its points' count, 0..=4.
    pub fn points(self) -> usize {
        // SAFETY: a live manifold's m_cachedPoints.
        (unsafe { read(self.0.cached()) } as usize).min(4)
    }

    fn point(self, i: usize) -> *mut u8 {
        // SAFETY: i below 4: inside m_pointCache.
        unsafe { self.0.points().at(i.min(3)).words().cast::<u8>() }
    }

    /// Point `i`'s bytes (i below `points`).
    pub fn read_point(self, i: usize) -> PointBytes {
        let mut out = [0u8; POINT_BYTES];
        let base = self.point(i);
        let mut at = 0;
        for (offset, len) in POINT_FIELDS {
            for k in 0..len {
                // SAFETY: inside the point (every field below +0C0h).
                out[at] = unsafe { base.add(offset + k).read_volatile() };
                at += 1;
            }
        }
        out
    }

    /// The manifold holds `points` and nothing else (each point's fields the
    /// world leaves out zero, as a new point's: the constructor 0x8F1B0
    /// zeroes the user pointer and the rows' accumulated impulses).
    pub fn fill(self, points: &[PointBytes]) {
        let n = points.len().min(4);
        for (i, point) in points.iter().take(n).enumerate() {
            let base = self.point(i);
            for k in 0..POINT_SIZE {
                // SAFETY: the whole point, the manifold's own memory.
                unsafe { base.add(k).write_volatile(0) };
            }
            let mut at = 0;
            for (offset, len) in POINT_FIELDS {
                for k in 0..len {
                    // SAFETY: as above.
                    unsafe { base.add(offset + k).write_volatile(point[at]) };
                    at += 1;
                }
            }
            // m_index0/1 are inside the 28 bytes at +50h: written above.
        }
        // SAFETY: the manifold's m_cachedPoints.
        unsafe { self.0.cached().write_volatile(n as u32) };
    }

    /// No points (clearManifold 0x8F5E0 with no destroy callback).
    pub fn clear(self) {
        // SAFETY: as fill.
        unsafe { self.0.cached().write_volatile(0) };
    }

    /// Whether these are its two bodies, in either order.
    pub fn joins(self, a: Body, b: Body) -> bool {
        self.bodies().is_some_and(|(x, y)| (x == a && y == b) || (x == b && y == a))
    }
}

/// A broadphase pair (btBroadphasePair).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Pair(raw::BtBroadphasePair);

impl Pair {
    /// Its two bodies, proxy 0's first.
    pub fn bodies(self) -> Option<(Body, Body)> {
        // SAFETY: a live pair's proxies and their bodies.
        unsafe { Some((Body(self.0.proxy0()?.client()?), Body(self.0.proxy1()?.client()?))) }
    }

    /// The body the near callback passes first (+0Ch: 1 proxy 1's).
    pub fn first(self) -> Option<Body> {
        let (a, b) = self.bodies()?;
        // SAFETY: the pair's +0Ch.
        Some(if unsafe { read(self.0.swapped()) } != 0 { b } else { a })
    }

    pub fn has_algorithm(self) -> bool {
        // SAFETY: the pair's m_algorithm.
        unsafe { self.0.algorithm().is_some() }
    }

    pub fn address(self) -> u32 {
        self.0.address()
    }
}

/// The dispatcher's manifolds.
#[derive(Clone, Copy)]
pub struct Dispatcher(raw::BtDispatcher);

impl Dispatcher {
    pub fn id(self) -> DispatcherId {
        DispatcherId(self.0.address())
    }

    /// Its manifolds, in its array's order.
    pub fn manifolds(self) -> impl Iterator<Item = Manifold> {
        // SAFETY: the live dispatcher's m_manifoldsPtr {size +0Ch, data +14h}.
        let (count, first) = unsafe { (read(self.0.manifold_count()) as usize, self.0.manifolds()) };
        (0..if first.is_some() { count.min(4096) } else { 0 }).filter_map(move |i| {
            // SAFETY: below the array's size; an entry is a manifold.
            unsafe { first?.at(i).manifold().map(Manifold) }
        })
    }
}

/// A physics world: its dispatcher, its bodies, its broadphase's pairs.
#[derive(Clone, Copy)]
pub struct World {
    dynamics: raw::BtDynamicsWorld,
    dispatcher: raw::BtDispatcher,
    cache: raw::BtPairCache,
}

impl World {
    fn of_dynamics(dynamics: raw::BtDynamicsWorld) -> Option<World> {
        // SAFETY: a live btDiscreteDynamicsWorld's dispatcher and broadphase.
        unsafe { Some(World { dynamics, dispatcher: dynamics.dispatcher()?, cache: dynamics.broadphase()?.pair_cache()? }) }
    }

    fn of_core(core: raw::CoreWorld) -> Option<World> {
        // SAFETY: the core world's physics world and its dynamics world.
        World::of_dynamics(unsafe { core.physics()?.dynamics()? })
    }

    pub fn dispatcher(self) -> Dispatcher {
        Dispatcher(self.dispatcher)
    }

    /// Its bodies.
    pub fn bodies(self) -> impl Iterator<Item = Body> {
        // SAFETY: m_collisionObjects {size +8, data +10h}.
        let (count, first) = unsafe { (read(self.dynamics.object_count()) as usize, self.dynamics.objects()) };
        (0..if first.is_some() { count.min(usize::from(u16::MAX)) } else { 0 }).filter_map(move |i| {
            // SAFETY: below the array's size; an entry is a body.
            unsafe { first?.at(i).object().map(Body) }
        })
    }

    /// The pair of two bodies, when their boxes overlap.
    pub fn pair(self, a: Body, b: Body) -> Option<Pair> {
        // SAFETY: findPair 0xB41D0 on the world's pair cache with two live
        // proxies; it only reads.
        let (pa, pb) = (a.proxy()?, b.proxy()?);
        unsafe { raw::engine_pair_find(Some(self.cache), Some(pa), Some(pb)).map(Pair) }
    }

    /// The pair's algorithm destroyed (cleanOverlappingPair 0xB40E0, as the
    /// near callback does when the order flips: its manifolds released).
    pub fn clean(self, pair: Pair) {
        // SAFETY: a live pair of this world's cache, outside any dispatch.
        unsafe { raw::engine_pair_clean(Some(self.cache), Some(pair.0), Some(self.dispatcher)) }
    }

    /// The pair's algorithm made as the near callback makes it (0x8F990..
    /// 0x8F9B1), `first` passed first, both bodies awake meanwhile (an
    /// algorithm that makes its manifold in its constructor makes none for
    /// two asleep, needsCollision 0x8F770).
    pub fn make(self, pair: Pair, first: Body, second: Body) {
        let Some((proxy0, _)) = pair.bodies() else { return };
        // SAFETY: live bodies' activation states, put back after; the pair's
        // own fields; findAlgorithm 0x8F6E0 with no shared manifold.
        unsafe {
            let states = [read(first.0.activation()), read(second.0.activation())];
            first.0.activation().write_volatile(ACTIVE);
            second.0.activation().write_volatile(ACTIVE);
            pair.0.swapped().write_volatile(u32::from(proxy0 != first));
            let algorithm = raw::engine_algorithm_find(Some(self.dispatcher), Some(first.0), Some(second.0), None);
            pair.0.set_algorithm(algorithm);
            first.0.activation().write_volatile(states[0]);
            second.0.activation().write_volatile(states[1]);
        }
    }
}

/// `game_world_core::serialize` after the physics world's own (nothing):
/// the core's physics world and the history item's physics writer.
pub struct Saving {
    pub world: World,
    writer: raw::BufferWriter,
}

impl Saving {
    pub fn at(ctx: &Context<'_>) -> Option<Saving> {
        use raw::site::physics_contacts_saved as site;
        // SAFETY: the site's ESI (the history item being written) and EBP
        // (the core world).
        unsafe {
            let item = site::item(ctx)?;
            Some(Saving { world: World::of_core(site::world(ctx)?)?, writer: item.physics_writer() })
        }
    }

    /// Appends bytes to the physics section.
    pub fn write(&self, bytes: &[u8]) {
        super::carry_runtime::write_bytes(self.writer, bytes);
    }
}

/// The end of a world's load (keeper.world_loaded): the core's physics world
/// and the physics section's bytes of the history item it loaded.
pub struct Loaded<'c> {
    pub world: World,
    pub bytes: &'c [u8],
}

impl<'c> Loaded<'c> {
    pub fn at(ctx: &'c Context<'c>) -> Option<Loaded<'c>> {
        use raw::site::keeper_world_loaded as site;
        // SAFETY: the site's EBX (the history item world apply read, alive
        // through the load) and EBP (the core world); the item's physics
        // buffer {data, size}, the wire reader's or the history's.
        unsafe {
            let item = site::item(ctx)?;
            let world = World::of_core(site::world(ctx)?)?;
            let size = read(item.physics_size()) as usize;
            let data = item.physics_bytes();
            let bytes: &[u8] = if !data.is_null() && size > 0 && size <= 0xF000 {
                core::slice::from_raw_parts(data.cast_const(), size)
            } else {
                &[]
            };
            Some(Loaded { world, bytes })
        }
    }
}

/// The near callback before a pair's processCollision: the pair and its
/// dispatcher.
pub fn near_process(ctx: &Context<'_>) -> Option<(Dispatcher, Pair)> {
    use raw::site::physics_contacts_near_process as site;
    // SAFETY: the site's ESI and EDI, live through the callback.
    unsafe { Some((Dispatcher(site::dispatcher(ctx)?), Pair(site::pair(ctx)?))) }
}

/// getNewManifold before it appends the manifold: its dispatcher and it.
pub fn new_manifold(ctx: &Context<'_>) -> Option<(Dispatcher, Manifold)> {
    use raw::site::physics_contacts_new_manifold as site;
    // SAFETY: the site's EDI and EBX (0 when no manifold was made).
    unsafe { Some((Dispatcher(site::dispatcher(ctx)?), Manifold(site::manifold(ctx)?))) }
}

/// internalSingleStepSimulation after the step's collision detection.
pub fn detected(ctx: &Context<'_>) -> Option<World> {
    use raw::site::physics_contacts_detected as site;
    // SAFETY: the site's ESI, the dynamics world being stepped.
    World::of_dynamics(unsafe { site::world(ctx)? })
}

/// solveGroup's manifolds, put in `order` (a stable sort by its key) before
/// its setup reads them.
pub fn sort_solve<K: Ord + Copy>(ctx: &Context<'_>, key: impl Fn(Manifold) -> K) {
    use raw::site::physics_contacts_solve as site;
    // SAFETY: solveGroup's arguments by its entry ESP: an array of
    // `manifold_count` manifold pointers (the island callback's batch or the
    // island manager's, rebuilt every step), ours to reorder in place.
    unsafe {
        let Some(frame) = site::frame(ctx) else { return };
        let count = read(frame.manifold_count()) as usize;
        let Some(first) = frame.manifolds() else { return };
        if !(2..=4096).contains(&count) {
            return;
        }
        // insertion sort: stable, no buffer; the batches are small
        for i in 1..count {
            let slot = first.at(i);
            let Some(moving) = slot.manifold() else { continue };
            let k = key(Manifold(moving));
            let mut j = i;
            while j > 0 {
                let before = first.at(j - 1);
                let Some(m) = before.manifold() else { break };
                if key(Manifold(m)) <= k {
                    break;
                }
                first.at(j).set_manifold(Some(m));
                j -= 1;
            }
            first.at(j).set_manifold(Some(moving));
        }
    }
}
