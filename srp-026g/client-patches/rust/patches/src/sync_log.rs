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
use crate::vostok::desync::{Input, InputCaller, Load, LoadKind, Newest, Return, Tick, TickCaller};
use crate::vostok::{Context, Flow, GameCell, NoExit, SyncLogOnInputExit, Thread};

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
            "sync: input p2 t334697 prev 334650 via set_input from 0x002D1F2A replay (newest 334800)",
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
