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
//! The ray's hit gives the pull the eye wants: how far in from the orbit it
//! must be to stay `MARGIN` short of the hit (the near plane is 0.05 m). The
//! pull shown goes to it smoothly, never at once (the level's meshes are
//! detailed, and a ray that meets a bar or a branch near the player would
//! otherwise throw the eye in and out): in at `IN_RATE`, out at the slower
//! `OUT_RATE`, a share of what is left each second. The pull is taken from
//! the distance the wheel keeps (+354h, not changed), so the wheel moves the
//! eye at once as before. A new orbit (the warmup's first frame, another
//! player) starts at the pull it wants.
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
/// wall (in) and back out: 1 - e^(-rate t) of it after t s (in: 95 % in
/// 0.37 s; out: in 1 s).
const IN_RATE: f32 = 8.0;
const OUT_RATE: f32 = 3.0;
/// The longest frame the pull steps by (s): a stall moves it no further.
const LONGEST_FRAME: f32 = 0.1;
/// A pull below this (m) with none wanted is none: the game's view as it is.
const NO_PULL: f32 = 0.001;

/// The pull of the camera's orbit, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pull {
    /// No orbit last frame.
    Off,
    /// `player`'s orbit, the eye `pull` m in from its distance, shown at
    /// `at` (the counter's milliseconds, wrapping).
    On { player: PlayerId, pull: f32, at: u32 },
}

static PULL: GameCell<Pull> = GameCell::new(Pull::Off);

/// update_camera after the view's update (`camera.orbited`).
pub fn on_orbited(ctx: &mut Context<'_>) -> Flow<NoExit> {
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
    let hit = orbit.player().physics_world().and_then(|world| {
        world
            .ray_test([pivot.x, pivot.y, pivot.z], [direction.x, direction.y, direction.z], distance + MARGIN, GROUP, MASK)
            .map(|hit| Vec3::new(hit.point[0], hit.point[1], hit.point[2]))
    });
    let wanted = wanted_pull(distance, hit.map(|hit| (hit - pivot).dot(direction)));
    let next = step(PULL.get(thread), orbit.player().id(), wanted, now(thread));
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
/// short of the level's first hit `hit` m out on the way (none: 0; at most
/// the whole distance). A hit that is no number is none.
fn wanted_pull(distance: f32, hit: Option<f32>) -> f32 {
    match hit {
        Some(hit) if hit.is_finite() => clamp(distance - (hit - MARGIN), 0.0, distance),
        _ => 0.0,
    }
}

/// The pull shown this frame: a new orbit's at once, else moved toward
/// `wanted` by the time since the last frame.
fn step(state: Pull, player: PlayerId, wanted: f32, now: u32) -> Pull {
    let pull = match state {
        Pull::On { player: p, pull, at } if p == player => {
            let dt = clamp(now.wrapping_sub(at) as f32 / 1000.0, 0.0, LONGEST_FRAME);
            let rate = if wanted > pull { IN_RATE } else { OUT_RATE };
            let pull = wanted + (pull - wanted) * exp_neg(rate * dt);
            if wanted == 0.0 && pull < NO_PULL { 0.0 } else { pull }
        }
        _ => wanted,
    };
    Pull::On { player, pull, at: now }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(n: u32) -> PlayerId {
        PlayerId::for_test(n)
    }

    fn pull(state: Pull) -> f32 {
        match state {
            Pull::On { pull, .. } => pull,
            Pull::Off => panic!("off"),
        }
    }

    #[test]
    fn the_pull_keeps_the_eye_short_of_the_hit() {
        assert_eq!(wanted_pull(3.0, None), 0.0);
        assert_eq!(wanted_pull(3.0, Some(3.2)), 0.0);
        assert!((wanted_pull(3.0, Some(1.5)) - 1.7).abs() < 1e-6);
        assert!((wanted_pull(3.0, Some(3.05)) - 0.15).abs() < 1e-6);
        assert_eq!(wanted_pull(3.0, Some(0.1)), 3.0);
        assert_eq!(wanted_pull(3.0, Some(f32::NAN)), 0.0);
    }

    #[test]
    fn a_new_orbit_starts_at_the_pull_it_wants() {
        assert_eq!(step(Pull::Off, player(1), 1.7, 500), Pull::On { player: player(1), pull: 1.7, at: 500 });
        let other = Pull::On { player: player(2), pull: 0.0, at: 400 };
        assert_eq!(pull(step(other, player(1), 1.7, 500)), 1.7);
    }

    #[test]
    fn the_pull_goes_in_and_out_smoothly() {
        let start = Pull::On { player: player(1), pull: 0.0, at: 1000 };
        // a wall comes: in by 1 - e^(-0.8) of the way in 100 ms, never at once
        let s = step(start, player(1), 2.0, 1100);
        assert!((pull(s) - 2.0 * (1.0 - exp_neg(0.8))).abs() < 1e-4, "{s:?}");
        let mut s = s;
        for t in 0..5 {
            s = step(s, player(1), 2.0, 1116 + 16 * t);
        }
        assert!(pull(s) > 1.0 && pull(s) < 2.0);
        // it goes: out more slowly, down to none
        let out = step(Pull::On { player: player(1), pull: 2.0, at: 0 }, player(1), 0.0, 100);
        assert!((pull(out) - 2.0 * exp_neg(0.3)).abs() < 1e-4, "{out:?}");
        let mut s = out;
        for t in 0..400 {
            s = step(s, player(1), 0.0, 116 + 16 * t);
        }
        assert_eq!(pull(s), 0.0);
    }

    #[test]
    fn a_stall_or_a_clock_going_back_moves_it_at_most_a_frame() {
        let s = Pull::On { player: player(1), pull: 0.0, at: 1000 };
        let stalled = pull(step(s, player(1), 2.0, 61_000));
        assert!((stalled - 2.0 * (1.0 - exp_neg(IN_RATE * LONGEST_FRAME))).abs() < 1e-4);
        // the counter wraps: a step "back" is a long frame
        assert!(pull(step(s, player(1), 2.0, 999)) <= stalled + 1e-6);
        // the same millisecond: no move
        assert_eq!(pull(step(s, player(1), 2.0, 1000)), 0.0);
    }
}
