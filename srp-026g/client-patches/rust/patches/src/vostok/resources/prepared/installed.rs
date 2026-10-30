//! One native slot transaction. The owner retains the displaced weapon and
//! coordinates registry, inventory, ammo and active-object callbacks together.
//! Match lifetime and snapshot ownership must be established by the eventual
//! match bank before this unsafe installation boundary can be called.

use core::mem::ManuallyDrop;
use core::sync::atomic::{AtomicU32, Ordering};
use weapon_pickup::{PickupGrant, WeaponSlot};

use super::{CrateCompletion, PreparedWeapon, ResupplyError, SnapshotWeapon};
use crate::raw;
use crate::vostok::world::{Object, pointer, read};

mod original;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallError {
    PlayerUnavailable,
    NotHeld,
    Switching,
    WrongWeapon,
    InvalidRegistry,
    AlreadyInstalled,
    InvalidSupply,
    GrantChanged,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavor {
    Client,
    Core,
}

impl Flavor {
    unsafe fn of(weapon: Object) -> Option<Self> {
        unsafe {
            let item = raw::weapon_core::item(weapon.ptr()).cast();
            match (
                read(raw::weapon_core::vtable(weapon.ptr())),
                read(raw::inventory_item::vtable(item)),
            ) {
                (raw::image::WEAPON_ACTIVE_VTABLE, raw::image::WEAPON_ITEM_VTABLE) => {
                    Some(Self::Client)
                }
                (raw::image::WEAPON_CORE_ACTIVE_VTABLE, raw::image::WEAPON_CORE_ITEM_VTABLE) => {
                    Some(Self::Core)
                }
                _ => None,
            }
        }
    }

    unsafe fn deactivate(self, weapon: Object) {
        unsafe {
            match self {
                Self::Client => raw::engine_weapon_deactivate(weapon.ptr(), 1),
                Self::Core => raw::engine_weapon_core_deactivate(weapon.ptr(), 1),
            }
        }
    }

    unsafe fn activate(self, weapon: Object, player: Object) {
        unsafe {
            match self {
                Self::Client => {
                    raw::engine_weapon_set_user(weapon.ptr(), player.ptr());
                    raw::engine_weapon_initialize(weapon.ptr());
                    raw::engine_weapon_activate(weapon.ptr(), 1);
                }
                Self::Core => {
                    raw::engine_weapon_core_set_user(weapon.ptr(), player.ptr());
                    raw::engine_weapon_core_initialize(weapon.ptr());
                    raw::engine_weapon_core_activate(weapon.ptr(), 1);
                }
            }
        }
    }
}

struct SavedSlot {
    player: Object,
    inventory: Object,
    core: Object,
    original: Object,
    slot: WeaponSlot,
    id: u8,
    flavor: Flavor,
}

/// A live native pickup. It cannot release private ammo while the weapon
/// still refers to it: restoring the displaced slot precedes resource release.
/// `restore` returns the private resource to its match bank for later rollback.
/// Dropping also restores, then releases it. No Clone/Send/Sync or raw access.
pub struct InstalledPickup {
    saved: SavedSlot,
    pickup: ManuallyDrop<PreparedWeapon>,
}

/// Native decode result and the owners keeping its restored slots attached.
pub struct DecodedPickups<R> {
    pub output: R,
    pub slots: [Option<InstalledPickup>; 2],
}

impl PreparedWeapon {
    /// Replace the held slot and reuse its existing active-object registry ID.
    /// Failure returns the untouched prepared resource. All checks precede
    /// native mutation; the transaction makes no resource queries. Native
    /// animation callbacks retain their normal allocation behavior.
    ///
    /// # Safety
    /// The match owner must keep this player, inventory and core alive until
    /// the returned owner is restored or dropped, on the simulation thread.
    /// It must restore before native respawn/setup, player destruction, world
    /// teardown or snapshot replacement changes this slot/registry mapping.
    /// No other mechanism may replace that mapping while this owner exists.
    /// `grant` must describe the accepted pickup in this exact simulation event.
    /// Native callbacks may run, but must not reenter this ownership transaction.
    /// This boundary is not yet called by a gameplay hook.
    pub(in crate::vostok) unsafe fn install(
        self,
        player: Object,
        grant: PickupGrant,
    ) -> Result<InstalledPickup, (InstallError, Self)> {
        let saved = match self.inspect_install(player, grant) {
            Ok(saved) => saved,
            Err(error) => return Err((error, self)),
        };
        // SAFETY: validated live native objects, callback/lifetime contract
        // supplied by caller. Retain before replacing the inventory's ref.
        unsafe {
            let original_item = raw::weapon_core::item(saved.original.ptr()).cast();
            AtomicU32::from_ptr(raw::unmanaged_resource::references(original_item))
                .fetch_add(1, Ordering::AcqRel);
            saved.flavor.deactivate(saved.original);
            raw::engine_animation_forget_weapon(
                raw::player::animation_player(saved.player.ptr()).cast(),
                saved.original.ptr(),
            );
            self.attach_mapping(&saved);
            raw::player::held_item(saved.player.ptr()).write(self.weapon.id());
            raw::player::pending_item(saved.player.ptr()).write(self.weapon.id());
            saved.flavor.activate(self.weapon, saved.player);
        }
        Ok(InstalledPickup {
            saved,
            pickup: ManuallyDrop::new(self),
        })
    }

    fn inspect_install(
        &self,
        player: Object,
        grant: PickupGrant,
    ) -> Result<SavedSlot, InstallError> {
        let saved = self.inspect_slot(player, grant.slot())?;
        unsafe {
            if read(raw::player::alive(player.ptr())) == 0 {
                return Err(InstallError::PlayerUnavailable);
            }
            if pointer(raw::player::held_item(player.ptr())) != Some(saved.original)
                || read(raw::weapon_core::user(saved.original.ptr())) != player.id()
            {
                return Err(InstallError::NotHeld);
            }
            if pointer(raw::player::pending_item(player.ptr())) != Some(saved.original) {
                return Err(InstallError::Switching);
            }
        }
        if self.ammunition() != Some(grant.ammo()) {
            return Err(InstallError::GrantChanged);
        }
        Ok(saved)
    }

    fn inspect_slot(&self, player: Object, slot: WeaponSlot) -> Result<SavedSlot, InstallError> {
        // SAFETY: Player's hook guarantees a live native object, and this
        // owner keeps its unbound prepared resource alive throughout validation.
        unsafe {
            let inventory = pointer(raw::player::inventory(player.ptr()))
                .ok_or(InstallError::PlayerUnavailable)?;
            let core =
                pointer(raw::player::core(player.ptr())).ok_or(InstallError::PlayerUnavailable)?;
            let original_item =
                pointer(raw::inventory::slots(inventory.ptr()).add(slot.native() as usize))
                    .ok_or(InstallError::NotHeld)?;
            if ![
                raw::image::WEAPON_ITEM_VTABLE,
                raw::image::WEAPON_CORE_ITEM_VTABLE,
            ]
            .contains(&read(raw::inventory_item::vtable(original_item.ptr())))
            {
                return Err(InstallError::WrongWeapon);
            }
            let original = Object::at(raw::weapon_item::active_object(original_item.ptr()) as u32)
                .ok_or(InstallError::WrongWeapon)?;
            let flavor = Flavor::of(original).ok_or(InstallError::WrongWeapon)?;
            if Flavor::of(self.weapon) != Some(flavor)
                || read(raw::inventory_item::owner(original_item.ptr())) != inventory.id()
                || read(raw::inventory_item::slot(original_item.ptr())) != slot.native()
            {
                return Err(InstallError::WrongWeapon);
            }
            let id = read(raw::weapon_core::active_id(original.ptr()));
            if read(raw::weapon_core::core(original.ptr())) != core.id()
                || read(raw::weapon_core::item_core(original.ptr())) != core.id()
                || registry(core, id).map(|cell| read(cell)) != Some(original.id())
            {
                return Err(InstallError::InvalidRegistry);
            }
            if inventory == self._inventory.this
                || read(raw::weapon_core::user(self.weapon.ptr())) != 0
                || read(raw::weapon_core::ammo(self.weapon.ptr())) != 0
                || read(raw::weapon_core::core(self.weapon.ptr())) != 0
                || read(raw::weapon_core::item_core(self.weapon.ptr())) != 0
                || read(raw::weapon_core::active_id(self.weapon.ptr())) != u8::MAX
                || read(raw::weapon_core::inventory(self.weapon.ptr())) != self._inventory.this.id()
            {
                return Err(InstallError::AlreadyInstalled);
            }
            let count = read(raw::weapon_core::ammo_slot_count(self.weapon.ptr()));
            if usize::from(count) != self.supplies.iter().flatten().count() {
                return Err(InstallError::InvalidSupply);
            }
            let slots = pointer(raw::weapon_core::ammo_slots(self.weapon.ptr()))
                .ok_or(InstallError::InvalidSupply)?;
            for (index, supply) in self.supplies.iter().enumerate() {
                let Some(supply) = supply else { continue };
                if read(
                    raw::inventory::slots(self._inventory.this.ptr())
                        .add(supply.original_slot as usize),
                ) != supply.stack.id()
                    || read(slots.ptr().cast::<u32>().add(index)) != supply.original_slot
                    || read(raw::inventory_item::owner(supply.stack.ptr()))
                        != self._inventory.this.id()
                    || read(raw::inventory_item::slot(supply.stack.ptr())) != supply.original_slot
                {
                    return Err(InstallError::InvalidSupply);
                }
            }
            Ok(SavedSlot {
                player,
                inventory,
                core,
                original,
                slot,
                id,
                flavor,
            })
        }
    }

    // All callers retain the original before this changes the inventory's
    // reference. User/held/animation transitions belong to the enclosing live
    // pickup or snapshot transaction, not to this mapping operation.
    unsafe fn attach_mapping(&self, saved: &SavedSlot) {
        unsafe {
            self.bind_supplies(saved.inventory);
            raw::weapon_core::core(self.weapon.ptr()).write(saved.core.id());
            raw::weapon_core::item_core(self.weapon.ptr()).write(saved.core.id());
            raw::weapon_core::active_id(self.weapon.ptr()).write(saved.id);
            let incoming = raw::weapon_core::item(self.weapon.ptr()) as u32;
            raw::engine_inventory_set(saved.inventory.ptr(), saved.slot.native(), &incoming);
            registry(saved.core, saved.id)
                .expect("validated registry")
                .write(self.weapon.id());
        }
    }

    unsafe fn bind_supplies(&self, recipient: Object) {
        unsafe {
            let slots = read(raw::weapon_core::ammo_slots(self.weapon.ptr())) as *mut u32;
            for (index, supply) in self.supplies.iter().enumerate() {
                let Some(supply) = supply else { continue };
                let cell = raw::inventory::slots(self._inventory.this.ptr())
                    .add(supply.original_slot as usize);
                let base = raw::inventory::slots(recipient.ptr()) as u32;
                let slot = (((cell as u32).wrapping_sub(base) as i32) >> 2) as u32;
                slots.add(index).write(slot);
                raw::engine_item_set_owner(supply.stack.ptr(), recipient.ptr(), slot);
            }
        }
    }

    // The enclosing transaction has deactivated this weapon and replaced
    // every external slot/registry/held reference. Its private inventory
    // continues owning the weapon and ammo while their bindings return home.
    unsafe fn detach_mapping(&self) {
        unsafe {
            let slots = read(raw::weapon_core::ammo_slots(self.weapon.ptr())) as *mut u32;
            for (index, supply) in self.supplies.iter().enumerate() {
                let Some(supply) = supply else { continue };
                slots.add(index).write(supply.original_slot);
                raw::engine_item_set_owner(
                    supply.stack.ptr(),
                    self._inventory.this.ptr(),
                    supply.original_slot,
                );
            }
            raw::engine_item_set_owner(
                raw::weapon_core::item(self.weapon.ptr()).cast(),
                self._inventory.this.ptr(),
                self.source_slot.native(),
            );
            raw::weapon_core::core(self.weapon.ptr()).write(0);
            raw::weapon_core::item_core(self.weapon.ptr()).write(0);
            raw::weapon_core::active_id(self.weapon.ptr()).write(u8::MAX);
            if let Some(ammo) = pointer(raw::weapon_core::ammo(self.weapon.ptr())) {
                raw::weapon_core::ammo(self.weapon.ptr()).write(0);
                let count = raw::unmanaged_resource::references(ammo.ptr());
                if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                    raw::engine_release_object(ammo.ptr(), count);
                }
            }
        }
    }
}

/// Attach both decoded pickup slots before running the native player reader.
/// All refusals leave `resources` untouched, before either slot changes or
/// `decode` is called. Success takes the resources into the returned owners.
/// The closure owns native held/user/animation transitions and ammo decoding;
/// the two InstalledPickup owners keep their originals/private cells alive.
///
/// # Safety
/// The same lifetime/exclusive-mapping contract as live install applies. Old
/// pickup owners must already be restored, and native roster/kit cleanup must
/// have selected this player. `decode` must synchronously run this player's
/// complete native deserializer exactly once with the trusted target snapshot,
/// including both weapon states. Private reserves are restored here before
/// the native reader can query them or compute inventory weight. The saved
/// original is restored afterward, once that reader has cleared its user.
/// Both saved originals/templates are validated before either slot changes.
/// The callback must not
/// reenter pickup ownership or expose the intermediate mapping to gameplay.
/// Prepared resources must correspond to that snapshot's decoded templates;
/// their loaded ammunition is overwritten by the native reader, not granted.
/// This boundary is not yet called by a gameplay hook.
pub(in crate::vostok) unsafe fn decode_pickup_slots<R>(
    player: Object,
    resources: &mut [Option<SnapshotWeapon>; 2],
    decode: impl FnOnce() -> R,
) -> Result<DecodedPickups<R>, InstallError> {
    let mut saved = [None, None];
    for (index, slot) in [WeaponSlot::First, WeaponSlot::Second]
        .into_iter()
        .enumerate()
    {
        let Some(resource) = &resources[index] else {
            continue;
        };
        let original = resource.weapon.inspect_slot(player, slot)?;
        original::validate(&original, resource.state.original)?;
        // If this is the old held weapon, the native reader may deactivate
        // it when the decoded ID maps to a replacement. Its user must still
        // be live. A stowed original must have no active user.
        unsafe {
            let held = pointer(raw::player::held_item(player.ptr())) == Some(original.original);
            let expected_user = if held { player.id() } else { 0 };
            if read(raw::weapon_core::user(original.original.ptr())) != expected_user {
                return Err(InstallError::WrongWeapon);
            }
        }
        saved[index] = Some((original, resource.state.original));
    }
    let mut installed = [None, None];
    for (index, original) in saved.into_iter().enumerate() {
        let Some((original, original_state)) = original else {
            continue;
        };
        let SnapshotWeapon {
            weapon: resource,
            state,
        } = resources[index].take().expect("validated resource");
        unsafe {
            let item = raw::weapon_core::item(original.original.ptr()).cast();
            AtomicU32::from_ptr(raw::unmanaged_resource::references(item))
                .fetch_add(1, Ordering::AcqRel);
            resource.attach_mapping(&original);
            for (supply, &count) in resource
                .supplies
                .iter()
                .flatten()
                .zip(state.reserves.counts())
            {
                raw::item::count(supply.stack.ptr()).write(count);
            }
            raw::inventory::weight_dirty(resource._inventory.this.ptr()).write(1);
            raw::inventory::weight_dirty(original.inventory.ptr()).write(1);
        }
        installed[index] = Some((
            InstalledPickup {
                saved: original,
                pickup: ManuallyDrop::new(resource),
            },
            original_state,
        ));
    }
    // In particular, do not prewrite player.held/pending: base_player's
    // decoder compares old held with the newly resolved ID to decide whether
    // to deactivate/set_user and which activation path to take.
    let decoded = decode();
    let slots = installed.map(|slot| {
        slot.map(|(owner, state)| {
            original::apply(&owner, state);
            owner
        })
    });
    Ok(DecodedPickups {
        output: decoded,
        slots,
    })
}

impl InstalledPickup {
    /// Replace a held borrowed weapon while retaining the true original.
    /// Refusal leaves this owner unchanged and returns the incoming resource.
    /// Success returns the displaced private resource for history reuse,
    /// with no remaining player binding.
    /// No intermediate activation or additional reference to the original is
    /// needed. This owner's installation contract covers native callbacks.
    pub fn replace(
        &mut self,
        next: PreparedWeapon,
        grant: PickupGrant,
    ) -> Result<PreparedWeapon, (InstallError, PreparedWeapon)> {
        let checked = (|| {
            if grant.slot() != self.saved.slot {
                return Err(InstallError::WrongWeapon);
            }
            let current = next.inspect_install(self.saved.player, grant)?;
            if current.original != self.pickup.weapon
                || current.inventory != self.saved.inventory
                || current.core != self.saved.core
                || current.id != self.saved.id
                || current.flavor != self.saved.flavor
            {
                return Err(InstallError::WrongWeapon);
            }
            self.pickup
                .refill_plan(self.saved.inventory)
                .map_err(|_| InstallError::InvalidSupply)?;
            Ok(())
        })();
        if let Err(error) = checked {
            return Err((error, next));
        }
        // SAFETY: both mappings and the held/pending transition were checked
        // without mutation. This owner retains the actual original; each
        // private inventory retains its own gun/ammo throughout handover.
        // The original install contract excludes callback reentrancy here.
        unsafe {
            self.saved.flavor.deactivate(self.pickup.weapon);
            raw::engine_animation_forget_weapon(
                raw::player::animation_player(self.saved.player.ptr()).cast(),
                self.pickup.weapon.ptr(),
            );
            next.attach_mapping(&self.saved);
            raw::player::held_item(self.saved.player.ptr()).write(next.weapon.id());
            raw::player::pending_item(self.saved.player.ptr()).write(next.weapon.id());
            self.pickup.detach_mapping();
            self.saved.flavor.activate(next.weapon, self.saved.player);
        }
        let previous = ManuallyDrop::into_inner(core::mem::replace(
            &mut self.pickup,
            ManuallyDrop::new(next),
        ));
        Ok(previous)
    }

    /// Capture every compatible private stack for this exact installed owner.
    /// Stale bindings fail rather than silently saving somebody else's ammo.
    pub fn snapshot_reserves(&self) -> Result<weapon_pickup::Reserves, ResupplyError> {
        self.pickup.refill_plan(self.saved.inventory)?;
        let mut counts = [0; 8];
        let mut count = 0;
        for supply in self.pickup.supplies.iter().flatten() {
            counts[count] = unsafe { read(raw::item::count(supply.stack.ptr())) };
            count += 1;
        }
        weapon_pickup::Reserves::parse(&counts[..count]).ok_or(ResupplyError::InvalidBinding)
    }
    // The crate bridge borrows the original solely for native profile ammo
    // accounting. It must never install this pointer in the live inventory.
    pub(super) fn original_for_crate(&self, inventory: Object, slot: WeaponSlot) -> Option<Object> {
        if self.saved.inventory != inventory || self.saved.slot != slot {
            return None;
        }
        self.pickup.refill_plan(inventory).ok()?;
        Some(self.saved.original)
    }

    pub(super) fn apply_crate_refill(
        &mut self,
        plan: [super::Refill; 8],
        inventory: Object,
    ) -> u32 {
        self.pickup.apply_refill(plan, inventory)
    }

    pub fn weapon(&self) -> &PreparedWeapon {
        &self.pickup
    }

    pub fn resupply(&mut self, completion: &CrateCompletion<'_>) -> Result<u32, ResupplyError> {
        self.pickup.resupply(completion)
    }

    /// Restore the displaced live slot; keep the private resource available
    /// to the match owner. Snapshot attachment uses `decode_pickup_slots`
    /// around the native reader; this is the ordinary live/respawn cleanup path.
    pub fn restore(self) -> PreparedWeapon {
        let mut this = ManuallyDrop::new(self);
        this.restore_native();
        unsafe { ManuallyDrop::take(&mut this.pickup) }
    }

    fn restore_native(&mut self) {
        let saved = &self.saved;
        // SAFETY: install's match-lifetime and exclusive-mapping contract is
        // maintained by this owner. Deactivation precedes all pointer changes;
        // refs survive callbacks, and all ownership is restored before release.
        unsafe {
            let held = read(raw::player::held_item(saved.player.ptr())) == self.pickup.weapon.id();
            if read(raw::weapon_core::user(self.pickup.weapon.ptr())) != 0 {
                saved.flavor.deactivate(self.pickup.weapon);
                raw::engine_animation_forget_weapon(
                    raw::player::animation_player(saved.player.ptr()).cast(),
                    self.pickup.weapon.ptr(),
                );
            }
            let original_item = raw::weapon_core::item(saved.original.ptr()) as u32;
            raw::engine_inventory_set(saved.inventory.ptr(), saved.slot.native(), &original_item);
            registry(saved.core, saved.id)
                .expect("owned registry lives until restoration")
                .write(saved.original.id());
            if read(raw::player::pending_item(saved.player.ptr())) == self.pickup.weapon.id() {
                raw::player::pending_item(saved.player.ptr()).write(saved.original.id());
            }
            if held {
                raw::player::held_item(saved.player.ptr()).write(saved.original.id());
                if read(raw::player::alive(saved.player.ptr())) != 0 {
                    saved.flavor.activate(saved.original, saved.player);
                }
            }
            self.pickup.detach_mapping();
            let item = original_item as *mut core::ffi::c_void;
            let count = raw::unmanaged_resource::references(item);
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(item, count);
            }
        }
    }
}

impl Drop for InstalledPickup {
    fn drop(&mut self) {
        self.restore_native();
        // SAFETY: native pointers no longer refer to private cells/weapon.
        unsafe { ManuallyDrop::drop(&mut self.pickup) };
    }
}

unsafe fn registry(core: Object, id: u8) -> Option<*mut u32> {
    unsafe {
        let begin = read(raw::game_core::active_begin(core.ptr()));
        let end = read(raw::game_core::active_end(core.ptr()));
        let bytes = end.checked_sub(begin)?;
        if begin == 0
            || !begin.is_multiple_of(4)
            || !bytes.is_multiple_of(4)
            || bytes > 255 * 4
            || id == u8::MAX
            || u32::from(id) >= bytes / 4
        {
            return None;
        }
        Some((begin as *mut u32).add(usize::from(id)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Fixture;
    use super::*;
    use core::cell::RefCell;
    use core::ffi::c_void;
    use std::{boxed::Box, vec::Vec};
    use weapon_pickup::{AmmoKind, CarryRule, Chamber, DroppedAmmo, Handedness, Loadout};

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Deactivate(u32),
        Forget(u32),
        Set(u32),
        User(u32),
        Initialize(u32),
        Activate(u32),
        RestoreOriginal(u32),
    }
    std::thread_local! { static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) }; }
    fn event(e: Event) {
        EVENTS.with_borrow_mut(|events| events.push(e));
    }

    struct CrateCall {
        inventory: u32,
        original: u32,
        profile: u32,
        player: u32,
        held: u32,
        checked: usize,
        refilled: usize,
        availability: u32,
    }
    std::thread_local! {
        static CRATE: RefCell<Option<CrateCall>> = const { RefCell::new(None) };
    }

    unsafe fn check_crate_view(view: *mut c_void, profile: *mut c_void, kit: u32, c: &CrateCall) {
        assert_ne!(view as u32, c.inventory);
        assert_eq!(profile as u32, c.profile);
        assert_eq!(kit, 1);
        unsafe {
            for slot in 0..23 {
                let expected = if slot == 10 {
                    c.original + 0x10
                } else {
                    *raw::inventory::slots(c.inventory as *mut c_void).add(slot)
                };
                assert_eq!(*raw::inventory::slots(view).add(slot), expected);
            }
            assert_eq!(*raw::player::held_item(c.player as *mut c_void), c.held);
            assert_eq!(*raw::player::pending_item(c.player as *mut c_void), c.held);
            assert_eq!(
                *raw::inventory::slots(c.inventory as *mut c_void).add(10),
                c.held + 0x10
            );
        }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_inventory_check_resupply(
        kit: u32,
        view: *mut c_void,
        profile: *mut c_void,
    ) -> u32 {
        CRATE.with_borrow_mut(|state| {
            let c = state.as_mut().expect("unexpected native crate call");
            unsafe { check_crate_view(view, profile, kit, c) };
            c.checked += 1;
            c.availability
        })
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_inventory_resupply(
        view: *mut c_void,
        profile: *mut c_void,
        kit: u32,
    ) {
        CRATE.with_borrow_mut(|state| {
            let c = state.as_mut().expect("unexpected native crate call");
            unsafe {
                check_crate_view(view, profile, kit, c);
                raw::inventory::weight_dirty(view).write(1);
            }
            c.refilled += 1;
        });
    }

    // These FFI stubs test Rust's transaction order/ownership, not native
    // animations or effects. The generated thunk ABI is checked separately.
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_item_set_owner(
        item: *mut c_void,
        inventory: *mut c_void,
        slot: u32,
    ) {
        unsafe {
            raw::inventory_item::owner(item).write(inventory as u32);
            raw::inventory_item::slot(item).write(slot);
        }
    }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_inventory_set(inventory: *mut c_void, slot: u32, item: *const u32) {
        unsafe {
            let incoming = *item as *mut c_void;
            let cell = raw::inventory::slots(inventory).add(slot as usize);
            let old = *cell as *mut c_void;
            *raw::unmanaged_resource::references(incoming) += 1;
            *cell = incoming as u32;
            if !old.is_null() {
                let count = raw::unmanaged_resource::references(old);
                assert!(*count > 1, "the displaced item must already be retained");
                *count -= 1;
            }
            engine_item_set_owner(incoming, inventory, slot);
            raw::inventory::weight_dirty(inventory).write(1);
            event(Event::Set(incoming as u32));
        }
    }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_animation_forget_weapon(
        _animation: *mut c_void,
        weapon: *mut c_void,
    ) {
        assert_eq!(unsafe { *raw::weapon_core::user(weapon) }, 0);
        event(Event::Forget(weapon as u32));
    }

    unsafe fn deactivate(weapon: *mut c_void, clear: u32) {
        assert_eq!(clear, 1);
        unsafe {
            assert_ne!(*raw::weapon_core::user(weapon), 0);
            raw::weapon_core::user(weapon).write(0);
        }
        event(Event::Deactivate(weapon as u32));
    }
    unsafe fn set_user(weapon: *mut c_void, player: *mut c_void) {
        unsafe {
            assert_eq!(*raw::player::held_item(player), weapon as u32);
            assert_eq!(*raw::weapon_core::user(weapon), 0);
            raw::weapon_core::user(weapon).write(player as u32);
        }
        event(Event::User(weapon as u32));
    }
    unsafe fn initialize(weapon: *mut c_void) {
        unsafe {
            assert_eq!(*raw::weapon_core::load_on_initialize(weapon), 0);
            let inventory = *raw::weapon_core::inventory(weapon) as *mut c_void;
            let index = *raw::weapon_core::ammo_kind(weapon) as usize;
            let slots = *raw::weapon_core::ammo_slots(weapon) as *const u32;
            let encoded = *slots.add(index);
            let cell = (raw::inventory::slots(inventory) as u32)
                .wrapping_add(encoded.wrapping_mul(4)) as *const u32;
            let stack = *cell as *mut c_void;
            assert_eq!(*raw::inventory_item::owner(stack), inventory as u32);
            assert_eq!(*raw::inventory_item::slot(stack), encoded);
            *raw::unmanaged_resource::references(stack) += 1;
            let old = *raw::weapon_core::ammo(weapon) as *mut c_void;
            if !old.is_null() {
                *raw::unmanaged_resource::references(old) -= 1;
            }
            raw::weapon_core::ammo(weapon).write(stack as u32);
        }
        event(Event::Initialize(weapon as u32));
    }
    unsafe fn activate(weapon: *mut c_void, flag: u32) {
        assert_eq!(flag, 1);
        unsafe {
            let core = Object::at(*raw::weapon_core::core(weapon)).unwrap();
            let id = *raw::weapon_core::active_id(weapon);
            assert_eq!(*registry(core, id).unwrap(), weapon as u32);
            assert_ne!(*raw::weapon_core::user(weapon), 0);
        }
        event(Event::Activate(weapon as u32));
    }
    macro_rules! lifecycle {
        ($flavor:expr, $set:ident, $init:ident, $activate:ident, $deactivate:ident) => {
            #[unsafe(no_mangle)]
            unsafe extern "C" fn $set(w: *mut c_void, p: *mut c_void) {
                assert!(unsafe { Flavor::of(Object::at(w as u32).unwrap()) } == Some($flavor));
                unsafe { set_user(w, p) }
            }
            #[unsafe(no_mangle)]
            unsafe extern "C" fn $init(w: *mut c_void) {
                assert!(unsafe { Flavor::of(Object::at(w as u32).unwrap()) } == Some($flavor));
                unsafe { initialize(w) }
            }
            #[unsafe(no_mangle)]
            unsafe extern "C" fn $activate(w: *mut c_void, f: u32) {
                assert!(unsafe { Flavor::of(Object::at(w as u32).unwrap()) } == Some($flavor));
                unsafe { activate(w, f) }
            }
            #[unsafe(no_mangle)]
            unsafe extern "C" fn $deactivate(w: *mut c_void, f: u32) {
                assert!(unsafe { Flavor::of(Object::at(w as u32).unwrap()) } == Some($flavor));
                unsafe { deactivate(w, f) }
            }
        };
    }
    lifecycle!(
        Flavor::Client,
        engine_weapon_set_user,
        engine_weapon_initialize,
        engine_weapon_activate,
        engine_weapon_deactivate
    );
    lifecycle!(
        Flavor::Core,
        engine_weapon_core_set_user,
        engine_weapon_core_initialize,
        engine_weapon_core_activate,
        engine_weapon_core_deactivate
    );

    struct Recipient {
        original: Fixture,
        entry: Box<[u32; 180]>,
        player: Box<[u32; 1024]>,
        core: Box<[u32; 0x19000 / 4]>,
        registry: Box<[u32; 4]>,
        roster: Box<[u32; 24]>,
    }
    impl Recipient {
        fn new() -> Self {
            let mut r = Self {
                original: Fixture::new(),
                entry: Box::new([0; 180]),
                player: Box::new([0; 1024]),
                core: Box::new([0; 0x19000 / 4]),
                registry: Box::new([0; 4]),
                roster: Box::new([0; 24]),
            };
            unsafe {
                let p = r.player.as_mut_ptr().cast();
                let core = r.core.as_mut_ptr().cast();
                let weapon = r.original.weapon.as_mut_ptr().cast();
                raw::item::entry(raw::weapon_core::item(weapon).cast())
                    .write(r.entry.as_mut_ptr() as u32);
                for place in [0, 1] {
                    for kit in 0..3 {
                        r.roster[place * 12 + kit] = p as u32;
                    }
                }
                raw::game_core::roster_begin(core).write(r.roster.as_mut_ptr() as u32);
                raw::game_core::roster_end(core).write(r.roster.as_mut_ptr().add(24) as u32);
                raw::player::alive(p).write(1);
                raw::player::inventory(p).write(r.original.inventory.as_mut_ptr() as u32);
                raw::player::core(p).write(core as u32);
                raw::player::held_item(p).write(weapon as u32);
                raw::player::pending_item(p).write(weapon as u32);
                raw::weapon_core::user(weapon).write(p as u32);
                raw::weapon_core::core(weapon).write(core as u32);
                raw::weapon_core::item_core(weapon).write(core as u32);
                raw::weapon_core::active_id(weapon).write(1);
                raw::weapon_core::load_on_initialize(weapon).write(0);
                raw::weapon_core::ammo_kind(weapon).write(1);
                raw::weapon_core::ammo(weapon).write(r.original.ammo[1].as_mut_ptr() as u32);
                raw::unmanaged_resource::references(r.original.ammo[1].as_mut_ptr().cast())
                    .write(2);
                r.registry[0] = 0x1234; // unrelated entry, never followed
                r.registry[1] = weapon as u32;
                raw::game_core::active_begin(core).write(r.registry.as_mut_ptr() as u32);
                raw::game_core::active_end(core).write(r.registry.as_mut_ptr().add(2) as u32);
            }
            r
        }
        fn object(&mut self) -> Object {
            Object::at(self.player.as_mut_ptr() as u32).unwrap()
        }
        fn original_restored(&self, alive: bool) {
            unsafe {
                let weapon = self.original.weapon.as_ptr().cast_mut().cast();
                let p = self.player.as_ptr().cast_mut().cast();
                assert_eq!(*raw::player::held_item(p), weapon as u32);
                assert_eq!(*raw::player::pending_item(p), weapon as u32);
                assert_eq!(self.registry[0], 0x1234);
                assert_eq!(self.registry[1], weapon as u32);
                assert_eq!(
                    *raw::inventory::slots(self.original.inventory.as_ptr().cast_mut().cast())
                        .add(10),
                    raw::weapon_core::item(weapon) as u32
                );
                assert_eq!(
                    *raw::unmanaged_resource::references(raw::weapon_core::item(weapon).cast()),
                    1
                );
                assert_eq!(
                    *raw::weapon_core::user(weapon),
                    if alive { p as u32 } else { 0 }
                );
                assert_eq!(*raw::weapon_core::magazine(weapon), 23);
            }
        }
    }
    fn ammo() -> DroppedAmmo {
        DroppedAmmo::parse(30, 7, Chamber::Loaded, AmmoKind::parse(1).unwrap()).unwrap()
    }
    fn grant() -> PickupGrant {
        PickupGrant::check(
            Loadout::new(
                [None, Some(Handedness::Two)],
                Some(WeaponSlot::Second),
                CarryRule::OneDoubleHanded,
            ),
            Handedness::Two,
            ammo(),
        )
        .unwrap()
    }

    fn snapshot_state(rounds: &[u16], slot: WeaponSlot) -> weapon_pickup::history::SlotState {
        let mut bytes = [0; 20];
        bytes[6] = slot.native() as u8;
        bytes[16] = 23;
        bytes[18] = 1;
        bytes[19] = 255;
        weapon_pickup::history::SlotState {
            reserves: weapon_pickup::Reserves::parse(rounds).unwrap(),
            original: weapon_pickup::Original::parse(&bytes).unwrap(),
        }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_weapon_core_deserialize(
        weapon: *mut c_void,
        reader: *mut c_void,
        client_reader: *mut c_void,
        context: u32,
    ) {
        unsafe {
            assert!(client_reader.is_null());
            assert_eq!(context, 0);
            assert_eq!(*raw::weapon_core::user(weapon), 0);
            let bytes = core::slice::from_raw_parts(
                *raw::buffer_reader::cursor(reader) as *const u8,
                *raw::buffer_reader::size(reader) as usize,
            );
            let state = weapon_pickup::Original::parse(bytes).unwrap();
            let (place, kit, slot) = state.owner();
            let core = *raw::weapon_core::item_core(weapon) as *mut c_void;
            let roster = *raw::game_core::roster_begin(core) as *const u32;
            let player =
                *roster.add(usize::from(place.get()) * 12 + usize::from(kit.get())) as *mut c_void;
            let inventory = *raw::player::inventory(player) as *mut c_void;
            let item = raw::weapon_core::item(weapon).cast();
            raw::item::condition_bits(item)
                .write(u32::from_le_bytes(bytes[..4].try_into().unwrap()));
            raw::weapon_core::inventory(weapon).write(inventory as u32);
            raw::inventory_item::slot(item).write(slot.native());
            raw::weapon_core::snapshot_state_word(weapon)
                .write(u32::from_le_bytes(bytes[7..11].try_into().unwrap()));
            raw::weapon_core::last_tick_time(weapon)
                .write(u32::from_le_bytes(bytes[11..15].try_into().unwrap()));
            raw::weapon_core::load_on_initialize(weapon).write(u8::from(bytes[15] != 0));
            raw::weapon_core::magazine(weapon).write(bytes[16]);
            raw::weapon_core::fire_queue_type(weapon).write(bytes[17]);
            raw::weapon_core::ammo_kind(weapon).write(bytes[18]);
            if bytes.len() == 20 {
                raw::weapon_core::chambered(weapon).write(u8::from(bytes[19] != 0));
            }
            let slots = *raw::weapon_core::ammo_slots(weapon) as *const u32;
            let stack = *raw::inventory::slots(inventory)
                .add(*slots.add(usize::from(bytes[18])) as usize)
                as *mut c_void;
            let old = *raw::weapon_core::ammo(weapon) as *mut c_void;
            *raw::unmanaged_resource::references(stack) += 1;
            raw::weapon_core::ammo(weapon).write(stack as u32);
            if !old.is_null() {
                *raw::unmanaged_resource::references(old) -= 1;
            }
            *raw::buffer_reader::cursor(reader) += bytes.len() as u32;
            event(Event::RestoreOriginal(weapon as u32));
        }
    }

    #[test]
    fn install_and_restore_keep_registry_refs_ammo_and_callback_order_together() {
        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        let prepared = source.prepare(ammo()).unwrap();
        let new = prepared.weapon.id();
        let old = recipient.original.weapon.as_mut_ptr() as u32;
        EVENTS.with_borrow_mut(Vec::clear);
        let installed = unsafe { prepared.install(recipient.object(), grant()) }
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        assert_eq!(installed.weapon().ammunition(), Some(ammo()));
        assert_eq!(recipient.registry[1], new);
        EVENTS.with_borrow(|events| {
            assert_eq!(
                events,
                &[
                    Event::Deactivate(old),
                    Event::Forget(old),
                    Event::Set(new + 0x10),
                    Event::User(new),
                    Event::Initialize(new),
                    Event::Activate(new)
                ]
            )
        });
        assert_eq!(
            installed.weapon().capture_for(
                Object::at(new).unwrap(),
                Object::at(recipient.original.inventory.as_mut_ptr() as u32).unwrap()
            ),
            Some(ammo())
        );
        EVENTS.with_borrow_mut(Vec::clear);
        let prepared = installed.restore();
        recipient.original_restored(true);
        EVENTS.with_borrow(|events| {
            assert_eq!(
                events,
                &[
                    Event::Deactivate(new),
                    Event::Forget(new),
                    Event::Set(old + 0x10),
                    Event::User(old),
                    Event::Initialize(old),
                    Event::Activate(old)
                ]
            )
        });
        unsafe {
            assert_eq!(*raw::weapon_core::ammo(prepared.weapon.ptr()), 0);
            assert_eq!(*raw::weapon_core::core(prepared.weapon.ptr()), 0);
            assert_eq!(*raw::weapon_core::active_id(prepared.weapon.ptr()), u8::MAX);
            assert_eq!(
                *raw::unmanaged_resource::references(source.ammo[1].as_mut_ptr().cast()),
                1
            );
        }
        assert_eq!(*source.slots, [8, 19]);
        // A restored owner can be installed again without double-retaining or
        // losing the original. Drop follows the same restoration as restore().
        let installed = unsafe { prepared.install(recipient.object(), grant()) }
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        drop(installed);
        recipient.original_restored(true);
    }

    #[test]
    fn repeated_pickups_keep_true_original_and_detach_previous_private_ammo() {
        for alive in [true, false] {
            let mut first_source = Fixture::new();
            let mut next_source = Fixture::new();
            let mut third_source = Fixture::new();
            let mut recipient = Recipient::new();
            let first = first_source.prepare(ammo()).unwrap();
            let mut installed = unsafe { first.install(recipient.object(), grant()) }
                .unwrap_or_else(|(error, _)| panic!("{error:?}"));
            let original = installed.saved.original;
            let old_weapon = installed.pickup.weapon;
            // The discarded gun may have been used and restocked. Handing
            // it back to the bank must not reset its ammo or touch the next.
            unsafe {
                raw::weapon_core::magazine(old_weapon.ptr()).write(2);
                raw::item::count(first_source.ammo[1].as_mut_ptr().cast()).write(112);
            }
            let next_ammo =
                DroppedAmmo::parse(30, 11, Chamber::Empty, AmmoKind::parse(0).unwrap()).unwrap();
            let next_grant = PickupGrant::check(
                Loadout::new(
                    [None, Some(Handedness::Two)],
                    Some(WeaponSlot::Second),
                    CarryRule::OneDoubleHanded,
                ),
                Handedness::Two,
                next_ammo,
            )
            .unwrap();
            let next = next_source.prepare(next_ammo).unwrap();
            let next_weapon = next.weapon;
            EVENTS.with_borrow_mut(Vec::clear);
            let previous = installed
                .replace(next, next_grant)
                .unwrap_or_else(|(error, _)| panic!("{error:?}"));
            assert_eq!(installed.saved.original, original);
            assert_eq!(installed.weapon().ammunition(), Some(next_ammo));
            assert_eq!(previous.ammunition().unwrap().magazine(), 2);
            assert_eq!(
                previous.supply(AmmoKind::parse(1).unwrap()).unwrap().rounds,
                112
            );
            assert_eq!(
                installed
                    .weapon()
                    .supply(AmmoKind::parse(0).unwrap())
                    .unwrap()
                    .rounds,
                30
            );
            EVENTS.with_borrow(|events| {
                assert_eq!(
                    events,
                    &[
                        Event::Deactivate(old_weapon.id()),
                        Event::Forget(old_weapon.id()),
                        Event::Set(next_weapon.id() + 0x10),
                        Event::User(next_weapon.id()),
                        Event::Initialize(next_weapon.id()),
                        Event::Activate(next_weapon.id()),
                    ]
                )
            });
            unsafe {
                for field in [
                    raw::weapon_core::user(old_weapon.ptr()),
                    raw::weapon_core::core(old_weapon.ptr()),
                    raw::weapon_core::item_core(old_weapon.ptr()),
                    raw::weapon_core::ammo(old_weapon.ptr()),
                ] {
                    assert_eq!(read(field), 0);
                }
                assert_eq!(read(raw::weapon_core::active_id(old_weapon.ptr())), u8::MAX);
                assert_eq!(
                    read(raw::weapon_core::inventory(old_weapon.ptr())),
                    previous._inventory.this.id()
                );
                assert_eq!(
                    read(raw::unmanaged_resource::references(
                        raw::weapon_core::item(original.ptr()).cast()
                    )),
                    1
                );
                assert_eq!(
                    read(raw::unmanaged_resource::references(
                        raw::weapon_core::item(old_weapon.ptr()).cast()
                    )),
                    1
                );
                assert_eq!(
                    read(raw::unmanaged_resource::references(
                        raw::weapon_core::item(next_weapon.ptr()).cast()
                    )),
                    2
                );
                for (index, slot) in [8, 19].into_iter().enumerate() {
                    let stack = first_source.ammo[index].as_mut_ptr().cast();
                    assert_eq!(
                        read(raw::inventory_item::owner(stack)),
                        previous._inventory.this.id()
                    );
                    assert_eq!(read(raw::inventory_item::slot(stack)), slot);
                    assert_eq!(read(raw::unmanaged_resource::references(stack)), 1);
                }
            }
            drop(previous);
            // A third acquisition must still restore the original, even
            // after the two preceding private resources have been released.
            let third = third_source.prepare(ammo()).unwrap();
            let previous = installed
                .replace(third, grant())
                .unwrap_or_else(|(error, _)| panic!("{error:?}"));
            assert_eq!(installed.saved.original, original);
            drop(previous);
            if !alive {
                unsafe {
                    raw::player::alive(recipient.object().ptr()).write(0);
                    raw::weapon_core::user(installed.pickup.weapon.ptr()).write(0);
                }
            }
            drop(installed);
            recipient.original_restored(alive);
        }
    }

    #[test]
    fn refused_replacement_preserves_both_owners_and_original_retention() {
        for bad in 0..6 {
            let mut first_source = Fixture::new();
            let mut next_source = Fixture::new();
            let mut recipient = Recipient::new();
            let first = first_source.prepare(ammo()).unwrap();
            let mut installed = unsafe { first.install(recipient.object(), grant()) }
                .unwrap_or_else(|(error, _)| panic!("{error:?}"));
            let next = next_source.prepare(ammo()).unwrap();
            let old_weapon = installed.pickup.weapon;
            let p = recipient.object();
            unsafe {
                match bad {
                    0 => raw::player::pending_item(p.ptr()).write(0),
                    1 => recipient.registry[1] = 0,
                    2 => raw::weapon_core::magazine(next.weapon.ptr()).write(6),
                    3 => next_source.slots[1] = 22,
                    4 => raw::inventory_item::owner(first_source.ammo[1].as_mut_ptr().cast())
                        .write(0),
                    _ => raw::player::alive(p.ptr()).write(0),
                }
            }
            let player = recipient.player.clone();
            let inventory = recipient.original.inventory.clone();
            let registry = recipient.registry.clone();
            let original = recipient.original.weapon.clone();
            let first_gun = first_source.weapon.clone();
            let next_gun = next_source.weapon.clone();
            let first_ammo = first_source.ammo.clone();
            let next_ammo = next_source.ammo.clone();
            EVENTS.with_borrow_mut(Vec::clear);
            let Err((_, next)) = installed.replace(next, grant()) else {
                panic!("invalid replacement accepted");
            };
            assert_eq!(recipient.player, player);
            assert_eq!(recipient.original.inventory, inventory);
            assert_eq!(recipient.registry, registry);
            assert_eq!(recipient.original.weapon, original);
            assert_eq!(first_source.weapon, first_gun);
            assert_eq!(next_source.weapon, next_gun);
            assert_eq!(first_source.ammo, first_ammo);
            assert_eq!(next_source.ammo, next_ammo);
            EVENTS.with_borrow(|events| assert!(events.is_empty()));
            // Undo fixture corruption before releasing the valid old owner.
            unsafe {
                raw::player::pending_item(p.ptr()).write(old_weapon.id());
                recipient.registry[1] = old_weapon.id();
                raw::inventory_item::owner(first_source.ammo[1].as_mut_ptr().cast())
                    .write(installed.saved.inventory.id());
                raw::player::alive(p.ptr()).write(1);
            }
            drop(next);
            drop(installed);
            recipient.original_restored(true);
        }
    }

    #[test]
    fn refusal_preserves_live_player_and_prepared_resource_without_callbacks() {
        for bad in 0..4 {
            let mut source = Fixture::new();
            let mut recipient = Recipient::new();
            let prepared = source.prepare(ammo()).unwrap();
            match bad {
                0 => unsafe {
                    raw::player::pending_item(recipient.player.as_mut_ptr().cast()).write(0)
                },
                1 => recipient.registry[1] = 0,
                2 => source.slots[1] = 22,
                _ => unsafe { raw::weapon_core::magazine(prepared.weapon.ptr()).write(6) },
            }
            let before_player = recipient.player.clone();
            let before_registry = recipient.registry.clone();
            let before_inventory = recipient.original.inventory.clone();
            EVENTS.with_borrow_mut(Vec::clear);
            let Err((_, prepared)) = (unsafe { prepared.install(recipient.object(), grant()) })
            else {
                panic!("accepted invalid transaction")
            };
            assert_eq!(recipient.player, before_player);
            assert_eq!(recipient.registry, before_registry);
            assert_eq!(recipient.original.inventory, before_inventory);
            EVENTS.with_borrow(|events| assert!(events.is_empty()));
            assert_eq!(
                unsafe { *raw::unmanaged_resource::references(prepared._inventory.this.ptr()) },
                1
            );
            drop(prepared);
        }
    }

    #[test]
    fn restore_after_native_death_does_not_activate_weapon_on_a_dead_player() {
        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        let prepared = source.prepare(ammo()).unwrap();
        let installed = unsafe { prepared.install(recipient.object(), grant()) }
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        unsafe {
            raw::player::alive(recipient.player.as_mut_ptr().cast()).write(0);
            raw::weapon_core::user(installed.pickup.weapon.ptr()).write(0);
        }
        EVENTS.with_borrow_mut(Vec::clear);
        let prepared = installed.restore();
        recipient.original_restored(false);
        EVENTS.with_borrow(|events| {
            assert_eq!(
                events,
                &[Event::Set(recipient.original.weapon.as_ptr() as u32 + 0x10)]
            )
        });
        drop(prepared);
    }

    #[test]
    fn stowed_restore_preserves_selection_and_retargets_only_its_pending_switch() {
        for pending_pickup in [false, true] {
            let mut source = Fixture::new();
            let mut recipient = Recipient::new();
            let prepared = source.prepare(ammo()).unwrap();
            let installed = unsafe { prepared.install(recipient.object(), grant()) }
                .unwrap_or_else(|(error, _)| panic!("{error:?}"));
            // The engine has switched away. Neither pointer for the other
            // held weapon may be followed or changed by this cleanup.
            let other = 0x1234;
            let player = recipient.player.as_mut_ptr().cast();
            let old = recipient.original.weapon.as_ptr() as u32;
            unsafe {
                raw::weapon_core::user(installed.pickup.weapon.ptr()).write(0);
                raw::player::held_item(player).write(other);
                raw::player::pending_item(player).write(if pending_pickup {
                    installed.pickup.weapon.id()
                } else {
                    other
                });
            }
            EVENTS.with_borrow_mut(Vec::clear);
            let detached = installed.restore();
            unsafe {
                assert_eq!(*raw::player::held_item(player), other);
                assert_eq!(
                    *raw::player::pending_item(player),
                    if pending_pickup { old } else { other }
                );
                assert_eq!(*raw::weapon_core::user(old as *mut c_void), 0);
                assert_eq!(*raw::weapon_core::ammo(detached.weapon.ptr()), 0);
                assert_eq!(
                    *raw::unmanaged_resource::references(source.ammo[1].as_mut_ptr().cast()),
                    1
                );
            }
            assert_eq!(recipient.registry[1], old);
            assert_eq!(detached.ammunition(), Some(ammo()));
            EVENTS.with_borrow(|events| assert_eq!(events, &[Event::Set(old + 0x10)]));
            drop(detached);
        }
    }

    #[test]
    fn core_weapon_uses_core_callbacks_and_free_refill_survives_detachment() {
        use crate::vostok::hook::{Handle, Thread};
        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        for fixture in [&mut source, &mut recipient.original] {
            let weapon = fixture.weapon.as_mut_ptr().cast();
            unsafe {
                raw::weapon_core::vtable(weapon).write(raw::image::WEAPON_CORE_ACTIVE_VTABLE);
                raw::inventory_item::vtable(raw::weapon_core::item(weapon).cast())
                    .write(raw::image::WEAPON_CORE_ITEM_VTABLE);
            }
        }
        let prepared = source.prepare(ammo()).unwrap();
        let mut installed = unsafe { prepared.install(recipient.object(), grant()) }
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        let original_ammo = recipient.original.ammo.clone();
        let original_profile = recipient.original.profile.clone();
        let thread = unsafe { Thread::new() };
        let completed = unsafe {
            CrateCompletion::from_raw(&thread, recipient.original.inventory.as_mut_ptr() as u32)
        }
        .unwrap();
        assert!(installed.resupply(&completed).unwrap() > 0);
        assert_eq!(installed.resupply(&completed), Ok(0));
        let kinds = [AmmoKind::parse(0).unwrap(), AmmoKind::parse(1).unwrap()];
        let supplies = kinds.map(|kind| installed.weapon().supply(kind));
        assert_eq!(installed.weapon().ammunition(), Some(ammo()));
        let detached = installed.restore();
        recipient.original_restored(true);
        assert_eq!(kinds.map(|kind| detached.supply(kind)), supplies);
        assert_eq!(recipient.original.ammo, original_ammo);
        assert_eq!(recipient.original.profile, original_profile);
    }

    #[test]
    fn crate_bridge_uses_saved_original_without_swapping_live_selection() {
        use super::super::{
            CrateAvailability, CrateError, CrateInventory, complete_crate, crate_availability,
        };
        use crate::vostok::hook::{Handle, Thread};
        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        let prepared = source.prepare(ammo()).unwrap();
        let player = recipient.object();
        unsafe {
            raw::player::profile(player.ptr())
                .write(recipient.original.profile.as_mut_ptr() as u32);
            raw::player::kit(player.ptr()).write(1);
        }
        let mut installed = unsafe { prepared.install(player, grant()) }
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        let thread = unsafe { Thread::new() };
        let inv = recipient.original.inventory.as_mut_ptr() as u32;
        let query = unsafe { CrateInventory::from_raw(&thread, inv) }.unwrap();
        let completion = unsafe { CrateCompletion::from_raw(&thread, inv) }.unwrap();
        CRATE.with_borrow_mut(|state| {
            *state = Some(CrateCall {
                inventory: inv,
                original: recipient.original.weapon.as_ptr() as u32,
                profile: recipient.original.profile.as_ptr() as u32,
                player: player.id(),
                held: installed.pickup.weapon.id(),
                checked: 0,
                refilled: 0,
                availability: 1, // no original account ammunition left
            })
        });
        assert_eq!(
            crate_availability(player, &query, [None, Some(&installed)]),
            Ok(CrateAvailability::Available)
        );
        // A missing/wrong slot owner must fail before any native storage write.
        assert_eq!(
            complete_crate(player, &completion, [None, None]),
            Err(CrateError::InvalidOriginal)
        );
        assert_eq!(
            complete_crate(player, &completion, [Some(&mut installed), None]),
            Err(CrateError::InvalidOriginal)
        );
        CRATE.with_borrow(|c| assert_eq!(c.as_ref().unwrap().refilled, 0));
        assert_eq!(
            complete_crate(player, &completion, [None, Some(&mut installed)]),
            Ok(232)
        );
        assert_eq!(installed.weapon().ammunition(), Some(ammo()));
        assert_eq!(
            crate_availability(player, &query, [None, Some(&installed)]),
            Ok(CrateAvailability::EmptyStorage)
        );
        assert_eq!(
            complete_crate(player, &completion, [None, Some(&mut installed)]),
            Ok(0)
        );
        CRATE.with_borrow_mut(|c| c.as_mut().unwrap().availability = 2);
        assert_eq!(
            crate_availability(player, &query, [None, Some(&installed)]),
            Ok(CrateAvailability::Full)
        );
        CRATE.with_borrow(|c| {
            assert_eq!(c.as_ref().unwrap().refilled, 2);
            assert_eq!(c.as_ref().unwrap().checked, 2);
        });
        assert_eq!(
            unsafe { *raw::inventory::weight_dirty(inv as *mut c_void) },
            1
        );
        drop(installed);
        recipient.original_restored(true);
        CRATE.with_borrow_mut(|c| *c = None);
    }

    #[test]
    fn displaced_original_capture_matches_native_stowed_codec_without_mutation() {
        for chamber in [false, true] {
            let mut source = Fixture::new();
            let mut recipient = Recipient::new();
            let mut entry = Box::new([0u32; 180]);
            let player = recipient.object();
            let old = recipient.original.weapon.as_mut_ptr().cast();
            unsafe {
                let item = raw::weapon_core::item(old).cast();
                raw::item::entry(item).write(entry.as_mut_ptr() as u32);
                raw::item::condition_bits(item).write(0x3f40_0000);
                raw::weapon_core::snapshot_state_word(old).write(0x3f20_0000);
                raw::weapon_core::last_tick_time(old).write(123456);
                raw::weapon_core::load_on_initialize(old).write(1);
                raw::weapon_core::magazine(old).write(7);
                raw::weapon_core::fire_queue_type(old).write(2);
                raw::weapon_core::has_chamber(old).write(u8::from(chamber));
                raw::weapon_core::chambered(old).write(u8::from(chamber));
                raw::player::place(player.ptr()).write(1);
                raw::player::kit(player.ptr()).write(2);
            }
            let installed = unsafe { source.prepare(ammo()).unwrap().install(player, grant()) }
                .unwrap_or_else(|(error, _)| panic!("{error:?}"));
            let before = recipient.original.weapon.clone();
            let inventory_before = recipient.original.inventory.clone();
            let state = installed.snapshot_original().unwrap();
            let expected = [
                0, 0, 0x40, 0x3f, 1, 2, 10, 0, 0, 0x20, 0x3f, 0x40, 0xe2, 1, 0, 255, 7, 2, 1, 255,
            ];
            assert_eq!(state.bytes(), &expected[..if chamber { 20 } else { 19 }]);
            assert_eq!(recipient.original.weapon, before);
            assert_eq!(recipient.original.inventory, inventory_before);
            unsafe { raw::item_entry::stack(entry.as_mut_ptr().cast()).write(1) };
            assert_eq!(
                installed.snapshot_original(),
                Err(InstallError::WrongWeapon)
            );
            unsafe {
                raw::item_entry::stack(entry.as_mut_ptr().cast()).write(0);
                raw::weapon_core::ammo_kind(old).write(7);
            }
            assert_eq!(
                installed.snapshot_original(),
                Err(InstallError::InvalidSupply)
            );
            unsafe {
                raw::weapon_core::ammo_kind(old).write(1);
                raw::weapon_core::user(old).write(player.id());
            }
            assert_eq!(
                installed.snapshot_original(),
                Err(InstallError::WrongWeapon)
            );
            unsafe { raw::weapon_core::user(old).write(0) };
            assert_eq!(installed.snapshot_original(), Ok(state));
            // The activation fixture models only the already-loaded path;
            // autoload is exercised by the original-executable ammo checks.
            unsafe { raw::weapon_core::load_on_initialize(old).write(0) };
            drop(installed);
        }
    }

    #[test]
    fn saved_reserves_undo_crate_refill_without_regranting_pickup_ammo() {
        use crate::vostok::hook::{Handle, Thread};
        use weapon_pickup::history::History;
        use weapon_pickup::state::{DropId, HeldAtDeath, Kit, Life, Place, World};

        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        let donor = Life {
            place: Place::parse(0).unwrap(),
            spawned: 50,
        };
        let picker = Life {
            place: Place::parse(1).unwrap(),
            spawned: 100,
        };
        let mut world = World::new();
        for life in [donor, picker] {
            world.spawn(life, Kit::parse(1).unwrap());
        }
        world.die(
            donor,
            Some(HeldAtDeath {
                slot: WeaponSlot::Second,
                handedness: Handedness::Two,
                ammo: ammo(),
            }),
        );
        let transfer = world
            .pickup(
                picker,
                DropId(donor),
                Loadout::new(
                    [None, Some(Handedness::Two)],
                    Some(WeaponSlot::Second),
                    CarryRule::OneDoubleHanded,
                ),
            )
            .unwrap();
        let prepared = source.prepare(ammo()).unwrap();
        let player = recipient.object();
        let mut entry = Box::new([0u32; 180]);
        unsafe {
            raw::item::entry(
                raw::weapon_core::item(recipient.original.weapon.as_mut_ptr().cast()).cast(),
            )
            .write(entry.as_mut_ptr() as u32);
            raw::player::place(player.ptr()).write(picker.place.get());
            raw::player::kit(player.ptr()).write(1);
        }
        let mut installed = unsafe { prepared.install(player, transfer.grant) }
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        let saved = History::capture(&world, |life, slot, borrowed| {
            assert_eq!(
                (life, slot, borrowed),
                (picker, WeaponSlot::Second, transfer.acquired)
            );
            installed.snapshot_slot()
        })
        .unwrap()
        .snapshot();
        let thread = unsafe { Thread::new() };
        let completion = unsafe {
            CrateCompletion::from_raw(&thread, recipient.original.inventory.as_mut_ptr() as u32)
        }
        .unwrap();
        assert_eq!(installed.resupply(&completion), Ok(232));
        assert_ne!(installed.snapshot_reserves().unwrap().counts(), &[0, 30]);
        let prepared = installed.restore();
        let history = History::from_snapshot(saved.bytes()).unwrap();
        assert_eq!(history.world(), &world);
        let state = history.slot(picker, WeaponSlot::Second).unwrap();
        let new = prepared.weapon.id();
        let old = recipient.original.weapon.as_ptr() as u32;
        let resource = prepared
            .for_snapshot(state)
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        let owners = unsafe {
            decode_pickup_slots(player, &mut [None, Some(resource)], || {
                // Record application supplies the native weapon state too.
                // This fixture's saved loaded rounds were unchanged by refill.
                engine_weapon_deactivate(old as *mut c_void, 1);
                raw::player::held_item(player.ptr()).write(new);
                engine_weapon_set_user(new as *mut c_void, player.ptr());
                raw::player::pending_item(player.ptr()).write(new);
                initialize(new as *mut c_void);
                engine_weapon_activate(new as *mut c_void, 1);
            })
        }
        .unwrap_or_else(|error| panic!("{error:?}"));
        let restored = owners.slots[1].as_ref().unwrap();
        assert_eq!(restored.snapshot_reserves().unwrap().counts(), &[0, 30]);
        assert_eq!(restored.weapon().ammunition(), Some(ammo()));
        // Saving the applied record produces identical canonical bytes.
        assert_eq!(
            History::capture(&world, |_, _, _| restored.snapshot_slot())
                .unwrap()
                .snapshot()
                .bytes(),
            saved.bytes()
        );
        // A stale selected binding cannot produce a plausible capture.
        unsafe { raw::weapon_core::ammo_kind(new as *mut c_void).write(7) };
        assert!(restored.snapshot_reserves().is_err());
        unsafe { raw::weapon_core::ammo_kind(new as *mut c_void).write(1) };
        drop(owners);
        recipient.original_restored(true);
    }

    #[test]
    fn snapshot_rejects_mismatched_supply_count_without_losing_the_resource() {
        let mut source = Fixture::new();
        let prepared = source.prepare(ammo()).unwrap();
        let (error, prepared) =
            match prepared.for_snapshot(snapshot_state(&[17], WeaponSlot::Second)) {
                Ok(_) => panic!("two-supply weapon accepted one supply"),
                Err(refused) => refused,
            };
        assert_eq!(error, InstallError::InvalidSupply);
        assert_eq!(prepared.ammunition(), Some(ammo()));
        assert_eq!(
            prepared.supply(AmmoKind::parse(1).unwrap()).unwrap().rounds,
            30
        );
        assert!(
            prepared
                .for_snapshot(snapshot_state(&[0, 17], WeaponSlot::Second))
                .is_ok()
        );
    }

    #[test]
    fn snapshot_attachment_preserves_native_transition_and_decoded_ammunition() {
        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        let prepared = source.prepare(ammo()).unwrap();
        let new = prepared.weapon.id();
        let old = recipient.original.weapon.as_ptr() as u32;
        let player = recipient.object();
        // Same-kit native removal retains the old held weapon and clears
        // pending. This must be accepted without applying live-pickup rules.
        unsafe { raw::player::pending_item(player.ptr()).write(0) };
        EVENTS.with_borrow_mut(Vec::clear);
        let snapshot = prepared
            .for_snapshot(snapshot_state(&[42, 17], WeaponSlot::Second))
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        let mut resources = [None, Some(snapshot)];
        let mut restored = unsafe {
            decode_pickup_slots(player, &mut resources, || {
                assert_eq!(*raw::player::held_item(player.ptr()), old);
                assert_eq!(*raw::player::pending_item(player.ptr()), 0);
                assert_eq!(recipient.registry[1], new);
                EVENTS.with_borrow(|events| assert_eq!(events, &[Event::Set(new + 0x10)]));
                // Model the native decoder's old/new transition, then its
                // weapon-state/ammo binding and later activation. The original
                // executable's transition itself is checked by Unicorn.
                engine_weapon_deactivate(old as *mut c_void, 1);
                raw::player::held_item(player.ptr()).write(new);
                engine_weapon_set_user(new as *mut c_void, player.ptr());
                raw::player::pending_item(player.ptr()).write(new);
                raw::weapon_core::magazine(new as *mut c_void).write(3);
                raw::weapon_core::ammo_kind(new as *mut c_void).write(0);
                initialize(new as *mut c_void);
                assert_eq!(*raw::item::count(source.ammo[0].as_mut_ptr().cast()), 42);
                assert_eq!(*raw::item::count(source.ammo[1].as_mut_ptr().cast()), 17);
                engine_weapon_activate(new as *mut c_void, 1);
                123
            })
        }
        .unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(restored.output, 123);
        assert!(resources.iter().all(Option::is_none));
        let owner = restored.slots[1].take().unwrap();
        assert_eq!(
            owner.weapon().ammunition(),
            DroppedAmmo::parse(30, 3, Chamber::Loaded, AmmoKind::parse(0).unwrap())
        );
        assert_eq!(
            owner
                .weapon()
                .supply(AmmoKind::parse(0).unwrap())
                .unwrap()
                .rounds,
            42
        );
        assert_eq!(
            owner
                .weapon()
                .supply(AmmoKind::parse(1).unwrap())
                .unwrap()
                .rounds,
            17
        );
        assert_eq!(owner.snapshot_reserves().unwrap().counts(), &[42, 17]);
        let detached = owner.restore();
        recipient.original_restored(true);
        assert_eq!(
            detached.supply(AmmoKind::parse(0).unwrap()).unwrap().rounds,
            42
        );
        assert_eq!(
            detached.supply(AmmoKind::parse(1).unwrap()).unwrap().rounds,
            17
        );
    }

    #[test]
    fn snapshot_restores_changed_original_after_native_deactivation_and_keeps_ammo_refs() {
        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        let player = recipient.object();
        let old = recipient.original.weapon.as_mut_ptr().cast();
        let prepared = source.prepare(ammo()).unwrap();
        let new = prepared.weapon.ptr();
        let mut state = snapshot_state(&[12, 0], WeaponSlot::Second);
        let mut bytes = [0; 20];
        bytes.copy_from_slice(state.original.bytes());
        bytes[..4].copy_from_slice(&0x3f40_0000u32.to_le_bytes());
        bytes[7..11].copy_from_slice(&12345u32.to_le_bytes());
        bytes[11..15].copy_from_slice(&67890u32.to_le_bytes());
        bytes[16] = 5;
        bytes[17] = 2;
        bytes[18] = 0; // restore another compatible ammo type
        bytes[19] = 0; // restore empty chamber
        state.original = weapon_pickup::Original::parse(&bytes).unwrap();
        let snapshot = prepared
            .for_snapshot(state)
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        let mut decoded = unsafe {
            decode_pickup_slots(player, &mut [None, Some(snapshot)], || {
                assert_eq!(*raw::weapon_core::magazine(old), 23);
                assert_eq!(*raw::weapon_core::user(old), player.id());
                engine_weapon_deactivate(old, 1);
                raw::player::held_item(player.ptr()).write(new as u32);
                engine_weapon_set_user(new, player.ptr());
                raw::player::pending_item(player.ptr()).write(new as u32);
                initialize(new);
                engine_weapon_activate(new, 1);
                // Original restoration must wait until this decoder returns.
                assert_eq!(*raw::weapon_core::magazine(old), 23);
            })
        }
        .unwrap_or_else(|error| panic!("{error:?}"));
        let owner = decoded.slots[1].take().unwrap();
        assert_eq!(owner.snapshot_slot().unwrap(), state);
        unsafe {
            assert_eq!(
                *raw::weapon_core::ammo(old),
                recipient.original.ammo[0].as_ptr() as u32
            );
            assert_eq!(
                *raw::unmanaged_resource::references(
                    recipient.original.ammo[0].as_mut_ptr().cast()
                ),
                2
            );
            assert_eq!(
                *raw::unmanaged_resource::references(
                    recipient.original.ammo[1].as_mut_ptr().cast()
                ),
                1
            );
        }
        drop(owner);
        unsafe {
            assert_eq!(*raw::player::held_item(player.ptr()), old as u32);
            assert_eq!(*raw::weapon_core::magazine(old), 5);
            assert_eq!(*raw::weapon_core::ammo_kind(old), 0);
        }
    }

    #[test]
    fn snapshot_rejects_invalid_original_template_and_roster_before_mutating() {
        for fault in 0..7 {
            let mut source = Fixture::new();
            let mut recipient = Recipient::new();
            let player = recipient.object();
            let prepared = source.prepare(ammo()).unwrap();
            let mut state = snapshot_state(&[0, 0], WeaponSlot::Second);
            let mut bytes = [0; 20];
            bytes.copy_from_slice(state.original.bytes());
            match fault {
                0 => bytes[4] = 1,            // another place, although it has a roster entry
                1 => bytes[5] = 1,            // wrong kit
                2 => bytes[6] = 7,            // wrong slot
                3 => bytes[16] = 31,          // beyond this weapon's capacity
                4 => bytes[18] = 2,           // valid protocol ordinal, not compatible here
                5 => recipient.roster[0] = 0, // missing native owner
                6 => {}                       // chamber shape doesn't match the original template
                _ => unreachable!(),
            }
            state.original =
                weapon_pickup::Original::parse(&bytes[..if fault == 6 { 19 } else { 20 }]).unwrap();
            let snapshot = prepared
                .for_snapshot(state)
                .unwrap_or_else(|(error, _)| panic!("{error:?}"));
            let mut resources = [None, Some(snapshot)];
            let before = recipient.original.inventory.clone();
            let weapon_before = recipient.original.weapon.clone();
            EVENTS.with_borrow_mut(Vec::clear);
            let result = unsafe {
                decode_pickup_slots::<()>(player, &mut resources, || {
                    panic!("decoded invalid original")
                })
            };
            assert!(result.is_err());
            assert!(resources[1].is_some());
            assert_eq!(recipient.original.inventory, before);
            assert_eq!(recipient.original.weapon, weapon_before);
            EVENTS.with_borrow(|events| assert!(events.is_empty()));
        }
    }

    #[test]
    fn snapshot_can_attach_a_stowed_weapon_without_activating_it() {
        let mut source = Fixture::new();
        let mut recipient = Recipient::new();
        let prepared = source.prepare(ammo()).unwrap();
        let new = prepared.weapon.id();
        let old = recipient.original.weapon.as_ptr() as u32;
        let player = recipient.object();
        unsafe {
            raw::player::held_item(player.ptr()).write(0);
            raw::player::pending_item(player.ptr()).write(0);
            raw::player::alive(player.ptr()).write(0);
            raw::weapon_core::user(old as *mut c_void).write(0);
        }
        EVENTS.with_borrow_mut(Vec::clear);
        let snapshot = prepared
            .for_snapshot(snapshot_state(&[0, 0], WeaponSlot::Second))
            .unwrap_or_else(|(error, _)| panic!("{error:?}"));
        let owners = unsafe {
            decode_pickup_slots(player, &mut [None, Some(snapshot)], || {
                assert_eq!(*raw::player::held_item(player.ptr()), 0);
                initialize(new as *mut c_void); // native inventory reader binds even stowed ammo
            })
        }
        .unwrap_or_else(|error| panic!("{error:?}"));
        EVENTS.with_borrow(|events| {
            assert_eq!(
                events,
                &[
                    Event::Set(new + 0x10),
                    Event::Initialize(new),
                    Event::RestoreOriginal(old)
                ]
            )
        });
        assert_eq!(
            owners.slots[1]
                .as_ref()
                .unwrap()
                .snapshot_reserves()
                .unwrap()
                .counts(),
            &[0, 0]
        );
        drop(owners);
        unsafe {
            assert_eq!(*raw::player::held_item(player.ptr()), 0);
            assert_eq!(*raw::weapon_core::user(old as *mut c_void), 0);
        }
        assert_eq!(recipient.registry[1], old);
    }

    #[test]
    fn snapshot_refuses_the_second_slot_before_attaching_the_first_or_decoding() {
        let mut first = Fixture::new();
        let mut second = Fixture::new();
        let mut recipient = Recipient::new();
        let player = recipient.object();
        unsafe {
            let inv = recipient.original.inventory.as_mut_ptr().cast();
            let item = raw::weapon_core::item(recipient.original.weapon.as_mut_ptr().cast()).cast();
            raw::inventory::slots(inv).add(7).write(item as u32);
            raw::inventory::slots(inv).add(10).write(0);
            raw::inventory_item::slot(item).write(7);
        }
        let before = recipient.original.inventory.clone();
        EVENTS.with_borrow_mut(Vec::clear);
        let mut decoded = false;
        let snapshot = |weapon: PreparedWeapon, slot| {
            weapon
                .for_snapshot(snapshot_state(&[42, 17], slot))
                .unwrap_or_else(|(error, _)| panic!("{error:?}"))
        };
        let mut resources = [
            Some(snapshot(first.prepare(ammo()).unwrap(), WeaponSlot::First)),
            Some(snapshot(
                second.prepare(ammo()).unwrap(),
                WeaponSlot::Second,
            )),
        ];
        let result = unsafe { decode_pickup_slots(player, &mut resources, || decoded = true) };
        assert!(matches!(result, Err(InstallError::NotHeld)));
        assert!(resources.iter().all(Option::is_some));
        assert!(!decoded);
        for resource in resources.iter().flatten() {
            assert_eq!(
                resource
                    .weapon
                    .supplies
                    .iter()
                    .flatten()
                    .map(|supply| unsafe { read(raw::item::count(supply.stack.ptr())) })
                    .collect::<Vec<_>>(),
                vec![0, 30]
            );
        }
        assert_eq!(recipient.original.inventory, before);
        assert_eq!(
            recipient.registry[1],
            recipient.original.weapon.as_ptr() as u32
        );
        EVENTS.with_borrow(|events| assert!(events.is_empty()));
    }
}
