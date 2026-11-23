//! Patch 200: a shot leaves the gun's muzzle, the way the gun looks
//! (docs/notes/muzzle-shots.md).
//!
//! The game fires from the eyes: instant_fire (0x2E23A0) copies the user's
//! eyes' transform (player+44Ch, the bone Item) and takes the bullet's start
//! and direction from it, wherever the gun is. Here the copy is replaced, at
//! `shot.eye`, by the gun's own:
//!
//! - its place is the gun's fire point (the model's `barrel_point`), on the
//!   gun as the simulation poses it: the hands' Weapon bone, and as the sight
//!   is raised the aim's alignment the drawing makes (the aim locator the
//!   eye), blended by the aim's progress;
//! - its forward is the way the gun looks: along the barrel from the hip,
//!   along the sights, which the alignment lays on the eye's line, fully
//!   aimed, and between them as the sight is raised;
//! - a wall between the eye and the muzzle keeps the shot at the eye: no
//!   bullet starts beyond what the barrel pokes through.
//!
//! The spread, the kick and the bullets are the game's, from that transform.
//! Every game computes it for every player from the simulation's own state,
//! so a match plays out otherwise than before: simulation_changes.tsv.

#![forbid(unsafe_code)]

use ragdoll::math::Vec3;

use crate::affine::{product, row, with_row};

/// What a shot is taken from, all in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// The eyes' transform (the bone Item): rows right, up, forward, place.
    pub eye: [f32; 16],
    /// The gun's model: the hands' Weapon bone, with the aim's alignment as
    /// far as the sight is raised.
    pub gun: [f32; 16],
    /// The fire point in the gun's model (its locator on its bone): it looks
    /// along its -z.
    pub barrel: [f32; 16],
    /// How far the sight is raised, 0 from the hip to 1 fully aimed.
    pub raised: f32,
}

/// Where the muzzle is and the way the gun looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muzzle {
    pub place: Vec3,
    pub along: Vec3,
}

impl Pose {
    /// The muzzle: at the fire point; looking along the barrel from the hip
    /// and along the eye's line fully aimed (the sights are on it then), by
    /// the sight's raising between. None for a gun or an eye with no way.
    pub fn muzzle(&self) -> Option<Muzzle> {
        let fire = product(&self.barrel, &self.gun);
        let (barrel, eye) = ((row(&fire, 2) * -1.0).direction()?, row(&self.eye, 2).direction()?);
        let raised = if self.raised.is_finite() { self.raised.clamp(0.0, 1.0) } else { 0.0 };
        let along = (barrel * (1.0 - raised) + eye * raised).direction()?;
        let place = row(&fire, 3);
        (place.is_finite() && along.is_finite()).then_some(Muzzle { place, along })
    }
}

/// The transform a shot leaves from, laid out as the eyes' (rows right, up,
/// forward, place) and of its hand: from `place` along the muzzle's way, its
/// right and up the eye's made square to that. `eye` itself when the eye's
/// axes cannot be squared to it.
pub fn shot_from(eye: &[f32; 16], place: Vec3, along: Vec3) -> [f32; 16] {
    let across = |v: Vec3, of: Vec3| v - of * v.dot(of);
    let Some(right) = across(row(eye, 0), along).direction() else { return *eye };
    let Some(up) = across(across(row(eye, 1), along), right).direction() else { return *eye };
    [right, up, along, place].into_iter().enumerate().fold(*eye, |m, (at, v)| with_row(m, at, v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affine::IDENTITY;

    fn at(x: f32, y: f32, z: f32) -> [f32; 16] {
        with_row(IDENTITY, 3, Vec3::new(x, y, z))
    }

    /// A fire point 0.6 ahead of the gun's origin, looking along the gun's z
    /// (its own -z): a half turn about y.
    fn barrel() -> [f32; 16] {
        [-1., 0., 0., 0., 0., 1., 0., 0., 0., 0., -1., 0., 0., 0., 0.6, 1.]
    }

    /// A gun held 0.1 under and 0.05 right of the eye, turned `sine` of an
    /// angle to the left about the up.
    fn gun(sine: f32) -> [f32; 16] {
        let cosine = (1.0 - sine * sine).sqrt();
        [cosine, 0., sine, 0., 0., 1., 0., 0., -sine, 0., cosine, 0., 0.05, -0.1, 0., 1.]
    }

    fn pose(raised: f32) -> Pose {
        Pose { eye: at(0.0, 1.5, 0.0), gun: product(&gun(0.1), &at(0.0, 1.5, 0.0)), barrel: barrel(), raised }
    }

    #[test]
    fn from_the_hip_the_shot_goes_along_the_barrel() {
        let muzzle = pose(0.0).muzzle().unwrap();
        // 0.6 along the turned gun from 0.05 right and 0.1 under the eye
        assert!((muzzle.place - Vec3::new(0.05 - 0.06, 1.4, 0.6 * 0.99498744)).length() < 1e-6, "{muzzle:?}");
        assert!((muzzle.along - Vec3::new(-0.1, 0.0, 0.99498744)).length() < 1e-6, "{muzzle:?}");
    }

    #[test]
    fn fully_aimed_it_goes_along_the_eyes_line_from_the_muzzle() {
        let muzzle = pose(1.0).muzzle().unwrap();
        assert_eq!(muzzle.along, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(muzzle.place, pose(0.0).muzzle().unwrap().place);
    }

    #[test]
    fn between_it_turns_with_the_sights_raising() {
        let (hip, half, aimed) = (pose(0.0).muzzle().unwrap(), pose(0.5).muzzle().unwrap(), pose(1.0).muzzle().unwrap());
        assert!(half.along.x < aimed.along.x && half.along.x > hip.along.x, "{half:?}");
        assert!((half.along.length() - 1.0).abs() < 1e-6);
        for odd in [f32::NAN, -3.0] {
            assert_eq!(pose(odd).muzzle(), pose(0.0).muzzle());
        }
        assert_eq!(pose(7.0).muzzle(), pose(1.0).muzzle());
    }

    #[test]
    fn a_gun_with_no_way_gives_no_muzzle() {
        let flat = Pose { barrel: with_row(barrel(), 2, Vec3::new(0.0, 0.0, 0.0)), ..pose(0.0) };
        assert_eq!(flat.muzzle(), None);
    }

    #[test]
    fn the_shot_is_laid_out_as_the_eye() {
        let eye = at(0.0, 1.5, 0.0);
        let muzzle = pose(0.0).muzzle().unwrap();
        let shot = shot_from(&eye, muzzle.place, muzzle.along);
        let (right, up, forward) = (row(&shot, 0), row(&shot, 1), row(&shot, 2));
        assert_eq!((forward, row(&shot, 3)), (muzzle.along, muzzle.place));
        assert!(right.dot(up).abs() < 1e-6 && right.dot(forward).abs() < 1e-6 && up.dot(forward).abs() < 1e-6);
        assert!((right.cross(up) - forward).length() < 1e-6, "the eye's hand");
        assert!((up - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6 && right.x > 0.9);
        assert_eq!(shot[15], 1.0);
        // along the eye's own right: nothing to square, the eye as it is
        assert_eq!(shot_from(&eye, muzzle.place, Vec3::new(1.0, 0.0, 0.0)), eye);
    }
}
