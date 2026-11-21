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
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::{Context, Reg};
use super::sync::Lock;
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

/// probe_diffuse_mult's constant host (patch 169): 0.44a5's particle
/// lighting multiplies a probe's diffuse light by it, which 0.26g's stage
/// registers no host for.
struct ProbeMultiplier(raw::ConstantHost);

// SAFETY: a constant host lives as long as the backend that made it, the
// process; only its address crosses from the constructor's thread to the
// render thread, which alone writes through it (set_ps_constant).
unsafe impl Send for ProbeMultiplier {}

static PROBE_MULTIPLIER: Lock<Option<ProbeMultiplier>> = Lock::new(None);

const PROBE_MULTIPLIER_NAME: &core::ffi::CStr = c"probe_diffuse_mult";

/// Registers probe_diffuse_mult's host at `stage_lights.constructed`, with
/// the game's own sequence for its other hosts (0x18FF3C..0x18FF6B): the
/// name's shared string in a holder, register_constant_host, the holder let
/// go. The backend returns the host already made for a name, so a second
/// stage_lights gets the same.
pub fn register_probe_multiplier(ctx: &Context<'_>) {
    // SAFETY: stage_lights' constructor runs where the game registers its
    // own hosts, the backend made ([2260490h]); the string's count is the
    // game's own reference counting (lock xadd, destroy at 0, 0x190140).
    let host = unsafe {
        let Some(backend) = raw::image::backend_global().backend() else { return };
        let string = raw::engine_shared_string_create(PROBE_MULTIPLIER_NAME.as_ptr().cast());
        let mut holder = None;
        raw::engine_shared_string_assign(&mut holder, string);
        let host = raw::engine_register_constant_host(Some(backend), &mut holder, 0);
        if let Some(held) = holder
            && AtomicU32::from_ptr(held.references()).fetch_sub(1, Ordering::AcqRel) == 1
        {
            raw::engine_shared_string_destroy(Some(held));
        }
        host
    };
    if let Some(host) = host {
        PROBE_MULTIPLIER.with(ctx.thread(), |slot| *slot = Some(ProbeMultiplier(host)));
    }
}

/// Sets probe_diffuse_mult at `stage_lights.particle_probe` as 0.44a5's
/// particle pass does (0x47BF20..0x47BF36): the batch's probe's diffuse
/// multiplier times the environment's probes diffuse intensity, 0 without
/// a probe. Nothing until the constructor registered its host.
pub fn set_probe_multiplier(ctx: &Context<'_>) {
    let Some(host) = PROBE_MULTIPLIER.with(ctx.thread(), |slot| slot.as_ref().map(|m| m.0)) else { return };
    use raw::site::stage_lights_particle_probe as site;
    // SAFETY: the site's registers: ESI the stage, its context's scene
    // view the frame's (alive for the frame), EBX the backend, [EBP-24h]
    // the batch's probe or 0 (alive for the pass); set_ps_constant on the
    // render thread with the pass's program bound, as the game's own sets
    // before and after this site.
    unsafe {
        let (Some(stage), Some(backend), Some(frame)) = (site::stage(ctx), site::backend(ctx), site::frame(ctx)) else { return };
        let Some(view) = stage.context().and_then(|context| context.scene_view()) else { return };
        let value = match frame.probe() {
            Some(probe) => probe.diffuse_multiplier().read() * view.probes_diffuse_intensity().read(),
            None => 0.0,
        };
        let buffer = [value, 0.0, 0.0, 0.0];
        raw::engine_set_ps_constant(Some(host), Some(backend), buffer.as_ptr());
    }
}

#[cfg(test)]
impl RenderSceneId {
    pub fn for_test(address: u32) -> RenderSceneId {
        RenderSceneId(NonZeroU32::new(address).expect("a test id is not 0"))
    }
}
