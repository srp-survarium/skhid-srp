//! The rain stages at their draw, on the render thread (patch 130,
//! docs/notes/client_patch.md section 3.82): which scene the frame draws,
//! the technique a stage applies, and stage_rain's drop scales. Every field
//! is fields_026g.tsv's, every site sites_026g.tsv's.
//!
//! - `pre_rain.wet_gate`, `pre_rain.wet_apply`: stage_pre_rain::execute
//!   (0x17CA20), ESI the stage;
//! - `rain.drop_scales`, `rain.drops_apply`: stage_rain::execute (0x1ADB00),
//!   EBP the stage.
//!
//! A stage's renderer context (+4) names the frame's scene (+416Ch, which
//! renderer::render 0x13D010 stores each frame before the stages run) and its
//! scene view (+4170h); both stay alive for the frame.

use core::num::NonZeroU32;

use super::context::{Context, Reg};
use crate::raw;

/// A render scene (vostok::render::scene) as an id: compared, never read
/// through. The same scene is the context's while a game scene is drawn and
/// that game scene's +4 (fields_026g.tsv `render_scene`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderSceneId(NonZeroU32);

impl RenderSceneId {
    pub(in crate::vostok) fn of(scene: raw::Scene) -> RenderSceneId {
        RenderSceneId(NonZeroU32::new(scene.address()).expect("a handle is never 0"))
    }
}

/// A technique of an effect, as res_effect::apply (0x11C1B0) takes it in ECX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Technique(u32);

impl Technique {
    pub const fn new(index: u32) -> Technique {
        Technique(index)
    }
}

/// The stage sites a frame's scene is read at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageSite {
    /// `pre_rain.wet_gate`.
    WetGate,
    /// `pre_rain.wet_apply`.
    WetApply,
    /// `rain.drop_scales`.
    DropScales,
    /// `rain.drops_apply`.
    DropsApply,
}

/// The renderer context of the stage at `site`.
fn context(ctx: &Context<'_>, site: StageSite) -> Option<raw::RendererContext> {
    // SAFETY: the context is the site's own (the handler is that site's):
    // ESI or EBP is the stage executing, on the render thread, alive for its
    // execute; +4 its renderer context, the renderer's for its life.
    unsafe {
        match site {
            StageSite::WetGate => raw::site::pre_rain_wet_gate::stage(ctx)?.context(),
            StageSite::WetApply => raw::site::pre_rain_wet_apply::stage(ctx)?.context(),
            StageSite::DropScales => raw::site::rain_drop_scales::stage(ctx)?.context(),
            StageSite::DropsApply => raw::site::rain_drops_apply::stage(ctx)?.context(),
        }
    }
}

/// The scene the frame draws (None before renderer::render named one).
pub fn drawn_scene(ctx: &Context<'_>, site: StageSite) -> Option<RenderSceneId> {
    // SAFETY: the renderer context's scene field, written by renderer::render
    // on this thread before it ran the stages; only its address is kept.
    unsafe { context(ctx, site)?.scene() }.map(RenderSceneId::of)
}

/// The apply sites: `pre_rain.wet_apply` (the wet pass, technique 0) and
/// `rain.drops_apply` (the sprite drops, technique 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplySite {
    Wet,
    Drops,
}

/// The machine for the site's exit to its res_effect::apply call with
/// `technique` of the stage's own effect: EAX the effect (the stage's +18h,
/// or +34h), ECX the technique, as the two replaced instructions would load
/// them with the game's technique. The caller takes the site's `apply` exit.
pub fn apply_technique(ctx: &mut Context<'_>, site: ApplySite, technique: Technique) {
    // SAFETY: the site's stage (see `context`); its effect field is a
    // res_effect address the stage's constructor stored, passed on in EAX
    // as the replaced `mov eax,[...]` passes it.
    let effect = unsafe {
        match site {
            ApplySite::Wet => raw::site::pre_rain_wet_apply::stage(ctx).map(|stage| stage.wet_effect().read()),
            ApplySite::Drops => raw::site::rain_drops_apply::stage(ctx).map(|stage| stage.drops_effect().read()),
        }
    }
    .expect("the stage executing");
    ctx.set_reg(Reg::Eax, effect);
    ctx.set_reg(Reg::Ecx, technique.0);
}

/// The environment's rain_u_scale and rain_v_scale of the frame's scene view
/// (`rain.drop_scales`).
pub fn rain_scales(ctx: &Context<'_>) -> Option<(f32, f32)> {
    // SAFETY: the frame's scene view (see `context`), its environment's two
    // floats, read on the render thread that draws it.
    unsafe {
        let view = context(ctx, StageSite::DropScales)?.scene_view()?;
        Some((view.rain_u_scale().read(), view.rain_v_scale().read()))
    }
}

/// Sets rain_drops_parameters1.z and .w as stage_rain::execute copies them
/// into the constant after this site (`rain.drop_scales`: its two locals).
pub fn set_drop_parameters(ctx: &Context<'_>, z: f32, w: f32) {
    // SAFETY: the site's ESP frame is stage_rain::execute's own, its two f32
    // locals at +40h and +44h read only by the two copies the span starts.
    unsafe {
        let frame = raw::site::rain_drop_scales::frame(ctx).expect("the site's ESP");
        frame.parameters1_z().write(z);
        frame.parameters1_w().write(w);
    }
}

#[cfg(test)]
impl RenderSceneId {
    pub fn for_test(address: u32) -> RenderSceneId {
        RenderSceneId(NonZeroU32::new(address).expect("a test id is not 0"))
    }
}
