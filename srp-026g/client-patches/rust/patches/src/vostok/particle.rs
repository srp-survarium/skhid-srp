//! Particle systems our patches play at a place of a game world's render
//! scene, as a damage zone plays its own (0.10b damage_zone::play_particles):
//! a particle system instance (a resource of class 61, a new one each query,
//! [`super::resources::query_particles`]) is played at a transform with
//! `scene_renderer::play_particle_system` (0x22660) and taken out with
//! `remove_particle_system_instance` (0x229D0). Both queue a command to the
//! render thread, each taking references of its own, so the patch's
//! reference ([`Particle`]) may go right after.
//!
//! The game world's render scene is made once, at its first match's load
//! (game_world::on_project_loaded keeps it while it is not 0), and lives as
//! long as the game: a particle played in it stays, from one match to the
//! next, until it is taken out. A looping one plays as long.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::hook::Thread;
use super::world::GameWorld;
use crate::raw;

/// A particle system instance, one reference of its own.
pub struct Particle(raw::ParticleSystemInstance);

// SAFETY: its count is atomic (every taker lock xadds it); it is kept in a
// patch's lock and played, taken out and let go on the game's thread.
unsafe impl Send for Particle {}

impl Particle {
    /// # Safety
    /// `instance` holds a reference that is now this Particle's.
    pub(super) unsafe fn owning(instance: raw::ParticleSystemInstance) -> Particle {
        Particle(instance)
    }
}

impl Drop for Particle {
    fn drop(&mut self) {
        let mut slot = Some(self.0);
        // SAFETY: our reference, let go once, by the game's own release of
        // an unmanaged resource (0x11540: count +0D0h, deleted at 0).
        unsafe { raw::engine_release_particle(&mut slot) }
    }
}

/// A game world's render scene and the scene renderer that plays in it.
pub struct WorldScene<'t> {
    renderer: raw::SceneRenderer,
    scene: raw::SceneReference,
    thread: PhantomData<&'t Thread>,
}

impl<'t> WorldScene<'t> {
    /// The scene of `world`, once it has one (its first match loaded).
    pub fn of(world: &GameWorld<'t>) -> Option<WorldScene<'t>> {
        // SAFETY: the live game world of the hook, on the game's thread; its
        // game's renderer lives as long as the game, the scene slot as long
        // as the world.
        unsafe {
            let renderer = world.this.game()?.renderer()?.scene_renderer()?;
            let scene = world.this.scene();
            if scene.value().read() == 0 {
                return None;
            }
            Some(WorldScene { renderer, scene, thread: PhantomData })
        }
    }

    /// Plays `particle` at `at` (upright: the world's axes), as
    /// game_world::play_particle gives its two transforms (0x25BEB4), no
    /// attachment.
    pub fn play(&self, particle: &Particle, at: [f32; 3]) {
        let mut transform = [0.0f32; 16];
        for i in 0..4 {
            transform[i * 5] = 1.0;
        }
        transform[12..15].copy_from_slice(&at);
        let (mut first, mut second) = (None, None);
        // SAFETY: the scene renderer and scene of a live world (of); the
        // instance by value with a reference taken for it, which the callee
        // lets go at its end (0x227EB), as game_world::play_particle takes
        // one (0x25BE95); the transforms ours, read before it returns; the
        // attachments empty resource pointers, read only.
        unsafe {
            AtomicU32::from_ptr(particle.0.references()).fetch_add(1, Ordering::AcqRel);
            raw::engine_play_particle_system(Some(self.renderer), Some(self.scene), Some(particle.0),
                                             transform.as_ptr(), transform.as_ptr(), &mut first, &mut second);
        }
    }

    /// Takes `particle` out of the scene.
    pub fn remove(&self, particle: &Particle) {
        let mut slot = Some(particle.0);
        // SAFETY: the scene renderer and scene of a live world (of), the
        // instance's pointer ours (the callee reads it and takes a reference
        // for its command, 0x22A57).
        unsafe { raw::engine_remove_particle_system(Some(self.renderer), Some(self.scene), &mut slot) }
    }
}
