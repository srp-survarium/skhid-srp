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
/// The shooter's movement at a fire tick or a hit: in the air (a jump
/// under way), still (under [`STILL_MPS`] or no walk key), sprinting
/// (forward and sprint), strafing (a side key, no forward or back),
/// walking.
const MOVES: [&str; 5] = ["still", "walk", "sprint", "strafe", "air"];
const STILL_MPS: f32 = 0.3;
/// Target switches: an enemy who hit the bot this recently is hurting it,
/// ms.
const HURT_MS: u32 = 3_000;

/// The wrong-jump detector (`jumps/detect.py`, `jumps.md` §9, final): its
/// flags, the second numbers (counted, but alone they make no press
/// wrong), and the outcome flags (a failed jump).
const FLAGS: [&str; 16] = [
    "TOO_LOW",
    "NO_SPRINT",
    "PRESSED_LOW",
    "STAND_TURN",
    "SHORT_RUNUP",
    "FWD_RELEASED",
    "FWD_RELEASED_085",
    "BACK_SIDE",
    "IN_PLACE",
    "REPEAT",
    "LINK_AGAIN",
    "NOT_UP",
    "UP_THEN_DOWN",
    "PRESSED_UP",
    "NOT_FREED",
    "DROPPED",
];
const SECOND_NUMBERS: [&str; 2] = ["FWD_RELEASED_085", "LINK_AGAIN"];
const OUTCOMES: [&str; 7] = [
    "REPEAT",
    "LINK_AGAIN",
    "NOT_UP",
    "UP_THEN_DOWN",
    "PRESSED_UP",
    "NOT_FREED",
    "DROPPED",
];
/// A link's rise a jump makes, m: with sprint (not carrying), without.
const REACH_M: f32 = 1.30;
const SPRINTLESS_M: f32 = 1.00;
/// Pressed this far under the take-off, m.
const PRESSED_LOW_M: f32 = 0.30;
/// The run-up: a forward run under this many ticks; with this many still
/// ticks right before it; gaps of up to this many ticks with no walk key
/// bridged; looked for this many ticks back.
const RUN_TICKS: usize = 7;
const STILL_TICKS: usize = 4;
const RUN_GAP_TICKS: usize = 2;
const RUN_LOOK_TICKS: usize = 30;
/// Forward let go this many ticks after the press, at most (and the second
/// number's `CARRY_S` 0.85 s).
const RELEASED_TICKS: usize = 5;
const CARRY_TICKS: usize = 17;
/// A press again within this, ms, and this near the last, m.
const REPEAT_MS: u32 = 3_000;
const REPEAT_M: f32 = 2.0;
/// A route link pressed again within this, ms (its take-off and landing
/// both within [`LINK_M`]).
const LINK_AGAIN_MS: u32 = 10_000;
const LINK_M: f32 = 0.3;
/// Up a link: the feet this close under the landing, m; looked for this
/// many ticks after the press (0.2-2.2 s), and needing a record this long.
const UP_SLACK_M: f32 = 0.35;
const UP_FROM_TICKS: usize = 4;
const UP_TO_TICKS: usize = 44;
const UP_SEEN_TICKS: usize = 64;
/// Up, then down: links rising more than this, m; back on the take-off
/// within this many ticks, within this of it, m; or the link pressed
/// again within this, ms.
const RISE_MIN_M: f32 = 0.4;
const DOWN_TICKS: usize = 50;
const DOWN_FAR_M: f32 = 3.5;
const AGAIN_MS: u32 = 6_000;
/// Pressed while already up: within this of the landing, m.
const PRESSED_UP_NEAR_M: f32 = 1.5;
/// Not freed: the jump watch's 3 s; moved under this when it ended by a
/// stuck watch or another press, or under [`WENT_ON_M`] when it went on;
/// or stood at the press and every half second 1.0-3.2 s after.
const FREED_MS: u32 = 3_000;
const FREED_M: f32 = 1.0;
const WENT_ON_M: f32 = 0.5;
const STOOD_MPS: f32 = 0.6;
const STOOD_TICKS: (usize, usize) = (20, 64);
/// Stalls (§9.4): walking forward under this, m/s, moving under this
/// between seconds, m, for this long, s; a ledge ahead this high (missed
/// jump; step), this far ahead and at most this to the side, m; missed
/// when no press came from this before it, ms, or the first came this
/// late into it, ms.
const STALL_MPS: f32 = 0.6;
const STALL_MOVE_M: f32 = 0.5;
const STALL_S: f32 = 1.8;
const LEDGES: [(f32, f32); 2] = [(0.6, 1.0), (0.3, 0.6)];
const LEDGE_AHEAD_M: (f32, f32) = (0.25, 1.1);
const LEDGE_SIDE_M: f32 = 0.4;
const MISSED_BEFORE_MS: u32 = 500;
const MISSED_LATE_MS: u32 = 2_000;

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

/// The stuck detector (`navigation/stuck.py`, `navigation.md` §2d, final).
/// A designed hold's steps (and the Heal play); the steps with a goal to
/// walk to; the Follow-type steps, a goal only with the leader (the goal)
/// farther than [`LEADER_FAR_M`].
const HOLD_STEPS: [&str; 13] = [
    "Hold", "Holding", "Wait", "Pair", "Pair)", "Staying", "Looking", "Peek", "Trade", "Take",
    "PutIn", "LetGo", "NoSpot)",
];
const TRAVEL_STEPS: [&str; 14] = [
    "Going", "Route", "Via", "Fetch)", "Search)", "Search", "Bring", "Bring)", "Approach)",
    "Approach", "Chase", "Round", "Seek)", "Cut",
];
const FOLLOW_STEPS: [&str; 4] = ["Follow", "Follow)", "Behind", "Ahead"];
const LEADER_FAR_M: f32 = 20.0;
/// Its window kinds: the least seconds, the radius about the positions'
/// middle (m), the least share of ticks with a walk key, the share of aim
/// or fire under which it counts (none: any).
const STUCK_KINDS: [(&str, f32, f32, f32, Option<f32>); 3] = [
    ("push", 3.0, 0.6, 0.75, None),
    ("nohead", 10.0, 2.5, 0.70, Some(0.35)),
    ("loop", 20.0, 8.0, 0.80, Some(0.35)),
];
/// An episode's kind: the first of these it holds.
const STUCK_ORDER: [&str; 5] = ["push", "nohead", "loop", "stand", "watch"];
const PUSH_STILL_SPEED: f32 = 0.5;
const PUSH_OPPOSING_MAX: f32 = 0.25;
const PUSH_SPEED: f32 = 1.2;
const NOHEAD_HOLD_MAX: f32 = 0.5;
const STAND_S: f32 = 10.0;
const STAND_R: f32 = 0.6;
const STAND_WALK_MAX: f32 = 0.25;
const STAND_AIM_MAX: f32 = 0.5;
const STAND_TRAVEL: f32 = 0.7;
const LOOP_NET_M: f32 = 4.0;
const LOOP_PATH_M: f32 = 12.0;
const LOOP_RISE_M: f32 = 2.5;
/// The server's stuck watch firing: the body within this of where it was
/// then, m, with walk keys on this share between samples, for this long.
const WATCH_M: f32 = 1.0;
const WATCH_WALK: f32 = 0.5;
const WATCH_S: f32 = 3.0;
/// A fight dance (not stuck): aim or fire on this share, median speed
/// over this, m/s, opposing keys over this share.
const DANCE_AIM: f32 = 0.5;
const DANCE_MPS: f32 = 1.2;
const DANCE_OPPOSING: f32 = 0.25;
const MERGE_GAP_S: f32 = 2.0;
const SPAWN_GRACE_S: f32 = 3.0;
/// Causes: a teammate (or an enemy) this near, m; the feet up and down
/// this much, m.
const MATE_NEAR_M: f32 = 1.2;
const UP_DOWN_M: f32 = 0.8;
/// Spots with two or more cases: this across and this in height, m
/// (`stucktable.py`).
const SPOT_M: [f32; 2] = [4.0, 2.0];

/// Campers (`campers.md` Q2, Q6 "How a bench would see it"): a shooter's
/// kills within this of his run's first spot, m, each within this of the
/// last, ms; a held line reaches the kill's distance and this more, and
/// at least this, m; an entry is fatal when the shooter kills the bot
/// within this, ms, and in the killed lane within this of a victim's
/// bearing, degrees; runs and streaks of this many count.
const SPOT_HOLD_M: f32 = 8.0;
const RUN_APART_MS: u32 = 120_000;
const REACH_PLUS_M: f32 = 15.0;
const REACH_LEAST_M: f32 = 40.0;
const FATAL_MS: u32 = 15_000;
const LANE_DEG: f32 = 15.0;
const CAMP_RUN: u32 = 3;
/// Trades into the killer (`tally.py`, `campers.md` K3): the partner's
/// death this long before the Trade take, or this long after it, ms; the
/// trader dies to (or kills) that killer within [`FATAL_MS`] of the take.
const TRADE_BEFORE_MS: u32 = 4_000;
const TRADE_AFTER_MS: u32 = 500;
/// Answers to a held spot (`campers.md` K2): a holder killed by a bot this
/// far off every victim's bearing from the spot, degrees.
const OFF_LANE_DEG: f32 = 45.0;

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

/// A jump press.
#[derive(Clone, Copy, Debug)]
struct Press {
    place: Place,
    t: u32,
    feet: Vec3,
    by: &'static str,
    /// The route's take-off and landing, for a route jump.
    route: Option<(Vec3, Vec3)>,
    carrying: bool,
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

/// A grenade let go: the thrower, where it lands, when it goes off, and
/// the bodies inside its radius at the throw.
#[derive(Clone, Debug)]
struct Blast {
    thrower: Place,
    at: Vec3,
    time: u32,
    inside: Vec<bool>,
}

/// A kill of an enemy: when, by whom, of whom, the shooter's feet, the
/// victim's, and the victim's team.
#[derive(Clone, Copy, Debug)]
struct Kill {
    t: u32,
    shooter: Place,
    victim: Place,
    spot: Vec3,
    at: Vec3,
    team: u8,
}

/// A shooter holding his kill spot (`zones.py`): from a kill until he
/// leaves the spot by [`SPOT_HOLD_M`] or [`RUN_APART_MS`] after his last
/// kill there. The victims' team, the line's reach, the victims'
/// bearings, and who of the victims' team is in his line now.
#[derive(Clone, Debug)]
struct KillSpot {
    shooter: Place,
    spot: Vec3,
    team: u8,
    last: u32,
    reach: f32,
    bearings: Vec<f32>,
    in_line: Vec<bool>,
}

/// A bot of the victims' team coming into a held line: when, whose,
/// which bot, in the killed lane.
#[derive(Clone, Copy, Debug)]
struct Entry {
    t: u32,
    shooter: Place,
    place: Place,
    lane: bool,
}

/// A stuck episode: its start and end, s; its kinds (the first in
/// [`STUCK_ORDER`] first); its cause; where.
#[derive(Clone, Debug)]
struct Episode {
    t0: f32,
    t1: f32,
    kinds: Vec<&'static str>,
    cause: &'static str,
    at: Vec3,
}

#[derive(Default)]
pub(super) struct OwnerTally {
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
    pub(super) camper: Option<Place>,
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

/// The range band of `d` m ([`FIRE_BANDS`]; past the last: the last).
fn band_of(d: f32) -> usize {
    FIRE_BANDS.iter().position(|&b| d <= b).unwrap_or(4)
}

/// The body's movement at a record ([`MOVES`]).
fn move_of(r: &Rec) -> usize {
    if r.air {
        4
    } else if r.speed < STILL_MPS || r.keys & WALK_KEYS == 0 {
        0
    } else if r.keys & key::SPRINT != 0 && r.keys & key::FORWARD != 0 {
        2
    } else if r.keys & (key::FORWARD | key::BACK) == 0 {
        3
    } else {
        1
    }
}

/// A bot's state by its play and step (§2d): in a designed hold; in a step
/// with a goal to walk to (a Follow-type one with its goal `goal_m` over
/// [`LEADER_FAR_M`] away).
fn status_of(play: &str, step: &str, goal_m: Option<f32>) -> (bool, bool) {
    let hold = play == "Heal" || HOLD_STEPS.contains(&step);
    let travel = !hold
        && (TRAVEL_STEPS.contains(&step)
            || (FOLLOW_STEPS.contains(&step) && goal_m.is_some_and(|m| m > LEADER_FAR_M)));
    (hold, travel)
}

/// Whether some of `him` shows from `me`'s eye within `reach` m (his chest
/// or his head), the rays cast added to `rays`.
fn in_sight(
    sight: &mapdata::RayTree,
    me: &super::Body,
    him: &super::Body,
    reach: f32,
    rays: &mut u64,
) -> bool {
    let eye = Game::eye(me);
    if (Game::chest(him) - eye).length() > reach {
        return false;
    }
    *rays += 1;
    if sight.visible(eye, Game::chest(him)) {
        return true;
    }
    *rays += 1;
    sight.visible(eye, Game::eye(him))
}

/// The run-up before a press at record `ri` (§9.2): the forward run's
/// ticks (no back key; gaps of up to [`RUN_GAP_TICKS`] with no walk key
/// bridged; a strafe or back key ends it), and the ticks with no walk key
/// right before it, within [`RUN_LOOK_TICKS`] of the press.
fn run_up(recs: &[Rec], ri: usize) -> (usize, usize) {
    let lo = ri.saturating_sub(RUN_LOOK_TICKS);
    let (mut k, mut start, mut gap) = (ri, ri, 0);
    while k > lo {
        let keys = recs[k - 1].keys;
        if keys & key::FORWARD != 0 && keys & key::BACK == 0 {
            k -= 1;
            start = k;
            gap = 0;
        } else if keys & WALK_KEYS == 0 && gap < RUN_GAP_TICKS {
            k -= 1;
            gap += 1;
        } else {
            break;
        }
    }
    let (mut k, mut still) = (start, 0);
    while k > lo && recs[k - 1].keys & WALK_KEYS == 0 {
        k -= 1;
        still += 1;
    }
    (ri - start, still)
}

/// A floor `lo`-`hi` m above `feet`, [`LEDGE_AHEAD_M`] ahead along the
/// facing `yaw` and at most [`LEDGE_SIDE_M`] to its side (`gridprobe`).
fn ledge_ahead(grid: &WalkGrid, feet: Vec3, yaw: f32, lo: f32, hi: f32) -> bool {
    let (dx, dz) = (-yaw.sin(), yaw.cos());
    let r = LEDGE_AHEAD_M.1 + 0.1;
    grid.floors_in([feet.x - r, feet.z - r, feet.x + r, feet.z + r])
        .any(|node| {
            let p = grid.position(node);
            let (x, z) = (p.x - feet.x, p.z - feet.z);
            let along = x * dx + z * dz;
            let side = (x * dz - z * dx).abs();
            let up = p.y - feet.y;
            (LEDGE_AHEAD_M.0..=LEDGE_AHEAD_M.1).contains(&along)
                && side <= LEDGE_SIDE_M
                && (lo..=hi).contains(&up)
        })
}

/// Up a link to `to` at a record `dt` s after the press (§9.3 `NOT_UP`):
/// alive, the feet within [`UP_SLACK_M`] under the landing (at or over it
/// in the air), near enough the landing for the time.
fn is_up(r: &Rec, to: Vec3, dt: f32) -> bool {
    let feet_up = if r.air {
        r.feet.y >= to.y
    } else {
        r.feet.y >= to.y - UP_SLACK_M
    };
    r.alive && feet_up && r.feet.distance_xz(to) <= (1.5 + 3.5 * (dt - 0.6)).min(4.0)
}

impl OwnerTally {
    pub(super) fn new(n: usize) -> Self {
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
    pub(super) fn tick(&mut self, game: &mut Game, bots: &Bots) {
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

    /// A kill: the shooter's held spot goes on (within [`SPOT_HOLD_M`] of
    /// it and [`RUN_APART_MS`] of his last kill there), or a new one starts
    /// with who of the victims' team is in his line now.
    fn kill_spot(&mut self, game: &Game, kill: &Kill, sight: &mapdata::RayTree, rays: &mut u64) {
        let bearing = yaw_of([kill.at.x - kill.spot.x, kill.at.z - kill.spot.z]);
        let reach = ((kill.at - kill.spot).length() + REACH_PLUS_M).max(REACH_LEAST_M);
        if let Some(held) = self.spots.iter_mut().find(|s| s.shooter == kill.shooter)
            && held.spot.distance_xz(kill.spot) <= SPOT_HOLD_M
            && kill.t <= held.last + RUN_APART_MS
            && held.team == kill.team
        {
            held.last = kill.t;
            held.reach = held.reach.max(reach);
            held.bearings.push(bearing);
            return;
        }
        self.spots.retain(|s| s.shooter != kill.shooter);
        let me = &game.bodies[kill.shooter];
        let in_line = game
            .bodies
            .iter()
            .map(|b| b.alive && b.team == kill.team && in_sight(sight, me, b, reach, rays))
            .collect();
        self.spots.push(KillSpot {
            shooter: kill.shooter,
            spot: kill.spot,
            team: kill.team,
            last: kill.t,
            reach,
            bearings: vec![bearing],
            in_line,
        });
    }

    /// Once a second: the held spots still held, and the victims' team's
    /// bots coming into their lines.
    fn held_lines(&mut self, game: &Game, sight: &mapdata::RayTree, rays: &mut u64) {
        let tick = game.tick;
        let mut spots = std::mem::take(&mut self.spots);
        spots.retain_mut(|held| {
            let me = &game.bodies[held.shooter];
            if !me.alive || me.feet.distance_xz(held.spot) > SPOT_HOLD_M || tick > held.last + RUN_APART_MS {
                return false;
            }
            for (place, b) in game.bodies.iter().enumerate() {
                if b.team != held.team {
                    continue;
                }
                let seen = b.alive && in_sight(sight, me, b, held.reach, rays);
                if seen && !held.in_line[place] {
                    let bearing = yaw_of([b.feet.x - held.spot.x, b.feet.z - held.spot.z]);
                    let lane = held
                        .bearings
                        .iter()
                        .any(|&w| wrap(w - bearing).abs().to_degrees() <= LANE_DEG);
                    self.entries.push(Entry {
                        t: tick,
                        shooter: held.shooter,
                        place,
                        lane,
                    });
                }
                held.in_line[place] = seen;
            }
            true
        });
        self.spots = spots;
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

    /// The tick of record index `k`, ms.
    fn time_of(&self, k: usize) -> u32 {
        self.first_tick + k as u32 * TICK_MS
    }

    /// A bot's note: takes, stuck watches, plays ended for no way to the
    /// goal, grenades thrown.
    pub(super) fn note(&mut self, note: &Note, game: &Game) {
        let at = |list: &mut Vec<Vec<u32>>| {
            if let Some(t) = list.get_mut(note.place) {
                t.push(note.time);
            }
        };
        if let Some(rest) = note.text.strip_prefix("takes ") {
            at(&mut self.takes);
            let mut words = rest.split_whitespace();
            let (kind, target) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
            let team = game.bodies.get(note.place).map(|b| b.team);
            if kind == "Trade"
                && let Some(team) = team
            {
                self.trades.push((note.place, team, note.time));
            }
            // A counter run: Flank or Crossfire on a shooter who holds his
            // kill spot against this bot's side.
            let on = target
                .strip_prefix("Place(")
                .and_then(|t| t.strip_suffix(')'))
                .and_then(|t| t.parse::<Place>().ok());
            if matches!(kind, "Flank" | "Crossfire")
                && let (Some(who), Some(team)) = (on, team)
                && self.spots.iter().any(|h| h.shooter == who && h.team == team)
            {
                self.counter_runs[0] += 1;
                self.counter_runs[1] += u32::from(kind == "Flank");
            }
        } else if note.text.starts_with("is stuck at") {
            at(&mut self.stuck_notes);
        } else if note.text.contains("no way to its goal") {
            at(&mut self.no_way);
        } else if let Some((aim, time)) = thrown_at(&note.text)
            && let Some(body) = game.bodies.get(note.place)
        {
            self.grenades += 1;
            let at = grenade_lands(&game.world.sight, Game::eye(body), aim);
            let inside = game
                .bodies
                .iter()
                .map(|b| b.alive && (Game::chest(b) - at).length() <= BLAST_M)
                .collect();
            self.blasts.push(Blast {
                thrower: note.place,
                at,
                time,
                inside,
            });
        }
    }

    /// The flags of press `i` of `presses` (sorted by place and time),
    /// `jumps.md` §9.2-9.3.
    fn press_flags(&self, presses: &[Press], i: usize) -> Vec<&'static str> {
        let p = &presses[i];
        let recs = &self.recs[p.place];
        let mut flags = Vec::new();
        if recs.is_empty() {
            return flags;
        }
        let ri = self.index(p.t).min(recs.len() - 1);
        let keys = recs[ri].keys;
        let forward = keys & key::FORWARD != 0;
        let walk = keys & WALK_KEYS;
        let sprint = forward && keys & key::SPRINT != 0;
        // The same bot's presses before and after this one.
        let first = presses[..i]
            .iter()
            .rposition(|q| q.place != p.place)
            .map_or(0, |k| k + 1);
        let before = &presses[first..i];
        let after = presses[i + 1..].iter().take_while(|q| q.place == p.place);
        let same_link = |q: &Press| match (p.route, q.route) {
            (Some((a, b)), Some((c, d))) => (a - c).length() <= LINK_M && (b - d).length() <= LINK_M,
            _ => false,
        };

        // Form.
        if let Some((from, to)) = p.route {
            let rise = to.y - from.y;
            if rise > REACH_M || (rise > SPRINTLESS_M && (sprint || p.carrying)) {
                flags.push("TOO_LOW");
            } else if rise > SPRINTLESS_M {
                flags.push("NO_SPRINT");
            }
            if from.y - p.feet.y > PRESSED_LOW_M {
                flags.push("PRESSED_LOW");
            }
        }
        if forward && matches!(p.by, "route" | "hop") {
            let (run, still) = run_up(recs, ri);
            if run < RUN_TICKS {
                flags.push(if still >= STILL_TICKS {
                    "STAND_TURN"
                } else {
                    "SHORT_RUNUP"
                });
            }
        }
        if forward && p.by == "route" {
            let off = (ri + 1..recs.len())
                .position(|k| recs[k].keys & key::FORWARD == 0)
                .map(|o| o + 1);
            match off {
                Some(o) if o <= RELEASED_TICKS => flags.push("FWD_RELEASED"),
                Some(o) if o <= CARRY_TICKS => flags.push("FWD_RELEASED_085"),
                _ => {}
            }
        }
        if !forward && walk & (key::BACK | key::LEFT | key::RIGHT) != 0 {
            flags.push("BACK_SIDE");
        }
        if walk == 0 {
            flags.push("IN_PLACE");
        }

        // Outcome.
        if before
            .last()
            .is_some_and(|q| p.t <= q.t + REPEAT_MS && p.feet.distance_xz(q.feet) <= REPEAT_M)
        {
            flags.push("REPEAT");
        }
        if p.route.is_some() && before.iter().any(|q| p.t <= q.t + LINK_AGAIN_MS && same_link(q)) {
            flags.push("LINK_AGAIN");
        }
        if let Some((from, to)) = p.route {
            let rise = to.y - from.y;
            let last = recs.len() - 1;
            let died = (ri..=(ri + UP_TO_TICKS).min(last)).any(|k| !recs[k].alive);
            let up_at = (ri + UP_FROM_TICKS..=(ri + UP_TO_TICKS).min(last))
                .find(|&k| is_up(&recs[k], to, (k - ri) as f32 * TICK_S));
            if up_at.is_none() && !died && ri + UP_SEEN_TICKS <= last {
                flags.push("NOT_UP");
            }
            if rise > RISE_MIN_M {
                if let Some(u) = up_at {
                    let link = (to.x - from.x).hypot(to.z - from.z).max(1e-3);
                    let on_take_off = |r: &Rec| {
                        let along = ((r.feet.x - from.x) * (to.x - from.x)
                            + (r.feet.z - from.z) * (to.z - from.z))
                            / link;
                        r.alive
                            && !r.air
                            && (r.feet.y - from.y).abs() <= UP_SLACK_M
                            && r.feet.distance_xz(from) <= DOWN_FAR_M
                            && along <= link
                    };
                    let back = (u + 1..=(ri + DOWN_TICKS).min(last)).any(|k| on_take_off(&recs[k]));
                    let again = after.clone().any(|q| {
                        q.t <= p.t + AGAIN_MS && same_link(q) && q.feet.y < to.y - UP_SLACK_M
                    });
                    if back || again {
                        flags.push("UP_THEN_DOWN");
                    }
                }
                let r = &recs[ri];
                if !r.air
                    && r.feet.y >= to.y - UP_SLACK_M.min(rise / 4.0)
                    && r.feet.distance_xz(to) <= PRESSED_UP_NEAR_M
                {
                    flags.push("PRESSED_UP");
                }
            }
        }
        if matches!(p.by, "hop" | "unstick") {
            let moved_at = |t: u32| {
                recs.get(self.index(t))
                    .filter(|r| r.alive)
                    .map(|r| r.feet.distance_xz(p.feet))
            };
            let next = after.clone().next().map(|q| q.t).filter(|&t| t <= p.t + FREED_MS);
            let stuck = self.stuck_notes[p.place]
                .iter()
                .copied()
                .find(|&t| t > p.t && t <= p.t + FREED_MS);
            let watched = match next.into_iter().chain(stuck).min() {
                Some(t) => moved_at(t).is_some_and(|m| m < FREED_M),
                None => moved_at(p.t + FREED_MS).is_some_and(|m| m < WENT_ON_M),
            };
            let stood = recs[ri].speed < STOOD_MPS
                && ri + STOOD_TICKS.1 < recs.len()
                && (ri + STOOD_TICKS.0..=ri + STOOD_TICKS.1).step_by(TPS / 2).all(|k| {
                    let r = &recs[k];
                    r.alive && r.feet.distance_xz(p.feet) <= FREED_M && r.speed < STOOD_MPS
                });
            if watched || stood {
                flags.push("NOT_FREED");
            }
        }
        // No jump came of it: not in the air 100-400 ms after, on the
        // ground at 400 ms, alive a second on.
        if ri + TPS < recs.len()
            && !(ri + 2..ri + 8).any(|k| recs[k].air)
            && recs[ri + 8].alive
            && !recs[ri + 8].air
            && recs[ri + TPS].alive
        {
            flags.push("DROPPED");
        }
        flags
    }

    /// One place's stalls at a ledge (§9.4): [missed jumps, stalls at a
    /// step, stalls]. From a sample a second.
    fn stalls(&self, place: Place, grid: &WalkGrid, presses: &[Press]) -> [u32; 3] {
        let recs = &self.recs[place];
        let samples: Vec<usize> = (0..recs.len() / TPS).map(|s| s * TPS).collect();
        let ok = |k: usize| {
            let r = &recs[k];
            r.alive
                && !r.air
                && r.keys & key::FORWARD != 0
                && r.keys & key::BACK == 0
                && r.speed < STALL_MPS
        };
        let mut out = [0u32; 3];
        let mut i = 0;
        while i < samples.len() {
            if !ok(samples[i]) {
                i += 1;
                continue;
            }
            let mut j = i;
            while j + 1 < samples.len()
                && ok(samples[j + 1])
                && recs[samples[j]].feet.distance_xz(recs[samples[j + 1]].feet) < STALL_MOVE_M
            {
                j += 1;
            }
            let (k0, k1) = (samples[i], samples[j]);
            if (k1 - k0) as f32 * TICK_S >= STALL_S {
                out[2] += 1;
                let (t0, t1) = (self.time_of(k0), self.time_of(k1));
                let pressed = presses
                    .iter()
                    .filter(|p| p.place == place && p.t + MISSED_BEFORE_MS >= t0 && p.t <= t1)
                    .map(|p| p.t)
                    .min();
                if pressed.is_none_or(|t| t > t0 + MISSED_LATE_MS) {
                    let r = &recs[k0];
                    if let Some(c) = LEDGES
                        .iter()
                        .position(|&(lo, hi)| ledge_ahead(grid, r.feet, r.yaw, lo, hi))
                    {
                        out[c] += 1;
                    }
                }
            }
            i = j + 1;
        }
        out
    }

    /// The counts, as the `owner-test` lines and the sums' `owner` object.
    pub(super) fn finish(&mut self, game: &Game) -> (String, serde_json::Value) {
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

    /// One bot's stuck episodes (`stuck.py`, `navigation.md` §2d) and its
    /// blips (stuck watches in no episode). From a sample a second of each
    /// life (a life breaks at a death, or where the body moves over 25 m
    /// in a second: a new spawn), the windows of each kind, merged when at
    /// most 2 s apart; each episode's cause. The bench has no start
    /// countdown, so nothing is left out at the start.
    fn stuck_episodes(&self, place: Place, teams: &[u8], presses: &[Press]) -> (Vec<Episode>, u32) {
        let recs = &self.recs[place];
        let secs: Vec<usize> = (0..recs.len() / TPS).map(|s| s * TPS).collect();
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
        let t_of = |k: usize| self.time_of(k) as f32 / 1000.0;
        // Ticks so far with a walk key, with aim or fire, and walking with
        // forward, back, left, right: a window's shares at once.
        let (walking, aiming) = (0, 1);
        let mut sums = vec![[0u32; 6]; recs.len() + 1];
        for (k, r) in recs.iter().enumerate() {
            let walk = r.keys & WALK_KEYS != 0;
            let mut now = sums[k];
            now[0] += u32::from(walk);
            now[1] += u32::from(r.keys & (key::AIM | key::FIRE) != 0);
            for (i, bit) in [key::FORWARD, key::BACK, key::LEFT, key::RIGHT].into_iter().enumerate() {
                now[2 + i] += u32::from(walk && r.keys & bit != 0);
            }
            sums[k + 1] = now;
        }
        let count = |k0: usize, k1: usize, i: usize| sums[k1 + 1][i] - sums[k0][i];
        let share = |k0: usize, k1: usize, i: usize| count(k0, k1, i) as f32 / (k1 - k0 + 1) as f32;
        let opposing = |k0: usize, k1: usize| {
            let n = count(k0, k1, walking);
            if n == 0 {
                0.0
            } else {
                let c = |i: usize| count(k0, k1, 2 + i);
                (c(0).min(c(1)) + c(2).min(c(3))) as f32 / n as f32
            }
        };
        let median_speed = |pts: &[usize]| {
            let mut sp: Vec<f32> = pts.iter().map(|&k| recs[k].speed).collect();
            sp.sort_by(f32::total_cmp);
            (sp[(sp.len() - 1) / 2] + sp[sp.len() / 2]) / 2.0
        };
        let stuck_line = |t0: f32, t1: f32| {
            self.stuck_notes[place]
                .iter()
                .any(|&t| (t as f32) >= t0 * 1000.0 && (t as f32) <= t1 * 1000.0)
        };
        let radius = |pts: &[usize]| {
            let n = pts.len() as f32;
            let cx = pts.iter().map(|&k| recs[k].feet.x).sum::<f32>() / n;
            let cz = pts.iter().map(|&k| recs[k].feet.z).sum::<f32>() / n;
            pts.iter()
                .map(|&k| (recs[k].feet.x - cx).hypot(recs[k].feet.z - cz))
                .fold(0.0, f32::max)
        };
        // Windows: start, end (record indexes), kind.
        let mut marks: Vec<(usize, usize, &'static str)> = Vec::new();
        for life in &lives {
            let n = life.len();
            let t_spawn = t_of(life[0]);
            // The smallest window from `i` of `secs` s, extended while
            // `ok` holds.
            let window = |i: usize, secs: f32, ok: &dyn Fn(usize) -> bool| -> Option<usize> {
                let mut j = i;
                while j < n && t_of(life[j]) - t_of(life[i]) < secs {
                    j += 1;
                }
                if j >= n || j - i + 1 < 3 || !ok(j) {
                    return None;
                }
                while j + 1 < n && ok(j + 1) {
                    j += 1;
                }
                Some(j)
            };
            for i in 0..n {
                for &(name, dmin, rmax, wmin, amax) in &STUCK_KINDS {
                    let ok = |j: usize| -> bool {
                        let pts = &life[i..=j];
                        let (k0, k1) = (pts[0], pts[pts.len() - 1]);
                        if radius(pts) > rmax || share(k0, k1, walking) < wmin {
                            return false;
                        }
                        if amax.is_some_and(|amax| share(k0, k1, aiming) >= amax) {
                            return false;
                        }
                        let held = pts.iter().filter(|&&k| recs[k].hold).count() as f32 / pts.len() as f32;
                        match name {
                            "push" => {
                                let med = median_speed(pts);
                                med <= PUSH_STILL_SPEED
                                    || (opposing(k0, k1) <= PUSH_OPPOSING_MAX
                                        && (med <= PUSH_SPEED || stuck_line(t_of(k0), t_of(k1) + 1.5)))
                            }
                            "nohead" => held < NOHEAD_HOLD_MAX,
                            _ => {
                                let (a, b) = (recs[k0].feet, recs[k1].feet);
                                let path: f32 = pts.windows(2).map(|w| recs[w[0]].feet.distance_xz(recs[w[1]].feet)).sum();
                                held == 0.0
                                    && a.distance_xz(b) <= LOOP_NET_M
                                    && (b.y - a.y).abs() < LOOP_RISE_M
                                    && path >= LOOP_PATH_M
                            }
                        }
                    };
                    if let Some(j) = window(i, dmin, &ok) {
                        marks.push((life[i], life[j], name));
                    }
                }
                if t_of(life[i]) - t_spawn < SPAWN_GRACE_S {
                    continue;
                }
                let ok_stand = |j: usize| -> bool {
                    let pts = &life[i..=j];
                    let (k0, k1) = (pts[0], pts[pts.len() - 1]);
                    radius(pts) <= STAND_R
                        && share(k0, k1, walking) <= STAND_WALK_MAX
                        && share(k0, k1, aiming) < STAND_AIM_MAX
                        && pts.iter().filter(|&&k| recs[k].travel).count() as f32
                            >= STAND_TRAVEL * pts.len() as f32
                };
                if let Some(j) = window(i, STAND_S, &ok_stand) {
                    marks.push((life[i], life[j], "stand"));
                }
            }
            // The server's stuck watch: from each firing, both ways while
            // the body keeps near where it was then, walking, not holding.
            let (lt0, lt1) = (t_of(life[0]), t_of(life[n - 1]));
            for &fired in &self.stuck_notes[place] {
                let tf = fired as f32 / 1000.0;
                if tf < lt0 || tf > lt1 {
                    continue;
                }
                let Some(m) = (0..n).find(|&m| t_of(life[m]) >= tf) else {
                    continue;
                };
                let p = recs[self.index(fired).min(recs.len() - 1)].feet;
                let fits = |m: usize| recs[life[m]].feet.distance_xz(p) <= WATCH_M && !recs[life[m]].hold;
                let walks = |a: usize, b: usize| share(life[a], life[b], walking) >= WATCH_WALK;
                if !fits(m) {
                    continue;
                }
                let (mut i, mut j) = (m, m);
                while j + 1 < n && fits(j + 1) && walks(j, j + 1) {
                    j += 1;
                }
                while i > 0 && fits(i - 1) && walks(i - 1, i) {
                    i -= 1;
                }
                let (k0, k1) = (life[i], life[j]);
                if t_of(k1) - t_of(k0) < WATCH_S || j - i + 1 < 3 {
                    continue;
                }
                let dance = share(k0, k1, aiming) >= DANCE_AIM
                    && median_speed(&life[i..=j]) > DANCE_MPS
                    && opposing(k0, k1) > DANCE_OPPOSING;
                if !dance {
                    marks.push((k0, k1, "watch"));
                }
            }
        }
        // Episodes: windows merged when at most 2 s apart.
        let order = |k: &str| STUCK_ORDER.iter().position(|o| *o == k).unwrap_or(STUCK_ORDER.len());
        marks.sort_by_key(|m| (m.0, m.1));
        let mut merged: Vec<(usize, usize, Vec<&'static str>)> = Vec::new();
        for (k0, k1, kind) in marks {
            match merged.last_mut() {
                Some(e) if t_of(k0) <= t_of(e.1) + MERGE_GAP_S => {
                    e.1 = e.1.max(k1);
                    if !e.2.contains(&kind) {
                        e.2.push(kind);
                    }
                }
                _ => merged.push((k0, k1, vec![kind])),
            }
        }
        let mut eps = Vec::new();
        for (k0, k1, mut kinds) in merged {
            kinds.sort_by_key(|k| order(k));
            let (t0, t1) = (t_of(k0), t_of(k1));
            let pts: Vec<usize> = (k0..=k1).step_by(TPS).collect();
            let within = |list: &[u32], a: f32, b: f32| {
                list.iter().filter(|&&t| (t as f32) >= a * 1000.0 && (t as f32) <= b * 1000.0).count()
            };
            // Samples with a body of one side (own or other) within reach.
            let near = |own: bool| {
                pts.iter()
                    .filter(|&&k| {
                        (0..self.recs.len()).any(|q| {
                            q != place
                                && (teams.get(q) == teams.get(place)) == own
                                && self.recs[q].get(k).is_some_and(|r| {
                                    r.alive && r.feet.distance_xz(recs[k].feet) <= MATE_NEAR_M
                                })
                        })
                    })
                    .count()
            };
            let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &k| {
                (lo.min(recs[k].feet.y), hi.max(recs[k].feet.y))
            });
            let route_jump = presses
                .iter()
                .any(|p| p.place == place && p.by == "route" && (p.t as f32) >= (t0 - 2.0) * 1000.0 && (p.t as f32) <= t1 * 1000.0);
            let cause = if within(&self.no_way[place], t0 - 1.0, t1 + 3.0) > 0 {
                "no_route"
            } else if 2 * near(true) >= pts.len() {
                "teammate"
            } else if 2 * near(false) >= pts.len() {
                "enemy"
            } else if route_jump {
                "route_jump"
            } else if hi - lo >= UP_DOWN_M {
                "ledge"
            } else if kinds.contains(&"loop") && within(&self.takes[place], t0, t1) >= 2 {
                "play_churn"
            } else if kinds[0] == "stand" {
                "stand"
            } else if share(k0, k1, aiming) >= DANCE_AIM {
                "fight"
            } else if stuck_line(t0, t1 + 1.5) {
                "wall"
            } else {
                "wall_no_line"
            };
            let at = pts.iter().fold(Vec3::ZERO, |a, &k| a + recs[k].feet) * (1.0 / pts.len() as f32);
            eps.push(Episode {
                t0,
                t1,
                kinds,
                cause,
                at,
            });
        }
        let blips = self.stuck_notes[place]
            .iter()
            .filter(|&&ms| {
                let t = ms as f32 / 1000.0;
                let alive = recs.get(self.index(ms)).is_some_and(|r| r.alive);
                alive && !eps.iter().any(|e| t >= e.t0 - 1.0 && t <= e.t1 + 1.0)
            })
            .count() as u32;
        (eps, blips)
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

/// The bots' jumps' flags, by name.
fn flags_of(json: &serde_json::Value, flag: &str) -> u64 {
    json["jump_flags"][flag].as_u64().unwrap_or(0)
}

#[test]
fn a_route_press_from_a_standstill_too_high_with_forward_let_go_is_flagged() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        // Bot 0: walks, stands 7 ticks, 3 ticks of forward, presses at
        // tick 100 for a 1.4 m rise, lets forward go 3 ticks on; its body
        // never gets up.
        let keys0 = match k {
            0..90 => key::FORWARD,
            90..97 => 0,
            97..100 => key::FORWARD,
            100 => key::FORWARD | key::JUMP,
            101..103 => key::FORWARD,
            _ => 0,
        };
        // Bot 1 (the near miss): runs into its press for a 0.9 m rise,
        // holds forward on, and is up half a second later.
        let keys1 = match k {
            100 => key::FORWARD | key::JUMP,
            0..121 => key::FORWARD,
            _ => 0,
        };
        put(game, 0, 10.0, 10.0, keys0);
        put(game, 1, 10.0, 20.0, keys1);
        if k >= 110 {
            game.bodies[1].feet = Vec3::new(10.8, 0.9, 20.0);
        }
        for body in &mut game.bodies {
            body.air_until = if (100..116).contains(&k) { game.tick + TICK_MS } else { 0 };
        }
        let mut nows = vec![bot_now(), bot_now()];
        if k == 100 {
            let low = Vec3::new(10.0, 0.0, 10.0);
            nows[0].as_mut().unwrap().jump = Some(("route", Some((low, Vec3::new(11.0, 1.4, 10.0)))));
            let low = Vec3::new(10.0, 0.0, 20.0);
            nows[1].as_mut().unwrap().jump = Some(("route", Some((low, Vec3::new(11.0, 0.9, 20.0)))));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["presses"], 2);
    for (flag, count) in [
        ("TOO_LOW", 1),
        ("NO_SPRINT", 0),
        ("STAND_TURN", 1),
        ("SHORT_RUNUP", 0),
        ("FWD_RELEASED", 1),
        ("FWD_RELEASED_085", 0),
        ("NOT_UP", 1),
        ("PRESSED_LOW", 0),
        ("DROPPED", 0),
        ("IN_PLACE", 0),
    ] {
        assert_eq!(flags_of(&json, flag), count, "{flag}: {json}");
    }
    assert_eq!(json["jumps_wrong"], 1);
    assert_eq!(json["jumps_failed"], 1);
}

#[test]
fn presses_again_at_one_place_repeat_and_one_that_never_leaves_the_ground_dropped() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 120, |k, game| {
        // Bot 0: three presses 1.5 s apart where it stands, each in the
        // air; bot 1: two presses 4 s apart (no repeat), with no key and
        // never in the air.
        let k0 = if [10, 40, 70].contains(&k) { key::JUMP | key::FORWARD } else { 0 };
        let k1 = if [10, 90].contains(&k) { key::JUMP } else { 0 };
        put(game, 0, 10.0, 10.0, k0);
        put(game, 1, 10.0, 20.0, k1);
        if [10, 40, 70].contains(&k) {
            game.bodies[0].air_until = game.tick + 600;
        }
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["presses"], 5);
    assert_eq!(flags_of(&json, "REPEAT"), 2, "{json}");
    assert_eq!(flags_of(&json, "IN_PLACE"), 2);
    assert_eq!(flags_of(&json, "DROPPED"), 2);
}

#[test]
fn a_route_link_pressed_again_and_a_hop_that_frees_nothing_are_flagged() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        let press = if [10, 110].contains(&k) { key::JUMP } else { 0 };
        put(game, 0, 10.0, 10.0, press);
        put(game, 1, 10.0, 20.0, press);
        put(game, 2, 30.0, 10.0, if k == 10 { key::JUMP } else { 0 });
        // Bot 3 (the near miss) walks off after its hop.
        let x3 = if k <= 10 { 30.0 } else { 30.0 + 0.1 * (k - 10) as f32 };
        put(game, 3, x3, 30.0, if k == 10 { key::JUMP } else { key::FORWARD });
        game.bodies[3].step_m = if k > 10 { 0.1 } else { 0.0 };
        let mut nows = vec![bot_now(); 4];
        let link = |z: f32, dx: f32| Some((Vec3::new(11.0 + dx, 0.0, z), Vec3::new(12.0 + dx, 1.0, z)));
        if k == 10 || k == 110 {
            // Bot 0 the same link twice, 5 s apart; bot 1 another 1 m on.
            nows[0].as_mut().unwrap().jump = Some(("route", link(10.0, 0.0)));
            let dx = if k == 10 { 0.0 } else { 1.0 };
            nows[1].as_mut().unwrap().jump = Some(("route", link(20.0, dx)));
        }
        if k == 10 {
            nows[2].as_mut().unwrap().jump = Some(("hop", None));
            nows[3].as_mut().unwrap().jump = Some(("hop", None));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(flags_of(&json, "LINK_AGAIN"), 1, "{json}");
    assert_eq!(flags_of(&json, "REPEAT"), 0);
    assert_eq!(flags_of(&json, "NOT_FREED"), 1);
}

#[test]
fn a_stall_at_a_ledge_with_no_press_is_a_missed_jump() {
    let grid = |rise: f32| {
        let triangles = super::ledge_triangles(rise, mapdata::WalkerBehaviour::BySlope);
        WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default())
    };
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 100, |k, game| {
        // Both push forward east at the ledge's foot; bot 1 (the near
        // miss) presses a jump 1.5 s in.
        put(game, 0, 9.6, 3.0, key::FORWARD);
        put(game, 1, 9.6, 7.0, if k == 30 { key::FORWARD | key::JUMP } else { key::FORWARD });
        for body in &mut game.bodies {
            body.yaw = -std::f32::consts::FRAC_PI_2;
        }
        vec![bot_now(), bot_now()]
    });
    let presses = tally.presses.clone();
    let high = grid(0.8);
    assert_eq!(tally.stalls(0, &high, &presses), [1, 0, 1]);
    assert_eq!(tally.stalls(1, &high, &presses), [0, 0, 1]);
    assert_eq!(tally.stalls(0, &grid(0.45), &presses), [0, 1, 1]);
    assert_eq!(tally.stalls(0, &grid(1.5), &presses), [0, 0, 1]);
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
fn fire_and_hits_count_by_the_shooters_movement_and_range() {
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1]);
    play(&mut game, &mut tally, 10, |k, game| {
        // Bot 0 fires standing at an enemy 15 m ahead and hits him once;
        // bot 2 (the near miss) fires sprinting and hits nothing.
        put(game, 0, 10.0, 10.0, key::FIRE);
        put(game, 1, 10.0, 25.0, 0);
        put(game, 2, 20.0, 10.0, key::FIRE | key::FORWARD | key::SPRINT);
        game.bodies[2].step_m = 0.3;
        put(game, 3, 20.0, 25.0, 0);
        game.bodies[0].yaw = 0.0;
        game.bodies[2].yaw = 0.0;
        if k == 5 {
            game.hit_log.push((game.tick, 0, 1));
        }
        vec![bot_now(); 4]
    });
    let class = game.bodies[0].gun.class();
    let (_, json) = tally.finish(&game);
    assert_eq!(json["fire_by_move"]["still"], 10, "{json}");
    assert_eq!(json["fire_by_move"]["sprint"], 10);
    assert_eq!(json["hits_by_move"]["still"], 1);
    assert_eq!(json["hits_by_move"]["sprint"], 0);
    assert_eq!(json["hits_by_class"][class][1], 1);
}

#[test]
fn fire_at_a_target_behind_a_wall_is_counted_and_in_line_is_not() {
    // The soak field's wall stands at x = 40 for z < 25.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1]);
    play(&mut game, &mut tally, 10, |_, game| {
        put(game, 0, 30.0, 10.0, key::FIRE);
        put(game, 1, 50.0, 10.0, 0);
        put(game, 2, 30.0, 32.0, key::FIRE);
        put(game, 3, 50.0, 32.0, 0);
        game.bodies[0].target = Some(1);
        game.bodies[2].target = Some(3);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["fire_on_target"], 20, "{json}");
    assert_eq!(json["fire_hidden"], 10);
}

#[test]
fn a_target_switch_away_while_the_hurter_is_in_line_is_counted() {
    let (mut game, mut tally) = made_up(&[0, 1, 1, 1, 0]);
    play(&mut game, &mut tally, 30, |k, game| {
        put(game, 0, 10.0, 30.0, 0);
        put(game, 4, 10.0, 35.0, 0);
        put(game, 1, 30.0, 30.0, 0);
        put(game, 2, 30.0, 35.0, 0);
        put(game, 3, 30.0, 38.0, 0);
        if k == 5 {
            game.hit_log.extend([(game.tick, 1, 0), (game.tick, 1, 4)]);
        }
        // Bot 0 goes from 2 to 3 while 1 hurts it; bot 4 (the near miss)
        // from 2 to 1, the one hurting it.
        game.bodies[0].target = Some(if k < 10 { 2 } else { 3 });
        game.bodies[4].target = Some(if k < 10 { 2 } else { 1 });
        vec![bot_now(); 5]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["target_switches"], 2, "{json}");
    assert_eq!(json["switches_off_hurter"], 1);
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
fn the_stuck_detector_finds_a_push_a_stand_and_a_watch_and_their_causes() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0, 0]);
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
        // Bots 2 and 3 stand all along with no key, 2 in a step with a
        // goal, 3 (near miss) in a hold.
        put(game, 2, 10.0, 30.0, 0);
        put(game, 3, 20.0, 30.0, 0);
        // Bot 4 walks east, dances left and right on one spot for 6 s
        // (the stuck watch fires), walks on.
        let (x4, keys4) = match k {
            0..100 => (10.0 + 0.2 * k as f32, key::FORWARD),
            100..220 if k % 2 == 0 => (30.0, key::FORWARD | key::LEFT),
            100..220 => (30.3, key::FORWARD | key::RIGHT),
            _ => (30.0 + 0.2 * (k - 220) as f32, key::FORWARD),
        };
        put(game, 4, x4, 5.0, keys4);
        game.bodies[4].step_m = 0.1;
        let mut nows = vec![bot_now(); 5];
        nows[2].as_mut().unwrap().step = Some("Going".to_owned());
        nows[3].as_mut().unwrap().step = Some("Hold".to_owned());
        nows
    });
    let fired = tally.time_of(150);
    tally.stuck_notes[4].push(fired);
    // Bot 1's watch fires in its 2 s push: a blip.
    let fired = tally.time_of(20);
    tally.stuck_notes[1].push(fired);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["stuck_by"]["push"][0], 1, "{json}");
    assert_eq!(json["stuck_by"]["stand"][0], 1, "{json}");
    assert_eq!(json["stuck_by"]["watch"][0], 1, "{json}");
    assert_eq!(json["stuck"], 3);
    assert_eq!(json["stuck_by_cause"]["wall_no_line"][0], 1);
    assert_eq!(json["stuck_by_cause"]["stand"][0], 1);
    assert_eq!(json["stuck_by_cause"]["wall"][0], 1);
    assert_eq!(json["stuck_blips"], 1);
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
fn a_grenade_at_ones_feet_hurts_the_thrower_and_mates_and_one_walked_in() {
    let (mut game, mut tally) = made_up(&[0, 0, 1, 0]);
    play(&mut game, &mut tally, 30, |_, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 12.0, 10.0, 0);
        put(game, 2, 30.0, 10.0, 0);
        put(game, 3, 30.0, 14.0, 0);
        vec![bot_now(); 4]
    });
    // Thrown at tick 30, gone off at 1000 ms already: once more ticks run.
    let note = Note {
        place: 0,
        time: game.tick,
        text: "throws a grenade at (11, 0, 10), blast at 1000".to_owned(),
    };
    tally.note(&note, &game);
    // Bot 3, a mate far off at the throw, walks in before the blast.
    game.bodies[3].feet = Vec3::new(13.0, 0.0, 10.0);
    game.tick += TICK_MS;
    tally.step(&mut game, &[bot_now(), bot_now(), bot_now(), bot_now()]);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grenades"], 1);
    assert!(json["grenade_self_dmg"].as_f64().unwrap() > 0.5, "{json}");
    assert!(json["grenade_mate_dmg"].as_f64().unwrap() > 0.3);
    assert_eq!(json["grenade_enemy_dmg"], 0.0);
    assert_eq!(json["grenade_kills"][0], 1);
    assert_eq!(json["grenade_in_radius"], serde_json::json!([1, 2, 0]));
    assert_eq!(json["grenade_walk_ins"], serde_json::json!([0, 1, 0]));
}

#[test]
fn a_stuck_bot_with_an_enemy_against_it_is_stuck_on_the_enemy() {
    // Bot 0 pushes for 5 s with enemy 2 at 1.0 m; bot 1 (near miss) pushes
    // as long with enemy 3 at 1.5 m.
    let (mut game, mut tally) = made_up(&[0, 0, 1, 1]);
    play(&mut game, &mut tally, 200, |k, game| {
        let s = k / 20;
        if s < 5 {
            put(game, 0, 10.0, 10.0, key::FORWARD);
            put(game, 1, 10.0, 30.0, key::FORWARD);
        } else {
            for (place, z) in [(0, 10.0), (1, 30.0)] {
                put(game, place, 10.0 + 0.2 * (k - 100) as f32, z, key::FORWARD);
                game.bodies[place].step_m = 0.2;
            }
        }
        put(game, 2, 11.0, 10.0, 0);
        put(game, 3, 11.5, 30.0, 0);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["stuck_by"]["push"][0], 2, "{json}");
    assert_eq!(json["stuck_by_cause"]["enemy"][0], 1, "{json}");
    assert_eq!(json["stuck_by_cause"]["wall_no_line"][0], 1, "{json}");
}

#[test]
fn the_blast_falls_off_to_its_minimum_at_ten_metres() {
    assert!((blast_damage(0.0) - BLAST_MAX).abs() < 1e-6);
    assert!((blast_damage(BLAST_M) - BLAST_MIN).abs() < 1e-6);
    assert_eq!(blast_damage(10.5), 0.0);
}

#[test]
fn kills_from_one_spot_and_a_bot_walking_into_the_held_line_are_counted() {
    // Shooter 1 holds (30, 30) and kills bots 0, 2 and 3 there 2 s apart;
    // bot 4 comes out from behind the wall (x = 40, z < 25) into his line
    // in the killed lane and dies to him. Bot 3 (the near miss) kills bot
    // 5 twice: a run of two.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 0, 0, 1]);
    play(&mut game, &mut tally, 200, |k, game| {
        put(game, 1, 30.0, 30.0, 0);
        put(game, 0, 20.0, 35.0, 0);
        put(game, 2, 21.0, 35.0, 0);
        put(game, 3, 22.0, 35.0, 0);
        put(game, 5, 60.0, 35.0, 0);
        if k >= 120 {
            put(game, 4, 20.0, 36.0, 0);
        } else {
            put(game, 4, 50.0, 10.0, 0);
        }
        for (at, shooter, victim) in [(20, 1, 0), (30, 3, 5), (60, 1, 2), (70, 3, 5), (100, 1, 3), (160, 1, 4)] {
            if k == at {
                game.kill_log.push((game.tick, shooter, victim));
            }
        }
        for (at, victim) in [(20, 0), (60, 2), (100, 3), (160, 4)] {
            if k >= at {
                game.bodies[victim].alive = false;
            }
        }
        vec![bot_now(); 6]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["camper_runs_3"], 1, "{json}");
    assert_eq!(json["camper_run_kills"], 4);
    assert_eq!(json["camper_run_longest"], 4);
    assert_eq!(json["deaths_in_row_3"], 1);
    assert_eq!(json["deaths_in_row_longest"], 4);
    assert_eq!(json["reentries"], 1);
    assert_eq!(json["reentries_fatal"], 1);
    assert_eq!(json["reentries_lane"], 1);
}

/// `ticks` ticks of `game` with every body at its spot (dead after its
/// death in `kills`, which go in the kill log at their tick), and the
/// notes given at theirs.
fn kills_and_notes(
    game: &mut Game,
    tally: &mut OwnerTally,
    ticks: usize,
    spots: &[(f32, f32)],
    kills: &[(usize, Place, Place)],
    notes: &[(usize, Place, &str)],
) {
    for k in 0..ticks {
        game.tick = (k as u32 + 1) * TICK_MS;
        for (place, &(x, z)) in spots.iter().enumerate() {
            put(game, place, x, z, 0);
        }
        for &(at, shooter, victim) in kills {
            if k == at {
                game.kill_log.push((game.tick, shooter, victim));
            }
            if k >= at {
                game.bodies[victim].alive = false;
            }
        }
        let nows = vec![bot_now(); spots.len()];
        tally.step(game, &nows);
        game.hit_log.clear();
        for &(at, place, text) in notes {
            if k == at {
                let note = Note {
                    place,
                    time: game.tick,
                    text: text.to_owned(),
                };
                tally.note(&note, game);
            }
        }
    }
}

#[test]
fn a_trade_into_the_partners_killer_and_a_trade_that_kills_him_are_counted() {
    // Bot 2 kills bot 0; bot 1 trades and dies to bot 2 in 15 s. Bot 5
    // kills bot 3; bot 4 trades and kills bot 5. Near miss: bot 4 trades
    // again with no partner's death in the 4 s before.
    let (mut game, mut tally) = made_up(&[0, 0, 1, 1, 1, 0]);
    let spots = [(10.0, 10.0), (11.0, 10.0), (20.0, 10.0), (40.0, 10.0), (41.0, 10.0), (30.0, 10.0)];
    let kills = [(20, 2, 0), (30, 5, 3), (80, 4, 5), (100, 2, 1)];
    let notes = [
        (39, 1, "takes Trade Place(0) (score 1.00: x)"),
        (39, 4, "takes Trade Place(3) (score 1.00: x)"),
        (179, 4, "takes Trade Place(3) (score 1.00: x)"),
    ];
    kills_and_notes(&mut game, &mut tally, 200, &spots, &kills, &notes);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["trades"], 3, "{json}");
    assert_eq!(json["trades_died_to_killer"], 1);
    assert_eq!(json["trade_kills"], 1);
}

#[test]
fn a_counter_run_on_a_holder_and_a_holder_killed_off_his_lane_are_counted() {
    // Shooter 1 holds (30, 30), killing bots 0 and 2 to the west; bot 3
    // takes Flank on him and kills him from the north. Shooter 5 holds
    // (60, 60), killing bot 4 to the west; bot 6 takes Crossfire on him
    // and kills him from that lane (the near miss). Near misses: Flank on
    // shooter 1 before he held, Crossfire on an exit.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 0, 0, 1, 0]);
    tally.camper = Some(1);
    let spots = [(20.0, 35.0), (30.0, 30.0), (21.0, 35.0), (30.0, 45.0), (50.0, 60.0), (60.0, 60.0), (49.0, 61.0)];
    let kills = [(20, 1, 0), (40, 1, 2), (50, 5, 4), (80, 3, 1), (90, 6, 5)];
    let notes = [
        (10, 3, "takes Flank Place(1) (score 1.00: x)"),
        (60, 3, "takes Flank Place(1) (score 1.00: x)"),
        (60, 6, "takes Crossfire Place(5) (score 1.00: x)"),
        (60, 6, "takes Crossfire Exit(0) (score 1.00: x)"),
    ];
    kills_and_notes(&mut game, &mut tally, 120, &spots, &kills, &notes);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["counter_runs"], 2, "{json}");
    assert_eq!(json["counter_runs_flank"], 1);
    assert_eq!(json["camper_kills"], 2);
    assert_eq!(json["camper_kills_off_lane"], 1);
    let camper = &json["stand_in_camper"];
    assert_eq!(camper["kills"], 2);
    assert_eq!(camper["deaths"], 1);
    assert_eq!(camper["killed_holding"], 1);
    assert_eq!(camper["killed_off_lane"], 1);
}
