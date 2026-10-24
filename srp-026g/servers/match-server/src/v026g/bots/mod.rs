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
pub mod body;
pub mod cheats;
pub mod curve;
pub mod encode;
pub mod friends;
pub mod guns;
pub mod hazards;
pub mod kin;
pub mod know;
pub mod maps;
pub mod motor;
pub mod path;
pub mod plays;
pub mod recoil;
pub mod reflex;
pub mod rng;
pub mod routes;
pub mod sense;
pub mod signals;
pub mod stuck;
pub mod tactics;
pub mod team;
pub mod trace;
pub mod tuning;
pub mod view;

use std::sync::Arc;

use mapdata::{NodeId, Vec3, World};
use survarium::v026g::loadouts::{BotRole, BotTier};
use survarium::v026g::match_msgs::PlayerInput;

use crate::v026g::drive::{half_value, key};
use body::Fix;
use cheats::CheatClock;
use encode::{Decision, Encoder};
use know::{DangerSpot, LookSpot, Loss, Memory, Percept, Source, TeamKnowledge, Theft};
use motor::{JumpBy, Motor, Pose, RoutePress, Walked};
use path::Route;
use plays::{BotFacts, Ctx, Go, Intent, Look, Pace, PlayKind, Running, Step};
use reflex::Reflex;
use rng::Rng;
use sense::RayBudget;
use signals::{Duel, MatchSignals, Mods, Skill};
use stuck::{Recovery, Stuck, TeamSpots};
use tactics::Tactics;
use team::{Asking, Assignment};
use trace::Trace;
use tuning::Tuning;
use view::{BatteryState, Clock, MatchView, Place, PlaceView, Tracker, WorldFacts};

/// From a bot's entry to its first move, ms.
const START_AFTER_MS: u32 = 2_000;
/// The own eye is led at most this far past its last decided tick
/// (`CombatTuning::lead_own`): a tick, or two after a skipped one.
const LEAD_OWN_MOST_MS: u32 = 100;
/// A body farther than this off its route plans again, metres.
const OFF_ROUTE_M: f32 = 3.0;
/// A goal that moved this much is planned to again, metres.
const REPLAN_MOVED_M: f32 = 2.0;
/// A world that puts the body this far from its reckoning plans again.
const REPLAN_ERROR_M: f32 = 2.0;
/// The reflex's side step is taken only with a clear walk this far that
/// side, metres (no step off a ledge or into a wall).
const STRAFE_CLEAR_M: f32 = 1.5;
/// A jump pressed while the weapon fired lately is held this long, ms,
/// with the trigger let go: the weapon takes a jump only idle, reloading or
/// chambering a round (`weapon_core::can_jump` 0x2E3A70: states 3, 4 and
/// 6; `jump_predicate` 0x2FBDC0 asks it before the jump key), never while
/// it fires (5), shows or hides (1, 2). Aiming is no state: it does not
/// stop a jump. PROVEN in the code; `docs/notes/player_bots_stuck.md` §6.
const JUMP_HOLD_MS: u32 = 300;
/// What followed a jump is traced this long after it at most, ms.
const JUMP_WATCH_MS: u32 = 3_000;
/// A play's use look (`Look::Use`) this near the eye wins over the
/// reflex's aim (`decide`): the client's use reaches 2 m from the eye
/// (`base_player::detect_usable_object`, PROVEN), and the half metre more
/// turns the view while the bot walks in.
const USE_LOOK_M: f32 = 2.5;
/// A goal no way was found to is not planned to again for this long, ms.
const NO_WAY_RETRY_MS: u32 = 500;
/// How long the plays leave a goal no way was found to (`Bot::no_way_to`):
/// a zone on a timer may open meanwhile, and the bot does something else.
pub const NO_WAY_KEEP_MS: u32 = 20_000;
/// How long the whole team leaves a goal one of its bots proved no way to
/// (`TeamState::no_way_to`): in the owner's match of 2026-10-16 all six
/// bots tried one battery 183 times, each learning on its own, for 20 s.
/// A pocket that zones on all match close is left for the match anyway
/// (`Ctx::reachable`); this is for the rest (a switching zone's, a spawn
/// area's).
pub const TEAM_NO_WAY_KEEP_MS: u32 = 60_000;
/// Grid searches a poll may make for all the match's bots: a search costs
/// up to tens of ms on the host, and the match loop is the server's one
/// thread. A bot left without one plans at its next tick, its old route
/// meanwhile. The floors they expand are bounded too
/// ([`tuning::ThinkTuning::search_floors_per_poll`], [`Searching`]).
const SEARCHES_PER_POLL: u32 = 2;
/// A dead enemy carrier's look spot is made only from a sense of him this
/// recent, ms.
const LOOK_SENSED_MS: u32 = 30_000;
/// After a theft whose thief dropped the battery alive ([`Bots::theft_drop`]):
/// the team looks round its container at this many spots this far off it
/// (each looked at within `battery_near_m`, 3 m: the ring 1-7 m off),
/// while the theft is this recent (ms); the looks go after
/// [`THEFT_LOOK_KEEP_MS`].
const THEFT_RING_POINTS: usize = 6;
const THEFT_RING_M: f32 = 4.0;
const THEFT_RING_MS: u32 = 20_000;
const THEFT_LOOK_KEEP_MS: u32 = 30_000;
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

/// A team's shared state: knowledge, modifiers, skill, the commander's
/// clock.
struct TeamState {
    knowledge: TeamKnowledge,
    mods: Mods,
    skill: Skill,
    cheats: CheatClock,
    signals: MatchSignals,
    next_commander: u32,
    /// Goals a bot of the team proved no way to, and until when the
    /// team's plays leave them ([`TEAM_NO_WAY_KEEP_MS`]).
    no_way_to: Vec<(Vec3, u32)>,
    /// Where its bots got stuck or fell, for the match ([`TeamSpots`]).
    spots: TeamSpots,
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
    /// A goal no way was found to, and until when it is not tried again.
    no_way: Option<(Vec3, u32)>,
    /// Goals a search from its own floor proved no way to (over the floors
    /// it may walk on then), and until when the plays leave them
    /// ([`NO_WAY_KEEP_MS`]): a battery ringed by fire is not fetched again
    /// and again, standing.
    no_way_to: Vec<(Vec3, u32)>,
    /// Keys to press for one tick.
    press: u32,
    /// Keys held for a few ticks to get unstuck.
    unstick: Option<(u32, u32)>,
    /// Ticks left of a jump held with the trigger let go ([`JUMP_HOLD_MS`]),
    /// and the last tick the trigger was down.
    jump_hold: u32,
    fired_at: Option<u32>,
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
    /// Medkits used this life.
    medkits_used: u8,
    /// The answer of its route search that went on over several polls
    /// ([`Pending`]): the goal and the route, or none. Taken at its next
    /// tick, and used if it still heads there.
    found: Option<(Vec3, Option<Route>)>,
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
    /// Stands its takes of batteries gave up at lately
    /// (`plays::collect::FailedStand`): its next take stands elsewhere.
    failed_stands: Vec<plays::collect::FailedStand>,
    /// The kit of this life, and the one it picked for its next (a kit
    /// choice, 0xF3, takes effect at the spawn).
    kit: usize,
    kit_chosen: usize,
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
            personality,
            skill: tuning.skill.of_tier(BotTier::default()).start,
            side: None,
            running: None,
            assignment: None,
            last_ran: Vec::new(),
            intent: Intent::stay(),
            route: None,
            no_way: None,
            no_way_to: Vec::new(),
            press: 0,
            unstick: None,
            jump_hold: 0,
            fired_at: None,
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
            found: None,
            dropped: path::Dropped::default(),
            kits,
            life_carried,
            role: role.and_then(|role| BotRole::ALL.into_iter().find(|each| each.name() == role)),
            set_aside: Vec::new(),
            failed_stands: Vec::new(),
            kit: 0,
            kit_chosen: 0,
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
        self.route = None;
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
        self.intent = Intent::stay();
        self.stuck.reset();
        self.unstick = None;
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
        if self.side.is_none() {
            self.side = tactics.and_then(|tactics| tactics.side_at(feet));
        }
    }
}

/// A poll's route searches, as Detour's crowd runs its path queue: at most
/// [`SEARCHES_PER_POLL`] searches and `floors` floors expanded, all the
/// match's bots together. A search that runs past the floors is left under
/// way (`pending`, one a match) and goes on first at the next polls; no
/// other search starts meanwhile, and its bot keeps its old route. The
/// floors are counted, not timed, so the bots stay deterministic.
struct Searching {
    left: u32,
    floors: usize,
    pending: Option<Pending>,
}

/// A match's hold on its thread's search arrays (8 bytes a floor of the
/// largest grid searched, twice while a search is left under way): when
/// the thread's last match with bots ends, they are given back
/// ([`mapdata::free_search_memory`]), so the match loop keeps none between
/// matches with bots.
struct SearchMemory;

impl SearchMemory {
    fn hold() -> Self {
        let _ = MATCHES_WITH_BOTS.try_with(|n| n.set(n.get() + 1));
        Self
    }
}

impl Drop for SearchMemory {
    fn drop(&mut self) {
        let left = MATCHES_WITH_BOTS.try_with(|n| {
            n.set(n.get().saturating_sub(1));
            n.get()
        });
        if left == Ok(0) {
            let bytes = mapdata::search_memory_bytes();
            mapdata::free_search_memory();
            if bytes > 0 {
                log::info!(
                    "No match with bots left: their search arrays given back ({:.1} MB)",
                    bytes as f64 / 1e6
                );
            }
        }
    }
}

thread_local! {
    /// This thread's matches with bots ([`SearchMemory`]).
    static MATCHES_WITH_BOTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A route search left under way, and what it is for: it lets through the
/// floors it started with ([`Tactics::resume_route`]).
struct Pending {
    place: Place,
    goal: Vec3,
    time: u32,
    side: Option<i32>,
    objective: bool,
    /// From and to.
    ends: (NodeId, NodeId),
    /// The spots it got stuck at, kept clear of: the first search of two
    /// (the second goes through them).
    spots: Option<Vec<Vec3>>,
    /// [`hazards::Hazards::version`] when it started.
    hazards: u64,
    planning: path::Planning,
}

/// Whether `point` is near one of `spots` ([`Stuck::near_spot`]).
fn near_any(spots: &[Vec3], point: Vec3) -> bool {
    spots.iter().any(|&spot| stuck::near(spot, point))
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
    tracker: Tracker,
    tactics: Option<Tactics>,
    teams: [TeamState; 2],
    bots: Vec<Option<Bot>>,
    trace: Trace,
    /// Every place's weapon 1 kind, where known (play 11).
    weapons: Vec<Option<reflex::WeaponKind>>,
    /// The human routes of this map and mode (play 17), till the map is
    /// built and they are put on its floors.
    routes: Vec<routes::HumanRoute>,
    /// This poll's route searches ([`Searching`]).
    searching: Searching,
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
            tactics: None,
            teams: [team_state(&tuning), team_state(&tuning)],
            bots,
            trace,
            tuning,
            #[cfg(test)]
            side1_tuning: None,
            routes: Vec::new(),
            weapons: Vec::new(),
            searching: Searching {
                left: SEARCHES_PER_POLL,
                floors: 0,
                pending: None,
            },
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
        summary
    }

    /// The human routes of the match's map and mode (play 17), from
    /// `SRP_BOT_ROUTES`; before [`Self::map_ready`].
    pub fn set_routes(&mut self, table: &routes::RouteTable) {
        self.routes = table.for_map(self.level, self.mode);
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

    /// A trusted world's word on one place's body.
    pub fn fixed(&mut self, place: Place, fix: Fix, health: Option<f32>) -> Vec<Note> {
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
        if let Some((recovery, by_keys)) = bot.stuck.walking(time, feet, meant, keys) {
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
            // Each time again at the spot it backs off another way.
            let escape = bot.stuck.escape(side, other);
            let ticks = |rng: &mut Rng, s: (f32, f32)| (rng.range(s.0, s.1) / kin::TICK_S) as u32;
            match recovery {
                Recovery::BackOff => {
                    bot.unstick = Some((escape | key::JUMP, ticks(&mut bot.rng, (0.5, 0.8))));
                    bot.route = None;
                }
                Recovery::Sidestep => {
                    bot.unstick = Some((escape | key::JUMP, ticks(&mut bot.rng, (0.8, 1.3))));
                    bot.end_play(time, "stuck twice there", &mut notes);
                    self.teams[team].next_commander = time;
                }
                Recovery::GiveUp => {
                    bot.unstick = Some((escape, ticks(&mut bot.rng, (0.8, 1.3))));
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
            bot.route = None;
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

    /// The rest of a trusted world: deaths become danger spots and duels,
    /// a battery of ours carried off is a theft.
    ///
    /// A lying battery's position is taken as the world gives it: the
    /// worlds are in the map's frame (PROVEN, 0x4da3b51d: the batteries at
    /// 50 s lie on level_08's spawner spots to 0.01 m). The bots once
    /// guessed a mirrored z until a battery far from z = 0 voted otherwise;
    /// on level_05, whose spawners lie at z -0.5 to -1, they took the mirror
    /// image all match and never took those batteries.
    pub fn facts(&mut self, facts: WorldFacts) -> Vec<Note> {
        let mut notes = Vec::new();
        let time = facts.time;
        let at = |clock| Clock {
            clock,
            time_left_s: 0.0,
            total_s: 0.0,
            playing: true,
        };
        let before = self.tracker.view(&at(time), &self.tuning.kin, None);
        let new_deaths = self.tracker.facts(facts);
        // Who the view believes carries the batteries, and why (`view`).
        for text in self.tracker.take_notes() {
            notes.push(Note {
                place: usize::MAX,
                time,
                text,
            });
        }
        let after = self.tracker.view(&at(time), &self.tuning.kin, None);
        let kills_by = |bot: bool| {
            after
                .places
                .iter()
                .zip(&before.places)
                .any(|(a, b)| a.bot == bot && a.kills > b.kills)
        };
        let (human_kills, bot_kills) = (kills_by(false), kills_by(true));
        for (a, b) in after.places.iter().zip(&before.places) {
            if a.deaths <= b.deaths {
                continue;
            }
            let team = usize::from(a.team & 1);
            // The kill feed's spot: where the body was last reckoned alive
            // (the world showing the death has it without feet, and was
            // fixed first: `before` has none). A body never placed gives
            // no spot.
            if let Some(death) = new_deaths.iter().find(|d| d.place == a.place) {
                self.teams[team].knowledge.danger.push(DangerSpot {
                    at: death.at,
                    time,
                    victim: a.place,
                });
                // And a loss for the spawners' search: the spot the bot
                // was sent to look at, if it was.
                let spawner = self
                    .bots
                    .iter()
                    .flatten()
                    .find(|b| b.place == a.place)
                    .and_then(|b| b.assignment.as_ref())
                    .and_then(|a| match (a.kind, a.target) {
                        (plays::PlayKind::Collect, plays::TargetRef::Spawner(i)) => Some(i),
                        _ => None,
                    });
                self.teams[team].knowledge.losses.push(Loss {
                    at: death.at,
                    time,
                    spawner,
                });
            }
            // An enemy known to carry a battery died: his team's HUD shows
            // where it fell, ours does not. The team looks where it last
            // sensed him itself (never the thief cheat's place).
            if !self.tuning.cheats.batteries {
                self.lost_carrier(a.place, 1 - team, (&before, &after), time, &mut notes);
            }
            // Self-balancing counts duels between bots and humans.
            if a.bot && human_kills {
                self.teams[team].skill.duel(Duel {
                    time,
                    bot_won: false,
                });
            }
            if !a.bot && bot_kills {
                self.teams[1 - team].skill.duel(Duel {
                    time,
                    bot_won: true,
                });
            }
            for bot in self.bots.iter_mut().flatten() {
                bot.memory.forget(a.place);
            }
            // Nor do the team's callouts of him bring him back (match
            // 0x5402a730: 115 of 125 bot lines 1.5-5 s after a death still
            // knew the dead enemy, told).
            for state in &mut self.teams {
                state.knowledge.forget_callouts(a.place);
            }
        }
        // With the battery cheat off, what each team's HUD shows of the
        // batteries.
        if !self.tuning.cheats.batteries {
            for team in 0..2u8 {
                let knowledge = &mut self.teams[usize::from(team)].knowledge;
                knowledge.hud_update(team, &before, &after, time);
                knowledge.settle_looks(team, &after);
            }
        }
        // A battery of ours taken from home.
        for (battery, (now, then)) in after.batteries.iter().zip(&before.batteries).enumerate() {
            if let (BatteryState::Carried(thief), BatteryState::Home(id)) = (now, then)
                && let Some(container) = after.containers.iter().find(|c| c.id == *id)
            {
                let team = usize::from(container.team & 1);
                self.teams[team].knowledge.thefts.push(Theft {
                    battery,
                    at: container.at,
                    time,
                    thief: *thief,
                });
                notes.push(Note {
                    place: usize::MAX,
                    time,
                    text: format!("team {team}'s battery {battery} is stolen"),
                });
            }
        }
        // A battery of ours stolen and dropped by its thief alive, or (in
        // one world) taken from home and dropped: the team looks for it.
        if !self.tuning.cheats.batteries {
            for (battery, (now, then)) in after.batteries.iter().zip(&before.batteries).enumerate()
            {
                if !matches!(now, BatteryState::Free(_)) {
                    continue;
                }
                match *then {
                    BatteryState::Carried(Some(thief))
                        if after.place(thief).is_some_and(|p| p.alive) =>
                    {
                        let Some(thief_team) = after.place(thief).map(|p| usize::from(p.team & 1))
                        else {
                            continue;
                        };
                        let team = 1 - thief_team;
                        let Some(theft) = self.teams[team]
                            .knowledge
                            .thefts
                            .iter()
                            .rev()
                            .find(|t| t.battery == battery && t.thief.is_none_or(|p| p == thief))
                            .copied()
                        else {
                            continue;
                        };
                        self.theft_drop(
                            team,
                            battery,
                            theft.at,
                            theft.time,
                            Some(thief),
                            time,
                            &mut notes,
                        );
                    }
                    BatteryState::Home(id) => {
                        let Some(container) = after.containers.iter().find(|c| c.id == id) else {
                            continue;
                        };
                        let team = usize::from(container.team & 1);
                        self.theft_drop(team, battery, container.at, time, None, time, &mut notes);
                    }
                    _ => {}
                }
            }
        }
        notes
    }

    /// The enemy at `who` died, and team `team` (his enemies) knew he
    /// carried a battery: it stole from us (the HUD's theft), or a bot of
    /// the team saw him carrying it. The battery fell where he died, shown
    /// on his team's HUD only (`TeamKnowledge::hud_update`), and a nook may
    /// hide it from sight (match 0x15aa5b99: 126 s 3.6 m from our
    /// container). The team gets a spot to look at: where it last sensed
    /// him itself, the killer's own sight first, else the newest sight,
    /// hearing or callout of him (never the thief cheat's place).
    fn lost_carrier(
        &mut self,
        who: Place,
        team: usize,
        (before, after): (&MatchView, &MatchView),
        time: u32,
        notes: &mut Vec<Note>,
    ) {
        let knowledge = &self.teams[team].knowledge;
        // His battery: carried by him, or (a world showed him dead first,
        // and the carrier went with it) one carried by nobody known that
        // he was known to carry, lying now or still carried.
        let battery = (0..after.batteries.len()).find(|&b| {
            let was = before.batteries.get(b).copied();
            let carried = was == Some(BatteryState::Carried(Some(who)));
            let fell = was == Some(BatteryState::Carried(None))
                && matches!(
                    after.batteries[b],
                    BatteryState::Free(_) | BatteryState::Carried(None)
                );
            (carried || fell) && knowledge.knew_carrying(who, b)
        });
        let Some(battery) = battery else {
            return;
        };
        if knowledge.battery(battery).is_some() {
            return;
        }
        let killers: Vec<Place> = after
            .places
            .iter()
            .zip(&before.places)
            .filter(|(a, b)| usize::from(a.team & 1) == team && a.kills > b.kills)
            .map(|(a, _)| a.place)
            .collect();
        let Some((killer, sensed, at)) = self.last_sensed(who, team, &killers) else {
            return;
        };
        if sensed + LOOK_SENSED_MS < time {
            return;
        }
        self.teams[team].knowledge.add_look(LookSpot {
            battery,
            at,
            time,
            until: time + know::LOOK_KEEP_MS,
            theft: false,
        });
        notes.push(Note {
            place: usize::MAX,
            time,
            text: format!(
                "team {team} looks for battery {battery} where its carrier died ({}, {:.1} s before)",
                if killer { "the killer's sight" } else { "last sensed" },
                time.saturating_sub(sensed) as f32 / 1000.0
            ),
        });
    }

    /// Where team `team` last sensed the enemy at `who` itself: the sight
    /// of one of `killers` first, else the newest sight, hearing or
    /// callout of him (never the thief cheat's place). (Whether a killer's
    /// sight, when, where.)
    fn last_sensed(&self, who: Place, team: usize, killers: &[Place]) -> Option<(bool, u32, Vec3)> {
        let knowledge = &self.teams[team].knowledge;
        // (the killer's sight, when, where)
        let mut best: Option<(bool, u32, Vec3)> = None;
        let mut take = |killer: bool, sensed: u32, at: Vec3| {
            if best.is_none_or(|(k, t, _)| (killer, sensed) > (k, t)) {
                best = Some((killer, sensed, at));
            }
        };
        for bot in self
            .bots
            .iter()
            .flatten()
            .filter(|b| usize::from(b.team & 1) == team)
        {
            if let Some(track) = bot.memory.tracks().iter().find(|t| t.who == who)
                && let Some((at, sensed)) = track.own
            {
                let killer = killers.contains(&bot.place) && track.source == Source::Seen;
                take(killer, sensed, at);
            }
        }
        for seen in knowledge.carriers_seen.iter().filter(|s| s.who == who) {
            take(false, seen.time, seen.at);
        }
        best
    }

    /// F2 of the analysis of the owner's matches 0x652d/0x1a09 of
    /// 2026-10-16: a battery of team `team` stolen at `theft_time` from its
    /// container at `container` lies now, and its thief (`thief`, if the
    /// team can tell) lives: he dropped it (the owner took batteries out
    /// and dropped them 2-5 m off; they lay unknown 23-40 s). The HUD gave
    /// the theft and the container's count; the drop is on his team's HUD
    /// only. Unless the team knows where it lies, it looks round its
    /// container (a ring of [`THEFT_RING_POINTS`] spots [`THEFT_RING_M`]
    /// off it, while the theft is [`THEFT_RING_MS`] recent) and where it
    /// last sensed him itself, if that is off the ring; one bot does it
    /// (Collect), and the spots go after [`THEFT_LOOK_KEEP_MS`].
    #[allow(clippy::too_many_arguments)]
    fn theft_drop(
        &mut self,
        team: usize,
        battery: usize,
        container: Vec3,
        theft_time: u32,
        thief: Option<Place>,
        time: u32,
        notes: &mut Vec<Note>,
    ) {
        if self.teams[team].knowledge.battery(battery).is_some() {
            return;
        }
        let mut spots = Vec::new();
        let ring = time.saturating_sub(theft_time) <= THEFT_RING_MS;
        if ring {
            spots.extend((0..THEFT_RING_POINTS).map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / THEFT_RING_POINTS as f32;
                container + Vec3::new(angle.cos(), 0.0, angle.sin()) * THEFT_RING_M
            }));
        }
        let sensed = thief
            .and_then(|who| self.last_sensed(who, team, &[]))
            .filter(|&(_, sensed, _)| sensed + LOOK_SENSED_MS >= time)
            .map(|(_, sensed, at)| (sensed, at));
        let far = THEFT_RING_M + self.tuning.sense.battery_near_m;
        if let Some((_, at)) = sensed
            && (!ring || at.distance_xz(container) > far)
        {
            spots.push(at);
        }
        if spots.is_empty() {
            return;
        }
        self.teams[team].knowledge.add_theft_looks(
            battery,
            &spots,
            time,
            time + THEFT_LOOK_KEEP_MS,
        );
        notes.push(Note {
            place: usize::MAX,
            time,
            text: format!(
                "team {team} looks for battery {battery}, stolen {:.1} s before, its thief no longer carrying it ({}{})",
                time.saturating_sub(theft_time) as f32 / 1000.0,
                if ring { "round the container" } else { "" },
                match sensed {
                    Some((sensed, _)) if spots.len() > THEFT_RING_POINTS || !ring => format!(
                        "{}where last sensed {:.1} s before",
                        if ring { ", " } else { "" },
                        time.saturating_sub(sensed) as f32 / 1000.0
                    ),
                    _ => String::new(),
                }
            ),
        });
    }

    /// The inputs due with the clock at `clock`.
    pub fn poll(&mut self, clock: Clock) -> (Vec<Made>, Vec<Note>) {
        let (mut made, mut notes) = (Vec::new(), Vec::new());
        let now = clock.clock;
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        self.tracker.held_sounds(now);
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

    /// The route search left under way goes on, first in the poll (as
    /// Detour's path queue goes on with its head): its answer to its bot,
    /// for its next tick. Dropped when its bot left or the hazards changed.
    fn go_on_searching(&mut self) {
        let Some(mut pending) = self.searching.pending.take() else {
            return;
        };
        let Some(tactics) = self.tactics.as_ref() else {
            return;
        };
        if tactics.hazards.version() != pending.hazards
            || self.bots.get(pending.place).is_none_or(Option::is_none)
        {
            return;
        }
        if self.searching.left == 0 {
            self.searching.pending = Some(pending);
            return;
        }
        self.searching.left -= 1;
        let floors = &mut self.searching.floors;
        let (time, side, objective) = (pending.time, pending.side, pending.objective);
        let grid = &tactics.world.grid;
        let spots = pending.spots.take();
        let open = |node: NodeId| {
            spots
                .as_deref()
                .is_none_or(|spots| !near_any(spots, grid.position(node)))
        };
        let ended =
            tactics.resume_route(&mut pending.planning, time, side, objective, open, floors);
        let Some(mut route) = ended else {
            pending.spots = spots;
            self.searching.pending = Some(pending);
            return;
        };
        // None round the spots it got stuck at: through them.
        if route.is_none() && spots.is_some() {
            let (from, to) = pending.ends;
            match tactics.start_route(from, to, time, side, objective, |_| true, floors) {
                Ok(through) => route = through,
                Err(planning) => {
                    pending.planning = planning;
                    self.searching.pending = Some(pending);
                    return;
                }
            }
        }
        if let Some(Some(bot)) = self.bots.get_mut(pending.place) {
            bot.found = Some((pending.goal, route));
        }
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

    /// A team's commander tick: signals, modifiers, skill, and a play for
    /// each living bot.
    fn commander(&mut self, team: u8, view: &MatchView, notes: &mut Vec<Note>) {
        let Some(tactics) = self.tactics.as_ref() else {
            return;
        };
        let tuning = self.tuning_of(team);
        let now = view.clock;
        let state = &mut self.teams[usize::from(team)];
        state.next_commander = now + tuning.think.commander_ms;
        state.knowledge.forget_old(now);
        state.knowledge.settle_looks(team, view);
        // What a bot of the team proved no way to lately, the whole team
        // leaves for longer, from the proof on.
        state.no_way_to.retain(|&(_, until)| now < until);
        for bot in self.bots.iter_mut().flatten().filter(|b| b.team == team) {
            learn_no_way(&mut state.no_way_to, &bot.no_way_to, now);
            state.spots.learn(now, &bot.stuck.tell());
        }
        // Where its bots got stuck or fell, every bot keeps clear of.
        let spots = state.spots.avoided();
        for bot in self.bots.iter_mut().flatten().filter(|b| b.team == team) {
            bot.stuck.share(spots.clone());
        }
        let since_theft = state
            .knowledge
            .thefts
            .iter()
            .map(|t| now.saturating_sub(t.time) as f32 / 1000.0)
            .fold(1e6, f32::min);
        state.signals = MatchSignals::read(view, team, since_theft);
        state.mods.update(&state.signals, &tuning.mods);
        // A team that can no longer respawn (Team Deathmatch, its tickets
        // out) plays it safe.
        if !view.can_respawn[usize::from(team & 1)] {
            state.mods.aggression = state.mods.aggression.min(0.2);
        }
        if let Some((before, after, kd)) = state.skill.balance(now, &tuning.skill) {
            notes.push(Note {
                place: usize::MAX,
                time: now,
                text: format!(
                    "team {team}'s skill {before:.2} -> {after:.2} (K/D against humans {kd:.2})"
                ),
            });
        }
        // Partners for play 21: the team's bots two by two (pairs, play
        // 25); an odd one out partners its nearest living teammate bot one
        // way, so a third bot too trades its partner's death.
        let mates: Vec<Place> = self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .map(|b| b.place)
            .collect();
        for pair in mates.chunks(2) {
            match pair {
                [a, b] => {
                    for (me, other) in [(*a, *b), (*b, *a)] {
                        if let Some(Some(bot)) = self.bots.get_mut(me) {
                            bot.partner = Some(other);
                            bot.paired = true;
                        }
                    }
                }
                [odd] => {
                    let alive = |place: Place| view.place(place).filter(|p| p.alive);
                    let nearest = alive(*odd).and_then(|me| {
                        mates
                            .iter()
                            .filter(|&&other| other != *odd)
                            .filter_map(|&other| alive(other))
                            .min_by(|a, b| {
                                a.feet
                                    .distance_xz(me.feet)
                                    .total_cmp(&b.feet.distance_xz(me.feet))
                            })
                            .map(|p| p.place)
                    });
                    if let Some(Some(bot)) = self.bots.get_mut(*odd) {
                        // A partner down is kept (its death is the one to
                        // trade); dead itself, or none alive: the one it
                        // had, else the previous bot.
                        let kept = bot.partner.filter(|p| mates.contains(p) && p != odd);
                        let down = kept.filter(|&p| alive(p).is_none());
                        bot.partner = down
                            .or(nearest)
                            .or(kept)
                            .or_else(|| mates.len().checked_sub(2).map(|i| mates[i]));
                        bot.paired = false;
                    }
                }
                _ => {}
            }
        }
        // The bots checking spawners' spots now, and where (no more of
        // them than batteries missing, and apart, `plays::collect`).
        let searching = self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team)
            .filter_map(|b| match b.assignment.as_ref()? {
                a if a.kind == plays::PlayKind::Collect => match a.target {
                    plays::TargetRef::Spawner(index) => Some((b.place, index)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        self.teams[usize::from(team)].knowledge.searching = searching;
        let state = &self.teams[usize::from(team)];
        let mut asked: Vec<(Place, Vec<plays::Candidate>, Option<Assignment>)> = Vec::new();
        for bot in self
            .bots
            .iter()
            .flatten()
            .filter(|b| b.team == team && b.entered.is_some())
        {
            let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
                continue;
            };
            let no_way = team_no_way(&bot.no_way_to, &state.no_way_to);
            let cx = Ctx {
                now,
                view,
                me,
                bot: BotFacts {
                    personality: bot.personality(&tuning),
                    side: bot.side,
                    current: bot.assignment.as_ref().map(|a| (a.kind, a.target, a.since)),
                    last_ran: &bot.last_ran,
                    partner: bot.partner,
                    paired: bot.paired,
                    medkits_used: bot.medkits_used,
                    no_way_to: &no_way,
                    set_aside: &bot.set_aside,
                    failed_stands: &bot.failed_stands,
                    dry: bot.reflex.dry(),
                },
                memory: &bot.memory,
                team: &state.knowledge,
                sig: &state.signals,
                mods: &state.mods,
                tactics,
                tuning: &tuning,
            };
            let mut candidates = Vec::new();
            plays::candidates(&cx, &mut candidates);
            asked.push((bot.place, candidates, bot.assignment.clone()));
        }
        let asking = asked
            .iter()
            .map(|(place, candidates, current)| Asking {
                place: *place,
                candidates: candidates.clone(),
                current: current.as_ref(),
            })
            .collect();
        let gap = state.signals.score_gap;
        for (place, assignment, changed) in team::assign_at(asking, now, gap) {
            let Some(me) = view.place(place) else {
                continue;
            };
            let Some(Some(bot)) = self.bots.get_mut(place) else {
                continue;
            };
            if !changed {
                bot.assignment = Some(assignment);
                continue;
            }
            // The new play starts.
            bot.give_way(assignment.kind, now);
            let no_way = team_no_way(&bot.no_way_to, &state.no_way_to);
            let cx = Ctx {
                now,
                view,
                me,
                bot: BotFacts {
                    personality: bot.personality(&tuning),
                    side: bot.side,
                    current: Some((assignment.kind, assignment.target, now)),
                    last_ran: &bot.last_ran,
                    partner: bot.partner,
                    paired: bot.paired,
                    medkits_used: bot.medkits_used,
                    no_way_to: &no_way,
                    set_aside: &bot.set_aside,
                    failed_stands: &bot.failed_stands,
                    dry: bot.reflex.dry(),
                },
                memory: &bot.memory,
                team: &state.knowledge,
                sig: &state.signals,
                mods: &state.mods,
                tactics,
                tuning: &tuning,
            };
            let mut rng = bot.rng.clone();
            let running = plays::start(assignment.kind, assignment.target, &cx, &mut rng);
            bot.rng = rng;
            bot.note(
                notes,
                now,
                format!(
                    "takes {:?} {:?} (score {:.2}: {})",
                    assignment.kind, assignment.target, assignment.score, assignment.reason
                ),
            );
            if self.trace.enabled() {
                let (mut top, left) = asked
                    .iter()
                    .find(|(p, _, _)| *p == place)
                    .map(|(_, c, was)| (c.clone(), was.clone()))
                    .unwrap_or_default();
                let bands = team::Bands::used();
                top.sort_by(|a, b| bands.better(a, b, gap < 0.0));
                let json = |c: &plays::Candidate| {
                    serde_json::json!({
                        "play": format!("{:?}", c.kind),
                        "target": format!("{:?}", c.target),
                        "rank": format!("{:?}", c.rank),
                        "score": c.score.get(),
                        "parts": c.why.parts.iter()
                            .map(|(i, v, s)| serde_json::json!([format!("{i:?}"), v, s]))
                            .collect::<Vec<_>>(),
                    })
                };
                // The play it leaves: its candidate now (null when it is
                // none), so a trace says why it was left.
                let left = left.map(|was| {
                    let now_as = top
                        .iter()
                        .find(|c| c.kind == was.kind && c.target == was.target)
                        .map(json);
                    serde_json::json!({
                        "play": format!("{:?}", was.kind),
                        "since": was.since,
                        "candidate": now_as,
                    })
                });
                let top: Vec<serde_json::Value> = top.iter().take(5).map(json).collect();
                self.trace.line(serde_json::json!({
                    "t": now, "bot": place, "team": team,
                    "pos": me.feet.to_array(), "health": me.health,
                    "takes": format!("{:?}", assignment.kind),
                    "reason": assignment.reason,
                    "left": left,
                    "top": top,
                    "mods": [state.mods.aggression, state.mods.risk, state.mods.objective_urgency],
                    "skill": state.skill.skill,
                }));
            }
            bot.running = Some(running);
            bot.route = None;
            bot.assignment = Some(assignment);
        }
    }

    /// A bot's think, when due: senses, memory, reflex, the play's step.
    fn think(
        &mut self,
        index: usize,
        view: &MatchView,
        rays: &mut RayBudget,
        notes: &mut Vec<Note>,
    ) {
        let Some(tactics) = self.tactics.as_ref() else {
            return;
        };
        let Some(team) = self.bots[index].as_ref().map(|bot| bot.team) else {
            return;
        };
        let tuning = self.tuning_of(team);
        let now = view.clock;
        let Some(bot) = self.bots[index].as_mut() else {
            return;
        };
        if bot.entered.is_none() || now < bot.next_think {
            return;
        }
        bot.next_think = now
            + bot.rng.range(
                tuning.think.think_ms.0 as f32,
                tuning.think.think_ms.1 as f32,
            ) as u32;
        let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
            return;
        };
        let team = usize::from(bot.team & 1);
        // Senses: sight (the order turns each think, so no enemy waits
        // long for a ray), hearing, the cheats, the team's callouts.
        let mut order: Vec<Place> = view.enemies_of(bot.team).map(|p| p.place).collect();
        if !order.is_empty() {
            let k = (now / 100) as usize % order.len();
            order.rotate_left(k);
        }
        let mut percepts = sense::sense(
            me,
            view,
            &tactics.world.sight,
            &tuning.sense,
            &tuning.cheats,
            bot.heard_until,
            &order,
            rays,
            &mut bot.rng,
        );
        bot.heard_until = now;
        let quiet = percepts.is_empty() && bot.memory.all_seen(now).is_empty();
        let state = &mut self.teams[team];
        for percept in &percepts {
            state
                .knowledge
                .call_out(bot.place, percept, tuning.sense.callout_ms);
            // An enemy seen carrying a battery (it is in his hands).
            if percept.source == Source::Seen
                && let Some(battery) = view.place(percept.who).and_then(|p| p.carrying)
            {
                state
                    .knowledge
                    .saw_carrying(percept.who, battery, percept.at, percept.time);
            }
        }
        percepts.extend(cheats::cheat_percepts(
            bot.team,
            view,
            &tuning.cheats,
            &mut state.cheats,
            me.feet,
            quiet,
            &mut bot.rng,
        ));
        // An enemy carrier's place, first-hand or the thief cheat's (a
        // callout is a teammate's own): whether he stays put (Intercept's
        // camping carrier).
        let carrying = |p: Place| view.batteries.contains(&BatteryState::Carried(Some(p)));
        for percept in &percepts {
            if matches!(
                percept.source,
                Source::Seen | Source::Heard | Source::Cheat(know::CheatKind::Thief)
            ) && carrying(percept.who)
            {
                state.knowledge.carrier_at(
                    percept.who,
                    percept.at,
                    percept.error_m,
                    percept.time,
                    carrying,
                );
            }
        }
        // Going for a battery, far off its reckoning and below it near it:
        // it fell there (a ledge by the battery), which the team counts against
        // offering it again (`plays::collect`, `fallen_at`).
        if let Some(a) = bot.assignment.as_ref()
            && a.kind == PlayKind::Collect
            && let plays::TargetRef::Battery(battery) = a.target
            && let Some(at) = state.knowledge.battery(battery).map(|k| k.at).or_else(|| {
                match view.batteries.get(battery) {
                    Some(BatteryState::Free(at)) if tuning.cheats.batteries => Some(*at),
                    _ => None,
                }
            })
            && me.feet.distance_xz(at) <= tuning.number(PlayKind::Collect, "fall_near_m")
            && me.feet.y < at.y - plays::collect::BELOW_STAND_M
            && self
                .tracker
                .body(bot.place)
                .and_then(|body| body.last_error())
                .is_some_and(|error| error > REPLAN_ERROR_M)
        {
            state.knowledge.fell_near(bot.place, at, now);
        }
        percepts.extend(state.knowledge.heard_by(bot.place, now));
        // With the battery cheat off, the batteries and spawners' spots it
        // sees become its team's knowledge.
        if !tuning.cheats.batteries && view.mode.has_batteries() {
            // Spawners only while some battery is missing.
            let missing = plays::batteries_unaccounted(view, bot.team, &state.knowledge) > 0;
            let worth: Vec<bool> = (0..tactics.world.map.battery_spawners.len())
                .map(|index| {
                    missing
                        && plays::spawner_worth_checking(
                            view,
                            &state.knowledge,
                            tactics,
                            &tuning,
                            index,
                        )
                })
                .collect();
            sense::sense_batteries(
                me,
                view,
                &tactics.world.sight,
                &tactics.world.map.battery_spawners,
                &tuning.sense,
                &mut state.knowledge,
                |index| worth[index],
                rays,
            );
        }
        let skill = bot.skill;
        let notice_factor = signals::mix(tuning.aim.notice, skill);
        let feet = me.feet;
        bot.memory.update(
            &percepts,
            now,
            |p: &Percept| {
                sense::notice_ms(&tuning.sense, p.at.distance_xz(feet), p.busy, notice_factor)
            },
            tuning.cheats.forget_ms.max(tuning.sense.forget_ms),
            tuning.sense.drop_ms,
        );
        let sit = reflex::Situation {
            view,
            tactics: Some(tactics),
            team: Some(&self.teams[team].knowledge),
            side: bot.side,
            partner: bot.partner,
            weapon: reflex::WeaponKind::of_role(&bot.personality(&tuning).name),
            weapons: &self.weapons,
        };
        bot.reflex
            .think_with(me, &sit, &bot.memory, now, &tuning.aim, skill, &mut bot.rng);
        // Hit from a direction alone (the HUD's marker): its callout.
        if let Some((at, bearing)) = bot.reflex.take_hit_note() {
            self.teams[team].knowledge.shots_from.push(know::ShotFrom {
                by: bot.place,
                at,
                bearing,
                time: now + tuning.sense.callout_ms,
            });
        }
        if now < bot.pause_until {
            bot.intent = Intent::stay();
            return;
        }
        // The play's step.
        let state = &self.teams[team];
        let Some(mut running) = bot.running.take() else {
            bot.intent = Intent::stay();
            return;
        };
        let target = bot
            .assignment
            .as_ref()
            .map_or(plays::TargetRef::None, |a| a.target);
        let cx = Ctx {
            now,
            view,
            me,
            bot: BotFacts {
                personality: bot.personality(&tuning),
                side: bot.side,
                current: bot.assignment.as_ref().map(|a| (a.kind, a.target, a.since)),
                last_ran: &bot.last_ran,
                partner: bot.partner,
                paired: bot.paired,
                medkits_used: bot.medkits_used,
                no_way_to: &bot.no_way_to,
                set_aside: &bot.set_aside,
                failed_stands: &bot.failed_stands,
                dry: bot.reflex.dry(),
            },
            memory: &bot.memory,
            team: &state.knowledge,
            sig: &state.signals,
            mods: &state.mods,
            tactics,
            tuning: &tuning,
        };
        let mut rng = bot.rng.clone();
        let step = plays::step(&mut running, target, &cx, &mut rng);
        bot.rng = rng;
        let step_name = running.step_name();
        bot.running = Some(running);
        match step {
            Step::Continue(mut intent) => {
                bot.press |= intent.press;
                if let Some(failed) = intent.failed_stand.take() {
                    bot.failed_stands.retain(|f| f.until > now);
                    bot.failed_stands.push(failed);
                    bot.note(notes, now, failed.note());
                }
                bot.intent = intent;
            }
            Step::Done(why) => {
                bot.end_play(now, why, notes);
                self.teams[team].next_commander = now;
            }
            Step::Failed(why) => {
                if let Some(a) = bot.assignment.as_ref()
                    && now < a.since + plays::QUICK_FAIL_MS
                {
                    bot.set_aside.retain(|&(_, _, until)| until > now);
                    bot.set_aside
                        .push((a.kind, a.target, now + plays::SET_ASIDE_MS));
                }
                bot.end_play(now, why, notes);
                self.teams[team].next_commander = now;
            }
        }
        if self.trace.enabled() && now >= bot.last_trace + tuning.think.trace_every_ms {
            bot.last_trace = now;
            let line = serde_json::json!({
                "t": now, "bot": bot.place, "pos": me.feet.to_array(), "health": me.health,
                "play": format!("{:?}", bot.assignment.as_ref().map(|a| a.kind)),
                "step": step_name,
                "aim_at": bot.reflex.target(),
                "known": bot.memory.tracks().iter()
                    .map(|t| serde_json::json!([t.who, format!("{:?}", t.source), t.error_m]))
                    .collect::<Vec<_>>(),
            });
            self.trace.line(line);
        }
    }

    /// A bot's ticks due: the motor drives the intent (the reflex
    /// overriding the look and the trigger), the encoder makes inputs.
    fn ticks(&mut self, index: usize, clock: &Clock, view: &MatchView, made: &mut Vec<Made>) {
        let Some(team) = self.bots[index].as_ref().map(|bot| bot.team) else {
            return;
        };
        let tuning = self.tuning_of(team);
        let Some(bot) = self.bots[index].as_mut() else {
            return;
        };
        let Some(entered) = bot.entered else {
            return;
        };
        bot.tracing = self.trace.enabled();
        bot.reflex.tracing = bot.tracing;
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        let searching = &mut self.searching;
        for time in bot.encoder.ticks(clock.clock, entered + START_AFTER_MS) {
            let estimate = self
                .tracker
                .body(bot.place)
                .and_then(|body| body.estimate().copied());
            let me = view.place(bot.place).filter(|p| p.alive);
            let decision = match (estimate, me, self.tactics.as_ref()) {
                (Some(at), Some(me), Some(tactics)) if clock.playing => {
                    // The base pitch: `drive_tick` adds the kick.
                    let me = PlaceView {
                        feet: at.feet,
                        yaw: at.yaw,
                        pitch: at.pitch,
                        velocity: at.velocity,
                        known_at: at.time,
                        ..me.clone()
                    };
                    drive_tick(bot, &me, view, tactics, &tuning, time, searching)
                }
                _ => bot.motor.rest(),
            };
            if let Some(line) = bot.reflex.take_friend_note() {
                bot.traced.push(line);
            }
            let (input, turn) = bot.encoder.encode(time, decision);
            let keys = bot.encoder.keys();
            // [recoil] The part of the pitch turn the kick takes, and the
            // tick's shots: the base pitch moves by the rest.
            let taken = bot.reflex.sent(time, keys, turn.1);
            self.tracker.input(
                bot.place,
                time,
                keys,
                (turn.0, turn.1 + taken),
                bot.reflex.kick(),
                &tuning.kin,
                world.as_deref(),
            );
            if let Some(input) = input {
                made.push(Made {
                    place: bot.place,
                    time,
                    input,
                });
            }
        }
        for line in bot.traced.drain(..) {
            self.trace.line(line);
        }
    }
}

/// A team learns its bot's goals with no way lately (`Bot::no_way_to`,
/// kept [`NO_WAY_KEEP_MS`] from the proof): each is left by the team
/// [`TEAM_NO_WAY_KEEP_MS`] from its latest proof.
fn learn_no_way(team: &mut Vec<(Vec3, u32)>, own: &[(Vec3, u32)], now: u32) {
    for &(goal, until) in own {
        let until = until - NO_WAY_KEEP_MS.min(until) + TEAM_NO_WAY_KEEP_MS;
        if now >= until {
            continue;
        }
        match team
            .iter_mut()
            .find(|(old, _)| old.distance_xz(goal) <= REPLAN_MOVED_M)
        {
            Some(old) => old.1 = old.1.max(until),
            None => team.push((goal, until)),
        }
    }
}

/// A bot's goals with no way lately and its team's
/// ([`TEAM_NO_WAY_KEEP_MS`]), for the commander's choice.
fn team_no_way(own: &[(Vec3, u32)], team: &[(Vec3, u32)]) -> Vec<(Vec3, u32)> {
    own.iter().chain(team).copied().collect()
}

/// One tick of a living bot (`me` with its base pitch): the reflex's look
/// and trigger, the play's legs and stance, the presses.
fn drive_tick(
    bot: &mut Bot,
    me: &PlaceView,
    view: &MatchView,
    tactics: &Tactics,
    tuning: &Tuning,
    time: u32,
    searching: &mut Searching,
) -> Decision {
    // [recoil] The weapon's update first, as the client's; the view is the
    // base pitch and the kick the last tick left (the reflex takes back
    // this tick's rise itself, `Override::pull`).
    let kick = bot.reflex.weapon_tick(me, time);
    let me = &PlaceView {
        pitch: (me.pitch + kick).clamp(-1.0, 1.0),
        ..me.clone()
    };
    let intent = bot.intent;
    let pose = Pose {
        feet: me.feet,
        yaw: me.yaw,
        pitch: me.pitch,
        speed: me.speed(),
    };
    if bot
        .jump_watch
        .as_ref()
        .is_some_and(|watch| time >= watch.at + JUMP_WATCH_MS)
    {
        bot.jump_followed(time, Some(me.feet), "went on");
    }
    // Getting unstuck overrides the rest: the keys for a few ticks, the
    // jump on the first.
    if let Some((keys, ticks)) = bot.unstick {
        bot.unstick = (ticks > 1).then_some((keys & !key::JUMP, ticks - 1));
        bot.motor.turn_time = None;
        bot.motor.track = None;
        let decision = bot.motor.rest();
        let by = (keys & key::JUMP != 0).then_some("unstick");
        let keys = jumping(bot, keys, by, me.feet, time);
        return Decision { keys, ..decision };
    }
    // The reflex sees and aims from the eye where this tick's move puts it
    // (`CombatTuning::lead_own`): the shot leaves from there.
    let led_me;
    let aiming_me = if tuning.aim.combat.lead_own {
        let ahead = time.saturating_sub(me.known_at).min(LEAD_OWN_MOST_MS) as f32 / 1000.0;
        led_me = PlaceView {
            feet: me.feet + Vec3::new(me.velocity[0] * ahead, 0.0, me.velocity[1] * ahead),
            known_at: time,
            ..me.clone()
        };
        &led_me
    } else {
        me
    };
    // [aimpoint] What of its target shows, before the reflex aims.
    bot.reflex.look(aiming_me, view, &tactics.world, time);
    let over = bot.reflex.tick(
        aiming_me,
        view,
        &bot.memory,
        time,
        // No shot while a jump is held (`JUMP_HOLD_MS`).
        intent.fire && bot.jump_hold == 0,
        &tuning.aim,
        bot.skill,
        &mut bot.rng,
    );
    bot.motor.turn_time = over.turn_time_s;
    let eye = aiming_me.eye();
    let aim_at = |p: Vec3| {
        let (dx, dz) = (p.x - eye.x, p.z - eye.z);
        (kin::yaw_of([dx, dz]), (p.y - eye.y).atan2(dx.hypot(dz)))
    };
    // A glance at a heard shot gives way to a play that uses something or
    // carries (`Override::glance`), and with `glance_run_yields` to one
    // that runs somewhere (a turned view slows the legs).
    let running = tuning.aim.combat.glance_run_yields
        && intent.pace == Pace::Run
        && matches!(intent.go, Go::To { at, within_m } if me.feet.distance_xz(at) > within_m);
    let glance_yields =
        over.glance && (matches!(intent.look, Look::Use(_)) || me.carrying.is_some() || running);
    // A use within its reach (a take of a battery, a put-in, a steal): the
    // play's view wins over the reflex's aim, and the reflex neither takes
    // the legs nor fires meanwhile. The client's use is one ray along the
    // view (`base_player::detect_usable_object`, PROVEN), so a bot that
    // kept its view on an enemy pressed use at him: in the whole-match
    // soaks the battery where the teams meet lay there all match in 6 runs
    // of 16 (its takers had a target 91 % of their time by it).
    let using = matches!(intent.look, Look::Use(p) if (p - eye).length() <= USE_LOOK_M);
    bot.motor.track = over.track.filter(|_| !glance_yields && !using);
    let aim = over
        .look_at
        .filter(|_| !glance_yields && !using)
        .map(|p| {
            let (yaw, pitch) = aim_at(p);
            (yaw + over.error.0, pitch + over.error.1)
        })
        .or(match intent.look {
            Look::At(p) => Some(aim_at(p + Vec3::new(0.0, 1.0, 0.0))),
            Look::Use(p) => Some(aim_at(p)),
            _ => None,
        });
    // The legs: the reflex's when it takes them (a push, breaking sight,
    // a re-peek, the weapon's range, in after a grenade), else the play's.
    let mut intent = intent;
    if let Some(legs) = over.legs.filter(|_| !using) {
        intent.go = Go::To {
            at: legs.at,
            within_m: legs.within_m,
        };
        intent.crouch = legs.crouch;
        intent.pace = if legs.run { Pace::Run } else { Pace::Walk };
        intent.objective = false;
        intent.straight = false;
    }
    let goal = match intent.go {
        Go::To { at, within_m } if me.feet.distance_xz(at) > within_m => Some(at),
        _ => None,
    };
    // Standing to aim (no way to walk): the reflex's side step may move it;
    // walking a route while it aims, the step weaves the walk
    // ([`weave`], `strafe_walking`).
    let mut aiming_still = false;
    let mut walking_aimed = false;
    let mut decision = match goal {
        // A step or two in a line the play vouches for: by keys, no route.
        Some(at) if intent.straight => {
            bot.route = None;
            bot.motor.step_to(&pose, at, aim)
        }
        Some(at) => {
            let replan = bot
                .route
                .as_ref()
                .is_none_or(|(_, planned)| planned.distance_xz(at) > REPLAN_MOVED_M);
            // A goal with no way lately: not searched again every tick.
            let no_way = bot.no_way.is_some_and(|(goal, until)| {
                time < until && goal.distance_xz(at) <= REPLAN_MOVED_M
            });
            // The answer of its search that went on over several polls, if
            // it is for this goal (it walks the way so far meanwhile).
            let found = bot
                .found
                .take()
                .filter(|(goal, _)| goal.distance_xz(at) <= REPLAN_MOVED_M);
            // Its own search under way: for this goal, else no longer
            // wanted.
            let waiting = match searching.pending.as_ref() {
                Some(pending) if pending.place == bot.place => {
                    let still = pending.goal.distance_xz(at) <= REPLAN_MOVED_M;
                    if !still {
                        bot.dropped.dropped(time, pending.goal, at);
                        searching.pending = None;
                    }
                    still
                }
                _ => false,
            };
            // Why it has no route, if it has none after this.
            let mut no_route = path::NoRoute::Searched;
            if let Some((goal, route)) = found {
                bot.no_way = route.is_none().then_some((goal, time + NO_WAY_RETRY_MS));
                // Both ends were on floors, from one it may walk on: proven.
                if route.is_none() {
                    bot.no_way_to.retain(|&(old, until)| {
                        time < until && old.distance_xz(goal) > REPLAN_MOVED_M
                    });
                    bot.no_way_to.push((goal, time + NO_WAY_KEEP_MS));
                }
                bot.route = route.map(|route| (route, goal));
            } else if replan && no_way {
                bot.route = None;
                no_route = path::NoRoute::NoWay;
            } else if waiting
                || (replan
                    && (searching.left == 0
                        || searching.floors == 0
                        || searching.pending.is_some()))
            {
                // Its search goes on, or none left to this poll: the old
                // route meanwhile.
                no_route = if waiting {
                    path::NoRoute::Searching
                } else {
                    path::NoRoute::Starved
                };
            } else if replan {
                searching.left -= 1;
                let grid = &tactics.world.grid;
                let side = bot.side;
                let objective = intent.objective;
                // Round the spots it got stuck at, if there is a way; else
                // through them (a second search only when there are any).
                let spots: Vec<Vec3> = bot.stuck.avoided_for(time, at).collect();
                let avoiding = !spots.is_empty();
                let ends = grid.locate(me.feet, 1.5).zip(path::goal_floor(grid, at));
                // Already on the goal's floor (the nearest to a goal on none):
                // as near as it gets, nothing to search.
                let there = ends.is_some_and(|(from, to)| from == to);
                // On a floor it may not walk on (in a fire it stepped
                // into): out to the nearest one it may first; the goal is
                // planned from there.
                let escaping =
                    ends.is_some_and(|(from, _)| !tactics.allowed(from, time, side, objective));
                let floors = &mut searching.floors;
                // Left under way past this poll's floors: whether round the
                // spots, and the search.
                let mut left = None;
                let route = if there {
                    None
                } else if escaping {
                    ends.and_then(|(from, _)| tactics.escape(from, time, side, objective))
                } else if let Some((from, to)) = ends {
                    let open = |node: NodeId| !(avoiding && near_any(&spots, grid.position(node)));
                    match tactics.start_route(from, to, time, side, objective, open, floors) {
                        Ok(None) if avoiding => {
                            match tactics.start_route(
                                from,
                                to,
                                time,
                                side,
                                objective,
                                |_| true,
                                floors,
                            ) {
                                Ok(route) => route,
                                Err(planning) => {
                                    left = Some((false, planning));
                                    None
                                }
                            }
                        }
                        // Round them only where that is not much longer
                        // (`stuck::worth_round`), else through them.
                        Ok(Some(round))
                            if avoiding
                                && stuck::may_be_long(
                                    round.length(),
                                    grid.position(from),
                                    grid.position(to),
                                ) =>
                        {
                            let through = tactics.start_route(
                                from,
                                to,
                                time,
                                side,
                                objective,
                                |_| true,
                                floors,
                            );
                            match through {
                                Ok(Some(through))
                                    if !stuck::worth_round(round.length(), through.length()) =>
                                {
                                    Some(through)
                                }
                                _ => Some(round),
                            }
                        }
                        Ok(route) => route,
                        Err(planning) => {
                            left = Some((avoiding, planning));
                            None
                        }
                    }
                } else {
                    None
                };
                if let (Some((round, planning)), Some(ends)) = (left, ends) {
                    // It goes on at the next polls (`Bots::go_on_searching`).
                    // Meanwhile the way so far, as Detour's partial paths:
                    // toward the goal, not standing (else the old route).
                    let open = |node: NodeId| !(round && near_any(&spots, grid.position(node)));
                    if let Some(partial) =
                        tactics.partial_route(&planning, time, side, objective, open)
                    {
                        bot.route = Some((partial, at));
                    }
                    no_route = path::NoRoute::Searching;
                    searching.pending = Some(Pending {
                        place: bot.place,
                        goal: at,
                        time,
                        side,
                        objective,
                        ends,
                        spots: round.then_some(spots),
                        hazards: tactics.hazards.version(),
                        planning,
                    });
                } else {
                    bot.no_way = route.is_none().then_some((at, time + NO_WAY_RETRY_MS));
                    // Both ends on floors, from one it may walk on, and no way:
                    // proven, for the plays.
                    if route.is_none() && ends.is_some() && !escaping && !there {
                        bot.no_way_to.retain(|&(goal, until)| {
                            time < until && goal.distance_xz(at) > REPLAN_MOVED_M
                        });
                        bot.no_way_to.push((at, time + NO_WAY_KEEP_MS));
                    }
                    bot.route = route.map(|route| (route, at));
                }
            }
            match bot.route.as_mut() {
                Some((route, _)) => {
                    let side = bot.side;
                    let objective = intent.objective;
                    let clear = |p: Vec3| tactics.clear_line_for(me.feet, p, time, side, objective);
                    let (decision, walked) = match aim {
                        Some(aim) => {
                            walking_aimed = true;
                            bot.motor.walk_aiming(route, &pose, OFF_ROUTE_M, clear, aim)
                        }
                        None => bot.motor.walk(route, &pose, OFF_ROUTE_M, clear),
                    };
                    if walked != Walked::OnTheWay {
                        // Fallen off it: the spot it fell from (`Stuck::fell`).
                        if let Walked::Off(off) = walked
                            && off.is_infinite()
                            && let Some(from) = route.fell_from(pose.feet)
                        {
                            bot.stuck.fell(time, from);
                        }
                        // A jump it could not make: its take-off, likewise.
                        if let Walked::JumpFailed(take_off) = walked {
                            bot.stuck.failed_jump(time, take_off);
                        }
                        bot.route = None;
                        path::no_route(path::NoRoute::Ended);
                    }
                    decision
                }
                None => {
                    path::no_route(no_route);
                    match aim {
                        Some(aim) => {
                            aiming_still = true;
                            bot.motor.aim_still(&pose, aim)
                        }
                        None => bot.motor.stand(&pose, &mut bot.rng),
                    }
                }
            }
        }
        None => {
            bot.route = None;
            match aim {
                Some(aim) => {
                    aiming_still = true;
                    bot.motor.aim_still(&pose, aim)
                }
                None => bot.motor.stand(&pose, &mut bot.rng),
            }
        }
    };
    // The side step, where the ground that side is clear for a stride:
    // standing, or weaving a walk to or from the target.
    let stepped = if aiming_still {
        Some(decision.keys | over.strafe)
    } else if walking_aimed && tuning.aim.combat.strafe_walking {
        weave(decision.keys, over.strafe)
    } else {
        None
    };
    if let Some(keys) = stepped.filter(|_| over.strafe != 0 && !using) {
        let left = over.strafe == key::LEFT;
        let [sx, sz] = kin::forward(me.yaw + std::f32::consts::FRAC_PI_2);
        let sign = if left { 1.0 } else { -1.0 };
        let to = me.feet + Vec3::new(sign * sx * STRAFE_CLEAR_M, 0.0, sign * sz * STRAFE_CLEAR_M);
        if tactics.clear_line_for(me.feet, to, time, bot.side, intent.objective) {
            decision.keys = keys;
        }
    }
    // The stance and pace.
    if intent.crouch || intent.pace == Pace::Sneak {
        decision.keys |= key::CROUCH;
        decision.keys &= !key::SPRINT;
    }
    if intent.pace != Pace::Run {
        decision.keys &= !key::SPRINT;
    }
    decision.turn.1 += over.pull;
    if over.fire && !using {
        decision.keys |= key::FIRE;
        decision.keys &= !key::SPRINT;
    } else if over.fire {
        // The shot the reflex counted this tick is not fired: given back.
        bot.reflex.hold_fire(time);
    }
    decision.keys |= over.keys;
    if over.keys & (key::GRENADE | key::RELOAD | key::AIM | key::CROUCH) != 0 {
        decision.keys &= !key::SPRINT;
    }
    decision.keys |= intent.hold | std::mem::take(&mut bot.press);
    // Lining up for a jump of its route, and in it: standing (a crouched
    // body seldom jumps: 9 route jumps of 12 pressed crouched did in the
    // owner's matches, 6 unstick jumps of 23, against 149 of 159 standing;
    // `player_bots_stuck.md` §8.9), and no sprint.
    if bot.motor.lining_up() {
        decision.keys &= !(key::CROUCH | key::SPRINT);
    }
    if decision.keys & key::QUICK[0] != 0 {
        bot.medkits_used = bot.medkits_used.saturating_add(1);
    }
    let by = bot.motor.jumped().map(|by| match by {
        JumpBy::Route => "route",
        JumpBy::Hop => "hop",
    });
    decision.keys = jumping(bot, decision.keys, by, me.feet, time);
    decision
}

/// A jump watched for its trace: why, when and from where; for a jump of
/// the route its take-off and landing, which try it was, and the first
/// world's tick that showed the body up it.
#[derive(Clone, Copy, Debug)]
struct JumpWatch {
    by: &'static str,
    at: u32,
    from: Vec3,
    route: Option<RoutePress>,
    up: Option<u32>,
}

impl Bot {
    /// The trace line of what followed the jump watched, if any (`then`, at
    /// `time`, the feet then if known); the watch ends.
    fn jump_followed(&mut self, time: u32, feet: Option<Vec3>, then: &str) {
        let Some(watch) = self.jump_watch.take() else {
            return;
        };
        if self.tracing {
            let mut line = serde_json::json!({
                "t": time, "bot": self.place, "jumped": watch.by,
                "after_ms": time.saturating_sub(watch.at), "then": then,
                "moved_m": feet.map(|f| f.distance_xz(watch.from)),
                "rose_m": feet.map(|f| f.y - watch.from.y),
            });
            if let Some(route) = watch.route {
                line["try"] = route.tries.into();
                line["up_ms"] = watch.up.map(|up| up.saturating_sub(watch.at)).into();
            }
            self.traced.push(line);
        }
    }

    /// A world of `time` showed the body at `feet`: up the route jump
    /// watched, if it is ([`motor::is_up`]).
    fn jump_seen(&mut self, time: u32, feet: Vec3) {
        if let Some(watch) = self.jump_watch.as_mut()
            && let Some(route) = watch.route
            && watch.up.is_none()
            && time > watch.at
            && motor::is_up(feet, route.take_off, route.landing)
        {
            watch.up = Some(time);
        }
    }
}

/// A tick's keys with its jump. A jump pressed now (`by`: the route's, a
/// hop, getting unstuck) is traced and watched; with the trigger down now or
/// within [`JUMP_HOLD_MS`] it is held that long with the trigger let go, the
/// shot of this tick given back ([`Reflex::hold_fire`]), as the weapon takes
/// no jump while it fires.
fn jumping(bot: &mut Bot, keys: u32, by: Option<&'static str>, feet: Vec3, time: u32) -> u32 {
    let firing = keys & key::FIRE != 0;
    if let Some(by) = by.filter(|_| bot.jump_hold == 0) {
        let lately = firing || bot.fired_at.is_some_and(|at| time < at + JUMP_HOLD_MS);
        bot.jump_hold = if lately {
            (JUMP_HOLD_MS as f32 / (kin::TICK_S * 1000.0)).round() as u32
        } else {
            1
        };
        bot.jump_followed(time, Some(feet), "jumped again");
        let route = (by == "route").then(|| bot.motor.route_press()).flatten();
        if bot.tracing {
            let mut line = serde_json::json!({
                "t": time, "bot": bot.place, "jump": by, "pos": feet.to_array(),
                "firing": lately, "aiming": keys & key::AIM != 0,
            });
            if let Some(route) = route {
                line["try"] = route.tries.into();
                line["from"] = route.take_off.to_array().into();
                line["to"] = route.landing.to_array().into();
                line["before_m"] = route.before.into();
                line["facing"] = route.facing.into();
                line["crouch"] = (keys & key::CROUCH != 0).into();
            }
            bot.traced.push(line);
        }
        bot.jump_watch = Some(JumpWatch {
            by,
            at: time,
            from: feet,
            route,
            up: None,
        });
    }
    if bot.jump_hold == 0 {
        if firing {
            bot.fired_at = Some(time);
        }
        return keys;
    }
    bot.jump_hold -= 1;
    if firing {
        bot.reflex.hold_fire(time);
    }
    (keys | key::JUMP) & !key::FIRE
}

/// A walk's keys with the side step `strafe` woven in: a walk straight to
/// or from where it looks (forward or back alone) goes the diagonal to that
/// side, so the walk goes on while the body weaves; a walk already to a side
/// is left as it is (it strafes already), and so is no walk.
fn weave(keys: u32, strafe: u32) -> Option<u32> {
    let along = keys & (key::FORWARD | key::BACK);
    let side = keys & (key::LEFT | key::RIGHT);
    (along != 0 && side == 0 && strafe != 0).then_some(keys | strafe)
}

#[cfg(test)]
mod tests;
