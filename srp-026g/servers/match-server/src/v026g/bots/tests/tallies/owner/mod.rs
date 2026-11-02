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
//!   in a row to one shooter from one spot, bots entering a held line;
//! - sight (`pr-sight.md`, the sight refresh's paired counts): fire runs and what ended them,
//!   gaps between runs, fire to kill and presses a kill, the targets' sights lost by cause.
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
use crate::v026g::bots::Bot;
use crate::v026g::drive::key;

mod answer;
mod breach;
mod campers;
mod collect_at;
mod crouch;
mod damage_zones;
mod feel;
mod fight;
mod fire_gate;
mod gang_up;
mod golden;
mod grenades;
mod grid;
mod gun_shots;
mod heal;
mod hit_marks;
mod hit_shapes;
mod jumps;
mod kick_gap;
mod live_list;
mod look;
mod metrics;
mod moving;
mod outcomes;
mod outranged;
mod pace;
mod pierce;
mod pin_cover;
mod pitch_bands;
mod pockets;
mod pre;
mod range_leg;
mod resupply;
mod run;
mod scoreboard;
mod search_queue;
mod sense_counts;
mod shot_bands;
mod shot_heights;
mod shot_offsets;
mod sight;
mod skill_hits;
mod smoke;
mod snipers;
mod spacing;
mod sprint_tables;
mod stamina;
mod steals;
mod stock;
mod team_kills;
mod threat_answer;
mod through;
mod topic;
mod worth;
mod zone_deaths;

use campers::*;
use collect_at::*;
use crouch::*;
use damage_zones::*;
use feel::*;
use fight::*;
use fire_gate::FireGate;
use gang_up::*;
use grenades::*;
use grid::*;
use gun_shots::GunShots;
use heal::*;
use hit_marks::*;
use hit_shapes::*;
use jumps::*;
use kick_gap::*;
use live_list::*;
use look::*;
use metrics::*;
use moving::*;
use answer::*;
use outcomes::*;
use outranged::*;
use pace::*;
use pierce::*;
use pin_cover::*;
use pitch_bands::*;
use pockets::*;
use pre::*;
use range_leg::*;
use resupply::*;
use run::*;
use scoreboard::*;
use search_queue::*;
use sense_counts::*;
use snipers::SniperCounts;
use shot_bands::*;
use shot_heights::*;
use shot_offsets::*;
use sight::*;
use skill_hits::SkillHits;
use smoke::*;
use spacing::*;
use sprint_tables::*;
use stamina::*;
use steals::StealCounts;
use stock::*;
use team_kills::*;
use threat_answer::*;
use breach::*;
use through::*;
use topic::*;
use worth::*;
use zone_deaths::*;

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
    /// The elevation the view looks along, radians (the base pitch and the weapon's kick through the pitch unit's scale, as
    /// the bots and the client reckon it): a grenade leaves along it.
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
    /// The bot's tick as the FWDREL probe reads it: `Bot::bench_walk`, `Bot::bench_why`, and whether it had a route.
    walk: u8,
    why: u16,
    has_route: bool,
    /// The motor's own keys, the side step, the step apart and the lip guard of the tick ([`run::Doing`]).
    motor_keys: u32,
    stepped: u8,
    apart: u8,
    guard: u8,
    /// The lip guard's watch ([`LipWatchKind`]) and why it said the keys fall (`Bot::bench_guard_why`): the stuck episodes' print.
    watch: LipWatchKind,
    guard_why: u8,
}

/// [`crate::v026g::bots::drive::LipWatch`] without its data, for the record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LipWatchKind {
    #[default]
    Watching,
    Off,
    GaveUp,
}

impl LipWatchKind {
    fn of(watch: &crate::v026g::bots::drive::LipWatch) -> Self {
        use crate::v026g::bots::drive::LipWatch;
        match watch {
            LipWatch::Watching(_) => Self::Watching,
            LipWatch::Off(_) => Self::Off,
            LipWatch::GaveUp { .. } => Self::GaveUp,
        }
    }

    fn letter(self) -> char {
        match self {
            Self::Watching => 'w',
            Self::Off => 'o',
            Self::GaveUp => 'g',
        }
    }
}

/// The owner counts: the run's record ([`Run`]) and the topics that count from it ([`Topic`], [`topics`]).
pub(crate) struct OwnerTally {
    run: Run,
    topics: Vec<Box<dyn Topic>>,
}

impl OwnerTally {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            run: Run::new(n),
            topics: topics(n),
        }
    }

    /// The scoreboard's context line (`<project> NvN seed S`) and which places are the stand-in human and the stand-in camper.
    pub(crate) fn set_scoreboard(&mut self, context: String, mode: u8, human: Option<Place>, camper: Option<Place>) {
        self.topic::<Feel>().set_human(human, camper);
        self.topic::<LookCounts>().set_human(human);
        self.topic::<CollectAtCounts>().set_stand_in(human.or(camper));
        self.topic::<Scoreboard>().set_context(context, mode, human, camper);
    }

    /// The bench's stand-in camper (`SRP_BENCH_CAMPER`): its counts are also given apart.
    pub(crate) fn set_camper(&mut self, camper: Option<Place>) {
        self.run.camper = camper;
    }

    /// After a game step, before the hit tally takes the game's hits: each topic reads the bots, then
    /// [`Self::step`].
    pub(crate) fn tick(&mut self, game: &mut Game, bots: &Bots) {
        let tick = game.tick;
        // The step once a second, at the stuck detector's samples.
        let sample = self.run.first_tick == 0
            || ((tick.saturating_sub(self.run.first_tick) / TICK_MS) as usize).is_multiple_of(TPS);
        for topic in &mut self.topics {
            topic.bots(bots, game);
        }
        let mut doing = Vec::with_capacity(game.bodies.len());
        for place in 0..game.bodies.len() {
            let bot = bots.bots.get(place).and_then(Option::as_ref);
            for topic in &mut self.topics {
                topic.read(place, bot, tick);
            }
            doing.push(bot.map(|bot| Doing::of(bot, game, place, sample)));
        }
        self.step(game, &doing);
    }

    /// One tick of the counts with what the bots were doing (none: not a bot): the topics' `begin`, each body
    /// recorded and given to the topics, their `end`. Takes the game's logs.
    fn step(&mut self, game: &mut Game, doing: &[Option<Doing>]) {
        let tick = game.tick;
        if self.run.first_tick == 0 {
            self.run.first_tick = tick;
        }
        let mut rays = 0u64;
        self.run.read_game(game);
        for topic in &mut self.topics {
            topic.begin(&self.run, game);
        }
        let sight = game.world.sight.clone();
        for place in 0..game.bodies.len() {
            let (rec, prev) = self.run.push(game, place, doing.get(place).and_then(Option::as_ref));
            let at = BodyAt { game, place, tick, rec: &rec, prev: &prev, sight: &sight };
            for topic in &mut self.topics {
                topic.body(&self.run, &at, &mut rays);
            }
        }
        for topic in &mut self.topics {
            topic.end(&self.run, game, &sight, &mut rays);
        }
        game.metric_rays += rays;
    }

    /// A bot's note: what the topics count of it (takes, stuck watches, plays ended for no way to the goal,
    /// grenades thrown). The note is read once here, into its [`NoteKind`].
    pub(crate) fn note(&mut self, note: &Note, game: &Game) {
        let kind = NoteKind::of(&note.text);
        if matches!(kind, NoteKind::Stuck)
            && let Some(t) = self.run.stuck_notes.get_mut(note.place)
        {
            t.push(note.time);
        }
        for topic in &mut self.topics {
            topic.note(&self.run, note, &kind, game);
        }
    }

    /// A fabricated bot's state for a test: the run's record of what it was doing, and what each topic reads.
    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        for topic in &mut self.topics {
            topic.inject(place, now);
        }
    }

    /// [`Self::step`] with fabricated bots: `nows[place]` (none: not a bot, and a place past the end is none).
    #[cfg(test)]
    fn step_with(&mut self, game: &mut Game, nows: &[Option<BotNow>]) {
        let mut doing = Vec::new();
        for place in 0..game.bodies.len() {
            let now = nows.get(place).and_then(Option::as_ref);
            self.inject(place, now);
            doing.push(now.map(BotNow::doing));
        }
        self.step(game, &doing);
    }

    /// A topic by its type, for a test that reaches into one.
    #[cfg(test)]
    fn topic<T: Topic>(&mut self) -> &mut T {
        self.topics
            .iter_mut()
            .find_map(|t| t.as_any_mut().downcast_mut::<T>())
            .expect("a topic of that type")
    }

    /// The counts, as the `owner-test` lines and the sums' `owner` object: each topic works out its own; what
    /// they share is the bots' minutes and the rounding ([`Scale`]).
    pub(crate) fn finish(&mut self, game: &Game) -> (String, serde_json::Value) {
        let Topics { core, parts } = self.finish_topics(game);
        let mut json = core;
        let mut text = Vec::new();
        for (_, part) in parts {
            json.extend(part.json);
            text.push(part.text);
        }
        (text.join("
  "), serde_json::Value::Object(json))
    }

    /// [`Self::finish`] before it joins the topics: what the run shares (`bot_min`, `ticks`) and each topic's
    /// [`Part`]s under their names (the golden counts keep one file a name).
    pub(super) fn finish_topics(&mut self, game: &Game) -> Topics {
        let ticks = self.run.recs.first().map_or(0, Vec::len);
        let bot_min = self.run.bot_min();
        let scale = Scale { tick_s: TICK_S as f64, bot_min };
        let mut parts = Vec::new();
        let mut shared = Shared::new();
        for topic in &mut self.topics {
            parts.extend(topic.parts(&self.run, game, &scale, &shared));
            shared.extend(topic.publish());
        }
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
    let mut tally = OwnerTally::new(teams.len());
    tally.set_scoreboard(String::new(), survarium::v026g::maps::mode::BATTERIES, None, None);
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
        tally.step_with(game, &nows);
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
