//! Patch 130: a replay played in the running game (path A,
//! docs/notes/replay-stream.md; client_patch.md 3.76). The match server
//! streams a recording to a game with no place of its own (patch 121) and
//! runs its playback; its first playback note (an 0xF6, the
//! `replay-control` crate) makes the spectator's game a replay's viewer:
//!
//! - the playback the server tells it is the game's clock: paused
//!   (game::pause, the world and its sounds stand) and its speed (the
//!   replay controls' time_scale), 0.25x to 8x;
//! - its keys ask the server for a change (an 0xE0): Space pause, Left and
//!   Right 10 s back and on, Shift+Left and Shift+Right the previous and
//!   the next kill (4 s before it, from the server's events), `[` and `]`
//!   slower and faster; F11 and H hide the interface; Esc leaves, as the
//!   game's own leave does (an empty 0xE0, back to the lobby). F5, F6 and F2
//!   stay patch 121's.
//!
//! A live match's spectator (no playback note) and a player's game are not
//! touched: every key goes on to the game. Nothing a match computes changes:
//! the clock's pace is the server's to keep, the world the recording's.
#![forbid(unsafe_code)]

use replay_control::{newer, Event, EventKind, EventsNote, MatchMs, Op, Playback, Request, ServerNote, Speed};

use crate::vostok::replay_view::{GameClock, MatchClose, ServerNoteSite};
use crate::vostok::world::Role;
use crate::vostok::{
    Context, Dik, Flow, GameCell, GameWorldKey, KeyKind, NoExit, ReplayViewerOnKeyExit, ReplayViewerOnServerNoteExit,
};

/// What a replay's viewer key asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayKey {
    TogglePause,
    Back,
    Forward,
    PreviousKill,
    NextKill,
    Slower,
    Faster,
    ToggleUi,
    Leave,
}

impl ReplayKey {
    fn of(key: Dik, shift: bool) -> Option<ReplayKey> {
        Some(match key {
            Dik::SPACE => ReplayKey::TogglePause,
            Dik::LEFT if shift => ReplayKey::PreviousKill,
            Dik::RIGHT if shift => ReplayKey::NextKill,
            Dik::LEFT => ReplayKey::Back,
            Dik::RIGHT => ReplayKey::Forward,
            Dik::LEFT_BRACKET => ReplayKey::Slower,
            Dik::RIGHT_BRACKET => ReplayKey::Faster,
            Dik::F11 | Dik::H => ReplayKey::ToggleUi,
            Dik::ESCAPE => ReplayKey::Leave,
            _ => return None,
        })
    }
}

/// How far Left and Right go.
const SKIP_MS: u32 = 10_000;
/// A kill is played from this long before it.
const KILL_LEAD_MS: u32 = 4_000;
/// A kill this near the clock (once led) is the one just seen, not the next or the previous.
const KILL_MARGIN_MS: u32 = 1_000;

/// The interface's look (F11 and H).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ui {
    Shown,
    Hidden,
}

/// What a viewer last asked for: the next key's toggle and step start from
/// it until the server's next playback says otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Asked {
    paused: bool,
    speed: Speed,
}

/// A replay's viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Viewer {
    /// The server's last playback taken.
    playback: Playback,
    asked: Asked,
    ui: Ui,
    /// The next request's number.
    seq: u8,
}

/// The game in its match: a replay's viewer from the server's first
/// playback note until the match ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Session {
    /// A player's game, a live match's spectator, or no match.
    Live,
    Replay(Viewer),
}

/// What happens to the session.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Input {
    /// A match's options (0xE4): a new match.
    NewMatch,
    /// close_current_match past its guard: the match ends.
    MatchEnded,
    /// The server's playback note.
    Playback(Playback),
    /// A key went down; the match's clock, when the game has a world.
    Key { key: ReplayKey, now: Option<MatchMs> },
}

/// What the game does for an input.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Act {
    /// Not the viewer's: the game goes on as its own (a key goes to it).
    Pass,
    /// The viewer's, and nothing to do (a key taken).
    Nothing,
    /// The game's clock stands or runs, at this speed.
    Play { paused: bool, speed: Speed },
    /// A request to the server.
    Ask(Request),
    /// The interface shown or hidden.
    ShowUi(bool),
    /// Back to the lobby (the game's leave).
    Leave,
    /// The clock as every match has it (running, at 1).
    Restore,
}

/// The session's one transition, total over the inputs.
fn step(session: Session, input: Input, events: &Events) -> (Session, Act) {
    match (session, input) {
        (Session::Live, Input::NewMatch | Input::MatchEnded) => (Session::Live, Act::Pass),
        (Session::Replay(_), Input::NewMatch | Input::MatchEnded) => (Session::Live, Act::Restore),
        (Session::Live, Input::Playback(playback)) => {
            let viewer = Viewer { playback, asked: Asked::of(playback), ui: Ui::Shown, seq: 0 };
            (Session::Replay(viewer), Act::play(playback))
        }
        (Session::Replay(viewer), Input::Playback(playback)) if newer(playback.revision, viewer.playback.revision) => {
            (Session::Replay(Viewer { playback, asked: Asked::of(playback), ..viewer }), Act::play(playback))
        }
        (Session::Replay(viewer), Input::Playback(_)) => (Session::Replay(viewer), Act::Nothing),
        (Session::Live, Input::Key { .. }) => (Session::Live, Act::Pass),
        (Session::Replay(viewer), Input::Key { key, now }) => {
            let (viewer, act) = viewer.key(key, now, events);
            (Session::Replay(viewer), act)
        }
    }
}

impl Asked {
    fn of(playback: Playback) -> Asked {
        Asked { paused: playback.paused, speed: playback.speed }
    }
}

impl Act {
    fn play(playback: Playback) -> Act {
        Act::Play { paused: playback.paused, speed: playback.speed }
    }
}

impl Viewer {
    fn key(self, key: ReplayKey, now: Option<MatchMs>, events: &Events) -> (Viewer, Act) {
        let (start, end) = (self.playback.start, self.playback.end);
        let seek = |time: Option<MatchMs>| time.map(|time| Op::Seek(time.within(start, end)));
        let op = match key {
            ReplayKey::TogglePause => Some(Op::Pause(!self.asked.paused)),
            ReplayKey::Back => seek(now.map(|now| now.minus(SKIP_MS))),
            ReplayKey::Forward => seek(now.map(|now| now.plus(SKIP_MS))),
            ReplayKey::PreviousKill => seek(now.and_then(|now| events.previous_kill(now))),
            ReplayKey::NextKill => seek(now.and_then(|now| events.next_kill(now))),
            ReplayKey::Slower => Some(self.asked.speed.slower()).filter(|&s| s != self.asked.speed).map(Op::Speed),
            ReplayKey::Faster => Some(self.asked.speed.faster()).filter(|&s| s != self.asked.speed).map(Op::Speed),
            ReplayKey::ToggleUi => {
                let ui = match self.ui {
                    Ui::Shown => Ui::Hidden,
                    Ui::Hidden => Ui::Shown,
                };
                return (Viewer { ui, ..self }, Act::ShowUi(ui == Ui::Shown));
            }
            ReplayKey::Leave => return (self, Act::Leave),
        };
        let Some(op) = op else { return (self, Act::Nothing) };
        let asked = match op {
            Op::Pause(paused) => Asked { paused, ..self.asked },
            Op::Speed(speed) => Asked { speed, ..self.asked },
            Op::Seek(_) | Op::Leave => self.asked,
        };
        (Viewer { asked, seq: self.seq.wrapping_add(1), ..self }, Act::Ask(Request { seq: self.seq, op }))
    }
}

/// The most events a replay's list keeps here.
const MAX_EVENTS: usize = 256;

/// The replay's events as the server sent them (by their index in its
/// list), for the keys' previous and next kill. Kept apart from the session:
/// a note may come before the first playback note (the transport reorders).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Events {
    slots: [Option<Event>; MAX_EVENTS],
}

impl Events {
    const NONE: Events = Events { slots: [None; MAX_EVENTS] };

    fn take(mut self, note: &EventsNote<'_>) -> Events {
        for (index, event) in note.events() {
            if let Some(slot) = self.slots.get_mut(usize::from(index)) {
                *slot = event;
            }
        }
        self
    }

    /// The times kills are played from (each 4 s before its kill).
    fn kill_starts(&self) -> impl Iterator<Item = MatchMs> + '_ {
        self.slots.iter().flatten().filter(|event| event.kind == EventKind::Kill).map(|event| event.time.minus(KILL_LEAD_MS))
    }

    /// Where the first kill after the one being watched is played from.
    fn next_kill(&self, now: MatchMs) -> Option<MatchMs> {
        self.kill_starts().filter(|&start| start > now.plus(KILL_MARGIN_MS)).min()
    }

    /// Where the last kill before the one being watched is played from.
    fn previous_kill(&self, now: MatchMs) -> Option<MatchMs> {
        self.kill_starts().filter(|&start| start < now.minus(KILL_MARGIN_MS)).max()
    }
}

static SESSION: GameCell<Session> = GameCell::new(Session::Live);
static EVENTS: GameCell<Events> = GameCell::new(Events::NONE);

/// `match_packet.server_note`: an 0xF6. One of ours (its first byte 'R') is
/// taken here whatever it holds, and acted on only in a spectator's game; any
/// other (the snapshot upload's request) goes on to the game's case.
pub fn on_server_note(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnServerNoteExit> {
    let Some(site) = ServerNoteSite::at(ctx) else { return Flow::Continue };
    if !ServerNote::is_ours(site.payload) {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    if site.client.role() != Some(Role::Spectator) {
        // Never sent to a player's game: dropped, its keys stay the game's.
        return Flow::Exit(ReplayViewerOnServerNoteExit::Done);
    }
    match ServerNote::parse(site.payload) {
        Some(ServerNote::Playback(playback)) => {
            let (next, act) = step(SESSION.get(thread), Input::Playback(playback), &EVENTS.get(thread));
            SESSION.set(thread, next);
            if let (Act::Play { paused, speed }, Some(clock)) = (act, GameClock::of(&site.client)) {
                clock.set_paused(paused);
                clock.set_speed(speed.factor());
            }
        }
        Some(ServerNote::Events(note)) => EVENTS.update(thread, |events| events.take(&note)),
        Some(ServerNote::View(_)) | None => {}
    }
    Flow::Exit(ReplayViewerOnServerNoteExit::Done)
}

/// `tab_cursor.key` (after patch 86's): a viewer's keys, taken; every other
/// key, and every key of a game that is no viewer, go on.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnKeyExit> {
    let session = SESSION.get(ctx.thread());
    if session == Session::Live {
        return Flow::Continue;
    }
    let Some(key) = GameWorldKey::at_start(ctx) else { return Flow::Continue };
    let thread = key.thread();
    let shift = matches!(key.key(), Dik::LEFT | Dik::RIGHT) && key.shift_down();
    let Some(replay_key) = ReplayKey::of(key.key(), shift) else { return Flow::Continue };
    if key.kind() != KeyKind::Down {
        // A viewer's key held or let go: the game's player handler never sees it.
        key.take();
        return Flow::Exit(ReplayViewerOnKeyExit::Taken);
    }
    let client = key.hud().client(thread);
    let now = client.as_ref().and_then(|client| client.clock());
    let (next, act) = step(session, Input::Key { key: replay_key, now }, &EVENTS.get(thread));
    SESSION.set(thread, next);
    match act {
        Act::Ask(request) => {
            if let Some(client) = &client {
                client.send_request(request);
            }
        }
        Act::ShowUi(shown) => {
            let hud = key.hud();
            for movie in [hud.movie(), hud.icons_movie()].into_iter().flatten() {
                movie.invoke_bool(c"root.hide_interface", !shown);
            }
        }
        Act::Leave => {
            if let Some(client) = &client {
                client.leave();
            }
        }
        Act::Pass | Act::Nothing | Act::Play { .. } | Act::Restore => {}
    }
    key.take();
    Flow::Exit(ReplayViewerOnKeyExit::Taken)
}

/// `close_match.reason`: a match's end (every reason, past the guard) ends a
/// replay's viewing; the game's clock runs again at 1 before the world ends.
pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(close) = MatchClose::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let (next, act) = step(SESSION.get(thread), Input::MatchEnded, &EVENTS.get(thread));
    SESSION.set(thread, next);
    EVENTS.set(thread, Events::NONE);
    if let (Act::Restore, Some(clock)) = (act, GameClock::of(&close.client)) {
        clock.restore();
    }
    Flow::Continue
}

/// `static_info.options_read` (after patches 65, 86, 83 and 103's): a new
/// match starts as no replay, with no events.
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let (next, _) = step(SESSION.get(thread), Input::NewMatch, &EVENTS.get(thread));
    SESSION.set(thread, next);
    EVENTS.set(thread, Events::NONE);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playback(revision: u8, paused: bool, speed: Speed) -> Playback {
        Playback { revision, paused, speed, start: MatchMs(10_000), end: MatchMs(600_000) }
    }

    fn viewer() -> Session {
        step(Session::Live, Input::Playback(playback(3, false, Speed::Normal)), &Events::NONE).0
    }

    fn key(session: Session, key: ReplayKey, now: u32, events: &Events) -> (Session, Act) {
        step(session, Input::Key { key, now: Some(MatchMs(now)) }, events)
    }

    fn kill(time: u32) -> Option<Event> {
        Some(Event { time: MatchMs(time), kind: EventKind::Kill, actor: Some(1), victim: Some(2) })
    }

    #[test]
    fn the_first_playback_makes_a_viewer_and_plays_it() {
        let (session, act) = step(Session::Live, Input::Playback(playback(200, true, Speed::Half)), &Events::NONE);
        assert!(matches!(session, Session::Replay(_)));
        assert_eq!(act, Act::Play { paused: true, speed: Speed::Half });
    }

    #[test]
    fn a_live_game_passes_every_key_and_end() {
        for k in [ReplayKey::TogglePause, ReplayKey::Leave, ReplayKey::ToggleUi] {
            assert_eq!(key(Session::Live, k, 0, &Events::NONE), (Session::Live, Act::Pass));
        }
        assert_eq!(step(Session::Live, Input::MatchEnded, &Events::NONE), (Session::Live, Act::Pass));
    }

    #[test]
    fn only_a_newer_playback_is_taken() {
        let session = viewer();
        let (same, act) = step(session, Input::Playback(playback(2, true, Speed::Double)), &Events::NONE);
        assert_eq!((same, act), (session, Act::Nothing));
        let (_, act) = step(session, Input::Playback(playback(4, true, Speed::Double)), &Events::NONE);
        assert_eq!(act, Act::Play { paused: true, speed: Speed::Double });
    }

    #[test]
    fn space_asks_the_other_of_what_it_asked_last() {
        let (session, first) = key(viewer(), ReplayKey::TogglePause, 0, &Events::NONE);
        assert_eq!(first, Act::Ask(Request { seq: 0, op: Op::Pause(true) }));
        // Before the server's answer, a second press asks to play again.
        let (session, second) = key(session, ReplayKey::TogglePause, 0, &Events::NONE);
        assert_eq!(second, Act::Ask(Request { seq: 1, op: Op::Pause(false) }));
        // The server's playback is what the next press starts from.
        let (session, _) = step(session, Input::Playback(playback(4, true, Speed::Normal)), &Events::NONE);
        assert_eq!(key(session, ReplayKey::TogglePause, 0, &Events::NONE).1, Act::Ask(Request { seq: 2, op: Op::Pause(false) }));
    }

    #[test]
    fn arrows_seek_ten_seconds_within_the_replay() {
        assert_eq!(key(viewer(), ReplayKey::Forward, 50_000, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(60_000)) }));
        assert_eq!(key(viewer(), ReplayKey::Back, 15_000, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(10_000)) }));
        assert_eq!(key(viewer(), ReplayKey::Forward, 595_000, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(600_000)) }));
        // No world yet: nothing asked, the key still the viewer's.
        let (_, act) = step(viewer(), Input::Key { key: ReplayKey::Back, now: None }, &Events::NONE);
        assert_eq!(act, Act::Nothing);
    }

    #[test]
    fn shift_arrows_go_from_kill_to_kill() {
        let mut events = Events::NONE;
        events.slots[0] = kill(30_000);
        events.slots[1] = Some(Event { time: MatchMs(40_000), kind: EventKind::Objective, actor: None, victim: None });
        events.slots[5] = kill(60_000);
        let seek = |ms| Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(ms)) });
        assert_eq!(key(viewer(), ReplayKey::NextKill, 12_000, &events).1, seek(26_000));
        // Just sent to a kill (4 s before it): the next is the one after it.
        assert_eq!(key(viewer(), ReplayKey::NextKill, 26_000, &events).1, seek(56_000));
        assert_eq!(key(viewer(), ReplayKey::PreviousKill, 56_200, &events).1, seek(26_000));
        assert_eq!(key(viewer(), ReplayKey::PreviousKill, 26_000, &events).1, Act::Nothing);
        assert_eq!(key(viewer(), ReplayKey::NextKill, 56_000, &events).1, Act::Nothing);
    }

    #[test]
    fn brackets_step_the_speed_and_stop_at_the_ends() {
        let (session, act) = key(viewer(), ReplayKey::Faster, 0, &Events::NONE);
        assert_eq!(act, Act::Ask(Request { seq: 0, op: Op::Speed(Speed::Double) }));
        let (_, act) = key(session, ReplayKey::Faster, 0, &Events::NONE);
        assert_eq!(act, Act::Ask(Request { seq: 1, op: Op::Speed(Speed::Quadruple) }));
        let (slow, _) = step(Session::Live, Input::Playback(playback(0, false, Speed::Quarter)), &Events::NONE);
        assert_eq!(key(slow, ReplayKey::Slower, 0, &Events::NONE), (slow, Act::Nothing));
    }

    #[test]
    fn ui_toggles_and_esc_leaves() {
        let (session, act) = key(viewer(), ReplayKey::ToggleUi, 0, &Events::NONE);
        assert_eq!(act, Act::ShowUi(false));
        assert_eq!(key(session, ReplayKey::ToggleUi, 0, &Events::NONE).1, Act::ShowUi(true));
        assert_eq!(key(viewer(), ReplayKey::Leave, 0, &Events::NONE).1, Act::Leave);
    }

    #[test]
    fn a_match_end_or_a_new_match_restores_the_clock() {
        for input in [Input::MatchEnded, Input::NewMatch] {
            assert_eq!(step(viewer(), input, &Events::NONE), (Session::Live, Act::Restore));
        }
    }

    #[test]
    fn keys_map_with_shift() {
        assert_eq!(ReplayKey::of(Dik::LEFT, true), Some(ReplayKey::PreviousKill));
        assert_eq!(ReplayKey::of(Dik::LEFT, false), Some(ReplayKey::Back));
        assert_eq!(ReplayKey::of(Dik::H, true), Some(ReplayKey::ToggleUi));
        assert_eq!(ReplayKey::of(Dik::TAB, false), None);
    }
}
