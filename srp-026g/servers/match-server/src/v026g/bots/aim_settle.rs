//! The aim settling with time on target, and the deliberate shot (A182 #29, the far shots).
//!
//! A player who keeps the crosshair on a target gets steadier, and waits for it before a shot that must count: the humans'
//! recordings hold a bolt action's first shot a median 0.95 / 1.6 / 2.5 / 3.35 s after the aim went down at 0-20 / 20-40 /
//! 40-80 / 80+ m (the VSK-94 0.4 / 1.05 / 2.75 / 3.75 s), the longer the farther. The bots' wander is stationary (`wander_deg`
//! at a tick and at ten seconds), so waiting gains them nothing. Two parts, both off in the game ([`Settle`]):
//!
//! - **the wander settles**: its spread goes from the skill's `wander_deg` at the moment the target is taken to the skill's
//!   `settled_wander_deg` with the time constant `settle_tau_s` (a pair each, skill 0 to 1: the dial; skill 1 settles to about
//!   nothing, a lower skill keeps more);
//! - **the shot policy**: a shot (or the burst's first) waits while the error to expect, with the zoom of the sights in, is
//!   larger than `settle_k` times the angular half width of the body at that distance, up to `settle_max_wait_ms` after the target
//!   was taken. One rule at every range: the waits grow with the range by themselves.
//!
//! The policy goes by the expected error ([`error_rms_deg`], what a player knows of his own hand), not by the draw of the tick.

use serde::{Deserialize, Serialize};
use survarium::v026g::weapons::Optic;

/// Whether the aim settles and the shots wait for it (a bench setting to size the effect; off in the game).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Settle {
    /// The wander is the skill's whatever the time on target, shots do not wait (today).
    #[default]
    Off,
    /// The wander settles with time on target.
    Wander,
    /// The wander settles, and a shot waits for the error to be small enough for the target's size at his distance.
    WanderAndPolicy,
}

impl Settle {
    pub fn settles(self) -> bool {
        self != Self::Off
    }

    pub fn holds_fire(self) -> bool {
        self == Self::WanderAndPolicy
    }
}

/// The guns it applies to: the ones with a reference in the recordings (the bolt actions and the marksman rifles with a scope: the
/// humans wait the longer the farther), or every gun (an automatic rifle's first shot waits for no body width the humans were
/// measured to: A182 #29 bench, tier 9-10, delays them past the cap and lifts their hit rate over the tier bar).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettleGuns {
    #[default]
    Scoped,
    All,
}

impl SettleGuns {
    /// `settle` for a gun with these sights.
    pub fn settle(self, settle: Settle, optic: Optic) -> Settle {
        match (self, optic) {
            (Self::Scoped, Optic::Iron { .. }) => Settle::Off,
            _ => settle,
        }
    }
}

/// The wander's spread, degrees (standard deviation, yaw), `t_s` seconds after the target was taken: `wander` today and at the
/// moment he is taken, `settled` (at most `wander`) with the time constant `tau_s` later.
pub fn wander_deg(settle: Settle, wander: f32, settled: f32, tau_s: f32, t_s: f32) -> f32 {
    if !settle.settles() {
        return wander;
    }
    let settled = settled.clamp(0.0, wander);
    settled + (wander - settled) * (-t_s.max(0.0) / tau_s.max(0.05)).exp()
}

/// The error to expect at the shot, degrees (yaw rms): the start error `U(-3e, 3e)` faded by `fade`, the wander, the floor
/// `U(-e/2, e/2)` (`e` the skill's `error_deg`), all times the sights' zoom.
pub fn error_rms_deg(error_deg: f32, fade: f32, wander_deg: f32, zoom: f32) -> f32 {
    let start = 3.0 * error_deg / 3f32.sqrt() * fade;
    let floor = error_deg / 12f32.sqrt();
    (start * start + wander_deg * wander_deg + floor * floor).sqrt() * zoom
}

/// Whether a shot at a target `distance` m away waits: the policy is on, the error to expect is larger than `k` times the angular half
/// width of a body of half width `half_m` there, and the target has been taken for less than `max_wait_ms`.
pub fn waits(settle: Settle, rms_deg: f32, distance: f32, half_m: f32, k: f32, waited_ms: u32, max_wait_ms: u32) -> bool {
    settle.holds_fire()
        && waited_ms < max_wait_ms
        && rms_deg > k * half_m.atan2(distance.max(0.5)).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Off, the wander is the skill's at every time; on, it starts there and settles to the floor with the time constant, and a floor
    /// above the wander is the wander.
    #[test]
    fn the_wander_settles_from_its_value_to_the_floor() {
        assert_eq!(wander_deg(Settle::Off, 1.35, 0.0, 1.5, 9.0), 1.35);
        assert!((wander_deg(Settle::Wander, 1.35, 0.3, 1.5, 0.0) - 1.35).abs() < 1e-6);
        let after_tau = wander_deg(Settle::Wander, 1.35, 0.3, 1.5, 1.5);
        assert!((after_tau - (0.3 + 1.05 / std::f32::consts::E)).abs() < 1e-5, "{after_tau}");
        assert!((wander_deg(Settle::WanderAndPolicy, 1.35, 0.3, 1.5, 60.0) - 0.3).abs() < 1e-3);
        assert_eq!(wander_deg(Settle::Wander, 0.8, 2.0, 1.0, 5.0), 0.8, "a floor above the wander is no settling");
    }

    /// The error to expect is the three parts in quadrature times the zoom.
    #[test]
    fn the_expected_error_is_the_parts_in_quadrature_times_the_zoom() {
        // Skill 0.75 at 0.45 s: e = 2.175 degrees, fade exp(-0.45 / 0.1625), wander 1.35: start 0.24, wander 1.35, floor 0.63.
        let fade = (-0.45f32 / 0.1625).exp();
        let rms = error_rms_deg(2.175, fade, 1.35, 1.0);
        assert!((rms - 1.507).abs() < 0.01, "{rms}");
        assert!((error_rms_deg(2.175, fade, 1.35, 0.25) - rms * 0.25).abs() < 1e-6);
    }

    /// Scoped: only a gun with a scope settles; All: every gun.
    #[test]
    fn the_settling_applies_to_the_guns_its_switch_names() {
        let iron = Optic::Iron { zoom: 0.909 };
        let scope = Optic::Scope { zoom: 0.25 };
        assert_eq!(SettleGuns::Scoped.settle(Settle::WanderAndPolicy, iron), Settle::Off);
        assert_eq!(SettleGuns::Scoped.settle(Settle::WanderAndPolicy, scope), Settle::WanderAndPolicy);
        assert_eq!(SettleGuns::All.settle(Settle::Wander, iron), Settle::Wander);
        assert_eq!(SettleGuns::Scoped.settle(Settle::Off, scope), Settle::Off);
    }

    /// The same rule at every range: an error to expect of 1.2 degrees fits the body at 10 m (its half width is 1.43 degrees
    /// there) and not at 60 m (0.24), where the shot waits until the error has settled, up to the cap, and with the policy
    /// off none waits.
    #[test]
    fn a_shot_waits_for_the_error_to_fit_the_body_at_its_distance() {
        let half = 0.25;
        let near = waits(Settle::WanderAndPolicy, 1.2, 10.0, half, 1.0, 450, 5_000);
        let far = waits(Settle::WanderAndPolicy, 1.2, 60.0, half, 1.0, 450, 5_000);
        assert!(!near && far, "near {near} far {far}");
        assert!(!waits(Settle::WanderAndPolicy, 0.18, 60.0, half, 1.0, 3_000, 5_000), "settled");
        assert!(!waits(Settle::WanderAndPolicy, 1.2, 60.0, half, 1.0, 5_000, 5_000), "the cap");
        assert!(waits(Settle::WanderAndPolicy, 0.3, 60.0, half, 1.0, 450, 5_000));
        assert!(!waits(Settle::WanderAndPolicy, 0.3, 60.0, half, 2.0, 450, 5_000), "a larger k lets it go");
        assert!(!waits(Settle::Wander, 1.2, 60.0, half, 1.0, 450, 5_000), "the policy is off");
        assert!(!waits(Settle::Off, 1.2, 60.0, half, 1.0, 450, 5_000));
    }
}
