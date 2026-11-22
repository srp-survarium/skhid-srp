//! The hi-Z occlusion's answers for the frame's lights, at the stores of
//! stage_visibility::occlusion_culling (0x1B79A0; renderer-026g.md 5.7),
//! for patch 190 (light_occlusion).
//!
//! occlusion_culling runs in a frame whose query results came in
//! (quary_and_get_results_if_ready 0x1D7E30). Each object in view reads its
//! answer at the index it had in the last query and stores it (the sites:
//! `mov [ecx+N],al`, AL 1 occluded), then takes its index in the query sent
//! now. The answer is a frame or more late, from a depth one frame older
//! still. The render thread's.

use super::context::{Context, Reg};
use crate::raw;

/// What the hi-Z query answered for an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Visible,
    Occluded,
}

impl Answer {
    fn of(al: u32) -> Answer {
        if al & 0xFF == 0 { Answer::Visible } else { Answer::Occluded }
    }

    fn al(self) -> u8 {
        match self {
            Answer::Visible => 0,
            Answer::Occluded => 1,
        }
    }
}

/// A point or spot light, by its address (the same light frame to frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightId(u32);

impl LightId {
    #[cfg(test)]
    pub const fn test(id: u32) -> LightId {
        LightId(id)
    }
}

/// `hiz.light_answer`: a point or spot light's answer, about to be stored.
pub struct LightAnswer {
    pub light: LightId,
    pub answer: Answer,
}

impl LightAnswer {
    pub fn at(ctx: &Context<'_>) -> Option<LightAnswer> {
        // SAFETY: the site's registers: the light (ECX) and its answer (AL).
        unsafe {
            let light = raw::site::hiz_light_answer::light(ctx)?;
            Some(LightAnswer {
                light: LightId(light.address()),
                answer: Answer::of(raw::site::hiz_light_answer::answer(ctx)),
            })
        }
    }
}

/// At `hiz.light_answer`, `hiz.ambient_light_answer` or `hiz.probe_answer`:
/// the answer the game stores.
pub fn store(ctx: &mut Context<'_>, answer: Answer) {
    ctx.set_reg8(Reg::Eax, answer.al());
}
