//! A freshly cooked pickup and its private ammunition. Preparation consumes
//! the resource owner once; it cannot be used as a refill setter on a live gun.

use weapon_pickup::state::Kit;
use weapon_pickup::{AmmoKind, Chamber, DroppedAmmo, Handedness, WeaponSlot};

use super::{CookError, InventoryRecipe, OwnedInventory, ProfileTemplate};
use crate::raw;
use crate::vostok::hook::{Handle, Thread};
use crate::vostok::world::{Object, pointer, read};

mod installed;
pub(in crate::vostok) use installed::decode_pickup_slots;
pub use installed::{DecodedPickups, InstallError, InstalledPickup};
mod crate_refill;
pub use crate_refill::{CrateAvailability, CrateError};
pub(in crate::vostok) use crate_refill::{complete_crate, crate_availability};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    Cook(CookError),
    MissingWeapon,
    WeaponMismatch,
    AlreadyBound,
    InvalidSupply,
    InvalidTemplate,
}

/// A value read from one compatible ammunition supply. Allowance is the
/// original kit's total carried allowance (including loaded rounds), not a
/// claim on the recipient's persistent ammunition storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmunitionSupply {
    pub kind: AmmoKind,
    pub rounds: u16,
    pub allowance: u16,
}

#[derive(Clone, Copy)]
struct Supply {
    stack: Object,
    allowance: u16,
    original_slot: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResupplyError {
    NotInstalled,
    InvalidBinding,
    InvalidAmmo,
}

/// Read-only inventory view at a native crate availability/hint query.
/// Construction is confined to the native hook boundary.
pub struct CrateInventory<'t> {
    this: Object,
    _thread: &'t Thread,
}

impl<'t> Handle<'t> for CrateInventory<'t> {
    unsafe fn from_raw(thread: &'t Thread, address: u32) -> Option<Self> {
        let this = Object::at(address)?;
        // SAFETY: hook supplies a live native inventory resource.
        if unsafe { read(this.ptr().cast::<u32>()) } != raw::image::INVENTORY_VTABLE {
            return None;
        }
        Some(Self {
            this,
            _thread: thread,
        })
    }
}

/// Permission to refill at the native completed-crate-use call (2F81C1).
/// The native path has already enforced its team and held-use-time rules.
/// An availability query cannot construct this token through safe Rust.
pub struct CrateCompletion<'t>(CrateInventory<'t>);

impl<'t> Handle<'t> for CrateCompletion<'t> {
    /// # Safety
    /// Besides a live inventory, the caller must be the completed crate-use
    /// hook. An arbitrary simulation hook or availability query is not enough.
    unsafe fn from_raw(thread: &'t Thread, address: u32) -> Option<Self> {
        unsafe { CrateInventory::from_raw(thread, address) }.map(Self)
    }
}

/// Owns a newly cooked weapon and its compatible ammunition through their native
/// inventory resource. No raw pointer or independent ammo setter escapes.
/// Installing this in a player consumes it into an [`InstalledPickup`].
pub struct PreparedWeapon {
    _inventory: OwnedInventory,
    weapon: Object,
    supplies: [Option<Supply>; 8],
    source_slot: WeaponSlot,
}

/// A prepared resource paired with the complete saved slot state, including
/// the displaced original. Restoration never applies a pickup grant.
pub struct SnapshotWeapon {
    weapon: PreparedWeapon,
    state: weapon_pickup::history::SlotState,
}

impl InventoryRecipe<'_> {
    /// Create a separate original weapon, preserving captured loaded rounds
    /// and granting exactly one magazine of the selected ammunition in reserve.
    ///
    /// # Safety
    /// The dispatch and same-match context contract of `cook_from_template`
    /// applies. This does not attach the result to any player or registry.
    pub unsafe fn prepare_weapon(
        &self,
        template: &ProfileTemplate,
        kit: Kit,
        slot: WeaponSlot,
        handedness: Handedness,
        ammo: DroppedAmmo,
    ) -> Result<PreparedWeapon, PrepareError> {
        let selected = template.weapon_profile(kit, slot)?;
        // SAFETY: caller establishes the native resource dispatch contract.
        // The private copy stays alive throughout cooking and preparation;
        // native queries cannot create the donor's unrelated equipment.
        let inventory = unsafe { self.cook_from_template(&selected.profile, selected.kit) }
            .map_err(PrepareError::Cook)?;
        inventory.prepare(
            selected.profile.this,
            selected.kit,
            selected.slot,
            handedness,
            ammo,
        )
    }
}

impl OwnedInventory {
    // Only a fresh result can enter. Consuming self makes preparation a
    // one-time transition; failure releases the resource without partial edits.
    fn prepare(
        self,
        profile: Object,
        kit: Kit,
        slot: WeaponSlot,
        handedness: Handedness,
        ammo: DroppedAmmo,
    ) -> Result<PreparedWeapon, PrepareError> {
        // SAFETY: the synchronous no-reuse query owns these fully constructed
        // objects; none has been exposed to a player. ProfileTemplate owns the
        // valid source body. No calls or callbacks occur during preparation.
        unsafe {
            let kit_count = read(raw::player_profile::kit_count(profile.ptr()));
            if kit_count > 3 || kit.get() >= kit_count {
                return Err(PrepareError::InvalidTemplate);
            }
            let item_count = read(raw::player_profile::item_count(profile.ptr()));
            if item_count == 0 || item_count > 69 {
                return Err(PrepareError::InvalidTemplate);
            }
            let mut kit_base = profile.ptr();
            for _ in 0..kit.get() {
                kit_base = raw::profile_kit::end(kit_base).cast();
            }
            let descriptions = raw::player_profile::kit_slots(kit_base);
            let weapon_description = read(descriptions.add(slot.native() as usize));
            if (weapon_description as u16) >= u16::from(item_count) {
                return Err(PrepareError::InvalidTemplate);
            }
            let item = pointer(raw::inventory::slots(self.this.ptr()).add(slot.native() as usize))
                .ok_or(PrepareError::MissingWeapon)?;
            let vtable = read(raw::inventory_item::vtable(item.ptr()));
            if vtable != raw::image::WEAPON_ITEM_VTABLE
                && vtable != raw::image::WEAPON_CORE_ITEM_VTABLE
            {
                return Err(PrepareError::WeaponMismatch);
            }
            let weapon = Object::at(raw::weapon_item::active_object(item.ptr()) as u32)
                .ok_or(PrepareError::WeaponMismatch)?;
            if read(raw::inventory_item::owner(item.ptr())) != self.this.id()
                || read(raw::inventory_item::slot(item.ptr())) != slot.native()
                || read(raw::weapon_core::magazine_capacity(weapon.ptr())) != ammo.capacity()
                || read(raw::weapon_core::double_handed(weapon.ptr()))
                    != u8::from(handedness == Handedness::Two)
                || read(raw::weapon_core::has_chamber(weapon.ptr()))
                    != u8::from(ammo.chamber() != Chamber::None)
            {
                return Err(PrepareError::WeaponMismatch);
            }
            if read(raw::weapon_core::user(weapon.ptr())) != 0
                || read(raw::weapon_core::ammo(weapon.ptr())) != 0
            {
                return Err(PrepareError::AlreadyBound);
            }
            let mut all_ammo = [None; 8];
            for (index, slot) in super::weapon_profile::AMMO_SLOTS.into_iter().enumerate() {
                let Some(stack) =
                    pointer(raw::inventory::slots(self.this.ptr()).add(slot as usize))
                else {
                    continue;
                };
                if read(raw::inventory_item::vtable(stack.ptr())) != raw::image::AMMO_ITEM_VTABLE
                    || read(raw::inventory_item::owner(stack.ptr())) != self.this.id()
                    || read(raw::inventory_item::slot(stack.ptr())) != slot
                {
                    return Err(PrepareError::InvalidSupply);
                }
                let entry =
                    pointer(raw::item::entry(stack.ptr())).ok_or(PrepareError::InvalidSupply)?;
                if read(raw::item_entry::stack(entry.ptr())) != 1 {
                    return Err(PrepareError::InvalidSupply);
                }
                all_ammo[index] = Some(stack);
            }
            let count = read(raw::weapon_core::ammo_slot_count(weapon.ptr()));
            if count == 0 || count > 8 || ammo.kind().get() >= count {
                return Err(PrepareError::InvalidSupply);
            }
            let slots = pointer(raw::weapon_core::ammo_slots(weapon.ptr()))
                .ok_or(PrepareError::InvalidSupply)?;
            let mut supplies: [Option<Supply>; 8] = [None; 8];
            for index in 0..usize::from(count) {
                let slot = read(slots.ptr().cast::<u32>().add(index));
                let native_index = super::weapon_profile::AMMO_SLOTS
                    .iter()
                    .position(|s| *s == slot)
                    .ok_or(PrepareError::InvalidSupply)?;
                let stack = all_ammo[native_index].ok_or(PrepareError::InvalidSupply)?;
                if supplies[..index].iter().flatten().any(|s| s.stack == stack) {
                    return Err(PrepareError::InvalidSupply);
                }
                let description = read(descriptions.add(slot as usize));
                let allowance = (description >> 16) as u16;
                if (description as u16) >= u16::from(item_count) || allowance == 0 {
                    return Err(PrepareError::InvalidTemplate);
                }
                supplies[index] = Some(Supply {
                    stack,
                    allowance,
                    original_slot: slot,
                });
            }

            // All validation precedes all writes. Empty the entire private
            // inventory, including ammo for the donor's other weapon. The
            // only reserve granted is one magazine of the captured type.
            for stack in all_ammo.into_iter().flatten() {
                raw::item::count(stack.ptr()).write(0);
            }
            let selected = supplies[usize::from(ammo.kind().get())].unwrap();
            raw::item::count(selected.stack.ptr()).write(ammo.pickup_reserve());
            raw::weapon_core::ammo_kind(weapon.ptr()).write(ammo.kind().get());
            raw::weapon_core::magazine(weapon.ptr()).write(ammo.magazine());
            raw::weapon_core::chambered(weapon.ptr())
                .write(u8::from(ammo.chamber() == Chamber::Loaded));
            raw::weapon_core::load_on_initialize(weapon.ptr()).write(0);
            raw::inventory::weight_dirty(self.this.ptr()).write(1);
            Ok(PreparedWeapon {
                _inventory: self,
                weapon,
                supplies,
                source_slot: slot,
            })
        }
    }
}

impl PreparedWeapon {
    pub fn for_snapshot(
        self,
        state: weapon_pickup::history::SlotState,
    ) -> Result<SnapshotWeapon, (InstallError, Self)> {
        if state.reserves.counts().len() != self.supplies.iter().flatten().count() {
            return Err((InstallError::InvalidSupply, self));
        }
        Ok(SnapshotWeapon {
            weapon: self,
            state,
        })
    }

    /// Resolve a held weapon through its resource owner, never by following
    /// an arbitrary external slot from native memory. A stale loan cannot
    /// supply ammunition for a different weapon or recipient inventory.
    pub(in crate::vostok) fn capture_for(
        &self,
        weapon: Object,
        inventory: Object,
    ) -> Option<DroppedAmmo> {
        if weapon != self.weapon {
            return None;
        }
        self.refill_plan(inventory).ok()?;
        self.ammunition()
    }

    /// Native crate availability must consider private borrowed supplies as
    /// well as the original profile. No mutation occurs for a hint/query.
    pub fn needs_resupply(&self, inventory: &CrateInventory<'_>) -> Result<bool, ResupplyError> {
        Ok(self
            .refill_plan(inventory.this)?
            .iter()
            .any(|change| matches!(change, Refill::Add { .. })))
    }

    /// Refill borrowed ammunition for free after native crate use completes.
    /// Loaded magazine/chamber rounds count against the selected allowance.
    /// This never loads the magazine or accesses persistent profile storage.
    /// Returns the total reserve rounds added; repeated use when full adds zero.
    pub fn resupply(&mut self, completion: &CrateCompletion<'_>) -> Result<u32, ResupplyError> {
        let plan = self.refill_plan(completion.0.this)?;
        Ok(self.apply_refill(plan, completion.0.this))
    }

    fn apply_refill(&mut self, plan: [Refill; 8], inventory: Object) -> u32 {
        let mut added = 0;
        // All bindings and ammo counts were read before any write. The hook
        // runs on the simulation thread, with no dispatch between these steps.
        for change in plan {
            if let Refill::Add {
                stack,
                reserve,
                rounds,
            } = change
            {
                unsafe { raw::item::count(stack.ptr()).write(reserve) };
                added += u32::from(rounds);
            }
        }
        if added != 0 {
            // Both inventories own references into this resource set; either
            // cached weight must be recomputed after changing the private ammo.
            unsafe {
                raw::inventory::weight_dirty(inventory.ptr()).write(1);
                raw::inventory::weight_dirty(self._inventory.this.ptr()).write(1);
            }
        }
        added
    }

    fn refill_plan(&self, inventory: Object) -> Result<[Refill; 8], ResupplyError> {
        // SAFETY: this resource owns the weapon and each original ammo cell;
        // the crate hook supplies the live recipient inventory. Address-relative
        // indices are checked against the owned cells before following them.
        unsafe {
            if inventory == self._inventory.this
                || read(raw::weapon_core::inventory(self.weapon.ptr())) != inventory.id()
            {
                return Err(ResupplyError::NotInstalled);
            }
            let item = raw::weapon_core::item(self.weapon.ptr()).cast();
            let slot = WeaponSlot::from_native(read(raw::inventory_item::slot(item)))
                .ok_or(ResupplyError::InvalidBinding)?;
            if read(raw::inventory::slots(inventory.ptr()).add(slot.native() as usize))
                != item as u32
            {
                return Err(ResupplyError::InvalidBinding);
            }
            let ammo = self.ammunition().ok_or(ResupplyError::InvalidAmmo)?;
            let count = read(raw::weapon_core::ammo_slot_count(self.weapon.ptr()));
            if usize::from(count) != self.supplies.iter().flatten().count()
                || ammo.kind().get() >= count
            {
                return Err(ResupplyError::InvalidBinding);
            }
            let slots = pointer(raw::weapon_core::ammo_slots(self.weapon.ptr()))
                .ok_or(ResupplyError::InvalidBinding)?;
            let mut plan = [Refill::Unchanged; 8];
            for (index, supply) in self.supplies.iter().enumerate() {
                let Some(supply) = supply else { continue };
                let cell = raw::inventory::slots(self._inventory.this.ptr())
                    .add(supply.original_slot as usize);
                let base = raw::inventory::slots(inventory.ptr()) as u32;
                // This is x86 address arithmetic, not a Rust pointer index:
                // private cells can be before or after the recipient inventory.
                let encoded = (((cell as u32).wrapping_sub(base) as i32) >> 2) as u32;
                if read(cell) != supply.stack.id()
                    || read(slots.ptr().cast::<u32>().add(index)) != encoded
                    || read(raw::inventory_item::owner(supply.stack.ptr())) != inventory.id()
                    || read(raw::inventory_item::slot(supply.stack.ptr())) != encoded
                {
                    return Err(ResupplyError::InvalidBinding);
                }
                let loaded = if index == usize::from(ammo.kind().get()) {
                    if read(raw::weapon_core::ammo(self.weapon.ptr())) != supply.stack.id() {
                        return Err(ResupplyError::InvalidBinding);
                    }
                    ammo.loaded_rounds()
                } else {
                    0
                };
                let reserve = read(raw::item::count(supply.stack.ptr()));
                // Never remove rounds if the original allowance is smaller
                // than the pickup grant or the player already carries extra.
                let target = reserve.max(supply.allowance.saturating_sub(loaded));
                if target > reserve {
                    plan[index] = Refill::Add {
                        stack: supply.stack,
                        reserve: target,
                        rounds: target - reserve,
                    };
                }
            }
            Ok(plan)
        }
    }

    /// Reads current loaded rounds, including after installation or death.
    /// Preparation disables native autoload so the initial grant is preserved.
    pub fn ammunition(&self) -> Option<DroppedAmmo> {
        // SAFETY: the inventory owner keeps the weapon alive; fields
        // are read together without any native dispatch or competing mutation.
        unsafe {
            let chamber = match (
                read(raw::weapon_core::has_chamber(self.weapon.ptr())),
                read(raw::weapon_core::chambered(self.weapon.ptr())),
            ) {
                (0, 0) => Chamber::None,
                (1, 0) => Chamber::Empty,
                (1, 1) => Chamber::Loaded,
                _ => return None,
            };
            DroppedAmmo::parse(
                read(raw::weapon_core::magazine_capacity(self.weapon.ptr())),
                read(raw::weapon_core::magazine(self.weapon.ptr())),
                chamber,
                AmmoKind::parse(read(raw::weapon_core::ammo_kind(self.weapon.ptr())))?,
            )
        }
    }

    pub fn supply(&self, kind: AmmoKind) -> Option<AmmunitionSupply> {
        let supply = self.supplies[usize::from(kind.get())]?;
        Some(AmmunitionSupply {
            kind,
            // SAFETY: the same resource owner keeps each ammo stack alive.
            rounds: unsafe { read(raw::item::count(supply.stack.ptr())) },
            allowance: supply.allowance,
        })
    }
}

#[derive(Clone, Copy)]
enum Refill {
    Unchanged,
    Add {
        stack: Object,
        reserve: u16,
        rounds: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;

    pub(super) struct Fixture {
        pub(super) inventory: Box<[u32; 110]>,
        pub(super) weapon: Box<[u32; 512]>,
        pub(super) ammo: [Box<[u32; 180]>; 2],
        entries: [Box<[u32; 180]>; 2],
        pub(super) slots: Box<[u32; 2]>,
        pub(super) profile: Box<[u32; 0x4634 / 4]>,
    }

    impl Fixture {
        pub(super) fn new() -> Self {
            let mut f = Self {
                inventory: Box::new([0; 110]),
                weapon: Box::new([0; 512]),
                ammo: [Box::new([0; 180]), Box::new([0; 180])],
                entries: [Box::new([0; 180]), Box::new([0; 180])],
                slots: Box::new([8, 19]),
                profile: Box::new([0; 0x4634 / 4]),
            };
            // SAFETY: aligned test allocations cover every native field. The
            // resource release stub checks the final ref, but does not free
            // these boxes; they remain readable to check failure atomicity.
            unsafe {
                let inv = f.inventory.as_mut_ptr().cast();
                f.inventory[0] = raw::image::INVENTORY_VTABLE;
                raw::unmanaged_resource::references(inv).write(1);
                let gun = f.weapon.as_mut_ptr().cast::<core::ffi::c_void>();
                let item = gun.byte_add(0x10);
                raw::weapon_core::vtable(gun).write(raw::image::WEAPON_ACTIVE_VTABLE);
                raw::weapon_core::active_id(gun).write(u8::MAX);
                raw::unmanaged_resource::references(item).write(1);
                raw::inventory_item::vtable(item).write(raw::image::WEAPON_ITEM_VTABLE);
                raw::inventory_item::owner(item).write(inv as u32);
                raw::inventory_item::slot(item).write(10);
                raw::inventory::slots(inv).add(10).write(item as u32);
                raw::weapon_core::double_handed(gun).write(1);
                raw::weapon_core::magazine_capacity(gun).write(30);
                raw::weapon_core::magazine(gun).write(23);
                raw::weapon_core::has_chamber(gun).write(1);
                raw::weapon_core::chambered(gun).write(1);
                raw::weapon_core::load_on_initialize(gun).write(1);
                raw::weapon_core::ammo_slots(gun).write(f.slots.as_mut_ptr() as u32);
                raw::weapon_core::ammo_slot_count(gun).write(2);
                let profile = f.profile.as_mut_ptr().cast();
                raw::player_profile::kit_count(profile).write(2);
                raw::player_profile::item_count(profile).write(3);
                let kit = raw::profile_kit::end(profile).cast();
                let descriptions = raw::player_profile::kit_slots(kit);
                descriptions.add(10).write(1 << 16);
                for (index, slot) in [8, 19].into_iter().enumerate() {
                    let stack = f.ammo[index].as_mut_ptr().cast();
                    raw::unmanaged_resource::references(stack).write(1);
                    raw::inventory_item::vtable(stack).write(raw::image::AMMO_ITEM_VTABLE);
                    raw::inventory_item::owner(stack).write(inv as u32);
                    raw::inventory_item::slot(stack).write(slot as u32);
                    raw::inventory::slots(inv).add(slot).write(stack as u32);
                    let entry = f.entries[index].as_mut_ptr().cast();
                    raw::item::entry(stack).write(entry as u32);
                    raw::item_entry::stack(entry).write(1);
                    raw::item::count(stack).write(90 + index as u16);
                    descriptions
                        .add(slot)
                        .write(((120 + index as u32 * 30) << 16) | (index as u32 + 1));
                }
            }
            f
        }

        // The borrow keeps fixture allocation alive through the operation;
        // callers drop the returned owner before dropping these boxes.
        pub(super) fn prepare(
            &mut self,
            ammo: DroppedAmmo,
        ) -> Result<PreparedWeapon, PrepareError> {
            let inventory = OwnedInventory {
                this: Object::at(self.inventory.as_mut_ptr() as u32).unwrap(),
            };
            inventory.prepare(
                Object::at(self.profile.as_mut_ptr() as u32).unwrap(),
                Kit::parse(1).unwrap(),
                WeaponSlot::Second,
                Handedness::Two,
                ammo,
            )
        }

        fn attached_inventory(&mut self, pickup: &PreparedWeapon) -> Box<[u32; 110]> {
            let mut target = Box::new([0u32; 110]);
            // A fixture of the completed attachment's fields, not a native
            // transfer test. The refill code must validate all these bindings.
            unsafe {
                let inventory = target.as_mut_ptr().cast();
                target[0] = raw::image::INVENTORY_VTABLE;
                raw::inventory::slots(inventory)
                    .add(10)
                    .write(raw::weapon_core::item(pickup.weapon.ptr()) as u32);
                raw::weapon_core::inventory(pickup.weapon.ptr()).write(inventory as u32);
                // Only checked for absence, never followed by the refill path.
                raw::weapon_core::user(pickup.weapon.ptr()).write(1);
                for (index, supply) in pickup.supplies.iter().enumerate() {
                    let Some(supply) = supply else { continue };
                    let cell = raw::inventory::slots(pickup._inventory.this.ptr())
                        .add(supply.original_slot as usize);
                    let encoded = (((cell as u32)
                        .wrapping_sub(raw::inventory::slots(inventory) as u32)
                        as i32)
                        >> 2) as u32;
                    self.slots[index] = encoded;
                    raw::inventory_item::owner(supply.stack.ptr()).write(inventory as u32);
                    raw::inventory_item::slot(supply.stack.ptr()).write(encoded);
                }
                raw::weapon_core::ammo(pickup.weapon.ptr()).write(self.ammo[1].as_mut_ptr() as u32);
                raw::inventory::weight_dirty(pickup._inventory.this.ptr()).write(0);
            }
            target
        }

        fn assert_unchanged(&self) {
            unsafe {
                let gun = self.weapon.as_ptr().cast_mut().cast();
                assert_eq!(read(raw::weapon_core::magazine(gun)), 23);
                assert_eq!(read(raw::weapon_core::chambered(gun)), 1);
                assert_eq!(read(raw::weapon_core::ammo_kind(gun)), 0);
                assert_eq!(read(raw::weapon_core::load_on_initialize(gun)), 1);
                for (index, stack) in self.ammo.iter().enumerate() {
                    assert_eq!(
                        read(raw::item::count(stack.as_ptr().cast_mut().cast())),
                        90 + index as u16
                    );
                }
                let inv = self.inventory.as_ptr().cast_mut().cast();
                assert_eq!(read(raw::inventory::weight_dirty(inv)), 0);
                assert_eq!(read(raw::unmanaged_resource::references(inv)), 0);
            }
        }
    }

    fn captured(magazine: u8, chamber: Chamber) -> DroppedAmmo {
        DroppedAmmo::parse(30, magazine, chamber, AmmoKind::parse(1).unwrap()).unwrap()
    }

    #[test]
    fn preserves_loaded_ammo_and_grants_only_one_selected_spare_magazine() {
        for magazine in [0, 7, 30] {
            for chamber in [Chamber::None, Chamber::Empty, Chamber::Loaded] {
                let mut f = Fixture::new();
                unsafe {
                    raw::weapon_core::has_chamber(f.weapon.as_mut_ptr().cast())
                        .write(u8::from(chamber != Chamber::None));
                }
                let ammo = captured(magazine, chamber);
                let prepared = f.prepare(ammo).unwrap();
                assert_eq!(prepared.ammunition(), Some(ammo));
                assert_eq!(
                    prepared.supply(AmmoKind::parse(0).unwrap()).unwrap().rounds,
                    0
                );
                let supply = prepared.supply(ammo.kind()).unwrap();
                assert_eq!(supply.rounds, 30);
                assert_eq!(supply.allowance, 150);
                assert_eq!(prepared.supply(AmmoKind::parse(2).unwrap()), None);
                unsafe {
                    assert_eq!(
                        read(raw::weapon_core::load_on_initialize(prepared.weapon.ptr())),
                        0
                    );
                    assert_eq!(read(raw::weapon_core::ammo(prepared.weapon.ptr())), 0);
                    let inv = f.inventory.as_mut_ptr().cast();
                    assert_eq!(read(raw::inventory::weight_dirty(inv)), 1);
                    assert_eq!(read(raw::unmanaged_resource::references(inv)), 1);
                }
                drop(prepared);
                assert_eq!(
                    unsafe {
                        read(raw::unmanaged_resource::references(
                            f.inventory.as_mut_ptr().cast(),
                        ))
                    },
                    0
                );
            }
        }
    }

    #[test]
    fn donor_other_calibre_is_emptied_and_invalid_extra_supply_is_atomic() {
        for valid in [true, false] {
            let mut f = Fixture::new();
            let mut other = Box::new([0u32; 180]);
            let mut entry = Box::new([0u32; 180]);
            unsafe {
                let stack = other.as_mut_ptr().cast();
                let inventory = f.inventory.as_mut_ptr().cast();
                raw::inventory::slots(inventory).add(9).write(stack as u32);
                raw::inventory_item::vtable(stack).write(raw::image::AMMO_ITEM_VTABLE);
                raw::inventory_item::owner(stack).write(if valid { inventory as u32 } else { 0 });
                raw::inventory_item::slot(stack).write(9);
                raw::item::entry(stack).write(entry.as_mut_ptr() as u32);
                raw::item_entry::stack(entry.as_mut_ptr().cast()).write(1);
                raw::item::count(stack).write(200);
            }
            let result = f.prepare(captured(7, Chamber::Loaded));
            if valid {
                let prepared = result.unwrap();
                assert_eq!(prepared.ammunition(), Some(captured(7, Chamber::Loaded)));
                assert_eq!(
                    prepared.supply(AmmoKind::parse(1).unwrap()).unwrap().rounds,
                    30
                );
                assert_eq!(
                    unsafe { read(raw::item::count(other.as_mut_ptr().cast())) },
                    0
                );
                drop(prepared);
            } else {
                assert!(matches!(result, Err(PrepareError::InvalidSupply)));
                f.assert_unchanged();
                assert_eq!(
                    unsafe { read(raw::item::count(other.as_mut_ptr().cast())) },
                    200
                );
            }
        }
    }

    #[test]
    fn late_supply_failure_does_not_grant_ammo_or_change_weapon() {
        for invalid in 0..5 {
            let mut f = Fixture::new();
            unsafe {
                let second = f.ammo[1].as_mut_ptr().cast();
                match invalid {
                    0 => f.slots[1] = 8, // duplicate/aliased supply
                    1 => raw::inventory_item::vtable(second).write(0),
                    2 => raw::inventory_item::owner(second).write(0),
                    3 => raw::item_entry::stack(f.entries[1].as_mut_ptr().cast()).write(0),
                    _ => {
                        let kit = raw::profile_kit::end(f.profile.as_mut_ptr().cast()).cast();
                        raw::player_profile::kit_slots(kit).add(19).write(u32::MAX);
                    }
                }
            }
            assert!(f.prepare(captured(7, Chamber::Loaded)).is_err());
            f.assert_unchanged();
        }
    }

    #[test]
    fn rejects_bound_weapon_and_incompatible_capture_without_writes() {
        for invalid in 0..5 {
            let mut f = Fixture::new();
            unsafe {
                let gun = f.weapon.as_mut_ptr().cast();
                match invalid {
                    0 => raw::weapon_core::user(gun).write(1),
                    1 => raw::weapon_core::ammo(gun).write(f.ammo[0].as_mut_ptr() as u32),
                    2 => raw::weapon_core::magazine_capacity(gun).write(20),
                    3 => raw::weapon_core::double_handed(gun).write(0),
                    _ => raw::weapon_core::has_chamber(gun).write(0),
                }
            }
            let result = f.prepare(captured(7, Chamber::Loaded));
            let expected = if invalid < 2 {
                PrepareError::AlreadyBound
            } else {
                PrepareError::WeaponMismatch
            };
            assert!(matches!(result, Err(error) if error == expected));
            f.assert_unchanged();
        }
    }

    #[test]
    fn free_crate_refill_counts_loaded_rounds_and_never_touches_original_supplies() {
        for chamber in [Chamber::None, Chamber::Empty, Chamber::Loaded] {
            let mut f = Fixture::new();
            unsafe {
                raw::weapon_core::has_chamber(f.weapon.as_mut_ptr().cast())
                    .write(u8::from(chamber != Chamber::None));
            }
            let ammo = captured(7, chamber);
            let mut pickup = f.prepare(ammo).unwrap();
            let mut target = f.attached_inventory(&pickup);
            let mut original_ammo = Box::new([0u32; 180]);
            let profile_before = f.profile.clone();
            unsafe {
                raw::inventory::slots(target.as_mut_ptr().cast())
                    .add(8)
                    .write(original_ammo.as_mut_ptr() as u32);
                raw::item::count(original_ammo.as_mut_ptr().cast()).write(73);
            }
            let original_before = original_ammo.clone();
            let thread = unsafe { Thread::new() };
            let view =
                unsafe { CrateInventory::from_raw(&thread, target.as_mut_ptr() as u32) }.unwrap();
            let completion =
                unsafe { CrateCompletion::from_raw(&thread, target.as_mut_ptr() as u32) }.unwrap();
            assert_eq!(pickup.needs_resupply(&view), Ok(true));
            assert_eq!(pickup.supply(ammo.kind()).unwrap().rounds, 30);
            assert_eq!(
                pickup.resupply(&completion),
                Ok(120 + 150 - u32::from(ammo.loaded_rounds()) - 30)
            );
            assert_eq!(
                pickup.supply(AmmoKind::parse(0).unwrap()).unwrap().rounds,
                120
            );
            assert_eq!(
                pickup.supply(ammo.kind()).unwrap().rounds,
                150 - ammo.loaded_rounds()
            );
            assert_eq!(pickup.ammunition(), Some(ammo));
            assert_eq!(pickup.needs_resupply(&view), Ok(false));
            assert_eq!(pickup.resupply(&completion), Ok(0));
            assert_eq!(original_ammo, original_before);
            assert_eq!(f.profile, profile_before);
            unsafe {
                assert_eq!(
                    read(raw::inventory::weight_dirty(target.as_mut_ptr().cast())),
                    1
                );
                assert_eq!(
                    read(raw::inventory::weight_dirty(pickup._inventory.this.ptr())),
                    1
                );
            }
            drop(pickup);
        }
    }

    #[test]
    fn crate_refill_rejects_uninstalled_or_stale_external_bindings_before_any_write() {
        for invalid in 0..5 {
            let mut f = Fixture::new();
            let ammo = captured(7, Chamber::Loaded);
            let mut pickup = f.prepare(ammo).unwrap();
            let mut target = f.attached_inventory(&pickup);
            unsafe {
                match invalid {
                    0 => raw::weapon_core::inventory(pickup.weapon.ptr())
                        .write(pickup._inventory.this.id()),
                    1 => raw::inventory::slots(target.as_mut_ptr().cast())
                        .add(10)
                        .write(0),
                    2 => f.slots[1] = 19, // wrong cell, checked before following it
                    3 => raw::inventory_item::owner(f.ammo[1].as_mut_ptr().cast()).write(0),
                    _ => raw::weapon_core::ammo(pickup.weapon.ptr())
                        .write(f.ammo[0].as_mut_ptr() as u32),
                }
            }
            let thread = unsafe { Thread::new() };
            let completion =
                unsafe { CrateCompletion::from_raw(&thread, target.as_mut_ptr() as u32) }.unwrap();
            assert!(pickup.resupply(&completion).is_err());
            assert_eq!(
                pickup.supply(AmmoKind::parse(0).unwrap()).unwrap().rounds,
                0
            );
            assert_eq!(pickup.supply(ammo.kind()).unwrap().rounds, 30);
            assert_eq!(pickup.ammunition(), Some(ammo));
            unsafe {
                assert_eq!(
                    read(raw::inventory::weight_dirty(target.as_mut_ptr().cast())),
                    0
                );
                assert_eq!(
                    read(raw::inventory::weight_dirty(pickup._inventory.this.ptr())),
                    0
                );
            }
            drop(pickup);
        }
    }

    #[test]
    fn refill_never_removes_extra_rounds_and_can_fill_the_full_native_count() {
        let mut f = Fixture::new();
        let ammo = captured(30, Chamber::Loaded);
        let mut pickup = f.prepare(ammo).unwrap();
        pickup.supplies[0].as_mut().unwrap().allowance = 0;
        pickup.supplies[1].as_mut().unwrap().allowance = 10;
        let mut target = f.attached_inventory(&pickup);
        let thread = unsafe { Thread::new() };
        let completion =
            unsafe { CrateCompletion::from_raw(&thread, target.as_mut_ptr() as u32) }.unwrap();
        // Switching away from a gun clears its user; stowed borrowed weapons
        // still refill because the inventory, not current selection, owns them.
        unsafe { raw::weapon_core::user(pickup.weapon.ptr()).write(0) };
        assert_eq!(pickup.resupply(&completion), Ok(0));
        assert_eq!(pickup.supply(ammo.kind()).unwrap().rounds, 30);
        pickup.supplies[1].as_mut().unwrap().allowance = u16::MAX;
        assert_eq!(
            pickup.resupply(&completion),
            Ok(u32::from(u16::MAX) - 31 - 30)
        );
        assert_eq!(pickup.supply(ammo.kind()).unwrap().rounds, u16::MAX - 31);
        assert_eq!(pickup.resupply(&completion), Ok(0));
        assert_eq!(pickup.ammunition(), Some(ammo));
        drop(pickup);
    }

    #[test]
    fn captures_borrowed_weapon_at_death_through_its_owner_after_ammo_switch() {
        use crate::vostok::weapon::PickupInventory;

        let mut f = Fixture::new();
        let initial = captured(7, Chamber::Loaded);
        let pickup = f.prepare(initial).unwrap();
        let mut target = f.attached_inventory(&pickup);
        let mut player_memory = Box::new([0u32; 500]);
        unsafe {
            let p = player_memory.as_mut_ptr().cast();
            raw::player::inventory(p).write(target.as_mut_ptr() as u32);
            raw::player::profile(p).write(f.profile.as_mut_ptr() as u32);
            raw::player::kit(p).write(1);
            raw::player::held_item(p).write(pickup.weapon.id());
            raw::player::alive(p).write(1);
            raw::weapon_core::user(pickup.weapon.ptr()).write(p as u32);
        }
        let player = Object::at(player_memory.as_mut_ptr() as u32).unwrap();
        // The original reader must not try to follow a wrapped external slot.
        assert!(PickupInventory::read(player, [None, None]).is_none());
        let held = PickupInventory::read(player, [None, Some(&pickup)])
            .unwrap()
            .held
            .unwrap();
        assert_eq!(held.slot, WeaponSlot::Second);
        assert_eq!(held.ammo, initial);

        unsafe {
            raw::weapon_core::ammo_kind(pickup.weapon.ptr()).write(0);
            raw::weapon_core::ammo(pickup.weapon.ptr()).write(f.ammo[0].as_mut_ptr() as u32);
            raw::weapon_core::magazine(pickup.weapon.ptr()).write(3);
            raw::weapon_core::chambered(pickup.weapon.ptr()).write(0);
        }
        let current =
            DroppedAmmo::parse(30, 3, Chamber::Empty, AmmoKind::parse(0).unwrap()).unwrap();
        let held = PickupInventory::read(player, [None, Some(&pickup)])
            .unwrap()
            .held
            .unwrap();
        assert_eq!(held.ammo, current);

        let mut different = Fixture::new();
        let stale = different.prepare(initial).unwrap();
        assert!(PickupInventory::read(player, [None, Some(&stale)]).is_none());
        unsafe { raw::weapon_core::user(pickup.weapon.ptr()).write(0) };
        assert!(PickupInventory::read(player, [None, Some(&pickup)]).is_none());
        // Native death deactivates the held weapon, clearing its user. Its
        // inventory/ammo ownership is still valid for capturing a new drop.
        unsafe { raw::player::alive(player.ptr()).write(0) };
        assert_eq!(
            PickupInventory::read(player, [None, Some(&pickup)])
                .unwrap()
                .held
                .unwrap()
                .ammo,
            current
        );
        drop(stale);
        drop(pickup);
    }
}
