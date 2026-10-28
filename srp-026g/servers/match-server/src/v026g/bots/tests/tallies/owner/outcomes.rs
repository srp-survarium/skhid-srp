//! The cross-report outcome counts of the 2026-10-28 analyses (`pvb-343-check/traces-20261028`), one topic: what the fixes
//! of the five reports are for, in the numbers those reports used, so that a fix is read on the bench against the reports'
//! targets. A package's own counters (how often its switch fires) are its own topic file; these are the outcomes.
//!
//! - **Holders** (an-anticamper §2.5, §8.2): a shooter's hold is a run of kills from one place (within 10 m, 120 s apart).
//!   Of the runs that reached two kills, the share that made a third (live humans 25 %, the owner against the bots 45 %);
//!   of the holders killed after two, the share killed by a bot that came from a flank, more than 60 degrees off every
//!   victim's bearing (32 % / 9 %); the share of kills that follow another kill of the same shooter within 3 s (live 3.5-3.9 %,
//!   the owner 18 %); lane entries into held lines per 10 s of held time (live 0.24-0.60, bots against the owner 0.84-1.42).
//! - **Fights** (an-fights §5): bot-seconds a carrier is in an enemy's clear sight within 40 m and is not that enemy's target;
//!   enemies hit to pain under 30 % that live 15 s more while a bot of the other team was within 30 m; the time a battery
//!   dropped by a carrier takes to be retaken, by whether a hurt enemy was within 10 m of it.
//! - **Objectives** (an-objective §2.5): battery pickups from a marked zone floor and the carriers that die within 10 s of one;
//!   plays ended "no way to its goal" (Escort, Collect).
//! - **Gear** (an-gear-moves §3.2): traps placed, and caught enemies, mates and the placer himself, by the placer's stance.
//! - **Sight** (an-sight §8.1): of the moments a body was truly visible, the share the bots' picture says hidden (live 22 %);
//!   of the moments the picture says visible, the share truly hidden (live 20 %). Read with `SRP_BENCH_VIEW_ERROR`.
//!
//! Where a count cannot be read as the report did, the text says how it differs.

use super::*;
use crate::v026g::bots::tests::standin_damage::Pools;
use crate::v026g::bots::tests::standin_traps::Event as TrapEvent;
use crate::v026g::bots::tests::{Kill as KillKind, Lying};
use crate::v026g::bots::view::still_eye;

/// A hold: kills from one place within this of the first, m, each within [`HOLD_APART_MS`] of the last.
const HOLD_M: f32 = 10.0;
const HOLD_APART_MS: u32 = 120_000;
/// A holder's killer is from a flank when more than this off every victim's bearing, degrees (an-anticamper §2.5).
const FLANK_DEG: f32 = 60.0;
/// Two kills of one shooter this close are a quick second kill, ms (an-anticamper §8.2).
const QUICK_MS: u32 = 3_000;
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

/// A shooter's hold under way.
#[derive(Clone, Debug)]
struct Hold {
    spot: Vec3,
    last: u32,
    kills: u32,
    bearings: Vec<f32>,
}

/// A battery dropped and not yet taken again.
#[derive(Clone, Copy, Debug)]
struct Drop {
    battery: usize,
    t: u32,
    hurt_near: bool,
}

/// The sight flip counts (an-sight §8.1): `[truly visible, of them the picture says hidden, the picture says visible, of
/// them truly hidden]`.
type Flips = [u32; 4];

#[derive(Default)]
pub(super) struct OutcomeCounts {
    // Holders.
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    holds: BTreeMap<Place, Hold>,
    /// Holds that reached two kills, and of them three.
    hold_runs: [u32; 2],
    /// Holders killed with two kills made, and of them by a flank.
    holders_killed: [u32; 2],
    kills: u32,
    quick_second: u32,
    last_by: BTreeMap<Place, u32>,
    // Fights.
    carrier_clear_ms: u32,
    carrier_clear_off_ms: u32,
    /// By body: since when it is hurt (pain under [`HURT_PAIN`]), its spawn then, and whether a bot of the other team was near.
    hurt: Vec<Option<(u32, u32, bool)>>,
    /// By body: its hurt spell was counted and has not ended (pain back over the line, or a death).
    hurt_counted: Vec<bool>,
    hurt_events: u32,
    hurt_lived: u32,
    /// By battery: carried at the last reading.
    carried: Vec<bool>,
    drops: Vec<Drop>,
    /// Retake times by [no hurt enemy near the drop, one near]: the sum, ms, and the number.
    retake: [(u64, u32); 2],
    // Objectives.
    zone_takes: u32,
    zone_pickups: Vec<(Place, u32)>,
    zone_take_died: u32,
    escort_no_way: u32,
    collect_no_way: u32,
    // Gear.
    traps_seen: usize,
    /// By place: crouched at the last placement.
    stance: BTreeMap<Place, bool>,
    /// By stance [standing, crouched]: placed, and caught [enemy, mate, own].
    placed: [u32; 2],
    caught: [[u32; 3]; 2],
    // Sight.
    estimates: Vec<Option<Vec3>>,
    flips: Flips,
    flip_samples: u32,
}

impl OutcomeCounts {
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
            // A holder with two kills made, killed.
            if let Some(hold) = self.holds.remove(&victim) {
                if hold.kills >= 2 {
                    self.holders_killed[0] += 1;
                    let from = yaw_of([s.feet.x - hold.spot.x, s.feet.z - hold.spot.z]);
                    let flank = hold.bearings.iter().all(|&w| wrap(w - from).abs().to_degrees() > FLANK_DEG);
                    self.holders_killed[1] += u32::from(flank);
                }
                self.close(&hold);
            }
            self.kills += 1;
            if self.last_by.insert(shooter, t).is_some_and(|before| t.saturating_sub(before) <= QUICK_MS) {
                self.quick_second += 1;
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
                    self.holds.insert(shooter, Hold { spot: s.feet, last: t, kills: 1, bearings: vec![bearing] });
                }
            }
        }
    }

    /// A hold is over: the runs that reached two and three kills.
    fn close(&mut self, hold: &Hold) {
        self.hold_runs[0] += u32::from(hold.kills >= 2);
        self.hold_runs[1] += u32::from(hold.kills >= 3);
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
                    self.carrier_clear_ms += ms;
                    self.carrier_clear_off_ms += ms * u32::from(enemy.target != Some(place));
                }
            }
        }
    }

    /// Bodies hurt to pain under [`HURT_PAIN`] and alive [`HURT_LIVES_MS`] later.
    fn hurt_tick(&mut self, game: &Game) {
        let full = Pools::default().pain;
        if self.hurt.len() < game.bodies.len() {
            self.hurt.resize(game.bodies.len(), None);
            self.hurt_counted.resize(game.bodies.len(), false);
        }
        for place in 0..game.bodies.len() {
            let body = &game.bodies[place];
            if !body.alive || body.pools.pain >= HURT_PAIN * full {
                self.hurt_counted[place] = false;
            }
            match self.hurt[place] {
                Some((since, spawned, near)) => {
                    if body.spawned != spawned || !body.alive {
                        // Died (or was born again): not alive 15 s later.
                        self.hurt[place] = None;
                    } else if game.tick >= since + HURT_LIVES_MS {
                        self.hurt_lived += u32::from(near);
                        self.hurt[place] = None;
                    }
                }
                None if body.alive && body.pools.pain < HURT_PAIN * full && !self.hurt_counted[place] => {
                    self.hurt_counted[place] = true;
                    let near = game
                        .bodies
                        .iter()
                        .any(|o| o.alive && o.team != body.team && o.feet.distance_xz(body.feet) <= HURT_NEAR_M);
                    self.hurt_events += u32::from(near);
                    self.hurt[place] = Some((game.tick, body.spawned, near));
                }
                None => {}
            }
        }
    }

    /// The batteries: a drop (carried, then lying) and its retake; a pickup from a zone floor.
    fn batteries(&mut self, bots: &Bots, game: &Game) {
        if self.carried.len() < game.batteries.len() {
            self.carried.resize(game.batteries.len(), false);
        }
        let full = Pools::default().pain;
        for (i, lying) in game.batteries.iter().enumerate() {
            match *lying {
                Lying::Free(at, _) if self.carried[i] => {
                    self.carried[i] = false;
                    let hurt_near = game.bodies.iter().any(|b| {
                        b.alive && b.pools.pain < HURT_PAIN * full && b.feet.distance_xz(at) <= RETAKE_NEAR_M && b.carrying.is_none()
                    });
                    self.drops.push(Drop { battery: i, t: game.tick, hurt_near });
                }
                Lying::Carried(by) => {
                    if !self.carried[i] {
                        self.carried[i] = true;
                        self.retaken(i, game.tick);
                        if let Some(body) = game.bodies.get(by)
                            && bots.tactics.as_ref().is_some_and(|t| {
                                game.world.grid.locate(body.feet, 1.0).is_some_and(|node| t.hazards.zones_at(node).next().is_some())
                            })
                        {
                            self.zone_takes += 1;
                            self.zone_pickups.push((by, game.tick));
                        }
                    }
                }
                Lying::Home(_) => {
                    if self.carried[i] {
                        self.carried[i] = false;
                    }
                    self.retaken(i, game.tick);
                }
                Lying::Free(..) => {}
            }
        }
    }

    /// Battery `i` is taken (or home) again: the time since its drop.
    fn retaken(&mut self, i: usize, tick: u32) {
        if let Some(k) = self.drops.iter().position(|d| d.battery == i) {
            let drop = self.drops.remove(k);
            let slot = &mut self.retake[usize::from(drop.hurt_near)];
            slot.0 += u64::from(tick.saturating_sub(drop.t));
            slot.1 += 1;
        }
    }

    /// Carriers that took a battery from a zone and died soon after.
    fn zone_deaths(&mut self, game: &Game) {
        let tick = game.tick;
        self.zone_pickups.retain(|&(place, t)| {
            let died = game.bodies.get(place).is_some_and(|b| !b.alive);
            if died {
                self.zone_take_died += 1;
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
                    let crouched = game.bodies.get(place).is_some_and(|b| b.keys & key::CROUCH != 0);
                    self.stance.insert(place, crouched);
                    self.placed[usize::from(crouched)] += 1;
                }
                TrapEvent::Caught { placer, kind, .. } => {
                    let stance = usize::from(self.stance.get(&placer).copied().unwrap_or(false));
                    let k = match kind {
                        KillKind::Enemy => 0,
                        KillKind::Mate => 1,
                        KillKind::Own => 2,
                    };
                    self.caught[stance][k] += 1;
                }
                _ => {}
            }
        }
        self.traps_seen = events.len();
    }

    /// Once a second: for each living bot its nearest enemies, the line from its eye to the enemy's chest as it is (the true
    /// bodies) and as the bots' picture has it (the reckoned bodies).
    fn flip_sample(&mut self, game: &Game) {
        let sight = &game.world.sight;
        let picture = |place: Place| self.estimates.get(place).copied().flatten();
        let mut flips = self.flips;
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
                flips[0] += u32::from(truth);
                flips[1] += u32::from(truth && !seen);
                flips[2] += u32::from(seen);
                flips[3] += u32::from(seen && !truth);
            }
        }
        self.flips = flips;
        self.flip_samples += 1;
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
        self.batteries(bots, game);
        self.zone_deaths(game);
        if (game.tick / TICK_MS).is_multiple_of(1000 / TICK_MS) {
            self.flip_sample(game);
        }
    }

    fn note(&mut self, _run: &Run, note: &Note, _kind: &NoteKind<'_>, _game: &Game) {
        if note.text.starts_with("ends Escort:") && note.text.contains("no way") {
            self.escort_no_way += 1;
        }
        if note.text.starts_with("ends Collect:") && note.text.contains("no way") {
            self.collect_no_way += 1;
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
        let open: Vec<Hold> = self.holds.values().cloned().collect();
        let mut runs = self.hold_runs;
        for hold in &open {
            runs[0] += u32::from(hold.kills >= 2);
            runs[1] += u32::from(hold.kills >= 3);
        }
        let num = |name: &str| shared.get(name).and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let (entries, held_s) = (num("camper_entries"), num("camper_held_s"));
        let per_10s = if held_s > 0.0 { (1000.0 * 10.0 * entries / held_s).round() / 1000.0 } else { 0.0 };
        let retake = |k: usize| {
            let (sum, n) = self.retake[k];
            if n > 0 { (sum / u64::from(n)) as u32 } else { 0 }
        };
        let rate = Scale::rate;
        let [vis, vis_hidden, pic, pic_blocked] = self.flips;
        let json = serde_json::json!({
            "hold_runs_2": runs[0],
            "hold_runs_3": runs[1],
            "hold_3rd_share": rate(runs[1], runs[0]),
            "holders_killed_after_2": self.holders_killed[0],
            "holders_killed_flank": self.holders_killed[1],
            "holders_flank_share": rate(self.holders_killed[1], self.holders_killed[0]),
            "kills": self.kills,
            "quick_second_kills": self.quick_second,
            "quick_second_share": rate(self.quick_second, self.kills),
            "lane_entries_per_10s": per_10s,
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
            "trap_placed_by_stance": self.placed,
            "trap_caught_by_stance": self.caught,
            "view_flip": {
                "samples": self.flip_samples,
                "truly_visible": vis,
                "picture_hidden": vis_hidden,
                "missed_sight_share": rate(vis_hidden, vis),
                "picture_visible": pic,
                "truly_hidden": pic_blocked,
                "fire_into_cover_share": rate(pic_blocked, pic),
            },
        });
        let text = format!(
            "owner-test outcomes: holders (kills from one place within {HOLD_M} m): {} runs of 2+ kills, {} of 3+ = {:.1} % (live 25 %, the owner 45 %); holders killed after two {}, by a flank {} = {:.1} % (live 32 %, the owner 9 %); quick second kills (within 3 s of the shooter's last) {} of {} = {:.1} % (live 3.5-3.9 %, the owner 18 %); lane entries a 10 s of held time {per_10s} (live 0.24-0.60, bots against the owner 0.84-1.42); fights: carriers in clear sight within 40 m {:.0} s, not targeted {:.0} s; hurt (pain < 30 %, an enemy within 30 m) {}, alive 15 s later {}; battery retake mean ms (no hurt enemy within 10 m of the drop: {} of {}, one: {} of {}); objectives: pickups from a zone floor {}, died within 10 s {}, Escort 'no way' ends {}, Collect 'no way' ends {}; traps by the placer's stance [standing, crouched]: placed {:?}, caught [enemy, mate, own] {:?}; sight: truly visible {vis}, the picture says hidden {vis_hidden} = {:.1} % (live 22 %); the picture says visible {pic}, truly hidden {pic_blocked} = {:.1} % (live 20 %) (the picture is the bots' reckoned bodies, a pair a second, the 2 nearest enemies)",
            runs[0],
            runs[1],
            100.0 * rate(runs[1], runs[0]),
            self.holders_killed[0],
            self.holders_killed[1],
            100.0 * rate(self.holders_killed[1], self.holders_killed[0]),
            self.quick_second,
            self.kills,
            100.0 * rate(self.quick_second, self.kills),
            f64::from(self.carrier_clear_ms) / 1000.0,
            f64::from(self.carrier_clear_off_ms) / 1000.0,
            self.hurt_events,
            self.hurt_lived,
            retake(0),
            self.retake[0].1,
            retake(1),
            self.retake[1].1,
            self.zone_takes,
            self.zone_take_died,
            self.escort_no_way,
            self.collect_no_way,
            self.placed,
            self.caught,
            100.0 * rate(vis_hidden, vis),
            100.0 * rate(pic_blocked, pic),
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
    assert_eq!(counts.hold_runs, [1, 1]);
    assert_eq!(counts.holders_killed, [1, 1]);
    assert_eq!((counts.kills, counts.quick_second), (4, 2));
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
    assert_eq!(counts.holders_killed, [1, 0]);
    assert_eq!(counts.quick_second, 0);
    assert_eq!(counts.hold_runs, [1, 0]);
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
    assert!(counts.carrier_clear_ms >= 4_000, "{}", counts.carrier_clear_ms);
    // The first 60 ticks (3 s) not targeted, the last 40 targeted: the off share is the larger part of the first.
    assert!(counts.carrier_clear_off_ms >= 2_500 && counts.carrier_clear_off_ms < counts.carrier_clear_ms, "{} of {}", counts.carrier_clear_off_ms, counts.carrier_clear_ms);
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
    assert_eq!((counts.hurt_events, counts.hurt_lived), (2, 1));
}

/// A battery lying again is retaken: the time since its drop, by whether a hurt enemy was near the drop.
#[test]
pub(crate) fn a_retaken_battery_is_timed_by_the_hurt_enemy_near_its_drop() {
    let mut counts = OutcomeCounts::default();
    counts.drops.push(Drop { battery: 0, t: 1_000, hurt_near: true });
    counts.drops.push(Drop { battery: 1, t: 1_000, hurt_near: false });
    counts.retaken(0, 5_000);
    counts.retaken(1, 11_000);
    counts.retaken(1, 12_000);
    assert_eq!(counts.retake, [(10_000, 1), (4_000, 1)]);
    assert!(counts.drops.is_empty());
}

/// Traps are counted by the stance the placer was in, and a catch by the stance of his last placement.
#[test]
pub(crate) fn traps_are_counted_by_the_placers_stance() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    game.traps.events.push(TrapEvent::Placed { tick: 50, place: 0 });
    play(&mut game, &mut tally, 2, |_, game| {
        put(game, 0, 20.0, 35.0, key::CROUCH);
        put(game, 1, 30.0, 36.0, 0);
        vec![bot_now(); 2]
    });
    game.traps.events.push(TrapEvent::Caught { tick: 60, placer: 0, victim: 0, kind: KillKind::Own });
    play(&mut game, &mut tally, 1, |_, game| {
        put(game, 0, 20.0, 35.0, 0);
        put(game, 1, 30.0, 36.0, 0);
        vec![bot_now(); 2]
    });
    let counts = tally.topic::<OutcomeCounts>();
    assert_eq!(counts.placed, [0, 1]);
    assert_eq!(counts.caught, [[0, 0, 0], [0, 0, 1]]);
}
