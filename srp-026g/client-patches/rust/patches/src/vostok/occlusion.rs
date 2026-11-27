//! The hi-Z occlusion's answers for the frame's lights and model surfaces,
//! at the stores of stage_visibility::occlusion_culling (0x1B79A0;
//! renderer-026g.md 5.7), for patches 190 (light_occlusion) and 213
//! (first_person_occlusion).
//!
//! occlusion_culling runs in a frame whose query results came in
//! (quary_and_get_results_if_ready 0x1D7E30). Each object in view reads its
//! answer at the index it had in the last query and stores it (the sites:
//! `mov [ecx+N],al`, AL 1 occluded), then takes its index in the query sent
//! now. The answer is a frame or more late, from a depth one frame older
//! still. The render thread's.

use super::context::{Context, Reg};
use super::world::read;
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

/// An object's index in the query being sent: it grows within a pass of
/// occlusion_culling, from 0 at each (one counter for every kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueryIndex(pub u32);

/// `hiz.light_answer`: a point or spot light's answer, about to be stored.
pub struct LightAnswer {
    pub light: LightId,
    pub answer: Answer,
    pub index: QueryIndex,
}

impl LightAnswer {
    pub fn at(ctx: &Context<'_>) -> Option<LightAnswer> {
        // SAFETY: the site's registers: the light (ECX) and its answer (AL).
        unsafe {
            let light = raw::site::hiz_light_answer::light(ctx)?;
            Some(LightAnswer {
                light: LightId(light.address()),
                answer: Answer::of(raw::site::hiz_light_answer::answer(ctx)),
                index: QueryIndex(raw::site::hiz_light_answer::index(ctx)),
            })
        }
    }
}

/// How a model surface in the camera's view is drawn: what
/// render_forward_models (0x1AB412..0x1AB433) and stage_lights'
/// first-person pass (0x192DC4) go by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawn {
    /// In the first-person pass, after the world and over it: its model's
    /// foreground byte (+114h) or its own (+63h) is set.
    FirstPerson,
    /// With the world.
    World,
}

impl Drawn {
    fn of(model_foreground: u8, surface_foreground: u8) -> Drawn {
        if model_foreground != 0 || surface_foreground != 0 { Drawn::FirstPerson } else { Drawn::World }
    }
}

/// `hiz.model_answer`: a model surface's answer, about to be stored.
pub struct ModelAnswer {
    pub drawn: Drawn,
    pub answer: Answer,
}

impl ModelAnswer {
    pub fn at(ctx: &Context<'_>) -> Option<ModelAnswer> {
        // SAFETY: the site's registers: a surface of the camera's list (ECX),
        // alive for the pass, and its answer (AL); the surface's model is the
        // instance render_forward_models reads the same byte of.
        unsafe {
            let surface = raw::site::hiz_model_answer::surface(ctx)?;
            let model = surface.model().map_or(0, |model| read(model.foreground()));
            Some(ModelAnswer {
                drawn: Drawn::of(model, read(surface.foreground())),
                answer: Answer::of(raw::site::hiz_model_answer::answer(ctx)),
            })
        }
    }
}

/// At `hiz.light_answer`, `hiz.ambient_light_answer`, `hiz.probe_answer` or
/// `hiz.model_answer`: the answer the game stores.
pub fn store(ctx: &mut Context<'_>, answer: Answer) {
    ctx.set_reg8(Reg::Eax, answer.al());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surface_is_first_person_by_its_models_byte_or_its_own() {
        assert_eq!(Drawn::of(0, 0), Drawn::World);
        assert_eq!(Drawn::of(1, 0), Drawn::FirstPerson);
        assert_eq!(Drawn::of(0, 1), Drawn::FirstPerson);
        assert_eq!(Drawn::of(0xFF, 0xFF), Drawn::FirstPerson);
    }
}
