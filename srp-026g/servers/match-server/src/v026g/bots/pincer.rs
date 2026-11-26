//! [pincer] Two teammates heading for one objective come in on two lanes and enter its sight together (lane draw v5, 2026-11-26).
//!
//! The lane draw's second lanes (`lane_draw`) share their last `ends_m` with the first by construction, so they arrive from one side:
//! on bench v73 (seeds 1-6) teammates reaching one objective together came in 45 deg or more apart no more often with them than
//! without (35 % against 32 % of co-arrivals). Humans on two lanes come in 45 deg or more apart 88 % of the time when their shared
//! last stretch is under 25 m, 42 % when it is longer (Fable task D §3.4). Here the second walker of a pair takes a lane whose last
//! stretch is its own, and both hold where their routes come into the objective's sight until both are there, then go in together:
//! a defender turned to one comes round to the other only after he is done with the first.
//!
//! - **Who** ([`plays`], [`Objective`]). Two walks of one team under way to one objective, of the plays that walk to the objective
//!   itself against enemies (Collect, Intercept, LateDenial, Trade, Breach, Raid). The position plays (Crossfire, HoldExit, Cut,
//!   GuardLead) send each bot to its own spot round a target, which their plays set apart: no pincer. One objective is one battery,
//!   container, exit, spawner or held enemy spot by its id; two enemies within [`CLUSTER_M`] of each other (one fight); two plain
//!   points within the lane draw's `away_m`. Never a carrier, nor a walker in a fight (either of the two: its claim publishes it).
//! - **The flank** (`lane_draw::draw`): the second walker's lane, searched as the lane draw's second lane but off its teammate's
//!   lane all the way in: off the goal's end only the goal's own region is shared, and the regions next to it the teammate's lane does
//!   not pass. Its caps are the lane draw's (the band on its own L1's cost, no more leaps than L1's). Checked before the pairing: a
//!   refusal moves nobody. Over the caps, or no such lane, the walk goes as the lane draw has it.
//! - **The stage** ([`stage_on`]): each walker's own route's last point out of its goal's sight (an eye over the route, an eye over
//!   the goal; [`EYE_M`]) and off the ground its team's held shooters cover (`threat::Covered::exposed_at`, the lane draw's own
//!   exposure: a stage out of the objective's sight is not out of another shooter's), found again for each new route. A route with
//!   no such point has none: that walker goes in alone. A hold whose stage comes under a held shooter's cover lets go.
//! - **The plays held.** While a pincer is under way the commander holds both walkers' plays as a started play is held
//!   (`team::Commit::Held`): against every candidate but a named interrupter (`team::interrupts`), and only while the play is still a
//!   candidate (its objective there). On the way it breaks at a hit, a fight nearer than its stage, or the round's last
//!   [`HOLD_CAP_MS`].
//! - **The hold** (`Reflex::hold_for_pincer`): each stands at its stage looking at its goal until the other is there; the second to
//!   arrive lets both go at once ([`Shared::Go`]). At most [`HOLD_CAP_MS`] (the p90 of the walks' time error), and let go at once when
//!   hit, fighting, or when its own senses notice an enemy. No pairing when the two arrivals are more than the cap apart, nor when the
//!   round has less time left than the cap.

use mapdata::Vec3;

use super::plays::{PlayKind, TargetRef};
use super::view::Place;

/// The pairing's arrival times: the bots' median walking speed over their long legs, m/s (bench v73 A, 4,422 legs).
pub const SPEED_MPS: f32 = 3.07;
/// The longest hold at a stage, ms: the p90 of a long leg's time error against its length over [`SPEED_MPS`] (bench v73 A,
/// 10.6 s): a teammate not staged by then is late past what the walks' own spread explains.
pub const HOLD_CAP_MS: u32 = 10_600;
/// Two enemies within this of each other are one fight, m (the lane draw's `away_m` and the lane test's cell).
pub const CLUSTER_M: f32 = 15.0;
/// A walker is at its stage this near it, m (the cloud hold's `HOLD_AT_M`).
pub const STAGED_M: f32 = 1.5;
/// The eye's height over the feet for the stage's sight test and the hold's look, m (a standing body's eye).
pub const EYE_M: f32 = 1.6;
/// The stage's sight test steps along the route this far, m.
const SIGHT_STEP_M: f32 = 1.0;

/// The plays a pincer is for: those whose walkers walk to the objective itself against enemies (not the position plays, whose
/// walkers each take their own spot round a target).
pub fn plays(kind: PlayKind) -> bool {
    matches!(
        kind,
        PlayKind::Collect | PlayKind::Intercept | PlayKind::LateDenial | PlayKind::Trade | PlayKind::Breach | PlayKind::Raid
    )
}

/// What a walk heads for, for "the same objective": the play's target, or its plain goal point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Objective {
    Battery(usize),
    Container(u8),
    Exit(usize),
    Spawner(usize),
    Spot(u32),
    Enemy(Place),
    /// A point's (or a target-less play's) goal.
    Point(Vec3),
}

impl Objective {
    /// The objective of a play's `target` whose goal is `goal`.
    pub fn of(target: TargetRef, goal: Vec3) -> Self {
        match target {
            TargetRef::Battery(i) => Self::Battery(i),
            TargetRef::Container(t) => Self::Container(t),
            TargetRef::Exit(i) => Self::Exit(i),
            TargetRef::Spawner(i) => Self::Spawner(i),
            TargetRef::ThreatSpot(id) => Self::Spot(id),
            TargetRef::Place(p) => Self::Enemy(p),
            TargetRef::Point(_) | TargetRef::None => Self::Point(goal),
        }
    }

    /// Whether two walks head for one objective: one object; two enemies one fight (`near`); two points within `away_m`.
    pub fn same(self, other: Self, near: impl Fn(Place, Place) -> bool, away_m: f32) -> bool {
        match (self, other) {
            (Self::Enemy(a), Self::Enemy(b)) => a == b || near(a, b),
            (Self::Point(a), Self::Point(b)) => a.distance_xz(b) <= away_m,
            (a, b) => a == b,
        }
    }
}

/// The angle between two bearings, degrees (0-180).
pub fn apart_deg(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d).to_degrees()
}

/// The bearing a point comes into `centre` on, radians (atan2 of z, x).
pub fn bearing_in(from: Vec3, centre: Vec3) -> f32 {
    (centre.z - from.z).atan2(centre.x - from.x)
}

/// The stage on a route of `points` (its corners, from the walker to its goal, the last): walked back from the goal in steps of
/// [`SIGHT_STEP_M`], the first point from whose eye the goal's eye is not seen (`sees`, eyes [`EYE_M`] over the feet) and that is not
/// `exposed` (its feet), with the metres from it to the goal along the route. None when there is no such point (or the route has no
/// length).
pub fn stage_on(points: &[Vec3], mut sees: impl FnMut(Vec3, Vec3) -> bool, mut exposed: impl FnMut(Vec3) -> bool) -> Option<(Vec3, f32)> {
    let goal = *points.last()?;
    let eye = |p: Vec3| p + Vec3::new(0.0, EYE_M, 0.0);
    let mut back = 0.0;
    for w in points.windows(2).rev() {
        let (a, b) = (w[0], w[1]);
        let len = (b - a).length();
        let n = (len / SIGHT_STEP_M).ceil().max(1.0) as usize;
        for k in 1..=n {
            let s = len * k as f32 / n as f32;
            let p = b + (a - b) * (s / len.max(1e-6));
            if !sees(eye(p), eye(goal)) && !exposed(p) {
                return Some((p, back + s));
            }
        }
        back += len;
    }
    None
}

/// Where a bot is in its pincer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Walking to its stage on its route: the route's length the stage was found on and the stage (none: in sight all the way), or
    /// none yet (no route found since the pairing, or a new one).
    ToStage { found: Option<(f32, Option<Vec3>)> },
    /// At its stage, holding since then (ms).
    Staged { since: u32 },
    /// Let go: walking in to the objective (or alone, the pincer over).
    In,
}

/// A bot's pincer: its teammate, its goal (the objective's point), its phase, whether it is the flank (the walk that took the
/// flank lane), and since when (ms).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pincer {
    pub mate: Place,
    pub goal: Vec3,
    pub phase: Phase,
    pub flank: bool,
    pub since: u32,
}

impl Pincer {
    /// Its stage, when found on its route.
    pub fn stage(&self) -> Option<Vec3> {
        match self.phase {
            Phase::ToStage { found: Some((_, stage)) } => stage,
            _ => None,
        }
    }
}

/// A walk's state for its team in a pincer ([`super::lane_draw::TeamLanes`]): what its teammate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shared {
    /// Walking to its stage.
    Walking,
    /// At its stage, holding.
    Staged,
    /// Both staged: the second to arrive lets both go in.
    Go,
    /// Out of it: the teammate holds for it no longer.
    Gone,
}

/// The pincer's nav lines (`nav_log::Kind::Lane`).
pub mod lines {
    use mapdata::Vec3;

    use super::super::nav_log::PREFIX;
    use super::super::view::Place;

    /// A pairing: the flank lane's cost against its own L1's and its leaps against L1's, the teammate's metres left, the two
    /// arrivals' seconds apart.
    pub fn paired(play: impl std::fmt::Debug, mate: Place, goal: Vec3, flank: (f32, u32), first: (f32, u32), mate_m: f32, gap_s: f32) -> String {
        format!(
            "{PREFIX}pincer: {play:?} with b{mate} to ({:.1}, {:.1}): flank {:.1} m ({:.3}) {} leaps against L1 {:.1} m {} leaps, b{mate} {mate_m:.1} m left, arrivals {gap_s:.1} s apart",
            goal.x,
            goal.z,
            flank.0,
            flank.0 / first.0.max(0.1),
            flank.1,
            first.0,
            first.1,
        )
    }

    pub fn shared(play: impl std::fmt::Debug, mate: Place, why: &str) -> String {
        format!("{PREFIX}pincer: {play:?} with b{mate}: shared ({why})")
    }

    /// Its stage found on a new route: where, and its metres to the goal along the route; or none (in sight all the way).
    pub fn stage(mate: Place, stage: Option<(Vec3, f32)>, route_m: f32) -> String {
        match stage {
            Some((at, m)) => format!("{PREFIX}pincer: stage at ({:.1}, {:.1}), {m:.1} m before the goal on its {route_m:.1} m route, b{mate}", at.x, at.z),
            None => format!("{PREFIX}pincer: no stage, its {route_m:.1} m route in the goal's sight all the way, b{mate}"),
        }
    }

    /// A hold let go: its stage came under a held shooter's cover.
    pub fn exposed(mate: Place, stage: Vec3) -> String {
        format!("{PREFIX}pincer: hold ended (its stage ({:.1}, {:.1}) under a held shooter's cover), b{mate}", stage.x, stage.z)
    }

    pub fn staged(stage: Vec3) -> String {
        format!("{PREFIX}pincer: staged at ({:.1}, {:.1}), holds", stage.x, stage.z)
    }

    /// The second to its stage: both go in.
    pub fn both(mate: Place, stage: Vec3) -> String {
        format!("{PREFIX}pincer: staged at ({:.1}, {:.1}) with b{mate} staged: both go in", stage.x, stage.z)
    }

    pub fn ended(mate: Place, why: impl std::fmt::Debug, held_ms: u32) -> String {
        format!("{PREFIX}pincer: hold ended ({why:?}) after {:.1} s, b{mate}", held_ms as f32 / 1000.0)
    }

    pub fn alone(mate: Place, why: &str) -> String {
        format!("{PREFIX}pincer: goes in without b{mate} ({why})")
    }

    /// Its walk in the pincer ended (a new play, no goal, no way, there) with `play` now and the commander's reason for it.
    pub fn walk_ended(mate: Place, play: Option<(super::super::plays::PlayKind, &str)>) -> String {
        let (kind, why) = play.map_or(("None".to_owned(), "-"), |(k, w)| (format!("{k:?}"), w));
        format!("{PREFIX}pincer: its walk ended before the stages (play now {kind}: {why}), b{mate} goes alone")
    }

    /// A break of the pincer on the way to the stages.
    pub fn broke(mate: Place, why: &str) -> String {
        format!("{PREFIX}pincer: broken on the way ({why}), b{mate} goes alone")
    }

    pub fn joined(mate: Place, goal: Vec3) -> String {
        format!("{PREFIX}pincer: paired by b{mate} to ({:.1}, {:.1}): its own lane", goal.x, goal.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_objective_is_its_id_one_fight_or_points_within_away() {
        let none = |_: Place, _: Place| false;
        let b3 = Objective::of(TargetRef::Battery(3), Vec3::ZERO);
        assert!(b3.same(Objective::Battery(3), none, 15.0));
        assert!(!b3.same(Objective::Battery(4), none, 15.0));
        assert!(!b3.same(Objective::Point(Vec3::ZERO), none, 15.0), "a battery is not a point");
        let p = Objective::of(TargetRef::Point(Vec3::ZERO), Vec3::new(1.0, 0.0, 1.0));
        assert_eq!(p, Objective::Point(Vec3::new(1.0, 0.0, 1.0)), "a point's objective is the walk's goal");
        assert!(p.same(Objective::Point(Vec3::new(14.0, 0.0, 1.0)), none, 15.0));
        assert!(!p.same(Objective::Point(Vec3::new(17.0, 0.0, 1.0)), none, 15.0));
        assert!(Objective::Enemy(4).same(Objective::Enemy(5), |a, b| a + b == 9, 15.0), "one fight");
        assert!(!Objective::Enemy(4).same(Objective::Enemy(6), |a, b| a + b == 9, 15.0));
        assert!(plays(PlayKind::Collect) && plays(PlayKind::Raid) && !plays(PlayKind::Escort) && !plays(PlayKind::Roam));
        for position in [PlayKind::Crossfire, PlayKind::HoldExit, PlayKind::Cut, PlayKind::GuardLead] {
            assert!(!plays(position), "{position:?}: each walker its own spot");
        }
    }

    #[test]
    fn bearings_are_compared_round_the_circle() {
        assert!((apart_deg(0.1, std::f32::consts::TAU - 0.1) - 0.2f32.to_degrees()).abs() < 1e-3);
        assert!((apart_deg(0.0, std::f32::consts::PI) - 180.0).abs() < 1e-3);
        let c = Vec3::new(10.0, 0.0, 10.0);
        assert!((bearing_in(Vec3::new(0.0, 0.0, 10.0), c)).abs() < 1e-6, "from the west: bearing 0");
    }

    /// A route east to (30, 0, 0), then north to (30, 0, 10): the goal is seen only from x 25 on (a wall); the stage is the first
    /// point back from the goal out of its sight, a 1 m step past x 25 (x 24), 16 m before the goal along the route; none when seen
    /// all the way; off a held shooter's cover, further back; none when covered wherever the goal is not seen.
    #[test]
    fn the_stage_is_the_last_point_of_its_route_out_of_the_goals_sight() {
        let points = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(30.0, 0.0, 0.0), Vec3::new(30.0, 0.0, 10.0)];
        let seen_from_25 = |a: Vec3, _: Vec3| a.x >= 25.0;
        let open = |_: Vec3| false;
        let (at, m) = stage_on(&points, seen_from_25, open).unwrap();
        assert!((24.0..25.0).contains(&at.x) && at.z == 0.0, "{at:?}");
        assert!((m - (10.0 + (30.0 - at.x))).abs() < 1e-3, "{m}");
        assert!(stage_on(&points, |_, _| true, open).is_none(), "in sight all the way");
        assert!(stage_on(&points[..1], |_, _| false, open).is_none(), "no length");
        // A held shooter covers x 20-24.9: the stage is further back, out of his cover too; covered all the way back, none.
        let (at, _) = stage_on(&points, seen_from_25, |p: Vec3| (20.0..25.0).contains(&p.x)).unwrap();
        assert!((19.0..20.0).contains(&at.x), "{at:?}");
        assert!(stage_on(&points, seen_from_25, |p: Vec3| p.x < 25.0).is_none(), "covered wherever the goal is not seen");
        // The eyes are over the feet.
        let mut asked = Vec::new();
        let _ = stage_on(
            &points[1..],
            |a, b| {
                asked.push((a.y, b.y));
                false
            },
            open,
        );
        assert_eq!(asked[0], (EYE_M, EYE_M));
    }
}
