//! A live match's screen and the engine's free-fly camera, for a local
//! build's spawn marks (`spawn_marks.rs`, docs/notes/client_patch.md
//! section 3.82). This module is compiled only with `cfg(srp_spawn_marks)`,
//! which `patch_026g.py --spawn-marks` alone gives (and for the crate's
//! tests): a free camera in a live match is a cheat, so the exe players get
//! has none of this code.
//!
//! The game world's camera director (+98h) shows one game_camera at a time
//! (+84h) and copies its view to its own +4 every frame
//! (camera_director::apply 0x243440). Its switch (switch_to_camera 0x243510)
//! takes the focus from the old camera and gives it to the new one, and each
//! camera adds or removes its own input handler in the input world with it
//! (player_input_handler 0x242500, free_fly_camera 0x2498B0): while the
//! free-fly camera has the screen, the player's handler gets no key, button
//! or mouse move.
//!
//! The free-fly camera (the game world's +4D50h) is the one a replay's free
//! view uses, and a live match's before a player is bound. Its tick
//! (0x249A30) turns it with the mouse and moves it 6 m a second: the left
//! mouse button forward, the right one back, A and D sideways, W and S up
//! and down, Ctrl held 20 times as fast, Shift a tenth (the keys by their
//! codes, each counted only while it is bound to some action).
//!
//! Going to it here is the two things game_world::switch_to_free_fly_camera
//! (0x1F8270) does for a bound player that the switch back undoes: the
//! handler's mode 1 (so the input builder builds nothing, 0x2411C1, and the
//! player's body is drawn whole, 0x24782A) and the director's switch. Its
//! other two steps are left out, because switch_to_player_camera (0x1F8200)
//! does not undo them: the first-person effects presenter losing its player
//! (0x1F82D4) and the project's effect played on the free-fly camera
//! (0x1F8330). Coming back is the game's own switch_to_player_camera, first
//! person, as its F1 in a replay.

use core::marker::PhantomData;

use super::hook::Thread;
use super::keyboard::GameWorldKey;
use super::world::{GameWorld, Playback, floats};
use crate::raw;

/// The input handler's mode while the free-fly camera has the screen
/// (switch_to_free_fly_camera 0x1F82A3; free_fly_mode).
const FREE_FLY_MODE: u32 = 1;

/// Whose camera a live match's screen has, with what can be done from there.
pub enum Screen<'t> {
    /// The bound local player's own (its input handler's, in whatever mode:
    /// first person, the warmup's, a spectator's).
    Player(PlayerCamera<'t>),
    /// The free-fly camera, over a bound player.
    FreeFly(FreeFlight<'t>),
    /// No player is bound (the free-fly camera is then the game's own, with
    /// nobody to come back to), or no camera is on.
    Unbound,
}

impl<'t> Screen<'t> {
    /// The screen of the game world this key came to, while its game plays a
    /// live match; none in a replay, whose views are the game's own keys.
    pub fn of_live(key: &'t GameWorldKey<'_, '_>) -> Option<Screen<'t>> {
        let world = world_of(key)?;
        if GameWorld::of(world, key.thread()).playback() != Playback::Match {
            return None;
        }
        // SAFETY: the live game world of the hook's site, on the game's
        // thread; its director, free-fly camera and bound handler live as
        // long as it does (each field 0 when there is none).
        let (director, free_fly, handler) =
            unsafe { (world.camera_director(), world.free_fly_camera(), world.player_handler()) };
        let (Some(director), Some(free_fly), Some(handler)) = (director, free_fly, handler) else {
            return Some(Screen::Unbound);
        };
        // SAFETY: the same objects; a camera's game_camera part is inside it.
        let (active, players, free) = unsafe { (director.active(), handler.camera(), free_fly.camera()) };
        Some(match active {
            Some(active) if active == players => {
                Screen::Player(PlayerCamera { director, free, handler, thread: PhantomData })
            }
            Some(active) if active == free => Screen::FreeFly(FreeFlight { world, thread: PhantomData }),
            _ => Screen::Unbound,
        })
    }
}

/// The player's own camera on the screen.
pub struct PlayerCamera<'t> {
    director: raw::CameraDirector,
    free: raw::GameCamera,
    handler: raw::PlayerInputHandler,
    thread: PhantomData<&'t Thread>,
}

impl PlayerCamera<'_> {
    /// The screen goes to the free-fly camera, which starts at this view;
    /// the player stays in the match where it stands, with no input.
    pub fn fly(self) {
        // SAFETY: the live handler, director and free-fly camera, on the
        // game's thread, inside game_world::on_keyboard_action: where the
        // game's own view keys switch cameras (0x25CE23..0x25CE94). The mode
        // is the one switch_to_free_fly_camera writes for a bound handler,
        // read as "neither 0, 2, 3 nor 4" by every reader of it.
        unsafe {
            *self.handler.mode() = FREE_FLY_MODE;
            raw::engine_switch_to_camera(Some(self.director), Some(self.free));
        }
    }
}

/// The free-fly camera on the screen, a player bound under it.
pub struct FreeFlight<'t> {
    world: raw::GameWorld,
    thread: PhantomData<&'t Thread>,
}

impl FreeFlight<'_> {
    /// The screen goes back to the player's camera, first person
    /// (switch_to_player_camera 0x1F8200: the handler's mode 0, the
    /// director's switch).
    pub fn land(self) {
        // SAFETY: the live game world on the game's thread, as `fly`; the
        // function does nothing for a world without a bound handler.
        unsafe { raw::engine_switch_to_player_camera(Some(self.world), 1) }
    }
}

/// Which camera a view was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewCamera {
    /// The engine's free-fly camera.
    Free,
    /// Any other: the player's own in a live match, a followed player's in a
    /// replay.
    Player,
}

/// What the screen shows: the active camera's place and forward (the
/// director's view, a row-vector 4x4: right, up, forward, place).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenView {
    pub place: [f32; 3],
    pub forward: [f32; 3],
    pub camera: ViewCamera,
}

/// The view on the screen of the game world this key came to (a live match
/// or a replay); none while no camera is on.
pub fn view(key: &GameWorldKey<'_, '_>) -> Option<ScreenView> {
    let world = world_of(key)?;
    // SAFETY: the live game world of the hook's site, on the game's thread;
    // its director and free-fly camera live as long as it does, the
    // director's view 16 floats.
    unsafe {
        let director = world.camera_director()?;
        let active = director.active()?;
        let free = world.free_fly_camera().map(|camera| camera.camera());
        let view = settled(|| floats::<16>(director.view()));
        Some(ScreenView {
            place: [view[12], view[13], view[14]],
            forward: [view[8], view[9], view[10]],
            camera: if Some(active) == free { ViewCamera::Free } else { ViewCamera::Player },
        })
    }
}

/// The local player's place (its simulated transform's translation: the
/// feet), when the game world this key came to has one.
pub fn player_place(key: &GameWorldKey<'_, '_>) -> Option<[f32; 3]> {
    let world = GameWorld::of(world_of(key)?, key.thread());
    Some(world.network_client()?.local_player()?.feet())
}

fn world_of(key: &GameWorldKey<'_, '_>) -> Option<raw::GameWorld> {
    // SAFETY: the hud of the hook's site and its world, alive while
    // on_keyboard_action runs on the game's thread.
    unsafe { key.hud().raw().world() }
}

/// A value read until two readings in a row agree (at most four): the
/// director's view is copied from the camera's by the draw, which may run on
/// a task of the game's thread, so one reading alone could hold half of each
/// of two frames.
fn settled<T: PartialEq>(mut read: impl FnMut() -> T) -> T {
    let mut last = read();
    for _ in 0..3 {
        let next = read();
        if next == last {
            break;
        }
        last = next;
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_is_taken_when_two_agree() {
        let mut readings = [1, 2, 2, 9].into_iter();
        assert_eq!(settled(|| readings.next().unwrap()), 2);
        assert_eq!(readings.next(), Some(9), "no reading past the two that agree");
        let mut steady = [5, 5, 7].into_iter();
        assert_eq!(settled(|| steady.next().unwrap()), 5);
        let mut never = 0;
        assert_eq!(settled(|| { never += 1; never }), 4, "the fourth reading at the latest");
    }
}
