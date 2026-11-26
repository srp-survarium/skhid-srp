//! Patch 215: everything held in first person moves with the view in TAA, as
//! the gun and the hands do (the owner, 2026-11-26: "motion blur breaking
//! scope the way it looks": the optic smeared and see-through, the gun and
//! the side sight beside it sharp).
//!
//! TAA takes half of a pixel's colour from where its motion vector says the
//! pixel was the frame before, while that motion is under 1 % of the screen
//! (`temporal_antialiasing.ps`, no clamp to the pixel's neighbours).
//! accumulate_motion_vectors writes each surface's: a skinned surface's
//! shader writes 0 with the object flag (the gun and the hands keep their own
//! pixel's history), a static one's its position now less its position the
//! frame before (its world and view matrices of the frame before). The
//! optics and the parts of patch 161 are static models (0.69d0's modules),
//! which the weapon's draw moves with the camera; turning, stepping and the
//! gun's sway gave them a motion of a few pixels that the screen does not
//! show, so TAA took half of each of their pixels from the world beside them
//! (the optic half see-through) and the motion blur smeared them along it.
//!
//! A first-person surface's previous transform is now its world and view
//! matrices of this frame: its motion is 0 with the object flag, the gun's.
//! World surfaces keep theirs.
#![forbid(unsafe_code)]

use crate::vostok::motion_vectors::{Matrix, MotionDraw};
use crate::vostok::occlusion::Drawn;
use crate::vostok::{Context, Flow, NoExit};

/// `a` then `b`, row vectors (the game's 0xEE700).
fn product(a: &Matrix, b: &Matrix) -> Matrix {
    core::array::from_fn(|i| {
        let (row, column) = (i / 4, i % 4);
        (0..4).map(|k| a[row * 4 + k] * b[k * 4 + column]).sum()
    })
}

/// The previous transform a surface drawn so is given: None keeps the game's.
fn previous(drawn: Drawn, world: &Matrix, view: &Matrix) -> Option<Matrix> {
    match drawn {
        Drawn::FirstPerson => Some(product(world, view)),
        Drawn::World => None,
    }
}

/// `motion_vectors.previous`: a first-person surface moves with the view.
pub fn on_previous(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(draw) = MotionDraw::at(ctx)
        && let Some(held) = previous(draw.drawn, &draw.world, &draw.view)
    {
        draw.set_previous(held);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: Matrix = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

    fn translation(x: f32, y: f32, z: f32) -> Matrix {
        let mut m = IDENTITY;
        m[12] = x;
        m[13] = y;
        m[14] = z;
        m
    }

    #[test]
    fn the_product_takes_the_world_matrix_first() {
        // a point at the origin, put at (1, 2, 3) by the world and moved by
        // (-1, 0, 5) by the view: (0, 2, 8)
        let p = product(&translation(1.0, 2.0, 3.0), &translation(-1.0, 0.0, 5.0));
        assert_eq!(&p[12..15], &[0.0, 2.0, 8.0]);
        let turn = [0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        // row vectors: x' = x * m: the world's translation is turned by the view
        assert_eq!(&product(&translation(1.0, 0.0, 0.0), &turn)[12..15], &[0.0, 1.0, 0.0]);
    }

    #[test]
    fn only_first_person_surfaces_are_held_still() {
        let world = translation(1.0, 2.0, 3.0);
        assert_eq!(previous(Drawn::FirstPerson, &world, &IDENTITY), Some(world));
        assert_eq!(previous(Drawn::World, &world, &IDENTITY), None);
    }
}
