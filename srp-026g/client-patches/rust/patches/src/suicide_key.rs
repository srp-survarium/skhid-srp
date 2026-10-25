//! Patch 34: the commit_suicide key (K) kills one's own player.
//!
//! The input builder never asks for commit_suicide, so the input's suicide
//! bit, which the match simulation still reads, is never set. Hook: the
//! builder's last action call, 0x242196 (`RUST_HOOKS` in patch_026g.py),
//! before it. The proof: client_patch.md section 3.32.

#![forbid(unsafe_code)]

use crate::vostok::{Action, ActionMask, ActionState, InputHandler};

/// The key went down: this input kills the player.
pub fn on_input_built(handler: &mut InputHandler) {
    if handler.action_state(Action::CommitSuicide) == Some(ActionState::Down) {
        handler.add_actions(ActionMask::SUICIDE);
    }
}
