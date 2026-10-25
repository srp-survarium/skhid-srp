//! A rigid capsule body and the XPBD corrections on it (Müller et al.,
//! "Detailed Rigid Body Simulation with Extended Position Based Dynamics",
//! 2020): a positional correction at a point, an angular correction, and
//! the velocity update after a substep.

use crate::math::{Pose, Quat, Vec3, max, min};

/// A capsule along the body's local X axis: the segment from -half to
/// +half, swept by `radius`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub half: f32,
    pub radius: f32,
}

impl Capsule {
    /// The segment's two ends in world space, for a body at `pose`.
    pub fn ends(self, pose: Pose) -> (Vec3, Vec3) {
        let axis = pose.rotation.rotate(Vec3::X) * self.half;
        (pose.position - axis, pose.position + axis)
    }
}

/// What a body touches of the level: the plane of its last swept query,
/// kept for the substeps of a frame and the frames after while the body
/// does not move away from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    Free,
    Plane { point: Vec3, normal: Vec3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub pose: Pose,
    /// The pose at the start of the substep (the velocity update's).
    pub previous: Pose,
    pub velocity: Vec3,
    /// World angular velocity, rad/s.
    pub spin: Vec3,
    pub inverse_mass: f32,
    /// The inverse of the diagonal inertia in the body's frame.
    pub inverse_inertia: Vec3,
    pub shape: Capsule,
    pub contact: Contact,
}

impl Body {
    /// A capsule of `mass` kg at `pose` (a solid cylinder's inertia, the
    /// length the capsule's segment plus one radius).
    pub fn new(pose: Pose, shape: Capsule, mass: f32) -> Body {
        let r = shape.radius;
        let l = 2.0 * shape.half + r;
        let across = mass * (3.0 * r * r + l * l) / 12.0;
        // a thin limb's spin about its own axis boosted to a quarter of the
        // other two: the usual ragdoll stabilizer (no roll or twist jitter)
        let axial = max(0.5 * mass * r * r, 0.25 * across);
        Body {
            pose,
            previous: pose,
            velocity: Vec3::ZERO,
            spin: Vec3::ZERO,
            inverse_mass: 1.0 / mass,
            inverse_inertia: Vec3::new(1.0 / axial, 1.0 / across, 1.0 / across),
            shape,
            contact: Contact::Free,
        }
    }

    /// I^-1 v, in world space.
    pub fn inverse_inertia_times(&self, v: Vec3) -> Vec3 {
        let q = self.pose.rotation;
        q.rotate(q.conjugate().rotate(v).mul_each(self.inverse_inertia))
    }

    /// The generalized inverse mass of a push along the unit `n` at the
    /// world offset `r` from the centre (0 offset: the linear part alone).
    pub fn push_weight(&self, r: Vec3, n: Vec3) -> f32 {
        let rn = r.cross(n);
        self.inverse_mass + rn.dot(self.inverse_inertia_times(rn))
    }

    /// The generalized inverse inertia about the unit axis `n`.
    pub fn turn_weight(&self, n: Vec3) -> f32 {
        n.dot(self.inverse_inertia_times(n))
    }

    /// Moves the body by the positional impulse `p` at the world offset `r`.
    pub fn apply_push(&mut self, r: Vec3, p: Vec3) {
        self.pose.position += p * self.inverse_mass;
        let turn = self.inverse_inertia_times(r.cross(p));
        self.pose.rotation = self.pose.rotation.integrate(turn);
    }

    /// Turns the body by the angular impulse `p`.
    pub fn apply_turn(&mut self, p: Vec3) {
        let turn = self.inverse_inertia_times(p);
        self.pose.rotation = self.pose.rotation.integrate(turn);
    }

    /// The velocity impulse `j` at the world offset `r`.
    pub fn apply_impulse(&mut self, r: Vec3, j: Vec3) {
        self.velocity += j * self.inverse_mass;
        self.spin += self.inverse_inertia_times(r.cross(j));
    }

    /// How fast the capsule's surface moves at most (an upper bound: the
    /// speed plus the spin times the reach of its ends).
    pub fn end_speed(&self) -> f32 {
        self.velocity.length() + self.spin.length() * (self.shape.half + self.shape.radius)
    }

    /// The velocity of the world point at offset `r` from the centre.
    pub fn point_velocity(&self, r: Vec3) -> Vec3 {
        self.velocity + self.spin.cross(r)
    }

    /// The substep's start: gravity, damping and the move by the velocity.
    pub fn integrate(&mut self, h: f32, gravity: Vec3, linear_damping: f32, angular_damping: f32) {
        self.previous = self.pose;
        self.velocity += gravity * h;
        self.velocity = self.velocity * (1.0 - min(1.0, linear_damping * h));
        self.spin = self.spin * (1.0 - min(1.0, angular_damping * h));
        self.pose.position += self.velocity * h;
        self.pose.rotation = self.pose.rotation.integrate(self.spin * h);
    }

    /// The substep's end: the velocities the move gave.
    pub fn update_velocity(&mut self, h: f32) {
        self.velocity = (self.pose.position - self.previous.position) * (1.0 / h);
        self.spin = self.pose.rotation.rotation_from(self.previous.rotation) * (1.0 / h);
    }
}

/// Two bodies, or one against the fixed world.
pub enum Pair<'a> {
    Two(&'a mut Body, &'a mut Body),
    One(&'a mut Body),
}

/// Moves the world point `a` of the first body (offset `ra`) and `b` of the
/// second (offset `rb`) toward each other by `gap` = b - a, with the
/// compliance `alpha_h` (alpha / h^2). Returns the impulse's size.
pub fn close_gap(pair: Pair<'_>, ra: Vec3, rb: Vec3, gap: Vec3, alpha_h: f32) -> f32 {
    let c = gap.length();
    let Some(n) = gap.direction() else { return 0.0 };
    match pair {
        Pair::Two(a, b) => {
            let w = a.push_weight(ra, n) + b.push_weight(rb, n);
            let lambda = c / (w + alpha_h);
            a.apply_push(ra, n * lambda);
            b.apply_push(rb, n * -lambda);
            lambda
        }
        Pair::One(a) => {
            let w = a.push_weight(ra, n);
            let lambda = c / (w + alpha_h);
            a.apply_push(ra, n * lambda);
            lambda
        }
    }
}

/// Turns the first body by the rotation vector `turn` and the second by its
/// opposite, shared by their inertia, with the compliance `alpha_h`.
pub fn share_turn(a: &mut Body, b: &mut Body, turn: Vec3, alpha_h: f32) {
    let angle = turn.length();
    let Some(n) = turn.direction() else { return };
    let w = a.turn_weight(n) + b.turn_weight(n);
    let lambda = angle / (w + alpha_h);
    a.apply_turn(n * lambda);
    b.apply_turn(n * -lambda);
}

/// Rotates the unit `v` about the unit `axis` by `angle`.
pub fn rotate_about(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    Quat::from_axis_angle(axis, angle).rotate(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_push_conserves_momentum_between_two_bodies() {
        let shape = Capsule {
            half: 0.2,
            radius: 0.05,
        };
        let mut a = Body::new(Pose::new(Quat::IDENTITY, Vec3::ZERO), shape, 2.0);
        let mut b = Body::new(
            Pose::new(Quat::IDENTITY, Vec3::new(1.0, 0.0, 0.0)),
            shape,
            6.0,
        );
        close_gap(
            Pair::Two(&mut a, &mut b),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::new(0.4, 0.0, 0.0),
            0.0,
        );
        // the gap closes, the centre of mass stays
        assert!(((b.pose.position - a.pose.position).x - 0.6).abs() < 1e-5);
        let centre = a.pose.position * 2.0 + b.pose.position * 6.0;
        assert!((centre.x - 6.0).abs() < 1e-5);
    }
}
