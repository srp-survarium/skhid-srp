//! The bench's deterministic work counters, by team (TEST ONLY: this module exists in test builds): how much a team's bots made the
//! match-server do, counted by the thing done and not by the time it took, so that two arms of one bench compare on any machine
//! (`traces-20261028/BENCH-DECISIONS.md` §1b, §2F). Counts only; the owner tally prints them (`tests/tallies/owner/metrics.rs`).
//!
//! A poll says whose work it is doing ([`set`]): a bot's think and ticks are its team's, the commander's and the shared search
//! slices are [`Who::Shared`], and outside a poll nothing is counted (the tally's own rays are not the bots').

use std::cell::Cell;
use std::cell::RefCell;
use std::time::Instant;

/// Whose work is being done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    /// Outside a poll: not counted.
    Idle,
    /// A poll's work that belongs to no one bot (the commanders, the search slices).
    Shared,
    Team(u8),
}

/// What is counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// Floors a route search expanded.
    Floors,
    /// Sight and aim rays cast.
    Rays,
    /// Spot queries (`Tactics::spots`).
    Spots,
    /// Plays scored (the candidates the plays offered).
    Plays,
}

/// The kinds of work.
pub const KINDS: usize = 4;

/// The counts, by owner (team 0, team 1, shared) and kind.
pub type Totals = [[u64; KINDS]; 3];

thread_local! {
    static WHO: Cell<Who> = const { Cell::new(Who::Idle) };
    static TOTALS: Cell<Totals> = const { Cell::new([[0; KINDS]; 3]) };
}

/// From now on the work is `who`'s.
pub fn set(who: Who) {
    WHO.with(|w| w.set(who));
}

/// `n` of `work`, for whoever is working.
pub fn add(work: Work, n: u64) {
    let row = match WHO.with(Cell::get) {
        Who::Idle => return,
        Who::Shared => 2,
        Who::Team(team) => usize::from(team & 1),
    };
    TOTALS.with(|t| {
        let mut totals = t.get();
        totals[row][work as usize] += n;
        t.set(totals);
    });
}

/// The counts so far on this thread.
pub fn totals() -> Totals {
    TOTALS.with(Cell::get)
}

thread_local! {
    static SPANS_ON: Cell<bool> = const { Cell::new(false) };
    static SPANS: RefCell<Vec<(&'static str, f32)>> = const { RefCell::new(Vec::new()) };
}

/// [A276] A named stretch of a poll's work, timed while spans are on ([`spans_on`]) for the bench's spike lines: its time
/// is kept when it ends ([`Span::end`]). Timing only; nothing the bots do reads it.
pub struct Span {
    name: &'static str,
    started: Option<Instant>,
}

impl Span {
    pub fn new(name: &'static str) -> Self {
        Self { name, started: SPANS_ON.with(Cell::get).then(Instant::now) }
    }

    pub fn end(self) {
        if let Some(started) = self.started {
            let us = started.elapsed().as_secs_f32() * 1e6;
            SPANS.with(|spans| spans.borrow_mut().push((self.name, us)));
        }
    }
}

thread_local! {
    static COUNTS: RefCell<Vec<(&'static str, u64)>> = const { RefCell::new(Vec::new()) };
}

/// [A276] `n` more of a named count, kept beside the spans while they are on (for the bench's spike lines).
pub fn add_span_count(name: &'static str, n: usize) {
    if !SPANS_ON.with(Cell::get) {
        return;
    }
    COUNTS.with(|counts| {
        let mut counts = counts.borrow_mut();
        match counts.iter_mut().find(|row| row.0 == name) {
            Some(row) => row.1 += n as u64,
            None => counts.push((name, n as u64)),
        }
    });
}

/// The named counts since the last take.
pub fn take_counts() -> Vec<(&'static str, u64)> {
    COUNTS.with(|counts| std::mem::take(&mut *counts.borrow_mut()))
}

/// Spans are timed from now on (`true`) or not.
pub fn spans_on(on: bool) {
    SPANS_ON.with(|o| o.set(on));
}

/// The spans timed since the last take, by name: (name, how many, µs in all, the longest µs), the most µs first.
pub fn take_spans() -> Vec<(&'static str, u32, f32, f32)> {
    let spans = SPANS.with(|spans| std::mem::take(&mut *spans.borrow_mut()));
    let mut by_name: Vec<(&'static str, u32, f32, f32)> = Vec::new();
    for (name, us) in spans {
        match by_name.iter_mut().find(|row| row.0 == name) {
            Some(row) => {
                row.1 += 1;
                row.2 += us;
                row.3 = row.3.max(us);
            }
            None => by_name.push((name, 1, us, us)),
        }
    }
    by_name.sort_by(|a, b| b.2.total_cmp(&a.2));
    by_name
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Work is counted for whoever is working, and not at all outside a poll.
    #[test]
    fn work_is_counted_for_whoever_is_working() {
        let before = totals();
        set(Who::Idle);
        add(Work::Rays, 5);
        set(Who::Team(1));
        add(Work::Rays, 3);
        add(Work::Spots, 1);
        set(Who::Shared);
        add(Work::Floors, 7);
        set(Who::Idle);
        let after = totals();
        let delta = |row: usize, work: Work| after[row][work as usize] - before[row][work as usize];
        assert_eq!((delta(1, Work::Rays), delta(1, Work::Spots), delta(2, Work::Floors)), (3, 1, 7));
        assert_eq!((delta(0, Work::Rays), delta(2, Work::Rays)), (0, 0));
    }
}
