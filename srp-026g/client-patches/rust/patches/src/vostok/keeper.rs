//! Keeper of the Artifacts' native sites (patch 109, artefact-skills.md "Keeper of the
//! Artifacts"): the map's hidden Keeper pool, as the core world registers it, and a spawn's
//! player, kit, statistics and first container.
//!
//! - The pool: the artefacts of the map's anomaly with no container (content edit
//!   artefact-keeper-1.json; no anomaly of the stock maps has none), registered by
//!   generic_anomaly_core::register_artefacts (0x31B8E0) at the core world's artefact vector's
//!   length on, in pool order. Every resolve links rebuilds the vector from empty, so the first
//!   registration of a load sees it empty and forgets the last map's pool. Every game, a late
//!   joiner's too, registers the same map the same way: the same ids.
//! - What the pool's artefacts are is native: the anomaly ticks them, its save writes them in
//!   every world, the inventory's save names the one a container holds.
use super::world::read;
use super::{Context, GameCell, Thread, artefact_skills, statistics};
use crate::artefact_keeper::{Registered, Room, Standing, Whereabouts};
use crate::artefact_skills::Skills;
use crate::raw;
use artefact_carry::ArtefactId;
use artefact_carry::keeper::Kind;

/// The map's Keeper pool: its world's ids `first .. first + count`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Pool {
    world: u32,
    first: u8,
    count: u8,
}

static POOL: GameCell<Option<Pool>> = GameCell::new(None);

/// The core world's registered artefacts (+18940h), each with its id.
fn registered(world: raw::CoreWorld) -> impl Iterator<Item = (ArtefactId, raw::Artefact)> + Clone {
    // SAFETY: the live core world's vector of registered artefacts (inventory::deserialize
    // 0x2FE655 indexes it by id); its bounded length is checked before indexing.
    let (first, count) = unsafe {
        match (world.artefacts_begin(), world.artefacts_end()) {
            (Some(first), Some(end)) if first.address().is_multiple_of(4) => (Some(first), end.index_from(first).unwrap_or(0)),
            _ => (None, 0),
        }
    };
    (0..count.min(255)).filter_map(move |n| {
        // SAFETY: below the vector's length; an entry is an artefact or null.
        let item = unsafe { first?.at(n).item()?.artefact() };
        Some((ArtefactId::parse(n as u8)?, item))
    })
}

/// A registered artefact's kind, by its client class's vtable; none for another kind (the
/// Rattle).
fn kind(item: raw::Artefact) -> Option<Kind> {
    // SAFETY: a registered artefact's primary vtable.
    match unsafe { read(item.item().vtable()) } {
        raw::image::LIFEBONE_VTABLE => Some(Kind::Larkspur),
        raw::image::ONYX_VTABLE => Some(Kind::Onyx),
        raw::image::SPRING_VTABLE => Some(Kind::Spring),
        _ => None,
    }
}

/// generic_anomaly_core::register_artefacts' entry: a load's first registration forgets the
/// last pool; an anomaly with no container and a pool is the Keeper pool.
pub fn register(ctx: &Context<'_>) {
    use raw::site::keeper_register as site;
    // SAFETY: the site's arguments (by its ESP), the anomaly and the core world it registers
    // into, alive through the load's resolve links.
    let Some(frame) = (unsafe { site::frame(ctx) }) else { return };
    // SAFETY: as above.
    let (Some(anomaly), Some(world)) = (unsafe { frame.anomaly() }, unsafe { frame.world() }) else { return };
    // SAFETY: the core world's artefact vector bounds; the anomaly's container vector bounds
    // and pool count, set by its load.
    let (start, hidden, count) = unsafe {
        let start = world.artefacts_begin().zip(world.artefacts_end()).and_then(|(b, e)| e.index_from(b));
        (start, read(anomaly.containers_begin()) == read(anomaly.containers_end()), read(anomaly.pool_count()))
    };
    let Some(start) = start else { return };
    POOL.update(ctx.thread(), |pool| {
        // A load's first registration: the last map's pool is gone.
        let pool = pool.filter(|_| start != 0);
        match (pool, u8::try_from(start), u8::try_from(count)) {
            (None, Ok(first), Ok(count)) if hidden && count > 0 => Some(Pool { world: world.address(), first, count }),
            _ => pool,
        }
    });
}

/// A player at its tick (base_player::tick before tick_animations, `battery.carrier_tick`), its
/// world core's.
pub struct Recipient<'t> {
    player: raw::Player,
    world: raw::CoreWorld,
    thread: &'t Thread,
}

impl<'t> Recipient<'t> {
    pub fn at_tick(ctx: &'t Context<'_>) -> Option<Recipient<'t>> {
        // SAFETY: base_player::tick's EBX, its live player; its world core.
        let player = unsafe { raw::site::battery_carrier_tick::player(ctx)? };
        let world = unsafe { player.world_core()? };
        Some(Recipient { player, world, thread: ctx.thread() })
    }

    pub fn place(&self) -> u8 {
        // SAFETY: the live player's place byte.
        unsafe { read(self.player.place()) }
    }

    /// Its kit's packed artefact skills (modifier 40: the Keeper bit, the match's draw, srp's
    /// late joiner's mark).
    pub fn skills(&self) -> Skills {
        artefact_skills::skills(self.player)
    }

    /// The match's standing for its place: not started before the time rule is in process (core
    /// +18FE4h 3; 1 waiting, 2 the countdown), else the deaths and suicides the statistics count for
    /// the place.
    pub fn standing(&self) -> Standing {
        // SAFETY: the world core's time state, a u32 the time rule writes.
        if unsafe { read(self.world.time_state()) } != 3 {
            return Standing::NotStarted;
        }
        // SAFETY: the world core's live statistics handler, when set.
        let shared = unsafe { self.world.statistics().map(|s| s.shared()) };
        Standing::Started { deaths: shared.map_or(0, |shared| statistics::deaths(shared, self.place())) }
    }

    /// Its artefact containers, as a gift can go into them: none while it is dead.
    pub fn room(&self) -> Room {
        // SAFETY: the live player's alive byte.
        if unsafe { read(self.player.alive()) } == 0 {
            return Room::None;
        }
        match super::inventory::Inventory::of_player(self.player, self.thread).map(|i| i.container_space()) {
            Some(super::inventory::ContainerSpace::Available) => Room::Free,
            Some(super::inventory::ContainerSpace::Full) => Room::Full,
            Some(super::inventory::ContainerSpace::Missing) | None => Room::None,
        }
    }

    /// The map's Keeper pool (none on a map without it), each artefact with its kind and
    /// whereabouts, in id order.
    pub fn pool(&self) -> impl Iterator<Item = Registered> + Clone + use<'t> {
        let pool = POOL.get(self.thread).filter(|pool| pool.world == self.world.address());
        registered(self.world).filter_map(move |(id, item)| {
            let pool = pool?;
            (pool.first..pool.first.saturating_add(pool.count)).contains(&id.get()).then_some(())?;
            // SAFETY: a registered artefact: state 0 and no inventory is a dormant one; its count
            // is 0 until a take gives it one (inventory::take_dormant).
            let whereabouts = unsafe {
                match (read(item.state()) == 0 && item.item().inventory().is_none(), read(item.item().count())) {
                    (false, _) => Whereabouts::InPlay,
                    (true, 0) => Whereabouts::Unused,
                    (true, _) => Whereabouts::Returned,
                }
            };
            Some(Registered { id, kind: kind(item)?, whereabouts })
        })
    }

    /// Gives the player the pool's artefact `id` into its first empty container; false when it
    /// is no longer unused or no container is empty.
    pub fn give(&self, id: ArtefactId) -> bool {
        let Some((_, item)) = registered(self.world).find(|(at, _)| *at == id) else { return false };
        super::inventory::Inventory::of_player(self.player, self.thread)
            .is_some_and(|mut inventory| inventory.take_dormant(item))
    }
}
