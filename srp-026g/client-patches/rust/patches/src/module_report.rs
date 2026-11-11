//! Patch 125: the server hears what runs inside the game besides Windows'
//! own DLLs (`srp_validate::modules`, `docs/notes/module-report.md`): the
//! loader's modules and the executable memory that is no listed module,
//! what a watch kept from the process's first instruction on (the DLLs
//! loaded) and which other processes' always-on-top windows are over it,
//! when a match connection's first world is in and when the match closes
//! on the game. The server only logs it.
#![forbid(unsafe_code)]
use crate::vostok::log::{line, Piece::*};
use crate::vostok::modules::{presenting, snapshot};
use crate::vostok::module_watch;
use crate::vostok::packet_recovery::Connection;
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread};
use srp_validate::modules::Moment;
use srp_validate::report::Report;

/// A match connection: its client and match seed.
type Key = (usize, u32);

/// What the game has reported, of which connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reported {
    Nothing,
    /// Its first world's report.
    Loaded(Key),
    /// Its end's too: nothing more of it.
    Ended(Key),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    WorldLoaded(Key),
    Closing(Key),
}

/// The next state, and the moment to report, if any. A world after a
/// connection's first (a resync's) and a close of a connection that had
/// no world, or closes again, report nothing.
fn step(state: Reported, event: Event) -> (Reported, Option<Moment>) {
    match (state, event) {
        (Reported::Loaded(seen) | Reported::Ended(seen), Event::WorldLoaded(key)) if seen == key => (state, None),
        (_, Event::WorldLoaded(key)) => (Reported::Loaded(key), Some(Moment::Loaded)),
        (Reported::Loaded(seen), Event::Closing(key)) if seen == key => (Reported::Ended(key), Some(Moment::Ended)),
        (_, Event::Closing(_)) => (state, None),
    }
}

static REPORTED: GameCell<Reported> = GameCell::new(Reported::Nothing);

pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(connection) = Connection::at_loaded(ctx) {
        report(ctx.thread(), &connection, Event::WorldLoaded(connection.key()));
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
    let (next, moment) = step(REPORTED.get(thread), event);
    REPORTED.set(thread, next);
    let Some(moment) = moment else { return };
    let Some(modules) = snapshot(thread, moment) else {
        line(&[Text("modules "), Text(moment.word()), Text(": the walks could not run; nothing told")]);
        return;
    };
    let entries = modules.entries().len() as u32;
    let report = Report::Modules(modules);
    let told = connection.tell(&report);
    line(&[
        Text("modules "),
        Text(moment.word()),
        Text(", match "),
        Hex(connection.match_id()),
        Text(": "),
        Dec(entries),
        Text(if told { " entries told the server" } else { " entries not told (no link or no room)" }),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_connection_reports_its_first_world_and_its_end_once_each() {
        let (a, b) = ((0x1000, 7), (0x1000, 8));
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
            let (next, moment) = step(state, event);
            state = next;
            told.extend(moment.map(|moment| (event, moment)));
        }
        assert_eq!(
            told,
            [
                (Event::WorldLoaded(a), Moment::Loaded),
                (Event::Closing(a), Moment::Ended),
                (Event::WorldLoaded(b), Moment::Loaded),
                (Event::Closing(b), Moment::Ended),
            ]
        );
        assert_eq!(state, Reported::Ended(b));
    }
}
