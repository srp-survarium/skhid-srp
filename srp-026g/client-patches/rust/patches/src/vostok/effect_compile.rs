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

impl CompilerId {
    pub(in crate::vostok) fn of(id: NonZeroU32) -> CompilerId {
        CompilerId(id)
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
    /// `composition_<abc>.compiled`: the composition effect's epilogue.
    CompositionEnd(Composition),
    /// `composition_<abc>.begin_pass`: its technique's begin_pass.
    CompositionPass(Composition),
    /// `composition_<abc>.sun_shadow`: its t_sun_shadow_and_scattering.
    CompositionSunShadow(Composition),
}

/// One of stage_composition's eight effects, effect_composition<a,b,c>
/// (0x1A9330..0x1AA590, one compile each, the same code at the same offsets
/// but for the configuration's bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Composition {
    C000,
    C100,
    C010,
    C110,
    C001,
    C101,
    C011,
    C111,
}

fn compiler(ctx: &Context<'_>, site: CompileSite) -> raw::EffectCompiler {
    // SAFETY: the context is the site's own; EBX is the compile's
    // effect_compiler there (each compile loads it from its first argument,
    // 0x1CE10A, 0x1D3B9A), alive while its compile runs.
    unsafe {
        match site {
            CompileSite::WetSurfaceEnd => raw::site::wet_surface_compiled::compiler(ctx),
            CompileSite::WetSurfacePass => raw::site::wet_surface_begin_pass::compiler(ctx),
            CompileSite::RainEnd => raw::site::rain_compiled::compiler(ctx),
            CompileSite::RainDropTexture => raw::site::rain_drop_texture::compiler(ctx),
            CompileSite::WaterTexture => raw::site::water_normal_texture::compiler(ctx),
            CompileSite::CompositionEnd(composition) => composition_end_compiler(ctx, composition),
            CompileSite::CompositionPass(composition) => composition_pass_compiler(ctx, composition),
            CompileSite::CompositionSunShadow(composition) => composition_sun_shadow_compiler(ctx, composition),
        }
    }
    .expect("the compile's compiler")
}

/// # Safety
/// The context is the site's (`composition_<abc>.compiled`).
unsafe fn composition_end_compiler(ctx: &Context<'_>, composition: Composition) -> Option<raw::EffectCompiler> {
    use raw::site::*;
    unsafe {
        match composition {
            Composition::C000 => composition_000_compiled::compiler(ctx),
            Composition::C100 => composition_100_compiled::compiler(ctx),
            Composition::C010 => composition_010_compiled::compiler(ctx),
            Composition::C110 => composition_110_compiled::compiler(ctx),
            Composition::C001 => composition_001_compiled::compiler(ctx),
            Composition::C101 => composition_101_compiled::compiler(ctx),
            Composition::C011 => composition_011_compiled::compiler(ctx),
            Composition::C111 => composition_111_compiled::compiler(ctx),
        }
    }
}

/// # Safety
/// The context is the site's (`composition_<abc>.begin_pass`).
unsafe fn composition_pass_compiler(ctx: &Context<'_>, composition: Composition) -> Option<raw::EffectCompiler> {
    use raw::site::*;
    unsafe {
        match composition {
            Composition::C000 => composition_000_begin_pass::compiler(ctx),
            Composition::C100 => composition_100_begin_pass::compiler(ctx),
            Composition::C010 => composition_010_begin_pass::compiler(ctx),
            Composition::C110 => composition_110_begin_pass::compiler(ctx),
            Composition::C001 => composition_001_begin_pass::compiler(ctx),
            Composition::C101 => composition_101_begin_pass::compiler(ctx),
            Composition::C011 => composition_011_begin_pass::compiler(ctx),
            Composition::C111 => composition_111_begin_pass::compiler(ctx),
        }
    }
}

/// # Safety
/// The context is the site's (`composition_<abc>.sun_shadow`).
unsafe fn composition_sun_shadow_compiler(ctx: &Context<'_>, composition: Composition) -> Option<raw::EffectCompiler> {
    use raw::site::*;
    unsafe {
        match composition {
            Composition::C000 => composition_000_sun_shadow::compiler(ctx),
            Composition::C100 => composition_100_sun_shadow::compiler(ctx),
            Composition::C010 => composition_010_sun_shadow::compiler(ctx),
            Composition::C110 => composition_110_sun_shadow::compiler(ctx),
            Composition::C001 => composition_001_sun_shadow::compiler(ctx),
            Composition::C101 => composition_101_sun_shadow::compiler(ctx),
            Composition::C011 => composition_011_sun_shadow::compiler(ctx),
            Composition::C111 => composition_111_sun_shadow::compiler(ctx),
        }
    }
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

/// Makes `name` the pixel shader of the begin_pass at `wet_surface.begin_pass`
/// (its pushed argument; the VS and the configuration stay).
pub fn set_wet_surface_pixel_shader(ctx: &Context<'_>, name: &'static CStr) {
    // SAFETY: the site's ESP: the PS name begin_pass's caller pushed (its
    // third argument once the gs name and the compiler follow), which only
    // begin_pass reads.
    unsafe { raw::site::wet_surface_begin_pass::frame(ctx).expect("the site's ESP").set_pixel_shader(name.as_ptr().cast_mut().cast()) }
}

/// Makes `name` the pixel shader of the composition technique's begin_pass
/// at `composition_<abc>.begin_pass` (its pushed argument; the VS, also
/// `composition`, and the configuration stay).
pub fn set_composition_pixel_shader(ctx: &Context<'_>, composition: Composition, name: &'static CStr) {
    use raw::site::*;
    // SAFETY: the site's ESP: the PS name begin_pass's caller pushed (0x1A939D
    // in effect_composition<0,0,0>, the same offset in each), which only
    // begin_pass reads.
    unsafe {
        let frame = match composition {
            Composition::C000 => composition_000_begin_pass::frame(ctx),
            Composition::C100 => composition_100_begin_pass::frame(ctx),
            Composition::C010 => composition_010_begin_pass::frame(ctx),
            Composition::C110 => composition_110_begin_pass::frame(ctx),
            Composition::C001 => composition_001_begin_pass::frame(ctx),
            Composition::C101 => composition_101_begin_pass::frame(ctx),
            Composition::C011 => composition_011_begin_pass::frame(ctx),
            Composition::C111 => composition_111_begin_pass::frame(ctx),
        };
        frame.expect("the site's ESP").set_pixel_shader(name.as_ptr().cast_mut().cast())
    }
}

/// Binds the composition's sun shadow under `slot` where its block at
/// `composition_<abc>.sun_shadow` binds it as t_sun_shadow_and_scattering:
/// the game's set_texture (0x3D98B0) with the block's arguments (its pushes:
/// ESI, 1.0, ESI, -1, ESI, $user$sun_shadow_and_scattering, the slot, the
/// compiler; ESI 0 there), the slot ours. The caller takes the site's `bound`
/// exit, which skips the block.
pub fn bind_composition_sun_shadow(ctx: &Context<'_>, composition: Composition, slot: &'static CStr) {
    let compiler = compiler(ctx, CompileSite::CompositionSunShadow(composition));
    let esi = ctx.reg(Reg::Esi);
    // SAFETY: set_texture (stdcall, its eight arguments on the stack), on the
    // compile's own thread and compiler, in the place of the game's call; it
    // reads the two names (ours and the game's own render target's, static)
    // and keeps them by name.
    unsafe {
        raw::engine_set_texture(Some(compiler), slot.as_ptr().cast(), c"$user$sun_shadow_and_scattering".as_ptr().cast(), esi, u32::MAX, esi, 1.0, esi)
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

    /// The PS family name the pass is about to begin with.
    pub fn pixel_shader(&self) -> Option<&'c CStr> {
        // SAFETY: the PS name compile_begin pushed for begin_pass (its second
        // argument: a C string of the exe's, or one of ours).
        unsafe { super::ui::c_string(self.frame.pixel_shader(), 128) }
    }

    /// Makes `name` the PS family of this pass (the pushed slot begin_pass
    /// takes it from; nothing reads it after).
    pub fn set_pixel_shader(&mut self, name: &'static CStr) {
        // SAFETY: begin_pass's argument slot compile_begin pushed (see `at`).
        unsafe { self.frame.set_pixel_shader(name.as_ptr().cast_mut().cast()) }
    }
}

/// A material effect's pass at compile_begin's end (`material.begun`): its
/// pass begun and its states set, before the effect's own textures and
/// constants.
pub struct MaterialBegun<'c> {
    compiler: raw::EffectCompiler,
    config: Option<ConfigTable<'c>>,
}

impl<'c> MaterialBegun<'c> {
    pub fn at(ctx: &'c Context<'_>) -> MaterialBegun<'c> {
        use raw::site::material_begun as site;
        // SAFETY: the site's EBP is compile_begin's compiler (its third
        // argument, 0x1CD0B7); the ESP frame holds its arguments, the config
        // tree the compile data's, immutable while the effect compiles.
        unsafe {
            let compiler = site::compiler(ctx).expect("the compile's compiler");
            let config = site::frame(ctx).and_then(|frame| frame.config()).and_then(|tree| ConfigValue::read(tree).ok()?.table().ok());
            MaterialBegun { compiler, config }
        }
    }

    /// The material effect's merged config (its <stage>.effect table), if a table.
    pub fn config(&self) -> Option<&ConfigTable<'c>> {
        self.config.as_ref()
    }

    /// Sets a float4 constant of the pass: the game's set_constant<float4>,
    /// which binds it only while building and only when no binding has the
    /// name, and which a program without the name skips.
    pub fn send_float4(&self, name: &'static CStr, value: [f32; 4]) {
        // SAFETY: set_constant<float4> (thiscall: ECX the value, then the
        // compiler and the name), on the compile's own thread and compiler;
        // it copies the value and interns the name.
        unsafe { raw::engine_set_constant_float4(value.as_ptr(), Some(self.compiler), name.as_ptr().cast()) }
    }
}

/// The shader configuration's byte 0Dh, as fill_shader_configuration_macros
/// (0x3C9B80) makes macros of it. Its other bits (soft edges, terrain blend,
/// thickness, scattering) are the compile's own and stay as they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switches0d(u8);

impl Switches0d {
    /// Bit 0, CONFIG_USE_FUZZINESS: in the forward programs, the uv
    /// distortion moves the emissive map only.
    pub const FUZZINESS: u8 = 0x01;
    /// Bit 6, CONFIG_USE_OLTA: in the forward and particle lighting
    /// programs, the transparency map is a DXT1 image.
    pub const OLTA: u8 = 0x40;
    /// Bit 7, CONFIG_USE_DISPLACEMENT: in the forward programs (and their
    /// vertex shader), the two maps tile by constant_tile_uv.
    pub const DISPLACEMENT: u8 = 0x80;

    pub fn bits(self) -> u8 {
        self.0
    }

    /// The byte with `bit` (one of the constants) on or off.
    pub fn with(self, bit: u8, on: bool) -> Switches0d {
        Switches0d(if on { self.0 | bit } else { self.0 & !bit })
    }
}

/// The compiles a patch sees at their call of compile_begin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeginSite {
    /// `forward.compile_begin`: forward_default's compile.
    Forward,
    /// `particle_lighting.compile_begin`: particle_lighting's compile.
    ParticleLighting,
}

/// A material effect's call of compile_begin: its shader configuration,
/// final there (compile_begin copies it for begin_pass), and its config tree.
pub struct CompileBegin<'c> {
    configuration: raw::ShaderConfiguration,
    tree: Option<ConfigTable<'c>>,
}

impl<'c> CompileBegin<'c> {
    pub fn at(ctx: &'c Context<'_>, site: BeginSite) -> Option<CompileBegin<'c>> {
        // SAFETY: the site's ESP holds the arguments the compile pushed for
        // compile_begin: the configuration is the compile's own local, the
        // tree the compile data's, immutable while the effect compiles.
        unsafe {
            let (configuration, tree) = match site {
                BeginSite::Forward => {
                    let frame = raw::site::forward_compile_begin::frame(ctx)?;
                    (frame.configuration()?, frame.tree())
                }
                BeginSite::ParticleLighting => {
                    let frame = raw::site::particle_lighting_compile_begin::frame(ctx)?;
                    (frame.configuration(), frame.tree())
                }
            };
            let tree = tree.and_then(|tree| ConfigValue::read(tree).ok()?.table().ok());
            Some(CompileBegin { configuration, tree })
        }
    }

    /// The material's merged config (its <stage>.effect table), if a table.
    pub fn tree(&self) -> Option<&ConfigTable<'c>> {
        self.tree.as_ref()
    }

    pub fn switches_0d(&self) -> Switches0d {
        // SAFETY: the compile's configuration (see `at`).
        Switches0d(unsafe { self.configuration.switches_0d().read() })
    }

    pub fn set_switches_0d(&mut self, switches: Switches0d) {
        // SAFETY: the compile's configuration (see `at`), read next by
        // compile_begin's copy for begin_pass.
        unsafe { self.configuration.switches_0d().write(switches.0) }
    }
}

/// A material's values as a compile reads them: a key's `value` child,
/// whatever its type.
pub trait MaterialFields {
    /// A switch: on when its value is not 0, as the compiles test it
    /// (`cmp dword [eax],0`); off when the key is missing.
    fn switch(&self, name: &CStr) -> bool;
    /// A number: a float, or an integer made a float (the compiles' test of
    /// the type word +14h); None when the key is missing.
    fn number(&self, name: &CStr) -> Option<f32>;
}

impl MaterialFields for ConfigTable<'_> {
    fn switch(&self, name: &CStr) -> bool {
        match value_of(self, name) {
            Some(ConfigValue::Boolean(on)) => on,
            Some(ConfigValue::Integer(n)) => n != 0,
            Some(ConfigValue::Number(x)) => x.to_bits() != 0,
            _ => false,
        }
    }

    fn number(&self, name: &CStr) -> Option<f32> {
        value_of(self, name)?.number().ok()
    }
}

fn value_of<'t>(table: &ConfigTable<'t>, name: &CStr) -> Option<ConfigValue<'t>> {
    table.get(name).ok()??.table().ok()?.get(c"value").ok()?
}

/// The material config of the forward compile at `forward.tile_uv`.
pub fn tile_uv_tree<'c>(ctx: &'c Context<'_>) -> Option<ConfigTable<'c>> {
    // SAFETY: the site's EBX, the compile's config tree (its second
    // argument, 0x1D8F78), immutable while the effect compiles.
    unsafe { ConfigValue::read(raw::site::forward_tile_uv::tree(ctx)?).ok()?.table().ok() }
}

/// Sends constant_tile_uv as a float4 at `forward.tile_uv`, before the
/// forward compile's own block sends its float2: a set_constant keeps a
/// name's first value (the block's set of the name then adds nothing), so
/// the float4 is the constant.
pub fn send_tile_uv(ctx: &Context<'_>, value: [f32; 4]) {
    // SAFETY: the site's EDI, the compile's compiler, which both of the
    // block's calls are given.
    let compiler = unsafe { raw::site::forward_tile_uv::compiler(ctx) }.expect("the compile's compiler");
    // SAFETY: set_constant<float4> (thiscall: ECX the value, then the
    // compiler and the name), on the compile's own thread and compiler, in
    // the place of the game's set_constant<float2>; it copies the value and
    // interns the name.
    unsafe { raw::engine_set_constant_float4(value.as_ptr(), Some(compiler), c"constant_tile_uv".as_ptr().cast()) }
}

/// The g-buffer compile's parallax block (`gstage.parallax`): its material
/// config and its compiler.
pub struct GstageParallax<'c> {
    compiler: raw::EffectCompiler,
    tree: Option<ConfigTable<'c>>,
}

impl<'c> GstageParallax<'c> {
    pub fn at(ctx: &'c Context<'_>) -> GstageParallax<'c> {
        use raw::site::gstage_parallax as site;
        // SAFETY: the site's EBP is effect_gstage_default_materials::compile's
        // frame, [EBP+8] its compiler; EBX the compile's config tree,
        // immutable while the effect compiles.
        unsafe {
            let compiler = site::frame(ctx).and_then(|frame| frame.compiler()).expect("the compile's compiler");
            let tree = site::tree(ctx).and_then(|tree| ConfigValue::read(tree).ok()?.table().ok());
            GstageParallax { compiler, tree }
        }
    }

    /// The material's merged config (its g_stage effect table), if a table.
    pub fn tree(&self) -> Option<&ConfigTable<'c>> {
        self.tree.as_ref()
    }

    /// Sets a float2 constant of the compile: the game's set_constant<float2>,
    /// which binds it only while building and only when no binding has the name.
    pub fn send_float2(&self, name: &'static CStr, value: [f32; 2]) {
        // SAFETY: set_constant<float2> (thiscall: ECX the value, then the
        // compiler and the name), on the compile's own thread and compiler;
        // it copies the value and interns the name.
        unsafe { raw::engine_set_constant_float2(value.as_ptr(), Some(self.compiler), name.as_ptr().cast()) }
    }
}

#[cfg(test)]
impl Switches0d {
    pub fn default_for_test(byte: u8) -> Switches0d {
        Switches0d(byte)
    }
}

#[cfg(test)]
impl CompilerId {
    pub fn for_test(address: u32) -> CompilerId {
        CompilerId(NonZeroU32::new(address).expect("a test id is not 0"))
    }
}
