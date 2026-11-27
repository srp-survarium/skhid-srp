//! Patch 212: keys in texts (client-patches/README.md "Keys in texts").
//!
//! The game's localizer made a text's `{USE}` the key bound to Use and any
//! other `{NAME}` nothing, and an unbound Use "()". Here every `{NAME}` that
//! is an action's name (ASCII case aside: `{USE}`, `{SCOPE_NEXT}`,
//! `{HIDE_INTERFACE}`) is the key bound to that action, named as the game's
//! Settings name it ("E", "LMB", "Leertaste"), and an action without a key is
//! `st_srp_key_unbound` ("unbound" in the player's language). Any other name
//! is nothing, as before.
//!
//! A movie is handed its texts once (the hud at each match's start, the lobby
//! once loaded), so a key rebound in the Settings left the old key in them
//! until the next match. Each scene keeps the bindings its texts were made
//! with; a frame of it that finds other bindings has the game make its texts
//! again (vostok::key_text). Only text: nothing a match computes.
#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::key_text::{Binding, BindingsStamp, LobbyFrame, Placeholder, hud_texts_at};
use crate::vostok::{Context, Flow, GameCell, GameWorldUi, KeyPromptsOnPlaceholderExit, NoExit};

/// What an action with no key is called.
const UNBOUND: &CStr = c"st_srp_key_unbound";

/// The movie a scene handed its texts to (its id) and the bindings they were made with.
type Told = (usize, BindingsStamp);

static HUD: GameCell<Option<Told>> = GameCell::new(None);
static LOBBY: GameCell<Option<Told>> = GameCell::new(None);

/// What a scene's frame does with its movie's texts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Texts {
    Keep,
    /// Made again: the movie that was handed them is the scene's still, and the bindings changed.
    Remake,
}

/// `told`: the movie last handed its texts and the bindings then; `now`: the scene's movie and the
/// bindings now. A movie not handed its texts yet (a new hud, a lobby before its scene is ready) is
/// left to the game, which hands them itself.
fn texts_at_frame<S: PartialEq>(told: Option<(usize, S)>, now: Option<(usize, S)>) -> Texts {
    match (told, now) {
        (Some((told_movie, told_stamp)), Some((movie, stamp))) if told_movie == movie && told_stamp != stamp => {
            Texts::Remake
        }
        _ => Texts::Keep,
    }
}

/// expand_text at a `{NAME}`: the action's key, "unbound", or (no action of that name) nothing.
pub fn on_placeholder(ctx: &mut Context<'_>) -> Flow<KeyPromptsOnPlaceholderExit> {
    let Some(mut placeholder) = Placeholder::at(ctx) else { return Flow::Continue };
    let Some(binder) = placeholder.binder() else { return Flow::Continue };
    let key = match binder.binding(placeholder.name()) {
        None => return Flow::Continue,
        Some(Binding::Bound(key)) => key.name,
        Some(Binding::Unbound) => UNBOUND,
    };
    placeholder.becomes(key);
    Flow::Exit(KeyPromptsOnPlaceholderExit::Text)
}

/// The hud handed its texts: the bindings they are made with.
pub fn on_hud_texts(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let told = hud_texts_at(ctx);
    HUD.set(ctx.thread(), told);
    Flow::Continue
}

/// A frame of a match: the hud's texts made again when the bindings changed.
pub fn on_hud_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(ui) = GameWorldUi::at_update_icons(ctx) else { return Flow::Continue };
    if texts_at_frame(HUD.get(ctx.thread()), ui.texts_stamp()) == Texts::Remake {
        ui.remake_texts();
    }
    Flow::Continue
}

/// The lobby handed its texts: the bindings they are made with.
pub fn on_lobby_texts(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let told = LobbyFrame::at_texts(ctx).and_then(|lobby| lobby.texts_stamp());
    LOBBY.set(ctx.thread(), told);
    Flow::Continue
}

/// A frame of the lobby: its texts made again when the bindings changed.
pub fn on_lobby_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(lobby) = LobbyFrame::at_update(ctx) else { return Flow::Continue };
    if texts_at_frame(LOBBY.get(ctx.thread()), lobby.texts_stamp()) == Texts::Remake {
        lobby.remake_texts();
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_movie_with_other_bindings_is_made_again() {
        assert_eq!(texts_at_frame(Some((1, 10)), Some((1, 11))), Texts::Remake);
    }

    #[test]
    fn the_same_bindings_keep_the_texts() {
        assert_eq!(texts_at_frame(Some((1, 10)), Some((1, 10))), Texts::Keep);
    }

    #[test]
    fn a_movie_not_handed_its_texts_is_left_to_the_game() {
        assert_eq!(texts_at_frame(None, Some((1, 11))), Texts::Keep);
        assert_eq!(texts_at_frame(Some((1, 10)), Some((2, 11))), Texts::Keep);
    }

    #[test]
    fn a_scene_without_its_movie_or_game_keeps_nothing_to_make() {
        assert_eq!(texts_at_frame(Some((1, 10)), None), Texts::Keep);
    }
}
