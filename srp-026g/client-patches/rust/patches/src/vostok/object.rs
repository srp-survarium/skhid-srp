//! A game object as the handles hold it, and how a field is read or
//! written: the primitives every handle of `vostok` is built on. A field's
//! address is the `raw` layer's (fields_026g.tsv); what it means is the
//! handle's.

use core::ffi::c_void;
use core::ptr::NonNull;

/// A game object's address, never null.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vostok) struct Object(NonNull<c_void>);

impl Object {
    pub(in crate::vostok) fn at(raw: u32) -> Option<Object> {
        NonNull::new(raw as usize as *mut c_void).map(Object)
    }

    pub(in crate::vostok) fn ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }

    /// The address as a value (as the game's registers hold it): an id a
    /// patch may keep between hooks and compare, never read through.
    pub(in crate::vostok) fn id(self) -> u32 {
        self.0.as_ptr() as usize as u32
    }
}

/// Reads a field (its address from `raw`), as the game's code does: one
/// access of the field's width, never cached.
///
/// # Safety
/// `at` is the field's address in a live object of its kind.
pub(in crate::vostok) unsafe fn read<T: Copy>(at: *mut T) -> T {
    unsafe { at.read_volatile() }
}

/// Writes a field.
///
/// # Safety
/// As `read`; the game lets this code write the field there.
#[allow(dead_code)] // the ports that follow
pub(in crate::vostok) unsafe fn write<T: Copy>(at: *mut T, value: T) {
    unsafe { at.write_volatile(value) }
}

/// The object a pointer field names (0: none).
///
/// # Safety
/// As `read`.
pub(in crate::vostok) unsafe fn pointer(at: *mut u32) -> Option<Object> {
    Object::at(unsafe { read(at) })
}

/// `N` f32 from a field on.
///
/// # Safety
/// As `read`, for the whole run.
pub(in crate::vostok) unsafe fn floats<const N: usize>(at: *mut f32) -> [f32; N] {
    unsafe { at.cast::<[f32; N]>().read_unaligned() }
}

/// An object's vtable (its first dword), the address alone: compared with
/// the `raw::image` constants, never read through here.
///
/// # Safety
/// `object` is a live object with a vtable.
pub(in crate::vostok) unsafe fn vtable(object: Object) -> u32 {
    unsafe { read(object.ptr().cast::<u32>()) }
}
