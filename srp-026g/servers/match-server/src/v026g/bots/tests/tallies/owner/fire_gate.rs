//! [A228] The owner counts of the fire gate on the tip (`traces-20261028/a221-bot-misses.md` class A: the live bots fire with the view
//! 2-10 degrees off their target): each shot at its target by how far the view was off his chest, in his chest's half-widths at his
//! distance (`body_half_m` 0.25 m, the gate's own; the live table's bins), whether it began a burst or went on in one, the distance
//! band, and whether the shooter was under fire, with the hits among them; and each engagement (a body's target set to an enemy) by
//! the time from the target's choice to its first shot at him and how the duel ended (the shooter killed him, he killed the
//! shooter, neither before the target changed). Bots only (`Run::is_bot`); what they did against the bench's stand-in camper
//! (`Run::camper`, the human stand-in) is also given apart. Counts only: nothing decides by them.
//!
//! Key of the `owner` object `fire_gate`: `"team<t>"` -> `cells` (`"<off>:<first|burst>:<band>:<calm|under_fire>"` -> `[shots,
//! hits]`, only cells with a shot), `ttfs_bins` (engagements with a shot, by the time to the first, bins of [`TTFS_BINS_MS`]),
//! `duels` (`won`, `lost`, `none`: engagements by their end, `won_ttfs` / `lost_ttfs`: `[engagements with a shot, ms summed]`);
//! `vs_camper`: the same `duels` for the engagements on the stand-in camper, and `shots` (`[shots, hits]`) at him; `split`: the shots
//! the bot's own numbers at the shot came with (`reflex::FireAim`, within [`AIM_MATCH_MS`]), by its aim error (`error_bins`) and by
//! how far its view was from where it meant to aim (`slack_bins`: what the gate let through), both in his chest's half-widths,
//! `[shots, hits]`, and the gate's bound summed (`bound_sum`, half-widths).

use super::*;
use crate::v026g::bots::reflex::FireAim;

/// His chest's half-width, metres (the gate's `CombatTuning::body_half_m`).
const HALF_M: f32 = 0.25;
/// The view's offset bins in half-widths (upper edges; the last takes the rest): the live table's (A221).
const OFF_EDGES: [f32; 4] = [1.0, 3.0, 10.0, 30.0];
const OFF_LABELS: [&str; 5] = ["on", "1-3", "3-10", "10-30", "30+"];
/// The distance bands, metres (upper edges; the last takes the rest).
const BAND_EDGES: [f32; 4] = [10.0, 20.0, 40.0, 65.0];
const BAND_LABELS: [&str; 5] = ["0-10", "10-20", "20-40", "40-65", "65+"];
/// A shot within this of the shooter's last one goes on its burst, ms (GUESS: two cycles of a 600-round-a-minute rifle and a
/// tick's slack).
const BURST_MS: u32 = 300;
/// The shooter is under fire when an enemy's shot was at him, or hit him, within this, ms (GUESS: a second).
const UNDER_FIRE_MS: u32 = 1_000;
/// The time to the first shot, bins (upper edges, ms; the last takes the rest).
const TTFS_BINS_MS: [u32; 8] = [250, 500, 750, 1_000, 1_500, 2_000, 3_000, 5_000];

const CELLS: usize = OFF_LABELS.len() * 2 * BAND_LABELS.len() * 2;
/// A shot takes the bot's numbers of its last shot when they are this near in time, ms (a tick and the bench's slack).
const AIM_MATCH_MS: u32 = 100;

/// An engagement under way: its target, since when, its first shot at him, and since when the input has had no target, while
/// that is a moment (within [`LOST_MS`]: the reflex's own "lost a moment", `reflex::LOST_MS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Engagement {
    target: Place,
    since: u32,
    first_shot: Option<u32>,
    lost_since: Option<u32>,
}

/// A target dropped for at most this long keeps its engagement, ms.
const LOST_MS: u32 = crate::v026g::bots::reflex::LOST_MS;

/// How an engagement ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Won,
    Lost,
    None,
}

#[derive(Default)]
struct TeamCounts {
    /// By cell ([`FireGate::cell`]): shots, hits.
    cells: Vec<[u32; 2]>,
    ttfs: [u32; TTFS_BINS_MS.len() + 1],
    duels: Duels,
    /// The engagements on the stand-in camper, and the shots at him (shots, hits).
    camper: Duels,
    camper_shots: [u32; 2],
    split: Split,
}

/// The shots with the bot's own numbers: by its aim error and by the gate's slack (half-widths bins of [`OFF_EDGES`]), shots and hits;
/// the gate's bound summed (half-widths).
#[derive(Clone, Copy, Default)]
struct Split {
    error: [[u32; 2]; OFF_LABELS.len()],
    slack: [[u32; 2]; OFF_LABELS.len()],
    bound_sum: f64,
}

impl Split {
    fn add(&mut self, aim: FireAim, half_deg: f32, hit: bool) {
        let bin = |halves: f32| OFF_EDGES.iter().position(|&edge| halves <= edge).unwrap_or(OFF_EDGES.len());
        for (cells, deg) in [(&mut self.error, aim.error_deg), (&mut self.slack, aim.off_deg)] {
            let cell = &mut cells[bin(deg / half_deg)];
            cell[0] += 1;
            cell[1] += u32::from(hit);
        }
        self.bound_sum += f64::from(aim.within_deg / half_deg);
    }

    fn shots(&self) -> u32 {
        self.error.iter().map(|c| c[0]).sum()
    }
}

/// Engagements by their end (won, lost, none), and the time to the first shot of those won and lost (with a shot, ms summed).
#[derive(Clone, Copy, Default)]
struct Duels {
    ends: [u32; 3],
    won_ttfs: [u64; 2],
    lost_ttfs: [u64; 2],
}

impl Duels {
    fn end(&mut self, end: End, ttfs_ms: Option<u64>) {
        self.ends[end as usize] += 1;
        match (end, ttfs_ms) {
            (End::Won, Some(ms)) => self.won_ttfs = [self.won_ttfs[0] + 1, self.won_ttfs[1] + ms],
            (End::Lost, Some(ms)) => self.lost_ttfs = [self.lost_ttfs[0] + 1, self.lost_ttfs[1] + ms],
            _ => {}
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "won": self.ends[0], "lost": self.ends[1], "none": self.ends[2], "won_ttfs": self.won_ttfs, "lost_ttfs": self.lost_ttfs,
        })
    }

    fn text(&self) -> String {
        let mean = |s: [u64; 2]| if s[0] == 0 { 0.0 } else { s[1] as f64 / s[0] as f64 };
        format!(
            "won {} lost {} none {}, time to the first shot won {:.0} ms lost {:.0} ms",
            self.ends[0],
            self.ends[1],
            self.ends[2],
            mean(self.won_ttfs),
            mean(self.lost_ttfs)
        )
    }
}

pub(super) struct FireGate {
    teams: [TeamCounts; 2],
    /// By place: its engagement under way, its last shot's tick, the last tick an enemy's shot was at it.
    engaged: Vec<Option<Engagement>>,
    last_shot: Vec<Option<u32>>,
    shot_at: Vec<Option<u32>>,
    /// By place: the bot's own numbers at its last shot ([`Topic::read`]).
    aims: Vec<Option<FireAim>>,
    /// The run's kills read so far.
    kills_read: usize,
}

impl FireGate {
    pub(super) fn new(n: usize) -> Self {
        let team = || TeamCounts { cells: vec![[0; 2]; CELLS], ..TeamCounts::default() };
        Self {
            teams: [team(), team()],
            engaged: vec![None; n],
            last_shot: vec![None; n],
            shot_at: vec![None; n],
            aims: vec![None; n],
            kills_read: 0,
        }
    }

    /// A shot's cell: the view's offset bin, first or in a burst, the distance band, under fire or not.
    fn cell(off_halves: f32, burst: bool, distance: f32, under_fire: bool) -> usize {
        let off = OFF_EDGES.iter().position(|&edge| off_halves <= edge).unwrap_or(OFF_EDGES.len());
        let band = BAND_EDGES.iter().position(|&edge| distance < edge).unwrap_or(BAND_EDGES.len());
        ((off * 2 + usize::from(burst)) * BAND_LABELS.len() + band) * 2 + usize::from(under_fire)
    }

    fn label(cell: usize) -> String {
        let fire = cell % 2;
        let band = (cell / 2) % BAND_LABELS.len();
        let burst = (cell / 2 / BAND_LABELS.len()) % 2;
        let off = cell / 2 / BAND_LABELS.len() / 2;
        format!(
            "{}:{}:{}:{}",
            OFF_LABELS[off],
            if burst == 1 { "burst" } else { "first" },
            BAND_LABELS[band],
            if fire == 1 { "under_fire" } else { "calm" }
        )
    }

    /// Ends `place`'s engagement under way, if any (`camper`: the stand-in camper's place).
    fn close(&mut self, place: Place, team: usize, end: End, camper: Option<Place>) {
        let Some(e) = self.engaged.get_mut(place).and_then(Option::take) else {
            return;
        };
        let ttfs = e.first_shot.map(|first| u64::from(first.saturating_sub(e.since)));
        let counts = &mut self.teams[team];
        counts.duels.end(end, ttfs);
        if camper == Some(e.target) {
            counts.camper.end(end, ttfs);
        }
    }
}

impl Topic for FireGate {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// Each bot's own numbers at its last shot (read after the game step, before [`Topic::begin`] takes the step's shots).
    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(slot) = self.aims.get_mut(place) {
            *slot = bot.and_then(|bot| bot.reflex.fire_aim());
        }
    }

    /// The tick's shots and kills, and each body's target. `game.tick` and the facts' ticks are the match time in ms.
    fn begin(&mut self, run: &Run, game: &mut Game) {
        let tick = game.tick;
        let camper = run.camper;
        let bot = |p: Place| run.is_bot.get(p).copied().unwrap_or(false) && camper != Some(p);
        let team_of = |p: Place| usize::from(game.bodies[p].team).min(1);
        // The shots at a body this tick: under fire from now on.
        for fact in &game.shot_facts {
            for at in fact.target.map(|t| t.0).into_iter().chain(fact.hit) {
                if at < self.shot_at.len() && game.bodies[at].team != game.bodies[fact.shooter].team {
                    self.shot_at[at] = Some(tick);
                }
            }
        }
        for fact in game.shot_facts.iter().filter(|f| bot(f.shooter)) {
            let s = fact.shooter;
            let burst = self.last_shot[s].is_some_and(|last| tick.saturating_sub(last) <= BURST_MS);
            self.last_shot[s] = Some(tick);
            let (Some((target, distance, _)), Some(rise)) = (fact.target, fact.rise) else {
                continue;
            };
            let half_deg = HALF_M.atan2(distance.max(0.5)).to_degrees();
            // Under fire: an enemy's shot at him in the second before this tick (this tick's own shots at him included).
            let under_fire = self.shot_at[s].is_some_and(|at| tick.saturating_sub(at) <= UNDER_FIRE_MS);
            let cell = Self::cell(rise.off_deg / half_deg, burst, distance, under_fire);
            let hit = u32::from(fact.hit == Some(target));
            let counts = &mut self.teams[team_of(s)];
            counts.cells[cell][0] += 1;
            counts.cells[cell][1] += hit;
            if camper == Some(target) {
                counts.camper_shots[0] += 1;
                counts.camper_shots[1] += hit;
            }
            if let Some(aim) = self.aims[s].filter(|aim| aim.at.abs_diff(fact.tick) <= AIM_MATCH_MS) {
                counts.split.add(aim, half_deg, hit == 1);
            }
            if let Some(e) = self.engaged[s].as_mut().filter(|e| e.target == target && e.first_shot.is_none()) {
                e.first_shot = Some(tick);
                let ms = tick.saturating_sub(e.since);
                let bin = TTFS_BINS_MS.iter().position(|&edge| ms < edge).unwrap_or(TTFS_BINS_MS.len());
                self.teams[team_of(s)].ttfs[bin] += 1;
            }
        }
        // The kills since the last tick (the run's list: the bench drains the game's): a duel won by the killer, lost by the killed.
        let new: Vec<(u32, Place, Place)> = run.kills.get(self.kills_read..).map(<[_]>::to_vec).unwrap_or_default();
        self.kills_read = run.kills.len();
        for (_, killer, killed) in new {
            if killer < self.engaged.len() && self.engaged[killer].is_some_and(|e| e.target == killed) {
                self.close(killer, team_of(killer), End::Won, camper);
            }
            if killed < self.engaged.len() {
                let lost = self.engaged[killed].is_some_and(|e| e.target == killer);
                self.close(killed, team_of(killed), if lost { End::Lost } else { End::None }, camper);
            }
        }
        // Each bot's target as its input was made: the same keeps its engagement (also after a moment with none), another opens one;
        // none for longer than a moment ends it.
        for (place, body) in game.bodies.iter().enumerate().take(self.engaged.len()) {
            let team = usize::from(body.team).min(1);
            let target = body
                .target
                .filter(|&t| bot(place) && body.alive && game.bodies.get(t).is_some_and(|b| b.alive && b.team != body.team));
            match (self.engaged[place], target) {
                (Some(e), Some(t)) if e.target == t => self.engaged[place] = Some(Engagement { lost_since: None, ..e }),
                (Some(e), None) if body.alive => match e.lost_since {
                    None => self.engaged[place] = Some(Engagement { lost_since: Some(tick), ..e }),
                    Some(at) if tick.saturating_sub(at) > LOST_MS => self.close(place, team, End::None, camper),
                    Some(_) => {}
                },
                (_, target) => {
                    self.close(place, team, End::None, camper);
                    self.engaged[place] = target.map(|target| Engagement { target, since: tick, first_shot: None, lost_since: None });
                }
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        if self.teams.iter().all(|t| t.cells.iter().all(|c| c[0] == 0) && t.duels.ends.iter().all(|&d| d == 0)) {
            return Vec::new();
        }
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for (team, counts) in self.teams.iter().enumerate() {
            let mut cells = serde_json::Map::new();
            for (i, c) in counts.cells.iter().enumerate().filter(|(_, c)| c[0] > 0) {
                cells.insert(Self::label(i), serde_json::json!(c));
            }
            json.insert(
                format!("team{team}"),
                serde_json::json!({
                    "cells": cells, "ttfs_bins": counts.ttfs, "duels": counts.duels.json(),
                    "vs_camper": { "duels": counts.camper.json(), "shots": counts.camper_shots },
                    "split": { "error_bins": counts.split.error, "slack_bins": counts.split.slack, "bound_sum": counts.split.bound_sum },
                }),
            );
            // The line: shots and hit rate by the view's offset bin, first shots and bursts, calm and under fire; the duels.
            let sum = |keep: &dyn Fn(usize) -> bool| {
                counts.cells.iter().enumerate().filter(|(i, _)| keep(*i)).fold([0u32; 2], |a, (_, c)| [a[0] + c[0], a[1] + c[1]])
            };
            let rate = |c: [u32; 2]| if c[0] == 0 { 0.0 } else { 100.0 * f64::from(c[1]) / f64::from(c[0]) };
            let per_off: Vec<String> = (0..OFF_LABELS.len())
                .map(|o| {
                    let c = sum(&|i| i / 2 / BAND_LABELS.len() / 2 == o);
                    format!("{} {}/{} {:.1} %", OFF_LABELS[o], c[1], c[0], rate(c))
                })
                .collect();
            let first = sum(&|i| (i / 2 / BAND_LABELS.len()).is_multiple_of(2));
            let burst = sum(&|i| !(i / 2 / BAND_LABELS.len()).is_multiple_of(2));
            let calm = sum(&|i| i.is_multiple_of(2));
            let fire = sum(&|i| !i.is_multiple_of(2));
            let bins = |cells: &[[u32; 2]; OFF_LABELS.len()]| {
                cells.iter().zip(OFF_LABELS).map(|(c, l)| format!("{l} {}/{} {:.1} %", c[1], c[0], rate(*c))).collect::<Vec<_>>().join(", ")
            };
            let split = counts.split;
            let split_text = format!(
                "of {} shots with the bot's numbers: its aim error {}; its view off where it meant (the gate's slack) {}; mean bound {:.1}",
                split.shots(),
                bins(&split.error),
                bins(&split.slack),
                if split.shots() == 0 { 0.0 } else { split.bound_sum / f64::from(split.shots()) },
            );
            text.push(format!(
                "team {team}: view off (chest half-widths) {}; first {}/{} {:.1} %, burst {}/{} {:.1} %; calm {}/{} {:.1} %, under fire {}/{} {:.1} %; duels {}; vs the stand-in camper: shots {}/{} {:.1} %, duels {}; {}",
                per_off.join(", "),
                first[1],
                first[0],
                rate(first),
                burst[1],
                burst[0],
                rate(burst),
                calm[1],
                calm[0],
                rate(calm),
                fire[1],
                fire[0],
                rate(fire),
                counts.duels.text(),
                counts.camper_shots[1],
                counts.camper_shots[0],
                rate(counts.camper_shots),
                counts.camper.text(),
                split_text,
            ));
        }
        vec![("fire_gate", Part::new(serde_json::json!({ "fire_gate": json }), format!("owner-test fire gate: {}", text.join("; "))))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_finds_its_cell_and_the_cell_its_label() {
        let at = FireGate::cell(0.5, false, 5.0, false);
        assert_eq!(FireGate::label(at), "on:first:0-10:calm");
        let at = FireGate::cell(12.0, true, 50.0, true);
        assert_eq!(FireGate::label(at), "10-30:burst:40-65:under_fire");
        let at = FireGate::cell(100.0, true, 200.0, true);
        assert_eq!(at, CELLS - 1);
        assert_eq!(FireGate::label(at), "30+:burst:65+:under_fire");
    }

    #[test]
    fn an_engagement_ends_won_lost_or_none_with_its_time_to_the_first_shot() {
        let mut gate = FireGate::new(4);
        // Ticks are the match time in ms: the first shot 600 ms after the target was taken.
        gate.engaged[0] = Some(Engagement { target: 2, since: 10_000, first_shot: Some(10_600), lost_since: None });
        gate.close(0, 0, End::Won, Some(2));
        gate.engaged[1] = Some(Engagement { target: 3, since: 10, first_shot: None, lost_since: None });
        gate.close(1, 0, End::Lost, Some(2));
        gate.close(1, 0, End::None, Some(2));
        let team = &gate.teams[0];
        assert_eq!(team.duels.ends, [1, 1, 0]);
        assert_eq!(team.duels.won_ttfs, [1, 600]);
        assert_eq!(team.duels.lost_ttfs, [0, 0]);
        // Only the engagement on the stand-in camper (place 2) counts apart.
        assert_eq!(team.camper.ends, [1, 0, 0]);
    }

    #[test]
    fn a_shot_with_the_bots_numbers_splits_into_its_aim_error_and_the_gates_slack() {
        let mut split = Split::default();
        // At 20 m a half-width is about 0.72 deg: an error of 2 deg is 1-3 half-widths, a slack of 3 deg 3-10.
        let half = HALF_M.atan2(20.0).to_degrees();
        split.add(FireAim { at: 0, distance: 20.0, off_deg: 3.0, within_deg: 4.0, error_deg: 2.0 }, half, true);
        assert_eq!(split.error[1], [1, 1]);
        assert_eq!(split.slack[2], [1, 1]);
        assert_eq!(split.shots(), 1);
        assert!((split.bound_sum - f64::from(4.0 / half)).abs() < 1e-6);
    }
}
