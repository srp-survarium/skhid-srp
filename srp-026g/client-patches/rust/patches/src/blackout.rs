//! Patch 65: Blackout, a battery match with no interface (a prototype; the
//! rules: docs/notes/blackout.md; client_patch.md section 3.58). The hooks
//! are `blackout.toml`'s, each a `lohook` at a site of its own:
//!
//! - the match's options read (`static_info.options_read`, after the 0xE4's
//!   match_options::deserialize; `replay.options_read`, after a replay's
//!   chunk 1's): srp's block after the squads ("SRPX", tagged entries) may
//!   hold tag 2, one byte 1. The options keep a mark of it (+57870h) and the
//!   patch the match's flag, before the match's world is made;
//! - each frame's player icons update (`player_icons.update_start`): the hud
//!   movie and the player icons movie are told the forced interface
//!   (`root.force_interface_hidden`: the hud keeps the pre-match countdown and
//!   its Tab view, the Tab list by names with its map among it, HUD.as; the
//!   icons hide the match's objects' markers and keep the players', the
//!   teammates' as in a plain match), once for each change of a movie or of
//!   the state;
//! - the game world's keys (`keyboard.tab_state`): Tab's state, and in a
//!   Blackout match the key goes on as with Tab up (the game's list keeps
//!   Tab's byte; the kits' keys 1 to 3 of patch 58 come after the game's Tab
//!   test);
//! - the player name decision (`player_names.visibility`): Tab shows names
//!   for teammates in Blackout, after the native visibility checks.
//!
//! The player's input is the game's, Tab or not, as in every mode (the owner,
//! 2026-11-11: shooting on Tab "like in other modes, since map now closes the
//! shooting view"). Nothing a match computes: the interface only.

#![forbid(unsafe_code)]

use crate::vostok::breadcrumbs::{self, Crumb};
use crate::vostok::parse::Reader;
use crate::vostok::{Context, Flow, GameCell, GameWorldUi, Movie, NameVisibility, NoExit, OptionsRead, TabTest, Thread};

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";
/// The Blackout entry: one byte, 1.
const TAG_BLACKOUT: u8 = 2;

/// What a movie was last told: which movie (0 none) and the forced state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Told {
    movie: usize,
    forced: bool,
}

const NONE_TOLD: Told = Told { movie: 0, forced: false };

/// The options last read (a match's or a replay's) are a Blackout match's.
static BLACKOUT: GameCell<bool> = GameCell::new(false);
/// Tab, as the game world's keys last reported it; a new match starts up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabState {
    Up,
    Down,
}

impl TabState {
    fn of(down: bool) -> Self { if down { Self::Down } else { Self::Up } }
}

static TAB: GameCell<TabState> = GameCell::new(TabState::Up);
static HUD_TOLD: GameCell<Told> = GameCell::new(NONE_TOLD);
static ICONS_TOLD: GameCell<Told> = GameCell::new(NONE_TOLD);

/// Whether srp's block (the bytes after the options' squads) says Blackout:
/// its magic, then entries; an entry of another tag or length is stepped
/// over, a cut one ends the block.
pub fn blackout_in(mut rest: Reader) -> bool {
    if rest.array::<4>() != Some(SRPX) {
        return false;
    }
    let mut on = false;
    while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
        let Some(entry) = rest.take(usize::from(len)) else { break };
        if tag == TAG_BLACKOUT && entry == [1] {
            on = true;
        }
    }
    on
}

fn options_read(thread: &Thread, read: Option<OptionsRead>, replay: bool) -> Flow<NoExit> {
    let mut on = false;
    if let Some(mut read) = read {
        on = blackout_in(read.rest());
        read.set_blackout_mark(on);
        breadcrumbs::record(thread, Crumb::Match { replay, context: read.context(on) });
    }
    BLACKOUT.set(thread, on);
    TAB.set(thread, TabState::Up);
    // A new match can allocate its movies at the same addresses as the
    // previous match. Their new roots must receive the forced state again.
    HUD_TOLD.set(thread, NONE_TOLD);
    ICONS_TOLD.set(thread, NONE_TOLD);
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_static_info(ctx), false)
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_replay(ctx), true)
}

/// `movie` told `root.force_interface_hidden(forced)` when it is another
/// movie or another state than `cell` last told, and again while the movie
/// does not take it.
fn tell(thread: &Thread, cell: &GameCell<Told>, movie: Option<Movie>, forced: bool) {
    let Some(movie) = movie else { return };
    let now = Told { movie: movie.id(), forced };
    if cell.get(thread) != now && movie.invoke_bool(c"root.force_interface_hidden", forced) {
        cell.set(thread, now);
    }
}

/// update_player_icons' start (0x23AA10), every frame: [esp+4] the ui.
pub fn on_update_icons(ctx: &mut Context) -> Flow<NoExit> {
    crate::artefact_carry::on_hud(ctx);
    let Some(ui) = GameWorldUi::at_update_icons(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let forced = BLACKOUT.get(thread);
    for (cell, movie) in [(&HUD_TOLD, ui.hud_movie()), (&ICONS_TOLD, ui.icons_movie())] {
        tell(thread, cell, movie, forced);
    }
    Flow::Continue
}

/// on_keyboard_action after its ask of Tab (0x25CD2C). In a Blackout match
/// the key goes on as with Tab up.
pub fn on_tab(ctx: &mut Context) -> Flow<NoExit> {
    let blackout = BLACKOUT.get(ctx.thread());
    let down = {
        let mut key = TabTest::at(ctx);
        let down = key.tab_down();
        if blackout {
            key.key_goes_on();
        }
        down
    };
    TAB.set(ctx.thread(), TabState::of(down));
    Flow::Continue
}

/// render_name's Alt/crosshair result, after the game's visibility checks.
/// Tab reveals teammate names only; the game's other name decisions survive.
pub fn on_name_visibility(ctx: &mut Context) -> Flow<NoExit> {
    if BLACKOUT.get(ctx.thread()) && TAB.get(ctx.thread()) == TabState::Down
        && let Some(name) = NameVisibility::for_teammate(ctx)
    {
        name.show();
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(bytes: &[u8]) -> bool {
        blackout_in(Reader::new(bytes))
    }

    #[test]
    fn the_block() {
        assert!(block(b"SRPX\x02\x01\x01"));
        assert!(block(b"SRPX\x01\x0c\0\0\x40\x40\x8f\xc2\x2b\x3f\0\0\0\x40\x02\x01\x01"));
        assert!(block(b"SRPX\x09\x03xyz\x02\x01\x01"));
        assert!(!block(b""));
        assert!(!block(b"SRPX\x02\x01\x00"));
        assert!(!block(b"SRPX\x02\x02\x01\x01"));
        assert!(!block(b"SRPX\x02\x01"));
        assert!(!block(b"SRPY\x02\x01\x01"));
    }

}
