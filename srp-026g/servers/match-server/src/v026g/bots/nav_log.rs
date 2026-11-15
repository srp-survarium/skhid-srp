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

/// [owner-5 R4] A drop's landing this near the body's, flat (m), is where the body came down (the falls study's test,
/// `owner-5/falls-rootcause.md`).
pub const LANDING_FLAT_M: f32 = 2.0;
/// [owner-5 R4] ... and within this of its height, m.
pub const LANDING_HEIGHT_M: f32 = 1.0;
/// [owner-5 R4] A grid drop link is at the fall's edge when its take-off floor is this near the line between the two worlds'
/// feet, flat, m (the study: the floors within 0.5 m of the split).
pub const EDGE_M: f32 = 0.5;
/// [owner-5 R4] The model is going down the same edge when its feet are this far or more under the top, m (the study's
/// "its height 0.5 m+ under the top at the note's world").
pub const MODEL_DOWN_M: f32 = 0.5;

/// A fall between two worlds of one life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fall {
    pub from: Vec3,
    pub to: Vec3,
    pub gap_ms: u32,
    /// The reckoning's feet at the later world's tick, if the trail had it.
    pub reckoned: Option<Vec3>,
    /// [owner-5 R4] Why a fall the model did not come down with is still the route's or the grid's ([`Self::of_route`]).
    pub by: Option<Planned>,
    /// [owner-5 R4] The route's next link where the motor last had the body ([`Self::of_route`]); none not asked.
    pub next: Option<NextLink>,
    /// [A388] The edge at the world before the fall ([`EdgeWord::at`]); none not asked.
    pub edge: Option<EdgeWord>,
}

/// [owner-5 R4] What makes a fall the model did not come down with a planned one: of 66 such notes in the owner's five
/// matches, a grid drop link at that edge landed where the body came down in 44, and the model went down the same edge
/// in 56 (`owner-5/falls-rootcause.md`): the model walks off 0.3 s after the body, which a 1 s world calls a fall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Planned {
    /// One of the route's own drops (a Drop link of its planner's path, or a gap jump down) lands where the body came down.
    RouteDrop(Vec3),
    /// A Drop or GapDrop link of the grid out of a floor at that edge lands where the body came down.
    DropLink(Vec3),
    /// The model's feet are [`MODEL_DOWN_M`] or more under the top: it is going down the same edge.
    ModelDown,
}

/// [owner-5 R4] The route's next link at a fall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NextLink {
    /// The bot had no route.
    NoRoute,
    /// The body was on the route's last leg's end.
    End,
    /// To the next waypoint `to` from the one before it `from`, by `how`.
    Link { from: Vec3, to: Vec3, how: LinkHow },
}

/// [owner-5 R4] How a route's link is made (the route's own word: its jump label, else its planner's drops).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkHow {
    Walk,
    Drop,
    Jump,
    SprintJump,
    Gap,
    SprintGap,
}

impl LinkHow {
    fn word(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Drop => "drop",
            Self::Jump => "jump",
            Self::SprintJump => "sprint jump",
            Self::Gap => "gap jump",
            Self::SprintGap => "sprint gap jump",
        }
    }
}

impl Fall {
    /// The fall from the world `before` (tick, feet) to the one `now`, if it is one.
    pub fn between(before: (u32, Vec3), now: (u32, Vec3), reckoned: Option<Vec3>) -> Option<Self> {
        let ((t0, from), (t1, to)) = (before, now);
        (t1 > t0 && t1 - t0 <= FALL_GAP_MS && from.y - to.y >= FALL_M).then(|| Self { from, to, gap_ms: t1 - t0, reckoned, by: None, next: None, edge: None })
    }

    /// Whether the model expected it: false when its feet stayed [`MODEL_M`] or more above the world's.
    pub fn planned(&self) -> Option<bool> {
        self.reckoned.map(|r| r.y - self.to.y < MODEL_M)
    }

    /// [owner-5 R4] The fall with the route's word on it (`route`, the bot's; `grid`, the map's): what makes it planned
    /// ([`Planned`], the first that holds, in its order) and the route's next link. Reads the route's waypoints and the
    /// floors under the line between the two worlds and their links: no search, no ray.
    pub fn of_route(self, grid: &mapdata::WalkGrid, route: Option<&super::path::Route>) -> Self {
        let lands = |landing: Vec3| landing.distance_xz(self.to) <= LANDING_FLAT_M && (landing.y - self.to.y).abs() <= LANDING_HEIGHT_M;
        let route_drop = route.and_then(|route| route_drops(route).into_iter().find(|&landing| lands(landing)));
        let link_drop = || {
            let (a, b) = (self.from, self.to);
            let area = [a.x.min(b.x) - EDGE_M, a.z.min(b.z) - EDGE_M, a.x.max(b.x) + EDGE_M, a.z.max(b.z) + EDGE_M];
            grid.floors_in(area)
                .filter(|&node| {
                    let p = grid.position(node);
                    (p.y - a.y).abs() <= LANDING_HEIGHT_M && flat_to_segment(p, a, b) <= EDGE_M
                })
                .flat_map(|node| grid.links(node))
                .filter(|&(_, kind)| matches!(kind, mapdata::grid::LinkKind::Drop | mapdata::grid::LinkKind::GapDrop))
                .map(|(to, _)| grid.position(to))
                .find(|&landing| lands(landing))
        };
        let model_down = || self.reckoned.is_some_and(|r| self.from.y - r.y >= MODEL_DOWN_M);
        let by = route_drop
            .map(Planned::RouteDrop)
            .or_else(|| link_drop().map(Planned::DropLink))
            .or_else(|| model_down().then_some(Planned::ModelDown));
        Self { by, next: Some(route.map_or(NextLink::NoRoute, next_link)), ..self }
    }

    pub fn line(&self) -> String {
        let next = match self.next {
            None => String::new(),
            Some(NextLink::NoRoute) => ", no route".to_string(),
            Some(NextLink::End) => ", the route at its end".to_string(),
            Some(NextLink::Link { from, to, how }) => format!(", the route's next link a {} from {} to {}", how.word(), at(from), at(to)),
        };
        let model = match (self.reckoned, self.planned()) {
            (Some(r), Some(planned)) => {
                let word = match (planned, self.by) {
                    (true, _) => "planned".to_string(),
                    (false, Some(Planned::RouteDrop(landing))) => format!("planned: the route's drop to {}", at(landing)),
                    (false, Some(Planned::DropLink(landing))) => format!("planned: a drop link at that edge to {}", at(landing)),
                    (false, Some(Planned::ModelDown)) => "planned: the model going down the same edge".to_string(),
                    (false, None) => "unplanned".to_string(),
                };
                format!(", the model had it at {:.1} ({word})", r.y)
            }
            _ => String::new(),
        };
        let edge = self.edge.map_or(String::new(), |edge| format!(", {edge}"));
        format!("{PREFIX}fell {:.1} m from {} to {} in {} ms{next}{model}{edge}", self.from.y - self.to.y, at(self.from), at(self.to), self.gap_ms)
    }
}

/// [A388] Both bodies' word on an edge at some feet, for the log (decision-neutral): the grid's (the floor that holds the feet and
/// how far its centre is, against the hold to `body::SUPPORT_M`), the client walker's (ground under the feet within a step), and the
/// reckoning's kept controller's (its last step on the ground or off it; none where none is kept).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeWord {
    /// The flat distance from the feet to the centre of the floor the grid has them on, m; none: no floor within reach.
    pub floor_m: Option<f32>,
    /// The client's walker has ground within a step under the feet; none: no walker.
    pub walker_ground: Option<bool>,
    /// The kept controller's last step left the ground; none: no controller kept.
    pub controller_off: Option<bool>,
}

impl EdgeWord {
    /// The word at `feet` on `grid`, with the kept controller's `controller_off`.
    pub fn at(grid: &mapdata::WalkGrid, feet: Vec3, controller_off: Option<bool>) -> Self {
        let floor_m = grid.locate(feet, 1.0).map(|node| grid.position(node).distance_xz(feet));
        let step = grid.body().step;
        let walker_ground = grid.has_client_walker().then(|| grid.walker_ground_below(feet, feet.y + 0.1, 0.1 + step).is_some());
        Self { floor_m, walker_ground, controller_off }
    }
}

impl std::fmt::Display for EdgeWord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let support = super::body::SUPPORT_M;
        match self.floor_m {
            Some(m) => write!(f, "edge: the grid's floor centre {m:.2} m away ({} its hold of {support})", if m <= support { "within" } else { "past" })?,
            None => write!(f, "edge: no grid floor")?,
        }
        match self.walker_ground {
            Some(ground) => write!(f, ", the walker's ground under the feet {}", if ground { "yes" } else { "no" })?,
            None => write!(f, ", no walker")?,
        }
        match self.controller_off {
            Some(off) => write!(f, ", the kept controller {}", if off { "off the ground" } else { "on the ground" }),
            None => write!(f, ", no kept controller"),
        }
    }
}

/// [A388] A lip guard's change of a tick's keys: `nav: lip guard <swapped|braked|doomed> at (..), keys K to K2, speed S m/s, <edge>`.
pub fn guard_line(verdict: &str, feet: Vec3, keys: u32, to: Option<u32>, speed: f32, edge: EdgeWord) -> String {
    let to = to.map_or("none".to_string(), |k| format!("{k:#x}"));
    format!("{PREFIX}lip guard {verdict} at {}, keys {keys:#x} to {to}, speed {speed:.2} m/s, {edge}", at(feet))
}

/// [owner-5 R4] The landings of `route`'s drops: its planner's Drop links ([`super::path::DropTruth::Links`]) and its gap
/// jumps down (a landing a lip's height or more under the take-off).
fn route_drops(route: &super::path::Route) -> Vec<Vec3> {
    let points = route.points();
    let mut drops = route.drop_landings(0.0, route.length(), super::path::DropTruth::Links);
    drops.extend(points.windows(2).filter_map(|w| {
        let gap = matches!(w[1].jump, Some(super::path::Leap::Gap(_) | super::path::Leap::SprintGap(_)));
        (gap && w[0].at.y - w[1].at.y >= mapdata::grid::LIP_DROP_M).then_some(w[1].at)
    }));
    drops
}

/// [owner-5 R4] The route's next link from where the motor last had the body (its segment).
fn next_link(route: &super::path::Route) -> NextLink {
    let points = route.points();
    let k = route.segment() + 1;
    let (Some(from), Some(to)) = (points.get(k - 1), points.get(k)) else {
        return NextLink::End;
    };
    use super::path::Leap;
    let how = match to.jump {
        Some(Leap::Ledge) => LinkHow::Jump,
        Some(Leap::SprintLedge) => LinkHow::SprintJump,
        Some(Leap::Gap(_)) => LinkHow::Gap,
        Some(Leap::SprintGap(_)) => LinkHow::SprintGap,
        None if route.drop_landings(0.0, route.length(), super::path::DropTruth::Links).contains(&to.at) => LinkHow::Drop,
        None => LinkHow::Walk,
    };
    NextLink::Link { from: from.at, to: to.at, how }
}

/// How far `p` is from the segment `a`-`b`, flat, m.
fn flat_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let length2 = dx * dx + dz * dz;
    let t = if length2 < 1e-6 { 0.0 } else { (((p.x - a.x) * dx + (p.z - a.z) * dz) / length2).clamp(0.0, 1.0) };
    (p.x - (a.x + t * dx)).hypot(p.z - (a.z + t * dz))
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
    /// [owner-5 A395a] A walk out over a zone's floors begun: from where, the floor dry or being hit, to where (`drive.rs`).
    WalkOut,
    /// [A388] The lip guard changed a tick's keys at an edge, with both bodies' word on it (`drive.rs` `guard_keys`).
    Guard,
    /// [S7] A crouched climb of the route (a Walk link only a crouching body climbs): crouched at its foot, and whether it got
    /// up (`drive.rs` `crouch_climb_watch`).
    CrouchClimb,
    /// [Gap 2] An enemy's carry the world names and the bot's team does not know of, kept from the intercept
    /// (`plays/intercept.rs`).
    Carry,
    /// [F42] A bot with both legs broken creeping (the crouch key with its move keys): begun, and ended with how far it went
    /// (`drive.rs` `creep_watch`).
    Creep,
}

/// How many kinds of line there are ([`Kind`]).
const KINDS: usize = 13;

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

/// [S7] A crouched climb: `nav: crouch climb up|not up: from (..) to (..), rise R m, crouched at its foot T ms before, feet (..)`:
/// `up` once a world showed the body up it (`motor::is_up`), `not up` when none did within the watch.
pub fn crouch_climb_line(up: bool, take_off: Vec3, top: Vec3, after_ms: u32, feet: Vec3) -> String {
    let word = if up { "up" } else { "not up" };
    format!(
        "{PREFIX}crouch climb {word}: from {} to {}, rise {:.2} m, crouched at its foot {after_ms} ms before, feet {}",
        at(take_off),
        at(top),
        top.y - take_off.y,
        at(feet)
    )
}

/// [F42] A creep begun: `nav: creep begun (both legs broken): at (..), the crouch key with move keys; speed in the reckoning
/// pending (the selector's run / on-site alternation)`.
pub fn creep_begun_line(feet: Vec3) -> String {
    format!(
        "{PREFIX}creep begun (both legs broken): at {}, the crouch key with move keys; speed in the reckoning pending (the selector's run / on-site alternation)",
        at(feet)
    )
}

/// [F42] A creep ended: `nav: creep ended: from (..) to (..), D m flat in T ms (S m/s)`.
pub fn creep_ended_line(from: Vec3, feet: Vec3, ms: u32) -> String {
    let d = from.distance_xz(feet);
    format!(
        "{PREFIX}creep ended: from {} to {}, {d:.2} m flat in {ms} ms ({:.2} m/s)",
        at(from),
        at(feet),
        if ms > 0 { d * 1_000.0 / ms as f32 } else { 0.0 }
    )
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

/// [C6] A zone spawner searched from a look spot on the margin's band (a floor no zone lists a standing body on, that the
/// margin alone marks): `nav: zone spawner <i> at (..) looked at from the band at (..)`.
pub fn spawner_band_line(index: usize, item: Vec3, spot: Vec3) -> String {
    format!("{PREFIX}zone spawner {index} at {} looked at from the band at {}", at(item), at(spot))
}

/// [owner-5 A395a] A walk out over a zone's floors begun: `nav: zone walk-out from (..) on a <dry|hit> floor to (..), next
/// (..), lines tested over the zone's floors up to R m`. `hit`: a zone on now overlaps the standing body's capsule there
/// (`Hazards::hits_at`, the client's contact test); `dry`: none does (the floor is only in a zone's margin, or its zone is
/// off). `to` is the route's goal, `next` its next waypoint.
pub fn walkout_line(from: Vec3, hit: Option<bool>, goal: Vec3, next: Option<Vec3>, reach: f32) -> String {
    let floor = match hit {
        Some(true) => "on a hit floor",
        Some(false) => "on a dry floor",
        None => "on no floor",
    };
    let next = next.map_or(String::new(), |p| format!(", next {}", at(p)));
    format!("{PREFIX}zone walk-out from {} {floor} to {}{next}, lines tested over the zone's floors up to {reach:.1} m", at(from), at(goal))
}

/// [A395a] A walk off the margin's band begun (`Marking::Capsule`, where the client hurts no body): `nav: zone band exit
/// from (..) to (..), next (..), lines tested over the band's floors up to R m`. Its lines cross no floor a zone hurts.
/// [A395a] The line for a walk out whose held route goes on across the zone past the way out's reach
/// (`Tactics::route_shut_ahead`): the route is planned again from where the body stands.
/// [S6] The line for a pocket escape begun by a body with no sprint (a carrier): the exits whose take-offs it walks to by
/// its own reach (no sprint link without its walking press).
pub fn carrier_escape_line(feet: Vec3, exits: usize) -> String {
    format!("{PREFIX}pocket escape with no sprint (a carrier) from {}: {exits} exits whose take-offs it walks to", at(feet))
}

pub fn walk_route_shut_line(from: Vec3, reach: f32) -> String {
    format!("{PREFIX}zone walk's held route goes on across the zone from {}, past the way out's {reach:.1} m: planned again", at(from))
}

/// [A395a] The line for a walk out that the client's capsule at the body's own feet starts, on a floor the band's marks
/// (tested at the floor's centre) leave clear.
pub fn feet_touch_line(feet: Vec3) -> String {
    format!("{PREFIX}zone walk-out from {}: the capsule at the feet touches a zone, the floor's centre does not", at(feet))
}

/// [A395a] The line for a walk out's first tick on a floor a zone on hurts (`Hazards::hits_at`), after `ms` of the walk, on
/// the route it held or one planned since.
pub fn walk_hurt_line(at_feet: Vec3, ms: u32, route: super::tactics::WalkRoute) -> String {
    let route = match route {
        super::tactics::WalkRoute::Held => "its held route",
        super::tactics::WalkRoute::Planned => "a route planned since",
    };
    format!("{PREFIX}zone walk on a hurt floor at {}, {ms} ms into it, on {route}", at(at_feet))
}

pub fn band_exit_line(from: Vec3, goal: Vec3, next: Option<Vec3>, reach: f32) -> String {
    let next = next.map_or(String::new(), |p| format!(", next {}", at(p)));
    format!("{PREFIX}zone band exit from {} to {}{next}, lines tested over the band's floors up to {reach:.1} m", at(from), at(goal))
}

/// [owner-5] A key for [`NavLog::push_once`] from the line's parts.
pub fn key(parts: impl std::hash::Hash) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault};
    BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(parts)
}

/// [owner-5] Lines a play writes while it is asked (`Ctx::lines`, the candidates or a step), for its bot's [`NavLog`] once
/// the poll lets go of the bot: `(kind, key, text)`; a key of `None` is a plain [`NavLog::push`].
#[derive(Debug, Default)]
pub struct Lines(std::cell::RefCell<Vec<Line>>);

/// [owner-5] One of [`Lines`]: its kind, its key for [`NavLog::push_once`] (none: a plain push) and its text.
pub type Line = (Kind, Option<u64>, String);

impl Lines {
    pub fn push(&self, kind: Kind, key: Option<u64>, text: String) {
        self.0.borrow_mut().push((kind, key, text));
    }

    /// The lines written, for [`Self::write`].
    pub fn take(&self) -> Vec<Line> {
        std::mem::take(&mut *self.0.borrow_mut())
    }

    /// `lines` into `log` at `time`.
    pub fn write(lines: Vec<Line>, log: &mut NavLog, time: u32) {
        for (kind, key, text) in lines {
            match key {
                Some(key) => log.push_once(kind, key, time, text),
                None => log.push(kind, time, text),
            }
        }
    }
}

/// [A395a] The line for a battery's stand on the margin's band, which the margin alone refuses: whether the client's
/// capsule rule (`Hazards::stand_allowed` with `hazard.capsule_goals`) lets a body stand there now.
pub fn band_stand_line(feet: Vec3, allowed: bool) -> String {
    let verdict = if allowed { "allowed: the capsule touches no zone on there" } else { "refused" };
    format!("{PREFIX}stand on the margin's band at {}: {verdict}", at(feet))
}

/// [Gap 2] The line for an enemy's carry kept from the intercept: the world names `carrier` holding `battery`, and the
/// team neither saw him carrying it nor saw him take it (`TeamKnowledge::knew_carrying`).
pub fn hidden_carry_line(battery: usize, carrier: usize) -> String {
    format!("{PREFIX}carry of battery {battery} by player {carrier} not known to the team: chased at the theft's spot, if any")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [owner-5] The owner-5 kinds stop at [`MAX_PER_KIND`] lines a match; a key writes once; the old kinds are not capped.
    #[test]
    fn the_owner5_lines_are_capped_and_keyed() {
        let mut log = NavLog::default();
        for i in 0..(MAX_PER_KIND + 20) {
            log.push(Kind::Zone, i * EVERY_MS, format!("{PREFIX}z {i}"));
        }
        assert_eq!(log.take().len(), MAX_PER_KIND as usize);
        for i in 0..(MAX_PER_KIND + 20) {
            log.push(Kind::Jump, i * EVERY_MS, format!("{PREFIX}j {i}"));
        }
        assert_eq!(log.take().len(), (MAX_PER_KIND + 20) as usize, "a route jump's line is not capped");
        let k = key(("route", 1, 2));
        log.push_once(Kind::Route, k, 0, format!("{PREFIX}a"));
        log.push_once(Kind::Route, k, 10 * EVERY_MS, format!("{PREFIX}a again"));
        log.push_once(Kind::Route, key(("route", 1, 3)), 20 * EVERY_MS, format!("{PREFIX}b"));
        let lines: Vec<String> = log.take().into_iter().map(|(_, t)| t).collect();
        assert_eq!(lines, vec![format!("{PREFIX}a"), format!("{PREFIX}b")]);
    }

    /// [owner-5] Every owner-5 line begins with [`PREFIX`], so the bench's decision digest leaves it out
    /// (`tests/tallies/decisions.rs`).
    #[test]
    fn the_owner5_lines_are_the_logs() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        for line in [
            role_line("flanker", "ThreatSpot(7)", "Flank", "no_spot", None),
            sprint_line(p, p, 0.4, "sprint not on"),
            zone_line(p, "infection pool: cost 0.300 over 0.50 of 0.400"),
            spawner_line(3, p, "take.zone_look: no dry look spot in sight of it"),
            route_line(None, p, "its start on no floor"),
        ] {
            assert!(line.starts_with(PREFIX), "{line}");
        }
        assert_eq!(route_line(Some(p), p, "x"), "nav: no way from (1.0, 2.0, 3.0) to (1.0, 2.0, 3.0): x");
    }

    /// [owner-5 R4] A fall off a 3 m deck's edge: planned by the route's own drop when the route makes it, by the grid's
    /// drop link at that edge with no route, by the model going down when no drop lands there; unplanned otherwise. The
    /// route's next link is written.
    #[test]
    fn a_fall_at_a_drop_is_planned_and_names_the_routes_next_link() {
        use crate::v026g::bots::body::tests::floor;
        let mut tris = floor(0.0, 0.0, 10.0, 10.0, 3.0).to_vec();
        tris.extend(floor(10.0, 0.0, 24.0, 10.0, 0.0));
        let grid = mapdata::WalkGrid::build(&tris, [0.0, 0.0, 24.0, 10.0], 0.25, mapdata::Body::default());
        let (a, b) = (grid.locate(Vec3::new(5.0, 3.0, 5.0), 0.3).unwrap(), grid.locate(Vec3::new(11.5, 0.0, 5.0), 0.3).unwrap());
        let route = super::super::path::plan(&grid, a, b).expect("a way down");
        let (top, under) = (Vec3::new(9.8, 3.0, 5.0), Vec3::new(10.6, 0.0, 5.0));
        let fall = Fall::between((10_000, top), (11_000, under), Some(Vec3::new(9.9, 3.0, 5.0))).unwrap();
        assert_eq!(fall.planned(), Some(false), "the model stayed on the deck");
        let by_route = fall.of_route(&grid, Some(&route));
        assert!(matches!(by_route.by, Some(Planned::RouteDrop(_))), "{by_route:?}");
        let line = by_route.line();
        assert!(line.contains("the route's next link a ") && line.contains("(planned: the route's drop to "), "{line}");
        let by_link = fall.of_route(&grid, None);
        assert!(matches!(by_link.by, Some(Planned::DropLink(_))), "{by_link:?}");
        assert!(by_link.line().contains(", no route,") && by_link.line().contains("(planned: a drop link at that edge to "), "{}", by_link.line());
        // Far past any drop's landing: the model going down too, else unplanned.
        let far = Vec3::new(17.0, 0.0, 5.0);
        let going = Fall::between((10_000, top), (11_000, far), Some(Vec3::new(10.2, 2.0, 5.0))).unwrap().of_route(&grid, None);
        assert_eq!(going.by, Some(Planned::ModelDown));
        let stayed = Fall::between((10_000, top), (11_000, far), Some(Vec3::new(9.9, 3.0, 5.0))).unwrap().of_route(&grid, None);
        assert_eq!(stayed.by, None);
        assert!(stayed.line().ends_with("(unplanned)"), "{}", stayed.line());
        // Not asked: the line as before.
        assert_eq!(fall.line(), "nav: fell 3.0 m from (9.8, 3.0, 5.0) to (10.6, 0.0, 5.0) in 1000 ms, the model had it at 3.0 (unplanned)");
    }

    /// [owner-5 A395a] The walk-out line names its start floor's state and its goal.
    #[test]
    fn an_edge_word_says_each_bodys_verdict() {
        let edge = EdgeWord { floor_m: Some(0.31), walker_ground: Some(false), controller_off: Some(true) };
        assert_eq!(
            guard_line("braked", Vec3::new(1.0, 2.0, 3.0), 0x1, Some(0x2), 1.5, edge),
            "nav: lip guard braked at (1.0, 2.0, 3.0), keys 0x1 to 0x2, speed 1.50 m/s, edge: the grid's floor centre 0.31 m away (within its hold of 0.435), the walker's ground under the feet no, the kept controller off the ground"
        );
        let none = EdgeWord { floor_m: None, walker_ground: None, controller_off: None };
        assert_eq!(none.to_string(), "edge: no grid floor, no walker, no kept controller");
    }

    #[test]
    fn a_walkout_line_says_dry_or_hit() {
        let (p, q) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 2.0, 3.0));
        assert_eq!(
            walkout_line(p, Some(false), q, Some(q), 3.0),
            "nav: zone walk-out from (1.0, 2.0, 3.0) on a dry floor to (4.0, 2.0, 3.0), next (4.0, 2.0, 3.0), lines tested over the zone's floors up to 3.0 m"
        );
        assert!(walkout_line(p, Some(true), q, None, 3.0).contains("on a hit floor to (4.0, 2.0, 3.0), lines"));
    }

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
