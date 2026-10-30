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
    // [K2] The team's job on a held spot gives this bot the flank: the
    // kills tell where he is and which way he faces, not his keys (a human
    // camper stands and peeks, RC1). The gate below stays a second trigger.
    if cx.tuning.threat.job
        && let Some((job, Role::Flanker, line)) = cx.team.threat.job_of(cx.me.place)
    {
        let has_spot = matches!(cx.bot.current, Some((PlayKind::Flank, TargetRef::ThreatSpot(id), _)) if id == line.id)
            || !job_flank_spots(cx, line).is_empty();
        #[cfg(test)]
        if !has_spot {
            let why = JOB_SPOTS_WHY.with(|m| m.borrow().get(&(cx.bot.side, line.id)).copied()).unwrap_or("unknown");
            crate::v026g::bots::threat::no_spot_why(cx.me.place, Role::Flanker, why);
            crate::v026g::bots::threat::no_spot_centre(Role::Flanker, &cx.tactics.world.grid, line.at);
        }
        out.extend(cx.job_offer(Role::Flanker, PlayKind::Flank, TargetRef::ThreatSpot(line.id), JOB_KEY + job.spot, JOB_HEALTH, has_spot));
    }
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

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    // The team's job (K2): him at the spot, facing down his death lanes.
    if let Some(line) = job_line(cx, target) {
        let spots = job_flank_spots(cx, line);
        return match round(cx, line.at, line.facing_dir(), &spots, rng) {
            State::Wait { .. } => State::NoSpot,
            state => state,
        };
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

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Flank, name);
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return Step::Failed("flanked too long");
    }
    let Some(him) = cx.point_of(target) else {
        return Step::Done("he is gone from the memory");
    };
    // The team's job ended: the spot is no longer held.
    if matches!(target, TargetRef::ThreatSpot(_)) && job_line(cx, target).is_none() {
        return Step::Done("the spot is no longer held");
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
