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

use crate::vostok::config::ConfigValue;
use crate::vostok::effect_compile::{self, CompileSite, End, Pass, PassSite, Rounds};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::project::{ProjectId, ProjectView};
use crate::vostok::render_stage::{self, ApplySite, AtmosphereStage, RenderSceneId, StageSite};
use crate::vostok::{self, Context, Flow, Lock, NoExit};

/// The key of a project config's root that makes its map a 0.11 weather:
/// the weather's inscatter distance in metres (0.11e's post-process
/// material's `atmosphere_inscatter_distance`).
pub const MAP_KEY: &CStr = c"srp_renderer_v011w";

/// The twins' pixel shaders (shaders/sm_4_0/<name>.ps/ + 67 `_`, the content
/// pack's): 0.11e's scattering tables and sky.
const TABLES_PS: &CStr = c"make_mie_rayleigh_texture_v011w";
const SKY_PS: &CStr = c"atmosphere_v011w";

/// A project's inscatter distance, metres: above 0 (both passes divide by it).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Distance(f32);

impl Distance {
    fn parse(value: ConfigValue<'_>) -> Distance {
        let metres = match value {
            ConfigValue::Integer(metres) => metres as f32,
            ConfigValue::Number(metres) => metres,
            _ => panic!("srp_renderer_v011w is a number of metres"),
        };
        assert!(metres.is_finite() && metres > 0.0, "srp_renderer_v011w is a distance above 0");
        Distance(metres)
    }
}

/// The map whose scene is fogged the 0.11 way, if one is loaded.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Map {
    Native,
    V011w { project: ProjectId, scene: RenderSceneId, distance: Distance },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MapEvent {
    Inserted { project: ProjectId, scene: RenderSceneId, distance: Distance },
    Gone(ProjectId),
}

impl Map {
    fn step(self, event: MapEvent) -> Map {
        match (self, event) {
            (_, MapEvent::Inserted { project, scene, distance }) => Map::V011w { project, scene, distance },
            (Map::V011w { project, .. }, MapEvent::Gone(gone)) if project == gone => Map::Native,
            (map, MapEvent::Gone(_)) => map,
        }
    }

    /// How the frame drawing `drawn` looks.
    fn look(self, drawn: Option<RenderSceneId>) -> Look {
        match self {
            Map::V011w { scene, distance, .. } if Some(scene) == drawn => Look::V011w(distance),
            Map::V011w { .. } | Map::Native => Look::Native,
        }
    }
}

/// How a frame's atmosphere is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Native,
    /// The 0.11 weather's scene, with its inscatter distance.
    V011w(Distance),
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

/// The effect this patch compiles twins for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Twins {
    Scattering,
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
        let distance = Distance::parse(value);
        let scene = project.render_scene().expect("a project being inserted has its scene");
        MAP.with(ctx.thread(), |map| *map = map.step(MapEvent::Inserted { project: project.id(), scene, distance }));
        log::line(&[Text("v011w_render: a 0.11 weather; its scene's geometry fogged the 0.11 way")]);
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
        Look::V011w(distance) => {
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

fn twin_pass(ctx: &Context<'_>, site: CompileSite, pass: PassSite, name: &'static CStr) {
    let compiler = effect_compile::compiler_id(ctx, site);
    if ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.twins(compiler)) == Some(Twins::Scattering) {
        effect_compile::set_pixel_shader(ctx, pass, name);
    }
}

/// Its technique 0 (the tables) begins its pass: the twin's with 0.11e's
/// pixel shader.
pub fn on_scattering_tables_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::ScatteringTablesPass, PassSite::ScatteringTables, TABLES_PS);
    Flow::Continue
}

/// Its technique 1 (the sky): the twin's with 0.11e's.
pub fn on_scattering_sky_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::ScatteringSkyPass, PassSite::ScatteringSky, SKY_PS);
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
        let map = Map::Native.step(MapEvent::Inserted { project: project(1), scene: scene(10), distance: Distance(200.0) });
        assert_eq!(map.look(Some(scene(10))), Look::V011w(Distance(200.0)));
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

    #[test]
    fn the_distance_is_a_number_above_0() {
        assert_eq!(Distance::parse(ConfigValue::Integer(200)), Distance(200.0));
        assert_eq!(Distance::parse(ConfigValue::Number(300.0)), Distance(300.0));
        assert!(std::panic::catch_unwind(|| Distance::parse(ConfigValue::Boolean(true))).is_err());
        assert!(std::panic::catch_unwind(|| Distance::parse(ConfigValue::Number(0.0))).is_err());
    }
}
