//! Patch 132: the orbit camera stays on the player's side of the level.
//!
//! Before the start (the input handler's warmup mode, 3) the camera orbits
//! the player at 1.5 to 4.6 m (patch 31's limits): update_inverted_view
//! (0x2422B0) puts the eye out along the view's back and never tests the
//! level, so at many spawns it sat inside a wall or on the far side of a
//! thin one. The lobby's camera cuts its distance at the level's first hit
//! (lobby_camera::process_collision 0x2844D0: ray_test from the point it
//! looks at, group 10h, mask 8); this does the same for the orbit, after the
//! view's update and before the director gives the view on.
//!
//! The ray's hit gives the pull the eye needs: how far in from the orbit it
//! must be to stay `MARGIN` short of the hit (the near plane is 0.05 m). The
//! eye is never shown behind that wall: a pull short of it is made up at
//! once. So that this seldom has to happen, the pull starts early and goes
//! in smoothly: while the orbit turns, a second ray goes where its way will
//! be `LEAD` s on at this frame's turn, and the pull that wall would need is
//! wanted already, approached at `IN_RATE`. Back out it goes at the slower
//! `OUT_RATE`, a share of what is left each second (the level's meshes are
//! detailed, and a ray that leaves a bar or a branch near the player would
//! otherwise throw the eye out again). The pull is taken from the distance
//! the wheel keeps (+354h, not changed), so the wheel moves the eye at once
//! as before. A new orbit (the warmup's first frame, another player) starts
//! at the pull it needs.
//!
//! The eye is never nearer the pivot than `NEAREST`, out of the body (0.69d's
//! third-person camera's rule, third_person_camera_controller::tick
//! 0x2002AE3B0). Where the way has less room (a player backed against a wall
//! or tucked in a corner), the eye stays on the way in front of the wall and
//! is lifted off it, up (the level's up: along every upright wall) or, under
//! a ceiling, to a side, just as far as makes it `NEAREST` out, each lift
//! tested by a ray of its own. The view's turn is always the game's: the
//! drag turns the camera wherever the eye is. (The first version held the
//! view of the frame before, and turned a new orbit sideways, where the way
//! had no room: the drag did nothing until the way had it again, "the camera
//! gets stuck", the owner, 2026-11-27.)
//!
//! The camera alone: nothing a match computes.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, clamp, exp_neg, max, min, sqrt};

use crate::vostok::camera::Orbit;
use crate::vostok::world::PlayerId;
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread, clock};

/// The lobby camera's ray (process_collision 0x2844E4..0x2844EA): the
/// level's collision that bullets hit (cgroup 8 / cmask 16), which follows
/// the drawn walls; not the players, their hit models or the grenades.
const GROUP: u32 = 0x10;
const MASK: u32 = 0x08;
/// How far in front of a hit the eye stays (m), beyond the near plane.
const MARGIN: f32 = 0.2;
/// The nearest the eye comes to the pivot (m), out of the body: 0.69d's
/// third-person camera's (0x2002AE3FF, 0x2002AE813), from the same pivot
/// (the place raised 1.4 m) with the same margin.
const NEAREST: f32 = 0.75;
/// The share of the rest of the way the pull goes in a second, toward a
/// wall the turn is coming to (in) and back out: 1 - e^(-rate t) of it
/// after t s (in: 95 % in 0.3 s; out: in 1 s). A lift goes back by
/// `OUT_RATE` too.
const IN_RATE: f32 = 10.0;
const OUT_RATE: f32 = 3.0;
/// How far on the second ray looks (s): where the orbit's way will be at
/// the turn since the last frame.
const LEAD: f32 = 0.3;
/// The furthest the second ray is from the way (the tangent of the angle:
/// 31 degrees), however fast the turn.
const LONGEST_LEAD: f32 = 0.6;
/// A turn that would move the way less than this in `LEAD` s (the same
/// measure: half a degree) is none: no second ray.
const NO_TURN: f32 = 0.01;
/// The longest frame the pull steps by (s): a stall moves it no further.
const LONGEST_FRAME: f32 = 0.1;
/// A pull or a lift below this (m) with none wanted is none: the game's
/// view as it is.
const NO_PULL: f32 = 0.001;
/// How long a lift takes to go from one way off to another (s).
const SWITCH: f32 = 0.25;

/// The pull of the camera's orbit, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pull {
    /// No orbit last frame.
    Off,
    /// `player`'s orbit, its eye along the unit `way` from the pivot, `pull`
    /// m in from its distance and lifted off it by `lift`, at `at` (the
    /// counter's milliseconds, wrapping).
    On { player: PlayerId, way: Vec3, pull: f32, lift: Lift, at: u32 },
}

/// A way off the orbit's way the eye is lifted along: the level's up, the
/// view's (level) right or its left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Off {
    Up,
    Right,
    Left,
}

impl Off {
    /// The unit move, `right` the view's right.
    fn unit(self, right: Vec3) -> Vec3 {
        match self {
            Off::Up => Vec3::Y,
            Off::Right => right,
            Off::Left => -right,
        }
    }

    /// `by` m this way, as up and right.
    fn parts(self, by: f32) -> (f32, f32) {
        match self {
            Off::Up => (by, 0.0),
            Off::Right => (0.0, by),
            Off::Left => (0.0, -by),
        }
    }
}

/// The eye off the orbit's way, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Lift {
    /// On the way.
    OnWay,
    /// `by` m off it along `off`: further out at once with what the room
    /// wants (the eye out of the body), back by `OUT_RATE`.
    Along { off: Off, by: f32 },
    /// Going from `from` (m up and right) to `by` m along `to` (what the
    /// room wants this frame) in `SWITCH` s, `gone` s of it gone.
    Moving { from: (f32, f32), to: Off, by: f32, gone: f32 },
}

impl Lift {
    /// The lift as a move of the eye, `right` the view's right.
    fn offset(self, right: Vec3) -> Vec3 {
        let (up, side) = self.parts();
        Vec3::Y * up + right * side
    }

    /// How far up and right.
    fn parts(self) -> (f32, f32) {
        match self {
            Lift::OnWay => (0.0, 0.0),
            Lift::Along { off, by } => off.parts(by),
            Lift::Moving { from, to, by, gone } => {
                let (x, (up, side)) = (clamp(gone / SWITCH, 0.0, 1.0), to.parts(by));
                let k = x * x * (3.0 - 2.0 * x);
                (from.0 + (up - from.0) * k, from.1 + (side - from.1) * k)
            }
        }
    }

    fn moving(from: (f32, f32), to: Off, by: f32, gone: f32) -> Lift {
        if gone >= SWITCH { Lift::Along { off: to, by } } else { Lift::Moving { from, to, by, gone } }
    }

    /// The side it is on: the left, or else the right.
    fn left(self) -> bool {
        self.parts().1 < 0.0
    }

    fn scaled(self, k: f32) -> Lift {
        match self {
            Lift::OnWay => Lift::OnWay,
            Lift::Along { off, by } => Lift::Along { off, by: by * k },
            Lift::Moving { from, to, by, gone } => Lift::Moving { from: (from.0 * k, from.1 * k), to, by: by * k, gone },
        }
    }

    /// This frame's lift, `dt` s after the last frame's (`self`), toward
    /// `wanted` (None: on the way).
    fn next(self, wanted: Option<(Off, f32)>, dt: f32) -> Lift {
        match (self, wanted) {
            (Lift::OnWay, None) => Lift::OnWay,
            (Lift::OnWay, Some((off, by))) => Lift::Along { off, by },
            (Lift::Along { off, by: last }, Some((o, by))) if o == off => {
                Lift::Along { off, by: if by >= last { by } else { by + (last - by) * exp_neg(OUT_RATE * dt) } }
            }
            (Lift::Along { off, by }, None) => {
                let by = by * exp_neg(OUT_RATE * dt);
                if by < NO_PULL { Lift::OnWay } else { Lift::Along { off, by } }
            }
            // going on to that way off, or (none wanted) to none along it
            (Lift::Moving { from, to, gone, .. }, None) => Lift::moving(from, to, 0.0, gone + dt),
            (Lift::Moving { from, to, gone, .. }, Some((o, by))) if o == to => Lift::moving(from, to, by, gone + dt),
            // to another way off, from where the eye is
            (_, Some((to, by))) => Lift::Moving { from: self.parts(), to, by, gone: dt },
        }
    }
}

/// The orbit as the game made it this frame, from its pivot.
#[derive(Clone, Copy, Debug)]
struct Way {
    /// The unit way from the pivot to the game's eye.
    way: Vec3,
    /// How far out the game's eye is.
    distance: f32,
    /// The view's right (its first row: level, the yaw's).
    right: Vec3,
}

static PULL: GameCell<Pull> = GameCell::new(Pull::Off);

/// update_camera after the view's update (`camera.viewed`), before patch
/// 133's and 156's handlers there: they read the eye this leaves.
pub fn on_viewed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(mut orbit) = Orbit::at(ctx) else {
        PULL.set(thread, Pull::Off);
        return Flow::Continue;
    };
    let pivot = orbit.pivot();
    let out = orbit.eye() - pivot;
    let Some(way) = out.direction() else {
        PULL.set(thread, Pull::Off);
        return Flow::Continue;
    };
    let player = orbit.player().id();
    let world = orbit.player().physics_world();
    // how far on from `from` (from the pivot) along the unit `way` the eye can be, at most `length`
    let room_on = |from: Vec3, way: Vec3, length: f32| {
        let start = pivot + from;
        let hit = world.as_ref().and_then(|world| {
            world.ray_test([start.x, start.y, start.z], [way.x, way.y, way.z], length + MARGIN, GROUP, MASK)
        });
        room(length, hit.map(|hit| (Vec3::new(hit.point[0], hit.point[1], hit.point[2]) - start).dot(way)))
    };
    let game = Way { way, distance: out.length(), right: orbit.turn()[0] };
    let (state, eye) = frame(PULL.get(thread), player, game, now(thread), room_on);
    PULL.set(thread, state);
    if let Some(eye) = eye {
        orbit.set_eye(pivot + eye);
    }
    Flow::Continue
}

fn now(thread: &Thread) -> u32 {
    clock::milliseconds(thread)
}

/// One frame of `player`'s orbit after `state`: the state after it, and
/// where the eye goes from the pivot (None: the game's, as it is). `room`
/// is how far on from a point (from the pivot) along a unit way the eye can
/// be, at most the length given (a ray).
fn frame(state: Pull, player: PlayerId, game: Way, now: u32, mut room: impl FnMut(Vec3, Vec3, f32) -> f32) -> (Pull, Option<Vec3>) {
    let Way { way, distance, right } = game;
    let free = room(Vec3::ZERO, way, distance);
    let ahead = way_ahead(state, player, way, now).map_or(0.0, |ahead| distance - room(Vec3::ZERO, ahead, distance));
    let pull = step(state, player, distance - free, ahead, now);
    let nearest = min(NEAREST, distance);
    // how far out on the way: never behind its wall, never nearer than `nearest` where it has the room
    let along = if free >= nearest { max(distance - pull, nearest) } else { free };
    let on_way = way * along;
    let last = match state {
        Pull::On { player: p, lift, at, .. } if p == player => Some((lift, at)),
        _ => None,
    };
    let left = last.is_some_and(|(lift, _)| lift.left());
    let wanted = (along < nearest).then(|| lift_off(way, along, nearest, right, left, &mut room)).flatten();
    let lift = last.map_or(Lift::OnWay.next(wanted, 0.0), |(lift, at)| lift.next(wanted, elapsed(at, now)));
    // the lift too stays in front of what it meets
    let off = lift.offset(right);
    let lift = off.direction().map_or(lift, |unit| lift.scaled(room(on_way, unit, off.length()) / off.length()));
    let state = Pull::On { player, way, pull, lift, at: now };
    let off = lift.offset(right);
    let eye = (off != Vec3::ZERO || along < distance).then(|| on_way + off);
    (state, eye)
}

/// The lift that puts the eye `nearest` out from the pivot, from `along` m
/// out on the unit `way` (less than `nearest`): up, or else to the side, the
/// right first unless the lift was on the `left`, the first with the room
/// `room` finds; with none, the one that comes furthest out (None: none
/// comes out at all).
fn lift_off(way: Vec3, along: f32, nearest: f32, right: Vec3, left: bool, mut room: impl FnMut(Vec3, Vec3, f32) -> f32) -> Option<(Off, f32)> {
    let sides = if left { [Off::Left, Off::Right] } else { [Off::Right, Off::Left] };
    let on_way = way * along;
    let mut best = (along, None);
    for off in [Off::Up, sides[0], sides[1]] {
        let unit = off.unit(right);
        let needed = rise(along, way.dot(unit), nearest);
        let r = room(on_way, unit, needed);
        if r >= needed {
            return Some((off, r));
        }
        let out = (on_way + unit * r).length();
        if out > best.0 {
            best = (out, Some((off, r)));
        }
    }
    best.1
}

/// How far from `along` m out on a way the eye must go along a unit
/// direction (`cos` the cosine between them) to be `nearest` out.
fn rise(along: f32, cos: f32, nearest: f32) -> f32 {
    -along * cos + sqrt(along * along * (cos * cos - 1.0) + nearest * nearest)
}

/// How far out on a way the eye can be, `length` the furthest: `MARGIN`
/// short of the level's first hit `hit` m out (none, or a hit that is no
/// number: all of it).
fn room(length: f32, hit: Option<f32>) -> f32 {
    match hit {
        Some(hit) if hit.is_finite() => clamp(hit - MARGIN, 0.0, length),
        _ => length,
    }
}

/// The seconds from `at` to `now` (the counter's milliseconds, wrapping), at
/// most `LONGEST_FRAME`.
fn elapsed(at: u32, now: u32) -> f32 {
    clamp(now.wrapping_sub(at) as f32 / 1000.0, 0.0, LONGEST_FRAME)
}

/// Where the unit `way` of `player`'s orbit will be in `LEAD` s at its turn
/// since the last frame, at most `LONGEST_LEAD` off; None with no frame of
/// that orbit before, or no turn.
fn way_ahead(state: Pull, player: PlayerId, way: Vec3, now: u32) -> Option<Vec3> {
    let Pull::On { player: p, way: last, at, .. } = state else { return None };
    let dt = now.wrapping_sub(at) as f32 / 1000.0;
    if p != player || dt <= 0.0 || dt > LONGEST_FRAME {
        return None;
    }
    let turn = (way - last) * (LEAD / dt);
    let far = turn.length();
    if far < NO_TURN {
        return None;
    }
    (way + turn * (clamp(far, 0.0, LONGEST_LEAD) / far)).direction()
}

/// The pull of this frame: moved toward the larger of `needed` (by the
/// wall on the way) and `ahead` (by the wall the turn is coming to) by the
/// time since the last frame, and never less than `needed`; a new orbit's
/// is `needed`.
fn step(state: Pull, player: PlayerId, needed: f32, ahead: f32, now: u32) -> f32 {
    match state {
        Pull::On { player: p, pull, at, .. } if p == player => {
            let wanted = needed.max(ahead);
            let rate = if wanted > pull { IN_RATE } else { OUT_RATE };
            let pull = wanted + (pull - wanted) * exp_neg(rate * elapsed(at, now));
            if wanted == 0.0 && pull < NO_PULL { 0.0 } else { pull.max(needed) }
        }
        _ => needed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragdoll::math::{PI, abs, sin_cos};

    const WAY: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    fn player(n: u32) -> PlayerId {
        PlayerId::for_test(n)
    }

    fn on(n: u32, pull: f32, at: u32) -> Pull {
        Pull::On { player: player(n), way: WAY, pull, lift: Lift::OnWay, at }
    }

    /// Walls: planes through a point (from the pivot) facing the pivot.
    type Walls = [(Vec3, Vec3)];

    /// The room a ray finds among `walls`, as the patch's.
    fn rays(walls: &Walls) -> impl FnMut(Vec3, Vec3, f32) -> f32 + '_ {
        move |from, way, length| {
            let hit = walls
                .iter()
                .filter(|(_, n)| way.dot(*n) < 0.0)
                .map(|(p, n)| (*p - from).dot(*n) / way.dot(*n))
                .filter(|t| (0.0..=length + MARGIN).contains(t))
                .fold(None, |best: Option<f32>, t| Some(best.map_or(t, |b| b.min(t))));
            room(length, hit)
        }
    }

    /// The game's orbit at `yaw`, looking down 0.3 rad, 3 m out: the eye
    /// on the view's back and a fifth of that to its right.
    fn orbit(yaw: f32) -> Way {
        let ((sy, cy), (sp, cp)) = (sin_cos(yaw), sin_cos(0.3));
        let forward = Vec3::new(sy * cp, -sp, cy * cp);
        let right = Vec3::new(cy, 0.0, -sy);
        let out = (right * 0.2 - forward) * 3.0;
        Way { way: out.direction().unwrap(), distance: out.length(), right }
    }

    fn upright(bearing: f32, at: f32) -> (Vec3, Vec3) {
        let (s, c) = sin_cos(bearing);
        let along = Vec3::new(s, 0.0, c);
        (along * at, -along)
    }

    #[test]
    fn the_room_keeps_the_eye_short_of_the_hit() {
        assert_eq!(room(3.0, None), 3.0);
        assert_eq!(room(3.0, Some(3.2)), 3.0);
        assert!((room(3.0, Some(1.5)) - 1.3).abs() < 1e-6);
        assert!((room(3.0, Some(3.05)) - 2.85).abs() < 1e-6);
        assert_eq!(room(3.0, Some(0.1)), 0.0);
        assert_eq!(room(3.0, Some(f32::NAN)), 3.0);
    }

    #[test]
    fn a_new_orbit_starts_at_the_pull_it_needs() {
        assert_eq!(step(Pull::Off, player(1), 1.7, 0.0, 500), 1.7);
        assert_eq!(step(on(2, 0.0, 400), player(1), 1.7, 0.0, 500), 1.7);
    }

    #[test]
    fn the_wall_on_the_way_pulls_at_once_and_lets_out_smoothly() {
        assert_eq!(step(on(1, 0.0, 1000), player(1), 2.0, 0.0, 1016), 2.0);
        let out = step(on(1, 2.0, 0), player(1), 0.0, 0.0, 100);
        assert!((out - 2.0 * exp_neg(0.3)).abs() < 1e-4, "{out:?}");
        let mut s = (out, 100);
        for t in 0..400 {
            let at = 116 + 16 * t;
            s = (step(on(1, s.0, s.1), player(1), 0.0, 0.0, at), at);
        }
        assert_eq!(s.0, 0.0);
    }

    #[test]
    fn the_wall_ahead_pulls_smoothly_and_never_under_the_one_on_the_way() {
        // only ahead: in by 1 - e^(-1) of the way in 100 ms
        let s = step(on(1, 0.0, 1000), player(1), 0.0, 2.0, 1100);
        assert!((s - 2.0 * (1.0 - exp_neg(1.0))).abs() < 1e-4, "{s:?}");
        // the wall arrives on the way before the pull is there: the rest at once
        assert_eq!(step(on(1, s, 1100), player(1), 2.0, 2.0, 1116), 2.0);
        // a stall moves it a frame's worth
        let stalled = step(on(1, 0.0, 1000), player(1), 0.0, 2.0, 61_000);
        assert!((stalled - s).abs() < 1e-6);
    }

    #[test]
    fn the_way_ahead_follows_the_turn() {
        let turned = Vec3::new(0.01, 0.0, 1.0).direction().unwrap();
        // 0.01 in 10 ms: 0.3 off in 0.3 s
        let ahead = way_ahead(on(1, 0.0, 1000), player(1), turned, 1010).unwrap();
        assert!((ahead.x / ahead.z - 0.31).abs() < 0.01, "{ahead:?}");
        // a faster turn: no further than LONGEST_LEAD off
        let ahead = way_ahead(on(1, 0.0, 1000), player(1), turned, 1001).unwrap();
        assert!((ahead.x / ahead.z - (0.01 + LONGEST_LEAD)).abs() < 0.01, "{ahead:?}");
        // no turn, no frame before, another player's, the same millisecond, a stall: none
        assert_eq!(way_ahead(on(1, 0.0, 1000), player(1), WAY, 1010), None);
        assert_eq!(way_ahead(Pull::Off, player(1), turned, 1010), None);
        assert_eq!(way_ahead(on(2, 0.0, 1000), player(1), turned, 1010), None);
        assert_eq!(way_ahead(on(1, 0.0, 1000), player(1), turned, 1000), None);
        assert_eq!(way_ahead(on(1, 0.0, 1000), player(1), turned, 5000), None);
    }

    #[test]
    fn a_rise_puts_the_eye_at_the_nearest() {
        for (along, cos) in [(0.3, 0.0), (0.3, 0.3), (0.0, 0.5), (0.7, -0.4), (0.5, 0.99)] {
            let way = Vec3::new(0.0, 0.0, 1.0);
            let off = Vec3::new(sqrt(1.0 - cos * cos), 0.0, cos);
            let eye = way * along + off * rise(along, cos, 0.75);
            assert!((eye.length() - 0.75).abs() < 1e-5, "{along} {cos}: {eye:?}");
        }
    }

    #[test]
    fn a_lift_goes_out_at_once_back_slowly_and_to_another_way_smoothly() {
        let raised = |by| Lift::Along { off: Off::Up, by };
        // off the way, and further out the same way: at once
        assert_eq!(Lift::OnWay.next(Some((Off::Up, 0.6)), 0.016), raised(0.6));
        assert_eq!(raised(0.3).next(Some((Off::Up, 0.6)), 0.016), raised(0.6));
        // back by OUT_RATE, to the way at last
        let Lift::Along { by, .. } = raised(0.6).next(Some((Off::Up, 0.2)), 0.1) else { panic!() };
        assert!((by - (0.2 + 0.4 * exp_neg(0.3))).abs() < 1e-5, "{by}");
        let Lift::Along { by, .. } = raised(0.6).next(None, 0.1) else { panic!() };
        assert!((by - 0.6 * exp_neg(0.3)).abs() < 1e-5, "{by}");
        assert_eq!(raised(0.0005).next(None, 0.016), Lift::OnWay);
        // to the other side, then up: from where the eye is, in SWITCH s, then along it
        let mut lift = Lift::Along { off: Off::Right, by: 0.6 }.next(Some((Off::Left, 0.6)), 0.016);
        assert_eq!(lift, Lift::Moving { from: (0.0, 0.6), to: Off::Left, by: 0.6, gone: 0.016 });
        lift = lift.next(Some((Off::Left, 0.5)), 0.109);
        assert!(lift.parts().0 == 0.0 && (lift.parts().1 - 0.05).abs() < 1e-6, "{:?}", lift.parts());
        lift = lift.next(Some((Off::Up, 0.5)), 0.016);
        assert!(matches!(lift, Lift::Moving { to: Off::Up, from: (0.0, side), .. } if (side - 0.05).abs() < 1e-6), "{lift:?}");
        let mut frames = 1;
        while matches!(lift, Lift::Moving { .. }) {
            lift = lift.next(Some((Off::Up, 0.5)), 0.016);
            frames += 1;
        }
        assert_eq!((lift, frames), (raised(0.5), 16));
        // moving, then none wanted: to none, then down
        let lift = Lift::Moving { from: (0.0, 0.4), to: Off::Up, by: 0.5, gone: 0.2 }.next(None, 0.05);
        assert_eq!(lift, raised(0.0));
        assert_eq!(lift.next(None, 0.016), Lift::OnWay);
        assert!(!Lift::Moving { from: (0.2, 0.4), to: Off::Up, by: 0.1, gone: 0.0 }.left());
        assert!(Lift::Along { off: Off::Left, by: 0.1 }.left());
    }

    #[test]
    fn the_lift_is_up_else_the_side_it_was_on_else_the_one_with_room() {
        let Way { way, right, .. } = orbit(0.0);
        let out = |off: Off, by: f32| (way * 0.3 + off.unit(right) * by).length();
        // nothing above: up, as far as makes it 0.75 m out
        let (off, by) = lift_off(way, 0.3, NEAREST, right, false, rays(&[])).unwrap();
        assert!(off == Off::Up && (out(off, by) - NEAREST).abs() < 1e-5, "{off:?} {by}");
        // a ceiling 0.5 m over the pivot: the right
        let ceiling = [(Vec3::Y * 0.5, -Vec3::Y)];
        let (off, by) = lift_off(way, 0.3, NEAREST, right, false, rays(&ceiling)).unwrap();
        assert!(off == Off::Right && (out(off, by) - NEAREST).abs() < 1e-5, "{off:?} {by}");
        // on the left last frame: the left
        assert_eq!(lift_off(way, 0.3, NEAREST, right, true, rays(&ceiling)).unwrap().0, Off::Left);
        // a wall at the right too: the left
        let walls = [ceiling[0], (right * 0.4, -right)];
        assert_eq!(lift_off(way, 0.3, NEAREST, right, false, rays(&walls)).unwrap().0, Off::Left);
        // no room anywhere: the one that comes furthest out (here up), in front of its wall
        let walls = [ceiling[0], (right * 0.4, -right), (right * -0.3, right)];
        let (off, by) = lift_off(way, 0.3, NEAREST, right, false, rays(&walls)).unwrap();
        assert!(off == Off::Up && (by - (0.5 - MARGIN - way.y * 0.3)).abs() < 1e-5, "{off:?} {by}");
        // none that comes out at all
        let walls = [(Vec3::Y * 0.2, -Vec3::Y), (right * 0.2, -right), (right * -0.2, right)];
        assert_eq!(lift_off(way, 0.0, NEAREST, right, false, rays(&walls)), None);
    }

    /// The player drags the orbit round and round by `walls`, 3 degrees a
    /// frame and back 1: the eye always in front of them, `NEAREST` out or
    /// more but in `under` frames a turn, its steps smooth.
    fn sweep(walls: &Walls, under: u32) {
        for degrees in [3.0f32, -1.0] {
            let (mut state, mut last, mut nearer) = (Pull::Off, None::<Vec3>, 0);
            let frames = (720.0 / abs(degrees)) as u32;
            for k in 0..frames {
                let yaw = PI / 5.0 + (k as f32 * degrees).to_radians();
                let game = orbit(yaw);
                let (next, eye) = frame(state, player(1), game, 1000 + 16 * k, rays(walls));
                let eye = eye.unwrap_or(game.way * game.distance);
                for (p, n) in walls {
                    assert!((eye - *p).dot(*n) > 0.02, "{yaw}: {eye:?} behind {p:?}");
                }
                nearer += u32::from(eye.length() < NEAREST - 1e-4);
                if let Some(last) = last {
                    assert!((eye - last).length() < 0.3, "{yaw}: {last:?} to {eye:?}");
                }
                (state, last) = (next, Some(eye));
            }
            assert!(nearer <= under * 2, "{degrees}: {nearer} frames nearer than {NEAREST}");
        }
    }

    #[test]
    fn a_drag_round_and_round_by_walls_and_in_corners() {
        sweep(&[upright(0.0, 0.5)], 0);
        sweep(&[upright(0.0, 0.3), upright(-PI / 2.0, 0.3)], 0);
        sweep(&[upright(0.0, 0.3), upright(-PI / 2.0, 0.35), upright(PI / 2.0, 0.35)], 0);
        let box_ = [upright(0.0, 0.4), upright(PI / 2.0, 0.4), upright(PI, 0.4), upright(-PI / 2.0, 0.4)];
        sweep(&box_, 0);
        // a ceiling 0.5 m over the pivot and a wall: to a side, nearer only while going to the other
        let ceiling = (Vec3::Y * 0.5, -Vec3::Y);
        sweep(&[ceiling, upright(0.0, 0.5)], 30);
        // walls all round and a ceiling: no room, but in front of every wall
        sweep(&[box_[0], box_[1], box_[2], box_[3], ceiling], u32::MAX / 2);
    }
}
