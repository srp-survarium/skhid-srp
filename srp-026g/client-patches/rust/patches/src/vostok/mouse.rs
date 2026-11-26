//! The mouse look's steps (patch 112, docs/notes/mouse-pitch.md).
//!
//! `player_input_handler::on_mouse_move` (0x240200) turns the yaw by
//! x * pi/180 * s radians, s the yaw's step (the handler's fov factor +94h x
//! the option sensitivity x 0.1), and the look pitch by y * pi/180 * the
//! pitch's step * invert * 3/pi units ([`View`] says what a unit turns). The
//! stock pitch's step is the render window's h / w x s (0x2402E9..0x2402FA,
//! `mouse.pitch_step`); patch 46's death camera turns by its own steps, in
//! radians (`mouse.death_pitch`).

use super::context::{Context, Reg};
use crate::raw;

/// What a unit of the turn's pitch turns: the first-person view by the look
/// clip (60 degrees a unit while the look pitch is up, 67.5 while it is down:
/// bone Item of on_site_camera_look, scrubbed at (pitch + 1) / 2, 0x2FBF90),
/// or the warmup camera's orbit by a radian (mode 3, process_warmup_mode
/// 0x242200 adds the turn to its yaw and pitch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Up,
    Down,
    Orbit,
}

/// The handler's mode of the warmup camera.
const WARMUP: u32 = 3;

impl View {
    /// The handler's mode and, in the first person, the local player's look
    /// pitch (none: up, the clip's middle).
    fn of(mode: u32, look_pitch: Option<f32>) -> View {
        match (mode, look_pitch) {
            (WARMUP, _) => View::Orbit,
            (_, Some(pitch)) if pitch < 0.0 => View::Down,
            _ => View::Up,
        }
    }
}

/// on_mouse_move at its pitch step (`mouse.pitch_step`): the yaw's step the
/// game made, the handler's fov factor and what the pitch turns, read once;
/// `set` writes the pitch's step where the replaced lines leave it.
pub struct PitchStep<'a, 'c> {
    ctx: &'a mut Context<'c>,
    horizontal: f32,
    fov_factor: f32,
    view: View,
}

impl<'a, 'c> PitchStep<'a, 'c> {
    pub fn at(ctx: &'a mut Context<'c>) -> Option<PitchStep<'a, 'c>> {
        use raw::site::mouse_pitch_step as site;
        // SAFETY: the site's ESP is on_mouse_move's frame on the game's
        // thread; its [esp+18h] the handler (the function's ECX, alive for
        // the call), its [esp+28h] the float the game stored at 0x240289;
        // the handler's game world, its game, network client and local
        // player are the game's for the call (patch 46 reads the same chain).
        let (horizontal, fov_factor, view) = unsafe {
            let frame = site::frame(ctx)?;
            let handler = frame.handler()?;
            let look_pitch = handler.game_world().and_then(|world| world.game()).and_then(|game| game.network_client())
                .and_then(|client| client.local_player()).map(|player| *player.look_pitch());
            (*frame.horizontal(), *handler.fov_factor(), View::of(*handler.mode(), look_pitch))
        };
        Some(PitchStep { ctx, horizontal, fov_factor, view })
    }

    pub fn view(&self) -> View {
        self.view
    }

    /// The yaw's step s: fov factor x option sensitivity x 0.1.
    pub fn horizontal(&self) -> f32 {
        self.horizontal
    }

    /// The handler's fov factor (both axes; 1 unaimed).
    pub fn fov_factor(&self) -> f32 {
        self.fov_factor
    }

    /// The pitch's step before the invert and 3/pi (XMM1), and the yaw's step
    /// in XMM2, as the replaced lines leave them; the caller skips them.
    pub fn set(self, pitch: f32) {
        self.ctx.set_xmm_f32(1, pitch);
        self.ctx.set_xmm_f32(2, self.horizontal);
    }
}

/// Patch 46's drag asking the pitch's sensitivity (`mouse.death_pitch`):
/// XMM1 holds the option sensitivity it loaded; `locked` leaves the drag its
/// stock step (EAX 0), `split` gives it a sensitivity at 1:1 (EAX 1, XMM1).
pub struct DeathPitch<'a, 'c> {
    ctx: &'a mut Context<'c>,
}

impl<'a, 'c> DeathPitch<'a, 'c> {
    pub fn at(ctx: &'a mut Context<'c>) -> DeathPitch<'a, 'c> {
        DeathPitch { ctx }
    }

    pub fn locked(self) {
        self.ctx.set_reg(Reg::Eax, 0);
    }

    pub fn split(self, sensitivity: f32) {
        self.ctx.set_reg(Reg::Eax, 1);
        self.ctx.set_xmm_f32(1, sensitivity);
    }
}

/// process_first_person_mode's end (`input_builder.end`, 0x2421B0; every way
/// through it, also a frame with no key and no mouse event, 0x241284): the
/// turn of the frame's input, two half floats (the yaw in radians, growing to
/// the left; the pitch in the look pitch's units). on_before_processing
/// (0x241107) empties it each frame, on_mouse_move (0x240374, 0x2403D1) adds
/// each move of the mouse and the builder its turn keys (0x2413EA..0x24151F);
/// nothing of the builder reads it after its check at 0x241526.
pub struct FrameTurn<'c> {
    handler: raw::PlayerInputHandler,
    site: core::marker::PhantomData<&'c ()>,
}

impl<'c> FrameTurn<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<FrameTurn<'c>> {
        // SAFETY: the context is this site's: EBX process_first_person_mode's
        // handler, alive for the call on the game's thread.
        let handler = unsafe { raw::site::input_builder_end::handler(ctx)? };
        Some(FrameTurn { handler, site: core::marker::PhantomData })
    }

    /// The turn: the yaw's half and the pitch's.
    pub fn halves(&self) -> (u16, u16) {
        // SAFETY: the live handler's own dword (+338h: the two halves).
        let turn = unsafe { self.handler.turn().read_volatile() };
        (turn as u16, (turn >> 16) as u16)
    }

    /// The turn the game goes on to read.
    pub fn set(&mut self, (yaw, pitch): (u16, u16)) {
        // SAFETY: as `halves`; the game's thread takes the input after the builder.
        unsafe { self.handler.turn().write_volatile(u32::from(yaw) | u32::from(pitch) << 16) };
    }

    /// The living local player whose turn it is; none without one, and while
    /// the scope's own view is on.
    pub fn holder(&self) -> Option<Holder> {
        // SAFETY: the handler's game world, its game, network client and
        // local player are the game's for the call (as `PitchStep::at`); the
        // player's own byte, its held item (a weapon's first dword its
        // vtable) and that weapon's own float and byte.
        unsafe {
            let player = self.handler.game_world()?.game()?.network_client()?.local_player()?;
            if player.alive().read_volatile() == 0 {
                return None;
            }
            let look_pitch = player.look_pitch().read_volatile();
            let hip = Some(Holder { raised: 0.0, look_pitch });
            let Some(item) = player.held_item() else { return hip };
            if item.vtable().read_volatile() != raw::image::WEAPON_VTABLE {
                return hip;
            }
            let weapon = item.weapon();
            (weapon.scope_view().read_volatile() == 0).then(|| Holder { raised: weapon.aim_progress().read_volatile(), look_pitch })
        }
    }
}

/// The player a frame's turn turns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holder {
    /// How far the sight of its gun is raised, 0 from the hip (and with no
    /// client weapon in hand) to 1.
    pub raised: f32,
    /// Its look pitch (units; one up, one down).
    pub look_pitch: f32,
}
