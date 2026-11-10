//! Patch 126: the game tells the match server how much memory it has left
//! (`docs/notes/memory-report.md`): each arena the engine registered, the
//! address space and the video budget (`vostok::memory`), when a match
//! connection's first world is in (the load's bakes are its peak) and when
//! the match closes on the game with its world still there (the match's).
//! The same snapshot goes to `srp_patches.log`, so a crash report carries
//! the last one. The server only logs it.
#![forbid(unsafe_code)]
use crate::vostok::log::{line, line_of, Piece::*};
use crate::vostok::memory::snapshot;
use crate::vostok::packet_recovery::Connection;
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread};
use srp_validate::memory::{Entry, Moment, Usage};
use srp_validate::report::Report;

/// A match connection: its client and match seed.
type Key = (usize, u32);

/// What the game has reported, of which connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reported {
    Nothing,
    /// Its first world's snapshot.
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

fn report(thread: &Thread, connection: &Connection<'_>, event: Event) {
    let (next, moment) = step(REPORTED.get(thread), event);
    REPORTED.set(thread, next);
    let Some(moment) = moment else { return };
    let report = Report::Memory(snapshot(thread, moment));
    let told = connection.tell(&report);
    let Report::Memory(memory) = &report else { return };
    for entry in memory.entries() {
        match *entry {
            Entry::Process(process) => line(&[
                Text("memory "),
                Text(moment.word()),
                Text(", match "),
                Hex(connection.match_id()),
                Text(": address space free "),
                Dec(process.free),
                Text(", largest free "),
                Dec(process.largest_free),
                Text("; video budget "),
                Dec((process.video_budget >> 20) as u32),
                Text(if told { " MiB; told the server" } else { " MiB; not told (no link or no room)" }),
            ]),
            Entry::Arena(arena) => {
                let head = [Text("memory \""), Text(arena.name.as_str()), Text("\": size "), Dec(arena.size)];
                match arena.usage {
                    Usage::Unknown => line_of(&[&head, &[Text(", use unknown")]]),
                    Usage::Counted { used } => line_of(&[&head, &[Text(", counted "), Dec(used)]]),
                    Usage::Heap { used, largest_free } => {
                        line_of(&[&head, &[Text(", used "), Dec(used), Text(", largest free "), Dec(largest_free)]])
                    }
                }
            }
        }
    }
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
            Event::Closing(a),     // a handshake that never had a world
            Event::WorldLoaded(a), // its first world
            Event::WorldLoaded(a), // a resync's
            Event::Closing(b),     // another connection's close
            Event::Closing(a),     // its end
            Event::Closing(a),     // the close again (the link's own after 0xEA)
            Event::WorldLoaded(a), // nothing more of it
            Event::WorldLoaded(b), // the next match, a late join's first world
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
