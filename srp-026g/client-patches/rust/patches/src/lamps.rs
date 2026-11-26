//! Breakable lamps: safe simulation decisions over typed engine views.
//! Project registration, rendering and teardown belong to ProjectRegistry.
#![forbid(unsafe_code)]

pub mod model;

use crate::vostok::{self, Context, Flow, NoExit};
use crate::vostok::clock::FrameClock;
use crate::vostok::world::GameWorld;
use crate::vostok::bullet::{BulletDecal, BulletRay, DecalSite, MaterialId, SurfaceBreakEffects};
use crate::vostok::explosion::Explosion;
use crate::vostok::collection::BoundedVec;
use crate::vostok::config::{ConfigError, ConfigTable};
use crate::vostok::history::{HistoryReader, HistoryWriter};
use crate::vostok::log::Piece::Text;
use crate::vostok::project::{BindingError, MethodTables, Presentation, ProjectPlan, ProjectRegistry, ProjectView, Share,
                             VisualIndex, VisualTransform};
use ::ragdoll::math::{Pose, Vec3};
use model::{Bounds, BreakEffect, BulletSegment, Condition, Distance, Fixture, Flight, GlassLine, LampCount, LampEvent, LampId,
            MAX_LAMPS, MAX_PIECES, RayDirection, Snapshot, Visibility, WorldPoint};

#[derive(Clone, Copy)]
enum Binding {
    /// Drawn while its fixture is in `visible_when` (a model, a light).
    Part { lamp: LampId, visible_when: Condition },
    /// A broken fixture's loose piece, `pieces[piece]`: drawn where its flight is.
    Loose { piece: usize },
}

/// A fixture's loose piece: the visual seated at `seat` (its placement in
/// the map) while the fixture is intact.
#[derive(Clone, Copy)]
struct Piece { lamp: LampId, seat: Pose, flight: Flight }

struct Lamps {
    fixtures: BoundedVec<Fixture, MAX_LAMPS>,
    pieces: BoundedVec<Piece, MAX_PIECES>,
    /// The pieces' frames: the real time between two presents.
    clock: FrameClock,
    state: Snapshot,
    /// The fixtures' glass (their collision's material).
    material: MaterialId,
    /// The material whose pair with the glass plays a break, whatever broke
    /// it: bullet x glass, the game's own glass shards and sound.
    effect: MaterialId,
}
impl Presentation<Binding> for Lamps {
    fn visibility(&self, binding: Binding) -> Visibility {
        let (lamp, visible_when) = match binding {
            Binding::Part { lamp, visible_when } => (lamp, visible_when),
            Binding::Loose { piece } => (self.piece(piece).lamp, Condition::Broken),
        };
        if self.state.condition(lamp) == visible_when { Visibility::Visible } else { Visibility::Hidden }
    }
    fn placement(&self, binding: Binding) -> Option<VisualTransform> {
        let Binding::Loose { piece } = binding else { return None };
        VisualTransform::parse(crate::ragdoll::matrix_of(self.piece(piece).flight.pose()?))
    }
}
/// The level's rays a frame for the pieces (two an end of a moving one).
const PIECE_RAYS: u32 = 32;
impl Lamps {
    fn piece(&self, index: usize) -> &Piece { self.pieces.get(index).expect("a bound piece") }
    fn shoot(&mut self, segment: BulletSegment, shatter: impl FnMut(GlassLine, RayDirection)) {
        self.hit(|bounds| bounds.intersects(segment), |_| segment.direction(), shatter);
    }
    /// A blast breaks every fixture whose bounds come within its splash
    /// radius of the centre, where the game's splash reaches them (as it
    /// would a player's bone there: walls stop it, glass does not).
    fn explode(&mut self, centre: WorldPoint, radius: Distance, reaches: impl Fn(WorldPoint) -> bool,
               shatter: impl FnMut(GlassLine, RayDirection)) {
        self.hit(|bounds| {
            let nearest = bounds.nearest(centre);
            centre.within(nearest, radius) && reaches(nearest)
        }, |glass| RayDirection::between(centre, glass.middle()).unwrap_or_else(down), shatter);
    }
    /// The one transition of every break, a bullet's or a blast's: a first
    /// hit of an intact fixture shatters it, once, and knocks its loose
    /// pieces off along the push.
    fn hit(&mut self, struck: impl Fn(Bounds) -> bool, push: impl Fn(GlassLine) -> RayDirection,
           mut shatter: impl FnMut(GlassLine, RayDirection)) {
        for (i, fixture) in self.fixtures.iter().enumerate() {
            let lamp = self.state.count().id(i).expect("fixtures and history have the same authored count");
            if self.state.condition(lamp) == Condition::Intact && struck(fixture.bounds)
                && self.state.transition(lamp, LampEvent::Hit) == BreakEffect::Shatter {
                let direction = push(fixture.glass);
                for piece in self.pieces.iter_mut().filter(|piece| piece.lamp == lamp) {
                    piece.flight = piece.flight.strike(direction);
                }
                shatter(fixture.glass, direction);
            }
        }
    }
    /// The pieces' frame of `dt` s in the level: each follows its fixture's
    /// condition (a rollback seats it again).
    fn fly(&mut self, dt: f32, level: &mut impl ::ragdoll::ragdoll::Level) {
        for piece in self.pieces.iter_mut() {
            piece.flight = piece.flight.step(self.state.condition(piece.lamp), piece.seat, dt, Vec3::Y, level);
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
enum MetadataError { Binding(BindingError), Config(ConfigError), Version, Material, Count, Bounds, Condition, Share, Seat }
/// The metadata's format: 3 adds effect_material and each fixture's glass
/// line. A row's optional tables (ambient_lights, particles, probes, loose)
/// need none: a client without them leaves them out.
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
            Self::Binding(BindingError::UnknownAmbientLight) => Some("an ambient light position no ambient light of the map has"),
            Self::Binding(BindingError::AmbiguousAmbientLight) => Some("an ambient light position two ambient lights of the map have"),
            Self::Binding(BindingError::UnknownParticle) => Some("a particle position no particle of the map has"),
            Self::Binding(BindingError::AmbiguousParticle) => Some("a particle position two particles of the map have"),
            Self::Binding(BindingError::UnknownProbe) => Some("a probe position no environment probe of the map has"),
            Self::Binding(BindingError::AmbiguousProbe) => Some("a probe position two environment probes of the map have"),
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
    let methods = MethodTables::new(vostok::lamps_visual_vtable_handle(), vostok::lamps_light_vtable_handle(),
                                    vostok::lamps_ambient_light_vtable_handle(), vostok::lamps_particle_vtable_handle(),
                                    vostok::lamps_probe_vtable_handle());
    let mut plan = project.plan((), methods)?;
    let mut fixtures = BoundedVec::new();
    let mut pieces = BoundedVec::new();
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
            plan.visual(VisualIndex::from(visual.required(c"index")?.integer()?), Binding::Part { lamp, visible_when })?;
        }
        // A piece that comes off when the fixture breaks (a skeleton's cap):
        // a visual of its own, drawn while broken, flying off its seat as a
        // corpse's headwear is shot off. Its placement in the map is its
        // seat, rigid (no scale), its origin the body's centre.
        if let Some(loose) = row.get(c"loose")? {
            for visual in loose.entries()?.iter() {
                let index = VisualIndex::from(visual?.table()?.required(c"index")?.integer()?);
                let binding = Binding::Loose { piece: pieces.len() };
                let seat = plan.loose_visual(index, binding)?;
                let seat = crate::ragdoll::pose_of(&seat.matrix()).ok_or(MetadataError::Seat)?;
                pieces.push(Piece { lamp, seat, flight: Flight::Seated }).map_err(|_| MetadataError::Count)?;
            }
        }
        for light in row.required(c"lights")?.entries()?.iter() {
            let position = position(&light?.table()?, [c"x", c"y", c"z"])?;
            plan.light_at(position, Binding::Part { lamp, visible_when: Condition::Intact })?;
        }
        // A glow that is an ambient light (an alarm lamp's): few fixtures
        // have one, the others' rows have no table.
        if let Some(lights) = row.get(c"ambient_lights")? {
            for light in lights.entries()?.iter() {
                let position = position(&light?.table()?, [c"x", c"y", c"z"])?;
                plan.ambient_light_at(position, Binding::Part { lamp, visible_when: Condition::Intact })?;
            }
        }
        // A kerosene lamp's flame, a particle: the same, in a table of its own.
        if let Some(particles) = row.get(c"particles")? {
            for particle in particles.entries()?.iter() {
                let position = position(&particle?.table()?, [c"x", c"y", c"z"])?;
                plan.particle_at(position, Binding::Part { lamp, visible_when: Condition::Intact })?;
            }
        }
        // The room's environment probe a lamp's light is baked into: once the
        // lamp is broken it is drawn with the share of its light that is not
        // the lamp's (`keeps`, authored from its cubemap).
        if let Some(probes) = row.get(c"probes")? {
            for probe in probes.entries()?.iter() {
                let probe = probe?.table()?;
                let keeps = Share::parse(probe.required(c"keeps")?.number()?).ok_or(MetadataError::Share)?;
                plan.probe_at(position(&probe, [c"x", c"y", c"z"])?, keeps, Binding::Part { lamp, visible_when: Condition::Intact })?;
            }
        }
    }
    Ok(Some(plan.with_data(Lamps { fixtures, pieces, clock: FrameClock::UNSTARTED, state: Snapshot::intact(count), material,
                                   effect })))
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
pub fn on_present(ctx: &mut Context<'_>) -> Flow<NoExit> {
    // The pieces' step: the bullets' workers have joined, the level is the
    // world's own (the ragdolls', patch 76), the frame the real time.
    if let Some(world) = GameWorld::at_after_move(ctx) && let Some(physics) = world.physics_world() {
        let thread = ctx.thread();
        PROJECT.with_physics(thread, physics.id(), |lamps| {
            let dt = lamps.clock.tick(thread);
            if !lamps.pieces.is_empty() { lamps.fly(dt, &mut crate::ragdoll::GameLevel::new(Some(physics), PIECE_RAYS)); }
        });
    }
    PROJECT.present(ctx);
    Flow::Continue
}
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
pub fn on_ambient_light_insert(_: &vostok::OrigAmbientLightInsert, light: &mut vostok::ObjectAmbientLight<'_>) {
    PROJECT.ambient_light_insert(light);
}
pub fn on_ambient_light_remove(_: &vostok::OrigAmbientLightRemove, light: &mut vostok::ObjectAmbientLight<'_>) {
    PROJECT.ambient_light_remove(light);
}
pub fn on_particle_insert(_: &vostok::OrigParticleInsert, particle: &mut vostok::ObjectParticleVisual<'_>) {
    PROJECT.particle_insert(particle);
}
pub fn on_particle_remove(_: &vostok::OrigParticleRemove, particle: &mut vostok::ObjectParticleVisual<'_>) {
    PROJECT.particle_remove(particle);
}
pub fn on_probe_insert(_: &vostok::OrigProbeInsert, probe: &mut vostok::ObjectEnvironmentProbe<'_>) {
    PROJECT.probe_insert(probe);
}
pub fn on_probe_remove(_: &vostok::OrigProbeRemove, probe: &mut vostok::ObjectEnvironmentProbe<'_>) {
    PROJECT.probe_remove(probe);
}
