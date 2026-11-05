//! A bullet's ray at `bullet.ray_traced` as patch 103 tests it
//! (docs/notes/subtick_bullet_hits.md): the game's own ray test has run
//! (bullet::check_collision 0x32D4E0 called bullet_physics_world::ray_test
//! 0x75E20 for the level and every living hit model at the step's end), and
//! its result is not yet read. The patch may trace the level again and test
//! the hit models one by one, each where it stood when the bullet crossed it,
//! and the result it leaves is what check_collision then reads (the damage,
//! the pierce, the decals, the other hooks at the site).
//!
//! Everything here is the game's own code on the game's own objects: the ray
//! is made as ray_test makes it (its Bullet frame: z negated, -0 made 0), a
//! hit model is tested by Bullet's rayTestSingle (0xA02D0) through the
//! game's closest-ray callback (0x75B40: its filter and its hit model
//! delegate, which poses the bones at the step's time as ray_test's second
//! pass does), and a hit is written into the result as ray_test writes it.
//! Nothing of the physics world is changed: a hit model moved back is tested
//! at a copy of its transform, the body itself stays where it is.
//!
//! Every field is fields_026g.tsv's, every function engine_026g.tsv's.

use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::hook::Thread;
use super::types::{GameTime, Place, Tick};
use super::world::{BodyId, floats, read, write_floats};
use crate::raw;

/// check_collision's filter (0x32D5F1, 0x32D5F3): a bullet's ray is group
/// 10h and meets the hittable level (8) and the hit models (40h).
const BULLET_GROUP: u32 = 0x10;
const BULLET_MASK: u32 = 0x48;
/// The level alone: every object a bullet meets but the hit models.
const LEVEL_MASK: u32 = 0x08;
/// ray_test's last argument as check_collision passes it (1): m_flags bit 2,
/// the unflipped normal (0x75F2E).
const UNFLIPPED: u32 = 0x2;
/// The callback's cutoff before any hit (the ctor's 1.0).
const NO_HIT: f32 = 1.0;
/// The material a living player's hit model is to bullets (actor), as
/// vostok::bullet reads it.
const ACTOR: u16 = 10;

/// A float of the game's frame as ray_test hands it to Bullet: -0 is 0 (its
/// `ucomiss` with 0 and the `jp`: equal and ordered gives 0).
fn bullet_float(value: f32) -> f32 {
    if value == 0.0 { 0.0 } else { value }
}

/// A point of the game's frame in Bullet's (z negated), as ray_test makes
/// its ends: a float4 with w 0.
fn to_bullet(point: [f32; 3]) -> [f32; 4] {
    [bullet_float(point[0]), bullet_float(point[1]), bullet_float(-point[2]), 0.0]
}

/// A Bullet vector in the game's frame, as ray_test writes its result.
fn from_bullet(point: [f32; 4]) -> [f32; 3] {
    [bullet_float(point[0]), bullet_float(point[1]), bullet_float(-point[2])]
}

/// Bullet's objects keep their vectors 16-byte aligned.
#[repr(C, align(16))]
struct Aligned<T>(T);

/// The bullet's flight, in seconds since its segment's origin (bullet::tick
/// 0x32C180: its spawn, or its last pierce or ricochet): the ray spans
/// `start..end`, and the world's step ends at `step_end` (end <= step_end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub start: f32,
    pub end: f32,
    pub step_end: f32,
}

/// The world's step being ticked (atomic_tick 0x2D1E20): from the time the
/// world was at (the core's time, written after the step's delegates) to the
/// time it reaches, more than 0 and at most a tick later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    from: GameTime,
    ms: u32,
}

impl Step {
    /// A step from `from` to `to`; none when `to` is not 1..=50 ms later.
    pub fn new(from: GameTime, to: GameTime) -> Option<Step> {
        let ms = to.since(from);
        (1..=Tick::MS).contains(&ms).then_some(Step { from, ms })
    }

    /// The time the world was at before the step.
    pub fn from(self) -> GameTime {
        self.from
    }

    /// Its length, 1..=50 ms.
    pub fn ms(self) -> u32 {
        self.ms
    }
}

/// What the bullet's ray hit, as its result says now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    Nothing,
    /// The level, or anything else that is not a living player's hit model.
    Level,
    Body(BodyId),
}

/// A living player's hit model in the physics world: what a bullet's ray
/// may meet.
pub struct Body<'t> {
    player: raw::Player,
    rigid: raw::BtAnimatedRigidBody,
    object: raw::BtCollisionObject,
    proxy: raw::BtBroadphaseProxy,
    shape: raw::BtCollisionShape,
    _thread: &'t Thread,
}

impl Body<'_> {
    pub fn id(&self) -> BodyId {
        BodyId(self.rigid.address())
    }

    /// Its root, game frame: its collision object's origin (the player's
    /// feet, which assign_physics_transform 0x2D93A0 moved it to this step).
    pub fn root(&self) -> [f32; 3] {
        // SAFETY: the object is in the physics world for the hook; its
        // transform's origin is four f32.
        from_bullet(unsafe { floats(self.object.transform().origin()) })
    }

    /// How far the player moved in the step, game frame: its character
    /// controller's last step, from where it started (m_last_pos, which
    /// player_step 0x7BDA0 sets at its start) to where it is. The physics
    /// tick (the step's list 3) moved it this step, and
    /// assign_physics_transform (list 4) took the hit model with it, before
    /// the bullets' list 5; a controller that did not move (updateAction
    /// 0x7B730 skips a step only then) left both the same. None for a player
    /// not alive, or inserted (spawned) after the step began: his controller's
    /// last step is not this step's.
    pub fn motion(&self, step: Step) -> Option<[f32; 3]> {
        // SAFETY: the player and his controller are alive for the hook; the
        // positions are three f32 each.
        unsafe {
            if read(self.player.alive()) == 0 || !GameTime(read(self.player.spawned())).not_after(step.from()) {
                return None;
            }
            let controller = self.player.controller()?.controller()?;
            let to: [f32; 3] = floats(controller.position());
            let from: [f32; 3] = floats(controller.last_position());
            Some([to[0] - from[0], to[1] - from[1], to[2] - from[2]])
        }
    }
}

/// The bullet's ray at `bullet.ray_traced`, its result the patch's to decide.
pub struct RayTest<'t> {
    bullet: raw::Bullet,
    result: raw::RayResult,
    frame: raw::CheckCollisionFrame,
    physics: raw::BulletPhysicsWorld,
    thread: &'t Thread,
}

impl<'t> RayTest<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<RayTest<'t>> {
        use raw::site::bullet_ray_traced as site;
        // SAFETY: the context is this site's: the bullet, its ray's result
        // (check_collision's local) and check_collision's frame, alive while
        // it runs, on its thread; the manager and its physics world live for
        // the bullet job.
        unsafe {
            let (bullet, result, frame) = (site::bullet(ctx)?, site::result(ctx)?, site::frame(ctx)?);
            let physics = bullet.manager()?.physics()?;
            Some(RayTest { bullet, result, frame, physics, thread: ctx.thread() })
        }
    }

    /// The ray's origin, game frame.
    pub fn origin(&self) -> [f32; 3] {
        // SAFETY: check_collision's frame, alive while it runs.
        unsafe { floats(self.frame.origin()) }
    }

    /// The ray's unit direction, game frame.
    pub fn direction(&self) -> [f32; 3] {
        // SAFETY: as origin.
        unsafe { floats(self.frame.direction()) }
    }

    /// The ray's length.
    pub fn length(&self) -> f32 {
        // SAFETY: as origin.
        unsafe { read(self.frame.length()) }
    }

    /// The bullet's flight times check_collision was given.
    pub fn flight(&self) -> Option<Flight> {
        // SAFETY: check_collision's three time arguments, bullet::tick's
        // locals, alive while it runs.
        unsafe {
            let (start, end, step_end) = (self.frame.start(), self.frame.end(), self.frame.step_end());
            if start.is_null() || end.is_null() || step_end.is_null() {
                return None;
            }
            Some(Flight { start: read(start), end: read(end), step_end: read(step_end) })
        }
    }

    /// The shooter's world core (none for a bullet without a shooter).
    fn core(&self) -> Option<raw::CoreWorld> {
        // SAFETY: the bullet's owner, its player's base: alive for the hook.
        unsafe { self.bullet.shooter()?.world_core() }
    }

    /// The world's step the bullet is ticked in: from the core's time to the
    /// bullet's (bullet::tick wrote it).
    pub fn step(&self) -> Option<Step> {
        let core = self.core()?;
        // SAFETY: the core and the bullet are alive for the hook.
        unsafe { Step::new(GameTime(read(core.time())), GameTime(read(self.bullet.now()))) }
    }

    /// Each place's player whose hit model is in the physics world, in the
    /// places' order (core_active_player 0x2D0AB0: the world's own, the
    /// place's current kit; a reference added is given back at once, as
    /// vostok::world's lookups do).
    pub fn bodies(&self, mut each: impl FnMut(Body<'t>)) {
        let Some(core) = self.core() else { return };
        for place in Place::all() {
            let mut out = None;
            // SAFETY: the core's own lookup; it writes the player, a
            // reference added, through `out`. The player stays the place's
            // (the place holds its reference) for the hook; his hit model,
            // its body and collision object and proxy, alive with him.
            unsafe {
                raw::engine_core_active_player(Some(core), u32::from(place.get()), &mut out);
                let Some(player) = out else { continue };
                let count = player.references();
                if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                    raw::engine_release_object(Some(player), count);
                    continue;
                }
                let Some(rigid) = player.hit_model().and_then(|model| model.body()) else { continue };
                let Some(object) = rigid.object() else { continue };
                let (Some(proxy), Some(shape)) = (object.broadphase(), object.shape()) else { continue };
                each(Body { player, rigid, object, proxy, shape, _thread: self.thread });
            }
        }
    }

    /// What the result names now.
    pub fn struck(&self) -> Struck {
        // SAFETY: the result, check_collision's local; its object's
        // material read only once its vtable says it is a hit model's body.
        unsafe {
            let Some(object) = self.result.object() else { return Struck::Nothing };
            if read(object.vtable()) == raw::image::ANIMATED_BODY_VTABLE && read(object.body().material()) == ACTOR {
                Struck::Body(BodyId(object.address()))
            } else {
                Struck::Level
            }
        }
    }

    /// The game's ray test again into the result, the level alone (every
    /// object a bullet meets but the hit models): what the bullet meets
    /// before the hit models are tested where they stood.
    pub fn trace_level(&mut self) {
        let (origin, direction) = (self.origin(), self.direction());
        // SAFETY: the physics world, the result (check_collision's) and our
        // float3s, as check_collision calls it with the level's mask.
        unsafe {
            raw::engine_ray_test(Some(self.physics), Some(self.result), origin.as_ptr(), direction.as_ptr(), self.length(),
                                 BULLET_GROUP, LEVEL_MASK, self.bullet.address() as usize as *const core::ffi::c_void,
                                 1);
        }
    }

    /// The ray's ends in Bullet's frame, as ray_test makes them: the origin
    /// and origin + direction x length.
    fn ends(&self) -> ([f32; 4], [f32; 4]) {
        let (o, d, length) = (self.origin(), self.direction(), self.length());
        let end = [o[0] + d[0] * length, o[1] + d[1] * length, o[2] + d[2] * length];
        (to_bullet(o), to_bullet(end))
    }

    /// Tests `body` against the ray, placed `back` (game frame) from where
    /// it is (none: where it is), its bones as they are posed at the step's
    /// end: a hit closer than the result's (if it has one) replaces it.
    /// Whether it did.
    pub fn trace_body(&mut self, body: &Body<'_>, back: Option<[f32; 3]>) -> bool {
        let (from, to) = self.ends();
        let mut callback = Aligned(raw::ClosestRayResultCallbackStorage::zeroed());
        let mut from_t = Aligned(raw::BtTransformStorage::zeroed());
        let mut to_t = Aligned(raw::BtTransformStorage::zeroed());
        let mut moved = Aligned(raw::BtTransformStorage::zeroed());
        let (callback, from_t, to_t, moved) =
            (callback.0.handle(), from_t.0.handle(), to_t.0.handle(), moved.0.handle());
        // SAFETY: the callback, the transforms and the ends are ours, the
        // body's objects alive for the hook (the game's own ray test reads
        // them alike from the bullet threads); the callback is the game's
        // own, made by its constructor; rayTestSingle writes it alone (a
        // compound's shape it swaps for each child it puts back).
        unsafe {
            raw::engine_ray_callback(Some(callback), from.as_ptr(), to.as_ptr(), BULLET_GROUP, BULLET_MASK,
                                     Some(self.bullet));
            *callback.flags() |= UNFLIPPED;
            *callback.single_pass() = 1;
            *callback.fraction() = match self.result.object() {
                Some(_) => read(self.result.fraction()),
                None => NO_HIT,
            };
            if raw::engine_ray_needs_collision(Some(callback), Some(body.proxy)) == 0 {
                return false;
            }
            // the ray's ends as btSingleRayCallback 0xA3050 makes them
            for (transform, origin) in [(from_t, from), (to_t, to)] {
                write_floats(transform.basis(), [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
                write_floats(transform.origin(), origin);
            }
            let world = match back {
                None => body.object.transform(),
                Some(back) => {
                    let mut copy: [f32; 16] = floats(body.object.transform().basis());
                    // the game frame's z is Bullet's -z
                    copy[12] -= back[0];
                    copy[13] -= back[1];
                    copy[14] += back[2];
                    write_floats(moved.basis(), copy);
                    moved
                }
            };
            raw::engine_ray_test_single(Some(from_t), Some(to_t), Some(body.object), Some(body.shape), Some(world),
                                        Some(callback));
            let Some(hit) = callback.object() else { return false };
            // as ray_test writes its result (0x75F8F..0x760A9)
            write_floats(self.result.point(), from_bullet(floats(callback.point())));
            *self.result.fraction() = read(callback.fraction());
            write_floats(self.result.normal(), from_bullet(floats(callback.normal())));
            self.result.set_object(hit.user());
            *self.result.part() = read(callback.part());
            *self.result.part_kind() = read(callback.part_kind());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_as_ray_test_makes_them() {
        assert_eq!(to_bullet([1.0, -0.0, 2.0]).map(f32::to_bits), [1.0f32, 0.0, -2.0, 0.0].map(f32::to_bits));
        assert_eq!(to_bullet([0.0, 0.0, 0.0])[2].to_bits(), 0.0f32.to_bits(), "-0 is 0");
        assert_eq!(from_bullet([3.0, 4.0, -5.0, 9.0]), [3.0, 4.0, 5.0]);
        assert!(bullet_float(f32::NAN).is_nan());
    }

    #[test]
    fn a_step_is_one_to_fifty_ms() {
        assert_eq!(Step::new(GameTime(100), GameTime(150)).map(Step::ms), Some(50));
        assert_eq!(Step::new(GameTime(130), GameTime(150)).map(Step::ms), Some(20));
        assert_eq!(Step::new(GameTime(150), GameTime(150)), None);
        assert_eq!(Step::new(GameTime(100), GameTime(151)), None);
        assert_eq!(Step::new(GameTime(u32::MAX - 9), GameTime(20)).map(Step::ms), Some(30), "across the wrap");
        assert_eq!(Step::new(GameTime(150), GameTime(100)), None, "backwards");
    }
}
