//! Patch 143: a game sent to a match with no place of its own watches it
//! (the rooms' queue, docs/notes/queue-spectate.md; client_patch.md 3.82).
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
use crate::vostok::spectate::{world_camera_client, CameraKey, FollowedAlive, Left, MatchLine, MatchStart, PlayerLeft, PlayerLeftEvent, SpectateKey};
use crate::vostok::world::{Followed, NetworkClient, Step};
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
    /// A world was loaded: the player followed against its place's.
    World(Followed),
    /// A frame of the hud, between worlds: the same (a respawn in another kit).
    Frame(Followed),
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
    /// The current player of the place it follows (its new kit's).
    FollowPlace(Place),
    FlyFree,
}

/// The camera's one transition, total over the events: F2 lets go and
/// flies until F6/F5 (from the last one followed) or the next match; a
/// flying camera is kept over a world's load, a frame and a player's leave.
/// A following camera stays with its place when that place plays another
/// kit (a world or a frame); a world in which neither the player followed
/// nor its place's is in the match gives it the first player.
fn step(camera: Camera, event: Event) -> (Camera, Act) {
    match (camera, event) {
        (_, Event::NewMatch) => (Camera::Follows, Act::Nothing),
        (_, Event::Key(CameraKey::Follow(step))) => (Camera::Follows, Act::Follow(step)),
        (_, Event::Key(CameraKey::FlyFree)) => (Camera::FliesFree, Act::FlyFree),
        (Camera::FliesFree, Event::World(_) | Event::Frame(_) | Event::FollowedLeft) => (Camera::FliesFree, Act::Nothing),
        (Camera::Follows, Event::World(Followed::Replaced(place)) | Event::Frame(Followed::Replaced(place))) => {
            (Camera::Follows, Act::FollowPlace(place))
        }
        (Camera::Follows, Event::World(Followed::InMatch) | Event::Frame(Followed::InMatch | Followed::Gone)) => {
            (Camera::Follows, Act::Nothing)
        }
        (Camera::Follows, Event::World(Followed::Gone)) => (Camera::Follows, Act::Follow(Step::First)),
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
        Act::FollowPlace(place) => {
            client.follow_place(place);
        }
        Act::FlyFree => client.fly_free(),
    }
}

/// Each frame of the hud (patch 144's `hud.update`, after the tick's logic, before the frame
/// is drawn): a spectator's camera goes with the place it follows into another kit, which the
/// game does for the local place alone (0x2678E0); a compare when nothing changed.
pub fn keep_followed(client: &NetworkClient<'_>, thread: &Thread) {
    if client.is_spectator() {
        camera(client, thread, Event::Frame(client.followed()));
    }
}

/// The replay controls' player list (patch 144): the camera follows the
/// player of `place`, as F6/F5 would reach it.
pub fn view_player(client: &NetworkClient<'_>, thread: &Thread, place: Place) {
    if client.follow_place(place) {
        CAMERA.set(thread, Camera::Follows);
    }
}

/// The replay controls' free view (patch 144): as F2.
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

/// `close_match.reason`, after patch 144's (which reads the role): the match is
/// over, its role with it and a spectator's input off for the next match.
pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(close) = MatchClose::at(ctx) {
        close.client.end_match();
    }
    Flow::Continue
}

/// `world_load.loaded`: after a world. A player's game: the game's own
/// camera back to its place (and its input on). A spectator's: the player
/// it follows kept while that one is in the match, its place's current
/// player when that place plays another kit in this world, else the first
/// one; a free flight kept; none of the game's own steps (they ask for the
/// place it does not have).
pub fn on_world_camera(ctx: &mut Context<'_>) -> Flow<SpectatorOnWorldCameraExit> {
    let Some(client) = world_camera_client(ctx) else {
        return Flow::Continue;
    };
    if !client.is_spectator() {
        return Flow::Continue;
    }
    camera(&client, ctx.thread(), Event::World(client.followed()));
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

/// `player_left.entry`: process_player_left_match (0x27C690) whole, in Rust. For every game the
/// 0xEA read and its leave and stats, as the game does them; then a player's game takes it the
/// game's way ([`PlayerLeft::finish_player_left`]: its own end with the results, or the followed
/// player's leave). A spectator's game takes one naming place 0xFF as its own end (the server's
/// farewell) and goes back to the lobby with no results screen; one naming the player it follows
/// moves its camera to the next; any other, nothing.
pub fn on_player_left(ctx: &mut Context<'_>) -> Flow<SpectatorOnPlayerLeftExit> {
    let Some(event) = PlayerLeftEvent::at(ctx) else {
        return Flow::Continue;
    };
    if let PlayerLeftEvent::Left(left) = event {
        finish_left(ctx.thread(), left);
    }
    Flow::Exit(SpectatorOnPlayerLeftExit::Done)
}

/// The game's or the spectator's reaction to an 0xEA.
fn finish_left(thread: &Thread, left: PlayerLeft<'_>) {
    if !left.client.is_spectator() {
        left.client.finish_player_left(left.place);
        return;
    }
    match left.left {
        Some(Left::Spectator) => left.client.stop_watching(),
        Some(Left::Place(place)) if left.client.local_player().is_some_and(|player| player.place() == place.get()) => {
            camera(&left.client, thread, Event::FollowedLeft);
        }
        Some(Left::Place(_)) | None => {}
    }
}

/// `hud.followed_alive`: a spectator's hud has no quick slots, stamina or kits of the player it
/// follows to show or hide at a death or a spawn (a kit picked there would be the followed
/// player's, in its own world alone): the game's change of those panels is skipped, once for
/// all, here, and so is patch 31's ui_start after it, which before play would show the kits and
/// the hud's cursor for a living player followed: that cursor took every mouse move, and once F2
/// let the player go (the function returns early with nobody followed, 0x23B75E) nothing hid it
/// again, so the free-fly camera could not turn. The slots hide themselves at a match's start
/// (0x23B565) and nothing shows them.
pub fn on_followed_alive(ctx: &mut Context<'_>) -> Flow<SpectatorOnFollowedAliveExit> {
    let mut site = FollowedAlive::at(ctx);
    if !site.client().is_some_and(|client| client.is_spectator()) {
        return Flow::Continue;
    }
    match site.keep_panels() {
        Some(()) => Flow::Exit(SpectatorOnFollowedAliveExit::Kept),
        None => Flow::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::{step, Act, Camera, Event};
    use crate::vostok::spectate::CameraKey;
    use crate::vostok::world::{spectate_order, Followed, Step};
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

    fn every_followed() -> [Followed; 3] {
        [Followed::InMatch, Followed::Replaced(Place::new(2).unwrap()), Followed::Gone]
    }

    #[test]
    fn a_flight_is_kept_over_a_world_a_frame_and_a_leave() {
        for followed in every_followed() {
            assert_eq!(step(Camera::FliesFree, Event::World(followed)), (Camera::FliesFree, Act::Nothing));
            assert_eq!(step(Camera::FliesFree, Event::Frame(followed)), (Camera::FliesFree, Act::Nothing));
        }
        assert_eq!(step(Camera::FliesFree, Event::FollowedLeft), (Camera::FliesFree, Act::Nothing));
    }

    #[test]
    fn a_following_camera_finds_a_player_when_it_has_none() {
        assert_eq!(step(Camera::Follows, Event::World(Followed::InMatch)), (Camera::Follows, Act::Nothing));
        assert_eq!(step(Camera::Follows, Event::World(Followed::Gone)), (Camera::Follows, Act::Follow(Step::First)));
        assert_eq!(step(Camera::Follows, Event::FollowedLeft), (Camera::Follows, Act::Follow(Step::Next)));
    }

    #[test]
    fn a_following_camera_stays_with_its_place_in_another_kit() {
        let place = Place::new(2).unwrap();
        for event in [Event::World(Followed::Replaced(place)), Event::Frame(Followed::Replaced(place))] {
            assert_eq!(step(Camera::Follows, event), (Camera::Follows, Act::FollowPlace(place)));
        }
        // A frame never looks for another player: only a world, a key or a leave does.
        for followed in [Followed::InMatch, Followed::Gone] {
            assert_eq!(step(Camera::Follows, Event::Frame(followed)), (Camera::Follows, Act::Nothing));
        }
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
