//! The death camera on a ragdoll's head (patch 46 turns the view about the
//! eye transform player+44Ch, the "Item" bone, which rides the head).
//!
//! A tumbling head would shake and roll the view. The camera's forward
//! follows the eye's by a critically damped spring, its up kept level (no
//! roll), its pitch within +-55 degrees.
//!
//! Its place is the eye's itself, never trailing it: the game draws the
//! local player's body without its head (set_head_visibility, dead too), so
//! a camera behind or above the head shows the body headless. A ray from
//! the skull to the eye keeps it out of the ground and walls: a surface
//! between them pulls the camera back toward the skull, and the camera keeps
//! its margin from that surface and from the ground the head lies on, but
//! never further from them than the skull is (it stays in the hidden head).
//!
//! The eye's frame is a view's (vostok's rows i, j, k: right, up, forward):
//! local +Y is up, +Z forward.

use crate::math::{Pose, Quat, Vec3, clamp, exp_neg, max, min, sqrt};
use crate::ragdoll::{Level, Sweep};

/// How fast the view turns after the eye (rad/s of the critically damped
/// spring): about 0.3 s to settle on a still head.
const TURN_RATE: f32 = 9.0;
/// The view's pitch stays within this (sin of 55 degrees).
const MAX_PITCH_SIN: f32 = 0.819;
/// The camera's margin from a surface, m: its near plane stays out of it.
const MARGIN: f32 = 0.08;

/// Where the camera may be this frame: the eye on the drawn head, the
/// skull (the head capsule's far end, inside the hidden head), and the
/// plane the head lies on (a point and its unit normal), if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadView {
    pub eye: Pose,
    pub skull: Vec3,
    pub ground: Option<(Vec3, Vec3)>,
}

/// What the last ray from the skull to the eye found (kept while the
/// frame's rays are spent or the body has settled).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ray {
    /// Nothing: the camera is at the eye.
    Clear,
    /// A surface: the camera `reach` of the way (0..1) from the skull to
    /// the eye, off the surface's plane (a point, a unit normal).
    Blocked { reach: f32, wall: (Vec3, Vec3) },
}

/// The follow's state: where the camera is, the view's forward and how fast
/// it turns, and the last skull ray's answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeFollow {
    position: Vec3,
    forward: Vec3,
    turning: Vec3,
    ray: Ray,
}

/// A camera move shorter than this (m) asks no ray.
const STILL: f32 = 0.001;

/// One straight move of the camera: done, or stopped at a surface.
enum Leg {
    Reached,
    Stopped { at: Vec3, normal: Vec3 },
}

/// The camera moved from `from` to `to`: stopped `MARGIN` short of the
/// first surface on the way; `None` when the level was not asked.
fn leg(level: &mut impl Level, from: Vec3, to: Vec3) -> Option<Leg> {
    let along = to - from;
    let Some(direction) = along.direction().filter(|_| along.length() >= STILL) else {
        return Some(Leg::Reached);
    };
    // MARGIN off the surface, not along the move: an oblique move stops
    // further back, so the ray reaches past the move by MARGIN at the
    // steepest approach counted (a grazing one keeps less)
    const LEAST_APPROACH: f32 = 1.0 / 3.0;
    match level.sweep(from, to, MARGIN / LEAST_APPROACH) {
        Sweep::Clear => Some(Leg::Reached),
        Sweep::Hit { point, normal } => {
            let normal = normal.direction().unwrap_or(-direction);
            let approach = max(-direction.dot(normal), LEAST_APPROACH);
            let stop = (point - from).dot(direction) - MARGIN / approach;
            if stop >= along.length() {
                return Some(Leg::Reached);
            }
            Some(Leg::Stopped {
                at: from + direction * max(stop, 0.0),
                normal,
            })
        }
        Sweep::Deferred => None,
    }
}

/// Where the camera gets from `from` toward `to` without crossing a
/// surface: the ragdoll's head can sink into a wall or the ground for a
/// while (a capsule end holds one plane, and a depth found late leaves
/// slowly), the camera never does. Stopped short of a surface, the rest of
/// the move slides along it; with the level not asked it stays.
fn travel(level: &mut impl Level, from: Vec3, to: Vec3) -> Vec3 {
    match leg(level, from, to) {
        None => from,
        Some(Leg::Reached) => to,
        Some(Leg::Stopped { at, normal }) => {
            let slide = at + (to - at).reject(normal);
            match leg(level, at, slide) {
                Some(Leg::Reached) => slide,
                Some(Leg::Stopped { at, .. }) => at,
                None => at,
            }
        }
    }
}

/// One critically damped spring step of `x` (speed `v`) toward `target`.
fn spring(x: Vec3, v: Vec3, target: Vec3, omega: f32, dt: f32) -> (Vec3, Vec3) {
    let d = x - target;
    let temp = (v + d * omega) * dt;
    let e = exp_neg(omega * dt);
    (target + (d + temp) * e, (v - temp * omega) * e)
}

/// `position` moved off the plane at `point` (unit `normal`) to the margin,
/// but no further from it than the skull is.
fn clear_of(position: Vec3, skull: Vec3, (point, normal): (Vec3, Vec3)) -> Vec3 {
    let height = (position - point).dot(normal);
    let wanted = min(MARGIN, max((skull - point).dot(normal), height));
    if height < wanted {
        position + normal * (wanted - height)
    } else {
        position
    }
}

/// A forward, its pitch limited, made unit.
fn level_forward(forward: Vec3, up: Vec3) -> Vec3 {
    let pitch = clamp(forward.dot(up), -MAX_PITCH_SIN, MAX_PITCH_SIN);
    let flat = forward.reject(up).direction_or(up.any_perpendicular());
    flat * sqrt(1.0 - pitch * pitch) + up * pitch
}

impl EyeFollow {
    /// The follow starting on the eye at `eye`.
    pub fn new(eye: Pose, up: Vec3) -> EyeFollow {
        EyeFollow {
            position: eye.position,
            forward: level_forward(eye.rotation.rotate(Vec3::Z), up),
            turning: Vec3::ZERO,
            ray: Ray::Clear,
        }
    }

    /// One frame: the camera's pose (a view's frame) on `head` after `dt` s
    /// of following; one ray of `level` asked.
    pub fn follow(&mut self, head: HeadView, up: Vec3, level: &mut impl Level, dt: f32) -> Pose {
        let dt = clamp(dt, 0.0, 0.1);
        let HeadView { eye, skull, ground } = head;
        let along = eye.position - skull;
        self.ray = match along.direction() {
            None => Ray::Clear,
            Some(direction) => match level.sweep(skull, eye.position, MARGIN) {
                Sweep::Clear => Ray::Clear,
                Sweep::Hit { point, normal } => match normal.direction() {
                    Some(normal) => Ray::Blocked {
                        reach: clamp(
                            ((point - skull).dot(direction) - MARGIN) / along.length(),
                            0.0,
                            1.0,
                        ),
                        wall: (point, normal),
                    },
                    None => self.ray,
                },
                Sweep::Deferred => self.ray,
            },
        };
        let (reach, wall) = match self.ray {
            Ray::Clear => (1.0, None),
            Ray::Blocked { reach, wall } => (reach, Some(wall)),
        };
        let mut position = skull + along * reach;
        for plane in [wall, ground].into_iter().flatten() {
            position = clear_of(position, skull, plane);
        }
        if !position.is_finite() {
            position = eye.position;
        }
        let position = if self.position.is_finite() {
            travel(level, self.position, position)
        } else {
            position
        };
        self.position = position;
        let wanted = level_forward(eye.rotation.rotate(Vec3::Z), up);
        let (forward, turning) = spring(self.forward, self.turning, wanted, TURN_RATE, dt);
        self.forward = level_forward(forward.direction_or(wanted), up);
        self.turning = turning;
        // a view's frame: Y up (level: no roll), Z forward, X = Y x Z
        let z = self.forward;
        let y = up.reject(z).direction_or(up);
        let x = y.cross(z);
        Pose::new(Quat::from_basis(x, y, z), position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Planes (a point, a unit normal) a ray stops at, facing it.
    struct Planes(&'static [([f32; 3], [f32; 3])]);

    impl Level for Planes {
        fn sweep(&mut self, from: Vec3, to: Vec3, radius: f32) -> Sweep {
            let Some(d) = (to - from).direction() else {
                return Sweep::Clear;
            };
            let reach = (to - from).length() + radius;
            self.0
                .iter()
                .map(|&(p, n)| (Vec3::new(p[0], p[1], p[2]), Vec3::new(n[0], n[1], n[2])))
                .filter_map(|(p, n)| {
                    let facing = d.dot(n);
                    let t = (p - from).dot(n) / facing;
                    (facing < 0.0 && (0.0..=reach).contains(&t)).then_some((t, n))
                })
                .reduce(|a, b| if a.0 <= b.0 { a } else { b })
                .map_or(Sweep::Clear, |(t, normal)| Sweep::Hit {
                    point: from + d * t,
                    normal,
                })
        }
    }

    const FLOOR: ([f32; 3], [f32; 3]) = ([0.0; 3], [0.0, 1.0, 0.0]);
    const WALL: ([f32; 3], [f32; 3]) = ([0.15, 0.0, 0.0], [-1.0, 0.0, 0.0]);

    fn view(eye: Vec3, look: Quat, skull: Vec3) -> HeadView {
        HeadView {
            eye: Pose::new(look, eye),
            skull,
            ground: Some((Vec3::ZERO, Vec3::Y)),
        }
    }

    #[test]
    fn the_camera_is_on_the_eye_of_a_falling_head_never_behind_it() {
        let up = Vec3::Y;
        let mut level = Planes(&[FLOOR]);
        let start = view(
            Vec3::new(0.1, 1.6, 0.0),
            Quat::IDENTITY,
            Vec3::new(0.0, 1.6, 0.0),
        );
        let mut follow = EyeFollow::new(start.eye, up);
        // a head falling at 4 m/s: the old spring trailed it by half a metre
        for frame in 0..20 {
            let y = 1.6 - 4.0 * frame as f32 / 60.0;
            let head = view(
                Vec3::new(0.1, y, 0.0),
                Quat::IDENTITY,
                Vec3::new(0.0, y, 0.0),
            );
            let camera = follow.follow(head, up, &mut level, 1.0 / 60.0);
            assert!(
                (camera.position - head.eye.position).length() < 1e-4,
                "{frame}: {camera:?}"
            );
        }
    }

    #[test]
    fn a_face_on_the_ground_keeps_the_camera_in_the_head_above_it() {
        let up = Vec3::Y;
        let mut level = Planes(&[FLOOR]);
        // face down: the skull 0.1 m up, the eye 0.02 m up (its near plane
        // would see under the ground)
        let down = Quat::from_axis_angle(Vec3::X, 1.4);
        let head = view(Vec3::new(0.0, 0.02, 0.09), down, Vec3::new(0.0, 0.1, 0.0));
        let mut follow = EyeFollow::new(head.eye, up);
        let mut camera = Pose::IDENTITY;
        for _ in 0..60 {
            camera = follow.follow(head, up, &mut level, 1.0 / 60.0);
        }
        assert!(camera.position.y >= MARGIN - 1e-4, "{camera:?}");
        // between the skull and the eye: inside the hidden head
        assert!((camera.position - head.skull).length() <= 0.1, "{camera:?}");
        let x = camera.rotation.rotate(Vec3::X);
        let z = camera.rotation.rotate(Vec3::Z);
        assert!(x.dot(up).abs() < 1e-3, "rolled: {x:?}");
        assert!(z.dot(up).abs() <= MAX_PITCH_SIN + 1e-3);
    }

    #[test]
    fn a_wall_before_the_eye_pulls_the_camera_back_and_a_skull_at_it_stops_it() {
        let up = Vec3::Y;
        // a wall at x = 0.15 facing -x; the eye 0.12 past the skull at x 0
        let mut level = Planes(&[FLOOR, WALL]);
        let head = view(
            Vec3::new(0.12, 0.5, 0.0),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.5, 0.0),
        );
        let mut follow = EyeFollow::new(head.eye, up);
        let camera = follow.follow(head, up, &mut level, 1.0 / 60.0);
        assert!(camera.position.x <= 0.15 - MARGIN + 1e-4, "{camera:?}");
        assert!(camera.position.x >= 0.0, "{camera:?}");
        // a skull nearer the wall than the margin: the camera keeps its
        // margin from the wall
        let close = view(
            Vec3::new(0.16, 0.5, 0.0),
            Quat::IDENTITY,
            Vec3::new(0.12, 0.5, 0.0),
        );
        let camera = follow.follow(close, up, &mut level, 1.0 / 60.0);
        assert!(camera.position.x <= 0.15 - MARGIN + 1e-4, "{camera:?}");
    }

    #[test]
    fn a_head_sunk_into_a_wall_leaves_the_camera_before_it_until_it_comes_out() {
        let up = Vec3::Y;
        let mut level = Planes(&[FLOOR, WALL]);
        let at = |x: f32| {
            view(
                Vec3::new(x + 0.1, 0.5, 0.02),
                Quat::IDENTITY,
                Vec3::new(x, 0.5, 0.0),
            )
        };
        let mut follow = EyeFollow::new(at(-0.3).eye, up);
        // the whole head goes through the wall at 0.15 and back
        let mut path: [f32; 24] = [0.0; 24];
        for (i, x) in path.iter_mut().enumerate() {
            let t = i as f32 / 23.0;
            *x = -0.3 + 0.9 * (1.0 - (2.0 * t - 1.0).abs());
        }
        for (i, &x) in path.iter().enumerate() {
            let head = at(x);
            let camera = follow.follow(head, up, &mut level, 1.0 / 60.0);
            assert!(
                camera.position.x <= 0.15 - MARGIN + 1e-4,
                "{i}: through the wall: {camera:?}"
            );
            assert!(camera.is_finite());
        }
        // out again: on the eye
        let head = at(-0.3);
        let camera = follow.follow(head, up, &mut level, 1.0 / 60.0);
        assert!(
            (camera.position - head.eye.position).length() < 1e-4,
            "{camera:?}"
        );
    }

    #[test]
    fn spent_rays_keep_the_last_reach() {
        struct Spent;
        impl Level for Spent {
            fn sweep(&mut self, _: Vec3, _: Vec3, _: f32) -> Sweep {
                Sweep::Deferred
            }
        }
        let up = Vec3::Y;
        let mut level = Planes(&[FLOOR, WALL]);
        let head = view(
            Vec3::new(0.12, 0.5, 0.0),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.5, 0.0),
        );
        let mut follow = EyeFollow::new(head.eye, up);
        let blocked = follow.follow(head, up, &mut level, 1.0 / 60.0);
        let kept = follow.follow(head, up, &mut Spent, 1.0 / 60.0);
        assert!(
            (kept.position - blocked.position).length() < 1e-4,
            "{kept:?} {blocked:?}"
        );
    }

    #[test]
    fn a_straight_down_eye_and_bad_input_stay_finite() {
        let up = Vec3::Y;
        let down = Pose::new(
            Quat::from_axis_angle(Vec3::X, core::f32::consts::FRAC_PI_2),
            Vec3::ZERO,
        );
        let mut follow = EyeFollow::new(down, up);
        let mut level = Planes(&[]);
        let nan = Vec3::new(f32::NAN, 0.0, 0.0);
        for (dt, skull) in [
            (0.0, Vec3::ZERO),
            (1.0 / 60.0, Vec3::Y),
            (5.0, nan),
            (f32::NAN, Vec3::Y),
        ] {
            let head = HeadView {
                eye: down,
                skull,
                ground: None,
            };
            assert!(follow.follow(head, up, &mut level, dt).is_finite());
        }
    }
}
