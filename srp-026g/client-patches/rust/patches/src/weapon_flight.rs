//! Patch 45's camera flight (client_patch.md section 3.42, round 7): a
//! weapon's upgrade window flies the lobby camera to the lobby's weapon
//! spot, and its close flies it back.
//!
//! Patch 45's NASM (weapon_table_026g.asm) keeps the weapon at the bench and
//! answers the window's start and close: it turns its table on or off and
//! counts a request, with the targets (the spot's point, the bench's view,
//! its zoom; the setter gives the height, the side angle and the distance).
//! This is the flight alone, at `lobby_camera.view` every tick:
//!
//! - the eye moves on a straight line from where it is to where it goes,
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
use crate::vostok::{Context, Flow, GameCell, NoExit};

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

/// The pose a share `s` of the way: the eye on the straight line between
/// the two eyes, the yaw (its nearest turn), the pitch, the distance, the
/// side angle and the zoom by the same share; the point where that view
/// looks from that eye.
fn between(from: &CameraPose, to: &CameraPose, s: f32) -> CameraPose {
    let eye = from.eye().lerp(to.eye(), s);
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
pub fn on_view(ctx: &mut Context<'_>) -> Flow<NoExit> {
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
    table.set_flight(flight.word());
    STATE.set(thread, State { flight, answered });
    Flow::Continue
}

#[cfg(test)]
mod tests;
