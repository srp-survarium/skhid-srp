//! The particle worlds as patch 138's census reads them (the census is
//! `patches::particle_census`): a world at its tick, and a particle system
//! instance as its cook finishes it, each read once here into typed values.
//!
//! Nothing here writes the game's memory or calls the engine. A world's
//! ticked list, each instance's emitter lists and their counts are read on
//! the thread that ticks that world, at the start of its tick: the engine
//! itself walks them there with no lock (particle_world::tick 0x35AE10), and
//! adds or removes instances on that thread alone.
//!
//! The census is dormant: its two sites keep the game's bytes until
//! [`start`] writes their jumps, at the process's start, only when the game
//! was started with the census on.

use super::context::Context;
use super::hook::Thread;
use super::world::read;
use crate::raw;
use core::marker::PhantomData;
use core::num::NonZeroU32;
use core::sync::atomic::{AtomicBool, Ordering};

/// A particle_system (the shared template instances are made from), as an
/// identity only: never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateId(NonZeroU32);

/// A particle world, as an identity only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldId(NonZeroU32);

#[cfg(test)]
impl TemplateId {
    pub fn at(address: u32) -> TemplateId {
        TemplateId(NonZeroU32::new(address).expect("an address"))
    }
}

#[cfg(test)]
impl WorldId {
    pub fn at(address: u32) -> WorldId {
        WorldId(NonZeroU32::new(address).expect("an address"))
    }
}

impl WorldId {
    /// Its address, for a log line.
    pub fn address(self) -> u32 {
        self.0.get()
    }
}

/// An instance's m_visible, which only the game sets
/// (update_particle_system_instance): a hidden instance is not handed to the
/// renderer, but its emitters tick and keep their particles all the same.
/// It is not culling: what is off screen is still `Shown`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Shown,
    Hidden,
}

/// What one ticked particle system instance holds now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance {
    /// Its first lod's template (None: no template set).
    pub template: Option<TemplateId>,
    pub shown: Shown,
    /// Its current lod's emitter instances.
    pub emitters: u32,
    /// Their live particles (m_num_live_particles).
    pub particles: u32,
}

/// The lods an instance has room for (max_supported_lods).
const LODS: u32 = 10;
/// The most instances of a world and emitters of an instance a walk follows:
/// far above the game's, so a list that does not end (it would hang the
/// game's own walk too) cannot hang ours.
const MOST_INSTANCES: usize = 1 << 14;
const MOST_EMITTERS: usize = 1 << 10;

/// A particle world at the start of its tick (`particle.world_tick`).
pub struct World<'t> {
    this: raw::ParticleWorld,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> World<'t> {
    /// `particle.world_tick`: ECX the world.
    pub fn at_tick(ctx: &'t Context<'_>) -> Option<World<'t>> {
        // SAFETY: the context is the site's: ECX is particle_world::tick's this.
        let this = unsafe { raw::site::particle_world_tick::world(ctx) }?;
        Some(World { this, _thread: PhantomData })
    }

    pub fn id(&self) -> Option<WorldId> {
        NonZeroU32::new(self.this.address()).map(WorldId)
    }

    /// The particles alive in the world (m_num_particles).
    pub fn particles(&self) -> u32 {
        // SAFETY: the world is alive: its own tick is running.
        unsafe { read(self.this.particles()) }
    }

    /// The world's cap (m_max_particles).
    pub fn max_particles(&self) -> u32 {
        // SAFETY: as particles.
        unsafe { read(self.this.max_particles()) }
    }

    /// Each instance of the ticked list, in its order.
    pub fn instances(&self, mut each: impl FnMut(Instance)) {
        // SAFETY: the tick's thread at the tick's start: the list and each
        // instance's emitter lists change on this thread alone (the module's
        // note), and the list holds a reference to each instance.
        unsafe {
            let mut next = self.this.instances();
            for _ in 0..MOST_INSTANCES {
                let Some(instance) = next else { return };
                let lods = instance.lods();
                let template = lods.template().and_then(|template| NonZeroU32::new(template.address())).map(TemplateId);
                let shown = if read(instance.visible()) != 0 { Shown::Shown } else { Shown::Hidden };
                let (mut emitters, mut particles) = (0u32, 0u32);
                let lod = read(instance.current_lod());
                if lod < LODS {
                    let mut emitter = lods.at(lod as usize).emitters();
                    for _ in 0..MOST_EMITTERS {
                        let Some(one) = emitter else { break };
                        emitters += 1;
                        particles = particles.saturating_add(read(one.live_particles()));
                        emitter = one.next();
                    }
                }
                each(Instance { template, shown, emitters, particles });
                next = instance.next();
            }
        }
    }
}

/// The longest path read for a name.
const PATH_LIMIT: usize = 260;

/// A particle system instance its cook has just made
/// (`particle.materials_loaded`).
#[derive(Clone, Copy, Debug)]
pub struct Created<'t> {
    /// The template every lod of it was given.
    pub template: TemplateId,
    /// The path its query asked for: the effect's name.
    pub path: &'t [u8],
    /// The emitter instances made with it, over all its lods: one material
    /// query each.
    pub emitters: u32,
}

impl<'t> Created<'t> {
    /// `particle.materials_loaded`: the frame's results and instance. None
    /// when either is absent, the instance has no template or its query no
    /// path.
    pub fn at_site(ctx: &'t Context<'_>) -> Option<Created<'t>> {
        // SAFETY: the context is the site's: on_materials_loaded's start,
        // its arguments above the return address. The results and their
        // parent query (the instance's own, not finished before this
        // function finishes it) are alive for the call, the query's path a
        // C string it keeps; the instance was made by the cook and is only
        // read.
        unsafe {
            let frame = raw::site::particle_materials_loaded::frame(ctx)?;
            let results = frame.results()?;
            let template = frame.instance()?.lods().template()?;
            let query = results.parent()?;
            let resolved = query.resolved();
            let path = if resolved.is_null() { query.requested() } else { resolved };
            let path = super::ui::c_string(path, PATH_LIMIT)?.to_bytes();
            Some(Created {
                template: TemplateId(NonZeroU32::new(template.address())?),
                path,
                emitters: read(results.count()),
            })
        }
    }
}

/// [`start`] has been asked (once only, whatever it answered).
static STARTED: AtomicBool = AtomicBool::new(false);

/// Why [`start`] left the census off: nothing written, every page as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartRefused {
    /// Not memory.arenas_ready's context at the process's start, or not the
    /// first time.
    NotTheStart,
    /// A site (its bytes, its page) refused.
    Site(super::InstallRefused),
}

/// Switches the census on, at memory.arenas_ready alone: writes the jumps
/// of `particle_census.toml`'s two hooks installed on demand
/// (particle_world::tick's start, on_materials_loaded's start), every
/// site's bytes checked and every page made writable first. A refusal
/// leaves the game as it was.
///
/// Its precondition is checked, not trusted
/// ([`at_start`](super::memory_profile::at_start)): there only the main
/// thread runs the game's code, and no particle world or cook exists yet,
/// so no thread runs the sites' code while they are written.
pub fn start(ctx: &Context<'_>) -> Result<(), StartRefused> {
    if !super::memory_profile::at_start(ctx) || STARTED.swap(true, Ordering::AcqRel) {
        return Err(StartRefused::NotTheStart);
    }
    let sites = super::particle_census_on_demand();
    // SAFETY: checked above: inside allocate_region at the start, the main
    // thread alone runs the game's code until the jumps are written.
    let unlocked = unsafe { super::hook::unlock_on_demand(ctx.thread(), &sites) }.map_err(StartRefused::Site)?;
    unlocked.write();
    Ok(())
}
