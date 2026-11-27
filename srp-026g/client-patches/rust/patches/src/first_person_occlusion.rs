//! Patch 213: the hi-Z occlusion never leaves a first-person surface out:
//! the gun in the hands, its parts, its optic and the optic's reticle (the
//! owner, 2026-11-26: "the red dot often flickers when scoping or moving";
//! renderer-026g.md 5.7).
//!
//! Every surface in the camera's view takes the world's hi-Z test, the
//! first person's too: its bounding sphere, projected with the world's
//! camera, against the linear depth of a frame before (the 3x3 maximum of
//! `$user$position`, then its mips), its answer a frame or more late, and
//! read at the index the surface had in the last query (another object's
//! answer when it was not in that query). An occluded surface is left out
//! of every pass. A first-person surface is drawn over the world whatever
//! is in front of it there, so the world's depth says nothing of it; and a
//! collimator's dot, a disk under a millimetre across, is the smallest
//! thing it tests: a wrong answer drops it for a frame or more, the housing
//! around it (another surface, another answer) staying.
//!
//! The first-person pass's own test is kept: its surfaces are the ones
//! render_forward_models draws there (their model's foreground byte or
//! their own). World surfaces keep their answers.

#![forbid(unsafe_code)]

use crate::vostok::occlusion::{self, Answer, Drawn, ModelAnswer};
use crate::vostok::{Context, Flow, NoExit};

/// The answer the frame acts on, for the hi-Z's answer of a surface.
fn acted(drawn: Drawn, answer: Answer) -> Answer {
    match drawn {
        Drawn::FirstPerson => Answer::Visible,
        Drawn::World => answer,
    }
}

/// `hiz.model_answer`: a first-person surface is stored visible.
pub fn on_model_answer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(ModelAnswer { drawn, answer }) = ModelAnswer::at(ctx) else {
        return Flow::Continue;
    };
    occlusion::store(ctx, acted(drawn, answer));
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_person_surface_is_never_occluded() {
        assert_eq!(acted(Drawn::FirstPerson, Answer::Occluded), Answer::Visible);
        assert_eq!(acted(Drawn::FirstPerson, Answer::Visible), Answer::Visible);
    }

    #[test]
    fn a_world_surface_keeps_its_answer() {
        assert_eq!(acted(Drawn::World, Answer::Occluded), Answer::Occluded);
        assert_eq!(acted(Drawn::World, Answer::Visible), Answer::Visible);
    }
}
