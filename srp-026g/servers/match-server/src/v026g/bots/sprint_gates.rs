//! [2e, owner-14 row 15] What the client's keys cannot do, for the reckoned body ([`super::kin::SprintGates`],
//! [`super::kin::SpeedTerms::LegsStagger`]).
//!
//! The client (PROVEN, `sprint-jump-re/FINDINGS.md` 1.4 and 1.8): `base_player::can_sprint` 0x2DBF40 takes no sprint while the
//! hands' `can_sprint` (0x2E3A10) refuses (the aim transition not at exactly 0, so for the gun's `aim_time` after AIM goes up; the
//! weapon not idle: a reload) or a stagger is on (|player+0x300| > 1e-5 at 0x2DBFE5); a reload takes the sights down, so AIM held
//! through it does not slow the body. `hit_impl` 0x2D9B90 queues each hit's stopping speed; the update 0x2DBCD0 writes
//! -min(sum of the hits younger than `stagger_duration` 0.5 s, `max_stagger` 0.5) into the movement factor.
//!
//! The reckoning moved by keys and legs only: on the owner's 14 recordings 1,313 walking-class worlds over 0.2 m were a sprint
//! the client refused (911 a reload, 116 an aim-out), 2,615 AIM held through a reload, 2,086 a hit's stagger
//! (`drift-walking-rootcause.md` C, D, E; PROVEN). The bot knows its own reloads and its gun's aim time ([`OwnGates`], set by
//! the driver); every body's hits come with the worlds (`m_recent_hits`, the world's clock) ([`Stagger`]).

use crate::v026g::drive::key;

/// `stagger_duration`, ms (`default.player`'s `stagger_params`, PROVEN).
pub const STAGGER_MS: u32 = 500;
/// `max_stagger`: the most the hits take off the speed factor (PROVEN).
pub const STAGGER_MAX: f32 = 0.5;
/// Reloads kept, the newest last: the reckoning replays at most a few seconds of ticks.
const RELOADS_KEPT: usize = 4;
/// Hits kept, the newest last.
const HITS_KEPT: usize = 16;

/// A bot's own gates on its keys: its reloads (from the press to the magazine's end) and how long its sights take to come down.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OwnGates {
    /// (from, until) ms, the newest last.
    reloads: Vec<(u32, u32)>,
    /// The gun in hands' `aim_time`, ms: after AIM goes up the sprint waits this long.
    aim_out_ms: u32,
    /// [A364 P10] The bot's reckoned bar's low flag: `can_sprint` 0x2DBF40 refuses the sprint under it (PROVEN,
    /// `docs/notes/sprint_state.md`).
    low_stamina: bool,
}

impl OwnGates {
    /// A reload from `from` until `until` (ms); the same reload again changes nothing.
    pub fn reload(&mut self, from: u32, until: u32) {
        if until <= from {
            return;
        }
        if let Some(kept) = self.reloads.iter_mut().find(|kept| kept.0 == from) {
            kept.1 = until;
            return;
        }
        if self.reloads.len() >= RELOADS_KEPT && self.reloads.first().is_some_and(|oldest| from < oldest.0) {
            return;
        }
        self.reloads.push((from, until));
        self.reloads.sort_unstable_by_key(|kept| kept.0);
        if self.reloads.len() > RELOADS_KEPT {
            self.reloads.remove(0);
        }
    }

    pub fn set_aim_out_ms(&mut self, ms: u32) {
        self.aim_out_ms = ms;
    }

    pub fn set_low_stamina(&mut self, low: bool) {
        self.low_stamina = low;
    }

    pub fn aim_out_ms(&self) -> u32 {
        self.aim_out_ms
    }

    /// Whether a reload is under way at `time`.
    pub fn reloading(&self, time: u32) -> bool {
        self.reloads.iter().any(|&(from, until)| from <= time && time < until)
    }

    /// The keys of a tick at `time` as the client takes them: a reload has the sights down (no AIM) and no sprint; the low
    /// stamina flag no sprint.
    pub fn taken(&self, time: u32, keys: u32) -> u32 {
        let keys = if self.reloading(time) { keys & !(key::AIM | key::SPRINT) } else { keys };
        if self.low_stamina { keys & !key::SPRINT } else { keys }
    }
}

/// A body's hits as the worlds showed them: when (the world's ms) and each hit's stopping speed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stagger {
    hits: Vec<(u32, f32)>,
}

impl Stagger {
    /// A world's `m_recent_hits`: the list replaces what an older world showed (the client keeps its own queue).
    pub fn seen(&mut self, hits: &[(u32, f32)]) {
        self.hits.clear();
        let from = hits.len().saturating_sub(HITS_KEPT);
        self.hits.extend(hits[from..].iter().copied().filter(|&(_, power)| power.is_finite() && power > 0.0));
    }

    /// The share the hits take off the speed factor at `time`: the hits younger than [`STAGGER_MS`] summed, at most
    /// [`STAGGER_MAX`]. [F4] A hit stamped at `time` itself is not in it yet (0 < age < 500 ms): the client's stagger node
    /// takes a hit on the step after its stamp (exactness S5: 1,571 worlds where a hit of the world's own time was counted a
    /// tick early).
    pub fn share(&self, time: u32) -> f32 {
        self.hits
            .iter()
            .filter(|&&(at, _)| at < time && time < at + STAGGER_MS)
            .map(|&(_, power)| power)
            .sum::<f32>()
            .min(STAGGER_MAX)
    }

    /// Whether the client takes no sprint at `time` for a stagger.
    pub fn on(&self, time: u32) -> bool {
        self.share(time) > 1e-5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reload_takes_the_sights_and_the_sprint() {
        let mut gates = OwnGates::default();
        gates.reload(1_000, 3_000);
        gates.reload(1_000, 3_000);
        let keys = key::FORWARD | key::SPRINT | key::AIM;
        assert_eq!(gates.taken(999, keys), keys);
        assert_eq!(gates.taken(1_000, keys), key::FORWARD);
        assert_eq!(gates.taken(2_950, keys | key::FIRE), key::FORWARD | key::FIRE);
        assert_eq!(gates.taken(3_000, keys), keys);
        for k in 0..10 {
            gates.reload(10_000 + k * 5_000, 12_000 + k * 5_000);
        }
        assert!(!gates.reloading(1_500), "old reloads are let go of");
        assert!(gates.reloading(56_000));
    }

    /// The game's rule: the hits of the last half second summed, capped at a half (one shotgun pellet's 0.8 halves the speed).
    #[test]
    fn hits_stagger_for_half_a_second_capped_at_a_half() {
        let mut stagger = Stagger::default();
        stagger.seen(&[(1_000, 0.1), (1_200, 0.25), (1_300, f32::NAN)]);
        assert_eq!(stagger.share(999), 0.0);
        assert!((stagger.share(1_100) - 0.1).abs() < 1e-6);
        assert!((stagger.share(1_250) - 0.35).abs() < 1e-6);
        assert!((stagger.share(1_550) - 0.25).abs() < 1e-6);
        assert!(!stagger.on(1_700));
        stagger.seen(&[(5_000, 0.8)]);
        assert_eq!(stagger.share(5_100), STAGGER_MAX);
        assert_eq!(stagger.share(1_250), 0.0, "a new world's list replaces the old one");
    }
}
