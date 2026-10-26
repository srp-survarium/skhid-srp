//! Scaleform GFx as the game's interface code uses it: a GFx value (24
//! bytes: its object interface, its type, its data), and an object's member
//! set through its interface, as the game's own `call [eax+14h]` does
//! (Value::ObjectInterface::SetMember, engine row engine_gfx_set_member).

use core::ffi::CStr;

use super::object::{read, write};
use crate::raw;

/// A GFx value (Scaleform's `GFx::Value`, 18h bytes): the object interface
/// of an object (else 0), the type, the data (an int, a bool, an object's
/// own pointer). The game keeps them in its frames; the movies take them by
/// address.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vostok) struct Value {
    pub interface: u32,
    pub kind: u32,
    pub data: u32,
    pub rest: [u32; 3],
}

const _: () = assert!(size_of::<Value>() == 0x18);

impl Value {
    /// A value's type: an int (VT_Int).
    pub const INT: u32 = 3;
    /// The type's bits that say what it is (VTC_TypeMask: the convert bit
    /// and the low four; the managed bit 40h left out).
    const TYPE_MASK: u32 = 0x8F;
    /// A display object (VT_DisplayObject).
    const DISPLAY_OBJECT: u32 = 0x0A;
}

/// Sets `object`'s member `name` to the value at `value`, as the game sets a
/// description's members (0x238157..0x238183): the object's interface
/// asked through its vtable (slot +14h), with the object's data and whether
/// it is a display object. Its answer (AL, set or not) is not used.
///
/// # Safety
/// `object` is a GFx value of an object (a live interface and data), and
/// `value` a GFx value, both the game's for the duration of the call.
pub(in crate::vostok) unsafe fn set_member(object: *const Value, name: &CStr, value: *const Value) {
    // SAFETY: the caller's: `object` is a live GFx value, read field by
    // field as the game's code reads it.
    let (interface, kind, data) = unsafe {
        let at = object.cast_mut();
        (read(&raw mut (*at).interface), read(&raw mut (*at).kind), read(&raw mut (*at).data))
    };
    let display = u32::from(kind & Value::TYPE_MASK == Value::DISPLAY_OBJECT);
    // SAFETY: the interface is the object's own (its vtable's slot +14h is
    // SetMember, engine_026g.tsv), the name a C string it only reads.
    unsafe {
        raw::engine_gfx_set_member(interface as usize as *mut _, data as usize as *mut _, name.as_ptr().cast(),
                                   value.cast(), display);
    }
}

/// Writes a value's type and data (an int), leaving its interface and the
/// rest as they are, as the game writes a frame's value it reuses.
///
/// # Safety
/// `value` is a GFx value of the game's frame, writable.
pub(in crate::vostok) unsafe fn set_int(value: *mut Value, int: u32) {
    // SAFETY: the caller's.
    unsafe {
        write(&raw mut (*value).kind, Value::INT);
        write(&raw mut (*value).data, int);
    }
}
