//! The stand-in's stagger after a hit: the game's rule, from the weapons' data (A91 W3).
//!
//! Every bullet (pellet) that hits queues its weapon's `stopping_power` times the ammunition's `k_power` for
//! `stagger_duration` (0.5 s); the stagger update sums the queue, caps it at `max_stagger` (0.5), and takes that from
//! the victim's movement speed modifier: his speed is cut by that share, and he cannot sprint while it is not zero
//! (PROVEN, `base_player::hit_impl` 0x2D9B90, the update 0x2DBCD0, sprint-jump RE `FINDINGS.md` §1.1). A rifle bullet is
//! 0.1-0.6 (most 0.15-0.35), a shotgun pellet 0.8, a bolt rifle 0.4-0.8: one shotgun or bolt hit halves the speed, one
//! rifle hit takes 15-35 %. Before: every hit slowed to [`super::STAND_IN_STAGGER`] (0.75, a GUESS between) for
//! [`super::STAND_IN_STAGGER_MS`]. `SRP_BENCH_STAGGER_MODEL=off` is the old rule.
//!
//! The speed share is `1 - stagger` of a base of 1: the player's own modifiers (the weapon's +-0.05) are not in the
//! stand-in's speed, which is a share of the bot's walking speed.

use std::sync::OnceLock;

use super::standin_damage;
use super::*;

/// `stagger_duration`, ms (`default.player`'s `stagger_params`, PROVEN).
pub(crate) const DURATION_MS: u32 = 500;

/// `max_stagger`: the most the sum takes off the speed (PROVEN).
pub(crate) const MAX: f32 = 0.5;

/// The hits of the last [`DURATION_MS`]: when, and the stopping power each added.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Stagger {
    hits: Vec<(u32, f32)>,
}

impl Stagger {
    /// A bullet or pellet that hit at `tick` with `power` ([`power_of`]).
    pub(crate) fn hit(&mut self, tick: u32, power: f32) {
        self.hits.retain(|&(at, _)| tick < at + DURATION_MS);
        self.hits.push((tick, power));
    }

    /// The share taken off the speed at `tick`: the hits younger than [`DURATION_MS`] summed, at most [`MAX`].
    pub(crate) fn share(&self, tick: u32) -> f32 {
        self.hits
            .iter()
            .filter(|&&(at, _)| tick < at + DURATION_MS)
            .map(|&(_, power)| power)
            .sum::<f32>()
            .min(MAX)
    }

    pub(crate) fn reset(&mut self) {
        self.hits.clear();
    }
}

thread_local! {
    /// A test's word on the model, for its own thread.
    static FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// A test puts the model on or off for its own thread (`None`: as the run says).
pub(crate) fn force_model(on: Option<bool>) {
    FORCED.with(|f| f.set(on));
}

/// Whether the stand-in staggers by the game's rule (`SRP_BENCH_STAGGER_MODEL`: `on`, the default, or `off`).
pub(crate) fn model_on() -> bool {
    if let Some(on) = FORCED.with(std::cell::Cell::get) {
        return on;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAGGER_MODEL").as_deref() != Ok("off"))
}

/// What one bullet of `gun` adds: its weapon's `stopping_power` times its ammunition's `k_power`.
pub(crate) fn power_of(model: &standin_damage::Model, gun: &Gun) -> f32 {
    gun.stopping_power * standin_damage::ammo_of(model, gun).k_power
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::{damage, weapons};

    #[test]
    fn the_stagger_sums_the_last_half_second_and_stops_at_a_half() {
        let mut s = Stagger::default();
        assert_eq!(s.share(1000), 0.0);
        s.hit(1000, 0.15);
        assert!((s.share(1000) - 0.15).abs() < 1e-6);
        assert!((s.share(1499) - 0.15).abs() < 1e-6);
        assert_eq!(s.share(1500), 0.0, "0.5 s after, the hit is out");
        s.hit(1100, 0.3);
        assert!((s.share(1200) - 0.45).abs() < 1e-6);
        s.hit(1200, 0.3);
        assert_eq!(s.share(1250), MAX, "the sum is capped at max_stagger");
        // A hit within the window restarts it; the old ones fall out.
        assert!((s.share(1550) - 0.6f32.min(MAX)).abs() < 1e-6);
        assert!((s.share(1650) - 0.3).abs() < 1e-6, "only the hit of 1200 is left at 1650");
        s.reset();
        assert_eq!(s.share(1250), 0.0);
    }

    /// The powers the data gives the bots' weapons (sprint-jump RE: Uzi 0.30, AK-74U 0.15, TOZ-34 0.8 a pellet, Mosin
    /// 0.6, TT-33 0.1): one shotgun or bolt hit is the whole stagger, a rifle hit a third or less.
    #[test]
    fn the_stopping_power_is_read_from_the_weapons() {
        let power = |name: &str| weapons::weapon_named(name).unwrap().stopping_power;
        for (name, want) in [("uzi", 0.30), ("ak_74u", 0.15), ("toz_34", 0.8), ("mosin_nagant", 0.6), ("tt_33", 0.1)] {
            assert!((power(name) - want).abs() < 1e-5, "{name}: {}", power(name));
        }
        assert!(damage::ammo().iter().all(|a| (a.k_power - 1.0).abs() < 1e-6), "k_power is 1 for every stock ammunition");
    }
}
