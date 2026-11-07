//! Hidden Stash (patch 109, docs/notes/artefact-skills.md "Hidden Stash"): one more artefact
//! container, the slot right after the kit's own in the native run, that exists only from the
//! take that fills it to its holder's death, where its artefact goes back to where it came from.
//!
//! - srp sets modifier 40 bit 14 only on a kit with the skill whose pockets and containers are
//!   fewer than six (the hidden slot is one of the six quick positions).
//! - The run is the inventory's container slots ([+168h] the first, [+16Ch] how many), in the
//!   world save; whether its last slot is the hidden one follows from the kit's own count, which
//!   setup_from_profile computes as min(trunc(modifier 14), 19 - the run's start). Nothing is
//!   kept outside the world.
//! - The take (an anomaly's, Keeper's, patch 78's): with no container of the run empty and the
//!   stash shut, the run grows by its slot first, so the game's own take_artefact fills it.
//! - The death (both death entries, before patch 78's drop): an open stash's artefact is taken
//!   out by the game's own remove_item (dormant, back in its anomaly's pool or Keeper's) and the
//!   run is the kit's own again.
#![forbid(unsafe_code)]

use crate::vostok::hidden_stash as native;
use crate::vostok::{Context, Flow, NoExit};

/// An inventory slot of the six quick positions (13..=18), the only ones a run may take (the
/// world check's rule, srp-validate `InventoryHeader::read`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickSlot(u8);

impl QuickSlot {
    pub const FIRST: u8 = 13;
    /// One past the last (19, where the ammunition starts).
    pub const END: u8 = 19;

    pub const fn parse(slot: u8) -> Option<Self> {
        if slot >= Self::FIRST && slot < Self::END { Some(Self(slot)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The inventory's container run as the game has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// No container (count 0); the first-slot pointer means nothing then.
    Empty,
    /// `count` containers (1..) from `first`, all inside the quick positions.
    Slots { first: QuickSlot, count: u8 },
}

impl Run {
    /// The game's count and its first slot's index (None when the pointer is no slot of the
    /// inventory's). A run outside 13..18 is none this patch acts on.
    pub fn parse(count: u8, first: Option<u8>) -> Option<Self> {
        if count == 0 {
            return Some(Self::Empty);
        }
        let first = QuickSlot::parse(first?)?;
        (first.get().checked_add(count)? <= QuickSlot::END).then_some(Self::Slots { first, count })
    }
}

/// The kit's own container count from its modifier 14 (apply_modifier's sum: the kit's float and
/// its worn items' nodes) as setup_from_profile makes it: truncated, at most `19 - first`. A
/// value outside 0..=6 (a negative sum, which the game turns into a wrapped byte) is none: the
/// stash is not used with it.
pub fn own_count(containers: f32, run: Run) -> Option<u8> {
    if !(0.0..7.0).contains(&containers) {
        return None;
    }
    let containers = containers as u8;
    Some(match run {
        Run::Empty => containers,
        Run::Slots { first, .. } => containers.min(QuickSlot::END - first.get()),
    })
}

/// The kit's hidden slot at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stash {
    /// None for this kit: no skill (or no room, srp's bit), an unreadable run or count, or the
    /// slot it would take not empty.
    Unavailable,
    /// Shut: the run is the kit's own; a take with no container empty opens it at `at`.
    Shut { at: QuickSlot },
    /// Open: the run's last slot `at` is the hidden one (holding an artefact, or empty after a
    /// hand-over).
    Open { at: QuickSlot },
}

/// What the inventory says of the stash: the kit's skill bit, its modifier 14, the run, and
/// whether a slot is empty.
pub fn stash(skill: bool, containers: f32, run: Option<Run>, empty: impl Fn(QuickSlot) -> bool) -> Stash {
    let (true, Some(run)) = (skill, run) else { return Stash::Unavailable };
    let Some(own) = own_count(containers, run) else { return Stash::Unavailable };
    match run {
        // No container: the kit has none of its own (a run setup could not place, own > 0, is
        // not one the stash takes over). The slot is the first empty quick position.
        Run::Empty if own == 0 => {
            match (QuickSlot::FIRST..QuickSlot::END).filter_map(QuickSlot::parse).find(|&slot| empty(slot)) {
                Some(at) => Stash::Shut { at },
                None => Stash::Unavailable,
            }
        }
        Run::Empty => Stash::Unavailable,
        Run::Slots { first, count } if count == own => {
            match QuickSlot::parse(first.get() + count) {
                Some(at) if empty(at) => Stash::Shut { at },
                _ => Stash::Unavailable,
            }
        }
        Run::Slots { first, count } if count == own + 1 => {
            QuickSlot::parse(first.get() + count - 1).map_or(Stash::Unavailable, |at| Stash::Open { at })
        }
        Run::Slots { .. } => Stash::Unavailable,
    }
}

/// Room for a take, as patch 78, Keeper and the anomaly's can_use see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    /// No container and no stash to open.
    Missing,
    /// Every container full and no stash to open (an anomaly's take evicts, natively).
    Full,
    /// A container of the run is empty.
    Container,
    /// None empty, but the stash opens for the take.
    Stash(QuickSlot),
}

/// The room of a run whose containers are `empty` (each slot of the run, in order) with `stash`.
pub fn room(empty: impl IntoIterator<Item = bool>, stash: Stash) -> Room {
    let mut empty = empty.into_iter().peekable();
    let any = empty.peek().is_some();
    if empty.any(|empty| empty) {
        return Room::Container;
    }
    match (stash, any) {
        (Stash::Shut { at }, _) => Room::Stash(at),
        (_, true) => Room::Full,
        (_, false) => Room::Missing,
    }
}

/// artefact_container_core::use_execute before transfer_artefact (`artefact_container.transfer`,
/// after patch 80's score): the anomaly's take opens the user's stash when it needs it.
pub fn on_anomaly_take(ctx: &mut Context<'_>) -> Flow<NoExit> {
    native::open_for_anomaly(ctx);
    Flow::Continue
}

/// artefact_container_core::can_use's container test (0x31C6FB): a user with no container but a
/// stash to open may use the anomaly.
pub fn on_can_use(ctx: &mut Context<'_>) -> Flow<crate::vostok::HiddenStashOnCanUseExit> {
    if native::can_use_with_stash(ctx) {
        return Flow::Exit(crate::vostok::HiddenStashOnCanUseExit::Room);
    }
    Flow::Continue
}

/// base_player::on_player_death's entry (0x2D77C0), before patch 78's drop: an open stash's
/// artefact goes back, dormant.
pub fn on_death(ctx: &mut Context<'_>) -> Flow<NoExit> {
    native::empty_at_death(ctx, native::Death::Base);
    Flow::Continue
}

/// The client player's death override (0x1F28E0, which never calls the base's): the same.
pub fn on_client_death(ctx: &mut Context<'_>) -> Flow<NoExit> {
    native::empty_at_death(ctx, native::Death::Client);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(n: u8) -> QuickSlot {
        QuickSlot::parse(n).unwrap()
    }

    fn run(count: u8, first: u8) -> Option<Run> {
        Run::parse(count, Some(first))
    }

    #[test]
    fn a_run_is_inside_the_quick_positions() {
        assert_eq!(Run::parse(0, None), Some(Run::Empty));
        assert_eq!(Run::parse(0, Some(3)), Some(Run::Empty));
        assert_eq!(run(2, 15), Some(Run::Slots { first: slot(15), count: 2 }));
        assert_eq!(run(6, 13), Some(Run::Slots { first: slot(13), count: 6 }));
        assert_eq!(run(1, 18), Some(Run::Slots { first: slot(18), count: 1 }));
        assert_eq!(run(2, 18), None);
        assert_eq!(run(1, 12), None);
        assert_eq!(run(1, 19), None);
        assert_eq!(Run::parse(1, None), None);
    }

    #[test]
    fn the_kits_own_count_is_setups() {
        let at15 = run(1, 15).unwrap();
        assert_eq!(own_count(2.0, at15), Some(2));
        assert_eq!(own_count(2.9, at15), Some(2));
        assert_eq!(own_count(6.0, at15), Some(4));
        assert_eq!(own_count(0.0, Run::Empty), Some(0));
        assert_eq!(own_count(0.5, Run::Empty), Some(0));
        assert_eq!(own_count(-1.0, at15), None);
        assert_eq!(own_count(-0.5, at15), None);
        assert_eq!(own_count(7.0, at15), None);
        assert_eq!(own_count(f32::NAN, at15), None);
    }

    #[test]
    fn shut_after_the_kits_own_and_open_one_past_them() {
        let empty = |_: QuickSlot| true;
        // Two drugs (13, 14), two containers (15, 16): the stash is 17.
        assert_eq!(stash(true, 2.0, run(2, 15), empty), Stash::Shut { at: slot(17) });
        assert_eq!(stash(true, 2.0, run(3, 15), empty), Stash::Open { at: slot(17) });
        // No skill, an unreadable run, a count that is neither.
        assert_eq!(stash(false, 2.0, run(2, 15), empty), Stash::Unavailable);
        assert_eq!(stash(true, 2.0, None, empty), Stash::Unavailable);
        assert_eq!(stash(true, 2.0, run(4, 15), empty), Stash::Unavailable);
        assert_eq!(stash(true, 2.0, run(1, 15), empty), Stash::Unavailable);
        // The slot past the run holds something (a drug): shut it stays, no stash.
        assert_eq!(stash(true, 2.0, run(2, 15), |s| s != slot(17)), Stash::Unavailable);
        // Six used: the run ends at 18, nothing past it.
        assert_eq!(stash(true, 2.0, run(2, 17), empty), Stash::Unavailable);
        // Open at the last quick position.
        assert_eq!(stash(true, 1.0, run(2, 17), empty), Stash::Open { at: slot(18) });
    }

    #[test]
    fn with_no_container_the_first_empty_quick_position() {
        // Three drugs: 16 is the first empty one.
        let empty = |s: QuickSlot| s.get() >= 16;
        assert_eq!(stash(true, 0.0, Some(Run::Empty), empty), Stash::Shut { at: slot(16) });
        // Opened: a run of one at 16, the kit's own 0.
        assert_eq!(stash(true, 0.0, run(1, 16), empty), Stash::Open { at: slot(16) });
        // Six drugs: none empty.
        assert_eq!(stash(true, 0.0, Some(Run::Empty), |_| false), Stash::Unavailable);
        // A kit with containers whose run setup could not place.
        assert_eq!(stash(true, 2.0, Some(Run::Empty), empty), Stash::Unavailable);
    }

    #[test]
    fn room_is_own_containers_first_then_the_stash() {
        let shut = Stash::Shut { at: slot(17) };
        assert_eq!(room([false, true], shut), Room::Container);
        assert_eq!(room([false, false], shut), Room::Stash(slot(17)));
        assert_eq!(room([], Stash::Shut { at: slot(13) }), Room::Stash(slot(13)));
        assert_eq!(room([false, false], Stash::Unavailable), Room::Full);
        assert_eq!(room([false, false, false], Stash::Open { at: slot(17) }), Room::Full);
        // Open and emptied by a hand-over: its own empty slot is the room.
        assert_eq!(room([false, false, true], Stash::Open { at: slot(17) }), Room::Container);
        assert_eq!(room([], Stash::Unavailable), Room::Missing);
    }
}
