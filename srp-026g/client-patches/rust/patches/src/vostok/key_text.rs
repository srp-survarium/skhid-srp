//! Keys in texts (patch 212, crate::key_prompts): a dictionary text's `{NAME}`
//! is the key bound to the action NAME, and the movies' texts are made again
//! when the bindings change.
//!
//! The game's localizer, text_translator::expand_text (0x1FEF90), runs on
//! every text translate_text (0x1FF0F0) gives: each text a movie is handed
//! (set_localization_data), each message, each prompt. It copies a `{NAME}`'s
//! name to its frame (+10h) and, for USE alone, translates the name of the key
//! bound to Use (binding 35h) into its frame (+118h), then appends that. With
//! patch 212 every name reaches `text_translator.expand_name` ([`Placeholder`]).
//!
//! The key is named as the game's own Settings name it (game_options
//! 0x201CF0) and the stock USE did: the binding's first key, else its second;
//! the first row of the key names' table with that key's code; that row's
//! name ("kE", "mouse1") translated, the dictionary's text of the running
//! language ("E", "LMB", "Leertaste").
//!
//! The hud and the lobby are handed their texts once (the hud at every match's
//! initialize_resources, the lobby once its movie is loaded), with the keys of
//! the moment. Each records the bindings it was handed them with
//! ([`BindingsStamp`], at `hud.texts` and `lobby_menu.texts`); each frame of
//! that scene ([`GameWorldUi::texts_stamp`] at the hud's update,
//! [`LobbyFrame`] at the lobby's) compares them with the bindings now, and the
//! game's own function hands the movie its texts again
//! ([`GameWorldUi::remake_texts`], [`LobbyFrame::remake_texts`]).

use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::ui::{GameWorldUi, c_string, movie_of};
use crate::raw;

/// The key binder's bindings: patch 149's 60h (patch_026g.py's
/// ITEM_HOTKEYS_BINDINGS: the binder's allocation, its construction and
/// get_binded_action's count), one per action id.
const BINDINGS: usize = 0x60;

/// More rows than the key names' table has (135): a bound on its walk.
const KEY_NAMES: usize = 0x200;

/// A name's room in expand_text's frame (+10h up to its text at +118h).
const NAME_ROOM: usize = 0x108;

/// An action's name or a key's: well under this.
const NAME_LIMIT: usize = 0x80;

/// The bindings as they are: every binding's two keys' codes. Equal
/// stamps, the same keys for every action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingsStamp([[Option<u32>; 2]; BINDINGS]);

/// The key a binding names, by the dictionary key of its text ("kE",
/// "mouse1"): the key names' table's first row of its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundKey<'t> {
    pub name: &'t CStr,
}

/// An action's binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding<'t> {
    Bound(BoundKey<'t>),
    /// No key (or none the key names' table names).
    Unbound,
}

/// The game's key binder (game+5344h), on the hook's thread.
#[derive(Clone, Copy)]
pub struct KeyBinder<'t> {
    this: raw::KeyBinder,
    thread: PhantomData<&'t Thread>,
}

impl<'t> KeyBinder<'t> {
    /// # Safety
    /// `game` is alive on the hook's thread for `'t`.
    unsafe fn of_game(game: raw::Game) -> Option<KeyBinder<'t>> {
        // SAFETY: the caller's live game; its binder is made with it (0x1EA288) and freed with it.
        let this = unsafe { game.binder() }?;
        Some(KeyBinder { this, thread: PhantomData })
    }

    /// The two keys' codes of binding `id`.
    fn codes(&self, id: usize) -> [Option<u32>; 2] {
        // SAFETY: a binding of the live binder (id under its 60h); a key a row of the key names'
        // table, static data.
        unsafe {
            let binding = self.this.binding().at(id);
            [binding.key(), binding.second_key()].map(|key| key.map(|key| *key.code()))
        }
    }

    pub fn stamp(&self) -> BindingsStamp {
        BindingsStamp(core::array::from_fn(|id| self.codes(id)))
    }

    /// The binding of the action named `name`, ASCII case aside (a text's
    /// `{USE}` is the action `use`); None when no action has that name.
    pub fn binding(&self, name: &[u8]) -> Option<Binding<'t>> {
        let id = (0..BINDINGS).find(|&id| {
            // SAFETY: a binding of the live binder; its description a row of the action table
            // (patch 149's `actions`, static data), its name a C string there.
            let row_name = unsafe { self.this.binding().at(id).description().and_then(|row| c_string(row.name(), NAME_LIMIT)) };
            row_name.is_some_and(|row_name| row_name.to_bytes().eq_ignore_ascii_case(name))
        })?;
        let code = self.codes(id).into_iter().flatten().next();
        Some(code.and_then(key_named).map_or(Binding::Unbound, |name| Binding::Bound(BoundKey { name })))
    }
}

/// The name of the key names' table's first row of `code`, as keycode_to_ptr (0x207BF0) finds it.
fn key_named<'t>(code: u32) -> Option<&'t CStr> {
    let first = raw::image::key_names();
    (0..KEY_NAMES)
        .map_while(|index| {
            // SAFETY: the binder's static table, its rows read up to the one whose name is 0
            // (map_while stops there), each name a C string of the image.
            unsafe {
                let row = first.at(index);
                let name = row.name();
                (!name.is_null()).then(|| (name, *row.code()))
            }
        })
        .find(|&(_, row_code)| row_code == code)
        // SAFETY: as above: a C string of the image.
        .and_then(|(name, _)| unsafe { c_string(name, NAME_LIMIT) })
}

/// A `{NAME}` of a text at `text_translator.expand_name`.
pub struct Placeholder<'c> {
    translator: raw::TextTranslator,
    frame: raw::ExpandTextFrame,
    thread: PhantomData<&'c Thread>,
}

impl<'c> Placeholder<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<Placeholder<'c>> {
        use raw::site::text_translator_expand_name as site;
        // SAFETY: the context is this site's: expand_text's translator (EBP) and frame (ESP), alive
        // while it runs on the game's thread.
        let (translator, frame) = unsafe { (site::translator(ctx)?, site::frame(ctx)?) };
        Some(Placeholder { translator, frame, thread: PhantomData })
    }

    /// Its name, as the text writes it ("USE").
    pub fn name(&self) -> &[u8] {
        // SAFETY: the frame's copy of the name, zero-terminated by expand_text (0x1FEFFE) in its
        // room; read no further than the room.
        unsafe { c_string(self.frame.name(), NAME_ROOM) }.map_or(&[], CStr::to_bytes)
    }

    /// The key binder of the translator's game.
    pub fn binder(&self) -> Option<KeyBinder<'c>> {
        // SAFETY: the translator is in its game (+52A8h), alive while it translates.
        unsafe { KeyBinder::of_game(self.translator.game()?) }
    }

    /// The placeholder becomes the dictionary text of `key` (translate_text of the translator into
    /// the frame's room, which the site's `text` exit appends, as the stock USE path does).
    pub fn becomes(&mut self, key: &CStr) {
        // SAFETY: the live translator; the frame's 200h bytes of room for the text, which
        // translate_text fills with a text and its zero (a dictionary's key's text is under 200h
        // bytes, the catalog check's limit) or leaves as it is (the zero written first).
        unsafe {
            *self.frame.text() = 0;
            raw::engine_translate_text(Some(self.translator), key.as_ptr().cast(), self.frame.text());
        }
    }
}

impl GameWorldUi<'_> {
    /// The hud's movie (its id) and the bindings now, when it has a movie and a game.
    pub fn texts_stamp(&self) -> Option<(usize, BindingsStamp)> {
        let movie = self.hud_movie()?.id();
        // SAFETY: the live ui's world and its game, alive while the hud is.
        let binder = unsafe { KeyBinder::of_game(self.raw().world()?.game()?) }?;
        Some((movie, binder.stamp()))
    }

    /// Hands the hud movie its texts again, with the bindings now (the game's own
    /// game_world_ui::set_localization_data, 0x2346E0, which `hud.texts` hears).
    pub fn remake_texts(&self) {
        if self.hud_movie().is_none() {
            return;
        }
        // SAFETY: the live ui, in its update on the game's thread, with its movie (+8) loaded and its
        // world's game (+18h, +9Ch): what the function reads; it reads the mapping and translates.
        unsafe {
            if self.raw().world().and_then(|world| world.game()).is_some() {
                raw::engine_hud_texts(Some(self.raw()));
            }
        }
    }
}

/// At `hud.texts`: the hud (its movie's id) and the bindings it is handed its texts with.
pub fn hud_texts_at(ctx: &Context<'_>) -> Option<(usize, BindingsStamp)> {
    // SAFETY: the context is this site's: set_localization_data's argument, the hud, alive while
    // it runs on the game's thread.
    let ui = unsafe { raw::site::hud_texts::frame(ctx)?.ui() }?;
    GameWorldUi::of(ui, ctx.thread()).texts_stamp()
}

/// The lobby menu at `lobby_menu.texts` (handed its texts) or `lobby_menu.update_ui` (a frame).
pub struct LobbyFrame<'t> {
    menu: raw::LobbyMenu,
    thread: PhantomData<&'t Thread>,
}

impl<'t> LobbyFrame<'t> {
    pub fn at_texts(ctx: &'t Context<'_>) -> Option<LobbyFrame<'t>> {
        // SAFETY: the context is this site's: set_localization_data's argument, the lobby menu,
        // alive while it runs on the game's thread.
        let menu = unsafe { raw::site::lobby_menu_texts::frame(ctx)?.menu() }?;
        Some(LobbyFrame { menu, thread: PhantomData })
    }

    pub fn at_update(ctx: &'t Context<'_>) -> Option<LobbyFrame<'t>> {
        // SAFETY: the context is this site's: update_ui's lobby menu (ESI), alive while it runs.
        let menu = unsafe { raw::site::lobby_menu_update_ui::menu(ctx) }?;
        Some(LobbyFrame { menu, thread: PhantomData })
    }

    /// The lobby's movie (its id) and the bindings now, when it has a movie and a game.
    pub fn texts_stamp(&self) -> Option<(usize, BindingsStamp)> {
        // SAFETY: the live menu's movie holder (+690h) and game (+9Ch), alive while the menu is.
        unsafe {
            let movie = movie_of(self.menu.movie())?.id();
            let binder = KeyBinder::of_game(self.menu.game()?)?;
            Some((movie, binder.stamp()))
        }
    }

    /// Hands the lobby movie its texts again, with the bindings now (the game's own
    /// lobby_menu::set_localization_data, 0x286260, which `lobby_menu.texts` hears).
    pub fn remake_texts(&self) {
        // SAFETY: the live menu in its frame on the game's thread, with its movie (+690h) and its
        // game (+9Ch): what the function reads; it reads the mapping and translates.
        unsafe {
            if movie_of(self.menu.movie()).is_some() && self.menu.game().is_some() {
                raw::engine_lobby_texts(Some(self.menu));
            }
        }
    }
}
