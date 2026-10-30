//! The owner counts of play 28, answer a sniper (`plays/answer.rs`), by team: the bot-ticks and the starts of the play, the
//! snipers killed (the victim's weapon class when he died) and of those by an answerer (a bot that had the play within
//! [`AFTER_MS`] of the kill), the answerers who died and of those to a sniper. What it gave or cost is read from the pair's score.

use super::*;
use crate::v026g::bots::plays::PlayKind;

/// A kill by, or the death of, a bot that had the play this recently counts as the play's, ms.
const AFTER_MS: u32 = 10_000;

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

pub(super) struct AnswerCounts {
    by: [Counts; 2],
    /// By place: whether the last reading had the play, and the tick it last did.
    on: Vec<bool>,
    last_on: Vec<Option<u32>>,
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
}

impl Default for AnswerCounts {
    fn default() -> Self {
        crate::v026g::bots::plays::answer::reset_ends();
        Self { by: Default::default(), on: Vec::new(), last_on: Vec::new(), kills_seen: 0, last_kill: None }
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
        }
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
        for (t, killer, victim) in new {
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
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
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
        let json = serde_json::json!({
            "answer_ends": ends,
            "answer_ticks": a.ticks + b.ticks,
            "answer_starts": a.starts + b.starts,
            "answer": { "by_team": { "0": one(&a), "1": one(&b) } },
        });
        let text = format!(
            "owner-test answer: the play held {} bot-ticks in {} starts (team 0 {} / team 1 {}); snipers killed {} (team 0's {} / team 1's {}), by an answerer {} (team 0's {} / team 1's {}); answerers who died {} ({} to a sniper)",
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
