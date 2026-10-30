//! bullet::check_collision 0x32D4E0 at `bullet.ray_traced` (sites_026g.tsv):
//! the bullet, its ray's result, and the ray itself in the function's frame
//! (sites_026g.tsv's `holds`). The bullet manager's
//! effect queues, for a body hit the game did not see.
//!
//! Read only: the bullet goes on exactly as before (the hook replays the
//! two moves of the result it sits on).
//!
//! At the `bullet.decal_*` sites, a hit about to queue its decal: whether to
//! queue it is the patch's (the hook exits past add_decal or replays).

use super::context::Context;
use super::world::{BodyId, floats, read};
use crate::raw;
use super::geometry::{WorldPoint, RayDirection};
use super::project::PhysicsId;

/// The material a living player's hit model is to bullets: every hit body's
/// +38h is 10, `actor` (bt_animated_rigid_body's ctor 0x70E70); the pair
/// bullet x actor plays fx/material_hits/blod and body_hits.
const ACTOR: u32 = 10;
/// The material pairs' table is 128 by 128 (material_pair 0x306D50).
const MATERIALS: u32 = 128;

/// A valid material-pair index, parsed from authored content once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialId(u32);
impl MaterialId {
    pub fn parse(value: u32) -> Option<Self> { (value < MATERIALS).then_some(Self(value)) }
}

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

    pub fn physics(&self) -> Option<PhysicsId> {
        // SAFETY: the manager and its physics world live for the bullet job.
        unsafe { Some(PhysicsId::of(self.bullet.manager()?.physics()?)) }
    }

    /// The break feedback of the material pair source x target, on this
    /// bullet's manager. A quiet bullet has none; simulation can still change
    /// lamp state.
    pub fn surface_break_effects(&self, source: MaterialId, target: MaterialId) -> Option<SurfaceBreakEffects<'t>> {
        // SAFETY: the bullet and its manager are alive for the bullet job;
        // the result borrows this hook's thread.
        unsafe {
            if read(self.bullet.quiet()) != 0 { return None; }
            SurfaceBreakEffects::on(self.bullet.manager()?, source, target, self._thread)
        }
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


/// The four places a bullet's hit queues its decal (bullet_manager::add_decal
/// 0x321FA0), each at its material pair's decal slot (sites_026g.tsv
/// `bullet.decal_*`): entering, leaving, ricocheting off and stopping in a
/// surface.
#[derive(Clone, Copy, Debug)]
pub enum DecalSite { Enter, Leave, Ricochet, Stop }

/// A bullet's hit, its decal not yet queued. The decal is the game world's,
/// at the hit's point, a world-space box of a ring of ids: no model owns it,
/// so a model removed later leaves it where it was.
pub struct BulletDecal<'t> {
    bullet: raw::Bullet,
    hit: raw::RayResult,
    _thread: &'t super::Thread,
}

impl<'t> BulletDecal<'t> {
    pub fn at(ctx: &'t Context<'_>, site: DecalSite) -> Option<BulletDecal<'t>> {
        use raw::site::{bullet_decal_back as leave, bullet_decal_collide as stop, bullet_decal_front as enter,
                        bullet_decal_reflect as ricochet};
        // SAFETY: the context is this site's: the bullet and its hit, alive
        // while the process_* function runs, on its thread.
        let (bullet, hit) = unsafe {
            match site {
                DecalSite::Enter => (enter::bullet(ctx)?, enter::hit(ctx)?),
                DecalSite::Leave => (leave::bullet(ctx)?, leave::hit(ctx)?),
                DecalSite::Ricochet => (ricochet::bullet(ctx)?, ricochet::hit(ctx)?),
                DecalSite::Stop => (stop::bullet(ctx)?, stop::hit(ctx)?),
            }
        };
        Some(BulletDecal { bullet, hit, _thread: ctx.thread() })
    }

    pub fn physics(&self) -> Option<PhysicsId> {
        // SAFETY: the manager and its physics world live for the bullet job.
        unsafe { Some(PhysicsId::of(self.bullet.manager()?.physics()?)) }
    }

    /// The material struck (None outside the material pairs' table).
    pub fn surface(&self) -> Option<MaterialId> {
        // SAFETY: the bullet's struck material, set by check_collision before
        // this hit's processing; the material table's, alive for the job.
        unsafe { MaterialId::parse(u32::from(read(self.bullet.surface()?.id()))) }
    }

    /// Where the decal goes: the hit's point.
    pub fn point(&self) -> Option<WorldPoint> {
        // SAFETY: the hit, check_collision's local, alive while it runs.
        WorldPoint::parse(unsafe { floats(self.hit.point()) })
    }
}
struct ParticleEffect { slot: raw::ParticleSlot, parameter: u32 }
struct SoundEffect { slot: *mut u32 }

/// A parsed native material pair's worker-safe feedback queues. Optional
/// particle/sound resources carry only the arguments valid for that effect.
pub struct SurfaceBreakEffects<'t> {
    manager: raw::BulletManager,
    particle: Option<ParticleEffect>,
    sound: Option<SoundEffect>,
    _thread: &'t super::Thread,
}
impl<'t> SurfaceBreakEffects<'t> {
    /// Parse the pair's feedback once from the manager's material table:
    /// none without a renderer (the game's own queues play nothing then).
    ///
    /// # Safety
    /// `manager` is alive for the job of `thread`'s hook.
    pub(in crate::vostok) unsafe fn on(manager: raw::BulletManager, source: MaterialId, target: MaterialId,
                                       thread: &'t super::Thread) -> Option<Self> {
        // SAFETY: the manager and its material table own these resources
        // until the worker joins (the caller's word for the manager).
        unsafe {
            if read(manager.renderer()) == 0 { return None; }
            let pair = raw::engine_material_pair(Some(manager.materials()?), source.0, target.0)?;
            let particle = pair.particles().filter(|first| Some(*first) != pair.particles_end())
                .map(|slot| ParticleEffect { slot, parameter: read(pair.parameter()) });
            let sound = (read(pair.sound()) != 0).then(|| SoundEffect { slot: pair.sound() });
            Some(Self { manager, particle, sound, _thread: thread })
        }
    }
    /// The pair's particles at each of `shards`, its sound once at `sound_at`.
    pub fn play(&self, shards: impl IntoIterator<Item = WorldPoint>, sound_at: WorldPoint, direction: RayDirection) {
        let direction = direction.coordinates();
        let normal = direction.map(|v| -v);
        // SAFETY: the constructor parsed the resources from this worker's
        // live manager. Native queues synchronize writes and copy the local
        // float3s; the thread borrow prevents retaining this capability.
        unsafe {
            if let Some(particle) = &self.particle {
                for point in shards {
                    raw::engine_play_particle(point.coordinates().as_ptr(), Some(self.manager), Some(particle.slot),
                                              direction.as_ptr(), normal.as_ptr(), particle.parameter);
                }
            }
            if let Some(sound) = &self.sound {
                raw::engine_play_sound(Some(self.manager), sound.slot, sound_at.coordinates().as_ptr());
            }
        }
    }
}
