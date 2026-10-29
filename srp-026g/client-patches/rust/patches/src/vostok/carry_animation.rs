//! Independent native animation storage for a carry adapter. Preparation
//! shares only immutable clip resources, never a weapon's mutable selector.
//! This owner is not yet installed as an active object or bound to a user.

use core::ffi::c_void;
use core::mem::size_of;

use super::hook::Thread;
use super::world::{Object, pointer, read};
use crate::raw;

/// The native factory allocates this separately for each portable object.
/// Its internal selector nodes point back to this storage, so it must live
/// at one stable heap address until both native selectors are destroyed.
#[repr(C, align(8))]
struct Storage([u8; 0x200]);

/// Owned, unbound portable animation state. No method exposes its pointer
/// or permits attaching a transient player handle. A later registered
/// adapter must own the bound lifecycle, including event unsubscription.
pub struct Prepared<'t> {
    storage: Object,
    allocator: Object,
    _thread: &'t Thread,
}

impl<'t> Prepared<'t> {
    pub(in crate::vostok) fn for_player(
        player: Object,
        seed: u32,
        thread: &'t Thread,
    ) -> Option<Self> {
        let resource = Self::resource_for(player)?;
        // SAFETY: the global allocator used by the native portable factory.
        let allocator = unsafe { pointer(raw::image::ALLOCATOR as *mut u32)? };
        Self::prepare(resource, allocator, seed, thread)
    }

    fn resource_for(player: Object) -> Option<Object> {
        // SAFETY: the Player handle's live native object. Parse the held
        // interface by its known weapon vtables before reading
        // weapon-specific storage. No virtual call or pointer arithmetic
        // treats batteries or empty hands as a weapon.
        unsafe {
            let held = pointer(raw::player::held_item(player.ptr()))?;
            let table = read(raw::active_item::vtable(held.ptr()));
            if ![raw::image::WEAPON_VTABLE, raw::image::WEAPON_CORE_VTABLE].contains(&table) {
                return None;
            }
            let source = pointer(raw::weapon_core::portable(held.ptr()))?;
            pointer(raw::portable_core::user_animations(source.ptr()))
        }
    }

    fn prepare(resource: Object, allocator: Object, seed: u32, thread: &'t Thread) -> Option<Self> {
        // SAFETY: the engine heap returns stable storage with its normal
        // object alignment. The constructor owns its independently allocated
        // nodes. The resource binding retains the immutable source resource
        // twice (selector and hit selector), so the weapon may later vanish.
        let storage = unsafe {
            let allocated = raw::engine_malloc(
                allocator.ptr(),
                size_of::<Storage>() as u32,
                c"artefact carry animations".as_ptr().cast(),
            );
            let storage = Object::at(allocated as u32)?;
            storage.ptr().cast::<Storage>().write(Storage([0; 0x200]));
            raw::engine_portable_construct(storage.ptr(), seed);
            let resource = resource.id();
            raw::engine_portable_bind_resource(storage.ptr(), &resource);
            storage
        };
        Some(Self {
            storage,
            allocator,
            _thread: thread,
        })
    }
}

#[cfg(test)]
#[path = "carry_animation/tests.rs"]
mod tests;

impl Drop for Prepared<'_> {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner, and preparation never attaches a
        // user or subscribes to player events. Match the original portable
        // factories' teardown ordering: hit resource/callback, selector
        // graph/resource, then the stable backing allocation. Object's
        // NonNull makes this owner !Send/!Sync. The Thread borrow also
        // bounds preparation and teardown to the native hook's lifetime.
        unsafe {
            raw::engine_hit_selector_destroy(
                raw::portable_core::hit_selector(self.storage.ptr()).cast(),
            );
            raw::engine_selector_destroy(raw::portable_core::selector(self.storage.ptr()).cast());
            let mut pointer = self.storage.ptr();
            raw::engine_free(
                self.allocator.ptr(),
                self.storage.ptr(),
                &mut pointer as *mut *mut c_void,
            );
        }
    }
}
