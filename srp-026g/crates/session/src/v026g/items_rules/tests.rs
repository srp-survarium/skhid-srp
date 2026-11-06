//! The item rules, each answer read by the strict client model
//! (`survarium::v026g::lobby::client_model`): after the answer the client
//! must hold what the account does.

use survarium::v026g::lobby::client_model::Client;
use survarium::v026g::lobby::{self, ItemDescr, ServerMessage};

use super::*;
use crate::v026g::account::{KitSlot, Reroll};
use crate::v026g::wire;

const NOW: u64 = 1_700_000_000;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

fn decal() -> u16 {
    Dictionary::shared()
        .items
        .iter()
        .find(|item| item.category == 26)
        .unwrap()
        .dict_id
}

/// An account with money and nothing but what a test adds.
fn rich() -> Account {
    let mut account = Account::new("Alice");
    account.generic_money = 1_000_000;
    account.premium_money = 1_000;
    account
}

/// A whole item of `name` with these modifications.
fn give(account: &mut Account, name: &str, modifications: &[u32]) -> u32 {
    let item_id = account.add_item(dict(name), FULL_CONDITION);
    account.item_mut(item_id).unwrap().modifications =
        modifications.iter().map(|&id| (id, false)).collect();
    item_id
}

/// A client that holds the account.
fn client_of(account: &Account) -> Client {
    let mut client = Client::signed_in();
    client
        .receive_all(&wire::after_change(account, NOW).unwrap())
        .unwrap();
    client
}

fn bytes(message: &ServerMessage) -> Vec<u8> {
    let mut out = vec![message.id_byte()];
    out.extend_from_slice(message.body());
    out
}

/// The client takes `message` and then holds the account's items.
#[track_caller]
fn takes(client: &mut Client, message: &ServerMessage, account: &Account) {
    client.receive(message).unwrap();
    let held: Vec<ItemDescr> = account
        .inventory
        .iter()
        .map(|item| wire::item_descr_at(item, NOW))
        .collect();
    let mut seen = client.inventory().to_vec();
    // The order of an item's upgrades is the client's own.
    let by_slot = |items: &mut Vec<ItemDescr>| {
        for item in items {
            item.upgrades.sort_by_key(|upgrade| upgrade.param);
        }
    };
    let mut held = held;
    by_slot(&mut held);
    by_slot(&mut seen);
    assert_eq!(seen, held);
}

#[test]
fn cosmetics_go_on_and_come_off() {
    let mut account = rich();
    let vest = give(&mut account, "edge_torso_2", &[]);
    let other = give(&mut account, "bm_torso_1", &[]);
    let rifle = give(&mut account, "ak_74m", &[]);
    let camo = account.add_item(dict("camo_city"), FULL_CONDITION);
    let first = account.add_item(decal(), FULL_CONDITION);
    let second = account.add_item(decal(), FULL_CONDITION);
    account.kits[0].slots[1] = None;
    let torso_slot = (0..23)
        .find(|&slot| Dictionary::shared().fits(dict("edge_torso_2"), slot))
        .unwrap();
    account.kits[0].slots[usize::from(torso_slot)] = Some(KitSlot {
        item_id: vest,
        count: 1,
    });
    let revision = account.kits[0].revision;
    let mut client = client_of(&account);

    // The camouflage into slot 3, byte for byte.
    let attached = attach_upgrade(&mut account, vest, camo, 3).unwrap();
    assert_eq!(
        attached,
        Attached {
            host_item_id: vest,
            upgrade_item_id: camo,
            slot: 3,
            value: 0,
            detach_first: false
        }
    );
    let message = lobby::permitted_upgrade_attached(vest, camo, 3, 0, false);
    let mut expected = vec![0x83, 0x46, 0x01];
    expected.extend(vest.to_le_bytes());
    expected.extend(camo.to_le_bytes());
    expected.extend([3, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(bytes(&message), expected);
    takes(&mut client, &message, &account);
    assert_eq!(
        account.item(vest).unwrap().upgrades,
        [Upgrade {
            id: camo,
            a: dict("camo_city").into(),
            value: 0,
            param: 3
        }]
    );
    // The kit that wears the vest shows another one.
    assert_eq!(account.kits[0].revision, revision + 1);
    assert_eq!(account.kits[1].revision, 1);
    // It is still an item of the inventory.
    assert!(account.item(camo).is_some());

    // Two decals; a third has no slot, and a decal is no camouflage.
    for (decal, slot) in [(first, 0), (second, 1)] {
        attach_upgrade(&mut account, vest, decal, slot).unwrap();
        let message = lobby::permitted_upgrade_attached(vest, decal, slot, 0, false);
        takes(&mut client, &message, &account);
    }
    assert_eq!(account.item(vest).unwrap().upgrades.len(), 3);
    for (upgrade, slot) in [(first, 3), (camo, 0), (first, 2), (rifle, 0)] {
        assert_eq!(
            attach_upgrade(&mut account, vest, upgrade, slot),
            Err(ItemError::Upgrade("it does not go into that slot"))
        );
    }
    // Nothing goes on a rifle, on itself, or from nowhere.
    assert!(attach_upgrade(&mut account, rifle, camo, 3).is_err());
    assert!(attach_upgrade(&mut account, vest, vest, 3).is_err());
    assert!(attach_upgrade(&mut account, vest, 999, 3).is_err());
    assert!(attach_upgrade(&mut account, 999, camo, 3).is_err());
    assert_eq!(ItemError::Upgrade("").text_key(), "st_cannot_upgrade_item");

    // A decal moved to the other slot comes off the first and takes the
    // place of the one there.
    let before_move = account.kits[0].revision;
    let moved = attach_upgrade(&mut account, vest, first, 1).unwrap();
    assert_eq!(account.kits[0].revision, before_move + 1);
    assert!(moved.detach_first);
    let message = lobby::permitted_upgrade_attached(vest, first, 1, 0, true);
    takes(&mut client, &message, &account);
    let slots: Vec<(u32, u8)> = account
        .item(vest)
        .unwrap()
        .upgrades
        .iter()
        .map(|upgrade| (upgrade.id, upgrade.param))
        .collect();
    assert_eq!(slots, [(camo, 3), (first, 1)]);
    // Placed, then moved back: its place goes with it, in the answer too.
    place_upgrade(&mut account, vest, first, 0x8130_4601_2008_6E4E).unwrap();
    let back = attach_upgrade(&mut account, vest, first, 0).unwrap();
    assert!(back.detach_first);
    assert_eq!(back.value, 0x8130_4601_2008_6E4E);
    let message = lobby::permitted_upgrade_attached(vest, first, 0, 0x8130_4601_2008_6E4E, true);
    takes(&mut client, &message, &account);
    let moved = attach_upgrade(&mut account, vest, first, 1).unwrap();
    let message = lobby::permitted_upgrade_attached(vest, first, 1, moved.value, true);
    takes(&mut client, &message, &account);
    assert_eq!(moved.value, 0x8130_4601_2008_6E4E);

    // The camouflage onto the other vest too: this one keeps it.
    let shared = attach_upgrade(&mut account, other, camo, 3).unwrap();
    assert!(!shared.detach_first);
    let message = lobby::permitted_upgrade_attached(other, camo, 3, 0, false);
    takes(&mut client, &message, &account);
    assert_eq!(account.item(vest).unwrap().upgrades.len(), 2);
    assert_eq!(account.item(other).unwrap().upgrades[0].id, camo);
    // Again into the same slot: nothing changes.
    let again = attach_upgrade(&mut account, other, camo, 3).unwrap();
    assert!(!again.detach_first);
    assert_eq!(account.item(other).unwrap().upgrades.len(), 1);

    // The decal editor's value (a vest: nothing to send).
    assert!(!place_upgrade(&mut account, vest, first, 0x0102_0304_0506_0708).unwrap());
    let held = |account: &Account, host: u32, upgrade: u32| {
        account
            .item(host)
            .unwrap()
            .upgrades
            .iter()
            .find(|held| held.id == upgrade)
            .copied()
    };
    assert_eq!(
        held(&account, vest, first).unwrap().value,
        0x0102_0304_0506_0708
    );
    assert!(place_upgrade(&mut account, other, first, 1).is_err());
    assert!(place_upgrade(&mut account, 999, first, 1).is_err());
    // The client wrote it itself: the next inventory agrees with it.
    client
        .receive_all(&wire::after_change(&account, NOW).unwrap())
        .unwrap();

    // Off the other vest, byte for byte; the first keeps it.
    assert_eq!(detach_upgrade(&mut account, other, camo), Ok(Detached::Off));
    let message = lobby::permitted_upgrade_detached(camo, other);
    let mut expected = vec![0x83, 0x46, 0x02];
    expected.extend(camo.to_le_bytes());
    expected.extend(other.to_le_bytes());
    assert_eq!(bytes(&message), expected);
    takes(&mut client, &message, &account);
    assert!(held(&account, vest, camo).is_some());
    assert_eq!(
        detach_upgrade(&mut account, other, camo),
        Err(ItemError::Upgrade("the cosmetic is not on that item"))
    );
    assert!(detach_upgrade(&mut account, 999, camo).is_err());

    // A cosmetic that is sold is on no item any more, and `46 02` for it
    // (Flash's on_item_sold sends it after the sale) does nothing.
    crate::v026g::rules::sell(&mut account, first, 1).unwrap();
    assert!(held(&account, vest, first).is_none());
    let before = account.clone();
    assert_eq!(
        detach_upgrade(&mut account, vest, first),
        Ok(Detached::Gone)
    );
    assert_eq!(account, before);
}

#[test]
fn one_cosmetic_is_worn_by_many_items() {
    let mut account = rich();
    let vests: Vec<u32> = ["edge_torso_2", "bm_torso_1", "edge_torso_2"]
        .into_iter()
        .map(|name| give(&mut account, name, &[]))
        .collect();
    let camo = account.add_item(dict("camo_city"), FULL_CONDITION);
    let decal = account.add_item(decal(), FULL_CONDITION);
    let mut client = client_of(&account);
    let items = account.inventory.len();
    for &vest in &vests {
        for (upgrade, slot) in [(camo, 3), (decal, 0)] {
            let attached = attach_upgrade(&mut account, vest, upgrade, slot).unwrap();
            assert!(!attached.detach_first);
            let message = lobby::permitted_upgrade_attached(vest, upgrade, slot, 0, false);
            takes(&mut client, &message, &account);
        }
    }
    // No item more: the same two are on each vest.
    assert_eq!(account.inventory.len(), items);
    // Each vest has its own slot and place for the decal.
    attach_upgrade(&mut account, vests[1], decal, 1).unwrap();
    for (index, &vest) in vests.iter().enumerate() {
        place_upgrade(&mut account, vest, decal, index as u64 + 1).unwrap();
    }
    let decal_on = |account: &Account, vest: u32| {
        account
            .item(vest)
            .unwrap()
            .upgrades
            .iter()
            .find(|held| held.id == decal)
            .map(|held| (held.param, held.value))
    };
    assert_eq!(decal_on(&account, vests[0]), Some((0, 1)));
    assert_eq!(decal_on(&account, vests[1]), Some((1, 2)));
    assert_eq!(decal_on(&account, vests[2]), Some((0, 3)));
    client
        .receive_all(&wire::after_change(&account, NOW).unwrap())
        .unwrap();
    // Off one: the others keep it.
    assert_eq!(
        detach_upgrade(&mut account, vests[0], decal),
        Ok(Detached::Off)
    );
    takes(
        &mut client,
        &lobby::permitted_upgrade_detached(decal, vests[0]),
        &account,
    );
    assert_eq!(decal_on(&account, vests[0]), None);
    assert_eq!(decal_on(&account, vests[2]), Some((0, 3)));
    // The camouflage sold: off every vest, paid once.
    let money = account.generic_money;
    crate::v026g::rules::sell(&mut account, camo, 1).unwrap();
    assert!(account.generic_money > money);
    for &vest in &vests {
        assert!(
            account
                .item(vest)
                .unwrap()
                .upgrades
                .iter()
                .all(|held| held.id != camo)
        );
    }
    assert_eq!(decal_on(&account, vests[2]), Some((0, 3)));
}

/// What a load does to an account (`Account::repair`, with `make_safe` for
/// each item, and `sync_kits`) keeps a cosmetic worn by many items on each.
#[test]
fn a_cosmetic_on_many_items_is_kept_by_the_load() {
    let mut account = rich();
    let vests: Vec<u32> = ["edge_torso_2", "bm_torso_1", "edge_torso_2"]
        .into_iter()
        .map(|name| give(&mut account, name, &[]))
        .collect();
    let camo = account.add_item(dict("camo_city"), FULL_CONDITION);
    let decal = account.add_item(decal(), FULL_CONDITION);
    for (index, &vest) in vests.iter().enumerate() {
        attach_upgrade(&mut account, vest, camo, 3).unwrap();
        attach_upgrade(&mut account, vest, decal, (index % 2) as u8).unwrap();
        place_upgrade(&mut account, vest, decal, index as u64 + 1).unwrap();
    }
    let worn = account.clone();
    assert!(!account.repair());
    assert!(!account.sync_kits());
    assert_eq!(account, worn);
    for &vest in &vests {
        assert_eq!(account.item(vest).unwrap().upgrades.len(), 2);
    }
    client_of(&account);
}

#[test]
fn a_cosmetic_put_on_another_item_stays_on_the_first() {
    for (cosmetic_dict, cosmetic_slot) in [(dict("camo_city"), 3), (decal(), 0)] {
        for equip_destination in [false, true] {
            let mut account = rich();
            let source = give(&mut account, "edge_torso_2", &[]);
            let destination = give(&mut account, "bm_torso_1", &[]);
            let cosmetic = account.add_item(cosmetic_dict, FULL_CONDITION);
            let torso = (0..23)
                .find(|&slot| Dictionary::shared().fits(dict("edge_torso_2"), slot))
                .unwrap() as usize;
            // Two alternate kits share the source; the third either wears
            // the destination or is unrelated and must keep its revision.
            for kit in &mut account.kits[..2] {
                kit.slots[torso] = Some(KitSlot {
                    item_id: source,
                    count: 1,
                });
            }
            if equip_destination {
                account.kits[2].slots[torso] = Some(KitSlot {
                    item_id: destination,
                    count: 1,
                });
            }
            attach_upgrade(&mut account, source, cosmetic, cosmetic_slot).unwrap();
            let before: Vec<_> = account.kits.iter().map(|kit| kit.revision).collect();
            let shared =
                attach_upgrade(&mut account, destination, cosmetic, cosmetic_slot).unwrap();
            assert!(!shared.detach_first);
            assert_eq!(account.item(source).unwrap().upgrades[0].id, cosmetic);
            assert_eq!(account.item(destination).unwrap().upgrades[0].id, cosmetic);
            // Only the kits wearing the destination show another item.
            assert_eq!(account.kits[0].revision, before[0]);
            assert_eq!(account.kits[1].revision, before[1]);
            assert_eq!(
                account.kits[2].revision,
                before[2] + u32::from(equip_destination)
            );
        }
    }
}

/// The scope pairs of these tests. The test data has no variant (the
/// players' data before the release patch: no pair at all), so the table
/// is made up over items the dictionary has, which the wire needs for an
/// owned item: 0.26g's own Leupold (69) and PSO-1 (304) stand for two
/// shop scopes. The variants' stand-ins are other ids the dictionary has
/// (the 1P69 and two rifles): the wire sends only a record whose dict id is
/// an item of the game data (`wire::item_descr_at`).
const SCOPE: u16 = 69;
const OTHER_SCOPE: u16 = 304;
const AK_VARIANT: u16 = 362;
const AK_OTHER_VARIANT: u16 = 363;
const SVD_VARIANT: u16 = 364;

fn scope_table() -> ScopeFits {
    use survarium::v026g::scopes::Fit;
    let fit = |weapon: &str, scope, variant| Fit {
        weapon: dict(weapon),
        scope,
        variant,
    };
    ScopeFits::new(vec![
        fit("ak_74n", SCOPE, AK_VARIANT),
        fit("ak_74n_premium", SCOPE, AK_VARIANT),
        fit("svd", SCOPE, SVD_VARIANT),
        fit("ak_74n", OTHER_SCOPE, AK_OTHER_VARIANT),
    ])
    .0
}

fn scope_record(scope_item_id: u32, variant: u16) -> Upgrade {
    Upgrade {
        id: scope_item_id,
        a: variant.into(),
        value: 0,
        param: 3,
    }
}

/// The records on `host`.
fn records(account: &Account, host: u32) -> Vec<Upgrade> {
    account.item(host).unwrap().upgrades.clone()
}

#[test]
fn a_scope_is_bought_once_and_put_on_many_weapons() {
    let table = scope_table();
    let mut account = rich();
    let ak = give(&mut account, "ak_74n", &[]);
    let ak_premium = give(&mut account, "ak_74n_premium", &[]);
    let svd = give(&mut account, "svd", &[]);
    let rifle = give(&mut account, "ak_74m", &[]);
    let vest = give(&mut account, "edge_torso_2", &[]);
    let scope = account.add_item(SCOPE, FULL_CONDITION);
    let other = account.add_item(OTHER_SCOPE, FULL_CONDITION);
    let mut client = client_of(&account);
    let items = account.inventory.len();

    // On three weapons, each holding the variant made for it; the scope
    // stays one item of the inventory.
    for (host, variant) in [
        (ak, AK_VARIANT),
        (ak_premium, AK_VARIANT),
        (svd, SVD_VARIANT),
    ] {
        let attached = attach_upgrade_in(&mut account, &table, host, scope, 3).unwrap();
        assert_eq!(
            attached,
            Attached {
                host_item_id: host,
                upgrade_item_id: scope,
                slot: 3,
                value: 0,
                detach_first: false
            }
        );
        assert_eq!(records(&account, host), [scope_record(scope, variant)]);
        // The client's own copy after `83 46 01` holds the scope item's
        // dict id (0x275671); the inventory that follows every answer
        // (`after_change`) puts the variant's in its place.
        client
            .receive(&lobby::permitted_upgrade_attached(host, scope, 3, 0, false))
            .unwrap();
        let held = |client: &Client| {
            let item = client.inventory().iter().find(|item| item.item_id == host);
            item.unwrap().upgrades[0].a
        };
        assert_eq!(held(&client), u32::from(SCOPE));
        client
            .receive_all(&wire::after_change(&account, NOW).unwrap())
            .unwrap();
        assert_eq!(held(&client), u32::from(variant));
    }
    assert_eq!(account.inventory.len(), items);
    assert!(account.item(scope).unwrap().upgrades.is_empty());
    // Again on the same weapon: nothing changes.
    let again = attach_upgrade_in(&mut account, &table, ak, scope, 3).unwrap();
    assert!(!again.detach_first);
    assert_eq!(records(&account, ak), [scope_record(scope, AK_VARIANT)]);

    // No variant for the pair, another slot, a vest, the scope on itself:
    // refused, nothing changes.
    let before = account.clone();
    for (host, upgrade, slot) in [
        (rifle, scope, 3),
        (svd, other, 3),
        (vest, scope, 3),
        (ak, scope, 0),
        (ak, scope, 1),
        (scope, scope, 3),
    ] {
        assert!(
            attach_upgrade_in(&mut account, &table, host, upgrade, slot).is_err(),
            "{host} {upgrade} {slot}"
        );
    }
    assert_eq!(account, before);

    // A second scope on a weapon replaces the first there, only there.
    let replaced = attach_upgrade_in(&mut account, &table, ak, other, 3).unwrap();
    assert!(!replaced.detach_first);
    assert_eq!(
        records(&account, ak),
        [scope_record(other, AK_OTHER_VARIANT)]
    );
    assert_eq!(
        records(&account, ak_premium),
        [scope_record(scope, AK_VARIANT)]
    );
    assert_eq!(records(&account, svd), [scope_record(scope, SVD_VARIANT)]);

    // Off one weapon: the others keep it (on that one a stock scope comes
    // back: the client builds it from its plain config).
    assert_eq!(detach_upgrade(&mut account, svd, scope), Ok(Detached::Off));
    assert!(records(&account, svd).is_empty());
    assert_eq!(
        records(&account, ak_premium),
        [scope_record(scope, AK_VARIANT)]
    );
    client
        .receive(&lobby::permitted_upgrade_detached(scope, svd))
        .unwrap();
    takes(
        &mut client,
        &wire::after_change(&account, NOW).unwrap()[0],
        &account,
    );

    // The scope gone (sold): off every weapon; the other scope stays.
    attach_upgrade_in(&mut account, &table, svd, scope, 3).unwrap();
    account.inventory.retain(|item| item.item_id != scope);
    let mut stripped = drop_gone_upgrades_in(&mut account.inventory, &table);
    stripped.sort_unstable();
    assert_eq!(stripped, [ak_premium, svd]);
    assert!(records(&account, ak_premium).is_empty());
    assert!(records(&account, svd).is_empty());
    assert_eq!(
        records(&account, ak),
        [scope_record(other, AK_OTHER_VARIANT)]
    );
    assert!(drop_gone_upgrades_in(&mut account.inventory, &table).is_empty());
}

/// A client without patch 30 (every released revision, and the owner's test
/// exe of 2026-10-17: "неможливо перемістити приціл") takes a scope off
/// with `46 02 <kit> <upgrade>`, no weapon: the server takes it off the
/// first item of the inventory holding it, as the client does on the
/// answer. On the SVD that is the bought scope coming off: its record goes,
/// and the weapon is built from its plain config, with the stock PSO-1.
#[test]
fn a_client_without_patch_30_takes_a_scope_off_its_first_holder() {
    let table = scope_table();
    let mut account = rich();
    let svd = give(&mut account, "svd", &[]);
    let ak = give(&mut account, "ak_74n", &[]);
    let scope = account.add_item(SCOPE, FULL_CONDITION);
    let mut client = client_of(&account);
    for host in [svd, ak] {
        attach_upgrade_in(&mut account, &table, host, scope, 3).unwrap();
    }
    client
        .receive_all(&wire::after_change(&account, NOW).unwrap())
        .unwrap();

    // The SVD comes first in the inventory: it loses the scope.
    assert_eq!(detach_upgrade(&mut account, 0, scope), Ok(Detached::Off));
    assert!(records(&account, svd).is_empty());
    assert_eq!(records(&account, ak), [scope_record(scope, AK_VARIANT)]);
    // The stock client's own answer handling agrees, before the inventory
    // that follows.
    takes(
        &mut client,
        &lobby::permitted_upgrade_detached(scope, 0),
        &account,
    );
    // Then the AK, and then there is nothing to take off.
    assert_eq!(detach_upgrade(&mut account, 0, scope), Ok(Detached::Off));
    assert!(records(&account, ak).is_empty());
    assert_eq!(
        detach_upgrade(&mut account, 0, scope),
        Err(ItemError::Upgrade("the cosmetic is not on that item"))
    );
}

#[test]
fn a_wrong_scope_record_is_taken_off() {
    let table = scope_table();
    let mut account = rich();
    let ak = give(&mut account, "ak_74n", &[]);
    let svd = give(&mut account, "svd", &[]);
    let rifle = give(&mut account, "ak_74m", &[]);
    let scope = account.add_item(SCOPE, FULL_CONDITION);
    let other = account.add_item(OTHER_SCOPE, FULL_CONDITION);
    let good = scope_record(scope, AK_VARIANT);

    // The load (`make_safe`): a variant made for another weapon, the scope
    // item's own id, a scope out of its slot, two in the slot, any on a
    // weapon with no pair.
    let check = |host: u32, held: Vec<Upgrade>, kept: &[Upgrade]| {
        let mut item = account.item(host).unwrap().clone();
        item.upgrades = held;
        make_safe_in(&mut item, &table);
        assert_eq!(item.upgrades, kept, "{host}");
    };
    check(ak, vec![good], &[good]);
    check(svd, vec![scope_record(scope, AK_VARIANT)], &[]);
    check(ak, vec![scope_record(scope, SCOPE)], &[]);
    check(ak, vec![Upgrade { param: 0, ..good }], &[]);
    check(
        ak,
        vec![good, scope_record(other, AK_OTHER_VARIANT)],
        &[good],
    );
    check(rifle, vec![scope_record(scope, AK_VARIANT)], &[]);
    // The players' data before the patch has no pair: every scope record
    // goes (none can be made there in the first place).
    let mut item = account.item(ak).unwrap().clone();
    item.upgrades = vec![good];
    make_safe(&mut item);
    assert!(item.upgrades.is_empty());

    // After a change of the inventory (`drop_gone_upgrades`): a record
    // that is not what its scope makes on the weapon (the first scope's
    // variant under the other scope's id) goes; a right one stays.
    account.item_mut(ak).unwrap().upgrades = vec![scope_record(other, AK_VARIANT)];
    account.item_mut(svd).unwrap().upgrades = vec![scope_record(scope, SVD_VARIANT)];
    assert_eq!(drop_gone_upgrades_in(&mut account.inventory, &table), [ak]);
    assert!(records(&account, ak).is_empty());
    assert_eq!(records(&account, svd), [scope_record(scope, SVD_VARIANT)]);
    // Cosmetics keep their own rule.
    let vest = give(&mut account, "edge_torso_2", &[]);
    let camo = account.add_item(dict("camo_city"), FULL_CONDITION);
    attach_upgrade_in(&mut account, &table, vest, camo, 3).unwrap();
    assert!(drop_gone_upgrades_in(&mut account.inventory, &table).is_empty());
    assert_eq!(records(&account, vest).len(), 1);
}

#[test]
fn a_modification_is_changed_for_one_of_three() {
    let mut account = rich();
    // A level 10 rifle: 2400 silver or 12 gold a change.
    let rifle = give(&mut account, "ak_74m", &[2000, 2010]);
    let mut client = client_of(&account);
    let mut rng = Rng::new(5);

    // The window of an item nothing was changed on.
    let window = reroll_window(&account, rifle).unwrap();
    assert_eq!(
        window,
        RerollWindow {
            item_id: rifle,
            regular_cost: 2400,
            premium_cost: 12,
            changed: None,
            offer: None
        }
    );
    let message = lobby::permitted_reroll_window(rifle, 2400, 12, None);
    let mut expected = vec![0x83, 0x46, 0x06];
    expected.extend(rifle.to_le_bytes());
    expected.extend([0x60, 0x09, 0, 0, 12, 0, 0, 0, 0]);
    assert_eq!(bytes(&message), expected);
    takes(&mut client, &message, &account);
    assert!(reroll_window(&account, 999).is_err());

    // A modification it has not, and a defect's id.
    assert_eq!(
        reroll(&mut account, rifle, 2020, false, NOW, &mut rng),
        Err(ItemError::Reroll("the item has no such modification"))
    );
    // Not without the silver.
    let mut poor = account.clone();
    poor.generic_money = 2399;
    assert_eq!(
        reroll(&mut poor, rifle, 2000, false, NOW, &mut rng),
        Err(ItemError::NotEnoughMoney)
    );
    assert_eq!(poor, {
        let mut same = account.clone();
        same.generic_money = 2399;
        same
    });
    poor.premium_money = 11;
    assert_eq!(
        reroll(&mut poor, rifle, 2000, true, NOW, &mut rng),
        Err(ItemError::NotEnoughGold)
    );

    // Paid in silver: three are offered, and silver does not wait (the owner's rule of 2026-10-22).
    let rolled = reroll(&mut account, rifle, 2000, false, NOW, &mut rng).unwrap();
    assert_eq!(rolled.paid, (2400, 0));
    assert_eq!(rolled.reroll_seconds, 0);
    assert_eq!(account.generic_money, 1_000_000 - 2400);
    let table = Modifications::shared();
    for &choice in &rolled.choices {
        assert!(table.fits(dict("ak_74m"), &[2010], choice));
        assert_ne!(choice, 2000);
    }
    let item = account.item(rifle).unwrap();
    assert_eq!(
        item.reroll,
        Reroll {
            modification: 2000,
            count: 1,
            offer: Some((2000, rolled.choices))
        }
    );
    // No wait before the next silver reroll (the owner's rule of 2026-10-22).
    assert_eq!(item.reroll_time, 0);
    // Nothing of the item changed yet.
    assert_eq!(own_modifications(item), [2000, 2010]);
    let message = lobby::permitted_reroll_result(rifle, rolled.choices, rolled.reroll_seconds);
    let mut expected = vec![0x83, 0x46, 0x07];
    expected.extend(rifle.to_le_bytes());
    for choice in rolled.choices {
        expected.extend(choice.to_le_bytes());
    }
    expected.extend([0, 0, 0, 0]);
    assert_eq!(bytes(&message), expected);
    takes(&mut client, &message, &account);

    // Asked again before the choice (the client came back): the same
    // three, and nothing more is taken.
    let again = reroll(&mut account, rifle, 2000, true, NOW + 60, &mut rng).unwrap();
    assert_eq!(again.choices, rolled.choices);
    assert_eq!(again.paid, (0, 0));
    assert_eq!(again.reroll_seconds, 0);
    assert_eq!(account.premium_money, 1_000);
    let window = reroll_window(&account, rifle).unwrap();
    assert_eq!(window.offer, Some(rolled.choices));
    assert_eq!(window.changed, Some((2000, 0)));

    // The choice: by its place among the three; the old one is named.
    assert_eq!(
        choose_modifier(&mut account, rifle, 2010, 1),
        Err(ItemError::Reroll("another modification is being changed"))
    );
    assert_eq!(
        choose_modifier(&mut account, rifle, 2000, 4),
        Err(ItemError::Reroll("no such choice"))
    );
    let chosen = choose_modifier(&mut account, rifle, 2000, 1).unwrap();
    assert_eq!(chosen.old_modification, 2000);
    assert_eq!(chosen.new_modification, rolled.choices[1]);
    // The next change costs the same (the owner's rule of 2026-10-25).
    assert_eq!(
        chosen.window,
        RerollWindow {
            item_id: rifle,
            regular_cost: 2400,
            premium_cost: 12,
            changed: Some((rolled.choices[1], 0)),
            offer: None
        }
    );
    let item = account.item(rifle).unwrap();
    // In the old one's place.
    assert_eq!(own_modifications(item), [rolled.choices[1], 2010]);
    assert_eq!(item.reroll.offer, None);
    let message =
        lobby::permitted_modifier_chosen(rifle, 2000, chosen.new_modification, [2400, 12, 0]);
    let mut expected = vec![0x83, 0x46, 0x08];
    expected.extend(rifle.to_le_bytes());
    expected.extend(2000_u32.to_le_bytes());
    expected.extend(chosen.new_modification.to_le_bytes());
    expected.extend([0x60, 0x09, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(bytes(&message), expected);
    takes(&mut client, &message, &account);
    assert_eq!(
        choose_modifier(&mut account, rifle, 2000, 0),
        Err(ItemError::Reroll("nothing was offered"))
    );

    // The new one may be changed again, in gold or silver at once (no
    // wait: the owner's rule of 2026-10-22); so may the others
    // (`every_modification_of_an_item_may_be_changed`).
    let new = chosen.new_modification;
    let rolled = reroll(&mut account, rifle, new, true, NOW + 100, &mut rng).unwrap();
    assert_eq!(rolled.paid, (0, 12));
    assert_eq!(account.premium_money, 1_000 - 12);
    assert_eq!(rolled.reroll_seconds, 0);
    let chosen = choose_modifier(&mut account, rifle, new, 0).unwrap();
    assert_eq!(chosen.window.regular_cost, 2400);
    // Silver pays again at once.
    let later = NOW + 200;
    assert_eq!(reroll_window(&account, rifle).unwrap().changed, {
        Some((chosen.new_modification, 0))
    });
    let rolled = reroll(
        &mut account,
        rifle,
        chosen.new_modification,
        false,
        later,
        &mut rng,
    )
    .unwrap();
    assert_eq!(rolled.paid, (2400, 0));

    // A damaged item, a stack and an item in repair can not be changed.
    let mut account = rich();
    let broken = give(&mut account, "ak_74m", &[2000]);
    account
        .item_mut(broken)
        .unwrap()
        .modifications
        .push((3000, true));
    assert_eq!(
        reroll(&mut account, broken, 2000, false, NOW, &mut rng),
        Err(ItemError::RerollBroken)
    );
    assert_eq!(
        ItemError::RerollBroken.text_key(),
        "st_reroll_message_broken_item"
    );
    let ammo = account.kits[0].slots[8].unwrap().item_id;
    assert!(reroll(&mut account, ammo, 2000, false, NOW, &mut rng).is_err());
    let busy = give(&mut account, "ak_74m", &[2000]);
    account.item_mut(busy).unwrap().repair_time = (NOW + 10) as u32;
    assert_eq!(
        reroll(&mut account, busy, 2000, true, NOW, &mut rng),
        Err(ItemError::Reroll("the item is in repair"))
    );
    assert!(reroll(&mut account, busy, 2000, true, NOW + 10, &mut rng).is_ok());
}

/// `docs/notes/reroll-keep-old.md`.
#[test]
fn a_change_may_be_given_up_for_the_old_one() {
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[2000, 2010]);
    let mut client = client_of(&account);
    let mut rng = Rng::new(7);

    // Paid when the three are offered; the item keeps the old one until
    // the choice.
    let rolled = reroll(&mut account, rifle, 2000, false, NOW, &mut rng).unwrap();
    assert_eq!(rolled.paid, (2400, 0));
    let paid = account.generic_money;
    assert_eq!(paid, 1_000_000 - 2400);
    let item = account.item(rifle).unwrap();
    assert_eq!(own_modifications(item), [2000, 2010]);
    takes(
        &mut client,
        &lobby::permitted_reroll_result(rifle, rolled.choices, rolled.reroll_seconds),
        &account,
    );

    // A pending offer is saved with the item and kept by the load (a
    // restart, the client gone): the same three come back, and no second
    // change is made or paid for until it is answered, in silver or gold.
    let mut loaded = account.item(rifle).unwrap().clone();
    make_safe(&mut loaded);
    assert_eq!(loaded.reroll.offer, Some((2000, rolled.choices)));
    // (Whichever modification is named: the window shows the offer.)
    for (modification, gold) in [(2000, false), (2000, true), (2010, true)] {
        let again = reroll(&mut account, rifle, modification, gold, NOW + 30, &mut rng).unwrap();
        assert_eq!(again.choices, rolled.choices);
        assert_eq!(again.paid, (0, 0));
    }
    assert_eq!(
        (account.generic_money, account.premium_money),
        (paid, 1_000)
    );
    assert_eq!(account.item(rifle).unwrap().reroll.count, 1);

    // Keep the old one: named as the one being changed.
    assert_eq!(
        choose_modifier(&mut account, rifle, 2010, KEEP_OLD_MODIFICATION),
        Err(ItemError::Reroll("another modification is being changed"))
    );
    let before = account.clone();
    let kept = choose_modifier(&mut account, rifle, 2000, KEEP_OLD_MODIFICATION).unwrap();
    assert_eq!(kept.old_modification, 2000);
    assert_eq!(kept.new_modification, 2000);
    // The same price for the next change; no wait.
    assert_eq!(
        kept.window,
        RerollWindow {
            item_id: rifle,
            regular_cost: 2400,
            premium_cost: 12,
            changed: Some((2000, 0)),
            offer: None
        }
    );
    let item = account.item(rifle).unwrap();
    assert_eq!(own_modifications(item), [2000, 2010]);
    assert_eq!(
        item.reroll,
        Reroll {
            modification: 2000,
            count: 1,
            offer: None
        }
    );
    // Nothing is given back, and nothing else of the account changed.
    assert_eq!(account.generic_money, paid);
    let mut expected = before.clone();
    expected.item_mut(rifle).unwrap().reroll.offer = None;
    assert_eq!(account, expected);
    // The client's copies stay as they were.
    let message = lobby::permitted_modifier_chosen(rifle, 2000, 2000, [2400, 12, 0]);
    takes(&mut client, &message, &account);

    // Answered once: neither a choice nor keeping it again.
    for choice in [0, KEEP_OLD_MODIFICATION] {
        assert_eq!(
            choose_modifier(&mut account, rifle, 2000, choice),
            Err(ItemError::Reroll("nothing was offered"))
        );
    }
    assert_eq!(account, expected);

    // The next change is paid again (gold now: silver waits), and its
    // choice takes the new one in the old one's place.
    let rolled = reroll(&mut account, rifle, 2000, true, NOW + 60, &mut rng).unwrap();
    assert_eq!(rolled.paid, (0, 12));
    assert_eq!(account.premium_money, 1_000 - 12);
    let chosen = choose_modifier(&mut account, rifle, 2000, 2).unwrap();
    assert_eq!(chosen.new_modification, rolled.choices[2]);
    assert_eq!(
        own_modifications(account.item(rifle).unwrap()),
        [rolled.choices[2], 2010]
    );
    assert_eq!(
        (account.generic_money, account.premium_money),
        (paid, 1_000 - 12)
    );
}

/// The owner's rule of 2026-10-22: any modification of an item may be
/// changed, each any number of times; the one changed before locks
/// nothing (`docs/notes/reroll-keep-old.md`). Every change of the item, of
/// whichever modification, kept or not, costs the same (the owner's rule of
/// 2026-10-25, `docs/notes/flat-reroll.md`).
#[test]
fn every_modification_of_an_item_may_be_changed() {
    let mut account = rich();
    // A level 10 rifle: 2400 silver or 12 gold a change.
    let rifle = give(&mut account, "ak_74m", &[2000, 2010, 2020]);
    let mut client = client_of(&account);
    let mut rng = Rng::new(11);
    let table = Modifications::shared();
    let mut change = |account: &mut Account, client: &mut Client, old: u32, gold, choice| {
        let rolled = reroll(account, rifle, old, gold, NOW, &mut rng).unwrap();
        let others: Vec<u32> = own_modifications(account.item(rifle).unwrap())
            .into_iter()
            .filter(|&id| id != old)
            .collect();
        for &offered in &rolled.choices {
            assert!(table.fits(dict("ak_74m"), &others, offered));
            assert_ne!(offered, old);
        }
        takes(
            client,
            &lobby::permitted_reroll_result(rifle, rolled.choices, rolled.reroll_seconds),
            account,
        );
        let chosen = choose_modifier(account, rifle, old, choice).unwrap();
        let window = chosen.window;
        takes(
            client,
            &lobby::permitted_modifier_chosen(
                rifle,
                old,
                chosen.new_modification,
                [window.regular_cost, window.premium_cost, 0],
            ),
            account,
        );
        (rolled.paid, chosen.new_modification, window)
    };

    // The first: 2000, in silver.
    let (paid, first, window) = change(&mut account, &mut client, 2000, false, 0);
    assert_eq!(paid, (2400, 0));
    assert_eq!(window.changed, Some((first, 0)));
    assert_eq!((window.regular_cost, window.premium_cost), (2400, 12));

    // The second, another one: not locked out, and the same price.
    let (paid, second, window) = change(&mut account, &mut client, 2010, false, 1);
    assert_eq!(paid, (2400, 0));
    assert_eq!(window.changed, Some((second, 0)));
    assert_eq!((window.regular_cost, window.premium_cost), (2400, 12));
    assert_eq!(
        own_modifications(account.item(rifle).unwrap()),
        [first, second, 2020]
    );

    // The third, in gold, kept: paid all the same.
    let (paid, kept, window) =
        change(&mut account, &mut client, 2020, true, KEEP_OLD_MODIFICATION);
    assert_eq!(paid, (0, 12));
    assert_eq!(kept, 2020);
    assert_eq!(window.changed, Some((2020, 0)));
    assert_eq!((window.regular_cost, window.premium_cost), (2400, 12));
    assert_eq!(
        own_modifications(account.item(rifle).unwrap()),
        [first, second, 2020]
    );

    // The first changed again. While it is offered, naming another one
    // shows the same three and takes nothing: one offer at a time.
    let rolled = reroll(&mut account, rifle, first, false, NOW, &mut rng).unwrap();
    assert_eq!(rolled.paid, (2400, 0));
    let again = reroll(&mut account, rifle, second, true, NOW, &mut rng).unwrap();
    assert_eq!((again.choices, again.paid), (rolled.choices, (0, 0)));
    assert_eq!((rolled.replaced, rolled.again), (first, false));
    assert_eq!((again.replaced, again.again), (first, true));
    assert_eq!(
        choose_modifier(&mut account, rifle, second, 0),
        Err(ItemError::Reroll("another modification is being changed"))
    );
    let chosen = choose_modifier(&mut account, rifle, first, 2).unwrap();
    assert_eq!(chosen.new_modification, rolled.choices[2]);
    assert_eq!(
        own_modifications(account.item(rifle).unwrap()),
        [rolled.choices[2], second, 2020]
    );
    assert_eq!(
        (chosen.window.regular_cost, chosen.window.premium_cost),
        (2400, 12)
    );

    // Four changes, each paid once.
    let item = account.item(rifle).unwrap();
    assert_eq!(
        item.reroll,
        Reroll {
            modification: rolled.choices[2],
            count: 4,
            offer: None
        }
    );
    assert_eq!(
        (account.generic_money, account.premium_money),
        (1_000_000 - 3 * 2400, 1_000 - 12)
    );
}

/// The owner's rule of 2026-10-25 (`docs/notes/flat-reroll.md`): the
/// Nth change costs what the first did, and the window shows what is
/// taken. An item with many changes from before pays the same.
#[test]
fn every_change_costs_the_first_ones_price() {
    let mut rng = Rng::new(3);
    for count in [0, 1, 4, 9, 1_000, u32::MAX] {
        for gold in [false, true] {
            let mut account = rich();
            let rifle = give(&mut account, "ak_74m", &[2000, 2010]);
            account.item_mut(rifle).unwrap().reroll.count = count;
            let window = reroll_window(&account, rifle).unwrap();
            assert_eq!((window.regular_cost, window.premium_cost), (2400, 12));
            let shown = if gold {
                window.premium_cost
            } else {
                window.regular_cost
            };
            let before = (account.generic_money, account.premium_money);
            let rolled = reroll(&mut account, rifle, 2000, gold, NOW, &mut rng).unwrap();
            let taken = (
                before.0 - account.generic_money,
                before.1 - account.premium_money,
            );
            let expected = if gold { (0, shown) } else { (shown, 0) };
            assert_eq!((rolled.paid, taken), (expected, expected));
            // The count does not wrap round.
            assert_eq!(
                account.item(rifle).unwrap().reroll.count,
                count.saturating_add(1)
            );
            // Not a coin short.
            let mut poor = rich();
            let rifle = give(&mut poor, "ak_74m", &[2000, 2010]);
            poor.item_mut(rifle).unwrap().reroll.count = count;
            poor.generic_money = 2399;
            poor.premium_money = 11;
            assert!(reroll(&mut poor, rifle, 2000, gold, NOW, &mut rng).is_err());
            assert_eq!((poor.generic_money, poor.premium_money), (2399, 11));
        }
    }
}

#[test]
fn an_item_is_upgraded_with_a_donor() {
    let table = Modifications::shared();
    let (mut added, mut nothing) = (0, 0);
    // An item with one modification and a donor with none: in silver a
    // quarter gain one; the rest (the data's quarter of "-1 mod." too) keep
    // what they had (the owner's rule of 2026-10-23).
    for seed in 0..400 {
        let mut account = rich();
        let rifle = give(&mut account, "ak_74m", &[2000]);
        let donor = give(&mut account, "ak_74m", &[]);
        let mut client = client_of(&account);
        let mut rng = Rng::new(seed * 7 + 1);
        let crafted = craft(&mut account, rifle, donor, false, false, NOW, &mut rng).unwrap();
        // The price of a level 10 item, and the donor is gone.
        assert_eq!(account.generic_money, 1_000_000 - 10_000);
        assert!(account.item(donor).is_none());
        let held = own_modifications(account.item(rifle).unwrap());
        match crafted {
            Crafted::Added {
                consumed_item_id,
                modification,
            } => {
                added += 1;
                assert_eq!(consumed_item_id, donor);
                assert_eq!(held, [2000, modification]);
                assert!(table.fits(dict("ak_74m"), &[2000], modification));
            }
            Crafted::Nothing { consumed_item_id } => {
                nothing += 1;
                assert_eq!(consumed_item_id, donor);
                assert_eq!(held, [2000]);
            }
            other => panic!("{other:?}"),
        }
        let message = lobby::permitted_crafted(rifle, crafted).unwrap();
        takes(&mut client, &message, &account);
    }
    assert!((60..140).contains(&added), "{added} gained");
    assert_eq!(added + nothing, 400);

    // In gold with as many modifications as the donor: always one more.
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[2000]);
    let donor = give(&mut account, "ak_74m", &[2011]);
    let mut rng = Rng::new(9);
    let crafted = craft(&mut account, rifle, donor, true, false, NOW, &mut rng).unwrap();
    let Crafted::Added { modification, .. } = crafted else {
        panic!("{crafted:?}");
    };
    assert_eq!(account.premium_money, 1_000 - 50);
    assert_eq!(account.generic_money, 1_000_000);
    // Byte for byte: mode 1, the item, the donor, the modification.
    let mut expected = vec![0x83, 0x46, 0x09, 1];
    expected.extend(rifle.to_le_bytes());
    expected.extend(donor.to_le_bytes());
    expected.extend(modification.to_le_bytes());
    assert_eq!(
        bytes(&lobby::permitted_crafted(rifle, crafted).unwrap()),
        expected
    );
}

/// The owner's rule of 2026-10-23 ("remove chance for bonus removal during
/// combination"): an upgrade never takes a modification away. Whatever the
/// item holds (one or two, reliability, a golden weapon's silver bonus,
/// premium armour's bonuses), the donor and the price go, exactly, and the
/// item keeps every modification it had: one more on a success, as before
/// (the data's success chances), and none fewer.
#[test]
fn an_upgrade_never_takes_a_modification_away() {
    let table = Modifications::shared();
    let (golden_rifle, premium_legs) = (dict("ak_74n_premium"), dict("bm_legs_4_premium"));
    // (item, its modifications, the donor's): the item's own ones are
    // given after its bonuses.
    let cases: [(u16, &[u32], &[u32]); 9] = [
        (dict("ak_74m"), &[2000], &[]),
        (dict("ak_74m"), &[2000], &[2011]),
        (dict("ak_74m"), &[2000, 2010], &[]),
        (dict("ak_74m"), &[2000, 2010], &[2021]),
        (dict("ak_74m"), &[2000, 2010], &[2021, 2031]),
        // Reliability, which no roll gives any more: kept as any other.
        (dict("ak_74m"), &[2093], &[]),
        (dict("ak_74m"), &[2093, 2000], &[]),
        (golden_rifle, &[], &[]),
        (premium_legs, &[], &[]),
    ];
    for (dict_id, own, donor_held) in cases {
        for gold in [false, true] {
            let (mut added, mut nothing) = (0_u32, 0_u32);
            for seed in 0..300 {
                let mut account = rich();
                let item_id = account.add_item(dict_id, FULL_CONDITION);
                account
                    .item_mut(item_id)
                    .unwrap()
                    .modifications
                    .extend(own.iter().map(|&id| (id, false)));
                let donor_dict = if dict_id == golden_rifle {
                    dict("ak_74n")
                } else if dict_id == premium_legs {
                    dict("bm_legs_4")
                } else {
                    dict_id
                };
                let donor = account.add_item(donor_dict, FULL_CONDITION);
                account.item_mut(donor).unwrap().modifications =
                    donor_held.iter().map(|&id| (id, false)).collect();
                let before = account.clone();
                let item_before = before.item(item_id).unwrap().clone();
                let held_before = own_modifications(&item_before);
                let price = table.craft.price(item_level(dict_id), gold);
                let mut client = client_of(&account);
                let mut rng = Rng::new(seed * 11 + 3);
                let crafted =
                    craft(&mut account, item_id, donor, gold, false, NOW, &mut rng).unwrap();

                // The price, in the money chosen, and nothing else.
                let (silver, golds) = if gold { (0, price) } else { (price, 0) };
                assert_eq!(account.generic_money, before.generic_money - silver);
                assert_eq!(account.premium_money, before.premium_money - golds);
                // The donor is gone; nothing else is made or lost.
                assert!(account.item(donor).is_none());
                assert_eq!(account.inventory.len(), before.inventory.len() - 1);
                for other in &before.inventory {
                    if other.item_id != item_id && other.item_id != donor {
                        assert_eq!(account.item(other.item_id), Some(other));
                    }
                }
                // The item keeps every modification it had, in its place.
                let item = account.item(item_id).unwrap();
                let held = own_modifications(item);
                assert_eq!(held[..held_before.len()], held_before[..]);
                let mut expected = item_before.clone();
                match crafted {
                    Crafted::Added {
                        consumed_item_id,
                        modification,
                    } => {
                        added += 1;
                        assert_eq!(consumed_item_id, donor);
                        assert!(!held_before.contains(&modification));
                        expected.modifications.push((modification, false));
                    }
                    Crafted::Nothing { consumed_item_id } => {
                        nothing += 1;
                        assert_eq!(consumed_item_id, donor);
                    }
                    other => panic!("{other:?}"),
                }
                assert_eq!(item, &expected);
                let message = lobby::permitted_crafted(item_id, crafted).unwrap();
                takes(&mut client, &message, &account);
            }
            // The success chance is the data's, as before; the rest leave
            // the item as it was.
            let difference = held_before_len(dict_id, own) - donor_held.len();
            let (success, failure) = table.craft.chances(difference, gold).unwrap();
            let expected = 300.0 * success / 100.0;
            let slack = 4.0 * (300.0 * success / 100.0 * (1.0 - success / 100.0)).sqrt() + 1.0;
            assert!(
                (f64::from(added) - f64::from(expected)).abs() <= f64::from(slack),
                "{dict_id} {own:?} {donor_held:?} gold {gold}: {added} of 300, {success} %"
            );
            assert_eq!(added + nothing, 300);
            if failure > 0.0 {
                assert!(nothing > 0);
            }
        }
    }
}

/// How many modifications an item of `dict_id` holds with `own` given
/// after the bonuses it is made with.
fn held_before_len(dict_id: u16, own: &[u32]) -> usize {
    let mut account = rich();
    let item_id = account.add_item(dict_id, FULL_CONDITION);
    own_modifications(account.item(item_id).unwrap()).len() + own.len()
}

#[test]
fn a_craft_the_lobby_would_not_offer_is_refused() {
    let mut rng = Rng::new(3);
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[2000]);
    let donor = give(&mut account, "ak_74m", &[]);
    let before = account.clone();
    let mut refused = |account: &mut Account, item, donor, gold, queued| {
        let result = craft(account, item, donor, gold, queued, NOW, &mut rng);
        result.expect_err("refused")
    };
    assert_eq!(
        refused(&mut account, rifle, rifle, false, false),
        "an item is its own donor"
    );
    assert_eq!(
        refused(&mut account, rifle, 999, false, false),
        "no such donor"
    );
    assert_eq!(
        refused(&mut account, 999, donor, false, false),
        "no such item"
    );
    // Another item as the donor, and things that take no modifications.
    let pistol = account.kits[0].slots[7].unwrap().item_id;
    let ammo = account.kits[0].slots[8].unwrap().item_id;
    assert_eq!(
        refused(&mut account, rifle, pistol, false, false),
        "the donor is another item"
    );
    assert_eq!(
        refused(&mut account, ammo, ammo + 100, false, false),
        "no such donor"
    );
    assert_eq!(
        refused(&mut account, ammo, rifle, false, false),
        "not an item with modifications"
    );
    // A donor with more modifications.
    let better = give(&mut account, "ak_74m", &[2000, 2010]);
    assert_eq!(
        refused(&mut account, rifle, better, false, false),
        "the donor has more modifications"
    );
    // Three is the most.
    let full = give(&mut account, "ak_74m", &[2000, 2010, 2020]);
    assert_eq!(
        refused(&mut account, full, donor, false, false),
        "the most modifications already"
    );
    assert_eq!(
        refused(&mut account, full, full - 1, false, false),
        "the most modifications already"
    );
    // Damaged, or in repair.
    let broken = give(&mut account, "ak_74m", &[]);
    account
        .item_mut(broken)
        .unwrap()
        .modifications
        .push((3000, true));
    assert_eq!(
        refused(&mut account, rifle, broken, false, false),
        "a damaged item"
    );
    let busy = give(&mut account, "ak_74m", &[]);
    account.item_mut(busy).unwrap().repair_time = (NOW + 1) as u32;
    assert_eq!(
        refused(&mut account, rifle, busy, false, false),
        "an item in repair"
    );
    // An equipped item while the account waits for a match.
    let slot = (0..23)
        .find(|&slot| Dictionary::shared().fits(dict("ak_74m"), slot))
        .unwrap();
    account.kits[1].slots[usize::from(slot)] = Some(KitSlot {
        item_id: donor,
        count: 1,
    });
    assert_eq!(
        refused(&mut account, rifle, donor, false, true),
        "an equipped item while waiting for a match"
    );
    // No money.
    account.generic_money = 9_999;
    assert_eq!(
        refused(&mut account, rifle, donor, false, false),
        "not enough money"
    );
    account.premium_money = 49;
    assert_eq!(
        refused(&mut account, rifle, donor, true, false),
        "not enough money"
    );
    // Nothing was taken by any of it.
    account.generic_money = before.generic_money;
    account.premium_money = before.premium_money;
    assert_eq!(account.item(rifle), before.item(rifle));
    assert_eq!(account.item(donor), before.item(donor));

    // Out of the queue the equipped donor is used, and its slot empties.
    let crafted = craft(
        &mut account,
        rifle,
        donor,
        true,
        false,
        NOW,
        &mut Rng::new(1),
    );
    assert!(crafted.is_ok());
    assert_eq!(account.kits[1].slots[usize::from(slot)], None);

    // The client is told with mode 0xFE: the item and nothing else.
    let message = lobby::permitted_crafted(rifle, Crafted::Failed).unwrap();
    let mut expected = vec![0x83, 0x46, 0x09, 0xFE];
    expected.extend(rifle.to_le_bytes());
    assert_eq!(bytes(&message), expected);
    let mut client = client_of(&account);
    takes(&mut client, &message, &account);
}

#[test]
fn a_repair_mends_the_wear_and_the_defects() {
    let dictionary = Dictionary::shared();
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[2000]);
    let kits = account.add_item(dict("repair_voucher"), 3);
    account.item_mut(rifle).unwrap().condition_or_stack = 400_000;
    let wear = prices::repair_cost(dictionary, dict("ak_74m"), 40);
    assert!(wear > 0);

    // Wear alone: silver, at once; gold and kits are for defects.
    assert_eq!(repair_quote(account.item(rifle).unwrap()), (wear, 0, 0, 0));
    assert_eq!(
        repair(&mut account.clone(), rifle, RepairPayment::Gold, NOW),
        Err(ItemError::Repair("no defects to pay gold for"))
    );
    assert_eq!(
        repair(
            &mut account.clone(),
            rifle,
            RepairPayment::Kit { kit_item_id: kits },
            NOW
        ),
        Err(ItemError::Repair("no defects to use a kit on"))
    );
    let mut whole = account.clone();
    let mut client = client_of(&whole);
    assert_eq!(
        repair(&mut whole, rifle, RepairPayment::Silver, NOW),
        Ok(Repaired::AtOnce)
    );
    assert_eq!(whole.generic_money, 1_000_000 - wear);
    assert_eq!(
        whole.item(rifle).unwrap().condition_or_stack,
        FULL_CONDITION
    );
    let message = lobby::permitted_repaired(rifle, Repaired::AtOnce).unwrap();
    let mut expected = vec![0x83, 0x47, 0x03];
    expected.extend(rifle.to_le_bytes());
    expected.extend([0; 8]);
    assert_eq!(bytes(&message), expected);
    takes(&mut client, &message, &whole);
    assert_eq!(
        repair(&mut whole, rifle, RepairPayment::Silver, NOW),
        Err(ItemError::Repair("nothing to repair"))
    );

    // Two defects: 100 silver and six minutes each on top, or 50 gold
    // each, or a kit each.
    let item = account.item_mut(rifle).unwrap();
    item.modifications.extend([(3000, true), (3010, true)]);
    assert_eq!(
        repair_quote(account.item(rifle).unwrap()),
        (wear + 200, 100, 720, 2)
    );

    // Silver: the item is whole and has no defect at once, and is locked
    // for twelve minutes.
    let mut slow = account.clone();
    let mut client = client_of(&slow);
    let repaired = repair(&mut slow, rifle, RepairPayment::Silver, NOW).unwrap();
    assert_eq!(repaired, Repaired::Timed { repair_time: 720 });
    assert_eq!(slow.generic_money, 1_000_000 - wear - 200);
    let item = slow.item(rifle).unwrap();
    assert_eq!(item.condition_or_stack, FULL_CONDITION);
    assert_eq!(item.modifications, [(2000, false)]);
    assert_eq!(u64::from(item.repair_time), NOW + 720);
    let message = lobby::permitted_repaired(rifle, repaired).unwrap();
    let mut expected = vec![0x83, 0x47, 0x03];
    expected.extend(rifle.to_le_bytes());
    expected.extend([0xD0, 0x02, 0, 0]);
    assert_eq!(bytes(&message), expected);
    // The answer leaves the client's defects: the inventory takes them.
    client.receive(&message).unwrap();
    let at = slow
        .inventory
        .iter()
        .position(|item| item.item_id == rifle)
        .unwrap();
    assert_eq!(client.inventory()[at].modifications.len(), 3);
    assert_eq!(client.inventory()[at].repair_time, 720);
    let push = wire::after_change(&slow, NOW).unwrap();
    takes(&mut client, &push[0], &slow);
    // Not twice.
    assert_eq!(
        repair(&mut slow, rifle, RepairPayment::Silver, NOW + 1),
        Err(ItemError::Repair("already in repair"))
    );
    // The seconds left go out, and the end is told once.
    assert_eq!(
        wire::item_descr_at(slow.item(rifle).unwrap(), NOW + 700).repair_time,
        20
    );
    assert!(finish_repairs(&mut slow, NOW + 719).is_empty());
    assert_eq!(finish_repairs(&mut slow, NOW + 720), [rifle]);
    assert_eq!(slow.item(rifle).unwrap().repair_time, 0);
    assert!(finish_repairs(&mut slow, NOW + 721).is_empty());
    assert_eq!(repaired_notice(rifle), (6, format!("#ir[{rifle}]")));

    // Gold: at once.
    let mut quick = account.clone();
    let mut client = client_of(&quick);
    assert_eq!(
        repair(&mut quick, rifle, RepairPayment::Gold, NOW),
        Ok(Repaired::AtOnce)
    );
    assert_eq!((quick.generic_money, quick.premium_money), (1_000_000, 900));
    let item = quick.item(rifle).unwrap();
    assert_eq!(
        (item.condition_or_stack, item.repair_time),
        (FULL_CONDITION, 0)
    );
    assert_eq!(item.modifications, [(2000, false)]);
    client
        .receive(&lobby::permitted_repaired(rifle, Repaired::AtOnce).unwrap())
        .unwrap();
    let push = wire::after_change(&quick, NOW).unwrap();
    takes(&mut client, &push[0], &quick);
    quick.premium_money = 0;
    quick
        .item_mut(rifle)
        .unwrap()
        .modifications
        .push((3000, true));
    assert_eq!(
        repair(&mut quick, rifle, RepairPayment::Gold, NOW),
        Err(ItemError::NotEnoughGold)
    );

    // Kits: one a defect.
    let mut kitted = account.clone();
    let mut client = client_of(&kitted);
    let repaired = repair(
        &mut kitted,
        rifle,
        RepairPayment::Kit { kit_item_id: kits },
        NOW,
    )
    .unwrap();
    assert_eq!(
        repaired,
        Repaired::WithKit {
            kit_item_id: kits,
            used: 2
        }
    );
    assert_eq!(kitted.item(kits).unwrap().condition_or_stack, 1);
    assert_eq!(kitted.generic_money, 1_000_000);
    assert!(defects(kitted.item(rifle).unwrap()).is_empty());
    let message = lobby::permitted_repaired(rifle, repaired).unwrap();
    let mut expected = vec![0x83, 0x47, 0x03];
    expected.extend(rifle.to_le_bytes());
    expected.extend([0, 0, 0, 0]);
    expected.extend(kits.to_le_bytes());
    expected.push(2);
    assert_eq!(bytes(&message), expected);
    client.receive(&message).unwrap();
    let push = wire::after_change(&kitted, NOW).unwrap();
    takes(&mut client, &push[0], &kitted);
    // One kit left for two defects, an item that is no kit, and no item.
    kitted
        .item_mut(rifle)
        .unwrap()
        .modifications
        .extend([(3000, true), (3010, true)]);
    for kit_item_id in [kits, rifle, 0, 999] {
        assert_eq!(
            repair(&mut kitted, rifle, RepairPayment::Kit { kit_item_id }, NOW),
            Err(ItemError::NoRepairKit)
        );
    }
    // The last kit used up is gone from the inventory.
    kitted.item_mut(rifle).unwrap().modifications.pop();
    assert!(
        repair(
            &mut kitted,
            rifle,
            RepairPayment::Kit { kit_item_id: kits },
            NOW
        )
        .is_ok()
    );
    assert!(kitted.item(kits).is_none());

    // No silver, a stack, nothing, another kind.
    let mut poor = account.clone();
    poor.generic_money = wear + 199;
    assert_eq!(
        repair(&mut poor, rifle, RepairPayment::Silver, NOW),
        Err(ItemError::NotEnoughMoneyForRepair)
    );
    assert_eq!(poor.item(rifle), account.item(rifle));
    assert!(repair(&mut account, kits, RepairPayment::Silver, NOW).is_err());
    assert!(repair(&mut account, 999, RepairPayment::Silver, NOW).is_err());
    assert!(repair(&mut account, rifle, RepairPayment::Other(7), NOW).is_err());
    for (error, key) in [
        (ItemError::Repair(""), "st_cannot_repair_item"),
        (ItemError::NoRepairKit, "st_you_have_no_voucher_for_repair"),
        (
            ItemError::NotEnoughMoneyForRepair,
            "st_not_enought_money_for_repair",
        ),
        (ItemError::NotEnoughGold, "st_cant_buy_premium_message"),
        (ItemError::NotEnoughMoney, "st_not_enougth_money"),
        (
            ItemError::Reroll(""),
            "st_cannot_select_rerolled_modification",
        ),
    ] {
        assert_eq!(error.text_key(), key);
    }
}

#[test]
fn a_worn_item_may_get_a_defect() {
    let mut rng = Rng::new(2);
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[]);
    // Over 90 % nothing breaks.
    account.item_mut(rifle).unwrap().condition_or_stack = 950_000;
    for _ in 0..200 {
        let worn = [(rifle, 960_000, 950_000)];
        assert!(roll_defects(&mut account, &worn, &mut rng).is_empty());
    }
    // An item that did not wear is not rolled for.
    assert!(roll_defects(&mut account, &[(rifle, 100, 100)], &mut rng).is_empty());
    // Worn out, match after match: three defects at most, each of another
    // kind and one the rifle may have, and the client takes them.
    let mut broke = Vec::new();
    for _ in 0..200 {
        broke.extend(roll_defects(&mut account, &[(rifle, 1000, 0)], &mut rng));
    }
    assert_eq!(broke.len(), 3);
    let ids = defects(account.item(rifle).unwrap());
    assert_eq!(ids.len(), 3);
    for &(item_id, defect) in &broke {
        assert_eq!(item_id, rifle);
        assert!([3000, 3010, 3020, 3030, 3040, 3060].contains(&defect));
        assert_eq!(ids.iter().filter(|&&id| id == defect).count(), 1);
    }
    let client = client_of(&account);
    assert_eq!(client.inventory().last().unwrap().modifications.len(), 3);
    // A damaged item's rank is still its modifications.
    assert_eq!(rank(account.item(rifle).unwrap()), 0);
    // Half worn: the first slot about one match in three (10 % at 90 %
    // to 50 % at nothing).
    let mut count = 0;
    for _ in 0..2000 {
        let mut account = rich();
        let rifle = give(&mut account, "ak_74m", &[]);
        count += roll_defects(&mut account, &[(rifle, 460_000, 450_000)], &mut rng).len();
    }
    assert!((500..700).contains(&count), "{count}");
}

#[test]
fn wear_drops_and_pay_follow_the_modifications() {
    let mut rng = Rng::new(4);
    let mut account = rich();
    // Reliability: a fifth less wear with 2093.
    let plain = give(&mut account, "ak_74m", &[]);
    let reliable = give(&mut account, "ak_74m", &[2093]);
    assert_eq!(wear_factor(&account, plain), 1.0);
    assert!((wear_factor(&account, reliable) - 0.8).abs() < 1e-6);
    assert_eq!(wear_factor(&account, 999), 1.0);

    // A drop of rarity n has n modifications, and that is its rank.
    for rarity in 0..=3 {
        let item_id = account.add_item(dict("ak_74m"), FULL_CONDITION);
        let shown = roll_modifications(&mut account, item_id, rarity, &mut rng);
        assert_eq!(shown, rarity);
        let item = account.item(item_id).unwrap();
        assert_eq!(rank(item), usize::from(rarity));
        assert!(defects(item).is_empty());
    }
    // A camouflage takes none.
    let camo = account.add_item(dict("camo_city"), FULL_CONDITION);
    assert_eq!(roll_modifications(&mut account, camo, 3, &mut rng), 0);
    assert!(account.item(camo).unwrap().modifications.is_empty());
    // And the client takes all of it.
    client_of(&account);

    // A kit with a golden rifle and premium trousers (their bonuses their
    // own modifications, as new ones have them): 10 % silver, 5 %
    // experience and 5 % reputation with the Black Market.
    let premium = account.add_item(dict("ak_74n_premium"), FULL_CONDITION);
    let trousers = account.add_item(dict("bm_legs_4_premium"), FULL_CONDITION);
    let dictionary = Dictionary::shared();
    for item_id in [premium, trousers] {
        let dict_id = account.item(item_id).unwrap().dict_id;
        let slot = (0..23)
            .find(|&slot| dictionary.fits(dict_id, slot))
            .unwrap();
        account.kits[1].slots[usize::from(slot)] = Some(KitSlot { item_id, count: 1 });
    }
    assert_eq!(kit_bonuses(&account, 0), Bonuses::default());
    let bonuses = kit_bonuses(&account, 1);
    assert!((bonuses.money - 0.1).abs() < 1e-6);
    assert!((bonuses.experience - 0.05).abs() < 1e-6);
    assert!((bonuses.reputation[1] - 0.05).abs() < 1e-6);
    assert_eq!(bonuses.reputation[0], 0.0);
    assert_eq!(kit_bonuses(&account, 9), Bonuses::default());
}

// ---------------------------------------------------- no reliability rolls

/// Reliability ("Надежность"): the modifications that only change wear.
fn is_reliability(id: u32) -> bool {
    Modifications::shared()
        .modification(id)
        .is_some_and(|row| row.only_changes_wear())
}

/// What the account holds, money and items, for the sums.
fn holdings(account: &Account) -> (u32, u32, usize) {
    (
        account.generic_money,
        account.premium_money,
        account.inventory.len(),
    )
}

/// The owner's rule of 2026-10-23 (docs/notes/no-reliability-roll.md): an
/// upgrade never gains reliability, and costs its price and its donor, no
/// more; an upgrade with nothing left to roll is refused and takes nothing.
#[test]
fn an_upgrade_never_gains_reliability() {
    let table = Modifications::shared();
    // In gold with as many as the donor, always one more: never reliability.
    for seed in 0..300 {
        let mut account = rich();
        let rifle = give(&mut account, "ak_74m", &[2000]);
        let donor = give(&mut account, "ak_74m", &[2011]);
        let before = holdings(&account);
        let mut rng = Rng::new(seed);
        let crafted = craft(&mut account, rifle, donor, true, false, NOW, &mut rng).unwrap();
        let Crafted::Added { modification, .. } = crafted else {
            panic!("{crafted:?}");
        };
        assert!(!is_reliability(modification), "{modification}");
        assert!(table.fits(dict("ak_74m"), &[2000], modification));
        assert_eq!(holdings(&account), (before.0, before.1 - 50, before.2 - 1));
    }
    // A backpack of the stock data (the unit tests' archive, without the edit
    // lists that give it a third type on live): of its three types,
    // reliability's never does; with one of its own the other is gained.
    for seed in 0..100 {
        let mut account = rich();
        let backpack = give(&mut account, "scav_back_1", &[1062]);
        let donor = give(&mut account, "scav_back_1", &[1063]);
        let mut rng = Rng::new(seed);
        let crafted = craft(&mut account, backpack, donor, true, false, NOW, &mut rng).unwrap();
        let Crafted::Added { modification, .. } = crafted else {
            panic!("{crafted:?}");
        };
        assert_eq!(table.modification(modification).unwrap().kind, 24);
    }
    // With both held, nothing is left to roll: refused, nothing taken, and
    // the client is told as for any refusal.
    let mut account = rich();
    let backpack = give(&mut account, "scav_back_1", &[1062, 1120]);
    let donor = give(&mut account, "scav_back_1", &[1063]);
    let before = account.clone();
    for gold in [false, true] {
        assert_eq!(
            craft(
                &mut account,
                backpack,
                donor,
                gold,
                false,
                NOW,
                &mut Rng::new(1)
            ),
            Err("nothing left to roll for the item")
        );
    }
    assert_eq!(account, before);
    let mut client = client_of(&account);
    takes(
        &mut client,
        &lobby::permitted_crafted(backpack, Crafted::Failed).unwrap(),
        &account,
    );
    // One with reliability from before still takes its other type.
    let mut account = rich();
    let backpack = give(&mut account, "scav_back_1", &[1062, 1204]);
    let donor = give(&mut account, "scav_back_1", &[1063, 1121]);
    let crafted = craft(
        &mut account,
        backpack,
        donor,
        true,
        false,
        NOW,
        &mut Rng::new(2),
    );
    let Ok(Crafted::Added { modification, .. }) = crafted else {
        panic!("{crafted:?}");
    };
    assert_eq!(table.modification(modification).unwrap().kind, 24);
    assert_eq!(
        own_modifications(account.item(backpack).unwrap()),
        [1062, 1204, modification]
    );
}

/// A reroll never offers reliability, and one an item has turns into
/// another, valid one for its price; with nothing to offer it is refused
/// and takes nothing.
#[test]
fn a_reroll_never_offers_reliability_and_changes_it() {
    let table = Modifications::shared();
    for seed in 0..200 {
        let mut account = rich();
        let rifle = give(&mut account, "ak_74m", &[2093, 2000]);
        let mut client = client_of(&account);
        let mut rng = Rng::new(seed);
        let before = holdings(&account);
        let rolled = reroll(&mut account, rifle, 2093, false, NOW, &mut rng).unwrap();
        assert_eq!(rolled.paid, (2400, 0));
        for &choice in &rolled.choices {
            assert!(!is_reliability(choice), "{choice}");
            assert!(table.fits(dict("ak_74m"), &[2000], choice));
        }
        takes(
            &mut client,
            &lobby::permitted_reroll_result(rifle, rolled.choices, rolled.reroll_seconds),
            &account,
        );
        let chosen = choose_modifier(&mut account, rifle, 2093, (seed % 3) as u32).unwrap();
        assert_eq!(chosen.new_modification, rolled.choices[(seed % 3) as usize]);
        assert_eq!(
            own_modifications(account.item(rifle).unwrap()),
            [chosen.new_modification, 2000]
        );
        assert_eq!(holdings(&account), (before.0 - 2400, before.1, before.2));
        // Another of its modifications: never reliability either.
        let rolled = reroll(&mut account, rifle, 2000, true, NOW, &mut rng).unwrap();
        assert!(rolled.choices.iter().all(|&choice| !is_reliability(choice)));
    }
    // A full backpack of the stock data: its reliability has nothing to turn
    // into (its two other types are held): refused, nothing paid or changed.
    let mut account = rich();
    let backpack = give(&mut account, "scav_back_1", &[1062, 1120, 1201]);
    let before = account.clone();
    assert_eq!(
        reroll(&mut account, backpack, 1201, false, NOW, &mut Rng::new(3)),
        Err(ItemError::Reroll("nothing to offer for the item"))
    );
    assert_eq!(account, before);
    // Its others are offered other strengths of their own type.
    let rolled = reroll(&mut account, backpack, 1120, false, NOW, &mut Rng::new(3)).unwrap();
    assert!(rolled.choices.iter().all(|&choice| {
        !is_reliability(choice) && table.modification(choice).unwrap().kind == 24
    }));
}

/// A drop's modifications are never reliability; an item of few types
/// (a backpack) gets two at rarity 3, and its rank says so.
#[test]
fn a_drop_never_has_reliability() {
    let mut rng = Rng::new(10);
    for _ in 0..200 {
        let mut account = rich();
        for (name, most) in [("ak_74m", 3), ("scav_back_1", 2), ("edge_torso_2", 3)] {
            let item_id = account.add_item(dict(name), FULL_CONDITION);
            let shown = roll_modifications(&mut account, item_id, 3, &mut rng);
            let held = own_modifications(account.item(item_id).unwrap());
            assert!(
                held.iter().all(|&id| !is_reliability(id)),
                "{name}: {held:?}"
            );
            assert_eq!(held.len(), most, "{name}");
            assert_eq!(usize::from(shown), most, "{name}");
        }
        client_of(&account);
    }
}

#[test]
fn a_loaded_item_is_made_safe() {
    let mut account = rich();
    let vest = give(&mut account, "edge_torso_2", &[]);
    let camo = account.add_item(dict("camo_city"), FULL_CONDITION);
    let item = account.item_mut(vest).unwrap();
    // An id of no table, a defect's id as a modification and the other
    // way round, one twice, and four of its own.
    item.modifications = vec![
        (7, false),
        (3000, false),
        (1000, true),
        (1000, false),
        (1000, false),
        (4000, true),
        (1020, false),
        (1060, false),
        (1090, false),
    ];
    item.reroll = Reroll {
        modification: 2000,
        count: 4,
        offer: Some((2000, [1, 2, 3])),
    };
    // A camouflage in a decal's slot, two in one slot, and a rifle.
    let camo_dict = u32::from(dict("camo_city"));
    let upgrade = |param| Upgrade {
        id: camo,
        a: camo_dict,
        value: 0,
        param,
    };
    item.upgrades = vec![upgrade(0), upgrade(3), upgrade(3)];
    assert!(account.repair());
    let item = account.item(vest).unwrap();
    assert_eq!(
        item.modifications,
        [(1000, false), (4000, true), (1020, false), (1060, false)]
    );
    assert_eq!(
        item.reroll,
        Reroll {
            modification: 0,
            count: 4,
            offer: None
        }
    );
    assert_eq!(item.upgrades, [upgrade(3)]);
    // And it is stable, and the client takes it.
    assert!(!account.repair());
    client_of(&account);

    // An upgrade whose cosmetic is gone goes with it.
    account.inventory.retain(|item| item.item_id != camo);
    account.sync_kits();
    assert!(account.item(vest).unwrap().upgrades.is_empty());
}

// ------------------------------------------------------- golden weapons

const SILVER: u32 = golden::SILVER_BONUS;

#[test]
fn a_new_golden_weapon_has_the_silver_bonus_as_its_first_modification() {
    let mut account = rich();
    let rifle = account.add_item(dict("ak_74n_premium"), FULL_CONDITION);
    let item = account.item(rifle).unwrap();
    assert_eq!(item.modifications, [(SILVER, false)]);
    // The lobby's rank 1 (no base rarity), so it can be upgraded.
    assert_eq!(rank(item), 1);
    assert_eq!(item_params(item.dict_id).unwrap().base_rarity_level, 0);
    // The client takes it as it is.
    let client = client_of(&account);
    let shown = client
        .inventory()
        .iter()
        .find(|item| item.item_id == rifle)
        .unwrap();
    let shown: Vec<(u32, bool)> = shown
        .modifications
        .iter()
        .map(|m| (m.id, m.state))
        .collect();
    assert_eq!(shown, [(SILVER, false)]);
    // A plain rifle has none.
    let plain = account.add_item(dict("ak_74n"), FULL_CONDITION);
    assert!(account.item(plain).unwrap().modifications.is_empty());
    // Were one to drop, its rarity would count the silver bonus.
    let mut rng = Rng::new(5);
    let dropped = account.add_item(dict("ak_74n_premium"), FULL_CONDITION);
    assert_eq!(roll_modifications(&mut account, dropped, 2, &mut rng), 2);
    let held = own_modifications(account.item(dropped).unwrap());
    assert_eq!((held.len(), held[0]), (2, SILVER));
}

#[test]
fn a_golden_weapon_is_upgraded_like_its_plain_weapon() {
    let table = Modifications::shared();
    let (golden_rifle, plain_rifle) = (dict("ak_74n_premium"), dict("ak_74n"));
    // The plain weapon's price: the same item level.
    assert_eq!(item_level(golden_rifle), item_level(plain_rifle));
    let price = table.craft.price(item_level(plain_rifle), false);
    let (mut added, mut nothing) = (0, 0);
    // The golden rifle (the silver bonus: rank 1) with a plain one as the
    // donor (rank 0).
    for seed in 0..400 {
        let mut account = rich();
        let rifle = account.add_item(golden_rifle, FULL_CONDITION);
        let donor = account.add_item(plain_rifle, FULL_CONDITION);
        let mut client = client_of(&account);
        let mut rng = Rng::new(seed * 7 + 1);
        let crafted = craft(&mut account, rifle, donor, false, false, NOW, &mut rng).unwrap();
        assert_eq!(account.generic_money, 1_000_000 - price);
        assert!(account.item(donor).is_none());
        // It stays the golden rifle.
        let item = account.item(rifle).unwrap();
        assert_eq!(item.dict_id, golden_rifle);
        let held = own_modifications(item);
        match crafted {
            Crafted::Added { modification, .. } => {
                added += 1;
                assert_eq!(held, [SILVER, modification]);
                assert!(table.fits(plain_rifle, &[], modification));
            }
            // A failure keeps the silver bonus, as any modification (the
            // owner's rule of 2026-10-23).
            Crafted::Nothing { .. } => {
                nothing += 1;
                assert_eq!(held, [SILVER]);
            }
            other => panic!("{other:?}"),
        }
        let message = lobby::permitted_crafted(rifle, crafted).unwrap();
        takes(&mut client, &message, &account);
    }
    // The chances of a plain rifle with one modification and a plain donor.
    assert!((60..140).contains(&added), "{added} gained");
    assert_eq!(added + nothing, 400);

    let mut rng = Rng::new(9);
    let mut account = rich();
    // A plain rifle takes a golden one as its donor too (the owner's rule of
    // 2026-10-22): the golden one is used up, the rifle stays plain.
    let plain = give(&mut account, "ak_74n", &[2000]);
    let golden = account.add_item(golden_rifle, FULL_CONDITION);
    assert!(craft(&mut account, plain, golden, true, false, NOW, &mut rng).is_ok());
    assert!(account.item(golden).is_none());
    assert_eq!(account.item(plain).unwrap().dict_id, dict("ak_74n"));
    // Two golden rifles, as two plain ones.
    let golden = account.add_item(golden_rifle, FULL_CONDITION);
    let other = account.add_item(golden_rifle, FULL_CONDITION);
    assert!(craft(&mut account, golden, other, true, false, NOW, &mut rng).is_ok());
    // Another weapon's plain copy is not its kin.
    let golden = account.add_item(golden_rifle, FULL_CONDITION);
    let ak_74m = give(&mut account, "ak_74m", &[]);
    assert_eq!(
        craft(&mut account, golden, ak_74m, false, false, NOW, &mut rng),
        Err("the donor is another item")
    );
    // The silver bonus counts as a modification: a donor with two is
    // more, and two more make three, the most.
    let better = give(&mut account, "ak_74n", &[2000, 2010]);
    assert_eq!(
        craft(&mut account, golden, better, false, false, NOW, &mut rng),
        Err("the donor has more modifications")
    );
    let full = account.add_item(golden_rifle, FULL_CONDITION);
    account
        .item_mut(full)
        .unwrap()
        .modifications
        .extend([(2000, false), (2010, false)]);
    assert_eq!(
        craft(&mut account, full, ak_74m, false, false, NOW, &mut rng),
        Err("the donor is another item")
    );
    let donor = give(&mut account, "ak_74n", &[]);
    assert_eq!(
        craft(&mut account, full, donor, false, false, NOW, &mut rng),
        Err("the most modifications already")
    );
}

#[test]
fn the_silver_bonus_of_a_golden_weapon_rerolls_like_any_modification() {
    let table = Modifications::shared();
    let mut rng = Rng::new(11);
    let mut account = rich();
    let rifle = account.add_item(dict("ak_74n_premium"), FULL_CONDITION);
    // The plain weapon's prices.
    let level = item_level(dict("ak_74n"));
    let window = reroll_window(&account, rifle).unwrap();
    assert_eq!(
        (window.regular_cost, window.premium_cost),
        (
            table.reroll.price(level, false),
            table.reroll.price(level, true)
        )
    );
    let mut client = client_of(&account);
    let rolled = reroll(&mut account, rifle, SILVER, true, NOW, &mut rng).unwrap();
    // Three of the plain weapon's, none of them the silver bonus (it never
    // rolls: drop weight 0).
    for choice in rolled.choices {
        assert_ne!(choice, SILVER);
        assert!(table.fits(dict("ak_74n"), &[], choice), "{choice}");
    }
    let message = lobby::permitted_reroll_result(rifle, rolled.choices, rolled.reroll_seconds);
    takes(&mut client, &message, &account);
    let chosen = choose_modifier(&mut account, rifle, SILVER, 2).unwrap();
    assert_eq!(chosen.new_modification, rolled.choices[2]);
    let item = account.item(rifle).unwrap();
    assert_eq!(own_modifications(item), [rolled.choices[2]]);
    // And without it, no more silver.
    assert_eq!(table.bonuses(item.dict_id, &item.modifications).money, 0.0);
}

#[test]
fn golden_weapons_owned_before_get_the_silver_bonus_once() {
    let mut account = rich();
    let golden_rifle = dict("ak_74n_premium");
    // As the store had them before the rule: no modification of their own
    // (the silver bonus was the config's).
    let owned = |account: &mut Account, modifications: &[u32]| {
        let item_id = account.add_item(golden_rifle, FULL_CONDITION);
        account.item_mut(item_id).unwrap().modifications =
            modifications.iter().map(|&id| (id, false)).collect();
        item_id
    };
    let bare = owned(&mut account, &[]);
    let one = owned(&mut account, &[2000]);
    let full = owned(&mut account, &[2000, 2010, 2020]);
    let has = owned(&mut account, &[SILVER]);
    let damaged = owned(&mut account, &[]);
    account
        .item_mut(damaged)
        .unwrap()
        .modifications
        .push((3000, true));
    let plain = give(&mut account, "ak_74n", &[]);
    let vss = account.add_item(dict("vss_premium"), FULL_CONDITION);
    account.item_mut(vss).unwrap().modifications.clear();
    // A kit wearing one: it shows another item.
    let slot = (0..23)
        .find(|&slot| Dictionary::shared().fits(golden_rifle, slot))
        .unwrap();
    account.kits[1].slots[usize::from(slot)] = Some(KitSlot {
        item_id: bare,
        count: 1,
    });
    let revision = account.kits[1].revision;

    let done = give_golden_silver(&mut account);
    assert_eq!(done.given, [bare, one, damaged, vss]);
    assert_eq!(done.full, [full]);
    let mods = |account: &Account, item_id| account.item(item_id).unwrap().modifications.clone();
    assert_eq!(mods(&account, bare), [(SILVER, false)]);
    assert_eq!(mods(&account, one), [(SILVER, false), (2000, false)]);
    assert_eq!(
        mods(&account, full),
        [(2000, false), (2010, false), (2020, false)]
    );
    assert_eq!(mods(&account, has), [(SILVER, false)]);
    assert_eq!(mods(&account, damaged), [(SILVER, false), (3000, true)]);
    assert_eq!(mods(&account, vss), [(SILVER, false)]);
    assert!(mods(&account, plain).is_empty());
    assert_ne!(account.kits[1].revision, revision);
    // The client takes it all.
    client_of(&account);
    // Once: a second run changes nothing.
    let again = account.clone();
    assert_eq!(give_golden_silver(&mut account).given, Vec::<u32>::new());
    assert_eq!(account, again);
}

// ------------------------------------------------------- premium armour

const EXPERIENCE: u32 = golden::EXPERIENCE_BONUS;
const BM_REPUTATION: u32 = golden::BLACK_MARKET_REPUTATION_BONUS;

#[test]
fn a_new_premium_armour_piece_has_its_bonuses_as_its_own_modifications() {
    let table = Modifications::shared();
    let mut account = rich();
    let jacket = account.add_item(dict("bm_torso_4_premium"), FULL_CONDITION);
    let item = account.item(jacket).unwrap();
    assert_eq!(
        item.modifications,
        [(EXPERIENCE, false), (BM_REPUTATION, false)]
    );
    // The lobby's rank 2 (no base rarity): one more place to craft.
    assert_eq!(rank(item), 2);
    assert_eq!(item_params(item.dict_id).unwrap().base_rarity_level, 0);
    // It pays as before: 5 % experience, 5 % with the Black Market; its
    // plain piece's default (a pocket) is still the config's.
    let bonuses = table.bonuses(item.dict_id, &item.modifications);
    assert!((bonuses.experience - 0.05).abs() < 1e-6);
    assert!((bonuses.reputation[1] - 0.05).abs() < 1e-6);
    assert_eq!(bonuses.money, 0.0);
    assert_eq!(
        item_params(item.dict_id).unwrap().default_modifications,
        item_params(dict("bm_torso_4"))
            .unwrap()
            .default_modifications
    );
    // The client takes it as it is.
    let client = client_of(&account);
    let shown = client
        .inventory()
        .iter()
        .find(|item| item.item_id == jacket)
        .unwrap();
    let shown: Vec<u32> = shown.modifications.iter().map(|m| m.id).collect();
    assert_eq!(shown, [EXPERIENCE, BM_REPUTATION]);
    // The plain piece has none of its own.
    let plain = account.add_item(dict("bm_torso_4"), FULL_CONDITION);
    assert!(account.item(plain).unwrap().modifications.is_empty());
}

#[test]
fn a_premium_armour_piece_is_upgraded_and_rerolled_like_its_plain_piece() {
    let table = Modifications::shared();
    let (premium, plain) = (dict("bm_legs_4_premium"), dict("bm_legs_4"));
    assert_eq!(item_level(premium), item_level(plain));
    // Its plain piece as the donor: it stays the premium piece.
    let mut rng = Rng::new(3);
    let mut seen_added = false;
    for seed in 0..40 {
        let mut account = rich();
        let legs = account.add_item(premium, FULL_CONDITION);
        let donor = account.add_item(plain, FULL_CONDITION);
        let mut rng = Rng::new(seed * 5 + 2);
        let crafted = craft(&mut account, legs, donor, false, false, NOW, &mut rng).unwrap();
        assert!(account.item(donor).is_none());
        let item = account.item(legs).unwrap();
        assert_eq!(item.dict_id, premium);
        if let Crafted::Added { modification, .. } = crafted {
            seen_added = true;
            assert_eq!(
                own_modifications(item),
                [EXPERIENCE, BM_REPUTATION, modification]
            );
            assert!(table.fits(plain, &[], modification));
        }
    }
    assert!(seen_added);
    // The premium piece is a donor for its plain one too (used up), never
    // another piece. As any donor it may hold no more modifications than the
    // item (the game's own rule): the plain piece holds as many here.
    let mut account = rich();
    let legs = account.add_item(premium, FULL_CONDITION);
    let plain_legs = account.add_item(plain, FULL_CONDITION);
    let held = account.item(legs).unwrap().modifications.clone();
    account.item_mut(plain_legs).unwrap().modifications = held;
    let crafted = craft(&mut account, plain_legs, legs, true, false, NOW, &mut rng);
    assert!(crafted.is_ok(), "{crafted:?}");
    assert!(account.item(legs).is_none());
    let legs = account.add_item(premium, FULL_CONDITION);
    let other = give(&mut account, "scav_legs_4", &[]);
    assert_eq!(
        craft(&mut account, legs, other, false, false, NOW, &mut rng),
        Err("the donor is another item")
    );

    // A premium bonus rerolls like any modification: three of the plain
    // piece's offered, never a premium bonus (drop weight 0).
    let mut client = client_of(&account);
    let rolled = reroll(&mut account, legs, BM_REPUTATION, false, NOW, &mut rng).unwrap();
    for choice in rolled.choices {
        assert!(choice != EXPERIENCE && choice != BM_REPUTATION);
        assert!(table.fits(plain, &[], choice), "{choice}");
    }
    let message = lobby::permitted_reroll_result(legs, rolled.choices, rolled.reroll_seconds);
    takes(&mut client, &message, &account);
    choose_modifier(&mut account, legs, BM_REPUTATION, 0).unwrap();
    let item = account.item(legs).unwrap();
    assert_eq!(own_modifications(item), [EXPERIENCE, rolled.choices[0]]);
    assert_eq!(
        table.bonuses(item.dict_id, &item.modifications).reputation[1],
        0.0
    );
}

#[test]
fn premium_armour_owned_before_gets_its_bonuses_once() {
    let mut account = rich();
    let jacket = dict("scav_torso_4_premium");
    let scav_reputation = golden::SCAVENGER_REPUTATION_BONUS;
    // As the store had them before the rule: no modification of their own
    // (the bonuses were the config's).
    let owned = |account: &mut Account, modifications: &[u32]| {
        let item_id = account.add_item(jacket, FULL_CONDITION);
        account.item_mut(item_id).unwrap().modifications =
            modifications.iter().map(|&id| (id, false)).collect();
        item_id
    };
    let bare = owned(&mut account, &[]);
    let one = owned(&mut account, &[1090]);
    let two = owned(&mut account, &[1090, 1100]);
    let full = owned(&mut account, &[1090, 1100, 1110]);
    let has = owned(&mut account, &[EXPERIENCE, scav_reputation]);
    let half = owned(&mut account, &[scav_reputation]);
    let damaged = owned(&mut account, &[]);
    account
        .item_mut(damaged)
        .unwrap()
        .modifications
        .push((3000, true));
    let plain = give(&mut account, "scav_torso_4", &[]);
    let rifle = account.add_item(dict("ak_74n_premium"), FULL_CONDITION);
    account.item_mut(rifle).unwrap().modifications.clear();
    // A kit wearing one: it shows another item.
    let slot = (0..23)
        .find(|&slot| Dictionary::shared().fits(jacket, slot))
        .unwrap();
    account.kits[1].slots[usize::from(slot)] = Some(KitSlot {
        item_id: bare,
        count: 1,
    });
    let revision = account.kits[1].revision;

    let done = give_premium_armour_bonuses(&mut account);
    assert_eq!(done.given, [bare, one, two, half, damaged]);
    assert_eq!(done.full, [two, full]);
    let mods = |account: &Account, item_id| -> Vec<u32> {
        account
            .item(item_id)
            .unwrap()
            .modifications
            .iter()
            .map(|&(id, _)| id)
            .collect()
    };
    assert_eq!(mods(&account, bare), [EXPERIENCE, scav_reputation]);
    assert_eq!(mods(&account, one), [EXPERIENCE, scav_reputation, 1090]);
    // Room for one: the first.
    assert_eq!(mods(&account, two), [EXPERIENCE, 1090, 1100]);
    assert_eq!(mods(&account, full), [1090, 1100, 1110]);
    assert_eq!(mods(&account, has), [EXPERIENCE, scav_reputation]);
    assert_eq!(mods(&account, half), [EXPERIENCE, scav_reputation]);
    assert_eq!(mods(&account, damaged), [EXPERIENCE, scav_reputation, 3000]);
    assert!(mods(&account, plain).is_empty());
    // Not a golden weapon's silver: that is the other grant's.
    assert!(mods(&account, rifle).is_empty());
    assert_ne!(account.kits[1].revision, revision);
    client_of(&account);
    // A second run changes nothing; nor does the silver grant to armour.
    let again = account.clone();
    assert_eq!(
        give_premium_armour_bonuses(&mut account).given,
        Vec::<u32>::new()
    );
    let silver = give_golden_silver(&mut account);
    assert_eq!(silver.given, [rifle]);
    account.item_mut(rifle).unwrap().modifications.clear();
    assert_eq!(account.inventory, again.inventory);
}

#[test]
fn a_decal_placed_on_a_weapon_asks_for_the_account() {
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[]);
    let decal_id = account.add_item(decal(), FULL_CONDITION);
    // A decal on the rifle (the data patch gives weapons `decal_available`).
    account.item_mut(rifle).unwrap().upgrades.push(Upgrade {
        id: decal_id,
        a: u32::from(decal()),
        value: 0,
        param: 0,
    });
    let revision = account.kits[0].revision;
    account.kits[0].slots[7] = Some(KitSlot {
        item_id: rifle,
        count: 1,
    });
    assert!(place_upgrade(&mut account, rifle, decal_id, 0x0102_0304_0506_0708).unwrap());
    let held = account.item(rifle).unwrap().upgrades[0];
    assert_eq!((held.value, held.param), (0x0102_0304_0506_0708, 0));
    assert_ne!(account.kits[0].revision, revision);
}

/// A weapon's upgrades are the weapon kit's (`survarium::v026g::weapon_kit`):
/// six logical slots kept in the account, at most three records on the
/// wire, which the strict client model holds; an account saved before the
/// kit (a rifle's decal in 0, its camouflage in 2, its scope in 3: the
/// logical slots are those it was saved in) needs no rewrite, it is sent
/// packed. Armour keeps three records sent as they are.
#[test]
fn a_weapons_upgrades_go_to_the_client_packed_in_the_weapon_kit() {
    use survarium::v026g::weapon_kit::{self, KIT_MAGIC};
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[]);
    let vest = give(&mut account, "edge_torso_2", &[]);
    assert!(is_weapon(dict("ak_74m")) && !is_weapon(dict("edge_torso_2")));
    assert_eq!(upgrade_capacity(dict("ak_74m")), 6);
    assert_eq!(upgrade_capacity(dict("edge_torso_2")), limits::UPGRADES);
    let camo = account.add_item(dict("camo_city"), FULL_CONDITION);
    let scope = account.add_item(69, FULL_CONDITION);
    let first = account.add_item(decal(), FULL_CONDITION);
    let saved = vec![
        Upgrade {
            id: first,
            a: decal().into(),
            value: 0x0102_0304,
            param: 0,
        },
        Upgrade {
            id: camo,
            a: dict("camo_city").into(),
            value: 0,
            param: 2,
        },
        Upgrade {
            id: scope,
            a: 362,
            value: 0,
            param: 3,
        },
    ];
    account.item_mut(rifle).unwrap().upgrades = saved.clone();
    account.item_mut(vest).unwrap().upgrades = vec![Upgrade {
        id: camo,
        a: dict("camo_city").into(),
        value: 0,
        param: 3,
    }];
    let sent = wire::item_descr_at(account.item(rifle).unwrap(), NOW).upgrades;
    let kit = u64::from(KIT_MAGIC) << 56 | 3 << 48 | u64::from(dict("camo_city"));
    assert_eq!(
        sent.iter()
            .map(|u| (u.id, u.a, u.value, u.param))
            .collect::<Vec<_>>(),
        [
            (first, u32::from(decal()), 0x0102_0304, 0),
            (scope, 362, kit, 3)
        ]
    );
    // unpacked: the account's own, the camouflage named by its dict id
    let logical = weapon_kit::unpack(&sent).expect("a kit the client takes");
    assert_eq!(
        logical.iter().map(|u| (u.a, u.param)).collect::<Vec<_>>(),
        [
            (u32::from(decal()), 0),
            (u32::from(dict("camo_city")), 2),
            (362, 3)
        ]
    );
    // the vest's camouflage as it is (slot 3, value 0: no kit)
    let vest_sent = wire::item_descr_at(account.item(vest).unwrap(), NOW).upgrades;
    assert_eq!((vest_sent[0].param, vest_sent[0].value), (3, 0));
    assert!(!weapon_kit::is_kit(&vest_sent[0]));
    // the client takes it; the account was not rewritten
    let client = client_of(&account);
    let held = client
        .inventory()
        .iter()
        .find(|item| item.item_id == rifle)
        .unwrap();
    assert_eq!(held.upgrades, sent);
    assert_eq!(account.item(rifle).unwrap().upgrades, saved);
}

/// Two items of one camouflage, the higher on a scoped rifle: the window
/// shows the lowest item of that dict id (the kit names the dict id alone,
/// `docs/notes/weapon-kit.md`), and taking it off takes off the one the
/// rifle holds. Armour still needs the item it holds.
#[test]
fn a_weapons_camouflage_named_by_its_kit_comes_off_by_its_dict_id() {
    let mut account = rich();
    let rifle = give(&mut account, "ak_74m", &[]);
    let vest = give(&mut account, "edge_torso_2", &[]);
    let lower = account.add_item(dict("camo_city"), FULL_CONDITION);
    let worn = account.add_item(dict("camo_city"), FULL_CONDITION);
    let scope = account.add_item(69, FULL_CONDITION);
    assert!(lower < worn);
    let camo = |id| Upgrade {
        id,
        a: dict("camo_city").into(),
        value: 0,
        param: 2,
    };
    let scoped = Upgrade {
        id: scope,
        a: 362,
        value: 0,
        param: 3,
    };
    account.item_mut(rifle).unwrap().upgrades = vec![scoped, camo(worn)];
    account.item_mut(vest).unwrap().upgrades = vec![Upgrade {
        param: 3,
        ..camo(worn)
    }];
    assert_eq!(
        detach_upgrade(&mut account, rifle, lower),
        Ok(Detached::Off)
    );
    assert_eq!(account.item(rifle).unwrap().upgrades, [scoped]);
    // the vest keeps it; on armour another item of its dict id is refused
    assert_eq!(
        detach_upgrade(&mut account, vest, lower),
        Err(ItemError::Upgrade("the cosmetic is not on that item"))
    );
    assert_eq!(account.item(vest).unwrap().upgrades.len(), 1);
    // nothing of that dict id on the rifle any more: refused
    assert_eq!(
        detach_upgrade(&mut account, rifle, lower),
        Err(ItemError::Upgrade("the cosmetic is not on that item"))
    );
    // the item itself still comes off by its id
    account.item_mut(rifle).unwrap().upgrades = vec![scoped, camo(worn)];
    assert_eq!(detach_upgrade(&mut account, rifle, worn), Ok(Detached::Off));
    assert_eq!(account.item(rifle).unwrap().upgrades, [scoped]);
}
