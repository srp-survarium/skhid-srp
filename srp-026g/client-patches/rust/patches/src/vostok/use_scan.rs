//! What a use scan reads of the game (base_player::detect_usable_object,
//! 0x2D84C0; patch 23's rule is the patch use_scan's): the player's view
//! ray and its hits, each hit's collision group and usable object, a usable
//! object's can_use and, for a battery, its centre. Every native step is the
//! one detect_usable_object takes, in its order: the physics world's
//! ray_query (vtable+4Ch), the hit object's get_collision_group (+0), its
//! user data's cast_to_collision_geometry (+4), query_objects_by_type
//! (0x2DC390) with the cast 0x2DD730, the usable object's can_use (+30h) and
//! get_transform (+44h); both vectors with the multithreaded allocator,
//! freed by it. One step is earlier than the game's: a hit's usable list is
//! freed as soon as its first object is read, before that object's
//! get_transform and can_use (the game frees it after, 0x2D85FC); those only
//! read, so nothing sees the difference.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::world::{floats, read, write_floats, Player};
use crate::raw;

/// detect_usable_object's ray filter (0x2D84EF, 0x2D84F4): the groups and
/// the mask a hit must have.
const GROUP: u32 = 0x114;
const MASK: u32 = 0x8A;
/// The two frees' line (std_allocator_inline.h, 67h).
const FREE_LINE: u32 = 0x67;

/// The player at a use scan's start (`use.detect`, 0x2D84C0) and the point
/// it asks for. [`Self::finish`] gives the scan's answer back.
pub struct UseScanCall<'c, 'm> {
    ctx: &'c mut Context<'m>,
    player: raw::Player,
    /// Where the caller wants the point (None: it passed 0, as the HUD's
    /// st_draw and 0x2D86A0 do).
    point: Option<NonNull<f32>>,
}

impl<'c, 'm> UseScanCall<'c, 'm> {
    pub fn at(ctx: &'c mut Context<'m>) -> Option<UseScanCall<'c, 'm>> {
        use raw::site::use_detect as site;
        // SAFETY: the context is this site's: its ESP the function's
        // arguments (the player, the point or 0), alive while it runs.
        let (player, point) = unsafe {
            let frame = site::frame(&*ctx)?;
            (frame.player()?, NonNull::new(frame.point()))
        };
        Some(UseScanCall { ctx, player, point })
    }

    pub fn player(&self) -> Player<'_> {
        Player { this: self.player, thread: self.ctx.thread() }
    }

    /// The scan's answer, as detect_usable_object gives it: EAX the object
    /// (0 for none) and, with one, its point written where asked. Only a
    /// usable object the scan met can be the answer. The caller leaves by
    /// the site's `returned` exit.
    pub fn finish(self, found: Option<(Taken, [f32; 3])>) {
        if let (Some((_, at)), Some(point)) = (found, self.point) {
            // SAFETY: the caller's float3 (apply_new_input's frame).
            unsafe { write_floats(point.as_ptr(), at) };
        }
        self.ctx.set_reg(Reg::Eax, found.map_or(0, |(Taken(object), _)| object.address()));
    }
}

/// What a use scan needs of its player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseView {
    /// The eye (+47Ch): where the ray starts.
    pub eye: [f32; 3],
    /// The feet (+43Ch).
    pub feet: [f32; 3],
}

impl<'t> Player<'t> {
    /// How far its use reaches: usable_objects_detection_distance (2.0 in
    /// the image; a use's tick checks it too, 0x300830).
    pub fn use_distance(&self) -> f32 {
        // SAFETY: the image's float, read on the game's thread.
        unsafe { read(raw::image::usable_distance().value()) }
    }

    /// Its eye and feet for a use scan; None when it is dead (m_is_alive,
    /// +33Ch: detect_usable_object gives 0 at once, 0x2D84CC).
    pub fn use_view(&self) -> Option<UseView> {
        if !self.alive() {
            return None;
        }
        let eye = self.eye_transform();
        // SAFETY: the live player's feet, a float3.
        let feet = unsafe { floats(self.this.feet()) };
        Some(UseView { eye: [eye[12], eye[13], eye[14]], feet })
    }

    /// Every hit of its view ray, `length` long, nearest first (ray_query
    /// sorts them): from its eye (+47Ch) along its view (+46Ch), with the
    /// use scan's filter. None without a physics world.
    pub fn use_ray(&self, length: f32) -> Option<UseHits<'t>> {
        // SAFETY: the live player's physics world and its eye matrix; the
        // vector is ours, set up as detect_usable_object sets its own
        // (0x2D850F..0x2D851F), the world appends to it.
        unsafe {
            let world = self.this.physics_world()?;
            let mut room = raw::RayHitsStorage::zeroed();
            let hits = room.handle();
            hits.set_allocator(Some(raw::image::mt_allocator()));
            let eye = self.this.eye();
            raw::engine_ray_query(Some(world), eye.add(12), eye.add(8), length, Some(hits), GROUP, MASK);
            Some(UseHits { room, thread: PhantomData })
        }
    }

    /// The user data a usable object's can_use gets (+130h).
    fn use_user(&self) -> raw::UsableUser {
        // SAFETY: an object inside the live player.
        unsafe { self.this.use_user() }
    }
}

/// A use scan's hits (a buffer_vector the world filled), freed when dropped
/// as detect_usable_object frees its own (0x2D866D).
pub struct UseHits<'t> {
    room: raw::RayHitsStorage,
    thread: PhantomData<&'t Thread>,
}

impl<'t> UseHits<'t> {
    /// Its hits in their order; one without an object (none: ray_query
    /// names every hit's) ends them.
    pub fn iter(&mut self) -> impl Iterator<Item = UseHit<'t>> + '_ {
        let hits = self.room.handle();
        // SAFETY: the vector the world filled, alive while self is.
        let (begin, end) = unsafe { (hits.begin(), hits.end()) };
        let count = match (begin, end) {
            (Some(begin), Some(end)) => end.index_from(begin).unwrap_or(0),
            _ => 0,
        };
        (0..count).map_while(move |i| {
            // SAFETY: the i-th of `count` hits of the vector.
            let hit = unsafe { begin?.at(i) };
            // SAFETY: as above; the object a live physics object.
            let (object, point) = unsafe { (hit.object()?, floats(hit.point())) };
            Some(UseHit { object, point, thread: PhantomData })
        })
    }
}

impl Drop for UseHits<'_> {
    fn drop(&mut self) {
        let hits = self.room.handle();
        // SAFETY: the vector's own block, from its allocator (ours).
        unsafe {
            if let (Some(begin), Some(allocator)) = (hits.begin(), hits.allocator()) {
                raw::engine_mt_free(Some(allocator), begin.address() as usize as *mut c_void,
                                    raw::image::HITS_DEALLOCATE as usize as *const u8,
                                    raw::image::STD_ALLOCATOR_FILE as usize as *const u8, FREE_LINE);
            }
        }
    }
}

/// One hit of a use scan's ray.
#[derive(Clone, Copy)]
pub struct UseHit<'t> {
    object: raw::HitObject,
    point: [f32; 3],
    thread: PhantomData<&'t Thread>,
}

impl<'t> UseHit<'t> {
    pub fn point(&self) -> [f32; 3] {
        self.point
    }

    /// Its object's collision group (get_collision_group: AX, widened by the thunk).
    pub fn group(&self) -> u16 {
        // SAFETY: a live physics object (its vtable's slot 0).
        unsafe { raw::engine_collision_group(Some(self.object)) }
    }

    /// The usable object it belongs to: the first of its collision
    /// geometry's subscribers that is one (query_objects_by_type with the
    /// cast to usable, as 0x2D8584..0x2D85BC). None when it has no game
    /// object, is no collision geometry or has none (where the game would
    /// read a null).
    pub fn usable(&self) -> Option<Usable<'t>> {
        // SAFETY: the hit's live object and its user data; the list is
        // ours, set up as detect_usable_object's (0x2D859F..0x2D85AF), and
        // freed as it frees it (0x2D85FC).
        unsafe {
            let geometry = raw::engine_collision_geometry(Some(self.object.user()?))?;
            let mut room = raw::UsableListStorage::zeroed();
            let list = room.handle();
            list.set_allocator(Some(raw::image::mt_allocator()));
            raw::engine_query_usables(Some(list), Some(geometry), raw::image::USABLE_CAST as usize as *const c_void, 0);
            let (begin, end) = (list.begin(), list.end());
            let first = begin.filter(|&b| end != Some(b)).and_then(|b| b.object());
            if let (Some(begin), Some(allocator)) = (begin, list.allocator()) {
                raw::engine_mt_free(Some(allocator), begin.address() as usize as *mut c_void,
                                    raw::image::USABLES_DEALLOCATE as usize as *const u8,
                                    raw::image::STD_ALLOCATOR_FILE as usize as *const u8, FREE_LINE);
            }
            Some(Usable { this: first?, thread: PhantomData })
        }
    }
}

/// A use scan's answer for [`UseScanCall::finish`]: only a [`Usable`] makes one
/// ([`Usable::taken`]), so nothing else reaches the game's EAX.
#[derive(Clone, Copy)]
pub struct Taken(raw::UsableObject);

/// A usable object a use scan met (a battery, a container, a crate, a trap).
#[derive(Clone, Copy)]
pub struct Usable<'t> {
    this: raw::UsableObject,
    /// Alive while the hook that met it runs.
    thread: PhantomData<&'t Thread>,
}

impl Usable<'_> {
    /// It as the scan's answer.
    pub fn taken(&self) -> Taken {
        Taken(self.this)
    }

    /// Its can_use for `player` (vtable+30h, the player's user data).
    pub fn can_use(&self, player: &Player<'_>) -> bool {
        // SAFETY: a live usable object; the player's user data.
        unsafe { raw::engine_can_use(Some(self.this), Some(player.use_user())) != 0 }
    }

    /// Its centre when it is a battery: victory_item_core's or victory_item's
    /// usable vtable (the only two whose can_use is victory_item_core's),
    /// its get_transform's (vtable+44h) translation. None for anything else.
    pub fn battery_centre(&self) -> Option<[f32; 3]> {
        // SAFETY: a live usable object's vtable; then its get_transform
        // into our matrix.
        unsafe {
            let vtable = read(self.this.vtable());
            if vtable != raw::image::VICTORY_ITEM_CORE_USABLE_VTABLE && vtable != raw::image::VICTORY_ITEM_USABLE_VTABLE {
                return None;
            }
            let mut matrix = [0f32; 16];
            raw::engine_usable_transform(Some(self.this), matrix.as_mut_ptr());
            Some([matrix[12], matrix[13], matrix[14]])
        }
    }

    /// The client's battery it is the usable object of, None for anything else.
    pub fn battery(&self) -> Option<super::battery::BatteryId> {
        // SAFETY: a live usable object's vtable; only the client battery's
        // usable has that one, and is 14h into its battery.
        unsafe {
            (read(self.this.vtable()) == raw::image::VICTORY_ITEM_USABLE_VTABLE)
                .then(|| super::battery::BatteryId::of(self.this.victory_item()))
        }
    }
}
