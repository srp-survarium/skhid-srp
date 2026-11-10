//! The simulation's steps, rollbacks and live bullets as patch 138's census
//! counts them (`patches::particle_census`), each site's registers read once
//! here into what they mean. Nothing here writes the game's memory or calls
//! the engine; the sites are installed with the census
//! (`vostok::particle::start`).

use super::context::Context;
use super::world::read;
use crate::raw;

/// The simulation's grid: a tick every 50 ms.
const GRID_MS: u32 = 50;

/// What game_world_core::atomic_tick is about to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A step that ends on the grid (its time a multiple of 50 ms): the
    /// game's own test, which its tick lists get as their grid flag.
    Grid,
    /// A part of a step, ending between two grid times.
    Part,
}

impl Step {
    fn ending(ms: u32) -> Step {
        if ms.is_multiple_of(GRID_MS) { Step::Grid } else { Step::Part }
    }

    /// `simulation.atomic_tick`: EDI the step's end time.
    pub fn at_site(ctx: &Context<'_>) -> Step {
        // SAFETY: the context is the site's: a register's value.
        Step::ending(unsafe { raw::site::simulation_atomic_tick::now(ctx) })
    }
}

/// A kept state game_world_core::move loads over the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    /// A rollback: the state is older than where the simulation stood, and
    /// that many grid steps (a begun one counts) are played again.
    Back { grid_steps: u32 },
    /// A state not older than the simulation's time.
    Other,
}

impl Load {
    /// The simulation at `from` loads a state of `to` (ms; the clock wraps).
    fn between(from: u32, to: u32) -> Option<Load> {
        match from.wrapping_sub(to) as i32 {
            0 => None,
            behind if behind > 0 => Some(Load::Back { grid_steps: (behind as u32).div_ceil(GRID_MS) }),
            _ => Some(Load::Other),
        }
    }

    /// `simulation.reload`: the state is loaded whatever its time.
    pub fn at_reload(ctx: &Context<'_>) -> Load {
        // SAFETY: the context is the site's: EBX the core, ESI the item.
        let times = unsafe { times(raw::site::simulation_reload::core(ctx), raw::site::simulation_reload::item(ctx)) };
        times.and_then(|(from, to)| Load::between(from, to)).unwrap_or(Load::Other)
    }

    /// `simulation.reload_late`: None when the state's time is the core's,
    /// and the game does not load it.
    pub fn at_reload_late(ctx: &Context<'_>) -> Option<Load> {
        // SAFETY: as at_reload, the other site's registers.
        let times = unsafe { times(raw::site::simulation_reload_late::core(ctx), raw::site::simulation_reload_late::item(ctx)) };
        times.and_then(|(from, to)| Load::between(from, to))
    }
}

/// The core's time and the history item's.
///
/// # Safety
/// Both are alive: move's own, at one of its reload sites.
unsafe fn times(core: Option<raw::CoreWorld>, item: Option<raw::HistoryItem>) -> Option<(u32, u32)> {
    // SAFETY: the caller's.
    unsafe { Some((read(core?.time()), read(item?.time()))) }
}

/// `simulation.bullets`: the manager's live bullets at a grid tick.
pub fn live_bullets(ctx: &Context<'_>) -> Option<u32> {
    // SAFETY: the context is the site's: ESI the manager whose tick runs;
    // its vector's two words are read as numbers only.
    unsafe {
        let manager = raw::site::simulation_bullets::manager(ctx)?;
        Some(read(manager.bullets_end()).wrapping_sub(read(manager.bullets())) / 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_ending_on_a_multiple_of_50_ms_is_a_grid_step() {
        assert_eq!([0, 50, 1000, 20, 49, 51].map(Step::ending), [Step::Grid, Step::Grid, Step::Grid, Step::Part, Step::Part, Step::Part]);
    }

    #[test]
    fn a_load_of_an_older_state_is_a_rollback_of_its_grid_steps() {
        assert_eq!(Load::between(1100, 1000), Some(Load::Back { grid_steps: 2 }));
        assert_eq!(Load::between(1100, 980), Some(Load::Back { grid_steps: 3 }));
        assert_eq!(Load::between(1100, 1099), Some(Load::Back { grid_steps: 1 }));
        assert_eq!(Load::between(1100, 1100), None);
        assert_eq!(Load::between(1100, 1150), Some(Load::Other));
        // the clock's wrap
        assert_eq!(Load::between(30, u32::MAX - 19), Some(Load::Back { grid_steps: 1 }));
    }
}
