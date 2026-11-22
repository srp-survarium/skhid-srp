//! Patch 180: Sharp Ears (412, technical-skills.md). A local player whose current kit has the skill
//! hears an enemy's grenade pin pull from about 25 m: at the stock pull's sound (every game plays it
//! as a point sound, silent from 5 m), this game also plays a long-range copy at the thrower,
//! `weapons/grenade_throw/grenade_pin_out_far` (content edit technical-sharp-ears-1: the same sample,
//! spl_preset material_hit, silent from 35 m). Presentation only, on the skilled player's own game:
//! nothing the simulation computes changes.
//!
//! A weapon's sounds are played where weapon::draw runs its delayed callbacks (0x244E28): mt_draw's
//! tasks, any of their threads. So the copy is kept in a [`Lock`], and asked for on the game's
//! thread (its query calls back on the thread that asked).
#![forbid(unsafe_code)]

use crate::vostok::log::{self, Piece::Text};
use crate::vostok::resources::{self, Arrived, QueryResults, QueryTag, Serial};
use crate::vostok::sound::Emitter;
use crate::vostok::technical_sharp_ears::{self, EnemyPin};
use crate::vostok::{Context, Flow, Lock, NoExit, Thread};

/// The copy (sounds/single/weapons/grenade_throw/grenade_pin_out_far.*).
const FAR_PIN: &core::ffi::CStr = c"weapons/grenade_throw/grenade_pin_out_far";

/// The copy's sound: asked for at the first match frame, kept for the process once it came.
/// `E` the loaded sound.
#[derive(Debug, PartialEq, Eq)]
enum Load<E> {
    Unasked,
    Asked,
    Ready(E),
    /// The query failed (the file is not in the game's data): no copy this session.
    Missing,
}

#[derive(Debug, PartialEq, Eq)]
enum Event<E> {
    /// A match frame, on the game's thread.
    Frame,
    /// An enemy's pin pull, the local player with Sharp Ears.
    EnemyPin,
    Arrived(Option<E>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Nothing,
    Ask,
    /// Play the copy (the state is `Ready`).
    Play,
    /// Say once that the copy did not load.
    Report,
}

impl<E> Load<E> {
    fn step(self, event: Event<E>) -> (Load<E>, Action) {
        match (self, event) {
            (Load::Unasked, Event::Frame) => (Load::Asked, Action::Ask),
            (load @ (Load::Asked | Load::Ready(_) | Load::Missing), Event::Frame) => (load, Action::Nothing),
            (Load::Ready(sound), Event::EnemyPin) => (Load::Ready(sound), Action::Play),
            // not there yet, or never: the stock pull alone
            (load @ (Load::Unasked | Load::Asked | Load::Missing), Event::EnemyPin) => (load, Action::Nothing),
            (Load::Asked, Event::Arrived(Some(sound))) => (Load::Ready(sound), Action::Nothing),
            (Load::Asked, Event::Arrived(None)) => (Load::Missing, Action::Report),
            // one query: nothing else arrives
            (load @ (Load::Unasked | Load::Ready(_) | Load::Missing), Event::Arrived(_)) => (load, Action::Nothing),
        }
    }
}

static FAR: Lock<Load<Emitter>> = Lock::new(Load::Unasked);

/// Steps the load; `play` gets the copy when the step says to play it (inside the lock: a play makes
/// and starts a proxy and calls nothing of this patch). A query is asked after, outside it.
fn step(thread: &Thread, event: Event<Emitter>, play: impl FnOnce(&Emitter)) -> Action {
    FAR.with(thread, |load| {
        let (next, action) = core::mem::replace(load, Load::Unasked).step(event);
        *load = next;
        if let (Action::Play, Load::Ready(sound)) = (action, &*load) {
            play(sound);
        }
        action
    })
}

/// A match frame after the simulation's move (the game's thread): the copy asked for at the first.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    // Only a game whose local player has the skill asks for the copy.
    if !technical_sharp_ears::local_listens(ctx) {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    if step(thread, Event::Frame, |_| {}) == Action::Ask {
        resources::query_track(thread, FAR_PIN, Serial::FIRST, crate::vostok::technical_sharp_ears_on_loaded_entry());
    }
    Flow::Continue
}

/// A weapon sound was made (after Concentration's tag, order 1): an enemy's pull also plays the copy.
pub fn on_weapon_sound(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(pin) = EnemyPin::at(ctx) {
        step(ctx.thread(), Event::EnemyPin, |sound| pin.play(sound));
    }
    Flow::Continue
}

/// The copy's query came back (on the thread that asked: the game's).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let arrived = match results.arrived(tag) {
        Arrived::Track(_, sound) => sound,
        // this callback is only the copy's query's
        Arrived::Bus(_) | Arrived::Effects(_) | Arrived::Particles(_) | Arrived::Assets(_) => return,
    };
    if step(results.thread(), Event::Arrived(arrived), |_| {}) == Action::Report {
        log::line(&[Text("technical_sharp_ears: the far pin pull did not load; Sharp Ears is silent this session")]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_once_and_plays_only_when_ready() {
        // a pull before the first frame asks nothing and is not heard from afar
        let (load, action) = Load::<u32>::Unasked.step(Event::EnemyPin);
        assert_eq!((&load, action), (&Load::Unasked, Action::Nothing));
        let (load, action) = load.step(Event::Frame);
        assert_eq!((&load, action), (&Load::Asked, Action::Ask));
        let (load, action) = load.step(Event::Frame);
        assert_eq!((&load, action), (&Load::Asked, Action::Nothing));
        let (load, action) = load.step(Event::EnemyPin);
        assert_eq!((&load, action), (&Load::Asked, Action::Nothing));
        let (load, action) = load.step(Event::Arrived(Some(7)));
        assert_eq!((&load, action), (&Load::Ready(7), Action::Nothing));
        let (load, action) = load.step(Event::Frame);
        assert_eq!((&load, action), (&Load::Ready(7), Action::Nothing));
        let (load, action) = load.step(Event::EnemyPin);
        assert_eq!((&load, action), (&Load::Ready(7), Action::Play));
        // a stray result changes nothing
        let (load, action) = load.step(Event::Arrived(Some(8)));
        assert_eq!((&load, action), (&Load::Ready(7), Action::Nothing));
    }

    #[test]
    fn a_missing_copy_is_reported_once_and_never_played() {
        let (load, _) = Load::<u32>::Unasked.step(Event::Frame);
        let (load, action) = load.step(Event::Arrived(None));
        assert_eq!((&load, action), (&Load::Missing, Action::Report));
        for event in [Event::Frame, Event::EnemyPin, Event::Arrived(None)] {
            let (next, action) = Load::<u32>::Missing.step(event);
            assert_eq!((next, action), (Load::Missing, Action::Nothing));
        }
        assert_eq!(load, Load::Missing);
    }
}
