//! The match server's bots: stages 2 and 3 of the player bots
//! (`docs/notes/player_bots.md` §6), built to the design of
//! `docs/notes/player_bots_research.md` part B:
//!
//! ```text
//! view    -> what the match is: every body, sound, death, battery  (view.rs)
//! sense   -> what one bot sees and hears                           (sense.rs, cheats.rs)
//! know    -> what it and its team know, each fact with its source  (know.rs)
//! team    -> the commander hands out plays every ~0.5 s             (team.rs, signals.rs)
//! play    -> one play at a time, a small state machine              (plays/)
//! tactics -> spots and routes; hazards and spawn areas kept off     (tactics.rs, hazards.rs, path.rs)
//! motor   -> aim, move, fire, smooth as a player's hands           (motor.rs, reflex.rs, stuck.rs)
//! guns    -> each weapon's rounds, shots and reloads, from the game (guns.rs)
//! recoil  -> its kick, which moves the view, as the client moves it  (recoil.rs)
//! encode  -> the inputs a client would send                         (encode.rs)
//! ```
//!
//! Orders go down, reports come up. Two tracks per bot: the play (where to
//! be and why) and the combat reflex (target, aim, fire), which overrides
//! only the look and the trigger. Every number is the tuning's
//! (`tuning.rs`, `SRP_BOT_TUNING`); each bot's random numbers are its own
//! (`rng.rs`); every decision can be traced (`trace.rs`,
//! `SRP_BOT_TRACE_DIR`).
//!
//! [`Bots`] holds a match's bots and is all the match (`game/driven.rs`)
//! sees of them: entries, every player's inputs, the trusted worlds' word
//! on bodies and statistics, the map once its grid is built; and the
//! inputs the bots make.

pub mod aimpoint;
pub mod anomalies;
pub mod arc;
pub mod body;
pub mod cheats;
pub mod crossing;
pub mod curve;
pub mod encode;
pub mod escape;
pub mod foliage;
pub mod friends;
pub mod grenade_threat;
pub mod guns;
pub mod hazards;
#[cfg(feature = "jump-test")]
pub mod jump_test;
pub mod kin;
pub mod know;
pub mod live_grenades;
pub mod maps;
pub mod motor;
pub mod path;
pub mod plays;
pub mod pools;
pub mod recoil;
pub mod reflex;
pub mod rng;
pub mod route_end;
pub mod route_filter;
pub mod routes;
pub mod sense;
pub mod signals;
pub mod stock;
pub mod stuck;
pub mod tactics;
pub mod team;
pub mod threat;
pub mod trace;
pub mod trap_know;
pub mod trap_react;
pub mod traps;
pub mod tuning;
pub mod view;

mod commander;
mod drive;
mod facts;
mod search;
mod think;

use std::sync::Arc;

use mapdata::{NodeId, Vec3, World};
use survarium::v026g::loadouts::{BotRole, BotTier};
use survarium::v026g::match_msgs::PlayerInput;

use crate::v026g::drive::{half_value, key};
use body::Fix;
use cheats::CheatClock;
use encode::{Decision, Encoder};
use know::{DangerSpot, LookSpot, Loss, Memory, Percept, Source, TeamKnowledge, Theft};
use motor::{Hurt, JumpBy, Motor, Pose, RoutePress, Walked};
use path::Route;
use pools::Pools;
use plays::{BotFacts, Ctx, Go, Intent, Look, Pace, PlayKind, Running, Step};
use reflex::Reflex;
use route_end::RouteEnd;
use route_filter::RouteFilter;
use rng::Rng;
use sense::RayBudget;
use signals::{Duel, MatchSignals, Mods, Skill};
use stuck::{Recovery, Stuck, TeamSpots};
use tactics::Tactics;
use team::{Asking, Assignment};
use trace::Trace;
use tuning::Tuning;
use view::{BatteryState, Clock, MatchView, Place, PlaceView, Tracker, WorldFacts};
use commander::*;
use drive::*;
use plays::goals::goal_shut;
use search::*;

/// From a bot's entry to its first move, ms.
const START_AFTER_MS: u32 = 2_000;
/// An input a bot made.
#[derive(Clone, Debug)]
pub struct Made {
    pub place: usize,
    /// Its tick's time on the match clock.
    pub time: u32,
    pub input: PlayerInput,
}

/// Something worth a line in the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub place: usize,
    pub time: u32,
    pub text: String,
}

/// One bot.
pub struct Bot {
    place: Place,
    team: u8,
    rng: Rng,
    encoder: Encoder,
    motor: Motor,
    reflex: Reflex,
    memory: Memory,
    stuck: Stuck,
    /// The regions the team's held threat spots see ([`threat`]), told at
    /// the commander's ticks; its routes keep out of them where they can.
    threat: Arc<threat::Covered>,
    personality: usize,
    /// The skill it took at its last spawn.
    skill: f32,
    /// Its map side (the spawns' team it first came out of).
    side: Option<i32>,
    running: Option<Running>,
    assignment: Option<Assignment>,
    last_ran: Vec<(PlayKind, u32)>,
    intent: Intent,
    /// The route and the goal it was planned to.
    route: Option<(Route, Vec3)>,
    /// When the route was put in place, and how the last one ended with its
    /// age in ms (the bench's counts and the jump test's log, `route_end.rs`).
    #[cfg(any(test, feature = "jump-test"))]
    route_born: u32,
    #[cfg(any(test, feature = "jump-test"))]
    route_end: Option<(RouteEnd, u32)>,
    /// A goal no way was found to, and until when it is not tried again.
    no_way: Option<(Vec3, u32)>,
    /// Goals a search from its own floor proved no way to (over the floors
    /// it may walk on then), and until when the plays leave them
    /// ([`NO_WAY_KEEP_MS`]): a battery ringed by fire is not fetched again
    /// and again, standing.
    no_way_to: Vec<(Vec3, u32)>,
    /// The lip guard's watch over its own hold on this bot's keys (`drive::LipWatch`).
    lip_watch: drive::LipWatch,
    /// The bench's stood line (`real_bot_cost`): whether the reflex had the
    /// legs at its last motor tick (a fight, not a route missing), and the
    /// poll clock of that tick (a play taken after it has had no motor
    /// tick yet to plan its route).
    #[cfg(test)]
    pub(crate) bench_legs: bool,
    /// Which reflex play had the legs at that tick (the falls probe).
    #[cfg(test)]
    pub(crate) bench_legs_why: Option<reflex::LegsWhy>,
    /// The side step of the last motor tick: 0 none, 1 standing and taken, 2 woven into a walk and taken, 3 refused.
    #[cfg(test)]
    pub(crate) bench_stepped: u8,
    /// The lip guard of the last motor tick: 0 not near a lip, 1 keys safe, 2 changed, 3 none safe (braked), 4 planned drop, 5 jumping.
    #[cfg(test)]
    pub(crate) bench_guard: u8,
    #[cfg(test)]
    pub(crate) bench_guard_dbg: u8,
    /// How the route was walked at the last motor tick: 0 not, 1 facing the way, 2 aiming (the eight keys).
    #[cfg(test)]
    pub(crate) bench_walk: u8,
    /// The velocity and feet the last guard check was made from.
    #[cfg(test)]
    pub(crate) bench_guard_at: ([f32; 2], Vec3),
    /// The feet the bot reckoned for a tick (known_at ms), the last ten (the falls probe's reckoning error).
    #[cfg(test)]
    pub(crate) bench_est: std::collections::VecDeque<(u32, Vec3)>,
    /// The time of its last route search, for the bench's counts of asks (`search::asks`).
    #[cfg(test)]
    pub(crate) bench_asked: search::asks::Asked,
    /// Whether the last drive tick had it shut in its pocket (`search::asks::Ask::Episodes`).
    #[cfg(test)]
    pub(crate) bench_shut_in: bool,
    #[cfg(test)]
    pub(crate) bench_driven: u32,
    /// The bench's stand-in camper (`SRP_BENCH_CAMPER`): it holds its
    /// spot, and the reflex never takes its legs.
    #[cfg(test)]
    pub(crate) bench_camper: bool,
    /// The bench's zone stretches: this tick's motives ([`why`]), what its
    /// route is, and whether the search that went on over polls ended in a
    /// ring's walk out.
    #[cfg(test)]
    pub(crate) bench_why: u16,
    #[cfg(test)]
    bench_route: u16,
    #[cfg(test)]
    bench_found_ring: bool,
    /// Keys to press for one tick.
    press: u32,
    /// Keys held for a few ticks to get unstuck.
    unstick: Option<(u32, u32)>,
    /// Ticks left of a jump held with the trigger let go ([`JUMP_HOLD_MS`]),
    /// and the last tick the trigger was down.
    jump_hold: u32,
    fired_at: Option<u32>,
    /// Its hurts as the last trusted world showed them ([`Bots::hurt`]).
    hurt: Hurt,
    /// The last jump, until what followed it is traced.
    jump_watch: Option<JumpWatch>,
    /// Whether the match traces, and the trace lines made while driving
    /// (written at the bot's ticks).
    tracing: bool,
    traced: Vec<serde_json::Value>,
    /// Standing still until, after giving a spot up.
    pause_until: u32,
    next_think: u32,
    /// Sounds up to this time were heard already.
    heard_until: u32,
    entered: Option<u32>,
    last_trace: u32,
    partner: Option<Place>,
    /// Whether its partner has it for partner too (a pair); a team's odd
    /// bot has a partner one way, for its trades only.
    paired: bool,
    /// Medkits used this life (the tip's count, which the Heal play and Retreat go by while `HealTuning::on` is off).
    medkits_used: u8,
    /// The bear-trap habit (`traps.rs`), and the point of the trap it let go
    /// of since the last think.
    traps: traps::Habit,
    trap_placed: Option<(Vec3, u32)>,
    /// Its notice of the traps of others, and the counts of its passes
    /// (`trap_react.rs`).
    react: trap_react::React,
    /// The floors of its team's traps it keeps off (`trap_know.rs`), shared
    /// at each commander tick.
    trap_field: Arc<trap_know::TrapField>,
    /// The drugs of its kits, counted for the match ([`stock::Pantry`]).
    pantry: stock::Pantry,
    /// Its health pools as the last trusted world showed them
    /// ([`Bots::pools`]); none before the first.
    pools: Option<Pools>,
    /// The answer of its route search that went on over several polls
    /// ([`Pending`]): the goal, the route or none, and whether the route is
    /// a crossing (`crossing.rs`). Taken at its next tick, and used if it
    /// still heads there.
    found: Option<(Vec3, Option<Route>, bool)>,
    /// The goal its route crosses zones on all match to (`crossing.rs`):
    /// while it heads there, those zones' floors are not escaped from nor
    /// shut its route ahead.
    crossing: Option<Vec3>,
    /// A goal a crossing's search for a long plain route kept the plain one
    /// to, and until when such a search is not made again to a goal near it
    /// (`crossing::LONG_RETRY_MS`).
    long_kept: Option<(Vec3, u32)>,
    /// Its route searches dropped since it last noted them.
    dropped: path::Dropped,
    /// Its kits by index (the profile's, `survarium::v026g::loadouts::
    /// bot_kits` of its tier and role) and the rounds of weapon 1's ammo
    /// each has left for the match (its storage, `guns::Arms::reserve`):
    /// a spawn fills a life's rounds from it, and only the rounds fired
    /// are gone. Empty for a bot of no role.
    kits: Vec<(&'static guns::Arms, u32)>,
    /// The rounds this life started with (what its kit's storage gave).
    life_carried: u32,
    /// Its role, read from its gear (`Bots::new`'s `roles`).
    role: Option<BotRole>,
    /// Plays that failed at once at a target, and until when they are not
    /// taken there again (`plays::SET_ASIDE_MS`).
    set_aside: Vec<(PlayKind, plays::TargetRef, u32)>,
    /// What it remembers of giving way to a teammate on a spot
    /// ([`plays::goals::keep_apart`]).
    apart: plays::goals::Apart,
    /// Stands its takes of batteries gave up at lately
    /// (`plays::collect::FailedStand`): its next take stands elsewhere.
    failed_stands: Vec<plays::collect::FailedStand>,
    /// The kit of this life, and the one it picked for its next (a kit
    /// choice, 0xF3, takes effect at the spawn).
    kit: usize,
    kit_chosen: usize,
    /// Its state towards the live grenades it knows of, and what it did
    /// ([`grenade_threat`]).
    avoid: grenade_threat::Avoid,
    /// The resupply holds it finished this match (`Intent::resupplied`).
    resupplies: u32,
}

impl Bot {
    fn new(match_id: u32, place: Place, team: u8, role: Option<&str>, tuning: &Tuning) -> Self {
        let mut rng = Rng::for_bot(match_id, place);
        let motor = Motor::new(tuning.feel.clone(), &mut rng);
        // The personality of its role (play 14: the lobby gave it that
        // role's gear, `survarium::v026g::loadouts::BotRole`), else one
        // drawn by the shares.
        let mut pick = rng.unit();
        let personality = role
            .and_then(|role| tuning.personalities.iter().position(|p| p.name == role))
            .unwrap_or_else(|| {
                tuning
                    .personalities
                    .iter()
                    .position(|p| {
                        pick -= p.share;
                        pick < 0.0
                    })
                    .unwrap_or(0)
            });
        // What it carries: its role's gear (the lowest tier's until
        // `Bots::with_tiers` says another), else its personality's name as
        // a role (the tests' bots), else the fallback.
        let kits = Self::kits_of(role, BotTier::Low);
        let arms = role.and_then(guns::Arms::of_role).or_else(|| {
            tuning
                .personalities
                .get(personality)
                .and_then(|p| guns::Arms::of_role(&p.name))
        });
        let life_carried = kits
            .first()
            .map_or(arms.map_or(0, |arms| arms.primary.1), |&(arms, _)| {
                arms.primary.1
            });
        Self {
            place,
            team,
            rng,
            encoder: Encoder::default(),
            motor,
            reflex: Reflex::armed(arms),
            memory: Memory::default(),
            stuck: Stuck::default(),
            threat: Arc::default(),
            personality,
            skill: tuning.skill.of_tier(BotTier::default()).start,
            side: None,
            running: None,
            assignment: None,
            last_ran: Vec::new(),
            intent: Intent::stay(),
            route: None,
            #[cfg(any(test, feature = "jump-test"))]
            route_born: 0,
            #[cfg(any(test, feature = "jump-test"))]
            route_end: None,
            no_way: None,
            no_way_to: Vec::new(),
            lip_watch: drive::LipWatch::default(),
            #[cfg(test)]
            bench_legs: false,
            #[cfg(test)]
            bench_legs_why: None,
            #[cfg(test)]
            bench_stepped: 0,
            #[cfg(test)]
            bench_guard: 0,
            #[cfg(test)]
            bench_guard_dbg: 0,
            #[cfg(test)]
            bench_walk: 0,
            #[cfg(test)]
            bench_guard_at: ([0.0; 2], Vec3::ZERO),
            #[cfg(test)]
            bench_est: std::collections::VecDeque::new(),
            #[cfg(test)]
            bench_asked: Default::default(),
            #[cfg(test)]
            bench_shut_in: false,
            #[cfg(test)]
            bench_driven: 0,
            #[cfg(test)]
            bench_camper: false,
            #[cfg(test)]
            bench_why: 0,
            #[cfg(test)]
            bench_route: 0,
            #[cfg(test)]
            bench_found_ring: false,
            press: 0,
            unstick: None,
            jump_hold: 0,
            fired_at: None,
            hurt: Hurt::default(),
            jump_watch: None,
            tracing: false,
            traced: Vec::new(),
            pause_until: 0,
            next_think: 0,
            heard_until: 0,
            entered: None,
            last_trace: 0,
            partner: None,
            paired: false,
            medkits_used: 0,
            traps: traps::Habit::none(),
            trap_placed: None,
            react: trap_react::React::default(),
            trap_field: Arc::new(trap_know::TrapField::default()),
            pantry: stock::Pantry::standard(),
            pools: None,
            found: None,
            crossing: None,
            long_kept: None,
            dropped: path::Dropped::default(),
            kits,
            life_carried,
            role: role.and_then(|role| BotRole::ALL.into_iter().find(|each| each.name() == role)),
            set_aside: Vec::new(),
            apart: Default::default(),
            failed_stands: Vec::new(),
            kit: 0,
            kit_chosen: 0,
            avoid: grenade_threat::Avoid::default(),
            resupplies: 0,
        }
    }

    /// The kits of a bot of `role` (a personality's name) in `tier`, each
    /// with its whole storage; none for no role.
    fn kits_of(role: Option<&str>, tier: BotTier) -> Vec<(&'static guns::Arms, u32)> {
        role.and_then(|role| BotRole::ALL.into_iter().find(|each| each.name() == role))
            .map(|role| guns::Arms::kits_of(tier, role))
            .unwrap_or_default()
            .into_iter()
            .map(|arms| (arms, arms.reserve))
            .collect()
    }

    /// Its kits are those of `tier` (its role's): each full, the first in
    /// hand. For a bot made before its life begins.
    fn arm(&mut self, tier: BotTier) {
        let kits = Self::kits_of(self.role.map(BotRole::name), tier);
        if kits.is_empty() {
            return;
        }
        self.kits = kits;
        self.kit = 0;
        self.kit_chosen = 0;
        let (arms, reserve) = self.kits[0];
        self.life_carried = reserve.min(arms.primary.1);
        self.reflex = Reflex::armed_carrying(Some(arms), self.life_carried);
    }

    /// Its weapon 1's ammo and what a resupply could bring it to; none for
    /// a bot of no role.
    pub fn ammo(&self) -> guns::Ammo {
        let Some(&(arms, _)) = self.kits.get(self.kit) else {
            return guns::Ammo::default();
        };
        let have = self.reflex.left();
        guns::Ammo {
            have,
            whole: arms.primary.1,
            storage: self.kit_left(self.kit).saturating_sub(have),
        }
    }

    /// A resupply zone's top-up came: the rounds go from the kit's storage
    /// to the stack (what the kit has for the match is the same), and how
    /// many.
    fn top_up(&mut self) -> u32 {
        let take = self.ammo().missing();
        if take > 0 {
            self.reflex.top_up(take);
            self.life_carried += take;
        }
        take
    }

    /// The rounds kit `i` has left for the match: the present kit's
    /// storage less what this life fired.
    fn kit_left(&self, i: usize) -> u32 {
        let Some(&(_, left)) = self.kits.get(i) else {
            return 0;
        };
        if i == self.kit {
            left.saturating_sub(self.life_carried.saturating_sub(self.reflex.left()))
        } else {
            left
        }
    }

    /// The kit for its next life, when it changes (the owner's words,
    /// 2026-10-16 and 2026-10-23: another kit, with its own ammo, when the
    /// present one is spent). Only when the present kit has less than a
    /// magazine left for the match: the kit of the others with the most
    /// rounds; when every other has less than a magazine too, that one
    /// only if it has more rounds than the present one; none with no
    /// round. Ties go to the lower index. Chosen once: a choice is not
    /// made again while it stands, so the choice does not go back and
    /// forth. Its index, and the rounds left in the present kit and in
    /// it.
    fn pick_kit(&mut self) -> Option<(usize, u32, u32)> {
        let low = |bot: &Self, i: usize| bot.kit_left(i) < bot.kits[i].0.primary.0.magazine;
        if self.kits.len() < 2 || !low(self, self.kit) {
            return None;
        }
        let present = self.kit_left(self.kit);
        let mut best: Option<usize> = None;
        for i in (0..self.kits.len()).filter(|&i| i != self.kit && self.kit_left(i) > 0) {
            if best.is_none_or(|b| self.kit_left(i) > self.kit_left(b)) {
                best = Some(i);
            }
        }
        let best = best?;
        if low(self, best) && self.kit_left(best) <= present {
            return None;
        }
        if best == self.kit_chosen {
            return None;
        }
        self.kit_chosen = best;
        Some((best, present, self.kit_left(best)))
    }

    fn note(&self, notes: &mut Vec<Note>, time: u32, text: String) {
        notes.push(Note {
            place: self.place,
            time,
            text,
        });
    }

    fn personality<'t>(&self, tuning: &'t Tuning) -> &'t tuning::Personality {
        &tuning.personalities[self.personality.min(tuning.personalities.len() - 1)]
    }

    /// The commander gave it play `next`: the running play stops, and its
    /// cooldown counts from now. A switch to another target of the same
    /// play goes on with it, so the cooldown counts from the play's own end
    /// instead: stamped at the switch, it zeroed the new run's candidate at
    /// the next poll and the run was dropped within the second (Crossfire
    /// to Crossfire 88-93 % of the time in the owner's matches 4-7 of
    /// 2026-10-24, Cut to Cut 70-89 %).
    fn give_way(&mut self, next: PlayKind, now: u32) {
        if let Some(running) = self.running.take()
            && running.kind() != next
        {
            let kind = running.kind();
            self.last_ran.retain(|(k, _)| *k != kind);
            self.last_ran.push((kind, now));
        }
    }

    /// Its play ended: remembered, and the commander asked again.
    fn end_play(&mut self, now: u32, why: &str, notes: &mut Vec<Note>) {
        if let Some(running) = self.running.take() {
            let kind = running.kind();
            self.last_ran.retain(|(k, _)| *k != kind);
            self.last_ran.push((kind, now));
            self.note(notes, now, format!("ends {kind:?}: {why}"));
        }
        self.assignment = None;
        self.end_route(RouteEnd::EndPlay, now);
        self.intent = Intent::stay();
    }

    /// A new life: a fresh start with the team's present skill (it changes
    /// only at a spawn, play 23). The radio needs no key: it shortens the
    /// respawn by being carried (owner's word; being checked in the
    /// client's code).
    fn new_life(&mut self, skill: f32, tactics: Option<&Tactics>, feet: Vec3) {
        self.running = None;
        self.assignment = None;
        self.route = None;
        self.found = None;
        self.crossing = None;
        self.long_kept = None;
        self.intent = Intent::stay();
        self.stuck.reset();
        self.unstick = None;
        self.jump_hold = 0;
        self.fired_at = None;
        self.avoid.died();
        // The kit it spawned in keeps its storage less what it fired; the
        // kit it picked fills this life from its own (0x2FCB70: the slot's
        // rounds, at most what the storage has).
        let left = self.kit_left(self.kit);
        if let Some(kit) = self.kits.get_mut(self.kit) {
            kit.1 = left;
        }
        self.kit = self.kit_chosen.min(self.kits.len().saturating_sub(1));
        self.reflex = match self.kits.get(self.kit) {
            Some(&(arms, left)) => {
                self.life_carried = left.min(arms.primary.1);
                Reflex::armed_carrying(Some(arms), self.life_carried)
            }
            None => Reflex::armed(self.reflex.arms()),
        };
        self.memory = Memory::default();
        self.skill = skill;
        self.medkits_used = 0;
        self.traps.new_life();
        self.react.new_life();
        self.pantry.new_life(self.kit);
        if self.side.is_none() {
            self.side = tactics.and_then(|tactics| tactics.side_at(feet));
        }
    }
}

thread_local! {
    /// This thread's matches with bots ([`SearchMemory`]).
    static MATCHES_WITH_BOTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A match's bots.
pub struct Bots {
    level: u8,
    /// The mode's id (2 batteries, 3 timed batteries, 4 TDM).
    mode: u8,
    /// Whose the battery containers are, by id, as the server's game
    /// data has the map (`MapMode::container_teams`); without it, as the
    /// built map has them ([`Self::with_container_teams`]).
    container_teams: Option<[u8; 2]>,
    /// The static info's seed (`registry::static_id`): every anomaly's
    /// random stream starts from it (`anomalies.rs`).
    static_seed: u32,
    pub tuning: Arc<Tuning>,
    /// The real-map bench's one-sided runs: team 1 plays by this tuning
    /// instead (`real_bot_cost`, `SRP_BENCH_ONE_SIDE`).
    #[cfg(test)]
    side1_tuning: Option<Arc<Tuning>>,
    /// The real-map bench's human stand-in (`SRP_BENCH_HUMAN`, CG-12):
    /// the plays take this bot for a human, and its commander gives it
    /// Raid whenever Raid is a candidate.
    #[cfg(test)]
    pub bench_human: Option<Place>,
    /// The real-map bench's stand-in camper (`SRP_BENCH_CAMPER`,
    /// `campers.md` K1 §4): this bot holds one spot all match, and the
    /// spot once found (with how many route points it sees), or none.
    #[cfg(test)]
    pub bench_camper: Option<(Place, Option<tests::CamperSpot>)>,
    tracker: Tracker,
    /// TEST ONLY: the jump test for the real client (`jump_test.rs`).
    #[cfg(feature = "jump-test")]
    pub jump_test: Option<jump_test::JumpTest>,
    tactics: Option<Tactics>,
    /// The plays' kept answers about these tactics (`plays::memo`), made
    /// anew with them.
    memo: plays::memo::Memo,
    teams: [TeamState; 2],
    bots: Vec<Option<Bot>>,
    trace: Trace,
    /// Every place's weapon 1 kind, where known (play 11).
    weapons: Vec<Option<reflex::WeaponKind>>,
    /// The human routes of this map and mode (play 17), till the map is
    /// built and they are put on its floors.
    routes: Vec<routes::HumanRoute>,
    /// The map's bushes a sight line crosses (`foliage.rs`), till the map is built.
    foliage: foliage::Foliage,
    /// This poll's route searches ([`Searching`]).
    searching: Searching,
    /// How the teams' own traps went off ([`trap_know::OwnTrapCounts`]).
    own_traps: trap_know::OwnTrapCounts,
    /// Kit choices made since they were last taken
    /// ([`Bots::take_kit_choices`]): the place and the kit's index.
    kit_choices: Vec<(Place, u8)>,
    /// Last, so a search left under way is dropped before it.
    _search_memory: SearchMemory,
}

impl Bots {
    /// `teams[place]`, `driven[place]` (a bot's place), `roles[place]` (a
    /// bot's role by its gear); `mode` and `level` the map's.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        match_id: u32,
        mode: u8,
        level: u8,
        teams: &[u8],
        driven: &[bool],
        roles: &[Option<&str>],
        tuning: Arc<Tuning>,
        trace: Trace,
    ) -> Self {
        let team_state = |tuning: &Tuning| TeamState {
            knowledge: TeamKnowledge::default(),
            mods: Mods::default(),
            skill: Skill::new(&tuning.skill, BotTier::default()),
            cheats: CheatClock::default(),
            signals: MatchSignals::default(),
            next_commander: 0,
            no_way_to: Vec::new(),
            spots: TeamSpots::default(),
            trap_field: Arc::new(trap_know::TrapField::default()),
            trap_field_version: 0,
            world_traps: Arc::new(Vec::new()),
            threat: threat::ThreatSpots::default(),
            covered: None,
        };
        let bots = teams
            .iter()
            .zip(driven)
            .enumerate()
            .map(|(place, (&team, &driven))| {
                let role = roles.get(place).copied().flatten();
                driven.then(|| Bot::new(match_id, place, team, role, &tuning))
            })
            .collect();
        Self {
            level,
            mode,
            container_teams: None,
            static_seed: crate::v026g::registry::static_id(match_id),
            tracker: Tracker::new(mode, teams, driven, Vec::new()),
            #[cfg(feature = "jump-test")]
            jump_test: jump_test::JumpTest::from_env(match_id),
            tactics: None,
            memo: Default::default(),
            teams: [team_state(&tuning), team_state(&tuning)],
            bots,
            trace,
            tuning,
            #[cfg(test)]
            side1_tuning: None,
            #[cfg(test)]
            bench_human: None,
            #[cfg(test)]
            bench_camper: None,
            routes: Vec::new(),
            foliage: foliage::Foliage::none(),
            weapons: Vec::new(),
            searching: Searching {
                left: SEARCHES_PER_POLL,
                floors: 0,
                pending: None,
            },
            own_traps: trap_know::OwnTrapCounts::default(),
            kit_choices: Vec::new(),
            _search_memory: SearchMemory::hold(),
        }
    }

    /// The map's battery containers' teams by container id, as the
    /// server's game data has them (`MapMode::container_teams`): they win
    /// over the built map's, which may be of other data.
    pub fn with_container_teams(mut self, teams: Option<[u8; 2]>) -> Self {
        self.container_teams = teams;
        self
    }

    /// The bots' tier (the equipment band of the kits the lobby gave
    /// them): each team's skill starts at the tier's and keeps above its
    /// floor ([`tuning::SkillTuning::of_tier`]). Without it, the 4-5 tier.
    pub fn with_tier(mut self, tier: BotTier) -> Self {
        for state in &mut self.teams {
            state.skill = Skill::new(&self.tuning.skill, tier);
        }
        let start = self.tuning.skill.of_tier(tier).start;
        for bot in self.bots.iter_mut().flatten() {
            bot.skill = start;
        }
        self
    }

    /// The tier of each bot's gear (`tiers[place]`, read from its profile:
    /// `survarium::v026g::loadouts::bot_of`): its kits are its role's of
    /// that tier (`Bots::new` gives the lowest tier's), and the skill
    /// starts at the highest of them ([`Bots::with_tier`]; the lobby gives
    /// a match's bots one tier). Before play.
    pub fn with_tiers(mut self, tiers: &[Option<BotTier>]) -> Self {
        for (bot, tier) in self.bots.iter_mut().zip(tiers) {
            if let (Some(bot), Some(tier)) = (bot.as_mut(), tier) {
                bot.arm(*tier);
            }
        }
        match tiers.iter().flatten().max() {
            Some(&tier) => self.with_tier(tier),
            None => self,
        }
    }

    /// The kit choices the bots made since the last call: each place and
    /// the index of the kit for its next life, to be relayed as a
    /// player's 0xDE is (0xF3).
    pub fn take_kit_choices(&mut self) -> Vec<(Place, u8)> {
        std::mem::take(&mut self.kit_choices)
    }

    /// The speed level each place's profile gives, by place
    /// (`survarium::v026g::bot_skills::profile_movement_level`): the bodies
    /// move at it until a world shows a life's own.
    pub fn set_speed_levels(&mut self, levels: &[Option<f32>]) {
        self.tracker.set_prior_levels(levels);
    }

    /// Every place's pockets (a human's), for the throw keys
    /// ([`live_grenades`]).
    pub fn set_pockets(&mut self, pockets: Vec<Option<live_grenades::Pockets>>) {
        self.tracker.set_pockets(pockets);
    }

    /// Every place's weapon 1 kind (the matchup, play 11).
    pub fn set_weapons(&mut self, weapons: Vec<Option<reflex::WeaponKind>>) {
        self.weapons = weapons;
    }

    pub fn tracker(&self) -> &Tracker {
        &self.tracker
    }

    pub fn tactics(&self) -> Option<&Tactics> {
        self.tactics.as_ref()
    }

    /// The map's walk grid is built: the tactics (hazards, exits, the
    /// regions) and the containers. 0.15-0.3 s on the big maps on a fast
    /// machine: the match server builds the tactics on a thread of its own
    /// ([`Self::tactics_ready`]). Returns a line for the log.
    pub fn map_ready(&mut self, world: Arc<World>) -> String {
        let tactics = Tactics::new(world, self.tuning.hazard_margin);
        self.tactics_ready(tactics)
    }

    /// [`Self::map_ready`] with the tactics built ([`Tactics::new`] with
    /// this match's `tuning.hazard_margin`): the routes, the anomalies'
    /// clocks and the containers, a few ms.
    pub fn tactics_ready(&mut self, mut tactics: Tactics) -> String {
        tactics.set_routes(std::mem::take(&mut self.routes));
        tactics.escape_ceiling = self.tuning.escape_ceiling;
        tactics.foliage = Arc::new(std::mem::take(&mut self.foliage));
        // The anomalies that switch (level_03's electric pairs, level_02's
        // cloud) run on the match clock from the seed.
        let anomalies = anomalies::Anomalies::new(&tactics.world.map, self.static_seed);
        tactics.hazards.attach(anomalies);
        let teams = self.container_teams;
        let mut containers: Vec<(u8, Vec3)> = Vec::new();
        for container in &tactics.world.map.containers {
            let id = container.id.max(0) as usize;
            if containers.len() <= id {
                containers.resize(id + 1, (0, Vec3::ZERO));
            }
            let team = teams.map_or(container.team.clamp(0, 1) as u8, |teams| teams[id.min(1)]);
            containers[id] = (team, container.use_point());
        }
        self.tracker.set_containers(containers);
        let sides: std::collections::BTreeSet<i32> =
            tactics.world.map.spawns.iter().map(|s| s.team).collect();
        let summary = format!(
            "{} damage zones over {} floors; exits by side {:?}; human routes by side {:?}",
            tactics.hazards.zones.len(),
            tactics.hazards.marked(),
            sides
                .iter()
                .map(|side| (*side, tactics.exits(*side).len()))
                .collect::<Vec<_>>(),
            sides
                .iter()
                .map(|side| (*side, tactics.routes_of(*side).count()))
                .collect::<Vec<_>>()
        );
        self.tactics = Some(tactics);
        self.memo = Default::default();
        summary
    }

    /// The human routes of the match's map and mode (play 17), from
    /// `SRP_BOT_ROUTES`; before [`Self::map_ready`].
    pub fn set_routes(&mut self, table: &routes::RouteTable) {
        self.routes = table.for_map(self.level, self.mode);
    }

    /// The map's bushes (`bot-foliage.json`); before [`Self::map_ready`].
    pub fn set_foliage(&mut self, foliage: foliage::Foliage) {
        self.foliage = foliage;
    }

    pub fn has_map(&self) -> bool {
        self.tactics.is_some()
    }

    /// A place added to the match (a late joiner: never a bot).
    pub fn push_place(&mut self, team: u8) {
        self.bots.push(None);
        self.tracker.push_place(team);
    }

    pub fn is_bot(&self, place: Place) -> bool {
        self.bots.get(place).is_some_and(Option::is_some)
    }

    pub fn any(&self) -> bool {
        self.bots.iter().any(Option::is_some)
    }

    /// The place entered (`time` its 0xE9) or left.
    ///
    /// A bot that left gave its place up to a late joiner (`game/driven.rs`)
    /// and never comes back (its place is not used again): it is forgotten,
    /// so it makes no input, holds no play, is nobody's partner and is in
    /// no commander's count. Whoever leaves, a bot's memory of it and its
    /// team's callouts about it go: nobody is to be hunted where it stood.
    pub fn entered(&mut self, place: Place, entered: bool, time: u32) {
        self.tracker.entered(place, entered);
        if entered {
            if let Some(Some(bot)) = self.bots.get_mut(place) {
                bot.entered = Some(time);
            }
            return;
        }
        if let Some(slot) = self.bots.get_mut(place)
            && let Some(bot) = slot.take()
        {
            self.trace.line(serde_json::json!({
                "t": time, "bot": place, "team": bot.team, "left": true,
            }));
        }
        for bot in self.bots.iter_mut().flatten() {
            bot.memory.forget(place);
            if bot.partner == Some(place) {
                bot.partner = None;
            }
        }
        for team in &mut self.teams {
            team.knowledge.forget(place);
        }
    }

    pub fn has_entered(&self, place: Place) -> bool {
        self.bots
            .get(place)
            .and_then(Option::as_ref)
            .is_some_and(|bot| bot.entered.is_some())
    }

    /// Whether a bot waits to see its body in a world (just entered, or
    /// dead).
    pub fn need_world(&self) -> bool {
        self.bots.iter().flatten().any(|bot| {
            bot.entered.is_some()
                && self
                    .tracker
                    .body(bot.place)
                    .is_none_or(|body| body.estimate().is_none())
        })
    }

    /// A human's input as relayed.
    pub fn input(&mut self, place: Place, time: u32, input: &PlayerInput) {
        if self.is_bot(place) {
            return;
        }
        view::set_live_eye(self.tuning.aim.combat.live_eye);
        let r = input.rotation;
        let turn = (
            half_value(u16::from_le_bytes([r[0], r[1]])),
            half_value(u16::from_le_bytes([r[2], r[3]])),
        );
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        // A human's kick is not known.
        self.tracker.input(
            place,
            time,
            input.actions,
            turn,
            0.0,
            &self.tuning.kin,
            world.as_deref(),
        );
    }

    /// A trusted world's word on one place's hurts (with its body's,
    /// [`Self::fixed`]): what its jumps may do ([`Hurt::may_jump`]).
    pub fn hurt(&mut self, place: Place, hurt: Hurt) {
        if let Some(Some(bot)) = self.bots.get_mut(place) {
            bot.hurt = hurt;
        }
    }

    /// What the bot at `place` counts of its drugs, for the tests.
    #[cfg(test)]
    pub(crate) fn pantry_of(&self, place: Place) -> Option<&stock::Pantry> {
        self.bots.get(place)?.as_ref().map(|bot| &bot.pantry)
    }

    /// A trusted world's word on one place's health pools (with its body's,
    /// [`Self::fixed`]): the consumables' need.
    pub fn pools(&mut self, place: Place, pools: Pools) {
        if let Some(Some(bot)) = self.bots.get_mut(place) {
            bot.pools = Some(pools);
        }
    }

    /// A trusted world's word on what one place's kits hold of drugs
    /// ([`stock::readings`], from `players::Supplies`): the bot's own count
    /// follows it.
    pub fn stock(&mut self, place: Place, time: u32, readings: &stock::Readings) {
        if let Some(Some(bot)) = self.bots.get_mut(place) {
            bot.pantry.read(time, readings);
        }
    }

    /// The bodies of the jump test's last asked world as the reckoning's worlds: the game passes an asked world on to
    /// nobody, and with the test asking all the time the bots would get none.
    #[cfg(feature = "jump-test")]
    pub fn fix_from_burst(&mut self) -> Vec<Note> {
        let rows = self.jump_test.as_mut().map(jump_test::JumpTest::take_fresh).unwrap_or_default();
        let mut notes = Vec::new();
        for row in rows {
            let fix = Fix {
                time: row.time,
                alive: row.alive,
                inserted: row.inserted,
                feet: row.alive.then_some(row.feet),
                yaw: row.yaw,
                pitch: row.pitch,
                keys: row.keys,
                speed: Some(row.speed),
            };
            notes.extend(self.fixed(row.place, fix, None));
        }
        notes
    }

    /// A trusted world's word on one place's body.
    pub fn fixed(&mut self, place: Place, fix: Fix, health: Option<f32>) -> Vec<Note> {
        view::set_live_eye(self.tuning.aim.combat.live_eye);
        let mut notes = Vec::new();
        let (time, feet, keys) = (fix.time, fix.feet, fix.keys);
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        let new_life = self
            .tracker
            .fixed(place, fix, health, &self.tuning.kin, world.as_deref());
        let Some(Some(bot)) = self.bots.get_mut(place) else {
            return notes;
        };
        let team = usize::from(bot.team & 1);
        if let Some(feet) = feet {
            bot.jump_seen(time, feet);
        }
        if new_life && let Some(feet) = feet {
            bot.jump_followed(time, None, "a new life");
            bot.end_route(RouteEnd::NewLife, time);
            bot.new_life(self.teams[team].skill.skill, self.tactics.as_ref(), feet);
            bot.note(
                &mut notes,
                time,
                format!("is alive at {:?} (side {:?})", feet.to_array(), bot.side),
            );
            return notes;
        }
        let (Some(feet), Some((_, goal))) = (feet, bot.route.as_ref()) else {
            return notes;
        };
        let goal = *goal;
        let meant = self.tracker.body(place).and_then(|body| body.last_meant());
        // [pockets] A try at a pocket's exit runs up to its rim and jumps:
        // not stuck, and the try has its own clock (`escape::TRY_MS`). No
        // note, no spot for the team, no recovery fighting the try.
        let trying = bot
            .stuck
            .escape
            .as_ref()
            .is_some_and(escape::Escape::trying);
        if trying {
            bot.stuck.reset();
        } else if let Some((recovery, by_keys)) = bot.stuck.walking(time, feet, meant, keys) {
            bot.jump_followed(time, Some(feet), &format!("stuck: {recovery:?}"));
            bot.note(
                &mut notes,
                time,
                format!(
                    "is stuck at {:?}: {recovery:?}{}",
                    feet.to_array(),
                    if by_keys {
                        " (walk keys held, the body still)"
                    } else {
                        ""
                    }
                ),
            );
            let (side, other) = if bot.rng.unit() < 0.5 {
                (key::LEFT, key::RIGHT)
            } else {
                (key::RIGHT, key::LEFT)
            };
            // Each time again at the spot it backs off another way; not
            // into a zone that is on (none of the four: it only jumps).
            let escapes = bot.stuck.escapes(side, other);
            let yaw = self
                .tracker
                .body(place)
                .and_then(|body| body.estimate())
                .map(|at| at.yaw);
            let escape = match (self.tactics.as_ref(), yaw) {
                (Some(tactics), Some(yaw)) => escapes
                    .into_iter()
                    .find(|&keys| {
                        tactics.zone_free_step(feet, feet + unstick_step(keys, yaw), time)
                    })
                    .unwrap_or(0),
                _ => escapes[0],
            };
            let ticks = |rng: &mut Rng, s: (f32, f32)| (rng.range(s.0, s.1) / kin::TICK_S) as u32;
            // [pockets] No jump the game would refuse (a broken leg, a hit's
            // stagger): the keys alone.
            let jump = if bot.hurt.may_jump(time) {
                key::JUMP
            } else {
                0
            };
            // [unstick] A body pushing at a ledge it can jump up (more than
            // the client's step, under the standing jump's top) jumps up it
            // forward; the world must show it pushing forward, standing,
            // able to jump, and the step ahead off the zones' floors. Any
            // other stop backs off: a back-diagonal jump goes nowhere useful
            // (`Tactics::ledge_ahead`; the first stop at a spot gets no jump,
            // the second keeps the old one).
            let ledge = match (self.tactics.as_ref(), yaw) {
                (Some(tactics), Some(yaw))
                    if matches!(recovery, Recovery::BackOff | Recovery::Sidestep)
                        && keys & key::FORWARD != 0
                        && keys & (key::BACK | key::CROUCH) == 0
                        && jump != 0
                        && tactics.zone_free_step(
                            feet,
                            feet + unstick_step(key::FORWARD, yaw),
                            time,
                        ) =>
                {
                    tactics.ledge_ahead(feet, yaw)
                }
                _ => None,
            };
            if let Some(rise) = ledge {
                bot.note(
                    &mut notes,
                    time,
                    format!("unstick_ledge: forward jump up a ledge of {rise:.2} m"),
                );
            }
            let keys = stuck::unstick_keys(recovery, ledge.is_some(), escape, jump);
            match recovery {
                Recovery::BackOff => {
                    bot.unstick = Some((keys, ticks(&mut bot.rng, (0.5, 0.8))));
                    bot.route = None;
                }
                Recovery::Sidestep => {
                    bot.unstick = Some((keys, ticks(&mut bot.rng, (0.8, 1.3))));
                    bot.end_play(time, "stuck twice there", &mut notes);
                    self.teams[team].next_commander = time;
                }
                Recovery::GiveUp => {
                    bot.unstick = Some((keys, ticks(&mut bot.rng, (0.8, 1.3))));
                    bot.end_play(time, "stuck there again", &mut notes);
                    bot.pause_until = time + 3_000;
                }
            }
            return notes;
        }
        let error = self
            .tracker
            .body(place)
            .and_then(|body| body.last_error())
            .unwrap_or(0.0);
        if error > REPLAN_ERROR_M {
            bot.note(
                &mut notes,
                time,
                format!(
                    "was {error:.1} m off its reckoning; plans again to {:?}",
                    goal.to_array()
                ),
            );
            bot.end_route(RouteEnd::ReckoningError, time);
        }
        notes
    }

    /// A trusted world's objects section: the anomalies' states put right.
    /// Returns (cores read, cores missed).
    pub fn anomalies_from_world(&mut self, objects: &[u8], time: u32) -> (usize, usize) {
        self.tactics.as_mut().map_or((0, 0), |tactics| {
            let report = tactics.hazards.sync_from_world(objects, time);
            (report.synced.len(), report.missed.len())
        })
    }

    /// A trusted world's objects section: the bear traps the teams' bots
    /// promised confirmed, followed and dropped (`trap_know.rs`). Returns
    /// the traps the world shows (0 while no bot has put one down).
    pub fn traps_from_world(&mut self, objects: &[u8], time: u32) -> usize {
        let react = self.tuning.traps.react.on;
        let own = self.teams.iter().any(|team| !team.knowledge.traps.all().is_empty());
        if !own && !react {
            return 0;
        }
        let found = survarium::v026g::match_msgs::traps::find(objects, time);
        // The armed traps of the world, for the bots' notice (off: none).
        let armed = Arc::new(if react {
            trap_react::armed_of(&found)
        } else {
            Vec::new()
        });
        let mut went_off: Vec<(u8, Vec3)> = Vec::new();
        for (team, state) in self.teams.iter_mut().enumerate() {
            if own {
                let fired = state.knowledge.traps.sync(time, &found);
                went_off.extend(fired.into_iter().map(|at| (team as u8, at)));
                if let Some(tactics) = self.tactics.as_ref() {
                    state.refresh_traps(&tactics.world.grid);
                }
            }
            state.world_traps = armed.clone();
        }
        if !went_off.is_empty() {
            self.count_own_traps(time, &went_off);
        }
        found.len()
    }

    /// A team's trap went off: counted, and whether a body of the team with
    /// a broken leg (a catch's mark, PROVEN) stands within
    /// [`trap_know::CAUGHT_NEAR_M`] of it as the world shows (a proxy: the
    /// worlds are seconds apart, and a human ally's legs are not known).
    fn count_own_traps(&mut self, time: u32, went_off: &[(u8, Vec3)]) {
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        let clock = Clock {
            clock: time,
            time_left_s: 0.0,
            total_s: 0.0,
            playing: true,
        };
        let view = self
            .tracker
            .view(&clock, &self.tuning.kin, world.as_deref());
        for &(team, at) in went_off {
            self.own_traps.fired += 1;
            let caught = self.bots.iter().flatten().any(|bot| {
                bot.team == team
                    && bot.hurt.leg_broken
                    && view
                        .place(bot.place)
                        .is_some_and(|p| p.feet.distance_xz(at) <= trap_know::CAUGHT_NEAR_M)
            });
            self.own_traps.own_team_caught += u32::from(caught);
        }
    }

    /// How the teams' own traps went off (`trap_know.rs`).
    pub fn own_trap_counts(&self) -> trap_know::OwnTrapCounts {
        self.own_traps
    }

    /// The counts of the bots' notice of traps and passes by them, over all
    /// the bots (`trap_react.rs`).
    pub fn react_counts(&self) -> trap_react::ReactCounts {
        let mut sum = trap_react::ReactCounts::default();
        for bot in self.bots.iter().flatten() {
            sum.add(&bot.react.counts());
        }
        sum
    }

    /// The inputs due with the clock at `clock`.
    pub fn poll(&mut self, clock: Clock) -> (Vec<Made>, Vec<Note>) {
        view::set_live_eye(self.tuning.aim.combat.live_eye);
        let (mut made, mut notes) = (Vec::new(), Vec::new());
        let now = clock.clock;
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        self.tracker.held_sounds(now, self.tuning.sense.sprint_body_mps);
        self.tracker.launch_grenades(now, world.as_deref());
        if let Some(tactics) = self.tactics.as_mut() {
            tactics.hazards.advance(now);
        }
        let view = self
            .tracker
            .view(&clock, &self.tuning.kin, world.as_deref());
        if self.tactics.is_some() && clock.playing {
            for team in 0..2u8 {
                if now >= self.teams[usize::from(team)].next_commander {
                    self.commander(team, &view, &mut notes);
                }
            }
        }
        let mut rays = RayBudget(self.tuning.think.rays_per_poll);
        self.searching.left = SEARCHES_PER_POLL;
        self.searching.floors = self.tuning.think.search_floors_per_poll as usize;
        self.go_on_searching();
        for index in 0..self.bots.len() {
            if self.bots[index].is_none() {
                continue;
            }
            // Before play begins: no plays, no routes, only rest.
            if clock.playing {
                self.think(index, &view, &mut rays, &mut notes);
            }
            self.ticks(index, &clock, &view, &mut made);
            if let Some(bot) = self.bots[index].as_mut()
                && let Some(text) = bot.dropped.take()
            {
                bot.note(&mut notes, now, text);
            }
            // A kit picked before play begins would respawn the bot where
            // it stands at once (the games' patch 31), not at its next
            // life: the bot picks once play has begun, for its next life.
            if clock.playing
                && let Some(bot) = self.bots[index].as_mut()
                && bot.entered.is_some()
                && let Some((kit, left, other)) = bot.pick_kit()
            {
                bot.note(
                    &mut notes,
                    now,
                    format!(
                        "picks kit {} for its next life: kit {} has {left} rounds left, kit {} {other}",
                        kit + 1,
                        bot.kit + 1,
                        kit + 1
                    ),
                );
                self.kit_choices.push((bot.place, kit as u8));
            }
        }
        (made, notes)
    }

    /// The tuning team `team` plays by: the match's (the bench's one-sided
    /// runs give team 1 another).
    #[cfg_attr(not(test), allow(unused_variables))]
    fn tuning_of(&self, team: u8) -> Arc<Tuning> {
        #[cfg(test)]
        if let Some(tuning) = self.side1_tuning.as_ref().filter(|_| team == 1) {
            return tuning.clone();
        }
        self.tuning.clone()
    }

}

/// The bench's motives of a bot's tick (`Bot::bench_why`), a mask: what
/// moved it into a zone or kept it there (tests.rs `ZoneSpell`).
#[cfg(test)]
pub(crate) mod why {
    /// Its route: a plain one, a zone's way out (`Tactics::escape`), a
    /// crossing, a ring's walk out (`crossing.rs`); none.
    pub const PLAIN: u16 = 1;
    pub const ESCAPE: u16 = 2;
    pub const CROSS: u16 = 4;
    pub const RING: u16 = 8;
    pub const NONE: u16 = 16;
    /// A crossing walked on with its goal moved.
    pub const WALK_ON: u16 = 32;
    /// The reflex's legs (a fight).
    pub const LEGS: u16 = 64;
    /// No goal, out of a zone (`zone_way_out`).
    pub const ZONE_OUT: u16 = 128;
    /// The play's own leave to be in zones (`Intent::zone_ok`).
    pub const ZONE_OK: u16 = 256;
    /// Keys in a line, no route (`Intent::straight`).
    pub const STRAIGHT: u16 = 512;
    /// A pocket's way out (`escape.rs`).
    pub const POCKET: u16 = 1024;
}

/// The goal of the crossing (or a ring's walk out) a bot's motor walks now:
/// `Bot::crossing` while it has a route. The plays' `BotFacts::crossing`:
/// a carrier's line way out yields to it (`collect.rs`). A crossing left
/// with no route (the unstick's back-off and the other places that drop the
/// route, a search that found none) holds nothing back.
fn walks_crossing(bot: &Bot) -> Option<Vec3> {
    bot.crossing.filter(|_| bot.route.is_some())
}

impl Bot {
}

#[cfg(test)]
mod tests;
