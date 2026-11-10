//! Patch 135: a replay shows the enemies too: their dots on the minimap and
//! the Tab map, their markers over their heads and, with Alt, their names in
//! red (docs/notes/replay-map.md, README "A replay shows every player").
//!
//! The game shows an enemy only while it is spotted (+348h above 0), in two
//! places: update_minimap_players (0x239420) lists its dot, and render_name
//! (0x1F2AD0) places its marker and, while left Alt is held, its name, red
//! (0xFF0000, 0x1F300F). The player icons movie also never shows an enemy's
//! marker (`HUDIconLayer.canShow`, the list initialize_match sends it as
//! `root.set_enemy_icon_ids`). In a replay the local player is the one the
//! camera is on, so the replay showed that player's team and little else. A
//! replay is watched after the match:
//!
//! - at both spotted tests (0x239505, 0x1F2B50) every enemy goes on as a
//!   spotted one;
//! - as initialize_match sends the enemy list (0x235709, just before), before
//!   any marker of the match, the icons movie is told `root.srp_show_enemies`
//!   (true in a replay, false in a match): its enemies' markers show,
//!   whatever enemy list it is given.
//!
//! **A match never gets this.** There is no state of ours that says "a
//! replay": each hook asks the game's own network client what it is, by its
//! class (`vostok::world::Playback`: the replay_network_client's vtable, which
//! only a `.sur` replay's client has). A live match, and a live game's
//! spectator, are played by the network_client, and the hooks then leave the
//! game's tests as they are. Making a live game take this path means making
//! its network client another class, which ends its match; it is no easier
//! than changing the game's own test. The interface only: nothing a match
//! computes.

#![forbid(unsafe_code)]

use crate::vostok::world::{GameWorld, Playback};
use crate::vostok::{Context, Flow, GameWorldUi, NoExit, ReplayMapOnEnemyExit, ReplayMapOnNameExit};

/// An enemy the game would show only while spotted: shown whatever, in a replay.
fn shows_every_enemy(playback: Option<Playback>) -> bool {
    playback == Some(Playback::Replay)
}

/// update_minimap_players at an enemy's spotted test (0x239505).
pub fn on_enemy(ctx: &mut Context<'_>) -> Flow<ReplayMapOnEnemyExit> {
    if shows_every_enemy(GameWorld::at_minimap_enemy(ctx).map(|world| world.playback())) {
        return Flow::Exit(ReplayMapOnEnemyExit::Shown);
    }
    Flow::Continue
}

/// render_name at an enemy's spotted test (0x1F2B50).
pub fn on_name(ctx: &mut Context<'_>) -> Flow<ReplayMapOnNameExit> {
    if shows_every_enemy(GameWorld::at_name_enemy(ctx).map(|world| world.playback())) {
        return Flow::Exit(ReplayMapOnNameExit::Shown);
    }
    Flow::Continue
}

/// initialize_match as it sends the icons movie its enemy list (0x235709,
/// just before): the movie told whether to show them, before any marker.
pub fn on_enemy_ids(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let shown = shows_every_enemy(GameWorld::at_enemy_ids(ctx).map(|world| world.playback()));
    if let Some(movie) = GameWorldUi::at_enemy_ids(ctx).and_then(|ui| ui.icons_movie()) {
        movie.invoke_bool(c"root.srp_show_enemies", shown);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_replay_shows_every_enemy() {
        assert!(shows_every_enemy(Some(Playback::Replay)));
        assert!(!shows_every_enemy(Some(Playback::Match)));
        assert!(!shows_every_enemy(None));
    }
}
