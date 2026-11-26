//! Patch 210: the sync log (docs/notes/sync-log.md). With SKHID_SYNC_LOG=on in the game's
//! environment (read once, at the first hook that asks), the run's log (`srp_patches.<run>.log`)
//! gets a line for what can tick a player at a time off the 50 ms grid, the stamina's last
//! difference between games in the desyncs of 2026-11-25 (sync-log.md):
//!
//! - an input applied at an off-grid time (apply_new_input's entry, 0x2D86E0): the place, the
//!   time, the player's last tick time and who called (player::set_input's own return);
//! - an animation tick (base_player::tick_animations 0x2D8EE0, which ticks the stamina at
//!   0x2D8FC2) that apply_new_input makes at an off-grid time, or that a step makes from an
//!   off-grid time (its previous tick off the grid: a step that went on from a part);
//! - each world the server sends that the game loads (a join or a resync), with its time.
//! - in Freeze Tag, once a second while the rule shows an enemy of the viewer on the map (his team's
//!   last one fighting), whether update_minimap_players tested and listed him (`minimap.enemy_spotted`
//!   first, `sync.minimap_listed` 0x23951B) and how far he is.
//!
//! Each line says whether its time is played for the first time (`live`) or again (`replay`: at
//! or before the core's newest time played, after which load). A step to a frame's time ahead
//! of the newest state (the frame's part, put back by the next frame's load) is not listed; a
//! step that goes on from one is. At most [`PER_SECOND`] lines a second of game time, and
//! [`PER_RUN`] in all; what is held back is counted in a line. It only reads: nothing the game
//! computes changes, whether it is on or off, and off it is one test of a cell.

#![forbid(unsafe_code)]

use crate::vostok::log::{line, Piece, Piece::*};
use crate::vostok::memory_profile::environment;
use crate::vostok::desync::{self, Input, InputCaller, Load, LoadKind, MapView, Newest, Return, Tick, TickCaller};
use srp_validate::{Place, Team};
use crate::vostok::{Context, Flow, GameCell, NoExit, SyncLogOnInputExit, SyncLogOnMinimapTestExit, Thread};

/// The simulation's tick, ms (`core_world.tick_ms`).
const TICK_MS: u32 = 50;
/// Lines a second of game time at most.
const PER_SECOND: u16 = 20;
/// Lines a run at most.
const PER_RUN: u32 = 50_000;

const VARIABLE: &[u8] = b"SKHID_SYNC_LOG\0";

fn off_grid(time: u32) -> bool {
    !time.is_multiple_of(TICK_MS)
}

/// How many lines may go out: a window of one second of game time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Budget {
    /// The window's second (game time / 1000).
    second: u32,
    written: u16,
    held: u32,
    total: u32,
}

/// What [`Budget::admit`] says of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admit {
    Write,
    /// The run's last line: the limit's.
    Last,
    Hold,
}

/// A window left with lines held back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    second: u32,
    lines: u32,
}

impl Budget {
    const NEW: Budget = Budget { second: 0, written: 0, held: 0, total: 0 };

    /// A line at `time`: whether it goes out, and the window it leaves if it held some back.
    fn admit(self, time: u32) -> (Budget, Admit, Option<Held>) {
        let second = time / 1000;
        let (mut next, left) = if second == self.second {
            (self, None)
        } else {
            let left = (self.held > 0).then_some(Held { second: self.second, lines: self.held });
            (Budget { second, written: 0, held: 0, total: self.total }, left)
        };
        let admit = if next.total >= PER_RUN {
            Admit::Hold
        } else if next.written >= PER_SECOND {
            next.held += 1;
            Admit::Hold
        } else {
            next.written += 1;
            next.total += 1;
            if next.total == PER_RUN { Admit::Last } else { Admit::Write }
        };
        (next, admit, left)
    }
}

/// Played for the first time, or again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Live,
    Replay { newest: Newest, after: Option<Load> },
}

fn phase(time: u32, newest: Option<Newest>, last: Option<Load>) -> Phase {
    match newest {
        Some(newest) if time <= newest.0 => Phase::Replay { newest, after: last },
        _ => Phase::Live,
    }
}

/// What the log does with an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Input(Input),
    Tick(Tick),
    ServerWorld(u32),
}

impl Event {
    fn time(self) -> u32 {
        match self {
            Event::Input(input) => input.time,
            Event::Tick(tick) => tick.time,
            Event::ServerWorld(time) => time,
        }
    }
}

/// Whether a tick is listed: one an input makes off the grid, one a step makes from an off-grid
/// time, any other caller's off the grid; not a step's to a frame's part (off-grid from on-grid).
fn listed(tick: &Tick) -> bool {
    match tick.caller {
        TickCaller::Step => off_grid(tick.previous),
        TickCaller::Input => off_grid(tick.time),
        TickCaller::Other(_) => off_grid(tick.time) || off_grid(tick.previous),
    }
}

/// The log's state: not read yet, off, or on with its budget and the last load seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Log {
    Unread,
    Off,
    On { budget: Budget, last: Option<Load> },
}

static LOG: GameCell<Log> = GameCell::new(Log::Unread);

fn parse(value: Option<&[u8]>) -> Log {
    match value {
        Some(value) if value.eq_ignore_ascii_case(b"on") || value == b"1" => {
            Log::On { budget: Budget::NEW, last: None }
        }
        _ => Log::Off,
    }
}

fn state(thread: &Thread) -> Log {
    match LOG.get(thread) {
        Log::Unread => {
            let mut value = [0u8; 8];
            let read = parse(environment(thread, VARIABLE, &mut value));
            LOG.set(thread, read);
            if let Log::On { .. } = read {
                line(&[Text("sync log: on (SKHID_SYNC_LOG)")]);
            }
            read
        }
        known => known,
    }
}

/// One event through the log: the lines it writes, the state after.
fn take(log: Log, event: Event, mut write: impl FnMut(&[Piece<'_>])) -> Log {
    let Log::On { budget, last } = log else { return log };
    let (budget, admit, left) = budget.admit(event.time());
    if let Some(held) = left {
        write(&[Text("sync: "), Dec(held.lines), Text(" lines held back in second "), Dec(held.second)]);
    }
    if admit != Admit::Hold {
        match event {
            Event::Input(input) => write_input(&input, phase(input.time, input.newest, last), &mut write),
            Event::Tick(tick) => write_tick(&tick, phase(tick.time, tick.newest, last), &mut write),
            Event::ServerWorld(time) => {
                write(&[Text("sync: the server's world loaded (a join or a resync), its time "), Dec(time)])
            }
        }
    }
    if admit == Admit::Last {
        write(&[Text("sync log: "), Dec(PER_RUN), Text(" lines, no more this run")]);
    }
    Log::On { budget, last }
}

fn phase_pieces(phase: Phase, write: &mut impl FnMut(&[Piece<'_>]), head: &[Piece<'_>]) {
    let mut out: [Piece<'_>; 24] = [Text(""); 24];
    let mut n = 0;
    for piece in head.iter().copied().chain(match phase {
        Phase::Live => [Text(" live"), Text(""), Text(""), Text(""), Text(""), Text("")],
        Phase::Replay { newest, after: None } => {
            [Text(" replay (newest "), Dec(newest.0), Text(")"), Text(""), Text(""), Text("")]
        }
        Phase::Replay { newest, after: Some(load) } => [
            Text(" replay (newest "),
            Dec(newest.0),
            Text(") after a load of "),
            Dec(load.time),
            Text(" by "),
            load_name(load.kind),
        ],
    }) {
        if n < out.len() {
            out[n] = piece;
            n += 1;
        }
    }
    write(&out[..n]);
}

fn load_name(kind: LoadKind) -> Piece<'static> {
    match kind {
        LoadKind::Rollback => Text("rollback"),
        LoadKind::Frame => Text("frame"),
        LoadKind::Server => Text("server"),
        LoadKind::History => Text("load_history_state"),
        LoadKind::Other(Return(at)) => Hex(at),
    }
}

fn write_input(input: &Input, phase: Phase, write: &mut impl FnMut(&[Piece<'_>])) {
    let (via, from) = match input.caller {
        InputCaller::SetInput(Return(at)) => (Text(" via set_input from "), Hex(at)),
        InputCaller::Other(Return(at)) => (Text(" from "), Hex(at)),
    };
    let head = [
        Text("sync: input p"),
        Dec(u32::from(input.place)),
        Text(" t"),
        Dec(input.time),
        Text(" prev "),
        Dec(input.previous),
        via,
        from,
    ];
    phase_pieces(phase, write, &head);
}

fn write_tick(tick: &Tick, phase: Phase, write: &mut impl FnMut(&[Piece<'_>])) {
    let by = match tick.caller {
        TickCaller::Step => Text("step"),
        TickCaller::Input => Text("input"),
        TickCaller::Other(Return(at)) => Hex(at),
    };
    let head = [
        Text("sync: tick p"),
        Dec(u32::from(tick.place)),
        Text(" t"),
        Dec(tick.time),
        Text(" prev "),
        Dec(tick.previous),
        Text(" by "),
        by,
    ];
    phase_pieces(phase, write, &head);
}

fn record(thread: &Thread, event: Event) {
    let log = state(thread);
    if let Log::On { .. } = log {
        LOG.set(thread, take(log, event, line));
    }
}

/// apply_new_input's entry (`carry.input`, before Freeze Tag's and patch 78's handlers): an input
/// applied off the grid.
pub fn on_input(ctx: &mut Context<'_>) -> Flow<SyncLogOnInputExit> {
    if let Log::On { .. } = state(ctx.thread())
        && let Some(input) = Input::at(ctx)
        && off_grid(input.time)
    {
        record(ctx.thread(), Event::Input(input));
    }
    Flow::Continue
}

/// tick_animations' entry (`sync.animations`).
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Log::On { .. } = state(ctx.thread())
        && let Some(tick) = Tick::at(ctx)
        && listed(&tick)
    {
        record(ctx.thread(), Event::Tick(tick));
    }
    Flow::Continue
}

/// game_world_core::deserialize's entry (`sync.world_load`): the load each later line is after;
/// the server's world, a line.
pub fn on_world_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Log::On { budget, .. } = state(thread)
        && let Some(load) = Load::at(ctx)
    {
        LOG.set(thread, Log::On { budget, last: Some(load) });
        if load.kind == LoadKind::Server {
            record(thread, Event::ServerWorld(load.time));
        }
    }
    Flow::Continue
}

// ---- the last one fighting on the map (Freeze Tag) --------------------------------------------

/// A second of frames while the rule shows an enemy of the viewer on the map: how often the
/// minimap ran (listed anyone), tested each place as an enemy (and found him spotted natively),
/// and listed each place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tally {
    second: u32,
    frames: u16,
    ran: u16,
    tested: [u16; PLAYERS],
    spotted: [u16; PLAYERS],
    listed: [u16; PLAYERS],
}

/// The map log: nothing being counted, or a second's tally; the listings since the last frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MapLog {
    Idle { listings: u16 },
    Counting { tally: Tally, listings: u16 },
}

static MAP: GameCell<MapLog> = GameCell::new(MapLog::Idle { listings: 0 });

const PLAYERS: usize = srp_validate::MAX_PLAYERS;

impl MapLog {
    fn listings(self) -> u16 {
        match self {
            MapLog::Idle { listings } | MapLog::Counting { listings, .. } => listings,
        }
    }

    fn visit(self, edit: impl FnOnce(&mut Tally), listing: bool) -> MapLog {
        let listings = self.listings().saturating_add(u16::from(listing));
        match self {
            MapLog::Idle { .. } => MapLog::Idle { listings },
            MapLog::Counting { mut tally, .. } => {
                edit(&mut tally);
                MapLog::Counting { tally, listings }
            }
        }
    }
}

/// The enemies of the viewer the rule shows on the map, with their teams.
fn enemies(fighters: &[Option<Team>; PLAYERS], viewer: Option<Team>) -> [Option<Team>; PLAYERS] {
    fighters.map(|team| team.filter(|team| viewer.is_some_and(|viewer| viewer != *team)))
}

/// A frame (`game_world.after_move`) at `time`, `shown` the enemies the rule shows: the map log's
/// next state and the second it closes, if any.
fn frame(log: MapLog, time: u32, shown: bool) -> (MapLog, Option<Tally>) {
    let second = time / 1000;
    let ran = u16::from(log.listings() > 0);
    match (log, shown) {
        (_, false) => (MapLog::Idle { listings: 0 }, None),
        (MapLog::Counting { tally, .. }, true) if tally.second != second => {
            let next = Tally { second, frames: 1, ran, tested: [0; PLAYERS], spotted: [0; PLAYERS], listed: [0; PLAYERS] };
            (MapLog::Counting { tally: next, listings: 0 }, Some(tally))
        }
        (MapLog::Counting { mut tally, .. }, true) => {
            tally.frames = tally.frames.saturating_add(1);
            tally.ran = tally.ran.saturating_add(ran);
            (MapLog::Counting { tally, listings: 0 }, None)
        }
        (MapLog::Idle { .. }, true) => {
            let tally = Tally { second, frames: 1, ran, tested: [0; PLAYERS], spotted: [0; PLAYERS], listed: [0; PLAYERS] };
            (MapLog::Counting { tally, listings: 0 }, None)
        }
    }
}

/// Decimetres between two points.
fn decimetres(a: [f32; 3], b: [f32; 3]) -> u32 {
    let metres = ragdoll::math::Vec3::new(a[0] - b[0], a[1] - b[1], a[2] - b[2]).length();
    if metres.is_finite() { (metres * 10.0 + 0.5) as u32 } else { u32::MAX }
}

fn write_map(
    tally: &Tally,
    place: Place,
    team: Team,
    viewer: Place,
    distance: Option<u32>,
    write: &mut impl FnMut(&[Piece<'_>]),
) {
    let i = place.index();
    let (whole, tenth, unit) = match distance {
        Some(dm) if dm != u32::MAX => (Dec(dm / 10), Dec(dm % 10), Text(" m")),
        _ => (Text("?"), Text(""), Text("")),
    };
    write(&[
        Text("sync: map, second "),
        Dec(tally.second),
        Text(": p"),
        Dec(place.index() as u32),
        Text(" (team "),
        Dec(team.index() as u32),
        Text(") his team's last one fighting, the rule shows him; viewer p"),
        Dec(viewer.index() as u32),
        Text(" at "),
        whole,
        Text("."),
        tenth,
        unit,
        Text("; frames "),
        Dec(u32::from(tally.frames)),
        Text(", minimap ran "),
        Dec(u32::from(tally.ran)),
        Text(", tested him "),
        Dec(u32::from(tally.tested[i])),
        Text(" (spotted by the game "),
        Dec(u32::from(tally.spotted[i])),
        Text("), listed him "),
        Dec(u32::from(tally.listed[i])),
    ]);
}

/// update_minimap_players' test of an enemy (`minimap.enemy_spotted`, before every patch's verdict).
pub fn on_minimap_test(ctx: &mut Context<'_>) -> Flow<SyncLogOnMinimapTestExit> {
    let thread = ctx.thread();
    if let Log::On { .. } = state(thread)
        && let Some((place, spotted)) = desync::minimap_test(ctx)
        && let Some(i) = Place::new(place).map(Place::index)
    {
        MAP.set(thread, MAP.get(thread).visit(|tally| {
            tally.tested[i] = tally.tested[i].saturating_add(1);
            tally.spotted[i] = tally.spotted[i].saturating_add(u16::from(spotted > 0));
        }, false));
    }
    Flow::Continue
}

/// update_minimap_players listing a player (`sync.minimap_listed`, 0x23951B).
pub fn on_minimap_listed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Log::On { .. } = state(thread)
        && let Some(i) = desync::minimap_listed(ctx).and_then(Place::new).map(Place::index)
    {
        MAP.set(thread, MAP.get(thread).visit(|tally| tally.listed[i] = tally.listed[i].saturating_add(1), true));
    }
    Flow::Continue
}

/// Each frame after the simulation's move: a second's line for each enemy of the viewer the rule
/// shows on the map.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Log::On { .. } = state(thread) else { return Flow::Continue };
    let Some(world) = crate::vostok::world::GameWorld::at_after_move(ctx) else { return Flow::Continue };
    let Some(view) = MapView::of(&world) else {
        MAP.set(thread, MapLog::Idle { listings: 0 });
        return Flow::Continue;
    };
    let shown = crate::freeze_tag::last_fighters(thread)
        .map(|fighters| enemies(&fighters, crate::freeze_tag::team_of(thread, view.viewer)))
        .unwrap_or([None; PLAYERS]);
    let (next, closed) = frame(MAP.get(thread), view.time, shown.iter().any(Option::is_some));
    MAP.set(thread, next);
    if let Some(tally) = closed {
        for (place, team) in Place::all().zip(shown).filter_map(|(place, team)| Some((place, team?))) {
            let distance = view.feet[place.index()].zip(view.feet[view.viewer.index()]).map(|(a, b)| decimetres(a, b));
            write_map(&tally, place, team, view.viewer, distance, &mut |pieces| line(pieces));
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(pieces: &[Piece<'_>]) -> String {
        pieces
            .iter()
            .map(|piece| match piece {
                Text(t) => (*t).to_string(),
                Dec(n) => n.to_string(),
                Hex(n) => format!("0x{n:08x}"),
            })
            .collect()
    }

    fn run(log: Log, events: &[Event]) -> (Log, Vec<String>) {
        let mut lines = Vec::new();
        let log = events.iter().fold(log, |log, event| take(log, *event, |p| lines.push(text(p))));
        (log, lines)
    }

    fn on() -> Log {
        Log::On { budget: Budget::NEW, last: None }
    }

    fn input(time: u32, newest: u32) -> Event {
        Event::Input(Input {
            place: 2,
            time,
            previous: time - time % TICK_MS,
            caller: InputCaller::SetInput(Return(0x2D1F2A)),
            newest: Some(Newest(newest)),
        })
    }

    #[test]
    fn the_switch() {
        assert_eq!(parse(Some(b"on")), on());
        assert_eq!(parse(Some(b"ON")), on());
        assert_eq!(parse(Some(b"1")), on());
        for off in [None, Some(&b"off"[..]), Some(b"0"), Some(b"yes")] {
            assert_eq!(parse(off), Log::Off);
        }
    }

    #[test]
    fn off_writes_nothing() {
        let (log, lines) = run(Log::Off, &[input(334697, 334650)]);
        assert_eq!(log, Log::Off);
        assert!(lines.is_empty());
    }

    #[test]
    fn an_input_line_says_who_and_when() {
        let (_, lines) = run(on(), &[input(334697, 334650), input(334697, 334800)]);
        assert_eq!(lines, [
            "sync: input p2 t334697 prev 334650 via set_input from 0x002d1f2a live",
            "sync: input p2 t334697 prev 334650 via set_input from 0x002d1f2a replay (newest 334800)",
        ]);
    }

    #[test]
    fn which_ticks_are_listed() {
        let tick = |time, previous, caller| Tick { place: 5, time, previous, caller, newest: None };
        assert!(listed(&tick(334697, 334650, TickCaller::Input)));
        assert!(!listed(&tick(334700, 334650, TickCaller::Input)));
        // A step to a frame's part is not listed; a step from one is.
        assert!(!listed(&tick(334697, 334650, TickCaller::Step)));
        assert!(listed(&tick(334700, 334697, TickCaller::Step)));
        assert!(!listed(&tick(334700, 334650, TickCaller::Step)));
        assert!(listed(&tick(334700, 334697, TickCaller::Other(Return(1)))));
    }

    #[test]
    fn a_replay_names_the_load_before_it() {
        let (log, _) = run(on(), &[]);
        let Log::On { budget, .. } = log else { panic!() };
        let log = Log::On { budget, last: Some(Load { kind: LoadKind::Rollback, time: 334600 }) };
        let tick = Event::Tick(Tick {
            place: 1,
            time: 334700,
            previous: 334697,
            caller: TickCaller::Step,
            newest: Some(Newest(334750)),
        });
        let (_, lines) = run(log, &[tick, Event::ServerWorld(92250)]);
        assert_eq!(lines, [
            "sync: tick p1 t334700 prev 334697 by step replay (newest 334750) after a load of 334600 by rollback",
            "sync: the server's world loaded (a join or a resync), its time 92250",
        ]);
    }

    #[test]
    fn a_second_holds_twenty_and_says_what_it_held() {
        let events: Vec<Event> = (0..25).map(|i| input(10_001 + i, 0)).chain([input(11_003, 0)]).collect();
        let (_, lines) = run(on(), &events);
        assert_eq!(lines.len(), 20 + 1 + 1);
        assert_eq!(lines[20], "sync: 5 lines held back in second 10");
        assert!(lines[21].starts_with("sync: input p2 t11003 "));
    }

    fn place(n: u8) -> Place {
        Place::new(n).unwrap()
    }

    #[test]
    fn the_map_counts_a_second_and_closes_it() {
        let idle = MapLog::Idle { listings: 0 };
        assert_eq!(frame(idle, 90_500, false), (idle, None));
        let (log, closed) = frame(idle, 90_500, true);
        assert!(closed.is_none());
        // A frame of the minimap: it tests p7 (not spotted) and lists p3 (a teammate).
        let log = log.visit(|t| t.tested[7] += 1, false).visit(|t| t.listed[3] += 1, true);
        let (log, closed) = frame(log, 90_550, true);
        assert!(closed.is_none());
        let (_, closed) = frame(log, 91_000, true);
        let tally = closed.expect("the second 90 closed");
        assert_eq!((tally.second, tally.frames, tally.ran, tally.tested[7], tally.listed[7], tally.listed[3]), (90, 2, 1, 1, 0, 1));
        let mut lines = Vec::new();
        write_map(&tally, place(7), Team::Second, place(3), Some(234), &mut |p| lines.push(text(p)));
        assert_eq!(lines, ["sync: map, second 90: p7 (team 1) his team's last one fighting, the rule shows him; viewer p3 at 23.4 m; frames 2, minimap ran 1, tested him 1 (spotted by the game 0), listed him 0"]);
        // No enemy shown: idle, nothing closed.
        assert_eq!(frame(log, 91_050, false), (idle, None));
    }

    #[test]
    fn only_the_viewers_enemies() {
        let mut fighters = [None; PLAYERS];
        fighters[7] = Some(Team::Second);
        fighters[2] = Some(Team::First);
        let shown = enemies(&fighters, Some(Team::First));
        assert_eq!((shown[7], shown[2]), (Some(Team::Second), None));
        assert!(enemies(&fighters, None).iter().all(Option::is_none));
        assert_eq!(decimetres([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]), 50);
    }

    #[test]
    fn a_run_stops_at_its_limit() {
        let mut budget = Budget::NEW;
        let mut written = 0;
        let mut last = 0;
        for i in 0..(PER_RUN + 500) {
            let (next, admit, _) = budget.admit(i * 1000);
            budget = next;
            match admit {
                Admit::Write => written += 1,
                Admit::Last => last += 1,
                Admit::Hold => {}
            }
        }
        assert_eq!((written + last, last), (PER_RUN, 1));
    }
}
