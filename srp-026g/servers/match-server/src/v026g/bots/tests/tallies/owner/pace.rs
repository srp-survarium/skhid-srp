//! The owner counts of the sprint on calm stretches (N1, `pace.calm_sprint`, `bots/pace.rs`), by the bot's team:
//!
//! - **The calm sprint by the true nearest enemy** (walking ticks and the sprinting ones, bands 0-15 / 15-30 / 30-45 / 45-60 /
//!   60-90 / 90+ m): the human curve is 36 / 43 / 51 / 59 / 66 / 77 % (144 live matches of 12+ players, push-record N1 §1.6). A walking tick
//!   is a body alive and not carrying with a move key held and neither AIM nor FIRE; a sprint one has the sprint keys.
//! - **The gate** (the ticks a play handed `Cruise`): how many ran, how many walked and for what (a track of the bot's memory or a death spot, in the
//!   ring of the feet or of a point of the path ahead; the track's age; the radar cheat's tracks apart), the walks held after the ring, and the
//!   flips between run and walk (a ring-edge flicker would show here).
//! - **The sprint key**: its presses a bot-minute, and the bursts (the keys held): how many, median and p90 length, and why each ended (the map: a
//!   corner, a jump, an arrival; the gate; the bar's reserve).
//! - **The guard on deaths after a calm sprint leg**: a leg is a burst that starts with no enemy within 30 m; the control is a body-second of calm walking
//!   with no sprint; both counted when the body is dead 4.5-8 s later (the live players: 2.77 % of 23,129 legs, control 4.39 %).
//! - **The bar at contact**: the bot's own bar when it gives or takes a hit after 5 s without one (the live players' median is 97).
//!
//! What it cannot say: the bench's matches are 180 s and its enemies stand in for the live ones, so the levels differ; the counts are for the arms of
//! one bench against each other.

use super::*;
use crate::v026g::bots::kin::Kinematics;
use crate::v026g::bots::pace::{Age, Cause, Danger, Found, Gate, Walk};
use crate::v026g::bots::plays::Pace;

/// The bands of the true nearest living enemy, metres (upper edges; the last is open).
const EDGES_M: [f32; 5] = [15.0, 30.0, 45.0, 60.0, 90.0];
const BANDS: usize = 6;
const BAND_NAMES: [&str; BANDS] = ["0_15", "15_30", "30_45", "45_60", "60_90", "90_up"];

/// A calm stretch: no living enemy within this, metres (the live census's, and `Rec::calm`'s).
const CALM_M: f32 = 30.0;

/// A death counts for a leg when it comes this long after the leg's start, ms.
const LEG_FROM_MS: u32 = 4_500;
const LEG_TO_MS: u32 = 8_000;

/// A hit is a contact when none came to the body in this long before it, ms.
const QUIET_MS: u32 = 5_000;

/// A sprint is heard by an enemy within this, metres (`SenseTuning::sprint_m` 15 and the hearing bonus 1.25).
const AUDIBLE_M: f32 = 18.75;

/// A death is "after a sprint" when the body sprinted in the last this long, ms; "after an audible sprint" within `AUDIBLE_WINDOW_MS`.
const AFTER_SPRINT_MS: u32 = 5_000;
const AUDIBLE_WINDOW_MS: u32 = 10_000;

/// The hit that kills is the one in the last this long before the death, ms.
const KILLER_MS: u32 = 1_000;

/// A teammate is near a body within this, metres (the first contact's company); the enemies within `CONTACT_ENEMIES_M`.
const COMPANY_M: f32 = 15.0;
const CONTACT_ENEMIES_M: f32 = 30.0;

/// What the bot's gate and bar were at the tick (read from the bot, or from a test's [`BotNow`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct PaceNow {
    pub(super) gate: Gate,
    pub(super) cause: Cause,
    pub(super) bar: f32,
    /// What the play handed (none: not read).
    pub(super) pace: Option<Pace>,
}

#[derive(Default)]
struct Team {
    /// Alive bot ticks.
    ticks: u64,
    walk: [u64; BANDS],
    sprint: [u64; BANDS],
    /// Walking ticks over 30 m from the nearest enemy and the sprinting ones, by the pace the play handed: Walk, Run, Cruise, Sneak.
    by_pace: [[u64; 2]; 5],
    /// The ticks of those the gate answered Run (the play handed Cruise): walking and sprinting.
    cruise_ran: [u64; 2],
    asked: u64,
    ran: u64,
    held: u64,
    ring: BTreeMap<&'static str, u64>,
    flips: u64,
    presses: u64,
    bursts: Vec<u32>,
    ended: [u64; 3],
    /// Calm sprint legs and the calm walking control: how many, and how many had the body dead 4.5-8 s later.
    legs: [u64; 2],
    legs_dead: [u64; 2],
    contacts: Vec<f32>,
    /// Sprinting ticks with an enemy within [`AUDIBLE_M`].
    audible_ticks: u64,
    /// Deaths; by a sprint in the last 5 s or not and the killer known to the victim's memory (0) or not (1); those with an audible sprint in the last 10 s;
    /// without a hit in the last second (a fall, a zone, a blast); the killer's distance after a sprint (under 20 m, 20-40, 40 and more).
    deaths: u64,
    deaths_sprint: [u64; 2],
    deaths_walk: [u64; 2],
    deaths_audible: u64,
    deaths_no_killer: u64,
    sprint_killer_m: [u64; 3],
    /// Batteries: the carriers' ticks, the nearest living teammate's distance summed over them, and those with a teammate within 15 m; the walking and the
    /// sprinting ticks of the escorts (the play "Escort").
    carrier_ticks: u64,
    carrier_mate_m: f64,
    carrier_mate15: u64,
    escort: [u64; 2],
    /// The first contacts (a hit given or taken after 5 s without one): the teammates within 15 m and the enemies within 30 m, summed.
    contact_mates: u64,
    contact_enemies: u64,
    /// By the play the body ran (the record's name): walking ticks 30 m or more from the nearest enemy, the sprinting ones, and the deaths.
    by_play: BTreeMap<String, [u64; 3]>,
}

#[derive(Default)]
struct Body {
    now: PaceNow,
    cause_before: Cause,
    gate: Gate,
    sprinting: bool,
    burst: u32,
    /// The legs and controls begun (tick, control) not yet 8 s old.
    pending: Vec<(u32, bool)>,
    last_hit: Option<u32>,
    /// The tick of its last sprint, and of its last audible one.
    last_sprint: Option<u32>,
    last_audible: Option<u32>,
    /// The last hit it took: the shooter and the tick.
    hit_by: Option<(Place, u32)>,
    /// Whom its memory holds a track of (as of the last read).
    known: Vec<Place>,
}

#[derive(Default)]
pub(super) struct PaceCounts {
    teams: [Team; 2],
    bodies: Vec<Body>,
}

impl PaceCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { teams: Default::default(), bodies: (0..n).map(|_| Body::default()).collect() }
    }
}

fn band_of(m: f32) -> usize {
    EDGES_M.iter().position(|&edge| m < edge).unwrap_or(BANDS - 1)
}

fn kind_of(gate: Gate) -> Option<bool> {
    match gate {
        Gate::Unasked => None,
        Gate::Run => Some(true),
        Gate::Walk(_) => Some(false),
    }
}

/// The name of what walked the bot.
fn why(walk: Walk) -> &'static str {
    match walk {
        Walk::Hold => "hold",
        Walk::Ring { at: Found::Feet, by: Danger::Track { radar: true, .. } } => "feet_radar",
        Walk::Ring { at: Found::Path, by: Danger::Track { radar: true, .. } } => "path_radar",
        Walk::Ring { at: Found::Feet, by: Danger::Track { age: Age::Fresh, .. } } => "feet_track_0_5s",
        Walk::Ring { at: Found::Feet, by: Danger::Track { age: Age::Mid, .. } } => "feet_track_5_10s",
        Walk::Ring { at: Found::Feet, by: Danger::Track { age: Age::Old, .. } } => "feet_track_10s_up",
        Walk::Ring { at: Found::Path, by: Danger::Track { age: Age::Fresh, .. } } => "path_track_0_5s",
        Walk::Ring { at: Found::Path, by: Danger::Track { age: Age::Mid, .. } } => "path_track_5_10s",
        Walk::Ring { at: Found::Path, by: Danger::Track { age: Age::Old, .. } } => "path_track_10s_up",
        Walk::Ring { at: Found::Goal, by: Danger::Track { radar: true, .. } } => "goal_radar",
        Walk::Ring { at: Found::Goal, by: Danger::Track { .. } } => "goal_track",
        Walk::Ring { at: Found::Goal, by: Danger::DeathSpot } => "goal_death_spot",
        Walk::Ring { at: Found::Feet, by: Danger::DeathSpot } => "feet_death_spot",
        Walk::Ring { at: Found::Path, by: Danger::DeathSpot } => "path_death_spot",
    }
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f32 * q).round() as usize]
}

impl Topic for PaceCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let (Some(bot), Some(body)) = (bot, self.bodies.get_mut(place)) {
            body.cause_before = body.now.cause;
            body.now = PaceNow { gate: bot.pace.last(), cause: bot.pace.cause(), bar: bot.stamina.value(), pace: bot.pace.pace() };
            body.known = bot.memory.tracks().iter().map(|t| t.who).collect();
        }
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        if let Some(body) = self.bodies.get_mut(place) {
            body.cause_before = body.now.cause;
            body.now = now.and_then(|n| n.pace).unwrap_or_default();
            body.known = now.map(|n| n.tracks.iter().map(|&(who, ..)| who).collect()).unwrap_or_default();
        }
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        let tick = game.tick;
        for &(_, shooter, target) in &game.hit_log {
            if let Some(body) = self.bodies.get_mut(target) {
                body.hit_by = Some((shooter, tick));
            }
            for place in [shooter, target] {
                let Some(body) = self.bodies.get_mut(place) else { continue };
                let quiet = body.last_hit.is_none_or(|at| tick.saturating_sub(at) >= QUIET_MS);
                body.last_hit = Some(tick);
                if quiet && body.now.bar > 0.0 {
                    let me = &game.bodies[place];
                    let team = &mut self.teams[usize::from(me.team & 1)];
                    team.contacts.push(body.now.bar);
                    for (other_place, other) in game.bodies.iter().enumerate() {
                        if other_place == place || !other.alive {
                            continue;
                        }
                        let d = other.feet.distance_xz(me.feet);
                        if other.team == me.team {
                            team.contact_mates += u64::from(d < COMPANY_M);
                        } else {
                            team.contact_enemies += u64::from(d < CONTACT_ENEMIES_M);
                        }
                    }
                }
            }
        }
    }

    fn body(&mut self, run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        let (game, place, tick, rec) = (at.game, at.place, at.tick, at.rec);
        let Some(body) = self.bodies.get_mut(place) else { return };
        let team = &mut self.teams[usize::from(game.bodies[place].team & 1)];
        let sprinting = rec.alive && Kinematics::sprints(rec.keys);
        // A death: the legs and controls begun 4.5-8 s ago are the guard's deaths; an open burst ends.
        if !rec.alive {
            if at.prev.alive {
                for &(began, control) in &body.pending {
                    let ago = tick.saturating_sub(began);
                    team.legs_dead[usize::from(control)] += u64::from((LEG_FROM_MS..=LEG_TO_MS).contains(&ago));
                }
                if body.burst > 0 {
                    team.bursts.push(body.burst);
                    team.ended[0] += 1;
                }
                team.deaths += 1;
                if let Some(rec) = run.recs[place].last() {
                    team.by_play.entry(run.plays.get(usize::from(rec.play)).cloned().unwrap_or_default()).or_default()[2] += 1;
                }
                let since = |last: Option<u32>, window: u32| last.is_some_and(|at| tick.saturating_sub(at) <= window);
                let sprinted = since(body.last_sprint, AFTER_SPRINT_MS);
                team.deaths_audible += u64::from(since(body.last_audible, AUDIBLE_WINDOW_MS));
                match body.hit_by.filter(|&(_, at)| tick.saturating_sub(at) <= KILLER_MS) {
                    None => team.deaths_no_killer += 1,
                    Some((killer, _)) => {
                        let unknown = usize::from(!body.known.contains(&killer));
                        if sprinted {
                            team.deaths_sprint[unknown] += 1;
                            let d = game.bodies[killer].feet.distance_xz(rec.feet);
                            team.sprint_killer_m[if d < 20.0 { 0 } else if d < 40.0 { 1 } else { 2 }] += 1;
                        } else {
                            team.deaths_walk[unknown] += 1;
                        }
                    }
                }
            }
            *body = Body { now: body.now, ..Body::default() };
            return;
        }
        team.ticks += 1;
        // The true nearest living enemy.
        let nearest = game
            .bodies
            .iter()
            .filter(|other| other.alive && other.team != game.bodies[place].team)
            .map(|other| other.feet.distance_xz(rec.feet))
            .fold(f32::INFINITY, f32::min);
        let band = band_of(nearest);
        let calm = nearest >= CALM_M;
        if sprinting {
            body.last_sprint = Some(tick);
            if nearest < AUDIBLE_M {
                body.last_audible = Some(tick);
                team.audible_ticks += 1;
            }
        }
        // The batteries' carrier and its escort.
        if rec.carrying {
            team.carrier_ticks += 1;
            let mate = game
                .bodies
                .iter()
                .enumerate()
                .filter(|&(other, b)| other != place && b.alive && b.team == game.bodies[place].team)
                .map(|(_, b)| b.feet.distance_xz(rec.feet))
                .fold(f32::INFINITY, f32::min);
            if mate.is_finite() {
                team.carrier_mate_m += f64::from(mate);
            }
            team.carrier_mate15 += u64::from(mate < COMPANY_M);
        }
        if rec.keys & WALK_KEYS != 0 && run.plays.get(usize::from(rec.play)).is_some_and(|p| p == "Escort") {
            team.escort[0] += 1;
            team.escort[1] += u64::from(sprinting);
        }
        let walking = !rec.carrying && rec.keys & WALK_KEYS != 0 && rec.keys & (key::AIM | key::FIRE) == 0;
        let gate = body.now.gate;
        if walking && calm {
            let entry = team.by_play.entry(run.plays.get(usize::from(rec.play)).cloned().unwrap_or_default()).or_default();
            entry[0] += 1;
            entry[1] += u64::from(sprinting);
        }
        if walking {
            team.walk[band] += 1;
            team.sprint[band] += u64::from(sprinting);
            if calm && let Some(pace) = body.now.pace {
                let kind = match pace {
                    Pace::Walk => 0,
                    Pace::Run => 1,
                    Pace::Cruise => 2,
                    Pace::Sneak => 3,
                    Pace::Arrive => 4,
                };
                team.by_pace[kind][0] += 1;
                team.by_pace[kind][1] += u64::from(sprinting);
                if gate == Gate::Run {
                    team.cruise_ran[0] += 1;
                    team.cruise_ran[1] += u64::from(sprinting);
                }
            }
        }
        // The gate.
        if let Some(runs) = kind_of(gate) {
            team.asked += 1;
            team.ran += u64::from(runs);
            match gate {
                Gate::Walk(Walk::Hold) => team.held += 1,
                Gate::Walk(walk) => *team.ring.entry(why(walk)).or_default() += 1,
                _ => {}
            }
            if kind_of(body.gate).is_some_and(|before| before != runs) {
                team.flips += 1;
            }
        }
        body.gate = gate;
        // The sprint key: presses, and the bursts with their ends.
        if sprinting && !body.sprinting {
            team.presses += 1;
            if calm {
                team.legs[0] += 1;
                body.pending.push((tick, false));
            }
        }
        if sprinting {
            body.burst += 1;
        } else if body.sprinting {
            team.bursts.push(body.burst);
            let cause = [body.now.cause, body.cause_before].into_iter().find(|&c| c != Cause::Map).unwrap_or(Cause::Map);
            team.ended[cause as usize] += 1;
            body.burst = 0;
        }
        body.sprinting = sprinting;
        // The control: a body-second of calm walking without the sprint.
        if ((tick.saturating_sub(run.first_tick) / TICK_MS) as usize).is_multiple_of(TPS) && calm && walking && !sprinting {
            team.legs[1] += 1;
            body.pending.push((tick, true));
        }
        body.pending.retain(|&(began, _)| tick.saturating_sub(began) <= LEG_TO_MS);
    }

    fn parts(&mut self, _run: &Run, _game: &Game, scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("pace", self.part(scale))]
    }
}

impl PaceCounts {
    fn part(&self, scale: &Scale) -> Part {
        let pct = |a: u64, b: u64| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
        let mut by_team = serde_json::Map::new();
        let mut lines = Vec::new();
        for (t, team) in self.teams.iter().enumerate() {
            let mut json = serde_json::Map::new();
            let mut put = |key: String, value: serde_json::Value| {
                json.insert(key, value);
            };
            let (mut walk, mut sprint) = (0, 0);
            for b in 0..BANDS {
                put(format!("calm_walk_{}", BAND_NAMES[b]), team.walk[b].into());
                put(format!("calm_sprint_{}", BAND_NAMES[b]), team.sprint[b].into());
                if b >= 2 {
                    walk += team.walk[b];
                    sprint += team.sprint[b];
                }
            }
            for (name, [w, s]) in ["walk", "run", "cruise", "sneak", "arrive"].into_iter().zip(team.by_pace) {
                put(format!("calm30_walking_{name}"), w.into());
                put(format!("calm30_sprinting_{name}"), s.into());
            }
            put("gate_ran_walking".into(), team.cruise_ran[0].into());
            put("gate_ran_sprinting".into(), team.cruise_ran[1].into());
            put("gate_asked".into(), team.asked.into());
            put("gate_ran".into(), team.ran.into());
            put("gate_held".into(), team.held.into());
            for (name, n) in &team.ring {
                put(format!("gate_walk_{name}"), (*n).into());
            }
            put("gate_flips".into(), team.flips.into());
            put("sprint_presses".into(), team.presses.into());
            put("sprint_presses_a_minute".into(), serde_json::json!(scale.per_min(team.presses as f64)));
            let mut lengths: Vec<f32> = team.bursts.iter().map(|&k| k as f32 * TICK_S).collect();
            lengths.sort_by(f32::total_cmp);
            put("bursts".into(), lengths.len().into());
            put("burst_median_s".into(), serde_json::json!(f64::from(percentile(&lengths, 0.5))));
            put("burst_p90_s".into(), serde_json::json!(f64::from(percentile(&lengths, 0.9))));
            for (name, n) in ["map", "gate", "reserve"].into_iter().zip(team.ended) {
                put(format!("burst_ended_{name}"), n.into());
            }
            put("legs".into(), team.legs[0].into());
            put("legs_dead_4_8s".into(), team.legs_dead[0].into());
            put("control".into(), team.legs[1].into());
            put("control_dead_4_8s".into(), team.legs_dead[1].into());
            let mut bars = team.contacts.clone();
            bars.sort_by(f32::total_cmp);
            for (play, [w, s, d]) in &team.by_play {
                put(format!("play_{play}_calm_walking"), (*w).into());
                put(format!("play_{play}_calm_sprinting"), (*s).into());
                put(format!("play_{play}_deaths"), (*d).into());
            }
            put("audible_sprint_ticks".into(), team.audible_ticks.into());
            put("deaths".into(), team.deaths.into());
            put("deaths_after_sprint_known_killer".into(), team.deaths_sprint[0].into());
            put("deaths_after_sprint_unknown_killer".into(), team.deaths_sprint[1].into());
            put("deaths_after_walk_known_killer".into(), team.deaths_walk[0].into());
            put("deaths_after_walk_unknown_killer".into(), team.deaths_walk[1].into());
            put("deaths_after_audible_sprint".into(), team.deaths_audible.into());
            put("deaths_no_killer".into(), team.deaths_no_killer.into());
            put("sprint_death_killer_0_20".into(), team.sprint_killer_m[0].into());
            put("sprint_death_killer_20_40".into(), team.sprint_killer_m[1].into());
            put("sprint_death_killer_40_up".into(), team.sprint_killer_m[2].into());
            put("carrier_ticks".into(), team.carrier_ticks.into());
            put("carrier_mate_m_sum".into(), serde_json::json!((team.carrier_mate_m * 10.0).round() / 10.0));
            put("carrier_mate_within_15".into(), team.carrier_mate15.into());
            put("escort_walking".into(), team.escort[0].into());
            put("escort_sprinting".into(), team.escort[1].into());
            put("contact_mates_within_15".into(), team.contact_mates.into());
            put("contact_enemies_within_30".into(), team.contact_enemies.into());
            put("contacts".into(), bars.len().into());
            put("bar_at_contact_median".into(), serde_json::json!(f64::from(percentile(&bars, 0.5))));
            put("bar_at_contact_p10".into(), serde_json::json!(f64::from(percentile(&bars, 0.1))));
            lines.push(format!(
                "team {t}: calm sprint over 30 m {:.0} % of {} walking ticks (by band {}); gate asked {} ran {} ({:.0} %), held {}, flips {}; sprint presses {:.1} a bot-minute, {} bursts (median {:.1} s, p90 {:.1} s; ended by the map {}, the gate {}, the reserve {}); \
                 calm legs {} dead 4.5-8 s later {} ({:.2} %), control {} dead {} ({:.2} %); bar at {} contacts median {:.0}",
                pct(sprint, walk),
                walk,
                (0..BANDS).map(|b| format!("{} {:.0} %", BAND_NAMES[b], pct(team.sprint[b], team.walk[b]))).collect::<Vec<_>>().join(", "),
                team.asked,
                team.ran,
                pct(team.ran, team.asked),
                team.held,
                team.flips,
                scale.per_min(team.presses as f64),
                lengths.len(),
                percentile(&lengths, 0.5),
                percentile(&lengths, 0.9),
                team.ended[0],
                team.ended[1],
                team.ended[2],
                team.legs[0],
                team.legs_dead[0],
                pct(team.legs_dead[0], team.legs[0]),
                team.legs[1],
                team.legs_dead[1],
                pct(team.legs_dead[1], team.legs[1]),
                bars.len(),
                percentile(&bars, 0.5),
            ));
            by_team.insert(t.to_string(), serde_json::Value::Object(json));
        }
        let json = serde_json::json!({ "pace": { "by_team": by_team } });
        Part::new(json, format!("owner-test pace: {}", lines.join("; ")))
    }
}

/// Place 0 (team 0) sprints for 2 s with the enemy 50 m away, walks, and sprints again; the gate answers `Run` and then walks for a track in the ring
/// of its path. The calm sprint is counted in the 45-60 m band, the presses and the bursts, the gate's reasons and its flips, and the leg of the second burst,
/// ended by the gate, has the body dead 5 s after it began.
#[test]
pub(crate) fn the_pace_topic_counts_the_calm_sprint_by_band_and_the_gate_and_the_deaths_after_a_leg() {
    use crate::v026g::bots::pace::Walk;
    let (mut game, mut tally) = made_up(&[0, 1]);
    let fwd = key::FORWARD;
    let sprint = key::FORWARD | key::SPRINT;
    let now = |gate: Gate, cause: Cause| Some(BotNow { play: "Roam".to_owned(), pace: Some(PaceNow { gate, cause, bar: 80.0, pace: Some(Pace::Cruise) }), ..BotNow::default() });
    play(&mut game, &mut tally, 300, |k, game| {
        put(game, 0, 5.0, 5.0, 0);
        put(game, 1, 55.0, 5.0, 0);
        // 0-1.9 s: sprint, answered Run. 2-3.9 s: walk for a track on the path. 4-5.9 s: sprint again; from 6 s it walks, and dies at 10 s.
        let (keys, gate, cause) = match k {
            0..40 => (sprint, Gate::Run, Cause::Map),
            40..80 => (fwd, Gate::Walk(Walk::Ring { at: Found::Path, by: Danger::Track { age: Age::Fresh, radar: false } }), Cause::Map),
            80..120 => (sprint, Gate::Run, Cause::Map),
            _ => (fwd, Gate::Walk(Walk::Hold), Cause::Ring),
        };
        game.bodies[0].keys = keys;
        if k >= 200 {
            game.bodies[0].alive = false;
        }
        vec![now(gate, cause), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    let team = &json["pace"]["by_team"]["0"];
    assert_eq!(team["calm_sprint_45_60"], 80, "two 2-s sprints at 20 ticks a second: {json}");
    assert_eq!(team["calm_walk_45_60"], 200, "{json}");
    assert_eq!(team["calm30_walking_cruise"], 200, "all of its walking was asked of the gate: {json}");
    assert_eq!(team["calm30_sprinting_cruise"], 80, "{json}");
    assert_eq!(team["gate_ran_walking"], 80, "the ticks the gate ran: the two sprints, no walking one: {json}");
    assert_eq!(team["gate_ran_sprinting"], 80, "{json}");
    assert_eq!(team["sprint_presses"], 2, "{json}");
    assert_eq!(team["bursts"], 2, "{json}");
    assert_eq!(team["burst_ended_gate"], 1, "the second burst was ended by the gate (the first by the map): {json}");
    assert_eq!(team["gate_walk_path_track_0_5s"], 40, "{json}");
    assert_eq!(team["gate_held"], 80, "{json}");
    assert_eq!(team["gate_flips"], 3, "run to walk, walk to run, run to walk: {json}");
    assert_eq!(team["legs"], 2, "{json}");
    assert_eq!(team["legs_dead_4_8s"], 1, "the leg that began at 4 s and the body dead at 10 s: {json}");
}

/// A death after a sprint with the killer in the victim's memory, 10 m away (an audible sprint), is counted as such: the hit that kills is the last
/// one in the second before the death; the first contact counts the company (a teammate within 15 m, an enemy within 30 m).
#[test]
pub(crate) fn a_death_after_an_audible_sprint_is_classed_by_the_killer_and_the_company_at_contact() {
    use crate::v026g::bots::know::Source;
    let (mut game, mut tally) = made_up(&[0, 0, 1]);
    let sprint = key::FORWARD | key::SPRINT;
    play(&mut game, &mut tally, 60, |k, game| {
        put(game, 0, 5.0, 5.0, 0);
        put(game, 1, 9.0, 5.0, 0);
        put(game, 2, 15.0, 5.0, 0);
        game.bodies[0].keys = if k < 40 { sprint } else { 0 };
        if k == 40 {
            game.hit_log.push(((k as u32 + 1) * TICK_MS, 2, 0));
        }
        if k >= 41 {
            game.bodies[0].alive = false;
        }
        let victim = BotNow { play: "Roam".to_owned(), pace: Some(PaceNow { bar: 90.0, ..PaceNow::default() }), tracks: vec![(2, Source::Seen, 0)], ..BotNow::default() };
        vec![Some(victim), bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    let team = &json["pace"]["by_team"]["0"];
    assert_eq!(team["deaths"], 1, "{json}");
    assert_eq!(team["deaths_after_sprint_known_killer"], 1, "{json}");
    assert_eq!(team["deaths_after_audible_sprint"], 1, "{json}");
    assert_eq!(team["sprint_death_killer_0_20"], 1, "{json}");
    assert_eq!(team["audible_sprint_ticks"], 40, "{json}");
    assert_eq!(team["contact_mates_within_15"], 1, "the teammate 4 m away: {json}");
    assert_eq!(team["contact_enemies_within_30"], 1, "{json}");
}
