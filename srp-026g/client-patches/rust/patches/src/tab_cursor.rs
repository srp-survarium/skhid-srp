//! Patch 86: a cursor over the Tab list, and a friend request from its rows
//! (docs/notes/tab-cursor-friends.md). The hooks are `tab_cursor.toml`'s:
//!
//! - the game world's keys at their start (`tab_cursor.key`,
//!   on_keyboard_action 0x25CD00): Ctrl pressed while the Tab list is up shows
//!   the hud's cursor (the kits' own show_cursor: the mouse goes to the hud's
//!   movies, the look and the buttons stop) until Tab is let go, and the Ctrl
//!   key is taken from the player (no crouch) until it is let go;
//! - a match's options read (`static_info.options_read`,
//!   `replay.options_read`, after Blackout's): the cursor state starts over;
//! - the hud movie's calls (`hud.callback`, scaleform_callback 0x233D30):
//!   `srp_player_menu(place)`, the row menu asking whether "Add to friends"
//!   is offered for that place, answered `root.srp_player_menu(place, offer)`;
//!   `srp_add_friend(place)`, answered `root.srp_friend_request(place, sent)`
//!   after the lobby's own friend request (`4D` type 0, as the lobby's menus
//!   send it) or none.
//!
//! The client's checks are the lobby's (servers/lobby-server social.rs,
//! `Core::important`): never the player's own name, never a friend, within
//! the lobby's allowance ([`REQUEST_BURST`] at once, one more every
//! [`REQUEST_EVERY_S`]); and the lobby connection up, as the lobby's
//! senders check it. Nothing a match computes: the local interface
//! and a lobby message.

#![forbid(unsafe_code)]

use crate::vostok::{
    GameCell, GameWorldKey, Flow, Context, HudCall, Key, KeyKind, Lobby, NoExit, NotConnected, TabCursorOnHudCallExit,
    TabCursorOnKeyExit, Thread, clock,
};
use crate::vostok::types::Place;

/// Friend requests sent at once, and the time that earns one more: the
/// lobby's `FRIEND_REQUEST_BURST` and `FRIEND_REQUEST_EVERY` (social.rs),
/// the same rule.
pub const REQUEST_BURST: u64 = 5;
pub const REQUEST_EVERY_S: u64 = 10;

/// The Ctrl keys a Tab list's cursor took: each one's events go to no one
/// until it is let go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CtrlKeys(u8);

impl CtrlKeys {
    const NONE: CtrlKeys = CtrlKeys(0);

    fn of(key: Key) -> Option<CtrlKeys> {
        match key {
            Key::LEFT_CONTROL => Some(CtrlKeys(1)),
            Key::RIGHT_CONTROL => Some(CtrlKeys(2)),
            _ => None,
        }
    }

    fn with(self, other: CtrlKeys) -> CtrlKeys {
        CtrlKeys(self.0 | other.0)
    }

    fn without(self, other: CtrlKeys) -> CtrlKeys {
        CtrlKeys(self.0 & !other.0)
    }

    fn holds(self, other: CtrlKeys) -> bool {
        self.0 & other.0 != 0
    }
}

/// The cursor over the Tab list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cursor {
    /// None of ours: every key goes on.
    Off,
    /// Shown over the Tab list; `taken` the Ctrl keys still down that came while it was up.
    Shown { taken: CtrlKeys },
    /// Tab let go, the cursor hidden; these Ctrl keys (some) are still taken until they go up.
    Draining { taken: CtrlKeys },
}

/// A key as the cursor sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KeyEvent {
    /// The key when it is a Ctrl key.
    ctrl: Option<CtrlKeys>,
    kind: KeyKind,
    /// The Tab list is up after this key (never in Blackout: no list).
    list: bool,
}

/// What the cursor does with a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Step {
    next: Cursor,
    /// The key goes to no later handler (the player's).
    take: bool,
    /// Show (true) or hide (false) the hud's cursor.
    show: Option<bool>,
}

impl Cursor {
    fn taken(self) -> CtrlKeys {
        match self {
            Cursor::Off => CtrlKeys::NONE,
            Cursor::Shown { taken } | Cursor::Draining { taken } => taken,
        }
    }

    /// The Ctrl keys still taken once the cursor is gone.
    fn draining(taken: CtrlKeys) -> Cursor {
        if taken == CtrlKeys::NONE { Cursor::Off } else { Cursor::Draining { taken } }
    }

    /// `ctrl` no longer taken.
    fn released(self, ctrl: CtrlKeys) -> Cursor {
        let taken = self.taken().without(ctrl);
        match self {
            Cursor::Shown { .. } => Cursor::Shown { taken },
            _ => Cursor::draining(taken),
        }
    }

    /// The one transition: a key in a state.
    fn step(self, event: KeyEvent) -> Step {
        // The list went away (Tab let go): the cursor with it.
        let (state, hide) = match self {
            Cursor::Shown { taken } if !event.list => (Cursor::draining(taken), true),
            state => (state, false),
        };
        let (next, take, show) = match (event.ctrl, event.kind) {
            (Some(ctrl), KeyKind::Down) if event.list => {
                (Cursor::Shown { taken: state.taken().with(ctrl) }, true, Some(true))
            }
            (Some(ctrl), KeyKind::Up) if state.taken().holds(ctrl) => (state.released(ctrl), true, None),
            // A taken key down again with no list: its up was missed (the
            // chat took it, the window lost the keyboard): the player's again.
            (Some(ctrl), KeyKind::Down) if state.taken().holds(ctrl) => (state.released(ctrl), false, None),
            (Some(ctrl), KeyKind::Held) if state.taken().holds(ctrl) => (state, true, None),
            _ => (state, false, None),
        };
        Step { next, take, show: show.or(hide.then_some(false)) }
    }
}

static CURSOR: GameCell<Cursor> = GameCell::new(Cursor::Off);

/// The friend requests still allowed, in the clock's ticks
/// ([`REQUEST_EVERY_S`] seconds of them each), and when that was counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Allowance {
    saved: u64,
    at: u64,
}

/// One request at the counter's `now`, if the allowance (None: never
/// counted, the whole burst) has one; the allowance after. The counter only
/// grows.
fn take_request(allowance: Option<Allowance>, now: u64, frequency: u64) -> (bool, Allowance) {
    // (no 64-bit division: the arena has no __udivdi3)
    let every = REQUEST_EVERY_S.saturating_mul(frequency);
    let full = every.saturating_mul(REQUEST_BURST);
    let saved = allowance.map_or(full, |allowance| {
        allowance.saved.saturating_add(now.saturating_sub(allowance.at)).min(full)
    });
    if saved < every {
        return (false, Allowance { saved, at: now });
    }
    (true, Allowance { saved: saved - every, at: now })
}

static REQUESTS: GameCell<Option<Allowance>> = GameCell::new(None);

/// A match's options read (0xE4, after Blackout's): nothing of ours is shown
/// or held in a new match's hud.
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    CURSOR.set(ctx.thread(), Cursor::Off);
    Flow::Continue
}

/// A replay's options read (its chunk 1, after Blackout's): the same.
pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    on_static_info(ctx)
}

/// on_keyboard_action's start (0x25CD00): every key the game world gets.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<TabCursorOnKeyExit> {
    let blackout = crate::blackout::forced(ctx.thread());
    let Some(key) = GameWorldKey::at_start(ctx) else { return Flow::Continue };
    let view = key.hud().view();
    // The list after this key: Tab's own event says; any other key leaves it
    // as the key before it made it (on_keyboard_action asks for Tab at every key).
    let list = !blackout && if key.key() == Key::TAB { key.kind() != KeyKind::Up } else { view.list };
    let event = KeyEvent { ctrl: CtrlKeys::of(key.key()), kind: key.kind(), list };
    let thread = key.thread();
    let step = CURSOR.get(thread).step(event);
    CURSOR.set(thread, step.next);
    match step.show {
        Some(true) => key.hud().show_cursor(true),
        // The kits keep the cursor they showed.
        Some(false) if !view.kits => key.hud().show_cursor(false),
        _ => {}
    }
    if step.take {
        key.take();
        return Flow::Exit(TabCursorOnKeyExit::Taken);
    }
    Flow::Continue
}

/// Why no friend request is offered or sent to a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    /// The match has no such place.
    NoPlace,
    /// The player's own place.
    Own,
    /// Already a friend.
    Friend,
    /// The lobby connection is down.
    NotConnected,
    /// Past the allowance.
    TooMany,
}

/// The name a friend request to `place` would go to.
fn offered<'l>(lobby: &'l Lobby<'_>, place: Place) -> Result<&'l core::ffi::CStr, Refusal> {
    let name = lobby.place_name(place).filter(|name| !name.is_empty()).ok_or(Refusal::NoPlace)?;
    if lobby.own_name().is_some_and(|own| own.to_bytes().eq_ignore_ascii_case(name.to_bytes())) {
        return Err(Refusal::Own);
    }
    if lobby.is_friend(name.to_bytes()) {
        return Err(Refusal::Friend);
    }
    Ok(name)
}

impl From<NotConnected> for Refusal {
    fn from(_: NotConnected) -> Refusal {
        Refusal::NotConnected
    }
}

fn send_request(thread: &Thread, lobby: &Lobby<'_>, place: Place) -> Result<(), Refusal> {
    let name = offered(lobby, place)?;
    let (allowed, after) = take_request(REQUESTS.get(thread), clock::counter(thread), clock::frequency(thread));
    if !allowed {
        REQUESTS.set(thread, Some(after));
        return Err(Refusal::TooMany);
    }
    // a request that did not go keeps its share of the allowance
    lobby.send_friend_request(name)?;
    REQUESTS.set(thread, Some(after));
    Ok(())
}

/// The hud movie's calls the patch answers.
enum Request {
    Menu,
    AddFriend,
}

/// scaleform_callback's start (0x233D30): every call of the hud movie.
pub fn on_hud_call(ctx: &mut Context<'_>) -> Flow<TabCursorOnHudCallExit> {
    let Some(call) = HudCall::at(ctx) else { return Flow::Continue };
    let request = match call.method().to_bytes() {
        b"srp_player_menu" => Request::Menu,
        b"srp_add_friend" => Request::AddFriend,
        _ => return Flow::Continue,
    };
    let Some(place) = call.string_argument(0).and_then(|text| Place::parse(text.to_bytes())) else {
        return Flow::Exit(TabCursorOnHudCallExit::Done);
    };
    let lobby = call.hud().lobby();
    let index = u32::from(place.get());
    match request {
        Request::Menu => {
            let offer = lobby.as_ref().is_some_and(|lobby| offered(lobby, place).is_ok());
            call.answer(c"root.srp_player_menu", &[index, u32::from(offer)]);
        }
        Request::AddFriend => {
            let sent = lobby.as_ref().ok_or(Refusal::NotConnected)
                .and_then(|lobby| send_request(call.thread(), lobby, place)).is_ok();
            call.answer(c"root.srp_friend_request", &[index, u32::from(sent)]);
        }
    }
    Flow::Exit(TabCursorOnHudCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: CtrlKeys = CtrlKeys(1);
    const RIGHT: CtrlKeys = CtrlKeys(2);

    fn ctrl(key: CtrlKeys, kind: KeyKind, list: bool) -> KeyEvent {
        KeyEvent { ctrl: Some(key), kind, list }
    }

    fn other(kind: KeyKind, list: bool) -> KeyEvent {
        KeyEvent { ctrl: None, kind, list }
    }

    #[test]
    fn ctrl_on_the_list_shows_the_cursor_and_is_taken() {
        let step = Cursor::Off.step(ctrl(LEFT, KeyKind::Down, true));
        assert_eq!(step, Step { next: Cursor::Shown { taken: LEFT }, take: true, show: Some(true) });
        let held = step.next.step(ctrl(LEFT, KeyKind::Held, true));
        assert_eq!(held, Step { next: Cursor::Shown { taken: LEFT }, take: true, show: None });
        // let go, the cursor stays while Tab is held
        let up = held.next.step(ctrl(LEFT, KeyKind::Up, true));
        assert_eq!(up, Step { next: Cursor::Shown { taken: CtrlKeys::NONE }, take: true, show: None });
        // Tab let go: hidden
        let tab = up.next.step(other(KeyKind::Up, false));
        assert_eq!(tab, Step { next: Cursor::Off, take: false, show: Some(false) });
    }

    #[test]
    fn ctrl_without_the_list_crouches() {
        for kind in [KeyKind::Down, KeyKind::Held, KeyKind::Up] {
            assert_eq!(Cursor::Off.step(ctrl(LEFT, kind, false)),
                       Step { next: Cursor::Off, take: false, show: None });
        }
    }

    #[test]
    fn a_ctrl_held_from_before_tab_stays_the_players() {
        // held into the list: its held and up events are not ours
        for kind in [KeyKind::Held, KeyKind::Up] {
            assert_eq!(Cursor::Off.step(ctrl(RIGHT, kind, true)),
                       Step { next: Cursor::Off, take: false, show: None });
        }
    }

    #[test]
    fn a_ctrl_held_past_tab_is_taken_until_let_go() {
        let shown = Cursor::Shown { taken: LEFT };
        let tab = shown.step(other(KeyKind::Up, false));
        assert_eq!(tab, Step { next: Cursor::Draining { taken: LEFT }, take: false, show: Some(false) });
        let held = tab.next.step(ctrl(LEFT, KeyKind::Held, false));
        assert_eq!(held, Step { next: Cursor::Draining { taken: LEFT }, take: true, show: None });
        let up = held.next.step(ctrl(LEFT, KeyKind::Up, false));
        assert_eq!(up, Step { next: Cursor::Off, take: true, show: None });
        // the next Ctrl is the player's again
        assert!(!up.next.step(ctrl(LEFT, KeyKind::Down, false)).take);
    }

    #[test]
    fn the_cursor_goes_with_the_list_even_on_a_ctrl_key() {
        // Ctrl's own held event after Tab went up: hidden and still taken
        let step = Cursor::Shown { taken: LEFT }.step(ctrl(LEFT, KeyKind::Held, false));
        assert_eq!(step, Step { next: Cursor::Draining { taken: LEFT }, take: true, show: Some(false) });
    }

    #[test]
    fn both_ctrl_keys_are_kept_apart() {
        let both = Cursor::Off.step(ctrl(LEFT, KeyKind::Down, true)).next.step(ctrl(RIGHT, KeyKind::Down, true));
        assert_eq!(both.next, Cursor::Shown { taken: LEFT.with(RIGHT) });
        let drain = both.next.step(other(KeyKind::Up, false)).next;
        let left_up = drain.step(ctrl(LEFT, KeyKind::Up, false));
        assert_eq!(left_up.next, Cursor::Draining { taken: RIGHT });
        assert_eq!(left_up.next.step(ctrl(RIGHT, KeyKind::Up, false)).next, Cursor::Off);
    }

    #[test]
    fn ctrl_again_on_a_new_list_shows_it_again() {
        let step = Cursor::Draining { taken: LEFT }.step(ctrl(RIGHT, KeyKind::Down, true));
        assert_eq!(step, Step { next: Cursor::Shown { taken: LEFT.with(RIGHT) }, take: true, show: Some(true) });
    }

    #[test]
    fn a_missed_release_leaves_the_next_crouch_alone() {
        // the cursor's Ctrl still taken, its up lost (the chat took it)
        let state = Cursor::Draining { taken: LEFT };
        let down = state.step(ctrl(LEFT, KeyKind::Down, false));
        assert_eq!(down, Step { next: Cursor::Off, take: false, show: None });
        for kind in [KeyKind::Held, KeyKind::Up] {
            assert!(!down.next.step(ctrl(LEFT, kind, false)).take);
        }
        // the other Ctrl stays taken
        let both = Cursor::Draining { taken: LEFT.with(RIGHT) }.step(ctrl(LEFT, KeyKind::Down, false));
        assert_eq!(both, Step { next: Cursor::Draining { taken: RIGHT }, take: false, show: None });
    }

    #[test]
    fn other_keys_go_on() {
        for state in [Cursor::Off, Cursor::Shown { taken: LEFT }, Cursor::Draining { taken: LEFT }] {
            for kind in [KeyKind::Down, KeyKind::Held, KeyKind::Up] {
                let step = state.step(other(kind, true));
                assert!(!step.take);
                assert_eq!(step.next, state);
            }
        }
    }

    #[test]
    fn five_requests_at_once_then_one_every_ten_seconds() {
        let frequency = 10_000_000;
        let (mut allowance, mut now) = (None, 7);
        for _ in 0..5 {
            let (allowed, after) = take_request(allowance, now, frequency);
            assert!(allowed);
            allowance = Some(after);
        }
        let (allowed, after) = take_request(allowance, now, frequency);
        assert!(!allowed, "the sixth at once");
        now += 10 * frequency - 1;
        let (allowed, after) = take_request(Some(after), now, frequency);
        assert!(!allowed, "just under ten seconds after");
        let (allowed, after) = take_request(Some(after), now + 1, frequency);
        assert!(allowed, "ten seconds after");
        // a long wait earns the burst again, no more
        let mut allowance = Some(after);
        now += 3600 * frequency;
        for i in 0..6 {
            let (allowed, after) = take_request(allowance, now, frequency);
            assert_eq!(allowed, i < 5);
            allowance = Some(after);
        }
    }
}
