//! A project's registered presentation. This owner alone installs/restores
//! object vtables and changes renderer membership. No native handle escapes
//! into patch data; project removal and destruction end the registration.
use core::{marker::PhantomData, num::NonZeroU32};
use crate::raw;
use super::{Context, Handle, Lock, Thread};
use super::collection::BoundedVec;
use super::config::{ConfigError, ConfigValue};
use super::geometry::WorldPoint;
use super::presentation::{Membership, PresentationEvent, RendererAction, Visibility};

const MAX_BINDINGS: usize = 1024;
/// The loose visuals of a project (ProjectPlan::loose_visual): School's
/// three bottles' 24 shards and room to spare.
pub const MAX_LOOSE: usize = 32;

/// A generated immutable vtable of the native layout T. Only generated
/// glue constructs one, after checking every replaced slot against the exe.
#[derive(Clone, Copy)]
pub struct NativeVtable<T> { address: NonZeroU32, layout: PhantomData<fn() -> T> }
impl<T> NativeVtable<T> {
    /// # Safety
    /// The address names a static table of layout T with its original RTTI;
    /// all replaced slots have generated native-convention entry points.
    pub(in crate::vostok) unsafe fn from_address(address: u32) -> Self {
        Self { address: NonZeroU32::new(address).expect("linked vtable"), layout: PhantomData }
    }

    /// The table's address, the word an object of layout T's vtable field
    /// takes (vostok::console's command).
    pub(in crate::vostok) fn address(self) -> u32 {
        self.address.get()
    }
}

pub struct MethodTables {
    visual: NativeVtable<raw::RenderVisualMethods>,
    light: NativeVtable<raw::ObjectLightMethods>,
    ambient_light: NativeVtable<raw::ObjectAmbientLightMethods>,
    particle: NativeVtable<raw::ObjectParticleVisualMethods>,
    probe: NativeVtable<raw::ObjectEnvironmentProbeMethods>,
}
impl MethodTables {
    pub fn new(visual: NativeVtable<raw::RenderVisualMethods>, light: NativeVtable<raw::ObjectLightMethods>,
               ambient_light: NativeVtable<raw::ObjectAmbientLightMethods>,
               particle: NativeVtable<raw::ObjectParticleVisualMethods>,
               probe: NativeVtable<raw::ObjectEnvironmentProbeMethods>) -> Self {
        Self { visual, light, ambient_light, particle, probe }
    }
}

macro_rules! native_view {
    ($name:ident) => {
        pub struct $name<'t> { native: raw::$name, thread: &'t Thread }
        impl<'t> Handle<'t> for $name<'t> {
            type Raw = raw::$name;
            unsafe fn from_raw(thread: &'t Thread, native: Option<Self::Raw>) -> Option<Self> {
                Some(Self { native: native?, thread })
            }
        }
        impl $name<'_> {
            pub(in crate::vostok) fn raw(&self) -> raw::$name { self.native }
        }
    };
}
native_view!(RenderVisual);
native_view!(ObjectLight);
native_view!(ObjectAmbientLight);
native_view!(ObjectParticleVisual);
native_view!(ObjectEnvironmentProbe);
native_view!(GameScene);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsId(NonZeroU32);
impl PhysicsId {
    pub(super) fn of(physics: raw::BulletPhysicsWorld) -> Self {
        Self(NonZeroU32::new(physics.address()).expect("non-null native handle"))
    }
    /// A made-up world's (the tests').
    #[cfg(test)]
    pub fn made_up(id: u32) -> Self {
        Self(NonZeroU32::new(id).expect("a made-up world's id is not 0"))
    }
}

/// Where a loose visual is drawn: a row-vector 4x4 (rows i, j, k, place),
/// every value finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualTransform([f32; 16]);
impl VisualTransform {
    pub fn parse(matrix: [f32; 16]) -> Option<Self> { matrix.iter().all(|v| v.is_finite()).then_some(Self(matrix)) }
    pub fn matrix(self) -> [f32; 16] { self.0 }
}

/// Index in the authored render-visual array, distinct from a lamp id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualIndex(u32);
impl From<u32> for VisualIndex { fn from(value: u32) -> Self { Self(value) } }

/// A project (simple_game_project) as an id: compared, never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectId(NonZeroU32);
impl ProjectId {
    fn of(project: raw::SimpleGameProject) -> Self {
        Self(NonZeroU32::new(project.address()).expect("non-null native handle"))
    }
    /// The project project.removed is at (remove_static's epilogue, after the native removals).
    pub fn removed(ctx: &Context<'_>) -> Option<Self> {
        // SAFETY: the site's ESI is the project being removed; only its address is kept.
        unsafe { raw::site::project_removed::project(ctx) }.map(Self::of)
    }
    /// The project project.destroy is at (its destructor's entry).
    pub fn destroyed(ctx: &Context<'_>) -> Option<Self> {
        // SAFETY: the site's ECX is the project being destroyed; only its address is kept.
        unsafe { raw::site::project_destroy::project(ctx) }.map(Self::of)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BindingError {
    MissingProject, MissingScene, MissingPhysics, InvalidObjects, InvalidVisual,
    UnknownVisualType, UnknownLight, AmbiguousLight, UnknownAmbientLight, AmbiguousAmbientLight, UnknownParticle, AmbiguousParticle,
    UnknownProbe, AmbiguousProbe,
    DuplicateObject, Capacity,
    OtherProjectActive, Config(ConfigError),
}
impl From<ConfigError> for BindingError {
    fn from(value: ConfigError) -> Self { Self::Config(value) }
}

/// The live project at its insertion hook. A prepared plan borrows this
/// hook, so it cannot be retained or installed after native initialization.
#[derive(Clone, Copy)]
pub struct ProjectView<'t> {
    native: raw::SimpleGameProject,
    frame: raw::ProjectInsertFrame,
    thread: &'t Thread,
}

struct PreparedProject<'t> {
    native: raw::SimpleGameProject,
    physics: PhysicsId,
    scene: raw::GameScene,
    _thread: &'t Thread,
}

impl<'t> ProjectView<'t> {
    fn inserting(ctx: &'t Context<'_>) -> Result<Self, BindingError> {
        use raw::site::project_insert as site;
        // SAFETY: this is the project.insert detour, before the native
        // insert_static call. The entry supplies the physics world in its
        // first argument; the project's camera is not a physics world.
        unsafe {
            let native = site::project(ctx).ok_or(BindingError::MissingProject)?;
            let frame = site::frame(ctx).ok_or(BindingError::MissingPhysics)?;
            Ok(Self { native, frame, thread: ctx.thread() })
        }
    }
    /// The project at project.insert, for a patch that reads only its
    /// config and its scene (patch 130); the lamps' registry makes its own.
    pub fn at_insert(ctx: &'t Context<'_>) -> Result<Self, BindingError> { Self::inserting(ctx) }
    pub fn id(&self) -> ProjectId { ProjectId::of(self.native) }
    /// The render scene its game scene holds (game_scene+4), the one the
    /// renderer's context names while that scene is drawn; None without one.
    pub fn render_scene(&self) -> Option<super::render_stage::RenderSceneId> {
        // SAFETY: the inserted project's game scene is alive for the hook;
        // only the render scene's address is kept, as an id.
        unsafe { self.native.scene().and_then(|scene| scene.render_scene()).map(super::render_stage::RenderSceneId::of) }
    }
    pub fn root(&self) -> Result<ConfigValue<'t>, BindingError> {
        // SAFETY: the inserted project owns the config resource for the hook.
        unsafe {
            let root = self.native.config().and_then(|config| config.root()).ok_or(ConfigError::Missing)?;
            Ok(ConfigValue::read(root)?)
        }
    }
    pub fn plan<D, K>(self, data: D, methods: MethodTables) -> Result<ProjectPlan<'t, D, K>, BindingError> {
        // SAFETY: the configured project is still inside its insertion hook.
        // Scenery without metadata never needs these optional engine inputs.
        let project = unsafe { PreparedProject {
            native: self.native, scene: self.native.scene().ok_or(BindingError::MissingScene)?,
            physics: PhysicsId::of(self.frame.physics().ok_or(BindingError::MissingPhysics)?), _thread: self.thread,
        } };
        Ok(ProjectPlan { project, data, methods, bindings: BoundedVec::new(), loose: BoundedVec::new() })
    }
}

/// The share of its light a bound probe keeps while its binding is Hidden:
/// a fraction in (0, 1].
#[derive(Clone, Copy, Debug)]
pub struct Share(f32);
impl Share {
    pub fn parse(value: f32) -> Option<Self> { (value > 0.0 && value <= 1.0).then_some(Self(value)) }
}

/// A probe's diffuse and specular multipliers (+3F0h, +3F4h) and radius
/// (+3ECh): its own, and those it is drawn with while its binding is Hidden:
/// the multipliers dimmed, the radius `DRAWN_OVER` less, so that it is drawn
/// over the probes of its own radius, whose light would show through where
/// they overlap it.
#[derive(Clone, Copy)]
struct ProbeLight { own: ProbeFields, dimmed: ProbeFields }
#[derive(Clone, Copy)]
struct ProbeFields { diffuse: f32, specular: f32, radius: f32 }
/// The renderer draws the visible probes largest sort key first; a render
/// probe's key is its radius plus a number in [0, 0.1) its constructor makes
/// from its heap address (+380h, 0x1C2F30..0x1C2FA6; set_properties 0x1C370F
/// adds it), so probes of one radius come in no set order. 0.1 less puts the
/// dimmed probe after every one of its radius, whatever their numbers and its
/// own. A box probe's volume and falloff are its transform and side width,
/// not its radius; a sphere's would draw 10 cm smaller (London binds boxes).
const DRAWN_OVER: f32 = 0.1;
impl ProbeLight {
    fn new(own: ProbeFields, keeps: Share) -> Self {
        let dimmed = ProbeFields { diffuse: own.diffuse * keeps.0, specular: own.specular * keeps.0,
                                   radius: own.radius - DRAWN_OVER };
        Self { own, dimmed }
    }
    fn at(self, membership: Membership) -> ProbeFields {
        if membership == Membership::Hidden { self.dimmed } else { self.own }
    }
}

#[derive(Clone, Copy)]
enum NativeObject {
    Visual(raw::RenderVisual),
    /// A visual its patch moves ([`Presentation::placement`]): its place is
    /// written into its transform, and its model moved while it is drawn.
    Loose(raw::RenderVisual),
    Light(raw::ObjectLight), AmbientLight(raw::ObjectAmbientLight),
    Particle(raw::ObjectParticleVisual),
    /// Never taken out of the renderer by its lamp: Shown draws it with its
    /// own light, Hidden with the dimmed one.
    Probe(raw::ObjectEnvironmentProbe, ProbeLight),
}
impl NativeObject {
    /// The native object's address, its identity.
    fn address(self) -> u32 {
        match self {
            Self::Visual(v) | Self::Loose(v) => v.address(), Self::Light(l) => l.address(), Self::AmbientLight(l) => l.address(),
            Self::Particle(p) => p.address(), Self::Probe(p, _) => p.address(),
        }
    }
}

struct Binding<K> { object: NativeObject, key: K, membership: Membership }

/// A loose visual's own transform, as the map placed it: where it is drawn
/// while its patch places it nowhere, and what it is given back.
#[derive(Clone, Copy)]
struct Loose { visual: raw::RenderVisual, authored: VisualTransform }

/// An uninstalled, fully owned proposal. Dropping it changes nothing in
/// the game. Every add resolves and type-checks an object; commit is infallible.
pub struct ProjectPlan<'t, D, K> {
    project: PreparedProject<'t>,
    data: D,
    methods: MethodTables,
    bindings: BoundedVec<Binding<K>, MAX_BINDINGS>,
    loose: BoundedVec<Loose, MAX_LOOSE>,
}
impl<'t, D, K> ProjectPlan<'t, D, K> {
    pub fn with_data<T>(self, data: T) -> ProjectPlan<'t, T, K> {
        ProjectPlan { project: self.project, data, methods: self.methods, bindings: self.bindings, loose: self.loose }
    }
    fn add(&mut self, object: NativeObject, key: K) -> Result<(), BindingError> {
        if self.bindings.iter().any(|binding| binding.object.address() == object.address()) { return Err(BindingError::DuplicateObject); }
        self.bindings.push(Binding { object, key, membership: Membership::Detached }).map_err(|_| BindingError::Capacity)?;
        Ok(())
    }
    fn render_visual(&self, index: VisualIndex) -> Result<raw::RenderVisual, BindingError> {
        // SAFETY: the plan borrows the insertion hook. Count bounds the
        // native contiguous visual array; only its native type is accepted.
        unsafe {
            if index.0 >= self.project.native.visual_count().read() { return Err(BindingError::InvalidVisual); }
            let visual = self.project.native.visuals().ok_or(BindingError::InvalidVisual)?.at(index.0 as usize);
            if visual.vtable().read() != raw::image::visual_methods().address() { return Err(BindingError::UnknownVisualType); }
            Ok(visual)
        }
    }
    pub fn visual(&mut self, index: VisualIndex, key: K) -> Result<(), BindingError> {
        let visual = self.render_visual(index)?;
        self.add(NativeObject::Visual(visual), key)
    }
    /// A visual drawn where [`Presentation::placement`] says: the map's
    /// place until it says another. That place, its transform as loaded.
    pub fn loose_visual(&mut self, index: VisualIndex, key: K) -> Result<VisualTransform, BindingError> {
        let visual = self.render_visual(index)?;
        // SAFETY: a visual of the inserted project; its 16 transform floats.
        let authored = VisualTransform::parse(unsafe { super::world::floats(visual.transform()) })
            .ok_or(BindingError::InvalidVisual)?;
        self.add(NativeObject::Loose(visual), key)?;
        self.loose.push(Loose { visual, authored }).map_err(|_| BindingError::Capacity)?;
        Ok(authored)
    }
    /// The project's one object at `position` among those `kind` reads as a
    /// T with its position; `none` or `several` when it is not one.
    fn object_at<T>(&self, position: WorldPoint, kind: impl Fn(raw::ProjectObject) -> Option<(T, [f32; 3])>,
                    none: BindingError, several: BindingError) -> Result<T, BindingError> {
        let mut found = None;
        // SAFETY: the native project vector and its objects live for this insertion.
        unsafe {
            let Some(first) = self.project.native.objects() else { return Err(none) };
            let end = self.project.native.objects_end().ok_or(BindingError::InvalidObjects)?;
            let count = end.index_from(first).ok_or(BindingError::InvalidObjects)?;
            for i in 0..count {
                let object = first.at(i).object().ok_or(BindingError::InvalidObjects)?;
                let Some((native, xyz)) = kind(object) else { continue };
                if xyz == position.coordinates() && found.replace(native).is_some() { return Err(several); }
            }
        }
        found.ok_or(none)
    }
    pub fn light_at(&mut self, position: WorldPoint, key: K) -> Result<(), BindingError> {
        // SAFETY: only an object with the light vtable is read as a light.
        let light = self.object_at(position, |object| unsafe {
            if object.vtable().read() != raw::image::light_methods().address() { return None; }
            Some((object.light(), super::world::floats(object.light().position())))
        }, BindingError::UnknownLight, BindingError::AmbiguousLight)?;
        self.add(NativeObject::Light(light), key)
    }
    pub fn ambient_light_at(&mut self, position: WorldPoint, key: K) -> Result<(), BindingError> {
        // SAFETY: only an object with the ambient light vtable is read as one.
        let light = self.object_at(position, |object| unsafe {
            if object.vtable().read() != raw::image::ambient_light_methods().address() { return None; }
            Some((object.ambient_light(), super::world::floats(object.ambient_light().position())))
        }, BindingError::UnknownAmbientLight, BindingError::AmbiguousAmbientLight)?;
        self.add(NativeObject::AmbientLight(light), key)
    }
    pub fn particle_at(&mut self, position: WorldPoint, key: K) -> Result<(), BindingError> {
        // SAFETY: only an object with the particle object's vtable is read as one.
        let particle = self.object_at(position, |object| unsafe {
            if object.vtable().read() != raw::image::particle_methods().address() { return None; }
            Some((object.particle(), super::world::floats(object.particle().position())))
        }, BindingError::UnknownParticle, BindingError::AmbiguousParticle)?;
        self.add(NativeObject::Particle(particle), key)
    }
    pub fn probe_at(&mut self, position: WorldPoint, keeps: Share, key: K) -> Result<(), BindingError> {
        // SAFETY: only an object with the probe's vtable is read as one; it
        // is loaded (its multipliers read) before the project is inserted.
        let probe = self.object_at(position, |object| unsafe {
            if object.vtable().read() != raw::image::probe_methods().address() { return None; }
            Some((object.probe(), super::world::floats(object.probe().position())))
        }, BindingError::UnknownProbe, BindingError::AmbiguousProbe)?;
        // SAFETY: the probe object_at found is alive and loaded for this insertion.
        let own = unsafe { ProbeFields { diffuse: probe.diffuse_multiplier().read(), specular: probe.specular_multiplier().read(),
                                         radius: probe.radius().read() } };
        self.add(NativeObject::Probe(probe, ProbeLight::new(own, keeps)), key)
    }
    pub fn data_mut(&mut self) -> &mut D { &mut self.data }
}

/// The patch supplies only the meaning of a binding's key. The registry
/// remains the sole owner of renderer membership and native object access.
pub trait Presentation<K> {
    fn visibility(&self, key: K) -> Visibility;
    /// Where a loose visual's binding is drawn now; None: where the map
    /// placed it. Asked once a frame, at present.
    fn placement(&self, _key: K) -> Option<VisualTransform> { None }
}

enum Registration<D, K> { Empty, Bound(BoundProject<D, K>) }
struct BoundProject<D, K> {
    project: ProjectId,
    physics: PhysicsId,
    scene: raw::GameScene,
    data: D,
    bindings: BoundedVec<Binding<K>, MAX_BINDINGS>,
    loose: BoundedVec<Loose, MAX_LOOSE>,
}

// SAFETY: only ProjectRegistry can reach this private owner. Worker access
// exposes D alone; native handles are used solely by the scene lifecycle
// and presentation callbacks, bracketed by insertion/removal/destruction.
// Moving the stored pointer values between lock holders does not move or
// access the engine objects. D and K themselves must be safe to transfer.
unsafe impl<D: Send, K: Send> Send for BoundProject<D, K> {}

enum ProjectEvent<'t, D, K> { Bind(ProjectPlan<'t, D, K>), Remove(ProjectId) }

/// One owner for the simulation data and every native registration. The
/// external lifecycle is bracketed by insert, remove and destructor hooks.
/// Worker access is serialized and exposes only D, never rendering methods.
pub struct ProjectRegistry<D, K> { state: Lock<Registration<D, K>> }
impl<D, K> Default for ProjectRegistry<D, K> {
    fn default() -> Self { Self::new() }
}
impl<D, K> ProjectRegistry<D, K> {
    pub const fn new() -> Self { Self { state: Lock::new(Registration::Empty) } }
    pub fn with_physics<R>(&self, thread: &Thread, physics: PhysicsId, f: impl FnOnce(&mut D) -> R) -> Option<R> {
        self.state.with(thread, |state| match state {
            Registration::Bound(bound) if bound.physics == physics => Some(f(&mut bound.data)),
            _ => None,
        })
    }
}

/// A bound probe from one membership to the next. It stays in the renderer
/// while bound: a change between Shown and Hidden takes it out and puts it
/// back with the other multipliers (insert hands the object's fields to
/// update_environment_probe); Detached leaves the object with its own.
///
/// # Safety
/// `probe` is a bound, loaded probe of the live registration, on the scene thread.
unsafe fn drive_probe(probe: raw::ObjectEnvironmentProbe, light: ProbeLight, previous: Membership, next: Membership) {
    if previous == next { return; }
    unsafe {
        if previous != Membership::Detached { raw::engine_probe_remove(Some(probe)); }
        let fields = light.at(next);
        probe.diffuse_multiplier().write(fields.diffuse);
        probe.specular_multiplier().write(fields.specular);
        probe.radius().write(fields.radius);
        if next != Membership::Detached { raw::engine_probe_insert(Some(probe)); }
    }
}

/// A loose visual's model made a movable one before it is drawn, as
/// weapon::on_show makes a gun's (set_movable_static 0x243CB0): the
/// renderer treats a model without it as static (0x172C4B).
///
/// # Safety
/// `visual` is a bound visual of the live registration, on the scene thread.
unsafe fn make_movable(visual: raw::RenderVisual) {
    // SAFETY: the visual's model holder and its instance, alive while the
    // project is; the byte set_movable_static writes.
    unsafe {
        if let Some(instance) = visual.model().and_then(|model| model.render_instance().instance()) {
            instance.movable().write(1);
        }
    }
}

/// Writes `transform` into a loose visual's own; whether it changed.
///
/// # Safety
/// As make_movable.
unsafe fn place(visual: raw::RenderVisual, transform: VisualTransform) -> bool {
    // SAFETY: the visual's 16 transform floats, on the scene thread, the
    // one their readers (the visual's insert and remove) run on.
    unsafe {
        let now: [f32; 16] = super::world::floats(visual.transform());
        if now == transform.0 { return false; }
        super::world::write_floats(visual.transform(), transform.0);
        true
    }
}

/// Moves a drawn loose visual's model to its transform, where the native
/// insert adds it (0x285FA0: the scene's scene renderer, the model's
/// reference, the scene's, the transform twice).
///
/// # Safety
/// As make_movable; `scene` is its project's game scene.
unsafe fn update_model(visual: raw::RenderVisual, scene: raw::GameScene) {
    // SAFETY: the visual's model holder; the scene's game, renderer and
    // scene renderer, alive while the project is drawn. update_model copies
    // the matrices; it is asked only of a model the scene holds.
    unsafe {
        let Some(model) = visual.model() else { return };
        let reference = model.render_instance();
        if reference.instance().is_none_or(|instance| super::world::read(instance.in_scene()) == 0) { return; }
        let Some(renderer) = scene.game().and_then(|game| game.renderer()).and_then(|r| r.scene_renderer()) else { return };
        let matrix = visual.transform() as *const f32;
        raw::engine_update_model(Some(renderer), Some(reference), Some(scene.scene_reference()), matrix, matrix);
    }
}

impl<D: Presentation<K>, K: Copy> BoundProject<D, K> {
    fn drive(&mut self, index: usize, event: PresentationEvent) -> RendererAction {
        let binding = self.bindings.get_mut(index).expect("owned binding");
        let (next, action) = binding.membership.transition(event);
        let previous = core::mem::replace(&mut binding.membership, next);
        if let NativeObject::Probe(probe, light) = binding.object {
            // SAFETY: as below; the multipliers are the loaded probe's own
            // fields, which its insert hands to the renderer.
            unsafe { drive_probe(probe, light, previous, next) };
            return RendererAction::Keep;
        }
        // SAFETY: the registration is alive from project insertion until
        // remove/destructor entry. These calls run only on the game's scene
        // thread; workers can access D alone. Native methods call renderer
        // endpoints directly, never the replacement vtable (no lock reentry).
        unsafe {
            match (binding.object, action) {
                (NativeObject::Visual(visual), RendererAction::Add) => raw::engine_visual_insert(Some(visual), Some(self.scene)),
                (NativeObject::Loose(visual), RendererAction::Add) => {
                    make_movable(visual);
                    raw::engine_visual_insert(Some(visual), Some(self.scene));
                }
                (NativeObject::Visual(visual) | NativeObject::Loose(visual), RendererAction::Remove) => {
                    raw::engine_visual_remove(Some(visual), Some(self.scene));
                }
                (NativeObject::Light(light), RendererAction::Add) => raw::engine_light_insert(Some(light)),
                (NativeObject::Light(light), RendererAction::Remove) => raw::engine_light_remove(Some(light)),
                (NativeObject::AmbientLight(light), RendererAction::Add) => raw::engine_ambient_light_insert(Some(light)),
                (NativeObject::AmbientLight(light), RendererAction::Remove) => raw::engine_ambient_light_remove(Some(light)),
                (NativeObject::Particle(particle), RendererAction::Add) => raw::engine_particle_insert(Some(particle)),
                (NativeObject::Particle(particle), RendererAction::Remove) => raw::engine_particle_remove(Some(particle)),
                (NativeObject::Probe(..), _) => unreachable!("a probe is driven by drive_probe"),
                (_, RendererAction::Keep) => {},
            }
        }
        action
    }
    /// A loose visual's transform as the map placed it.
    fn authored(&self, visual: raw::RenderVisual) -> VisualTransform {
        self.loose.iter().find(|l| l.visual.address() == visual.address()).expect("a loose binding has its transform").authored
    }
    /// A binding presented this frame. A loose visual is placed first: an
    /// add draws it there, one drawn already is moved there.
    fn present(&mut self, index: usize) {
        let binding = self.bindings.get(index).expect("owned binding");
        let (object, key) = (binding.object, binding.key);
        let visibility = self.data.visibility(key);
        let NativeObject::Loose(visual) = object else {
            self.drive(index, PresentationEvent::Present(visibility));
            return;
        };
        let transform = self.data.placement(key).unwrap_or_else(|| self.authored(visual));
        // SAFETY: a bound visual of the live registration, on the scene thread.
        let moved = unsafe { place(visual, transform) };
        let action = self.drive(index, PresentationEvent::Present(visibility));
        let drawn = self.bindings.get(index).expect("owned binding").membership == Membership::Shown;
        if moved && drawn && action == RendererAction::Keep {
            // SAFETY: as above; its project's scene.
            unsafe { update_model(visual, self.scene) };
        }
    }
    /// The binding of the native object at `address` (a hook's `this`).
    fn callback(&mut self, address: u32, event: impl FnOnce(Visibility) -> PresentationEvent) {
        let index = self.bindings.iter().position(|b| b.object.address() == address)
            .expect("only registered objects carry our vtables");
        let binding = self.bindings.get(index).expect("owned binding");
        self.drive(index, event(self.data.visibility(binding.key)));
    }
    fn detach(&mut self) {
        for i in 0..self.bindings.len() {
            self.drive(i, PresentationEvent::Remove);
            // SAFETY: the native project has not released any members yet;
            // each original table was checked before installing this owner.
            unsafe {
                match self.bindings.get(i).expect("owned binding").object {
                    NativeObject::Visual(v) => v.vtable().write(raw::image::visual_methods().address()),
                    NativeObject::Loose(v) => {
                        v.vtable().write(raw::image::visual_methods().address());
                        place(v, self.authored(v));
                    }
                    NativeObject::Light(l) => l.vtable().write(raw::image::light_methods().address()),
                    NativeObject::AmbientLight(l) => l.vtable().write(raw::image::ambient_light_methods().address()),
                    NativeObject::Particle(p) => p.vtable().write(raw::image::particle_methods().address()),
                    NativeObject::Probe(p, _) => p.vtable().write(raw::image::probe_methods().address()),
                }
            }
        }
    }
}

impl<D: Presentation<K>, K: Copy> Registration<D, K> {
    fn transition(&mut self, event: ProjectEvent<'_, D, K>) -> Result<(), BindingError> {
        match event {
            ProjectEvent::Bind(plan) => {
                if let Self::Bound(bound) = self {
                    return if bound.project == ProjectId::of(plan.project.native) { Ok(()) } else { Err(BindingError::OtherProjectActive) };
                }
                // Every fallible step is complete. Install only these
                // resolved objects and retain their one owner before returning
                // to the native insertion loops. The game thread is exclusive
                // during insertion and teardown; worker state uses the lock.
                for binding in plan.bindings.iter() {
                    // SAFETY: plan construction proved type, uniqueness and
                    // lifetime; the generated table has the native slot ABIs.
                    unsafe {
                        match binding.object {
                            NativeObject::Visual(v) | NativeObject::Loose(v) => v.vtable().write(plan.methods.visual.address.get()),
                            NativeObject::Light(l) => l.vtable().write(plan.methods.light.address.get()),
                            NativeObject::AmbientLight(l) => l.vtable().write(plan.methods.ambient_light.address.get()),
                            NativeObject::Particle(p) => p.vtable().write(plan.methods.particle.address.get()),
                            NativeObject::Probe(p, _) => p.vtable().write(plan.methods.probe.address.get()),
                        }
                    }
                }
                *self = Self::Bound(BoundProject { project: ProjectId::of(plan.project.native), physics: plan.project.physics,
                                                   scene: plan.project.scene, data: plan.data, bindings: plan.bindings,
                                                   loose: plan.loose });
            }
            ProjectEvent::Remove(project) => {
                if let Self::Bound(bound) = self && bound.project == project { bound.detach(); *self = Self::Empty; }
            }
        }
        Ok(())
    }
}

impl<D: Presentation<K>, K: Copy> ProjectRegistry<D, K> {
    pub fn insert<'t, E: From<BindingError>>(&self, ctx: &'t Context<'_>,
        prepare: impl FnOnce(ProjectView<'t>) -> Result<Option<ProjectPlan<'t, D, K>>, E>) -> Result<(), E> {
        let project = ProjectView::inserting(ctx)?;
        self.state.with(project.thread, |state| {
            if matches!(state, Registration::Bound(b) if b.project == ProjectId::of(project.native)) { return Ok(()); }
            if let Some(plan) = prepare(project)? { state.transition(ProjectEvent::Bind(plan))?; }
            Ok(())
        })
    }
    pub fn removed(&self, ctx: &Context<'_>) {
        // SAFETY: project.removed, after native removal but before storage release.
        if let Some(project) = unsafe { raw::site::project_removed::project(ctx) } {
            self.state.with(ctx.thread(), |s| s.transition(ProjectEvent::Remove(ProjectId::of(project))).expect("removal is total"));
        }
    }
    pub fn destroying(&self, ctx: &Context<'_>) {
        // SAFETY: project.destroy at entry, before any resource member dies.
        if let Some(project) = unsafe { raw::site::project_destroy::project(ctx) } {
            self.state.with(ctx.thread(), |s| s.transition(ProjectEvent::Remove(ProjectId::of(project))).expect("destruction is total"));
        }
    }
    pub fn present(&self, ctx: &Context<'_>) {
        // Called at game_world.after_move: bullet workers have joined; the
        // game's thread owns renderer changes until native drawing begins.
        self.state.with(ctx.thread(), |s| {
            if let Registration::Bound(bound) = s {
                for i in 0..bound.bindings.len() { bound.present(i); }
            }
        });
    }
    pub fn visual_insert(&self, visual: &RenderVisual<'_>, scene: &GameScene<'_>) {
        self.visual(visual, scene, PresentationEvent::Insert);
    }
    pub fn visual_remove(&self, visual: &RenderVisual<'_>, scene: &GameScene<'_>) {
        self.visual(visual, scene, |_| PresentationEvent::Remove);
    }
    fn visual(&self, visual: &RenderVisual<'_>, scene: &GameScene<'_>, event: impl FnOnce(Visibility) -> PresentationEvent) {
        assert!(core::ptr::eq(visual.thread, scene.thread), "one native callback thread");
        self.state.with(visual.thread, |s| {
            let Registration::Bound(bound) = s else { panic!("registered visual without an owner") };
            assert_eq!(bound.scene, scene.native, "visual belongs to its project's scene");
            bound.callback(visual.native.address(), event);
        });
    }
    pub fn light_insert(&self, light: &ObjectLight<'_>) { self.light(light, PresentationEvent::Insert); }
    pub fn light_remove(&self, light: &ObjectLight<'_>) { self.light(light, |_| PresentationEvent::Remove); }
    fn light(&self, light: &ObjectLight<'_>, event: impl FnOnce(Visibility) -> PresentationEvent) {
        self.state.with(light.thread, |s| {
            let Registration::Bound(bound) = s else { panic!("registered light without an owner") };
            bound.callback(light.native.address(), event);
        });
    }
    pub fn ambient_light_insert(&self, light: &ObjectAmbientLight<'_>) { self.ambient_light(light, PresentationEvent::Insert); }
    pub fn ambient_light_remove(&self, light: &ObjectAmbientLight<'_>) { self.ambient_light(light, |_| PresentationEvent::Remove); }
    fn ambient_light(&self, light: &ObjectAmbientLight<'_>, event: impl FnOnce(Visibility) -> PresentationEvent) {
        self.state.with(light.thread, |s| {
            let Registration::Bound(bound) = s else { panic!("registered ambient light without an owner") };
            bound.callback(light.native.address(), event);
        });
    }
    pub fn particle_insert(&self, particle: &ObjectParticleVisual<'_>) { self.particle(particle, PresentationEvent::Insert); }
    pub fn particle_remove(&self, particle: &ObjectParticleVisual<'_>) { self.particle(particle, |_| PresentationEvent::Remove); }
    fn particle(&self, particle: &ObjectParticleVisual<'_>, event: impl FnOnce(Visibility) -> PresentationEvent) {
        self.state.with(particle.thread, |s| {
            let Registration::Bound(bound) = s else { panic!("registered particle without an owner") };
            bound.callback(particle.native.address(), event);
        });
    }
    pub fn probe_insert(&self, probe: &ObjectEnvironmentProbe<'_>) { self.probe(probe, PresentationEvent::Insert); }
    pub fn probe_remove(&self, probe: &ObjectEnvironmentProbe<'_>) { self.probe(probe, |_| PresentationEvent::Remove); }
    fn probe(&self, probe: &ObjectEnvironmentProbe<'_>, event: impl FnOnce(Visibility) -> PresentationEvent) {
        self.state.with(probe.thread, |s| {
            let Registration::Bound(bound) = s else { panic!("registered probe without an owner") };
            bound.callback(probe.native.address(), event);
        });
    }
}

#[cfg(test)]
impl ProjectId {
    pub fn for_test(address: u32) -> Self { Self(NonZeroU32::new(address).expect("a test id is not 0")) }
}
