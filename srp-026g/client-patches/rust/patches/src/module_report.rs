//! Patch 125: the server hears what runs inside the game besides Windows'
//! own DLLs (`srp_validate::modules`, `docs/notes/module-report.md`): the
//! loader's modules and the executable memory that is no listed module,
//! what a watch kept from the process's first instruction on (the DLLs
//! loaded) and which other processes' always-on-top windows are over it,
//! when a match connection's first world is in and when the match closes
//! on the game; in between, a look at those windows each minute, told
//! when it saw otherwise than the report before. The server only logs it.
#![forbid(unsafe_code)]
use crate::vostok::files::tick_count;
use crate::vostok::log::{line, Piece::*};
use crate::vostok::modules::{presenting, snapshot, Windows};
use crate::vostok::module_watch;
use crate::vostok::packet_recovery::Connection;
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread};
use srp_validate::modules::{Minute, Moment};
use srp_validate::report::Report;

/// A match connection: its client and match seed.
type Key = (usize, u32);

/// A minute in GetTickCount's milliseconds.
const MINUTE: u32 = 60_000;

/// What the game has reported, of which connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reported {
    Nothing,
    /// Its first world's report; since then, a look at the windows each minute.
    Loaded(Key, Watch),
    /// Its end's too: nothing more of it.
    Ended(Key),
}

/// The looks at the windows over the game's in a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Watch {
    /// When the last look was (GetTickCount's ms).
    at: u32,
    /// The minute of the next look; `None` after the last.
    next: Option<Minute>,
    /// What the last look that ran saw, as told; `None`: none ran, or its
    /// report did not go, so the next look tells what it sees.
    last: Option<Windows>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    WorldLoaded(Key),
    /// A frame of the game's match.
    Frame(Key),
    Closing(Key),
    /// The report just made did not go (its walks could not run, or the
    /// packet pool had no room).
    Lost(Key),
}

/// What to report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tell {
    /// Everything ([`snapshot`]) with what the look saw; `None`: it could not run.
    All(Moment, Option<Windows>),
    /// The windows a minute's look saw, otherwise than the report before.
    Windows(Minute, Windows),
}

/// The next state, and what to report, if anything, at `now` (GetTickCount's
/// ms). `look` looks at the windows; it is called only where a look is due.
/// A world after a connection's first (a resync's) and a close of a
/// connection that had no world, or closes again, report nothing. A minute
/// after the last look, a frame of the connection's match looks again and
/// tells what it saw if it differs from what the last look saw; after a
/// report that did not go (`Lost`) it tells what it sees.
fn step(state: Reported, event: Event, now: u32, look: impl FnOnce() -> Option<Windows>) -> (Reported, Option<Tell>) {
    match (state, event) {
        (Reported::Loaded(seen, _) | Reported::Ended(seen), Event::WorldLoaded(key)) if seen == key => (state, None),
        (_, Event::WorldLoaded(key)) => {
            let last = look();
            (Reported::Loaded(key, Watch { at: now, next: Some(Minute::FIRST), last }), Some(Tell::All(Moment::Loaded, last)))
        }
        (Reported::Loaded(seen, Watch { at, next: Some(minute), last }), Event::Frame(key))
            if seen == key && now.wrapping_sub(at) >= MINUTE =>
        {
            let windows = look();
            let watch = Watch { at: now, next: minute.next(), last: windows.or(last) };
            let tell = windows.filter(|&windows| Some(windows) != last).map(|windows| Tell::Windows(minute, windows));
            (Reported::Loaded(key, watch), tell)
        }
        (_, Event::Frame(_)) => (state, None),
        (Reported::Loaded(seen, _), Event::Closing(key)) if seen == key => {
            (Reported::Ended(key), Some(Tell::All(Moment::Ended, look())))
        }
        (_, Event::Closing(_)) => (state, None),
        (Reported::Loaded(seen, watch), Event::Lost(key)) if seen == key => {
            (Reported::Loaded(key, Watch { last: None, ..watch }), None)
        }
        (_, Event::Lost(_)) => (state, None),
    }
}

static REPORTED: GameCell<Reported> = GameCell::new(Reported::Nothing);

pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(connection) = Connection::at_loaded(ctx) {
        report(ctx.thread(), &connection, Event::WorldLoaded(connection.key()));
    }
    Flow::Continue
}

/// Each frame: a minute's look at the windows, when one is due.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(connection) = Connection::at_game_tick(ctx) {
        report(ctx.thread(), &connection, Event::Frame(connection.key()));
    }
    Flow::Continue
}

pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(connection) = Connection::at_close(ctx) {
        report(ctx.thread(), &connection, Event::Closing(connection.key()));
    }
    Flow::Continue
}

/// The exe's TLS callback: the process starts (the watch does), or a thread.
pub fn on_thread(ctx: &mut Context<'_>) -> Flow<NoExit> {
    module_watch::tls(ctx);
    Flow::Continue
}

/// The loader's notification of a DLL.
pub fn on_dll(ctx: &mut Context<'_>) -> Flow<NoExit> {
    module_watch::dll(ctx);
    Flow::Continue
}

/// Each Present: which window the game presents to.
pub fn on_present(ctx: &mut Context<'_>) -> Flow<NoExit> {
    presenting(ctx);
    Flow::Continue
}

fn report(thread: &Thread, connection: &Connection<'_>, event: Event) {
    let advance = |event| {
        let (next, tell) = step(REPORTED.get(thread), event, tick_count(), Windows::look);
        REPORTED.set(thread, next);
        tell
    };
    let modules = match advance(event) {
        None => return,
        Some(Tell::Windows(minute, windows)) => windows.report(Moment::Minute(minute)),
        Some(Tell::All(moment, windows)) => match windows.and_then(|windows| snapshot(thread, moment, windows)) {
            Some(modules) => modules,
            None => {
                line(&[Text("modules "), Text(moment.word()), Text(": the walks could not run; nothing told")]);
                advance(Event::Lost(connection.key()));
                return;
            }
        },
    };
    let (moment, entries) = (modules.moment(), modules.entries().len() as u32);
    let told = connection.tell(&Report::Modules(modules));
    if !told {
        advance(Event::Lost(connection.key()));
    }
    let told = if told { " entries told the server" } else { " entries not told (no link or no room)" };
    let match_id = connection.match_id();
    match moment {
        Moment::Minute(minute) => line(&[
            Text("modules minute "),
            Dec(u32::from(minute.get())),
            Text(", match "),
            Hex(match_id),
            Text(": "),
            Dec(entries),
            Text(told),
        ]),
        moment => line(&[Text("modules "), Text(moment.word()), Text(", match "), Hex(match_id), Text(": "), Dec(entries), Text(told)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use srp_validate::modules::Unseen;

    #[test]
    fn a_connection_reports_its_first_world_and_its_end_once_each() {
        let (a, b) = ((0x1000, 7), (0x1000, 8));
        let windows = Windows::unseen(Unseen::NoOrder);
        let mut state = Reported::Nothing;
        let mut told = std::vec::Vec::new();
        for event in [
            Event::Closing(a),
            Event::WorldLoaded(a),
            Event::WorldLoaded(a),
            Event::Closing(b),
            Event::Closing(a),
            Event::Closing(a),
            Event::WorldLoaded(a),
            Event::WorldLoaded(b),
            Event::Closing(b),
        ] {
            let (next, tell) = step(state, event, 0, || Some(windows));
            state = next;
            told.extend(tell.map(|tell| (event, tell)));
        }
        assert_eq!(
            told,
            [
                (Event::WorldLoaded(a), Tell::All(Moment::Loaded, Some(windows))),
                (Event::Closing(a), Tell::All(Moment::Ended, Some(windows))),
                (Event::WorldLoaded(b), Tell::All(Moment::Loaded, Some(windows))),
                (Event::Closing(b), Tell::All(Moment::Ended, Some(windows))),
            ]
        );
        assert_eq!(state, Reported::Ended(b));
    }

    #[test]
    fn a_minute_looks_once_and_tells_only_a_change() {
        let (a, b) = ((0x1000, 7), (0x1000, 8));
        let (none, over) = (Windows::unseen(Unseen::NoWindow), Windows::unseen(Unseen::NoOrder));
        let mut state = step(Reported::Nothing, Event::WorldLoaded(a), u32::MAX - 1000, || Some(none)).0;
        let looks = core::cell::Cell::new(0);
        let frame = |state: &mut Reported, key, now, seen: Option<Windows>| {
            let (next, tell) = step(*state, Event::Frame(key), now, || {
                looks.set(looks.get() + 1);
                seen
            });
            *state = next;
            tell
        };
        // before the minute (the clock wraps on the way): no look
        assert_eq!(frame(&mut state, a, 58_000, Some(over)), None);
        // another connection's frames: no look
        assert_eq!(frame(&mut state, b, 59_000, Some(over)), None);
        // minute 1 saw the same as the load: looked, nothing told
        assert_eq!(frame(&mut state, a, 59_000, Some(none)), None);
        // minute 2: something over the game
        assert_eq!(frame(&mut state, a, 119_000, Some(over)), Some(Tell::Windows(Minute::FIRST.next().unwrap(), over)));
        // minute 3: a look that could not run keeps what was seen; minute 4 then sees it again
        assert_eq!(frame(&mut state, a, 179_000, None), None);
        assert_eq!(frame(&mut state, a, 239_000, Some(over)), None);
        // minute 5: gone again
        let fifth = Minute::FIRST.next().and_then(Minute::next).and_then(Minute::next).and_then(Minute::next).unwrap();
        assert_eq!(frame(&mut state, a, 299_000, Some(none)), Some(Tell::Windows(fifth, none)));
        assert_eq!(looks.get(), 5);
        // the end: no more looks
        state = step(state, Event::Closing(a), 300_000, || Some(none)).0;
        assert_eq!(frame(&mut state, a, 999_000, Some(over)), None);
        assert_eq!(looks.get(), 5);
    }

    #[test]
    fn the_last_minute_looks_no_more() {
        let a = (0x1000, 7);
        let windows = Windows::unseen(Unseen::NoOrder);
        let watch = Watch { at: 0, next: Some(Minute::LAST), last: Some(windows) };
        let (state, _) = step(Reported::Loaded(a, watch), Event::Frame(a), MINUTE, || Some(windows));
        assert_eq!(state, Reported::Loaded(a, Watch { at: MINUTE, next: None, last: Some(windows) }));
        assert_eq!(step(state, Event::Frame(a), 9 * MINUTE, || panic!("no look after the last minute")), (state, None));
    }

    #[test]
    fn a_report_that_did_not_go_is_told_at_the_next_minute() {
        let (a, b) = ((0x1000, 7), (0x1000, 8));
        let over = Windows::unseen(Unseen::NoOrder);
        let (state, _) = step(Reported::Nothing, Event::WorldLoaded(a), 0, || Some(over));
        // the load's report did not go: minute 1 tells the same windows
        let (state, _) = step(state, Event::Lost(a), 0, || panic!("no look at a loss"));
        let (state, tell) = step(state, Event::Frame(a), MINUTE, || Some(over));
        assert_eq!(tell, Some(Tell::Windows(Minute::FIRST, over)));
        // told: minute 2 tells nothing; another connection's loss changes nothing
        assert_eq!(step(state, Event::Lost(b), MINUTE, || None), (state, None));
        let (state, tell) = step(state, Event::Frame(a), 2 * MINUTE, || Some(over));
        assert_eq!(tell, None);
        // a loss after the end changes nothing
        let (ended, _) = step(state, Event::Closing(a), 2 * MINUTE, || Some(over));
        assert_eq!(step(ended, Event::Lost(a), 2 * MINUTE, || None), (ended, None));
    }
}
