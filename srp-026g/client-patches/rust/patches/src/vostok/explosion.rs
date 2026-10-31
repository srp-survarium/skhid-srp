//! bullet_manager::explode 0x339630 at `bullet_manager.explode`: every blast
//! of the simulation, a grenade's (0x30A4ED) and a booby trap's (0x305728),
//! before the game applies its splash. Its centre, its splash radius and the
//! game's own test of what the splash passes through (0x3393F0).
//!
//! Read only: the game's splash goes on exactly as before.

use super::bullet::{MaterialId, SurfaceBreakEffects};
use super::context::Context;
use super::site;
use super::geometry::{Distance, WorldPoint};
use super::project::PhysicsId;
use super::world::{floats, read};
use crate::raw;

/// A blast about to splash.
pub struct Explosion<'t> {
    params: raw::ExplosionParams,
    manager: raw::BulletManager,
    centre: WorldPoint,
    radius: Distance,
    _thread: &'t super::Thread,
}

impl<'t> Explosion<'t> {
    pub fn at(ctx: &'t Context<'_, site::BulletManagerExplode>) -> Option<Explosion<'t>> {
        use raw::site::bullet_manager_explode as at;
        let (params, frame) = (at::params(ctx)?, at::frame(ctx)?);
        // SAFETY: the site's splash and explode's arguments, alive while it
        // runs, on its thread.
        unsafe {

            let manager = frame.manager()?;
            let centre = frame.centre();
            if centre.is_null() { return None; }
            Some(Explosion {
                params, manager,
                centre: WorldPoint::parse(floats(centre))?,
                radius: Distance::parse(read(params.splash_radius()))?,
                _thread: ctx.thread(),
            })
        }
    }

    pub fn physics(&self) -> Option<PhysicsId> {
        // SAFETY: the manager and its physics world live for the simulation.
        unsafe { Some(PhysicsId::of(self.manager.physics()?)) }
    }

    pub fn centre(&self) -> WorldPoint { self.centre }

    /// The splash radius the game damages within (the thrower's modifier in).
    pub fn radius(&self) -> Distance { self.radius }

    /// Whether the splash reaches `point` by the game's own rule for a
    /// player's bone: its splash_pierce kept through every surface between
    /// (armor 0, glass, passes), above 0. The centre itself is reached.
    pub fn reaches(&self, point: WorldPoint) -> bool {
        let (from, to) = (self.centre.coordinates(), point.coordinates());
        let ray: [f32; 3] = core::array::from_fn(|k| to[k] - from[k]);
        if ray == [0.0; 3] { return true; }
        // SAFETY: the manager, its physics world and materials, alive for the
        // simulation; the game's own trace of explode, our float3s, no target.
        unsafe {
            let strength = read(self.params.splash_pierce());
            raw::engine_explosion_reach(ray.as_ptr(), Some(self.manager), from.as_ptr(), strength,
                                        core::ptr::null()) > 0.0
        }
    }

    /// The break feedback of the material pair source x target on the
    /// blast's bullet manager (none without a renderer).
    pub fn surface_break_effects(&self, source: MaterialId, target: MaterialId) -> Option<SurfaceBreakEffects<'t>> {
        // SAFETY: the manager is alive for the simulation; the result
        // borrows this hook's thread.
        unsafe { SurfaceBreakEffects::on(self.manager, source, target, self._thread) }
    }
}
