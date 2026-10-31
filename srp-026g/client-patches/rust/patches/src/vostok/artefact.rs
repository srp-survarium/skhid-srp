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
use super::site;
use super::hook::Thread;
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

/// An event of `match_stats_events_dict_enum` (28 of them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreEvent(u8);

impl ScoreEvent {
    /// A taken artefact, the one use_execute counts.
    pub const ARTEFACT: ScoreEvent = ScoreEvent(5);
    /// 0.21d's winners' battery bonus: 0.26g counts it nowhere (its
    /// finish_match 0x2F4CF0 counts 11, 12 and 27 only; no site adds to it).
    pub const WINNING_TEAM_VI_BONUS: ScoreEvent = ScoreEvent(16);
}

/// A place of the match: the statistics hold 20 records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place(u8);

impl Place {
    fn parse(place: u8) -> Option<Place> {
        (place < 20).then_some(Place(place))
    }
}

/// A kit: a record counts each event for three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kit(u8);

impl Kit {
    const COUNT: usize = 3;

    fn parse(kit: u8) -> Option<Kit> {
        (usize::from(kit) < Self::COUNT).then_some(Kit(kit))
    }
}

/// A user about to be given a container's artefact, at
/// `artefact_container.transfer`: its slots then, and its cells of the
/// match's statistics.
pub struct ArtefactPickup<'t> {
    slots: Slots,
    statistics: raw::SharedStatistics,
    place: Place,
    kit: Kit,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ArtefactPickup<'t> {
    /// The pickup the site is about to transfer; None when the match has no
    /// statistics handler (use_execute then counts nothing either) or the
    /// user's place or kit is none of the records'.
    pub fn at_transfer(ctx: &'t Context<'_, site::ArtefactContainerTransfer>) -> Option<ArtefactPickup<'t>> {
        use raw::site::artefact_container_transfer as at;
        let (user, container) = (at::user(ctx)?, at::container(ctx)?);
        // SAFETY: the site's user and container use_execute holds, their
        // inventory, world core and statistics alive on the game's thread;
        // take_artefact reads the same slots.
        unsafe {

            let statistics = container.world_core()?.statistics()?.shared();
            let place = Place::parse(read(user.place()))?;
            let kit = Kit::parse(read(user.kit()))?;
            let inventory = user.inventory()?;
            // take_artefact's: the first slot and how many (0x2FC858, 0x2FC86E).
            let count = usize::from(read(inventory.artefact_slot_count()));
            let slots = match inventory.artefact_slots_begin() {
                Some(first) => Slots::of((0..count).map(|slot| first.at(slot).item().is_none())),
                None => Slots::Full,
            };
            Some(ArtefactPickup { slots, statistics, place, kit, thread: PhantomData })
        }
    }

    pub fn slots(&self) -> Slots {
        self.slots
    }

    /// Counts `event` once for the user's place and kit as the game counts
    /// one (0x2F5606, 0x2F5610): its cell + 1, then the listener told
    /// (place, event, 1) when it is set, so the scoreboard shows it at once.
    pub fn count(&self, event: ScoreEvent) {
        let cell = usize::from(event.0) * Kit::COUNT + usize::from(self.kit.0);
        // SAFETY: the statistics are alive for the hook; the place is one of
        // the 20 records, the cell one of its 28 x 3.
        unsafe {
            let count = self.statistics.players().at(usize::from(self.place.0)).events().at(cell).count();
            count.write_volatile(read(count).wrapping_add(1));
            let listener = self.statistics.listener();
            if read(listener.function()) != 0 {
                raw::engine_statistics_notify(Some(listener), u32::from(self.place.0), u32::from(event.0), 1);
            }
        }
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

    #[test]
    fn places_and_kits_are_the_records() {
        assert_eq!(Place::parse(19), Some(Place(19)));
        assert_eq!(Place::parse(20), None);
        assert_eq!(Kit::parse(2), Some(Kit(2)));
        assert_eq!(Kit::parse(3), None);
    }
}
