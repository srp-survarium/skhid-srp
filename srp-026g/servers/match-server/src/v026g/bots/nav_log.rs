//! A285: the bots' falls and route jumps as lines of the server's log, so a play log shows them as the bench does.
//!
//! The server judged both on the client's worlds already (`drive.rs` `jump_followed`, `stuck.rs` `track_fall`) but wrote them only to
//! the per-bot traces, so the owner's match logs had no fall or jump line at all (A283, fx-grid: "no falls in the log" was a logging
//! gap). Two lines, each a bot's [`super::Note`] (the log's `player N (the server's bot) at T ms ...`):
//!
//! - `nav: route jump up|short|down|not down|gave up: from (x, y, z) to (x, y, z), rise R m, try N, after MS ms (then)`: what
//!   followed each jump the route pressed. A jump up (or level): `up` once a world showed the body up the jump (`motor::is_up`),
//!   `gave up` when the watch ended stuck or by a new life, `short` otherwise (it went on below, or jumped again). A jump down (a gap
//!   link to a floor more than [`motor::UP_M`] lower, A293): `down` once a world showed the feet on the landing's level
//!   (`motor::on_landing`), `gave up` as above, `not down` otherwise (the body stayed on top, or came down elsewhere). `is_up` has
//!   no upper bound, so it took a body still on the top of a jump down for one across: 38 of 45 such `up` lines never left the top
//!   (A289, match 0x8681e02);
//! - `nav: fell D m from (x, y, z) to (x, y, z) in MS ms, the model had it at Y (unplanned|planned)`: two worlds of one life at most
//!   [`FALL_GAP_MS`] apart with the feet [`FALL_M`] or more lower. `unplanned` when the bot's own reckoning for the later world's tick
//!   was [`MODEL_M`] or more above the world's feet (the body went over an edge the model kept it on: a fall past an edge); no
//!   word when the reckoning has no tick for that world. A drop the model came down too (a planned drop, a walk down a slope
//!   between two worlds) is not written: on a 2 s world cadence those are most of the drops (681 of 724 in a bench match on
//!   Mamayev Kurgan) and would bury the falls past edges.
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
    /// A jump up or level: a world showed the body up it.
    Up,
    /// A jump up or level: no world showed it up (it went on below, or jumped again).
    Short,
    /// A jump down: a world showed the feet on the landing's level.
    Down,
    /// A jump down: no world showed the feet on the landing (it stayed on top, or came down elsewhere).
    NotDown,
    /// The watch ended stuck or by a new life, the body not up (or not down).
    GaveUp,
}

impl JumpOutcome {
    /// For a jump that rises `rise` m: a jump down (lower by more than [`super::motor::UP_M`]) by `landed` (a world showed the
    /// feet on its landing, `motor::on_landing`), any other by `up` (a world showed the body up it, `motor::is_up`); else by how
    /// the watch ended (`then`, `drive.rs` `jump_followed`).
    pub fn of(rise: f32, up: bool, landed: bool, then: &str) -> Self {
        let down = rise < -super::motor::UP_M;
        let gave_up = then.starts_with("stuck") || then == "a new life";
        let seen = if down { landed } else { up };
        match (down, seen, gave_up) {
            (true, true, _) => Self::Down,
            (false, true, _) => Self::Up,
            (_, false, true) => Self::GaveUp,
            (true, false, false) => Self::NotDown,
            (false, false, false) => Self::Short,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Short => "short",
            Self::Down => "down",
            Self::NotDown => "not down",
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

pub(crate) fn at(p: Vec3) -> String {
    format!("({:.1}, {:.1}, {:.1})", p.x, p.y, p.z)
}

/// Which line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jump,
    Fall,
    /// A294's drift line (`nav_drift.rs`).
    Drift,
    /// [owner-5] A job role's offer to this bot and what kept it from being one (`Ctx::job_offer`).
    Role,
    /// [owner-5] A sprint gap or rim link pressed without sprint, and the gate that held the sprint (`motor.rs`).
    Sprint,
    /// [owner-5] A zone battery refused by its crossing's price: the term that refused it against its limit (`Ctx::take_block`).
    Zone,
    /// [owner-5] A zone spawner the battery search skipped, and the filter that dropped it (`collect.rs`).
    Spawner,
    /// [owner-5] A route search that found no way, and what its searches said (`drive.rs`, `search.rs`).
    Route,
}

/// How many kinds of line there are ([`Kind`]).
const KINDS: usize = 8;

/// [owner-5] At most this many lines of a kind per bot per match (the first ones; the rest are counted into the last): the
/// owner-5 kinds fire on events that can repeat all match (a battery refused at every poll, a role offered and refused
/// again), so a match's log stays a few hundred of these lines at most.
pub const MAX_PER_KIND: u32 = 60;

/// One bot's lines waiting for the next poll, and the rate limit.
#[derive(Clone, Debug, Default)]
pub struct NavLog {
    /// Per kind: the last line's time and the lines held back since.
    last: [Option<(u32, u32)>; KINDS],
    /// [owner-5] Per kind: the lines written this match, against [`MAX_PER_KIND`].
    written: [u32; KINDS],
    /// [owner-5] The keys already written ([`Self::push_once`]): one line per bot per decision, not one per poll.
    seen: std::collections::HashSet<(u8, u64)>,
    pending: Vec<(u32, String)>,
}

impl NavLog {
    /// A line of `kind` at `time`: kept for the next poll, or held back (counted) within [`EVERY_MS`] of the last one. The
    /// owner-5 kinds also stop at [`MAX_PER_KIND`] a match.
    pub fn push(&mut self, kind: Kind, time: u32, text: String) {
        let capped = !matches!(kind, Kind::Jump | Kind::Fall | Kind::Drift);
        if capped && self.written[kind as usize] >= MAX_PER_KIND {
            return;
        }
        let slot = &mut self.last[kind as usize];
        match slot {
            Some((at, held)) if time < *at + EVERY_MS => *held += 1,
            _ => {
                let held = slot.map_or(0, |(_, held)| held);
                let text = if held > 0 { format!("{text} (+{held} held)") } else { text };
                *slot = Some((time, 0));
                self.written[kind as usize] += 1;
                self.pending.push((time, text));
            }
        }
    }

    /// [owner-5] A line of `kind` for the decision `key` (a hash of what the line is about and its verdict): written the first
    /// time the key is seen this match ([`Self::push`]'s limits on top), nothing after.
    pub fn push_once(&mut self, kind: Kind, key: u64, time: u32, text: String) {
        if self.seen.insert((kind as u8, key)) {
            self.push(kind, time, text);
        }
    }

    /// The lines kept since the last call.
    pub fn take(&mut self) -> Vec<(u32, String)> {
        std::mem::take(&mut self.pending)
    }
}

/// [owner-5] A job role's offer: `nav: job role <role> on <target> (<play>): <outcome>[, score S]` (the outcome: offered,
/// no_spot, set_aside, dry, low_health; the score when offered, for the play that outscored it).
pub fn role_line(role: &str, target: impl std::fmt::Debug, play: impl std::fmt::Debug, outcome: &str, score: Option<f32>) -> String {
    let score = score.map_or(String::new(), |s| format!(", score {s:.2}"));
    format!("{PREFIX}job role {role} on {target:?} ({play:?}): {outcome}{score}")
}

/// [owner-5] A sprint link pressed without sprint: `nav: sprint link pressed unsprinted: from (..) to (..), before B m, gate G`.
pub fn sprint_line(take_off: Vec3, landing: Vec3, before: f32, gate: &str) -> String {
    format!("{PREFIX}sprint link pressed unsprinted: from {} to {}, before {before:.2} m, gate {gate}", at(take_off), at(landing))
}

/// [owner-5] A zone battery refused by its crossing's price: `nav: zone battery at (..) refused: <term>`.
pub fn zone_line(battery: Vec3, term: &str) -> String {
    format!("{PREFIX}zone battery at {} refused: {term}", at(battery))
}

/// [owner-5] A zone spawner the battery search skipped: `nav: zone spawner <i> at (..) not searched: <filter>`.
pub fn spawner_line(index: usize, item: Vec3, filter: &str) -> String {
    format!("{PREFIX}zone spawner {index} at {} not searched: {filter}", at(item))
}

/// [owner-5] A route search that found no way: `nav: no way from (..) to (..): <verdict>`.
pub fn route_line(from: Option<Vec3>, goal: Vec3, verdict: &str) -> String {
    let from = from.map_or("no floor".to_string(), at);
    format!("{PREFIX}no way from {from} to {}: {verdict}", at(goal))
}

/// [owner-5] A key for [`NavLog::push_once`] from the line's parts.
pub fn key(parts: impl std::hash::Hash) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault};
    BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(parts)
}

/// [owner-5] Lines a play writes while it is asked (`Ctx::lines`, the candidates or a step), for its bot's [`NavLog`] once
/// the poll lets go of the bot: `(kind, key, text)`; a key of `None` is a plain [`NavLog::push`].
#[derive(Debug, Default)]
pub struct Lines(std::cell::RefCell<Vec<(Kind, Option<u64>, String)>>);

impl Lines {
    pub fn push(&self, kind: Kind, key: Option<u64>, text: String) {
        self.0.borrow_mut().push((kind, key, text));
    }

    /// The lines written, for [`Self::write`].
    pub fn take(&self) -> Vec<(Kind, Option<u64>, String)> {
        std::mem::take(&mut *self.0.borrow_mut())
    }

    /// `lines` into `log` at `time`.
    pub fn write(lines: Vec<(Kind, Option<u64>, String)>, log: &mut NavLog, time: u32) {
        for (kind, key, text) in lines {
            match key {
                Some(key) => log.push_once(kind, key, time, text),
                None => log.push(kind, time, text),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jump_outcome_is_up_short_or_given_up() {
        assert_eq!(JumpOutcome::of(1.1, true, false, "went on"), JumpOutcome::Up);
        assert_eq!(JumpOutcome::of(1.1, true, false, "stuck: BackOff"), JumpOutcome::Up);
        assert_eq!(JumpOutcome::of(1.1, false, false, "went on"), JumpOutcome::Short);
        assert_eq!(JumpOutcome::of(1.1, false, false, "jumped again"), JumpOutcome::Short);
        assert_eq!(JumpOutcome::of(1.1, false, false, "stuck: Sidestep"), JumpOutcome::GaveUp);
        assert_eq!(JumpOutcome::of(1.1, false, false, "a new life"), JumpOutcome::GaveUp);
        // A level gap jump (within a step either way) is read as a jump up.
        assert_eq!(JumpOutcome::of(-0.3, true, false, "went on"), JumpOutcome::Up);
        let line = jump_line(JumpOutcome::Short, Vec3::new(57.1, 20.9, -3.8), Vec3::new(57.6, 22.0, -3.8), 2, 1500, "went on");
        assert_eq!(line, "nav: route jump short: from (57.1, 20.9, -3.8) to (57.6, 22.0, -3.8), rise 1.10 m, try 2, after 1500 ms (went on)");
    }

    #[test]
    fn a_jump_down_is_down_only_once_on_its_landing() {
        // A289: the body hopped at the edge and stayed on top; `is_up` called it up.
        let (take_off, landing) = (Vec3::new(20.0, 20.9, -24.5), Vec3::new(19.5, 16.7, -25.0));
        let top = Vec3::new(19.7, 20.95, -24.7);
        assert!(super::super::motor::is_up(top, take_off, landing), "is_up takes the top for across");
        assert!(!super::super::motor::on_landing(top, landing));
        assert!(super::super::motor::on_landing(Vec3::new(19.3, 16.72, -25.2), landing));
        let rise = landing.y - take_off.y;
        assert_eq!(JumpOutcome::of(rise, true, false, "went on"), JumpOutcome::NotDown);
        assert_eq!(JumpOutcome::of(rise, true, true, "went on"), JumpOutcome::Down);
        assert_eq!(JumpOutcome::of(rise, false, false, "stuck: BackOff"), JumpOutcome::GaveUp);
        assert_eq!(JumpOutcome::of(rise, false, true, "stuck: BackOff"), JumpOutcome::Down);
        let line = jump_line(JumpOutcome::NotDown, take_off, landing, 1, 3000, "went on");
        assert_eq!(line, "nav: route jump not down: from (20.0, 20.9, -24.5) to (19.5, 16.7, -25.0), rise -4.20 m, try 1, after 3000 ms (went on)");
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
