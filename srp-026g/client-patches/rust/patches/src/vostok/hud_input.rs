//! The match hud: its view and cursor (the game world's keys that act on
//! them are `keyboard`'s), and the hud movie's calls to the exe (`hud.callback`,
//! game_world_ui::scaleform_callback 0x233D30, before it matches the
//! method's name), with the hud's cursor and the Tab list they act on.
//!
//! The hud lives as long as the game (inside its game world, game+18h+1FCh),
//! so its kits (+2DFh) and cursor (+2DCh) outlast a match: `hud.initialize_match`
//! is its next match's start. The main menu (game_options, game+57F4h) comes
//! over a match with a cursor movie of its own; the hud's is drawn under it
//! until [`CursorMovie`] takes it off.

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

    /// The hud whose show_cursor is about to hide its cursor's movie
    /// (`hud.cursor_hide`).
    pub fn at_cursor_hidden(ctx: &'t Context<'_>) -> Option<Hud<'t>> {
        // SAFETY: as at_cursor_shown, show_cursor's hide.
        let ui = unsafe { raw::site::hud_cursor_hide::ui(ctx)? };
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

    /// The main menu shown over the hud's game: its cursor movie is off the
    /// scene then ([`MenuChange`]).
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

    /// The lobby the match's network client keeps (its messaging, the match's places).
    pub fn lobby(&self) -> Option<Lobby<'_>> {
        // SAFETY: the hud's game world, its game and network client live as
        // long as the hud, on its thread.
        let client = unsafe { self.this.world()?.game()?.network_client()? };
        Some(Lobby::of(client))
    }
}

/// The main menu (game_options, inside the game) as it changes: about to be
/// shown (`options.activate`: Esc in a match, the lobby's menu) or closed
/// (`options.deactivate`: its Resume, its key, a match's end, the lobby's).
/// activate and deactivate return at once when the menu already is so, so
/// there is a change only for a closed menu at activate and a shown one at
/// deactivate ([`MenuClose`]).
///
/// While the menu is shown the hud's cursor movie is off the game world's
/// scene whatever the hud's cursor flag (+2DCh) says: the movie is in the
/// scene exactly when the flag is set and the menu is closed. The scene's
/// remove_movie (world::hide_movie 0x118FE0) erases what std::find returns, so
/// a movie taken off that is not there walks the erase off the end of the
/// list (exe+0x14EE8D); show_cursor's own show and hide under the menu are
/// therefore kept (`hud.cursor_movie`, `hud.cursor_hide`), and the menu's
/// change moves the movie for the flag the hud has then, in or out of a match.
pub struct MenuChange<'t> {
    options: raw::GameOptions,
    thread: PhantomData<&'t Thread>,
}

impl<'t> MenuChange<'t> {
    /// The menu about to be shown, when it is closed now.
    pub fn at_show(ctx: &'t Context<'_>) -> Option<MenuChange<'t>> {
        // SAFETY: the context is this site's: game_options::activate's menu,
        // inside the game, on the game's thread; its shown byte is its own.
        let (options, shown) = unsafe {
            let options = raw::site::options_activate::options(ctx)?;
            (options, *options.shown() != 0)
        };
        (!shown).then_some(MenuChange { options, thread: PhantomData })
    }

    /// The menu about to close, when it is shown now.
    pub fn at_close(ctx: &'t Context<'_>) -> Option<MenuClose<'t>> {
        // SAFETY: as at_show, game_options::deactivate's menu; its awaited
        // action is a word of its own.
        let (options, shown, awaited) = unsafe {
            let options = raw::site::options_deactivate::options(ctx)?;
            (options, *options.shown() != 0, *options.awaited_action())
        };
        shown.then_some(MenuClose {
            change: MenuChange { options, thread: PhantomData },
            awaited: AwaitedKey::of(awaited),
        })
    }

    /// The hud's cursor movie when the hud's cursor is shown (+2DCh): the
    /// game's one game world and hud, in a match or not (a match's end
    /// clears the world's active byte before it closes the menu).
    pub fn shown_cursor(&self) -> Option<CursorMovie<'t>> {
        // SAFETY: the menu's game, its game world and hud inside it, alive as
        // long as the game, on the game's thread; plain bytes of theirs.
        unsafe {
            let world = self.options.game()?.world();
            let ui = world.ui();
            (*ui.cursor_shown() != 0).then_some(CursorMovie { world, ui, thread: PhantomData })
        }
    }
}

/// What the shown menu's key-binding settings wait for (game_options +3Ch):
/// the action a click on its key asked a key for (start_bind_key, 0x1FF8A3),
/// until a key, a mouse button or the wheel gives it one or Esc cancels
/// (finish_binding 0x2015A0 and its copies, a conflict's message); 48h none.
/// While one is awaited the menu's own cursor movie (+10h) is off the scene
/// (0x1FF8A9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwaitedKey {
    /// The action's row (the table at 0xA9DB28); never 48h: made only by [`AwaitedKey::of`].
    action: u32,
}

impl AwaitedKey {
    /// No key awaited: the constructor's (0x1FFA13) and finish_binding's (0x2015C8) value.
    const NONE: u32 = 0x48;

    /// The awaited key of the menu's +3Ch, as the game reads it (`cmp
    /// [options+3Ch],48h`, 0x2015E3): any other value is an action.
    pub fn of(raw: u32) -> Option<AwaitedKey> {
        (raw != Self::NONE).then_some(AwaitedKey { action: raw })
    }
}

/// The shown main menu about to close (`options.deactivate`, before its
/// stock body). deactivate takes the menu movie (+0Ch) and its cursor movie
/// (+10h) off the scene whatever the menu waits for, but while a key is
/// awaited the cursor movie is off already: closing then (a match's end, a
/// kick, a match that starts over the lobby's menu) took it off a second
/// time, the crash at exe+0x14EE8D. [`MenuClose::close`] ends the wait first,
/// as Esc ends it, so the menu's movies are in the scene exactly while it is
/// shown and awaits no key.
#[must_use = "dropped, the menu closes with its key wait (exe+0x14EE8D) and the hud's cursor stays off"]
pub struct MenuClose<'t> {
    change: MenuChange<'t>,
    awaited: Option<AwaitedKey>,
}

impl<'t> MenuClose<'t> {
    /// Closes the menu's key wait, then hands back the hud's cursor movie when
    /// the hud's cursor is shown, for the menu's close to put it back.
    ///
    /// An awaited key is given up as Esc gives it up (on_keyboard_action's
    /// press, 0x2015F6..0x201605): finish_binding tells the movie
    /// (`root.end_keybind`, which closes its "press a key" message), puts the
    /// cursor movie back in the scene and clears +3Ch, so deactivate's own
    /// hide (0x201058) takes off a movie the scene holds. The show and the
    /// hide go to the render world in one queue, in that order.
    pub fn close(self) -> Option<CursorMovie<'t>> {
        if self.awaited.is_some() {
            // SAFETY: the shown menu at deactivate, on the game's thread; a
            // key awaited means its movie called start_bind_key, so its menu
            // and cursor movies are loaded (finish_binding reads both).
            unsafe { raw::engine_finish_key_binding(Some(self.change.options)) }
        }
        self.change.shown_cursor()
    }
}

/// The hud's cursor movie while the hud's cursor is shown (+2DCh): in the game
/// world's scene with the menu closed, out of it with the menu shown.
pub struct CursorMovie<'t> {
    world: raw::GameWorld,
    ui: raw::GameWorldUi,
    thread: PhantomData<&'t Thread>,
}

impl CursorMovie<'_> {
    /// Takes the movie off the scene as the menu comes; the hud keeps its
    /// cursor shown, so its mouse handling is as before.
    pub fn withdraw(&self) {
        // SAFETY: the live hud's cursor movie and its game world, the scene
        // show_cursor hides it from, on the game's thread.
        unsafe { raw::engine_hide_scene_movie(Some(self.ui.cursor_movie()), Some(self.world)) }
    }

    /// Puts the movie back as the menu closes, as show_cursor shows it; only
    /// a loaded one (show_movie reads the holder).
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

#[cfg(test)]
mod tests {
    use super::AwaitedKey;

    #[test]
    fn awaited_key_is_any_action_but_48h() {
        assert_eq!(AwaitedKey::of(0x48), None);
        assert_eq!(AwaitedKey::of(0), Some(AwaitedKey { action: 0 }));
        assert_eq!(AwaitedKey::of(0x47), Some(AwaitedKey { action: 0x47 }));
        assert_eq!(AwaitedKey::of(0x49), Some(AwaitedKey { action: 0x49 }));
    }
}
