//! A match's options as the static info gives them: survarium::match_options
//! (0x578A0 bytes, read by match_options::deserialize 0x1F0E70) and the
//! reader of the message's bytes it read from, after the read, at the sites
//! `static_info.options_read` (process_static_match_info, 0x27B92D: ESI the
//! options, the reader its frame's local at EBP-2Ch) and
//! `replay.options_read` (a replay's chunk 1, 0x1F0971: the options its
//! first argument, the reader its local; both by the site's ESP).

use core::ffi::c_void;
use core::ptr::NonNull;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::parse::Reader;
use crate::raw;

/// The options just read and the reader they were read from (its cursor
/// where the options end).
pub struct OptionsRead<'t> {
    options: NonNull<c_void>,
    reader: NonNull<c_void>,
    _thread: &'t Thread,
}

fn non_null(raw: u32) -> Option<NonNull<c_void>> {
    NonNull::new(raw as usize as *mut c_void)
}

impl<'t> OptionsRead<'t> {
    /// At `static_info.options_read`.
    pub fn at_static_info(ctx: &'t Context<'_>) -> Option<OptionsRead<'t>> {
        let options = non_null(ctx.reg(Reg::Esi))?;
        let frame = non_null(ctx.reg(Reg::Ebp))?;
        // SAFETY: the reader is the frame's local (static_info_frame, by EBP).
        let reader = non_null(unsafe { raw::static_info_frame::reader(frame.as_ptr()) } as usize as u32)?;
        Some(OptionsRead { options, reader, _thread: ctx.thread() })
    }

    /// At `replay.options_read`.
    pub fn at_replay(ctx: &'t Context<'_>) -> Option<OptionsRead<'t>> {
        // Context already removes the dispatch slot and flags from saved ESP.
        let frame = non_null(ctx.reg(Reg::Esp))?;
        // SAFETY: the frame's argument and local (replay_options_frame, by the site's ESP).
        let (options, reader) = unsafe {
            (*raw::replay_options_frame::options(frame.as_ptr()), raw::replay_options_frame::reader(frame.as_ptr()))
        };
        Some(OptionsRead { options: non_null(options)?, reader: non_null(reader as usize as u32)?, _thread: ctx.thread() })
    }

    /// The bytes after the options (srp's block, if any): from the reader's
    /// cursor to its end; none when the cursor is outside its bytes.
    pub fn rest(&self) -> Reader<'t> {
        let this = self.reader.as_ptr();
        // SAFETY: a live reader; its three fields are dwords.
        let (begin, cursor, size) = unsafe {
            (*raw::buffer_reader::begin(this), *raw::buffer_reader::cursor(this), *raw::buffer_reader::size(this))
        };
        let end = begin.wrapping_add(size);
        if cursor == 0 || cursor < begin || cursor > end || end < begin {
            return Reader::new(&[]);
        }
        // SAFETY: begin..begin+size are the message's bytes, alive for the hook,
        // and cursor..end lies inside them.
        unsafe { Reader::of_memory(cursor as usize as *const u8, (end - cursor) as usize) }
    }

    /// Patch 65's mark in the options (+57870h): "SRBO" for a Blackout
    /// match, else zeros.
    pub fn set_blackout_mark(&mut self, blackout: bool) {
        let mark = u32::from_le_bytes(*b"SRBO");
        // SAFETY: live options; +57870h is in their unused "Mode" string.
        unsafe { *raw::match_options::srp_blackout(self.options.as_ptr()) = if blackout { mark } else { 0 } }
    }
}
