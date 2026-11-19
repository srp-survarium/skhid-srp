//! The rain stages at their draw, on the render thread (patch 130,
//! docs/notes/client_patch.md section 3.82): which scene the frame draws,
//! the technique a stage applies, and stage_rain's drop scales. Every field
//! is fields_026g.tsv's, every site sites_026g.tsv's.
//!
//! - `pre_rain.wet_gate`, `pre_rain.wet_apply`: stage_pre_rain::execute
//!   (0x17CA20), ESI the stage;
//! - `rain.drop_scales`, `rain.drops_apply`: stage_rain::execute (0x1ADB00),
//!   EBP the stage;
//! - `atmosphere.god_rays`: the god-rays pass's entry (0x1A5520), its stack
//!   argument the stage_atmosphere.
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
    /// `atmosphere.god_rays`.
    GodRays,
    /// `atmosphere.geometry_fog`.
    GeometryFog,
    /// `atmosphere.geometry_fog_applied`.
    GeometryFogApplied,
    /// `atmosphere.tables_due`.
    TablesDue,
    /// `atmosphere.tables_apply`.
    TablesApply,
    /// `atmosphere.sky_apply`.
    SkyApply,
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
            StageSite::GodRays => raw::site::atmosphere_god_rays::frame(ctx)?.stage()?.context(),
            StageSite::GeometryFog => raw::site::atmosphere_geometry_fog::stage(ctx)?.context(),
            StageSite::GeometryFogApplied => raw::site::atmosphere_geometry_fog_applied::stage(ctx)?.context(),
            StageSite::TablesDue => raw::site::atmosphere_tables_due::stage(ctx)?.context(),
            StageSite::TablesApply => raw::site::atmosphere_tables_apply::stage(ctx)?.context(),
            StageSite::SkyApply => raw::site::atmosphere_sky_apply::stage(ctx)?.context(),
        }
    }
}

/// The scene the frame draws (None before renderer::render named one).
pub fn drawn_scene(ctx: &Context<'_>, site: StageSite) -> Option<RenderSceneId> {
    // SAFETY: the renderer context's scene field, written by renderer::render
    // on this thread before it ran the stages; only its address is kept.
    unsafe { context(ctx, site)?.scene() }.map(RenderSceneId::of)
}

/// Which of the renderer's two atmosphere stages a god-rays pass is for
/// (stage_atmosphere +ACh, its constructor's argument).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtmosphereStage {
    /// 0: the sky's (the scattering lookup's update, the dome).
    Sky,
    /// 1: the geometry's.
    Geometry,
}

/// The stage the god-rays pass at `atmosphere.god_rays` draws for (None for
/// a mode neither stage has).
pub fn god_rays_stage(ctx: &Context<'_>) -> Option<AtmosphereStage> {
    // SAFETY: the site's ESP is the pass's entry, its argument the stage
    // executing (stage_atmosphere::execute pushes it, alive for its call);
    // +ACh a field its constructor wrote.
    let mode = unsafe { raw::site::atmosphere_god_rays::frame(ctx)?.stage()?.mode().read() };
    match mode {
        0 => Some(AtmosphereStage::Sky),
        1 => Some(AtmosphereStage::Geometry),
        _ => None,
    }
}

/// The apply sites: `pre_rain.wet_apply` (the wet pass, technique 0) and
/// `rain.drops_apply` (the sprite drops, technique 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplySite {
    Wet,
    Drops,
    /// `atmosphere.tables_apply` (the scattering tables, technique 0).
    Tables,
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
            ApplySite::Tables => raw::site::atmosphere_tables_apply::stage(ctx)
                .and_then(|stage| stage.scattering_effect())
                .map(|effect| effect.address()),
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

/// The geometry fog of 0.11e (patch v011w_render): 0.26g's stage_atmosphere
/// draws one pass of its scattering effect, technique 5; 0.11e drew two,
/// a multiplicative and an additive one, which 0.26g still compiles as
/// techniques 3 and 4 (effect_atmospheric_scattering::compile 0x1D2585,
/// 0x1D2722; their stencil and targets as 5's, their blends dst x src and
/// one + one). The multiplicative pass takes 5's place at
/// `atmosphere.geometry_fog`, the additive one is drawn after it, before the
/// god rays (`atmosphere.god_rays`, the geometry stage).
pub const FOG_MULTIPLY: Technique = Technique::new(3);
pub const FOG_ADD: Technique = Technique::new(4);

/// 0.11e's scattering tables and sky (patch v011w_render): techniques 6
/// and 7, the twins effect_atmospheric_scattering compiles of its 0 and 1
/// (the same passes with 0.11e's pixel shaders; `vostok::effect_compile`).
pub const TABLES_V011W: Technique = Technique::new(6);
pub const SKY_V011W: Technique = Technique::new(7);

/// Makes the sky stage remake its scattering tables this frame
/// (`atmosphere.tables_due`): the scene view's targets id made unequal to
/// the context's (ECX), as a resize makes it; the tables' pass writes the
/// context's again once it made them (0x1A44EB).
pub fn remake_tables(ctx: &Context<'_>) {
    let targets = ctx.reg(Reg::Ecx);
    // SAFETY: the site's ESI is the frame's scene view (execute loaded it
    // from the context's +4170h at 0x1A42EB), read and written only by this
    // stage on the render thread; the replayed cmp reads the field next.
    unsafe {
        let view = raw::site::atmosphere_tables_due::view(ctx).expect("the frame's scene view");
        view.atmosphere_targets().write(!targets);
    }
}

/// Sets the technique the sky's pass applies (`atmosphere.sky_apply`; the
/// caller takes its `applied` exit, past the game's write of 1).
pub fn set_sky_technique(ctx: &Context<'_>, technique: Technique) {
    // SAFETY: as `set_fog_technique`, at this site.
    unsafe {
        let effect = raw::site::atmosphere_sky_apply::stage(ctx)
            .and_then(|stage| stage.scattering_effect())
            .expect("the stage's scattering effect");
        effect.technique().write(technique.0);
    }
}

/// The render target the geometry fog draws into (renderer_context::get_rt
/// 0x11C1D0 with 37h, as execute takes it at 0x1A5123).
const FOG_TARGET: u32 = 0x37;

/// `atmosphere.geometry_fog`: the stage's effect applies `technique`
/// instead of 5 (the site's `applied` exit skips execute's own write).
pub fn set_fog_technique(ctx: &Context<'_>, technique: Technique) {
    // SAFETY: the site's EBP is the stage executing (its execute's frame
    // pointer), on the render thread; +50h its scattering effect, which the
    // constructor made; +5620h the technique apply_pass reads next.
    unsafe {
        let effect = raw::site::atmosphere_geometry_fog::stage(ctx)
            .and_then(|stage| stage.scattering_effect())
            .expect("the stage's scattering effect");
        effect.technique().write(technique.0);
    }
}

/// The constant 0.11e's fog passes read as inscatter_parameters: .x the
/// weather's inscatter distance (both passes read .x alone).
fn inscatter(distance: f32) -> [f32; 4] {
    [distance, 1.0, 0.0, 0.0]
}

fn backend() -> raw::Backend {
    // SAFETY: the render backend's global, set once at the render system's
    // start, before any stage executes.
    unsafe { raw::image::backend_global().backend() }.expect("the render backend")
}

/// `atmosphere.geometry_fog_applied`: the multiplicative pass's program is
/// bound; its inscatter_parameters.
pub fn set_multiply_constants(ctx: &Context<'_>, distance: f32) {
    let value = inscatter(distance);
    // SAFETY: the site's stage (above); its inscatter host the constructor
    // made; set_ps_constant writes the bound pixel shader's constant and
    // reads `value` during the call only.
    unsafe {
        let stage = raw::site::atmosphere_geometry_fog_applied::stage(ctx).expect("the stage executing");
        raw::engine_set_ps_constant(Some(backend()), stage.inscatter_constant(), value.as_ptr());
    }
}

/// At the god rays' entry in the geometry stage, after the multiplicative
/// pass: the additive pass, as execute draws a fog pass (0x1A50FA..0x1A512E:
/// apply_pass, the pass's constants, target 37h, fill_surfaces), with
/// to_sun_direction_parameter as execute makes it for the lookup tables
/// (0x1A4276..0x1A42B1: minus the environment's sun direction, .w its sun
/// intensity), inscatter_parameters and s_eye_ray_corner.
pub fn draw_fog_add(ctx: &Context<'_>, distance: f32) {
    // SAFETY: the site's ESP is the god-rays pass's entry, its argument the
    // stage executing (execute pushes it at 0x1A5133, alive for the call);
    // the stage's effect, hosts and context are its constructor's; the
    // context's scene view is the frame's (renderer::render set it); every
    // engine call runs on this, the render thread, as execute's own do, and
    // the target fill_surfaces takes is the reference get_rt made (it
    // releases it, as execute's).
    unsafe {
        let stage = raw::site::atmosphere_god_rays::frame(ctx)
            .and_then(|frame| frame.stage())
            .expect("the stage the god rays draw for");
        let effect = stage.scattering_effect().expect("the stage's scattering effect");
        let context = stage.context().expect("the stage's renderer context");
        let view = context.scene_view().expect("the frame's scene view");
        effect.technique().write(FOG_ADD.0);
        raw::engine_apply_effect_pass(Some(effect));
        let environment = view.environment();
        let mut direction = [0.0f32; 3];
        raw::engine_sun_direction(Some(environment), direction.as_mut_ptr());
        // 0.11e's geometry passes give .w 1 (0x60E1C2), not the sun's intensity (the sky's, 0x60D68D).
        let to_sun = [-direction[0], -direction[1], -direction[2], 1.0];
        let backend = backend();
        raw::engine_set_ps_constant(Some(backend), stage.to_sun_constant(), to_sun.as_ptr());
        let scattering = inscatter(distance);
        raw::engine_set_ps_constant(Some(backend), stage.inscatter_constant(), scattering.as_ptr());
        let eye_ray = [context.eye_ray_x().read(), context.eye_ray_y().read(), context.eye_ray_z().read(), 0.0];
        raw::engine_set_ps_constant(Some(backend), stage.eye_ray_constant(), eye_ray.as_ptr());
        let mut target: *mut core::ffi::c_void = core::ptr::null_mut();
        raw::engine_render_target(Some(context), FOG_TARGET, &mut target);
        raw::engine_fill_atmosphere(Some(stage), target, core::ptr::null_mut(), 1);
    }
}

#[cfg(test)]
impl RenderSceneId {
    pub fn for_test(address: u32) -> RenderSceneId {
        RenderSceneId(NonZeroU32::new(address).expect("a test id is not 0"))
    }
}
