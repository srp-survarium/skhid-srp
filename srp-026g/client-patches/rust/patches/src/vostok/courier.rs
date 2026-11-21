//! Courier's native sites (patch 109, crate::artefact_courier): the respawn rule's spawn
//! (0x316DA0) at its start, the place's kit and the one it spawns in, and at its end, the new
//! kit's player and its place's kits. The move is the inventory's (`Inventory::take_from_dead_kit`).
use super::inventory::Inventory;
use super::world::read;
use super::{Context, Thread, artefact_skills};
use crate::artefact_courier::{Carried, Kit};
use crate::artefact_skills::Skills;
use crate::raw;

/// The spawn's start (`courier.spawn_kits`): its place, the place's kit and the kit it spawns in.
pub fn spawn_kits(ctx: &Context<'_>) -> Option<(u32, u8, u8)> {
    use raw::site::courier_spawn_kits as site;
    // SAFETY: the spawn's BL (EBX, the place: movzx eax,bl at 0x316DA7) and EAX, its entry of the
    // core's places ([core+18458h] + place x 30h, 0x316DC2..0x316DD4).
    let (place, entry) = unsafe { (site::place(ctx) & 0xFF, site::entry(ctx)?) };
    // SAFETY: the live entry's kit bytes; +0Ch is written only after this site (0x316DE3).
    Some(unsafe { (place, read(entry.kit()), read(entry.next_kit())) })
}

/// The spawn's end (`courier.spawned`): the new kit's player, initialised and in the world.
pub struct Spawned<'t> {
    player: raw::Player,
    entry: raw::CorePlace,
    place: u8,
    kit: Kit,
    thread: &'t Thread,
}

impl<'t> Spawned<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Spawned<'t>> {
        use raw::site::courier_spawned as site;
        // SAFETY: the spawn's ESI, the new current kit's player, and BL its place.
        let (player, place) = unsafe { (site::player(ctx)?, (site::place(ctx) & 0xFF) as u8) };
        // SAFETY: the live player's world core; its places vector, bounded by its end.
        let entry = unsafe {
            let world = player.world_core()?;
            let (first, end) = world.places().zip(world.places_end())?;
            (usize::from(place) < end.index_from(first)?).then(|| first.at(usize::from(place)))?
        };
        // SAFETY: the live entry's kit, the spawn's new one (0x316DE3).
        let kit = Kit::parse(unsafe { read(entry.kit()) })?;
        // The spawn's player is its place's current kit's.
        (Self::kit_player(entry, kit)? == player).then_some(Spawned { player, entry, place, kit, thread: ctx.thread() })
    }

    fn kit_player(entry: raw::CorePlace, kit: Kit) -> Option<raw::Player> {
        // SAFETY: the live entry's three kit players, kept while the match lasts.
        unsafe {
            match kit.get() {
                0 => entry.kit0(),
                1 => entry.kit1(),
                _ => entry.kit2(),
            }
        }
    }

    pub fn place(&self) -> u8 {
        self.place
    }

    pub fn kit(&self) -> Kit {
        self.kit
    }

    /// The new kit's packed artefact skills.
    pub fn skills(&self) -> Skills {
        artefact_skills::skills(self.player)
    }

    /// The artefacts the death left in kit `from` into this kit's empty containers.
    pub fn carry_from(&self, from: Kit) -> Carried {
        let Some(donor) = Self::kit_player(self.entry, from).filter(|&donor| donor != self.player) else {
            return Carried::default();
        };
        match (Inventory::of_player(self.player, self.thread), Inventory::of_player(donor, self.thread)) {
            (Some(mut recipient), Some(mut donor)) => recipient.take_from_dead_kit(&mut donor),
            _ => Carried::default(),
        }
    }
}
