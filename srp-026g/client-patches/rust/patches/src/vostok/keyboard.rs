//! The game world's keys at `keyboard.tab_state`: game_world::on_keyboard_action
//! (0x25CD00) after it asked the keyboard for Tab (0x25CD26: BL Tab down,
//! stored at [esp+14h] for show_player_list). At 0x25CD39 `test bl,bl / jnz`
//! sends every key to the function's end while Tab is down (the Tab list
//! takes the keyboard); with BL clear the key goes on, to patch 58's kit keys
//! (0x25CD41) and the binder's actions. Before that ask, at `keyboard.key_state`
//! (0x25CD1C), the input world's keyboard: its keys' state.

use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::ui::{Movie, movie_of};
use crate::raw;

/// The key being handled, after the game's ask of Tab.
pub struct TabTest<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> TabTest<'c, 'm> {
    pub fn at(ctx: &'c mut Context<'m>) -> TabTest<'c, 'm> {
        TabTest { ctx }
    }

    /// Tab is down.
    pub fn tab_down(&self) -> bool {
        self.ctx.reg(Reg::Ebx) & 0xFF != 0
    }

    /// The key goes on as with Tab up (show_player_list still gets Tab's byte, stored before).
    pub fn key_goes_on(&mut self) {
        self.ctx.set_reg8(Reg::Ebx, 0);
    }
}

/// A key by its DirectInput code (`DIK_*`), the index of the keyboard's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dik(u8);

impl Dik {
    /// DIK_LMENU: the left Alt, the key the game shows the names over the players by (render_name 0x1F2DCD).
    pub const LEFT_ALT: Dik = Dik(0x38);
    /// DIK_RMENU: the right Alt (alt_is_held 0x241200 asks both).
    pub const RIGHT_ALT: Dik = Dik(0xB8);
}

/// The game world's keys at `keyboard.key_state` (0x25CD1C): on_keyboard_action
/// with the input world's keyboard, before it asks it for Tab. The game world
/// gets every key before the player's handler, a held one every tick (the
/// keyboard's process 0x3B9AF0, kind 3), so the state read here is this tick's
/// poll. Only the platform keyboard's state is read (its vtable checked).
pub struct GameWorldKeys<'t> {
    keys: raw::KeyState,
    ui: raw::GameWorldUi,
    thread: PhantomData<&'t Thread>,
}

impl<'t> GameWorldKeys<'t> {
    pub fn at_key_state(ctx: &'t Context<'_>) -> Option<GameWorldKeys<'t>> {
        use raw::site::keyboard_key_state as site;
        // SAFETY: the context is this site's: the input world's live keyboard
        // (EAX, its vtable+24h's answer) and the game world's input handler
        // (EDI, the hud inside it), on the game's thread. The keys are read
        // only after the vtable says it is the platform keyboard, whose state
        // is its +4.
        let (keys, ui) = unsafe {
            let (keyboard, input) = (site::keyboard(ctx)?, site::input(ctx)?);
            if *keyboard.vtable() != raw::image::KEYBOARD_VTABLE {
                return None;
            }
            (keyboard.keys(), input.ui())
        };
        Some(GameWorldKeys { keys, ui, thread: PhantomData })
    }

    /// The key is down in this tick's poll (bit 80h, as is_key_down 0x3B8D00 reads it).
    pub fn down(&self, key: Dik) -> bool {
        // SAFETY: a platform keyboard's 256 states, the key one of them (a u8 code).
        unsafe { *self.keys.at(usize::from(key.0)).state() & 0x80 != 0 }
    }

    /// The hud movie, when the hud has it.
    pub fn hud_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: the hud lives inside the game world, alive for the hook.
        unsafe { movie_of(self.ui.hud_movie()) }
    }
}
