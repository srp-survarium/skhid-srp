//! Breakable lamps: safe simulation decisions over typed engine views.
//! Project registration, rendering and teardown belong to ProjectRegistry.
#![forbid(unsafe_code)]

pub mod model;

use crate::vostok::{self, Context, Flow, Lock, NoExit, Thread};
use crate::vostok::clock::FrameClock;
use crate::vostok::resources::{self, Arrived, QueryResults, QueryTag, Serial};
use crate::vostok::sound::Emitter;
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
use model::{bell_seed, shot_seed, Bell, BellEvent, Bodies, Bounds, BreakEffect, BulletSegment, Condition, Distance, Fixture, GlassLine, LampCount,
            LampEvent, LampId, MAX_LAMPS, MAX_PIECES, PieceKind, RayDirection, RingEvery, Snapshot, Visibility,
            Perch, WareAxis, WareShape, WorldPoint, pane_frame};

#[derive(Clone, Copy)]
enum Binding {
    /// Drawn while its fixture is in `visible_when` (a model, a light).
    Part { lamp: LampId, visible_when: Condition },
    /// A broken fixture's loose piece, `pieces[piece]`: drawn where its flight is.
    Loose { piece: usize },
}

/// What is left of a fixture's collision once it broke, and so whether a
/// mark of the break material in its bounds is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leftover {
    /// The glass's collision stays where the glass was (every lamp): a mark
    /// there would hang in the air, intact or broken.
    Glass,
    /// The figure's own collision, which the broken model still covers (a
    /// skeleton's head without its cap): marked once broken, never while the
    /// piece that flies off is on it.
    Body,
}

/// A fixture's loose piece: the visual seated at `seat` (its placement in
/// the map) while the fixture is intact.
#[derive(Clone, Copy)]
/// A fixture's loose piece: its `look` its fixture's (what a shot that
/// knocks it plays: glass on a shard, metal on a can).
struct Piece { lamp: LampId, seat: Pose, kind: PieceKind, look: Look, perch: Perch }

/// How a breaking fixture looks when it breaks, and its marks: the table's
/// unless its row says otherwise (a skeleton's cap or a tin can among
/// School's lamps).
#[derive(Clone, Copy)]
struct Look {
    /// Its collision's material (a lamp's glass, a skeleton's stone).
    material: MaterialId,
    /// The material whose pair with `material` plays its break, whatever
    /// broke it: bullet x glass, the game's own glass shards and sound.
    effect: MaterialId,
    leftover: Leftover,
}

/// What a fixture does when a shot or a blast reaches it.
#[derive(Clone, Copy)]
enum Reaction {
    /// It breaks, once, fixture `lamp` of the history.
    Break { lamp: LampId, look: Look },
    /// It rings (School's bells); it never breaks.
    Ring(Bell),
}

#[derive(Clone, Copy)]
struct Row { fixture: Fixture, reaction: Reaction }

struct Lamps {
    rows: BoundedVec<Row, MAX_LAMPS>,
    pieces: BoundedVec<Piece, MAX_PIECES>,
    /// The bodies of the pieces in the air.
    bodies: Bodies,
    /// The pieces' frames: the real time between two presents.
    clock: FrameClock,
    /// The presents' time since the project's first, s (the bells').
    now: f32,
    /// The breaking rows' conditions.
    state: Snapshot,
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
        let piece = self.piece(piece);
        VisualTransform::parse(crate::ragdoll::matrix_of(piece.kind.drawn(self.bodies.pose(piece.perch)?)))
    }
}
/// The level's rays a frame for the pieces (two an end of a moving one: a
/// Vector capsule's 24 shards at once, two capsules' in one blast; past
/// them a sweep waits a frame).
const PIECE_RAYS: u32 = 96;
/// The most bells one frame rings.
const MAX_RINGS: usize = 8;
/// A bell's ring this frame: where (its glass line's middle) and its take.
#[derive(Clone, Copy)]
struct Ring { at: WorldPoint, seed: u32 }
fn piece_point(pose: Pose) -> Option<WorldPoint> {
    WorldPoint::parse([pose.position.x, pose.position.y, pose.position.z])
}
impl Lamps {
    fn piece(&self, index: usize) -> &Piece { self.pieces.get(index).expect("a bound piece") }
    fn shoot(&mut self, segment: BulletSegment, seed: u32, shatter: impl FnMut(GlassLine, RayDirection, Look),
             knock: impl FnMut(WorldPoint, RayDirection, Look)) {
        self.hit(|bounds| bounds.intersects(segment), |_| segment.direction(), seed, shatter);
        // A piece off its seat has no collision: the ray passes it by (to
        // what it hits behind), and knocks it on where it passes near.
        self.kick(|point, reach| segment.passes_within(point, reach).then(|| segment.direction()), knock);
    }
    /// A blast breaks every fixture whose bounds come within its splash
    /// radius of the centre, where the game's splash reaches them (as it
    /// would a player's bone there: walls stop it, glass does not).
    fn explode(&mut self, centre: WorldPoint, radius: Distance, reaches: impl Fn(WorldPoint) -> bool,
               shatter: impl FnMut(GlassLine, RayDirection, Look), knock: impl FnMut(WorldPoint, RayDirection, Look)) {
        let seed = shot_seed(centre.coordinates());
        self.hit(|bounds| {
            let nearest = bounds.nearest(centre);
            centre.within(nearest, radius) && reaches(nearest)
        }, |glass| RayDirection::between(centre, glass.middle()).unwrap_or_else(down), seed, shatter);
        self.kick(|point, reach| radius.plus(reach).is_some_and(|reach| centre.within(point, reach) && reaches(point))
            .then(|| RayDirection::between(centre, point).unwrap_or_else(down)), knock);
    }
    /// Each piece off its seat that `push` reaches (at its centre, within
    /// its kind's reach) goes on along the push, and the first KNOCKS of
    /// them `knock` (the hit of its fixture's look at it: glass struck again
    /// sounds as glass; the owner, 2026-11-27: "hitting shards doesn't make
    /// glass sound"; a blast through a heap of shards one sound or four,
    /// not fifty).
    fn kick(&mut self, push: impl Fn(WorldPoint, Distance) -> Option<RayDirection>,
            mut knock: impl FnMut(WorldPoint, RayDirection, Look)) {
        const KNOCKS: usize = 4;
        let mut knocked = 0;
        for piece in self.pieces.iter_mut() {
            let reach = piece.kind.reach();
            let Some(point) = self.bodies.pose(piece.perch).and_then(piece_point) else { continue };
            if let Some(direction) = push(point, reach) {
                piece.perch = self.bodies.kick(piece.perch, direction);
                if knocked < KNOCKS {
                    knock(point, direction, piece.look);
                    knocked += 1;
                }
            }
        }
    }
    /// The one transition of every break, a bullet's or a blast's: a first
    /// hit of an intact fixture shatters it, once, and knocks its loose
    /// pieces off along the push. A bell it reaches rings, the take the
    /// shot's `seed` picks.
    fn hit(&mut self, struck: impl Fn(Bounds) -> bool, push: impl Fn(GlassLine) -> RayDirection, seed: u32,
           mut shatter: impl FnMut(GlassLine, RayDirection, Look)) {
        let now = self.now;
        for (index, row) in self.rows.iter_mut().enumerate() {
            match &mut row.reaction {
                Reaction::Break { lamp, look } => {
                    if self.state.condition(*lamp) == Condition::Intact && struck(row.fixture.bounds)
                        && self.state.transition(*lamp, LampEvent::Hit) == BreakEffect::Shatter {
                        let direction = push(row.fixture.glass);
                        for piece in self.pieces.iter_mut().filter(|piece| piece.lamp == *lamp) {
                            piece.perch = self.bodies.strike(piece.perch, direction);
                        }
                        shatter(row.fixture.glass, direction, *look);
                    }
                }
                Reaction::Ring(bell) => {
                    if struck(row.fixture.bounds) {
                        *bell = bell.ring(BellEvent::Struck { now, seed: bell_seed(seed, index) }).0;
                    }
                }
            }
        }
    }
    /// The pieces' frame of `dt` s in the level: each follows its fixture's
    /// condition (a rollback seats it again).
    fn fly(&mut self, dt: f32, level: &mut impl ::ragdoll::ragdoll::Level) {
        for piece in self.pieces.iter_mut() {
            piece.perch = self.bodies.step(piece.perch, piece.kind, self.state.condition(piece.lamp), piece.seat, dt,
                                           Vec3::Y, level);
        }
    }
    /// A frame of the bells, at `now`: each that rings now.
    fn ring(&mut self, now: f32) -> BoundedVec<Ring, MAX_RINGS> {
        let mut rings = BoundedVec::new();
        for row in self.rows.iter_mut() {
            if let Reaction::Ring(bell) = &mut row.reaction {
                let (next, play) = bell.ring(BellEvent::Frame { now });
                *bell = next;
                // past MAX_RINGS a frame, unheard (a blast among more bells)
                if let Some(seed) = play { let _ = rings.push(Ring { at: row.fixture.glass.middle(), seed }); }
            }
        }
        rings
    }
    fn has_bells(&self) -> bool { self.rows.iter().any(|row| matches!(row.reaction, Reaction::Ring(_))) }
    /// Whether a hit's mark would be on a fixture's glass. The game's decal
    /// is a world-space box no model owns: one on the glass would stay in the
    /// air, or on the broken model behind it, once the glass is gone. A shot
    /// at a fixture's glass breaks it, so none is drawn there, intact or
    /// broken: the rule needs no lamp state, and rollback, resimulation and
    /// late joins (which carry no decals) agree with it. A figure's (Leftover::
    /// Body) is its piece's while intact and its own once broken: marked then.
    fn marks_glass(&self, surface: MaterialId, point: WorldPoint) -> bool {
        self.rows.iter().any(|row| match row.reaction {
            Reaction::Break { lamp, look } => surface == look.material && row.fixture.bounds.holds(point, mark_tolerance())
                && match look.leftover {
                    Leftover::Glass => true,
                    Leftover::Body => self.state.condition(lamp) == Condition::Intact,
                },
            // a bell keeps its own (none: School's have no collision)
            Reaction::Ring(_) => false,
        })
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
/// A loose piece knocked again: one burst of its look's hit at it, its sound there.
fn knock(effects: Option<SurfaceBreakEffects<'_>>, point: WorldPoint, direction: RayDirection) {
    if let Some(effects) = effects { effects.play([point], point, direction); }
}
/// A bell's ring's takes (pack school-bell): each a collection of one take,
/// a point sound at the bell (sounds/collections/<this>). A ring's seed
/// picks one (the owner, 2026-11-27: "can be w/some randomness, so everyone
/// hears the same one"): every game's of one shot the same.
const RING_TAKES: [&core::ffi::CStr; 10] = [
    c"props/school_bell_srp/ring_0", c"props/school_bell_srp/ring_1", c"props/school_bell_srp/ring_2",
    c"props/school_bell_srp/ring_3", c"props/school_bell_srp/ring_4", c"props/school_bell_srp/ring_5",
    c"props/school_bell_srp/ring_6", c"props/school_bell_srp/ring_7", c"props/school_bell_srp/ring_8",
    c"props/school_bell_srp/ring_9",
];

/// A take: asked for at the first frame of a project with bells, kept for
/// the process.
enum RingLoad {
    Unasked,
    Asked,
    Ready(Emitter),
    /// Not in the game's data: this take is silent this session.
    Missing,
}
static RING: Lock<[RingLoad; RING_TAKES.len()]> = Lock::new([const { RingLoad::Unasked }; RING_TAKES.len()]);

/// Take `k`'s query's serial: the first and those after it.
fn take_serial(k: usize) -> Serial {
    (0..k).fold(Serial::FIRST, |serial, _| serial.next())
}

/// Plays each ring at its bell, the takes' queries asked first (a take is
/// heard from the frame it has loaded).
fn play_rings(thread: &Thread, world: &GameWorld<'_>, rings: &BoundedVec<Ring, MAX_RINGS>) {
    let ask = RING.with(thread, |takes| match takes[0] {
        RingLoad::Unasked => {
            for take in takes.iter_mut() { *take = RingLoad::Asked; }
            true
        }
        RingLoad::Asked | RingLoad::Ready(_) | RingLoad::Missing => false,
    });
    if ask {
        for (k, path) in RING_TAKES.iter().enumerate() {
            resources::query_collection(thread, path, take_serial(k), vostok::lamps_on_loaded_entry());
        }
    }
    if rings.is_empty() { return; }
    RING.with(thread, |takes| {
        for ring in rings.iter() {
            if let RingLoad::Ready(sound) = &takes[ring.seed as usize % RING_TAKES.len()] {
                world.play_sound_at(sound, ring.at);
            }
        }
    });
}

/// A take's query came back (on the game's thread, which asked).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let Arrived::Track(serial, sound) = results.arrived(tag) else { return };
    let Some(k) = (0..RING_TAKES.len()).find(|&k| take_serial(k) == serial) else { return };
    let missing = sound.is_none();
    RING.with(results.thread(), |takes| takes[k] = sound.map_or(RingLoad::Missing, RingLoad::Ready));
    if missing { vostok::log::line(&[Text("lamps: a bell's ring did not load; it is silent this session")]); }
}

/// The game moves a hit's point onto the struck plane to 1 mm; the authored
/// bounds hold the fixture's collision. A margin for both and for rounding.
fn mark_tolerance() -> Distance { Distance::parse(0.01).expect("a finite, non-negative distance") }
static PROJECT: ProjectRegistry<Lamps, Binding> = ProjectRegistry::new();

#[derive(Debug)]
enum MetadataError { Binding(BindingError), Config(ConfigError), Version, Material, Count, Bounds, Condition, Share, Seat,
                     Leftover, PieceKind, Reaction, Shape }
/// The metadata's format: 3 adds effect_material and each fixture's glass
/// line. Optional keys (the table's leftover, a row's break_material,
/// effect_material, leftover, reaction, ambient_lights, particles, probes,
/// loose, a piece's kind and its shape) need none: a client without them
/// leaves them out.
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

/// A direction of three keys (finite).
fn vector(table: &ConfigTable<'_>, keys: [&core::ffi::CStr; 3]) -> Result<Vec3, MetadataError> {
    let mut v = [0.0; 3];
    for (value, key) in v.iter_mut().zip(keys) { *value = table.required(key)?.number()?; }
    let v = Vec3::new(v[0], v[1], v[2]);
    if v.is_finite() { Ok(v) } else { Err(MetadataError::Shape) }
}

/// A table's or a row's look: `defaults` for each key it leaves out (a
/// table's own: none, its leftover glass).
fn look(table: &ConfigTable<'_>, defaults: Option<Look>) -> Result<Look, MetadataError> {
    let material_at = |key: &core::ffi::CStr, default: Option<MaterialId>| -> Result<MaterialId, MetadataError> {
        match table.get(key)? {
            Some(value) => MaterialId::parse(value.integer()?).ok_or(MetadataError::Material),
            None => default.ok_or(MetadataError::Material),
        }
    };
    let leftover = match table.get(c"leftover")? {
        None => defaults.map_or(Leftover::Glass, |look| look.leftover),
        Some(value) => match value.text()?.to_bytes() {
            b"glass" => Leftover::Glass,
            b"body" => Leftover::Body,
            _ => return Err(MetadataError::Leftover),
        },
    };
    Ok(Look { material: material_at(c"break_material", defaults.map(|look| look.material))?,
              effect: material_at(c"effect_material", defaults.map(|look| look.effect))?, leftover })
}

/// What a row does: `reaction` "break" (the default) or "ring", every
/// `ring_every` s at most.
fn rings(row: &ConfigTable<'_>) -> Result<Option<RingEvery>, MetadataError> {
    match row.get(c"reaction")? {
        None => Ok(None),
        Some(value) => match value.text()?.to_bytes() {
            b"break" => Ok(None),
            b"ring" => RingEvery::parse(row.required(c"ring_every")?.number()?).map(Some).ok_or(MetadataError::Reaction),
            _ => Err(MetadataError::Reaction),
        },
    }
}

/// A loose piece's kind (its `kind`, "headwear" the default), seated at
/// `seat`, of a fixture whose glass's middle is `middle`.
fn piece_kind(visual: &ConfigTable<'_>, seat: Pose, middle: WorldPoint) -> Result<PieceKind, MetadataError> {
    let Some(kind) = visual.get(c"kind")? else { return Ok(PieceKind::Headwear) };
    let [x, y, z] = middle.coordinates();
    let outward = seat.position - Vec3::new(x, y, z);
    Ok(match kind.text()?.to_bytes() {
        b"headwear" => PieceKind::Headwear,
        b"shard" => PieceKind::Shard { outward },
        // its glass's outward normal and its long side, in its visual's frame
        b"pane_shard" => {
            let out = vector(visual, [c"out_x", c"out_y", c"out_z"])?;
            let frame = pane_frame(vector(visual, [c"along_x", c"along_y", c"along_z"])?, out)
                .ok_or(MetadataError::Shape)?;
            PieceKind::PaneShard { out: seat.rotation.rotate(out), frame }
        }
        b"ware" => {
            let shape = WareShape::new(visual.required(c"half")?.number()?, visual.required(c"radius")?.number()?)
                .ok_or(MetadataError::Shape)?;
            let axis = match visual.required(c"axis")?.text()?.to_bytes() {
                b"x" => WareAxis::X,
                b"y" => WareAxis::Y,
                b"z" => WareAxis::Z,
                _ => return Err(MetadataError::Shape),
            };
            PieceKind::Ware { shape, frame: axis.frame() }
        }
        _ => return Err(MetadataError::PieceKind),
    })
}

fn prepare(project: ProjectView<'_>) -> Result<Option<ProjectPlan<'_, Lamps, Binding>>, MetadataError> {
    let root = project.root()?.table()?;
    let Some(spec) = root.get(c"srp_lamps")? else { return Ok(None) };
    let spec = spec.table()?;
    if spec.required(c"version")?.integer()? != VERSION { return Err(MetadataError::Version); }
    let table_look = look(&spec, None)?;
    let rows = spec.required(c"lamps")?.entries()?;
    // The history's fixtures: the rows that break (a table of bells alone
    // is refused: none to keep).
    let mut breaking = 0u32;
    for row in rows.iter() {
        if rings(&row?.table()?)?.is_none() { breaking += 1; }
    }
    let count = LampCount::parse(breaking).ok_or(MetadataError::Count)?;
    let methods = MethodTables::new(vostok::lamps_visual_vtable_handle(), vostok::lamps_light_vtable_handle(),
                                    vostok::lamps_ambient_light_vtable_handle(), vostok::lamps_particle_vtable_handle(),
                                    vostok::lamps_probe_vtable_handle());
    let mut plan = project.plan((), methods)?;
    let mut table = BoundedVec::new();
    let mut pieces = BoundedVec::new();
    let mut lamps = 0;
    for row in rows.iter() {
        let row = row?.table()?;
        let low = position(&row, [c"min_x", c"min_y", c"min_z"])?;
        let high = position(&row, [c"max_x", c"max_y", c"max_z"])?;
        let bounds = Bounds::parse(low, high).ok_or(MetadataError::Bounds)?;
        let glass = GlassLine::new(position(&row, [c"glass_from_x", c"glass_from_y", c"glass_from_z"])?,
                                   position(&row, [c"glass_to_x", c"glass_to_y", c"glass_to_z"])?);
        let fixture = Fixture { bounds, glass };
        // A bell has its bounds and its place alone: nothing of it is drawn
        // otherwise, nothing comes off it.
        if let Some(every) = rings(&row)? {
            table.push(Row { fixture, reaction: Reaction::Ring(Bell::new(every)) }).map_err(|_| MetadataError::Count)?;
            continue;
        }
        let lamp = count.id(lamps).expect("a breaking row of the counted ones");
        lamps += 1;
        let row_look = look(&row, Some(table_look))?;
        table.push(Row { fixture, reaction: Reaction::Break { lamp, look: row_look } })
            .map_err(|_| MetadataError::Count)?;
        for visual in row.required(c"visuals")?.entries()?.iter() {
            let visual = visual?.table()?;
            let visible_when = Condition::parse(visual.required(c"state")?.integer()?).ok_or(MetadataError::Condition)?;
            plan.visual(VisualIndex::from(visual.required(c"index")?.integer()?), Binding::Part { lamp, visible_when })?;
        }
        // A piece that comes off when the fixture breaks (a skeleton's cap, a
        // bottle's shards, a whole tin can): a visual of its own, drawn while
        // broken, flying off its seat. Its placement in the map is its seat,
        // rigid (no scale), its origin the body's centre. Its `kind`:
        // "headwear" (the default) flies as a corpse's headwear is shot off,
        // "shard" out from the glass's middle, "pane_shard" out of its frame
        // (its `out_*` normal, its body along its `along_*`), "ware" jumps
        // off whole (its capsule's `half`, `radius` and `axis`).
        if let Some(loose) = row.get(c"loose")? {
            for visual in loose.entries()?.iter() {
                let visual = visual?.table()?;
                let index = VisualIndex::from(visual.required(c"index")?.integer()?);
                let binding = Binding::Loose { piece: pieces.len() };
                let seat = plan.loose_visual(index, binding)?;
                let seat = crate::ragdoll::pose_of(&seat.matrix()).ok_or(MetadataError::Seat)?;
                let kind = piece_kind(&visual, seat, glass.middle())?;
                pieces.push(Piece { lamp, seat, kind, look: row_look, perch: Perch::Seated }).map_err(|_| MetadataError::Count)?;
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
    Ok(Some(plan.with_data(Lamps { rows: table, pieces, bodies: Bodies::new(), clock: FrameClock::UNSTARTED, now: 0.0,
                                   state: Snapshot::intact(count) })))
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
        let rings = PROJECT.with_physics(thread, physics.id(), |lamps| {
            let dt = lamps.clock.tick(thread);
            lamps.now += dt;
            if !lamps.pieces.is_empty() { lamps.fly(dt, &mut crate::ragdoll::GameLevel::new(Some(physics), PIECE_RAYS)); }
            lamps.has_bells().then(|| lamps.ring(lamps.now))
        }).flatten();
        // the rings outside the registry's lock, on the game's thread
        if let Some(rings) = rings { play_rings(thread, &world, &rings); }
    }
    PROJECT.present(ctx);
    Flow::Continue
}
pub fn on_ray(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(ray) = BulletRay::at(ctx) && let Some(physics) = ray.physics() {
        let seed = shot_seed(ray.origin());
        let segment = (|| {
            let hit = match ray.struck() { Some(p) => Some(WorldPoint::parse(p)?), None => None };
            BulletSegment::clipped(WorldPoint::parse(ray.origin())?, RayDirection::parse(ray.direction())?,
                                   Distance::parse(ray.length())?, hit)
        })();
        if let Some(segment) = segment {
            PROJECT.with_physics(ctx.thread(), physics, |lamps| {
                lamps.shoot(segment, seed, |glass, direction, look| {
                    shatter(ray.surface_break_effects(look.effect, look.material), glass, direction);
                }, |point, direction, look| {
                    knock(ray.surface_break_effects(look.effect, look.material), point, direction);
                });
            });
        }
    }
    Flow::Continue
}
pub fn on_explode(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(blast) = Explosion::at(ctx) && let Some(physics) = blast.physics() {
        PROJECT.with_physics(ctx.thread(), physics, |lamps| {
            lamps.explode(blast.centre(), blast.radius(), |point| blast.reaches(point), |glass, direction, look| {
                shatter(blast.surface_break_effects(look.effect, look.material), glass, direction);
            }, |point, direction, look| {
                knock(blast.surface_break_effects(look.effect, look.material), point, direction);
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
