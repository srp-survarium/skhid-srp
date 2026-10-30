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
//! - the player name decision (`player_names.visibility`): Tab shows names
//!   for teammates in Blackout, after the native visibility checks;
//! - the input builder's end (`input_builder.end`): while Tab is held the
//!   input has no act; moves stop after 200 ms (the look stays). An act held through
//!   Tab stays out until its key is let go;
//! - a character's visual (`skin_visual.parts_done`, on_configs_loaded after
//!   its parts and the head): the teams' duct tape. The band is the visual's
//!   own profile's (srp writes it in the protection field: 11 blue, 12 red; 10,
//!   none, everywhere else), so a match's start, a late joiner, a respawn, a
//!   replay and the lobby are all the same read; none on the game's own
//!   player. Each taped record (`tape`) gets a shell record in the band's
//!   colour, and the visual's name a team mark.
//!
//! Nothing a match computes: the interface, the local input and the
//! characters' look only (the input is filtered before it is sent, so every
//! game plays what was sent).

#![forbid(unsafe_code)]

pub mod tape;

use crate::vostok::skin::{NewPart, SkinVisual, Wearer};
use crate::vostok::{clock, parse::Reader};
use crate::vostok::{
    ActionMask, Context, Flow, GameCell, GameWorldUi, InputHandler, Movie, NameVisibility, NoExit, OptionsRead,
    TabTest, Thread,
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
/// One owner for the key and its movement grace period. Repeated key reports
/// cannot restart the grace; release or a new match ends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabState {
    Up,
    Grace { pressed_at: u64 },
    Blocking,
}

impl TabState {
    fn down(self) -> bool { self != Self::Up }

    fn key(self, down: bool, now: u64) -> Self {
        match (self, down) {
            (_, false) => Self::Up,
            (Self::Up, true) => Self::Grace { pressed_at: now },
            (state, true) => state,
        }
    }

    fn advance(self, now: u64, ticks_per_second: u64) -> Self {
        match self {
            Self::Grace { pressed_at } if now.wrapping_sub(pressed_at) as f64 * 5.0 >= ticks_per_second as f64 => Self::Blocking,
            state => state,
        }
    }
}

static TAB: GameCell<TabState> = GameCell::new(TabState::Up);
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
fn filtered(blackout: bool, tab: TabState, held: ActionMask, built: ActionMask) -> (ActionMask, ActionMask) {
    if !blackout {
        return (built, ActionMask::NONE);
    }
    if tab.down() {
        let blocked = if tab == TabState::Blocking { ACTS.union(MOVES) } else { ACTS };
        return (built.without(blocked), built.intersection(ACTS));
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
    TAB.set(thread, TabState::Up);
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
    crate::artefact_carry::on_hud(ctx);
    let Some(ui) = GameWorldUi::at_update_icons(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let forced = BLACKOUT.get(thread);
    let tab = TAB.get(thread).down();
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
    let thread = ctx.thread();
    let tab = TAB.get(thread);
    // Only a fresh Blackout press needs a timestamp. The regular modes do
    // not consult a clock or change their native key behavior.
    let now = if blackout && down && tab == TabState::Up { clock::counter(thread) } else { 0 };
    TAB.set(thread, tab.key(down, now));
    Flow::Continue
}

/// render_name's Alt/crosshair result, after the game's visibility checks.
/// Tab reveals teammate names only; the game's other name decisions survive.
pub fn on_name_visibility(ctx: &mut Context) -> Flow<NoExit> {
    if BLACKOUT.get(ctx.thread()) && TAB.get(ctx.thread()).down()
        && let Some(name) = NameVisibility::for_teammate(ctx)
    {
        name.show();
    }
    Flow::Continue
}

/// process_first_person_mode's end (0x2421B0): ebx the input handler.
pub fn on_input_end(ctx: &mut Context) -> Flow<NoExit> {
    let thread = ctx.thread();
    let (blackout, mut tab, held) = (BLACKOUT.get(thread), TAB.get(thread), HELD.get(thread));
    if blackout && matches!(tab, TabState::Grace { .. }) {
        tab = tab.advance(clock::counter(thread), clock::frequency(thread));
        TAB.set(thread, tab);
    }
    let Some(mut handler) = InputHandler::at_builder_end(ctx) else { return Flow::Continue };
    let (sent, still) = filtered(blackout, tab, held, handler.actions());
    handler.set_actions(sent);
    HELD.set(ctx.thread(), still);
    crate::artefact_carry::on_input_end(ctx)
}

/// on_configs_loaded after its parts and the head (0x2056F0): each taped
/// record of the visual (of those in use here) gets the tape's shell, in the
/// band's colour, while the cook data has room; the visual's name then gets
/// the team's mark. Nothing for the game's own player or a profile without
/// a band.
pub fn on_skin_parts(ctx: &mut Context) -> Flow<NoExit> {
    let Some(mut visual) = SkinVisual::at_parts_done(ctx) else { return Flow::Continue };
    let Wearer::Player { band: Some(band) } = visual.wearer() else { return Flow::Continue };
    let mut added = false;
    for index in 0..visual.count() {
        let Some((kind, shell)) = visual.part(index).as_ref().and_then(tape::shell_of) else { continue };
        let part = NewPart {
            kind,
            base_model: shell.as_cstr(),
            part_name: tape::SHELL_SURFACE,
            material: tape::material(band),
        };
        if visual.add_part(&part).is_err() {
            break;
        }
        added = true;
    }
    if added {
        visual.mark_name(tape::name_mark(band));
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

    #[test]
    fn tab_stops_moves_and_acts() {
        let moving = ActionMask::FORWARD.union(ActionMask::SPRINT).union(ActionMask::FIRE).union(ActionMask::AIM)
            .union(ActionMask::CROUCH);
        assert_eq!(filtered(false, TabState::Blocking, ActionMask::NONE, moving), (moving, ActionMask::NONE));
        let (sent, held) = filtered(true, TabState::Blocking, ActionMask::NONE, moving);
        assert_eq!(sent, ActionMask::CROUCH);
        assert_eq!(held, ActionMask::FIRE.union(ActionMask::AIM));
        // Tab up, fire still held: it stays out; a move comes back.
        let (sent, held) = filtered(true, TabState::Up, held, ActionMask::FORWARD.union(ActionMask::FIRE));
        assert_eq!(sent, ActionMask::FORWARD);
        assert_eq!(held, ActionMask::FIRE);
        // fire let go, then pressed again: it fires.
        let (_, held) = filtered(true, TabState::Up, held, ActionMask::FORWARD);
        assert_eq!(filtered(true, TabState::Up, held, ActionMask::FIRE).0, ActionMask::FIRE);
    }
    #[test]
    fn a_quick_tab_keeps_movement_and_repeats_do_not_extend_the_grace() {
        let moving = MOVES.union(ActionMask::FIRE);
        let tab = TabState::Up.key(true, 1_000);
        assert_eq!(tab.key(true, 1_100), tab);
        for now in [1_000, 1_001, 1_199] {
            let state = tab.advance(now, 1_000);
            assert_eq!(filtered(true, state, ActionMask::NONE, moving), (MOVES, ActionMask::FIRE));
        }
        assert_eq!(tab.advance(1_200, 1_000), TabState::Blocking);
        assert_eq!(tab.key(false, 1_100), TabState::Up);
        assert_eq!(tab.key(false, 1_100).key(true, 1_150), TabState::Grace { pressed_at: 1_150 });
        assert_eq!(TabState::Grace { pressed_at: u64::MAX - 99 }.advance(100, 1_000), TabState::Blocking);
        // Non-integral counters round up, never blocking before 200 ms.
        assert!(matches!(tab.advance(1_200, 1_001), TabState::Grace { .. }));
        assert_eq!(tab.advance(1_201, 1_001), TabState::Blocking);
    }

}
