//! Patch 132: the orbit camera stays on the player's side of the level.
//!
//! Before the start (the input handler's warmup mode, 3) the camera orbits
//! the player at 1.5 to 4.6 m (patch 31's limits): update_inverted_view
//! (0x2422B0) puts the eye out along the view's back and never tests the
//! level, so at many spawns it sits inside a wall or on the far side of a
//! thin one, and the player sees the inside of the wall or the room behind
//! it. The lobby's camera cuts its distance at the level's first hit
//! (lobby_camera::process_collision 0x2844D0: ray_test from the point it
//! looks at, group 10h, mask 8); this does the same for the orbit, after the
//! view's update and before the director gives the view on: the eye is put
//! `MARGIN` short of the first hit of five parallel rays (the middle one and
//! one `RADIUS` off it to each side, up and down, so that the near plane,
//! 0.05 m, does not graze a wall the middle ray passes by). The distance the
//! wheel keeps (+354h) is not changed: the eye goes back out as soon as the
//! wall is out of the way.
//!
//! The camera alone: nothing a match computes.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, max, min};

use crate::vostok::camera::Orbit;
use crate::vostok::{Context, Flow, NoExit};

/// The lobby camera's ray (process_collision 0x2844E4..0x2844EA): the
/// level's collision that bullets hit (cgroup 8 / cmask 16), which follows
/// the drawn walls; not the players, their hit models or the grenades.
const GROUP: u32 = 0x10;
const MASK: u32 = 0x08;
/// How far in front of a hit the eye stays (m), beyond the near plane.
const MARGIN: f32 = 0.2;
/// How far the side rays are off the middle one (m).
const RADIUS: f32 = 0.1;

/// update_camera after the view's update (`camera.orbited`).
pub fn on_orbited(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut orbit) = Orbit::at(ctx) else { return Flow::Continue };
    let Some(world) = orbit.player().physics_world() else { return Flow::Continue };
    let (right, up) = orbit.axes();
    let ray = |from: Vec3, direction: Vec3, length: f32| {
        world
            .ray_test([from.x, from.y, from.z], [direction.x, direction.y, direction.z], length, GROUP, MASK)
            .map(|hit| Vec3::new(hit.point[0], hit.point[1], hit.point[2]))
    };
    if let Some(eye) = clear_eye(orbit.pivot(), orbit.eye(), right, up, ray) {
        orbit.set_eye(eye);
    }
    Flow::Continue
}

/// The eye moved toward the pivot to `MARGIN` short of the nearest hit of
/// the rays from the pivot (and `RADIUS` off it along `right` and `up`)
/// toward the eye, each `MARGIN` longer than the orbit; None when nothing is
/// in the way (the eye as it is). `ray(from, direction, length)` is the
/// level's first hit point.
fn clear_eye(
    pivot: Vec3,
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    mut ray: impl FnMut(Vec3, Vec3, f32) -> Option<Vec3>,
) -> Option<Vec3> {
    let out = eye - pivot;
    let distance = out.length();
    let direction = out.direction()?;
    let offsets = [Vec3::ZERO, right * RADIUS, right * -RADIUS, up * RADIUS, up * -RADIUS];
    let free = offsets.iter().fold(distance, |free, &offset| {
        let from = pivot + offset;
        match ray(from, direction, distance + MARGIN) {
            Some(hit) if hit.is_finite() => min(free, (hit - from).dot(direction) - MARGIN),
            _ => free,
        }
    });
    if free >= distance {
        return None;
    }
    Some(pivot + direction * max(free, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIVOT: Vec3 = Vec3::new(10.0, 1.4, -3.0);
    const RIGHT: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    /// 3 m behind the pivot (+z the view's back here).
    const EYE: Vec3 = Vec3::new(10.0, 1.4, 0.0);

    /// A wall across z = `at` (a plane facing the pivot), as ray_test meets it.
    fn wall(at: f32) -> impl FnMut(Vec3, Vec3, f32) -> Option<Vec3> {
        move |from: Vec3, direction: Vec3, length: f32| {
            let t = (at - from.z) / direction.z;
            (t >= 0.0 && t <= length).then(|| from + direction * t)
        }
    }

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn nothing_in_the_way_leaves_the_eye() {
        assert_eq!(clear_eye(PIVOT, EYE, RIGHT, UP, |_, _, _| None), None);
        // a wall past the eye and its margin
        assert_eq!(clear_eye(PIVOT, EYE, RIGHT, UP, wall(0.25)), None);
    }

    #[test]
    fn a_wall_between_puts_the_eye_in_front_of_it() {
        let eye = clear_eye(PIVOT, EYE, RIGHT, UP, wall(-1.0)).unwrap();
        assert!(near(eye, Vec3::new(10.0, 1.4, -1.2)), "{eye:?}");
    }

    #[test]
    fn a_wall_just_behind_the_eye_still_moves_it_by_the_margin() {
        // the eye 5 cm in front of the wall: the near plane would cut it
        let eye = clear_eye(PIVOT, EYE, RIGHT, UP, wall(0.05)).unwrap();
        assert!(near(eye, Vec3::new(10.0, 1.4, -0.15)), "{eye:?}");
    }

    #[test]
    fn a_wall_at_the_pivot_brings_the_eye_to_it() {
        let eye = clear_eye(PIVOT, EYE, RIGHT, UP, wall(-2.9)).unwrap();
        assert!(near(eye, PIVOT), "{eye:?}");
    }

    #[test]
    fn a_wall_beside_the_middle_ray_is_met_by_a_side_one() {
        // a post 0.05..0.2 m to the right of the middle ray, at z = -1
        let post = |from: Vec3, direction: Vec3, length: f32| {
            let t = (-1.0 - from.z) / direction.z;
            let hit = from + direction * t;
            (t <= length && hit.x > 10.05 && hit.x < 10.2).then_some(hit)
        };
        let eye = clear_eye(PIVOT, EYE, RIGHT, UP, post).unwrap();
        assert!(near(eye, Vec3::new(10.0, 1.4, -1.2)), "{eye:?}");
    }

    #[test]
    fn the_rays_go_from_the_pivot_toward_the_eye() {
        let mut rays = std::vec::Vec::new();
        clear_eye(PIVOT, EYE, RIGHT, UP, |from, direction, length| {
            rays.push((from, direction, length));
            None
        });
        assert_eq!(rays.len(), 5);
        assert!(near(rays[0].0, PIVOT));
        for (from, direction, length) in rays {
            assert!(near(direction, Vec3::new(0.0, 0.0, 1.0)) && (length - 3.2).abs() < 1e-5);
            assert!((from - PIVOT).length() < RADIUS + 1e-5);
        }
    }

    #[test]
    fn an_eye_at_the_pivot_or_a_nan_hit_changes_nothing() {
        assert_eq!(clear_eye(PIVOT, PIVOT, RIGHT, UP, wall(-1.0)), None);
        let nan = Vec3::new(f32::NAN, f32::NAN, f32::NAN);
        assert_eq!(clear_eye(PIVOT, EYE, RIGHT, UP, |_, _, _| Some(nan)), None);
    }
}
