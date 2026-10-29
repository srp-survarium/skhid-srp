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
//! The reflex still aims and fires as ever, and keeps the legs off (`Bot::bench_camper`). His reflex and decisions play by a
//! fixed tuning ([`BenchCamper::tuning`], the build's defaults), not his team's: in a head-to-head run (one team ON, the other OFF,
//! sides swapped) he is then the same opponent in both runs.

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
    /// Unset: [`Self::Free`]; `battery`, `container`, `exit`; anything else is refused.
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None => Ok(Self::Free),
            Some("battery") => Ok(Self::Battery),
            Some("container") => Ok(Self::Container),
            Some("exit") => Ok(Self::Exit),
            Some(other) => Err(format!("SRP_BENCH_CAMPER_AT={other:?}: battery, container or exit")),
        }
    }
}

/// How near a battery, a container or a spawn the spot of [`CamperAt`] lies, m (an-anticamper §8.1's definition).
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
        CamperAt::Battery => map.battery_spawners.iter().map(|s| s.item).collect(),
        CamperAt::Container => map.containers.iter().filter(|c| c.team == side).map(|c| c.position).collect(),
        CamperAt::Exit => map.spawns.iter().filter(|s| s.team == 1 - side).map(|s| s.position).collect(),
    };
    let near: Vec<Candidate> = candidates
        .into_iter()
        .filter(|c| c.sees > 0 && anchors.iter().any(|a| a.distance_xz(c.at) <= AT_NEAR_M))
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

/// The moving camper's numbers that a run may change (`SRP_BENCH_CAMPER_MOVE=1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MoveOptions {
    /// `SRP_BENCH_CAMPER_DWELL_S`: seconds he stays with fewer than two kills (a draw within 10 % of this).
    pub(crate) dwell_s: f32,
}

impl MoveOptions {
    /// Unset or `0` of the move flag: none (the camper stands); `1`: on. Any other value is refused, as is a dwell that is not a
    /// number over zero.
    pub(crate) fn parse(flag: Option<&str>, dwell_s: Option<&str>) -> Result<Option<Self>, String> {
        match flag {
            None | Some("0") => return Ok(None),
            Some("1") => {}
            Some(other) => return Err(format!("SRP_BENCH_CAMPER_MOVE={other:?}: 1 or unset")),
        }
        let dwell_s = match dwell_s {
            None => 22.5,
            Some(text) => text
                .parse::<f32>()
                .ok()
                .filter(|s| s.is_finite() && *s > 0.0)
                .ok_or_else(|| format!("SRP_BENCH_CAMPER_DWELL_S={text:?}: seconds over 0"))?,
        };
        Ok(Some(Self { dwell_s }))
    }
}

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
    /// Holding a spot: the area (the spot first), the point he is at, his kills this stay and when to leave if nothing happens.
    Stay { area: Vec<Vec3>, here: usize, kills: u32, leave_at: u32, hold: Hold },
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
    dwell_s: f32,
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
        let leave_at = now + dwell_ms(moving.dwell_s, &mut rng);
        let hold = Hold::Cover {
            peek_at: now + rng.range(PERIOD_MS.0, PERIOD_MS.1) as u32,
            side: PeekSide::Right,
        };
        let next = now + rng.range(CROUCH_EVERY_MS.0, CROUCH_EVERY_MS.1) as u32;
        Self {
            options,
            dwell_s: moving.dwell_s,
            mode: Mode::Stay { area, here: 0, kills: 0, leave_at, hold },
            stance: Stance::Standing { next },
            kills: me.kills,
            health: me.health,
            life: me.spawned,
            last_think: now,
            previous: None,
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
            Mode::Stay { area, here, kills, leave_at, hold } => self.stay(t, area, here, kills, leave_at, hold, killed, hit),
        };
        self.stance = self.next_stance(now);
        self.intent(t)
    }

    /// A new life: back to the same spot after the death with the human's chance, else a new spot.
    fn respawned(&mut self, t: &Think<'_>, dead_ms: u32) {
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

    #[allow(clippy::too_many_arguments)]
    fn stay(&mut self, t: &Think<'_>, area: Vec<Vec3>, mut here: usize, mut kills: u32, mut leave_at: u32, hold: Hold, killed: bool, hit: bool) -> Mode {
        let now = t.now;
        if killed {
            kills += 1;
            // Two kills make it a hold; he goes when nothing new comes for a few seconds.
            leave_at = now
                + if kills >= 2 {
                    rng_ms(&mut self.rng, 3_000.0, 5_000.0)
                } else {
                    dwell_ms(self.dwell_s, &mut self.rng)
                };
        }
        // The cause of a move, if any: a kill moves him first.
        let cause = if killed {
            Some((AFTER_KILL_M, true))
        } else if hit {
            Some((AFTER_HIT_M, false))
        } else {
            None
        };
        if now >= leave_at {
            return match self.next_spot(t, area[0]) {
                Some(to) => {
                    self.counts.leaves += 1;
                    self.previous = Some(area[0]);
                    Mode::Leave { to, since: now }
                }
                // No other spot: he stays and the clock restarts.
                None => {
                    let leave_at = now + dwell_ms(self.dwell_s, &mut self.rng);
                    let hold = self.cover(now, PeekSide::Right);
                    Mode::Stay { area, here, kills, leave_at, hold }
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
        Mode::Stay { area, here, kills, leave_at, hold }
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
        let far = if self.options.at == CamperAt::Free { LEAVE_MIN_M } else { ANCHORED_MIN_M };
        let pool = camper_pool(t.tactics, t.side, &self.options);
        let usable = |c: &Candidate, skip_previous: bool| {
            c.at.distance_xz(from) >= far && !(skip_previous && self.previous.is_some_and(|p| p.distance_xz(c.at) < far))
        };
        pool.pick(self.options.rule, |c| usable(c, true))
            .or_else(|| pool.pick(self.options.rule, |c| usable(c, false)))
            .map(|(at, _)| at)
    }

    /// At the new spot: its area, a new stay.
    fn arrive(&mut self, t: &Think<'_>, spot: Vec3) -> Mode {
        let area = plan_area(t.tactics, t.side, spot, self.options.crouch);
        let leave_at = t.now + dwell_ms(self.dwell_s, &mut self.rng);
        let hold = self.cover(t.now, PeekSide::Right);
        Mode::Stay { area, here: 0, kills: 0, leave_at, hold }
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
    /// Works an area, peeks, steps off, leaves and returns ([`CamperMotion`]); the spot he began at and how many route points it sees.
    Moving { start: Vec3, sees: u32, motion: Box<CamperMotion> },
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
}

impl BenchCamper {
    pub(crate) fn new(place: Place, options: CamperOptions, seed: u32, tuning: Arc<Tuning>) -> Self {
        Self { place, options, seed, tuning, state: CamperState::Seeking }
    }

    pub(crate) fn place(&self) -> Place {
        self.place
    }

    pub(crate) fn tuning(&self) -> &Arc<Tuning> {
        &self.tuning
    }

    /// The spot he began at and how many route points it sees, once found.
    pub(crate) fn first_spot(&self) -> Option<(Vec3, u32)> {
        match &self.state {
            CamperState::Seeking | CamperState::NoSpot => None,
            CamperState::Still { at, sees } => Some((*at, *sees)),
            CamperState::Moving { start, sees, .. } => Some((*start, *sees)),
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
                Some((at, sees)) => match self.options.moving {
                    None => CamperState::Still { at, sees },
                    Some(moving) => CamperState::Moving {
                        start: at,
                        sees,
                        motion: Box::new(CamperMotion::new(Start {
                            options: self.options,
                            moving,
                            seed: self.seed,
                            spot: at,
                            area: plan_area(tactics, side, at, self.options.crouch),
                            now,
                            me,
                        })),
                    },
                },
            };
        }
        match &mut self.state {
            CamperState::Seeking | CamperState::NoSpot => false,
            CamperState::Still { at, .. } => {
                bot.bench_camper = true;
                bot.intent = Intent {
                    go: Go::To { at: *at, within_m: CAMPER_WITHIN_M },
                    pace: Pace::Run,
                    // Crouched once there, if it is to be.
                    crouch: self.options.crouch && me.feet.distance_xz(*at) <= CAMPER_WITHIN_M,
                    ..Intent::stay()
                };
                true
            }
            CamperState::Moving { motion, .. } => {
                let Some(side) = bot.side else {
                    return false;
                };
                bot.bench_camper = true;
                let enemy = nearest_known(bot.memory.tracks(), me.feet, now);
                bot.intent = motion.step(&Think { now, me, enemy, tactics, side });
                true
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
            moving: Some(MoveOptions { dwell_s: 22.5 }),
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
        let quick = CamperOptions { moving: Some(MoveOptions { dwell_s: 8.0 }), ..CamperOptions::OFF };
        let mut m = motion_with(quick, 1);
        // One kill at 1 s; with an 8 s dwell he is gone by 10 s (0.9 x 8 s after the kill at most 8.8 s).
        run(&mut m, &tactics, 0, 11_000, None, |now| u32::from(now >= 1_000));
        assert_eq!(m.counts.leaves, 1, "{:?}", m.mode);
        let slow = CamperOptions { moving: Some(MoveOptions { dwell_s: 60.0 }), ..CamperOptions::OFF };
        let mut s = motion_with(slow, 1);
        run(&mut s, &tactics, 0, 40_000, None, |now| u32::from(now >= 1_000));
        assert_eq!(s.counts.leaves, 0, "a first kill does not shorten a 60 s dwell");
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
        // The soak field has no battery spawners, containers or spawns on the other side: nothing is near.
        for at in [CamperAt::Battery, CamperAt::Container, CamperAt::Exit] {
            let options = CamperOptions { at, ..CamperOptions::OFF };
            assert_eq!(camper_spot_at(&tactics, 0, &options), None, "{at:?}");
            assert_eq!(camper_pool(&tactics, 0, &options).pick(options.rule, |_| true), None, "{at:?}");
        }
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

    /// An unknown value is refused, not read as the default.
    #[test]
    fn unknown_settings_are_refused() {
        assert_eq!(CamperAt::parse(None), Ok(CamperAt::Free));
        assert_eq!(CamperAt::parse(Some("exit")), Ok(CamperAt::Exit));
        assert!(CamperAt::parse(Some("exits")).is_err());
        assert_eq!(MoveOptions::parse(None, None), Ok(None));
        assert_eq!(MoveOptions::parse(Some("0"), None), Ok(None));
        assert_eq!(MoveOptions::parse(Some("1"), Some("30")), Ok(Some(MoveOptions { dwell_s: 30.0 })));
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
