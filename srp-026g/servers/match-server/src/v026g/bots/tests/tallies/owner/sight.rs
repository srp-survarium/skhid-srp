use std::collections::VecDeque;

use super::*;
use crate::v026g::bots::reflex::{Gates, gate};
use crate::v026g::bots::sense::in_view;

/// The sight counts (`pr-sight.md`'s target metric and guards): the fire runs and what ended them,
/// the gaps between runs, the kills' bursts, and the targets' sights lost. Worked out from the
/// stand-in game alone (keys, targets, bodies, hits) and the one record of the bot's own state the
/// gate tally also reads (`Reflex::gates`, the fire gates of a tick); the same code counts before
/// and after the sight refresh, so a run of either gives its side of a paired comparison. What
/// only the refreshed bots know (the poll gaps, the ray budget's refusals, `Lapse` by ping band)
/// is the match-end line of `Bots::poll_report`, printed by the bench as `sight polls:`.
///
/// A fire run is the ticks in a row with the fire key held. It ends at the first tick without it:
/// by the shooter's kill (within two ticks), his death, the death of the target by another hand,
/// a sight lapse (`gate::LOST` or `gate::NO_TARGET` that tick), or the first other gate that
/// tick (the reflex's gate bits for the tick the key was decided for; "not_run" when no record
/// has that tick). A burst is `fact-fire-human-rule.md`'s and `duels-humans-vs-bots.md` §1's:
/// fire ticks with no more than 0.8 s between (3 s for a bolt or a shotgun, 1.5 s for a
/// marksman's rifle or a pistol).
///
/// A target's sight lapses when the bot's target (the one its inputs were made with) goes to none
/// while that enemy lives; the cause is the stand-in's truth at that tick, in the order the
/// refreshed look tests: beyond the sight range, outside the view, no line on the bullet mesh, and
/// else "visible" (the enemy showed; the bot let him go: the think cadence before the refresh).
#[derive(Default)]
pub(super) struct SightCounts {
    /// By place: the fire run under way.
    run: Vec<Option<FireRun>>,
    /// By place: the tick a run that ended with its target alive ended at, and its weapon's class.
    after: Vec<Option<(u32, &'static str)>>,
    /// By place: the last gate records ((tick, bits)), the newest last.
    gates: Vec<VecDeque<(u32, u32)>>,
    /// By place: the fire gates of the bot's last tick, as read (`Reflex::gates`).
    now_gates: Vec<Gates>,
    /// By place: the tick of the last kill.
    last_kill: Vec<Option<u32>>,
    /// By place: the target of the last tick.
    target_was: Vec<Option<Place>>,
    /// The range and the width of the view of the bots' sight, m and degrees (the tuning's).
    pub(super) range_m: f32,
    pub(super) view_deg: f32,
    /// By place: the ping the bench gave it (`SRP_BENCH_PING`; taken as its lateness, like the
    /// bench's tracker), ms.
    pub(super) pings: Vec<u32>,
    /// By bot: runs and the ticks they held.
    runs_by_bot: Vec<[u32; 2]>,
    /// Runs by length: 1 tick, 2, 3-4, 5 or more.
    lengths: [u32; 4],
    /// Runs by what ended them.
    ends: BTreeMap<&'static str, u32>,
    /// Ticks held in the runs that ended with the target alive.
    held_alive_ticks: u64,
    /// Gaps after those runs to the next run of the same class within [`GAP_MAX_TICKS`], by length:
    /// 1-2 ticks, 3-4, 5-8, 9-20.
    gaps: [u32; 4],
    /// By weapon class: for each kill, the ms from the first fire tick of its last burst, and the
    /// presses in it.
    kills: BTreeMap<&'static str, (Vec<u32>, Vec<u32>)>,
    /// Target sights lost, by cause ([`CAUSES`]) and by the target's ping band.
    lapses: [u32; 4],
    lapses_by_band: [u32; PING_BANDS],
    /// Runs ended by a sight lapse, by the target's ping band.
    cuts_by_band: [u32; PING_BANDS],
    /// By place: the target and the last tick the truth (the stand-in's bullet mesh from the eye to the
    /// chest or the head, as `fire_hidden` tests) had a line to him.
    clear: Vec<Option<(Place, u32)>>,
    /// Fire ticks at a target with no line, by the age of the break: up to 150 ms, up to 450, over 450,
    /// never a line since the bot took him ([`AGES`]).
    hidden_by_age: [u32; 4],
    /// By place: the bot's memory counts read last (the sight refreshes, the hold uses), and the sums
    /// (a count that goes down is a new memory: the new total is all new).
    memory_seen: Vec<(u32, u32)>,
    refreshes: u64,
    hold_uses: u64,
}

/// The age buckets of the break of the line to a target fired at ([`SightCounts::hidden_by_age`]).
const AGES: [&str; 4] = ["0-150", "150-450", "450+", "never"];

/// A fire run under way.
#[derive(Clone, Copy, Debug)]
struct FireRun {
    start: u32,
    ticks: u32,
    /// The target at its last tick.
    target: Option<Place>,
    class: &'static str,
}

impl SightCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            run: vec![None; n],
            after: vec![None; n],
            gates: vec![VecDeque::new(); n],
            now_gates: vec![Gates::default(); n],
            clear: vec![None; n],
            memory_seen: vec![(0, 0); n],
            last_kill: vec![None; n],
            target_was: vec![None; n],
            range_m: 150.0,
            view_deg: 90.0,
            runs_by_bot: vec![[0; 2]; n],
            ..Self::default()
        }
    }
}

impl Topic for SightCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(gates) = self.now_gates.get_mut(place) {
            *gates = bot.map_or_else(Gates::default, |bot| bot.reflex.gates());
        }
        if let (Some(bot), Some(seen)) = (bot, self.memory_seen.get_mut(place)) {
            let (refreshes, holds) = (bot.memory.refreshes(), bot.memory.hold_uses());
            let new = |now: u32, before: u32| u64::from(if now >= before { now - before } else { now });
            self.refreshes += new(refreshes, seen.0);
            self.hold_uses += new(holds, seen.1);
            *seen = (refreshes, holds);
        }
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        if let Some(gates) = self.now_gates.get_mut(place) {
            *gates = now.map_or_else(Gates::default, |n| n.gates);
        }
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.range_m = bots.match_wide_tuning().sense.sight_m;
        self.view_deg = bots.match_wide_tuning().sense.view_deg;
    }

    fn body(&mut self, run: &Run, at: &BodyAt<'_>, rays: &mut u64) {
        // Only a bot has a gate record.
        if run.is_bot[at.place] {
            self.sight_tick(at, rays);
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        let deaths: Vec<Place> = (0..run.recs.len())
            .filter(|&place| match run.recs[place].as_slice() {
                [.., prev, last] => prev.alive && !last.alive,
                _ => false,
            })
            .collect();
        self.sight_kills(run, game, &deaths);
    }

    fn parts(&mut self, run: &Run, _game: &Game, scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("sight", self.part(run, scale))]
    }
}

/// The run ends not made by a gate.
const KILL: &str = "kill";
const DIED: &str = "died";
const TARGET_DIED: &str = "target_died";
const SIGHT: &str = "sight";
const NOT_RUN: &str = "not_run";

/// The gate records kept for each bot, ticks.
const GATE_HISTORY: usize = 12;

/// A gap after a run counts when the next run of the weapon's class comes within this many ticks
/// (1 s).
const GAP_MAX_TICKS: usize = 20;

/// The causes of a target sight lost, in the order the look tests them.
const CAUSES: [&str; 4] = ["visible", "range", "cone", "blocked"];

/// The ping bands, as `polls::PING_BAND_EDGES_MS` and `LATENESS_PER_PING` make them in lateness: a
/// bench ping is taken as the lateness itself.
const PING_BANDS: usize = 4;
const LATENESS_EDGES_MS: [f32; PING_BANDS - 1] = [9.0, 27.0, 67.5];
const BAND_NAMES: [&str; PING_BANDS] = ["<20", "20-60", "60-150", "150+"];

/// The longest gap that joins fire ticks into one burst, ticks, by weapon class
/// (`duels-humans-vs-bots.md` §1).
fn burst_gap_ticks(class: &str) -> usize {
    let ms = match class {
        "bolt" | "shotgun" => 3_000,
        "marksman" | "pistol" => 1_500,
        _ => 800,
    };
    (ms / TICK_MS) as usize
}

/// The first gate of `bits` in the reflex's order, a sight lapse for the two gates that are one.
fn first_gate(bits: u32) -> &'static str {
    gate::NAMES
        .iter()
        .find(|(bit, _)| bits & bit != 0)
        .map_or("none", |&(bit, name)| {
            if bit == gate::LOST || bit == gate::NO_TARGET { SIGHT } else { name }
        })
}

/// The quartiles of `v` (q1, median, q3; the lower element between two).
fn quartiles(v: &mut [u32]) -> [u32; 3] {
    if v.is_empty() {
        return [0; 3];
    }
    v.sort_unstable();
    let q = |k: usize| v[(v.len() - 1) * k / 4];
    [q(1), q(2), q(3)]
}

/// `x` a hundred seconds of `secs`.
fn per_100(x: f64, secs: f64) -> f64 {
    if secs > 0.0 { (1000.0 * 100.0 * x / secs).round() / 1000.0 } else { 0.0 }
}

impl OwnerTally {
    /// The bench's ping for each place (`SRP_BENCH_PING`; none: all the lowest band), for the counts
    /// by the target's ping band.
    pub(crate) fn set_pings(&mut self, pings: Vec<u32>) {
        self.topic::<SightCounts>().pings = pings;
    }
}

impl SightCounts {
    /// The bot at `place`: its fire run, the end of one, the gap before a new one, and the loss of
    /// its target's sight. The killing shots are taken by [`Self::sight_kills`] after the tick.
    fn sight_tick(&mut self, at: &BodyAt<'_>, rays: &mut u64) {
        let (game, place, sight) = (at.game, at.place, at.sight);
        let body = &game.bodies[place];
        let now = self.now_gates[place];
        let tick = game.tick;
        let class = body.gun.class();
        let firing = body.alive && body.keys & key::FIRE != 0;
        self.hidden_age(game, place, body, sight, firing);
        let gates = &mut self.gates[place];
        if gates.back().is_none_or(|&(at, _)| at != now.at) {
            gates.push_back((now.at, now.bits));
            if gates.len() > GATE_HISTORY {
                gates.pop_front();
            }
        }
        match (self.run[place], firing) {
            (Some(mut run), true) => {
                run.ticks += 1;
                run.target = body.target;
                self.run[place] = Some(run);
            }
            (None, true) => {
                if let Some((ended, was)) = self.after[place].take() {
                    let up = ((tick - ended) / TICK_MS) as usize;
                    if was == class && (1..=GAP_MAX_TICKS).contains(&up) {
                        self.gaps[[0, 0, 1, 1, 2, 2, 2, 2].get(up - 1).copied().unwrap_or(3)] += 1;
                    }
                }
                self.run[place] = Some(FireRun {
                    start: tick,
                    ticks: 1,
                    target: body.target,
                    class,
                });
            }
            (Some(run), false) => {
                self.run[place] = None;
                self.sight_run_ended(game, place, run, body);
            }
            (None, false) => {}
        }
        // The loss of the target's sight: to none while he lives.
        let target = body.target.filter(|_| body.alive);
        if let (Some(was), None) = (self.target_was[place], target)
            && body.alive
            && let Some(him) = enemy_of(game, place, body.team, was)
        {
            let cause = if body.feet.distance_xz(him.feet) > self.range_m {
                1
            } else if !in_view(body.feet, body.yaw, him.feet, self.view_deg) {
                2
            } else if !in_sight(sight, body, him, f32::INFINITY, rays) {
                3
            } else {
                0
            };
            self.lapses[cause] += 1;
            self.lapses_by_band[self.sight_band(was)] += 1;
        }
        self.target_was[place] = target;
    }

    /// The line to the target of the body at `place` this tick, by the truth of the stand-in (the test of
    /// `fire_hidden`, its rays not counted): the last tick it held, and, on a tick of fire at a target with
    /// no line, how long ago that was.
    fn hidden_age(&mut self, game: &Game, place: Place, body: &super::super::Body, sight: &mapdata::RayTree, firing: bool) {
        let Some((target, him)) = body
            .target
            .filter(|_| body.alive)
            .and_then(|t| enemy_of(game, place, body.team, t).map(|him| (t, him)))
        else {
            return;
        };
        let mut spare = 0u64;
        if in_sight(sight, body, him, f32::INFINITY, &mut spare) {
            self.clear[place] = Some((target, game.tick));
        } else if firing {
            let ago = self.clear[place]
                .filter(|&(was, _)| was == target)
                .map(|(_, at)| game.tick - at);
            let bucket = match ago {
                Some(ms) if ms <= 150 => 0,
                Some(ms) if ms <= 450 => 1,
                Some(_) => 2,
                None => 3,
            };
            self.hidden_by_age[bucket] += 1;
        }
    }

    /// The ping band of `place` (the bench's ping for it, none: the lowest).
    fn sight_band(&self, place: Place) -> usize {
        let ping = self.pings.get(place).copied().unwrap_or(0) as f32;
        LATENESS_EDGES_MS.iter().take_while(|&&edge| ping >= edge).count()
    }

    /// A run that ended at this tick: by what.
    fn sight_run_ended(&mut self, game: &Game, place: Place, run: FireRun, body: &super::super::Body) {
        let tick = game.tick;
        let long = (run.ticks as usize).clamp(1, 5);
        self.lengths[[0, 0, 1, 2, 2, 3][long]] += 1;
        let by_bot = &mut self.runs_by_bot[place];
        by_bot[0] += 1;
        by_bot[1] += run.ticks;
        let killed = self.last_kill[place]
            .is_some_and(|k| k >= run.start && tick <= k + 2 * TICK_MS);
        let target_gone = run
            .target
            .is_some_and(|t| game.bodies.get(t).is_some_and(|b| !b.alive));
        let ended = if !body.alive {
            DIED
        } else if killed {
            KILL
        } else if target_gone {
            TARGET_DIED
        } else {
            match self.gates[place].iter().find(|&&(at, _)| at == tick) {
                None => NOT_RUN,
                Some(&(_, bits)) => first_gate(bits),
            }
        };
        *self.ends.entry(ended).or_default() += 1;
        if ended == SIGHT
            && let Some(target) = run.target
        {
            self.cuts_by_band[self.sight_band(target)] += 1;
        }
        if !matches!(ended, KILL | DIED | TARGET_DIED) {
            self.held_alive_ticks += u64::from(run.ticks);
            self.after[place] = Some((tick, run.class));
        }
    }

    /// The kills of this tick (`deaths`: the bodies that died): by the shooter of the hit that tick.
    /// The burst the kill ended, and its first fire tick and presses.
    fn sight_kills(&mut self, run: &Run, game: &Game, deaths: &[Place]) {
        let tick = game.tick;
        for &victim in deaths {
            let Some(&(_, killer, _)) = game
                .hit_log
                .iter()
                .rev()
                .find(|&&(t, _, v)| t == tick && v == victim)
            else {
                continue;
            };
            if !run.is_bot.get(killer).copied().unwrap_or(false) {
                continue;
            }
            self.last_kill[killer] = Some(tick);
            let class = game.bodies[killer].gun.class();
            let gap = burst_gap_ticks(class);
            let recs = &run.recs[killer];
            let fires = |r: &Rec| r.alive && r.keys & key::FIRE != 0;
            let Some(last) = recs.iter().rposition(fires) else {
                continue;
            };
            // Back from the last fire tick while the ticks between are no more than the gap.
            let (mut first, mut quiet) = (last, 0usize);
            for k in (0..last).rev() {
                if fires(&recs[k]) {
                    first = k;
                    quiet = 0;
                } else {
                    quiet += 1;
                    if quiet > gap {
                        break;
                    }
                }
            }
            // A press is a fire tick after a tick without.
            let presses = (first..=last)
                .filter(|&k| fires(&recs[k]) && (k == 0 || !fires(&recs[k - 1])))
                .count() as u32;
            let here = run.index(tick);
            let (ms, presses_at) = self.kills.entry(class).or_default();
            ms.push((here.saturating_sub(first) as u32) * TICK_MS);
            presses_at.push(presses);
        }
    }

    /// The sight counts at the end: their keys of the `owner` object and their lines of the
    /// owner-test text.
    fn part(&self, run: &Run, scale: &Scale) -> Part {
        let s = self;
        let tick_s = scale.tick_s;
        let runs: u32 = s.lengths.iter().sum();
        let held_ticks: u64 = s.runs_by_bot.iter().map(|r| u64::from(r[1])).sum();
        let held_s = held_ticks as f64 * tick_s;
        let held_alive_s = s.held_alive_ticks as f64 * tick_s;
        let ends: Vec<(&'static str, u32)> = [KILL, DIED, TARGET_DIED, SIGHT, NOT_RUN]
            .into_iter()
            .chain(gate::NAMES.iter().map(|&(_, name)| name))
            .chain(["none"])
            .filter(|name| !matches!(*name, "no_target" | "lost"))
            .map(|name| (name, s.ends.get(name).copied().unwrap_or(0)))
            .filter(|&(_, n)| n > 0)
            .collect();
        let ends_per_100: BTreeMap<&str, f64> = ends
            .iter()
            .map(|&(name, n)| (name, per_100(f64::from(n), held_s)))
            .collect();
        let gap_names = ["1-2", "3-4", "5-8", "9-20"];
        let gaps_per_100: BTreeMap<&str, f64> = gap_names
            .iter()
            .zip(&s.gaps)
            .map(|(name, &n)| (*name, per_100(f64::from(n), held_alive_s)))
            .collect();
        let pause_sized: u32 = s.gaps[1..].iter().sum();
        let pause_per_100 = per_100(f64::from(pause_sized), held_alive_s);
        let mut kills_n = 0usize;
        let mut ftk: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
        let mut presses: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
        let mut kill_text = Vec::new();
        for (class, (ms, press)) in &s.kills {
            kills_n += ms.len();
            let (q_ms, q_p) = (quartiles(&mut ms.clone()), quartiles(&mut press.clone()));
            let mean = f64::from(press.iter().sum::<u32>()) / press.len().max(1) as f64;
            ftk.insert(class, serde_json::json!([q_ms[0], q_ms[1], q_ms[2], ms.len()]));
            presses.insert(
                class,
                serde_json::json!([q_p[0], q_p[1], q_p[2], (mean * 100.0).round() / 100.0, press.len()]),
            );
            kill_text.push(format!(
                "{class} {}/{}/{} ms, presses {}/{}/{} (mean {:.2}) over {}",
                q_ms[0],
                q_ms[1],
                q_ms[2],
                q_p[0],
                q_p[1],
                q_p[2],
                mean,
                ms.len()
            ));
        }
        let lapses: u32 = s.lapses.iter().sum();
        let cuts: u32 = s.cuts_by_band.iter().sum();
        let by_cause: BTreeMap<&str, u32> = CAUSES.iter().copied().zip(s.lapses).collect();
        let by_cause_min: BTreeMap<&str, f64> = CAUSES
            .iter()
            .copied()
            .zip(s.lapses)
            .map(|(name, n)| (name, scale.per_min(f64::from(n))))
            .collect();
        let by_band = |counts: &[u32; PING_BANDS]| -> BTreeMap<&str, u32> {
            BAND_NAMES.iter().copied().zip(*counts).collect()
        };
        let runs_by_bot: BTreeMap<String, [u32; 2]> = s
            .runs_by_bot
            .iter()
            .enumerate()
            .filter(|&(place, _)| run.is_bot.get(place).copied().unwrap_or(false))
            .map(|(place, r)| (place.to_string(), *r))
            .collect();
        let json = serde_json::json!({
            "sight_fire_runs": runs,
            "sight_fire_held_s": held_s.round(),
            "sight_run_lengths": s.lengths,
            "sight_runs_by_bot": runs_by_bot,
            "sight_run_ends": ends.iter().copied().collect::<BTreeMap<_, _>>(),
            "sight_run_ends_per_100s_held": ends_per_100,
            "sight_gap_held_s": held_alive_s.round(),
            "sight_gaps_per_100s_held": gaps_per_100,
            "sight_pause_gaps_per_100s_held": pause_per_100,
            "sight_gun_kills": kills_n,
            "sight_fire_to_kill_ms_by_class": ftk,
            "sight_presses_per_kill_by_class": presses,
            "sight_target_lapses": lapses,
            "sight_target_lapses_per_min": scale.per_min(f64::from(lapses)),
            "sight_lapse_causes": by_cause,
            "sight_lapse_causes_per_min": by_cause_min,
            "sight_lapses_by_ping_band": by_band(&s.lapses_by_band),
            "sight_cuts_by_ping_band": by_band(&s.cuts_by_band),
            "sight_refreshes": s.refreshes,
            "sight_refreshes_per_min": scale.per_min(s.refreshes as f64),
            "sight_hold_uses": s.hold_uses,
            "sight_hold_uses_per_min": scale.per_min(s.hold_uses as f64),
            "sight_hidden_fire_by_age": AGES.iter().copied().zip(s.hidden_by_age).collect::<BTreeMap<_, _>>(),
        });
        let list = |parts: Vec<String>| if parts.is_empty() { "none".to_owned() } else { parts.join(" ") };
        let text = format!(
            "owner-test sight fire: runs {runs} over {held_s:.0} s held (lengths 1 tick {} 2 {} 3-4 {} 5+ {}); ended by {}\n  owner-test sight gaps: after runs that ended with the target alive ({held_alive_s:.0} s held), per 100 s held: 1-2 ticks {} 3-4 {} 5-8 {} 9-20 {}; pause-sized (3+ ticks) {pause_per_100}\n  owner-test sight kills: {kills_n} by a gun; fire to kill (first fire tick of the last burst) q1/med/q3 by class {}\n  owner-test sight lapses: target sights lost {lapses} ({} a bot-minute): {}; by the target's ping band {}; runs cut by a lapse {cuts}, by band {}
  owner-test sight activity: sight refreshes {} ({} a bot-minute), hold uses {} ({} a bot-minute); fire at a hidden target by the age of the break of the line: {}",
            s.lengths[0],
            s.lengths[1],
            s.lengths[2],
            s.lengths[3],
            list(ends.iter().map(|(name, n)| format!("{name} {n}")).collect()),
            gaps_per_100["1-2"],
            gaps_per_100["3-4"],
            gaps_per_100["5-8"],
            gaps_per_100["9-20"],
            if kill_text.is_empty() { "none".to_owned() } else { kill_text.join("; ") },
            scale.per_min(f64::from(lapses)),
            list(CAUSES.iter().zip(s.lapses).map(|(name, n)| format!("{name} {n}")).collect()),
            list(BAND_NAMES.iter().zip(s.lapses_by_band).map(|(name, n)| format!("{name} {n}")).collect()),
            list(BAND_NAMES.iter().zip(s.cuts_by_band).map(|(name, n)| format!("{name} {n}")).collect()),
            s.refreshes,
            scale.per_min(s.refreshes as f64),
            s.hold_uses,
            scale.per_min(s.hold_uses as f64),
            list(AGES.iter().zip(s.hidden_by_age).map(|(name, n)| format!("{name} {n}")).collect()),
        );
        Part::new(json, text)
    }
}

/// A bot firing on the ticks `fire` (numbered from 1, 50 ms each) at a standing enemy 20 m on in
/// the open, who dies of its hit at tick `kill`; `gates` is the bot's gate record for each tick:
/// the counts after `ticks` ticks.
#[cfg(test)]
fn shoot(
    fire: &[usize],
    kill: Option<usize>,
    gates: impl Fn(usize) -> (u32, u32),
    ticks: usize,
) -> serde_json::Value {
    let (mut game, mut tally) = made_up(&[0, 1]);
    let mut alive = true;
    play(&mut game, &mut tally, ticks, |k, game| {
        let tick = k + 1;
        put(game, 0, 10.0, 10.0, if fire.contains(&tick) { key::FIRE } else { 0 });
        put(game, 1, 30.0, 10.0, 0);
        game.bodies[0].target = Some(1);
        if kill == Some(tick) {
            alive = false;
            game.hit_log.push((game.tick, 0, 1));
        }
        game.bodies[1].alive = alive;
        let (at, bits) = gates(tick);
        vec![
            Some(BotNow {
                play: "Roam".to_owned(),
                gates: Gates { at, bits, target: Some(1) },
                ..BotNow::default()
            }),
            None,
        ]
    });
    tally.finish(&game).1
}

#[test]
fn a_run_ended_by_the_kill_and_the_burst_before_it_are_counted() {
    // Fire on ticks 2-4, quiet 5-6, fire 7-9 and the kill at 9: one burst of two presses, from
    // tick 2 to 9: 350 ms.
    let json = shoot(&[2, 3, 4, 7, 8, 9], Some(9), |t| ((t as u32 + 1) * TICK_MS, 0), 14);
    assert_eq!(json["sight_fire_runs"], 2, "{json}");
    assert_eq!(json["sight_run_ends"]["kill"], 1);
    assert_eq!(json["sight_fire_to_kill_ms_by_class"]["rifle"], serde_json::json!([350, 350, 350, 1]), "{json}");
    assert_eq!(json["sight_presses_per_kill_by_class"]["rifle"][1], 2);
    // The first run ended with the target alive at tick 5; the gap to tick 7 is two ticks.
    assert!(json["sight_gaps_per_100s_held"]["1-2"].as_f64().unwrap() > 0.0, "{json}");
    assert_eq!(json["sight_pause_gaps_per_100s_held"], 0.0);
}

#[test]
fn a_run_ended_by_a_lost_target_is_a_sight_lapse_and_a_longer_gap_is_a_pause() {
    // The gate record of the tick a run ended at has `LOST` (the tick after the run's last).
    let lost = |t: usize| ((t as u32) * TICK_MS, if t == 5 { gate::LOST } else { 0 });
    // Fire on 2-4, up at 5 (lost), fire again on 9-10 (the gap 4 ticks).
    let json = shoot(&[2, 3, 4, 9, 10], None, lost, 14);
    assert_eq!(json["sight_run_ends"]["sight"], 1, "{json}");
    assert!(json["sight_gaps_per_100s_held"]["3-4"].as_f64().unwrap() > 0.0, "{json}");
    assert!(json["sight_pause_gaps_per_100s_held"].as_f64().unwrap() > 0.0);
}

#[test]
fn a_target_dropped_in_the_open_is_visible_and_one_behind_the_wall_is_blocked() {
    // The soak field's wall stands at x = 40 for z < 25.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1]);
    play(&mut game, &mut tally, 10, |k, game| {
        put(game, 0, 30.0, 10.0, 0);
        put(game, 1, 50.0, 10.0, 0);
        put(game, 2, 30.0, 32.0, 0);
        put(game, 3, 50.0, 32.0, 0);
        // Both face their enemy (+x) and lose him at tick 5.
        game.bodies[0].yaw = yaw_of([1.0, 0.0]);
        game.bodies[2].yaw = yaw_of([1.0, 0.0]);
        game.bodies[0].target = (k < 5).then_some(1);
        game.bodies[2].target = (k < 5).then_some(3);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["sight_target_lapses"], 2, "{json}");
    assert_eq!(json["sight_lapse_causes"]["blocked"], 1);
    assert_eq!(json["sight_lapse_causes"]["visible"], 1);
}

#[test]
fn fire_at_a_hidden_target_is_counted_by_the_age_of_the_break_of_the_line() {
    // The soak field's wall stands at x = 40 for z < 25. Body 1 is in the open for ten ticks, then
    // behind the wall; body 0 fires at him from tick 5. Body 3 is behind the wall all along; body 2
    // fires at him for ticks 5 to 10.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1]);
    play(&mut game, &mut tally, 40, |k, game| {
        let tick = k + 1;
        put(game, 0, 30.0, 10.0, if tick >= 5 { key::FIRE } else { 0 });
        put(game, 1, if tick <= 10 { 35.0 } else { 50.0 }, 10.0, 0);
        put(game, 2, 30.0, 14.0, if (5..=10).contains(&tick) { key::FIRE } else { 0 });
        put(game, 3, 50.0, 14.0, 0);
        game.bodies[0].yaw = yaw_of([1.0, 0.0]);
        game.bodies[2].yaw = yaw_of([1.0, 0.0]);
        game.bodies[0].target = Some(1);
        game.bodies[2].target = Some(3);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    // Body 1 is hidden from tick 11: 50, 100 and 150 ms after the last tick with a line (the first
    // bucket), 200 to 450 ms (the second), then over 450 ms; body 3 never was in line.
    assert_eq!(
        json["sight_hidden_fire_by_age"],
        serde_json::json!({"0-150": 3, "150-450": 6, "450+": 21, "never": 6}),
        "{json}"
    );
    // The tally of the fight is the same fire: the buckets are its `fire_hidden`.
    assert_eq!(json["fire_hidden"], 36, "{json}");
}
