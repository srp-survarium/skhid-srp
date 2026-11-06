//! Keeper of the Artifacts (patch 109, docs/notes/artefact-skills.md "Keeper of the
//! Artifacts"): a player with the skill starts the match with an artefact of the match's kind in
//! its first container, whatever the map's anomalies hold.
//!
//! - The kind: srp draws it once a match and writes its draw into every kit (modifier 40 bits
//!   9..11; artefact_carry::keeper, the table srp draws from).
//! - The artefact: a dormant one of that kind from the map's hidden Keeper pool (content edit
//!   artefact-keeper-1.json: an anomaly with no container, 20 of each kind), the lowest id, moved
//!   into the container by the game's own take_artefact. The pool's anomaly cooks, ticks and
//!   saves it as any anomaly does its own, so late join, resync and replay load it natively.
//! - When: the respawn rule's spawn, after the player is in the world, in every game and in
//!   every replay of the tick: with the skill, in the place's first life (no death or suicide
//!   counted), into an empty first container. A kit switch before the start unloads the gift
//!   back into the pool (dormant again), and the new kit gets one.
//! - Death: as any found artefact (state 5 on the body, back to the pool at the respawn's
//!   unload; patch 78 drops one held in the hand). Never given again after a death.
//! - Every input is simulated state: the static info's kits, the statistics, the pool's states
//!   and the inventory.
#![forbid(unsafe_code)]

use artefact_carry::ArtefactId;
use artefact_carry::keeper::Kind;

use crate::artefact_skills::Skills;
use crate::vostok::keeper::Spawn;
use crate::vostok::log::{Piece::{Dec, Text}, line};
use crate::vostok::{Context, Flow, NoExit};

/// Where a pool artefact is: dormant (state 0, in no inventory) or given (in a container, on
/// a body, on the ground).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whereabouts {
    Dormant,
    InPlay,
}

/// One artefact of the map's Keeper pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registered {
    pub id: ArtefactId,
    pub kind: Kind,
    pub whereabouts: Whereabouts,
}

/// The place's lives so far, by the match's statistics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lives {
    /// No death or suicide counted yet (none before the start).
    First,
    Later,
}

/// The spawned kit's first artefact container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstContainer {
    Missing,
    Empty,
    Held,
}

/// What a spawn gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gift {
    /// No skill, a later life, no empty first container.
    Nothing,
    /// The kits carry no draw of a kind (the lobby's kits; an srp without the draw).
    NoDraw,
    /// The pool has no dormant artefact of the kind (a map without the pool).
    NoneFree(Kind),
    Take(ArtefactId, Kind),
}

/// The decision of one spawn, from its kit's skills, its lives, its first container and the
/// map's Keeper pool (in id order).
pub fn gift(skills: Skills, lives: Lives, container: FirstContainer, pool: impl Iterator<Item = Registered>) -> Gift {
    if !skills.keeper() || lives != Lives::First || container != FirstContainer::Empty {
        return Gift::Nothing;
    }
    let Some(kind) = skills.draw().and_then(|draw| Kind::of_draw(draw.get())) else { return Gift::NoDraw };
    // The lowest id: the same artefact in every game.
    pool.filter(|r| r.kind == kind && r.whereabouts == Whereabouts::Dormant)
        .min_by_key(|r| r.id.get())
        .map_or(Gift::NoneFree(kind), |r| Gift::Take(r.id, kind))
}

/// The register_artefacts entry (0x31B8E0): the map's Keeper pool, as the core world numbers it.
pub fn on_register(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::keeper::register(ctx);
    Flow::Continue
}

/// The respawn rule's spawn (0x316DA0) after the player is in the world (0x316EAB).
pub fn on_spawn(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(spawn) = Spawn::at(ctx) else { return Flow::Continue };
    let place = || Dec(spawn.place().into());
    match gift(spawn.skills(), spawn.lives(), spawn.first_container(), spawn.pool()) {
        Gift::Nothing => {}
        Gift::NoDraw => line(&[Text("keeper: place "), place(), Text(": no draw of a kind in its kit")]),
        Gift::NoneFree(kind) => line(&[Text("keeper: place "), place(), Text(": no "), Text(kind.name()), Text(" free in the pool")]),
        Gift::Take(id, kind) => {
            let given = spawn.give(id);
            line(&[
                Text("keeper: place "), place(), Text(if given { " takes " } else { " REFUSED " }),
                Text(kind.name()), Text(" id "), Dec(id.get().into()),
            ]);
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skills(value: f32) -> Skills {
        Skills::parse(value).unwrap()
    }

    fn pool() -> [Registered; 6] {
        let at = |id: u8, kind, whereabouts| Registered { id: ArtefactId::parse(id).unwrap(), kind, whereabouts };
        [
            at(30, Kind::Larkspur, Whereabouts::Dormant),
            at(31, Kind::Larkspur, Whereabouts::Dormant),
            at(32, Kind::Onyx, Whereabouts::InPlay),
            at(33, Kind::Onyx, Whereabouts::Dormant),
            at(34, Kind::Spring, Whereabouts::InPlay),
            at(35, Kind::Spring, Whereabouts::InPlay),
        ]
    }

    #[test]
    fn a_keepers_first_spawn_takes_the_lowest_dormant_of_the_draws_kind() {
        let keeper = |draw: u32| skills((8 + (draw << 9)) as f32);
        let first = |s| gift(s, Lives::First, FirstContainer::Empty, pool().into_iter());
        let id = |n| ArtefactId::parse(n).unwrap();
        assert_eq!(first(keeper(1)), Gift::Take(id(30), Kind::Larkspur));
        assert_eq!(first(keeper(2)), Gift::Take(id(33), Kind::Onyx));
        assert_eq!(first(keeper(3)), Gift::NoneFree(Kind::Spring));
        // A draw past the kinds (4..6, the parser's room) or none.
        assert_eq!(first(keeper(4)), Gift::NoDraw);
        assert_eq!(first(skills(8.0)), Gift::NoDraw);
    }

    #[test]
    fn nothing_without_the_skill_after_a_death_or_without_an_empty_container() {
        let keeper = skills((8 + (1 << 9)) as f32);
        // The draw without bit 3: no skill.
        assert_eq!(gift(skills(512.0), Lives::First, FirstContainer::Empty, pool().into_iter()), Gift::Nothing);
        assert_eq!(gift(keeper, Lives::Later, FirstContainer::Empty, pool().into_iter()), Gift::Nothing);
        assert_eq!(gift(keeper, Lives::First, FirstContainer::Held, pool().into_iter()), Gift::Nothing);
        assert_eq!(gift(keeper, Lives::First, FirstContainer::Missing, pool().into_iter()), Gift::Nothing);
        assert_eq!(gift(keeper, Lives::First, FirstContainer::Empty, core::iter::empty()), Gift::NoneFree(Kind::Larkspur));
    }
}
