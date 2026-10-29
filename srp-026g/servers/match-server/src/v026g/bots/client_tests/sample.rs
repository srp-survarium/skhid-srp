//! The sample step: a stand-in camper at a spawn exit, the owner as the camper. It shows the API; the runner's own
//! tests use their own steps.

use super::*;

/// School's way from the raiders' spawn to the container of the owner's team (`an-anticamper.md` §6.1, the owner's own
/// hold of 2026-10-27).
const SPAWN_EXIT: Spot = Spot::at(47.0, 17.0, 116.0);

pub fn steps() -> Vec<Step> {
    vec![
        Step::new(
            "sample-camper-exit",
            Group::new(2, BATTERIES),
            "Walk to the marked spawn exit on School, stand there with a long gun and kill what comes; do not move for 90 s.",
        )
        .start(&[Cond::OwnerNear { at: SPAWN_EXIT, r_m: 3.0 }])
        .end(&[End::OwnerDied, End::OwnerLeft { at: SPAWN_EXIT, r_m: 12.0 }])
        .measure(Measure::OwnerKills)
        .measure(Measure::OwnerDeaths)
        .measure(Measure::FirstKillMs)
        .measure(Measure::DiedAtMs)
        .measure(Measure::MinHealth)
        .measure(Measure::TravelM)
        // The owner is the camper: a spot that does not draw two kills in 90 s is not a camping spot.
        .bar("owner_kills", Test::AtLeast(2.0))
        .budget_s(90)
        .tries(2),
    ]
}
