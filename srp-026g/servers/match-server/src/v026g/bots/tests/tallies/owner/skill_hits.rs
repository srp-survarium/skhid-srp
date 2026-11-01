//! [A242] The owner counts of the bots' hit rate in the terms of the owner's bar (quiz 2026-10-30, `a182-sniper/AUTO-GUN-HIT-GAP.md`:
//! per shot on the target's line, against humans; for the automatic guns, rifles + SMGs at 0-65 m, skill 0 the live median 10.2 %
//! and skill 1 18-20 % since 2026-11-01, a straight line between; the other classes get their own bars, A245), to read the hit rate
//! along the skill scale: each bot's shot at its target whose line is within [`ON_DEG`] of his chest (the humans' view test, as
//! `gun_shots`), by who the target is (the stand-in human, `SRP_BENCH_HUMAN`; the stand-in camper; a bot), the gun's class (the game
//! data's, `guns::WeaponClass`) and the live humans' distance band (`hit_marks.py`), with the hits among them (a shot with a bullet
//! on an enemy), and its bullets fired and on an enemy: the game's counters count a shotgun's pellets (live, 10 to a press:
//! `shotgun_unit.py`), so a shotgun's live rate is the pellets'; and the shooters' skill at their shots, summed, so a run says what
//! skill its bots had. Bots only: the stand-ins' own shots are not counted. Counts only: nothing decides by them.
//!
//! Key of the `owner` object `skill_hits`: `"team<t>"` -> `cells` (`"<human|camper|bot>:<class>:<band>"` -> `[shots, hits, bullets,
//! bullets on an enemy]`, only cells with a shot; the class `WeaponClass::name`) and `skill` (`[shots, the shooters' skill
//! summed]`).

use super::*;
use crate::v026g::bots::guns::WeaponClass;

/// The shot's line is on the target when within this of his chest, degrees (the humans' test on the view, `gun_shots`).
const ON_DEG: f32 = 10.0;
/// The live humans' distance bands, metres (upper edges; the last takes the rest): 0-20, 20-40, 40-65, 65-80, 80-105, 105+.
const BAND_EDGES: [f32; 5] = [20.0, 40.0, 65.0, 80.0, 105.0];
const BAND_LABELS: [&str; 6] = ["0-20", "20-40", "40-65", "65-80", "80-105", "105+"];
const TARGETS: [&str; 3] = ["human", "camper", "bot"];
const CLASSES: [WeaponClass; 7] = [
    WeaponClass::Pistol,
    WeaponClass::Smg,
    WeaponClass::Rifle,
    WeaponClass::MachineGun,
    WeaponClass::Shotgun,
    WeaponClass::Marksman,
    WeaponClass::Bolt,
];
const CELLS: usize = TARGETS.len() * CLASSES.len() * BAND_LABELS.len();

/// Who a shot was at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Human,
    Camper,
    Bot,
}

#[derive(Default)]
struct TeamCounts {
    /// By cell ([`SkillHits::cell`]): shots, hits, bullets (a shotgun's pellets), bullets on an enemy.
    cells: Vec<[u32; 4]>,
    /// The shots counted and their shooters' skill summed.
    skill: (u32, f64),
}

pub(super) struct SkillHits {
    teams: [TeamCounts; 2],
    /// By place: the bot's skill this tick ([`Topic::read`]).
    skills: Vec<Option<f32>>,
    /// The stand-in human's place (`SRP_BENCH_HUMAN`), read at the first tick.
    human: Option<Option<Place>>,
}

impl SkillHits {
    pub(super) fn new(n: usize) -> Self {
        let team = || TeamCounts { cells: vec![[0; 4]; CELLS], ..TeamCounts::default() };
        Self { teams: [team(), team()], skills: vec![None; n], human: None }
    }

    /// The distance band of `distance` metres.
    fn band(distance: f32) -> usize {
        BAND_EDGES.iter().position(|&edge| distance < edge).unwrap_or(BAND_EDGES.len())
    }

    /// A shot's cell: who it was at, the gun's class, the distance band.
    fn cell(target: Target, class: WeaponClass, band: usize) -> usize {
        let class = CLASSES.iter().position(|&c| c == class).unwrap_or(0);
        ((target as usize) * CLASSES.len() + class) * BAND_LABELS.len() + band
    }

    fn label(cell: usize) -> String {
        let band = cell % BAND_LABELS.len();
        let class = (cell / BAND_LABELS.len()) % CLASSES.len();
        let target = cell / BAND_LABELS.len() / CLASSES.len();
        format!("{}:{}:{}", TARGETS[target], CLASSES[class].name(), BAND_LABELS[band])
    }
}

impl Topic for SkillHits {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// Each bot's skill (the one it took at its last spawn).
    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(slot) = self.skills.get_mut(place) {
            *slot = bot.map(|bot| bot.skill);
        }
    }

    /// The tick's shots.
    fn begin(&mut self, run: &Run, game: &mut Game) {
        let human = *self
            .human
            .get_or_insert_with(|| crate::v026g::bots::tests::bench_settings().var("SRP_BENCH_HUMAN").ok().and_then(|p| p.parse().ok()));
        for fact in &game.shot_facts {
            let s = fact.shooter;
            if !run.is_bot.get(s).copied().unwrap_or(false) || run.camper == Some(s) || human == Some(s) {
                continue;
            }
            let Some((target, distance, _)) = fact.target.filter(|t| t.2 <= ON_DEG) else { continue };
            let who = if human == Some(target) {
                Target::Human
            } else if run.camper == Some(target) {
                Target::Camper
            } else {
                Target::Bot
            };
            let body = &game.bodies[s];
            let counts = &mut self.teams[usize::from(body.team).min(1)];
            let cell = &mut counts.cells[Self::cell(who, body.gun.class, Self::band(distance))];
            cell[0] += 1;
            cell[1] += u32::from(fact.landed.on_enemy > 0);
            cell[2] += u32::from(fact.landed.fired);
            cell[3] += u32::from(fact.landed.on_enemy);
            if let Some(skill) = self.skills.get(s).copied().flatten() {
                counts.skill = (counts.skill.0 + 1, counts.skill.1 + f64::from(skill));
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        if self.teams.iter().all(|t| t.cells.iter().all(|c| c[0] == 0)) {
            return Vec::new();
        }
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        let rate = |c: [u32; 2]| if c[0] == 0 { 0.0 } else { 100.0 * f64::from(c[1]) / f64::from(c[0]) };
        for (team, counts) in self.teams.iter().enumerate() {
            let mut cells = serde_json::Map::new();
            for (i, c) in counts.cells.iter().enumerate().filter(|(_, c)| c[0] > 0) {
                cells.insert(Self::label(i), serde_json::json!(c));
            }
            json.insert(format!("team{team}"), serde_json::json!({ "cells": cells, "skill": [counts.skill.0, counts.skill.1] }));
            // The line: the bar's own cut (rifles + SMGs, 0-65 m) and its bands, then every other class over all bands (a shotgun's
            // pellets); at the stand-in human and at the bots.
            let sum = |target: Target, classes: &[WeaponClass], bands: &[usize]| {
                let mut c = [0u32; 2];
                for &class in classes {
                    for &band in bands {
                        let cell = counts.cells[Self::cell(target, class, band)];
                        c = if class == WeaponClass::Shotgun { [c[0] + cell[2], c[1] + cell[3]] } else { [c[0] + cell[0], c[1] + cell[1]] };
                    }
                }
                c
            };
            let automatic = [WeaponClass::Rifle, WeaponClass::Smg];
            let at = |target: Target| {
                let all = sum(target, &automatic, &[0, 1, 2]);
                let bands: Vec<String> =
                    (0..3).map(|b| format!("{} {:.1} %", BAND_LABELS[b], rate(sum(target, &automatic, &[b])))).collect();
                let others: Vec<String> = CLASSES
                    .iter()
                    .filter(|&&c| !automatic.contains(&c))
                    .map(|&c| {
                        let s = sum(target, &[c], &[0, 1, 2, 3, 4, 5]);
                        format!("{} {}/{}", c.name(), s[1], s[0])
                    })
                    .collect();
                format!("rifles + SMGs 0-65 m {}/{} {:.1} % ({}), others {}", all[1], all[0], rate(all), bands.join(", "), others.join(", "))
            };
            let skill = if counts.skill.0 == 0 { 0.0 } else { counts.skill.1 / f64::from(counts.skill.0) };
            text.push(format!(
                "team {team} (skill {skill:.2}): at the stand-in human {}; at the camper {}; at bots {}",
                at(Target::Human),
                at(Target::Camper),
                at(Target::Bot),
            ));
        }
        vec![("skill_hits", Part::new(serde_json::json!({ "skill_hits": json }), format!("owner-test skill hits: {}", text.join("; "))))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_finds_its_cell_and_the_cell_its_label() {
        let at = |target, class, distance| SkillHits::label(SkillHits::cell(target, class, SkillHits::band(distance)));
        assert_eq!(at(Target::Human, WeaponClass::Rifle, 5.0), "human:rifle:0-20");
        assert_eq!(at(Target::Camper, WeaponClass::Shotgun, 30.0), "camper:shotgun:20-40");
        assert_eq!(at(Target::Bot, WeaponClass::Smg, 64.9), "bot:smg:40-65");
        assert_eq!(at(Target::Bot, WeaponClass::Bolt, 90.0), "bot:bolt:80-105");
        let last = SkillHits::cell(Target::Bot, WeaponClass::Bolt, SkillHits::band(200.0));
        assert_eq!(last, CELLS - 1);
        assert_eq!(SkillHits::label(last), "bot:bolt:105+");
    }
}
