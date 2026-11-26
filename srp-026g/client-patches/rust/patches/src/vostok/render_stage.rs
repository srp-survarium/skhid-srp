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
//!   argument the stage_atmosphere;
//! - `composition.technique`, `composition.constants`:
//!   stage_composition::execute (0x1A8170), EBP the stage (patch 178).
//!
//! A stage's renderer context (+4) names the frame's scene (+416Ch, which
//! renderer::render 0x13D010 stores each frame before the stages run) and its
//! scene view (+4170h); both stay alive for the frame.

use core::num::NonZeroU32;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::{Context, Reg};
use super::log::{self, Piece::Text};
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
    /// `composition.technique`.
    CompositionTechnique,
    /// `composition.constants`.
    CompositionConstants,
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
            StageSite::CompositionTechnique => raw::site::composition_technique::stage(ctx)?.context(),
            StageSite::CompositionConstants => raw::site::composition_constants::stage(ctx)?.context(),
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

/// Makes `technique` the one apply_pass applies of the composition effect
/// stage_composition::execute chose (`composition.technique`: EAX, its
/// res_effect), in the place of the game's 0. The caller takes the site's
/// `apply` exit, past the game's write.
pub fn set_composition_technique(ctx: &Context<'_>, technique: Technique) {
    // SAFETY: the site's EAX is the effect the stage holds (one of its
    // +10h..+2Ch, made by its constructor, alive with the stage); +5620h is
    // the technique the next apply_pass reads, written here as the game does.
    unsafe {
        let effect = raw::site::composition_technique::effect(ctx).expect("the composition effect");
        effect.technique().write(technique.0);
    }
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

/// The four constants 0.44a5's composition programs read that 0.26g's
/// stage_composition registers no host for (patch 178): 0.44a5's own
/// stage_composition registers them (0x48DF10: +54h, +58h, +5Ch, +78h).
pub struct CompositionLighting {
    /// direct_translucency_color times direct_translucency_intensity, w 1
    /// (0.44a5's execute 0x48F883..0x48F8CB).
    pub direct_translucency_color: [f32; 3],
    pub sun_dir_vs_normal_factor: f32,
    pub translucency_power: f32,
    /// The diffuse factor of a surface sheltered from the rain (0.26g's
    /// programs: the constant 0.85).
    pub rain_shadow_diffuse_multiplier: f32,
}

struct CompositionHosts([raw::ConstantHost; 4]);

// SAFETY: as ProbeMultiplier's: hosts live as long as the backend, the
// process; only the render thread writes through them.
unsafe impl Send for CompositionHosts {}

static COMPOSITION_HOSTS: Lock<Option<CompositionHosts>> = Lock::new(None);

const COMPOSITION_HOST_NAMES: [&core::ffi::CStr; 4] =
    [c"direct_translucency_color", c"sun_dir_vs_normal_factor", c"translucency_power", c"rain_shadow_diffuse_multiplier"];

/// Registers a constant host by name with the game's own sequence (see
/// `register_probe_multiplier`); the backend returns the host already made
/// for a name.
fn register_host(name: &core::ffi::CStr) -> Option<raw::ConstantHost> {
    // SAFETY: called where a stage's constructor registers its own hosts, the
    // backend made ([2260490h]); the string's count is the game's own
    // reference counting (lock xadd, destroy at 0).
    unsafe {
        let backend = raw::image::backend_global().backend()?;
        let string = raw::engine_shared_string_create(name.as_ptr().cast());
        let mut holder = None;
        raw::engine_shared_string_assign(&mut holder, string);
        let host = raw::engine_register_constant_host(Some(backend), &mut holder, 0);
        if let Some(held) = holder
            && AtomicU32::from_ptr(held.references()).fetch_sub(1, Ordering::AcqRel) == 1
        {
            raw::engine_shared_string_destroy(Some(held));
        }
        host
    }
}

struct BakeCoordinates(raw::ConstantHost);

// SAFETY: the backend owns the host for its lifetime. Only the render thread
// writes its value, after applying the material for the current surface.
unsafe impl Send for BakeCoordinates {}

static BAKE_COORDINATES: Lock<Option<BakeCoordinates>> = Lock::new(None);

/// Registers `srp_weapon_bake_channels` at `gbuffer.bake_host`. With no host,
/// every surface draws with its material coordinates (a baked weapon's decal
/// and camouflage in the wrong place), so the stage says so once.
pub fn register_bake_coordinates(ctx: &Context<'_>) {
    let host = register_host(c"srp_weapon_bake_channels").map(BakeCoordinates);
    if host.is_none() {
        log::line(&[Text("weapon decals: no host srp_weapon_bake_channels: baked weapons drawn in material UVs")]);
    }
    BAKE_COORDINATES.with(ctx.thread(), |slot| *slot = host);
}

/// At `gbuffer.bake_coordinates`: one flag a channel (diffuse, normal,
/// fresnel, smoothness), 1 where the surface's override slot holds a bake.
pub fn set_bake_coordinates(ctx: &Context<'_>) {
    use raw::site::gbuffer_bake_coordinates as site;
    let Some(host) = BAKE_COORDINATES.with(ctx.thread(), |slot| slot.as_ref().map(|h| h.0)) else { return };
    // SAFETY: render_models has a live instance (EDX) and backend (EDI)
    // across this draw. Its four override slots, in D/N/F/S order, are the
    // ones it just used for t_base, t_normal, t_fresnel and t_roughness; the
    // value is the 16 bytes a host's binding reads.
    unsafe {
        let (Some(surface), Some(backend)) = (site::surface(ctx), site::backend(ctx)) else { return };
        let channels: [f32; 4] =
            core::array::from_fn(|i| if surface.textures().at(i).texture().is_some() { 1.0 } else { 0.0 });
        raw::engine_set_ps_constant(Some(host), Some(backend), channels.as_ptr());
    }
}

/// Registers the four hosts at `composition.constructed`.
pub fn register_composition_lighting(ctx: &Context<'_>) {
    let [a, b, c, d] = COMPOSITION_HOST_NAMES.map(register_host);
    if let (Some(a), Some(b), Some(c), Some(d)) = (a, b, c, d) {
        COMPOSITION_HOSTS.with(ctx.thread(), |slot| *slot = Some(CompositionHosts([a, b, c, d])));
    }
}

/// Sets the four at `composition.constants`, the composition's program
/// bound: set_ps_constant with the site's backend, as the game sets its own
/// after it. Nothing until the constructor registered the hosts.
pub fn set_composition_lighting(ctx: &Context<'_>, lighting: &CompositionLighting) {
    let Some(hosts) = COMPOSITION_HOSTS.with(ctx.thread(), |slot| slot.as_ref().map(|hosts| hosts.0)) else { return };
    let [r, g, b] = lighting.direct_translucency_color;
    let values = [
        [r, g, b, 1.0],
        [lighting.sun_dir_vs_normal_factor, 0.0, 0.0, 0.0],
        [lighting.translucency_power, 0.0, 0.0, 0.0],
        [lighting.rain_shadow_diffuse_multiplier, 0.0, 0.0, 0.0],
    ];
    // SAFETY: the site's EDI is the backend execute set its own constants
    // with ([2260490h], 0x1A85C7), on the render thread, the composition's
    // program bound by the apply_pass before it; each value is 16 bytes, at
    // least what a host's binding reads.
    unsafe {
        let Some(backend) = raw::site::composition_constants::backend(ctx) else { return };
        for (host, value) in hosts.into_iter().zip(values.iter()) {
            raw::engine_set_ps_constant(Some(host), Some(backend), value.as_ptr());
        }
    }
}
