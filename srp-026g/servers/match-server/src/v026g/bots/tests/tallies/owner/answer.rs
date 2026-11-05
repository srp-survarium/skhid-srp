//! The owner counts of play 28, answer a sniper (`plays/answer.rs`), by team: the bot-ticks and the starts of the play, the
//! snipers killed (the victim's weapon class when he died) and of those by an answerer (a bot that had the play within
//! [`AFTER_MS`] of the kill), the answerers who died and of those to a sniper. What it gave or cost is read from the pair's score.
//!
//! And each run of the play (A210: a living bot's ticks with the play on one sniper): how far from him it started, whether and
//! when it stood at its spot (the play's peek) and how far from him that was, when it first pressed fire with its view on him
//! (within [`AIMED_DEG`]) and first hit him, whether it killed him, how old its own sense of him was at its last tick with the
//! play, and how the run ended: dead, the play replaced by another (named), dropped, or the match over.

use super::*;
use crate::v026g::bots::plays::{PlayKind, Running, TargetRef, answer};

/// A kill by, or the death of, a bot that had the play this recently counts as the play's, ms.
const AFTER_MS: u32 = 10_000;
/// Fire with the view this near the way to him counts as at him, degrees (the snipers topic's).
const AIMED_DEG: f32 = 10.0;

#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    /// Bot-ticks with the play assigned.
    ticks: u32,
    /// Starts of the play (a tick with it after one without).
    starts: u32,
    /// The team's snipers killed (by the other team).
    snipers_killed: u32,
    /// Enemy snipers the team's answerers killed.
    sniper_kills_by_answerers: u32,
    /// Deaths of the team's bots that had the play lately.
    answerers_died: u32,
    /// The same, to a sniper.
    answerers_died_to_sniper: u32,
}

/// What a bot's tick said of the play: on whom, whether it stood at its spot, and how old its own sense of him was (ms).
#[derive(Clone, Copy, Debug)]
struct Reading {
    sniper: Place,
    team: usize,
    peek: bool,
    sense_age: Option<u32>,
}

/// One run of the play, open: on whom, since when, and what it reached so far (ms are the game's).
#[derive(Clone, Copy, Debug)]
struct OpenRun {
    sniper: Place,
    team: usize,
    start: u32,
    /// Its distance from him at the start, m.
    start_m: f32,
    /// When it first stood at its spot, and its distance from him then.
    reached: Option<(u32, f32)>,
    /// When it first pressed fire with its view on him, and when its first hit on him landed.
    fired: Option<u32>,
    hit: Option<u32>,
    killed: bool,
    /// The age of the bot's own sense of him at its last tick with the play, ms (none: no own sense of him).
    sense_age: Option<u32>,
}

/// How a run ended.
#[derive(Clone, Copy, Debug)]
enum RunEnd {
    Died,
    /// Another play took over (a run of the play on another sniper included).
    Replaced(PlayKind),
    /// The play let go with nothing after it.
    Dropped,
    MatchOver,
}

impl RunEnd {
    fn label(self) -> String {
        match self {
            Self::Died => "died".into(),
            Self::Replaced(kind) => format!("replaced by {kind:?}"),
            Self::Dropped => "dropped".into(),
            Self::MatchOver => "match over".into(),
        }
    }
}

pub(super) struct AnswerCounts {
    by: [Counts; 2],
    /// By place: whether the last reading had the play, and the tick it last did.
    on: Vec<bool>,
    last_on: Vec<Option<u32>>,
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    /// By place: this tick's reading and the bot's play then, and its open run.
    reading: Vec<Option<Reading>>,
    next: Vec<Option<PlayKind>>,
    open: Vec<Option<OpenRun>>,
    /// The runs done: their records; by what they reached (runs, at the spot, fired at him, hit him, killed him); by end.
    runs: Vec<serde_json::Value>,
    reached: [u32; 5],
    ends: std::collections::BTreeMap<String, u32>,
}

impl Default for AnswerCounts {
    fn default() -> Self {
        crate::v026g::bots::plays::answer::reset_ends();
        Self {
            by: Default::default(),
            on: Vec::new(),
            last_on: Vec::new(),
            kills_seen: 0,
            last_kill: None,
            reading: Vec::new(),
            next: Vec::new(),
            open: Vec::new(),
            runs: Vec::new(),
            reached: [0; 5],
            ends: Default::default(),
        }
    }
}

impl AnswerCounts {
    /// A run done at `tick`: its record and the counts.
    fn close(&mut self, run: OpenRun, tick: u32, end: RunEnd) {
        let s = |t: u32| f64::from(t.saturating_sub(run.start)) / 1000.0;
        let m = |m: f32| (m * 10.0).round() / 10.0;
        self.reached[0] += 1;
        self.reached[1] += u32::from(run.reached.is_some());
        self.reached[2] += u32::from(run.fired.is_some());
        self.reached[3] += u32::from(run.hit.is_some());
        self.reached[4] += u32::from(run.killed);
        *self.ends.entry(end.label()).or_default() += 1;
        self.runs.push(serde_json::json!({
            "team": run.team,
            "start_m": m(run.start_m),
            "dur_s": s(tick),
            "reached_s": run.reached.map(|(t, _)| s(t)),
            "reached_m": run.reached.map(|(_, d)| m(d)),
            "fired_s": run.fired.map(s),
            "hit_s": run.hit.map(s),
            "killed": run.killed,
            "sense_age_s": run.sense_age.map(|a| f64::from(a) / 1000.0),
            "end": end.label(),
        }));
    }

    /// This tick's runs: a run whose bot died, let the play go or turned to another sniper ends; a living bot with the play
    /// and no run starts one; the open runs take what they reached (`hits` and `kills`: this tick's, shooter and victim).
    fn runs_tick(&mut self, game: &Game, hits: &[(u32, Place, Place)], kills: &[(u32, Place, Place)]) {
        let now = game.tick;
        for place in 0..self.open.len() {
            let alive = game.bodies.get(place).is_some_and(|b| b.alive);
            let reading = self.reading[place];
            if let Some(run) = self.open[place]
                && (!alive || reading.is_none_or(|r| r.sniper != run.sniper))
            {
                self.open[place] = None;
                let end = match self.next[place] {
                    _ if !alive => RunEnd::Died,
                    Some(kind) => RunEnd::Replaced(kind),
                    None => RunEnd::Dropped,
                };
                self.close(run, now, end);
            }
            let (Some(r), true) = (reading, alive) else {
                continue;
            };
            let (Some(body), Some(him)) = (game.bodies.get(place), game.bodies.get(r.sniper)) else {
                continue;
            };
            let d = (him.feet - body.feet).length();
            let run = self.open[place].get_or_insert(OpenRun {
                sniper: r.sniper,
                team: r.team,
                start: now,
                start_m: d,
                reached: None,
                fired: None,
                hit: None,
                killed: false,
                sense_age: None,
            });
            if r.peek && run.reached.is_none() {
                run.reached = Some((now, d));
            }
            run.sense_age = r.sense_age;
            if body.keys & key::FIRE != 0 && off_deg(body.yaw, body.feet, him.feet) <= AIMED_DEG {
                run.fired.get_or_insert(now);
            }
            if let Some(&(t, _, _)) = hits.iter().find(|h| h.1 == place && h.2 == r.sniper && h.0 >= run.start) {
                run.hit.get_or_insert(t);
            }
            run.killed |= kills.iter().any(|&(_, killer, victim)| killer == place && victim == r.sniper);
        }
    }
}

impl Topic for AnswerCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, tick: u32) {
        if self.on.len() <= place {
            self.on.resize(place + 1, false);
            self.last_on.resize(place + 1, None);
            self.reading.resize(place + 1, None);
            self.next.resize(place + 1, None);
            self.open.resize(place + 1, None);
        }
        self.reading[place] = None;
        self.next[place] = None;
        let Some(bot) = bot else {
            return;
        };
        let has = bot.assignment.as_ref().is_some_and(|a| a.kind == PlayKind::Answer);
        let team = usize::from(bot.team).min(1);
        if has {
            self.by[team].ticks += 1;
            self.by[team].starts += u32::from(!self.on[place]);
            self.last_on[place] = Some(tick);
        }
        self.on[place] = has;
        self.next[place] = bot.assignment.as_ref().map(|a| a.kind);
        self.reading[place] = bot.assignment.as_ref().and_then(|a| match (a.kind, a.target) {
            (PlayKind::Answer, TargetRef::Place(sniper)) => Some(Reading {
                sniper,
                team,
                peek: matches!(bot.running, Some(Running::Answer(answer::State::Peek { .. }))),
                sense_age: bot
                    .memory
                    .tracks()
                    .iter()
                    .find(|t| t.who == sniper)
                    .and_then(|t| t.own)
                    .map(|(_, time)| tick.saturating_sub(time)),
            }),
            _ => None,
        });
    }

    fn end(&mut self, _run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new: Vec<_> = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        let lately = |last: &[Option<u32>], place: Place, t: u32| {
            last.get(place).copied().flatten().is_some_and(|at| t >= at && t - at <= AFTER_MS)
        };
        for &(t, killer, victim) in &new {
            let (Some(k), Some(v)) = (game.bodies.get(killer), game.bodies.get(victim)) else {
                continue;
            };
            let (kt, vt) = (usize::from(k.team).min(1), usize::from(v.team).min(1));
            let killer_sniper = matches!(k.gun.class(), "marksman" | "bolt");
            if matches!(v.gun.class(), "marksman" | "bolt") {
                self.by[vt].snipers_killed += 1;
                self.by[kt].sniper_kills_by_answerers += u32::from(lately(&self.last_on, killer, t));
            }
            if lately(&self.last_on, victim, t) {
                self.by[vt].answerers_died += 1;
                self.by[vt].answerers_died_to_sniper += u32::from(killer_sniper);
            }
        }
        let hits = game.hit_log.clone();
        self.runs_tick(game, &hits, &new);
    }

    fn parts(&mut self, _run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        for place in 0..self.open.len() {
            if let Some(run) = self.open[place].take() {
                self.close(run, game.tick, RunEnd::MatchOver);
            }
        }
        let one = |c: &Counts| {
            serde_json::json!({
                "ticks": c.ticks,
                "starts": c.starts,
                "snipers_killed": c.snipers_killed,
                "sniper_kills_by_answerers": c.sniper_kills_by_answerers,
                "answerers_died": c.answerers_died,
                "answerers_died_to_sniper": c.answerers_died_to_sniper,
            })
        };
        let [a, b] = self.by;
        let ends: std::collections::BTreeMap<_, _> = crate::v026g::bots::plays::answer::ends().into_iter().collect();
        let [runs, at_spot, fired, hit, killed] = self.reached;
        let json = serde_json::json!({
            "answer_ends": ends,
            "answer_ticks": a.ticks + b.ticks,
            "answer_starts": a.starts + b.starts,
            "answer": { "by_team": { "0": one(&a), "1": one(&b) } },
            "answer_runs": self.runs,
            "answer_run_ends": self.ends,
        });
        let text = format!(
            "owner-test answer: the play held {} bot-ticks in {} starts (team 0 {} / team 1 {}); snipers killed {} (team 0's {} / team 1's {}), by an answerer {} (team 0's {} / team 1's {}); answerers who died {} ({} to a sniper); runs {runs}: at the spot {at_spot}, fired at him {fired}, hit him {hit}, killed him {killed}",
            a.ticks + b.ticks,
            a.starts + b.starts,
            a.starts,
            b.starts,
            a.snipers_killed + b.snipers_killed,
            a.snipers_killed,
            b.snipers_killed,
            a.sniper_kills_by_answerers + b.sniper_kills_by_answerers,
            a.sniper_kills_by_answerers,
            b.sniper_kills_by_answerers,
            a.answerers_died + b.answerers_died,
            a.answerers_died_to_sniper + b.answerers_died_to_sniper,
        );
        vec![("answer", Part::new(json, text))]
    }
}
