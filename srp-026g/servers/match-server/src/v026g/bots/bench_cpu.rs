//! [CPU] The bench's split of a poll on the thread's CPU (TEST ONLY: this module exists in test builds). A part here is timed where
//! it runs (a think that is due, the route-search queue's slices, a bot tick's drive, its lip guard, its reckoning input), with the
//! named costs inside it (grid searches, pulls, the kept controllers' steps: [`named_ns`]) told apart, so the bench's "rest" of the
//! ticks is told by part. A part is read only where its work runs, so the clock's reads (a system call each) stay few. Timing only:
//! the steps and their order are the same with it.

use std::cell::Cell;
use std::time::Duration;

/// A part of a poll timed on its own ([`Span::end`], [`Guard`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// A bot's think that is due (`Bots::think` past its schedule).
    Think,
    /// The route-search queue's slices between the bots (`Bots::go_on_searching` in `each_bot`).
    Queue,
    /// A bot tick's `drive_tick` whole (its lip guard in it too; the bench's line takes it out).
    Drive,
    /// The lip guard in the drive (`guard_keys`).
    LipGuard,
    /// A bot tick's reckoning input (`tracker.input`).
    Input,
}

pub(crate) const PARTS: usize = 5;

thread_local! {
    static TOTAL_NS: Cell<[u64; PARTS]> = const { Cell::new([0; PARTS]) };
    static NAMED_NS: Cell<[u64; PARTS]> = const { Cell::new([0; PARTS]) };
}

/// The named costs' thread CPU on this thread so far, ns: the grid searches' slices and their pulls (`path::searches`, counted as
/// each slice ends), the kept controllers' steps of the reckoning, the lip guard forecast and the press forecast
/// (`body::kept_clock`).
pub(crate) fn named_ns() -> u64 {
    let s = super::path::searches();
    ((s.spent_cpu_us + s.pull_cpu_us) * 1e3) as u64 + super::body::kept_clock::bots_cpu_ns()
}

/// A span begun: the thread's CPU and the named costs' at its start.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    cpu: Duration,
    named: u64,
}

impl Span {
    pub(crate) fn begin() -> Self {
        Self { cpu: super::cpu_time::thread(), named: named_ns() }
    }

    /// `part`'s time from this span: its thread CPU, and the named costs' part of it.
    pub(crate) fn end(self, part: Part) {
        let cpu = super::cpu_time::thread().saturating_sub(self.cpu).as_nanos() as u64;
        let named = named_ns().saturating_sub(self.named);
        let add = |cell: &'static std::thread::LocalKey<Cell<[u64; PARTS]>>, ns: u64| {
            cell.with(|parts| {
                let mut p = parts.get();
                p[part as usize] += ns;
                parts.set(p);
            });
        };
        add(&TOTAL_NS, cpu);
        add(&NAMED_NS, named);
    }
}

/// A span that ends when it is dropped: a part with early returns ([`Part::Think`]).
pub(crate) struct Guard(Option<(Span, Part)>);

impl Guard {
    pub(crate) fn begin(part: Part) -> Self {
        Self(Some((Span::begin(), part)))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some((span, part)) = self.0.take() {
            span.end(part);
        }
    }
}

/// Each part's thread CPU and its named costs' part since the last [`take`] (ns, in [`Part`]'s order), left as they are: a poll's
/// own split is the difference of two (the bench's worst polls).
pub(crate) fn peek() -> ([u64; PARTS], [u64; PARTS]) {
    (TOTAL_NS.with(Cell::get), NAMED_NS.with(Cell::get))
}

/// Each part's thread CPU and its named costs' part since the last call (ns, in [`Part`]'s order), and zero again.
pub(crate) fn take() -> ([u64; PARTS], [u64; PARTS]) {
    (TOTAL_NS.with(|p| p.replace([0; PARTS])), NAMED_NS.with(|p| p.replace([0; PARTS])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_counts_its_own_cpu_and_take_zeroes_it() {
        let _ = take();
        {
            let _guard = Guard::begin(Part::Queue);
            let busy = super::super::cpu_time::thread();
            let mut x = 0u64;
            while super::super::cpu_time::thread() - busy < Duration::from_millis(2) {
                x = std::hint::black_box(x.wrapping_add(1));
            }
        }
        assert_eq!(peek(), peek(), "a peek leaves the parts as they are");
        let (total, named) = take();
        if cfg!(target_os = "linux") {
            assert!(total[Part::Queue as usize] >= 2_000_000, "{total:?}");
        }
        assert_eq!(named[Part::Queue as usize], 0, "no search or kept step ran");
        assert_eq!(total[Part::Drive as usize], 0);
        assert_eq!(take(), ([0; PARTS], [0; PARTS]));
    }
}
