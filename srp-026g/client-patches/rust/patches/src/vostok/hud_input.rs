//! The match hud: its view and cursor (the game world's keys that act on
//! them are `keyboard`'s), and the hud movie's calls to the exe (`hud.callback`,
//! game_world_ui::scaleform_callback 0x233D30, before it matches the
//! method's name), with the hud's cursor and the Tab list they act on.
//!
//! The hud lives as long as the game (inside its game world, game+18h+1FCh),
//! so its kits (+2DFh) and cursor (+2DCh) outlast a match: `hud.initialize_match`
//! is its next match's start. The main menu (game_options, game+57F4h) comes
//! over a match with a cursor movie of its own; the hud's is drawn under it
//! until [`MatchCursor`] takes it off.

use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::lobby::Lobby;
use super::world::NetworkClient;
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

    /// The hud at its match's start (`hud.initialize_match`): once a match,
    /// before the match's kits are shown.
    pub fn at_match_start(ctx: &'t Context<'_>) -> Option<Hud<'t>> {
        // SAFETY: the context is this site's: initialize_match's first
        // argument, the live hud, on the game's thread.
        let ui = unsafe { raw::site::hud_initialize_match::frame(ctx)?.ui()? };
        Some(Hud { this: ui, thread: PhantomData })
    }

    /// The hud whose show_cursor is about to show its cursor's movie
    /// (`hud.cursor_movie`).
    pub fn at_cursor_shown(ctx: &'t Context<'_>) -> Option<Hud<'t>> {
        // SAFETY: the context is this site's: show_cursor's hud, alive for
        // the call, on the game's thread.
        let ui = unsafe { raw::site::hud_cursor_movie::ui(ctx)? };
        Some(Hud { this: ui, thread: PhantomData })
    }

    /// Shows or hides the hud's cursor (show_cursor 0x23A930, as the kits do:
    /// shown, it starts at the screen's centre; nothing when it already is so).
    pub fn show_cursor(&self, shown: bool) {
        // SAFETY: the live hud on the game's thread, the kits' own call.
        unsafe { raw::engine_show_cursor(Some(self.this), u32::from(shown)) }
    }

    /// The kits and the cursor hidden, as the hud's constructor leaves them
    /// and the hud movie's `reset` its kits: what the last match left is
    /// gone, so the match's first showing of the kits (show_profiles_selector
    /// 0x2383B0, which returns at once while +2DFh says they are shown)
    /// tells the movie the kit the match gave.
    pub fn start_over(&self) {
        self.show_cursor(false);
        // SAFETY: the live hud on the game's thread; the kits' state is a
        // byte of its own, which show_profiles_selector alone reads and writes.
        unsafe { *self.this.kits_shown() = 0 };
    }

    /// The main menu shown over the hud's game.
    pub fn under_menu(&self) -> bool {
        // SAFETY: the hud's game world and game live as long as the hud, the
        // menu inside the game; on the game's thread.
        unsafe { self.this.world().and_then(|world| world.game()).is_some_and(|game| *game.options().shown() != 0) }
    }

    /// The hud movie (hud.swf), when the hud has it.
    pub fn movie(&self) -> Option<Movie<'_>> {
        // SAFETY: the hud is alive for the hook, on its thread.
        unsafe { movie_of(self.this.hud_movie()) }
    }

    /// The player icons' movie (player_icons.swf), when the hud has it.
    pub fn icons_movie(&self) -> Option<Movie<'_>> {
        // SAFETY: the hud is alive for the hook, on its thread.
        unsafe { movie_of(self.this.icons_movie()) }
    }

    /// The game's network client, on the hook's `thread`.
    pub fn client<'a>(&self, thread: &'a Thread) -> Option<NetworkClient<'a>> {
        // SAFETY: the hud's game world, its game and network client live as
        // long as the hud, on its thread.
        let client = unsafe { self.this.world()?.game()?.network_client()? };
        Some(NetworkClient::of(client, thread))
    }

    /// The lobby the match's network client keeps (its messaging, the match's places).
    pub fn lobby(&self) -> Option<Lobby<'_>> {
        // SAFETY: the hud's game world, its game and network client live as
        // long as the hud, on its thread.
        let client = unsafe { self.this.world()?.game()?.network_client()? };
        Some(Lobby::of(client))
    }
}

/// The main menu (game_options, inside the game) as it is about to be shown
/// (`options.activate`: Esc in a match, the lobby's menu) or closed
/// (`options.deactivate`: its Resume, its key, a match's end, the lobby's).
pub struct MainMenu<'t> {
    options: raw::GameOptions,
    thread: PhantomData<&'t Thread>,
}

impl<'t> MainMenu<'t> {
    pub fn at_show(ctx: &'t Context<'_>) -> Option<MainMenu<'t>> {
        // SAFETY: the context is this site's: game_options::activate's menu,
        // inside the game, on the game's thread.
        let options = unsafe { raw::site::options_activate::options(ctx)? };
        Some(MainMenu { options, thread: PhantomData })
    }

    pub fn at_close(ctx: &'t Context<'_>) -> Option<MainMenu<'t>> {
        // SAFETY: as at_show, game_options::deactivate's menu.
        let options = unsafe { raw::site::options_deactivate::options(ctx)? };
        Some(MainMenu { options, thread: PhantomData })
    }

    /// Shown now (at its close: whether there is anything to close).
    pub fn shown(&self) -> bool {
        // SAFETY: the menu is alive for the hook; a byte of its own.
        unsafe { *self.options.shown() != 0 }
    }

    /// The hud's cursor of the match the menu is over: its game world the
    /// active scene (not the lobby's time, not a match's end, which clears it
    /// before it closes the menu) and the cursor shown.
    pub fn match_cursor(&self) -> Option<MatchCursor<'t>> {
        // SAFETY: the menu's game, its game world and hud inside it, alive as
        // long as the game, on the game's thread; plain bytes of theirs.
        unsafe {
            let world = self.options.game()?.world();
            let ui = world.ui();
            (*world.active() != 0 && *ui.cursor_shown() != 0).then_some(MatchCursor { world, ui, thread: PhantomData })
        }
    }
}

/// The hud's cursor in a running match, shown (+2DCh): its movie drawn, or
/// taken off the screen while the main menu, which draws its own, covers it.
pub struct MatchCursor<'t> {
    world: raw::GameWorld,
    ui: raw::GameWorldUi,
    thread: PhantomData<&'t Thread>,
}

impl MatchCursor<'_> {
    /// Takes the cursor's movie off the screen; the hud keeps it shown, so its
    /// mouse handling is as before. The renderer takes out every copy, and
    /// does nothing when there is none.
    pub fn withdraw(&self) {
        // SAFETY: the live hud's cursor movie and its game world, the scene
        // show_cursor hides it from, on the game's thread.
        unsafe { raw::engine_hide_scene_movie(Some(self.ui.cursor_movie()), Some(self.world)) }
    }

    /// Puts the movie back, as show_cursor shows it; only a loaded one
    /// (show_movie reads the holder).
    pub fn restore(&self) {
        // SAFETY: as withdraw; the holder is read once, show_movie's own read.
        unsafe {
            let movie = self.ui.cursor_movie();
            if movie.holder().is_some() {
                raw::engine_show_scene_movie(Some(self.world), Some(movie));
            }
        }
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
