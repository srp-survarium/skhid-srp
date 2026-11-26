//! [pincer] Two teammates heading for one objective come in on two bearings and arrive together (lane draw v4, 2026-11-26).
//!
//! The lane draw's second lanes (`lane_draw`) share their last `ends_m` with the first by construction, so they arrive from one side:
//! on bench v73 (seeds 1-6) teammates reaching one objective together came in 45 deg or more apart no more often with them than
//! without (35 % against 32 % of co-arrivals), and the walked lanes did not spread (15 m lane test: 4.65 lanes a group against the
//! tip's 4.39). Here the second walker of a pair takes a stage on another bearing round the objective, and both hold at their stages
//! until both are there, then go in together: a defender turned to one comes round to the other only after he is done with the
//! first.
//!
//! - **Who.** Two walks of one team under way to one objective ([`Objective`]: the play's target, not its point): one battery,
//!   container, exit, spawner or held enemy spot by its id; two enemies within [`CLUSTER_M`] of each other (one fight); two points
//!   of one region or linked regions (one area). The plays that walk to an objective against enemies ([`plays`]); never a carrier,
//!   an escort, a pair (Buddy, PairUp), a retreat, a sneak or Roam.
//! - **The window W** ([`window_s`]): how long a defender is busy with the first attacker, its time to kill him and its turn to the
//!   second: [`TTK_S`] (the bots' kits' automatics, every torso hit landing at 20 m against a same-tier kit: 134-240 ms, median
//!   186 ms; from the weapons' own `rounds_per_minute` and `bullet_damage` with the kits' armour) plus half the view
//!   (`sense.view_deg / 2`, the least angle a pincer takes) at [`TURN_DEG_S`], the live humans' P95 turn rate
//!   (`Feel::max_turn_speed_top`'s data). PvB's defenders are human players, so their turn rate is the one that matters. W is
//!   0.34 s at the 90 deg view.
//! - **The stage** ([`stage`]): a floor [`final_leg_m`] from the objective's centre (the longest last leg whose time error stays
//!   within W: W x [`SPEED_MPS`] / [`ETA_REL_P75`], both measured on the bench's walks; 5.2 m), walked to the centre in a straight
//!   line (`path::straight`: no jump, no drop), at least half the view off the teammate's own stage's bearing; the one nearest the
//!   walker. The teammate's stage is its own route's point that far before its end.
//! - **The hold** (`Reflex::hold_for_pincer`): each stands at its stage looking at the centre until the other is there, at most
//!   [`HOLD_CAP_MS`] (the p90 of the walks' time error), and lets go at once when hit, fighting, or when its own senses notice an
//!   enemy. No hold when the round has less time left than the cap.
//! - **The flank's caps.** The flank (its route to the stage, then the straight leg in) takes no more leaps and no more metres on
//!   edge floors ([`ledge_m`]) than the flanker's own plain way to the objective, searched once a pairing ([`PLAIN_WAY_FLOORS`]):
//!   a second way asks no more jumps, gaps, drops or ledge-walking of the body than the one it would have walked.
//! - **The fall-backs.** No free stage, or the flank over the band on its own way, or the two stages more than the cap apart in
//!   time, or the flank over its caps (or its own way not found within the floors): the walk goes as the lane draw has it (with
//!   the teammate's lane: shared, in its cover). A hold that ends without the teammate: the walk goes on alone.

use mapdata::{NodeId, Vec3, WalkGrid};

use super::plays::{PlayKind, TargetRef};
use super::view::Place;

/// [`window_s`]: the defender's time to kill one attacker, s: the median over the bots' kits' automatics (vityaz, ak_74u,
/// pp2000, akmn, sr3m, ak_12, vsk_94) of `(shots - 1) x 60 / rounds_per_minute` with every torso hit landing at 20 m against a
/// body in the same tier's kit (the probe `pincer_ttk_probe` on the bench data: 172, 186, 240, 200, 134, 186, 200 ms).
pub const TTK_S: f32 = 0.186;
/// [`window_s`]: the defender's turn rate, deg/s: the live humans' P95 while moving (`Feel::max_turn_speed_top`'s source).
pub const TURN_DEG_S: f32 = 288.0;
/// [`final_leg_m`]: the bots' median walking speed over their long legs, m/s (bench v73 A, 4,422 legs).
pub const SPEED_MPS: f32 = 3.07;
/// [`final_leg_m`]: the p75 of a long leg's relative time error against its length over [`SPEED_MPS`] (bench v73 A).
pub const ETA_REL_P75: f32 = 0.202;
/// The longest hold at a stage, ms: the p90 of a long leg's time error against its length over [`SPEED_MPS`] (bench v73 A,
/// 10.6 s): a teammate not staged by then is late past what the walks' own spread explains.
pub const HOLD_CAP_MS: u32 = 10_600;
/// Two enemies within this of each other are one fight, m (the lane draw's `away_m` and the lane test's cell).
pub const CLUSTER_M: f32 = 15.0;
/// The stage's ring is tried at this many bearings.
const RING: usize = 24;
/// The floors the flanker's own plain way to the objective is searched over: past the most of the probe's 177 long walks' plain
/// searches (p50 12,519, p90 24,239, max 33,429); its way not found within them, the pincer is not taken.
pub const PLAIN_WAY_FLOORS: usize = 40_000;
/// A walker is at its stage this near it, m (the cloud hold's `HOLD_AT_M`).
pub const STAGED_M: f32 = 1.5;
/// The look's height over the objective's centre while holding, m (a standing body's eye).
pub const EYE_M: f32 = 1.6;

/// The arrival window, s: [`TTK_S`] plus the turn through half the view at [`TURN_DEG_S`].
pub fn window_s(view_deg: f32) -> f32 {
    TTK_S + view_deg / 2.0 / TURN_DEG_S
}

/// The stage's distance from the objective's centre, m: the longest last leg whose time error (at [`ETA_REL_P75`]) stays within
/// the window.
pub fn final_leg_m(view_deg: f32) -> f32 {
    window_s(view_deg) * SPEED_MPS / ETA_REL_P75
}

/// The plays a pincer is for: those that walk to an objective against enemies.
pub fn plays(kind: PlayKind) -> bool {
    matches!(
        kind,
        PlayKind::Collect
            | PlayKind::Cut
            | PlayKind::Crossfire
            | PlayKind::HoldExit
            | PlayKind::Intercept
            | PlayKind::LateDenial
            | PlayKind::GuardLead
            | PlayKind::Trade
            | PlayKind::Breach
            | PlayKind::Raid
    )
}

/// What a walk heads for, for "the same goal": the play's target, or the goal's area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    Battery(usize),
    Container(u8),
    Exit(usize),
    Spawner(usize),
    Spot(u32),
    Enemy(Place),
    /// A point's (or a target-less goal's) region.
    Area(u32),
}

impl Objective {
    /// The objective of a play's `target` whose goal is in region `area`.
    pub fn of(target: TargetRef, area: Option<u32>) -> Option<Self> {
        Some(match target {
            TargetRef::Battery(i) => Self::Battery(i),
            TargetRef::Container(t) => Self::Container(t),
            TargetRef::Exit(i) => Self::Exit(i),
            TargetRef::Spawner(i) => Self::Spawner(i),
            TargetRef::ThreatSpot(id) => Self::Spot(id),
            TargetRef::Place(p) => Self::Enemy(p),
            TargetRef::Point(_) | TargetRef::None => Self::Area(area?),
        })
    }

    /// Whether two walks head for one objective: one object; two enemies one fight (`near`); two areas one region or linked ones
    /// (`linked`).
    pub fn same(self, other: Self, near: impl Fn(Place, Place) -> bool, linked: impl Fn(u32, u32) -> bool) -> bool {
        match (self, other) {
            (Self::Enemy(a), Self::Enemy(b)) => a == b || near(a, b),
            (Self::Area(a), Self::Area(b)) => a == b || linked(a, b),
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

/// The stage for a walker from `from` into `centre` (on floor `centre_node`), its teammate's stage at `mate_stage`: of the floors
/// `leg_m` from the centre on [`RING`] bearings, those `allowed`, at least `half_view_deg` off the teammate's bearing in, and walked
/// to the centre in a straight line; the one nearest `from`. With its bearing's angle off the teammate's.
#[allow(clippy::too_many_arguments)]
pub fn stage(
    grid: &WalkGrid,
    from: Vec3,
    centre: Vec3,
    centre_node: NodeId,
    mate_stage: Vec3,
    leg_m: f32,
    half_view_deg: f32,
    allowed: impl Fn(NodeId) -> bool,
) -> Option<(NodeId, f32)> {
    let mate_in = bearing_in(mate_stage, centre);
    (0..RING)
        .filter_map(|k| {
            let th = k as f32 * std::f32::consts::TAU / RING as f32;
            let node = grid.locate(Vec3::new(centre.x + leg_m * th.cos(), centre.y, centre.z + leg_m * th.sin()), 1.0)?;
            let off = apart_deg(bearing_in(grid.position(node), centre), mate_in);
            (off >= half_view_deg && allowed(node) && super::path::straight(grid, node, centre_node)).then_some((node, off))
        })
        .min_by(|a, b| grid.position(a.0).distance_xz(from).total_cmp(&grid.position(b.0).distance_xz(from)))
}

/// Metres of a route on floors next to a lip or a fall (the motor's edge floors: `is_lip`, `near_lip`, `near_fall`), sampled every
/// half metre.
pub fn ledge_m(grid: &WalkGrid, route: &super::path::Route) -> f32 {
    let len = route.length();
    let n = (len / 0.5).ceil().max(1.0) as usize;
    let step = len / n as f32;
    (0..=n)
        .filter(|&k| {
            grid.locate(route.point_at(step * k as f32), 1.0).is_some_and(|f| grid.is_lip(f) || grid.near_lip(f) || grid.near_fall(f))
        })
        .count() as f32
        * step
}

/// Where a bot is in its pincer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Walking to its stage; the length of the route its caps were checked on (`drive`'s `pincer_tick`), none yet; and its own plain
    /// way's leaps and edge metres, searched once a pincer.
    ToStage { checked_m: Option<f32>, own: Option<(u32, f32)> },
    /// At its stage, holding since then (ms).
    Staged { since: u32 },
    /// Let go: walking in to the objective (or alone, the pincer over).
    In,
}

/// A bot's pincer: its teammate, its stage, the objective's centre, its phase, and whether it is the flank (the walk that took a
/// stage off its teammate's bearing; the teammate's stage is on its own route, its way not capped).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pincer {
    pub mate: Place,
    pub stage: Vec3,
    pub centre: Vec3,
    pub phase: Phase,
    pub flank: bool,
}

/// A walk's state for its team in a pincer ([`super::lane_draw::TeamLanes`]): what its teammate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shared {
    /// Walking to its stage.
    Walking,
    /// At its stage.
    Staged,
    /// Going in, or out of it: the teammate holds for it no longer.
    Gone,
}

/// The pincer's nav lines (`nav_log::Kind::Lane`).
pub mod lines {
    use mapdata::Vec3;

    use super::super::nav_log::PREFIX;
    use super::super::view::Place;

    #[allow(clippy::too_many_arguments)]
    pub fn paired(play: impl std::fmt::Debug, mate: Place, stage: Vec3, centre: Vec3, off_deg: f32, leg_m: f32, w_s: f32, gap_s: f32) -> String {
        format!(
            "{PREFIX}pincer: {play:?} with b{mate}: stage ({:.1}, {:.1}) {leg_m:.1} m from ({:.1}, {:.1}), {off_deg:.0} deg off its bearing, W {w_s:.2} s, stages {gap_s:.1} s apart",
            stage.x, stage.z, centre.x, centre.z
        )
    }

    pub fn shared(play: impl std::fmt::Debug, mate: Place, why: &str) -> String {
        format!("{PREFIX}pincer: {play:?} with b{mate}: shared ({why})")
    }

    pub fn staged(stage: Vec3) -> String {
        format!("{PREFIX}pincer: staged at ({:.1}, {:.1}), holds", stage.x, stage.z)
    }

    pub fn ended(mate: Place, why: impl std::fmt::Debug, held_ms: u32) -> String {
        format!("{PREFIX}pincer: hold ended ({why:?}) after {:.1} s, b{mate}", held_ms as f32 / 1000.0)
    }

    pub fn alone(mate: Place, why: &str) -> String {
        format!("{PREFIX}pincer: goes in without b{mate} ({why})")
    }

    /// Its walk in the pincer ended (a new play, no goal, no way, there) with `play` now.
    pub fn walk_ended(mate: Place, play: Option<super::super::plays::PlayKind>) -> String {
        format!("{PREFIX}pincer: its walk ended before the stages (play now {play:?}), b{mate} goes alone")
    }

    pub fn joined(mate: Place, stage: Vec3) -> String {
        format!("{PREFIX}pincer: paired by b{mate}: stage ({:.1}, {:.1}) on its own route", stage.x, stage.z)
    }

    /// The flank checked against its own plain way: the leaps and edge metres of each, the plain search's floors and time, and the
    /// verdict.
    pub fn checked(mate: Place, leaps: (u32, Option<u32>), ledge_m: (f32, Option<f32>), floors: usize, us: u128, kept: bool) -> String {
        let own = |v: Option<String>| v.unwrap_or_else(|| "-".to_owned());
        format!(
            "{PREFIX}pincer: with b{mate}: flank {} leaps against its own way's {}, {:.1} m on edge floors against {}; own way searched {floors} floors {us} us: {}",
            leaps.0,
            own(leaps.1.map(|l| l.to_string())),
            ledge_m.0,
            own(ledge_m.1.map(|m| format!("{m:.1}"))),
            if kept { "kept" } else { "shared, goes in" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_and_the_last_leg_are_derived_from_the_kill_and_the_turn() {
        // 0.186 s + 45 deg at 288 deg/s.
        assert!((window_s(90.0) - (0.186 + 45.0 / 288.0)).abs() < 1e-6);
        assert!((final_leg_m(90.0) - window_s(90.0) * 3.07 / 0.202).abs() < 1e-4);
        assert!((5.0..5.5).contains(&final_leg_m(90.0)), "{}", final_leg_m(90.0));
    }

    #[test]
    fn one_objective_is_its_id_one_fight_or_one_area() {
        let none = |_: Place, _: Place| false;
        let unlinked = |_: u32, _: u32| false;
        let b3 = Objective::of(TargetRef::Battery(3), Some(9)).unwrap();
        assert!(b3.same(Objective::Battery(3), none, unlinked));
        assert!(!b3.same(Objective::Battery(4), none, unlinked));
        assert!(!b3.same(Objective::Area(9), none, unlinked), "a battery is not its region");
        assert_eq!(Objective::of(TargetRef::Point(Vec3::ZERO), Some(9)), Some(Objective::Area(9)));
        assert_eq!(Objective::of(TargetRef::Point(Vec3::ZERO), None), None);
        assert!(Objective::Area(9).same(Objective::Area(10), none, |a, b| (a, b) == (9, 10)));
        assert!(!Objective::Area(9).same(Objective::Area(11), none, |a, b| (a, b) == (9, 10)));
        assert!(Objective::Enemy(4).same(Objective::Enemy(5), |a, b| a + b == 9, unlinked), "one fight");
        assert!(!Objective::Enemy(4).same(Objective::Enemy(6), |a, b| a + b == 9, unlinked));
        assert!(plays(PlayKind::Collect) && !plays(PlayKind::Escort) && !plays(PlayKind::Roam) && !plays(PlayKind::Buddy));
    }

    #[test]
    fn bearings_are_compared_round_the_circle() {
        assert!((apart_deg(0.1, std::f32::consts::TAU - 0.1) - 0.2f32.to_degrees()).abs() < 1e-3);
        assert!((apart_deg(0.0, std::f32::consts::PI) - 180.0).abs() < 1e-3);
        let c = Vec3::new(10.0, 0.0, 10.0);
        assert!((bearing_in(Vec3::new(0.0, 0.0, 10.0), c)).abs() < 1e-6, "from the west: bearing 0");
    }

    /// On the test field (80 by 40 m, a wall at x 40 from z 0 to 25): a stage round (60, 30) away from a teammate staged west of it is
    /// on the far side, at the last leg's distance, half the view off; none when every floor is refused.
    #[test]
    fn a_stage_is_off_the_teammates_bearing_and_walks_straight_in() {
        let world = crate::v026g::bots::tactics::tests::field_with(|_| {});
        let grid = &world.grid;
        let centre = Vec3::new(60.0, 0.0, 30.0);
        let centre_node = grid.locate(centre, 1.5).unwrap();
        let mate = Vec3::new(55.0, 0.0, 30.0);
        let leg = final_leg_m(90.0);
        let (node, off) = stage(grid, Vec3::new(70.0, 0.0, 10.0), centre, centre_node, mate, leg, 45.0, |_| true).unwrap();
        let at = grid.position(node);
        assert!(off >= 45.0, "{off}");
        assert!((at.distance_xz(centre) - leg).abs() < 1.0, "{at:?}");
        assert!(apart_deg(bearing_in(at, centre), bearing_in(mate, centre)) >= 45.0);
        assert!(stage(grid, Vec3::ZERO, centre, centre_node, mate, leg, 45.0, |_| false).is_none());
    }
}
