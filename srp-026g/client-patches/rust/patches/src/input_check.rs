//! Patch 99: a world takes no player input whose rotation no client makes,
//! the client-side twin of the match server's check
//! (`PlayerInput::rotation_is_sane`).
//!
//! 0.26g's change_input<player_input> (0x1FAB50) takes any input: the two
//! halves of its rotation are added to that player's angles by every world
//! (0x2D89A4 -> 0x2D8AD0), so a NaN or an infinity stays in the player's
//! state, in every world of the match and every snapshot made of it. 0.26i
//! fixed "crashes of all clients ... 'bad' input of one of the clients": in
//! 0.27d change_input first calls a validator (0x1EA990, the two halves made
//! floats and refused when either is a NaN or an infinity) and drops a
//! refused input with "change input (client %s) request REJECTED, reason:
//! input is invalid", returning the event action 6. So does this hook, at
//! the same place, for every input a world takes (local, relayed, moved, a
//! replay's): every world refuses the same inputs, so they stay the same.
//! The rule is the server's (under 1024 radians, a superset of 0.27d's), so
//! an input the server relays is never refused here.
//!
//! What a match computes: `simulation_changes.tsv`. The proof is
//! client_patch.md section 3.62.

#![forbid(unsafe_code)]

use crate::vostok::log::{self, Piece::*};
use crate::vostok::{Context, Flow, InputChange, InputCheckOnChangeInputExit};

/// change_input<player_input>'s start (0x1FAB50).
pub fn on_change_input(ctx: &mut Context<'_>) -> Flow<InputCheckOnChangeInputExit> {
    let Some(change) = InputChange::at(ctx) else { return Flow::Continue };
    if change.rotation().is_sane() {
        return Flow::Continue;
    }
    log::line(&[
        Text("input_check: place "),
        Dec(u32::from(change.place())),
        Text("'s input at "),
        Dec(change.time()),
        Text(" ms has a rotation no client makes ("),
        Hex(change.rotation().bits()),
        Text("), not taken"),
    ]);
    change.refuse();
    Flow::Exit(InputCheckOnChangeInputExit::Refused)
}
