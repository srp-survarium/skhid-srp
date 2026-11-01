//! What the 0.26g client's lobby actions do to its account
//! (`docs/notes/lobby_requests.md` §2). Each rule checks what
//! the client's own windows check, changes nothing when it refuses, and
//! refuses with an [`ActionError`] whose [`ActionError::text_key`] is the
//! game's own text for it (a localization key of 0.26g's
//! `localization.txt`), for the server's `84 <op> 00 <key>`.
//!
//! Where the rules come from:
//! - moves (`46 00`): 0.21d's rule, since the client sends the same list
//!   the same way (`apply_profile_items` 0x2A5850 first moves every ammo
//!   and pocket slot of the kit to the inventory, then the dialog's rows
//!   back; PROVEN). The ammo dialog sends no purchase of its own, so
//!   rounds wanted beyond the stack owned are bought here, by the clip.
//! - the shop (`47 00/01/03/04`): `survarium::v026g::prices` (what the
//!   client's own dialogs show).
//! - skills (`48`): `survarium::v026g::skills` (the skills window's rules).
//!
//! Not checked here, because the client's rule is not known well enough to
//! refuse by it: the 30 kg carry limit (which skills raise) and which ammo
//! goes with which weapon. The client's lobby checks both before it sends.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params::{covered_slots, item_params, shop_unit};
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation, RepairPayment, SHOP_SLOT};
use survarium::v026g::lobby::{
    Bought, CONDITION_UNIT, FULL_CONDITION, Repaired, ServicePrices, limits,
};
use survarium::v026g::prices;
use survarium::v026g::quests::FACTIONS;
use survarium::v026g::skills::{SkillError, SkillsTree};

use super::account::{Account, KIT_SLOTS, KitSlot, cap_name, doubled_pockets};

/// What a skills reset costs, in silver (0.21d's value) or in gold [choice:
/// the same at the exchange rate].
pub const REROLL_COST: u32 = 100;
pub const REROLL_COST_PREMIUM: u32 = 1;
/// Silver for one gold: 0.21d's rate (its web page sold gold at it).
pub const PREMIUM_EXCHANGE_RATE: u32 = 100;
/// Most items one purchase of an item that is no stack makes [choice,
/// 0.21d's].
pub const MAX_BOUGHT_AT_ONCE: u32 = 8;

/// The six prices the lobby is told (0x8D). The kit's price is 0: a kit
/// can not be bought in 0.26g at all (the lobby's `setProfileCost` does
/// nothing). A nickname change costs `premium::Settings::rename_cost`
/// gold (`names`); the clan's price is `clans`'.
pub fn service_prices() -> ServicePrices {
    ServicePrices {
        reroll: REROLL_COST,
        reroll_premium: REROLL_COST_PREMIUM,
        profile: 0,
        change_name: super::premium::settings().rename_cost,
        premium_exchange_rate: PREMIUM_EXCHANGE_RATE,
        clan_creation: super::clans::Config::shared().cost,
    }
}

/// Why an action is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    /// No kit with that id.
    NoSuchKit,
    /// The account may not use the kit ([`Account::kit_usable`]; today it
    /// may use every kit of its own).
    KitNotUsable,
    /// No such item in the inventory (or its dict id is not the one named).
    NoSuchItem,
    /// The item is not in the slot it is moved from.
    NotInSlot,
    /// The item does not go into that slot.
    WrongSlot,
    /// The shop does not sell the item (or not that seller).
    NotSold,
    /// The reputation with the seller is under what the item needs.
    SellerLevel {
        needs: u8,
        has: u8,
    },
    /// A count of 0, or more than one purchase makes.
    BadCount,
    NotEnoughMoney,
    NotEnoughPremiumMoney,
    /// The skills asked for break the tree's rules.
    Skills(SkillError),
    /// The skills asked for take points back (only a reset does that).
    SkillsLowered,
    /// A reset of a branch that is none, or with a payment that is none.
    BadReset,
    /// The account has no silver for the reset.
    NotEnoughMoneyForReset,
    /// The item is a stack or is not worn: nothing to repair.
    NothingToRepair,
    /// A repair paid in gold or with a kit: those fix defects, and srp's
    /// items have none yet.
    NoDefects,
    NotEnoughMoneyForRepair,
    /// Not one of the lobby's factions.
    BadFaction,
    /// The kit has no weapon.
    NoWeapon,
    /// A weapon of the kit has no ammo in its slots.
    NoAmmo,
    /// An item of the kit is worn in a slot another worn item covers.
    CoveredSlot,
    /// An item type would be in a second pocket of the kit: one pocket a
    /// type ([`crate::v026g::account::doubled_pockets`]). The game has no
    /// text of its own for it.
    PocketTaken,
}

impl ActionError {
    /// The game's own text for the refusal: a key of its localization,
    /// which the client localizes and shows.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::NoSuchKit
            | Self::KitNotUsable
            | Self::NoSuchItem
            | Self::NotInSlot
            | Self::WrongSlot
            | Self::CoveredSlot
            | Self::PocketTaken => "st_cannot_relocate_item",
            Self::NotSold | Self::SellerLevel { .. } | Self::BadCount | Self::BadFaction => {
                "st_cannot_buy_item"
            }
            Self::NotEnoughMoney => "st_not_enougth_money",
            Self::NotEnoughPremiumMoney => "st_cant_buy_premium_message",
            Self::Skills(SkillError::OverLevel { .. } | SkillError::TooManyPoints { .. }) => {
                "st_wrong_skill_points_count"
            }
            Self::Skills(_) | Self::SkillsLowered => "st_player_skills_check_failed",
            Self::BadReset => "st_cannot_reroll_skills",
            Self::NotEnoughMoneyForReset => "st_not_enougth_money_for_reroll",
            Self::NothingToRepair | Self::NoDefects => "st_cannot_repair_item",
            Self::NotEnoughMoneyForRepair => "st_not_enought_money_for_repair",
            Self::NoWeapon => "st_profile_check_failed_weapon",
            Self::NoAmmo => "st_profile_check_failed_weapon_ammo",
        }
    }
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?} ({})", self.text_key())
    }
}

impl std::error::Error for ActionError {}

fn slot_index(slot: u8) -> Option<usize> {
    let slot = usize::from(slot);
    (slot < KIT_SLOTS).then_some(slot)
}

/// The owned stack a relocation names: by its id, or by its item for id 0
/// (a stack the dialog shows but the player does not own yet).
fn owned_stack(account: &Account, relocation: &Relocation) -> Option<u32> {
    account
        .inventory
        .iter()
        .find(|item| {
            item.dict_id == relocation.dict_id
                && (relocation.item_id == 0 || item.item_id == relocation.item_id)
        })
        .map(|item| item.item_id)
}

/// `46 00`: move items between a kit's slots and the inventory, all of
/// them or none.
///
/// A stack moved into a slot names the rounds the slot takes (`count`) and
/// the rounds wanted in all (`count_in_inventory`); what that is beyond the
/// stack owned is bought first, by the clip, from whoever sells it
/// ([`top_up`]). A stack moved in with no rounds leaves its slot empty: the
/// client empties an ammo slot that way. A request that would leave an item
/// type in two pockets of the kit is refused ([`ActionError::PocketTaken`]).
pub fn move_items(
    account: &mut Account,
    kit_id: u32,
    moves: &[Relocation],
    now: u64,
) -> Result<(), ActionError> {
    let dictionary = Dictionary::shared();
    let mut changed = account.clone();
    let kit_index = changed.kit_index(kit_id).ok_or(ActionError::NoSuchKit)?;
    if !changed.kit_usable(kit_index, now) {
        return Err(ActionError::KitNotUsable);
    }
    let mut resolved = Vec::with_capacity(moves.len());
    for &relocation in moves {
        let mut relocation = relocation;
        let is_stack = dictionary
            .item(relocation.dict_id)
            .is_some_and(|item| item.is_stack);
        if is_stack && relocation.target_slot != INVENTORY_SLOT {
            let have = owned_stack(&changed, &relocation)
                .and_then(|item_id| changed.item(item_id))
                .map_or(0, |item| item.condition_or_stack);
            let wanted = relocation.count_in_inventory.max(relocation.count);
            if wanted > have {
                top_up(&mut changed, relocation.dict_id, wanted - have)?;
            }
            relocation.item_id =
                owned_stack(&changed, &relocation).ok_or(ActionError::NoSuchItem)?;
            // A stack dropped from the shop on a pocket: what was missing is
            // bought above, and goes in from the inventory.
            if relocation.source_slot == SHOP_SLOT {
                relocation.source_slot = INVENTORY_SLOT;
            }
        }
        resolved.push(relocation);
    }
    relocate(&mut changed, dictionary, kit_index, &resolved)?;
    changed.sync_kits();
    *account = changed;
    Ok(())
}

/// The kit at `kit_index` emptied, then `moves` (each from the inventory)
/// made into it by the moves' own rules ([`relocate`]): the item owned and
/// fitting the slot, one item in one slot of a kit, a covering item taking
/// off what it covers, one pocket an item type. Unlike [`move_items`] it
/// never buys: a stack gets at most the units owned. All or nothing; a kit
/// whose slots changed gets a new revision. A kit preset is put on with it
/// (`super::presets`).
pub fn fill_kit(
    account: &mut Account,
    kit_index: usize,
    moves: &[Relocation],
) -> Result<(), ActionError> {
    let before = account
        .kits
        .get(kit_index)
        .ok_or(ActionError::NoSuchKit)?
        .clone();
    account.kits[kit_index].slots = [None; KIT_SLOTS];
    let filled = relocate(account, Dictionary::shared(), kit_index, moves);
    let kit = &mut account.kits[kit_index];
    if filled.is_err() {
        *kit = before;
        return filled;
    }
    kit.revision = if kit.slots == before.slots {
        before.revision
    } else {
        before.revision.wrapping_add(1)
    };
    Ok(())
}

/// [`move_items`] with every item owned.
///
/// The moves are made in order, and a move may name as its source a slot
/// that an earlier move of the same request emptied of that item. The ammo
/// dialog asks that way: its request starts with every occupied ammo and
/// pocket slot going to the inventory (`apply_profile_items` 0x2A5850),
/// and its row for an item that was in a pocket slot then names that slot
/// as the source (`AmmoShareLite.relocateItem` sends the item's
/// `descriptor`, which is its slot: `LocalPlayerProfile.createItem`).
fn relocate(
    account: &mut Account,
    dictionary: &Dictionary,
    kit_index: usize,
    moves: &[Relocation],
) -> Result<(), ActionError> {
    relocate_covering(account, dictionary, kit_index, moves, &covered_slots)
}

/// [`relocate`] with the slots each item covers when worn given
/// ([`covered_slots`]).
///
/// An item that covers other slots (a whole suit worn in the back slot,
/// whose `data.additional_model_slots` are the helmet, mask, torso, legs,
/// gloves and boots) is worn alone in them: putting it on takes whatever is
/// in them off, to the inventory, and putting anything into one of them
/// takes the suit off. The client's paperdoll locks those cells while the
/// suit is worn (`InventoryItem`), so it does not ask otherwise. A slot so
/// emptied counts as taken by this request: a later move of the same
/// request may name it as the source of the item it held.
fn relocate_covering(
    account: &mut Account,
    dictionary: &Dictionary,
    kit_index: usize,
    moves: &[Relocation],
    covered: &dyn Fn(u16) -> &'static [u8],
) -> Result<(), ActionError> {
    let mut slots = account.kits[kit_index].slots;
    // The item this request took out of each slot.
    let mut taken = [None; KIT_SLOTS];
    for relocation in moves {
        let owned = account
            .inventory
            .iter()
            .find(|item| item.item_id == relocation.item_id && item.dict_id == relocation.dict_id)
            .ok_or(ActionError::NoSuchItem)?;
        let is_stack = dictionary
            .item(owned.dict_id)
            .ok_or(ActionError::NoSuchItem)?
            .is_stack;

        if relocation.source_slot != INVENTORY_SLOT {
            let source = slot_index(relocation.source_slot).ok_or(ActionError::WrongSlot)?;
            if slots[source].map(|slot| slot.item_id) == Some(owned.item_id) {
                slots[source] = None;
                taken[source] = Some(owned.item_id);
            } else if taken[source] != Some(owned.item_id) {
                return Err(ActionError::NotInSlot);
            }
        }

        if relocation.target_slot != INVENTORY_SLOT {
            let target = slot_index(relocation.target_slot).ok_or(ActionError::WrongSlot)?;
            if !dictionary.fits(owned.dict_id, relocation.target_slot) {
                return Err(ActionError::WrongSlot);
            }
            let count = if is_stack {
                // The rounds put in the slot, at most what is owned.
                if relocation.count == 0 {
                    continue;
                }
                relocation.count.min(owned.condition_or_stack)
            } else {
                // One item is in one slot of a kit.
                for slot in slots.iter_mut() {
                    if slot.is_some_and(|held| held.item_id == owned.item_id) {
                        *slot = None;
                    }
                }
                1
            };
            slots[target] = Some(KitSlot {
                item_id: owned.item_id,
                count,
            });
            for &slot in covered(owned.dict_id) {
                let slot = usize::from(slot);
                if slot != target
                    && let Some(held) = slots.get_mut(slot).and_then(Option::take)
                {
                    taken[slot] = Some(held.item_id);
                }
            }
            for other in 0..KIT_SLOTS {
                let Some(held) = slots[other].filter(|_| other != target) else {
                    continue;
                };
                let worn = account
                    .inventory
                    .iter()
                    .find(|item| item.item_id == held.item_id);
                if worn.is_some_and(|item| covered(item.dict_id).contains(&(target as u8))) {
                    slots[other] = None;
                    taken[other] = Some(held.item_id);
                }
            }
        }
    }
    // One pocket an item type, judged on what the whole request leaves: the
    // ammo window empties every pocket first and fills them again, and two
    // pockets changing places name each other's slot.
    let dict_of = |item_id| account.item(item_id).map(|item| item.dict_id);
    if !doubled_pockets(&slots, dict_of).is_empty() {
        return Err(ActionError::PocketTaken);
    }
    let kit = &mut account.kits[kit_index];
    if kit.slots != slots {
        kit.slots = slots;
        kit.revision = kit.revision.wrapping_add(1);
    }
    Ok(())
}

/// The most units of a free stack an account holds: the client's
/// `InventoryList.MAX_FREE_ITEMS_COUNT`.
pub const MAX_FREE_UNITS: u32 = 1000;

/// What a step (a clip of ammo, else one item) of `dict_id` costs whoever
/// may buy it: the price list's cost, or nothing for a stack whose
/// `base_cost` is 0 and which the shop therefore does not list (the
/// starting pistol's ammo) [choice: its price in the game's data is 0, so a
/// refill costs that].
fn step_cost(account: &Account, dictionary: &Dictionary, dict_id: u16) -> Result<u32, ActionError> {
    if let Some(record) = record_for(account, dict_id, 0)? {
        return Ok(record.cost);
    }
    match dictionary.item(dict_id) {
        Some(item) if item.is_stack && item.base_cost == 0 && !item.is_premium => Ok(0),
        _ => Err(ActionError::NotSold),
    }
}

/// The price record `account` buys `dict_id` by: a seller's whose
/// reputation level it has, `faction`'s if that one sells to it (an item
/// may have two sellers; seller 0 sells to everyone;
/// [`prices::record_for`]). `None` when the shop does not list the item.
fn record_for(
    account: &Account,
    dict_id: u16,
    faction: u8,
) -> Result<Option<&'static prices::PriceItem>, ActionError> {
    prices::record_for(dict_id, faction, |seller| account.reputation_level(seller)).map_err(
        |refused| ActionError::SellerLevel {
            needs: refused.level,
            has: account.reputation_level(refused.faction_id),
        },
    )
}

/// Adds `units` to the account's stack of `dict_id`, or makes the stack;
/// returns its id.
fn add_to_stack(account: &mut Account, dict_id: u16, units: u32) -> Result<u32, ActionError> {
    match account
        .inventory
        .iter_mut()
        .find(|item| item.dict_id == dict_id)
    {
        Some(owned) => {
            owned.condition_or_stack = owned
                .condition_or_stack
                .checked_add(units)
                .ok_or(ActionError::BadCount)?;
            Ok(owned.item_id)
        }
        None => Ok(account.add_item(dict_id, units)),
    }
}

/// Buys at least `missing` units of a stack, in whole steps: what the ammo
/// dialog asks for beyond the stack owned.
fn top_up(account: &mut Account, dict_id: u16, missing: u32) -> Result<(), ActionError> {
    let dictionary = Dictionary::shared();
    let unit = shop_unit(dict_id);
    let steps = missing.div_ceil(unit);
    let step_cost = step_cost(account, dictionary, dict_id)?;
    let cost = u64::from(step_cost) * u64::from(steps);
    if cost > u64::from(account.generic_money) {
        return Err(ActionError::NotEnoughMoney);
    }
    let mut units = steps.checked_mul(unit).ok_or(ActionError::BadCount)?;
    if step_cost == 0 {
        // What costs nothing is owned up to the client's own limit
        // (`InventoryList.MAX_FREE_ITEMS_COUNT`), not without end.
        let have = account
            .inventory
            .iter()
            .find(|item| item.dict_id == dict_id)
            .map_or(0, |item| item.condition_or_stack);
        units = units.min(MAX_FREE_UNITS.saturating_sub(have));
        if units == 0 {
            return Err(ActionError::BadCount);
        }
    }
    add_to_stack(account, dict_id, units)?;
    account.generic_money -= cost as u32;
    Ok(())
}

/// `47 00`: buy `count` steps of an item (a clip of ammo, else one item)
/// from `faction` (0: whoever lists it). With a `target_slot` that is a
/// kit's slot the first item bought goes into it (the client buys that way
/// when an item is dragged from the shop onto the character). Returns what
/// `83 47 00` lists.
pub fn buy(
    account: &mut Account,
    dict_id: u16,
    count: u32,
    faction: u8,
    target_slot: u8,
    kit_id: u32,
    now: u64,
) -> Result<Vec<Bought>, ActionError> {
    let dictionary = Dictionary::shared();
    let item = dictionary.item(dict_id).ok_or(ActionError::NotSold)?;
    // The client names a price record's own seller (0x288BC0 fills in the
    // first record's when the window gave none).
    let sellers = prices::sellers(dict_id);
    if sellers.is_empty() || (faction != 0 && sellers.iter().all(|r| r.faction_id != faction)) {
        return Err(ActionError::NotSold);
    }
    record_for(account, dict_id, faction)?;
    if count == 0 || (!item.is_stack && count > MAX_BOUGHT_AT_ONCE) {
        return Err(ActionError::BadCount);
    }
    let cost = prices::buy_cost(dict_id, count).ok_or(ActionError::NotSold)?;
    if cost > u64::from(account.generic_money) {
        return Err(ActionError::NotEnoughMoney);
    }

    let mut changed = account.clone();
    let bought: Vec<Bought> = if item.is_stack {
        let units = count
            .checked_mul(shop_unit(dict_id))
            .ok_or(ActionError::BadCount)?;
        let item_id = add_to_stack(&mut changed, dict_id, units)?;
        vec![Bought {
            dict_id,
            item_id,
            value: units,
        }]
    } else {
        (0..count)
            .map(|_| Bought {
                dict_id,
                item_id: changed.add_item(dict_id, FULL_CONDITION),
                value: FULL_CONDITION,
            })
            .collect()
    };
    changed.generic_money -= cost as u32;

    if target_slot != INVENTORY_SLOT {
        let kit_index = changed.kit_index(kit_id).ok_or(ActionError::NoSuchKit)?;
        if !changed.kit_usable(kit_index, now) {
            return Err(ActionError::KitNotUsable);
        }
        let first = bought[0];
        let relocation = Relocation {
            item_id: first.item_id,
            dict_id,
            source_slot: INVENTORY_SLOT,
            target_slot,
            count: if item.is_stack { first.value } else { 1 },
            count_in_inventory: 0,
        };
        relocate(&mut changed, dictionary, kit_index, &[relocation])?;
    }
    changed.sync_kits();
    *account = changed;
    Ok(bought)
}

/// What a sale did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sold {
    pub item_id: u32,
    /// The item's dictionary id.
    pub dict_id: u16,
    /// What `83 47 01` carries: the units taken off a stack; 1 for an item
    /// that is no stack.
    pub amount: u32,
    /// The silver paid.
    pub money: u32,
}

/// The condition the lobby shows: 0..=100.
pub fn shown_condition(condition: u32) -> u32 {
    ((condition + CONDITION_UNIT / 2) / CONDITION_UNIT).min(100)
}

/// `47 01`: sell `steps` of an item (clips of ammo; an item that is no
/// stack is sold whole). The pay is what the lobby shows
/// ([`prices::sell_price`]): a quarter of the cost a step, for a worn item
/// times its condition.
pub fn sell(account: &mut Account, item_id: u32, steps: u32) -> Result<Sold, ActionError> {
    let dictionary = Dictionary::shared();
    let index = account
        .inventory
        .iter()
        .position(|item| item.item_id == item_id)
        .ok_or(ActionError::NoSuchItem)?;
    let owned = &account.inventory[index];
    let dict_id = owned.dict_id;
    let is_stack = dictionary
        .item(owned.dict_id)
        .ok_or(ActionError::NoSuchItem)?
        .is_stack;
    let (amount, pay) = if is_stack {
        let asked = u64::from(steps) * u64::from(shop_unit(owned.dict_id));
        let amount = asked.min(owned.condition_or_stack.into()) as u32;
        if amount == 0 {
            return Err(ActionError::BadCount);
        }
        // A step that the stack's last rounds do not fill is still a step.
        let paid_for = amount.div_ceil(shop_unit(owned.dict_id)) * shop_unit(owned.dict_id);
        (
            amount,
            prices::sell_price(dictionary, owned.dict_id, paid_for, PREMIUM_EXCHANGE_RATE),
        )
    } else {
        (
            1,
            prices::sell_price(
                dictionary,
                owned.dict_id,
                shown_condition(owned.condition_or_stack),
                PREMIUM_EXCHANGE_RATE,
            ),
        )
    };
    if is_stack && amount < owned.condition_or_stack {
        account.inventory[index].condition_or_stack -= amount;
    } else {
        account.inventory.remove(index);
    }
    let money = pay.min(u64::from(u32::MAX)) as u32;
    account.generic_money = account.generic_money.saturating_add(money);
    account.sync_kits();
    Ok(Sold {
        item_id,
        dict_id,
        amount,
        money,
    })
}

/// `47 03`: repair an item to full. Paid in silver it costs what the
/// repair window shows ([`prices::repair_cost`]) and is done at once: the
/// window's time and its gold and kit payments are for an item's defects,
/// which srp's items do not have yet. Returns what `83 47 03` carries.
pub fn repair(
    account: &mut Account,
    item_id: u32,
    payment: RepairPayment,
) -> Result<Repaired, ActionError> {
    let dictionary = Dictionary::shared();
    let owned = account.item(item_id).ok_or(ActionError::NoSuchItem)?;
    let is_stack = dictionary
        .item(owned.dict_id)
        .ok_or(ActionError::NoSuchItem)?
        .is_stack;
    if is_stack || owned.condition_or_stack >= FULL_CONDITION {
        return Err(ActionError::NothingToRepair);
    }
    if payment != RepairPayment::Silver {
        return Err(ActionError::NoDefects);
    }
    let cost = prices::repair_cost(
        dictionary,
        owned.dict_id,
        shown_condition(owned.condition_or_stack),
    );
    if cost > account.generic_money {
        return Err(ActionError::NotEnoughMoneyForRepair);
    }
    account.generic_money -= cost;
    if let Some(item) = account.item_mut(item_id) {
        item.condition_or_stack = FULL_CONDITION;
        item.repair_time = 0;
    }
    Ok(Repaired::AtOnce)
}

/// Wears an item that is no stack by `fraction` of its full condition (the
/// item configs' `degrade_per_match` and the like): for the results of a
/// match. Returns the new condition.
pub fn wear(account: &mut Account, item_id: u32, fraction: f32) -> Option<u32> {
    let dictionary = Dictionary::shared();
    let item = account.item_mut(item_id)?;
    if dictionary.is_stack(item.dict_id) || !fraction.is_finite() || fraction <= 0.0 {
        return Some(item.condition_or_stack);
    }
    let lost = (fraction.min(1.0) * FULL_CONDITION as f32) as u32;
    item.condition_or_stack = item.condition_or_stack.saturating_sub(lost);
    Some(item.condition_or_stack)
}

/// The wear a match puts on an item (`degrade_per_match`), as a fraction.
pub fn wear_per_match(dict_id: u16) -> f32 {
    item_params(dict_id).map_or(0.0, |params| params.degrade_per_match)
}

/// `47 04`: gold for silver at the rate the lobby was told. Returns the
/// silver gained.
pub fn exchange_premium(account: &mut Account, gold: u32) -> Result<u32, ActionError> {
    if gold == 0 {
        return Err(ActionError::BadCount);
    }
    if gold > account.premium_money {
        return Err(ActionError::NotEnoughPremiumMoney);
    }
    let silver = gold
        .checked_mul(PREMIUM_EXCHANGE_RATE)
        .and_then(|silver| account.generic_money.checked_add(silver).map(|_| silver))
        .ok_or(ActionError::BadCount)?;
    account.premium_money -= gold;
    account.generic_money += silver;
    Ok(silver)
}

/// `47 04` from srp's exchange window: silver for gold, at the same rate.
/// Returns the silver paid.
pub fn buy_premium(account: &mut Account, gold: u32) -> Result<u32, ActionError> {
    if gold == 0 {
        return Err(ActionError::BadCount);
    }
    let silver = gold
        .checked_mul(PREMIUM_EXCHANGE_RATE)
        .ok_or(ActionError::BadCount)?;
    if silver > account.generic_money {
        return Err(ActionError::NotEnoughMoney);
    }
    account.premium_money = account
        .premium_money
        .checked_add(gold)
        .ok_or(ActionError::BadCount)?;
    account.generic_money -= silver;
    Ok(silver)
}

/// `48 00`: the account's whole set of skills. The set must keep the
/// tree's rules ([`SkillsTree::validate`]) and take no point back: the
/// skills window only adds, a reset is `48 01`.
pub fn set_skills(account: &mut Account, skills: &[(u16, u8)]) -> Result<(), ActionError> {
    let tree = SkillsTree::shared();
    tree.validate(account.level(), skills)
        .map_err(ActionError::Skills)?;
    let points_in = |set: &[(u16, u8)], id: u16| {
        set.iter()
            .find(|&&(skill, _)| skill == id)
            .map_or(0, |&(_, points)| points)
    };
    if account
        .skills
        .iter()
        .any(|&(id, points)| points_in(skills, id) < points)
    {
        return Err(ActionError::SkillsLowered);
    }
    let mut kept: Vec<(u16, u8)> = skills
        .iter()
        .copied()
        .filter(|&(_, points)| points > 0)
        .collect();
    kept.sort_unstable();
    if kept != account.skills {
        account.skills = kept;
        // The kits carry the skills' modifiers.
        account.bump_kits();
    }
    Ok(())
}

/// How a skills reset is paid (`reroll_ok_clicked`'s second argument).
pub mod reset_payment {
    pub const SILVER: u8 = 0;
    pub const GOLD: u8 = 1;
}

/// `48 01`: take back the points of one branch of the tree (`branch` is
/// the skills window's tab: 0 for skills 101.., 1 for 201..), for
/// [`REROLL_COST`] silver or [`REROLL_COST_PREMIUM`] gold. Returns the
/// points freed.
pub fn reset_skills(account: &mut Account, branch: u8, payment: u8) -> Result<u32, ActionError> {
    if branch > 1 {
        return Err(ActionError::BadReset);
    }
    let in_branch = |id: u16| id / 100 == u16::from(branch) + 1;
    let freed: u32 = account
        .skills
        .iter()
        .filter(|&&(id, _)| in_branch(id))
        .map(|&(_, points)| u32::from(points))
        .sum();
    // The window's button is off with no points in the branch.
    if freed == 0 {
        return Err(ActionError::BadReset);
    }
    match payment {
        reset_payment::SILVER => {
            if account.generic_money < REROLL_COST {
                return Err(ActionError::NotEnoughMoneyForReset);
            }
            account.generic_money -= REROLL_COST;
        }
        reset_payment::GOLD => {
            if account.premium_money < REROLL_COST_PREMIUM {
                return Err(ActionError::NotEnoughPremiumMoney);
            }
            account.premium_money -= REROLL_COST_PREMIUM;
        }
        _ => return Err(ActionError::BadReset),
    }
    account.skills.retain(|&(id, _)| !in_branch(id));
    account.bump_kits();
    Ok(freed)
}

/// `49 00`: name a kit; an empty name gives it the client's own again.
pub fn rename_kit(account: &mut Account, kit_id: u32, name: &str) -> Result<(), ActionError> {
    let index = account.kit_index(kit_id).ok_or(ActionError::NoSuchKit)?;
    let name = cap_name(name.trim());
    let kit = &mut account.kits[index];
    if kit.name != name {
        kit.name = name;
        kit.revision = kit.revision.wrapping_add(1);
    }
    Ok(())
}

/// `49 01`: the account's autobuy box.
pub fn set_autobuy(account: &mut Account, on: bool) {
    account.autobuy = on;
}

/// `49 02`: the kit the account plays.
pub fn set_active_kit(account: &mut Account, kit_id: u32, now: u64) -> Result<(), ActionError> {
    let index = account.kit_index(kit_id).ok_or(ActionError::NoSuchKit)?;
    if !account.kit_usable(index, now) {
        return Err(ActionError::KitNotUsable);
    }
    account.active_kit = kit_id;
    Ok(())
}

/// The slots of the two weapons (the client's table at 0x9266A0).
const WEAPON_SLOTS: [usize; 2] = [7, 10];
/// The slots whose ammo counts for either weapon (the client's table at
/// 0x9266A8): the ammo is one pool, not two slots a weapon.
const AMMO_SLOTS: [usize; 8] = [8, 9, 11, 12, 19, 20, 21, 22];

/// The kit a match would take now: the active one when usable, else the
/// first ([`Account::settle_active_kit`], without changing the account).
pub fn kit_to_play(account: &Account, now: u64) -> u32 {
    if account
        .kit_index(account.active_kit)
        .is_some_and(|index| account.kit_usable(index, now))
    {
        account.active_kit
    } else {
        account
            .kits
            .first()
            .map_or(account.active_kit, |kit| kit.id)
    }
}

/// Why a kit can not play: each a bit of a [`PlayBlocks`] set
/// (`presets.md`, the table of the reasons: code, when, the player's text,
/// where it is checked). A new reason gets a bit here, a row there, a test
/// and its texts.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayBlock {
    /// The account may not use the kit ([`Account::kit_usable`]).
    KitLocked = 1,
    /// An item is worn in a slot another worn item covers.
    CoveredSlot = 2,
    /// An item type is in two pockets.
    PocketTaken = 4,
    /// A weapon has too little ammo in the kit (or owned) for its
    /// category's minimum.
    NoAmmo = 8,
    /// No weapon.
    NoWeapon = 16,
}

impl PlayBlock {
    /// Every reason, in the order [`check_play`] says the first.
    pub const ALL: [PlayBlock; 5] = [
        PlayBlock::KitLocked,
        PlayBlock::CoveredSlot,
        PlayBlock::PocketTaken,
        PlayBlock::NoAmmo,
        PlayBlock::NoWeapon,
    ];

    pub fn error(self) -> ActionError {
        match self {
            PlayBlock::KitLocked => ActionError::KitNotUsable,
            PlayBlock::CoveredSlot => ActionError::CoveredSlot,
            PlayBlock::PocketTaken => ActionError::PocketTaken,
            PlayBlock::NoAmmo => ActionError::NoAmmo,
            PlayBlock::NoWeapon => ActionError::NoWeapon,
        }
    }
}

/// A set of [`PlayBlock`]s, their bits; 0: the kit can play.
pub type PlayBlocks = u8;

/// Every reason the kit `kit_id` can not play now (0: it can).
pub fn play_blocks(account: &Account, kit_id: u32, now: u64) -> Result<PlayBlocks, ActionError> {
    let index = account.kit_index(kit_id).ok_or(ActionError::NoSuchKit)?;
    let mut blocks = 0;
    if !account.kit_usable(index, now) {
        blocks |= PlayBlock::KitLocked as u8;
    }
    let kit = &account.kits[index];
    // Never a kit the lobby's rule would not have let be: the match gets
    // it as not loaded (`match_profile`).
    if !account.covered_items(kit).is_empty() {
        blocks |= PlayBlock::CoveredSlot as u8;
    }
    if !account.doubled_pockets(kit).is_empty() {
        blocks |= PlayBlock::PocketTaken as u8;
    }
    let dictionary = Dictionary::shared();
    let mut weapons = 0;
    for weapon_slot in WEAPON_SLOTS {
        let Some(weapon) = kit.slots[weapon_slot].and_then(|held| account.item(held.item_id))
        else {
            continue;
        };
        weapons += 1;
        let minimum = dictionary
            .item(weapon.dict_id)
            .and_then(|item| dictionary.categories.iter().find(|c| c.id == item.category))
            .map_or(0, |category| u64::from(category.minimum_count));
        let (mut in_kit, mut owned) = (0_u64, 0_u64);
        for slot in AMMO_SLOTS {
            let Some(held) = kit.slots[slot] else {
                continue;
            };
            let Some(ammo) = account.item(held.item_id) else {
                continue;
            };
            if ammo.condition_or_stack == 0 || !dictionary.compatible(weapon.dict_id, ammo.dict_id)
            {
                continue;
            }
            in_kit += u64::from(held.count);
            owned += u64::from(ammo.condition_or_stack);
        }
        if in_kit < minimum || owned < minimum {
            blocks |= PlayBlock::NoAmmo as u8;
        }
    }
    if weapons == 0 {
        blocks |= PlayBlock::NoWeapon as u8;
    }
    Ok(blocks)
}

/// `2F`: whether the account may enter a battle with a kit: the kit must be
/// its own and usable, wear nothing in a slot another worn item covers
/// ([`Account::covered_items`]), hold no item type in two pockets
/// ([`Account::doubled_pockets`]), hold a weapon, and each weapon must have its
/// category's `minimum_count` of rounds that fit it, in the kit and owned.
/// This is the client's own check before Play (0x2F7DA0, PROVEN; corrected
/// 2026-10-12 after the first live session, where the server asked for a
/// weapon's ammo in "its own" two slots and refused a kit the client had
/// let through): for each weapon it adds up, over the eight ammo slots, the
/// kit's units and the stacks of the ammo that is compatible with the weapon
/// (0x2CA770) and fails when either sum is under
/// `categories_dict/category_<weapon's>/minimum_count`. The texts are its
/// `st_profile_check_failed_*`.
///
/// Whether the kit `kit_id` can play now: the first of its
/// [`play_blocks`], in [`PlayBlock::ALL`]'s order.
pub fn check_play(account: &Account, kit_id: u32, now: u64) -> Result<(), ActionError> {
    let blocks = play_blocks(account, kit_id, now)?;
    match PlayBlock::ALL
        .into_iter()
        .find(|block| blocks & *block as u8 != 0)
    {
        Some(block) => Err(block.error()),
        None => Ok(()),
    }
}

/// Every reason the kit a match would take ([`kit_to_play`]) can not play
/// (0: it can): the rooms' entries refuse on any (code 15), the rooms'
/// member check (`member_fit`) takes such a member out at its match's
/// start, and the lobby movie greys Play and Join by them and lists them in
/// the buttons' tip (the presets list's `<blocks>`, `presets.md`).
pub fn kits_play_blocks(account: &Account, now: u64) -> PlayBlocks {
    play_blocks(account, kit_to_play(account, now), now).unwrap_or(PlayBlock::KitLocked as u8)
}

/// [`kits_play_blocks`] is none.
pub fn kits_can_play(account: &Account, now: u64) -> bool {
    kits_play_blocks(account, now) == 0
}

/// Bit `f - 1` for each faction the account may take quests from and buy
/// at (0x96): all four [choice, as 0.21d: nothing in the client's data
/// locks a faction].
pub fn unlocked_factions(_account: &Account) -> u8 {
    FACTIONS
        .iter()
        .fold(0, |mask, &faction| mask | 1 << (faction - 1))
}

/// Whether `faction` is one the account may choose.
pub fn faction_unlocked(account: &Account, faction: u8) -> bool {
    FACTIONS.contains(&faction) && unlocked_factions(account) & 1 << (faction - 1) != 0
}

/// A kit's item table can not outgrow the client's: three kits of 23 slots
/// are the 69 it holds.
const _: () = assert!(limits::KITS * limits::KIT_SLOTS == limits::ACCOUNT_ITEMS);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::account::Item;
    use survarium::v026g::loadouts;

    const NOW: u64 = 1_000;

    fn dict(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    fn account() -> Account {
        Account::new("Alice")
    }

    fn relocation(item: &Item, source_slot: u8, target_slot: u8, count: u32) -> Relocation {
        Relocation {
            item_id: item.item_id,
            dict_id: item.dict_id,
            source_slot,
            target_slot,
            count,
            count_in_inventory: 0,
        }
    }

    /// A scope (category 22) has no profile slot: no move puts it into a
    /// kit, and a loaded kit that holds one loses it. So no scope is ever in
    /// a kit's ammo slots, where `check_play` reads `items_compatibility`
    /// (the rows weapon/scope item the lobby movie may add change nothing).
    #[test]
    fn a_scope_never_goes_into_a_kit_slot() {
        let mut account = account();
        let scope_id = account.add_item(304, FULL_CONDITION);
        let scope = account.item(scope_id).unwrap().clone();
        for slot in 0..KIT_SLOTS as u8 {
            assert_eq!(
                move_items(
                    &mut account,
                    1,
                    &[relocation(&scope, INVENTORY_SLOT, slot, 1)],
                    NOW
                ),
                Err(ActionError::WrongSlot),
                "slot {slot}"
            );
        }
        account.kits[0].slots[8] = Some(KitSlot {
            item_id: scope_id,
            count: 1,
        });
        assert!(account.repair());
        assert_eq!(account.kits[0].slots[8], None);
    }

    fn rich(money: u32) -> Account {
        let mut account = account();
        account.generic_money = money;
        account
    }

    /// [`rich`], and known to every seller: drugs and gear are the
    /// factions' to sell, each from a reputation level on (ammo from none).
    fn rich_and_known(money: u32) -> Account {
        let mut account = rich(money);
        for faction in 1..=4 {
            account.add_reputation(faction, 40_000);
        }
        account
    }

    #[test]
    fn every_refusal_has_a_text_of_the_game() {
        // Keys of 0.26g's localization.txt, checked against the dump in
        // `tools/re-kit/v026g/data/localization`.
        let known = [
            "st_cannot_relocate_item",
            "st_cannot_buy_item",
            "st_not_enougth_money",
            "st_cant_buy_premium_message",
            "st_wrong_skill_points_count",
            "st_player_skills_check_failed",
            "st_cannot_reroll_skills",
            "st_not_enougth_money_for_reroll",
            "st_cannot_repair_item",
            "st_not_enought_money_for_repair",
            "st_profile_check_failed_weapon",
            "st_profile_check_failed_weapon_ammo",
        ];
        let errors = [
            ActionError::NoSuchKit,
            ActionError::KitNotUsable,
            ActionError::NoSuchItem,
            ActionError::NotInSlot,
            ActionError::WrongSlot,
            ActionError::NotSold,
            ActionError::SellerLevel { needs: 1, has: 0 },
            ActionError::BadCount,
            ActionError::NotEnoughMoney,
            ActionError::NotEnoughPremiumMoney,
            ActionError::Skills(SkillError::Unknown(1)),
            ActionError::Skills(SkillError::OverLevel { spent: 2, level: 1 }),
            ActionError::SkillsLowered,
            ActionError::BadReset,
            ActionError::NotEnoughMoneyForReset,
            ActionError::NothingToRepair,
            ActionError::NoDefects,
            ActionError::NotEnoughMoneyForRepair,
            ActionError::BadFaction,
            ActionError::NoWeapon,
            ActionError::NoAmmo,
            ActionError::CoveredSlot,
            ActionError::PocketTaken,
        ];
        for error in errors {
            assert!(known.contains(&error.text_key()), "{error}");
        }
    }

    /// Any back item standing in for a whole suit (as the Duty exoskeleton:
    /// back slot, covering the other six armour slots).
    fn suit_covers(suit: u16) -> impl Fn(u16) -> &'static [u8] {
        move |dict_id| {
            if dict_id == suit {
                &[0, 1, 2, 4, 5, 6]
            } else {
                &[]
            }
        }
    }

    /// An owned item of the stock that fits `slot`, other than `but`.
    fn owned_for(account: &mut Account, slot: u8, but: u16) -> Item {
        let dictionary = Dictionary::shared();
        let dict_id = dictionary
            .items
            .iter()
            .find(|item| {
                !item.is_stack && item.dict_id != but && dictionary.fits(item.dict_id, slot)
            })
            .unwrap()
            .dict_id;
        let item_id = account.add_item(dict_id, 100);
        account.item(item_id).unwrap().clone()
    }

    fn put_on(account: &mut Account, covers: &dyn Fn(u16) -> &'static [u8], moves: &[Relocation]) {
        relocate_covering(account, Dictionary::shared(), 0, moves, covers).unwrap();
    }

    #[test]
    fn a_suit_is_worn_alone_in_the_slots_it_covers() {
        let mut account = account();
        let suit = owned_for(&mut account, 3, 0);
        let covers = suit_covers(suit.dict_id);
        let helmet = owned_for(&mut account, 0, suit.dict_id);
        let torso = owned_for(&mut account, 2, suit.dict_id);
        put_on(
            &mut account,
            &covers,
            &[
                relocation(&helmet, INVENTORY_SLOT, 0, 1),
                relocation(&torso, INVENTORY_SLOT, 2, 1),
            ],
        );
        let pistol = account.kits[0].slots[7];
        assert!(pistol.is_some());
        // On: the helmet and the torso go to the inventory, still owned.
        put_on(
            &mut account,
            &covers,
            &[relocation(&suit, INVENTORY_SLOT, 3, 1)],
        );
        let slots = account.kits[0].slots;
        assert_eq!(slots[3].map(|held| held.item_id), Some(suit.item_id));
        for covered in [0, 1, 2, 4, 5, 6] {
            assert!(slots[covered].is_none(), "slot {covered}");
        }
        assert_eq!(slots[7], pistol);
        assert!(account.item(helmet.item_id).is_some() && account.item(torso.item_id).is_some());
        // Its weight counts once.
        let pistol_weight =
            account.kit_weight(&account.kits[0]) - item_params(suit.dict_id).unwrap().weight;
        assert!((0.0..2.0).contains(&pistol_weight), "{pistol_weight}");
        // A weapon slot is not covered.
        let pistol_item = account.item(pistol.unwrap().item_id).unwrap().clone();
        put_on(&mut account, &covers, &[relocation(&pistol_item, 7, 10, 1)]);
        assert_eq!(
            account.kits[0].slots[3].map(|held| held.item_id),
            Some(suit.item_id)
        );
        // Anything worn in a covered slot takes the suit off.
        put_on(
            &mut account,
            &covers,
            &[relocation(&helmet, INVENTORY_SLOT, 0, 1)],
        );
        let slots = account.kits[0].slots;
        assert!(slots[3].is_none());
        assert_eq!(slots[0].map(|held| held.item_id), Some(helmet.item_id));
        // An ordinary back item is worn beside the rest.
        let back = owned_for(&mut account, 3, suit.dict_id);
        put_on(
            &mut account,
            &covers,
            &[relocation(&back, INVENTORY_SLOT, 3, 1)],
        );
        assert_eq!(
            account.kits[0].slots[0].map(|held| held.item_id),
            Some(helmet.item_id)
        );
    }

    #[test]
    fn a_suit_request_may_also_name_the_slots_it_emptied() {
        let mut account = account();
        let suit = owned_for(&mut account, 3, 0);
        let covers = suit_covers(suit.dict_id);
        let helmet = owned_for(&mut account, 0, suit.dict_id);
        put_on(
            &mut account,
            &covers,
            &[relocation(&helmet, INVENTORY_SLOT, 0, 1)],
        );
        // The client may send the covered items' moves after the suit's.
        put_on(
            &mut account,
            &covers,
            &[
                relocation(&suit, INVENTORY_SLOT, 3, 1),
                relocation(&helmet, 0, INVENTORY_SLOT, 1),
            ],
        );
        assert!(account.kits[0].slots[0].is_none());
        assert_eq!(
            account.kits[0].slots[3].map(|held| held.item_id),
            Some(suit.item_id)
        );
    }

    #[test]
    fn the_slots_an_item_covers_are_its_configs() {
        // 0.26g's four helmets with a built-in mask cover the mask's slot,
        // and nothing else of the test data covers anything (the Duty
        // exoskeleton, which covers the six armour slots but its own, is
        // not in it: the suit tests above stand in for it).
        let dictionary = Dictionary::shared();
        let covering: Vec<(&str, &[u8])> = dictionary
            .items
            .iter()
            .filter(|item| !covered_slots(item.dict_id).is_empty())
            .map(|item| (item.name.as_str(), covered_slots(item.dict_id)))
            .collect();
        assert_eq!(covering.len(), 4, "{covering:?}");
        for (name, slots) in covering {
            assert_eq!(slots, [1], "{name}");
        }
    }

    /// The Zubr UM-4 helmet (`renesanse_helmet_6`), which has its mask built
    /// in, and the Bandit balaclava of the test patch, owned.
    fn owned_zubr_and_balaclava(account: &mut Account) -> (Item, Item) {
        let dictionary = Dictionary::shared();
        let zubr = dictionary
            .items
            .iter()
            .find(|item| item.cfg_name.ends_with("/renesanse_helmet_6.options"))
            .unwrap()
            .dict_id;
        assert_eq!(covered_slots(zubr), [1]);
        let mut owned = |dict_id: u16| {
            let item_id = account.add_item(dict_id, 100);
            account.item(item_id).unwrap().clone()
        };
        (owned(zubr), owned(1810))
    }

    #[test]
    fn a_helmet_with_its_mask_built_in_is_worn_alone_in_the_masks_slot() {
        let mut account = account();
        let (helmet, balaclava) = owned_zubr_and_balaclava(&mut account);
        move_items(
            &mut account,
            1,
            &[relocation(&balaclava, INVENTORY_SLOT, 1, 1)],
            NOW,
        )
        .unwrap();
        assert_eq!(
            account.kits[0].slots[1].map(|held| held.item_id),
            Some(balaclava.item_id)
        );
        // On: the balaclava goes to the inventory, still owned.
        move_items(
            &mut account,
            1,
            &[relocation(&helmet, INVENTORY_SLOT, 0, 1)],
            NOW,
        )
        .unwrap();
        let slots = account.kits[0].slots;
        assert_eq!(slots[0].map(|held| held.item_id), Some(helmet.item_id));
        assert!(slots[1].is_none());
        assert!(account.item(balaclava.item_id).is_some());
        assert!(account.covered_items(&account.kits[0]).is_empty());
        // The balaclava back on takes the helmet off.
        move_items(
            &mut account,
            1,
            &[relocation(&balaclava, INVENTORY_SLOT, 1, 1)],
            NOW,
        )
        .unwrap();
        let slots = account.kits[0].slots;
        assert!(slots[0].is_none());
        assert_eq!(slots[1].map(|held| held.item_id), Some(balaclava.item_id));
        assert!(account.item(helmet.item_id).is_some());
        assert!(account.covered_items(&account.kits[0]).is_empty());
    }

    #[test]
    fn a_kit_against_the_rule_does_not_play() {
        // A kit saved before the rule, as it would come from srp.db: the
        // Zubr helmet over a mask.
        let mut account = account();
        let (helmet, balaclava) = owned_zubr_and_balaclava(&mut account);
        assert!(check_play(&account, 1, NOW).is_ok());
        account.kits[0].slots[0] = Some(KitSlot {
            item_id: helmet.item_id,
            count: 1,
        });
        account.kits[0].slots[1] = Some(KitSlot {
            item_id: balaclava.item_id,
            count: 1,
        });
        assert_eq!(account.covered_items(&account.kits[0]), [1]);
        assert_eq!(check_play(&account, 1, NOW), Err(ActionError::CoveredSlot));
        assert_eq!(account.uncover_kits(), 1);
        assert_eq!(
            account.kits[0].slots[0].map(|held| held.item_id),
            Some(helmet.item_id)
        );
        assert!(account.kits[0].slots[1].is_none());
        assert!(check_play(&account, 1, NOW).is_ok());
    }

    #[test]
    fn a_mask_that_no_longer_covers_the_helmet_slot_is_worn_with_a_helmet() {
        // The Protective mask (1816) covered the helmet slot until
        // `protective-mask-1.json` (T28). The Bandit balaclava stands in for
        // it (the test data has no 1816): first as it was, covering the
        // helmet slot, then as the data now says, covering nothing.
        let mut account = account();
        let mask_id = account.add_item(1810, 100);
        let mask = account.item(mask_id).unwrap().clone();
        // The Paratrooper's helmet, which has no mask built in.
        let helmet_id = account.add_item(dict("ren_helmet_1"), 100);
        let helmet = account.item(helmet_id).unwrap().clone();
        assert!(covered_slots(helmet.dict_id).is_empty());
        let covers_helmet =
            |dict_id: u16| -> &'static [u8] { if dict_id == mask.dict_id { &[0] } else { &[] } };
        // A player's kit today: the mask on took the helmet off; the mask is
        // in the mask slot (its category fits no other), the helmet owned.
        put_on(
            &mut account,
            &covers_helmet,
            &[relocation(&helmet, INVENTORY_SLOT, 0, 1)],
        );
        put_on(
            &mut account,
            &covers_helmet,
            &[relocation(&mask, INVENTORY_SLOT, 1, 1)],
        );
        let slots = account.kits[0].slots;
        assert!(slots[0].is_none());
        assert_eq!(slots[1].map(|held| held.item_id), Some(mask_id));
        assert!(account.item(helmet_id).is_some());
        // Under the new data the kit loads as it was, and plays.
        let kit = account.kits[0].slots;
        account.repair();
        assert_eq!(account.kits[0].slots, kit);
        assert!(account.covered_items(&account.kits[0]).is_empty());
        assert!(check_play(&account, 1, NOW).is_ok());
        // A helmet goes on beside the mask, and the mask stays.
        move_items(
            &mut account,
            1,
            &[relocation(&helmet, INVENTORY_SLOT, 0, 1)],
            NOW,
        )
        .unwrap();
        let slots = account.kits[0].slots;
        assert_eq!(slots[0].map(|held| held.item_id), Some(helmet_id));
        assert_eq!(slots[1].map(|held| held.item_id), Some(mask_id));
        assert_eq!(account.uncover_kits(), 0);
        assert!(check_play(&account, 1, NOW).is_ok());
        // A helmet with its mask built in still takes the mask off.
        let (zubr, _) = owned_zubr_and_balaclava(&mut account);
        move_items(
            &mut account,
            1,
            &[relocation(&zubr, INVENTORY_SLOT, 0, 1)],
            NOW,
        )
        .unwrap();
        let slots = account.kits[0].slots;
        assert_eq!(slots[0].map(|held| held.item_id), Some(zubr.item_id));
        assert!(slots[1].is_none());
        assert!(account.item(mask_id).is_some());
        assert!(account.item(helmet_id).is_some());
    }

    #[test]
    fn an_item_moves_to_the_inventory_and_back() {
        let mut account = account();
        let pistol = account.inventory[0].clone();
        let revision = account.kits[0].revision;
        move_items(
            &mut account,
            1,
            &[relocation(&pistol, 7, INVENTORY_SLOT, 1)],
            NOW,
        )
        .unwrap();
        assert_eq!(account.kits[0].slots[7], None);
        assert_eq!(account.kits[0].revision, revision + 1);
        // Still owned.
        assert!(account.item(pistol.item_id).is_some());
        // Into the second weapon slot.
        move_items(
            &mut account,
            1,
            &[relocation(&pistol, INVENTORY_SLOT, 10, 1)],
            NOW,
        )
        .unwrap();
        assert_eq!(
            account.kits[0].slots[10],
            Some(KitSlot {
                item_id: pistol.item_id,
                count: 1
            })
        );
        // And from there to the first: one item is in one slot of a kit.
        move_items(&mut account, 1, &[relocation(&pistol, 10, 7, 1)], NOW).unwrap();
        assert_eq!(account.kits[0].slots[10], None);
        assert!(account.kits[0].slots[7].is_some());
        // Another kit may wear the same item.
        move_items(
            &mut account,
            2,
            &[relocation(&pistol, INVENTORY_SLOT, 7, 1)],
            NOW,
        )
        .unwrap();
        assert!(account.kits[0].slots[7].is_some());
        assert!(account.kits[1].slots[7].is_some());
    }

    #[test]
    fn a_move_is_all_or_nothing() {
        let mut account = account();
        let before = account.clone();
        let pistol = account.inventory[0].clone();
        let ammo = account.inventory[1].clone();
        // The pistol into a helmet's slot.
        assert_eq!(
            move_items(&mut account, 1, &[relocation(&pistol, 7, 0, 1)], NOW),
            Err(ActionError::WrongSlot)
        );
        // From a slot it is not in.
        assert_eq!(
            move_items(
                &mut account,
                1,
                &[relocation(&pistol, 10, INVENTORY_SLOT, 1)],
                NOW
            ),
            Err(ActionError::NotInSlot)
        );
        // A good move, then an item not owned.
        let mut ghost = pistol.clone();
        ghost.item_id = 99;
        assert_eq!(
            move_items(
                &mut account,
                1,
                &[
                    relocation(&ammo, 8, INVENTORY_SLOT, 50),
                    relocation(&ghost, INVENTORY_SLOT, 7, 1)
                ],
                NOW
            ),
            Err(ActionError::NoSuchItem)
        );
        // The right id under another dict id.
        let mut renamed = pistol.clone();
        renamed.dict_id = ammo.dict_id;
        assert!(
            move_items(
                &mut account,
                1,
                &[relocation(&renamed, 7, INVENTORY_SLOT, 1)],
                NOW
            )
            .is_err()
        );
        // A slot past the kit's.
        assert_eq!(
            move_items(&mut account, 1, &[relocation(&pistol, 7, 23, 1)], NOW),
            Err(ActionError::WrongSlot)
        );
        assert_eq!(
            move_items(&mut account, 9, &[], NOW),
            Err(ActionError::NoSuchKit)
        );
        assert_eq!(account, before);
    }

    #[test]
    fn the_third_kit_needs_no_premium() {
        let mut account = account();
        assert!(!account.is_premium(NOW));
        let pistol = account.inventory[0].clone();
        let third_kit = account.kits[2].id;
        let moves = [relocation(&pistol, INVENTORY_SLOT, 7, 1)];
        move_items(&mut account, third_kit, &moves, NOW).unwrap();
        set_active_kit(&mut account, third_kit, NOW).unwrap();
        assert_eq!(account.active_kit, third_kit);
        assert_eq!(
            set_active_kit(&mut account, 77, NOW),
            Err(ActionError::NoSuchKit)
        );
    }

    /// The client's ammo dialog: every ammo slot goes to the inventory and
    /// comes back with the rounds now wanted; none leaves the slot empty.
    #[test]
    fn an_ammo_slot_set_to_no_rounds_is_emptied() {
        let mut account = account();
        let ammo = account.inventory[1].clone();
        let moves = [
            relocation(&ammo, 8, INVENTORY_SLOT, 50),
            relocation(&ammo, INVENTORY_SLOT, 8, 0),
        ];
        move_items(&mut account, 1, &moves, NOW).unwrap();
        assert_eq!(account.kits[0].slots[8], None);
        assert_eq!(account.item(ammo.item_id).unwrap().condition_or_stack, 50);
        // And back with 20 of them.
        move_items(
            &mut account,
            1,
            &[relocation(&ammo, INVENTORY_SLOT, 8, 20)],
            NOW,
        )
        .unwrap();
        assert_eq!(account.kits[0].slots[8].unwrap().count, 20);
        // A slot takes at most what is owned.
        move_items(
            &mut account,
            1,
            &[
                relocation(&ammo, 8, INVENTORY_SLOT, 20),
                Relocation {
                    count_in_inventory: 50,
                    ..relocation(&ammo, INVENTORY_SLOT, 8, 50)
                },
            ],
            NOW,
        )
        .unwrap();
        assert_eq!(account.kits[0].slots[8].unwrap().count, 50);
        // Nothing was missing, so nothing was bought.
        assert_eq!(account.item(ammo.item_id).unwrap().condition_or_stack, 50);
    }

    /// The request the ammo dialog made on 2026-10-12 for a kit with ammo
    /// in slot 8 and a drug in pocket slot 13, refused then as `NotInSlot`.
    #[test]
    fn the_ammo_dialog_keeps_what_is_in_a_pocket_slot() {
        const POCKET: u8 = 13;
        let dictionary = Dictionary::shared();
        let mut account = rich_and_known(10_000);
        // Something for the pocket: a stack the dictionary lets into it.
        let drug = dictionary
            .items
            .iter()
            .find(|item| item.is_stack && dictionary.fits(item.dict_id, POCKET))
            .expect("a pocket item")
            .dict_id;
        let drug_id = account.add_item(drug, 1);
        let drug = account.item(drug_id).unwrap().clone();
        move_items(
            &mut account,
            1,
            &[relocation(&drug, INVENTORY_SLOT, POCKET, 1)],
            NOW,
        )
        .unwrap();
        let ammo = account.inventory[1].clone();
        let in_slot = account.kits[0].slots[8].unwrap().count;
        let before = account.clone();

        // Every occupied ammo and pocket slot to the inventory, then the
        // rows: the ammo from the inventory, the drug "from" its slot.
        let dialog = [
            relocation(&ammo, 8, INVENTORY_SLOT, in_slot),
            relocation(&drug, POCKET, INVENTORY_SLOT, 1),
            Relocation {
                count_in_inventory: ammo.condition_or_stack,
                ..relocation(&ammo, INVENTORY_SLOT, 8, in_slot)
            },
            Relocation {
                count_in_inventory: 1,
                ..relocation(&drug, POCKET, POCKET, 1)
            },
        ];
        move_items(&mut account, 1, &dialog, NOW).unwrap();
        assert_eq!(account.kits[0].slots, before.kits[0].slots);
        assert_eq!(account.inventory, before.inventory);
        assert_eq!(account.generic_money, before.generic_money);

        // Two pocket items changing places name each other's slot.
        let other = dictionary
            .items
            .iter()
            .find(|item| {
                item.is_stack && item.dict_id != drug.dict_id && dictionary.fits(item.dict_id, 14)
            })
            .expect("another pocket item")
            .dict_id;
        let other_id = account.add_item(other, 1);
        let other = account.item(other_id).unwrap().clone();
        move_items(
            &mut account,
            1,
            &[relocation(&other, INVENTORY_SLOT, 14, 1)],
            NOW,
        )
        .unwrap();
        let swap = [
            relocation(&drug, POCKET, INVENTORY_SLOT, 1),
            relocation(&other, 14, INVENTORY_SLOT, 1),
            Relocation {
                count_in_inventory: 1,
                ..relocation(&drug, POCKET, 14, 1)
            },
            Relocation {
                count_in_inventory: 1,
                ..relocation(&other, 14, POCKET, 1)
            },
        ];
        move_items(&mut account, 1, &swap, NOW).unwrap();
        assert_eq!(account.kits[0].slots[13].unwrap().item_id, other_id);
        assert_eq!(account.kits[0].slots[14].unwrap().item_id, drug_id);

        // A slot the request did not empty of the item is still refused.
        assert_eq!(
            move_items(
                &mut account,
                1,
                &[relocation(&drug, POCKET, INVENTORY_SLOT, 1)],
                NOW
            ),
            Err(ActionError::NotInSlot)
        );
    }

    /// A stack dropped from the shop's list on a pocket (an energy drink on
    /// slot 14, 2026-10-29): the ammo dialog names it from `SHOP_SLOT`, item
    /// 0, one wanted. It is bought and put in; it was refused as `WrongSlot`.
    #[test]
    fn a_stack_dropped_from_the_shop_on_a_pocket_is_bought_into_it() {
        const POCKET: u8 = 14;
        let dictionary = Dictionary::shared();
        let mut account = rich_and_known(10_000);
        let drink = dictionary
            .items
            .iter()
            .find(|item| {
                item.is_stack
                    && dictionary.fits(item.dict_id, POCKET)
                    && step_cost(&account, dictionary, item.dict_id).is_ok_and(|cost| cost > 0)
            })
            .expect("a pocket item sold for silver")
            .dict_id;
        let cost = step_cost(&account, dictionary, drink).unwrap();
        let money = account.generic_money;
        let dialog = [Relocation {
            item_id: 0,
            dict_id: drink,
            source_slot: SHOP_SLOT,
            target_slot: POCKET,
            count: 1,
            count_in_inventory: 1,
        }];
        move_items(&mut account, 1, &dialog, NOW).unwrap();
        let bought = account
            .inventory
            .iter()
            .find(|item| item.dict_id == drink)
            .expect("bought");
        assert_eq!(
            account.kits[0].slots[usize::from(POCKET)].map(|slot| slot.item_id),
            Some(bought.item_id)
        );
        assert_eq!(account.generic_money, money - cost);

        // Dropped again on another pocket while one holds it: one pocket an
        // item type, so nothing is bought or moved.
        let before = account.clone();
        let again = [Relocation {
            target_slot: POCKET + 1,
            ..dialog[0]
        }];
        assert_eq!(
            move_items(&mut account, 1, &again, NOW),
            Err(ActionError::PocketTaken)
        );
        assert_eq!(account, before);

        // Not enough silver: nothing changes.
        let mut poor = rich_and_known(0);
        let before = poor.clone();
        assert_eq!(
            move_items(&mut poor, 1, &dialog, NOW),
            Err(ActionError::NotEnoughMoney)
        );
        assert_eq!(poor, before);
    }

    /// A consumable bought onto a pocket at more than the pocket takes
    /// (`docs/notes/items.md` §8): the lobby names the units wanted in all
    /// (those owned and the 30 bought) and the pocket's. The 30 are bought,
    /// the pocket holds its share, the rest stays in the inventory.
    #[test]
    fn a_buy_onto_a_pocket_beyond_its_size_keeps_the_rest_in_the_inventory() {
        const POCKET: u8 = 14;
        let dictionary = Dictionary::shared();
        let mut account = rich_and_known(10_000);
        let medkit = dict("medkit");
        let owned = account.add_item(medkit, 4);
        let cost = step_cost(&account, dictionary, medkit).unwrap();
        let money = account.generic_money;
        let dialog = [Relocation {
            item_id: 0,
            dict_id: medkit,
            source_slot: SHOP_SLOT,
            target_slot: POCKET,
            count: 3,
            count_in_inventory: 4 + 30,
        }];
        move_items(&mut account, 1, &dialog, NOW).unwrap();
        assert_eq!(account.item(owned).unwrap().condition_or_stack, 34);
        assert_eq!(
            account.kits[0].slots[usize::from(POCKET)],
            Some(KitSlot {
                item_id: owned,
                count: 3
            })
        );
        assert_eq!(account.generic_money, money - 30 * cost);
    }

    /// One pocket an item type (`docs/notes/one-pocket-a-type.md`).
    #[test]
    fn an_item_type_goes_into_one_pocket() {
        let mut account = rich_and_known(10_000);
        let grenade_id = account.add_item(dict("rgd-5"), 3);
        let grenade = account.item(grenade_id).unwrap().clone();
        let medkit_id = account.add_item(dict("medkit"), 5);
        let medkit = account.item(medkit_id).unwrap().clone();
        move_items(
            &mut account,
            1,
            &[relocation(&grenade, INVENTORY_SLOT, 13, 2)],
            NOW,
        )
        .unwrap();
        let before = account.clone();

        // A second pocket of it, from the inventory: refused, nothing changed.
        assert_eq!(
            move_items(
                &mut account,
                1,
                &[relocation(&grenade, INVENTORY_SLOT, 14, 1)],
                NOW
            ),
            Err(ActionError::PocketTaken)
        );
        assert_eq!(account, before);
        // Dragged from the shop onto a second pocket: nothing bought.
        assert_eq!(
            buy(&mut account, grenade.dict_id, 1, 0, 14, 1, NOW),
            Err(ActionError::PocketTaken)
        );
        assert_eq!(account, before);
        // The ammo window's request leaving it in two pockets (what the
        // lobby asked when a second pocket of it was filled): refused.
        let dialog = [
            relocation(&grenade, 13, INVENTORY_SLOT, 2),
            Relocation {
                count_in_inventory: 3,
                ..relocation(&grenade, 13, 13, 2)
            },
            Relocation {
                count_in_inventory: 3,
                ..relocation(&grenade, INVENTORY_SLOT, 14, 1)
            },
        ];
        assert_eq!(
            move_items(&mut account, 1, &dialog, NOW),
            Err(ActionError::PocketTaken)
        );
        assert_eq!(account, before);

        // Its own pocket takes more of it, and it may move to another one.
        move_items(
            &mut account,
            1,
            &[relocation(&grenade, INVENTORY_SLOT, 13, 3)],
            NOW,
        )
        .unwrap();
        assert_eq!(account.kits[0].slots[13].unwrap().count, 3);
        move_items(&mut account, 1, &[relocation(&grenade, 13, 15, 3)], NOW).unwrap();
        assert_eq!(account.kits[0].slots[13], None);
        assert_eq!(
            account.kits[0].slots[15],
            Some(KitSlot {
                item_id: grenade_id,
                count: 3
            })
        );
        // Other types in the other pockets; another kit has its own.
        move_items(
            &mut account,
            1,
            &[relocation(&medkit, INVENTORY_SLOT, 13, 2)],
            NOW,
        )
        .unwrap();
        move_items(
            &mut account,
            2,
            &[relocation(&grenade, INVENTORY_SLOT, 13, 2)],
            NOW,
        )
        .unwrap();
        // One ammo in two ammo slots is no pocket's business.
        let ammo = account.inventory[1].clone();
        move_items(
            &mut account,
            1,
            &[relocation(&ammo, INVENTORY_SLOT, 9, 20)],
            NOW,
        )
        .unwrap();
        assert_eq!(account.kits[0].slots[8].unwrap().item_id, ammo.item_id);
        assert_eq!(account.kits[0].slots[9].unwrap().item_id, ammo.item_id);
        // Nothing was bought or lost by any of it.
        assert_eq!(account.inventory, before.inventory);
        assert_eq!(account.generic_money, before.generic_money);
        assert_eq!(check_play(&account, 1, NOW), Ok(()));

        // A kit that breaks the rule (saved so before it) is no kit to play.
        account.kits[0].slots[16] = Some(KitSlot {
            item_id: grenade_id,
            count: 1,
        });
        assert_eq!(check_play(&account, 1, NOW), Err(ActionError::PocketTaken));
    }

    #[test]
    fn the_ammo_dialog_buys_what_is_missing() {
        let dictionary = Dictionary::shared();
        // A stack the shop lists: 5.45x39, 30 rounds a clip.
        let rounds = dict("ammo_5.45x39_7n6m");
        let record = prices::price(rounds).unwrap();
        let unit = shop_unit(rounds);
        assert!(unit > 1);
        // Known to no seller: ammo needs no reputation level.
        let mut account = rich(10_000);
        // Not owned at all: the dialog names it with item id 0, in the
        // first ammo slot of the second weapon... which holds only ammo
        // that fits: use a slot the dictionary lets it into.
        let slot = (0..KIT_SLOTS as u8)
            .find(|&slot| dictionary.fits(rounds, slot))
            .unwrap();
        let wanted = unit + 1;
        let moves = [Relocation {
            item_id: 0,
            dict_id: rounds,
            source_slot: INVENTORY_SLOT,
            target_slot: slot,
            count: wanted,
            count_in_inventory: wanted,
        }];
        move_items(&mut account, 1, &moves, NOW).unwrap();
        // Two clips for one round more than a clip.
        let stack = account
            .inventory
            .iter()
            .find(|item| item.dict_id == rounds)
            .unwrap();
        assert_eq!(stack.condition_or_stack, 2 * unit);
        assert_eq!(account.generic_money, 10_000 - 2 * record.cost);
        assert_eq!(
            account.kits[0].slots[usize::from(slot)],
            Some(KitSlot {
                item_id: stack.item_id,
                count: wanted
            })
        );
        // With no money for it nothing changes.
        let mut poor = rich(record.cost - 1);
        let before = poor.clone();
        assert_eq!(
            move_items(&mut poor, 1, &moves, NOW),
            Err(ActionError::NotEnoughMoney)
        );
        assert_eq!(poor, before);
    }

    #[test]
    fn the_starting_ammo_is_refilled_for_its_price_of_nothing() {
        let mut account = account();
        let ammo = account.inventory[1].clone();
        // Listed for nothing by seller 0 (so the client lets it be added to).
        let record = prices::price(ammo.dict_id).unwrap();
        assert_eq!((record.cost, record.faction_id, record.level), (0, 0, 0));
        let money = account.generic_money;
        let moves = [
            relocation(&ammo, 8, INVENTORY_SLOT, 50),
            Relocation {
                count_in_inventory: 120,
                ..relocation(&ammo, INVENTORY_SLOT, 8, 100)
            },
        ];
        move_items(&mut account, 1, &moves, NOW).unwrap();
        // 70 missing: two clips of 50.
        assert_eq!(account.item(ammo.item_id).unwrap().condition_or_stack, 150);
        assert_eq!(account.kits[0].slots[8].unwrap().count, 100);
        assert_eq!(account.generic_money, money);
        // Up to the client's limit for what is free, and no further.
        let all = [Relocation {
            count_in_inventory: 5_000,
            ..relocation(&ammo, INVENTORY_SLOT, 8, 100)
        }];
        move_items(&mut account, 1, &all, NOW).unwrap();
        assert_eq!(
            account.item(ammo.item_id).unwrap().condition_or_stack,
            MAX_FREE_UNITS
        );
        assert_eq!(
            move_items(&mut account, 1, &all, NOW),
            Err(ActionError::BadCount)
        );
        assert_eq!(account.generic_money, money);
        // The pistol itself is sold for the owner's 500 (2026-10-23).
        let pistol = account.inventory[0].dict_id;
        assert!(buy(&mut account, pistol, 1, 0, INVENTORY_SLOT, 1, NOW).is_ok());
        assert_eq!(account.generic_money, money - 500);
    }

    #[test]
    fn buying_a_stack_adds_to_the_one_owned() {
        let medkit = dict("medkit");
        let record = prices::price(medkit).unwrap();
        let mut account = rich_and_known(1_000);
        let bought = buy(&mut account, medkit, 3, 0, INVENTORY_SLOT, 1, NOW).unwrap();
        let item_id = account.inventory.last().unwrap().item_id;
        assert_eq!(
            bought,
            [Bought {
                dict_id: medkit,
                item_id,
                value: 3
            }]
        );
        assert_eq!(account.generic_money, 1_000 - 3 * record.cost);
        // Again: the same stack grows.
        let again = buy(
            &mut account,
            medkit,
            2,
            record.faction_id,
            INVENTORY_SLOT,
            1,
            NOW,
        )
        .unwrap();
        assert_eq!(again[0].item_id, item_id);
        assert_eq!(again[0].value, 2);
        assert_eq!(account.item(item_id).unwrap().condition_or_stack, 5);
        assert_eq!(account.inventory.len(), 6);
    }

    #[test]
    fn ammo_is_bought_by_the_clip() {
        let rounds = dict("ammo_5.45x39_7n6m");
        let record = prices::price(rounds).unwrap();
        let unit = shop_unit(rounds);
        let mut account = rich_and_known(1_000);
        let bought = buy(&mut account, rounds, 2, 0, INVENTORY_SLOT, 1, NOW).unwrap();
        assert_eq!(bought[0].value, 2 * unit);
        assert_eq!(account.generic_money, 1_000 - 2 * record.cost);
    }

    /// The reputation a faction's `level` starts at.
    fn reputation_for(faction_id: u8, level: u8) -> u32 {
        Dictionary::shared()
            .factions
            .iter()
            .find(|faction| faction.id == faction_id)
            .unwrap()
            .levels[usize::from(level)]
        .value
    }

    #[test]
    fn supplies_need_a_sellers_level_and_either_seller_of_two_sells() {
        // The RGD-5: the Scavengers and the Black Market, level 3 each.
        let grenade = dict("rgd-5");
        let sellers: Vec<(u8, u8)> = prices::sellers(grenade)
            .iter()
            .map(|record| (record.faction_id, record.level))
            .collect();
        assert_eq!(sellers, [(1, 3), (2, 3)]);
        let cost = prices::price(grenade).unwrap().cost;
        let mut account = rich(10 * cost);
        // Known to neither: refused with the level of the seller named (the
        // first when none is).
        for faction in [0, 1, 2] {
            assert_eq!(
                buy(
                    &mut account.clone(),
                    grenade,
                    1,
                    faction,
                    INVENTORY_SLOT,
                    1,
                    NOW
                ),
                Err(ActionError::SellerLevel { needs: 3, has: 0 })
            );
        }
        // A faction that does not sell it.
        assert_eq!(
            buy(&mut account.clone(), grenade, 1, 3, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotSold)
        );
        // Known to the Black Market only: it sells, whichever of the two
        // the client named (it names the first record's when its window
        // names none).
        account.add_reputation(2, reputation_for(2, 3));
        assert_eq!(
            (account.reputation_level(1), account.reputation_level(2)),
            (0, 3)
        );
        for faction in [0, 1, 2] {
            let mut buyer = account.clone();
            let bought = buy(&mut buyer, grenade, 2, faction, INVENTORY_SLOT, 1, NOW).unwrap();
            assert_eq!(bought.len(), 1);
            assert_eq!(bought[0].value, 2 * shop_unit(grenade));
            assert_eq!(buyer.generic_money, 8 * cost);
        }
        // One level short is short.
        let mut short = rich(10 * cost);
        short.add_reputation(1, reputation_for(1, 3) - 1);
        assert_eq!(
            buy(&mut short, grenade, 1, 1, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::SellerLevel { needs: 3, has: 2 })
        );
        // Ammo needs no level: 12x70 slugs, the same two sellers, bought by
        // an account known to neither, whichever seller the client names.
        let slug = dict("ammo_12mm_slug");
        let sellers: Vec<(u8, u8)> = prices::sellers(slug)
            .iter()
            .map(|record| (record.faction_id, record.level))
            .collect();
        assert_eq!(sellers, [(1, 0), (2, 0)]);
        let cost = prices::price(slug).unwrap().cost;
        for faction in [0, 1, 2] {
            let mut buyer = rich(10 * cost);
            let bought = buy(&mut buyer, slug, 2, faction, INVENTORY_SLOT, 1, NOW).unwrap();
            assert_eq!(bought[0].value, 2 * shop_unit(slug));
            assert_eq!(buyer.generic_money, 8 * cost);
        }
        // A faction that does not sell it still does not.
        assert_eq!(
            buy(&mut rich(10 * cost), slug, 1, 3, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotSold)
        );
        // A medkit: the Scavengers at level 3, no longer everyone's.
        let medkit = dict("medkit");
        let mut account = rich(1_000);
        assert_eq!(
            buy(&mut account, medkit, 1, 0, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::SellerLevel { needs: 3, has: 0 })
        );
        account.add_reputation(1, reputation_for(1, 3));
        assert!(buy(&mut account, medkit, 1, 1, INVENTORY_SLOT, 1, NOW).is_ok());
        // The boosters are sold to everyone.
        let booster = dict("speed_booster");
        let mut account = rich(prices::price(booster).unwrap().cost);
        assert!(buy(&mut account, booster, 1, 0, INVENTORY_SLOT, 1, NOW).is_ok());
    }

    #[test]
    fn buying_gear_makes_new_items_and_needs_the_sellers_level() {
        let dictionary = Dictionary::shared();
        // The cheapest gear a faction sells at level 0 or 1 (no cosmetic,
        // category 25 on).
        let record = prices::price_list()
            .iter()
            .filter(|record| {
                record.faction_id != 0
                    && !dictionary.is_stack(record.dict_id)
                    && dictionary.item(record.dict_id).unwrap().category < 25
            })
            .min_by_key(|record| (record.level, record.cost))
            .unwrap();
        let mut account = rich(record.cost * 3);
        // Reach the level the item needs.
        let needed = dictionary
            .factions
            .iter()
            .find(|faction| faction.id == record.faction_id)
            .unwrap()
            .levels[usize::from(record.level)]
        .value;
        if record.level > 0 {
            assert_eq!(
                buy(
                    &mut account,
                    record.dict_id,
                    1,
                    record.faction_id,
                    INVENTORY_SLOT,
                    1,
                    NOW
                ),
                Err(ActionError::SellerLevel {
                    needs: record.level,
                    has: 0
                })
            );
            account.add_reputation(record.faction_id, needed);
        }
        let bought = buy(
            &mut account,
            record.dict_id,
            2,
            record.faction_id,
            INVENTORY_SLOT,
            1,
            NOW,
        )
        .unwrap();
        assert_eq!(bought.len(), 2);
        assert_ne!(bought[0].item_id, bought[1].item_id);
        for entry in &bought {
            assert_eq!(entry.value, FULL_CONDITION);
            assert_eq!(
                account.item(entry.item_id).unwrap().condition_or_stack,
                FULL_CONDITION
            );
        }
        assert_eq!(account.generic_money, record.cost);
        // Another seller than the item's.
        let other = if record.faction_id == 1 { 2 } else { 1 };
        assert_eq!(
            buy(
                &mut account,
                record.dict_id,
                1,
                other,
                INVENTORY_SLOT,
                1,
                NOW
            ),
            Err(ActionError::NotSold)
        );
        // Counts: none, and more than a purchase makes.
        assert_eq!(
            buy(&mut account, record.dict_id, 0, 0, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::BadCount)
        );
        assert_eq!(
            buy(&mut account, record.dict_id, 9, 0, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::BadCount)
        );
        // No money left for two.
        assert_eq!(
            buy(&mut account, record.dict_id, 2, 0, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotEnoughMoney)
        );
        assert_eq!(
            buy(&mut account, 9999, 1, 0, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotSold)
        );
    }

    #[test]
    fn a_faction_camouflage_and_the_vityaz_are_bought_at_no_rank() {
        // Known to no faction: the Black Market's Denim (1,000) from the
        // Black Market, the Vityaz (25,000, the owner's price) and the
        // Lancaster (2,500) from seller 0, whichever seller is named for
        // those.
        let denim = dict("camo_denim");
        let mut account = rich(1_000);
        let bought = buy(&mut account, denim, 1, 2, INVENTORY_SLOT, 1, NOW).unwrap();
        assert_eq!(bought.len(), 1);
        assert_eq!(account.item(bought[0].item_id).unwrap().dict_id, denim);
        assert_eq!(account.generic_money, 0);
        // Not from another faction.
        assert_eq!(
            buy(&mut rich(1_000), denim, 1, 1, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotSold)
        );
        for (name, cost) in [("vityaz", 25_000), ("izh_27_lancaster", 2_500)] {
            let weapon = dict(name);
            let mut account = rich(cost);
            let bought = buy(&mut account, weapon, 1, 0, INVENTORY_SLOT, 1, NOW).unwrap();
            assert_eq!(bought[0].value, FULL_CONDITION, "{name}");
            assert_eq!(account.generic_money, 0, "{name}");
            assert_eq!(
                buy(&mut rich(cost - 1), weapon, 1, 0, INVENTORY_SLOT, 1, NOW),
                Err(ActionError::NotEnoughMoney),
                "{name}"
            );
        }
    }

    #[test]
    fn an_item_bought_onto_the_character_is_equipped() {
        let dictionary = Dictionary::shared();
        let record = prices::price_list()
            .iter()
            .filter(|record| {
                record.faction_id != 0 && record.level == 0 && dictionary.fits(record.dict_id, 7)
            })
            .min_by_key(|record| record.cost);
        let Some(record) = record else {
            // No weapon is sold at level 0: nothing to check.
            return;
        };
        let mut account = rich(record.cost);
        let bought = buy(
            &mut account,
            record.dict_id,
            1,
            record.faction_id,
            10,
            1,
            NOW,
        )
        .unwrap();
        assert_eq!(
            account.kits[0].slots[10],
            Some(KitSlot {
                item_id: bought[0].item_id,
                count: 1
            })
        );
        // Into a slot it does not fit: nothing is bought.
        let mut account = rich(record.cost);
        let before = account.clone();
        assert_eq!(
            buy(
                &mut account,
                record.dict_id,
                1,
                record.faction_id,
                0,
                1,
                NOW
            ),
            Err(ActionError::WrongSlot)
        );
        assert_eq!(account, before);
    }

    #[test]
    fn selling_pays_a_quarter() {
        let medkit = dict("medkit");
        let record = prices::price(medkit).unwrap();
        let mut account = rich_and_known(1_000);
        let bought = buy(&mut account, medkit, 4, 0, INVENTORY_SLOT, 1, NOW).unwrap();
        let money = account.generic_money;
        let sold = sell(&mut account, bought[0].item_id, 3).unwrap();
        assert_eq!(
            sold,
            Sold {
                item_id: bought[0].item_id,
                dict_id: medkit,
                amount: 3,
                money: 3 * (record.cost / 4)
            }
        );
        assert_eq!(account.generic_money, money + sold.money);
        assert_eq!(
            account.item(bought[0].item_id).unwrap().condition_or_stack,
            1
        );
        // More than is left sells what is left, and the stack is gone.
        let sold = sell(&mut account, bought[0].item_id, 10).unwrap();
        assert_eq!(sold.amount, 1);
        assert!(account.item(bought[0].item_id).is_none());
        assert_eq!(
            sell(&mut account, bought[0].item_id, 1),
            Err(ActionError::NoSuchItem)
        );
    }

    #[test]
    fn selling_what_a_kit_wears_empties_its_slot() {
        let mut account = account();
        let pistol = account.inventory[0].clone();
        let ammo = account.inventory[1].clone();
        let revision = account.kits[0].revision;
        // The pistol costs nothing, so it sells for nothing.
        let sold = sell(&mut account, pistol.item_id, 1).unwrap();
        assert_eq!((sold.amount, sold.money), (1, 0));
        assert_eq!(account.kits[0].slots[7], None);
        assert_eq!(account.kits[0].revision, revision + 1);
        // Selling none of a stack is refused.
        assert_eq!(
            sell(&mut account, ammo.item_id, 0),
            Err(ActionError::BadCount)
        );
        // The whole clip: the slot empties with it.
        sell(&mut account, ammo.item_id, 1).unwrap();
        assert_eq!(account.kits[0].slots[8], None);
        // What is left is what a new account does not wear.
        assert_eq!(account.inventory.len(), loadouts::starter_stash().len());
    }

    #[test]
    fn a_worn_item_sells_for_less_and_repairs_for_silver() {
        let dictionary = Dictionary::shared();
        let record = prices::price_list()
            .iter()
            .filter(|record| !dictionary.is_stack(record.dict_id))
            .max_by_key(|record| record.cost)
            .unwrap();
        let mut account = rich(1_000_000);
        let item_id = account.add_item(record.dict_id, FULL_CONDITION);
        // Nothing to repair on a whole item, or on a stack.
        assert_eq!(
            repair(&mut account, item_id, RepairPayment::Silver),
            Err(ActionError::NothingToRepair)
        );
        let ammo = account.inventory[1].item_id;
        assert_eq!(
            repair(&mut account, ammo, RepairPayment::Silver),
            Err(ActionError::NothingToRepair)
        );
        assert_eq!(
            repair(&mut account, 999, RepairPayment::Silver),
            Err(ActionError::NoSuchItem)
        );
        // A match's wear, a few times over.
        let per_match = wear_per_match(record.dict_id);
        assert!(per_match > 0.0);
        assert_eq!(wear(&mut account, item_id, 0.5), Some(FULL_CONDITION / 2));
        assert_eq!(shown_condition(FULL_CONDITION / 2), 50);
        let cost = prices::repair_cost(dictionary, record.dict_id, 50);
        assert!(cost > 0);
        // Gold and kits fix defects, which no item has.
        assert_eq!(
            repair(&mut account, item_id, RepairPayment::Gold),
            Err(ActionError::NoDefects)
        );
        assert_eq!(
            repair(&mut account, item_id, RepairPayment::Kit { kit_item_id: 5 }),
            Err(ActionError::NoDefects)
        );
        let mut poor = account.clone();
        poor.generic_money = cost - 1;
        assert_eq!(
            repair(&mut poor, item_id, RepairPayment::Silver),
            Err(ActionError::NotEnoughMoneyForRepair)
        );
        let money = account.generic_money;
        assert_eq!(
            repair(&mut account, item_id, RepairPayment::Silver),
            Ok(Repaired::AtOnce)
        );
        assert_eq!(account.generic_money, money - cost);
        assert_eq!(
            account.item(item_id).unwrap().condition_or_stack,
            FULL_CONDITION
        );
        // Sold at half its condition: half of a quarter.
        wear(&mut account, item_id, 0.5);
        let sold = sell(&mut account, item_id, 1).unwrap();
        assert_eq!(sold.money, record.cost / 4 * 50 / 100);
        // Wear stops at nothing left, and leaves a stack alone.
        let item_id = account.add_item(record.dict_id, FULL_CONDITION);
        assert_eq!(wear(&mut account, item_id, 7.0), Some(0));
        assert_eq!(wear(&mut account, ammo, 0.5), Some(50));
        assert_eq!(wear(&mut account, 999, 0.5), None);
    }

    #[test]
    fn gold_is_exchanged_at_the_rate() {
        let mut account = account();
        account.premium_money = 10;
        let money = account.generic_money;
        assert_eq!(exchange_premium(&mut account, 4), Ok(400));
        assert_eq!(account.premium_money, 6);
        assert_eq!(account.generic_money, money + 400);
        assert_eq!(
            exchange_premium(&mut account, 7),
            Err(ActionError::NotEnoughPremiumMoney)
        );
        assert_eq!(
            exchange_premium(&mut account, 0),
            Err(ActionError::BadCount)
        );
        assert_eq!(service_prices().premium_exchange_rate, 100);
    }

    #[test]
    fn gold_is_bought_at_the_rate() {
        let mut account = account();
        account.generic_money = 1_000;
        account.premium_money = 2;
        assert_eq!(buy_premium(&mut account, 3), Ok(300));
        assert_eq!(account.generic_money, 700);
        assert_eq!(account.premium_money, 5);
        assert_eq!(
            buy_premium(&mut account, 8),
            Err(ActionError::NotEnoughMoney)
        );
        assert_eq!(account.generic_money, 700);
        assert_eq!(buy_premium(&mut account, 0), Err(ActionError::BadCount));
        // More gold than silver could ever pay for, and a full gold balance.
        assert_eq!(
            buy_premium(&mut account, u32::MAX),
            Err(ActionError::BadCount)
        );
        account.generic_money = u32::MAX;
        account.premium_money = u32::MAX;
        assert_eq!(buy_premium(&mut account, 1), Err(ActionError::BadCount));
        assert_eq!(account.generic_money, u32::MAX);
    }

    #[test]
    fn skill_points_are_spent_within_the_level() {
        let mut account = account();
        assert_eq!(
            set_skills(&mut account, &[(101, 1)]),
            Err(ActionError::Skills(SkillError::OverLevel {
                spent: 1,
                level: 0
            }))
        );
        account.add_experience(100_000);
        let level = account.level();
        assert!(level >= 10);
        let revisions: Vec<u32> = account.kits.iter().map(|kit| kit.revision).collect();
        set_skills(&mut account, &[(102, 3), (101, 2), (103, 0)]).unwrap();
        // Kept sorted, without the skill with no points.
        assert_eq!(account.skills, [(101, 2), (102, 3)]);
        assert_eq!(account.free_skill_points(), u32::from(level) - 5);
        // Every kit carries the new modifiers.
        for (kit, before) in account.kits.iter().zip(&revisions) {
            assert_eq!(kit.revision, before + 1);
        }
        // The same set again changes nothing.
        set_skills(&mut account, &[(101, 2), (102, 3)]).unwrap();
        assert_eq!(account.kits[0].revision, revisions[0] + 1);
        // A point taken back, or a skill left out, is no reset.
        assert_eq!(
            set_skills(&mut account, &[(101, 1), (102, 3)]),
            Err(ActionError::SkillsLowered)
        );
        assert_eq!(
            set_skills(&mut account, &[(101, 2)]),
            Err(ActionError::SkillsLowered)
        );
        // The tree's own rules.
        assert!(matches!(
            set_skills(&mut account, &[(101, 2), (102, 3), (999, 1)]),
            Err(ActionError::Skills(SkillError::Unknown(999)))
        ));
        assert!(matches!(
            set_skills(&mut account, &[(101, 6), (102, 3)]),
            Err(ActionError::Skills(SkillError::TooManyPoints { .. }))
        ));
        assert_eq!(account.skills, [(101, 2), (102, 3)]);
    }

    #[test]
    fn a_reset_frees_one_branch_for_its_price() {
        let mut account = account();
        account.add_experience(100_000);
        // A new account has no gold.
        account.premium_money = 10;
        set_skills(&mut account, &[(101, 2), (102, 3), (201, 1)]).unwrap();
        let money = account.generic_money;
        // The second tab: skills 201 and on.
        assert_eq!(reset_skills(&mut account, 1, reset_payment::SILVER), Ok(1));
        assert_eq!(account.skills, [(101, 2), (102, 3)]);
        assert_eq!(account.generic_money, money - REROLL_COST);
        // Nothing left in it to reset.
        assert_eq!(
            reset_skills(&mut account, 1, reset_payment::SILVER),
            Err(ActionError::BadReset)
        );
        // In gold.
        let gold = account.premium_money;
        assert_eq!(reset_skills(&mut account, 0, reset_payment::GOLD), Ok(5));
        assert!(account.skills.is_empty());
        assert_eq!(account.premium_money, gold - REROLL_COST_PREMIUM);
        assert_eq!(account.generic_money, money - REROLL_COST);

        set_skills(&mut account, &[(101, 1)]).unwrap();
        let before = account.clone();
        assert_eq!(
            reset_skills(&mut account, 2, reset_payment::SILVER),
            Err(ActionError::BadReset)
        );
        assert_eq!(reset_skills(&mut account, 0, 7), Err(ActionError::BadReset));
        account.generic_money = REROLL_COST - 1;
        assert_eq!(
            reset_skills(&mut account, 0, reset_payment::SILVER),
            Err(ActionError::NotEnoughMoneyForReset)
        );
        account.premium_money = 0;
        assert_eq!(
            reset_skills(&mut account, 0, reset_payment::GOLD),
            Err(ActionError::NotEnoughPremiumMoney)
        );
        assert_eq!(account.skills, before.skills);
    }

    #[test]
    fn kits_are_named_and_chosen() {
        let mut account = account();
        let revision = account.kits[1].revision;
        rename_kit(&mut account, 2, "  Sniper ").unwrap();
        assert_eq!(account.kits[1].name, "Sniper");
        assert_eq!(account.kits[1].revision, revision + 1);
        rename_kit(&mut account, 2, &"x".repeat(100)).unwrap();
        assert_eq!(account.kits[1].name.len(), 63);
        assert_eq!(
            rename_kit(&mut account, 9, "x"),
            Err(ActionError::NoSuchKit)
        );
        set_active_kit(&mut account, 2, NOW).unwrap();
        assert_eq!(account.active_kit, 2);
        set_autobuy(&mut account, true);
        assert!(account.autobuy);
    }

    /// Each reason a kit can not play comes out as its bit, every one of a
    /// kit's together (the presets list's `<blocks>`, `presets.md`'s
    /// table); `check_play` says the first.
    #[test]
    fn every_reason_a_kit_can_not_play_is_its_bit() {
        let mut account = account();
        assert_eq!(play_blocks(&account, 1, NOW), Ok(0));
        assert_eq!(kits_play_blocks(&account, NOW), 0);
        assert!(kits_can_play(&account, NOW));
        // No weapon: the second kit, empty.
        assert_eq!(play_blocks(&account, 2, NOW), Ok(PlayBlock::NoWeapon as u8));
        assert_eq!(play_blocks(&account, 9, NOW), Err(ActionError::NoSuchKit));
        // No ammo: the pistol's slot emptied.
        let ammo = account.kits[0].slots[8];
        account.kits[0].slots[8] = None;
        assert_eq!(play_blocks(&account, 1, NOW), Ok(PlayBlock::NoAmmo as u8));
        assert_eq!(check_play(&account, 1, NOW), Err(ActionError::NoAmmo));
        account.kits[0].slots[8] = ammo;
        // A covered slot: the Zubr helmet over a mask.
        let (helmet, balaclava) = owned_zubr_and_balaclava(&mut account);
        account.kits[0].slots[0] = Some(KitSlot {
            item_id: helmet.item_id,
            count: 1,
        });
        account.kits[0].slots[1] = Some(KitSlot {
            item_id: balaclava.item_id,
            count: 1,
        });
        assert_eq!(
            play_blocks(&account, 1, NOW),
            Ok(PlayBlock::CoveredSlot as u8)
        );
        // One type in two pockets too: both reasons.
        let grenade = account.add_item(dict("rgd-5"), 3);
        for slot in [13, 14] {
            account.kits[0].slots[slot] = Some(KitSlot {
                item_id: grenade,
                count: 1,
            });
        }
        assert_eq!(
            play_blocks(&account, 1, NOW),
            Ok(PlayBlock::CoveredSlot as u8 | PlayBlock::PocketTaken as u8)
        );
        assert_eq!(check_play(&account, 1, NOW), Err(ActionError::CoveredSlot));
        account.kits[0].slots[0] = None;
        assert_eq!(
            play_blocks(&account, 1, NOW),
            Ok(PlayBlock::PocketTaken as u8)
        );
        // The kit a match takes is the active one: kit 2 played, no weapon.
        account.active_kit = 2;
        assert_eq!(kits_play_blocks(&account, NOW), PlayBlock::NoWeapon as u8);
        // No kit at all: locked (a kit the account may not use).
        account.kits.clear();
        assert_eq!(kits_play_blocks(&account, NOW), PlayBlock::KitLocked as u8);
        assert!(!kits_can_play(&account, NOW));
    }

    #[test]
    fn play_needs_a_weapon_with_ammo() {
        let mut account = account();
        assert_eq!(check_play(&account, 1, NOW), Ok(()));
        // The second and third kits are empty.
        assert_eq!(check_play(&account, 2, NOW), Err(ActionError::NoWeapon));
        assert_eq!(check_play(&account, 3, NOW), Err(ActionError::NoWeapon));
        assert_eq!(check_play(&account, 9, NOW), Err(ActionError::NoSuchKit));
        // The pistol with its ammo slot emptied.
        let ammo = account.inventory[1].clone();
        move_items(
            &mut account,
            1,
            &[relocation(&ammo, 8, INVENTORY_SLOT, 50)],
            NOW,
        )
        .unwrap();
        assert_eq!(check_play(&account, 1, NOW), Err(ActionError::NoAmmo));
        // Any of the eight ammo slots counts for either weapon: the first
        // live session put the second weapon's ammo beside the first's
        // (slot 9) and the first's stayed in slot 8.
        for slot in [9, 11, 20] {
            move_items(
                &mut account,
                1,
                &[relocation(&ammo, INVENTORY_SLOT, slot, 50)],
                NOW,
            )
            .unwrap();
            assert_eq!(check_play(&account, 1, NOW), Ok(()), "slot {slot}");
            move_items(
                &mut account,
                1,
                &[relocation(&ammo, slot, INVENTORY_SLOT, 50)],
                NOW,
            )
            .unwrap();
        }
        // Ammo of another weapon does not count.
        let dictionary = Dictionary::shared();
        let pistol = account.inventory[0].dict_id;
        let other = dictionary
            .items
            .iter()
            .find(|item| {
                dictionary.fits(item.dict_id, 8) && !dictionary.compatible(pistol, item.dict_id)
            })
            .unwrap()
            .dict_id;
        account.inventory[1].dict_id = other;
        account.kits[0].slots[8] = Some(KitSlot {
            item_id: ammo.item_id,
            count: 50,
        });
        assert_eq!(check_play(&account, 1, NOW), Err(ActionError::NoAmmo));
        // The game's own test loadouts pass.
        for loadout in loadouts::Loadouts::shared().usable() {
            let account = Account::with_loadout("Bob", loadout);
            assert_eq!(check_play(&account, 1, NOW), Ok(()));
        }
    }

    #[test]
    fn every_lobby_faction_is_unlocked() {
        let account = account();
        assert_eq!(unlocked_factions(&account), 0x0F);
        assert!(faction_unlocked(&account, 1) && faction_unlocked(&account, 4));
        assert!(!faction_unlocked(&account, 0) && !faction_unlocked(&account, 5));
    }
}
