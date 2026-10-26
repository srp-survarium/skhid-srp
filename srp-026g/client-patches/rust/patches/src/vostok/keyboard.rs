//! The game world's keys at `keyboard.tab_state`: game_world::on_keyboard_action
//! (0x25CD00) after it asked the keyboard for Tab (0x25CD26: BL Tab down,
//! stored at [esp+14h] for show_player_list). At 0x25CD39 `test bl,bl / jnz`
//! sends every key to the function's end while Tab is down (the Tab list
//! takes the keyboard); with BL clear the key goes on, to patch 58's kit keys
//! (0x25CD41) and the binder's actions.

use super::context::{Context, Reg};

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
