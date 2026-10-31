//! The bots' "is stuck" notes, and those of them by the walk keys alone; and what each note's reckoning-against-body gap was.
//!
//! A note is judged by the body 3 s on: it moved [`MOVED_M`] or more (a false alarm: the body was not stuck) or not (stuck). At the note
//! the gap between the bot's reckoned feet (its view at the last decision) and the real body is classed as `vertical` (the two on
//! floors [`VERTICAL_M`] or more apart in height), `horizontal` (across by [`HORIZONTAL_M`] or more, the body on a lip's floors or
//! not) or `none`.

use std::collections::BTreeMap;

use super::*;

/// The body moved this far in the 3 s after the note: not stuck.
const MOVED_M: f32 = 1.0;
const AFTER_MS: u32 = 3_000;
/// The reckoning and the body this far apart in height: another floor.
const VERTICAL_M: f32 = 0.5;
/// This far apart across.
const HORIZONTAL_M: f32 = 0.3;

struct Pending {
    place: usize,
    time: u32,
    feet: Vec3,
    /// The gap at the first tick of the note: `vertical_up` (the reckoning above the body), `vertical_down`, `horizontal_lip`,
    /// `horizontal`, `none`.
    gap: Option<&'static str>,
}

#[derive(Default)]
pub(super) struct Stuck {
    stuck: (u32, u32),
    pending: Vec<Pending>,
    /// Notes by `<stuck|false_alarm>_<gap>`.
    gaps: BTreeMap<String, u32>,
}

impl RouteTopic for Stuck {
    fn note(&mut self, _run: &RouteRun, note: &Note) {
        if note.text.starts_with("is stuck at") {
            self.stuck.0 += 1;
            self.stuck.1 += u32::from(note.text.contains("(walk keys held"));
            self.pending.push(Pending { place: note.place, time: note.time, feet: Vec3::ZERO, gap: None });
        }
    }

    fn tick(&mut self, _run: &RouteRun, at: &TickAt<'_>) {
        let mut done = Vec::new();
        for (i, p) in self.pending.iter_mut().enumerate().filter(|(_, p)| p.place == at.place) {
            if p.gap.is_none() && at.time >= p.time {
                let Some(bot) = at.bots.bots[at.place].as_ref() else {
                    continue;
                };
                let (est, real) = (bot.bench_guard_at.1, at.body.feet);
                p.feet = real;
                let dy = est.y - real.y;
                let lip = at.grid.locate(real, 1.0).is_some_and(|n| at.grid.near_lip(n) || at.grid.is_lip(n));
                p.gap = Some(if dy >= VERTICAL_M {
                    "vertical_up"
                } else if dy <= -VERTICAL_M {
                    "vertical_down"
                } else if est.distance_xz(real) >= HORIZONTAL_M {
                    if lip { "horizontal_lip" } else { "horizontal" }
                } else {
                    "none"
                });
            }
            if let Some(gap) = p.gap
                && at.time >= p.time + AFTER_MS
            {
                let moved = at.body.feet.distance_xz(p.feet) >= MOVED_M;
                *self.gaps.entry(format!("{}_{gap}", if moved { "false_alarm" } else { "stuck" })).or_default() += 1;
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            self.pending.swap_remove(i);
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!("stuck {} stuck_keys {}", self.stuck.0, self.stuck.1));
        let gaps: Vec<String> = std::mem::take(&mut self.gaps).into_iter().map(|(k, n)| format!("{k}={n}")).collect();
        line.tail.push(format!("stuck_note_gaps {}", if gaps.is_empty() { "-".to_owned() } else { gaps.join(",") }));
    }
}
