//! Publish native medical-effect timing without changing simulation state.
//!
//! A drug a bullet stopped (patch 84) shows red for a second: the slot's
//! drug was seen in use, and is now ended with time left (what
//! set_active(0) leaves; a spent drug has none), for the first
//! `INTERRUPTED_MS` of the local player's simulation clock after that was
//! first seen. What is kept between updates is each quick slot's drug
//! identity and that time: presentation only, nothing of the simulation.
#![forbid(unsafe_code)]
use crate::vostok::consumable::{DrugId, GameTime, Observed, Shown};
use crate::vostok::{Context, Flow, GameCell, NoExit, QuickSlotActivity};

/// How long a stopped drug's slot shows red (the owner, 2026-10-31).
const INTERRUPTED_MS: u32 = 1000;

/// What a quick slot's last update saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seen {
    Idle,
    InUse(DrugId),
    /// Stopped while seen in use, first seen so at `at`.
    Stopped { drug: DrugId, at: GameTime },
}

static SEEN: GameCell<[Seen; 6]> = GameCell::new([Seen::Idle; 6]);

/// One update of a slot: what it saw before and sees now, at `now`.
fn step(seen: Seen, observed: Observed, now: GameTime) -> (Seen, Shown) {
    match (seen, observed) {
        (_, Observed::Idle) => (Seen::Idle, Shown::Inactive),
        (_, Observed::InUse { drug, course }) => (Seen::InUse(drug), Shown::InUse(course)),
        (Seen::InUse(was), Observed::Stopped { drug }) if was == drug => {
            (Seen::Stopped { drug, at: now }, Shown::Interrupted)
        }
        (Seen::Stopped { drug: was, at }, Observed::Stopped { drug }) if was == drug => {
            let shown = if now.since(at) < INTERRUPTED_MS { Shown::Interrupted } else { Shown::Inactive };
            (seen, shown)
        }
        // Ended with time left, never seen in use: a previous life's drug,
        // a late join's, another item in the slot.
        (_, Observed::Stopped { .. }) => (Seen::Idle, Shown::Inactive),
    }
}

pub fn on_update(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(slot) = QuickSlotActivity::at_update(ctx) {
        let index = slot.slot();
        if index < 6 {
            let mut seen = SEEN.get(ctx.thread());
            let (next, shown) = step(seen[index], slot.observed(), slot.now());
            seen[index] = next;
            SEEN.set(ctx.thread(), seen);
            slot.publish(shown);
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::consumable::Course;

    fn run(observed: &[(Observed, u32)]) -> std::vec::Vec<Shown> {
        let mut seen = Seen::Idle;
        observed.iter().map(|&(o, t)| {
            let (next, shown) = step(seen, o, GameTime::at(t));
            seen = next;
            shown
        }).collect()
    }

    const A: DrugId = DrugId::new(0x1000);
    const B: DrugId = DrugId::new(0x2000);
    fn using(drug: DrugId) -> Observed {
        Observed::InUse { drug, course: Course::waiting(900, 2000) }
    }
    fn stopped(drug: DrugId) -> Observed {
        Observed::Stopped { drug }
    }

    #[test]
    fn a_stop_shows_red_for_a_second() {
        let shown = run(&[(using(A), 0), (stopped(A), 100), (stopped(A), 1099), (stopped(A), 1100), (stopped(A), 5000)]);
        assert_eq!(shown[1..], [Shown::Interrupted, Shown::Interrupted, Shown::Inactive, Shown::Inactive]);
    }

    #[test]
    fn a_natural_end_a_death_or_a_stale_drug_shows_none() {
        assert_eq!(run(&[(using(A), 0), (Observed::Idle, 50)])[1], Shown::Inactive);
        // dead: the reader says Idle; after the respawn the old drug's timers
        // still say stopped, never seen in use in this life
        assert_eq!(run(&[(using(A), 0), (Observed::Idle, 50), (stopped(A), 8000)])[2], Shown::Inactive);
        assert_eq!(run(&[(stopped(A), 0)])[0], Shown::Inactive);
        assert_eq!(run(&[(using(B), 0), (stopped(A), 50)])[1], Shown::Inactive);
    }

    #[test]
    fn a_rollback_that_takes_the_hit_back_ends_the_red() {
        let shown = run(&[(using(A), 0), (stopped(A), 100), (using(A), 150), (Observed::Idle, 3000)]);
        assert!(matches!(shown[2], Shown::InUse(_)));
        assert_eq!(shown[3], Shown::Inactive);
        // the clock run back by a rollback keeps it red, not expired
        assert_eq!(run(&[(using(A), 0), (stopped(A), 500), (stopped(A), 400)])[2], Shown::Interrupted);
    }
}
