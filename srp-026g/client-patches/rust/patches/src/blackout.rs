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
//!   its Tab view, HUD.as; the icons hide the match's objects' markers) and
//!   the icons Tab's state (`root.force_interface_tab`: the players' markers
//!   while Tab is held), once for each change of a movie or of the state;
//! - the game world's keys (`keyboard.tab_state`): Tab's state, and in a
//!   Blackout match the key goes on as with Tab up (no Tab list: the kits'
//!   keys 1 to 3 of patch 58 come after the game's Tab test);
//! - the input builder's end (`input_builder.end`): while Tab is held the
//!   input has no move and no act (the look stays), and an act held through
//!   Tab stays out until its key is let go.
//!
//! Nothing a match computes: the interface and the local input only (the
//! input is filtered before it is sent, so every game plays what was sent).

#![forbid(unsafe_code)]

use crate::vostok::parse::Reader;
use crate::vostok::{
    ActionMask, Context, Flow, GameCell, GameWorldUi, InputHandler, Movie, NoExit, OptionsRead, TabTest, Thread,
};

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";
/// The Blackout entry: one byte, 1.
const TAG_BLACKOUT: u8 = 2;

/// Tab held stops these: the moves and the acts.
pub const MOVES: ActionMask = ActionMask::FORWARD
    .union(ActionMask::BACK)
    .union(ActionMask::LEFT)
    .union(ActionMask::RIGHT)
    .union(ActionMask::SPRINT);
pub const ACTS: ActionMask = ActionMask::JUMP
    .union(ActionMask::FIRE)
    .union(ActionMask::RELOAD)
    .union(ActionMask::AIM)
    .union(ActionMask::QUICK_USE)
    .union(ActionMask::HOLD_BREATH)
    .union(ActionMask::USE)
    .union(ActionMask::DROP)
    .union(ActionMask::GRENADE)
    .union(ActionMask::MELEE);

/// What a movie was last told: which movie (0 none) and the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Told {
    movie: usize,
    state: u8,
}

const NONE_TOLD: Told = Told { movie: 0, state: 0 };

/// The options last read (a match's or a replay's) are a Blackout match's.
static BLACKOUT: GameCell<bool> = GameCell::new(false);
/// Tab held (the game's last test of it).
static TAB_DOWN: GameCell<bool> = GameCell::new(false);
/// The acts held through Tab (each stays out until its key is let go).
static HELD: GameCell<ActionMask> = GameCell::new(ActionMask::NONE);
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

/// The input's action mask as built, and the acts held through Tab: what
/// is sent and what stays held. Not a Blackout match: as built, nothing held.
pub fn filtered(blackout: bool, tab_down: bool, held: ActionMask, built: ActionMask) -> (ActionMask, ActionMask) {
    if !blackout {
        return (built, ActionMask::NONE);
    }
    if tab_down {
        return (built.without(MOVES.union(ACTS)), built.intersection(ACTS));
    }
    let still = held.intersection(built);
    (built.without(still), still)
}

fn options_read(thread: &Thread, read: Option<OptionsRead>) -> Flow<NoExit> {
    let mut on = false;
    if let Some(mut read) = read {
        on = blackout_in(read.rest());
        read.set_blackout_mark(on);
    }
    BLACKOUT.set(thread, on);
    TAB_DOWN.set(thread, false);
    HELD.set(thread, ActionMask::NONE);
    // A new match can allocate its movies at the same addresses as the
    // previous match. Their new roots must receive the forced state again.
    HUD_TOLD.set(thread, NONE_TOLD);
    ICONS_TOLD.set(thread, NONE_TOLD);
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_replay(ctx))
}

/// `movie` told `state` (by `tell`, which says whether the movie took it)
/// when it is another movie or another state than `cell` last told.
fn tell(thread: &Thread, cell: &GameCell<Told>, movie: Option<Movie>, state: u8, tell: impl Fn(&Movie) -> bool) {
    let Some(movie) = movie else { return };
    let now = Told { movie: movie.id(), state };
    if cell.get(thread) != now && tell(&movie) {
        cell.set(thread, now);
    }
}

/// update_player_icons' start (0x23AA10), every frame: [esp+4] the ui.
pub fn on_update_icons(ctx: &mut Context) -> Flow<NoExit> {
    let Some(ui) = GameWorldUi::at_update_icons(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let forced = BLACKOUT.get(thread);
    let tab = TAB_DOWN.get(thread);
    tell(thread, &HUD_TOLD, ui.hud_movie(), u8::from(forced), |movie| {
        movie.invoke_bool(c"root.force_interface_hidden", forced)
    });
    tell(thread, &ICONS_TOLD, ui.icons_movie(), u8::from(forced) | u8::from(tab) << 1, |movie| {
        movie.invoke_bool(c"root.force_interface_hidden", forced) && movie.invoke_bool(c"root.force_interface_tab", tab)
    });
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
    TAB_DOWN.set(ctx.thread(), down);
    Flow::Continue
}

/// process_first_person_mode's end (0x2421B0): ebx the input handler.
pub fn on_input_end(ctx: &mut Context) -> Flow<NoExit> {
    let thread = ctx.thread();
    let (blackout, tab, held) = (BLACKOUT.get(thread), TAB_DOWN.get(thread), HELD.get(thread));
    let Some(mut handler) = InputHandler::at_builder_end(ctx) else { return Flow::Continue };
    let (sent, still) = filtered(blackout, tab, held, handler.actions());
    handler.set_actions(sent);
    HELD.set(ctx.thread(), still);
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

    #[test]
    fn tab_stops_moves_and_acts() {
        let moving = ActionMask::FORWARD.union(ActionMask::SPRINT).union(ActionMask::FIRE).union(ActionMask::AIM)
            .union(ActionMask::CROUCH);
        assert_eq!(filtered(false, true, ActionMask::NONE, moving), (moving, ActionMask::NONE));
        let (sent, held) = filtered(true, true, ActionMask::NONE, moving);
        assert_eq!(sent, ActionMask::CROUCH);
        assert_eq!(held, ActionMask::FIRE.union(ActionMask::AIM));
        // Tab up, fire still held: it stays out; a move comes back.
        let (sent, held) = filtered(true, false, held, ActionMask::FORWARD.union(ActionMask::FIRE));
        assert_eq!(sent, ActionMask::FORWARD);
        assert_eq!(held, ActionMask::FIRE);
        // fire let go, then pressed again: it fires.
        let (_, held) = filtered(true, false, held, ActionMask::FORWARD);
        assert_eq!(filtered(true, false, held, ActionMask::FIRE).0, ActionMask::FIRE);
    }
}
