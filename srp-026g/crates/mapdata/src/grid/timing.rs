//! [incremental grids] Where a grid build's time goes: the wall seconds of each pass of [`super::WalkGrid::build_kept_with_shapes`]
//! and, for each call of [`super::controlled::parallel`] (the build's heavy simulation), its items, the summed seconds its jobs ran,
//! its longest job and its wall seconds, by the call's place in the source. For the build's log only: a build's timing is in no
//! grid file, no key and no digest, and two builds' [`super::BuildStats`] are equal whatever their timing ([`BuildTiming`]'s
//! `PartialEq`).
//!
//! The build owns one [`Clock`] (made at its start, ended by [`Clock::finish`]). A `parallel` call made on the thread that holds a
//! running clock is recorded on it; one made elsewhere (a job's own `parallel`, a caller outside a build) records nothing and
//! times nothing.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::panic::Location;
use std::time::{Duration, Instant};

/// A build's times ([`super::BuildStats::timing`]).
#[derive(Clone, Debug, Default)]
pub struct BuildTiming {
    /// The build's wall seconds, from its clock's start to its end.
    pub total_s: f64,
    /// Each pass in the build's order, as its clock's laps.
    pub passes: Vec<PassTime>,
    /// Each `parallel` call, in the order they ran.
    pub sites: Vec<SiteTime>,
}

/// Timing is not a build's result: builds that made the same grid are equal stats however long they took.
impl PartialEq for BuildTiming {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// One pass's wall time.
#[derive(Clone, Debug)]
pub struct PassTime {
    pub pass: &'static str,
    pub wall_s: f64,
}

/// One `parallel` call.
#[derive(Clone, Debug)]
pub struct SiteTime {
    /// The call's source file (as the compiler names it) and line.
    pub file: &'static str,
    pub line: u32,
    /// The pass it ran in: the name of the lap that ended after it.
    pub pass: &'static str,
    pub items: usize,
    pub threads: usize,
    pub wall_s: f64,
    /// The jobs' summed seconds (each job's own wall time, on its thread).
    pub job_s: f64,
    /// The longest job.
    pub max_job_s: f64,
}

impl SiteTime {
    /// `file:line` without the folders before `src/`.
    pub fn place(&self) -> String {
        let file = self.file.replace('\\', "/");
        let short = file.rfind("src/").map_or(file.as_str(), |at| &file[at..]);
        format!("{short}:{}", self.line)
    }
}

impl BuildTiming {
    /// The table for a log, one line each: `pass <name> <wall s> s`, then under it its sites `site <file:line> items <n> job
    /// <s> s max <s> s wall <s> s threads <n>`, the last line the total and the sites' share of it.
    pub fn table(&self) -> String {
        let mut out = String::new();
        for pass in &self.passes {
            let _ = writeln!(out, "pass {:<34} {:>10.2} s", pass.pass, pass.wall_s);
            for site in self.sites.iter().filter(|s| s.pass == pass.pass) {
                let _ = writeln!(
                    out,
                    "  site {:<32} items {:>9} job {:>10.2} s max {:>8.3} s wall {:>9.2} s threads {}",
                    site.place(),
                    site.items,
                    site.job_s,
                    site.max_job_s,
                    site.wall_s,
                    site.threads
                );
            }
        }
        let site_wall: f64 = self.sites.iter().map(|s| s.wall_s).sum();
        let site_job: f64 = self.sites.iter().map(|s| s.job_s).sum();
        let _ = write!(
            out,
            "total {:.2} s: parallel sites wall {site_wall:.2} s (job {site_job:.2} s), the rest (serial) {:.2} s",
            self.total_s,
            (self.total_s - site_wall).max(0.0)
        );
        out
    }
}

/// The running clock's record on its thread.
struct Running {
    started: Instant,
    last: Instant,
    timing: BuildTiming,
    /// The first site not given a pass yet.
    open: usize,
}

thread_local! {
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
}

/// A build's clock: running from [`Clock::start`] on this thread until [`Clock::finish`] (or its drop, which puts back the clock
/// it replaced, if any).
pub(crate) struct Clock {
    replaced: Option<Running>,
}

impl Clock {
    pub(crate) fn start() -> Self {
        let now = Instant::now();
        let running = Running { started: now, last: now, timing: BuildTiming::default(), open: 0 };
        Clock { replaced: RUNNING.with(|r| r.borrow_mut().replace(running)) }
    }

    /// Ends the pass `pass`: the time since the last lap, and the `parallel` calls made since then.
    pub(crate) fn lap(&mut self, pass: &'static str) {
        RUNNING.with(|r| {
            if let Some(running) = r.borrow_mut().as_mut() {
                let now = Instant::now();
                running.timing.passes.push(PassTime { pass, wall_s: (now - running.last).as_secs_f64() });
                running.last = now;
                for site in &mut running.timing.sites[running.open..] {
                    site.pass = pass;
                }
                running.open = running.timing.sites.len();
            }
        });
    }

    /// The build's times; the clock this one replaced runs again.
    pub(crate) fn finish(mut self) -> BuildTiming {
        let replaced = self.replaced.take();
        // Its drop would put back what it replaced, which this does now.
        std::mem::forget(self);
        let running = RUNNING.with(|r| std::mem::replace(&mut *r.borrow_mut(), replaced));
        let mut timing = running.map(|r| BuildTiming { total_s: r.started.elapsed().as_secs_f64(), ..r.timing }).unwrap_or_default();
        // Calls after the last lap.
        for site in timing.sites.iter_mut().filter(|s| s.pass.is_empty()) {
            site.pass = "(after the last pass)";
        }
        timing
    }
}

/// A clock left without its finish (a panic in the build) stops: the one it replaced, or none, runs on this thread.
impl Drop for Clock {
    fn drop(&mut self) {
        let replaced = self.replaced.take();
        RUNNING.with(|r| *r.borrow_mut() = replaced);
    }
}

/// Whether a clock runs on this thread: a `parallel` call times its jobs only then.
pub(crate) fn running() -> bool {
    RUNNING.with(|r| r.borrow().is_some())
}

/// One job's times on a worker: summed and longest.
#[derive(Clone, Copy, Default)]
pub(crate) struct JobTimes {
    pub sum: Duration,
    pub max: Duration,
}

impl JobTimes {
    pub(crate) fn add(&mut self, took: Duration) {
        self.sum += took;
        self.max = self.max.max(took);
    }

    pub(crate) fn merge(&mut self, other: JobTimes) {
        self.sum += other.sum;
        self.max = self.max.max(other.max);
    }
}

/// Records a `parallel` call made at `at` on the running clock of this thread (none: nothing).
pub(crate) fn record_site(at: &'static Location<'static>, items: usize, threads: usize, wall: Duration, jobs: JobTimes) {
    RUNNING.with(|r| {
        if let Some(running) = r.borrow_mut().as_mut() {
            running.timing.sites.push(SiteTime {
                file: at.file(),
                line: at.line(),
                pass: "",
                items,
                threads,
                wall_s: wall.as_secs_f64(),
                job_s: jobs.sum.as_secs_f64(),
                max_job_s: jobs.max.as_secs_f64(),
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laps_name_the_sites_called_before_them() {
        let mut clock = Clock::start();
        assert!(running());
        let at = Location::caller();
        record_site(at, 10, 2, Duration::from_millis(5), JobTimes { sum: Duration::from_millis(9), max: Duration::from_millis(2) });
        clock.lap("first");
        clock.lap("second");
        record_site(at, 3, 1, Duration::ZERO, JobTimes::default());
        let timing = clock.finish();
        assert!(!running(), "the clock stops at its finish");
        assert_eq!(timing.passes.iter().map(|p| p.pass).collect::<Vec<_>>(), ["first", "second"]);
        assert_eq!(timing.sites.len(), 2);
        assert_eq!(timing.sites[0].pass, "first");
        assert_eq!(timing.sites[0].items, 10);
        assert_eq!(timing.sites[1].pass, "(after the last pass)");
        assert!(timing.sites[0].place().starts_with("src/grid/timing.rs:"), "{}", timing.sites[0].place());
        let table = timing.table();
        assert!(table.contains("pass first") && table.contains("site src/grid/timing.rs:") && table.starts_with("pass"), "{table}");
    }

    #[test]
    fn no_clock_records_nothing_and_a_nested_clock_gives_back_the_outer() {
        assert!(!running());
        record_site(Location::caller(), 1, 1, Duration::ZERO, JobTimes::default());
        let mut outer = Clock::start();
        {
            let inner = Clock::start();
            record_site(Location::caller(), 1, 1, Duration::ZERO, JobTimes::default());
            assert_eq!(inner.finish().sites.len(), 1);
        }
        outer.lap("outer");
        let timing = outer.finish();
        assert_eq!(timing.sites.len(), 0, "the inner clock's call is the inner's");
        assert_eq!(timing.passes.len(), 1);
        // A clock dropped without its finish puts back the one it replaced.
        let outer = Clock::start();
        drop(Clock::start());
        assert!(running());
        drop(outer.finish());
        assert!(!running());
        drop(Clock::start());
        assert!(!running(), "a dropped outermost clock stops");
    }

    #[test]
    fn timing_never_makes_stats_differ() {
        let a = BuildTiming { total_s: 1.0, ..Default::default() };
        let b = BuildTiming { total_s: 2.0, passes: vec![PassTime { pass: "x", wall_s: 2.0 }], sites: Vec::new() };
        assert_eq!(a, b);
    }
}
