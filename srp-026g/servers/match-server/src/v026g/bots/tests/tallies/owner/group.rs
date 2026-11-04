//! [A297 `tdm.group`] The owner counts of group fights and trades, by the dead's team (test only): every kill of an enemy,
//! - how soon the dead's team answered: the killer's first hit by it (`answer` ms, the median and within 1 s / 3 s) and his
//!   death to it (traded within 1 s / 3 s);
//! - how many of the dead's teammates had the killer for target at the kill (alone 0, 1, 2+): the bots engaged together;
//! - a group-fed death: a teammate died within [`FED_M`] of it in the [`FED_MS`] before (the lane re-entered); and the
//!   coordinator's narrower count (the owner's "they shouldnt group feed"): a teammate died to the same killer within
//!   [`FED_M`] in the [`SAME_MS`] before;
//! - a death in file: a living teammate within [`FILE_M`] of it on the killer's bearing within [`FILE_DEG`];
//! - and the same three for the stand-in human apart: his kills (the bots he killed) and his deaths (bots on him).
//!
//! The switch's own counts (`group::Count`) follow: covers taken, held, and those not offered or with no spot, by reason.
//! Root cause and design: `traces-20261028/fx-team-a297-design.md`.

use super::*;
use crate::v026g::bots::group;

/// A teammate's death this near and this recent makes a death group-fed, m and ms.
const FED_M: f32 = 8.0;
const FED_MS: u32 = 5_000;
/// The same killer's kills this close make a death group-fed in the narrow count, ms.
const SAME_MS: u32 = 2_000;
/// A living teammate this near on the killer's bearing within this makes a death in file, m and degrees.
const FILE_M: f32 = 6.0;
const FILE_DEG: f32 = 12.0;
/// The answer windows, ms.
const FAST_MS: u32 = 1_000;
const SLOW_MS: u32 = 3_000;

pub(super) struct GroupCounts {
    /// The switch's counts when the match began (the counts are the thread's).
    start: [u64; group::COUNTS.len()],
    human: Option<Place>,
    /// By place: its bot's target at the latest tick read.
    targets: Vec<Option<Place>>,
    /// Kills seen so far in the game's log, and the last of them (the bench drains the log).
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    /// By kill: (tick, killer, victim, the victim's mates targeting the killer, the killer's team targeting the victim).
    on_killer: Vec<(u32, Place, Place, u32, u32)>,
    /// By place: its bot's team and play at the latest tick read ("Cover" for a Trade on a mate, the switch's cover).
    doing: Vec<Option<(u8, String)>>,
    /// By team: bot-ticks in a cover; bot-ticks with the stand-in human for target.
    cover_ticks: [u64; 2],
    on_human_ticks: [u64; 2],
    /// By the killer's team: the stand-in human's deaths by the killer's play.
    human_killed_by: [std::collections::BTreeMap<String, u32>; 2],
}

impl Default for GroupCounts {
    fn default() -> Self {
        Self {
            start: group::counted(),
            human: None,
            targets: Vec::new(),
            kills_seen: 0,
            last_kill: None,
            on_killer: Vec::new(),
            doing: Vec::new(),
            cover_ticks: [0; 2],
            on_human_ticks: [0; 2],
            human_killed_by: Default::default(),
        }
    }
}

impl GroupCounts {
    pub(super) fn set_human(&mut self, human: Option<Place>) {
        self.human = human;
    }
}

impl Topic for GroupCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if self.targets.len() <= place {
            self.targets.resize(place + 1, None);
        }
        self.targets[place] = bot.and_then(|b| b.reflex.target());
        if self.doing.len() <= place {
            self.doing.resize(place + 1, None);
        }
        self.doing[place] = bot.map(|b| {
            let play = match b.assignment.as_ref() {
                Some(a) if a.kind == crate::v026g::bots::plays::PlayKind::Trade && matches!(a.target, crate::v026g::bots::plays::TargetRef::Place(_)) => {
                    "Cover".to_string()
                }
                Some(a) => format!("{:?}", a.kind),
                None => "none".to_string(),
            };
            (b.team & 1, play)
        });
        if let Some(Some((team, play))) = self.doing.get(place) {
            let t = usize::from(*team);
            self.cover_ticks[t] += u64::from(play == "Cover");
            self.on_human_ticks[t] += u64::from(self.human.is_some() && self.targets[place] == self.human);
        }
    }

    fn end(&mut self, _run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        for (t, killer, victim) in new {
            let team_of = |p: Place| game.bodies.get(p).map(|b| b.team);
            let targeting = |team: Option<u8>, skip: Place, at: Place| {
                (0..game.bodies.len())
                    .filter(|&p| p != skip && team_of(p) == team && game.bodies[p].alive)
                    .filter(|&p| self.targets.get(p).copied().flatten() == Some(at))
                    .count() as u32
            };
            let on_killer = targeting(team_of(victim), victim, killer);
            // The killer counts himself: he had the victim for target, or he would not have killed him.
            let on_victim = targeting(team_of(killer), killer, victim) + 1;
            if self.human == Some(victim)
                && let Some(Some((team, play))) = self.doing.get(killer)
            {
                *self.human_killed_by[usize::from(*team)].entry(play.clone()).or_default() += 1;
            }
            self.on_killer.push((t, killer, victim, on_killer, on_victim));
        }
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("group", self.part(run, game))]
    }
}

/// One side's counts (by the dead's team, or the stand-in human's kills or deaths).
#[derive(Clone, Default, Debug)]
struct Side {
    deaths: u32,
    answers: Vec<u32>,
    hit_fast: u32,
    hit_slow: u32,
    traded_fast: u32,
    traded_slow: u32,
    on_killer: [u32; 3],
    /// The killer's side targeting the dead at the kill, the killer counted: 1, 2, 3+.
    attackers: [u32; 3],
    fed: u32,
    fed_same: u32,
    file: u32,
}

impl Side {
    fn json(&self) -> serde_json::Value {
        let mut answers = self.answers.clone();
        answers.sort_unstable();
        serde_json::json!({
            "deaths": self.deaths,
            "answer_median_ms": answers.get(answers.len() / 2),
            "answered": answers.len(),
            "hit_within_1s": self.hit_fast,
            "hit_within_3s": self.hit_slow,
            "traded_within_1s": self.traded_fast,
            "traded_within_3s": self.traded_slow,
            "mates_on_killer_0_1_2plus": self.on_killer,
            "attackers_1_2_3plus": self.attackers,
            "group_fed": self.fed,
            "group_fed_same_killer_2s": self.fed_same,
            "in_file": self.file,
        })
    }

    fn text(&self) -> String {
        let mut answers = self.answers.clone();
        answers.sort_unstable();
        format!(
            "deaths {} answered {} (median {} ms), hit 1 s {} 3 s {}, traded 1 s {} 3 s {}, mates on killer 0/1/2+ {:?}, attackers 1/2/3+ {:?}, group-fed {} (same killer 2 s {}), in file {}",
            self.deaths,
            answers.len(),
            answers.get(answers.len() / 2).map_or("-".to_string(), u32::to_string),
            self.hit_fast,
            self.hit_slow,
            self.traded_fast,
            self.traded_slow,
            self.on_killer,
            self.attackers,
            self.fed,
            self.fed_same,
            self.file
        )
    }
}

fn bearing_deg(from: Vec3, to: Vec3) -> f32 {
    (to.z - from.z).atan2(to.x - from.x).to_degrees()
}

impl GroupCounts {
    fn feet(run: &Run, place: Place, tick: u32) -> Option<Vec3> {
        let recs = run.recs.get(place)?;
        recs.get(run.index(tick).min(recs.len().checked_sub(1)?)).map(|r| r.feet)
    }

    fn alive(run: &Run, place: Place, tick: u32) -> bool {
        run.recs.get(place).and_then(|recs| recs.get(run.index(tick).min(recs.len().saturating_sub(1)))).is_some_and(|r| r.alive)
    }

    fn part(&self, run: &Run, game: &Game) -> Part {
        let team = |place: Place| game.bodies.get(place).map_or(0, |b| b.team & 1);
        let mut by_team = [Side::default(), Side::default()];
        let (mut human_kills, mut human_deaths) = (Side::default(), Side::default());
        for &(t, killer, victim, on, attackers) in &self.on_killer {
            if team(killer) == team(victim) {
                continue;
            }
            let vt = team(victim);
            let mut s = Side { deaths: 1, ..Side::default() };
            // The dead's team's answer: its first hit on the killer, his death to it.
            let answer = run
                .hits
                .iter()
                .filter(|&&(t2, shooter, target)| t2 >= t && target == killer && team(shooter) == vt && shooter != victim)
                .map(|&(t2, ..)| t2 - t)
                .min();
            if let Some(ms) = answer.filter(|&ms| ms <= 10_000) {
                s.answers.push(ms);
                s.hit_fast += u32::from(ms <= FAST_MS);
                s.hit_slow += u32::from(ms <= SLOW_MS);
            }
            let traded = run
                .kills
                .iter()
                .filter(|&&(t2, k2, v2)| t2 >= t && v2 == killer && team(k2) == vt)
                .map(|&(t2, ..)| t2 - t)
                .min();
            if let Some(ms) = traded {
                s.traded_fast += u32::from(ms <= FAST_MS);
                s.traded_slow += u32::from(ms <= SLOW_MS);
            }
            s.on_killer[(on as usize).min(2)] += 1;
            s.attackers[(attackers as usize).min(3) - 1] += 1;
            if let Some(at) = Self::feet(run, victim, t) {
                s.fed = u32::from(run.kills.iter().any(|&(t2, _, v2)| {
                    v2 != victim && team(v2) == vt && t2 < t && t - t2 <= FED_MS && Self::feet(run, v2, t2).is_some_and(|f| f.distance_xz(at) <= FED_M)
                }));
                s.fed_same = u32::from(run.kills.iter().any(|&(t2, k2, v2)| {
                    k2 == killer && v2 != victim && team(v2) == vt && t2 < t && t - t2 <= SAME_MS && Self::feet(run, v2, t2).is_some_and(|f| f.distance_xz(at) <= FED_M)
                }));
                if let Some(k) = Self::feet(run, killer, t) {
                    let b = bearing_deg(k, at);
                    s.file = u32::from((0..run.recs.len()).any(|p| {
                        p != victim
                            && team(p) == vt
                            && Self::alive(run, p, t)
                            && Self::feet(run, p, t).is_some_and(|f| {
                                let d = (bearing_deg(k, f) - b + 540.0).rem_euclid(360.0) - 180.0;
                                f.distance_xz(at) <= FILE_M && d.abs() <= FILE_DEG
                            })
                    }));
                }
            }
            let add = |into: &mut Side| {
                into.deaths += s.deaths;
                into.answers.extend(&s.answers);
                into.hit_fast += s.hit_fast;
                into.hit_slow += s.hit_slow;
                into.traded_fast += s.traded_fast;
                into.traded_slow += s.traded_slow;
                for i in 0..3 {
                    into.on_killer[i] += s.on_killer[i];
                    into.attackers[i] += s.attackers[i];
                }
                into.fed += s.fed;
                into.fed_same += s.fed_same;
                into.file += s.file;
            };
            add(&mut by_team[usize::from(vt)]);
            if self.human == Some(killer) {
                add(&mut human_kills);
            }
            if self.human == Some(victim) {
                add(&mut human_deaths);
            }
        }
        let now = group::counted();
        let counted: Vec<u64> = now.iter().zip(self.start).map(|(n, s)| n - s).collect();
        let switch: serde_json::Map<String, serde_json::Value> = group::COUNTS
            .iter()
            .zip(&counted)
            .map(|(c, n)| (format!("{c:?}"), serde_json::json!(n)))
            .collect();
        let json = serde_json::json!({
            "group": {
                "by_dead_team": [by_team[0].json(), by_team[1].json()],
                "human_kills": human_kills.json(),
                "human_deaths": human_deaths.json(),
                "switch": switch,
                "cover_ticks": self.cover_ticks,
                "on_human_ticks": self.on_human_ticks,
                "human_killed_by": [&self.human_killed_by[0], &self.human_killed_by[1]],
            }
        });
        let text = format!(
            "owner-test group (A297, by the dead's team): team 0: {}; team 1: {}; the stand-in human's kills: {}; his deaths: {}; switch {:?}; by team: cover bot-ticks {:?}, bot-ticks on the human {:?}, the human killed by play {:?} / {:?}",
            by_team[0].text(),
            by_team[1].text(),
            human_kills.text(),
            human_deaths.text(),
            counted,
            self.cover_ticks,
            self.on_human_ticks,
            self.human_killed_by[0],
            self.human_killed_by[1]
        );
        Part::new(json, text)
    }
}
