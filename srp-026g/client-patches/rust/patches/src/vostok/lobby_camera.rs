//! The lobby camera (lobby menu+10Ch; ctor 0x283BC0, tick 0x283EF0, its
//! collision ray 0x2844D0) at `lobby_camera.view`, and patch 45's state, the
//! interface between its NASM (weapon_table_026g.asm: the weapon at the
//! bench, the window's requests) and the Rust camera flight
//! (`weapon_flight.rs`).
//!
//! The camera looks at its point (+A0h, its current height +A4h, +A8h) from
//! the distance +C4h along forward = (-sin yaw cos pitch, sin pitch, cos yaw
//! cos pitch); the side angle +ACh turns only the view; +94h is patch 22's
//! zoom. At the site the frame's yaw and pitch are final; the side angle's
//! and the distance's steps, the ray, the last clamp and the eye follow.

use ragdoll::math::{Vec3, sin_cos};

use super::context::Context;
use super::hook::Thread;
use crate::raw;

/// Where the camera is and how it looks (its current values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    /// The point it looks at (x, the current height, z).
    pub point: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub side: f32,
    pub zoom: f32,
}

/// The camera's forward of a yaw and pitch (the tick's rotY(yaw) rotX(pitch)).
pub fn forward(yaw: f32, pitch: f32) -> Vec3 {
    let (sy, cy) = sin_cos(yaw);
    let (sp, cp) = sin_cos(pitch);
    Vec3::new(-sy * cp, sp, cy * cp)
}

impl CameraPose {
    /// The eye: the point less the distance along forward.
    pub fn eye(&self) -> Vec3 {
        self.point - forward(self.yaw, self.pitch) * self.distance
    }
}

/// The targets the camera's own steps go to (the setter 0x2845C0's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepTargets {
    pub height: f32,
    pub side: f32,
    pub distance: f32,
}

/// The lobby camera at `lobby_camera.view`.
pub struct LobbyCamera<'t> {
    ticker: raw::LobbyCameraTicker,
    _thread: &'t Thread,
}

/// The ctor's distance and its target (0x283C2C: 1.96 m).
const STOCK_DISTANCE: f32 = 1.96;

/// The ctor's speeds of the side angle's and the height's steps, and the
/// wheel's of the distance's (0x28409E): the camera's own again after a flight.
const STOCK_SPEEDS: [f32; 3] = [1.0, 1.0, 3.0];

impl<'t> LobbyCamera<'t> {
    /// The camera and the frame's time (s, at most 0.1).
    pub fn at(ctx: &'t Context<'_>) -> Option<(LobbyCamera<'t>, f32)> {
        // SAFETY: the context is this site's: EBX the camera's +4 and the
        // tick's frame at ESP, alive while the tick runs on the game's thread.
        let (ticker, frame) = unsafe {
            (raw::site::lobby_camera_view::camera(ctx)?, raw::site::lobby_camera_view::frame(ctx)?)
        };
        // SAFETY: as above: the frame's time, written before the site.
        let dt = unsafe { *frame.dt() };
        Some((LobbyCamera { ticker, _thread: ctx.thread() }, dt))
    }

    /// The hook's thread.
    pub fn thread(&self) -> &'t Thread {
        self._thread
    }

    pub fn pose(&self) -> CameraPose {
        let t = self.ticker;
        // SAFETY: the camera's own fields (fields_026g.tsv), the tick's
        // object on its thread.
        unsafe {
            CameraPose {
                point: Vec3::new(*t.point_x(), *t.height(), *t.point_z()),
                yaw: *t.yaw(),
                pitch: *t.pitch(),
                distance: *t.distance(),
                side: *t.side(),
                zoom: *t.zoom(),
            }
        }
    }

    pub fn targets(&self) -> StepTargets {
        let t = self.ticker;
        // SAFETY: as pose.
        unsafe { StepTargets { height: *t.height_target(), side: *t.side_target(), distance: *t.distance_target() } }
    }

    /// Puts the camera there; its own steps hold still (their speeds 0)
    /// while a flight moves it, or move again (`release`).
    pub fn set(&mut self, pose: &CameraPose, steps: Steps) {
        let t = self.ticker;
        let speeds = match steps {
            Steps::Held => [0.0; 3],
            Steps::Released => STOCK_SPEEDS,
        };
        // SAFETY: as pose; the tick reads them after the site.
        unsafe {
            *t.point_x() = pose.point.x;
            *t.height() = pose.point.y;
            *t.point_z() = pose.point.z;
            *t.yaw() = pose.yaw;
            *t.pitch() = pose.pitch;
            *t.distance() = pose.distance;
            *t.side() = pose.side;
            *t.zoom() = pose.zoom;
            *t.side_speed() = speeds[0];
            *t.height_speed() = speeds[1];
            *t.distance_speed() = speeds[2];
        }
    }

    /// The lobby menu the camera is its (the ctor's), when it has one.
    pub(in crate::vostok) fn menu(&self) -> Option<raw::LobbyMenu> {
        // SAFETY: the ticker's own field, the camera's menu (made before it, alive while it is).
        unsafe { self.ticker.menu() }
    }

    /// Puts the camera at the pose its ctor gives (0x283C24..0x283C78: the yaw, the pitch and the
    /// side angle 0, the distance and its target 1.96 m), when patch 45's camera is the stock one
    /// (no weapon at the bench, no flight): a new lobby's first view (patch 81).
    pub fn to_stock_pose(&mut self) {
        if WeaponTableState::of(self._thread).camera() != TableCamera::Stock {
            return;
        }
        let t = self.ticker;
        // SAFETY: as pose; the tick reads them after the site.
        unsafe {
            *t.side() = 0.0;
            *t.side_target() = 0.0;
        }
        self.look(0.0, 0.0, STOCK_DISTANCE);
    }

    /// A diagnostics build's camera sweep (camera_sweep.rs): the yaw and the pitch now, the
    /// distance's target (the stock camera's ray then cuts it as ever) and the distance itself.
    pub fn look(&mut self, yaw: f32, pitch: f32, distance: f32) {
        let t = self.ticker;
        // SAFETY: as pose; the tick reads them after the site.
        unsafe {
            *t.yaw() = yaw;
            *t.pitch() = pitch;
            *t.distance_target() = distance;
            *t.distance() = distance;
        }
    }

    /// A diagnostics build's squad dummies (camera_sweep.rs): both squad panels' characters
    /// made the account's own (its profile and chosen kit), as patch 32's show_kit makes the
    /// lobby's own character. Whether they are there (false: no lobby menu, profile or
    /// character).
    pub fn show_squad_dummies(&self) -> bool {
        // SAFETY: the camera's lobby menu (the ctor's), its account profile and its two squad
        // characters, made by the menu's ctor and alive while the menu is; the game's own
        // setup_profile, on the game's thread, as show_kit calls it.
        unsafe {
            let Some(menu) = self.ticker.menu() else { return false };
            let Some(profile) = menu.account_profile() else { return false };
            let kit = u32::from(*profile.chosen_kit());
            let mut shown = false;
            for character in [menu.squad_right(), menu.squad_left()].into_iter().flatten() {
                raw::engine_lobby_character_setup_profile(Some(profile), Some(character), kit);
                shown = true;
            }
            shown
        }
    }

    /// A diagnostics build's empty lobby (camera_sweep.rs, the lobby window's pictures): the
    /// lobby's own character and both squad panels' characters drop their players and stay
    /// without (0x284AA0 with its flag set: the player taken out of the world and released, and
    /// on_player_ready makes none while +D2C9h is set), so the scene is drawn without anyone in
    /// it, its camera and light as ever. Whether there was a menu.
    pub fn empty_characters(&self) -> bool {
        // SAFETY: the camera's lobby menu (the ctor's) and its three characters, made by the
        // menu's ctor and alive while the menu is; the game's own drop of a character's player,
        // on the game's thread, as on_player_ready drops the old one.
        unsafe {
            let Some(menu) = self.ticker.menu() else { return false };
            for character in [menu.character(), menu.squad_right(), menu.squad_left()].into_iter().flatten() {
                raw::engine_lobby_character_release_player(Some(character), 1);
            }
            true
        }
    }

    /// The distance the camera's collision ray (0x2844D0, the lobby's static
    /// collisions) leaves of `distance` from `point` back along `forward`.
    pub fn ray_cut(&self, point: Vec3, forward: Vec3, distance: f32) -> f32 {
        let p = [point.x, point.y, point.z];
        let back = [-forward.x, -forward.y, -forward.z];
        let mut d = distance;
        // SAFETY: the camera (the ticker's -4), with its collision scene;
        // the ray reads the point and the direction and writes the distance,
        // all ours on this stack, as the tick's own call (0x284309) does.
        unsafe { raw::engine_lobby_camera_ray(Some(self.ticker.camera()), p.as_ptr(), &mut d, back.as_ptr()) };
        d
    }
}

/// The camera's own steps during a flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Steps {
    Held,
    Released,
}

/// What patch 45's NASM asks of the camera (its state, read once a tick).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableRequest {
    /// Its request count: a change is a new request.
    pub count: u32,
    /// The table on (the bench's view) or off (the stock camera).
    pub table_on: bool,
    /// The point's x and z, the yaw, the pitch and the zoom to go to.
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

/// The flight's word in patch 45's state, which its NASM reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FlightWord {
    /// The stock camera.
    Stock = 0,
    /// Moving (there or back): no ray; the wheel's farthest on the target only.
    Moving = 1,
    /// At the bench: no ray; the wheel's farthest on the distance too.
    Resting = 2,
}

/// Whose the lobby camera is, by patch 45's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableCamera {
    /// The stock camera's: the table off and no flight.
    Stock,
    /// The table's: a weapon at the lobby's spot (read from the lobby's
    /// project when the table turned on), or the flight there or back.
    Table,
}

/// Patch 45's state (weapon_table_026g.asm's STATE, `.srpd`).
pub struct WeaponTableState<'t> {
    state: raw::WeaponTable,
    _thread: &'t Thread,
}

impl<'t> WeaponTableState<'t> {
    pub fn of(thread: &'t Thread) -> WeaponTableState<'t> {
        WeaponTableState { state: raw::image::weapon_table(), _thread: thread }
    }

    pub fn request(&self) -> TableRequest {
        let s = self.state;
        // SAFETY: patch 45's state page, written by its NASM on this thread
        // (the movie's callbacks and the camera's tick are the game's thread).
        unsafe {
            TableRequest {
                count: *s.request(),
                table_on: *s.table() != 0,
                x: *s.target_x(),
                z: *s.target_z(),
                yaw: *s.target_yaw(),
                pitch: *s.target_pitch(),
                zoom: *s.target_zoom(),
            }
        }
    }

    /// Whose the camera is: the stock camera's with the table off and no flight.
    pub fn camera(&self) -> TableCamera {
        // SAFETY: as request.
        let stock = unsafe { *self.state.table() == 0 && *self.state.flight() == FlightWord::Stock as u32 };
        if stock { TableCamera::Stock } else { TableCamera::Table }
    }

    pub fn set_flight(&mut self, word: FlightWord) {
        // SAFETY: as request; the word is the flight's alone.
        unsafe { *self.state.flight() = word as u32 }
    }
}
