//! Spotting: base_player::apply_new_input (0x2D86E0) walks a spot ray's hits
//! and asks each spottable hit's owner for a player (slot 2) or a geometry
//! (slot 1). At `spot.hit_owner` (sites_026g.tsv) the hit had no player
//! behind it: ESI is the hit's object pointer (hit+1Ch), EDI the spotter.
//! What those registers hold is said here, once.

use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, pointer, vtable};
use crate::raw;

/// The hit `spot.hit_owner` looks at, and who spots.
pub struct SpotHit<'t> {
    owner: Option<Object>,
    spotter: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> SpotHit<'t> {
    /// The hit at the site: its object's owner (`mov ecx,[esi]; mov ecx,
    /// [ecx+0Ch]`, read as the game reads it) and the spotter (EDI).
    pub fn at(ctx: &'t Context<'_>) -> SpotHit<'t> {
        // SAFETY: at the site ESI is the hit's object pointer, a live
        // collision object's address, whose +0Ch is its owner (the span the
        // site replaces reads both).
        let owner = unsafe {
            Object::at(ctx.reg(Reg::Esi))
                .and_then(|at| pointer(at.ptr().cast()))
                .and_then(|object| pointer(raw::collision_object::owner(object.ptr()).cast()))
        };
        SpotHit { owner, spotter: ctx.reg(Reg::Edi), thread: PhantomData }
    }

    /// The battery whose rigid body was hit: the owner is a
    /// victory_item_core's body owner (its vtable), whose own slots answer no
    /// player and no geometry.
    pub fn battery(&self) -> Option<BatterySpottable<'t>> {
        let owner = self.owner?;
        // SAFETY: the owner a hit's object names, a live object with a vtable.
        if unsafe { vtable(owner) } != raw::image::VICTORY_ITEM_BODY_OWNER_VTABLE {
            return None;
        }
        // SAFETY: the owner is a victory_item_core's body owner (its
        // vtable): the item's spottable interface is beside it.
        let spottable = Object::at(unsafe { raw::victory_item_body_owner::spottable(owner.ptr()) } as usize as u32)?;
        Some(BatterySpottable { spottable, spotter: self.spotter, thread: PhantomData })
    }
}

/// A battery's spottable interface (victory_item_core+54h), asked as the
/// game asks a geometry's first spottable (0x2D8A50..0x2D8A65): through its
/// vtable.
pub struct BatterySpottable<'t> {
    spottable: Object,
    spotter: u32,
    thread: PhantomData<&'t Thread>,
}

impl BatterySpottable<'_> {
    /// Whether the spotter may spot it (its vtable's slot 1).
    pub fn can_be_spotted(&self) -> bool {
        // SAFETY: a battery's spottable interface (its owner's vtable said
        // so), the spotter the game's register at the site.
        unsafe { raw::engine_spottable_can_be_spotted_by(self.spottable.ptr(), self.spotter as usize as *mut _) != 0 }
    }

    /// Spots it for the spotter's team (its vtable's slot 0).
    pub fn set_spotted(&self) {
        // SAFETY: as can_be_spotted.
        unsafe { raw::engine_spottable_set_spotted(self.spottable.ptr(), self.spotter as usize as *mut _) }
    }
}
