//! The plays' shared rules for a goal a bot may go to (HOT-FILES "Shared or
//! targeted": one home each, never written out again in a play):
//!
//! - [`no_way_near`]: whether a list of goals no search found a way to
//!   (`Bot::no_way_to`, the team's) holds one near `at` yet. The one
//!   distance test: [`Ctx::no_way_lately`] and the think's `goal_shut` both
//!   use it.
//! - [`Ctx::can_go`]: a goal the bot can go to: a way may be open
//!   ([`Ctx::reachable`], checked before any search: Red Blob Games, A*
//!   notes, "no path") and no search of its own proved none lately.
//! - [`repick_if_shut`]: a play holding a spot swaps it for another pick
//!   only on that proof, and asks for the pick only then (a pick draws
//!   from the bot's random stream, so asking it earlier would change every
//!   draw after it). The failed target is reported by the search and the
//!   play chooses again (Detour's crowd), by the play's own score, not by
//!   nearness to the old spot (benched worse, 2026-10-25).

use mapdata::Vec3;

use super::{Ctx, Go, Intent};
use crate::v026g::bots::view::PlaceView;

/// Whether `list` (goals no way was found to, each with the time it is
/// left until) has one near `at`, within the motor's replan distance
/// ([`crate::v026g::bots::drive::REPLAN_MOVED_M`]), still unexpired at `now`.
pub fn no_way_near(list: &[(Vec3, u32)], at: Vec3, now: u32) -> bool {
    list.iter()
        .any(|&(goal, until)| now < until && goal.distance_xz(at) <= crate::v026g::bots::drive::REPLAN_MOVED_M)
}

/// A track of an enemy counts for the climb's exposure while its own sense
/// (sight, hearing, a teammate's word) is this fresh, ms.
pub const CLIMB_FRESH_MS: u32 = 5_000;
/// The climb is looked at every this many metres, and by the nearest this
/// many tracks.
const CLIMB_STEP_M: f32 = 3.0;
const CLIMB_TRACKS: usize = 3;
/// A spot higher than this over its anchor is on a storey over it, metres
/// (`tactics::STOREY_FROM_M`).
const STOREY_OVER_M: f32 = 2.0;

impl Ctx<'_> {
    /// Whether the way up to `to`, a spot on a storey over its `anchor`, lies
    /// in sight of an enemy the bot knows of just now: the straight line from
    /// where it stands, looked at every 3 m from the eyes of the nearest three
    /// tracks (no new information: only what its senses and its team's gave in
    /// the last 5 s, and a ray each).
    ///
    /// Root cause (`push-record.md` V-S2a, held-line entries): in the
    /// deathmatch bench the extra bots that came into a held line were
    /// those bound for a spot more than 2 m up (158 to 243 of 1,404 to 1,510
    /// entries in the killed lane; 99 % knew the shooter), under a shooter
    /// over them: the choice of a storey spot never weighed the enemy it
    /// knew of in the way.
    pub fn exposed_climb(&self, to: Vec3, anchor: Vec3) -> bool {
        if to.y <= anchor.y + STOREY_OVER_M {
            return false;
        }
        let eye = Vec3::new(0.0, crate::v026g::bots::view::EYE_STANDING, 0.0);
        let mut seen: Vec<Vec3> = self
            .memory
            .tracks()
            .iter()
            .filter_map(|t| t.own.filter(|&(_, time)| self.now.saturating_sub(time) < CLIMB_FRESH_MS))
            .map(|(at, _)| at)
            .collect();
        seen.sort_by(|a, b| {
            a.distance_xz(self.me.feet)
                .total_cmp(&b.distance_xz(self.me.feet))
        });
        seen.truncate(CLIMB_TRACKS);
        let length = self.me.feet.distance_xz(to);
        let steps = (length / CLIMB_STEP_M).ceil().max(1.0) as usize;
        seen.iter().any(|&enemy| {
            (1..=steps).any(|k| {
                let p = self.me.feet + (to - self.me.feet) * (k as f32 / steps as f32);
                self.tactics.sees(enemy + eye, p + eye)
            })
        })
    }


    /// Whether the bot can go to `at`: a way may be open there
    /// ([`Ctx::reachable`], not for the objective) and no search of its own
    /// proved none lately ([`Ctx::no_way_lately`]).
    pub fn can_go(&self, at: Vec3) -> bool {
        self.reachable(at, false) && !self.no_way_lately(at)
    }
}

/// A spot proved no way to is swapped for the pick `pick` makes. `pick` is
/// asked only when the spot is shut, so a bot whose spot stands draws
/// nothing. Returns whether the spot was swapped; with no pick (none
/// left) the spot is kept, and the think leaves the play (`goal_shut`).
pub fn repick_if_shut(cx: &Ctx, spot: &mut Vec3, pick: impl FnOnce() -> Option<Vec3>) -> bool {
    if cx.no_way_lately(*spot)
        && let Some(new) = pick()
    {
        *spot = new;
        return true;
    }
    false
}

/// Whether a play's goal is one the bot's own search proved no way to
/// lately (`Bot::no_way_to`, [`NO_WAY_KEEP_MS`] from the proof): the bench
/// at 1dee0543 (288 matches) had a fifth of its stood-with-no-route ticks
/// at such goals, Buddy's and Crossfire's most, each searched again every
/// [`NO_WAY_RETRY_MS`] and kept up to the play's end. A carrier keeps
/// its play: its way home is the play's to change (`collect::way_home`).
pub fn goal_shut(intent: &Intent, me: &PlaceView, no_way_to: &[(Vec3, u32)], now: u32) -> bool {
    me.carrying.is_none()
        && matches!(intent.go, Go::To { at, within_m }
        if me.feet.distance_xz(at) > within_m
            && no_way_near(no_way_to, at, now))
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::drive::REPLAN_MOVED_M;
    use crate::v026g::bots::view::Mode;

    /// The way up to a spot on a storey over its anchor is out of the question
    /// when it lies in sight of an enemy known of just now, and for a spot that
    /// is not up there, or an enemy not known (or only stale), it never is.
    #[test]
    fn a_climb_in_sight_of_a_known_enemy_is_exposed_only_for_a_storey_spot() {
        use crate::v026g::bots::know::Source;
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.tactics = crate::v026g::bots::tactics::Tactics::new(crate::v026g::bots::tactics::tests::platform_world(), 1.0);
        s.bot(0, 0, Vec3::new(14.0, 0.0, 24.0));
        let anchor = Vec3::new(26.0, 0.0, 24.0);
        let up = Vec3::new(26.0, 4.0, 33.0);
        // No enemy known: nothing is exposed.
        assert!(!s.cx(0).exposed_climb(up, anchor));
        // An enemy on the ground in the open, seen just now.
        s.enemy(5, 1, Vec3::new(20.0, 0.0, 20.0), Some(Source::Seen));
        assert!(s.cx(0).exposed_climb(up, anchor));
        // The same climb to a spot on the anchor's own level is not looked at.
        assert!(!s.cx(0).exposed_climb(Vec3::new(30.0, 0.5, 24.0), anchor));
        // Stale: known 10 s ago.
        s.view.clock += 10_000;
        assert!(!s.cx(0).exposed_climb(up, anchor));
    }

    #[test]
    fn a_no_way_proof_holds_within_the_replan_distance_until_it_expires() {
        let goal = Vec3::new(10.0, 0.0, 10.0);
        let list = [(goal, 5_000)];
        let near = Vec3::new(10.0 + REPLAN_MOVED_M, 0.0, 10.0);
        let far = Vec3::new(10.0 + REPLAN_MOVED_M + 0.1, 0.0, 10.0);
        assert!(no_way_near(&list, goal, 4_999));
        assert!(no_way_near(&list, near, 0));
        assert!(!no_way_near(&list, far, 0));
        assert!(!no_way_near(&list, goal, 5_000), "expired at `until`");
        assert!(!no_way_near(&[], goal, 0));
    }

    #[test]
    fn can_go_needs_a_way_and_no_proof_against_it() {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        let there = Vec3::new(25.0, 0.0, 30.0);
        assert!(s.cx(0).can_go(there));
        // A proof against it, still unexpired.
        s.no_way_to = vec![(there, s.view.clock + 1_000)];
        assert!(!s.cx(0).can_go(there));
        s.no_way_to = vec![(there, s.view.clock)];
        assert!(s.cx(0).can_go(there));
        // Off the map: no way may be open.
        let off = Vec3::new(1_000.0, 0.0, 1_000.0);
        assert!(!s.cx(0).can_go(off));
    }

    #[test]
    fn a_spot_is_repicked_only_when_shut_and_only_to_a_pick() {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        let spot = Vec3::new(25.0, 0.0, 30.0);
        let other = Vec3::new(30.0, 0.0, 30.0);
        // Not shut: the pick is not asked, the spot stands.
        let mut kept = spot;
        let mut asked = false;
        let swapped = repick_if_shut(&s.cx(0), &mut kept, || {
            asked = true;
            Some(other)
        });
        assert!(!swapped && !asked && kept == spot);
        // Shut, with a pick: swapped.
        s.no_way_to = vec![(spot, s.view.clock + 1_000)];
        let swapped = repick_if_shut(&s.cx(0), &mut kept, || Some(other));
        assert!(swapped && kept == other);
        // Shut, with none to pick: kept.
        let mut kept = spot;
        let swapped = repick_if_shut(&s.cx(0), &mut kept, || None);
        assert!(!swapped && kept == spot);
    }
}
