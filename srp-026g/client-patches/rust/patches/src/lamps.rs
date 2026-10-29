//! Breakable lamps: safe simulation decisions over typed engine views.
//! Project registration, rendering and teardown belong to ProjectRegistry.
#![forbid(unsafe_code)]

pub mod model;

use crate::vostok::{self, Context, Flow, NoExit};
use crate::vostok::bullet::{BulletRay, MaterialId};
use crate::vostok::collection::BoundedVec;
use crate::vostok::config::{ConfigError, ConfigTable};
use crate::vostok::history::{HistoryReader, HistoryWriter};
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
}
static PROJECT: ProjectRegistry<Lamps, Binding> = ProjectRegistry::new();

#[derive(Debug)]
enum MetadataError { Binding(BindingError), Config(ConfigError), Version, Material, Count, Bounds, Condition }
impl MetadataError {
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
    if let Err(error) = PROJECT.insert(ctx, prepare) { error.refuse(); }
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
                let effects = ray.surface_break_effects(lamps.material);
                lamps.shoot(segment, |point, direction| {
                    if let Some(effects) = &effects { effects.play(point, direction); }
                });
            });
        }
    }
    Flow::Continue
}
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
