//! The ported 0.11 weathers fog their geometry the 0.11 way (Vostok Radar
//! Station v0.11's evening and rain; srp-026g/docs/notes/v011-weather.md).
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
//! Presentation only: nothing a match computes reads any of it.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::ConfigValue;
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::project::{ProjectId, ProjectView};
use crate::vostok::render_stage::{self, AtmosphereStage, RenderSceneId, StageSite};
use crate::vostok::{self, Context, Flow, Lock, NoExit};

/// The key of a project config's root that makes its map a 0.11 weather:
/// the weather's inscatter distance in metres (0.11e's post-process
/// material's `atmosphere_inscatter_distance`).
pub const MAP_KEY: &CStr = c"srp_renderer_v011w";

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

    /// The inscatter distance when `drawn` is the map's scene.
    fn fog(self, drawn: Option<RenderSceneId>) -> Option<Distance> {
        match self {
            Map::V011w { scene, distance, .. } if Some(scene) == drawn => Some(distance),
            Map::V011w { .. } | Map::Native => None,
        }
    }
}

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

fn fog(ctx: &Context<'_>, site: StageSite) -> Option<Distance> {
    let drawn = render_stage::drawn_scene(ctx, site);
    MAP.with(ctx.thread(), |map| map.fog(drawn))
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
    match fog(ctx, StageSite::GeometryFog) {
        Some(distance) => {
            render_stage::set_fog_technique(ctx, render_stage::FOG_MULTIPLY);
            FOG.with(ctx.thread(), |fog| *fog = fog.step(FogEvent::Multiply(distance)).0);
            Flow::Exit(vostok::V011WRenderOnGeometryFogExit::Applied)
        }
        None => Flow::Continue,
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
        assert_eq!(map.fog(Some(scene(10))), Some(Distance(200.0)));
        assert_eq!(map.fog(Some(scene(20))), None);
        assert_eq!(map.fog(None), None);
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
    fn the_distance_is_a_number_above_0() {
        assert_eq!(Distance::parse(ConfigValue::Integer(200)), Distance(200.0));
        assert_eq!(Distance::parse(ConfigValue::Number(300.0)), Distance(300.0));
        assert!(std::panic::catch_unwind(|| Distance::parse(ConfigValue::Boolean(true))).is_err());
        assert!(std::panic::catch_unwind(|| Distance::parse(ConfigValue::Number(0.0))).is_err());
    }
}
