//! [A416, human-steering part D] When a walking bot's view leaves its course: the view schedule.
//!
//! The client has one yaw for view and body and walks the keys relative to it (`get_move_direction` 0x2D6510, PROVEN in
//! `traces-20261028/human-steering/exe-movement-rules.md`), so a view held off the course turns the key set: past 67.5 degrees
//! the sprint is lost (no W in the set), past 90 the walk goes sideways or back. The bots held any look the reflex or the play
//! gave for as long as it lasted (`drive.rs`: `over.look_at`, else `intent.look`): 11.7 % of their calm walking was backwards
//! (S or S+side) against the humans' 2.3 %, and their glances off the course were long (P90 1.95 s against 1.6 s).
//!
//! Live humans (124 matches, 246 players; human-steering 2.2-2.3): while running they glance 14.1 times a minute, P50 45 and P90
//! 81 degrees off the course, for P50 600 ms and P90 1,600 ms, mostly at nothing visible (69 %), W or W+side held through it.
//!
//! The schedule, one state and one transition a tick ([`View::step`]): a body walking to a goal with no threat on it (no target
//! seen, no shot of its own, no aim held: the reflex's [`super::reflex::Override`]) takes a look that is not its course as a
//! [`View::Glance`] of at most [`GLANCE_MOST_MS`] (the humans' P90); after a glance the view stays on the course for
//! [`COURSE_LEAST_MS`] (the humans' mean start-to-start gap 60 s / 14.1 = 4.26 s, less their P50 glance 0.6 s). A threat, a use
//! (`Look::Use`) or a body not walking anywhere takes its look at once, as before.

/// The longest glance off the course while walking, ms: the live humans' P90 glance length (human-steering 2.3, 88,794 glances).
pub const GLANCE_MOST_MS: u32 = 1_600;
/// The least time on the course between two glances while walking, ms: the humans' mean start-to-start gap (60,000 / 14.1 a
/// running minute = 4,255 ms) less their P50 glance (600 ms).
pub const COURSE_LEAST_MS: u32 = 3_655;

/// Where the walking body's view is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// On the course (the route's way), a glance allowed from `ready_at` (ms).
    Course { ready_at: u32 },
    /// Off it at a look of its own until `until` (ms).
    Glance { until: u32 },
    /// The look is free: a threat, a use, or a body not walking anywhere.
    #[default]
    Free,
}

/// What the tick asks of the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ask {
    /// The reflex or the play gives a look this tick.
    pub look: bool,
    /// A threat (a target seen, its own shot, the aim key held) or a use takes the look at once.
    pub urgent: bool,
    /// The body walks to a goal (a route ahead).
    pub walking: bool,
}

/// What the tick does with the look it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    /// The look is taken.
    Look,
    /// The view stays on the course (the walk's own).
    Course,
}

impl View {
    /// The tick at `now`: the next state and whether the look is taken.
    pub fn step(self, ask: Ask, now: u32) -> (View, Take) {
        if ask.urgent || !ask.walking {
            // A glance that a threat or a stop cut keeps its gap: the next one waits as after any glance.
            let next = match self {
                View::Glance { .. } if !ask.urgent => View::Course { ready_at: now.saturating_add(COURSE_LEAST_MS) },
                _ => View::Free,
            };
            return (next, if ask.look { Take::Look } else { Take::Course });
        }
        match (self, ask.look) {
            (View::Glance { until }, true) if now < until => (self, Take::Look),
            (View::Glance { .. }, _) => (View::Course { ready_at: now.saturating_add(COURSE_LEAST_MS) }, Take::Course),
            (View::Course { ready_at }, true) if now >= ready_at => (View::Glance { until: now.saturating_add(GLANCE_MOST_MS) }, Take::Look),
            (View::Course { .. }, _) => (self, Take::Course),
            (View::Free, true) => (View::Glance { until: now.saturating_add(GLANCE_MOST_MS) }, Take::Look),
            (View::Free, false) => (View::Course { ready_at: now }, Take::Course),
        }
    }
}

/// The schedule's counts: glances begun, glances ended at their most, looks held back on the course.
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

    pub const BEGUN: usize = 0;
    pub const CUT: usize = 1;
    pub const HELD: usize = 2;

    pub fn add(kind: usize) {
        COUNTS[kind].fetch_add(1, Ordering::Relaxed);
    }

    /// Glances begun, cut at [`super::GLANCE_MOST_MS`], look ticks held on the course.
    pub fn counts() -> [u64; 3] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALK: Ask = Ask { look: true, urgent: false, walking: true };

    /// A walking body's look is a glance of at most 1.6 s, then the course for 3.655 s whatever it is given, then a glance again;
    /// a threat takes the look at once and at any time; a body not walking keeps any look.
    #[test]
    fn a_walking_look_is_a_short_glance_then_the_course_and_a_threat_is_at_once() {
        let mut view = View::default();
        let mut taken = Vec::new();
        for t in (0..10_000).step_by(50) {
            let (next, take) = view.step(WALK, t);
            view = next;
            taken.push((t, take));
        }
        let looked = |from: u32, to: u32| taken.iter().filter(|&&(t, k)| t >= from && t < to && k == Take::Look).count();
        assert_eq!(looked(0, GLANCE_MOST_MS), (GLANCE_MOST_MS / 50) as usize, "the first glance, whole");
        assert_eq!(looked(GLANCE_MOST_MS, GLANCE_MOST_MS + COURSE_LEAST_MS), 0, "then the course");
        assert!(looked(GLANCE_MOST_MS + COURSE_LEAST_MS + 50, 10_000) > 0, "then a glance again");
        // A threat in the course's gap: taken.
        let (_, take) = View::Course { ready_at: 9_000 }.step(Ask { urgent: true, ..WALK }, 5_000);
        assert_eq!(take, Take::Look);
        // Standing: any look, any time.
        let (_, take) = View::Course { ready_at: 9_000 }.step(Ask { walking: false, ..WALK }, 5_000);
        assert_eq!(take, Take::Look);
        // No look given: the course, and a glance is ready at once from free.
        let (next, take) = View::Free.step(Ask { look: false, ..WALK }, 100);
        assert_eq!((next, take), (View::Course { ready_at: 100 }, Take::Course));
    }
}
