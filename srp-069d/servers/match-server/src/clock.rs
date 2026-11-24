//! The match clock's rate.
//!
//! The relay paces its DFs, confirmations and timeouts by `Instant` (CLOCK_MONOTONIC). The clients run on the Windows
//! clock: their own timer runs at wall speed times a servo factor driven by the relay's confirmations,
//! so a relay clock that runs slow drags every client's world with it. Measured on a WSL test server:
//! the VM's CLOCK_MONOTONIC (clocksource `tsc`) ran 6.2-7.1 % slower than Windows' QueryPerformanceCounter, tick count and
//! system time (which agree within 0.005 %), while the VM's CLOCK_REALTIME is stepped back to the Windows time by
//! `hv_utils` timesync (a sawtooth within about +-1.3 s). The client's countdown of 20 s of world time then took 21.4 s of
//! Windows time: the whole match ran 7 % in slow motion.
//!
//! [`Discipline`] maps monotonic instants onto a match clock that runs at the wall clock's long-term rate: the slope of a
//! least-squares line through (monotonic, wall) samples of the last [`WINDOW_S`] seconds (the realtime steps average out),
//! applied smoothly (the rate moves at most [`MAX_SLEW_PER_S`] per second, so the clients' servo follows without a jolt) and
//! never backwards. With a correct monotonic clock the slope is 1 and the mapping stays the identity.

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The rate estimate looks back this far (seconds of monotonic time).
pub const WINDOW_S: f64 = 600.0;
/// No estimate before this much history: the rate stays 1 until then.
pub const MIN_SPAN_S: f64 = 30.0;
/// One sample per this many monotonic seconds.
pub const SAMPLE_EVERY_S: f64 = 1.0;
/// The applied rate moves toward the estimate by at most this much per second (a 7 % gap is taken up in 70 s).
pub const MAX_SLEW_PER_S: f64 = 0.001;
/// The estimated rate is clamped to this range (anything outside is not a clock rate but a wall-clock jump).
pub const RATE_MIN: f64 = 0.8;
pub const RATE_MAX: f64 = 1.25;
/// A wall sample this far from the fitted line is a wall-clock jump (manual set, VM resume): the history starts over.
pub const JUMP_S: f64 = 30.0;
/// The mapped clock's offset from the fitted wall line is pulled in with this time constant, at most `OFFSET_RATE_MAX`.
pub const OFFSET_TAU_S: f64 = 120.0;
pub const OFFSET_RATE_MAX: f64 = 0.01;

#[derive(Debug)]
pub struct Discipline {
    /// the first monotonic instant and the mapped instant at the anchor of the current rate
    origin: Option<Instant>,
    anchor_mono: f64,
    anchor_out: f64,
    rate: f64,
    /// (monotonic s since origin, wall s since origin's wall) samples
    samples: VecDeque<(f64, f64)>,
    wall0: f64,
    /// the fitted wall line minus the mapped clock when the estimate (re)started: the offset the mapping keeps
    offset0: Option<f64>,
    last_sample: f64,
    last_out: f64,
    /// the latest least-squares slope (None before MIN_SPAN_S of history)
    estimate: Option<f64>,
    last_log: f64,
    warned: bool,
}

impl Default for Discipline {
    fn default() -> Self {
        Self::new()
    }
}

fn secs(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

impl Discipline {
    pub fn new() -> Self {
        Discipline {
            origin: None,
            anchor_mono: 0.0,
            anchor_out: 0.0,
            rate: 1.0,
            samples: VecDeque::new(),
            wall0: 0.0,
            offset0: None,
            last_sample: f64::NEG_INFINITY,
            last_out: 0.0,
            estimate: None,
            last_log: 0.0,
            warned: false,
        }
    }

    /// The rate the mapping applies now (match clock seconds per monotonic second).
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The wall clock's rate against the monotonic clock as last estimated (None before [`MIN_SPAN_S`] of history).
    pub fn estimate(&self) -> Option<f64> {
        self.estimate
    }

    fn out_at(&self, m: f64) -> f64 {
        self.anchor_out + (m - self.anchor_mono) * self.rate
    }

    /// Map the monotonic instant `mono` (read now) onto the match clock, with the wall clock `wall` read at the same moment.
    pub fn map(&mut self, mono: Instant, wall: SystemTime) -> Instant {
        let origin = *self.origin.get_or_insert(mono);
        if self.samples.is_empty() && self.last_sample == f64::NEG_INFINITY {
            self.wall0 = secs(wall);
        }
        let m = if mono >= origin { mono.duration_since(origin).as_secs_f64() } else { -origin.duration_since(mono).as_secs_f64() };
        if m - self.last_sample >= SAMPLE_EVERY_S {
            let dt = if self.last_sample.is_finite() { m - self.last_sample } else { 0.0 };
            self.last_sample = m;
            self.sample(m, secs(wall) - self.wall0, dt);
        }
        let out = self.out_at(m).max(self.last_out);
        self.last_out = out;
        origin + Duration::from_secs_f64(out.max(0.0))
    }

    fn fit(&self) -> Option<(f64, f64)> {
        let n = self.samples.len() as f64;
        let (first, last) = (self.samples.front()?, self.samples.back()?);
        if last.0 - first.0 < MIN_SPAN_S {
            return None;
        }
        let (sx, sy) = self.samples.iter().fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y));
        let (mx, my) = (sx / n, sy / n);
        let (sxx, sxy) = self.samples.iter().fold((0.0, 0.0), |(a, b), (x, y)| (a + (x - mx) * (x - mx), b + (x - mx) * (y - my)));
        if sxx <= 0.0 {
            return None;
        }
        let slope = sxy / sxx;
        Some((slope, my - slope * mx))
    }

    fn sample(&mut self, m: f64, w: f64, dt: f64) {
        // a wall-clock jump: the history starts over (the rate in use stays)
        if let Some((s, i)) = self.fit() {
            if (w - (i + s * m)).abs() > JUMP_S {
                log::warn!("match clock: the wall clock jumped by {:+.1} s against the monotonic clock: the rate estimate starts over", w - (i + s * m));
                self.samples.clear();
                self.offset0 = None;
                self.estimate = None;
            }
        }
        self.samples.push_back((m, w));
        while self.samples.front().is_some_and(|(x, _)| m - x > WINDOW_S) {
            self.samples.pop_front();
        }
        let Some((slope, icpt)) = self.fit() else { return };
        let slope = slope.clamp(RATE_MIN, RATE_MAX);
        self.estimate = Some(slope);
        let out = self.out_at(m);
        let line = icpt + slope * m;
        let offset0 = *self.offset0.get_or_insert(line - out);
        // keep the mapped clock at a fixed offset from the fitted wall line: the slope plus a small pull on the error
        let err = (line - offset0) - out;
        let target = slope + (err / OFFSET_TAU_S).clamp(-OFFSET_RATE_MAX, OFFSET_RATE_MAX);
        let step = MAX_SLEW_PER_S * dt.max(0.0);
        let rate = self.rate + (target - self.rate).clamp(-step, step);
        if rate != self.rate {
            self.anchor_out = out;
            self.anchor_mono = m;
            self.rate = rate;
        }
        if !self.warned && (slope - 1.0).abs() > 0.005 {
            self.warned = true;
            log::warn!(
                "match clock: the monotonic clock runs {:+.2} % against the wall clock (over {:.0} s; a VM clocksource?): the match clock follows the wall clock's rate",
                (1.0 / slope - 1.0) * 100.0,
                m - self.samples.front().map_or(m, |s| s.0)
            );
        }
        if m - self.last_log >= 60.0 {
            self.last_log = m;
            log::info!("match clock: wall rate {slope:.5} per monotonic s (window {:.0} s), applied rate {:.5}, offset error {:+.0} ms", m - self.samples.front().map_or(m, |s| s.0), self.rate, err * 1000.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A monotonic clock `slow` slower than the wall clock, whose realtime is stepped back to the wall every `period` s (the
    /// WSL sawtooth): the mapped clock runs at the wall's rate after the slew, never backwards, without jumps.
    fn run(slow: f64, period: f64, secs_total: u32) -> (Discipline, Vec<(f64, f64)>) {
        let mut d = Discipline::new();
        let t0 = Instant::now();
        let w0 = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let mut out = vec![];
        let mut realtime_err = 0.0f64;
        for k in 0..=secs_total * 20 {
            let wall = f64::from(k) * 0.05; // true seconds
            let mono = wall * (1.0 - slow);
            // the VM's realtime: runs with the monotonic clock, stepped to the wall every `period` s
            realtime_err = if (wall % period) < 0.05 { 0.0 } else { realtime_err + 0.05 * slow };
            let rt = wall - realtime_err;
            let o = d.map(t0 + Duration::from_secs_f64(mono), w0 + Duration::from_secs_f64(rt));
            out.push((wall, o.duration_since(t0).as_secs_f64()));
        }
        (d, out)
    }

    #[test]
    fn a_slow_monotonic_clock_is_brought_to_the_wall_rate_smoothly() {
        let (d, out) = run(0.065, 33.0, 900);
        assert!((d.rate() - 1.0 / 0.935).abs() < 0.002, "rate {}", d.rate());
        assert!(out.windows(2).all(|w| w[1].1 >= w[0].1), "never backwards");
        // no jump: 50 ms of wall time never maps to more than 60 ms or less than 40 ms of match clock
        assert!(out.windows(2).all(|w| (w[1].1 - w[0].1) < 0.06 && (w[1].1 - w[0].1) > 0.04 || w[0].0 < 1.0), "smooth");
        // after the slew the match clock keeps the wall's pace: over the last 5 minutes within 0.2 %
        let (a, b) = (out[out.len() - 6000], out[out.len() - 1]);
        let r = (b.1 - a.1) / (b.0 - a.0);
        assert!((r - 1.0).abs() < 0.002, "match clock per wall second {r}");
    }

    #[test]
    fn a_correct_monotonic_clock_maps_to_itself() {
        let (d, out) = run(0.0, 33.0, 300);
        assert!((d.rate() - 1.0).abs() < 1e-6);
        let last = out.last().unwrap();
        assert!((last.1 - last.0).abs() < 0.001, "identity: {last:?}");
    }

    #[test]
    fn a_wall_jump_restarts_the_estimate_and_keeps_the_rate() {
        let mut d = Discipline::new();
        let t0 = Instant::now();
        let w0 = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        for k in 0..120u64 {
            d.map(t0 + Duration::from_secs(k), w0 + Duration::from_secs(k));
        }
        let before = d.rate();
        // the wall clock is set one hour ahead
        for k in 120..200u64 {
            d.map(t0 + Duration::from_secs(k), w0 + Duration::from_secs(k + 3600));
        }
        assert!((d.rate() - before).abs() < 1e-9, "the jump is no rate");
        let o = d.map(t0 + Duration::from_secs(200), w0 + Duration::from_secs(3800));
        assert!((o.duration_since(t0).as_secs_f64() - 200.0).abs() < 0.01);
    }

    #[test]
    fn instants_before_the_last_one_never_go_back() {
        let mut d = Discipline::new();
        let t0 = Instant::now() + Duration::from_secs(10);
        let w = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let a = d.map(t0 + Duration::from_secs(5), w);
        let b = d.map(t0 + Duration::from_secs(4), w);
        assert!(b >= a);
    }
}
