//! A match's options as the static info gives them: survarium::match_options
//! (0x578A0 bytes, read by match_options::deserialize 0x1F0E70) and the
//! reader of the message's bytes it read from, after the read, at the sites
//! `static_info.options_read` (process_static_match_info, 0x27B92D: ESI the
//! options, the reader its frame's local at EBP-2Ch) and
//! `replay.options_read` (a replay's chunk 1, 0x1F0971: the options its
//! first argument, the reader its local; both by the site's ESP).

use super::context::Context;
use super::site;
use super::hook::Thread;
use super::parse::Reader;
use crate::raw;

/// The options just read and the reader they were read from (its cursor
/// where the options end).
pub struct OptionsRead<'t> {
    options: raw::MatchOptions,
    reader: raw::BufferReader,
    _thread: &'t Thread,
}

impl<'t> OptionsRead<'t> {
    /// At `static_info.options_read`.
    pub fn at_static_info(ctx: &'t Context<'_, site::StaticInfoOptionsRead>) -> Option<OptionsRead<'t>> {
        use raw::site::static_info_options_read as at;
        let (options, frame) = (at::options(ctx)?, at::frame(ctx)?);
        // SAFETY: the site's frame, whose local reader the options were read
        // from, alive while it runs on the game's thread.
        let reader = unsafe { frame.reader() };
        Some(OptionsRead { options, reader, _thread: ctx.thread() })
    }

    /// At `replay.options_read`.
    pub fn at_replay(ctx: &'t Context<'_, site::ReplayOptionsRead>) -> Option<OptionsRead<'t>> {
        let frame = raw::site::replay_options_read::frame(ctx)?;
        // SAFETY: the site's frame, its argument the options just read and
        // its local reader, alive while it runs on the game's thread.
        let (options, reader) = unsafe { (frame.options()?, frame.reader()) };

        Some(OptionsRead { options, reader, _thread: ctx.thread() })
    }

    /// The bytes after the options (srp's block, if any): from the reader's
    /// cursor to its end; none when the cursor is outside its bytes.
    pub fn rest(&self) -> Reader<'t> {
        // SAFETY: a live reader, for the hook.
        let (begin, cursor, size) = unsafe { (self.reader.begin(), self.reader.cursor(), *self.reader.size()) };
        let (first, at) = (begin as usize, cursor as usize);
        let end = first.wrapping_add(size as usize);
        if cursor.is_null() || at < first || at > end || end < first {
            return Reader::new(&[]);
        }
        // SAFETY: begin..begin+size are the message's bytes, alive for the hook,
        // and cursor..end lies inside them.
        unsafe { Reader::of_memory(cursor, end - at) }
    }

    /// Patch 65's mark in the options: "SRBO" for a Blackout match, else
    /// zeros.
    pub fn set_blackout_mark(&mut self, blackout: bool) {
        let mark = u32::from_le_bytes(*b"SRBO");
        // SAFETY: the options are alive for the hook; the mark is in their
        // unused "Mode" string.
        unsafe { *self.options.srp_blackout() = if blackout { mark } else { 0 } }
    }
}
