//! The decision metrics of a bench match that are not a mechanism's own (`traces-20261028/BENCH-DECISIONS.md` §2A, §2D, §2F), all by
//! team and for any mode, so that two arms (or the two sides of a head-to-head) can be told apart on them:
//!
//! - **enemy position reads** (§2D, "decisions about an enemy not seen or heard within 15 s"): how many reads of an enemy's position
//!   by a decision (`Ctx::known_place`, `enemies_near`, `threats`, and wherever a package calls [`fairness::read`]) rested on the
//!   team's own senses within 15 s (fresh), on an older sense (stale), on the HUD or on a cheat. New code should add no stale, HUD
//!   or cheat read.
//! - **reaction** (§2D): ms from the first tick a bot has a live sight of an enemy (none in the second before) to its first fire key
//!   down, median and p10, by the bot's team, with the engagements that ended with no shot (sight lost, or the bot died first).
//! - **trades** (§2D): a death is traded when a teammate of the dead kills the killer within [`TRADE_MS`]; by the dead's team the
//!   deaths and those traded, by the avenger's team the trades made. Team-wide (the Trade play's own count is one play's).
//! - **damage** (§2A): the health bodies lost in a tick, by the team that lost it (taken) and by the team of the shooters of that tick
//!   (dealt; a tick's loss is shared by the enemy hits on the body in it); the rest (a grenade, a fall, a zone, bleeding, a mate) is
//!   `other`. In lives: 1.0 is a body's full health.
//! - **work** (§2F, "the deterministic work counters decide"): floors expanded, rays cast, spot queries and plays scored by the
//!   bots' thinks and ticks of each team, a match-second (`bench_work`); the work no bot owns (the commanders, the shared search
//!   slices) is `shared`.

use super::*;
use crate::v026g::bots::bench_work::{self, Work};
use crate::v026g::bots::fairness;

/// A death is traded when a teammate kills the killer within this long, ms.
const TRADE_MS: u32 = 5_000;
/// A bot with no live sight for this long, ms, is out of its engagement.
const GAP_MS: u32 = 1_000;
/// The lower tenth of reactions.
const P10: f64 = 0.10;

/// Where a bot is in meeting an enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Engage {
    /// No live sight (for [`GAP_MS`] and more).
    #[default]
    Calm,
    /// A sight began at `since`, no shot yet; `lost` is when the sight was last seen to be gone.
    Sighted { since: u32, lost: Option<u32> },
    /// It fired; `lost` as above.
    Fighting { lost: Option<u32> },
}

/// What one tick tells a bot's engagement.
#[derive(Clone, Copy, Debug)]
struct Ev {
    tick: u32,
    alive: bool,
    sight: bool,
    firing: bool,
}

/// How an engagement ended or went on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// First shot, ms after the sight began.
    Reaction(u32),
    /// The sight went with no shot.
    Lost,
    /// The bot died with no shot.
    Died,
}

impl Engage {
    /// The one transition: the state and what the tick tells, to the new state and what ended.
    fn next(self, ev: Ev) -> (Engage, Option<Outcome>) {
        if !ev.alive {
            let ended = matches!(self, Engage::Sighted { .. }).then_some(Outcome::Died);
            return (Engage::Calm, ended);
        }
        let lost_for = |lost: Option<u32>| match (ev.sight, lost) {
            (true, _) => (None, false),
            (false, None) => (Some(ev.tick), false),
            (false, Some(at)) => (Some(at), ev.tick.saturating_sub(at) > GAP_MS),
        };
        match self {
            Engage::Calm if ev.sight => Engage::Sighted { since: ev.tick, lost: None }.next(ev),
            Engage::Calm => (Engage::Calm, None),
            Engage::Sighted { since, .. } if ev.firing => {
                (Engage::Fighting { lost: None }, Some(Outcome::Reaction(ev.tick.saturating_sub(since))))
            }
            Engage::Sighted { since, lost } => match lost_for(lost) {
                (_, true) => (Engage::Calm, Some(Outcome::Lost)),
                (lost, false) => (Engage::Sighted { since, lost }, None),
            },
            Engage::Fighting { lost } => match lost_for(lost) {
                (_, true) => (Engage::Calm, None),
                (lost, false) => (Engage::Fighting { lost }, None),
            },
        }
    }
}

#[derive(Default)]
pub(super) struct Metrics {
    /// The thread's counts and the tick when the match began (the tally's first tick).
    start: Option<Baseline>,
    /// By place: the bot has a live sight of an enemy now (read before the tick's bodies).
    sighted: Vec<bool>,
    engage: Vec<Engage>,
    /// By team: ms from the sight to the first shot, engagements ended with no shot, and deaths before a shot.
    reactions: [Vec<u32>; 2],
    lost: [u32; 2],
    died: [u32; 2],
    /// By place: the health at the last tick while it lived.
    health: Vec<Option<f32>>,
    /// By team: bodies' ticks spent dead, waiting to respawn (or out of tickets).
    dead_ticks: [u64; 2],
    /// By team and by whether the kit the body died in carries the radio (0 with, 1 without): ticks dead and deaths.
    dead_by_radio: [[u64; 2]; 2],
    deaths_by_radio: [[u32; 2]; 2],
    /// By team: damage dealt, taken and other (in lives).
    dealt: [f64; 2],
    taken: [f64; 2],
    other: [f64; 2],
}

/// What the thread had counted when the match began.
#[derive(Clone, Copy)]
struct Baseline {
    reads: fairness::Counts,
    work: bench_work::Totals,
    tick: u32,
}

impl Metrics {
    fn grow(&mut self, places: usize) {
        if self.sighted.len() < places {
            self.sighted.resize(places, false);
            self.engage.resize(places, Engage::Calm);
            self.health.resize(places, None);
        }
    }

    /// A bot's live sights, read by the tally from the bot or a test's fabricated one.
    fn sees(&mut self, place: Place, sighted: bool) {
        self.grow(place + 1);
        self.sighted[place] = sighted;
    }
}

impl Topic for Metrics {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        self.sees(place, bot.is_some_and(|bot| bot.memory.live_sights().next().is_some()));
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        self.sees(place, now.is_some_and(|now| now.sighted));
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        self.grow(game.bodies.len());
        self.start.get_or_insert_with(|| Baseline { reads: fairness::counts(), work: bench_work::totals(), tick: game.tick });
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        let ev = Ev {
            tick: at.tick,
            alive: at.rec.alive,
            sight: self.sighted.get(at.place).copied().unwrap_or(false),
            firing: at.rec.keys & key::FIRE != 0,
        };
        let team = usize::from(at.game.bodies[at.place].team & 1);
        let (state, ended) = self.engage[at.place].next(ev);
        self.engage[at.place] = state;
        match ended {
            Some(Outcome::Reaction(ms)) => self.reactions[team].push(ms),
            Some(Outcome::Lost) => self.lost[team] += 1,
            Some(Outcome::Died) => self.died[team] += 1,
            None => {}
        }
    }

    fn end(&mut self, _run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        self.grow(game.bodies.len());
        for (place, body) in game.bodies.iter().enumerate() {
            let team = usize::from(body.team & 1);
            if let Some(last) = self.health[place] {
                // A death tick can leave the health below nothing: a body loses at most what it had.
                let lost = f64::from((last - body.health.max(0.0)).max(0.0));
                if lost > 0.0 {
                    self.taken[team] += lost;
                    let shooters: Vec<Place> =
                        game.hit_log.iter().filter(|&&(_, _, victim)| victim == place).map(|&(_, shooter, _)| shooter).collect();
                    if shooters.is_empty() {
                        self.other[team] += lost;
                    } else {
                        for shooter in &shooters {
                            self.dealt[usize::from(game.bodies[*shooter].team & 1)] += lost / shooters.len() as f64;
                        }
                    }
                }
            }
            let radio = usize::from(!body.radio.get(body.heal.kit()).copied().unwrap_or(true));
            if self.health[place].is_some() && !body.alive {
                self.deaths_by_radio[team][radio] += 1;
            }
            self.health[place] = body.alive.then_some(body.health);
            self.dead_ticks[team] += u64::from(!body.alive);
            self.dead_by_radio[team][radio] += u64::from(!body.alive);
        }
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let seconds = f64::from(game.tick.saturating_sub(self.start.map_or(game.tick, |start| start.tick))) / 1000.0;
        vec![
            ("fairness", self.reads_part()),
            ("reaction", self.reaction_part()),
            ("trades", self.trades_part(run, game)),
            ("damage", self.damage_part()),
            ("work", self.work_part(seconds)),
            ("respawn", self.respawn_part(game)),
        ]
    }
}

/// The value at the fraction `p` of the sorted `values` (none: no values).
pub(super) fn quantile(values: &[u32], p: f64) -> Option<u32> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let last = sorted.len().checked_sub(1)?;
    sorted.get((last as f64 * p).round() as usize).copied()
}

pub(super) fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

pub(super) fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

impl Metrics {
    fn reads_part(&self) -> Part {
        let now = fairness::counts();
        let before = self.start.map_or(now, |start| start.reads);
        const SITES: [&str; fairness::SITES] = ["place", "near", "threats"];
        const BASES: [&str; fairness::BASES] = ["fresh", "stale", "hud", "cheat"];
        let delta = |team: usize, site: usize, basis: usize| now[team][site][basis] - before[team][site][basis];
        let mut json = serde_json::Map::new();
        let mut text = vec!["owner-test fairness: enemy position reads by team (fresh within 15 s / stale / hud / cheat)".to_owned()];
        for (site, name) in SITES.iter().enumerate() {
            let per_team: Vec<Vec<u64>> = (0..2).map(|team| (0..fairness::BASES).map(|basis| delta(team, site, basis)).collect()).collect();
            text.push(format!("{name} {:?} {:?}", per_team[0], per_team[1]));
            json.insert((*name).to_owned(), serde_json::json!(per_team));
        }
        let (mut fresh, mut all) = ([0u64; 2], [0u64; 2]);
        for team in 0..2 {
            for site in 0..fairness::SITES {
                for basis in 0..fairness::BASES {
                    all[team] += delta(team, site, basis);
                    if basis == fairness::Basis::Fresh as usize {
                        fresh[team] += delta(team, site, basis);
                    }
                }
            }
        }
        let unseen_pct: Vec<f64> =
            (0..2).map(|team| if all[team] > 0 { round1(100.0 * (all[team] - fresh[team]) as f64 / all[team] as f64) } else { 0.0 }).collect();
        text.push(format!("reads {all:?} not fresh {unseen_pct:?} % (by team; bases {BASES:?})"));
        Part::new(
            serde_json::json!({ "enemy_reads": json, "enemy_reads_total": all, "enemy_reads_unseen_pct": unseen_pct }),
            text.join("; "),
        )
    }

    fn reaction_part(&self) -> Part {
        let stat = |team: usize, p: f64| quantile(&self.reactions[team], p);
        let n: Vec<usize> = (0..2).map(|team| self.reactions[team].len()).collect();
        let median: Vec<Option<u32>> = (0..2).map(|team| stat(team, 0.5)).collect();
        let p10: Vec<Option<u32>> = (0..2).map(|team| stat(team, P10)).collect();
        let text = format!(
            "owner-test reaction (first live sight to first fire key, ms, by team): n {n:?} median {median:?} p10 {p10:?}; engagements with no shot: sight lost {:?}, died first {:?}",
            self.lost, self.died
        );
        Part::new(
            serde_json::json!({ "reaction": { "n": n, "median_ms": median, "p10_ms": p10, "lost": self.lost, "died_first": self.died } }),
            text,
        )
    }

    fn trades_part(&self, run: &Run, game: &Game) -> Part {
        let team = |place: Place| usize::from(game.bodies.get(place).map_or(0, |b| b.team) & 1);
        let (mut deaths, mut traded) = ([0u32; 2], [0u32; 2]);
        for (i, &(t, killer, victim)) in run.kills.iter().enumerate() {
            deaths[team(victim)] += 1;
            // A teammate of the victim kills the killer within the window (not the victim, who is dead).
            let avenger = run.kills[i + 1..]
                .iter()
                .take_while(|&&(t2, ..)| t2.saturating_sub(t) <= TRADE_MS)
                .find(|&&(_, killer2, victim2)| victim2 == killer && killer2 != victim && team(killer2) == team(victim));
            if avenger.is_some() {
                traded[team(victim)] += 1;
            }
        }
        let rate: Vec<f64> = (0..2).map(|t| if deaths[t] > 0 { round3(f64::from(traded[t]) / f64::from(deaths[t])) } else { 0.0 }).collect();
        let text = format!(
            "owner-test trades within {} s, team-wide (by the dead's team; the avenger is a teammate): deaths {deaths:?} traded {traded:?} rate {rate:?}",
            TRADE_MS / 1000
        );
        Part::new(
            serde_json::json!({ "trades_team": { "deaths": deaths, "traded": traded, "rate": rate, "window_s": TRADE_MS / 1000 } }),
            text,
        )
    }

    fn damage_part(&self) -> Part {
        let lives = |x: [f64; 2]| x.map(round3);
        let text = format!(
            "owner-test damage (lives, by team): dealt {:?} taken {:?} other (grenade, fall, zone, bleed, mate) {:?}",
            lives(self.dealt),
            lives(self.taken),
            lives(self.other)
        );
        Part::new(
            serde_json::json!({ "damage": { "dealt": lives(self.dealt), "taken": lives(self.taken), "other": lives(self.other) } }),
            text,
        )
    }

    /// Seconds the team's bodies spent dead, and per death (their wait for the respawn: the radio's -20 % is in it for a kit that carries
    /// one, `Game::respawn_ms_of_body`).
    fn respawn_part(&self, game: &Game) -> Part {
        let dead_s = self.dead_ticks.map(|ticks| round1(ticks as f64 * f64::from(TICK_MS) / 1000.0));
        let mut deaths = [0u32; 2];
        for (place, body) in game.bodies.iter().enumerate() {
            deaths[usize::from(body.team & 1)] += game.stats.get(place).map_or(0, |s| s.1);
        }
        let per_death: Vec<f64> = (0..2).map(|t| if deaths[t] > 0 { round1(dead_s[t] / f64::from(deaths[t])) } else { 0.0 }).collect();
        // By the radio of the kit the body died in: its deaths and the seconds it was dead, per team.
        let by_radio = |radio: usize| {
            let dead: Vec<f64> = (0..2).map(|t| round1(self.dead_by_radio[t][radio] as f64 * f64::from(TICK_MS) / 1000.0)).collect();
            let n: Vec<u32> = (0..2).map(|t| self.deaths_by_radio[t][radio]).collect();
            let per: Vec<f64> = (0..2).map(|t| if n[t] > 0 { round1(dead[t] / f64::from(n[t])) } else { 0.0 }).collect();
            (dead, n, per)
        };
        let (with, without) = (by_radio(0), by_radio(1));
        let text = format!(
            "owner-test respawn (by team): seconds dead {dead_s:?}, deaths {deaths:?}, seconds dead a death {per_death:?}; by the kit's radio: with {:?} deaths, {:?} s a death; without {:?} deaths, {:?} s a death",
            with.1, with.2, without.1, without.2
        );
        Part::new(
            serde_json::json!({ "respawn": {
                "dead_s": dead_s, "deaths": deaths, "dead_s_per_death": per_death,
                "with_radio": { "deaths": with.1, "dead_s": with.0, "dead_s_per_death": with.2 },
                "without_radio": { "deaths": without.1, "dead_s": without.0, "dead_s_per_death": without.2 },
            } }),
            text,
        )
    }

    fn work_part(&self, seconds: f64) -> Part {
        let now = bench_work::totals();
        let before = self.start.map_or(now, |start| start.work);
        const NAMES: [(&str, Work); bench_work::KINDS] =
            [("floors", Work::Floors), ("rays", Work::Rays), ("spots", Work::Spots), ("plays", Work::Plays)];
        let per_s = |row: usize, work: Work| {
            let n = now[row][work as usize] - before[row][work as usize];
            if seconds > 0.0 { round1(n as f64 / seconds) } else { 0.0 }
        };
        let mut json = serde_json::Map::new();
        let mut text = vec![format!("owner-test work a match-second (team 0, team 1, shared; over {seconds:.0} s)")];
        for (name, work) in NAMES {
            let row = [per_s(0, work), per_s(1, work), per_s(2, work)];
            text.push(format!("{name} {row:?}"));
            json.insert(name.to_owned(), serde_json::json!(row));
        }
        Part::new(serde_json::json!({ "work_per_s": json }), text.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(tick: u32, sight: bool, firing: bool) -> Ev {
        Ev { tick, alive: true, sight, firing }
    }

    /// The first shot after a sight is the reaction, once; the shots after it are none; a sight lost for more than a second ends it.
    #[test]
    fn the_first_shot_after_a_sight_is_the_reaction() {
        let (s, out) = Engage::Calm.next(ev(1_000, true, false));
        assert_eq!((s, out), (Engage::Sighted { since: 1_000, lost: None }, None));
        let (s, out) = s.next(ev(1_050, true, false));
        assert_eq!(out, None);
        let (s, out) = s.next(ev(1_350, true, true));
        assert_eq!((s, out), (Engage::Fighting { lost: None }, Some(Outcome::Reaction(350))));
        let (s, out) = s.next(ev(1_400, true, true));
        assert_eq!((s, out), (Engage::Fighting { lost: None }, None));
        // Out of sight for a second: still the same engagement; past it: calm, and a new sight starts a new one.
        let (s, _) = s.next(ev(2_000, false, false));
        let (s, _) = s.next(ev(3_000, false, false));
        assert!(matches!(s, Engage::Fighting { .. }));
        let (s, _) = s.next(ev(3_050, false, false));
        assert_eq!(s, Engage::Calm);
    }

    /// A shot in the tick a sight begins is a reaction of 0; a sight lost with no shot, or a death with none, ends it with no reaction.
    #[test]
    fn an_engagement_with_no_shot_ends_with_no_reaction() {
        assert_eq!(Engage::Calm.next(ev(500, true, true)).1, Some(Outcome::Reaction(0)));
        let (mut s, mut out) = Engage::Calm.next(ev(0, true, false));
        for tick in (50..=1_100).step_by(50) {
            (s, out) = s.next(ev(tick, false, false));
        }
        assert_eq!((s, out), (Engage::Calm, Some(Outcome::Lost)));
        let (s, _) = Engage::Calm.next(ev(0, true, false));
        let dead = Ev { alive: false, ..ev(100, true, false) };
        assert_eq!(s.next(dead), (Engage::Calm, Some(Outcome::Died)));
    }

    /// A teammate's kill of the killer within the window trades the death; a kill after it, by an enemy, or by the victim's own side's
    /// other death, does not.
    #[test]
    fn a_death_is_traded_by_a_teammate_killing_the_killer_in_five_seconds() {
        let (mut game, mut tally) = made_up(&[0, 0, 1, 1]);
        // Bot 0 (team 0) dies to bot 2 at 10 s; bot 1 kills bot 2 at 13 s: traded. Bot 1 dies to bot 3 at 30 s; bot 0 kills him at 36 s: late.
        tally.run.kills.extend([(10_000, 2, 0), (13_000, 1, 2), (30_000, 3, 1), (36_000, 0, 3)]);
        game.tick = 40_000;
        let (_, json) = tally.finish(&game);
        let trades = &json["trades_team"];
        assert_eq!(trades["deaths"], serde_json::json!([2, 2]));
        assert_eq!(trades["traded"], serde_json::json!([1, 0]));
    }

    /// Health lost in a tick with an enemy hit on the body is dealt by the shooter's team; health lost with none is `other`; both are taken.
    #[test]
    fn health_lost_to_an_enemy_hit_is_dealt_and_the_rest_is_other() {
        let (mut game, mut tally) = made_up(&[0, 1]);
        game.tick = 1_000;
        tally.step_with(&mut game, &[None, None]);
        game.bodies[1].health = 0.5;
        game.hit_log.push((1_050, 0, 1));
        game.tick = 1_050;
        tally.step_with(&mut game, &[None, None]);
        game.hit_log.clear();
        game.bodies[0].health = 0.75;
        game.tick = 1_100;
        tally.step_with(&mut game, &[None, None]);
        let m = tally.topic::<Metrics>();
        let near = |a: [f64; 2], b: [f64; 2]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
        assert!(near(m.dealt, [0.5, 0.0]), "{:?}", m.dealt);
        assert!(near(m.taken, [0.25, 0.5]), "{:?}", m.taken);
        assert!(near(m.other, [0.25, 0.0]), "{:?}", m.other);
    }

    /// A death is counted by the radio of the kit the body died in, and its dead seconds with it.
    #[test]
    fn deaths_and_dead_seconds_are_counted_by_the_kits_radio() {
        let (mut game, mut tally) = made_up(&[0, 1]);
        game.bodies[1].radio = [false; 3];
        game.tick = 1_000;
        tally.step_with(&mut game, &[None, None]);
        for place in 0..2 {
            game.bodies[place].alive = false;
        }
        for k in 1..=4u32 {
            game.tick = 1_000 + k * TICK_MS;
            tally.step_with(&mut game, &[None, None]);
        }
        let m = tally.topic::<Metrics>();
        assert_eq!((m.deaths_by_radio[0], m.deaths_by_radio[1]), ([1, 0], [0, 1]));
        assert_eq!((m.dead_by_radio[0], m.dead_by_radio[1]), ([4, 0], [0, 4]));
    }
}
