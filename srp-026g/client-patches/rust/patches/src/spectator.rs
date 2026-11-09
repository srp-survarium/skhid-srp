//! Patch 121: a game sent to a match with no place of its own watches it
//! (the rooms' queue, docs/notes/queue-spectate.md; client_patch.md 3.76).
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

use crate::vostok::replay_view::MatchClose;
use crate::vostok::spectate::{world_camera_client, CameraKey, FollowedAlive, Left, MatchLine, MatchStart, PlayerLeft, SpectateKey};
use crate::vostok::world::{NetworkClient, Step};
use crate::vostok::types::Place;
use crate::vostok::{
    Context, Flow, GameCell, NoExit, SpectatorOnFollowedAliveExit, SpectatorOnKeyExit, SpectatorOnMatchLineExit,
    SpectatorOnPlayerLeftExit, SpectatorOnWorldCameraExit, Thread,
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

/// The replay controls' player list (patch 130): the camera follows the
/// player of `place`, as F6/F5 would reach it.
pub fn view_player(client: &NetworkClient<'_>, thread: &Thread, place: Place) {
    if client.follow_place(place) {
        CAMERA.set(thread, Camera::Follows);
    }
}

/// The replay controls' free view (patch 130): as F2.
pub fn view_free(client: &NetworkClient<'_>, thread: &Thread) {
    camera(client, thread, Event::Key(CameraKey::FlyFree));
}

/// `static_info.options_read` (after patches 65, 86 and 58's): a new match.
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(start) = MatchStart::at(ctx) {
        start.client.begin_match(start.local_place, start.places);
    }
    // A new match's camera does nothing yet (Act::Nothing): its first world
    // finds it a player.
    CAMERA.update(ctx.thread(), |camera| step(camera, Event::NewMatch).0);
    Flow::Continue
}

/// `close_match.reason`, after patch 130's (which reads the role): the match is
/// over, its role with it and a spectator's input off for the next match.
pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(close) = MatchClose::at(ctx) {
        close.client.end_match();
    }
    Flow::Continue
}

/// `world_load.loaded`: after a world. A player's game: the game's own
/// camera back to its place (and its input on). A spectator's: the player
/// it follows kept while that one is in the match, else the first one; a
/// free flight kept; none of the game's own steps (they ask for the place
/// it does not have).
pub fn on_world_camera(ctx: &mut Context<'_>) -> Flow<SpectatorOnWorldCameraExit> {
    let Some(client) = world_camera_client(ctx) else {
        return Flow::Continue;
    };
    if !client.is_spectator() {
        return Flow::Continue;
    }
    let followed_in_match = client.local_player().is_some_and(|player| player.inserted());
    camera(&client, ctx.thread(), Event::World { followed_in_match });
    Flow::Exit(SpectatorOnWorldCameraExit::Done)
}

/// `keyboard.spectate`: a spectator's F6 and F5 move its camera to the next
/// or the previous player, of any team; F2 lets go and flies free. Every
/// other key and every player's game go on as the game's.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<SpectatorOnKeyExit> {
    let Some(key) = SpectateKey::at(ctx) else {
        return Flow::Continue;
    };
    match (key.client.is_spectator(), key.key) {
        (true, Some(camera_key)) => {
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
    if !line.client.is_spectator() {
        return Flow::Continue;
    }
    line.as_plain();
    Flow::Exit(SpectatorOnMatchLineExit::Plain)
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
    if !left.client.is_spectator() {
        return Flow::Continue;
    }
    match left.left {
        Some(Left::Spectator) => {
            left.client.stop_watching();
            Flow::Exit(SpectatorOnPlayerLeftExit::Done)
        }
        Some(Left::Place(place)) if left.client.local_player().is_some_and(|player| player.place() == place.get()) => {
            camera(&left.client, ctx.thread(), Event::FollowedLeft);
            Flow::Exit(SpectatorOnPlayerLeftExit::Done)
        }
        Some(Left::Place(_)) | None => Flow::Continue,
    }
}

/// `hud.followed_alive`: a spectator's hud has no quick slots, stamina or kits of the player it
/// follows to show or hide at a death or a spawn (a kit picked there would be the followed
/// player's, in its own world alone): the game's change of those panels is skipped, once for
/// all, here. The slots hide themselves at a match's start (0x23B565) and nothing shows them.
pub fn on_followed_alive(ctx: &mut Context<'_>) -> Flow<SpectatorOnFollowedAliveExit> {
    let site = FollowedAlive::at(ctx);
    if !site.client().is_some_and(|client| client.is_spectator()) {
        return Flow::Continue;
    }
    Flow::Exit(SpectatorOnFollowedAliveExit::Kept)
}

#[cfg(test)]
mod tests {
    use super::{step, Act, Camera, Event};
    use crate::vostok::spectate::CameraKey;
    use crate::vostok::world::{spectate_order, Step};
    use crate::vostok::types::Place;
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

    fn order(places: u32, current: Option<u32>, step: Step) -> Vec<u32> {
        spectate_order(places, current.and_then(Place::new), step).map(|place| u32::from(place.get())).collect()
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
