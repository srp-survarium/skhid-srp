//! A file of many addresses (`client/updater/README.md`, "Mirrors"): what
//! the updater needs to move on from one that fails and to say why, with
//! no Windows in it. Every address of a file serves the same bytes (its
//! size and SHA-256 are pinned), so a download that broke off goes on at
//! the next address from the bytes already on disk, and a slow one can be
//! left for a faster one at no cost ([`Mirrors`]).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The most of a web page read for its title.
pub const PAGE_HEAD: usize = 8 * 1024;

/// Under this, in bytes a second, an address is slow ...
pub const SLOW_SPEED: u64 = 1 << 20;
/// ... when the file would still take longer than this at its speed: then
/// the other addresses are sampled (more than 120 MiB to go at 1 MiB/s,
/// 12 MiB at 100 KiB/s; a file of less is fetched before a round would pay).
pub const WORTH_MS: u64 = 120_000;
/// How long an address is measured before it is judged, and each stretch
/// after.
pub const MEASURE_MS: u64 = 30_000;
/// How long another address is sampled: enough for a connection to reach
/// its speed, short enough that a slower one costs little.
pub const SAMPLE_MS: u64 = 20_000;
/// The address chosen by a round, under half the speed it was chosen at
/// over this long, has the others sampled again.
pub const FALL_MS: u64 = 60_000;
/// No round sooner than this after the last one ended.
pub const ROUND_GAP_MS: u64 = 5 * 60_000;

/// A stretch of a download: since when, from how many bytes of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Window {
    since: u64,
    done: u64,
}

impl Window {
    /// Its average in bytes a second, once it is `long` milliseconds old.
    fn speed(self, ticks: u64, done: u64, long: u64) -> Option<u64> {
        let elapsed = ticks.saturating_sub(self.since);
        (elapsed >= long).then(|| done.saturating_sub(self.done).saturating_mul(1000) / elapsed)
    }
}

/// How an address failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The network: no connection, broken off, a 5xx. The next pass over
    /// the addresses tries it again.
    Network,
    /// What it answered is not the file (a status, a web page, another
    /// range, too many redirects, another SHA-256): not asked again, nor
    /// sampled, this run.
    Refused,
}

/// What an address is to a file this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Open,
    /// Failed on the network in this pass.
    Failed,
    Refused,
}

/// How the address in use is judged.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pace {
    /// Stretch by stretch of [`MEASURE_MS`]: slow, with much to go, starts
    /// a round.
    Measuring { window: Window },
    /// Chosen by a round at `best`: kept until it breaks off, or falls under
    /// half of it over [`FALL_MS`].
    Settled { best: u64, window: Window },
    /// A round: each other address sampled for [`SAMPLE_MS`] in turn, the
    /// download going on from the bytes on disk; `measured` so far, the
    /// address that started the round first.
    Sampling { measured: Vec<(usize, u64)>, window: Window },
}

/// What happens to a file's download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A request of the address in use goes out; the file has `done` bytes
    /// on disk.
    Begin { ticks: u64, done: u64 },
    /// Bytes came: the file has `done`, of `size` when the answer said it.
    Progress {
        ticks: u64,
        done: u64,
        size: Option<u64>,
    },
    /// The address in use failed.
    Failed { ticks: u64, failure: Failure },
}

/// What the download does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Goes on as it is.
    Stay,
    /// Goes on at address `to` (the one in use: no new request), from the
    /// bytes on disk; after a failure, a new pass first waits a moment
    /// (`pause`). `decision`: what was measured and decided, for the log.
    Go {
        to: usize,
        pause: bool,
        decision: Option<Decision>,
    },
    /// After a failure: no address is left this run.
    GiveUp,
}

/// A decision of speed, for its line in the log ([`Decision::line`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Address `at` was slow (its speed over [`MEASURE_MS`]) with `left`
    /// bytes to go: the round's first sample is next.
    Slow { at: usize, speed: u64, left: u64 },
    /// The round's choice `at`, under half its `best` over [`FALL_MS`].
    Fell { at: usize, speed: u64, best: u64 },
    /// Address `at` sampled; the next sample is next.
    Sampled { at: usize, speed: u64 },
    /// The round's speeds, in the order measured: the fastest is gone on
    /// at.
    Chosen { speeds: Vec<(usize, u64)> },
}

fn kb(speed: u64) -> u64 {
    speed / 1024
}

impl Decision {
    /// What was measured, and where the download goes on (`to`, from byte
    /// `from`): `host(i)` names address i.
    pub fn line(&self, to: usize, from: u64, host: &dyn Fn(usize) -> String) -> String {
        let named = |at: usize| format!("{} (address {})", host(at), at + 1);
        let next = format!("goes on at {} from byte {from}", named(to));
        match self {
            Self::Slow { at, speed, left } => format!(
                "{}: {} KB/s over {} s, {} MB to go: {next}, to sample it",
                named(*at),
                kb(*speed),
                MEASURE_MS / 1000,
                left >> 20
            ),
            Self::Fell { at, speed, best } => format!(
                "{}: {} KB/s over {} s, under half of the {} KB/s it was chosen at: {next}, \
                 to sample it",
                named(*at),
                kb(*speed),
                FALL_MS / 1000,
                kb(*best)
            ),
            Self::Sampled { at, speed } => format!(
                "{}: {} KB/s over {} s: {next}, to sample it",
                named(*at),
                kb(*speed),
                SAMPLE_MS / 1000
            ),
            Self::Chosen { speeds } => {
                let measured: Vec<String> = speeds
                    .iter()
                    .map(|(at, speed)| format!("{} {} KB/s", host(*at), kb(*speed)))
                    .collect();
                format!("measured {}: {next}", measured.join(", "))
            }
        }
    }
}

/// One file's addresses as a download uses them: in their order, a
/// failure moving on to the next open one, passes over them as the caller
/// allows (a new pass tries again those that failed on the network); and
/// a slow address, with much of the file to go, has the others sampled
/// and the fastest measured gone on at. A slow address is never a failure:
/// when every one is slow, the fastest is used and waited for. One
/// connection at a time; every address goes on from the bytes on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mirrors {
    marks: Vec<Mark>,
    passes: u32,
    pass: u32,
    at: usize,
    pace: Pace,
    /// When the last round ended.
    last_round: Option<u64>,
}

impl Mirrors {
    /// `count` addresses (at least one), `passes` over them at most.
    pub fn new(count: usize, passes: u32) -> Self {
        Self {
            marks: alloc::vec![Mark::Open; count],
            passes,
            pass: 1,
            at: 0,
            pace: Pace::Measuring {
                window: Window { since: 0, done: 0 },
            },
            last_round: None,
        }
    }

    /// The address in use.
    pub fn at(&self) -> usize {
        self.at
    }

    /// The pass over the addresses, from 1.
    pub fn pass(&self) -> u32 {
        self.pass
    }

    /// The download's next step, from what happened.
    pub fn on(&mut self, event: Event) -> Action {
        match event {
            Event::Begin { ticks, done } => {
                *self.window() = Window { since: ticks, done };
                Action::Stay
            }
            Event::Progress { ticks, done, size } => self.progress(ticks, done, size),
            Event::Failed { ticks, failure } => {
                self.marks[self.at] = match failure {
                    Failure::Network => Mark::Failed,
                    Failure::Refused => Mark::Refused,
                };
                if matches!(self.pace, Pace::Sampling { .. }) {
                    return self.next_sample(ticks, 0, None);
                }
                self.pace = Pace::Measuring {
                    window: Window { since: ticks, done: 0 },
                };
                self.next_open()
            }
        }
    }

    fn window(&mut self) -> &mut Window {
        match &mut self.pace {
            Pace::Measuring { window } | Pace::Settled { window, .. } | Pace::Sampling { window, .. } => {
                window
            }
        }
    }

    fn progress(&mut self, ticks: u64, done: u64, size: Option<u64>) -> Action {
        let window = self.window();
        if done < window.done {
            // The whole file again from byte 0 (a server that gives no part).
            *window = Window { since: ticks, done };
            return Action::Stay;
        }
        let at = self.at;
        let round_open = self
            .last_round
            .is_none_or(|ended| ticks >= ended.saturating_add(ROUND_GAP_MS));
        let left = size.map(|size| size.saturating_sub(done));
        // Much to go at this speed: worth a round.
        let much = |speed: u64| {
            left.is_some_and(|left| left.saturating_mul(1000) / speed.max(1) > WORTH_MS)
        };
        let restart = Window { since: ticks, done };
        match &mut self.pace {
            Pace::Measuring { window } => {
                let Some(speed) = window.speed(ticks, done, MEASURE_MS) else {
                    return Action::Stay;
                };
                *window = restart;
                if speed < SLOW_SPEED && much(speed) && round_open {
                    let left = left.unwrap_or(0);
                    return self.round(speed, Decision::Slow { at, speed, left });
                }
                Action::Stay
            }
            Pace::Settled { best, window } => {
                let best = *best;
                let Some(speed) = window.speed(ticks, done, FALL_MS) else {
                    return Action::Stay;
                };
                *window = restart;
                if speed < best / 2 && much(speed) && round_open {
                    return self.round(speed, Decision::Fell { at, speed, best });
                }
                Action::Stay
            }
            Pace::Sampling { measured, window } => {
                let Some(speed) = window.speed(ticks, done, SAMPLE_MS) else {
                    return Action::Stay;
                };
                measured.push((at, speed));
                self.next_sample(ticks, done, Some(speed))
            }
        }
    }

    /// The open addresses not measured in the round, in their order after
    /// the one in use.
    fn unsampled(&self, measured: &[(usize, u64)]) -> Option<usize> {
        let count = self.marks.len();
        (1..count)
            .map(|step| (self.at + step) % count)
            .find(|&at| self.marks[at] == Mark::Open && measured.iter().all(|(seen, _)| *seen != at))
    }

    /// A round from the address in use, at `speed`: its first sample, if
    /// any other address is open.
    fn round(&mut self, speed: u64, decision: Decision) -> Action {
        let measured = alloc::vec![(self.at, speed)];
        let Some(to) = self.unsampled(&measured) else {
            return Action::Stay;
        };
        self.pace = Pace::Sampling {
            measured,
            window: Window { since: 0, done: 0 },
        };
        self.at = to;
        Action::Go {
            to,
            pause: false,
            decision: Some(decision),
        }
    }

    /// After a sample (its speed; `None`: it failed, and a new request
    /// follows): the next one, or the fastest measured. `done`: the bytes on
    /// disk, for a choice of the address in use.
    fn next_sample(&mut self, ticks: u64, done: u64, speed: Option<u64>) -> Action {
        let Pace::Sampling { measured, .. } = &self.pace else {
            unreachable!("only while sampling");
        };
        let measured = measured.clone();
        let sampled = self.at;
        if let Some(to) = self.unsampled(&measured) {
            self.at = to;
            if let Pace::Sampling { window, .. } = &mut self.pace {
                *window = Window { since: ticks, done };
            }
            return Action::Go {
                to,
                pause: false,
                decision: speed.map(|speed| Decision::Sampled { at: sampled, speed }),
            };
        }
        let (to, best) = measured
            .iter()
            .copied()
            .filter(|(at, _)| self.marks[*at] == Mark::Open)
            .fold(None, |fastest: Option<(usize, u64)>, (at, speed)| match fastest {
                Some((_, top)) if top >= speed => fastest,
                _ => Some((at, speed)),
            })
            .expect("the address that started the round is open");
        self.at = to;
        self.last_round = Some(ticks);
        self.pace = Pace::Settled {
            best,
            window: Window { since: ticks, done },
        };
        Action::Go {
            to,
            pause: false,
            decision: Some(Decision::Chosen { speeds: measured }),
        }
    }

    /// After a failure out of a round: the next open address after it, else
    /// a new pass, else none.
    fn next_open(&mut self) -> Action {
        let count = self.marks.len();
        if let Some(to) = (1..count)
            .map(|step| (self.at + step) % count)
            .find(|&at| self.marks[at] == Mark::Open)
        {
            self.at = to;
            return Action::Go {
                to,
                pause: false,
                decision: None,
            };
        }
        if self.pass >= self.passes {
            return Action::GiveUp;
        }
        self.pass += 1;
        for mark in &mut self.marks {
            if *mark == Mark::Failed {
                *mark = Mark::Open;
            }
        }
        match self.marks.iter().position(|mark| *mark == Mark::Open) {
            Some(to) => {
                self.at = to;
                Action::Go {
                    to,
                    pause: true,
                    decision: None,
                }
            }
            None => Action::GiveUp,
        }
    }
}

/// Where the body of a `206 Partial Content` answer starts, by its
/// `Content-Range: bytes <first>-<last>/<size or *>`.
pub fn range_start(content_range: &str) -> Option<u64> {
    let range = content_range.trim().strip_prefix("bytes ")?;
    let (first, rest) = range.split_once('-')?;
    let (last, _) = rest.split_once('/')?;
    let number = |text: &str| -> Option<u64> {
        (!text.is_empty() && text.len() <= 19 && text.bytes().all(|c| c.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    let first = number(first)?;
    (number(last)? >= first).then_some(first)
}

/// Why an answer to a request for a file's bytes is not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Another status than 200 (or 206 or 416 to a request from a byte on).
    Status(u32),
    /// A `206` of another range than the one asked for, or of none.
    BadRange,
    /// A `416` to `Range: bytes=<n>-`: the file has no byte n, so the bytes
    /// on disk are at least the whole file. Its size, when the answer names
    /// it (`Content-Range: bytes */<size>`).
    PastEnd(Option<u64>),
}

/// Where the body of an answer starts in the file, for a request from byte
/// `from` (`Range: bytes=<from>-` when it is not 0): a `200` is the whole
/// file again from byte 0 (a server that gives no part); a `206` goes on
/// from `from` when its `Content-Range` starts there.
pub fn body_start(from: u64, status: u32, content_range: Option<&str>) -> Result<u64, Refused> {
    match status {
        200 => Ok(0),
        206 if from > 0 => content_range
            .and_then(range_start)
            .filter(|&start| start == from)
            .ok_or(Refused::BadRange),
        416 if from > 0 => Err(Refused::PastEnd(content_range.and_then(unsatisfied_size))),
        status => Err(Refused::Status(status)),
    }
}

/// The size a `416` names: `Content-Range: bytes */<size>`.
fn unsatisfied_size(content_range: &str) -> Option<u64> {
    let size = content_range.trim().strip_prefix("bytes */")?;
    (!size.is_empty() && size.len() <= 19 && size.bytes().all(|c| c.is_ascii_digit()))
        .then(|| size.parse().ok())
        .flatten()
}

/// The `<title>` of a web page's first bytes (any host's: a quota page, a
/// sign-in page, a virus-scan page), as one line of at most 100
/// characters; `None` when they have none.
pub fn title(page: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(page);
    let lower = text.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let mut title = String::new();
    for word in text[start..end].split_whitespace() {
        if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(word);
    }
    let title = title
        .replace("&amp;", "&")
        .replace("&quot;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace('"', "'");
    let mut shown: String = title
        .chars()
        .filter(|c| !c.is_control())
        .take(100)
        .collect();
    if shown.len() < title.len() {
        shown.push_str("...");
    }
    (!shown.is_empty()).then_some(shown)
}

/// A line of the log about one address of a file: `<what>  <verb>
/// <host>: <detail>`, the verb `try`, `failed` or `ok`, as
/// `Skhid-without-updater.bat` writes them too:
///
/// ```text
/// skhid-game  failed   drive.usercontent.google.com: web page "Google Drive - Quota exceeded", not the file
/// ```
pub fn line(what: &str, verb: &str, host: &str, detail: &str) -> String {
    format!("{what}  {verb:<8} {host}: {detail}")
}

/// What a player is told when no address gave the file: each address's
/// host and why it failed, in their order, `; ` between.
pub fn failures(failed: &[(String, String)]) -> String {
    let mut out = String::new();
    for (host, why) in failed {
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(&format!("{host}: {why}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KB: u64 = 1024;
    const MB: u64 = 1 << 20;
    /// patch_69's size.
    const SIZE: u64 = 3_698_326_100;

    /// A download on a fake clock: each address gives its bytes a second;
    /// a progress event every second, a new request (`Begin`) after every
    /// move to another address.
    struct Fake {
        mirrors: Mirrors,
        speeds: Vec<u64>,
        size: Option<u64>,
        ticks: u64,
        done: u64,
    }

    impl Fake {
        fn new(speeds: &[u64], passes: u32) -> Self {
            let mut mirrors = Mirrors::new(speeds.len(), passes);
            assert_eq!(mirrors.on(Event::Begin { ticks: 0, done: 0 }), Action::Stay);
            Self {
                mirrors,
                speeds: speeds.into(),
                size: Some(SIZE),
                ticks: 0,
                done: 0,
            }
        }

        fn follow(&mut self, action: &Action, from: usize) {
            if let Action::Go { to, .. } = action
                && *to != from
            {
                let begin = Event::Begin {
                    ticks: self.ticks,
                    done: self.done,
                };
                assert_eq!(self.mirrors.on(begin), Action::Stay);
            }
        }

        /// `seconds` of the download: the decisions made, with their second.
        fn run(&mut self, seconds: u64) -> Vec<(u64, Action)> {
            let mut decided = Vec::new();
            for _ in 0..seconds {
                self.ticks += 1000;
                let at = self.mirrors.at();
                self.done = (self.done + self.speeds[at]).min(self.size.unwrap_or(u64::MAX));
                let action = self.mirrors.on(Event::Progress {
                    ticks: self.ticks,
                    done: self.done,
                    size: self.size,
                });
                if action != Action::Stay {
                    self.follow(&action, at);
                    decided.push((self.ticks / 1000, action));
                }
            }
            decided
        }

        fn fail(&mut self, failure: Failure) -> Action {
            let at = self.mirrors.at();
            let action = self.mirrors.on(Event::Failed {
                ticks: self.ticks,
                failure,
            });
            self.follow(&action, at);
            action
        }
    }

    fn go(to: usize, decision: Decision) -> Action {
        Action::Go {
            to,
            pause: false,
            decision: Some(decision),
        }
    }

    fn moved(to: usize, pause: bool) -> Action {
        Action::Go {
            to,
            pause,
            decision: None,
        }
    }

    #[test]
    fn a_slow_address_has_the_others_sampled_and_the_fastest_is_gone_on_at() {
        // The player of 2026-11-22: Dropbox at 512 KB/s, our server at 3 MB/s.
        let mut fake = Fake::new(&[512 * KB, 3 * MB], 2);
        let decided = fake.run(3600);
        let left = SIZE - 30 * 512 * KB;
        assert_eq!(
            decided,
            [
                (30, go(1, Decision::Slow { at: 0, speed: 512 * KB, left })),
                (50, go(1, Decision::Chosen { speeds: alloc::vec![(0, 512 * KB), (1, 3 * MB)] })),
            ]
        );
        // The rest at 3 MB/s: the whole file in about 20 minutes.
        assert_eq!(fake.done, SIZE);
        let host = |at: usize| String::from(["dl.dropboxusercontent.com", "2.28.121.133"][at]);
        let Action::Go { decision: Some(slow), .. } = &decided[0].1 else { unreachable!() };
        assert_eq!(
            slow.line(1, 15_728_640, &host),
            "dl.dropboxusercontent.com (address 1): 512 KB/s over 30 s, 3511 MB to go: goes on \
             at 2.28.121.133 (address 2) from byte 15728640, to sample it"
        );
        let Action::Go { decision: Some(chosen), .. } = &decided[1].1 else { unreachable!() };
        assert_eq!(
            chosen.line(1, 78_643_200, &host),
            "measured dl.dropboxusercontent.com 512 KB/s, 2.28.121.133 3072 KB/s: goes on at \
             2.28.121.133 (address 2) from byte 78643200"
        );
    }

    #[test]
    fn when_every_address_is_slow_the_fastest_is_waited_for() {
        let mut fake = Fake::new(&[100 * KB, 200 * KB, 50 * KB], 2);
        let decided = fake.run(4 * 3600);
        assert_eq!(
            decided,
            [
                (30, go(1, Decision::Slow { at: 0, speed: 100 * KB, left: SIZE - 3000 * KB })),
                (50, go(2, Decision::Sampled { at: 1, speed: 200 * KB })),
                (
                    70,
                    go(
                        1,
                        Decision::Chosen {
                            speeds: alloc::vec![(0, 100 * KB), (1, 200 * KB), (2, 50 * KB)]
                        }
                    )
                ),
            ],
            "one round, then the fastest kept: no failure, no flapping"
        );
    }

    #[test]
    fn a_choice_that_falls_under_half_is_measured_again_only_after_a_pause() {
        let mut fake = Fake::new(&[512 * KB, 3 * MB], 2);
        assert_eq!(fake.run(60).len(), 2);
        // The chosen one falls to 1 MB/s: under half of 3 MB/s, but the
        // round ended at 50 s, so the next starts no sooner than 350 s.
        fake.speeds[1] = MB;
        let decided = fake.run(400);
        assert_eq!(
            decided,
            [
                (350, go(0, Decision::Fell { at: 1, speed: MB, best: 3 * MB })),
                (370, go(1, Decision::Chosen { speeds: alloc::vec![(1, MB), (0, 512 * KB)] })),
            ]
        );
        // Kept at 1 MB/s from then on.
        assert_eq!(fake.run(1200), []);
    }

    #[test]
    fn a_file_with_little_to_go_or_of_no_known_size_is_not_worth_a_round() {
        // 50 MB at 512 KB/s: 100 s to go, less than a round would pay.
        let mut fake = Fake::new(&[512 * KB, 3 * MB], 2);
        fake.size = Some(50 * MB);
        assert_eq!(fake.run(200), []);
        assert_eq!(fake.done, 50 * MB);
        let mut fake = Fake::new(&[512 * KB, 3 * MB], 2);
        fake.size = None;
        assert_eq!(fake.run(600), []);
        // One address: nothing to sample.
        let mut fake = Fake::new(&[10 * KB], 2);
        assert_eq!(fake.run(600), []);
    }

    #[test]
    fn a_file_started_again_from_byte_0_is_measured_from_there() {
        let mut fake = Fake::new(&[512 * KB, 3 * MB], 2);
        fake.done = 2 * 1000 * MB;
        fake.mirrors.on(Event::Begin { ticks: 0, done: fake.done });
        // The answer is the whole file again (a 200): byte 0.
        fake.done = 0;
        let decided = fake.run(30);
        assert_eq!(decided, [], "not judged at a speed below nothing");
        let decided = fake.run(1);
        assert_eq!(decided, [(31, go(1, Decision::Slow { at: 0, speed: 512 * KB, left: SIZE - 31 * 512 * KB }))]);
    }

    #[test]
    fn an_address_that_refused_is_not_sampled_again_and_one_that_broke_off_not_this_pass() {
        let mut fake = Fake::new(&[400 * KB, 300 * KB, 2 * MB], 2);
        fake.run(1);
        // A web page, say: the next address.
        assert_eq!(fake.fail(Failure::Refused), moved(1, false));
        let decided = fake.run(60);
        assert_eq!(decided[0].1, go(2, Decision::Slow { at: 1, speed: 300 * KB, left: SIZE - 400 * KB - 30 * 300 * KB }));
        assert_eq!(
            decided[1].1,
            go(2, Decision::Chosen { speeds: alloc::vec![(1, 300 * KB), (2, 2 * MB)] })
        );
        // The choice breaks off: the next open one, never the refused one.
        assert_eq!(fake.fail(Failure::Network), moved(1, false));
        // Slow, but the only other open address is one that failed this pass.
        assert_eq!(fake.run(3600), []);
        // A new pass opens it again.
        assert_eq!(fake.fail(Failure::Network), moved(1, true));
        assert_eq!(fake.mirrors.pass(), 2);
        assert_eq!(fake.fail(Failure::Network), moved(2, false));
        assert_eq!(fake.fail(Failure::Network), Action::GiveUp);
    }

    #[test]
    fn a_sample_that_fails_moves_on_to_the_next_sample() {
        let mut fake = Fake::new(&[512 * KB, 2 * MB, 3 * MB], 2);
        assert_eq!(fake.run(30).len(), 1);
        fake.run(5);
        assert_eq!(fake.fail(Failure::Network), moved(2, false));
        assert_eq!(
            fake.run(20),
            [(55, go(2, Decision::Chosen { speeds: alloc::vec![(0, 512 * KB), (2, 3 * MB)] }))]
        );
    }

    #[test]
    fn failures_without_speed_go_in_order_and_a_second_pass_tries_the_network_ones() {
        let mut mirrors = Mirrors::new(3, 2);
        let mut tried = alloc::vec![mirrors.at()];
        for failure in [Failure::Network, Failure::Refused, Failure::Network, Failure::Network] {
            match mirrors.on(Event::Failed { ticks: 0, failure }) {
                Action::Go { to, .. } => tried.push(to),
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(tried, [0, 1, 2, 0, 2]);
        assert_eq!(mirrors.on(Event::Failed { ticks: 0, failure: Failure::Network }), Action::GiveUp);
        // The game's archive: one pass.
        let mut mirrors = Mirrors::new(2, 1);
        assert_eq!(mirrors.on(Event::Failed { ticks: 0, failure: Failure::Network }), moved(1, false));
        assert_eq!(mirrors.on(Event::Failed { ticks: 0, failure: Failure::Network }), Action::GiveUp);
    }

    #[test]
    fn a_partial_answer_says_where_it_starts() {
        assert_eq!(range_start("bytes 100-2906544102/2906544103"), Some(100));
        assert_eq!(range_start(" bytes 0-0/*"), Some(0));
        for bad in [
            "",
            "bytes",
            "bytes */100",
            "bytes 5-4/10",
            "bytes -5/10",
            "items 0-1/2",
            "bytes 1-x/2",
        ] {
            assert_eq!(range_start(bad), None, "{bad}");
        }
    }

    #[test]
    fn an_answer_goes_on_from_the_byte_asked_for_or_from_0() {
        let range = "bytes 2833252352-3698326099/3698326100";
        // Resumed: a 206 from the byte asked for.
        assert_eq!(body_start(2_833_252_352, 206, Some(range)), Ok(2_833_252_352));
        // A server that gives no part: the whole file again.
        assert_eq!(body_start(2_833_252_352, 200, None), Ok(0));
        assert_eq!(body_start(0, 200, None), Ok(0));
        // Another range, or none named, is refused.
        assert_eq!(body_start(100, 206, Some(range)), Err(Refused::BadRange));
        assert_eq!(body_start(100, 206, Some("bytes 0-99/3698326100")), Err(Refused::BadRange));
        assert_eq!(body_start(100, 206, None), Err(Refused::BadRange));
        // A part never asked for.
        assert_eq!(body_start(0, 206, Some(range)), Err(Refused::Status(206)));
        // Nothing past what is on disk: as long as the file, or longer.
        assert_eq!(
            body_start(3_698_326_100, 416, Some("bytes */3698326100")),
            Err(Refused::PastEnd(Some(3_698_326_100)))
        );
        assert_eq!(body_start(500, 416, Some("bytes */x")), Err(Refused::PastEnd(None)));
        assert_eq!(body_start(500, 416, None), Err(Refused::PastEnd(None)));
        assert_eq!(body_start(0, 416, None), Err(Refused::Status(416)));
        assert_eq!(body_start(500, 404, None), Err(Refused::Status(404)));
    }

    #[test]
    fn a_web_page_is_named_by_its_title() {
        let quota = b"<!DOCTYPE html><html><head><meta charset=\"utf-8\"><TITLE>Google Drive -\n  Quota exceeded</TITLE></head><body>...";
        assert_eq!(
            title(quota).as_deref(),
            Some("Google Drive - Quota exceeded")
        );
        assert_eq!(
            title(b"<title lang=en>Sign in &amp; \"go\"</title>").as_deref(),
            Some("Sign in & 'go'")
        );
        assert_eq!(title(b"<html><body>no title</body></html>"), None);
        assert_eq!(title(b"<title>   </title>"), None);
        assert_eq!(title(b"<title>cut off"), None);
        let long = format!("<title>{}</title>", "x".repeat(300));
        assert_eq!(title(long.as_bytes()).unwrap().len(), 103);
    }

    #[test]
    fn the_lines_of_the_log_and_the_players_words() {
        assert_eq!(
            line(
                "skhid-game",
                "failed",
                "drive.usercontent.google.com",
                "web page \"Google Drive - Quota exceeded\", not the file"
            ),
            "skhid-game  failed   drive.usercontent.google.com: web page \"Google Drive - Quota exceeded\", not the file"
        );
        assert_eq!(
            failures(&[
                ("2.28.121.133".into(), "connect timeout".into()),
                ("archive.org".into(), "HTTP 503".into())
            ]),
            "2.28.121.133: connect timeout; archive.org: HTTP 503"
        );
    }
}
