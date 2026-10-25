//! The game's memory through typed views.
//!
//! A patch declares what it reads once, as constants:
//!
//! ```
//! use patch_core::{Field, Obj, Va, Global};
//! /// player_input_handler (a marker: the object's kind).
//! enum InputHandler {}
//! /// Its action mask of the input being built.
//! const ACTIONS: Field<InputHandler, u32> = Field::at(0x33C);
//! /// A game global: the hash mismatch dump's switch.
//! const DUMP_ON: Global<u8> = Global::at(Va(0x1F1D32C));
//! # fn f(handler: Obj<InputHandler>) {
//! let mask = unsafe { handler.get(ACTIONS) };
//! unsafe { handler.set(ACTIONS, mask | 1 << 24) };
//! # }
//! ```
//!
//! and never adds offsets up in its logic: a field of another kind does not
//! type-check, and a pointer field reads as `Option<Obj<K>>` (null is none).
//! The exe is never rebased: a [`Va`] is the address the code runs at.

use core::marker::PhantomData;
use core::ptr::NonNull;

/// An address in the game's process (a VA of the exe, or of what it
/// allocated).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Va(pub u32);

impl Va {
    pub const fn ptr<T>(self) -> *mut T {
        self.0 as usize as *mut T
    }
}

/// A field of an object of kind `K` (a marker type), of type `T`, at its
/// offset.
pub struct Field<K, T> {
    offset: usize,
    kind: PhantomData<fn(K) -> T>,
}

impl<K, T> Clone for Field<K, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, T> Copy for Field<K, T> {}

impl<K, T> Field<K, T> {
    pub const fn at(offset: usize) -> Field<K, T> {
        Field { offset, kind: PhantomData }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }
}

/// An object of the game's of kind `K`, by its address (never null).
#[repr(transparent)]
pub struct Obj<K> {
    at: NonNull<u8>,
    kind: PhantomData<*mut K>,
}

impl<K> Clone for Obj<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Obj<K> {}

impl<K> PartialEq for Obj<K> {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at
    }
}

impl<K> Eq for Obj<K> {}

impl<K> core::fmt::Debug for Obj<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Obj({:p})", self.at)
    }
}

// A pointer field of the game's reads as Option<Obj<K>>: null is none.
const _: () = assert!(size_of::<Option<Obj<u8>>>() == size_of::<*mut u8>());

impl<K> Obj<K> {
    /// The object at `at`; none for null.
    pub fn new(at: *mut u8) -> Option<Obj<K>> {
        NonNull::new(at).map(|at| Obj { at, kind: PhantomData })
    }

    /// The object at a game address; none for 0.
    pub fn at(va: Va) -> Option<Obj<K>> {
        Obj::new(va.ptr())
    }

    pub fn ptr(self) -> *mut u8 {
        self.at.as_ptr()
    }

    pub fn va(self) -> Va {
        Va(self.at.as_ptr() as usize as u32)
    }

    /// Where a field of it is.
    pub fn field_ptr<T>(self, field: Field<K, T>) -> *mut T {
        self.at.as_ptr().wrapping_add(field.offset).cast()
    }

    /// A field's value.
    ///
    /// # Safety
    /// The object is live and its field holds a valid `T` (for a pointer
    /// field, `Option<Obj<_>>`, any value is valid).
    pub unsafe fn get<T: Copy>(self, field: Field<K, T>) -> T {
        unsafe { self.field_ptr(field).read_unaligned() }
    }

    /// Writes a field.
    ///
    /// # Safety
    /// The object is live and the game expects a `T` there.
    pub unsafe fn set<T>(self, field: Field<K, T>, value: T) {
        unsafe { self.field_ptr(field).write_unaligned(value) }
    }

    /// The object embedded in this one at a field (its address, not a
    /// pointer read).
    pub fn embedded<E>(self, field: Field<K, Embedded<E>>) -> Obj<E> {
        Obj { at: unsafe { NonNull::new_unchecked(self.at.as_ptr().wrapping_add(field.offset)) }, kind: PhantomData }
    }

    /// The object as another kind (a base class at offset 0).
    pub fn cast<E>(self) -> Obj<E> {
        Obj { at: self.at, kind: PhantomData }
    }
}

/// The type of a field that is an object of kind `E` in place (not a
/// pointer to it): read with [`Obj::embedded`].
pub enum Embedded<E> {
    #[doc(hidden)]
    Never(core::convert::Infallible, PhantomData<E>),
}

/// A global of the game's at a fixed address.
pub struct Global<T> {
    va: Va,
    kind: PhantomData<fn() -> T>,
}

impl<T> Global<T> {
    pub const fn at(va: Va) -> Global<T> {
        Global { va, kind: PhantomData }
    }

    pub const fn va(&self) -> Va {
        self.va
    }

    /// # Safety
    /// The global holds a valid `T`.
    pub unsafe fn get(&self) -> T
    where
        T: Copy,
    {
        unsafe { self.va.ptr::<T>().read_unaligned() }
    }

    /// # Safety
    /// The game expects a `T` there.
    pub unsafe fn set(&self, value: T) {
        unsafe { self.va.ptr::<T>().write_unaligned(value) }
    }
}

/// A function of the game's at a fixed address that Rust can call
/// directly: `F` is its type, `unsafe extern "C" fn(..)`, `"stdcall"` or
/// `"thiscall"` (ECX the first argument). A function with arguments in other
/// registers is a row of the engine library instead (`engine_026g.tsv`: a
/// cdecl thunk, [`crate::engine`]).
pub struct GameFn<F> {
    va: Va,
    kind: PhantomData<F>,
}

impl<F: Copy> GameFn<F> {
    pub const fn at(va: Va) -> GameFn<F> {
        const { assert!(size_of::<F>() == size_of::<usize>(), "F is a function pointer type") };
        GameFn { va, kind: PhantomData }
    }

    pub const fn va(&self) -> Va {
        self.va
    }

    /// The function, to call.
    ///
    /// # Safety
    /// `F` is the function's true signature and calling convention.
    pub unsafe fn get(&self) -> F {
        let at = self.va.0 as usize;
        unsafe { core::mem::transmute_copy(&at) }
    }
}

/// A function the exe imports (its IAT slot): `F` as for [`GameFn`],
/// `stdcall` for Windows' own.
pub struct Import<F> {
    slot: Va,
    kind: PhantomData<F>,
}

impl<F: Copy> Import<F> {
    pub const fn at(slot: Va) -> Import<F> {
        const { assert!(size_of::<F>() == size_of::<usize>(), "F is a function pointer type") };
        Import { slot, kind: PhantomData }
    }

    /// The function the loader put in the slot.
    ///
    /// # Safety
    /// `F` is the import's true signature; the exe is loaded.
    pub unsafe fn get(&self) -> F {
        unsafe { self.slot.ptr::<F>().read_unaligned() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Thing {}
    enum Inner {}
    const COUNT: Field<Thing, u32> = Field::at(4);
    const NEXT: Field<Thing, Option<Obj<Thing>>> = Field::at(8);
    const INNER: Field<Thing, Embedded<Inner>> = Field::at(16);
    const FLAG: Field<Inner, u8> = Field::at(1);

    #[test]
    fn fields_read_and_write_by_offset() {
        let mut memory = [0u8; 32];
        let thing: Obj<Thing> = Obj::new(memory.as_mut_ptr()).unwrap();
        unsafe {
            thing.set(COUNT, 0x1234_5678);
            assert_eq!(thing.get(COUNT), 0x1234_5678);
            assert_eq!(thing.get(NEXT), None);
            thing.set(NEXT, Some(thing));
            assert_eq!(thing.get(NEXT), Some(thing));
            thing.embedded(INNER).set(FLAG, 7);
        }
        assert_eq!(memory[4..8], 0x1234_5678u32.to_le_bytes());
        assert_eq!(memory[17], 7);
        assert!(Obj::<Thing>::new(core::ptr::null_mut()).is_none());
    }
}
