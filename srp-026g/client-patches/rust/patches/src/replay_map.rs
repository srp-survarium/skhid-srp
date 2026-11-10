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
//!   (true in a replay, false in a live match): its enemies' markers show,
//!   whatever enemy list it is given.
//!
//! A live match, and a live game's spectator, see the enemies as the game
//! shows them. What is watched is set by the match's options read: a
//! replay's chunk 1 (`replay.options_read`) or a live match's 0xE4
//! (`static_info.options_read`), each before the match's world and its
//! initialize_match. The interface only: nothing a match computes.

#![forbid(unsafe_code)]

use crate::vostok::{
    Context, Flow, GameCell, GameWorldUi, NoExit, ReplayMapOnEnemyExit, ReplayMapOnNameExit, Thread,
};

/// What the game shows: a live match or a replay. A byte, 0 or 1:
/// check_replay_map_026g.py writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Watching {
    /// A match played or spectated live (and before any match).
    Live,
    /// A `.sur` replay.
    Replay,
}

impl Watching {
    /// An enemy the game would show only while spotted: shown whatever.
    fn shows_every_enemy(self) -> bool {
        self == Watching::Replay
    }
}

static WATCHING: GameCell<Watching> = GameCell::new(Watching::Live);

fn watching(ctx: &Context<'_>) -> Watching {
    WATCHING.get(ctx.thread())
}

fn watch(thread: &Thread, watching: Watching) -> Flow<NoExit> {
    WATCHING.set(thread, watching);
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    watch(ctx.thread(), Watching::Live)
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    watch(ctx.thread(), Watching::Replay)
}

/// update_minimap_players at an enemy's spotted test (0x239505).
pub fn on_enemy(ctx: &mut Context<'_>) -> Flow<ReplayMapOnEnemyExit> {
    if watching(ctx).shows_every_enemy() { Flow::Exit(ReplayMapOnEnemyExit::Shown) } else { Flow::Continue }
}

/// render_name at an enemy's spotted test (0x1F2B50).
pub fn on_name(ctx: &mut Context<'_>) -> Flow<ReplayMapOnNameExit> {
    if watching(ctx).shows_every_enemy() { Flow::Exit(ReplayMapOnNameExit::Shown) } else { Flow::Continue }
}

/// initialize_match as it sends the icons movie its enemy list (0x235709,
/// just before): the movie told whether to show them, before any marker.
pub fn on_enemy_ids(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let shown = watching(ctx).shows_every_enemy();
    if let Some(movie) = GameWorldUi::at_enemy_ids(ctx).and_then(|ui| ui.icons_movie()) {
        movie.invoke_bool(c"root.srp_show_enemies", shown);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_match_leaves_the_enemies_to_the_game() {
        assert!(!Watching::Live.shows_every_enemy());
    }

    #[test]
    fn a_replay_shows_every_enemy() {
        assert!(Watching::Replay.shows_every_enemy());
    }
}
