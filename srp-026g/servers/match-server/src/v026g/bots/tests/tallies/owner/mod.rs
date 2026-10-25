//! The bench's counts of what the owner saw in their local test of
//! 2026-10-25 (private `pvb-343-check/owner-test-analysis`):
//! - fighting: bots dry or reloading in a fight, fire at ranges their
//!   weapon is poor at, no turn to a shooter; and the misses' causes (fire
//!   and hits by the shooter's movement, hits by range and weapon class,
//!   fire at a target out of line, target switches while the one hurting
//!   the bot is in line);
//! - jumps: the wrong-jump detector of `jumps.md` §9 (form and outcome
//!   flags, missed jumps at a ledge);
//! - moving: the stuck detector of `navigation.md` §2d, idle, drops,
//!   U-turns, bodies close or in one another;
//! - grenades on their own side, and bots inside a blast that were outside
//!   at the throw (the stand-in has no grenade damage: counted only);
//! - campers (`campers.md`): kills in a run from one spot, a team's deaths
//!   in a row to one shooter from one spot, bots entering a held line.
//!
//! Each count copies the analysis's definition (its script named at the
//! count), so a fix can be shown on the bench before a live test. What
//! the bench cannot see as the game does is said at the count.
//!
//! Read from the stand-in game (bodies, keys, shots, hits, kills,
//! landings) and, where only the bot knows it (its ammunition, its
//! reloads, why it jumped, its play and step), from the bot's state;
//! never fed back. Most counts are worked out at the match's end from a
//! record of every body every tick ([`Rec`]); those that need rays or the
//! bots' state at the moment are counted as the match runs.

use std::collections::BTreeMap;

use mapdata::{Vec3, WalkGrid};
use survarium::v026g::match_msgs::TICK_MS;

use crate::v026g::bots::kin::{TICK_S, wrap, yaw_of};
use crate::v026g::bots::view::Place;
use super::{Bots, Game, Go, Landing, Note};
use crate::v026g::drive::key;

mod campers;
mod fight;
mod golden;
mod grenades;
mod jumps;
mod moving;

use campers::*;
use fight::*;
use grenades::*;
use jumps::*;
use moving::*;

const WALK_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;
/// Ticks a second.
const TPS: usize = (1000 / TICK_MS) as usize;

/// One body at one tick.
#[derive(Clone, Copy, Debug, Default)]
struct Rec {
    alive: bool,
    keys: u32,
    feet: Vec3,
    yaw: f32,
    /// m/s over the tick.
    speed: f32,
    calm: bool,
    carrying: bool,
    /// A jump under way.
    air: bool,
    /// The bot's play (an index into the names); in a designed hold; in a
    /// step with a goal to walk to (§2d).
    play: u16,
    hold: bool,
    travel: bool,
}

/// What the bench reads of a bot at a tick: out of rounds with nothing
/// to load, its last reload (start, end), the jump it pressed (why, and
/// the route's take-off and landing for a route jump), its play, its
/// play's step (once a second: none between), and how far its goal is.
#[derive(Clone, Debug, Default)]
struct BotNow {
    dry: bool,
    reload: Option<(u32, u32)>,
    jump: Option<(&'static str, Option<(Vec3, Vec3)>)>,
    play: String,
    step: Option<String>,
    goal_m: Option<f32>,
}

#[derive(Default)]
pub(crate) struct OwnerTally {
    first_tick: u32,
    recs: Vec<Vec<Rec>>,
    is_bot: Vec<bool>,
    plays: Vec<String>,
    play_index: BTreeMap<String, u16>,
    /// By place: in a designed hold, in a step with a goal (the last
    /// step read).
    status: Vec<(bool, bool)>,
    /// Bot state of the last tick: dry, the last reload's (at, until).
    dry: Vec<bool>,
    last_reload: Vec<Option<(u32, u32)>>,
    reloads: Vec<Reload>,
    reloads_n: u32,
    reloads_in_line: u32,
    reloads_in_line_died: u32,
    dry_ticks: u64,
    reload_ticks: u64,
    deaths_dry: u32,
    deaths_reloading: u32,
    /// Fire ticks by weapon class and band; hits by the same.
    fire: BTreeMap<&'static str, [u32; 5]>,
    hits_band: BTreeMap<&'static str, [u32; 5]>,
    /// Fire ticks and hits by the shooter's movement ([`MOVES`]).
    fire_move: [u32; 5],
    hits_move: [u32; 5],
    /// Fire ticks with an enemy target: of them, those with no line from
    /// the eye to his chest or head; with none.
    fire_on_target: u32,
    fire_hidden: u32,
    fire_no_target: u32,
    /// By place: the last tick's target, and the enemies who hit it (who,
    /// when).
    last_target: Vec<Option<Place>>,
    hurt_by: Vec<Vec<(Place, u32)>>,
    switches: u32,
    switches_off_hurter: u32,
    hits: Vec<(u32, Place, Place)>,
    presses: Vec<Press>,
    landings: Vec<Landing>,
    takes: Vec<Vec<u32>>,
    stuck_notes: Vec<Vec<u32>>,
    no_way: Vec<Vec<u32>>,
    blasts: Vec<Blast>,
    grenades: u32,
    /// Blast damage to the thrower, to his teammates, to enemies; the
    /// bodies it would have killed of each; the bodies inside the radius
    /// when it went off, and of them those outside at the throw.
    blast_dmg: [f32; 3],
    blast_kills: [u32; 3],
    blast_in: [u32; 3],
    walked_in: [u32; 3],
    /// Enemy kills; the kill log read so far (the events take it).
    kills: Vec<Kill>,
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    spots: Vec<KillSpot>,
    entries: Vec<Entry>,
    /// Trade takes: the trader, its team, when.
    trades: Vec<(Place, u8, u32)>,
    /// Flank or Crossfire takes on a shooter holding his kill spot: all,
    /// and Flank's.
    counter_runs: [u32; 2],
    /// Shooters killed while holding their kill spot: who, and whether the
    /// killer was [`OFF_LANE_DEG`] or more off every victim's bearing.
    held_kills: Vec<(Place, bool)>,
    /// The bench's stand-in camper (`SRP_BENCH_CAMPER`): its counts are
    /// also given apart.
    pub(crate) camper: Option<Place>,
    metric_rays: u64,
}

impl OwnerTally {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            recs: vec![Vec::new(); n],
            is_bot: vec![false; n],
            status: vec![(false, false); n],
            dry: vec![false; n],
            last_reload: vec![None; n],
            last_target: vec![None; n],
            hurt_by: vec![Vec::new(); n],
            takes: vec![Vec::new(); n],
            stuck_notes: vec![Vec::new(); n],
            no_way: vec![Vec::new(); n],
            ..Self::default()
        }
    }

    fn play_id(&mut self, name: String) -> u16 {
        if let Some(&id) = self.play_index.get(&name) {
            return id;
        }
        let id = self.plays.len() as u16;
        self.plays.push(name.clone());
        self.play_index.insert(name, id);
        id
    }

    /// After a game step, before the hit tally takes the game's hits: the
    /// bots' state as [`BotNow`], then [`Self::step`].
    pub(crate) fn tick(&mut self, game: &mut Game, bots: &Bots) {
        let tick = game.tick;
        // The step once a second, at the stuck detector's samples.
        let sample = self.first_tick == 0
            || ((tick.saturating_sub(self.first_tick) / TICK_MS) as usize).is_multiple_of(TPS);
        let nows: Vec<Option<BotNow>> = bots
            .bots
            .iter()
            .enumerate()
            .map(|(place, bot)| {
                let bot = bot.as_ref()?;
                let feet = game.bodies[place].feet;
                let goal_m = match bot.intent.go {
                    Go::To { at, .. } => Some(feet.distance_xz(at)),
                    Go::Stay => None,
                };
                let watch = bot
                    .jump_watch
                    .filter(|w| w.at.abs_diff(tick) <= TICK_MS * 2);
                Some(BotNow {
                    dry: bot.reflex.dry(),
                    reload: bot.reflex.reload_note().map(|r| (r.at, r.until)),
                    jump: watch.map(|w| (w.by, w.route.map(|r| (r.take_off, r.landing)))),
                    play: bot
                        .assignment
                        .as_ref()
                        .map_or("none".to_owned(), |a| format!("{:?}", a.kind)),
                    step: sample.then(|| {
                        bot.running
                            .as_ref()
                            .map_or_else(String::new, |r| r.step_name())
                    }),
                    goal_m,
                })
            })
            .collect();
        self.step(game, &nows);
    }

    /// One tick of the counts with the bots' state `nows` (none: not a
    /// bot): records the bodies; the bots' ammunition, reloads, jumps and
    /// targets; fire by range, movement and line; hits; kills and held
    /// lines; reloads in a line; the grenades going off. Takes the game's
    /// landings.
    fn step(&mut self, game: &mut Game, nows: &[Option<BotNow>]) {
        let tick = game.tick;
        let n = game.bodies.len();
        if self.first_tick == 0 {
            self.first_tick = tick;
        }
        let mut rays = 0u64;
        self.hits.extend(game.hit_log.iter().copied());
        for &(t, shooter, victim) in &game.hit_log {
            if let Some(hurt) = self.hurt_by.get_mut(victim) {
                hurt.retain(|&(who, _)| who != shooter);
                hurt.push((shooter, t));
            }
        }
        for hurt in &mut self.hurt_by {
            hurt.retain(|&(_, t)| tick <= t + HURT_MS);
        }
        self.landings.append(&mut game.landings);
        let sight = game.world.sight.clone();
        for place in 0..n {
            let body = &game.bodies[place];
            let now = nows.get(place).and_then(Option::as_ref);
            let prev = self.recs[place].last().copied().unwrap_or_default();
            if let Some(now) = now {
                self.is_bot[place] = true;
                if let Some(step) = &now.step {
                    self.status[place] = status_of(&now.play, step, now.goal_m);
                }
            }
            // A death: dry or reloading at it.
            if prev.alive && !body.alive {
                self.deaths_dry += u32::from(self.dry[place]);
                self.deaths_reloading += u32::from(self.last_reload[place].is_some_and(
                    |(at, until)| at <= tick && tick <= until + RELOAD_AFTER_MS,
                ));
            }
            let enemy_near = game.bodies.iter().any(|b| {
                b.alive && b.team != body.team && b.feet.distance_xz(body.feet) <= CALM_ENEMY_M
            });
            let play = self.play_id(now.map_or("human".to_owned(), |b| b.play.clone()));
            let (hold, travel) = self.status[place];
            let rec = Rec {
                alive: body.alive,
                keys: body.keys,
                feet: body.feet,
                yaw: body.yaw,
                speed: body.step_m / TICK_S,
                calm: body.keys & (key::AIM | key::FIRE) == 0 && !enemy_near,
                carrying: body.carrying.is_some(),
                air: body.flight.is_some() || body.air_until > tick,
                play,
                hold,
                travel,
            };
            self.recs[place].push(rec);
            let Some(now) = now else {
                continue;
            };
            // The bot's ammunition and reloads.
            self.dry[place] = body.alive && now.dry;
            if self.dry[place] {
                self.dry_ticks += 1;
            }
            if let Some((at, until)) = now.reload {
                if self.last_reload[place].is_none_or(|(was, _)| was != at) {
                    self.reloads_n += 1;
                    self.reloads.push(Reload {
                        place,
                        at,
                        until,
                        in_line: false,
                    });
                }
                self.last_reload[place] = Some((at, until));
                if body.alive && at <= tick && tick < until {
                    self.reload_ticks += 1;
                }
            }
            // A jump pressed: why, from the bot's watch of it.
            if body.alive && body.keys & key::JUMP != 0 && prev.keys & key::JUMP == 0 {
                self.presses.push(Press {
                    place,
                    t: tick,
                    feet: body.feet,
                    by: now.jump.map_or("other", |(by, _)| by),
                    route: now.jump.and_then(|(_, route)| route),
                    carrying: rec.carrying,
                });
            }
            let enemy = |t: Place| {
                game.bodies
                    .get(t)
                    .filter(|b| t != place && b.alive && b.team != body.team)
            };
            if body.alive && body.keys & key::FIRE != 0 {
                self.fire_move[move_of(&rec)] += 1;
                // At the target: with a line to him or not.
                match body.target.and_then(enemy) {
                    Some(him) => {
                        self.fire_on_target += 1;
                        if !in_sight(&sight, body, him, f32::INFINITY, &mut rays) {
                            self.fire_hidden += 1;
                        }
                    }
                    None => self.fire_no_target += 1,
                }
                // By range: the enemy nearest the view.
                let on = game
                    .bodies
                    .iter()
                    .filter(|b| b.alive && b.team != body.team)
                    .filter_map(|b| {
                        let d = b.feet.distance_xz(body.feet);
                        let off = off_deg(body.yaw, body.feet, b.feet);
                        ((FIRE_MIN_M..=FIRE_BANDS[4]).contains(&d) && off <= FIRE_ON_DEG)
                            .then_some((off, d))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, d)) = on {
                    self.fire.entry(body.gun.class()).or_insert([0; 5])[band_of(d)] += 1;
                }
            }
            // A target switch while an enemy who hurt it is in its line,
            // not the new target.
            let target = body.target.filter(|_| body.alive);
            if let (Some(was), Some(new)) = (self.last_target[place], target)
                && was != new
            {
                self.switches += 1;
                let hurter_in_line = self.hurt_by[place].iter().any(|&(who, _)| {
                    who != new
                        && enemy(who).is_some_and(|him| {
                            in_sight(&sight, him, body, f32::INFINITY, &mut rays)
                        })
                });
                self.switches_off_hurter += u32::from(hurter_in_line);
            }
            self.last_target[place] = target;
        }
        // Hits by the shooter's movement, the range and his weapon class.
        for &(_, shooter, victim) in &game.hit_log {
            let (Some(s), Some(v)) = (game.bodies.get(shooter), game.bodies.get(victim)) else {
                continue;
            };
            self.hits_move[self.recs[shooter].last().map_or(0, move_of)] += 1;
            let band = band_of(s.feet.distance_xz(v.feet));
            self.hits_band.entry(s.gun.class()).or_insert([0; 5])[band] += 1;
        }
        // Kills of enemies, and the lines they make held.
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new_kills = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        for (t, shooter, victim) in new_kills {
            let (Some(s), Some(v)) = (game.bodies.get(shooter), game.bodies.get(victim)) else {
                continue;
            };
            if s.team == v.team {
                continue;
            }
            let kill = Kill {
                t,
                shooter,
                victim,
                spot: s.feet,
                at: v.feet,
                team: v.team,
            };
            // A shooter killed holding his spot by one of his victims'
            // side: from off his killed lanes or not.
            if let Some(held) = self.spots.iter().find(|h| {
                h.shooter == victim
                    && h.team == s.team
                    && v.feet.distance_xz(h.spot) <= SPOT_HOLD_M
                    && t <= h.last + RUN_APART_MS
            }) {
                let bearing = yaw_of([s.feet.x - held.spot.x, s.feet.z - held.spot.z]);
                let off = held
                    .bearings
                    .iter()
                    .all(|&w| wrap(w - bearing).abs().to_degrees() >= OFF_LANE_DEG);
                self.held_kills.push((victim, off));
            }
            self.kills.push(kill);
            self.kill_spot(game, &kill, &sight, &mut rays);
        }
        if (((tick - self.first_tick) / TICK_MS) as usize).is_multiple_of(TPS) {
            self.held_lines(game, &sight, &mut rays);
        }
        // Reloads in an enemy's line: once a reload, every 4th tick.
        if (tick / TICK_MS).is_multiple_of(4) {
            for reload in &mut self.reloads {
                if reload.in_line || tick < reload.at || tick >= reload.until {
                    continue;
                }
                let me = &game.bodies[reload.place];
                if !me.alive {
                    continue;
                }
                reload.in_line = game.bodies.iter().any(|him| {
                    if !him.alive
                        || him.team == me.team
                        || him.feet.distance_xz(me.feet) > RELOAD_LINE_M
                    {
                        return false;
                    }
                    rays += 2;
                    sight.visible(Game::eye(me), Game::chest(him))
                        || sight.visible(Game::eye(him), Game::chest(me))
                });
            }
        }
        let mut done = Vec::new();
        self.reloads.retain(|r| {
            let over = tick > r.at.max(r.until) + RELOAD_DIED_MS;
            if over {
                done.push(*r);
            }
            !over
        });
        for r in done {
            self.reload_done(&r);
        }
        // The grenades going off: who is inside, who walked in, the damage
        // it would do (the stand-in has none).
        let mut gone = Vec::new();
        self.blasts.retain(|b| {
            let off = tick >= b.time;
            if off {
                gone.push(b.clone());
            }
            !off
        });
        for blast in gone {
            let Some(team) = game.bodies.get(blast.thrower).map(|b| b.team) else {
                continue;
            };
            for (place, body) in game.bodies.iter().enumerate() {
                let d = (Game::chest(body) - blast.at).length();
                if !body.alive || d > BLAST_M {
                    continue;
                }
                let who = if place == blast.thrower {
                    0
                } else if body.team == team {
                    1
                } else {
                    2
                };
                self.blast_in[who] += 1;
                self.walked_in[who] += u32::from(!blast.inside.get(place).copied().unwrap_or(false));
                rays += 1;
                if !sight.visible(blast.at + Vec3::new(0.0, 0.3, 0.0), Game::chest(body)) {
                    continue;
                }
                let dmg = blast_damage(d);
                self.blast_dmg[who] += dmg;
                self.blast_kills[who] += u32::from(dmg >= body.health);
            }
        }
        game.metric_rays += std::mem::take(&mut self.metric_rays) + rays;
    }

    /// The record index of `tick`.
    fn index(&self, tick: u32) -> usize {
        (tick.saturating_sub(self.first_tick) / TICK_MS) as usize
    }

    /// The tick of record index `k`, ms.
    fn time_of(&self, k: usize) -> u32 {
        self.first_tick + k as u32 * TICK_MS
    }

    /// The counts, as the `owner-test` lines and the sums' `owner` object.
    pub(crate) fn finish(&mut self, game: &Game) -> (String, serde_json::Value) {
        for r in std::mem::take(&mut self.reloads) {
            self.reload_done(&r);
        }
        let n = self.recs.len();
        let bot = |place: Place| place < n;
        let ticks = self.recs.first().map_or(0, Vec::len);
        let tick_s = TICK_S as f64;
        let bot_alive_ticks: u64 = (0..n)
            .filter(|&p| self.is_bot[p])
            .map(|p| self.recs[p].iter().filter(|r| r.alive).count() as u64)
            .sum();
        let bot_min = bot_alive_ticks as f64 * tick_s / 60.0;
        let per_min = |x: f64| if bot_min > 0.0 { (1000.0 * x / bot_min).round() / 1000.0 } else { 0.0 };
        let pct = |a: f64, b: f64| if b > 0.0 { (1000.0 * a / b).round() / 10.0 } else { 0.0 };
        let rate = |a: u32, b: u32| if b > 0 { (1000.0 * f64::from(a) / f64::from(b)).round() / 1000.0 } else { 0.0 };

        // Turned on the shooter.
        let (mut first_hits, mut hits_side, mut turned, mut died_unturned) = (0u32, 0u32, 0u32, 0u32);
        let mut last_hit: BTreeMap<Place, u32> = BTreeMap::new();
        for &(t, shooter, victim) in &self.hits {
            let apart = last_hit.get(&victim).is_none_or(|&l| t >= l + HIT_APART_MS);
            last_hit.insert(victim, t);
            if !apart || !bot(victim) {
                continue;
            }
            let i = self.index(t);
            let recs = &self.recs[victim];
            let Some(at) = recs.get(i) else {
                continue;
            };
            if !at.alive {
                continue;
            }
            first_hits += 1;
            let from = |k: usize| self.recs[shooter].get(k).map_or(Vec3::ZERO, |r| r.feet);
            if off_deg(at.yaw, at.feet, from(i)) <= HIT_SIDE_DEG {
                continue;
            }
            hits_side += 1;
            let end = (i + (TURN_MS / TICK_MS) as usize).min(recs.len() - 1);
            let mut outcome = None;
            for k in i..=end {
                if !recs[k].alive {
                    outcome = Some(false);
                    break;
                }
                if off_deg(recs[k].yaw, recs[k].feet, from(k)) <= TURNED_DEG {
                    outcome = Some(true);
                    break;
                }
            }
            match outcome {
                Some(true) => turned += 1,
                Some(false) => died_unturned += 1,
                None => {}
            }
        }

        // Jumps: the flags of every press, the stalls.
        let mut presses = self.presses.clone();
        presses.sort_by_key(|p| (p.place, p.t));
        let mut flag_n: BTreeMap<&str, u32> = FLAGS.iter().map(|&f| (f, 0)).collect();
        let (mut wrong, mut failed) = (0u32, 0u32);
        let mut by_kind: BTreeMap<&str, [u32; 2]> = BTreeMap::new();
        let mut route_landed = [0u32; 2];
        for (i, p) in presses.iter().enumerate() {
            let flags = self.press_flags(&presses, i);
            for &f in &flags {
                *flag_n.entry(f).or_default() += 1;
            }
            let is_wrong = flags.iter().any(|f| !SECOND_NUMBERS.contains(f));
            wrong += u32::from(is_wrong);
            failed += u32::from(flags.iter().any(|f| OUTCOMES.contains(f) && !SECOND_NUMBERS.contains(f)));
            let entry = by_kind.entry(p.by).or_default();
            entry[0] += 1;
            entry[1] += u32::from(is_wrong);
            // The honest stand-in's own landing of a route jump: up or not.
            if let Some((_, to)) = p.route
                && let Some(l) = self.landings.iter().find(|l| l.place == p.place && l.at == p.t)
            {
                route_landed[usize::from(l.to.y < to.y - UP_SLACK_M)] += 1;
            }
        }
        let sprint_jumps = self.landings.iter().filter(|l| l.sprint).count() as u32;
        let mut stalls = [0u32; 3];
        for place in (0..n).filter(|&p| self.is_bot[p]) {
            for (c, s) in stalls.iter_mut().zip(self.stalls(place, &game.world.grid, &presses)) {
                *c += s;
            }
        }

        // Calm walking and sprinting; idle; drops; U-turns.
        let (mut calm_walk, mut calm_sprint, mut calm_m) = (0u64, 0u64, 0f64);
        let mut idle: BTreeMap<String, f64> = BTreeMap::new();
        let (mut idle_s, mut drops) = (0u32, [0u32; 2]);
        let (mut uturns, mut takes_moving) = (0u32, 0u32);
        for (place, recs) in self.recs.iter().enumerate() {
            for r in recs.iter().filter(|r| r.alive && r.calm && !r.carrying) {
                if r.keys & WALK_KEYS != 0 {
                    calm_walk += 1;
                    calm_sprint += u64::from(r.keys & key::SPRINT != 0);
                    calm_m += f64::from(r.speed) * tick_s;
                }
            }
            // Seconds: idle ones, and the drops between them.
            let mut streak: Vec<u16> = Vec::new();
            let mut flush = |streak: &mut Vec<u16>, idle: &mut BTreeMap<String, f64>| {
                if streak.len() >= IDLE_S {
                    idle_s += streak.len() as u32;
                    for &p in streak.iter() {
                        *idle.entry(self.plays[usize::from(p)].clone()).or_default() += 1.0;
                    }
                }
                streak.clear();
            };
            for s in 0..recs.len() / TPS {
                let sec = &recs[s * TPS..(s + 1) * TPS];
                let whole = sec.iter().all(|r| r.alive);
                let moved: f32 = sec.iter().map(|r| r.speed * TICK_S).sum();
                let walk = sec.iter().filter(|r| r.keys & WALK_KEYS != 0).count() as f32 / TPS as f32;
                let is_idle = whole && moved < IDLE_M && walk < IDLE_WALK && sec.iter().all(|r| r.calm);
                if is_idle {
                    streak.push(sec[TPS - 1].play);
                } else {
                    flush(&mut streak, &mut idle);
                }
                if whole && s > 0 && recs[s * TPS - 1].alive {
                    let down = recs[s * TPS - 1].feet.y - sec[TPS - 1].feet.y;
                    for (c, m) in drops.iter_mut().zip(DROP_M) {
                        *c += u32::from(down >= m);
                    }
                }
            }
            flush(&mut streak, &mut idle);
            for &t in &self.takes[place] {
                let i = self.index(t);
                if i < UTURN_TICKS || i + UTURN_TICKS >= recs.len() {
                    continue;
                }
                let (a, b, c) = (&recs[i - UTURN_TICKS], &recs[i], &recs[i + UTURN_TICKS]);
                if !(a.alive && b.alive && c.alive) || recs[i - UTURN_TICKS..=i + UTURN_TICKS].iter().any(|r| !r.alive) {
                    continue;
                }
                let (u, v) = (b.feet - a.feet, c.feet - b.feet);
                let (lu, lv) = (u.x.hypot(u.z), v.x.hypot(v.z));
                if lu < UTURN_M || lv < UTURN_M {
                    continue;
                }
                takes_moving += 1;
                let cos = ((u.x * v.x + u.z * v.z) / (lu * lv)).clamp(-1.0, 1.0);
                uturns += u32::from(cos.acos().to_degrees() > UTURN_DEG);
            }
        }

        // Close pairs (teammates, from the seconds) and one body (any two,
        // every tick).
        let (mut close_eps, mut close_s, mut one_eps, mut one_ticks) = (0u32, 0u32, 0u32, 0u32);
        for a in 0..n {
            for b in a + 1..n {
                let (ra, rb) = (&self.recs[a], &self.recs[b]);
                let len = ra.len().min(rb.len());
                let same = game.bodies[a].team == game.bodies[b].team;
                if same {
                    let mut ep: Option<(u32, u32)> = None;
                    let mut end = |ep: Option<(u32, u32)>| {
                        if let Some((s0, s1)) = ep
                            && s1 - s0 + 1 >= CLOSE_S
                        {
                            close_eps += 1;
                            close_s += s1 - s0 + 1;
                        }
                    };
                    for s in 0..(len / TPS) as u32 {
                        let k = s as usize * TPS + TPS - 1;
                        let near = ra[k].alive && rb[k].alive && ra[k].feet.distance_xz(rb[k].feet) <= CLOSE_M;
                        if !near {
                            continue;
                        }
                        ep = match ep {
                            Some((s0, s1)) if s - s1 <= CLOSE_GAP_S + 1 => Some((s0, s)),
                            other => {
                                end(other);
                                Some((s, s))
                            }
                        };
                    }
                    end(ep);
                }
                let mut last: Option<usize> = None;
                for k in 0..len {
                    let (x, y) = (&ra[k], &rb[k]);
                    if x.alive
                        && y.alive
                        && x.feet.distance_xz(y.feet) <= ONE_BODY_M
                        && (x.feet.y - y.feet.y).abs() <= ONE_BODY_M
                    {
                        one_ticks += 1;
                        if last.is_none_or(|l| k > l + TPS / 2) {
                            one_eps += 1;
                        }
                        last = Some(k);
                    }
                }
            }
        }

        // Stuck (bots only).
        let teams: Vec<u8> = game.bodies.iter().map(|b| b.team).collect();
        let mut stuck: BTreeMap<&str, (u32, f32)> = BTreeMap::new();
        let mut stuck_cause: BTreeMap<&str, (u32, f32)> = BTreeMap::new();
        let mut spots: BTreeMap<(i32, i32, i32), u32> = BTreeMap::new();
        let mut blips = 0u32;
        for place in (0..n).filter(|&p| self.is_bot[p]) {
            let (eps, b) = self.stuck_episodes(place, &teams, &presses);
            blips += b;
            for e in eps {
                for (map, name) in [(&mut stuck, e.kinds[0]), (&mut stuck_cause, e.cause)] {
                    let c = map.entry(name).or_default();
                    c.0 += 1;
                    c.1 += e.t1 - e.t0;
                }
                let cell = (
                    (e.at.x / SPOT_M[0]).floor() as i32,
                    (e.at.z / SPOT_M[0]).floor() as i32,
                    (e.at.y / SPOT_M[1]).floor() as i32,
                );
                *spots.entry(cell).or_default() += 1;
            }
        }
        let stuck_n: u32 = stuck.values().map(|e| e.0).sum();
        let stuck_s: f32 = stuck.values().map(|e| e.1).sum();
        let stuck_spots = spots.values().filter(|&&c| c >= 2).count();
        let round = |m: BTreeMap<&'static str, (u32, f32)>| -> BTreeMap<&'static str, (u32, f32)> {
            m.into_iter().map(|(k, (c, s))| (k, (c, s.round()))).collect()
        };

        // Campers: runs of kills from one spot, a team's deaths in a row to
        // one shooter from one spot, entries into held lines.
        // `only`: the runs of that shooter alone are counted.
        let in_runs = |groups: &BTreeMap<Place, Vec<Kill>>, only: Option<Place>| {
            let (mut runs, mut kills, mut longest) = (0u32, 0u32, 0u32);
            for list in groups.values() {
                let mut run: Option<(Place, Vec3, u32, u32)> = None;
                let mut close = |run: Option<(Place, Vec3, u32, u32)>| {
                    if let Some((shooter, .., len)) = run
                        && only.is_none_or(|o| o == shooter)
                    {
                        longest = longest.max(len);
                        if len >= CAMP_RUN {
                            runs += 1;
                            kills += len;
                        }
                    }
                };
                for k in list {
                    run = match run {
                        Some((shooter, spot, last, len))
                            if shooter == k.shooter
                                && k.spot.distance_xz(spot) <= SPOT_HOLD_M
                                && k.t <= last + RUN_APART_MS =>
                        {
                            Some((shooter, spot, k.t, len + 1))
                        }
                        other => {
                            close(other);
                            Some((k.shooter, k.spot, k.t, 1))
                        }
                    };
                }
                close(run);
            }
            (runs, kills, longest)
        };
        let mut by_shooter: BTreeMap<Place, Vec<Kill>> = BTreeMap::new();
        let mut by_team: BTreeMap<Place, Vec<Kill>> = BTreeMap::new();
        for k in &self.kills {
            by_shooter.entry(k.shooter).or_default().push(*k);
            by_team.entry(usize::from(k.team)).or_default().push(*k);
        }
        let (camp_runs, camp_run_kills, camp_longest) = in_runs(&by_shooter, None);
        let (in_row, _, in_row_longest) = in_runs(&by_team, None);
        let fatal = |e: &Entry| {
            self.kills
                .iter()
                .any(|k| k.shooter == e.shooter && k.victim == e.place && k.t >= e.t && k.t <= e.t + FATAL_MS)
        };
        let entries = self.entries.len() as u32;
        let entries_fatal = self.entries.iter().filter(|e| fatal(e)).count() as u32;
        let entries_lane = self.entries.iter().filter(|e| e.lane).count() as u32;
        let entries_lane_fatal = self.entries.iter().filter(|e| e.lane && fatal(e)).count() as u32;
        // Trades into the killer: of each Trade take, the partner's killer
        // (the last kill of a teammate from 4 s before to 0.5 s after);
        // the trader killed by him, or killing him, within 15 s.
        let mut trades = [self.trades.len() as u32, 0, 0];
        for &(place, team, t) in &self.trades {
            let killer = self
                .kills
                .iter()
                .rfind(|k| {
                    k.team == team && k.victim != place && k.t + TRADE_BEFORE_MS >= t && k.t <= t + TRADE_AFTER_MS
                })
                .map(|k| k.shooter);
            let Some(killer) = killer else {
                continue;
            };
            let within = |k: &Kill| k.t >= t && k.t <= t + FATAL_MS;
            trades[1] += u32::from(self.kills.iter().any(|k| within(k) && k.victim == place && k.shooter == killer));
            trades[2] += u32::from(self.kills.iter().any(|k| within(k) && k.shooter == place && k.victim == killer));
        }
        let held_killed = |who: Option<Place>| {
            let mine = self.held_kills.iter().filter(|(p, _)| who.is_none_or(|w| w == *p));
            [mine.clone().count() as u32, mine.filter(|(_, off)| *off).count() as u32]
        };
        let held_all = held_killed(None);
        // The stand-in camper's part of the counts (the rest is the bots').
        let camper = self.camper.map(|c| {
            let (runs, run_kills, longest) = in_runs(&by_shooter, Some(c));
            let (row, _, row_longest) = in_runs(&by_team, Some(c));
            let into: Vec<&Entry> = self.entries.iter().filter(|e| e.shooter == c).collect();
            let killed = held_killed(Some(c));
            serde_json::json!({
                "place": c,
                "kills": self.kills.iter().filter(|k| k.shooter == c).count(),
                "deaths": self.kills.iter().filter(|k| k.victim == c).count(),
                "runs_3": runs,
                "run_kills": run_kills,
                "run_longest": longest,
                "deaths_in_row_3": row,
                "deaths_in_row_longest": row_longest,
                "reentries": into.len(),
                "reentries_fatal": into.iter().filter(|e| fatal(e)).count(),
                "reentries_lane": into.iter().filter(|e| e.lane).count(),
                "killed_holding": killed[0],
                "killed_off_lane": killed[1],
            })
        });

        let fire_far: BTreeMap<&str, [u32; 2]> = self
            .fire
            .iter()
            .map(|(class, bands)| (*class, [bands.iter().sum(), bands[3] + bands[4]]))
            .collect();
        let fire_near_class: [u32; 2] = self
            .fire
            .iter()
            .filter(|(class, _)| matches!(**class, "shotgun" | "smg"))
            .fold([0, 0], |acc, (_, b)| [acc[0] + b.iter().sum::<u32>(), acc[1] + b[3] + b[4]]);
        let by_move = |v: &[u32; 5]| -> BTreeMap<&str, u32> { MOVES.iter().copied().zip(v.iter().copied()).collect() };
        let hit_rate_move: BTreeMap<&str, f64> = MOVES
            .iter()
            .enumerate()
            .map(|(i, m)| (*m, rate(self.hits_move[i], self.fire_move[i])))
            .collect();
        let hit_rate_class: BTreeMap<&str, Vec<f64>> = self
            .fire
            .iter()
            .map(|(class, fire)| {
                let hits = self.hits_band.get(class).copied().unwrap_or_default();
                (*class, (0..5).map(|b| rate(hits[b], fire[b])).collect())
            })
            .collect();
        // Three objects: one json! of all of them goes past the macro's
        // recursion limit.
        let mut json = serde_json::json!({
            "dry_s": (self.dry_ticks as f64 * tick_s).round(),
            "reload_s": (self.reload_ticks as f64 * tick_s).round(),
            "deaths_dry": self.deaths_dry,
            "deaths_reloading": self.deaths_reloading,
            "reloads": self.reloads_n,
            "reloads_in_line": self.reloads_in_line,
            "reloads_in_line_died": self.reloads_in_line_died,
            "fire_by_class": self.fire,
            "fire_far_by_class": fire_far,
            "fire_far_close_class_pct": pct(fire_near_class[1].into(), fire_near_class[0].into()),
            "hits_by_class": self.hits_band,
            "hit_rate_by_class": hit_rate_class,
            "fire_by_move": by_move(&self.fire_move),
            "hits_by_move": by_move(&self.hits_move),
            "hit_rate_by_move": hit_rate_move,
            "fire_on_target": self.fire_on_target,
            "fire_hidden": self.fire_hidden,
            "fire_hidden_pct": pct(self.fire_hidden.into(), self.fire_on_target.into()),
            "fire_no_target": self.fire_no_target,
            "target_switches": self.switches,
            "switches_off_hurter": self.switches_off_hurter,
            "hits_first": first_hits,
            "hits_side": hits_side,
            "turned_3s": turned,
            "turned_3s_pct": pct(turned.into(), hits_side.into()),
            "died_unturned": died_unturned,
        });
        let jumps = serde_json::json!({
            "presses": presses.len(),
            "presses_by": by_kind,
            "jump_flags": flag_n,
            "jumps_wrong": wrong,
            "jumps_failed": failed,
            "missed_jump": stalls[0],
            "stall_at_step": stalls[1],
            "stalls": stalls[2],
            "route_landed_up": route_landed[0],
            "route_landed_down": route_landed[1],
            "sprint_jumps": sprint_jumps,
            "stuck": stuck_n,
            "stuck_s": stuck_s.round(),
            "stuck_by": round(stuck),
            "stuck_by_cause": round(stuck_cause),
            "stuck_per_bot_min": per_min(stuck_n.into()),
            "stuck_s_per_bot_min": per_min(stuck_s.into()),
            "stuck_blips": blips,
            "stuck_blips_per_bot_min": per_min(blips.into()),
            "stuck_spots_2": stuck_spots,
            "bot_min": bot_min.round(),
        });
        let more = serde_json::json!({
            "calm_walk_s": (calm_walk as f64 * tick_s).round(),
            "calm_sprint_pct": pct(calm_sprint as f64, calm_walk as f64),
            "calm_mps": if calm_walk > 0 { (100.0 * calm_m / (calm_walk as f64 * tick_s)).round() / 100.0 } else { 0.0 },
            "idle_s": idle_s,
            "idle_by_play": idle,
            "drops_2_5m": drops[0] - drops[1],
            "drops_4m": drops[1],
            "uturns": uturns,
            "takes_moving": takes_moving,
            "uturns_per_bot_min": per_min(uturns.into()),
            "close_pairs": close_eps,
            "close_pair_s": close_s,
            "one_body": one_eps,
            "one_body_s": (f64::from(one_ticks) * tick_s).round(),
            "grenades": self.grenades,
            "grenade_damage_applied": false,
            "grenade_self_dmg": (100.0 * self.blast_dmg[0]).round() / 100.0,
            "grenade_mate_dmg": (100.0 * self.blast_dmg[1]).round() / 100.0,
            "grenade_enemy_dmg": (100.0 * self.blast_dmg[2]).round() / 100.0,
            "grenade_kills": self.blast_kills,
            "grenade_in_radius": self.blast_in,
            "grenade_walk_ins": self.walked_in,
            "team_kills_gun": game.team_kills,
            "team_hits": game.team_hits,
            "camper_runs_3": camp_runs,
            "camper_run_kills": camp_run_kills,
            "camper_run_longest": camp_longest,
            "deaths_in_row_3": in_row,
            "deaths_in_row_longest": in_row_longest,
            "reentries": entries,
            "reentries_fatal": entries_fatal,
            "reentries_lane": entries_lane,
            "reentries_lane_fatal": entries_lane_fatal,
            "ticks": ticks,
        });
        let answers = serde_json::json!({
            "trades": trades[0],
            "trades_died_to_killer": trades[1],
            "trade_kills": trades[2],
            "counter_runs": self.counter_runs[0],
            "counter_runs_flank": self.counter_runs[1],
            "camper_kills": held_all[0],
            "camper_kills_off_lane": held_all[1],
            "stand_in_camper": camper,
        });
        if let Some(all) = json.as_object_mut() {
            for part in [jumps, more, answers] {
                if let serde_json::Value::Object(part) = part {
                    all.extend(part);
                }
            }
        }
        let flags_set: Vec<String> = flag_n
            .iter()
            .filter(|(_, c)| **c > 0)
            .map(|(f, c)| format!("{f} {c}"))
            .collect();
        let line = format!(
            "owner-test: dry {} s (deaths {}), reloading {} s (deaths {}), reloads {} in a line {} died {}; fire >=35 m by class {}; side hits {} turned in 3 s {} died unturned {}\n  \
             owner-test fight: fire/hits by move {}; hits a fire tick by class and band {}; fire at a target {} out of line {} ({:.1} %), no target {}; target switches {} with the hurter in line {}\n  \
             owner-test jumps: presses {} by [presses, wrong] {}; wrong {} failed {}; flags {}; missed jumps {} stalls at a step {} (stalls {}); route landings up {} down {}; sprint jumps {}\n  \
             owner-test moves: stuck {} ({:.0} s, {:.3} a bot-minute) by kind {} by cause {}, blips {}, spots of 2+ {}; calm sprint {:.1} % of {:.0} s walking, {:.2} m/s; idle {} s {}; drops 2.5-4 m {} >=4 m {}; U-turns {} of {} takes moving ({:.3} a bot-minute); close pairs {} ({} s); one body {} ({:.0} s)\n  \
             owner-test other: grenades {} (no damage in the stand-in) would damage self {:.2} mates {:.2} enemies {:.2}, would kill {:?}, inside at the blast {:?} walked in {:?}; team kills by gunfire {}, team hits {}\n  \
             owner-test campers: runs of 3+ from one spot {} ({} kills, longest {}); a team's deaths in a row to one shooter's spot, 3+ {} (longest {}); entries into a held line {} fatal {}, in the killed lane {} fatal {}; Trade takes {} died to the partner's killer {} killed him {}; counter runs on a holder {} (Flank {}); holders killed {} from off their lanes {}; the stand-in camper {}",
            json["dry_s"], self.deaths_dry, json["reload_s"], self.deaths_reloading,
            self.reloads_n, self.reloads_in_line, self.reloads_in_line_died,
            fire_far.iter().map(|(c, v)| format!("{c} {}/{}", v[1], v[0])).collect::<Vec<_>>().join(" "),
            hits_side, turned, died_unturned,
            MOVES.iter().enumerate().map(|(i, m)| format!("{m} {}/{}", self.fire_move[i], self.hits_move[i])).collect::<Vec<_>>().join(" "),
            json["hit_rate_by_class"], self.fire_on_target, self.fire_hidden,
            json["fire_hidden_pct"].as_f64().unwrap_or(0.0), self.fire_no_target,
            self.switches, self.switches_off_hurter,
            presses.len(), json["presses_by"], wrong, failed, flags_set.join(", "),
            stalls[0], stalls[1], stalls[2], route_landed[0], route_landed[1], sprint_jumps,
            stuck_n, stuck_s, json["stuck_per_bot_min"].as_f64().unwrap_or(0.0),
            json["stuck_by"], json["stuck_by_cause"], blips, stuck_spots,
            json["calm_sprint_pct"].as_f64().unwrap_or(0.0), calm_walk as f64 * tick_s, json["calm_mps"].as_f64().unwrap_or(0.0),
            idle_s, json["idle_by_play"], drops[0] - drops[1], drops[1],
            uturns, takes_moving, json["uturns_per_bot_min"].as_f64().unwrap_or(0.0),
            close_eps, close_s, one_eps, f64::from(one_ticks) * tick_s,
            self.grenades, self.blast_dmg[0], self.blast_dmg[1], self.blast_dmg[2], self.blast_kills,
            self.blast_in, self.walked_in,
            game.team_kills, game.team_hits,
            camp_runs, camp_run_kills, camp_longest, in_row, in_row_longest,
            entries, entries_fatal, entries_lane, entries_lane_fatal,
            trades[0], trades[1], trades[2], self.counter_runs[0], self.counter_runs[1],
            held_all[0], held_all[1], json["stand_in_camper"],
        );
        (line, json)
    }

}

/// A game of `teams` on the soak field and a tally of it, for the counts'
/// made-up cases.
fn made_up(teams: &[u8]) -> (Game, OwnerTally) {
    let game = Game::new(super::soak_world(), teams, 1, None, None);
    let tally = OwnerTally::new(teams.len());
    (game, tally)
}

/// `ticks` ticks of `game`: before each, `set(k, game)` places the bodies
/// and gives the bots' state; the game's hits are taken after each.
fn play(
    game: &mut Game,
    tally: &mut OwnerTally,
    ticks: usize,
    mut set: impl FnMut(usize, &mut Game) -> Vec<Option<BotNow>>,
) {
    for k in 0..ticks {
        game.tick = (k as u32 + 1) * TICK_MS;
        let nows = set(k, game);
        tally.step(game, &nows);
        game.hit_log.clear();
    }
}

fn bot_now() -> Option<BotNow> {
    Some(BotNow {
        play: "Roam".to_owned(),
        ..BotNow::default()
    })
}

fn put(game: &mut Game, place: Place, x: f32, z: f32, keys: u32) {
    let body = &mut game.bodies[place];
    body.alive = true;
    body.feet = Vec3::new(x, 0.0, z);
    body.keys = keys;
    body.step_m = 0.0;
}
