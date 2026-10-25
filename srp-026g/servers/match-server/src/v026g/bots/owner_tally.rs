//! The bench's counts of what the owner saw in their local test of
//! 2026-10-25 (private `pvb-343-check/owner-test-analysis`): bots dry or
//! reloading in a fight, fire at ranges their weapon is poor at, no turn to
//! a shooter, jumps pressed from a standstill, with forward let go, up
//! more than a jump makes, again and again at one place; stuck, idle,
//! drops, U-turns, bodies close or in one another; grenades on their own
//! side. Each count copies the analysis's definition (its script named at
//! the count), so a fix can be shown on the bench before a live test.
//!
//! Read from the stand-in game (bodies, keys, shots, hits, landings) and,
//! where only the bot knows it (its ammunition, its reloads, why it
//! jumped, its play and goal), from the bot's state; never fed back.
//! Most counts are worked out at the match's end from a record of every
//! body every tick ([`Rec`]); those that need rays or the bots' state at
//! the moment are counted as the match runs.

use std::collections::BTreeMap;

use mapdata::Vec3;
use survarium::v026g::match_msgs::TICK_MS;

use super::super::kin::{TICK_S, wrap, yaw_of};
use super::super::view::Place;
use super::{Bots, Game, Go, Landing, Note};
use crate::v026g::drive::key;

const WALK_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;
/// Ticks a second.
const TPS: usize = (1000 / TICK_MS) as usize;

/// Calm (`gait.py`): no aim or fire key held and no living enemy within
/// this, m.
const CALM_ENEMY_M: f32 = 30.0;
/// Reloads in an enemy's line (`behaviour.py` D): within this, m; and a
/// death this soon after the reload's start counts, ms.
const RELOAD_LINE_M: f32 = 40.0;
const RELOAD_DIED_MS: u32 = 4_000;
/// A death this soon after a reload's end counts as while reloading, ms.
const RELOAD_AFTER_MS: u32 = 1_000;
/// Fire by range (`fire_stats.py`): the enemy nearest the view within this
/// many degrees of yaw, at 2-80 m; the bands' upper bounds, m.
const FIRE_ON_DEG: f32 = 10.0;
const FIRE_BANDS: [f32; 5] = [10.0, 20.0, 35.0, 50.0, 80.0];
const FIRE_MIN_M: f32 = 2.0;
/// Turned on the shooter (`turn_hit.py`): hits this far apart start a
/// case, ms; from more than this off the view, degrees; turned when the
/// view is within this of him, degrees, within the time, ms.
const HIT_APART_MS: u32 = 3_000;
const HIT_SIDE_DEG: f32 = 60.0;
const TURNED_DEG: f32 = 15.0;
const TURN_MS: u32 = 3_000;
/// Jumps (`runup.py`, `classify.py`, `jumps.py`): a stand-turn press has a
/// forward run of at most `STAND_RUN` ticks after at least `STAND_STILL`
/// ticks with no walk key; forward let go when held at most `LET_GO`
/// ticks from the press; a jump needs over 1 m (and 1.2 m) of rise; a
/// series is presses by one bot each within 3 s and 2 m of the last.
const STAND_RUN: usize = 6;
const STAND_STILL: usize = 4;
const LET_GO: usize = 5;
const NEED_M: [f32; 2] = [1.0, 1.2];
const SERIES_MS: u32 = 3_000;
const SERIES_M: f32 = 2.0;
/// A route jump made: its landing within this under the route's, m.
const UP_SLACK_M: f32 = 0.3;
/// Idle (`idle.py`): seconds under 0.3 m moved, walk keys under 30 % of
/// the ticks, calm; at least 5 of them in a row.
const IDLE_M: f32 = 0.3;
const IDLE_WALK: f32 = 0.3;
const IDLE_S: usize = 5;
/// Drops (`drops.py`): the feet down this far between two worlds, m.
const DROP_M: [f32; 2] = [2.5, 4.0];
/// U-turns (`uturns.py`): the moves 2.2 s before and after a take, each
/// at least 2 m, more than 120 degrees apart.
const UTURN_TICKS: usize = 44;
const UTURN_M: f32 = 2.0;
const UTURN_DEG: f32 = 120.0;
/// Close pairs (`teamwork.py`): teammates within 3 m for 5 s or more,
/// gaps under 2.5 s; one body (`overlap.py`): two bodies within 0.5 m
/// across and in height.
const CLOSE_M: f32 = 3.0;
const CLOSE_S: u32 = 5;
const CLOSE_GAP_S: u32 = 2;
const ONE_BODY_M: f32 = 0.5;
/// Grenades: the RGD-5's blast (`rgd-5.options`: `splash_radius` 10 m,
/// `splash_max_damage` 1.3 to `splash_min_damage` 0.05; linear between:
/// GUESS), on a body with a line from the landing.
const BLAST_M: f32 = 10.0;
const BLAST_MAX: f32 = 1.3;
const BLAST_MIN: f32 = 0.05;

/// The stuck detector (`navigation/stuck.py`, the analysis's bench
/// definition): kinds, the least seconds, the radius about the
/// positions' middle (m), the least share of ticks with a walk key, the
/// share of aim or fire under which it counts (none: any), and whether a
/// hold (no goal to walk to) leaves it out.
const STUCK_KINDS: [(&str, f32, f32, f32, Option<f32>, bool); 3] = [
    ("push", 3.0, 0.6, 0.75, None, false),
    ("nohead", 10.0, 2.5, 0.70, Some(0.35), true),
    ("loop", 20.0, 8.0, 0.80, Some(0.35), true),
];
const PUSH_STILL_SPEED: f32 = 0.5;
const PUSH_OPPOSING_MAX: f32 = 0.25;
const PUSH_SPEED: f32 = 1.2;
const STAND_S: f32 = 10.0;
const STAND_R: f32 = 0.6;
const STAND_WALK_MAX: f32 = 0.25;
const LOOP_NET_M: f32 = 4.0;
const LOOP_PATH_M: f32 = 12.0;
const MERGE_GAP_S: f32 = 2.0;
const SPAWN_GRACE_S: f32 = 3.0;

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
    /// The bot's play (an index into the names), whether it has a goal
    /// to walk to (not there yet), and none at all (a hold).
    play: u16,
    travel: bool,
    hold: bool,
}

/// What the bench reads of a bot at a tick: out of rounds with nothing
/// to load, its last reload (start, end), the jump it pressed (why, and
/// the route's landing for a route jump), its play, whether it has a goal
/// to walk to (not there yet), and none at all (a hold).
#[derive(Clone, Debug, Default)]
struct BotNow {
    dry: bool,
    reload: Option<(u32, u32)>,
    jump: Option<(&'static str, Option<Vec3>)>,
    play: String,
    travel: bool,
    hold: bool,
}

/// A jump press.
#[derive(Clone, Copy, Debug)]
struct Press {
    place: Place,
    t: u32,
    feet: Vec3,
    by: &'static str,
    /// The route's landing, for a route jump.
    landing: Option<Vec3>,
}

/// A reload under way: its place, start and end, an enemy's line seen
/// during it, the bot died within [`RELOAD_DIED_MS`].
#[derive(Clone, Copy, Debug)]
struct Reload {
    place: Place,
    at: u32,
    until: u32,
    in_line: bool,
}

/// A grenade let go: the thrower, where it lands, when it goes off.
#[derive(Clone, Copy, Debug)]
struct Blast {
    thrower: Place,
    at: Vec3,
    time: u32,
}

#[derive(Default)]
pub(super) struct OwnerTally {
    first_tick: u32,
    recs: Vec<Vec<Rec>>,
    plays: Vec<String>,
    play_index: BTreeMap<String, u16>,
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
    /// Fire ticks by weapon class and band.
    fire: BTreeMap<&'static str, [u32; 5]>,
    hits: Vec<(u32, Place, Place)>,
    presses: Vec<Press>,
    landings: Vec<Landing>,
    takes: Vec<Vec<u32>>,
    stuck_notes: Vec<Vec<u32>>,
    blasts: Vec<Blast>,
    grenades: u32,
    /// Blast damage to the thrower, to his teammates, to enemies; and the
    /// bodies it would have killed of each.
    blast_dmg: [f32; 3],
    blast_kills: [u32; 3],
    metric_rays: u64,
}

/// "throws a grenade at (x, y, z), blast at <ms>": the place and the time.
fn thrown_at(text: &str) -> Option<(Vec3, u32)> {
    let rest = text.strip_prefix("throws a grenade at (")?;
    let (xyz, rest) = rest.split_once("), blast at ")?;
    let mut v = xyz.split(", ").map(|n| n.parse::<f32>().ok());
    let at = Vec3::new(v.next()??, v.next()??, v.next()??);
    Some((at, rest.trim().parse().ok()?))
}

/// How far a view at `yaw` from `from` is off the way to `to`, degrees.
fn off_deg(yaw: f32, from: Vec3, to: Vec3) -> f32 {
    wrap(yaw - yaw_of([to.x - from.x, to.z - from.z]))
        .abs()
        .to_degrees()
}

/// Where a grenade thrown from `eye` at `aim` comes down: the first point
/// the bullet mesh stops its arc at, else `aim`. The arc as two lines,
/// up to a top over the middle a quarter of the way's length higher (a
/// lob; GUESS: the throw's speed and the bounces are not modelled).
fn grenade_lands(sight: &mapdata::RayTree, eye: Vec3, aim: Vec3) -> Vec3 {
    let d = eye.distance_xz(aim);
    let top = (eye + aim) * 0.5 + Vec3::new(0.0, d * 0.25, 0.0);
    for (a, b) in [(eye, top), (top, aim)] {
        let way = b - a;
        let len = way.length();
        if len < 1e-3 {
            continue;
        }
        let dir = way * (1.0 / len);
        if let Some(t) = sight.ray_cast(a, dir, len) {
            return a + dir * (t - 0.05).max(0.0);
        }
    }
    aim
}

/// The blast's damage at `d` m (linear, [`BLAST_MAX`] to [`BLAST_MIN`]).
fn blast_damage(d: f32) -> f32 {
    if d > BLAST_M {
        0.0
    } else {
        BLAST_MAX - (BLAST_MAX - BLAST_MIN) * d / BLAST_M
    }
}

impl OwnerTally {
    pub(super) fn new(n: usize) -> Self {
        Self {
            recs: vec![Vec::new(); n],
            dry: vec![false; n],
            last_reload: vec![None; n],
            takes: vec![Vec::new(); n],
            stuck_notes: vec![Vec::new(); n],
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
    pub(super) fn tick(&mut self, game: &mut Game, bots: &Bots) {
        let tick = game.tick;
        let nows: Vec<Option<BotNow>> = bots
            .bots
            .iter()
            .enumerate()
            .map(|(place, bot)| {
                let bot = bot.as_ref()?;
                let feet = game.bodies[place].feet;
                let (travel, hold) = match bot.intent.go {
                    Go::To { at, within_m } => (feet.distance_xz(at) > within_m, false),
                    _ => (false, true),
                };
                let watch = bot
                    .jump_watch
                    .filter(|w| w.at.abs_diff(tick) <= TICK_MS * 2);
                Some(BotNow {
                    dry: bot.reflex.dry(),
                    reload: bot.reflex.reload_note().map(|r| (r.at, r.until)),
                    jump: watch.map(|w| (w.by, w.route.map(|r| r.landing))),
                    play: bot
                        .assignment
                        .as_ref()
                        .map_or("none".to_owned(), |a| format!("{:?}", a.kind)),
                    travel,
                    hold,
                })
            })
            .collect();
        self.step(game, &nows);
    }

    /// One tick of the counts with the bots' state `nows` (none: not a
    /// bot): records the bodies; the bots' ammunition, reloads and jumps;
    /// fire by range; reloads in a line; the grenades going off. Takes the
    /// game's landings.
    fn step(&mut self, game: &mut Game, nows: &[Option<BotNow>]) {
        let tick = game.tick;
        let n = game.bodies.len();
        if self.first_tick == 0 {
            self.first_tick = tick;
        }
        self.hits.extend(game.hit_log.iter().copied());
        self.landings.append(&mut game.landings);
        let sight = game.world.sight.clone();
        for place in 0..n {
            let body = &game.bodies[place];
            let now = nows.get(place).and_then(Option::as_ref);
            let prev = self.recs[place].last().copied().unwrap_or_default();
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
            let rec = Rec {
                alive: body.alive,
                keys: body.keys,
                feet: body.feet,
                yaw: body.yaw,
                speed: body.step_m / TICK_S,
                calm: body.keys & (key::AIM | key::FIRE) == 0 && !enemy_near,
                carrying: body.carrying.is_some(),
                play,
                travel: now.is_some_and(|b| b.travel),
                hold: now.is_none_or(|b| b.hold),
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
                    landing: now.jump.and_then(|(_, landing)| landing),
                });
            }
            // Fire by range: the enemy nearest the view.
            if body.alive && body.keys & key::FIRE != 0 {
                let on = game
                    .bodies
                    .iter()
                    .filter(|b| b.alive && b.team != body.team)
                    .filter_map(|b| {
                        let d = b.feet.distance_xz(body.feet);
                        let off = off_deg(body.yaw, body.feet, b.feet);
                        (d >= FIRE_MIN_M && d <= FIRE_BANDS[4] && off <= FIRE_ON_DEG)
                            .then_some((off, d))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, d)) = on {
                    let band = FIRE_BANDS.iter().position(|&b| d <= b).unwrap_or(4);
                    self.fire.entry(body.gun.class()).or_insert([0; 5])[band] += 1;
                }
            }
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
                    self.metric_rays += 2;
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
        // The grenades going off.
        let mut gone = Vec::new();
        self.blasts.retain(|b| {
            let off = tick >= b.time;
            if off {
                gone.push(*b);
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
                self.metric_rays += 1;
                if !sight.visible(blast.at + Vec3::new(0.0, 0.3, 0.0), Game::chest(body)) {
                    continue;
                }
                let who = if place == blast.thrower {
                    0
                } else if body.team == team {
                    1
                } else {
                    2
                };
                let dmg = blast_damage(d);
                self.blast_dmg[who] += dmg;
                self.blast_kills[who] += u32::from(dmg >= body.health);
            }
        }
        game.metric_rays += std::mem::take(&mut self.metric_rays);
    }

    /// A reload over (and the 4 s after its start): in a line, died.
    fn reload_done(&mut self, r: &Reload) {
        if !r.in_line {
            return;
        }
        self.reloads_in_line += 1;
        let recs = &self.recs[r.place];
        let i0 = self.index(r.at);
        let i1 = self.index(r.at + RELOAD_DIED_MS).min(recs.len().saturating_sub(1));
        let died = (i0.max(1)..=i1).any(|i| recs[i - 1].alive && !recs[i].alive);
        self.reloads_in_line_died += u32::from(died);
    }

    /// The record index of `tick`.
    fn index(&self, tick: u32) -> usize {
        (tick.saturating_sub(self.first_tick) / TICK_MS) as usize
    }

    /// A bot's note: takes, stuck watches, grenades thrown.
    pub(super) fn note(&mut self, note: &Note, game: &Game) {
        if note.text.starts_with("takes ") {
            if let Some(t) = self.takes.get_mut(note.place) {
                t.push(note.time);
            }
        } else if note.text.starts_with("is stuck at") {
            if let Some(t) = self.stuck_notes.get_mut(note.place) {
                t.push(note.time);
            }
        } else if let Some((aim, time)) = thrown_at(&note.text)
            && let Some(body) = game.bodies.get(note.place)
        {
            self.grenades += 1;
            let at = grenade_lands(&game.world.sight, Game::eye(body), aim);
            self.blasts.push(Blast {
                thrower: note.place,
                at,
                time,
            });
        }
    }

    /// The counts, as the `owner-test:` line and the sums' `owner` object.
    pub(super) fn finish(&mut self, game: &Game) -> (String, serde_json::Value) {
        for r in std::mem::take(&mut self.reloads) {
            self.reload_done(&r);
        }
        let n = self.recs.len();
        let bot = |place: Place| place < n;
        let ticks = self.recs.first().map_or(0, Vec::len);
        let tick_s = TICK_S as f64;
        let alive_ticks: u64 = self
            .recs
            .iter()
            .map(|r| r.iter().filter(|r| r.alive).count() as u64)
            .sum();
        let bot_min = alive_ticks as f64 * tick_s / 60.0;

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

        // Jumps.
        let mut by_kind: BTreeMap<&str, [u32; 2]> = BTreeMap::new();
        let (mut need, mut stand_turn, mut still_ticks, mut let_go, mut forward_presses) =
            ([0u32; 2], 0u32, 0u64, 0u32, 0u32);
        let (mut route_up, mut route_failed, mut sprint_jumps) = (0u32, 0u32, 0u32);
        let mut presses = self.presses.clone();
        presses.sort_by_key(|p| (p.place, p.t));
        let (mut series, mut series3, mut in_series) = (0u32, 0u32, 0u32);
        let mut run: Option<(Place, u32, Vec3, u32)> = None;
        let mut close_series = |len: u32| {
            if len >= 2 {
                series += 1;
                in_series += len;
            }
            if len >= 3 {
                series3 += 1;
            }
        };
        for p in &presses {
            let recs = &self.recs[p.place];
            let i = self.index(p.t).min(recs.len().saturating_sub(1));
            let keys = |k: usize| recs.get(k).map_or(0, |r| r.keys);
            let forward = keys(i) & key::FORWARD != 0;
            let entry = by_kind.entry(p.by).or_default();
            entry[0] += 1;
            entry[1] += u32::from(!forward);
            // The run-up: forward (not back) ticks right before the press,
            // and the ticks with no walk key before them.
            let f = |k: usize| keys(k) & key::FORWARD != 0 && keys(k) & key::BACK == 0;
            let mut k = i;
            let mut f_run = 0;
            while k > 0 && f(k - 1) && f_run < 200 {
                k -= 1;
                f_run += 1;
            }
            let mut still = 0;
            while k > 0 && keys(k - 1) & WALK_KEYS == 0 && still < 200 {
                k -= 1;
                still += 1;
            }
            if p.by == "route" {
                if f_run <= STAND_RUN && still >= STAND_STILL {
                    stand_turn += 1;
                    still_ticks += still as u64;
                }
                if let Some(landing) = p.landing {
                    for (c, m) in need.iter_mut().zip(NEED_M) {
                        *c += u32::from(landing.y - p.feet.y > m);
                    }
                    if let Some(l) = self.landings.iter().find(|l| l.place == p.place && l.at == p.t) {
                        if l.to.y >= landing.y - UP_SLACK_M {
                            route_up += 1;
                        } else {
                            route_failed += 1;
                        }
                    }
                }
            }
            if forward {
                forward_presses += 1;
                let held = (i..recs.len()).take_while(|&k| keys(k) & key::FORWARD != 0).count();
                let_go += u32::from(held <= LET_GO);
            }
            // Series.
            match run {
                Some((place, t, at, len))
                    if place == p.place
                        && p.t <= t + SERIES_MS
                        && p.feet.distance_xz(at) <= SERIES_M =>
                {
                    run = Some((place, p.t, p.feet, len + 1));
                }
                _ => {
                    if let Some((.., len)) = run {
                        close_series(len);
                    }
                    run = Some((p.place, p.t, p.feet, 1));
                }
            }
        }
        if let Some((.., len)) = run {
            close_series(len);
        }
        sprint_jumps += self.landings.iter().filter(|l| l.sprint).count() as u32;

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

        // Stuck.
        let mut stuck: BTreeMap<&str, (u32, f32)> = BTreeMap::new();
        for place in 0..n {
            for (t0, t1, kind) in self.stuck_episodes(place) {
                let e = stuck.entry(kind).or_default();
                e.0 += 1;
                e.1 += t1 - t0;
            }
        }
        let stuck_n: u32 = stuck.values().map(|e| e.0).sum();
        let stuck_s: f32 = stuck.values().map(|e| e.1).sum();

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
        let pct = |a: f64, b: f64| if b > 0.0 { (1000.0 * a / b).round() / 10.0 } else { 0.0 };
        // Two objects: one json! of all of them goes past the macro's
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
            "hits_first": first_hits,
            "hits_side": hits_side,
            "turned_3s": turned,
            "turned_3s_pct": pct(turned.into(), hits_side.into()),
            "died_unturned": died_unturned,
            "presses": self.presses.len(),
            "presses_by": by_kind,
            "presses_no_forward": by_kind.values().map(|v| v[1]).sum::<u32>(),
            "route_need_1m": need[0],
            "route_need_1_2m": need[1],
            "route_stand_turn": stand_turn,
            "route_still_ticks": still_ticks,
            "f_let_go": let_go,
            "f_presses": forward_presses,
            "press_series": series,
            "press_series_presses": in_series,
            "press_series_3": series3,
            "route_up": route_up,
            "route_failed": route_failed,
            "sprint_jumps": sprint_jumps,
        });
        let more = serde_json::json!({
            "stuck": stuck_n,
            "stuck_s": stuck_s.round(),
            "stuck_by": stuck,
            "calm_walk_s": (calm_walk as f64 * tick_s).round(),
            "calm_sprint_pct": pct(calm_sprint as f64, calm_walk as f64),
            "calm_mps": if calm_walk > 0 { (100.0 * calm_m / (calm_walk as f64 * tick_s)).round() / 100.0 } else { 0.0 },
            "idle_s": idle_s,
            "idle_by_play": idle,
            "drops_2_5m": drops[0] - drops[1],
            "drops_4m": drops[1],
            "uturns": uturns,
            "takes_moving": takes_moving,
            "uturns_per_bot_min": if bot_min > 0.0 { (1000.0 * f64::from(uturns) / bot_min).round() / 1000.0 } else { 0.0 },
            "close_pairs": close_eps,
            "close_pair_s": close_s,
            "one_body": one_eps,
            "one_body_s": (f64::from(one_ticks) * tick_s).round(),
            "grenades": self.grenades,
            "grenade_self_dmg": (100.0 * self.blast_dmg[0]).round() / 100.0,
            "grenade_mate_dmg": (100.0 * self.blast_dmg[1]).round() / 100.0,
            "grenade_enemy_dmg": (100.0 * self.blast_dmg[2]).round() / 100.0,
            "grenade_kills": self.blast_kills,
            "team_kills_gun": game.team_kills,
            "team_hits": game.team_hits,
            "ticks": ticks,
        });
        if let (Some(all), serde_json::Value::Object(more)) = (json.as_object_mut(), more) {
            all.extend(more);
        }
        let line = format!(
            "owner-test: dry {} s (deaths {}), reloading {} s (deaths {}), reloads {} in a line {} died {}; fire >=35 m by class {}; side hits {} turned in 3 s {} died unturned {}\n  \
             owner-test jumps: presses {} by {:?}, no forward {}; route need >1 m {} >=1.2 m {}, stand-turn {} ({} still ticks), up {} failed {}; forward let go {} of {}; series {} ({} presses, {} of 3+); sprint jumps {}\n  \
             owner-test moves: stuck {} ({:.0} s) {:?}; calm sprint {:.1} % of {:.0} s walking, {:.2} m/s; idle {} s {:?}; drops 2.5-4 m {} >=4 m {}; U-turns {} of {} takes moving ({:.3} a bot-minute); close pairs {} ({} s); one body {} ({:.0} s)\n  \
             owner-test other: grenades {} damage self {:.2} mates {:.2} enemies {:.2}, would kill {:?}; team kills by gunfire {}, team hits {}",
            json["dry_s"], self.deaths_dry, json["reload_s"], self.deaths_reloading,
            self.reloads_n, self.reloads_in_line, self.reloads_in_line_died,
            fire_far.iter().map(|(c, v)| format!("{c} {}/{}", v[1], v[0])).collect::<Vec<_>>().join(" "),
            hits_side, turned, died_unturned,
            self.presses.len(), by_kind, json["presses_no_forward"], need[0], need[1], stand_turn, still_ticks,
            route_up, route_failed, let_go, forward_presses, series, in_series, series3, sprint_jumps,
            stuck_n, stuck_s, stuck,
            json["calm_sprint_pct"].as_f64().unwrap_or(0.0), calm_walk as f64 * tick_s, json["calm_mps"].as_f64().unwrap_or(0.0),
            idle_s, json["idle_by_play"], drops[0] - drops[1], drops[1],
            uturns, takes_moving, json["uturns_per_bot_min"].as_f64().unwrap_or(0.0),
            close_eps, close_s, one_eps, f64::from(one_ticks) * tick_s,
            self.grenades, self.blast_dmg[0], self.blast_dmg[1], self.blast_dmg[2], self.blast_kills,
            game.team_kills, game.team_hits,
        );
        (line, json)
    }

    /// One place's stuck episodes (`stuck.py` `detect` and `merge`): from
    /// a sample a second of each life (broken where the body moves over
    /// 25 m in one), the windows of each kind, merged when under 2 s apart.
    /// Each episode's start and end, s, and its first kind.
    fn stuck_episodes(&self, place: Place) -> Vec<(f32, f32, &'static str)> {
        let recs = &self.recs[place];
        let secs: Vec<usize> = (0..recs.len() / TPS).map(|s| s * TPS + TPS - 1).collect();
        // Lives: runs of living samples.
        let mut lives: Vec<Vec<usize>> = Vec::new();
        let mut cur: Vec<usize> = Vec::new();
        for &k in &secs {
            if !recs[k].alive {
                if !cur.is_empty() {
                    lives.push(std::mem::take(&mut cur));
                }
                continue;
            }
            if let Some(&last) = cur.last()
                && recs[last].feet.distance_xz(recs[k].feet) > 25.0
            {
                lives.push(std::mem::take(&mut cur));
            }
            cur.push(k);
        }
        if !cur.is_empty() {
            lives.push(cur);
        }
        let t_of = |k: usize| (self.first_tick as f32 + k as f32 * TICK_MS as f32) / 1000.0;
        let share = |k0: usize, k1: usize, test: &dyn Fn(&Rec) -> bool| {
            let n = (k1 - k0 + 1) as f32;
            recs[k0..=k1].iter().filter(|r| test(r)).count() as f32 / n
        };
        let opposing = |k0: usize, k1: usize| {
            let (mut n, mut c) = (0u32, [0u32; 4]);
            for r in &recs[k0..=k1] {
                if r.keys & WALK_KEYS != 0 {
                    n += 1;
                    for (i, bit) in [key::FORWARD, key::BACK, key::LEFT, key::RIGHT].into_iter().enumerate() {
                        c[i] += u32::from(r.keys & bit != 0);
                    }
                }
            }
            if n == 0 {
                0.0
            } else {
                (c[0].min(c[1]) + c[2].min(c[3])) as f32 / n as f32
            }
        };
        let stuck_line = |k0: usize, k1: usize| {
            let (t0, t1) = (t_of(k0) * 1000.0, t_of(k1) * 1000.0 + 1500.0);
            self.stuck_notes[place].iter().any(|&t| (t as f32) >= t0 && (t as f32) <= t1)
        };
        let radius = |pts: &[usize]| {
            let n = pts.len() as f32;
            let cx = pts.iter().map(|&k| recs[k].feet.x).sum::<f32>() / n;
            let cz = pts.iter().map(|&k| recs[k].feet.z).sum::<f32>() / n;
            pts.iter()
                .map(|&k| (recs[k].feet.x - cx).hypot(recs[k].feet.z - cz))
                .fold(0.0, f32::max)
        };
        let mut marks: Vec<(f32, f32, &'static str)> = Vec::new();
        for life in &lives {
            let n = life.len();
            let t_spawn = t_of(life[0]);
            for i in 0..n {
                for &(name, dmin, rmax, wmin, amax, holdx) in &STUCK_KINDS {
                    let ok = |j: usize| -> bool {
                        let pts = &life[i..=j];
                        if radius(pts) > rmax {
                            return false;
                        }
                        if name == "loop" {
                            let (a, b) = (recs[pts[0]].feet, recs[pts[pts.len() - 1]].feet);
                            let path: f32 = pts.windows(2).map(|w| recs[w[0]].feet.distance_xz(recs[w[1]].feet)).sum();
                            if a.distance_xz(b) > LOOP_NET_M || path < LOOP_PATH_M {
                                return false;
                            }
                        }
                        let (k0, k1) = (pts[0], pts[pts.len() - 1]);
                        if share(k0, k1, &|r: &Rec| r.keys & WALK_KEYS != 0) < wmin {
                            return false;
                        }
                        if let Some(amax) = amax
                            && share(k0, k1, &|r: &Rec| r.keys & (key::AIM | key::FIRE) != 0) >= amax
                        {
                            return false;
                        }
                        if holdx {
                            let held = pts.iter().filter(|&&k| recs[k].hold).count() as f32 / pts.len() as f32;
                            if held >= if name == "nohead" { 0.5 } else { 1e-9 } {
                                return false;
                            }
                        }
                        if name == "push" {
                            let mut sp: Vec<f32> = pts.iter().map(|&k| recs[k].speed).collect();
                            sp.sort_by(f32::total_cmp);
                            let med = (sp[(sp.len() - 1) / 2] + sp[sp.len() / 2]) / 2.0;
                            if med > PUSH_STILL_SPEED
                                && (opposing(k0, k1) > PUSH_OPPOSING_MAX
                                    || (med > PUSH_SPEED && !stuck_line(k0, k1)))
                            {
                                return false;
                            }
                        }
                        true
                    };
                    let mut j = i;
                    while j < n && t_of(life[j]) - t_of(life[i]) < dmin {
                        j += 1;
                    }
                    if j >= n || j - i + 1 < 3 || !ok(j) {
                        continue;
                    }
                    while j + 1 < n && ok(j + 1) {
                        j += 1;
                    }
                    marks.push((t_of(life[i]), t_of(life[j]), name));
                }
                if t_of(life[i]) - t_spawn < SPAWN_GRACE_S {
                    continue;
                }
                let ok_stand = |j: usize| -> bool {
                    let pts = &life[i..=j];
                    if radius(pts) > STAND_R {
                        return false;
                    }
                    let (k0, k1) = (pts[0], pts[pts.len() - 1]);
                    if share(k0, k1, &|r: &Rec| r.keys & WALK_KEYS != 0) > STAND_WALK_MAX
                        || share(k0, k1, &|r: &Rec| r.keys & (key::AIM | key::FIRE) != 0) >= 0.5
                    {
                        return false;
                    }
                    pts.iter().filter(|&&k| recs[k].travel).count() as f32 >= 0.7 * pts.len() as f32
                };
                let mut j = i;
                while j < n && t_of(life[j]) - t_of(life[i]) < STAND_S {
                    j += 1;
                }
                if j >= n || j - i + 1 < 3 || !ok_stand(j) {
                    continue;
                }
                while j + 1 < n && ok_stand(j + 1) {
                    j += 1;
                }
                marks.push((t_of(life[i]), t_of(life[j]), "stand"));
            }
        }
        // Merged: an episode's kind is the first of push, nohead, loop,
        // stand it holds.
        let order = |k: &str| ["push", "nohead", "loop", "stand"].iter().position(|o| *o == k).unwrap_or(4);
        marks.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut eps: Vec<(f32, f32, &'static str)> = Vec::new();
        for (t0, t1, k) in marks {
            match eps.last_mut() {
                Some(e) if t0 <= e.1 + MERGE_GAP_S => {
                    e.1 = e.1.max(t1);
                    if order(k) < order(e.2) {
                        e.2 = k;
                    }
                }
                _ => eps.push((t0, t1, k)),
            }
        }
        eps
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

#[test]
fn route_presses_from_a_standstill_with_forward_let_go_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 160, |k, game| {
        // Bot 0: walks, stands 7 ticks, 3 ticks of forward, presses at
        // tick 100 for a 1.3 m rise, lets forward go 2 ticks on.
        let keys0 = match k {
            0..90 => key::FORWARD,
            90..97 => 0,
            97..100 => key::FORWARD,
            100 => key::FORWARD | key::JUMP,
            101..103 => key::FORWARD,
            _ => 0,
        };
        // Bot 1 (the near miss): runs into its press for a 0.9 m rise and
        // holds forward on.
        let keys1 = match k {
            100 => key::FORWARD | key::JUMP,
            0..111 => key::FORWARD,
            _ => 0,
        };
        put(game, 0, 10.0, 10.0, keys0);
        put(game, 1, 10.0, 20.0, keys1);
        let mut nows = vec![bot_now(), bot_now()];
        if k == 100 {
            nows[0].as_mut().unwrap().jump = Some(("route", Some(Vec3::new(11.0, 1.3, 10.0))));
            nows[1].as_mut().unwrap().jump = Some(("route", Some(Vec3::new(11.0, 0.9, 20.0))));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["presses"], 2);
    assert_eq!(json["route_stand_turn"], 1, "{json}");
    assert_eq!(json["route_still_ticks"], 7);
    assert_eq!(json["f_let_go"], 1);
    assert_eq!(json["route_need_1m"], 1);
    assert_eq!(json["route_need_1_2m"], 1);
    assert_eq!(json["presses_no_forward"], 0);
    assert_eq!(json["press_series"], 0);
}

#[test]
fn presses_again_and_again_at_one_place_are_a_series() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 120, |k, game| {
        // Bot 0: three presses 1.5 s apart where it stands; bot 1: two
        // presses 4 s apart (not a series), with no forward key.
        let k0 = if [10, 40, 70].contains(&k) { key::JUMP | key::FORWARD } else { 0 };
        let k1 = if [10, 90].contains(&k) { key::JUMP } else { 0 };
        put(game, 0, 10.0, 10.0, k0);
        put(game, 1, 10.0, 20.0, k1);
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["presses"], 5);
    assert_eq!(json["press_series"], 1, "{json}");
    assert_eq!(json["press_series_presses"], 3);
    assert_eq!(json["press_series_3"], 1);
    assert_eq!(json["presses_no_forward"], 2);
}

#[test]
fn a_bot_hit_from_behind_is_counted_turned_or_not() {
    // Bot 0 hit from behind turns within 2 s; bot 2 hit from behind
    // never turns; bot 4 hit from ahead (the near miss) is no side hit.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1, 0, 1]);
    play(&mut game, &mut tally, 120, |k, game| {
        put(game, 0, 10.0, 20.0, 0);
        put(game, 1, 10.0, 10.0, 0);
        put(game, 2, 20.0, 20.0, 0);
        put(game, 3, 20.0, 10.0, 0);
        put(game, 4, 30.0, 10.0, 0);
        put(game, 5, 30.0, 20.0, 0);
        // Facing +z (yaw 0): 1 and 3 are behind, 5 ahead.
        for p in [0, 2, 4] {
            game.bodies[p].yaw = 0.0;
        }
        if k >= 40 {
            game.bodies[0].yaw = std::f32::consts::PI;
        }
        if k == 20 {
            game.hit_log.extend([(game.tick, 1, 0), (game.tick, 3, 2), (game.tick, 5, 4)]);
        }
        vec![bot_now(); 6]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["hits_first"], 3);
    assert_eq!(json["hits_side"], 2, "{json}");
    assert_eq!(json["turned_3s"], 1);
}

#[test]
fn calm_walking_counts_its_sprint_share_and_a_fight_does_not() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 40, |k, game| {
        let keys = if k < 20 { key::FORWARD | key::SPRINT } else { key::FORWARD };
        put(game, 0, 10.0, 10.0, keys);
        // Aiming: not calm.
        put(game, 1, 10.0, 20.0, key::FORWARD | key::AIM);
        game.bodies[0].step_m = 0.2;
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["calm_sprint_pct"], 50.0, "{json}");
    assert_eq!(json["calm_walk_s"], 2.0);
}

#[test]
fn five_calm_seconds_standing_are_idle_and_four_are_not() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        let s = k / 20;
        put(game, 0, 10.0, 10.0, if s < 6 { 0 } else { key::FORWARD });
        put(game, 1, 10.0, 20.0, if s < 4 { 0 } else { key::FORWARD });
        game.bodies[0].step_m = if s < 6 { 0.0 } else { 0.2 };
        game.bodies[1].step_m = if s < 4 { 0.0 } else { 0.2 };
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["idle_s"], 6, "{json}");
    assert_eq!(json["idle_by_play"]["Roam"], 6.0);
}

#[test]
fn a_drop_of_four_metres_in_a_second_is_counted() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 60, |k, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 10.0, 20.0, 0);
        game.bodies[0].feet.y = if k < 20 { 10.0 } else { 4.5 };
        game.bodies[1].feet.y = if k < 20 { 10.0 } else { 8.0 };
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["drops_4m"], 1, "{json}");
    assert_eq!(json["drops_2_5m"], 0);
}

#[test]
fn a_take_that_turns_a_walking_bot_back_is_a_u_turn() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 100, |k, game| {
        // Bot 0: east 0.1 m a tick, back west after tick 50; bot 1: east,
        // then north (90 degrees).
        let k = k as f32;
        let x0 = if k <= 50.0 { 5.0 + 0.1 * k } else { 10.0 - 0.1 * (k - 50.0) };
        put(game, 0, x0, 10.0, key::FORWARD);
        let (x1, z1) = if k <= 50.0 { (5.0 + 0.1 * k, 20.0) } else { (10.0, 20.0 + 0.1 * (k - 50.0)) };
        put(game, 1, x1, z1, key::FORWARD);
        vec![bot_now(), bot_now()]
    });
    let t = tally.first_tick + 50 * TICK_MS;
    tally.takes[0].push(t);
    tally.takes[1].push(t);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["takes_moving"], 2);
    assert_eq!(json["uturns"], 1, "{json}");
}

#[test]
fn teammates_close_five_seconds_and_bodies_in_one_another_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0]);
    play(&mut game, &mut tally, 160, |k, game| {
        let s = k / 20;
        put(game, 0, 10.0, 10.0, 0);
        // Within 1 m for 6 s, then 0.2 m (one body) for a second.
        let x1 = if s < 6 { 11.0 } else if s < 7 { 10.2 } else { 30.0 };
        put(game, 1, x1, 10.0, 0);
        // Within 1 m for 4 s, never closer than 0.6 m.
        put(game, 2, 10.0, 30.0, 0);
        let x3 = if s < 4 { 10.6 } else { 30.0 };
        put(game, 3, x3, 30.0, 0);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["close_pairs"], 1, "{json}");
    assert_eq!(json["close_pair_s"], 7);
    assert_eq!(json["one_body"], 1);
    assert_eq!(json["one_body_s"], 1.0);
}

#[test]
fn the_stuck_detector_finds_a_push_and_a_stand() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0]);
    play(&mut game, &mut tally, 400, |k, game| {
        let s = k / 20;
        // Bot 0 pushes forward without moving for 5 s, then walks off.
        if s < 5 {
            put(game, 0, 10.0, 10.0, key::FORWARD);
        } else {
            put(game, 0, 10.0 + 0.2 * (k - 100) as f32, 10.0, key::FORWARD);
            game.bodies[0].step_m = 0.2;
        }
        // Bot 1 (near miss) pushes 2 s.
        if s < 2 {
            put(game, 1, 10.0, 20.0, key::FORWARD);
        } else {
            put(game, 1, 10.0 + 0.2 * (k - 40) as f32, 20.0, key::FORWARD);
            game.bodies[1].step_m = 0.2;
        }
        // Bots 2 and 3 stand all along with no key, 2 with a goal to walk
        // to, 3 (near miss) holding.
        put(game, 2, 10.0, 30.0, 0);
        put(game, 3, 20.0, 30.0, 0);
        let mut nows = vec![bot_now(); 4];
        nows[2].as_mut().unwrap().travel = true;
        nows[3].as_mut().unwrap().hold = true;
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["stuck_by"]["push"][0], 1, "{json}");
    assert_eq!(json["stuck_by"]["stand"][0], 1, "{json}");
    assert_eq!(json["stuck"], 2);
}

#[test]
fn deaths_dry_or_reloading_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0]);
    play(&mut game, &mut tally, 100, |k, game| {
        for p in 0..4 {
            put(game, p, 10.0 + 5.0 * p as f32, 10.0, 0);
        }
        // Bot 0 dry, dies at 40; bot 1 (near miss) not dry, dies at 40.
        // Bot 2 dies 0.5 s after its reload; bot 3 (near miss) 2 s after.
        let dead = |at: usize| k >= at;
        game.bodies[0].alive = !dead(40);
        game.bodies[1].alive = !dead(40);
        game.bodies[2].alive = !dead(70);
        game.bodies[3].alive = !dead(90);
        let mut nows = vec![bot_now(); 4];
        nows[0].as_mut().unwrap().dry = true;
        for p in [2, 3] {
            nows[p].as_mut().unwrap().reload = Some((20 * TICK_MS, 60 * TICK_MS));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["deaths_dry"], 1, "{json}");
    assert_eq!(json["deaths_reloading"], 1);
    assert_eq!(json["reloads"], 2);
    assert_eq!(json["dry_s"], 2.0);
}

#[test]
fn a_reload_in_an_enemys_line_is_counted_and_one_behind_a_wall_is_not() {
    // The soak field's wall stands at x = 40 for z < 25.
    let (mut game, mut tally) = made_up(&[0, 1, 0]);
    play(&mut game, &mut tally, 160, |k, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 30.0, 10.0, 0);
        put(game, 2, 60.0, 10.0, 0);
        // Bot 0 dies 3 s into its reload.
        game.bodies[0].alive = k < 60;
        let mut nows = vec![bot_now(); 3];
        for p in [0, 2] {
            nows[p].as_mut().unwrap().reload = Some((TICK_MS, 40 * TICK_MS));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["reloads"], 2);
    assert_eq!(json["reloads_in_line"], 1, "{json}");
    assert_eq!(json["reloads_in_line_died"], 1);
}

#[test]
fn fire_counts_by_the_range_of_the_enemy_in_view() {
    let (mut game, mut tally) = made_up(&[0, 1, 1]);
    play(&mut game, &mut tally, 10, |_, game| {
        // Bot 0 fires facing +z: the enemy ahead 28 m; one to its side.
        put(game, 0, 5.0, 5.0, key::FIRE);
        game.bodies[0].yaw = 0.0;
        put(game, 1, 5.0, 33.0, 0);
        put(game, 2, 25.0, 5.0, 0);
        vec![bot_now(); 3]
    });
    let class = game.bodies[0].gun.class();
    let (_, json) = tally.finish(&game);
    assert_eq!(json["fire_by_class"][class], serde_json::json!([0, 0, 10, 0, 0]), "{json}");
}

#[test]
fn a_grenade_at_ones_feet_hurts_the_thrower_and_a_mate_not_a_far_enemy() {
    let (mut game, mut tally) = made_up(&[0, 0, 1]);
    let note = |game: &mut Game, tally: &mut OwnerTally| {
        let note = Note {
            place: 0,
            time: game.tick,
            text: "throws a grenade at (11, 0, 10), blast at 1000".to_owned(),
        };
        tally.note(&note, game);
    };
    play(&mut game, &mut tally, 30, |_, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 12.0, 10.0, 0);
        put(game, 2, 30.0, 10.0, 0);
        vec![bot_now(); 3]
    });
    // Thrown at tick 30, gone off at 1000 ms already: once more ticks run.
    note(&mut game, &mut tally);
    game.tick += TICK_MS;
    tally.step(&mut game, &[bot_now(), bot_now(), bot_now()]);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grenades"], 1);
    assert!(json["grenade_self_dmg"].as_f64().unwrap() > 0.5, "{json}");
    assert!(json["grenade_mate_dmg"].as_f64().unwrap() > 0.3);
    assert_eq!(json["grenade_enemy_dmg"], 0.0);
    assert_eq!(json["grenade_kills"][0], 1);
}

#[test]
fn the_blast_falls_off_to_its_minimum_at_ten_metres() {
    assert!((blast_damage(0.0) - BLAST_MAX).abs() < 1e-6);
    assert!((blast_damage(BLAST_M) - BLAST_MIN).abs() < 1e-6);
    assert_eq!(blast_damage(10.5), 0.0);
}
