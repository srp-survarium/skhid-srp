//! What a jump link costs in time, against cruising (A55, the owner's question: "how fast bots take jump links").
//!
//! Read-only: nothing is decided from it. A link is *watched* from the tick the motor first lines the body up for it (the
//! sprint is let go of, the body slows; [`Watch::begin`]) to the tick the body is back at cruise speed past the landing
//! ([`CRUISE_SHARE`] of its speed at the start, landing brake included), or [`AFTER_WINDOW_M`] past the landing. Counted by kind of link and by band of rise
//! ([`counts`]): the time, the time over what the same path would take at the starting speed (the *excess*), the metres
//! walked against the straight line, the back-offs for another run-up with their seconds, the presses and the links got
//! up by the first press (or, for a lift, with none).
//!
//! The watch is a state, not flags: [`Phase::Before`] the body is not up the link yet, [`Phase::After`] it is and has not
//! regained its speed.

use std::sync::atomic::{AtomicU64, Ordering};

use mapdata::Vec3;

use super::kin::TICK_S;

/// The kinds of link a route has the motor slow down for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A walking jump up a ledge (a rise the walking jump makes).
    Walking,
    /// A sprint-band ledge jump (a rise over the walking jump's).
    SprintBand,
    /// A sprint gap link.
    SprintGap,
    /// A gap link jumped at a walk.
    Gap,
    /// A planned drop, walked or sprinted off.
    Drop,
    /// The sprint lift up a low top ([`super::motor::Feel::fight_lift`]).
    Lift,
    /// A walking jump kept at a sprint ([`super::motor::Feel::sprint_through`]).
    Through,
}

impl Kind {
    pub const ALL: [Kind; 7] =
        [Kind::Walking, Kind::SprintBand, Kind::SprintGap, Kind::Gap, Kind::Drop, Kind::Lift, Kind::Through];
    pub const NAMES: [&'static str; 7] = ["walking", "sprint_band", "sprint_gap", "gap", "drop", "lift", "through"];

    /// The band of the link's `rise` (negative for a drop): 0.1 m from 0.5 m up for a climb, 1 m of height for a drop;
    /// [`BANDS`] in all.
    fn band(self, rise: f32) -> usize {
        let b = if self == Kind::Drop { (-rise).max(0.0) } else { (rise - 0.5).max(0.0) * 10.0 };
        (b as usize).min(BANDS - 1)
    }
}

pub const BANDS: usize = 11;

/// The body has regained its cruise when its speed is this share of the starting speed.
const CRUISE_SHARE: f32 = 0.9;
/// A start slower than this, m/s, is taken as a walk (a body that begins from a stand, a turn).
const MIN_CRUISE_MS: f32 = 3.0;
/// A link not touched by the motor for this long, ticks, and the body not up it, is given up on (the route changed).
const ABANDON_TICKS: u32 = 40;
/// A body up the link that has not regained its speed by this many ticks is counted slow, not timed.
const SLOW_TICKS: u32 = 120;
/// A body slower than this, m/s, is stopped.
const STOPPED_MS: f32 = 1.0;
/// A link begun at this speed or more, m/s, began at a sprint.
const SPRINT_START_MS: f32 = 4.2;
/// The link is timed to this far past its landing at most, m.
const AFTER_WINDOW_M: f32 = 4.0;
/// A drop is landed when the feet are this near its landing across, m, and this near in height.
const DROP_NEAR_M: f32 = 1.5;
const DROP_HEIGHT_M: f32 = 0.5;

/// What the motor was doing with the body in a tick of a link's approach (the cause of a slow or stopped tick).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Why {
    /// Nothing known (the first tick).
    #[default]
    Unknown,
    /// The client takes no sprint now for a fight's reasons: aiming, firing, a reload, a hit.
    Fight,
    /// Not on the jump's line yet: walking to the line-up point, or turning on the route's corner.
    Approach,
    /// On the line and standing to turn onto the jump's way ([`super::motor::LINE_UP_GO`]).
    Turning,
    /// On the line, walking and facing it.
    Lined,
}

impl Why {
    pub const ALL: [Why; 5] = [Why::Unknown, Why::Fight, Why::Approach, Why::Turning, Why::Lined];
    pub const NAMES: [&'static str; 5] = ["unknown", "fight", "approach", "turning", "lined"];
}

/// Why the motor stopped asking for the link watched, as far as it knows (the last tick it was not asked for).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lost {
    /// The motor did not walk the route at all (the body died, stood, aimed still, another override).
    #[default]
    Silent,
    /// The route has no jump ahead of the progress any more (a new route, or the body is past it).
    NoJump,
    /// The route's next jump is this link but the progress is further from the take-off than the line-up distance.
    Far,
}

/// The link being watched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Link {
    pub kind: Kind,
    pub take_off: Vec3,
    pub landing: Vec3,
    /// How deep the top is past the face, m, where the planner measured it ([`super::path::Route::ledge_top`]).
    pub top: Option<f32>,
}

/// Where and how fast the body was at the first press of a link: metres before the take-off along its way (negative past
/// it), metres to the side of its line, and the ground speed.
#[derive(Clone, Copy, Debug)]
struct Press {
    before: f32,
    side: f32,
    speed: f32,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    /// Not up the link; `touched`: the motor's tick it was last asked for.
    Before { touched: u32 },
    /// Up it since `at`.
    After { at: u32 },
}

#[derive(Clone, Copy, Debug)]
struct Run {
    link: Link,
    phase: Phase,
    start: u32,
    from: Vec3,
    last: Vec3,
    v0: f32,
    path_m: f32,
    presses: u32,
    backoffs: u32,
    backoff_ticks: u32,
    lifted: bool,
    /// Why the link was last not asked for since it was (see [`Lost`]).
    lost: Lost,
    /// The ending ([`super::route_end::RouteEnd`] as an index) of the route walked, if it ended while the link was watched.
    route_ended_by: Option<usize>,
    /// The tick of the first press, and of getting up (the phases of the time: before the press, in the air, after).
    pressed_at: Option<u32>,
    press: Option<Press>,
    up_at: Option<u32>,
    /// Ticks before the first press (or getting up) spent stopped (under [`STOPPED_MS`]) and slowed (under the cruise).
    stopped_ticks: u32,
    slowed_ticks: u32,
    /// The same ticks by [`Why`]: stopped, slowed.
    why_ticks: [[u32; 2]; 5],
}

#[derive(Clone, Debug, Default)]
pub struct Watch {
    run: Option<Run>,
    /// The link last given up on, followed for [`GHOST_TICKS`] to see where the body went (a face of 0.8-1.2 m only).
    ghost: Option<Ghost>,
}

/// A link given up on, followed on: does the body climb it anyway, or stay near its foot?
#[derive(Clone, Copy, Debug)]
struct Ghost {
    link: Link,
    kind: usize,
    ended: usize,
    since: u32,
    near_ticks: u32,
    climbed: bool,
}

/// How [`Watch::tick`] says the body is up the link: the feet, the link's kind.
fn is_up(kind: Kind, feet: Vec3, link: &Link) -> bool {
    match kind {
        Kind::Drop => feet.distance_xz(link.landing) <= DROP_NEAR_M && (feet.y - link.landing.y).abs() <= DROP_HEIGHT_M,
        _ => super::motor::is_up(feet, link.take_off, link.landing),
    }
}

impl Watch {
    /// Whether no link is watched.
    pub fn idle(&self) -> bool {
        self.run.is_none()
    }

    /// The motor lines the body up for `link` at tick `clock`: the first call starts the watch (the speed now is the cruise
    /// it is measured against); a call for the link watched keeps it; another link ends the watch before it, counted as
    /// cut.
    pub fn begin(&mut self, clock: u32, link: Link, feet: Vec3, speed: f32) {
        if let Some(run) = self.run.as_mut() {
            if same(&run.link, &link) {
                if let Phase::Before { touched } = &mut run.phase {
                    *touched = clock;
                }
                run.lost = Lost::Silent;
                // A lift given up for the jump is the jump's kind from then on; it was a lift for a while.
                run.link.kind = link.kind;
                run.lifted |= link.kind == Kind::Lift;
                return;
            }
            self.finish(clock, feet, speed, Ending::Cut);
        }
        self.run = Some(Run {
            link,
            phase: Phase::Before { touched: clock },
            start: clock,
            from: feet,
            last: feet,
            v0: speed.max(MIN_CRUISE_MS),
            path_m: 0.0,
            presses: 0,
            backoffs: 0,
            backoff_ticks: 0,
            lifted: link.kind == Kind::Lift,
            lost: Lost::Silent,
            route_ended_by: None,
            pressed_at: None,
            press: None,
            up_at: None,
            stopped_ticks: 0,
            slowed_ticks: 0,
            why_ticks: [[0; 2]; 5],
        });
    }

    /// The motor walked the route this tick and did not ask for the link watched, for this reason.
    pub fn lost(&mut self, why: Lost) {
        if let Some(run) = self.run.as_mut()
            && matches!(run.phase, Phase::Before { .. })
        {
            run.lost = why;
        }
    }

    /// The route walked ended (or was put in place of) for this reason, by its index in [`super::route_end::RouteEnd::ALL`].
    pub fn route_ended(&mut self, why: usize) {
        if let Some(run) = self.run.as_mut()
            && matches!(run.phase, Phase::Before { .. })
        {
            run.route_ended_by.get_or_insert(why);
        }
    }

    /// A jump key pressed for the link, with the body's feet and ground speed.
    pub fn pressed(&mut self, clock: u32, feet: Vec3, speed: f32) {
        if let Some(run) = self.run.as_mut() {
            run.presses += 1;
            if run.pressed_at.is_none() {
                run.pressed_at = Some(clock);
                let (dx, dz) = (run.link.landing.x - run.link.take_off.x, run.link.landing.z - run.link.take_off.z);
                let len = (dx * dx + dz * dz).sqrt().max(1e-3);
                let (wx, wz) = (dx / len, dz / len);
                let (fx, fz) = (feet.x - run.link.take_off.x, feet.z - run.link.take_off.z);
                run.press = Some(Press { before: -(fx * wx + fz * wz), side: (fx * wz - fz * wx).abs(), speed });
            }
        }
    }

    /// An arrival backed off from for another run-up.
    pub fn backed_off(&mut self) {
        if let Some(run) = self.run.as_mut() {
            run.backoffs += 1;
        }
    }

    /// A tick spent walking back to the run-up.
    pub fn backing(&mut self) {
        if let Some(run) = self.run.as_mut() {
            run.backoff_ticks += 1;
        }
    }

    /// One tick of the walk: the feet and ground speed now.
    pub fn tick(&mut self, clock: u32, feet: Vec3, speed: f32, why: Why) {
        self.follow_ghost(clock, feet);
        let Some(run) = self.run.as_mut() else {
            return;
        };
        run.path_m += feet.distance_xz(run.last);
        run.last = feet;
        if run.pressed_at.is_none() && matches!(run.phase, Phase::Before { .. }) {
            if speed < STOPPED_MS {
                run.stopped_ticks += 1;
                run.why_ticks[why as usize][0] += 1;
            } else if speed < CRUISE_SHARE * run.v0 {
                run.slowed_ticks += 1;
                run.why_ticks[why as usize][1] += 1;
            }
        }
        match run.phase {
            Phase::Before { touched } => {
                if is_up(run.link.kind, feet, &run.link) {
                    run.phase = Phase::After { at: clock };
                    run.up_at = Some(clock);
                } else if clock.wrapping_sub(touched) > ABANDON_TICKS {
                    self.finish(clock, feet, speed, Ending::Abandoned);
                }
            }
            Phase::After { at } => {
                // Back at its speed, or [`AFTER_WINDOW_M`] on from the landing (a route whose next stretch is short never
                // gets the sprint back: the window is what the link itself cost).
                if speed >= CRUISE_SHARE * run.v0 || feet.distance_xz(run.link.landing) >= AFTER_WINDOW_M {
                    self.finish(clock, feet, speed, Ending::Cruising);
                } else if clock.wrapping_sub(at) > SLOW_TICKS {
                    self.finish(clock, feet, speed, Ending::Slow);
                }
            }
        }
    }

    /// One tick of the link given up on: up it, and near its foot; counted when [`GHOST_TICKS`] have passed.
    fn follow_ghost(&mut self, clock: u32, feet: Vec3) {
        let Some(g) = self.ghost.as_mut() else {
            return;
        };
        if feet.distance_xz(g.link.take_off) <= GONE_NEAR_M {
            g.near_ticks += 1;
        }
        if is_up(Kind::Walking, feet, &g.link) {
            g.climbed = true;
        }
        if clock.wrapping_sub(g.since) > GHOST_TICKS {
            self.settle_ghost(feet);
        }
    }

    /// Count the ghost: climbed anyway, still within [`GHOST_STAY_M`] of the take-off, or gone elsewhere.
    fn settle_ghost(&mut self, feet: Vec3) {
        let Some(g) = self.ghost.take() else {
            return;
        };
        let slot = if g.climbed {
            0
        } else if feet.distance_xz(g.link.take_off) <= GHOST_STAY_M {
            2
        } else {
            1
        };
        GHOST[g.kind][g.ended][slot].fetch_add(1, Ordering::Relaxed);
        GHOST[g.kind][g.ended][3].fetch_add((g.near_ticks as f32 * TICK_S * 1000.0) as u64, Ordering::Relaxed);
    }

    fn finish(&mut self, clock: u32, feet: Vec3, _speed: f32, ending: Ending) {
        let Some(run) = self.run.take() else {
            return;
        };
        let rise = run.link.landing.y - run.link.take_off.y;
        let kind = run.link.kind as usize;
        let band = run.link.kind.band(rise);
        let add = |field: Field, by: u64| COUNTS[kind][band][field as usize].fetch_add(by, Ordering::Relaxed);
        match ending {
            Ending::Abandoned => {
                add(Field::Abandoned, 1);
                add(
                    match run.lost {
                        Lost::Silent => Field::GoneSilent,
                        Lost::NoJump => Field::GoneNoJump,
                        Lost::Far => Field::GoneFar,
                    },
                    1,
                );
                let touched = match run.phase {
                    Phase::Before { touched } => touched,
                    Phase::After { at } => at,
                };
                add(Field::GoneMs, (touched.wrapping_sub(run.start) as f32 * TICK_S * 1000.0) as u64);
                if run.pressed_at.is_some() {
                    add(Field::GonePressed, 1);
                }
                if run.last.distance_xz(run.link.take_off) <= GONE_NEAR_M {
                    add(Field::GoneNear, 1);
                }
                let ended = run.route_ended_by.unwrap_or(ENDED_NONE);
                ENDED[kind][ended].fetch_add(1, Ordering::Relaxed);
                // The faces of a rise of 0.8-1.2 m (the bands 3 to 6 of a climb) by method, for the lift's whole-face table.
                if (3..=6).contains(&band) {
                    ENDED_FACE[kind][ended].fetch_add(1, Ordering::Relaxed);
                    press_hist(&run, 1);
                    self.settle_ghost(feet);
                    self.ghost = Some(Ghost { link: run.link, kind, ended, since: clock, near_ticks: 0, climbed: false });
                }
                return;
            }
            Ending::Cut => {
                add(Field::Cut, 1);
                if run.up_at.is_some() && (3..=6).contains(&band) {
                    press_hist(&run, 0);
                }
                return;
            }
            Ending::Slow => {
                add(Field::Slow, 1);
                if (3..=6).contains(&band) {
                    press_hist(&run, 0);
                }
                return;
            }
            Ending::Cruising => {
                if (3..=6).contains(&band) {
                    press_hist(&run, 0);
                }
            }
        }
        let ticks = clock.wrapping_sub(run.start);
        let ms = (ticks as f32 * TICK_S * 1000.0) as u64;
        let ideal_ms = (run.path_m / run.v0 * 1000.0) as u64;
        add(Field::Links, 1);
        add(Field::Ms, ms);
        // The time lost: a link taken faster than the starting speed's path loses none.
        add(Field::ExcessMs, ms.saturating_sub(ideal_ms));
        // The phases: from the start to the first press (to getting up when there was none), in the air to getting up, and
        // from there to the end.
        let ms_of = |ticks: u32| (ticks as f32 * TICK_S * 1000.0) as u64;
        let up = run.up_at.unwrap_or(clock);
        let press = run.pressed_at.unwrap_or(up).clamp(run.start, up);
        add(Field::PreMs, ms_of(press.wrapping_sub(run.start)));
        add(Field::AirMs, ms_of(up.wrapping_sub(press)));
        add(Field::PostMs, ms_of(clock.wrapping_sub(up)));
        for (w, ticks) in run.why_ticks.iter().enumerate() {
            add_why(kind, w, 0, ms_of(ticks[0]));
            add_why(kind, w, 1, ms_of(ticks[1]));
        }
        add(Field::StoppedMs, ms_of(run.stopped_ticks));
        add(Field::SlowedMs, ms_of(run.slowed_ticks));
        if run.v0 >= SPRINT_START_MS {
            add(Field::SprintStarts, 1);
        }
        add(Field::PathMm, (run.path_m * 1000.0) as u64);
        add(Field::StraightMm, (feet.distance_xz(run.from) * 1000.0) as u64);
        add(Field::Presses, u64::from(run.presses));
        // Back-offs: the arrivals backed off from, and each press after the first (a press that did not get up).
        add(Field::Backoffs, u64::from(run.backoffs + run.presses.saturating_sub(1)));
        add(Field::BackoffMs, (run.backoff_ticks as f32 * TICK_S * 1000.0) as u64);
        if run.presses <= 1 {
            add(Field::FirstUp, 1);
        }
        if run.lifted && run.presses == 0 {
            add(Field::LiftedUp, 1);
        }
    }
}

fn same(a: &Link, b: &Link) -> bool {
    (a.take_off - b.take_off).length() < 0.3 && (a.landing - b.landing).length() < 0.3
}

enum Ending {
    Cruising,
    Slow,
    Cut,
    Abandoned,
}

/// What is counted per (kind, band of rise).
#[derive(Clone, Copy, Debug)]
pub enum Field {
    /// Links timed (the body got up and regained its speed).
    Links,
    Ms,
    /// The time over the same path at the starting speed, a link taken faster counting none.
    ExcessMs,
    PathMm,
    /// The distance from where the watch began to where it ended.
    StraightMm,
    Presses,
    Backoffs,
    BackoffMs,
    /// Got up with at most one press (a link that took none counts).
    FirstUp,
    /// Of those, lifted: no press.
    LiftedUp,
    /// The time split: start to the first press (or to getting up), press to getting up, and getting up to the end.
    PreMs,
    AirMs,
    PostMs,
    /// Of the time before the press: stopped, and slowed under the cruise; and the links that began at a sprint.
    StoppedMs,
    SlowedMs,
    SprintStarts,
    /// Up the link but not at speed after [`SLOW_TICKS`]; ended by the next link; given up on before the landing.
    Slow,
    Cut,
    /// The links given up by the reason the motor last had ([`Lost`]), and the milliseconds from the start to the last ask.
    GoneSilent,
    GoneNoJump,
    GoneFar,
    GoneMs,
    Abandoned,
    /// Of the links given up: a jump had been pressed (a climb tried and not got up), and the body ended within
    /// [`GONE_NEAR_M`] of the take-off (it stayed at the face) rather than going on elsewhere.
    GonePressed,
    GoneNear,
}

const FIELDS: usize = Field::GoneNear as usize + 1;
static COUNTS: [[[AtomicU64; FIELDS]; BANDS]; 7] = [const { [const { [const { AtomicU64::new(0) }; FIELDS] }; BANDS] }; 7];

/// The links given up by how the route walked ended: the index in `RouteEnd::ALL`, or [`ENDED_NONE`] if it did not.
static ENDED: [[AtomicU64; ENDED_NONE + 1]; 7] = [const { [const { AtomicU64::new(0) }; ENDED_NONE + 1] }; 7];
pub const ENDED_NONE: usize = 16;
/// The same for the links of a rise of 0.8 to 1.2 m.
static ENDED_FACE: [[AtomicU64; ENDED_NONE + 1]; 7] = [const { [const { AtomicU64::new(0) }; ENDED_NONE + 1] }; 7];
/// A link given up with the body this near its take-off stayed at the face.
const GONE_NEAR_M: f32 = 3.0;

/// The links given up on followed for this long, ticks (10 s), and the body is counted to have stayed at the foot if it is
/// within [`GHOST_STAY_M`] of the take-off at the end.
const GHOST_TICKS: u32 = 300;
const GHOST_STAY_M: f32 = 6.0;
/// What became of the links of a 0.8-1.2 m face given up on: by kind and route end, climbed anyway / went elsewhere /
/// still near the foot (counts), and the milliseconds within [`GONE_NEAR_M`] of the take-off in the 10 s.
static GHOST: [[[AtomicU64; 4]; ENDED_NONE + 1]; 7] = [const { [const { [const { AtomicU64::new(0) }; 4] }; ENDED_NONE + 1] }; 7];

/// The first press of the links of a 0.8-1.2 m face that were walking jumps or sprint jumps, by outcome (got up, pressed
/// and not up): the metres before the take-off (0.25 m bins), to the side of its line (0.1 m), the ground speed (0.5 m/s),
/// the top's depth (0.5 m) and the rise from 0.8 m (0.05 m).
pub const PRESS_BINS: usize = 12;
pub const PRESS_METRICS: usize = 5;
static PRESS: [[[[AtomicU64; PRESS_BINS]; PRESS_METRICS]; 2]; 2] =
    [const { [const { [const { [const { AtomicU64::new(0) }; PRESS_BINS] }; PRESS_METRICS] }; 2] }; 2];

fn press_hist(run: &Run, outcome: usize) {
    let (Some(press), kind) = (run.press, run.link.kind) else {
        return;
    };
    let k = match kind {
        Kind::Walking => 0,
        Kind::SprintBand => 1,
        _ => return,
    };
    let bin = |x: f32, width: f32| ((x / width).max(0.0) as usize).min(PRESS_BINS - 1);
    let rise = run.link.landing.y - run.link.take_off.y;
    let h = &PRESS[k][outcome];
    h[0][bin(press.before, 0.25)].fetch_add(1, Ordering::Relaxed);
    h[1][bin(press.side, 0.1)].fetch_add(1, Ordering::Relaxed);
    h[2][bin(press.speed, 0.5)].fetch_add(1, Ordering::Relaxed);
    if let Some(top) = run.link.top {
        h[3][bin(top, 0.5)].fetch_add(1, Ordering::Relaxed);
    }
    h[4][bin(rise - 0.8, 0.05)].fetch_add(1, Ordering::Relaxed);
}

static WHY: [[[AtomicU64; 2]; 5]; 7] = [const { [const { [const { AtomicU64::new(0) }; 2] }; 5] }; 7];

fn add_why(kind: usize, why: usize, slow: usize, ms: u64) {
    WHY[kind][why][slow].fetch_add(ms, Ordering::Relaxed);
}

/// The counts: by kind ([`Kind::ALL`]), band, [`Field`]; then the stopped and slowed milliseconds before the press by
/// kind and [`Why`] (`[kind][why][stopped, slowed]`).
pub fn counts() -> Vec<u64> {
    let fields = COUNTS.iter().flatten().flatten();
    fields.chain(WHY.iter().flatten().flatten()).chain(ENDED.iter().flatten()).chain(ENDED_FACE.iter().flatten())
        .chain(GHOST.iter().flatten().flatten())
        .chain(PRESS.iter().flatten().flatten().flatten())
        .map(|c| c.load(Ordering::Relaxed))
        .collect()
}

/// The index of (`kind`, `band`, `field`) in [`counts`].
pub fn at(kind: Kind, band: usize, field: Field) -> usize {
    (kind as usize * BANDS + band) * FIELDS + field as usize
}

pub const FIELD_COUNT: usize = FIELDS;

#[cfg(test)]
mod tests {
    use super::*;

    fn link(kind: Kind, rise: f32) -> Link {
        Link { kind, take_off: Vec3::new(5.0, 0.0, 0.0), landing: Vec3::new(5.4, rise, 0.0), top: Some(3.0) }
    }

    /// A link timed from the lining up to the regained speed: the time, the excess over the path at the starting speed,
    /// the presses, in the band of its rise.
    #[test]
    fn a_link_is_timed_from_the_lining_up_to_the_regained_speed() {
        // A kind and band no other test uses: the counts are the process's.
        let (kind, band) = (Kind::Gap, Kind::Gap.band(0.9));
        let before = counts();
        let mut watch = Watch::default();
        let mut clock = 100;
        let mut feet = Vec3::new(2.5, 0.0, 0.0);
        let l = link(kind, 0.9);
        watch.begin(clock, l, feet, 5.0);
        // 10 ticks slowing to 2 m/s, a press, up at tick 20, back to 5 m/s at tick 30.
        for t in 1..=30u32 {
            clock += 1;
            let speed = if t < 12 { 2.0 } else if t < 30 { 3.0 } else { 5.0 };
            feet.x += speed * TICK_S;
            if t == 10 {
                watch.pressed(clock, feet, speed);
            }
            if t >= 20 {
                feet.y = 0.9;
            }
            watch.begin(clock, l, feet, speed);
            watch.tick(clock, feet, speed, Why::Lined);
        }
        assert!(watch.idle(), "finished at the regained speed");
        let after = counts();
        let d = |f: Field| after[at(kind, band, f)] - before[at(kind, band, f)];
        assert_eq!(d(Field::Links), 1);
        assert_eq!(d(Field::Ms), 1_500);
        assert!(d(Field::ExcessMs) > 0 && d(Field::ExcessMs) < 1_500, "slower than 5 m/s over the path: {}", d(Field::ExcessMs));
        assert_eq!(d(Field::Presses), 1);
        assert_eq!(d(Field::FirstUp), 1);
        assert_eq!(d(Field::Backoffs), 0);
    }

    /// A body that never gets up the link is given up on after a while, counted apart, not timed.
    #[test]
    fn a_link_never_got_up_is_abandoned_not_timed() {
        let (kind, band) = (Kind::SprintGap, Kind::SprintGap.band(1.0));
        let before = counts();
        let mut watch = Watch::default();
        let feet = Vec3::new(2.5, 0.0, 0.0);
        watch.begin(0, link(kind, 1.0), feet, 5.0);
        for t in 1..60 {
            watch.tick(t, feet, 1.0, Why::Unknown);
        }
        assert!(watch.idle());
        let after = counts();
        assert_eq!(after[at(kind, band, Field::Abandoned)] - before[at(kind, band, Field::Abandoned)], 1);
        assert_eq!(after[at(kind, band, Field::Links)], before[at(kind, band, Field::Links)]);
    }

    /// The offset of the ghost counts of (`kind`, no route end) in [`counts`].
    fn ghost_at(kind: Kind) -> usize {
        let ended_n = ENDED_NONE + 1;
        let before_ghost = Kind::ALL.len() * (BANDS * FIELDS + 5 * 2 + 2 * ended_n);
        before_ghost + (kind as usize * ended_n + ENDED_NONE) * 4
    }

    /// A link of a 0.8-1.2 m face given up on is followed for 10 s: the body that climbs it anyway is counted so, one that
    /// stays at its foot is counted near, with the seconds it spent there.
    #[test]
    fn a_link_given_up_on_is_followed_to_see_if_the_body_climbs_it_anyway() {
        let kind = Kind::Through; // a kind no other test gives up on
        let foot = Vec3::new(2.5, 0.0, 0.0);
        let top = Vec3::new(5.4, 1.0, 0.0);
        let run = |end_feet: Vec3| {
            let before = counts();
            let mut watch = Watch::default();
            watch.begin(0, link(kind, 1.0), foot, 5.0);
            for t in 1..60 {
                watch.tick(t, foot, 1.0, Why::Unknown);
            }
            for t in 60..60 + GHOST_TICKS + 5 {
                watch.tick(t, end_feet, 1.0, Why::Unknown);
            }
            let after = counts();
            let d = |s: usize| after[ghost_at(kind) + s] - before[ghost_at(kind) + s];
            [d(0), d(1), d(2), d(3)]
        };
        assert_eq!(run(top)[..3], [1, 0, 0], "climbed anyway");
        let stayed = run(foot);
        assert_eq!(stayed[..3], [0, 0, 1], "still at the foot");
        assert!(stayed[3] > 5_000, "the ms within 3 m of the take-off: {stayed:?}");
        assert_eq!(run(Vec3::new(40.0, 0.0, 0.0))[..3], [0, 1, 0], "gone elsewhere");
    }

    #[test]
    fn the_bands_are_a_tenth_of_a_metre_of_rise_and_a_metre_of_drop() {
        assert_eq!(Kind::Walking.band(0.55), 0);
        assert_eq!(Kind::Walking.band(1.05), 5);
        assert_eq!(Kind::SprintBand.band(9.0), BANDS - 1);
        assert_eq!(Kind::Drop.band(-3.2), 3);
    }
}
