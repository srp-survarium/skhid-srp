//! Patch 158: the first-person eye stays out of the level's surfaces
//! (docs/notes/head-clearance.md).
//!
//! The view is given the eyes' transform (the player's +44Ch, bone "Item"):
//! 1.06 m over the feet crouched, 1.5 m standing, 0.13 m ahead of the
//! controller's axis. The crouched capsule is 1.1 m tall, so under anything
//! it crouches beneath the eye is a few centimetres from the surface (inside
//! the capsule's round top), and the near plane (0.05 m, its corners 0.07 m
//! out) reaches through it: the screen shows the surface's inside. Where an
//! object has shot collision but no walker collision (a log, a rock), the
//! capsule walks into it and the eye with it.
//!
//! At update_camera's eye (`camera.eye`), for a living player's own eye (a
//! ragdoll's, patch 76's, is left as it is): one ray from a point inside the
//! capsule, `BELOW_EYE` under the eye on the body's up, to the eye and
//! `CLEARANCE` past it, against the level's collision that bullets hit (the
//! drawn walls, the lobby camera's filter), going through a bush's leaves
//! (stock's vine bushes are shot collision a walker goes into: the Fort has
//! 147, Vector 319). A hit puts the eye `CLEARANCE`
//! short of it on that way, at once; when the way clears the eye goes back
//! at `OUT_RATE`. Only the view moves: the eyes' transform, the shots, the
//! hit models and the controller are the game's. Nothing a match computes.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, clamp, exp_neg};

use crate::vostok::camera;
use crate::vostok::step::GroundMaterial;
use crate::vostok::world::PlayerId;
use crate::vostok::{Context, Flow, GameCell, NoExit, clock};

/// The game materials of leaves (game.materials: foliage_coniferous,
/// foliage_deciduous, foliage_dry), which a walker goes through and the eye
/// with him.
const LEAVES: [u16; 3] = [24, 44, 49];
/// How far short of a surface the eye stays (m): the near plane's corners
/// (0.05 m ahead, 0.057 x 0.032 across at 65 degrees and 16:9) and a margin.
const CLEARANCE: f32 = 0.12;
/// How far under the eye the ray starts (m): inside the capsule, under its
/// top crouched (1.1 m, the eye at 1.06) as standing (1.8 m, at 1.5).
const BELOW_EYE: f32 = 0.4;
/// The share of the rest of the way back the eye goes in a second once the
/// way clears (1 - e^(-rate t) after t s: 95 % in 0.5 s).
const OUT_RATE: f32 = 6.0;
/// The longest frame the pull steps by (s).
const LONGEST_FRAME: f32 = 0.1;
/// A pull below this (m) with none wanted is none.
const NO_PULL: f32 = 0.001;

/// The eye's pull, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pull {
    Off,
    /// `player`'s eye, pulled `pull` m toward the ray's start, shown at `at`
    /// (the counter's milliseconds, wrapping).
    On { player: PlayerId, pull: f32, at: u32 },
}

static PULL: GameCell<Pull> = GameCell::new(Pull::Off);

/// update_camera's eye (`camera.eye`, after the ragdoll's).
pub fn on_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some((player, true)) = camera::eye_player(ctx) else {
        PULL.set(thread, Pull::Off);
        return Flow::Continue;
    };
    let body = player.transform();
    let eye = camera::given_eye(ctx);
    let Some(way) = Way::of(place(&body), Vec3::new(body[4], body[5], body[6]), place(&eye)) else {
        PULL.set(thread, Pull::Off);
        return Flow::Continue;
    };
    let hit = player.physics_world().and_then(|world| {
        let (from, along) = (way.from, way.along);
        world
            .shot_ray_test_past([from.x, from.y, from.z], [along.x, along.y, along.z], way.length + CLEARANCE, is_leaves)
            .map(|hit| (Vec3::new(hit.point[0], hit.point[1], hit.point[2]) - from).dot(along))
    });
    let next = step(PULL.get(thread), player.id(), wanted_pull(way.length, hit), clock::milliseconds(thread));
    PULL.set(thread, next);
    if let Pull::On { pull, .. } = next
        && pull > 0.0
    {
        let moved = way.from + way.along * (way.length - pull);
        let mut view = eye;
        view[12..15].copy_from_slice(&[moved.x, moved.y, moved.z]);
        camera::look_from(ctx, view);
    }
    Flow::Continue
}

fn is_leaves(material: GroundMaterial) -> bool {
    LEAVES.contains(&material.id())
}

/// The ray's way: from inside the capsule to the eye.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Way {
    from: Vec3,
    along: Vec3,
    length: f32,
}

impl Way {
    /// From `BELOW_EYE` under the eye on the body's `up` (at least half way
    /// up from the `feet`), over the body's axis, to the `eye`; none for a
    /// body with no up or an eye at its start.
    fn of(feet: Vec3, up: Vec3, eye: Vec3) -> Option<Way> {
        let up = up.direction()?;
        let height = (eye - feet).dot(up);
        if !height.is_finite() || height <= 0.0 {
            return None;
        }
        let from = feet + up * (height - BELOW_EYE).max(height * 0.5);
        let out = eye - from;
        let along = out.direction()?;
        Some(Way { from, along, length: out.length() })
    }
}

/// How far back toward the start the eye must go to stay `CLEARANCE` short
/// of the level's first hit `hit` m out on the way `length` long (none: 0;
/// at most the whole way). A hit that is no number is none.
fn wanted_pull(length: f32, hit: Option<f32>) -> f32 {
    match hit {
        Some(hit) if hit.is_finite() => clamp(length + CLEARANCE - hit, 0.0, length),
        _ => 0.0,
    }
}

/// The pull shown this frame: what is wanted when it is more (at once),
/// else eased back toward it by the time since the last frame.
fn step(state: Pull, player: PlayerId, wanted: f32, now: u32) -> Pull {
    let pull = match state {
        Pull::On { player: p, pull, at } if p == player && pull > wanted => {
            let dt = clamp(now.wrapping_sub(at) as f32 / 1000.0, 0.0, LONGEST_FRAME);
            let pull = wanted + (pull - wanted) * exp_neg(OUT_RATE * dt);
            if pull - wanted < NO_PULL { wanted } else { pull }
        }
        _ => wanted,
    };
    Pull::On { player, pull, at: now }
}

/// A row-vector 4x4's place (its row 3).
fn place(m: &[f32; 16]) -> Vec3 {
    Vec3::new(m[12], m[13], m[14])
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
    fn the_way_starts_inside_the_capsule_under_the_eye() {
        let feet = Vec3::new(10.0, 2.0, 5.0);
        let crouched = Way::of(feet, Vec3::new(0.0, 1.0, 0.0), Vec3::new(10.13, 3.06, 5.0)).unwrap();
        assert!((crouched.from.y - 2.66).abs() < 1e-5 && crouched.from.x == 10.0, "{crouched:?}");
        assert!((crouched.length - (0.4f32 * 0.4 + 0.13 * 0.13).sqrt()).abs() < 1e-5);
        let low = Way::of(feet, Vec3::new(0.0, 2.0, 0.0), Vec3::new(10.0, 2.5, 5.0)).unwrap();
        assert!((low.from.y - 2.25).abs() < 1e-5, "{low:?}");
        assert_eq!(Way::of(feet, Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 3.0, 5.0)), None);
        assert_eq!(Way::of(feet, Vec3::new(0.0, 1.0, 0.0), Vec3::new(10.0, 1.0, 5.0)), None);
    }

    #[test]
    fn the_pull_keeps_the_eye_short_of_the_hit() {
        assert_eq!(wanted_pull(0.4, None), 0.0);
        assert_eq!(wanted_pull(0.4, Some(0.6)), 0.0);
        assert!((wanted_pull(0.4, Some(0.45)) - 0.07).abs() < 1e-6);
        assert!((wanted_pull(0.4, Some(0.2)) - 0.32).abs() < 1e-6);
        assert_eq!(wanted_pull(0.4, Some(0.0)), 0.4);
        assert_eq!(wanted_pull(0.4, Some(f32::NAN)), 0.0);
    }

    #[test]
    fn the_eye_goes_in_at_once_and_back_smoothly() {
        let s = step(Pull::Off, player(1), 0.1, 1000);
        assert_eq!(s, Pull::On { player: player(1), pull: 0.1, at: 1000 });
        assert_eq!(pull(step(s, player(1), 0.3, 1016)), 0.3);
        let out = step(Pull::On { player: player(1), pull: 0.3, at: 0 }, player(1), 0.0, 100);
        assert!((pull(out) - 0.3 * exp_neg(0.6)).abs() < 1e-5, "{out:?}");
        let mut s = out;
        for t in 0..200 {
            s = step(s, player(1), 0.0, 116 + 16 * t);
        }
        assert_eq!(pull(s), 0.0);
        let other = Pull::On { player: player(2), pull: 0.3, at: 0 };
        assert_eq!(pull(step(other, player(1), 0.0, 16)), 0.0);
    }

    #[test]
    fn a_stall_moves_it_back_at_most_a_frame() {
        let s = Pull::On { player: player(1), pull: 0.3, at: 1000 };
        let stalled = pull(step(s, player(1), 0.0, 61_000));
        assert!((stalled - 0.3 * exp_neg(OUT_RATE * LONGEST_FRAME)).abs() < 1e-5);
        assert_eq!(pull(step(s, player(1), 0.0, 1000)), 0.3);
    }
}
