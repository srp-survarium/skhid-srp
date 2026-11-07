//! Native artefact inventory ownership. No method exposes an unretained
//! item pointer or a way to take into a full container. The transfer method
//! coordinates native passive unlink/relink and resource references.

use core::sync::atomic::{AtomicU32, Ordering};

use artefact_carry::{Artefact, ArtefactId, ContainerIndex, Refused};


use super::hook::Thread;
use super::world::{read};
use crate::hidden_stash::{self, QuickSlot, Room, Run, Stash};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerSpace { Missing, Full, Available }

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
        // SAFETY: a live inventory owns this array for the hook lifetime.
        let (first, count) = unsafe {
            (
                self.this.artefact_slots_begin(),
                read(self.this.artefact_slot_count()),
            )
        };
        let first = first?;
        if !first.address().is_multiple_of(4) || count == 0 {
            return None;
        }
        first.address().checked_add(u32::from(count) * 4)?;
        Some(Containers { first, count })
    }

    /// Room for a take: an empty container of the run, or the kit's hidden stash to open
    /// (Hidden Stash, crate::hidden_stash), which every take of this API opens first.
    pub fn container_space(&self) -> ContainerSpace {
        match self.room() {
            Room::Container | Room::Stash(_) => ContainerSpace::Available,
            Room::Full => ContainerSpace::Full,
            Room::Missing => ContainerSpace::Missing,
        }
    }

    fn room(&self) -> Room {
        let count = self.containers().map_or(0, |containers| containers.count);
        let empty = (0..count).map(|n| self.item_in(ContainerIndex::parse(n).unwrap()).is_none());
        hidden_stash::room(empty, self.stash())
    }

    /// The run as the game has it: its count and its first slot's index of the 23.
    fn run(&self) -> Option<Run> {
        // SAFETY: the live inventory's count byte and first-slot pointer; the pointer is only
        // compared with the slots' start, never read through here.
        let (count, first) = unsafe { (read(self.this.artefact_slot_count()), self.this.artefact_slots_begin()) };
        // SAFETY: as above, the slots' start inside the live inventory.
        let slots = unsafe { self.this.slots() };
        let first = first.and_then(|first| first.index_from(slots)).and_then(|n| u8::try_from(n).ok());
        Run::parse(count, first)
    }

    fn quick_slot(&self, slot: QuickSlot) -> raw::InventorySlot {
        // SAFETY: a quick position, 13..18, of the inventory's 23 slots.
        unsafe { self.this.slots().at(usize::from(slot.get())) }
    }

    fn quick_empty(&self, slot: QuickSlot) -> bool {
        // SAFETY: the live inventory's slot (quick_slot).
        unsafe { self.quick_slot(slot).item().is_none() }
    }

    /// The kit's hidden stash now: its skill bit, its modifier 14 (apply_modifier's sum, as
    /// setup_from_profile reads it) and the run.
    pub(super) fn stash(&self) -> Stash {
        let skills = super::artefact_skills::skills(self.player);
        if !skills.hidden() {
            return Stash::Unavailable;
        }
        let Some(modifiers) = super::medical::kit_modifiers(self.player) else { return Stash::Unavailable };
        // SAFETY: the native getter over the live current kit's modifiers (setup_from_profile's
        // own call, 0x2FD20F: index 14, value 0, weight 1).
        let containers = unsafe { raw::engine_apply_modifier(Some(modifiers), 14, 0.0, 1.0) };
        hidden_stash::stash(true, containers, self.run(), |slot| self.quick_empty(slot))
    }

    /// Before a take: when no container is empty and the stash is shut, the run grows by its
    /// slot, so the game's own take_artefact puts the artefact there (its first empty one).
    fn open_for_take(&mut self) {
        let Room::Stash(at) = self.room() else { return };
        // SAFETY: the live inventory's run fields; `at` is an empty quick position right after
        // the run (or the first empty one with no run), so the run stays inside 13..18.
        unsafe {
            let count = read(self.this.artefact_slot_count());
            if count == 0 {
                self.this.set_artefact_slots_begin(Some(self.quick_slot(at)));
            }
            self.this.artefact_slot_count().write_volatile(count + 1);
        }
    }

    /// An anomaly's take (use_execute before transfer_artefact): the stash opens if it is the
    /// room.
    pub(super) fn open_for_anomaly(&mut self) {
        if self.alive() {
            self.open_for_take();
        }
    }

    /// An open stash whose slot no longer holds an artefact (handed over): the run the kit's own
    /// again.
    fn close_emptied_stash(&mut self) {
        let Stash::Open { at } = self.stash() else { return };
        if !self.quick_empty(at) {
            return;
        }
        // SAFETY: the live inventory's count byte; the run's last slot (the stash's) is empty,
        // so it ends before it.
        unsafe {
            let count = read(self.this.artefact_slot_count());
            self.this.artefact_slot_count().write_volatile(count.saturating_sub(1));
        }
    }

    /// At death, an open stash: its artefact out by the game's own remove_item (on_removed:
    /// the active ended, the passive unlinked, state 0, dormant in its anomaly's or Keeper's
    /// pool; no owner), and the run the kit's own again.
    pub(super) fn empty_stash(&mut self) -> Option<bool> {
        let Stash::Open { at } = self.stash() else { return None };
        // SAFETY: the open stash's slot of the live inventory: remove_item takes the item there
        // (ESI the inventory, EAX the slot); the run then ends before it.
        unsafe {
            let held = self.quick_slot(at).item().is_some();
            if held {
                raw::engine_inventory_remove(Some(self.this), u32::from(at.get()));
            }
            let count = read(self.this.artefact_slot_count());
            self.this.artefact_slot_count().write_volatile(count.saturating_sub(1));
            Some(held)
        }
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

    /// Resolve a quick-use ordinal with the native ordering, then require a
    /// passive, owned artefact. Medkits and empty containers return None.
    pub fn quick_artefact(&self, quick: u8) -> Option<Artefact> {
        if quick >= 6 || !self.alive() {
            return None;
        }
        // SAFETY: native lookup, verified ABI; it only reads this inventory.
        let slot = unsafe { raw::engine_quick_inventory_slot(Some(self.this), u32::from(quick)) };
        if slot == 23 {
            return None;
        }
        let containers = self.containers()?;
        // Reconstruct an address using native wrapping arithmetic, then
        // parse it against the external allocation before dereferencing.
        let base = unsafe { self.this.slots().address() };
        let entry = base.wrapping_add(slot.wrapping_mul(4));
        let offset = entry.checked_sub(containers.first.address())?;
        if offset % 4 != 0 {
            return None;
        }
        let slot = ContainerIndex::parse(u8::try_from(offset / 4).ok()?)?;
        let item = self.item_in(slot)?;
        // SAFETY: the item came from a native artefact slot, not a drug slot.
        let id = ArtefactId::parse(unsafe { read(item.id()) })?;
        let artefact = Artefact { id, slot };
        self.passive_item(artefact).map(|_| artefact)
    }

    /// Keeper's gift: an unused pool artefact (state 0, in no inventory, never given: keeper's
    /// mark) into the first empty container. It is marked given (`keeper::mark_given`) and gets
    /// the count and the cooldown the anomaly's spawn gives one (artefact_base::on_spawned
    /// 0x3134C0), then the game's own take_artefact (as a ground pickup) puts it into the first
    /// empty container and links its passive.
    pub(super) fn take_dormant(&mut self, item: raw::Artefact) -> bool {
        if !self.alive() || self.container_space() != ContainerSpace::Available {
            return false;
        }
        if super::keeper::history(item) != crate::artefact_keeper::History::Unused {
            return false;
        }
        // SAFETY: a registered artefact of the live core world; its fields are artefact_base's
        // and inventory_item's. Retained through the take; the empty first container keeps
        // take_artefact off its evicting path.
        unsafe {
            if read(item.state()) != 0 || item.item().inventory().is_some() {
                return false;
            }
            super::keeper::mark_given(item);
            let count = match read(item.default_count()) { 0xFFFF => 1, count => count };
            item.item().count().write_volatile(count);
            item.cooldown_left().write_volatile(0);
            self.open_for_take();
            let held = RetainedItem::new(item, self.thread);
            let mut incoming = Some(held.this);
            raw::engine_inventory_take_artefact(Some(self.this), &mut incoming);
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
        self.open_for_take();
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
        recipient.open_for_take();
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
        // Handed over from the stash: it closes at once (the HUD's position goes with it,
        // hidden_stash::stale).
        self.close_emptied_stash();
        Ok(())
    }
}

/// Validated allocation metadata; no address escapes the inventory owner.
struct Containers {
    first: raw::InventorySlot,
    count: u8,
}
impl Containers {
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
