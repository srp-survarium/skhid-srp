//! What the lobby does to one item: its cosmetics ("upgrades": a
//! camouflage and decals), changing a modification ("reroll"), upgrading it
//! with a donor ("craft"), the whole repair with its defects, and what a
//! match and a drop do to it. The data is the game's
//! ([`survarium::v026g::modifications`]); the requests and answers are in
//! `docs/notes/lobby_requests.md` (`46 01`, `02`, `05`..`09`,
//! `47 03`).
//!
//! What the client shows and checks itself (PROVEN, `lobby_menu.swf` and
//! the exe at the addresses the notes give) is kept as it is. The rest is
//! the server's, and where the data does not say, srp's own choice, marked
//! [srp]:
//!
//! - **Cosmetics.** A camouflage goes into slot 3 and a decal into slot 0
//!   or 1 of an item whose config has `camo_available` /
//!   `decal_available` (vests and trousers). The cosmetic stays an item of
//!   the inventory. [srp, the owner's rule of 2026-10-16] **One cosmetic
//!   is worn by any number of items at once**: each item that wears it
//!   holds a record of it (the upgrade's id, its dict id, its value, its
//!   slot), so each has its slot and a decal its own placement (`46 05`
//!   names the item). Putting it on an item leaves it on the others; on
//!   the same item in another slot it moves (the answer's `detach_first`,
//!   which patch 30 of `client-patches` makes take it off that item);
//!   taking it off (`46 02`, which patch 30 makes name the item) takes it
//!   off that item alone. A cosmetic that goes (sold, used up as a donor)
//!   comes off every item ([`drop_gone_upgrades`]). Nothing here is of the
//!   cosmetic's kind: the same holds for any upgrade in any slot.
//!   [srp] Any camouflage fits any such item: `items_compatibility` pairs
//!   none.
//! - **Scopes** ([`survarium::v026g::scopes`], `docs/notes/weapon_scopes.md`).
//!   [srp, the owner's rule of 2026-10-17] A scope item is bought once and
//!   worn like a cosmetic: by any number of weapons, each holding its own
//!   record, in slot 3 (a weapon's camouflage goes into 2), one scope a weapon
//!   (another replaces it). It goes only on a weapon the game data has a
//!   variant of it for, and the record holds **that variant's dict id**,
//!   not the scope item's: the client builds the weapon with the record's
//!   modification (0x2F15A2), and a variant's sets the weapon's scope.
//!   Taking it off brings a stock scope back (the plain config). The rest
//!   is the cosmetics' rule: off one weapon with `46 02`, off every weapon
//!   when it is sold ([`drop_gone_upgrades`]). After `83 46 01` the client's
//!   own copy holds the scope item's dict id (0x275671) until the inventory
//!   that follows every answer puts the variant's in its place.
//! - **A weapon's upgrades: the weapon kit** ([`survarium::v026g::weapon_kit`],
//!   `docs/notes/weapon-kit.md`, client patch 68). [srp, the owner's ask of
//!   2026-10-25] A weapon holds up to six, one a logical slot: decals in 0
//!   and 1, its camouflage in 2, its scope in 3, a muzzle device in 4 and a
//!   grip in 5 ([`upgrade_capacity`]); the account keeps them so, and the
//!   client gets them packed into at most three records
//!   (`super::wire::item_descr_at`). The client never writes a weapon's
//!   records itself: a change of one is answered with the inventory and the
//!   account only (no `83 46 01` or `83 46 02`, whose own copy of the
//!   record could be a fourth, the client's fatal assert).
//! - **Reroll.** The game's: one modification of an item may be changed,
//!   any number of times (`st_modifier_reroll_description`); the first
//!   change picks which (the window locks the others,
//!   `was_rerolled_before`). [srp, the owner's rule of 2026-10-22] Any of
//!   the item's own modifications may be changed, each any number of
//!   times; the one changed last is only marked in the window
//!   (`docs/notes/reroll-keep-old.md`). A change offers
//!   three modifications and the player takes one
//!   (`MessageRerollItem`: the window can not be closed before), or
//!   [srp, the owner's rule of 2026-10-21] keeps the old one: the window's
//!   Cancel button (`docs/notes/reroll-keep-old.md`). [srp, the
//!   owner's rule of 2026-10-22] In silver or gold, at any time: the
//!   game's six hours between silver changes are gone, so the item's
//!   `reroll_time` stays 0 (one left from before is cleared,
//!   [`finish_repairs`]). A damaged
//!   item can not be changed (`st_reroll_message_broken_item`). The prices
//!   are `reroll.options`' by item level, [srp, the owner's rule of
//!   2026-10-25] the same for every change: no growth with the changes
//!   made (`docs/notes/flat-reroll.md`,
//!   [`Reroll::price`](survarium::v026g::modifications::Reroll::price)).
//!   [srp] The payment is taken when the three are offered, whatever is
//!   chosen; until the choice the item keeps the old one, and an offer not
//!   answered (the client went away, a restart: it is saved) is kept and
//!   shown again, with nothing more to pay.
//! - **Craft.** The item and a donor: neither damaged nor in repair, the
//!   item with fewer than three modifications and no fewer than the donor
//!   (`MessageCraftItem`). The price is `craft.options`' by the item's
//!   level; the chances by how many more modifications the item has than
//!   the donor, in silver or in gold: one more modification
//!   (`st_item_craft_success_label`), one fewer
//!   (`st_item_craft_fail_label`), or nothing. [srp, the owner's rule of
//!   2026-10-23] Never one fewer: a failure is a "nothing", the item keeps
//!   its modifications, 0x9B says 0 % (`wire::craft_tables`) and the
//!   window shows no failure row. The donor is used up and the price paid
//!   whatever happens (the answer names it, and the client drops it).
//!   [LIKELY] The donor is another of the same item: the client's result
//!   window shows the *donor's* dictionary entry as the upgraded item
//!   (0x275B84 → 0x29F770 `dict_id`). [srp, the owner's rule of
//!   2026-10-21] A golden weapon takes its plain weapon, or a golden
//!   weapon of the same plain weapon, as its donor and stays golden (the
//!   lobby shows the item, not the donor); [the owner's rule of
//!   2026-10-22] a plain weapon takes a golden one too, which is used up
//!   ([`golden::may_donate`]).
//!   [srp] The modification gained is rolled as a drop's are.
//! - **Golden weapons** ([`golden`], the owner's rule of 2026-10-21): the
//!   silver bonus is the item's own first modification, given when it is
//!   made ([`Account::add_item`]) and once to those owned before
//!   ([`give_golden_silver`]); from then on it rerolls, crafts and counts
//!   for the rank as any modification.
//! - **Premium armour** ([`golden::PREMIUM_ARMOUR`], the owner's rule of
//!   2026-10-22): the same, with its two premium bonuses (+5 % experience,
//!   +5 % reputation with its faction) as its own first modifications,
//!   given once to those owned before ([`give_premium_armour_bonuses`]);
//!   its plain piece and it are donors for each other as a golden weapon
//!   and its plain weapon are.
//! - **Repair.** In silver: what the window shows,
//!   `(1 - condition) * repair_cost_ratio * cost` and each defect's
//!   `fix_cost_general`, and the item is locked for the defects'
//!   `fix_time_general` together (six minutes each); with no defect it is
//!   done at once. In gold ("quick repair"): the defects' `fix_cost_gold`,
//!   at once. With repair kits (the voucher `repair_voucher`): one a
//!   defect, at once. The client offers gold and kits only for an item with
//!   defects. Every repair makes the item whole (the client sets its
//!   condition to full on the answer) and [srp] takes its defects off at
//!   once: the wait only locks the item in the lobby. When it ends the
//!   client is told (`#ir[<item>]`, an important message of type 6).
//! - **Defects.** [LIKELY, the reading of `defect_slots` in
//!   `modifications`] After a match that wore an item, one roll for its
//!   first free defect slot. `SRP_ITEM_DEFECTS=off` switches them off.
//! - **Drops.** A dropped weapon or armour of rarity `n` has `n`
//!   modifications (the lobby's rank is their count), fewer when the item
//!   takes fewer types.
//! - **No reliability rolls** ([srp, the owner's rule of 2026-10-23],
//!   `docs/notes/no-reliability-roll.md`): no drop, upgrade or reroll offer
//!   gives a modification that only changes wear (items wear nothing since
//!   #35); one an item has stays, and rerolls into another. Every live item
//!   rolls three types or more (`third-mod-1.json`); an upgrade of
//!   an item with nothing left to roll is still refused, nothing taken.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::drops::Rng;
use survarium::v026g::golden;
use survarium::v026g::item_params::item_params;
use survarium::v026g::lobby::requests::{KEEP_OLD_MODIFICATION, RepairPayment};
use survarium::v026g::lobby::{Crafted, FULL_CONDITION, Repaired, limits};
use survarium::v026g::modifications::{self, Bonuses, CraftOutcome, MAX_RANK, Modifications};
use survarium::v026g::prices;
use survarium::v026g::scopes::ScopeFits;
use survarium::v026g::weapon_kit;

use super::account::{Account, Item, Upgrade};
use super::rules::shown_condition;

/// The important message that tells the client a repair is over
/// (`social.md` §1.4, type 6; the text is `#ir[<item id>]`, 0x226848).
pub const REPAIRED_MESSAGE: u8 = 6;
/// The setting that switches defects off.
pub const DEFECTS_VAR: &str = "SRP_ITEM_DEFECTS";

/// Why an item action is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemError {
    /// A cosmetic can not go on, come off or be placed; why, for the log.
    Upgrade(&'static str),
    /// A modification can not be changed or chosen; why, for the log.
    Reroll(&'static str),
    /// The item has defects.
    RerollBroken,
    NotEnoughMoney,
    NotEnoughGold,
    /// Nothing to repair, or not that way; why, for the log.
    Repair(&'static str),
    /// Fewer repair kits than defects.
    NoRepairKit,
    NotEnoughMoneyForRepair,
}

impl ItemError {
    /// The game's own text for the refusal: a key of its localization.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::Upgrade(_) => "st_cannot_upgrade_item",
            Self::Reroll(_) => "st_cannot_select_rerolled_modification",
            Self::RerollBroken => "st_reroll_message_broken_item",
            Self::NotEnoughMoney => "st_not_enougth_money",
            Self::NotEnoughGold => "st_cant_buy_premium_message",
            Self::Repair(_) => "st_cannot_repair_item",
            Self::NoRepairKit => "st_you_have_no_voucher_for_repair",
            Self::NotEnoughMoneyForRepair => "st_not_enought_money_for_repair",
        }
    }
}

impl std::fmt::Display for ItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?} ({})", self.text_key())
    }
}

impl std::error::Error for ItemError {}

/// Whether defects appear ([`DEFECTS_VAR`]; on unless `off`, `0`, `no` or
/// `false`).
pub fn defects_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| crate::switch(DEFECTS_VAR, true))
}

// ------------------------------------------------------------ the item

/// The item's own modifications (not its defects, not its defaults).
pub fn own_modifications(item: &Item) -> Vec<u32> {
    item.modifications
        .iter()
        .filter(|&&(_, state)| !state)
        .map(|&(id, _)| id)
        .collect()
}

/// The item's defects.
pub fn defects(item: &Item) -> Vec<u32> {
    item.modifications
        .iter()
        .filter(|&&(_, state)| state)
        .map(|&(id, _)| id)
        .collect()
}

/// The rank the lobby shows: how many modifications of its own it has.
pub fn rank(item: &Item) -> usize {
    item.modifications
        .iter()
        .filter(|&&(_, state)| !state)
        .count()
}

/// The seconds from `now` until the Unix second `until`; 0 once it is
/// over, or for none.
pub fn seconds_left(until: u32, now: u64) -> u32 {
    u64::from(until)
        .saturating_sub(now)
        .min(u64::from(u32::MAX)) as u32
}

fn until(now: u64, seconds: u32) -> u32 {
    now.saturating_add(u64::from(seconds))
        .min(u64::from(u32::MAX)) as u32
}

/// Takes off every upgrade whose cosmetic the inventory lacks
/// (`Account::sync_kits` calls it after each change of the inventory), by
/// the game data's scopes ([`drop_gone_upgrades_in`]). Returns the ids of
/// the items that lost one.
pub fn drop_gone_upgrades(inventory: &mut [Item]) -> Vec<u32> {
    drop_gone_upgrades_in(inventory, ScopeFits::shared())
}

/// [`drop_gone_upgrades`] with `scopes`: an upgrade comes off when its
/// cosmetic or scope is no item of the inventory, or when its record is not
/// the one that item makes on the host (a scope's: the variant made for
/// that weapon; [`modifications::record_dict_in`]).
pub fn drop_gone_upgrades_in(inventory: &mut [Item], scopes: &ScopeFits) -> Vec<u32> {
    let mut stripped = Vec::new();
    if inventory.iter().all(|item| item.upgrades.is_empty()) {
        return stripped;
    }
    let owned: Vec<(u32, u16)> = inventory
        .iter()
        .map(|item| (item.item_id, item.dict_id))
        .collect();
    for item in inventory.iter_mut() {
        let before = item.upgrades.len();
        let (host, host_dict) = (item.item_id, item.dict_id);
        item.upgrades.retain(|upgrade| {
            upgrade.id != host
                && owned.iter().any(|&(id, dict_id)| {
                    id == upgrade.id
                        && upgrade.a
                            == u32::from(modifications::record_dict_in(scopes, host_dict, dict_id))
                })
        });
        if item.upgrades.len() != before {
            stripped.push(host);
        }
    }
    stripped
}

/// The upgrade records of `before` that `after` (the same account,
/// repaired) no longer has, each as `(host item id, record, why)`: for the
/// log of a load ([`super::GameAccounts::with_repository`]).
pub fn upgrades_taken_off(before: &Account, after: &Account) -> Vec<(u32, Upgrade, &'static str)> {
    let dictionary = Dictionary::shared();
    let mut gone = Vec::new();
    for item in &before.inventory {
        let kept = after.item(item.item_id).map(|kept| &kept.upgrades);
        for record in &item.upgrades {
            if kept.is_some_and(|kept| kept.contains(record)) {
                continue;
            }
            let known = u16::try_from(record.a)
                .ok()
                .is_some_and(|dict_id| dictionary.item(dict_id).is_some());
            let why = if !known {
                "its dict id is no longer in the game data"
            } else if after.item(record.id).is_none() {
                "its upgrade item is gone"
            } else {
                "it does not fit the item (a withdrawn scope pair, or another slot)"
            };
            gone.push((item.item_id, *record, why));
        }
    }
    gone
}

/// Makes a loaded item one the client can be sent: no modification or
/// defect its tables lack (it reads the row it did not find), no more of
/// its own than three, no offer of what it lacks, and upgrades that are in
/// a slot of their kind, one a slot; by the game data's scopes
/// ([`make_safe_in`]).
pub fn make_safe(item: &mut Item) {
    make_safe_in(item, ScopeFits::shared());
}

/// [`make_safe`] with `scopes`: a scope's record must hold a variant made
/// for the item ([`modifications::record_slots_in`]).
pub fn make_safe_in(item: &mut Item, scopes: &ScopeFits) {
    let table = Modifications::shared();
    item.modifications
        .retain(|&(id, state)| table.is_known(id, state));
    let mut seen: Vec<(u32, bool)> = Vec::new();
    item.modifications.retain(|entry| {
        let fresh = !seen.contains(entry);
        seen.push(*entry);
        fresh
    });
    let mut own = 0;
    item.modifications.retain(|&(_, state)| {
        own += usize::from(!state);
        state || own <= MAX_RANK
    });
    let held = own_modifications(item);
    if let Some((replaced, choices)) = item.reroll.offer {
        let good =
            held.contains(&replaced) && choices.iter().all(|&choice| table.is_known(choice, false));
        if !good {
            item.reroll.offer = None;
        }
    }
    if item.reroll.modification != 0 && !held.contains(&item.reroll.modification) {
        item.reroll.modification = 0;
    }
    let host = item.dict_id;
    let mut slots: Vec<u8> = Vec::new();
    item.upgrades.retain(|upgrade| {
        let dict_id = u16::try_from(upgrade.a).unwrap_or(0);
        let fits = modifications::record_slots_in(scopes, host, dict_id).contains(&upgrade.param)
            && !slots.contains(&upgrade.param);
        slots.push(upgrade.param);
        fits
    });
}

/// An item of `dict_id` is a weapon (`wpn_*`): its upgrades are the
/// weapon kit's ([`upgrade_capacity`], [`survarium::v026g::weapon_kit`]).
pub fn is_weapon(dict_id: u16) -> bool {
    item_params(dict_id).is_some_and(|params| modifications::is_weapon_kind(params.kind))
}

/// The upgrades an item of `dict_id` may hold: a weapon's six logical
/// slots, which go to the client packed into three records
/// ([`survarium::v026g::weapon_kit::pack`]); any other item's three
/// records ([`limits::UPGRADES`], the client's).
pub fn upgrade_capacity(dict_id: u16) -> usize {
    if is_weapon(dict_id) {
        weapon_kit::WEAPON_SLOTS.len()
    } else {
        limits::UPGRADES
    }
}

/// The kits wearing `item_id` show another item: new revisions.
fn bump_kits_wearing(account: &mut Account, item_id: u32) {
    for kit in &mut account.kits {
        if kit.slot_of(item_id).is_some() {
            kit.revision = kit.revision.wrapping_add(1);
        }
    }
}

fn is_gear(item: &Item) -> bool {
    !Dictionary::shared().is_stack(item.dict_id)
        && modifications::item_mods(item.dict_id).is_some_and(|mods| mods.modifications != 0)
}

fn item_level(dict_id: u16) -> u8 {
    item_params(dict_id).map_or(0, |params| params.item_level)
}

// ----------------------------------------------------------- cosmetics

/// What `83 46 01` carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attached {
    pub host_item_id: u32,
    pub upgrade_item_id: u32,
    pub slot: u8,
    pub value: u64,
    /// The cosmetic was on the host in another slot and came off it
    /// first (the answer's `detach_first`: patch 30 takes it off the host,
    /// the stock client off the first item holding it).
    pub detach_first: bool,
}

/// `46 01 <kit> <host> <upgrade> <slot> <u64 0>`: put a camouflage, a
/// decal or a scope on an item, by the game data's scopes
/// ([`attach_upgrade_in`]).
pub fn attach_upgrade(
    account: &mut Account,
    host_item_id: u32,
    upgrade_item_id: u32,
    slot: u8,
) -> Result<Attached, ItemError> {
    attach_upgrade_in(
        account,
        ScopeFits::shared(),
        host_item_id,
        upgrade_item_id,
        slot,
    )
}

/// [`attach_upgrade`] with `scopes`. What was in the slot comes off the
/// item; the cosmetic stays on every other item wearing it. On the item
/// already in that slot it stays as it is (its value too); in another slot
/// of it, it moves, its value (a decal's place) with it. A scope's record holds the variant made for the weapon
/// ([`modifications::record_dict_in`]).
pub fn attach_upgrade_in(
    account: &mut Account,
    scopes: &ScopeFits,
    host_item_id: u32,
    upgrade_item_id: u32,
    slot: u8,
) -> Result<Attached, ItemError> {
    if host_item_id == upgrade_item_id {
        return Err(ItemError::Upgrade("an item on itself"));
    }
    let host = account
        .item(host_item_id)
        .ok_or(ItemError::Upgrade("no such item"))?;
    let upgrade = account
        .item(upgrade_item_id)
        .ok_or(ItemError::Upgrade("no such cosmetic"))?;
    let upgrade_dict = upgrade.dict_id;
    if !modifications::upgrade_slots_in(scopes, host.dict_id, upgrade_dict).contains(&slot) {
        return Err(ItemError::Upgrade("it does not go into that slot"));
    }
    let record_dict = modifications::record_dict_in(scopes, host.dict_id, upgrade_dict);
    // What the host keeps: all but what is in the slot and the cosmetic
    // itself (moved from another slot).
    let kept = host
        .upgrades
        .iter()
        .filter(|held| held.param != slot && held.id != upgrade_item_id)
        .count();
    if kept >= upgrade_capacity(host.dict_id) {
        return Err(ItemError::Upgrade("the item holds no more"));
    }
    let host = account
        .item_mut(host_item_id)
        .expect("the host was found above");
    if let Some(held) = host.upgrades.iter().find(|held| {
        held.id == upgrade_item_id && held.param == slot && held.a == u32::from(record_dict)
    }) {
        return Ok(Attached {
            host_item_id,
            upgrade_item_id,
            slot,
            value: held.value,
            detach_first: false,
        });
    }
    // Moved from another slot of the host: its place goes with it (the
    // answer carries it, and the client's record takes it from there).
    let value = host
        .upgrades
        .iter()
        .find(|held| held.id == upgrade_item_id && held.a == u32::from(record_dict))
        .map_or(0, |held| held.value);
    let before = host.upgrades.len();
    host.upgrades.retain(|held| held.id != upgrade_item_id);
    let detach_first = host.upgrades.len() != before;
    host.upgrades.retain(|held| held.param != slot);
    host.upgrades.push(Upgrade {
        id: upgrade_item_id,
        a: record_dict.into(),
        value,
        param: slot,
    });
    // Only the host shows another cosmetic: every kit wearing it, the
    // alternate kits sharing it too.
    bump_kits_wearing(account, host_item_id);
    Ok(Attached {
        host_item_id,
        upgrade_item_id,
        slot,
        value,
        detach_first,
    })
}

/// What `46 02` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detached {
    /// The cosmetic came off the host (the answer is `83 46 02`).
    Off,
    /// The cosmetic is no item of the inventory any more: Flash's
    /// `on_item_sold` sends `46 02` for a worn camouflage after it went
    /// (sold, used up as a donor), when it came off every item already.
    /// Nothing changes; it is off (the lobby answers `83 46 02`, which the
    /// client takes for an item it no longer has without a change).
    Gone,
}

/// `46 02 <kit> <upgrade> <host>`: take a cosmetic off one item; any
/// other item wearing it keeps it. A host of 0 is a client without patch
/// 30, which names no item (`46 02 <kit> <upgrade>`) and on the answer
/// takes the upgrade off the first item of its inventory that holds it
/// (0x275220, find_if 0x277CF0); the server takes it off that same item, the
/// first of the account's inventory (the order the inventory is sent in).
/// With the upgrade on several items that may not be the item the window
/// showed; client and server still agree.
///
/// On a weapon a cosmetic its kit names (the camouflage beside a scope) is
/// known to the client by its dict id alone: the window shows the
/// inventory's item of that dict id with the lowest item id
/// (`docs/notes/weapon-kit.md`). With two items of one camouflage that may
/// be another item than the one the weapon holds, so on a weapon an item
/// it does not hold takes off the one of the same dict id it does.
pub fn detach_upgrade(
    account: &mut Account,
    host_item_id: u32,
    upgrade_item_id: u32,
) -> Result<Detached, ItemError> {
    if account.item(upgrade_item_id).is_none() {
        return Ok(Detached::Gone);
    }
    let host_item_id = if host_item_id == 0 {
        account
            .inventory
            .iter()
            .find(|item| item.upgrades.iter().any(|held| held.id == upgrade_item_id))
            .map_or(0, |item| item.item_id)
    } else {
        host_item_id
    };
    let dict_id = account
        .item(upgrade_item_id)
        .map(|item| u32::from(item.dict_id));
    let host = account
        .item_mut(host_item_id)
        .ok_or(ItemError::Upgrade("the cosmetic is not on that item"))?;
    let held_id = if host.upgrades.iter().any(|held| held.id == upgrade_item_id) {
        Some(upgrade_item_id)
    } else if is_weapon(host.dict_id) {
        // a weapon's kit names this cosmetic by its dict id: the one of it
        host.upgrades
            .iter()
            .find(|held| Some(held.a) == dict_id)
            .map(|held| held.id)
    } else {
        None
    }
    .ok_or(ItemError::Upgrade("the cosmetic is not on that item"))?;
    host.upgrades.retain(|held| held.id != held_id);
    bump_kits_wearing(account, host_item_id);
    Ok(Detached::Off)
}

/// `46 05 <kit> <host> <upgrade> <u64>`: the decal editor's value for a
/// cosmetic on an item (the client has written it into its own copy, and
/// expects no answer). Each item wearing the cosmetic has its own. True
/// when the host is a weapon: the client's copy is the inventory's, and
/// its lobby shows the weapon with the new place only when it is sent the
/// account (patch 47 bakes the decal when the weapon is built).
pub fn place_upgrade(
    account: &mut Account,
    host_item_id: u32,
    upgrade_item_id: u32,
    value: u64,
) -> Result<bool, ItemError> {
    let held = account
        .item_mut(host_item_id)
        .and_then(|host| {
            host.upgrades
                .iter_mut()
                .find(|held| held.id == upgrade_item_id)
        })
        .ok_or(ItemError::Upgrade("the cosmetic is not on that item"))?;
    held.value = value;
    bump_kits_wearing(account, host_item_id);
    let weapon = account
        .item(host_item_id)
        .is_some_and(|host| is_weapon(host.dict_id));
    Ok(weapon)
}

// -------------------------------------------------------------- reroll

/// What the reroll window of an item shows (`83 46 06`, and the tail of
/// `83 46 08`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RerollWindow {
    pub item_id: u32,
    /// What the next change costs, in silver and in gold.
    pub regular_cost: u32,
    pub premium_cost: u32,
    /// Once a modification was changed: the id of the one changed last
    /// (the window marks it; any may be changed) and the seconds until
    /// silver pays again, always 0 now.
    pub changed: Option<(u32, u32)>,
    /// A paid change that waits for its choice (`83 46 07` again).
    pub offer: Option<[u32; 3]>,
}

fn window_of(item: &Item) -> RerollWindow {
    let reroll = &Modifications::shared().reroll;
    let level = item_level(item.dict_id);
    RerollWindow {
        item_id: item.item_id,
        regular_cost: reroll.price(level, false),
        premium_cost: reroll.price(level, true),
        changed: (item.reroll.modification != 0).then_some((item.reroll.modification, 0)),
        offer: item.reroll.offer.map(|(_, choices)| choices),
    }
}

/// `46 06 <item>`: the reroll window was opened.
pub fn reroll_window(account: &Account, item_id: u32) -> Result<RerollWindow, ItemError> {
    let item = account
        .item(item_id)
        .ok_or(ItemError::Reroll("no such item"))?;
    Ok(window_of(item))
}

/// What a reroll did (`83 46 07`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rerolled {
    pub choices: [u32; 3],
    /// The item's `reroll_time` for the client: always 0 (no wait).
    pub reroll_seconds: u32,
    /// What was taken: silver, gold. Both 0 for an offer made before and
    /// shown again.
    pub paid: (u32, u32),
    /// The modification the three are offered for: the one asked for, or
    /// the pending offer's when it is shown again.
    pub replaced: u32,
    /// The pending offer shown again (nothing was rolled or taken).
    pub again: bool,
}

/// `46 07 <item> <modification> <gold>`: change `modification` of the
/// item, any of its own (the one changed before locks nothing: the
/// owner's rule of 2026-10-22); three are offered in its place.
pub fn reroll(
    account: &mut Account,
    item_id: u32,
    modification: u32,
    gold: bool,
    now: u64,
    rng: &mut Rng,
) -> Result<Rerolled, ItemError> {
    let item = account
        .item(item_id)
        .ok_or(ItemError::Reroll("no such item"))?;
    if let Some((replaced, choices)) = item.reroll.offer {
        // Paid for and not chosen yet: the same three, whichever
        // modification is named.
        return Ok(Rerolled {
            choices,
            reroll_seconds: 0,
            paid: (0, 0),
            replaced,
            again: true,
        });
    }
    if !is_gear(item) {
        return Err(ItemError::Reroll("not an item with modifications"));
    }
    if !defects(item).is_empty() {
        return Err(ItemError::RerollBroken);
    }
    if seconds_left(item.repair_time, now) != 0 {
        return Err(ItemError::Reroll("the item is in repair"));
    }
    let held = own_modifications(item);
    if !held.contains(&modification) {
        return Err(ItemError::Reroll("the item has no such modification"));
    }
    let table = Modifications::shared();
    let price = table.reroll.price(item_level(item.dict_id), gold);
    let balance = if gold {
        account.premium_money
    } else {
        account.generic_money
    };
    if price > balance {
        return Err(if gold {
            ItemError::NotEnoughGold
        } else {
            ItemError::NotEnoughMoney
        });
    }
    let choices = table
        .reroll_choices(item.dict_id, &held, modification, rng)
        .ok_or(ItemError::Reroll("nothing to offer for the item"))?;
    let paid = if gold {
        account.premium_money -= price;
        (0, price)
    } else {
        account.generic_money -= price;
        (price, 0)
    };
    let item = account.item_mut(item_id).expect("the item was found above");
    item.reroll.modification = modification;
    item.reroll.count = item.reroll.count.saturating_add(1);
    item.reroll.offer = Some((modification, choices));
    Ok(Rerolled {
        choices,
        reroll_seconds: 0,
        paid,
        replaced: modification,
        again: false,
    })
}

/// What a choice did (`83 46 08`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub old_modification: u32,
    /// The one the item has now: the old one when it was kept.
    pub new_modification: u32,
    /// The window as it is now.
    pub window: RerollWindow,
}

/// `46 08 <item> <old modification> <choice>`: take one of the three
/// offered; `choice` is its place among them (the client sends the index
/// of the row clicked, `MessageRerollItem.onMessageAccept`), or
/// [`KEEP_OLD_MODIFICATION`]: none, the item keeps the old one
/// (`docs/notes/reroll-keep-old.md`). Either way the offer is over; what
/// the change cost was paid when it was made ([`reroll`]).
pub fn choose_modifier(
    account: &mut Account,
    item_id: u32,
    old_modification: u32,
    choice: u32,
) -> Result<Chosen, ItemError> {
    let item = account
        .item_mut(item_id)
        .ok_or(ItemError::Reroll("no such item"))?;
    let (replaced, choices) = item
        .reroll
        .offer
        .ok_or(ItemError::Reroll("nothing was offered"))?;
    if replaced != old_modification {
        return Err(ItemError::Reroll("another modification is being changed"));
    }
    if choice == KEEP_OLD_MODIFICATION {
        // Nothing of the item changes: the offer goes, and the old one
        // stays the one changed last (it already is, since the change was
        // paid).
        item.reroll.offer = None;
        return Ok(Chosen {
            old_modification,
            new_modification: old_modification,
            window: window_of(item),
        });
    }
    let new_modification = *choices
        .get(choice as usize)
        .ok_or(ItemError::Reroll("no such choice"))?;
    let held = item
        .modifications
        .iter_mut()
        .find(|entry| **entry == (replaced, false))
        .ok_or(ItemError::Reroll("the item lost the modification"))?;
    held.0 = new_modification;
    item.reroll.modification = new_modification;
    item.reroll.offer = None;
    let window = window_of(item);
    bump_kits_wearing(account, item_id);
    Ok(Chosen {
        old_modification,
        new_modification,
        window,
    })
}

// --------------------------------------------------------------- craft

/// `46 09 <item> <donor> <gold>`: upgrade an item with a donor. `Err` is
/// why it is refused, for the log: the client is told with
/// [`Crafted::Failed`] (its window waits for an `83 46 09`). `queued`:
/// the account waits for a match (`st_craft_message_equipped_item`).
pub fn craft(
    account: &mut Account,
    item_id: u32,
    donor_item_id: u32,
    gold: bool,
    queued: bool,
    now: u64,
    rng: &mut Rng,
) -> Result<Crafted, &'static str> {
    if item_id == donor_item_id {
        return Err("an item is its own donor");
    }
    let item = account.item(item_id).ok_or("no such item")?;
    let donor = account.item(donor_item_id).ok_or("no such donor")?;
    if !is_gear(item) || !is_gear(donor) {
        return Err("not an item with modifications");
    }
    if !golden::may_donate(donor.dict_id, item.dict_id) {
        return Err("the donor is another item");
    }
    if !defects(item).is_empty() || !defects(donor).is_empty() {
        return Err("a damaged item");
    }
    if seconds_left(item.repair_time, now) != 0 || seconds_left(donor.repair_time, now) != 0 {
        return Err("an item in repair");
    }
    if item.reroll.offer.is_some() {
        return Err("a modification of the item is being changed");
    }
    let (held, donor_rank) = (own_modifications(item), rank(donor));
    if held.len() >= MAX_RANK || donor_rank >= MAX_RANK {
        return Err("the most modifications already");
    }
    if donor_rank > held.len() {
        return Err("the donor has more modifications");
    }
    let equipped = |id: u32| account.kits.iter().any(|kit| kit.slot_of(id).is_some());
    if queued && (equipped(item_id) || equipped(donor_item_id)) {
        return Err("an equipped item while waiting for a match");
    }
    let table = Modifications::shared();
    let dict_id = item.dict_id;
    // [srp, 2026-10-23] Nothing left that may roll (every type besides
    // reliability held; none on live data, `third-mod-1.json`): an upgrade
    // could gain nothing, so none is made and nothing is taken.
    if table.candidates(dict_id, &held).is_empty() {
        return Err("nothing left to roll for the item");
    }
    let price = table.craft.price(item_level(dict_id), gold);
    let balance = if gold {
        account.premium_money
    } else {
        account.generic_money
    };
    if price > balance {
        return Err("not enough money");
    }
    let at = rng.below(100_000) as f32 / 1000.0;
    let outcome = table
        .craft
        .outcome_at(held.len() - donor_rank, gold, at)
        .ok_or("no chances for that difference")?;
    // The modification gained, before anything changes. [srp, the owner's
    // rule of 2026-10-23] An upgrade never takes one away: the data's
    // failure ("-1 mod.") does what "nothing happened" does, the price and
    // the donor spent and the item as it was (`docs/notes/craft-menu.md`).
    let gained = match outcome {
        CraftOutcome::Success => table.roll(dict_id, &held, rng),
        CraftOutcome::Failure | CraftOutcome::Neutral => None,
    };

    if gold {
        account.premium_money -= price;
    } else {
        account.generic_money -= price;
    }
    account
        .inventory
        .retain(|item| item.item_id != donor_item_id);
    let item = account.item_mut(item_id).expect("the item was found above");
    let crafted = match gained {
        Some(modification) => {
            item.modifications.push((modification, false));
            Crafted::Added {
                consumed_item_id: donor_item_id,
                modification,
            }
        }
        None => Crafted::Nothing {
            consumed_item_id: donor_item_id,
        },
    };
    bump_kits_wearing(account, item_id);
    account.sync_kits();
    Ok(crafted)
}

// -------------------------------------------------------------- repair

/// What the repair window shows for the item: `(silver, gold, seconds,
/// defects)`.
pub fn repair_quote(item: &Item) -> (u32, u32, u32, usize) {
    let table = Modifications::shared();
    let wear = prices::repair_cost(
        Dictionary::shared(),
        item.dict_id,
        shown_condition(item.condition_or_stack),
    );
    let rows: Vec<_> = defects(item)
        .into_iter()
        .filter_map(|id| table.defect(id))
        .collect();
    let sum = |value: fn(&modifications::Defect) -> u32| {
        rows.iter()
            .fold(0_u32, |sum, row| sum.saturating_add(value(row)))
    };
    (
        wear.saturating_add(sum(|row| row.fix_cost_general)),
        sum(|row| row.fix_cost_gold),
        sum(|row| row.fix_time_general),
        rows.len(),
    )
}

/// `47 03 <item> <kind> [<kit item>]`: repair an item whole. Returns what
/// `83 47 03` carries.
pub fn repair(
    account: &mut Account,
    item_id: u32,
    payment: RepairPayment,
    now: u64,
) -> Result<Repaired, ItemError> {
    let dictionary = Dictionary::shared();
    let item = account
        .item(item_id)
        .ok_or(ItemError::Repair("no such item"))?;
    if dictionary.is_stack(item.dict_id) {
        return Err(ItemError::Repair("a stack"));
    }
    if seconds_left(item.repair_time, now) != 0 {
        return Err(ItemError::Repair("already in repair"));
    }
    let (silver, gold, seconds, broken) = repair_quote(item);
    if broken == 0 && item.condition_or_stack >= FULL_CONDITION {
        return Err(ItemError::Repair("nothing to repair"));
    }
    let repaired = match payment {
        RepairPayment::Silver => {
            if silver > account.generic_money {
                return Err(ItemError::NotEnoughMoneyForRepair);
            }
            account.generic_money -= silver;
            if seconds == 0 {
                Repaired::AtOnce
            } else {
                Repaired::Timed {
                    repair_time: seconds,
                }
            }
        }
        // Gold and kits mend defects: the window offers neither without.
        RepairPayment::Gold => {
            if broken == 0 {
                return Err(ItemError::Repair("no defects to pay gold for"));
            }
            if gold > account.premium_money {
                return Err(ItemError::NotEnoughGold);
            }
            account.premium_money -= gold;
            Repaired::AtOnce
        }
        RepairPayment::Kit { kit_item_id } => {
            if broken == 0 {
                return Err(ItemError::Repair("no defects to use a kit on"));
            }
            let used = broken as u32;
            let index = account
                .inventory
                .iter()
                .position(|kit| {
                    kit.item_id == kit_item_id
                        && item_params(kit.dict_id).is_some_and(|params| params.kind == "vch_fixr")
                })
                .ok_or(ItemError::NoRepairKit)?;
            let kit = &mut account.inventory[index];
            if kit.condition_or_stack < used {
                return Err(ItemError::NoRepairKit);
            }
            kit.condition_or_stack -= used;
            if kit.condition_or_stack == 0 {
                account.inventory.remove(index);
            }
            Repaired::WithKit {
                kit_item_id,
                used: used as u8,
            }
        }
        RepairPayment::Other(_) => return Err(ItemError::Repair("no such payment")),
    };
    let item = account.item_mut(item_id).expect("the item was found above");
    item.condition_or_stack = FULL_CONDITION;
    item.modifications.retain(|&(_, state)| !state);
    item.repair_time = match repaired {
        Repaired::Timed { repair_time } => until(now, repair_time),
        _ => 0,
    };
    bump_kits_wearing(account, item_id);
    account.sync_kits();
    Ok(repaired)
}

/// The repairs that are over at `now`: their timers are cleared, and the
/// ids returned are what the client is told ([`repaired_notice`]).
pub fn finish_repairs(account: &mut Account, now: u64) -> Vec<u32> {
    let mut done = Vec::new();
    for item in &mut account.inventory {
        if item.repair_time != 0 && u64::from(item.repair_time) <= now {
            item.repair_time = 0;
            done.push(item.item_id);
        }
        // No wait between rerolls any more: one left from before goes.
        item.reroll_time = 0;
    }
    done
}

/// The important message for a repair that is over: `(type, text)`.
pub fn repaired_notice(item_id: u32) -> (u8, String) {
    (REPAIRED_MESSAGE, format!("#ir[{item_id}]"))
}

/// Makes every weapon and armour piece of the account whole: full
/// condition, no defect, no repair running. Returns how many needed it.
/// For the day wear was switched off (`pay::item_wear`): what matches had
/// worn until then would stay worn for ever.
pub fn mend_all(account: &mut Account) -> usize {
    let dictionary = Dictionary::shared();
    let mut mended = Vec::new();
    for item in &mut account.inventory {
        let gear = !dictionary.is_stack(item.dict_id)
            && item_params(item.dict_id).is_some_and(|params| {
                params.kind.starts_with("wpn_") || params.kind.starts_with("arm_")
            });
        let broken = item.modifications.iter().any(|&(_, state)| state);
        if !gear || (item.condition_or_stack >= FULL_CONDITION && !broken && item.repair_time == 0)
        {
            continue;
        }
        item.condition_or_stack = FULL_CONDITION;
        item.modifications.retain(|&(_, state)| !state);
        item.repair_time = 0;
        mended.push(item.item_id);
    }
    for &item_id in &mended {
        bump_kits_wearing(account, item_id);
    }
    if !mended.is_empty() {
        account.sync_kits();
    }
    mended.len()
}

/// What [`give_golden_silver`] or [`give_premium_armour_bonuses`] did to
/// one account.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoldenGrant {
    /// The items that got a bonus (all they lacked that fit) as their own
    /// first modifications.
    pub given: Vec<u32>,
    /// Items with no place left for a bonus they lack (three modifications
    /// of their own): a fourth would be cut at the next load
    /// (`make_safe`), so they keep theirs; for the log.
    pub full: Vec<u32>,
}

/// The golden weapons of the account (`survarium::v026g::golden`) get
/// the silver bonus as their own first modification, unless they hold a
/// modification of its type already. Once in the database's life, for the
/// golden weapons owned before the rule (their silver was a default of
/// the item's config, which the golden-weapons patch takes off).
pub fn give_golden_silver(account: &mut Account) -> GoldenGrant {
    let table = Modifications::shared();
    let silver_kind = table.modification(golden::SILVER_BONUS).map(|row| row.kind);
    let mut done = GoldenGrant::default();
    for item in &mut account.inventory {
        if !golden::is_golden_weapon(item.dict_id) {
            continue;
        }
        let held = own_modifications(item);
        let has_silver = held.iter().any(|&id| {
            id == golden::SILVER_BONUS || table.modification(id).map(|row| row.kind) == silver_kind
        });
        if has_silver {
            continue;
        }
        if held.len() >= MAX_RANK {
            done.full.push(item.item_id);
            continue;
        }
        item.modifications.insert(0, (golden::SILVER_BONUS, false));
        done.given.push(item.item_id);
    }
    for &item_id in &done.given {
        bump_kits_wearing(account, item_id);
    }
    if !done.given.is_empty() {
        account.sync_kits();
    }
    done
}

/// The premium armour pieces of the account (`survarium::v026g::golden::
/// PREMIUM_ARMOUR`) get each premium bonus they lack as their own
/// modification, first and in the table's order; a bonus is lacking when
/// the piece holds no modification of its type. Once in the database's
/// life, for the pieces owned before the rule (their bonuses were defaults
/// of the item's config, which the premium-armour patch takes off). A
/// piece with no place left for a bonus it lacks (three of its own) gets
/// those that fit and is listed in `full`.
pub fn give_premium_armour_bonuses(account: &mut Account) -> GoldenGrant {
    let table = Modifications::shared();
    let kind = |id: u32| table.modification(id).map(|row| row.kind);
    let mut done = GoldenGrant::default();
    for item in &mut account.inventory {
        let Some(bonuses) = Dictionary::shared()
            .item(item.dict_id)
            .and_then(|entry| golden::armour_bonuses(&entry.name))
        else {
            continue;
        };
        let mut held = own_modifications(item);
        let (mut given, mut short) = (0, false);
        for &bonus in bonuses {
            let lacking = !held
                .iter()
                .any(|&id| id == bonus || (kind(id).is_some() && kind(id) == kind(bonus)));
            if !lacking {
                continue;
            }
            if held.len() >= MAX_RANK {
                short = true;
                continue;
            }
            item.modifications.insert(given, (bonus, false));
            held.push(bonus);
            given += 1;
        }
        if given > 0 {
            done.given.push(item.item_id);
        }
        if short {
            done.full.push(item.item_id);
        }
    }
    for &item_id in &done.given {
        bump_kits_wearing(account, item_id);
    }
    if !done.given.is_empty() {
        account.sync_kits();
    }
    done
}

// ------------------------------------------------- matches and drops

/// Gives a new item its modifications by the rarity a drop rolled: as many
/// as the rarity, those it came with (a golden weapon's silver bonus)
/// counted; returns the rank the lobby shows for it (the item's
/// `base_rarity_level` and its modifications, 3 at most).
pub fn roll_modifications(account: &mut Account, item_id: u32, rarity: u8, rng: &mut Rng) -> u8 {
    let Some(item) = account.item_mut(item_id) else {
        return 0;
    };
    let table = Modifications::shared();
    let mut held = own_modifications(item);
    while held.len() < usize::from(rarity).min(MAX_RANK) {
        let Some(id) = table.roll(item.dict_id, &held, rng) else {
            break;
        };
        held.push(id);
        item.modifications.push((id, false));
    }
    let base = item_params(item.dict_id).map_or(0, |params| params.base_rarity_level);
    (usize::from(base) + rank(item)).min(MAX_RANK) as u8
}

/// What the item's wear is multiplied by: its "reliability"
/// modifications (5 % to 25 % less).
pub fn wear_factor(account: &Account, item_id: u32) -> f32 {
    account.item(item_id).map_or(1.0, |item| {
        Modifications::shared().wear_factor(item.dict_id, &item.modifications, "degrade_per_match")
    })
}

/// The defect rolls for the items a match wore: `worn` is `(item id,
/// condition before, after)`. Returns `(item id, defect)` for each that
/// got one. Nothing with [`DEFECTS_VAR`] off.
pub fn after_wear(
    account: &mut Account,
    worn: &[(u32, u32, u32)],
    rng: &mut Rng,
) -> Vec<(u32, u32)> {
    if !defects_enabled() {
        return Vec::new();
    }
    roll_defects(account, worn, rng)
}

/// [`after_wear`] whatever the setting.
pub fn roll_defects(
    account: &mut Account,
    worn: &[(u32, u32, u32)],
    rng: &mut Rng,
) -> Vec<(u32, u32)> {
    let table = Modifications::shared();
    let mut broke = Vec::new();
    for &(item_id, before, after) in worn {
        if after >= before {
            continue;
        }
        let Some(item) = account.item_mut(item_id) else {
            continue;
        };
        if item.modifications.len() >= limits::MODIFICATIONS {
            continue;
        }
        let condition = after as f32 / FULL_CONDITION as f32;
        let rolled = table.roll_defect(
            item.dict_id,
            &own_modifications(item),
            &defects(item),
            condition,
            rng,
        );
        if let Some(defect) = rolled {
            item.modifications.push((defect, true));
            broke.push((item_id, defect));
        }
    }
    for &(item_id, _) in &broke {
        bump_kits_wearing(account, item_id);
    }
    broke
}

/// What the items of the kit at `kit_index` add to a match's pay (the
/// premium copies of weapons and armour: 10 % silver, 5 % experience, 5 %
/// reputation with one faction).
pub fn kit_bonuses(account: &Account, kit_index: usize) -> Bonuses {
    let table = Modifications::shared();
    let mut total = Bonuses::default();
    let Some(kit) = account.kits.get(kit_index) else {
        return total;
    };
    // An item in two slots is worn once.
    let mut seen: Vec<u32> = Vec::new();
    for slot in kit.slots.iter().flatten() {
        if seen.contains(&slot.item_id) {
            continue;
        }
        seen.push(slot.item_id);
        let Some(item) = account.item(slot.item_id) else {
            continue;
        };
        let bonuses = table.bonuses(item.dict_id, &item.modifications);
        total.money += bonuses.money;
        total.experience += bonuses.experience;
        for (sum, value) in total.reputation.iter_mut().zip(bonuses.reputation) {
            *sum += value;
        }
    }
    total
}

#[cfg(test)]
mod tests;
