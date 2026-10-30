//! Patch 82: the names on the Tab map while Alt is held (the owner,
//! 2026-10-30; the rules: docs/notes/tab-map-names.md). The hud movie shows
//! them (`HUD.show_map_names`); this tells it whether Alt is held.
//!
//! The hook is `map_names.toml`'s, a `lohook` at `keyboard.key_state`
//! (game_world::on_keyboard_action 0x25CD1C, before it asks the keyboard for
//! Tab): the game world is given every key, a held one every tick, so every
//! change of an Alt key comes with a key of its own. Either Alt held is "shown";
//! the hud movie is told when that or the movie changes, and again at the next
//! key while the movie refuses it (no root yet).
//!
//! Nothing a match computes: the interface only.

#![forbid(unsafe_code)]

use crate::vostok::{Context, Dik, Flow, GameCell, GameWorldKeys, NoExit};

/// What the hud movie was last told: which movie, and whether the names show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Told {
    movie: usize,
    shown: bool,
}

static TOLD: GameCell<Option<Told>> = GameCell::new(None);

/// The names show while either Alt key is held.
fn shown(down: impl Fn(Dik) -> bool) -> bool {
    down(Dik::LEFT_ALT) || down(Dik::RIGHT_ALT)
}

/// on_keyboard_action with the input world's keyboard (0x25CD1C).
pub fn on_key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(keys) = GameWorldKeys::at_key_state(ctx) else { return Flow::Continue };
    let Some(movie) = keys.hud_movie() else { return Flow::Continue };
    let now = Told { movie: movie.id(), shown: shown(|key| keys.down(key)) };
    let thread = ctx.thread();
    if TOLD.get(thread) != Some(now) && movie.invoke_bool(c"root.show_map_names", now.shown) {
        TOLD.set(thread, Some(now));
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_alt_shows_the_names() {
        assert!(!shown(|_| false));
        assert!(shown(|key| key == Dik::LEFT_ALT));
        assert!(shown(|key| key == Dik::RIGHT_ALT));
        assert!(shown(|_| true));
    }
}
