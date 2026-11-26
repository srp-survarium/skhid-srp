//! Patch 214: a mode's notice (Blackout's, Freeze Tag's: the hud's
//! `ModeNotice`, under the match timer) only in each account's first three
//! matches of the mode (docs/notes/mode_introductions.md).
//!
//! The lobby decides it for each place of a match and says so in srp's block
//! after the options: the notices entry (tag 7, a byte a place, 1 shown, 0
//! not; `static_info::SRPX_NOTICES`), in every match of a mode with a notice.
//! The hooks are `mode_notice.toml`'s:
//!
//! - the match's options read (`static_info.options_read`, a replay's
//!   `replay.options_read`): the entry kept, or that there is none;
//! - initialize_match as it sends the icons movie its enemy list
//!   (`player_icons.enemy_ids`, 0x235709): after the hud's `root.reset`
//!   (0x234AE8, which forgets the last match's word) and before any frame of
//!   the match, the hud movie is told `root.mode_notice(shown)`. Its notice
//!   waits for it; a late join's rebuild of the lists, which comes here
//!   again, tells it the same.
//!
//! Shown: in a match, when the entry marks the local place (the options'
//! is_local profile), or when the options have no entry (an older server:
//! every match shows it, as before). A replay never shows it, whatever its
//! options say. Nothing a match computes: the interface only.

#![forbid(unsafe_code)]

use crate::vostok::parse::Reader;
use crate::vostok::world::{GameWorld, Playback};
use crate::vostok::{Context, Flow, GameCell, GameWorldUi, NoExit, OptionsRead};

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";
/// The notices entry's tag (static_info.rs `SRPX_NOTICES`).
const TAG_NOTICES: u8 = 7;

/// What the options last read say of the places' notices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notices {
    /// No entry: an older server's options (or none read).
    Unlisted,
    /// The entry: bit `p` set when place `p`'s notice is shown.
    Places(u32),
}

static NOTICES: GameCell<Notices> = GameCell::new(Notices::Unlisted);

/// The notices entry of srp's block (the bytes after the options' squads):
/// its magic, then entries; an entry of another tag is stepped over, a cut
/// one ends the block. A byte other than 0 is a shown place; a place past
/// 32 cannot be (20 at most).
fn notices_in(mut rest: Reader) -> Notices {
    if rest.array::<4>() != Some(SRPX) {
        return Notices::Unlisted;
    }
    let mut notices = Notices::Unlisted;
    while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
        let Some(entry) = rest.take(usize::from(len)) else { break };
        if tag == TAG_NOTICES {
            let mask = entry
                .iter()
                .take(32)
                .enumerate()
                .filter(|&(_, &byte)| byte != 0)
                .fold(0u32, |mask, (place, _)| mask | 1 << place);
            notices = Notices::Places(mask);
        }
    }
    notices
}

/// Whether the hud shows the mode's notice: never in a replay (or a game
/// whose world is not known); in a match, the local place's word, or with no
/// word at all (an older server) shown.
fn shows(playback: Option<Playback>, notices: Notices, local: Option<u8>) -> bool {
    match (playback, notices) {
        (Some(Playback::Match), Notices::Unlisted) => true,
        (Some(Playback::Match), Notices::Places(mask)) => {
            local.is_some_and(|place| place < 32 && mask & 1 << place != 0)
        }
        (Some(Playback::Replay) | None, _) => false,
    }
}

fn options_read(ctx: &Context<'_>, read: Option<OptionsRead<'_>>) {
    NOTICES.set(ctx.thread(), read.map_or(Notices::Unlisted, |read| notices_in(read.rest())));
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options_read(ctx, OptionsRead::at_static_info(ctx));
    Flow::Continue
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options_read(ctx, OptionsRead::at_replay(ctx));
    Flow::Continue
}

/// initialize_match as it sends the icons movie its enemy list (0x235709):
/// the hud, reset just before, told this match's word.
pub fn on_enemy_ids(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let world = GameWorld::at_enemy_ids(ctx);
    let playback = world.as_ref().map(GameWorld::playback);
    let local = world.and_then(|world| world.network_client()?.local_profile());
    let shown = shows(playback, NOTICES.get(ctx.thread()), local);
    if let Some(movie) = GameWorldUi::at_enemy_ids(ctx).and_then(|ui| ui.hud_movie()) {
        movie.invoke_bool(c"root.mode_notice", shown);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(bytes: &[u8]) -> Notices {
        notices_in(Reader::new(bytes))
    }

    #[test]
    fn the_entry_is_read_among_the_others() {
        assert_eq!(read(b""), Notices::Unlisted);
        assert_eq!(read(b"SRPX\x02\x01\x01"), Notices::Unlisted);
        assert_eq!(read(b"SRPX\x02\x01\x01\x07\x03\x00\x01\x01"), Notices::Places(0b110));
        assert_eq!(read(b"SRPX\x06\x01\x01\x05\x02\x00\x02\x07\x02\x00\x00"), Notices::Places(0));
        assert_eq!(read(b"SRPY\x07\x01\x01"), Notices::Unlisted);
        // A cut entry ends the block.
        assert_eq!(read(b"SRPX\x07\x03\x01"), Notices::Unlisted);
    }

    #[test]
    fn a_match_shows_the_local_places_word_and_a_replay_never() {
        let match_ = Some(Playback::Match);
        assert!(shows(match_, Notices::Unlisted, Some(3)), "an older server");
        assert!(shows(match_, Notices::Places(0b1000), Some(3)));
        assert!(!shows(match_, Notices::Places(0b0111), Some(3)));
        assert!(!shows(match_, Notices::Places(u32::MAX), None), "no local place");
        for notices in [Notices::Unlisted, Notices::Places(u32::MAX)] {
            assert!(!shows(Some(Playback::Replay), notices, Some(0)));
            assert!(!shows(None, notices, Some(0)));
        }
    }
}
