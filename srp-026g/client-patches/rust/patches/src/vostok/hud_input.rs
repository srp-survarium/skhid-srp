//! The match hud: its view and cursor (the game world's keys that act on
//! them are `keyboard`'s), and the hud movie's calls to the exe (`hud.callback`,
//! game_world_ui::scaleform_callback 0x233D30, before it matches the
//! method's name), with the hud's cursor and the Tab list they act on.

use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::lobby::Lobby;
use super::ui::{Movie, TEXT_LIMIT, c_string, movie_of, string_argument};
use crate::raw;

/// What the hud shows, as the game keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudView {
    /// The Tab list (show_player_list's +0BAh, from the key before this one).
    pub list: bool,
    /// The hud's cursor (+2DCh): the mouse goes to the hud's movies, not the player.
    pub cursor: bool,
    /// The kits (+2DFh), which show and hide the cursor with them.
    pub kits: bool,
}

/// The hud a hook's site has: its view and its cursor.
pub struct Hud<'t> {
    this: raw::GameWorldUi,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Hud<'t> {
    /// # Safety
    /// `this` is the live hud of the hook's site, alive for `'t` on the game's thread.
    pub(super) unsafe fn of(this: raw::GameWorldUi) -> Hud<'t> {
        Hud { this, thread: PhantomData }
    }

    pub fn view(&self) -> HudView {
        // SAFETY: the hud is alive for the hook, on its thread.
        unsafe {
            HudView {
                list: *self.this.list_shown() != 0,
                cursor: *self.this.cursor_shown() != 0,
                kits: *self.this.kits_shown() != 0,
            }
        }
    }

    /// Shows or hides the hud's cursor (show_cursor 0x23A930, as the kits do:
    /// shown, it starts at the screen's centre; nothing when it already is so).
    pub fn show_cursor(&self, shown: bool) {
        // SAFETY: the live hud on the game's thread, the kits' own call.
        unsafe { raw::engine_show_cursor(Some(self.this), u32::from(shown)) }
    }

    /// The hud movie (hud.swf), when the hud has it.
    pub fn movie(&self) -> Option<Movie<'_>> {
        // SAFETY: the hud is alive for the hook, on its thread.
        unsafe { movie_of(self.this.hud_movie()) }
    }

    /// The lobby the match's network client keeps (its messaging, the match's places).
    pub fn lobby(&self) -> Option<Lobby<'_>> {
        // SAFETY: the hud's game world, its game and network client live as
        // long as the hud, on its thread.
        let client = unsafe { self.this.world()?.game()?.network_client()? };
        Some(Lobby::of(client))
    }
}

/// The hud movie's call to the exe at `hud.callback`: its
/// `ExternalInterface.call(method, ...)`, before the hud matches the name.
pub struct HudCall<'c> {
    hud: Hud<'c>,
    frame: raw::HudCallbackFrame,
    method: &'c CStr,
    thread: &'c Thread,
}

impl<'c> HudCall<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<HudCall<'c>> {
        use raw::site::hud_callback as site;
        // SAFETY: the context is this site's: the hud and the callback's
        // frame, alive for the call, on the game's thread; the method's name a
        // C string the movie gave.
        let (ui, frame, method) = unsafe {
            let (ui, frame) = (site::ui(ctx)?, site::frame(ctx)?);
            (ui, frame, c_string(frame.method(), TEXT_LIMIT)?)
        };
        Some(HudCall { hud: Hud { this: ui, thread: PhantomData }, frame, method, thread: ctx.thread() })
    }

    pub fn method(&self) -> &CStr {
        self.method
    }

    /// The call's argument `index` when it is a string.
    pub fn string_argument(&self, index: u32) -> Option<&CStr> {
        // SAFETY: the callback's arguments and their count, alive for the call.
        unsafe { string_argument(self.frame.arguments(), *self.frame.count(), index) }
    }

    pub fn hud(&self) -> &Hud<'c> {
        &self.hud
    }

    pub fn thread(&self) -> &Thread {
        self.thread
    }

    /// Answers the hud movie: invokes its `method` with plain unsigned values
    /// (at most four); whether the movie took it.
    pub fn answer(&self, method: &CStr, values: &[u32]) -> bool {
        self.hud.movie().is_some_and(|movie| movie.invoke_uints(method, values))
    }
}
