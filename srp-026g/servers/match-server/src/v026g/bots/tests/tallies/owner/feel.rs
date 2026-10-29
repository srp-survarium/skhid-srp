//! The feel of a match for one player (`traces-20261028/an-top-player.md` §5, `an-gang-up.md`): the counters that tell whether the
//! bots treat a star, or a human, as a live crowd does. Each is worked out the way the live census worked it out, so that a bench
//! number can be set beside the live one. By team for the team's top bot, for the stand-in human (`SRP_BENCH_HUMAN`) and, for the
//! rest of the bots, as the baseline they are compared with.
//!
//! - **top:** a team's top bot is the one the bots' own rule (`plays::pair_up`, `streak_weight`, `top_min`, `top_lead`) names:
//!   kills + 2 x streak at least 8 and 3 over the team's next, read at the last tick (so a death does not hide that he was top).
//! - **deaths a minute alive** as top and not (live 1.19 and 0.81).
//! - **gang-up share:** deaths with 3 or more distinct enemies within 30 m of the dead at some tick of the 10 s before (live tops 41 to
//!   45 %, non-tops 27 to 29 %), and the mean number of enemies within 30 m at the death, and distinct in the 10 s (live 1.1 for a
//!   top's whole life). Distance is the straight line.
//! - **spawn to first contact:** seconds from a life's start to the first tick an enemy is within 30 m, by the group the life began
//!   in (live median 14 to 15 s for a top's lives, 17 to 18 s for the others); lives that ended with no contact are counted apart.
//! - **a note per crowded death** of a top or of the human: who was within 30 m, the play each ran, and how each one's track of the
//!   dead came to it (Seen, Heard, Told, Hud, Cheat(Radar), Cheat(Thief), none), from the bot's own memory at the tick.

use super::*;
use crate::v026g::bots::know::{CheatKind, Source};
use crate::v026g::bots::plays::pair_up;

/// Within this of the dead, m, an enemy is near.
const NEAR_M: f32 = 30.0;
/// The window before a death, in ticks (10 s).
const WINDOW_TICKS: usize = (10_000 / TICK_MS) as usize;
/// Enemies near in the window that make a death crowded.
const CROWD: u32 = 3;

/// What a player is for these counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cat {
    /// The top bot of team `n`.
    Top(u8),
    /// Any other bot of team `n`.
    Rest(u8),
    /// The stand-in human.
    Human,
}

impl Cat {
    const ALL: [Cat; 5] = [Cat::Top(0), Cat::Top(1), Cat::Rest(0), Cat::Rest(1), Cat::Human];

    fn index(self) -> usize {
        match self {
            Cat::Top(team) => usize::from(team & 1),
            Cat::Rest(team) => 2 + usize::from(team & 1),
            Cat::Human => 4,
        }
    }

    fn name(self) -> String {
        match self {
            Cat::Top(team) => format!("top of team {team}"),
            Cat::Rest(team) => format!("other bots of team {team}"),
            Cat::Human => "the stand-in human".to_owned(),
        }
    }
}

/// A player's life, for the first contact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Life {
    #[default]
    Dead,
    /// Alive since `spawned`; `cat` is what it was at the start; whether an enemy has come near yet.
    Alive { spawned: u32, cat: Cat, met: bool },
}

/// What is counted for a group of players.
#[derive(Default)]
struct Group {
    /// Ticks alive.
    alive_ticks: u64,
    deaths: u32,
    crowded: u32,
    /// Enemies within 30 m at the death, summed; distinct in the 10 s before, summed.
    near_at_death: u64,
    near_in_window: u64,
    /// Lives that began in this group: ms to the first contact, and lives that ended with none.
    contact_ms: Vec<u32>,
    no_contact: u32,
}

/// A crowded death, for its note: when, who, what he was, the enemies near him at it (a set of places) and distinct in 10 s.
#[derive(Clone, Copy)]
struct Death {
    t: u32,
    victim: Place,
    cat: Cat,
    near: u64,
    window: u32,
}

/// Where a bot's track of each enemy came from, by place of the enemy.
type Sources = Vec<(Place, &'static str)>;

#[derive(Default)]
pub(super) struct Feel {
    context: String,
    human: Option<Place>,
    groups: [Group; 5],
    life: Vec<Life>,
    /// The top of each team at the last tick.
    tops: [Option<Place>; 2],
    kills_seen: usize,
    /// By place: the sources of the bot's tracks, as of its last read.
    sources: Vec<Sources>,
    /// The notes of crowded deaths.
    notes: Vec<String>,
}

fn number(name: &str) -> f32 {
    pair_up::NUMBERS.iter().find(|(n, _)| *n == name).map_or(0.0, |(_, v)| *v)
}

/// A source as the note names it.
fn source_name(source: Source) -> &'static str {
    match source {
        Source::Seen => "Seen",
        Source::Heard => "Heard",
        Source::Told { .. } => "Told",
        Source::Hud => "Hud",
        Source::Cheat(CheatKind::Radar) => "Cheat(Radar)",
        Source::Cheat(CheatKind::Thief) => "Cheat(Thief)",
    }
}

impl Feel {
    pub(super) fn set_human(&mut self, context: String, human: Option<Place>) {
        self.context = context;
        self.human = human;
    }

    fn grow(&mut self, places: usize) {
        if self.life.len() < places {
            self.life.resize(places, Life::Dead);
            self.sources.resize(places, Vec::new());
        }
    }

    fn cat(&self, place: Place, team: u8) -> Cat {
        if self.human == Some(place) {
            Cat::Human
        } else if self.tops[usize::from(team & 1)] == Some(place) {
            Cat::Top(team)
        } else {
            Cat::Rest(team)
        }
    }

    /// The team's top by the bots' rule from the game's kills and streaks.
    fn top_of(&self, game: &Game, team: u8) -> Option<Place> {
        let worth = |place: Place| {
            let (kills, _, streak) = game.stats.get(place).copied().unwrap_or_default();
            kills as f32 + number("streak_weight") * streak as f32
        };
        let mut mates: Vec<(Place, f32)> = game
            .bodies
            .iter()
            .enumerate()
            .filter(|(place, body)| body.team == team && self.human != Some(*place))
            .map(|(place, _)| (place, worth(place)))
            .collect();
        mates.sort_by(|a, b| b.1.total_cmp(&a.1));
        let (top, best) = *mates.first()?;
        let next = mates.get(1).map_or(0.0, |m| m.1);
        (best >= number("top_min") && best - next >= number("top_lead")).then_some(top)
    }

    /// The enemies of `team` near `at` at record `idx` of the run, as a set of places.
    fn near(run: &Run, game: &Game, team: u8, at: Vec3, idx: usize) -> u64 {
        let mut set = 0u64;
        for (place, body) in game.bodies.iter().enumerate().take(64) {
            if body.team == team {
                continue;
            }
            if let Some(rec) = run.recs.get(place).and_then(|recs| recs.get(idx))
                && rec.alive
                && (rec.feet - at).length() <= NEAR_M
            {
                set |= 1 << place;
            }
        }
        set
    }

    /// The tick's deaths: crowded or not, and a note for a top's or the human's.
    fn deaths_tick(&mut self, run: &Run, game: &Game) {
        let new: Vec<(u32, Place, Place)> = run.kills[self.kills_seen.min(run.kills.len())..].to_vec();
        self.kills_seen = run.kills.len();
        for (t, _killer, victim) in new {
            let Some(body) = game.bodies.get(victim) else {
                continue;
            };
            let team = body.team;
            let Some(recs) = run.recs.get(victim) else {
                continue;
            };
            let Some(last) = recs.len().checked_sub(1) else {
                continue;
            };
            let cat = self.cat(victim, team);
            let at = recs[last].feet;
            let now = Self::near(run, game, team, at, last);
            let mut window = 0u64;
            for idx in last.saturating_sub(WINDOW_TICKS)..=last {
                window |= Self::near(run, game, team, recs[idx].feet, idx);
            }
            let crowded = window.count_ones() >= CROWD;
            let group = &mut self.groups[cat.index()];
            group.deaths += 1;
            group.crowded += u32::from(crowded);
            group.near_at_death += u64::from(now.count_ones());
            group.near_in_window += u64::from(window.count_ones());
            if crowded && cat != Cat::Rest(team) {
                self.notes.push(self.crowd_line(run, game, &Death { t, victim, cat, near: now, window: window.count_ones() }));
            }
        }
    }

    /// One note for a crowded death.
    fn crowd_line(&self, run: &Run, game: &Game, death: &Death) -> String {
        let Death { t, victim, cat, near, window } = *death;
        let mut who = Vec::new();
        for place in (0..game.bodies.len().min(64)).filter(|p| near & (1 << p) != 0) {
            let play = run.recs[place].last().and_then(|rec| run.plays.get(usize::from(rec.play))).map_or("-", String::as_str);
            let knew = self
                .sources
                .get(place)
                .and_then(|list| list.iter().find(|(who, _)| *who == victim))
                .map_or("none", |(_, name)| name);
            who.push(format!("{place} {play} {knew}"));
        }
        format!(
            "crowded death: {} | tick {t} place {victim} {} | enemies within {NEAR_M:.0} m in 10 s {window}; at the death [{}]",
            self.context,
            cat.name(),
            who.join(", ")
        )
    }

    /// Each body's life: its first contact, once.
    fn lives_tick(&mut self, run: &Run, game: &Game, tick: u32) {
        for place in 0..game.bodies.len() {
            let body = &game.bodies[place];
            let Some(rec) = run.recs.get(place).and_then(|recs| recs.last()) else {
                continue;
            };
            let idx = run.recs[place].len() - 1;
            let cat = self.cat(place, body.team);
            let state = match (self.life[place], rec.alive) {
                (Life::Dead, true) => Life::Alive { spawned: tick, cat, met: false },
                (Life::Alive { cat: began, met, .. }, false) => {
                    // The life ended; one with no contact is counted.
                    if !met {
                        self.groups[began.index()].no_contact += 1;
                    }
                    Life::Dead
                }
                (state, _) => state,
            };
            self.life[place] = state;
            if rec.alive {
                self.groups[cat.index()].alive_ticks += 1;
                if let Life::Alive { spawned, cat: began, met: false } = state
                    && Self::near(run, game, body.team, rec.feet, idx) != 0
                {
                    self.groups[began.index()].contact_ms.push(tick.saturating_sub(spawned));
                    self.life[place] = Life::Alive { spawned, cat: began, met: true };
                }
            }
        }
    }
}

impl Topic for Feel {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        self.grow(place + 1);
        let list = &mut self.sources[place];
        list.clear();
        if let Some(bot) = bot {
            list.extend(bot.memory.tracks().iter().map(|track| (track.who, source_name(track.source))));
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        self.grow(game.bodies.len());
        // The categories of this tick are those of the last: a death keeps the standing he had.
        self.lives_tick(run, game, game.tick);
        self.deaths_tick(run, game);
        for team in 0..2u8 {
            self.tops[usize::from(team)] = self.top_of(game, team);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let mut json = serde_json::Map::new();
        let mut lines = vec!["owner-test feel (the live census's definitions; live in brackets)".to_owned()];
        for cat in Cat::ALL {
            let g = &self.groups[cat.index()];
            let minutes = g.alive_ticks as f64 * f64::from(TICK_MS) / 60_000.0;
            let per_min = if minutes > 0.0 { round3(f64::from(g.deaths) / minutes) } else { 0.0 };
            let share = if g.deaths > 0 { round1(100.0 * f64::from(g.crowded) / f64::from(g.deaths)) } else { 0.0 };
            let mean = |sum: u64| if g.deaths > 0 { round3(sum as f64 / f64::from(g.deaths)) } else { 0.0 };
            let contact: Vec<Option<f64>> =
                [0.25, 0.5, 0.75].map(|p| quantile(&g.contact_ms, p).map(|ms| round1(f64::from(ms) / 1000.0))).to_vec();
            lines.push(format!(
                "{}: alive {minutes:.1} min, deaths {} = {per_min} a minute (live 1.19 top, 0.81 not), crowded {} = {share} % (live top 41-45, not 27-29), enemies within 30 m at death {} / distinct in 10 s {} (live 1.1), spawn to first contact p25/median/p75 s {contact:?} over {} lives, {} lives with none (live 14-15 top, 17-18 not)",
                cat.name(),
                g.deaths,
                g.crowded,
                mean(g.near_at_death),
                mean(g.near_in_window),
                g.contact_ms.len(),
                g.no_contact,
            ));
            json.insert(
                cat.name(),
                serde_json::json!({
                    "alive_min": round3(minutes),
                    "deaths": g.deaths,
                    "deaths_per_min": per_min,
                    "crowded": g.crowded,
                    "crowded_pct": share,
                    "enemies_30m_at_death": mean(g.near_at_death),
                    "enemies_30m_in_10s": mean(g.near_in_window),
                    "contact_s_p25_median_p75": contact,
                    "contact_lives": g.contact_ms.len(),
                    "no_contact_lives": g.no_contact,
                }),
            );
        }
        lines.extend(self.notes.iter().cloned());
        vec![("feel", Part::new(serde_json::json!({ "feel": json }), lines.join("\n  ")))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body of team 0 with kills and a streak that stand out is the team's top; two close ones are none.
    #[test]
    fn a_top_stands_out_by_kills_and_streak_from_the_next() {
        let (mut game, _) = made_up(&[0, 0, 0, 1]);
        let feel = Feel::default();
        game.stats[0] = (6, 0, 2); // 6 + 4 = 10
        game.stats[1] = (6, 0, 0); // 6: lead 4
        assert_eq!(feel.top_of(&game, 0), Some(0));
        game.stats[1] = (8, 0, 0); // lead 2: no top
        assert_eq!(feel.top_of(&game, 0), None);
        game.stats[0] = (3, 0, 2); // 7: under the minimum
        game.stats[1] = (0, 0, 0);
        assert_eq!(feel.top_of(&game, 0), None);
    }

    /// Three enemies within 30 m of the dead in the 10 s before make a death crowded; two do not; the first contact is timed from
    /// the spawn.
    #[test]
    fn a_death_with_three_enemies_near_is_crowded_and_a_contact_is_timed() {
        let (mut game, mut tally) = made_up(&[0, 1, 1, 1, 1]);
        // Body 0 (team 0) at the origin; enemies 1-3 come within 30 m at 2 s, body 4 stays far.
        for k in 0..60u32 {
            game.tick = (k + 1) * TICK_MS;
            put(&mut game, 0, 10.0, 10.0, 0);
            for place in 1..=3 {
                let near = k >= 40;
                put(&mut game, place, if near { 20.0 } else { 70.0 }, 10.0 + place as f32, 0);
            }
            put(&mut game, 4, 75.0, 30.0, 0);
            tally.step_with(&mut game, &[None, None, None, None, None]);
        }
        game.tick += TICK_MS;
        game.bodies[0].alive = false;
        game.kill_log.push((game.tick, 1, 0));
        tally.step_with(&mut game, &[None, None, None, None, None]);
        let (text, json) = tally.finish(&game);
        let rest = &json["feel"]["other bots of team 0"];
        assert_eq!((rest["deaths"].as_u64(), rest["crowded"].as_u64()), (Some(1), Some(1)), "{text}");
        // Body 0 met an enemy 2 s after it spawned (the tick of the first record).
        let contact = rest["contact_s_p25_median_p75"][1].as_f64().unwrap_or(f64::NAN);
        assert!((contact - 2.0).abs() < 0.2, "{contact}");
    }
}
