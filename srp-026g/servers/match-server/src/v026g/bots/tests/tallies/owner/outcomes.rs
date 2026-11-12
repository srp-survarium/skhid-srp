//! The cross-report outcome counts of the 2026-10-28 analyses (`pvb-343-check/traces-20261028`), one topic: what the fixes
//! of the five reports are for, in the numbers those reports used, so that a fix is read on the bench against the reports'
//! targets. A package's own counters (how often its switch fires) are its own topic file; these are the outcomes.
//!
//! - **Holders** (an-anticamper §2.5, §8.2): a shooter's hold is a run of kills from one place (within [`HOLD_M`] of the first
//!   kill, [`HOLD_APART_MS`] apart, one life). Of the runs that reached two kills, the share that made a third (live humans
//!   25 %, the owner against the bots 45 %); of the holders killed after two, the share killed by a bot that came from a flank,
//!   more than [`FLANK_DEG`] off every victim's bearing (32 % / 9 %); the share of kills that follow another kill of the same
//!   shooter within 3 s (live 3.5-3.9 %, the owner 18 %); lane entries into held lines per 10 s of 2+ kill holds (live
//!   0.24-0.60, bots against the owner 0.84-1.42). The hold is the camper topic's, one definition (§2.5's 10 m, 120 s, 60 degrees,
//!   from the live holds and the owner's the report counted); the camper topic counts a holder killed after one kill, these after two.
//! - **Fights** (an-fights §5): bot-seconds a carrier is in an enemy's clear sight within 40 m and is not that enemy's target;
//!   enemies hit to pain under 30 % that live 15 s more while a bot of the other team was within 30 m (any bot of the other
//!   team, not only one that knew them: the bench has no per-bot knowledge of a body here); the time a battery dropped by a
//!   carrier takes to be taken again, by whether a hurt enemy of the dropper was within 10 m of it.
//! - **Objectives** (an-objective §2.5): battery pickups from a marked zone floor and the carriers that die within 10 s of one;
//!   plays ended "no way to its goal" (Escort, Collect).
//! - **Gear** (an-gear-moves §3.2): traps placed, and caught enemies, mates and the placer himself, by the placer's stance.
//! - **Sight** (an-sight §8.1): of the moments a body was truly visible, the share the bots' picture says hidden (live 22 %);
//!   of the moments the picture says visible, the share truly hidden (live 20 %). Read with `SRP_BENCH_VIEW_ERROR`.
//!
//! **By team.** Every key is also given for each team in `by_team`, filed under **the team whose decision the number measures**,
//! so that a head-to-head run (one team ON, the other OFF) reads the ON team's number as the ON team's:
//!
//! | keys | filed under |
//! |---|---|
//! | `hold_runs_*`, `hold_3rd_share`, `kills`, `quick_second_*` | the team the shooter fought (the victims': how well it stops a holder) |
//! | `holders_killed_*`, `holders_flank_share` | the killer's team (it chose the flank) |
//! | `carrier_clear_*` | the enemy's team (it did not target the carrier in its sight) |
//! | `hurt_enemies`, `hurt_alive_15s*` | the team of the near bot (it did not finish the hurt body) |
//! | `retake_ms_*` | the team that took the battery again |
//! | `zone_takes`, `zone_take_died_10s`, `*_no_way_ends` | the carrier's / the bot's own team |
//! | `trap_*_by_stance` | the placer's team |
//! | `view_flip.*` | the looking bot's team |
//!
//! Where a count cannot be read as the report did, the text says how it differs.

use super::*;
use crate::v026g::bots::tests::standin_damage::Pools;
use crate::v026g::bots::tests::standin_traps::Event as TrapEvent;
use crate::v026g::bots::tests::{Kill as KillKind, Lying};
use crate::v026g::bots::view::still_eye;

/// A hold is the camper topic's (`campers.rs`): kills from one place within [`SPOT_HOLD_M`] of the first, each within
/// [`RUN_APART_MS`] of the last; a holder's killer is from a flank when [`OFF_LANE_DEG`] or more off every victim's bearing.
/// One definition, an-anticamper §2.5 (10 m, 120 s, 60 degrees: the live holds and the owner's the report counted).
const HOLD_M: f32 = SPOT_HOLD_M;
const HOLD_APART_MS: u32 = RUN_APART_MS;
const FLANK_DEG: f32 = OFF_LANE_DEG;
/// Two kills of one shooter this close are a quick second kill, ms (an-anticamper §8.2).
const QUICK_MS: u32 = 3_000;
/// An escort play that ended after less than this, ms, is a short end (the churn of A157: median 875 ms).
const ESCORT_SHORT_MS: u32 = 1_500;
/// A carrier counts as in sight within this of an enemy, m.
const CARRIER_SIGHT_M: f32 = 40.0;
/// The carrier and sight samples are taken every this many ticks.
const SAMPLE_TICKS: u32 = 5;
/// Pain under this share of the full pool is hurt (an-fights §5b: pain < 0.3), a bot of the other team within
/// [`HURT_NEAR_M`] when it began, and the body alive [`HURT_LIVES_MS`] later.
const HURT_PAIN: f32 = 0.3;
const HURT_NEAR_M: f32 = 30.0;
const HURT_LIVES_MS: u32 = 15_000;
/// A battery dropped within this of a hurt enemy, m.
const RETAKE_NEAR_M: f32 = 10.0;
/// A carrier who dies within this of a zone pickup died of it (nearly), ms.
const ZONE_DEATH_MS: u32 = 10_000;
/// Pairs of bots and enemies sampled for the sight flips a second: each bot's nearest enemies, within this, m.
const FLIP_NEAREST: usize = 2;
const FLIP_M: f32 = 80.0;

/// A shooter's hold under way: where it began, whose kills they were (the victims' team), when the last was, and the victims'
/// bearings from the spot.
#[derive(Clone, Debug)]
struct Hold {
    /// The team the victims were on: the team the hold is filed under.
    against: u8,
    spot: Vec3,
    last: u32,
    kills: u32,
    bearings: Vec<f32>,
}

/// A body's hurt spell, for the share of hurt enemies that live on (an-fights §5b).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum HurtSpell {
    /// Not hurt (pain over the line), or dead.
    #[default]
    Calm,
    /// Hurt since this tick, in the life that spawned then, with a bot of the other team near (the team to file it under) or none.
    Watching { since: u32, spawned: u32, near: Option<u8> },
    /// Still hurt, its spell counted and its window over.
    Spent,
}

/// A battery's state for the retake time (an-fights §5c).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BatteryNow {
    /// Home, or lying where it was not dropped by a carrier.
    #[default]
    Resting,
    /// In this place's hands.
    Carried(Place),
    /// Dropped by a carrier of this team at this tick, with a hurt enemy of the dropper within [`RETAKE_NEAR_M`] or none.
    Dropped { team: u8, at: u32, hurt_near: bool },
}

/// A placer's stance at his last placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stance {
    Standing,
    Crouched,
}

impl Stance {
    fn of(crouched: bool) -> Self {
        if crouched { Self::Crouched } else { Self::Standing }
    }

    fn index(self) -> usize {
        usize::from(self == Self::Crouched)
    }
}

/// The sight flip counts (an-sight §8.1): `[truly visible, of them the picture says hidden, the picture says visible, of them
/// truly hidden]`.
type Flips = [u32; 4];

/// The numbers one team earned (the team a count is of is said in the module docs); the topic keeps one per team, so a
/// one-sided run (`SRP_BENCH_ONE_SIDE`) is read by side, and the log's `by_team` object has the same keys per team.
#[derive(Clone, Copy, Default)]
struct Counts {
    /// Holds that reached two kills, and of them three.
    hold_runs: [u32; 2],
    /// Holders killed with two kills made, and of them by a flank.
    holders_killed: [u32; 2],
    kills: u32,
    quick_second: u32,
    carrier_clear_ms: u32,
    carrier_clear_off_ms: u32,
    hurt_events: u32,
    hurt_lived: u32,
    /// Retake times by [no hurt enemy near the drop, one near]: the sum, ms, and the number.
    retake: [(u64, u32); 2],
    zone_takes: u32,
    zone_take_died: u32,
    escort_no_way: u32,
    collect_no_way: u32,
    /// Escort plays that ended (any reason), the sum of their lives, ms, and those that lived under [`ESCORT_SHORT_MS`].
    escort_ended: u32,
    escort_life_ms: u64,
    escort_short: u32,
    /// By stance [standing, crouched]: placed, and caught [enemy, mate, own].
    placed: [u32; 2],
    caught: [[u32; 3]; 2],
    flips: Flips,
    /// Moments sampled (a second each), and the pairs of bot and enemy in them.
    flip_moments: u32,
    flip_pairs: u32,
}

impl Counts {
    fn plus(mut self, o: &Counts) -> Counts {
        let add2 = |a: &mut [u32; 2], b: &[u32; 2]| a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
        add2(&mut self.hold_runs, &o.hold_runs);
        add2(&mut self.holders_killed, &o.holders_killed);
        add2(&mut self.placed, &o.placed);
        for (row, other) in self.caught.iter_mut().zip(&o.caught) {
            row.iter_mut().zip(other).for_each(|(x, y)| *x += y);
        }
        self.flips.iter_mut().zip(&o.flips).for_each(|(x, y)| *x += y);
        for (r, q) in self.retake.iter_mut().zip(&o.retake) {
            r.0 += q.0;
            r.1 += q.1;
        }
        self.kills += o.kills;
        self.quick_second += o.quick_second;
        self.carrier_clear_ms += o.carrier_clear_ms;
        self.carrier_clear_off_ms += o.carrier_clear_off_ms;
        self.hurt_events += o.hurt_events;
        self.hurt_lived += o.hurt_lived;
        self.zone_takes += o.zone_takes;
        self.zone_take_died += o.zone_take_died;
        self.escort_no_way += o.escort_no_way;
        self.collect_no_way += o.collect_no_way;
        self.escort_ended += o.escort_ended;
        self.escort_life_ms += o.escort_life_ms;
        self.escort_short += o.escort_short;
        self.flip_moments += o.flip_moments;
        self.flip_pairs += o.flip_pairs;
        self
    }

    /// The counts as the log's keys (shares as the reports read them).
    fn json(&self) -> serde_json::Value {
        let rate = Scale::rate;
        let retake = |k: usize| {
            let (sum, n) = self.retake[k];
            if n > 0 { (sum / u64::from(n)) as u32 } else { 0 }
        };
        let [vis, vis_hidden, pic, pic_blocked] = self.flips;
        serde_json::json!({
            "hold_runs_2": self.hold_runs[0],
            "hold_runs_3": self.hold_runs[1],
            "hold_3rd_share": rate(self.hold_runs[1], self.hold_runs[0]),
            "holders_killed_after_2": self.holders_killed[0],
            "holders_killed_flank": self.holders_killed[1],
            "holders_flank_share": rate(self.holders_killed[1], self.holders_killed[0]),
            "kills": self.kills,
            "quick_second_kills": self.quick_second,
            "quick_second_share": rate(self.quick_second, self.kills),
            "carrier_clear_s": f64::from(self.carrier_clear_ms) / 1000.0,
            "carrier_clear_not_targeted_s": f64::from(self.carrier_clear_off_ms) / 1000.0,
            "hurt_enemies": self.hurt_events,
            "hurt_alive_15s": self.hurt_lived,
            "hurt_alive_15s_share": rate(self.hurt_lived, self.hurt_events),
            "retake_ms_no_hurt_near": [retake(0), self.retake[0].1],
            "retake_ms_hurt_near": [retake(1), self.retake[1].1],
            "zone_takes": self.zone_takes,
            "zone_take_died_10s": self.zone_take_died,
            "escort_no_way_ends": self.escort_no_way,
            "collect_no_way_ends": self.collect_no_way,
            "escort_plays_ended": self.escort_ended,
            "escort_life_s": self.escort_life_ms as f64 / 1000.0,
            "escort_short_ends": self.escort_short,
            "trap_placed_by_stance": self.placed,
            "trap_caught_by_stance": self.caught,
            "view_flip": {
                "samples": self.flip_moments,
                "pairs": self.flip_pairs,
                "truly_visible": vis,
                "picture_hidden": vis_hidden,
                "missed_sight_share": rate(vis_hidden, vis),
                "picture_visible": pic,
                "truly_hidden": pic_blocked,
                "fire_into_cover_share": rate(pic_blocked, pic),
            },
        })
    }
}

/// A team's index in the per-team counts (the game has two).
fn side(team: u8) -> usize {
    usize::from(team.min(1))
}

#[derive(Default)]
pub(super) struct OutcomeCounts {
    /// Team 0's and team 1's (see the module docs for what each is filed under).
    by: [Counts; 2],
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    holds: BTreeMap<Place, Hold>,
    last_by: BTreeMap<Place, u32>,
    /// By body: its hurt spell.
    hurt: Vec<HurtSpell>,
    /// By battery.
    batteries: Vec<BatteryNow>,
    zone_pickups: Vec<ZonePick>,
    traps_seen: usize,
    /// By place: the stance at the last placement.
    stance: BTreeMap<Place, Stance>,
    estimates: Vec<Option<Vec3>>,
    /// By place: the tick its running Escort play began.
    escort_started: BTreeMap<Place, u32>,
}

/// A pickup from a zone floor, followed until the carrier dies or [`ZONE_DEATH_MS`] pass: where, and what escorts ran on him.
#[derive(Clone, Copy)]
struct ZonePick {
    place: Place,
    t: u32,
    at: Vec3,
    /// The most bots escorting him at once, and the age of the pickup when the first began.
    escorts_max: u32,
    first_escort_ms: Option<u32>,
}

impl OutcomeCounts {
    /// Both teams' counts together.
    #[cfg(test)]
    fn all(&self) -> Counts {
        self.by[0].plus(&self.by[1])
    }

    /// The kills the game logged since the last tick: holds, quick second kills, holders killed.
    fn kills_tick(&mut self, game: &Game) {
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new: Vec<_> = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        for (t, shooter, victim) in new {
            let (Some(s), Some(v)) = (game.bodies.get(shooter), game.bodies.get(victim)) else {
                continue;
            };
            if s.team == v.team {
                continue;
            }
            // A holder with two kills made, killed: while the hold lasts (the last kill within [`HOLD_APART_MS`]) and he is near its
            // spot. A hold that has run out is over, not a hold he died in.
            if let Some(hold) = self.holds.remove(&victim) {
                let alive_hold = t <= hold.last + HOLD_APART_MS && v.feet.distance_xz(hold.spot) <= HOLD_M;
                if alive_hold && hold.kills >= 2 {
                    let c = &mut self.by[side(s.team)];
                    c.holders_killed[0] += 1;
                    let from = yaw_of([s.feet.x - hold.spot.x, s.feet.z - hold.spot.z]);
                    let flank = hold.bearings.iter().all(|&w| wrap(w - from).abs().to_degrees() > FLANK_DEG);
                    c.holders_killed[1] += u32::from(flank);
                }
                self.close(&hold);
            }
            self.by[side(v.team)].kills += 1;
            if self.last_by.insert(shooter, t).is_some_and(|before| t.saturating_sub(before) <= QUICK_MS) {
                self.by[side(v.team)].quick_second += 1;
            }
            let bearing = yaw_of([v.feet.x - s.feet.x, v.feet.z - s.feet.z]);
            match self.holds.get_mut(&shooter) {
                Some(hold) if hold.spot.distance_xz(s.feet) <= HOLD_M && t <= hold.last + HOLD_APART_MS => {
                    hold.last = t;
                    hold.kills += 1;
                    hold.bearings.push(bearing);
                }
                _ => {
                    if let Some(old) = self.holds.remove(&shooter) {
                        self.close(&old);
                    }
                    self.holds.insert(shooter, Hold { against: v.team, spot: s.feet, last: t, kills: 1, bearings: vec![bearing] });
                }
            }
        }
    }

    /// A hold is over: the runs that reached two and three kills.
    fn close(&mut self, hold: &Hold) {
        let c = &mut self.by[side(hold.against)];
        c.hold_runs[0] += u32::from(hold.kills >= 2);
        c.hold_runs[1] += u32::from(hold.kills >= 3);
    }

    /// Every [`SAMPLE_TICKS`] ticks: carriers in clear sight and not targeted.
    fn carriers(&mut self, game: &Game, sight: &mapdata::RayTree, rays: &mut u64) {
        let ms = SAMPLE_TICKS * TICK_MS;
        for (place, carrier) in game.bodies.iter().enumerate() {
            if !carrier.alive || carrier.carrying.is_none() {
                continue;
            }
            let chest = Game::chest(carrier);
            for enemy in &game.bodies {
                if !enemy.alive || enemy.team == carrier.team || enemy.feet.distance_xz(carrier.feet) > CARRIER_SIGHT_M {
                    continue;
                }
                *rays += 1;
                if sight.visible(still_eye(enemy.feet, enemy.keys & key::CROUCH != 0), chest) {
                    // The enemy's decision: it saw the carrier and did not take him for its target.
                    let c = &mut self.by[side(enemy.team)];
                    c.carrier_clear_ms += ms;
                    c.carrier_clear_off_ms += ms * u32::from(enemy.target != Some(place));
                }
            }
        }
    }

    /// Bodies hurt to pain under [`HURT_PAIN`] and alive [`HURT_LIVES_MS`] later, filed under the team of the bot that was near.
    fn hurt_tick(&mut self, game: &Game) {
        let full = Pools::default().pain;
        self.hurt.resize(game.bodies.len(), HurtSpell::Calm);
        for (place, body) in game.bodies.iter().enumerate() {
            let hurt_now = body.alive && body.pools.pain < HURT_PAIN * full;
            self.hurt[place] = match self.hurt[place] {
                HurtSpell::Calm if hurt_now => {
                    // Any bot of the other team within the reach: its team is the one that is to finish him.
                    let near = game
                        .bodies
                        .iter()
                        .find(|o| o.alive && o.team != body.team && o.feet.distance_xz(body.feet) <= HURT_NEAR_M)
                        .map(|o| o.team);
                    if let Some(team) = near {
                        self.by[side(team)].hurt_events += 1;
                    }
                    HurtSpell::Watching { since: game.tick, spawned: body.spawned, near }
                }
                HurtSpell::Watching { spawned, .. } if body.spawned != spawned || !body.alive => {
                    // Died (or was born again): not alive 15 s later.
                    HurtSpell::Calm
                }
                HurtSpell::Watching { since, near, .. } if game.tick >= since + HURT_LIVES_MS => {
                    if let Some(team) = near {
                        self.by[side(team)].hurt_lived += 1;
                    }
                    if hurt_now { HurtSpell::Spent } else { HurtSpell::Calm }
                }
                HurtSpell::Spent if !hurt_now => HurtSpell::Calm,
                other => other,
            };
        }
    }

    /// The batteries: a drop (carried, then lying) and its retake; a pickup from a zone floor (`in_zone`: the feet stand on one).
    fn batteries_tick(&mut self, game: &Game, in_zone: impl Fn(Vec3) -> bool) {
        self.batteries.resize(game.batteries.len(), BatteryNow::Resting);
        let full = Pools::default().pain;
        for (i, lying) in game.batteries.iter().enumerate() {
            self.batteries[i] = match (*lying, self.batteries[i]) {
                (Lying::Free(at, _), BatteryNow::Carried(by)) => {
                    let team = game.bodies.get(by).map_or(0, |b| b.team);
                    let hurt_near = game.bodies.iter().any(|b| {
                        b.alive
                            && b.team != team
                            && b.pools.pain < HURT_PAIN * full
                            && b.feet.distance_xz(at) <= RETAKE_NEAR_M
                            && b.carrying.is_none()
                    });
                    BatteryNow::Dropped { team, at: game.tick, hurt_near }
                }
                (Lying::Free(..), state) => state,
                (Lying::Carried(by), before) => {
                    if before != BatteryNow::Carried(by) {
                        if let (BatteryNow::Dropped { at, hurt_near, .. }, Some(taker)) = (before, game.bodies.get(by)) {
                            // Filed under the team that takes it again.
                            let slot = &mut self.by[side(taker.team)].retake[usize::from(hurt_near)];
                            slot.0 += u64::from(game.tick.saturating_sub(at));
                            slot.1 += 1;
                        }
                        if let Some(body) = game.bodies.get(by)
                            && !matches!(before, BatteryNow::Carried(_))
                            && in_zone(body.feet)
                        {
                            self.by[side(body.team)].zone_takes += 1;
                            self.zone_pickups.push(ZonePick { place: by, t: game.tick, at: body.feet, escorts_max: 0, first_escort_ms: None });
                        }
                    }
                    BatteryNow::Carried(by)
                }
                // Home: delivered or put back, not taken again.
                (Lying::Home(_), _) => BatteryNow::Resting,
            };
        }
    }

    /// The bots escorting each carrier that took a battery from a zone, now.
    fn escorts_on_zone_takes(&mut self, bots: &Bots, game: &Game) {
        use crate::v026g::bots::plays::{PlayKind, TargetRef};
        if self.zone_pickups.is_empty() {
            return;
        }
        let mut count: BTreeMap<Place, u32> = BTreeMap::new();
        for bot in bots.bots.iter().flatten() {
            let Some(a) = bot.assignment.as_ref().filter(|a| a.kind == PlayKind::Escort) else {
                continue;
            };
            let carrier = match a.target {
                TargetRef::Place(p) => Some(p),
                TargetRef::Battery(b) => match game.batteries.get(b) {
                    Some(Lying::Carried(by)) => Some(*by),
                    _ => None,
                },
                _ => None,
            };
            if let Some(c) = carrier {
                *count.entry(c).or_default() += 1;
            }
        }
        for pick in &mut self.zone_pickups {
            if let Some(&n) = count.get(&pick.place) {
                pick.escorts_max = pick.escorts_max.max(n);
                pick.first_escort_ms.get_or_insert(game.tick - pick.t);
            }
        }
    }

    /// Carriers that took a battery from a zone and died soon after.
    fn zone_deaths(&mut self, game: &Game) {
        let tick = game.tick;
        let by = &mut self.by;
        self.zone_pickups.retain(|pick| {
            let (place, t) = (pick.place, pick.t);
            let died = game.bodies.get(place).is_some_and(|b| !b.alive);
            if died {
                by[side(game.bodies[place].team)].zone_take_died += 1;
                // The bench's trace of a take that died (A157): who he was, what escorted him, who killed him from where.
                let team = game.bodies[place].team;
                let killer = game.kill_log.iter().rev().find(|k| k.2 == place && k.0 + 1_000 >= t).map(|k| k.1);
                eprintln!(
                    "ZTAKE team={team} place={place} age_ms={} at=({:.1},{:.1}) died_at=({:.1},{:.1}) escorts_max={} first_escort_ms={:?} refused={} killer={} killer_team={} killer_at={} killer_dist={}",
                    tick - t,
                    pick.at.x,
                    pick.at.z,
                    game.bodies[place].feet.x,
                    game.bodies[place].feet.z,
                    pick.escorts_max,
                    pick.first_escort_ms,
                    crate::v026g::bots::hazards::damage_zone_fixes::refused_for(team, place as u32, t, tick),
                    killer.map_or("none".to_owned(), |k| k.to_string()),
                    killer.map_or(-1, |k| i32::from(game.bodies[k].team)),
                    killer.map_or("-".to_owned(), |k| format!("({:.1},{:.1})", game.bodies[k].feet.x, game.bodies[k].feet.z)),
                    killer.map_or(-1.0, |k| game.bodies[k].feet.distance_xz(game.bodies[place].feet)),
                );
            }
            !died && tick < t + ZONE_DEATH_MS
        });
    }

    /// The stand-in's traps this tick: placements by the placer's stance, catches by his stance at the last placement.
    fn traps(&mut self, game: &Game) {
        let events = &game.traps.events;
        for event in &events[self.traps_seen.min(events.len())..] {
            match *event {
                TrapEvent::Placed { place, .. } => {
                    let stance = Stance::of(game.bodies.get(place).is_some_and(|b| b.keys & key::CROUCH != 0));
                    self.stance.insert(place, stance);
                    let team = game.bodies.get(place).map_or(0, |b| b.team);
                    self.by[side(team)].placed[stance.index()] += 1;
                }
                TrapEvent::Caught { placer, kind, .. } => {
                    let stance = self.stance.get(&placer).copied().unwrap_or(Stance::Standing);
                    let k = match kind {
                        KillKind::Enemy => 0,
                        KillKind::Mate => 1,
                        KillKind::Own => 2,
                    };
                    let team = game.bodies.get(placer).map_or(0, |b| b.team);
                    self.by[side(team)].caught[stance.index()][k] += 1;
                }
                _ => {}
            }
        }
        self.traps_seen = events.len();
    }

    /// Once a second: for each living bot its nearest enemies, the line from its eye to the enemy's chest as it is (the true
    /// bodies) and as the bots' picture has it (the reckoned bodies). Counts the pairs, not only the moments.
    fn flip_sample(&mut self, game: &Game) {
        let sight = &game.world.sight;
        let picture = |place: Place| self.estimates.get(place).copied().flatten();
        let mut flips = [Flips::default(); 2];
        let mut pairs = [0u32; 2];
        for (me, body) in game.bodies.iter().enumerate() {
            let Some(my_picture) = picture(me).filter(|_| body.alive) else {
                continue;
            };
            let mut near: Vec<(f32, Place)> = game
                .bodies
                .iter()
                .enumerate()
                .filter(|(_, o)| o.alive && o.team != body.team)
                .map(|(p, o)| (o.feet.distance_xz(body.feet), p))
                .filter(|(d, _)| *d <= FLIP_M)
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            for &(_, other) in near.iter().take(FLIP_NEAREST) {
                let (o, Some(their_picture)) = (&game.bodies[other], picture(other)) else {
                    continue;
                };
                let crouch = body.keys & key::CROUCH != 0;
                let truth = sight.visible(still_eye(body.feet, crouch), Game::chest(o));
                let seen = sight.visible(
                    still_eye(my_picture, crouch),
                    their_picture + (Game::chest(o) - o.feet),
                );
                let f = &mut flips[side(body.team)];
                f[0] += u32::from(truth);
                f[1] += u32::from(truth && !seen);
                f[2] += u32::from(seen);
                f[3] += u32::from(seen && !truth);
                pairs[side(body.team)] += 1;
            }
        }
        for ((c, f), p) in self.by.iter_mut().zip(flips).zip(pairs) {
            c.flips.iter_mut().zip(f).for_each(|(x, y)| *x += y);
            c.flip_pairs += p;
            c.flip_moments += 1;
        }
    }
}

impl Topic for OutcomeCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The bots' picture of every body and the hazards, once a tick before the readings: the batteries and the sight flips.
    fn bots(&mut self, bots: &Bots, game: &Game) {
        self.estimates = (0..game.bodies.len())
            .map(|p| bots.tracker.body(p).and_then(|body| body.estimate()).map(|e| e.feet))
            .collect();
        self.batteries_tick(game, |feet| {
            bots.tactics.as_ref().is_some_and(|t| {
                game.world.grid.locate(feet, 1.0).is_some_and(|node| t.hazards.zones_at(node).next().is_some())
            })
        });
        self.escorts_on_zone_takes(bots, game);
        self.zone_deaths(game);
        if (game.tick / TICK_MS).is_multiple_of(1000 / TICK_MS) {
            self.flip_sample(game);
        }
    }

    fn note(&mut self, _run: &Run, note: &Note, _kind: &NoteKind<'_>, game: &Game) {
        let team = side(game.bodies.get(note.place).map_or(0, |b| b.team));
        if note.text.starts_with("takes ") {
            if note.text.starts_with("takes Escort ") {
                self.escort_started.insert(note.place, game.tick);
            } else {
                self.escort_started.remove(&note.place);
            }
        }
        if note.text.starts_with("ends Escort:")
            && let Some(began) = self.escort_started.remove(&note.place)
        {
            let life = game.tick.saturating_sub(began);
            self.by[team].escort_ended += 1;
            self.by[team].escort_life_ms += u64::from(life);
            self.by[team].escort_short += u32::from(life < ESCORT_SHORT_MS);
        }
        if note.text.starts_with("ends Escort:") && note.text.contains("no way") {
            self.by[team].escort_no_way += 1;
        }
        if note.text.starts_with("ends Collect:") && note.text.contains("no way") {
            self.by[team].collect_no_way += 1;
        }
    }

    fn end(&mut self, _run: &Run, game: &mut Game, sight: &mapdata::RayTree, rays: &mut u64) {
        self.kills_tick(game);
        self.hurt_tick(game);
        self.traps(game);
        if (game.tick / TICK_MS).is_multiple_of(SAMPLE_TICKS) {
            self.carriers(game, sight, rays);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, shared: &Shared) -> Vec<(&'static str, Part)> {
        // The holds still under way count as they stand.
        let mut by = self.by;
        for hold in self.holds.values() {
            let c = &mut by[side(hold.against)];
            c.hold_runs[0] += u32::from(hold.kills >= 2);
            c.hold_runs[1] += u32::from(hold.kills >= 3);
        }
        let all = by[0].plus(&by[1]);
        let num = |name: &str| shared.get(name).and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        // Lane entries (the camper topic's cone) per 10 s of holds of two kills or more, as the live figure is.
        let (entries, held_s) = (num("camper_lane_entries"), num("camper_held2_s"));
        let per_10s = if held_s > 0.0 { (1000.0 * 10.0 * entries / held_s).round() / 1000.0 } else { 0.0 };
        let rate = Scale::rate;
        let retake = |k: usize| {
            let (sum, n) = all.retake[k];
            if n > 0 { (sum / u64::from(n)) as u32 } else { 0 }
        };
        let [vis, vis_hidden, pic, pic_blocked] = all.flips;
        let mut json = all.json();
        // Lane entries are the camper topic's, not split by team; the rest of the keys are per team too.
        json["lane_entries_per_10s"] = serde_json::json!(per_10s);
        json["by_team"] = serde_json::json!({ "0": by[0].json(), "1": by[1].json() });
        let team_line = |t: usize| {
            let c = &by[t];
            format!(
                "team {t}: kills against it {}, holds against it 2+/3+ {}/{}, holders it killed {} (flank {}), quick second against it {}, carrier in its sight not targeted {:.0} s, hurt with it near {} (alive 15 s {}), zone takes {} (died {}), no-way ends {}+{}, view flips {}/{} and {}/{}",
                c.kills,
                c.hold_runs[0],
                c.hold_runs[1],
                c.holders_killed[0],
                c.holders_killed[1],
                c.quick_second,
                f64::from(c.carrier_clear_off_ms) / 1000.0,
                c.hurt_events,
                c.hurt_lived,
                c.zone_takes,
                c.zone_take_died,
                c.escort_no_way,
                c.collect_no_way,
                c.flips[1],
                c.flips[0],
                c.flips[3],
                c.flips[2],
            )
        };
        let text = format!(
            "owner-test outcomes: holders (kills from one place within {HOLD_M} m): {} runs of 2+ kills, {} of 3+ = {:.1} % (live 25 %, the owner 45 %); holders killed after two {}, by a flank {} = {:.1} % (live 32 %, the owner 9 %); quick second kills (within 3 s of the shooter's last) {} of {} = {:.1} % (live 3.5-3.9 %, the owner 18 %); lane entries a 10 s of 2+ kill holds {per_10s} (live 0.24-0.60, bots against the owner 0.84-1.42; the campers' cone of 15 degrees, not the report's 20); fights: carriers in clear sight within 40 m {:.0} s, not targeted {:.0} s; hurt (pain < 30 %, an enemy within 30 m) {}, alive 15 s later {}; battery retake mean ms (no hurt enemy within 10 m of the drop: {} of {}, one: {} of {}); objectives: pickups from a zone floor {}, died within 10 s {}, Escort 'no way' ends {}, Collect 'no way' ends {}; traps by the placer's stance [standing, crouched]: placed {:?}, caught [enemy, mate, own] {:?}; sight: truly visible {vis}, the picture says hidden {vis_hidden} = {:.1} % (live 22 %); the picture says visible {pic}, truly hidden {pic_blocked} = {:.1} % (live 20 %) (the picture is the bots' reckoned bodies, a pair a second, the 2 nearest enemies: {} pairs); by team (filed under the team whose decision it measures): [{}] [{}]",
            all.hold_runs[0],
            all.hold_runs[1],
            100.0 * rate(all.hold_runs[1], all.hold_runs[0]),
            all.holders_killed[0],
            all.holders_killed[1],
            100.0 * rate(all.holders_killed[1], all.holders_killed[0]),
            all.quick_second,
            all.kills,
            100.0 * rate(all.quick_second, all.kills),
            f64::from(all.carrier_clear_ms) / 1000.0,
            f64::from(all.carrier_clear_off_ms) / 1000.0,
            all.hurt_events,
            all.hurt_lived,
            retake(0),
            all.retake[0].1,
            retake(1),
            all.retake[1].1,
            all.zone_takes,
            all.zone_take_died,
            all.escort_no_way,
            all.collect_no_way,
            all.placed,
            all.caught,
            100.0 * rate(vis_hidden, vis),
            100.0 * rate(pic_blocked, pic),
            all.flip_pairs,
            team_line(0),
            team_line(1),
        );
        vec![("outcomes", Part::new(json, text))]
    }
}

/// A holder who kills three bots a second apart and is killed from the side: a run of three, a holder killed after two by
/// a flank, and two quick second kills.
#[test]
pub(crate) fn a_hold_of_three_a_flank_killer_and_quick_second_kills_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 1, 0, 0, 0]);
    kills_and_notes(
        &mut game,
        &mut tally,
        140,
        &[(20.0, 35.0), (30.0, 30.0), (21.0, 35.0), (22.0, 35.0), (45.0, 30.0)],
        &[(20, 1, 0), (40, 1, 2), (60, 1, 3), (100, 4, 1)],
        &[],
    );
    let counts = tally.topic::<OutcomeCounts>();
    assert_eq!(counts.all().hold_runs, [1, 1]);
    assert_eq!(counts.all().holders_killed, [1, 1]);
    assert_eq!((counts.all().kills, counts.all().quick_second), (4, 2));
    let (_, json) = tally.finish(&game);
    assert_eq!(json["hold_3rd_share"], 1.0);
    assert_eq!(json["holders_flank_share"], 1.0);
}

/// A holder killed by a bot in the killed lane is not a flank, and a kill 10 s after the last is not a quick second.
#[test]
pub(crate) fn a_killer_in_the_killed_lane_is_no_flank() {
    let (mut game, mut tally) = made_up(&[0, 1, 0, 0, 0]);
    kills_and_notes(
        &mut game,
        &mut tally,
        400,
        &[(20.0, 35.0), (30.0, 30.0), (21.0, 35.0), (22.0, 35.0), (19.0, 36.0)],
        &[(20, 1, 0), (220, 1, 2), (300, 4, 1)],
        &[],
    );
    let counts = tally.topic::<OutcomeCounts>();
    assert_eq!(counts.all().holders_killed, [1, 0]);
    assert_eq!(counts.all().quick_second, 0);
    assert_eq!(counts.all().hold_runs, [1, 0]);
}

/// A carrier in an enemy's clear sight is counted, and not targeted unless the enemy's target is him.
#[test]
pub(crate) fn a_carrier_in_clear_sight_is_counted_targeted_or_not() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    play(&mut game, &mut tally, 100, |k, game| {
        put(game, 0, 10.0, 35.0, 0);
        put(game, 1, 30.0, 36.0, 0);
        game.bodies[0].carrying = Some(0);
        game.bodies[1].target = (k >= 60).then_some(0);
        vec![bot_now(); 2]
    });
    let counts = tally.topic::<OutcomeCounts>();
    assert!(counts.all().carrier_clear_ms >= 4_000, "{}", counts.all().carrier_clear_ms);
    // The first 60 ticks (3 s) not targeted, the last 40 targeted: the off share is the larger part of the first.
    assert!(counts.all().carrier_clear_off_ms >= 2_500 && counts.all().carrier_clear_off_ms < counts.all().carrier_clear_ms, "{} of {}", counts.all().carrier_clear_off_ms, counts.all().carrier_clear_ms);
}

/// An enemy hurt to pain under 30 % with a bot near, alive 15 s later, is counted; one who dies before is not.
#[test]
pub(crate) fn hurt_enemies_that_live_fifteen_seconds_more_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 1, 1]);
    let low = 0.1 * Pools::default().pain;
    play(&mut game, &mut tally, 340, |k, game| {
        put(game, 0, 20.0, 35.0, 0);
        put(game, 1, 30.0, 36.0, 0);
        put(game, 2, 31.0, 36.0, 0);
        game.bodies[1].pools.pain = low;
        game.bodies[2].pools.pain = low;
        game.bodies[2].alive = k < 100;
        vec![bot_now(); 3]
    });
    let counts = tally.topic::<OutcomeCounts>();
    assert_eq!((counts.all().hurt_events, counts.all().hurt_lived), (2, 1));
}

/// A battery taken again is timed from its drop, by whether a hurt enemy of the dropper was near the drop, and filed under the team
/// that took it; a battery put home is no retake.
#[test]
pub(crate) fn a_retaken_battery_is_timed_by_the_hurt_enemy_near_its_drop_and_filed_under_the_taker() {
    let (mut game, _) = made_up(&[0, 1, 0]);
    let mut counts = OutcomeCounts::default();
    let low = 0.1 * Pools::default().pain;
    let at = Vec3::new(20.0, 0.0, 35.0);
    // Bot 0 (team 0) carries battery 0, bot 1 (team 1) carries battery 1.
    game.batteries = vec![Lying::Carried(0), Lying::Carried(1)];
    game.tick = 1_000;
    counts.batteries_tick(&game, |_| false);
    // Both drop them at 2 s: bot 1 stands hurt within 10 m of battery 0's drop (an enemy of its dropper), bot 0 far from battery 1's.
    put(&mut game, 0, 90.0, 35.0, 0);
    put(&mut game, 1, 25.0, 35.0, 0);
    game.bodies[1].pools.pain = low;
    game.batteries = vec![Lying::Free(at, 0), Lying::Free(Vec3::new(60.0, 0.0, 35.0), 0)];
    game.tick = 2_000;
    counts.batteries_tick(&game, |_| false);
    // Battery 0 is taken by bot 2 (team 0) at 6 s; battery 1 goes home at 7 s (no retake), is carried again by bot 0 and dropped.
    game.batteries = vec![Lying::Carried(2), Lying::Home(1)];
    game.tick = 6_000;
    counts.batteries_tick(&game, |_| false);
    game.batteries = vec![Lying::Carried(2), Lying::Carried(0)];
    game.tick = 7_000;
    counts.batteries_tick(&game, |_| false);
    assert_eq!(counts.by[0].retake, [(0, 0), (4_000, 1)], "taken by team 0 after 4 s, a hurt enemy of the dropper near the drop");
    assert_eq!(counts.by[1].retake, [(0, 0), (0, 0)]);
    assert_eq!(counts.all().retake[0], (0, 0), "the battery that went home was no retake");
}

/// Traps are counted by the stance the placer was in, and a catch by the stance of his last placement.
#[test]
pub(crate) fn traps_are_counted_by_the_placers_stance() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    game.traps.events.push(TrapEvent::Placed { tick: 50, place: 0, team: 0, press_m: 1.0, placer_crouched: true, placer_speed: 0.0 });
    play(&mut game, &mut tally, 2, |_, game| {
        put(game, 0, 20.0, 35.0, key::CROUCH);
        put(game, 1, 30.0, 36.0, 0);
        vec![bot_now(); 2]
    });
    game.traps.events.push(TrapEvent::Caught {
        tick: 60,
        placer: 0,
        victim: 0,
        kind: KillKind::Own,
        placed: 50,
        placer_crouched: true,
        press_m: 1.0,
        eye_m: 1.04,
        pitch_down: 1.0,
        placer_team: 0,
        victim_team: 0,
        placer_speed: 0.0,
    });
    play(&mut game, &mut tally, 1, |_, game| {
        put(game, 0, 20.0, 35.0, 0);
        put(game, 1, 30.0, 36.0, 0);
        vec![bot_now(); 2]
    });
    let counts = tally.topic::<OutcomeCounts>();
    assert_eq!(counts.all().placed, [0, 1]);
    assert_eq!(counts.all().caught, [[0, 0, 0], [0, 0, 1]]);
}

/// A holder is killed "after two" only inside his hold: a shooter who made two kills, then went quiet for over 120 s, is not a
/// holder killed, and a hold is filed under the team of its victims, the kill of the holder under his killer's.
#[test]
pub(crate) fn a_holder_is_killed_only_inside_his_hold_and_the_counts_are_filed_by_decision() {
    // Kills at 1 s and 2 s by bot 1 (team 1) of bots 0 and 2 (team 0), then bot 3 (team 0) kills him a long time later.
    let late = |at_s: u32| {
        let (mut game, mut tally) = made_up(&[0, 1, 0, 0]);
        let ticks = (at_s * 1000 / TICK_MS + 40) as usize;
        let late = (at_s * 1000 / TICK_MS) as usize;
        kills_and_notes(
            &mut game,
            &mut tally,
            ticks,
            &[(20.0, 35.0), (30.0, 30.0), (21.0, 35.0), (45.0, 30.0)],
            &[(20, 1, 0), (40, 1, 2), (late, 3, 1)],
            &[],
        );
        tally.topic::<OutcomeCounts>().by
    };
    // Inside the hold: filed under the killer's team 0 (its flank decision), the hold under the victims' team 0.
    let inside = late(10);
    assert_eq!(inside[0].holders_killed, [1, 1]);
    assert_eq!(inside[1].holders_killed, [0, 0]);
    assert_eq!(inside[0].kills, 2, "the kills were suffered by team 0: filed under it");
    assert_eq!(inside[1].kills, 1, "and the holder's death is team 1's");
    // After 120 s with no kill: the hold had run out.
    let after = late(130);
    assert_eq!(after[0].holders_killed, [0, 0]);
}

/// The pairs sampled and the picture's flips: a wall between a body and its picture is a truth and a picture that differ.
#[test]
pub(crate) fn the_sight_flips_are_counted_by_pairs_and_by_the_looking_bots_team() {
    // The soak field's wall stands at x = 40 for z < 25. Bot 0 (team 0) at (30, 30); bot 1 (team 1) behind the wall at (50, 10),
    // so truly hidden from him; the picture of bot 1 put past the wall's end at (50, 30), where it is visible: "visible, truly
    // hidden".
    let (mut game, mut tally) = made_up(&[0, 1]);
    put(&mut game, 0, 30.0, 30.0, 0);
    put(&mut game, 1, 50.0, 10.0, 0);
    let counts = tally.topic::<OutcomeCounts>();
    counts.estimates = vec![Some(Vec3::new(30.0, 0.0, 30.0)), Some(Vec3::new(50.0, 0.0, 30.0))];
    counts.flip_sample(&game);
    assert_eq!(counts.by[0].flips, [0, 0, 1, 1], "team 0 looks: the picture says visible, truly hidden");
    assert_eq!((counts.by[0].flip_pairs, counts.by[0].flip_moments), (1, 1));
    assert_eq!(counts.by[1].flips, [0, 0, 1, 1], "team 1 looks too, at bot 0 with its picture of him");
    // Past the wall's end both see each other truly, and a picture behind the wall says hidden: a missed sight.
    put(&mut game, 0, 30.0, 30.0, 0);
    put(&mut game, 1, 50.0, 30.0, 0);
    counts.estimates = vec![Some(Vec3::new(30.0, 0.0, 30.0)), Some(Vec3::new(50.0, 0.0, 10.0))];
    counts.flip_sample(&game);
    assert_eq!(counts.by[0].flips[0..2], [1, 1], "team 0: truly visible, the picture says hidden");
    assert_eq!(counts.by[0].flip_pairs, 2);
}

/// Escort plays are counted by the team of the bot: those that ended, the sum of their lives, and those under 1.5 s. A play
/// replaced by another (a `takes` of another kind) is not counted; its later `ends` has no start.
#[test]
pub(crate) fn escort_plays_are_counted_with_their_lives_by_team() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    kills_and_notes(
        &mut game,
        &mut tally,
        200,
        &[(20.0, 35.0), (30.0, 30.0)],
        &[],
        &[
            (10, 0, "takes Escort Battery(1) (score 0.5: the best play)"),
            (14, 0, "ends Escort: no way to its goal"),
            (20, 1, "takes Escort Place(0) (score 0.4: the best play)"),
            (120, 1, "ends Escort: he is home"),
            (130, 1, "takes Escort Place(0) (score 0.4: the best play)"),
            (135, 1, "takes Collect Battery(2) (score 0.4: a better play)"),
            (140, 1, "ends Escort: he is home"),
        ],
    );
    let counts = tally.topic::<OutcomeCounts>();
    assert_eq!((counts.by[0].escort_ended, counts.by[0].escort_short, counts.by[0].escort_no_way), (1, 1, 1));
    assert_eq!((counts.by[1].escort_ended, counts.by[1].escort_short, counts.by[1].escort_no_way), (1, 0, 0));
    let (_, json) = tally.finish(&game);
    assert_eq!(json["by_team"]["0"]["escort_plays_ended"], 1, "{json}");
    assert!((json["by_team"]["1"]["escort_life_s"].as_f64().unwrap() - 5.0).abs() < 0.01, "{json}");
}
