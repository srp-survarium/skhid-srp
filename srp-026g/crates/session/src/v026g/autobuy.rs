//! What a match took of an account's supplies, and the two boxes of the
//! kit window that put it right: autobuy and auto-repair (`49 01`, the
//! lobby's number: bit 0 and bit 1).
//!
//! The money the server spends this way is shown on the results screen:
//! the three lists that end `89` (`lobby.md` §2.9), "Ammunition", "Special
//! equipment" and "Repair cost", rows of `(silver, dict id)`.
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
//! # The boxes ([`settle`])
//!
//! - **Autobuy** buys back what was taken, in whole steps of the shop (a
//!   clip of ammo, one of anything else), as many as the silver pays for
//!   [0.21d's rule, `survarium::v021d::supplies`], at the shop's price for
//!   an account that may buy the item there; ammo the game prices at
//!   nothing (the starting pistol's) is refilled for nothing [as
//!   `rules::buy`'s refill]. What the account may not buy is not bought.
//! - **Auto-repair** repairs every item the match wore, for silver, by the
//!   lobby's own repair ([`super::items_rules::repair`], `47 03` paid in silver), in the
//!   order the match wore them, while the silver lasts.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params::{item_params, shop_unit};
use survarium::v026g::lobby::requests::RepairPayment;
use survarium::v026g::lobby::{SpentMoney, limits};
use survarium::v026g::match_msgs::players::Supplies;
use survarium::v026g::prices;

use super::account::Account;

/// The ammo slots of a kit: two for each weapon.
pub const AMMO_SLOTS: [usize; 4] = [8, 9, 11, 12];
/// The ammo whose round is ten bullets.
pub const BUCKSHOT: &str = "ammo_12mm_buck";

/// What a match's supplies did to an account.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled {
    /// `(dict id, units)` bought back.
    pub bought: Vec<(u16, u32)>,
    /// `(item id, silver)`: the repairs made.
    pub repaired: Vec<(u32, u32)>,
    /// What the results screen lists.
    pub spent: SpentMoney,
}

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

/// What a step of the shop (a clip of ammo, else one item) of `dict_id`
/// costs the account, if it may buy it: the shop's price from a seller
/// whose level it has, or nothing for a stack the game prices at nothing.
fn step_cost(account: &Account, dict_id: u16) -> Option<u32> {
    if prices::price(dict_id).is_some() {
        // Any of the item's sellers that sells to the account.
        return prices::record_for(dict_id, 0, |seller| account.reputation_level(seller))
            .ok()
            .flatten()
            .map(|record| record.cost);
    }
    Dictionary::shared()
        .item(dict_id)
        .filter(|item| item.is_stack && item.base_cost == 0 && !item.is_premium)
        .map(|_| 0)
}

/// After a match: take what it used ([`used`]) off the stacks, buy it back
/// if the account's autobuy is on, and repair the items the match wore
/// (`worn`, by item id) if its auto-repair is. Call it before the kits are
/// brought up to the inventory ([`Account::sync_kits`]).
pub fn settle(account: &mut Account, used: &[(u16, u32)], worn: &[u32], now: u64) -> Settled {
    let mut settled = Settled::default();
    for &(dict_id, units) in used {
        let mut bought = 0;
        if account.autobuy
            && let Some(cost) = step_cost(account, dict_id)
        {
            let unit = shop_unit(dict_id);
            let wanted = units.div_ceil(unit);
            let steps = match cost {
                0 => wanted,
                cost => wanted.min(account.generic_money / cost),
            };
            let silver = steps * cost;
            account.generic_money -= silver;
            bought = steps * unit;
            if bought > 0 {
                settled.bought.push((dict_id, bought));
            }
            let (list, most) = if is_ammo(dict_id) {
                (&mut settled.spent.autobuy_ammo, limits::SPENT_AUTOBUY_AMMO)
            } else {
                (
                    &mut settled.spent.autobuy_consumables,
                    limits::SPENT_AUTOBUY_CONSUMABLES,
                )
            };
            if silver > 0 && list.len() < most {
                list.push((silver, dict_id));
            }
        }
        if let Some(index) = account
            .inventory
            .iter()
            .position(|item| item.dict_id == dict_id)
        {
            let stack = &mut account.inventory[index].condition_or_stack;
            *stack = stack.saturating_sub(units).saturating_add(bought);
            if *stack == 0 {
                account.inventory.remove(index);
            }
        }
    }
    if account.extras.auto_repair {
        for &item_id in worn {
            let Some(dict_id) = account.item(item_id).map(|item| item.dict_id) else {
                continue;
            };
            let before = account.generic_money;
            // The items rule: it mends defects too (the plain one does not).
            if super::items_rules::repair(account, item_id, RepairPayment::Silver, now).is_err() {
                continue;
            }
            let silver = before.saturating_sub(account.generic_money);
            settled.repaired.push((item_id, silver));
            if silver > 0 && settled.spent.repair.len() < limits::SPENT_REPAIR {
                settled.spent.repair.push((silver, dict_id));
            }
        }
    }
    settled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::account::KitSlot;
    use survarium::v026g::lobby::FULL_CONDITION;
    use survarium::v026g::lobby::client_model::{Client, Decoded};
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
    /// match server's read of the record's parked own kit) says the own
    /// 5.45 is all there: nothing of it is charged, though the kit's
    /// counter says 300 bullets.
    #[test]
    fn a_borrowed_guns_shots_are_not_the_kits_ammo() {
        let (account, _, _) = rifleman(300);
        let rounds = dict("ammo_5.45x39_7n6m");
        // 180 in store, 120 parked out of the kit's slot (and no magazine
        // rounds of the AK left: it was emptied before the pickup).
        let mut supplies = block(&[(rounds, 180)]);
        supplies.alive = true;
        supplies.inventory_read = true;
        supplies.items[0].in_hand = Some(120);
        supplies.items[0].slot = Some(8);
        assert_eq!(used(&account, Some(&supplies), &[300, 0, 0], 0), []);
        // Had it fired 40 of its own before the pickup (80 parked): 40.
        supplies.items[0].in_hand = Some(80);
        assert_eq!(used(&account, Some(&supplies), &[340, 0, 0], 0), [(rounds, 40)]);
        // Without the record's parked kit the old fallback charged the 300.
        assert_eq!(used(&account, None, &[300, 0, 0], 0), [(rounds, 300)]);
    }

    #[test]
    fn without_the_boxes_what_was_used_is_gone_and_nothing_is_spent() {
        let (mut account, rifle, ammo) = rifleman(300);
        // Autobuy is on for a new account: the player turned it off.
        account.autobuy = false;
        account.item_mut(rifle).unwrap().condition_or_stack = 900_000;
        let money = account.generic_money;
        let rounds = dict("ammo_5.45x39_7n6m");
        let settled = settle(&mut account, &[(rounds, 45)], &[rifle], 0);
        assert_eq!(settled, Settled::default());
        assert_eq!(account.item(ammo).unwrap().condition_or_stack, 255);
        assert_eq!(account.item(rifle).unwrap().condition_or_stack, 900_000);
        assert_eq!(account.generic_money, money);
        // A stack used up leaves the inventory and its slot.
        settle(&mut account, &[(rounds, 255)], &[], 0);
        assert!(account.item(ammo).is_none());
        assert!(account.sync_kits());
        assert_eq!(account.kits[0].slots[8], None);
    }

    #[test]
    fn autobuy_buys_back_whole_clips_within_the_money() {
        let (mut account, _, ammo) = rifleman(300);
        account.autobuy = true;
        account.generic_money = 1_000;
        let rounds = dict("ammo_5.45x39_7n6m");
        // 45 used: two clips of 30 at 30 silver (`prices::AMMO_PRICES`).
        let settled = settle(&mut account, &[(rounds, 45)], &[], 0);
        assert_eq!(settled.bought, [(rounds, 60)]);
        assert_eq!(settled.spent.autobuy_ammo, [(60, rounds)]);
        assert!(settled.spent.autobuy_consumables.is_empty());
        assert_eq!(account.item(ammo).unwrap().condition_or_stack, 315);
        assert_eq!(account.generic_money, 940);
        // Silver for one clip only.
        account.generic_money = 50;
        let settled = settle(&mut account, &[(rounds, 90)], &[], 0);
        assert_eq!(settled.bought, [(rounds, 30)]);
        assert_eq!(settled.spent.autobuy_ammo, [(30, rounds)]);
        assert_eq!(account.item(ammo).unwrap().condition_or_stack, 255);
        assert_eq!(account.generic_money, 20);
        // No silver: nothing bought, nothing listed.
        account.generic_money = 0;
        let settled = settle(&mut account, &[(rounds, 30)], &[], 0);
        assert_eq!(settled, Settled::default());
        assert_eq!(account.item(ammo).unwrap().condition_or_stack, 225);
    }

    #[test]
    fn ammo_is_bought_back_at_no_reputation_and_a_drug_is_not() {
        let (mut account, _, ammo) = rifleman(300);
        account.autobuy = true;
        account.generic_money = 1_000;
        // Known to no seller.
        for (_, points) in &mut account.reputations {
            *points = 0;
        }
        let rounds = dict("ammo_5.45x39_7n6m");
        let medkit = dict("medkit");
        let kits = account.add_item(medkit, 5);
        let settled = settle(&mut account, &[(rounds, 30), (medkit, 2)], &[], 0);
        // The clip came back; the medkits (Scavengers, level 3) did not.
        assert_eq!(settled.bought, [(rounds, 30)]);
        assert_eq!(account.item(ammo).unwrap().condition_or_stack, 300);
        assert_eq!(account.item(kits).unwrap().condition_or_stack, 3);
        assert_eq!(account.generic_money, 970);
    }

    #[test]
    fn consumables_are_bought_back_one_by_one_in_their_own_list() {
        let (mut account, _, _) = rifleman(300);
        account.autobuy = true;
        account.generic_money = 1_000;
        let medkit = dict("medkit");
        let kits = account.add_item(medkit, 5);
        let cost = prices::price(medkit).unwrap().cost;
        let settled = settle(&mut account, &[(medkit, 2)], &[], 0);
        assert_eq!(settled.bought, [(medkit, 2)]);
        assert_eq!(settled.spent.autobuy_consumables, [(2 * cost, medkit)]);
        assert!(settled.spent.autobuy_ammo.is_empty());
        assert_eq!(account.item(kits).unwrap().condition_or_stack, 5);
        assert_eq!(account.generic_money, 1_000 - 2 * cost);
    }

    #[test]
    fn the_starting_ammo_is_refilled_for_nothing() {
        let mut account = Account::new("Alice");
        account.autobuy = true;
        let money = account.generic_money;
        let ammo = account.kits[0].slots[8].unwrap().item_id;
        let rounds = dict("ammo_7.62x25");
        let unit = shop_unit(rounds);
        let settled = settle(&mut account, &[(rounds, 10)], &[], 0);
        // A whole clip back, for the 0 the game prices it at: no row.
        assert_eq!(settled.bought, [(rounds, unit)]);
        assert_eq!(settled.spent, SpentMoney::default());
        assert_eq!(account.generic_money, money);
        assert_eq!(
            account.item(ammo).unwrap().condition_or_stack,
            50 - 10 + unit
        );
    }

    #[test]
    fn auto_repair_pays_the_lobbys_price() {
        let (mut account, rifle, _) = rifleman(300);
        account.extras.auto_repair = true;
        account.generic_money = 100_000;
        account.item_mut(rifle).unwrap().condition_or_stack = 900_000;
        let settled = settle(&mut account, &[], &[rifle], 0);
        let cost = 100_000 - account.generic_money;
        assert!(cost > 0);
        assert_eq!(settled.repaired, [(rifle, cost)]);
        assert_eq!(settled.spent.repair, [(cost, dict("ak_74m"))]);
        assert_eq!(
            account.item(rifle).unwrap().condition_or_stack,
            FULL_CONDITION
        );
        // Nothing worn, or no silver for it: no repair.
        assert_eq!(settle(&mut account, &[], &[rifle], 0), Settled::default());
        account.item_mut(rifle).unwrap().condition_or_stack = 500_000;
        account.generic_money = 1;
        assert_eq!(settle(&mut account, &[], &[rifle], 0), Settled::default());
        assert_eq!(account.item(rifle).unwrap().condition_or_stack, 500_000);
    }

    #[test]
    fn the_spent_lists_go_out_with_the_inventory() {
        let (mut account, rifle, _) = rifleman(300);
        account.autobuy = true;
        account.extras.auto_repair = true;
        account.generic_money = 100_000;
        account.item_mut(rifle).unwrap().condition_or_stack = 900_000;
        let rounds = dict("ammo_5.45x39_7n6m");
        let settled = settle(&mut account, &[(rounds, 45)], &[rifle], 0);
        account.sync_kits();
        let message = super::super::wire::inventory_with_spent(&account, &settled.spent).unwrap();
        let mut client = Client::signed_in();
        let Decoded::Inventory { spent, .. } = client.receive(&message).unwrap() else {
            panic!("not the inventory");
        };
        assert_eq!(spent, settled.spent);
        assert_eq!(spent.autobuy_ammo.len(), 1);
        assert_eq!(spent.repair.len(), 1);
        // The tail: one ammo row, no consumables, one repair row.
        let body = message.body();
        let tail = &body[body.len() - 15..];
        assert_eq!(tail[0], 1);
        assert_eq!(tail[1..5], 60_u32.to_le_bytes());
        assert_eq!(tail[5..7], rounds.to_le_bytes());
        assert_eq!(tail[7], 0);
        assert_eq!(tail[8], 1);
        assert_eq!(tail[13..15], dict("ak_74m").to_le_bytes());
    }
}
