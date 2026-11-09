//! Patch 120: a match its game never played closes too (client_patch.md
//! section 3.74, README "A match closed before it played").
//!
//! close_current_match (0x27A790) returns at its guard for every reason while
//! the time rule's state is 0: from the connection until the first tick
//! played after the first world. A game kicked, timed out, cut off or given
//! its farewell there stayed on the loading screen or in the free camera
//! for good. Since this patch, a match open since its connect and not closed:
//!
//! - closed in its handshake (no level asked for): the lobby is told `31`,
//!   as a lost connection tells it; there is no world to end;
//! - closed while its level loads: nothing is ended under the load. The
//!   load finishes (on_match_ready, the world shown) and then the game's own
//!   close runs for the reason the close came with. A lobby that says
//!   "the lobby" before the load is done (0x85 00) no longer makes
//!   on_project_loaded unload the level under the loading screen and return:
//!   the load goes on and is closed the same way;
//! - closed with its level loaded (waiting for its first world): the game's
//!   own close, past the guard, as a match that played.
//!
//! A match already closed (the guard's other meaning) and every played
//! match are left to the game. A logoff gives the match up without a close.

#![forbid(unsafe_code)]

use crate::vostok::match_close::{CloseReason, Closing, LoadEnd, MatchClose, ReadyMatch, Unplayed};
use crate::vostok::{
    Context, Flow, GameCell, NoExit, Thread, UnplayedMatchOnCloseExit, UnplayedMatchOnLoadEndExit,
};

/// The match the game's link was last connected for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Match {
    /// None open: never connected, closed, or given up at a logoff.
    Closed,
    /// Connected and not closed.
    Open,
    /// Closed while its level loaded: the close waits for the load's end.
    Abandoned(CloseReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    /// connect_to_match connects the link.
    Connected,
    /// network_client::disconnect (the logoff).
    SignedOff,
    /// close_current_match at its guard.
    Closing(Closing, CloseReason),
    /// on_project_loaded's abort test; whether the lobby says "the lobby".
    LoadEnd { in_lobby: bool },
    /// on_match_ready's end.
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    /// The game's code as it is.
    None,
    /// Past close_current_match's guard: the reason's own close.
    PastGuard,
    /// `31` to the lobby, then the guard's return.
    TellLobby,
    /// The load goes on to on_match_ready, not to the abort.
    LoadOn,
    /// close_current_match(reason) now.
    Close(CloseReason),
}

fn step(state: Match, event: Event) -> (Match, Action) {
    use Match::{Abandoned, Closed, Open};
    match (state, event) {
        (_, Event::Connected) => (Open, Action::None),
        (_, Event::SignedOff) => (Closed, Action::None),
        (_, Event::Closing(Closing::Played, _)) => (Closed, Action::None),
        (Closed, Event::Closing(Closing::Unplayed(_), _)) => (Closed, Action::None),
        (Open | Abandoned(_), Event::Closing(Closing::Unplayed(Unplayed::Loaded), _)) => (Closed, Action::PastGuard),
        (Open, Event::Closing(Closing::Unplayed(Unplayed::Loading), reason)) => (Abandoned(reason), Action::None),
        (Abandoned(first), Event::Closing(Closing::Unplayed(Unplayed::Loading), _)) => (Abandoned(first), Action::None),
        (Open | Abandoned(_), Event::Closing(Closing::Unplayed(Unplayed::Connecting), _)) => {
            (Closed, Action::TellLobby)
        }
        (Open, Event::LoadEnd { in_lobby: true }) => (Abandoned(CloseReason::TIMEOUT), Action::LoadOn),
        (Abandoned(reason), Event::LoadEnd { in_lobby: true }) => (Abandoned(reason), Action::LoadOn),
        (state, Event::LoadEnd { .. }) => (state, Action::None),
        (Abandoned(reason), Event::Ready) => (Open, Action::Close(reason)),
        (state, Event::Ready) => (state, Action::None),
    }
}

static MATCH: GameCell<Match> = GameCell::new(Match::Closed);

fn run(thread: &Thread, event: Event) -> Action {
    let (next, action) = step(MATCH.get(thread), event);
    MATCH.set(thread, next);
    action
}

/// connect_to_match connects (`unplayed.connect`).
pub fn on_connect(ctx: &mut Context<'_>) -> Flow<NoExit> {
    run(ctx.thread(), Event::Connected);
    Flow::Continue
}

/// The logoff (`unplayed.sign_off`).
pub fn on_sign_off(ctx: &mut Context<'_>) -> Flow<NoExit> {
    run(ctx.thread(), Event::SignedOff);
    Flow::Continue
}

/// close_current_match's guard (`unplayed.close_guard`).
pub fn on_close(ctx: &mut Context<'_>) -> Flow<UnplayedMatchOnCloseExit> {
    let Some(close) = MatchClose::at_guard(ctx) else { return Flow::Continue };
    match run(close.thread(), Event::Closing(close.closing(), close.reason())) {
        Action::PastGuard => Flow::Exit(UnplayedMatchOnCloseExit::Played),
        Action::TellLobby => {
            close.tell_lobby_back();
            Flow::Continue
        }
        _ => Flow::Continue,
    }
}

/// on_project_loaded's abort test (`unplayed.load_end`).
pub fn on_load_end(ctx: &mut Context<'_>) -> Flow<UnplayedMatchOnLoadEndExit> {
    let Some(load) = LoadEnd::at(ctx) else { return Flow::Continue };
    match run(load.thread(), Event::LoadEnd { in_lobby: load.in_lobby() }) {
        Action::LoadOn => Flow::Exit(UnplayedMatchOnLoadEndExit::Load),
        _ => Flow::Continue,
    }
}

/// on_match_ready's end (`unplayed.ready_end`).
pub fn on_ready(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(ready) = ReadyMatch::at(ctx) else { return Flow::Continue };
    if let Action::Close(reason) = run(ready.thread(), Event::Ready) {
        // The state is Open again: the guard takes this close past it.
        ready.close(reason);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOST: CloseReason = CloseReason::CONNECTION_LOST;

    fn unplayed(what: Unplayed) -> Event {
        Event::Closing(Closing::Unplayed(what), LOST)
    }

    fn run_all(events: &[Event]) -> (Match, Vec<Action>) {
        let mut state = Match::Closed;
        let mut actions = Vec::new();
        for &event in events {
            let (next, action) = step(state, event);
            state = next;
            actions.push(action);
        }
        (state, actions)
    }

    #[test]
    fn a_played_match_is_the_games() {
        for state in [Match::Closed, Match::Open, Match::Abandoned(LOST)] {
            assert_eq!(step(state, Event::Closing(Closing::Played, LOST)), (Match::Closed, Action::None));
        }
    }

    #[test]
    fn a_closed_match_stays_closed() {
        for what in [Unplayed::Connecting, Unplayed::Loading, Unplayed::Loaded] {
            assert_eq!(step(Match::Closed, unplayed(what)), (Match::Closed, Action::None));
        }
        assert_eq!(step(Match::Closed, Event::Ready), (Match::Closed, Action::None));
        assert_eq!(step(Match::Closed, Event::LoadEnd { in_lobby: true }), (Match::Closed, Action::None));
    }

    #[test]
    fn the_handshake_tells_the_lobby_once() {
        let (state, actions) = run_all(&[Event::Connected, unplayed(Unplayed::Connecting), unplayed(Unplayed::Connecting)]);
        assert_eq!(state, Match::Closed);
        assert_eq!(actions, [Action::None, Action::TellLobby, Action::None]);
    }

    #[test]
    fn a_loaded_match_goes_past_the_guard_once() {
        let (state, actions) = run_all(&[Event::Connected, unplayed(Unplayed::Loaded), unplayed(Unplayed::Loaded)]);
        assert_eq!(state, Match::Closed);
        assert_eq!(actions, [Action::None, Action::PastGuard, Action::None]);
    }

    #[test]
    fn a_loading_match_closes_after_its_load() {
        let (state, actions) = run_all(&[
            Event::Connected,
            unplayed(Unplayed::Loading),
            Event::LoadEnd { in_lobby: false },
            Event::Ready,
            unplayed(Unplayed::Loaded),
        ]);
        assert_eq!(state, Match::Closed);
        assert_eq!(actions, [Action::None, Action::None, Action::None, Action::Close(LOST), Action::PastGuard]);
    }

    #[test]
    fn the_lobby_before_the_load_ends_does_not_strand_it() {
        // Closed while loading, then 0x85 00: the load goes on, the first reason closes it.
        let (_, actions) =
            run_all(&[Event::Connected, unplayed(Unplayed::Loading), Event::LoadEnd { in_lobby: true }, Event::Ready]);
        assert_eq!(actions[2..], [Action::LoadOn, Action::Close(LOST)]);
        // 0x85 00 alone while loading: closed as a timeout once loaded.
        let (_, actions) = run_all(&[Event::Connected, Event::LoadEnd { in_lobby: true }, Event::Ready]);
        assert_eq!(actions[1..], [Action::LoadOn, Action::Close(CloseReason::TIMEOUT)]);
    }

    #[test]
    fn a_logoff_gives_the_match_up() {
        let (state, actions) = run_all(&[Event::Connected, Event::SignedOff, unplayed(Unplayed::Loaded)]);
        assert_eq!(state, Match::Closed);
        assert_eq!(actions[2], Action::None);
    }

    #[test]
    fn a_new_connect_opens_again() {
        let (state, _) = run_all(&[Event::Connected, unplayed(Unplayed::Loaded), Event::Connected]);
        assert_eq!(state, Match::Open);
    }
}
