//! An error in the bots' picture of bodies (`SRP_BENCH_VIEW_ERROR=1`, an-sight §8.1, the cheap stand-in for A127).
//!
//! In the bench every body moves by the model the bots reckon it with, so a bot's picture of another body is exact; in the
//! game the picture is reckoned from a world fix that is never under 0.5 s old (median 1.16 s) with the bots' movement model
//! on a body the client moves by its own physics, and is 0.2-0.4 m off (median) and 1-3 m at the tails (stairs, drops, jumps,
//! stops). That makes the bots' picture say "hidden" in 22 % of the moments a body is truly visible and "visible" in 20 % of
//! the moments it is truly hidden (the flip test, LIKELY: an-sight §8.1, 16,365 pairs of the owner's 8 matches, "15-25 %", chest
//! and head; the bench samples the chest only and each bot's 2 nearest enemies, so the target and so the scale are LIKELY, not
//! PROVEN). The model's tails are drawn for every fix of a still body; the table's "just after a stop or start" tail (p90 1.05 m,
//! p99 2.9 m) is therefore over-drawn for a body still for minutes, which the scale absorbs (LIKELY): a recalibration should draw
//! near zero after about a second still.
//!
//! The model: every world fix a bot is given is moved by an error drawn once for the fix from the measured tables, by what the
//! body was doing and how old the fix will be when the next one replaces it; the reckoning then carries the error along until
//! the next fix, which is how the live error behaves (a picture off by the fix's age of movement). The stand-in's own hits go by
//! the true bodies, so a shot at the picture goes where the picture is. The error applies to every body, the bots' own too
//! (the table's bots rows, median 0.36 m).
//!
//! Off by default: no arm has it until the owner of the bench scripts turns it on. Calibrated on 9 matches a side (School and
//! Cologne batteries, Vector Laboratory TDM, 10 a side, tier 13-15, with the live pings) against the flip counts of
//! `tallies/owner/outcomes.rs` (`view_flip`), live 22 % missed sight / 20 % fire into cover:
//!
//! | arm | missed sight | fire into cover |
//! |---|---|---|
//! | no error (the pings alone) | 8.6 % | 9.1 % (seeds 1-3; 9.3 % / 8.8 % on seeds 4-6) |
//! | scale 1.0 | 24.6 % | 23.6 % |
//! | scale 0.75 | 23.0 % | 21.7 % |
//! | scale 0.6 | 21.5 % | 19.0 % |
//!
//! The bench's pings already give it a ninth of those; the tables' errors include the lateness, so the whole table over-counts.
//! The default is [`DEFAULT_SCALE`] (0.7), checked on seeds 4-6 (not used to set it): 21.9 % / 20.8 %.

use mapdata::Vec3;

use super::*;
use crate::v026g::bots::body::Fix;
use crate::v026g::drive::key;

/// What the body was doing at the fix (the table's rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Moving {
    Still,
    Walking,
    Sprinting,
    CrouchedMove,
    /// A height change over 0.4 m since the last fix (stairs, drops).
    Height,
    Jumping,
}

/// The quantiles of the error, m: median, 75th, 90th, 99th percentile (an-sight §8.1: the owner's rows, "exact" samples).
fn quantiles(moving: Moving) -> [f32; 4] {
    match moving {
        Moving::Still => [0.01, 0.19, 1.05, 2.9],
        Moving::Walking => [0.36, 0.57, 0.85, 2.6],
        Moving::Sprinting => [0.43, 0.65, 0.90, 1.2],
        Moving::CrouchedMove => [0.23, 0.36, 0.50, 1.3],
        Moving::Height => [0.29, 0.62, 1.32, 3.3],
        Moving::Jumping => [0.76, 1.96, 2.40, 3.1],
    }
}

/// The error at the draw `u` in [0, 1): the inverse of the table's distribution, straight between its points, the last one
/// a fifth past the 99th percentile.
pub(crate) fn magnitude(moving: Moving, u: f32) -> f32 {
    let [p50, p75, p90, p99] = quantiles(moving);
    let points = [(0.0, 0.0), (0.5, p50), (0.75, p75), (0.9, p90), (0.99, p99), (1.0, 1.2 * p99)];
    points
        .windows(2)
        .find(|w| u <= w[1].0)
        .map_or(points[5].1, |w| w[0].1 + (w[1].1 - w[0].1) * (u - w[0].0) / (w[1].0 - w[0].0))
}

/// By the age the fix will have, the 90th percentile of the error over the overall 0.93 m (an-sight §8.1: 0.5-0.8 s 0.68,
/// 0.8-1.1 s 0.83, 1.1-2 s 1.04). The error is flat in the age but for this slow rise.
pub(crate) fn age_factor(age_ms: u32) -> f32 {
    let p90 = match age_ms {
        0..=799 => 0.68,
        800..=1_099 => 0.83,
        _ => 1.04,
    };
    p90 / 0.93
}

/// The scale that puts the flip counts at the live 22 % / 20 % with the bench's pings on (see the module docs).
pub(crate) const DEFAULT_SCALE: f32 = 0.7;

/// The error model of a run.
pub(crate) struct ViewError {
    rng: Rng,
    /// A multiplier on every error (`SRP_BENCH_VIEW_ERROR_SCALE`).
    scale: f32,
    /// By place: the height of the last fix.
    last_y: Vec<Option<f32>>,
    /// Fixes moved, and the sum of how far, m.
    pub(crate) fixes: u32,
    pub(crate) metres: f64,
}

impl ViewError {
    /// On with `SRP_BENCH_VIEW_ERROR=1`; none otherwise.
    pub(crate) fn from_env(places: usize, seed: u32) -> Option<Self> {
        let settings = bench_settings();
        let on = match settings.var("SRP_BENCH_VIEW_ERROR").ok().as_deref() {
            None | Some("0") => false,
            Some("1") => true,
            Some(other) => panic!("SRP_BENCH_VIEW_ERROR={other:?}: 1 or unset"),
        };
        on.then(|| Self {
            rng: Rng::new(0x71E3_0000 + u64::from(seed)),
            scale: settings.number_or_refuse("SRP_BENCH_VIEW_ERROR_SCALE").unwrap_or(DEFAULT_SCALE),
            last_y: vec![None; places],
            fixes: 0,
            metres: 0.0,
        })
    }

    /// What the fix says the body was doing.
    fn moving(&self, place: usize, fix: &Fix) -> Moving {
        let height = match (self.last_y.get(place).copied().flatten(), fix.feet) {
            (Some(last), Some(feet)) => (feet.y - last).abs() > 0.4,
            _ => false,
        };
        let moves = fix.keys & (key::FORWARD | key::BACK | key::LEFT | key::RIGHT) != 0 || fix.speed.is_some_and(|s| s > 0.3);
        if fix.keys & key::JUMP != 0 {
            Moving::Jumping
        } else if height {
            Moving::Height
        } else if !moves {
            Moving::Still
        } else if fix.keys & key::CROUCH != 0 {
            Moving::CrouchedMove
        } else if fix.keys & key::SPRINT != 0 {
            Moving::Sprinting
        } else {
            Moving::Walking
        }
    }

    /// `fix` with its place moved by the error of the body's state, in a direction drawn evenly round; a dead body's fix is
    /// as it was. `period_ms` is how often fixes come (the next replaces this one then).
    pub(crate) fn perturb(&mut self, place: usize, mut fix: Fix, clock: u32, period_ms: u32) -> Fix {
        let Some(feet) = fix.feet else {
            if let Some(slot) = self.last_y.get_mut(place) {
                *slot = None;
            }
            return fix;
        };
        let moving = self.moving(place, &fix);
        if let Some(slot) = self.last_y.get_mut(place) {
            *slot = Some(feet.y);
        }
        let age = clock.saturating_sub(fix.time) + period_ms / 2;
        let m = self.scale * age_factor(age) * magnitude(moving, self.rng.unit());
        let angle = self.rng.range(0.0, std::f32::consts::TAU);
        fix.feet = Some(feet + Vec3::new(m * angle.cos(), 0.0, m * angle.sin()));
        self.fixes += 1;
        self.metres += f64::from(m);
        fix
    }

    pub(crate) fn line(&self) -> String {
        format!(
            "view error: {} fixes moved, {:.2} m on average",
            self.fixes,
            if self.fixes > 0 { self.metres / f64::from(self.fixes) } else { 0.0 }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inverse distribution passes through the table's points and is steady between them.
    #[test]
    fn the_magnitude_follows_the_measured_quantiles() {
        for moving in [Moving::Still, Moving::Walking, Moving::Sprinting, Moving::CrouchedMove, Moving::Height, Moving::Jumping] {
            let [p50, p75, p90, p99] = quantiles(moving);
            assert!((magnitude(moving, 0.0)).abs() < 1e-6);
            assert!((magnitude(moving, 0.5) - p50).abs() < 1e-5, "{moving:?}");
            assert!((magnitude(moving, 0.75) - p75).abs() < 1e-5);
            assert!((magnitude(moving, 0.9) - p90).abs() < 1e-5);
            assert!((magnitude(moving, 0.99) - p99).abs() < 1e-5);
            assert!((magnitude(moving, 1.0) - 1.2 * p99).abs() < 1e-5);
            let mut last = 0.0;
            for k in 0..=100 {
                let m = magnitude(moving, k as f32 / 100.0);
                assert!(m >= last, "{moving:?} at {k}");
                last = m;
            }
        }
    }

    /// A fix is moved by a draw of the table, the dead are left, and the state comes from the keys, the speed and the height.
    #[test]
    fn a_fix_is_moved_by_its_states_error() {
        let fix = |keys: u32, y: f32| Fix {
            time: 1_000,
            alive: true,
            inserted: 1,
            feet: Some(Vec3::new(10.0, y, 10.0)),
            yaw: 0.0,
            pitch: 0.0,
            keys,
            speed: None,
        };
        let mut error = ViewError { rng: Rng::new(7), scale: 1.0, last_y: vec![None; 2], fixes: 0, metres: 0.0 };
        assert_eq!(error.moving(0, &fix(0, 0.0)), Moving::Still);
        assert_eq!(error.moving(0, &fix(key::FORWARD, 0.0)), Moving::Walking);
        assert_eq!(error.moving(0, &fix(key::FORWARD | key::SPRINT, 0.0)), Moving::Sprinting);
        assert_eq!(error.moving(0, &fix(key::FORWARD | key::CROUCH, 0.0)), Moving::CrouchedMove);
        assert_eq!(error.moving(0, &fix(key::JUMP | key::FORWARD, 0.0)), Moving::Jumping);
        // The height since the last fix.
        error.last_y[0] = Some(0.0);
        assert_eq!(error.moving(0, &fix(0, 1.0)), Moving::Height);
        // 4,000 walking fixes: the median is the table's, 0.36 m (the age factor on it, 0.89).
        let mut distances: Vec<f32> = (0..4_000)
            .map(|_| {
                let moved = error.perturb(1, fix(key::FORWARD, 0.0), 1_300, 1_000);
                moved.feet.expect("alive").distance_xz(Vec3::new(10.0, 0.0, 10.0))
            })
            .collect();
        distances.sort_by(f32::total_cmp);
        let median = distances[2_000];
        assert!((0.28..=0.36).contains(&median), "{median}");
        let dead = Fix { feet: None, alive: false, ..fix(0, 0.0) };
        assert_eq!(error.perturb(1, dead.clone(), 1_300, 1_000), dead);
    }
}
