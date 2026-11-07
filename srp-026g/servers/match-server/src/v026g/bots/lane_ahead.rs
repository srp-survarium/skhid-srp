//! [A317] The one "known lane ahead" test, for every crossing decision (the crossing smoke `smoke.crossing`, fx-fights'
//! `threat.check_spot`): the first point of a bot's way within a stretch ahead that a known spot's shooter SEES, tested from
//! the spot itself. Not from the team's covered regions: at the deaths to a farming camper they held no crossing ahead (48 %)
//! or held the bot already (39 %) where he killed (A308, `smoke_camper_gate_at_death`).
//!
//! A known spot is a held lane of the team (its own senses, callouts, the kill feed: `threat::ThreatSpots`) that killed
//! `kills` of its bots within the last `within_ms`, and whose shooter is taken to be there ([`Presence`]): sensed there within
//! a time (at most the fairness rule's 15 s), or [A308] remembered (the owner's area memory: two of ours died from there, and
//! the kill feed has not shown him dead since). A looser rule (any spot with one death, or sensed within 15 s) holds on almost
//! every route: in A317's first bench the crossing smoke fired about once a life (29,058 plans vs 6,104) and lost 41 % of the
//! wins to the stops.

use super::threat::HeldLane;
use mapdata::Vec3;

/// Whether a known spot's shooter is taken to be on it now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// The team sensed him there within this, ms.
    SensedWithin(u32),
    /// [A308] The spot is remembered while it lives (it decays as `threat::DECAY_PER_S`) and he is not known dead since its
    /// last sense: the kill feed of his death clears the spot's sense (`ThreatSpots`' shooter death), and a dead man's spot is
    /// not his until he is sensed on it again. The coordinator's ruling (2026-11-07): a human remembers "2 of us died from that
    /// window", as the owner's area memory for routes (`threat.sniper_memory`).
    Remembered,
}

/// What makes a held lane known for a crossing decision: a farming spot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Known {
    /// This many of the team's deaths there (at most 3: the times a lane keeps),
    pub kills: usize,
    /// within this of now, ms,
    pub within_ms: u32,
    /// and its shooter taken to be there.
    pub presence: Presence,
}

impl Known {
    /// The farming rule: 2 of our deaths within 60 s, the shooter sensed within 15 s.
    pub const FARMING: Self = Self { kills: 2, within_ms: 60_000, presence: Presence::SensedWithin(15_000) };

    pub fn holds(self, lane: &HeldLane, clock: u32) -> bool {
        let recent = lane.latest_kills.iter().flatten().filter(|&&k| clock.saturating_sub(k) <= self.within_ms).count();
        let there = match self.presence {
            Presence::SensedWithin(ms) => lane.sensed.is_some_and(|s| clock.saturating_sub(s) <= ms),
            Presence::Remembered => lane.sensed.is_some(),
        };
        recent >= self.kills && there
    }
}

/// A known lane the way enters: the lane, how far along the way (from its start) and where, and whether the bot stands in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneAhead<'a> {
    pub lane: &'a HeldLane,
    pub crossing_s: f32,
    pub crossing: Vec3,
    pub in_line: bool,
}

/// The eye over a spot and the chest over a floor point, m.
pub const EYE_M: f32 = 1.6;
pub const CHEST_M: f32 = 1.3;

/// The nearest-ahead known lane the way enters: `at(s)` is the way's point `s` metres from its start, sampled every `step_m`
/// from `from_s` to `to_s`; `feet` is the bot's (in the line when a known spot sees it); `sees(eye, point)` the line test. A
/// lane counts within its reach only.
#[allow(clippy::too_many_arguments)]
pub fn known_lane_ahead<'a>(
    lanes: &'a [HeldLane],
    known: Known,
    clock: u32,
    feet: Vec3,
    at: impl Fn(f32) -> Vec3,
    (from_s, to_s, step_m): (f32, f32, f32),
    sees: impl Fn(Vec3, Vec3) -> bool,
) -> Option<LaneAhead<'a>> {
    let up = |h: f32| Vec3::new(0.0, h, 0.0);
    let sees_point = |l: &HeldLane, p: Vec3| l.at.distance_xz(p) <= l.reach_m && sees(l.at + up(EYE_M), p + up(CHEST_M));
    let known_lanes: Vec<&HeldLane> = lanes.iter().filter(|l| known.holds(l, clock)).collect();
    if let Some(lane) = known_lanes.iter().copied().find(|l| sees_point(l, feet)) {
        return Some(LaneAhead { lane, crossing_s: from_s, crossing: feet, in_line: true });
    }
    let steps = ((to_s - from_s) / step_m).floor().max(0.0) as usize;
    (0..=steps).map(|k| from_s + k as f32 * step_m).find_map(|s| {
        let p = at(s);
        known_lanes.iter().copied().find(|l| sees_point(l, p)).map(|lane| LaneAhead { lane, crossing_s: s, crossing: p, in_line: false })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lane(kills: &[u32], sensed: Option<u32>) -> HeldLane {
        let mut latest = [None; 3];
        for (slot, &t) in latest.iter_mut().zip(kills) {
            *slot = Some(t);
        }
        HeldLane { at: Vec3::new(20.0, 0.0, 20.0), shooter: 7, reach_m: 60.0, sensed, area_m: 5.0, kills: kills.len(), latest_kills: latest }
    }

    /// A farming spot (2 deaths within 60 s, sensed within 15 s) is known; one death, old deaths or a stale sense are not.
    #[test]
    fn a_farming_spot_is_known_and_a_passing_one_is_not() {
        let k = Known::FARMING;
        let now = 100_000;
        assert!(k.holds(&lane(&[95_000, 70_000], Some(99_000)), now));
        assert!(!k.holds(&lane(&[95_000], Some(99_000)), now), "one death");
        assert!(!k.holds(&lane(&[95_000, 30_000], Some(99_000)), now), "the second is older than 60 s");
        assert!(!k.holds(&lane(&[95_000, 70_000], Some(80_000)), now), "not sensed within 15 s");
    }

    /// The first point ahead a known spot sees is the crossing; in his sight already is in line.
    #[test]
    fn the_first_point_a_known_spot_sees_is_the_crossing() {
        let now = 100_000;
        // A wall hides z < 10 from the spot at (20, 0, 20).
        let sees = |_: Vec3, p: Vec3| p.z >= 10.0;
        let at = |s: f32| Vec3::new(0.0, 0.0, s);
        let spot = [lane(&[95_000, 70_000], Some(99_000))];
        let ahead = known_lane_ahead(&spot, Known::FARMING, now, at(0.0), at, (0.0, 25.0, 2.0), sees).expect("a crossing");
        assert!(!ahead.in_line);
        assert_eq!(ahead.crossing_s, 10.0);
        let passing = [lane(&[95_000], Some(99_000))];
        assert_eq!(known_lane_ahead(&passing, Known::FARMING, now, at(0.0), at, (0.0, 25.0, 2.0), sees), None);
        let ahead = known_lane_ahead(&spot, Known::FARMING, now, at(12.0), at, (12.0, 25.0, 2.0), sees).expect("in line");
        assert!(ahead.in_line);
    }

    /// [A308 `smoke.threat_lane`] The known camper's rule: two deaths within 5 min, the spot remembered while its shooter is not
    /// known dead (no sense needed); one death, a death out of the window, or his death since (the sense cleared) is not. Sensed
    /// within 15 s instead, a stale sense does not count.
    #[test]
    fn a_known_camper_is_two_deaths_at_a_remembered_spot() {
        let k = crate::v026g::bots::tuning::ThreatLane::default().known();
        let now = 400_000;
        assert!(k.holds(&lane(&[390_000, 150_000], Some(300_000)), now), "two deaths in 5 min, sensed 100 s ago");
        assert!(!k.holds(&lane(&[390_000], Some(399_000)), now), "one death");
        assert!(!k.holds(&lane(&[390_000, 90_000], Some(399_000)), now), "the second death is older than 5 min");
        assert!(!k.holds(&lane(&[390_000, 150_000], None), now), "he died since: the feed cleared the sense");
        let sensed = Known { presence: Presence::SensedWithin(15_000), ..k };
        assert!(sensed.holds(&lane(&[390_000, 150_000], Some(386_000)), now), "sensed 14 s ago");
        assert!(!sensed.holds(&lane(&[390_000, 150_000], Some(384_000)), now), "sensed 16 s ago");
    }
}
