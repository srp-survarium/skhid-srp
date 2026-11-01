//! Patch 90: tactical reloads (docs/notes/tactical-reload.md). A two-clip
//! reload (state type 314) plays its tactical clip with rounds left in the
//! magazine and its empty clip without, as the game always has; retail
//! 0.69d0 also gives each its own time, `parameters/tactical_reload_time`
//! and `reload_time`, where 0.26g plays both at the tactical clip's speed
//! over `reload_time`. A weapon whose config has `tactical_reload_time` is
//! timed as retail's; one without keeps the game's.
//!
//! The clip a reload plays is the game's choice; only its speed is ours,
//! computed once, when the weapon's states are cooked, from its config: every
//! client of a match times a reload alike.

#![forbid(unsafe_code)]

use crate::vostok::reload::{Magazine, ReloadCook, ReloadLexeme, ReloadTiming};
use crate::vostok::{Context, Flow, NoExit};

/// The state's construction: its two speeds from the config.
pub fn on_constructed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(cook) = ReloadCook::at(ctx) {
        cook.time(cook.times());
    }
    Flow::Continue
}

/// The hands clip's speed, before the player's reload modifier: the
/// tactical clip's when the magazine has rounds left.
pub fn on_lexeme_time_scale(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(lexeme) = ReloadLexeme::at(ctx)
        && let ReloadTiming::Split { tactical } = lexeme.timing()
        && lexeme.magazine() == Some(Magazine::RoundsLeft)
    {
        lexeme.set_time_scale(tactical);
    }
    Flow::Continue
}
