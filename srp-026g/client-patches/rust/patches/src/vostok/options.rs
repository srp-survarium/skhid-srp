//! A match's options as the static info gives them: survarium::match_options
//! (0x578A0 bytes, read by match_options::deserialize 0x1F0E70) and the
//! reader of the message's bytes it read from, after the read, at the sites
//! `static_info.options_read` (process_static_match_info, 0x27B92D: ESI the
//! options, the reader its frame's local at EBP-2Ch) and
//! `replay.options_read` (a replay's chunk 1, 0x1F0971: the options its
//! first argument, the reader its local; both by the site's ESP).

use super::context::Context;
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

    /// Patch 65's mark in the options: "SRBO" for a Blackout match, else
    /// zeros.
    pub fn set_blackout_mark(&mut self, blackout: bool) {
        let mark = u32::from_le_bytes(*b"SRBO");
        // SAFETY: the options are alive for the hook; the mark is in their
        // unused "Mode" string.
        unsafe { *self.options.srp_blackout() = if blackout { mark } else { 0 } }
    }

    /// Patch 95's countdown kept in the options, for the time rule made from
    /// them ([`super::timelimit::MadeRule`]): "SRCD" and the milliseconds,
    /// else zeros.
    pub fn set_countdown(&mut self, countdown: Option<Countdown>) {
        let (mark, ms) = countdown.map_or((0, 0), |countdown| (COUNTDOWN_MARK, countdown.ms()));
        // SAFETY: the options are alive for the hook; both words are in their
        // unused "Mode" string.
        unsafe {
            *self.options.srp_countdown_mark() = mark;
            *self.options.srp_countdown() = ms;
        }
    }
}

/// Patch 95's mark: a countdown follows it.
const COUNTDOWN_MARK: u32 = u32::from_le_bytes(*b"SRCD");

/// The countdown patch 95 kept in `options` ([`OptionsRead::set_countdown`]).
///
/// # Safety
/// `options` are alive while it reads.
pub(super) unsafe fn countdown(options: raw::MatchOptions) -> Option<Countdown> {
    // SAFETY: the caller's; both words are in the unused "Mode" string.
    let (mark, ms) = unsafe { (*options.srp_countdown_mark(), *options.srp_countdown()) };
    (mark == COUNTDOWN_MARK).then_some(Countdown(ms))
}

/// The countdown before play, in the simulation's milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Countdown(u32);

impl Countdown {
    pub fn from_seconds(seconds: u8) -> Countdown {
        Countdown(u32::from(seconds) * 1_000)
    }

    pub fn ms(self) -> u32 {
        self.0
    }
}

/// The entries of srp's block after the options (`rest`): its magic "SRPX",
/// then `{u8 tag, u8 len, len bytes}` each; another magic is no entry, a cut
/// entry ends them.
pub fn srp_entries<'a>(mut rest: Reader<'a>) -> impl Iterator<Item = (u8, &'a [u8])> {
    if rest.array::<4>() != Some(*b"SRPX") {
        rest = Reader::new(&[]);
    }
    core::iter::from_fn(move || {
        let (tag, len) = (rest.u8()?, rest.u8()?);
        Some((tag, rest.take(usize::from(len))?))
    })
}
