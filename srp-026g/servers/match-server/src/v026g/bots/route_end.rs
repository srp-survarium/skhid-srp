//! Why a bot's route ended, and how old it was.
//!
//! Every place that drops or replaces a bot's route goes through
//! [`Bot::end_route`] / [`Bot::set_route`], so the bench (`cfg(test)`, and the jump test's build) can tell,
//! per ending, what ended the route and after how many ms: a route that ends
//! at age 0 ms is one that never walked a step (a first leg the body cannot
//! make), one that ends at 30 s was walked. Nothing here decides anything.

use mapdata::Vec3;

use super::Bot;
use super::path::Route;

/// What ended a route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteEnd {
    /// The walk got to the route's end (`Walked::Arrived`).
    Arrived,
    /// The body fell off it or was too far from it (`Walked::Off`).
    Off,
    /// The motor gave up a jump of it (`Walked::JumpFailed`).
    JumpFailed,
    /// The play went straight, by keys (`Intent::straight`).
    Straight,
    /// A crossing's zone was no longer borne (`crossing::still_borne`).
    CrossingAborted,
    /// A zone shut ahead of it (`Tactics::route_shut_ahead`).
    ZoneShut,
    /// A replan found no way, and the goal's no-way is kept.
    NoWay,
    /// The play has no goal.
    NoGoal,
    /// A new play was assigned (`commander`).
    NewAssignment,
    /// The play ended (`Bot::end_play`).
    EndPlay,
    /// The body died and came back (`Bot::new_life`).
    NewLife,
    /// The world put the body far from its reckoning (`REPLAN_ERROR_M`).
    ReckoningError,
    /// A pocket's way out took the route (`escape::tick`).
    EscapeStep,
    /// A search put another route in its place (or none).
    Replanned,
    /// It ran over a bear trap's disc (`traps::keep_off`).
    TrapAhead,
    /// Its body walked for the tuned time without getting further along the route (`Stuck::no_progress`).
    NoProgress,
}

impl RouteEnd {
    /// Every ending, in the order of the bench's counts.
    pub const ALL: [RouteEnd; 16] = [
        RouteEnd::Arrived,
        RouteEnd::Off,
        RouteEnd::JumpFailed,
        RouteEnd::Straight,
        RouteEnd::CrossingAborted,
        RouteEnd::ZoneShut,
        RouteEnd::NoWay,
        RouteEnd::NoGoal,
        RouteEnd::NewAssignment,
        RouteEnd::EndPlay,
        RouteEnd::NewLife,
        RouteEnd::ReckoningError,
        RouteEnd::EscapeStep,
        RouteEnd::Replanned,
        RouteEnd::TrapAhead,
        RouteEnd::NoProgress,
    ];

    /// The name the bench prints.
    pub fn name(self) -> &'static str {
        match self {
            RouteEnd::Arrived => "arrived",
            RouteEnd::Off => "off",
            RouteEnd::JumpFailed => "jump_failed",
            RouteEnd::Straight => "straight",
            RouteEnd::CrossingAborted => "crossing_aborted",
            RouteEnd::ZoneShut => "zone_shut",
            RouteEnd::NoWay => "no_way",
            RouteEnd::NoGoal => "no_goal",
            RouteEnd::NewAssignment => "new_assignment",
            RouteEnd::EndPlay => "end_play",
            RouteEnd::NewLife => "new_life",
            RouteEnd::ReckoningError => "reckoning_error",
            RouteEnd::EscapeStep => "escape_step",
            RouteEnd::Replanned => "replanned",
            RouteEnd::TrapAhead => "trap_ahead",
            RouteEnd::NoProgress => "no_progress",
        }
    }
}

impl Bot {
    /// Drops its route, if it has one, because of `why`.
    pub(super) fn end_route(&mut self, why: RouteEnd, now: u32) {
        if self.route.take().is_some() {
            self.route_ended(why, now);
        }
    }

    /// Puts `route` in place of its route (the old one, if any, ends as
    /// [`RouteEnd::Replanned`]).
    pub(super) fn set_route(&mut self, route: Option<(Route, Vec3)>, now: u32) {
        if self.route.is_some() {
            self.route_ended(RouteEnd::Replanned, now);
        }
        // The bench's: a new route is no search's way so far (the drive marks one that is).
        #[cfg(test)]
        {
            self.bench_on_partial = false;
        }
        #[cfg(any(test, feature = "jump-test"))]
        if route.is_some() {
            self.route_born = now;
        }
        #[cfg(not(any(test, feature = "jump-test")))]
        let _ = now;
        self.route = route;
    }

    #[cfg(any(test, feature = "jump-test"))]
    pub(super) fn route_ended(&mut self, why: RouteEnd, now: u32) {
        let age = now.saturating_sub(self.route_born);
        self.route_end = Some((why, age));
        #[cfg(test)]
        counts::ended(why, age);
    }

    #[cfg(not(any(test, feature = "jump-test")))]
    pub(super) fn route_ended(&mut self, _: RouteEnd, _: u32) {}

    /// How its last route ended, and its age in ms then.
    #[cfg(any(test, feature = "jump-test"))]
    pub fn last_route_end(&self) -> Option<(RouteEnd, u32)> {
        self.route_end
    }
}

/// The bench's counts of endings (all matches of the run): how many of each
/// [`RouteEnd`], and the sum of their ages in ms.
#[cfg(test)]
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::RouteEnd;

    static ENDED: [AtomicU64; RouteEnd::ALL.len()] =
        [const { AtomicU64::new(0) }; RouteEnd::ALL.len()];
    static AGE_MS: [AtomicU64; RouteEnd::ALL.len()] =
        [const { AtomicU64::new(0) }; RouteEnd::ALL.len()];

    pub fn ended(why: RouteEnd, age_ms: u32) {
        let k = why as usize;
        ENDED[k].fetch_add(1, Ordering::Relaxed);
        AGE_MS[k].fetch_add(u64::from(age_ms), Ordering::Relaxed);
    }

    /// (endings, summed ages in ms), by [`RouteEnd::ALL`].
    pub fn counts() -> [(u64, u64); RouteEnd::ALL.len()] {
        std::array::from_fn(|k| {
            (
                ENDED[k].load(Ordering::Relaxed),
                AGE_MS[k].load(Ordering::Relaxed),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ending_has_its_place_and_name() {
        for (k, why) in RouteEnd::ALL.into_iter().enumerate() {
            assert_eq!(why as usize, k);
            assert!(!why.name().is_empty());
        }
    }
}
