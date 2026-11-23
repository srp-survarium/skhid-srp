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
//! The camera alone: nothing a match computes.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, clamp, exp_neg};

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
    /// milliseconds, wrapping).
    On { player: PlayerId, way: Vec3, pull: f32, at: u32 },
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
    let distance = out.length();
    let Some(direction) = out.direction() else {
        PULL.set(thread, Pull::Off);
        return Flow::Continue;
    };
    let player = orbit.player().id();
    let world = orbit.player().physics_world();
    // how far out on `way` the level is first met, within the orbit and its margin
    let met = |way: Vec3| {
        let hit = world.as_ref()?.ray_test([pivot.x, pivot.y, pivot.z], [way.x, way.y, way.z], distance + MARGIN, GROUP, MASK)?;
        Some((Vec3::new(hit.point[0], hit.point[1], hit.point[2]) - pivot).dot(way))
    };
    let (state, now) = (PULL.get(thread), now(thread));
    let needed = needed_pull(distance, met(direction));
    let ahead = way_ahead(state, player, direction, now).map_or(0.0, |way| needed_pull(distance, met(way)));
    let next = step(state, player, direction, needed, ahead, now);
    PULL.set(thread, next);
    if let Pull::On { pull, .. } = next
        && pull > 0.0
    {
        orbit.set_eye(pivot + direction * clamp(distance - pull, 0.0, distance));
    }
    Flow::Continue
}

fn now(thread: &Thread) -> u32 {
    clock::milliseconds(thread)
}

/// How far in from the orbit's `distance` the eye must be to stay `MARGIN`
/// short of the level's first hit `hit` m out on a way (none: 0; at most
/// the whole distance). A hit that is no number is none.
fn needed_pull(distance: f32, hit: Option<f32>) -> f32 {
    match hit {
        Some(hit) if hit.is_finite() => clamp(distance - (hit - MARGIN), 0.0, distance),
        _ => 0.0,
    }
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

/// The pull shown this frame: moved toward the larger of `needed` (by the
/// wall on the way) and `ahead` (by the wall the turn is coming to) by the
/// time since the last frame, and never less than `needed`; a new orbit's
/// is `needed`.
fn step(state: Pull, player: PlayerId, way: Vec3, needed: f32, ahead: f32, now: u32) -> Pull {
    let pull = match state {
        Pull::On { player: p, pull, at, .. } if p == player => {
            let wanted = needed.max(ahead);
            let dt = clamp(now.wrapping_sub(at) as f32 / 1000.0, 0.0, LONGEST_FRAME);
            let rate = if wanted > pull { IN_RATE } else { OUT_RATE };
            let pull = wanted + (pull - wanted) * exp_neg(rate * dt);
            if wanted == 0.0 && pull < NO_PULL { 0.0 } else { pull.max(needed) }
        }
        _ => needed,
    };
    Pull::On { player, way, pull, at: now }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAY: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    fn player(n: u32) -> PlayerId {
        PlayerId::for_test(n)
    }

    fn on(n: u32, pull: f32, at: u32) -> Pull {
        Pull::On { player: player(n), way: WAY, pull, at }
    }

    fn pull(state: Pull) -> f32 {
        match state {
            Pull::On { pull, .. } => pull,
            Pull::Off => panic!("off"),
        }
    }

    #[test]
    fn the_pull_keeps_the_eye_short_of_the_hit() {
        assert_eq!(needed_pull(3.0, None), 0.0);
        assert_eq!(needed_pull(3.0, Some(3.2)), 0.0);
        assert!((needed_pull(3.0, Some(1.5)) - 1.7).abs() < 1e-6);
        assert!((needed_pull(3.0, Some(3.05)) - 0.15).abs() < 1e-6);
        assert_eq!(needed_pull(3.0, Some(0.1)), 3.0);
        assert_eq!(needed_pull(3.0, Some(f32::NAN)), 0.0);
    }

    #[test]
    fn a_new_orbit_starts_at_the_pull_it_needs() {
        assert_eq!(step(Pull::Off, player(1), WAY, 1.7, 0.0, 500), on(1, 1.7, 500));
        assert_eq!(pull(step(on(2, 0.0, 400), player(1), WAY, 1.7, 0.0, 500)), 1.7);
    }

    #[test]
    fn the_wall_on_the_way_pulls_at_once_and_lets_out_smoothly() {
        assert_eq!(pull(step(on(1, 0.0, 1000), player(1), WAY, 2.0, 0.0, 1016)), 2.0);
        let out = step(on(1, 2.0, 0), player(1), WAY, 0.0, 0.0, 100);
        assert!((pull(out) - 2.0 * exp_neg(0.3)).abs() < 1e-4, "{out:?}");
        let mut s = out;
        for t in 0..400 {
            s = step(s, player(1), WAY, 0.0, 0.0, 116 + 16 * t);
        }
        assert_eq!(pull(s), 0.0);
    }

    #[test]
    fn the_wall_ahead_pulls_smoothly_and_never_under_the_one_on_the_way() {
        // only ahead: in by 1 - e^(-1) of the way in 100 ms
        let s = step(on(1, 0.0, 1000), player(1), WAY, 0.0, 2.0, 1100);
        assert!((pull(s) - 2.0 * (1.0 - exp_neg(1.0))).abs() < 1e-4, "{s:?}");
        // the wall arrives on the way before the pull is there: the rest at once
        assert_eq!(pull(step(s, player(1), WAY, 2.0, 2.0, 1116)), 2.0);
        // a stall moves it a frame's worth
        let stalled = pull(step(on(1, 0.0, 1000), player(1), WAY, 0.0, 2.0, 61_000));
        assert!((stalled - pull(s)).abs() < 1e-6);
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
}
