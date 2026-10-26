//! The match's interface: survarium::game_world_ui (the hud) and the
//! Scaleform movies it holds, the hud's and the player icons', at the site
//! `player_icons.update_start` (update_player_icons' start, 0x23AA10: the ui
//! its argument, by the site's ESP).

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::context::{Context, Reg};
use super::gfx::Movie;
use super::hook::Thread;
use super::object::Object;
use crate::raw;

/// The hud, survarium::game_world_ui, as a hook's site has it.
pub struct GameWorldUi<'t> {
    this: NonNull<c_void>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> GameWorldUi<'t> {
    /// At `player_icons.update_start`.
    pub fn at_update_icons(ctx: &'t Context<'_>) -> Option<GameWorldUi<'t>> {
        // the site's ESP: the entry pushed its dispatch slot and the flags
        // before `pushad` saved ESP (hooks_026g.lohook_entry)
        let frame = ctx.reg(Reg::Esp).wrapping_add(8) as usize as *mut c_void;
        // SAFETY: the function's argument at its start (update_player_icons_frame).
        let ui = unsafe { *raw::update_player_icons_frame::ui(frame) };
        NonNull::new(ui as usize as *mut c_void).map(|this| GameWorldUi { this, thread: PhantomData })
    }

    /// The hud movie (hud.swf), when the hud has it.
    pub fn hud_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: the handle is a live game_world_ui; its holder is a movie holder or 0.
        unsafe { movie_of(*raw::game_world_ui::hud_movie(self.this.as_ptr())) }
    }

    /// The player icons movie (player_icons.swf, the game world's own), when the hud has it.
    pub fn icons_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: as hud_movie.
        unsafe { movie_of(*raw::game_world_ui::icons_movie(self.this.as_ptr())) }
    }
}

/// # Safety
/// `holder` is a movie holder or 0, alive on the hook's thread.
unsafe fn movie_of<'t>(holder: u32) -> Option<Movie<'t>> {
    // SAFETY: the caller's: a live movie holder, its view a movie view or 0.
    unsafe { Movie::of_owner(Object::at(holder)?) }
}
