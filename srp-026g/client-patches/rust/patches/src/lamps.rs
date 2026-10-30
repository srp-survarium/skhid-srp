//! Breakable lamps: safe simulation decisions over typed engine views.
//! Project registration, rendering and teardown belong to ProjectRegistry.
#![forbid(unsafe_code)]

pub mod model;

use crate::vostok::{self, Context, Flow, NoExit};
use crate::vostok::bullet::{BulletDecal, BulletRay, DecalSite, MaterialId};
use crate::vostok::collection::BoundedVec;
use crate::vostok::config::{ConfigError, ConfigTable};
use crate::vostok::history::{HistoryReader, HistoryWriter};
use crate::vostok::log::Piece::Text;
use crate::vostok::project::{BindingError, MethodTables, Presentation, ProjectPlan, ProjectRegistry, ProjectView, VisualIndex};
use model::{Bounds, BreakEffect, BulletSegment, Condition, Distance, LampCount, LampEvent, LampId, MAX_LAMPS,
            RayDirection, Snapshot, Visibility, WorldPoint};

#[derive(Clone, Copy)]
struct Binding { lamp: LampId, visible_when: Condition }

struct Lamps {
    bounds: BoundedVec<Bounds, MAX_LAMPS>,
    state: Snapshot,
    material: MaterialId,
}
impl Presentation<Binding> for Lamps {
    fn visibility(&self, binding: Binding) -> Visibility {
        if self.state.condition(binding.lamp) == binding.visible_when { Visibility::Visible } else { Visibility::Hidden }
    }
}
impl Lamps {
    fn shoot(&mut self, segment: BulletSegment, mut shatter: impl FnMut(WorldPoint, RayDirection)) {
        for (i, bounds) in self.bounds.iter().enumerate() {
            let lamp = self.state.count().id(i).expect("bounds and history have the same authored count");
            if bounds.intersects(segment) && self.state.transition(lamp, LampEvent::Hit) == BreakEffect::Shatter {
                shatter(bounds.centre(), segment.direction());
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
        surface == self.material && self.bounds.iter().any(|bounds| bounds.holds(point, mark_tolerance()))
    }
}
/// The game moves a hit's point onto the struck plane to 1 mm; the authored
/// bounds hold the fixture's collision. A margin for both and for rounding.
fn mark_tolerance() -> Distance { Distance::parse(0.01).expect("a finite, non-negative distance") }
static PROJECT: ProjectRegistry<Lamps, Binding> = ProjectRegistry::new();

#[derive(Debug)]
enum MetadataError { Binding(BindingError), Config(ConfigError), Version, Material, Count, Bounds, Condition }
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
    if spec.required(c"version")?.integer()? != 2 { return Err(MetadataError::Version); }
    let material = MaterialId::parse(spec.required(c"break_material")?.integer()?).ok_or(MetadataError::Material)?;
    let rows = spec.required(c"lamps")?.entries()?;
    let count = LampCount::parse(rows.len() as u32).ok_or(MetadataError::Count)?;
    let methods = MethodTables::new(vostok::lamps_visual_vtable_handle(), vostok::lamps_light_vtable_handle());
    let mut plan = project.plan((), methods)?;
    let mut bounds = BoundedVec::new();
    for (i, row) in rows.iter().enumerate() {
        let row = row?.table()?;
        let lamp = count.id(i).expect("entry belongs to the parsed count");
        let low = position(&row, [c"min_x", c"min_y", c"min_z"])?;
        let high = position(&row, [c"max_x", c"max_y", c"max_z"])?;
        bounds.push(Bounds::parse(low, high).ok_or(MetadataError::Bounds)?).map_err(|_| MetadataError::Count)?;
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
    Ok(Some(plan.with_data(Lamps { bounds, state: Snapshot::intact(count), material })))
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
                let material = lamps.material;
                lamps.shoot(segment, |point, direction| {
                    if let Some(effects) = ray.surface_break_effects(material) { effects.play(point, direction); }
                });
            });
        }
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
