//! A hit that took health, and the victim's drugs in use (patch 84,
//! docs/notes/drug-interrupt.md). base_player::hit_impl (0x2D9B90) asks
//! damage_model::hit_body_part (0x2E5080) to deal the hit; it says 1 when
//! the damage left by armour and protectors is above 0.0, and only then does
//! hit_impl go on (the slow, the listeners, the death). `player.hit_applied`
//! is there. Every game runs it for every hit of every player in its
//! simulation, so what is decided there is decided alike everywhere.
//!
//! A drug is a `survarium::medkit` (the one vtable MEDKIT_VTABLE, whatever
//! folder its item's options are in); its data/influences are the pools it
//! heals (`Effect`). Its use is set_active's flag (+2B0h): set at
//! the press (medkit::action 0x33D700, which takes the unit then), through
//! its use time (+2ACh counting down) and its active time (+2A8h counting
//! down, healing each tick); medkit::tick ends it with set_active(0) when
//! the active time is spent or the holder died. A used drug stays in its
//! slot, a stack of none too, until its effect ends (0x33D765). Every field
//! is fields_026g.tsv's.

use super::context::Context;
use super::hook::Thread;
use super::medical::name_equal;
use super::sync::Lock;
use super::world::read;
use crate::raw;

/// hit_type_enum: survarium::hit_type (0x304370) gives a name's index in the
/// names at 0x9257E0 (0.21d's 0x73E7A0 the same), which hit_info +1Ch holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitType {
    /// A bullet: ammo_data's damage_type, which no 0.26g ammo sets, is 0.
    /// (The suicide key's brain hit is one too, 0x2D8C44.)
    Ranged,
    /// A weapon's melee (every 0.26g weapon's melee/data), a fall.
    Blunt,
    /// The trap.
    Slash,
    /// A grenade's or a trap's blast (splash_damage_type).
    Explosive,
    Toxic,
    Radiation,
    Fire,
    Shock,
}

impl HitType {
    /// The names' order (0x9257E0); a value past them is no hit type.
    pub(super) fn parse(value: u32) -> Option<HitType> {
        const ALL: [HitType; 8] = [
            HitType::Ranged,
            HitType::Blunt,
            HitType::Slash,
            HitType::Explosive,
            HitType::Toxic,
            HitType::Radiation,
            HitType::Fire,
            HitType::Shock,
        ];
        ALL.get(usize::try_from(value).ok()?).copied()
    }
}

/// A hit's simulation milliseconds (hit_info +3Ch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitTime(u32);

/// A hit hit_impl has just dealt: it took health off its victim.
pub struct AppliedHit<'t> {
    victim: raw::Player,
    kind: HitType,
    time: HitTime,
    thread: &'t Thread,
}

impl<'t> AppliedHit<'t> {
    /// The hit at `player.hit_applied`; None for a hit type the game has no
    /// name for.
    pub fn at_applied(ctx: &'t Context<'_>) -> Option<AppliedHit<'t>> {
        use raw::site::player_hit_applied as site;
        // SAFETY: the context is this site's: hit_impl's victim (alive: it
        // returned at 0x2D9BA3 otherwise) and its hit_info, on the thread
        // running the hit.
        unsafe {
            let (victim, hit) = (site::victim(ctx)?, site::hit(ctx)?);
            let kind = HitType::parse(read(hit.kind()))?;
            let time = HitTime(read(hit.time()));
            Some(AppliedHit { victim, kind, time, thread: ctx.thread() })
        }
    }

    pub fn kind(&self) -> HitType {
        self.kind
    }

    pub fn time(&self) -> HitTime {
        self.time
    }

    /// The victim's drugs in use, in its inventory slots' order.
    pub fn drugs_in_use(&self) -> DrugsInUse<'_> {
        // SAFETY: the victim is alive for the hook, on its thread.
        unsafe { DrugsInUse::of(self.thread, self.victim) }
    }
}

/// A player's drugs in use, one slot after another.
pub struct DrugsInUse<'h> {
    first: Option<raw::InventorySlot>,
    count: usize,
    next: usize,
    thread: &'h Thread,
}

impl<'h> DrugsInUse<'h> {
    /// A player's drugs in use, in its inventory slots' order.
    ///
    /// # Safety
    /// The player is alive for the hook running on `thread`.
    pub(super) unsafe fn of(thread: &'h Thread, player: raw::Player) -> DrugsInUse<'h> {
        // SAFETY: the caller's live player owns its inventory and the 23
        // slots' array (inventory::update_weight's loop).
        let slots = unsafe {
            player.inventory().map(|inventory| {
                let first = inventory.slots();
                (first, inventory.slots_end().index_from(first).unwrap_or(0))
            })
        };
        let (first, count) = match slots {
            Some((first, count)) => (Some(first), count),
            None => (None, 0),
        };
        DrugsInUse { first, count, next: 0, thread }
    }
}

impl<'h> Iterator for DrugsInUse<'h> {
    type Item = DrugInUse<'h>;

    fn next(&mut self) -> Option<DrugInUse<'h>> {
        let first = self.first?;
        while self.next < self.count {
            let slot = self.next;
            self.next += 1;
            // SAFETY: a slot of the victim's array (slot < count); an item
            // there is the inventory's, alive while it holds it; a medkit's
            // fields are read only once its vtable says it is one.
            let drug = unsafe {
                first.at(slot).item().and_then(|item| {
                    (read(item.vtable()) == raw::image::MEDKIT_VTABLE).then(|| item.medkit())
                })
            };
            let Some(medkit) = drug else { continue };
            // SAFETY: as above.
            let in_use = unsafe { read(medkit.active()) != 0 };
            if in_use {
                return Some(DrugInUse { medkit, thread: self.thread });
            }
        }
        None
    }
}

/// What a drug does while active, from its data/influences: the records
/// (+280h, 14h each, the pool's name inline, what it heals a second at
/// +10h) and their count (+288h; medkit::tick's loop, 0x33D8F5 / 0x33DA1B).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// It heals the body: the medkits and the bandages (the science medkit
    /// the infection pool too).
    Heals,
    /// Of its records, those that heal (an amount above 0) heal the
    /// infection pool and nothing else: the antitoxin, one record or the
    /// medkit template's seven kept at 0 beside it.
    Antitoxin,
    /// It names none: its damage_protection is all it does (the painkiller).
    ProtectsOnly,
}

/// The pool of a record that heals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pool {
    Infection,
    Body,
}

impl Effect {
    /// Of `records` data/influences, `healing` the pools of those whose
    /// amount is above 0.
    fn of(records: u8, healing: impl Iterator<Item = Pool>) -> Effect {
        let mut healing = healing.peekable();
        if records == 0 {
            Effect::ProtectsOnly
        } else if healing.peek().is_some() && healing.all(|pool| pool == Pool::Infection) {
            Effect::Antitoxin
        } else {
            Effect::Heals
        }
    }
}

/// A drug's effect, read from its records.
///
/// # Safety
/// `medkit` is a live medkit (its vtable checked): it owns its declared
/// array of healing records, each with its pool's name inline.
pub(super) unsafe fn effect(medkit: raw::Medkit) -> Effect {
    // SAFETY: the caller's.
    unsafe {
        let records = read(medkit.influences());
        let healing = medkit.heals().into_iter().flat_map(|first| {
            (0..usize::from(records))
                .map(move |i| first.at(i))
                .filter(|record| read(record.amount()) > 0.0)
                .map(|record| {
                    if name_equal(record.name(), c"infection".as_ptr().cast()) { Pool::Infection } else { Pool::Body }
                })
        });
        Effect::of(records, healing)
    }
}

/// A drug its holder is using: in its use time or its active time.
pub struct DrugInUse<'h> {
    medkit: raw::Medkit,
    thread: &'h Thread,
}

/// Stops one at a time: with 256 bullets or more in flight
/// bullet_manager::tick (0x321D10) runs them as tasks on several threads,
/// hits and all, and set_active(0) unlinks the drug from the world's tick
/// list (0x2D2DB0), which every drug shares.
static STOPPING: Lock<()> = Lock::new(());

impl DrugInUse<'_> {
    pub fn effect(&self) -> Effect {
        // SAFETY: a medkit of its holder's slot, which owns its records.
        unsafe { effect(self.medkit) }
    }

    /// The drug's item, for reading its records; `stop` is the one way to
    /// end it.
    pub(super) fn medkit(&self) -> raw::Medkit {
        self.medkit
    }

    /// Ends it as medkit::tick does once the active time is spent:
    /// set_active(0). In its use time it never starts healing; in its
    /// active time it heals no more, and what it healed stays. Its own
    /// state says how far it got (+2ACh, +2A8h), so set_active undoes only
    /// what was done (the drug_activity node once the active time began).
    /// The unit was taken at the press. Once only: set_active(0) twice
    /// unlinks its tick node and drug_activity node again (the tick lists
    /// at world+19830h, the kit's modifier list emptied), so the flag
    /// `DrugsInUse` saw is read again under the lock: a bullet of another
    /// task that hit the same holder in this tick may have stopped it since.
    pub fn stop(self, at: HitTime) {
        STOPPING.with(self.thread, |_| {
            // SAFETY: a drug in the victim's slot, on the thread of the hit,
            // under the lock every stop takes; set_active is what the game's
            // own tick calls on it in the simulation, and what it touches is
            // the drug's, its holder's and the world's tick list.
            unsafe {
                if read(self.medkit.active()) != 0 {
                    raw::engine_medkit_set_active(Some(self.medkit), 0, at.0);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_types_are_the_names_order() {
        assert_eq!(HitType::parse(0), Some(HitType::Ranged));
        assert_eq!(HitType::parse(1), Some(HitType::Blunt));
        assert_eq!(HitType::parse(3), Some(HitType::Explosive));
        assert_eq!(HitType::parse(7), Some(HitType::Shock));
        assert_eq!(HitType::parse(8), None);
        assert_eq!(HitType::parse(u32::MAX), None);
    }

    #[test]
    fn a_drug_without_influences_only_protects() {
        assert_eq!(Effect::of(0, [].into_iter()), Effect::ProtectsOnly);
    }

    #[test]
    fn a_drug_that_heals_the_infection_pool_alone_is_the_antitoxin() {
        use Pool::{Body, Infection};
        // The live config's one record; the medkit template's seven at 0 beside it.
        assert_eq!(Effect::of(1, [Infection].into_iter()), Effect::Antitoxin);
        assert_eq!(Effect::of(8, [Infection].into_iter()), Effect::Antitoxin);
        // The medkits and the bandages; the science medkit heals both.
        assert_eq!(Effect::of(7, [Body; 7].into_iter()), Effect::Heals);
        assert_eq!(Effect::of(8, [Body, Infection].into_iter()), Effect::Heals);
        assert_eq!(Effect::of(8, [Infection, Body].into_iter()), Effect::Heals);
        // Records that heal nothing: no antitoxin.
        assert_eq!(Effect::of(7, [].into_iter()), Effect::Heals);
    }
}
