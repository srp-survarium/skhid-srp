//! The item requests (`items`): cosmetics, rerolls, upgrades with a donor
//! and the repair, each from the client's request to what its strict model
//! holds afterwards.

use session::v026g::account::Upgrade;
use session::v026g::items_rules;
use survarium::v026g::lobby::Crafted;
use survarium::v026g::lobby::client_model::Decoded;
use survarium::v026g::lobby::requests::KEEP_OLD_MODIFICATION;
use survarium::v026g::modifications::Modifications;

use super::*;

/// The requests of `items`, as the client writes them
/// (`lobby_requests.md`).
mod ask {
    fn body(sub: u8, values: &[u32]) -> Vec<u8> {
        let mut body = vec![sub];
        for value in values {
            body.extend(value.to_le_bytes());
        }
        body
    }

    /// `46 01 <kit> <host> <upgrade> <slot> <u64 0>`.
    pub fn attach(host: u32, upgrade: u32, slot: u8) -> (u8, Vec<u8>) {
        let mut body = body(1, &[1, host, upgrade]);
        body.push(slot);
        body.extend([0; 8]);
        (0x46, body)
    }

    /// `46 02 <kit> <upgrade> <host>` (patch 30's).
    pub fn detach(host: u32, upgrade: u32) -> (u8, Vec<u8>) {
        (0x46, body(2, &[1, upgrade, host]))
    }

    /// `46 02 <kit> <upgrade>`: a client without patch 30 names no host.
    pub fn detach_stock(upgrade: u32) -> (u8, Vec<u8>) {
        (0x46, body(2, &[1, upgrade]))
    }

    /// `46 05 <kit> <host> <upgrade> <u64>`.
    pub fn place(host: u32, upgrade: u32, value: u64) -> (u8, Vec<u8>) {
        let mut body = body(5, &[1, host, upgrade]);
        body.extend(value.to_le_bytes());
        (0x46, body)
    }

    /// `46 06 <item>`.
    pub fn open_reroll(item: u32) -> (u8, Vec<u8>) {
        (0x46, body(6, &[item]))
    }

    /// `46 07 <item> <modification> <gold>`.
    pub fn reroll(item: u32, modification: u32, gold: bool) -> (u8, Vec<u8>) {
        let mut body = body(7, &[item, modification]);
        body.push(u8::from(gold));
        (0x46, body)
    }

    /// `46 08 <item> <old modification> <choice>`.
    pub fn choose(item: u32, old: u32, choice: u32) -> (u8, Vec<u8>) {
        (0x46, body(8, &[item, old, choice]))
    }

    /// `46 09 <item> <donor> <gold>`.
    pub fn craft(item: u32, donor: u32, gold: bool) -> (u8, Vec<u8>) {
        let mut body = body(9, &[item, donor]);
        body.push(u8::from(gold));
        (0x46, body)
    }

    /// `47 03 <item> 02 <kit item>`.
    pub fn repair_with_kit(item: u32, kit: u32) -> (u8, Vec<u8>) {
        let mut body = body(3, &[item]);
        body.push(2);
        body.extend(kit.to_le_bytes());
        (0x47, body)
    }
}

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

/// A whole item of `name` with these modifications, behind the lobby's
/// back.
fn give(world: &mut World, index: usize, name: &str, modifications: &[u32]) -> u32 {
    let mut given = 0;
    world.change(index, |account| {
        given = account.add_item(dict(name), FULL_CONDITION);
        account.item_mut(given).unwrap().modifications =
            modifications.iter().map(|&id| (id, false)).collect();
    });
    given
}

/// An item's id, its `(modification, state)` and its `(upgrade, slot)`.
type Held = (u32, Vec<(u32, bool)>, Vec<(u32, u8)>);

/// The client holds the account's items: the same ids, modifications and
/// upgrades (a weapon's as they are sent, packed: the weapon kit).
#[track_caller]
fn assert_same_items(world: &mut World, index: usize) {
    let mut held: Vec<Held> = world
        .account(index)
        .inventory
        .iter()
        .map(|item| {
            (
                item.item_id,
                item.modifications.clone(),
                session::v026g::wire::upgrades_descr(item)
                    .iter()
                    .map(|u| (u.id, u.param))
                    .collect(),
            )
        })
        .collect();
    let mut seen: Vec<Held> = world
        .client(index)
        .model
        .inventory()
        .iter()
        .map(|item| {
            (
                item.item_id,
                item.modifications.iter().map(|m| (m.id, m.state)).collect(),
                item.upgrades.iter().map(|u| (u.id, u.param)).collect(),
            )
        })
        .collect();
    for list in [&mut held, &mut seen] {
        list.sort();
        for item in list.iter_mut() {
            item.2.sort();
        }
    }
    assert_eq!(seen, held);
}

#[test]
fn a_sign_in_brings_the_craft_tables() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let tables = world.take(alice).into_iter().find_map(|message| {
        if let Decoded::CraftTables {
            chances,
            prices_general,
            prices_premium,
        } = message
        {
            Some((chances, prices_general, prices_premium))
        } else {
            None
        }
    });
    let (chances, general, premium) = tables.expect("0x9B at sign-in");
    let craft = &Modifications::shared().craft;
    let (data, data_general, data_premium) = craft.tables();
    assert_eq!((general, premium), (data_general, data_premium));
    // The successes are the data's; every failure ("-1 mod.") is 0: an
    // upgrade never takes a modification away (the owner's rule of
    // 2026-10-23), so the window shows no such chance.
    for (index, (&sent, &data)) in chances.iter().zip(&data).enumerate() {
        let failure = index % 2 == 1;
        assert_eq!(sent, if failure { 0.0 } else { data }, "{index}");
    }
    assert!(data.iter().skip(1).step_by(2).any(|&chance| chance > 0.0));
    world.assert_clean();
}

#[test]
fn a_camouflage_and_decals_go_on_a_vest() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let vest = give(&mut world, alice, "edge_torso_2", &[]);
    let camo = give(&mut world, alice, "camo_city", &[]);
    let decal_name = Dictionary::shared()
        .items
        .iter()
        .find(|item| item.category == 26)
        .unwrap()
        .name
        .clone();
    let decal = give(&mut world, alice, &decal_name, &[]);

    world.ask(alice, ask::attach(vest, camo, 3));
    assert_eq!(world.client(alice).ids, [0x83, 0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    assert_eq!(
        decoded[0],
        Decoded::Permitted(Permitted::UpgradeAttached {
            host_item_id: vest,
            upgrade_item_id: camo,
            slot: 3,
            value: 0,
            detach_first: false
        })
    );
    assert_same_items(&mut world, alice);
    assert_eq!(world.account(alice).item(vest).unwrap().upgrades.len(), 1);

    world.ask(alice, ask::attach(vest, decal, 0));
    world.take(alice);
    // The decal's place: no answer, and it is kept.
    world.ask(alice, ask::place(vest, decal, 0x1122_3344_5566_7788));
    assert!(world.take(alice).is_empty());
    let value = world.account(alice).item(vest).unwrap().upgrades[1].value;
    assert_eq!(value, 0x1122_3344_5566_7788);

    // A camouflage does not go into a decal's slot: the game's text, and
    // the client's window is put right.
    world.ask(alice, ask::attach(vest, camo, 1));
    let decoded = world.take(alice);
    assert_eq!(
        denial(&decoded, 0x46).as_deref(),
        Some("st_cannot_upgrade_item")
    );
    assert!(
        decoded
            .iter()
            .any(|m| matches!(m, Decoded::Inventory { .. }))
    );
    assert_same_items(&mut world, alice);
    // A place for a cosmetic that is not on the item.
    world.ask(alice, ask::place(vest, 999, 1));
    assert_eq!(
        denial(&world.take(alice), 0x46).as_deref(),
        Some("st_cannot_upgrade_item")
    );

    world.ask(alice, ask::detach(vest, camo));
    let decoded = world.take(alice);
    assert_eq!(
        decoded[0],
        Decoded::Permitted(Permitted::UpgradeDetached {
            upgrade_item_id: camo,
            host_item_id: vest
        })
    );
    assert_same_items(&mut world, alice);
    world.ask(alice, ask::detach(vest, camo));
    assert_eq!(
        denial(&world.take(alice), 0x46).as_deref(),
        Some("st_cannot_upgrade_item")
    );
    world.assert_clean();
}

#[test]
fn a_decal_placed_on_a_weapon_sends_the_account() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let rifle = give(&mut world, alice, "ak_74m", &[]);
    let decal_name = Dictionary::shared()
        .items
        .iter()
        .find(|item| item.category == 26)
        .unwrap()
        .name
        .clone();
    let decal = give(&mut world, alice, &decal_name, &[]);
    // The decal in slot 0 of the rifle (the data patch gives weapons
    // `decal_available`; the stock data the tests read has none).
    world.change(alice, |account| {
        let a = u32::from(account.item(decal).unwrap().dict_id);
        account.item_mut(rifle).unwrap().upgrades.push(Upgrade {
            id: decal,
            a,
            value: 0,
            param: 0,
        });
    });
    world.ask(alice, ask::place(rifle, decal, 0x1122_3344_5566_7788));
    let decoded = world.take(alice);
    assert!(
        decoded
            .iter()
            .any(|m| matches!(m, Decoded::Inventory { .. }))
    );
    assert_eq!(
        world.account(alice).item(rifle).unwrap().upgrades[0].value,
        0x1122_3344_5566_7788
    );
    assert_same_items(&mut world, alice);
    world.assert_clean();
}

/// The owner's sequence on a Mosin (2026-10-21, the packet log of
/// weapon-camo-27): a decal into slot 0, placed, then dragged into slot 1.
/// The window's movie of 10e3fe00 sent the move (`46 01`, slot 1) and,
/// for the slot it left, `46 02` in the same datagram (CLIK's DragManager:
/// the target's drop, then the source's drag end), which took the decal
/// off the weapon: "it doesn't appear any longer". The movie no longer
/// drops a cosmetic another slot of the window holds; the move keeps its
/// place.
#[test]
fn a_decal_dragged_to_the_other_slot_stays_with_its_place() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let vest = give(&mut world, alice, "edge_torso_2", &[]);
    let decal_name = Dictionary::shared()
        .items
        .iter()
        .find(|item| item.category == 26)
        .unwrap()
        .name
        .clone();
    let decal = give(&mut world, alice, &decal_name, &[]);
    let placed = u64::from_le_bytes([0x4E, 0x6E, 0x08, 0x20, 0x01, 0x46, 0x30, 0x81]);
    world.ask(alice, ask::attach(vest, decal, 0));
    world.take(alice);
    world.ask(alice, ask::place(vest, decal, placed));
    world.take(alice);
    // The fixed movie: the move alone.
    world.ask(alice, ask::attach(vest, decal, 1));
    let decoded = world.take(alice);
    assert_eq!(
        decoded[0],
        Decoded::Permitted(Permitted::UpgradeAttached {
            host_item_id: vest,
            upgrade_item_id: decal,
            slot: 1,
            value: placed,
            detach_first: true
        })
    );
    let held = world.account(alice).item(vest).unwrap().upgrades.clone();
    assert_eq!(held.len(), 1);
    assert_eq!((held[0].param, held[0].value), (1, placed));
    assert_same_items(&mut world, alice);
    // The old movie's two requests: the decal ends off the item.
    world.ask(alice, ask::attach(vest, decal, 0));
    world.ask(alice, ask::detach(vest, decal));
    world.take(alice);
    assert!(world.account(alice).item(vest).unwrap().upgrades.is_empty());
    assert_same_items(&mut world, alice);
    world.assert_clean();
}

/// One camouflage on the worn vest and a spare one: putting it on the spare
/// leaves the worn vest (and its kit) as they were; taking it off the worn
/// vest refreshes the owner's kit, and the squadmate's cache of it when
/// there is a squad, and leaves it on the spare.
fn sharing_camouflage_with_a_spare_vest(with_squad: bool) {
    let mut world = World::new();
    let observer = with_squad.then(|| world.connect("Alice"));
    let actor = world.connect("Bob");
    let source = give(&mut world, actor, "edge_torso_2", &[]);
    let destination = give(&mut world, actor, "bm_torso_1", &[]);
    let cosmetic = give(&mut world, actor, "camo_city", &[]);
    let torso = (0..23)
        .find(|&slot| Dictionary::shared().fits(dict("edge_torso_2"), slot))
        .unwrap();
    world.ask(
        actor,
        request::move_item(1, source, dict("edge_torso_2"), INVENTORY_SLOT, torso, 1),
    );
    world.ask(actor, ask::attach(source, cosmetic, 3));
    let revision = world.account(actor).kits[0].revision;
    let actor_id = world.ids[actor].0;
    let squad_revision = |messages: &[Decoded]| {
        messages.iter().rev().find_map(|message| {
            let Decoded::SquadInfo { members, .. } = message else {
                return None;
            };
            members
                .iter()
                .find(|member| member.account_id == actor_id)
                .map(|member| (member.kit_id, member.kit_revision))
        })
    };
    let listed = observer.map(|observer| {
        form_squad(&mut world, observer, actor);
        world.advance(Duration::from_secs(2));
        // An unacknowledged earlier squad update would defer the next one.
        for (address, _) in &world.clients {
            let link = &world.core.links[address];
            let crate::v026g::core::Stage::Player(player) = &link.stage else {
                panic!("connected player")
            };
            assert!(
                player
                    .squad_serial
                    .is_some_and(|serial| link.transport.is_delivered(serial))
            );
        }
        assert!(!world.core.squads_dirty);
        squad_revision(&world.take(observer)).expect("initial squad revision")
    });
    world.take(actor);
    world.ask(actor, ask::attach(destination, cosmetic, 3));
    let messages = world.take(actor);
    assert!(messages.iter().any(|message| matches!(
        message,
        Decoded::Permitted(Permitted::UpgradeAttached {
            detach_first: false,
            ..
        })
    )));
    assert_eq!(world.account(actor).kits[0].revision, revision);
    for vest in [source, destination] {
        assert_eq!(
            world.account(actor).item(vest).unwrap().upgrades[0].id,
            cosmetic
        );
    }
    assert_same_items(&mut world, actor);
    world.ask(actor, ask::detach(source, cosmetic));
    let messages = world.take(actor);
    assert!(messages.iter().any(|message| matches!(
        message,
        Decoded::Permitted(Permitted::UpgradeDetached {
            upgrade_item_id,
            host_item_id,
        }) if *upgrade_item_id == cosmetic && *host_item_id == source
    )));
    let updated = messages
        .iter()
        .find_map(|message| match message {
            Decoded::Account(account) => Some(account),
            _ => None,
        })
        .expect("owner receives updated account");
    assert_eq!(
        updated.revisions[0],
        revision + 1,
        "owner must refresh the equipped vest"
    );
    assert!(
        world
            .account(actor)
            .item(source)
            .unwrap()
            .upgrades
            .is_empty()
    );
    assert_eq!(
        world.account(actor).item(destination).unwrap().upgrades[0].id,
        cosmetic
    );
    assert_same_items(&mut world, actor);
    if let Some(observer) = observer {
        world.advance(Duration::from_secs(2));
        let before = listed.unwrap();
        let after =
            squad_revision(&world.take(observer)).expect("squadmate receives changed revision");
        assert_eq!(before.0, after.0);
        assert_ne!(before.1, after.1);
        world.ask(observer, request::squad_member(actor_id, after.0));
        assert!(world.take(observer).iter().any(|message| matches!(message,
            Decoded::SquadMemberAccount { revision, .. } if *revision == after.1
        )));
    }
    world.assert_clean();
}

#[test]
fn shared_camouflage_taken_off_refreshes_the_owner_without_a_squad() {
    sharing_camouflage_with_a_spare_vest(false);
}

#[test]
fn shared_camouflage_taken_off_refreshes_the_squadmate() {
    sharing_camouflage_with_a_spare_vest(true);
}

#[test]
fn one_camouflage_on_three_vests_comes_off_one_and_goes_with_its_sale() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let vests = [
        give(&mut world, alice, "edge_torso_2", &[]),
        give(&mut world, alice, "bm_torso_1", &[]),
        give(&mut world, alice, "edge_torso_2", &[]),
    ];
    let camo = give(&mut world, alice, "camo_city", &[]);
    let decal_name = Dictionary::shared()
        .items
        .iter()
        .find(|item| item.category == 26)
        .unwrap()
        .name
        .clone();
    let decal = give(&mut world, alice, &decal_name, &[]);
    for vest in vests {
        world.ask(alice, ask::attach(vest, camo, 3));
        world.ask(alice, ask::attach(vest, decal, 0));
    }
    world.take(alice);
    assert_same_items(&mut world, alice);
    let inventory = world.account(alice).inventory.len();
    // The decal's place is each vest's own.
    for (index, vest) in vests.into_iter().enumerate() {
        world.ask(alice, ask::place(vest, decal, 0x100 + index as u64));
    }
    assert!(world.take(alice).is_empty());
    for (index, vest) in vests.into_iter().enumerate() {
        let upgrades = world.account(alice).item(vest).unwrap().upgrades.clone();
        assert_eq!(upgrades.len(), 2);
        assert_eq!(upgrades[1].value, 0x100 + index as u64);
    }
    // Off the middle vest: the others keep it; nothing new is in the stash.
    world.ask(alice, ask::detach(vests[1], camo));
    world.take(alice);
    assert_same_items(&mut world, alice);
    let worn = |world: &World, upgrade: u32| -> Vec<u32> {
        vests
            .into_iter()
            .filter(|&vest| {
                world
                    .account(alice)
                    .item(vest)
                    .unwrap()
                    .upgrades
                    .iter()
                    .any(|held| held.id == upgrade)
            })
            .collect()
    };
    assert_eq!(worn(&world, camo), [vests[0], vests[2]]);
    assert_eq!(worn(&world, decal), vests);
    assert_eq!(world.account(alice).inventory.len(), inventory);
    // Sold, it comes off every vest; the client's echo (Flash's
    // on_item_sold sends `46 02` for it with one of them) is answered as
    // off, not refused (which would show an error for the sale).
    world.ask(alice, request::sell(camo, 1));
    world.take(alice);
    assert_eq!(worn(&world, camo), Vec::<u32>::new());
    assert_eq!(worn(&world, decal), vests);
    assert_same_items(&mut world, alice);
    world.ask(alice, ask::detach(vests[0], camo));
    let decoded = world.take(alice);
    assert_eq!(denial(&decoded, 0x46), None);
    assert_eq!(
        decoded[0],
        Decoded::Permitted(Permitted::UpgradeDetached {
            upgrade_item_id: camo,
            host_item_id: vests[0]
        })
    );
    assert_same_items(&mut world, alice);
    world.assert_clean();
}

/// A client without patch 30 (every released revision) takes an upgrade off
/// with `46 02 <kit> <upgrade>`: it is answered, and the upgrade comes off
/// the first item of the inventory holding it, where the client's own
/// answer handling takes it off too. Before 2026-10-17 it was refused as
/// cut short (the owner's "неможливо перемістити приціл").
#[test]
fn a_client_without_patch_30_takes_an_upgrade_off_its_first_holder() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let first = give(&mut world, alice, "edge_torso_2", &[]);
    let second = give(&mut world, alice, "bm_torso_1", &[]);
    let camo = give(&mut world, alice, "camo_city", &[]);
    for vest in [first, second] {
        world.ask(alice, ask::attach(vest, camo, 3));
    }
    world.take(alice);
    world.ask(alice, ask::detach_stock(camo));
    let decoded = world.take(alice);
    assert_eq!(denial(&decoded, 0x46), None);
    assert_eq!(
        decoded[0],
        Decoded::Permitted(Permitted::UpgradeDetached {
            upgrade_item_id: camo,
            host_item_id: 0
        })
    );
    let wears =
        |world: &World, vest: u32| !world.account(alice).item(vest).unwrap().upgrades.is_empty();
    assert!(!wears(&world, first) && wears(&world, second));
    assert_same_items(&mut world, alice);
    world.assert_clean();
}

/// A weapon's upgrades are the weapon kit's (`items_rules`,
/// `survarium::v026g::weapon_kit`): up to six in the account, at most three
/// records on the wire, and a change is answered with the inventory and the
/// account alone: no `83 46 02` (or `83 46 01`) whose own copy the client
/// would write into the item. The four the account holds here (two decals,
/// a camouflage, the game's 1P69 standing for a scope variant) reach the
/// client as the decals and the kit; taking the camouflage off leaves the
/// decals and a kit of the scope alone.
#[test]
fn a_weapons_upgrade_change_is_answered_with_the_inventory_alone() {
    use survarium::v026g::weapon_kit::{self, KIT_MAGIC};
    let mut world = World::new();
    let alice = world.connect("Alice");
    let rifle = give(&mut world, alice, "ak_74m", &[]);
    let camo = give(&mut world, alice, "camo_city", &[]);
    let scope = give(&mut world, alice, "scope_leupold", &[]);
    let decals: Vec<String> = Dictionary::shared()
        .items
        .iter()
        .filter(|item| item.category == 26)
        .take(2)
        .map(|item| item.name.clone())
        .collect();
    let first = give(&mut world, alice, &decals[0], &[]);
    let second = give(&mut world, alice, &decals[1], &[]);
    let (camo_dict, first_dict, second_dict) =
        (dict("camo_city"), dict(&decals[0]), dict(&decals[1]));
    world.change(alice, |account| {
        account.item_mut(rifle).unwrap().upgrades = vec![
            Upgrade {
                id: camo,
                a: camo_dict.into(),
                value: 0,
                param: 2,
            },
            Upgrade {
                id: first,
                a: first_dict.into(),
                value: 0x0102,
                param: 0,
            },
            Upgrade {
                id: scope,
                a: 362,
                value: 0,
                param: 3,
            },
            Upgrade {
                id: second,
                a: second_dict.into(),
                value: 0x0304,
                param: 1,
            },
        ];
    });
    world.take(alice);
    let sent = session::v026g::wire::upgrades_descr(world.account(alice).item(rifle).unwrap());
    assert_eq!(sent.len(), 3);
    assert_eq!(
        (sent[2].id, sent[2].a, sent[2].value, sent[2].param),
        (
            scope,
            362,
            u64::from(KIT_MAGIC) << 56 | 3 << 48 | u64::from(camo_dict),
            3
        )
    );
    assert_eq!(weapon_kit::unpack(&sent).map(|logical| logical.len()), Ok(4));

    world.ask(alice, ask::detach(rifle, camo));
    assert_eq!(world.client(alice).ids, [0x89, 0x87, 0x8B]);
    world.take(alice);
    assert_same_items(&mut world, alice);
    let held = world
        .client(alice)
        .model
        .inventory()
        .iter()
        .find(|item| item.item_id == rifle)
        .cloned();
    let held = held.expect("the rifle");
    assert_eq!(
        held.upgrades
            .iter()
            .map(|u| (u.id, u.value, u.param))
            .collect::<Vec<_>>(),
        [
            (first, 0x0102, 0),
            (second, 0x0304, 1),
            (scope, u64::from(KIT_MAGIC) << 56 | 3 << 48, 3)
        ]
    );
    world.assert_clean();
}

/// The players' game data before the release patch has no scope variant
/// (`survarium::v026g::scopes`): no scope goes on a weapon, the stock
/// scope items are not sold, and a `46 01` for one is refused with the
/// game's text and the client's window put right. The rules with a pair
/// table are `session::v026g::items_rules`' tests.
#[test]
fn a_scope_is_refused_while_the_game_data_has_no_variant() {
    use survarium::v026g::prices;
    use survarium::v026g::scopes::ScopeFits;
    assert!(ScopeFits::shared().is_empty());
    let mut world = World::new();
    let alice = world.connect("Alice");
    let svd = give(&mut world, alice, "svd", &[]);
    let rifle = give(&mut world, alice, "ak_74m", &[]);
    let scopes = ["scope_pso1", "scope_leupold", "scope_giperon"]
        .map(|name| give(&mut world, alice, name, &[]));
    for scope in scopes {
        assert!(prices::price(world.account(alice).item(scope).unwrap().dict_id).is_none());
        for host in [svd, rifle] {
            world.ask(alice, ask::attach(host, scope, 3));
            let decoded = world.take(alice);
            assert_eq!(
                denial(&decoded, 0x46).as_deref(),
                Some("st_cannot_upgrade_item")
            );
            assert!(
                decoded
                    .iter()
                    .any(|m| matches!(m, Decoded::Inventory { .. }))
            );
            assert_same_items(&mut world, alice);
        }
    }
    assert!(world.account(alice).item(svd).unwrap().upgrades.is_empty());
    world.assert_clean();
}

#[test]
fn a_modification_is_rerolled_and_chosen() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, |account| {
        account.generic_money = 100_000;
        // A new account has no gold.
        account.premium_money = 1_000;
    });
    let rifle = give(&mut world, alice, "ak_74m", &[2000, 2010]);

    world.ask(alice, ask::open_reroll(rifle));
    assert_eq!(
        world.take(alice),
        [Decoded::Permitted(Permitted::RerollWindow {
            item_id: rifle,
            a: 2400,
            b: 12,
            more: None
        })]
    );

    world.ask(alice, ask::reroll(rifle, 2000, false));
    assert_eq!(world.client(alice).ids, [0x83, 0x8B]);
    let decoded = world.take(alice);
    let Decoded::Permitted(Permitted::RerollResult {
        item_id,
        choices,
        reroll_time,
    }) = decoded[0]
    else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!(item_id, rifle);
    // No wait before the next silver reroll (the owner's rule of 2026-10-22).
    assert_eq!(reroll_time, 0);
    assert_eq!(
        decoded[1],
        Decoded::Money {
            generic: 100_000 - 2400,
            premium: 1000
        }
    );
    assert_eq!(world.account(alice).generic_money, 100_000 - 2400);

    // The window opened again (the client came back): the choice still
    // waits.
    world.ask(alice, ask::open_reroll(rifle));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::RerollWindow {
            more: Some((2000, _)),
            ..
        })
    ));
    assert!(matches!(
        decoded[1],
        Decoded::Permitted(Permitted::RerollResult { choices: again, .. }) if again == choices
    ));

    // A choice that is none: refused, and the window is filled again.
    world.ask(alice, ask::choose(rifle, 2000, 7));
    let decoded = world.take(alice);
    assert_eq!(
        denial(&decoded, 0x46).as_deref(),
        Some("st_cannot_select_rerolled_modification")
    );
    assert!(
        decoded
            .iter()
            .any(|message| matches!(message, Decoded::Permitted(Permitted::RerollWindow { .. })))
    );

    world.ask(alice, ask::choose(rifle, 2000, 2));
    let decoded = world.take(alice);
    let Decoded::Permitted(Permitted::ModifierChosen {
        item_id,
        old_modification,
        new_modification,
        rest,
    }) = decoded[0]
    else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!(
        (item_id, old_modification, new_modification),
        (rifle, 2000, choices[2])
    );
    // The next change costs the same (the owner's rule of 2026-10-25).
    assert_eq!(rest[..2], [2400, 12]);
    // No wait (the owner's rule of 2026-10-22).
    assert_eq!(rest[2], 0);
    assert_eq!(decoded.len(), 1);
    // The client changed its copy itself, as the account was.
    assert_same_items(&mut world, alice);
    assert_eq!(
        items_rules::own_modifications(world.account(alice).item(rifle).unwrap()),
        [choices[2], 2010]
    );

    // Silver again at once: no wait (the owner's rule of 2026-10-22).
    world.ask(alice, ask::reroll(rifle, choices[2], false));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::RerollResult { reroll_time: 0, .. })
    ));
    assert_eq!(world.account(alice).generic_money, 100_000 - 2 * 2400);
    // An item that is none.
    world.ask(alice, ask::open_reroll(999));
    assert_eq!(
        denial(&world.take(alice), 0x46).as_deref(),
        Some("st_cannot_select_rerolled_modification")
    );
    world.assert_clean();
}

/// `docs/notes/reroll-keep-old.md`: the window's Cancel (keep old), after the
/// game was started again.
#[test]
fn a_rerolled_modification_may_be_kept() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, |account| account.generic_money = 100_000);
    let rifle = give(&mut world, alice, "ak_74m", &[2000, 2010]);

    world.ask(alice, ask::reroll(rifle, 2000, false));
    let decoded = world.take(alice);
    let Decoded::Permitted(Permitted::RerollResult { choices, .. }) = decoded[0] else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!(world.account(alice).generic_money, 100_000 - 2400);

    // The game started again: the offer waits in the window, paid for.
    let (account_id, _) = world.ids[alice];
    let session = world.new_session(account_id);
    let again = world.connect_session((account_id, session));
    world.take(again);
    world.ask(again, ask::open_reroll(rifle));
    let decoded = world.take(again);
    assert!(matches!(
        decoded[1],
        Decoded::Permitted(Permitted::RerollResult { choices: shown, .. }) if shown == choices
    ));
    assert_same_items(&mut world, again);

    world.ask(again, ask::choose(rifle, 2000, KEEP_OLD_MODIFICATION));
    let decoded = world.take(again);
    let Decoded::Permitted(Permitted::ModifierChosen {
        item_id,
        old_modification,
        new_modification,
        rest,
    }) = decoded[0]
    else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!(
        (item_id, old_modification, new_modification),
        (rifle, 2000, 2000)
    );
    // The change was made and paid: the next costs the same.
    assert_eq!(rest[..2], [2400, 12]);
    assert_eq!(decoded.len(), 1);
    assert_same_items(&mut world, again);
    {
        let account = world.account(again);
        let item = account.item(rifle).unwrap();
        assert_eq!(items_rules::own_modifications(item), [2000, 2010]);
        assert_eq!(item.reroll.offer, None);
        assert_eq!(account.generic_money, 100_000 - 2400);
    }

    // Answered: the window is the item's as it is, with no offer.
    world.ask(again, ask::choose(rifle, 2000, KEEP_OLD_MODIFICATION));
    let decoded = world.take(again);
    assert_eq!(
        denial(&decoded, 0x46).as_deref(),
        Some("st_cannot_select_rerolled_modification")
    );
    assert!(matches!(
        decoded.last(),
        Some(Decoded::Permitted(Permitted::RerollWindow {
            more: Some((2000, _)),
            ..
        }))
    ));
    world.assert_clean();
}

/// The owner's rule of 2026-10-22 (`docs/notes/reroll-keep-old.md`): after
/// one modification was changed, another is changed too, at the next price;
/// while an offer waits, asking for another shows that offer again, free.
#[test]
fn another_modification_is_rerolled_after_the_first() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, |account| account.generic_money = 100_000);
    let premium = world.account(alice).premium_money;
    let rifle = give(&mut world, alice, "ak_74m", &[2000, 2010]);

    // 2000: offered and paid.
    world.ask(alice, ask::reroll(rifle, 2000, false));
    let decoded = world.take(alice);
    let Decoded::Permitted(Permitted::RerollResult { choices: first, .. }) = decoded[0] else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!(world.account(alice).generic_money, 100_000 - 2400);

    // 2010 asked for while 2000's offer waits: 2000's three again, nothing
    // charged.
    world.ask(alice, ask::reroll(rifle, 2010, false));
    let decoded = world.take(alice);
    assert_eq!(denial(&decoded, 0x46), None);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::RerollResult { item_id, choices, .. })
            if item_id == rifle && choices == first
    ));
    assert_eq!(
        decoded[1],
        Decoded::Money {
            generic: 100_000 - 2400,
            premium
        }
    );
    assert_eq!(world.account(alice).item(rifle).unwrap().reroll.count, 1);

    world.ask(alice, ask::choose(rifle, 2000, 0));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::ModifierChosen { new_modification, .. })
            if new_modification == first[0]
    ));

    // 2010 now: not refused, at the same price.
    world.ask(alice, ask::reroll(rifle, 2010, false));
    let decoded = world.take(alice);
    assert_eq!(denial(&decoded, 0x46), None);
    let Decoded::Permitted(Permitted::RerollResult {
        item_id,
        choices: second,
        ..
    }) = decoded[0]
    else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!(item_id, rifle);
    assert_eq!(
        decoded[1],
        Decoded::Money {
            generic: 100_000 - 2 * 2400,
            premium
        }
    );
    assert_eq!(world.account(alice).generic_money, 100_000 - 2 * 2400);

    // The window: 2010 is the one changed last, the next change costs
    // the same, the offer waiting.
    world.ask(alice, ask::open_reroll(rifle));
    let decoded = world.take(alice);
    assert_eq!(
        decoded[0],
        Decoded::Permitted(Permitted::RerollWindow {
            item_id: rifle,
            a: 2400,
            b: 12,
            more: Some((2010, 0))
        })
    );
    assert!(matches!(
        decoded[1],
        Decoded::Permitted(Permitted::RerollResult { choices, .. }) if choices == second
    ));

    world.ask(alice, ask::choose(rifle, 2010, 1));
    let decoded = world.take(alice);
    let Decoded::Permitted(Permitted::ModifierChosen {
        old_modification,
        new_modification,
        rest,
        ..
    }) = decoded[0]
    else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!((old_modification, new_modification), (2010, second[1]));
    assert_eq!(rest, [2400, 12, 0]);
    assert_same_items(&mut world, alice);
    assert_eq!(
        items_rules::own_modifications(world.account(alice).item(rifle).unwrap()),
        [first[0], second[1]]
    );
    world.ask(alice, ask::open_reroll(rifle));
    assert_eq!(
        world.take(alice),
        [Decoded::Permitted(Permitted::RerollWindow {
            item_id: rifle,
            a: 2400,
            b: 12,
            more: Some((second[1], 0))
        })]
    );
    world.assert_clean();
}

#[test]
fn an_item_is_crafted_with_a_donor() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, |account| account.premium_money = 500);
    let rifle = give(&mut world, alice, "ak_74m", &[2000]);
    let donor = give(&mut world, alice, "ak_74m", &[2011]);
    let pistol = 1;

    // The pistol is no donor for the rifle: the window gets its "can't".
    world.ask(alice, ask::craft(rifle, pistol, true));
    assert_eq!(
        world.take(alice),
        [Decoded::Permitted(Permitted::Crafted {
            item_id: rifle,
            crafted: Crafted::Failed
        })]
    );
    assert_eq!(world.account(alice).premium_money, 500);

    // In gold, with as many modifications as the donor: one more, always.
    world.ask(alice, ask::craft(rifle, donor, true));
    assert_eq!(world.client(alice).ids, [0x83, 0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    let Decoded::Permitted(Permitted::Crafted {
        item_id,
        crafted: Crafted::Added {
            consumed_item_id,
            modification,
        },
    }) = decoded[0]
    else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!((item_id, consumed_item_id), (rifle, donor));
    {
        let account = world.account(alice);
        assert_eq!(account.premium_money, 500 - 50);
        assert!(account.item(donor).is_none());
        assert_eq!(
            items_rules::own_modifications(account.item(rifle).unwrap()),
            [2000, modification]
        );
    }
    assert_same_items(&mut world, alice);
    world.assert_clean();
}

#[test]
fn defects_are_repaired_in_time_or_at_once() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let rifle = give(&mut world, alice, "ak_74m", &[]);
    let spare = give(&mut world, alice, "ak_74m", &[]);
    let mut kits = 0;
    world.change(alice, |account| {
        account.generic_money = 1_000_000;
        kits = account.add_item(dict("repair_voucher"), 2);
        for item_id in [rifle, spare] {
            let item = account.item_mut(item_id).unwrap();
            item.condition_or_stack = 500_000;
            item.modifications.push((3000, true));
        }
    });
    // The client sees the damage.
    assert_same_items(&mut world, alice);
    let (silver, _, seconds, _) =
        items_rules::repair_quote(world.account(alice).item(rifle).unwrap());
    assert_eq!(seconds, 360);

    // In silver: six minutes a defect.
    world.ask(alice, request::repair(rifle, 0));
    assert_eq!(world.client(alice).ids, [0x83, 0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::Repaired {
            repaired: survarium::v026g::lobby::Repaired::Timed { repair_time: 360 },
            ..
        })
    ));
    assert_eq!(world.account(alice).generic_money, 1_000_000 - silver);
    assert_same_items(&mut world, alice);
    let held = world.client(alice).model.inventory().to_vec();
    let seen = held.iter().find(|item| item.item_id == rifle).unwrap();
    assert_eq!(seen.condition_or_stack, FULL_CONDITION);
    assert!(
        (350..=360).contains(&seen.repair_time),
        "{}",
        seen.repair_time
    );
    // Not again while it lasts.
    world.ask(alice, request::repair(rifle, 0));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_repair_item")
    );

    // The time is over: the client is told which item, once.
    world.account(alice).item_mut(rifle).unwrap().repair_time = 1;
    world.advance(Duration::from_millis(1500));
    let told: Vec<String> = world
        .take(alice)
        .into_iter()
        .filter_map(|message| match message {
            Decoded::Important { kind: 6, text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(told, [format!("#ir[{rifle}]")]);
    assert_eq!(world.account(alice).item(rifle).unwrap().repair_time, 0);
    world.advance(Duration::from_millis(1500));
    assert!(
        !world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::Important { kind: 6, .. }))
    );

    // With a repair kit: at once, one a defect.
    world.ask(alice, ask::repair_with_kit(spare, kits));
    let decoded = world.take(alice);
    assert_eq!(
        decoded[0],
        Decoded::Permitted(Permitted::Repaired {
            item_id: spare,
            repaired: survarium::v026g::lobby::Repaired::WithKit {
                kit_item_id: kits,
                used: 1
            }
        })
    );
    assert_eq!(
        world.account(alice).item(kits).unwrap().condition_or_stack,
        1
    );
    assert_same_items(&mut world, alice);
    // No kit for an item with no defect; gold neither.
    world.ask(alice, ask::repair_with_kit(spare, kits));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_repair_item")
    );
    world.ask(alice, request::repair(spare, 1));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_repair_item")
    );
    world.assert_clean();
}
