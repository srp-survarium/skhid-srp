//! A match's options as the static info gives them: survarium::match_options
//! (0x578A0 bytes, read by match_options::deserialize 0x1F0E70) and the
//! reader of the message's bytes it read from, after the read, at the sites
//! `static_info.options_read` (process_static_match_info, 0x27B92D: ESI the
//! options, the reader its frame's local at EBP-2Ch) and
//! `replay.options_read` (a replay's chunk 1, 0x1F0971: the options its
//! first argument, the reader its local; both by the site's ESP).

use super::context::Context;
use super::hook::Thread;
use super::breadcrumbs::Match;
use super::parse::Reader;
use super::ui::c_string;
use super::world::read;
use crate::raw;
use core::ffi::CStr;

/// A place's name: 64 bytes with its zero.
const NAME_LIMIT: usize = 64;

/// The bytes `reader` has not read: from its cursor to its end; none when the
/// cursor is outside its bytes.
///
/// # Safety
/// `reader` is a live reader of a message, its bytes alive for `'a`.
pub(super) unsafe fn unread<'a>(reader: raw::BufferReader) -> Reader<'a> {
    // SAFETY: the caller's.
    let (begin, cursor, size) = unsafe { (reader.begin(), reader.cursor(), *reader.size()) };
    let (first, at) = (begin as usize, cursor as usize);
    let end = first.wrapping_add(size as usize);
    if cursor.is_null() || at < first || at > end || end < first {
        return Reader::new(&[]);
    }
    // SAFETY: begin..begin+size are the message's bytes, alive for 'a, and
    // cursor..end lies inside them.
    unsafe { Reader::of_memory(cursor, end - at) }
}

/// The options just read and the reader they were read from (its cursor
/// where the options end).
pub struct OptionsRead<'t> {
    options: raw::MatchOptions,
    reader: raw::BufferReader,
    _thread: &'t Thread,
}

impl<'t> OptionsRead<'t> {
    /// At `static_info.options_read`.
    pub fn at_static_info(ctx: &'t Context<'_>) -> Option<OptionsRead<'t>> {
        use raw::site::static_info_options_read as site;
        // SAFETY: the context is this site's: the options just read and the
        // function's frame, whose local reader they were read from, alive while
        // it runs on the game's thread.
        let (options, reader) = unsafe { (site::options(ctx)?, site::frame(ctx)?.reader()) };
        Some(OptionsRead { options, reader, _thread: ctx.thread() })
    }

    /// At `replay.options_read`.
    pub fn at_replay(ctx: &'t Context<'_>) -> Option<OptionsRead<'t>> {
        // SAFETY: the context is this site's: the frame, its argument the
        // options just read and its local reader, alive while it runs on the
        // game's thread.
        let (options, reader) = unsafe {
            let frame = raw::site::replay_options_read::frame(ctx)?;
            (frame.options()?, frame.reader())
        };
        Some(OptionsRead { options, reader, _thread: ctx.thread() })
    }

    /// The bytes after the options (srp's block, if any): from the reader's
    /// cursor to its end; none when the cursor is outside its bytes.
    pub fn rest(&self) -> Reader<'t> {
        // SAFETY: a live reader, for the hook.
        unsafe { unread(self.reader) }
    }

    /// The name of each of the match's places, place 0 first (none for a
    /// name that is no C string in its 64 bytes).
    pub fn place_names(&self) -> impl Iterator<Item = Option<&'t CStr>> + '_ {
        // SAFETY: the options are alive for the hook; a place below the count
        // is one of the array, its name a C string in its 64 bytes.
        let places = unsafe { read(self.options.places()) };
        (0..usize::from(places)).map(|place| unsafe { c_string(self.options.first_place().at(place).name(), NAME_LIMIT) })
    }

    /// What names the match, for the crash report's breadcrumbs.
    pub fn context(&self, blackout: bool) -> Match {
        // SAFETY: the options are alive for the hook; plain values of theirs.
        unsafe {
            Match {
                id: read(self.options.seed()),
                map: read(self.options.map()),
                places: read(self.options.places()),
                level: read(self.options.level()),
                blackout,
            }
        }
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
