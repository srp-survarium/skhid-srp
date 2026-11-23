//! A sprint's tests in the simulation, for patch 201 (sprint_fire.rs;
//! docs/notes/muzzle-shots.md, "A shot in a sprint"):
//!
//! - the input mask at the three inlined copies of player_input::is_sprinting
//!   (`sprint.keys_*`): EAX the mask, SPRINT and FORWARD already found in it,
//!   about to be refused with any of BACK, FIRE and CROUCH (`test eax,122h`);
//! - the weapon at weapon_core's fire predicate (`weapon.fire_wait`, 0x2E3CD3)
//!   and at its can_sprint (`weapon.can_sprint`, 0x2E3A43): ESI.

use super::context::{Context, Reg};
use super::world::read;
use crate::raw;

/// The sprint clip the weapon waits out before it fires again: none (the
/// weapon's +5ECh is 6; 0, 2 or 4 while a sprint's clips are selected and
/// until the last one ends, weapon_core 0x2E2639).
const NO_SPRINT_CLIP: u32 = 6;
/// The weapon state's id (+124h) of the fire state.
const FIRE: u32 = 5;

/// The input's action mask at a `sprint.keys_*` site.
pub fn keys(ctx: &Context<'_>) -> u32 {
    ctx.reg(Reg::Eax)
}

/// A weapon as a sprint's tests read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprintingGun {
    /// A sprint's clip is selected or still running (the weapon's +5ECh).
    pub sprint_clip: bool,
    /// Its user's posture is the sprint (the posture's +2Ch,
    /// weapon_core::is_sprinting 0x2E30AC).
    pub sprinting: bool,
    /// It is in its fire state.
    pub firing: bool,
}

impl SprintingGun {
    pub fn at_fire_wait(ctx: &Context<'_>) -> Option<SprintingGun> {
        // SAFETY: the context is this site's: ESI weapon_core::fire_pred's
        // weapon, alive for the call.
        unsafe { SprintingGun::of(raw::site::weapon_fire_wait::weapon(ctx)?) }
    }

    pub fn at_can_sprint(ctx: &Context<'_>) -> Option<SprintingGun> {
        // SAFETY: the context is this site's: ESI weapon_core::can_sprint's
        // weapon, alive for the call.
        unsafe { SprintingGun::of(raw::site::weapon_can_sprint::weapon(ctx)?) }
    }

    /// # Safety
    /// `weapon` is alive.
    unsafe fn of(weapon: raw::Weapon) -> Option<SprintingGun> {
        // SAFETY: the live weapon's own word, its clip set's posture now
        // (fire_pred reads both at 0x2E3CC4..0x2E3CD3) and its state
        // (can_sprint's at 0x2E3A43).
        unsafe {
            Some(SprintingGun {
                sprint_clip: read(weapon.state_5ec()) != NO_SPRINT_CLIP,
                sprinting: read(weapon.user_animations()?.posture()?.sprinting()) != 0,
                firing: read(weapon.state()?.id()) == FIRE,
            })
        }
    }
}
