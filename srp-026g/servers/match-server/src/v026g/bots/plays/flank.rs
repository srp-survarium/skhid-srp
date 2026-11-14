//! Play 8: Read a held angle (`player_bots_behaviour.md` §3): an enemy
//! crouched (key `CROUCH`), aiming (key `AIM`) and still, seen so a while,
//! is holding an angle. Do not walk into it: flank him from a spot 15-30 m
//! from him well off the way he faces, reached by a way point hidden from
//! him; or, with no such spot, wait out of his line and watch him.
//!
//! Fairness (§2): his keys, speed and facing are read only while he is
//! seen (`Memory::seen`), so the candidate needs a sight; the facing is
//! taken once at the start and kept in the state (the direction he faced
//! when last seen). After that the play goes by his last known place
//! (`Ctx::known_place`). It never aims or shoots: at the spot it looks at
//! him and the reflex does the rest.
//!
//! "A while" is the sight's length (`Track::sight_since`) with the keys and
//! speed of now: the view keeps no history of his keys, and a sight broken
//! starts the count again.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input, Score};
use crate::v026g::bots::kin;
use crate::v026g::bots::tactics::SpotQuery;
use crate::v026g::bots::threat::Role;
use crate::v026g::drive::key;

/// The slot keys of the team job's flanks (K2) start here: one flanker a
/// held spot, apart from the flanks of a seen holder (keyed by his place).
pub const JOB_KEY: u32 = 1_000_000;
/// A flanker of a job goes above this health: the play's own gate.
const JOB_HEALTH: f32 = 0.4;

pub const NUMBERS: &[(&str, f32)] = &[
    // He counts as still under this, m/s.
    ("still_speed", 0.5),
    // Seen holding at least this long, seconds.
    ("held_s", 1.5),
    // Only an enemy this near is read, metres.
    ("max_m", 70.0),
    // The flank spot: this far from him, metres.
    ("spot_min_m", 15.0),
    ("spot_max_m", 30.0),
    // The flank spot lies at least this far off his facing, degrees.
    ("off_facing_deg", 70.0),
    // The flanks are looked for this much farther round, degrees.
    ("flank_extra_deg", 25.0),
    // The way point: this far from the spot, hidden from him, metres.
    ("via_min_m", 6.0),
    ("via_max_m", 15.0),
    // Crouched for the last metres to the spot.
    ("sneak_m", 10.0),
    ("reach_m", 2.0),
    // At the spot: watching him this long, seconds.
    ("peek_min_s", 3.0),
    ("peek_max_s", 6.0),
    // No flank spot: waiting this long, seconds.
    ("wait_min_s", 4.0),
    ("wait_max_s", 8.0),
    ("give_up_s", 35.0),
    // The score times this by mode: Team Deathmatch is a fight for the
    // map, the battery modes a race for batteries.
    ("weight_tdm", 1.0),
    ("weight_batteries", 0.8),
    // Timed batteries: as the plain ones until a bench says otherwise (it was
    // `weight_batteries` there).
    ("weight_timed", 0.8),
    // Spot samples per query.
    ("samples", 64.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            considerations::personality(),
            considerations::pushing(),
            considerations::near(100.0),
            considerations::health_above(0.4),
            // Fewer friends already by him: a flank pays where no one else is.
            Consideration::new(Input::FriendsNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
            considerations::cooldown(15.0),
            considerations::runtime(40.0),
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

/// Whether the enemy at `who` is seen holding an angle now.
fn holding(cx: &Ctx, who: Place) -> bool {
    let num = |name| cx.num(PlayKind::Flank, name);
    let Some(seen) = cx.memory.seen(who, cx.now, cx.view.clock) else {
        return false;
    };
    let Some(p) = cx.view.place(who).filter(|p| p.alive) else {
        return false;
    };
    let held_ms = (num("held_s") * 1000.0) as u32;
    let long = seen
        .track()
        .sight_since
        .is_some_and(|since| cx.now >= since + held_ms);
    long && p.keys & key::CROUCH != 0
        && p.keys & key::AIM != 0
        && p.speed() < num("still_speed")
        && seen.at().distance_xz(cx.me.feet) <= num("max_m")
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    // [A320 bench] A route walker no longer on its route's flank: what took it (another play, or the job / its held spot gone).
    #[cfg(test)]
    {
        let walking = ROUTE_SEEN.with(|m| m.borrow().contains_key(&cx.me.place));
        if walking {
            let why = match cx.bot.current {
                // Still on this flank: with camp_flank on, a walk goes on past its job's end by design (not a leave).
                Some((PlayKind::Flank, TargetRef::ThreatSpot(id), _)) => {
                    let line = cx.team.threat.line(id).is_some();
                    let role = matches!(cx.team.threat.job_of(cx.me.place), Some((_, Role::Flanker, l)) if l.id == id);
                    (!role && cx.tuning.threat.camp_flank.is_none())
                        .then(|| format!("job role gone (held spot {})", if line { "still held" } else { "gone" }))
                }
                Some((kind, _, _)) => Some(format!("another play {kind:?}")),
                // A bot is asked again only alive: a life begun after its last step of the walk is a death on the way.
                None => {
                    let died = FLANKING.with(|m| m.borrow().get(&cx.me.place).is_some_and(|&t| cx.me.spawned > t));
                    Some(if died { "no play: died on the way" } else { "no play: the play ended" }.to_string())
                }
            };
            if let Some(why) = why {
                ROUTE_SEEN.with(|m| m.borrow_mut().remove(&cx.me.place));
                eprintln!("A320 camp_flank route left: t {} place {} {why}", cx.now, cx.me.place);
            }
        }
    }
    // [K2] The team's job on a held spot gives this bot the flank: the
    // kills tell where he is and which way he faces, not his keys (a human
    // camper stands and peeks, RC1). The gate below stays a second trigger.
    if cx.tuning.threat.job
        && let Some((job, Role::Flanker, line)) = cx.team.threat.job_of(cx.me.place)
    {
        let has_spot = matches!(cx.bot.current, Some((PlayKind::Flank, TargetRef::ThreatSpot(id), _)) if id == line.id)
            || camp_spot_of(cx, line).is_some()
            || !job_flank_spots(cx, line).is_empty();
        #[cfg(test)]
        if !has_spot {
            let why = JOB_SPOTS_WHY.with(|m| m.borrow().get(&(cx.bot.side, line.id)).copied()).unwrap_or("unknown");
            crate::v026g::bots::threat::no_spot_why(cx.me.place, Role::Flanker, why);
            crate::v026g::bots::threat::no_spot_centre(Role::Flanker, &cx.tactics.world.grid, line.at);
        }
        out.extend(cx.job_offer(Role::Flanker, PlayKind::Flank, TargetRef::ThreatSpot(line.id), JOB_KEY + job.spot, JOB_HEALTH, has_spot));
    }
    // [A320 `threat.camp_flank`] The job ended (his death, or the team no longer senses him: B3g2b, 14 of 41 walks left their
    // route so) with this bot on its flank route or at its firing floor: the flank stays offered to it, so it goes on to the
    // known camp spot (and holds it for his return with `hold_ms`); the play's step ends it at the route's or the hold's end.
    // So also when its role went with the spot still held: the job's time (`threat::JOB_MS`, 45 s) is shorter than many
    // routes (B3g8: 100-155 m, 27-80 s to the firing floor). A flank of the old kind ends at its step then (as before).
    let own_role = |id| matches!(cx.team.threat.job_of(cx.me.place), Some((_, Role::Flanker, l)) if l.id == id);
    if cx.tuning.threat.camp_flank.is_some()
        && cx.me.carrying.is_none()
        && !cx.bot.dry
        && let Some((PlayKind::Flank, TargetRef::ThreatSpot(id), _)) = cx.bot.current
        && !own_role(id)
    {
        out.extend(cx.job_candidate(PlayKind::Flank, TargetRef::ThreatSpot(id), JOB_KEY + id, JOB_HEALTH));
    }
    near_fire_alarm(cx, out);
    for track in cx.memory.tracks() {
        let target = TargetRef::Place(track.who);
        let running = matches!(cx.bot.current, Some((PlayKind::Flank, t, _)) if t == target);
        if running || holding(cx, track.who) {
            out.extend(
                cx.candidate(PlayKind::Flank, target, Some(track.who as u32))
                    .and_then(|c| weighed(cx, PlayKind::Flank, c)),
            );
        }
    }
    // [A320 bench] A route walker's own offer, as the commander will weigh it (on a change only): none and why, or its
    // score and band (a running objective under `Bands::keep` of the floor competes in the roam band).
    #[cfg(test)]
    if let Some((PlayKind::Flank, TargetRef::ThreatSpot(id), _)) = cx.bot.current
        && ROUTE_SEEN.with(|m| m.borrow().contains_key(&cx.me.place))
    {
        let offer = out.iter().find(|c| c.kind == PlayKind::Flank && c.target == TargetRef::ThreatSpot(id));
        let now = match offer {
            Some(c) => {
                let band = crate::v026g::bots::team::Bands::used().band(c, true, false, false);
                format!("band {band:?} score {:.2}", c.score.get())
            }
            None => {
                let job = match cx.team.threat.job_of(cx.me.place) {
                    Some((_, Role::Flanker, l)) if l.id == id => "its job role",
                    Some((_, Role::Flanker, _)) => "a flank of another spot",
                    Some(_) => "another role",
                    None if cx.team.threat.line(id).is_some() => "no role, spot still held",
                    None => "no role, spot gone",
                };
                let health = cx.me.health;
                format!("none ({job}, health {health:.2}, carrying {})", cx.me.carrying.is_some())
            }
        };
        let changed = OFFER_SEEN.with(|m| m.borrow_mut().insert(cx.me.place, now.clone()).as_ref() != Some(&now));
        if changed && (offer.is_none() || !now.starts_with("band Objective")) {
            eprintln!("A320 camp_flank offer: t {} place {} {now}", cx.now, cx.me.place);
        }
    }
}

/// [A320 `threat.camp_flank.alarm_near_fire_m`] Near its firing floor a route walker's own flank is in the alarm band (a
/// Trade does not take it there).
fn near_fire_alarm(cx: &Ctx, out: &mut [Candidate]) {
    let Some(camp) = cx.tuning.threat.camp_flank else { return };
    let Some((PlayKind::Flank, target @ TargetRef::ThreatSpot(_), _)) = cx.bot.current else { return };
    let Some(left) = cx.bot.route_left_m else { return };
    let near = camp.alarm_near_fire_m.is_some_and(|near| left <= near);
    // [F1] Inertia: the whole walk in the alarm band, its score raised with the share of the route walked.
    let bonus = camp.inertia.map(|inertia| {
        let walked = cx.bot.route_total_m.filter(|&t| t > 0.0).map_or(0.0, |t| (1.0 - left / t).clamp(0.0, 1.0));
        inertia * walked
    });
    if !near && bonus.is_none() {
        return;
    }
    for c in out.iter_mut().filter(|c| c.kind == PlayKind::Flank && c.target == target) {
        c.rank = c.rank.max(Rank::Alarm);
        if let Some(bonus) = bonus {
            c.score = Score::new(c.score.get() + bonus);
        }
    }
}

/// [A320 `threat.camp_flank`] A route's length from its start to its firing floor, m (a hold's: its route's whole); None
/// for a flank of the old kind.
pub fn route_total_m(state: &State, tactics: &crate::v026g::bots::tactics::Tactics) -> Option<f32> {
    let (spot, route) = match state {
        State::Route { spot, route, .. } => (*spot, *route),
        _ => return None,
    };
    let r = tactics.camp_spots.get(spot)?.routes.get(route)?;
    let grid = &tactics.world.grid;
    Some((0..r.fire).map(|i| (grid.position(r.floors[i + 1]) - grid.position(r.floors[i])).length()).sum())
}

/// [A320 `threat.camp_flank`] The metres of route a walker at `feet` has left to its firing floor (0 at it, holding it); None
/// for a flank of the old kind.
pub fn route_left_m(state: &State, tactics: &crate::v026g::bots::tactics::Tactics, feet: Vec3) -> Option<f32> {
    let (spot, route, next) = match state {
        State::Route { spot, route, next, .. } => (*spot, *route, *next),
        State::Hold { .. } => return Some(0.0),
        _ => return None,
    };
    let r = tactics.camp_spots.get(spot)?.routes.get(route)?;
    if next > r.fire {
        return Some(0.0);
    }
    let grid = &tactics.world.grid;
    let at = |i: usize| grid.position(r.floors[i]);
    let along: f32 = (next..r.fire).map(|i| (at(i + 1) - at(i)).length()).sum();
    Some(at(next).distance_xz(feet) + along)
}

#[cfg(test)]
thread_local! {
    /// [A320 bench] Each route walker's last offer line (the line is printed on a change).
    static OFFER_SEEN: std::cell::RefCell<std::collections::HashMap<Place, String>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// To a way point hidden from him, then to the spot.
    Via {
        via: Vec3,
        spot: Vec3,
    },
    Approach {
        spot: Vec3,
    },
    /// [A320 `threat.camp_flank`] A route made ahead of time round a known camp spot: `spot` and `route` index
    /// `Tactics::camp_spots`; `next` is the first of its floors not yet passed; the play gives up at `give_up_at`;
    /// `camper` is the held spot's shooter, and `orphan` when the job ended (his death) with the flanker on the way
    /// (`threat.camp_flank.hold_ms`: it goes on to hold the firing floor for his return).
    Route {
        spot: usize,
        route: usize,
        next: usize,
        give_up_at: u32,
        camper: Place,
        orphan: Option<u32>,
    },
    /// [A320 `threat.camp_flank.hold_ms`] At the route's firing floor with its sights on the spot: as a peek until
    /// `peek_until` while he lives; after his death (`orphan`) held for his return until the hold's end.
    Hold {
        spot: usize,
        camper: Place,
        peek_until: u32,
        orphan: Option<u32>,
    },
    /// At the spot, watching him.
    Peek {
        until: u32,
    },
    /// No flank spot: out of his line, watching him.
    Wait {
        until: u32,
    },
    /// A job's flank with no flank spot round the held spot: it fails at
    /// once, so the play is set aside and the role goes to another bot
    /// (waiting in place, re-taken at once, was 157 of the job's flank ends
    /// and the bench's idle +41 %, p1-bench 631feece).
    NoSpot,
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

fn wait(cx: &Ctx, rng: &mut Rng) -> State {
    let s = rng.range(
        cx.num(PlayKind::Flank, "wait_min_s"),
        cx.num(PlayKind::Flank, "wait_max_s"),
    );
    State::Wait {
        until: cx.now + (s * 1000.0) as u32,
    }
}

/// The held spot a job's flank is on (K2), by its number.
fn job_line<'a>(cx: &Ctx<'a>, target: TargetRef) -> Option<&'a crate::v026g::bots::threat::HeldLine> {
    let TargetRef::ThreatSpot(id) = target else {
        return None;
    };
    cx.team.threat.line(id)
}

/// [A320] A route floor this near is passed, m.
const ROUTE_PASS_M: f32 = 2.0;
/// [A320] The firing floor is reached this near only, m: it is the route's first floor that sees the spot (the edge of his
/// cover), so a flanker stopped 2 m short is still behind it (B3g4: no line to him at 17 of 21 arrivals). Half a cell's
/// diagonal and a margin.
const ROUTE_FIRE_M: f32 = 0.5;
/// [A320] The way point is the first route floor this far ahead at least, m.
const ROUTE_LEAD_M: f32 = 5.0;
/// [A320] The team's knowledge of an enemy counts toward a route's risk this long after it was sensed or called out, ms,
/// and weighs less as it ages (1 when fresh, 0 at the end).
const ROUTE_KNOWN_MS: u32 = 10_000;
/// [A320] A route metre is at risk within this of a known enemy (and a storey), m...
const ROUTE_ENEMY_M: f32 = 18.0;
/// [A320] ... or in his sight within this, m.
const ROUTE_ENEMY_SIGHT_M: f32 = 40.0;
/// [A320] Routes within this of the least risk are as good: the order and the rotation choose among them, m.
const ROUTE_RISK_TIE_M: f32 = 2.0;
/// [A320] A route whose first metres hold a known enemy (within [`ROUTE_ON_M`] of a floor) is not taken, m.
const ROUTE_NEXT_M: f32 = 20.0;
const ROUTE_ON_M: f32 = 6.0;
/// [A320] Every this many floors of a route are tested against the known enemies (0.25-0.75 m apart).
const ROUTE_ENEMY_STEP: usize = 4;

/// [A320] The route's time: its length at this pace, m/s, and this more, ms (a walk is about 3.5 m/s).
const ROUTE_PACE_MPS: f32 = 2.0;
const ROUTE_SPARE_MS: u32 = 15_000;

/// [A320 `threat.camp_flank`] The known camp spot of the other side the job's held spot is on (within its match radius,
/// a storey at most), with its routes; none with the switch off.
fn camp_spot_of(cx: &Ctx, line: &crate::v026g::bots::threat::HeldLine) -> Option<usize> {
    camp_spot_near(cx, line.at)
}

/// [A320] The known camp spot of the other side at `at` with routes, with the switch on.
fn camp_spot_near(cx: &Ctx, at: Vec3) -> Option<usize> {
    cx.tuning.threat.camp_flank?;
    let spot = crate::v026g::bots::camp_flank::camp_spot_at(&cx.tactics.camp_spots, at, cx.bot.side?)?;
    (!cx.tactics.camp_spots[spot].routes.is_empty()).then_some(spot)
}

/// [A320 `threat.camp_flank.follow_m`] How fresh the team's sense of the camper must be to follow him, ms.
const FOLLOW_FRESH_MS: u32 = 3_000;

/// [A320 `threat.camp_flank.follow_m`] Where the team senses the camper now (`Track::own`: its own senses and callouts,
/// within [`FOLLOW_FRESH_MS`]), if farther than `follow_m` from the walked spot `spot_at`.
fn moved_from(cx: &Ctx, camper: Place, spot_at: Vec3) -> Option<Vec3> {
    let follow = cx.tuning.threat.camp_flank.and_then(|c| c.follow_m)?;
    cx.memory
        .tracks()
        .iter()
        .find(|t| t.who == camper)
        .and_then(|t| t.own)
        .filter(|&(at, time)| cx.now.saturating_sub(time) <= FOLLOW_FRESH_MS && at.distance_xz(spot_at) > follow)
        .map(|(at, _)| at)
}

/// [A320] A route against the other enemies the team knows: none of them (the camper aside) whom the team's own senses or
/// callouts placed (`Track::own`, no cheat place) within [`ROUTE_KNOWN_MS`] is on its first [`ROUTE_NEXT_M`]
/// ([`ROUTE_ON_M`]): `None` (skipped); else its risk, the metres of it within [`ROUTE_ENEMY_M`] of one of them or in his
/// sight within [`ROUTE_ENEMY_SIGHT_M`], each weighed by how fresh the most recent such knowledge is.
fn route_risk(cx: &Ctx, floors: &[mapdata::NodeId], camper: Place) -> Option<f32> {
    use crate::v026g::bots::view::still_eye;
    let grid = &cx.tactics.world.grid;
    let known: Vec<(Vec3, f32)> = cx
        .memory
        .tracks()
        .iter()
        .filter(|t| t.who != camper)
        .filter_map(|t| t.own)
        .filter_map(|(at, time)| {
            let age = cx.now.saturating_sub(time);
            (age <= ROUTE_KNOWN_MS).then(|| (at, 1.0 - age as f32 / ROUTE_KNOWN_MS as f32))
        })
        .collect();
    if known.is_empty() {
        return Some(0.0);
    }
    let (mut walked, mut risk) = (0.0_f32, 0.0_f32);
    let mut prev: Option<Vec3> = None;
    for &n in floors.iter().step_by(ROUTE_ENEMY_STEP) {
        let p = grid.position(n);
        let step = prev.map_or(0.0, |q| q.distance_xz(p));
        walked += step;
        prev = Some(p);
        if walked <= ROUTE_NEXT_M && known.iter().any(|&(e, _)| e.distance_xz(p) <= ROUTE_ON_M && (e.y - p.y).abs() <= 3.0) {
            return None;
        }
        let weight = known
            .iter()
            .filter(|&&(e, _)| {
                let d = e.distance_xz(p);
                (d <= ROUTE_ENEMY_M && (e.y - p.y).abs() <= 4.0)
                    || (d <= ROUTE_ENEMY_SIGHT_M && cx.tactics.sees(still_eye(e, false), still_eye(p, false)))
            })
            .map(|&(_, w)| w)
            .fold(0.0_f32, f32::max);
        risk += step * weight;
    }
    Some(risk)
}

/// [A320] The job flanker's route round the known camp spot `line` is on: the first to arrive first (the bot's walk to its
/// start and its length; B3d: the camper died before 18 of 24 flankers arrived), then the human ones, those near a human
/// win, the rest, of those with the least risk from the other enemies the
/// team knows ([`route_risk`]; every route skipped: none, and the flank as before); the job's n-th flanker
/// (`Job::passed_count`) takes the n-th, so a route a flanker died on is not the next one's.
fn camp_route(cx: &Ctx, line: &crate::v026g::bots::threat::HeldLine) -> Option<State> {
    camp_route_at(cx, camp_spot_of(cx, line)?, line.shooter)
}

/// [`camp_route`] round the known camp spot `spot` for the camper `shooter`.
fn camp_route_at(cx: &Ctx, spot: usize, shooter: Place) -> Option<State> {
    use crate::v026g::bots::camp_flank::Origin;
    let camp = &cx.tactics.camp_spots[spot];
    let grid = &cx.tactics.world.grid;
    let rank = |o: Origin| match o {
        Origin::Human => 0,
        Origin::AstarNearWin => 1,
        Origin::Astar => 2,
    };
    let start_m = |k: usize| grid.position(camp.routes[k].floors[0]).distance_xz(cx.me.feet);
    let mut order: Vec<usize> = (0..camp.routes.len()).collect();
    // [A320 B3d] The first to arrive first (the bot's walk to the start and the route's length), then by what backs it: his
    // lives last ~30 s, and a flank that comes after his death is no flank.
    let length_m = |k: usize| -> f32 {
        camp.routes[k].floors.windows(2).map(|w| (grid.position(w[1]) - grid.position(w[0])).length()).sum()
    };
    let arrival_m: Vec<f32> = (0..camp.routes.len()).map(|k| start_m(k) + length_m(k)).collect();
    order.sort_by(|&a, &b| {
        arrival_m[a].total_cmp(&arrival_m[b]).then(rank(camp.routes[a].origin).cmp(&rank(camp.routes[b].origin)))
    });
    // The other enemies the team knows: a route with one on its next stretch is skipped, the rest ranked by risk; those
    // within the tie band of the least are taken in the order above, the job's n-th flanker the n-th.
    let risks: Vec<Option<f32>> = (0..camp.routes.len()).map(|k| route_risk(cx, &camp.routes[k].floors, shooter)).collect();
    let least = risks.iter().flatten().copied().fold(f32::INFINITY, f32::min);
    let skipped = risks.iter().filter(|r| r.is_none()).count();
    #[cfg(test)]
    let available = camp.routes.len();
    order.retain(|&k| risks[k].is_some_and(|r| r <= least + ROUTE_RISK_TIE_M));
    if order.is_empty() {
        #[cfg(test)]
        eprintln!("A320 camp_flank cue: t {} place {} spot {spot} routes {available} skipped {skipped} fallback", cx.now, cx.me.place);
        return None;
    }
    let passed = cx.team.threat.job_of(cx.me.place).map_or(0, |(job, _, _)| job.passed_count());
    let route = order[passed % order.len()];
    #[cfg(test)]
    eprintln!(
        "A320 camp_flank cue: t {} place {} spot {spot} routes {available} skipped {skipped} chosen risk {:.1}",
        cx.now,
        cx.me.place,
        risks[route].unwrap_or(0.0)
    );
    let _ = skipped;
    let floors = &camp.routes[route].floors;
    let length: f32 = floors.windows(2).map(|w| (grid.position(w[1]) - grid.position(w[0])).length()).sum::<f32>()
        + start_m(route);
    let give_up_at = cx.now + (length / ROUTE_PACE_MPS * 1000.0) as u32 + ROUTE_SPARE_MS;
    // The bench's count of the routes walked (only with the switch on, so an OFF run prints nothing new).
    #[cfg(test)]
    ROUTE_SEEN.with(|m| m.borrow_mut().insert(cx.me.place, (0, 0)));
    #[cfg(test)]
    eprintln!(
        "A320 camp_flank route start: t {} place {} spot {spot} route {route} ({:?}, {:.0} m, passed {passed})",
        cx.now, cx.me.place, camp.routes[route].origin, length
    );
    Some(State::Route { spot, route, next: 0, give_up_at, camper: shooter, orphan: None })
}

/// The old job flank round the held spot `line` (its runtime flank spots), or `NoSpot`.
fn old_job_flank(cx: &Ctx, line: &crate::v026g::bots::threat::HeldLine, rng: &mut Rng) -> State {
    let spots = job_flank_spots(cx, line);
    match round(cx, line.at, line.facing_dir(), &spots, rng) {
        State::Wait { .. } => State::NoSpot,
        state => state,
    }
}

/// [F6] The metres a flank state's walk has to its spot from `feet`: a camp route's start and its length to the firing
/// floor; the old flank's way point and spot, straight; None for the other states.
fn walk_m(state: &State, tactics: &crate::v026g::bots::tactics::Tactics, feet: Vec3) -> Option<f32> {
    match state {
        State::Route { .. } => Some(route_left_m(state, tactics, feet)?),
        State::Via { via, spot } => Some(feet.distance_xz(*via) + via.distance_xz(*spot)),
        State::Approach { spot } => Some(feet.distance_xz(*spot)),
        _ => None,
    }
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    // The team's job (K2): him at the spot, facing down his death lanes.
    if let Some(line) = job_line(cx, target) {
        if let Some(route) = camp_route(cx, line) {
            // [F6] The old flank when it arrives sooner.
            if cx.tuning.threat.camp_flank.is_some_and(|c| c.keep_old_flank) {
                let old = old_job_flank(cx, line, rng);
                if let (Some(old_m), Some(route_m)) = (walk_m(&old, cx.tactics, cx.me.feet), walk_m(&route, cx.tactics, cx.me.feet))
                    && old_m < route_m
                {
                    #[cfg(test)]
                    {
                        ROUTE_SEEN.with(|m| m.borrow_mut().remove(&cx.me.place));
                        eprintln!("A320 camp_flank old flank sooner: t {} place {} {old_m:.0} m vs the route's {route_m:.0} m", cx.now, cx.me.place);
                    }
                    return old;
                }
            }
            return route;
        }
        return old_job_flank(cx, line, rng);
    }
    let TargetRef::Place(who) = target else {
        return wait(cx, rng);
    };
    // His facing, only while he is seen; else wait and watch.
    let (Some(seen), Some(p)) = (
        cx.memory.seen(who, cx.now, cx.view.clock),
        cx.view.place(who),
    ) else {
        return wait(cx, rng);
    };
    let facing = kin::forward(p.yaw);
    let spots = flank_spots(cx, seen.at(), facing, p.yaw, rng);
    round(cx, seen.at(), facing, &spots, rng)
}

/// The flank spots round him at `him`, facing `facing` (yaw `yaw`): in
/// sight of him, `spot_min_m`-`spot_max_m` from him, at least
/// `off_facing_deg` off his facing, fine goals out of the spawn areas.
fn flank_spots(cx: &Ctx, him: Vec3, facing: [f32; 2], yaw: f32, rng: &mut Rng) -> Vec<Vec3> {
    flank_spots_of(cx, him, facing, yaw, None, rng)
}

/// The query round a flank's centre `centre` on him at `him`, for a job's held spot `job` (the storeys under and over
/// with `job_spot_levels`) or a seen holder (none).
fn flank_query(cx: &Ctx, him: Vec3, centre: Vec3, job: Option<&crate::v026g::bots::threat::HeldLine>) -> SpotQuery {
    let num = |name| cx.num(PlayKind::Flank, name);
    let (lo, hi) = (num("spot_min_m"), num("spot_max_m"));
    let threat = &cx.tuning.threat;
    let levels = if job.is_some() && threat.job_spot_levels { crate::v026g::bots::threat::JOB_LEVELS_M } else { 0.0 };
    SpotQuery {
        sight_to: Some(him),
        up_m: levels,
        down_m: levels,
        side: cx.bot.side,
        samples: num("samples") as usize / 2,
        ..SpotQuery::ring(centre, 0.0, (hi - lo) * 0.5 + 2.0)
    }
}

/// [`flank_spots`] for a job's held spot `job` (its query, [`flank_query`]) or a seen holder (none).
fn flank_spots_of(cx: &Ctx, him: Vec3, facing: [f32; 2], yaw: f32, job: Option<&crate::v026g::bots::threat::HeldLine>, rng: &mut Rng) -> Vec<Vec3> {
    let num = |name| cx.num(PlayKind::Flank, name);
    // Looked for on both flanks: round a point off his facing by
    // `off_facing_deg` and a bit more, at the band's middle distance (the
    // spot query samples a full ring; centred on a flank, few samples fall
    // in his face).
    let (lo, hi) = (num("spot_min_m"), num("spot_max_m"));
    let bearing = (num("off_facing_deg") + num("flank_extra_deg")).min(170.0);
    let mut found: Vec<Vec3> = Vec::new();
    for sign in [1.0_f32, -1.0] {
        let yaw = yaw + sign * bearing.to_radians();
        let dir = kin::forward(yaw);
        let mid = (lo + hi) * 0.5;
        let centre = Vec3::new(him.x + dir[0] * mid, him.y, him.z + dir[1] * mid);
        let query = flank_query(cx, him, centre, job);
        found.extend(
            cx.tactics
                .spots(&query, cx.now, 6, rng)
                .into_iter()
                .map(|s| s.at),
        );
    }
    found
        .into_iter()
        .filter(|at| {
            let d = at.distance_xz(him);
            (lo..=hi).contains(&d)
                && degrees_between(facing, [at.x - him.x, at.z - him.z]) >= num("off_facing_deg")
                && cx.tactics.fine_goal(*at, cx.now, cx.bot.side)
                && !cx.tactics.in_spawn_area(*at, None)
        })
        .collect()
}

/// A job's flank spots round its held spot (K2), found once for the side and
/// kept `JOB_SPOTS_KEEP_MS` (Crossfire's `spots_by` pattern): the job's
/// flanker is offered the play only where there is one. Offered everywhere,
/// a held spot with none sent every flanker in turn to fail at once (1,244
/// "no flank spot" ends, quick ends +129 %, p1-bench fd89b84f).
fn job_flank_spots(cx: &Ctx, line: &crate::v026g::bots::threat::HeldLine) -> std::sync::Arc<Vec<Vec3>> {
    use crate::v026g::bots::tactics::Tactics;
    let key = Tactics::spot_key(PlayKind::Flank as u32 + 1_000, cx.bot.side, line.at);
    cx.tactics.spots_kept(key, cx.now, JOB_SPOTS_KEEP_MS, || {
        let facing = line.facing_dir();
        let mut rng = Rng::new(key ^ u64::from(cx.now));
        // Never in a held line, his own lanes too (as the pinner's,
        // `crossfire.rs`): with victims far apart a flank spot can sit in one.
        let mut spots = flank_spots_of(cx, line.at, facing, kin::yaw_of(facing), Some(line), &mut rng);
        #[cfg(test)]
        let found = spots.len();
        spots.retain(|&at| cx.team.threat.line_at(at).is_none());
        #[cfg(test)]
        if spots.is_empty() {
            // The bench's class of a spot with no flank spot: why flank_spots found none (its tests), or all in a
            // held line. A fresh sample of the query with its own rng: the play's spots are not touched.
            let why = if found > 0 { "in_held_line" } else { why_no_flank_spot(cx, line, key) };
            JOB_SPOTS_WHY.with(|m| m.borrow_mut().insert((cx.bot.side, line.id), why));
        }
        spots
    })
}

#[cfg(test)]
thread_local! {
    /// Why a job's held spot had no flank spot at its last look: (side, spot id) -> why.
    static JOB_SPOTS_WHY: std::cell::RefCell<std::collections::HashMap<(Option<i32>, u32), &'static str>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Why [`flank_spots`] found none round `line` (the bench's class): the test that left the most of the query's
/// samples out, in its order, or none sampled. Its own rng (the play's draws are not touched).
#[cfg(test)]
fn why_no_flank_spot(cx: &Ctx, line: &crate::v026g::bots::threat::HeldLine, key: u64) -> &'static str {
    let num = |name| cx.num(PlayKind::Flank, name);
    let (him, facing) = (line.at, line.facing_dir());
    let yaw = kin::yaw_of(facing);
    let (lo, hi) = (num("spot_min_m"), num("spot_max_m"));
    let bearing = (num("off_facing_deg") + num("flank_extra_deg")).min(170.0);
    let mut rng = Rng::new(key ^ 0x5eed);
    let mut fails: Vec<&'static str> = Vec::new();
    let mut queries = Vec::new();
    for sign in [1.0_f32, -1.0] {
        let dir = kin::forward(yaw + sign * bearing.to_radians());
        let mid = (lo + hi) * 0.5;
        let centre = Vec3::new(him.x + dir[0] * mid, him.y, him.z + dir[1] * mid);
        let query = flank_query(cx, him, centre, Some(line));
        queries.push(query);
        for s in cx.tactics.spots(&query, cx.now, 6, &mut rng) {
            let at = s.at;
            let d = at.distance_xz(him);
            fails.push(if !(lo..=hi).contains(&d) {
                "out_of_band"
            } else if degrees_between(facing, [at.x - him.x, at.z - him.z]) < num("off_facing_deg") {
                "in_his_facing"
            } else if !cx.tactics.fine_goal(at, cx.now, cx.bot.side) {
                "not_fine_goal"
            } else {
                "in_spawn_area"
            });
        }
    }
    if fails.is_empty() {
        // None sampled: which stage of the spot query lost them.
        let whys: Vec<&'static str> = queries.iter().map(|q| cx.tactics.spots_why(q, cx.now, &mut rng)).collect();
        return match crate::v026g::bots::threat::most_failed(&whys) {
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

/// How long a job's flank spots are kept, ms (as Crossfire's `spots_keep_s`).
const JOB_SPOTS_KEEP_MS: u32 = 15_000;

/// The way round him at `him` from the flank spots `spots`: the nearest,
/// reached by a way point hidden from him; none, a wait.
fn round(cx: &Ctx, him: Vec3, facing: [f32; 2], spots: &[Vec3], rng: &mut Rng) -> State {
    let num = |name| cx.num(PlayKind::Flank, name);
    // The nearest of the good ones: a flank is quick.
    let spot = spots
        .iter()
        .copied()
        .min_by(|a, b| a.distance_xz(cx.me.feet).total_cmp(&b.distance_xz(cx.me.feet)));
    let Some(spot) = spot else {
        return wait(cx, rng);
    };
    // A way point near the spot, out of his sight, so the way there is
    // not across his line.
    let via_query = SpotQuery {
        unseen_by: Some(him),
        side: cx.bot.side,
        samples: num("samples") as usize,
        ..SpotQuery::ring(spot, num("via_min_m"), num("via_max_m"))
    };
    let via = cx
        .tactics
        .spots(&via_query, cx.now, 6, rng)
        .into_iter()
        .map(|s| s.at)
        .filter(|at| {
            degrees_between(facing, [at.x - him.x, at.z - him.z]) >= num("off_facing_deg")
                && cx.tactics.fine_goal(*at, cx.now, cx.bot.side)
        })
        .min_by(|a, b| {
            a.distance_xz(cx.me.feet)
                .total_cmp(&b.distance_xz(cx.me.feet))
        });
    match via {
        Some(via) if via.distance_xz(cx.me.feet) < spot.distance_xz(cx.me.feet) => {
            State::Via { via, spot }
        }
        _ => State::Approach { spot },
    }
}

#[cfg(test)]
thread_local! {
    /// [A320 bench] The job flankers' last step, by place (`SRP_BENCH_CAMPER_FLANK_ONLY`: only they hurt the camper).
    static FLANKING: std::cell::RefCell<std::collections::HashMap<Place, u32>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// [A320 bench] Per route walker: the steps of its walk and those the camper had a line to it (bench truth).
    static ROUTE_SEEN: std::cell::RefCell<std::collections::HashMap<Place, (u32, u32)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// [A320 bench] Whether `place` stepped a job's flank within the last 1.5 s of `now` (the bench's isolated flank test).
#[cfg(test)]
pub fn flanking(place: Place, now: u32) -> bool {
    FLANKING.with(|m| m.borrow().get(&place).is_some_and(|&t| now.saturating_sub(t) <= 1_500))
}

/// [A320 `threat.camp_flank.follow_m`] The camper moved from the walked spot `from` to `his` (the team's own senses): the
/// routes of the known camp spot there, if one (the walk starts again from where the bot is), else the walk ends.
#[allow(clippy::too_many_arguments)]
fn follow(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng, from: usize, shooter: Place, orphan: Option<u32>, his: Vec3) -> Step {
    match camp_spot_near(cx, his).filter(|&s| s != from).and_then(|s| camp_route_at(cx, s, shooter)) {
        Some(mut next) => {
            if let State::Route { orphan: o, .. } = &mut next {
                *o = orphan;
            }
            #[cfg(test)]
            eprintln!("A320 camp_flank follow: t {} place {} from spot {from} to {next:?}", cx.now, cx.me.place);
            *state = next;
            step(state, target, cx, rng)
        }
        None => {
            // [F6] The old flank round where the team senses him now (the held spot is where he was: B3f14, sensed and true
            // within 3-5 m, the held spot 40-50 m off), facing as the held spot, instead of no flank.
            if cx.tuning.threat.camp_flank.is_some_and(|c| c.keep_old_flank)
                && let Some(line) = job_line(cx, target)
            {
                let mut moved = line.clone();
                moved.at = his;
                let old = old_job_flank(cx, &moved, rng);
                if !matches!(old, State::NoSpot) {
                    #[cfg(test)]
                    {
                        ROUTE_SEEN.with(|m| m.borrow_mut().remove(&cx.me.place));
                        eprintln!("A320 camp_flank follow: t {} place {} from spot {from}: he moved, the old flank goes on", cx.now, cx.me.place);
                    }
                    *state = old;
                    return step(state, target, cx, rng);
                }
            }
            #[cfg(test)]
            {
                // Bench truth beside the team's sense: where he is, where the walked spot and the held spot are.
                let truth = cx.view.place(shooter).filter(|p| p.alive).map(|p| p.feet);
                let spot_at = cx.tactics.camp_spots.get(from).map(|c| c.at);
                let held = match target {
                    TargetRef::ThreatSpot(id) => cx.team.threat.line(id).map(|l| l.at),
                    _ => None,
                };
                let xz = |p: Option<Vec3>| p.map_or("-".to_string(), |p| format!("({:.0},{:.0})", p.x, p.z));
                eprintln!(
                    "A320 camp_flank follow: t {} place {} from spot {from}: he moved {:.0} m off, no known spot there; sensed {} true {} spot {} held {}",
                    cx.now,
                    cx.me.place,
                    spot_at.map_or(0.0, |a| a.distance_xz(his)),
                    xz(Some(his)),
                    xz(truth),
                    xz(spot_at),
                    xz(held)
                );
            }
            Step::Done("he moved from the spot")
        }
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Flank, name);
    #[cfg(test)]
    if matches!(target, TargetRef::ThreatSpot(_)) {
        FLANKING.with(|m| m.borrow_mut().insert(cx.me.place, cx.now));
    }
    let route_until = match state {
        State::Route { give_up_at, .. } => Some(*give_up_at),
        _ => None,
    };
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > route_until.unwrap_or(started + (num("give_up_s") * 1000.0) as u32)
    {
        #[cfg(test)]
        if route_until.is_some() {
            eprintln!("A320 camp_flank route gave up: t {} place {}", cx.now, cx.me.place);
        }
        return Step::Failed("flanked too long");
    }
    // [A320] A route's or a hold's point is its known camp spot when the held spot is gone (his death, or the team lost him).
    let camp_flank_on = cx.tuning.threat.camp_flank.is_some();
    let hold_ms = cx.tuning.threat.camp_flank.and_then(|c| c.hold_ms);
    let camp_at = match state {
        State::Route { spot, .. } | State::Hold { spot, .. } if camp_flank_on => cx.tactics.camp_spots.get(*spot).map(|c| c.at),
        _ => None,
    };
    let Some(him) = cx.point_of(target).or(camp_at) else {
        return Step::Done("he is gone from the memory");
    };
    // The team's job ended: the spot is no longer held.
    if matches!(target, TargetRef::ThreatSpot(_)) && job_line(cx, target).is_none() {
        match state {
            State::Route { orphan, .. } | State::Hold { orphan, .. } if camp_flank_on => {
                if orphan.is_none() {
                    *orphan = Some(cx.now);
                    #[cfg(test)]
                    eprintln!("A320 camp_flank hold: t {} place {} his death, holding on", cx.now, cx.me.place);
                }
            }
            _ => {
                #[cfg(test)]
                if let State::Route { next, .. } = state {
                    eprintln!("A320 camp_flank route end: t {} place {} job over at floor {next}", cx.now, cx.me.place);
                }
                return Step::Done("the spot is no longer held");
            }
        }
    }
    // [A320] With camp_flank on the flank stays offered after its job role went (see `candidates`): only a route or a hold
    // goes on; a flank of the old kind ends here, as it ended before when the role's offer went.
    if camp_flank_on
        && let TargetRef::ThreatSpot(id) = target
        && !matches!(state, State::Route { .. } | State::Hold { .. })
        && !matches!(cx.team.threat.job_of(cx.me.place), Some((_, Role::Flanker, l)) if l.id == id)
    {
        return Step::Done("the job role is gone");
    }
    // Seen up and moving: he no longer holds the angle.
    if let TargetRef::Place(who) = target
        && cx.memory.seen(who, cx.now, cx.view.clock).is_some()
        && let Some(p) = cx.view.place(who)
        && (p.speed() >= num("still_speed") * 2.0 || p.keys & key::CROUCH == 0)
    {
        return Step::Done("he left his angle");
    }
    let reach = num("reach_m");
    match state {
        State::NoSpot => Step::Failed("no flank spot round the held spot"),
        State::Route { spot, route, next, camper, orphan, .. } => {
            let Some(r) = cx.tactics.camp_spots.get(*spot).and_then(|c| c.routes.get(*route)) else {
                return Step::Failed("the camp spot's route is gone");
            };
            if let (Some(o), Some(hold)) = (*orphan, hold_ms)
                && cx.now >= o + hold
            {
                return Step::Done("held on for his return, not there yet");
            }
            if let Some(his) = moved_from(cx, *camper, cx.tactics.camp_spots[*spot].at) {
                let (from, shooter, was_orphan) = (*spot, *camper, *orphan);
                return follow(state, target, cx, rng, from, shooter, was_orphan, his);
            }
            let grid = &cx.tactics.world.grid;
            let at = |i: usize| grid.position(r.floors[i]);
            let fire = r.fire;
            // Passed: every floor up to the last one within reach (the route is walked in order).
            let pass = |i: usize| if i == fire { ROUTE_FIRE_M } else { ROUTE_PASS_M };
            if let Some(k) = (*next..=fire).rev().find(|&i| at(i).distance_xz(cx.me.feet) <= pass(i)) {
                *next = k + 1;
            }
            // [A320 bench] Whether the camper (bench truth: the world's place of the held spot's shooter) has a line to it.
            #[cfg(test)]
            {
                let seen = cx.view.place(*camper).filter(|p| p.alive).is_some_and(|p| {
                    cx.tactics.sees(crate::v026g::bots::view::still_eye(p.feet, p.keys & key::CROUCH != 0), crate::v026g::bots::view::still_eye(cx.me.feet, false))
                });
                ROUTE_SEEN.with(|m| {
                    let mut m = m.borrow_mut();
                    let e = m.entry(cx.me.place).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += u32::from(seen);
                });
            }
            if *next > fire {
                #[cfg(test)]
                {
                    let (steps, seen) = ROUTE_SEEN.with(|m| m.borrow_mut().remove(&cx.me.place)).unwrap_or((0, 0));
                    // At the firing floor (bench truth): does it see him, and how far.
                    let sees = cx.view.place(*camper).filter(|p| p.alive).map(|p| {
                        let eye = crate::v026g::bots::view::still_eye(cx.me.feet, true);
                        let his = crate::v026g::bots::view::still_eye(p.feet, p.keys & key::CROUCH != 0);
                        (cx.tactics.sees(eye, his), p.feet.distance_xz(cx.me.feet))
                    });
                    let at = match sees {
                        Some((true, d)) => format!("sees him at {d:.0} m"),
                        Some((false, d)) => format!("no line to him at {d:.0} m"),
                        None => "he is dead".to_string(),
                    };
                    eprintln!("A320 camp_flank route fire: t {} place {} spot {spot} route {route} seen {seen} of {steps} steps, {at}", cx.now, cx.me.place);
                }
                let s = rng.range(num("peek_min_s"), num("peek_max_s"));
                let until = cx.now + (s * 1000.0) as u32;
                *state = State::Hold { spot: *spot, camper: *camper, peek_until: until, orphan: *orphan };
                return Step::Continue(Intent::stay().crouched().looking(Look::At(him)));
            }
            let way = (*next..=fire).find(|&i| at(i).distance_xz(cx.me.feet) >= ROUTE_LEAD_M).unwrap_or(fire);
            let to_fire = at(fire).distance_xz(cx.me.feet);
            let pace = if to_fire < num("sneak_m") { Pace::Sneak } else { Pace::Walk };
            let mut intent = Intent::to(at(way), pace).within(pass(way) * 0.7);
            if to_fire < num("sneak_m") {
                intent = intent.looking(Look::At(him));
            }
            Step::Continue(intent)
        }
        State::Hold { spot, camper, peek_until, orphan } => {
            if let Some(his) = moved_from(cx, *camper, cx.tactics.camp_spots.get(*spot).map_or(him, |c| c.at)) {
                let (from, shooter, was_orphan) = (*spot, *camper, *orphan);
                return follow(state, target, cx, rng, from, shooter, was_orphan, his);
            }
            if cx.me.carrying.is_some() {
                return Step::Done("carries a battery");
            }
            match (*orphan, hold_ms) {
                (None, _) | (Some(_), None) if cx.now >= *peek_until => return Step::Done("flanked and watched him"),
                (Some(o), Some(hold)) => {
                    if cx.now >= o + hold {
                        #[cfg(test)]
                        eprintln!("A320 camp_flank hold: t {} place {} ended, the hold's time", cx.now, cx.me.place);
                        return Step::Done("held the spot for his return");
                    }
                    // His next life sensed away from the spot (the team's own senses): he is not coming back here.
                    let elsewhere = cx.memory.tracks().iter().any(|t| {
                        t.who == *camper
                            && t.own.is_some_and(|(at, time)| time > o && cx.now.saturating_sub(time) <= 2_000 && at.distance_xz(him) > 30.0)
                    });
                    if elsewhere {
                        #[cfg(test)]
                        eprintln!("A320 camp_flank hold: t {} place {} ended, he is elsewhere", cx.now, cx.me.place);
                        return Step::Done("his next life is elsewhere");
                    }
                }
                _ => {}
            }
            // [B3g4] The fallback peek: no line from the crouched eye to his spot's standing eye (the table's spot, not his
            // place): it stands to peek over the cover.
            let crouched_sees = cx.tactics.sees(
                crate::v026g::bots::view::still_eye(cx.me.feet, true),
                crate::v026g::bots::view::still_eye(him, false),
            );
            let intent = Intent::stay().looking(Look::At(him));
            Step::Continue(if crouched_sees { intent.crouched() } else { intent })
        }
        State::Via { via, spot } => {
            let (via, spot) = (*via, *spot);
            if cx.me.feet.distance_xz(via) < reach * 1.5 {
                *state = State::Approach { spot };
            }
            Step::Continue(Intent::to(via, Pace::Walk).within(reach))
        }
        State::Approach { spot } => {
            let spot = *spot;
            let d = cx.me.feet.distance_xz(spot);
            if d <= reach {
                let s = rng.range(num("peek_min_s"), num("peek_max_s"));
                *state = State::Peek {
                    until: cx.now + (s * 1000.0) as u32,
                };
                return Step::Continue(Intent::stay().crouched().looking(Look::At(him)));
            }
            let pace = if d < num("sneak_m") {
                Pace::Sneak
            } else {
                Pace::Walk
            };
            let mut intent = Intent::to(spot, pace).within(reach * 0.7);
            if d < num("sneak_m") {
                intent = intent.looking(Look::At(him));
            }
            Step::Continue(intent)
        }
        State::Peek { until } => {
            if cx.now >= *until {
                return Step::Done("flanked and watched him");
            }
            Step::Continue(Intent::stay().crouched().looking(Look::At(him)))
        }
        State::Wait { until } => {
            if cx.now >= *until {
                return Step::Done("waited him out");
            }
            Step::Continue(Intent::stay().crouched().looking(Look::At(him)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::{Percept, Source};
    use crate::v026g::bots::view::Mode;

    /// The enemy at `who` seen without a break for the last `ms`.
    fn watched(s: &mut Scenario, who: Place, ms: u32) {
        let now = s.view.clock;
        let at = s.view.places[who].feet;
        let mut t = now - ms;
        while t <= now {
            s.memory.update(
                &[Percept {
                    who,
                    at,
                    error_m: 0.0,
                    time: t,
                    source: Source::Seen,
                    busy: false,
                }],
                t,
                |_| 200,
                3_000,
                20_000,
            );
            t += 200;
        }
    }

    /// Bot 0 at (25, 0, 32); enemy 1 at (55, 0, 30) facing -x (at the bot),
    /// crouched, aiming, still, seen 2 s.
    fn holder() -> Scenario {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 32.0));
        s.enemy(1, 1, Vec3::new(55.0, 0.0, 30.0), None);
        let p = &mut s.view.places[1];
        p.keys = key::CROUCH | key::AIM;
        p.yaw = kin::yaw_of([-1.0, 0.0]);
        watched(&mut s, 1, 2_000);
        s
    }

    fn flank_of(s: &Scenario) -> Option<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out.into_iter().find(|c| c.kind == PlayKind::Flank)
    }

    #[test]
    fn a_held_angle_is_a_candidate_and_one_flanker_per_holder() {
        let s = holder();
        let c = flank_of(&s).expect("a held angle is flanked");
        assert_eq!(c.target, TargetRef::Place(1));
        assert_eq!(c.rank, Rank::Fight);
        assert_eq!(
            c.slot,
            Some(SlotKey {
                kind: PlayKind::Flank,
                key: 1
            })
        );
    }

    #[test]
    fn standing_moving_not_aiming_or_only_heard_is_no_held_angle() {
        let mut s = holder();
        s.view.places[1].keys = key::AIM;
        assert!(flank_of(&s).is_none(), "standing");
        let mut s = holder();
        s.view.places[1].keys = key::CROUCH;
        assert!(flank_of(&s).is_none(), "not aiming");
        let mut s = holder();
        s.view.places[1].velocity = [2.0, 0.0];
        assert!(flank_of(&s).is_none(), "moving");
        // Seen only a moment: not yet "a while".
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 32.0));
        s.enemy(1, 1, Vec3::new(55.0, 0.0, 30.0), Some(Source::Seen));
        s.view.places[1].keys = key::CROUCH | key::AIM;
        assert!(flank_of(&s).is_none(), "seen 0.4 s");
        // Heard: his keys are not his to read.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 32.0));
        s.enemy(1, 1, Vec3::new(55.0, 0.0, 30.0), Some(Source::Heard));
        s.view.places[1].keys = key::CROUCH | key::AIM;
        assert!(flank_of(&s).is_none(), "heard");
    }

    #[test]
    fn team_deathmatch_favours_a_flank() {
        let tdm = flank_of(&holder()).unwrap().score.get();
        let mut s = holder();
        s.view.mode = Mode::Batteries;
        let batteries = flank_of(&s).unwrap().score.get();
        assert!(batteries < tdm * 0.9, "{tdm} {batteries}");
    }

    #[test]
    fn with_his_facing_unknown_it_waits_watching_him() {
        // Not seen at the start (heard only): his facing is not known.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 32.0));
        s.enemy(1, 1, Vec3::new(55.0, 0.0, 30.0), Some(Source::Heard));
        let mut rng = Rng::new(4);
        s.current = Some((PlayKind::Flank, TargetRef::Place(1), s.view.clock));
        let mut state = start(TargetRef::Place(1), &s.cx(0), &mut rng);
        assert!(matches!(state, State::Wait { .. }), "{state:?}");
        let st = step(&mut state, TargetRef::Place(1), &s.cx(0), &mut rng);
        assert!(
            matches!(st, Step::Continue(i) if i.go == Go::Stay && i.look == Look::At(Vec3::new(55.0, 0.0, 30.0))),
            "{st:?}"
        );
        s.view.clock += 9_000;
        assert_eq!(
            step(&mut state, TargetRef::Place(1), &s.cx(0), &mut rng),
            Step::Done("waited him out")
        );
    }

    /// [owner-5] A job role's offer writes its outcome to the log (`nav_log::Kind::Role`), once per role, spot, holding
    /// and outcome: asked again, the same outcome writes nothing new to the bot's log.
    #[test]
    fn a_job_offer_writes_its_outcome_to_the_log_once() {
        use crate::v026g::bots::nav_log::{Kind, Lines, NavLog};
        use crate::v026g::bots::threat::{HeldLine, Holder, Job, Picture};
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let me = Vec3::new(20.0, 0.0, 35.0);
        let him = Vec3::new(60.0, 0.0, 12.0);
        s.bot(0, 0, me);
        let now = s.view.clock;
        let line = HeldLine::for_test(7, 15, him, &[Vec3::new(45.0, 0.0, 12.0)], Some(now));
        let job = Job::of(7, 15, Some(Holder { place: 0, since: now }), None);
        s.team.threat = std::sync::Arc::new(Picture { held: vec![line], jobs: vec![job], ..Picture::default() });
        s.tuning.threat.job = true;
        let mut log = NavLog::default();
        for _ in 0..3 {
            let cx = s.cx(0);
            let mut out = Vec::new();
            candidates(&cx, &mut out);
            let lines = cx.lines.take();
            let role: Vec<_> = lines.iter().filter(|(kind, _, _)| *kind == Kind::Role).collect();
            assert_eq!(role.len(), 1, "{lines:?}");
            let text = &role[0].2;
            assert!(text.starts_with("nav: job role flanker on ThreatSpot(7) (Flank): "), "{text}");
            let outcome = text.trim_start_matches("nav: job role flanker on ThreatSpot(7) (Flank): ");
            assert!(["offered", "no_spot", "set_aside", "dry", "low_health"].iter().any(|o| outcome.starts_with(o)), "{text}");
            Lines::write(lines, &mut log, now);
        }
        assert_eq!(log.take().len(), 1, "three asks, one line");
    }

    /// [A320 `threat.camp_flank`] With the switch on, the job's flanker on a held spot at a known camp spot of the other
    /// side walks the table's route to its firing floor and watches him there; off, the job's flank is as before.
    #[test]
    fn a_job_flanker_at_a_known_camp_spot_walks_its_route_with_camp_flank() {
        use crate::v026g::bots::camp_flank::{CampSpot, FlankRoute, Origin};
        use crate::v026g::bots::threat::{HeldLine, Holder, Job, Picture};
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let me = Vec3::new(20.0, 0.0, 35.0);
        let him = Vec3::new(60.0, 0.0, 12.0);
        s.bot(0, 0, me);
        let now = s.view.clock;
        let line = HeldLine::for_test(7, 15, him, &[Vec3::new(45.0, 0.0, 12.0)], Some(now));
        let job = Job::of(7, 15, Some(Holder { place: 0, since: now }), None);
        s.team.threat = std::sync::Arc::new(Picture { held: vec![line], jobs: vec![job], ..Picture::default() });
        let grid = s.tactics.world.grid.clone();
        let fire_at = Vec3::new(62.0, 0.0, 30.0);
        let (a, b) = (grid.locate(me, 1.0).unwrap(), grid.locate(fire_at, 1.0).unwrap());
        let floors = grid.path(a, b).unwrap().nodes;
        let route_floors = floors.clone();
        let fire = floors.len() - 1;
        let route = FlankRoute { origin: Origin::Astar, floors, fire, exposed_m: 0.0 };
        let camp = CampSpot { camper_side: 1, at: him, match_radius_m: 5.0, follow_radius_m: 10.0, routes: vec![route] };
        s.tactics.camp_spots = std::sync::Arc::new(vec![camp]);
        let target = TargetRef::ThreatSpot(7);
        let mut rng = Rng::new(3);
        let off = start(target, &s.cx(0), &mut rng);
        assert!(!matches!(off, State::Route { .. }), "off: as before, {off:?}");
        s.tuning.threat.camp_flank = Some(crate::v026g::bots::tuning::CampFlank::default());
        let on = start(target, &s.cx(0), &mut rng);
        assert!(matches!(on, State::Route { spot: 0, route: 0, next: 0, .. }), "{on:?}");
        // On its route it is a walk to its spot (`commit.hold_to_arrival`: a play of its band does not take it); the old
        // flank's walk is not.
        assert!(super::super::Running::Flank(on.clone()).travelling());
        assert!(!super::super::Running::Flank(off.clone()).travelling(), "{off:?}");
        // The job's time ran out with the spot still held (no role): the route goes on, a flank of the old kind ends.
        {
            let saved = s.team.threat.clone();
            s.team.threat = std::sync::Arc::new(Picture { held: saved.held.clone(), jobs: vec![], ..Picture::default() });
            let mut r = Rng::new(5);
            let mut walk = on.clone();
            assert!(matches!(step(&mut walk, target, &s.cx(0), &mut r), Step::Continue(_)), "{walk:?}");
            let mut old = off.clone();
            assert_eq!(step(&mut old, target, &s.cx(0), &mut r), Step::Done("the job role is gone"));
            s.team.threat = saved;
        }
        // The route's metres left: from the bot to its next floor, then along it to the firing floor.
        let left = route_left_m(&on, &s.tactics, me).unwrap();
        let along: f32 = (0..fire).map(|i| (grid.position(route_floors[i + 1]) - grid.position(route_floors[i])).length()).sum();
        assert!((left - along).abs() <= 1.5, "{left} vs {along}");
        // `alarm_near_fire_m`: near its firing floor the walker's flank is in the alarm band; farther, its own.
        {
            s.current = Some((PlayKind::Flank, target, 0));
            s.tuning.threat.camp_flank = Some(crate::v026g::bots::tuning::CampFlank { alarm_near_fire_m: Some(25.0), ..Default::default() });
            let rank_of = |s: &Scenario| {
                let mut found = [
                    Candidate { kind: PlayKind::Flank, target, rank: Rank::Objective, score: Score::new(0.5), why: Default::default(), slot: None },
                    Candidate { kind: PlayKind::Roam, target: TargetRef::None, rank: Rank::Roam, score: Score::new(0.5), why: Default::default(), slot: None },
                ];
                near_fire_alarm(&s.cx(0), &mut found);
                (found[0].rank, found[1].rank)
            };
            s.route_left_m = Some(10.0);
            assert_eq!(rank_of(&s), (Rank::Alarm, Rank::Roam), "near: its flank only in the alarm band");
            s.route_left_m = Some(40.0);
            assert_eq!(rank_of(&s), (Rank::Objective, Rank::Roam), "far: its own band");
            // [F1] Inertia: halfway (40 m of 80 left) the whole walk is in the alarm band with half the bonus.
            s.tuning.threat.camp_flank = Some(crate::v026g::bots::tuning::CampFlank { inertia: Some(0.3), ..Default::default() });
            s.route_total_m = Some(80.0);
            let mut found = [Candidate { kind: PlayKind::Flank, target, rank: Rank::Objective, score: Score::new(0.5), why: Default::default(), slot: None }];
            near_fire_alarm(&s.cx(0), &mut found);
            assert_eq!(found[0].rank, Rank::Alarm);
            assert!((found[0].score.get() - 0.65).abs() < 1e-4, "{}", found[0].score.get());
            s.route_total_m = None;
            // [F6] keep_old_flank: the shorter walk of the old flank and the camp route.
            s.current = None;
            s.tuning.threat.camp_flank = Some(crate::v026g::bots::tuning::CampFlank { keep_old_flank: true, ..Default::default() });
            let mut r = Rng::new(3);
            let chosen = start(target, &s.cx(0), &mut r);
            let (old_m, route_m) = (walk_m(&off, &s.tactics, me), walk_m(&on, &s.tactics, me));
            assert!(old_m.is_some() && route_m.is_some(), "{off:?} {on:?}");
            assert_eq!(matches!(chosen, State::Route { .. }), route_m <= old_m, "{chosen:?}: old {old_m:?} route {route_m:?}");
            s.current = Some((PlayKind::Flank, target, 0));
            s.route_left_m = None;
            s.current = None;
            s.tuning.threat.camp_flank = Some(crate::v026g::bots::tuning::CampFlank::default());
        }
        let steps = s.run(0, PlayKind::Flank, target, 200, 300, 3.5);
        assert!(
            steps.iter().any(|st| matches!(st, Step::Continue(i) if i.go == Go::Stay && i.crouch && i.look == Look::At(him))),
            "it peeks at him: {steps:?}"
        );
        let at = s.view.places[0].feet;
        assert!(at.distance_xz(fire_at) <= 3.0, "at the route's firing floor: {at:?}");
        // Another enemy the team sees by a later stretch of the only route (not the camper): a risk, still the least, so taken.
        s.view.places[0].feet = me;
        s.enemy(12, 1, Vec3::new(55.0, 0.0, 38.0), Some(crate::v026g::bots::know::Source::Seen));
        let risky = start(target, &s.cx(0), &mut rng);
        assert!(matches!(risky, State::Route { .. }), "a risk is ranked, not refused: {risky:?}");
        // One on its first 20 m: skipped, and with no other route the flank as before.
        s.enemy(13, 1, Vec3::new(30.0, 0.0, 36.0), Some(crate::v026g::bots::know::Source::Seen));
        let blocked = start(target, &s.cx(0), &mut rng);
        assert!(!matches!(blocked, State::Route { .. }), "an enemy on the next stretch: {blocked:?}");
        // `follow_m`: the team senses the camper 40 m off his spot, where no known camp spot is: the walk ends; without the
        // option it goes on to the spot of its cue.
        let mut walk = risky.clone();
        s.enemy(15, 1, him + Vec3::new(0.0, 0.0, 40.0), Some(crate::v026g::bots::know::Source::Seen));
        let mut r = Rng::new(7);
        assert!(matches!(step(&mut walk, target, &s.cx(0), &mut r), Step::Continue(_)), "no follow_m: it goes on, {walk:?}");
        s.tuning.threat.camp_flank = Some(crate::v026g::bots::tuning::CampFlank { follow_m: Some(10.0), ..Default::default() });
        let mut walk = risky;
        assert_eq!(step(&mut walk, target, &s.cx(0), &mut r), Step::Done("he moved from the spot"));
    }

    /// [A342a `threat.lane_goal_weight`, A342b `threat.no_lone_m`] A cued spot (2 kills, lanes along +x): a Cut whose goal is
    /// in his lanes is weighed down, one out of them is not; a PairUp on him alone is not offered, with a teammate as near
    /// it is; off, neither changes anything.
    #[test]
    fn a_cued_spots_lanes_weigh_goals_in_them_and_hold_a_lone_counter() {
        use crate::v026g::bots::threat::{HeldLine, Picture};
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let him = Vec3::new(60.0, 0.0, 20.0);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 20.0));
        let line = HeldLine::for_test(7, 15, him, &[Vec3::new(30.0, 0.0, 20.0), Vec3::new(30.0, 0.0, 21.0)], Some(s.view.clock));
        s.team.threat = std::sync::Arc::new(Picture { held: vec![line], ..Picture::default() });
        s.enemy(15, 1, him, Some(crate::v026g::bots::know::Source::Seen));
        let (in_lane, out) = (TargetRef::Point(Vec3::new(40.0, 0.0, 20.0)), TargetRef::Point(Vec3::new(60.0, 0.0, 60.0)));
        // Off: as before.
        assert_eq!(s.cx(0).lane_goal_weight(PlayKind::Cut, in_lane), 1.0);
        assert!(!s.cx(0).lone_into_cued(PlayKind::PairUp, TargetRef::Place(15)));
        s.tuning.threat.lane_goal_weight = Some(0.5);
        s.tuning.threat.no_lone_m = Some(10.0);
        assert_eq!(s.cx(0).lane_goal_weight(PlayKind::Cut, in_lane), 0.5);
        assert_eq!(s.cx(0).lane_goal_weight(PlayKind::Cut, out), 1.0);
        assert_eq!(s.cx(0).lane_goal_weight(PlayKind::Roam, in_lane), 1.0, "not one of its plays");
        assert!(s.cx(0).lone_into_cued(PlayKind::PairUp, TargetRef::Place(15)), "alone: held");
        // A teammate as near him: the pair goes.
        s.bot(1, 0, Vec3::new(22.0, 0.0, 24.0));
        assert!(!s.cx(0).lone_into_cued(PlayKind::PairUp, TargetRef::Place(15)));
    }

    /// [A320 `threat.camp_flank.hold_ms`] The camper dies (the job and its held spot go) with the flanker on its route: with
    /// the hold it goes on to the firing floor and holds it, sights on the spot, until the hold's end; without it the flank
    /// ends with the job, as before.
    #[test]
    fn with_the_hold_a_flanker_whose_camper_died_holds_his_spot_for_his_return() {
        use crate::v026g::bots::camp_flank::{CampSpot, FlankRoute, Origin};
        use crate::v026g::bots::threat::{HeldLine, Holder, Job, Picture};
        use crate::v026g::bots::tuning::CampFlank;
        let run = |hold_ms: Option<u32>| {
            let mut s = Scenario::field(Mode::TeamDeathmatch);
            let me = Vec3::new(20.0, 0.0, 35.0);
            let him = Vec3::new(60.0, 0.0, 12.0);
            s.bot(0, 0, me);
            let now = s.view.clock;
            let line = HeldLine::for_test(7, 15, him, &[Vec3::new(45.0, 0.0, 12.0)], Some(now));
            let job = Job::of(7, 15, Some(Holder { place: 0, since: now }), None);
            s.team.threat = std::sync::Arc::new(Picture { held: vec![line], jobs: vec![job], ..Picture::default() });
            let grid = s.tactics.world.grid.clone();
            let fire_at = Vec3::new(62.0, 0.0, 30.0);
            let (a, b) = (grid.locate(me, 1.0).unwrap(), grid.locate(fire_at, 1.0).unwrap());
            let floors = grid.path(a, b).unwrap().nodes;
            let fire = floors.len() - 1;
            let route = FlankRoute { origin: Origin::Astar, floors, fire, exposed_m: 0.0 };
            let camp = CampSpot { camper_side: 1, at: him, match_radius_m: 5.0, follow_radius_m: 10.0, routes: vec![route] };
            s.tactics.camp_spots = std::sync::Arc::new(vec![camp]);
            s.tuning.threat.camp_flank = Some(CampFlank { hold_ms, ..CampFlank::default() });
            let target = TargetRef::ThreatSpot(7);
            let mut rng = Rng::new(3);
            s.current = Some((PlayKind::Flank, target, now));
            let mut state = start(target, &s.cx(0), &mut rng);
            assert!(matches!(state, State::Route { .. }), "{state:?}");
            // His death: the job and its held spot are gone.
            s.team.threat = std::sync::Arc::new(Picture::default());
            let first = step(&mut state, target, &s.cx(0), &mut rng);
            (first, state, s)
        };
        let (no_hold, no_hold_state, mut s0) = run(None);
        assert!(matches!(no_hold, Step::Continue(_)), "camp_flank on, no hold: the route goes on to the known spot: {no_hold:?}");
        assert!(matches!(no_hold_state, State::Route { orphan: Some(_), .. }), "{no_hold_state:?}");
        // The switch off at the step (the flank as before): the held spot gone ends it.
        s0.tuning.threat.camp_flank = None;
        let mut st = no_hold_state;
        let mut rng0 = Rng::new(9);
        assert_eq!(step(&mut st, TargetRef::ThreatSpot(7), &s0.cx(0), &mut rng0), Step::Done("he is gone from the memory"));
        let (on, state, mut s) = run(Some(20_000));
        assert!(matches!(on, Step::Continue(_)), "the hold goes on: {on:?}");
        assert!(matches!(state, State::Route { orphan: Some(_), .. }), "{state:?}");
        // The flank stays offered to it while it holds.
        let offered = {
            let mut out = Vec::new();
            candidates(&s.cx(0), &mut out);
            out.iter().any(|c| c.kind == PlayKind::Flank && c.target == TargetRef::ThreatSpot(7))
        };
        assert!(offered, "the hold keeps its flank offered");
        // At the firing floor it holds, sights on the spot, until the hold's end.
        let fire_at = Vec3::new(62.0, 0.0, 30.0);
        s.view.places[0].feet = fire_at;
        let mut state = state;
        let mut rng = Rng::new(5);
        let at_fire = step(&mut state, TargetRef::ThreatSpot(7), &s.cx(0), &mut rng);
        assert!(matches!(state, State::Hold { orphan: Some(_), .. }), "{state:?}");
        assert!(matches!(at_fire, Step::Continue(i) if i.go == Go::Stay && i.crouch), "{at_fire:?}");
        s.view.clock += 21_000;
        assert_eq!(step(&mut state, TargetRef::ThreatSpot(7), &s.cx(0), &mut rng), Step::Done("held the spot for his return"));
    }

    #[test]
    fn the_flanker_goes_off_his_facing_and_watches_him() {
        let mut s = holder();
        let him = Vec3::new(55.0, 0.0, 30.0);
        let steps = s.run(0, PlayKind::Flank, TargetRef::Place(1), 80, 300, 3.5);
        assert!(
            steps
                .iter()
                .any(|st| matches!(st, Step::Continue(i) if i.go == Go::Stay && i.crouch && i.look == Look::At(him))),
            "{steps:?}"
        );
        assert!(matches!(steps.last(), Some(Step::Done(_))), "{steps:?}");
        // Where it ended: 15-30 m from him, well off his facing.
        let at = s.view.places[0].feet;
        let d = at.distance_xz(him);
        assert!((13.0..=32.0).contains(&d), "{at:?} {d}");
        let off = degrees_between([-1.0, 0.0], [at.x - him.x, at.z - him.z]);
        assert!(off >= 65.0, "{at:?} {off}");
        assert!(!s.tactics.in_spawn_area(at, None), "{at:?}");
    }
}
