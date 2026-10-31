//! Threat spots (K1, `campers.md`): where a shooter has killed the team's
//! bots from, remembered by the team so that routes keep out of his line.
//!
//! Root cause (PROVEN by code, `campers.md` K1): no memory held "a shooter
//! kills from there". A track dies with the bot, the team kept only the
//! victims' spots for 30 s, and no route or spot choice read any danger, so
//! bots walked into a known shooter's line (33 % of the entries into the
//! owner's held lines were fatal).
//!
//! A death of a bot of the team whose killer the team sensed in the 5 s
//! before ([`SENSED_MS`]) adds to a spot at the killer's place; deaths
//! within [`MERGE_M`] of one add up. A spot is *held* when it has killed
//! twice, or once with its shooter sensed on it lately. What a held spot
//! *covers* is the wedge ([`LANE_DEG`]) round the bearing of each of his
//! victims, as far as he sees from his standing eye to each region's
//! middle: his lanes.
//!
//! Routes: the plain route is searched first. If more than [`EXPOSED_M`] of
//! its first [`HORIZON_M`] runs through a lane ([`exposure`], a function of
//! the route and the ground seen, shared with whoever else needs it), a
//! local detour round the lane is searched and spliced in where it is not
//! much longer ([`super::route_filter::RouteFilter::avoiding_threat`], a soft
//! closure; the search is capped, `ThreatTuning::search_floors`); otherwise
//! the plain route is kept and the bot runs through the lane
//! (`ThreatTuning::sprint`): it crosses fast and keeps its task. A route that
//! crosses no lane costs the check and no search. Only what the team's own
//! senses and the kill feed gave goes in: nothing is read from the enemy's
//! state (no cheat).
//!
//! Best practice: Valve's per-team danger from deaths with a decay (Source
//! SDK `nav_area.cpp`), Killzone's costs on links under fire (Beij,
//! Straatman, GDC Europe 2005), Dave Mark's direct-fire threat spread only
//! to what the agent sees (Game AI Pro 2 ch. 30). Where it differs: a soft
//! closure with a fallback instead of a cost, for `Tactics::start_route`
//! takes a yes/no filter (`campers.md` K1 §3); and the plain route first,
//! the avoiding one only past an exposure (Darken's cheap test first, the
//! Close Quarters AI article's flank around the exposed nodes only).

use mapdata::Vec3;

use super::path::{Route, RouteHeight};
use super::tactics::Tactics;
use super::view::{Place, eye_height};

/// A death adds to a spot within this of it, m.
pub const MERGE_M: f32 = 8.0;
/// A killer sensed this long before the death gives the spot, ms.
pub const SENSED_MS: u32 = 5_000;
/// A spot's strength falls this much a second: one death is forgotten in
/// two minutes (Valve).
pub const DECAY_PER_S: f32 = 1.0 / 120.0;
/// Strength at which a spot is held, and with its shooter sensed on it in
/// the last [`SEEN_MS`], the least that holds it.
pub const HELD_STRENGTH: f32 = 1.5;
pub const HELD_SEEN_STRENGTH: f32 = 0.8;
/// The shooter counts as on the spot within this of it, m, for this long
/// after he was sensed there, ms.
pub const SEEN_ON_M: f32 = 8.0;
pub const SEEN_MS: u32 = 15_000;
/// His death takes this share of the strength (he usually comes back).
pub const SHOOTER_DIED_KEEPS: f32 = 0.5;
/// The reach of a spot: at least this, or the longest kill from it and this
/// more, m.
pub const REACH_FLOOR_M: f32 = 40.0;
pub const REACH_PLUS_M: f32 = 15.0;
/// A spot is cleared when a bot of the team has a line to it within this,
/// m, for [`CLEAR_MS`], and senses no enemy within [`SEEN_ON_M`] of it.
pub const CLEAR_LINE_M: f32 = 40.0;
pub const CLEAR_MS: u32 = 3_000;
/// Teammates asked whether they see a spot empty, at most.
pub const WATCHERS_ASKED: usize = 3;
/// What a spot covers is the wedge of this many degrees either side of the
/// bearing of each of its victims, and what he sees there: the lanes he
/// killed in (44 of the 51 entries after the owner's first kill from a hold
/// on Vostok Radar Station came in a lane he had already killed in,
/// `campers.md` Q2). The first version kept out all he saw within reach, and
/// searched round it at every replan: floors expanded +8 to +20 % and 4.5 to
/// 8.6 % fewer kills in the stand-in camper smoke (seeds 1-4, off against on).
pub const LANE_DEG: f32 = 20.0;
/// A route is searched again round the covered regions when more than this
/// of its first [`HORIZON_M`] runs through them, m.
pub const EXPOSED_M: f32 = 8.0;
pub const HORIZON_M: f32 = 100.0;
/// A local detour round a lane is searched from this far before its first
/// exposed stretch to this far after its last, m: a little more than
/// [`FREE_M`], so the lane itself is closed in the search.
pub const DETOUR_MARGIN_M: f32 = 13.0;
/// The route is sampled this often, m.
pub const STEP_M: f32 = 3.0;
/// Ground this near the spot is in every lane, m.
pub const LANE_CLOSE_M: f32 = 3.0;
/// An enemy sensed this recently counts as there now, ms.
pub const NOW_MS: u32 = 1_000;
/// Rays cast at the regions of a newly held spot in one tick.
pub const RAYS_PER_TICK: usize = 64;
/// Spots gone lately, kept for a job's end reason: more than go in a tick.
const GONE_KEPT: usize = 64;
/// Height of the point a ray aims at in a region's middle, m (a chest).
pub const TARGET_HEIGHT_M: f32 = 1.0;
/// Bearings of a spot's victims kept.
const BEARINGS_KEPT: usize = 16;
/// A spot is dropped under this strength.
const GONE_STRENGTH: f32 = 0.05;
/// The floors of a route's ends this near are never kept out, m: a bot in a
/// covered spawn, or going to a covered goal, still moves.
pub const FREE_M: f32 = 10.0;

/// Where a route runs through ground someone sees (a shooter's lane, `K1`;
/// the sight of a shooter who outranges a bot, fighting's X3): the metres
/// exposed in all, and each stretch as (from, to) metres along the route.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exposure {
    pub metres: f32,
    pub spans: Vec<(f32, f32)>,
}

/// The exposure of the first [`HORIZON_M`] of `route`, sampled every
/// [`STEP_M`]: a sample is exposed when `exposed` says its point is, unless
/// it lies within [`FREE_M`] of one of `free` (the ends of the way: a bot
/// in a covered spawn, or going to a covered goal, still moves). The one
/// function of a route and the ground seen; what is seen is the caller's
/// ([`Covered::exposed_at`] for the threat spots).
pub fn exposure(route: &Route, height: RouteHeight, exposed: impl Fn(Vec3) -> bool, free: &[Vec3]) -> Exposure {
    exposure_from(route, 0.0, height, exposed, free)
}

/// [`exposure`] of the [`HORIZON_M`] of `route` after `from` metres along
/// it (the way ahead of a bot that walked them), spans in metres along the
/// whole route.
pub fn exposure_from(route: &Route, from: f32, height: RouteHeight, exposed: impl Fn(Vec3) -> bool, free: &[Vec3]) -> Exposure {
    let length = route.length().min(from + HORIZON_M);
    let mut out = Exposure::default();
    let mut open: Option<f32> = None;
    let mut s = from;
    while s <= length {
        let p = route.at(s, height);
        let seen = !free.iter().any(|f| f.distance_xz(p) < FREE_M) && exposed(p);
        match (seen, open) {
            (true, None) => open = Some(s),
            (false, Some(from)) => {
                out.spans.push((from, s - STEP_M));
                open = None;
            }
            _ => {}
        }
        out.metres += if seen { STEP_M } else { 0.0 };
        s += STEP_M;
    }
    if let Some(from) = open {
        out.spans.push((from, s - STEP_M));
    }
    out
}

/// Where the first [`HORIZON_M`] of `route` runs through any of the discs
/// `(centre, radius)` (small hazards such as a trap, which sampling every
/// [`STEP_M`] would step over): each leg is tested against each disc on the
/// ground plane, so the spans are exact to the leg. Spans are in metres along
/// the route, in order; overlapping ones are merged.
pub fn hits(route: &Route, discs: &[(Vec3, f32)]) -> Exposure {
    let mut spans: Vec<(f32, f32)> = Vec::new();
    let mut along = 0.0_f32;
    for leg in route.points().windows(2) {
        let (a, b) = (leg[0].at, leg[1].at);
        let length = a.distance_xz(b);
        if along > HORIZON_M {
            break;
        }
        if length > 1e-4 {
            let (dx, dz) = ((b.x - a.x) / length, (b.z - a.z) / length);
            for &(centre, radius) in discs {
                // The leg's distance from `a` where it is within `radius` of
                // the centre: the chord of the circle on the line.
                let (rx, rz) = (centre.x - a.x, centre.z - a.z);
                let t = rx * dx + rz * dz;
                let off2 = rx * rx + rz * rz - t * t;
                if off2 > radius * radius {
                    continue;
                }
                let half = (radius * radius - off2).sqrt();
                let (from, to) = ((t - half).max(0.0), (t + half).min(length));
                if from <= to {
                    spans.push((along + from, along + to));
                }
            }
        }
        along += length;
    }
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (from, to) in spans {
        match merged.last_mut() {
            Some(last) if from <= last.1 => last.1 = last.1.max(to),
            _ => merged.push((from, to)),
        }
    }
    let metres = merged.iter().map(|(from, to)| to - from).sum();
    Exposure { metres, spans: merged }
}

#[cfg(test)]
thread_local! {
    static CHECKED: std::cell::Cell<[u64; 2]> = const { std::cell::Cell::new([0; 2]) };
}

/// A route was looked at for exposure; the bench counts them and those
/// exposed ([`EXPOSED_M`]).
#[cfg(test)]
pub fn checked(seen: &Exposure) {
    CHECKED.with(|c| {
        let mut n = c.get();
        n[0] += 1;
        n[1] += u64::from(seen.metres > EXPOSED_M);
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn checked(_: &Exposure) {}

/// The routes counted by [`checked`] so far and those exposed.
#[cfg(test)]
pub fn checks() -> [u64; 2] {
    CHECKED.with(|c| c.get())
}

/// How a route search with the covered regions kept out ended: a way round
/// (or none needed), none (the plain search follows), or one much longer than
/// the way through (which is taken).
#[derive(Clone, Copy, Debug)]
pub enum Searched {
    Round,
    NoWay,
    TooLong,
    /// The floors it may take ran out first.
    Budget,
}

#[cfg(test)]
thread_local! {
    static SEARCHES: std::cell::Cell<[u64; 4]> = const { std::cell::Cell::new([0; 4]) };
}

/// A route search was made with the covered regions kept out as a
/// preference (the bench's count).
#[cfg(test)]
pub fn searched(how: Searched) {
    SEARCHES.with(|c| {
        let mut n = c.get();
        n[how as usize] += 1;
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn searched(_: Searched) {}

#[cfg(test)]
thread_local! {
    /// By place: the last detour's time and the metres the match's detours added to their routes.
    static ROUNDED: std::cell::RefCell<std::collections::HashMap<usize, (u32, f32)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The bot at `place` took a route round the covered regions at `time`, `added_m` longer than the way it replaced (the
/// bench's count of what follows).
#[cfg(test)]
pub fn rounded(place: usize, time: u32, added_m: f32) {
    ROUNDED.with(|m| {
        let mut m = m.borrow_mut();
        let e = m.entry(place).or_insert((time, 0.0));
        *e = (time, e.1 + added_m.max(0.0));
    });
}

#[cfg(not(test))]
pub fn rounded(_: usize, _: u32, _: f32) {}

/// Whether the bot at `place` took such a route within `ms` before `time`
/// (one stamped later than `time` is another match's).
#[cfg(test)]
pub fn rounded_within(place: usize, time: u32, ms: u32) -> bool {
    ROUNDED.with(|m| m.borrow().get(&place).is_some_and(|&(t, _)| t <= time && time - t <= ms))
}

/// When the bot at `place` last took such a route, and the metres the match's detours added (the bench's count of detours
/// by team: a new stamp is a new one).
#[cfg(test)]
pub fn rounded_at(place: usize) -> Option<(u32, f32)> {
    ROUNDED.with(|m| m.borrow().get(&place).copied())
}

/// A new match: the detours of the last are forgotten. The map is the
/// thread's and the bench runs match after match on one thread, so a time
/// of an earlier match read as "within" until the new clock passed it
/// (`an-anticamper.md` §3b bench note: 665,224 detour bot-s of 2,008
/// detours).
#[cfg(test)]
pub fn new_match() {
    ROUNDED.with(|m| m.borrow_mut().clear());
}

#[cfg(test)]
thread_local! {
    static SPENT: std::cell::Cell<[u64; 3]> = const { std::cell::Cell::new([0; 3]) };
}

/// A search round the lanes expanded `floors`; the exposed route it was
/// asked for had `plain_m` in the lanes, the route kept `kept_m` (the
/// bench's count: floors a search, and the metres the detours took off).
#[cfg(test)]
pub fn spent(floors: usize, plain_m: f32, kept_m: f32) {
    SPENT.with(|c| {
        let mut n = c.get();
        n[0] += floors as u64;
        n[1] += (plain_m * 10.0).round() as u64;
        n[2] += (kept_m * 10.0).round() as u64;
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn spent(_: usize, _: f32, _: f32) {}

/// The counts of [`spent`] so far: floors, and the exposed decimetres of
/// the routes asked for and of the routes kept.
#[cfg(test)]
pub fn spents() -> [u64; 3] {
    SPENT.with(|c| c.get())
}

/// A decision of the threat answer, counted for the bench's owner tally
/// (`tests/tallies/owner/threat_answer.rs`): each switch's firings and
/// refusals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// A3: the way ahead looked at against new cover; found exposed (a
    /// replan), or clear.
    ReplanAsked,
    ReplanClear,
    /// A4: a held spot's lanes cast again (any reason).
    Recast,
    /// K2: a role given, let go (not free / no play taken), a job ended
    /// (unheld / its time / no bot / a holder down).
    JobGiven,
    JobPassedNotFree,
    JobPassedNoPlay,
    JobEndedUnheld,
    JobEndedTime,
    JobEndedNobody,
    JobEndedDown,
    /// K2: a held spot (a hold) given no job at a tick because no free bot
    /// could take a role of it (spot-ticks): its slot went to the next spot.
    JobNotOpened,
    /// A3's replans judged (the bench): the next route a new way (its
    /// length ahead or exposure off the old by over [`JUDGE_SAME_M`]) or
    /// the same; of the new ways, less exposed, and out of the lanes (at
    /// most [`EXPOSED_M`] exposed); no new route within [`JUDGE_MS`].
    ReplanNew,
    ReplanSame,
    ReplanLess,
    ReplanOut,
    ReplanNoRoute,
    /// K3: a trade not taken into a proven held line.
    TradeRefused,
    /// K3: a teammate not joined for standing in a held line; a join spot
    /// out of sight found, none found; a wait spot found.
    PairUpMateRefused,
    PairUpJoinHidden,
    PairUpJoinNone,
    PairUpWaitHidden,
    /// K3: Crossfire spots left out for lying in a held line (at its picks).
    CrossfireSpotRefused,
    /// K2: a Crossfire choke (an offer, a pick, a step) whose point is a held
    /// spot's centre while its target is not that spot: the by-point lookup
    /// took it for a job's pin (`job_line_by_id` off), the id does not.
    CrossfireChokeOnSpot,
}

/// The [`Count`]s, in order, and their names in the tally.
pub const COUNTS: [(Count, &str); 23] = [
    (Count::ReplanAsked, "replan_asked"),
    (Count::ReplanClear, "replan_clear"),
    (Count::Recast, "recast"),
    (Count::JobGiven, "job_given"),
    (Count::JobPassedNotFree, "job_passed_not_free"),
    (Count::JobPassedNoPlay, "job_passed_no_play"),
    (Count::JobEndedUnheld, "job_ended_unheld"),
    (Count::JobEndedTime, "job_ended_time"),
    (Count::JobEndedNobody, "job_ended_nobody"),
    (Count::JobEndedDown, "job_ended_down"),
    (Count::JobNotOpened, "job_not_opened"),
    (Count::ReplanNew, "replan_new"),
    (Count::ReplanSame, "replan_same"),
    (Count::ReplanLess, "replan_less"),
    (Count::ReplanOut, "replan_out"),
    (Count::ReplanNoRoute, "replan_no_route"),
    (Count::TradeRefused, "trade_refused"),
    (Count::PairUpMateRefused, "pair_up_mate_refused"),
    (Count::PairUpJoinHidden, "pair_up_join_hidden"),
    (Count::PairUpJoinNone, "pair_up_join_none"),
    (Count::PairUpWaitHidden, "pair_up_wait_hidden"),
    (Count::CrossfireSpotRefused, "crossfire_spot_refused"),
    (Count::CrossfireChokeOnSpot, "crossfire_choke_on_spot"),
];

#[cfg(test)]
thread_local! {
    static COUNTED: std::cell::Cell<[u64; 23]> = const { std::cell::Cell::new([0; 23]) };
    static JOB_COUNTED: std::cell::Cell<[[u64; 11]; 2]> = const { std::cell::Cell::new([[0; 11]; 2]) };
    static JOB_LIVES: std::cell::RefCell<Vec<(u8, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A team job's outcome (K2), counted by team for the bench's tally: how it
/// ended, who killed a holder that went down (his spot's shooter, another
/// enemy, not known), and roles given on a spot whose shooter the team had
/// not sensed for [`SEEN_MS`] (the fairness count: 0 is not required, the
/// spot is what the kill feed and the team's own eyes gave).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobTally {
    EndShooterDied,
    EndUnseen,
    EndFaded,
    EndCleared,
    EndDown,
    EndTime,
    EndNobody,
    DownByShooter,
    DownByOther,
    DownUnknown,
    GivenUnsensed,
}

/// The [`JobTally`]s, in order, and their names in the tally.
pub const JOB_TALLIES: [(JobTally, &str); 11] = [
    (JobTally::EndShooterDied, "ended_shooter_died"),
    (JobTally::EndUnseen, "ended_unseen"),
    (JobTally::EndFaded, "ended_faded"),
    (JobTally::EndCleared, "ended_cleared"),
    (JobTally::EndDown, "ended_holder_down"),
    (JobTally::EndTime, "ended_time"),
    (JobTally::EndNobody, "ended_nobody"),
    (JobTally::DownByShooter, "down_by_shooter"),
    (JobTally::DownByOther, "down_by_other"),
    (JobTally::DownUnknown, "down_by_unknown"),
    (JobTally::GivenUnsensed, "given_unsensed"),
];

impl JobTally {
    /// A job end's tally.
    pub fn of_end(end: JobEnd) -> Self {
        match end {
            JobEnd::ShooterDied => JobTally::EndShooterDied,
            JobEnd::Unseen => JobTally::EndUnseen,
            JobEnd::Faded => JobTally::EndFaded,
            JobEnd::Cleared => JobTally::EndCleared,
            JobEnd::HolderDown { .. } => JobTally::EndDown,
            JobEnd::Time => JobTally::EndTime,
            JobEnd::Nobody => JobTally::EndNobody,
        }
    }
}

/// Team `team`'s job outcome `t` (the bench's count; nothing live).
#[cfg(test)]
pub fn job_tally(team: u8, t: JobTally) {
    JOB_COUNTED.with(|cell| {
        let mut all = cell.get();
        all[usize::from(team & 1)][t as usize] += 1;
        cell.set(all);
    });
}

#[cfg(not(test))]
pub fn job_tally(_: u8, _: JobTally) {}

/// A job of team `team` lived `ms` (the bench's count; nothing live).
#[cfg(test)]
pub fn job_life(team: u8, ms: u32) {
    JOB_LIVES.with(|lives| lives.borrow_mut().push((team & 1, ms)));
}

#[cfg(not(test))]
pub fn job_life(_: u8, _: u32) {}

/// The [`JobTally`]s so far, by team and [`JOB_TALLIES`] order.
#[cfg(test)]
pub fn job_tallied() -> [[u64; 11]; 2] {
    JOB_COUNTED.with(|cell| cell.get())
}

/// The jobs' lives so far (team, ms), from the `from`th on.
#[cfg(test)]
pub fn job_lives(from: usize) -> Vec<(u8, u32)> {
    JOB_LIVES.with(|lives| lives.borrow().get(from..).map(<[_]>::to_vec).unwrap_or_default())
}

/// `c` happened `n` times (the bench's count; nothing live).
#[cfg(test)]
pub fn count_n(c: Count, n: u64) {
    COUNTED.with(|cell| {
        let mut all = cell.get();
        all[c as usize] += n;
        cell.set(all);
    });
}

#[cfg(not(test))]
pub fn count_n(_: Count, _: u64) {}

/// `c` happened once.
pub fn count(c: Count) {
    count_n(c, 1);
}

/// The [`Count`]s so far, by [`COUNTS`] order.
#[cfg(test)]
pub fn counted() -> [u64; 23] {
    COUNTED.with(|cell| cell.get())
}

/// How a job role's play was offered to its bot at a poll (the bench's count; nothing live): the class of a role
/// let go unplayed (`PassedWhy::NoPlay`), design-p1c §1's opening of a Breach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// The held spot has no spot for the role (no flank spot round it; no pin spot fine for the bot).
    NoSpot,
    /// The play on the spot is set aside for the bot (it failed lately).
    SetAside,
    /// The bot runs dry (a fight play is not offered).
    Dry,
    /// Under the play's health gate.
    LowHealth,
    /// Offered.
    Offered,
}

impl Offer {
    pub fn name(self) -> &'static str {
        match self {
            Offer::NoSpot => "no_spot",
            Offer::SetAside => "set_aside",
            Offer::Dry => "dry",
            Offer::LowHealth => "low_health",
            Offer::Offered => "offered",
        }
    }
}

#[cfg(test)]
fn role_name(role: Role) -> &'static str {
    match role {
        Role::Flanker => "flanker",
        Role::Pinner => "pinner",
    }
}

#[cfg(test)]
thread_local! {
    /// The last offer to each role's bot since the role was given: (place, role) -> offer.
    static OFFERS: std::cell::RefCell<std::collections::HashMap<(Place, bool), Offer>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// Why the last no-spot offer had none: (place, role) -> why.
    static NO_SPOT_WHY: std::cell::RefCell<std::collections::HashMap<(Place, bool), &'static str>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// By class name: the polls of each offer (`polls_<role>_<offer>`) and the roles let go unplayed by their last
    /// offer (`noplay_<role>_<class>`).
    static OFFER_COUNTS: std::cell::RefCell<std::collections::BTreeMap<String, u64>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

#[cfg(test)]
fn offer_count(key: String) {
    OFFER_COUNTS.with(|m| *m.borrow_mut().entry(key).or_default() += 1);
}

/// A poll of `place`'s play for its job `role`, and how it was offered.
#[cfg(test)]
pub fn offered(place: Place, role: Role, offer: Offer) {
    offer_count(format!("polls_{}_{}", role_name(role), offer.name()));
    OFFERS.with(|m| m.borrow_mut().insert((place, role == Role::Flanker), offer));
}

/// Why the last no-spot offer to `place`'s `role` had no spot (`why`: the test that left the most spots out, or
/// "none_sampled"), counted by poll (`nospot_<role>_<why>`) and kept for the role's class if it is let go.
#[cfg(test)]
pub fn no_spot_why(place: Place, role: Role, why: &'static str) {
    offer_count(format!("nospot_{}_{why}", role_name(role)));
    NO_SPOT_WHY.with(|m| m.borrow_mut().insert((place, role == Role::Flanker), why));
}

#[cfg(not(test))]
pub fn no_spot_why(_: Place, _: Role, _: &'static str) {}

/// Where a no-spot held spot's point lies (the bench's class, by poll, `nospot_<role>_centre_<where>`): on a floor
/// (within 0.6 m: a place he could stand), over the nearest floor within 30 m by 0.6-3 m or more (a mean between
/// storeys, a sill, a roof edge), under one, or with no floor near.
#[cfg(test)]
pub fn no_spot_centre(role: Role, grid: &mapdata::WalkGrid, at: Vec3) -> &'static str {
    let place = if grid.locate(at, 0.6).is_some() {
        "on_floor"
    } else {
        match grid.locate(at, 30.0).map(|n| at.y - grid.position(n).y) {
            None => "no_floor_near",
            Some(dy) if dy >= 3.0 => "over_floor_3m_plus",
            Some(dy) if dy > 0.0 => "over_floor_under_3m",
            Some(_) => "under_floor",
        }
    };
    offer_count(format!("nospot_{}_centre_{place}", role_name(role)));
    place
}

/// The tally of the first test each spot failed, as [`no_spot_why`] names it: the test that left the most out.
pub fn most_failed(fails: &[&'static str]) -> &'static str {
    let mut by: Vec<(&'static str, usize)> = Vec::new();
    for &f in fails {
        match by.iter_mut().find(|(n, _)| *n == f) {
            Some((_, c)) => *c += 1,
            None => by.push((f, 1)),
        }
    }
    by.into_iter().max_by_key(|&(_, c)| c).map_or("none_sampled", |(n, _)| n)
}

#[cfg(not(test))]
pub fn offered(_: Place, _: Role, _: Offer) {}

/// `place` was given `role`: its offers start again.
#[cfg(test)]
pub fn offers_reset(place: Place, role: Role) {
    OFFERS.with(|m| m.borrow_mut().remove(&(place, role == Role::Flanker)));
    NO_SPOT_WHY.with(|m| m.borrow_mut().remove(&(place, role == Role::Flanker)));
}

#[cfg(not(test))]
pub fn offers_reset(_: Place, _: Role) {}

/// `place` let `role` go without taking its play: counted by the role's last offer since it was given (none: its
/// play was never polled), and when it was offered, by the play the bot runs instead (`running`, "none").
#[cfg(test)]
pub fn passed_unplayed(place: Place, role: Role, running: Option<&str>) {
    let last = OFFERS.with(|m| m.borrow_mut().remove(&(place, role == Role::Flanker)));
    let why = NO_SPOT_WHY.with(|m| m.borrow_mut().remove(&(place, role == Role::Flanker)));
    let class = match last {
        None => "not_polled".to_string(),
        Some(Offer::Offered) => format!("outscored_by_{}", running.unwrap_or("none")),
        Some(Offer::NoSpot) => format!("no_spot_{}", why.unwrap_or("unknown")),
        Some(offer) => offer.name().to_string(),
    };
    offer_count(format!("noplay_{}_{class}", role_name(role)));
}

#[cfg(not(test))]
pub fn passed_unplayed(_: Place, _: Role, _: Option<&str>) {}

/// The offer counts so far, by name.
#[cfg(test)]
pub fn offer_counts() -> std::collections::BTreeMap<String, u64> {
    OFFER_COUNTS.with(|m| m.borrow().clone())
}

/// An A3 replan asked for (the bench's judgment of it): the way ahead it
/// replaced, exposed and left (m), that route's birth, and when.
#[derive(Clone, Copy, Debug)]
pub struct Asked {
    pub exposed_m: f32,
    pub left_m: f32,
    pub born: u32,
    pub time: u32,
}

/// A replan's next route within this, ms, or none came (a search can go on
/// over polls).
pub const JUDGE_MS: u32 = 5_000;
/// A next route this near the old in length ahead and exposure is the same way, m.
pub const JUDGE_SAME_M: f32 = 1.0;

/// An A3 replan's next route, `exposed_m` exposed and `left_m` long ahead,
/// against the way ahead it replaced.
pub fn judged(asked: Asked, exposed_m: f32, left_m: f32) {
    let same = (left_m - asked.left_m).abs() <= JUDGE_SAME_M && (exposed_m - asked.exposed_m).abs() <= JUDGE_SAME_M;
    if same {
        count(Count::ReplanSame);
        return;
    }
    count(Count::ReplanNew);
    if exposed_m < asked.exposed_m {
        count(Count::ReplanLess);
    }
    if exposed_m <= EXPOSED_M {
        count(Count::ReplanOut);
    }
}

/// A job event's count.
pub fn count_job(event: JobEvent) {
    count(match event {
        JobEvent::Given { .. } => Count::JobGiven,
        JobEvent::Passed { why: PassedWhy::NotFree, .. } => Count::JobPassedNotFree,
        JobEvent::Passed { why: PassedWhy::NoPlay, .. } => Count::JobPassedNoPlay,
        JobEvent::Ended { end: JobEnd::Time, .. } => Count::JobEndedTime,
        JobEvent::Ended { end: JobEnd::Nobody, .. } => Count::JobEndedNobody,
        JobEvent::Ended { end: JobEnd::HolderDown { .. }, .. } => Count::JobEndedDown,
        JobEvent::Ended { .. } => Count::JobEndedUnheld,
    });
}

/// The searches counted by [`searched`] so far, by [`Searched`] in order.
#[cfg(test)]
pub fn searches() -> [u64; 4] {
    SEARCHES.with(|c| c.get())
}

/// What the spots read of the map: what is seen from where, and the
/// regions' middles.
pub trait Eyes {
    /// A sight line: what a teammate sees (the clear test).
    fn sees(&self, from: Vec3, to: Vec3) -> bool;
    /// Whether a shooter's bullet from `from` still hurts at `to`: what his
    /// lanes cover (the casts). [`FireEyes`] answers it by the team's cover
    /// rule (#705: thin cover no longer hides a lane); any other `Eyes`, by a
    /// sight line, as before.
    fn shot_reaches(&self, from: Vec3, to: Vec3) -> bool {
        self.sees(from, to)
    }
    /// Every region and the point at its middle.
    fn region_points(&self) -> Vec<(u32, Vec3)>;
}

/// The spots' eyes with the team's cover rule (`Tuning::cover.rule()`, #705)
/// for the lane casts (`ThreatTuning::lanes_pierce`): a lane reaches through
/// what the team's bullets pierce (a crate, a car, a thin wall), as the bots'
/// own hide spots are judged (`SpotQuery::out_of_fire_of`).
pub struct FireEyes<'a> {
    pub tactics: &'a Tactics,
    pub cover: Option<super::tuning::CoverRule>,
}

impl Eyes for FireEyes<'_> {
    fn sees(&self, from: Vec3, to: Vec3) -> bool {
        self.tactics.sees(from, to)
    }

    fn shot_reaches(&self, from: Vec3, to: Vec3) -> bool {
        self.tactics.shot_reaches(self.cover, from, to)
    }

    fn region_points(&self) -> Vec<(u32, Vec3)> {
        Eyes::region_points(self.tactics)
    }
}

impl Eyes for Tactics {
    fn sees(&self, from: Vec3, to: Vec3) -> bool {
        Tactics::sees(self, from, to)
    }

    fn region_points(&self) -> Vec<(u32, Vec3)> {
        let regions = self.regions();
        (0..regions.len() as u32)
            .map(|r| (r, self.world.grid.position(regions.middle(r))))
            .collect()
    }
}

/// The covered regions kept open for a route from floor `from` to floor
/// `to`: those with a middle within [`FREE_M`] of an end, and the regions
/// of the two ends.
pub fn free_regions(covered: &Covered, tactics: &Tactics, from: mapdata::NodeId, to: mapdata::NodeId) -> Vec<u32> {
    let (grid, regions) = (&tactics.world.grid, tactics.regions());
    let ends = [grid.position(from), grid.position(to)];
    let mut free: Vec<u32> = covered
        .regions()
        .iter()
        .copied()
        .filter(|&r| {
            let middle = grid.position(regions.middle(r));
            ends.iter().any(|end| end.distance_xz(middle) < FREE_M)
        })
        .collect();
    free.extend([regions.of(from), regions.of(to)]);
    free
}

/// Whether `p` lies within [`LANE_DEG`] of one of `bearings` seen from
/// `at`.
fn in_lane(bearings: &[f32], at: Vec3, p: Vec3) -> bool {
    // His own ground has no bearing.
    if p.distance_xz(at) < LANE_CLOSE_M {
        return true;
    }
    let bearing = bearing(at, p);
    bearings.iter().any(|&b| angle_between(bearing, b) <= LANE_DEG.to_radians())
}

/// The yaw from `from` to `to` on the ground plane.
fn bearing(from: Vec3, to: Vec3) -> f32 {
    (to.x - from.x).atan2(to.z - from.z)
}

/// The angle between two yaws, radians, 0 to pi.
fn angle_between(a: f32, b: f32) -> f32 {
    ((a - b + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs()
}

/// A spot is the shooter's area, kills within this of its centre merged,
/// and he is on it while sensed within it, m (`ThreatTuning::area`). A
/// holder works a circle about 8 m across, p90 7.9-8.0 m from his hold's
/// centre (`an-anticamper.md` §8.4, owner and 8,474 live holds: PROVEN);
/// 10 m is the hold definition those were measured with.
pub const AREA_M: f32 = 10.0;
/// A spot's lanes are cast again when its centre moved this far from where
/// they were cast from, m: at 20 m a 3 m shift turns a bearing by about 9
/// degrees, half a lane (`LANE_DEG`). GUESS.
pub const RECAST_MOVED_M: f32 = 3.0;
/// Spot events kept between two commander ticks, at most (the notes).
const EVENTS_KEPT: usize = 64;
/// A sensed sniper's spot follows him once he moved this far from where its lanes were cast, m (the lanes of a
/// shooter who sees as far as he does turn little with a few metres; at 10 m the first bench cast them 1,350 times a match).
pub const SNIPER_MOVED_M: f32 = 25.0;
/// The bearings of a sniper's spot: this many lanes round, each [`LANE_DEG`] either side: all he sees.
const SNIPER_LANES: usize = 9;
/// A remembered shooter is on his exact spot this long after he was last sensed, ms: after a kill 51 % of live snipers
/// are still on it at 10 s, 22 % at 30 s (`human25/reveal_decay.txt`, 145 live recordings of 12+ players). PROVEN.
pub const AREA_EXACT_MS: u32 = 10_000;
/// ... and by this long his area has widened to [`AREA_WIDE_M`], ms (22 % still on the spot at 30 s). LIKELY.
pub const AREA_WIDE_MS: u32 = 30_000;
/// The area a moved sniper holds again in, m: 85 % of the live snipers who move after a kill and hold again in the same
/// life do it within 20 m (median 3 m; `human25`). LIKELY.
pub const AREA_WIDE_M: f32 = 20.0;

/// Where a remembered shooter may be now, as a distance from where the team last sensed him, m, at `age_ms` since then:
/// his exact spot for [`AREA_EXACT_MS`], then widening evenly to [`AREA_WIDE_M`] at [`AREA_WIDE_MS`], that after. Not
/// sensed since his spot was made (None): the whole area. The one value of a spot's area ([`HeldLane::area_m`]).
pub fn area_m(age_ms: Option<u32>) -> f32 {
    let Some(age) = age_ms else {
        return AREA_WIDE_M;
    };
    let widening = age.saturating_sub(AREA_EXACT_MS) as f32 / (AREA_WIDE_MS - AREA_EXACT_MS) as f32;
    AREA_WIDE_M * widening.min(1.0)
}

/// How the team last sensed a sniper (`know::Source::own`): its own sight or hearing, or a teammate's word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    Seen,
    Heard,
    Told,
}

impl Sense {
    /// The team's own sense of `source`; none for the HUD or a cheat.
    pub fn of(source: super::know::Source) -> Option<Self> {
        use super::know::Source;
        match source {
            Source::Seen => Some(Self::Seen),
            Source::Heard => Some(Self::Heard),
            Source::Told { .. } => Some(Self::Told),
            Source::Hud | Source::Cheat(_) => None,
        }
    }

    /// Its index in the counters (seen, heard, told).
    pub fn index(self) -> usize {
        match self {
            Self::Seen => 0,
            Self::Heard => 1,
            Self::Told => 2,
        }
    }
}

/// Which held spots a consumer of [`Covered`] (the routes, K1's sprint, the lane smoke) reads: all, or (its
/// `SniperMemory` switch off) those of kills only, the sensed snipers' spots then read by the others alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptOut {
    All,
    KillsOnly,
}

/// How a sensed sniper's spot is kept (`ThreatTuning::sniper_memory`, parsed once a call): the strength each fresh sense
/// of him fills it to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SniperLife {
    /// As #836 made it: held while he is sensed within [`SEEN_MS`] (each sense fills it to [`HELD_STRENGTH`]).
    WhileSensed,
    /// The owner's area memory: each sense fills it to [`HELD_STRENGTH`] and `memory_ms` of [`DECAY_PER_S`] more, so it
    /// is held that long after the last one; its place moves only with a fresh sense.
    Remembered { memory_ms: u32 },
}

impl SniperLife {
    fn fill(self) -> f32 {
        match self {
            Self::WhileSensed => HELD_STRENGTH,
            Self::Remembered { memory_ms } => HELD_STRENGTH + memory_ms as f32 / 1000.0 * DECAY_PER_S,
        }
    }
}

/// What a spot is: the two models (`ThreatTuning::area`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpotModel {
    /// As it was (c3dc7241): the first kill's place; kills within
    /// [`MERGE_M`] add to the first such spot; each lane's apex is that
    /// place, its bearing taken from its own kill's place; the shooter is on
    /// it within [`SEEN_ON_M`].
    Point,
    /// The shooter's area (§8.4): kills within `m` add to the nearest such
    /// spot, its place is their middle, each lane's bearing is from there,
    /// and the shooter is on it within `m`.
    Area { m: f32 },
}

/// How the team's spots are kept, from `ThreatTuning` (parsed once a call).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotRules {
    pub model: SpotModel,
    /// Lanes cast again on a new bearing, the old cover kept meanwhile.
    pub recast: bool,
    /// How a sensed sniper's spot is kept.
    pub sniper: SniperLife,
}

impl SpotRules {
    pub fn of(tuning: &crate::v026g::bots::tuning::ThreatTuning) -> Self {
        Self {
            model: if tuning.area { SpotModel::Area { m: AREA_M } } else { SpotModel::Point },
            recast: tuning.recast,
            sniper: match tuning.sniper_memory {
                Some(m) => SniperLife::Remembered { memory_ms: (m.memory_s.max(0.0) * 1000.0) as u32 },
                None => SniperLife::WhileSensed,
            },
        }
    }

    /// Kills within this of a spot add to it, m.
    fn merge_m(self) -> f32 {
        match self.model {
            SpotModel::Point => MERGE_M,
            SpotModel::Area { m } => m,
        }
    }

    /// The distance within which the shooter is on a spot, m.
    fn on_m(self) -> f32 {
        match self.model {
            SpotModel::Point => SEEN_ON_M,
            SpotModel::Area { m } => m,
        }
    }

    /// Whether the spot follows its kills (its place their middle).
    fn recentres(self) -> bool {
        matches!(self.model, SpotModel::Area { .. })
    }
}

impl Default for SpotRules {
    fn default() -> Self {
        Self::of(&crate::v026g::bots::tuning::ThreatTuning::default())
    }
}

/// A kill a spot counts: where the shooter was sensed, where the victim
/// died.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Kill {
    from: Vec3,
    victim: Vec3,
}

/// What a cast was made from: the eye's ground point, the bearings and the
/// reach.
#[derive(Clone, Debug, PartialEq)]
struct Lanes {
    from: Vec3,
    bearings: Vec<f32>,
}

/// Where a spot's lanes stand: one state, and the cover there is in each.
#[derive(Clone, Debug, PartialEq)]
enum Cast {
    /// To be cast at its next tick held; the cover of the cast before kept
    /// meanwhile (`SpotRules::recast`), if any.
    Due { kept: Option<Vec<u32>> },
    /// Rays going out, [`RAYS_PER_TICK`] a tick: the regions left, those
    /// seen so far, what the lanes are cast from, the cover kept meanwhile.
    Casting {
        todo: Vec<(u32, Vec3)>,
        seen: Vec<u32>,
        lanes: Lanes,
        kept: Option<Vec<u32>>,
    },
    /// All cast: the regions of his lanes, and what they were cast from.
    Done { cover: Vec<u32>, lanes: Lanes },
}

impl Cast {
    fn cover(&self) -> Option<&[u32]> {
        match self {
            Cast::Done { cover, .. } => Some(cover),
            Cast::Due { kept } | Cast::Casting { kept, .. } => kept.as_deref(),
        }
    }

    fn lanes(&self) -> Option<&Lanes> {
        match self {
            Cast::Done { lanes, .. } | Cast::Casting { lanes, .. } => Some(lanes),
            Cast::Due { .. } => None,
        }
    }

    /// To be cast again; the cover there is now kept when `keep`.
    fn again(&mut self, keep: bool) {
        let kept = if keep { self.cover().map(<[u32]>::to_vec) } else { None };
        *self = Cast::Due { kept };
    }
}

/// What a spot is made of: one state, and what each knows.
#[derive(Clone, Debug, PartialEq)]
enum SpotSource {
    /// Kills of the team's bots from there ([`ThreatSpots::died`]): those kept, and when the team last sensed the
    /// shooter on it since his last death (none: he died since).
    Kills { kills: Vec<Kill>, seen: Option<u32> },
    /// A sniper the team sensed ([`ThreatSpots::sniper_sensed`]), no kill behind it: lanes all round him; how and when
    /// the team last sensed him (his death lets the spot go, so there always is a last time).
    Sensed { how: Sense, last: u32 },
}

/// One place a shooter killed from, or a sniper was sensed at.
#[derive(Clone, Debug)]
pub struct Spot {
    /// The team's number for it, for the notes and the job's slots.
    pub id: u32,
    /// Its centre ([`SpotModel::Area`]) or its first kill's place; a sensed sniper's: where he was last sensed.
    pub at: Vec3,
    strength: f32,
    stamp: u32,
    pub shooter: Place,
    pub reach_m: f32,
    empty_since: Option<u32>,
    cast: Cast,
    /// Held at the last commander tick (the Held note is a change of it).
    held_then: bool,
    source: SpotSource,
}

impl Spot {
    fn strength_at(&self, now: u32) -> f32 {
        let s = now.saturating_sub(self.stamp) as f32 / 1000.0;
        (self.strength - s * DECAY_PER_S).max(0.0)
    }

    /// Whether the spot is held at `now`.
    pub fn held(&self, now: u32) -> bool {
        let strength = self.strength_at(now);
        strength >= HELD_STRENGTH
            || (strength >= HELD_SEEN_STRENGTH
                && self.sensed().is_some_and(|t| now.saturating_sub(t) <= SEEN_MS))
    }

    /// When the team last sensed its shooter here (seen, heard or told), if since it was made and since his death.
    pub fn sensed(&self) -> Option<u32> {
        match self.source {
            SpotSource::Kills { seen, .. } => seen,
            SpotSource::Sensed { last, .. } => Some(last),
        }
    }

    /// A sensed sniper's spot ([`SpotSource::Sensed`]): the jobs and the plays that read the held lines do not see it.
    fn is_sensed(&self) -> bool {
        matches!(self.source, SpotSource::Sensed { .. })
    }

    /// The kills it counts (a sensed sniper's: none).
    fn kill_count(&self) -> usize {
        match &self.source {
            SpotSource::Kills { kills, .. } => kills.len(),
            SpotSource::Sensed { .. } => 0,
        }
    }

    fn kills(&self) -> &[Kill] {
        match &self.source {
            SpotSource::Kills { kills, .. } => kills,
            SpotSource::Sensed { .. } => &[],
        }
    }

    /// The regions of his line, once all cast (or the last cast's, kept
    /// while a new one goes on).
    pub fn covered(&self) -> Option<&[u32]> {
        self.cast.cover()
    }

    /// Each lane as (its apex, the victim's bearing): the apex is the
    /// spot's place; the bearing is from there ([`SpotModel::Area`]) or, as
    /// it was, from the kill's own place ([`SpotModel::Point`], c3dc7241's
    /// `in_lane(&bearings, s.at, p)` with each bearing from `shooter_at`).
    /// A sensed sniper's: [`SNIPER_LANES`] bearings evenly round his place, all he sees.
    fn lanes(&self, rules: SpotRules) -> Vec<(Vec3, f32)> {
        match &self.source {
            SpotSource::Kills { kills, .. } => kills
                .iter()
                .map(|k| {
                    let from = if rules.recentres() { self.at } else { k.from };
                    (self.at, bearing(from, k.victim))
                })
                .collect(),
            SpotSource::Sensed { .. } => (0..SNIPER_LANES)
                .map(|k| (self.at, k as f32 / SNIPER_LANES as f32 * std::f32::consts::TAU))
                .collect(),
        }
    }

    /// The way he faces: the mean of his victims' bearings from the centre (a sensed sniper's: none, 0).
    pub fn facing(&self) -> f32 {
        let (sx, sz) = self.kills().iter().fold((0.0_f32, 0.0_f32), |(x, z), k| {
            let b = bearing(self.at, k.victim);
            (x + b.sin(), z + b.cos())
        });
        sx.atan2(sz)
    }

    /// The middle of its kills' places.
    fn centre(&self) -> Vec3 {
        let kills = self.kills();
        let n = kills.len().max(1) as f32;
        let (x, y, z) = kills
            .iter()
            .fold((0.0, 0.0, 0.0), |(x, y, z), k| (x + k.from.x, y + k.from.y, z + k.from.z));
        Vec3::new(x / n, y / n, z / n)
    }
}

/// A floor's region.
pub type RegionOf = std::sync::Arc<dyn Fn(mapdata::NodeId) -> u32 + Send + Sync>;

/// The regions (and so the floors of them) the team's held spots see, shared
/// with the bots of the team. A bit by region, not by floor: a set is made
/// again whenever a spot is cast, cleared or lets go (36 times a match in
/// the bench), and a bit by floor cost 8-11 ms of every bot-second.
#[derive(Clone, Default)]
pub struct Covered {
    /// Sorted.
    regions: Vec<u32>,
    /// By region, whether it is covered.
    bits: Vec<bool>,
    region_of: Option<RegionOf>,
}

impl std::fmt::Debug for Covered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Covered").field("regions", &self.regions.len()).finish()
    }
}

impl Covered {
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn region(&self, region: u32) -> bool {
        self.bits.get(region as usize).copied().unwrap_or(false)
    }

    pub fn floor(&self, node: mapdata::NodeId) -> bool {
        self.region_of.as_ref().is_some_and(|of| self.region(of(node)))
    }

    /// Whether the ground at `p` is covered, on `grid`.
    pub fn exposed_at(&self, grid: &mapdata::WalkGrid, p: Vec3) -> bool {
        grid.locate(p, 1.5).is_some_and(|n| self.floor(n))
    }

    /// The covered regions.
    pub fn regions(&self) -> &[u32] {
        &self.regions
    }

    /// The covered regions of `regions` out of `total`, the region of a floor
    /// given by `region_of`.
    pub fn build(mut regions: Vec<u32>, total: usize, region_of: RegionOf) -> Self {
        regions.sort_unstable();
        regions.dedup();
        let mut bits = vec![false; total];
        for &r in &regions {
            if let Some(bit) = bits.get_mut(r as usize) {
                *bit = true;
            }
        }
        Covered { regions, bits, region_of: Some(region_of) }
    }
}


/// What happened to a spot, for its note (F4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpotNote {
    /// A first kill from a new place.
    Made,
    /// Another kill added to it, its strength now.
    Added { strength: f32 },
    /// Held from now.
    Held,
    /// Its lanes are to be cast again, and why.
    Recast { why: &'static str },
    /// All its lanes cast: the regions they cover.
    Cast { regions: usize },
    /// Seen empty by a teammate for [`CLEAR_MS`].
    Cleared,
    /// Its strength ran out.
    Decayed,
    /// Its shooter died (it keeps [`SHOOTER_DIED_KEEPS`]).
    ShooterDied,
}

/// A spot's event: when, which spot, its shooter and place then, what.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotEvent {
    pub time: u32,
    pub id: u32,
    pub shooter: Place,
    pub at: Vec3,
    pub note: SpotNote,
}

impl std::fmt::Display for SpotEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let what = match self.note {
            SpotNote::Made => "made".to_string(),
            SpotNote::Added { strength } => format!("a kill added (strength {strength:.2})"),
            SpotNote::Held => "held".to_string(),
            SpotNote::Recast { why } => format!("cast again ({why})"),
            SpotNote::Cast { regions } => format!("cast: {regions} regions"),
            SpotNote::Cleared => "cleared, seen empty".to_string(),
            SpotNote::Decayed => "let go, decayed".to_string(),
            SpotNote::ShooterDied => "its shooter died".to_string(),
        };
        write!(
            f,
            "threat spot {} of shooter {} at ({:.1}, {:.1}, {:.1}): {what}",
            self.id, self.shooter, self.at.x, self.at.y, self.at.z
        )
    }
}

/// A team's threat spots.
#[derive(Clone, Debug, Default)]
pub struct ThreatSpots {
    spots: Vec<Spot>,
    /// Changes when the set of regions kept out may have.
    pub version: u32,
    /// The next spot's number.
    next_id: u32,
    /// The spots gone lately and why (a job's end reads it, K2).
    gone: Vec<(u32, JobEnd)>,
    /// What happened since the commander last took them ([`Self::take_events`]).
    events: Vec<SpotEvent>,
    /// [measure] What the sensed snipers' spots did.
    sniper_lanes: SniperLaneStats,
}

/// [measure] What a team's sniper spots did (the bench's owner counts; nothing decides by them): spots made, lanes cast again
/// because he moved, casts done and the regions they cover in all, and the commander ticks a sniper spot was held; the
/// spots made by the sense that made them (seen, heard, told), the held ticks by the age of the last sense (within
/// [`SEEN_MS`], past it: the owner's area memory), and the fairness count: a spot's place moved by a sense older than
/// [`SEEN_MS`] (0: only a fresh sense moves it, never his live place).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SniperLaneStats {
    pub made: u32,
    pub moved: u32,
    pub casts: u32,
    pub regions: u32,
    pub held_ticks: u32,
    pub made_by: [u32; 3],
    pub held_by_age: [u32; 2],
    pub unfair: u32,
}

/// [measure] The objective plays' ways weighed by the remembered snipers' lanes (`threat.sniper_memory`), by team: the
/// candidates weighed (a sniper spot held), those scored down (some of their way in his lanes), and the metres in them,
/// summed. Test builds only.
pub mod sniper_way_tally {
    #[cfg(test)]
    thread_local! {
        static WAYS: std::cell::Cell<[[u64; 3]; 2]> = const { std::cell::Cell::new([[0; 3]; 2]) };
    }

    #[cfg(test)]
    pub fn weighed(team: u8, exposed_m: f32) {
        WAYS.with(|w| {
            let mut all = w.get();
            let t = &mut all[usize::from(team).min(1)];
            t[0] += 1;
            t[1] += u64::from(exposed_m > 0.0);
            t[2] += exposed_m.round() as u64;
            w.set(all);
        });
    }

    #[cfg(not(test))]
    pub fn weighed(_: u8, _: f32) {}

    /// [weighed, scored down, metres] by team, since the thread began.
    #[cfg(test)]
    pub fn counted() -> [[u64; 3]; 2] {
        WAYS.with(std::cell::Cell::get)
    }
}

/// [measure, #25b] The detours round the covered regions against the ways they replaced, by team: does a detour pass the
/// enemies the bot knows of (its own senses and its team's calls within [`detour_path_tally::KNOWN_MS`], `Track::own`;
/// any weapon) more than the plain way did. Test builds only; nothing decides by it.
#[cfg(test)]
pub mod detour_path_tally {
    use crate::v026g::bots::path::{Route, RouteHeight};
    use mapdata::Vec3;

    /// A known enemy this near a point of the way counts the way's metre as passing him, m.
    pub const NEAR_M: f32 = 25.0;
    /// A known place this fresh counts, ms.
    pub const KNOWN_MS: u32 = 10_000;
    /// The way is sampled every this many metres.
    const STEP_M: f32 = 2.0;

    thread_local! {
        /// By team: [detours, plain metres, detour metres, plain metres near a known enemy, detour metres near one].
        static PATHS: std::cell::Cell<[[f64; 5]; 2]> = const { std::cell::Cell::new([[0.0; 5]; 2]) };
    }

    /// The metres of `route` between `s0` and `s1` within [`NEAR_M`] of one of `known`.
    pub fn near_m(route: &Route, heights: RouteHeight, s0: f32, s1: f32, known: &[Vec3]) -> f32 {
        if known.is_empty() || s1 <= s0 {
            return 0.0;
        }
        let steps = ((s1 - s0) / STEP_M).ceil().max(1.0) as usize;
        let step = (s1 - s0) / steps as f32;
        (0..steps)
            .filter(|&k| {
                let p = route.at(s0 + (k as f32 + 0.5) * step, heights);
                known.iter().any(|e| e.distance_xz(p) <= NEAR_M)
            })
            .count() as f32
            * step
    }

    /// A detour taken by a bot of `team`: the plain way's and the detour's metres, and of each the metres near a known enemy.
    pub fn record(team: u8, plain_m: f32, detour_m: f32, plain_near_m: f32, detour_near_m: f32) {
        PATHS.with(|cell| {
            let mut all = cell.get();
            let t = &mut all[usize::from(team).min(1)];
            t[0] += 1.0;
            t[1] += f64::from(plain_m);
            t[2] += f64::from(detour_m);
            t[3] += f64::from(plain_near_m);
            t[4] += f64::from(detour_near_m);
            cell.set(all);
        });
    }

    /// The counts by team since the thread began.
    pub fn counted() -> [[f64; 5]; 2] {
        PATHS.with(std::cell::Cell::get)
    }
}

impl ThreatSpots {
    fn event(&mut self, time: u32, id: u32, shooter: Place, at: Vec3, note: SpotNote) {
        if self.events.len() < EVENTS_KEPT {
            self.events.push(SpotEvent { time, id, shooter, at, note });
        }
    }

    /// The places of `shooter`'s spots that are held at `now` (before his death takes their sense) and that killed at least
    /// `min_kills` of the team's bots: a holder, not a shooter who killed once in passing.
    pub fn held_of(&self, shooter: Place, now: u32, min_kills: usize) -> Vec<Vec3> {
        self.spots
            .iter()
            .filter(|s| s.shooter == shooter && s.held(now) && s.kill_count() >= min_kills)
            .map(|s| s.at)
            .collect()
    }

    /// The events since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<SpotEvent> {
        std::mem::take(&mut self.events)
    }

    /// A bot of the team died to `shooter`, whom the team sensed at
    /// `shooter_at` at `sensed`, the victim at `victim_at`.
    pub fn died(&mut self, victim_at: Vec3, shooter: Place, shooter_at: Vec3, sensed: u32, now: u32, rules: SpotRules) {
        let reach = REACH_FLOOR_M.max(shooter_at.distance_xz(victim_at) + REACH_PLUS_M);
        let kill = Kill { from: shooter_at, victim: victim_at };
        let merge_m = rules.merge_m();
        let mut matching = self
            .spots
            .iter_mut()
            .filter(|s| !s.is_sensed() && s.shooter == shooter && s.at.distance_xz(shooter_at) <= merge_m);
        // The area: the nearest; as it was, the first in the list.
        let spot = if rules.recentres() {
            matching.min_by(|a, b| a.at.distance_xz(shooter_at).total_cmp(&b.at.distance_xz(shooter_at)))
        } else {
            matching.next()
        };
        let Some(s) = spot else {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            self.spots.push(Spot {
                id,
                at: shooter_at,
                strength: 1.0,
                stamp: now,
                shooter,
                reach_m: reach,
                empty_since: None,
                cast: Cast::Due { kept: None },
                held_then: false,
                source: SpotSource::Kills { kills: vec![kill], seen: Some(sensed) },
            });
            self.event(now, id, shooter, shooter_at, SpotNote::Made);
            return;
        };
        s.strength = s.strength_at(now) + 1.0;
        s.stamp = now;
        s.empty_since = None;
        let SpotSource::Kills { kills, seen } = &mut s.source else {
            return;
        };
        *seen = Some(seen.map_or(sensed, |t| t.max(sensed)));
        // The kills kept: the latest, so the centre follows him (as it was:
        // the first ones).
        let kept = if kills.len() < BEARINGS_KEPT {
            kills.push(kill);
            true
        } else if rules.recentres() {
            kills.remove(0);
            kills.push(kill);
            true
        } else {
            false
        };
        if rules.recentres() {
            s.at = s.centre();
        }
        let why = match s.cast.lanes() {
            _ if reach > s.reach_m => Some("a longer kill"),
            Some(lanes) if rules.recast && lanes.from.distance_xz(s.at) > RECAST_MOVED_M => Some("its centre moved"),
            Some(lanes)
                if rules.recast
                    && kept
                    && !in_lane(&lanes.bearings, lanes.from, victim_at) =>
            {
                Some("a kill off its lanes")
            }
            _ => None,
        };
        s.reach_m = s.reach_m.max(reach);
        if why.is_some() {
            s.cast.again(rules.recast);
            count(Count::Recast);
        }
        let (id, at, strength) = (s.id, s.at, s.strength);
        self.event(now, id, shooter, at, SpotNote::Added { strength });
        if let Some(why) = why {
            self.event(now, id, shooter, at, SpotNote::Recast { why });
        }
    }

    /// The team sensed the sniper `shooter` at `at` at `time` (`how`: by its own sense; none known, as the last one, or
    /// told for a new spot), his weapon reaching `reach_m`, at `now`: a spot with lanes all round him, kept out by the
    /// routes like any held spot's ([`Covered`]): he sees the ground before he has killed from it. Held while he is sensed
    /// ([`SEEN_MS`]), or with the area memory ([`SniperLife::Remembered`]) that long after the last sense, its place his
    /// last sensed one ([`area_m`] says how far he may be from it). Its lanes are cast again when he has moved
    /// [`SNIPER_MOVED_M`].
    #[allow(clippy::too_many_arguments)]
    pub fn sniper_sensed(&mut self, shooter: Place, at: Vec3, time: u32, how: Option<Sense>, reach_m: f32, now: u32, rules: SpotRules) {
        let fill = rules.sniper.fill();
        let remembered = matches!(rules.sniper, SniperLife::Remembered { .. });
        let Some(s) = self.spots.iter_mut().find(|s| s.is_sensed() && s.shooter == shooter) else {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            let how = how.unwrap_or(Sense::Told);
            self.spots.push(Spot {
                id,
                at,
                strength: fill,
                stamp: time,
                shooter,
                reach_m,
                empty_since: None,
                cast: Cast::Due { kept: None },
                held_then: false,
                source: SpotSource::Sensed { how, last: time },
            });
            self.sniper_lanes.made += 1;
            self.sniper_lanes.made_by[how.index()] += 1;
            self.event(time, id, shooter, at, SpotNote::Made);
            return;
        };
        let SpotSource::Sensed { how: was, last } = &mut s.source else {
            return;
        };
        // A sense older than the last (another bot's, sensed before) says nothing new: it neither fills nor moves the
        // spot (#836 moved it back to the older place).
        if time <= *last {
            return;
        }
        *last = time;
        *was = how.unwrap_or(*was);
        s.strength = fill;
        s.stamp = time;
        s.empty_since = None;
        // Lanes to be cast (not yet) follow him for nothing; those cast or going out are cast again when he has moved. With
        // the memory the place is always his last sensed one (the lanes keep the place they were cast from).
        let moved = s.cast.lanes().map(|lanes| lanes.from.distance_xz(at) > SNIPER_MOVED_M);
        let follows = match moved {
            None => true,
            Some(moved) if moved || reach_m > s.reach_m => {
                s.reach_m = reach_m;
                s.cast.again(rules.recast);
                self.sniper_lanes.moved += 1;
                true
            }
            Some(_) => remembered,
        };
        if follows && s.at != at {
            s.at = at;
            // [fair] Only a fresh sense moves it.
            if now.saturating_sub(time) > SEEN_MS {
                self.sniper_lanes.unfair += 1;
            }
        }
    }

    /// What the sniper spots did, summed.
    pub fn sniper_stats(&self) -> SniperLaneStats {
        self.sniper_lanes
    }

    /// The shooter died: his spots keep half; a sniper's own is let go.
    pub fn shooter_died(&mut self, shooter: Place, now: u32) {
        let before = self.spots.len();
        self.spots.retain(|s| !(s.is_sensed() && s.shooter == shooter));
        if self.spots.len() != before {
            self.version = self.version.wrapping_add(1);
        }
        let mut noted = Vec::new();
        for s in self.spots.iter_mut().filter(|s| s.shooter == shooter) {
            s.strength = s.strength_at(now) * SHOOTER_DIED_KEEPS;
            s.stamp = now;
            if let SpotSource::Kills { seen, .. } = &mut s.source {
                *seen = None;
            }
            noted.push((s.id, s.at));
        }
        for (id, at) in noted {
            self.event(now, id, shooter, at, SpotNote::ShooterDied);
        }
    }

    /// The team sensed `shooter` at `at` at `time`. A sensed sniper's spot is [`Self::sniper_sensed`]'s, which moves it
    /// with the sense.
    pub fn shooter_sensed(&mut self, shooter: Place, at: Vec3, time: u32, rules: SpotRules) {
        for s in self.spots.iter_mut().filter(|s| s.shooter == shooter) {
            let near = s.at.distance_xz(at) <= rules.on_m();
            if let SpotSource::Kills { seen, .. } = &mut s.source
                && near
                && seen.is_none_or(|t| time > t)
            {
                *seen = Some(time);
            }
        }
    }

    /// Why spot `id` is not held at `now` (K2, a job's end): gone (cleared
    /// by a teammate, or faded out), its shooter died (his death took his
    /// sense and half its strength), the window of his last sense over
    /// ([`SEEN_MS`]), or its strength under [`HELD_STRENGTH`] with no sense.
    pub fn unheld_why(&self, id: u32, now: u32) -> JobEnd {
        let Some(s) = self.spots.iter().find(|s| s.id == id) else {
            return self.gone.iter().rev().find(|(g, _)| *g == id).map_or(JobEnd::Faded, |&(_, why)| why);
        };
        match s.sensed() {
            None => JobEnd::ShooterDied,
            Some(_) if s.strength_at(now) >= HELD_SEEN_STRENGTH => JobEnd::Unseen,
            Some(_) => JobEnd::Faded,
        }
    }

    /// The spots held at `now`.
    pub fn held(&self, now: u32) -> impl Iterator<Item = &Spot> {
        self.spots.iter().filter(move |s| s.held(now))
    }

    /// Any spot at all.
    pub fn is_empty(&self) -> bool {
        self.spots.is_empty()
    }

    /// A commander tick at `now`: spots too weak are dropped; a spot a
    /// teammate (`watchers`, eye points) has a line to within
    /// [`CLEAR_LINE_M`] for [`CLEAR_MS`] with no enemy (`enemies`, sensed
    /// points) on it is cleared; a held spot's regions are cast at,
    /// [`RAYS_PER_TICK`] a tick.
    pub fn tick(&mut self, now: u32, watchers: &[Vec3], enemies: &[Vec3], eyes: &dyn Eyes, rules: SpotRules) {
        let before = self.spots.len();
        let on_m = rules.on_m();
        let mut notes = Vec::new();
        let mut gone = Vec::new();
        self.spots.retain_mut(|s| {
            if s.strength_at(now) < GONE_STRENGTH {
                notes.push((s.id, s.shooter, s.at, SpotNote::Decayed));
                gone.push((s.id, JobEnd::Faded));
                return false;
            }
            let at = s.at + Vec3::new(0.0, eye_height(false, 0.0, 0.0), 0.0);
            let empty = !enemies.iter().any(|e| e.distance_xz(s.at) <= on_m) && {
                // The nearest few teammates within reach look: a sight ray each.
                let mut near: Vec<Vec3> = watchers
                    .iter()
                    .copied()
                    .filter(|w| w.distance_xz(s.at) <= CLEAR_LINE_M)
                    .collect();
                near.sort_by(|a, b| a.distance_xz(s.at).total_cmp(&b.distance_xz(s.at)));
                near.iter().take(WATCHERS_ASKED).any(|&w| eyes.sees(w, at))
            };
            if !empty {
                s.empty_since = None;
                return true;
            }
            let since = *s.empty_since.get_or_insert(now);
            let keep = now < since + CLEAR_MS;
            if !keep {
                notes.push((s.id, s.shooter, s.at, SpotNote::Cleared));
                gone.push((s.id, JobEnd::Cleared));
            }
            keep
        });
        self.gone.extend(gone);
        let extra = self.gone.len().saturating_sub(GONE_KEPT);
        self.gone.drain(..extra);
        // Held from now: noted once a change, whatever made it held (its
        // first kills, or a sense of him again).
        for s in self.spots.iter_mut() {
            let held = s.held(now);
            if held && !s.held_then {
                notes.push((s.id, s.shooter, s.at, SpotNote::Held));
            }
            s.held_then = held;
        }
        let mut changed = self.spots.len() != before;
        let mut points = None;
        for s in self.spots.iter_mut().filter(|s| s.held(now)) {
            let sniper = s.is_sensed();
            if sniper {
                self.sniper_lanes.held_ticks += 1;
                let fresh = s.sensed().is_some_and(|t| now.saturating_sub(t) <= SEEN_MS);
                self.sniper_lanes.held_by_age[usize::from(!fresh)] += 1;
            }
            if let Cast::Due { kept } = &mut s.cast {
                let kept = kept.take();
                let lanes = s.lanes(rules);
                let reach = s.reach_m;
                let at = s.at;
                let todo = points
                    .get_or_insert_with(|| eyes.region_points())
                    .iter()
                    .copied()
                    .filter(|(_, p)| {
                        p.distance_xz(at) <= reach && lanes.iter().any(|&(from, b)| in_lane(&[b], from, *p))
                    })
                    .collect();
                s.cast = Cast::Casting {
                    todo,
                    seen: Vec::new(),
                    lanes: Lanes { from: at, bearings: lanes.iter().map(|&(_, b)| b).collect() },
                    kept,
                };
            }
            let Cast::Casting { todo, seen, lanes, .. } = &mut s.cast else {
                continue;
            };
            // The rays go from the spot's point at a standing eye. A sniper's are by sight, one ray a region: the bullet
            // follow through cover of `shot_reaches` cast about 15 rays a region and put the lanes' rays at +9 % of the match's.
            let eye = lanes.from + Vec3::new(0.0, eye_height(false, 0.0, 0.0), 0.0);
            let take = todo.len().min(RAYS_PER_TICK);
            for (region, p) in todo.drain(..take) {
                let to = p + Vec3::new(0.0, TARGET_HEIGHT_M, 0.0);
                if if sniper { eyes.sees(eye, to) } else { eyes.shot_reaches(eye, to) } {
                    seen.push(region);
                }
            }
            if todo.is_empty() {
                let (cover, lanes) = (std::mem::take(seen), lanes.clone());
                notes.push((s.id, s.shooter, s.at, SpotNote::Cast { regions: cover.len() }));
                if sniper {
                    self.sniper_lanes.casts += 1;
                    self.sniper_lanes.regions += cover.len() as u32;
                }
                s.cast = Cast::Done { cover, lanes };
                changed = true;
            }
        }
        for (id, shooter, at, note) in notes {
            self.event(now, id, shooter, at, note);
        }
        if changed {
            self.version = self.version.wrapping_add(1);
        }
    }

    /// The regions the held spots see, those cast at so far only when all
    /// of a spot are (a half-cast line keeps nothing out, unless the cast
    /// before is kept meanwhile).
    pub fn covered_regions(&self, now: u32) -> Vec<u32> {
        self.covered_regions_kept(now, KeptOut::All)
    }

    /// [`Self::covered_regions`] of the spots `kept` keeps out of the routes.
    pub fn covered_regions_kept(&self, now: u32, kept: KeptOut) -> Vec<u32> {
        self.kept(now, kept)
            .filter_map(Spot::covered)
            .flatten()
            .copied()
            .collect()
    }

    /// The held spots `kept` keeps out of the routes.
    fn kept(&self, now: u32, kept: KeptOut) -> impl Iterator<Item = &Spot> {
        self.held(now).filter(move |s| kept == KeptOut::All || !s.is_sensed())
    }

    /// [`Self::covered_regions_kept`], less the regions `hidden` says smoke hides from the spot's shooter (given his feet and
    /// the region), and the regions so opened (covered by no held spot's clear line). `smoke.spot_rules`.
    pub fn covered_regions_smoked(&self, now: u32, kept: KeptOut, hidden: impl Fn(Vec3, u32) -> bool) -> (Vec<u32>, Vec<u32>) {
        let (mut covered, mut opened) = (Vec::new(), Vec::new());
        for spot in self.kept(now, kept) {
            for &r in spot.covered().into_iter().flatten() {
                if hidden(spot.at, r) {
                    opened.push(r);
                } else {
                    covered.push(r);
                }
            }
        }
        covered.sort_unstable();
        covered.dedup();
        opened.sort_unstable();
        opened.dedup();
        opened.retain(|r| covered.binary_search(r).is_err());
        (covered, opened)
    }

    /// The held spots as the plays read them ([`Picture`]).
    pub fn held_lines(&self, now: u32, rules: SpotRules) -> Vec<HeldLine> {
        self.held(now)
            .filter(|s| !s.is_sensed())
            .map(|s| HeldLine {
                id: s.id,
                at: s.at,
                shooter: s.shooter,
                lanes: s.lanes(rules),
                facing: s.facing(),
                reach_m: s.reach_m,
                kills: s.kill_count(),
                seen: s.sensed(),
            })
            .collect()
    }

    /// [threat.sniper_memory] The regions the held sensed snipers' spots see (their casts, as [`Self::covered_regions`]),
    /// sorted: the ground an objective's way is weighed by.
    pub fn sniper_regions(&self, now: u32) -> Vec<u32> {
        let mut regions: Vec<u32> = self
            .held(now)
            .filter(|s| s.is_sensed())
            .filter_map(Spot::covered)
            .flatten()
            .copied()
            .collect();
        regions.sort_unstable();
        regions.dedup();
        regions
    }
}

/// How far round a job's held spot its flank and pin rings look under and over his height for floors, m
/// (`ThreatTuning::job_spot_levels`): a storey or two, as Crossfire's `up_m`.
pub const JOB_LEVELS_M: f32 = 8.0;

/// A held spot as the plays read it: its place, shooter, lanes and reach.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldLine {
    pub id: u32,
    pub at: Vec3,
    pub shooter: Place,
    /// Each lane: the point it is seen from and the victim's bearing.
    lanes: Vec<(Vec3, f32)>,
    /// The way he faces (the mean of his victims' bearings), yaw.
    pub facing: f32,
    pub reach_m: f32,
    /// The kills it counts (at most [`BEARINGS_KEPT`]).
    pub kills: usize,
    /// When the team last sensed him on it, if since his last death.
    pub seen: Option<u32>,
}

impl HeldLine {
    /// A line for the tests of those who read one: a shooter at `at` who killed `victims`, sensed on it at `seen`.
    #[cfg(test)]
    pub(super) fn for_test(id: u32, shooter: Place, at: Vec3, victims: &[Vec3], seen: Option<u32>) -> Self {
        let lanes: Vec<(Vec3, f32)> = victims.iter().map(|&v| (at, bearing(at, v))).collect();
        Self {
            id,
            at,
            shooter,
            facing: lanes.first().map_or(0.0, |l| l.1),
            lanes,
            reach_m: 50.0,
            kills: victims.len(),
            seen,
        }
    }

    /// The ground direction he faces, as `kin::forward` gives one.
    pub fn facing_dir(&self) -> [f32; 2] {
        [self.facing.sin(), self.facing.cos()]
    }

    /// Whether `p` is in one of his lanes, within his reach: the ground he
    /// killed in. Geometry only: what of it he sees is the cast's
    /// ([`Covered`]); the plays ask this one, which needs no cast finished.
    pub fn covers(&self, p: Vec3) -> bool {
        p.distance_xz(self.at) <= self.reach_m && self.lanes.iter().any(|&(from, b)| in_lane(&[b], from, p))
    }
}

/// A role of the team job on a held spot (K2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Goes round to his side (Flank, facing from his death lanes).
    Flanker,
    /// Holds him from the edge of his reach, out of his lanes (Crossfire).
    Pinner,
}

/// A role's bot and when it was given the role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holder {
    pub place: Place,
    pub since: u32,
}

/// The team job on one held spot: the bots given each role, and those
/// that let a role of it go (not offered it again in this job).
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// The spot's number ([`Spot::id`]).
    pub spot: u32,
    /// Its lanes cover an objective area when it opened: objective work,
    /// its holders outside the [`JOB_SHARE`] cap.
    pub objective: bool,
    /// Its shooter: one job a shooter (a camper who moved has two spots).
    pub shooter: Place,
    pub since: u32,
    pub flanker: Option<Holder>,
    pub pinner: Option<Holder>,
    passed: Vec<Place>,
}

impl Job {
    pub fn role_of(&self, place: Place) -> Option<Role> {
        if self.flanker.is_some_and(|h| h.place == place) {
            Some(Role::Flanker)
        } else if self.pinner.is_some_and(|h| h.place == place) {
            Some(Role::Pinner)
        } else {
            None
        }
    }

    fn holder(&self, role: Role) -> Option<Holder> {
        match role {
            Role::Flanker => self.flanker,
            Role::Pinner => self.pinner,
        }
    }

    fn holder_mut(&mut self, role: Role) -> &mut Option<Holder> {
        match role {
            Role::Flanker => &mut self.flanker,
            Role::Pinner => &mut self.pinner,
        }
    }
}

/// A job runs this long at most, ms, then its spot rests [`JOB_REST_MS`]
/// before another: a flank takes 15-35 s (Flank's `give_up_s` is 35). GUESS.
pub const JOB_MS: u32 = 45_000;
pub const JOB_REST_MS: u32 = 20_000;
/// A role's bot that has not taken its play this long after the role was
/// given lets it go (a cooldown, its health, a higher play), ms: the
/// commander polls each second, so a few polls. GUESS.
pub const JOB_START_MS: u32 = 5_000;
/// A bot this far from a spot or more is given no role, m: Flank's `near`
/// consideration (100 m) less a margin.
pub const JOB_NEAR_M: f32 = 90.0;
/// Jobs a team runs at once, at most: two bots each.
pub const JOBS_MAX: usize = 2;
/// The team's bots in jobs, at most one in this many of its players
/// (rounded up): two jobs took 4 of a 6-bot team. GUESS (a third: the
/// flank-and-pin pair of a fire team, the rest on the objective).
pub const JOB_SHARE: usize = 3;

/// A teammate as the job's choice reads it: its place, feet, whether it
/// may take a role (alive, not an objective actor ([`super::objective::ObjectiveMap::is_actor`]),
/// something to fire), and the job play it runs now (the role and the
/// spot), if any.
#[derive(Clone, Copy, Debug)]
pub struct Mate {
    pub place: Place,
    pub feet: Vec3,
    pub free: bool,
    pub playing: Option<(Role, u32)>,
}

/// The team as the jobs read it: its living bots that may take a role, its
/// players (bots and humans, the [`JOB_SHARE`] cap's count), and the
/// objective areas ([`super::objective::ObjectiveMap::home_spots`]): a job on a spot whose lanes
/// cover one is objective work, outside the cap.
pub struct JobTeam<'a> {
    pub mates: &'a [Mate],
    pub size: usize,
    pub objective_areas: &'a [Vec3],
}

/// Why a role went ([`JobEvent::Passed`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassedWhy {
    /// Its bot carries or runs dry.
    NotFree,
    /// Its bot did not take the play within [`JOB_START_MS`].
    NoPlay,
}

impl std::fmt::Display for PassedWhy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PassedWhy::NotFree => "not free: carrying or dry",
            PassedWhy::NoPlay => "did not take the play",
        })
    }
}

/// Why a job ended ([`JobEvent::Ended`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JobEnd {
    /// Its spot is no longer held: his death took his sense and half its
    /// strength (a success when a holder killed him).
    ShooterDied,
    /// No longer held: the window of his last sense on it is over.
    Unseen,
    /// No longer held, or gone: its strength ran out.
    Faded,
    /// Gone: a teammate saw it empty.
    Cleared,
    /// A holder went down: the job ends and its spot rests (no feeding one
    /// by one).
    HolderDown { role: Role, place: Place },
    /// It ran [`JOB_MS`].
    Time,
    /// No bot holds a role of it.
    Nobody,
}

impl std::fmt::Display for JobEnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobEnd::ShooterDied => write!(f, "unheld: its shooter died"),
            JobEnd::Unseen => write!(f, "unheld: not sensed on it lately"),
            JobEnd::Faded => write!(f, "unheld: faded"),
            JobEnd::Cleared => write!(f, "unheld: cleared"),
            JobEnd::HolderDown { role, place } => write!(f, "its {role:?} bot {place} went down"),
            JobEnd::Time => write!(f, "ran its time"),
            JobEnd::Nobody => write!(f, "no bot free to hold it"),
        }
    }
}

/// What a job tick did, for the notes and the bench's counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JobEvent {
    /// A role given to a bot.
    Given { spot: u32, role: Role, place: Place },
    /// A role let go, and why.
    Passed { spot: u32, role: Role, place: Place, why: PassedWhy },
    /// A job ended: why, its shooter, after how long (ms).
    Ended { spot: u32, shooter: Place, end: JobEnd, life_ms: u32 },
}

impl std::fmt::Display for JobEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            JobEvent::Given { spot, role, place } => write!(f, "job on threat spot {spot}: {role:?} is bot {place}"),
            JobEvent::Passed { spot, role, place, why } => {
                write!(f, "job on threat spot {spot}: bot {place} lets the {role:?} role go ({why})")
            }
            JobEvent::Ended { spot, end, life_ms, .. } => {
                write!(f, "job on threat spot {spot} ends ({end}) after {:.1} s", life_ms as f32 / 1000.0)
            }
        }
    }
}

/// What the team's plays read of its threat spots (`TeamKnowledge::threat`):
/// the held lines and the jobs on them, and (`threat.sniper_memory`) the regions the held sensed snipers see, sorted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picture {
    pub held: Vec<HeldLine>,
    pub jobs: Vec<Job>,
    pub sniper_regions: Vec<u32>,
}

impl Picture {
    /// The held line covering `p`, if any.
    pub fn line_at(&self, p: Vec3) -> Option<&HeldLine> {
        self.held.iter().find(|l| l.covers(p))
    }

    /// The held line of spot `id`, if it is held.
    pub fn line(&self, id: u32) -> Option<&HeldLine> {
        self.held.iter().find(|l| l.id == id)
    }

    /// The held line whose spot is at `at` now (the by-point lookup, the
    /// `job_line_by_id` switch off).
    pub fn line_by_spot(&self, at: Vec3) -> Option<&HeldLine> {
        self.held.iter().find(|l| l.at.distance_xz(at) < 0.01)
    }

    /// This bot's job, its role and the held line the job is on.
    pub fn job_of(&self, place: Place) -> Option<(&Job, Role, &HeldLine)> {
        self.jobs.iter().find_map(|j| {
            let role = j.role_of(place)?;
            Some((j, role, self.line(j.spot)?))
        })
    }
}

/// The held lines a job may open on (K2): the holds, with `min_kills`
/// kills or more ([`ThreatTuning::job_kills`]).
///
/// [`ThreatTuning::job_kills`]: crate::v026g::bots::tuning::ThreatTuning::job_kills
pub fn holds(held: &[HeldLine], min_kills: u32) -> Vec<HeldLine> {
    held.iter().filter(|l| l.kills >= min_kills as usize).cloned().collect()
}

/// The team's jobs at `now` (K2), and what changed:
/// - a job ends when its spot is no longer held (`unheld` says why), when a
///   holder goes down (missing from `mates`: no refill of a dead holder in
///   the same tick, no feeding one by one), or after [`JOB_MS`]; its spot
///   then rests [`JOB_REST_MS`]; and when no bot holds a role of it;
/// - a role's bot that is not free (carrying or dry), or has not taken its
///   play [`JOB_START_MS`] after it was given the role, lets it go (and is
///   not offered a role of that job again);
/// - a free role goes to the nearest free teammate within [`JOB_NEAR_M`]
///   that stands out of every held line and has no role, the flanker first,
///   while the team's bots in jobs off the objective are fewer than a
///   [`JOB_SHARE`]th of its players (rounded up; a job on a spot covering
///   an objective area is objective work and not counted);
/// - a held spot with no job, whose shooter has none, gets one, up to
///   [`JOBS_MAX`], only when a role of it can be given at once: a spot no
///   free bot can take keeps no slot (nor its shooter's one job), and the
///   next spot is tried ([`Count::JobNotOpened`]).
///
/// `held`: the spots jobs open on (the holds, [`holds`]); `lanes`: the held
/// lines a role's bot stands out of (every held line). `rested`: spots
/// resting, and until when. `opening`: how a new job opens ([`Opening`]).
#[allow(clippy::too_many_arguments)]
pub fn jobs_tick(
    jobs: &mut Vec<Job>,
    rested: &mut Vec<(u32, u32)>,
    held: &[HeldLine],
    lanes: &[HeldLine],
    opening: Opening,
    team: &JobTeam,
    unheld: &dyn Fn(u32) -> JobEnd,
    now: u32,
) -> Vec<JobEvent> {
    let mates = team.mates;
    let mut events = Vec::new();
    rested.retain(|&(_, until)| now < until);
    let mate = |place: Place| mates.iter().find(|m| m.place == place);
    jobs.retain(|j| {
        let down = [Role::Flanker, Role::Pinner]
            .into_iter()
            .find_map(|role| j.holder(role).filter(|h| mate(h.place).is_none()).map(|h| JobEnd::HolderDown { role, place: h.place }));
        let end = if !held.iter().any(|l| l.id == j.spot) {
            Some(unheld(j.spot))
        } else if let Some(down) = down {
            Some(down)
        } else if now >= j.since + JOB_MS {
            Some(JobEnd::Time)
        } else {
            None
        };
        if let Some(end) = end {
            rested.push((j.spot, now + JOB_REST_MS));
            events.push(JobEvent::Ended { spot: j.spot, shooter: j.shooter, end, life_ms: now.saturating_sub(j.since) });
        }
        end.is_none()
    });
    for j in jobs.iter_mut() {
        for role in [Role::Flanker, Role::Pinner] {
            let spot = j.spot;
            let Some(h) = j.holder(role) else {
                continue;
            };
            let Some(m) = mate(h.place) else {
                continue;
            };
            let why = if !m.free {
                Some(PassedWhy::NotFree)
            } else if m.playing != Some((role, spot)) && now >= h.since + JOB_START_MS {
                Some(PassedWhy::NoPlay)
            } else {
                None
            };
            if let Some(why) = why {
                *j.holder_mut(role) = None;
                j.passed.push(h.place);
                events.push(JobEvent::Passed { spot, role, place: h.place, why });
            }
        }
    }
    let share = team.size.div_ceil(JOB_SHARE);
    let staff = StaffRules { mates, lanes, share, now };
    if opening == Opening::ThenDropped {
        open_then_drop(jobs, rested, held, team, &staff, &mut events);
        return events;
    }
    for i in 0..jobs.len() {
        if let Some(line) = held.iter().find(|l| l.id == jobs[i].spot) {
            staff.roles(jobs, i, line, &mut events);
        }
    }
    // A job nobody holds ends.
    jobs.retain(|j| {
        let staffed = j.flanker.is_some() || j.pinner.is_some();
        if !staffed {
            rested.push((j.spot, now + JOB_REST_MS));
            events.push(JobEvent::Ended { spot: j.spot, shooter: j.shooter, end: JobEnd::Nobody, life_ms: now.saturating_sub(j.since) });
        }
        staffed
    });
    // New jobs, each opened only when a role of it is given now.
    for line in held {
        if jobs.len() >= JOBS_MAX {
            break;
        }
        let free = !jobs.iter().any(|j| j.spot == line.id || j.shooter == line.shooter)
            && !rested.iter().any(|&(id, _)| id == line.id);
        if !free {
            continue;
        }
        let objective = team.objective_areas.iter().any(|&a| line.covers(a));
        jobs.push(Job {
            spot: line.id,
            objective,
            shooter: line.shooter,
            since: now,
            flanker: None,
            pinner: None,
            passed: Vec::new(),
        });
        let i = jobs.len() - 1;
        staff.roles(jobs, i, line, &mut events);
        if jobs[i].flanker.is_none() && jobs[i].pinner.is_none() {
            jobs.pop();
            count(Count::JobNotOpened);
        }
    }
    events
}

/// How [`jobs_tick`] opens a job on a held spot ([`ThreatTuning::job_open_staffed`]).
///
/// [`ThreatTuning::job_open_staffed`]: crate::v026g::bots::tuning::ThreatTuning::job_open_staffed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opening {
    /// Only when a role of it is given at once: the next spot is tried.
    Staffed,
    /// Opened on every free spot up to [`JOBS_MAX`] first, staffed after,
    /// those with nobody dropped at the tick's end (holding a slot and their
    /// shooter's one job meanwhile).
    ThenDropped,
}

/// [`jobs_tick`]'s opening and staffing by [`Opening::ThenDropped`].
fn open_then_drop(jobs: &mut Vec<Job>, rested: &mut Vec<(u32, u32)>, held: &[HeldLine], team: &JobTeam, staff: &StaffRules, events: &mut Vec<JobEvent>) {
    let now = staff.now;
    let mut opened = Vec::new();
    for line in held {
        if jobs.len() >= JOBS_MAX {
            break;
        }
        let free = !jobs.iter().any(|j| j.spot == line.id || j.shooter == line.shooter)
            && !rested.iter().any(|&(id, _)| id == line.id);
        if free {
            let objective = team.objective_areas.iter().any(|&a| line.covers(a));
            jobs.push(Job {
                spot: line.id,
                objective,
                shooter: line.shooter,
                since: now,
                flanker: None,
                pinner: None,
                passed: Vec::new(),
            });
            opened.push(line.id);
        }
    }
    for i in 0..jobs.len() {
        if let Some(line) = held.iter().find(|l| l.id == jobs[i].spot) {
            staff.roles(jobs, i, line, events);
        }
    }
    // A job nobody holds: a new one is not opened; an old one ends.
    jobs.retain(|j| {
        let staffed = j.flanker.is_some() || j.pinner.is_some();
        if !staffed && !opened.contains(&j.spot) {
            rested.push((j.spot, now + JOB_REST_MS));
            events.push(JobEvent::Ended { spot: j.spot, shooter: j.shooter, end: JobEnd::Nobody, life_ms: now.saturating_sub(j.since) });
        }
        if !staffed && opened.contains(&j.spot) {
            count(Count::JobNotOpened);
        }
        staffed
    });
}

/// Who may take a job's free roles ([`jobs_tick`]): the team's `mates`, out
/// of every held line (`lanes`), while the team's bots in jobs off the
/// objective are fewer than `share`.
struct StaffRules<'a> {
    mates: &'a [Mate],
    lanes: &'a [HeldLine],
    share: usize,
    now: u32,
}

impl StaffRules<'_> {
    /// Job `i`'s free roles (on `line`) to the nearest free teammates, the
    /// flanker first.
    fn roles(&self, jobs: &mut [Job], i: usize, line: &HeldLine, events: &mut Vec<JobEvent>) {
        for role in [Role::Flanker, Role::Pinner] {
            if jobs[i].holder(role).is_some() {
                continue;
            }
            let off_objective: usize = jobs
                .iter()
                .filter(|j| !j.objective)
                .map(|j| usize::from(j.flanker.is_some()) + usize::from(j.pinner.is_some()))
                .sum();
            if !jobs[i].objective && off_objective >= self.share {
                break;
            }
            let busy = |place: Place| jobs.iter().any(|j| j.role_of(place).is_some());
            let passed = &jobs[i].passed;
            let pick = self
                .mates
                .iter()
                .filter(|m| m.free && !busy(m.place) && !passed.contains(&m.place))
                .filter(|m| m.feet.distance_xz(line.at) <= JOB_NEAR_M)
                .filter(|m| !self.lanes.iter().any(|l| l.covers(m.feet)))
                .min_by(|a, b| a.feet.distance_xz(line.at).total_cmp(&b.feet.distance_xz(line.at)))
                .map(|m| m.place);
            if let Some(place) = pick {
                *jobs[i].holder_mut(role) = Some(Holder { place, since: self.now });
                events.push(JobEvent::Given { spot: line.id, role, place });
            }
        }
    }
}

// The room of the cheapest way from region `from` to `to` (A*, A2): a walk
/// into a covered region (`covered`, but those in `free`) costs
/// `1 + lane_cost` times its length (the whole route weighed, Valve's and
/// Killzone's danger cost, not a closure); `region_ok` shuts regions (never
/// the goal). The room is the way's regions and those next to it that are
/// not covered: by region, whether the floors' search may go there. None
/// with no way.
#[allow(clippy::too_many_arguments)]
pub fn costed_corridor(
    grid: &mapdata::WalkGrid,
    regions: &mapdata::regions::Regions,
    from: u32,
    to: u32,
    covered: &Covered,
    free: &[u32],
    lane_cost: f32,
    region_ok: impl Fn(u32) -> bool,
) -> Option<Vec<bool>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let n = regions.len();
    if from as usize >= n || to as usize >= n {
        return None;
    }
    let goal = grid.position(regions.middle(to));
    let h = |r: u32| grid.position(regions.middle(r)).distance_xz(goal);
    let in_lane = |r: u32| covered.region(r) && !free.contains(&r);
    let mut cost = vec![f32::INFINITY; n];
    let mut parent: Vec<Option<u32>> = vec![None; n];
    let mut closed = vec![false; n];
    let mut open = BinaryHeap::new();
    // Ordered by the bits of a non-negative f32, which order as its value.
    let key = |c: f32| Reverse(c.max(0.0).to_bits());
    cost[from as usize] = 0.0;
    open.push((key(h(from)), from));
    let mut found = false;
    while let Some((_, u)) = open.pop() {
        if std::mem::replace(&mut closed[u as usize], true) {
            continue;
        }
        if u == to {
            found = true;
            break;
        }
        let cu = cost[u as usize];
        for &(v, step) in regions.out(u) {
            if closed[v as usize] || (v != to && !region_ok(v)) {
                continue;
            }
            let c = cu + step * if in_lane(v) { 1.0 + lane_cost } else { 1.0 };
            if c < cost[v as usize] {
                cost[v as usize] = c;
                parent[v as usize] = Some(u);
                open.push((key(c + h(v)), v));
            }
        }
    }
    if !found {
        return None;
    }
    let mut room = vec![false; n];
    let mut at = to;
    loop {
        room[at as usize] = true;
        for v in regions.out(at).iter().map(|&(v, _)| v).chain(regions.into(at).iter().copied()) {
            if !in_lane(v) && region_ok(v) {
                room[v as usize] = true;
            }
        }
        match parent[at as usize] {
            Some(up) if at != from => at = up,
            _ => break,
        }
    }
    Some(room)
}

/// [smoke] A held spot as the team's bots read it for a smoke across its lane (`smoke.cross_lane`): where its
/// shooter killed from (a sensed sniper's: where he was last sensed), how far his line reaches, when the team last
/// sensed him there, and how far from `at` he may be by now ([`area_m`] of that age). Being in `bot.lanes` is being held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeldLane {
    pub at: Vec3,
    pub shooter: Place,
    pub reach_m: f32,
    pub sensed: Option<u32>,
    pub area_m: f32,
}

impl ThreatSpots {
    /// The spots held at `now`, as lanes.
    pub fn lanes(&self, now: u32) -> Vec<HeldLane> {
        self.lanes_kept(now, KeptOut::All)
    }

    /// [`Self::lanes`] of the spots `kept` keeps (`SniperMemory::smoke`).
    pub fn lanes_kept(&self, now: u32, kept: KeptOut) -> Vec<HeldLane> {
        self.kept(now, kept)
            .map(|s| HeldLane {
                at: s.at,
                shooter: s.shooter,
                reach_m: s.reach_m,
                sensed: s.sensed(),
                area_m: area_m(s.sensed().map(|t| now.saturating_sub(t))),
            })
            .collect()
    }
}

/// A team's threat state, by feature ([`super::features`]).
#[derive(Default)]
pub struct TeamThreat {
    /// Where a shooter killed its bots from ([`ThreatSpots`]).
    pub spots: ThreatSpots,
    /// The regions his line covers now.
    pub covered: Option<std::sync::Arc<Covered>>,
    /// [threat.sniper_memory] The regions of the kill spots' lines alone, while a consumer's switch keeps the sensed
    /// snipers' spots out of it.
    pub covered_kills: Option<std::sync::Arc<Covered>>,
    /// The team jobs on held spots (K2).
    pub jobs: Vec<Job>,
    /// Spots whose job ended, resting until then.
    pub rested: Vec<(u32, u32)>,
    /// The team's deaths of the last [`DEATH_KEPT_MS`] and their killers
    /// (the kill feed's), for a job holder's end.
    pub deaths: Vec<Death>,
}

/// A death of the team: who, by whom if the kill feed told, when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub victim: Place,
    pub killer: Option<Place>,
    pub time: u32,
}

/// The team's deaths are kept this long, ms: the commander ticks each
/// half second, so a holder's death is there at its next tick.
pub const DEATH_KEPT_MS: u32 = 5_000;

/// A bot's threat state (A3): the cover its route was last planned
/// against, and the cover its way ahead was last found clear of (looked at
/// once a change; a replan the poll's budget starves is looked at again).
#[derive(Default)]
pub struct BotThreat {
    pub planned_with: Option<std::sync::Arc<Covered>>,
    pub clear_with: Option<std::sync::Arc<Covered>>,
    /// The bench's replan waiting for its judgment ([`judged`]).
    #[cfg(test)]
    pub asked: Option<Asked>,
    /// The bench's tag of its last search (`path::tag`), for its resumed part.
    #[cfg(test)]
    pub search_tag: u8,
}

impl super::features::Feature for BotThreat {
    fn new_life(&mut self, _life: &super::features::Life) {
        self.planned_with = None;
        self.clear_with = None;
        #[cfg(test)]
        {
            self.asked = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regions 10 m apart on a line; the ones in `blind` are behind a wall.
    struct Line {
        blind: Vec<u32>,
    }

    impl Eyes for Line {
        fn sees(&self, _from: Vec3, to: Vec3) -> bool {
            !self.blind.contains(&((to.x / 10.0).round() as u32))
        }

        fn region_points(&self) -> Vec<(u32, Vec3)> {
            (0..20).map(|r| (r, Vec3::new(r as f32 * 10.0, 0.0, 0.0))).collect()
        }
    }

    /// The rules as they were before the area and the recast.
    const OLD: SpotRules = SpotRules { model: SpotModel::Point, recast: false, sniper: SniperLife::WhileSensed };

    fn at(x: f32) -> Vec3 {
        Vec3::new(x, 0.0, 0.0)
    }

    #[test]
    fn deaths_add_up_on_a_spot_two_hold_it_and_time_lets_it_go() {
        let mut spots = ThreatSpots::default();
        // One death, the shooter sensed on the spot 2 s before: held for the
        // 15 s from that sense.
        spots.died(at(30.0), 7, at(0.0), 1_000, 3_000, OLD);
        assert_eq!(spots.held(3_000).count(), 1);
        assert_eq!(spots.held(16_000).count(), 1);
        assert_eq!(spots.held(16_001).count(), 0, "one death, unsensed for 15 s");
        // Two deaths hold it without a sense.
        spots.died(at(35.0), 7, at(3.0), 8_000, 10_000, OLD);
        assert_eq!(spots.spots.len(), 1);
        assert_eq!(spots.held(60_000).count(), 1);
        // Two minutes on, the 2.0 is down to 1.5 less 1.0: not held.
        assert_eq!(spots.held(10_000 + 120_000).count(), 0);
        // Another shooter on the same ground is another spot.
        spots.died(at(35.0), 8, at(3.0), 11_000, 11_500, OLD);
        assert_eq!(spots.spots.len(), 2);
    }

    #[test]
    fn a_shooter_sensed_on_a_weak_spot_holds_it_and_one_off_it_does_not() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 1_000, OLD);
        assert_eq!(spots.held(20_000).count(), 0);
        spots.shooter_sensed(7, at(2.0), 20_000, OLD);
        assert_eq!(spots.held(21_000).count(), 1);
        // Sensed 20 m off the spot: not on it.
        spots.shooter_sensed(7, at(20.0), 40_000, OLD);
        assert_eq!(spots.held(41_000).count(), 0);
    }

    #[test]
    fn the_shooters_death_halves_the_spot() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 500, OLD);
        spots.died(at(30.0), 7, at(0.0), 500, 1_000, OLD);
        assert_eq!(spots.held(1_000).count(), 1);
        spots.shooter_died(7, 2_000);
        // 2.0 becomes 1.0, and he is not sensed on it any more.
        assert_eq!(spots.held(2_000).count(), 0);
    }

    /// [A145] A killed shooter's spot is a holder's only when he killed from it again (the camper class), whatever else is held.
    #[test]
    fn a_holder_is_a_shooter_who_killed_again_from_one_place() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 500, OLD);
        assert_eq!(spots.held(1_000).count(), 1, "one kill, the shooter sensed on it: held");
        assert!(spots.held_of(7, 1_000, 2).is_empty(), "but no holder");
        assert_eq!(spots.held_of(7, 1_000, 1).len(), 1);
        spots.died(at(31.0), 7, at(0.0), 700, 900, OLD);
        assert_eq!(spots.held_of(7, 1_000, 2).len(), 1, "killed again from it");
        assert!(spots.held_of(8, 1_000, 1).is_empty(), "another shooter's");
    }

    #[test]
    fn a_held_spot_casts_at_its_regions_and_keeps_those_seen() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 500, OLD);
        spots.died(at(30.0), 7, at(0.0), 400, 600, OLD);
        let eyes = Line { blind: vec![3, 4] };
        spots.tick(1_000, &[], &[], &eyes, OLD);
        // Reach 45 m: regions 0 to 4; 3 and 4 are behind a wall.
        let mut seen = spots.covered_regions(1_000);
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2]);
        assert!(spots.version > 0);
    }

    #[test]
    fn a_big_reach_is_cast_in_steps_and_keeps_nothing_until_all_is_cast() {
        let mut spots = ThreatSpots::default();
        // 190 m: all 20 regions are within reach and 64 rays a tick finish
        // them at once, so make the field larger through a long line.
        spots.died(at(30.0), 7, at(0.0), 0, 500, OLD);
        spots.died(at(30.0), 7, at(0.0), 0, 600, OLD);
        struct Wide;
        impl Eyes for Wide {
            fn sees(&self, _: Vec3, _: Vec3) -> bool {
                true
            }
            fn region_points(&self) -> Vec<(u32, Vec3)> {
                (0..150).map(|r| (r, Vec3::new(r as f32 * 0.2, 0.0, 0.0))).collect()
            }
        }
        spots.tick(1_000, &[], &[], &Wide, OLD);
        assert!(spots.covered_regions(1_000).is_empty(), "64 of 150 cast");
        spots.tick(1_100, &[], &[], &Wide, OLD);
        assert!(spots.covered_regions(1_100).is_empty(), "128 of 150 cast");
        spots.tick(1_200, &[], &[], &Wide, OLD);
        assert_eq!(spots.covered_regions(1_200).len(), 150);
    }

    #[test]
    fn a_spot_is_cleared_when_a_watcher_sees_it_empty_for_three_seconds() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 0, OLD);
        let eyes = Line { blind: vec![] };
        let watcher = [at(20.0)];
        spots.tick(1_000, &watcher, &[], &eyes, OLD);
        spots.tick(3_900, &watcher, &[], &eyes, OLD);
        assert!(!spots.is_empty());
        spots.tick(4_100, &watcher, &[], &eyes, OLD);
        assert!(spots.is_empty());
        // An enemy sensed on the spot keeps it.
        spots.died(at(30.0), 7, at(0.0), 5_000, 5_000, OLD);
        spots.tick(6_000, &watcher, &[at(1.0)], &eyes, OLD);
        spots.tick(9_500, &watcher, &[at(1.0)], &eyes, OLD);
        assert!(!spots.is_empty());
        // Nobody near enough to look: kept.
        spots.tick(20_000, &[at(100.0)], &[], &eyes, OLD);
        assert!(!spots.is_empty());
    }

    #[test]
    fn a_spot_covers_the_lanes_of_its_victims_not_all_it_sees() {
        let mut spots = ThreatSpots::default();
        // Victims 30 m along +x, then 30 m along +z.
        spots.died(at(30.0), 7, at(0.0), 0, 500, OLD);
        spots.died(Vec3::new(0.0, 0.0, 30.0), 7, at(0.0), 0, 600, OLD);
        struct Ring;
        impl Eyes for Ring {
            fn sees(&self, _: Vec3, _: Vec3) -> bool {
                true
            }
            fn region_points(&self) -> Vec<(u32, Vec3)> {
                vec![
                    (0, Vec3::new(20.0, 0.0, 0.0)),
                    (1, Vec3::new(20.0, 0.0, 5.0)),
                    (2, Vec3::new(0.0, 0.0, 20.0)),
                    (3, Vec3::new(-20.0, 0.0, 0.0)),
                    (4, Vec3::new(14.0, 0.0, 14.0)),
                    (5, Vec3::new(20.0, 0.0, 9.0)),
                ]
            }
        }
        spots.tick(1_000, &[], &[], &Ring, OLD);
        let mut seen = spots.covered_regions(1_000);
        seen.sort_unstable();
        // On +x (0), 14 degrees off it (1), on +z (2); not behind (3), not
        // between the two lanes (4), not 24 degrees off (5).
        assert_eq!(seen, vec![0, 1, 2]);
    }

    #[test]
    fn exposure_is_the_metres_a_route_runs_through_seen_ground_and_where() {
        use crate::v026g::bots::path::Waypoint;
        let wp = |x: f32| Waypoint { at: at(x), node: 0, jump: None, low: false };
        let route = Route::new(vec![wp(0.0), wp(30.0), wp(60.0)]);
        let seen = |p: Vec3| (20.0..35.0).contains(&p.x);
        // Samples every 3 m: 21, 24, 27, 30, 33 are seen.
        let e = exposure(&route, RouteHeight::Chord, seen, &[]);
        assert_eq!(e.metres, 15.0);
        assert_eq!(e.spans, vec![(21.0, 33.0)]);
        // An end in the middle of it: the ground within 10 m is free.
        assert_eq!(exposure(&route, RouteHeight::Chord, seen, &[at(30.0)]).metres, 0.0);
        // Two lanes make two spans; the horizon stops the count.
        let two = |p: Vec3| (6.0..10.0).contains(&p.x) || (40.0..44.0).contains(&p.x);
        let e = exposure(&route, RouteHeight::Chord, two, &[]);
        assert_eq!(e.spans, vec![(6.0, 9.0), (42.0, 42.0)]);
        let long = Route::new(vec![wp(0.0), wp(500.0)]);
        assert_eq!(exposure(&long, RouteHeight::Chord, |p| p.x > 150.0, &[]).metres, 0.0, "beyond the horizon");
    }

    #[test]
    fn hits_finds_a_small_disc_the_sampling_would_step_over() {
        use crate::v026g::bots::path::Waypoint;
        let wp = |x: f32, z: f32| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
        let route = Route::new(vec![wp(0.0, 0.0), wp(30.0, 0.0), wp(30.0, 30.0)]);
        // A 0.5 m disc on the first leg at x 10.1, z 0.2: the chord is 2 * sqrt(0.25 - 0.04).
        let e = hits(&route, &[(Vec3::new(10.1, 0.0, 0.2), 0.5)]);
        let chord = 2.0 * (0.25_f32 - 0.04).sqrt();
        assert_eq!(e.spans.len(), 1);
        assert!((e.spans[0].0 - (10.1 - chord / 2.0)).abs() < 1e-3 && (e.metres - chord).abs() < 1e-3, "{e:?}");
        // The sampling at 3 m (0, 3, 6, 9, 12) misses it.
        assert_eq!(exposure(&route, RouteHeight::Chord, |p| p.distance_xz(Vec3::new(10.1, 0.0, 0.2)) < 0.5, &[]).metres, 0.0);
        // Off the line by more than the radius: none. On the second leg: after 30 m.
        assert!(hits(&route, &[(Vec3::new(10.0, 0.0, 0.6), 0.5)]).spans.is_empty());
        let e = hits(&route, &[(Vec3::new(30.2, 0.0, 12.0), 0.9)]);
        assert!((e.spans[0].0 - (42.0 - (0.81_f32 - 0.04).sqrt())).abs() < 1e-3, "{e:?}");
        // Two discs touching make one span.
        let e = hits(&route, &[(Vec3::new(5.0, 0.0, 0.0), 0.5), (Vec3::new(5.6, 0.0, 0.0), 0.5)]);
        assert_eq!(e.spans.len(), 1);
        assert!((e.metres - 1.6).abs() < 1e-3, "{e:?}");
    }

    /// The rules on by default: the area, the recentre, the recast.
    const NEW: SpotRules = SpotRules { model: SpotModel::Area { m: AREA_M }, recast: true, sniper: SniperLife::WhileSensed };

    #[test]
    fn a_spot_is_the_shooters_area_and_its_centre_follows_his_kills() {
        let mut spots = ThreatSpots::default();
        spots.died(at(40.0), 7, at(0.0), 0, 500, NEW);
        // 9 m off his first kill's place: the same spot (8 m was not).
        spots.died(at(45.0), 7, at(9.0), 600, 700, NEW);
        assert_eq!(spots.spots.len(), 1);
        assert!((spots.spots[0].at.x - 4.5).abs() < 1e-4, "{:?}", spots.spots[0].at);
        let mut old = ThreatSpots::default();
        old.died(at(40.0), 7, at(0.0), 0, 500, OLD);
        old.died(at(45.0), 7, at(9.0), 600, 700, OLD);
        assert_eq!(old.spots.len(), 2, "as it was: two spots");
        // He is on it while sensed in the area: 9 m from the centre.
        let mut spots = ThreatSpots::default();
        spots.died(at(40.0), 7, at(0.0), 0, 500, NEW);
        spots.shooter_sensed(7, at(9.0), 20_000, NEW);
        assert_eq!(spots.held(21_000).count(), 1);
    }

    #[test]
    fn a_kill_off_the_cast_lanes_casts_again_and_keeps_the_old_cover_meanwhile() {
        struct Ring;
        impl Eyes for Ring {
            fn sees(&self, _: Vec3, _: Vec3) -> bool {
                true
            }
            fn region_points(&self) -> Vec<(u32, Vec3)> {
                vec![(0, Vec3::new(20.0, 0.0, 0.0)), (1, Vec3::new(0.0, 0.0, 20.0))]
            }
        }
        for (rules, kept) in [(NEW, true), (OLD, false)] {
            let mut spots = ThreatSpots::default();
            spots.died(at(30.0), 7, at(0.0), 0, 500, rules);
            spots.died(at(30.0), 7, at(0.0), 500, 600, rules);
            spots.tick(1_000, &[], &[], &Ring, rules);
            assert_eq!(spots.covered_regions(1_000), vec![0]);
            spots.take_events();
            // A kill along +z, a lane not cast; his reach is no longer.
            spots.died(Vec3::new(0.0, 0.0, 30.0), 7, at(0.0), 1_500, 1_600, rules);
            let events: Vec<SpotNote> = spots.take_events().into_iter().map(|e| e.note).collect();
            if kept {
                assert!(events.contains(&SpotNote::Recast { why: "a kill off its lanes" }), "{events:?}");
                // The old cover meanwhile, the new one after the cast.
                assert_eq!(spots.covered_regions(1_600), vec![0]);
                spots.tick(2_000, &[], &[], &Ring, rules);
                let mut seen = spots.covered_regions(2_000);
                seen.sort_unstable();
                assert_eq!(seen, vec![0, 1]);
            } else {
                // As it was: the new lane is never cast.
                spots.tick(2_000, &[], &[], &Ring, rules);
                assert_eq!(spots.covered_regions(2_000), vec![0]);
            }
        }
    }

    #[test]
    fn a_sensed_sniper_is_a_spot_with_lanes_all_round_him_that_the_jobs_do_not_see() {
        struct Ring;
        impl Eyes for Ring {
            fn sees(&self, _: Vec3, _: Vec3) -> bool {
                true
            }
            fn region_points(&self) -> Vec<(u32, Vec3)> {
                // Four regions in the four directions, 40 m out.
                vec![
                    (0, Vec3::new(40.0, 0.0, 0.0)),
                    (1, Vec3::new(-40.0, 0.0, 0.0)),
                    (2, Vec3::new(0.0, 0.0, 40.0)),
                    (3, Vec3::new(0.0, 0.0, -40.0)),
                ]
            }
        }
        let mut spots = ThreatSpots::default();
        // No kill: sensed, held for as long as he is sensed, then not.
        spots.sniper_sensed(5, at(0.0), 1_000, Some(Sense::Heard), 100.0, 1_000, NEW);
        assert_eq!(spots.held(1_500).count(), 1);
        assert_eq!(spots.sniper_stats().made_by, [0, 1, 0], "made by a sound");
        spots.tick(1_500, &[], &[at(0.0)], &Ring, NEW);
        let mut seen = spots.covered_regions(1_500);
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2, 3], "all round him");
        assert_eq!(spots.sniper_regions(1_500), vec![0, 1, 2, 3]);
        assert!(spots.covered_regions_kept(1_500, KeptOut::KillsOnly).is_empty(), "routes off: the routes do not keep out of it");
        assert!(spots.lanes_kept(1_500, KeptOut::KillsOnly).is_empty(), "smoke off: no lane of his for a smoke");
        assert_eq!(spots.lanes_kept(1_500, KeptOut::All).len(), 1);
        assert!(spots.held_lines(1_500, NEW).is_empty(), "the jobs and the plays read no sniper spot");
        assert!(spots.held_of(5, 1_500, 1).is_empty(), "no kill behind it: no holder");
        assert_eq!(spots.held(1_000 + SEEN_MS + 1_000).count(), 0, "not sensed for a while: not held");
        // He moves a little: the same spot; far: cast again.
        spots.sniper_sensed(5, at(4.0), 2_000, None, 100.0, 2_000, NEW);
        assert_eq!((spots.spots.len(), spots.sniper_stats().moved), (1, 0));
        spots.sniper_sensed(5, at(30.0), 3_000, None, 100.0, 3_000, NEW);
        assert_eq!((spots.spots.len(), spots.sniper_stats().moved), (1, 1));
        // A kill by him is another spot, and his death lets his own go.
        spots.died(at(40.0), 5, at(30.0), 3_000, 3_100, NEW);
        assert_eq!(spots.spots.len(), 2);
        spots.shooter_died(5, 4_000);
        assert!(spots.spots.iter().all(|s| !s.is_sensed()));
        assert_eq!(spots.sniper_stats().unfair, 0);
    }

    /// The area memory (`threat.sniper_memory`): a sensed sniper is held two minutes after the last sense, his place the
    /// last sensed one and never his live one, his area widening from his exact spot to [`AREA_WIDE_M`].
    #[test]
    fn a_remembered_sniper_is_held_two_minutes_where_he_was_last_sensed_never_where_he_is() {
        const MEMORY: SpotRules = SpotRules {
            model: SpotModel::Area { m: AREA_M },
            recast: true,
            sniper: SniperLife::Remembered { memory_ms: 120_000 },
        };
        let eyes = Line { blind: vec![] };
        let mut spots = ThreatSpots::default();
        spots.sniper_sensed(5, at(0.0), 1_000, Some(Sense::Seen), 100.0, 1_000, MEMORY);
        // He walks off unseen: the commander's ticks see him nowhere (no sense), the spot stays where he was sensed.
        for k in 1..=120 {
            let now = 1_000 + k * 1_000;
            spots.tick(now, &[], &[], &eyes, MEMORY);
            assert_eq!(spots.spots[0].at, at(0.0), "the spot never follows his live place ({now} ms)");
        }
        assert_eq!(spots.held(1_000 + 115_000).count(), 1, "held well past the 15 s of a sense");
        assert_eq!(spots.held(1_000 + 125_000).count(), 0, "let go after about two minutes");
        let lane = |spots: &ThreatSpots, now| spots.lanes(now)[0];
        assert_eq!(lane(&spots, 1_000 + 5_000).area_m, 0.0, "his exact spot at first");
        assert_eq!(lane(&spots, 1_000 + 20_000).area_m, AREA_WIDE_M / 2.0);
        assert_eq!(lane(&spots, 1_000 + 60_000).area_m, AREA_WIDE_M, "then his area");
        assert_eq!(lane(&spots, 1_000 + 60_000).at, at(0.0));
        // A fresh sense 12 m off (less than a recast's move): the place is his new one, his area his exact spot again.
        spots.sniper_sensed(5, at(12.0), 70_000, Some(Sense::Told), 100.0, 70_000, MEMORY);
        assert_eq!((lane(&spots, 70_500).at, lane(&spots, 70_500).area_m), (at(12.0), 0.0));
        assert_eq!(spots.sniper_stats().moved, 0, "the lanes are not cast again for 12 m");
        assert_eq!(spots.held(70_000 + 115_000).count(), 1, "two minutes from the last sense");
        // A sense older than the last moves nothing.
        spots.sniper_sensed(5, at(40.0), 60_000, Some(Sense::Seen), 100.0, 70_600, MEMORY);
        assert_eq!(lane(&spots, 70_600).at, at(12.0));
        assert_eq!(spots.sniper_stats().unfair, 0);
        // Off (as #836): held only while sensed within 15 s.
        let mut off = ThreatSpots::default();
        off.sniper_sensed(5, at(0.0), 1_000, Some(Sense::Seen), 100.0, 1_000, NEW);
        assert_eq!(off.held(1_000 + SEEN_MS + 1_000).count(), 0);
    }

    #[test]
    fn a_sniper_spots_area_is_his_exact_spot_then_widens_to_twenty_metres() {
        assert_eq!(area_m(Some(0)), 0.0);
        assert_eq!(area_m(Some(AREA_EXACT_MS)), 0.0);
        assert!((area_m(Some((AREA_EXACT_MS + AREA_WIDE_MS) / 2)) - AREA_WIDE_M / 2.0).abs() < 1e-4);
        assert_eq!(area_m(Some(AREA_WIDE_MS)), AREA_WIDE_M);
        assert_eq!(area_m(Some(600_000)), AREA_WIDE_M);
        assert_eq!(area_m(None), AREA_WIDE_M, "never sensed since: the whole area");
    }

    #[test]
    fn each_spot_event_is_noted_once() {
        let mut spots = ThreatSpots::default();
        let eyes = Line { blind: vec![] };
        spots.died(at(30.0), 7, at(0.0), 0, 0, NEW);
        spots.tick(500, &[], &[], &eyes, NEW);
        spots.tick(600, &[], &[], &eyes, NEW);
        let notes: Vec<SpotNote> = spots.take_events().into_iter().map(|e| e.note).collect();
        assert_eq!(notes[0], SpotNote::Made);
        assert_eq!(notes.iter().filter(|n| **n == SpotNote::Held).count(), 1, "{notes:?}");
        assert!(matches!(notes.last(), Some(SpotNote::Cast { .. })), "{notes:?}");
        spots.shooter_died(7, 700);
        spots.tick(800, &[at(20.0)], &[], &eyes, NEW);
        spots.tick(4_000, &[at(20.0)], &[], &eyes, NEW);
        let notes: Vec<SpotNote> = spots.take_events().into_iter().map(|e| e.note).collect();
        assert_eq!(notes, vec![SpotNote::ShooterDied, SpotNote::Cleared]);
        let text = SpotEvent { time: 0, id: 3, shooter: 7, at: at(1.0), note: SpotNote::Cleared }.to_string();
        assert_eq!(text, "threat spot 3 of shooter 7 at (1.0, 0.0, 0.0): cleared, seen empty");
    }

    fn line(id: u32, at: Vec3, bearings: &[f32]) -> HeldLine {
        HeldLine {
            id,
            at,
            shooter: 7,
            lanes: bearings.iter().map(|&b| (at, b)).collect(),
            facing: bearings[0],
            reach_m: 50.0,
            kills: bearings.len(),
            seen: None,
        }
    }

    /// No spot is gone or unseen in these tests but by their own steps.
    fn unheld(_: u32) -> JobEnd {
        JobEnd::Faded
    }

    #[test]
    fn a_held_line_covers_his_lanes_within_his_reach() {
        // Lanes along +x (bearing pi/2 as `bearing` gives it).
        let l = line(0, at(0.0), &[bearing(at(0.0), at(30.0))]);
        assert!(l.covers(at(30.0)) && l.covers(Vec3::new(30.0, 0.0, 8.0)));
        assert!(!l.covers(Vec3::new(30.0, 0.0, 15.0)), "28 degrees off");
        assert!(!l.covers(at(60.0)), "past his reach");
        assert!(!l.covers(at(-30.0)), "behind him");
        // He faces down it.
        let d = l.facing_dir();
        assert!((d[0] - 1.0).abs() < 1e-4 && d[1].abs() < 1e-4, "{d:?}");
    }

    #[test]
    fn one_job_a_shooter_and_a_third_of_the_team_at_most() {
        // Two spots of one shooter, a third of another, far apart.
        let mut a = line(1, at(0.0), &[bearing(at(0.0), at(30.0))]);
        let mut b = line(2, Vec3::new(0.0, 0.0, 40.0), &[bearing(Vec3::new(0.0, 0.0, 40.0), Vec3::new(30.0, 0.0, 40.0))]);
        let mut c = line(3, Vec3::new(0.0, 0.0, -40.0), &[bearing(Vec3::new(0.0, 0.0, -40.0), Vec3::new(30.0, 0.0, -40.0))]);
        (a.shooter, b.shooter, c.shooter) = (7, 7, 9);
        let held = [a, b, c];
        let mates: Vec<Mate> = (10..16).map(|place| Mate { place, feet: Vec3::new(-20.0, 0.0, place as f32), free: true, playing: None }).collect();
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        let team = |size, objective_areas| JobTeam { mates: &mates, size, objective_areas };
        // A 12-player team: 4 bots in jobs; shooter 7 has one job, shooter 9 the other.
        jobs_tick(&mut jobs, &mut rested, &held, &held, Opening::Staffed, &team(12, &[]), &unheld, 1_000);
        let mut shooters: Vec<Place> = jobs.iter().map(|j| j.shooter).collect();
        shooters.sort_unstable();
        assert_eq!(shooters, [7, 9]);
        let in_jobs = |jobs: &[Job]| jobs.iter().map(|j| usize::from(j.flanker.is_some()) + usize::from(j.pinner.is_some())).sum::<usize>();
        assert_eq!(in_jobs(&jobs), 4);
        // A 6-player team: 2 bots in jobs at most (one job staffed).
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        jobs_tick(&mut jobs, &mut rested, &held, &held, Opening::Staffed, &team(6, &[]), &unheld, 1_000);
        assert_eq!(in_jobs(&jobs), 2);
        assert_eq!(jobs.len(), 1, "the other opened job had nobody and is not kept: {jobs:?}");
        // Shooter 9's lanes cover an objective area: clearing him is objective work, outside the cap.
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        jobs_tick(&mut jobs, &mut rested, &held, &held, Opening::Staffed, &team(6, &[Vec3::new(30.0, 0.0, -40.0)]), &unheld, 1_000);
        assert_eq!(in_jobs(&jobs), 4);
        assert!(jobs.iter().any(|j| j.objective && j.shooter == 9), "{jobs:?}");
    }

    #[test]
    fn a_job_opens_only_on_a_hold_of_two_kills() {
        let one = line(4, at(0.0), &[bearing(at(0.0), at(30.0))]);
        let two = line(5, at(50.0), &[bearing(at(50.0), at(80.0)), bearing(at(50.0), at(70.0))]);
        let held = [one, two];
        let ids = |min| holds(&held, min).iter().map(|l| l.id).collect::<Vec<_>>();
        assert_eq!(ids(2), [5], "one kill and a sighting is no hold");
        assert_eq!(ids(1), [4, 5]);
        assert!(ids(3).is_empty());
    }

    #[test]
    fn a_spot_no_bot_can_take_keeps_no_slot_from_one_that_can() {
        // Lanes along +x; the team's bots stand at x = -20, out of them, near the spots at the origin.
        let lane = |at: Vec3| [bearing(at, at + Vec3::new(30.0, 0.0, 0.0)), bearing(at, at + Vec3::new(30.0, 0.0, 1.0))];
        let spot = |id, at: Vec3, shooter| HeldLine { shooter, ..line(id, at, &lane(at)) };
        let far = Vec3::new(0.0, 0.0, 500.0);
        let mates: Vec<Mate> = (10..16).map(|place| Mate { place, feet: Vec3::new(-20.0, 0.0, place as f32), free: true, playing: None }).collect();
        let team = JobTeam { mates: &mates, size: 12, objective_areas: &[] };
        let ids = |jobs: &[Job]| jobs.iter().map(|j| j.spot).collect::<Vec<_>>();
        // A far spot first in the list: both slots still go to the spots the bots can reach.
        let held = [spot(2, far, 8), spot(3, at(0.0), 9), spot(4, Vec3::new(0.0, 0.0, 30.0), 10)];
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        let before = counted()[Count::JobNotOpened as usize];
        jobs_tick(&mut jobs, &mut rested, &held, &held, Opening::Staffed, &team, &unheld, 1_000);
        assert_eq!(ids(&jobs), [3, 4]);
        assert_eq!(counted()[Count::JobNotOpened as usize] - before, 1, "the far spot, once this tick");
        assert!(rested.is_empty(), "a spot never opened does not rest: {rested:?}");
        // Off (the old rule): the far spot holds a slot through the tick, and only one job opens.
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        jobs_tick(&mut jobs, &mut rested, &held, &held, Opening::ThenDropped, &team, &unheld, 1_000);
        assert_eq!(ids(&jobs), [3]);
        // A camper who moved: his first spot out of reach does not take his one job from the second.
        let held = [spot(5, far, 7), spot(6, at(0.0), 7)];
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        jobs_tick(&mut jobs, &mut rested, &held, &held, Opening::Staffed, &team, &unheld, 1_000);
        assert_eq!(ids(&jobs), [6]);
    }

    #[test]
    fn a_role_goes_to_no_bot_in_a_held_line_of_one_kill() {
        // The hold (two kills) at the origin, lanes along +x; a line of one kill from (-40, 0, 0) along +x covers the bot
        // nearest the hold (12 m), which K1 keeps its routes out of as well.
        let hold = line(4, at(0.0), &[bearing(at(0.0), at(30.0)), bearing(at(0.0), Vec3::new(30.0, 0.0, 1.0))]);
        let from = Vec3::new(-40.0, 0.0, 0.0);
        let one = line(9, from, &[bearing(from, Vec3::new(-10.0, 0.0, 0.0))]);
        let mate = |place, x: f32, z: f32| Mate { place, feet: Vec3::new(x, 0.0, z), free: true, playing: None };
        let mates = [mate(1, -12.0, 0.0), mate(2, -10.0, 30.0), mate(3, -10.0, -30.0)];
        let team = JobTeam { mates: &mates, size: 10, objective_areas: &[] };
        let (holds, lanes) = ([hold.clone()], [hold, one]);
        assert!(lanes[1].covers(mates[0].feet), "the test's bot stands in the one-kill line");
        assert!(!lanes.iter().any(|l| l.covers(mates[1].feet) || l.covers(mates[2].feet)), "the others in none");
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        jobs_tick(&mut jobs, &mut rested, &holds, &lanes, Opening::Staffed, &team, &unheld, 1_000);
        let given: Vec<Place> = jobs.iter().flat_map(|j| [j.flanker, j.pinner]).flatten().map(|h| h.place).collect();
        assert!(!given.contains(&1), "{given:?}");
        assert_eq!(given.len(), 2, "{given:?}");
    }

    #[test]
    fn a_held_spot_gets_a_flanker_and_a_pinner_out_of_its_lanes() {
        let held = vec![line(4, at(0.0), &[bearing(at(0.0), at(30.0))])];
        let mate = |place, x: f32, z: f32, free| Mate { place, feet: Vec3::new(x, 0.0, z), free, playing: None };
        let mates = [
            // In his lane: not chosen.
            mate(1, 20.0, 0.0, true),
            mate(2, 0.0, 30.0, true),
            mate(3, -10.0, 40.0, true),
            // Carrying: not free.
            mate(5, 0.0, 10.0, false),
            // Too far.
            mate(6, 0.0, 95.0, true),
        ];
        let holders = |jobs: &[Job]| (jobs[0].flanker.map(|h| h.place), jobs[0].pinner.map(|h| h.place));
        let tick = |jobs: &mut Vec<Job>, rested: &mut Vec<(u32, u32)>, held: &[HeldLine], mates: &[Mate], now| {
            jobs_tick(jobs, rested, held, held, Opening::Staffed, &JobTeam { mates, size: 10, objective_areas: &[] }, &unheld, now)
        };
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        let events = tick(&mut jobs, &mut rested, &held, &mates, 1_000);
        assert_eq!(holders(&jobs), (Some(2), Some(3)));
        assert_eq!(events.len(), 2, "{events:?}");
        // Both take their plays: kept past the start time.
        let playing = [
            Mate { playing: Some((Role::Flanker, 4)), ..mates[1] },
            Mate { playing: Some((Role::Pinner, 4)), ..mates[2] },
            mates[0],
            mate(8, 5.0, -20.0, true),
        ];
        tick(&mut jobs, &mut rested, &held, &playing, 1_000 + JOB_START_MS + 1);
        assert_eq!(holders(&jobs), (Some(2), Some(3)));
        // The pinner starts carrying: it lets the role go and another takes it.
        let carrying = [playing[0], Mate { free: false, ..playing[1] }, playing[2], playing[3]];
        let events = tick(&mut jobs, &mut rested, &held, &carrying, 7_000);
        assert_eq!(holders(&jobs), (Some(2), Some(8)));
        assert!(events.contains(&JobEvent::Passed { spot: 4, role: Role::Pinner, place: 3, why: PassedWhy::NotFree }), "{events:?}");
        // The new pinner never takes the play: it lets it go after the start
        // time and is not offered it again; nobody else is free: the flanker holds on.
        let events = tick(&mut jobs, &mut rested, &held, &carrying, 7_000 + JOB_START_MS);
        assert_eq!(holders(&jobs), (Some(2), None));
        assert!(events.contains(&JobEvent::Passed { spot: 4, role: Role::Pinner, place: 8, why: PassedWhy::NoPlay }), "{events:?}");
        // The flanker goes down (no longer among the mates): the job ends at
        // once, nobody is sent after him, and the spot rests.
        let fewer = [playing[1], playing[2], playing[3]];
        let events = tick(&mut jobs, &mut rested, &held, &fewer, 8_000);
        assert!(jobs.is_empty(), "{jobs:?}");
        assert_eq!(
            events,
            vec![JobEvent::Ended { spot: 4, shooter: 7, end: JobEnd::HolderDown { role: Role::Flanker, place: 2 }, life_ms: 7_000 }]
        );
        assert!(tick(&mut jobs, &mut rested, &held, &mates, 8_000 + JOB_REST_MS - 1).is_empty(), "resting");
        // Held again after its rest; then no longer held: it ends with the reason given.
        tick(&mut jobs, &mut rested, &held, &mates, 8_000 + JOB_REST_MS);
        assert_eq!(jobs.len(), 1);
        let events = tick(&mut jobs, &mut rested, &[], &mates, 30_000);
        assert!(jobs.is_empty());
        assert_eq!(events, vec![JobEvent::Ended { spot: 4, shooter: 7, end: JobEnd::Faded, life_ms: 2_000 }]);
        // A job runs out.
        tick(&mut jobs, &mut rested, &held, &mates, 30_000 + JOB_REST_MS);
        let events = tick(&mut jobs, &mut rested, &held, &playing, 30_000 + JOB_REST_MS + JOB_MS);
        assert!(jobs.is_empty() && events.iter().any(|e| matches!(e, JobEvent::Ended { end: JobEnd::Time, .. })), "{events:?}");
        // Nobody to hold it: no job is opened at all.
        let (mut jobs, mut rested) = (Vec::new(), Vec::new());
        assert!(tick(&mut jobs, &mut rested, &held, &[mates[3], mates[4]], 1_000).is_empty());
        assert!(jobs.is_empty() && rested.is_empty());
        let picture = Picture {
            held: held.clone(),
            jobs: vec![Job {
                spot: 4,
                objective: false,
                shooter: 0,
                since: 0,
                flanker: Some(Holder { place: 2, since: 0 }),
                pinner: None,
                passed: Vec::new(),
            }],
            sniper_regions: Vec::new(),
        };
        assert!(matches!(picture.job_of(2), Some((_, Role::Flanker, l)) if l.id == 4));
        assert!(picture.job_of(3).is_none());
        assert!(picture.line_at(at(30.0)).is_some() && picture.line(4).is_some() && picture.line_by_spot(at(0.0)).is_some());
    }

    /// As it was (c3dc7241): every lane's apex is the spot's first point,
    /// each bearing taken from its own kill's place; and a kill adds to the
    /// first spot within reach, not the nearest.
    #[test]
    fn the_point_model_casts_and_merges_as_before() {
        // Kills from (0, 0) and (5, 0): the second's victim due +z of its
        // shooter, so its bearing is 0 from there.
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 500, OLD);
        spots.died(Vec3::new(5.0, 0.0, 30.0), 7, at(5.0), 0, 600, OLD);
        assert_eq!(spots.spots.len(), 1);
        let s = &spots.spots[0];
        assert_eq!(s.at, at(0.0));
        let lanes = s.lanes(OLD);
        assert!(lanes.iter().all(|&(apex, _)| apex == at(0.0)), "{lanes:?}");
        assert!(lanes[1].1.abs() < 1e-5, "bearing from its own kill's place: {lanes:?}");
        // So a point due +z of the spot is in the second lane, as it was.
        let line = &spots.held_lines(600, OLD)[0];
        assert!(line.covers(Vec3::new(0.0, 0.0, 20.0)));
        // The merge: spots at 0 and 9, a kill at 5 adds to the first.
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 0, OLD);
        spots.died(at(40.0), 7, at(9.0), 0, 0, OLD);
        assert_eq!(spots.spots.len(), 2);
        spots.died(at(35.0), 7, at(5.0), 0, 0, OLD);
        assert_eq!(spots.spots[0].kill_count(), 2, "the first in the list");
        // The area model: the nearest.
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 0, NEW);
        spots.died(at(60.0), 7, at(12.0), 0, 0, NEW);
        spots.died(at(50.0), 7, at(8.0), 0, 0, NEW);
        assert_eq!(spots.spots[1].kill_count(), 2, "the nearest");
    }

    #[test]
    fn the_costed_corridor_goes_round_covered_regions_when_they_cost() {
        use crate::v026g::bots::tactics::tests::field_with;
        use mapdata::regions::Regions;
        // An 80 x 40 m field, a wall at x 40 from z 0 to 25: the way from
        // (10, 5) to (70, 5) goes through the gap, z 25 to 40.
        let world = field_with(|_| {});
        let grid = &world.grid;
        let regions = std::sync::Arc::new(Regions::build(grid, None));
        let (a, b) = (grid.locate(Vec3::new(10.0, 0.0, 5.0), 1.5).unwrap(), grid.locate(Vec3::new(70.0, 0.0, 5.0), 1.5).unwrap());
        // The near half of the gap covered.
        let lane: Vec<u32> = (0..regions.len() as u32)
            .filter(|&r| {
                let m = grid.position(regions.middle(r));
                (30.0..50.0).contains(&m.x) && (24.0..31.0).contains(&m.z)
            })
            .collect();
        assert!(!lane.is_empty());
        let of = regions.clone();
        let covered = Covered::build(lane.clone(), regions.len(), std::sync::Arc::new(move |n| of.of(n)));
        let room = |cost| costed_corridor(grid, &regions, regions.of(a), regions.of(b), &covered, &[], cost, |_| true).unwrap();
        let through = room(0.0);
        let round = room(100.0);
        assert!(lane.iter().any(|&r| through[r as usize]), "free to go through, it does");
        assert!(lane.iter().all(|&r| !round[r as usize]), "costly: round them");
        assert!(round[regions.of(a) as usize] && round[regions.of(b) as usize]);
        // A free region is no cost.
        let freed = costed_corridor(grid, &regions, regions.of(a), regions.of(b), &covered, &lane, 100.0, |_| true).unwrap();
        assert!(lane.iter().any(|&r| freed[r as usize]));
        // A route kept to that room never steps into the lane.
        let filter = crate::v026g::bots::route_filter::RouteFilter::open()
            .keeping_to(std::sync::Arc::new(round), { let of = regions.clone(); std::sync::Arc::new(move |n| of.of(n)) });
        let route = crate::v026g::bots::path::plan_in(grid, &regions, a, b, |n| filter.floor_open(grid, n), |r| filter.region_open(r)).unwrap();
        assert!(route.points().iter().all(|p| !covered.exposed_at(grid, p.at)), "{route:?}");
    }

    #[test]
    fn covered_floors_follow_their_regions() {
        let covered = Covered::build(vec![2, 2, 5], 6, std::sync::Arc::new(|n| n / 2));
        assert!(covered.region(2) && covered.region(5) && !covered.region(3));

        assert!(covered.floor(4) && covered.floor(5) && !covered.floor(6) && !covered.floor(99));
        assert_eq!(covered.regions(), &[2, 5]);
    }
}
