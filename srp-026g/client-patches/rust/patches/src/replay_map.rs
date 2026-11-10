//! Patch 135: a replay's minimap and Tab map show the enemies too
//! (docs/notes/replay-map.md, README "A replay's map shows every player").
//!
//! update_minimap_players (0x239420) lists a place's player when it is alive
//! and in the world, and on the local player's team or spotted (+348h above
//! 0). In a replay the local player is the one the camera is on, so its map
//! showed that player's team and only the enemies it had spotted. A replay
//! is watched after the match: at the spotted test (0x239505) every enemy
//! is listed, a red icon of the hud movie's own (`player_red`). A live match,
//! and a live game's spectator, see the enemies as the game shows them.
//!
//! What is watched is set by the match's options read: a replay's chunk 1
//! (`replay.options_read`) or a live match's 0xE4 (`static_info.options_read`),
//! each before the match's world is made. The interface only: nothing a
//! match computes.

#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, GameCell, NoExit, ReplayMapOnEnemyExit, Thread};

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

static WATCHING: GameCell<Watching> = GameCell::new(Watching::Live);

/// What the spotted test does with an enemy it is given.
fn enemy(watching: Watching) -> Flow<ReplayMapOnEnemyExit> {
    match watching {
        Watching::Live => Flow::Continue,
        Watching::Replay => Flow::Exit(ReplayMapOnEnemyExit::Shown),
    }
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
    enemy(WATCHING.get(ctx.thread()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_match_leaves_the_spotted_test_to_the_game() {
        assert!(matches!(enemy(Watching::Live), Flow::Continue));
    }

    #[test]
    fn a_replay_lists_every_enemy() {
        assert!(matches!(enemy(Watching::Replay), Flow::Exit(ReplayMapOnEnemyExit::Shown)));
    }
}
