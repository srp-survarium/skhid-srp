//! A match's options as the static info gives them: survarium::match_options
//! (0x578A0 bytes, read by match_options::deserialize 0x1F0E70) and the
//! reader of the message's bytes it read from, after the read, at the sites
//! `static_info.options_read` (process_static_match_info, 0x27B92D: ESI the
//! options, the reader its frame's local at EBP-2Ch) and
//! `replay.options_read` (a replay's chunk 1, 0x1F0971: the options its
//! first argument, the reader its local; both by the site's ESP).
//!
//! srp's block after the options ("SRPX", then `{u8 tag, u8 len, bytes}`
//! entries) is read here once, into the match's [`MatchVariant`] (tag 2).

use super::context::Context;
use super::hook::Thread;
use super::breadcrumbs::Match;
use super::parse::Reader;
use super::world::read;
use crate::raw;

/// srp's block's magic.
const SRPX: [u8; 4] = *b"SRPX";
/// The variant's entry: one byte, 1 Blackout, 2 Scarcity
/// (`survarium::v026g::maps::Variant::wire`).
const TAG_VARIANT: u8 = 2;

/// What a battery match is beside its rules (srp's block, tag 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatchVariant {
    /// No entry, or one this patch does not know.
    #[default]
    Plain,
    /// Patch 65 (docs/notes/blackout.md).
    Blackout,
    /// Patch 119 (docs/notes/scarcity.md).
    Scarcity,
}

/// The variant srp's block (the bytes after the options' squads) says: its
/// magic, then entries; an entry of another tag or length is stepped over
/// (the last variant entry counts), a cut one ends the block.
pub fn variant_in(mut rest: Reader) -> MatchVariant {
    if rest.array::<4>() != Some(SRPX) {
        return MatchVariant::Plain;
    }
    let mut variant = MatchVariant::Plain;
    while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
        let Some(entry) = rest.take(usize::from(len)) else { break };
        if tag == TAG_VARIANT {
            variant = match entry {
                [1] => MatchVariant::Blackout,
                [2] => MatchVariant::Scarcity,
                _ => MatchVariant::Plain,
            };
        }
    }
    variant
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

    /// The match's variant, from srp's block ([`variant_in`]).
    pub fn variant(&self) -> MatchVariant {
        variant_in(self.rest())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn block(bytes: &[u8]) -> MatchVariant {
        variant_in(Reader::new(bytes))
    }

    #[test]
    fn the_block() {
        use MatchVariant::{Blackout, Plain, Scarcity};
        assert_eq!(block(b"SRPX\x02\x01\x01"), Blackout);
        assert_eq!(block(b"SRPX\x02\x01\x02"), Scarcity);
        assert_eq!(block(b"SRPX\x01\x0c\0\0\x40\x40\x8f\xc2\x2b\x3f\0\0\0\x40\x02\x01\x01"), Blackout);
        assert_eq!(block(b"SRPX\x09\x03xyz\x02\x01\x02"), Scarcity);
        assert_eq!(block(b""), Plain);
        assert_eq!(block(b"SRPX\x02\x01\x00"), Plain);
        assert_eq!(block(b"SRPX\x02\x01\x03"), Plain);
        assert_eq!(block(b"SRPX\x02\x02\x01\x01"), Plain);
        assert_eq!(block(b"SRPX\x02\x01"), Plain);
        assert_eq!(block(b"SRPY\x02\x01\x01"), Plain);
    }
}
