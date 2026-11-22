//! Patch 194: the markers over thrown grenades tell a smoke and a toxic smoke apart
//! (docs/notes/grenade-icons.md). The game marks the followed player's team's grenades within 7 m,
//! all with the frag's glyph; the marker of a smoke grenade, and of one thrown with Toxic Smoke,
//! is now told its kind (the player icons movie's MarkerType). Enemies' grenades stay unmarked.
//! Presentation only.

#![forbid(unsafe_code)]

use crate::vostok::grenade_icons::{self, Sensed};
use crate::vostok::{Context, Flow, NoExit};

/// A teammate's grenade took a marker (`grenade_icons.sensed`).
pub fn on_sensed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(sensed) = Sensed::at(ctx) {
        sensed.tell(ctx.thread());
    }
    Flow::Continue
}

/// initialize_match is about to make the match's markers (`player_icons.enemy_ids`).
pub fn on_markers_made(ctx: &mut Context<'_>) -> Flow<NoExit> {
    grenade_icons::markers_made(ctx.thread());
    Flow::Continue
}
