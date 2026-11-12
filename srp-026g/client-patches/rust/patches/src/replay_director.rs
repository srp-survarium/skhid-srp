//! Patch 143: a replay plays one player's moment, unattended
//! (docs/notes/replay-director.md).
//!
//! Started with a replay and the environment variable
//!
//! ```text
//! SKHID_REPLAY_DIRECTOR=<place>,<from ms>,<to ms> survarium_skhid.exe -match_replay=<file>
//! ```
//!
//! (the times the match's, as the world and the recordings count them),
//! the game seeks to `from` at the replay's first frame, follows the place's
//! player in the first person (again after a death, a seek, or a leave and a
//! return under another place of the same name), hides the replay's controls
//! and its cursor, and quits at `to` (or when the replay comes to its end
//! first). Each step is a line of srp_patches.log, which a capture times.
//! An environment variable, not command-line keys: the engine refuses a key
//! it has not registered (a message box, "has not been registered").
//!
//! Without the variable nothing happens, and a live match has no replay
//! (`GameWorld::replay`): its frames are untouched.

#![forbid(unsafe_code)]

use crate::vostok::log::{Piece::*, line};
use crate::vostok::renderer::{environment_set, get_environment};
use crate::vostok::replay::Replay;
use crate::vostok::types::{GameTime, Place};
use crate::vostok::world::GameWorld;
use crate::vostok::{Context, Flow, GameCell, NoExit};

/// What the variable asks: whose view, from when to when (the match's ms,
/// `from` before `to`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Direction {
    place: Place,
    from: GameTime,
    to: GameTime,
}

/// The variable, read once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Options {
    /// Not set: a game like any other.
    None,
    Direction(Direction),
    /// Set to something else than a direction: said in the log, and nothing done.
    NotUnderstood,
}

/// `<place>,<from ms>,<to ms>`.
const VARIABLE: &core::ffi::CStr = c"SKHID_REPLAY_DIRECTOR";

/// A decimal number of at most nine digits.
fn number(text: &[u8]) -> Option<u32> {
    if text.is_empty() || text.len() > 9 || !text.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(text.iter().fold(0, |n, digit| n * 10 + u32::from(digit - b'0')))
}

impl Options {
    /// The direction of the variable's value (empty: not set).
    fn parse(value: &[u8]) -> Options {
        if value.is_empty() {
            return Options::None;
        }
        let direction = || {
            let mut parts = value.split(|&b| b == b',');
            let place = Place::parse(parts.next()?)?;
            let (from, to) = (GameTime(number(parts.next()?)?), GameTime(number(parts.next()?)?));
            (parts.next().is_none() && from.0 < to.0).then_some(Direction { place, from, to })
        };
        direction().map_or(Options::NotUnderstood, Options::Direction)
    }
}

/// Where a directed replay is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// The variable not read yet: the first frame of any match reads it.
    Unread,
    /// Nothing asked (or not understood): nothing, ever.
    Off,
    /// Asked; the replay's first frame not come yet.
    Waiting(Direction),
    /// Seeked to `from`, the player followed: the clock not at `from` yet.
    /// After a seek the game's clock steps back a little (1.5 s seen) and
    /// the world, read to `from`, waits for it.
    Seeking(Direction),
    /// The clock from `from` on: the player followed until `to`.
    Watching(Direction),
    /// The game asked to quit.
    Quit,
}

/// What a replay's frame asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Nothing,
    /// The replay's first frame: seek there (None: `from` is not after the clock).
    Start(Option<GameTime>),
    /// Follow the player, controls hidden.
    Follow(Direction),
    /// The clock at `from`: the window begins (and the player is followed).
    Begin(Direction),
    /// `to` reached, `from` past the replay's end, or the clock back before
    /// `from`: the game sends a replay at its end back to its start.
    Quit,
}

impl Stage {
    /// The stage after a replay's frame with its clock and the replay's
    /// last time, and what to do.
    fn next(self, clock: GameTime, end: GameTime) -> (Stage, Act) {
        let within = |d: Direction| d.from.not_after(clock) && !d.to.not_after(clock);
        match self {
            Stage::Waiting(d) if !d.from.not_after(end) => (Stage::Quit, Act::Quit),
            Stage::Waiting(d) => (Stage::Seeking(d), Act::Start((!d.from.not_after(clock)).then_some(d.from))),
            Stage::Seeking(d) if within(d) => (Stage::Watching(d), Act::Begin(d)),
            Stage::Seeking(d) => (self, Act::Follow(d)),
            Stage::Watching(d) if within(d) => (self, Act::Follow(d)),
            Stage::Watching(_) => (Stage::Quit, Act::Quit),
            Stage::Unread | Stage::Off | Stage::Quit => (self, Act::Nothing),
        }
    }
}

/// The place to follow now: the first of the player's places that is in the
/// match, when the camera is not on its player already.
fn to_follow<P: PartialEq>(places: impl Iterator<Item = (Place, Option<P>)>, followed: Option<P>) -> Option<Place> {
    let (place, player) = places.filter_map(|(place, player)| Some((place, player?))).next()?;
    (followed != Some(player)).then_some(place)
}

static STAGE: GameCell<Stage> = GameCell::new(Stage::Unread);

/// The stage SKHID_REPLAY_DIRECTOR gives, said in the log.
fn read_options(ctx: &Context<'_>) -> Stage {
    let thread = ctx.thread();
    let mut room = [0u8; 64];
    let len = get_environment(thread, VARIABLE, &mut room);
    let options = match (len, environment_set(thread, VARIABLE)) {
        (0, true) => Options::NotUnderstood,
        _ => Options::parse(&room[..len]),
    };
    match options {
        Options::None => Stage::Off,
        Options::Direction(direction) => {
            line(&[Text("replay_director: follow place "), Dec(u32::from(direction.place.get())), Text(" from "),
                   Dec(direction.from.0), Text(" to "), Dec(direction.to.0)]);
            Stage::Waiting(direction)
        }
        Options::NotUnderstood => {
            line(&[Text("replay_director: SKHID_REPLAY_DIRECTOR not understood (<place 0-19>,<from ms>,<to ms>, "),
                   Text("from before to): nothing done")]);
            Stage::Off
        }
    }
}

/// Follows the direction's player (by its name, under whichever place it is
/// in now) and keeps the controls hidden.
fn watch(replay: &Replay<'_>, direction: Direction) {
    replay.hide_controls();
    let places = Place::all()
        .filter(|&place| replay.same_player(direction.place, place))
        .map(|place| (place, replay.player(place)));
    if let Some(place) = to_follow(places, replay.followed()) {
        replay.follow_first_person(place);
        line(&[Text("replay_director: follow place "), Dec(u32::from(place.get())), Text(" at "), Dec(replay.clock().0)]);
    }
}

/// game_world::tick after the simulation's move (0x1F7C3C), every frame: a
/// directed replay's steps.
pub fn on_move(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let stage = match STAGE.get(thread) {
        Stage::Unread => read_options(ctx),
        stage => stage,
    };
    STAGE.set(thread, stage);
    if matches!(stage, Stage::Off | Stage::Quit) {
        return Flow::Continue;
    }
    let Some(world) = GameWorld::at_after_move(ctx) else { return Flow::Continue };
    let Some(replay) = world.replay() else { return Flow::Continue };
    let (start, end) = replay.span();
    let (stage, act) = stage.next(replay.clock(), end);
    STAGE.set(thread, stage);
    match act {
        Act::Nothing => {}
        Act::Start(seek) => {
            if let Some(to) = seek {
                replay.seek(to);
            }
            replay.hide_controls();
            line(&[Text("replay_director: seeked to "), Dec(replay.clock().0), Text(" (replay "), Dec(start.0),
                   Text(" to "), Dec(end.0), Text(")")]);
        }
        Act::Follow(direction) => watch(&replay, direction),
        Act::Begin(direction) => {
            watch(&replay, direction);
            line(&[Text("replay_director: watching from "), Dec(replay.clock().0)]);
        }
        Act::Quit => {
            line(&[Text("replay_director: quit at "), Dec(replay.clock().0)]);
            replay.quit();
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direction(place: u32, from: u32, to: u32) -> Direction {
        Direction { place: Place::new(place).unwrap(), from: GameTime(from), to: GameTime(to) }
    }

    #[test]
    fn the_variable_is_a_place_and_two_times() {
        assert_eq!(Options::parse(b""), Options::None);
        assert_eq!(Options::parse(b"7,57000,122000"), Options::Direction(direction(7, 57_000, 122_000)));
        assert_eq!(Options::parse(b"19,0,1"), Options::Direction(direction(19, 0, 1)));
        for bad in [&b"7"[..], b"7,1", b"20,1,2", b"7,2,2", b"7,3,2", b"7,-1,2", b"7,1,1234567890", b"7,1,2,3",
                    b"7 ,1,2", b"a,1,2"] {
            assert_eq!(Options::parse(bad), Options::NotUnderstood, "{}", core::str::from_utf8(bad).unwrap());
        }
    }

    #[test]
    fn a_replay_seeks_once_watches_then_quits() {
        let (d, end) = (direction(3, 57_000, 122_000), GameTime(492_750));
        let (stage, act) = Stage::Waiting(d).next(GameTime(100), end);
        assert_eq!((stage, act), (Stage::Seeking(d), Act::Start(Some(GameTime(57_000)))));
        // The clock steps back after the seek: followed, the window not begun.
        assert_eq!(stage.next(GameTime(55_530), end), (Stage::Seeking(d), Act::Follow(d)));
        let (stage, act) = stage.next(GameTime(57_010), end);
        assert_eq!((stage, act), (Stage::Watching(d), Act::Begin(d)));
        assert_eq!(stage.next(GameTime(121_999), end), (Stage::Watching(d), Act::Follow(d)));
        assert_eq!(stage.next(GameTime(122_000), end), (Stage::Quit, Act::Quit));
        // At its end the replay goes back to its start: out of the window.
        assert_eq!(stage.next(GameTime(50), end), (Stage::Quit, Act::Quit));
        assert_eq!(Stage::Quit.next(GameTime(60_000), end), (Stage::Quit, Act::Nothing));
        assert_eq!(Stage::Off.next(GameTime(60_000), end), (Stage::Off, Act::Nothing));
    }

    #[test]
    fn a_window_from_the_start_does_not_seek() {
        let d = direction(0, 0, 5_000);
        assert_eq!(Stage::Waiting(d).next(GameTime(50), GameTime(90_000)), (Stage::Seeking(d), Act::Start(None)));
        assert_eq!(Stage::Seeking(d).next(GameTime(50), GameTime(90_000)), (Stage::Watching(d), Act::Begin(d)));
    }

    #[test]
    fn a_window_past_the_replay_quits_at_once() {
        let d = direction(0, 95_000, 99_000);
        assert_eq!(Stage::Waiting(d).next(GameTime(50), GameTime(90_000)), (Stage::Quit, Act::Quit));
    }

    #[test]
    fn the_camera_goes_to_the_place_in_the_match() {
        let (a, b) = (Place::new(2).unwrap(), Place::new(9).unwrap());
        let (p, q) = (1, 2);
        assert_eq!(to_follow([(a, Some(p))].into_iter(), None), Some(a));
        assert_eq!(to_follow([(a, Some(p))].into_iter(), Some(p)), None);
        assert_eq!(to_follow([(a, Some(p))].into_iter(), Some(q)), Some(a));
        // Left, and back under another place.
        assert_eq!(to_follow([(a, None), (b, Some(q))].into_iter(), Some(p)), Some(b));
        // Not in the match: nothing to follow.
        assert_eq!(to_follow([(a, None), (b, None)].into_iter(), Some(p)), None);
    }
}
