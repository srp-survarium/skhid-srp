//! What the lobby does to one item: its cosmetics ("upgrades": a
//! camouflage and decals), changing a modification ("reroll"), upgrading it
//! with a donor ("craft"), and what a
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
//!   (`super::wire::item_descr`). The client never writes a weapon's
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
//!   owner's rule of 2026-10-22] At any time: the
//!   game's six hours between silver changes are gone, so the item's
//!   `reroll_time` the client is told is 0, and none is kept. The prices
//!   are `reroll.options`' by item level, [srp, the owner's rule of
//!   2026-10-25] the same for every change: no growth with the changes
//!   made (`docs/notes/flat-reroll.md`,
//!   [`Reroll::price`](survarium::v026g::modifications::Reroll::price)).
//!   [the owner, 2026-11-08] Silver only: the game's gold price in silver.
//!   [srp] The payment is taken when the three are offered, whatever is
//!   chosen; until the choice the item keeps the old one, and an offer not
//!   answered (the client went away, a restart: it is saved) is kept and
//!   shown again, with nothing more to pay.
//! - **Craft.** The item and a donor: the item with fewer than three
//!   modifications and no fewer than the donor (`MessageCraftItem`). The
//!   price is `craft.options`' by the item's level, [the owner, 2026-11-08]
//!   the game's gold price in silver. [the owner, 2026-11-08: "make it 100%
//!   always"] It always adds a modification (`st_item_craft_success_label`);
//!   the game's chances, one fewer and "nothing happened" are gone. The
//!   donor is used up and the price paid (the answer names it, and the
//!   client drops it).
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
//!   made ([`Account::add_item`]); it rerolls, crafts and counts for the
//!   rank as any modification.
//! - **Premium armour** ([`golden::armour_bonuses`], the owner's rule of
//!   2026-10-22): the same, with its two premium bonuses (+5 % experience,
//!   +5 % reputation with its faction) as its own first modifications;
//!   its plain piece and it are donors for each other as a golden weapon
//!   and its plain weapon are.
//! - **No repair, no wear, no defects** (the owner, 2026-11-08: "we don't
//!   have repair in the game at all"): `47 03` is no request, a match wears
//!   nothing and no item gets a defect, so every item stays whole.
//! - **Drops.** A dropped weapon or armour of rarity `n` has `n`
//!   modifications (the lobby's rank is their count), fewer when the item
//!   takes fewer types.
//! - **No reliability rolls** ([srp, the owner's rule of 2026-10-23],
//!   `docs/notes/no-reliability-roll.md`): no drop, upgrade or reroll offer
//!   gives a modification that only changes wear (items wear nothing since
//!   #35); one an item has stays, and rerolls into another. Every live item
//!   rolls three types or more (`armor.tsv`'s masks); an upgrade of
//!   an item with nothing left to roll is still refused, nothing taken.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::drops::Rng;
use survarium::v026g::golden;
use survarium::v026g::item_params::item_params;
use survarium::v026g::lobby::requests::KEEP_OLD_MODIFICATION;
use survarium::v026g::lobby::{Crafted, limits};
use survarium::v026g::modifications::{self, Bonuses, MAX_RANK, Modifications};
use survarium::v026g::scopes::ScopeFits;
use survarium::v026g::weapon_kit;

use super::account::{Account, Item, Upgrade};

/// Why an item action is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemError {
    /// A cosmetic can not go on, come off or be placed; why, for the log.
    Upgrade(&'static str),
    /// A modification can not be changed or chosen; why, for the log.
    Reroll(&'static str),
    NotEnoughMoney,
}

impl ItemError {
    /// The game's own text for the refusal: a key of its localization.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::Upgrade(_) => "st_cannot_upgrade_item",
            Self::Reroll(_) => "st_cannot_select_rerolled_modification",
            Self::NotEnoughMoney => "st_not_enougth_money",
        }
    }
}

impl std::fmt::Display for ItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?} ({})", self.text_key())
    }
}

impl std::error::Error for ItemError {}

// ------------------------------------------------------------ the item

/// The item's own modifications (not its defects, not its defaults).
pub fn own_modifications(item: &Item) -> Vec<u32> {
    item.modifications
        .iter()
        .filter(|&&(_, state)| !state)
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
    /// What the next change costs, in silver.
    pub cost: u32,
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
        cost: reroll.price(level),
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
    /// The silver taken: 0 for an offer made before and shown again.
    pub paid: u32,
    /// The modification the three are offered for: the one asked for, or
    /// the pending offer's when it is shown again.
    pub replaced: u32,
    /// The pending offer shown again (nothing was rolled or taken).
    pub again: bool,
}

/// `46 07 <item> <modification>`: change `modification` of the
/// item, any of its own (the one changed before locks nothing: the
/// owner's rule of 2026-10-22); three are offered in its place.
pub fn reroll(
    account: &mut Account,
    item_id: u32,
    modification: u32,
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
            paid: 0,
            replaced,
            again: true,
        });
    }
    if !is_gear(item) {
        return Err(ItemError::Reroll("not an item with modifications"));
    }
    let held = own_modifications(item);
    if !held.contains(&modification) {
        return Err(ItemError::Reroll("the item has no such modification"));
    }
    let table = Modifications::shared();
    let price = table.reroll.price(item_level(item.dict_id));
    if price > account.generic_money {
        return Err(ItemError::NotEnoughMoney);
    }
    let choices = table
        .reroll_choices(item.dict_id, &held, modification, rng)
        .ok_or(ItemError::Reroll("nothing to offer for the item"))?;
    account.generic_money -= price;
    let item = account.item_mut(item_id).expect("the item was found above");
    item.reroll.modification = modification;
    item.reroll.offer = Some((modification, choices));
    Ok(Rerolled {
        choices,
        reroll_seconds: 0,
        paid: price,
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

/// `46 09 <item> <donor>`: upgrade an item with a donor. `Err` is
/// why it is refused, for the log: the client is told with
/// [`Crafted::Failed`] (its window waits for an `83 46 09`). `queued`:
/// the account waits for a match (`st_craft_message_equipped_item`).
pub fn craft(
    account: &mut Account,
    item_id: u32,
    donor_item_id: u32,
    queued: bool,
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
    let price = table.craft.price(item_level(dict_id));
    if price > account.generic_money {
        return Err("not enough money");
    }
    // [the owner, 2026-11-08: "make it 100% always"] An upgrade always adds
    // a modification: the one rolled here. [srp, 2026-10-23] With nothing
    // left that may roll (every type besides reliability held; none on live
    // data, `armor.tsv`'s masks) no upgrade is made and nothing is taken.
    let modification = table
        .roll(dict_id, &held, rng)
        .ok_or("nothing left to roll for the item")?;

    account.generic_money -= price;
    account
        .inventory
        .retain(|item| item.item_id != donor_item_id);
    let item = account.item_mut(item_id).expect("the item was found above");
    item.modifications.push((modification, false));
    bump_kits_wearing(account, item_id);
    account.sync_kits();
    Ok(Crafted::Added {
        consumed_item_id: donor_item_id,
        modification,
    })
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
