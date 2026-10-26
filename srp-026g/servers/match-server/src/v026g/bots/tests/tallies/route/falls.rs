//! The falls of a metre or more in a tick: off the bot's route (no drop of it there) and taken by it; and the
//! stucks and falls off at a spot a teammate had already (`stuck::TeamSpots`).

use super::*;

#[derive(Default)]
pub(super) struct Falls {
    /// Falls off the route, and taken by it.
    falls: (u32, u32),
    /// Where the bots got stuck or fell off their route (team, place, where), and the stucks and falls off at a
    /// spot a teammate had already.
    marks: Vec<(u8, usize, Vec3)>,
    repeats: (u32, u32),
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
            self.repeats.1 += u32::from(self.mark(run, at.place, fall.from));
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
    }
}
