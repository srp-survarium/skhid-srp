//! Dropped native artefacts: world-owned resources, synchronized release state.
use artefact_carry::{Artefact, ArtefactId, Refused};
use artefact_carry::carry::Time;
use artefact_carry::ground::Drop;
use super::{Context, Lock, Thread, site};
use super::world::{Player, read};
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

pub struct GroundItem<'t> { pub(super) item: raw::Artefact, key: Key, thread: &'t Thread }
impl<'t> GroundItem<'t> {
    pub(super) fn of_item(item: raw::Artefact, thread: &'t Thread) -> Option<Self> {
        // SAFETY: live inventory/registry/artefact callback argument.
        unsafe {
            let id = ArtefactId::parse(read(item.id()))?;
            let world = item.item().world()?;
            Some(Self { item, key: Key { world: world.address(), item: item.address(), id }, thread })
        }
    }
    pub fn id(&self) -> ArtefactId { self.key.id }
    pub fn position(&self, now: Time) -> Option<[f32; 3]> { self.release().map(|d| d.position(now)) }
    fn release(&self) -> Option<Drop> {
        // SAFETY: a world-owned live item, re-resolved on every interaction.
        if unsafe { read(self.item.state()) != DROPPED || self.item.item().inventory().is_some() } { return None; }
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
        unsafe { self.item.state().write(DROPPED); }
        self.store(Some(drop));
    }
}

impl<'t> Player<'t> {
    pub fn ground_item(&self, id: ArtefactId) -> Option<GroundItem<'t>> {
        // SAFETY: live core world owns a vector of registered artefact refs;
        // its bounded length is checked before indexing (native deserialize).
        unsafe {
            let world = self.this.world_core()?;
            let first = world.artefacts_begin()?;
            let end = world.artefacts_end()?;
            let count = end.index_from(first)?;
            if !first.address().is_multiple_of(4) || count > 255 || usize::from(id.get()) >= count { return None; }
            let item = first.at(usize::from(id.get())).item()?.artefact();
            let view = GroundItem::of_item(item, self.thread)?;
            (view.key.world == world.address() && view.id() == id).then_some(view)
        }
    }
    /// Drops stay where they fall for the rest of the match, so one is only
    /// made where it can be reached: on this side of a wall in front of the
    /// palm, above level collision. Otherwise the native death keeps it.
    pub fn drop_carried(&self, artefact: Artefact) -> bool {
        let Some(physics) = self.physics_world() else { return false };
        let (eye, forward) = self.carry_eye();
        let reach = 0.367_695_5; // |(0.34, 0.14)|
        let palm = [forward[0] * 0.34, forward[1] * 0.34 - 0.14, forward[2] * 0.34];
        let ray = palm.map(|v| v / reach);
        // Pulled back 5 cm from a wall between the eye and the palm.
        let along = physics.ray_test(eye, ray, reach, 0x24, 2)
            .map_or(reach, |h| (0..3).map(|n| (h.point[n] - eye[n]) * ray[n]).sum::<f32>() - 0.05).max(0.);
        let origin: [f32; 3] = core::array::from_fn(|n| eye[n] + ray[n] * along);
        // Include a small clearance above the palm; the ray lands on actual
        // level collision, including stairs. No visual-frame data enters sim.
        let start = [origin[0], origin[1] + 0.1, origin[2]];
        let Some(floor) = physics.ray_test(start, [0., -1., 0.], 200., 0x24, 2) else { return false };
        let Some(drop) = Drop::new(origin, floor.point[1].min(origin[1]), self.carry_time()) else { return false };
        self.inventory().is_some_and(|mut i| i.drop_carried(artefact, drop))
    }
    pub fn container_space(&self) -> ContainerSpace {
        self.inventory().map(|i| i.container_space()).unwrap_or(ContainerSpace::Missing)
    }
}

/// The record of a state-6 item without a release (none should be): the
/// load reads it as no release, and the stream stays the same length.
const NO_RELEASE: [u8; 20] = [0xFF; 20];

pub fn save(ctx: &Context<'_, site::CarryGroundSave>) {
    // artefact_base::serialize after its state-dependent fields.
    let (Some(item), Some(writer)) = (raw::site::carry_ground_save::item(ctx), raw::site::carry_ground_save::writer(ctx)) else { return };
    // SAFETY: the live artefact being serialized.
    if unsafe { read(item.state()) } != DROPPED { return; }
    // State 6 always has the extension; the native state byte is its tag.
    let record = GroundItem::of_item(item, ctx.thread()).and_then(|i| i.release()).map_or(NO_RELEASE, |d| d.snapshot());
    super::carry_runtime::write_bytes(writer, &record);
}
pub fn load(ctx: &Context<'_, site::CarryGroundLoad>) {
    let Some(item) = raw::site::carry_ground_load::item(ctx).and_then(|i| GroundItem::of_item(i, ctx.thread())) else { return };
    // SAFETY: deserialize just read this live artefact's native state byte.
    let drop = if unsafe { read(item.item.state()) } == DROPPED {
        raw::site::carry_ground_load::reader(ctx).and_then(super::carry_runtime::read_bytes::<20>).and_then(Drop::restore)

    } else { None };
    item.store(drop);
}
