//! Native artefact inventory ownership. No method exposes an unretained
//! item pointer or a way to take into a full container. The transfer method
//! coordinates native passive unlink/relink and resource references.

use core::sync::atomic::{AtomicU32, Ordering};

use artefact_carry::{Artefact, ArtefactId, ContainerIndex, Refused};
use artefact_carry::carry::QuickSlot;


use super::hook::Thread;
use super::world::{read};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerSpace { Missing, Full, Available }

/// An item's dictionary id (items_dict: its entry's u16 at +0; the id the
/// lobby and the server name it by, scripts/content-edit/items.tsv).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DictId(pub u16);

/// Whether a press of an item's quick key would use it now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    /// In use, cooling down, or used up.
    Busy,
}

/// The class of an item, where a patch tells classes apart: an artefact (an
/// artefact container's), a medicine (medkit's vtable: every drug), an oxygen
/// pack (oxygen_tank's vtable, whatever its dictionary id), or another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemClass {
    /// Ready: passive (state 3) with no cooldown left, what artefact_base::action
    /// (0x313300, patch 54's entry) fires, and one of the 4 of its kind that
    /// count for its holder (patch 109: a copy past them never fires).
    Artefact(Readiness),
    /// Ready: not active (+2B0h) and, a stack, not empty: what medkit::action
    /// (0x33D700) uses.
    Medicine(Readiness),
    OxygenTank,
    Other,
}

/// What a quick slot holds: its item's dictionary id and class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickItem {
    pub id: DictId,
    pub class: ItemClass,
}

/// Where a quick slot's item is: one of the 23 fixed slots, or an artefact
/// container (their own allocation).
#[derive(Clone, Copy)]
enum QuickPlace {
    Slot(raw::InventorySlot),
    Container(ContainerIndex),
}

/// A player's inventory on the simulation thread. These views are not
/// stored between hooks; a later interaction resolves the player again.
pub struct Inventory<'t> {
    player: raw::Player,
    this: raw::Inventory,
    thread: &'t Thread,
}

impl<'t> Inventory<'t> {
    pub(in crate::vostok) fn of_player(player: raw::Player, thread: &'t Thread) -> Option<Self> {
        // SAFETY: Player's live object, within the hook's Thread lifetime.
        let this = unsafe { player.inventory()? };
        Some(Self {
            player,
            this,
            thread,
        })
    }

    fn alive(&self) -> bool {
        // SAFETY: the live Player from which this handle was constructed.
        unsafe { read(self.player.alive()) != 0 }
    }

    /// Native containers are a separate allocation. Their ordinal is the
    /// stable snapshot value; address-relative slots stay inside this API.
    fn containers(&self) -> Option<Containers> {
        Containers::of(self.this)
    }

    pub fn container_space(&self) -> ContainerSpace {
        let Some(containers) = self.containers() else { return ContainerSpace::Missing };
        if (0..containers.count).any(|n| self.item_in(ContainerIndex::parse(n).unwrap()).is_none()) {
            ContainerSpace::Available
        } else { ContainerSpace::Full }
    }

    pub fn presentation(&self, expected: Artefact) -> Option<super::carry_runtime::ArtefactDraw> {
        super::carry_runtime::ArtefactDraw::of_item(self.passive_item(expected)?)
    }

    fn item_in(&self, index: ContainerIndex) -> Option<raw::Artefact> {
        let entry = self.containers()?.entry(index)?;
        // SAFETY: entry belongs to this live inventory's container array.
        unsafe { entry.item().map(|i| i.artefact()) }
    }

    fn native_slot(&self, index: ContainerIndex) -> Option<u32> {
        let entry = self.containers()?.entry(index)?;
        // Match the x86 subtraction and arithmetic shift. This value may
        // be a wrapped negative offset; it is never a Rust array index.
        let base = unsafe { self.this.slots().address() };
        Some(((entry.address().wrapping_sub(base) as i32) >> 2) as u32)
    }

    fn passive_item(&self, expected: Artefact) -> Option<raw::Artefact> {
        let slot = self.native_slot(expected.slot)?;
        let item = self.item_in(expected.slot)?;
        // SAFETY: an artefact slot's live item; all fields belong to the
        // inventory_item/artefact_base subobjects. Native state 3 is passive.
        let matches = unsafe {
            item.item().inventory() == Some(self.this)
                && read(item.item().inventory_slot()) == slot
                && read(item.id()) == expected.id.get()
                && read(item.state()) == 3
        };
        matches.then_some(item)
    }

    pub fn owns_passive(&self, expected: Artefact) -> bool {
        self.passive_item(expected).is_some()
    }

    /// Identity for presentation only, never dereferenced by a patch.
    pub fn presentation_id(&self, expected: Artefact) -> Option<u32> {
        self.passive_item(expected).map(|item| item.address())
    }

    /// Where quick-use ordinal `quick` is, by the game's ordering
    /// (quick_slot_to_inventory_slot 0x2FE7F0, as the hud's quick slots and
    /// process_quick_slots read it: the pockets, the gear's quick items, then
    /// every artefact container, an empty one too). The game's answer is a
    /// fixed slot, or a container's place relative to the fixed slots (native
    /// wrapping arithmetic); it is parsed against the 23 slots and the
    /// containers' allocation before anything is read through it.
    fn quick_place(&self, quick: QuickSlot) -> Option<QuickPlace> {
        // SAFETY: native lookup, verified ABI; it only reads this inventory.
        let slot = unsafe { raw::engine_quick_inventory_slot(Some(self.this), u32::from(quick.get())) };
        self.place_of_slot(slot)
    }

    fn place_of_slot(&self, slot: u32) -> Option<QuickPlace> {
        if slot == u32::from(SLOTS) {
            return None;
        }
        // The containers first: their place relative to the fixed slots is
        // the game's own arithmetic, whatever the allocation's address.
        // SAFETY: the slots' address as a value, never read through here.
        let base = unsafe { self.this.slots().address() };
        let entry = base.wrapping_add(slot.wrapping_mul(4));
        let container = self.containers().and_then(|containers| {
            let offset = entry.checked_sub(containers.first.address())?;
            if offset % 4 != 0 {
                return None;
            }
            let index = ContainerIndex::parse(u8::try_from(offset / 4).ok()?)?;
            containers.entry(index).map(|_| QuickPlace::Container(index))
        });
        // SAFETY: one of this live inventory's 23 slots (+10Ch + 4 x slot).
        container.or_else(|| (slot < u32::from(SLOTS)).then(|| QuickPlace::Slot(unsafe { self.this.slots().at(slot as usize) })))
    }

    pub(super) fn slot_wear(&self, slot: u32) -> Option<(u8, srp_validate::world::ArtefactWear)> {
        let QuickPlace::Container(index) = self.place_of_slot(slot)? else { return None };
        let item = self.item_in(index)?;
        super::artefact_skills::kind_of(item)?;
        // SAFETY: the live container holds this registered artefact.
        Some((unsafe { *item.id() }, super::artefact_skills::wear(item)))
    }

    /// The item quick-use ordinal `quick` holds, as the hud shows it
    /// (game_world_ui::update_local_player_state 0x23B740): none past the last
    /// quick slot, or for an empty artefact container.
    pub fn quick_item(&self, quick: QuickSlot) -> Option<QuickItem> {
        let ready = |ready: bool| if ready { Readiness::Ready } else { Readiness::Busy };
        let place = self.quick_place(quick)?;
        let entry = match place {
            QuickPlace::Slot(entry) => entry,
            QuickPlace::Container(index) => self.containers()?.entry(index)?,
        };
        // SAFETY: a slot of this live inventory, its item alive while the slot
        // holds it, its vtable its own, its dictionary entry the game's (static
        // data); a container's item is an artefact, a medkit's fields read only
        // after its vtable says it is one.
        unsafe {
            let item = entry.item()?;
            let dictionary = item.entry()?;
            let class = if let QuickPlace::Container(_) = place {
                let artefact = item.artefact();
                ItemClass::Artefact(ready(read(artefact.state()) == 3 && read(artefact.cooldown_left()) == 0
                    && super::artefact_skills::wear(artefact) != srp_validate::world::ArtefactWear::Spent
                    && !super::artefact_skills::uncounted(artefact)))
            } else {
                match read(item.vtable()) {
                    raw::image::MEDKIT_VTABLE => ItemClass::Medicine(ready(
                        read(item.medkit().active()) == 0 && (read(dictionary.stack()) == 0 || read(item.count()) > 0))),
                    raw::image::OXYGEN_TANK_VTABLE => ItemClass::OxygenTank,
                    _ => ItemClass::Other,
                }
            };
            Some(QuickItem { id: DictId(read(dictionary.dict_id())), class })
        }
    }

    /// Resolve a quick-use ordinal with the native ordering, then require a
    /// passive, owned artefact. Medkits and empty containers return None.
    pub fn quick_artefact(&self, quick: QuickSlot) -> Option<Artefact> {
        if !self.alive() {
            return None;
        }
        let QuickPlace::Container(slot) = self.quick_place(quick)? else { return None };
        let item = self.item_in(slot)?;
        // SAFETY: the item came from a native artefact slot, not a drug slot.
        let id = ArtefactId::parse(unsafe { read(item.id()) })?;
        let artefact = Artefact { id, slot };
        self.passive_item(artefact).map(|_| artefact)
    }

    /// Keeper's gift: an unused pool artefact (state 0, in no inventory, never given: keeper's
    /// mark) into the first empty container. It gets the count and the cooldown the anomaly's
    /// spawn gives one (artefact_base::on_spawned 0x3134C0), then the game's own take_artefact
    /// (as a ground pickup) puts it into the first empty container and links its passive. With a
    /// container free take_artefact has no refusal (its first empty one at 0x2FC883, set_item
    /// 0x2FCA32, on_taken), but it is marked given (`keeper::mark_given`) only once it is in this
    /// inventory, so a gift is never marked and not delivered.
    pub(super) fn take_dormant(&mut self, item: raw::Artefact) -> bool {
        if !self.alive() || self.container_space() != ContainerSpace::Available {
            return false;
        }
        if super::keeper::history(item) != crate::artefact_keeper::History::Unused {
            return false;
        }
        // SAFETY: a registered artefact of the live core world; its fields are artefact_base's
        // and inventory_item's. Retained through the take; a free container (`container_space`)
        // keeps take_artefact off its evicting path.
        unsafe {
            if read(item.state()) != 0 || item.item().inventory().is_some() {
                return false;
            }
            let count = match read(item.default_count()) { 0xFFFF => 1, count => count };
            item.item().count().write_volatile(count);
            item.cooldown_left().write_volatile(0);
            let held = RetainedItem::new(item, self.thread);
            let mut incoming = Some(held.this);
            raw::engine_inventory_take_artefact(Some(self.this), &mut incoming);
            if item.item().inventory() != Some(self.this) {
                return false;
            }
            super::keeper::mark_given(item);
        }
        true
    }

    pub(super) fn drop_carried(&mut self, expected: Artefact, drop: artefact_carry::ground::Drop) -> bool {
        let Some(item) = self.passive_item(expected) else { return false };
        let Some(ground) = super::carry_ground::GroundItem::of_item(item, self.thread) else { return false };
        let Some(slot) = self.native_slot(expected.slot) else { return false };
        let _held = RetainedItem::new(item, self.thread);
        // SAFETY: validated occupied slot; remove unlinks all passive effects
        // and clears ownership. RetainedItem bridges that mutation.
        unsafe { raw::engine_inventory_remove(Some(self.this), slot); }
        ground.release_from(drop);
        true
    }

    pub(super) fn take_ground(&mut self, ground: &super::carry_ground::GroundItem<'t>) -> Result<(), Refused> {
        if !self.alive() || !ground.detached() { return Err(Refused::OwnershipChanged); }
        if self.container_space() != ContainerSpace::Available { return Err(Refused::NoFreeContainer); }
        let held = RetainedItem::new(ground.item, self.thread);
        let mut incoming = Some(held.this);
        // SAFETY: detached registered artefact, retained through take; a free
        // recipient container prevents native eviction.
        unsafe { raw::engine_inventory_take_artefact(Some(self.this), &mut incoming); }
        Ok(())
    }

    /// Transfer one still-owned passive artefact into a free recipient
    /// container. The interaction owner separately checks reach and line
    /// of sight immediately before calling this low-level transaction.
    ///
    /// All failure paths precede mutation. The native passive callbacks
    /// adjust modifier lists, not the inventory's container range; after
    /// both ranges are checked there is no fallible allocation or lookup
    /// between removal and insertion. Native take_artefact's evicting path
    /// is therefore unreachable here.
    pub fn transfer_artefact(
        &mut self,
        recipient: &mut Inventory<'t>,
        expected: Artefact,
    ) -> Result<(), Refused> {
        if self.player.address() == recipient.player.address() || self.this.address() == recipient.this.address() {
            return Err(Refused::SamePlayer);
        }
        if !self.alive() || !recipient.alive() {
            return Err(Refused::OwnershipChanged);
        }
        let item = self
            .passive_item(expected)
            .ok_or(Refused::OwnershipChanged)?;
        if recipient.container_space() != ContainerSpace::Available {
            return Err(Refused::NoFreeContainer);
        }
        let native_slot = self
            .native_slot(expected.slot)
            .ok_or(Refused::OwnershipChanged)?;
        let held = RetainedItem::new(item, self.thread);
        // SAFETY: donor slot is occupied by `held`; native remove disables
        // its passive effects before changing its owner. `held` keeps the
        // resource alive across the empty-slot interval. Recipient has an
        // empty container, so native take cannot evict another artefact.
        unsafe {
            raw::engine_inventory_remove(Some(self.this), native_slot);
            let mut incoming = Some(held.this);
            raw::engine_inventory_take_artefact(Some(recipient.this), &mut incoming);
        }
        Ok(())
    }

    /// Courier (crate::artefact_courier): the artefacts of the kit `donor` the place left into
    /// this alive kit's empty containers, in the donor's container order, as many as there are
    /// empty ones.
    ///
    /// The donor is out of the world by then, dead or alive when it left: the respawn rule's tick
    /// takes a kit's player out (0x31699B, base_player::remove 0x2DA790 with 1) before its spawn
    /// (a round's start spawns everyone), as patch 31's switch does in a countdown. remove_alive
    /// (0x2D99C0) writes alive 0; unload_to_profile clears the container range (0x2FDBE8..0x2FDBF2:
    /// +168h 0, +16Ch 0) and leaves the artefacts as they were. Its slots keep their items, so the
    /// donor's are found in its 23 slots: an item of `world`'s registered artefacts whose owner and
    /// slot are that slot's, in slot order (the containers' order: their range is a run of slots).
    ///
    /// Each leaves as its state says ([`Held`]), then the game's own take_artefact (as a pickup:
    /// set_item, on_taken links the passive, state 3). A free container keeps take_artefact off
    /// its evicting path.
    pub(super) fn take_from_kit(&mut self, donor: &mut Inventory<'t>, world: raw::CoreWorld) -> crate::artefact_courier::Carried {
        let mut carried = crate::artefact_courier::Carried::default();
        if self.player.address() == donor.player.address() || self.this.address() == donor.this.address() {
            return carried;
        }
        if !self.alive() || donor.alive() {
            return carried;
        }
        for slot in 0..SLOTS {
            // SAFETY: one of the donor's 23 slots (+10Ch + 4 x slot).
            let entry = unsafe { donor.this.slots().at(usize::from(slot)) };
            // SAFETY: the slot's item, if any; read as an artefact only once it is a registered one.
            let Some(address) = (unsafe { entry.item() }).map(|item| item.address()) else { continue };
            let Some((_, item)) = super::keeper::registered(world).find(|(_, item)| item.address() == address) else {
                continue;
            };
            // SAFETY: a registered artefact's inventory_item's and artefact_base's fields.
            let held = unsafe {
                (item.item().inventory() == Some(donor.this) && read(item.item().inventory_slot()) == u32::from(slot))
                    .then(|| Held::parse(read(item.state())))
                    .flatten()
            };
            let Some(held) = held else { continue };
            if self.container_space() != ContainerSpace::Available {
                carried.left += 1;
                continue;
            }
            let retained = RetainedItem::new(item, self.thread);
            // SAFETY: the donor's occupied slot (`entry`) and its item, retained by `retained`
            // across the empty-slot interval; the recipient has an empty container.
            unsafe {
                match held {
                    Held::Live => raw::engine_inventory_remove(Some(donor.this), u32::from(slot)),
                    Held::Dead => {
                        item.item().set_inventory(None);
                        item.item().inventory_slot().write_volatile(u32::from(SLOTS));
                        item.state().write_volatile(0);
                        entry.set_item(None);
                        drop(RetainedItem::adopt(item, self.thread));
                        donor.this.weight_dirty().write_volatile(1);
                    }
                }
                let mut incoming = Some(retained.this);
                raw::engine_inventory_take_artefact(Some(self.this), &mut incoming);
                if item.item().inventory() == Some(self.this) {
                    carried.moved += 1;
                }
            }
        }
        carried
    }
}

/// How an artefact sits in the kit Courier takes it from (artefact_base +280h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    /// Its passive linked (3), its active too (4): the kit left alive (a countdown's switch, a
    /// round's start). It leaves as remove_item (0x2FC680) takes it: on_removed (vtable+34h,
    /// 0x313490) ends the active, unlinks the passive and writes 0; then the owner, the slot, the
    /// reference and the weight.
    Live,
    /// Its holder's death switched it off (5: artefact_base::tick 0x3132E5 unlinks its passive,
    /// vtable+84h, then writes 5; nothing else writes 5). It leaves as remove_item takes an item
    /// but without on_removed, whose vtable+84h would unlink the passive a second time: Spring's
    /// list removal (0x2D5D00) is not idempotent (an unlinked node empties its list's head). So:
    /// no owner and slot 23 (the set_owner remove_item calls, 0x243590), state 0 (on_removed's
    /// end), the slot and its reference released, the weight to recalculate.
    Dead,
}

impl Held {
    fn parse(state: u32) -> Option<Held> {
        match state {
            3 | 4 => Some(Held::Live),
            5 => Some(Held::Dead),
            _ => None,
        }
    }
}

/// The 23 slots of an inventory (+10Ch), by their native index (the item's +254h).
pub(in crate::vostok) const SLOTS: u8 = 23;

/// One kit's inventory at a world's load, its slots compared with the world's artefacts
/// (`keeper::reconcile`). It writes a slot's pointer and its reference (as set_item 0x2FC620 and
/// remove_item 0x2FC680 do), nothing else: not the weight (+180h) or its recalculation flag
/// (+184h), which a world saves and loads for the current kit (0x2FE37A / 0x2FE39A, 0x2FE56D /
/// 0x2FE583, a count of 0 too) and a spawn sets for the others (0x2FD2B8); no passive linked or
/// unlinked: what a held artefact's passive is, the kit's next spawn decides (setup_from_profile's
/// on_taken), as in a game that did not load the world.
pub(in crate::vostok) struct KitSlots<'t> {
    this: raw::Inventory,
    _thread: &'t Thread,
}

impl<'t> KitSlots<'t> {
    pub(in crate::vostok) fn of_player(player: raw::Player, thread: &'t Thread) -> Option<Self> {
        // SAFETY: a live player of the world's places; its inventory pointer (+10Ch).
        let this = unsafe { player.inventory()? };
        Some(Self { this, _thread: thread })
    }

    pub(in crate::vostok) fn inventory(&self) -> raw::Inventory {
        self.this
    }

    fn entry(&self, slot: u8) -> Option<raw::InventorySlot> {
        // SAFETY: one of this live inventory's 23 slots (+10Ch + 4 x slot).
        (slot < SLOTS).then(|| unsafe { self.this.slots().at(usize::from(slot)) })
    }

    /// The item in `slot`, if any.
    pub(in crate::vostok) fn item(&self, slot: u8) -> Option<raw::Item> {
        // SAFETY: as in `entry`.
        unsafe { self.entry(slot)?.item() }
    }

    /// Empties `slot`, releasing its reference (as remove_item does, without its owner write:
    /// the item's inventory is the world's, another's or none).
    pub(in crate::vostok) fn empty(&mut self, slot: u8) {
        let Some(entry) = self.entry(slot) else { return };
        // SAFETY: the slot's live item and the reference the slot holds on it.
        unsafe {
            let Some(item) = entry.item() else { return };
            entry.set_item(None);
            drop(RetainedItem::adopt(item.artefact(), self._thread));
        }
    }

    /// Puts `item` into the empty `slot`, with the slot's reference (as set_item does, without its
    /// owner write: the world has it in this inventory at this slot already).
    pub(in crate::vostok) fn fill(&mut self, slot: u8, item: raw::Artefact) {
        let Some(entry) = self.entry(slot) else { return };
        // SAFETY: an empty slot of this live inventory; a live registered artefact, retained
        // once more for the slot.
        unsafe {
            if entry.item().is_some() {
                return;
            }
            AtomicU32::from_ptr(item.item().references()).fetch_add(1, Ordering::AcqRel);
            entry.set_item(Some(item.item()));
        }
    }
}

/// Validated allocation metadata; no address escapes the inventory owner.
struct Containers {
    first: raw::InventorySlot,
    count: u8,
}
impl Containers {
    fn of(this: raw::Inventory) -> Option<Containers> {
        // SAFETY: a live inventory owns this array for the hook lifetime.
        let (first, count) = unsafe { (this.artefact_slots_begin(), read(this.artefact_slot_count())) };
        let first = first?;
        if !first.address().is_multiple_of(4) || count == 0 {
            return None;
        }
        first.address().checked_add(u32::from(count) * 4)?;
        Some(Containers { first, count })
    }

    fn entry(&self, index: ContainerIndex) -> Option<raw::InventorySlot> {
        // SAFETY: the ordinal is below the validated allocation count.
        (index.get() < self.count).then(|| unsafe { self.first.at(usize::from(index.get())) })
    }
}

/// Own the extra reference throughout both native callbacks, including any
/// return path added later. Callers never manage the reference themselves.
struct RetainedItem<'t> {
    this: raw::Artefact,
    _thread: &'t Thread,
}

impl<'t> RetainedItem<'t> {
    fn new(this: raw::Artefact, thread: &'t Thread) -> Self {
        // SAFETY: an owning inventory slot already retains this live item.
        unsafe {
            AtomicU32::from_ptr(this.item().references()).fetch_add(1, Ordering::AcqRel);
        }
        Self {
            this,
            _thread: thread,
        }
    }
}

impl<'t> RetainedItem<'t> {
    /// Takes over a reference the caller held (a slot's, the slot emptied).
    fn adopt(this: raw::Artefact, thread: &'t Thread) -> Self {
        Self { this, _thread: thread }
    }
}

impl Drop for RetainedItem<'_> {
    fn drop(&mut self) {
        // SAFETY: this guard owns exactly one reference; the native release
        // helper is called only if that was the last live reference.
        unsafe {
            let count = self.this.item().references();
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_item(Some(self.this), count);
            }
        }
    }
}
