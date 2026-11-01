//! A borrowed gun's install and restore (patch 85's engine side,
//! docs/notes/weapon_pickup.md "How a pickup is installed"): the native
//! moves only, no policy.
//!
//! - [`install`] swaps the picker's held own gun (its weapon slot) with a
//!   cooked gun (its slot of the cooked inventory), moves the gun's bound
//!   ammunition into the own gun's ammunition slots (the own stacks parked
//!   in the cooked inventory), then free ones, and rebinds the gun to them
//!   (its selected stack first), binds the gun to the world (the core, the own
//!   gun's registry id and the core's entry for it, the bullet manager), and
//!   either replays the weapon switch (deactivate, forget, held = pending,
//!   set_user, initialize, the magazine, activate) or, for a loaded world's
//!   record, leaves the held object to the native reader (0x2DB62B
//!   deactivates the old and set_users the new when they differ).
//! - [`restore`] puts everything back as it was cooked and the own gun in
//!   its slot and its registry entry; the held object is left to the caller
//!   (remove deactivates it, the reader switches it).
//! - every check is made before the first write: a refusal changes nothing.
//! - the crate: [`crate_view`], a copy of the inventory with the own gun and
//!   the own parked stacks in their places, what the native resupply reads;
//!   [`refill`] and [`reserve_low`] the borrowed stacks (the native refill's
//!   rule, in bags: the slot's allowance for the picker's own ammunition as
//!   rounds of the borrowed one).
//!
//! Every reference count change is the native set_item's/remove_item's; a
//! moved item is held by one more reference across the move.

use core::ffi::CStr;
use core::sync::atomic::{AtomicU32, Ordering};

pub use weapon_pickup::{Ammo, AmmoBags, CarryRule, Handedness, Refill, WeaponSlot};

use super::config::{ConfigError, ConfigValue};

use super::gun_cook::{self, CookId};
use super::world::{read, Player};
use super::{Context, Lock, Thread};
use crate::raw;

/// The slots a weapon's ammunition is bound to (cook 0x2C7855..0x2C79E5).
const AMMO: [u8; 8] = [8, 9, 11, 12, 19, 20, 21, 22];
const SLOTS: u32 = 23;
/// A profile kit slot naming no item (translate_query 0x2C7301..0x2C730C).
const NO_ITEM: u16 = 0xFFFF;
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
    /// The picker's own stack that was in `to`, parked in the cooked
    /// inventory's `from` until the restore (the owner: "your ammo is
    /// replaced").
    displaced: Option<u32>,
}

/// The cooked gun's ammunition binding as cooked (+5ACh's indices, +5B0h,
/// +5B1h), put back at the restore: the install binds the stacks it placed,
/// the selected one first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Binding {
    indices: [u32; MOVES],
    count: u8,
    selected: u8,
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
    cooked: Binding,
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

/// The cooked gun's handedness (+5E9h). None: it is missing.
pub fn handedness(thread: &Thread, cook: CookId, slot: WeaponSlot) -> Option<Handedness> {
    let inventory = gun_cook::inventory(thread, cook)?;
    // SAFETY: the made cooked inventory, its gun read as a weapon after its vtable.
    unsafe {
        let (_, gun) = cooked_gun(inventory, slot)?;
        Some(Handedness::from_double_handed(read(gun.weapon().double_handed()) != 0))
    }
}

/// The client weapon in `inventory`'s `slot`, if one is there.
///
/// # Safety
/// `inventory` is live.
unsafe fn weapon_in(inventory: raw::Inventory, slot: WeaponSlot) -> Option<raw::Item> {
    unsafe {
        let gun = inventory.slots().at(usize::from(slot.slot())).item()?.active_object();
        (read(gun.vtable()) == raw::image::WEAPON_VTABLE).then_some(gun)
    }
}

impl Player<'_> {
    /// The handedness of its `slot`'s client weapon, if one is there.
    pub fn weapon_handedness(&self, slot: WeaponSlot) -> Option<Handedness> {
        // SAFETY: the live player's inventory, its gun read after its vtable.
        unsafe {
            let gun = weapon_in(self.this.inventory()?, slot)?;
            Some(Handedness::from_double_handed(read(gun.weapon().double_handed()) != 0))
        }
    }

    /// Its current kit's carry rule: modifiers 64 (`disable_second_weapon`)
    /// and 25 (`use_two_double_handed_weapons`) of the kit's modifiers
    /// (profile +0F4h + kit x 704h), summed by the native apply_modifier
    /// (both additive: of 0 with weight 1, the sum). None: no profile.
    pub fn carry_rule(&self) -> Option<CarryRule> {
        // SAFETY: the live player's profile and current kit (+115h, below 3
        // for a match player: checked), the kit's modifiers container.
        unsafe {
            let kit = usize::from(read(self.this.kit()));
            if kit >= 3 {
                return None;
            }
            let modifiers = Some(self.this.profile()?.kits().at(kit).modifiers());
            let disabled = raw::engine_apply_modifier(modifiers, 64, 0.0, 1.0);
            let two = raw::engine_apply_modifier(modifiers, 25, 0.0, 1.0);
            Some(CarryRule::from_modifiers(disabled, two))
        }
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
        // The ammunition (the owner: "your ammo is replaced"): the gun's
        // bound stacks, its selected one first, each into a slot the own
        // gun's ammunition is in (that stack parked in the cooked slot it
        // came from; not a slot the other weapon binds too), then into a free
        // ammunition slot; a stack with no room left stays cooked and the gun
        // binds the ones placed.
        let cooked_binding = binding(w).ok_or(Refusal::BadAmmo)?;
        let count = usize::from(cooked_binding.count);
        let selected = usize::from(cooked_binding.selected);
        let order = |k: usize| match k {
            0 => selected,
            k if k <= selected => k - 1,
            k => k,
        };
        let mut targets = [0u8; MOVES];
        let mut targets_len = 0;
        let other = weapon_in(inventory, own.other()).and_then(|g| binding(g.weapon()));
        let candidates = binding(ow).into_iter().flat_map(|b| b.indices.into_iter().take(usize::from(b.count)))
            .filter(|&slot| AMMO.iter().any(|&a| u32::from(a) == slot))
            .filter(|&slot| !other.is_some_and(|o| o.indices[..usize::from(o.count)].contains(&slot)))
            .map(|slot| slot as u8)
            .chain(AMMO.into_iter().filter(|&slot| inventory.slots().at(usize::from(slot)).item().is_none()));
        for slot in candidates {
            if targets_len < MOVES && !targets[..targets_len].contains(&slot) {
                targets[targets_len] = slot;
                targets_len += 1;
            }
        }
        let placed = count.min(targets_len);
        let mut moved = [None; MOVES];
        for (k, room) in moved.iter_mut().enumerate().take(placed) {
            let from = cooked_binding.indices[order(k)];
            if from >= SLOTS {
                return Err(Refusal::BadAmmo);
            }
            let item = cooked.slots().at(from as usize).item().ok_or(Refusal::BadAmmo)?;
            let to = targets[k];
            let displaced = inventory.slots().at(usize::from(to)).item().map(|i| i.address());
            *room = Some(Moved { item: item.address(), from: from as u8, to,
                                 world: item.world().map_or(0, |c| c.address()), displaced });
        }
        let indices = w.ammo_slots();
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
        for (k, m) in moved.iter().enumerate() {
            let Some(m) = m else { continue };
            let Some(item) = cooked.slots().at(usize::from(m.from)).item() else { continue };
            let _hold = Hold::new(item);
            match inventory.slots().at(usize::from(m.to)).item() {
                Some(own) => {
                    let _own = Hold::new(own);
                    set(inventory, m.to, item);
                    set(cooked, m.from, own);
                }
                None => {
                    raw::engine_inventory_remove(Some(cooked), u32::from(m.from));
                    set(inventory, m.to, item);
                }
            }
            item.set_world(Some(core));
            if let Some(indices) = indices {
                *indices.at(k).slot() = u32::from(m.to);
            }
        }
        *w.ammo_count() = placed as u8;
        *w.ammo_selected() = 0;
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
        Ok(Installed { cook, own, slot, own_gun: own_gun.address(), gun: gun.address(), moved, saved,
                       cooked: cooked_binding })
    }
}

/// A weapon's ammunition binding (+5ACh's first +5B0h indices, +5B1h);
/// None when it binds more than [`MOVES`], or the selected one is past them.
///
/// # Safety
/// `w` is a live weapon.
unsafe fn binding(w: raw::Weapon) -> Option<Binding> {
    unsafe {
        let (count, selected) = (read(w.ammo_count()), read(w.ammo_selected()));
        if usize::from(count) > MOVES || (count > 0 && selected >= count) {
            return None;
        }
        let mut indices = [0u32; MOVES];
        if count > 0 {
            let at = w.ammo_slots()?;
            for (n, index) in indices.iter_mut().enumerate().take(usize::from(count)) {
                *index = read(at.at(n).slot());
            }
        }
        Some(Binding { indices, count, selected })
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
                || cooked.slots().at(usize::from(m.from)).item().map(|i| i.address()) != m.displaced {
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
        for m in installed.moved.iter().flatten() {
            let Some(item) = inventory.slots().at(usize::from(m.to)).item() else { continue };
            let _hold = Hold::new(item);
            match cooked.slots().at(usize::from(m.from)).item() {
                Some(own) => {
                    let _own = Hold::new(own);
                    set(cooked, m.from, item);
                    set(inventory, m.to, own);
                }
                None => {
                    raw::engine_inventory_remove(Some(inventory), u32::from(m.to));
                    set(cooked, m.from, item);
                }
            }
            item.set_world(world_of(m.world, core));
        }
        let binding = installed.cooked;
        if let Some(indices) = indices {
            for (n, &index) in binding.indices.iter().enumerate().take(usize::from(binding.count)) {
                *indices.at(n).slot() = index;
            }
        }
        *w.ammo_count() = binding.count;
        *w.ammo_selected() = binding.selected;
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

/// An ammunition's id and its config in bags, as the lobby reads them
/// (0x289EB2..0x289EF3: its dictionary entry's config,
/// parameters/clip_size and parameters/required_bags, integers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoKind {
    pub dict_id: u16,
    /// None: the config has no usable clip_size and required_bags.
    pub bags: Option<AmmoBags>,
}

/// `stack`'s [`AmmoKind`]; None without a dictionary entry or config.
///
/// # Safety
/// `stack` is a live inventory item.
unsafe fn ammo_kind(stack: raw::Item) -> Option<AmmoKind> {
    unsafe {
        let entry = stack.entry()?;
        let parameters = || -> Result<AmmoBags, ConfigError> {
            let root = entry.config().and_then(|c| c.root()).ok_or(ConfigError::Missing)?;
            let parameters = ConfigValue::read(root)?.table()?.required(c"parameters")?.table()?;
            let integer = |key: &CStr| parameters.required(key).and_then(ConfigValue::integer);
            AmmoBags::parse(integer(c"clip_size")?, integer(c"required_bags")?).ok_or(ConfigError::WrongType)
        };
        Some(AmmoKind { dict_id: read(entry.dict_id()), bags: parameters().ok() })
    }
}

/// One borrowed stack at the crate: its slot, its ammunition, its count, the
/// slot's allowance in the picker's kit (the rounds and the kind of its own
/// ammunition there, which the native refill fills it to), the gun's
/// magazine and chamber when it is the gun's selected stack, and the
/// refill (none: its ammunition's config has no bags).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackRefill {
    pub slot: u8,
    pub ammo: AmmoKind,
    pub before: u16,
    pub allowance: Option<(u16, AmmoKind)>,
    pub held: Option<(u8, bool)>,
    pub refill: Option<Refill>,
}

impl StackRefill {
    /// The count the crate leaves the stack at.
    pub fn after(&self) -> u16 {
        self.refill.map_or(self.before, |r| r.target)
    }
}

/// The borrowed stacks the crate refills, each in a slot of `MOVES`.
pub type StackRefills = [Option<StackRefill>; MOVES];

/// Each borrowed stack in the picker's inventory and what the crate refills
/// it to ([`weapon_pickup::crate_refill`]): the native refill's rule
/// (resupply_from_profile 0x2FDD50..0x2FDE8A: a slot the kit names an item
/// with an allowance in, filled to it, the magazine and chamber of the gun
/// whose selected stack it is inside it) with the borrowed ammunition's own
/// config: the bags the allowance takes of the own ammunition parked from
/// that slot, in rounds of the borrowed one.
///
/// # Safety
/// `player` is live.
unsafe fn borrowed_stacks(player: &Player<'_>, installed: &Installed) -> StackRefills {
    let mut out = [None; MOVES];
    let cooked = gun_cook::inventory(player.thread, installed.cook);
    unsafe {
        let p = player.this;
        let (Some(inventory), Some(profile)) = (p.inventory(), p.profile()) else { return out };
        let kit = read(p.kit());
        if kit >= 3 {
            return out;
        }
        let Some(item) = inventory.slots().at(usize::from(installed.own.slot())).item() else { return out };
        let gun = item.active_object();
        if gun.address() != installed.gun {
            return out;
        }
        let w = gun.weapon();
        let retained = w.retained_ammo();
        let magazine = (read(w.loaded()), read(w.has_chamber()) != 0 && read(w.chamber()) != 0);
        for (room, m) in out.iter_mut().zip(installed.moved.iter()) {
            let Some(m) = m else { continue };
            let Some(stack) = inventory.slots().at(usize::from(m.to)).item() else { continue };
            if stack.address() != m.item {
                continue;
            }
            let Some(ammo) = ammo_kind(stack) else { continue };
            // The native refill's: none for a slot the kit names no item in
            // or gives no allowance; the own stack parked from it says what.
            let word = profile.kit_slots().at(usize::from(kit)).slots().at(usize::from(m.to));
            let own = cooked.and_then(|c| c.slots().at(usize::from(m.from)).item())
                .filter(|own| Some(own.address()) == m.displaced)
                .and_then(|own| ammo_kind(own));
            let allowance = (read(word.item()) != NO_ITEM && read(word.allowance()) > 0)
                .then(|| own.map(|own| (read(word.allowance()), own))).flatten();
            let held = (retained == Some(stack)).then_some(magazine);
            let before = read(stack.count());
            let refill = ammo.bags.map(|bags| {
                let slot = allowance.and_then(|(rounds, own)| own.bags.map(|b| (rounds, b)));
                weapon_pickup::crate_refill(slot, bags, before, held)
            });
            *room = Some(StackRefill { slot: m.to, ammo, before, allowance, held, refill });
        }
    }
    out
}

/// Whether a borrowed stack is below what the crate refills it to.
pub fn reserve_low(player: &Player<'_>, installed: &Installed) -> bool {
    // SAFETY: the live player.
    unsafe { borrowed_stacks(player, installed) }.iter().flatten().any(|s| s.after() > s.before)
}

/// The crate's use: each borrowed stack to what [`borrowed_stacks`] says,
/// free (nobody's storage), and the real inventory's weight marked to be
/// summed again (the native resupply marks its view's). What it did, for
/// the log.
pub fn refill(player: &Player<'_>, installed: &Installed) -> StackRefills {
    // SAFETY: the live player's inventory and its borrowed stacks (each
    // compared with what the install placed in its slot).
    unsafe {
        let stacks = borrowed_stacks(player, installed);
        let Some(inventory) = player.this.inventory() else { return stacks };
        for s in stacks.iter().flatten() {
            if let Some(stack) = inventory.slots().at(usize::from(s.slot)).item() {
                *stack.count() = s.after();
            }
        }
        *inventory.weight_dirty() = 1;
        stacks
    }
}

/// The crate's inventory view (its address, for the native resupply).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View(u32);

static VIEW: Lock<raw::InventoryStorage> = Lock::new(raw::InventoryStorage::zeroed());

/// A copy of `player`'s inventory (188h) with its own gun in its slot and
/// its own parked stacks where the borrowed ones are (a free slot's empty):
/// the loadout the profile names, what
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
            // The picker's own stack where a borrowed one is (the native
            // refill needs an item in every slot the profile names), or none.
            for m in installed.moved.iter().flatten() {
                let own = cooked.slots().at(usize::from(m.from)).item().filter(|i| Some(i.address()) == m.displaced);
                view.slots().at(usize::from(m.to)).set_item(own);
            }
            View(view.address())
        }))
    }
}

// ---- the prompt's reads (stage 2b) -------------------------------------------------------------

/// A dead player's gun as its ragdoll draws it (`carry.weapon`): its world
/// transform (row vectors: a model point p is at p * M) and its model's box
/// in model space as the renderer turns it (Y by pi).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunPose {
    pub transform: [f32; 16],
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

impl GunPose {
    /// The authored box (min xyz, max xyz) turned as update_render_matrices
    /// 0x15A5B0 turns it: [-maxx, miny, -maxz] .. [-minx, maxy, -minz].
    /// Refused when anything is not finite, the box is inverted or a side is
    /// longer than 4 m.
    pub fn parse(transform: [f32; 16], bounds: [f32; 6]) -> Option<GunPose> {
        if !transform.iter().chain(&bounds).all(|v| v.is_finite()) {
            return None;
        }
        if (0..3).any(|n| bounds[n] > bounds[n + 3] || bounds[n + 3] - bounds[n] > 4.0) {
            return None;
        }
        let [x0, y0, z0, x1, y1, z1] = bounds;
        Some(GunPose { transform, lo: [-x1, y0, -z1], hi: [-x0, y1, -z0] })
    }
}

/// At `carry.weapon` (mt_draw, before the held item's draw; the site's
/// player is `player`): its held client weapon's pose, the transform the
/// site has (EAX) and the box of the weapon's model (+1810h -> its render
/// instance, a skeleton_render_model_instance -> its resource's +108h).
pub fn drawn_gun(ctx: &Context<'_>, player: &Player<'_>) -> Option<GunPose> {
    // SAFETY: the site's transform (a live float4x4) and its live player;
    // the held item read as a client weapon only after its vtable says so;
    // a client weapon's model is a skeleton model's render instance.
    unsafe {
        let transform = super::world::floats::<16>(raw::site::carry_weapon::transform(ctx)?.value());
        let held = player.this.held_item()?;
        if read(held.vtable()) != raw::image::WEAPON_VTABLE {
            return None;
        }
        let resource = held.weapon().model()?.render_instance().instance()?.skeleton().resource()?;
        GunPose::parse(transform, super::world::floats::<6>(resource.bounds()))
    }
}

/// A binary config's named table and string types (0.10b configs::enum_types).
const CFG_TABLE: u16 = 3;
const CFG_STRING: u16 = 5;
/// The longest name key kept.
const NAME_KEY: usize = 64;

/// A gun's name key (its config's ui_desc/text_descriptions/name), copied.
#[derive(Clone, Copy)]
pub struct NameKey([u8; NAME_KEY]);

impl NameKey {
    /// No name (the prompt shows the text without it).
    pub const EMPTY: NameKey = NameKey([0; NAME_KEY]);

    pub fn get(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.0).unwrap_or(c"")
    }
}

/// `table`'s child `key`, when `table` is a named table that has it
/// (engine_cfg_child reads past the end for a missing key: asked first, as
/// fill_items_dictionary_impl asks ui_desc, 0x2864B0).
///
/// # Safety
/// `table` is a live config value.
unsafe fn child(table: raw::EngineCfg, key: &CStr) -> Option<raw::EngineCfg> {
    unsafe {
        if read(table.node_type()) != CFG_TABLE || raw::engine_cfg_has(Some(table), key.as_ptr().cast()) & 0xFF == 0 {
            return None;
        }
        raw::engine_cfg_child(Some(table), key.as_ptr().cast())
    }
}

/// The name key of the cooked gun of `cook` (its `slot`): its dictionary
/// entry's config, ui_desc/text_descriptions/name (fill_items_dictionary_impl
/// 0x2865C9..0x2865E9), when every step is there and the key fits.
pub fn gun_name(thread: &Thread, cook: CookId, slot: WeaponSlot) -> Option<NameKey> {
    let inventory = gun_cook::inventory(thread, cook)?;
    // SAFETY: the made cooked inventory (its ticket holds it), its gun's
    // item, the item's dictionary entry and its config, which the dictionary
    // keeps for the match; each value's type read before its use.
    unsafe {
        let (item, _) = cooked_gun(inventory, slot)?;
        let root = item.entry()?.config()?.root()?;
        let name = child(child(child(root, c"ui_desc")?, c"text_descriptions")?, c"name")?;
        if read(name.node_type()) != CFG_STRING {
            return None;
        }
        let text = super::ui::c_string(name.text(), NAME_KEY)?;
        let mut out = [0u8; NAME_KEY];
        out[..text.to_bytes().len()].copy_from_slice(text.to_bytes());
        Some(NameKey(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_is_turned_as_the_renderer_turns_it() {
        let identity = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];
        let pose = GunPose::parse(identity, [-0.1, -0.05, -0.4, 0.2, 0.1, 0.6]).unwrap();
        assert_eq!((pose.lo, pose.hi), ([-0.2, -0.05, -0.6], [0.1, 0.1, 0.4]));
        assert!(GunPose::parse(identity, [0.2, 0., 0., 0.1, 1., 1.]).is_none(), "inverted");
        assert!(GunPose::parse(identity, [0., 0., 0., 5., 1., 1.]).is_none(), "too long");
        assert!(GunPose::parse(identity, [0., 0., f32::NAN, 1., 1., 1.]).is_none());
    }
}
