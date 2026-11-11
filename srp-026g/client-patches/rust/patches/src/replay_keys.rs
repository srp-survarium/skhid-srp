//! Patch 140: a replay's keys (docs/notes/replay-keys.md).
//!
//! The replay player movie gets no key from the game (its controls are the
//! mouse's). This patch tells it the keys that seek and pause, at the game
//! world's own key handler (on_keyboard_action's start, `<patch>.toml`): a
//! key that goes down while the game plays a replay is named to the movie
//! (`root.srp_key`), which does with it what its own buttons do. The key
//! goes on to the game as before: nothing is taken.
//!
//! The movie is told at the frame's end (game_world::tick after the
//! simulation's move), not at the key: its seek runs at once (0x1EE2E0, from
//! the movie's update_position), and the frame's replay reading (0x1EDEF0)
//! still has the clock from before the keys. A seek back at the key was
//! followed by a reading on to the old time: the world 5 s ahead of the
//! clock, still until the clock came to it ("cannot find older item for
//! time", the owner 2026-11-11: "everything freezes, time goes, and then it
//! snaps").
//!
//! It also tells the movie which places are in the match (`root.srp_places`,
//! a bool a place), whenever that changes: the game follows a place only
//! from the click on it, and only one in the match then, so a followed
//! player who left and came back (the same place again, or a new one) stayed
//! unfollowed. The movie follows the place again as it comes in.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::world::{GameWorld, Places};
use crate::vostok::{Context, Dik, Flow, GameCell, GameWorldKey, KeyKind, NoExit};

/// What a key asks of the replay player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayKey {
    /// A few seconds back or on.
    Back,
    Forward,
    /// The same on A and D, the free view's own keys: the movie seeks only
    /// while a player is followed.
    StrafeBack,
    StrafeForward,
    /// One tick of the game back or on.
    TickBack,
    TickForward,
    /// Play or pause.
    Pause,
    /// The game's own view keys, which the movie has to know of to tell the
    /// free view: a player followed (F1, F3), the free view (F2).
    Followed,
    FreeView,
    /// The game's own next and previous player (F6, F5): the movie's
    /// followed row is no longer the one watched.
    Another,
}

impl ReplayKey {
    fn of(key: Dik) -> Option<ReplayKey> {
        Some(match key {
            Dik::LEFT => ReplayKey::Back,
            Dik::RIGHT => ReplayKey::Forward,
            Dik::A => ReplayKey::StrafeBack,
            Dik::D => ReplayKey::StrafeForward,
            Dik::COMMA => ReplayKey::TickBack,
            Dik::PERIOD => ReplayKey::TickForward,
            Dik::SPACE => ReplayKey::Pause,
            Dik::F1 | Dik::F3 => ReplayKey::Followed,
            Dik::F2 => ReplayKey::FreeView,
            Dik::F5 | Dik::F6 => ReplayKey::Another,
            _ => return None,
        })
    }

    /// Its name in the movie (`MainTimeline.srp_key`).
    fn name(self) -> &'static CStr {
        match self {
            ReplayKey::Back => c"back",
            ReplayKey::Forward => c"forward",
            ReplayKey::StrafeBack => c"strafe_back",
            ReplayKey::StrafeForward => c"strafe_forward",
            ReplayKey::TickBack => c"tick_back",
            ReplayKey::TickForward => c"tick_forward",
            ReplayKey::Pause => c"pause",
            ReplayKey::Followed => c"followed",
            ReplayKey::FreeView => c"free_view",
            ReplayKey::Another => c"another",
        }
    }
}

/// The key a key event asks the replay player for: one going down only (a
/// held key says nothing more, as a video player's).
fn asked(key: Dik, kind: KeyKind) -> Option<ReplayKey> {
    (kind == KeyKind::Down).then(|| ReplayKey::of(key)).flatten()
}

/// The key asked of the replay player and not told yet: the last one of a
/// frame (a second in the same frame takes its place).
static ASKED: GameCell<Option<ReplayKey>> = GameCell::new(None);

/// on_keyboard_action's start (0x25CD00): every key the game world gets. A
/// replay's key waits for the frame's end.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(key) = GameWorldKey::at_start(ctx) else { return Flow::Continue };
    let Some(asked) = asked(key.key(), key.kind()) else { return Flow::Continue };
    if key.replay_player().is_some() {
        ASKED.set(key.thread(), Some(asked));
    }
    Flow::Continue
}

/// What a replay player movie was last told of the places: which movie (a new
/// match's is another) and the places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Told {
    movie: usize,
    places: Places,
}

static TOLD: GameCell<Option<Told>> = GameCell::new(None);

/// game_world::tick after the simulation's move (0x1F7C3C), every frame, a
/// paused one too: a replay's movie is told the key asked in this frame, and
/// the places in the match when they are others than it was last told (an
/// entry, a leave, a seek), again until it takes it.
pub fn on_move(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let asked = ASKED.get(thread);
    ASKED.set(thread, None);
    let Some(world) = GameWorld::at_after_move(ctx) else { return Flow::Continue };
    let Some(movie) = world.replay_player() else { return Flow::Continue };
    if let Some(asked) = asked {
        movie.invoke_text(c"root.srp_key", asked.name());
    }
    let Some(places) = world.places_in_match() else { return Flow::Continue };
    let now = Some(Told { movie: movie.id(), places });
    if TOLD.get(thread) != now && movie.invoke_bools(c"root.srp_places", places.each()) {
        TOLD.set(thread, now);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_asks_once_when_it_goes_down() {
        assert_eq!(asked(Dik::LEFT, KeyKind::Down), Some(ReplayKey::Back));
        assert_eq!(asked(Dik::LEFT, KeyKind::Held), None);
        assert_eq!(asked(Dik::LEFT, KeyKind::Up), None);
        assert_eq!(asked(Dik::TAB, KeyKind::Down), None);
    }

    #[test]
    fn each_key_has_its_own_name() {
        let keys =
            [Dik::LEFT, Dik::RIGHT, Dik::A, Dik::D, Dik::COMMA, Dik::PERIOD, Dik::SPACE, Dik::F2, Dik::F1, Dik::F5];
        let mut names: Vec<_> = keys.iter().map(|&key| ReplayKey::of(key).unwrap().name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), keys.len());
        assert_eq!(ReplayKey::of(Dik::F3), ReplayKey::of(Dik::F1));
        assert_eq!(ReplayKey::of(Dik::F6), ReplayKey::of(Dik::F5));
    }
}
