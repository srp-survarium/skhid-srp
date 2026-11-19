//! The ported 0.11 weathers render the 0.11 way what their profile names
//! (Vostok Radar Station v0.11's evening and rain;
//! srp-026g/docs/notes/v011-weather.md): the geometry's fog, the scattering
//! tables and the sky (with the inscatter distance), the clouds, the skylight
//! and the post. A profile names any of them, so a weather can keep 0.26g's
//! atmosphere and take only some of 0.11e's.
//!
//! 0.11e draws the geometry's fog in two passes of its scattering effect: it
//! multiplies the frame by 1 - min(depth, D) / D / 10, then adds the light
//! the air scatters toward the eye, read from the Rayleigh and Mie tables;
//! D is the weather's inscatter distance. 0.26g draws one pass of another
//! equation (technique 5) but still compiles 0.11e's two (3 and 4). On the
//! scene of a project whose profile ([`MAP_KEY`]) names its inscatter
//! distance the geometry stage draws 3 in 5's place and 4 after it; the
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
//! The light cones under Vostok v0.11's lamps are 0.11e's too, wherever their
//! material is compiled (the day as well as the weathers): a material whose
//! effect config has [`CONE_KEY`] begins its forward_volume_cone_base passes
//! with 0.11e's program (its sphere falloff and distance fade, [`cone_twin`]).
//!
//! Presentation only: nothing a match computes reads any of it.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::{ConfigTable, ConfigValue};
use crate::vostok::effect_compile::{self, CloudEffect, CloudPass, CompileSite, End, MaterialPass, Pass, PassSite, Rounds, VertexInput};
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
const POST_PS: &CStr = c"complex_post_process_blend_v011w_026g";
const SKYLIGHT_PS: &CStr = c"skylight_v011w";
const CLOUDS_ALPHA_BLEND_PS: &CStr = c"atmosphere_clouds_alpha_blend_v011w";
const CLOUDS_MODULATE_PS: &CStr = c"atmosphere_clouds_modulate_v011w";
/// 0.11e's gamma copy (pow(x, 1/(2.2 gamma)), 0.26g's copy_image.vs inputs).
const COPY_PS: &CStr = c"copy_image_v011w_026g";

/// The key of a material's effect config (`material/forward/effect`) whose
/// volume cones are 0.11e's; only its presence counts.
pub const CONE_KEY: &CStr = c"srp_cone_v011w";
/// 0.26g's cone family, the one a marked material's pass swaps.
const CONE_PS: &CStr = c"forward_volume_cone_base";
/// 0.11e's cone program for 0.26g's vertex outputs of each input type it is
/// built for (shaders/sm_4_0/<name>.ps/ + 67 `_`, the content pack's; the
/// diffuse texture on, no transparency texture, as the ported material).
const CONE_TWINS: [&CStr; 6] = [
    c"forward_volume_cone_base_v011w_1",
    c"forward_volume_cone_base_v011w_2",
    c"forward_volume_cone_base_v011w_3",
    c"forward_volume_cone_base_v011w_4",
    c"forward_volume_cone_base_v011w_5",
    c"forward_volume_cone_base_v011w_6",
];

/// A project's inscatter distance, metres: above 0 (both passes divide by it).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Distance(f32);

/// A 0.11 weather's profile, its `MAP_KEY` table: 0.11e's post material's
/// `atmosphere_inscatter_distance` and, when the table names it, its filmic
/// curve (all eight [`FILMIC_KEYS`] or none).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Profile {
    atmosphere: Atmosphere,
    sky: Sky,
    clouds: Clouds,
    post: Post,
}

/// A 0.11 weather's atmosphere: 0.11e's fog, scattering tables and sky when
/// the profile names its inscatter distance, else 0.26g's.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Atmosphere {
    Native,
    V011w(Distance),
}

/// A 0.11 weather's sky clouds: 0.11e's programs and parameters when the
/// profile names them, else 0.26g's.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Clouds {
    Native,
    V011w(CloudLayer),
}

/// 0.11e's cloud values (its post material's): `clouds_color` the clouds'
/// linear colour (sky_clouds_color^2.2 * sky_clouds_color_multiplier, as
/// 0.11e's material_manager computes it), the u tiling, the horizon fade's
/// power and band (sky_clouds_fog_power, sky_clouds_fog_up_limit).
#[derive(Clone, Copy, Debug, PartialEq)]
struct CloudLayer {
    color: [f32; 3],
    u_tile: f32,
    fade_power: f32,
    fade_band: f32,
}

/// The clouds' number keys (`clouds_color` is the vector).
const CLOUD_NUMBER_KEYS: [&CStr; 3] = [c"clouds_u_tile", c"clouds_fog_power", c"clouds_fog_up_limit"];

impl CloudLayer {
    /// sky_clouds_parameters0 and 1 as 0.11e's stage_atmosphere sets them.
    fn parameters(&self) -> ([f32; 4], [f32; 4]) {
        let [r, g, b] = self.color;
        ([r, g, b, self.u_tile], [self.fade_power, self.fade_band, 0.0, 0.0])
    }
}

/// A 0.11 weather's skylight: 0.11e's (the twin, lighting shade by height
/// and a six-face sky colour) when the profile names its values, else 0.26g's.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sky {
    Native,
    V011w(Skylight),
}

/// 0.11e's skylight values (its post material's): `skylight_light` the
/// light, linear (skylight_color^2.2 * skylight_multiplier, as 0.11e's
/// stage_ambient_lighting computes it); the lower and upper heights and the
/// power; the six faces' sky colours (+x, -x, +y up, -y down, +z, -z); the
/// sun's tint (environment_suncolor * environment_sun_color).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Skylight {
    light: [f32; 3],
    lower: f32,
    upper: f32,
    power: f32,
    faces: [[f32; 3]; 6],
    sun_tint: [f32; 3],
}

/// The skylight's keys: the numbers, then the vectors.
const SKY_NUMBER_KEYS: [&CStr; 3] = [c"skylight_lower", c"skylight_upper", c"skylight_power"];
const SKY_VECTOR_KEYS: [&CStr; 8] = [
    c"skylight_light",
    c"environment_skycolor_x",
    c"environment_skycolor_minus_x",
    c"environment_skycolor_up",
    c"environment_skycolor_down",
    c"environment_skycolor_z",
    c"environment_skycolor_minus_z",
    c"sun_tint",
];

impl Skylight {
    /// skylight_parameters0..7 as 0.11e's stage_ambient_lighting::execute
    /// packs them, for a sun of `direction` (toward the ground) and
    /// `intensity`.
    fn parameters(&self, direction: [f32; 3], intensity: f32) -> [[f32; 4]; 8] {
        let [plus_x, minus_x, plus_y, minus_y, plus_z, minus_z] = self.faces;
        let with = |v: [f32; 3], w: f32| [v[0], v[1], v[2], w];
        let sun = self.sun_tint.map(|c| c * intensity);
        [
            with(self.light, 0.0),
            [self.lower, self.upper - self.lower, self.power, 0.0],
            with(plus_x, plus_z[0]),
            with(minus_x, plus_z[1]),
            with(minus_y, plus_z[2]),
            with(minus_z, plus_y[0]),
            with(direction, plus_y[1]),
            with(sun, plus_y[2]),
        ]
    }
}

/// A 0.11 weather's post.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Post {
    /// 0.26g's blend.
    Native,
    /// 0.11e's (the twin) and its gamma copy at the screen, with its filmic curve packed as 0.11e's
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
    fn parse(number: impl Fn(&CStr) -> Option<f32>, vector: impl Fn(&CStr) -> Option<[f32; 3]>) -> Profile {
        let atmosphere = match number(c"inscatter_distance") {
            None => Atmosphere::Native,
            Some(metres) => {
                assert!(metres.is_finite() && metres > 0.0, "srp_renderer_v011w.inscatter_distance is a distance above 0");
                Atmosphere::V011w(Distance(metres))
            }
        };
        let values = FILMIC_KEYS.map(&number);
        let post = if values.iter().all(Option::is_none) {
            Post::Native
        } else {
            let v = values.map(|value| value.expect("srp_renderer_v011w names all eight filmic values or none"));
            Post::Filmic([[v[0], v[1], v[2], v[3]], [v[4], v[5], v[6], v[7]]])
        };
        let numbers = SKY_NUMBER_KEYS.map(&number);
        let vectors = SKY_VECTOR_KEYS.map(&vector);
        let sky = if numbers.iter().all(Option::is_none) && vectors.iter().all(Option::is_none) {
            Sky::Native
        } else {
            let n = numbers.map(|value| value.expect("srp_renderer_v011w names all the skylight's values or none"));
            let v = vectors.map(|value| value.expect("srp_renderer_v011w names all the skylight's values or none"));
            Sky::V011w(Skylight {
                light: v[0],
                lower: n[0],
                upper: n[1],
                power: n[2],
                faces: [v[1], v[2], v[3], v[4], v[5], v[6]],
                sun_tint: v[7],
            })
        };
        let numbers = CLOUD_NUMBER_KEYS.map(&number);
        let color = vector(c"clouds_color");
        let clouds = if numbers.iter().all(Option::is_none) && color.is_none() {
            Clouds::Native
        } else {
            let all = "srp_renderer_v011w names all the clouds' values or none";
            let n = numbers.map(|value| value.expect(all));
            Clouds::V011w(CloudLayer { color: color.expect(all), u_tile: n[0], fade_power: n[1], fade_band: n[2] })
        };
        Profile { atmosphere, sky, clouds, post }
    }

    fn of_table(table: ConfigTable<'_>) -> Profile {
        let value = |key: &CStr| table.get(key).expect("a project's config reads");
        Profile::parse(
            |key| match value(key) {
                None => None,
                Some(ConfigValue::Integer(n)) => Some(n as f32),
                Some(ConfigValue::Number(n)) => Some(n),
                Some(_) => panic!("srp_renderer_v011w: {key:?} is a number"),
            },
            |key| match value(key) {
                None => None,
                Some(ConfigValue::Vector(v)) => Some(v.xyz().expect("a colour of three or four")),
                Some(_) => panic!("srp_renderer_v011w: {key:?} is a vector"),
            },
        )
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
            Look::V011w(Profile { atmosphere: Atmosphere::V011w(_), .. }) => Programs::V011w,
            Look::V011w(Profile { atmosphere: Atmosphere::Native, .. }) => Programs::Native,
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
    Skylight,
    Clouds,
    Copy,
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

/// project.insert (after patch 130's): a project whose config root has a
/// profile ([`MAP_KEY`]) makes its scene the 0.11 weather's.
pub fn on_project_insert(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let project = ProjectView::at_insert(ctx).expect("project.insert's project");
    let root = project.root().ok().and_then(|root| root.table().ok()).expect("a project's config has a root table");
    if let Some(value) = root.get(MAP_KEY).expect("a project's config reads") {
        let ConfigValue::Table(table) = value else { panic!("srp_renderer_v011w is a table, the weather's profile") };
        let profile = Profile::of_table(table);
        let scene = project.render_scene().expect("a project being inserted has its scene");
        MAP.with(ctx.thread(), |map| *map = map.step(MapEvent::Inserted { project: project.id(), scene, profile }));
        log::line(&[Text("v011w_render: a 0.11 weather; its scene drawn the 0.11 way where its profile says")]);
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
        Look::V011w(Profile { atmosphere: Atmosphere::V011w(distance), .. }) => {
            render_stage::set_fog_technique(ctx, render_stage::FOG_MULTIPLY);
            FOG.with(ctx.thread(), |fog| *fog = fog.step(FogEvent::Multiply(distance)).0);
            Flow::Exit(vostok::V011WRenderOnGeometryFogExit::Applied)
        }
        Look::V011w(Profile { atmosphere: Atmosphere::Native, .. }) | Look::Native => Flow::Continue,
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

/// The tables' constant: on the 0.11 weather's scene 0.11e's four
/// multipliers, which its tables program reads.
pub fn on_tables_constants(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if look(ctx, StageSite::TablesConstants).programs() == Programs::V011w {
        render_stage::set_scattering_multipliers(ctx);
    }
    Flow::Continue
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

/// effect_copy_image::compile's end: the first time back to technique 0, for
/// the twins (2, 3); the second time its return.
pub fn on_copy_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnCopyCompiledExit> {
    use vostok::V011WRenderOnCopyCompiledExit as Exit;
    let compiler = effect_compile::compiler_id(ctx, CompileSite::CopyEnd);
    match ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler, Twins::Copy)) {
        End::Return => Flow::Continue,
        End::Twins => Flow::Exit(match effect_compile::pass(ctx, CompileSite::CopyEnd) {
            Pass::Building => Exit::Begin,
            Pass::Caching => Exit::Cached,
        }),
    }
}

/// Its technique 0 (the gamma copy) begins its pass: the twin's with 0.11e's.
pub fn on_copy_pass_0(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::CopyPass0, PassSite::Copy, Twins::Copy, COPY_PS);
    Flow::Continue
}

/// The screen's gamma copy: 0.11e's for a frame of the 0.11 weather's scene
/// whose profile names 0.11e's post.
pub fn on_copy_apply(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnCopyApplyExit> {
    match look(ctx, StageSite::CopyApply) {
        Look::V011w(Profile { post: Post::Filmic(_), .. }) => {
            render_stage::set_copy_technique(ctx, render_stage::COPY_V011W);
            Flow::Exit(vostok::V011WRenderOnCopyApplyExit::Applied)
        }
        Look::V011w(Profile { post: Post::Native, .. }) | Look::Native => Flow::Continue,
    }
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

/// effect_skylight::compile's end: the first time back to technique 0, for
/// the twins; the second time its return.
pub fn on_skylight_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnSkylightCompiledExit> {
    use vostok::V011WRenderOnSkylightCompiledExit as Exit;
    let compiler = effect_compile::compiler_id(ctx, CompileSite::SkylightEnd);
    match ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler, Twins::Skylight)) {
        End::Return => Flow::Continue,
        End::Twins => Flow::Exit(match effect_compile::pass(ctx, CompileSite::SkylightEnd) {
            Pass::Building => Exit::Begin,
            Pass::Caching => Exit::Cached,
        }),
    }
}

/// effect_skylight's techniques begin their passes: the twins' with 0.11e's.
pub fn on_skylight_pass_0(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::SkylightPass0, PassSite::Skylight0, Twins::Skylight, SKYLIGHT_PS);
    Flow::Continue
}

pub fn on_skylight_pass_1(ctx: &mut Context<'_>) -> Flow<NoExit> {
    twin_pass(ctx, CompileSite::SkylightPass1, PassSite::Skylight1, Twins::Skylight, SKYLIGHT_PS);
    Flow::Continue
}

/// The skylight's gate: 0.26g never draws its skylight (the stage's +E4h,
/// zeroed by its constructor, is never set); a 0.11 weather's scene whose
/// profile names the skylight draws it.
pub fn on_skylight_gate(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnSkylightGateExit> {
    match look(ctx, StageSite::SkylightGate) {
        Look::V011w(Profile { sky: Sky::V011w(_), .. }) => Flow::Exit(vostok::V011WRenderOnSkylightGateExit::Render),
        Look::V011w(Profile { sky: Sky::Native, .. }) | Look::Native => Flow::Continue,
    }
}

/// The skylight's pass: 0.11e's on the 0.11 weather's scene whose profile
/// names the skylight.
pub fn on_skylight_apply(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnSkylightApplyExit> {
    match look(ctx, StageSite::SkylightApply) {
        Look::V011w(Profile { sky: Sky::V011w(_), .. }) => {
            render_stage::set_skylight_technique(ctx, render_stage::SKYLIGHT_V011W);
            Flow::Exit(vostok::V011WRenderOnSkylightApplyExit::Applied)
        }
        Look::V011w(Profile { sky: Sky::Native, .. }) | Look::Native => Flow::Continue,
    }
}

/// Its constants: 0.11e's eight, which 0.26g does not set (but the first).
pub fn on_skylight_constants(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Look::V011w(Profile { sky: Sky::V011w(skylight), .. }) = look(ctx, StageSite::SkylightConstants) {
        let (direction, intensity) = render_stage::skylight_sun(ctx);
        render_stage::set_skylight(ctx, &skylight.parameters(direction, intensity));
    }
    Flow::Continue
}

/// An effect_atmosphere_clouds compile's ret: the first time back to its
/// start (a fresh call), for the twins; the second time its return.
fn clouds_end(ctx: &Context<'_>, effect: CloudEffect) -> End {
    let compiler = effect_compile::compiler_id(ctx, CompileSite::CloudsEnd(effect));
    ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler, Twins::Clouds))
}

pub fn on_clouds_00_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnClouds00CompiledExit> {
    match clouds_end(ctx, CloudEffect::C00) {
        End::Twins => Flow::Exit(vostok::V011WRenderOnClouds00CompiledExit::Entry),
        End::Return => Flow::Continue,
    }
}

pub fn on_clouds_10_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnClouds10CompiledExit> {
    match clouds_end(ctx, CloudEffect::C10) {
        End::Twins => Flow::Exit(vostok::V011WRenderOnClouds10CompiledExit::Entry),
        End::Return => Flow::Continue,
    }
}

pub fn on_clouds_01_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnClouds01CompiledExit> {
    match clouds_end(ctx, CloudEffect::C01) {
        End::Twins => Flow::Exit(vostok::V011WRenderOnClouds01CompiledExit::Entry),
        End::Return => Flow::Continue,
    }
}

pub fn on_clouds_11_compiled(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnClouds11CompiledExit> {
    match clouds_end(ctx, CloudEffect::C11) {
        End::Twins => Flow::Exit(vostok::V011WRenderOnClouds11CompiledExit::Entry),
        End::Return => Flow::Continue,
    }
}

/// A cloud technique begins its pass: the twin's with 0.11e's program.
fn clouds_pass(ctx: &Context<'_>, effect: CloudEffect, pass: CloudPass) {
    let name = match pass {
        CloudPass::AlphaBlend => CLOUDS_ALPHA_BLEND_PS,
        CloudPass::Modulate => CLOUDS_MODULATE_PS,
    };
    twin_pass(ctx, CompileSite::CloudsPass(effect, pass), PassSite::Clouds(effect, pass), Twins::Clouds, name);
}

pub fn on_clouds_00_pass_0(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C00, CloudPass::AlphaBlend);
    Flow::Continue
}

pub fn on_clouds_00_pass_1(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C00, CloudPass::Modulate);
    Flow::Continue
}

pub fn on_clouds_10_pass_0(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C10, CloudPass::AlphaBlend);
    Flow::Continue
}

pub fn on_clouds_10_pass_1(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C10, CloudPass::Modulate);
    Flow::Continue
}

pub fn on_clouds_01_pass_0(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C01, CloudPass::AlphaBlend);
    Flow::Continue
}

pub fn on_clouds_01_pass_1(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C01, CloudPass::Modulate);
    Flow::Continue
}

pub fn on_clouds_11_pass_0(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C11, CloudPass::AlphaBlend);
    Flow::Continue
}

pub fn on_clouds_11_pass_1(ctx: &mut Context<'_>) -> Flow<NoExit> {
    clouds_pass(ctx, CloudEffect::C11, CloudPass::Modulate);
    Flow::Continue
}

/// The clouds' pass: 0.11e's on the 0.11 weather's scene whose profile names
/// the clouds.
pub fn on_clouds_apply(ctx: &mut Context<'_>) -> Flow<vostok::V011WRenderOnCloudsApplyExit> {
    match look(ctx, StageSite::CloudsApply) {
        Look::V011w(Profile { clouds: Clouds::V011w(_), .. }) => {
            render_stage::set_clouds_technique(ctx);
            Flow::Exit(vostok::V011WRenderOnCloudsApplyExit::Apply)
        }
        Look::V011w(Profile { clouds: Clouds::Native, .. }) | Look::Native => Flow::Continue,
    }
}

/// Their constants: 0.11e's parameters 0 and 1 over 0.26g's.
pub fn on_clouds_constants(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Look::V011w(Profile { clouds: Clouds::V011w(layer), .. }) = look(ctx, StageSite::CloudsConstants) {
        let (parameters_0, parameters_1) = layer.parameters();
        render_stage::set_clouds(ctx, parameters_0, parameters_1);
    }
    Flow::Continue
}

/// The cone twin of a marked material's pass: 0.26g's cone family with a
/// vertex input the pack builds a twin for (1 to 6, the static meshes').
fn cone_twin(input: VertexInput, family: &CStr, marked: bool) -> Option<&'static CStr> {
    if !marked || family != CONE_PS {
        return None;
    }
    CONE_TWINS.get(usize::from(input.number()).checked_sub(1)?).copied()
}

/// A material effect's pass: a marked material's cones take 0.11e's program.
pub fn on_material_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut pass) = MaterialPass::at(ctx)
        && let Some(family) = pass.pixel_shader()
        && let Some(twin) = cone_twin(pass.vertex_input(), family, has_key(pass.config(), CONE_KEY))
    {
        pass.set_pixel_shader(twin);
    }
    Flow::Continue
}

fn has_key(config: Option<&ConfigTable<'_>>, key: &CStr) -> bool {
    config.is_some_and(|table| matches!(table.get(key), Ok(Some(_))))
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

    /// 0.11e's Vostok evening (its post material level_03_evn), its numbers.
    fn evening_number(key: &CStr) -> Option<f32> {
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
            b"skylight_lower" => 0.0,
            b"skylight_upper" => 5.0,
            b"skylight_power" => 0.1,
            b"clouds_u_tile" => 1.0,
            b"clouds_fog_power" => 10.0,
            b"clouds_fog_up_limit" => 0.15,
            other => panic!("no {other:?}"),
        })
    }

    /// Its vectors: the light (0.11e's colour^2.2 * 0.5), the faces, no sun
    /// tint; the clouds' colour (0.11e's colour^2.2 * 2.5).
    fn evening_vector(key: &CStr) -> Option<[f32; 3]> {
        const SIDE: [f32; 3] = [0.509_804, 0.529_412, 0.564_706];
        Some(match key.to_bytes() {
            b"skylight_light" => [0.32, 0.39, 0.5],
            b"environment_skycolor_x" | b"environment_skycolor_minus_x" => SIDE,
            b"environment_skycolor_z" | b"environment_skycolor_minus_z" => SIDE,
            b"environment_skycolor_up" | b"environment_skycolor_down" => [1.0, 1.0, 1.0],
            b"sun_tint" => [0.0, 0.0, 0.0],
            b"clouds_color" => [2.296, 1.382, 0.748],
            other => panic!("no {other:?}"),
        })
    }

    fn evening() -> Profile {
        Profile::parse(evening_number, evening_vector)
    }

    fn none(_: &CStr) -> Option<[f32; 3]> {
        None
    }

    #[test]
    fn a_profile_packs_the_filmic_curve_as_0_11e() {
        let profile = evening();
        assert_eq!(profile.atmosphere, Atmosphere::V011w(Distance(200.0)));
        assert_eq!(profile.post, Post::Filmic([[0.05, 7.0, 0.3, 6.2], [2.0, 0.02, 10000.0, 4.5]]));
        // without the curve and the sky, 0.26g's post and skylight
        let distance_only = |key: &CStr| (key == c"inscatter_distance").then_some(300.0);
        assert_eq!(
            Profile::parse(distance_only, none),
            Profile { atmosphere: Atmosphere::V011w(Distance(300.0)), sky: Sky::Native, clouds: Clouds::Native, post: Post::Native }
        );
        // all eight or none
        assert!(std::panic::catch_unwind(|| Profile::parse(
            |key| (key == c"inscatter_distance" || key == c"shoulder_strength").then_some(1.0),
            none
        ))
        .is_err());
        // without the distance, 0.26g's fog, tables and sky
        assert_eq!(Profile::parse(|_| None, none).atmosphere, Atmosphere::Native);
        // the distance divides both fog passes
        assert!(std::panic::catch_unwind(|| Profile::parse(|_| Some(0.0), none)).is_err());
    }

    #[test]
    fn the_skylight_packs_as_0_11e_stage_ambient_lighting() {
        let Sky::V011w(skylight) = evening().sky else { panic!("the evening names its skylight") };
        let side = [0.509_804, 0.529_412, 0.564_706];
        let p = skylight.parameters([0.1, -0.9, 0.2], 3.0);
        assert_eq!(p[0], [0.32, 0.39, 0.5, 0.0]);
        assert_eq!(p[1], [0.0, 5.0, 0.1, 0.0]);
        // +x with +z.x, -x with +z.y, -y (down) with +z.z, -z with +y.x
        assert_eq!(p[2], [side[0], side[1], side[2], side[0]]);
        assert_eq!(p[3], [side[0], side[1], side[2], side[1]]);
        assert_eq!(p[4], [1.0, 1.0, 1.0, side[2]]);
        assert_eq!(p[5], [side[0], side[1], side[2], 1.0]);
        // the sun's direction with +y.y, the tinted sun with +y.z
        assert_eq!(p[6], [0.1, -0.9, 0.2, 1.0]);
        assert_eq!(p[7], [0.0, 0.0, 0.0, 1.0]);
        // a skylight named in part is refused
        assert!(std::panic::catch_unwind(|| Profile::parse(evening_number, |key| {
            (key == c"skylight_light").then_some([1.0, 1.0, 1.0])
        }))
        .is_err());
    }

    #[test]
    fn the_clouds_pack_as_0_11e_stage_atmosphere() {
        let Clouds::V011w(layer) = evening().clouds else { panic!("the evening names its clouds") };
        assert_eq!(layer.parameters(), ([2.296, 1.382, 0.748, 1.0], [10.0, 0.15, 0.0, 0.0]));
        // clouds named in part are refused
        assert!(std::panic::catch_unwind(|| Profile::parse(evening_number, |key| {
            (key != c"clouds_color").then(|| evening_vector(key)).flatten()
        }))
        .is_err());
    }

    #[test]
    fn cone_twins_need_the_cone_family_an_input_and_the_mark() {
        for (n, twin) in CONE_TWINS.iter().enumerate() {
            let input = VertexInput::parse(n as u8 + 1);
            assert_eq!(cone_twin(input, CONE_PS, true), Some(*twin));
            assert_eq!(cone_twin(input, CONE_PS, false), None);
            assert_eq!(cone_twin(input, c"forward_volume_sphere_base", true), None);
            assert!(twin.to_str().unwrap().ends_with(&std::format!("_v011w_{}", n + 1)));
        }
        for other in [0u8, 7, 8, 11, 14] {
            assert_eq!(cone_twin(VertexInput::parse(other), CONE_PS, true), None);
        }
        // the type's two top bits are not the input's
        assert_eq!(cone_twin(VertexInput::parse(0xC1), CONE_PS, true), Some(CONE_TWINS[0]));
    }
}
