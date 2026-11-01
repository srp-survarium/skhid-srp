//! The stand-in camper of a bench run ([`BenchCamper`], `SRP_BENCH_CAMPER=<place>`), and its moving form
//! (`SRP_BENCH_CAMPER_MOVE=1`, an-anticamper §8.4): a human who holds a spot works an area about 8 m across, peeks every
//! second, steps off after a kill or a hit and leaves a few seconds after his last kill. The old stand-in stood within 0.7 m of
//! one point all match, which made Flank's gate (crouched, aiming, still for 1.5 s) the one thing that fired on him. The numbers
//! are the owner's holds and 8,474 live ones (§8.4's table):
//!
//! - **Area.** The spot and up to three cover-side points of the side's routes 3-8 m from it ([`plan_area`]), those seen
//!   from the fewest route points of the other side first. PROVEN (table): half the time within 3 m of the hold's centre,
//!   nine tenths within 8 m.
//! - **Peeks.** Out 0.4-0.45 s, a peek every 0.8-1.2 s, only while an enemy is known within reach. PROVEN (owner: 0.45 s /
//!   1.2 s; live: 0.40 s / 0.8 s). A peek here is a sideways step of [`PEEK_M`] and back (GUESS: whether the owner was out of
//!   cover in between is not tested in the table).
//! - **Stance.** Standing, with a short crouch spell now and then (about a tenth of the time; owner 0 %, live median 13 %).
//! - **Relocation.** To another area point 2.5-3.5 m away within 5 s of a kill (owner 2.5 m, live 3.6 m moved) and 2-2.5 m
//!   after a hit (2.1 / 2.6 m).
//! - **Dwell.** He leaves 3-5 s after a stay's second kill with no new one, and after `SRP_BENCH_CAMPER_DWELL_S` (default
//!   22.5 s, live hold median 25 s) from the stay's start or its first kill with fewer than two, to the next spot of his
//!   scenario ([`CamperAt`], the spot rule) at least [`LEAVE_MIN_M`] away, or, anchored, [`ANCHORED_MIN_M`].
//! - **Return.** After he is killed he goes back to the same spot 13-20 % of the time, median 26-42 s after the death
//!   (an-anticamper, `bp-anticamper-human.md`: our data): [`RETURN_P`], [`RETURN_MEDIAN_S`]. Otherwise he takes a new spot. A trap on
//!   a defeated camper's spot (A145) is judged on this rate.
//!
//! The reflex still aims and fires, and keeps the legs off (`Bot::bench_camper`). His reflex and decisions play by a
//! fixed tuning ([`BenchCamper::tuning`]), not his team's: in a head-to-head run (one team ON, the other OFF, sides swapped)
//! he is then the same opponent in both runs. His aim is his own too, the file `camper_human_aim.json` (`standin_aim.rs`), so
//! that no change of the bots' aim defaults moves him between builds; the rest is the build's defaults.

use super::*;
use crate::v026g::bots::control::Driver;
use crate::v026g::bots::know::Track;
use crate::v026g::bots::plays::{Go, Intent, Look, Pace};
use crate::v026g::bots::tactics::Tactics;
use crate::v026g::bots::think::Due;
use crate::v026g::bots::view::PlaceView;

/// Where the stand-in camper's spot is (`SRP_BENCH_CAMPER_AT`): the old rule over every route point of his side, or only those
/// near what the owner's cases hold (an-anticamper §8.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CamperAt {
    /// Any route point of his side, by the spot rule.
    #[default]
    Free,
    /// Within [`AT_NEAR_M`] of a battery that lies on the map (a spawner's battery): the camper by an objective (RC3).
    Battery,
    /// Within [`AT_NEAR_M`] of his own team's container.
    Container,
    /// [`EXIT_MIN_M`] to [`EXIT_MAX_M`] from the other team's nearest spawn, the point they are seen from in the narrowest sector: a spawn
    /// exit that cannot be flanked, where every sight on him comes from one way. Outside their spawn area: no route enters it (`Tactics`'s
    /// spawn rule), so a spot within 25 m was reached in 9 of 211 lives (A212 §2, `wsl-out/batteries/sb-base-efdd5d69-20261101`).
    Exit,
    /// [A239] A live camper's spot (the camp spec, `traces-20261028/fx-threat-obj-camp-spec-note.md`): [`LIVE_SPAWN_M`] from his own
    /// team's nearest spawn, the one that sees the most of the other side's route points [`LIVE_SIGHT_M`] off, then the one seen from
    /// the fewest (the hidden rule's measure); with none in the spawn band, the nearest to it ([`live_ranked`]).
    Live,
}

/// [A239] How far a live camper's spot ([`CamperAt::Live`]) lies from his own team's nearest spawn, m: the live human campers'
/// spot from their life's spawn against the old bots, median 95 m (77 m against humans); the stand-in's old rule gave 70-72 m.
const LIVE_SPAWN_M: (f32, f32) = (80.0, 100.0);

/// [A239] The sightlines a live camper's spot is chosen by ([`CamperAt::Live`]), m: the live campers' kill distance against the old
/// bots, median 16 m (p25 10, p75 25); the stand-in's old rule (any sight within [`CAMPER_SEES_M`]) killed at 20-24 m.
const LIVE_SIGHT_M: (f32, f32) = (10.0, 25.0);

/// The nearest a spawn exit spot ([`CamperAt::Exit`]) lies to the other team's spawns, m: their spawn area, which no route enters.
const EXIT_MIN_M: f32 = crate::v026g::bots::tactics::SPAWN_AREA_M;

/// The farthest, m: where the live spawn campers stood (A199 runs, the camper's spot to the victim's spawn place over 2,935 spawn kills of
/// humans: median 35 m, p25 24, p75 46; 3,611 of the old live bots' campers: median 34 m, p75 40).
const EXIT_MAX_M: f32 = 45.0;

/// Where each life of the stand-in camper starts (`SRP_BENCH_CAMPER_START`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CamperStart {
    /// On a spawn point of his team, as every body: he walks to his spot.
    #[default]
    Spawn,
    /// On his spot ([`camper_spot_at`]): a bench of what his victims do (a spawn exit held, §2), not of his walk across the map.
    Spot,
}

impl CamperStart {
    /// Unset: [`Self::Spawn`]; `spawn`, `spot`; anything else is refused.
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("spawn") => Ok(Self::Spawn),
            Some("spot") => Ok(Self::Spot),
            Some(other) => Err(format!("SRP_BENCH_CAMPER_START={other:?}: spawn or spot")),
        }
    }
}

impl CamperAt {
    /// Unset: [`Self::Free`]; `battery`, `container`, `exit`; anything else is refused.
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None => Ok(Self::Free),
            Some("battery") => Ok(Self::Battery),
            Some("container") => Ok(Self::Container),
            Some("exit") => Ok(Self::Exit),
            Some("live") => Ok(Self::Live),
            Some(other) => Err(format!("SRP_BENCH_CAMPER_AT={other:?}: battery, container, exit or live")),
        }
    }
}

/// How near a battery or a container the spot of [`CamperAt`] lies, m (an-anticamper §8.1's definition).
const AT_NEAR_M: f32 = 25.0;

/// The candidates a camper of `options` may hold: the spots of [`CamperAt`] over [`camper_candidates`], in the order he takes
/// them.
pub(crate) enum Pool {
    /// Chosen by the spot rule ([`pick_spot`]).
    ByRule(Vec<Candidate>),
    /// Best first: the spawn exit, narrowest sector of sight on it first.
    Ranked(Vec<Candidate>),
}

impl Pool {
    /// The spot he takes among the candidates `usable` lets through.
    pub(crate) fn pick(&self, rule: SpotRule, usable: impl Fn(&Candidate) -> bool) -> CamperSpot {
        match self {
            Pool::ByRule(all) => {
                let usable: Vec<Candidate> = all.iter().copied().filter(|c| usable(c)).collect();
                pick_spot(rule, &usable)
            }
            Pool::Ranked(ranked) => ranked.iter().find(|c| c.sees > 0 && usable(c)).map(|c| (c.at, c.sees)),
        }
    }
}

/// The spots of `side` for a camper of `options`: all of them, or only those near the anchors of [`CamperAt`] (none if no route
/// point of his side that sees the other side lies near the anchors).
pub(crate) fn camper_pool(tactics: &Tactics, side: i32, options: &CamperOptions) -> Pool {
    let candidates = camper_candidates(tactics, side, options.rule, options.crouch);
    let map = &tactics.world.map;
    let anchors: Vec<Vec3> = match options.at {
        CamperAt::Free => return Pool::ByRule(candidates),
        CamperAt::Live => return Pool::Ranked(live_ranked(tactics, side, options, candidates)),
        CamperAt::Battery => map.battery_spawners.iter().map(|s| s.item).collect(),
        CamperAt::Container => map.containers.iter().filter(|c| c.team == side).map(|c| c.position).collect(),
        CamperAt::Exit => map.spawns.iter().filter(|s| s.team == 1 - side).map(|s| s.position).collect(),
    };
    let near: Vec<Candidate> = candidates
        .into_iter()
        .filter(|c| {
            c.sees > 0
                && match options.at {
                    CamperAt::Exit => in_exit_band(&anchors, c.at),
                    _ => anchors.iter().any(|a| a.distance_xz(c.at) <= AT_NEAR_M),
                }
        })
        .collect();
    if options.at != CamperAt::Exit {
        return Pool::ByRule(near);
    }
    // The narrowest sector the other side's route points that see the spot lie in, as seen from it: with three at least.
    let theirs: Vec<Vec3> = tactics.routes_of(1 - side).flat_map(|r| r.points.iter().copied()).filter(|p| p.is_finite()).collect();
    let sight = &tactics.world.sight;
    let eye_up = super::super::view::still_eye(Vec3::ZERO, options.crouch);
    let mut ranked: Vec<(f32, Candidate)> = near
        .into_iter()
        .filter_map(|c| {
            let bearings: Vec<f32> = theirs
                .iter()
                .filter(|q| q.distance_xz(c.at) <= CAMPER_SEES_M)
                .filter(|&&q| super::super::aimpoint::shows(c.at + eye_up, q, false, sight, || true))
                .map(|q| (q.z - c.at.z).atan2(q.x - c.at.x))
                .collect();
            (bearings.len() >= 3).then(|| (sector_deg(bearings), c))
        })
        .collect();
    ranked.sort_by(|(a, ca), (b, cb)| a.total_cmp(b).then(cb.sees.cmp(&ca.sees)));
    Pool::Ranked(ranked.into_iter().map(|(_, c)| c).collect())
}

/// [A239] The candidates of [`CamperAt::Live`], best first: those in [`LIVE_SPAWN_M`] of his own spawns first, the rest by how far out
/// of it they lie ([`spawn_gap_m`]); then by the other side's route points they see [`LIVE_SIGHT_M`] off, more first; then by the share
/// of those seeing them seen from, fewer first (the hidden rule's measure; 0 under the spot rule `most`). A candidate that sees none
/// [`LIVE_SIGHT_M`] off is left out.
fn live_ranked(tactics: &Tactics, side: i32, options: &CamperOptions, candidates: Vec<Candidate>) -> Vec<Candidate> {
    let spawns: Vec<Vec3> = tactics.world.map.spawns.iter().filter(|s| s.team == side).map(|s| s.position).collect();
    let theirs: Vec<Vec3> = tactics.routes_of(1 - side).flat_map(|r| r.points.iter().copied()).filter(|p| p.is_finite()).collect();
    let sight = &tactics.world.sight;
    let eye_up = super::super::view::still_eye(Vec3::ZERO, options.crouch);
    let seen_share = |c: &Candidate| c.seen as f32 / c.sees.max(1) as f32;
    let mut ranked: Vec<(f32, u32, Candidate)> = candidates
        .into_iter()
        .filter(|c| c.sees > 0)
        .filter_map(|c| {
            let close = theirs
                .iter()
                .filter(|q| (LIVE_SIGHT_M.0..=LIVE_SIGHT_M.1).contains(&q.distance_xz(c.at)))
                .filter(|&&q| super::super::aimpoint::shows(c.at + eye_up, q, false, sight, || true))
                .count() as u32;
            (close > 0).then(|| (spawn_gap_m(&spawns, c.at), close, c))
        })
        .collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)).then(seen_share(&a.2).total_cmp(&seen_share(&b.2))));
    ranked.into_iter().map(|(_, _, c)| c).collect()
}

/// [A239] How far `at` lies out of [`LIVE_SPAWN_M`] from the nearest of `spawns`, m: 0 within it; infinite with no spawn.
fn spawn_gap_m(spawns: &[Vec3], at: Vec3) -> f32 {
    let nearest = spawns.iter().map(|s| s.distance_xz(at)).fold(f32::INFINITY, f32::min);
    if !nearest.is_finite() {
        return f32::INFINITY;
    }
    (LIVE_SPAWN_M.0 - nearest).max(nearest - LIVE_SPAWN_M.1).max(0.0)
}

/// Whether `at` lies [`EXIT_MIN_M`] to [`EXIT_MAX_M`] from the nearest of the other team's `spawns`.
fn in_exit_band(spawns: &[Vec3], at: Vec3) -> bool {
    let nearest = spawns.iter().map(|s| s.distance_xz(at)).fold(f32::INFINITY, f32::min);
    (EXIT_MIN_M..=EXIT_MAX_M).contains(&nearest)
}

/// The camper's first spot by [`CamperAt`] over [`camper_candidates`]; none if no route point of his side that sees the other
/// side lies near the anchors.
pub(crate) fn camper_spot_at(tactics: &Tactics, side: i32, options: &CamperOptions) -> CamperSpot {
    camper_pool(tactics, side, options).pick(options.rule, |_| true)
}

/// The width of the narrowest arc holding all of `bearings` (radians in), degrees: 360 less the widest gap round the circle.
fn sector_deg(mut bearings: Vec<f32>) -> f32 {
    bearings.sort_by(f32::total_cmp);
    let widest = bearings
        .windows(2)
        .map(|w| w[1] - w[0])
        .chain(std::iter::once(bearings[0] + std::f32::consts::TAU - bearings[bearings.len() - 1]))
        .fold(0.0, f32::max);
    (std::f32::consts::TAU - widest).to_degrees()
}

/// Out of cover this long, ms (the owner's 0.45 s, the live 0.40 s).
const OUT_MS: (f32, f32) = (400.0, 450.0);
/// From one peek's start to the next, ms (the owner's 1.2 s, the live 0.8 s).
const PERIOD_MS: (f32, f32) = (800.0, 1_200.0);
/// A peek steps this far sideways, m (0.4 s at a walk). GUESS.
const PEEK_M: f32 = 1.2;
/// Area points lie this far from the spot, m.
const AREA_NEAR_M: f32 = 3.0;
const AREA_FAR_M: f32 = 8.0;
/// At most this many area points besides the spot.
const AREA_POINTS: usize = 3;
/// The step after a kill and after a hit, m.
const AFTER_KILL_M: (f32, f32) = (2.5, 3.5);
const AFTER_HIT_M: (f32, f32) = (2.0, 2.5);
/// Health lost between two thinks that counts as a hit. GUESS.
const HIT_HEALTH: f32 = 0.02;
/// An enemy this recently known is "known", ms, and as near as this, m. GUESS.
const KNOWN_MS: u32 = 6_000;
const KNOWN_M: f32 = 60.0;
/// A crouch spell lasts this long, ms, and comes this often, ms.
const CROUCH_MS: (f32, f32) = (1_000.0, 2_000.0);
const CROUCH_EVERY_MS: (f32, f32) = (10_000.0, 16_000.0);
/// The next spot lies at least this far from the last, m (GUESS: a human relocates "elsewhere"), and, when the spots are
/// anchored ([`CamperAt`]), this far (the anchored spots all lie within [`AT_NEAR_M`] of the anchors).
pub(crate) const LEAVE_MIN_M: f32 = 25.0;
pub(crate) const ANCHORED_MIN_M: f32 = 8.0;
/// A body this near its goal has arrived, m; the motor is asked for [`ASK_M`], which is nearer, so it stops inside the arrival.
const ARRIVED_M: f32 = 0.9;
const ASK_M: f32 = 0.5;
/// A walk to an area point or a new spot that takes this long is given up, ms (an unreachable point).
const SHIFT_TIMEOUT_MS: u32 = 10_000;
const LEAVE_TIMEOUT_MS: u32 = 90_000;
/// After he is killed he goes back to the same spot with this chance (an-anticamper: 13-20 %), the median time from the death to the
/// return, s (26-42 s); the draw is log-uniform within a factor of [`RETURN_SPREAD`] either way of the median.
pub(crate) const RETURN_P: f32 = 0.165;
pub(crate) const RETURN_MEDIAN_S: f32 = 34.0;
const RETURN_SPREAD: f32 = 1.6;

/// How the moving camper moves (`SRP_BENCH_CAMPER_MOVE`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MoveOptions {
    /// `1` (an-anticamper §8.4): route points 3-8 m from the spot, 2.5-3.5 m after a kill, and he leaves 3-5 s after a stay's
    /// second kill or after the dwell (`SRP_BENCH_CAMPER_DWELL_S`: seconds he stays with fewer than two kills, a draw within 10 %).
    Area { dwell_s: f32 },
    /// `live` (A239, the camp spec of the live campers against the old bots, `traces-20261028/fx-threat-obj-camp-spec-note.md`): he
    /// holds within [`LIVE_AREA_M`] of his spot (floors on its cover side, not only route points), steps 2-5 m after a kill
    /// ([`LIVE_AFTER_KILL_M`]), and leaves a few seconds after a kill once a known enemy is within [`LIVE_LEAVE_ENEMY_M`] or after
    /// [`LIVE_LEAVE_KILLS`] kills (no dwell clock: the live campers hold until the enemy comes); each life starts on his first spot.
    Live,
}

impl MoveOptions {
    /// Unset or `0` of the move flag: none (the camper stands); `1`: [`Self::Area`]; `live`: [`Self::Live`]. Any other value is
    /// refused, as is a dwell that is not a number over zero.
    pub(crate) fn parse(flag: Option<&str>, dwell_s: Option<&str>) -> Result<Option<Self>, String> {
        match flag {
            None | Some("0") => return Ok(None),
            Some("live") => return Ok(Some(Self::Live)),
            Some("1") => {}
            Some(other) => return Err(format!("SRP_BENCH_CAMPER_MOVE={other:?}: 1, live or unset")),
        }
        let dwell_s = match dwell_s {
            None => 22.5,
            Some(text) => text
                .parse::<f32>()
                .ok()
                .filter(|s| s.is_finite() && *s > 0.0)
                .ok_or_else(|| format!("SRP_BENCH_CAMPER_DWELL_S={text:?}: seconds over 0"))?,
        };
        Ok(Some(Self::Area { dwell_s }))
    }

    /// The step after a kill, m.
    fn after_kill_m(self) -> (f32, f32) {
        match self {
            Self::Area { .. } => AFTER_KILL_M,
            Self::Live => LIVE_AFTER_KILL_M,
        }
    }
}

/// [A239] The live camper's area: floors this far from his spot, m (the live campers hold within 1.6 m of their centre, p90 3.1 m,
/// and move 2.0 m 5 s after a kill and 4.9 m 15 s after, against the old bots).
const LIVE_AREA_M: (f32, f32) = (2.0, 5.0);
/// [A239] The live camper's step after a kill, m.
const LIVE_AFTER_KILL_M: (f32, f32) = (2.0, 5.0);
/// [A239] The live camper leaves when, within [`LIVE_LEAVE_AFTER_MS`] of a kill, a known enemy is this near, m (the live campers
/// left 3.7 s after their last kill with the nearest enemy 12 m off against the old bots, 21 m against humans: the spec's 12-20 m).
const LIVE_LEAVE_ENEMY_M: f32 = 16.0;
const LIVE_LEAVE_AFTER_MS: u32 = 15_000;
/// [A239] ... a moment after that, ms: the leave decided, he goes.
const LIVE_LEAVE_DELAY_MS: (f32, f32) = (1_000.0, 3_000.0);
/// [A239] And after this many kills at a spot, 3-5 s after the last (the spec: 2-4 kills).
const LIVE_LEAVE_KILLS: u32 = 4;

/// The side of the next peek.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeekSide {
    Left,
    Right,
}

impl PeekSide {
    fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    fn sign(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
}

/// What he does inside a stay.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hold {
    /// Behind cover at the area point he is at; the next peek, if an enemy is known, at this time.
    Cover { peek_at: u32, side: PeekSide },
    /// Out at the peek point until this time.
    Out { until: u32, side: PeekSide },
    /// Walking to area point `to` after a kill or a hit, since this time.
    Shift { to: usize, since: u32 },
}

/// Where he is in his life as a camper.
#[derive(Clone, Debug)]
enum Mode {
    /// Holding a spot: the area (the spot first), the point he is at, his kills this stay and the last one's time, and when to leave
    /// (none: no clock, [`MoveOptions::Live`] until an enemy comes near after a kill).
    Stay { area: Vec<Vec3>, here: usize, kills: u32, last_kill: Option<u32>, leave_at: Option<u32>, hold: Hold },
    /// Walking to a new spot, since this time.
    Leave { to: Vec3, since: u32 },
    /// Back from the dead, waiting where he spawned until this time to walk back to `to`.
    Return { at: u32, to: Vec3 },
}

/// His stance over time: standing with a crouch spell now and then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stance {
    /// The next spell begins at this time.
    Standing { next: u32 },
    /// Crouched until this time; the next spell after.
    Crouched { until: u32, next: u32 },
}

/// The moving camper.
#[derive(Clone, Debug)]
pub(crate) struct CamperMotion {
    options: CamperOptions,
    moving: MoveOptions,
    mode: Mode,
    stance: Stance,
    /// His kills and health at the last think.
    kills: u32,
    health: f32,
    /// The life he is in (`PlaceView::spawned`), and the time of the last think of it.
    life: u32,
    last_think: u32,
    /// The spot he came from, to keep him from bouncing between two.
    previous: Option<Vec3>,
    /// [A239] [`MoveOptions::Live`]: the spot his next life starts on, chosen while he is dead ([`Self::start_after_death`]), and
    /// the life (its spawn time) it was chosen after.
    life_start: Option<Vec3>,
    chosen_after: Option<u32>,
    rng: Rng,
    /// What happened, for the notes and the tests: moves counted by cause.
    pub(crate) counts: MotionCounts,
}

/// Counted as it goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MotionCounts {
    pub(crate) peeks: u32,
    pub(crate) after_kill: u32,
    pub(crate) after_hit: u32,
    /// Stays that ended on the clock or after a hold, for a new spot.
    pub(crate) leaves: u32,
    /// Deaths after which he went back to the same spot, and after which he took another.
    pub(crate) returns: u32,
    pub(crate) moved_on: u32,
    /// Walks given up as unreachable.
    pub(crate) gave_up: u32,
    /// [A239] Leaves decided on an enemy near after a kill ([`MoveOptions::Live`]).
    pub(crate) enemy_near: u32,
    /// [A239] Lives started on another spot than the one he died at ([`CamperMotion::start_after_death`]).
    pub(crate) life_starts_elsewhere: u32,
}

/// What a think of the camper knows.
pub(crate) struct Think<'a> {
    pub(crate) now: u32,
    pub(crate) me: &'a PlaceView,
    /// The nearest enemy he knows within reach.
    pub(crate) enemy: Option<Vec3>,
    pub(crate) tactics: &'a Tactics,
    pub(crate) side: i32,
}

/// How a motion begins.
pub(crate) struct Start<'a> {
    pub(crate) options: CamperOptions,
    pub(crate) moving: MoveOptions,
    /// The run's seed: two seeds do not give the camper the same draws.
    pub(crate) seed: u32,
    pub(crate) spot: Vec3,
    pub(crate) area: Vec<Vec3>,
    pub(crate) now: u32,
    pub(crate) me: &'a PlaceView,
}

impl CamperMotion {
    pub(crate) fn new(start: Start<'_>) -> Self {
        let Start { options, moving, seed, spot, area, now, me } = start;
        let mut rng = Rng::new(0xCA3E_0000 + me.place as u64 + (u64::from(seed) << 20));
        let area = if area.is_empty() { vec![spot] } else { area };
        let leave_at = first_leave(moving, now, &mut rng);
        let hold = Hold::Cover {
            peek_at: now + rng.range(PERIOD_MS.0, PERIOD_MS.1) as u32,
            side: PeekSide::Right,
        };
        let next = now + rng.range(CROUCH_EVERY_MS.0, CROUCH_EVERY_MS.1) as u32;
        Self {
            options,
            moving,
            mode: Mode::Stay { area, here: 0, kills: 0, last_kill: None, leave_at, hold },
            stance: Stance::Standing { next },
            kills: me.kills,
            health: me.health,
            life: me.spawned,
            last_think: now,
            previous: None,
            life_start: None,
            chosen_after: None,
            rng,
            counts: MotionCounts::default(),
        }
    }

    /// The spot of the stay he is in, or the one he is going to.
    pub(crate) fn spot(&self) -> Vec3 {
        match &self.mode {
            Mode::Stay { area, .. } => area[0],
            Mode::Leave { to, .. } | Mode::Return { to, .. } => *to,
        }
    }

    /// Whether he is in the cover hold of a stay (not on a peek, a step to another point, nor on his way).
    pub(crate) fn holds_cover(&self) -> bool {
        matches!(&self.mode, Mode::Stay { hold: Hold::Cover { .. }, .. })
    }

    /// The area point, other than `here`, whose distance from it is nearest a draw in `range`.
    fn shift_to(rng: &mut Rng, area: &[Vec3], here: usize, range: (f32, f32)) -> Option<usize> {
        let want = rng.range(range.0, range.1);
        let from = area[here];
        (0..area.len()).filter(|&i| i != here).min_by(|&a, &b| {
            let off = |i: usize| (area[i].distance_xz(from) - want).abs();
            off(a).total_cmp(&off(b))
        })
    }

    /// One think: the intent for the body. A total transition over (mode, what happened this think).
    pub(crate) fn step(&mut self, t: &Think<'_>) -> Intent {
        let now = t.now;
        let killed = t.me.kills > self.kills;
        let hit = self.health - t.me.health > HIT_HEALTH;
        self.kills = t.me.kills;
        self.health = t.me.health;
        let away = now.saturating_sub(self.last_think);
        self.last_think = now;
        if t.me.spawned != self.life {
            self.life = t.me.spawned;
            self.respawned(t, away);
        }
        self.mode = match self.mode.clone() {
            Mode::Return { at, to } if now >= at => Mode::Leave { to, since: now },
            Mode::Return { at, to } => Mode::Return { at, to },
            Mode::Leave { to, .. } if t.me.feet.distance_xz(to) <= ARRIVED_M => self.arrive(t, to),
            Mode::Leave { since, .. } if now.saturating_sub(since) >= LEAVE_TIMEOUT_MS => {
                // Unreachable: he holds where he stands.
                self.counts.gave_up += 1;
                self.arrive(t, t.me.feet)
            }
            Mode::Leave { to, since } => Mode::Leave { to, since },
            Mode::Stay { area, here, kills, last_kill, leave_at, hold } => self.stay(t, Stayed { area, here, kills, last_kill, leave_at }, hold, killed, hit),
        };
        self.stance = self.next_stance(now);
        self.intent(t)
    }

    /// A new life: back to the same spot after the death with the human's chance, else a new spot; [`MoveOptions::Live`]: on the
    /// spot chosen for it while he was dead ([`Self::start_after_death`]; `SRP_BENCH_CAMPER_START=spot` puts him there, else he
    /// walks there), his first spot if none was.
    fn respawned(&mut self, t: &Think<'_>, dead_ms: u32) {
        if self.moving == MoveOptions::Live
            && let Some(to) = self.life_start.take().or_else(|| camper_spot_at(t.tactics, t.side, &self.options).map(|(at, _)| at))
        {
            self.mode = Mode::Leave { to, since: t.now };
            return;
        }
        let spot = self.spot();
        let back = self.rng.range(0.0, 1.0) < RETURN_P;
        if back {
            // The wait from the death (the last think of the old life) to the return: log-uniform about the median.
            let total = RETURN_MEDIAN_S * 1_000.0 * RETURN_SPREAD.powf(self.rng.range(-1.0, 1.0));
            let wait = (total as u32).saturating_sub(dead_ms);
            self.counts.returns += 1;
            self.mode = Mode::Return { at: t.now + wait, to: spot };
            return;
        }
        match self.next_spot(t, spot) {
            Some(to) => {
                self.counts.moved_on += 1;
                self.previous = Some(spot);
                self.mode = Mode::Leave { to, since: t.now };
            }
            // No other spot on this map for his scenario: he goes back at once.
            None => self.mode = Mode::Return { at: t.now, to: spot },
        }
    }

    fn stay(&mut self, t: &Think<'_>, stayed: Stayed, hold: Hold, killed: bool, hit: bool) -> Mode {
        let now = t.now;
        let Stayed { area, mut here, mut kills, mut last_kill, mut leave_at } = stayed;
        if killed {
            kills += 1;
            last_kill = Some(now);
            leave_at = match self.moving {
                // Two kills make it a hold; he goes when nothing new comes for a few seconds.
                MoveOptions::Area { dwell_s } => Some(
                    now + if kills >= 2 { rng_ms(&mut self.rng, 3_000.0, 5_000.0) } else { dwell_ms(dwell_s, &mut self.rng) },
                ),
                MoveOptions::Live if kills >= LIVE_LEAVE_KILLS => Some(now + rng_ms(&mut self.rng, 3_000.0, 5_000.0)),
                MoveOptions::Live => leave_at,
            };
        }
        // [A239] The live camper goes a moment after an enemy comes near within a while of his last kill.
        if self.moving == MoveOptions::Live
            && leave_at.is_none()
            && last_kill.is_some_and(|at| now.saturating_sub(at) <= LIVE_LEAVE_AFTER_MS)
            && t.enemy.is_some_and(|e| e.distance_xz(t.me.feet) <= LIVE_LEAVE_ENEMY_M)
        {
            self.counts.enemy_near += 1;
            leave_at = Some(now + rng_ms(&mut self.rng, LIVE_LEAVE_DELAY_MS.0, LIVE_LEAVE_DELAY_MS.1));
        }
        // The cause of a move, if any: a kill moves him first.
        let cause = if killed {
            Some((self.moving.after_kill_m(), true))
        } else if hit {
            Some((AFTER_HIT_M, false))
        } else {
            None
        };
        if leave_at.is_some_and(|at| now >= at) {
            return match self.next_spot(t, area[0]) {
                Some(to) => {
                    self.counts.leaves += 1;
                    self.previous = Some(area[0]);
                    Mode::Leave { to, since: now }
                }
                // No other spot: he stays and the clock restarts.
                None => {
                    let leave_at = first_leave(self.moving, now, &mut self.rng);
                    let hold = self.cover(now, PeekSide::Right);
                    Mode::Stay { area, here, kills, last_kill, leave_at, hold }
                }
            };
        }
        let hold = match hold {
            Hold::Shift { to, .. } if t.me.feet.distance_xz(area[to]) <= ARRIVED_M => {
                here = to;
                self.cover(now, PeekSide::Right)
            }
            Hold::Shift { since, .. } if now.saturating_sub(since) >= SHIFT_TIMEOUT_MS => {
                // Unreachable: he holds the point he was at.
                self.counts.gave_up += 1;
                self.cover(now, PeekSide::Right)
            }
            Hold::Shift { to, since } => Hold::Shift { to, since },
            Hold::Cover { .. } | Hold::Out { .. } if cause.is_some() => {
                let (range, kill) = cause.expect("checked");
                match Self::shift_to(&mut self.rng, &area, here, range) {
                    Some(to) => {
                        if kill {
                            self.counts.after_kill += 1;
                        } else {
                            self.counts.after_hit += 1;
                        }
                        Hold::Shift { to, since: now }
                    }
                    None => hold,
                }
            }
            Hold::Out { until, side } if now >= until => self.cover(now, side),
            Hold::Out { until, side } => Hold::Out { until, side },
            Hold::Cover { peek_at, side } if now >= peek_at && t.enemy.is_some() => {
                self.counts.peeks += 1;
                Hold::Out {
                    until: now + rng_ms(&mut self.rng, OUT_MS.0, OUT_MS.1),
                    side: side.other(),
                }
            }
            Hold::Cover { peek_at, side } => Hold::Cover { peek_at, side },
        };
        Mode::Stay { area, here, kills, last_kill, leave_at, hold }
    }

    fn next_stance(&mut self, now: u32) -> Stance {
        let in_cover = matches!(&self.mode, Mode::Stay { hold: Hold::Cover { .. }, .. });
        match self.stance {
            Stance::Standing { next } if now >= next && in_cover => {
                let until = now + rng_ms(&mut self.rng, CROUCH_MS.0, CROUCH_MS.1);
                Stance::Crouched {
                    until,
                    next: until + rng_ms(&mut self.rng, CROUCH_EVERY_MS.0, CROUCH_EVERY_MS.1),
                }
            }
            Stance::Crouched { until, next } if now >= until => Stance::Standing { next },
            other => other,
        }
    }

    fn cover(&mut self, now: u32, side: PeekSide) -> Hold {
        Hold::Cover {
            peek_at: now + rng_ms(&mut self.rng, PERIOD_MS.0 - OUT_MS.1, PERIOD_MS.1 - OUT_MS.0),
            side,
        }
    }

    /// The next spot of his scenario: the best by the rule, other than the one he came from, far enough from `from`.
    fn next_spot(&self, t: &Think<'_>, from: Vec3) -> Option<Vec3> {
        self.next_spot_in(t.tactics, t.side, from)
    }

    /// [`Self::next_spot`] on `tactics` for `side`. A live camper's spots ([`CamperAt::Live`]) are any of his kind on the map, so
    /// the next lies [`LEAVE_MIN_M`] off as with [`CamperAt::Free`]; the anchored scenarios' [`ANCHORED_MIN_M`].
    fn next_spot_in(&self, tactics: &Tactics, side: i32, from: Vec3) -> Option<Vec3> {
        let far = if matches!(self.options.at, CamperAt::Free | CamperAt::Live) { LEAVE_MIN_M } else { ANCHORED_MIN_M };
        let pool = camper_pool(tactics, side, &self.options);
        let usable = |c: &Candidate, skip_previous: bool| {
            c.at.distance_xz(from) >= far && !(skip_previous && self.previous.is_some_and(|p| p.distance_xz(c.at) < far))
        };
        pool.pick(self.options.rule, |c| usable(c, true))
            .or_else(|| pool.pick(self.options.rule, |c| usable(c, false)))
            .map(|(at, _)| at)
    }

    /// [A239] [`MoveOptions::Live`], once a life of his (spawned at `dead_life`) has ended: the spot his next life starts on, once per
    /// death (none again for the same life, nor for another motion): back to the spot he held with the live campers' chance of a
    /// return within 10 m ([`LIVE_SAME_P`] + [`LIVE_NEAR_P`], half of them), else another spot of his kind [`LEAVE_MIN_M`] or more
    /// off. With `SRP_BENCH_CAMPER_START=spot` every life began on his first spot, where the bots had just killed him: he was
    /// cleared there in 70 % of his lives (camp-live-move-replica-70ad999e).
    pub(crate) fn start_after_death(&mut self, tactics: &Tactics, side: i32, dead_life: u32) -> Option<Vec3> {
        if self.moving != MoveOptions::Live || self.chosen_after == Some(dead_life) {
            return None;
        }
        self.chosen_after = Some(dead_life);
        let last = self.spot();
        let to = if self.rng.range(0.0, 1.0) < LIVE_SAME_P + LIVE_NEAR_P {
            last
        } else {
            let next = self.next_spot_in(tactics, side, last).unwrap_or(last);
            self.previous = Some(last);
            next
        };
        self.counts.life_starts_elsewhere += u32::from(to != last);
        self.life_start = Some(to);
        Some(to)
    }

    /// At the new spot: its area, a new stay.
    fn arrive(&mut self, t: &Think<'_>, spot: Vec3) -> Mode {
        let area = area_of(self.moving, t.tactics, t.side, spot, self.options.crouch);
        let leave_at = first_leave(self.moving, t.now, &mut self.rng);
        let hold = self.cover(t.now, PeekSide::Right);
        Mode::Stay { area, here: 0, kills: 0, last_kill: None, leave_at, hold }
    }

    fn intent(&self, t: &Think<'_>) -> Intent {
        let crouch = matches!(self.stance, Stance::Crouched { .. }) && matches!(&self.mode, Mode::Stay { hold: Hold::Cover { .. }, .. });
        let to = |at: Vec3, within_m: f32, pace: Pace, straight: bool| Intent {
            go: Go::To { at, within_m },
            pace,
            straight,
            crouch,
            ..Intent::stay()
        };
        match &self.mode {
            Mode::Return { .. } => Intent::stay(),
            Mode::Leave { to: at, .. } => to(*at, ASK_M, Pace::Run, false),
            Mode::Stay { area, here, hold, .. } => match *hold {
                Hold::Cover { .. } => {
                    let at = area[*here];
                    // Close in, by keys: a step or two; far, by the route.
                    to(at, ASK_M, Pace::Walk, t.me.feet.distance_xz(at) < 2.5)
                }
                Hold::Out { side, .. } => {
                    let from = area[*here];
                    let toward = t.enemy.unwrap_or(from + Vec3::new(1.0, 0.0, 0.0));
                    let (dx, dz) = (toward.x - from.x, toward.z - from.z);
                    let len = dx.hypot(dz).max(0.01);
                    // Sideways of the line to the enemy.
                    let out = from + Vec3::new(-dz / len * PEEK_M * side.sign(), 0.0, dx / len * PEEK_M * side.sign());
                    to(out, 0.3, Pace::Walk, true)
                }
                Hold::Shift { to: i, .. } => to(area[i], ASK_M, Pace::Walk, t.me.feet.distance_xz(area[i]) < 2.5),
            },
        }
    }
}

fn rng_ms(rng: &mut Rng, low: f32, high: f32) -> u32 {
    rng.range(low, high) as u32
}

/// A stay's state that one think carries on (its [`Hold`] apart).
struct Stayed {
    area: Vec<Vec3>,
    here: usize,
    kills: u32,
    last_kill: Option<u32>,
    leave_at: Option<u32>,
}

/// When a new stay ends with nothing happening: after the dwell ([`MoveOptions::Area`]); never ([`MoveOptions::Live`]: he goes
/// when an enemy comes near after a kill, or after [`LIVE_LEAVE_KILLS`]).
fn first_leave(moving: MoveOptions, now: u32, rng: &mut Rng) -> Option<u32> {
    match moving {
        MoveOptions::Area { dwell_s } => Some(now + dwell_ms(dwell_s, rng)),
        MoveOptions::Live => None,
    }
}

/// The area of a camper of `moving` at `spot`: [`plan_area`]'s route points, or [`live_area`]'s floors.
pub(crate) fn area_of(moving: MoveOptions, tactics: &Tactics, side: i32, spot: Vec3, crouched: bool) -> Vec<Vec3> {
    match moving {
        MoveOptions::Area { .. } => plan_area(tactics, side, spot, crouched),
        MoveOptions::Live => live_area(tactics, side, spot, crouched),
    }
}

/// [A239] The live camper's area at `spot`: the spot, then walkable floors of its storey [`LIVE_AREA_M`] from it (twelve bearings at
/// the band's two ends), those the other side's route points see from the fewest places (cover side) first, kept 1.5 m apart, at
/// most [`AREA_POINTS`]. Route points alone left most spots without a point 2-5 m off.
pub(crate) fn live_area(tactics: &Tactics, side: i32, spot: Vec3, crouched: bool) -> Vec<Vec3> {
    let grid = &tactics.world.grid;
    let theirs: Vec<Vec3> = tactics.routes_of(1 - side).flat_map(|r| r.points.iter().copied()).filter(|p| p.is_finite()).collect();
    let sight = &tactics.world.sight;
    let standing = super::super::view::still_eye(Vec3::ZERO, false);
    let seen_from = |p: Vec3| {
        theirs
            .iter()
            .filter(|q| q.distance_xz(p) <= CAMPER_SEES_M)
            .filter(|&&q| super::super::aimpoint::shows(q + standing, p, crouched, sight, || true))
            .count()
    };
    let mut near: Vec<(usize, Vec3)> = Vec::new();
    for k in 0..12 {
        let angle = k as f32 * std::f32::consts::TAU / 12.0;
        for r in [LIVE_AREA_M.0, LIVE_AREA_M.1] {
            let probe = Vec3::new(spot.x + r * angle.cos(), spot.y + 1.0, spot.z + r * angle.sin());
            let Some(node) = grid.locate(probe, 1.5) else {
                continue;
            };
            let p = grid.position(node);
            if tactics.walkable(node)
                && (p.y - spot.y).abs() < 1.0
                && (LIVE_AREA_M.0 - 0.5..=LIVE_AREA_M.1 + 0.5).contains(&p.distance_xz(spot))
                && !near.iter().any(|(_, q)| q.distance_xz(p) < 1.5)
            {
                near.push((seen_from(p), p));
            }
        }
    }
    near.sort_by_key(|(seen, _)| *seen);
    let mut area = vec![spot];
    area.extend(near.into_iter().take(AREA_POINTS).map(|(_, p)| p));
    area
}

/// The dwell of a stay with fewer than two kills, ms: a draw within 10 % of `dwell_s`.
fn dwell_ms(dwell_s: f32, rng: &mut Rng) -> u32 {
    (dwell_s * 1_000.0 * rng.range(0.9, 1.1)) as u32
}

/// The area of a camper at `spot`: the spot, then the route points of his side 3-8 m from it, those the other side's route
/// points see from the fewest places (cover side) first, kept 2 m apart.
pub(crate) fn plan_area(tactics: &Tactics, side: i32, spot: Vec3, crouched: bool) -> Vec<Vec3> {
    let theirs: Vec<Vec3> = tactics
        .routes_of(1 - side)
        .flat_map(|r| r.points.iter().copied())
        .filter(|p| p.is_finite())
        .collect();
    let sight = &tactics.world.sight;
    let standing = super::super::view::still_eye(Vec3::ZERO, false);
    let seen_from = |p: Vec3| {
        theirs
            .iter()
            .filter(|q| q.distance_xz(p) <= CAMPER_SEES_M)
            .filter(|&&q| super::super::aimpoint::shows(q + standing, p, crouched, sight, || true))
            .count()
    };
    let mut near: Vec<(usize, Vec3)> = Vec::new();
    for p in tactics.routes_of(side).flat_map(|r| r.points.iter().copied()) {
        let d = p.distance_xz(spot);
        if p.is_finite()
            && (AREA_NEAR_M..=AREA_FAR_M).contains(&d)
            && (p.y - spot.y).abs() < 1.0
            && !near.iter().any(|(_, k)| k.distance_xz(p) < 2.0)
        {
            near.push((seen_from(p), p));
        }
    }
    near.sort_by_key(|(seen, _)| *seen);
    let mut area = vec![spot];
    area.extend(near.into_iter().take(AREA_POINTS).map(|(_, p)| p));
    area
}

/// The nearest enemy the bot knows within reach: a track not older than [`KNOWN_MS`], near enough and placed well enough.
pub(crate) fn nearest_known(tracks: &[Track], me: Vec3, now: u32) -> Option<Vec3> {
    tracks
        .iter()
        .filter(|t| now.saturating_sub(t.time) <= KNOWN_MS && t.error_m <= 6.0 && t.at.distance_xz(me) <= KNOWN_M)
        .map(|t| t.at)
        .min_by(|a, b| a.distance_xz(me).total_cmp(&b.distance_xz(me)))
}

/// Where a stand-in camper is in finding and holding his spot.
#[derive(Clone, Debug)]
pub(crate) enum CamperState {
    /// His side or the map is not known yet.
    Seeking,
    /// No spot on this map for his options: he plays as any bot.
    NoSpot,
    /// Holds one spot all match, as the old stand-in did.
    Still { at: Vec3, sees: u32 },
    /// Holds a spot standing, and after each death goes where a live camper goes ([`ReturnRule::Live`], [`LiveReturn`]); the spot
    /// he began at and how many route points it sees.
    Returning { start: Vec3, sees: u32, live: Box<LiveReturn> },
    /// Works an area, peeks, steps off, leaves and returns ([`CamperMotion`]); the spot he began at and how many route points it sees.
    Moving { start: Vec3, sees: u32, motion: Box<CamperMotion> },
}

/// [`ReturnRule::Live`] After a death the live camper of 6+ player matches is back within 1 m of the point he held (where a
/// trap on it would catch him) within 120 s in this share of cases: 230 of 1,070 cleared campers of 2+ kills (A145 payoff,
/// `fx-gear-trap-payoff.md`, PROVEN count, LIKELY place: reckoned tracks, 5 s worlds) ...
pub(crate) const LIVE_SAME_P: f32 = 0.215;
/// ... and within 10 m of it, but not within 1 m, in this share: 308 of 1,070 (in all 537 of 1,070 within 10 m).
pub(crate) const LIVE_NEAR_P: f32 = 0.288;
/// The median time from his death to that return, ms: back on the point P25 33 / P50 46 / P75 77 s; within 10 m 27 / 40 / 65 s.
/// A wait is drawn log-uniform within a factor of [`LIVE_SPREAD`] either way of its median (quartiles at its square root, 1.5).
const LIVE_SAME_MS: f32 = 46_000.0;
const LIVE_NEAR_MS: f32 = 40_000.0;
const LIVE_SPREAD: f32 = 2.3;

/// [`ReturnRule::Live`] The standing camper's lives: he holds a point standing; after each death he goes back to the very point
/// with [`LIVE_SAME_P`], to a point of its area 3-8 m off ([`plan_area`]) with [`LIVE_NEAR_P`], each after a wait drawn about the
/// live median during which he plays as any bot (a live camper fights on meanwhile), else at once to another spot of his
/// scenario, [`LEAVE_MIN_M`] (anchored: [`ANCHORED_MIN_M`]) or more away. The old stand-in went back to the very point at once
/// in every life, which makes a trap on it catch him each time.
#[derive(Clone, Debug)]
pub(crate) struct LiveReturn {
    hold: StillHold,
    /// The life he is in (`PlaceView::spawned`), and the time of the last think of it.
    life: u32,
    last_think: u32,
    rng: Rng,
    pub(crate) counts: ReturnCounts,
}

/// Where the live-return camper is in a life.
#[derive(Clone, Copy, Debug, PartialEq)]
enum StillHold {
    /// Holds this point, standing.
    Holding { at: Vec3 },
    /// Back from the dead: plays as any bot until `until`, then holds `to`.
    Waiting { until: u32, to: Vec3 },
}

/// What his deaths led to: back to the very point, to a point near it, to another spot; and the draws of a near point or another
/// spot that found none and went back to the very point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReturnCounts {
    pub(crate) same: u32,
    pub(crate) near: u32,
    pub(crate) elsewhere: u32,
    pub(crate) none_other: u32,
}

/// Where a death sends him ([`after_death`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Back {
    Same,
    Near,
    Elsewhere,
}

/// The live draw: `u` uniform in [0, 1).
pub(crate) fn after_death(u: f32) -> Back {
    if u < LIVE_SAME_P {
        Back::Same
    } else if u < LIVE_SAME_P + LIVE_NEAR_P {
        Back::Near
    } else {
        Back::Elsewhere
    }
}

/// Where he may go after a death from the point he held: the other points of its area ([`plan_area`]) and another spot of his
/// scenario ([`other_spot`]).
pub(crate) struct Elsewhere {
    pub(crate) near: Vec<Vec3>,
    pub(crate) other: Option<Vec3>,
}

impl LiveReturn {
    fn new(at: Vec3, seed: u32, me: &PlaceView, now: u32) -> Self {
        Self {
            hold: StillHold::Holding { at },
            life: me.spawned,
            last_think: now,
            rng: Rng::new(0xCA3E_8000 + me.place as u64 + (u64::from(seed) << 20)),
            counts: ReturnCounts::default(),
        }
    }

    /// One think at `now` of the body `me`: the point he holds now, or none while he waits (he plays as any bot). The transition
    /// over (hold, a new life, the wait over); `elsewhere` is asked only on a new life.
    fn step(&mut self, now: u32, me: &PlaceView, elsewhere: &dyn Fn(Vec3) -> Elsewhere) -> Option<Vec3> {
        let away = now.saturating_sub(self.last_think);
        self.last_think = now;
        if me.spawned != self.life {
            self.life = me.spawned;
            self.hold = self.respawned(now, away, elsewhere);
        }
        self.hold = match self.hold {
            StillHold::Waiting { until, to } if now >= until => StillHold::Holding { at: to },
            hold => hold,
        };
        match self.hold {
            StillHold::Holding { at } => Some(at),
            StillHold::Waiting { .. } => None,
        }
    }

    /// A new life at `now`, `dead_ms` after the last think of the old one: where to, and when.
    fn respawned(&mut self, now: u32, dead_ms: u32, elsewhere: &dyn Fn(Vec3) -> Elsewhere) -> StillHold {
        let at = match self.hold {
            StillHold::Holding { at } | StillHold::Waiting { to: at, .. } => at,
        };
        let back = after_death(self.rng.range(0.0, 1.0));
        let wait = |rng: &mut Rng, median_ms: f32| {
            let total = median_ms * LIVE_SPREAD.powf(rng.range(-1.0, 1.0));
            now + (total as u32).saturating_sub(dead_ms)
        };
        let to = match back {
            Back::Same => Some(at),
            Back::Near => {
                let near = elsewhere(at).near;
                (!near.is_empty()).then(|| near[(self.rng.range(0.0, near.len() as f32) as usize).min(near.len() - 1)])
            }
            Back::Elsewhere => elsewhere(at).other,
        };
        match (back, to) {
            (Back::Same, _) => {
                self.counts.same += 1;
                StillHold::Waiting { until: wait(&mut self.rng, LIVE_SAME_MS), to: at }
            }
            (Back::Near, Some(to)) => {
                self.counts.near += 1;
                StillHold::Waiting { until: wait(&mut self.rng, LIVE_NEAR_MS), to }
            }
            (Back::Elsewhere, Some(to)) => {
                self.counts.elsewhere += 1;
                StillHold::Holding { at: to }
            }
            // No other point for his scenario: back to the very point, at once.
            (_, None) => {
                self.counts.none_other += 1;
                StillHold::Holding { at }
            }
        }
    }
}

/// Another spot of his scenario than `from`: the best by the rule, far enough from it.
fn other_spot(tactics: &Tactics, side: i32, options: &CamperOptions, from: Vec3) -> Option<Vec3> {
    let far = if options.at == CamperAt::Free { LEAVE_MIN_M } else { ANCHORED_MIN_M };
    camper_pool(tactics, side, options).pick(options.rule, |c| c.at.distance_xz(from) >= far).map(|(at, _)| at)
}

/// The stand-in camper of a run (`SRP_BENCH_CAMPER=<place>`, `campers.md` K1 §4): that place's bot goes to a spot and holds it,
/// whatever its play; the reflex fires on sight.
#[derive(Clone, Debug)]
pub(crate) struct BenchCamper {
    place: Place,
    options: CamperOptions,
    seed: u32,
    /// The tuning his reflex and decisions play by: fixed, not his team's, so that a head-to-head run's two runs face one opponent.
    tuning: Arc<Tuning>,
    state: CamperState,
    /// The spot the lane was last worked out for and the point to look at along it (`Watch::Lane`).
    lane: Option<(Vec3, Option<Vec3>)>,
}

impl BenchCamper {
    pub(crate) fn new(place: Place, options: CamperOptions, seed: u32, tuning: Arc<Tuning>) -> Self {
        Self { place, options, seed, tuning, state: CamperState::Seeking, lane: None }
    }

    pub(crate) fn place(&self) -> Place {
        self.place
    }

    pub(crate) fn tuning(&self) -> &Arc<Tuning> {
        &self.tuning
    }

    /// [A239] While he is dead after the life spawned at `dead_life`: the spot his next life starts on, once per death, when he moves
    /// as a live camper ([`CamperMotion::start_after_death`]); none otherwise.
    pub(crate) fn start_after_death(&mut self, tactics: &Tactics, side: i32, dead_life: u32) -> Option<Vec3> {
        match &mut self.state {
            CamperState::Moving { motion, .. } => motion.start_after_death(tactics, side, dead_life),
            _ => None,
        }
    }

    /// The spot he began at and how many route points it sees, once found.
    pub(crate) fn first_spot(&self) -> Option<(Vec3, u32)> {
        match &self.state {
            CamperState::Seeking | CamperState::NoSpot => None,
            CamperState::Still { at, sees } => Some((*at, *sees)),
            CamperState::Returning { start, sees, .. } | CamperState::Moving { start, sees, .. } => Some((*start, *sees)),
        }
    }

    /// The log line: his place and the spot found, or `none` / `not yet`.
    pub(crate) fn line(&self) -> String {
        let spot = match &self.state {
            CamperState::Seeking => "not looked for yet".to_string(),
            CamperState::NoSpot => "none".to_string(),
            _ => format!("{:?}", self.first_spot()),
        };
        let counts = match &self.state {
            CamperState::Moving { motion, .. } => format!(" moves {:?}", motion.counts),
            CamperState::Returning { live, .. } => format!(" returns {:?}", live.counts),
            _ => String::new(),
        };
        format!("camper: place {} spot {spot}{counts}", self.place)
    }

    /// The stand-in camper's think: to its spot and held there, whatever its play. True when the bot is the camper and found its
    /// spot (its intent is set). With [`MoveOptions`] it works its area, peeks, steps off and leaves ([`CamperMotion`]); without,
    /// it stands on the spot all match, as it always did.
    pub(in crate::v026g::bots) fn drive(&mut self, due: &Due, bot: &mut Bot) -> bool {
        if bot.place != self.place {
            return false;
        }
        let Due { tactics, me, now, .. } = *due;
        if matches!(self.state, CamperState::Seeking)
            && let Some(side) = bot.side
        {
            self.state = match camper_spot_at(tactics, side, &self.options) {
                None => CamperState::NoSpot,
                Some((at, sees)) => match (self.options.moving, self.options.returns) {
                    (None, ReturnRule::Always) => CamperState::Still { at, sees },
                    (None, ReturnRule::Live) => CamperState::Returning { start: at, sees, live: Box::new(LiveReturn::new(at, self.seed, me, now)) },
                    (Some(moving), _) => CamperState::Moving {
                        start: at,
                        sees,
                        motion: Box::new(CamperMotion::new(Start {
                            options: self.options,
                            moving,
                            seed: self.seed,
                            spot: at,
                            area: area_of(moving, tactics, side, at, self.options.crouch),
                            now,
                            me,
                        })),
                    },
                },
            };
        }
        // The spot he watches the lane from, if he is where a watch is held.
        let watched = match &mut self.state {
            CamperState::Seeking | CamperState::NoSpot => return false,
            CamperState::Still { at, .. } => {
                bot.control = Driver::BenchCamper;
                bot.intent = Intent {
                    go: Go::To { at: *at, within_m: CAMPER_WITHIN_M },
                    pace: Pace::Run,
                    // Crouched once there, if it is to be.
                    crouch: self.options.crouch && me.feet.distance_xz(*at) <= CAMPER_WITHIN_M,
                    ..Intent::stay()
                };
                (me.feet.distance_xz(*at) <= CAMPER_WITHIN_M).then_some(*at)
            }
            CamperState::Returning { live, .. } => {
                let Some(side) = bot.side else {
                    return false;
                };
                let options = self.options;
                let elsewhere = |at: Vec3| Elsewhere {
                    near: plan_area(tactics, side, at, options.crouch).get(1..).map(<[Vec3]>::to_vec).unwrap_or_default(),
                    other: other_spot(tactics, side, &options, at),
                };
                let Some(at) = live.step(now, me, &elsewhere) else {
                    // Waiting to go back: he plays as any bot meanwhile, his legs his play's.
                    bot.control = Driver::Own;
                    return false;
                };
                bot.control = Driver::BenchCamper;
                bot.intent = Intent {
                    go: Go::To { at, within_m: CAMPER_WITHIN_M },
                    pace: Pace::Run,
                    crouch: self.options.crouch && me.feet.distance_xz(at) <= CAMPER_WITHIN_M,
                    ..Intent::stay()
                };
                (me.feet.distance_xz(at) <= CAMPER_WITHIN_M).then_some(at)
            }
            CamperState::Moving { motion, .. } => {
                let Some(side) = bot.side else {
                    return false;
                };
                bot.control = Driver::BenchCamper;
                let enemy = nearest_known(bot.memory.tracks(), me.feet, now);
                bot.intent = motion.step(&Think { now, me, enemy, tactics, side });
                motion.holds_cover().then(|| motion.spot())
            }
        };
        if self.options.watch == Watch::Lane
            && let (Some(spot), Some(side)) = (watched, bot.side)
            && me.speed() < AT_REST_MPS
            && let Some(look) = self.lane_look_from(tactics, side, spot)
        {
            bot.intent.look = Look::At(look);
            bot.intent.hold |= key::AIM;
        }
        true
    }

    /// The point to look at from `spot` along its lane ([`lane_look`]), worked out once a spot.
    fn lane_look_from(&mut self, tactics: &Tactics, side: i32, spot: Vec3) -> Option<Vec3> {
        match self.lane {
            Some((at, look)) if at == spot => look,
            _ => {
                let look = lane_look(tactics, side, spot, self.options.crouch);
                self.lane = Some((spot, look));
                look
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me_at(at: Vec3, kills: u32, health: f32, spawned: u32) -> PlaceView {
        PlaceView {
            place: 0,
            team: 0,
            bot: true,
            present: true,
            alive: true,
            feet: at,
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health,
            spawned,
            kills,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        }
    }

    fn options() -> CamperOptions {
        CamperOptions {
            moving: Some(MoveOptions::Area { dwell_s: 22.5 }),
            ..CamperOptions::OFF
        }
    }

    fn motion_with(options: CamperOptions, seed: u32) -> CamperMotion {
        let spot = Vec3::new(0.0, 0.0, 0.0);
        let area = vec![spot, Vec3::new(3.0, 0.0, 0.0), Vec3::new(-3.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 5.0)];
        CamperMotion::new(Start {
            options,
            moving: options.moving.expect("moving"),
            seed,
            spot,
            area,
            now: 0,
            me: &me_at(spot, 0, 1.0, 0),
        })
    }

    fn motion() -> CamperMotion {
        motion_with(options(), 1)
    }

    fn here(m: &CamperMotion) -> Vec3 {
        match &m.mode {
            Mode::Stay { area, here, .. } => area[*here],
            Mode::Leave { to, .. } | Mode::Return { to, .. } => *to,
        }
    }

    fn tactics() -> Tactics {
        let mut tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        tactics.set_routes(camper_routes());
        tactics
    }

    /// Runs `from..to` ms in 100 ms thinks with the body wherever the intent last sent it (it arrives at once), in life
    /// `life(now)`; returns the positions asked for in order (consecutive repeats once).
    fn run_lives(m: &mut CamperMotion, tactics: &Tactics, from: u32, to: u32, enemy: Option<Vec3>, mut kills: impl FnMut(u32) -> u32, life: impl Fn(u32) -> u32) -> Vec<Vec3> {
        let mut asked: Vec<Vec3> = Vec::new();
        let mut feet = here(m);
        for now in (from..to).step_by(100) {
            let t = Think { now, me: &me_at(feet, kills(now), 1.0, life(now)), enemy, tactics, side: 0 };
            let intent = m.step(&t);
            if let Go::To { at, .. } = intent.go {
                feet = at;
                if asked.last() != Some(&at) {
                    asked.push(at);
                }
            }
        }
        asked
    }

    fn run(m: &mut CamperMotion, tactics: &Tactics, from: u32, to: u32, enemy: Option<Vec3>, kills: impl FnMut(u32) -> u32) -> Vec<Vec3> {
        run_lives(m, tactics, from, to, enemy, kills, |_| 0)
    }

    /// With an enemy known he is out for 0.4-0.45 s about every second; with none he stays behind cover.
    #[test]
    fn he_peeks_about_every_second_only_while_an_enemy_is_known() {
        let tactics = tactics();
        let mut m = motion();
        run(&mut m, &tactics, 0, 10_000, Some(Vec3::new(30.0, 0.0, 0.0)), |_| 0);
        assert!((7..=13).contains(&m.counts.peeks), "{:?}", m.counts);
        let mut calm = motion();
        run(&mut calm, &tactics, 0, 10_000, None, |_| 0);
        assert_eq!(calm.counts.peeks, 0);
    }

    /// A kill moves him 2.5-3.5 m to another point of his area, and a hit 2-2.5 m.
    #[test]
    fn a_kill_and_a_hit_move_him_to_another_area_point() {
        let tactics = tactics();
        let mut m = motion();
        let from = here(&m);
        let asked = run(&mut m, &tactics, 0, 1_500, None, |now| u32::from(now >= 500));
        assert_eq!(m.counts.after_kill, 1);
        let moved = asked.last().expect("a place").distance_xz(from);
        assert!((2.4..=3.6).contains(&moved), "{moved}");
        // A hit: the health the think saw falls.
        let mut h = motion();
        h.step(&Think { now: 0, me: &me_at(here(&h), 0, 1.0, 0), enemy: None, tactics: &tactics, side: 0 });
        h.step(&Think { now: 100, me: &me_at(here(&h), 0, 0.7, 0), enemy: None, tactics: &tactics, side: 0 });
        assert_eq!(h.counts.after_hit, 1);
    }

    /// With no kill he leaves after about 22 s, and 3-5 s after a second kill with no third; the map has a next spot, so he
    /// does leave (a test that passes when he never does is no test).
    #[test]
    fn he_leaves_after_his_dwell_and_after_a_hold() {
        let tactics = tactics();
        let mut idle = motion();
        run(&mut idle, &tactics, 0, 19_000, None, |_| 0);
        assert_eq!(idle.counts.leaves, 0, "not yet");
        run(&mut idle, &tactics, 19_000, 30_000, None, |_| 0);
        assert_eq!(idle.counts.leaves, 1, "{:?}", idle.mode);
        // Two kills (2 s, 4 s), then quiet: gone within 5 s of the last, by 9.3 s.
        let mut hold = motion();
        run(&mut hold, &tactics, 0, 9_300, None, |now| u32::from(now >= 2_000) + u32::from(now >= 4_000));
        assert_eq!(hold.counts.after_kill, 2);
        assert_eq!(hold.counts.leaves, 1, "{:?}", hold.mode);
        assert!(matches!(hold.mode, Mode::Leave { .. } | Mode::Stay { .. }), "{:?}", hold.mode);
    }

    /// `SRP_BENCH_CAMPER_DWELL_S` is his dwell after a first kill too, not a fixed 20-25 s.
    #[test]
    fn the_dwell_setting_holds_after_a_first_kill() {
        let tactics = tactics();
        let quick = CamperOptions { moving: Some(MoveOptions::Area { dwell_s: 8.0 }), ..CamperOptions::OFF };
        let mut m = motion_with(quick, 1);
        // One kill at 1 s; with an 8 s dwell he is gone by 10 s (0.9 x 8 s after the kill at most 8.8 s).
        run(&mut m, &tactics, 0, 11_000, None, |now| u32::from(now >= 1_000));
        assert_eq!(m.counts.leaves, 1, "{:?}", m.mode);
        let slow = CamperOptions { moving: Some(MoveOptions::Area { dwell_s: 60.0 }), ..CamperOptions::OFF };
        let mut s = motion_with(slow, 1);
        run(&mut s, &tactics, 0, 40_000, None, |now| u32::from(now >= 1_000));
        assert_eq!(s.counts.leaves, 0, "a first kill does not shorten a 60 s dwell");
    }

    /// [A239] The live camper has no dwell clock: quiet, he holds a minute; a kill moves him 2-5 m; an enemy within 16 m within
    /// 15 s of a kill makes him leave a moment later; with no enemy near, a kill does not; four kills do.
    #[test]
    fn the_live_camper_holds_until_an_enemy_comes_near_after_a_kill() {
        let tactics = tactics();
        let live = CamperOptions { moving: Some(MoveOptions::Live), ..CamperOptions::OFF };
        let mut idle = motion_with(live, 1);
        run(&mut idle, &tactics, 0, 60_000, None, |_| 0);
        assert_eq!(idle.counts.leaves, 0, "no dwell: {:?}", idle.counts);
        // A kill at 1 s, an enemy 30 m off: he steps 2-5 m and stays.
        let mut far = motion_with(live, 1);
        let from = here(&far);
        run(&mut far, &tactics, 0, 20_000, Some(Vec3::new(30.0, 0.0, 0.0)), |now| u32::from(now >= 1_000));
        assert_eq!((far.counts.after_kill, far.counts.leaves), (1, 0), "{:?}", far.counts);
        let moved = here(&far).distance_xz(from);
        assert!((1.9..=5.1).contains(&moved), "{moved}");
        // The same kill with an enemy 10 m off: gone within 3 s of it.
        let mut near = motion_with(live, 1);
        run(&mut near, &tactics, 0, 4_200, Some(Vec3::new(10.0, 0.0, 0.0)), |now| u32::from(now >= 1_000));
        assert_eq!((near.counts.enemy_near, near.counts.leaves), (1, 1), "{:?}", near.counts);
        // An enemy near with no kill in the last 15 s: he holds.
        let mut calm = motion_with(live, 1);
        run(&mut calm, &tactics, 0, 10_000, Some(Vec3::new(10.0, 0.0, 0.0)), |_| 0);
        assert_eq!(calm.counts.leaves, 0, "{:?}", calm.counts);
        // Four kills, no enemy near: gone 3-5 s after the fourth.
        let mut busy = motion_with(live, 1);
        run(&mut busy, &tactics, 0, 9_100, Some(Vec3::new(40.0, 0.0, 0.0)), |now| (now / 1_000).min(4));
        assert_eq!(busy.counts.leaves, 1, "{:?}", busy.counts);
    }

    /// [A239] The live camper's area: his spot, then floors of its storey 2-5 m off, cover side first; each life he goes back to
    /// his first spot.
    #[test]
    fn the_live_area_is_floors_two_to_five_metres_off_and_each_life_starts_on_the_first_spot() {
        let tactics = tactics();
        let spot = Vec3::new(35.0, 0.0, 33.0);
        let area = live_area(&tactics, 0, spot, false);
        assert_eq!(area[0], spot);
        assert!(area.len() > 1, "the soak field's floor round the spot: {area:?}");
        assert!(area[1..].iter().all(|p| (1.5..=5.5).contains(&p.distance_xz(spot))), "{area:?}");
        let live = CamperOptions { moving: Some(MoveOptions::Live), ..CamperOptions::OFF };
        let (first, _) = camper_spot_at(&tactics, 0, &live).expect("a spot");
        let mut m = motion_with(live, 1);
        // A new life far from it: he heads for the first spot.
        m.step(&Think { now: 1_000, me: &me_at(Vec3::new(5.0, 0.0, 10.0), 0, 1.0, 900), enemy: None, tactics: &tactics, side: 0 });
        assert!(matches!(m.mode, Mode::Leave { to, .. } if to == first), "{:?}", m.mode);
    }

    /// [A239] While dead the live camper chooses his next life's spot once: back to his spot about half the time (the live returns
    /// within 10 m), else another 25 m or more off; the area camper chooses none.
    #[test]
    fn the_live_camper_chooses_his_next_lifes_spot_once_per_death() {
        let tactics = tactics();
        let live = CamperOptions { moving: Some(MoveOptions::Live), ..CamperOptions::OFF };
        let mut m = motion_with(live, 1);
        let first = m.start_after_death(&tactics, 0, 0).expect("a start");
        assert_eq!(m.start_after_death(&tactics, 0, 0), None, "once per death");
        assert_eq!(m.life_start, Some(first));
        let (mut back, mut elsewhere) = (0, 0);
        for seed in 0..200 {
            let mut m = motion_with(live, seed);
            let at = m.start_after_death(&tactics, 0, 0).expect("a start");
            if at == m.spot() {
                back += 1;
            } else {
                assert!(at.distance_xz(m.spot()) >= LEAVE_MIN_M, "{at:?}");
                elsewhere += 1;
            }
        }
        assert!((70..=130).contains(&back) && elsewhere > 0, "back {back}, elsewhere {elsewhere}");
        let mut area = motion();
        assert_eq!(area.start_after_death(&tactics, 0, 0), None);
    }

    /// He walks to his new spot, stops inside the arrival radius the motor is asked for, and a point he cannot reach is given up.
    #[test]
    fn he_arrives_at_a_new_spot_and_gives_up_on_an_unreachable_one() {
        let tactics = tactics();
        let mut m = motion();
        run(&mut m, &tactics, 0, 30_000, None, |_| 0);
        let Mode::Stay { area, .. } = &m.mode else {
            panic!("{:?}", m.mode);
        };
        assert!(area[0].distance_xz(Vec3::new(0.0, 0.0, 0.0)) >= LEAVE_MIN_M, "a new spot far from the old: {:?}", area[0]);
        // The motor stops at `ASK_M`, which is inside `ARRIVED_M`.
        const { assert!(ASK_M < ARRIVED_M) };
        // A body that never gets there: after the timeout he holds where he stands.
        let mut stuck = motion();
        let mut now = 0;
        let feet = Vec3::new(0.0, 0.0, 0.0);
        while now < 120_000 {
            stuck.step(&Think { now, me: &me_at(feet, 0, 1.0, 0), enemy: None, tactics: &tactics, side: 0 });
            now += 100;
        }
        assert!(stuck.counts.gave_up >= 1, "{:?}", stuck.counts);
        assert!(matches!(stuck.mode, Mode::Stay { .. }), "{:?}", stuck.mode);
    }

    /// After a death he goes back to the same spot about 16 % of the time, a median of about 34 s after it, and takes another
    /// spot otherwise.
    #[test]
    fn he_returns_after_a_death_at_the_humans_rate() {
        let tactics = tactics();
        let (mut back, mut other, mut waits) = (0u32, 0u32, Vec::new());
        for seed in 0..600u32 {
            let mut m = motion_with(options(), seed);
            let spot = here(&m);
            // Alive to 3 s, dead for 8 s, a new life from 11 s.
            run_lives(&mut m, &tactics, 0, 3_000, None, |_| 0, |_| 0);
            m.step(&Think { now: 11_000, me: &me_at(spot, 0, 1.0, 7), enemy: None, tactics: &tactics, side: 0 });
            match m.mode {
                Mode::Return { at, to } if to == spot => {
                    back += 1;
                    waits.push((at.saturating_sub(11_000) + 8_000) as f32 / 1_000.0);
                }
                Mode::Leave { to, .. } if to.distance_xz(spot) >= LEAVE_MIN_M => other += 1,
                ref mode => panic!("{mode:?}"),
            }
        }
        let share = back as f32 / (back + other) as f32;
        assert!((0.10..=0.23).contains(&share), "{share} ({back} of {})", back + other);
        waits.sort_by(f32::total_cmp);
        let median = waits[waits.len() / 2];
        assert!((26.0..=42.0).contains(&median), "{median}");
    }

    /// [`ReturnRule::Live`] After a death the standing camper goes back to the very point about 21.5 % of the time, a median of
    /// about 46 s after the death, to a point 3-8 m off about 29 %, about 40 s after, and to another spot 25 m or more away at
    /// once otherwise; he plays as any bot while he waits; the first life holds his first spot.
    #[test]
    fn the_live_returning_camper_goes_back_at_the_live_rates() {
        // His area's other points 4-5 m off and another spot 40 m off.
        let spot = Vec3::new(0.0, 0.0, 0.0);
        let elsewhere = |_: Vec3| Elsewhere { near: vec![Vec3::new(4.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 5.0)], other: Some(Vec3::new(40.0, 0.0, 0.0)) };
        let step = |live: &mut LiveReturn, now: u32, life: u32| live.step(now, &me_at(spot, 0, 1.0, life), &elsewhere);
        let (mut same, mut near, mut other, mut waits_same, mut waits_near) = (0u32, 0u32, 0u32, Vec::new(), Vec::new());
        for seed in 0..1_000u32 {
            let me = me_at(spot, 0, 1.0, 0);
            let mut live = LiveReturn::new(spot, seed, &me, 0);
            assert_eq!(step(&mut live, 1_000, 0), Some(spot), "the first life holds its spot");
            // Dead from 3 s, a new life at 11 s.
            assert_eq!(step(&mut live, 3_000, 0), Some(spot));
            let asked = step(&mut live, 11_000, 7);
            match live.hold {
                StillHold::Waiting { until, to } if to == spot => {
                    assert_eq!(asked, None, "he plays while he waits");
                    same += 1;
                    waits_same.push((until - 11_000 + 8_000) as f32 / 1_000.0);
                    assert_eq!(step(&mut live, until, 7), Some(spot));
                }
                StillHold::Waiting { until, to } => {
                    assert!((3.9..=5.1).contains(&to.distance_xz(spot)), "{to:?}");
                    near += 1;
                    waits_near.push((until - 11_000 + 8_000) as f32 / 1_000.0);
                }
                StillHold::Holding { at } => {
                    assert!(at.distance_xz(spot) >= LEAVE_MIN_M, "{at:?}");
                    assert_eq!(asked, Some(at));
                    other += 1;
                }
            }
        }
        assert_eq!(same + near + other, 1_000);
        let share = |n: u32| n as f32 / 1_000.0;
        assert!((0.17..=0.26).contains(&share(same)), "same {same}, near {near}, other {other}");
        assert!((0.24..=0.34).contains(&share(near)), "same {same}, near {near}, other {other}");
        let median = |mut w: Vec<f32>| {
            w.sort_by(f32::total_cmp);
            w[w.len() / 2]
        };
        assert!((38.0..=55.0).contains(&median(waits_same.clone())), "{:?}", median(waits_same));
        assert!((33.0..=48.0).contains(&median(waits_near.clone())), "{:?}", median(waits_near));
        assert_eq!((after_death(0.0), after_death(0.3), after_death(0.99)), (Back::Same, Back::Near, Back::Elsewhere));
    }

    /// A sector is 360 less the widest gap, round the circle's seam too.
    #[test]
    fn the_sector_the_other_side_is_seen_in_is_the_narrowest_arc() {
        assert!((sector_deg(vec![0.0, 0.1, -0.1]) - 11.459).abs() < 0.01);
        let seam = vec![3.0, -3.0];
        assert!((sector_deg(seam) - (std::f32::consts::TAU - 6.0).to_degrees()).abs() < 0.01, "across the seam: 0.28 rad");
        assert!((sector_deg(vec![0.0, std::f32::consts::PI]) - 180.0).abs() < 0.01);
    }

    /// With no anchor the free rule is the old spot; an anchor nobody is near gives no spot, and no next spot either.
    #[test]
    fn the_spot_near_an_anchor_is_among_the_old_candidates_or_none() {
        let tactics = tactics();
        let free = CamperOptions::OFF;
        assert_eq!(camper_spot_at(&tactics, 0, &free), camper_spot_by(&tactics, 0, SpotRule::Most, false));
        // The soak field has no battery spawners or containers: nothing is near.
        for at in [CamperAt::Battery, CamperAt::Container] {
            let options = CamperOptions { at, ..CamperOptions::OFF };
            assert_eq!(camper_spot_at(&tactics, 0, &options), None, "{at:?}");
            assert_eq!(camper_pool(&tactics, 0, &options).pick(options.rule, |_| true), None, "{at:?}");
        }
        // Its other side's spawns (75, 10) and (75, 20) have no route point of side 0 within 20 m, some 20-45 m off: the exit is one of them.
        let options = CamperOptions { at: CamperAt::Exit, ..CamperOptions::OFF };
        let theirs: Vec<Vec3> = tactics.world.map.spawns.iter().filter(|s| s.team == 1).map(|s| s.position).collect();
        let (at, _) = camper_spot_at(&tactics, 0, &options).expect("an exit spot 20-45 m from their spawns");
        assert!(in_exit_band(&theirs, at), "{at:?}");
        assert_eq!(camper_pool(&tactics, 0, &options).pick(options.rule, |_| true).map(|(p, _)| p), Some(at));
    }

    /// The next spot is drawn from the scenario's set and the hidden rule's: a pool that holds only some candidates never gives
    /// another, and a returned spot is allowed.
    #[test]
    fn the_next_spot_comes_from_the_scenarios_pool_by_the_rule() {
        let near = |x: f32, sees| Candidate { at: Vec3::new(x, 0.0, 0.0), sees, seen: 0 };
        let pool = Pool::ByRule(vec![near(0.0, 3), near(30.0, 5), near(60.0, 9)]);
        // From 60: the best of the others at least 25 m away is 30 (60 is too near, 0 sees fewer than 30's 5).
        let far = |from: f32| move |c: &Candidate| c.at.distance_xz(Vec3::new(from, 0.0, 0.0)) >= 25.0;
        assert_eq!(pool.pick(SpotRule::Most, far(60.0)).map(|(at, _)| at.x), Some(30.0));
        // From 30 the old spot 0 is allowed (a return): the better of 0 and 60 is 60, and with 60 taken it is 0.
        assert_eq!(pool.pick(SpotRule::Most, far(30.0)).map(|(at, _)| at.x), Some(60.0));
        assert_eq!(pool.pick(SpotRule::Most, |c| far(30.0)(c) && c.at.x < 50.0).map(|(at, _)| at.x), Some(0.0));
        // Ranked (the exit): the first that sees some and is usable.
        let ranked = Pool::Ranked(vec![near(10.0, 0), near(20.0, 2), near(40.0, 4)]);
        assert_eq!(ranked.pick(SpotRule::Most, |_| true).map(|(at, _)| at.x), Some(20.0));
        assert_eq!(ranked.pick(SpotRule::Most, |c| c.at.x > 30.0).map(|(at, _)| at.x), Some(40.0));
    }

    /// A spawn exit spot lies outside the other team's spawn area and no farther than the live spawn campers stood, by the nearest of
    /// their spawns.
    #[test]
    fn a_spawn_exit_spot_lies_twenty_to_forty_five_metres_from_their_nearest_spawn() {
        let spawns = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0)];
        assert!(!in_exit_band(&spawns, Vec3::new(10.0, 0.0, 0.0)), "inside their spawn area");
        assert!(in_exit_band(&spawns, Vec3::new(0.0, 5.0, 30.0)));
        assert!(!in_exit_band(&spawns, Vec3::new(0.0, 0.0, 50.0)), "farther than the live campers");
        assert!(!in_exit_band(&spawns, Vec3::new(85.0, 0.0, 0.0)), "the other spawn is 15 m off");
        assert!(!in_exit_band(&[], Vec3::ZERO), "no spawn of theirs: no exit");
    }

    /// [A239] A live camper's spot: 80-100 m from his own nearest spawn, else by how far out of that it lies.
    #[test]
    fn a_live_camper_spot_lies_eighty_to_a_hundred_metres_from_his_spawn() {
        let spawns = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 20.0)];
        assert_eq!(spawn_gap_m(&spawns, Vec3::new(90.0, 4.0, 0.0)), 0.0);
        assert_eq!(spawn_gap_m(&spawns, Vec3::new(80.0, 0.0, 20.0)), 0.0, "by the nearest spawn");
        assert!((spawn_gap_m(&spawns, Vec3::new(70.0, 0.0, 0.0)) - 10.0).abs() < 1e-4, "10 m short");
        assert!((spawn_gap_m(&spawns, Vec3::new(105.0, 0.0, 0.0)) - 5.0).abs() < 1e-4, "5 m past");
        assert_eq!(spawn_gap_m(&[], Vec3::ZERO), f32::INFINITY, "no spawn of his");
    }

    /// [A239] On the soak field (80 m across: no route point 80 m from a spawn) the live spot is the one nearest the band that sees the
    /// other side's routes 10-25 m off, from the old rule's candidates: side 0's north lane run on to x 45, 15 m short of side 1's.
    #[test]
    fn the_live_spot_is_the_nearest_the_band_that_sees_their_routes_close() {
        let mut tactics = tactics();
        let mut routes = camper_routes();
        routes[0].points.push(Vec3::new(45.0, 0.0, 33.0));
        tactics.set_routes(routes);
        let options = CamperOptions { at: CamperAt::Live, ..CamperOptions::OFF };
        let Pool::Ranked(ranked) = camper_pool(&tactics, 0, &options) else {
            panic!("a ranked pool");
        };
        assert!(!ranked.is_empty());
        let mine: Vec<Vec3> = tactics.world.map.spawns.iter().filter(|s| s.team == 0).map(|s| s.position).collect();
        let gaps: Vec<f32> = ranked.iter().map(|c| spawn_gap_m(&mine, c.at)).collect();
        assert!(gaps.windows(2).all(|w| w[0] <= w[1]), "nearest the band first: {gaps:?}");
        let all = camper_candidates(&tactics, 0, options.rule, options.crouch);
        assert!(ranked.iter().all(|c| all.contains(c)), "the old rule's candidates");
        let (at, _) = camper_spot_at(&tactics, 0, &options).expect("a live spot");
        assert_eq!(at, ranked[0].at);
    }

    /// An unknown value is refused, not read as the default.
    #[test]
    fn unknown_settings_are_refused() {
        assert_eq!(CamperAt::parse(None), Ok(CamperAt::Free));
        assert_eq!(CamperAt::parse(Some("exit")), Ok(CamperAt::Exit));
        assert_eq!(CamperAt::parse(Some("live")), Ok(CamperAt::Live));
        assert!(CamperAt::parse(Some("exits")).is_err());
        assert_eq!(CamperStart::parse(None), Ok(CamperStart::Spawn));
        assert_eq!(CamperStart::parse(Some("spot")), Ok(CamperStart::Spot));
        assert!(CamperStart::parse(Some("spots")).is_err());
        assert_eq!(MoveOptions::parse(None, None), Ok(None));
        assert_eq!(MoveOptions::parse(Some("0"), None), Ok(None));
        assert_eq!(MoveOptions::parse(Some("1"), Some("30")), Ok(Some(MoveOptions::Area { dwell_s: 30.0 })));
        assert_eq!(MoveOptions::parse(Some("live"), None), Ok(Some(MoveOptions::Live)));
        assert!(MoveOptions::parse(Some("yes"), None).is_err());
        assert!(MoveOptions::parse(Some("1"), Some("abc")).is_err());
        assert!(MoveOptions::parse(Some("1"), Some("-3")).is_err());
    }

    /// Two seeds do not give the camper one set of draws.
    #[test]
    fn the_run_seed_changes_his_draws() {
        let first_peek = |seed| match motion_with(options(), seed).mode {
            Mode::Stay { hold: Hold::Cover { peek_at, .. }, .. } => peek_at,
            ref other => panic!("{other:?}"),
        };
        assert_ne!(first_peek(1), first_peek(2));
        assert_eq!(first_peek(1), first_peek(1));
    }
}
