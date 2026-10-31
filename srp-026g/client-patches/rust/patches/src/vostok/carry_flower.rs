//! Held-only Lifebone model. The native cook loads a fourth resource for
//! this class; an owned reference lives until the exact item's destructor.
//! Its normal model, inventory state and ground collision are untouched.
use core::sync::atomic::{AtomicU32, Ordering};
use super::{Context, Lock, Thread, site};
use super::world::{read};
use super::carry_runtime::Scene;
use crate::raw;

struct Model(raw::SceneModel);
impl Model {
    unsafe fn retain(object: raw::SceneModel) -> Self {
        // SAFETY: live query result retains the model through this increment.
        unsafe { AtomicU32::from_ptr(object.references()).fetch_add(1, Ordering::AcqRel); }
        Self(object)
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        // SAFETY: this owner holds one reference, consumed exactly once.
        unsafe {
            let refs = self.0.references();
            if AtomicU32::from_ptr(refs).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_model(Some(self.0), refs);
            }
        }
    }
}
struct Flower { item: u32, model: Model }
// SAFETY: this private owner crosses native callback threads only inside
// FLOWERS. Its model has a retained, atomic resource reference; no Flower
// or Model escapes the registry. Rendering stays in the native scene
// callbacks, and the item's destructor alone removes and drops its owner.
// The lock serializes query completion, presentation and destruction.
unsafe impl Send for Flower {}
// Native artefacts use an 8-bit registry. If construction transiently fills
// the pool, retain nothing and leave that item using its original model.
static FLOWERS: Lock<[Option<Flower>; 256]> = Lock::new([const { None }; 256]);

pub fn query(ctx: &Context<'_, site::CarryFlowerQuery>) -> bool {
    let Some(frame) = raw::site::carry_flower_query::frame(ctx) else { return false };
    // SAFETY: 0x25E91A, the native cook's three resource requests. At this
    // call site stack has the five arguments, but no call return address.
    unsafe {
        let first = frame.requests();
        let path = first.path();
        if path.is_null() || read(first.class()) != 15 { return false; }
        for (i, byte) in b"item/artefacts/artefact_givokost_single\0".iter().enumerate() {
            if path.add(i).read_volatile() != *byte { return false; }
        }
        // The native three as they are, then the held model: {path, class}.
        let request = |n| { let r = first.at(n); [r.path() as usize as u32, read(r.class())] };
        let extended = [request(0), request(1), request(2),
            [c"item/artefacts/artefact_givokost_single_held".as_ptr() as usize as u32, 15]];
        // Query copies requests synchronously, just as it copies the native
        // cook's stack-local array. Callback and parent ownership stay native.
        raw::engine_query_resources(extended.as_ptr().cast(), 4, frame.callback().cast(),
            frame.allocator().cast(), frame.user_data().cast(), frame.parent().cast(),
            read(frame.flag()));
    }
    true
}

pub fn ready(ctx: &Context<'_, site::CarryFlowerReady>) {
    let Some(result) = raw::site::carry_flower_ready::result(ctx) else { return };
    let Some(item) = raw::site::carry_flower_ready::item(ctx).map(|i| i.address()) else { return };
    // SAFETY: Lifebone branch at 0x260278: EDI is the live artefact, EBX
    // the result passed to its native assignment routine. Count is checked
    // before reading the appended query (header 50h, stride 2E0h).
    unsafe {
        if read(result.count()) != 4 { return; }
        let Some(model) = result.flower() else { return };
        FLOWERS.with(ctx.thread(), |slots| {
            if slots.iter().flatten().any(|f| f.item == item) { return; }
            if let Some(slot) = slots.iter_mut().find(|s| s.is_none()) {
                *slot = Some(Flower { item, model: Model::retain(model) });
            }
        });
    }
}

pub fn destroy(ctx: &Context<'_, site::CarryFlowerDestroy>) {
    let Some(item) = raw::site::carry_flower_destroy::item(ctx) else { return };

    FLOWERS.with(ctx.thread(), |slots| {
        if let Some(slot) = slots.iter_mut().find(|s| s.as_ref().is_some_and(|f| f.item == item.address())) {
            let flower = slot.take().expect("matched flower owner");
            // SAFETY: destructor entry, native client world still live. Like
            // the native destructor, do not submit after its scene is gone.
            unsafe {
                let callback = item.callback();
                if let Some(world) = callback.world()
                    && read(world.scene().value()) != 0
                    && let Some(scene) = Scene::of_world(world) { scene.remove(flower.model.0); }
            }
            // Drop consumes the retained reference even after scene teardown.
        }
    });
}

pub(super) fn with_model<R>(thread: &Thread, item: u32, original: raw::SceneModel, f: impl FnOnce(raw::SceneModel) -> R) -> R {
    FLOWERS.with(thread, |slots| {
        let model = slots.iter().flatten().find(|f| f.item == item).map_or(original, |f| f.model.0);
        f(model)
    })
}
