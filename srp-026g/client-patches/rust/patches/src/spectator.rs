//! Patch 103: a game sent to a match with no place of its own watches it
//! (the rooms' queue, docs/notes/queue-spectate.md; client_patch.md 3.67).
//! The match server's 0xE4 names no place local; the game has no player and
//! sends no input. Its camera follows a player of any team, as the game's
//! spectate_next_player does (mode 4, the player's eyes), F6 and F5 (the
//! game's next_player and prev_player) the next or the previous one, F2 (the
//! replay's free view, cam_2) lets go and flies with the game's free-fly camera; an 0xEA
//! naming place 0xFF (the server's farewell) ends its match. A game
//! with a place of its own runs the game's code unchanged at every site.
//!
//! Nothing a match computes changes: a spectator's game builds and sends no
//! input (the client's +0Ch, as a replay's), and the server takes none from
//! it; the sites move the camera and the hud alone.
#![forbid(unsafe_code)]

use crate::vostok::spectate::{world_camera_client, CameraKey, FollowedAlive, MatchLine, PlayerLeft, SpectateKey};
use crate::vostok::world::{NetworkClient, Role, Step};
use crate::vostok::{
    Context, Flow, GameCell, NoExit, SpectatorOnKeyExit, SpectatorOnMatchLineExit, SpectatorOnPlayerLeftExit,
    SpectatorOnWorldCameraExit, Thread,
};

/// A spectator's camera, kept between hooks: it follows a player (the
/// game's own +4 names which, none before the first), or flies free since
/// its F2 (the game's free-fly camera, no player followed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Camera {
    Follows,
    FliesFree,
}

/// What happens to a spectator's camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    /// A match's options (0xE4): a new match, followed by nobody yet.
    NewMatch,
    /// A world was loaded; whether the player followed is in the match.
    World { followed_in_match: bool },
    /// F6/F5 or F2.
    Key(CameraKey),
    /// The player followed left the match (0xEA).
    FollowedLeft,
}

/// What the camera does for an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Nothing,
    Follow(Step),
    FlyFree,
}

/// The camera's one transition, total over the events: F2 lets go and
/// flies until F6/F5 (from the last one followed) or the next match; a
/// flying camera is kept over a world's load and a player's leave.
fn step(camera: Camera, event: Event) -> (Camera, Act) {
    match (camera, event) {
        (_, Event::NewMatch) => (Camera::Follows, Act::Nothing),
        (_, Event::Key(CameraKey::Follow(step))) => (Camera::Follows, Act::Follow(step)),
        (_, Event::Key(CameraKey::FlyFree)) => (Camera::FliesFree, Act::FlyFree),
        (Camera::FliesFree, Event::World { .. } | Event::FollowedLeft) => (Camera::FliesFree, Act::Nothing),
        (Camera::Follows, Event::World { followed_in_match: true }) => (Camera::Follows, Act::Nothing),
        (Camera::Follows, Event::World { followed_in_match: false }) => (Camera::Follows, Act::Follow(Step::First)),
        (Camera::Follows, Event::FollowedLeft) => (Camera::Follows, Act::Follow(Step::Next)),
    }
}

static CAMERA: GameCell<Camera> = GameCell::new(Camera::Follows);

/// A spectator's camera takes `event`.
fn camera(client: &NetworkClient<'_>, thread: &Thread, event: Event) {
    let (next, act) = step(CAMERA.get(thread), event);
    CAMERA.set(thread, next);
    match act {
        Act::Nothing => {}
        Act::Follow(step) => {
            client.follow(step);
        }
        Act::FlyFree => client.fly_free(),
    }
}

/// `static_info.options_read` (after patches 65, 86 and 58's): a new match.
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    // A new match's camera does nothing yet (Act::Nothing): its first world
    // finds it a player.
    CAMERA.update(ctx.thread(), |camera| step(camera, Event::NewMatch).0);
    Flow::Continue
}

/// `world_load.camera`: after a world. A player's game: the game's own
/// camera back to its place (and its input on). A spectator's: the player
/// it follows kept while that one is in the match, else the first one; a
/// free flight kept; none of the game's own steps (they ask for the place
/// it does not have).
pub fn on_world_camera(ctx: &mut Context<'_>) -> Flow<SpectatorOnWorldCameraExit> {
    let Some(client) = world_camera_client(ctx) else {
        return Flow::Continue;
    };
    match client.take_role() {
        Some(Role::Spectator) => {
            let followed_in_match = client.local_player().is_some_and(|player| player.inserted());
            camera(&client, ctx.thread(), Event::World { followed_in_match });
            Flow::Exit(SpectatorOnWorldCameraExit::Done)
        }
        Some(Role::Player) | None => Flow::Continue,
    }
}

/// `keyboard.spectate`: a spectator's F6 and F5 move its camera to the next
/// or the previous player, of any team; F2 lets go and flies free. Every
/// other key and every player's game go on as the game's.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<SpectatorOnKeyExit> {
    let Some(key) = SpectateKey::at(ctx) else {
        return Flow::Continue;
    };
    match (key.client.role(), key.key) {
        (Some(Role::Spectator), Some(camera_key)) => {
            camera(&key.client, ctx.thread(), Event::Key(camera_key));
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
        camera(&left.client, ctx.thread(), Event::FollowedLeft);
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
    use super::{step, Act, Camera, Event};
    use crate::vostok::spectate::CameraKey;
    use crate::vostok::world::{spectate_order, Step};
    use std::vec::Vec;

    #[test]
    fn f2_flies_until_f6_or_f5_or_the_next_match() {
        let fly = Event::Key(CameraKey::FlyFree);
        assert_eq!(step(Camera::Follows, fly), (Camera::FliesFree, Act::FlyFree));
        assert_eq!(step(Camera::FliesFree, fly), (Camera::FliesFree, Act::FlyFree));
        for s in [Step::Next, Step::Previous] {
            assert_eq!(step(Camera::FliesFree, Event::Key(CameraKey::Follow(s))), (Camera::Follows, Act::Follow(s)));
        }
        assert_eq!(step(Camera::FliesFree, Event::NewMatch), (Camera::Follows, Act::Nothing));
    }

    #[test]
    fn a_flight_is_kept_over_a_world_and_a_leave() {
        for followed_in_match in [false, true] {
            assert_eq!(step(Camera::FliesFree, Event::World { followed_in_match }), (Camera::FliesFree, Act::Nothing));
        }
        assert_eq!(step(Camera::FliesFree, Event::FollowedLeft), (Camera::FliesFree, Act::Nothing));
    }

    #[test]
    fn a_following_camera_finds_a_player_when_it_has_none() {
        assert_eq!(step(Camera::Follows, Event::World { followed_in_match: true }), (Camera::Follows, Act::Nothing));
        assert_eq!(step(Camera::Follows, Event::World { followed_in_match: false }),
                   (Camera::Follows, Act::Follow(Step::First)));
        assert_eq!(step(Camera::Follows, Event::FollowedLeft), (Camera::Follows, Act::Follow(Step::Next)));
    }

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
