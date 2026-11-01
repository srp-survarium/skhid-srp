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

pub mod aim_settle;
pub mod aimpoint;
pub mod anomalies;
pub mod apart;
pub mod arc;
pub mod artefact_know;
pub mod body;
pub mod breach;
pub mod cheats;
#[cfg(any(test, feature = "client-test"))]
pub mod client_tests;
pub mod concede;
pub mod contact;
pub mod control;
pub mod crossing;
pub mod curve;
pub mod doors;
pub mod encode;
pub mod escape;
pub mod field_source;
pub mod flight;
pub mod foliage;
pub mod friends;
pub mod grass;
pub mod grenade_threat;
pub mod guns;
pub mod hazards;
#[cfg(feature = "jump-test")]
pub mod jump_test;
pub mod kin;
pub mod know;
pub mod link_time;
pub mod live_grenades;
pub mod live_zones;
pub mod maps;
pub mod motor;
pub mod objective;
pub mod objective_actor;
pub mod path;
pub mod pitch;
pub mod plays;
pub mod pockets;
pub mod pools;
pub mod recoil;
pub mod reflex;
pub mod rest;
pub mod rng;
pub mod route_end;
pub mod route_filter;
pub mod routes;
pub mod sense;
pub mod shot_reach;
pub mod signals;
pub mod smoke;
pub mod stamina;
pub mod stock;
pub mod stuck;
pub mod switch_log;
pub mod tactics;
pub mod team;
pub mod threat;
pub mod trace;
pub mod trap_know;
pub mod trap_react;
pub mod traps;
pub mod tuning;
pub mod view;
pub mod worth;
pub mod zone_cost;
pub mod zone_search;

#[cfg(test)]
pub mod bench_work;
mod commander;
mod drive;
mod facts;
pub mod fairness;
mod features;
#[cfg(test)]
pub mod hold_trace;
mod layers;
pub mod pace;
#[cfg(test)]
pub mod pair_counts;
mod polls;
pub mod rays;
#[cfg(test)]
pub mod route_twins;
mod search;
mod think;

use std::sync::Arc;

use mapdata::NodeId;
use mapdata::Vec3;
use mapdata::World;
use survarium::v026g::loadouts::BotEquipment;
use survarium::v026g::loadouts::BotRole;
use survarium::v026g::loadouts::BotTier;
use survarium::v026g::loadouts::RusherKits;
use survarium::v026g::match_msgs::PlayerInput;
use survarium::v026g::match_msgs::traps::PlacedTrap;
use survarium::v026g::match_msgs::traps::TrapState;

use crate::v026g::drive::half_value;
use crate::v026g::drive::key;
use body::Fix;
use cheats::CheatClock;
use commander::*;
use control::Driver;
use drive::*;
use encode::Decision;
use encode::Encoder;
use features::BotFeatures;
use features::Life;
use features::MatchFeatures;
use features::TeamFeatures;
use know::DangerSpot;
use know::LookSpot;
use know::Loss;
use know::Memory;
use know::Percept;
use know::Source;
use know::TeamKnowledge;
use know::Theft;
use layers::MapLayers;
use motor::Hurt;
use motor::JumpBy;
use motor::Motor;
use motor::Pose;
use motor::RoutePress;
use motor::Walked;
use objective::Mode;
use path::Route;
use plays::BotFacts;
use plays::Ctx;
use plays::EscapeWhy;
use plays::Go;
use plays::Intent;
use plays::Look;
use plays::Pace;
use plays::PlayKind;
use plays::Running;
use plays::Step;
use plays::goals::goal_shut;
use pools::Pools;
use reflex::Reflex;
use rng::Rng;
use route_end::RouteEnd;
use route_filter::RouteFilter;
use search::*;
use sense::RayBudget;
use signals::Duel;
use signals::MatchSignals;
use signals::Mods;
use signals::Skill;
use stuck::Recovery;
use stuck::Stuck;
use tactics::Tactics;
use team::Asking;
use team::Assignment;
use trace::Trace;
use tuning::Tuning;
use view::BatteryState;
use view::Clock;
use view::MatchView;
use view::Place;
use view::PlaceView;
use view::Tracker;
use view::WorldFacts;

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
    /// The smoke clouds it knows of, and its watches of their edges (`smoke.rs`).
    clouds: smoke::KnownClouds,
    rng: Rng,
    encoder: Encoder,
    motor: Motor,
    reflex: Reflex,
    memory: Memory,
    /// Its target sights that ended, for the match-end log line.
    sight_counts: polls::SightCounts,
    /// By place, when a ray test of that enemy last came to an answer
    /// (seen or no part showing; one the rays ran out on did not): the
    /// longest unchecked is looked at first (`sense::look_order`).
    tested: Vec<u32>,
    stuck: Stuck,
    /// Its stamina bar as the bot reckons it from the keys it sent ([`stamina`]), and those keys.
    stamina: stamina::Reckoning,
    /// The sprint gate (`pace.rs`): its walk from the ring, and its last answer for the counts.
    pace: pace::PaceState,
    sent_keys: u32,
    /// Its rest, kept off a lip ([`rest`]).
    rest: rest::RestWatch,
    /// Whether it presses against something as it walks ([`contact`]), counted only.
    contact: contact::ContactWatch,
    /// Its last resort when nothing is left to try (`stuck::SelfKill`), and
    /// when it last used it (any life).
    self_kill: stuck::SelfKill,
    self_killed_at: Option<u32>,
    /// The regions the team's held threat spots see ([`threat`]), told at
    /// the commander's ticks; its routes keep out of them where they can.
    threat: Arc<threat::Covered>,
    /// The regions K1's sprint and the lane smoke read: [`Self::threat`]'s, or (a `SniperMemory` switch apart from the
    /// routes') those of the other held spots.
    sprint_threat: Arc<threat::Covered>,
    smoke_threat: Arc<threat::Covered>,
    /// [smoke] The team's held lanes, told at the commander's ticks while `smoke.cross_lane` is on.
    lanes: Arc<[threat::HeldLane]>,
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
    /// The enemy and the place its Crossfire began on, and when
    /// (`commit.crossfire_point`): the choke after the memory forgot him.
    held_choke: Option<(Place, Vec3, u32)>,
    /// The lip guard's watch over its own hold on this bot's keys (`drive::LipWatch`).
    lip_watch: drive::LipWatch,
    /// The bench's stood line (`real_bot_cost`): whether the reflex had the
    /// legs at its last motor tick (a fight, not a route missing), and the
    /// poll clock of that tick (a play taken after it has had no motor
    /// tick yet to plan its route).
    #[cfg(test)]
    pub(crate) bench_legs: bool,
    /// Which reflex play had the legs at that tick (the falls probe; a client test's `Mind`).
    #[cfg(any(test, feature = "client-test"))]
    pub(crate) bench_legs_why: Option<reflex::LegsWhy>,
    /// The goal the last motor tick walked to, after every override of the play's (the reflex's legs, the grenade
    /// avoid, the rest point); `None` when the tick walked to none (the stood count reads this, not the play's goal).
    #[cfg(test)]
    pub(crate) bench_go: Option<plays::Go>,
    /// The side step of the last motor tick: 0 none, 1 standing and taken, 2 woven into a walk and taken, 3 refused.
    #[cfg(test)]
    pub(crate) bench_stepped: u8,
    /// The step apart from a teammate of the last drive tick (`spacing.apart`): 0 no teammate inside, 1 taken, 2 refused (no clear ground that side), 3 held off (the reflex strafes or fires, a use is held, a side key is down).
    #[cfg(test)]
    pub(crate) bench_apart: u8,
    /// The line the bot walks (`spacing.lane`, A175): the route's own, or one beside it.
    pub(crate) lane: apart::Lane,
    /// What the lane did to the steering point in the last walk tick.
    #[cfg(test)]
    pub(crate) bench_lane: apart::LaneUse,
    /// What the queue (`spacing.queue`, A212) did on the last drive tick.
    #[cfg(test)]
    pub(crate) bench_queue: apart::QueueUse,
    /// The lip guard of the last motor tick: 0 not near a lip, 1 keys safe, 2 changed, 3 none safe (braked), 4 planned drop, 5 jumping.
    #[cfg(test)]
    pub(crate) bench_guard: u8,
    /// Why that verdict said the keys fall (`drive::GUARD_WHY`): 0 not, 1 the walk, 2 only a side's, plus 2 seen only about the feet.
    #[cfg(test)]
    pub(crate) bench_guard_why: u8,
    #[cfg(test)]
    pub(crate) bench_guard_dbg: u8,
    /// How the route was walked at the last motor tick: 0 not, 1 facing the way, 2 aiming (the eight keys).
    #[cfg(test)]
    pub(crate) bench_walk: u8,
    /// The keys the motor gave at the last drive tick, before the side step, the step apart and the lip guard (A198).
    #[cfg(test)]
    pub(crate) bench_motor_keys: u32,
    /// The velocity and feet the last guard check was made from.
    #[cfg(test)]
    pub(crate) bench_guard_at: ([f32; 2], Vec3),
    /// The feet the bot reckoned for a tick (known_at ms), the last ten (the falls probe's reckoning error).
    #[cfg(test)]
    pub(crate) bench_est: std::collections::VecDeque<(u32, Vec3)>,
    /// The reflex's mirror of the kick at each of its last ticks (`time`, [`Reflex::seen`]): what the stand-in's own kick is read against.
    #[cfg(test)]
    pub(crate) bench_mirror: std::collections::VecDeque<(u32, (f32, u8))>,
    /// The time of its last route search, for the bench's counts of asks (`search::asks`).
    #[cfg(test)]
    pub(crate) bench_asked: search::asks::Asked,
    /// Whether the last drive tick had it shut in its pocket (`search::asks::Ask::Episodes`).
    #[cfg(test)]
    pub(crate) bench_shut_in: bool,
    /// Why the drive last asked for a new route while it had a goal (the smoke topic's throw classes).
    #[cfg(test)]
    pub(crate) bench_replan: &'static str,
    /// The polls in a row it wanted a search and got none (`search::queue`).
    #[cfg(test)]
    pub(crate) bench_starved_polls: u32,
    /// Whether its route is a search's way so far (a partial route), and when it last walked one (`search::queue`).
    #[cfg(test)]
    pub(crate) bench_on_partial: bool,
    #[cfg(test)]
    pub(crate) bench_partial_at: Option<u32>,
    #[cfg(test)]
    pub(crate) bench_driven: u32,
    /// Who has the body: its play, the bench's stand-in camper or a client test's script (`control.rs`).
    pub(crate) control: Driver,
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
    /// Since when it stands for want of a world (`UnstickTuning::blind_stop_ms`); none: it walks as it will.
    blind_since: Option<u32>,
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
    /// Its route searches dropped since it last noted them.
    dropped: path::Dropped,
    /// Since when (ms) its walk tests its lines over a zone's floors, because it stands on floors the zone refuses
    /// (`Tactics::zone_exit_reach`); none off them. The note of each change is `zone_note`, said once.
    zone_walk: Option<u32>,
    zone_note: Option<String>,
    /// Its kits by index (the profile's, `survarium::v026g::loadouts::
    /// bot_kits` of its tier and role) and the rounds of weapon 1's ammo
    /// each has left for the match (its storage, `guns::Arms::reserve`):
    /// a spawn fills a life's rounds from it, and only the rounds fired
    /// are gone. Empty for a bot of no role.
    kits: Vec<(&'static guns::Arms, u32)>,
    /// The stamina rates of each of its kits, by the same index (`stamina::Rates::of_kit`); empty for a bot of no role
    /// (the light kit's then). A life's reckoning takes them with `think.stamina_rates` kit.
    kit_rates: Vec<stamina::Rates>,
    /// The rounds this life started with (what its kit's storage gave).
    life_carried: u32,
    /// Its role, read from its gear (`Bots::new`'s `roles`).
    role: Option<BotRole>,
    /// Rolls for a grenade refused because its kit held no frag (`CombatTuning::grenade_kit_cap`), the lives before this one's
    /// reflex (which counts its own).
    frag_rolls_refused: u32,
    /// The frags its lives carried and threw (the lives over; the present life's throws are the reflex's).
    frag_lives: live_grenades::FragLives,
    /// The kit of this life, and the one it picked for its next (a kit
    /// choice, 0xF3, takes effect at the spawn).
    kit: usize,
    kit_chosen: usize,
    /// The resupply holds it finished this match (`Intent::resupplied`).
    resupplies: u32,
    /// What each feature keeps of it ([`features`]).
    features: BotFeatures,
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
        let kits = Self::kits_of(role, BotEquipment::of(BotTier::Low, RusherKits::default()));
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
            clouds: smoke::KnownClouds::default(),
            rng,
            encoder: Encoder::default(),
            motor,
            reflex: Reflex::armed(arms),
            memory: Memory::default(),
            sight_counts: Default::default(),
            tested: Vec::new(),
            stuck: Stuck::default(),
            stamina: stamina::Reckoning::default(),
            pace: pace::PaceState::default(),
            sent_keys: 0,
            rest: rest::RestWatch::default(),
            contact: contact::ContactWatch::default(),
            self_kill: stuck::SelfKill::default(),
            self_killed_at: None,
            threat: Arc::default(),
            sprint_threat: Arc::default(),
            smoke_threat: Arc::default(),
            lanes: Arc::from(Vec::new()),
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
            held_choke: None,
            lip_watch: drive::LipWatch::default(),
            #[cfg(test)]
            bench_legs: false,
            #[cfg(any(test, feature = "client-test"))]
            bench_legs_why: None,
            #[cfg(test)]
            bench_go: None,
            #[cfg(test)]
            bench_stepped: 0,
            #[cfg(test)]
            bench_apart: 0,
            lane: apart::Lane::Centre,
            #[cfg(test)]
            bench_lane: apart::LaneUse::Idle,
            #[cfg(test)]
            bench_queue: apart::QueueUse::Idle,
            #[cfg(test)]
            bench_guard: 0,
            #[cfg(test)]
            bench_guard_why: 0,
            #[cfg(test)]
            bench_guard_dbg: 0,
            #[cfg(test)]
            bench_walk: 0,
            #[cfg(test)]
            bench_motor_keys: 0,
            #[cfg(test)]
            bench_guard_at: ([0.0; 2], Vec3::ZERO),
            #[cfg(test)]
            bench_est: std::collections::VecDeque::new(),
            #[cfg(test)]
            bench_mirror: std::collections::VecDeque::new(),
            #[cfg(test)]
            bench_asked: Default::default(),
            #[cfg(test)]
            bench_shut_in: false,
            #[cfg(test)]
            bench_replan: "none",
            #[cfg(test)]
            bench_starved_polls: 0,
            #[cfg(test)]
            bench_on_partial: false,
            #[cfg(test)]
            bench_partial_at: None,
            #[cfg(test)]
            bench_driven: 0,
            control: Driver::Own,
            #[cfg(test)]
            bench_why: 0,
            #[cfg(test)]
            bench_route: 0,
            #[cfg(test)]
            bench_found_ring: false,
            press: 0,
            unstick: None,
            blind_since: None,
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
            dropped: path::Dropped::default(),
            zone_walk: None,
            zone_note: None,
            kits,
            kit_rates: Vec::new(),
            life_carried,
            role: role.and_then(|role| BotRole::ALL.into_iter().find(|each| each.name() == role)),
            frag_rolls_refused: 0,
            frag_lives: live_grenades::FragLives::default(),
            kit: 0,


            kit_chosen: 0,
            resupplies: 0,
            features: BotFeatures::default(),
        }
    }

    /// The kits of a bot of `role` (a personality's name) with `equipment`,
    /// each with its whole storage; none for no role.
    fn kits_of(role: Option<&str>, equipment: BotEquipment) -> Vec<(&'static guns::Arms, u32)> {
        role.and_then(|role| BotRole::ALL.into_iter().find(|each| each.name() == role))
            .map(|role| guns::Arms::kits_of(equipment, role))
            .unwrap_or_default()
            .into_iter()
            .map(|arms| (arms, arms.reserve))
            .collect()
    }

    /// Its kits are those of `equipment` (its role's): each full, the first
    /// in hand. For a bot made before its life begins.
    fn arm(&mut self, equipment: BotEquipment) {
        // The bench's upgrades for one team only (`SRP_BENCH_UPGRADES_ONLY`): the team that has none gets its arms as if they were off.
        #[cfg(test)]
        let _kit = tests::team_kit_scope(self.team);
        let kits = Self::kits_of(self.role.map(BotRole::name), equipment);
        if kits.is_empty() {
            return;
        }
        self.kits = kits;
        self.kit_rates = self.role.map_or_else(Vec::new, |role| {
            let loadouts = survarium::v026g::loadouts::bot_kits(equipment, role);
            let modifiers = survarium::v026g::bot_skills::bot_modifiers(&loadouts);
            // The kits `kits_of` kept (those with arms), in its order.
            (0..role.kit_roles().len())
                .filter(|&k| guns::Arms::of_kit(equipment, role, k).is_some())
                .map(|k| stamina::Rates::of_kit(loadouts[k], &modifiers))
                .collect()
        });
        self.kit = 0;
        self.kit_chosen = 0;
        // The traps its kit carries (none in the old pockets): the habit is told at the spawn.
        self.features.traps.habit = traps::Habit::of_kit(self.role.and_then(|role| {
            survarium::v026g::loadouts::bot_trap(survarium::v026g::loadouts::bot_kit(equipment, role))
        }));
        let (arms, reserve) = self.kits[0];
        self.life_carried = reserve.min(arms.primary.1);
        self.features.weapon_mods.0.add(&self.reflex.mod_counts());
        self.frag_rolls_refused += self.reflex.refused_no_frag();
        self.frag_lives.thrown += u32::from(self.reflex.grenades_thrown());
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

    /// The kind of its weapon 1: the one the match gave its place
    /// (`weapons`, the kit it wears: an SMG rusher is an SMG), else its
    /// personality's name as a role (the tests' bots, whose place has none).
    /// With `weapon_class_by_data` on, the kind of the weapon it holds, by
    /// the game data's class (it follows a change of kit).
    fn weapon(
        &self,
        weapons: &[Option<reflex::WeaponKind>],
        tuning: &Tuning,
    ) -> Option<reflex::WeaponKind> {
        self.weapon_by(weapons, tuning, tuning.aim.combat.fire.weapon_class_by_data)
    }

    /// [`Self::weapon`] with the choice made: `by_data` or not.
    fn weapon_by(
        &self,
        weapons: &[Option<reflex::WeaponKind>],
        tuning: &Tuning,
        by_data: bool,
    ) -> Option<reflex::WeaponKind> {
        let held = if by_data {
            self.reflex
                .arms()
                .map(|_| reflex::WeaponKind::of_class(self.reflex.held_gun().class))
        } else {
            None
        };
        held.or_else(|| weapons.get(self.place).copied().flatten())
            .or_else(|| reflex::WeaponKind::of_role(&self.personality(tuning).name))
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
        #[cfg(test)]
        if let Some(a) = self.assignment.as_ref() {
            let dist = match self.intent.go {
                Go::To { at, .. } => Some(hold_trace::feet().distance_xz(at)),
                _ => None,
            };
            hold_trace::ended(a.kind, a.target, now.saturating_sub(a.since), why, dist);
        }
        if let Some(running) = self.running.take() {
            let kind = running.kind();
            self.last_ran.retain(|(k, _)| *k != kind);
            self.last_ran.push((kind, now));
            self.note(notes, now, format!("ends {kind:?}: {why}"));
        }
        self.assignment = None;
        self.held_choke = None;
        self.end_route(RouteEnd::EndPlay, now);
        self.intent = Intent::stay();
    }

    /// A new life: a fresh start with the team's present skill (it changes
    /// only at a spawn, play 23). The radio needs no key: it shortens the
    /// respawn by being carried (owner's word; being checked in the
    /// client's code).
    fn new_life(&mut self, skill: f32, tactics: Option<&Tactics>, feet: Vec3) {
        self.held_choke = None;
        self.running = None;
        self.assignment = None;
        self.route = None;
        self.intent = Intent::stay();
        self.stuck.reset();
        self.self_kill = stuck::SelfKill::default();
        self.unstick = None;
        // A death on zone floors ends the walk over them without a note of a way out.
        self.zone_walk = None;
        self.zone_note = None;
        self.jump_hold = 0;
        self.fired_at = None;
        // The kit it spawned in keeps its storage less what it fired; the
        // kit it picked fills this life from its own (0x2FCB70: the slot's
        // rounds, at most what the storage has).
        let left = self.kit_left(self.kit);
        if let Some(kit) = self.kits.get_mut(self.kit) {
            kit.1 = left;
        }
        self.kit = self.kit_chosen.min(self.kits.len().saturating_sub(1));
        self.features.weapon_mods.0.add(&self.reflex.mod_counts());
        self.frag_rolls_refused += self.reflex.refused_no_frag();
        self.frag_lives.thrown += u32::from(self.reflex.grenades_thrown());
        self.reflex = match self.kits.get(self.kit) {
            Some(&(arms, left)) => {
                self.life_carried = left.min(arms.primary.1);
                Reflex::armed_carrying(Some(arms), self.life_carried)
            }
            None => Reflex::armed(self.reflex.arms()),
        };
        self.memory = Memory::default();
        self.skill = skill;
        self.features.new_life(&Life { kit: self.kit });
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
    mode: Mode,
    /// Whose the battery containers are, by id, as the server's game
    /// data has the map (`MapMode::container_teams`); without it, as the
    /// built map has them ([`Self::with_container_teams`]).
    container_teams: Option<[u8; 2]>,
    /// The static info's seed (`registry::static_id`): every anomaly's
    /// random stream starts from it (`anomalies.rs`).
    static_seed: u32,
    /// The tuning each team plays by: `tunings.of(team)` for a decision, `tunings.match_wide()` for what exists once
    /// for both teams ([`tuning::TeamTunings`]).
    tunings: tuning::TeamTunings,
    /// The real-map bench's human stand-in (`SRP_BENCH_HUMAN`, CG-12):
    /// the plays take this bot for a human, and its commander gives it
    /// Raid whenever Raid is a candidate.
    #[cfg(test)]
    pub bench_human: Option<Place>,
    /// The real-map bench's model of Player vs Bots for the team's lanes of enemy humans (`SRP_BENCH_ENEMY_HUMAN_LANES`,
    /// **model**): every body of the other team counts as a human in [`know::HumanLanes`]; nothing else reads it.
    #[cfg(test)]
    pub(crate) bench_enemy_lanes: tests::EnemyLanes,
    /// Teams whose opening commander pass of the start countdown was made (`roam.fresh_claims`).
    opened: [bool; 2],
    /// The commander's pass in progress is the start countdown's (for its notes).
    opening_pass: bool,
    /// The real-map bench's stand-in camper (`SRP_BENCH_CAMPER`, `campers.md` K1 §4, `tests/camper_move.rs`): one bot holds a
    /// spot (or works an area), and plays by a tuning of its own.
    #[cfg(test)]
    pub(crate) bench_camper: Option<tests::BenchCamper>,
    tracker: Tracker,
    /// TEST ONLY: the jump test for the real client (`jump_test.rs`).
    #[cfg(feature = "jump-test")]
    pub jump_test: Option<jump_test::JumpTest>,
    /// TEST ONLY: the client-test runner of this match (`client_tests`), when `SRP_CLIENT_TEST` has steps for it.
    #[cfg(any(test, feature = "client-test"))]
    pub client_tests: Option<client_tests::Link>,
    polls: polls::PollStats,
    tactics: Option<Tactics>,
    /// The plays' kept answers about these tactics (`plays::memo`), made
    /// anew with them.
    memo: plays::memo::Memo,
    teams: [TeamState; 2],
    bots: Vec<Option<Bot>>,
    trace: Trace,
    /// The map's layers told before the map is built ([`layers`]).
    layers: MapLayers,
    /// What each feature keeps of the match ([`features`]).
    features: MatchFeatures,
    /// By place, the time of the last bullet hit its worlds showed, to see a new one ([`Self::bullet_hit`]).
    seen_hits: Vec<u32>,
    /// Last, so a search left under way is dropped before it.
    _search_memory: SearchMemory,
}

impl Bots {
    /// `teams[place]`, `driven[place]` (a bot's place), `roles[place]` (a
    /// bot's role by its gear); `mode` and `level` the map's. For tests that name the mode by its id.
    #[cfg(test)]
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
        let mode = Mode::parse(mode).expect("a test names a mode the bots play");
        Self::new_with(match_id, mode, level, teams, driven, roles, tuning::TeamTunings::new(tuning), trace)
    }

    /// [`Self::new`] with the tuning each team plays by (the bench's one-sided runs give team 1 another).
    #[allow(clippy::too_many_arguments)]
    pub fn new_with(
        match_id: u32,
        mode: Mode,
        level: u8,
        teams: &[u8],
        driven: &[bool],
        roles: &[Option<&str>],
        tunings: tuning::TeamTunings,
        trace: Trace,
    ) -> Self {
        let team_state = |tuning: &Tuning| TeamState {
            knowledge: TeamKnowledge::default(),
            mods: Mods::default(),
            skill: Skill::new(&tuning.skill, BotTier::default()),
            cheats: CheatClock::default(),
            signals: MatchSignals::default(),
            next_commander: 0,
            features: TeamFeatures::default(),
        };
        let bots = teams
            .iter()
            .zip(driven)
            .enumerate()
            .map(|(place, (&team, &driven))| {
                let role = roles.get(place).copied().flatten();
                driven.then(|| Bot::new(match_id, place, team, role, tunings.of(team)))
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
            #[cfg(feature = "client-test")]
            client_tests: client_tests::for_match(match_id, client_tests::group_of(level, mode.id(), teams)),
            #[cfg(all(test, not(feature = "client-test")))]
            client_tests: None,
            polls: Default::default(),
            tactics: None,
            memo: Default::default(),
            teams: [team_state(tunings.of(0)), team_state(tunings.of(1))],
            bots,
            trace,
            tunings,
            #[cfg(test)]
            bench_human: None,
            #[cfg(test)]
            bench_enemy_lanes: tests::EnemyLanes::Off,
            opened: [false; 2],
            opening_pass: false,
            #[cfg(test)]
            bench_camper: None,
            layers: MapLayers::default(),
            features: MatchFeatures::default(),
            seen_hits: Vec::new(),
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
        for (team, state) in (0u8..).zip(&mut self.teams) {
            state.skill = Skill::new(&self.tunings.of(team).skill, tier);
        }
        for bot in self.bots.iter_mut().flatten() {
            bot.skill = self.tunings.of(bot.team).skill.of_tier(tier).start;
        }
        self
    }

    /// The equipment of each bot's gear (`equipments[place]`, read from its
    /// profile: `survarium::v026g::loadouts::bot_equipment_of`): its kits
    /// are its role's of that tier and of the kit that tier's rusher wears
    /// (`Bots::new` gives the lowest tier's), and the skill starts at the
    /// highest tier among them ([`Bots::with_tier`]; the lobby gives a
    /// match's bots one tier). Before play.
    pub fn with_tiers(mut self, equipments: &[Option<BotEquipment>]) -> Self {
        for (bot, equipment) in self.bots.iter_mut().zip(equipments) {
            if let (Some(bot), Some(equipment)) = (bot.as_mut(), equipment) {
                bot.arm(*equipment);
            }
        }
        match equipments
            .iter()
            .flatten()
            .map(|equipment| equipment.tier)
            .max()
        {
            Some(tier) => self.with_tier(tier),
            None => self,
        }
    }

    /// The kit choices the bots made since the last call: each place and
    /// the index of the kit for its next life, to be relayed as a
    /// player's 0xDE is (0xF3).
    pub fn take_kit_choices(&mut self) -> Vec<(Place, u8)> {
        std::mem::take(&mut self.features.kit_choices)
    }

    /// The speed level each place's profile gives, by place
    /// (`survarium::v026g::bot_skills::profile_movement_level`): the bodies
    /// move at it until a world shows a life's own.
    pub fn set_speed_levels(&mut self, levels: &[Option<f32>]) {
        self.tracker.set_prior_levels(levels);
    }

    /// What every place's throw keys throw, for the live list
    /// ([`live_grenades`]).
    pub fn set_throws(&mut self, throws: Vec<live_grenades::Throws>) {
        self.tracker.set_throws(throws);
    }

    /// Every bot's drugs as its own profile has them ([`stock::Pantry::of_profile`]): the store the profile was built
    /// with is the prior of the count, until a world says.
    pub fn set_pantries(&mut self, pantries: Vec<stock::Pantry>) {
        for (place, pantry) in pantries.into_iter().enumerate() {
            if let Some(Some(bot)) = self.bots.get_mut(place) {
                bot.features.stock.pantry = pantry;
            }
        }
    }

    /// The pantry counts of both teams as the match-end log line (`the bots' pockets: ...`).
    pub fn pocket_report(&self) -> String {
        let [one, two] = self.pantry_counts();
        format!("team 0: {}; team 1: {}", one.line(), two.line())
    }

    /// What every bot's pantry has done, summed by team ([`stock::PantryCounts`]).
    pub fn pantry_counts(&self) -> [stock::PantryCounts; 2] {
        let mut sums = [stock::PantryCounts::default(); 2];
        for bot in self.bots.iter().flatten() {
            sums[usize::from(bot.team.min(1))] += bot.features.stock.pantry.counts();
        }
        sums
    }

    /// Every place's weapon 1's shot reach (H1c, [`shot_reach`]), where
    /// known: the sense hears a shot that far when `SenseTuning::shots` says.
    pub fn set_shot_reach(&mut self, reach: Vec<Option<f32>>) {
        self.tracker.set_shot_reach(reach);
    }

    /// What a bot wears against a zone's hits (F2, `zone_cost.rs`), from its kit; it is read when
    /// `HazardTuning::zone_pools` says.
    pub fn set_zone_armour(&mut self, place: Place, armour: zone_cost::ZoneArmour) {
        if let Some(Some(bot)) = self.bots.get_mut(place) {
            bot.features.crossing.armour = std::sync::Arc::new(armour);
        }
    }

    /// Every place's weapon 1 kind (the matchup, play 11).
    pub fn set_weapons(&mut self, weapons: Vec<Option<reflex::WeaponKind>>) {
        self.features.weapons = weapons;
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
        let tactics = Tactics::new(world, self.tunings.match_wide().hazard.margin_m.get());
        self.tactics_ready(tactics)
    }

    /// [`Self::map_ready`] with the tactics built ([`Tactics::new`] with
    /// this match's `tuning.hazard.margin_m`): the routes, the anomalies'
    /// clocks and the containers, a few ms.
    pub fn tactics_ready(&mut self, mut tactics: Tactics) -> String {
        std::mem::take(&mut self.layers).attach(&mut tactics);
        let tuning = self.tunings.match_wide();
        tactics.escape = tuning.hazard.escape;
        tactics.clear_step_sees_lip = tuning.aim.combat.clear_step_sees_lip;
        // The anomalies that switch (level_03's electric pairs, level_02's
        // cloud) run on the match clock from the seed.
        let anomalies = anomalies::Anomalies::new(&tactics.world.map, self.static_seed);
        tactics.hazards.attach(anomalies);
        self.features.artefacts = tuning
            .hazard
            .artefact_know
            .then(|| artefact_know::ArtefactKnow::of(&tactics.world.map));
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
        let summary = match &self.features.artefacts {
            Some(know) => format!(
                "{summary}; {} artefact spots ({} names unlinked)",
                know.spots().len(),
                know.unlinked()
            ),
            None => summary,
        };
        // What the bullet mesh is made of: a file before format 5 gives every face as solid (no material), so the cover
        // tests are the old ones; a v5 file has the armor of the face.
        let census = tactics.world.sight.armor_census();
        let summary = format!(
            "{summary}; bullet mesh {} faces: {} with armor, {} free, {} without a material (solid){}",
            census.total,
            census.armored,
            census.free,
            census.unknown,
            if census.armored == 0 && census.total > 0 { " [no materials: cover is solid, load format 5 files]" } else { "" }
        );
        self.tactics = Some(tactics);
        self.memo = Default::default();
        summary
    }

    /// The human routes of the match's map and mode (play 17), from
    /// `SRP_BOT_ROUTES`; before [`Self::map_ready`].
    pub fn set_routes(&mut self, table: &routes::RouteTable) {
        self.layers.routes = table.for_map(self.level, self.mode.id());
        self.layers.perches = table.perches_for(self.level);
        self.layers.perch_share = table.perch_share(self.level, self.mode.id());
    }

    /// The map's bushes (`bot-foliage.json`); before [`Self::map_ready`].
    pub fn set_foliage(&mut self, foliage: foliage::Foliage) {
        self.layers.foliage = foliage;
    }

    /// The map's grass (`bot-grass.json`); before [`Self::map_ready`].
    pub fn set_grass(&mut self, grass: grass::Grass) {
        self.layers.grass = grass;
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

    /// A human's input as relayed, which reached the server `late_ms` after
    /// its tick.
    pub fn input(&mut self, place: Place, time: u32, late_ms: u32, input: &PlayerInput) {
        if self.is_bot(place) {
            return;
        }
        view::set_live_eye(self.tunings.match_wide().aim.combat.live_eye);
        view::set_use_ray_m(self.tunings.match_wide().sense.use_ray_m);
        self.tracker.lateness(place, late_ms);
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
            &self.tunings.match_wide().kin,
            world.as_deref(),
        );
    }

    /// A trusted world's word on one place's hurts (with its body's,
    /// [`Self::fixed`]): what its jumps may do ([`Hurt::may_jump`]).
    pub fn hurt(&mut self, place: Place, hurt: Hurt) {
        if let Some(Some(bot)) = self.bots.get_mut(place) {
            bot.hurt = hurt;
            // [A217] A hit after a drug's press stops it (patch 84): the pantry keeps the first one.
            if let Some(hit) = hurt.last_hit_ms {
                bot.features.stock.pantry.hit(hit);
            }
        }
    }

    /// What the bot at `place` counts of its drugs, for the tests.
    #[cfg(test)]
    pub(crate) fn pantry_of(&self, place: Place) -> Option<&stock::Pantry> {
        self.bots.get(place)?.as_ref().map(|bot| &bot.features.stock.pantry)
    }

    /// A trusted world's word on one place's health pools (with its body's,
    /// [`Self::fixed`]): the consumables' need.
    pub fn pools(&mut self, place: Place, pools: Pools) {
        if let Some(Some(bot)) = self.bots.get_mut(place) {
            bot.features.stock.pools = Some(pools);
        }
        // A client test reads the human's pools (the owner's): the bots have their own.
        #[cfg(any(test, feature = "client-test"))]
        if !self.is_bot(place)
            && let Some(link) = self.client_tests.as_mut()
        {
            link.note_pools(pools);
        }
    }

    /// A trusted world's word on one place's last bullet hit, made at the world's time `hit_ms` (`m_last_hit_time_ms`;
    /// 0: none). A new one on an enemy of a team is a hit its bot saw land if the bot fired at him within that team's
    /// `worth.credit_ms` before it: a hit marker and a puff of blood, not a read of his pools (a zone, a fall or
    /// bleeding is no bullet; a teammate human's bullet is not its bot's). The weapon's nominal damage is the share
    /// ([`worth::Wounds::landed`]); the bot that fired last before the hit gets it.
    pub fn bullet_hit(&mut self, victim: Place, hit_ms: u32) {
        if self.seen_hits.len() <= victim {
            self.seen_hits.resize(victim + 1, 0);
        }
        if hit_ms == 0 || hit_ms <= self.seen_hits[victim] {
            return;
        }
        self.seen_hits[victim] = hit_ms;
        let life = self
            .tracker
            .body(victim)
            .and_then(|body| body.fix())
            .map_or(0, |fix| fix.inserted);
        // Per enemy team: the bot that fired at him last, within its tuning's window, and the weapon's nominal damage.
        let mut best: [Option<(u32, Place, f32)>; 2] = [None, None];
        for bot in self.bots.iter().flatten() {
            let team = usize::from(bot.team & 1);
            let window = self.tunings.of(bot.team & 1).aim.combat.worth.credit_ms;
            if let Some((at, share)) = bot.reflex.shot_landed(victim, hit_ms, window)
                && best[team].is_none_or(|(last, ..)| at > last)
            {
                best[team] = Some((at, bot.place, share));
            }
        }
        for (team, shot) in self.teams.iter_mut().zip(best) {
            if let Some((_, by, share)) = shot {
                team.knowledge.wounds.landed(victim, life, share, by, hit_ms);
            }
        }
    }

    /// A trusted world's word on what one place's kits hold of drugs
    /// ([`stock::readings`], from `players::Supplies`): the bot's own count
    /// follows it.
    pub fn stock(&mut self, place: Place, time: u32, readings: &stock::Readings) {
        if let Some(Some(bot)) = self.bots.get_mut(place) {
            bot.features.stock.pantry.read(time, readings);
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
        view::set_live_eye(self.tunings.match_wide().aim.combat.live_eye);
        view::set_use_ray_m(self.tunings.match_wide().sense.use_ray_m);
        let mut notes = Vec::new();
        let (time, feet, keys) = (fix.time, fix.feet, fix.keys);
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        let new_life = self
            .tracker
            .fixed(place, fix, health, &self.tunings.match_wide().kin, world.as_deref());
        let Some(Some(bot)) = self.bots.get_mut(place) else {
            return notes;
        };
        let team = usize::from(bot.team & 1);
        if let Some(feet) = feet {
            bot.jump_seen(time, feet);
            if new_life {
                bot.stuck.forget_heights();
            }
            bot.stuck.track_fall(time, feet, self.tunings.of(bot.team).think.fall_lock);
        }
        // A world ends a blind stop (`UnstickTuning::blind_stop_ms`); its reckoning is put right by the world itself.
        if bot.blind_since.is_some() && self.tracker.body(place).and_then(|body| body.fix()).is_some_and(|last| last.time == time) {
            bot.blind_since = None;
            #[cfg(test)]
            {
                stuck::blind::count(1);
                if self.tracker.body(place).and_then(|body| body.last_error()).is_some_and(|error| error < stuck::BLIND_NEEDLESS_M) {
                    stuck::blind::count(2);
                }
            }
        }
        if new_life && let Some(feet) = feet {
            bot.jump_followed(time, None, "a new life");
            bot.stamina.spawn(time);
            bot.sent_keys = 0;
            bot.end_route(RouteEnd::NewLife, time);
            bot.new_life(self.teams[team].skill.skill, self.tactics.as_ref(), feet);
            // The bar's rates of the kit this life is in (`think.stamina_rates`); the light kit's by default.
            if self.tunings.of(bot.team).think.stamina_rates == tuning::StaminaRates::Kit {
                bot.stamina.set_rates(bot.kit_rates.get(bot.kit).copied().unwrap_or(stamina::Rates::LIGHT));
            }
            // The sprint's gate and this life's bar reserve (`pace.rs`).
            bot.pace.new_life();
            // Drawn from a stream of its own (the spawn's time and the place), so the bot's own random numbers, which every decision shares, are the same with or without the reserve.
            let mut draw = Rng::new(u64::from(time).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ place as u64);
            bot.motor.reserve_for_life(self.tunings.of(bot.team).pace.reserve.as_ref(), &mut draw);
            // A life begins: the frags its kit in use holds (the tracker refilled it with the world that began the life).
            bot.frag_lives.spawn(self.tracker.frags_left(place));
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
        // The time since the world before: a long one is noted with the notes that follow it, counted, and (tuned)
        // ends the stuck check, as the bot was not shown the body for that long.
        let gap = self.tracker.body(place).and_then(|body| body.gap_of(time));
        #[cfg(test)]
        let after_gap = gap.is_some_and(|gap| gap >= stuck::FIX_GAP_MS);
        #[cfg(test)]
        if after_gap {
            stuck::fix_gaps::count(0);
        }
        let ends_check = self
            .tunings
            .of(bot.team)
            .unstick
            .fix_gap_ms
            .is_some_and(|ms| gap.is_some_and(|gap| gap > ms));
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
        } else if ends_check {
            #[cfg(test)]
            stuck::fix_gaps::count(3);
            bot.stuck.reset();
        } else if bot.control.reacts()
            && let Some((recovery, by_keys)) = bot.stuck.walking_in(time, feet, meant, keys, &self.tunings.of(bot.team).unstick)
        {
            bot.jump_followed(time, Some(feet), &format!("stuck: {recovery:?}"));
            #[cfg(test)]
            bot.stuck.watch_drop(time, feet, true);
            #[cfg(test)]
            if after_gap {
                stuck::fix_gaps::count(1);
            }
            let meant_text = stuck::meant_note(by_keys, bot.stuck.meant_checked());
            let beside = self.tracker.neighbour_of(place, feet);
            let route_text = stuck::route_note(
                bot.route.as_ref().map(|(route, _)| route),
                feet,
                self.tracker
                    .body(place)
                    .and_then(|body| body.estimate())
                    .map(|at| at.yaw),
                bot.stuck.avoided(time).count(),
            );
            #[cfg(test)]
            if let Some(b) = beside.filter(|b| b.distance_m <= stuck::BESIDE_M) {
                stuck::fix_gaps::count(if b.enemy { 4 } else { 5 });
            }
            bot.note(
                &mut notes,
                time,
                format!(
                    "is stuck at {:?}: {recovery:?}{}{}{}{}{}",
                    feet.to_array(),
                    if by_keys {
                        " (walk keys held, the body still)"
                    } else {
                        ""
                    },
                    meant_text,
                    stuck::gap_note(gap),
                    stuck::beside_note(beside),
                    route_text
                ),
            );
            // [unstick] Held at the top of a drop its route goes down (A199): the drops off where it stands are shut to its
            // searches, so the next one takes another way down, or none is left (`Stuck::hopeless`).
            if let Some(shut) = self.tunings.of(bot.team).unstick.drop_shut
                && let Some(tactics) = self.tactics.as_ref()
                && bot.route.as_ref().is_some_and(|(route, _)| route.heads_down_from(feet))
            {
                let landings = stuck::drops_off(&tactics.world.grid, feet, shut.near_m.get());
                if !landings.is_empty() {
                    bot.stuck.failed_drop(time, feet, &landings, shut.kept_ms);
                    bot.note(
                        &mut notes,
                        time,
                        format!(
                            "shuts {} drops off {:?} to its searches for {} s: held at their top",
                            landings.len(),
                            feet.to_array(),
                            shut.kept_ms / 1_000
                        ),
                    );
                }
            }
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
                // The bench's: an unstick jump, and whether its bot walked a search's way so far lately.
                #[cfg(test)]
                {
                    search::queue::add(bot.team, search::queue::UNSTICKS, 1);
                    if bot.bench_partial_at.is_some_and(|at| time.saturating_sub(at) <= search::queue::PARTIAL_RECENT_MS) {
                        search::queue::add(bot.team, search::queue::UNSTICKS_PARTIAL, 1);
                    }
                }
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
        #[cfg(test)]
        bot.stuck.watch_drop(time, feet, false);
        // The progress watch (`UnstickTuning::progress_ms`): a body that walks without getting further along its route.
        if bot.control.reacts()
            && let Some(window_ms) = self.tunings.of(bot.team).unstick.progress_ms
            && let Some((route, goal)) = bot.route.as_ref()
        {
            let length = route.length();
            let (s, off) = route.clone().progress(feet);
            if off < stuck::PROGRESS_OFF_M {
                // On the floor the route walks: the spot kept clear of is compared with the floor heights of a route (`Stuck::clear`).
                let leg = route.floor_point((s + stuck::PROGRESS_LEG_M).min(length));
                if bot.stuck.no_progress(time, s, keys, *goal, length, window_ms, leg) {
                    bot.note(
                        &mut notes,
                        time,
                        format!("makes no progress: {s:.1} of {length:.1} m along its route in {window_ms} ms; keeps clear of {:?} and plans again", leg.to_array()),
                    );
                    bot.end_route(RouteEnd::NoProgress, time);
                    return notes;
                }
            }
        }
        let error = self
            .tracker
            .body(place)
            .and_then(|body| body.last_error())
            .unwrap_or(0.0);
        if error > REPLAN_ERROR_M {
            #[cfg(test)]
            if after_gap {
                stuck::fix_gaps::count(2);
            }
            bot.note(
                &mut notes,
                time,
                format!(
                    "was {error:.1} m off its reckoning; plans again to {:?}{}",
                    goal.to_array(),
                    stuck::gap_note(gap)
                ),
            );
            bot.end_route(RouteEnd::ReckoningError, time);
        }
        notes
    }

    /// The note of `index`'s walk over a zone's floors, if its tick began or ended one (said once, at the change).
    pub(crate) fn zone_walk_note(&mut self, index: usize, now: u32, notes: &mut Vec<Note>) {
        if let Some(bot) = self.bots[index].as_mut()
            && let Some(text) = bot.zone_note.take()
        {
            bot.note(notes, now, text);
        }
    }

    /// A trusted world's objects section: the anomalies' states put right.
    /// Returns (cores read, cores missed).
    pub fn anomalies_from_world(&mut self, objects: &[u8], time: u32) -> (usize, usize) {
        // What the simulation said of the cores' artefacts, to count against the world
        // (`artefact_know`; none while it is off).
        let predicted = self.features.artefacts.as_ref().and_then(|_| {
            let anomalies = self.tactics.as_ref()?.hazards.anomalies()?;
            Some(artefact_know::ArtefactKnow::predicted(anomalies))
        });
        let mut synced: Vec<usize> = Vec::new();
        let read = self.tactics.as_mut().map_or((0, 0), |tactics| {
            let report = tactics.hazards.sync_from_world(objects, time);
            synced = report.synced.iter().map(|s| s.core).collect();
            (report.synced.len(), report.missed.len())
        });
        if let (Some(predicted), Some(know), Some(anomalies)) = (
            predicted,
            self.features.artefacts.as_mut(),
            self.tactics.as_ref().and_then(|t| t.hazards.anomalies()),
        ) {
            know.observe(&predicted, anomalies, &synced, time);
        }
        // The same world, as the live zones (the timed cores' states).
        self.fields_from_world(time);
        read
    }

    /// The artefact places of the map and how well the cores' artefacts are known
    /// (`tuning.hazard.artefact_know`; none while it is off or before the map is ready).
    pub fn artefact_know(&self) -> Option<&artefact_know::ArtefactKnow> {
        self.features.artefacts.as_ref()
    }

    /// The anomaly fields the cores show after a trusted world (`field_source::v1`) at match time `time`: the live
    /// zones of this match (`Tuning::live_zones`; off: nothing is read and nothing changes). Returns the fields the
    /// board holds.
    pub fn fields_from_world(&mut self, time: u32) -> usize {
        if !self.tunings.match_wide().hazard.live_zones {
            return 0;
        }
        self.tactics.as_mut().map_or(0, |tactics| {
            let fields = tactics
                .hazards
                .anomalies()
                .map(|anomalies| field_source::v1(anomalies, &tactics.world.map))
                .unwrap_or_default();
            tactics.hazards.set_fields(&fields, &tactics.world.grid, time);
            tactics.hazards.live.fields().len()
        })
    }

    /// The bench's source (`standin_fields`): the fields of a world as the generator lists them, with no world bytes.
    /// As [`Self::fields_from_world`], off with `Tuning::live_zones`.
    #[cfg(test)]
    pub fn fields_given(&mut self, fields: &[live_zones::WorldField], time: u32) -> usize {
        if !self.tunings.match_wide().hazard.live_zones {
            return 0;
        }
        self.tactics.as_mut().map_or(0, |tactics| {
            tactics.hazards.set_fields(fields, &tactics.world.grid, time);
            tactics.hazards.live.fields().len()
        })
    }

    /// The launched grenades of a trusted world's objects section (`match_msgs::grenades`): each smoke cloud whose
    /// grenade it shows is put where the grenade really is ([`view::Tracker::grenade_bodies`]). Returns how many it
    /// held.
    pub fn grenades_from_world(&mut self, objects: &[u8], time: u32) -> usize {
        let found = survarium::v026g::match_msgs::grenades::find(objects, time);
        if !found.is_empty() {
            let world = self.tactics.as_ref().map(|t| t.world.clone());
            self.tracker.grenade_bodies(&found, world.as_deref(), time);
        }
        found.len()
    }

    /// The bench's stand-in human put a trap down (`SRP_BENCH_HUMAN_TRAPS_KNOWN=on`): his team knows it from that tick, which is more
    /// than a live team has (it learns a human mate's trap once seen, A168); the same entry a bot's release makes, confirmed by the next
    /// world that shows it.
    #[cfg(test)]
    pub(crate) fn bench_team_knows_trap(&mut self, team: u8, by: Place, at: Vec3, released: u32) {
        let ring_m = self.tuning_of(team).traps.ring_m;
        let state = &mut self.teams[usize::from(team)];
        state.knowledge.traps.promise(by, at, released);
        if let Some(tactics) = self.tactics.as_ref() {
            state.refresh_traps(&tactics.world.grid, ring_m);
        }
    }

    /// A trusted world's objects section: the bear traps the teams' bots
    /// promised confirmed, followed and dropped (`trap_know.rs`). Returns
    /// the traps the world shows (0 while no bot has put one down).
    pub fn traps_from_world(&mut self, objects: &[u8], time: u32) -> usize {
        let react = self.tunings.match_wide().traps.react.on;
        let hear = self.tunings.match_wide().traps.react.hear;
        let own = self.teams.iter().any(|team| !team.knowledge.traps.all().is_empty());
        if !own && !react && !hear {
            return 0;
        }
        let found = survarium::v026g::match_msgs::traps::find(objects, time);
        if hear {
            self.hear_snaps(&found, time);
        }
        // The armed traps of the world, for the bots' notice (off: none).
        let armed = Arc::new(if react {
            trap_react::armed_of(&found)
        } else {
            Vec::new()
        });
        let mut went_off: Vec<(u8, Vec3)> = Vec::new();
        let rings = [self.tuning_of(0).traps.ring_m, self.tuning_of(1).traps.ring_m];
        for (team, state) in self.teams.iter_mut().enumerate() {
            if own {
                let fired = state.knowledge.traps.sync(time, &found);
                went_off.extend(fired.into_iter().map(|at| (team as u8, at)));
                if let Some(tactics) = self.tactics.as_ref() {
                    state.refresh_traps(&tactics.world.grid, rings[team & 1]);
                }
            }
            state.features.traps.world_traps = armed.clone();
        }
        if !went_off.is_empty() {
            self.count_own_traps(time, &went_off);
        }
        found.len()
    }

    /// A trap the last world showed armed that this one shows fired went off
    /// under the body nearest it: a sound of that body, heard like its steps
    /// (`SoundKind::Trap`, `traps.react.hear`). A disarmed trap makes none.
    fn hear_snaps(&mut self, found: &[PlacedTrap], time: u32) {
        let at = |trap: &PlacedTrap| Vec3::new(trap.position[0], trap.position[1], trap.position[2]);
        let near = self.tunings.match_wide().traps.react.snap_near_m;
        for fired in found.iter().filter(|trap| trap.state == TrapState::Fired) {
            let spot = at(fired);
            if !self.features.snaps.armed.iter().any(|armed| armed.distance_xz(spot) < trap_know::MATCH_M)
            {
                continue;
            }
            if let Some(victim) = self.tracker.nearest_body(spot, near) {
                self.tracker.snap(victim, spot, time);
                self.features.snaps.heard += 1;
            }
        }
        self.features.snaps.armed = found
            .iter()
            .filter(|trap| trap.state == TrapState::Armed)
            .map(at)
            .collect();
    }

    /// Snaps given to the bots' ears, for the bench.
    pub fn snaps_heard(&self) -> u32 {
        self.features.snaps.heard
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
            .view(&clock, &self.tunings.match_wide().kin, world.as_deref());
        for &(team, at) in went_off {
            self.features.own_traps.fired += 1;
            let caught = self.bots.iter().flatten().any(|bot| {
                bot.team == team
                    && bot.hurt.leg_broken
                    && view
                        .place(bot.place)
                        .is_some_and(|p| p.feet.distance_xz(at) <= trap_know::CAUGHT_NEAR_M)
            });
            self.features.own_traps.own_team_caught += u32::from(caught);
            #[cfg(test)]
            for bot in self.bots.iter().flatten().filter(|bot| bot.team == team && bot.hurt.leg_broken) {
                if view
                    .place(bot.place)
                    .is_some_and(|p| p.feet.distance_xz(at) <= trap_know::CAUGHT_NEAR_M)
                    && bot.features.traps.near.is_some_and(|(trap, _, _)| trap.distance_xz(at) < 1.0)
                {
                    let placer = self.teams[usize::from(team)]
                        .knowledge
                        .traps
                        .all()
                        .iter()
                        .any(|known| known.at.distance_xz(at) < 1.0 && known.by == bot.place);
                    traps::passes::caught(bot, at, placer, view.place(bot.place).map_or(at, |p| p.feet));
                }
            }
        }
    }

    /// How the teams' own traps went off (`trap_know.rs`).
    pub fn own_trap_counts(&self) -> trap_know::OwnTrapCounts {
        self.features.own_traps
    }

    /// Whether the other team's bodies all count as humans for `team`'s lanes (the bench's model; never in the game).
    pub(crate) fn bench_enemy_human_lanes(&self, team: u8) -> bool {
        #[cfg(test)]
        {
            self.bench_enemy_lanes.applies_to(team)
        }
        #[cfg(not(test))]
        {
            let _ = team;
            false
        }
    }

    /// What the bots' habit did, over all the bots and lives: traps placed, lives ended while placing
    /// (`traps.rs`).
    pub fn place_counts(&self) -> traps::PlaceCounts {
        let mut sum = traps::PlaceCounts::default();
        for bot in self.bots.iter().flatten() {
            sum.add(&bot.features.traps.habit.counts());
        }
        sum
    }

    /// The counts of the bots' notice of traps and passes by them, over all
    /// the bots (`trap_react.rs`).
    pub fn react_counts(&self) -> trap_react::ReactCounts {
        let mut sum = trap_react::ReactCounts::default();
        for bot in self.bots.iter().flatten() {
            sum.add(&bot.features.traps.react.counts());
        }
        sum
    }

    /// What the game data's weapon class changes for the bots now holding a
    /// weapon ([`ClassCounts`]): the same count with the switch off or on.
    pub fn class_counts(&self) -> guns::ClassCounts {
        let mut counts = guns::ClassCounts::default();
        for bot in self.bots.iter().flatten() {
            let Some(arms) = bot.reflex.arms() else {
                continue;
            };
            let weapons = &self.features.weapons;
            let tuning = self.tunings.of(bot.team);
            let kind = |by_data| bot.weapon_by(weapons, tuning, by_data);
            counts.add(arms.primary.0, kind(false) != kind(true));
        }
        counts
    }

    /// What the weapon mods did to the bots' reloads, over all the bots and
    /// lives (`guns.rs`), and which bots now hold a modded weapon, by what is
    /// modded and whether past what a player can have.
    pub fn mod_counts(&self) -> (guns::ModCounts, guns::HeldMods) {
        let mut sum = guns::ModCounts::default();
        let mut held = guns::HeldMods::default();
        for bot in self.bots.iter().flatten() {
            sum.add(&bot.features.weapon_mods.0);
            sum.add(&bot.reflex.mod_counts());
            if let Some(arms) = bot.reflex.arms() {
                held.add(arms.primary.0);
            }
        }
        (sum, held)
    }

    /// The inputs due with the clock at `clock`.
    /// What each bot's kit can throw now, told to its reflex: the frags it holds (`CombatTuning::grenade_kit_cap`) and the
    /// quick-use key of its smoke (`smoke`). At the start of every poll (`poll`, and the bench's `timed_poll`).
    pub(crate) fn tell_kit(&mut self) {
        for bot in self.bots.iter_mut().flatten() {
            bot.reflex.set_frags_left(self.tracker.frags_left(bot.place));
            bot.reflex.set_smoke_key(self.tracker.smoke_key(bot.place));
        }
    }

    pub fn poll(&mut self, clock: Clock) -> (Vec<Made>, Vec<Note>) {
        #[cfg(test)]
        bench_work::set(bench_work::Who::Shared);
        view::set_live_eye(self.tunings.match_wide().aim.combat.live_eye);
        view::set_use_ray_m(self.tunings.match_wide().sense.use_ray_m);
        let (mut made, mut notes) = (Vec::new(), Vec::new());
        let now = clock.clock;
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        self.tracker.held_sounds(now, self.tunings.match_wide().sense.sprint_body_mps);
        // The live list's notes are written while one of its switches is on (the match-wide tuning's: the list is one for all).
        let avoid = &self.tunings.match_wide().avoid;
        self.tracker.note_listings(avoid.bot_pockets || avoid.kit_in_use);
        self.tell_kit();
        self.tracker.launch_grenades(now, world.as_deref());
        if let Some(tactics) = self.tactics.as_mut() {
            tactics.hazards.advance(now);
        }
        let view = self
            .tracker
            .view(&clock, &self.tunings.match_wide().kin, world.as_deref());
        if self.tactics.is_some() {
            for team in 0..2u8 {
                if self.commander_due(team, &clock, &view) {
                    #[cfg(test)]
                    bench_work::set(bench_work::Who::Team(team));
                    self.commander(team, &view, &mut notes);
                    #[cfg(test)]
                    bench_work::set(bench_work::Who::Shared);
                }
            }
        }
        let mut rays = RayBudget::new(self.tunings.match_wide().think.rays_per_poll);
        rays.thinks_left = self.thinks_due(&view);
        let claims_on = [0u8, 1].map(|team| self.tuning_of(team).number(PlayKind::Roam, "fresh_claims") > 0.5);
        self.each_bot(&view, |bots, index| {
            // Before play begins: no plays, no routes, only rest.
            if clock.playing {
                bots.think(index, &view, &mut rays, &mut notes);
            }
            bots.sync_route_claim(index, now, claims_on);
            bots.ticks(index, &clock, &view, &mut made);
            if let Some(bot) = bots.bots[index].as_mut()
                && let Some(text) = bot.dropped.take()
            {
                bot.note(&mut notes, now, text);
            }
            bots.zone_walk_note(index, now, &mut notes);
            // A kit picked before play begins would respawn the bot where
            // it stands at once (the games' patch 31), not at its next
            // life: the bot picks once play has begun, for its next life.
            if clock.playing
                && let Some(bot) = bots.bots[index].as_mut()
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
                bots.features.kit_choices.push((bot.place, kit as u8));
            }
        });
        #[cfg(test)]
        bench_work::set(bench_work::Who::Idle);
        self.polls.record(now, &rays);
        #[cfg(any(test, feature = "client-test"))]
        self.client_tests_tick(&clock, &view, &notes);
        (made, notes)
    }

    /// A poll's bots, each given to `each` in the route-search queue's order
    /// (`search::Searching`): the poll's searches and floors first, the
    /// searches under way going on, each team done once its last bot went
    /// (its rest the other's), and the searches under way going on again at
    /// the poll's end on what both teams left (`SearchQueue::Fair`). The one
    /// loop of [`Self::poll`] and of the bench's timed copy of it.
    pub(super) fn each_bot(&mut self, view: &MatchView, mut each: impl FnMut(&mut Bots, usize)) {
        let think = &self.tunings.match_wide().think;
        let (queue, floors) = (think.search_queue, think.search_floors_per_poll as usize);
        self.features.searching.new_poll(queue, floors, search::living(&self.bots, view));
        self.go_on_searching();
        let mut to_go = [0usize; 2];
        for bot in self.bots.iter().flatten() {
            to_go[usize::from(bot.team & 1)] += 1;
        }
        for index in self.features.searching.order(&self.bots) {
            let Some(team) = self.bots[index].as_ref().map(|b| b.team) else {
                continue;
            };
            #[cfg(test)]
            bench_work::set(bench_work::Who::Team(team));
            each(self, index);
            let left = &mut to_go[usize::from(team & 1)];
            *left = left.saturating_sub(1);
            if *left == 0 {
                self.features.searching.team_done(team);
            }
        }
        #[cfg(test)]
        bench_work::set(bench_work::Who::Shared);
        if self.features.searching.goes_on_at_poll_end() {
            self.go_on_searching();
        }
        #[cfg(test)]
        self.features.searching.count_idle();
    }

    /// Whether `team`'s commander runs in this poll: when its time has come and play has begun; and, with `roam.fresh_claims` on, once
    /// in the start countdown, so the opening plays are chosen (and their routes drawn, each seeing the picks before it) before go,
    /// while the bots rest (no think runs before play begins).
    pub(super) fn commander_due(&mut self, team: u8, clock: &Clock, view: &MatchView) -> bool {
        if clock.clock < self.teams[usize::from(team)].next_commander {
            return false;
        }
        self.opening_pass = !clock.playing;
        if clock.playing {
            return true;
        }
        // Once, when the team's bots are in the match (alive in the view), not before.
        let opening = !self.opened[usize::from(team)]
            && self.tuning_of(team).number(PlayKind::Roam, "fresh_claims") > 0.5
            && self
                .bots
                .iter()
                .flatten()
                .any(|b| b.team == team && b.entered.is_some() && view.place(b.place).is_some_and(|p| p.alive));
        self.opened[usize::from(team)] |= opening;
        opening
    }

    /// A bot's drawn route is the team's to count while its run is on it, and not a moment after (`roam.fresh_claims`): the run
    /// ended for any reason (a new play, death, arrival, a replan), the claim goes in the same poll.
    fn sync_route_claim(&mut self, index: usize, now: u32, on: [bool; 2]) {
        let Some(bot) = self.bots[index].as_ref() else {
            return;
        };
        if !on.get(usize::from(bot.team)).copied().unwrap_or(false) {
            return;
        }
        let knowledge = &mut self.teams[usize::from(bot.team)].knowledge;
        match (bot.running.as_ref().and_then(Running::route_claim), bot.side) {
            (Some((route, drawn)), Some(side)) => knowledge.claim_route(bot.place, side, route, drawn, now),
            _ => knowledge.release_route(bot.place),
        }
    }

    /// The thinks that run in a poll at `view`: the bots that have entered,
    /// are alive and whose next think has come, counted before any ray is
    /// cast (`sense::fair_share`). None before play begins or the map is
    /// ready.
    pub(super) fn thinks_due(&self, view: &MatchView) -> u32 {
        if self.tactics.is_none() {
            return 0;
        }
        let due = self.bots.iter().flatten().filter(|bot| {
            bot.entered.is_some()
                && view.clock >= bot.next_think
                && view.place(bot.place).is_some_and(|p| p.alive)
        });
        due.count() as u32
    }

    /// The tuning team `team` plays by: the match's (the bench's one-sided
    /// runs give team 1 another).
    fn tuning_of(&self, team: u8) -> Arc<Tuning> {
        self.tunings.of(team).clone()
    }

    /// The tuning the bot at `place` of `team` plays by: its team's, except the bench's stand-in camper, who plays by a fixed one
    /// (`BenchCamper::tuning`) so that the two runs of a head-to-head bench face the same opponent.
    #[cfg_attr(not(test), allow(unused_variables))]
    fn tuning_for(&self, place: Place, team: u8) -> Arc<Tuning> {
        #[cfg(test)]
        if let Some(camper) = self.bench_camper.as_ref().filter(|camper| camper.place() == place) {
            return camper.tuning().clone();
        }
        self.tuning_of(team)
    }

    /// The tuning of what exists once for both teams (see [`tuning::TeamTunings::match_wide`]): for the server's map
    /// build and the bench's per-poll setup, never for a decision of a bot or a team ([`Self::tuning_of`]).
    pub fn match_wide_tuning(&self) -> &Tuning {
        self.tunings.match_wide()
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
    bot.features.crossing.goal.filter(|_| bot.route.is_some())
}

impl Bot {
}

#[cfg(test)]
mod tests;
