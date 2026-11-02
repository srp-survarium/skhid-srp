//! Patch 95: a match's countdown before play comes from its static info when
//! the server sends one (srp's block after the options, tag 3: one byte, the
//! seconds), in place of the mode's `countdown_time` (20 s in every mode):
//! the server sends 5 for a match whose only place is one player's (the
//! owner's, 2026-11-02: "starting private 1vs0 shouldn't have awaiting match
//! start timer (or it should have just 5 seconds of it top)";
//! docs/notes/rooms.md). Hooks of `countdown.toml`:
//!
//! - the match's options read (`static_info.options_read`, after the 0xE4's
//!   match_options::deserialize; `replay.options_read`, a replay's): the
//!   options keep the countdown (+57874h "SRCD", +57878h the milliseconds),
//!   or none;
//! - the time rule made from them (`timelimit.made`, its constructor with
//!   the mode's times stored): their countdown replaces the mode's.
//!
//! A match computes it: every game of the match, its late joiners' too (the
//! server sends each the same entry), counts down the same; a replay of an
//! older match, whose options have no entry, counts down the mode's as it was
//! played.

#![forbid(unsafe_code)]

use crate::vostok::parse::Reader;
use crate::vostok::{Context, Countdown, Flow, MadeRule, NoExit, OptionsRead, srp_entries};

/// The countdown entry of srp's block: one byte, the seconds.
const TAG_COUNTDOWN: u8 = 3;

/// The countdown srp's block (the bytes after the options' squads) gives:
/// the first entry of its tag and length; another tag or length is stepped
/// over, a cut one ends the block.
pub fn countdown_in(rest: Reader) -> Option<Countdown> {
    srp_entries(rest).find_map(|entry| match entry {
        (TAG_COUNTDOWN, &[seconds]) => Some(Countdown::from_seconds(seconds)),
        _ => None,
    })
}

fn options_read(read: Option<OptionsRead>) -> Flow<NoExit> {
    if let Some(mut read) = read {
        let countdown = countdown_in(read.rest());
        read.set_countdown(countdown);
    }
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    options_read(OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    options_read(OptionsRead::at_replay(ctx))
}

/// timelimit_rule_core's constructor with the mode's times (0x317EA7).
pub fn on_rule_made(ctx: &mut Context) -> Flow<NoExit> {
    if let Some(mut rule) = MadeRule::at_made(ctx)
        && let Some(countdown) = rule.options_countdown()
    {
        rule.set_countdown(countdown);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(bytes: &[u8]) -> Option<u32> {
        countdown_in(Reader::new(bytes)).map(Countdown::ms)
    }

    #[test]
    fn the_block() {
        assert_eq!(block(b"SRPX\x03\x01\x05"), Some(5_000));
        assert_eq!(block(b"SRPX\x02\x01\x01\x03\x01\x05"), Some(5_000));
        assert_eq!(block(b"SRPX\x09\x03xyz\x03\x01\x00"), Some(0));
        assert_eq!(block(b"SRPX\x03\x01\xff"), Some(255_000));
        assert_eq!(block(b""), None);
        assert_eq!(block(b"SRPX\x02\x01\x01"), None);
        assert_eq!(block(b"SRPX\x03\x02\x05\x00"), None);
        assert_eq!(block(b"SRPX\x03\x01"), None);
        assert_eq!(block(b"SRPY\x03\x01\x05"), None);
    }
}
