//! The use prompt's native state (patch 195, crate::use_prompt): game_world_ui's text key last
//! given root.set_context (+334h), at set_using_progress_message's start (0x23A430).
use super::Context;
use crate::raw;

/// No text given yet, as initialize_match leaves it (0x234ACC): set_using_info_message gives the
/// next prompt, whatever its text.
const NONE: u32 = 0;

/// The ui's last prompt text forgotten.
pub fn forget_text(ctx: &Context<'_>) {
    use raw::site::use_prompt_progress as site;
    // SAFETY: the site is the function's start: ESP+4 its ui, the game world's live hud.
    unsafe {
        if let Some(ui) = site::frame(ctx).and_then(|frame| frame.ui()) {
            ui.prompt_text().write_volatile(NONE);
        }
    }
}
