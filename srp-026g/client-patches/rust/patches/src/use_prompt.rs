//! Patch 195: a use's progress bar ends with the use (docs/notes/client_patch.md "The use prompt
//! after a use").
//!
//! The hud's one prompt widget (root.set_context) shows either what the local player can use or
//! the progress of a use. The game sets the prompt again only when its text changes from the one
//! it gave last (game_world_ui +334h), and the progress never changes that text. So a use that
//! ends while the prompt's text is the one before the use (a death, a finished or broken use
//! with nothing usable in view, after a use begun without the use's own prompt drawn) leaves the
//! progress bar on screen until another prompt comes. Each frame a use shows its progress, the
//! remembered text is forgotten: the first prompt after the use is given again, and it replaces
//! the bar. Only the hud: nothing a match computes.
#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, NoExit, use_prompt};

/// set_using_progress_message's start: the prompt's last text forgotten.
pub fn on_progress(ctx: &mut Context<'_>) -> Flow<NoExit> {
    use_prompt::forget_text(ctx);
    Flow::Continue
}
