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
use super::world::read;
use crate::raw;

/// A maps_dict id: the map and mode a match names. The options' byte holds
/// one up to 255, or the low byte of a wider one whose whole id srp's block
/// gives (patch 154 keeps it in the options' +577B8h, `map_id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapId(u16);

impl MapId {
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub const fn low_byte(self) -> u8 {
        self.0.to_le_bytes()[0]
    }

    /// Whether the options' byte cannot hold it.
    pub const fn over_a_byte(self) -> bool {
        self.0 > u8::MAX as u16
    }
}

impl From<u8> for MapId {
    fn from(id: u8) -> Self {
        Self(u16::from(id))
    }
}

/// The map id every reader of `options` takes (+577B8h): what patch 154
/// wrote at their read, a u16 in a u32.
///
/// # Safety
/// `options` are alive on the hook's thread.
pub(in crate::vostok) unsafe fn map_id_of(options: raw::MatchOptions) -> MapId {
    // SAFETY: the caller's; a plain value of the options.
    MapId::new(unsafe { read(options.map_id()) } as u16)
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

    /// The match's places' count: the roster the rules and the hud state are
    /// filled for.
    pub fn places(&self) -> u8 {
        // SAFETY: the options are alive for the hook; a plain value of theirs.
        unsafe { read(self.options.places()) }
    }

    /// The options' map byte (+577B4h): the map id, or its low byte.
    pub fn map_byte(&self) -> u8 {
        // SAFETY: the options are alive for the hook; a plain value of theirs.
        unsafe { read(self.options.map()) }
    }

    /// Patch 154: the map id every reader of the options takes from now on
    /// (+577B8h, the game's "match maker id", logged only).
    pub fn set_map_id(&mut self, id: MapId) {
        // SAFETY: the options are alive for the hook; the field is a plain u32
        // nothing else writes.
        unsafe { *self.options.map_id() = u32::from(id.get()) }
    }

    /// What names the match, for the crash report's breadcrumbs.
    pub fn context(&self, blackout: bool) -> Match {
        // SAFETY: the options are alive for the hook; plain values of theirs.
        unsafe {
            Match {
                id: read(self.options.seed()),
                map: map_id_of(self.options),
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
