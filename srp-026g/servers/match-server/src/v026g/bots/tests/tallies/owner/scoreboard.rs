//! The scoreboard of a bench match: one line per player at the match's end, with the game's own score, so that the owner's bar
//! ("an average player should be around 3rd in 10v10", `traces-20261028/BENCH-DECISIONS.md` §5) can be read off a bench run, by team.
//!
//! `scoreboard: <project> <N>v<N> seed S | place P team T kind bot|camper|human | kills K deaths D assists A streak B | events
//! {id: count} | objective {kind: points} | score X`, and the same objects as `sums.scoreboard`.
//!
//! - **kills, deaths, streak:** `game.stats` (the longest streak is the best seen at a tick).
//! - **events** (the game's event ids, `static_info::event`): 0 kill; 1 assist; 2 death; 6 brought (100 hundredths: the stand-in's
//!   carrier delivers alone, the client splits the bring by carry share); 20 put in (the carrier at the container, `put_in_log`);
//!   10 enemy carrier killed (`carrier_kills`). Not counted: headshot (15), steal (17), found (19): the stand-in has no part for
//!   the first, and the other two are not in its log as counts.
//! - **assists (UNCALIBRATED):** the rule is the original server's, not the client's: srp's server only relays the event
//!   (`snapshot::event::ASSIST`) and the client exe has no rule to read (checked 2026-10-29), so none is PROVEN. This one is a GUESS: a non-killer teammate of the killer who hit the victim within
//!   [`ASSIST_MS`] before the kill. The live figure is 0.19 assists a kill (`bench-decision-metrics.md` §c): the share is printed beside
//!   it and the window is the thing to move if it is far off.
//! - **score:** `snapshot::score` with `static_info::score_weights(mode)`, the client's own sum of the events (the table is not
//!   copied). The mode is the stand-in's: tickets mean Team Deathmatch, else batteries.
//! - **objective points** by kind, with the same weights: `delivered` (events 6 and 20), `carrier_kill` (event 10).
//! - **kind:** `human` the stand-in human (`SRP_BENCH_HUMAN`), `camper` the stand-in camper, else `bot`.

use survarium::v026g::maps::mode;
use survarium::v026g::match_msgs::snapshot::{self, PlayerStatistics};
use survarium::v026g::match_msgs::static_info;

use super::*;

/// A teammate's hit on the victim this long before the kill makes an assist (GUESS).
const ASSIST_MS: u32 = 10_000;
/// The windows the assists a kill are also counted for, s, so that the window can be chosen against the live 0.19 a kill.
const WINDOWS_S: [u32; 6] = [1, 2, 3, 5, 10, 30];

/// Score events the stand-in has a part for (the game's ids).
const KILL: usize = 0;
const ASSIST: usize = 1;
const DEATH: usize = 2;
const BROUGHT: usize = 6;
const CARRIER_KILL: usize = 10;
const PUT_IN: usize = 20;

/// What a place is in the bench.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Kind {
    #[default]
    Bot,
    Camper,
    Human,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Bot => "bot",
            Kind::Camper => "camper",
            Kind::Human => "human",
        }
    }
}

#[derive(Default)]
pub(super) struct Scoreboard {
    /// "<project> NvN seed S" for the line.
    context: String,
    human: Option<Place>,
    camper: Option<Place>,
    /// The hits of the last [`ASSIST_MS`]: tick, shooter, victim.
    hits: Vec<(u32, Place, Place)>,
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    assists: Vec<u32>,
    /// By [`WINDOWS_S`]: the assists the rule would give (distinct teammates who hit within the window), over the match.
    by_window: [u32; WINDOWS_S.len()],
    carrier_kills_seen: usize,
    /// By place: enemy carriers killed.
    carrier_kills: Vec<u32>,
    best_streak: Vec<u32>,
}

impl Scoreboard {
    pub(super) fn set_context(&mut self, context: String, human: Option<Place>, camper: Option<Place>) {
        self.context = context;
        self.human = human;
        self.camper = camper;
    }

    fn kind(&self, place: Place) -> Kind {
        if self.human == Some(place) {
            Kind::Human
        } else if self.camper == Some(place) {
            Kind::Camper
        } else {
            Kind::Bot
        }
    }

    fn grow(&mut self, places: usize) {
        for list in [&mut self.assists, &mut self.carrier_kills, &mut self.best_streak] {
            if list.len() < places {
                list.resize(places, 0);
            }
        }
    }

    /// The kills since the last tick: each non-killer teammate that hit the victim within [`ASSIST_MS`] before it has an assist.
    fn kills_tick(&mut self, game: &Game) {
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new: Vec<_> = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        for (t, killer, victim) in new {
            let Some(team) = game.bodies.get(killer).map(|b| b.team) else {
                continue;
            };
            let mut helped: Vec<Place> = Vec::new();
            for (k, window) in WINDOWS_S.iter().enumerate() {
                let mut within: Vec<Place> = Vec::new();
                for &(hit_t, shooter, target) in &self.hits {
                    if target == victim
                        && shooter != killer
                        && hit_t <= t
                        && t - hit_t <= window * 1000
                        && game.bodies.get(shooter).is_some_and(|b| b.team == team)
                        && !within.contains(&shooter)
                    {
                        within.push(shooter);
                    }
                }
                self.by_window[k] += within.len() as u32;
            }
            for &(hit_t, shooter, target) in &self.hits {
                if target == victim
                    && shooter != killer
                    && t.saturating_sub(hit_t) <= ASSIST_MS
                    && hit_t <= t
                    && game.bodies.get(shooter).is_some_and(|b| b.team == team)
                    && !helped.contains(&shooter)
                {
                    helped.push(shooter);
                }
            }
            for shooter in helped {
                self.assists[shooter] += 1;
            }
        }
    }
}

impl Topic for Scoreboard {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn end(&mut self, _run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        self.grow(game.bodies.len());
        self.hits.extend(game.hit_log.iter().copied());
        let tick = game.tick;
        self.hits.retain(|&(t, ..)| tick.saturating_sub(t) <= WINDOWS_S[WINDOWS_S.len() - 1] * 1000);
        self.kills_tick(game);
        for &(killer, _) in &game.carrier_kills[self.carrier_kills_seen.min(game.carrier_kills.len())..] {
            if let Some(count) = self.carrier_kills.get_mut(killer) {
                *count += 1;
            }
        }
        self.carrier_kills_seen = game.carrier_kills.len();
        for (place, &(_, _, streak)) in game.stats.iter().enumerate() {
            if let Some(best) = self.best_streak.get_mut(place) {
                *best = (*best).max(streak);
            }
        }
    }

    fn parts(&mut self, _run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        self.grow(game.bodies.len());
        let game_mode = if game.tickets.is_some() { mode::TEAM_DEATHMATCH } else { mode::BATTERIES };
        let weights = static_info::score_weights(game_mode);
        let puts = |place: Place| game.put_in_log.iter().filter(|(p, _)| *p == place).count() as u32;
        let mut entries = Vec::new();
        for (place, body) in game.bodies.iter().enumerate() {
            let (kills, deaths, _) = game.stats.get(place).copied().unwrap_or_default();
            let (assists, carrier_kills, put_in) = (self.assists[place], self.carrier_kills[place], puts(place));
            // The events as the client counts them: a bring is 100 hundredths of a battery.
            let mut stats = PlayerStatistics::default();
            let mut set = |event: usize, count: u32| stats.events[event][0] = u16::try_from(count).unwrap_or(u16::MAX);
            set(KILL, kills);
            set(ASSIST, assists);
            set(DEATH, deaths);
            set(BROUGHT, put_in * 100);
            set(CARRIER_KILL, carrier_kills);
            set(PUT_IN, put_in);
            let score = snapshot::score(&stats, &weights);
            // The same score with no assists, for a margin that does not rest on the uncalibrated rule.
            stats.events[ASSIST][0] = 0;
            let score_without_assists = snapshot::score(&stats, &weights);
            let points = |event: usize, count: u32| {
                let hundredths = if static_info_hundredths(event) { 0.01 } else { 1.0 };
                f64::from(count) * f64::from(weights[event]) * hundredths
            };
            let delivered = points(BROUGHT, put_in * 100) + points(PUT_IN, put_in);
            let events: BTreeMap<usize, u32> = [
                (KILL, kills),
                (ASSIST, assists),
                (DEATH, deaths),
                (BROUGHT, put_in * 100),
                (CARRIER_KILL, carrier_kills),
                (PUT_IN, put_in),
            ]
            .into_iter()
            .filter(|&(_, count)| count > 0)
            .collect();
            entries.push(Entry {
                place,
                team: body.team,
                kind: self.kind(place),
                kills,
                deaths,
                assists,
                streak: self.best_streak[place],
                events,
                delivered,
                carrier_kill: points(CARRIER_KILL, carrier_kills),
                score,
                score_without_assists,
            });
        }
        let json = serde_json::json!({ "scoreboard": entries.iter().map(Entry::json).collect::<Vec<_>>() });
        let mut lines: Vec<String> = entries.iter().map(|e| e.line(&self.context)).collect();
        // The assist rule is a GUESS: its share of the kills is printed beside the live 0.19.
        let (kills, assists): (u32, u32) = entries.iter().fold((0, 0), |(k, a), e| (k + e.kills, a + e.assists));
        let teams: Vec<String> = (0..2u8)
            .map(|team| {
                let mine = entries.iter().filter(|e| e.team == team);
                let mine: Vec<&Entry> = mine.collect();
                format!(
                    "team {team} score {} (without assists {})",
                    mine.iter().map(|e| e.score).sum::<u32>(),
                    mine.iter().map(|e| e.score_without_assists).sum::<u32>()
                )
            })
            .collect();
        lines.push(format!(
            "owner-test scoreboard: {}; assists UNCALIBRATED {assists} of {kills} kills = {:.2} a kill (live 0.19; the rule is a GUESS: a teammate's hit within {} s, not used in a margin); a kill's assists by window (s): {:?}",
            teams.join(", "),
            if kills > 0 { f64::from(assists) / f64::from(kills) } else { 0.0 },
            ASSIST_MS / 1000,
            WINDOWS_S.iter().zip(self.by_window).map(|(w, n)| (*w, if kills > 0 { (100.0 * f64::from(n) / f64::from(kills)).round() / 100.0 } else { 0.0 })).collect::<Vec<_>>()
        ));
        vec![("scoreboard", Part::new(json, lines.join("\n  ")))]
    }
}

/// Whether the client counts the event in hundredths (`snapshot::HUNDREDTHS`).
fn static_info_hundredths(event: usize) -> bool {
    snapshot::HUNDREDTHS.contains(&event)
}

/// One player's line.
struct Entry {
    place: Place,
    team: u8,
    kind: Kind,
    kills: u32,
    deaths: u32,
    assists: u32,
    streak: u32,
    events: BTreeMap<usize, u32>,
    /// Points of the objective by kind, in the score's units.
    delivered: f64,
    carrier_kill: f64,
    score: u32,
    score_without_assists: u32,
}

impl Entry {
    fn line(&self, context: &str) -> String {
        let events: Vec<String> = self.events.iter().map(|(id, count)| format!("{id}: {count}")).collect();
        format!(
            "scoreboard: {context} | place {} team {} kind {} | kills {} deaths {} assists {} (UNCALIBRATED) streak {} | events {{{}}} | objective {{delivered: {:.1}, carrier_kill: {:.1}}} | score {} (without assists {})",
            self.place,
            self.team,
            self.kind.name(),
            self.kills,
            self.deaths,
            self.assists,
            self.streak,
            events.join(", "),
            self.delivered,
            self.carrier_kill,
            self.score,
            self.score_without_assists
        )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "place": self.place,
            "team": self.team,
            "kind": self.kind.name(),
            "kills": self.kills,
            "deaths": self.deaths,
            "assists": self.assists,
            "streak": self.streak,
            "events": self.events,
            "objective": { "delivered": self.delivered, "carrier_kill": self.carrier_kill },
            "score": self.score,
            "score_without_assists": self.score_without_assists,
        })
    }
}

const _: () = assert!(snapshot::EVENTS > PUT_IN);

/// A kill, an assist by a teammate who hit the victim in the window, a delivery and a carrier kill make the score the game's sum.
#[test]
pub(crate) fn a_players_line_is_the_games_own_score_of_what_he_did() {
    let (mut game, mut tally) = made_up(&[0, 0, 1]);
    // Bot 0 and bot 1 (team 0): bot 1 hits bot 2 at 2 s, bot 0 kills him at 5 s and kills a carrier; bot 0 delivered once.
    game.hit_log.push((2_000, 1, 2));
    game.stats[0] = (2, 0, 2);
    game.stats[2] = (0, 1, 0);
    game.carrier_kills.push((0, 2));
    game.put_in_log.push((0, 6_000));
    game.tick = 5_000;
    tally.step_with(&mut game, &[None, None, None]);
    game.hit_log.clear();
    game.kill_log.push((5_000, 0, 2));
    game.tick = 5_050;
    tally.step_with(&mut game, &[None, None, None]);
    let (_, json) = tally.finish(&game);
    let board = json["scoreboard"].as_array().expect("a list");
    assert_eq!(board.len(), 3);
    let weights = static_info::score_weights(mode::BATTERIES);
    // Bot 0: 2 kills (10 each), a bring (10 x 1.00), a put in (5), a carrier kill (5): 20 + 10 + 5 + 5.
    assert_eq!(board[0]["score"], 40);
    assert_eq!(board[0]["events"]["0"], 2);
    assert_eq!(board[0]["events"]["10"], 1);
    assert_eq!(board[0]["objective"]["delivered"], f64::from(weights[BROUGHT]) + f64::from(weights[PUT_IN]));
    // Bot 1 hit the victim of his teammate within the window: one assist (5).
    assert_eq!((board[1]["assists"].as_u64(), board[1]["score"].as_u64()), (Some(1), Some(5)));
    // The dead enemy scores nothing, and the kinds are bot.
    assert_eq!((board[2]["deaths"].as_u64(), board[2]["score"].as_u64()), (Some(1), Some(0)));
    assert_eq!(board[0]["kind"], "bot");
}

/// A hit outside the window, by an enemy of the killer, or by the killer himself is no assist.
#[test]
pub(crate) fn only_a_teammates_hit_inside_the_window_is_an_assist() {
    let (mut game, mut tally) = made_up(&[0, 0, 1, 1]);
    game.hit_log.extend([(1_000, 1, 2), (30_000, 0, 2), (30_000, 3, 2)]);
    game.tick = 30_000;
    tally.step_with(&mut game, &[None, None, None, None]);
    game.hit_log.clear();
    game.kill_log.push((31_000, 0, 2));
    game.tick = 31_000;
    tally.step_with(&mut game, &[None, None, None, None]);
    let assists = tally.topic::<Scoreboard>().assists.clone();
    // Bot 1's hit was 30 s before; bot 0 is the killer; bot 3 is an enemy of the killer.
    assert_eq!(assists, [0, 0, 0, 0]);
}
