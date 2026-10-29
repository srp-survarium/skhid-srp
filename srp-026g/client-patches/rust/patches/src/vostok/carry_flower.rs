//! Held-only Lifebone model. The native cook loads a fourth resource for
//! this class; an owned reference lives until the exact item's destructor.
//! Its normal model, inventory state and ground collision are untouched.
use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};
use super::{Context, Lock, Reg, Thread};
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
// Native artefacts use an 8-bit registry. If construction transiently fills
// the pool, retain nothing and leave that item using its original model.
static FLOWERS: Lock<[Option<Flower>; 256]> = Lock::new([const { None }; 256]);

pub fn query(ctx: &Context<'_>) -> bool {
    // SAFETY: 0x25E91A, the native cook's three resource requests. At this
    // call site stack has the five arguments, but no call return address.
    unsafe {
        let Some(frame) = raw::site::carry_flower_query::frame(ctx) else { return false };
        let requests = frame.requests().cast::<[[u32; 2]; 3]>().read();
        let path = requests[0][0] as *const u8;
        if path.is_null() { return false; }
        if requests[0][1] != 15 { return false; }
        for (i, byte) in b"item/artefacts/artefact_givokost_single\0".iter().enumerate() {
            if path.add(i).read_volatile() != *byte { return false; }
        }
        let extra = [c"item/artefacts/artefact_givokost_single_held".as_ptr() as u32, 15];
        let extended = [requests[0], requests[1], requests[2], extra];
        let args = frame.arguments().cast::<[u32; 5]>().read();
        // Query copies requests synchronously, just as it copies the native
        // cook's stack-local array. Callback and parent ownership stay native.
        raw::engine_query_resources(extended.as_ptr().cast(), 4, args[0] as *mut c_void,
            args[1] as *mut c_void, args[2] as *mut c_void, args[3] as *mut c_void, args[4]);
    }
    true
}

pub fn ready(ctx: &Context<'_>) {
    // SAFETY: Lifebone branch at 0x260278: EDI is the live artefact, EBX
    // the result passed to its native assignment routine. Count is checked
    // before reading the appended query (header 50h, stride 2E0h).
    unsafe {
        let Some(result) = raw::site::carry_flower_ready::result(ctx) else { return };
        if read(result.count()) != 4 { return; }
        let Some(model) = result.flower() else { return };
        FLOWERS.with(ctx.thread(), |slots| {
            if slots.iter().flatten().any(|f| f.item == ctx.reg(Reg::Edi)) { return; }
            if let Some(slot) = slots.iter_mut().find(|s| s.is_none()) {
                *slot = Some(Flower { item: ctx.reg(Reg::Edi), model: Model::retain(model) });
            }
        });
    }
}

pub fn destroy(ctx: &Context<'_>) {
    // SAFETY: native Lifebone destructor argument.
    let Some(item) = (unsafe { raw::site::carry_flower_destroy::item(ctx) }) else { return };
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
