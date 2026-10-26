//! The game world's mouse moves and the cameras patches 31 and 46 turn (the
//! warmup camera before the start, the death screen's view):
//! - `world_mouse.move` (sites_026g.tsv): the game world's on_mouse_move at
//!   its start, ECX the game world's input handler (the game world's
//!   +1F8h), [esp+4] the input world, whose mouse has the frame's moves and
//!   the buttons held; the handler's hud pointer, its game (the local
//!   player, the render window) and the player's input handler (its mode,
//!   its warmup camera);
//! - `warmup_camera.pitch_store`: process_warmup_mode's store of the pitch,
//!   ESI the player's input handler, XMM0 the new pitch;
//! - `camera.view_update`: update_camera's call of the view's update, EBX
//!   the player, [esp] its input handler, [esp+4] the eyes' transform.
//!
//! What those registers and fields hold is said here, once.

use core::ffi::c_void;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, pointer, read, write};
use crate::raw;

/// The frame's mouse as the input world has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mouse {
    /// The moves of the frame.
    pub x: i32,
    pub y: i32,
    pub wheel: i32,
    /// The buttons held now (bit 0 the left).
    pub buttons: u8,
}

/// The game world's on_mouse_move, at its start.
pub struct WorldMouse<'t> {
    handler: Object,
    mouse: Option<Object>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> WorldMouse<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<WorldMouse<'t>> {
        let handler = Object::at(ctx.reg(Reg::Ecx))?;
        let frame = ctx.site_esp() as usize as *mut c_void;
        // SAFETY: the function's first argument (mouse_move_frame, by the
        // site's ESP), the input world, and its mouse or 0.
        let mouse = unsafe {
            pointer(raw::mouse_move_frame::input_world(frame))
                .and_then(|world| pointer(raw::world::mouse(world.ptr())))
        };
        Some(WorldMouse { handler, mouse, thread: PhantomData })
    }

    /// The hud shows the kits (the cursor with them).
    pub fn kits_shown(&self) -> bool {
        // SAFETY: ECX at the site, the game world's input handler.
        unsafe { read(raw::world_input_handler::kits(self.handler.ptr())) != 0 }
    }

    /// The player's input handler, if there is one.
    pub fn player_input(&self) -> Option<PlayerInput<'t>> {
        // SAFETY: the handler's player's input handler, or 0.
        unsafe { pointer(raw::world_input_handler::player_input(self.handler.ptr())) }
            .map(|this| PlayerInput { this, thread: PhantomData })
    }

    /// Whether the game's local player is alive (None: no game, no network
    /// client or no local player).
    pub fn local_alive(&self) -> Option<bool> {
        // SAFETY: the handler's game, its network client, the client's local
        // player and its byte m_is_alive; each a pointer or 0.
        unsafe {
            let game = pointer(raw::world_input_handler::game(self.handler.ptr()))?;
            let client = pointer(raw::game::network_client(game.ptr()))?;
            let player = pointer(raw::network_client::local_player(client.ptr()))?;
            Some(read(raw::player::alive(player.ptr())) != 0)
        }
    }

    /// The frame's mouse.
    pub fn mouse(&self) -> Mouse {
        let Some(mouse) = self.mouse else { return Mouse { x: 0, y: 0, wheel: 0, buttons: 0 } };
        // SAFETY: the input world's mouse, its frame's fields.
        unsafe {
            Mouse {
                x: read(raw::mouse::x(mouse.ptr())),
                y: read(raw::mouse::y(mouse.ptr())),
                wheel: read(raw::mouse::wheel(mouse.ptr())),
                buttons: read(raw::mouse::buttons(mouse.ptr())),
            }
        }
    }

    /// The frame's x and y made 0 (the wheel alone goes on to the next
    /// handler).
    pub fn clear_moves(&self) {
        let Some(mouse) = self.mouse else { return };
        // SAFETY: as mouse; the handlers after this one read them.
        unsafe {
            write(raw::mouse::x(mouse.ptr()), 0);
            write(raw::mouse::y(mouse.ptr()), 0);
        }
    }

    /// The hud's pointer (x, y), as the game world keeps it.
    pub fn pointer(&self) -> (u32, u32) {
        // SAFETY: the game world's fields, by the handler's place in it.
        unsafe {
            (read(raw::world_input_handler::pointer_x(self.handler.ptr())),
             read(raw::world_input_handler::pointer_y(self.handler.ptr())))
        }
    }

    fn world(&self) -> *mut c_void {
        // SAFETY: the game world, of which the handler is the +1F8h.
        unsafe { raw::world_input_handler::world(self.handler.ptr()).cast() }
    }

    /// The hud's pointer moved there (the game world's set_pointer, its
    /// vtable+30h).
    pub fn set_pointer(&self, x: i32, y: i32, wheel: i32) {
        // SAFETY: the game world's own slot, thiscall (x, y, the wheel).
        unsafe { raw::engine_world_set_pointer(self.world(), x as u32, y as u32, wheel as u32) }
    }

    /// The system's cursor and the hud's pointer put there (the game
    /// world's set_cursor_position, its vtable+3Ch).
    pub fn set_cursor(&self, x: u32, y: u32) {
        // SAFETY: as set_pointer (x, y).
        unsafe { raw::engine_world_set_cursor(self.world(), x, y) }
    }

    /// The render window's client size (width, height), as the player's look
    /// asks it (the game's +5348h, vtable+1Ch).
    pub fn window_size(&self) -> (f32, f32) {
        let mut size = [0f32; 2];
        // SAFETY: the handler's game and its window interface (the game
        // made it); the slot writes the two floats.
        unsafe {
            if let Some(window) = pointer(raw::world_input_handler::game(self.handler.ptr()))
                .and_then(|game| pointer(raw::game::window(game.ptr())))
            {
                raw::engine_window_size(window.ptr(), size.as_mut_ptr());
            }
        }
        (size[0], size[1])
    }
}

/// The player's input handler.
#[derive(Clone, Copy)]
pub struct PlayerInput<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

/// The warmup camera's yaw, pitch and distance, as f32 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Warmup {
    pub yaw: u32,
    pub pitch: u32,
    pub distance: u32,
}

impl<'t> PlayerInput<'t> {
    /// The handler at `warmup_camera.pitch_store` (ESI).
    pub fn at_pitch_store(ctx: &'t Context<'_>) -> Option<PlayerInput<'t>> {
        Object::at(ctx.reg(Reg::Esi)).map(|this| PlayerInput { this, thread: PhantomData })
    }

    /// Its mode (0 the first person, 3 the warmup).
    pub fn mode(&self) -> u32 {
        // SAFETY: a player's input handler's field.
        unsafe { read(raw::player_input_handler::mode(self.this.ptr())) }
    }

    pub fn set_mode(&self, mode: u32) {
        // SAFETY: as mode.
        unsafe { write(raw::player_input_handler::mode(self.this.ptr()), mode) }
    }

    /// Whether its game world's hud shows the kits.
    pub fn kits_shown(&self) -> bool {
        // SAFETY: the handler's game world (a pointer the game set) and its
        // byte; none: not shown.
        unsafe {
            pointer(raw::player_input_handler::view_world(self.this.ptr()))
                .is_some_and(|world| read(raw::game_world::kits(world.ptr())) != 0)
        }
    }

    pub fn warmup(&self) -> Warmup {
        let this = self.this.ptr();
        // SAFETY: as mode; f32 fields read as their bits.
        unsafe {
            Warmup {
                yaw: read(raw::player_input_handler::warmup_yaw(this).cast()),
                pitch: read(raw::player_input_handler::warmup_pitch(this).cast()),
                distance: read(raw::player_input_handler::warmup_distance(this).cast()),
            }
        }
    }

    pub fn set_warmup(&self, warmup: Warmup) {
        let this = self.this.ptr();
        // SAFETY: as warmup.
        unsafe {
            write(raw::player_input_handler::warmup_yaw(this).cast(), warmup.yaw);
            write(raw::player_input_handler::warmup_pitch(this).cast(), warmup.pitch);
            write(raw::player_input_handler::warmup_distance(this).cast(), warmup.distance);
        }
    }

    /// The pitch written (its bits).
    pub fn set_warmup_pitch(&self, bits: u32) {
        // SAFETY: as warmup.
        unsafe { write(raw::player_input_handler::warmup_pitch(self.this.ptr()).cast(), bits) }
    }

    /// The distance written (its bits).
    pub fn set_warmup_distance(&self, bits: u32) {
        // SAFETY: as warmup.
        unsafe { write(raw::player_input_handler::warmup_distance(self.this.ptr()).cast(), bits) }
    }

    /// The view (+48h): four rows of four floats.
    pub fn view(&self) -> [[f32; 4]; 4] {
        // SAFETY: the handler's 64 bytes of view.
        unsafe { raw::player_input_handler::view(self.this.ptr()).cast::<[[f32; 4]; 4]>().read_unaligned() }
    }

    pub fn set_view(&self, view: [[f32; 4]; 4]) {
        // SAFETY: as view.
        unsafe { raw::player_input_handler::view(self.this.ptr()).cast::<[[f32; 4]; 4]>().write_unaligned(view) }
    }

    /// The game's view update (update_inverted_view 0x2422B0) on
    /// `transform`, in the handler's mode as it is now.
    pub fn update_view(&self, transform: &[[f32; 4]; 4]) {
        // SAFETY: the game's own update, stdcall (the handler, a 4x4
        // transform it reads), on a handler of the game's.
        unsafe { raw::engine_view_update(self.this.ptr(), transform.as_ptr().cast()) }
    }
}

/// The options the player's look reads: the sensitivity, and whether the
/// mouse is inverted.
pub fn look_options() -> (f32, bool) {
    // SAFETY: the exe's own option values (an f32 and a byte), always there.
    unsafe {
        (read(raw::image::OPTION_SENSITIVITY as usize as *mut f32),
         read(raw::image::OPTION_MOUSE_INVERTION as usize as *mut u8) != 0)
    }
}

/// update_camera's call of the view's update, at `camera.view_update`.
pub struct ViewUpdate<'t> {
    player: Option<Object>,
    handler: Option<Object>,
    transform: Option<Object>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ViewUpdate<'t> {
    pub fn at(ctx: &'t Context<'_>) -> ViewUpdate<'t> {
        let frame = ctx.site_esp() as usize as *mut c_void;
        // SAFETY: the call's two pushed arguments (view_update_frame, by
        // the site's ESP).
        let (handler, transform) = unsafe {
            (pointer(raw::view_update_frame::handler(frame)), pointer(raw::view_update_frame::transform(frame)))
        };
        ViewUpdate { player: Object::at(ctx.reg(Reg::Ebx)), handler, transform, thread: PhantomData }
    }

    /// Whether the player is alive (none: not).
    pub fn player_alive(&self) -> bool {
        // SAFETY: EBX at the site, the player whose camera this is.
        self.player.is_some_and(|player| unsafe { read(raw::player::alive(player.ptr())) } != 0)
    }

    pub fn handler(&self) -> Option<PlayerInput<'t>> {
        self.handler.map(|this| PlayerInput { this, thread: PhantomData })
    }

    /// The eyes' transform (four rows of four floats).
    pub fn transform(&self) -> Option<[[f32; 4]; 4]> {
        // SAFETY: the call's transform, 64 bytes.
        self.transform.map(|at| unsafe { at.ptr().cast::<[[f32; 4]; 4]>().read_unaligned() })
    }
}

/// The new pitch at `warmup_camera.pitch_store` (XMM0's low float), as its
/// bits.
pub fn stored_pitch(ctx: &Context<'_>) -> u32 {
    ctx.xmm_u32(0)
}

const _: () = assert!(size_of::<*mut c_void>() == 4);
