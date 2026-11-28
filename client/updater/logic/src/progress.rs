//! What the launcher's progress line says while the updater downloads
//! (`client/updater/README.md`, "The launcher's bar"): the speed, measured
//! per request ([`Meter`]), and for a step of several files the bar over
//! them all ([`Step`]).
//!
//! The launcher computes the time left of a download itself, from the
//! progress message's bytes: `(total - done) / speed` (and "... left" is
//! `total - done`; `srp-026g/docs/notes/launcher.md` 5.4, updater state 9);
//! the message's own "seconds left" is shown only while it installs. So the
//! time left of every patch still to come is said by a bar over every
//! patch: its done the bytes of the files before, of this one and those
//! the later ones' parts already hold; its total all their sizes.

/// How long a new request's speed is measured before it is shown: until
/// then the speed measured last is (a new file, a new address, a new try),
/// so the time left does not vanish, or jump, at each one's start.
pub const MEASURE_MS: u64 = 2000;

/// The speed of the bytes a request brings, for the launcher's "Speed" and
/// so its time left.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Meter {
    /// The request measured now: since its first report.
    counting: Option<Counting>,
    /// The speed of the request before, measured over [`MEASURE_MS`] at
    /// least: shown while this one is younger.
    carried: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counting {
    /// The ticks (ms) and the bytes done at its first report.
    since: u64,
    from: u64,
    /// Its speed, once it was measured over [`MEASURE_MS`].
    measured: Option<u64>,
}

impl Meter {
    pub const fn new() -> Self {
        Self {
            counting: None,
            carried: None,
        }
    }

    /// Another kind of progress (checking, a download, an install): nothing
    /// measured, nothing carried.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// A new request of the same kind (the next file, another address, a
    /// new try): measured anew from its first report, the speed measured
    /// before carried meanwhile.
    pub fn restart(&mut self) {
        if let Some(measured) = self.counting.take().and_then(|counting| counting.measured) {
            self.carried = Some(measured);
        }
    }

    /// Bytes a second, at `now` (ms) with `done` bytes; 0: not known.
    /// Fewer bytes than at the first report (a file started again from
    /// byte 0) is a new request.
    pub fn speed(&mut self, now: u64, done: u64) -> u64 {
        if self.counting.is_some_and(|counting| done < counting.from) {
            self.restart();
        }
        let counting = self.counting.get_or_insert(Counting {
            since: now,
            from: done,
            measured: None,
        });
        let elapsed = now.saturating_sub(counting.since);
        let speed = (done - counting.from).saturating_mul(1000) / elapsed.max(1);
        if elapsed >= MEASURE_MS {
            counting.measured = Some(speed);
            return speed;
        }
        self.carried.unwrap_or(speed)
    }
}

/// The seconds left of `total` bytes with `done` of them at `speed`, for
/// the progress message: `u64::MAX`, the pipe's "not known", without a
/// speed or past the total.
pub fn seconds_left(done: u64, total: u64, speed: u64) -> u64 {
    match total.checked_sub(done) {
        Some(left) if speed > 0 => left / speed,
        _ => u64::MAX,
    }
}

/// What a file of a step brings to its bar: its size, and the bytes its
/// part held before the step (gone on from).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Planned {
    size: u64,
    held: u64,
}

/// The files a step downloads one after the other, each of a known size:
/// the bar over them all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    files: alloc::vec::Vec<Planned>,
}

impl Step {
    /// `files`: each one's size (`None`: not known) and the bytes its part
    /// holds already, in the order they are fetched; one that is not
    /// downloaded (the torrent gave it) has size 0. `None` unless every size
    /// is known: the bar is then each file's, as before.
    pub fn new(files: impl IntoIterator<Item = (Option<u64>, u64)>) -> Option<Self> {
        let files = files
            .into_iter()
            .map(|(size, held)| {
                size.map(|size| Planned {
                    size,
                    held: held.min(size),
                })
            })
            .collect::<Option<_>>()?;
        Some(Self { files })
    }

    /// The bytes of every file.
    pub fn total(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }

    /// The bar while the file at `index` is downloaded, the files before it
    /// whole (each is in place before the next starts).
    pub fn bar(&self, index: usize) -> Bar {
        let sum = |files: &[Planned], of: fn(&Planned) -> u64| files.iter().map(of).sum::<u64>();
        let (before, rest) = self.files.split_at(index.min(self.files.len()));
        let (size, later) = match rest.split_first() {
            Some((file, later)) => (file.size, later),
            None => (0, rest),
        };
        Bar::Step {
            before: sum(before, |file| file.size),
            size,
            later: sum(later, |file| file.size),
            later_held: sum(later, |file| file.held),
        }
    }
}

/// What the launcher's bar shows of a file being downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    /// The file alone: its bytes, of its size when its answer says it.
    File,
    /// The step's: the files `before` it whole, this one of `size` bytes
    /// (the list's), the `later` ones' bytes with `later_held` of them in
    /// their parts already.
    Step {
        before: u64,
        size: u64,
        later: u64,
        later_held: u64,
    },
}

impl Bar {
    /// The progress message's done and total, with `done` bytes of the file
    /// on disk and `total` its size as its answer says it (`None`: it does
    /// not). An answer of another size than the list's is taken: its
    /// SHA-256 decides later.
    pub fn shown(self, done: u64, total: Option<u64>) -> (u64, u64) {
        match self {
            Self::File => (done, total.unwrap_or(0)),
            Self::Step {
                before,
                size,
                later,
                later_held,
            } => {
                let size = total.unwrap_or(size).max(done);
                (
                    before.saturating_add(done).saturating_add(later_held),
                    before.saturating_add(size).saturating_add(later),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_left_is_of_every_file_still_to_come() {
        // 100 MB in place by now, 40 of 60 MB of this one, then 300 MB of
        // which a part holds 50.
        let step = Step::new([(Some(100), 0), (Some(60), 10), (Some(300), 50)]).unwrap();
        assert_eq!(step.total(), 460);
        let bar = step.bar(1);
        assert_eq!(
            bar,
            Bar::Step {
                before: 100,
                size: 60,
                later: 300,
                later_held: 50
            }
        );
        let (done, total) = bar.shown(40, Some(60));
        assert_eq!((done, total), (190, 460));
        // Left: 20 of this one and 250 of the next, at 10 a second.
        assert_eq!(seconds_left(done, total, 10), 27);
        // Without a Content-Length, the list's size.
        assert_eq!(bar.shown(40, None), (190, 460));
        // The first file, gone on from nothing; the last, at its end.
        assert_eq!(step.bar(0).shown(0, Some(100)), (50 + 10, 460));
        assert_eq!(step.bar(2).shown(300, Some(300)), (460, 460));
    }

    #[test]
    fn the_bar_goes_on_from_one_file_to_the_next_without_a_jump() {
        let step = Step::new([(Some(100), 30), (Some(60), 20), (Some(300), 50)]).unwrap();
        for index in 0..2 {
            let size = [100, 60, 300][index];
            let next_held = [30, 20, 50][index + 1];
            let end = step.bar(index).shown(size, Some(size));
            let start = step.bar(index + 1).shown(next_held, Some(size));
            assert_eq!(end.0, start.0, "file {index} to the next");
        }
    }

    #[test]
    fn a_size_not_known_leaves_the_bar_each_files() {
        assert_eq!(Step::new([(Some(100), 0), (None, 0)]), None);
        assert_eq!(Bar::File.shown(40, Some(60)), (40, 60));
        assert_eq!(Bar::File.shown(40, None), (40, 0));
        // A torrent's patch has nothing to download.
        let step = Step::new([(Some(0), 0), (Some(60), 0)]).unwrap();
        assert_eq!(step.bar(1).shown(30, Some(60)), (30, 60));
    }

    #[test]
    fn an_answer_unlike_the_list_never_shows_more_done_than_in_all() {
        // A part longer than its size counts as the size.
        let step = Step::new([(Some(10), 0), (Some(20), 99)]).unwrap();
        assert_eq!(step.bar(0).shown(0, Some(10)), (20, 30));
        // This file larger than the list says, or past its total.
        assert_eq!(step.bar(1).shown(5, Some(25)), (15, 35));
        let (done, total) = step.bar(1).shown(40, None);
        assert!(done <= total);
        // Past the end: no time.
        assert_eq!(seconds_left(31, 30, 5), u64::MAX);
        assert_eq!(seconds_left(10, 30, 0), u64::MAX);
        assert_eq!(seconds_left(30, 30, 5), 0);
    }

    #[test]
    fn the_last_speed_is_shown_until_a_new_request_is_measured() {
        let mut meter = Meter::new();
        // The first request: no speed before it, its own from the start.
        assert_eq!(meter.speed(1000, 500), 0);
        assert_eq!(meter.speed(1500, 1000), 1000);
        assert_eq!(meter.speed(3000, 3000), 1250);
        // The next file: the speed measured carried for MEASURE_MS.
        meter.restart();
        assert_eq!(meter.speed(5000, 0), 1250);
        assert_eq!(meter.speed(6000, 4000), 1250);
        assert_eq!(meter.speed(7000, 6000), 3000);
        // Restarted before it was measured: the older speed stays.
        meter.restart();
        assert_eq!(meter.speed(8000, 0), 3000);
        meter.restart();
        assert_eq!(meter.speed(8500, 100), 3000);
        // A file started again from byte 0 is a new request too.
        assert_eq!(meter.speed(10_500, 2100), 1000);
        assert_eq!(meter.speed(11_000, 0), 1000);
        // Another kind of progress: nothing carried.
        meter.reset();
        assert_eq!(meter.speed(20_000, 0), 0);
        assert_eq!(meter.speed(20_500, 100), 200);
    }
}
