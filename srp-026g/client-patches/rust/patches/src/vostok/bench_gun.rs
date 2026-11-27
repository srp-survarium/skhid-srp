//! The gun the lobby shows at the bench, as `weapon::draw` draws it
//! (`attachments.draw`), for patch 161's modification markers
//! (`docs/notes/modification-069.md`): where a point of the gun is on the
//! screen, and the lobby movie told so.
//!
//! The lobby's own character (lobby menu+694h) holds the weapon its upgrade
//! window shows (patch 32), and while that window is open patch 45 draws it
//! above the bench (its state's TABLE). Its player's held item is the weapon
//! `weapon::draw` draws then; every other drawn weapon (a squad panel's, a
//! match player's) is another one. A point is placed as the game places a
//! locator record (calculate_locator with the draw's own world matrix and
//! bones) and projected as the lobby projects its character for the movie's
//! drop region (`base_game_scene::point_to_screen` on the lobby menu,
//! update_player_region 0x21DF00).

use core::ffi::{CStr, c_void};

use weapon_modules::{BoneIndex, DictId, Placement};

use super::attachments::{model_bones, shown};
use super::context::Context;
use super::hook::Thread;
use super::lobby_camera::WeaponTableState;
use super::ui::movie_of;
use super::world::{read, write_floats};
use crate::raw;

/// calculate_locator's last matrix, applied first: weapon::draw's own.
const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// The lobby movie's method the markers' points go to.
const POINTS: &CStr = c"root.srp_slot_points";

/// What a weapon `weapon::draw` draws is to the lobby's markers.
pub enum Drawn<'c> {
    /// Not the lobby's own character's weapon: a squad panel's, a match's.
    Other,
    /// The lobby's own character's weapon, in its hands (no weapon window
    /// open, or patch 45's table off).
    InHands,
    /// The lobby's own character's weapon above the bench, in the scene.
    AtBench(BenchGun<'c>),
}

/// The lobby's own character's weapon above the bench, at its draw.
pub struct BenchGun<'c> {
    weapon: raw::Weapon,
    frame: raw::WeaponDrawFrame,
    menu: raw::LobbyMenu,
    dict: u16,
    thread: &'c Thread,
}

impl<'c> Drawn<'c> {
    /// The weapon of `attachments.draw`.
    pub fn at(ctx: &'c Context<'_>) -> Drawn<'c> {
        // SAFETY: weapon::draw's EBX and EBP: the weapon it draws and its
        // frame, alive for the draw on the game's thread; the weapon's world
        // (+180Ch), the game (+9Ch of either a world or the lobby menu, the
        // base scene's) and the lobby menu (game+5390h) live as long, as do
        // the menu's own character, its player and that one's held item (its
        // entry the items dictionary's).
        unsafe {
            let (Some(weapon), Some(frame)) = (raw::site::attachments_draw::weapon(ctx), raw::site::attachments_draw::frame(ctx)) else {
                return Drawn::Other;
            };
            let Some(menu) = weapon.world().and_then(|world| world.game()).and_then(|game| game.lobby_menu()) else {
                return Drawn::Other;
            };
            let Some(held) = menu.character().and_then(|character| character.player()).and_then(|player| player.held_item()) else {
                return Drawn::Other;
            };
            if held.address() != weapon.address() {
                return Drawn::Other;
            }
            if !WeaponTableState::of(ctx.thread()).request().table_on || shown(weapon).is_none() {
                return Drawn::InHands;
            }
            let Some(entry) = held.entry() else { return Drawn::InHands };
            Drawn::AtBench(BenchGun { weapon, frame, menu, dict: read(entry.dict_id()), thread: ctx.thread() })
        }
    }
}

impl<'c> BenchGun<'c> {
    pub fn thread(&self) -> &'c Thread {
        self.thread
    }

    /// The weapon item's dict id, its gun's in the modules table.
    pub fn dict(&self) -> Option<DictId> {
        DictId::new(self.dict)
    }

    /// The bones of its model: a placement's bone must be below it.
    pub fn bones(&self) -> Option<usize> {
        // SAFETY: the draw's live weapon.
        unsafe { model_bones(self.weapon) }
    }

    /// Where `at` (a placement on the gun) is on the screen as the gun is
    /// drawn now, in the render window's pixels from its top left; None off
    /// the screen, behind the camera, or for a bone the model lacks.
    pub fn on_screen(&self, at: &Placement) -> Option<[f32; 2]> {
        let BoneIndex(bone) = at.bone;
        if usize::from(bone) >= self.bones()? {
            return None;
        }
        let mut record = raw::WeaponLocatorStorage::zeroed();
        let handle = record.handle();
        let mut frame = [0.0f32; 16];
        let mut pixel = [0.0f32; 2];
        // SAFETY: the record is ours, its bone one of the model's (above);
        // calculate_locator reads it with the draw's own world matrix and
        // bones, as the draw itself does (0x244E51); point_to_screen reads
        // the lobby menu's camera and viewport (its base scene's), as
        // update_player_region does, our point and writes our pixel.
        let seen = unsafe {
            write_floats(handle.offset(), at.offset.matrix());
            handle.bone().write(bone);
            raw::engine_weapon_locator(frame.as_mut_ptr(), Some(handle), self.frame.world(), self.frame.bones(), IDENTITY.as_ptr());
            let point = [frame[12], frame[13], frame[14]];
            raw::engine_point_to_screen(self.menu.address() as usize as *mut c_void, point.as_ptr(), pixel.as_mut_ptr())
        };
        (seen & 0xFF != 0 && pixel.iter().all(|value| value.is_finite())).then_some(pixel)
    }

    /// The lobby movie's `root.srp_slot_points(numbers...)`; whether it took
    /// them.
    pub fn tell(&self, numbers: &[f64]) -> bool {
        // SAFETY: the lobby menu, alive on the game's thread; its movie
        // holder and the holder's movie live as long.
        unsafe { movie_of(self.menu.movie()) }.is_some_and(|movie| movie.invoke_number_list(POINTS, numbers))
    }
}
