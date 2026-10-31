//! The owner counts of the Breach (`threat.breach`, `bots/breach.rs`, design-p1c §1), by team: spots made due by a failed
//! Flank, Breaches opened (with a pinner, on an objective spot) and the refusals by reason (no cover point, no pair, no gather
//! point, too few free crossers, the cap, the spot memory: conceded or like the last try), the go (and the gather's wait), the
//! hold, the first crosser death and the trade of it (and its time), how the job ended (his death by the job or by another,
//! the spot unheld, the failures by reason), the pinners lost and the crossers down. A 0 where the switch is on and a Flank
//! failed shows the job never ran.
//!
//!
//! And the trap at a cleared camper's point (`traps.at_cleared`, `traps::cleared`), by team: points noted after a Breach's or a
//! flanker's kill, kills with no job bot that had a trap, thinks with the point wished, waits for it on the route, plans placed
//! on it; of those plans, the traps put down (the placer's within 5 s of its plan) and their catches: enemy, mate, own.
//!
//! The counts are the thread's (`breach::tally::counted`, `traps::cleared`), read as the difference from the topic's start: a
//! match runs on one thread.

use super::*;
use crate::v026g::bots::breach::tally;
use crate::v026g::bots::tests::Kill as KillKind;
use crate::v026g::bots::tests::standin_traps::Event as TrapEvent;
use crate::v026g::bots::traps::cleared;

/// The Breach's counts at the start of the match, for the difference at its end.
pub(super) struct BreachCounts {
    start: [[u64; tally::ALL]; 2],
    cleared: [[u64; 6]; 2],
    plans_from: usize,
}

impl Default for BreachCounts {
    fn default() -> Self {
        Self { start: tally::counted(), cleared: cleared::counted(), plans_from: cleared::plans(0).len() }
    }
}

impl Topic for BreachCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, _run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("breach", self.part()), ("trap_cleared", self.traps(game))]
    }
}

impl BreachCounts {
    fn part(&self) -> Part {
        let now = tally::counted();
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for team in 0..2usize {
            let n = |i: usize| now[team][i] - self.start[team][i];
            let mut t = serde_json::Map::new();
            for (i, name) in tally::NAMES.iter().enumerate() {
                t.insert((*name).into(), n(i).into());
            }
            t.insert("pinner_lost".into(), n(tally::PINNER_LOST).into());
            t.insert("crosser_downs".into(), n(tally::CROSSER_DOWNS).into());
            t.insert("objective_waits".into(), n(tally::OBJECTIVE_WAITS).into());
            let get = |name: &str| t.get(name).and_then(serde_json::Value::as_u64).unwrap_or(0);
            let went = get("went");
            let traded = get("traded");
            text.push(format!(
                "team {team}: due {}, opened {} (pinner {}, objective {}), refused no cover {} / no pair {} / no gather {} / no crossers {} / cap {} / conceded {} / like the last {}; went {} (gather wait mean {:.1} s), held {}, first deaths {} traded {} (mean {:.1} s); ended his death by the job {} / by another {} / unheld {}; failed holder down {} / untraded {} / gather timeout {} / aborted {} / time {}; pinners lost {}, crossers down {}; objective player waits (thinks) {}",
                get("due"),
                get("opened"),
                get("opened_with_pinner"),
                get("opened_objective"),
                get("no_cover"),
                get("no_pair"),
                get("no_gather"),
                get("no_crossers"),
                get("cap"),
                get("conceded"),
                get("like_the_last"),
                went,
                get("gather_wait_ms") as f64 / 1000.0 / went.max(1) as f64,
                get("holding"),
                get("first_death"),
                traded,
                get("trade_ms") as f64 / 1000.0 / traded.max(1) as f64,
                get("shooter_killed_by_job"),
                get("shooter_died_other"),
                get("unheld"),
                get("failed_holder_down"),
                get("failed_untraded"),
                get("failed_gather_timeout"),
                get("failed_aborted"),
                get("failed_time"),
                n(tally::PINNER_LOST),
                n(tally::CROSSER_DOWNS),
                n(tally::OBJECTIVE_WAITS),
            ));
            json.insert(format!("team{team}"), serde_json::Value::Object(t));
        }
        Part::new(serde_json::Value::Object(json), format!("owner-test breach: {}", text.join("; ")))
    }

    /// The trap at a cleared point: its counts by team, and the catches of the traps put down on its plans.
    fn traps(&self, game: &Game) -> Part {
        let now = cleared::counted();
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        // By the placer's team: put down, and caught [enemy, mate, own].
        let mut down = [0u64; 2];
        let mut caught = [[0u64; 3]; 2];
        for (placer, at) in cleared::plans(self.plans_from) {
            let Some(tick) = game.traps.events.iter().find_map(|e| match *e {
                TrapEvent::Placed { tick, place, .. } if place == placer && (at..=at + 5_000).contains(&tick) => Some(tick),
                _ => None,
            }) else {
                continue;
            };
            let team = usize::from(game.bodies.get(placer).map_or(0, |b| b.team) & 1);
            down[team] += 1;
            for e in &game.traps.events {
                if let TrapEvent::Caught { placer: p, placed, kind, .. } = *e
                    && p == placer
                    && placed == tick
                {
                    caught[team][match kind {
                        KillKind::Enemy => 0,
                        KillKind::Mate => 1,
                        KillKind::Own => 2,
                    }] += 1;
                }
            }
        }
        for team in 0..2usize {
            let n = |i: usize| now[team][i] - self.cleared[team][i];
            let mut t = serde_json::Map::new();
            for (i, name) in cleared::NAMES.iter().enumerate() {
                t.insert((*name).into(), n(i).into());
            }
            t.insert("put_down".into(), down[team].into());
            t.insert("caught_enemy".into(), caught[team][0].into());
            t.insert("caught_mate".into(), caught[team][1].into());
            t.insert("caught_own".into(), caught[team][2].into());
            text.push(format!(
                "team {team}: cleared points noted after a Breach {} / a flanker {} (no job bot with a trap {}), thinks wished {}, waited {}, planned {}, put down {}, caught enemy {} / mate {} / own {}",
                n(cleared::NOTED_BREACH),
                n(cleared::NOTED_FLANK),
                n(cleared::NO_PLACER),
                n(cleared::THINKS),
                n(cleared::WAITED),
                n(cleared::PLACED),
                down[team],
                caught[team][0],
                caught[team][1],
                caught[team][2],
            ));
            json.insert(format!("team{team}"), serde_json::Value::Object(t));
        }
        Part::new(serde_json::Value::Object(json), format!("owner-test trap at a cleared point: {}", text.join("; ")))
    }
}
