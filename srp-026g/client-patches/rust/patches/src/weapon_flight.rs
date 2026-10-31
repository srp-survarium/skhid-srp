//! Patch 45's camera flight (client_patch.md section 3.42, round 8): a
//! weapon's upgrade window flies the lobby camera to the lobby's weapon
//! spot, and its close flies it back.
//!
//! Patch 45's NASM (weapon_table_026g.asm) keeps the weapon at the bench and
//! answers the window's start and close: it turns its table on or off and
//! counts a request, with the targets (the spot's point, the bench's view,
//! its zoom; the setter gives the height, the side angle and the distance).
//! This is the flight alone, at `lobby_camera.view` every tick:
//!
//! - the eye moves on a straight line from where it is to where it goes
//!   (arced over a character's head where the line comes within 0.9 m of
//!   one: one arc over the whole flight, round 8),
//!   the yaw, the pitch, the distance, the side angle and the zoom with it,
//!   all by one smoothstep over [`DURATION`]: no cut, no start or stop at
//!   full speed; the eye of either end is in the room, and so is the line
//!   between them where nothing stands in it (docs/notes/lobby-scenes.md,
//!   the audit of 2026-10-30);
//! - back, it goes to where the stock camera will be: the point and the
//!   view it had, the distance its wheel's target cut by the camera's
//!   collision ray there and clamped as the tick clamps it, so the stock
//!   camera takes over with no snap (patch 45 until round 6 glided to the
//!   wheel's target with no ray, out of the room, and the ray snapped it
//!   back by up to 3 m);
//! - at the bench the distance follows its target (the weapon's framing,
//!   the wheel) by patch 45's glide.
//!
//! While it flies or rests at the bench the camera's own steps hold still,
//! and patch 45's NASM casts no ray ([`FlightWord`]). One enum, [`Flight`],
//! and one transition function, [`Flight::step`].

#![forbid(unsafe_code)]

use ragdoll::math::{PI, Vec3, clamp};

use crate::vostok::lobby_camera::{CameraPose, FlightWord, LobbyCamera, StepTargets, Steps, TableRequest,
                                  WeaponTableState, forward};
use crate::vostok::trace::{self, TraceFile};
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread, site};

/// How long a flight takes (s).
pub const DURATION: f32 = 0.45;
/// The bench distance's glide (patch 45's): this share of its way a second,
/// and this much more (m/s), landing exactly.
const RATE: f32 = 5.0;
const FLOOR: f32 = 0.01;
/// The stock camera's distances (the wheel's, the tick's last clamp once no
/// weapon is looked at).
const LEAST: f32 = 1.5;
const FARTHEST: f32 = 4.6;

/// Where a flight goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    /// The bench's view (the point, the view, the side angle and the zoom)
    /// from its distance's target of now (which the bench's distance then
    /// follows, gliding).
    Bench { view: BenchView, distance: f32 },
    /// The stock camera's pose (its ray's cut taken).
    Home(CameraPose),
}

/// The bench's view but its distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BenchView {
    pub point: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub side: f32,
    pub zoom: f32,
}

impl BenchView {
    fn at(&self, distance: f32) -> CameraPose {
        CameraPose { point: self.point, yaw: self.yaw, pitch: self.pitch, distance, side: self.side, zoom: self.zoom }
    }
}

/// The camera flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flight {
    /// The stock camera's own.
    Stock,
    /// Going to the bench: from where it was, `elapsed` s in, the bench's
    /// distance gliding.
    ToBench { from: CameraPose, view: BenchView, elapsed: f32, distance: f32 },
    /// At the bench.
    AtBench { view: BenchView, distance: f32 },
    /// Going back to the stock camera's pose.
    Home { from: CameraPose, to: CameraPose, elapsed: f32 },
    /// Back at it, for one tick more held there with no ray: the tick's
    /// last clamp (patch 32's min_final) sees the distance at the stock's
    /// least or more and gives the least back (1.5) before the stock ray is
    /// cast again; the next tick gives the camera its steps, and its ray
    /// cuts the distance there as it always does (where the flight put it).
    Landed { at: CameraPose },
}

/// What happened since the last tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Patch 45 asked for a flight; the camera is where it is now.
    Asked { goal: Goal, now: CameraPose },
    /// A tick of `dt` s; the distance's target (the bench's, at the bench).
    Tick { dt: f32, distance_target: f32 },
}

/// What the camera is given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Nothing: the stock camera's.
    Leave,
    /// This pose, its own steps held.
    Hold(CameraPose),
    /// This pose, the flight over: the camera's steps its own again.
    Release(CameraPose),
}

/// 0 at 0, 1 at 1, its slope 0 at both.
fn smoothstep(u: f32) -> f32 {
    let u = clamp(u, 0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// `to` as the nearest turn of it to `from`.
fn nearest_turn(from: f32, to: f32) -> f32 {
    let mut d = to - from;
    while d > PI {
        d -= 2.0 * PI;
    }
    while d < -PI {
        d += 2.0 * PI;
    }
    from + d
}

/// Where the lobby's characters stand (the exe's: the own at the origin, the
/// squad panels' at (+-1, 0, 0.5); 0x21CCD2, 0x21CD33, 0x21CD94), on the floor.
const CHARACTERS: [(f32, f32); 3] = [(0.0, 0.0), (1.0, 0.5), (-1.0, 0.5)];
/// A line nearer a character than this (across the floor) goes over its head.
const CLEAR: f32 = 0.9;
/// The eye's height over a character's head (a character is about 1.85 m tall).
const OVER: f32 = 2.25;
/// The arc's top at most this high (the rooms' ceilings are above 2.6 m).
const TOP: f32 = 2.5;

/// How high the eye's path arcs over the straight line between two eyes: the
/// whole flight rises and falls by `height * 4 s (1 - s)`, and the height is
/// the least that takes the eye over every character the line passes within
/// CLEAR of (at its share of the way there; the camp's spot is behind the
/// character: the line went through it, the game's capture of 2026-10-30).
/// One smooth arc over the whole flight: round 7 raised the eye in a bump
/// only where it came near a character, 0.8 m up and down within a tenth of
/// a second, the jump the owner saw in the Room (2026-10-31).
fn arc_height(from: Vec3, to: Vec3) -> f32 {
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    let across = dx * dx + dz * dz;
    let height = CHARACTERS
        .iter()
        .map(|&(x, z)| {
            let s = if across > 1e-6 { clamp(((x - from.x) * dx + (z - from.z) * dz) / across, 0.0, 1.0) } else { 0.0 };
            let at = from.lerp(to, s);
            let (ex, ez) = (at.x - x, at.z - z);
            let near = clamp(1.0 - (ex * ex + ez * ez) / (CLEAR * CLEAR), 0.0, 1.0);
            let rise = (OVER - at.y) * near * near;
            // the arc gives 4 s (1 - s) of its height there; near either end
            // at least a quarter (an eye that starts beside a head stays low)
            if rise > 0.0 { rise / clamp(4.0 * s * (1.0 - s), 0.25, 1.0) } else { 0.0 }
        })
        .fold(0.0, |a: f32, b| if b > a { b } else { a });
    let middle = (from.y + to.y) * 0.5;
    clamp(height, 0.0, if TOP > middle { TOP - middle } else { 0.0 })
}

/// The pose a share `s` of the way: the eye on the straight line between
/// the two eyes, raised by one arc over the whole way where the line passes a
/// character ([`arc_height`]), the yaw (its nearest turn), the pitch, the
/// distance, the side angle and the zoom by the same share; the point where
/// that view looks from that eye.
fn between(from: &CameraPose, to: &CameraPose, s: f32) -> CameraPose {
    let (a, b) = (from.eye(), to.eye());
    let line = a.lerp(b, s);
    let eye = Vec3::new(line.x, line.y + arc_height(a, b) * 4.0 * s * (1.0 - s), line.z);
    let yaw = from.yaw + (nearest_turn(from.yaw, to.yaw) - from.yaw) * s;
    let pitch = from.pitch + (to.pitch - from.pitch) * s;
    let distance = from.distance + (to.distance - from.distance) * s;
    CameraPose {
        point: eye + forward(yaw, pitch) * distance,
        yaw,
        pitch,
        distance,
        side: from.side + (to.side - from.side) * s,
        zoom: from.zoom + (to.zoom - from.zoom) * s,
    }
}

/// Patch 45's glide of one value: RATE * dt of the way and FLOOR * dt more,
/// onto the target when that is the whole way.
fn glide(now: f32, target: f32, dt: f32) -> f32 {
    let way = target - now;
    let step = way.abs() * clamp(RATE * dt, 0.0, 1.0) + FLOOR * dt;
    if step >= way.abs() { target } else if way > 0.0 { now + step } else { now - step }
}

impl Flight {
    /// The flight's word for patch 45's NASM.
    pub fn word(&self) -> FlightWord {
        match self {
            Flight::Stock => FlightWord::Stock,
            Flight::ToBench { .. } | Flight::Home { .. } | Flight::Landed { .. } => FlightWord::Moving,
            Flight::AtBench { .. } => FlightWord::Resting,
        }
    }

    /// The one transition: (state, event) to the next state and what the
    /// camera is given. Total over both.
    pub fn step(self, event: Event) -> (Flight, Action) {
        match (self, event) {
            (_, Event::Asked { goal: Goal::Bench { view, distance }, now }) => {
                (Flight::ToBench { from: now, view, elapsed: 0.0, distance }, Action::Hold(now))
            }
            (_, Event::Asked { goal: Goal::Home(to), now }) => {
                (Flight::Home { from: now, to, elapsed: 0.0 }, Action::Hold(now))
            }
            (Flight::Stock, Event::Tick { .. }) => (Flight::Stock, Action::Leave),
            (Flight::Landed { at }, Event::Tick { .. }) => (Flight::Stock, Action::Release(at)),
            (Flight::ToBench { from, view, elapsed, distance }, Event::Tick { dt, distance_target }) => {
                let elapsed = elapsed + dt;
                let distance = glide(distance, distance_target, dt);
                let to = view.at(distance);
                if elapsed >= DURATION {
                    (Flight::AtBench { view, distance }, Action::Hold(to))
                } else {
                    let pose = between(&from, &to, smoothstep(elapsed / DURATION));
                    (Flight::ToBench { from, view, elapsed, distance }, Action::Hold(pose))
                }
            }
            (Flight::AtBench { view, distance }, Event::Tick { dt, distance_target }) => {
                let distance = glide(distance, distance_target, dt);
                (Flight::AtBench { view, distance }, Action::Hold(view.at(distance)))
            }
            (Flight::Home { from, to, elapsed }, Event::Tick { dt, .. }) => {
                let elapsed = elapsed + dt;
                if elapsed >= DURATION {
                    (Flight::Landed { at: to }, Action::Hold(to))
                } else {
                    let pose = between(&from, &to, smoothstep(elapsed / DURATION));
                    (Flight::Home { from, to, elapsed }, Action::Hold(pose))
                }
            }
        }
    }
}

/// The flight, and the request count it last answered.
#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    flight: Flight,
    answered: u32,
}

static STATE: GameCell<State> = GameCell::new(State { flight: Flight::Stock, answered: 0 });

/// The goal of patch 45's request: the bench, or the stock camera's pose
/// with its ray's cut (`cut`: the ray from the point back along forward).
pub fn goal_of(request: &TableRequest, targets: &StepTargets, cut: impl FnOnce(Vec3, Vec3, f32) -> f32) -> Goal {
    let point = Vec3::new(request.x, targets.height, request.z);
    if request.table_on {
        let view = BenchView { point, yaw: request.yaw, pitch: request.pitch, side: targets.side, zoom: request.zoom };
        Goal::Bench { view, distance: targets.distance }
    } else {
        let f = forward(request.yaw, request.pitch);
        let distance = clamp(cut(point, f, targets.distance), LEAST, FARTHEST);
        Goal::Home(CameraPose { point, yaw: request.yaw, pitch: request.pitch, distance, side: targets.side,
                                zoom: request.zoom })
    }
}

/// `lobby_camera.view`: the tick with its yaw and pitch final.
pub fn on_view(ctx: &mut Context<'_, site::LobbyCameraView>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some((mut camera, dt)) = LobbyCamera::at(ctx) else { return Flow::Continue };
    let mut table = WeaponTableState::of(thread);
    let request = table.request();
    let targets = camera.targets();
    let State { mut flight, mut answered } = STATE.get(thread);
    if request.count != answered {
        let goal = goal_of(&request, &targets, |p, f, d| camera.ray_cut(p, f, d));
        flight = flight.step(Event::Asked { goal, now: camera.pose() }).0;
        answered = request.count;
    }
    let (flight, action) = flight.step(Event::Tick { dt, distance_target: targets.distance });
    match action {
        Action::Leave => {}
        Action::Hold(pose) => camera.set(&pose, Steps::Held),
        Action::Release(pose) => camera.set(&pose, Steps::Released),
    }
    if let Action::Hold(pose) | Action::Release(pose) = action {
        trace_pose(thread, dt, &flight, &pose);
    }
    table.set_flight(flight.word());
    STATE.set(thread, State { flight, answered });
    Flow::Continue
}

/// A diagnostics build's trace of the flight (never a release's): a line of
/// `srp_weapon_flight.csv` in the user data folder for every tick the flight
/// gives the camera a pose: the tick's time (ms), the eye and the point (mm),
/// the yaw and the pitch (mrad), the distance (mm) and the state (0 to the
/// bench, 1 at it, 2 home, 3 landed, 4 stock), for checking the path frame by
/// frame.
fn trace_pose(thread: &Thread, dt: f32, flight: &Flight, pose: &CameraPose) {
    static LOG: GameCell<Option<TraceFile>> = GameCell::new(None);
    if !trace::enabled() {
        return;
    }
    let file = match LOG.get(thread) {
        Some(file) => file,
        None => {
            let Some(file) = TraceFile::open(thread, "srp_weapon_flight.csv") else { return };
            LOG.set(thread, Some(file));
            file
        }
    };
    let state = match flight {
        Flight::ToBench { .. } => 0,
        Flight::AtBench { .. } => 1,
        Flight::Home { .. } => 2,
        Flight::Landed { .. } => 3,
        Flight::Stock => 4,
    };
    let eye = pose.eye();
    let thousand = |v: f32| if v >= 0.0 { (v * 1000.0 + 0.5) as i32 } else { (v * 1000.0 - 0.5) as i32 };
    let values = [thousand(dt), thousand(eye.x), thousand(eye.y), thousand(eye.z), thousand(pose.point.x),
                  thousand(pose.point.y), thousand(pose.point.z), thousand(pose.yaw), thousand(pose.pitch),
                  thousand(pose.distance), state];
    let mut line = [0u8; 160];
    let mut n = 0;
    for (k, v) in values.iter().enumerate() {
        if k > 0 {
            line[n] = b',';
            n += 1;
        }
        n = decimal(&mut line, n, *v);
    }
    line[n] = b'\n';
    file.write(thread, &line[..n + 1]);
}

/// `v` in decimal into `out` at `n`; the end.
fn decimal(out: &mut [u8], n: usize, v: i32) -> usize {
    let mut n = n;
    if v < 0 {
        out[n] = b'-';
        n += 1;
    }
    let mut digits = [0u8; 10];
    let mut k = 0;
    let mut u = v.unsigned_abs();
    loop {
        digits[k] = b'0' + (u % 10) as u8;
        k += 1;
        u /= 10;
        if u == 0 {
            break;
        }
    }
    while k > 0 {
        k -= 1;
        out[n] = digits[k];
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests;
