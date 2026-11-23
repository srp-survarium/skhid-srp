//! [anomaly batteries] Collect's take of a battery whose stand lies in an energy core's zones that switch (level_02's fruit
//! cloud: `switch_take`'s forecast). No crossing reaches such a stand (`collect::crossing_to` finds no dry floor in a line, and
//! the routed crossing walks only zones on all match), so the take walks its own way: from the cheapest dry entry over walk
//! links to the stand ([`switch_take::way_in`]), the stand, and the same way back out, by straight steps the play vouches for
//! (`Intent::straight` + `Intent::zone_ok`; no route, no way out of the zone walked for it meanwhile).
//!
//! The decision ([`decide`]), one event line each time it changes:
//! - rule (a): no zone of the core that switches comes on over the plan (the energy forecast with this body's move, the
//!   others moving in or bound there, and a container's use at the take), and the cost of the zones on all the while (the
//!   core's low states) is borne by the crossings' spend ([`Ctx::zone_spend`]);
//! - rule (b): else, with its team behind on points, the whole plan's cost (the zones that come on included, each at the
//!   floor's place in it, through the bot's armour into the pools) borne by that spend, which keeps a reserve in every pool it
//!   takes from.
//!
//! The take re-forecasts what is left every [`CHECK_MS`] and walks out at once when the rule that let it in no longer holds
//! (or neither does). The stand is the client's use ([`hold_ms`]): the press on the tick the view ray meets the box, carried
//! once the holster ends; a world past that with no carry is a use that started nothing, and the take walks out.

use std::sync::Arc;

use mapdata::{NodeId, Vec3};

use super::{Ctx, Intent, Look, Pace, Step, TargetRef};
use crate::v026g::bots::anomalies::EnergyCore;
use crate::v026g::bots::nav_log::{self, Kind, PREFIX};
use crate::v026g::bots::switch_take::{self, CARRIER_SHARE, Forecast, Pace as Gait, Part, RUN_MPS, SPRINT_MPS};
use crate::v026g::bots::zone_cost::{Cost, Reserve};

/// [anomaly batteries] How long a use of a battery on the ground takes from the press to the carry, ms: the holster of the
/// weapon in the hands, `hide_time / (1 + S22)` on the 50 ms tick (`tick_active_object` 0x2D7650, the holster's clip
/// stretched over `max(0, 1 + S22)` 0x30D9C0 -> 0x30DF10; `docs/notes/using_the_world.md`: the rule matched 1,958 of 1,973
/// bot takes in the live recordings). The largest `hide_time` of the game data's weapons with S22 at 0 (a skill only
/// shortens it): the longest any bot's use takes. The carry state's `take_time` (0.5 s, `states[0]/take_time`) plays after
/// the carry and does not hold the body.
pub fn hold_ms() -> u32 {
    static HOLD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *HOLD.get_or_init(|| {
        let tick = crate::v026g::bots::anomalies::TICK_MS;
        let longest = crate::v026g::bots::guns::guns().iter().map(|g| g.hide_ms).max().unwrap_or(0);
        longest.div_ceil(tick) * tick
    })
}

/// [`hold_ms`] in seconds, for the forecast's stand.
fn hold_s() -> f32 {
    hold_ms() as f32 / 1000.0
}
/// How often the take forecasts what is left of it again, ms.
pub const CHECK_MS: u32 = 500;
/// A body moving toward the way's entry or the stand and this near it, m (across), counts as bound for the core's zones (a
/// step or two from going in; a teammate walking along farther off, an escort, is not).
pub const HEADING_IN_M: f32 = 5.0;
/// A body moving at least this fast, m/s, adds its rise (the client: a speed of 0.001 or more, `notify_actions_subscribers`;
/// the view's velocity is a world's, so a little slack).
const MOVING_MPS: f32 = 0.3;
/// The way's next point the straight walk heads for, m along the way ahead of the body.
const AHEAD_M: f32 = 2.5;
/// A walk on the way (in or out) has stalled when it made less than this share of its pace's way over [`STALL_WINDOW_MS`],
/// or stayed on one floor of the way over [`STALL_LEG_MS`]: a straight walk has no stuck or pocket handling of its own
/// (`Intent::straight`), so the take watches it.
const STALL_SHARE: f32 = 0.5;
const STALL_WINDOW_MS: u32 = 1_000;
const STALL_LEG_MS: u32 = 2_000;
/// A walk's start (from a stand, or turned round for the way out) is given this long before the watch runs, ms.
const STALL_GRACE_MS: u32 = 500;
/// The pace a walk on the way is expected to make, m/s: a carrier's walk (`kin::CarrierSprint`, the run's 3.5 x 0.95), the
/// slowest the take walks; a sprint is asked for, not sure (the way may not allow it).
const STALL_MPS: f32 = RUN_MPS * CARRIER_SHARE;
/// The last stretch of the way in, m: at a walk with the view on the battery, USE pressed when the ray meets it (the client
/// refuses a use while sprinting, `can_use` 0x300710).
const LAST_M: f32 = 3.0;

/// Which rule let the take in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// (a): no zone that switches comes on over the plan.
    Off,
    /// (b): its team behind, the plan's whole cost borne with the reserve.
    Behind,
}

/// A take decided on: its way in (dry entry first, the stand last), the gait in and the forecast that let it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub way: Arc<Vec<NodeId>>,
    pub gait: Gait,
    pub rule: Rule,
    pub forecast: Forecast,
}

/// Where a take stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Walking the way in.
    In,
    /// At the stand since `since`.
    Stand { since: u32 },
    /// Walking the way back out, for `why`.
    Out { why: &'static str },
}

/// A take under way.
#[derive(Clone, Debug, PartialEq)]
pub struct Take {
    pub plan: Plan,
    pub phase: Phase,
    /// The floor of the way the body is at (its index).
    pub along: usize,
    /// When what is left was last forecast.
    pub checked: u32,
    /// The pools at the begin: the exit line's loss against the forecast.
    pub start: Reserve,
    /// The stall watch of the walk in or out.
    pub watch: Watch,
    /// When the take began, and when its USE was pressed (the drive's press on the view ray's meet), if it was.
    pub begun: u32,
    pub pressed: Option<u32>,
}

/// [stall] A straight walk's progress along the way: since when the window runs and the metres along the way then; the floor
/// of the way the body is on and since when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Watch {
    window: (u32, f32),
    leg: (usize, u32),
}

impl Watch {
    /// Afresh at `now`, the body on floor `along` of the way, `m` metres along it: it runs from [`STALL_GRACE_MS`] on.
    pub fn from(now: u32, along: usize, m: f32) -> Self {
        let start = now + STALL_GRACE_MS;
        Self { window: (start, m), leg: (along, start) }
    }

    /// Whether the walk at `mps` stalled ([`STALL_SHARE`]), the body on floor `along`, `m` metres along the way at `now`:
    /// why, else `None`.
    pub fn stalled(&mut self, now: u32, along: usize, m: f32, mps: f32) -> Option<&'static str> {
        if along != self.leg.0 {
            self.leg = (along, now);
        } else if now > self.leg.1 + STALL_LEG_MS {
            return Some("stalled: on one floor of the way over 2 s");
        }
        let (since, from) = self.window;
        if now >= since + STALL_WINDOW_MS {
            let expected = mps * now.saturating_sub(since) as f32 / 1000.0;
            if (m - from).abs() < STALL_SHARE * expected {
                return Some("stalled: under half its pace's way over 1 s");
            }
            self.window = (now, m);
        }
        None
    }
}

/// What a take's step says.
pub enum Next {
    Step(Step),
    /// Out of the zones with the battery: home with it.
    Bring,
}

/// The energy core whose zone that switches marks the floor `stand`, if one does.
fn core_at<'a>(cx: &Ctx<'a>, stand: NodeId) -> Option<EnergyCore<'a>> {
    let hazards = &cx.tactics.hazards;
    let anomalies = hazards.anomalies()?;
    hazards.zones_at(stand).filter(|zone| zone.switches()).find_map(|zone| anomalies.energy_core(zone.zone))
}

/// The floor of the stand to take the battery lying at `at` from.
fn stand_node(cx: &Ctx, at: Vec3) -> Option<NodeId> {
    let stand = cx.tactics.stand_to_take(at)?;
    cx.tactics.world.grid.locate(stand, 3.0)
}

/// Whether the battery lying at `at` stands in an energy core's zones that switch: its take is this module's.
pub fn in_core(cx: &Ctx, at: Vec3) -> bool {
    cx.tuning.take.switch_take
        && stand_node(cx, at).is_some_and(|node| core_at(cx, node).is_some())
}

/// The other bodies moving in the core's zones (the feet or the body's middle in a zone's own shape, no margin), or bound there
/// (within [`HEADING_IN_M`] of `entry` or `stand` and moving toward it): how many, and what they add a second (each at the
/// core's sprint rise, the most a body adds: their gait is not known).
fn others(cx: &Ctx, core: EnergyCore, entry: Vec3, stand: Vec3) -> (usize, f32) {
    let hazards = &cx.tactics.hazards;
    let in_shape = |feet: Vec3| {
        let middle = feet + Vec3::new(0.0, 0.9, 0.0);
        hazards
            .zones
            .iter()
            .filter(|zone| core.has_zone(zone.zone))
            .any(|zone| zone.shapes.iter().any(|shape| shape.contains(feet) || shape.contains(middle)))
    };
    let n = cx
        .view
        .places
        .iter()
        .filter(|p| p.place != cx.me.place && p.present && p.alive)
        .filter(|p| p.velocity[0].hypot(p.velocity[1]) >= MOVING_MPS)
        .filter(|p| {
            let toward = |to: Vec3| {
                p.feet.distance_xz(to) <= HEADING_IN_M && p.velocity[0] * (to.x - p.feet.x) + p.velocity[1] * (to.z - p.feet.z) > 0.0
            };
            toward(entry)
                || toward(stand)
                || in_shape(p.feet)
        })
        .count();
    (n, n as f32 * core.rise().sprint)
}

/// The forecast of the plan left from floor `along` of `way` in: the rest of the way in at `gait`, `stand_s` at the stand,
/// the container's use, the whole way back out at a carrier's pace (`out_from`: the floor the way out starts from).
#[allow(clippy::too_many_arguments)]
fn forecast_rest(
    cx: &Ctx,
    core: EnergyCore,
    way: &[NodeId],
    along: usize,
    gait: Gait,
    stand_s: f32,
    bump: bool,
    others_per_s: f32,
) -> Forecast {
    let mps = match gait {
        Gait::Sprint => SPRINT_MPS,
        Gait::Run | Gait::Walk => RUN_MPS,
    };
    let back: Vec<NodeId> = way.iter().rev().copied().collect();
    let stand = *way.last().expect("a way has its stand");
    let mut parts = Vec::with_capacity(4);
    let rest = &way[along.min(way.len() - 1)..];
    if rest.len() > 1 {
        parts.push(Part::Walk { nodes: rest, mps, pace: gait });
    }
    if stand_s > 0.0 {
        parts.push(Part::Stand { node: stand, seconds: stand_s });
    }
    if bump {
        parts.push(Part::Bump { energy: core.container_use() });
    }
    parts.push(Part::Walk { nodes: &back, mps: RUN_MPS * CARRIER_SHARE, pace: Gait::Run });
    let rule = super::memo::RoutedRule::of(&cx.tuning.hazard, cx.tactics.world.grid.body().radius);
    switch_take::forecast(&cx.tactics.hazards, &cx.tactics.world.grid, core, &parts, cx.now, others_per_s, cx.zone_armour(), rule.hurt)
}

/// The forecast out from floor `along` now (the way back from there), at a carrier's pace.
fn forecast_out(cx: &Ctx, core: EnergyCore, way: &[NodeId], along: usize, others_per_s: f32) -> Forecast {
    let back: Vec<NodeId> = way[..=along.min(way.len() - 1)].iter().rev().copied().collect();
    let parts = [Part::Walk { nodes: &back, mps: RUN_MPS * CARRIER_SHARE, pace: Gait::Run }];
    let rule = super::memo::RoutedRule::of(&cx.tuning.hazard, cx.tactics.world.grid.body().radius);
    switch_take::forecast(&cx.tactics.hazards, &cx.tactics.world.grid, core, &parts, cx.now, others_per_s, cx.zone_armour(), rule.hurt)
}

/// This team's points over the other's.
fn score_gap(cx: &Ctx) -> f32 {
    let team = usize::from(cx.me.team & 1);
    cx.view.points[team] as f32 - cx.view.points[1 - team] as f32
}

/// Rules (a) and (b) on `f`: the rule that lets it, else why each failed.
fn judge(cx: &Ctx, f: &Forecast, spend_left: f32, spend_share: f32) -> Result<Rule, String> {
    let budget = cx.budget();
    let borne = budget.reserve.refusal(f.cost, spend_share, spend_left);
    let a = match (f.switch_on, &borne) {
        (false, None) => return Ok(Rule::Off),
        (true, _) => format!(
            "rule (a) checked: a zone that switches is on {:.2} s of the plan (energy peak {:.0}, state threshold {})",
            f.on_s,
            f.energy_peak,
            f.peak_threshold.map_or("-".to_owned(), |t| format!("{t:.0}"))
        ),
        (false, Some(why)) => format!("rule (a) checked: no zone that switches comes on, but the cost is not borne: {why}"),
    };
    let gap = score_gap(cx);
    let b = if gap >= 0.0 {
        format!("rule (b): the team is not behind (points {gap:+.0})")
    } else if let Some(why) = borne {
        format!("rule (b): behind {gap:+.0}, the cost is not borne: {why}")
    } else {
        return Ok(Rule::Behind);
    };
    Err(format!("{a}; {b}"))
}

/// The event line of a decision or a take's turn.
#[allow(clippy::too_many_arguments)]
fn line(cx: &Ctx, at: Vec3, core: EnergyCore, what: &str, entry: Vec3, way_m: f32, gait: Gait, n_others: usize, f: &Forecast) -> String {
    let budget = cx.budget();
    let r = &budget.reserve;
    let energy = core.energy();
    format!(
        "{PREFIX}zone take battery at {} ({}, energy {energy:.0}, state {}): {what}; way {way_m:.1} m from {}, gait {gait:?}, \
         {:.2} s moving + {:.2} s standing, energy peak {:.0} end {:.0} (threshold {}), switching zone on {:.2} s, others moving {n_others} \
         (+{:.0}/s at sprint), container use +{:.0} counted (not proven), damage {}, health body {:.3} head {:.3} infection {:.3} pain {:.3}",
        nav_log::at(at),
        core.name(),
        core.state_at(energy).map_or("-".to_owned(), |s| s.to_string()),
        nav_log::at(entry),
        f.moving_s,
        f.standing_s,
        f.energy_peak,
        f.energy_end,
        f.peak_threshold.map_or("-".to_owned(), |t| format!("{t:.0}")),
        f.on_s,
        n_others as f32 * core.rise().sprint,
        core.container_use(),
        cost_text(f.cost),
        r.body,
        r.head,
        r.infection,
        r.pain,
    )
}

fn cost_text(c: Cost) -> String {
    format!("body {:.3} head {:.3} infection {:.3} pain {:.3}", c.body, c.head, c.infection, c.pain)
}

/// [anomaly batteries] Whether the battery lying at `at` (its stand in a core's zones that switch, [`in_core`]) is taken now,
/// by which rule and plan; else why not. Each verdict writes its event line once per battery, verdict, energy step and health
/// step (`Ctx::lines`, keyed); `every`: each time (the walk to the entry's own decision).
pub fn decide(cx: &Ctx, at: Vec3, every: bool) -> Result<Plan, &'static str> {
    let Some(stand) = stand_node(cx, at) else {
        return Err("no stand to take it from");
    };
    let Some(core) = core_at(cx, stand) else {
        return Err("no energy core at its stand");
    };
    let Some(side) = cx.bot.side else {
        return Err("no side known");
    };
    let grid = &cx.tactics.world.grid;
    let key = |verdict: &str, energy: f32, n: usize| {
        (!every).then(|| nav_log::key((
            "zone take",
            (at.x * 4.0) as i32,
            (at.z * 4.0) as i32,
            verdict.to_owned(),
            (energy / 250.0) as i32,
            (cx.budget().health() * 10.0) as i32,
            n,
            cx.view.points,
        )))
    };
    let Some(way) = cx.memo.zone_way(cx.tactics, core, side, stand) else {
        let text = format!(
            "{PREFIX}zone take battery at {} ({}): refused: no way in over walk links from a dry floor the side reaches",
            nav_log::at(at),
            core.name()
        );
        cx.lines.push(Kind::ZoneTake, key("no way", 0.0, 0), text);
        return Err("no way into its zone");
    };
    let entry = grid.position(way[0]);
    let way_m = switch_take::way_m(grid, &way);
    let (n, others_per_s) = others(cx, core, entry, grid.position(stand));
    // The gait in with the least damage (the most of any pool), then the lower energy peak.
    let (gait, f) = [Gait::Run, Gait::Sprint]
        .into_iter()
        .map(|gait| (gait, forecast_rest(cx, core, &way, 0, gait, hold_s(), true, others_per_s)))
        .min_by(|(_, a), (_, b)| a.cost.most().total_cmp(&b.cost.most()).then(a.energy_peak.total_cmp(&b.energy_peak)))
        .expect("two gaits");
    let spend = cx.zone_spend();
    match judge(cx, &f, spend.left, spend.share) {
        Ok(rule) => {
            let what = match rule {
                Rule::Off => "taken by rule (a)".to_owned(),
                Rule::Behind => format!("taken by rule (b): behind {:+.0}", score_gap(cx)),
            };
            if !every {
                cx.lines.push(Kind::ZoneTake, key(&what, core.energy(), n), line(cx, at, core, &what, entry, way_m, gait, n, &f));
            }
            Ok(Plan { way, gait, rule, forecast: f })
        }
        Err(why) => {
            let what = format!("refused: {why}");
            cx.lines.push(Kind::ZoneTake, key("refused", core.energy(), n), line(cx, at, core, &what, entry, way_m, gait, n, &f));
            Err("its zone's take is refused")
        }
    }
}

/// The event line of a walk to a take's entry given up (`collect::State::Fetch`): the verdict now, `why` (the decision's own line
/// says what it forecast, unless the same one was written before).
pub fn fetch_ended(cx: &Ctx, at: Vec3, why: &str) {
    cx.lines.push(Kind::ZoneTake, None, format!("{PREFIX}zone take battery at {}: walk to its entry ended: {why}", nav_log::at(at)));
}

impl Take {
    /// A take of `plan` begun at `now` (the body at its entry); its event line.
    pub fn begin(cx: &Ctx, at: Vec3, plan: Plan) -> Self {
        if let Some(core) = core_at(cx, *plan.way.last().expect("a way has its stand")) {
            let grid = &cx.tactics.world.grid;
            let text = line(cx, at, core, "begun", grid.position(plan.way[0]), switch_take::way_m(grid, &plan.way), plan.gait, 0, &plan.forecast);
            cx.lines.push(Kind::ZoneTake, None, text);
        }
        Self { plan, phase: Phase::In, along: 0, checked: cx.now, start: cx.budget().reserve, watch: Watch::from(cx.now, 0, 0.0), begun: cx.now, pressed: None }
    }
}

/// The pools lost since `start`, against the plan's forecast `forecast`: the exit line's part.
fn loss_text(cx: &Ctx, start: Reserve, forecast: Cost) -> String {
    let now = cx.budget().reserve;
    format!(
        "lost body {:.3} head {:.3} infection {:.3} pain {:.3} (enemy fire counts), forecast for the whole plan {}",
        start.body - now.body,
        start.head - now.head,
        start.infection - now.infection,
        start.pain - now.pain,
        cost_text(forecast)
    )
}

/// How far along the way from its last floor the body is looked for, m: more than a sprint covers between two of the play's
/// steps (the floors are 0.25 m apart; a window of floors lagged a sprinting body, and the point ahead fell behind it).
const TRACK_M: f32 = 8.0;

/// The floor of `way` the body at `feet` is at: the nearest within [`TRACK_M`] of `along` along the way (ahead on the way
/// in, back on the way out).
fn track(grid: &mapdata::WalkGrid, way: &[NodeId], along: usize, feet: Vec3, out: bool) -> usize {
    let along = along.min(way.len() - 1);
    let gap = |i: usize| (grid.position(way[i]) - feet).length();
    let (mut best, mut best_d) = (along, gap(along));
    let (mut i, mut walked) = (along, 0.0);
    loop {
        let next = if out { i.checked_sub(1) } else { (i + 1 < way.len()).then_some(i + 1) };
        let Some(next) = next else { break };
        walked += (grid.position(way[next]) - grid.position(way[i])).length();
        if walked > TRACK_M {
            break;
        }
        i = next;
        let d = gap(i);
        if d < best_d {
            (best, best_d) = (i, d);
        }
    }
    best
}

/// The point [`AHEAD_M`] along `way` from floor `from`, toward its end (`out`: toward its start).
fn ahead(grid: &mapdata::WalkGrid, way: &[NodeId], from: usize, out: bool) -> (usize, Vec3) {
    let mut i = from;
    let mut d = 0.0;
    loop {
        let next = if out { i.checked_sub(1) } else { (i + 1 < way.len()).then_some(i + 1) };
        let Some(next) = next else {
            return (i, grid.position(way[i]));
        };
        d += (grid.position(way[next]) - grid.position(way[i])).length();
        i = next;
        if d >= AHEAD_M {
            return (i, grid.position(way[i]));
        }
    }
}

/// One step of a take under way (`collect::State::ZoneTake`).
pub fn step(take: &mut Take, target: TargetRef, cx: &Ctx) -> Next {
    let grid = &cx.tactics.world.grid;
    let way = take.plan.way.clone();
    let stand_node = *way.last().expect("a way has its stand");
    let Some(core) = core_at(cx, stand_node) else {
        return Next::Step(Step::Failed("no energy core at its stand"));
    };
    let me = cx.me.feet;
    let carrying = cx.me.carrying.is_some();
    let lying = match target {
        TargetRef::Battery(b) => cx.battery_lying(b),
        _ => None,
    };
    let at = lying.unwrap_or_else(|| grid.position(stand_node));
    let out = matches!(take.phase, Phase::Out { .. });
    take.along = track(grid, &way, take.along, me, out);
    let entry = grid.position(way[0]);
    let way_m = switch_take::way_m(grid, &way);
    let along_m = switch_take::way_m(grid, &way[..=take.along.min(way.len() - 1)]);
    let leave = |take: &mut Take, why: &'static str, f: Option<&Forecast>| {
        let (n, others_per_s) = others(cx, core, entry, grid.position(stand_node));
        let f = f.copied().unwrap_or_else(|| forecast_out(cx, core, &way, take.along, others_per_s));
        cx.lines.push(Kind::ZoneTake, None, line(cx, at, core, &format!("walking out: {why}"), entry, way_m, take.plan.gait, n, &f));
        take.phase = Phase::Out { why };
        take.watch = Watch::from(cx.now, take.along, along_m);
    };
    // [stall] The way in or out walked by straight steps makes its pace's way, or the take gives it up: in, it walks out;
    // out, it hands the bot back to the motor at once (its own stuck and pocket handling, its way out of the zone).
    let mps = match take.phase {
        Phase::In | Phase::Out { .. } => STALL_MPS,
        Phase::Stand { .. } => 0.0,
    };
    if mps > 0.0
        && let Some(stall) = take.watch.stalled(cx.now, take.along, along_m, mps)
    {
        // The stall's trace: the feet, the floor of the way and where the next straight step went.
        cx.lines.push(
            Kind::ZoneTake,
            None,
            format!(
                "{PREFIX}zone take battery at {}: {stall}: feet {}, floor {} of {} ({along_m:.1} of {way_m:.1} m), heading for {}",
                nav_log::at(at),
                nav_log::at(me),
                take.along,
                way.len(),
                nav_log::at(ahead(grid, &way, take.along, out).1)
            ),
        );
        if out {
            let f = forecast_out(cx, core, &way, take.along, 0.0);
            let what = format!("{stall} on the way out: handed to the motor; {}", loss_text(cx, take.start, take.plan.forecast.cost));
            cx.lines.push(Kind::ZoneTake, None, line(cx, at, core, &what, entry, way_m, take.plan.gait, 0, &f));
            return Next::Step(Step::Failed("stalled on the way out of its zone"));
        }
        leave(take, stall, None);
    }
    let out = matches!(take.phase, Phase::Out { .. });
    // The take's USE, pressed by the drive on the tick the view ray met the box (kept: the drive's note lasts while asked).
    if take.pressed.is_none()
        && let Some((pressed_at, time)) = cx.bot.use_pressed
        && (pressed_at - at).length() < 0.01
        && time >= take.begun
    {
        take.pressed = Some(time);
    }
    if !out {
        if carrying {
            leave(take, "carrying the battery", None);
        } else if lying.is_none() {
            leave(take, "the battery is gone", None);
        } else if let Some(pressed) = take.pressed
            && cx.me.seen_at >= pressed + hold_ms()
        {
            // A world past the press and the hold, and no carry: the use started nothing (an ended use starts whole again at
            // a new press, 0x2D88B1; no second hold was forecast).
            leave(take, "the use started nothing (a world past the press and the hold shows no carry)", None);
        } else if take.pressed.is_none()
            && let Phase::Stand { since } = take.phase
            && cx.now > since + STALL_LEG_MS
        {
            leave(take, "stalled: no press at the stand over 2 s", None);
        } else if cx.now >= take.checked + CHECK_MS {
            take.checked = cx.now;
            // What is left, forecast again from here: the rule that let the take in, or the other, must still hold, and the
            // pools keep what a crossing walking inside gives up at (`Ctx::zone_give_up_left`).
            let (n, others_per_s) = others(cx, core, entry, grid.position(stand_node));
            let stand_s = match take.pressed {
                Some(pressed) => (pressed + hold_ms()).saturating_sub(cx.now) as f32 / 1000.0,
                None => hold_s(),
            };
            let along = if matches!(take.phase, Phase::Stand { .. }) { way.len() - 1 } else { take.along };
            let f = forecast_rest(cx, core, &way, along, take.plan.gait, stand_s.max(0.0), true, others_per_s);
            let spend = cx.zone_spend();
            let verdict = judge(cx, &f, spend.left, spend.share);
            let leaves = cx.budget().reserve.leaves(f.cost, cx.zone_give_up_left());
            match verdict {
                Ok(rule) if leaves => {
                    if rule != take.plan.rule {
                        let what = format!("goes on by rule {}", if rule == Rule::Off { "(a)" } else { "(b)" });
                        cx.lines.push(Kind::ZoneTake, None, line(cx, at, core, &what, entry, way_m, take.plan.gait, n, &f));
                        take.plan.rule = rule;
                    }
                }
                Ok(_) => leave(take, "the pools would fall under the give-up reserve", Some(&f)),
                Err(_) => leave(take, "no rule lets the rest of it", Some(&f)),
            }
        }
    }
    match take.phase {
        Phase::Out { why } => {
            // Out at the dry entry (or any dry floor on the way): home with the battery, else the play ends.
            let here = grid.locate(me, 1.5);
            let dry = here.is_some_and(|node| cx.tactics.hazards.hazard_free(node));
            if take.along == 0 || (dry && take.along < way.len() / 2) {
                let f = forecast_out(cx, core, &way, 0, 0.0);
                let what = format!("out ({why}); {}", loss_text(cx, take.start, take.plan.forecast.cost));
                cx.lines.push(Kind::ZoneTake, None, line(cx, at, core, &what, entry, way_m, take.plan.gait, 0, &f));
                return if carrying { Next::Bring } else { Next::Step(Step::Failed(why)) };
            }
            let (_, to) = ahead(grid, &way, take.along, true);
            let pace = if carrying { Pace::Walk } else { Pace::Run };
            Next::Step(Step::Continue(Intent::to(to, pace).within(0.3).straight().zone_ok()))
        }
        Phase::In => {
            if cx.in_use_reach(at) || take.along + 1 >= way.len() {
                take.phase = Phase::Stand { since: cx.now };
                return step(take, target, cx);
            }
            let to_stand = (grid.position(stand_node) - me).length();
            let (_, to) = ahead(grid, &way, take.along, false);
            if to_stand <= LAST_M {
                let intent = Intent::to(to, Pace::Walk).within(0.3).straight().zone_ok().looking(Look::Use(at));
                let intent = if take.pressed.is_none() { intent.pressing_use_on_meet(at) } else { intent.hold_fire() };
                return Next::Step(Step::Continue(intent));
            }
            let pace = match take.plan.gait {
                Gait::Sprint => Pace::Run,
                Gait::Run | Gait::Walk => Pace::Walk,
            };
            Next::Step(Step::Continue(Intent::to(to, pace).within(0.3).straight().zone_ok()))
        }
        Phase::Stand { .. } => {
            // Pressed: still, the view on it, no fire (a use goes on with no key or look, 0x300830; a firing weapon only
            // delays the holster). Not yet: the press on the tick the view ray meets the box, from within the use's reach.
            let intent = match take.pressed {
                Some(_) => Intent::stay().looking(Look::Use(at)).zone_ok().hold_fire(),
                None if cx.in_use_reach(at) => Intent::stay().looking(Look::Use(at)).zone_ok().pressing_use_on_meet(at),
                None => Intent::to(grid.position(stand_node), Pace::Walk)
                    .within(0.0)
                    .straight()
                    .zone_ok()
                    .looking(Look::Use(at))
                    .pressing_use_on_meet(at),
            };
            Next::Step(Step::Continue(intent))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The take's hold is the longest holster of the game data's weapons, on the 50 ms tick (printed: the PR's table).
    #[test]
    fn the_hold_is_the_longest_holster_on_the_tick() {
        let longest = crate::v026g::bots::guns::guns().iter().max_by_key(|g| g.hide_ms).expect("weapons");
        eprintln!("hold {} ms: {} hide_time {} ms", hold_ms(), longest.item, longest.hide_ms);
        assert_eq!(hold_ms() % 50, 0);
        assert!(hold_ms() >= longest.hide_ms && hold_ms() < longest.hide_ms + 50);
    }

    /// [stall] A walk making its pace goes on; under half its pace's way over 1 s, or one floor over 2 s, is a stall.
    #[test]
    fn a_walk_short_of_half_its_pace_or_on_one_floor_stalls() {
        // 3.5 m/s: 1.75 m a second is half.
        let mut w = Watch::from(0, 0, 0.0);
        for (k, t) in (50..=3_000).step_by(50).enumerate() {
            assert_eq!(w.stalled(t, k + 1, t as f32 / 1000.0 * 3.0, 3.5), None, "{t}");
        }
        // Nothing the first half second (the start), then under half the pace over the next second.
        let mut w = Watch::from(0, 0, 0.0);
        let mut why = None;
        for (k, t) in (50..=1_500).step_by(50).enumerate() {
            assert!(t >= 1_500 || why.is_none());
            why = why.or(w.stalled(t, k + 1, t as f32 / 1000.0 * 1.0, 3.5));
        }
        assert_eq!(why, Some("stalled: under half its pace's way over 1 s"));
        // Making way but held on one floor (a long leg the tracking does not leave): stalled 2 s after the start.
        let mut w = Watch::from(0, 4, 0.0);
        let mut why = None;
        for t in (50..=2_600).step_by(50) {
            why = why.or(w.stalled(t, 4, t as f32 / 1000.0 * 3.0, 3.5));
        }
        assert_eq!(why, Some("stalled: on one floor of the way over 2 s"));
    }
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::tactics::Tactics;
    use crate::v026g::bots::view::Mode;

    /// Chemical Plant's batteries under the fruit cloud (level_02, batteries): each one's verdict from both spawns, at the
    /// cloud's energy 0, 500 and 1200, the team even or behind, with its event line.
    #[test]
    #[ignore = "needs the private map files"]
    fn chemical_plant_fruit_batteries_verdicts() {
        let dir = crate::v026g::bots::tests::real_maps::dir().expect("set SRP_MAPDATA_DIR");
        let defaults = crate::v026g::bots::tuning::Tuning::default();
        let map = mapdata::MapData::read(&std::path::Path::new(&dir).join("level_02_day_2.mapdata")).unwrap();
        let world = std::sync::Arc::new(mapdata::World::build_with_links(map, mapdata::CELL, defaults.grid_body(), defaults.off_mesh_links()));
        let (mut in_cloud, mut taken) = (0, 0);
        for team in [0u8, 1] {
            let spawn = world.map.spawns.iter().find(|p| p.team == i32::from(team)).unwrap().position;
            for energy in [0.0, 500.0, 1200.0] {
                for behind in [false, true] {
                    let mut s = Scenario::field(Mode::Batteries);
                    s.tuning = defaults.clone();
                    s.tactics = Tactics::new(world.clone(), crate::v026g::bots::hazards::Marking::of(&defaults.hazard));
                    let mut anomalies = crate::v026g::bots::anomalies::Anomalies::new(&world.map, 0);
                    anomalies.advance(1_000);
                    for core in 0..world.map.anomalies.len() {
                        anomalies.set_energy(core, energy);
                    }
                    s.tactics.hazards.attach(anomalies);
                    s.side = Some(i32::from(team));
                    s.view.points = if behind { [u32::from(team == 1), u32::from(team == 0)] } else { [0, 0] };
                    s.bot(0, team, spawn);
                    let cx = s.cx(0);
                    for (i, spawner) in world.map.battery_spawners.iter().enumerate() {
                        if !in_core(&cx, spawner.item) {
                            continue;
                        }
                        in_cloud += 1;
                        let verdict = decide(&cx, spawner.item, false);
                        taken += usize::from(verdict.is_ok());
                        eprintln!(
                            "team {team} energy {energy} behind {behind} spawner {i}: {:?}",
                            verdict.as_ref().map(|p| (p.rule, p.gait, p.way.len(), p.forecast.switch_on, p.forecast.cost.most()))
                        );
                        for (_, _, text) in cx.lines.take() {
                            eprintln!("    {text}");
                        }
                    }
                }
            }
        }
        eprintln!("fruit batteries asked {in_cloud}, taken {taken}");
        assert!(in_cloud > 0, "batteries under the fruit cloud");
    }
}
