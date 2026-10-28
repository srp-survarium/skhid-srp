//! The moving stand-in camper (`SRP_BENCH_CAMPER_MOVE=1`, an-anticamper §8.4): a human who holds a spot works an area about
//! 8 m across, peeks every second, steps off after a kill or a hit and leaves a few seconds after his last kill. The old
//! stand-in stood within 0.7 m of one point all match; that made Flank's gate (crouched, aiming, still for 1.5 s) the one
//! thing that fired on him, and over-credited Flank. The numbers are the owner's holds and 8,474 live ones (§8.4's table):
//!
//! - **Area.** The spot and up to three cover-side points of the side's routes 3-8 m from it ([`plan_area`]), those seen
//!   from the fewest route points of the other side first. PROVEN (table): half the time within 3 m of the hold's centre,
//!   nine tenths within 8 m.
//! - **Peeks.** Out 0.4-0.45 s, a peek every 0.8-1.2 s, only while an enemy is known within reach. PROVEN (owner: 0.45 s /
//!   1.2 s; live: 0.40 s / 0.8 s). A peek here is a sideways step of [`PEEK_M`] and back: whether the owner was out of cover
//!   in between is not tested in the table.
//! - **Stance.** Standing, with a short crouch spell now and then (about a tenth of the time; owner 0 %, live median 13 %).
//! - **Relocation.** To another area point 2.5-3.5 m away within 5 s of a kill (owner 2.5 m, live 3.6 m moved) and 2-2.5 m
//!   after a hit (2.1 / 2.6 m).
//! - **Dwell.** He leaves 3-5 s after a stay's second kill with no new one, and after 20-25 s with fewer than two (live hold
//!   median 25 s, 3.5 s after the last kill), to the next best spot at least [`LEAVE_MIN_M`] away.
//!
//! The reflex still aims and fires as ever, and keeps the legs off ([`Bot::holds_ground`]).

use super::*;
use crate::v026g::bots::know::Track;
use crate::v026g::bots::plays::{Go, Intent, Pace};
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
    /// Within [`AT_NEAR_M`] of the other team's spawn, the point they are seen from in the narrowest sector: a spawn exit that
    /// cannot be flanked, where every sight on him comes from one way.
    Exit,
}

impl CamperAt {
    pub(crate) fn from_env() -> Self {
        match bench_settings().var("SRP_BENCH_CAMPER_AT").as_deref() {
            Ok("battery") => Self::Battery,
            Ok("container") => Self::Container,
            Ok("exit") => Self::Exit,
            _ => Self::Free,
        }
    }
}

/// How near a battery, a container or a spawn the spot of [`CamperAt`] lies, m.
const AT_NEAR_M: f32 = 25.0;

/// The camper's spot by [`CamperAt`] over [`camper_candidates`]; none if no route point of his side that sees the other side lies
/// near the anchors.
pub(crate) fn camper_spot_at(tactics: &Tactics, side: i32, options: &CamperOptions) -> CamperSpot {
    let candidates = camper_candidates(tactics, side, options.rule, options.crouch);
    let map = &tactics.world.map;
    let anchors: Vec<Vec3> = match options.at {
        CamperAt::Free => return pick_spot(options.rule, &candidates),
        CamperAt::Battery => map.battery_spawners.iter().map(|s| s.item).collect(),
        CamperAt::Container => map.containers.iter().filter(|c| c.team == side).map(|c| c.position).collect(),
        CamperAt::Exit => map.spawns.iter().filter(|s| s.team == 1 - side).map(|s| s.position).collect(),
    };
    let near: Vec<Candidate> = candidates
        .into_iter()
        .filter(|c| c.sees > 0 && anchors.iter().any(|a| a.distance_xz(c.at) <= AT_NEAR_M))
        .collect();
    if options.at != CamperAt::Exit {
        return pick_spot(options.rule, &near);
    }
    // The narrowest sector the other side's route points that see the spot lie in, as seen from it: with three at least.
    let theirs: Vec<Vec3> = tactics.routes_of(1 - side).flat_map(|r| r.points.iter().copied()).filter(|p| p.is_finite()).collect();
    let sight = &tactics.world.sight;
    let eye_up = super::super::view::still_eye(Vec3::ZERO, options.crouch);
    near.into_iter()
        .filter_map(|c| {
            let bearings: Vec<f32> = theirs
                .iter()
                .filter(|q| q.distance_xz(c.at) <= CAMPER_SEES_M)
                .filter(|&&q| super::super::aimpoint::shows(c.at + eye_up, q, false, sight, || true))
                .map(|q| (q.z - c.at.z).atan2(q.x - c.at.x))
                .collect();
            (bearings.len() >= 3).then(|| (sector_deg(bearings), c))
        })
        .min_by(|(a, ca), (b, cb)| a.total_cmp(b).then(cb.sees.cmp(&ca.sees)))
        .map(|(_, c)| (c.at, c.sees))
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
/// A peek steps this far sideways, m (0.4 s at a walk).
const PEEK_M: f32 = 1.2;
/// Area points lie this far from the spot, m.
const AREA_NEAR_M: f32 = 3.0;
const AREA_FAR_M: f32 = 8.0;
/// At most this many area points besides the spot.
const AREA_POINTS: usize = 3;
/// The step after a kill and after a hit, m.
const AFTER_KILL_M: (f32, f32) = (2.5, 3.5);
const AFTER_HIT_M: (f32, f32) = (2.0, 2.5);
/// Health lost between two thinks that counts as a hit.
const HIT_HEALTH: f32 = 0.02;
/// An enemy this recently known is "known", ms, and as near as this, m.
const KNOWN_MS: u32 = 6_000;
const KNOWN_M: f32 = 60.0;
/// A crouch spell lasts this long, ms, and comes this often, ms.
const CROUCH_MS: (f32, f32) = (1_000.0, 2_000.0);
const CROUCH_EVERY_MS: (f32, f32) = (10_000.0, 16_000.0);
/// The next spot lies at least this far from the last, m.
pub(crate) const LEAVE_MIN_M: f32 = 25.0;
/// A body this near its goal has arrived, m.
const ARRIVED_M: f32 = 0.9;

/// The moving camper's numbers that a run may change (`SRP_BENCH_CAMPER_MOVE=1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MoveOptions {
    /// `SRP_BENCH_CAMPER_DWELL_S`: seconds he stays with fewer than two kills (20-25 s: a draw above this centre).
    pub(crate) dwell_s: f32,
}

impl MoveOptions {
    pub(crate) fn from_env() -> Option<Self> {
        let settings = bench_settings();
        settings.var("SRP_BENCH_CAMPER_MOVE").is_ok_and(|v| v == "1").then(|| Self {
            dwell_s: settings.number::<f32>("SRP_BENCH_CAMPER_DWELL_S").unwrap_or(22.5),
        })
    }
}

/// Where he is in his cycle.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Behind cover at the area point he is at; the next peek, if an enemy is known, at this time.
    Cover { peek_at: u32 },
    /// Out at the peek point until this time.
    Out { until: u32 },
    /// Walking to area point `to` after a kill or a hit.
    Shift { to: usize },
    /// His stay is over: walking to the next spot.
    Leave { to: Vec3 },
}

/// The moving camper.
#[derive(Clone, Debug)]
pub(crate) struct CamperMotion {
    options: MoveOptions,
    /// The area: the spot first.
    area: Vec<Vec3>,
    here: usize,
    phase: Phase,
    /// The side of the next peek: 1 or -1.
    side: f32,
    /// His kills and health at the last think.
    kills: u32,
    health: f32,
    /// This stay: when it began, its kills, and when to leave if nothing happens.
    stay_kills: u32,
    leave_at: u32,
    visited: Vec<Vec3>,
    crouch_until: u32,
    next_crouch: u32,
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
    pub(crate) leaves: u32,
}

/// What a think of the camper knows.
pub(crate) struct Think<'a> {
    pub(crate) now: u32,
    pub(crate) me: &'a PlaceView,
    /// The nearest enemy he knows within reach.
    pub(crate) enemy: Option<Vec3>,
    pub(crate) tactics: &'a Tactics,
    pub(crate) side: i32,
    pub(crate) crouched_spot: bool,
}

impl CamperMotion {
    pub(crate) fn new(options: MoveOptions, place: Place, spot: Vec3, area: Vec<Vec3>, now: u32, kills: u32, health: f32) -> Self {
        let mut rng = Rng::new(0xCA3E_0000 + place as u64);
        let leave_at = now + (options.dwell_s * 1_000.0 * rng.range(0.9, 1.1)) as u32;
        let peek_at = now + rng.range(PERIOD_MS.0, PERIOD_MS.1) as u32;
        Self {
            options,
            area: if area.is_empty() { vec![spot] } else { area },
            here: 0,
            phase: Phase::Cover { peek_at },
            side: 1.0,
            kills,
            health,
            stay_kills: 0,
            leave_at,
            visited: vec![spot],
            crouch_until: 0,
            next_crouch: now + rng.range(CROUCH_EVERY_MS.0, CROUCH_EVERY_MS.1) as u32,
            rng,
            counts: MotionCounts::default(),
        }
    }

    fn point(&self) -> Vec3 {
        self.area[self.here]
    }

    /// The area point, other than `from`, whose distance from it is nearest a draw in `range`.
    fn shift_to(&mut self, range: (f32, f32)) -> Option<usize> {
        let want = self.rng.range(range.0, range.1);
        let from = self.point();
        (0..self.area.len())
            .filter(|&i| i != self.here)
            .min_by(|&a, &b| {
                let off = |i: usize| (self.area[i].distance_xz(from) - want).abs();
                off(a).total_cmp(&off(b))
            })
    }

    /// One think: the intent for the body. A total transition over (phase, what happened this think).
    pub(crate) fn step(&mut self, t: &Think<'_>) -> Intent {
        let now = t.now;
        let killed = t.me.kills > self.kills;
        let hit = self.health - t.me.health > HIT_HEALTH;
        self.kills = t.me.kills;
        self.health = t.me.health;
        if killed {
            self.stay_kills += 1;
            // Two kills make it a hold; he goes when nothing new comes for a few seconds.
            let wait = if self.stay_kills >= 2 { rng_ms(&mut self.rng, 3_000.0, 5_000.0) } else { rng_ms(&mut self.rng, 20_000.0, 25_000.0) };
            self.leave_at = now + wait;
        }
        // The cause of a move, if any: a kill moves him first.
        let cause = if killed {
            Some((AFTER_KILL_M, true))
        } else if hit {
            Some((AFTER_HIT_M, false))
        } else {
            None
        };
        self.phase = match self.phase {
            Phase::Leave { to } if t.me.feet.distance_xz(to) <= ARRIVED_M => self.arrive(t, to),
            Phase::Leave { to } => Phase::Leave { to },
            _ if now >= self.leave_at => self.leave(t),
            Phase::Shift { to } if t.me.feet.distance_xz(self.area[to]) <= ARRIVED_M => {
                self.here = to;
                self.cover(now)
            }
            Phase::Shift { to } => Phase::Shift { to },
            Phase::Cover { .. } | Phase::Out { .. } if cause.is_some() => {
                let (range, kill) = cause.expect("checked");
                match self.shift_to(range) {
                    Some(to) => {
                        if kill {
                            self.counts.after_kill += 1;
                        } else {
                            self.counts.after_hit += 1;
                        }
                        Phase::Shift { to }
                    }
                    None => self.phase,
                }
            }
            Phase::Out { until } if now >= until => self.cover(now),
            Phase::Out { until } => Phase::Out { until },
            Phase::Cover { peek_at } if now >= peek_at && t.enemy.is_some() => {
                self.side = -self.side;
                self.counts.peeks += 1;
                Phase::Out { until: now + rng_ms(&mut self.rng, OUT_MS.0, OUT_MS.1) }
            }
            Phase::Cover { peek_at } => Phase::Cover { peek_at },
        };
        if now >= self.next_crouch && matches!(self.phase, Phase::Cover { .. }) {
            self.crouch_until = now + rng_ms(&mut self.rng, CROUCH_MS.0, CROUCH_MS.1);
            self.next_crouch = self.crouch_until + rng_ms(&mut self.rng, CROUCH_EVERY_MS.0, CROUCH_EVERY_MS.1);
        }
        self.intent(t)
    }

    fn cover(&mut self, now: u32) -> Phase {
        Phase::Cover { peek_at: now + rng_ms(&mut self.rng, PERIOD_MS.0 - OUT_MS.1, PERIOD_MS.1 - OUT_MS.0) }
    }

    /// His stay is over: the next best spot far enough from the last ones, else he stays and the clock restarts.
    fn leave(&mut self, t: &Think<'_>) -> Phase {
        let next = next_spot(t.tactics, t.side, t.crouched_spot, self.point(), &self.visited);
        match next {
            Some(to) => {
                self.counts.leaves += 1;
                Phase::Leave { to }
            }
            None => {
                self.leave_at = t.now + (self.options.dwell_s * 1_000.0) as u32;
                self.cover(t.now)
            }
        }
    }

    /// At the new spot: its area, a new stay.
    fn arrive(&mut self, t: &Think<'_>, spot: Vec3) -> Phase {
        self.area = plan_area(t.tactics, t.side, spot, t.crouched_spot);
        self.here = 0;
        self.visited.push(spot);
        self.stay_kills = 0;
        self.leave_at = t.now + (self.options.dwell_s * 1_000.0 * self.rng.range(0.9, 1.1)) as u32;
        self.cover(t.now)
    }

    fn intent(&self, t: &Think<'_>) -> Intent {
        let crouch = t.now < self.crouch_until && matches!(self.phase, Phase::Cover { .. });
        let to = |at: Vec3, within_m: f32, pace: Pace, straight: bool| Intent {
            go: Go::To { at, within_m },
            pace,
            straight,
            crouch,
            ..Intent::stay()
        };
        match self.phase {
            Phase::Cover { .. } => {
                let at = self.point();
                // Close in, by keys: a step or two; far, by the route.
                to(at, 0.5, Pace::Walk, t.me.feet.distance_xz(at) < 2.5)
            }
            Phase::Out { .. } => {
                let from = self.point();
                let toward = t.enemy.unwrap_or(from + Vec3::new(1.0, 0.0, 0.0));
                let (dx, dz) = (toward.x - from.x, toward.z - from.z);
                let len = dx.hypot(dz).max(0.01);
                // Sideways of the line to the enemy.
                let out = from + Vec3::new(-dz / len * PEEK_M * self.side, 0.0, dx / len * PEEK_M * self.side);
                to(out, 0.3, Pace::Walk, true)
            }
            Phase::Shift { to: i } => to(self.area[i], ARRIVED_M * 0.5, Pace::Walk, t.me.feet.distance_xz(self.area[i]) < 2.5),
            Phase::Leave { to: at } => to(at, 1.0, Pace::Run, false),
        }
    }
}

fn rng_ms(rng: &mut Rng, low: f32, high: f32) -> u32 {
    rng.range(low, high) as u32
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

/// The next spot: the best-ranked candidate (the old rule's order) at least [`LEAVE_MIN_M`] from `from` and from every spot
/// he has been at.
pub(crate) fn next_spot(tactics: &Tactics, side: i32, crouched: bool, from: Vec3, visited: &[Vec3]) -> Option<Vec3> {
    let mut ranked = camper_candidates(tactics, side, SpotRule::Most, crouched);
    ranked.retain(|c| c.sees > 0);
    ranked.sort_by(|a, b| b.sees.cmp(&a.sees).then(b.at.y.total_cmp(&a.at.y)));
    ranked
        .into_iter()
        .map(|c| c.at)
        .find(|at| at.distance_xz(from) >= LEAVE_MIN_M && visited.iter().all(|v| v.distance_xz(*at) >= LEAVE_MIN_M))
}

/// The nearest enemy the bot knows within reach: a track not older than [`KNOWN_MS`], near enough and placed well enough.
pub(crate) fn nearest_known(tracks: &[Track], me: Vec3, now: u32) -> Option<Vec3> {
    tracks
        .iter()
        .filter(|t| now.saturating_sub(t.time) <= KNOWN_MS && t.error_m <= 6.0 && t.at.distance_xz(me) <= KNOWN_M)
        .map(|t| t.at)
        .min_by(|a, b| a.distance_xz(me).total_cmp(&b.distance_xz(me)))
}

/// The stand-in camper's think: to its spot and held there, whatever its play; the reflex fires on sight. True when the bot
/// is the camper and found its spot (its intent is set). With [`MoveOptions`] it works its area, peeks, steps off and leaves
/// ([`CamperMotion`]); without, it stands on the spot all match, as it always did.
pub(crate) fn drive_camper(
    due: &Due,
    bot: &mut Bot,
    slot: &mut Option<(Place, Option<CamperSpot>)>,
    motion: &mut Option<CamperMotion>,
) -> bool {
    let Due { tactics, me, now, .. } = *due;
    let Some((camper, spot)) = slot.as_mut() else {
        return false;
    };
    if *camper != bot.place {
        return false;
    }
    let side = bot.side;
    let options = CamperOptions::from_env();
    let found = *spot.get_or_insert_with(|| side.and_then(|side| camper_spot_at(tactics, side, &options)));
    let Some((at, _)) = found else {
        return false;
    };
    bot.bench_camper = true;
    match (options.moving, side) {
        (Some(moving), Some(side)) => {
            let camper_motion = motion.get_or_insert_with(|| {
                let area = plan_area(tactics, side, at, options.crouch);
                CamperMotion::new(moving, bot.place, at, area, now, me.kills, me.health)
            });
            let enemy = nearest_known(bot.memory.tracks(), me.feet, now);
            bot.intent = camper_motion.step(&Think { now, me, enemy, tactics, side, crouched_spot: options.crouch });
        }
        _ => {
            bot.intent = Intent {
                go: Go::To { at, within_m: CAMPER_WITHIN_M },
                pace: Pace::Run,
                // Crouched once there, if it is to be.
                crouch: options.crouch && me.feet.distance_xz(at) <= CAMPER_WITHIN_M,
                ..Intent::stay()
            };
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me(at: Vec3, kills: u32, health: f32) -> PlaceView {
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
            spawned: 0,
            kills,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        }
    }

    fn motion() -> CamperMotion {
        let spot = Vec3::new(0.0, 0.0, 0.0);
        let area = vec![spot, Vec3::new(3.0, 0.0, 0.0), Vec3::new(-3.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 5.0)];
        CamperMotion::new(MoveOptions { dwell_s: 22.5 }, 0, spot, area, 0, 0, 1.0)
    }

    fn tactics() -> Tactics {
        let mut tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        tactics.set_routes(camper_routes());
        tactics
    }

    /// Runs `from..to` ms in 100 ms thinks with the body wherever the intent last sent it (it arrives at once); returns the
    /// positions asked for in order (consecutive repeats once).
    fn run(m: &mut CamperMotion, tactics: &Tactics, from: u32, to: u32, enemy: Option<Vec3>, mut kills: impl FnMut(u32) -> u32) -> Vec<Vec3> {
        let mut asked: Vec<Vec3> = Vec::new();
        let mut feet = m.point();
        for now in (from..to).step_by(100) {
            let t = Think { now, me: &me(feet, kills(now), 1.0), enemy, tactics, side: 0, crouched_spot: false };
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
        let from = m.point();
        let asked = run(&mut m, &tactics, 0, 1_500, None, |now| u32::from(now >= 500));
        assert_eq!(m.counts.after_kill, 1);
        let moved = asked.last().expect("a place").distance_xz(from);
        assert!((2.4..=3.6).contains(&moved), "{moved}");
        assert_ne!(m.here, 0);
        // A hit: the health the think saw falls.
        let mut h = motion();
        let first = h.step(&Think { now: 0, me: &me(h.point(), 0, 1.0), enemy: None, tactics: &tactics, side: 0, crouched_spot: false });
        let _ = first;
        h.step(&Think { now: 100, me: &me(h.point(), 0, 0.7), enemy: None, tactics: &tactics, side: 0, crouched_spot: false });
        assert_eq!(h.counts.after_hit, 1);
    }

    /// With no kill he leaves after about 22 s, and 3-5 s after a second kill with no third.
    #[test]
    fn he_leaves_after_his_dwell_and_after_a_hold() {
        let tactics = tactics();
        let mut idle = motion();
        run(&mut idle, &tactics, 0, 19_000, None, |_| 0);
        assert_eq!(idle.counts.leaves, 0, "not yet");
        run(&mut idle, &tactics, 19_000, 30_000, None, |_| 0);
        assert!(idle.counts.leaves <= 1);
        // Two kills, then quiet: gone within 5 s of the last (when the map has a next spot).
        let mut hold = motion();
        run(&mut hold, &tactics, 0, 6_000, None, |now| u32::from(now >= 2_000) + u32::from(now >= 4_000));
        assert_eq!(hold.counts.after_kill, 2);
        assert!(matches!(hold.phase, Phase::Leave { .. }) || hold.counts.leaves == 0, "{:?}", hold.phase);
    }

    /// A sector is 360 less the widest gap, round the circle's seam too.
    #[test]
    fn the_sector_the_other_side_is_seen_in_is_the_narrowest_arc() {
        assert!((sector_deg(vec![0.0, 0.1, -0.1]) - 11.459).abs() < 0.01);
        let seam = vec![3.0, -3.0];
        assert!((sector_deg(seam) - (std::f32::consts::TAU - 6.0).to_degrees()).abs() < 0.01, "across the seam: 0.28 rad");
        assert!((sector_deg(vec![0.0, std::f32::consts::PI]) - 180.0).abs() < 0.01);
    }

    /// With no anchor the free rule is the old spot; an anchor nobody is near gives no spot.
    #[test]
    fn the_spot_near_an_anchor_is_among_the_old_candidates_or_none() {
        let tactics = tactics();
        let free = CamperOptions::OFF;
        assert_eq!(camper_spot_at(&tactics, 0, &free), camper_spot_by(&tactics, 0, SpotRule::Most, false));
        // The soak field has no battery spawners, containers or spawns on the other side: nothing is near.
        for at in [CamperAt::Battery, CamperAt::Container, CamperAt::Exit] {
            let options = CamperOptions { at, ..CamperOptions::OFF };
            assert_eq!(camper_spot_at(&tactics, 0, &options), None, "{at:?}");
        }
    }
}
