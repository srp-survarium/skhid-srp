//! The match's world as the ragdoll reads it (patch 76): the game world, the
//! network client and its places, a place's player (its state, transform,
//! model's skeleton, inventory, hit model), the physics world's ray. Every
//! field is fields_026g.tsv's, every function engine_026g.tsv's; this is
//! only what their values mean.
//!
//! Each handle borrows the hook's [`Thread`] (a patch cannot keep one past
//! the hook); what a patch keeps between hooks is an id ([`PlayerId`],
//! [`SkeletonId`], [`BodyId`]): a game address as a value, never read
//! through.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::{Context, Reg};
use super::hook::Thread;
use crate::raw;

/// A game object's address, never null.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vostok) struct Object(NonNull<c_void>);

impl Object {
    pub(in crate::vostok) fn at(raw: u32) -> Option<Object> {
        NonNull::new(raw as usize as *mut c_void).map(Object)
    }

    pub(in crate::vostok) fn ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }

    pub(in crate::vostok) fn id(self) -> u32 {
        self.0.as_ptr() as usize as u32
    }
}

/// Reads a field (its address from `raw`).
///
/// # Safety
/// `at` is the field's address in a live object of its kind.
pub(in crate::vostok) unsafe fn read<T: Copy>(at: *mut T) -> T {
    unsafe { at.read_volatile() }
}

/// An object a pointer field names (0: none).
///
/// # Safety
/// As `read`.
pub(in crate::vostok) unsafe fn pointer(at: *mut u32) -> Option<Object> {
    Object::at(unsafe { read(at) })
}

/// `count` f32 from a field on.
///
/// # Safety
/// As `read`, for the whole run.
pub(in crate::vostok) unsafe fn floats<const N: usize>(at: *mut f32) -> [f32; N] {
    unsafe { at.cast::<[f32; N]>().read_unaligned() }
}

/// The game world, at `game_world.after_move` (EBX, sites_026g.tsv).
pub struct GameWorld<'t> {
    this: Object,
    thread: &'t Thread,
}

impl<'t> GameWorld<'t> {
    /// The world the site's EBX holds.
    pub fn at_after_move(ctx: &'t Context<'_>) -> Option<GameWorld<'t>> {
        Object::at(ctx.reg(Reg::Ebx)).map(|this| GameWorld { this, thread: ctx.thread() })
    }

    /// Its game's network client.
    pub fn network_client(&self) -> Option<NetworkClient<'t>> {
        // SAFETY: the game world (the site's register), its game, the game's client.
        let client = unsafe {
            let game = pointer(raw::game_world::game(self.this.ptr()).cast())?;
            pointer(raw::game::network_client(game.ptr()).cast())?
        };
        Some(NetworkClient { this: client, thread: self.thread })
    }
}

/// The network client: the match's places and the local player.
pub struct NetworkClient<'t> {
    this: Object,
    thread: &'t Thread,
}

impl<'t> NetworkClient<'t> {
    /// The match's places' count (0 without match options).
    pub fn places(&self) -> u32 {
        // SAFETY: the network client; match_options is its own (thiscall,
        // the re-kit's), the options' count a byte field.
        unsafe {
            let options = raw::engine_match_options(self.this.ptr());
            if options.is_null() { 0 } else { u32::from(read(raw::match_options::places(options))) }
        }
    }

    /// The local player's place, if it has a player.
    pub fn local_place(&self) -> Option<u8> {
        // SAFETY: the client's local player field, the player's place.
        unsafe {
            let local = pointer(raw::network_client::local_player(self.this.ptr()).cast())?;
            Some(read(raw::player::place(local.ptr())))
        }
    }

    /// The place's current player. get_active_player adds a reference; it is
    /// given back at once (the place holds its own), as the game does when
    /// the last one goes (0x0E0210).
    pub fn player(&self, place: u32) -> Option<Player<'t>> {
        let mut out = 0u32;
        // SAFETY: the client's own get_active_player (thiscall; it writes the
        // player, a reference added, through `out`).
        unsafe { raw::engine_active_player(self.this.ptr(), &mut out, place) };
        let player = Object::at(out)?;
        // SAFETY: a player, its reference count a u32 the game changes with
        // locked instructions.
        unsafe {
            let count = raw::player::references(player.ptr());
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(player.ptr(), count);
                return None;
            }
        }
        Some(Player { this: player, thread: self.thread })
    }
}

/// What a player holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Nothing,
    /// A battery (a victory_item): no gun drops from a carrier's hands.
    Battery,
    /// A weapon or another item.
    Item,
}

/// A player as a value kept between hooks: its address, compared, never
/// read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerId(u32);

/// A model's skeleton, kept between hooks (a cache's key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkeletonId(u32);

/// A body in the physics world (a hit model's), kept between hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyId(pub(in crate::vostok) u32);

/// A player of the match.
#[derive(Clone, Copy)]
pub struct Player<'t> {
    this: Object,
    thread: &'t Thread,
}

impl<'t> Player<'t> {
    pub fn inventory(&self) -> Option<super::inventory::Inventory<'t>> {
        super::inventory::Inventory::of_player(self.this, self.thread)
    }

    /// The player a site's register names (`reg`, as sites_026g.tsv says).
    pub(in crate::vostok) fn at(ctx: &'t Context<'_>, reg: Reg) -> Option<Player<'t>> {
        Object::at(ctx.reg(reg)).map(|this| Player { this, thread: ctx.thread() })
    }

    pub fn id(&self) -> PlayerId {
        PlayerId(self.this.id())
    }

    /// In the match's world (m_has_been_inserted).
    pub fn inserted(&self) -> bool {
        // SAFETY: a player's byte field.
        unsafe { read(raw::player::inserted(self.this.ptr())) != 0 }
    }

    /// Alive (m_is_alive): a rollback or a loaded world sets it again.
    pub fn alive(&self) -> bool {
        // SAFETY: as inserted.
        unsafe { read(raw::player::alive(self.this.ptr())) != 0 }
    }

    /// The life's spawn time (m_inserted_time_in_ms): one death a life.
    pub fn spawned(&self) -> u32 {
        // SAFETY: as inserted.
        unsafe { read(raw::player::spawned(self.this.ptr())) }
    }

    /// What it holds (+128h: mt_draw draws it at the "Weapon" bone).
    pub fn held(&self) -> Held {
        // SAFETY: as inserted; the held item's vtable, a read of its first
        // field.
        unsafe {
            match pointer(raw::player::held_item(self.this.ptr()).cast()) {
                None => Held::Nothing,
                Some(item) if read(item.ptr().cast::<u32>()) == raw::image::VICTORY_ITEM_VTABLE => Held::Battery,
                Some(_) => Held::Item,
            }
        }
    }

    /// The model's render instance and the scene renderer, what
    /// update_skeleton takes (mt_draw 0x2477E2..0x24780A).
    pub(in crate::vostok) fn renderer(&self) -> Option<(*mut c_void, *mut c_void)> {
        // SAFETY: the player's model and game world, the world's game, its
        // renderer, the renderer's scene renderer.
        unsafe {
            let model = pointer(raw::player::model(self.this.ptr()).cast())?;
            let world = pointer(raw::player::game_world(self.this.ptr()).cast())?;
            let game = pointer(raw::game_world::game(world.ptr()).cast())?;
            let renderer = pointer(raw::game::renderer(game.ptr()).cast())?;
            let scene = pointer(raw::renderer::scene_renderer(renderer.ptr()).cast())?;
            Some((scene.ptr(), raw::player_model::render_instance(model.ptr()).cast()))
        }
    }

    /// The simulated transform, a row-vector 4x4 (rows i, j, k, place).
    pub fn transform(&self) -> [f32; 16] {
        // SAFETY: the transform's 16 f32 from its field on.
        unsafe { floats(raw::player::transform(self.this.ptr())) }
    }

    /// The camera's transform's address (what update_camera gives the view
    /// in the first person): compared with what the site was given.
    pub(in crate::vostok) fn eye_address(&self) -> u32 {
        // SAFETY: an address only.
        unsafe { raw::player::eye(self.this.ptr()) as usize as u32 }
    }

    /// The physics world (the level's ray).
    pub fn physics_world(&self) -> Option<PhysicsWorld<'t>> {
        // SAFETY: as inserted.
        let world = unsafe { pointer(raw::player::physics_world(self.this.ptr()).cast())? };
        Some(PhysicsWorld { this: world, thread: self.thread })
    }

    /// Its model's skeleton.
    pub fn skeleton(&self) -> Option<Skeleton<'t>> {
        // SAFETY: the player's model, the model's skeleton.
        let skeleton = unsafe {
            let model = pointer(raw::player::model(self.this.ptr()).cast())?;
            pointer(raw::player_model::skeleton(model.ptr()).cast())?
        };
        Some(Skeleton { this: skeleton, thread: PhantomData })
    }

    /// Its hit model's body in the physics world (what a bullet's ray names).
    pub fn hit_body(&self) -> Option<BodyId> {
        // SAFETY: the player's hit model, its body.
        unsafe {
            let model = pointer(raw::player::hit_model(self.this.ptr()).cast())?;
            pointer(raw::hit_model::body(model.ptr()).cast()).map(|b| BodyId(b.id()))
        }
    }

    /// What each inventory slot's item weighs, kg (a piece's weight times a
    /// stack's count, as inventory::update_weight sums it; None for an empty
    /// slot), the slots in the game's order (profile slots 0..6 helmet, mask,
    /// torso, back, legs, gloves, boots, then the weapons and the rest), to
    /// `f`; nothing without an inventory.
    pub fn slot_weights(&self, mut f: impl FnMut(usize, Option<f32>)) {
        // SAFETY: the player's inventory field.
        let Some(inventory) = (unsafe { pointer(raw::player::inventory(self.this.ptr()).cast()) }) else { return };
        // SAFETY: addresses only: the slots' run and its end in the inventory.
        let (first, end) = unsafe {
            (raw::inventory::slots(inventory.ptr()), raw::inventory::slots_end(inventory.ptr()))
        };
        let slots = (end as usize - first as usize) / 4;
        for slot in 0..slots {
            // SAFETY: a slot of the run (below its end).
            f(slot, unsafe { item_weight(first.cast::<u32>().add(slot)) });
        }
    }
}

/// The weight of the item a slot holds.
///
/// # Safety
/// `slot` is an inventory slot.
unsafe fn item_weight(slot: *mut u32) -> Option<f32> {
    // SAFETY: the slot's item, its fields, its entry's.
    unsafe {
        let item = pointer(slot)?;
        let piece = read(raw::item::weight(item.ptr()));
        let stack = pointer(raw::item::entry(item.ptr()).cast())
            .is_some_and(|entry| read(raw::item_entry::stack(entry.ptr())) != 0);
        let count = if stack { f32::from(read(raw::item::count(item.ptr()))) } else { 1.0 };
        Some(piece * count)
    }
}

/// A model's skeleton: its bones, object_movement first, each its name and
/// its parent (add_bone 0x3ACDA0's records).
pub struct Skeleton<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl Skeleton<'_> {
    pub fn id(&self) -> SkeletonId {
        SkeletonId(self.this.id())
    }

    /// Its bones' count.
    pub fn bone_count(&self) -> u32 {
        // SAFETY: the skeleton's count field.
        unsafe { read(raw::skeleton::bone_count(self.this.ptr())) }
    }

    /// The bones' first record and a record's size.
    fn records(&self) -> (*mut c_void, usize) {
        // SAFETY: addresses only: the bones' run in the skeleton, a record's
        // next one (its size).
        unsafe {
            let first = raw::skeleton::bones(self.this.ptr()).cast::<c_void>();
            (first, raw::skeleton_bone::next_bone(first) as usize - first as usize)
        }
    }

    /// The `index`th bone's record (within the count).
    fn bone(&self, index: u32) -> Option<*mut c_void> {
        let (first, size) = self.records();
        // SAFETY: a record of the run (below the count).
        (index < self.bone_count()).then(|| unsafe { first.byte_add(size * index as usize) })
    }

    /// Whether the `index`th bone is named `name` (false past the count or
    /// for a bone with no name).
    pub fn bone_named(&self, index: u32, name: &str) -> bool {
        let Some(bone) = self.bone(index) else { return false };
        // SAFETY: the record's name, a C string: compared up to its 0.
        unsafe {
            let Some(text) = pointer(raw::skeleton_bone::name(bone).cast()) else { return false };
            let text = text.ptr().cast::<u8>();
            for (n, &b) in name.as_bytes().iter().enumerate() {
                if text.add(n).read_volatile() != b {
                    return false;
                }
            }
            text.add(name.len()).read_volatile() == 0
        }
    }

    /// Whether the `index`th bone has a name at all.
    pub fn bone_has_name(&self, index: u32) -> bool {
        let Some(bone) = self.bone(index) else { return false };
        // SAFETY: the record's name field.
        unsafe { read(raw::skeleton_bone::name(bone)) != 0 }
    }

    /// The `index`th bone's parent's index (the record it names; the first
    /// record is index 0).
    pub fn bone_parent(&self, index: u32) -> Option<u32> {
        let bone = self.bone(index)?;
        let (first, size) = self.records();
        // SAFETY: the record's parent field.
        let parent = unsafe { read(raw::skeleton_bone::parent(bone)) } as usize;
        let offset = parent.checked_sub(first as usize)?;
        (offset % size == 0).then_some((offset / size) as u32)
    }
}

/// The physics world (the simulation's): read-only queries.
pub struct PhysicsWorld<'t> {
    this: Object,
    thread: &'t Thread,
}

/// A ray's first hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub point: [f32; 3],
    /// Toward the ray's start.
    pub normal: [f32; 3],
}

impl PhysicsWorld<'_> {
    /// bullet_physics_world::ray_test from `origin` along the unit
    /// `direction`, `length` long, with the filter `group`/`mask` (an object
    /// is tried when its mask has a bit of the group and its group a bit of
    /// the mask): the first hit. It changes nothing (Bullet's scratch only).
    pub fn ray_test(&self, origin: [f32; 3], direction: [f32; 3], length: f32, group: u32, mask: u32) -> Option<RayHit> {
        let _ = self.thread;
        // the result: 28h bytes (ray_result's end)
        let mut result = [0u32; 10];
        // SAFETY: the physics world; the result is ours, its size the
        // result's; the origin and direction float3s are ours.
        unsafe {
            raw::engine_ray_test(self.this.ptr(), result.as_mut_ptr().cast(), origin.as_ptr(), direction.as_ptr(),
                                 length, group, mask, core::ptr::null(), 0);
            let at = result.as_mut_ptr().cast::<c_void>();
            if read(raw::ray_result::object(at)) == 0 {
                return None;
            }
            Some(RayHit { point: floats(raw::ray_result::point(at)), normal: floats(raw::ray_result::normal(at)) })
        }
    }
}

const _: () = assert!(size_of::<[u32; 10]>() == 0x28);

impl BodyId {
    /// The body as a value (a shot's key).
    pub fn key(self) -> u32 {
        self.0
    }
}
