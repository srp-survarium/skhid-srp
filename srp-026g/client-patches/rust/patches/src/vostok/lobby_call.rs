//! The lobby movie's calls to the exe at `lobby.callback` (patch 207): its
//! `ExternalInterface.call(method, ...)` at lobby_menu::scaleform_callback's
//! start (0x28C2C0), before the menu's logic matches the name with its own
//! (lobby_menu_logic::call 0x2812E0); and the lobby's inventory, which such a
//! call acts on ([`LobbyInventory`]).
//!
//! A value of an upgrade on its host goes to the lobby as the decal editor's
//! accept sends it ([`LobbyInventory::place_upgrade`]): `46 05 <active kit>
//! <host> <upgrade> <u64 value>` by the game's own send_apply_decal
//! (0x270950), which then writes the value into the inventory's record whose
//! item id is the upgrade's and copies the account's items
//! (copy_account_items 0x270320). A weapon's records are srp's weapon kit
//! (`weapon-kit`): the record of that id can be a kit, whose value is not the
//! upgrade's, so the host is given the records the caller packed (the kit as
//! the server holds it after the request) and the account's items are copied
//! again.

use core::ffi::CStr;
use core::marker::PhantomData;

use weapon_kit::{RECORDS, Record, Wire};

use super::context::Context;
use super::hook::Thread;
use super::ui::{GFX_NUMBER, GFX_UINT, TEXT_LIMIT, c_string};
use super::upgrades::{self, Records};
use crate::raw;

/// A GFx value's type of a signed int (Scaleform::GFx::Value's VT_Int).
const GFX_INT: u32 = 3;
/// A GFx value's type bits (the rest its managed and display bits).
const GFX_TYPE: u32 = 0x3F;

/// The lobby movie's call to the exe at `lobby.callback`.
pub struct LobbyCall<'c> {
    handler: raw::LobbyMenuExternal,
    frame: raw::LobbyCallbackFrame,
    method: &'c CStr,
    thread: &'c Thread,
}

impl<'c> LobbyCall<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<LobbyCall<'c>> {
        use raw::site::lobby_callback as site;
        // SAFETY: the context is this site's: the lobby menu's flash external
        // handler and the callback's frame, alive for the call, on the lobby's
        // thread; the method's name a C string the movie gave.
        let (handler, frame, method) = unsafe {
            let (handler, frame) = (site::menu(ctx)?, site::frame(ctx)?);
            (handler, frame, c_string(frame.method(), TEXT_LIMIT)?)
        };
        Some(LobbyCall { handler, frame, method, thread: ctx.thread() })
    }

    pub fn method(&self) -> &CStr {
        self.method
    }

    pub fn thread(&self) -> &Thread {
        self.thread
    }

    /// The call's argument `index` when it is a whole number a u32 holds: a
    /// GFx uint, an int not below 0, or a Number (an AS3 `uint` reaches the
    /// exe as one of them; the native handlers read their +8, 0x29A837).
    pub fn uint_argument(&self, index: u32) -> Option<u32> {
        // SAFETY: the callback's arguments and their count, alive for the
        // call; `index` below the count.
        let (kind, low, high) = unsafe {
            if index >= *self.frame.count() {
                return None;
            }
            let value = self.frame.arguments()?.at(index as usize);
            (*value.kind() & GFX_TYPE, *value.value(), *value.value_high())
        };
        match kind {
            GFX_UINT => Some(low),
            GFX_INT => u32::try_from(low as i32).ok(),
            GFX_NUMBER => whole(f64::from_bits(u64::from(high) << 32 | u64::from(low))),
            _ => None,
        }
    }

    /// The lobby's inventory: the lobby client of the menu's game (its
    /// network client's, as accept_decal_edit reaches it: 0x2639A6..0x2639BD).
    pub fn inventory(&self) -> Option<LobbyInventory<'_>> {
        // SAFETY: the handler's own logic, its menu, the menu's game and its
        // network client live as long as the lobby menu, on its thread.
        let client = unsafe { self.handler.logic().menu()?.game()?.network_client()? };
        // SAFETY: the lobby client inside the network client (0x219960).
        let client = unsafe { client.lobby() };
        Some(LobbyInventory { client, thread: PhantomData })
    }
}

/// `n` when it is a whole number from 0 to u32::MAX.
fn whole(n: f64) -> Option<u32> {
    let fits = (0.0..=f64::from(u32::MAX)).contains(&n);
    (fits && f64::from(n as u32) == n).then_some(n as u32)
}

/// The account's inventory, the lobby client's item records
/// (+1EA978h..+1EA97Ch), on the lobby's thread.
pub struct LobbyInventory<'t> {
    client: raw::LobbyClient,
    thread: PhantomData<&'t Thread>,
}

/// An item of the inventory, read once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyItem {
    pub id: u32,
    pub dict: u16,
    /// Its upgrade records; None when they are not readable (more than
    /// three, a broken vector).
    pub records: Option<Records>,
}

/// Why [`LobbyInventory::place_upgrade`] gave the host no records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotGiven {
    /// The host is not in the inventory after the send.
    NoHost,
    /// The records are not the host's own with other values.
    OtherRecords,
}

impl LobbyInventory<'_> {
    /// The record of item `id` (none for 0), as find_item_by_id finds it.
    fn record(&self, id: u32) -> Option<raw::ItemRecord> {
        if id == 0 {
            return None;
        }
        // SAFETY: the lobby client's inventory vector and its end, alive on
        // the lobby's thread; begin..end are its item records.
        unsafe {
            let inventory = self.client.inventory();
            let begin = inventory.begin()?;
            let count = inventory.end()?.index_from(begin)?;
            (0..count).map(|i| begin.at(i)).find(|item| *item.id() == id)
        }
    }

    /// The inventory's item `id`.
    pub fn item(&self, id: u32) -> Option<LobbyItem> {
        let record = self.record(id)?;
        // SAFETY: an item record of the inventory, alive on its thread.
        unsafe { Some(LobbyItem { id, dict: *record.dict(), records: upgrades::read(record.upgrades()) }) }
    }

    /// Sends the lobby `46 05`, the value of `upgrade` on `host` (the active
    /// kit first), as the decal editor's accept does (0x263A28..0x263A5C),
    /// then gives the host `records` (the host's own with the values the
    /// lobby will hold) and copies the account's items again, as the send
    /// copied them after its own write. Whether the host got them.
    pub fn place_upgrade(&self, host: u32, upgrade: u32, value: u64, records: &Wire) -> Result<(), NotGiven> {
        // SAFETY: the lobby client and its active kit, on the lobby's thread;
        // send_apply_decal begins and sends the request on the client's own
        // link (none: nothing goes, 0x37A12A) and writes only the host's
        // record of `upgrade`, then copies the account's items.
        unsafe {
            let kit = *self.client.active_kit();
            raw::engine_send_apply_decal(Some(self.client), kit, host, upgrade, value as u32, (value >> 32) as u32);
        }
        let record = self.record(host).ok_or(NotGiven::NoHost)?;
        let records = records.as_slice();
        // SAFETY: the host's item record of the inventory, alive on its
        // thread; its own vector, the records written over its present ones
        // only (the same count, each the same item, dict id and slot).
        unsafe {
            let list = record.upgrades();
            let held = upgrades::read(list).ok_or(NotGiven::OtherRecords)?;
            if !same_records(held.as_slice(), records) {
                return Err(NotGiven::OtherRecords);
            }
            let begin = list.begin().ok_or(NotGiven::OtherRecords)?;
            for (i, given) in records.iter().enumerate() {
                upgrades::write(begin.at(i), given);
            }
            raw::engine_copy_account_items(Some(self.client));
        }
        Ok(())
    }
}

/// `given` are `held` with other values only: one for one, the same item id,
/// dict id and slot (at most [`RECORDS`]).
fn same_records(held: &[Record], given: &[Record]) -> bool {
    held.len() == given.len()
        && given.len() <= RECORDS
        && held.iter().zip(given).all(|(a, b)| (a.id, a.dict, a.slot) == (b.id, b.dict, b.slot))
}

#[cfg(test)]
mod tests {
    use super::{same_records, whole};
    use weapon_kit::Record;

    #[test]
    fn a_whole_number_in_u32() {
        assert_eq!(whole(0.0), Some(0));
        assert_eq!(whole(1.0), Some(1));
        assert_eq!(whole(4_294_967_295.0), Some(u32::MAX));
        assert_eq!(whole(4_294_967_296.0), None);
        assert_eq!(whole(-1.0), None);
        assert_eq!(whole(0.5), None);
        assert_eq!(whole(f64::NAN), None);
    }

    #[test]
    fn records_with_other_values_only() {
        let record = |id, value| Record { id, dict: 7, value, slot: 3 };
        assert!(same_records(&[record(1, 0)], &[record(1, 5)]));
        assert!(!same_records(&[record(1, 0)], &[record(2, 0)]));
        assert!(!same_records(&[record(1, 0)], &[]));
        assert!(!same_records(&[record(1, 0)], &[Record { slot: 6, ..record(1, 0) }]));
    }
}
