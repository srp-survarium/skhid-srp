//! The falls of a metre or more in a tick: off the bot's route (no drop of it there) and taken by it; and the
//! stucks and falls off at a spot a teammate had already (`stuck::TeamSpots`).

use super::*;

/// A fall is past a ledge's edge with no drop link (the stand-in's `past_edge`) when the body went down there this long before it
/// is told, ms: in the tick it drops, or, a walk-off falling on at its walk speed (`SRP_BENCH_STAND_IN_WALKOFF`), where it lands, a
/// fall's flight later (1.0 s from 5 m, 1.4 s from 10 m).
pub(super) const PAST_EDGE_FALL_MS: u32 = 3_000;

#[derive(Default)]
pub(super) struct Falls {
    /// Falls off the route, and taken by it.
    falls: (u32, u32),
    /// Of the falls off the route, those past a ledge's edge with no drop link ahead (the stand-in's `past_edge`, the client's
    /// capsule walking off where the bots' body model held it: A269).
    past_edge: u32,
    /// Where the bots got stuck or fell off their route (team, place, where), and the stucks and falls off at a
    /// spot a teammate had already.
    marks: Vec<(u8, usize, Vec3)>,
    repeats: (u32, u32),
    /// The falls off the route that began while the bot held a stuck recovery's keys (`Bot::unstick`).
    unsticking: u32,
}

impl Falls {
    /// A stuck or a fall off its route of `place`'s bot at `at`: whether a teammate had one there already.
    fn mark(&mut self, run: &RouteRun, place: usize, at: Vec3) -> bool {
        let Some(team) = run.teams.get(place).copied().flatten() else {
            return false;
        };
        let again = self
            .marks
            .iter()
            .any(|&(t, p, spot)| t == team && p != place && super::super::stuck::near(spot, at));
        self.marks.push((team, place, at));
        again
    }
}

impl RouteTopic for Falls {
    fn tick(&mut self, run: &RouteRun, at: &TickAt<'_>) {
        let Some(fall) = at.fall else {
            return;
        };
        if fall.planned {
            self.falls.1 += 1;
        } else {
            self.falls.0 += 1;
            self.past_edge += u32::from(at.body.past_edge_down.is_some_and(|t| at.time.saturating_sub(t) <= PAST_EDGE_FALL_MS));
            self.repeats.1 += u32::from(self.mark(run, at.place, fall.from));
            self.unsticking += u32::from(at.bots.bots[at.place].as_ref().is_some_and(|bot| bot.unstick.is_some()));
        }
    }

    fn note(&mut self, run: &RouteRun, note: &Note) {
        if note.text.starts_with("is stuck at")
            && let Some(feet) = run.feet.get(note.place).copied().flatten()
        {
            self.repeats.0 += u32::from(self.mark(run, note.place, feet));
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!(
            "falls_off {} falls_planned {} stuck_again {} falls_again {}",
            self.falls.0, self.falls.1, self.repeats.0, self.repeats.1
        ));
        line.tail.push(format!("falls_off_unsticking {}", self.unsticking));
        line.ends.push(format!("falls_off_past_edge {}", self.past_edge));
    }
}
