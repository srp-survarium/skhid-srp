//! Held-only Lifebone model. The native cook loads a fourth resource for
//! this class; an owned reference lives until the exact item's destructor.
//! Its normal model, inventory state and ground collision are untouched.
use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};
use super::{Context, Lock, Reg, Thread};
use super::world::{Object, pointer, read};
use super::carry_runtime::Scene;
use crate::raw;

struct Model(Object);
impl Model {
    unsafe fn retain(object: Object) -> Self {
        // SAFETY: live query result retains the model through this increment.
        unsafe { AtomicU32::from_ptr(raw::item::references(object.ptr())).fetch_add(1, Ordering::AcqRel); }
        Self(object)
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        // SAFETY: this owner holds one reference, consumed exactly once.
        unsafe {
            let refs = raw::item::references(self.0.ptr());
            if AtomicU32::from_ptr(refs).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(self.0.ptr(), refs);
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
        let requests = (ctx.reg(Reg::Esp).wrapping_add(8 + 0x2c) as *const [[u32; 2]; 3]).read();
        let Some(path) = Object::at(requests[0][0]) else { return false };
        if requests[0][1] != 15 { return false; }
        for (i, byte) in b"item/artefacts/artefact_givokost_single\0".iter().enumerate() {
            if path.ptr().cast::<u8>().add(i).read_volatile() != *byte { return false; }
        }
        let extra = [c"item/artefacts/artefact_givokost_single_held".as_ptr() as u32, 15];
        let extended = [requests[0], requests[1], requests[2], extra];
        let args = (ctx.reg(Reg::Esp).wrapping_add(8) as *const [u32; 5]).read();
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
        let Some(result) = Object::at(ctx.reg(Reg::Ebx)) else { return };
        if read(raw::carry_queries::count(result.ptr())) != 4 { return; }
        let Some(model) = pointer(raw::carry_queries::flower(result.ptr())) else { return };
        FLOWERS.with(ctx.thread(), |slots| {
            if slots.iter().flatten().any(|f| f.item == ctx.reg(Reg::Edi)) { return; }
            if let Some(slot) = slots.iter_mut().find(|s| s.is_none()) {
                *slot = Some(Flower { item: ctx.reg(Reg::Edi), model: Model::retain(model) });
            }
        });
    }
}

pub fn destroy(ctx: &Context<'_>) {
    let Some(item) = Object::at(ctx.reg(Reg::Ecx)) else { return };
    FLOWERS.with(ctx.thread(), |slots| {
        if let Some(slot) = slots.iter_mut().find(|s| s.as_ref().is_some_and(|f| f.item == item.id())) {
            let flower = slot.take().expect("matched flower owner");
            // SAFETY: destructor entry, native client world still live. Like
            // the native destructor, do not submit after its scene is gone.
            unsafe {
                let callback = raw::carry_lifebone::callback(item.ptr()).cast();
                if let Some(world) = pointer(raw::artefact_client::world(callback))
                    && read(raw::game_world::scene(world.ptr())) != 0
                    && let Some(scene) = Scene::of_world(world) { scene.remove(flower.model.0); }
            }
            // Drop consumes the retained reference even after scene teardown.
        }
    });
}

pub(super) fn with_model<R>(thread: &Thread, item: u32, original: Object, f: impl FnOnce(Object) -> R) -> R {
    FLOWERS.with(thread, |slots| {
        let model = slots.iter().flatten().find(|f| f.item == item).map_or(original, |f| f.model.0);
        f(model)
    })
}
