//! Dropped native artefacts: world-owned resources, synchronized release state.
use artefact_carry::{Artefact, ArtefactId, Refused};
use artefact_carry::carry::Time;
use artefact_carry::ground::Drop;
use super::{Context, Lock, Reg, Thread};
use super::world::{Object, Player, pointer, read};
use super::inventory::{ContainerSpace, Inventory};
use crate::raw;

// State 6 is outside native 0..5. The anomaly only respawns state 0;
// unowned state 6 keeps ticking its cooldown and serializes it normally.
pub(super) const DROPPED: u32 = 6;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Key { world: u32, item: u32, id: ArtefactId }
#[derive(Clone, Copy)]
struct Released { key: Key, drop: Drop }
static RELEASED: Lock<[Option<Released>; 255]> = Lock::new([None; 255]);

pub struct GroundItem<'t> { pub(super) item: Object, key: Key, thread: &'t Thread }
impl<'t> GroundItem<'t> {
    pub(super) fn of_item(item: Object, thread: &'t Thread) -> Option<Self> {
        // SAFETY: live inventory/registry/artefact callback argument.
        unsafe {
            let id = ArtefactId::parse(read(raw::artefact::id(item.ptr())))?;
            let world = pointer(raw::item::world(item.ptr()))?;
            Some(Self { item, key: Key { world: world.id(), item: item.id(), id }, thread })
        }
    }
    pub fn id(&self) -> ArtefactId { self.key.id }
    pub fn position(&self, now: Time) -> Option<[f32; 3]> { self.release().map(|d| d.position(now)) }
    fn release(&self) -> Option<Drop> {
        // SAFETY: a world-owned live item, re-resolved on every interaction.
        if unsafe { read(raw::artefact::state(self.item.ptr())) != DROPPED || read(raw::item::inventory(self.item.ptr())) != 0 } { return None; }
        RELEASED.with(self.thread, |all| all[usize::from(self.key.id.get())].filter(|d| d.key == self.key).map(|d| d.drop))
    }
    fn store(&self, drop: Option<Drop>) {
        RELEASED.with(self.thread, |all| all[usize::from(self.key.id.get())] = drop.map(|drop| Released { key: self.key, drop }));
    }
    pub fn take(&self, inventory: &mut Inventory<'t>) -> Result<(), Refused> {
        if self.release().is_none() { return Err(Refused::OwnershipChanged); }
        inventory.take_ground(self)?;
        self.store(None);
        Ok(())
    }
    pub(super) fn detached(&self) -> bool { self.release().is_some() }
    pub(super) fn release_from(&self, drop: Drop) {
        // SAFETY: caller removed this exact passive item from its inventory;
        // the world registry and anomaly retain it through the transition.
        unsafe { raw::artefact::state(self.item.ptr()).write(DROPPED); }
        self.store(Some(drop));
    }
}

impl<'t> Player<'t> {
    pub fn ground_item(&self, id: ArtefactId) -> Option<GroundItem<'t>> {
        // SAFETY: live core world owns a vector of registered artefact refs;
        // its bounded length is checked before indexing (native deserialize).
        unsafe {
            let world = pointer(raw::player::world_core(self.this.ptr()))?;
            let first = read(raw::core_world::artefacts_begin(world.ptr()));
            let end = read(raw::core_world::artefacts_end(world.ptr()));
            let bytes = end.checked_sub(first)?;
            if first == 0 || !first.is_multiple_of(4) || !bytes.is_multiple_of(4) || bytes / 4 > 255 || u32::from(id.get()) >= bytes / 4 { return None; }
            let item = pointer((first + u32::from(id.get()) * 4) as *mut u32)?;
            let view = GroundItem::of_item(item, self.thread)?;
            (view.key.world == world.id() && view.id() == id).then_some(view)
        }
    }
    pub fn drop_carried(&self, artefact: Artefact) -> bool {
        let Some(physics) = self.physics_world() else { return false };
        let (eye, forward) = self.carry_eye();
        let origin = [eye[0] + forward[0] * 0.34, eye[1] - 0.14 + forward[1] * 0.34, eye[2] + forward[2] * 0.34];
        // Include a small clearance above the palm; the ray lands on actual
        // level collision, including stairs. No visual-frame data enters sim.
        let start = [origin[0], origin[1] + 0.1, origin[2]];
        let floor = physics.ray_test(start, [0., -1., 0.], 200., 0x24, 2)
            .map(|h| h.point[1].min(origin[1])).unwrap_or(origin[1] - 200.);
        let Some(drop) = Drop::new(origin, floor, self.carry_time()) else { return false };
        self.inventory().is_some_and(|mut i| i.drop_carried(artefact, drop))
    }
    pub fn container_space(&self) -> ContainerSpace {
        self.inventory().map(|i| i.container_space()).unwrap_or(ContainerSpace::Missing)
    }
}

pub fn save(ctx: &Context<'_>) {
    let Some(item) = Object::at(ctx.reg(Reg::Esi)).and_then(|i| GroundItem::of_item(i, ctx.thread())) else { return };
    // Only state 6 has an extension; the native state byte is its tag.
    if let Some(drop) = item.release() { super::carry_runtime::write_bytes(ctx, Reg::Edi, &drop.snapshot()); }
}
pub fn load(ctx: &Context<'_>) {
    let Some(item) = Object::at(ctx.reg(Reg::Ebx)).and_then(|i| GroundItem::of_item(i, ctx.thread())) else { return };
    // SAFETY: deserialize just read this live artefact's native state byte.
    let drop = if unsafe { read(raw::artefact::state(item.item.ptr())) } == DROPPED {
        super::carry_runtime::read_bytes::<20>(ctx, Reg::Edi).and_then(Drop::restore)
    } else { None };
    item.store(drop);
}
