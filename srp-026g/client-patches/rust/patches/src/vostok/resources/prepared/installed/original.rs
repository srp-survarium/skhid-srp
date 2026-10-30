//! Simulation bytes for the displaced original, owned by InstalledPickup.
//! Capturing scalars avoids the native writer's optional debug-annotation
//! allocations. The byte layout is checked against the original executable.

use super::{InstallError, InstalledPickup, SavedSlot};
use crate::raw;
use crate::vostok::world::{pointer, read};
use weapon_pickup::{Chamber, Original};

/// Validate before either snapshot slot mutates. The old original may still
/// be held here; the native player reader owns its later deactivation.
pub(super) fn validate(saved: &SavedSlot, state: Original) -> Result<(), InstallError> {
    unsafe {
        let weapon = saved.original.ptr();
        let item = raw::weapon_core::item(weapon).cast();
        let (place, kit, slot) = state.owner();
        if place.get() != read(raw::player::place(saved.player.ptr()))
            || kit.get() != read(raw::player::kit(saved.player.ptr()))
            || slot != saved.slot
            || read(raw::weapon_core::inventory(weapon)) != saved.inventory.id()
            || read(raw::weapon_core::item_core(weapon)) != saved.core.id()
            || read(raw::player::inventory(saved.player.ptr())) != saved.inventory.id()
            || read(raw::player::core(saved.player.ptr())) != saved.core.id()
            || read(raw::inventory_item::slot(item)) != saved.slot.native()
        {
            return Err(InstallError::WrongWeapon);
        }
        let entry = pointer(raw::item::entry(item)).ok_or(InstallError::WrongWeapon)?;
        if read(raw::item_entry::stack(entry.ptr())) != 0 {
            return Err(InstallError::WrongWeapon);
        }
        // Native item decoding follows this roster lookup even for a stowed
        // weapon. Bound it and require the exact player before entering FFI.
        let begin = read(raw::game_core::roster_begin(saved.core.ptr()));
        let end = read(raw::game_core::roster_end(saved.core.ptr()));
        if begin == 0 || end <= begin {
            return Err(InstallError::WrongWeapon);
        }
        let stride = raw::roster_place::end(begin as *mut core::ffi::c_void) as u32 - begin;
        let offset = u32::from(place.get()) * stride;
        if !(end - begin).is_multiple_of(stride)
            || end - begin > 32 * stride
            || offset >= end - begin
        {
            return Err(InstallError::WrongWeapon);
        }
        let record = (begin + offset) as *mut core::ffi::c_void;
        if read(raw::roster_place::kits(record).add(usize::from(kit.get()))) != saved.player.id() {
            return Err(InstallError::WrongWeapon);
        }
        let ammo = state
            .ammunition(read(raw::weapon_core::magazine_capacity(weapon)))
            .ok_or(InstallError::InvalidSupply)?;
        if (ammo.chamber() != Chamber::None) != (read(raw::weapon_core::has_chamber(weapon)) != 0) {
            return Err(InstallError::InvalidSupply);
        }
        let count = read(raw::weapon_core::ammo_slot_count(weapon));
        if !(1..=8).contains(&count) || ammo.kind().get() >= count {
            return Err(InstallError::InvalidSupply);
        }
        let slots =
            pointer(raw::weapon_core::ammo_slots(weapon)).ok_or(InstallError::InvalidSupply)?;
        let old = read(raw::weapon_core::ammo(weapon));
        let mut old_owned = old == 0;
        for index in 0..usize::from(count) {
            let slot = read(slots.ptr().cast::<u32>().add(index));
            if ![8, 9, 11, 12, 19, 20, 21, 22].contains(&slot) {
                return Err(InstallError::InvalidSupply);
            }
            let stack = pointer(raw::inventory::slots(saved.inventory.ptr()).add(slot as usize))
                .ok_or(InstallError::InvalidSupply)?;
            if read(raw::inventory_item::vtable(stack.ptr())) != raw::image::AMMO_ITEM_VTABLE
                || read(raw::inventory_item::owner(stack.ptr())) != saved.inventory.id()
                || read(raw::inventory_item::slot(stack.ptr())) != slot
            {
                return Err(InstallError::InvalidSupply);
            }
            old_owned |= stack.id() == old;
        }
        if !old_owned {
            return Err(InstallError::InvalidSupply);
        }
        Ok(())
    }
}

/// Only the coordinated snapshot transaction calls this, after native player
/// decoding. Violating that unsafe callback contract stops before native
/// decoding can read a compact stowed record as an active-controller record.
pub(super) fn apply(owner: &InstalledPickup, state: Original) {
    assert!(
        validate(&owner.saved, state).is_ok(),
        "original binding changed during player decoding"
    );
    unsafe {
        assert_eq!(
            read(raw::weapon_core::user(owner.saved.original.ptr())),
            0,
            "native player decoder did not deactivate the displaced original"
        );
        let bytes = state.bytes();
        let begin = bytes.as_ptr() as u32;
        let mut reader = [begin, begin, bytes.len() as u32];
        raw::engine_weapon_core_deserialize(
            owner.saved.original.ptr(),
            reader.as_mut_ptr().cast(),
            core::ptr::null_mut(),
            0,
        );
        assert_eq!(
            reader[1],
            begin + bytes.len() as u32,
            "stowed weapon decoder length changed"
        );
        raw::inventory::weight_dirty(owner.saved.inventory.ptr()).write(1);
    }
}

impl InstalledPickup {
    /// Capture the complete extra state of one borrowed slot. History checks
    /// that this original belongs to the recipient's exact place/kit/slot.
    pub fn snapshot_slot(&self) -> Result<weapon_pickup::history::SlotState, InstallError> {
        Ok(weapon_pickup::history::SlotState {
            original: self.snapshot_original()?,
            reserves: self
                .snapshot_reserves()
                .map_err(|_| InstallError::InvalidSupply)?,
        })
    }

    /// Capture the retained, deactivated original's simulation state. The
    /// native inventory snapshot now contains the borrowed weapon instead,
    /// so this separate state must accompany it in the history extension.
    /// Client-only presentation/controller state is not part of this payload.
    pub fn snapshot_original(&self) -> Result<Original, InstallError> {
        self.pickup
            .refill_plan(self.saved.inventory)
            .map_err(|_| InstallError::WrongWeapon)?;
        let saved = &self.saved;
        // SAFETY: this owner retains the displaced original and the install
        // contract keeps its player/inventory alive. No active/controller
        // state is accessed. Every captured value is copied, never borrowed.
        unsafe {
            let weapon = saved.original.ptr();
            let item = raw::weapon_core::item(weapon).cast();
            if read(raw::weapon_core::user(weapon)) != 0
                || read(raw::weapon_core::inventory(weapon)) != saved.inventory.id()
                || read(raw::inventory_item::slot(item)) != saved.slot.native()
                || read(raw::player::inventory(saved.player.ptr())) != saved.inventory.id()
            {
                return Err(InstallError::WrongWeapon);
            }
            let entry = pointer(raw::item::entry(item)).ok_or(InstallError::WrongWeapon)?;
            if read(raw::item_entry::stack(entry.ptr())) != 0 {
                return Err(InstallError::WrongWeapon);
            }
            let count = read(raw::weapon_core::ammo_slot_count(weapon));
            let kind = read(raw::weapon_core::ammo_kind(weapon));
            if !(1..=8).contains(&count) || kind >= count {
                return Err(InstallError::InvalidSupply);
            }
            let mut bytes = [0; 20];
            bytes[..4].copy_from_slice(&read(raw::item::condition_bits(item)).to_le_bytes());
            bytes[4] = read(raw::player::place(saved.player.ptr()));
            bytes[5] = read(raw::player::kit(saved.player.ptr()));
            bytes[6] = saved.slot.native() as u8;
            bytes[7..11].copy_from_slice(
                &read(raw::weapon_core::snapshot_state_word(weapon)).to_le_bytes(),
            );
            bytes[11..15]
                .copy_from_slice(&read(raw::weapon_core::last_tick_time(weapon)).to_le_bytes());
            bytes[15] = if read(raw::weapon_core::load_on_initialize(weapon)) != 0 {
                255
            } else {
                0
            };
            bytes[16] = read(raw::weapon_core::magazine(weapon));
            bytes[17] = read(raw::weapon_core::fire_queue_type(weapon));
            bytes[18] = kind;
            bytes[19] = if read(raw::weapon_core::chambered(weapon)) != 0 {
                255
            } else {
                0
            };
            let size = if read(raw::weapon_core::has_chamber(weapon)) == 0 {
                19
            } else {
                20
            };
            let state = Original::parse(&bytes[..size]).ok_or(InstallError::WrongWeapon)?;
            state
                .ammunition(read(raw::weapon_core::magazine_capacity(weapon)))
                .ok_or(InstallError::InvalidSupply)?;
            Ok(state)
        }
    }
}
