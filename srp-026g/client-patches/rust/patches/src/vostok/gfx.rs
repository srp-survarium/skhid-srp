//! Scaleform GFx as the game's interface code uses it: a GFx value (24
//! bytes: its object interface, its type, its data), and an object's member
//! set through its interface, as the game's own `call [eax+14h]` does
//! (Value::ObjectInterface::SetMember, engine row engine_gfx_set_member); a
//! movie's ActionScript method invoked (Movie::Invoke 0x511600, engine row
//! engine_movie_invoke), the movie found through its owner as the game's
//! interfaces find theirs ([[owner+108h]+4]).

use core::ffi::CStr;
use core::marker::PhantomData;

use super::hook::Thread;
use super::object::{Object, pointer, read, write};
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
    /// A value's type: a bool (VT_Boolean; its value at +8, 0x2384E2).
    pub const BOOL: u32 = 2;
    /// An int (VT_Int).
    pub const INT: u32 = 3;
    /// An unsigned int (VT_UInt).
    pub const UINT: u32 = 4;
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

/// A movie of the game's interface (the hud's, the player icons', the
/// lobby's): what Movie::Invoke takes in ECX.
#[derive(Clone, Copy)]
pub struct Movie<'t> {
    movie: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Movie<'t> {
    /// Which movie this is (a new game world's is another), to tell it once.
    pub fn id(&self) -> usize {
        self.movie.ptr() as usize
    }

    /// Invokes the movie's `method` with one bool (Movie::Invoke 0x511600, no
    /// result); whether the movie took it (it has that method: its root is
    /// made).
    pub fn invoke_bool(&self, method: &CStr, value: bool) -> bool {
        let argument = Value { interface: 0, kind: Value::BOOL, data: u32::from(value), rest: [0; 3] };
        // SAFETY: a live movie on the hook's thread; the name is a C string and
        // the one argument a GFx bool, as the game's own Invokes pass them.
        let taken = unsafe {
            raw::engine_movie_invoke(self.movie.ptr(), method.as_ptr().cast(), core::ptr::null_mut(),
                                     (&raw const argument).cast(), 1)
        };
        taken & 0xFF != 0
    }

    /// The movie of `owner` (a movie's owner: [[owner+108h]+4]), or none.
    ///
    /// # Safety
    /// `owner` is a live movie owner (the hud's +8 or +10h, the lobby
    /// menu's +690h), its slot 0 or a live slot.
    pub(in crate::vostok) unsafe fn of_owner(owner: Object) -> Option<Movie<'t>> {
        // SAFETY: the caller's.
        let slot = unsafe { pointer(raw::movie_holder::view(owner.ptr())) }?;
        // SAFETY: a live slot; its +4 is its movie or 0.
        unsafe { pointer(raw::movie_view::movie(slot.ptr())) }.map(|movie| Movie { movie, thread: PhantomData })
    }

    fn invoke(&self, method: &CStr, result: *mut Value, args: &[Value]) {
        // SAFETY: a live movie (of_owner's); Invoke reads the method's name
        // and the arguments and writes the answer, all ours for the call.
        unsafe {
            raw::engine_movie_invoke(self.movie.ptr(), method.as_ptr().cast(), result.cast(), args.as_ptr().cast(),
                                     args.len() as u32);
        }
    }

    /// `method()`, no argument, no answer.
    pub fn invoke_none(&self, method: &CStr) {
        // SAFETY: as invoke: no result, no arguments (null, 0).
        unsafe {
            raw::engine_movie_invoke(self.movie.ptr(), method.as_ptr().cast(), core::ptr::null_mut(),
                                     core::ptr::null(), 0);
        }
    }

    /// `method()`, no argument, its answer's data's low byte not 0 (a bool's
    /// value; a method the movie has not: false).
    pub fn ask_bool(&self, method: &CStr) -> bool {
        let mut answer = Value { interface: 0, kind: 0, data: 0, rest: [0; 3] };
        // SAFETY: as invoke: the answer a GFx value of ours (a bool: nothing
        // to release), no arguments (null, 0).
        unsafe {
            raw::engine_movie_invoke(self.movie.ptr(), method.as_ptr().cast(), (&raw mut answer).cast(),
                                     core::ptr::null(), 0);
        }
        answer.data & 0xFF != 0
    }

    /// `method(value)`, an unsigned int, no answer.
    pub fn invoke_uint(&self, method: &CStr, value: u32) {
        let arg = Value { interface: 0, kind: Value::UINT, data: value, rest: [0; 3] };
        self.invoke(method, core::ptr::null_mut(), core::slice::from_ref(&arg));
    }
}
