//! The owner counts of the outranged rule (`CombatTuning::outranged_cover`) and of the sniper fights it belongs to, by team:
//! the sightings of a sniper beyond the weapon's band, those with him facing the bot, the ticks it stood in the open in them,
//! the breaks of sight begun and the ticks a carrier kept its way instead. And what the snipers did: the kills by a sniper
//! (the killer's weapon class when he killed), the deaths to one, and of those the ones within [`AFTER_MS`] of a sighting
//! (the bot had been outranged just before), and the deaths to the stand-in camper, of them those within [`AT_OBJECTIVE_M`]
//! of an objective. The sensed snipers' spots (`ThreatSpots::sniper_stats`, #836 and #25b's area memory) and the objective
//! ways weighed by them (`threat::sniper_way_tally`). What it cost or gave is read from the pair's score; nothing here decides.

use super::*;
use crate::v026g::bots::reflex::OutrangedStats;
use crate::v026g::bots::tests::Lying;

/// A death this soon after a sighting of the rule's case counts as after it, ms.
const AFTER_MS: u32 = 10_000;
/// Sightings this far apart are two episodes, ms.
const EPISODE_MS: u32 = 3_000;
/// A death to the stand-in camper this near an objective counts as at it, m (#25b's gate: a lying or carried battery, or a
/// container): the victims of a known camper were mostly on objective plays (`DESIGN-25-remembered-sniper.md`).
const AT_OBJECTIVE_M: f32 = 25.0;

#[derive(Clone, Copy, Debug, Default)]
struct Deaths {
    /// All deaths of the team's bots.
    all: u32,
    /// Deaths with a sighting of the rule's case within [`AFTER_MS`], by any killer.
    after_sighting: u32,
    /// Deaths to a sniper (the killer held a marksman or bolt rifle).
    to_sniper: u32,
    /// The same, within [`AFTER_MS`] of a sighting.
    to_sniper_after_sighting: u32,
    /// Kills by the team's snipers.
    sniper_kills: u32,
    /// Sighting episodes.
    episodes: u32,
    /// Deaths to the stand-in camper (`SRP_BENCH_CAMPER`), and of them within [`AT_OBJECTIVE_M`] of an objective.
    to_camper: u32,
    to_camper_at_objective: u32,
}

#[derive(Default)]
pub(super) struct OutrangedCounts {
    /// By team: the lives that ended.
    banked: [OutrangedStats; 2],
    /// By place: the team and the counts of the life under way.
    now: Vec<Option<(usize, OutrangedStats)>>,
    /// By place: the tick of the last sighting.
    last_sighting: Vec<Option<u32>>,
    deaths: [Deaths; 2],
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    /// By team: what the sensed snipers' spots did (`ThreatSpots::sniper_stats`).
    lanes: [crate::v026g::bots::threat::SniperLaneStats; 2],
    /// The objective ways weighed by them (`threat::sniper_way_tally`): the thread's counts when this match began, and
    /// the match's since, by team [weighed, scored down, metres].
    ways_from: Option<[[u64; 3]; 2]>,
    ways: [[u64; 3]; 2],
}

impl Topic for OutrangedCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, tick: u32) {
        if self.now.len() <= place {
            self.now.resize(place + 1, None);
            self.last_sighting.resize(place + 1, None);
        }
        let Some(bot) = bot else {
            return;
        };
        let (team, stats) = (usize::from(bot.team).min(1), bot.reflex.outranged_stats());
        // A reflex younger than the last reading is a new life: the last one is banked.
        let mut before = self.now[place].map(|(_, s)| s).unwrap_or_default();
        if let Some((last_team, last)) = self.now[place]
            && stats.total() < last.total()
        {
            self.banked[last_team].add(&last);
            before = OutrangedStats::default();
        }
        if stats.sightings > before.sightings {
            if self.last_sighting[place].is_none_or(|at| tick > at + EPISODE_MS) {
                self.deaths[team].episodes += 1;
            }
            self.last_sighting[place] = Some(tick);
        }
        self.now[place] = Some((team, stats));
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        for (team, lanes) in self.lanes.iter_mut().enumerate() {
            if let Some(state) = bots.teams.get(team) {
                *lanes = state.features.threat.spots.sniper_stats();
            }
        }
        let now = crate::v026g::bots::threat::sniper_way_tally::counted();
        let from = *self.ways_from.get_or_insert(now);
        for (t, team) in self.ways.iter_mut().enumerate() {
            for (k, n) in team.iter_mut().enumerate() {
                *n = now[t][k] - from[t][k];
            }
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new: Vec<_> = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        for (t, killer, victim) in new {
            let (Some(k), Some(v)) = (game.bodies.get(killer), game.bodies.get(victim)) else {
                continue;
            };
            let sniper = matches!(k.gun.class(), "marksman" | "bolt");
            let after = self
                .last_sighting
                .get(victim)
                .copied()
                .flatten()
                .is_some_and(|at| t >= at && t - at <= AFTER_MS);
            let (kt, vt) = (usize::from(k.team).min(1), usize::from(v.team).min(1));
            self.deaths[vt].all += 1;
            // The stand-in (`Run::camper`, `SRP_BENCH_CAMPER`); `Game::camper` is a body camping a container.
            if run.camper == Some(killer) {
                let objective = game
                    .batteries
                    .iter()
                    .filter_map(|lying| match *lying {
                        Lying::Free(at, _) => Some(at),
                        Lying::Carried(carrier) => game.bodies.get(carrier).map(|b| b.feet),
                        Lying::Home(_) => None,
                    })
                    .chain(game.containers.iter().map(|&(_, at)| at))
                    .any(|at| at.distance_xz(v.feet) <= AT_OBJECTIVE_M);
                self.deaths[vt].to_camper += 1;
                self.deaths[vt].to_camper_at_objective += u32::from(objective);
            }
            self.deaths[vt].after_sighting += u32::from(after);
            if sniper {
                self.deaths[kt].sniper_kills += 1;
                self.deaths[vt].to_sniper += 1;
                self.deaths[vt].to_sniper_after_sighting += u32::from(after);
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("outranged", self.part())]
    }
}

impl OutrangedCounts {
    fn totals(&self) -> [OutrangedStats; 2] {
        let mut all = self.banked;
        for &(team, stats) in self.now.iter().flatten() {
            all[team].add(&stats);
        }
        all
    }

    fn part(&self) -> Part {
        let all = self.totals();
        let d = &self.deaths;
        let one = |s: &OutrangedStats, d: &Deaths, l: &crate::v026g::bots::threat::SniperLaneStats, w: &[u64; 3]| {
            serde_json::json!({
                "lane_spots": l.made,
                "lane_made_by_seen_heard_told": l.made_by,
                "lane_held_ticks_within_past_15s": l.held_by_age,
                "lane_unfair": l.unfair,
                "ways_weighed": w[0],
                "ways_scored_down": w[1],
                "ways_m": w[2],
                "lane_moved": l.moved,
                "lane_casts": l.casts,
                "lane_regions": l.regions,
                "lane_held_ticks": l.held_ticks,
                "sightings": s.sightings,
                "facing": s.facing,
                "exposed": s.exposed,
                "hides": s.hides,
                "task_kept": s.task_kept,
                "episodes": d.episodes,
                "deaths": d.all,
                "deaths_after_sighting": d.after_sighting,
                "deaths_to_sniper": d.to_sniper,
                "deaths_to_sniper_after_sighting": d.to_sniper_after_sighting,
                "sniper_kills": d.sniper_kills,
                "deaths_to_camper": d.to_camper,
                "deaths_to_camper_at_objective": d.to_camper_at_objective,
            })
        };
        let json = serde_json::json!({
            "outranged_sightings": all[0].sightings + all[1].sightings,
            "outranged_hides": all[0].hides + all[1].hides,
            "outranged": { "by_team": { "0": one(&all[0], &d[0], &self.lanes[0], &self.ways[0]), "1": one(&all[1], &d[1], &self.lanes[1], &self.ways[1]) } },
        });
        let text = format!(
            "owner-test outranged: a sniper seen beyond the weapon's band {} ticks in {} episodes (ticks facing it {}, in the open {}), breaks of sight begun {}, a carrier kept its way {} ticks; kills by snipers {} (team 0 {} / team 1 {}), deaths to a sniper {} of which within 10 s of a sighting {}; sniper spots made by [seen, heard, told] {:?}, held ticks [within 15 s of a sense, past it] {:?}, objective ways weighed {} of which scored down {} ({} m in his lanes); deaths to the stand-in camper {} of which within 25 m of an objective {}; FAIR (0) moved by a sense older than 15 s {}",
            all[0].sightings + all[1].sightings,
            d[0].episodes + d[1].episodes,
            all[0].facing + all[1].facing,
            all[0].exposed + all[1].exposed,
            all[0].hides + all[1].hides,
            all[0].task_kept + all[1].task_kept,
            d[0].sniper_kills + d[1].sniper_kills,
            d[0].sniper_kills,
            d[1].sniper_kills,
            d[0].to_sniper + d[1].to_sniper,
            d[0].to_sniper_after_sighting + d[1].to_sniper_after_sighting,
            [0, 1, 2].map(|k| self.lanes[0].made_by[k] + self.lanes[1].made_by[k]),
            [0, 1].map(|k| self.lanes[0].held_by_age[k] + self.lanes[1].held_by_age[k]),
            self.ways[0][0] + self.ways[1][0],
            self.ways[0][1] + self.ways[1][1],
            self.ways[0][2] + self.ways[1][2],
            d[0].to_camper + d[1].to_camper,
            d[0].to_camper_at_objective + d[1].to_camper_at_objective,
            self.lanes[0].unfair + self.lanes[1].unfair,
        );
        Part::new(json, text)
    }
}
