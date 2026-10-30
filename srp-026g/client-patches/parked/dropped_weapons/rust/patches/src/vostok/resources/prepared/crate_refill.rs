//! Coordinate native profile refill with private borrowed ammunition. The
//! native routines receive a temporary slot view, never a live weapon swap.

use super::{CrateCompletion, CrateInventory, InstalledPickup, Refill, ResupplyError};
use crate::raw;
use crate::vostok::world::{Object, pointer, read};
use weapon_pickup::WeaponSlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrateAvailability {
    Available,
    EmptyStorage,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrateError {
    WrongPlayer,
    InvalidProfile,
    InvalidOriginal,
    Borrowed(ResupplyError),
    NativeResult,
}

struct OriginalLoadoutView {
    // 392 bytes, through inventory::weight_dirty, aligned as native u32s.
    // Only the 23 slot pointers are borrowed. This is NOT an inventory
    // resource: no vtable, owner, refcount, destructor or escaped address.
    // Original 2FDC10/2FDF50 only read slots and write weight_dirty here;
    // check_weapon_pickup.py traces those accesses in the original exe.
    storage: [u32; 98],
    profile: Object,
    kit: u8,
}

impl OriginalLoadoutView {
    fn new(
        player: Object,
        inventory: Object,
        borrowed: [Option<&InstalledPickup>; 2],
    ) -> Result<Self, CrateError> {
        // SAFETY: Player and crate token borrow live engine objects for this
        // hook. Each installed owner retains its displaced original weapon.
        unsafe {
            if pointer(raw::player::inventory(player.ptr())) != Some(inventory) {
                return Err(CrateError::WrongPlayer);
            }
            let profile =
                pointer(raw::player::profile(player.ptr())).ok_or(CrateError::InvalidProfile)?;
            let kit = read(raw::player::kit(player.ptr()));
            let count = read(raw::player_profile::kit_count(profile.ptr()));
            if count > 3 || kit >= count {
                return Err(CrateError::InvalidProfile);
            }
            let mut view = Self {
                storage: [0; 98],
                profile,
                kit,
            };
            let slots = raw::inventory::slots(view.storage.as_mut_ptr().cast());
            core::ptr::copy_nonoverlapping(raw::inventory::slots(inventory.ptr()), slots, 23);
            for (index, slot) in [WeaponSlot::First, WeaponSlot::Second]
                .into_iter()
                .enumerate()
            {
                let cell = slots.add(slot.native() as usize);
                if let Some(owner) = borrowed[index] {
                    let original = owner
                        .original_for_crate(inventory, slot)
                        .ok_or(CrateError::InvalidOriginal)?;
                    cell.write(raw::weapon_core::item(original.ptr()) as u32);
                }
                // Missing or stale match-bank entries must not silently
                // account a borrowed external ammo slot as original equipment.
                if let Some(item) = pointer(cell) {
                    if ![
                        raw::image::WEAPON_ITEM_VTABLE,
                        raw::image::WEAPON_CORE_ITEM_VTABLE,
                    ]
                    .contains(&read(raw::inventory_item::vtable(item.ptr())))
                        || read(raw::inventory_item::owner(item.ptr())) != inventory.id()
                        || read(raw::inventory_item::slot(item.ptr())) != slot.native()
                    {
                        return Err(CrateError::InvalidOriginal);
                    }
                    let weapon = raw::weapon_item::active_object(item.ptr()).cast();
                    let kind = read(raw::weapon_core::ammo_kind(weapon));
                    let count = read(raw::weapon_core::ammo_slot_count(weapon));
                    let ammo_slots = pointer(raw::weapon_core::ammo_slots(weapon))
                        .ok_or(CrateError::InvalidOriginal)?;
                    if count > 8 || kind >= count {
                        return Err(CrateError::InvalidOriginal);
                    }
                    let ammo_slot = read(ammo_slots.ptr().cast::<u32>().add(usize::from(kind)));
                    if ![8, 9, 11, 12, 19, 20, 21, 22].contains(&ammo_slot) {
                        return Err(CrateError::InvalidOriginal);
                    }
                }
            }
            Ok(view)
        }
    }

    fn availability(&mut self) -> Result<CrateAvailability, CrateError> {
        // SAFETY: the view's slot borrows stay live across this synchronous
        // native call. Item callbacks still see their real inventory owner.
        match unsafe {
            raw::engine_inventory_check_resupply(
                u32::from(self.kit),
                self.storage.as_mut_ptr().cast(),
                self.profile.ptr(),
            )
        } {
            0 => Ok(CrateAvailability::Available),
            1 => Ok(CrateAvailability::EmptyStorage),
            2 => Ok(CrateAvailability::Full),
            _ => Err(CrateError::NativeResult),
        }
    }

    fn refill(&mut self, inventory: Object) {
        // SAFETY: same borrow as availability. These native routines never
        // store the view pointer in an item; their intrusive item refs balance
        // before return. Propagate the outer routine's weight invalidation to
        // the actual inventory even if no ammo-item setter ran.
        unsafe {
            raw::engine_inventory_resupply(
                self.storage.as_mut_ptr().cast(),
                self.profile.ptr(),
                u32::from(self.kit),
            );
            raw::inventory::weight_dirty(inventory.ptr()).write(1);
        }
    }
}

pub(in crate::vostok) fn crate_availability(
    player: Object,
    inventory: &CrateInventory<'_>,
    borrowed: [Option<&InstalledPickup>; 2],
) -> Result<CrateAvailability, CrateError> {
    let mut view = OriginalLoadoutView::new(player, inventory.this, borrowed)?;
    let mut private_needs_refill = false;
    for owner in borrowed.into_iter().flatten() {
        private_needs_refill |= owner
            .weapon()
            .needs_resupply(inventory)
            .map_err(CrateError::Borrowed)?;
    }
    if private_needs_refill {
        Ok(CrateAvailability::Available)
    } else {
        view.availability()
    }
}

pub(in crate::vostok) fn complete_crate(
    player: Object,
    completion: &CrateCompletion<'_>,
    mut borrowed: [Option<&mut InstalledPickup>; 2],
) -> Result<u32, CrateError> {
    let inventory = completion.0.this;
    let mut view =
        OriginalLoadoutView::new(player, inventory, borrowed.each_ref().map(|b| b.as_deref()))?;
    let mut plans = [[Refill::Unchanged; 8]; 2];
    // Validate both private supplies before any profile/storage/native write.
    for (index, owner) in borrowed.iter().enumerate() {
        if let Some(owner) = owner {
            plans[index] = owner
                .weapon()
                .refill_plan(inventory)
                .map_err(CrateError::Borrowed)?;
        }
    }
    view.refill(inventory);
    let mut added = 0;
    for (owner, plan) in borrowed.iter_mut().zip(plans) {
        if let Some(owner) = owner {
            added += owner.apply_crate_refill(plan, inventory);
        }
    }
    Ok(added)
}
