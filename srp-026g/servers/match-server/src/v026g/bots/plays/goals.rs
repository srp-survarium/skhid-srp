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
//! - [`yields_to`], [`Ctx::spot_taken`]: the bots' claims on spots
//!   (`know::Claim`, one a bot, made at every think): a spot choice drops a
//!   spot within [`APART_M`] of a teammate's claim, and of two bots on one
//!   spot the higher place gives way.
//! - [`keep_apart`]: the one place a hold's intent is moved off a spot a
//!   teammate of a lower place has claimed (two bots on one spot: the higher
//!   place gives way, held [`YIELD_HOLD_MS`]).
//! - [`repick_if_shut`]: a play holding a spot swaps it for another pick
//!   only on that proof, and asks for the pick only then (a pick draws
//!   from the bot's random stream, so asking it earlier would change every
//!   draw after it). The failed target is reported by the search and the
//!   play chooses again (Detour's crowd), by the play's own score, not by
//!   nearness to the old spot (benched worse, 2026-10-25).

use mapdata::Vec3;

use super::{Ctx, Go, Intent, Look, PlayKind};
use crate::v026g::bots::view::{PlaceView, still_eye};

/// How near another bot's claim on a spot (`know::Claim`) a spot may be,
/// metres: two capsule widths (2 x 0.76 m, `default.player`). Players do
/// not collide in 0.26g, so it is a courtesy spacing, not a physical one.
pub const APART_M: f32 = 1.5;

/// Whether the bot of place `mine` gives way to the bot of place `theirs`
/// when their spots coincide: the higher place yields, the lower keeps.
/// The one rule of the plays' claims and of the motor's separation, so two
/// bots never both dodge. A human mate is yielded to whatever the places
/// (the caller decides that: this is the bot-against-bot rule).
pub fn yields_to(mine: usize, theirs: usize) -> bool {
    mine > theirs
}

/// A conflict (a claim of a lower place within [`APART_M`] of the point)
/// must hold this long before the bot gives way, ms: a bot passing by is no
/// conflict.
pub const YIELD_PROOF_MS: u32 = 1_000;
/// A point given way to is kept this long at least, ms (no flapping between
/// two spots: Johnson, "Taming Spatial Queries").
pub const YIELD_HOLD_MS: u32 = 5_000;
/// The first seconds of a life are exempt, ms: the bots come out of a few
/// spawn points.
pub const SETTLE_MS: u32 = 8_000;

/// The plays [`keep_apart`] moves: the holds, whose spot choices read the
/// claims too, and the two that walk to a mate (PairUp, Buddy), whose goals
/// are the mate's place: two of them on one mate stood on one spot (12 of 24
/// standing pairs of nine live matches were PairUp and PairUp).
fn keeps_apart(kind: PlayKind) -> bool {
    matches!(
        kind,
        PlayKind::Crossfire
            | PlayKind::Cut
            | PlayKind::HoldExit
            | PlayKind::GuardLead
            | PlayKind::PairUp
            | PlayKind::Buddy
    )
}

/// What a bot remembers of giving way: since when a claim of a lower place
/// has lain on its point, and the point it gave way to (with the point it
/// gave up and until when it is kept at least).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Apart {
    conflict_since: Option<u32>,
    held: Option<Held>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Held {
    gave_up: Vec3,
    at: Vec3,
    until: u32,
}

impl Apart {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `intent` with its goal moved to `at`; a bot that was to stay walks to
/// within `0.6` m of it.
fn moved(intent: Intent, at: Vec3) -> Intent {
    let within_m = match intent.go {
        Go::To { within_m, .. } => within_m,
        Go::Stay => 0.6,
    };
    Intent {
        go: Go::To { at, within_m },
        ..intent
    }
}

/// The step's intent for a hold, moved off a spot a teammate of a lower place
/// claims, and the point this bot claims now (its goal, or its feet when it
/// stays). Exempt: other plays, a carrier, the first [`SETTLE_MS`] of a life,
/// a point in a spawn area, an objective or a use, a straight walk.
///
/// Root cause (`push-record.md` S1, PROVEN from worlds): two teammates stood
/// on one spot for seconds (188 of 621 both-still pairs within 0.5 m in
/// nine matches): both chose in one pass, each seeing only the other's
/// feet, and a standing body is not steered off another. The higher place
/// gives way ([`yields_to`]) after [`YIELD_PROOF_MS`] of conflict, to the
/// nearest free point 1.75-3 m off; the point is kept [`YIELD_HOLD_MS`] and
/// longer while the claim still lies on the spot it gave up.
pub fn keep_apart(cx: &Ctx, apart: &mut Apart, kind: PlayKind, intent: Intent) -> (Intent, Vec3) {
    let point = match intent.go {
        Go::To { at, .. } => at,
        Go::Stay => cx.me.feet,
    };
    let exempt = !keeps_apart(kind)
        || cx.me.carrying.is_some()
        || cx.now.saturating_sub(cx.me.spawned) < SETTLE_MS
        || intent.objective
        || intent.straight
        || intent.press != 0
        || matches!(intent.look, Look::Use(_))
        || cx.tactics.in_spawn_area(point, None)
        || cx.tactics.in_spawn_area(cx.me.feet, None);
    if exempt {
        apart.reset();
        return (intent, point);
    }
    // The claim of a lower place on a spot, or a human mate's feet there (a
    // bot always gives way to a human, whatever the places).
    let lying_on = |spot: Vec3| {
        cx.team
            .claims_of_others(cx.me.place, cx.now)
            .any(|c| yields_to(cx.me.place, c.who) && c.at.distance_xz(spot) < APART_M)
            || cx
                .view
                .team(cx.me.team)
                .any(|p| p.place != cx.me.place && !p.bot && p.feet.distance_xz(spot) < APART_M)
    };
    if let Some(held) = apart.held {
        if cx.now < held.until || lying_on(held.gave_up) {
            let until = held.until.max(cx.now + 1);
            apart.held = Some(Held { until, ..held });
            return (moved(intent, held.at), held.at);
        }
        apart.held = None;
    }
    if !lying_on(point) {
        apart.conflict_since = None;
        return (intent, point);
    }
    let since = *apart.conflict_since.get_or_insert(cx.now);
    if cx.now < since + YIELD_PROOF_MS {
        return (intent, point);
    }
    let Some(free) = free_point(cx, point) else {
        return (intent, point);
    };
    apart.conflict_since = None;
    apart.held = Some(Held {
        gave_up: point,
        at: free,
        until: cx.now + YIELD_HOLD_MS,
    });
    (moved(intent, free), free)
}

/// The nearest to the bot of the points 1.75-3 m round `point`, on a floor a
/// goal may be on, with a way, out of the spawn areas and clear of every
/// claim of a teammate.
fn free_point(cx: &Ctx, point: Vec3) -> Option<Vec3> {
    let grid = &cx.tactics.world.grid;
    [APART_M + 0.25, 2.25, 3.0]
        .iter()
        .flat_map(|&m| {
            (0..8).map(move |k| {
                let yaw = k as f32 * std::f32::consts::FRAC_PI_4;
                Vec3::new(point.x + m * yaw.cos(), point.y, point.z + m * yaw.sin())
            })
        })
        .filter_map(|p| grid.locate(p, 1.5).map(|n| grid.position(n)))
        .filter(|p| (p.y - point.y).abs() < 1.0)
        .filter(|&p| {
            cx.tactics.fine_goal(p, cx.now, cx.bot.side)
                && cx.can_go(p)
                && !cx.tactics.in_spawn_area(p, None)
                && !cx.spot_taken(p, APART_M)
        })
        .min_by(|a, b| {
            a.distance_xz(cx.me.feet)
                .total_cmp(&b.distance_xz(cx.me.feet))
        })
}

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
                self.tactics.sees(still_eye(enemy, false), still_eye(p, false))
            })
        })
    }

    /// Whether a teammate's claim lies within `apart_m` of `at`: a spot
    /// choice drops it. The bot's own claim does not count.
    pub fn spot_taken(&self, at: Vec3, apart_m: f32) -> bool {
        self.team
            .claims_of_others(self.me.place, self.now)
            .any(|c| c.at.distance_xz(at) < apart_m)
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
    fn the_higher_place_yields_and_never_both() {
        assert!(yields_to(5, 2));
        assert!(!yields_to(2, 5));
        assert!(!yields_to(3, 3), "never to itself");
        for a in 0..6 {
            for b in 0..6 {
                assert!(!(yields_to(a, b) && yields_to(b, a)), "{a} and {b}");
            }
        }
    }

    #[test]
    fn a_spot_is_taken_by_a_teammates_fresh_claim_but_not_the_bots_own() {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        let spot = Vec3::new(25.0, 0.0, 30.0);
        let now = s.view.clock;
        assert!(!s.cx(0).spot_taken(spot, APART_M));
        s.team.claim(0, spot, now);
        assert!(!s.cx(0).spot_taken(spot, APART_M), "own claim");
        s.team.claim(3, Vec3::new(25.0 + APART_M - 0.1, 0.0, 30.0), now);
        assert!(s.cx(0).spot_taken(spot, APART_M));
        s.team.claim(3, Vec3::new(25.0 + APART_M + 0.1, 0.0, 30.0), now);
        assert!(!s.cx(0).spot_taken(spot, APART_M), "renewed elsewhere");
    }

    #[test]
    fn a_claim_not_renewed_expires_and_a_departure_drops_it() {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        let spot = Vec3::new(25.0, 0.0, 30.0);
        let now = s.view.clock;
        s.team.claim(3, spot, now);
        assert!(s.cx(0).spot_taken(spot, APART_M));
        s.team.claim(3, spot, now.saturating_sub(crate::v026g::bots::know::CLAIM_KEEP_MS + 1));
        assert!(!s.cx(0).spot_taken(spot, APART_M), "stale");
        s.team.claim(3, spot, now);
        s.team.forget(3);
        assert!(!s.cx(0).spot_taken(spot, APART_M), "left the match");
    }

    /// Two holds on one spot: the higher place gives way after a second of
    /// it, to a point clear of the claim, kept; the lower place keeps its spot.
    #[test]
    fn the_higher_place_gives_way_off_a_claimed_spot_after_a_second_and_keeps_to_it() {
        let mut s = Scenario::field(Mode::Batteries);
        let spot = Vec3::new(30.0, 0.0, 30.0);
        s.bot(1, 0, spot);
        s.bot(2, 0, Vec3::new(30.5, 0.0, 30.0));
        let mut apart = Apart::default();
        let mut now = s.view.clock;
        s.team.claim(1, spot, now);
        let stay = Intent::stay();
        // The first moment of the conflict: no move yet.
        let (intent, claim) = keep_apart(&s.cx(2), &mut apart, PlayKind::HoldExit, stay);
        assert_eq!(intent.go, Go::Stay);
        assert!(claim.distance_xz(Vec3::new(30.5, 0.0, 30.0)) < 0.01);
        // A second on, it gives way: to a point clear of the claim.
        now += YIELD_PROOF_MS;
        s.view.clock = now;
        s.team.claim(1, spot, now);
        let (intent, claim) = keep_apart(&s.cx(2), &mut apart, PlayKind::HoldExit, stay);
        let Go::To { at, .. } = intent.go else {
            panic!("did not give way: {intent:?}");
        };
        assert!(at.distance_xz(spot) >= APART_M, "{at:?}");
        assert_eq!(claim, at);
        // Kept while the claim lies on the spot it gave up, past the hold.
        now += YIELD_HOLD_MS + 2_000;
        s.view.clock = now;
        s.team.claim(1, spot, now);
        let (intent, _) = keep_apart(&s.cx(2), &mut apart, PlayKind::HoldExit, stay);
        assert_eq!(intent.go, Go::To { at, within_m: 0.6 });
        // The claim gone: after the hold it is its own again.
        now += 3_000;
        s.view.clock = now;
        let (intent, _) = keep_apart(&s.cx(2), &mut apart, PlayKind::HoldExit, stay);
        assert_eq!(intent.go, Go::Stay);
        // The lower place never gives way.
        let mut other = Apart::default();
        s.team.claim(2, Vec3::new(30.5, 0.0, 30.0), now);
        for _ in 0..3 {
            now += YIELD_PROOF_MS;
            s.view.clock = now;
            s.team.claim(2, Vec3::new(30.5, 0.0, 30.0), now);
            let (intent, _) = keep_apart(&s.cx(1), &mut other, PlayKind::HoldExit, stay);
            assert_eq!(intent.go, Go::Stay);
        }
    }

    /// A bot walking to a mate (PairUp, Buddy) gives way off a human mate's
    /// feet whatever the places, and off a bot of a lower place.
    #[test]
    fn a_bot_pairing_up_stands_beside_a_human_mate_not_on_him() {
        let mut s = Scenario::field(Mode::Batteries);
        let spot = Vec3::new(30.0, 0.0, 30.0);
        s.human(5, 0, spot);
        s.bot(2, 0, Vec3::new(30.4, 0.0, 30.0));
        let mut apart = Apart::default();
        let to_him = Intent::to(spot, super::super::Pace::Walk);
        let now = s.view.clock;
        let mut moved_to = None;
        for k in 0..3 {
            s.view.clock = now + k * YIELD_PROOF_MS;
            let (intent, _) = keep_apart(&s.cx(2), &mut apart, PlayKind::PairUp, to_him);
            if let Go::To { at, .. } = intent.go
                && at.distance_xz(spot) > APART_M
            {
                moved_to = Some(at);
            }
        }
        let at = moved_to.expect("did not give way to the human");
        assert!(at.distance_xz(spot) >= APART_M && at.distance_xz(spot) <= 3.1, "{at:?}");
        // Another play that stays by a human on purpose is left alone.
        let mut apart = Apart::default();
        for k in 0..3 {
            s.view.clock = now + k * YIELD_PROOF_MS;
            let (intent, _) = keep_apart(&s.cx(2), &mut apart, PlayKind::CoverHuman, to_him);
            assert_eq!(intent.go, to_him.go);
        }
    }

    /// Other plays, a carrier, a bot new to its life and a spawn area are not
    /// moved.
    #[test]
    fn a_bot_is_not_moved_off_a_claim_in_the_exempt_cases() {
        let mut s = Scenario::field(Mode::Batteries);
        let spot = Vec3::new(30.0, 0.0, 30.0);
        s.bot(1, 0, spot);
        s.bot(2, 0, spot);
        let now = s.view.clock;
        let stay = Intent::stay();
        // A conflict that lasts: the clock moves with each call.
        let moves = |s: &mut Scenario, kind: PlayKind| {
            let mut apart = Apart::default();
            for k in 0..4 {
                s.view.clock = now + k * YIELD_PROOF_MS;
                s.team.claim(1, spot, s.view.clock);
                let (intent, _) = keep_apart(&s.cx(2), &mut apart, kind, stay);
                if intent.go != Go::Stay {
                    return true;
                }
            }
            false
        };
        assert!(moves(&mut s, PlayKind::Crossfire));
        assert!(!moves(&mut s, PlayKind::Roam), "other plays");
        let carrier = s.view.places.iter().position(|p| p.place == 2).unwrap();
        s.view.places[carrier].carrying = Some(0);
        assert!(!moves(&mut s, PlayKind::Crossfire), "a carrier");
        s.view.places[carrier].carrying = None;
        s.view.places[carrier].spawned = now;
        assert!(!moves(&mut s, PlayKind::Crossfire), "a new life");
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
