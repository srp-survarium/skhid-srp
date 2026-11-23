//! Patch 201: a shot in a sprint (docs/notes/muzzle-shots.md, "A shot in a
//! sprint"). The owner, 2026-11-23: "you should also be able to shoot while
//! sprinting, it is just that weapon is not looking straight".
//!
//! The game ends a sprint at a trigger's press and lets the weapon fire only
//! once the sprint's clip has run out. Three things, all the simulation's:
//!
//! - FIRE no longer denies a sprint: the three inlined copies of
//!   player_input::is_sprinting (the speed and the stamina, the posture, the
//!   jump) refuse BACK and CROUCH only;
//! - the weapon fires in a sprint: its fire predicate does not wait for the
//!   sprint's clip while its user's posture is the sprint (after a sprint it
//!   waits as it did);
//! - a fire begun in a sprint keeps the sprint: can_sprint takes the fire
//!   state while a sprint's clip is selected, as it takes a melee's. A fire
//!   begun outside a sprint starts none.
//!
//! A match plays out otherwise than before: simulation_changes.tsv.

#![forbid(unsafe_code)]

use crate::vostok::sprint::{self, SprintingGun};
use crate::vostok::{
    Context, Flow, SprintFireOnCanSprintExit, SprintFireOnFireWaitExit, SprintFireOnJumpKeysExit, SprintFireOnPostureKeysExit,
    SprintFireOnSpeedKeysExit,
};

/// The input's actions that still deny a sprint.
const BACK: u32 = 0x2;
const CROUCH: u32 = 0x100;

/// Whether a mask with SPRINT and FORWARD sprints.
pub fn sprints(keys: u32) -> bool {
    keys & (BACK | CROUCH) == 0
}

/// is_trying_to_sprint's test (the sprint's speed and stamina).
pub fn on_speed_keys(ctx: &mut Context<'_>) -> Flow<SprintFireOnSpeedKeysExit> {
    if sprints(sprint::keys(ctx)) { Flow::Exit(SprintFireOnSpeedKeysExit::Sprints) } else { Flow::Continue }
}

/// sprint_predicate's test (the posture machine).
pub fn on_posture_keys(ctx: &mut Context<'_>) -> Flow<SprintFireOnPostureKeysExit> {
    if sprints(sprint::keys(ctx)) { Flow::Exit(SprintFireOnPostureKeysExit::Sprints) } else { Flow::Continue }
}

/// player_input::is_sprinting's own test (a sprint's jump).
pub fn on_jump_keys(ctx: &mut Context<'_>) -> Flow<SprintFireOnJumpKeysExit> {
    if sprints(sprint::keys(ctx)) { Flow::Exit(SprintFireOnJumpKeysExit::Sprints) } else { Flow::Continue }
}

/// Whether the fire predicate goes on past its wait for the sprint's clip.
pub fn fires(gun: SprintingGun) -> bool {
    gun.sprint_clip && gun.sprinting
}

/// Whether can_sprint takes a weapon that is not idle.
pub fn keeps_sprint(gun: SprintingGun) -> bool {
    gun.firing && gun.sprint_clip
}

/// weapon_core's fire predicate at its wait for the sprint's clip.
pub fn on_fire_wait(ctx: &mut Context<'_>) -> Flow<SprintFireOnFireWaitExit> {
    match SprintingGun::at_fire_wait(ctx) {
        Some(gun) if fires(gun) => Flow::Exit(SprintFireOnFireWaitExit::Fires),
        _ => Flow::Continue,
    }
}

/// weapon_core::can_sprint at its weapon state's test.
pub fn on_can_sprint(ctx: &mut Context<'_>) -> Flow<SprintFireOnCanSprintExit> {
    match SprintingGun::at_can_sprint(ctx) {
        Some(gun) if keeps_sprint(gun) => Flow::Exit(SprintFireOnCanSprintExit::Sprints),
        _ => Flow::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORWARD: u32 = 0x1;
    const FIRE: u32 = 0x20;
    const SPRINT: u32 = 0x200;

    #[test]
    fn a_trigger_does_not_end_a_sprint() {
        assert!(sprints(SPRINT | FORWARD) && sprints(SPRINT | FORWARD | FIRE));
        assert!(!sprints(SPRINT | FORWARD | BACK) && !sprints(SPRINT | FORWARD | CROUCH | FIRE));
    }

    #[test]
    fn the_weapon_fires_in_a_sprint_and_waits_after_one() {
        let gun = |sprint_clip, sprinting, firing| SprintingGun { sprint_clip, sprinting, firing };
        assert!(fires(gun(true, true, false)));
        // after the sprint, its clip still running: the wait as it was
        assert!(!fires(gun(true, false, false)));
        // no sprint's clip: the predicate's own test passes
        assert!(!fires(gun(false, false, false)));
    }

    #[test]
    fn a_fire_begun_in_a_sprint_keeps_it_and_another_starts_none() {
        let gun = |sprint_clip, firing| SprintingGun { sprint_clip, sprinting: sprint_clip, firing };
        assert!(keeps_sprint(gun(true, true)));
        assert!(!keeps_sprint(gun(false, true)) && !keeps_sprint(gun(true, false)));
    }
}
