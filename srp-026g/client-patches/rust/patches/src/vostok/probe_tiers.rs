//! The environment probes' draw and their four box effects' compiles, for
//! patch 203 (docs/notes/probe-tiers-069d.md): what a probe of the loop is,
//! the loop's place in the sorted list, the bound render targets, and a
//! technique's pixel shader, blend and extra texture while it compiles. Every
//! field is fields_026g.tsv's, every site sites_026g.tsv's.
//!
//! - `env_probes.next`, `env_probes.technique`, `env_probes.done`:
//!   stage_ambient_lighting::render_environment_probes (0x180980) on the
//!   render thread, EBP its frame (the stage at +8), ESP the loop's locals;
//! - `probe_lighting_<gsba>.*`: effect_environment_probe_lighting<1,0,b,a>
//!   ::compile (0x18A720, 0x18B770, 0x18C7C0, 0x18D810), one code at the
//!   same offsets but for their configuration bytes, EBX the compiler.

use core::ffi::CStr;
use core::num::NonZeroU32;

use super::context::{Context, Reg};
use super::effect_compile::CompilerId;
use crate::raw;

/// One of the four box effects without smart attenuation, by
/// effect_environment_probe_lighting<geometry 1, smart 0, blend, affect_diffuse>.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeEffect {
    /// <1,0,0,0>, index 8.
    P1000,
    /// <1,0,1,0>, index 10.
    P1010,
    /// <1,0,0,1>, index 9.
    P1001,
    /// <1,0,1,1>, index 11.
    P1011,
}

impl ProbeEffect {
    /// The effect at index `index` of the stage's sixteen (8 x geometry +
    /// 4 x smart_attenuation + 2 x blend + affect_diffuse), if it is one of
    /// the four.
    pub fn of_index(index: u32) -> Option<ProbeEffect> {
        match index {
            8 => Some(ProbeEffect::P1000),
            9 => Some(ProbeEffect::P1001),
            10 => Some(ProbeEffect::P1010),
            11 => Some(ProbeEffect::P1011),
            _ => None,
        }
    }
}

/// The compile sites of each of the four effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileSite {
    /// `.begin_pass0` / `.begin_pass1`: a technique's begin_pass arguments.
    Pass(ProbeEffect, Side),
    /// `.blend0` / `.blend1`: after a technique's set_alpha_blend.
    Blend(ProbeEffect, Side),
    /// `.texture0` / `.texture1`: before a technique's set_texture blocks.
    Texture(ProbeEffect, Side),
    /// `.compiled`: the compile's epilogue.
    End(ProbeEffect),
}

/// Technique 0 (front faces) or 1 (back faces) of a compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
}

macro_rules! by_effect {
    ($effect:expr, $site:ident, $f:ident, $ctx:expr) => {
        paste_effects!($effect, $f, $ctx, $site)
    };
}

macro_rules! paste_effects {
    ($effect:expr, $f:ident, $ctx:expr, begin_pass0) => {
        each_effect!($effect, $f, $ctx, probe_lighting_1000_begin_pass0, probe_lighting_1010_begin_pass0, probe_lighting_1001_begin_pass0, probe_lighting_1011_begin_pass0)
    };
    ($effect:expr, $f:ident, $ctx:expr, begin_pass1) => {
        each_effect!($effect, $f, $ctx, probe_lighting_1000_begin_pass1, probe_lighting_1010_begin_pass1, probe_lighting_1001_begin_pass1, probe_lighting_1011_begin_pass1)
    };
    ($effect:expr, $f:ident, $ctx:expr, blend0) => {
        each_effect!($effect, $f, $ctx, probe_lighting_1000_blend0, probe_lighting_1010_blend0, probe_lighting_1001_blend0, probe_lighting_1011_blend0)
    };
    ($effect:expr, $f:ident, $ctx:expr, blend1) => {
        each_effect!($effect, $f, $ctx, probe_lighting_1000_blend1, probe_lighting_1010_blend1, probe_lighting_1001_blend1, probe_lighting_1011_blend1)
    };
    ($effect:expr, $f:ident, $ctx:expr, texture0) => {
        each_effect!($effect, $f, $ctx, probe_lighting_1000_texture0, probe_lighting_1010_texture0, probe_lighting_1001_texture0, probe_lighting_1011_texture0)
    };
    ($effect:expr, $f:ident, $ctx:expr, texture1) => {
        each_effect!($effect, $f, $ctx, probe_lighting_1000_texture1, probe_lighting_1010_texture1, probe_lighting_1001_texture1, probe_lighting_1011_texture1)
    };
    ($effect:expr, $f:ident, $ctx:expr, compiled) => {
        each_effect!($effect, $f, $ctx, probe_lighting_1000_compiled, probe_lighting_1010_compiled, probe_lighting_1001_compiled, probe_lighting_1011_compiled)
    };
}

macro_rules! each_effect {
    ($effect:expr, $f:ident, $ctx:expr, $a:ident, $b:ident, $c:ident, $d:ident) => {
        match $effect {
            ProbeEffect::P1000 => raw::site::$a::$f($ctx),
            ProbeEffect::P1010 => raw::site::$b::$f($ctx),
            ProbeEffect::P1001 => raw::site::$c::$f($ctx),
            ProbeEffect::P1011 => raw::site::$d::$f($ctx),
        }
    };
}

fn compiler(ctx: &Context<'_>, site: CompileSite) -> raw::EffectCompiler {
    // SAFETY: the context is the site's own (each handler is its site's);
    // EBX is the compile's effect_compiler there (the compile loads it from
    // its first argument, 0x18A72A), alive while its compile runs.
    unsafe {
        match site {
            CompileSite::Pass(effect, Side::Front) => by_effect!(effect, begin_pass0, compiler, ctx),
            CompileSite::Pass(effect, Side::Back) => by_effect!(effect, begin_pass1, compiler, ctx),
            CompileSite::Blend(effect, Side::Front) => by_effect!(effect, blend0, compiler, ctx),
            CompileSite::Blend(effect, Side::Back) => by_effect!(effect, blend1, compiler, ctx),
            CompileSite::Texture(effect, Side::Front) => by_effect!(effect, texture0, compiler, ctx),
            CompileSite::Texture(effect, Side::Back) => by_effect!(effect, texture1, compiler, ctx),
            CompileSite::End(effect) => by_effect!(effect, compiled, compiler, ctx),
        }
    }
    .expect("the compile's compiler")
}

/// The compiler of the compile at `site`.
pub fn compiler_id(ctx: &Context<'_>, site: CompileSite) -> CompilerId {
    CompilerId::of(NonZeroU32::new(compiler(ctx, site).address()).expect("a handle is never 0"))
}

/// Makes `name` the pixel shader of the begin_pass at a `Pass` site (its
/// pushed argument; the VS `light` and the configuration stay).
pub fn set_pixel_shader(ctx: &Context<'_>, effect: ProbeEffect, side: Side, name: &'static CStr) {
    // SAFETY: the site's ESP: the PS name begin_pass's caller pushed (+7Eh
    // and +25Dh of each compile), which only begin_pass reads.
    let frame = unsafe {
        match side {
            Side::Front => by_effect!(effect, begin_pass0, frame, ctx),
            Side::Back => by_effect!(effect, begin_pass1, frame, ctx),
        }
    };
    // SAFETY: as above; the name is static, kept by name by the registries.
    unsafe { frame.expect("the site's ESP").set_pixel_shader(name.as_ptr().cast_mut().cast()) }
}

/// A blend's colour factors and operation (D3D11_BLEND, D3D11_BLEND_OP).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blend {
    pub source: u32,
    pub destination: u32,
    pub operation: u32,
}

impl Blend {
    pub const ZERO: u32 = 1;
    pub const ONE: u32 = 2;
    pub const SOURCE_ALPHA: u32 = 5;
    pub const ADD: u32 = 1;
    pub const REVERSE_SUBTRACT: u32 = 3;
}

/// Sets the colour blend of all eight targets of the technique at a `Blend`
/// site, after the game's set_alpha_blend (blending on, alpha as it set it).
pub fn set_blend(ctx: &Context<'_>, effect: ProbeEffect, side: Side, blend: Blend) {
    let compiler = compiler(ctx, CompileSite::Blend(effect, side));
    // SAFETY: the compiler's state descriptor of the pass being built,
    // eight records (set_alpha_blend 0x3E2EA0 writes the same eight), read
    // at end_pass; the compile's own thread.
    unsafe {
        let first = compiler.blend();
        for i in 0..8 {
            let record = first.at(i);
            record.source().write_volatile(blend.source);
            record.destination().write_volatile(blend.destination);
            record.operation().write_volatile(blend.operation);
        }
    }
}

/// Binds `texture` (a render target's name) under `slot` in the technique
/// at a `Texture` site: the game's set_texture (0x3D98B0) as the technique's
/// own blocks call it (ESI 0, -1, ESI, 1.0, flag 0).
pub fn bind_texture(ctx: &Context<'_>, effect: ProbeEffect, side: Side, slot: &'static CStr, texture: &'static CStr) {
    let compiler = compiler(ctx, CompileSite::Texture(effect, side));
    // SAFETY: set_texture (stdcall, its eight arguments on the stack), on the
    // compile's own thread and compiler, between the pass's begin_pass and
    // end_pass; it reads the two static names and keeps them by name.
    unsafe { raw::engine_set_texture(Some(compiler), slot.as_ptr().cast(), texture.as_ptr().cast(), 0, u32::MAX, 0, 1.0, 0) }
}

/// The three sites of the probe loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopSite {
    Next,
    Technique,
    Done,
}

fn frame(ctx: &Context<'_>, site: LoopSite) -> raw::ProbeLoopFrame {
    // SAFETY: the context is the site's; ESP is the loop's frame there (the
    // pushes of every call inside the loop are popped again).
    unsafe {
        match site {
            LoopSite::Next => raw::site::env_probes_next::frame(ctx),
            LoopSite::Technique => raw::site::env_probes_technique::frame(ctx),
            LoopSite::Done => raw::site::env_probes_done::frame(ctx),
        }
    }
    .expect("the loop's frame")
}

fn stage(ctx: &Context<'_>, site: LoopSite) -> raw::StageAmbientLighting {
    // SAFETY: EBP is render_environment_probes's frame at its loop's sites,
    // its argument (+8) the stage executing, alive for the execute.
    unsafe {
        let draw = match site {
            LoopSite::Next => raw::site::env_probes_next::stage(ctx),
            LoopSite::Technique => raw::site::env_probes_technique::stage(ctx),
            LoopSite::Done => raw::site::env_probes_done::stage(ctx),
        };
        draw.expect("the probes' frame").stage()
    }
    .expect("the stage")
}

/// The probe a loop site is at, as the draw needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopProbe {
    /// The frame's scene marker (its environment's mie_fog_base_height).
    pub marker: f32,
    /// Its number in the sorted list (probe_index, its place's).
    pub index: u32,
    /// Its radius (the sort key).
    pub radius: f32,
    /// The probe's address, for a message.
    pub address: u32,
}

/// The probe at `env_probes.next`.
pub fn probe_at_next(ctx: &Context<'_>) -> LoopProbe {
    let frame = frame(ctx, LoopSite::Next);
    // SAFETY: the loop's locals: its place in the scene view's sorted list
    // (alive for the frame) and that place's probe.
    unsafe {
        let place = frame.position().expect("the loop's place");
        let probe = place.probe().expect("a probe in the sorted list");
        let marker = frame.scene_view().expect("the frame's scene view").mie_fog_base_height().read_volatile();
        LoopProbe { marker, index: frame.probe_index().read_volatile(), radius: probe.radius().read_volatile(), address: probe.address() }
    }
}

/// The probe and its pass at `env_probes.technique`, with the effect's index.
pub fn technique_at(ctx: &Context<'_>) -> (LoopProbe, u32) {
    let frame = frame(ctx, LoopSite::Technique);
    // SAFETY: as `probe_at_next`; [esp+10h] the probe the loop draws.
    let probe = unsafe {
        let probe = frame.probe().expect("the probe drawn");
        let marker = frame.scene_view().expect("the frame's scene view").mie_fog_base_height().read_volatile();
        LoopProbe { marker, index: frame.probe_index().read_volatile(), radius: probe.radius().read_volatile(), address: probe.address() }
    };
    (probe, ctx.reg(Reg::Edx))
}

/// Sets the technique `env_probes.technique` writes (ECX).
pub fn set_technique(ctx: &mut Context<'_>, technique: u32) {
    ctx.set_reg(Reg::Ecx, technique);
}

/// The pass (ECX) at `env_probes.technique`.
pub fn pass_at(ctx: &Context<'_>) -> u32 {
    ctx.reg(Reg::Ecx)
}

/// Whether the probe `env_probes.done` is at is the list's last.
pub fn last_at_done(ctx: &Context<'_>) -> bool {
    let frame = frame(ctx, LoopSite::Done);
    // SAFETY: the loop's locals: its place and the list's end.
    unsafe {
        let place = frame.position().expect("the loop's place");
        let end = frame.end().expect("the list's end");
        place.at(1) == end
    }
}

/// At `env_probes.done`, before its `again` exit: the loop goes on at the
/// sorted list's probe number `index`.
pub fn rewind(ctx: &Context<'_>, index: u32) {
    let frame = frame(ctx, LoopSite::Done);
    // SAFETY: the loop's own locals and the frame's sorted list (alive for
    // the frame); `index` is a number the loop gave a probe of this list
    // (probe_index counts its places from 0); `again` (0x180B90) reads the
    // place.
    unsafe {
        let first = frame.scene_view().and_then(|v| v.sorted_probes()).expect("the sorted probes");
        frame.set_position(Some(first.at(index as usize)));
        frame.probe_index().write_volatile(index);
    }
}

/// The backend's four render-target slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Targets([u32; 4]);

fn backend() -> raw::Backend {
    // SAFETY: the render backend's global, made before any stage runs.
    unsafe { raw::image::backend_global().backend() }.expect("the render backend")
}

/// The targets bound now (render_environment_probes bound them before its loop).
pub fn bound_targets() -> Targets {
    let b = backend();
    // SAFETY: the backend's slots, the render thread's (a loop site's).
    unsafe {
        Targets([
            b.render_target0().read_volatile(),
            b.render_target1().read_volatile(),
            b.render_target2().read_volatile(),
            b.render_target3().read_volatile(),
        ])
    }
}

/// Binds `targets` as the probe loop binds its own (each slot's changed
/// byte set where it differs), for the next draw.
pub fn bind_targets(targets: Targets) {
    let b = backend();
    let [t0, t1, t2, t3] = targets.0;
    // SAFETY: as `bound_targets`; the next draw sets them on the device.
    unsafe {
        let slots = [
            (b.render_target0(), b.render_target0_changed()),
            (b.render_target1(), b.render_target1_changed()),
            (b.render_target2(), b.render_target2_changed()),
            (b.render_target3(), b.render_target3_changed()),
        ];
        for ((slot, changed), view) in slots.into_iter().zip([t0, t1, t2, t3]) {
            if slot.read_volatile() != view {
                slot.write_volatile(view);
                changed.write_volatile(1);
            }
        }
    }
}

/// `$user$accum_ambient_lights` (rt 32) alone, as stage_ambient_lighting's
/// clear_surfaces binds it.
pub fn ambient_lights_target(ctx: &Context<'_>, site: LoopSite) -> Targets {
    let stage = stage(ctx, site);
    // SAFETY: the stage's renderer context and its target table, the
    // renderer's for its life; rt 32 exists from create_targets on.
    let view = unsafe {
        let table = stage.context().and_then(|c| c.targets()).expect("the render targets");
        table.accum_ambient_lights().expect("accum_ambient_lights").view().read_volatile()
    };
    Targets([view, 0, 0, 0])
}

/// stage_ambient_lighting::clear_surfaces (0x180820): rt 32 cleared to 1 and
/// rt 33 to 0, rt 33 left bound.
pub fn clear_ambient_surfaces(ctx: &Context<'_>, site: LoopSite) {
    let stage = stage(ctx, site);
    // SAFETY: the stage's own clear (thiscall, ECX the stage), on the render
    // thread inside its execute, as the renderer calls it.
    unsafe { raw::engine_ambient_clear_surfaces(Some(stage)) }
}

#[cfg(test)]
impl Targets {
    pub fn for_test(views: [u32; 4]) -> Targets {
        Targets(views)
    }
}
