//! Patch 82: a world loaded from the server puts its alive players into it.
//!
//! Only a spawn inserts a player (player::insert 0x1F2720 sets
//! m_has_been_inserted, activates its physics and links the world's tick
//! lists); a world loaded from a 0xE8 (base_player::deserialize 0x2DB490)
//! sets m_is_alive alone. So a client that joins a match late, or one
//! resynced while a player it had dead respawned, holds alive players
//! outside its world until their next spawn: missing on its minimap and Tab
//! map (update_minimap_players 0x239420 skips them at 0x2394D1), without
//! their 3D icons (0x1F2AE0), physics or place in the rule's living list
//! (0x3168D9, 0x316987). Right after the load, every such player is put into
//! the world as its spawn did on the other clients (docs/notes/
//! late-join-insert.md).

#![forbid(unsafe_code)]

use crate::vostok::world::NetworkClient;
use crate::vostok::{Context, Flow, NoExit};

/// process_dynamic_match_info after game_world_core::deserialize (0x27C116).
pub fn on_world_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(client) = NetworkClient::at_world_loaded(ctx) {
        for place in 0..client.places() {
            if let Some(player) = client.player(place) {
                player.enter_world();
            }
        }
    }
    Flow::Continue
}
