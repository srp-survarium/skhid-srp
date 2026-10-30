//! The owner counts of the steals (A175), by the thief's team: a battery taken out of a container (home, then carried) and the company it was taken in, to set
//! beside the live players' (`private/pvb-343-check/A175/A175-design-note-20261030.md`, 117 live batteries matches of 6+ places, 327 takes with positions):
//!
//! - **steals** a team made, by thief (a bot, or the stand-in) and by the play the thief ran (a Raid, a Sneak, a Collect, the rest);
//! - **the company at the take**: the teammates within 15 m and within 30 m of the thief (live: none 79 % and 63 %, one 18 % and 28 %), the enemies within 30 m of the
//!   container (true positions; live: none 44 %, one 35 %, two 18 %, three or more 4 %) and those of them the thief's memory held a track of; the group 10 s before the
//!   take (the thief and his teammates within 30 m of the take's place; live: the thief alone 56 %, with one mate 29 %, with two 8 %);
//! - **the battery's age in the container** at the take (live: median 100 s, p25 45, p75 225);
//! - **the chance**: the seconds a team's opponents' container held a battery, and of those the seconds a body of the team ran a Raid or a Sneak, so the share of the
//!   chance that is tried.
//!
//! What it cannot say: why a raid did not start (the plays' scores are not read here), and the bench's matches are 180 s, so the levels differ from the live ones; the
//! counts are for the arms of one bench against each other.

use super::*;
use crate::v026g::bots::tests::Lying;
use std::collections::VecDeque;

/// The company counted within this of the thief or of the take's place, m (the live census's radius), and the near ring.
const NEAR_M: f32 = 30.0;
const CLOSE_M: f32 = 15.0;
/// The snapshots of the bodies kept, one a second; the take's company "before" is the one this many seconds back.
const SNAP_EVERY: u32 = 20;
const BEFORE_S: usize = 10;

#[derive(Clone, Default)]
struct Team {
    steals: u64,
    /// By thief: a bot, the stand-in.
    by_thief: [u64; 2],
    by_play: BTreeMap<String, u64>,
    /// Teammates within 15 m of the thief: 0, 1, 2, 3 or more.
    mates_15: [u64; 4],
    /// Teammates within 30 m of the thief: 0, 1, 2, 3, 4 or more.
    mates_30: [u64; 5],
    /// The thief and his teammates within 30 m of the place 10 s before: 0 (not even the thief), 1, 2, 3, 4 or more.
    group_before: [u64; 5],
    /// Enemies within 30 m of the container: 0, 1, 2, 3 or more; and those a track of which the thief held.
    enemies_30: [u64; 4],
    enemies_known: [u64; 4],
    ages_s: Vec<f32>,
    /// The ticks the opponents' container held a battery, and of those the ticks a body of the team ran a Raid or a Sneak.
    chance: u64,
    tried: u64,
}

pub(super) struct StealCounts {
    teams: [Team; 2],
    /// The battery states of the last tick, and the tick each came home.
    last: Vec<Lying>,
    home_since: Vec<u32>,
    /// Whether each place is a bot, and whom its memory holds a track of.
    bot: Vec<bool>,
    known: Vec<Vec<Place>>,
    /// The bodies a second apart: alive, team and feet.
    snaps: VecDeque<Vec<(bool, u8, Vec3)>>,
}

impl StealCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            teams: Default::default(),
            last: Vec::new(),
            home_since: Vec::new(),
            bot: vec![false; n],
            known: vec![Vec::new(); n],
            snaps: VecDeque::new(),
        }
    }
}

fn bucket<const N: usize>(counts: &mut [u64; N], n: usize) {
    counts[n.min(N - 1)] += 1;
}

impl Topic for StealCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let (Some(is_bot), Some(known)) = (self.bot.get_mut(place), self.known.get_mut(place)) {
            *is_bot = bot.is_some();
            *known = bot.map(|b| b.memory.tracks().iter().map(|t| t.who).collect()).unwrap_or_default();
        }
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        if let (Some(is_bot), Some(known)) = (self.bot.get_mut(place), self.known.get_mut(place)) {
            *is_bot = now.is_some();
            *known = now.map(|n| n.tracks.iter().map(|&(who, ..)| who).collect()).unwrap_or_default();
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        let tick = game.tick;
        self.home_since.resize(game.batteries.len(), 0);
        // The chance: a battery in the opponents' container; and a body of the team on a Raid or a Sneak.
        let team_raids = |team: u8| {
            game.bodies.iter().enumerate().any(|(place, b)| {
                b.alive
                    && b.team == team
                    && run.recs.get(place).and_then(|r| r.last()).is_some_and(|rec| {
                        run.plays.get(usize::from(rec.play)).is_some_and(|name| name.contains("Raid") || name.contains("Sneak"))
                    })
            })
        };
        for team in 0..2u8 {
            let open = game
                .batteries
                .iter()
                .any(|l| matches!(*l, Lying::Home(c) if game.containers.get(usize::from(c)).is_some_and(|&(owner, _)| owner & 1 != team)));
            if open {
                self.teams[usize::from(team)].chance += 1;
                self.teams[usize::from(team)].tried += u64::from(team_raids(team));
            }
        }
        // The takes: home, then carried.
        for (battery, lying) in game.batteries.iter().enumerate() {
            let before = self.last.get(battery).copied();
            if let Lying::Home(_) = lying
                && !matches!(before, Some(Lying::Home(_)))
            {
                self.home_since[battery] = tick;
            }
            let (Some(Lying::Home(_)), Lying::Carried(thief)) = (before, *lying) else {
                continue;
            };
            let Some(body) = game.bodies.get(thief) else {
                continue;
            };
            let team = usize::from(body.team & 1);
            let at = body.feet;
            let t = &mut self.teams[team];
            t.steals += 1;
            t.by_thief[usize::from(!self.bot.get(thief).copied().unwrap_or(false))] += 1;
            let play = run
                .recs
                .get(thief)
                .and_then(|r| r.last())
                .and_then(|rec| run.plays.get(usize::from(rec.play)))
                .cloned()
                .unwrap_or_default();
            let kind = if play.contains("Raid") {
                "Raid"
            } else if play.contains("Sneak") {
                "Sneak"
            } else if play.contains("Collect") {
                "Collect"
            } else {
                "other"
            };
            *t.by_play.entry(kind.to_string()).or_default() += 1;
            let mates = |radius: f32| {
                game.bodies
                    .iter()
                    .enumerate()
                    .filter(|&(p, b)| p != thief && b.alive && b.team == body.team && b.feet.distance_xz(at) <= radius)
                    .count()
            };
            bucket(&mut t.mates_15, mates(CLOSE_M));
            bucket(&mut t.mates_30, mates(NEAR_M));
            let foes: Vec<Place> = game
                .bodies
                .iter()
                .enumerate()
                .filter(|&(_, b)| b.alive && b.team != body.team && b.feet.distance_xz(at) <= NEAR_M)
                .map(|(p, _)| p)
                .collect();
            bucket(&mut t.enemies_30, foes.len());
            let known = self.known.get(thief).map_or(0, |k| foes.iter().filter(|p| k.contains(p)).count());
            bucket(&mut t.enemies_known, known);
            if let Some(then) = self.snaps.get(self.snaps.len().saturating_sub(BEFORE_S + 1)).filter(|_| self.snaps.len() > BEFORE_S) {
                let group = then.iter().filter(|&&(alive, team_of, feet)| alive && team_of == body.team && feet.distance_xz(at) <= NEAR_M).count();
                bucket(&mut t.group_before, group);
            }
            t.ages_s.push(tick.saturating_sub(self.home_since[battery]) as f32 * TICK_S);
        }
        self.last = game.batteries.clone();
        if tick % SNAP_EVERY == 0 {
            self.snaps.push_back(game.bodies.iter().map(|b| (b.alive, b.team, b.feet)).collect());
            while self.snaps.len() > BEFORE_S + 2 {
                self.snaps.pop_front();
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("steals", self.part(scale))]
    }
}

impl StealCounts {
    fn part(&self, scale: &Scale) -> Part {
        let pct = |a: u64, b: u64| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
        let mut by_team = serde_json::Map::new();
        let mut lines = Vec::new();
        for (team, t) in self.teams.iter().enumerate() {
            let mut ages = t.ages_s.clone();
            ages.sort_by(f32::total_cmp);
            let at = |q: f32| ages.get(((ages.len() as f32 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
            let json = serde_json::json!({
                "steals": t.steals,
                "steals_a_bot_minute": scale.per_min(t.steals as f64),
                "steals_by_bot_standin": t.by_thief,
                "steals_by_play": t.by_play,
                "steal_mates_within_15": t.mates_15,
                "steal_mates_within_30": t.mates_30,
                "steal_group_10s_before": t.group_before,
                "steal_enemies_within_30": t.enemies_30,
                "steal_enemies_known_within_30": t.enemies_known,
                "steal_age_median_s": at(0.5),
                "steal_age_p25_s": at(0.25),
                "steal_age_p75_s": at(0.75),
                "chance_s": (10.0 * t.chance as f64 * f64::from(TICK_S)).round() / 10.0,
                "chance_tried_s": (10.0 * t.tried as f64 * f64::from(TICK_S)).round() / 10.0,
            });
            lines.push(format!(
                "team {team}: steals {} (bot/stand-in {:?}, by play {:?}), mates within 15 m {:?} within 30 m {:?}, the group 10 s before {:?}, enemies within 30 m {:?} (known {:?}), age median {:.0} s, \
                 chance {:.0} s of which tried {:.0} s = {:.0} %",
                t.steals,
                t.by_thief,
                t.by_play,
                t.mates_15,
                t.mates_30,
                t.group_before,
                t.enemies_30,
                t.enemies_known,
                at(0.5),
                t.chance as f64 * f64::from(TICK_S),
                t.tried as f64 * f64::from(TICK_S),
                pct(t.tried, t.chance),
            ));
            by_team.insert(team.to_string(), json);
        }
        Part::new(serde_json::json!({ "steals": { "by_team": by_team } }), format!("owner-test steals: {}", lines.join("; ")))
    }
}
