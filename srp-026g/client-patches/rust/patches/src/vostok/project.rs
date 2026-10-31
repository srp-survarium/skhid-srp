//! A project's registered presentation. This owner alone installs/restores
//! object vtables and changes renderer membership. No native handle escapes
//! into patch data; project removal and destruction end the registration.
use core::{marker::PhantomData, num::NonZeroU32};
use crate::raw;
use super::{Context, Handle, Lock, Thread, site};
use super::collection::BoundedVec;
use super::config::{ConfigError, ConfigValue};
use super::geometry::WorldPoint;
use super::presentation::{Membership, PresentationEvent, RendererAction, Visibility};

const MAX_BINDINGS: usize = 1024;

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
}

pub struct MethodTables {
    visual: NativeVtable<raw::RenderVisualMethods>,
    light: NativeVtable<raw::ObjectLightMethods>,
}
impl MethodTables {
    pub fn new(visual: NativeVtable<raw::RenderVisualMethods>, light: NativeVtable<raw::ObjectLightMethods>) -> Self {
        Self { visual, light }
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
native_view!(GameScene);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsId(NonZeroU32);
impl PhysicsId {
    pub(super) fn of(physics: raw::BulletPhysicsWorld) -> Self {
        Self(NonZeroU32::new(physics.address()).expect("non-null native handle"))
    }
}

/// Index in the authored render-visual array, distinct from a lamp id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualIndex(u32);
impl From<u32> for VisualIndex { fn from(value: u32) -> Self { Self(value) } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProjectId(NonZeroU32);
impl ProjectId {
    fn of(project: raw::SimpleGameProject) -> Self {
        Self(NonZeroU32::new(project.address()).expect("non-null native handle"))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BindingError {
    MissingProject, MissingScene, MissingPhysics, InvalidObjects, InvalidVisual,
    UnknownVisualType, UnknownLight, AmbiguousLight, DuplicateObject, Capacity,
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
    /// At the project.insert detour, before the native insert_static call.
    /// The entry supplies the physics world in its first argument; the
    /// project's camera is not a physics world.
    fn inserting(ctx: &'t Context<'_, site::ProjectInsert>) -> Result<Self, BindingError> {
        use raw::site::project_insert as at;
        let native = at::project(ctx).ok_or(BindingError::MissingProject)?;
        let frame = at::frame(ctx).ok_or(BindingError::MissingPhysics)?;
        Ok(Self { native, frame, thread: ctx.thread() })
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
        Ok(ProjectPlan { project, data, methods, bindings: BoundedVec::new() })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeObject { Visual(raw::RenderVisual), Light(raw::ObjectLight) }

struct Binding<K> { object: NativeObject, key: K, membership: Membership }

/// An uninstalled, fully owned proposal. Dropping it changes nothing in
/// the game. Every add resolves and type-checks an object; commit is infallible.
pub struct ProjectPlan<'t, D, K> {
    project: PreparedProject<'t>,
    data: D,
    methods: MethodTables,
    bindings: BoundedVec<Binding<K>, MAX_BINDINGS>,
}
impl<'t, D, K> ProjectPlan<'t, D, K> {
    pub fn with_data<T>(self, data: T) -> ProjectPlan<'t, T, K> {
        ProjectPlan { project: self.project, data, methods: self.methods, bindings: self.bindings }
    }
    fn add(&mut self, object: NativeObject, key: K) -> Result<(), BindingError> {
        if self.bindings.iter().any(|binding| binding.object == object) { return Err(BindingError::DuplicateObject); }
        self.bindings.push(Binding { object, key, membership: Membership::Detached }).map_err(|_| BindingError::Capacity)?;
        Ok(())
    }
    pub fn visual(&mut self, index: VisualIndex, key: K) -> Result<(), BindingError> {
        // SAFETY: the plan borrows the insertion hook. Count bounds the
        // native contiguous visual array; only its native type is accepted.
        let visual = unsafe {
            if index.0 >= self.project.native.visual_count().read() { return Err(BindingError::InvalidVisual); }
            let visual = self.project.native.visuals().ok_or(BindingError::InvalidVisual)?.at(index.0 as usize);
            if visual.vtable().read() != raw::image::visual_methods().address() { return Err(BindingError::UnknownVisualType); }
            visual
        };
        self.add(NativeObject::Visual(visual), key)
    }
    pub fn light_at(&mut self, position: WorldPoint, key: K) -> Result<(), BindingError> {
        // SAFETY: native project vector and light objects live for this
        // insertion. Only an object with the light vtable is read as a light.
        let light = unsafe {
            let first = self.project.native.objects().ok_or(BindingError::UnknownLight)?;
            let end = self.project.native.objects_end().ok_or(BindingError::InvalidObjects)?;
            let count = end.index_from(first).ok_or(BindingError::InvalidObjects)?;
            let mut found = None;
            for i in 0..count {
                let object = first.at(i).object().ok_or(BindingError::InvalidObjects)?;
                if object.vtable().read() != raw::image::light_methods().address() { continue; }
                let light = object.light();
                let xyz = super::world::floats(light.position());
                if xyz == position.coordinates() && found.replace(light).is_some() { return Err(BindingError::AmbiguousLight); }
            }
            found.ok_or(BindingError::UnknownLight)?
        };
        self.add(NativeObject::Light(light), key)
    }
    pub fn data_mut(&mut self) -> &mut D { &mut self.data }
}

/// The patch supplies only the meaning of a binding's key. The registry
/// remains the sole owner of renderer membership and native object access.
pub trait Presentation<K> { fn visibility(&self, key: K) -> Visibility; }

enum Registration<D, K> { Empty, Bound(BoundProject<D, K>) }
struct BoundProject<D, K> {
    project: ProjectId,
    physics: PhysicsId,
    scene: raw::GameScene,
    data: D,
    bindings: BoundedVec<Binding<K>, MAX_BINDINGS>,
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

impl<D: Presentation<K>, K: Copy> BoundProject<D, K> {
    fn drive(&mut self, index: usize, event: PresentationEvent) {
        let binding = self.bindings.get_mut(index).expect("owned binding");
        let (next, action) = binding.membership.transition(event);
        binding.membership = next;
        // SAFETY: the registration is alive from project insertion until
        // remove/destructor entry. These calls run only on the game's scene
        // thread; workers can access D alone. Native methods call renderer
        // endpoints directly, never the replacement vtable (no lock reentry).
        unsafe {
            match (binding.object, action) {
                (NativeObject::Visual(visual), RendererAction::Add) => raw::engine_visual_insert(Some(visual), Some(self.scene)),
                (NativeObject::Visual(visual), RendererAction::Remove) => raw::engine_visual_remove(Some(visual), Some(self.scene)),
                (NativeObject::Light(light), RendererAction::Add) => raw::engine_light_insert(Some(light)),
                (NativeObject::Light(light), RendererAction::Remove) => raw::engine_light_remove(Some(light)),
                (_, RendererAction::Keep) => {},
            }
        }
    }
    fn callback(&mut self, object: NativeObject, event: impl FnOnce(Visibility) -> PresentationEvent) {
        let index = self.bindings.iter().position(|b| b.object == object).expect("only registered objects carry our vtables");
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
                    NativeObject::Light(l) => l.vtable().write(raw::image::light_methods().address()),
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
                            NativeObject::Visual(v) => v.vtable().write(plan.methods.visual.address.get()),
                            NativeObject::Light(l) => l.vtable().write(plan.methods.light.address.get()),
                        }
                    }
                }
                *self = Self::Bound(BoundProject { project: ProjectId::of(plan.project.native), physics: plan.project.physics,
                                                   scene: plan.project.scene, data: plan.data, bindings: plan.bindings });
            }
            ProjectEvent::Remove(project) => {
                if let Self::Bound(bound) = self && bound.project == project { bound.detach(); *self = Self::Empty; }
            }
        }
        Ok(())
    }
}

impl<D: Presentation<K>, K: Copy> ProjectRegistry<D, K> {
    pub fn insert<'t, E: From<BindingError>>(&self, ctx: &'t Context<'_, site::ProjectInsert>,
        prepare: impl FnOnce(ProjectView<'t>) -> Result<Option<ProjectPlan<'t, D, K>>, E>) -> Result<(), E> {
        let project = ProjectView::inserting(ctx)?;
        self.state.with(project.thread, |state| {
            if matches!(state, Registration::Bound(b) if b.project == ProjectId::of(project.native)) { return Ok(()); }
            if let Some(plan) = prepare(project)? { state.transition(ProjectEvent::Bind(plan))?; }
            Ok(())
        })
    }
    /// At project.removed, after native removal but before storage release.
    pub fn removed(&self, ctx: &Context<'_, site::ProjectRemoved>) {
        if let Some(project) = raw::site::project_removed::project(ctx) {
            self.state.with(ctx.thread(), |s| s.transition(ProjectEvent::Remove(ProjectId::of(project))).expect("removal is total"));
        }
    }
    /// At project.destroy's entry, before any resource member dies.
    pub fn destroying(&self, ctx: &Context<'_, site::ProjectDestroy>) {
        if let Some(project) = raw::site::project_destroy::project(ctx) {
            self.state.with(ctx.thread(), |s| s.transition(ProjectEvent::Remove(ProjectId::of(project))).expect("destruction is total"));
        }
    }
    pub fn present(&self, ctx: &Context<'_, site::GameWorldAfterMove>) {

        // Called at game_world.after_move: bullet workers have joined; the
        // game's thread owns renderer changes until native drawing begins.
        self.state.with(ctx.thread(), |s| {
            if let Registration::Bound(bound) = s {
                for i in 0..bound.bindings.len() {
                    let visibility = bound.data.visibility(bound.bindings.get(i).expect("owned binding").key);
                    bound.drive(i, PresentationEvent::Present(visibility));
                }
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
            bound.callback(NativeObject::Visual(visual.native), event);
        });
    }
    pub fn light_insert(&self, light: &ObjectLight<'_>) { self.light(light, PresentationEvent::Insert); }
    pub fn light_remove(&self, light: &ObjectLight<'_>) { self.light(light, |_| PresentationEvent::Remove); }
    fn light(&self, light: &ObjectLight<'_>, event: impl FnOnce(Visibility) -> PresentationEvent) {
        self.state.with(light.thread, |s| {
            let Registration::Bound(bound) = s else { panic!("registered light without an owner") };
            bound.callback(NativeObject::Light(light.native), event);
        });
    }
}
