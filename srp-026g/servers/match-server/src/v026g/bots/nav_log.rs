//! A285: the bots' falls and route jumps as lines of the server's log, so a play log shows them as the bench does.
//!
//! The server judged both on the client's worlds already (`drive.rs` `jump_followed`, `stuck.rs` `track_fall`) but wrote them only to
//! the per-bot traces, so the owner's match logs had no fall or jump line at all (A283, fx-grid: "no falls in the log" was a logging
//! gap). Two lines, each a bot's [`super::Note`] (the log's `player N (the server's bot) at T ms ...`):
//!
//! - `nav: route jump up|short|gave up: from (x, y, z) to (x, y, z), rise R m, try N, after MS ms (then)`: what followed each jump the
//!   route pressed: `up` once a world showed the body up the jump (`motor::is_up`), `gave up` when the watch ended stuck or by a new
//!   life, `short` otherwise (it went on below, or jumped again);
//! - `nav: fell D m from (x, y, z) to (x, y, z) in MS ms, the model had it at Y (unplanned|planned)`: two worlds of one life at most
//!   [`FALL_GAP_MS`] apart with the feet [`FALL_M`] or more lower. `unplanned` when the bot's own reckoning for the later world's tick
//!   was [`MODEL_M`] or more above the world's feet (the body went over an edge the model kept it on: a fall past an edge);
//!   `planned` when the model came down too (a drop the route took); no word when the reckoning has no tick for that world.
//!
//! Decision-neutral: the lines are made from what the bot already holds and go only to the notes; nothing reads them back. At most one
//! line of each kind per bot per [`EVERY_MS`]; the lines held back are counted into the next one (`(+N held)`).

use mapdata::Vec3;

/// Every line begins with this (the bench's decision hash leaves these lines out: they decide nothing).
pub const PREFIX: &str = "nav: ";

/// A drop this deep between two worlds is a fall, m (a storey; the client's step is 0.80).
pub const FALL_M: f32 = 1.5;
/// Two worlds further apart than this say nothing of a fall between them, ms (the stuck watch's `FIX_GAP_MS`).
pub const FALL_GAP_MS: u32 = 3_000;
/// The reckoning this far above the world's feet did not expect the drop, m.
pub const MODEL_M: f32 = 1.0;
/// One line of a kind per bot per this long at most, ms.
pub const EVERY_MS: u32 = 2_000;

/// What followed a jump the route pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpOutcome {
    Up,
    Short,
    GaveUp,
}

impl JumpOutcome {
    /// `up`: a world showed the body up the jump; else by how the watch ended (`then`, `drive.rs` `jump_followed`).
    pub fn of(up: bool, then: &str) -> Self {
        if up {
            Self::Up
        } else if then.starts_with("stuck") || then == "a new life" {
            Self::GaveUp
        } else {
            Self::Short
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Short => "short",
            Self::GaveUp => "gave up",
        }
    }
}

/// A route jump's line.
pub fn jump_line(outcome: JumpOutcome, take_off: Vec3, landing: Vec3, tries: u32, after_ms: u32, then: &str) -> String {
    format!(
        "{PREFIX}route jump {}: from {} to {}, rise {:.2} m, try {tries}, after {after_ms} ms ({then})",
        outcome.word(),
        at(take_off),
        at(landing),
        landing.y - take_off.y
    )
}

/// A fall between two worlds of one life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fall {
    pub from: Vec3,
    pub to: Vec3,
    pub gap_ms: u32,
    /// The reckoning's feet at the later world's tick, if the trail had it.
    pub reckoned: Option<Vec3>,
}

impl Fall {
    /// The fall from the world `before` (tick, feet) to the one `now`, if it is one.
    pub fn between(before: (u32, Vec3), now: (u32, Vec3), reckoned: Option<Vec3>) -> Option<Self> {
        let ((t0, from), (t1, to)) = (before, now);
        (t1 > t0 && t1 - t0 <= FALL_GAP_MS && from.y - to.y >= FALL_M).then(|| Self { from, to, gap_ms: t1 - t0, reckoned })
    }

    /// Whether the model expected it: false when its feet stayed [`MODEL_M`] or more above the world's.
    pub fn planned(&self) -> Option<bool> {
        self.reckoned.map(|r| r.y - self.to.y < MODEL_M)
    }

    pub fn line(&self) -> String {
        let model = match (self.reckoned, self.planned()) {
            (Some(r), Some(planned)) => format!(", the model had it at {:.1} ({})", r.y, if planned { "planned" } else { "unplanned" }),
            _ => String::new(),
        };
        format!("{PREFIX}fell {:.1} m from {} to {} in {} ms{model}", self.from.y - self.to.y, at(self.from), at(self.to), self.gap_ms)
    }
}

fn at(p: Vec3) -> String {
    format!("({:.1}, {:.1}, {:.1})", p.x, p.y, p.z)
}

/// Which line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jump,
    Fall,
}

/// One bot's lines waiting for the next poll, and the rate limit.
#[derive(Clone, Debug, Default)]
pub struct NavLog {
    /// Per kind: the last line's time and the lines held back since.
    last: [Option<(u32, u32)>; 2],
    pending: Vec<(u32, String)>,
}

impl NavLog {
    /// A line of `kind` at `time`: kept for the next poll, or held back (counted) within [`EVERY_MS`] of the last one.
    pub fn push(&mut self, kind: Kind, time: u32, text: String) {
        let slot = &mut self.last[kind as usize];
        match slot {
            Some((at, held)) if time < *at + EVERY_MS => *held += 1,
            _ => {
                let held = slot.map_or(0, |(_, held)| held);
                let text = if held > 0 { format!("{text} (+{held} held)") } else { text };
                *slot = Some((time, 0));
                self.pending.push((time, text));
            }
        }
    }

    /// The lines kept since the last call.
    pub fn take(&mut self) -> Vec<(u32, String)> {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jump_outcome_is_up_short_or_given_up() {
        assert_eq!(JumpOutcome::of(true, "went on"), JumpOutcome::Up);
        assert_eq!(JumpOutcome::of(true, "stuck: BackOff"), JumpOutcome::Up);
        assert_eq!(JumpOutcome::of(false, "went on"), JumpOutcome::Short);
        assert_eq!(JumpOutcome::of(false, "jumped again"), JumpOutcome::Short);
        assert_eq!(JumpOutcome::of(false, "stuck: Sidestep"), JumpOutcome::GaveUp);
        assert_eq!(JumpOutcome::of(false, "a new life"), JumpOutcome::GaveUp);
        let line = jump_line(JumpOutcome::Short, Vec3::new(57.1, 20.9, -3.8), Vec3::new(57.6, 22.0, -3.8), 2, 1500, "went on");
        assert_eq!(line, "nav: route jump short: from (57.1, 20.9, -3.8) to (57.6, 22.0, -3.8), rise 1.10 m, try 2, after 1500 ms (went on)");
    }

    #[test]
    fn a_fall_is_a_storey_down_between_close_worlds() {
        let top = Vec3::new(-25.0, 38.7, 140.7);
        let bottom = Vec3::new(-23.0, 35.2, 141.0);
        // The model kept the body on the rim: a fall past an edge.
        let fall = Fall::between((10_000, top), (11_500, bottom), Some(Vec3::new(-23.0, 38.6, 141.0))).unwrap();
        assert_eq!(fall.planned(), Some(false));
        assert_eq!(fall.line(), "nav: fell 3.5 m from (-25.0, 38.7, 140.7) to (-23.0, 35.2, 141.0) in 1500 ms, the model had it at 38.6 (unplanned)");
        // The model came down too: a drop the route took.
        let drop = Fall::between((10_000, top), (11_500, bottom), Some(Vec3::new(-23.0, 35.6, 141.0))).unwrap();
        assert_eq!(drop.planned(), Some(true));
        assert!(drop.line().ends_with("(planned)"));
        assert_eq!(Fall::between((10_000, top), (11_500, bottom), None).unwrap().planned(), None);
        // Not a fall: too shallow, too far apart, or older.
        assert_eq!(Fall::between((10_000, top), (11_000, Vec3::new(-25.0, 37.5, 140.7)), None), None);
        assert_eq!(Fall::between((10_000, top), (13_500, bottom), None), None);
        assert_eq!(Fall::between((11_500, top), (10_000, bottom), None), None);
    }

    #[test]
    fn lines_of_a_kind_come_at_most_every_two_seconds_and_count_the_held() {
        let mut log = NavLog::default();
        log.push(Kind::Jump, 1_000, "a".into());
        log.push(Kind::Jump, 1_500, "b".into());
        log.push(Kind::Fall, 1_600, "f".into());
        log.push(Kind::Jump, 2_900, "c".into());
        log.push(Kind::Jump, 3_000, "d".into());
        assert_eq!(log.take(), vec![(1_000, "a".to_string()), (1_600, "f".to_string()), (3_000, "d (+2 held)".to_string())]);
        assert!(log.take().is_empty());
    }
}
