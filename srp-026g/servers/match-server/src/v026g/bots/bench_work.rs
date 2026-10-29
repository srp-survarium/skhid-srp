//! The bench's deterministic work counters, by team (TEST ONLY: this module exists in test builds): how much a team's bots made the
//! match-server do, counted by the thing done and not by the time it took, so that two arms of one bench compare on any machine
//! (`traces-20261028/BENCH-DECISIONS.md` §1b, §2F). Counts only; the owner tally prints them (`tallies/owner/fairness.rs`).
//!
//! A poll says whose work it is doing ([`set`]): a bot's think and ticks are its team's, the commander's and the shared search
//! slices are [`Who::Shared`], and outside a poll nothing is counted (the tally's own rays are not the bots').

use std::cell::Cell;

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
