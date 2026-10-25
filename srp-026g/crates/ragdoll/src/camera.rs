//! The death camera on a ragdoll's head (patch 46 rides the head bone).
//!
//! A tumbling head would shake and roll the view and dip it into the
//! ground. The camera follows the head instead: its place and its forward
//! by critically damped springs, its up kept level (no roll), its pitch
//! within +-55 degrees, and its place kept above the head's ground.
//!
//! The head bone's frame (the hit targets of `base_character_full`: the
//! brain sphere at +X, the face box at +Y): +X is up along the neck, +Y the
//! face's forward.

use crate::math::{Pose, Quat, Vec3, clamp, exp_neg};

/// How fast the camera follows (rad/s of the critically damped springs):
/// about 0.25 s to settle on a still head.
const FOLLOW_RATE: f32 = 12.0;
const TURN_RATE: f32 = 9.0;
/// The view's pitch stays within this (sin of 55 degrees).
const MAX_PITCH_SIN: f32 = 0.819;
/// The eye stays this high above the ground the head lies on.
const EYE_CLEARANCE: f32 = 0.22;

/// The follow's state: where the camera is and how fast it moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadFollow {
    position: Vec3,
    velocity: Vec3,
    forward: Vec3,
    turning: Vec3,
}

/// One critically damped spring step of `x` (speed `v`) toward `target`.
fn spring(x: Vec3, v: Vec3, target: Vec3, omega: f32, dt: f32) -> (Vec3, Vec3) {
    let d = x - target;
    let temp = (v + d * omega) * dt;
    let e = exp_neg(omega * dt);
    (target + (d + temp) * e, (v - temp * omega) * e)
}

/// The head's forward, its pitch limited, made unit.
fn level_forward(forward: Vec3, up: Vec3) -> Vec3 {
    let pitch = clamp(forward.dot(up), -MAX_PITCH_SIN, MAX_PITCH_SIN);
    let flat = forward.reject(up).direction_or(up.any_perpendicular());
    flat * crate::math::sqrt(1.0 - pitch * pitch) + up * pitch
}

impl HeadFollow {
    /// The follow starting on the head at `head`.
    pub fn new(head: Pose, up: Vec3) -> HeadFollow {
        HeadFollow {
            position: head.position,
            velocity: Vec3::ZERO,
            forward: level_forward(head.rotation.rotate(Vec3::Y), up),
            turning: Vec3::ZERO,
        }
    }

    /// One frame: the camera's head pose (the bone's convention) after
    /// following `head` for `dt` s. `ground` is the plane the head rests on
    /// (a point and its unit normal), if any.
    pub fn follow(&mut self, head: Pose, up: Vec3, ground: Option<(Vec3, Vec3)>, dt: f32) -> Pose {
        let dt = clamp(dt, 0.0, 0.1);
        let mut target = head.position;
        if let Some((point, normal)) = ground {
            let height = (target - point).dot(normal);
            if height < EYE_CLEARANCE {
                target += normal * (EYE_CLEARANCE - height);
            }
        }
        (self.position, self.velocity) = spring(self.position, self.velocity, target, FOLLOW_RATE, dt);
        let wanted = level_forward(head.rotation.rotate(Vec3::Y), up);
        let (forward, turning) = spring(self.forward, self.turning, wanted, TURN_RATE, dt);
        self.forward = level_forward(forward.direction_or(wanted), up);
        self.turning = turning;
        // the bone's frame: X up (level: no roll), Y forward
        let y = self.forward;
        let x = up.reject(y).direction_or(up);
        let z = x.cross(y);
        Pose::new(Quat::from_basis(x, y, z), self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_settles_on_a_still_head_level_and_above_the_ground() {
        let up = Vec3::Y;
        // a head lying on its side, face tilted down, on the ground
        let roll = Quat::from_axis_angle(Vec3::Y, 1.4) * Quat::from_axis_angle(Vec3::X, 0.9);
        let head = Pose::new(roll, Vec3::new(1.0, 0.1, 2.0));
        let mut follow = HeadFollow::new(Pose::new(Quat::IDENTITY, Vec3::new(1.0, 1.6, 2.0)), up);
        let mut eye = Pose::IDENTITY;
        let mut last = Vec3::new(1.0, 1.6, 2.0);
        for _ in 0..120 {
            eye = follow.follow(head, up, Some((Vec3::ZERO, up)), 1.0 / 60.0);
            // no jump: never more than the spring's pace a frame
            assert!((eye.position - last).length() < 0.2);
            last = eye.position;
        }
        assert!(eye.position.y >= EYE_CLEARANCE - 1e-3, "{eye:?}");
        let x = eye.rotation.rotate(Vec3::X);
        let y = eye.rotation.rotate(Vec3::Y);
        // level: the camera's side has no up component
        assert!(x.cross(y).dot(up).abs() < 1e-3, "rolled: {x:?} {y:?}");
        assert!(y.dot(up).abs() <= MAX_PITCH_SIN + 1e-3);
        assert!(eye.is_finite());
    }

    #[test]
    fn a_straight_down_face_and_bad_input_stay_finite() {
        let up = Vec3::Y;
        let down = Pose::new(Quat::from_axis_angle(Vec3::X, core::f32::consts::FRAC_PI_2), Vec3::ZERO);
        let mut follow = HeadFollow::new(down, up);
        for dt in [0.0, 1.0 / 60.0, 5.0, f32::NAN] {
            assert!(follow.follow(down, up, None, dt).is_finite());
        }
    }
}
