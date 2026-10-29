//! Native artefact inventory ownership. No method exposes an unretained
//! item pointer or a way to take into a full container. The transfer method
//! coordinates native passive unlink/relink and resource references.

use core::sync::atomic::{AtomicU32, Ordering};

use artefact_carry::{Artefact, ArtefactId, ContainerIndex, Refused};

use super::hook::Thread;
use super::world::{Object, pointer, read};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerSpace { Missing, Full, Available }

/// A player's inventory on the simulation thread. These views are not
/// stored between hooks; a later interaction resolves the player again.
pub struct Inventory<'t> {
    player: Object,
    this: Object,
    thread: &'t Thread,
}

impl<'t> Inventory<'t> {
    pub(in crate::vostok) fn of_player(player: Object, thread: &'t Thread) -> Option<Self> {
        // SAFETY: Player's live object, within the hook's Thread lifetime.
        let this = unsafe { pointer(raw::player::inventory(player.ptr()))? };
        Some(Self {
            player,
            this,
            thread,
        })
    }

    fn alive(&self) -> bool {
        // SAFETY: the live Player from which this handle was constructed.
        unsafe { read(raw::player::alive(self.player.ptr())) != 0 }
    }

    /// Native containers are a separate allocation. Their ordinal is the
    /// stable snapshot value; address-relative slots stay inside this API.
    fn containers(&self) -> Option<Containers> {
        // SAFETY: a live inventory owns this array for the hook lifetime.
        let (first, count) = unsafe {
            (
                read(raw::inventory::artefact_slots_begin(self.this.ptr())),
                read(raw::inventory::artefact_slot_count(self.this.ptr())),
            )
        };
        if first == 0 || first % 4 != 0 || count == 0 {
            return None;
        }
        first.checked_add(u32::from(count) * 4)?;
        Some(Containers { first, count })
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

    fn item_in(&self, index: ContainerIndex) -> Option<Object> {
        let entry = self.containers()?.entry(index)?;
        // SAFETY: entry belongs to this live inventory's container array.
        unsafe { pointer(entry as *mut u32) }
    }

    fn native_slot(&self, index: ContainerIndex) -> Option<u32> {
        let entry = self.containers()?.entry(index)?;
        // Match the x86 subtraction and arithmetic shift. This value may
        // be a wrapped negative offset; it is never a Rust array index.
        let base = unsafe { raw::inventory::slots(self.this.ptr()) } as u32;
        Some(((entry.wrapping_sub(base) as i32) >> 2) as u32)
    }

    fn passive_item(&self, expected: Artefact) -> Option<Object> {
        let slot = self.native_slot(expected.slot)?;
        let item = self.item_in(expected.slot)?;
        // SAFETY: an artefact slot's live item; all fields belong to the
        // inventory_item/artefact_base subobjects. Native state 3 is passive.
        let matches = unsafe {
            read(raw::item::inventory(item.ptr())) == self.this.id()
                && read(raw::item::inventory_slot(item.ptr())) == slot
                && read(raw::artefact::id(item.ptr())) == expected.id.get()
                && read(raw::artefact::state(item.ptr())) == 3
        };
        matches.then_some(item)
    }

    pub fn owns_passive(&self, expected: Artefact) -> bool {
        self.passive_item(expected).is_some()
    }

    /// Identity for presentation only, never dereferenced by a patch.
    pub fn presentation_id(&self, expected: Artefact) -> Option<u32> {
        self.passive_item(expected).map(Object::id)
    }

    /// Resolve a quick-use ordinal with the native ordering, then require a
    /// passive, owned artefact. Medkits and empty containers return None.
    pub fn quick_artefact(&self, quick: u8) -> Option<Artefact> {
        if quick >= 6 || !self.alive() {
            return None;
        }
        // SAFETY: native lookup, verified ABI; it only reads this inventory.
        let slot = unsafe { raw::engine_quick_inventory_slot(self.this.ptr(), u32::from(quick)) };
        if slot == 23 {
            return None;
        }
        let containers = self.containers()?;
        // Reconstruct an address using native wrapping arithmetic, then
        // parse it against the external allocation before dereferencing.
        let base = unsafe { raw::inventory::slots(self.this.ptr()) } as u32;
        let entry = base.wrapping_add(slot.wrapping_mul(4));
        let offset = entry.checked_sub(containers.first)?;
        if offset % 4 != 0 {
            return None;
        }
        let slot = ContainerIndex::parse(u8::try_from(offset / 4).ok()?)?;
        let item = self.item_in(slot)?;
        // SAFETY: the item came from a native artefact slot, not a drug slot.
        let id = ArtefactId::parse(unsafe { read(raw::artefact::id(item.ptr())) })?;
        let artefact = Artefact { id, slot };
        self.passive_item(artefact).map(|_| artefact)
    }

    pub(super) fn drop_carried(&mut self, expected: Artefact, drop: artefact_carry::ground::Drop) -> bool {
        let Some(item) = self.passive_item(expected) else { return false };
        let Some(ground) = super::carry_ground::GroundItem::of_item(item, self.thread) else { return false };
        let Some(slot) = self.native_slot(expected.slot) else { return false };
        let _held = RetainedItem::new(item, self.thread);
        // SAFETY: validated occupied slot; remove unlinks all passive effects
        // and clears ownership. RetainedItem bridges that mutation.
        unsafe { raw::engine_inventory_remove(self.this.ptr(), slot); }
        ground.release_from(drop);
        true
    }

    pub(super) fn take_ground(&mut self, ground: &super::carry_ground::GroundItem<'t>) -> Result<(), Refused> {
        if !self.alive() || !ground.detached() { return Err(Refused::OwnershipChanged); }
        if self.container_space() != ContainerSpace::Available { return Err(Refused::NoFreeContainer); }
        let held = RetainedItem::new(ground.item, self.thread);
        let incoming = held.this.id();
        // SAFETY: detached registered artefact, retained through take; a free
        // recipient container prevents native eviction.
        unsafe { raw::engine_inventory_take_artefact(self.this.ptr(), &incoming); }
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
        if self.player.id() == recipient.player.id() || self.this.id() == recipient.this.id() {
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
            raw::engine_inventory_remove(self.this.ptr(), native_slot);
            let incoming = held.this.id();
            raw::engine_inventory_take_artefact(recipient.this.ptr(), &incoming);
        }
        Ok(())
    }
}

/// Validated allocation metadata; no address escapes the inventory owner.
struct Containers {
    first: u32,
    count: u8,
}
impl Containers {
    fn entry(&self, index: ContainerIndex) -> Option<u32> {
        (index.get() < self.count).then(|| self.first + u32::from(index.get()) * 4)
    }
}

/// Own the extra reference throughout both native callbacks, including any
/// return path added later. Callers never manage the reference themselves.
struct RetainedItem<'t> {
    this: Object,
    _thread: &'t Thread,
}

impl<'t> RetainedItem<'t> {
    fn new(this: Object, thread: &'t Thread) -> Self {
        // SAFETY: an owning inventory slot already retains this live item.
        unsafe {
            AtomicU32::from_ptr(raw::item::references(this.ptr())).fetch_add(1, Ordering::AcqRel);
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
            let count = raw::item::references(self.this.ptr());
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(self.this.ptr(), count);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;
    use std::boxed::Box;

    struct Fixture {
        player: Box<[u32; 256]>,
        inventory: Box<[u32; 128]>,
        containers: Box<[u32; 3]>,
        item: Box<[u32; 192]>,
    }
    fn index(n: u8) -> ContainerIndex {
        ContainerIndex::parse(n).unwrap()
    }
    impl Fixture {
        fn new(count: u8) -> Self {
            let mut f = Self {
                player: Box::new([0; 256]),
                inventory: Box::new([0; 128]),
                containers: Box::new([0; 3]),
                item: Box::new([0; 192]),
            };
            let inv = f.inventory.as_mut_ptr().cast();
            // SAFETY: fixture allocations cover all accessed generated fields.
            unsafe {
                raw::player::inventory(f.player.as_mut_ptr().cast()).write(inv as u32);
                raw::player::alive(f.player.as_mut_ptr().cast()).write(1);
                raw::inventory::artefact_slots_begin(inv).write(f.containers.as_mut_ptr() as u32);
                raw::inventory::artefact_slot_count(inv).write(count);
            }
            f
        }
        fn insert(&mut self, ordinal: u8, id: u8, state: u32) -> Artefact {
            let ptr = self.item.as_mut_ptr().cast();
            let inv = self.inventory.as_mut_ptr().cast();
            // SAFETY: boxed item and separate container allocation remain live.
            unsafe {
                let base = raw::inventory::slots(inv) as u32;
                let entry = self.containers.as_mut_ptr().add(usize::from(ordinal)) as u32;
                raw::item::references(ptr).write(2);
                raw::item::inventory(ptr).write(inv as u32);
                raw::item::inventory_slot(ptr)
                    .write(((entry.wrapping_sub(base) as i32) >> 2) as u32);
                raw::artefact::id(ptr).write(id);
                raw::artefact::state(ptr).write(state);
            }
            self.containers[usize::from(ordinal)] = ptr as u32;
            Artefact {
                id: ArtefactId::parse(id).unwrap(),
                slot: index(ordinal),
            }
        }
        fn view<'t>(&'t mut self, thread: &'t Thread) -> Inventory<'t> {
            Inventory::of_player(Object::at(self.player.as_mut_ptr() as u32).unwrap(), thread)
                .unwrap()
        }
    }
    // Original mutation routines run independently in the Unicorn check.
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_inventory_remove(inv: *mut c_void, slot: u32) {
        // SAFETY: fake engine ABI receives this test's live fixture.
        unsafe {
            let base = raw::inventory::slots(inv) as u32;
            let entry = base.wrapping_add(slot.wrapping_mul(4)) as *mut u32;
            let item = read(entry) as *mut c_void;
            assert_eq!(read(raw::item::inventory(item)), inv as u32);
            assert!(read(raw::item::references(item)) >= 3);
            raw::artefact::state(item).write(0);
            raw::item::inventory(item).write(0);
            raw::item::inventory_slot(item).write(23);
            entry.write(0);
            let refs = raw::item::references(item);
            refs.write(read(refs) - 1);
        }
    }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_inventory_take_artefact(inv: *mut c_void, incoming: *const u32) {
        // SAFETY: fake engine ABI receives this test's live fixture.
        unsafe {
            let item = incoming.read() as *mut c_void;
            assert_eq!(read(raw::item::inventory(item)), 0);
            let first = read(raw::inventory::artefact_slots_begin(inv)) as *mut u32;
            let count = usize::from(read(raw::inventory::artefact_slot_count(inv)));
            let i = (0..count)
                .find(|i| read(first.add(*i)) == 0)
                .expect("eviction reached");
            let entry = first.add(i);
            let base = raw::inventory::slots(inv) as u32;
            let slot = (((entry as u32).wrapping_sub(base) as i32) >> 2) as u32;
            entry.write(item as u32);
            raw::item::inventory(item).write(inv as u32);
            raw::item::inventory_slot(item).write(slot);
            raw::artefact::state(item).write(3);
            let refs = raw::item::references(item);
            refs.write(read(refs) + 1);
        }
    }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_quick_inventory_slot(inv: *mut c_void, ordinal: u32) -> u32 {
        // SAFETY: native lookup stub for an inventory with no medical items.
        unsafe {
            if ordinal >= u32::from(read(raw::inventory::artefact_slot_count(inv))) {
                return 23;
            }
            let first = read(raw::inventory::artefact_slots_begin(inv));
            let base = raw::inventory::slots(inv) as u32;
            (((first.wrapping_sub(base) as i32) >> 2) as u32).wrapping_add(ordinal)
        }
    }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_release_object(_: *mut c_void, _: *mut u32) {
        panic!("anomaly reference lost");
    }
    #[test]
    fn external_containers_resolve_and_transfer_same_resource() {
        // SAFETY: test confines fake simulation thread and its allocations.
        let thread = unsafe { Thread::new() };
        let mut from = Fixture::new(2);
        let expected = from.insert(1, 7, 3);
        let mut to = Fixture::new(3);
        to.insert(0, 8, 3);
        let mut donor = from.view(&thread);
        assert_eq!(donor.quick_artefact(1), Some(expected));
        assert_eq!(donor.quick_artefact(0), None);
        assert_eq!(donor.quick_artefact(2), None);
        let item = donor.item_in(index(1)).unwrap();
        let mut recipient = to.view(&thread);
        donor.transfer_artefact(&mut recipient, expected).unwrap();
        assert!(donor.item_in(index(1)).is_none());
        assert_eq!(recipient.item_in(index(1)).unwrap().id(), item.id());
        // SAFETY: fixture anomaly and recipient still own the live object.
        unsafe {
            assert_eq!(read(raw::item::references(item.ptr())), 2);
        }
    }
    #[test]
    fn full_or_absent_containers_preserve_donor() {
        // SAFETY: same bounded fixture scope.
        let thread = unsafe { Thread::new() };
        for count in [0, 1] {
            let mut from = Fixture::new(1);
            let expected = from.insert(0, 7, 3);
            let mut to = Fixture::new(count);
            if count == 1 {
                to.insert(0, 8, 3);
            }
            let mut donor = from.view(&thread);
            let mut recipient = to.view(&thread);
            assert_eq!(
                donor.transfer_artefact(&mut recipient, expected),
                Err(Refused::NoFreeContainer)
            );
            assert!(donor.item_in(index(0)).is_some());
        }
    }
    #[test]
    fn active_or_stale_items_cannot_transfer() {
        // SAFETY: same bounded fixture scope.
        let thread = unsafe { Thread::new() };
        for (id, state) in [(8, 3), (7, 4)] {
            let mut from = Fixture::new(1);
            from.insert(0, id, state);
            let expected = Artefact {
                id: ArtefactId::parse(7).unwrap(),
                slot: index(0),
            };
            let mut to = Fixture::new(1);
            let mut donor = from.view(&thread);
            let mut recipient = to.view(&thread);
            assert_eq!(
                donor.transfer_artefact(&mut recipient, expected),
                Err(Refused::OwnershipChanged)
            );
            assert!(donor.item_in(index(0)).is_some());
        }
    }
}
