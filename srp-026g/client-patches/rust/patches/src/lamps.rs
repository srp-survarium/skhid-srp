//! Breakable lamps: safe simulation decisions over typed engine views.
//! Project registration, rendering and teardown belong to ProjectRegistry.
#![forbid(unsafe_code)]

pub mod model;

use crate::vostok::{self, Context, Flow, NoExit};
use crate::vostok::bullet::{BulletDecal, BulletRay, DecalSite, MaterialId, SurfaceBreakEffects};
use crate::vostok::explosion::Explosion;
use crate::vostok::collection::BoundedVec;
use crate::vostok::config::{ConfigError, ConfigTable};
use crate::vostok::history::{HistoryReader, HistoryWriter};
use crate::vostok::log::Piece::Text;
use crate::vostok::project::{BindingError, MethodTables, Presentation, ProjectPlan, ProjectRegistry, ProjectView, VisualIndex};
use model::{Bounds, BreakEffect, BulletSegment, Condition, Distance, Fixture, GlassLine, LampCount, LampEvent, LampId,
            MAX_LAMPS, RayDirection, Snapshot, Visibility, WorldPoint};

#[derive(Clone, Copy)]
struct Binding { lamp: LampId, visible_when: Condition }

struct Lamps {
    fixtures: BoundedVec<Fixture, MAX_LAMPS>,
    state: Snapshot,
    /// The fixtures' glass (their collision's material).
    material: MaterialId,
    /// The material whose pair with the glass plays a break, whatever broke
    /// it: bullet x glass, the game's own glass shards and sound.
    effect: MaterialId,
}
impl Presentation<Binding> for Lamps {
    fn visibility(&self, binding: Binding) -> Visibility {
        if self.state.condition(binding.lamp) == binding.visible_when { Visibility::Visible } else { Visibility::Hidden }
    }
}
impl Lamps {
    fn shoot(&mut self, segment: BulletSegment, mut shatter: impl FnMut(GlassLine, RayDirection)) {
        self.hit(|bounds| bounds.intersects(segment), |glass| shatter(glass, segment.direction()));
    }
    /// A blast breaks every fixture whose bounds come within its splash
    /// radius of the centre, where the game's splash reaches them (as it
    /// would a player's bone there: walls stop it, glass does not).
    fn explode(&mut self, centre: WorldPoint, radius: Distance, reaches: impl Fn(WorldPoint) -> bool,
               mut shatter: impl FnMut(GlassLine, RayDirection)) {
        self.hit(|bounds| {
            let nearest = bounds.nearest(centre);
            centre.within(nearest, radius) && reaches(nearest)
        }, |glass| shatter(glass, RayDirection::between(centre, glass.middle()).unwrap_or_else(down)));
    }
    /// The one transition of every break, a bullet's or a blast's: a first
    /// hit of an intact fixture shatters it, once.
    fn hit(&mut self, struck: impl Fn(Bounds) -> bool, mut shatter: impl FnMut(GlassLine)) {
        for (i, fixture) in self.fixtures.iter().enumerate() {
            let lamp = self.state.count().id(i).expect("fixtures and history have the same authored count");
            if self.state.condition(lamp) == Condition::Intact && struck(fixture.bounds)
                && self.state.transition(lamp, LampEvent::Hit) == BreakEffect::Shatter {
                shatter(fixture.glass);
            }
        }
    }
    /// Whether a hit's mark would be on a fixture's glass. The game's decal
    /// is a world-space box no model owns: one on the glass would stay in the
    /// air, or on the broken model behind it, once the glass is gone. A shot
    /// at a fixture's glass breaks it, so none is drawn there, intact or
    /// broken: the rule needs no lamp state, and rollback, resimulation and
    /// late joins (which carry no decals) agree with it.
    fn marks_glass(&self, surface: MaterialId, point: WorldPoint) -> bool {
        surface == self.material && self.fixtures.iter().any(|fixture| fixture.bounds.holds(point, mark_tolerance()))
    }
}
/// A blast at a fixture's glass throws its shards down.
fn down() -> RayDirection { RayDirection::parse([0.0, -1.0, 0.0]).expect("a finite, non-zero direction") }
/// A break's shards: the game's glass hit (some 70 shards of 3.5 cm), about
/// one burst per 15 cm of the glass (a bulb's one, a 1.5 m tube's ten), at
/// most ten, thrown along `direction`, and its sound once at the glass's
/// middle.
fn shatter(effects: Option<SurfaceBreakEffects<'_>>, glass: GlassLine, direction: RayDirection) {
    const MOST: core::num::NonZeroUsize = core::num::NonZeroUsize::new(10).expect("non-zero");
    let spacing = Distance::parse(0.15).expect("a finite, non-negative distance");
    if let Some(effects) = effects { effects.play(glass.along(spacing, MOST), glass.middle(), direction); }
}
/// The game moves a hit's point onto the struck plane to 1 mm; the authored
/// bounds hold the fixture's collision. A margin for both and for rounding.
fn mark_tolerance() -> Distance { Distance::parse(0.01).expect("a finite, non-negative distance") }
static PROJECT: ProjectRegistry<Lamps, Binding> = ProjectRegistry::new();

#[derive(Debug)]
enum MetadataError { Binding(BindingError), Config(ConfigError), Version, Material, Count, Bounds, Condition }
/// The metadata's format: 3 adds effect_material and each fixture's glass line.
const VERSION: u32 = 3;
impl MetadataError {
    /// The objects the metadata names are not this map's (a wrong index, a
    /// moved light): the project stays without lamps, on every client alike,
    /// since every client loads the same content.
    fn unbound(&self) -> Option<&'static str> {
        match self {
            Self::Binding(BindingError::InvalidVisual) => Some("a visual index past the map's visuals"),
            Self::Binding(BindingError::UnknownVisualType) => Some("a visual index naming another kind of visual"),
            Self::Binding(BindingError::UnknownLight) => Some("a light position no light of the map has"),
            Self::Binding(BindingError::AmbiguousLight) => Some("a light position two lights of the map have"),
            Self::Binding(BindingError::DuplicateObject) => Some("one object bound twice"),
            _ => None,
        }
    }
    fn refuse(self) -> ! {
        match self {
            Self::Binding(error) => panic!("invalid lamp binding: {error:?}"),
            Self::Config(error) => panic!("invalid lamp config: {error:?}"),
            other => panic!("invalid lamp metadata: {other:?}"),
        }
    }
}
impl From<BindingError> for MetadataError { fn from(e: BindingError) -> Self { Self::Binding(e) } }
impl From<ConfigError> for MetadataError { fn from(e: ConfigError) -> Self { Self::Config(e) } }

fn position(table: &ConfigTable<'_>, keys: [&core::ffi::CStr; 3]) -> Result<WorldPoint, MetadataError> {
    let mut point = [0.0; 3];
    for (value, key) in point.iter_mut().zip(keys) { *value = table.required(key)?.number()?; }
    WorldPoint::parse(point).ok_or(MetadataError::Bounds)
}

fn prepare(project: ProjectView<'_>) -> Result<Option<ProjectPlan<'_, Lamps, Binding>>, MetadataError> {
    let root = project.root()?.table()?;
    let Some(spec) = root.get(c"srp_lamps")? else { return Ok(None) };
    let spec = spec.table()?;
    if spec.required(c"version")?.integer()? != VERSION { return Err(MetadataError::Version); }
    let material = MaterialId::parse(spec.required(c"break_material")?.integer()?).ok_or(MetadataError::Material)?;
    let effect = MaterialId::parse(spec.required(c"effect_material")?.integer()?).ok_or(MetadataError::Material)?;
    let rows = spec.required(c"lamps")?.entries()?;
    let count = LampCount::parse(rows.len() as u32).ok_or(MetadataError::Count)?;
    let methods = MethodTables::new(vostok::lamps_visual_vtable_handle(), vostok::lamps_light_vtable_handle());
    let mut plan = project.plan((), methods)?;
    let mut fixtures = BoundedVec::new();
    for (i, row) in rows.iter().enumerate() {
        let row = row?.table()?;
        let lamp = count.id(i).expect("entry belongs to the parsed count");
        let low = position(&row, [c"min_x", c"min_y", c"min_z"])?;
        let high = position(&row, [c"max_x", c"max_y", c"max_z"])?;
        let bounds = Bounds::parse(low, high).ok_or(MetadataError::Bounds)?;
        let glass = GlassLine::new(position(&row, [c"glass_from_x", c"glass_from_y", c"glass_from_z"])?,
                                   position(&row, [c"glass_to_x", c"glass_to_y", c"glass_to_z"])?);
        fixtures.push(Fixture { bounds, glass }).map_err(|_| MetadataError::Count)?;
        for visual in row.required(c"visuals")?.entries()?.iter() {
            let visual = visual?.table()?;
            let visible_when = Condition::parse(visual.required(c"state")?.integer()?).ok_or(MetadataError::Condition)?;
            plan.visual(VisualIndex::from(visual.required(c"index")?.integer()?), Binding { lamp, visible_when })?;
        }
        for light in row.required(c"lights")?.entries()?.iter() {
            let position = position(&light?.table()?, [c"x", c"y", c"z"])?;
            plan.light_at(position, Binding { lamp, visible_when: Condition::Intact })?;
        }
    }
    Ok(Some(plan.with_data(Lamps { fixtures, state: Snapshot::intact(count), material, effect })))
}

pub fn on_bind(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Err(error) = PROJECT.insert(ctx, prepare) {
        let Some(why) = error.unbound() else { error.refuse() };
        vostok::log::line(&[Text("lamps: this map's lamps are not breakable, its metadata has "), Text(why)]);
    }
    Flow::Continue
}
pub fn on_remove(ctx: &mut Context<'_>) -> Flow<NoExit> { PROJECT.removed(ctx); Flow::Continue }
pub fn on_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> { PROJECT.destroying(ctx); Flow::Continue }
pub fn on_present(ctx: &mut Context<'_>) -> Flow<NoExit> { PROJECT.present(ctx); Flow::Continue }
pub fn on_ray(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(ray) = BulletRay::at(ctx) && let Some(physics) = ray.physics() {
        let segment = (|| {
            let hit = match ray.struck() { Some(p) => Some(WorldPoint::parse(p)?), None => None };
            BulletSegment::clipped(WorldPoint::parse(ray.origin())?, RayDirection::parse(ray.direction())?,
                                   Distance::parse(ray.length())?, hit)
        })();
        if let Some(segment) = segment {
            PROJECT.with_physics(ctx.thread(), physics, |lamps| {
                let (effect, material) = (lamps.effect, lamps.material);
                lamps.shoot(segment, |glass, direction| {
                    shatter(ray.surface_break_effects(effect, material), glass, direction);
                });
            });
        }
    }
    Flow::Continue
}
pub fn on_explode(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(blast) = Explosion::at(ctx) && let Some(physics) = blast.physics() {
        PROJECT.with_physics(ctx.thread(), physics, |lamps| {
            let (effect, material) = (lamps.effect, lamps.material);
            lamps.explode(blast.centre(), blast.radius(), |point| blast.reaches(point), |glass, direction| {
                shatter(blast.surface_break_effects(effect, material), glass, direction);
            });
        });
    }
    Flow::Continue
}
fn glass_mark(ctx: &Context<'_>, site: DecalSite) -> bool {
    let Some(decal) = BulletDecal::at(ctx, site) else { return false };
    let (Some(physics), Some(surface), Some(point)) = (decal.physics(), decal.surface(), decal.point()) else {
        return false;
    };
    PROJECT.with_physics(ctx.thread(), physics, |lamps| lamps.marks_glass(surface, point)).unwrap_or(false)
}
macro_rules! decal_hook {
    ($fn:ident, $site:ident, $exit:ident) => {
        pub fn $fn(ctx: &mut Context<'_>) -> Flow<vostok::$exit> {
            if glass_mark(ctx, DecalSite::$site) { Flow::Exit(vostok::$exit::Skip) } else { Flow::Continue }
        }
    };
}
decal_hook!(on_decal_enter, Enter, LampsOnDecalEnterExit);
decal_hook!(on_decal_leave, Leave, LampsOnDecalLeaveExit);
decal_hook!(on_decal_ricochet, Ricochet, LampsOnDecalRicochetExit);
decal_hook!(on_decal_stop, Stop, LampsOnDecalStopExit);
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut writer) = HistoryWriter::at_saved(ctx) {
        PROJECT.with_physics(ctx.thread(), writer.physics(), |lamps| writer.append(lamps.state.words()));
    }
    Flow::Continue
}
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut reader) = HistoryReader::at_loaded(ctx) {
        let result = PROJECT.with_physics(ctx.thread(), reader.physics(), |lamps| {
            let restored = reader.decode(|source| Snapshot::parse(source, lamps.state.count()))?;
            lamps.state.restore(restored).ok_or(crate::vostok::history::HistoryError::InvalidPayload)?;
            Ok::<_, crate::vostok::history::HistoryError>(())
        });
        // Refuse only after releasing the registry lock. The decoder left
        // both the history cursor and lamp state untouched on failure.
        if let Some(result) = result { result.expect("valid lamp history"); }
    }
    Flow::Continue
}

pub fn on_visual_insert(_: &vostok::OrigVisualInsert, visual: &mut vostok::RenderVisual<'_>, scene: &mut vostok::GameScene<'_>) {
    PROJECT.visual_insert(visual, scene);
}
pub fn on_visual_remove(_: &vostok::OrigVisualRemove, visual: &mut vostok::RenderVisual<'_>, scene: &mut vostok::GameScene<'_>) {
    PROJECT.visual_remove(visual, scene);
}
pub fn on_light_insert(_: &vostok::OrigLightInsert, light: &mut vostok::ObjectLight<'_>) { PROJECT.light_insert(light); }
pub fn on_light_remove(_: &vostok::OrigLightRemove, light: &mut vostok::ObjectLight<'_>) { PROJECT.light_remove(light); }
