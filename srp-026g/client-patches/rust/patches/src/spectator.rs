//! Patch 103: a game sent to a match with no place of its own watches it
//! (the rooms' queue, docs/notes/queue-spectate.md; client_patch.md 3.67).
//! The match server's 0xE4 names no place local; the game has no player and
//! sends no input. Its camera follows a player of any team, as the game's
//! spectate_next_player does (mode 4, the player's eyes), F6 and F5 (the
//! game's next_player and prev_player) the next or the previous one; an 0xEA
//! naming place 0xFF (the server's farewell) ends its match. A game
//! with a place of its own runs the game's code unchanged at every site.
//!
//! Nothing a match computes changes: a spectator's game builds and sends no
//! input (the client's +0Ch, as a replay's), and the server takes none from
//! it; the sites move the camera and the hud alone.
#![forbid(unsafe_code)]

use crate::vostok::spectate::{world_camera_client, FollowedAlive, MatchLine, PlayerLeft, SpectateKey};
use crate::vostok::world::{Role, Step};
use crate::vostok::{
    Context, Flow, NoExit, SpectatorOnKeyExit, SpectatorOnMatchLineExit, SpectatorOnPlayerLeftExit,
    SpectatorOnWorldCameraExit,
};

/// `world_load.camera`: after a world. A player's game: the game's own
/// camera back to its place (and its input on). A spectator's: the player
/// it follows kept while that one is in the match, else the first one, and
/// none of the game's own steps (they ask for the place it does not have).
pub fn on_world_camera(ctx: &mut Context<'_>) -> Flow<SpectatorOnWorldCameraExit> {
    let Some(client) = world_camera_client(ctx) else {
        return Flow::Continue;
    };
    match client.take_role() {
        Some(Role::Spectator) => {
            if !client.local_player().is_some_and(|player| player.inserted()) {
                client.follow(Step::First);
            }
            Flow::Exit(SpectatorOnWorldCameraExit::Done)
        }
        Some(Role::Player) | None => Flow::Continue,
    }
}

/// `keyboard.spectate`: a spectator's F6 and F5 move its camera to the next
/// or the previous player, of any team; every other key and every player's
/// game go on as the game's.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<SpectatorOnKeyExit> {
    let Some(key) = SpectateKey::at(ctx) else {
        return Flow::Continue;
    };
    match (key.client.role(), key.step) {
        (Some(Role::Spectator), Some(step)) => {
            key.client.follow(step);
            Flow::Exit(SpectatorOnKeyExit::Taken)
        }
        _ => Flow::Continue,
    }
}

/// `chat.match_team`: a spectator has no team; its match lines are all
/// plain (the game would read a player of the place it does not have).
pub fn on_match_line(ctx: &mut Context<'_>) -> Flow<SpectatorOnMatchLineExit> {
    let Some(line) = MatchLine::at(ctx) else {
        return Flow::Continue;
    };
    match line.client.role() {
        Some(Role::Spectator) => {
            line.as_plain();
            Flow::Exit(SpectatorOnMatchLineExit::Plain)
        }
        Some(Role::Player) | None => Flow::Continue,
    }
}

/// `player_left.local`: an 0xEA, after leave_match. A spectator's game
/// takes one naming place 0xFF as its own end (the server's farewell: it is
/// taken in as a player, or its match ended) and goes back to the lobby with
/// no results screen; one naming the player it follows moves its camera to
/// the next. Every other 0xEA, and every player's game, go on as the game's
/// (a spectator's: nothing, 0x27C734).
pub fn on_player_left(ctx: &mut Context<'_>) -> Flow<SpectatorOnPlayerLeftExit> {
    let Some(left) = PlayerLeft::at(ctx) else {
        return Flow::Continue;
    };
    if left.client.role() != Some(Role::Spectator) {
        return Flow::Continue;
    }
    if left.place == NO_PLACE {
        left.client.stop_watching();
        return Flow::Exit(SpectatorOnPlayerLeftExit::Done);
    }
    if left.client.local_player().is_some_and(|player| player.place() == left.place) {
        left.client.follow(Step::Next);
        return Flow::Exit(SpectatorOnPlayerLeftExit::Done);
    }
    Flow::Continue
}

/// The place an 0xEA names for a spectator's own end: the local place it
/// does not have.
const NO_PLACE: u8 = 0xFF;

/// `hud.followed_alive`: a spectator's hud takes the player it follows for
/// alive, so a death shows it no kits and no cursor (a kit picked there
/// would be the followed player's, in its own world alone).
pub fn on_followed_alive(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let mut site = FollowedAlive::at(ctx);
    if site.client().and_then(|client| client.role()) != Some(Role::Spectator) {
        return Flow::Continue;
    }
    site.as_alive();
    Flow::Skip
}

#[cfg(test)]
mod tests {
    use crate::vostok::world::{spectate_order, Step};
    use std::vec::Vec;

    fn order(places: u32, current: Option<u8>, step: Step) -> Vec<u32> {
        spectate_order(places, current, step).collect()
    }

    #[test]
    fn a_first_camera_tries_every_place_from_0() {
        assert_eq!(order(4, None, Step::First), [0, 1, 2, 3]);
        assert_eq!(order(4, Some(2), Step::First), [0, 1, 2, 3]);
        assert_eq!(order(0, None, Step::First), [] as [u32; 0]);
    }

    #[test]
    fn next_and_previous_go_round_without_the_one_followed() {
        assert_eq!(order(4, Some(2), Step::Next), [3, 0, 1]);
        assert_eq!(order(4, Some(0), Step::Previous), [3, 2, 1]);
        assert_eq!(order(1, Some(0), Step::Next), [] as [u32; 0]);
    }

    #[test]
    fn a_step_from_nobody_or_past_the_places_is_a_first() {
        assert_eq!(order(3, None, Step::Next), [0, 1, 2]);
        assert_eq!(order(3, Some(7), Step::Previous), [0, 1, 2]);
    }
}
