//! A match's options as the static info gives them: survarium::match_options
//! (0x578A0 bytes, read by match_options::deserialize 0x1F0E70) and the
//! reader of the message's bytes it reads from.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::hook::{Handle, Thread};
use super::parse::Reader;
use crate::raw;

/// survarium::match_options, as a hook is given it.
pub struct MatchOptions<'t> {
    this: NonNull<c_void>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Handle<'t> for MatchOptions<'t> {
    /// # Safety
    /// `raw` is a match_options (or 0), alive for the hook, on `_`'s thread.
    unsafe fn from_raw(_: &'t Thread, raw: u32) -> Option<Self> {
        NonNull::new(raw as usize as *mut c_void).map(|this| MatchOptions { this, thread: PhantomData })
    }
}

impl MatchOptions<'_> {
    /// Patch 65's mark in the options (+57870h): "SRBO" for a Blackout
    /// match, else zeros.
    pub fn set_blackout_mark(&mut self, blackout: bool) {
        const MARK: u32 = u32::from_le_bytes(*b"SRBO");
        // SAFETY: the handle is a live match_options; +57870h is in its unused "Mode" string.
        unsafe { *raw::match_options::srp_blackout(self.this.as_ptr()) = if blackout { MARK } else { 0 } }
    }
}

/// The game's reader of a message's bytes {begin, cursor, size}
/// (match_options::deserialize's EAX), as a hook is given it.
pub struct BufferReader<'t> {
    this: NonNull<c_void>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Handle<'t> for BufferReader<'t> {
    /// # Safety
    /// `raw` is the address of a buffer reader (or 0) whose bytes are alive
    /// for the hook, on `_`'s thread.
    unsafe fn from_raw(_: &'t Thread, raw: u32) -> Option<Self> {
        NonNull::new(raw as usize as *mut c_void).map(|this| BufferReader { this, thread: PhantomData })
    }
}

impl<'t> BufferReader<'t> {
    /// The bytes not read yet: from the cursor to the end; none when the
    /// cursor is outside the bytes.
    pub fn rest(&self) -> Reader<'t> {
        let this = self.this.as_ptr();
        // SAFETY: the handle is a live buffer reader; its three fields are dwords.
        let (begin, cursor, size) = unsafe {
            (*raw::buffer_reader::begin(this), *raw::buffer_reader::cursor(this), *raw::buffer_reader::size(this))
        };
        let end = begin.wrapping_add(size);
        if cursor == 0 || cursor < begin || cursor > end || end < begin {
            return Reader::new(&[]);
        }
        // SAFETY: begin..begin+size are the message's bytes, alive for the hook
        // (from_raw's contract), and cursor..end lies inside them.
        unsafe { Reader::of_memory(cursor as usize as *const u8, (end - cursor) as usize) }
    }
}
