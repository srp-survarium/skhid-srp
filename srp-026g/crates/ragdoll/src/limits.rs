//! The joints' limits (the Bodycam study's table) and the angle helpers.
//!
//! Each limit is kept as vectors in the two bodies' own frames, made once
//! from the pose at the death, so a limit is measured from the body as it
//! is now, never from a stored angle.

use crate::body::{Body, rotate_about, share_turn};
use crate::math::{PI, Vec3, abs, atan2, clamp, sqrt};

pub const fn degrees(d: f32) -> f32 {
    d * (PI / 180.0)
}

/// The signed angle from the unit `a` to the unit `b` about the unit `n`
/// (both taken in the plane normal to `n` as they are).
pub fn signed_angle(a: Vec3, b: Vec3, n: Vec3) -> f32 {
    atan2(a.cross(b).dot(n), a.dot(b))
}

/// The unsigned angle between the unit `a` and `b`.
pub fn angle_between(a: Vec3, b: Vec3) -> f32 {
    atan2(a.cross(b).length(), a.dot(b))
}

/// `v` carried by the shortest rotation that takes the unit `from` to the
/// unit `to` (the swing; `v` unchanged when they are opposite).
pub fn swing_carry(v: Vec3, from: Vec3, to: Vec3) -> Vec3 {
    let c = from.dot(to);
    if c <= -0.999 {
        return v;
    }
    let k = from.cross(to);
    v * c + k.cross(v) + k * (k.dot(v) / (1.0 + c))
}

/// How far a swing may go from the cone's axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cone {
    /// The same angle every way.
    Round(f32),
    /// Toward the parent's forward vector, away from it, and sideways (an
    /// ellipse between them).
    Shaped { forward: f32, back: f32, side: f32 },
}

impl Cone {
    /// The cone's angle in the direction `toward` (a unit vector normal to
    /// the axis), `forward` the parent's forward (normal to the axis too).
    pub fn reach(self, toward: Vec3, forward: Vec3) -> f32 {
        match self {
            Cone::Round(a) => a,
            Cone::Shaped {
                forward: f,
                back,
                side,
            } => {
                let c = toward.dot(forward);
                let along = if c >= 0.0 { f } else { back };
                let s2 = 1.0 - c * c;
                // the ellipse's radius: 1/r^2 = cos^2/a^2 + sin^2/b^2
                1.0 / sqrt(c * c / (along * along) + s2 / (side * side))
            }
        }
    }
}

/// A joint's limit, its vectors in the parent's (`*_parent`) and the
/// child's (`*_child`) frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Limit {
    /// A ball joint: the child's axis within a cone about the parent's
    /// axis, and its twist about its own axis within +-`twist`.
    Swing {
        cone: Cone,
        axis_parent: Vec3,
        forward_parent: Vec3,
        axis_child: Vec3,
        reference_parent: Vec3,
        reference_child: Vec3,
        twist: f32,
    },
    /// A hinge about `hinge_*`: the child's limb axis turned from the
    /// parent's by `min..max` about it.
    Hinge {
        hinge_parent: Vec3,
        hinge_child: Vec3,
        limb_parent: Vec3,
        limb_child: Vec3,
        min: f32,
        max: f32,
    },
}

impl Limit {
    /// One XPBD pass of the limit on the two bodies (compliance `alpha_h`).
    pub fn solve(&self, parent: &mut Body, child: &mut Body, alpha_h: f32) {
        match *self {
            Limit::Swing {
                cone,
                axis_parent,
                forward_parent,
                axis_child,
                reference_parent,
                reference_child,
                twist,
            } => {
                // swing
                let a = parent.pose.rotation.rotate(axis_parent);
                let b = child.pose.rotation.rotate(axis_child);
                let angle = angle_between(a, b);
                if let Some(toward) = b.reject(a).direction() {
                    let forward = parent.pose.rotation.rotate(forward_parent);
                    let reach = cone.reach(toward, forward);
                    if angle > reach {
                        let n = a.cross(b).direction_or(toward.cross(a));
                        let edge = rotate_about(a, n, reach);
                        share_turn(child, parent, b.cross(edge), alpha_h);
                    }
                }
                // twist
                let a = parent.pose.rotation.rotate(axis_parent);
                let b = child.pose.rotation.rotate(axis_child);
                let carried =
                    swing_carry(parent.pose.rotation.rotate(reference_parent), a, b).reject(b);
                let own = child.pose.rotation.rotate(reference_child).reject(b);
                if let (Some(carried), Some(own)) = (carried.direction(), own.direction()) {
                    let phi = signed_angle(carried, own, b);
                    if abs(phi) > twist {
                        let excess = phi - clamp(phi, -twist, twist);
                        share_turn(child, parent, b * -excess, alpha_h);
                    }
                }
            }
            Limit::Hinge {
                hinge_parent,
                hinge_child,
                limb_parent,
                limb_child,
                min,
                max,
            } => {
                // keep the hinge axes one
                let h1 = parent.pose.rotation.rotate(hinge_parent);
                let h2 = child.pose.rotation.rotate(hinge_child);
                share_turn(child, parent, h2.cross(h1), alpha_h);
                // the bend within min..max
                let h = parent.pose.rotation.rotate(hinge_parent);
                let p = parent
                    .pose
                    .rotation
                    .rotate(limb_parent)
                    .reject(h)
                    .direction();
                let c = child.pose.rotation.rotate(limb_child).reject(h).direction();
                if let (Some(p), Some(c)) = (p, c) {
                    let phi = signed_angle(p, c, h);
                    let target = clamp(phi, min, max);
                    if phi != target {
                        share_turn(child, parent, h * (target - phi), alpha_h);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert!((signed_angle(Vec3::X, Vec3::Y, Vec3::Z) - PI / 2.0).abs() < 1e-5);
        assert!((signed_angle(Vec3::Y, Vec3::X, Vec3::Z) + PI / 2.0).abs() < 1e-5);
        let carried = swing_carry(Vec3::Z, Vec3::X, Vec3::Y);
        assert!((carried - Vec3::Z).length() < 1e-6);
        let carried = swing_carry(Vec3::Y, Vec3::X, Vec3::Y);
        assert!((carried + Vec3::X).length() < 1e-6);
        let cone = Cone::Shaped {
            forward: 1.0,
            back: 0.5,
            side: 0.8,
        };
        assert!((cone.reach(Vec3::X, Vec3::X) - 1.0).abs() < 1e-6);
        assert!((cone.reach(-Vec3::X, Vec3::X) - 0.5).abs() < 1e-6);
        assert!((cone.reach(Vec3::Y, Vec3::X) - 0.8).abs() < 1e-6);
    }
}
