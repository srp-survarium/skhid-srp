//! A bot that is stuck: it walks, and its body does not move.
//!
//! The walk grid is near the game's walking but not the same: a body may
//! catch on a ledge the grid thinks is a step, a door narrower than the
//! grid's, a crate the walker mesh lacks, or another player. The bot
//! cannot feel it: it sees its body only in the worlds it is given (one a
//! second or two while a bot needs one). So it is stuck when two worlds
//! [`STUCK_AFTER_MS`] apart show its body moved less than
//! [`STUCK_MOVED_M`] while its own keys, reckoned on the walk grid
//! ([`super::body::BodyTrack::last_meant`]), would have moved it at least
//! [`MEANT_M`]: 1.5 to 3 s after it caught, about when a player would
//! notice. A bot that stood to aim, crept, or waited for its search with a
//! route in hand meant to go nowhere, and is not stuck (the live logs of
//! 2026-10-17..23 had a third of their stuck lines with nothing within
//! reach of the body: `player_bots_stuck.md`).
//!
//! What it does then, more each time at the same spot (within
//! [`SPOT_M`] in [`AGAIN_MS`]), as a player would:
//! 1. [`Recovery::BackOff`]: a jump, a short step back and to a side, and
//!    the same route planned again from there;
//! 2. [`Recovery::Sidestep`]: a jump and a longer step to a side, and
//!    another goal;
//! 3. [`Recovery::GiveUp`]: a step back and to a side (each time again
//!    there the other way round: [`Stuck::escape`]), it stands and looks
//!    about, then goes anywhere else.
//!
//! A spot it got stuck at twice is kept for [`SPOT_KEPT_MS`]: a new route
//! that passes near it is not taken ([`Stuck::clear`]), so it does not
//! walk into the same trap again. Once is not enough: the spot may be the
//! only door to the other side, and a body caught there once by another
//! player gets through the next time; and where every route passes a spot,
//! the bot takes one anyway (`Bot::choose`).

use mapdata::Vec3;

use super::path::Route;
use crate::v026g::drive::key;

/// Worlds this far apart in time…
pub const STUCK_AFTER_MS: u32 = 1_500;
/// … that show the body moved less than this while walking: stuck.
pub const STUCK_MOVED_M: f32 = 0.4;
/// … while its keys meant to move it at least this far, metres (a walk
/// of 1.5 s is 5 m, a crouched one 2.5 m).
pub const MEANT_M: f32 = 1.2;
/// A stuck spot this near an earlier one is the same spot, metres.
pub const SPOT_M: f32 = 2.0;
/// … if the earlier one was within this, ms.
pub const AGAIN_MS: u32 = 20_000;
/// A spot is avoided this long, ms.
pub const SPOT_KEPT_MS: u32 = 45_000;
/// A spot is avoided once it caught the body this many times.
const AVOID_AFTER: u32 = 2;
/// A route passing this near a kept spot is not taken (its first metres,
/// where the body is now, are not looked at), metres.
const ROUTE_CLEAR_M: f32 = 1.2;
const ROUTE_FROM_M: f32 = SPOT_M + 0.5;

/// What a stuck bot does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    BackOff,
    Sidestep,
    GiveUp,
}

#[derive(Clone, Debug, Default)]
pub struct Stuck {
    /// The last world's feet and time while walking, and how far its keys
    /// meant to move it since (none: not known).
    walked_from: Option<(u32, Vec3, Option<f32>)>,
    /// Where it got stuck, the last time, and how often there.
    spots: Vec<Spot>,
    /// How often at the spot of the last time.
    last_times: u32,
}

#[derive(Clone, Copy, Debug)]
struct Spot {
    at: Vec3,
    last: u32,
    times: u32,
}

impl Stuck {
    /// It stopped walking (a new route, a pause, a death): the next world
    /// starts the watch again.
    pub fn reset(&mut self) {
        self.walked_from = None;
    }

    /// A world showed the body at `feet` while it had a route since the
    /// last one, and its keys meant to move it `meant` metres between the
    /// two worlds (none: not known, as at a life's first world; then it is
    /// taken as walking). `Some` when it is stuck: what to do about it.
    pub fn walking(&mut self, time: u32, feet: Vec3, meant: Option<f32>) -> Option<Recovery> {
        let Some((at, from, meant_since)) = self.walked_from else {
            self.walked_from = Some((time, feet, Some(0.0)));
            return None;
        };
        let meant_since = meant_since.zip(meant).map(|(a, b)| a + b);
        if time < at + STUCK_AFTER_MS {
            self.walked_from = Some((at, from, meant_since));
            return None;
        }
        self.walked_from = Some((time, feet, Some(0.0)));
        if from.distance_xz(feet) >= STUCK_MOVED_M || meant_since.is_some_and(|m| m < MEANT_M) {
            return None;
        }
        self.spots.retain(|spot| spot.last + SPOT_KEPT_MS > time);
        self.walked_from = None;
        let times = match self
            .spots
            .iter_mut()
            .find(|spot| spot.last + AGAIN_MS > time && spot.at.distance_xz(feet) < SPOT_M)
        {
            Some(spot) => {
                spot.last = time;
                spot.times += 1;
                spot.times
            }
            None => {
                self.spots.push(Spot {
                    at: feet,
                    last: time,
                    times: 1,
                });
                1
            }
        };
        self.last_times = times;
        Some(match times {
            1 => Recovery::BackOff,
            2 => Recovery::Sidestep,
            _ => Recovery::GiveUp,
        })
    }

    /// The keys that take it off the spot it is stuck at the `times`th
    /// time ([`Self::walking`] just said so): back and to `side` first,
    /// then the other side, then straight to each side (a body caught in a
    /// corner between two props gets out the way it did not try).
    pub fn escape(&self, side: u32, other: u32) -> u32 {
        match self.last_times % 4 {
            1 => key::BACK | side,
            2 => key::BACK | other,
            3 => other,
            _ => side,
        }
    }

    /// The spots to keep clear of at `time`.
    pub fn avoided(&self, time: u32) -> impl Iterator<Item = Vec3> + '_ {
        self.spots
            .iter()
            .filter(move |spot| spot.times >= AVOID_AFTER && spot.last + SPOT_KEPT_MS > time)
            .map(|spot| spot.at)
    }

    /// Whether there is any spot to keep clear of at `time`.
    pub fn avoids_any(&self, time: u32) -> bool {
        self.avoided(time).next().is_some()
    }

    /// Whether `point` is near a spot to keep clear of.
    pub fn near_spot(&self, time: u32, point: Vec3) -> bool {
        self.avoided(time)
            .any(|spot| spot.distance_xz(point) < SPOT_M)
    }

    /// Whether a route keeps clear of the spots (past its first metres).
    pub fn clear(&self, time: u32, route: &Route) -> bool {
        let spots: Vec<Vec3> = self.avoided(time).collect();
        if spots.is_empty() {
            return true;
        }
        let mut s = ROUTE_FROM_M;
        while s < route.length() {
            let point = route.point_at(s);
            if spots
                .iter()
                .any(|spot| spot.distance_xz(point) < ROUTE_CLEAR_M)
            {
                return false;
            }
            s += 0.5;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::path::Waypoint;

    #[test]
    fn a_body_that_does_not_move_is_stuck_and_each_time_more_is_done() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 10.0);
        // Walking along: not stuck.
        assert_eq!(stuck.walking(1_000, Vec3::new(0.0, 0.0, 0.0), None), None);
        assert_eq!(
            stuck.walking(2_000, Vec3::new(3.0, 0.0, 0.0), None),
            None,
            "too soon to tell"
        );
        assert_eq!(stuck.walking(2_600, Vec3::new(5.0, 0.0, 0.0), None), None);
        // Caught: two worlds 1.5 s apart, 0.2 m between them.
        stuck.reset();
        assert_eq!(stuck.walking(10_000, spot, None), None);
        assert_eq!(
            stuck.walking(11_600, Vec3::new(10.2, 0.0, 10.0), None),
            Some(Recovery::BackOff)
        );
        // Again there: more each time.
        assert_eq!(stuck.walking(14_000, spot, None), None);
        assert_eq!(stuck.walking(15_500, spot, None), Some(Recovery::Sidestep));
        assert_eq!(stuck.walking(17_000, spot, None), None);
        assert_eq!(stuck.walking(18_500, spot, None), Some(Recovery::GiveUp));
        // Elsewhere, or long after: a first time again.
        assert_eq!(stuck.walking(19_000, Vec3::ZERO, None), None);
        assert_eq!(
            stuck.walking(20_500, Vec3::ZERO, None),
            Some(Recovery::BackOff)
        );
        assert!(stuck.near_spot(30_000, spot), "caught there three times");
        assert!(!stuck.near_spot(30_000, Vec3::ZERO), "caught there once");
        assert!(!stuck.near_spot(18_500 + SPOT_KEPT_MS, spot));
    }

    #[test]
    fn a_route_through_a_spot_is_not_clear() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 0.3);
        for time in [0, 1_500, 3_000, 4_500] {
            stuck.walking(time, spot, None);
        }
        let route = |to: (f32, f32)| {
            Route::new(
                [(0.0, 0.0), to]
                    .iter()
                    .map(|&(x, z)| Waypoint {
                        at: Vec3::new(x, 0.0, z),
                        node: 0,
                        jump: false,
                        low: false,
                    })
                    .collect(),
            )
        };
        assert!(!stuck.clear(5_000, &route((20.0, 0.0))));
        assert!(stuck.clear(5_000, &route((0.0, 20.0))));
        assert!(stuck.clear(4_500 + SPOT_KEPT_MS, &route((20.0, 0.0))));
    }

    /// A body that did not move while its keys meant to move it nowhere
    /// (standing to aim, waiting with a route in hand) is not stuck; one
    /// whose keys meant a walk is.
    #[test]
    fn only_a_body_that_meant_to_walk_is_stuck() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 10.0);
        assert_eq!(stuck.walking(0, spot, None), None);
        assert_eq!(stuck.walking(800, spot, Some(0.2)), None);
        assert_eq!(stuck.walking(1_600, spot, Some(0.3)), None, "meant 0.5 m");
        assert_eq!(stuck.walking(2_400, spot, Some(1.5)), None, "too soon");
        assert_eq!(
            stuck.walking(3_200, spot, Some(1.5)),
            Some(Recovery::BackOff),
            "meant 3 m"
        );
    }

    /// Each time again at the spot it backs off another way.
    #[test]
    fn each_time_at_a_spot_it_gets_off_another_way() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 10.0);
        let mut ways = Vec::new();
        let mut time = 0;
        for _ in 0..4 {
            stuck.walking(time, spot, None);
            time += STUCK_AFTER_MS;
            assert!(stuck.walking(time, spot, None).is_some());
            ways.push(stuck.escape(key::LEFT, key::RIGHT));
            time += 100;
        }
        assert_eq!(
            ways,
            [
                key::BACK | key::LEFT,
                key::BACK | key::RIGHT,
                key::RIGHT,
                key::LEFT
            ]
        );
    }
}
