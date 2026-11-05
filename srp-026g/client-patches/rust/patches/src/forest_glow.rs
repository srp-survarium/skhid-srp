//! Forest Protection's glow (patch 110; docs/notes/artefact-skills.md,
//! "Forest Protection"): while the skill protects a player (its kit's
//! modifier 41 makes the anomaly hit's factor below 1, and it is inside a
//! damage zone: the native rule of 0x332810), a grey glow plays on its legs,
//! the game effect `forest_protection` (content pack artefact-glow,
//! effect_forest_protection: the artefacts' stock glow, grey, its legs only,
//! both views). Every game shows it on every player, from what it has: the
//! kit in the static info and the zone count of its world.
//!
//! Presentation only: nothing a match computes reads an effect. No state of
//! its own but the resource: whether a player glows is read from its world
//! every frame, and whether it plays the glow from its effect list.

#![forbid(unsafe_code)]

use crate::vostok::effect::{self, Clock, KeptEffect};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::resources::{self, Arrived, QueryResults, QueryTag};
use crate::vostok::world::GameWorld;
use crate::vostok::{Context, Flow, GameCell, NoExit};

/// The effect's name: gameplay/effects/effect_forest_protection.options.
const EFFECT: &core::ffi::CStr = c"forest_protection";
/// An instance for each place of a match (every player can glow at once).
const INSTANCES: u32 = 20;

/// Whether a player shows the glow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    On,
    Off,
}

impl Glow {
    /// On while Forest Protection protects it: alive, inside a damage zone
    /// (`zones` above 0), and a hit's factor there (`factor`, the native
    /// getter of modifier 41) below 1 (a rank or more).
    pub fn of(alive: bool, zones: u8, factor: f32) -> Glow {
        if alive && zones > 0 && factor < 1.0 { Glow::On } else { Glow::Off }
    }
}

/// The effect's resource: asked for at the first frame of a match (a world
/// with its network client), kept for the process once it came. `E` the
/// kept resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Load<E> {
    Unasked,
    Asked,
    Ready(E),
    /// The query failed (the effect is not in the game's data): no glow
    /// this session.
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event<E> {
    Frame,
    Arrived(Option<E>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action<E> {
    Nothing,
    Ask,
    /// Each player's glow, on or off, with this resource.
    Show(E),
    /// Say once that the effect did not load.
    Report,
}

impl<E: Copy> Load<E> {
    fn step(self, event: Event<E>) -> (Load<E>, Action<E>) {
        match (self, event) {
            (Load::Unasked, Event::Frame) => (Load::Asked, Action::Ask),
            (Load::Ready(effect), Event::Frame) => (self, Action::Show(effect)),
            (Load::Asked | Load::Missing, Event::Frame) => (self, Action::Nothing),
            (Load::Asked, Event::Arrived(Some(effect))) => (Load::Ready(effect), Action::Nothing),
            (Load::Asked, Event::Arrived(None)) => (Load::Missing, Action::Report),
            // one query: nothing else arrives
            (Load::Unasked | Load::Ready(_) | Load::Missing, Event::Arrived(_)) => (self, Action::Nothing),
        }
    }
}

static LOAD: GameCell<Load<KeptEffect>> = GameCell::new(Load::Unasked);

/// After the simulation's move: each player's glow on or off.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(world) = GameWorld::at_after_move(ctx) else { return Flow::Continue };
    let Some(client) = world.network_client() else { return Flow::Continue };
    let thread = ctx.thread();
    let (load, action) = LOAD.get(thread).step(Event::Frame);
    LOAD.set(thread, load);
    match action {
        Action::Ask => resources::query_effect(thread, EFFECT, crate::vostok::forest_glow_on_loaded_entry()),
        Action::Show(kept) => {
            for place in 0..client.places() {
                let Some(player) = client.player(place) else { continue };
                let glow = Glow::of(player.alive(), player.damage_zones(), player.weapon_damage_in_anomaly());
                match (glow, player.held_effect(kept)) {
                    (Glow::On, None) => player.hold_effect(kept),
                    (Glow::Off, Some(held)) => held.remove(),
                    (Glow::On, Some(_)) | (Glow::Off, None) => {}
                }
            }
        }
        Action::Nothing | Action::Report => {}
    }
    Flow::Continue
}

/// The effect's query came back (on the thread that asked: the game's).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let thread = results.thread();
    let arrived = match results.arrived(tag) {
        Arrived::Effect(kept) => kept,
        // this callback is only the glow's query's
        Arrived::Bus(_) | Arrived::Track(..) => return,
    };
    let (load, action) = LOAD.get(thread).step(Event::Arrived(arrived));
    LOAD.set(thread, load);
    if action == Action::Report {
        log::line(&[Text("forest_glow: the game effect forest_protection did not load; no glow this session")]);
    }
}

/// The game effect cook: an instance a place for the glow.
pub fn on_capacity(ctx: &mut Context<'_>) -> Flow<NoExit> {
    effect::effect_capacity(ctx, EFFECT, INSTANCES);
    Flow::Continue
}

/// The default clock: the glow's held clock, which never ends on its own.
pub fn on_clock(ctx: &mut Context<'_>) -> Flow<crate::vostok::ForestGlowOnClockExit> {
    if effect::effect_clock(ctx, Clock::Held) {
        Flow::Exit(crate::vostok::ForestGlowOnClockExit::Returned)
    } else {
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glows_while_protected_in_a_zone() {
        // ranks 0..4: the factor 1 - 0.05 a rank
        for (factor, protected) in [(1.0, false), (0.95, true), (0.8, true)] {
            for zones in [0u8, 1, 2] {
                for alive in [false, true] {
                    let on = alive && zones > 0 && protected;
                    assert_eq!(Glow::of(alive, zones, factor), if on { Glow::On } else { Glow::Off });
                }
            }
        }
        // a sum past -1: the getter's max(0, ...) is 0, still protected
        assert_eq!(Glow::of(true, 1, 0.0), Glow::On);
    }

    #[test]
    fn the_effect_is_asked_once_and_kept() {
        let (load, action) = Load::<u32>::Unasked.step(Event::Frame);
        assert_eq!((load, action), (Load::Asked, Action::Ask));
        assert_eq!(load.step(Event::Frame), (Load::Asked, Action::Nothing));
        let (load, action) = load.step(Event::Arrived(Some(7)));
        assert_eq!((load, action), (Load::Ready(7), Action::Nothing));
        assert_eq!(load.step(Event::Frame), (Load::Ready(7), Action::Show(7)));
        assert_eq!(load.step(Event::Arrived(Some(8))), (Load::Ready(7), Action::Nothing));
    }

    #[test]
    fn a_missing_effect_is_said_once() {
        let (load, action) = Load::<u32>::Asked.step(Event::Arrived(None));
        assert_eq!((load, action), (Load::Missing, Action::Report));
        assert_eq!(load.step(Event::Frame), (Load::Missing, Action::Nothing));
        assert_eq!(load.step(Event::Arrived(None)), (Load::Missing, Action::Nothing));
    }
}
