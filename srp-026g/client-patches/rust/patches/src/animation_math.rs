//! Patch 134: the animation's math, as fast as it can be while giving the
//! same floats (the simulation thread's largest engine costs after Bullet).
//!
//! Every frame each bone of each animation turns its Euler angles into a
//! quaternion (quaternion::quaternion 0xED2E0: a sine and a cosine of each
//! half angle) and a weighted one goes through an axis and an angle
//! (get_axis_and_angle 0x74300: an atan2; quaternion(axis, angle) 0x74290: a
//! sine and a cosine). The engine's sin, cos and atan2 are the x87's fsin,
//! fcos and fpatan, about 100 cycles each; every one of these sites stores
//! the result as a float at once. `vostok::trig` gives that float with SSE2
//! doubles where it is provably the same, the game's own function otherwise
//! (the proof and its tests are there). The knot search (time_channel::domain
//! 0x3B3340) is animation_math_026g.asm: the same tries without a division
//! each.

#![forbid(unsafe_code)]

use crate::vostok::trig;
use crate::vostok::{OrigAtan2, OrigCos, OrigSin};

fn sine(sin: &OrigSin, angle: f32) -> f32 {
    trig::sin(angle).unwrap_or_else(|| sin.call(angle))
}

fn cosine(cos: &OrigCos, angle: f32) -> f32 {
    trig::cos(angle).unwrap_or_else(|| cos.call(angle))
}

/// `quaternion.sin_x`: sin of half the x angle.
pub fn on_sin_x(sin: &OrigSin, angle: f32) -> f32 {
    sine(sin, angle)
}

/// `quaternion.cos_x`: cos of half the x angle.
pub fn on_cos_x(cos: &OrigCos, angle: f32) -> f32 {
    cosine(cos, angle)
}

/// `quaternion.sin_y`: sin of half the y angle.
pub fn on_sin_y(sin: &OrigSin, angle: f32) -> f32 {
    sine(sin, angle)
}

/// `quaternion.cos_y`: cos of half the y angle.
pub fn on_cos_y(cos: &OrigCos, angle: f32) -> f32 {
    cosine(cos, angle)
}

/// `quaternion.sin_z`: sin of half the z angle.
pub fn on_sin_z(sin: &OrigSin, angle: f32) -> f32 {
    sine(sin, angle)
}

/// `quaternion.cos_z`: cos of half the z angle.
pub fn on_cos_z(cos: &OrigCos, angle: f32) -> f32 {
    cosine(cos, angle)
}

/// `quaternion.axis_sin`: sin of half the angle about an axis.
pub fn on_axis_sin(sin: &OrigSin, angle: f32) -> f32 {
    sine(sin, angle)
}

/// `quaternion.axis_cos`: cos of half the angle about an axis.
pub fn on_axis_cos(cos: &OrigCos, angle: f32) -> f32 {
    cosine(cos, angle)
}

/// `quaternion.angle_atan2`: the half angle of a quaternion, atan2(|xyz|, w).
pub fn on_angle_atan2(atan2: &OrigAtan2, y: f32, x: f32) -> f32 {
    trig::atan2(y, x).unwrap_or_else(|| atan2.call(y, x))
}
