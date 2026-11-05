//! [A317] The one "known lane ahead" test, for every crossing decision (the crossing smoke `smoke.crossing`, and fx-fights'
//! `threat.check_spot`): the first point of a bot's way within reach ahead that a known spot's shooter SEES, tested from the
//! spot itself. Not from the team's covered regions: at the deaths to a farming camper they held no crossing ahead (48 %) or
//! held the bot already (39 %) where he killed (A308, `smoke_camper_gate_at_death`).
//!
//! A known spot is a held lane of the team (its own senses, callouts, the kill feed: `threat::ThreatSpots`) that counts
//! `kills` of its deaths, or whose shooter it sensed there within `sensed_ms` (at most the fairness rule's 15 s).

use super::threat::HeldLane;
use mapdata::Vec3;

/// What makes a held lane known for a crossing decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Known {
    /// Of the team's deaths it counts, at least.
    pub kills: usize,
    /// Or its shooter sensed there within this, ms.
    pub sensed_ms: u32,
}

impl Known {
    pub fn holds(self, lane: &HeldLane, clock: u32) -> bool {
        lane.kills >= self.kills || lane.sensed.is_some_and(|s| clock.saturating_sub(s) <= self.sensed_ms)
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
/// from `from_s` to `to_s`; `feet` is the bot's; `sees(eye, point)` the line test. A lane counts within its reach only.
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
    (1..=steps).map(|k| from_s + k as f32 * step_m).find_map(|s| {
        let p = at(s);
        known_lanes.iter().copied().find(|l| sees_point(l, p)).map(|lane| LaneAhead { lane, crossing_s: s, crossing: p, in_line: false })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lane(at: Vec3, kills: usize, sensed: Option<u32>) -> HeldLane {
        HeldLane { at, shooter: 7, reach_m: 60.0, sensed, area_m: 5.0, kills }
    }

    /// The first point ahead a known spot sees is the crossing; an unknown spot (no kill, not sensed lately) is passed by.
    #[test]
    fn the_first_point_a_known_spot_sees_is_the_crossing() {
        let known = Known { kills: 1, sensed_ms: 15_000 };
        // A wall hides z < 10 from the spot at (20, 0, 20).
        let sees = |_: Vec3, p: Vec3| p.z >= 10.0;
        let at = |s: f32| Vec3::new(0.0, 0.0, s);
        let spot = [lane(Vec3::new(20.0, 0.0, 20.0), 1, None)];
        let ahead = known_lane_ahead(&spot, known, 100_000, at(0.0), at, (0.0, 25.0, 2.0), sees).expect("a crossing");
        assert!(!ahead.in_line);
        assert_eq!(ahead.crossing_s, 10.0);
        let unknown = [lane(Vec3::new(20.0, 0.0, 20.0), 0, Some(10_000))];
        assert_eq!(known_lane_ahead(&unknown, known, 100_000, at(0.0), at, (0.0, 25.0, 2.0), sees), None);
        // In his line already.
        let ahead = known_lane_ahead(&spot, known, 100_000, at(12.0), at, (12.0, 25.0, 2.0), sees).expect("in line");
        assert!(ahead.in_line);
    }
}
