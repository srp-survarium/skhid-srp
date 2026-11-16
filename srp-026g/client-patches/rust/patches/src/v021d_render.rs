//! Patch 130: the ported 0.21 maps render the 0.21 way (the owner,
//! 2026-11-09: "patch our own render specifically for those 0.21 maps"; the
//! native maps unchanged; docs/notes/client_patch.md section 3.82).
//!
//! The falling rain (the sprite drops) already runs 0.21's code in 0.26g. On
//! a 0.21 map's scene the patch makes the rest 0.21's:
//!
//! - the wet surfaces: stage_pre_rain's wet pass draws with 0.21's pixel
//!   shader (`wet_surface_v021d.ps`, the content pack's byte copy of 0.21d's
//!   `wet_sufrace.ps`), and at any shading quality (0.26g also wants it not
//!   0; 0.21d does not);
//! - the drops: rain_drops_parameters1.zw are 0.21d's half floats, and the
//!   drop texture is 0.21's (`textures/ported/v021d/engine/rain_drop`).
//!
//! Each of the two effects compiles the twins of its techniques after its
//! own, the same code run again with the one name changed: effect_wet_surface
//! its three again (3 is technique 0's twin), effect_rain its technique 1
//! again (2). Every compile makes them, on every map, so the effects stay
//! whole through their life; a stage applies a twin only while the frame
//! draws the 0.21 map's scene. The map is a project whose config says so
//! ([`MAP_KEY`]); the lobby's projects stay loaded during a match, so the
//! map is that project's own scene, never a time.
//!
//! Two of the ported materials say so themselves, so it holds wherever they
//! are compiled (material effects compile asynchronously and again on an
//! options change, the map aside): grass ([`GRASS_KEY`], instanced grassmesh
//! passes take 0.21's vertex shader family, [`GRASS_SHADERS`]) and water
//! ([`WATER_KEY`], the forward water's wave normals are 0.21's 1024^2
//! `engine/water_waves2`).
//!
//! Presentation only: nothing a match computes reads any of it.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::{ConfigTable, ConfigValue};
use crate::vostok::effect_compile::{self, CompileSite, CompilerId, MaterialPass, Pass, TextureSite, VertexInput};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::project::{ProjectId, ProjectView};
use crate::vostok::render_stage::{self, ApplySite, RenderSceneId, StageSite, Technique};
use crate::vostok::{self, Context, Flow, Lock, NoExit};

/// The key of a project config's root that makes its map a 0.21 one: a
/// boolean (the content pack's v021d projects).
pub const MAP_KEY: &CStr = c"srp_renderer_v021d";
/// The key of a material's effect config (`material/g_stage/effect`) whose
/// grass takes 0.21's vertex shaders; only its presence counts.
pub const GRASS_KEY: &CStr = c"srp_grass_v021d";
/// The key of a forward water material's effect config whose waves are
/// 0.21's; only its presence counts.
pub const WATER_KEY: &CStr = c"srp_water_v021d";

/// 0.21d's wet pixel shader (shaders/sm_4_0/wet_surface_v021d.ps/ + 67 `_`).
const WET_SURFACE_PS: &CStr = c"wet_surface_v021d";
/// 0.21d's drop texture (textures/ported/v021d/engine/rain_drop).
const RAIN_DROP: &CStr = c"ported/v021d/engine/rain_drop";
/// 0.21d's wave normals (textures/ported/v021d/engine/water_waves2).
const WATER_WAVES: &CStr = c"ported/v021d/engine/water_waves2";

/// The wet pass's twin: effect_wet_surface compiles techniques 0 wet, 1
/// write_normal, 2 copy, then the three again, 0's with 0.21's shader.
const WET_TWIN: Technique = Technique::new(3);
/// The drops' twin: effect_rain compiles 0 rain (never drawn), 1 the sprite
/// drops, then 1 again with 0.21's texture.
const DROPS_TWIN: Technique = Technique::new(2);

/// The grass vertex shader families and their 0.21 twins (each
/// shaders/sm_4_0/<twin>.vs/ + 67 `_`, the content pack's byte copy of
/// 0.21d's grass permutation of the family). A grass pass of a family not
/// here keeps its shader; a twin here the pack does not ship would leave a
/// pass without one.
pub const GRASS_SHADERS: [(&CStr, &CStr); 7] = [
    (c"vertex_base", c"vertex_base_v021d"),
    (c"vertex_base_shadow", c"vertex_base_shadow_v021d"),
    (c"vertex_base_shadow_batched", c"vertex_base_shadow_batched_v021d"),
    (c"vertex_base_lpv", c"vertex_base_lpv_v021d"),
    (c"vertex_base_unwrap", c"vertex_base_unwrap_v021d"),
    (c"vertex_base_bake_decal_occlusion", c"vertex_base_bake_decal_occlusion_v021d"),
    (c"vertex_base_z_shifted", c"vertex_base_z_shifted_v021d"),
];

/// How a frame's scene is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Native,
    V021d,
}

/// The map whose scene is drawn the 0.21 way, if one is loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Map {
    Native,
    V021d { project: ProjectId, scene: RenderSceneId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MapEvent {
    /// A project whose config has the key on was inserted, with its scene.
    Inserted { project: ProjectId, scene: RenderSceneId },
    /// A project was removed or is being destroyed.
    Gone(ProjectId),
}

impl Map {
    fn step(self, event: MapEvent) -> Map {
        match (self, event) {
            (_, MapEvent::Inserted { project, scene }) => Map::V021d { project, scene },
            (Map::V021d { project, .. }, MapEvent::Gone(gone)) if project == gone => Map::Native,
            (map, MapEvent::Gone(_)) => map,
        }
    }

    fn look(self, drawn: Option<RenderSceneId>) -> Look {
        match self {
            Map::V021d { scene, .. } if Some(scene) == drawn => Look::V021d,
            Map::V021d { .. } | Map::Native => Look::Native,
        }
    }
}

static MAP: Lock<Map> = Lock::new(Map::Native);

/// An effect this patch compiles twins for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Twins {
    WetSurface,
    RainDrops,
}

/// A compiler between an effect's end and its second end: compiling the twins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Round {
    compiler: CompilerId,
    twins: Twins,
}

/// What a compile does at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    /// Compile the twins: back to the techniques' start.
    Twins,
    /// Return, the twins compiled.
    Return,
}

/// Compiles at once: one per resource thread at most; past this many the
/// game stops (a round lost would leave an effect without its twin).
const ROUNDS: usize = 64;

/// The compilers compiling twins.
struct Rounds([Option<Round>; ROUNDS]);

impl Rounds {
    const fn new() -> Rounds {
        Rounds([None; ROUNDS])
    }

    fn find(&self, compiler: CompilerId) -> Option<(usize, Round)> {
        self.0.iter().enumerate().find_map(|(i, slot)| slot.filter(|round| round.compiler == compiler).map(|round| (i, round)))
    }

    /// The compile of `twins`'s effect by `compiler` is at its end.
    fn end(&mut self, compiler: CompilerId, twins: Twins) -> End {
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
    fn twins(&self, compiler: CompilerId) -> Option<Twins> {
        self.find(compiler).map(|(_, round)| round.twins)
    }
}

static ROUNDS_NOW: Lock<Rounds> = Lock::new(Rounds::new());

/// A float's mantissa bits, and its implicit leading one.
const MANTISSA: u32 = (1 << 23) - 1;
const IMPLICIT: i32 = 1 << 23;

/// vostok::math::half::half (0.21d 0x3E9CE), bit for bit: a float to a half
/// float, its mantissa rounded half up (+1000h), an overflow to infinity; a
/// value below the half's range shifted by the count's low five bits, as
/// x86's sar does.
fn half_of(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> 16) & 0x8000;
    let mut exponent = ((bits >> 23) & 0xFF) as i32 - 0x70;
    let mut mantissa = (bits & MANTISSA) as i32;
    let half = if exponent <= 0 {
        let shifted = (mantissa | IMPLICIT).wrapping_shr((1 - exponent) as u32);
        (shifted.wrapping_add(0x1000) >> 13) as u32 | sign
    } else if exponent == 0x8F {
        let top = sign | 0x7C00;
        let kept = mantissa >> 13;
        if mantissa == 0 { top } else { u32::from(kept == 0) | top | kept as u32 }
    } else {
        mantissa += 0x1000;
        if mantissa & IMPLICIT != 0 {
            mantissa = 0;
            exponent += 1;
        }
        if exponent >= 0x1F { sign | 0x7C00 } else { (mantissa >> 13) as u32 | ((exponent as u32) << 10) | sign }
    };
    half as u16
}

/// vostok::math::half_pod::operator float (0.21d 0x3E966), bit for bit.
fn float_of(half: u16) -> f32 {
    let h = u32::from(half);
    let sign = (h & 0x8000) << 16;
    let exponent = (h >> 10) & 0x1F;
    let mut mantissa = h & 0x3FF;
    let bits = match exponent {
        0 if mantissa == 0 => sign,
        0 => {
            let mut e: i32 = 0;
            while mantissa & 0x400 == 0 {
                mantissa += mantissa;
                e -= 1;
            }
            e += 1;
            mantissa &= !0x400;
            sign | (((e + 0x70) as u32) << 23) | (mantissa << 13)
        }
        0x1F => sign | ((mantissa | (0xFF << 10)) << 13),
        _ => sign | ((exponent + 0x70) << 23) | (mantissa << 13),
    };
    f32::from_bits(bits)
}

/// A float through a half and back.
fn halved(value: f32) -> f32 {
    float_of(half_of(value))
}

/// rain_drops_parameters1.z and .w as 0.21d's stage_rain::execute makes
/// them (0x12339C..0x123434): the scales and the factors (0.05, 0.4) as
/// half2s, each product of their floats a half again (0.26g: the plain
/// products).
fn drop_parameters(u_scale: f32, v_scale: f32) -> (f32, f32) {
    (halved(halved(u_scale) * halved(0.05)), halved(halved(v_scale) * halved(0.4)))
}

/// The 0.21 twin of a grass pass's vertex shader family, if it has one.
fn grass_twin(input: VertexInput, family: &CStr, marked: bool) -> Option<&'static CStr> {
    if input != VertexInput::GRASSMESH || !marked {
        return None;
    }
    GRASS_SHADERS.iter().find(|(native, _)| *native == family).map(|(_, twin)| *twin)
}

fn has(config: Option<&ConfigTable<'_>>, key: &CStr) -> bool {
    config.is_some_and(|table| matches!(table.get(key), Ok(Some(_))))
}

fn look(ctx: &Context<'_>, site: StageSite) -> Look {
    let drawn = render_stage::drawn_scene(ctx, site);
    MAP.with(ctx.thread(), |map| map.look(drawn))
}

/// project.insert (after the lamps'): a project whose config has the key on
/// makes its scene the 0.21 map's.
pub fn on_project_insert(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let project = ProjectView::at_insert(ctx).expect("project.insert's project");
    let root = project.root().ok().and_then(|root| root.table().ok()).expect("a project's config has a root table");
    let marked = match root.get(MAP_KEY).expect("a project's config reads") {
        None => false,
        Some(ConfigValue::Boolean(on)) => on,
        Some(_) => panic!("srp_renderer_v021d is a boolean"),
    };
    if marked {
        let scene = project.render_scene().expect("a project being inserted has its scene");
        MAP.with(ctx.thread(), |map| *map = map.step(MapEvent::Inserted { project: project.id(), scene }));
        log::line(&[Text("v021d_render: a 0.21 map; its scene's wet surfaces and rain drops drawn the 0.21 way")]);
    }
    Flow::Continue
}

fn gone(ctx: &Context<'_>, project: Option<ProjectId>) {
    if let Some(project) = project {
        MAP.with(ctx.thread(), |map| *map = map.step(MapEvent::Gone(project)));
    }
}

/// project.removed: the map's project removed, its scene is no longer it.
pub fn on_project_removed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    gone(ctx, ProjectId::removed(ctx));
    Flow::Continue
}

/// project.destroy: as removed (a project destroyed without its removal).
pub fn on_project_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    gone(ctx, ProjectId::destroyed(ctx));
    Flow::Continue
}

/// effect_wet_surface::compile's end: the first time back to technique 0,
/// for the twins; the second time its return.
pub fn on_wet_surface_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V021DRenderOnWetSurfaceCompiledExit> {
    use vostok::V021DRenderOnWetSurfaceCompiledExit as Exit;
    let compiler = effect_compile::compiler_id(ctx, CompileSite::WetSurfaceEnd);
    match ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler, Twins::WetSurface)) {
        End::Return => Flow::Continue,
        End::Twins => Flow::Exit(match effect_compile::pass(ctx, CompileSite::WetSurfaceEnd) {
            Pass::Building => Exit::Begin,
            Pass::Caching => Exit::Cached,
        }),
    }
}

/// effect_wet_surface's technique 0 (wet) begins its pass: the twin's with
/// 0.21's pixel shader.
pub fn on_wet_surface_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let compiler = effect_compile::compiler_id(ctx, CompileSite::WetSurfacePass);
    if ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.twins(compiler)) == Some(Twins::WetSurface) {
        effect_compile::set_wet_surface_pixel_shader(ctx, WET_SURFACE_PS);
    }
    Flow::Continue
}

/// effect_rain::compile's end: the first time back to technique 1, for its
/// twin; the second time its return.
pub fn on_rain_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V021DRenderOnRainCompiledExit> {
    let compiler = effect_compile::compiler_id(ctx, CompileSite::RainEnd);
    match ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler, Twins::RainDrops)) {
        End::Return => Flow::Continue,
        End::Twins => {
            effect_compile::enter_rain_technique(ctx);
            Flow::Exit(vostok::V021DRenderOnRainCompiledExit::Technique)
        }
    }
}

/// effect_rain's technique 1 (the drops) binds t_rain_drop: the twin's to
/// 0.21's texture.
pub fn on_rain_drop_texture(ctx: &mut Context<'_>) -> Flow<vostok::V021DRenderOnRainDropTextureExit> {
    let compiler = effect_compile::compiler_id(ctx, CompileSite::RainDropTexture);
    if ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.twins(compiler)) == Some(Twins::RainDrops) {
        effect_compile::bind_texture(ctx, TextureSite::RainDrop, RAIN_DROP);
        return Flow::Exit(vostok::V021DRenderOnRainDropTextureExit::Bound);
    }
    Flow::Continue
}

/// stage_pre_rain's shading-quality gate: none on the 0.21 map's scene.
pub fn on_wet_gate(ctx: &mut Context<'_>) -> Flow<vostok::V021DRenderOnWetGateExit> {
    match look(ctx, StageSite::WetGate) {
        Look::V021d => Flow::Exit(vostok::V021DRenderOnWetGateExit::Wet),
        Look::Native => Flow::Continue,
    }
}

/// stage_pre_rain's wet pass: the twin on the 0.21 map's scene.
pub fn on_wet_apply(ctx: &mut Context<'_>) -> Flow<vostok::V021DRenderOnWetApplyExit> {
    match look(ctx, StageSite::WetApply) {
        Look::V021d => {
            render_stage::apply_technique(ctx, ApplySite::Wet, WET_TWIN);
            Flow::Exit(vostok::V021DRenderOnWetApplyExit::Apply)
        }
        Look::Native => Flow::Continue,
    }
}

/// stage_rain's rain_drops_parameters1.zw: 0.21d's on the 0.21 map's scene.
pub fn on_drop_scales(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if look(ctx, StageSite::DropScales) == Look::V021d
        && let Some((u_scale, v_scale)) = render_stage::rain_scales(ctx)
    {
        let (z, w) = drop_parameters(u_scale, v_scale);
        render_stage::set_drop_parameters(ctx, z, w);
    }
    Flow::Continue
}

/// stage_rain's drops: the twin on the 0.21 map's scene.
pub fn on_drops_apply(ctx: &mut Context<'_>) -> Flow<vostok::V021DRenderOnDropsApplyExit> {
    match look(ctx, StageSite::DropsApply) {
        Look::V021d => {
            render_stage::apply_technique(ctx, ApplySite::Drops, DROPS_TWIN);
            Flow::Exit(vostok::V021DRenderOnDropsApplyExit::Apply)
        }
        Look::Native => Flow::Continue,
    }
}

/// A material effect's pass: a marked material's grass takes 0.21's
/// vertex shader.
pub fn on_material_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut pass) = MaterialPass::at(ctx)
        && let Some(family) = pass.vertex_shader()
        && let Some(twin) = grass_twin(pass.vertex_input(), family, has(pass.config(), GRASS_KEY))
    {
        pass.set_vertex_shader(twin);
    }
    Flow::Continue
}

/// The forward water's wave normals: a marked material's are 0.21's.
pub fn on_water_texture(ctx: &mut Context<'_>) -> Flow<vostok::V021DRenderOnWaterTextureExit> {
    if has(effect_compile::water_config(ctx).as_ref(), WATER_KEY) {
        effect_compile::bind_texture(ctx, TextureSite::WaterWaves, WATER_WAVES);
        return Flow::Exit(vostok::V021DRenderOnWaterTextureExit::Bound);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(n: u32) -> ProjectId {
        ProjectId::for_test(n)
    }

    fn scene(n: u32) -> RenderSceneId {
        RenderSceneId::for_test(n)
    }

    #[test]
    fn the_map_is_its_project_scene_until_it_goes() {
        let map = Map::Native.step(MapEvent::Inserted { project: project(1), scene: scene(10) });
        assert_eq!(map.look(Some(scene(10))), Look::V021d);
        // the lobby's scene, drawn while the match's project is loaded
        assert_eq!(map.look(Some(scene(20))), Look::Native);
        assert_eq!(map.look(None), Look::Native);
        // another project going leaves it
        assert_eq!(map.step(MapEvent::Gone(project(2))), map);
        let gone = map.step(MapEvent::Gone(project(1)));
        assert_eq!(gone, Map::Native);
        assert_eq!(gone.look(Some(scene(10))), Look::Native);
        assert_eq!(Map::Native.step(MapEvent::Gone(project(1))), Map::Native);
    }

    #[test]
    fn a_later_map_takes_over() {
        let first = Map::Native.step(MapEvent::Inserted { project: project(1), scene: scene(10) });
        let second = first.step(MapEvent::Inserted { project: project(2), scene: scene(30) });
        assert_eq!(second.look(Some(scene(10))), Look::Native);
        assert_eq!(second.look(Some(scene(30))), Look::V021d);
        // the first one's removal, late, changes nothing
        assert_eq!(second.step(MapEvent::Gone(project(1))), second);
    }

    #[test]
    fn a_compile_ends_twice_once_for_its_twins() {
        let mut rounds = Rounds::new();
        let (a, b) = (CompilerId::for_test(0x100), CompilerId::for_test(0x200));
        assert_eq!(rounds.twins(a), None);
        assert_eq!(rounds.end(a, Twins::WetSurface), End::Twins);
        assert_eq!(rounds.twins(a), Some(Twins::WetSurface));
        // another thread's compile meanwhile
        assert_eq!(rounds.end(b, Twins::RainDrops), End::Twins);
        assert_eq!(rounds.twins(b), Some(Twins::RainDrops));
        assert_eq!(rounds.end(a, Twins::WetSurface), End::Return);
        assert_eq!(rounds.twins(a), None);
        assert_eq!(rounds.end(b, Twins::RainDrops), End::Return);
        // the next compile of the same compiler starts over
        assert_eq!(rounds.end(a, Twins::RainDrops), End::Twins);
        assert_eq!(rounds.end(a, Twins::RainDrops), End::Return);
    }

    #[test]
    #[should_panic(expected = "fewer compiles at once")]
    fn too_many_compiles_at_once_stop() {
        let mut rounds = Rounds::new();
        for i in 1..=ROUNDS as u32 + 1 {
            rounds.end(CompilerId::for_test(i), Twins::WetSurface);
        }
    }

    #[test]
    fn halves_are_0_21ds() {
        // exact halves stay
        for v in [0.0f32, -0.0, 1.0, 0.5, 2.0, -3.25, 65504.0, 6.103_515_6e-5] {
            assert_eq!(halved(v).to_bits(), v.to_bits(), "{v}");
        }
        // 10 bits of mantissa, rounded half up
        assert_eq!(half_of(0.05), 0x2A66);
        assert_eq!(half_of(0.4), 0x3666);
        assert_eq!(half_of(1.0 + 1.0 / 2048.0), 0x3C01);
        assert_eq!(half_of(70000.0), 0x7C00);
        assert_eq!(half_of(f32::INFINITY), 0x7C00);
        assert_eq!(half_of(f32::NEG_INFINITY), 0xFC00);
        assert_eq!(half_of(f32::NAN) & 0x7C00, 0x7C00);
        assert!(halved(f32::NAN).is_nan());
        // a half denormal and back
        assert_eq!(half_of(5.960_464_5e-8), 0x0001);
        assert_eq!(float_of(0x0001), 5.960_464_5e-8);
        assert_eq!(float_of(0x03FF).to_bits(), 0x387F_C000);
    }

    #[test]
    fn drop_parameters_are_halves_of_half_products() {
        let (z, w) = drop_parameters(0.77, 1.0);
        assert_eq!(halved(z), z);
        assert_eq!(halved(w), w);
        assert_eq!(w, halved(halved(1.0) * halved(0.4)));
        assert_ne!(z, 0.77 * 0.05);
        assert!((z - 0.77 * 0.05).abs() < 0.0001);
    }

    #[test]
    fn grass_twins_need_the_grass_input_and_the_mark() {
        for (native, twin) in GRASS_SHADERS {
            assert_eq!(grass_twin(VertexInput::GRASSMESH, native, true), Some(twin));
            assert_eq!(grass_twin(VertexInput::GRASSMESH, native, false), None);
            assert_eq!(grass_twin(VertexInput::parse(0x02), native, true), None);
            let mut name = std::string::String::from(native.to_str().unwrap());
            name.push_str("_v021d");
            assert_eq!(twin.to_str().unwrap(), name);
        }
        // the type's two top bits are not the input's
        assert_eq!(grass_twin(VertexInput::parse(0xCB), c"vertex_base", true), Some(c"vertex_base_v021d"));
        assert_eq!(grass_twin(VertexInput::GRASSMESH, c"vertex_base_tree", true), None);
        assert_eq!(grass_twin(VertexInput::GRASSMESH, c"vertex_base_v021d", true), None);
    }
}
