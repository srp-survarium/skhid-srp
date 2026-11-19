//! Effect compiles (patch 130, docs/notes/client_patch.md section 3.82): the
//! compiler a technique block runs with, the names a pass is made of, and a
//! material effect's config. Every field is fields_026g.tsv's, every site
//! sites_026g.tsv's.
//!
//! An effect compiles twice (0.10b effect_cook.cpp): once caching (the
//! compiler's +61F4Ch set: begin_pass only records the shaders it names), then
//! building, with the same inputs, on whichever resource thread cooks it. A
//! name given to begin_pass or set_texture is read there and kept by name
//! (the resource registries are keyed by it): one of our static strings
//! lives as long as the arena, the process.

use core::ffi::CStr;
use core::num::NonZeroU32;

use super::config::{ConfigTable, ConfigValue};
use super::context::{Context, Reg};
use crate::raw;

/// An effect_compiler as an id: compared, never read through. One compiles
/// one effect at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompilerId(NonZeroU32);

impl CompilerId {}

/// A compiler between an effect's end and its second end: compiling the
/// twins `T` names (patches 130 and 162: an effect compiles its techniques
/// again, with a name changed, after its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Round<T> {
    compiler: CompilerId,
    twins: T,
}

/// What a compile does at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Compile the twins: back to the techniques' start.
    Twins,
    /// Return, the twins compiled.
    Return,
}

/// Compiles at once: one per resource thread at most; past this many the
/// game stops (a round lost would leave an effect without its twin).
pub const ROUNDS: usize = 64;

/// The compilers compiling twins.
pub struct Rounds<T>([Option<Round<T>>; ROUNDS]);

impl<T: Copy + Eq + core::fmt::Debug> Default for Rounds<T> {
    fn default() -> Rounds<T> {
        Rounds::new()
    }
}

impl<T: Copy + Eq + core::fmt::Debug> Rounds<T> {
    pub const fn new() -> Rounds<T> {
        Rounds([None; ROUNDS])
    }

    fn find(&self, compiler: CompilerId) -> Option<(usize, Round<T>)> {
        self.0.iter().enumerate().find_map(|(i, slot)| slot.filter(|round| round.compiler == compiler).map(|round| (i, round)))
    }

    /// The compile of `twins`'s effect by `compiler` is at its end.
    pub fn end(&mut self, compiler: CompilerId, twins: T) -> End {
        match self.find(compiler) {
            None => {
                let free = self.0.iter_mut().find(|slot| slot.is_none()).expect("fewer compiles at once than ROUNDS");
                *free = Some(Round { compiler, twins });
                End::Twins
            }
            Some((i, round)) => {
                assert_eq!(round.twins, twins, "a compiler compiles one effect at a time");
                self.0[i] = None;
                End::Return
            }
        }
    }

    /// The twins `compiler` compiles now, if any.
    pub fn twins(&self, compiler: CompilerId) -> Option<T> {
        self.find(compiler).map(|(_, round)| round.twins)
    }
}

/// The compile sites whose EBX is the compiler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileSite {
    /// `wet_surface.compiled`: effect_wet_surface::compile's epilogue.
    WetSurfaceEnd,
    /// `wet_surface.begin_pass`: its technique 0's begin_pass.
    WetSurfacePass,
    /// `rain.compiled`: effect_rain::compile's epilogue.
    RainEnd,
    /// `rain.drop_texture`: its technique 1's t_rain_drop.
    RainDropTexture,
    /// `water.normal_texture`: the forward water's t_normal_map.
    WaterTexture,
    /// `scattering.compiled`: effect_atmospheric_scattering::compile's epilogue.
    ScatteringEnd,
    /// `scattering.tables_pass`: its technique 0's begin_pass.
    ScatteringTablesPass,
    /// `scattering.sky_pass`: its technique 1's begin_pass.
    ScatteringSkyPass,
    /// `post_blend_<ab>.compiled`: effect_complex_post_process_blend<a,b>::compile's epilogue.
    PostBlend00End,
    PostBlend10End,
    PostBlend11End,
    /// `post_blend_<ab>.pass`: its one technique's begin_pass.
    PostBlend00Pass,
    PostBlend10Pass,
    PostBlend11Pass,
}

fn compiler(ctx: &Context<'_>, site: CompileSite) -> raw::EffectCompiler {
    // SAFETY: the context is the site's own; EBX (ESI in the post blends'
    // compiles) is the compile's effect_compiler there (each compile loads it from its first argument,
    // 0x1CE10A, 0x1D3B9A), alive while its compile runs.
    unsafe {
        match site {
            CompileSite::WetSurfaceEnd => raw::site::wet_surface_compiled::compiler(ctx),
            CompileSite::WetSurfacePass => raw::site::wet_surface_begin_pass::compiler(ctx),
            CompileSite::RainEnd => raw::site::rain_compiled::compiler(ctx),
            CompileSite::RainDropTexture => raw::site::rain_drop_texture::compiler(ctx),
            CompileSite::WaterTexture => raw::site::water_normal_texture::compiler(ctx),
            CompileSite::ScatteringEnd => raw::site::scattering_compiled::compiler(ctx),
            CompileSite::ScatteringTablesPass => raw::site::scattering_tables_pass::compiler(ctx),
            CompileSite::ScatteringSkyPass => raw::site::scattering_sky_pass::compiler(ctx),
            CompileSite::PostBlend00End => raw::site::post_blend_00_compiled::compiler(ctx),
            CompileSite::PostBlend10End => raw::site::post_blend_10_compiled::compiler(ctx),
            CompileSite::PostBlend11End => raw::site::post_blend_11_compiled::compiler(ctx),
            CompileSite::PostBlend00Pass => raw::site::post_blend_00_pass::compiler(ctx),
            CompileSite::PostBlend10Pass => raw::site::post_blend_10_pass::compiler(ctx),
            CompileSite::PostBlend11Pass => raw::site::post_blend_11_pass::compiler(ctx),
        }
    }
    .expect("the compile's compiler")
}

/// The compiler of the compile at `site`.
pub fn compiler_id(ctx: &Context<'_>, site: CompileSite) -> CompilerId {
    CompilerId(NonZeroU32::new(compiler(ctx, site).address()).expect("a handle is never 0"))
}

/// Which of its two compiles an effect is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Recording the shaders its passes name (begin_technique and the
    /// states skipped).
    Caching,
    /// Making the techniques.
    Building,
}

/// The compile pass of the compiler at `site`.
pub fn pass(ctx: &Context<'_>, site: CompileSite) -> Pass {
    // SAFETY: the compiler's cache byte (see `compiler`), the one every
    // technique block tests.
    if unsafe { compiler(ctx, site).caching().read() } == 0 { Pass::Building } else { Pass::Caching }
}

/// effect_rain's technique 1 starts with ESI 0 (0x1D3D34, its
/// configuration's zeros 0x1D3D44..0x1D3D5A); its epilogue has ESI as
/// set_alpha_blend's setup left it (1). Called before `rain.compiled`'s
/// `technique` exit.
pub fn enter_rain_technique(ctx: &mut Context<'_>) {
    ctx.set_reg(Reg::Esi, 0);
}

/// The begin_pass sites a patch names its own pixel shader at: each is
/// while the pass's arguments are pushed, ESP the PS name's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassSite {
    /// `wet_surface.begin_pass`: effect_wet_surface's technique 0.
    WetSurface,
    /// `scattering.tables_pass`: effect_atmospheric_scattering's technique 0.
    ScatteringTables,
    /// `scattering.sky_pass`: its technique 1.
    ScatteringSky,
    /// `post_blend_<ab>.pass`: effect_complex_post_process_blend<a,b>'s technique.
    PostBlend00,
    PostBlend10,
    PostBlend11,
}

/// Makes `name` the pixel shader of the begin_pass at `site` (its pushed
/// argument; the VS and the configuration stay).
pub fn set_pixel_shader(ctx: &Context<'_>, site: PassSite, name: &'static CStr) {
    // SAFETY: the site's ESP: the PS name begin_pass's caller pushed (its
    // third argument once the gs name and the compiler follow), which only
    // begin_pass reads.
    unsafe {
        let frame = match site {
            PassSite::WetSurface => raw::site::wet_surface_begin_pass::frame(ctx),
            PassSite::ScatteringTables => raw::site::scattering_tables_pass::frame(ctx),
            PassSite::ScatteringSky => raw::site::scattering_sky_pass::frame(ctx),
            PassSite::PostBlend00 => raw::site::post_blend_00_pass::frame(ctx),
            PassSite::PostBlend10 => raw::site::post_blend_10_pass::frame(ctx),
            PassSite::PostBlend11 => raw::site::post_blend_11_pass::frame(ctx),
        };
        frame.expect("the site's ESP").set_pixel_shader(name.as_ptr().cast_mut().cast())
    }
}

/// The set_texture blocks a patch binds its own texture at instead: each
/// site is before the block's pushes, its exit `bound` after its call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureSite {
    /// `rain.drop_texture`: effect_rain's t_rain_drop, engine/rain_drop.
    RainDrop,
    /// `water.normal_texture`: the forward water's t_normal_map, engine/water_waves2.
    WaterWaves,
}

/// Binds `texture` where the block at `site` binds its own: the game's
/// set_texture (0x3D98B0) with the block's arguments (its pushes: 1, 1.0,
/// ESI, -1, ESI, the texture, the slot, the compiler), the texture ours.
/// The caller takes the site's `bound` exit, which skips the block (its
/// fld1 and its pushes the call pops: the stack and the x87 as after it).
pub fn bind_texture(ctx: &Context<'_>, site: TextureSite, texture: &'static CStr) {
    let (compiler, slot) = match site {
        TextureSite::RainDrop => (compiler(ctx, CompileSite::RainDropTexture), c"t_rain_drop"),
        TextureSite::WaterWaves => (compiler(ctx, CompileSite::WaterTexture), c"t_normal_map"),
    };
    let esi = ctx.reg(Reg::Esi);
    // SAFETY: set_texture (stdcall, its eight arguments on the stack), on the
    // compile's own thread and compiler, in the place of the game's call; it
    // reads the two names (ours, static) and keeps them by name.
    unsafe {
        raw::engine_set_texture(Some(compiler), slot.as_ptr().cast(), texture.as_ptr().cast(), esi, u32::MAX, esi, 1.0, 1)
    }
}

/// The forward water's material config tree (`water.normal_texture`).
pub fn water_config<'c>(ctx: &'c Context<'_>) -> Option<ConfigTable<'c>> {
    // SAFETY: effect_fstage_simpe_water_materials::compile's EBP frame, its
    // config argument: the compile data's tree, immutable while it compiles.
    unsafe { ConfigValue::read(raw::site::water_normal_texture::water(ctx)?.config()?).ok()?.table().ok() }
}

/// A vertex input type (the shader configuration's byte 6, bits 0..5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VertexInput(u8);

impl VertexInput {
    /// grassmesh_vertex_input_type: the vegetation layers' instanced grass.
    pub const GRASSMESH: VertexInput = VertexInput(0x0B);

    pub fn parse(byte: u8) -> VertexInput {
        VertexInput(byte & 0x3F)
    }
}

/// A material effect's pass at compile_begin's begin_pass
/// (`material.begin_pass`).
pub struct MaterialPass<'c> {
    frame: raw::CompileBeginFrame,
    vertex_input: VertexInput,
    config: Option<ConfigTable<'c>>,
}

impl<'c> MaterialPass<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<MaterialPass<'c>> {
        use raw::site::material_begin_pass as site;
        // SAFETY: the site's EBX is compile_begin's shader configuration,
        // its byte 6 written at 0x1CD0CD; the ESP frame holds compile_begin's
        // arguments, the config tree the compile data's, immutable while the
        // effect compiles.
        unsafe {
            let frame = site::frame(ctx)?;
            let vertex_input = VertexInput::parse(site::config(ctx)?.vertex_input().read());
            let config = frame.config().and_then(|tree| ConfigValue::read(tree).ok()?.table().ok());
            Some(MaterialPass { frame, vertex_input, config })
        }
    }

    pub fn vertex_input(&self) -> VertexInput {
        self.vertex_input
    }

    /// The material effect's merged config (its <stage>.effect table), if a table.
    pub fn config(&self) -> Option<&ConfigTable<'c>> {
        self.config.as_ref()
    }

    /// The VS family name the pass is about to begin with.
    pub fn vertex_shader(&self) -> Option<&'c CStr> {
        // SAFETY: compile_begin's first argument, a family name the effect's
        // compile passes (a C string of the exe's, or one of ours).
        unsafe { super::ui::c_string(self.frame.vertex_shader(), 128) }
    }

    /// Makes `name` the VS family of this pass (the argument slot the
    /// replayed mov reads; compile_begin reads it no more after).
    pub fn set_vertex_shader(&mut self, name: &'static CStr) {
        // SAFETY: compile_begin's own argument slot (see `at`).
        unsafe { self.frame.set_vertex_shader(name.as_ptr().cast_mut().cast()) }
    }
}

#[cfg(test)]
impl CompilerId {
    pub fn for_test(address: u32) -> CompilerId {
        CompilerId(NonZeroU32::new(address).expect("a test id is not 0"))
    }
}
