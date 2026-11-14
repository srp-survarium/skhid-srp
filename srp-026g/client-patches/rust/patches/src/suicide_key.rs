//! Patch 34: the commit_suicide key (K) kills one's own player.
//!
//! The input builder never asks the handler for commit_suicide, so the
//! input's suicide bit, which the match simulation still reads, is never
//! set. This patch wraps the builder's last action query (spot, the `call`
//! at 0x242196, `<patch>.toml`): before it, ask whether commit_suicide went
//! down and, if so, set the bit; then do the spot query the site asked for.
//! The proof is client_patch.md section 3.32.

#![forbid(unsafe_code)]

use crate::vostok::{Action, ActionMask, ActionState, InputHandler, OrigActionPresent, Out};

/// The builder's spot query at 0x242196. `spot` is the game's own
/// action_present, to call for the spot the site wanted.
pub fn on_spot(spot: &OrigActionPresent, handler: &mut InputHandler, action: Action, state: &mut Out<u32>) -> bool {
    if handler.action_state(Action::COMMIT_SUICIDE) == Some(ActionState::Down) {
        handler.add_actions(ActionMask::SUICIDE);
    }
    spot.call(handler, action, state)
}
