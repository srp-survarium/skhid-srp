//! The owner counts of smoke (`bots/smoke.rs`), by team: the smoke grenades thrown and the clouds they made (the
//! tracker's), what the clouds took from the bots' eyes (looks at an enemy lost to smoke, sights never begun for
//! it, the aim's target lost in one, ticks of fire sprayed at the last spot seen), what went through them (hits
//! and kills whose line from the shooter's eye to the victim's chest ran through a cloud, as the clouds hide:
//! `Veil::hides` with the default tuning, a spray's apart), and the bodies' time in a cloud and deaths there.
//!
//! **Fairness** (BENCH-DECISIONS D): sightings and fire through smoke, read from outside the bots' sight code so
//! that a path that bypasses the veil shows (coordinator, 2026-10-29: the replay is the count, the team's rule):
//! - a sighting through smoke: a bot's `Seen` track refreshed this tick (its memory's) whose look, replayed as the bot
//!   made it (`smoke::tally::SeenLook`: its eye as its view had it, his feet and crouch as its view had them, the
//!   look's clock), had every part `aimpoint::look_parts` tests hidden by the clouds then (`smoke.thin_m`, no margin);
//! - a sighting with no look: a `Seen` track refreshed with no recorded look at its time (a path around the looks);
//! - a shot through smoke: a shot at a target that is not a spray, from the shooter's eye to where its last `Seen`
//!   track had him, every part a look tests hidden, standing and crouched, "clearly" ([`FAIR_MARGIN_M`], 0.5 m past
//!   the 2 m rule) and from every eye ([`FAIR_EYE_UP_M`] above and below, [`FAIR_EYE_SIDE_M`] to either side across
//!   the line), with no look of the shooter's that showed him within `smoke.spray_ms` (0.75 s, the owner's cap for
//!   fire through cover), replayed as for a sighting: a shot a tick after a clear look is fire continued, not a shot
//!   through smoke (`h2h-radio-vs-off-20261029T1852`: all 51 such shots came within 250 ms of a look);
//! - [diagnosis] the shots that test counts whatever the look, and the sightings that same five-eye margin test from the game's bodies counts at the sensing's time:
//!   where the game's bodies and the bots' view part at a cloud's rim or flat top (`h2h-radio-vs-off-20261029T1211`
//!   had 74 with one eye and no replay: 34 within 1 m of a rim, 32 skimming a top or bottom).
//!
//! Both read 0 with `smoke.sight` on and count with it off: the loop test below reads 0 / 0 on and 1500 / 131 off.
//!
//! The eyes' counts are the bots' own (`smoke::tally`, this thread's, the change over the match); the rest is
//! read from the stand-in game, the bots' memories and the tracker's clouds each tick. A smoke throw by a bot comes
//! with the throw uses (PR 2): until then the bench throws none and every count is 0 on both arms.

use std::collections::BTreeMap;

use super::*;
use crate::v026g::bots::aimpoint::Part as BodyPart;
use crate::v026g::bots::know::Source;
use crate::v026g::bots::smoke::{self, Cloud, Clouds, SmokeCounts as Thrown};
use crate::v026g::bots::tuning::SmokeTuning;

/// A shot this near a spray's tick is the spray's, ms.
const SPRAY_NEAR_MS: u32 = 150;
/// The fairness counts take a line as through smoke past this much more smoke than the bots' rule, metres: the bots
/// look from and at their view's places, which lag the game's bodies the counts read by up to a few tenths of a metre.
const FAIR_MARGIN_M: f32 = 0.5;
/// The eyes a fairness count tests besides the game's: this much higher and lower, and this much to either side across
/// the line, metres (the bot's view of its own eye against the game's: pitch, pace and crouch of the tick, and the
/// bench's ping of up to 150 ms at a run).
const FAIR_EYE_UP_M: f32 = 0.3;
const FAIR_EYE_SIDE_M: f32 = 0.6;

/// The death classes of [`SmokeCounts::top_deaths`]: within [`AFTER_SMOKE_MS`] of letting go of its own smoke, else of standing by a
/// cloud of its team's (its feet within the cloud's radius and [`BY_CLOUD_M`]: a crossing it smoked or a mate did), else neither.
const DEATH_CLASSES: [&str; 3] = ["own throw", "by a team cloud", "neither"];
const AFTER_SMOKE_MS: u32 = 10_000;
/// The anti-sniper throws' classes (the coordinator, 2026-10-30): a legit, b1 a second smoke onto a line a team cloud already
/// covered, b2 a second against the same sniper onto another crossing, c wasted (died; no crossing within its time with its
/// route still by the crossing; or its route no longer by it, the plan changed after the throw), d wrong
/// target (no sniper rifle, dead, or moved past `SNIPER_MOVED_M` from where sensed), e no line from him onto the crossing.
const SNIPER_CLASSES: [&str; 8] = [
    "a legit",
    "b1 covered already",
    "b2 another crossing",
    "c died first",
    "c not crossed",
    "c route changed",
    "d wrong target",
    "e no line",
];
const SNIPER_REACH_MS: u32 = 15_000;
const SNIPER_REACH_M: f32 = 2.5;
const SNIPER_MOVED_M: f32 = 3.0;
const SNIPER_SECOND_MS: u32 = 20_000;

/// A sniper crossing is resolved this long after it began: died to him, died otherwise, or crossed, ms.
const SNIPER_CROSSING_MS: u32 = 10_000;
const BY_CLOUD_M: f32 = 3.0;
/// A death is traded when a mate of the dead kills the killer within this, ms (`metrics.rs`'s rule).
const TRADE_MS: u32 = 5_000;

/// A death as the smoke topic keeps it, classed at the match's end: the dead, the tick, [`DEATH_CLASSES`]'s index, crowded by
/// `feel`'s rule, enemies within 30 m at the death, and whether the line from the dead's nearest living mate to the killer ran
/// through a cloud (the mates could not see him to trade).
#[derive(Clone, Copy, Debug)]
struct Death {
    place: Place,
    tick: u32,
    class: usize,
    crowded: bool,
    near: u32,
    screened: bool,
}

/// A fairness case counted: the bot, the enemy, the tick, its eye, his feet as sensed.
#[cfg(test)]
type Case = (Place, Place, u32, Vec3, Vec3);

#[cfg(test)]
thread_local! {
    /// The fairness cases counted, for a test to look at.
    static SEEN_CASES: std::cell::RefCell<Vec<Case>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The smoke counts: counted as the match runs, kept for the end.
pub(super) struct SmokeCounts {
    /// The eyes' counts on this thread when the match began (`smoke::tally::counts`).
    eyes_start: [[u64; smoke::tally::EVENTS]; 2],
    throws_start: smoke::tally::Throws,
    /// [A208] The holds for a cloud and the thrown smokes' crossings at the match's start.
    waits_start: smoke::tally::Waits,
    /// The tracker's throws and clouds at the last tick, and its clouds then; grenade keys of humans whose pockets
    /// are not known (a real cloud the model cannot make).
    thrown: Thrown,
    clouds: Vec<Cloud>,
    unknown_pockets: u32,
    /// By the shooter's team: hits and kills through smoke, a spray's, and others.
    hits_through: [u32; 2],
    spray_hits: [u32; 2],
    kills_through: [u32; 2],
    /// By the body's team: ticks alive inside a cloud (feet within its radius then), deaths there.
    in_smoke_ticks: [u64; 2],
    deaths_in_smoke: [u32; 2],
    /// Fairness, by the bot's team: `Seen` sensings through smoke, and shots at a target through smoke not a spray.
    seen_through: [u32; 2],
    fire_through: [u32; 2],
    /// [diagnosis] Shots whose line from the game's bodies is clearly behind smoke from every eye, not a spray,
    /// whatever the shooter's last look: most follow a clear look by a tick or two (continuing fire).
    fire_through_margin: [u32; 2],
    /// By bot and enemy: the last time a `Seen` track was refreshed; the tick's new sensings to check.
    seen_times: BTreeMap<(Place, Place), u32>,
    /// By bot and enemy: where its last `Seen` track had him (what its shots are aimed from).
    sensed: BTreeMap<(Place, Place), Vec3>,
    fresh_seen: Vec<(Place, crate::v026g::bots::know::Track)>,
    /// By the bot's team: `Seen` sensings no recorded look made (a path to a sighting that bypasses the looks).
    seen_unlooked: [u32; 2],
    /// By the body's team: the last tick each body stood by a cloud of its team's.
    by_team_cloud: Vec<Option<u32>>,
    /// The match's deaths, classed at its end (the top is named at the last tick, as `feel` names it).
    deaths: Vec<Death>,
    /// By team, by [not smoked, smoked]: the sniper crossings (a route ahead entering ground a sniper who outranges the bot
    /// sees, `smoke_uses`) and how they ended: [crossings, crossed alive, died to that sniper, died to another].
    sniper_crossings: [[[u32; 4]; 2]; 2],
    /// [diagnosis] The anti-sniper plans seen, classed at their begin (`Some(class)` when decided then) and resolved over
    /// `SNIPER_REACH_MS`; and the counts by team and class.
    sniper_open: Vec<(smoke::tally::SniperPlan, Option<usize>, String)>,
    sniper_seen: usize,
    sniper_classes: [[u32; 8]; 2],
    /// [diagnosis] The sensings whose line from the game's bodies is clearly behind smoke from every eye
    /// ([`Self::hides_body`]): the view's lag at a cloud's rim or flat top, not a bypass (the replay says which).
    seen_through_margin: [u32; 2],
}

/// The fairness judges' eyes: the default smoke tuning with a forming cloud hiding from the client's
/// [`smoke::CLIENT_FORMING_MS`] (`smoke.forming_client`), what a human's client lets him see through, whatever the
/// bots' own switch: a bot's sighting through a cloud the client still shows is no sighting through smoke.
fn client_eyes() -> SmokeTuning {
    SmokeTuning { forming_client: true, ..SmokeTuning::default() }
}

impl SmokeCounts {
    pub(super) fn new() -> Self {
        smoke::tally::begin_match();
        Self {
            eyes_start: smoke::tally::counts(),
            throws_start: smoke::tally::throws(),
            waits_start: smoke::tally::waits(),
            thrown: Thrown::default(),
            clouds: Vec::new(),
            unknown_pockets: 0,
            hits_through: [0; 2],
            spray_hits: [0; 2],
            kills_through: [0; 2],
            in_smoke_ticks: [0; 2],
            deaths_in_smoke: [0; 2],
            seen_through: [0; 2],
            fire_through: [0; 2],
            fire_through_margin: [0; 2],
            seen_times: BTreeMap::new(),
            sensed: BTreeMap::new(),
            fresh_seen: Vec::new(),
            seen_unlooked: [0; 2],
            by_team_cloud: Vec::new(),
            deaths: Vec::new(),
            sniper_crossings: [[[0; 4]; 2]; 2],
            sniper_open: Vec::new(),
            sniper_seen: 0,
            sniper_classes: [[0; 8]; 2],
            seen_through_margin: [0; 2],
        }
    }

    /// The clouds as a bot's eyes with the default tuning read them at `tick`, a forming cloud as the client draws it
    /// ([`client_eyes`]).
    fn hides(&self, tick: u32, a: Vec3, b: Vec3) -> bool {
        Clouds::new(self.clouds.clone()).covers(tick, &client_eyes(), a, b)
    }

    /// Whether the clouds hide every part a look tests of a body with its feet at `feet` (`aimpoint::Part::SIGHT_ORDER`,
    /// standing and crouched): a man at a cloud's edge with a part out of it is seen, as a look sees him.
    ///
    /// With [`FAIR_MARGIN_M`] more smoke than the bots' rule: the bot looks from where its view has it and at where
    /// its view has him, up to a few tenths of a metre from the game's bodies here, which at a cloud's edge moves a
    /// line across the 2 m.
    ///
    /// And from every eye the bot may have looked from ([`FAIR_EYE_UP_M`], [`FAIR_EYE_SIDE_M`]): the game's eye here and
    /// the bot's view of itself differ by its pitch, pace and crouch of the tick and the bench's ping, and a level line
    /// that skims a cloud's flat top or bottom, or its rim, is hidden from one and not the other
    /// (`h2h-radio-vs-off-20261029T1211`: of 74 counts with one eye, 34 crossed 2-4 m of smoke at a rim and 32 ran
    /// within 0.5 m of a top or bottom; every bot path to a `Seen` sighting goes through the veil).
    fn hides_body(&self, tick: u32, eye: Vec3, feet: Vec3) -> bool {
        let clouds = Clouds::new(self.clouds.clone());
        let default = client_eyes();
        let tuning = SmokeTuning {
            thin_m: crate::v026g::bots::tuning::Positive::of(default.thin_m.get() + FAIR_MARGIN_M),
            ..default
        };
        let veil = clouds.veil(tick, &tuning);
        let (dx, dz) = (feet.x - eye.x, feet.z - eye.z);
        let length = dx.hypot(dz).max(1e-3);
        let side = Vec3::new(-dz / length * FAIR_EYE_SIDE_M, 0.0, dx / length * FAIR_EYE_SIDE_M);
        let up = Vec3::new(0.0, FAIR_EYE_UP_M, 0.0);
        [eye, eye + up, eye - up, eye + side, eye - side].iter().all(|&from| {
            [false, true].iter().all(|&crouched| {
                BodyPart::SIGHT_ORDER
                    .iter()
                    .all(|part| veil.hides(from, part.point_of(feet, crouched, from)))
            })
        })
    }

    /// The bot's own look replayed (`aimpoint::look_parts` with the team's rule, `smoke.thin_m` 2 m, no margin): from its
    /// eye as its view had it, at his feet and crouch as its view had them, through the clouds at the look's clock.
    /// Whether the clouds hid every part it tests: a look that showed him while they did is a sighting through smoke.
    fn replay_hides(&self, look: &smoke::tally::SeenLook) -> bool {
        let clouds = Clouds::new(self.clouds.clone());
        let tuning = client_eyes();
        let veil = clouds.veil(look.time, &tuning);
        BodyPart::SIGHT_ORDER
            .iter()
            .all(|part| veil.hides(look.eye, part.point_of(look.feet, look.crouched, look.eye)))
    }

    /// The death of the bot at `place` at `tick`: its class, crowded or not, and whether its killer stood behind smoke from its
    /// nearest living mate.
    fn death(&mut self, run: &Run, game: &Game, place: Place, tick: u32) {
        let team = game.bodies[place].team;
        let recent = |t: Option<u32>| t.is_some_and(|t| tick.saturating_sub(t) <= AFTER_SMOKE_MS);
        let class = if recent(smoke::tally::last_threw(place)) {
            0
        } else if recent(self.by_team_cloud.get(place).copied().flatten()) {
            1
        } else {
            2
        };
        let Some((crowded, near)) = super::feel::Feel::crowded_at_death(run, game, place) else {
            return;
        };
        let killer = run.kills.iter().rev().find(|&&(_, _, victim)| victim == place).map(|&(_, killer, _)| killer);
        let dead_at = game.bodies[place].feet;
        let mate = game
            .bodies
            .iter()
            .enumerate()
            .filter(|&(p, b)| p != place && b.team == team && b.alive)
            .min_by(|a, b| a.1.feet.distance_xz(dead_at).total_cmp(&b.1.feet.distance_xz(dead_at)));
        let screened = match (killer, mate) {
            (Some(killer), Some((_, mate))) if killer < game.bodies.len() => {
                !self.clouds.is_empty() && self.hides(tick, Game::eye(mate), Game::chest(&game.bodies[killer]))
            }
            _ => false,
        };
        self.deaths.push(Death { place, tick, class, crowded, near, screened });
    }

    /// The deaths by team, top or not and class: [deaths, crowded, enemies near at the deaths, traded within [`TRADE_MS`] by a
    /// mate, killer screened by smoke from the nearest mate].
    fn deaths_by_class(&self, run: &Run, game: &Game) -> [[[[u32; 5]; 3]; 2]; 2] {
        let team = |place: Place| usize::from(game.bodies.get(place).map_or(0, |b| b.team) & 1);
        let tops = [super::feel::Feel::top_of(game, 0), super::feel::Feel::top_of(game, 1)];
        let mut by = [[[[0u32; 5]; 3]; 2]; 2];
        for d in &self.deaths {
            let t = team(d.place);
            let kill = run.kills.iter().enumerate().find(|&(_, &(k_t, _, victim))| victim == d.place && k_t.abs_diff(d.tick) <= 2 * TICK_MS);
            let traded = kill.is_some_and(|(i, &(k_t, killer, _))| {
                run.kills[i + 1..]
                    .iter()
                    .take_while(|&&(t2, ..)| t2.saturating_sub(k_t) <= TRADE_MS)
                    .any(|&(_, killer2, victim2)| victim2 == killer && killer2 != d.place && team(killer2) == t)
            });
            let who = usize::from(tops[t] != Some(d.place));
            let row = &mut by[t][who][d.class];
            row[0] += 1;
            row[1] += u32::from(d.crowded);
            row[2] += d.near;
            row[3] += u32::from(traded);
            row[4] += u32::from(d.screened);
        }
        by
    }

    /// [diagnosis] The anti-sniper plans: new ones classed at their begin, open ones resolved for the body at `at.place`.
    fn sniper_throws(&mut self, at: &BodyAt<'_>) {
        let plans = smoke::tally::sniper_plans();
        let game = at.game;
        for plan in plans.iter().skip(self.sniper_seen) {
            let him = &game.bodies[plan.who];
            let rifle = crate::v026g::bots::reflex::WeaponKind::of_class(him.gun.class) == crate::v026g::bots::reflex::WeaponKind::Sniper;
            let moved = him.feet.distance_xz(plan.eye) > SNIPER_MOVED_M;
            let chest = plan.crossing + Vec3::new(0.0, 1.3, 0.0);
            let line = crate::v026g::bots::aimpoint::sight_line(at.sight, Game::eye(him), chest);
            let covered = Clouds::new(self.clouds.clone()).planning(plan.time, plan.team, smoke::Forming::Client).covers(plan.time, &client_eyes(), plan.eye, chest);
            let second = plans.iter().any(|p| {
                p.team == plan.team && p.who == plan.who && p.time < plan.time && plan.time - p.time <= SNIPER_SECOND_MS
            });
            // [diagnosis] The two lines at the plan: from the eye the decision used, and from his eye in the game.
            let game_eye = Game::eye(him);
            let from_plan = crate::v026g::bots::aimpoint::sight_line(at.sight, plan.eye, chest);
            let diag = format!(
                "plan eye ({:.2},{:.2},{:.2}) line {from_plan}; game eye ({:.2},{:.2},{:.2}) line {line}; crouched {} pitch {:.2} feet y {:.2}",
                plan.eye.x, plan.eye.y, plan.eye.z, game_eye.x, game_eye.y, game_eye.z,
                him.keys & crate::v026g::drive::key::CROUCH != 0, him.pitch, him.feet.y
            );
            let decided = if !him.alive || !rifle || moved {
                Some(6)
            } else if !line {
                Some(7)
            } else if covered {
                Some(1)
            } else {
                None
            };
            self.sniper_open.push((*plan, decided.or(second.then_some(usize::MAX)), diag));
        }
        self.sniper_seen = plans.len();
        // Resolve the open plans of the body this record is for.
        let place = at.place;
        let (tick, alive, feet) = (at.tick, at.rec.alive, at.rec.feet);
        let mut kept = Vec::new();
        for (plan, decided, diag) in std::mem::take(&mut self.sniper_open) {
            if plan.place != place {
                kept.push((plan, decided, diag));
                continue;
            }
            let reached = alive && feet.distance_xz(plan.crossing) <= SNIPER_REACH_M;
            let over = tick.saturating_sub(plan.time) >= SNIPER_REACH_MS;
            let route_left = plans.iter().find(|p| p.place == plan.place && p.time == plan.time).and_then(|p| p.route_left);
            let class = match decided {
                Some(c) if c != usize::MAX => Some(c),
                _ if !alive && tick > plan.time => Some(3),
                _ if reached => Some(if decided == Some(usize::MAX) { 2 } else { 0 }),
                _ if over => Some(if route_left.is_some() { 5 } else { 4 }),
                _ => None,
            };
            match class {
                Some(c) => {
                    self.sniper_classes[usize::from(plan.team & 1)][c] += 1;
                    if crate::v026g::bots::tests::bench_settings().flag("SRP_BENCH_SMOKE_FAIR_DUMP") {
                        let him = &game.bodies[plan.who];
                        eprintln!(
                            "SNIPER-THROW team {} bot {} at {} class {} sniper {} alive {} gun {:?} moved {:.1} m crossing ({:.1},{:.1},{:.1}) resolved {} ms route left {:?}; {diag}",
                            plan.team, plan.place, plan.time, SNIPER_CLASSES[c], plan.who, him.alive, him.gun.class,
                            him.feet.distance_xz(plan.eye), plan.crossing.x, plan.crossing.y, plan.crossing.z, tick.saturating_sub(plan.time),
                            route_left.map(|(t, why)| (t.saturating_sub(plan.time), why))
                        );
                    }
                }
                None => kept.push((plan, decided, diag)),
            }
        }
        self.sniper_open = kept;
    }

    /// Whether `feet` stand inside a cloud at `tick`.
    fn inside(&self, tick: u32, feet: Vec3) -> bool {
        let at = smoke::MatchMs(tick);
        self.clouds.iter().any(|c| c.holds(feet, at))
    }

    /// The line from `shooter`'s eye to `victim`'s chest, and the shooter's team.
    fn line(game: &Game, shooter: Place, victim: Place) -> (Vec3, Vec3, usize) {
        let (s, v) = (&game.bodies[shooter], &game.bodies[victim]);
        (Game::eye(s), Game::chest(v), usize::from(s.team & 1))
    }

    /// Whether `shooter` sprayed at `tick` (`smoke::tally::sprayed`).
    fn sprayed(shooter: Place, tick: u32) -> bool {
        smoke::tally::last_spray(shooter).is_some_and(|t| t.abs_diff(tick) <= SPRAY_NEAR_MS)
    }

    /// The tick's hits, kills and shots through smoke, by the shooter's team.
    fn through(&mut self, game: &Game) {
        if self.clouds.is_empty() {
            return;
        }
        let tick = game.tick;
        for &(t, shooter, victim) in &game.hit_log {
            let (eye, chest, team) = Self::line(game, shooter, victim);
            if t == tick && self.hides(tick, eye, chest) {
                if Self::sprayed(shooter, tick) {
                    self.spray_hits[team] += 1;
                } else {
                    self.hits_through[team] += 1;
                }
            }
        }
        for &(t, shooter, victim) in &game.kill_log {
            let (eye, chest, team) = Self::line(game, shooter, victim);
            if t == tick && shooter != victim && self.hides(tick, eye, chest) {
                self.kills_through[team] += 1;
            }
        }
        for fact in &game.shot_facts {
            if let Some((target, _, _)) = fact.target {
                let (eye, _, team) = Self::line(game, fact.shooter, target);
                // Where the shooter's eyes have him (its memory's sight), else the game's body.
                let feet = self.sensed.get(&(fact.shooter, target)).copied().unwrap_or(game.bodies[target].feet);
                if fact.tick == tick && self.hides_body(tick, eye, feet) && !Self::sprayed(fact.shooter, tick) {
                    self.fire_through_margin[team] += 1;
                    // Fair: the shooter's last look that found him, replayed with the team's rule, showed him within
                    // the spray's time (`smoke.spray_ms`, the owner's cap for fire through cover): fire continued.
                    let look = smoke::tally::seen_look(fact.shooter, target);
                    let spray_ms = SmokeTuning::default().spray_ms;
                    let fair = look.is_some_and(|l| tick.saturating_sub(l.time) <= spray_ms && !self.replay_hides(&l));
                    self.fire_through[team] += u32::from(!fair);
                    if crate::v026g::bots::tests::bench_settings().flag("SRP_BENCH_SMOKE_FAIR_DUMP") {
                        // The shooter's last sight of him: its age, and whether the clouds hid him then too.
                        let seen = self.seen_times.get(&(fact.shooter, target)).copied();
                        let hidden_then = seen.map(|t| self.hides_body(t, eye, feet));
                        let spray = smoke::tally::last_spray(fact.shooter).map(|t| tick as i64 - t as i64);
                        eprintln!(
                            "FAIR-SHOT bot {} (team {team}) at {target} tick {tick} seen {:?} (age {:?} ms) hidden-then {:?} spray-ago {:?} ms sensed {} eye ({:.1},{:.1},{:.1}) at ({:.1},{:.1},{:.1}) look-age {:?} look-replay-hidden {:?} counted {}",
                            fact.shooter,
                            seen,
                            seen.map(|t| tick.saturating_sub(t)),
                            hidden_then,
                            spray,
                            self.sensed.contains_key(&(fact.shooter, target)),
                            eye.x, eye.y, eye.z, feet.x, feet.y, feet.z,
                            look.map(|l| tick.saturating_sub(l.time)),
                            look.map(|l| self.replay_hides(&l)),
                            !fair,
                        );
                    }
                    #[cfg(test)]
                    if !fair {
                        SEEN_CASES.with(|c| c.borrow_mut().push((fact.shooter, target, tick, eye, feet)));
                    }
                }
            }
        }
    }

    /// The bot at `place`'s `Seen` tracks refreshed since the last tick: kept to check against the game's bodies.
    fn read_seen(&mut self, place: Place, tracks: &[crate::v026g::bots::know::Track]) {
        for track in tracks.iter().filter(|t| t.source == Source::Seen) {
            let last = self.seen_times.insert((place, track.who), track.time);
            self.sensed.insert((place, track.who), track.at);
            if last != Some(track.time) {
                self.fresh_seen.push((place, *track));
            }
        }
    }

    /// The tick's fresh `Seen` sensings of the bot at `place` whose every part the clouds hide: from its eye to where
    /// it sensed him (the track's place, the view's: the game's body is later than what the bot saw).
    fn check_seen(&mut self, game: &Game, place: Place, tick: u32) {
        let mine: Vec<crate::v026g::bots::know::Track> =
            self.fresh_seen.iter().filter(|(p, _)| *p == place).map(|(_, track)| *track).collect();
        self.fresh_seen.retain(|(p, _)| *p != place);
        let me = &game.bodies[place];
        for track in mine {
            let (who, at, time) = (track.who, track.at, track.time);
            let (eye, team) = (Game::eye(me), usize::from(me.team & 1));
            if !game.bodies[who].alive {
                continue;
            }
            // The look that made this sensing, as the bot made it; none is a path that bypassed the looks.
            let look = smoke::tally::seen_look(place, who).filter(|look| look.time == time);
            let replay = look.as_ref().map(|look| self.replay_hides(look));
            // [diagnosis] With the game's bodies, every eye and the margin, at the sensing's time.
            let margin = self.hides_body(time, eye, at);
            self.seen_through_margin[team] += u32::from(margin);
            let counted = match replay {
                None => {
                    self.seen_unlooked[team] += 1;
                    true
                }
                Some(hidden) => {
                    self.seen_through[team] += u32::from(hidden);
                    hidden
                }
            };
            if (counted || margin) && crate::v026g::bots::tests::bench_settings().flag("SRP_BENCH_SMOKE_FAIR_DUMP") {
                let near: Vec<String> = self
                    .clouds
                    .iter()
                    .filter(|c| c.centre.distance_xz(at) < 40.0 || c.centre.distance_xz(eye) < 40.0)
                    .map(|c| {
                        format!(
                            "cloud by {} team {} centre ({:.1},{:.1},{:.1}) pours {} r(tick) {:?} r(sense) {:?}",
                            c.by, c.team, c.centre.x, c.centre.y, c.centre.z, c.pours.0,
                            c.radius_at(smoke::MatchMs(tick)), c.radius_at(smoke::MatchMs(time))
                        )
                    })
                    .collect();
                eprintln!(
                    "FAIR-SEEN bot {place} (team {team}) saw {who} tick {tick} sensed {time} (lag {} ms) replay {} margin {margin} look {:?} sight_since {:?} held_until {} missed {:?} eye ({:.1},{:.1},{:.1}) at ({:.1},{:.1},{:.1}) body ({:.1},{:.1},{:.1}); {}",
                    tick.saturating_sub(time),
                    match replay { None => "unlooked", Some(true) => "hidden", Some(false) => "clear" },
                    look.map(|l| (l.eye, l.feet, l.crouched)),
                    track.sight_since,
                    track.held_until,
                    track.missed,
                    eye.x, eye.y, eye.z, at.x, at.y, at.z,
                    game.bodies[who].feet.x, game.bodies[who].feet.y, game.bodies[who].feet.z,
                    near.join(" | ")
                );
            }
            if counted {
                #[cfg(test)]
                SEEN_CASES.with(|c| c.borrow_mut().push((place, who, tick, eye, at)));
            }
        }
    }

    fn part(&self, deaths: &[[[[u32; 5]; 3]; 2]; 2]) -> Part {
        let now = smoke::tally::counts();
        let eyes: [[u64; smoke::tally::EVENTS]; 2] =
            std::array::from_fn(|team| std::array::from_fn(|e| now[team][e] - self.eyes_start[team][e]));
        let by = |e: smoke::tally::Event| [eyes[0][e as usize], eyes[1][e as usize]];
        use smoke::tally::Event::{
            AimLost, ClaimRefused, CloudThere, EdgeWatch, LearntHeard, LearntSeen, LearntTold, NotSeen, Outranged, OutrangedAny, OutrangedNoPlan, SightLost, Spray,
            SprintSkip, UnderCloud, WatchBusy,
        };
        let secs = |ticks: u64| (10.0 * ticks as f64 * f64::from(TICK_S)).round() / 10.0;
        let in_smoke_s = [secs(self.in_smoke_ticks[0]), secs(self.in_smoke_ticks[1])];
        // The bots' smoke throws, by team and use: [begun, thrown, given up]; and the thrown by team and role.
        let now_throws = smoke::tally::throws();
        let roles = survarium::v026g::loadouts::BotRole::ALL;
        let uses = smoke::SmokeUse::ALL;
        let throw = |team: usize, role: usize, u: usize, step: usize| {
            now_throws[team][role][u][step] - self.throws_start[team][role][u][step]
        };
        let by_use: [[[u64; smoke::tally::THROW_STEPS]; smoke::SmokeUse::ALL.len()]; 2] = std::array::from_fn(|team| {
            std::array::from_fn(|u| std::array::from_fn(|step| (0..roles.len()).map(|r| throw(team, r, u, step)).sum()))
        });
        let thrown_by_role: [[u64; survarium::v026g::loadouts::BotRole::ALL.len()]; 2] =
            std::array::from_fn(|team| std::array::from_fn(|r| (0..uses.len()).map(|u| throw(team, r, u, 1)).sum()));
        let uses_json: serde_json::Map<String, serde_json::Value> = uses
            .iter()
            .map(|u| (format!("{u:?}"), serde_json::json!([by_use[0][u.index()], by_use[1][u.index()]])))
            .collect();
        let roles_json: serde_json::Map<String, serde_json::Value> = roles
            .iter()
            .enumerate()
            .map(|(r, role)| (role.name().to_owned(), serde_json::json!([thrown_by_role[0][r], thrown_by_role[1][r]])))
            .collect();
        // By class: [team 0's, team 1's], each [deaths, crowded, enemies near at the deaths, traded, screened].
        let classes = |who: usize| -> serde_json::Map<String, serde_json::Value> {
            DEATH_CLASSES
                .iter()
                .enumerate()
                .map(|(c, name)| ((*name).to_owned(), serde_json::json!([deaths[0][who][c], deaths[1][who][c]])))
                .collect()
        };
        let by_who = |who: usize| -> [[[u32; 5]; 3]; 2] { [deaths[0][who], deaths[1][who]] };
        // [A208] The holds for a cloud and the thrown smokes' crossings, by team, each by whose cloud ([CrossLane,
        // AntiSniper, ObjectiveActor, Breach, mates]).
        let now_waits = smoke::tally::waits();
        let waits = |what: smoke::tally::Wait| -> serde_json::Value {
            let w = what as usize;
            let team = |t: usize| -> Vec<u64> { (0..smoke::WaitsFor::COUNT).map(|f| now_waits[t][f][w] - self.waits_start[t][f][w]).collect() };
            serde_json::json!([team(0), team(1)])
        };
        let json = serde_json::json!({
            "smoke_bot_throws_by_use": uses_json,
            "smoke_bot_thrown_by_role": roles_json,
            "smoke_thrown": self.thrown.thrown,
            "smoke_clouds": self.thrown.clouds,
            "smoke_dropped": self.thrown.dropped,
            "smoke_unknown_pockets": self.unknown_pockets,
            "smoke_bodies_seen": self.thrown.bodies_seen,
            "smoke_moved_dm": self.thrown.moved_dm,
            "smoke_sight_lost": by(SightLost),
            "smoke_not_seen": by(NotSeen),
            "smoke_aim_lost": by(AimLost),
            "smoke_spray_ticks": by(Spray),
            "smoke_edge_watches": by(EdgeWatch),
            "smoke_watches_busy": by(WatchBusy),
            "smoke_learnt_seen": by(LearntSeen),
            "smoke_learnt_heard": by(LearntHeard),
            "smoke_learnt_told": by(LearntTold),
            "smoke_outranged_planned": by(Outranged),
            "smoke_outranged_no_plan": by(OutrangedNoPlan),
            "smoke_outranged_any": by(OutrangedAny),
            "smoke_plans_cloud_there": by(CloudThere),
            "smoke_plans_claim_refused": by(ClaimRefused),
            "smoke_crossings_under_cloud": by(UnderCloud),
            "smoke_plans_skipped_sprinting": by(SprintSkip),
            "smoke_hits_through": self.hits_through,
            "smoke_spray_hits": self.spray_hits,
            "smoke_kills_through": self.kills_through,
            "smoke_in_s": in_smoke_s,
            "smoke_deaths_in": self.deaths_in_smoke,
            "smoke_fair_seen_through": self.seen_through,
            "smoke_fair_fire_through": self.fire_through,
            "smoke_fair_seen_unlooked": self.seen_unlooked,
            "smoke_fair_seen_through_margin": self.seen_through_margin,
            "smoke_fair_fire_through_margin": self.fire_through_margin,
            "smoke_sniper_crossings": self.sniper_crossings,
            "smoke_top_deaths_by_class": classes(0),
            "smoke_rest_deaths_by_class": classes(1),
        });
        let mut json = json;
        if let Some(map) = json.as_object_mut() {
            use smoke::tally::Wait::{Begun, Cap, Close, CrossedHidden, CrossedNever, CrossedOpen, Died, Fight, HeldMs, Hidden, Hit};
            for (key, what) in [
                ("smoke_hold_begun", Begun),
                ("smoke_hold_hidden", Hidden),
                ("smoke_hold_fight", Fight),
                ("smoke_hold_hit", Hit),
                ("smoke_hold_close", Close),
                ("smoke_hold_cap", Cap),
                ("smoke_hold_died", Died),
                ("smoke_hold_ms", HeldMs),
                ("smoke_crossed_hidden", CrossedHidden),
                ("smoke_crossed_open", CrossedOpen),
                ("smoke_crossed_never", CrossedNever),
            ] {
                map.insert(key.to_owned(), waits(what));
            }
        }
        let text = format!(
            "owner-test smoke (by team 0, 1): bots' smoke throws [begun, thrown, given up, died] by use {:?}, thrown by role {:?}; thrown {:?} clouds {:?} dropped {:?} (grenade keys with pockets unknown {}), placed by a world's grenade {:?} moved {:?} dm in all; looks lost to smoke {:?}, sights not begun for it {:?}, aim's target lost in it {:?}, spray ticks {:?}, smoke edges watched {:?} (busy {:?}), clouds learnt seen {:?} heard {:?} told {:?}, outranged by a seen sniper with a smoke: planned {:?} no plan {:?} (outranged at all {:?}); spot rules: plans refused, smoke there {:?}, a mate's claim {:?}, crossings under a cloud {:?}; plans skipped while sprinting {:?}; through smoke hits {:?} (spray {:?}) kills {:?}; time in smoke {:?} s, deaths there {:?}; FAIR (0 with smoke.sight on): seen through smoke (the bot's look replayed) {:?}, seen with no look {:?}, fire through smoke not a spray with no look within the spray's time {:?}; [diagnosis] seen with every eye clearly behind smoke from the game's bodies {:?}, fire so {:?}; anti-sniper throws by class {:?}: {:?}; sniper crossings by [not smoked, smoked] [crossings, crossed alive, died to him, died to another] {:?}; deaths [deaths, crowded, enemies near, traded, killer screened] by class {:?}: the tops' {:?}, the others' {:?}",
            uses.iter().map(|u| (format!("{u:?}"), [by_use[0][u.index()], by_use[1][u.index()]])).collect::<Vec<_>>(),
            roles.iter().enumerate().map(|(r, role)| (role.name(), [thrown_by_role[0][r], thrown_by_role[1][r]])).collect::<Vec<_>>(),
            self.thrown.thrown,
            self.thrown.clouds,
            self.thrown.dropped,
            self.unknown_pockets,
            self.thrown.bodies_seen,
            self.thrown.moved_dm,
            by(SightLost),
            by(NotSeen),
            by(AimLost),
            by(Spray),
            by(EdgeWatch),
            by(WatchBusy),
            by(LearntSeen),
            by(LearntHeard),
            by(LearntTold),
            by(Outranged),
            by(OutrangedNoPlan),
            by(OutrangedAny),
            by(CloudThere),
            by(ClaimRefused),
            by(UnderCloud),
            by(SprintSkip),
            self.hits_through,
            self.spray_hits,
            self.kills_through,
            in_smoke_s,
            self.deaths_in_smoke,
            self.seen_through,
            self.seen_unlooked,
            self.fire_through,
            self.seen_through_margin,
            self.fire_through_margin,
            SNIPER_CLASSES,
            self.sniper_classes,
            self.sniper_crossings,
            DEATH_CLASSES,
            by_who(0),
            by_who(1),
        );
        Part::new(json, text)
    }
}

impl Topic for SmokeCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.thrown = bots.tracker.smoke_counts();
        self.unknown_pockets = bots.tracker.throw_counts().no_pockets;
        self.clouds = bots.tracker.clouds().to_vec();
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(bot) = bot {
            self.read_seen(place, bot.memory.tracks());
        }
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        self.through(game);
    }

    fn body(&mut self, run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        self.sniper_throws(at);
        let died = at.prev.alive && !at.rec.alive;
        if died {
            self.death(run, at.game, at.place, at.tick);
        }
        // A sniper crossing resolved: the bot died (to him or another) or its time ran out alive.
        let tick = at.tick;
        if let Some((who, _, smoked)) =
            smoke::tally::take_sniper_crossing(at.place, |(_, began, _)| died || tick.saturating_sub(began) >= SNIPER_CROSSING_MS)
        {
            let team = usize::from(at.game.bodies[at.place].team & 1);
            let killer = run.kills.iter().rev().find(|&&(_, _, victim)| victim == at.place).map(|&(_, killer, _)| killer);
            let outcome = match (died, killer) {
                (false, _) => 1,
                (true, Some(killer)) if killer == who => 2,
                (true, _) => 3,
            };
            let row = &mut self.sniper_crossings[team][usize::from(smoked)];
            row[0] += 1;
            row[outcome] += 1;
        }
        if self.clouds.is_empty() {
            self.fresh_seen.retain(|(p, _)| *p != at.place);
            return;
        }
        let team = at.game.bodies[at.place].team;
        let by_cloud = at.rec.alive
            && self.clouds.iter().any(|c| {
                c.team == team
                    && c.radius_at(smoke::MatchMs(at.tick)).is_some_and(|r| c.centre.distance_xz(at.rec.feet) <= r + BY_CLOUD_M)
            });
        if by_cloud {
            if self.by_team_cloud.len() <= at.place {
                self.by_team_cloud.resize(at.place + 1, None);
            }
            self.by_team_cloud[at.place] = Some(at.tick);
        }
        self.check_seen(at.game, at.place, at.tick);
        let body = &at.game.bodies[at.place];
        let team = usize::from(body.team & 1);
        if at.rec.alive && self.inside(at.tick, at.rec.feet) {
            self.in_smoke_ticks[team] += 1;
        }
        if at.prev.alive && !at.rec.alive && self.inside(at.tick, at.prev.feet) {
            self.deaths_in_smoke[team] += 1;
        }
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let deaths = self.deaths_by_class(run, game);
        vec![("smoke", self.part(&deaths))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::know::Track;
    use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade};

    fn cloud(x: f32, z: f32) -> Cloud {
        Cloud {
            by: 1,
            team: 1,
            placed: crate::v026g::bots::smoke::Placed::Arc,
            centre: Vec3::new(x, 0.0, z),
            thrown: MatchMs(0),
            pours: MatchMs(0),
            shape: SmokeGrenade::RDG_M.cloud,
        }
    }

    /// A cloud between a shooter of team 0 and a victim of team 1 counts the hit and the kill through it, by the
    /// shooter's team, a spray's hit apart; the victim stood in it. One aside counts nothing.
    #[test]
    fn hits_and_kills_through_a_cloud_count_for_the_shooters_team() {
        let (mut game, _) = made_up(&[0, 1]);
        let mut counts = SmokeCounts::new();
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 30.0, 10.0, 0);
        game.tick = 5_000;
        game.hit_log = vec![(5_000, 0, 1)];
        game.kill_log = vec![(5_000, 0, 1)];
        counts.clouds = vec![cloud(60.0, 60.0)];
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.kills_through), ([0, 0], [0, 0]));
        counts.clouds = vec![cloud(28.0, 10.0)];
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.kills_through), ([1, 0], [1, 0]));
        // The same hit while the shooter sprays is a spray's hit.
        smoke::tally::sprayed(0, 4_950);
        game.kill_log.clear();
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.spray_hits), ([1, 0], [1, 0]));
        assert!(counts.inside(5_000, game.bodies[1].feet) && !counts.inside(5_000, game.bodies[0].feet));
        let part = counts.part(&[[[[0; 5]; 3]; 2]; 2]);
        assert!(part.text.contains("through smoke hits [1, 0] (spray [1, 0]) kills [1, 0]"), "{}", part.text);
    }

    /// A shot at a target behind a cloud counts as fire through smoke unless the shooter sprays then.
    #[test]
    fn a_shot_through_a_cloud_counts_unless_it_is_a_spray() {
        use crate::v026g::bots::tests::tallies::fight_tally::ShotFact;
        let (mut game, _) = made_up(&[0, 1]);
        let mut counts = SmokeCounts::new();
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 30.0, 10.0, 0);
        counts.clouds = vec![cloud(20.0, 10.0)];
        game.tick = 6_000;
        game.shot_facts = vec![ShotFact {
            shooter: 0,
            tick: 6_000,
            hit: None,
            target: Some((1, 20.0, 0.0)),
            moving: false,
            aimed: true,
            crouched: false,
            aim_state: Default::default(),
            cone: 0.0,
            offset: None,
            rise: None,
            landed: Default::default(),
        }];
        counts.through(&game);
        assert_eq!(counts.fire_through, [1, 0]);
        // A look that showed him 100 ms before (he stood clear of the cloud then): fire continued, not counted.
        let clear = smoke::tally::SeenLook { time: 5_900, eye: Game::eye(&game.bodies[0]), feet: Vec3::new(10.0, 0.0, 40.0), crouched: false };
        smoke::tally::looked_seen(0, 1, clear);
        counts.through(&game);
        assert_eq!((counts.fire_through, counts.fire_through_margin), ([1, 0], [2, 0]), "a clear look 100 ms before");
        // The same look 1 s before: past the spray's time, counted.
        smoke::tally::looked_seen(0, 1, smoke::tally::SeenLook { time: 5_000, ..clear });
        counts.through(&game);
        assert_eq!(counts.fire_through, [2, 0], "a look 1 s before");
        smoke::tally::sprayed(0, 6_000);
        counts.through(&game);
        assert_eq!(counts.fire_through, [2, 0], "a spray's shot");
        counts.clouds = vec![cloud(20.0, 40.0)];
        smoke::tally::sprayed(0, 0);
        counts.through(&game);
        assert_eq!(counts.fire_through, [2, 0], "a clear line");
    }

    /// The fairness counts in a real loop (the stand-in game, the bots' poll, the owner tally): 2 against 2 for 60 s on
    /// the soak field, three clouds that never end across its open passage (x 36-44, z 24-41; the wall closes the rest).
    /// With `smoke.sight` off the bots see and fire through them, so the counts read above 0 (they count); on, 0.
    fn fairness_in_a_loop(sight: bool, seed: u64) -> ([u32; 2], [u32; 2]) {
        use std::sync::Arc;

        use crate::v026g::bots::smoke::{CloudShape, Placed};
        use crate::v026g::bots::trace::Trace;
        use crate::v026g::bots::tuning::Tuning;
        use crate::v026g::bots::view::Clock;
        use crate::v026g::drive::half_value;
        let teams = [0u8, 0, 1, 1];
        let world = crate::v026g::bots::tests::soak_world();
        let mut tuning = Tuning::default();
        tuning.smoke.sight = sight;
        let mut bots = Bots::new(
            0x5E0E + seed as u32,
            survarium::v026g::maps::mode::TEAM_DEATHMATCH,
            0,
            &teams,
            &[true; 4],
            &[None; 4],
            Arc::new(tuning),
            Trace::none(0),
        );
        bots.map_ready(world.clone());
        let mut game = Game::new(world, &teams, seed, Some(50), None);
        for place in 0..teams.len() {
            bots.entered(place, true, 0);
        }
        let shape = CloudShape {
            forming_ms: 0,
            full_ms: 600_000,
            gone_ms: 600_001,
            ..SmokeGrenade::RDG_M.cloud
        };
        for x in [36.0, 40.0, 44.0] {
            bots.tracker.add_cloud(Cloud {
                by: 0,
                team: 0,
                placed: Placed::Arc,
                centre: Vec3::new(x, 0.0, 32.5),
                thrown: MatchMs(0),
                pours: MatchMs(0),
                shape,
            });
        }
        let mut owner = OwnerTally::new(teams.len());
        let (mut clock, mut next_world) = (0, 1_000);
        while clock < 60_000 {
            clock += 25;
            while game.tick + TICK_MS <= clock {
                game.step();
                owner.tick(&mut game, &bots);
                game.shot_facts.clear();
                game.hit_log.clear();
            }
            if clock >= next_world {
                next_world += 1_000;
                if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                    for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                        bots.fixed(place, fix, Some(health));
                    }
                    bots.facts(facts);
                }
            }
            let (made, _) = bots.poll(Clock {
                clock,
                time_left_s: (60_000 - clock) as f32 / 1000.0,
                total_s: 60.0,
                playing: true,
            });
            for made in made {
                let r = made.input.rotation;
                let turn = (
                    half_value(u16::from_le_bytes([r[0], r[1]])),
                    half_value(u16::from_le_bytes([r[2], r[3]])),
                );
                let bot = bots.bots[made.place].as_ref();
                let target = bot.and_then(|bot| bot.reflex.target());
                let gun = bot.map_or(crate::v026g::bots::guns::Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
                game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
            }
        }
        let counts = owner.topic::<SmokeCounts>();
        SEEN_CASES.with(|c| {
            for case in c.borrow_mut().drain(..) {
                println!("seen through (sight {sight}): {case:?}");
            }
        });
        let seen = [counts.seen_through[0] + counts.seen_unlooked[0], counts.seen_through[1] + counts.seen_unlooked[1]];
        (seen, counts.fire_through)
    }

    #[test]
    fn in_a_real_loop_the_fairness_counts_read_0_with_smoke_on_and_count_with_it_off() {
        let (mut off_seen, mut off_fire) = (0, 0);
        for seed in [1, 2] {
            let (seen, fire) = fairness_in_a_loop(false, seed);
            off_seen += seen[0] + seen[1];
            off_fire += fire[0] + fire[1];
            let (seen, fire) = fairness_in_a_loop(true, seed);
            assert_eq!((seen, fire), ([0, 0], [0, 0]), "seed {seed}: smoke on");
        }
        println!("smoke off: seen through {off_seen}, fire through {off_fire}");
        assert!(off_seen > 0 && off_fire > 0, "off: seen {off_seen} fire {off_fire}");
    }

    /// The fairness count: a `Seen` sensing whose look, replayed, the cloud hid counts once (a refreshed track), not
    /// again while the track stands, and not when the line is clear; one no look made counts apart.
    #[test]
    fn a_seen_sensing_through_a_cloud_is_counted_once() {
        let (mut game, _) = made_up(&[0, 1]);
        let mut counts = SmokeCounts::new();
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 30.0, 10.0, 0);
        let track = |time| Track {
            who: 1,
            at: game.bodies[1].feet,
            error_m: 0.0,
            time,
            source: Source::Seen,
            sight_since: Some(1_000),
            notice_ms: 300,
            own: None,
            missed: None,
            held_until: 0,
        };
        let look = |time| smoke::tally::SeenLook { time, eye: Game::eye(&game.bodies[0]), feet: game.bodies[1].feet, crouched: false };
        counts.clouds = vec![cloud(20.0, 10.0)];
        smoke::tally::looked_seen(0, 1, look(5_000));
        counts.read_seen(0, &[track(5_000)]);
        counts.check_seen(&game, 0, 5_000);
        assert_eq!((counts.seen_through, counts.seen_unlooked), ([1, 0], [0, 0]));
        counts.read_seen(0, &[track(5_000)]);
        counts.check_seen(&game, 0, 5_100);
        assert_eq!(counts.seen_through, [1, 0], "the same sensing");
        counts.clouds = vec![cloud(20.0, 40.0)];
        smoke::tally::looked_seen(0, 1, look(5_200));
        counts.read_seen(0, &[track(5_200)]);
        counts.check_seen(&game, 0, 5_200);
        assert_eq!((counts.seen_through, counts.seen_unlooked), ([1, 0], [0, 0]), "a clear line");
        // A sighting no look made: counted apart, whatever the clouds.
        counts.read_seen(0, &[track(5_400)]);
        counts.check_seen(&game, 0, 5_400);
        assert_eq!((counts.seen_through, counts.seen_unlooked), ([1, 0], [1, 0]), "no look");
    }
}
