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
//! - vertical: certain drops and climbs a kilometre walked, and the share
//!   of the time above the players' p75 height (`grid-record.md` section V);
//! - grenades on their own side, and bots inside a blast that were outside
//!   at the throw (the stand-in has no grenade damage: counted only); the
//!   blast where the thrower's view at the launch sends it (`arc.rs`), and
//!   the throws that went short, were refused, or were cut by a death or a
//!   shot;
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
use crate::v026g::bots::reflex::gate;
use crate::v026g::bots::reflex::GrenadeStats;
use crate::v026g::bots::view::Place;
use super::{Bots, Game, Go, Landing, Note};
use crate::v026g::drive::key;

mod campers;
mod fight;
mod golden;
mod grenades;
mod jumps;
mod moving;
mod pre;
mod stock;

use campers::*;
use fight::*;
use grenades::*;
use jumps::*;
use moving::*;
use pre::*;

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
    pitch: f32,
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
    /// The bot's target is a living enemy farther than the top of its
    /// weapon's preferred band ([`target_past_band`]).
    past_band: bool,
}

/// What the bench reads of a bot at a tick: out of rounds with nothing
/// to load, its last reload (start, end), the jump it pressed (why, and
/// the route's take-off and landing for a route jump), its play, its
/// play's step (once a second: none between), how far its goal is, and
/// the reflex tick, if any, at which the weapon's range alone held its shot.
#[derive(Clone, Debug, Default)]
struct BotNow {
    dry: bool,
    reload: Option<(u32, u32)>,
    jump: Option<(&'static str, Option<(Vec3, Vec3)>)>,
    play: String,
    step: Option<String>,
    goal_m: Option<f32>,
    range_held: Option<u32>,
    /// What its reflex's grenade logic has counted this life.
    grenades: GrenadeStats,
    /// Its view is held for a throw now, and how long ago, ms, it let go of
    /// its last grenade.
    throw_held: bool,
    thrown_ago_ms: Option<u32>,
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
    /// The bench's stand-in camper (`SRP_BENCH_CAMPER`): its counts are
    /// also given apart.
    pub(crate) camper: Option<Place>,
    metric_rays: u64,
    /// The fight counts ([`FightCounts`], `fight.rs`).
    fight: FightCounts,
    /// The jumps counts ([`JumpCounts`], `jumps.rs`).
    jumps: JumpCounts,
    /// The moving counts ([`MoveCounts`], `moving.rs`).
    moving: MoveCounts,
    /// The grenades counts ([`GrenadeCounts`], `grenades.rs`).
    grenades: GrenadeCounts,
    /// The campers counts ([`CamperCounts`], `campers.rs`).
    campers: CamperCounts,
}

impl OwnerTally {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            recs: vec![Vec::new(); n],
            is_bot: vec![false; n],
            status: vec![(false, false); n],
            fight: FightCounts::new(n),
            jumps: JumpCounts::default(),
            moving: MoveCounts::new(n),
            grenades: GrenadeCounts::default(),
            campers: CamperCounts::default(),
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
                let gates = bot.reflex.gates();
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
                    // The range was the only gate that failed (with the
                    // gate tally on, the wall test too was made).
                    range_held: (gates.at.abs_diff(tick) <= 2 * TICK_MS && gates.bits == gate::RANGE).then_some(gates.at),
                    grenades: bot.reflex.grenade_stats(),
                    throw_held: bot.reflex.view_held(),
                    thrown_ago_ms: bot.reflex.thrown_ago(tick),
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
        self.fight_hits_taken(game, tick);
        self.jumps_landings(game);
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
            self.fight_death(place, tick, &prev, body);
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
                pitch: body.pitch,
                speed: body.step_m / TICK_S,
                calm: body.keys & (key::AIM | key::FIRE) == 0 && !enemy_near,
                carrying: body.carrying.is_some(),
                air: body.flight.is_some() || body.air_until > tick,
                play,
                hold,
                travel,
                past_band: target_past_band(game, place, body),
            };
            self.recs[place].push(rec);
            let Some(now) = now else {
                continue;
            };
            self.grenades_keys(place, tick, body, &prev);
            self.grenades_logic(place, &now.grenades);
            self.fight.throw_state = (now.throw_held, now.thrown_ago_ms);
            self.fight_ammo(place, tick, body, now);
            self.jumps_press(place, tick, body, &prev, now, &rec);
            self.fight_fire(game, place, body, &rec, &sight, &mut rays);
            self.fight_range_held(place, body, now);
            self.fight_switch(game, place, body, &sight, &mut rays);
        }
        self.fight_hits_scored(game);
        self.campers_kills(game, &sight, &mut rays, tick);
        self.fight_reloads(game, &sight, &mut rays, tick);
        self.grenades_off(game, &sight, &mut rays, tick);
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

    /// The counts, as the `owner-test` lines and the sums' `owner` object: each topic works out its own
    /// ([`Self::fight_part`], ...); what they share is the bots' minutes and the rounding ([`Scale`]).
    pub(crate) fn finish(&mut self, game: &Game) -> (String, serde_json::Value) {
        let Topics { core, parts } = self.finish_topics(game);
        let mut json = core;
        let mut text = Vec::new();
        for (_, part) in parts {
            json.extend(part.json);
            text.push(part.text);
        }
        (text.join("\n  "), serde_json::Value::Object(json))
    }

    /// [`Self::finish`] before it joins the topics: what the run shares (`bot_min`, `ticks`) and each topic's
    /// [`Part`] under its name (the golden counts keep one file a topic).
    pub(super) fn finish_topics(&mut self, game: &Game) -> Topics {
        for r in std::mem::take(&mut self.fight.reloads) {
            self.reload_done(&r);
        }
        let n = self.recs.len();
        let ticks = self.recs.first().map_or(0, Vec::len);
        let tick_s = TICK_S as f64;
        let bot_alive_ticks: u64 = (0..n)
            .filter(|&p| self.is_bot[p])
            .map(|p| self.recs[p].iter().filter(|r| r.alive).count() as u64)
            .sum();
        let bot_min = bot_alive_ticks as f64 * tick_s / 60.0;
        let scale = Scale { tick_s, bot_min };
        let mut presses = self.jumps.presses.clone();
        presses.sort_by_key(|p| (p.place, p.t));
        let parts = vec![
            ("fight", self.fight_part(&scale)),
            ("jumps", self.jumps_part(game, &presses)),
            ("moving", self.moving_part(game, &presses, &scale)),
            ("vertical", self.vertical_part(game)),
            ("grenades", self.grenades_part(game)),
            ("stock", self.stock_part(game)),
            ("campers", self.campers_part()),
        ];
        let mut core = serde_json::Map::new();
        core.insert("bot_min".to_owned(), serde_json::json!(bot_min.round()));
        core.insert("ticks".to_owned(), serde_json::json!(ticks));
        Topics { core, parts }
    }
}

/// The counts before they are joined: the keys the run shares, and each topic's part by name.
pub(super) struct Topics {
    pub(super) core: serde_json::Map<String, serde_json::Value>,
    pub(super) parts: Vec<(&'static str, Part)>,
}

/// What the counts share of the whole run: its length in the bots' minutes, and how rates and shares are rounded.
pub(super) struct Scale {
    pub(super) tick_s: f64,
    pub(super) bot_min: f64,
}

impl Scale {
    /// A count a bot-minute.
    pub(super) fn per_min(&self, x: f64) -> f64 {
        if self.bot_min > 0.0 { (1000.0 * x / self.bot_min).round() / 1000.0 } else { 0.0 }
    }

    /// `a` of `b`, percent.
    pub(super) fn pct(a: f64, b: f64) -> f64 {
        if b > 0.0 { (1000.0 * a / b).round() / 10.0 } else { 0.0 }
    }

    /// `a` of `b`.
    pub(super) fn rate(a: u32, b: u32) -> f64 {
        if b > 0 { (1000.0 * f64::from(a) / f64::from(b)).round() / 1000.0 } else { 0.0 }
    }
}

/// What a topic gives at the end: its keys of the `owner` object and its line(s) of the owner-test text.
pub(super) struct Part {
    pub(super) json: serde_json::Map<String, serde_json::Value>,
    pub(super) text: String,
}

impl Part {
    pub(super) fn new(json: serde_json::Value, text: String) -> Self {
        let serde_json::Value::Object(json) = json else {
            unreachable!("a topic's keys are an object");
        };
        Self { json, text }
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
