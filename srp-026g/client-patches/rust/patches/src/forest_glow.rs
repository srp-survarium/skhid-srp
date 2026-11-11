//! The glows of patch 111 (docs/notes/artefact-skills.md, "Forest Protection"
//! and "Leg quarters"), every game deciding them from what it has (the kits in
//! the static info, its world):
//!
//! - Forest Protection's: while the skill protects a player (its kit's
//!   modifier 41 makes the anomaly hit's factor below 1, and it is inside a
//!   damage zone: the native rule of 0x332810), a grey glow on its arms, the
//!   game effect `forest_protection` (content pack artefact-glow: the
//!   artefacts' stock glow, grey; a composite whose first-person twin has no
//!   point light or haze, as the quarters'); everyone sees it;
//! - the leg quarters: each passive artefact copy that counts for a player
//!   (its own carried copies giving their passive and those Resonance
//!   shares with it, at most 4 of a kind) lights a quarter of its legs in
//!   that kind's colour: `artefact_leg_<kind>_<n>`, n = 1 the left lower leg,
//!   2 the right lower leg, 3 the left thigh, 4 the right one. Each kind
//!   lights its own quarters from the first up; two kinds on one quarter
//!   layer (their sprites add, as the stock glows of two kinds do). Shown
//!   only by a game whose local player's current kit has Knowledge of
//!   Artifacts (every player's quarters, its own too), or that has no local
//!   player (a spectator, a replay).
//!
//! In the simulation, as the artefacts' own glow: at a player's tick each
//! effect is played when the game shows it and none of it plays (so it is in
//! the world's history, and a rollback keeps it); the player's own
//! calculator ends one when the game no longer shows it. Presentation only:
//! nothing a match computes reads an effect. No state of its own but the
//! resources.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::artefact_skills::{Copies, Kind, Skills};
use crate::vostok::effect::{self, KeptEffect};
use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::resources::{self, Arrived, QueryResults, QueryTag};
use crate::vostok::world::{Player, PlayerId};
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread};

/// An instance of each effect for each place of a match (every player can
/// glow at once).
const INSTANCES: u32 = 20;

/// One of the effects the patch plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LegEffect {
    Forest,
    /// The `nth` (1 to 4) quarter of `kind`.
    Quarter { kind: Kind, nth: u8 },
}

/// The effects, in the order they are asked for and kept; [`NAMES`] are
/// their names (gameplay/effects/effect_<name>.options).
const EFFECTS: [LegEffect; 13] = [
    LegEffect::Forest,
    LegEffect::Quarter { kind: Kind::Spring, nth: 1 },
    LegEffect::Quarter { kind: Kind::Spring, nth: 2 },
    LegEffect::Quarter { kind: Kind::Spring, nth: 3 },
    LegEffect::Quarter { kind: Kind::Spring, nth: 4 },
    LegEffect::Quarter { kind: Kind::Lifebone, nth: 1 },
    LegEffect::Quarter { kind: Kind::Lifebone, nth: 2 },
    LegEffect::Quarter { kind: Kind::Lifebone, nth: 3 },
    LegEffect::Quarter { kind: Kind::Lifebone, nth: 4 },
    LegEffect::Quarter { kind: Kind::Onyx, nth: 1 },
    LegEffect::Quarter { kind: Kind::Onyx, nth: 2 },
    LegEffect::Quarter { kind: Kind::Onyx, nth: 3 },
    LegEffect::Quarter { kind: Kind::Onyx, nth: 4 },
];
const NAMES: [&CStr; 13] = [
    c"forest_protection",
    c"artefact_leg_spring_1",
    c"artefact_leg_spring_2",
    c"artefact_leg_spring_3",
    c"artefact_leg_spring_4",
    c"artefact_leg_lifebone_1",
    c"artefact_leg_lifebone_2",
    c"artefact_leg_lifebone_3",
    c"artefact_leg_lifebone_4",
    c"artefact_leg_onyx_1",
    c"artefact_leg_onyx_2",
    c"artefact_leg_onyx_3",
    c"artefact_leg_onyx_4",
];

/// Whether this game shows the leg quarters (the owner, 2026-11-07): only to a local player whose
/// current kit has Artifact Sight (513, its own quarters too), to everyone's game without a local
/// player (a spectator, a replay). Forest Protection's glow is everyone's regardless.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quarters {
    Shown,
    Hidden,
}

impl Quarters {
    /// From the local player's packed artefact skills (modifier 40), none without a local player.
    fn of(viewer: Option<Skills>) -> Quarters {
        match viewer {
            None => Quarters::Shown,
            Some(skills) if skills.sight() => Quarters::Shown,
            Some(_) => Quarters::Hidden,
        }
    }

    /// For the game `player` is in: its local player, read now.
    fn seen_in(player: &Player<'_>) -> Quarters {
        Quarters::of(player.viewer_artefact_skills())
    }
}

/// What a player's legs and hands show in this game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Legs {
    forest: Glow,
    quarters: Quarters,
    copies: Copies,
}

impl LegEffect {
    fn shown(self, legs: Legs) -> bool {
        match self {
            LegEffect::Forest => legs.forest == Glow::On,
            LegEffect::Quarter { kind, nth } => legs.quarters == Quarters::Shown && legs.copies.get(kind) >= nth,
        }
    }
}

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

/// The effect's resource: asked for at the first player tick of a match,
/// kept for the process once it came. `E` the kept resource.
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
    /// Say once that the effects arrived (the line's time against the ask's: how long the glows
    /// were missing at the first match).
    Arrived,
}

impl<E: Copy> Load<E> {
    fn step(self, event: Event<E>) -> (Load<E>, Action<E>) {
        match (self, event) {
            (Load::Unasked, Event::Frame) => (Load::Asked, Action::Ask),
            (Load::Ready(effect), Event::Frame) => (self, Action::Show(effect)),
            (Load::Asked | Load::Missing, Event::Frame) => (self, Action::Nothing),
            (Load::Asked, Event::Arrived(Some(effect))) => (Load::Ready(effect), Action::Arrived),
            (Load::Asked, Event::Arrived(None)) => (Load::Missing, Action::Report),
            // one query: nothing else arrives
            (Load::Unasked | Load::Ready(_) | Load::Missing, Event::Arrived(_)) => (self, Action::Nothing),
        }
    }
}

static LOAD: GameCell<Load<[KeptEffect; 13]>> = GameCell::new(Load::Unasked);

/// Whether `player` shows Forest Protection's glow.
fn glow(player: &Player<'_>) -> Glow {
    Glow::of(player.alive(), player.damage_zones(), player.weapon_damage_in_anomaly())
}

/// Whether `player` shows `effect` (the copies counted only for a quarter).
fn shows(effect: LegEffect, player: &Player<'_>) -> bool {
    match effect {
        LegEffect::Forest => glow(player) == Glow::On,
        LegEffect::Quarter { kind, nth } => {
            Quarters::seen_in(player) == Quarters::Shown && player.passive_copies().get(kind) >= nth
        }
    }
}

/// A match's places.
const PLACES: usize = 20;

/// The player object each place's presenter last came back to: a rollback
/// gives it back again at each tick it simulates again, and its line is said
/// once an object.
static PRESENTERS_BACK: GameCell<[Option<PlayerId>; PLACES]> = GameCell::new([None; PLACES]);

/// Whether the presenter came back to another object than the place's last.
fn presenter_first_back(thread: &Thread, player: &Player<'_>) -> bool {
    let mut back = PRESENTERS_BACK.get(thread);
    let Some(last) = back.get_mut(usize::from(player.place())) else { return true };
    if *last == Some(player.id()) {
        return false;
    }
    *last = Some(player.id());
    PRESENTERS_BACK.set(thread, back);
    true
}

/// A player's simulation tick: each effect it shows played when none of it
/// plays; the effects asked for at the first tick.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(player) = Player::at_tick(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    if player.take_its_presenter() && presenter_first_back(thread, &player) {
        log::line(&[Text("forest_glow: place "), Dec(player.place().into()), Text("'s presenter back on its current kit")]);
    }
    let (load, action) = LOAD.get(thread).step(Event::Frame);
    LOAD.set(thread, load);
    match action {
        Action::Ask => {
            log::line(&[Text("forest_glow: the glows' 13 game effects asked for (none plays until they arrive)")]);
            resources::query_effects(thread, &NAMES, crate::vostok::forest_glow_on_loaded_entry());
        }
        Action::Show(kept) => {
            let legs = Legs { forest: glow(&player), quarters: Quarters::seen_in(&player), copies: player.passive_copies() };
            for (effect, kept) in EFFECTS.into_iter().zip(kept) {
                if effect.shown(legs) && !player.plays_effect(kept) {
                    player.hold_effect(kept);
                }
            }
        }
        Action::Nothing | Action::Report | Action::Arrived => {}
    }
    Flow::Continue
}

/// The effect's query came back (on the thread that asked: the game's).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let thread = results.thread();
    let arrived = match results.arrived(tag) {
        Arrived::Effects(kept) => kept.all(),
        // this callback is only the glow's query's
        Arrived::Bus(_) | Arrived::Track(..) => return,
    };
    let (load, action) = LOAD.get(thread).step(Event::Arrived(arrived));
    LOAD.set(thread, load);
    match action {
        Action::Report => {
            log::line(&[Text("forest_glow: a game effect of the glows did not load; no glow this session")]);
        }
        Action::Arrived => log::line(&[Text("forest_glow: the glows' 13 game effects arrived")]),
        _ => {}
    }
}

/// The game effect cook: an instance a place for each of the effects.
pub fn on_capacity(ctx: &mut Context<'_>) -> Flow<NoExit> {
    for name in NAMES {
        effect::effect_capacity(ctx, name, INSTANCES);
    }
    Flow::Continue
}

/// The default clock: an effect ends when its player no longer shows it.
pub fn on_clock(ctx: &mut Context<'_>) -> Flow<crate::vostok::ForestGlowOnClockExit> {
    let Load::Ready(kept) = LOAD.get(ctx.thread()) else { return Flow::Continue };
    if effect::kept_clock(ctx, &kept, |index, player| shows(EFFECTS[index], player)) {
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
    fn names_follow_the_effects() {
        for (effect, name) in EFFECTS.into_iter().zip(NAMES) {
            let name = name.to_str().unwrap();
            match effect {
                LegEffect::Forest => assert_eq!(name, "forest_protection"),
                LegEffect::Quarter { kind, nth } => {
                    let kind = match kind {
                        Kind::Spring => "spring",
                        Kind::Lifebone => "lifebone",
                        Kind::Onyx => "onyx",
                    };
                    assert_eq!(name, std::format!("artefact_leg_{kind}_{nth}"));
                }
            }
        }
    }

    #[test]
    fn copies_light_their_kinds_quarters_from_the_first() {
        let copies = Copies::of(|kind| match kind {
            Kind::Spring => (1, 1),
            Kind::Lifebone => (0, 0),
            Kind::Onyx => (4, 0),
        });
        let legs = Legs { forest: Glow::Off, quarters: Quarters::Shown, copies };
        let lit: std::vec::Vec<_> = EFFECTS.into_iter().filter(|effect| effect.shown(legs)).collect();
        assert_eq!(lit, [
            LegEffect::Quarter { kind: Kind::Spring, nth: 1 },
            LegEffect::Quarter { kind: Kind::Spring, nth: 2 },
            LegEffect::Quarter { kind: Kind::Onyx, nth: 1 },
            LegEffect::Quarter { kind: Kind::Onyx, nth: 2 },
            LegEffect::Quarter { kind: Kind::Onyx, nth: 3 },
            LegEffect::Quarter { kind: Kind::Onyx, nth: 4 },
        ]);
        let none = Legs { forest: Glow::On, quarters: Quarters::Shown, copies: Copies::default() };
        let hidden = Legs { forest: Glow::On, quarters: Quarters::Hidden, copies };
        assert_eq!(EFFECTS.into_iter().filter(|effect| effect.shown(hidden)).collect::<std::vec::Vec<_>>(), [LegEffect::Forest]);
        assert_eq!(EFFECTS.into_iter().filter(|effect| effect.shown(none)).collect::<std::vec::Vec<_>>(), [LegEffect::Forest]);
    }

    #[test]
    fn quarters_show_to_artifact_sight_or_without_a_local_player() {
        assert_eq!(Quarters::of(None), Quarters::Shown);
        assert_eq!(Quarters::of(Skills::parse(4096.0)), Quarters::Shown);
        assert_eq!(Quarters::of(Skills::parse(4096.0 + 508.0)), Quarters::Shown);
        assert_eq!(Quarters::of(Skills::parse(508.0)), Quarters::Hidden);
        assert_eq!(Quarters::of(Some(Skills::default())), Quarters::Hidden);
    }

    #[test]
    fn the_effect_is_asked_once_and_kept() {
        let (load, action) = Load::<u32>::Unasked.step(Event::Frame);
        assert_eq!((load, action), (Load::Asked, Action::Ask));
        assert_eq!(load.step(Event::Frame), (Load::Asked, Action::Nothing));
        let (load, action) = load.step(Event::Arrived(Some(7)));
        assert_eq!((load, action), (Load::Ready(7), Action::Arrived));
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
