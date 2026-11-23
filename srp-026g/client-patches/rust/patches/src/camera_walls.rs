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
//! The eye is never nearer the pivot than `NEAREST`, out of the body: 0.69d's
//! third-person camera (third_person_camera_controller::tick 0x2002AE3B0)
//! places its eye only where the room is more than that and otherwise keeps
//! the camera where it was. So where the way has less room (a player tucked
//! in a corner), the view shown last is held, tested again; and where that
//! has none either (an orbit that starts in the corner), the game's view is
//! raised toward looking straight down, the eye over the head, by the least
//! step that has the room.
//!
//! The camera alone: nothing a match computes.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, clamp, exp_neg, max, min};

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
/// The steps of the raise toward looking straight down (the last).
const RAISES: u16 = 6;
/// The share of the rest of the way the pull goes in a second, toward a
/// wall the turn is coming to (in) and back out: 1 - e^(-rate t) of it
/// after t s (in: 95 % in 0.3 s; out: in 1 s).
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
/// A pull below this (m) with none wanted is none: the game's view as it is.
const NO_PULL: f32 = 0.001;

/// The pull of the camera's orbit, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pull {
    /// No orbit last frame.
    Off,
    /// `player`'s orbit, its eye along the unit `way` from the pivot and
    /// `pull` m in from its distance, shown at `at` (the counter's
    /// milliseconds, wrapping) as `shown`.
    On { player: PlayerId, way: Vec3, pull: f32, at: u32, shown: Pose },
}

/// A view of the orbit, apart from its pivot: its turn (the rows right, up
/// and forward) and where the eye is from the pivot.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    turn: [Vec3; 3],
    out: Vec3,
}

impl Pose {
    /// The pose with its eye `room` m out on its way, when that is nearer.
    fn within(self, room: f32) -> Pose {
        let length = self.out.length();
        if room >= length {
            return self;
        }
        Pose { out: self.out * (max(room, 0.0) / length), ..self }
    }

    /// The pose turned about its right row, `t` of the way (0 none, 1 all)
    /// toward looking straight down: the eye over the pivot, as far out.
    fn raised(self, t: f32) -> Option<Pose> {
        let [right, up, forward] = self.turn;
        let turned = (forward * (1.0 - t) - Vec3::Y * t).reject(right).direction()?;
        let hand = if forward.cross(right).dot(up) < 0.0 { -1.0 } else { 1.0 };
        let (back, side) = (-self.out.dot(forward), self.out.dot(right));
        Some(Pose { turn: [right, turned.cross(right) * hand, turned], out: right * side - turned * back })
    }
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
    let game = Pose { turn: orbit.turn(), out: orbit.eye() - pivot };
    let distance = game.out.length();
    let Some(direction) = game.out.direction() else {
        PULL.set(thread, Pull::Off);
        return Flow::Continue;
    };
    let player = orbit.player().id();
    let world = orbit.player().physics_world();
    // how far out on the unit `way` the eye can be, at most `length`
    let room_on = |way: Vec3, length: f32| {
        let hit = world.as_ref().and_then(|world| {
            world.ray_test([pivot.x, pivot.y, pivot.z], [way.x, way.y, way.z], length + MARGIN, GROUP, MASK)
        });
        room(length, hit.map(|hit| (Vec3::new(hit.point[0], hit.point[1], hit.point[2]) - pivot).dot(way)))
    };
    let (state, now) = (PULL.get(thread), now(thread));
    let free = room_on(direction, distance);
    let ahead = way_ahead(state, player, direction, now).map_or(0.0, |way| distance - room_on(way, distance));
    let pull = step(state, player, distance - free, ahead, now);
    let nearest = min(NEAREST, distance);
    let shown = if free >= nearest {
        game.within(max(distance - pull, nearest))
    } else {
        let held = match state {
            Pull::On { player: p, shown, .. } if p == player => Some(shown),
            _ => None,
        };
        clear_pose(held, game, free, nearest, |pose| pose.out.direction().map_or(0.0, |way| room_on(way, pose.out.length())))
    };
    PULL.set(thread, Pull::On { player, way: direction, pull, at: now, shown });
    if shown != game {
        orbit.look(shown.turn, pivot + shown.out);
    }
    Flow::Continue
}

fn now(thread: &Thread) -> u32 {
    clock::milliseconds(thread)
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

/// The pose shown when the game's (`free` m of room) has less than
/// `nearest`: the first of `held` (the one shown last frame, as 0.69d holds
/// its camera) and the game's raised step by step whose `room` is that
/// much, as far out as it has; with none, the one with the most room.
fn clear_pose(held: Option<Pose>, game: Pose, free: f32, nearest: f32, mut room: impl FnMut(Pose) -> f32) -> Pose {
    let raised = (1..=RAISES).filter_map(|k| game.raised(f32::from(k) / f32::from(RAISES)));
    let mut best = (free, game);
    for pose in held.into_iter().chain(raised) {
        let r = room(pose);
        if r >= nearest {
            return pose.within(r);
        }
        if r > best.0 {
            best = (r, pose);
        }
    }
    best.1.within(best.0)
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
            let dt = clamp(now.wrapping_sub(at) as f32 / 1000.0, 0.0, LONGEST_FRAME);
            let rate = if wanted > pull { IN_RATE } else { OUT_RATE };
            let pull = wanted + (pull - wanted) * exp_neg(rate * dt);
            if wanted == 0.0 && pull < NO_PULL { 0.0 } else { pull.max(needed) }
        }
        _ => needed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAY: Vec3 = Vec3::new(0.0, 0.0, 1.0);
    /// Looking along -z, level: the eye 3 m behind (+z) and 0.6 m right.
    const GAME: Pose = Pose { turn: [Vec3::X, Vec3::Y, Vec3::new(0.0, 0.0, -1.0)], out: Vec3::new(0.6, 0.0, 3.0) };

    fn player(n: u32) -> PlayerId {
        PlayerId::for_test(n)
    }

    fn on(n: u32, pull: f32, at: u32) -> Pull {
        Pull::On { player: player(n), way: WAY, pull, at, shown: GAME }
    }

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
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
    fn a_raise_turns_the_view_about_its_right_toward_looking_down() {
        let length = GAME.out.length();
        let mut last = GAME.out.y;
        for k in 1..=RAISES {
            let pose = GAME.raised(f32::from(k) / f32::from(RAISES)).unwrap();
            let [right, up, forward] = pose.turn;
            assert_eq!(right, Vec3::X);
            assert!((up.length() - 1.0).abs() < 1e-5 && (forward.length() - 1.0).abs() < 1e-5);
            // the game's hand: forward x right is down
            assert!(up.dot(forward).abs() < 1e-5 && near(forward.cross(right), -up), "{pose:?}");
            // as far out, as far right, as far back of the view; higher each step
            assert!((pose.out.length() - length).abs() < 1e-4);
            assert!((pose.out.dot(right) - 0.6).abs() < 1e-5 && (pose.out.dot(forward) + 3.0).abs() < 1e-4);
            assert!(pose.out.y > last);
            last = pose.out.y;
        }
        let top = GAME.raised(1.0).unwrap();
        assert!(near(top.turn[2], -Vec3::Y) && near(top.out, Vec3::new(0.6, 3.0, 0.0)), "{top:?}");
    }

    #[test]
    fn with_no_room_on_the_way_the_view_held_or_the_least_raise_with_room() {
        let raised = |k: u16| GAME.raised(f32::from(k) / f32::from(RAISES)).unwrap();
        let held = Pose { turn: GAME.turn, out: Vec3::new(-0.3, 0.0, 1.5) };
        // the view held, where it has the room
        let mut rays = 0;
        assert_eq!(clear_pose(Some(held), GAME, 0.3, NEAREST, |p| { rays += 1; p.out.length() }), held);
        assert_eq!(rays, 1);
        // the held one has none (or none is held): the first raise with the room, as far out as it has
        let room = |p: Pose| if p.out.y > raised(3).out.y - 1e-4 { 1.2 } else { 0.4 };
        let shown = clear_pose(Some(held), GAME, 0.3, NEAREST, room);
        assert_eq!(shown.turn, raised(3).turn);
        assert!((shown.out.length() - 1.2).abs() < 1e-5 && near(shown.out.direction().unwrap(), raised(3).out.direction().unwrap()));
        assert_eq!(clear_pose(None, GAME, 0.3, NEAREST, room), shown);
        // nowhere has it: the one with the most room, at it
        let shown = clear_pose(None, GAME, 0.3, NEAREST, |p| if p == raised(2) { 0.6 } else { 0.1 });
        assert_eq!(shown.turn, raised(2).turn);
        assert!((shown.out.length() - 0.6).abs() < 1e-5);
        assert_eq!(clear_pose(None, GAME, 0.3, NEAREST, |_| 0.1), GAME.within(0.3));
    }
}
