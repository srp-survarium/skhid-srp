//! Native weapon facts for pickup. Every pointer is resolved inside a player
//! hook and converted to values before returning; no item address escapes.
//! Inventory pointers use the item subobject, held pointers the active object.

use weapon_pickup::state::HeldAtDeath;
use weapon_pickup::{AmmoKind, CarryRule, Chamber, DroppedAmmo, Handedness, Loadout, WeaponSlot};

use super::resources::PreparedWeapon;
use super::world::{Object, pointer, read};
use crate::raw;

/// One coherent native read for an input/death hook. This does not own or
/// mutate native ammunition, and cannot refill an item by being queried.
pub struct PickupInventory {
    pub loadout: Loadout,
    pub held: Option<HeldAtDeath>,
}

impl PickupInventory {
    pub(in crate::vostok) fn read(
        player: Object,
        borrowed: [Option<&PreparedWeapon>; 2],
    ) -> Option<Self> {
        // SAFETY: Player supplied its live object for the current hook. The
        // inventory/profile are owned by that player for this hook's duration.
        unsafe {
            let inventory = pointer(raw::player::inventory(player.ptr()))?;
            let profile = pointer(raw::player::profile(player.ptr()))?;
            let kit = read(raw::player::kit(player.ptr()));
            if kit >= 3 {
                return None;
            }
            let held = pointer(raw::player::held_item(player.ptr()));
            let mut classes = [None; 2];
            let mut captured = None;
            for (index, slot) in [WeaponSlot::First, WeaponSlot::Second]
                .into_iter()
                .enumerate()
            {
                let entry = raw::inventory::slots(inventory.ptr()).add(slot.native() as usize);
                let Some(item) = pointer(entry) else { continue };
                // Only these concrete types have the fields accessed below.
                let vtable = read(raw::inventory_item::vtable(item.ptr()));
                if vtable != raw::image::WEAPON_ITEM_VTABLE
                    && vtable != raw::image::WEAPON_CORE_ITEM_VTABLE
                {
                    return None;
                }
                // This generated accessor is the enclosing interface's
                // address, not a pointer stored in the item.
                let weapon = Object::at(raw::weapon_item::active_object(item.ptr()) as u32)?;
                if read(raw::weapon_core::inventory(weapon.ptr())) != inventory.id() {
                    return None;
                }
                let handedness = match read(raw::weapon_core::double_handed(weapon.ptr())) {
                    0 => Handedness::One,
                    1 => Handedness::Two,
                    _ => return None,
                };
                classes[index] = Some(handedness);
                if held == Some(weapon) {
                    let user = read(raw::weapon_core::user(weapon.ptr()));
                    if user != player.id()
                        && (user != 0 || read(raw::player::alive(player.ptr())) != 0)
                    {
                        return None;
                    }
                    let ammo = match borrowed[index] {
                        Some(owner) => owner.capture_for(weapon, inventory)?,
                        None => capture_ammo(weapon, inventory)?,
                    };
                    captured = Some(HeldAtDeath {
                        slot,
                        handedness,
                        ammo,
                    });
                }
            }
            let mut kit_base = profile.ptr();
            for _ in 0..kit {
                kit_base = raw::profile_kit::end(kit_base).cast();
            }
            let modifiers = raw::player_profile::kit_modifiers(kit_base).cast();
            let disabled = raw::engine_apply_modifier(modifiers, 64, 0.0, 1.0);
            let double = raw::engine_apply_modifier(modifiers, 25, 0.0, 1.0);
            if !disabled.is_finite() || !double.is_finite() {
                return None;
            }
            let rule = if disabled > 0.0 {
                CarryRule::OneWeapon
            } else if double > 0.0 {
                CarryRule::TwoDoubleHanded
            } else {
                CarryRule::OneDoubleHanded
            };
            Some(Self {
                loadout: Loadout::new(classes, captured.map(|item| item.slot), rule),
                held: captured,
            })
        }
    }
}

/// # Safety
/// `weapon` is a concrete weapon active-object base in `inventory`, both
/// live on the current simulation hook. All pointers read are native-owned.
unsafe fn capture_ammo(weapon: Object, inventory: Object) -> Option<DroppedAmmo> {
    unsafe {
        let count = read(raw::weapon_core::ammo_slot_count(weapon.ptr()));
        let ordinal = read(raw::weapon_core::ammo_kind(weapon.ptr()));
        if count == 0 || count > 8 || ordinal >= count {
            return None;
        }
        let slots = pointer(raw::weapon_core::ammo_slots(weapon.ptr()))?;
        let slot = read(slots.ptr().cast::<u32>().add(ordinal as usize));
        if ![8, 9, 11, 12, 19, 20, 21, 22].contains(&slot) {
            return None;
        }
        let stack = pointer(raw::inventory::slots(inventory.ptr()).add(slot as usize))?;
        if pointer(raw::weapon_core::ammo(weapon.ptr())) != Some(stack) {
            return None;
        }
        let entry = pointer(raw::item::entry(stack.ptr()))?;
        if read(raw::item_entry::stack(entry.ptr())) != 1 {
            return None;
        }
        let chamber = match (
            read(raw::weapon_core::has_chamber(weapon.ptr())),
            read(raw::weapon_core::chambered(weapon.ptr())),
        ) {
            (0, 0) => Chamber::None,
            (1, 0) => Chamber::Empty,
            (1, 1) => Chamber::Loaded,
            _ => return None,
        };
        DroppedAmmo::parse(
            read(raw::weapon_core::magazine_capacity(weapon.ptr())),
            read(raw::weapon_core::magazine(weapon.ptr())),
            chamber,
            AmmoKind::parse(ordinal)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;
    use std::boxed::Box;

    // The adapter calls the existing native modifier evaluator. This fixture
    // supplies its results, proving modifier indices and current-kit selection;
    // it does not claim to reimplement that evaluator's native modifier chains.
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_apply_modifier(
        modifiers: *const c_void,
        index: u32,
        base: f32,
        weight: f32,
    ) -> f32 {
        assert_eq!(base, 0.0);
        assert_eq!(weight, 1.0);
        assert!(index == 25 || index == 64);
        unsafe { *modifiers.cast::<f32>().add(index as usize) }
    }

    struct Fixture {
        player: Box<[u32; 500]>,
        inventory: Box<[u32; 128]>,
        profile: Box<[u32; 1500]>,
        guns: [Box<[u32; 512]>; 2],
        ammo: Box<[u32; 180]>,
        entry: Box<[u32; 180]>,
        slots: Box<[u32; 2]>,
    }

    impl Fixture {
        fn new() -> Self {
            let mut f = Self {
                player: Box::new([0; 500]),
                inventory: Box::new([0; 128]),
                profile: Box::new([0; 1500]),
                guns: [Box::new([0; 512]), Box::new([0; 512])],
                ammo: Box::new([0; 180]),
                entry: Box::new([0; 180]),
                slots: Box::new([8, 9]),
            };
            // SAFETY: fixture allocations are aligned and cover every field.
            unsafe {
                let player = f.player.as_mut_ptr().cast();
                let inv = f.inventory.as_mut_ptr().cast();
                raw::player::inventory(player).write(inv as u32);
                raw::player::profile(player).write(f.profile.as_mut_ptr() as u32);
                raw::player::kit(player).write(1);
                raw::player::alive(player).write(1);
                let ammo = f.ammo.as_mut_ptr().cast();
                raw::item::entry(ammo).write(f.entry.as_mut_ptr() as u32);
                raw::item_entry::stack(f.entry.as_mut_ptr().cast()).write(1);
                raw::inventory::slots(inv).add(9).write(ammo as u32);
                for (index, slot) in [7, 10].into_iter().enumerate() {
                    let gun = f.guns[index].as_mut_ptr().cast::<c_void>();
                    let item = gun.byte_add(0x10);
                    raw::inventory_item::vtable(item).write(raw::image::WEAPON_ITEM_VTABLE);
                    raw::inventory::slots(inv).add(slot).write(item as u32);
                    raw::weapon_core::inventory(gun).write(inv as u32);
                    raw::weapon_core::user(gun).write(player as u32);
                    raw::weapon_core::double_handed(gun).write(u8::from(index == 1));
                    raw::weapon_core::ammo(gun).write(ammo as u32);
                    raw::weapon_core::ammo_slots(gun).write(f.slots.as_mut_ptr() as u32);
                    raw::weapon_core::ammo_slot_count(gun).write(2);
                    raw::weapon_core::ammo_kind(gun).write(1);
                    raw::weapon_core::magazine_capacity(gun).write(30);
                    raw::weapon_core::magazine(gun).write(7);
                    raw::weapon_core::has_chamber(gun).write(1);
                    raw::weapon_core::chambered(gun).write(1);
                }
                raw::player::held_item(player).write(f.guns[0].as_mut_ptr() as u32);
            }
            f
        }

        fn read(&self) -> Option<PickupInventory> {
            PickupInventory::read(
                Object::at(self.player.as_ptr() as u32).unwrap(),
                [None, None],
            )
        }

        fn modifier(&mut self, kit: usize, index: usize, value: f32) {
            // SAFETY: the fixture has three full kit blocks and modifier storage.
            unsafe {
                let mut base = self.profile.as_mut_ptr().cast();
                for _ in 0..kit {
                    base = raw::profile_kit::end(base).cast();
                }
                raw::player_profile::kit_modifiers(base)
                    .cast::<f32>()
                    .add(index)
                    .write(value);
            }
        }
    }

    #[test]
    fn reads_the_item_subobject_and_held_active_object_without_confusing_them() {
        let f = Fixture::new();
        let state = f.read().unwrap();
        let held = state.held.unwrap();
        assert_eq!(held.slot, WeaponSlot::First);
        assert_eq!(held.handedness, Handedness::One);
        assert_eq!(held.ammo.magazine(), 7);
        assert_eq!(held.ammo.chamber(), Chamber::Loaded);
        assert_eq!(held.ammo.kind().get(), 1);
        assert_eq!(
            state.loadout.replacement(Handedness::Two),
            Err(weapon_pickup::Refused::TwoDoubleHandedWeapons)
        );
    }

    #[test]
    fn evaluates_current_kit_skills_and_keeps_the_disabled_slot_restricted() {
        let mut f = Fixture::new();
        f.modifier(0, 25, 1.0);
        assert!(
            f.read()
                .unwrap()
                .loadout
                .replacement(Handedness::Two)
                .is_err()
        );
        f.modifier(1, 25, 1.0);
        assert_eq!(
            f.read().unwrap().loadout.replacement(Handedness::Two),
            Ok(WeaponSlot::First)
        );
        f.modifier(1, 64, 1.0);
        assert_eq!(
            f.read().unwrap().loadout.replacement(Handedness::Two),
            Err(weapon_pickup::Refused::SecondWeaponDisabled)
        );
    }

    #[test]
    fn unknown_types_and_stale_ammo_bindings_are_not_interpreted_as_a_weapon() {
        let mut f = Fixture::new();
        unsafe {
            raw::weapon_core::ammo_kind(f.guns[0].as_mut_ptr().cast()).write(2);
        }
        assert!(f.read().is_none());
        unsafe {
            raw::weapon_core::ammo_kind(f.guns[0].as_mut_ptr().cast()).write(0);
        }
        // Slot 8 is empty; only slot 9 holds the current ammo stack.
        assert!(f.read().is_none());
        unsafe {
            raw::weapon_core::ammo_kind(f.guns[0].as_mut_ptr().cast()).write(1);
            raw::inventory_item::vtable(f.guns[0].as_mut_ptr().cast::<c_void>().byte_add(0x10))
                .write(raw::image::MEDKIT_VTABLE);
        }
        assert!(f.read().is_none());
    }
}
