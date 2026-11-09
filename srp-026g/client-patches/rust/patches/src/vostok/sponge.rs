//! Patch 59, the Sponge: its native side. The rules are the sponge crate's
//! (`machine`, `record`, `spots`); this module is the object in the game's
//! memory and what the game does for it, written again from the C version
//! (sponge_a_026g.c, 0.28d0's artefact_sponge_core 0x5136E0..0x515270 on
//! 0.26g's interfaces): its slots, its body, its zone, its splash, its cook,
//! its spots. Every game fact is a row of the raw tables (fields_026g.tsv
//! `skhid::sponge` and the kinds it names, engine_026g.tsv); the proofs are
//! client_patch.md section 3.52 and docs/notes/sponge.md.
//!
//! The object is Rattle's (artefact<artefact_rattle_core>, 360h bytes, built
//! by its constructor 0x25FA10: a bare artefact_base and its drawable at
//! +340h), then the sponge's fields (`skhid::sponge`, 6C8h bytes). Its
//! vtables are the generated copies of patch `sponge`'s TOML (Rattle's with
//! ten slots ours, its drawable's, grenade_core's hit inflictor's).
//!
//! A sponge is destroyed when the game drops the last hold on it: the
//! anomaly that spawned it keeps it for the whole match, so that is at the
//! match's end, when the world goes, in whatever order its parts go. So the
//! destructor touches nothing the world owns (as 0.26g's own destructors of
//! the same things): the physics world, the holder's pool and the scene's
//! list are only forgotten; what is the sponge's own goes.

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::num::NonZeroU32;
use core::sync::atomic::{AtomicU32, Ordering};

use ::sponge::machine::{self, Consumed, Effect, Event, Hold, Physics, Rules, State, Step, Thrower};
use ::sponge::record::{self, Field, Motion, Saved};
use ::sponge::spots::{self, Allowed, Choice, Core, Picks, Row, Slot, SpotTable};
use ::sponge::{GameTime, Place, TICK};

use super::world::read;
use super::{Context, GameCell, Handle, Reg, Thread};
use crate::raw;

type Matrix = [f32; 16];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
/// The game's artefact states (+280h): artefact_base's, and the two the
/// sponge shows it (armed: in the hand or the air; its cloud).
const ST_NONE: u32 = 0;
const ST_PASSIVE: u32 = 3;
const ST_ACTIVE: u32 = 4;
const ST_DEAD: u32 = 5;
const ST_ARMED: u32 = record::STATE_ARMED as u32;
const ST_ZONE: u32 = record::STATE_EXPLODED as u32;
/// The inventory's grenade slot of none (m_grenade_slot 17h; its 23 slots).
const NO_SLOT: u32 = 0x17;
const SLOTS: u32 = 0x17;
/// data/type of the Sponge (the cook's switch reads it before `add eax,-3`).
const TYPE_SPONGE: u32 = 12;
/// The zone's own "no hit yet" (+1CCh, as 0x31B9A0 resets it).
const ZONE_NO_HIT: u32 = u32::MAX;
/// Sound and effect zones moved with the explosion.
const MAX_ZONES: u32 = 4;
/// The resource classes of the sponge's query.
const CLASS_MODEL: u32 = 0xF;
const CLASS_PARTICLE: u32 = 0x3D;
const CLASS_DAMAGE_ZONE: u32 = 0x75;
/// The modifiers the splash and the throw take (0x30A3A0, 0x30A3D3, 0x30A2CC).
const M_DAMAGE: u32 = 11;
const M_RADIUS: u32 = 12;
const M_THROW: u32 = 27;
/// 0.69d0's sensor: its way swept with spheres of the body's radius.
const SENSOR_RADIUS: f32 = 0.1;
const SENSOR_STEPS: u32 = 16;
/// The physics world's group and mask of a thrown body (grenade_core's 0x303C10).
const BODY_GROUP: u32 = 0x818;
const BODY_MASK: u32 = 8;

macro_rules! at {
    ($h:expr) => {
        $h.address() as usize as *mut c_void
    };
}

/// A sponge (an artefact whose vtable is the Sponge's copy).
#[derive(Clone, Copy)]
pub struct Sponge<'t> {
    this: raw::Sponge,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for Sponge<'t> {
    type Raw = raw::Artefact;
    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::Artefact>) -> Option<Self> {
        // SAFETY: the generated slot entries of the Sponge's vtable alone make
        // one, so the artefact is a sponge (its object is skhid::sponge's).
        native.map(|a| Sponge { this: unsafe { a.sponge() }, thread })
    }
}

/// A sponge's drawable (+340h), as the scene's loop calls its draw.
pub struct SpongeDrawable<'t> {
    sponge: Sponge<'t>,
    drawable: raw::SceneDrawable,
}

impl<'t> Handle<'t> for SpongeDrawable<'t> {
    type Raw = raw::SceneDrawable;
    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::SceneDrawable>) -> Option<Self> {
        let drawable = native?;
        // SAFETY: only the Sponge's drawable vtable's entry makes one: the
        // drawable at a sponge's +340h.
        let sponge = Sponge { this: unsafe { drawable.artefact().sponge() }, thread };
        Some(SpongeDrawable { sponge, drawable })
    }
}

/// A sponge's hit inflictor (+68Ch), as a hit asks it.
pub struct SpongeInflictor {
    inflictor: raw::HitInflictor,
}

impl<'t> Handle<'t> for SpongeInflictor {
    type Raw = raw::HitInflictor;
    unsafe fn from_raw(_: &'t Thread, native: Option<raw::HitInflictor>) -> Option<Self> {
        native.map(|inflictor| SpongeInflictor { inflictor })
    }
}

impl SpongeInflictor {
    pub(in crate::vostok) fn raw(&self) -> raw::HitInflictor {
        self.inflictor
    }

    /// Where it is: the address of its position (as 0.26g's getter 0x346480,
    /// `lea eax,[ecx+10h]`, the C version's slot).
    pub fn position(&self) -> u32 {
        // SAFETY: a live sponge's inflictor (its slot entry's ECX).
        (unsafe { self.inflictor.position() }) as usize as u32
    }
}

macro_rules! handle {
    ($name:ident, $raw:ty, $what:literal) => {
        #[doc = $what]
        pub struct $name($raw);
        impl<'t> Handle<'t> for $name {
            type Raw = $raw;
            unsafe fn from_raw(_: &'t Thread, native: Option<$raw>) -> Option<Self> {
                native.map($name)
            }
        }
        impl $name {
            pub(in crate::vostok) fn raw(&self) -> $raw {
                self.0
            }
        }
    };
}
handle!(SpongeWriter, raw::BufferWriter, "The writer a world is saved through (a sponge's serialize slot).");
handle!(SpongeReader, raw::BufferReader, "The reader a world is loaded through (its deserialize slot).");
/// A runtime callback's sponge (the boost function's object): a binding the
/// generated entry makes; none for a null object.
pub struct SpongeCallee<'t>(Option<Sponge<'t>>);

impl<'t> SpongeCallee<'t> {
    /// # Safety
    /// `native` is the object our boost function names: a live sponge.
    pub unsafe fn bind(thread: &'t Thread, native: Option<raw::Artefact>) -> Self {
        SpongeCallee(unsafe { Sponge::from_raw(thread, native) })
    }

    pub fn sponge(&self) -> Option<&Sponge<'t>> {
        self.0.as_ref()
    }
}

/// A sponge's query's results (its loaded callback), as a binding.
pub struct SpongeLoad(Option<raw::QueriesResult>);

impl SpongeLoad {
    /// # Safety
    /// `native` is the query's live results, through the callback.
    pub unsafe fn bind(_: &Thread, native: Option<raw::QueriesResult>) -> Self {
        SpongeLoad(native)
    }
}

// ---- the game's memory ---------------------------------------------------

fn read_matrix(at: *mut f32) -> Matrix {
    // SAFETY: callers name a 16-f32 field of a live object.
    unsafe { core::ptr::read_unaligned(at.cast::<Matrix>()) }
}

fn write_matrix(at: *mut f32, m: &Matrix) {
    // SAFETY: as read_matrix.
    unsafe { core::ptr::write_unaligned(at.cast::<Matrix>(), *m) }
}

fn position(m: &Matrix) -> [f32; 3] {
    [m[12], m[13], m[14]]
}

fn allocator() -> Option<*mut c_void> {
    // SAFETY: the image's allocator slot, set before any match.
    let allocator = unsafe { raw::image::game_allocator().allocator() }?;
    Some(at!(allocator))
}

fn acquire(references: *mut u32) {
    // SAFETY: a live resource's intrusive count, atomic as the game's.
    unsafe { AtomicU32::from_ptr(references).fetch_add(1, Ordering::AcqRel) };
}

/// One reference less; true when it was the last (the caller releases it).
fn drop_reference(references: *mut u32) -> bool {
    // SAFETY: as acquire.
    unsafe { AtomicU32::from_ptr(references).fetch_sub(1, Ordering::AcqRel) == 1 }
}

fn release_zone(zone: raw::DamageZone) {
    // SAFETY: a zone resource we hold one reference of, given back once.
    unsafe {
        if drop_reference(zone.references()) {
            raw::engine_release_zone(Some(zone), zone.references());
        }
    }
}

fn release_config(config: raw::ConfigResource) {
    // SAFETY: as release_zone, a config resource.
    unsafe {
        if drop_reference(config.references()) {
            raw::engine_release_config(Some(config), config.references());
        }
    }
}

/// A config's child `key` (0xF6B10), if it has one.
fn child(table: raw::EngineCfg, key: &core::ffi::CStr) -> Option<raw::EngineCfg> {
    // SAFETY: a live config value; operator[] and value_exists read it.
    unsafe {
        if raw::engine_cfg_has(Some(table), key.as_ptr().cast()) == 0 {
            return None;
        }
        raw::engine_cfg_child(Some(table), key.as_ptr().cast())
    }
}

/// A config's child `key` the loaders read whether or not it exists (the
/// game's operator[] answers a missing key with its empty value).
fn must(table: raw::EngineCfg, key: &core::ffi::CStr) -> raw::EngineCfg {
    // SAFETY: as child; operator[] always returns a value.
    unsafe { raw::engine_cfg_child(Some(table), key.as_ptr().cast()) }.expect("operator[] answers every key")
}

const CFG_INT: u16 = 1;
const CFG_FLOAT: u16 = 2;
const CFG_NAMED: u16 = 3;
const CFG_INDEXED: u16 = 4;
const CFG_TEXT: u16 = 5;

fn kind_of(value: raw::EngineCfg) -> u16 {
    // SAFETY: a live config value.
    unsafe { read(value.node_type()) }
}

fn count_of(value: raw::EngineCfg) -> u16 {
    // SAFETY: as kind_of.
    unsafe { read(value.count()) }
}

fn bits_of(value: raw::EngineCfg) -> u32 {
    // SAFETY: as kind_of.
    unsafe { read(value.data()) }
}

fn item(table: raw::EngineCfg, index: u16) -> Option<raw::EngineCfg> {
    // SAFETY: a table's children, `index` below its count.
    (index < count_of(table)).then(|| unsafe { table.children() }).flatten().map(|first| unsafe { first.at(index.into()) })
}

/// A number as the loaders read one: a float, or an int converted (0.28's 0x543930, 0.26g's 0x309B9D).
fn number(value: raw::EngineCfg) -> f32 {
    let bits = bits_of(value);
    if kind_of(value) == CFG_FLOAT { f32::from_bits(bits) } else { bits as i32 as f32 }
}

/// 0.28's `imul eax,[eax],3E8h`: seconds in ms (a float's taken whole).
fn whole_ms(value: raw::EngineCfg) -> u32 {
    let bits = bits_of(value);
    if kind_of(value) == CFG_FLOAT { (f32::from_bits(bits) as i32 as u32).wrapping_mul(1000) } else { bits.wrapping_mul(1000) }
}

fn text(value: raw::EngineCfg) -> *const u8 {
    // SAFETY: a text node's string (relocated by the loader).
    unsafe { value.text() }
}

/// A named table's `key`, if an int (dict_id, count, index, kind).
fn int_key(table: raw::EngineCfg, key: &core::ffi::CStr) -> Option<raw::EngineCfg> {
    if kind_of(table) != CFG_NAMED {
        return None;
    }
    child(table, key).filter(|v| kind_of(*v) == CFG_INT)
}

/// A named table's `key`, if a text: its bytes (no zero).
fn text_key<'a>(table: raw::EngineCfg, key: &core::ffi::CStr) -> Option<&'a [u8]> {
    if kind_of(table) != CFG_NAMED {
        return None;
    }
    let value = child(table, key).filter(|v| kind_of(*v) == CFG_TEXT)?;
    // SAFETY: the loader's zero-terminated string, alive with its config
    // (the dictionary keeps every map's mode config loaded).
    unsafe {
        let at = text(value);
        if at.is_null() {
            return None;
        }
        let mut length = 0;
        while at.add(length).read_volatile() != 0 { // volatile: no strlen (the arena has none)
            length += 1;
        }
        Some(core::slice::from_raw_parts(at, length))
    }
}

// ---- the sponge's own fields --------------------------------------------

impl<'t> Sponge<'t> {
    pub(in crate::vostok) fn raw(&self) -> raw::Artefact {
        self.artefact()
    }
}

impl<'t> Sponge<'t> {
    fn artefact(&self) -> raw::Artefact {
        // SAFETY: the object is Rattle's at +0.
        unsafe { self.this.artefact() }
    }

    fn item(&self) -> raw::Item {
        // SAFETY: every artefact's inventory item at +0.
        unsafe { self.artefact().item() }
    }

    pub fn id(&self) -> u32 {
        self.this.address()
    }

    pub fn state(&self) -> State {
        // SAFETY: +41Ch holds a machine::State (repr C), written only here;
        // zeroed by the constructor (State::Held), 4-aligned.
        unsafe { core::ptr::read(self.this.state().cast::<State>()) }
    }

    fn set_state(&self, state: State) {
        // SAFETY: as state.
        unsafe { core::ptr::write(self.this.state().cast::<State>(), state) }
    }

    fn game_state(&self) -> u32 {
        // SAFETY: artefact_base's state, of a live artefact.
        unsafe { read(self.artefact().state()) }
    }

    fn set_game_state(&self, state: u32) {
        // SAFETY: as game_state.
        unsafe { *self.artefact().state() = state }
    }

    fn inventory(&self) -> Option<raw::Inventory> {
        // SAFETY: the item's inventory, 0 out of one.
        unsafe { self.item().inventory() }
    }

    fn slot(&self) -> u32 {
        // SAFETY: the item's slot, 17h out of one.
        unsafe { read(self.item().inventory_slot()) }
    }

    fn world(&self) -> Option<raw::CoreWorld> {
        // SAFETY: the world it is registered in (set_world), 0 before.
        unsafe { self.item().world() }
    }

    fn physics_world(&self) -> Option<raw::BulletPhysicsWorld> {
        // SAFETY: the registered world's physics world.
        self.world().and_then(|w| unsafe { w.physics() })
    }

    /// Its holder: the player whose inventory it is in.
    fn holder(&self) -> Option<raw::Player> {
        // SAFETY: an inventory's holder interface is a base_player's (+108h).
        self.inventory().and_then(|i| unsafe { i.holder() }).map(|h| unsafe { h.player() })
    }

    fn body(&self) -> Option<raw::BtDynamicRigidBody> {
        // SAFETY: its body field, ours.
        unsafe { self.this.body() }
    }

    fn zone(&self) -> Option<raw::DamageZone> {
        // SAFETY: its zone field, ours (a resource it holds).
        unsafe { self.this.zone() }
    }

    fn transform(&self) -> Matrix {
        // SAFETY: its m_transform field.
        read_matrix(unsafe { self.this.transform() })
    }

    fn set_transform(&self, m: &Matrix) {
        // SAFETY: as transform.
        write_matrix(unsafe { self.this.transform() }, m)
    }

    fn inflictor(&self) -> raw::HitInflictor {
        // SAFETY: its inflictor, inside it.
        unsafe { self.this.inflictor() }
    }

    /// The inflictor's position: where m_transform is (0.28's slot 2 read the
    /// transform at each call; 0.26g's getter reads the field).
    fn place(&self, m: &Matrix) {
        // SAFETY: the inflictor's three floats.
        unsafe { core::ptr::write_unaligned(self.inflictor().position().cast::<[f32; 3]>(), position(m)) }
    }

    fn explode_on_contact(&self) -> bool {
        // SAFETY: its config's byte.
        unsafe { read(self.this.explode_on_contact()) != 0 }
    }

    fn rules(&self) -> Rules {
        // SAFETY: its config's numbers; the world's grid origin.
        unsafe {
            Rules {
                ttl_ms: read(self.this.ttl()),
                safe_time_ms: read(self.this.safe_time()),
                duration_ms: read(self.this.duration()),
                explode_on_contact: self.explode_on_contact(),
                origin: GameTime(self.world().map_or(0, |w| read(w.grid_origin()))),
            }
        }
    }

    fn physics_id(world: raw::BulletPhysicsWorld) -> Physics {
        Physics(NonZeroU32::new(world.address()).expect("a handle is never 0"))
    }

    /// The physics world an armed sponge's body is in, from its state's id:
    /// the world it is registered in (the only one a body goes into).
    fn physics_of(&self, physics: Physics) -> Option<raw::BulletPhysicsWorld> {
        self.physics_world().filter(|w| w.address() == physics.0.get())
    }

    /// One event that only changes the state (and the game state 6 at the pull).
    fn step(&self, event: Event) -> Step {
        let step = machine::transition(&self.rules(), self.state(), event);
        self.set_state(step.state);
        if step.effect == Effect::Arm {
            self.set_game_state(ST_ARMED);
        }
        step
    }
}

// ---- players ---------------------------------------------------------------

/// The player of a place, only while the place is in the match (core
/// active_player 0x2D0AB0's test: below the places' end and its bit in
/// +18FD8h, cleared at the tick of a leave by 0x2D2040). A place is never
/// given to another player in a match, so the place names the thrower; a
/// thrower who left gets no credit, and one back in his place does.
/// The current kit's player of a place in the match (core_active_player's
/// walk, no check of whom it names).
fn place_kit_player(world: raw::CoreWorld, place: u8) -> Option<raw::Player> {
    // SAFETY: the live world's place table and its players.
    unsafe {
        let (first, end) = (world.places()?, world.places_end()?);
        let count = (end.address().checked_sub(first.address())? as usize) / raw::CorePlace::SIZE;
        if usize::from(place) >= count || read(world.active_places()) >> place & 1 == 0 {
            return None;
        }
        let entry = first.at(place.into());
        match read(entry.kit()) {
            0 => entry.kit0(),
            1 => entry.kit1(),
            2 => entry.kit2(),
            _ => None,
        }
    }
}

fn place_player(world: raw::CoreWorld, place: u8) -> Option<raw::Player> {
    // SAFETY: the live player's place byte.
    place_kit_player(world, place).filter(|p| unsafe { read(p.place()) } == place)
}

/// The holder's modifier `index` on `b` (0x2D5AE0, weight 1).
fn modified(player: raw::Player, index: u32, b: f32) -> f32 {
    // SAFETY: the live player's profile and current kit's modifiers.
    unsafe {
        let kit = usize::from(read(player.kit()));
        let Some(profile) = player.profile() else { return b };
        if kit >= 3 {
            return b;
        }
        raw::engine_apply_modifier(Some(profile.kits().at(kit).modifiers()), index, b, 1.0)
    }
}

impl Sponge<'_> {
    fn owner_player(&self) -> Option<raw::Player> {
        let place = self.state().thrower().place()?;
        place_player(self.world()?, place.get())
    }

    /// His game object: the killer a hit names.
    fn owner_object(&self) -> Option<raw::GameObject> {
        // SAFETY: the live player's game object, inside it.
        self.owner_player().map(|p| unsafe { p.game_object() })
    }

    fn holder_thrower(&self) -> Thrower {
        // SAFETY: the live holder's place byte.
        match self.holder().and_then(|p| Place::new(unsafe { read(p.place()) })) {
            Some(place) => Thrower::Place(place),
            None => Thrower::Nobody,
        }
    }
}

// ---- the rigid body --------------------------------------------------------

/// The body description 0.28's 0x514400 and 0.26g's grenade 0x303890 both
/// give: {0.3, 0, 0, 0, 0.5, 0, 0.8, 1, 0, 0.005, 0.01 x 3, 1 x 6, 1}.
#[repr(C)]
struct BodyDescription {
    a: [f32; 4],
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    flag: u8,
    pad: [u8; 3],
    f: f32,
    g: [f32; 3],
    h: [f32; 6],
    last: u8,
    pad2: [u8; 3],
}
const _: () = assert!(core::mem::size_of::<BodyDescription>() == 0x50);

impl Sponge<'_> {
    /// A sphere of 0.05 (type 3, dims 0.05, 0.1, 0.1, scale 1) of its game
    /// material (the shape's contacts' sounds and decals: 45 "grenade", the
    /// RGD-5's and 0.69d0's Sponge's), damping 0.01 and 0.03; 0.28 flags the
    /// shape (0.26g's resource flags +FCh) with 2.
    fn make_body(&self) {
        let dims = [0.05f32, 0.1, 0.1];
        let scale = [1.0f32; 3];
        let desc = BodyDescription {
            a: [0.3, 0.0, 0.0, 0.0],
            b: 0.5,
            c: 0.0,
            d: 0.8,
            e: 1.0,
            flag: 0,
            pad: [0; 3],
            f: 0.005,
            g: [0.01; 3],
            h: [1.0; 6],
            last: 1,
            pad2: [0; 3],
        };
        // SAFETY: the game's shape and body making, as grenade_core's; the
        // body is ours from here (destroyed by destroy_body).
        unsafe {
            let material = u32::from(read(self.this.material()));
            let shape = raw::engine_create_collision_shape(3, dims.as_ptr(), scale.as_ptr(), material);
            let body = raw::engine_create_rigid_body((&desc as *const BodyDescription).cast(), shape);
            if let Some(body) = body {
                if let Some(shape) = body.shape() {
                    *shape.flags() |= 2;
                }
                if let Some(bullet) = body.bullet() {
                    *bullet.linear_damping() = 0.01;
                    *bullet.angular_damping() = 0.03;
                }
            }
            self.this.set_body(body);
        }
    }

    /// grenade_core's destructor 0x303770's way: the shape, the body's
    /// destructor, its memory to the physics allocator.
    fn destroy_body(&self, body: raw::BtDynamicRigidBody) {
        // SAFETY: our body, not in any physics world we still know; its
        // memory the physics allocator's, freed once.
        unsafe {
            if let Some(shape) = body.shape() {
                raw::engine_destroy_shape(at!(shape));
            }
            let memory = raw::engine_rt_cast_to_void(at!(body));
            raw::engine_body_destroy(Some(body), 0);
            let allocator = raw::image::physics_allocator().allocator();
            raw::engine_pthreads_free(allocator, memory, raw::image::DESTROY_BODY_FUNCTION, raw::image::DESTROY_BODY_FILE, 0x4E);
        }
        // SAFETY: the field ours.
        unsafe { self.this.set_body(None) };
    }

    fn physics_update(&self, world: raw::BulletPhysicsWorld, body: raw::BtDynamicRigidBody) {
        // SAFETY: a live body in a live physics world.
        unsafe {
            let object = raw::engine_body_object(Some(body));
            raw::engine_physics_update(Some(world), object);
        }
    }

    fn contact_fn(&self) -> raw::BoostFunction {
        // SAFETY: its own field.
        unsafe { self.this.contact_fn() }
    }

    /// 0.28's 0x5146E0: into the world's physics at `transform` with
    /// `linear`, no spin; the item count 0, the inventory dirty; its contacts
    /// called back (as grenade_core::throw_grenade 0x303C10, vt+70h) while
    /// it explodes on contact. The physics world it is in now.
    fn launch(&self, transform: &Matrix, linear: [f32; 3]) -> Option<Physics> {
        let world = self.physics_world()?;
        let body = self.body()?;
        self.set_transform(transform); // where its way starts (the sensor's sweep)
        // SAFETY: our body into the registered world's physics world.
        unsafe {
            raw::engine_body_set_transform(Some(body), transform.as_ptr());
            self.place(transform);
            raw::engine_physics_add_body(Some(world), Some(body), BODY_GROUP, BODY_MASK);
            if self.explode_on_contact() {
                raw::engine_physics_subscribe_contact(Some(world), Some(body), Some(self.contact_fn()));
            }
            raw::engine_body_set_linear_velocity(linear.as_ptr(), at!(body));
            raw::engine_body_set_angular_velocity([0.0f32; 3].as_ptr(), at!(body));
            if read(self.item().count()) != 0 {
                *self.item().count() = 0;
                if let Some(inventory) = self.inventory() {
                    *inventory.weight_dirty() = 1;
                }
            }
        }
        Some(Sponge::physics_id(world))
    }

    /// Out of the physics world, its contacts' callback out first (0x303D50).
    fn leave_physics(&self, physics: Physics) {
        let (Some(world), Some(body)) = (self.physics_of(physics), self.body()) else { return };
        // SAFETY: our body, in that world.
        unsafe {
            if self.explode_on_contact() {
                raw::engine_physics_unsubscribe_contact(Some(world), Some(body), Some(self.contact_fn()));
            }
            raw::engine_physics_remove_body(Some(world), Some(body), 1);
        }
    }

    fn hand_transform(&self) -> Matrix {
        let mut out = IDENTITY;
        // SAFETY: the holder's RightMiddle1 bone (0x309F00), the item held.
        unsafe { raw::engine_hand_transform(at!(self.artefact()), out.as_mut_ptr().cast()) };
        out
    }

    /// 0.28's vt+80h: the hand while held, the body in the air.
    fn current_transform(&self) -> Matrix {
        if !matches!(self.state(), State::InAir(_)) && self.inventory().is_some() {
            self.hand_transform()
        } else {
            self.transform()
        }
    }
}

// ---- the damage zone -------------------------------------------------------

/// What a sponge's zone's hits name: its thrower's object (0 none), its
/// inflictor and it.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Hit {
    zone: u32,
    instigator: u32,
    inflictor: u32,
    item: u32,
}

/// The zone hit hooks' state (sponge.zone_hit, sponge.zone_hit_done): no
/// sponge's zone ticking; one ticking; and between a hit's last pushes and
/// its call, the loop's EBX lent to the `push ebx` of the instigator.
/// `repr(C, u32)`: check_sponge_026g.py reads it (the tag, then the hit).
#[repr(C, u32)]
#[derive(Clone, Copy)]
enum ZoneHit {
    Idle,
    Ticking(Hit),
    Lent(Hit, u32),
}

static ZONE_HIT: GameCell<ZoneHit> = GameCell::new(ZoneHit::Idle);

impl Sponge<'_> {
    /// 0.28's 0x51E520 (as 0.26g's anomaly does it, 0x31B9A0): the zone's
    /// geometries' ghosts into the physics world, its timers reset. Its
    /// vt+1Ch activation is damage_zone's 0x75AF0 (`ret 4`): nothing.
    fn zone_on(&self) {
        let (Some(zone), Some(world)) = (self.zone(), self.physics_world()) else { return };
        // SAFETY: our zone, the registered world's physics world.
        unsafe {
            zone.set_physics(Some(world));
            raw::engine_sensor_insert(at!(zone.sensor()), at!(world));
            *zone.last_hit() = ZONE_NO_HIT;
            *zone.hitting() = 0;
        }
    }

    fn zone_off(&self, flag: u32) {
        if let Some(zone) = self.zone() {
            // SAFETY: our zone.
            unsafe { raw::engine_zone_deactivate(at!(zone), flag) };
        }
    }

    pub fn zone_active(&self) -> bool {
        // SAFETY: our zone's flag.
        self.zone().is_some_and(|z| unsafe { read(z.active()) } != 0)
    }

    fn geometry(&self) -> Option<raw::CollisionGeometry> {
        let zone = self.zone()?;
        // SAFETY: our zone's geometry list, its count checked.
        unsafe { (read(zone.geometry_count()) != 0).then(|| zone.geometries()).flatten()?.geometry() }
    }

    /// 0.28's +A8h (0x4AE930): the zone's sound and effect zones where the
    /// explosion is, their own offsets kept (local x transform).
    fn zone_place(&self, transform: &Matrix) {
        let Some(zone) = self.zone() else { return };
        // SAFETY: our zone's matrices, their counts checked; our locals.
        unsafe {
            let sounds = read(self.this.sounds()).min(read(zone.sound_count()));
            for i in 0..sounds as usize {
                let local = self.this.sound_local().add(16 * i);
                raw::engine_matrix_multiply(transform.as_ptr().cast(), local.cast(), zone.sounds().add(16 * i).cast());
            }
            let effects = read(self.this.effects()).min(read(zone.effect_count()));
            for i in 0..effects as usize {
                let local = self.this.effect_local().add(16 * i);
                raw::engine_matrix_multiply(transform.as_ptr().cast(), local.cast(), zone.effects().add(16 * i).cast());
            }
        }
    }

    /// The zone's tick (0x319A70), its hits the thrower's (the hit hooks read
    /// ZONE_HIT): the instigator found again from the thrower's place for
    /// each tick, none kept after it.
    fn zone_tick(&self, dt: u32, time: u32) {
        let Some(zone) = self.zone() else { return };
        let instigator = self.owner_object();
        // SAFETY: our inflictor's field.
        unsafe { self.inflictor().set_instigator(instigator) };
        let hit = Hit {
            zone: zone.address(),
            instigator: instigator.map_or(0, |o| o.address()),
            inflictor: self.inflictor().address(),
            item: self.this.address(),
        };
        ZONE_HIT.set(self.thread, ZoneHit::Ticking(hit));
        // SAFETY: our zone, ticked as the anomaly ticks its own.
        unsafe { raw::engine_zone_tick(at!(zone), dt, time, u32::from(time.is_multiple_of(TICK))) };
        ZONE_HIT.set(self.thread, ZoneHit::Idle);
        // SAFETY: as above.
        unsafe { self.inflictor().set_instigator(None) };
    }
}

/// sponge.zone_hit: a sponge's zone's hit names its thrower, its inflictor
/// and it (the receiver's call's item and inflictor already pushed 0, its
/// instigator pushed from EBX next).
pub fn zone_hit(ctx: &mut Context<'_>) {
    let ZoneHit::Ticking(hit) = ZONE_HIT.get(ctx.thread()) else { return };
    // SAFETY: the site's EDI, the zone ticked.
    let Some(zone) = (unsafe { raw::site::sponge_zone_hit::zone(ctx) }) else { return };
    if zone.address() != hit.zone {
        return;
    }
    // SAFETY: the site's frame: the call's arguments pushed so far.
    if let Some(frame) = unsafe { raw::site::sponge_zone_hit::frame(ctx) } {
        unsafe {
            *frame.inflictor() = hit.inflictor;
            *frame.item() = hit.item;
        }
    }
    let ebx = ctx.reg(Reg::Ebx);
    ctx.set_reg(Reg::Ebx, hit.instigator);
    ZONE_HIT.set(ctx.thread(), ZoneHit::Lent(hit, ebx));
}

/// sponge.zone_hit_done: EBX the loop's own again.
pub fn zone_hit_done(ctx: &mut Context<'_>) {
    if let ZoneHit::Lent(hit, ebx) = ZONE_HIT.get(ctx.thread()) {
        ctx.set_reg(Reg::Ebx, ebx);
        ZONE_HIT.set(ctx.thread(), ZoneHit::Ticking(hit));
    }
}

// ---- the passive (0.28's 0x5139A0 / 0x513A10, on 0.26g's Lifebone list) ---

impl Sponge<'_> {
    /// Linked once: out of the list it may still be in first.
    pub fn enable_passive(&self) {
        self.disable_passive();
        // SAFETY: the live holder's damage model and its infection pool; our node.
        unsafe {
            let Some(model) = self.holder().and_then(|p| p.damage_model()) else { return };
            let name = c"infection".as_ptr().cast::<u8>();
            let Some(pool) = raw::engine_find_pool(Some(model.pools()), &name) else { return };
            let list = pool.regeneration();
            let node = self.this.node();
            node.set_next(None);
            match list.tail() {
                None => list.set_head(Some(node)),
                Some(tail) => tail.set_next(Some(node)),
            }
            list.set_tail(Some(node));
            self.this.set_linked(Some(list));
        }
    }

    /// Lifebone's 0x312380 for our node: out of the list it went into.
    pub fn disable_passive(&self) {
        // SAFETY: our node and the list it was linked into.
        unsafe {
            let Some(list) = self.this.linked() else { return };
            self.this.set_linked(None);
            let node = self.this.node();
            let (mut before, mut at) = (None, list.head());
            while let Some(n) = at {
                if n.address() == node.address() {
                    break;
                }
                before = Some(n);
                at = n.next();
            }
            let Some(found) = at else { return };
            match before {
                Some(b) => b.set_next(found.next()),
                None => list.set_head(found.next()),
            }
            if found.next().is_none() {
                list.set_tail(before);
            }
        }
    }

    /// The passive as the state says: in while it is in an inventory and
    /// held (3), active (4) or armed, out otherwise.
    fn sync_passive(&self) {
        let game = self.game_state();
        let state = self.state();
        let armed = state.armed().is_some();
        let held = state == State::Held && (game == ST_PASSIVE || game == ST_ACTIVE);
        if self.inventory().is_some() && (armed || held) {
            self.enable_passive();
        } else {
            self.disable_passive();
        }
    }
}

// ---- the explosion -------------------------------------------------------

impl Sponge<'_> {
    /// 0.28's 0x513FD0: the splash's parameters, the thrower's radius (12)
    /// and damage (11) modifiers on them, the inflictor this one naming him
    /// (a thrower out of the match neither).
    fn splash_hit(&self, centre: [f32; 3], time: u32) {
        let mut out = raw::SplashParametersStorage::zeroed();
        let player = self.owner_player();
        // SAFETY: our template copied into our room, the world's hit manager.
        unsafe {
            let hit = out.handle();
            core::ptr::copy_nonoverlapping(
                self.this.splash().address() as usize as *const u8,
                hit.address() as usize as *mut u8,
                raw::SplashParameters::SIZE,
            );
            hit.set_inflictor(Some(self.inflictor()));
            self.inflictor().set_instigator(player.map(|p| p.game_object()));
            if let Some(player) = player {
                *hit.radius() = modified(player, M_RADIUS, read(hit.radius()));
                *hit.min() = modified(player, M_DAMAGE, read(hit.min()));
                *hit.max() = modified(player, M_DAMAGE, read(hit.max()));
                *hit.stamina() = modified(player, M_DAMAGE, read(hit.stamina()));
                *hit.stopping() = modified(player, M_DAMAGE, read(hit.stopping()));
            }
            let Some(hits) = self.world().and_then(|w| w.hits()) else { return };
            raw::engine_splash_hit(at!(hit), at!(hits), centre.as_ptr(), time);
            self.inflictor().set_instigator(None); // the zone's ticks name him again each time
        }
    }

    /// The explosion at `transform`: the splash, the zone on there, out of
    /// the physics world, game state 7, the passive out (0.28 leaves it in
    /// the thrower's list). Still in its slot while a throw state holds it.
    fn explode(&self, transform: &Matrix, physics: Option<Physics>, time: u32) {
        self.place(transform);
        self.splash_hit(position(transform), time);
        self.set_transform(transform); // the explosion point: the zone's hits' position
        self.zone_on();
        if let Some(geometry) = self.geometry() {
            // SAFETY: our zone's geometry.
            unsafe { raw::engine_geometry_set_transform(Some(geometry), transform.as_ptr()) };
        }
        self.zone_place(transform);
        if let Some(physics) = physics {
            self.leave_physics(physics);
        }
        self.set_game_state(ST_ZONE);
        self.disable_passive();
    }

    /// Out of its slot when it is still in it (0x2FC680; its
    /// removed_from_inventory sees it spent and gives the grenade slot back).
    fn consume(&self) {
        let (Some(inventory), slot) = (self.inventory(), self.slot()) else { return };
        // SAFETY: the live inventory's slot, below its 23.
        unsafe {
            if slot < SLOTS && inventory.slots().at(slot as usize).item().map(|i| i.address()) == Some(self.id()) {
                raw::engine_inventory_remove(Some(inventory), slot);
            }
        }
    }
}

// ---- the sensor (0.69d0's thrown_with_sensor state, its tick 0x2003EBF10) --

/// Every living player's damage collision bones as of now (0x2DA5B0, as the
/// splash calls it for each player it finds), so the sweep finds them where
/// every game has them this tick.
fn fresh_bones(world: raw::CoreWorld) {
    for place in 0..::sponge::PLACES {
        if let Some(player) = place_kit_player(world, place) {
            // SAFETY: a live player of the world.
            unsafe {
                if read(player.alive()) != 0 {
                    raw::engine_recompute_damage_collision_bones(at!(player), core::ptr::null());
                }
            }
        }
    }
}

impl Sponge<'_> {
    /// A living player other than the thrower in its way from `from` to
    /// `to`: spheres of SENSOR_RADIUS at most that far apart, each the hit
    /// manager's collision query the splash makes (0x339650..0x339679).
    fn in_the_way(&self, from: [f32; 3], to: [f32; 3], thrower: Thrower) -> bool {
        let Some(world) = self.world() else { return false };
        // SAFETY: the world's hit manager and its query object.
        let Some(query) = (unsafe { world.hits() }).and_then(|h| unsafe { h.collision() }) else { return false };
        fresh_bones(world);
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let length2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let mut steps = 1u32;
        while steps < SENSOR_STEPS && length2 > (steps * steps) as f32 * SENSOR_RADIUS * SENSOR_RADIUS {
            steps += 1;
        }
        for i in 1..=steps {
            let k = i as f32 / steps as f32;
            let centre = [from[0] + d[0] * k, from[1] + d[1] * k, from[2] + d[2] * k];
            let mut found = raw::FoundObjectsStorage::zeroed();
            let mut hit = false;
            // SAFETY: the query fills our vector from its allocator, freed as
            // the splash frees it (0x3398CE); the found objects' owners live.
            unsafe {
                let list = found.handle();
                list.set_allocator(Some(raw::image::splash_found_allocator()));
                raw::engine_object_query(Some(query), centre.as_ptr(), SENSOR_RADIUS, 0x10, 0x40, Some(list));
                if let (Some(first), Some(end)) = (list.begin(), list.end()) {
                    let count = end.address().saturating_sub(first.address()) as usize / raw::FoundObject::SIZE;
                    for n in 0..count {
                        let owner = first.at(n).object().and_then(|o| o.owner());
                        let player = owner.and_then(|o| raw::engine_collision_owner(Some(o)));
                        if let Some(player) = player {
                            let place = read(player.place());
                            let thrower_hit = thrower.place().is_some_and(|p| p.get() == place);
                            if read(player.alive()) != 0 && !thrower_hit {
                                hit = true; // a corpse is no contact
                                break;
                            }
                        }
                    }
                }
                if let Some(first) = list.begin() {
                    raw::engine_pthreads_free(
                        list.allocator(),
                        at!(first),
                        raw::image::SPLASH_FOUND_FUNCTION,
                        raw::image::SPLASH_FOUND_FILE,
                        0x67,
                    );
                }
            }
            if hit {
                return true;
            }
        }
        false
    }

    /// Whether a throw state holds it now, from the world: its holder's held
    /// item a weapon, its current state the throw-grenade state with this
    /// sponge its grenade. Every game reads the same world.
    fn throw_state_holds(&self) -> bool {
        // SAFETY: the live holder's held item, its state checked by its vtable.
        unsafe {
            let Some(weapon) = self.holder().and_then(|p| p.held_item()) else { return false };
            let vtable = read(weapon.vtable());
            if vtable != raw::image::WEAPON_VTABLE && vtable != raw::image::WEAPON_CORE_VTABLE {
                return false;
            }
            let Some(state) = weapon.fsm_state() else { return false };
            read(state.vtable()) == raw::image::THROW_GRENADE_STATE_VTABLE
                && state.throw_grenade().grenade().map(|g| g.address()) == Some(self.id())
        }
    }
}

// ---- the slots ---------------------------------------------------------------

impl Sponge<'_> {
    /// 0.28's action 0x514580: a press (not armed) puts it in the grenade
    /// slot (the throw state starts: its pull arms it); a release takes it
    /// out (the throw state throws it). Another grenade in the grenade slot
    /// (a stock grenade being cooked) is its throw state's, not ours.
    pub fn action(&self, pressed: bool) {
        let Some(inventory) = self.inventory() else { return };
        // SAFETY: the live inventory's grenade slot.
        unsafe {
            let grenade = read(inventory.grenade_slot());
            let mine = self.slot();
            if grenade != NO_SLOT && grenade != mine {
                return;
            }
            *inventory.grenade_slot() = if pressed && self.state() == State::Held { mine } else { NO_SLOT };
        }
    }

    /// 0.28's removed_from_inventory 0x513200, its holes closed: the grenade
    /// slot back to none when it names its slot (the crash of 2026-10-25), the
    /// passive out; armed in the hand, it falls from the hand (into the
    /// physics world there, no speed), its fuse and thrower kept.
    pub fn removed(&self) {
        if let Some(inventory) = self.inventory() {
            // SAFETY: remove_item calls this before the item's inventory and slot are cleared.
            unsafe {
                if read(inventory.grenade_slot()) == self.slot() {
                    *inventory.grenade_slot() = NO_SLOT;
                }
            }
        }
        self.disable_passive();
        match self.state() {
            State::InHand(_) => {
                if self.body().is_some() && self.holder().is_some() {
                    let hand = self.hand_transform();
                    if let Some(physics) = self.launch(&hand, [0.0; 3]) {
                        self.step(Event::Launch { physics });
                    }
                }
            }
            State::InAir(_) | State::Cloud(_) | State::Spent => {}
            // its slot 34 (disable_active) is Rattle's 0x1AF330, a `ret`
            State::Held => self.set_game_state(ST_NONE),
        }
    }

    /// 0.28's 0x515190: the holder back, from state 5.
    pub fn revived(&self) {
        if self.state() == State::Held && self.game_state() == ST_DEAD {
            self.enable_passive();
            self.set_game_state(ST_PASSIVE);
        }
    }

    fn let_go(&self) {
        let step = self.step(Event::LetGo);
        if step.effect == Effect::Consume {
            if step.state == State::Held && self.game_state() == ST_ZONE {
                self.set_game_state(ST_NONE);
            }
            self.consume();
        }
    }

    /// 0.28's tick 0x514160: let go when no throw state holds it; the base's
    /// tick while it is an artefact; in the air its body's transform and the
    /// sensor's sweep; then the transition, the zone between the explosion
    /// and the cloud's end.
    pub fn tick(&self, dt: u32, time: u32) {
        if self.state().hold() == Hold::ThrowState && !self.throw_state_holds() {
            self.let_go();
        }
        if self.state() == State::Held {
            // SAFETY: artefact_base's tick on our artefact.
            unsafe { raw::engine_artefact_tick(Some(self.artefact()), dt, time) };
        }
        if let State::InAir(air) = self.state()
            && let (Some(world), Some(body)) = (self.physics_of(air.physics), self.body()) {
                let from = position(&self.transform());
                let mut at = IDENTITY;
                // SAFETY: our body's world transform.
                unsafe { raw::engine_body_get_transform(Some(body), at.as_mut_ptr()) };
                self.set_transform(&at);
                self.place(&at);
                self.physics_update(world, body);
                if air.armed.contact == machine::Contact::Waiting && self.in_the_way(from, position(&at), air.armed.thrower) {
                    self.step(Event::Contact);
                }
            }
        let was = self.state();
        let step = machine::transition(&self.rules(), was, Event::Tick { time: GameTime(time) });
        match step.effect {
            Effect::Explode(_) => {
                let at = self.current_transform();
                self.set_state(step.state); // the thrower the cloud's: the splash names him
                let physics = match was {
                    State::InAir(air) => Some(air.physics),
                    _ => None,
                };
                self.explode(&at, physics, time);
            }
            Effect::CloudOff => {}
            Effect::None | Effect::Arm | Effect::Consume => self.set_state(step.state),
        }
        if step.effect == Effect::Explode(Consumed::Now) {
            self.consume();
        }
        // the zone runs whatever the item's state; its last tick names the cloud's thrower too
        if self.zone_active() {
            self.zone_tick(dt, time);
        }
        if step.effect == Effect::CloudOff {
            self.zone_off(0);
            self.set_state(step.state);
            if step.state == State::Held && self.game_state() == ST_ZONE {
                self.set_game_state(ST_NONE);
            }
        }
    }

    /// 0.28's grenade_set_base[6]: ready to be thrown.
    pub fn can_throw(&self) -> bool {
        self.state().can_throw()
    }

    /// [1] 0x514610: the pin; the fuse starts here.
    pub fn pull(&self, time: u32) {
        self.step(Event::Pull { time: GameTime(time), thrower: self.holder_thrower() });
    }

    /// [2] 0x514620: thrown along the aim's z axis at throw_power x the
    /// holder's throw modifier (27), from the hand's transform.
    pub fn throw(&self, hand: &Matrix, aim: &Matrix) {
        if !matches!(self.state(), State::InHand(_)) || self.zone_active() {
            return;
        }
        let Some(player) = self.holder() else { return };
        // SAFETY: our config's number.
        let power = modified(player, M_THROW, unsafe { read(self.this.throw_power()) });
        let v = [aim[8] * power, aim[9] * power, aim[10] * power];
        if let Some(physics) = self.launch(hand, v) {
            self.step(Event::Launch { physics });
        }
    }

    /// A contact of its body (through the game's invoker): only noted; the
    /// next tick rules on it, on the world's tick grid as every game sees it.
    pub fn contact(&self) {
        self.step(Event::Contact);
    }
}

// ---- serialization (0.28's 0x513A70 / 0x513C80) ---------------------------

const SP_FILE: &core::ffi::CStr = c"artefact_sponge_core.cpp";
const SP_SERIALIZE: &core::ffi::CStr = c"survarium::artefact_sponge_core::serialize";

fn put(writer: raw::BufferWriter, field: Field) {
    // SAFETY: the game's field writers onto the live writer.
    unsafe {
        let (file, function) = (SP_FILE.as_ptr().cast(), SP_SERIALIZE.as_ptr().cast());
        match field {
            Field::U32 { name, line, value } => {
                raw::engine_write_u32((&value as *const u32).cast(), Some(writer), file, line, function, label(name))
            }
            Field::U8 { name, line, value } => {
                raw::engine_write_u8((&value as *const u8).cast(), Some(writer), file, line, function, label(name))
            }
            Field::Matrix { name, line, value } => {
                raw::engine_write_matrix(value.as_ptr().cast(), Some(writer), file, line, function, label(name))
            }
            Field::Float3 { name, line, value } => {
                raw::engine_write_float3(value.as_ptr().cast(), Some(writer), file, line, function, label(name))
            }
        }
    }
}

/// A field's name as a C string for the dumper (the record's names are
/// static texts; their copies with a zero here).
fn label(name: &str) -> *const u8 {
    for known in LABELS {
        if known.to_bytes() == name.as_bytes() {
            return known.as_ptr().cast();
        }
    }
    c"".as_ptr().cast()
}

const LABELS: [&core::ffi::CStr; 10] = [
    c"m_explode_time_ms",
    c"m_is_pin_pulled",
    c"thrown()",
    c"m_transform",
    c"linear_velocity",
    c"angular_velocity",
    c"m_contact",
    c"m_zone_owner",
    c"m_throw_state_holds",
    c"m_damage_zone->get_collision_geometry(0)->transform()",
];

/// What a reader has left: its bytes from the cursor to its end.
fn unread<'a>(reader: raw::BufferReader) -> &'a [u8] {
    // SAFETY: the live reader's message: begin + size its end, the cursor within.
    unsafe {
        let (begin, cursor) = (reader.begin(), reader.cursor());
        let end = begin as usize + read(reader.size()) as usize;
        let at = cursor as usize;
        if begin.is_null() || cursor.is_null() || at > end {
            return &[];
        }
        core::slice::from_raw_parts(at as *const u8, end - at)
    }
}

fn advance(reader: raw::BufferReader, bytes: usize) {
    // SAFETY: the live reader's cursor, moved over what was parsed.
    unsafe {
        let cursor = reader.cursor();
        if !cursor.is_null() {
            reader.set_cursor(cursor.add(bytes));
        }
    }
}

impl Sponge<'_> {
    fn motion(&self) -> Motion {
        let mut linear = [0.0f32; 3];
        let mut angular = [0.0f32; 3];
        if let Some(body) = self.body() {
            // SAFETY: our body's velocities.
            unsafe {
                raw::engine_body_linear_velocity(at!(body), linear.as_mut_ptr());
                raw::engine_body_angular_velocity(at!(body), angular.as_mut_ptr());
            }
        }
        Motion { transform: self.transform(), linear, angular }
    }

    pub fn serialize(&self, writer: &SpongeWriter, dumper: u32) {
        let (writer, dumper_pointer) = (writer.0, dumper as usize as *mut c_void);
        let base = self.game_state() as u8;
        let saved = Saved::of(&self.state(), base, || self.motion());
        // SAFETY: artefact_base's serialize first, as 0.28's.
        unsafe { raw::engine_artefact_serialize(Some(self.artefact()), Some(writer), dumper) };
        record::write(&saved, |field| put(writer, field));
        if let (Saved::Cloud(_), Some(zone)) = (saved, self.zone()) {
            // SAFETY: our zone's serializable.
            unsafe { raw::engine_zone_serialize(Some(zone.serial()), at!(writer), dumper_pointer) };
            put(writer, record::cloud_transform(self.transform()));
        }
    }

    /// The stream parsed once into the state, then the world made so: the
    /// body into or out of the physics world, the zone on or off, the passive.
    pub fn deserialize(&self, reader: &SpongeReader, a: u32, b: u32) {
        let (reader, a_pointer, b_pointer) = (reader.0, a as usize as *mut c_void, b as usize as *mut c_void);
        // SAFETY: artefact_base's deserialize first.
        unsafe { raw::engine_artefact_deserialize(Some(self.artefact()), Some(reader), a, b) };
        let was = self.state();
        let base = self.game_state() as u8;
        // A world refused by the world check (patch 87) never gets here; a
        // record not the Sponge's is read as an artefact's.
        let saved = match record::parse(base, unread(reader)) {
            Ok((saved, used)) => {
                advance(reader, used);
                saved
            }
            Err(_) => Saved::Held { base },
        };
        let now = match saved {
            Saved::InAir(armed, motion) => {
                self.set_transform(&motion.transform);
                self.place(&motion.transform);
                let physics = match (was, self.body()) {
                    (State::InAir(air), Some(body)) => {
                        if let Some(world) = self.physics_of(air.physics) {
                            // SAFETY: our body, in that world.
                            unsafe {
                                raw::engine_body_set_transform(Some(body), motion.transform.as_ptr());
                                self.physics_update(world, body);
                                raw::engine_body_set_linear_velocity(motion.linear.as_ptr(), at!(body));
                            }
                        }
                        Some(air.physics)
                    }
                    _ => self.launch(&motion.transform, motion.linear),
                };
                if let Some(body) = self.body() {
                    // SAFETY: our body.
                    unsafe { raw::engine_body_set_angular_velocity(motion.angular.as_ptr(), at!(body)) };
                }
                match physics {
                    Some(physics) => State::InAir(machine::Air { armed, physics }),
                    None => State::InHand(armed),
                }
            }
            other => {
                if let State::InAir(air) = was {
                    self.leave_physics(air.physics);
                }
                match other {
                    Saved::InHand(armed) => State::InHand(armed),
                    Saved::Cloud(cloud) => State::Cloud(cloud),
                    Saved::Spent => State::Spent,
                    Saved::Held { .. } | Saved::InAir(..) => State::Held,
                }
            }
        };
        self.set_state(now);
        // SAFETY: our inflictor.
        unsafe { self.inflictor().set_instigator(None) };
        match (now, self.zone()) {
            (State::Cloud(_), Some(zone)) => {
                if !self.zone_active() {
                    self.zone_on();
                }
                // SAFETY: our zone's serializable on the live reader.
                unsafe { raw::engine_zone_deserialize(Some(zone.serial()), at!(reader), a_pointer, b_pointer) };
                if let Ok((transform, used)) = record::parse_cloud_transform(unread(reader)) {
                    advance(reader, used);
                    self.set_transform(&transform);
                    self.place(&transform);
                    if let Some(geometry) = self.geometry() {
                        // SAFETY: our zone's geometry.
                        unsafe { raw::engine_geometry_set_transform(Some(geometry), transform.as_ptr()) };
                    }
                    self.zone_place(&transform);
                }
                // SAFETY: as above, the world registered.
                unsafe { raw::engine_zone_after_load(Some(zone.serial()), self.world()) };
            }
            _ => {
                if self.zone_active() {
                    self.zone_off(1);
                }
            }
        }
        self.sync_passive();
    }
}

// ---- the world and the draw ----------------------------------------------

fn unlink_drawable(list: raw::DrawableList, drawable: raw::SceneDrawable) {
    // SAFETY: the client world's live list of drawables; ours in it or not.
    unsafe {
        let mut at = list.head();
        while let Some(d) = at {
            if d.address() == drawable.address() {
                break;
            }
            at = d.next();
        }
        if at.is_none() {
            return;
        }
        let (prev, next) = (drawable.prev(), drawable.next());
        match prev {
            Some(p) => p.set_next(next),
            None => list.set_head(next),
        }
        match next {
            Some(n) => n.set_prev(prev),
            None => list.set_tail(prev),
        }
        drawable.set_prev(None);
        drawable.set_next(None);
    }
}

/// At the list's end, as Rattle's set_world 0x25FB30 links its drawable.
fn link_drawable(list: raw::DrawableList, drawable: raw::SceneDrawable) {
    // SAFETY: as unlink_drawable.
    unsafe {
        drawable.set_next(None);
        drawable.set_prev(list.tail());
        match list.tail() {
            None => list.set_head(Some(drawable)),
            Some(tail) => tail.set_next(Some(drawable)),
        }
        list.set_tail(Some(drawable));
    }
}

impl Sponge<'_> {
    fn client(&self) -> raw::ArtefactClient {
        // SAFETY: Rattle's drawable (its callback) at +340h.
        unsafe { self.artefact().rattle_callback() }
    }

    /// 0.28's set_world 0x46A760 / 0x514400: the world, the rigid body, both
    /// drawables (the artefact's, as Rattle's 0x25FB30, and the zone's).
    pub fn set_world(&self, world: u32) {
        // SAFETY: Rattle's set_world on our artefact.
        unsafe { raw::engine_rattle_set_world(Some(self.artefact()), world) };
        if self.body().is_none() {
            self.make_body();
        }
        // SAFETY: the client world the drawable names; our zone's drawable.
        unsafe {
            if let (Some(scene), Some(zone)) = (self.client().world(), self.zone()) {
                unlink_drawable(scene.drawables(), zone.drawable()); // once, however often the world is set
                link_drawable(scene.drawables(), zone.drawable());
            }
        }
    }

    /// The destructor: the passive's link, the physics world and the zone
    /// only forgotten; the geometry it made, its body and its hold on the
    /// zone go; then Rattle's destructor.
    pub fn destroy(&self, flags: u32) -> u32 {
        // SAFETY: our fields; the geometry ours; Rattle's destructor last.
        unsafe {
            self.this.set_linked(None);
            self.set_state(State::Held);
            let zone = self.zone();
            if let (ZoneHit::Ticking(hit) | ZoneHit::Lent(hit, _), Some(zone)) = (ZONE_HIT.get(self.thread), zone)
                && hit.zone == zone.address()
            {
                ZONE_HIT.set(self.thread, ZoneHit::Idle);
            }
            if let Some(made) = self.this.made_geometry() {
                self.this.set_made_geometry(None);
                if let Some(zone) = zone
                    && read(zone.geometry_count()) != 0
                        && let Some(slot) = zone.geometries()
                            && slot.geometry().map(|g| g.address()) == Some(made.address()) {
                                slot.set_geometry(None);
                            }
                raw::engine_geometry_destroy(at!(made));
                let mut pointer = at!(made);
                if let Some(allocator) = allocator() {
                    raw::engine_free(allocator, at!(made), &mut pointer);
                }
            }
            if let Some(body) = self.body() {
                self.destroy_body(body);
            }
            if let Some(zone) = zone {
                self.this.set_zone(None);
                release_zone(zone);
            }
            raw::engine_rattle_destroy(Some(self.artefact()), flags)
        }
    }
}

impl SpongeDrawable<'_> {
    pub(in crate::vostok) fn raw(&self) -> raw::SceneDrawable {
        self.drawable
    }

    fn model_call(&self, add_or_update: Option<bool>, m: &Matrix) {
        let client = self.sponge.client();
        // SAFETY: the drawable's client world, its game's renderer, the model's
        // render instance (0x25FB80's calls).
        unsafe {
            let (Some(world), Some(model)) = (client.world(), client.model()) else { return };
            let Some(renderer) = world.game().and_then(|g| g.renderer()).and_then(|r| r.scene_renderer()) else { return };
            let instance = at!(model.render_instance());
            let scene = at!(world.scene());
            match add_or_update {
                Some(true) => raw::engine_scene_add_model(instance, at!(renderer), scene, m.as_ptr().cast(), m.as_ptr().cast()),
                Some(false) => {
                    raw::engine_scene_update_model(instance, at!(renderer), scene, m.as_ptr().cast(), m.as_ptr().cast())
                }
                None => raw::engine_scene_remove_model(instance, at!(renderer), scene),
            }
        }
    }

    /// 0.28's presenter (0x4B9F20): armed, the model at the sponge's
    /// transform (the hand, the body in the air), added when it comes,
    /// updated, taken away when it goes; every other state the stock draw's.
    pub fn draw(&self) {
        let client = self.sponge.client();
        // SAFETY: the callback's state field.
        let last = unsafe { read(client.cached_state()) };
        if self.sponge.state().armed().is_some() {
            let m = self.sponge.current_transform();
            self.model_call(Some(last != ST_ARMED), &m);
            // SAFETY: as above.
            unsafe { *client.cached_state() = ST_ARMED };
            return;
        }
        if last == ST_ARMED {
            self.model_call(None, &IDENTITY);
            // SAFETY: as above.
            unsafe { *client.cached_state() = ST_NONE };
        }
        // SAFETY: Rattle's draw on its drawable.
        unsafe { raw::engine_rattle_draw(Some(self.drawable)) };
    }
}

// ---- the cook ----------------------------------------------------------------

fn root_of(config: raw::ConfigHandle) -> Option<raw::EngineCfg> {
    // SAFETY: a cook's &config: the config resource, its root.
    unsafe { config.resource()?.root() }
}

fn is_sponge_type(value: raw::EngineCfg) -> bool {
    bits_of(value) == TYPE_SPONGE
}

/// The construct site's `&data/type`: a sponge's type.
fn construct_type(ctx: &Context<'_>) -> bool {
    // SAFETY: the site's EAX, &data/type.
    unsafe { raw::site::sponge_construct::value(ctx) }.is_some_and(is_sponge_type)
}

/// sponge.size for a sponge: EAX its size.
pub fn size(ctx: &mut Context<'_>) -> bool {
    // SAFETY: as construct_type.
    if !unsafe { raw::site::sponge_size::value(ctx) }.is_some_and(is_sponge_type) {
        return false;
    }
    ctx.set_reg(Reg::Eax, raw::Sponge::SIZE as u32);
    true
}

/// Is `config`'s artefact a sponge (data/type 12)?
fn config_is_sponge(config: raw::ConfigHandle) -> bool {
    root_of(config).map(|root| must(must(root, c"data"), c"type")).is_some_and(is_sponge_type)
}

/// 0.28's reader 0x543930/0x5439E0, onto 0.26g's splash layout (the grenade
/// set's 0x309AE0: bodies -1 filled, the type by 0x304370).
fn read_config(this: raw::Sponge, root: raw::EngineCfg) {
    let data = must(root, c"data");
    let passive = must(data, c"passive");
    let active = must(data, c"active");
    // SAFETY: our fields, filled from the config.
    unsafe {
        let p = this.passive();
        *p.rate_add() = number(must(passive, c"regen_add"));
        *p.rate_mul() = number(must(passive, c"regen_mul"));
        *p.delay_add() = number(must(passive, c"timeout_add"));
        *p.delay_mul() = number(must(passive, c"timeout_mul"));
        *p.limit_add() = number(must(passive, c"threshold_add"));
        *p.limit_mul() = number(must(passive, c"threshold_mul"));
        *this.duration() = whole_ms(must(active, c"duration_s"));
        *this.ttl() = number(must(data, c"ttl_ms")) as i32 as u32;
        *this.safe_time() = number(must(data, c"safe_time")) as i32 as u32;
        *this.explode_on_contact() =
            u8::from(child(data, c"explode_on_contact").is_some_and(|v| bits_of(v) != 0));
        *this.material() = bits_of(must(data, c"mtl_idx")) as u16;
        *this.throw_power() = number(must(active, c"throw_power"));
        let h = this.splash();
        *h.radius() = number(must(active, c"splash_radius"));
        *h.max() = number(must(active, c"splash_max_damage"));
        *h.min() = number(must(active, c"splash_min_damage"));
        *h.pierce() = number(must(active, c"splash_pierce"));
        *h.stamina() = number(must(active, c"stamina_damage"));
        *h.stopping() = number(must(active, c"stopping_power"));
        *h.kind() = raw::engine_damage_type_of(text(must(active, c"splash_damage_type")));
        let bodies = h.bodies();
        for i in 0..8 {
            *bodies.add(i) = u32::MAX;
        }
        let list = must(active, c"splash_affected_bodies");
        for i in 0..count_of(list).min(8) {
            if let Some(body) = item(list, i) {
                *bodies.add(i.into()) = bits_of(body);
            }
        }
    }
}

/// sponge.construct for a sponge: Rattle's object, then the sponge's (0.28's
/// ctor 0x5136E0); EAX the object (0 for no memory).
pub fn construct(ctx: &mut Context<'_>) -> bool {
    if !construct_type(ctx) {
        return false;
    }
    let object = construct_object(ctx);
    ctx.set_reg(Reg::Eax, object);
    true
}

fn construct_object(ctx: &Context<'_>) -> u32 {
    // SAFETY: the site's frame: the construct's arguments.
    let Some(frame) = (unsafe { raw::site::sponge_construct::frame(ctx) }) else { return 0 };
    // SAFETY: the memory the cook sized (our SIZE), its config live.
    unsafe {
        let Some(memory) = frame.memory() else { return 0 };
        let Some(config) = frame.config() else { return 0 };
        let Some(root) = root_of(config) else { return 0 };
        let this = memory;
        raw::engine_rattle_construct(read(frame.effect()) as usize as *mut c_void, at!(root), at!(this),
                                     read(frame.scene()) as usize as *mut c_void);
        // the sponge's own fields after Rattle's object, zeroed (machine::State::Held is 0)
        core::ptr::write_bytes((this.address() as usize as *mut u8).add(raw::RattleObject::SIZE), 0,
                               raw::Sponge::SIZE - raw::RattleObject::SIZE);
        install_vtables(this);
        let inflictor = this.inflictor();
        *inflictor.id() = 0xFFFF;
        inflictor.set_item(Some(this.artefact()));
        read_config(this, root);
        let node = this.node();
        *node.vtable() = raw::image::REGENERATION_FUNCTION_VTABLE | 1;
        *node.function() = raw::image::LIFEBONE_REGENERATION;
        *node.this() = this.address();
        node.set_modifier(Some(this.passive()));
        core::ptr::write(this.state().cast::<State>(), State::Held);
        let contact = this.contact_fn();
        *contact.vtable() = raw::image::CONTACT_FUNCTION_VTABLE | 1;
        *contact.function() = super::sponge_on_contact_entry().entry() as usize as u32;
        *contact.object() = this.address();
        write_matrix(this.transform(), &IDENTITY);
        this.address()
    }
}

fn install_vtables(this: raw::Sponge) {
    // SAFETY: the generated copies (patch sponge's TOML), installed on our
    // object as its constructor's last vtable writes.
    unsafe {
        *this.artefact().item().vtable() = super::sponge_artefact_vtable_handle().address();
        *this.artefact().drawable().vtable() = super::sponge_drawable_vtable_handle().address();
        *this.inflictor().vtable() = super::sponge_inflictor_vtable_handle().address();
    }
}

/// Is this artefact a sponge (its vtable the Sponge's copy)?
fn is_sponge(artefact: raw::Item) -> bool {
    // SAFETY: a live item's vtable, compared only.
    unsafe { read(artefact.vtable()) == super::sponge_artefact_vtable_handle().address() }
}

/// The item a site names, if it is a sponge.
fn sponge_item<'t>(thread: &'t Thread, item: Option<raw::Item>) -> Option<Sponge<'t>> {
    let item = item.filter(|i| is_sponge(*i))?;
    // SAFETY: a sponge's item is its artefact's (+0).
    Some(Sponge { this: unsafe { item.artefact().sponge() }, thread })
}

/// The zone's creation data: a variant holding the config value (as 0x2F2C55).
fn local_matrix(zone: raw::EngineCfg, rotated: bool) -> Matrix {
    let mut t = IDENTITY;
    // SAFETY: the config's float3s, the game's matrix helpers.
    unsafe {
        raw::engine_matrix_translation(t.as_mut_ptr().cast(), bits_of(must(zone, c"position")) as usize as *const f32);
        if !rotated {
            return t;
        }
        let mut r = IDENTITY;
        raw::engine_matrix_rotation(bits_of(must(zone, c"rotation")) as usize as *const f32, r.as_mut_ptr().cast());
        let mut out = IDENTITY;
        raw::engine_matrix_multiply(t.as_ptr().cast(), r.as_ptr().cast(), out.as_mut_ptr().cast()); // rotation, then translation (0x257DB7)
        out
    }
}

/// The zone cooked: its collision geometry from the inline config (as the
/// collision sensors with inline geometries do it, 0x3046F0), the sound and
/// effect zones' own offsets kept for the explosion.
fn zone_cooked(this: raw::Sponge, zone: raw::DamageZone, config: raw::EngineCfg) {
    // SAFETY: the zone resource the query made, one reference ours from here.
    unsafe {
        this.set_zone(Some(zone));
        acquire(zone.references());
        let geometries = must(config, c"collision_geometries");
        if read(zone.geometry_count()) != 0 && count_of(geometries) != 0
            && let (Some(allocator), Some(slot), Some(first)) = (allocator(), zone.geometries(), item(geometries, 0)) {
                let shape = raw::engine_malloc_geometry(allocator, raw::CollisionGeometry::SIZE as u32,
                                                        raw::image::COLLISION_GEOMETRY_INFO as usize as *const c_void);
                if let Some(shape) = shape {
                    core::ptr::write_bytes(shape.address() as usize as *mut u8, 0, raw::CollisionGeometry::SIZE);
                    *shape.vtable() = raw::image::COLLISION_GEOMETRY_VTABLE;
                    *shape.vtable2() = raw::image::COLLISION_GEOMETRY_VTABLE2;
                    slot.set_geometry(Some(shape));
                    this.set_made_geometry(Some(shape));
                    raw::engine_geometry_load(at!(shape), at!(first));
                }
            }
        for (key, rotated, count, locals) in [
            (c"sound_zones", false, this.sounds(), this.sound_local()),
            (c"effect_zones", true, this.effects(), this.effect_local()),
        ] {
            if let Some(zones) = child(config, key) {
                let n = u32::from(count_of(zones)).min(MAX_ZONES);
                for i in 0..n {
                    if let Some(z) = item(zones, i as u16) {
                        write_matrix(locals.add(16 * i as usize), &local_matrix(z, rotated));
                    }
                }
                *count = n;
            }
        }
    }
}

/// The four resources come: Rattle's loader for the model, the particle and
/// the sound (0..2), the zone (3); then the cook's finish, as the stock
/// callback 0x25E960 does for Rattle.
pub fn loaded(callee: &SpongeCallee<'_>, results: &SpongeLoad) {
    let (Some(sponge), Some(results)) = (callee.sponge(), results.0) else { return };
    let this = sponge.this;
    // SAFETY: the query's results, alive through the callback; our fields.
    unsafe {
        let cook = this.loading_cook();
        let config = this.loading_config();
        raw::engine_rattle_loaded(at!(this.artefact()), at!(results));
        let zone = results.results().at(3).zone();
        let damage = config.and_then(|c| c.root()).map(|root| must(must(must(root, c"data"), c"active"), c"damage_zone"));
        if let (Some(zone), Some(damage)) = (zone, damage) {
            zone_cooked(this, zone, damage);
        }
        raw::engine_cook_finish_query(cook.map_or(core::ptr::null_mut(), |c| at!(c)), at!(this.loading_handle()),
                                      at!(this.artefact()),
                                      read(results.query()) as usize as *mut c_void);
        if let Some(config) = config {
            release_config(config);
        }
        this.set_loading_cook(None);
        this.set_loading_config(None);
    }
}

/// 0.28's cook 0x46A8A0 on 0.26g's (0x25E800): one query of the model, the
/// spawn particle and sound and a damage zone (class 75h) made from
/// data/active/damage_zone. False for anything not a sponge.
pub fn query(ctx: &Context<'_>) -> bool {
    // SAFETY: the site's ECX and frame: the cook and the query's arguments.
    let (Some(cook), Some(frame)) = (unsafe { raw::site::sponge_query::cook(ctx) }, unsafe { raw::site::sponge_query::frame(ctx) })
    else {
        return false;
    };
    // SAFETY: the frame's live arguments.
    let (Some(config), Some(artefact)) = (unsafe { frame.config() }, unsafe { frame.artefact() }) else { return false };
    if !config_is_sponge(config) {
        return false;
    }
    let Some(root) = root_of(config) else { return false };
    let object = must(root, c"object");
    let sound = must(object, c"sound_on_spawn");
    let damage = must(must(must(root, c"data"), c"active"), c"damage_zone");
    let (Some(sound_kind), Some(sound_name)) = (item(sound, 0), item(sound, 1)) else { return false };
    let mut requests = [raw::ResourceRequestStorage::zeroed(), raw::ResourceRequestStorage::zeroed(),
                        raw::ResourceRequestStorage::zeroed(), raw::ResourceRequestStorage::zeroed()];
    let names = [text(must(object, c"model")), text(must(object, c"particle_on_spawn")), text(sound_name),
                 c"damage_zone".as_ptr().cast()];
    let classes = [CLASS_MODEL, CLASS_PARTICLE, bits_of(sound_kind), CLASS_DAMAGE_ZONE];
    let mut variant = raw::ConfigVariantStorage::zeroed();
    let mut callback = raw::QueryCallbackStorage::zeroed();
    let Some(allocator) = allocator() else { return false };
    // SAFETY: our rooms, on this stack for the call (the query copies the
    // requests, the creation data and the callback before it returns); the
    // config held until loaded() releases it.
    unsafe {
        for (i, room) in requests.iter_mut().enumerate() {
            let q = room.handle();
            q.set_path(names[i].cast_mut());
            *q.class() = classes[i];
        }
        let v = variant.handle();
        *v.vtable() = raw::image::CONFIG_VARIANT_VTABLE;
        core::ptr::copy_nonoverlapping(damage.address() as usize as *const u8, v.value().address() as usize as *mut u8,
                                       raw::EngineCfg::SIZE);
        *v.this() = v.address();
        *v.type_id() = raw::engine_config_variant_type();
        let this = artefact.sponge();
        this.set_loading_cook(Some(cook));
        let resource = config.resource();
        this.set_loading_config(resource);
        if let Some(resource) = resource {
            acquire(resource.references());
        }
        let member = super::sponge_on_loaded_entry().entry();
        let bound = raw::engine_bind_member_callback(Some(callback.handle()), member, 0, this.address(), 0);
        let mut creation = [core::ptr::null_mut::<c_void>(), core::ptr::null_mut(), core::ptr::null_mut(), at!(v)];
        raw::engine_query_resources(requests.as_ptr().cast(), 4, bound.map_or(core::ptr::null_mut(), |c| at!(c)),
                                    allocator, creation.as_mut_ptr().cast(), read(frame.parent()) as usize as *mut c_void, 1);
    }
    true
}

// ---- the throw-grenade state's sites -------------------------------------

/// sponge.predicate: AL the answer for a sponge or an empty slot (false for the stock path).
pub fn predicate(ctx: &mut Context<'_>) -> bool {
    // SAFETY: the site's ECX, the grenade slot's item (0 for an empty slot).
    let answer = match unsafe { raw::site::sponge_predicate::item(ctx) } {
        None => Some(false), // an empty slot: none can be thrown
        item => sponge_item(ctx.thread(), item).map(|s| s.can_throw()),
    };
    let Some(answer) = answer else { return false };
    ctx.set_reg8(Reg::Eax, u8::from(answer));
    true
}

/// What the throw state holds: none, a sponge, or a stock grenade.
pub enum Held<'t> {
    Nothing,
    Sponge(Sponge<'t>),
    Stock,
}

fn held<'t>(thread: &'t Thread, state: Option<raw::ThrowGrenadeState>) -> Held<'t> {
    // SAFETY: a live throw state's grenade.
    match state.and_then(|s| unsafe { s.grenade() }) {
        None => Held::Nothing,
        item => sponge_item(thread, item).map_or(Held::Stock, Held::Sponge),
    }
}

/// sponge.ready: Some(ready) for a sponge or no grenade.
pub fn ready(ctx: &Context<'_>) -> Option<bool> {
    // SAFETY: the site's ESI, the throw state.
    let state = unsafe { raw::site::sponge_ready::state(ctx) };
    match held(ctx.thread(), state) {
        // SAFETY: its index byte.
        Held::Nothing => Some(state.is_some_and(|s| unsafe { read(s.index()) } != 0xFF)),
        Held::Sponge(sponge) => Some(sponge.zone_active()),
        Held::Stock => None,
    }
}

/// sponge.throw: true when the site's throw is done here (a sponge, or none held).
pub fn throw(ctx: &Context<'_>) -> bool {
    // SAFETY: the site's ESI, EDI and frame.
    let (state, player, frame) = unsafe {
        (raw::site::sponge_throw::state(ctx), raw::site::sponge_throw::player(ctx), raw::site::sponge_throw::frame(ctx))
    };
    match held(ctx.thread(), state) {
        Held::Nothing => true,
        Held::Sponge(sponge) => {
            if let (Some(player), Some(frame)) = (player, frame) {
                // SAFETY: the hand's transform on the frame, the player's aim.
                let (hand, aim) = unsafe { (read_matrix(frame.hand()), read_matrix(player.eye())) };
                sponge.throw(&hand, &aim);
            }
            true
        }
        Held::Stock => false,
    }
}

/// sponge.pull: done here (a sponge, or none held): AL the index, ESI the throw state.
pub fn pull(ctx: &mut Context<'_>) -> bool {
    // SAFETY: the site's ESI and EAX.
    let (substate, time) = unsafe { (raw::site::sponge_pull::substate(ctx), raw::site::sponge_pull::time(ctx)) };
    // SAFETY: the substate's throw state.
    let state = substate.and_then(|s| unsafe { s.state() });
    let index = match held(ctx.thread(), state) {
        Held::Nothing => 0xFF,
        Held::Sponge(sponge) => {
            sponge.pull(time);
            0
        }
        Held::Stock => return false,
    };
    ctx.set_reg(Reg::Esi, state.map_or(0, |s| s.address()));
    ctx.set_reg8(Reg::Eax, index);
    true
}

/// sponge.save: no grenade held: DL 17h (none).
pub fn save(ctx: &mut Context<'_>) -> bool {
    // SAFETY: the site's EBX, the throw state.
    let nothing = unsafe { raw::site::sponge_save::state(ctx) }.is_some_and(|s| unsafe { s.grenade() }.is_none());
    if nothing {
        ctx.set_reg8(Reg::Edx, NO_SLOT as u8);
    }
    nothing
}

/// sponge.load: a slot past the inventory's 23: no item (EAX 0).
pub fn load(ctx: &mut Context<'_>) -> bool {
    // SAFETY: the site's CL.
    let past = unsafe { raw::site::sponge_load::slot(ctx) } & 0xFF >= SLOTS;
    if past {
        ctx.set_reg(Reg::Eax, 0);
    }
    past
}

// ---- the owner's spots ---------------------------------------------------

/// The cores' tables extended with the Sponge's units: copies the cook reads
/// right after the look-up, a ring of eight (a copy is not overwritten while
/// the cook that asked reads it).
struct Kinds {
    table: raw::EngineCfgStorage,
    entries: [raw::EngineCfgStorage; spots::KINDS_MAX as usize + 1],
    keys: [raw::EngineCfgStorage; 2],
}

struct Ring(UnsafeCell<[Kinds; 8]>);
// SAFETY: written and read only on the game's thread that cooks (the
// resource system's cook thread for a project), one copy at a time.
unsafe impl Sync for Ring {}

static KINDS: Ring = Ring(UnsafeCell::new(
    [const {
        Kinds {
            table: raw::EngineCfgStorage::zeroed(),
            entries: [const { raw::EngineCfgStorage::zeroed() }; spots::KINDS_MAX as usize + 1],
            keys: [const { raw::EngineCfgStorage::zeroed() }; 2],
        }
    }; 8],
));
static KINDS_NEXT: GameCell<u32> = GameCell::new(0);
static SPOTS: GameCell<SpotTable> = GameCell::new(SpotTable::new());
/// The current map's `mode/artefact_spots` (set at each core's cook; the
/// cores of a project are cooked before any is linked).
static MAP_SPOTS: GameCell<Option<raw::EngineCfg>> = GameCell::new(None);

/// The match's map's `mode/artefact_spots`, None for none.
fn map_spots(cook: Option<raw::Cook>) -> Option<raw::EngineCfg> {
    // SAFETY: the cook's context, the game, its match options and dictionary.
    unsafe {
        let game = cook?.context()?.game()?;
        let options = raw::engine_match_options(game.network_client())?;
        let id = u32::from(read(options.map()));
        let dictionary = game.dictionary()?;
        let maps = dictionary.maps()?;
        for i in 0..read(dictionary.map_count()) as usize {
            let map = maps.at(i);
            if read(map.id()) != id {
                continue;
            }
            let mode = child(map.config()?.root()?, c"mode").filter(|m| kind_of(*m) == CFG_NAMED)?;
            return child(mode, c"artefact_spots").filter(|s| kind_of(*s) == CFG_INDEXED);
        }
        None
    }
}

fn rows<'a>(spots: raw::EngineCfg) -> impl Iterator<Item = Row<'a>> + Clone {
    (0..count_of(spots)).filter_map(move |i| item(spots, i)).map(|row| Row {
        core: text_key(row, c"core"),
        index: int_key(row, c"index").map(bits_of),
        kind: int_key(row, c"kind").map(bits_of),
    })
}

/// sponge.artefacts: the table the cook reads for a core (EAX).
pub fn artefacts(ctx: &mut Context<'_>) {
    let thread = ctx.thread();
    // SAFETY: the site's EAX and frame.
    let (Some(table), Some(frame)) = (unsafe { raw::site::sponge_artefacts::table(ctx) }, unsafe {
        raw::site::sponge_artefacts::frame(ctx)
    }) else {
        return;
    };
    let mut table_spots = SPOTS.get(thread);
    table_spots.cooking();
    SPOTS.set(thread, table_spots);
    // SAFETY: the frame's cook and core config.
    let (cook, core) = unsafe { (frame.cook(), frame.config()) };
    let spots = map_spots(cook);
    MAP_SPOTS.set(thread, spots);
    let (Some(spots), Some(core)) = (spots, core) else { return };
    if kind_of(table) != CFG_INDEXED || count_of(table) == 0 || count_of(table) > spots::KINDS_MAX {
        return;
    }
    let Some(containers) = child(core, c"artefact_containers").filter(|c| kind_of(*c) == CFG_INDEXED && count_of(*c) != 0)
    else {
        return;
    };
    let Some(name) = text_key(core, c"full_name") else { return };
    let picks = spots::picks(rows(spots), name, count_of(containers).into());
    let Some(first) = item(table, 0) else { return };
    if picks == 0 || count_of(first) != 2 || int_key(first, c"dict_id").is_none() || int_key(first, c"count").is_none() {
        return;
    }
    let n = KINDS_NEXT.get(thread);
    KINDS_NEXT.set(thread, n.wrapping_add(1));
    // SAFETY: our ring's copy (see Ring); the table's children copied as
    // they are, the added entry's keys our copies of the first entry's.
    unsafe {
        let copy = &mut (*KINDS.0.get())[n as usize % 8];
        let count = usize::from(count_of(table));
        let entries = raw::EngineCfgStorage::first(&mut copy.entries).expect("rooms");
        core::ptr::copy_nonoverlapping(table.children().map_or(core::ptr::null(), |c| c.address() as usize as *const u8),
                                       entries.address() as usize as *mut u8, count * raw::EngineCfg::SIZE);
        let keys = raw::EngineCfgStorage::first(&mut copy.keys).expect("rooms");
        core::ptr::copy_nonoverlapping(first.children().map_or(core::ptr::null(), |c| c.address() as usize as *const u8),
                                       keys.address() as usize as *mut u8, 2 * raw::EngineCfg::SIZE);
        let entry = entries.at(count);
        core::ptr::copy_nonoverlapping(first.address() as usize as *const u8, entry.address() as usize as *mut u8,
                                       raw::EngineCfg::SIZE);
        entry.set_children(Some(keys));
        *must(entry, c"dict_id").data() = ::sponge::KIND;
        *must(entry, c"count").data() = picks;
        let out = copy.table.handle();
        core::ptr::copy_nonoverlapping(table.address() as usize as *const u8, out.address() as usize as *mut u8,
                                       raw::EngineCfg::SIZE);
        out.set_children(Some(entries));
        *out.count() = (count + 1) as u16;
        ctx.set_reg(Reg::Eax, out.address());
    }
}

fn core_id(core: raw::GenericAnomalyCore) -> Core {
    Core(NonZeroU32::new(core.address()).expect("a handle is never 0"))
}

/// sponge.links: after a core's resolve_links, the kinds of its picked containers.
pub fn links(ctx: &Context<'_>) {
    let thread = ctx.thread();
    // SAFETY: the site's frame: the core's link interface and its config.
    let Some(frame) = (unsafe { raw::site::sponge_links::frame(ctx) }) else { return };
    let Some(links) = (unsafe { frame.links() }) else { return };
    // SAFETY: the interface at the core's +4.
    let core = unsafe { links.core() };
    let config = unsafe { frame.config() };
    // SAFETY: the core's containers, linked just now.
    let containers = unsafe { read(core.containers_end()).saturating_sub(read(core.containers_begin())) / 4 };
    let picks = MAP_SPOTS.get(thread).and_then(|spots| {
        let name = text_key(config, c"full_name")?;
        Picks::of(rows(spots), name, containers)
    });
    let mut table = SPOTS.get(thread);
    table.link(core_id(core), picks);
    SPOTS.set(thread, table);
}

/// sponge.spawn: the container chosen: true to take the artefact (EDX), false
/// for none this time (the core kept due when it declined).
pub fn spawn(ctx: &mut Context<'_>) -> bool {
    let taken = spawn_pick(ctx);
    if let Some(artefact) = taken {
        ctx.set_reg(Reg::Edx, artefact);
    }
    taken.is_some()
}

fn spawn_pick(ctx: &Context<'_>) -> Option<u32> {
    let thread = ctx.thread();
    // SAFETY: the site's EDI and frame.
    let (Some(core), Some(frame)) = (unsafe { raw::site::sponge_spawn::core(ctx) }, unsafe { raw::site::sponge_spawn::frame(ctx) })
    else {
        return None;
    };
    // SAFETY: the live core's pool and containers, its random state ours to draw.
    unsafe {
        let container = read(frame.container());
        let count = read(core.pool_count()) as usize;
        let pool = core.pool()?;
        let slot = |i: usize| {
            let artefact = pool.at(i).item();
            Slot {
                free: artefact.is_some_and(|a| read(a.artefact().state()) == 0),
                kind: if artefact.is_some_and(is_sponge) { spots::Kind::Sponge } else { spots::Kind::Other },
            }
        };
        let picks = SPOTS.get(thread).picks(core_id(core));
        let (begin, end) = (read(core.containers_begin()), read(core.containers_end()));
        let index = (0..end.saturating_sub(begin) / 4)
            .find(|&i| *((begin + 4 * i) as usize as *const u32) == container)
            .map(|i| i as usize);
        let (choice, random) = spots::spawn(count, slot, Allowed::of(picks, index), read(core.random()));
        *core.random() = random;
        match choice {
            Choice::Take(i) => pool.at(i).item().map(|a| a.address()),
            Choice::Declined => {
                *core.due() = 1;
                None
            }
            Choice::None => None,
        }
    }
}

