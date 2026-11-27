//! Courier (patch 109, docs/notes/artefact-skills.md "Courier"): at a respawn into another kit,
//! the artefacts of the kit the player left come along into the new kit's empty containers.
//!
//! - When: the respawn rule's spawn (0x316DA0), every spawn of a match and every replay of its
//!   tick. Its start says which kit the place was in and which it spawns in (`on_spawn_kits`), its
//!   end has the new kit set up and in the world (`on_spawned`): the move is made there, once.
//! - What: only the kit the place was in gives (the owner, 2026-11-21: kit 1 with two artefacts
//!   dies into kit 2, kit 2 dies into kit 3: kit 3 gets nothing of kit 1's), each artefact it
//!   holds, in container order, as many as the new kit has empty containers; the rest stay where
//!   they are. Any artefact, found or Keeper's. The kit was left at a death, or alive: a kit
//!   switch in a countdown (a round mode's, a match start's) and a round's start.
//! - A respawn in the same kit moves nothing: the game keeps its artefacts (setup_from_profile's
//!   on_taken switches their passives on again).
//! - Every input is simulated state (the place's kits, the static info's skills, the inventories,
//!   the artefacts' states), so every game, late joiner, resync and replay moves the same.
#![forbid(unsafe_code)]

use crate::artefact_skills::Skills;
use crate::vostok::log::{Piece::{Dec, Text}, line};
use crate::vostok::{Context, Flow, GameCell, NoExit, courier};

/// One of a place's three kits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kit(u8);

impl Kit {
    pub fn parse(n: u8) -> Option<Kit> {
        (n < 3).then_some(Kit(n))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// A spawn from one kit into another, between the spawn's start and its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaving {
    pub place: u8,
    pub from: Kit,
}

/// The spawn's start: the place, the kit it is in and the kit it spawns in. None for a spawn in
/// the same kit (or a value past the game's).
pub fn leaving(place: u32, current: u8, next: u8) -> Option<Leaving> {
    let place = u8::try_from(place).ok().filter(|&place| usize::from(place) < crate::vostok::types::Place::COUNT)?;
    let (from, into) = (Kit::parse(current)?, Kit::parse(next)?);
    (from != into).then_some(Leaving { place, from })
}

/// The kit whose artefacts come to the spawn's new kit `now`: the one its own start left, when the
/// new kit has the skill.
pub fn giver(skills: Skills, leaving: Option<Leaving>, place: u8, now: Kit) -> Option<Kit> {
    let leaving = leaving.filter(|leaving| leaving.place == place && leaving.from != now)?;
    skills.courier().then_some(leaving.from)
}

/// What a move did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Carried {
    /// Into the new kit's containers.
    pub moved: u8,
    /// Left in the old kit: no empty container for them.
    pub left: u8,
}

/// The spawn in progress (set at its start, taken at its end, in the same call).
static LEAVING: GameCell<Option<Leaving>> = GameCell::new(None);

/// The respawn rule's spawn before it makes the chosen kit current.
pub fn on_spawn_kits(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let leaving = courier::spawn_kits(ctx).and_then(|(place, current, next)| leaving(place, current, next));
    LEAVING.set(ctx.thread(), leaving);
    Flow::Continue
}

/// The respawn rule's spawn at its end: the new kit set up and in the world.
pub fn on_spawned(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let leaving = LEAVING.get(ctx.thread());
    LEAVING.set(ctx.thread(), None);
    let Some(spawned) = courier::Spawned::at(ctx) else { return Flow::Continue };
    let Some(from) = giver(spawned.skills(), leaving, spawned.place(), spawned.kit()) else { return Flow::Continue };
    let carried = spawned.carry_from(from);
    if carried != Carried::default() {
        line(&[
            Text("courier: place "), Dec(spawned.place().into()), Text(" kit "), Dec(from.get().into()),
            Text(" -> kit "), Dec(spawned.kit().get().into()), Text(": "), Dec(carried.moved.into()),
            Text(" artefacts moved, "), Dec(carried.left.into()), Text(" left"),
        ]);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const COURIER: f32 = 16384.0;

    fn kit(n: u8) -> Kit {
        Kit::parse(n).unwrap()
    }

    #[test]
    fn a_spawn_into_another_kit_leaves_the_one_it_was_in() {
        assert_eq!(leaving(3, 0, 1), Some(Leaving { place: 3, from: kit(0) }));
        assert_eq!(leaving(19, 2, 0), Some(Leaving { place: 19, from: kit(2) }));
        // The same kit, a kit past three, a place past the roster.
        assert_eq!(leaving(3, 1, 1), None);
        assert_eq!(leaving(3, 3, 1), None);
        assert_eq!(leaving(3, 0, 3), None);
        assert_eq!(leaving(20, 0, 1), None);
        assert_eq!(leaving(0x103, 0, 1), None);
    }

    #[test]
    fn only_the_kit_the_spawn_left_gives_and_only_with_the_skill() {
        let with = Skills::parse(COURIER).unwrap();
        let without = Skills::parse(8.0).unwrap();
        let left = leaving(4, 0, 1);
        assert_eq!(giver(with, left, 4, kit(1)), Some(kit(0)));
        assert_eq!(giver(without, left, 4, kit(1)), None);
        // Another place's spawn, no spawn start seen, the kit it left.
        assert_eq!(giver(with, left, 5, kit(1)), None);
        assert_eq!(giver(with, None, 4, kit(1)), None);
        assert_eq!(giver(with, left, 4, kit(0)), None);
        // The owner's run: kit 1 into kit 2, kit 2 into kit 3, kit 3 into kit 1: each spawn
        // takes from the kit it left, never from an earlier one.
        let runs = [(0, 1), (1, 2), (2, 0)];
        let givers = runs.map(|(from, into)| giver(with, leaving(4, from, into), 4, kit(into)));
        assert_eq!(givers, [Some(kit(0)), Some(kit(1)), Some(kit(2))]);
    }
}
