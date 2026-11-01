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

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::hook::Thread;
use crate::raw;

/// Reads a field's place (raw's accessor gives it).
///
/// # Safety
/// `at` is the field's place in a live object.
pub(in crate::vostok) unsafe fn read<T: Copy>(at: *mut T) -> T {
    unsafe { at.read_volatile() }
}

/// `N` f32 from a field on.
///
/// # Safety
/// As `read`, for the whole run.
pub(in crate::vostok) unsafe fn floats<const N: usize>(at: *mut f32) -> [f32; N] {
    unsafe { at.cast::<[f32; N]>().read_unaligned() }
}

/// Writes `N` f32 from a field on.
///
/// # Safety
/// As `floats`, and the run is the caller's to write now.
pub(in crate::vostok) unsafe fn write_floats<const N: usize>(at: *mut f32, value: [f32; N]) {
    unsafe { at.cast::<[f32; N]>().write_unaligned(value) }
}

/// The game world, at `game_world.after_move`.
pub struct GameWorld<'t> {
    pub(in crate::vostok) this: raw::GameWorld,
    thread: &'t Thread,
}

impl<'t> GameWorld<'t> {
    /// The world the site's code is ticking.
    pub fn at_after_move(ctx: &'t Context<'_>) -> Option<GameWorld<'t>> {
        // SAFETY: the context is this site's: the game world it ticks, alive
        // on the game's thread.
        let this = unsafe { raw::site::game_world_after_move::world(ctx) }?;
        Some(GameWorld { this, thread: ctx.thread() })
    }

    /// Its game's network client.
    pub fn network_client(&self) -> Option<NetworkClient<'t>> {
        // SAFETY: the world, its game and the game's client, alive for the hook.
        let this = unsafe { self.this.game()?.network_client() }?;
        Some(NetworkClient { this, thread: self.thread })
    }
}

/// The network client: the match's places and the local player.
pub struct NetworkClient<'t> {
    this: raw::NetworkClient,
    thread: &'t Thread,
}

impl<'t> NetworkClient<'t> {
    /// The client a site's typed register holds, on its hook's thread.
    pub(in crate::vostok) fn of(this: raw::NetworkClient, thread: &'t Thread) -> NetworkClient<'t> {
        NetworkClient { this, thread }
    }

    /// The match's places' count (0 without match options).
    pub fn places(&self) -> u32 {
        // SAFETY: the client, alive for the hook; match_options is its own.
        unsafe { raw::engine_match_options(Some(self.this)).map_or(0, |options| u32::from(read(options.places()))) }
    }

    /// The local player's place, if it has a player.
    pub fn local_place(&self) -> Option<u8> {
        // SAFETY: the client's local player, alive for the hook.
        unsafe { Some(read(self.this.local_player()?.place())) }
    }

    /// The place's current player. get_active_player adds a reference; it is
    /// given back at once (the place holds its own), as the game does when
    /// the last one goes (0x0E0210).
    pub fn player(&self, place: u32) -> Option<Player<'t>> {
        let mut out = None;
        // SAFETY: the client's own get_active_player (it writes the player, a
        // reference added, through `out`).
        unsafe { raw::engine_active_player(Some(self.this), &mut out, place) };
        let player = out?;
        // SAFETY: a player, its reference count a u32 the game changes with
        // locked instructions.
        unsafe {
            let count = player.references();
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(Some(player), count);
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

/// An inventory item (or an active object, a weapon) as a value kept between
/// hooks: its address, compared, never read through. Made only from its
/// handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemId(u32);

impl ItemId {
    pub(in crate::vostok) fn of(item: raw::Item) -> ItemId {
        ItemId(item.address())
    }

    /// Its address, for the log.
    pub fn address(self) -> u32 {
        self.0
    }
}

/// A match's world core as a value kept between hooks: which match a
/// record is of. Made only from its handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldKey(u32);

impl WorldKey {
    pub(in crate::vostok) fn of(core: raw::CoreWorld) -> WorldKey {
        WorldKey(core.address())
    }
}

/// A model's skeleton, kept between hooks (a cache's key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkeletonId(u32);

/// A body in the physics world (a hit model's), kept between hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyId(pub(in crate::vostok) u32);

/// A player of the match.
#[derive(Clone, Copy)]
pub struct Player<'t> {
    pub(in crate::vostok) this: raw::Player,
    pub(in crate::vostok) thread: &'t Thread,
}

impl<'t> Player<'t> {
    pub fn eye_transform(&self) -> [f32; 16] {
        // SAFETY: the live player's complete computed eye matrix.
        unsafe { floats(self.this.eye()) }
    }

    pub fn inventory(&self) -> Option<super::inventory::Inventory<'t>> {
        super::inventory::Inventory::of_player(self.this, self.thread)
    }
    pub fn core_skeleton(&self) -> Option<Skeleton<'t>> {
        // SAFETY: the live core player retains its simulation skeleton.
        Some(Skeleton { this: unsafe { self.this.core_skeleton() }?, thread: PhantomData })
    }

    /// The player a site's typed register holds (raw's `site`), on its hook's thread.
    pub(in crate::vostok) fn of(ctx: &'t Context<'_>, this: Option<raw::Player>) -> Option<Player<'t>> {
        this.map(|this| Player { this, thread: ctx.thread() })
    }

    pub fn id(&self) -> PlayerId {
        PlayerId(self.this.address())
    }

    /// The player's place in the match roster.
    pub fn place(&self) -> u8 {
        // SAFETY: the live base player's place byte.
        unsafe { read(self.this.place()) }
    }

    /// In the match's world (m_has_been_inserted).
    pub fn inserted(&self) -> bool {
        // SAFETY: the player is alive for the hook.
        unsafe { read(self.this.inserted()) != 0 }
    }

    /// Alive (m_is_alive): a rollback or a loaded world sets it again.
    pub fn alive(&self) -> bool {
        // SAFETY: as inserted.
        unsafe { read(self.this.alive()) != 0 }
    }

    /// The life's spawn time (m_inserted_time_in_ms): one death a life.
    pub fn spawned(&self) -> u32 {
        // SAFETY: as inserted.
        unsafe { read(self.this.spawned()) }
    }

    /// What it holds (+128h: mt_draw draws it at the "Weapon" bone).
    pub fn held(&self) -> Held {
        // SAFETY: as inserted; the held item is alive as long.
        unsafe {
            match self.this.held_item() {
                None => Held::Nothing,
                Some(item) if read(item.vtable()) == raw::image::VICTORY_ITEM_VTABLE => Held::Battery,
                Some(_) => Held::Item,
            }
        }
    }

    /// The scene renderer and the model's render instance, what
    /// update_skeleton takes (mt_draw 0x2477E2..0x24780A).
    pub(in crate::vostok) fn renderer(&self) -> Option<(raw::SceneRenderer, raw::RenderModelInstance)> {
        // SAFETY: the player's model and game world, the world's game, its
        // renderer, the renderer's scene renderer: alive for the hook.
        unsafe {
            let model = self.this.model()?;
            let scene = self.this.game_world()?.game()?.renderer()?.scene_renderer()?;
            Some((scene, model.render_instance()))
        }
    }

    /// The simulated transform, a row-vector 4x4 (rows i, j, k, place).
    pub fn transform(&self) -> [f32; 16] {
        // SAFETY: the player is alive for the hook; 16 f32 from its field on.
        unsafe { floats(self.this.transform()) }
    }

    /// The camera's transform's address (what update_camera gives the view
    /// in the first person): compared with what the site was given.
    pub(in crate::vostok) fn eye_address(&self) -> u32 {
        // SAFETY: an address only.
        unsafe { self.this.eye() as usize as u32 }
    }

    /// The physics world (the level's ray).
    pub fn physics_world(&self) -> Option<PhysicsWorld<'t>> {
        // SAFETY: as inserted.
        let this = unsafe { self.this.physics_world() }?;
        Some(PhysicsWorld { this, thread: self.thread })
    }

    /// Its model's skeleton.
    pub fn skeleton(&self) -> Option<Skeleton<'t>> {
        // SAFETY: the player's model, the model's skeleton: alive for the hook.
        let this = unsafe { self.this.model()?.skeleton() }?;
        Some(Skeleton { this, thread: PhantomData })
    }

    /// Its hit model's body in the physics world (what a bullet's ray names).
    pub fn hit_body(&self) -> Option<BodyId> {
        // SAFETY: the player's hit model, its body: alive for the hook.
        unsafe { self.this.hit_model()?.body() }.map(|body| BodyId(body.address()))
    }

    /// What each inventory slot's item weighs, kg (a piece's weight times a
    /// stack's count, as inventory::update_weight sums it; None for an empty
    /// slot), the slots in the game's order (profile slots 0..6 helmet, mask,
    /// torso, back, legs, gloves, boots, then the weapons and the rest), to
    /// `f`; nothing without an inventory.
    pub fn slot_weights(&self, mut f: impl FnMut(usize, Option<f32>)) {
        // SAFETY: the player's inventory, alive for the hook; its slots' run.
        let Some((first, slots)) = (unsafe {
            self.this.inventory().map(|inventory| (inventory.slots(), inventory.slots_end()))
        }) else {
            return;
        };
        for slot in 0..slots.index_from(first).unwrap_or(0) {
            // SAFETY: a slot of the run (below its end).
            f(slot, unsafe { item_weight(first.at(slot)) });
        }
    }
}

/// The weight of the item a slot holds.
///
/// # Safety
/// `slot` is a live inventory's slot.
unsafe fn item_weight(slot: raw::InventorySlot) -> Option<f32> {
    // SAFETY: the slot's item, its fields, its entry's.
    unsafe {
        let item = slot.item()?;
        let piece = read(item.weight());
        let stack = item.entry().is_some_and(|entry| read(entry.stack()) != 0);
        let count = if stack { f32::from(read(item.count())) } else { 1.0 };
        Some(piece * count)
    }
}

/// A model's skeleton: its bones, object_movement first, each its name and
/// its parent (add_bone 0x3ACDA0's records).
pub struct Skeleton<'t> {
    this: raw::Skeleton,
    thread: PhantomData<&'t Thread>,
}

impl Skeleton<'_> {
    pub fn bone_computed(&self, index: u32, layer: u8) -> bool {
        // SAFETY: bounded native skeleton record.
        self.bone(index).is_some_and(|bone| unsafe { read(bone.computed_layers()) & layer != 0 })
    }

    pub fn id(&self) -> SkeletonId {
        SkeletonId(self.this.address())
    }

    /// Its bones' count.
    pub fn bone_count(&self) -> u32 {
        // SAFETY: the skeleton is alive for the hook.
        unsafe { read(self.this.bone_count()) }
    }

    /// The `index`th bone's record (within the count).
    fn bone(&self, index: u32) -> Option<raw::SkeletonBone> {
        // SAFETY: a record of the run (below the count).
        (index < self.bone_count()).then(|| unsafe { self.this.bones().at(index as usize) })
    }

    /// Whether the `index`th bone is named `name` (false past the count or
    /// for a bone with no name).
    pub fn bone_named(&self, index: u32, name: &str) -> bool {
        let Some(bone) = self.bone(index) else { return false };
        // SAFETY: the record's name, a C string: compared up to its 0.
        unsafe {
            let text = bone.name();
            if text.is_null() {
                return false;
            }
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
        // SAFETY: the record is the live skeleton's.
        unsafe { !bone.name().is_null() }
    }

    /// The `index`th bone's parent's index (the record it names; the first
    /// record is index 0).
    pub fn bone_parent(&self, index: u32) -> Option<u32> {
        let bone = self.bone(index)?;
        // SAFETY: the record's parent field, the skeleton's run.
        let (parent, first) = unsafe { (bone.parent()?, self.this.bones()) };
        parent.index_from(first).map(|i| i as u32)
    }
}

/// The physics world (the simulation's): read-only queries.
pub struct PhysicsWorld<'t> {
    this: raw::BulletPhysicsWorld,
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
        let mut room = raw::RayResultStorage::zeroed();
        let result = room.handle();
        // SAFETY: the physics world, alive for the hook; the result is ours;
        // the origin and direction float3s are ours.
        unsafe {
            raw::engine_ray_test(Some(self.this), Some(result), origin.as_ptr(), direction.as_ptr(), length, group,
                                 mask, core::ptr::null(), 0);
            result.object()?;
            Some(RayHit { point: floats(result.point()), normal: floats(result.normal()) })
        }
    }
}

impl BodyId {
    /// The body as a value (a shot's key).
    pub fn key(self) -> u32 {
        self.0
    }
}
