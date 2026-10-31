//! Play 9: Crossfire (`player_bots_behaviour.md` §3): two bots cover one
//! choke from angles at least 60 degrees apart, so whoever comes through
//! is under fire from two sides and cannot face both.
//!
//! A choke is an exit of the enemy base (`Tactics::exits`) or a known
//! enemy's last place. Each choke has two slots (A and B), so two bots at
//! most cover one. The play does not know which slot its bot holds, nor
//! reads the other bot's play: it reads where the teammates stand (a
//! teammate's place is known). A teammate already in the choke's ring is
//! the other half: the spot is chosen at least `apart_deg` round the choke
//! from him. Two bots starting at once may pick close spots; then the one
//! of the higher place picks again (`Hold` and `Going` check it).
//!
//! The spot: 15-30 m from the choke with a sight line to it, higher ground
//! preferred, never in a spawn area. Held crouched, looking at the choke,
//! for a drawn while.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input, Score};
use crate::v026g::bots::tactics::{SpotQuery, Tactics};
use crate::v026g::bots::threat::{HeldLine, Role};

/// The slot keys of the team job's pinners (K2) start here: one pinner a
/// held spot.
pub const JOB_KEY: u32 = 2_000_000;
/// A pinner of a job holds above this health: the play's own gate.
const JOB_HEALTH: f32 = 0.35;
/// The pinner's spot lies this far inside the held spot's reach at most, m:
/// at the edge of where he kills (his longest kill and 15 m more).
pub const PIN_DEPTH_M: f32 = 15.0;

pub const NUMBERS: &[(&str, f32)] = &[
    // The spot: this far from the choke, metres.
    ("spot_min_m", 15.0),
    ("spot_max_m", 30.0),
    // Two spots at least this far apart round the choke, degrees.
    ("apart_deg", 60.0),
    // A teammate this near the choke is the other half, metres.
    ("partner_m", 35.0),
    // Higher ground: weight per metre above the choke.
    ("prefer_high", 0.3),
    // Spots on the storeys over the anchor are looked at up to this many metres
    // over it (0: only the floor the grid finds, at most 2 m over it). GUESS: 8 m,
    // a storey or two (the players' heights span 9-18 m a map); V-S2a.
    ("up_m", 8.0),
    // A known enemy's last place is a choke this near the bot, metres.
    ("enemy_choke_m", 60.0),
    ("reach_m", 2.0),
    ("sneak_m", 8.0),
    // Held this long, seconds: Team Deathmatch (long holds pay there),
    // the battery modes (brief).
    ("hold_tdm_min_s", 20.0),
    ("hold_tdm_max_s", 40.0),
    ("hold_batteries_min_s", 10.0),
    ("hold_batteries_max_s", 20.0),
    // The score times this by mode.
    ("weight_tdm", 1.0),
    ("weight_batteries", 0.7),
    // Timed batteries: as the plain ones until a bench says otherwise (it was
    // `weight_batteries` there).
    ("weight_timed", 0.7),
    ("give_up_s", 70.0),
    // A spot too near the other half's is chosen again at most this often.
    ("repick_every_s", 3.0),
    ("samples", 64.0),
    // A choke's spots (in sight of it, out of the spawn areas) are looked
    // for once and kept this long for the whole team, seconds: the play is
    // offered only where one is fine for the bot, as the commander polls
    // each second. GUESS, as HoldExit's.
    ("spots_keep_s", 15.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            considerations::personality(),
            considerations::holding(),
            considerations::not_behind_late(),
            considerations::near(200.0),
            considerations::health_above(0.35),
            // Best with one friend there already (the other half); two or
            // more there is crowded.
            Consideration::new(
                Input::FriendsNearTarget,
                0.0,
                3.0,
                Curve::Piecewise {
                    points: vec![(0.0, 0.6), (0.34, 1.0), (0.67, 0.3), (1.0, 0.1)],
                },
            ),
            considerations::cooldown(30.0),
            considerations::runtime(60.0),
        ],
        numbers: Default::default(),
    }
}

/// The candidate's score times the mode's weight (`weight_tdm`,
/// `weight_batteries`, `weight_timed`): the players' research favours the
/// play in one mode.
fn weighed(cx: &Ctx, kind: PlayKind, mut c: Candidate) -> Option<Candidate> {
    let weight = cx.num(kind, cx.view.objective().weight_key());
    c.score = Score::new(c.score.get() * weight);
    (!c.score.is_zero()).then_some(c)
}

/// The choke's number for its slots.
fn choke_key(target: TargetRef) -> Option<u32> {
    match target {
        TargetRef::Exit(index) => Some(index as u32),
        TargetRef::Place(who) => Some(1_000 + who as u32),
        _ => None,
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    // [K2] The team's job on a held spot gives this bot the pin: hold him
    // from the edge of his reach, out of his lanes, while the flanker goes
    // round (F.E.A.R.'s suppress while others flank).
    // Offered only where a spot is fine for it now, as the plain one
    // (taking it to find none fails at once, then the cooldown).
    if cx.tuning.threat.job
        && let Some((job, Role::Pinner, line)) = cx.team.threat.job_of(cx.me.place)
    {
        let choke = Choke { at: line.at, job: job_at(cx, TargetRef::ThreatSpot(line.id), line.at) };
        // With none out of the lanes, a peek spot in his own (the pin from cover).
        let has_spot = matches!(cx.bot.current, Some((PlayKind::Crossfire, TargetRef::ThreatSpot(id), _)) if id == line.id)
            || has_fine_spot(cx, &choke)
            || has_peek_spot(cx, &choke);
        #[cfg(test)]
        if !has_spot {
            crate::v026g::bots::threat::no_spot_why(cx.me.place, Role::Pinner, why_no_fine_spot(cx, &choke));
            crate::v026g::bots::threat::no_spot_centre(Role::Pinner, &cx.tactics.world.grid, line.at);
        }
        out.extend(cx.job_offer(Role::Pinner, PlayKind::Crossfire, TargetRef::ThreatSpot(line.id), JOB_KEY + job.spot, JOB_HEALTH, has_spot));
    }
    // A crossfire needs two: some teammate alive.
    if cx.view.team(cx.me.team).all(|p| p.place == cx.me.place) {
        return;
    }
    let mut chokes: Vec<TargetRef> = Vec::new();
    if let Some(enemy) = cx.bot.side.and_then(|side| cx.enemy_side(side)) {
        chokes.extend((0..cx.tactics.exits(enemy).len()).map(TargetRef::Exit));
    }
    let far = cx.num(PlayKind::Crossfire, "enemy_choke_m");
    for track in cx.memory.tracks() {
        // A last known place, not one seen now: the reflex has those.
        if cx.memory.seen(track.who, cx.now, cx.view.clock).is_none()
            && track.at.distance_xz(cx.me.feet) <= far
            && !cx.tactics.in_spawn_area(track.at, None)
        {
            chokes.push(TargetRef::Place(track.who));
        }
    }
    // The running one stays while it has a place.
    let running = match cx.bot.current {
        Some((PlayKind::Crossfire, t, _)) => Some(t),
        _ => None,
    };
    if let Some(t) = running
        && !chokes.contains(&t)
        && choke_key(t).is_some()
        && cx.point_of(t).is_some()
    {
        chokes.push(t);
    }
    // ... and is the only one offered: a crossfire is held on its choke
    // until it ends, not moved to each enemy's newer last place. Moved, the
    // bots chased (bench, one side so: deaths past half way +8 a run and
    // K/D −0.047 ± 0.022; kept: K/D +0.016 ± 0.024; §3.2).
    if let Some(t) = running {
        chokes.retain(|c| *c == t);
    }
    for choke in chokes {
        let Some(key) = choke_key(choke) else {
            continue;
        };
        // Offered only where a spot is fine for it now: taking it to find
        // none ended it within the second (the audit's churn: "no spot
        // round the choke", mostly round a known enemy's last place).
        if running != Some(choke)
            && choke_of(cx, choke).is_none_or(|c| !has_fine_spot(cx, &c))
        {
            continue;
        }
        for half in 0..2 {
            out.extend(
                cx.candidate(PlayKind::Crossfire, choke, Some(key * 2 + half))
                    .and_then(|c| weighed(cx, PlayKind::Crossfire, c)),
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// No spot was found.
    NoSpot,
    Going {
        spot: Vec3,
        checked: u32,
    },
    Hold {
        spot: Vec3,
        until: u32,
        checked: u32,
    },
    /// A job's pin from cover ([`PinFromCover`]): walking to its peek spot.
    ///
    /// [`PinFromCover`]: crate::v026g::bots::tuning::PinFromCover
    PeekGoing {
        spot: Vec3,
    },
    /// At the peek spot: hidden or up, and the peeks made.
    Peek {
        spot: Vec3,
        phase: Peek,
        peeks: u32,
    },
}

/// A pin from cover at its spot: crouched out of his sight until `until`, or up looking at his spot until `until`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Peek {
    Hidden { until: u32 },
    Up { until: u32 },
}

/// The degrees between two ground directions.
fn degrees_between(a: [f32; 2], b: [f32; 2]) -> f32 {
    let (la, lb) = (a[0].hypot(a[1]), b[0].hypot(b[1]));
    if la < 1e-4 || lb < 1e-4 {
        return 0.0;
    }
    ((a[0] * b[0] + a[1] * b[1]) / (la * lb))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

/// The degrees round `choke` between `a` and `b`.
fn round(choke: Vec3, a: Vec3, b: Vec3) -> f32 {
    degrees_between(
        [a.x - choke.x, a.z - choke.z],
        [b.x - choke.x, b.z - choke.z],
    )
}

/// The teammates in the choke's ring (the other half): (place, feet).
fn halves(cx: &Ctx, choke: Vec3) -> Vec<(Place, Vec3)> {
    let near = cx.num(PlayKind::Crossfire, "partner_m");
    cx.view
        .team(cx.me.team)
        .filter(|p| p.place != cx.me.place && p.feet.distance_xz(choke) <= near)
        .map(|p| (p.place, p.feet))
        .collect()
}

/// The spots to cover `choke` from that stay so for a while, best first:
/// in sight of it, outside every spawn area; found once for the side and
/// kept `spots_keep_s`.
fn spots_by(cx: &Ctx, choke: &Choke) -> std::sync::Arc<Vec<Vec3>> {
    let num = |name| cx.num(PlayKind::Crossfire, name);
    // A job's pin on a held spot (K2): its own ring, at the edge of his reach.
    let (job, choke) = (choke.job, choke.at);
    let kind = PlayKind::Crossfire as u32 + if job.is_some() { 1_000 } else { 0 };
    let key = Tactics::spot_key(kind, cx.bot.side, choke);
    let keep = (num("spots_keep_s") * 1000.0) as u32;
    cx.tactics.spots_kept(key, cx.now, keep, || {
        let query = choke_query(cx, job, choke);
        let mut rng = Rng::new(key ^ u64::from(cx.now));
        cx.tactics
            .spots(&query, cx.now, query.samples, &mut rng)
            .into_iter()
            .map(|s| s.at)
            .filter(|at| !cx.tactics.in_spawn_area(*at, None))
            .collect()
    })
}

/// The spot query of [`spots_by`] round `choke` (a job's pin ring with `job`).
fn choke_query(cx: &Ctx, job: Option<&HeldLine>, choke: Vec3) -> SpotQuery {
    let num = |name| cx.num(PlayKind::Crossfire, name);
    let (min_m, max_m) = match job {
        Some(line) => pin_ring(cx, line),
        None => (num("spot_min_m"), num("spot_max_m")),
    };
    let threat = &cx.tuning.threat;
    SpotQuery {
        sight_to: Some(choke),
        prefer_high: num("prefer_high"),
        up_m: num("up_m"),
        down_m: if job.is_some() && threat.job_spot_levels { crate::v026g::bots::threat::JOB_LEVELS_M } else { 0.0 },
        side: cx.bot.side,
        samples: num("samples") as usize,
        ..SpotQuery::ring(choke, min_m, max_m)
    }
}

/// What a crossfire covers: the choke's point, and the held line when it is
/// a job's pin (K2, a `TargetRef::ThreatSpot` with `job` on), found by the
/// spot's id once ([`choke_of`]).
#[derive(Clone, Copy)]
struct Choke<'a> {
    at: Vec3,
    job: Option<&'a HeldLine>,
}

/// The choke of `target` now, if it has a place.
fn choke_of<'a>(cx: &Ctx<'a>, target: TargetRef) -> Option<Choke<'a>> {
    let at = cx.point_of(target)?;
    Some(Choke { at, job: job_at(cx, target, at) })
}

/// The held line a crossfire on `target` (at `at`) pins as a job (K2): by
/// the spot's id ([`ThreatTuning::job_line_by_id`]), else by the point.
///
/// [`ThreatTuning::job_line_by_id`]: crate::v026g::bots::tuning::ThreatTuning::job_line_by_id
fn job_at<'a>(cx: &Ctx<'a>, target: TargetRef, at: Vec3) -> Option<&'a HeldLine> {
    let by_point = cx.team.threat.line_by_spot(at);
    #[cfg(test)]
    if by_point.is_some_and(|l| target != TargetRef::ThreatSpot(l.id)) {
        crate::v026g::bots::threat::count(crate::v026g::bots::threat::Count::CrossfireChokeOnSpot);
    }
    let line = if cx.tuning.threat.job_line_by_id {
        match target {
            TargetRef::ThreatSpot(id) => cx.team.threat.line(id),
            _ => None,
        }
    } else {
        by_point
    };
    line.filter(|_| cx.tuning.threat.job)
}

/// The pinner's ring round a held spot: from [`PIN_DEPTH_M`] inside his
/// reach to his reach, never nearer than the play's own nearest.
fn pin_ring(cx: &Ctx, line: &HeldLine) -> (f32, f32) {
    let min_m = cx.num(PlayKind::Crossfire, "spot_min_m");
    let max_m = line.reach_m.max(min_m + 5.0);
    ((max_m - PIN_DEPTH_M).max(min_m), max_m)
}

/// Of [`spots_by`], those this bot may take now, best first: fine goals it
/// has a way to ([`Ctx::reachable`], no search lately proved none),
/// `apart_deg` round the choke from every teammate in its ring.
fn fine_spots(cx: &Ctx, choke: &Choke) -> Vec<Vec3> {
    let apart = cx.num(PlayKind::Crossfire, "apart_deg");
    let others = halves(cx, choke.at);
    let spots = spots_by(cx, choke);
    // The bench's count: the spots of a pick left out for a held line.
    #[cfg(test)]
    if out_of_lines(cx, choke) {
        let refused = spots.iter().filter(|at| cx.team.threat.line_at(**at).is_some()).count();
        crate::v026g::bots::threat::count_n(crate::v026g::bots::threat::Count::CrossfireSpotRefused, refused as u64);
    }
    spots.iter().copied().filter(|at| is_fine(cx, choke, apart, &others, *at)).collect()
}

/// Whether this choke's spots keep out of the held lines (K3; a job's pin
/// always).
fn out_of_lines(cx: &Ctx, choke: &Choke) -> bool {
    cx.tuning.threat.crossfire_out_of_lines || choke.job.is_some()
}

/// Whether [`fine_spots`] has one: it stops at the first, where the list
/// is built whole (the offer only asks whether there is a spot).
fn has_fine_spot(cx: &Ctx, choke: &Choke) -> bool {
    let apart = cx.num(PlayKind::Crossfire, "apart_deg");
    let others = halves(cx, choke.at);
    spots_by(cx, choke)
        .iter()
        .any(|at| is_fine(cx, choke, apart, &others, *at))
}

/// The pin-from-cover rule of the bot's team, when it is on.
fn pin_rule(cx: &Ctx) -> Option<crate::v026g::bots::tuning::PinRule> {
    cx.tuning.threat.pin_from_cover.map(|p| p.rule())
}

/// A job pin's peek spots round his held spot `line` ([`PinFromCover`], kept as the play's spots are): the pin ring
/// sampled `samples_x` times as densely, with no sight filter, then the floors his eye does not see crouched and
/// whose standing eye sees his point, out of every other held spot's lanes (his own are where they lie).
///
/// [`PinFromCover`]: crate::v026g::bots::tuning::PinFromCover
fn peek_spots(cx: &Ctx, line: &HeldLine, rule: crate::v026g::bots::tuning::PinRule) -> std::sync::Arc<Vec<Vec3>> {
    use crate::v026g::bots::view::still_eye;
    let key = Tactics::spot_key(PlayKind::Crossfire as u32 + 2_000, cx.bot.side, line.at);
    let keep = (cx.num(PlayKind::Crossfire, "spots_keep_s") * 1000.0) as u32;
    cx.tactics.spots_kept(key, cx.now, keep, || {
        let base = choke_query(cx, Some(line), line.at);
        let n = base.samples * rule.samples_x as usize;
        let query = SpotQuery { sight_to: None, samples: n, ..base };
        let mut rng = Rng::new(key ^ u64::from(cx.now));
        let his_eye = still_eye(line.at, false);
        let point = line.at + Vec3::new(0.0, 1.0, 0.0);
        let found: Vec<Vec3> = cx
            .tactics
            .spots(&query, cx.now, n, &mut rng)
            .into_iter()
            .map(|s| s.at)
            .filter(|&at| {
                !cx.tactics.in_spawn_area(at, None)
                    && !in_other_lines(cx, line, at)
                    && cx.tactics.sees(still_eye(at, false), point)
                    && !cx.tactics.sees(his_eye, still_eye(at, true))
            })
            .collect();
        pin_tally::add(pin_tally::SEARCHED);
        if !found.is_empty() {
            pin_tally::add(pin_tally::FOUND);
        }
        found
    })
}

/// Whether `at` lies in the lanes of a held spot other than `line`.
fn in_other_lines(cx: &Ctx, line: &HeldLine, at: Vec3) -> bool {
    cx.team.threat.held.iter().any(|l| l.id != line.id && l.covers(at))
}

/// Of a job pin's peek spots, those this bot may take now (as [`is_fine`], his own lanes allowed).
fn fine_peek_spots(cx: &Ctx, choke: &Choke, line: &HeldLine, rule: crate::v026g::bots::tuning::PinRule) -> Vec<Vec3> {
    peek_spots(cx, line, rule)
        .iter()
        .copied()
        .filter(|&at| usable_peek(cx, choke, line, at) && cx.path_dist(at) <= rule.reach_m)
        .collect()
}

/// A peek spot's tests but the walk cap: a fine goal, in no other spot's lanes, not taken, a way to it, the climb to it
/// out of a known enemy's sight.
fn usable_peek(cx: &Ctx, choke: &Choke, line: &HeldLine, at: Vec3) -> bool {
    cx.tactics.fine_goal(at, cx.now, cx.bot.side)
        && !in_other_lines(cx, line, at)
        && !cx.spot_taken(at, super::goals::APART_M)
        && cx.can_go(at)
        && !cx.exposed_climb(at, choke.at)
}

/// Whether a job pin with no fine spot out of the lanes has a peek spot to pin from (the switch on); the bench counts
/// the polls whose peek spots were all past the walk cap.
fn has_peek_spot(cx: &Ctx, choke: &Choke) -> bool {
    match (choke.job, pin_rule(cx)) {
        (Some(line), Some(rule)) => {
            let any = !fine_peek_spots(cx, choke, line, rule).is_empty();
            #[cfg(test)]
            if !any && peek_spots(cx, line, rule).iter().any(|&at| usable_peek(cx, choke, line, at)) {
                pin_tally::add(pin_tally::PAST_REACH);
            }
            any
        }
        _ => false,
    }
}

/// A peek spot for a job pin, among the best three as [`pick`].
fn pick_peek(cx: &Ctx, choke: &Choke, rng: &mut Rng) -> Option<Vec3> {
    let (line, rule) = (choke.job?, pin_rule(cx)?);
    let fine = super::goals::perch_first(cx, choke.at, rng, fine_peek_spots(cx, choke, line, rule));
    let n = fine.len().min(3);
    (n > 0).then(|| fine[rng.below(n)])
}

/// The bench's counts of the pin from cover (test builds; `tests/tallies/owner/pin_cover.rs` reads them): peek-spot
/// searches, those that found one, pins started from a peek spot.
pub mod pin_tally {
    use crate::v026g::bots::view::Place;

    pub const SEARCHED: usize = 0;
    pub const FOUND: usize = 1;
    pub const STARTED: usize = 2;
    /// Pins from cover that reached their peek spot.
    pub const ARRIVED: usize = 3;
    /// The pinners' distances to their peek spots at the start, summed, in decimetres.
    pub const START_DM: usize = 4;
    /// Polls whose usable peek spots were all past the walk cap.
    pub const PAST_REACH: usize = 5;
    #[cfg(test)]
    const N: usize = 6;

    #[cfg(test)]
    thread_local! {
        static COUNTS: std::cell::Cell<[u64; N]> = const { std::cell::Cell::new([0; N]) };
    }

    pub fn add(i: usize) {
        add_n(i, 1);
    }

    #[cfg(test)]
    pub fn add_n(i: usize, n: u64) {
        COUNTS.with(|c| {
            let mut all = c.get();
            all[i] += n;
            c.set(all);
        });
    }

    #[cfg(not(test))]
    pub fn add_n(_: usize, _: u64) {}

    /// The counts so far on this thread (a match runs on one).
    #[cfg(test)]
    pub fn counted() -> [u64; N] {
        COUNTS.with(|c| c.get())
    }

    /// One walk to a peek spot, for the bench's read of walk time against the job's time: when it started, the job's
    /// age then, the distance to the spot at the start and at its last step, and whether it got there.
    #[derive(Clone, Copy, Debug)]
    pub struct Walk {
        pub place: Place,
        pub start: u32,
        pub job_age_ms: u32,
        pub d0: f32,
        pub d_last: f32,
        pub t_last: u32,
        pub arrived: bool,
    }

    #[cfg(test)]
    thread_local! {
        static WALKS: std::cell::RefCell<Vec<Walk>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    #[cfg(test)]
    pub fn walk_start(place: Place, now: u32, job_age_ms: u32, d: f32) {
        WALKS.with(|w| w.borrow_mut().push(Walk { place, start: now, job_age_ms, d0: d, d_last: d, t_last: now, arrived: false }));
    }

    /// The place's latest walk moved on: its distance now, and whether it is there.
    #[cfg(test)]
    pub fn walk_step(place: Place, now: u32, d: f32, arrived: bool) {
        WALKS.with(|w| {
            if let Some(walk) = w.borrow_mut().iter_mut().rev().find(|w| w.place == place) {
                walk.d_last = d;
                walk.t_last = now;
                walk.arrived = arrived;
            }
        });
    }

    #[cfg(not(test))]
    pub fn walk_start(_: Place, _: u32, _: u32, _: f32) {}

    #[cfg(not(test))]
    pub fn walk_step(_: Place, _: u32, _: f32, _: bool) {}

    /// The walks so far on this thread.
    #[cfg(test)]
    pub fn walks() -> Vec<Walk> {
        WALKS.with(|w| w.borrow().clone())
    }
}

/// Why [`has_fine_spot`] found none (the bench's class): the test that left the most of the choke's spots out
/// ([`is_fine`]'s tests, in its order), or none sampled.
#[cfg(test)]
fn why_no_fine_spot(cx: &Ctx, choke: &Choke) -> &'static str {
    let apart = cx.num(PlayKind::Crossfire, "apart_deg");
    let others = halves(cx, choke.at);
    let fails: Vec<&'static str> = spots_by(cx, choke)
        .iter()
        .map(|&at| {
            if !cx.tactics.fine_goal(at, cx.now, cx.bot.side) {
                "not_fine_goal"
            } else if out_of_lines(cx, choke) && cx.team.threat.line_at(at).is_some() {
                "in_held_line"
            } else if !others.iter().all(|(_, o)| round(choke.at, at, *o) >= apart) {
                "near_a_mate_round"
            } else if cx.spot_taken(at, super::goals::APART_M) {
                "taken"
            } else if !cx.can_go(at) {
                "no_way"
            } else {
                "exposed_climb"
            }
        })
        .collect();
    if fails.is_empty() {
        // None sampled: which stage of the spot query lost them (its own rng).
        let mut rng = Rng::new(u64::from(cx.now) ^ 0x5eed);
        return match cx.tactics.spots_why(&choke_query(cx, choke.job, choke.at), cx.now, &mut rng) {
            "no_floor" => "none_sampled_no_floor",
            "floor_off_height" => "none_sampled_floor_off_height",
            "off_ring" => "none_sampled_off_ring",
            "not_walkable" => "none_sampled_not_walkable",
            "not_allowed" => "none_sampled_not_allowed",
            "storey_no_walk" => "none_sampled_storey_no_walk",
            "no_sight" => "none_sampled_no_sight",
            _ => "none_sampled_other",
        };
    }
    crate::v026g::bots::threat::most_failed(&fails)
}

/// Whether the bot may take `at` now: a fine goal it has a way to, `apart`
/// degrees round the choke from every teammate in its ring (`others`).
fn is_fine(cx: &Ctx, choke: &Choke, apart: f32, others: &[(Place, Vec3)], at: Vec3) -> bool {
    cx.tactics.fine_goal(at, cx.now, cx.bot.side)
        // Never in a held spot's lanes (K3, and a job's pin): where he
        // killed is where the next one dies.
        && !(out_of_lines(cx, choke) && cx.team.threat.line_at(at).is_some())
        && others.iter().all(|(_, o)| round(choke.at, at, *o) >= apart)
        && !cx.spot_taken(at, super::goals::APART_M)
        && cx.can_go(at)
        && !cx.exposed_climb(at, choke.at)
}

/// A spot for the choke, `apart_deg` from every teammate in its ring.
fn pick(cx: &Ctx, choke: &Choke, rng: &mut Rng) -> Option<Vec3> {
    let fine = super::goals::perch_first(cx, choke.at, rng, fine_spots(cx, choke));
    // Among the best three, any (not always the best: hard to read).
    let n = fine.len().min(3);
    (n > 0).then(|| fine[rng.below(n)])
}

/// Whether a teammate of a lower place stands within `apart_deg` of `spot`
/// round the choke: this bot then picks again.
fn crowded(cx: &Ctx, choke: Vec3, spot: Vec3) -> bool {
    let apart = cx.num(PlayKind::Crossfire, "apart_deg");
    halves(cx, choke)
        .iter()
        .any(|(place, o)| *place < cx.me.place && round(choke, spot, *o) < apart)
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let Some(choke) = choke_of(cx, target) else {
        return State::NoSpot;
    };
    if let Some(spot) = pick(cx, &choke, rng) {
        return State::Going { spot, checked: cx.now };
    }
    // A job's pin with no spot out of the lanes: from cover in his own.
    match pick_peek(cx, &choke, rng) {
        Some(spot) => {
            pin_tally::add(pin_tally::STARTED);
            let d = cx.me.feet.distance_xz(spot);
            pin_tally::add_n(pin_tally::START_DM, (d * 10.0) as u64);
            let job_age_ms = cx.team.threat.job_of(cx.me.place).map_or(0, |(job, ..)| cx.now.saturating_sub(job.since));
            pin_tally::walk_start(cx.me.place, cx.now, job_age_ms, d);
            State::PeekGoing { spot }
        }
        None => State::NoSpot,
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Crossfire, name);
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return Step::Done("covered the choke long enough");
    }
    let Some(choke) = choke_of(cx, target) else {
        return Step::Done("the choke is gone");
    };
    // The team's job ended: the spot is no longer held.
    if matches!(target, TargetRef::ThreatSpot(_)) && choke.job.is_none() {
        return Step::Done("the spot is no longer held");
    }
    let every = (num("repick_every_s") * 1000.0) as u32;
    let reach = num("reach_m");
    match state {
        // A job's pin has one slot a spot and no other half: a teammate by
        // the spot (its flanker, most often) is no reason to move or end
        // (the bench: pins ended "no spot apart from the other half" 3 s in).
        State::Going { spot, checked } | State::Hold { spot, checked, .. }
            if cx.now >= *checked + every && choke.job.is_none() && crowded(cx, choke.at, *spot) =>
        {
            *checked = cx.now;
            let Some(new) = pick(cx, &choke, rng) else {
                return Step::Failed("no spot apart from the other half");
            };
            *state = State::Going {
                spot: new,
                checked: cx.now,
            };
            Step::Continue(Intent::to(new, cx.pace_toward(new)).within(reach))
        }
        State::NoSpot => Step::Failed("no spot round the choke"),
        State::Going { spot, checked } => {
            // Its spot proved no way to: another of the fine ones (none:
            // kept, and the think leaves the play, `goal_shut`).
            if goals::repick_if_shut(cx, spot, || pick(cx, &choke, rng)) {
                *checked = cx.now;
            }
            let spot = *spot;
            let d = cx.me.feet.distance_xz(spot);
            if d <= reach {
                let (lo, hi) = cx.view.objective().by_mode(
                    (num("hold_tdm_min_s"), num("hold_tdm_max_s")),
                    (num("hold_batteries_min_s"), num("hold_batteries_max_s")),
                    (num("hold_batteries_min_s"), num("hold_batteries_max_s")),
                );
                let s = rng.range(lo, hi);
                *state = State::Hold {
                    spot,
                    until: cx.now + (s * 1000.0) as u32,
                    checked: cx.now,
                };
                return Step::Continue(Intent::stay().crouched().looking(Look::At(choke.at)));
            }
            let pace = if d < num("sneak_m") {
                Pace::Sneak
            } else {
                cx.pace_toward(spot)
            };
            Step::Continue(Intent::to(spot, pace).within(reach * 0.7))
        }
        State::Hold { until, .. } => {
            if cx.now >= *until {
                return Step::Done("held the crossfire");
            }
            Step::Continue(Intent::stay().crouched().looking(Look::At(choke.at)))
        }
        State::PeekGoing { spot } => {
            let Some(rule) = pin_rule(cx) else {
                return Step::Done("the pin from cover is off");
            };
            // Its spot proved no way to: another peek spot (none: kept, and the think leaves the play).
            goals::repick_if_shut(cx, spot, || pick_peek(cx, &choke, rng));
            let spot = *spot;
            let d = cx.me.feet.distance_xz(spot);
            pin_tally::walk_step(cx.me.place, cx.now, d, d <= reach);
            if d <= reach {
                // The first peek on arrival, as the humans' (the crouched hide comes between peeks): the job it
                // serves lasts a median 6.9 s in the bench, shorter than a hide first.
                pin_tally::add(pin_tally::ARRIVED);
                *state = State::Peek { spot, phase: Peek::Up { until: cx.now + rule.peek_ms }, peeks: 1 };
                return Step::Continue(Intent::stay().looking(Look::At(choke.at)));
            }
            // Walked in low and slow: the spot is in his lanes, hidden only crouched.
            let pace = if d < num("sneak_m") { Pace::Sneak } else { cx.pace_toward(spot) };
            Step::Continue(Intent::to(spot, pace).within(reach * 0.7))
        }
        State::Peek { spot: _, phase, peeks } => {
            let Some(rule) = pin_rule(cx) else {
                return Step::Done("the pin from cover is off");
            };
            match *phase {
                Peek::Hidden { until } if cx.now >= until => {
                    if *peeks >= rule.peeks {
                        return Step::Done("pinned him from cover");
                    }
                    *peeks += 1;
                    *phase = Peek::Up { until: cx.now + rule.peek_ms };
                    Step::Continue(Intent::stay().looking(Look::At(choke.at)))
                }
                Peek::Hidden { .. } => Step::Continue(Intent::stay().crouched().looking(Look::At(choke.at))),
                Peek::Up { until } if cx.now >= until => {
                    let (lo, hi) = rule.hide_ms;
                    *phase = Peek::Hidden { until: cx.now + rng.range(lo as f32, hi as f32) as u32 };
                    Step::Continue(Intent::stay().crouched().looking(Look::At(choke.at)))
                }
                // Up: standing, looking at his spot; the reflex fires if he shows.
                Peek::Up { .. } => Step::Continue(Intent::stay().looking(Look::At(choke.at))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::view::Mode;

    fn crossfires(s: &Scenario, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(place), &mut out);
        out
    }

    #[test]
    fn each_exit_has_two_slots_and_a_lone_bot_has_none() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0));
        assert!(crossfires(&s, 0).is_empty(), "no teammate: no crossfire");
        s.bot(2, 0, Vec3::new(20.0, 0.0, 35.0));
        let out = crossfires(&s, 0);
        let exits = s.tactics.exits(1).len();
        assert!(exits > 0);
        assert_eq!(out.len(), exits * 2, "{out:#?}");
        for c in &out {
            assert_eq!(c.rank, Rank::Fight);
            let TargetRef::Exit(i) = c.target else {
                panic!("{c:?}")
            };
            let key = c.slot.unwrap().key;
            assert!(key == i as u32 * 2 || key == i as u32 * 2 + 1, "{c:?}");
        }
        // On cooldown: none.
        s.last_ran.push((PlayKind::Crossfire, s.view.clock - 5_000));
        assert!(crossfires(&s, 0).is_empty());
    }

    #[test]
    fn the_second_bot_covers_the_choke_from_sixty_degrees_off_the_first() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let choke = s.tactics.exits(1)[0];
        // The first half already 20 m from the choke, toward our side.
        let first = Vec3::new(choke.x - 20.0, 0.0, choke.z.min(38.0));
        s.bot(2, 0, first);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 35.0));
        let steps = s.run(0, PlayKind::Crossfire, TargetRef::Exit(0), 80, 300, 3.5);
        let held = steps
            .iter()
            .any(|st| matches!(st, Step::Continue(i) if i.go == Go::Stay && i.crouch && i.look == Look::At(choke)));
        assert!(held, "{steps:?}");
        let at = s.view.places[0].feet;
        let d = at.distance_xz(choke);
        assert!((13.0..=32.0).contains(&d), "{at:?} {d}");
        let apart = round(choke, at, first);
        assert!(apart >= 59.0, "{at:?} {first:?} {apart}");
        assert!(!s.tactics.in_spawn_area(at, None));
    }

    #[test]
    fn team_deathmatch_favours_a_crossfire_and_holds_it_longer() {
        let score = |mode| {
            let mut s = Scenario::field(mode);
            s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0));
            s.bot(2, 0, Vec3::new(20.0, 0.0, 35.0));
            crossfires(&s, 0)[0].score.get()
        };
        let (tdm, batteries) = (score(Mode::TeamDeathmatch), score(Mode::Batteries));
        assert!(batteries < tdm * 0.8, "{tdm} {batteries}");
        // The hold: 20-40 s in Team Deathmatch, 10-20 s in the battery modes.
        for (mode, lo, hi) in [
            (Mode::TeamDeathmatch, 20_000, 40_000),
            (Mode::Batteries, 10_000, 20_000),
        ] {
            let mut s = Scenario::field(mode);
            s.bot(0, 0, Vec3::new(25.0, 0.0, 35.0));
            s.bot(2, 0, Vec3::new(20.0, 0.0, 38.0));
            let mut rng = Rng::new(2);
            s.current = Some((PlayKind::Crossfire, TargetRef::Exit(0), s.view.clock));
            let mut state = start(TargetRef::Exit(0), &s.cx(0), &mut rng);
            let State::Going { spot, .. } = state else {
                panic!("{state:?}")
            };
            s.view.places[0].feet = spot;
            step(&mut state, TargetRef::Exit(0), &s.cx(0), &mut rng);
            let State::Hold { until, .. } = state else {
                panic!("{state:?}")
            };
            let left = until - s.view.clock;
            assert!((lo..=hi).contains(&left), "{mode:?} {left}");
        }
    }

    #[test]
    fn a_running_crossfire_is_offered_on_its_own_choke_only() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0));
        s.bot(2, 0, Vec3::new(20.0, 0.0, 35.0));
        // An enemy heard 30 m off: his last place is a choke too.
        s.enemy(5, 1, Vec3::new(60.0, 0.0, 35.0), Some(Source::Heard));
        let all = crossfires(&s, 0);
        let exit = TargetRef::Exit(0);
        assert!(all.iter().any(|c| c.target == TargetRef::Place(5)), "{all:#?}");
        assert!(all.iter().any(|c| c.target == exit), "{all:#?}");
        // Running on the exit, it is not moved to his place.
        s.current = Some((PlayKind::Crossfire, exit, s.view.clock));
        let out = crossfires(&s, 0);
        assert_eq!(out.len(), 2, "{out:#?}");
        assert!(out.iter().all(|c| c.target == exit), "{out:#?}");
    }

    #[test]
    fn a_crossfire_on_a_last_known_place_outlives_the_memory_of_him_only_when_the_switch_is_on() {
        let place = TargetRef::Place(5);
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0));
        s.bot(2, 0, Vec3::new(20.0, 0.0, 35.0));
        s.enemy(5, 1, Vec3::new(60.0, 0.0, 35.0), Some(Source::Heard));
        let at = s.cx(0).point_of(place).expect("his place is known");
        // Running on it; the memory then forgets him (no track).
        s.current = Some((PlayKind::Crossfire, place, s.view.clock));
        let now = s.view.clock;
        s.memory = Default::default();
        let offered = |s: &Scenario| crossfires(s, 0).iter().any(|c| c.target == place);
        // Off: no point, no choke, the play is no longer offered.
        assert!(s.cx(0).point_of(place).is_none());
        assert!(!offered(&s));
        // On, the choke is kept where the play began, for crossfire_point_s.
        s.tuning.commit.crossfire_point = true;
        s.held_choke = Some((5, at, now));
        assert_eq!(s.cx(0).point_of(place), Some(at));
        assert!(offered(&s));
        // Another enemy's place is not that point.
        assert!(s.cx(0).point_of(TargetRef::Place(6)).is_none());
        // Past the cap it is gone again.
        s.view.clock = now + (s.tuning.commit.crossfire_point_s * 1000.0) as u32 + 1;
        assert!(s.cx(0).point_of(place).is_none());
    }

    /// The offer asks whether a fine spot exists; it says what the list does,
    /// for every exit, with teammates near or far and with every spot proved
    /// no way to (the list then empty).
    #[test]
    fn the_offer_finds_a_spot_when_the_list_has_one() {
        for mate in [Vec3::new(20.0, 0.0, 38.0), Vec3::new(70.0, 0.0, 5.0)] {
            let mut s = Scenario::field(Mode::TeamDeathmatch);
            s.bot(0, 0, Vec3::new(25.0, 0.0, 35.0));
            s.bot(2, 0, mate);
            let mut any = 0;
            for &choke in &s.tactics.exits(1).to_vec() {
                let at = Choke { at: choke, job: None };
                let list = fine_spots(&s.cx(0), &at);
                assert_eq!(has_fine_spot(&s.cx(0), &at), !list.is_empty(), "{choke:?}");
                any += list.len();
                // Every one of them proved no way to: none left, both ways.
                s.no_way_to = list.iter().map(|&at| (at, s.view.clock + 5_000)).collect();
                assert!(fine_spots(&s.cx(0), &at).is_empty(), "{choke:?}");
                assert!(!has_fine_spot(&s.cx(0), &at), "{choke:?}");
                s.no_way_to.clear();
            }
            assert!(any > 0, "the scenario has fine spots");
        }
    }

    #[test]
    fn of_two_bots_too_close_the_higher_place_picks_again() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let choke = s.tactics.exits(1)[0];
        let near = Vec3::new(choke.x - 20.0, 0.0, choke.z.min(38.0));
        s.bot(0, 0, near);
        s.bot(3, 0, near + Vec3::new(0.0, 0.0, -1.0));
        assert!(crowded(&s.cx(3), choke, s.view.places[3].feet));
        assert!(!crowded(&s.cx(0), choke, s.view.places[0].feet));
    }

    /// A spot its bot proved no way to is left for another fine one; with
    /// none left it is kept, for the think to leave the play (`goal_shut`).
    #[test]
    fn a_spot_proved_no_way_to_is_left_for_another() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 35.0));
        s.bot(2, 0, Vec3::new(20.0, 0.0, 38.0));
        let mut rng = Rng::new(2);
        let exit = TargetRef::Exit(0);
        s.current = Some((PlayKind::Crossfire, exit, s.view.clock));
        let mut state = start(exit, &s.cx(0), &mut rng);
        let State::Going { spot: first, .. } = state else {
            panic!("{state:?}")
        };
        s.no_way_to = vec![(first, s.view.clock + 5_000)];
        let st = step(&mut state, exit, &s.cx(0), &mut rng);
        let State::Going { spot, .. } = state else {
            panic!("{state:?}")
        };
        assert!(!s.cx(0).no_way_lately(spot), "{first:?} {spot:?}");
        assert!(
            matches!(st, Step::Continue(Intent { go: Go::To { at, .. }, .. }) if at == spot),
            "{st:?}"
        );
        // Every spot proved no way to: kept.
        let choke = s.tactics.exits(1)[0];
        let until = s.view.clock + 5_000;
        s.no_way_to = spots_by(&s.cx(0), &Choke { at: choke, job: None })
            .iter()
            .map(|&at| (at, until))
            .collect();
        step(&mut state, exit, &s.cx(0), &mut rng);
        assert!(matches!(state, State::Going { spot: kept, .. } if kept == spot), "{state:?}");
    }

    /// A held spot `9` of shooter 5 at `him`, who killed at `victim`, sensed now; the team's jobs on (a job's pin finds
    /// its line by the spot's id) and the pin from cover at its defaults.
    fn held(s: &mut Scenario, him: Vec3, victim: Vec3) {
        let now = s.view.clock;
        s.tuning.threat.job = true;
        s.tuning.threat.job_line_by_id = true;
        s.tuning.threat.pin_from_cover = Some(crate::v026g::bots::tuning::PinFromCover::default());
        std::sync::Arc::make_mut(&mut s.team.threat).held.push(HeldLine::for_test(9, 5, him, &[victim], Some(now)));
    }

    /// Steps a pin from cover at its spot until it ends: the stands it made (when each began), and how it ended.
    fn peek_run(s: &mut Scenario, him: Vec3, spot: Vec3) -> (Vec<u32>, Step) {
        let mut state = State::Peek { spot, phase: Peek::Hidden { until: s.view.clock }, peeks: 0 };
        let mut rng = Rng::new(1);
        let (mut ups, mut was_up) = (Vec::new(), false);
        for _ in 0..3_000 {
            let st = step(&mut state, TargetRef::ThreatSpot(9), &s.cx(0), &mut rng);
            match st {
                Step::Continue(i) => {
                    assert_eq!(i.look, Look::At(him), "{i:?}");
                    assert_eq!(i.go, Go::Stay, "{i:?}");
                    if !i.crouch && !was_up {
                        ups.push(s.view.clock);
                    }
                    was_up = !i.crouch;
                }
                end => return (ups, end),
            }
            s.view.clock += 100;
        }
        panic!("no end: {state:?}")
    }

    /// A pin from cover stands to look at his spot `peeks` times, crouched out of sight between, never more often
    /// than `per_minute`, then ends.
    #[test]
    fn a_pin_from_cover_peeks_a_few_times_crouched_between_then_ends() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let him = Vec3::new(30.0, 0.0, 70.0);
        held(&mut s, him, Vec3::new(30.0, 0.0, 40.0));
        let spot = Vec3::new(30.0, 0.0, 45.0);
        s.bot(0, 0, spot);
        let cover = s.tuning.threat.pin_from_cover.expect("set by held");
        let (ups, end) = peek_run(&mut s, him, spot);
        assert_eq!(ups.len() as u32, cover.peeks, "{ups:?}");
        assert!(matches!(end, Step::Done("pinned him from cover")), "{end:?}");
        let cycle = 60_000 / cover.per_minute;
        assert!(ups.windows(2).all(|w| w[1] - w[0] >= cycle), "{ups:?} under {cycle} ms apart");
    }

    /// The peeks-a-minute cap stretches a short hide: a cycle is never under 60 s / `per_minute`.
    #[test]
    fn the_pin_from_cover_rule_keeps_the_peeks_a_minute_under_its_cap() {
        let quick = crate::v026g::bots::tuning::PinFromCover { peek_s: 2.0, hide_min_s: 1.0, hide_max_s: 1.5, per_minute: 8, ..Default::default() };
        let rule = quick.rule();
        assert_eq!(rule.peek_ms, 2_000);
        assert_eq!(rule.hide_ms, (5_500, 5_500), "60 / 8 = 7.5 s a cycle, less the 2 s up");
    }

    /// A peek spot is offered only within a walk that fits a job: 8 s at 1.1 m/s is an 8.8 m way; no time, no way.
    #[test]
    fn the_pin_from_cover_walk_cap_is_its_time_at_its_pace() {
        let rule = crate::v026g::bots::tuning::PinFromCover::default().rule();
        assert!((rule.reach_m - 8.8).abs() < 1e-4, "{}", rule.reach_m);
        let none = crate::v026g::bots::tuning::PinFromCover { peek_reach_s: 0.0, ..Default::default() };
        assert_eq!(none.rule().reach_m, 0.0);
    }

    /// With the switch off a pin from cover ends at once, and a job pin with no spot out of the lanes is not offered.
    #[test]
    fn with_the_pin_from_cover_off_none_is_played() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let him = Vec3::new(30.0, 0.0, 70.0);
        held(&mut s, him, Vec3::new(30.0, 0.0, 40.0));
        let spot = Vec3::new(30.0, 0.0, 45.0);
        s.bot(0, 0, spot);
        s.tuning.threat.pin_from_cover = None;
        let (ups, end) = peek_run(&mut s, him, spot);
        assert!(ups.is_empty());
        assert!(matches!(end, Step::Done("the pin from cover is off")), "{end:?}");
        let choke = Choke { at: him, job: s.team.threat.line(9) };
        assert!(!has_peek_spot(&s.cx(0), &choke));
    }
}
