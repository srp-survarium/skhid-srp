//! Patch 130: a replay played in the running game (path A,
//! docs/notes/replay-stream.md; client_patch.md 3.77). The match server
//! streams a recording to a game with no place of its own (patch 121) and
//! runs its playback; its first playback note (an 0xF6, the
//! `replay-control` crate) makes the spectator's game a replay's viewer:
//!
//! - the playback the server tells it is the game's clock: paused
//!   (game::pause, the world and its sounds stand) and its speed (the
//!   replay controls' time_scale), 0.25x to 8x;
//! - its keys ask the server for a change (an 0xE0): Space pause, Left and
//!   Right 10 s back and on, Shift+Left and Shift+Right the previous and
//!   the next moment the Moments switch lights (4 s before it, from the
//!   server's events), `[` and `]` slower and faster; M the switch's next
//!   choice (All, Mine, Followed); F11 and H hide the interface; Ctrl shows or
//!   hides the hud's cursor (the controls' bar and buttons take the mouse,
//!   the camera stands); Esc leaves, as the game's own leave does (an empty
//!   0xE0, back to the lobby); the controls' Back asks the server to let the
//!   game go (its farewell, as at the recording's end). F5, F6 and F2 stay
//!   patch 121's.
//!
//! A spectator's game, a live match's too, follows the place its watcher asked
//! for when the server names it (a view note), and picks no kit: its 1 to 3 and a pick of the
//! hud's chooser go nowhere. It shows the hud movie's own kit chooser, read-only, for the place in
//! view: the game fills it from its own copy of the match's options, with only the kits the
//! server's kits note allows (a replay: every playable kit; a live match: the ones the place has
//! played by the spectator's delayed clock), the initialize_match feed of the local place's kits
//! being withheld from a spectator. Live it is the game's own place; in a replay it is left of the
//! replay's player list, over the chat, and gives way to the Tab list. Nothing a match computes changes.
#![forbid(unsafe_code)]

use replay_control::{newer, Event, EventKind, EventsNote, MatchMs, Op, Playback, Request, ServerNote, Speed, KIT_NUMBERS};

use core::ffi::CStr;

use crate::spectator;
use crate::vostok::log::{line, Piece};
use crate::vostok::replay_view::{button_client, pointer_at, pointer_client, query_network, take_network, CoreLoad, GameClock, HudFrame, KitChooser, KitData, KitFeed, MatchClose, Names, Overlay, ServerNoteSite};
use crate::vostok::world::NetworkClient;
use crate::vostok::{
    Context, Dik, Flow, GameCell, GameWorldKey, HudCall, KeyKind, NoExit, ReplayViewerOnHudCallExit,
    ReplayViewerOnKeyExit, ReplayViewerOnMouseButtonExit, ReplayViewerOnMousePointerExit, ReplayViewerOnMovieQueryExit, ReplayViewerOnSetupProfilesExit, Reg,
    ReplayViewerOnMovieTakeExit, ReplayViewerOnServerNoteExit,
};

/// What a replay's viewer asks for, by a key or the replay controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    TogglePause,
    /// The bar's position, ms from the replay's start.
    SeekTo(u32),
    /// The speed stepper's value, as the nearest step.
    SetSpeed(Speed),
    Back,
    Forward,
    PreviousMoment,
    NextMoment,
    Slower,
    Faster,
    /// M: the Moments switch's next choice.
    NextMoments,
    /// A choice of the switch, clicked.
    SetMoments(Moments),
    ToggleUi,
    /// Ctrl: the hud's cursor on or off, for the controls' bar and buttons.
    ToggleCursor,
    Leave,
    /// The controls' Back: the server asked to let the game go (its
    /// farewell ends the world in the match's message handling; ending it
    /// from the controls' own call did nothing, and from the hud's next
    /// frame crashed the game in update_player_icons, 0x23AA46).
    BackButton,
}

impl Command {
    fn of(key: Dik, shift: bool) -> Option<Command> {
        Some(match key {
            Dik::SPACE => Command::TogglePause,
            Dik::LEFT if shift => Command::PreviousMoment,
            Dik::RIGHT if shift => Command::NextMoment,
            Dik::LEFT => Command::Back,
            Dik::RIGHT => Command::Forward,
            Dik::LEFT_BRACKET => Command::Slower,
            Dik::RIGHT_BRACKET => Command::Faster,
            Dik::M => Command::NextMoments,
            Dik::F11 | Dik::H => Command::ToggleUi,
            Dik::LEFT_CONTROL | Dik::RIGHT_CONTROL => Command::ToggleCursor,
            Dik::ESCAPE => Command::Leave,
            _ => return None,
        })
    }
}

/// How far Left and Right go.
const SKIP_MS: u32 = 10_000;
/// A moment is played from this long before it.
const MOMENT_LEAD_MS: u32 = 4_000;
/// A moment this near the clock (once led) is the one just seen, not the next or the previous.
const MOMENT_MARGIN_MS: u32 = 1_000;

/// Which of the events the Moments switch lights: their ticks on the bar
/// (the others faded) and Shift+Left/Right's steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Moments {
    All,
    /// The viewer's own: it played in the match ([`Own`]).
    Mine,
    /// The player in view's.
    Followed,
}

impl Moments {
    /// The first choice: Mine when the viewer played, else All.
    fn first(own: Own) -> Moments {
        if own.played() { Moments::Mine } else { Moments::All }
    }

    /// The choice after this one (M), in the switch's order, past Mine when
    /// the viewer did not play.
    fn next(self, own: Own) -> Moments {
        match self {
            Moments::All if own.played() => Moments::Mine,
            Moments::All | Moments::Mine => Moments::Followed,
            Moments::Followed => Moments::All,
        }
    }

    /// `wanted` when the viewer may choose it (Mine only for one who played).
    fn chosen(self, wanted: Moments, own: Own) -> Moments {
        if wanted == Moments::Mine && !own.played() { self } else { wanted }
    }

    /// The switch's index of the choice (the movie's set_moments and
    /// moments_clicked: 0 All, 1 Mine, 2 Followed).
    fn index(self) -> u32 {
        match self {
            Moments::All => 0,
            Moments::Mine => 1,
            Moments::Followed => 2,
        }
    }

    fn of_index(index: u32) -> Option<Moments> {
        Some(match index {
            0 => Moments::All,
            1 => Moments::Mine,
            2 => Moments::Followed,
            _ => return None,
        })
    }

    /// Whether this choice lights `event`, of the viewer's own places `own`
    /// and with `watched` in view.
    fn lights(self, event: &Event, own: Own, watched: Option<u8>) -> bool {
        match self {
            Moments::All => true,
            Moments::Mine => own.in_event(event),
            Moments::Followed => watched.is_some_and(|place| [event.actor, event.victim].contains(&Some(place))),
        }
    }
}

/// The viewer's own places in the replay: those under its account's name (a
/// player who left and came back may have had two). None: it did not play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Own(u32);

impl Own {
    /// The places among `places` whose name is `own` (ignoring ASCII case, as
    /// the lobby finds an account).
    fn by_name<'n>(own: Option<&[u8]>, places: u32, name: impl Fn(u8) -> Option<&'n [u8]>) -> Own {
        let Some(own) = own.filter(|own| !own.is_empty()) else { return Own(0) };
        let mask = (0..places.min(32) as u8)
            .filter(|&place| name(place).is_some_and(|name| name.eq_ignore_ascii_case(own)))
            .fold(0, |mask, place| mask | 1 << place);
        Own(mask)
    }

    fn played(self) -> bool {
        self.0 != 0
    }

    fn has(self, place: Option<u8>) -> bool {
        place.is_some_and(|place| place < 32 && self.0 & 1 << place != 0)
    }

    /// One of its places made the event or died in it.
    fn in_event(self, event: &Event) -> bool {
        self.has(event.actor) || self.has(event.victim)
    }
}

/// The interface's look (F11 and H), or the cursor's (Ctrl).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ui {
    Shown,
    Hidden,
}

impl Ui {
    fn other(self) -> Ui {
        match self {
            Ui::Shown => Ui::Hidden,
            Ui::Hidden => Ui::Shown,
        }
    }
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
    /// The hud's cursor (Ctrl): the controls take the mouse while it shows.
    cursor: Ui,
    /// The viewer's own places, and the Moments switch's choice.
    own: Own,
    moments: Moments,
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
    /// The server's playback note; the viewer's own places (the first note
    /// makes the viewer with them).
    Playback { playback: Playback, own: Own },
    /// A key went down or the controls were used; the match's clock, when
    /// the game has a world, and the player in view.
    Command { command: Command, now: Option<MatchMs>, watched: Option<u8> },
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
        (Session::Live, Input::Playback { playback, own }) => {
            let viewer = Viewer {
                playback,
                asked: Asked::of(playback),
                ui: Ui::Shown,
                cursor: Ui::Hidden,
                own,
                moments: Moments::first(own),
                seq: 0,
            };
            (Session::Replay(viewer), Act::play(playback))
        }
        (Session::Replay(viewer), Input::Playback { playback, .. }) if newer(playback.revision, viewer.playback.revision) => {
            (Session::Replay(Viewer { playback, asked: Asked::of(playback), ..viewer }), Act::play(playback))
        }
        (Session::Replay(viewer), Input::Playback { .. }) => (Session::Replay(viewer), Act::Nothing),
        (Session::Live, Input::Command { .. }) => (Session::Live, Act::Pass),
        (Session::Replay(viewer), Input::Command { command, now, watched }) => {
            let (viewer, act) = viewer.command(command, now, watched, events);
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
    fn command(self, command: Command, now: Option<MatchMs>, watched: Option<u8>, events: &Events) -> (Viewer, Act) {
        let (start, end) = (self.playback.start, self.playback.end);
        let seek = |time: Option<MatchMs>| time.map(|time| Op::Seek(time.within(start, end)));
        let lit = |event: &Event| self.moments.lights(event, self.own, watched);
        let op = match command {
            Command::TogglePause => Some(Op::Pause(!self.asked.paused)),
            Command::SeekTo(offset) => seek(Some(start.plus(offset))),
            Command::SetSpeed(speed) => Some(speed).filter(|&s| s != self.asked.speed).map(Op::Speed),
            Command::Back => seek(now.map(|now| now.minus(SKIP_MS))),
            Command::Forward => seek(now.map(|now| now.plus(SKIP_MS))),
            Command::PreviousMoment => seek(now.and_then(|now| events.previous_moment(now, lit))),
            Command::NextMoment => seek(now.and_then(|now| events.next_moment(now, lit))),
            Command::Slower => Some(self.asked.speed.slower()).filter(|&s| s != self.asked.speed).map(Op::Speed),
            Command::Faster => Some(self.asked.speed.faster()).filter(|&s| s != self.asked.speed).map(Op::Speed),
            Command::NextMoments => return (Viewer { moments: self.moments.next(self.own), ..self }, Act::Nothing),
            Command::SetMoments(wanted) => {
                return (Viewer { moments: self.moments.chosen(wanted, self.own), ..self }, Act::Nothing);
            }
            Command::ToggleUi => {
                let ui = self.ui.other();
                return (Viewer { ui, ..self }, Act::ShowUi(ui == Ui::Shown));
            }
            Command::ToggleCursor => return (Viewer { cursor: self.cursor.other(), ..self }, Act::Nothing),
            Command::BackButton => Some(Op::Leave),
            Command::Leave => return (self, Act::Leave),
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

/// The most events a viewer keeps: more than any real match's, whose kills and
/// objectives are 20 players' for 15 minutes (the arena is nearly full: each
/// slot is 12 bytes), and the rest are only the bar's marks. A replay with
/// more: [`Events::take`] tells the log.
const MAX_EVENTS: usize = 512;

/// The replay's events as the server sent them (by their index in its
/// list), for the keys' previous and next moment. Kept apart from the session:
/// a note may come before the first playback note (the transport reorders).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Events {
    slots: [Option<Event>; MAX_EVENTS],
    /// Counts the notes taken (the controls' markers follow it).
    version: u16,
    /// Whether this match's line about events past [`MAX_EVENTS`] was written.
    capacity: Capacity,
}

/// Events past [`MAX_EVENTS`]: none, or some and told once a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Capacity {
    Enough,
    Exceeded,
}

/// A replay with events past [`MAX_EVENTS`] ([`Events::take`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TooManyEvents {
    total: u16,
}

impl Events {
    const NONE: Events = Events { slots: [None; MAX_EVENTS], version: 0, capacity: Capacity::Enough };

    /// Takes a note. `Err` the first time a note names an event past
    /// [`MAX_EVENTS`]: its marks are lost and the log must say so.
    fn take(&mut self, note: &EventsNote<'_>) -> Result<(), TooManyEvents> {
        let mut over = false;
        for (index, event) in note.events() {
            match self.slots.get_mut(usize::from(index)) {
                Some(slot) => *slot = event,
                None => over = true,
            }
        }
        self.version = self.version.wrapping_add(1);
        if over && self.capacity == Capacity::Enough {
            self.capacity = Capacity::Exceeded;
            return Err(TooManyEvents { total: note.total });
        }
        Ok(())
    }

    /// The controls' markers text (`time_ms:kind:whose:actor:victim;...`, the
    /// time from the replay's `start`), whole ([`MARKERS_ROOM`] is sized for
    /// [`MAX_EVENTS`]); an event before the start is left out. The kind is the
    /// movie's tick: 1 a kill, 2 an objective, 3 a death of the player
    /// `watched`; whose, what the Moments switch lights it under: 1 one of the
    /// viewer's `own` places in it, 2 the player `watched`, 3 both, 0 neither;
    /// actor and victim its places, named by the movie's list
    /// (root.set_marker_names), empty for none (a death no one made has only a
    /// victim, an objective only an actor).
    fn markers<'o>(&self, start: MatchMs, own: Own, watched: Option<u8>, out: &'o mut [u8; MARKERS_ROOM]) -> &'o CStr {
        let mut text = Text::new(out);
        for event in self.slots.iter().flatten() {
            let Some(offset) = event.time.0.checked_sub(start.0) else { continue };
            let kind = match event.kind {
                EventKind::Kill if event.victim.is_some() && event.victim == watched => 3,
                EventKind::Kill => 1,
                EventKind::Objective => 2,
            };
            let (actor, victim) = match event.kind {
                EventKind::Kill => (event.actor, event.victim),
                EventKind::Objective => (event.actor, None),
            };
            let whose = u32::from(Moments::Mine.lights(event, own, watched))
                | u32::from(Moments::Followed.lights(event, own, watched)) << 1;
            let fits = (text.len == 0 || text.push(b";")) && text.number(offset) && text.push(b":")
                && text.number(kind) && text.push(b":") && text.number(whose) && text.push(b":")
                && text.place(actor) && text.push(b":") && text.place(victim);
            // MARKERS_ROOM holds MAX_EVENTS at their longest (a test proves it).
            debug_assert!(fits);
            if !fits {
                break;
            }
        }
        text.finish()
    }

    /// The times the `lit` events are played from (each 4 s before it).
    fn moment_starts<'e>(&'e self, lit: impl Fn(&Event) -> bool + 'e) -> impl Iterator<Item = MatchMs> + 'e {
        self.slots.iter().flatten().filter(move |event| lit(event)).map(|event| event.time.minus(MOMENT_LEAD_MS))
    }

    /// Where the first lit moment after the one being watched is played from.
    fn next_moment(&self, now: MatchMs, lit: impl Fn(&Event) -> bool) -> Option<MatchMs> {
        self.moment_starts(lit).filter(|&start| start > now.plus(MOMENT_MARGIN_MS)).min()
    }

    /// Where the last lit moment before the one being watched is played from.
    fn previous_moment(&self, now: MatchMs, lit: impl Fn(&Event) -> bool) -> Option<MatchMs> {
        self.moment_starts(lit).filter(|&start| start < now.minus(MOMENT_MARGIN_MS)).max()
    }
}

/// The most places a match has (`sur::MAX_PLAYERS`, and more as a margin).
const MAX_PLACES: usize = 32;

/// Which of each place's kits the server's notes allow the game to show. A live match's grows; a
/// place's note says its whole mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KitMasks {
    masks: [u8; MAX_PLACES],
    /// Counts the notes taken.
    version: u16,
}

impl KitMasks {
    const NONE: KitMasks = KitMasks { masks: [0; MAX_PLACES], version: 0 };

    fn take(mut self, place: u8, mask: u8) -> KitMasks {
        if let Some(slot) = self.masks.get_mut(usize::from(place)) {
            *slot = mask;
            self.version = self.version.wrapping_add(1);
        }
        self
    }

    fn of(&self, place: u8) -> u8 {
        self.masks.get(usize::from(place)).copied().unwrap_or(0)
    }
}

static SESSION: GameCell<Session> = GameCell::new(Session::Live);
static EVENTS: GameCell<Events> = GameCell::new(Events::NONE);
static MASKS: GameCell<KitMasks> = GameCell::new(KitMasks::NONE);
/// Whether `key` is one of the kit keys (patch 58's 1, 2 and 3).
fn is_kit_key(key: Dik) -> bool {
    matches!(key, Dik::KEY_1 | Dik::KEY_2 | Dik::KEY_3)
}

/// `match_packet.server_note`: an 0xF6. One of ours (its first byte 'R') is
/// taken here whatever it holds, and acted on only in a spectator's game; any
/// other (the snapshot upload's request) goes on to the game's case.
pub fn on_server_note(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnServerNoteExit> {
    let Some(site) = ServerNoteSite::at(ctx) else { return Flow::Continue };
    if !ServerNote::is_ours(site.payload) {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    if !site.client.is_spectator() {
        // Never sent to a player's game: dropped, its keys stay the game's.
        return Flow::Exit(ReplayViewerOnServerNoteExit::Done);
    }
    match ServerNote::parse(site.payload) {
        Some(ServerNote::Playback(playback)) => {
            let names = Names::of(&site.client);
            let own = Own::by_name(names.own(), site.client.places(), |place| names.of_place(place));
            let (next, act) = EVENTS.with(thread, |events| step(SESSION.get(thread), Input::Playback { playback, own }, events));
            SESSION.set(thread, next);
            site.client.become_replay_viewer();
            if let (Act::Play { paused, speed }, Some(clock)) = (act, GameClock::of(&site.client)) {
                clock.set_paused(paused);
                clock.set_speed(speed.factor());
            }
        }
        Some(ServerNote::Events(note)) => {
            if let Err(TooManyEvents { total }) = EVENTS.with(thread, |events| events.take(&note)) {
                line(&[Piece::Text("replay_viewer: the replay has "), Piece::Dec(u32::from(total)),
                       Piece::Text(" events, the viewer keeps "), Piece::Dec(MAX_EVENTS as u32), Piece::Text(": the rest have no marks")]);
            }
        }
        Some(ServerNote::Kits(note)) => MASKS.update(thread, |masks| masks.take(note.place, note.mask)),
        // The place its watcher asked to see (a replay's or a live match's
        // spectator, once its first world is in): followed, as a player list
        // click does.
        Some(ServerNote::View(place)) => spectator::view_player(&site.client, thread, place),
        None => {}
    }
    Flow::Exit(ReplayViewerOnServerNoteExit::Done)
}

/// `tab_cursor.key` (after patch 86's): a viewer's keys, taken; every other
/// key, and every key of a game that is no viewer, go on.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnKeyExit> {
    let Some(key) = GameWorldKey::at_start(ctx) else { return Flow::Continue };
    let thread = key.thread();
    // A spectator has no kits: its 1 to 3 pick nothing (before patch 58's key, further in this
    // function; the key would reach the kit chooser the game shows it).
    if is_kit_key(key.key()) && key.kind() == KeyKind::Down
        && key.hud().client(thread).is_some_and(|client| client.is_spectator()) {
        key.take();
        return Flow::Exit(ReplayViewerOnKeyExit::Taken);
    }
    let session = SESSION.get(thread);
    if session == Session::Live {
        return Flow::Continue;
    }
    let shift = matches!(key.key(), Dik::LEFT | Dik::RIGHT) && key.shift_down();
    let Some(replay_key) = Command::of(key.key(), shift) else { return Flow::Continue };
    if key.kind() != KeyKind::Down {
        // A viewer's key held or let go: the game's player handler never sees it.
        key.take();
        return Flow::Exit(ReplayViewerOnKeyExit::Taken);
    }
    let client = key.hud().client(thread);
    let now = client.as_ref().and_then(|client| client.clock());
    let watched = client.as_ref().and_then(|client| client.local_place());
    let (next, act) = EVENTS.with(thread, |events| step(session, Input::Command { command: replay_key, now, watched }, events));
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
    let (next, act) = EVENTS.with(thread, |events| step(SESSION.get(thread), Input::MatchEnded, events));
    SESSION.set(thread, next);
    EVENTS.with(thread, |events| *events = Events::NONE);
    MASKS.set(thread, KitMasks::NONE);
    // A world may tick on after this: nothing is shown again until the next match.
    FED.set(thread, Fed::ENDED);
    if close.client.is_spectator()
        && let Some(chooser) = KitChooser::of(&close.client)
    {
        chooser.show(CHOOSER_HIDDEN);
    }
    if let (Act::Restore, Some(clock)) = (act, GameClock::of(&close.client)) {
        clock.restore();
    }
    if act == Act::Restore {
        // The movie outlives the match: a viewer left by Back, whose world the server's farewell
        // ends, would otherwise keep its controls and cursor over the next real match.
        if let Some(overlay) = Overlay::of(&close.client) {
            overlay.release_held_buttons();
            overlay.set_cursor_shown(false);
            overlay.keep_cursor(false);
            overlay.show(false);
        }
    }
    Flow::Continue
}

/// `static_info.options_read` (after patches 65, 86, 83 and 103's): a new
/// match starts as no replay, with no events.
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let (next, _) = EVENTS.with(thread, |events| step(SESSION.get(thread), Input::NewMatch, events));
    SESSION.set(thread, next);
    EVENTS.with(thread, |events| *events = Events::NONE);
    MASKS.set(thread, KitMasks::NONE);
    FED.set(thread, Fed::NOTHING);
    Flow::Continue
}

/// `hud.setup_profiles`: initialize_match's feed of the local place's kits. A spectator has none,
/// and would be given place 0's, used or not: its chooser is filled from the notes instead. Asked
/// here, of the place the feed was built for: at the static info no place is local yet (the game
/// marks its own later in the same handler, 0x27BE5F), and a player would count as a spectator.
pub fn on_setup_profiles(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnSetupProfilesExit> {
    if KitFeed::at(ctx).is_some_and(|feed| !feed.is_the_games_own()) {
        ctx.set_reg(Reg::Esi, 0); // what the `xor esi,esi` it skips leaves
        return Flow::Exit(ReplayViewerOnSetupProfilesExit::Skip);
    }
    Flow::Continue
}

/// One marker at most: `4294967295:3:3:255:255;`.
const MARKER_MAX: usize = 10 + 1 + 1 + 1 + 1 + 1 + 3 + 1 + 3 + 1;
/// The markers text's room, its zero included: every event of the list.
const MARKERS_ROOM: usize = MAX_EVENTS * MARKER_MAX + 1;
/// A name the movie is given at most (it never cuts one): two and the arrow fit its label.
const NAME_MAX: usize = 30;
/// One place of the names text at most: `255:name;`.
const NAME_ENTRY_MAX: usize = 3 + 1 + NAME_MAX + 1;
/// The names text's room, its zero included: every place.
const NAMES_ROOM: usize = MAX_PLACES * NAME_ENTRY_MAX + 1;
/// The hud chooser's text's room: its three kits at their most (a name of 64
/// bytes, 23 items of `65535:255:65535,`) with their fields.
const CHOOSER_ROOM: usize = KIT_NUMBERS as usize * (3 + 1 + 10 + 1 + 5 + 1 + 64 + 1 + 23 * (5 + 1 + 3 + 1 + 5 + 1) + 1) + 1;

/// A text written into a fixed room, kept one byte short for its zero.
struct Text<'o> {
    out: &'o mut [u8],
    len: usize,
}

impl<'o> Text<'o> {
    fn new(out: &'o mut [u8]) -> Text<'o> {
        Text { out, len: 0 }
    }

    fn push(&mut self, bytes: &[u8]) -> bool {
        let end = self.len + bytes.len();
        if end >= self.out.len() {
            return false;
        }
        self.out[self.len..end].copy_from_slice(bytes);
        self.len = end;
        true
    }

    fn number(&mut self, value: u32) -> bool {
        let mut digits = [0u8; 10];
        let (mut n, mut at) = (value, digits.len());
        loop {
            at -= 1;
            digits[at] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.push(&digits[at..])
    }

    /// A place's number, or nothing for none.
    fn place(&mut self, place: Option<u8>) -> bool {
        place.is_none_or(|place| self.number(u32::from(place)))
    }

    /// A name, cut to [`NAME_MAX`] bytes on a character's start; a `;` (the
    /// entries' separator) becomes `,`.
    fn name(&mut self, name: &[u8]) -> bool {
        let mut cut = name.len().min(NAME_MAX);
        while cut > 0 && cut < name.len() && name[cut] & 0xC0 == 0x80 {
            cut -= 1;
        }
        name[..cut].iter().all(|&b| self.push(&[if b == b';' { b',' } else { b }]))
    }

    fn finish(self) -> &'o CStr {
        self.out[self.len] = 0;
        CStr::from_bytes_until_nul(&self.out[..]).unwrap_or(c"")
    }
}

/// The names the movie's markers show, by place: `place:name;...`.
fn names_text<'o, 'n>(places: u32, name: impl Fn(u8) -> Option<&'n [u8]>, out: &'o mut [u8; NAMES_ROOM]) -> &'o CStr {
    let mut text = Text::new(out);
    for place in (0..places.min(MAX_PLACES as u32)).map(|place| place as u8) {
        if let Some(name) = name(place) {
            let fits = (text.len == 0 || text.push(b";")) && text.number(u32::from(place)) && text.push(b":") && text.name(name);
            debug_assert!(fits, "NAMES_ROOM holds every place");
            if !fits {
                break;
            }
        }
    }
    text.finish()
}

/// What the replay controls were last given (they are fed each frame from
/// the session): kept apart from the session, as the movie's own state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fed {
    /// The movie made a viewer's (its Back button, Moments switch and key
    /// hint, with their texts): its id.
    movie: Option<usize>,
    /// The playback revision its bar's length is from.
    revision: Option<u8>,
    /// What its markers are from.
    markers: Option<MarkersOf>,
    /// Shown or hidden.
    ui: Option<Ui>,
    /// The switch's choice it shows, and the cursor it shows the key hint for.
    moments: Option<Moments>,
    cursor: Option<Ui>,
    /// What the hud's kit chooser has been given, and whether the match has ended.
    chooser: Option<ChooserFed>,
    ended: MatchEnd,
}

impl Fed {
    const NOTHING: Fed = Fed { movie: None, revision: None, markers: None, ui: None, moments: None, cursor: None, chooser: None, ended: MatchEnd::Running };
    const ENDED: Fed = Fed { ended: MatchEnd::Ended, ..Fed::NOTHING };
}

/// The dictionary keys of the viewer's own controls, in root.start_viewer's
/// order: the Back button, the Moments switch and its choices, the key hint's
/// actions and its Ctrl and Space caps.
const VIEWER_TEXTS: [&CStr; 11] = [c"st_srp_viewer_back", c"st_srp_viewer_moments", c"st_srp_viewer_moments_all",
                                   c"st_srp_viewer_moments_mine", c"st_srp_viewer_moments_followed",
                                   c"st_srp_viewer_cursor", c"st_srp_viewer_pause", c"st_srp_viewer_filter",
                                   c"st_srp_viewer_hide", c"st_srp_viewer_cap_ctrl", c"st_srp_viewer_cap_space"];

/// The markers' sources: the events' version, and the player watched (whose
/// deaths are their own kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MarkersOf {
    events: u16,
    watched: Option<u8>,
}

/// Whether the frames that remain still belong to a running match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatchEnd {
    Running,
    Ended,
}

/// What the hud's kit chooser shows: the place, the notes' version and the current kit it was
/// given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChooserFed {
    place: Option<u8>,
    version: u16,
    current: Option<u8>,
}

/// The chooser's modes (`HUD.srp_spectator_kits_show`).
const CHOOSER_HIDDEN: u32 = 0;
const CHOOSER_LIVE: u32 = 1;
const CHOOSER_REPLAY: u32 = 2;

static FED: GameCell<Fed> = GameCell::new(Fed::NOTHING);

/// The controls of a viewer's game, brought to the session: made a viewer's
/// once (Back, the Moments switch, the key hint), shown or hidden with the
/// interface, the bar's length and the speed stepper at each playback, the
/// markers and the switch when they changed, the cursor for the key hint, its
/// position and play button every frame (as the replay client's tick does,
/// 0x218C16 / 0x218C27), the hud's kit chooser and quick slots kept hidden,
/// and its cursor kept as Ctrl left it.
fn feed(client: &NetworkClient<'_>, viewer: &Viewer, fed: Fed, thread: &crate::vostok::Thread) -> Fed {
    let Some(overlay) = Overlay::of(client) else { return fed };
    let Playback { revision, paused, speed, start, end } = viewer.playback;
    let movie = Some(overlay.id()).filter(|&id| fed.movie == Some(id) || overlay.start_viewer(&VIEWER_TEXTS));
    if fed.ui != Some(viewer.ui) {
        overlay.show(viewer.ui == Ui::Shown);
    }
    if fed.revision != Some(revision) {
        overlay.set_length(end.0.saturating_sub(start.0));
        overlay.set_speed(speed.percent());
    }
    let watched = client.local_place();
    let markers = MarkersOf { events: EVENTS.with(thread, |events| events.version), watched };
    if fed.markers != Some(markers) || fed.revision.is_none() {
        let names = Names::of(client);
        let mut names_room = [0; NAMES_ROOM];
        overlay.set_marker_names(names_text(client.places(), |place| names.of_place(place), &mut names_room));
        let mut room = [0; MARKERS_ROOM];
        let text = EVENTS.with(thread, |events| events.markers(start, viewer.own, watched, &mut room));
        overlay.set_markers(text);
    }
    if fed.moments != Some(viewer.moments) {
        overlay.set_moments(viewer.moments.index(), viewer.own.played());
    }
    if fed.cursor != Some(viewer.cursor) {
        overlay.set_cursor_shown(viewer.cursor == Ui::Shown);
    }
    if let Some(now) = client.clock() {
        overlay.set_position(now.within(start, end).0 - start.0);
    }
    overlay.set_paused(paused);
    overlay.hide_player_panels();
    overlay.keep_cursor(viewer.cursor == Ui::Shown);
    let chooser = feed_chooser(client, fed.chooser, CHOOSER_REPLAY, thread);
    Fed {
        movie,
        revision: Some(revision),
        markers: Some(markers),
        ui: Some(viewer.ui),
        moments: Some(viewer.moments),
        cursor: Some(viewer.cursor),
        chooser,
        ended: MatchEnd::Running,
    }
}

/// The hud's chooser brought up to date with the place in view: its kits (only those a note
/// allows, read from the game's own copy) when the place or the notes changed, the kit that place
/// has now, and its place on the screen (each frame: the game's own show_profiles_selector calls
/// would hide or show it).
fn feed_chooser(client: &NetworkClient<'_>, fed: Option<ChooserFed>, mode: u32, thread: &crate::vostok::Thread) -> Option<ChooserFed> {
    let chooser = KitChooser::of(client)?;
    let place = client.local_place();
    let masks = MASKS.get(thread);
    // New plates lose the highlight, so it is told again; a kit without a plate has none to move to.
    let new_plates = fed.map(|fed| (fed.place, fed.version)) != Some((place, masks.version));
    if new_plates {
        let mut room = [0; CHOOSER_ROOM];
        let mut names = [[0; 0x200]; KIT_NUMBERS as usize];
        let named = &chooser;
        chooser.set_kits(chooser_text(place.map(|place| {
            (0..KIT_NUMBERS).zip(names.iter_mut()).filter(move |&(kit, _)| masks.of(place) & 1 << kit != 0).filter_map(move |(kit, name)| {
                Some(named.named(client.kit_data(place, kit)?, name))
            })
        }), &mut room));
    }
    let current = place.and_then(|place| client.current_kit(place).filter(|&kit| masks.of(place) & 1 << kit != 0));
    if let (Some(kit), true) = (current, new_plates || fed.map(|fed| fed.current) != Some(current)) {
        chooser.set_current(kit);
    }
    chooser.show(mode);
    Some(ChooserFed { place, version: masks.version, current })
}

/// The kits' text for the hud's chooser: `<kit>|<state>|<level>|<name>|<dict>:<slot>:<count>,...;`
/// a kit; a name's `|`, `;`, `,` and `:` are spaces.
fn chooser_text<'o, 'k>(kits: Option<impl Iterator<Item = KitData<'k>>>, out: &'o mut [u8; CHOOSER_ROOM]) -> &'o CStr {
    let mut text = Text::new(out);
    for kit in kits.into_iter().flatten() {
        let mark = text.len;
        let fits = text.number(u32::from(kit.number)) && text.push(b"|") && text.number(kit.state) && text.push(b"|")
            && text.number(u32::from(kit.level)) && text.push(b"|")
            && kit.name.iter().all(|&byte| text.push(&[if b"|;,:".contains(&byte) { b' ' } else { byte }]))
            && text.push(b"|")
            && kit.items.iter().flatten().enumerate().all(|(at, &(dict, slot, count))| {
                (at == 0 || text.push(b",")) && text.number(u32::from(dict)) && text.push(b":") && text.number(u32::from(slot))
                    && text.push(b":") && text.number(u32::from(count))
            })
            && text.push(b";");
        debug_assert!(fits, "CHOOSER_ROOM holds the kits at their longest");
        if !fits {
            text.len = mark;
            break;
        }
    }
    text.finish()
}

/// `hud.update`(game_world_ui::update_ui, every frame of the hud): a
/// viewer's controls fed for this frame.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(hud) = HudFrame::at(ctx) else { return Flow::Continue };
    let Some(client) = hud.client(thread) else { return Flow::Continue };
    match SESSION.get(thread) {
        Session::Replay(viewer) => FED.set(thread, feed(&client, &viewer, FED.get(thread), thread)),
        // A live match's spectator: the kit chooser and the slots stay hidden (it may not have
        // the replay controls' movie, whose Overlay a viewer's frame goes through).
        Session::Live if client.is_spectator() => {
            hud.hide_player_panels();
            hud.keep_cursor_off();
            let fed = FED.get(thread);
            if fed.ended == MatchEnd::Running {
                FED.set(thread, Fed { chooser: feed_chooser(&client, fed.chooser, CHOOSER_LIVE, thread), ..fed });
            }
        }
        Session::Live => {}
    }
    Flow::Continue
}

/// `world_load.unpacked` and `world_load.raw_args`: every world a viewer
/// is sent is the server starting it again from the recording (a seek, a
/// speed's or a drift's restart), so it is loaded as the replay's own seek
/// loads one, its pending inputs and masks cleared; an older world loaded as
/// a network one crashed the game (0x2D0DA4). A live game's go on as its own.
fn load_as_seek(load: Option<CoreLoad>, thread: &crate::vostok::Thread) {
    if let (Session::Replay(_), Some(load)) = (SESSION.get(thread), load) {
        load.as_seek();
    }
}

pub fn on_world_unpacked(ctx: &mut Context<'_>) -> Flow<NoExit> {
    load_as_seek(CoreLoad::at_unpacked(ctx), ctx.thread());
    Flow::Continue
}

pub fn on_world_raw(ctx: &mut Context<'_>) -> Flow<NoExit> {
    load_as_seek(CoreLoad::at_raw(ctx), ctx.thread());
    Flow::Continue
}

/// `replay_movie.query`: a network game's first world queries the replay
/// controls' movie too (hidden until a replay's viewer shows it).
pub fn on_movie_query(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnMovieQueryExit> {
    if query_network(ctx) { Flow::Exit(ReplayViewerOnMovieQueryExit::Load) } else { Flow::Continue }
}

/// `replay_movie.take`: and takes it from the query's results.
pub fn on_movie_take(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnMovieTakeExit> {
    if take_network(ctx) { Flow::Exit(ReplayViewerOnMovieTakeExit::Take) } else { Flow::Continue }
}

/// A hud with the replay controls gives them the mouse (its buttons and
/// pointer) instead of the hud movie: a network game's only while it is a
/// viewer's, so a player's kits and a live spectator's hud keep it. The
/// game's own replay client goes on as the game's.
fn mouse_to_hud(client: Option<NetworkClient<'_>>, thread: &crate::vostok::Thread) -> bool {
    client.is_some() && !matches!(SESSION.get(thread), Session::Replay(_))
}

/// `hud_mouse.replay_button`.
pub fn on_mouse_button(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnMouseButtonExit> {
    if mouse_to_hud(button_client(ctx), ctx.thread()) { Flow::Exit(ReplayViewerOnMouseButtonExit::Hud) } else { Flow::Continue }
}

/// `hud_mouse.replay_pointer`.
pub fn on_mouse_pointer(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnMousePointerExit> {
    let thread = ctx.thread();
    let client = pointer_client(ctx);
    // A viewer's pointer goes to the controls; while the Ctrl cursor is up, the hud's chooser is
    // told where it is too, for its hover list.
    if let (Some(client), Session::Replay(Viewer { cursor: Ui::Shown, .. }), Some((x, y))) = (&client, SESSION.get(thread), pointer_at(ctx))
        && let Some(chooser) = KitChooser::of(client)
    {
        chooser.pointer(x, y);
    }
    if mouse_to_hud(client, thread) { Flow::Exit(ReplayViewerOnMousePointerExit::Hud) } else { Flow::Continue }
}

/// The replay controls' calls a network game takes from its hud's
/// scaleform_callback (0x233D30), whose own cases are a replay client's:
/// update_position seeks with the replay reader (0x1EE2E0), the free view
/// and the player list call vtable slots past a network client's.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ControlCall {
    Play,
    Position(u32),
    Speed(f64),
    Player(u8),
    FreeView,
    /// The Moments switch's choice clicked (srp's).
    Moments(Moments),
    /// The Back button (srp's).
    Back,
    Other,
}

impl ControlCall {
    fn of(call: &HudCall<'_>) -> Option<ControlCall> {
        let number = |index| call.number_argument(index).filter(|n| n.is_finite() && *n >= 0.0);
        Some(match call.method().to_bytes() {
            b"button_play_clicked" => ControlCall::Play,
            b"update_position" => number(0).map_or(ControlCall::Other, |n| ControlCall::Position(n.min(f64::from(u32::MAX)) as u32)),
            b"update_play_speed" => number(0).map_or(ControlCall::Other, ControlCall::Speed),
            b"select_player" => number(0).filter(|&n| n < 256.0).map_or(ControlCall::Other, |n| ControlCall::Player(n as u8)),
            b"select_free_view" => ControlCall::FreeView,
            b"moments_clicked" => number(0).and_then(|n| Moments::of_index(n as u32)).map_or(ControlCall::Other, ControlCall::Moments),
            b"back_clicked" => ControlCall::Back,
            b"camera_smooth_changed" => ControlCall::Other,
            _ => return None,
        })
    }
}

/// `hud.callback` (after patch 86's): the replay controls in a network
/// game. A viewer's become requests to the server (play, the bar, the
/// speed, Back's leave), patch 121's camera (a player, the free view) or the
/// Moments switch; any other game's are taken with nothing done.
/// A replay client's, and every other call, go on as the game's.
pub fn on_hud_call(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnHudCallExit> {
    let Some(call) = HudCall::at(ctx) else { return Flow::Continue };
    // A spectator picks no kit: the chooser's pick of a spectator's game is dropped (the movie
    // would tell the game profile_changed).
    if call.method().to_bytes() == b"profile_changed" {
        let spectator = call.hud().client(call.thread()).is_some_and(|client| client.is_spectator());
        return if spectator { Flow::Exit(ReplayViewerOnHudCallExit::Done) } else { Flow::Continue };
    }
    let Some(control) = ControlCall::of(&call) else { return Flow::Continue };
    let thread = call.thread();
    let Some(client) = call.hud().client(thread).filter(|client| client.is_match_client()) else {
        return Flow::Continue;
    };
    let session = SESSION.get(thread);
    if session == Session::Live {
        return Flow::Exit(ReplayViewerOnHudCallExit::Done);
    }
    let command = match control {
        ControlCall::Play => Some(Command::TogglePause),
        ControlCall::Position(offset) => Some(Command::SeekTo(offset)),
        ControlCall::Speed(factor) => Some(Command::SetSpeed(Speed::nearest(factor as f32))),
        ControlCall::Player(place) => {
            spectator::view_player(&client, thread, place);
            None
        }
        ControlCall::FreeView => {
            spectator::view_free(&client, thread);
            None
        }
        ControlCall::Moments(moments) => Some(Command::SetMoments(moments)),
        ControlCall::Back => Some(Command::BackButton),
        ControlCall::Other => None,
    };
    if let Some(command) = command {
        let input = Input::Command { command, now: client.clock(), watched: client.local_place() };
        let (next, act) = EVENTS.with(thread, |events| step(session, input, events));
        SESSION.set(thread, next);
        if let Act::Ask(request) = act {
            client.send_request(request);
        }
    }
    Flow::Exit(ReplayViewerOnHudCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playback(revision: u8, paused: bool, speed: Speed) -> Playback {
        Playback { revision, paused, speed, start: MatchMs(10_000), end: MatchMs(600_000) }
    }

    /// A playback note to a game whose viewer did not play.
    fn played(playback: Playback) -> Input {
        Input::Playback { playback, own: Own(0) }
    }

    fn viewer() -> Session {
        step(Session::Live, played(playback(3, false, Speed::Normal)), &Events::NONE).0
    }

    fn key(session: Session, key: Command, now: u32, events: &Events) -> (Session, Act) {
        step(session, Input::Command { command: key, now: Some(MatchMs(now)), watched: None }, events)
    }

    fn kill(time: u32) -> Option<Event> {
        Some(Event { time: MatchMs(time), kind: EventKind::Kill, actor: Some(1), victim: Some(2) })
    }

    #[test]
    fn the_first_playback_makes_a_viewer_and_plays_it() {
        let (session, act) = step(Session::Live, played(playback(200, true, Speed::Half)), &Events::NONE);
        assert!(matches!(session, Session::Replay(_)));
        assert_eq!(act, Act::Play { paused: true, speed: Speed::Half });
    }

    #[test]
    fn a_live_game_passes_every_key_and_end() {
        for k in [Command::TogglePause, Command::Leave, Command::ToggleUi] {
            assert_eq!(key(Session::Live, k, 0, &Events::NONE), (Session::Live, Act::Pass));
        }
        assert_eq!(step(Session::Live, Input::MatchEnded, &Events::NONE), (Session::Live, Act::Pass));
    }

    #[test]
    fn only_a_newer_playback_is_taken() {
        let session = viewer();
        let (same, act) = step(session, played(playback(2, true, Speed::Double)), &Events::NONE);
        assert_eq!((same, act), (session, Act::Nothing));
        let (_, act) = step(session, played(playback(4, true, Speed::Double)), &Events::NONE);
        assert_eq!(act, Act::Play { paused: true, speed: Speed::Double });
    }

    #[test]
    fn space_asks_the_other_of_what_it_asked_last() {
        let (session, first) = key(viewer(), Command::TogglePause, 0, &Events::NONE);
        assert_eq!(first, Act::Ask(Request { seq: 0, op: Op::Pause(true) }));
        // Before the server's answer, a second press asks to play again.
        let (session, second) = key(session, Command::TogglePause, 0, &Events::NONE);
        assert_eq!(second, Act::Ask(Request { seq: 1, op: Op::Pause(false) }));
        // The server's playback is what the next press starts from.
        let (session, _) = step(session, played(playback(4, true, Speed::Normal)), &Events::NONE);
        assert_eq!(key(session, Command::TogglePause, 0, &Events::NONE).1, Act::Ask(Request { seq: 2, op: Op::Pause(false) }));
    }

    #[test]
    fn arrows_seek_ten_seconds_within_the_replay() {
        assert_eq!(key(viewer(), Command::Forward, 50_000, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(60_000)) }));
        assert_eq!(key(viewer(), Command::Back, 15_000, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(10_000)) }));
        assert_eq!(key(viewer(), Command::Forward, 595_000, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(600_000)) }));
        // No world yet: nothing asked, the key still the viewer's.
        let (_, act) = step(viewer(), Input::Command { command: Command::Back, now: None, watched: None }, &Events::NONE);
        assert_eq!(act, Act::Nothing);
    }

    #[test]
    fn shift_arrows_go_from_moment_to_moment() {
        let mut events = Events::NONE;
        events.slots[0] = kill(30_000);
        events.slots[1] = Some(Event { time: MatchMs(40_000), kind: EventKind::Objective, actor: None, victim: None });
        events.slots[5] = kill(60_000);
        let seek = |ms| Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(ms)) });
        assert_eq!(key(viewer(), Command::NextMoment, 12_000, &events).1, seek(26_000));
        // Just sent to a kill (4 s before it): the next is the one after it.
        assert_eq!(key(viewer(), Command::NextMoment, 26_000, &events).1, seek(36_000));
        assert_eq!(key(viewer(), Command::PreviousMoment, 56_200, &events).1, seek(36_000));
        assert_eq!(key(viewer(), Command::PreviousMoment, 26_000, &events).1, Act::Nothing);
        assert_eq!(key(viewer(), Command::NextMoment, 56_000, &events).1, Act::Nothing);
    }

    #[test]
    fn the_moments_switch_starts_at_mine_for_a_player_and_lights_what_it_says() {
        // Place 2 is the viewer's: it was killed at 30 s by 1, killed 3 at 50 s; 1 killed 3 at 70 s.
        let mut events = Events::NONE;
        events.slots[0] = kill(30_000);
        events.slots[1] = Some(Event { time: MatchMs(50_000), kind: EventKind::Kill, actor: Some(2), victim: Some(3) });
        events.slots[2] = Some(Event { time: MatchMs(70_000), kind: EventKind::Kill, actor: Some(1), victim: Some(3) });
        let own = Own(1 << 2);
        let (player, _) = step(Session::Live, Input::Playback { playback: playback(3, false, Speed::Normal), own }, &events);
        assert!(matches!(player, Session::Replay(Viewer { moments: Moments::Mine, .. })));
        assert!(matches!(viewer(), Session::Replay(Viewer { moments: Moments::All, .. })));
        let step_at = |session, command, now, watched| {
            step(session, Input::Command { command, now: Some(MatchMs(now)), watched }, &events)
        };
        let seek = |seq, ms| Act::Ask(Request { seq, op: Op::Seek(MatchMs(ms)) });
        // Mine: its death and its kill, not 1's kill of 3.
        let (player, act) = step_at(player, Command::NextMoment, 30_000, Some(3));
        assert_eq!(act, seek(0, 46_000));
        assert_eq!(step_at(player, Command::NextMoment, 46_000, Some(3)).1, Act::Nothing);
        // M: Followed (3 in view: both its deaths), then All, then Mine again.
        let (player, _) = step_at(player, Command::NextMoments, 46_000, Some(3));
        assert!(matches!(player, Session::Replay(Viewer { moments: Moments::Followed, .. })));
        assert_eq!(step_at(player, Command::NextMoment, 46_000, Some(3)).1, seek(1, 66_000));
        assert_eq!(step_at(player, Command::NextMoment, 10_000, None).1, Act::Nothing);
        let (all, _) = step_at(player, Command::NextMoments, 0, None);
        assert!(matches!(all, Session::Replay(Viewer { moments: Moments::All, .. })));
        let (mine, _) = step_at(all, Command::NextMoments, 0, None);
        assert!(matches!(mine, Session::Replay(Viewer { moments: Moments::Mine, .. })));
        // One who did not play: All and Followed only, Mine clicked is no choice.
        let (watcher, _) = key(viewer(), Command::NextMoments, 0, &events);
        assert!(matches!(watcher, Session::Replay(Viewer { moments: Moments::Followed, .. })));
        assert!(matches!(key(watcher, Command::NextMoments, 0, &events).0, Session::Replay(Viewer { moments: Moments::All, .. })));
        assert_eq!(key(watcher, Command::SetMoments(Moments::Mine), 0, &events).0, watcher);
        assert!(matches!(key(watcher, Command::SetMoments(Moments::All), 0, &events).0,
                         Session::Replay(Viewer { moments: Moments::All, .. })));
    }

    #[test]
    fn own_places_are_those_of_the_accounts_name() {
        let names = |place: u8| match place {
            0 => Some(&b"Ada"[..]),
            1 => Some(&b"replaytester"[..]),
            3 => Some(&b"ReplayTester"[..]),
            _ => None,
        };
        assert_eq!(Own::by_name(Some(b"ReplayTester"), 4, names), Own(0b1010));
        assert_eq!(Own::by_name(Some(b"ReplayTester"), 2, names), Own(0b10));
        assert_eq!(Own::by_name(Some(b"Bob"), 4, names), Own(0));
        assert_eq!(Own::by_name(Some(b""), 4, |_| Some(&b""[..])), Own(0));
        assert_eq!(Own::by_name(None, 4, names), Own(0));
    }

    #[test]
    fn brackets_step_the_speed_and_stop_at_the_ends() {
        let (session, act) = key(viewer(), Command::Faster, 0, &Events::NONE);
        assert_eq!(act, Act::Ask(Request { seq: 0, op: Op::Speed(Speed::Double) }));
        let (_, act) = key(session, Command::Faster, 0, &Events::NONE);
        assert_eq!(act, Act::Ask(Request { seq: 1, op: Op::Speed(Speed::Quadruple) }));
        let (slow, _) = step(Session::Live, played(playback(0, false, Speed::Quarter)), &Events::NONE);
        assert_eq!(key(slow, Command::Slower, 0, &Events::NONE), (slow, Act::Nothing));
    }

    #[test]
    fn ui_toggles_and_esc_leaves() {
        let (session, act) = key(viewer(), Command::ToggleUi, 0, &Events::NONE);
        assert_eq!(act, Act::ShowUi(false));
        assert_eq!(key(session, Command::ToggleUi, 0, &Events::NONE).1, Act::ShowUi(true));
        assert_eq!(key(viewer(), Command::Leave, 0, &Events::NONE).1, Act::Leave);
        // Back: the server asked to let the game go.
        assert_eq!(key(viewer(), Command::BackButton, 0, &Events::NONE).1, Act::Ask(Request { seq: 0, op: Op::Leave }));
        let (session, act) = key(viewer(), Command::ToggleCursor, 0, &Events::NONE);
        assert!(matches!((session, act), (Session::Replay(Viewer { cursor: Ui::Shown, .. }), Act::Nothing)));
        assert!(matches!(key(session, Command::ToggleCursor, 0, &Events::NONE).0, Session::Replay(Viewer { cursor: Ui::Hidden, .. })));
    }

    #[test]
    fn a_match_end_or_a_new_match_restores_the_clock() {
        for input in [Input::MatchEnded, Input::NewMatch] {
            assert_eq!(step(viewer(), input, &Events::NONE), (Session::Live, Act::Restore));
        }
    }

    #[test]
    fn the_controls_seek_on_the_bar_and_snap_the_speed() {
        assert_eq!(key(viewer(), Command::SeekTo(5_000), 0, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(15_000)) }));
        assert_eq!(key(viewer(), Command::SeekTo(u32::MAX), 0, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Seek(MatchMs(600_000)) }));
        assert_eq!(key(viewer(), Command::SetSpeed(Speed::Normal), 0, &Events::NONE).1, Act::Nothing);
        assert_eq!(key(viewer(), Command::SetSpeed(Speed::Quadruple), 0, &Events::NONE).1,
                   Act::Ask(Request { seq: 0, op: Op::Speed(Speed::Quadruple) }));
    }

    #[test]
    fn markers_are_from_the_start_in_m_ss_and_fit_their_room() {
        let mut events = Events::NONE;
        events.slots[0] = kill(5_000);
        events.slots[1] = kill(75_500);
        events.slots[2] = Some(Event { time: MatchMs(611_000), kind: EventKind::Objective, actor: None, victim: None });
        events.slots[3] = Some(Event { time: MatchMs(90_000), kind: EventKind::Kill, actor: None, victim: Some(1) });
        let names = |place: u8| match place {
            1 => Some(&b"Ada"[..]),
            2 => Some(&b"Bob;"[..]),
            _ => None,
        };
        let mut room = [0; MARKERS_ROOM];
        // Kill 1 > 2, the objective (nobody), a death of 1 no one made.
        assert_eq!(events.markers(MatchMs(10_000), Own(0), None, &mut room).to_bytes(),
                   b"65500:1:0:1:2;601000:2:0::;80000:1:0::1");
        // Watching 2: its death is kind 3, followed; watching 1: the lone death and its kill.
        assert_eq!(events.markers(MatchMs(10_000), Own(0), Some(2), &mut room).to_bytes(),
                   b"65500:3:2:1:2;601000:2:0::;80000:1:0::1");
        assert_eq!(events.markers(MatchMs(10_000), Own(0), Some(1), &mut room).to_bytes(),
                   b"65500:1:2:1:2;601000:2:0::;80000:3:2::1");
        // The viewer was 1, watching 2: its kill is both, its death its own.
        assert_eq!(events.markers(MatchMs(10_000), Own(1 << 1), Some(2), &mut room).to_bytes(),
                   b"65500:3:3:1:2;601000:2:0::;80000:1:1::1");
        assert_eq!(Events::NONE.markers(MatchMs(0), Own(0), None, &mut room).to_bytes(), b"");
        // The names are told apart, a `;` of one replaced, long ones (UTF-8) cut on a character.
        let long = |_: u8| Some("ЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖЖ".as_bytes());
        let mut names_room = [0; NAMES_ROOM];
        assert_eq!(names_text(3, names, &mut names_room).to_bytes(), b"1:Ada;2:Bob,");
        let text = names_text(1, long, &mut names_room).to_bytes();
        let name = &text[2..];
        assert!(name.len() <= NAME_MAX && core::str::from_utf8(name).is_ok(), "{}", name.len());
    }

    #[test]
    fn a_full_list_of_the_longest_markers_fits_whole() {
        let mut many = Events::NONE;
        for slot in many.slots.iter_mut() {
            *slot = Some(Event { time: MatchMs(u32::MAX), kind: EventKind::Kill, actor: Some(255), victim: Some(255) });
        }
        let mut room = [0; MARKERS_ROOM];
        let text = many.markers(MatchMs(0), Own(u32::MAX), Some(255), &mut room).to_bytes();
        assert_eq!(text.split(|&b| b == b';').count(), MAX_EVENTS);
        assert!(text.len() < MARKERS_ROOM, "{}", text.len());
        // And the last one is whole.
        assert!(text.ends_with(b"4294967295:3:2:255:255"), "{}", core::str::from_utf8(&text[text.len() - 30..]).unwrap());
    }

    #[test]
    fn events_past_the_cap_are_told_once() {
        let kill = Event { time: MatchMs(1), kind: EventKind::Kill, actor: None, victim: None };
        let mut events = Events::NONE;
        let mut take = |first, total| {
            let mut bytes = [0u8; 13];
            let len = EventsNote::encode(first, total, &[kill], &mut bytes).unwrap();
            let Some(ServerNote::Events(note)) = ServerNote::parse(&bytes[..len]) else { panic!() };
            events.take(&note)
        };
        assert_eq!(take(5, 2000), Ok(()));
        assert_eq!(take(MAX_EVENTS as u16, 2000), Err(TooManyEvents { total: 2000 }));
        assert_eq!(take(MAX_EVENTS as u16 + 1, 2000), Ok(()), "told once a match");
        assert_eq!(events.capacity, Capacity::Exceeded);
    }

    #[test]
    fn a_place_s_note_replaces_its_mask() {
        let masks = KitMasks::NONE.take(3, 0b001).take(4, 0b010);
        assert_eq!((masks.of(3), masks.of(4), masks.version), (0b001, 0b010, 2));
        let masks = masks.take(3, 0b011);
        assert_eq!((masks.of(3), masks.of(4), masks.version), (0b011, 0b010, 3));
        assert_eq!(masks.of(200), 0);
    }

    fn kit(number: u8, name: &'static [u8], items: &[(u16, u8, u16)]) -> KitData<'static> {
        let mut room = [None; crate::vostok::replay_view::KIT_SLOTS];
        for (at, item) in items.iter().enumerate() {
            room[at] = Some(*item);
        }
        KitData { number, state: 0, level: 7, name, items: room }
    }

    #[test]
    fn the_chooser_text_has_only_the_kits_given() {
        let mut room = [0; CHOOSER_ROOM];
        let kits = [kit(0, b"Alpha", &[(7, 0, 1), (8, 3, 2)]), kit(2, b"", &[(9, 1, 1)])];
        let text = chooser_text(Some(kits.into_iter()), &mut room);
        assert_eq!(text.to_bytes(), b"0|0|7|Alpha|7:0:1,8:3:2;2|0|7||9:1:1;");
        let mut room = [0; CHOOSER_ROOM];
        assert_eq!(chooser_text(Some(core::iter::empty::<KitData<'static>>()), &mut room).to_bytes(), b"");
        let mut room = [0; CHOOSER_ROOM];
        assert_eq!(chooser_text(None::<core::iter::Empty<KitData<'static>>>, &mut room).to_bytes(), b"");
    }

    #[test]
    fn a_kit_name_cannot_break_the_text() {
        let mut room = [0; CHOOSER_ROOM];
        let kits = [kit(1, b"a|b;c,d:e", &[])];
        assert_eq!(chooser_text(Some(kits.into_iter()), &mut room).to_bytes(), b"1|0|7|a b c d e|;");
    }

    #[test]
    fn keys_map_with_shift() {
        assert_eq!(Command::of(Dik::LEFT, true), Some(Command::PreviousMoment));
        assert_eq!(Command::of(Dik::M, false), Some(Command::NextMoments));
        assert_eq!(Command::of(Dik::LEFT, false), Some(Command::Back));
        assert_eq!(Command::of(Dik::H, true), Some(Command::ToggleUi));
        assert_eq!(Command::of(Dik::TAB, false), None);
        assert_eq!(Command::of(Dik::RIGHT_CONTROL, false), Some(Command::ToggleCursor));
    }
}
