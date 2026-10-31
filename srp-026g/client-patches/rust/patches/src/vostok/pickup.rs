//! A borrowed gun's install and restore (patch 83's engine side,
//! docs/notes/weapon_pickup.md "How a pickup is installed"): the native
//! moves only, no policy.
//!
//! - [`install`] swaps the picker's held own gun (its weapon slot) with a
//!   cooked gun (its slot of the cooked inventory), moves the gun's bound
//!   ammunition into free ammunition slots of the picker's inventory and
//!   rebinds the gun to them, binds the gun to the world (the core, the own
//!   gun's registry id and the core's entry for it, the bullet manager), and
//!   either replays the weapon switch (deactivate, forget, held = pending,
//!   set_user, initialize, the magazine, activate) or, for a loaded world's
//!   record, leaves the held object to the native reader (0x2DB62B
//!   deactivates the old and set_users the new when they differ).
//! - [`restore`] puts everything back as it was cooked and the own gun in
//!   its slot and its registry entry; the held object is left to the caller
//!   (remove deactivates it, the reader switches it).
//! - every check is made before the first write: a refusal changes nothing.
//! - the crate: [`crate_view`], a copy of the inventory with the own gun in
//!   its place and the moved ammunition's slots empty, what the native
//!   resupply reads; [`refill`] and [`reserve_low`] the borrowed reserve.
//!
//! Every reference count change is the native set_item's/remove_item's; a
//! moved item is held by one more reference across the move.

use core::sync::atomic::{AtomicU32, Ordering};

pub use weapon_pickup::{Ammo, WeaponSlot};

use super::gun_cook::{self, CookId};
use super::world::{read, Player};
use super::{Context, Lock, Thread};
use crate::raw;

/// The slots a weapon's ammunition is bound to (cook 0x2C7855..0x2C79E5).
const AMMO: [u8; 8] = [8, 9, 11, 12, 19, 20, 21, 22];
const SLOTS: u32 = 23;
/// Bound ammunition stacks a gun can have (its +5B0h, a cooked gun's from the
/// eight slots).
const MOVES: usize = 8;

/// Why a gun was not installed or restored (nothing was changed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The player has no inventory or no world core.
    NoPlayerState,
    /// The own gun is not a client weapon in its slot, held and pending.
    NotInHands,
    /// The cooked inventory is gone, or its slot holds no client weapon.
    NoCookedGun,
    /// A bound ammunition slot of the cooked gun is empty or out of range.
    BadAmmo,
    /// Not enough free ammunition slots in the picker's inventory.
    NoAmmoSlot,
    /// What is in the slots is not what the install put there.
    Changed,
}

impl Refusal {
    pub fn text(self) -> &'static str {
        match self {
            Refusal::NoPlayerState => "the player has no inventory or no world core",
            Refusal::NotInHands => "its own gun is not held in its slot",
            Refusal::NoCookedGun => "the cooked gun is gone or not a weapon",
            Refusal::BadAmmo => "a bound ammunition slot of the cooked gun is empty",
            Refusal::NoAmmoSlot => "no free ammunition slot for the gun's ammunition",
            Refusal::Changed => "the slots no longer hold what the install put there",
        }
    }
}

/// One ammunition stack moved: from the cooked inventory's slot to the
/// picker's, its world before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Moved {
    item: u32,
    from: u8,
    to: u8,
    world: u32,
}

/// An installed borrowed gun, as values (compared, never read through):
/// what [`restore`] puts back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Installed {
    cook: CookId,
    own: WeaponSlot,
    slot: WeaponSlot,
    own_gun: u32,
    gun: u32,
    moved: [Option<Moved>; MOVES],
    /// The cooked gun's own core, registry id, world (+258h) and bullet
    /// manager, put back at the restore.
    saved: (u32, u8, u32, u32),
}

impl Installed {
    pub fn cook(&self) -> CookId {
        self.cook
    }
}

/// How an install treats the held object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The pickup: the switch replayed, the magazine and reserve granted
    /// ([`Ammo::grant`]) from the donor's gun's magazine and chamber (a full
    /// magazine when its gun is not there).
    Switch(Option<(u8, bool)>),
    /// A loaded world's record (a rollback, a late join): the native reader
    /// switches the held object; the snapshot gives the magazine and reserve.
    Load,
}

/// A pickup site's player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Save,
    Load,
    Remove,
    CanUse,
    UseHint,
    Resupply,
    ClientResupply,
    Destroy,
}

pub fn player<'t>(ctx: &'t Context<'_>, site: Site) -> Option<Player<'t>> {
    // SAFETY: each site's typed register, its live player on the game's thread.
    let raw = unsafe {
        match site {
            Site::Save => raw::site::pickup_save::player(ctx),
            Site::Load => raw::site::pickup_load::player(ctx),
            Site::Remove => raw::site::pickup_remove::player(ctx),
            Site::CanUse => raw::site::pickup_can_use::player(ctx),
            Site::UseHint => raw::site::pickup_use_hint::player(ctx),
            Site::Resupply => raw::site::pickup_resupply::player(ctx),
            Site::ClientResupply => raw::site::pickup_client_resupply::player(ctx),
            Site::Destroy => raw::site::player_destroy::player(ctx),
        }
    };
    Player::of(ctx, raw)
}

/// Writes the record's bytes at `pickup.save` (the native writer, as the
/// adjacent fields).
pub fn write_record(ctx: &Context<'_>, bytes: &[u8; weapon_pickup::Record::BYTES]) {
    // SAFETY: base_player::serialize's live writer (the site's ESI).
    if let Some(writer) = unsafe { raw::site::pickup_save::writer(ctx) } {
        super::carry_runtime::write_bytes(writer, bytes);
    }
}

/// Reads the record's bytes at `pickup.load` (None: the state is shorter).
pub fn read_record(ctx: &Context<'_>) -> Option<[u8; weapon_pickup::Record::BYTES]> {
    // SAFETY: base_player::deserialize's live reader (the site's ESI).
    super::carry_runtime::read_bytes(unsafe { raw::site::pickup_load::reader(ctx) }?)
}

/// Whether `pickup.remove`'s call is the death/respawn removal (flag 1):
/// held deactivated, the inventory unloaded into the profile.
pub fn removing(ctx: &Context<'_>) -> bool {
    // SAFETY: base_player::remove's entry frame, its first argument's low byte.
    unsafe { raw::site::pickup_remove::frame(ctx).is_some_and(|frame| read(frame.unload()) != 0) }
}

/// Sets EAX or ECX (the inventory the native resupply is about to be
/// handed) to `view`.
pub fn hand_view(ctx: &mut Context<'_>, site: Site, view: View) {
    let reg = match site {
        Site::CanUse | Site::UseHint => super::Reg::Ecx,
        _ => super::Reg::Eax,
    };
    ctx.set_reg(reg, view.0);
}

impl Player<'_> {
    /// The world core's address: the match the records are of.
    pub fn world_key(&self) -> Option<u32> {
        // SAFETY: the live player's world core pointer, compared only.
        unsafe { self.this.world_core().map(|core| core.address()) }
    }

    /// The input's actions as set_input copied them (+330h), before patch
    /// 78 filters them.
    pub fn input_actions(&self) -> u32 {
        // SAFETY: the live player's input mask.
        unsafe { read(self.this.actions()) }
    }

    /// The weapon slot whose own client weapon is in its hands: held and
    /// pending (+128h, +12Ch, no switch under way) are one client weapon
    /// whose inventory item is in slot 7 or 10.
    pub fn gun_in_hands(&self) -> Option<WeaponSlot> {
        // SAFETY: the live player, its held and pending objects, its inventory.
        unsafe {
            let held = self.this.held_item()?;
            if self.this.pending_item() != Some(held) || read(held.vtable()) != raw::image::WEAPON_VTABLE {
                return None;
            }
            let item = held.inventory_item();
            let slots = self.this.inventory()?.slots();
            [WeaponSlot::First, WeaponSlot::Second].into_iter()
                .find(|slot| slots.at(usize::from(slot.slot())).item() == Some(item))
        }
    }

    /// The magazine and the chambered round of its `slot`'s client weapon
    /// (a donor's, as it died), if one is there.
    pub fn magazine(&self, slot: WeaponSlot) -> Option<(u8, bool)> {
        // SAFETY: the live player's inventory slot, its item read as a
        // weapon only after its active object's vtable says it is one.
        unsafe {
            let item = self.this.inventory()?.slots().at(usize::from(slot.slot())).item()?;
            let gun = item.active_object();
            if read(gun.vtable()) != raw::image::WEAPON_VTABLE {
                return None;
            }
            let w = gun.weapon();
            Some((read(w.loaded()), read(w.has_chamber()) != 0 && read(w.chamber()) != 0))
        }
    }
}

/// The cooked inventory's `slot` gun: its inventory item and active object,
/// when a client weapon.
///
/// # Safety
/// `inventory` is a live cooked inventory (its ticket made).
unsafe fn cooked_gun(inventory: raw::Inventory, slot: WeaponSlot) -> Option<(raw::Item, raw::Item)> {
    unsafe {
        let item = inventory.slots().at(usize::from(slot.slot())).item()?;
        let gun = item.active_object();
        (read(gun.vtable()) == raw::image::WEAPON_VTABLE).then_some((item, gun))
    }
}

/// Whether the cooked gun is of the held own gun's kind (both two-handed
/// or both not, +5E9h). None: either is missing.
pub fn same_kind(player: &Player<'_>, own: WeaponSlot, cook: CookId, slot: WeaponSlot) -> Option<bool> {
    let inventory = gun_cook::inventory(player.thread, cook)?;
    // SAFETY: the live player's inventory and the made cooked inventory.
    unsafe {
        let (_, gun) = cooked_gun(inventory, slot)?;
        let item = player.this.inventory()?.slots().at(usize::from(own.slot())).item()?;
        let own_gun = item.active_object();
        if read(own_gun.vtable()) != raw::image::WEAPON_VTABLE {
            return None;
        }
        Some((read(gun.weapon().double_handed()) != 0) == (read(own_gun.weapon().double_handed()) != 0))
    }
}

/// One reference more to `item` for as long as it lives (a move's bridge).
struct Hold(raw::Item);

impl Hold {
    /// # Safety
    /// `item` is live (an inventory slot holds it).
    unsafe fn new(item: raw::Item) -> Hold {
        unsafe { AtomicU32::from_ptr(item.references()).fetch_add(1, Ordering::AcqRel) };
        Hold(item)
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        // SAFETY: the reference `new` added, given back once; the native
        // release at the last.
        unsafe {
            let count = self.0.references();
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_inventory_item(Some(self.0), count);
            }
        }
    }
}

/// inventory::set_item: `item` into `slot` (its reference added, the old
/// one's released, its owner set).
///
/// # Safety
/// `inventory` is live, `item` live and held across the call.
unsafe fn set(inventory: raw::Inventory, slot: u8, item: raw::Item) {
    let mut reference = Some(item);
    unsafe { raw::engine_inventory_set(Some(inventory), u32::from(slot), &mut reference) };
}

/// Installs the cooked gun of `cook` (its `slot`) in place of `player`'s own
/// gun of `own`. Refused before any write when anything is not as expected.
pub fn install(player: &Player<'_>, own: WeaponSlot, cook: CookId, slot: WeaponSlot, mode: Mode)
               -> Result<Installed, Refusal> {
    let cooked = gun_cook::inventory(player.thread, cook).ok_or(Refusal::NoCookedGun)?;
    // SAFETY: the live player (its hook's), its inventory and world core;
    // the made cooked inventory (its ticket holds a reference); items read
    // as weapons only after their vtables say so; every slot index below 23.
    unsafe {
        let p = player.this;
        let (inventory, core) = (p.inventory().ok_or(Refusal::NoPlayerState)?, p.world_core().ok_or(Refusal::NoPlayerState)?);
        let own_item = inventory.slots().at(usize::from(own.slot())).item().ok_or(Refusal::NotInHands)?;
        let own_gun = own_item.active_object();
        if read(own_gun.vtable()) != raw::image::WEAPON_VTABLE {
            return Err(Refusal::NotInHands);
        }
        if matches!(mode, Mode::Switch(_)) && (p.held_item() != Some(own_gun) || p.pending_item() != Some(own_gun)) {
            return Err(Refusal::NotInHands);
        }
        let (gun_item, gun) = cooked_gun(cooked, slot).ok_or(Refusal::NoCookedGun)?;
        let (w, ow) = (gun.weapon(), own_gun.weapon());
        // The ammunition: each bound slot's stack to a free slot of ours.
        let mut moved = [None; MOVES];
        let count = usize::from(read(w.ammo_count()));
        if count > MOVES {
            return Err(Refusal::BadAmmo);
        }
        let indices = w.ammo_slots();
        let mut free = AMMO.into_iter().filter(|&s| inventory.slots().at(usize::from(s)).item().is_none());
        for (n, room) in moved.iter_mut().enumerate().take(count) {
            let from = read(indices.ok_or(Refusal::BadAmmo)?.at(n).slot());
            if from >= SLOTS {
                return Err(Refusal::BadAmmo);
            }
            let item = cooked.slots().at(from as usize).item().ok_or(Refusal::BadAmmo)?;
            let to = free.next().ok_or(Refusal::NoAmmoSlot)?;
            *room = Some(Moved { item: item.address(), from: from as u8, to,
                                 world: item.world().map_or(0, |c| c.address()) });
        }
        let id = read(ow.registry_id());
        let objects = core.objects().ok_or(Refusal::NoPlayerState)?;
        let saved = (w.core().map_or(0, |c| c.address()), read(w.registry_id()),
                     gun_item.world().map_or(0, |c| c.address()), read(w.bullet_manager()));
        // Nothing written before here.
        w.set_core(Some(core));
        *w.registry_id() = id;
        gun_item.set_world(Some(core));
        *w.bullet_manager() = read(ow.bullet_manager());
        *w.load_on_initialize() = 0;
        for (n, m) in moved.iter().enumerate() {
            let Some(m) = m else { continue };
            let Some(item) = cooked.slots().at(usize::from(m.from)).item() else { continue };
            let _hold = Hold::new(item);
            raw::engine_inventory_remove(Some(cooked), u32::from(m.from));
            set(inventory, m.to, item);
            item.set_world(Some(core));
            if let Some(indices) = indices {
                *indices.at(n).slot() = u32::from(m.to);
            }
        }
        {
            let (_own, _gun) = (Hold::new(own_item), Hold::new(gun_item));
            set(inventory, own.slot(), gun_item);
            set(cooked, slot.slot(), own_item);
        }
        objects.at(usize::from(id)).set_item(Some(gun));
        match mode {
            Mode::Switch(ammo) => {
                raw::engine_weapon_deactivate(Some(own_gun), 1);
                raw::engine_forget_active_object(p.animation().cast(), Some(own_gun));
                p.set_held_item(Some(gun));
                p.set_pending_item(Some(gun));
                raw::engine_weapon_set_user(Some(gun), Some(p));
                raw::engine_weapon_initialize(Some(gun));
                let capacity = read(w.capacity());
                let (loaded, chamber) = ammo.unwrap_or((capacity, false));
                grant(w, Ammo::grant(capacity, loaded, chamber));
                raw::engine_weapon_activate(Some(gun), 1);
            }
            Mode::Load => {
                raw::engine_weapon_set_user(Some(gun), Some(p));
                raw::engine_weapon_initialize(Some(gun));
            }
        }
        Ok(Installed { cook, own, slot, own_gun: own_gun.address(), gun: gun.address(), moved, saved })
    }
}

/// The granted magazine (and chamber) and reserve: the retained stack holds
/// the reserve, any other bound stack none.
///
/// # Safety
/// `w` is the installed live gun, initialized (its +538h bound).
unsafe fn grant(w: raw::Weapon, ammo: Ammo) {
    unsafe {
        *w.loaded() = ammo.loaded.min(read(w.capacity()));
        if read(w.has_chamber()) != 0 {
            *w.chamber() = u8::from(ammo.chamber);
        }
        let retained = w.retained_ammo();
        let count = usize::from(read(w.ammo_count())).min(MOVES);
        let Some(indices) = w.ammo_slots() else { return };
        let Some(inventory) = retained.and_then(|r| r.inventory()) else { return };
        for n in 0..count {
            let slot = read(indices.at(n).slot());
            if slot >= SLOTS {
                continue;
            }
            if let Some(stack) = inventory.slots().at(slot as usize).item() {
                *stack.count() = if Some(stack) == retained { ammo.reserve } else { 0 };
            }
        }
        *inventory.weight_dirty() = 1;
    }
}

/// Puts `installed` back: the own gun in its slot and registry entry, the
/// cooked gun and its ammunition back in the cooked inventory as cooked.
/// The held object is left as it is (the caller's: remove deactivates it,
/// the native reader switches it). Refused before any write when the slots
/// do not hold what the install put there.
pub fn restore(player: &Player<'_>, installed: &Installed) -> Result<(), Refusal> {
    let cooked = gun_cook::inventory(player.thread, installed.cook).ok_or(Refusal::NoCookedGun)?;
    // SAFETY: as install; every item is compared with what the install
    // recorded before it is used.
    unsafe {
        let p = player.this;
        let (inventory, core) = (p.inventory().ok_or(Refusal::NoPlayerState)?, p.world_core().ok_or(Refusal::NoPlayerState)?);
        let gun_item = inventory.slots().at(usize::from(installed.own.slot())).item().ok_or(Refusal::Changed)?;
        let own_item = cooked.slots().at(usize::from(installed.slot.slot())).item().ok_or(Refusal::Changed)?;
        let (gun, own_gun) = (gun_item.active_object(), own_item.active_object());
        if gun.address() != installed.gun || own_gun.address() != installed.own_gun {
            return Err(Refusal::Changed);
        }
        for m in installed.moved.iter().flatten() {
            if inventory.slots().at(usize::from(m.to)).item().map(|i| i.address()) != Some(m.item)
                || cooked.slots().at(usize::from(m.from)).item().is_some() {
                return Err(Refusal::Changed);
            }
        }
        let objects = core.objects().ok_or(Refusal::NoPlayerState)?;
        let w = gun.weapon();
        let indices = w.ammo_slots();
        // Nothing written before here.
        {
            let (_own, _gun) = (Hold::new(own_item), Hold::new(gun_item));
            set(inventory, installed.own.slot(), own_item);
            set(cooked, installed.slot.slot(), gun_item);
        }
        for (n, m) in installed.moved.iter().enumerate() {
            let Some(m) = m else { continue };
            let Some(item) = inventory.slots().at(usize::from(m.to)).item() else { continue };
            let _hold = Hold::new(item);
            raw::engine_inventory_remove(Some(inventory), u32::from(m.to));
            set(cooked, m.from, item);
            item.set_world(world_of(m.world, core));
            if let Some(indices) = indices {
                *indices.at(n).slot() = u32::from(m.from);
            }
        }
        objects.at(usize::from(read(own_gun.weapon().registry_id()))).set_item(Some(own_gun));
        let (saved_core, saved_id, saved_world, saved_bullets) = installed.saved;
        w.set_core(world_of(saved_core, core));
        *w.registry_id() = saved_id;
        gun_item.set_world(world_of(saved_world, core));
        *w.bullet_manager() = saved_bullets;
        Ok(())
    }
}

/// A saved world pointer back as a handle: none, or this core (the only
/// world a cooked gun's items can have been given).
fn world_of(saved: u32, core: raw::CoreWorld) -> Option<raw::CoreWorld> {
    (saved == core.address()).then_some(core)
}

/// The installed gun's retained reserve and its magazine's capacity.
///
/// # Safety
/// `player` is live.
unsafe fn reserve(player: &Player<'_>, installed: &Installed) -> Option<(raw::Item, u16, u8)> {
    unsafe {
        let item = player.this.inventory()?.slots().at(usize::from(installed.own.slot())).item()?;
        let gun = item.active_object();
        if gun.address() != installed.gun {
            return None;
        }
        let w = gun.weapon();
        let stack = w.retained_ammo()?;
        Some((stack, read(stack.count()), read(w.capacity())))
    }
}

/// Whether the borrowed gun's reserve is below one magazine.
pub fn reserve_low(player: &Player<'_>, installed: &Installed) -> bool {
    // SAFETY: the live player.
    unsafe { reserve(player, installed) }.is_some_and(|(_, count, capacity)| count < u16::from(capacity))
}

/// The crate's use: the borrowed reserve to one magazine, free, and the
/// real inventory's weight marked to be summed again (the native resupply
/// marks its view's).
pub fn refill(player: &Player<'_>, installed: &Installed) {
    // SAFETY: the live player's inventory, its installed gun's retained stack.
    unsafe {
        if let Some((stack, count, capacity)) = reserve(player, installed)
            && count < u16::from(capacity) {
                *stack.count() = u16::from(capacity);
            }
        if let Some(inventory) = player.this.inventory() {
            *inventory.weight_dirty() = 1;
        }
    }
}

/// The crate's inventory view (its address, for the native resupply).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View(u32);

static VIEW: Lock<raw::InventoryStorage> = Lock::new(raw::InventoryStorage::zeroed());

/// A copy of `player`'s inventory (188h) with its own gun in its slot and
/// the moved ammunition's slots empty: the loadout the profile names, what
/// resupply_from_profile and check_resupply read (slots +10Ch..+167h; they
/// write +184h only).
pub fn crate_view(player: &Player<'_>, installed: &Installed, thread: &Thread) -> Option<View> {
    let cooked = gun_cook::inventory(thread, installed.cook)?;
    // SAFETY: the live inventory, 188h bytes of it copied into our room; the
    // own gun's item (in the cooked inventory) compared before it is used.
    unsafe {
        let inventory = player.this.inventory()?;
        let own_item = cooked.slots().at(usize::from(installed.slot.slot())).item()?;
        if own_item.active_object().address() != installed.own_gun {
            return None;
        }
        Some(VIEW.with(thread, |room| {
            let view = room.handle();
            core::ptr::copy_nonoverlapping(inventory.address() as usize as *const u8, view.address() as usize as *mut u8,
                                           raw::Inventory::SIZE);
            view.slots().at(usize::from(installed.own.slot())).set_item(Some(own_item));
            for m in installed.moved.iter().flatten() {
                view.slots().at(usize::from(m.to)).set_item(None);
            }
            View(view.address())
        }))
    }
}
