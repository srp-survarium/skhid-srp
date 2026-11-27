//! Patch 178: the ported Londons' deferred composition is 0.44a5's
//! (docs/notes/client_patch.md section 3.98, README "The ported Londons'
//! composition").
//!
//! 0.44a5 and 0.32a4 light foliage through in their deferred composition
//! pass: where a surface's translucency is above 0.001, the sun's light
//! passes through it by
//! `min(pow(sat(N . normalize(lerp(L, N, sat(sun_dir_vs_normal_factor)))), translucency_power), 1)`
//! times direct_translucency_color times the light and the sun's shadow.
//! 0.26g's programs have another term (max(N . L, 0) and a view term), and
//! they hard-code the rain-sheltered diffuse factor 0.85 that 0.44a5 reads
//! as rain_shadow_diffuse_multiplier. The two donors' 64 composition
//! programs are the same, code and resources; the content pack
//! composition-v044a gives them as the family `composition_v044a.ps` under
//! 0.26g's keys.
//!
//! - The compiles: each of stage_composition's eight effects compiles its
//!   one technique again after its own (technique 1, [`TWIN`]), the same
//!   code with 0.44a5's pixel shader, its sun shadow bound as t_sun_shadow
//!   (0.44a5's name of the slot 0.26g calls t_sun_shadow_and_scattering;
//!   .x is the shadow in both). Every compile makes it, on every map, so the
//!   effects stay whole through their life.
//! - The map: a project whose config has [`MAP_KEY`], a table of the four
//!   values its donor's environment sets (the first key frame, the one the
//!   port's environment is made of; the donors set the same in both). They
//!   are the map's lighting for as long as it is loaded, not blended by the
//!   game's post-process effects, which never name them (0.26g's data has no
//!   such key; London's donors set them only in the map's environment).
//! - The draw: a frame that draws that project's scene applies the twin and
//!   sets the four constants (0.44a5's execute 0x48F2E0: the colour times
//!   the intensity, w 1). Any other scene, the lobby's included, draws with
//!   0.26g's programs: the lobby's projects stay loaded during a match, so
//!   the map is its scene, never a time.
//!
//! Presentation only: nothing a match computes reads any of it.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::{ConfigTable, ConfigValue};
use crate::vostok::effect_compile::{self, CompileSite, CompilerId, Composition};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::project::{ProjectId, ProjectView};
use crate::vostok::render_stage::{self, CompositionLighting, RenderSceneId, StageSite, Technique};
use crate::vostok::{self, Context, Flow, Lock, NoExit};

/// The key of a project config's root that draws its map with 0.44a5's
/// composition: a table of the donor's four settings (edit list
/// composition-v044a-1.json).
pub const MAP_KEY: &CStr = c"srp_composition_v044a";

/// 0.44a5's composition programs (shaders/sm_4_0/composition_v044a.ps/<key>).
const PIXEL_SHADER: &CStr = c"composition_v044a";
/// 0.44a5's name of the sun shadow's slot.
const SUN_SHADOW: &CStr = c"t_sun_shadow";

/// Each composition effect compiles technique 0 composition, then 1, its twin.
const TWIN: Technique = Technique::new(1);

/// The four settings as a donor's environment writes them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    direct_translucency_color: [f32; 3],
    direct_translucency_intensity: f32,
    sun_dir_vs_normal_factor: f32,
    translucency_power: f32,
    rain_shadow_diffuse_multiplier: f32,
}

impl Settings {
    /// The table at [`MAP_KEY`]; every key is required.
    fn parse(table: &ConfigTable<'_>) -> Settings {
        let number = |name: &CStr| -> f32 {
            table
                .required(name)
                .and_then(ConfigValue::number)
                .unwrap_or_else(|_| panic!("srp_composition_v044a has a number {name:?}"))
        };
        Settings {
            direct_translucency_color: [
                number(c"direct_translucency_r"),
                number(c"direct_translucency_g"),
                number(c"direct_translucency_b"),
            ],
            direct_translucency_intensity: number(c"direct_translucency_intensity"),
            sun_dir_vs_normal_factor: number(c"sun_dir_vs_normal_factor"),
            translucency_power: number(c"translucency_power"),
            rain_shadow_diffuse_multiplier: number(c"rain_shadow_diffuse_multiplier"),
        }
    }

    /// The constants as 0.44a5's stage_composition::execute sets them.
    fn lighting(self) -> CompositionLighting {
        CompositionLighting {
            direct_translucency_color: self.direct_translucency_color.map(|c| c * self.direct_translucency_intensity),
            sun_dir_vs_normal_factor: self.sun_dir_vs_normal_factor,
            translucency_power: self.translucency_power,
            rain_shadow_diffuse_multiplier: self.rain_shadow_diffuse_multiplier,
        }
    }
}

/// The map whose scene is drawn with 0.44a5's composition, if one is loaded.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Map {
    Native,
    Ported { project: ProjectId, scene: RenderSceneId, settings: Settings },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MapEvent {
    /// A project whose config has the key was inserted, with its scene.
    Inserted { project: ProjectId, scene: RenderSceneId, settings: Settings },
    /// A project was removed or is being destroyed.
    Gone(ProjectId),
}

impl Map {
    fn step(self, event: MapEvent) -> Map {
        match (self, event) {
            (_, MapEvent::Inserted { project, scene, settings }) => Map::Ported { project, scene, settings },
            (Map::Ported { project, .. }, MapEvent::Gone(gone)) if project == gone => Map::Native,
            (map, MapEvent::Gone(_)) => map,
        }
    }

    /// The settings of the frame's scene, if it is the ported map's.
    fn drawn(self, drawn: Option<RenderSceneId>) -> Option<Settings> {
        match self {
            Map::Ported { scene, settings, .. } if Some(scene) == drawn => Some(settings),
            Map::Ported { .. } | Map::Native => None,
        }
    }
}

static MAP: Lock<Map> = Lock::new(Map::Native);

/// What a compile does at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    /// Compile the twin: back to the technique's start.
    Twin,
    /// Return, the twin compiled.
    Return,
}

/// Compiles at once: one per resource thread at most; past this many the
/// game stops (a round lost would leave an effect without its twin).
const ROUNDS: usize = 64;

/// The compilers compiling a composition effect's twin.
struct Rounds([Option<CompilerId>; ROUNDS]);

impl Rounds {
    const fn new() -> Rounds {
        Rounds([None; ROUNDS])
    }

    fn position(&self, compiler: CompilerId) -> Option<usize> {
        self.0.iter().position(|slot| *slot == Some(compiler))
    }

    /// The compile by `compiler` is at its end.
    fn end(&mut self, compiler: CompilerId) -> End {
        match self.position(compiler) {
            None => {
                let free = self.0.iter_mut().find(|slot| slot.is_none()).expect("fewer compiles at once than ROUNDS");
                *free = Some(compiler);
                End::Twin
            }
            Some(i) => {
                self.0[i] = None;
                End::Return
            }
        }
    }

    /// Whether `compiler` compiles a twin now.
    fn twin(&self, compiler: CompilerId) -> bool {
        self.position(compiler).is_some()
    }
}

static ROUNDS_NOW: Lock<Rounds> = Lock::new(Rounds::new());

fn in_twin(ctx: &Context<'_>, site: CompileSite) -> bool {
    let compiler = effect_compile::compiler_id(ctx, site);
    ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.twin(compiler))
}

/// project.insert (after patch 130's): a project whose config has the key
/// makes its scene the ported map's.
pub fn on_project_insert(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let project = ProjectView::at_insert(ctx).expect("project.insert's project");
    let root = project.root().ok().and_then(|root| root.table().ok()).expect("a project's config has a root table");
    let settings = match root.get(MAP_KEY).expect("a project's config reads") {
        None => return Flow::Continue,
        Some(ConfigValue::Table(table)) => Settings::parse(&table),
        Some(_) => panic!("srp_composition_v044a is a table"),
    };
    let scene = project.render_scene().expect("a project being inserted has its scene");
    MAP.with(ctx.thread(), |map| *map = map.step(MapEvent::Inserted { project: project.id(), scene, settings }));
    log::line(&[Text("composition_v044a: a ported London; its scene's composition drawn as 0.44a5 draws it")]);
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

/// A composition effect's begin_pass: the twin's with 0.44a5's program.
fn begin_pass(ctx: &mut Context<'_>, composition: Composition) -> Flow<NoExit> {
    if in_twin(ctx, CompileSite::CompositionPass(composition)) {
        effect_compile::set_composition_pixel_shader(ctx, composition, PIXEL_SHADER);
    }
    Flow::Continue
}

/// A composition effect's t_sun_shadow_and_scattering: the twin binds the
/// same target as t_sun_shadow. True when the caller takes `bound`.
fn sun_shadow(ctx: &mut Context<'_>, composition: Composition) -> bool {
    let twin = in_twin(ctx, CompileSite::CompositionSunShadow(composition));
    if twin {
        effect_compile::bind_composition_sun_shadow(ctx, composition, SUN_SHADOW);
    }
    twin
}

/// A composition effect's end: the first time back to its technique, for
/// the twin; the second time its return.
fn compiled(ctx: &mut Context<'_>, composition: Composition) -> End {
    let compiler = effect_compile::compiler_id(ctx, CompileSite::CompositionEnd(composition));
    ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler))
}

/// The three handlers of each composition effect's compile.
macro_rules! composition_hooks {
    ($($composition:ident: $pass:ident, $shadow:ident, $shadow_exit:ident, $compiled:ident, $compiled_exit:ident;)*) => {$(
        pub fn $pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
            begin_pass(ctx, Composition::$composition)
        }

        pub fn $shadow(ctx: &mut Context<'_>) -> Flow<vostok::$shadow_exit> {
            if sun_shadow(ctx, Composition::$composition) { Flow::Exit(vostok::$shadow_exit::Bound) } else { Flow::Continue }
        }

        pub fn $compiled(ctx: &mut Context<'_>) -> Flow<vostok::$compiled_exit> {
            match compiled(ctx, Composition::$composition) {
                End::Twin => Flow::Exit(vostok::$compiled_exit::Technique),
                End::Return => Flow::Continue,
            }
        }
    )*};
}

composition_hooks! {
    C000: on_begin_pass_000, on_sun_shadow_000, CompositionV044AOnSunShadow000Exit, on_compiled_000, CompositionV044AOnCompiled000Exit;
    C100: on_begin_pass_100, on_sun_shadow_100, CompositionV044AOnSunShadow100Exit, on_compiled_100, CompositionV044AOnCompiled100Exit;
    C010: on_begin_pass_010, on_sun_shadow_010, CompositionV044AOnSunShadow010Exit, on_compiled_010, CompositionV044AOnCompiled010Exit;
    C110: on_begin_pass_110, on_sun_shadow_110, CompositionV044AOnSunShadow110Exit, on_compiled_110, CompositionV044AOnCompiled110Exit;
    C001: on_begin_pass_001, on_sun_shadow_001, CompositionV044AOnSunShadow001Exit, on_compiled_001, CompositionV044AOnCompiled001Exit;
    C101: on_begin_pass_101, on_sun_shadow_101, CompositionV044AOnSunShadow101Exit, on_compiled_101, CompositionV044AOnCompiled101Exit;
    C011: on_begin_pass_011, on_sun_shadow_011, CompositionV044AOnSunShadow011Exit, on_compiled_011, CompositionV044AOnCompiled011Exit;
    C111: on_begin_pass_111, on_sun_shadow_111, CompositionV044AOnSunShadow111Exit, on_compiled_111, CompositionV044AOnCompiled111Exit;
}

/// stage_composition's constructor: the four constants' hosts.
pub fn on_constructed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    render_stage::register_composition_lighting(ctx);
    Flow::Continue
}

fn drawn(ctx: &Context<'_>, site: StageSite) -> Option<Settings> {
    let scene = render_stage::drawn_scene(ctx, site);
    MAP.with(ctx.thread(), |map| map.drawn(scene))
}

/// stage_composition::execute's technique: the twin on the ported map's scene.
pub fn on_technique(ctx: &mut Context<'_>) -> Flow<vostok::CompositionV044AOnTechniqueExit> {
    match drawn(ctx, StageSite::CompositionTechnique) {
        Some(_) => {
            render_stage::set_composition_technique(ctx, TWIN);
            Flow::Exit(vostok::CompositionV044AOnTechniqueExit::Apply)
        }
        None => Flow::Continue,
    }
}

/// stage_composition::execute's constants: the map's four on its scene.
pub fn on_constants(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(settings) = drawn(ctx, StageSite::CompositionConstants) {
        render_stage::set_composition_lighting(ctx, &settings.lighting());
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn london_day() -> Settings {
        Settings {
            direct_translucency_color: [0.573, 0.561, 0.4],
            direct_translucency_intensity: 1.2,
            sun_dir_vs_normal_factor: 0.5,
            translucency_power: 0.37,
            rain_shadow_diffuse_multiplier: 0.85,
        }
    }

    #[test]
    fn the_colour_is_sent_times_the_intensity() {
        let lighting = london_day().lighting();
        let [r, g, b] = lighting.direct_translucency_color;
        assert!((r - 0.6876).abs() < 1e-6 && (g - 0.6732).abs() < 1e-6 && (b - 0.48).abs() < 1e-6);
        assert_eq!((lighting.sun_dir_vs_normal_factor, lighting.translucency_power), (0.5, 0.37));
    }

    #[test]
    fn only_the_maps_own_scene_draws_the_twin() {
        let (project, scene, lobby) = (ProjectId::for_test(1), RenderSceneId::for_test(0x100), RenderSceneId::for_test(0x200));
        let map = Map::Native.step(MapEvent::Inserted { project, scene, settings: london_day() });
        assert_eq!(map.drawn(Some(scene)), Some(london_day()));
        assert_eq!(map.drawn(Some(lobby)), None);
        assert_eq!(map.drawn(None), None);
        assert_eq!(map.step(MapEvent::Gone(ProjectId::for_test(2))), map);
        assert_eq!(map.step(MapEvent::Gone(project)).drawn(Some(scene)), None);
    }

    #[test]
    fn a_compile_returns_on_its_second_end() {
        let mut rounds = Rounds::new();
        let (a, b) = (CompilerId::for_test(1), CompilerId::for_test(2));
        assert_eq!(rounds.end(a), End::Twin);
        assert!(rounds.twin(a) && !rounds.twin(b));
        assert_eq!(rounds.end(b), End::Twin);
        assert_eq!(rounds.end(a), End::Return);
        assert!(!rounds.twin(a) && rounds.twin(b));
    }
}
