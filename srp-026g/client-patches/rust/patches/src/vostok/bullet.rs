//! bullet::check_collision 0x32D4E0 at `bullet.ray_traced` (sites_026g.tsv):
//! EDI the bullet, EAX its ray's result; the ray itself in the function's
//! frame (check_collision_frame, by the site's ESP). The bullet manager's
//! effect queues, for a body hit the game did not see.
//!
//! Read only: the bullet goes on exactly as before (the hook replays the
//! two moves of the result it sits on).

use core::ffi::c_void;

use super::context::{Context, Reg};
use super::world::{BodyId, Object, floats, pointer, read};
use crate::raw;

/// The material a living player's hit model is to bullets: every hit body's
/// +38h is 10, `actor` (bt_animated_rigid_body's ctor 0x70E70); the pair
/// bullet x actor plays fx/material_hits/blod and body_hits.
const ACTOR: u32 = 10;
/// The material pairs' table is 128 by 128 (material_pair 0x306D50).
const MATERIALS: u32 = 128;

/// A bullet's ray just traced, its result not yet read.
pub struct BulletRay<'t> {
    bullet: Object,
    result: Object,
    /// check_collision's frame: the site's ESP.
    frame: *mut c_void,
    _thread: &'t super::Thread,
}

impl<'t> BulletRay<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<BulletRay<'t>> {
        let bullet = Object::at(ctx.reg(Reg::Edi))?;
        let result = Object::at(ctx.reg(Reg::Eax))?;
        // Context already removes the dispatch slot and flags from saved ESP.
        let frame = ctx.reg(Reg::Esp) as usize as *mut c_void;
        Some(BulletRay { bullet, result, frame, _thread: ctx.thread() })
    }

    /// The bullet's id.
    pub fn id(&self) -> u32 {
        // SAFETY: the bullet's field.
        unsafe { read(raw::bullet::id(self.bullet.ptr())) }
    }

    /// The shooter's place; None without a shooter (the game then plays no
    /// hit effects).
    pub fn shooter_place(&self) -> Option<u8> {
        // SAFETY: the bullet's shooter, its place.
        unsafe {
            let shooter = pointer(raw::bullet::shooter(self.bullet.ptr()).cast())?;
            Some(read(raw::bullet_owner::place(shooter.ptr())))
        }
    }

    /// Its segments past the first (pierces and ricochets).
    pub fn segment(&self) -> u32 {
        // SAFETY: the bullet's field.
        unsafe { read(raw::bullet::segment(self.bullet.ptr())) }
    }

    /// The ammo config's `mass` (NaN without ammo data).
    pub fn ammo_mass(&self) -> f32 {
        // SAFETY: the bullet's ammo data, its mass.
        unsafe {
            pointer(raw::bullet::ammo(self.bullet.ptr()).cast()).map_or(f32::NAN, |a| read(raw::ammo_data::mass(a.ptr())))
        }
    }

    /// The ammo config's `muzzle_speed`, m/s (NaN without ammo data): with
    /// the mass, which ammo it is (one or two configs a pair).
    pub fn muzzle_speed(&self) -> f32 {
        // SAFETY: the bullet's ammo data, its muzzle speed.
        unsafe {
            pointer(raw::bullet::ammo(self.bullet.ptr()).cast())
                .map_or(f32::NAN, |a| read(raw::ammo_data::muzzle_speed(a.ptr())))
        }
    }

    /// Its velocity now, its segment's (m/s).
    pub fn velocity(&self) -> [f32; 3] {
        // SAFETY: the bullet's float3.
        unsafe { floats(raw::bullet::velocity(self.bullet.ptr())) }
    }

    /// The ray's origin.
    pub fn origin(&self) -> [f32; 3] {
        // SAFETY: check_collision's frame at the site, its local.
        unsafe { floats(raw::check_collision_frame::origin(self.frame)) }
    }

    /// The ray's unit direction.
    pub fn direction(&self) -> [f32; 3] {
        // SAFETY: as origin.
        unsafe { floats(raw::check_collision_frame::direction(self.frame)) }
    }

    /// The ray's length.
    pub fn length(&self) -> f32 {
        // SAFETY: as origin.
        unsafe { read(raw::check_collision_frame::length(self.frame)) }
    }

    /// Where the ray hit something (the level, a living player), if it did.
    pub fn struck(&self) -> Option<[f32; 3]> {
        // SAFETY: the result's fields.
        unsafe {
            (read(raw::ray_result::object(self.result.ptr())) != 0).then(|| floats(raw::ray_result::point(self.result.ptr())))
        }
    }

    /// The living player's hit model the ray hit, if that is what it hit (a
    /// bt_animated_rigid_body of material actor).
    pub fn struck_body(&self) -> Option<BodyId> {
        // SAFETY: the result's object, its vtable and material.
        unsafe {
            let object = pointer(raw::ray_result::object(self.result.ptr()))?;
            let body = read(raw::bt_animated_rigid_body::vtable(object.ptr())) == raw::image::ANIMATED_BODY_VTABLE
                && u32::from(read(raw::bt_animated_rigid_body::material(object.ptr()))) == ACTOR;
            body.then_some(BodyId(object.id()))
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
        // SAFETY: the bullet's manager and material, the manager's renderer
        // and materials, the pair the game's own material_pair gives, its
        // fields; the manager's own effect queues, our float3s.
        unsafe {
            let bullet = self.bullet.ptr();
            let Some(manager) = pointer(raw::bullet::manager(bullet).cast()) else { return };
            let (Some(materials), Some(material)) =
                (pointer(raw::bullet_manager::materials(manager.ptr()).cast()), pointer(raw::bullet::material(bullet).cast()))
            else {
                return;
            };
            if read(raw::bullet_manager::renderer(manager.ptr())) == 0 {
                return;
            }
            let first = u32::from(read(raw::game_material::id(material.ptr())));
            if first >= MATERIALS {
                return;
            }
            let pair = raw::engine_material_pair(materials.ptr(), first, ACTOR).cast_mut();
            if pair.is_null() {
                return;
            }
            let particles = read(raw::material_pair::particles(pair));
            if particles != 0 && particles != read(raw::material_pair::particles_end(pair)) {
                raw::engine_play_particle(point.as_ptr(), manager.ptr(), particles as usize as *const c_void,
                                          direction.as_ptr(), normal.as_ptr(), read(raw::material_pair::parameter(pair)));
            }
            if read(raw::material_pair::sound(pair)) != 0 && read(raw::bullet::quiet(bullet)) == 0 && self.segment() <= 1 {
                raw::engine_play_sound(manager.ptr(), raw::material_pair::sound(pair).cast(), point.as_ptr());
            }
        }
    }
}
