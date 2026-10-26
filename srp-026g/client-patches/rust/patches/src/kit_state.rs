//! Patch 20: a weapon's state that a respawn kept (client_patch.md section
//! 3.20; patch_026g.py KIT_STATE).
//!
//! A world holds the current kit's player alone. A game that loaded one
//! during a match has the other kits' players as they were built; the games
//! that played them have them as their last life left them. A weapon keeps
//! through the removal before a spawn, and the initialize does not set
//! again, the time it was last ticked, the fire mode and the ammunition
//! kind (seen in the players' server's saved worlds). So at every spawn
//! each weapon of the new kit's player forgets its last tick, and at a
//! spawn into another kit than the one before it is also given the
//! constructor's values of what it keeps, before the initialize; a respawn
//! into the same kit keeps what the player chose (that kit's player is in
//! every world, so every game has it alike).

#![forbid(unsafe_code)]

use crate::vostok::spawn::Spawning;
use crate::vostok::{Context, Flow, NoExit};

/// `spawn.kit_state`: the new kit is current, its player is about to be
/// initialised. Then the two instructions replaced (Continue); the x87's
/// fldz the spawn keeps across the site is the entry's to keep.
pub fn on_spawn(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let spawn = Spawning::at(ctx);
    if let Some(player) = spawn.player() {
        let changes_kit = spawn.changes_kit();
        for (_, weapon) in player.weapons() {
            weapon.forget_last_tick();
            if changes_kit {
                weapon.reset_as_constructed();
            }
        }
    }
    Flow::Continue
}
