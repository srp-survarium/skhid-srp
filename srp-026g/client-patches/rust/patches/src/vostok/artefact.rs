//! An anomaly's artefact taken (patch 80). artefact_container_core::
//! use_execute (0x31C780), once the collection time is over, hands the
//! container's artefact to its user (transfer_artefact 0x31C8A0 ->
//! inventory::take_artefact 0x2FC850) and then counts event 5 for the user's
//! place and kit (the statistics handler's slot +10h, 0x26DAC0 -> 0x2F55F0).
//! take_artefact puts it into the first empty artefact slot or, when every
//! slot holds one, in place of the first slot's (0x2FC897). Every field is
//! fields_026g.tsv's; this is what they mean before the transfer.

use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::statistics::{Score, ScoreEvent};
use super::world::read;
use crate::raw;

/// The user's artefact slots before the transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slots {
    /// One is empty: the artefact goes there.
    Free,
    /// Every one holds an artefact (or there is none): the artefact takes
    /// the first slot's place.
    Full,
}

impl Slots {
    /// take_artefact's own search (0x2FC880): each slot empty or not, in
    /// order; Free at the first empty one.
    fn of(empty: impl IntoIterator<Item = bool>) -> Slots {
        if empty.into_iter().any(|empty| empty) { Slots::Free } else { Slots::Full }
    }
}

/// A user about to be given a container's artefact, at
/// `artefact_container.transfer`: its slots then, and its cells of the
/// match's statistics.
pub struct ArtefactPickup<'t> {
    slots: Slots,
    score: Score,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ArtefactPickup<'t> {
    /// The pickup the site is about to transfer; None when the match has no
    /// statistics handler (use_execute then counts nothing either) or the
    /// user's place or kit is none of the records'.
    pub fn at_transfer(ctx: &'t Context<'_>) -> Option<ArtefactPickup<'t>> {
        use raw::site::artefact_container_transfer as site;
        // SAFETY: the context is this site's: the user and the container
        // use_execute holds, their inventory, world core and statistics
        // alive on the game's thread; take_artefact reads the same slots.
        unsafe {
            let (user, container) = (site::user(ctx)?, site::container(ctx)?);
            let score = Score::of(container.world_core()?.statistics()?.shared(), user)?;
            let inventory = user.inventory()?;
            // take_artefact's: the first slot and how many (0x2FC858, 0x2FC86E).
            let count = usize::from(read(inventory.artefact_slot_count()));
            let slots = match inventory.artefact_slots_begin() {
                Some(first) => Slots::of((0..count).map(|slot| first.at(slot).item().is_none())),
                None => Slots::Full,
            };
            Some(ArtefactPickup { slots, score, thread: PhantomData })
        }
    }

    pub fn slots(&self) -> Slots {
        self.slots
    }

    /// Counts `event` once for the user's place and kit, as the game counts
    /// one.
    pub fn count(&self, event: ScoreEvent) {
        self.score.count(event, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_free_with_an_empty_one() {
        assert_eq!(Slots::of([false, true, false]), Slots::Free);
        assert_eq!(Slots::of([true]), Slots::Free);
        assert_eq!(Slots::of([false, false]), Slots::Full);
        assert_eq!(Slots::of([]), Slots::Full);
    }
}
