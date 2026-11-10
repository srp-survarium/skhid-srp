//! What a match took of an account's supplies. Nothing is bought back for
//! the player: autobuy, the kit window's box, is gone (the owner,
//! 2026-11-10), as is repair (2026-11-08). The player buys in the shop or
//! the ammo window.
//!
//! # What a match is known to have used ([`used`])
//!
//! A match's world is the clients' own. The server knows of a player's
//! supplies only what a trusted snapshot says:
//!
//! 1. **The player's block** (`survarium::v026g::match_msgs::players::
//!    Supplies`, when the match server could read it): what is left of
//!    every stack item of the profile, in store (PROVEN layout) and in the
//!    hands of a player alive (LIKELY: found by the item headers). What
//!    the account owned less what is left is what the match used: ammo,
//!    drugs, grenades, traps alike. Because part of that reading is LIKELY
//!    and no 0.26g snapshot was seen yet, what it says is held to what can
//!    be true:
//!    - never more than a life takes times the lives the player had
//!      (`respawns + 1` times the most a kit's slots hold of the item: a
//!      life takes at most its slots' counts out of the store, 0x2FCC4A;
//!      a respawn is a death or a kit change before play began, which the
//!      games of patch 31 make a respawn in place);
//!    - for ammo, never more than the bullets the statistics counted
//!      (event 22), except buckshot, whose round flies as ten bullets (its
//!      config's `buckshot: 10`) while it is not known whether the counter
//!      counts rounds or bullets;
//!    - a player alive whose slots were not found says nothing.
//! 2. **The counters alone**, when there is no block: event 22, bullets
//!    fired with a kit, proves ammo used only for a kit whose ammo slots
//!    (8, 9, 11, 12) hold **one** kind of ammo that is not buckshot. A kit
//!    with two kinds does not say which was fired. No counter says a drug,
//!    a grenade or a trap was used: without the block none is taken.
//!
//! `SRP_AMMO_USE=off` takes nothing at all (ammo and the rest).
//!
//! # Off the stacks ([`settle`])
//!
//! What was used leaves its stack; a stack used up leaves the inventory.
//! One thing comes back, as it did with autobuy: a stack the game prices at
//! nothing (the starting pistol's ammo) is refilled for nothing, in whole
//! steps of the shop (a clip) [as `rules::buy`'s refill], for every
//! account. No silver is spent, and a new player's first gun is never left
//! without rounds.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params::{item_params, shop_unit};
use survarium::v026g::match_msgs::players::Supplies;
use survarium::v026g::prices;

use super::account::Account;

/// The ammo slots of a kit: two for each weapon.
pub const AMMO_SLOTS: [usize; 4] = [8, 9, 11, 12];
/// The ammo whose round is ten bullets.
pub const BUCKSHOT: &str = "ammo_12mm_buck";

fn is_ammo(dict_id: u16) -> bool {
    item_params(dict_id).is_some_and(|params| params.kind == "ammo")
}

fn owned(account: &Account, dict_id: u16) -> u32 {
    account
        .inventory
        .iter()
        .find(|item| item.dict_id == dict_id)
        .map_or(0, |item| item.condition_or_stack)
}

/// The most a kit's slots hold of `dict_id`: what one life can take.
fn a_life_takes(account: &Account, dict_id: u16) -> u32 {
    account
        .kits
        .iter()
        .map(|kit| {
            kit.slots
                .iter()
                .flatten()
                .filter(|slot| {
                    account
                        .item(slot.item_id)
                        .is_some_and(|item| item.dict_id == dict_id)
                })
                .map(|slot| slot.count)
                .sum::<u32>()
        })
        .max()
        .unwrap_or(0)
}

/// What the counters alone prove: the bullets a kit fired, for a kit with
/// one kind of ammo that is not buckshot. `shots[kit]`.
fn used_by_counters(account: &Account, shots: &[u32], buckshot: Option<u16>) -> Vec<(u16, u32)> {
    let mut used: Vec<(u16, u32)> = Vec::new();
    for (kit, &fired) in account.kits.iter().zip(shots) {
        if fired == 0 {
            continue;
        }
        let mut kinds: Vec<u16> = AMMO_SLOTS
            .iter()
            .filter_map(|&slot| kit.slots[slot])
            .filter_map(|slot| account.item(slot.item_id))
            .map(|item| item.dict_id)
            .filter(|&dict_id| is_ammo(dict_id))
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        let &[dict_id] = kinds.as_slice() else {
            continue;
        };
        if Some(dict_id) == buckshot {
            continue;
        }
        match used.iter_mut().find(|(id, _)| *id == dict_id) {
            Some(entry) => entry.1 = entry.1.saturating_add(fired),
            None => used.push((dict_id, fired)),
        }
    }
    used
}

/// What the match is known to have used of the account's stacks: `(dict
/// id, units)`. `supplies` is the player's block of the last trusted
/// snapshot, `shots[kit]` the bullets its counters say each kit fired,
/// `respawns` how often it began a new life: its deaths and its kit
/// changes before play began. Call it before the match's pay or drop
/// changes a stack. See the module.
pub fn used(
    account: &Account,
    supplies: Option<&Supplies>,
    shots: &[u32],
    respawns: u32,
) -> Vec<(u16, u32)> {
    let dictionary = Dictionary::shared();
    let buckshot = dictionary.item_named(BUCKSHOT).map(|item| item.dict_id);
    let block = supplies.filter(|supplies| !supplies.alive || supplies.inventory_read);
    let mut used = match block {
        Some(supplies) => {
            let fired: u32 = shots
                .iter()
                .fold(0, |sum, &shots| sum.saturating_add(shots));
            let lives = respawns.saturating_add(1);
            supplies
                .left()
                .into_iter()
                .map(|(dict_id, left)| {
                    let mut units = owned(account, dict_id).saturating_sub(left);
                    units = units.min(lives.saturating_mul(a_life_takes(account, dict_id)));
                    if is_ammo(dict_id) && Some(dict_id) != buckshot {
                        units = units.min(fired);
                    }
                    (dict_id, units)
                })
                .collect()
        }
        None => used_by_counters(account, shots, buckshot),
    };
    // Never more than the account owns.
    for (dict_id, units) in &mut used {
        *units = (*units).min(owned(account, *dict_id));
    }
    used.retain(|&(_, units)| units > 0);
    used
}

/// Whether the game prices a step of the shop of `dict_id` at nothing for
/// the account: the shop's price from a seller whose level it has, or a
/// stack without a price.
fn is_free(account: &Account, dict_id: u16) -> bool {
    if prices::price(dict_id).is_some() {
        // Any of the item's sellers that sells to the account.
        return prices::record_for(dict_id, 0, |seller| account.reputation_level(seller))
            .ok()
            .flatten()
            .is_some_and(|record| record.cost == 0);
    }
    Dictionary::shared()
        .item(dict_id)
        .is_some_and(|item| item.is_stack && item.base_cost == 0 && !item.is_premium)
}

/// After a match: take what it used ([`used`]) off the stacks; a stack the
/// game prices at nothing is refilled in whole steps of the shop. Call it
/// before the kits are brought up to the inventory ([`Account::sync_kits`]).
pub fn settle(account: &mut Account, used: &[(u16, u32)]) {
    for &(dict_id, units) in used {
        let refilled = if is_free(account, dict_id) {
            let unit = shop_unit(dict_id);
            units.div_ceil(unit) * unit
        } else {
            0
        };
        if let Some(index) = account
            .inventory
            .iter()
            .position(|item| item.dict_id == dict_id)
        {
            let stack = &mut account.inventory[index].condition_or_stack;
            *stack = stack.saturating_sub(units).saturating_add(refilled);
            if *stack == 0 {
                account.inventory.remove(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::account::KitSlot;
    use survarium::v026g::lobby::FULL_CONDITION;
    use survarium::v026g::match_msgs::players::ItemLeft;

    fn dict(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    /// The AK-74M and `rounds` of its ammo (120 a life) in kit 1, with two
    /// medkits of five owned in a pocket; the starter's pistol and its
    /// ammo moved to kit 2.
    fn rifleman(rounds: u32) -> (Account, u32, u32) {
        let mut account = Account::new("Alice");
        // Drugs are the factions' to sell, each from a level on (ammo from
        // none).
        for faction in 1..=4 {
            account.add_reputation(faction, 40_000);
        }
        let pistol = account.kits[0].slots[7].take().unwrap();
        let pistol_ammo = account.kits[0].slots[8].take().unwrap();
        account.kits[1].slots[7] = Some(pistol);
        account.kits[1].slots[8] = Some(pistol_ammo);
        let rifle = account.add_item(dict("ak_74m"), FULL_CONDITION);
        let ammo = account.add_item(dict("ammo_5.45x39_7n6m"), rounds);
        account.kits[0].slots[7] = Some(KitSlot {
            item_id: rifle,
            count: 1,
        });
        account.kits[0].slots[8] = Some(KitSlot {
            item_id: ammo,
            count: rounds.min(120),
        });
        (account, rifle, ammo)
    }

    /// A block that says `left` of each stack is in store, for a dead
    /// player.
    fn block(left: &[(u16, u32)]) -> Supplies {
        Supplies {
            alive: false,
            kit: 0,
            inventory_read: false,
            items: left
                .iter()
                .map(|&(dict_id, stored)| ItemLeft {
                    dict_id,
                    stack: true,
                    stored,
                    in_hand: None,
                    condition: None,
                    magazine: None,
                    slot: None,
                })
                .collect(),
        }
    }

    #[test]
    fn the_counters_alone_prove_one_kind_of_ammo() {
        let (mut account, _, ammo) = rifleman(300);
        let rounds = dict("ammo_5.45x39_7n6m");
        // 45 bullets with kit 1, whose one ammo is the 5.45.
        assert_eq!(used(&account, None, &[45, 0, 0], 0), [(rounds, 45)]);
        // Both kits fired: each its own ammo.
        assert_eq!(
            used(&account, None, &[45, 7, 0], 0),
            [(rounds, 45), (dict("ammo_7.62x25"), 7)]
        );
        // Never more than the stack.
        assert_eq!(used(&account, None, &[1_000, 0, 0], 0), [(rounds, 300)]);
        // A second kind of ammo in the kit: which was fired is not known.
        let other = account.add_item(dict("ammo_5.45x39_7n10"), 60);
        account.kits[0].slots[9] = Some(KitSlot {
            item_id: other,
            count: 60,
        });
        assert_eq!(used(&account, None, &[45, 0, 0], 0), []);
        // The same kind in both slots is one kind.
        account.kits[0].slots[9] = Some(KitSlot {
            item_id: ammo,
            count: 30,
        });
        assert_eq!(used(&account, None, &[45, 0, 0], 0), [(rounds, 45)]);
        // Buckshot: a round is ten bullets, and the counter's unit is not
        // known.
        let shells = account.add_item(dict(BUCKSHOT), 100);
        account.kits[0].slots[8] = Some(KitSlot {
            item_id: shells,
            count: 30,
        });
        account.kits[0].slots[9] = None;
        assert_eq!(used(&account, None, &[45, 0, 0], 0), []);
        // A kit that fired nothing uses nothing.
        assert_eq!(used(&account, None, &[0, 0, 0], 0), []);
    }

    #[test]
    fn the_players_block_says_what_is_left() {
        let (mut account, _, _) = rifleman(300);
        let rounds = dict("ammo_5.45x39_7n6m");
        let medkit = dict("medkit");
        let kits = account.add_item(medkit, 5);
        account.kits[0].slots[13] = Some(KitSlot {
            item_id: kits,
            count: 2,
        });
        // 240 rounds and 4 medkits left of 300 and 5; 60 bullets counted.
        let supplies = block(&[(rounds, 240), (medkit, 4)]);
        assert_eq!(
            used(&account, Some(&supplies), &[60, 0, 0], 0),
            [(rounds, 60), (medkit, 1)]
        );
        // The block says more ammo is gone than bullets were counted: the
        // bullets hold.
        assert_eq!(
            used(&account, Some(&supplies), &[45, 0, 0], 0),
            [(rounds, 45), (medkit, 1)]
        );
        // It says everything is gone: no more than its lives could take
        // (120 rounds and 2 medkits a life; one death is two lives).
        let empty = block(&[(rounds, 0), (medkit, 0)]);
        assert_eq!(
            used(&account, Some(&empty), &[1_000, 0, 0], 0),
            [(rounds, 120), (medkit, 2)]
        );
        assert_eq!(
            used(&account, Some(&empty), &[1_000, 0, 0], 1),
            [(rounds, 240), (medkit, 4)]
        );
        // More left than owned (a stack bought since): nothing used.
        let more = block(&[(rounds, 500), (medkit, 9)]);
        assert_eq!(used(&account, Some(&more), &[60, 0, 0], 0), []);
        // A player alive whose slots were not found says nothing: the
        // counters alone.
        let mut unread = block(&[(rounds, 0), (medkit, 0)]);
        unread.alive = true;
        assert_eq!(
            used(&account, Some(&unread), &[60, 0, 0], 0),
            [(rounds, 60)]
        );
        // Alive and read: what is in store and in hand.
        let mut alive = block(&[(rounds, 150), (medkit, 3)]);
        alive.alive = true;
        alive.inventory_read = true;
        alive.items[0].in_hand = Some(100);
        alive.items[1].in_hand = Some(1);
        assert_eq!(
            used(&account, Some(&alive), &[60, 0, 0], 0),
            [(rounds, 50), (medkit, 1)]
        );
    }

    /// The owner's case: the AK kit took a PKP (patch 85) and fired 300
    /// rounds with it; the match ends with the PKP in hand. The block (the
    /// match server's read of a picker: the own stacks a pick sets aside as
    /// the kit gives them, neither charged nor refunded) says the own 5.45
    /// is all there: nothing of it is charged, though the kit's counter
    /// says 300 bullets.
    #[test]
    fn a_borrowed_guns_shots_are_not_the_kits_ammo() {
        let (account, _, _) = rifleman(300);
        let rounds = dict("ammo_5.45x39_7n6m");
        // 180 in store, the kit's 120 set aside out of its slot.
        let mut supplies = block(&[(rounds, 180)]);
        supplies.alive = true;
        supplies.inventory_read = true;
        supplies.items[0].in_hand = Some(120);
        supplies.items[0].slot = Some(8);
        assert_eq!(used(&account, Some(&supplies), &[300, 0, 0], 0), []);
        // Without the block the fallback, the kit's counter, charges the 300.
        assert_eq!(used(&account, None, &[300, 0, 0], 0), [(rounds, 300)]);
    }

    #[test]
    fn what_was_used_is_gone_and_nothing_is_bought() {
        let (mut account, rifle, ammo) = rifleman(300);
        account.item_mut(rifle).unwrap().condition_or_stack = 900_000;
        account.generic_money = 100_000;
        let rounds = dict("ammo_5.45x39_7n6m");
        let medkit = dict("medkit");
        let kits = account.add_item(medkit, 5);
        settle(&mut account, &[(rounds, 45), (medkit, 2)]);
        assert_eq!(account.item(ammo).unwrap().condition_or_stack, 255);
        assert_eq!(account.item(kits).unwrap().condition_or_stack, 3);
        assert_eq!(account.item(rifle).unwrap().condition_or_stack, 900_000);
        assert_eq!(account.generic_money, 100_000);
        // A stack used up leaves the inventory and its slot.
        settle(&mut account, &[(rounds, 255)]);
        assert!(account.item(ammo).is_none());
        assert!(account.sync_kits());
        assert_eq!(account.kits[0].slots[8], None);
    }

    #[test]
    fn the_starting_ammo_is_refilled_for_nothing() {
        let mut account = Account::new("Alice");
        let money = account.generic_money;
        let ammo = account.kits[0].slots[8].unwrap().item_id;
        let rounds = dict("ammo_7.62x25");
        let unit = shop_unit(rounds);
        settle(&mut account, &[(rounds, 10)]);
        // A whole clip back, for the 0 the game prices it at.
        assert_eq!(account.generic_money, money);
        assert_eq!(
            account.item(ammo).unwrap().condition_or_stack,
            50 - 10 + unit
        );
    }
}
