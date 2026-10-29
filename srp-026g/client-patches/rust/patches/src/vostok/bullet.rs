//! bullet::check_collision 0x32D4E0 at `bullet.ray_traced` (sites_026g.tsv):
//! the bullet, its ray's result, and the ray itself in the function's frame
//! (sites_026g.tsv's `holds`). The bullet manager's
//! effect queues, for a body hit the game did not see.
//!
//! Read only: the bullet goes on exactly as before (the hook replays the
//! two moves of the result it sits on).

use super::context::Context;
use super::world::{BodyId, floats, read};
use crate::raw;

/// The material a living player's hit model is to bullets: every hit body's
/// +38h is 10, `actor` (bt_animated_rigid_body's ctor 0x70E70); the pair
/// bullet x actor plays fx/material_hits/blod and body_hits.
const ACTOR: u32 = 10;
/// The material pairs' table is 128 by 128 (material_pair 0x306D50).
const MATERIALS: u32 = 128;

/// A bullet's ray just traced, its result not yet read.
pub struct BulletRay<'t> {
    bullet: raw::Bullet,
    result: raw::RayResult,
    /// check_collision's frame.
    frame: raw::CheckCollisionFrame,
    _thread: &'t super::Thread,
}

impl<'t> BulletRay<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<BulletRay<'t>> {
        use raw::site::bullet_ray_traced as site;
        // SAFETY: the context is this site's: the bullet, its ray's result and
        // check_collision's frame, alive while it runs, on its thread.
        let (bullet, result, frame) = unsafe { (site::bullet(ctx)?, site::result(ctx)?, site::frame(ctx)?) };
        Some(BulletRay { bullet, result, frame, _thread: ctx.thread() })
    }

    /// The bullet's id.
    pub fn id(&self) -> u32 {
        // SAFETY: the bullet is alive for the hook.
        unsafe { read(self.bullet.id()) }
    }

    /// The shooter's place; None without a shooter (the game then plays no
    /// hit effects).
    pub fn shooter_place(&self) -> Option<u8> {
        // SAFETY: the bullet's shooter, alive for the hook.
        unsafe { Some(read(self.bullet.shooter()?.place())) }
    }

    /// Its segments past the first (pierces and ricochets).
    pub fn segment(&self) -> u32 {
        // SAFETY: as id.
        unsafe { read(self.bullet.segment()) }
    }

    /// The ammo config's `mass` (NaN without ammo data).
    pub fn ammo_mass(&self) -> f32 {
        // SAFETY: the bullet's ammo data, alive for the hook.
        unsafe { self.bullet.ammo().map_or(f32::NAN, |ammo| read(ammo.mass())) }
    }

    /// The ammo config's `muzzle_speed`, m/s (NaN without ammo data): with
    /// the mass, which ammo it is (one or two configs a pair).
    pub fn muzzle_speed(&self) -> f32 {
        // SAFETY: as ammo_mass.
        unsafe { self.bullet.ammo().map_or(f32::NAN, |ammo| read(ammo.muzzle_speed())) }
    }

    /// Its velocity now, its segment's (m/s).
    pub fn velocity(&self) -> [f32; 3] {
        // SAFETY: as id.
        unsafe { floats(self.bullet.velocity()) }
    }

    /// The ray's origin.
    pub fn origin(&self) -> [f32; 3] {
        // SAFETY: check_collision's frame, alive while it runs.
        unsafe { floats(self.frame.origin()) }
    }

    /// The ray's unit direction.
    pub fn direction(&self) -> [f32; 3] {
        // SAFETY: as origin.
        unsafe { floats(self.frame.direction()) }
    }

    /// The ray's length.
    pub fn length(&self) -> f32 {
        // SAFETY: as origin.
        unsafe { read(self.frame.length()) }
    }

    /// Where the ray hit something (the level, a living player), if it did.
    pub fn struck(&self) -> Option<[f32; 3]> {
        // SAFETY: the result, check_collision's local, alive while it runs.
        unsafe { self.result.object().map(|_| floats(self.result.point())) }
    }

    /// The living player's hit model the ray hit, if that is what it hit (a
    /// bt_animated_rigid_body of material actor).
    pub fn struck_body(&self) -> Option<BodyId> {
        // SAFETY: the result's object, alive while check_collision runs; its
        // material read only once its vtable says it is a bt_animated_rigid_body.
        unsafe {
            let object = self.result.object()?;
            let body = read(object.vtable()) == raw::image::ANIMATED_BODY_VTABLE
                && u32::from(read(object.body().material())) == ACTOR;
            body.then_some(BodyId(object.address()))
        }
    }

    /// The game's own effects of this bullet in a living body, at `point`:
    /// the material pair bullet x actor's first particle and its sound, as
    /// process_front_face_piercing 0x32E4F0 plays them for a pierced player
    /// (the sound only when the bullet is not quiet and at most one segment
    /// on). The bullet manager's queues take them (it runs them after the
    /// tick). No wound decal: the game's own goes nowhere on a body (a
    /// player's hittable gives add_decal no model, 0x1F1780).
    pub fn play_body_hit(&self, point: [f32; 3], direction: [f32; 3], normal: [f32; 3]) {
        // SAFETY: the bullet's manager and material, the manager's materials,
        // the pair the game's own material_pair gives: alive for the hook; the
        // manager's own effect queues, our float3s.
        unsafe {
            let bullet = self.bullet;
            let Some(manager) = bullet.manager() else { return };
            let (Some(materials), Some(material)) = (manager.materials(), bullet.material()) else {
                return;
            };
            if read(manager.renderer()) == 0 {
                return;
            }
            let first = u32::from(read(material.id()));
            if first >= MATERIALS {
                return;
            }
            let Some(pair) = raw::engine_material_pair(Some(materials), first, ACTOR) else { return };
            let particles = pair.particles();
            if particles.is_some() && particles != pair.particles_end() {
                raw::engine_play_particle(point.as_ptr(), Some(manager), particles, direction.as_ptr(),
                                          normal.as_ptr(), read(pair.parameter()));
            }
            if read(pair.sound()) != 0 && read(bullet.quiet()) == 0 && self.segment() <= 1 {
                raw::engine_play_sound(Some(manager), pair.sound(), point.as_ptr());
            }
        }
    }
}
