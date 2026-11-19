//! The ported 0.11 weathers render their atmosphere the 0.11 way (Vostok
//! Radar Station v0.11's evening and rain; srp-026g/docs/notes/v011-weather.md):
//! the geometry's fog, the scattering tables and the sky.
//!
//! 0.11e draws the geometry's fog in two passes of its scattering effect: it
//! multiplies the frame by 1 - min(depth, D) / D / 10, then adds the light
//! the air scatters toward the eye, read from the Rayleigh and Mie tables;
//! D is the weather's inscatter distance. 0.26g draws one pass of another
//! equation (technique 5) but still compiles 0.11e's two (3 and 4). On the
//! scene of a project whose config root names its inscatter distance
//! ([`MAP_KEY`]) the geometry stage draws 3 in 5's place and 4 after it; the
//! content pack v011e-radar-fog gives 4 0.11e's program (nothing else draws
//! it). The map is that project's own scene, as patch 130's is.
//!
//! The scattering tables (the Rayleigh and Mie 256x128 targets the sky and
//! the fog read) and the sky are 0.11e's programs on that scene too:
//! effect_atmospheric_scattering compiles twins of its techniques 0 and 1
//! after its own (6 and 7, the same passes with `_v011w` pixel shaders, the
//! content pack's builds of srp-026g/shaders/v011w), on every map, as patch
//! 130's effects do; the sky stage applies a twin only on that scene. The
//! tables are made once and kept while nothing changes, for whichever scene
//! made them, so a frame whose look differs from theirs remakes them.
//!
//! Presentation only: nothing a match computes reads any of it.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::{ConfigTable, ConfigValue};
use crate::vostok::effect_compile::{self, CompileSite, End, Pass, PassSite, Rounds};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::project::{ProjectId, ProjectView};
use crate::vostok::render_stage::{self, ApplySite, AtmosphereStage, RenderSceneId, StageSite};
use crate::vostok::{self, Context, Flow, Lock, NoExit};

/// The key of a project config's root that makes its map a 0.11 weather: a
/// table, the weather's profile ([`Profile`]: 0.11e's post-process material
/// values of that weather), so another map's project takes a 0.11 weather by
/// naming one.
pub const MAP_KEY: &CStr = c"srp_renderer_v011w";

/// The twins' pixel shaders (shaders/sm_4_0/<name>.ps/ + 67 `_`, the content
/// pack's): 0.11e's scattering tables, sky and post.
const TABLES_PS: &CStr = c"make_mie_rayleigh_texture_v011w";
const SKY_PS: &CStr = c"atmosphere_v011w";
const POST_PS: &CStr = c"complex_post_process_blend_v011w";

/// A project's inscatter distance, metres: above 0 (both passes divide by it).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Distance(f32);

/// A 0.11 weather's profile, its `MAP_KEY` table: 0.11e's post material's
/// `atmosphere_inscatter_distance` and, when the table names it, its filmic
/// curve (all eight [`FILMIC_KEYS`] or none).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Profile {
    distance: Distance,
    post: Post,
}

/// A 0.11 weather's post.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Post {
    /// 0.26g's blend.
    Native,
    /// 0.11e's (the twin), with its filmic curve packed as 0.11e's
    /// stage_postprocess packs it: filmic_tonemap_packed_parameters_0
    /// (shoulder strength, linear strength, linear angle, toe strength) and _1
    /// (toe numerator, toe denominator, linear white, dark desaturation limit).
    Filmic([[f32; 4]; 2]),
}

/// The filmic curve's keys, in packing order.
const FILMIC_KEYS: [&CStr; 8] = [
    c"shoulder_strength",
    c"linear_strength",
    c"linear_angle",
    c"toe_strength",
    c"toe_numerator",
    c"toe_denominator",
    c"linear_white",
    c"dark_desaturation_limit",
];

impl Profile {
    /// From the table's numbers by key (`number` is None for a missing key
    /// and panics on a value that is not a number).
    fn parse(number: impl Fn(&CStr) -> Option<f32>) -> Profile {
        let metres = number(c"inscatter_distance").expect("srp_renderer_v011w names its inscatter_distance");
        assert!(metres.is_finite() && metres > 0.0, "srp_renderer_v011w.inscatter_distance is a distance above 0");
        let values = FILMIC_KEYS.map(&number);
        let post = if values.iter().all(Option::is_none) {
            Post::Native
        } else {
            let v = values.map(|value| value.expect("srp_renderer_v011w names all eight filmic values or none"));
            Post::Filmic([[v[0], v[1], v[2], v[3]], [v[4], v[5], v[6], v[7]]])
        };
        Profile { distance: Distance(metres), post }
    }

    fn of_table(table: ConfigTable<'_>) -> Profile {
        Profile::parse(|key| match table.get(key).expect("a project's config reads") {
            None => None,
            Some(ConfigValue::Integer(n)) => Some(n as f32),
            Some(ConfigValue::Number(n)) => Some(n),
            Some(_) => panic!("srp_renderer_v011w: {key:?} is a number"),
        })
    }
}

/// The map whose scene is fogged the 0.11 way, if one is loaded.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Map {
    Native,
    V011w { project: ProjectId, scene: RenderSceneId, profile: Profile },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MapEvent {
    Inserted { project: ProjectId, scene: RenderSceneId, profile: Profile },
    Gone(ProjectId),
}

impl Map {
    fn step(self, event: MapEvent) -> Map {
        match (self, event) {
            (_, MapEvent::Inserted { project, scene, profile }) => Map::V011w { project, scene, profile },
            (Map::V011w { project, .. }, MapEvent::Gone(gone)) if project == gone => Map::Native,
            (map, MapEvent::Gone(_)) => map,
        }
    }

    /// How the frame drawing `drawn` looks.
    fn look(self, drawn: Option<RenderSceneId>) -> Look {
        match self {
            Map::V011w { scene, profile, .. } if Some(scene) == drawn => Look::V011w(profile),
            Map::V011w { .. } | Map::Native => Look::Native,
        }
    }
}

/// How a frame's atmosphere is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Native,
    /// The 0.11 weather's scene, with its profile.
    V011w(Profile),
}

impl Look {
    fn programs(self) -> Programs {
        match self {
            Look::Native => Programs::Native,
            Look::V011w(_) => Programs::V011w,
        }
    }
}

/// Whose programs made the scattering tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Programs {
    Native,
    V011w,
}

/// The scattering tables the context holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tables {
    /// Not made since the game started (the game makes them on the first sky).
    Unmade,
    Made(Programs),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TablesEvent {
    /// `atmosphere.tables_due`: the sky stage decides, for a frame of this look.
    Due(Programs),
    /// `atmosphere.tables_apply`: they are being made with these programs.
    Making(Programs),
}

/// What a tables event asks of the handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TablesAction {
    None,
    /// Make them this frame whatever the game decides.
    Remake,
}

impl Tables {
    fn step(self, event: TablesEvent) -> (Tables, TablesAction) {
        match (self, event) {
            (Tables::Made(made), TablesEvent::Due(frame)) if made != frame => (self, TablesAction::Remake),
            (Tables::Made(_) | Tables::Unmade, TablesEvent::Due(_)) => (self, TablesAction::None),
            (_, TablesEvent::Making(programs)) => (Tables::Made(programs), TablesAction::None),
        }
    }
}

static TABLES: Lock<Tables> = Lock::new(Tables::Unmade);

/// The effects this patch compiles twins for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Twins {
    Scattering,
    PostBlend,
}

static ROUNDS_NOW: Lock<Rounds<Twins>> = Lock::new(Rounds::new());

static MAP: Lock<Map> = Lock::new(Map::Native);

/// Where the geometry stage's fog stands this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fog {
    /// No 0.11 pass is under way.
    Idle,
    /// The multiplicative pass took technique 5's place; the additive one is due.
    Multiplied(Distance),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FogEvent {
    /// `atmosphere.geometry_fog` on a 0.11 weather's scene.
    Multiply(Distance),
    /// The god rays' entry in the geometry stage.
    GodRays,
}

/// What a fog event asks of the handler.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FogAction {
    None,
    /// Draw the additive pass with this distance.
    Add(Distance),
}

impl Fog {
    fn step(self, event: FogEvent) -> (Fog, FogAction) {
        match (self, event) {
            (_, FogEvent::Multiply(distance)) => (Fog::Multiplied(distance), FogAction::None),
            (Fog::Multiplied(distance), FogEvent::GodRays) => (Fog::Idle, FogAction::Add(distance)),
            (Fog::Idle, FogEvent::GodRays) => (Fog::Idle, FogAction::None),
        }
    }
}

static FOG: Lock<Fog> = Lock::new(Fog::Idle);

fn look(ctx: &Context<'_>, site: StageSite) -> Look {
    let drawn = render_stage::drawn_scene(ctx, site);
    MAP.with(ctx.thread(), |map| map.look(drawn))
}

/// project.insert (after patch 130's): a project whose config names its
/// inscatter distance makes its scene the 0.11 weather's.
pub fn on_project_insert(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let project = ProjectView::at_insert(ctx).expect("project.insert's project");
    let root = project.root().ok().and_then(|root| root.table().ok()).expect("a project's config has a root table");
    if let Some(value) = root.get(MAP_KEY).expect("a project's config reads") {
        let ConfigValue::Table(table) = value else { panic!("srp_renderer_v011w is a table, the weather's profile") };
        let profile = Profile::of_table(table);
        let scene = project.render_scene().expect("a project being inserted has its scene");
        MAP.with(ctx.thread(), |map| *map = map.step(MapEvent::Inserted { project: project.id(), scene, profile }));
        log::line(&[Text("v011w_render: a 0.11 weather; its scene's atmosphere and post drawn the 0.11 way")]);
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

/// project.destroy: as removed.
pub fn on_project_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    gone(ctx, ProjectId::destroyed(ctx));
    Flow::Continue
}

/// The geometry stage's fog pass: 0.11e's multiplicative one on the 0.11
/// weather's scene.
pub fn on_geometry_fog(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnGeometryFogExit> {
    match look(ctx, StageSite::GeometryFog) {
        Look::V011w(Profile { distance, .. }) => {
            render_stage::set_fog_technique(ctx, render_stage::FOG_MULTIPLY);
            FOG.with(ctx.thread(), |fog| *fog = fog.step(FogEvent::Multiply(distance)).0);
            Flow::Exit(vostok::V011WRenderOnGeometryFogExit::Applied)
        }
        Look::Native => Flow::Continue,
    }
}

/// After its apply_pass: the multiplicative pass's inscatter distance.
pub fn on_geometry_fog_applied(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Fog::Multiplied(Distance(metres)) = FOG.with(ctx.thread(), |fog| *fog) {
        render_stage::set_multiply_constants(ctx, metres);
    }
    Flow::Continue
}

/// The god rays' entry: in the geometry stage, after the multiplicative
/// pass, the additive one.
pub fn on_god_rays(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if render_stage::god_rays_stage(ctx) == Some(AtmosphereStage::Geometry) {
        let action = FOG.with(ctx.thread(), |fog| {
            let (next, action) = fog.step(FogEvent::GodRays);
            *fog = next;
            action
        });
        if let FogAction::Add(Distance(metres)) = action {
            render_stage::draw_fog_add(ctx, metres);
        }
    }
    Flow::Continue
}

/// effect_atmospheric_scattering::compile's end: the first time back to
/// technique 0, for the twins; the second time its return.
pub fn on_scattering_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnScatteringCompiledExit> {
    use vostok::V011WRenderOnScatteringCompiledExit as Exit;
    let compiler = effect_compile::compiler_id(ctx, CompileSite::ScatteringEnd);
    match ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler, Twins::Scattering)) {
        End::Return => Flow::Continue,
        End::Twins => Flow::Exit(match effect_compile::pass(ctx, CompileSite::ScatteringEnd) {
            Pass::Building => Exit::Begin,
            Pass::Caching => Exit::Cached,
        }),
    }
}

fn twin_pass(ctx: &Context<'_>, site: CompileSite, pass: PassSite, twins: Twins, name: &'static CStr) {
    let compiler = effect_compile::compiler_id(ctx, site);
    if ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.twins(compiler)) == Some(twins) {
        effect_compile::set_pixel_shader(ctx, pass, name);
    }
}

/// Its technique 0 (the tables) begins its pass: the twin's with 0.11e's
/// pixel shader.
pub fn on_scattering_tables_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::ScatteringTablesPass, PassSite::ScatteringTables, Twins::Scattering, TABLES_PS);
    Flow::Continue
}

/// Its technique 1 (the sky): the twin's with 0.11e's.
pub fn on_scattering_sky_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::ScatteringSkyPass, PassSite::ScatteringSky, Twins::Scattering, SKY_PS);
    Flow::Continue
}

/// The sky stage decides whether to remake the tables: also when they were
/// made with the other look's programs.
pub fn on_tables_due(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let programs = look(ctx, StageSite::TablesDue).programs();
    let action = TABLES.with(ctx.thread(), |tables| {
        let (next, action) = tables.step(TablesEvent::Due(programs));
        *tables = next;
        action
    });
    if action == TablesAction::Remake {
        render_stage::remake_tables(ctx);
    }
    Flow::Continue
}

/// The tables are made: with 0.11e's programs on the 0.11 weather's scene.
pub fn on_tables_apply(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnTablesApplyExit> {
    let programs = look(ctx, StageSite::TablesApply).programs();
    TABLES.with(ctx.thread(), |tables| *tables = tables.step(TablesEvent::Making(programs)).0);
    match programs {
        Programs::V011w => {
            render_stage::apply_technique(ctx, ApplySite::Tables, render_stage::TABLES_V011W);
            Flow::Exit(vostok::V011WRenderOnTablesApplyExit::Apply)
        }
        Programs::Native => Flow::Continue,
    }
}

/// The sky's pass: 0.11e's on the 0.11 weather's scene.
pub fn on_sky_apply(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnSkyApplyExit> {
    match look(ctx, StageSite::SkyApply).programs() {
        Programs::V011w => {
            render_stage::set_sky_technique(ctx, render_stage::SKY_V011W);
            Flow::Exit(vostok::V011WRenderOnSkyApplyExit::Applied)
        }
        Programs::Native => Flow::Continue,
    }
}

/// An effect_complex_post_process_blend compile's end: the first time back to
/// its start, for the twin; the second time its return.
fn post_blend_end(ctx: &Context<'_>, site: CompileSite) -> End {
    let compiler = effect_compile::compiler_id(ctx, site);
    ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler, Twins::PostBlend))
}

pub fn on_post_blend_00_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnPostBlend00CompiledExit> {
    match post_blend_end(ctx, CompileSite::PostBlend00End) {
        End::Twins => Flow::Exit(vostok::V011WRenderOnPostBlend00CompiledExit::Begin),
        End::Return => Flow::Continue,
    }
}

pub fn on_post_blend_10_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnPostBlend10CompiledExit> {
    match post_blend_end(ctx, CompileSite::PostBlend10End) {
        End::Twins => Flow::Exit(vostok::V011WRenderOnPostBlend10CompiledExit::Begin),
        End::Return => Flow::Continue,
    }
}

pub fn on_post_blend_11_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnPostBlend11CompiledExit> {
    match post_blend_end(ctx, CompileSite::PostBlend11End) {
        End::Twins => Flow::Exit(vostok::V011WRenderOnPostBlend11CompiledExit::Begin),
        End::Return => Flow::Continue,
    }
}

/// A post blend's technique begins its pass: the twin's with 0.11e's post.
pub fn on_post_blend_00_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::PostBlend00Pass, PassSite::PostBlend00, Twins::PostBlend, POST_PS);
    Flow::Continue
}

pub fn on_post_blend_10_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::PostBlend10Pass, PassSite::PostBlend10, Twins::PostBlend, POST_PS);
    Flow::Continue
}

pub fn on_post_blend_11_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::PostBlend11Pass, PassSite::PostBlend11, Twins::PostBlend, POST_PS);
    Flow::Continue
}

/// The post blend's pass: 0.11e's on the 0.11 weather's scene.
pub fn on_post_blend_apply(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnPostBlendApplyExit> {
    match look(ctx, StageSite::PostBlendApply) {
        Look::V011w(Profile { post: Post::Filmic(_), .. }) => {
            render_stage::apply_technique(ctx, ApplySite::PostBlend, render_stage::POST_V011W);
            Flow::Exit(vostok::V011WRenderOnPostBlendApplyExit::Apply)
        }
        Look::V011w(Profile { post: Post::Native, .. }) | Look::Native => Flow::Continue,
    }
}

/// Its constants set: on the 0.11 weather's scene the filmic curve, which
/// 0.26g never sets.
pub fn on_post_blend_constants(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Look::V011w(Profile { post: Post::Filmic([packed_0, packed_1]), .. }) = look(ctx, StageSite::PostBlendConstants) {
        render_stage::set_filmic(ctx, packed_0, packed_1);
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
    fn the_map_is_its_project_scene_with_its_distance() {
        let map = Map::Native.step(MapEvent::Inserted { project: project(1), scene: scene(10), profile: evening() });
        assert_eq!(map.look(Some(scene(10))), Look::V011w(evening()));
        assert_eq!(map.look(Some(scene(20))), Look::Native);
        assert_eq!(map.look(None), Look::Native);
        assert_eq!(map.step(MapEvent::Gone(project(2))), map);
        assert_eq!(map.step(MapEvent::Gone(project(1))), Map::Native);
    }

    #[test]
    fn the_additive_pass_follows_only_a_multiplicative_one() {
        assert_eq!(Fog::Idle.step(FogEvent::GodRays), (Fog::Idle, FogAction::None));
        let (fog, action) = Fog::Idle.step(FogEvent::Multiply(Distance(300.0)));
        assert_eq!(action, FogAction::None);
        assert_eq!(fog.step(FogEvent::GodRays), (Fog::Idle, FogAction::Add(Distance(300.0))));
        // a frame whose god rays never came (a lost device) is overtaken by the next
        assert_eq!(fog.step(FogEvent::Multiply(Distance(200.0))).0, Fog::Multiplied(Distance(200.0)));
    }

    #[test]
    fn the_tables_are_remade_when_the_look_changes() {
        use Programs::{Native, V011w};
        // the game makes the first ones itself
        assert_eq!(Tables::Unmade.step(TablesEvent::Due(V011w)), (Tables::Unmade, TablesAction::None));
        let (made, _) = Tables::Unmade.step(TablesEvent::Making(Native));
        assert_eq!(made, Tables::Made(Native));
        assert_eq!(made.step(TablesEvent::Due(Native)), (made, TablesAction::None));
        assert_eq!(made.step(TablesEvent::Due(V011w)), (made, TablesAction::Remake));
        let (made, _) = made.step(TablesEvent::Making(V011w));
        assert_eq!(made.step(TablesEvent::Due(V011w)), (Tables::Made(V011w), TablesAction::None));
        assert_eq!(made.step(TablesEvent::Due(Native)), (made, TablesAction::Remake));
    }

    /// 0.11e's Vostok evening (its post material level_03_evn).
    fn evening() -> Profile {
        Profile::parse(|key| {
            Some(match key.to_bytes() {
                b"inscatter_distance" => 200.0,
                b"shoulder_strength" => 0.05,
                b"linear_strength" => 7.0,
                b"linear_angle" => 0.3,
                b"toe_strength" => 6.2,
                b"toe_numerator" => 2.0,
                b"toe_denominator" => 0.02,
                b"linear_white" => 10000.0,
                b"dark_desaturation_limit" => 4.5,
                other => panic!("no {other:?}"),
            })
        })
    }

    #[test]
    fn a_profile_packs_the_filmic_curve_as_0_11e() {
        let profile = evening();
        assert_eq!(profile.distance, Distance(200.0));
        assert_eq!(profile.post, Post::Filmic([[0.05, 7.0, 0.3, 6.2], [2.0, 0.02, 10000.0, 4.5]]));
        // without the curve the post stays 0.26g's
        let distance_only = |key: &CStr| (key == c"inscatter_distance").then_some(300.0);
        assert_eq!(Profile::parse(distance_only), Profile { distance: Distance(300.0), post: Post::Native });
        // all eight or none
        assert!(std::panic::catch_unwind(|| Profile::parse(|key| {
            (key == c"inscatter_distance" || key == c"shoulder_strength").then_some(1.0)
        }))
        .is_err());
        // the distance divides both fog passes
        assert!(std::panic::catch_unwind(|| Profile::parse(|_| Some(0.0))).is_err());
    }
}
