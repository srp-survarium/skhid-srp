//! Sprint on the calm stretches (N1, `pace.calm_sprint`).
//!
//! A play that may run hands [`Pace::Cruise`](super::plays::Pace) and this gate decides it just before the sprint key is
//! kept or stripped (`drive.rs`): the bot walks when a known danger is within the ring of its feet or of a point of the path it
//! is about to walk, and sprints otherwise. What it replaces is the plays' rule (`Ctx::pace_toward`: walk when a track
//! or a death spot is within 30 m of the bot **or of the point the play handed**), which walked 83 % of calm ticks against
//! the live players' 40 %: on a route leg the handed point is the next route point, 26 m away, so a danger 40 m off the
//! bot made it walk whatever lay between.
//!
//! - **The path, not the line.** The look-ahead is sampled along the route the bot walks ([`ahead_points`], every
//!   [`SAMPLE_M`] for `look_s` seconds of a sprint, the path's end included), where the straight line to a point can
//!   cross a wall or miss the corner the path turns into. Without a route (a straight step) it is the line.
//! - **A ring with two radii.** The walk is entered at `ring_m` and left at `leave_m`, and lasts `hold_ms` from the last
//!   sight of a danger in the ring: a track that moves a metre between two thinks does not flicker the sprint key.
//! - **What it reads** is what the plays' rule read: the bot's tracks of any source (the radar cheat's too with
//!   `ring_radar`) and the team's death spots of the last 20 s (with `ring_death_spots`). The bot's own 3-D position is not
//!   used: the distance is on the ground, as before (a track on another floor counts, the safe side).
//! - **Not here:** the bar reserve is the motor's (`Motor::sprint_by_the_bar`, `pace.reserve`); a play that runs on
//!   purpose (a chase, a retreat) hands `Run` and is not asked.

use mapdata::Vec3;

use super::know::CheatKind;
use super::know::Memory;
use super::know::Source;
use super::know::TeamKnowledge;
use super::path::Route;
use super::plays::Pace;
use super::tuning::PaceTuning;

/// The look-ahead is sampled this far apart along the path, metres.
pub const SAMPLE_M: f32 = 4.0;

/// The most points sampled ahead (the look-ahead is capped at `MAX_AHEAD * SAMPLE_M` = 20 m).
pub const MAX_AHEAD: usize = 5;

/// The team's death spots count for this long, ms (the plays' rule's).
const DANGER_MS: u32 = 20_000;

/// How old the track that walked the bot is, for the counts: under 5 s, 5-10 s, over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Age {
    Fresh,
    Mid,
    Old,
}

impl Age {
    fn of(ms: u32) -> Self {
        match ms {
            0..5_000 => Self::Fresh,
            5_000..10_000 => Self::Mid,
            _ => Self::Old,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// What made the bot walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Danger {
    /// A track of the bot's memory: how old, and whether the radar cheat gave it.
    Track { age: Age, radar: bool },
    /// A teammate's death spot.
    DeathSpot,
}

/// Where the danger was found: within the ring of the bot's feet, or of a point of the path ahead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    Feet,
    Path,
    /// The destination, within `goal_walk_m` of it.
    Goal,
}

/// Why the gate walks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walk {
    /// A danger in the ring now.
    Ring { at: Found, by: Danger },
    /// The ring was left, but not [`PaceTuning::hold_ms`] ago.
    Hold,
}

/// The gate's answer for a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gate {
    /// Not asked: the play did not hand `Cruise`, or the switch is off.
    #[default]
    Unasked,
    Run,
    Walk(Walk),
}

/// Whether the bot is walking from the ring, and since when: one state, not a flag and a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Ring {
    #[default]
    Clear,
    Walking {
        last_seen: u32,
    },
}

/// Why the sprint a bot was holding ended this tick, for the counts: the gate walked it, the bar's reserve, or the map (a
/// corner, a jump, an arrival, the facing: the motor's own rules).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cause {
    #[default]
    Map,
    Ring,
    Reserve,
}

/// What the gate looks at in a tick: the feet, the points ahead on the path, the destination if the play has one.
#[derive(Clone, Copy)]
struct Look<'a> {
    feet: Vec3,
    ahead: &'a [Vec3],
    goal: Option<Vec3>,
}

/// One bot's gate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaceState {
    ring: Ring,
    last: Gate,
    cause: Cause,
    /// The team's death spots of the last [`DANGER_MS`] that count for the ring (`ring_death_spots`), as of the last [`Self::learn_spots`].
    spots: Vec<Vec3>,
    pace: Option<Pace>,
}

impl PaceState {
    /// A new life: no walk in progress.
    pub fn new_life(&mut self) {
        self.ring = Ring::Clear;
        self.last = Gate::Unasked;
        self.cause = Cause::Map;
    }

    /// The team's death spots that walk the bot now (none when `ring_death_spots` is off): taken before the tick, so the gate reads
    /// the bot's own list.
    pub fn learn_spots(&mut self, knowledge: &TeamKnowledge, now: u32, ring_death_spots: bool) {
        self.spots.clear();
        if ring_death_spots {
            self.spots.extend(knowledge.danger_within(now, DANGER_MS).map(|d| d.at));
        }
    }

    /// The gate's answer in the tick that was decided last.
    pub fn last(&self) -> Gate {
        self.last
    }

    /// The pace the play handed in the tick that was decided last (for the counts: how much of the walking was ever asked of the gate).
    pub fn pace(&self) -> Option<Pace> {
        self.pace
    }

    /// What the play handed this tick.
    pub fn saw(&mut self, pace: Pace) {
        self.pace = Some(pace);
    }

    /// Why a sprint ended in the tick that was decided last (read with `last`).
    pub fn cause(&self) -> Cause {
        self.cause
    }

    /// Not asked this tick.
    pub fn unasked(&mut self) {
        self.last = Gate::Unasked;
        self.cause = Cause::Map;
    }

    /// The tick's cause once the keys are known: the gate walked a sprint the motor held, the bar's reserve let it go, or
    /// neither.
    pub fn caused(&mut self, stripped: bool, by_bar: bool) {
        self.cause = if stripped {
            Cause::Ring
        } else if by_bar {
            Cause::Reserve
        } else {
            Cause::Map
        };
    }

    /// Whether to run now: a known danger within the ring of the feet or of a point `ahead` on the path means a walk.
    pub fn decide(&mut self, tuning: &PaceTuning, now: u32, feet: Vec3, ahead: &[Vec3], goal: Option<Vec3>, memory: &Memory) -> Gate {
        let (enter, leave) = (tuning.ring_m.get(), tuning.leave_m.get().max(tuning.ring_m.get()));
        let radius = match self.ring {
            Ring::Clear => enter,
            Ring::Walking { .. } => leave,
        };
        let found = Self::find(tuning, now, radius, &Look { feet, ahead, goal }, memory, &self.spots);
        let gate = match (found, self.ring) {
            (Some((at, by)), _) => {
                self.ring = Ring::Walking { last_seen: now };
                Gate::Walk(Walk::Ring { at, by })
            }
            (None, Ring::Walking { last_seen }) if now.saturating_sub(last_seen) < tuning.hold_ms => Gate::Walk(Walk::Hold),
            (None, _) => {
                self.ring = Ring::Clear;
                Gate::Run
            }
        };
        self.last = gate;
        gate
    }

    /// The first danger within `radius` of the feet, else of a point ahead.
    fn find(tuning: &PaceTuning, now: u32, radius: f32, look: &Look, memory: &Memory, spots: &[Vec3]) -> Option<(Found, Danger)> {
        let Look { feet, ahead, goal } = *look;
        let dangers = || {
            let tracks = memory.tracks().iter().filter_map(move |t| {
                let radar = matches!(t.source, Source::Cheat(CheatKind::Radar));
                let age_ms = now.saturating_sub(t.time);
                ((tuning.ring_radar || !radar) && age_ms <= tuning.track_age_ms).then_some((t.at, Danger::Track { age: Age::of(age_ms), radar }))
            });
            tracks.chain(spots.iter().map(|&at| (at, Danger::DeathSpot)))
        };
        let near = |p: Vec3| dangers().find(|(at, _)| at.distance_xz(p) < radius).map(|(_, by)| by);
        if let Some(by) = near(feet) {
            return Some((Found::Feet, by));
        }
        // The destination, for the last stretch to it (the plays' own rule's goal clause).
        if tuning.goal_clause
            && let Some(goal) = goal
            && feet.distance_xz(goal) < tuning.goal_walk_m.get()
            && let Some(by) = near(goal)
        {
            return Some((Found::Goal, by));
        }
        // A cheap filter: no danger within the ring and the whole look-ahead of the feet, none is within the ring of a point
        // of it (the path is at least as long as the line).
        let reach = radius + ahead.len() as f32 * SAMPLE_M;
        if !dangers().any(|(at, _)| at.distance_xz(feet) < reach) {
            return None;
        }
        ahead.iter().find_map(|&p| near(p)).map(|by| (Found::Path, by))
    }
}

/// Up to [`MAX_AHEAD`] points `SAMPLE_M` apart on the route from where the body is, ending at the end of the route. The
/// body's progress is found on it (a short search of the next segments).
pub fn ahead_points(route: &mut Route, feet: Vec3, look_m: f32) -> ([Vec3; MAX_AHEAD], usize) {
    let (s, _) = route.progress(feet);
    let n = ((look_m / SAMPLE_M).ceil() as usize).clamp(1, MAX_AHEAD);
    let mut points = [feet; MAX_AHEAD];
    for (k, point) in points.iter_mut().enumerate().take(n) {
        *point = route.point_at(s + SAMPLE_M * (k + 1) as f32);
    }
    (points, n)
}

/// The same along the straight line from the feet to `to`, when there is no route.
pub fn line_points(feet: Vec3, to: Vec3, look_m: f32) -> ([Vec3; MAX_AHEAD], usize) {
    let n = ((look_m / SAMPLE_M).ceil() as usize).clamp(1, MAX_AHEAD);
    let (dx, dz) = (to.x - feet.x, to.z - feet.z);
    let length = dx.hypot(dz);
    let mut points = [feet; MAX_AHEAD];
    for (k, point) in points.iter_mut().enumerate().take(n) {
        let along = (SAMPLE_M * (k + 1) as f32).min(length);
        let t = if length < 1e-3 { 0.0 } else { along / length };
        *point = Vec3::new(feet.x + dx * t, feet.y + (to.y - feet.y) * t, feet.z + dz * t);
    }
    (points, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::know::Track;
    use crate::v026g::bots::path::Waypoint;

    fn tuning() -> PaceTuning {
        PaceTuning::default()
    }

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    fn memory_with(tracks: &[(Vec3, Source, u32)]) -> Memory {
        Memory::with_tracks(
            tracks
                .iter()
                .enumerate()
                .map(|(k, &(at, source, time))| Track {
                    who: k,
                    at,
                    error_m: 0.5,
                    time,
                    source,
                    sight_since: None,
                    notice_ms: 0,
                    own: None,
                    missed: None,
                    held_until: 0,
                })
                .collect(),
        )
    }

    fn waypoint(x: f32, z: f32) -> Waypoint {
        Waypoint { at: at(x, z), node: 0, jump: None, low: false }
    }

    fn straight_route(length: f32) -> Route {
        Route::new(vec![waypoint(0.0, 0.0), waypoint(length, 0.0)])
    }

    /// The look-ahead is the path's: a danger 14 m from a bot that sprints toward it walks it at once from 12 m back, a
    /// danger 40 m off the bot but 20 m from the end of a 26 m leg does not, as the plays' rule did.
    #[test]
    fn the_ring_is_looked_at_along_the_path_not_at_the_play_goal() {
        let mut state = PaceState::default();
        let mut route = straight_route(26.0);
        // A danger 40 m from the feet, 20 m from the end of the leg: the plays' rule (the goal within 30 m) walked.
        let memory = memory_with(&[(at(46.0, 0.0), Source::Seen, 0)]);
        let (points, n) = ahead_points(&mut route, at(0.0, 0.0), 12.0);
        assert_eq!(state.decide(&tuning(), 1_000, at(0.0, 0.0), &points[..n], None, &memory), Gate::Run);
        // Twelve metres on, the ring (30 m) reaches the end of the look-ahead: walk, found on the path.
        let (points, n) = ahead_points(&mut route, at(10.0, 0.0), 12.0);
        let gate = state.decide(&tuning(), 1_200, at(10.0, 0.0), &points[..n], None, &memory);
        assert!(matches!(gate, Gate::Walk(Walk::Ring { at: Found::Path, by: Danger::Track { radar: false, .. } })), "{gate:?}");
        // At 20 m the feet are within the ring.
        let (points, n) = ahead_points(&mut route, at(20.0, 0.0), 12.0);
        let gate = state.decide(&tuning(), 1_400, at(20.0, 0.0), &points[..n], None, &memory);
        assert!(matches!(gate, Gate::Walk(Walk::Ring { at: Found::Feet, .. })), "{gate:?}");
    }

    /// A path that turns away from a danger the straight line points at: the line crosses the ring, the path never enters it.
    #[test]
    fn a_danger_off_the_path_is_not_met_where_the_line_to_the_goal_crosses_its_ring() {
        let mut state = PaceState::default();
        // The path goes north, away from a danger 40 m east of the feet; a line to a goal in the east nears it.
        let mut route = Route::new(vec![waypoint(0.0, 0.0), waypoint(0.0, 60.0)]);
        let memory = memory_with(&[(at(40.0, 0.0), Source::Seen, 0)]);
        let (points, n) = ahead_points(&mut route, at(0.0, 0.0), 12.0);
        // The path's samples (4-12 m north) are 40 m or more from it.
        let gate = state.decide(&tuning(), 500, at(0.0, 0.0), &points[..n], None, &memory);
        assert_eq!(gate, Gate::Run, "{gate:?}");
        // The straight line from the feet to a goal past the danger does enter the ring.
        let (line, n) = line_points(at(0.0, 0.0), at(60.0, 0.0), 12.0);
        let mut other = PaceState::default();
        assert!(matches!(other.decide(&tuning(), 500, at(0.0, 0.0), &line[..n], None, &memory), Gate::Walk(_)));
    }

    /// The ring has two radii and a hold: entered at 30 m, a track at 32 m keeps the walk, one at 36 m does not (once the
    /// hold is over), so a track that moves a metre does not flicker the gate.
    #[test]
    fn the_walk_is_entered_at_the_ring_and_left_over_the_leave_radius_after_the_hold() {
        let mut state = PaceState::default();
        let feet = at(0.0, 0.0);
        let enter = memory_with(&[(at(29.0, 0.0), Source::Heard, 0)]);
        assert!(matches!(state.decide(&tuning(), 1_000, feet, &[], None, &enter), Gate::Walk(Walk::Ring { .. })));
        let edge = memory_with(&[(at(32.0, 0.0), Source::Heard, 0)]);
        // 32 m: outside the 30 m ring, inside the 34 m leave radius: still walking, and it is a ring walk (seen).
        assert!(matches!(state.decide(&tuning(), 1_200, feet, &[], None, &edge), Gate::Walk(Walk::Ring { .. })));
        let far = memory_with(&[(at(36.0, 0.0), Source::Heard, 0)]);
        // Over the leave radius but under the hold: walking.
        assert_eq!(state.decide(&tuning(), 1_500, feet, &[], None, &far), Gate::Walk(Walk::Hold));
        // Over it for the hold: running.
        assert_eq!(state.decide(&tuning(), 1_900, feet, &[], None, &far), Gate::Run);
        // And entering again takes the 30 m ring, not the leave radius.
        assert_eq!(state.decide(&tuning(), 2_000, feet, &[], None, &edge), Gate::Run);
    }

    /// `ring_radar` and `ring_death_spots` take the radar cheat's tracks and the death spots out of the ring.
    #[test]
    fn the_radar_and_the_death_spots_count_only_while_their_switches_are_on() {
        let feet = at(0.0, 0.0);
        let radar = memory_with(&[(at(10.0, 0.0), Source::Cheat(CheatKind::Radar), 0)]);
        let mut state = PaceState::default();
        assert!(matches!(
            state.decide(&tuning(), 100, feet, &[], None, &radar),
            Gate::Walk(Walk::Ring { by: Danger::Track { radar: true, .. }, .. })
        ));
        let mut strict = PaceState::default();
        let t = PaceTuning { ring_radar: false, ..tuning() };
        assert_eq!(strict.decide(&t, 100, feet, &[], None, &radar), Gate::Run);
    }

    /// A teammate's death spot of the last 20 s walks the bot while `ring_death_spots` is on, and is not even taken up with it off.
    #[test]
    fn a_death_spot_walks_the_bot_for_twenty_seconds_unless_its_switch_is_off() {
        use crate::v026g::bots::know::DangerSpot;
        let mut knowledge = TeamKnowledge::default();
        knowledge.danger.push(DangerSpot { at: at(10.0, 0.0), time: 1_000, victim: 3 });
        let memory = Memory::default();
        let feet = at(0.0, 0.0);
        let mut state = PaceState::default();
        state.learn_spots(&knowledge, 5_000, true);
        assert!(matches!(
            state.decide(&tuning(), 5_000, feet, &[], None, &memory),
            Gate::Walk(Walk::Ring { at: Found::Feet, by: Danger::DeathSpot })
        ));
        // Over 20 s old: gone.
        let mut late = PaceState::default();
        late.learn_spots(&knowledge, 21_500, true);
        assert_eq!(late.decide(&tuning(), 21_500, feet, &[], None, &memory), Gate::Run);
        let mut off = PaceState::default();
        off.learn_spots(&knowledge, 5_000, false);
        assert_eq!(off.decide(&tuning(), 5_000, feet, &[], None, &memory), Gate::Run);
    }

    /// The goal clause: with it, a bot within `goal_walk_m` of its destination walks when a known danger is within the ring of the destination, and not before; without it,
    /// or for a play with no destination (`goal` none), the destination is not looked at.
    #[test]
    fn the_goal_clause_walks_only_the_last_stretch_to_a_destination_near_a_known_danger() {
        let memory = memory_with(&[(at(100.0, 0.0), Source::Seen, 0)]);
        let goal = Some(at(90.0, 0.0));
        let with = PaceTuning { goal_clause: true, ..tuning() };
        // 70 m from the destination, 30 m from the danger: nothing in the ring of the feet (100 m off) or of the 12 m ahead.
        let mut state = PaceState::default();
        assert_eq!(state.decide(&with, 100, at(20.0, 0.0), &[], goal, &memory), Gate::Run);
        // 25 m from the destination (inside the last 30 m) and the destination is 10 m from the danger: walk, found at the goal.
        let mut near = PaceState::default();
        let gate = near.decide(&with, 200, at(65.0, 0.0), &[], goal, &memory);
        assert!(matches!(gate, Gate::Walk(Walk::Ring { at: Found::Goal, .. }) | Gate::Walk(Walk::Ring { at: Found::Feet, .. })), "{gate:?}");
        // Without the clause, or with no destination: the same bot runs while the feet and the path are clear.
        let mut off = PaceState::default();
        let clear = PaceTuning { ring_m: crate::v026g::bots::tuning::Metres::of(20.0), leave_m: crate::v026g::bots::tuning::Metres::of(22.0), ..tuning() };
        assert_eq!(off.decide(&PaceTuning { goal_clause: false, ..clear.clone() }, 300, at(65.0, 0.0), &[], goal, &memory), Gate::Run);
        let mut none = PaceState::default();
        assert_eq!(none.decide(&PaceTuning { goal_clause: true, ..clear.clone() }, 300, at(65.0, 0.0), &[], None, &memory), Gate::Run);
        let mut found = PaceState::default();
        let gate = found.decide(&PaceTuning { goal_clause: true, ..clear }, 300, at(65.0, 0.0), &[], goal, &memory);
        assert!(matches!(gate, Gate::Walk(Walk::Ring { at: Found::Goal, .. })), "{gate:?}");
    }

    /// The age of the track that walked the bot is binned for the counts.
    #[test]
    fn a_tracks_age_is_binned_under_five_ten_and_over() {
        assert_eq!([0, 4_999, 5_000, 9_999, 10_000, 20_000].map(Age::of), [Age::Fresh, Age::Fresh, Age::Mid, Age::Mid, Age::Old, Age::Old]);
    }
}
