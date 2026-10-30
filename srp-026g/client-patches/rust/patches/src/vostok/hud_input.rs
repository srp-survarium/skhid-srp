//! The match hud's input: the game world's keys at their start
//! (`tab_cursor.key`, game_world::on_keyboard_action 0x25CD00, before its Tab
//! test) and the hud movie's calls to the exe (`hud.callback`,
//! game_world_ui::scaleform_callback 0x233D30, before it matches the
//! method's name), with the hud's cursor and the Tab list they act on.

use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::lobby::Lobby;
use super::ui::{Movie, TEXT_LIMIT, c_string, movie_of, string_argument};
use crate::raw;

/// A DirectInput key code, as the game world's keys come.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key(u32);

impl Key {
    pub const TAB: Key = Key(0x0F);
    pub const LEFT_CONTROL: Key = Key(0x1D);
    pub const RIGHT_CONTROL: Key = Key(0x9D);
}

/// A key's event (the keyboard's process 0x3B9AF0): down and up once each,
/// held at every tick while it is down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Down,
    Up,
    Held,
}

impl KeyKind {
    fn of(raw: u32) -> Option<KeyKind> {
        match raw {
            1 => Some(KeyKind::Down),
            2 => Some(KeyKind::Up),
            3 => Some(KeyKind::Held),
            _ => None,
        }
    }
}

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

impl Hud<'_> {
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

/// A key at the game world's on_keyboard_action's start (`tab_cursor.key`).
pub struct GameWorldKey<'c, 'm> {
    ctx: &'c mut Context<'m>,
    hud: Hud<'c>,
    key: Key,
    kind: KeyKind,
}

impl<'c, 'm> GameWorldKey<'c, 'm> {
    /// None for a kind the keyboard does not give.
    pub fn at_start(ctx: &'c mut Context<'m>) -> Option<GameWorldKey<'c, 'm>> {
        use raw::site::tab_cursor_key as site;
        // SAFETY: the context is this site's: the game world's input handler
        // (its hud inside it) and the function's arguments, alive while it
        // runs on the game's thread.
        let (ui, key, kind) = unsafe {
            let (input, frame) = (site::input(&*ctx)?, site::frame(&*ctx)?);
            (input.ui(), *frame.key(), *frame.state())
        };
        let kind = KeyKind::of(kind)?;
        Some(GameWorldKey { ctx, hud: Hud { this: ui, thread: PhantomData }, key: Key(key), kind })
    }

    pub fn key(&self) -> Key {
        self.key
    }

    pub fn kind(&self) -> KeyKind {
        self.kind
    }

    pub fn hud(&self) -> &Hud<'c> {
        &self.hud
    }

    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }

    /// The key is taken here: no later handler (the player's) gets it. The
    /// caller then leaves by the site's `taken` exit (its `ret 0Ch`).
    pub fn take(self) {
        self.ctx.set_reg8(Reg::Eax, 1);
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
