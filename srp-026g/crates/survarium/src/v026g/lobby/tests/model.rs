//! The client model: it reads what the writers write, and refuses what the
//! real client would crash on or misread (`lobby.md` §3, `results.md` §6).

use std::num::NonZeroU16;

use super::super::client_model::{Client, Decoded, FriendshipPush, ModelError, Permitted};
use super::super::*;
use super::h;

const PISTOL: u16 = 18;
const AMMO: u16 = 50;

fn pistol(item_id: u32) -> ItemDescr {
    ItemDescr::plain(item_id, PISTOL, FULL_CONDITION)
}

fn ammo(item_id: u32, count: u32) -> ItemDescr {
    ItemDescr::plain(item_id, AMMO, count)
}

fn account(items: Vec<ItemDescr>) -> OwnAccount {
    let mut kit = Kit::empty(1);
    for (index, _) in items.iter().enumerate() {
        kit.slots[7 + index] = Some(KitSlot {
            item_index: index as u16,
            count: 1,
        });
    }
    kit.modifiers[5] = 1.5;
    kit.modifiers[67] = -0.25;
    OwnAccount {
        body: AccountBody {
            team: 0,
            name: "Alice".into(),
            clan_tag: "SR".into(),
            protection: NO_PROTECTION,
            kits: vec![kit],
            items,
        },
        revisions: vec![3],
        active_kit: 1,
        autobuy: true,
        auto_repair: false,
        level: 4,
        exp_total: 10,
        exp_level_start: 5,
        exp_next_level: 20,
    }
}

#[test]
fn forbidden_ids_never_reach_a_reader() {
    let mut client = Client::signed_in();
    for id in (0..0x80_u8)
        .chain(0xAB..=0xFF)
        .chain([0x81, 0x86, 0xA0, 0xA7])
    {
        assert_eq!(client.receive_raw(id, &[]), Err(ModelError::BadId(id)));
    }
    assert!(client.receive_raw(0xAA, &[]).is_ok());
}

#[test]
fn only_the_sign_in_answer_is_taken_first() {
    let mut client = Client::new();
    assert_eq!(
        client.receive(&money(1, 2)),
        Err(ModelError::NotSignedIn(ServerId::Money))
    );
    assert_eq!(
        client.receive(&signed_in(0, false)),
        Ok(Decoded::SignedIn {
            client_state: 0,
            flag: false
        })
    );
    assert!(client.is_signed_in());
    assert!(client.receive(&money(1, 2)).is_ok());
    assert_eq!(client.received(), 2);
}

#[test]
fn the_account_needs_its_items_in_the_inventory_first() {
    let items = vec![pistol(1), ammo(2, 50)];
    let own = own_account(&account(items.clone())).unwrap();

    // 0x87 before 0x89: the client copies each item from an inventory that
    // lacks it.
    let mut client = Client::signed_in();
    assert_eq!(client.receive(&own), Err(ModelError::NotInInventory(1)));

    // An inventory that lacks one of them.
    let mut client = Client::signed_in();
    client.receive(&inventory(&items[..1]).unwrap()).unwrap();
    assert_eq!(client.receive(&own), Err(ModelError::NotInInventory(2)));

    // The right order reads back what was written.
    let mut client = Client::signed_in();
    assert_eq!(
        client.receive(&inventory(&items).unwrap()),
        Ok(Decoded::Inventory {
            items: items.clone(),
            spent: SpentMoney::default(),
        })
    );
    assert_eq!(client.receive(&own), Ok(Decoded::Account(account(items))));
}

#[test]
fn raw_accounts_the_client_would_misread() {
    let mut client = Client::signed_in();
    client.receive(&inventory(&[pistol(1)]).unwrap()).unwrap();
    let tail = "01000000 01000000 00 01 00000000 00000000 01000000";
    let kit = |mask: &str, slots: &str| {
        format!("00 01000000 00 {mask} {slots} 0000000000000000 0000000000000000")
    };
    let item = "01000000 1200 40420F00 00000000 00000000 00 00";
    let send = |client: &mut Client, text: &str| client.receive_raw(0x87, &h(text));

    // As a writer would send it.
    let good = format!(
        "00 'A' 00 0A000000 01 01 {} {item} {tail}",
        kit("80000000", "0000 0100")
    );
    assert!(send(&mut client, &good).is_ok());

    // A slot naming item 1 of a table of 1.
    let stale = format!(
        "00 'A' 00 0A000000 01 01 {} {item} {tail}",
        kit("80000000", "0100 0100")
    );
    assert_eq!(
        send(&mut client, &stale),
        Err(ModelError::BadItemIndex { index: 1, items: 1 })
    );
    // Four kits.
    assert!(matches!(
        send(&mut client, "00 'A' 00 0A000000 04 00"),
        Err(ModelError::Overflow { what: "kits", .. })
    ));
    // Seventy items.
    assert!(matches!(
        send(&mut client, "00 'A' 00 0A000000 00 46"),
        Err(ModelError::Overflow {
            what: "account items",
            len: 70,
            most: 69,
            ..
        })
    ));
    // A clan abbreviation of 15 bytes runs into the kit count.
    assert!(matches!(
        send(&mut client, "00 'A' 'ABCDEFGHIJKLMNO' 0A000000 00 00"),
        Err(ModelError::Overflow {
            what: "account clan tag",
            ..
        })
    ));
    // The pistol written as a stack (no times): the client reads the two
    // counts as times and runs past the body.
    let as_stack = format!(
        "00 'A' 00 0A000000 01 01 {} 01000000 1200 40420F00 00 00 {tail}",
        kit("80000000", "0000 0100")
    );
    assert!(send(&mut client, &as_stack).is_err());
    // An unknown dict id.
    let unknown = format!(
        "00 'A' 00 0A000000 01 01 {} 01000000 0F27 01000000 00 00 {tail}",
        kit("80000000", "0000 0100")
    );
    assert_eq!(
        send(&mut client, &unknown),
        Err(ModelError::UnknownItem(9999))
    );
    // An active kit id that is none of the kits.
    let other_kit = good.replace(tail, "01000000 09000000 00 01 00000000 00000000 01000000");
    assert!(matches!(
        send(&mut client, &other_kit),
        Err(ModelError::Bad(ServerId::Account, _))
    ));
    // One byte too many, one too few.
    assert_eq!(
        send(&mut client, &format!("{good} 00")),
        Err(ModelError::Trailing {
            id: ServerId::Account,
            extra: 1
        })
    );
    let mut short = h(&good);
    short.pop();
    assert_eq!(
        client.receive_raw(0x87, &short),
        Err(ModelError::Short(ServerId::Account))
    );
}

#[test]
fn raw_items_the_client_would_crash_on() {
    let mut client = Client::signed_in();
    // Nine modifications: a fatal assert.
    let nine = format!(
        "01000000  01000000 1200 40420F00 00000000 00000000 09 {} 00",
        "07000000 00 ".repeat(9)
    );
    assert!(matches!(
        client.receive_raw(0x89, &h(&nine)),
        Err(ModelError::Overflow {
            what: "item modifications",
            ..
        })
    ));
    // Four upgrades.
    let four = format!(
        "01000000  01000000 1200 40420F00 00000000 00000000 00 04 {}",
        "01000000 00000000 0000000000000000 00 ".repeat(4)
    );
    assert!(matches!(
        client.receive_raw(0x89, &h(&four)),
        Err(ModelError::Overflow {
            what: "item upgrades",
            ..
        })
    ));
    // A defect (state set) or a modification (state clear) that the
    // client's table of that kind lacks: both are looked up.
    let set = "01000000  01000000 1200 40420F00 00000000 00000000 01 FFFFFF7F 01 00";
    assert_eq!(
        client.receive_raw(0x89, &h(set)),
        Err(ModelError::UnknownModification(0x7FFF_FFFF))
    );
    let clear = "01000000  01000000 1200 40420F00 00000000 00000000 01 FFFFFF7F 00 00  000000";
    assert_eq!(
        client.receive_raw(0x89, &h(clear)),
        Err(ModelError::UnknownModification(0x7FFF_FFFF))
    );
    // 1050 is a modification, not a defect; 3000 is a defect.
    let wrong = "01000000  01000000 1200 40420F00 00000000 00000000 01 1A040000 01 00  000000";
    assert_eq!(
        client.receive_raw(0x89, &h(wrong)),
        Err(ModelError::UnknownModification(1050))
    );
    let known =
        "01000000  01000000 1200 40420F00 00000000 00000000 02 1A040000 00 B80B0000 01 00  000000";
    assert!(client.receive_raw(0x89, &h(known)).is_ok());
    // An empty entry (dict id 0) in the inventory.
    assert_eq!(
        client.receive_raw(0x89, &h("01000000 01000000 0000")),
        Err(ModelError::UnknownItem(0))
    );
    // A count past the items sent.
    assert_eq!(
        client.receive_raw(0x89, &h("02000000 02000000 3200 32000000 00 00")),
        Err(ModelError::Short(ServerId::Inventory))
    );
    // The items without the three lists that end the message (0x26EF40).
    assert_eq!(
        client.receive_raw(0x89, &h("01000000 02000000 3200 32000000 00 00")),
        Err(ModelError::Short(ServerId::Inventory))
    );
    // 25 rows for a buffer of 24.
    let rows = format!("00000000 19 {} 00 00", "01000000 3200 ".repeat(25));
    assert!(matches!(
        client.receive_raw(0x89, &h(&rows)),
        Err(ModelError::Bad(ServerId::Inventory, _))
    ));
}

#[test]
fn the_search_step_must_not_be_zero() {
    let mut client = Client::signed_in();
    let queued = |step: &str| {
        format!(
            "01 07000000 02000000 09000000 1E000000 3C000000 {step} {}",
            "0000 ".repeat(20)
        )
    };
    assert_eq!(
        client.receive_raw(0x85, &h(&queued("0000"))),
        Err(ModelError::ZeroSearchStep)
    );
    assert!(client.receive_raw(0x85, &h(&queued("0A00"))).is_ok());
    // The 0.21d layout (no step, no properties) is too short.
    assert_eq!(
        client.receive_raw(0x85, &h("01 07000000 02000000 09000000 1E000000 3C000000")),
        Err(ModelError::Short(ServerId::ClientState))
    );
    let state = ClientState::InQueue {
        order_id: 1,
        place: 0,
        queue_total: 2,
        current_wait_seconds: 3,
        average_wait_seconds: 4,
        search_step: NonZeroU16::new(5).unwrap(),
        search_properties: [6; 20],
    };
    assert_eq!(
        client.receive(&client_state(&state, Some("st_text"))),
        Ok(Decoded::ClientState {
            state,
            status_text: Some("st_text".into())
        })
    );
    // A status text of 128 bytes overruns its buffer.
    let mut long = vec![0, 128];
    long.extend(std::iter::repeat_n(b'x', 128));
    assert!(matches!(
        client.receive_raw(0x85, &long),
        Err(ModelError::Overflow {
            what: "status text",
            ..
        })
    ));
}

#[test]
fn packed_payloads_must_unpack_to_the_counted_size() {
    let mut client = Client::signed_in();
    let list = crate::v026g::prices::price_list();
    assert_eq!(
        client.receive(&prices(list).unwrap()),
        Ok(Decoded::Prices(list.to_vec()))
    );
    assert_eq!(
        client.receive(&prices(&[]).unwrap()),
        Ok(Decoded::Prices(Vec::new()))
    );
    // Two records counted, one packed.
    assert_eq!(
        client.receive_raw(0x8A, &h("0200 0C000000 01 0007 8CB90000 0300 05 03 00")),
        Err(ModelError::BadPacked(ServerId::Prices))
    );
    // The 0.21d layout: records with no packed size.
    assert!(
        client
            .receive_raw(0x8A, &h("0100 8CB90000 0300 05 03"))
            .is_err()
    );
    assert_eq!(
        client.receive(&shop(&[]).unwrap()),
        Ok(Decoded::Shop(Vec::new()))
    );
    let record = ShopRecord {
        id: 7,
        ui_key: 9,
        kind: 0,
        value: 100,
        items: vec![ShopRecordItem {
            dict_id: 3,
            amount: 1,
            a: 2,
            b: 3,
            kind: 1,
        }],
        money_type: 1,
    };
    assert_eq!(
        client.receive(&shop(std::slice::from_ref(&record)).unwrap()),
        Ok(Decoded::Shop(vec![record]))
    );
}

#[test]
fn the_inventory_follows_purchases_and_sales() {
    let mut client = Client::signed_in();
    client
        .receive(&inventory(&[pistol(1), ammo(2, 50)]).unwrap())
        .unwrap();
    // 100 more rounds onto the stack, and a new pistol.
    let bought = [
        Bought {
            dict_id: AMMO,
            item_id: 2,
            value: 100,
        },
        Bought {
            dict_id: PISTOL,
            item_id: 3,
            value: 0,
        },
    ];
    client.receive(&permitted_bought(&bought).unwrap()).unwrap();
    assert_eq!(client.inventory(), [pistol(1), ammo(2, 150), pistol(3)]);
    // So an account that lists the new pistol is fine without a new 0x89.
    let own = own_account(&account(vec![pistol(3)])).unwrap();
    assert!(client.receive(&own).is_ok());

    client.receive(&permitted_sold(2, 100)).unwrap();
    client.receive(&permitted_sold(1, 1)).unwrap();
    assert_eq!(client.inventory(), [ammo(2, 50), pistol(3)]);
    client.receive(&permitted_sold(2, 50)).unwrap();
    assert_eq!(client.inventory(), [pistol(3)]);
    // Selling what the client does not have.
    assert_eq!(
        client.receive(&permitted_sold(2, 1)),
        Err(ModelError::NotInInventory(2))
    );
    // A purchase of a dict id the client lacks.
    assert_eq!(
        client.receive_raw(0x83, &h("47 00 01 0F27 09000000 01000000")),
        Err(ModelError::UnknownItem(9999))
    );
}

#[test]
fn repairs_and_drops_change_the_inventory() {
    let mut client = Client::signed_in();
    let mut worn = pistol(1);
    worn.condition_or_stack = 400_000;
    // The repair kit: any stack serves the model.
    client
        .receive(&inventory(&[worn, ammo(4, 2)]).unwrap())
        .unwrap();
    let repaired = permitted_repaired(
        1,
        Repaired::WithKit {
            kit_item_id: 4,
            used: 1,
        },
    )
    .unwrap();
    assert_eq!(
        client.receive(&repaired),
        Ok(Decoded::Permitted(Permitted::Repaired {
            item_id: 1,
            repaired: Repaired::WithKit {
                kit_item_id: 4,
                used: 1
            }
        }))
    );
    assert_eq!(client.inventory(), [pistol(1), ammo(4, 1)]);
    // A repair of an item the client lacks is read no further.
    assert_eq!(
        client.receive(&permitted_repaired(9, Repaired::AtOnce).unwrap()),
        Err(ModelError::NotInInventory(9))
    );
    assert!(
        client
            .receive(&permitted_repaired(1, Repaired::Timed { repair_time: 60 }).unwrap())
            .is_ok()
    );
    assert_eq!(client.inventory()[0].repair_time, 60);

    // A drop: the new item is added and the card's count set.
    client.receive(&drop(&pistol(7), 4, 0, 2).unwrap()).unwrap();
    assert_eq!(client.inventory().len(), 3);
    assert_eq!(client.inventory()[1].condition_or_stack, 0);
    // One of a usable item used.
    client.receive(&permitted_item_used(4)).unwrap();
    assert_eq!(client.inventory().len(), 2);
    // A drop with dict id 0: the UI dereferences its dictionary entry.
    assert_eq!(
        client.receive_raw(0x98, &h("01000000 0000 00000000 00000000 00")),
        Err(ModelError::UnknownItem(0))
    );
}

#[test]
fn every_core_push_reads_back() {
    let mut client = Client::signed_in();
    let mut round_trip = |message: ServerMessage, expected: Decoded| {
        assert_eq!(client.receive(&message), Ok(expected), "{:?}", message.id());
    };
    round_trip(
        connect_to_match("10.0.0.1", 1236, 5, 1, 99),
        Decoded::ConnectToMatch {
            host: "10.0.0.1".into(),
            port: 1236,
            match_id: 5,
            team: 1,
            match_session: 99,
        },
    );
    round_trip(
        permitted(0x2F).unwrap(),
        Decoded::Permitted(Permitted::NoEffect {
            op: 0x2F,
            event: None,
        }),
    );
    round_trip(
        permitted_quest_faction(),
        Decoded::Permitted(Permitted::QuestFaction),
    );
    round_trip(
        denied(0x46, "st_cannot_relocate_item"),
        Decoded::Denied {
            op: 0x46,
            text: "st_cannot_relocate_item".into(),
        },
    );
    round_trip(
        client_state(&ClientState::Lobby, None),
        Decoded::ClientState {
            state: ClientState::Lobby,
            status_text: None,
        },
    );
    let playing = ClientState::InMatch {
        order_id: 1,
        match_id: 2,
        team: 0,
    };
    round_trip(
        client_state(&playing, None),
        Decoded::ClientState {
            state: playing,
            status_text: None,
        },
    );
    round_trip(
        money(5, 6),
        Decoded::Money {
            generic: 5,
            premium: 6,
        },
    );
    round_trip(
        level_and_skills(3, 30, 20, 40, &[(101, 2)]).unwrap(),
        Decoded::Skills {
            level: 3,
            exp_total: 30,
            exp_level_start: 20,
            exp_next_level: 40,
            skills: vec![(101, 2)],
        },
    );
    let service = ServicePrices {
        reroll: 1,
        reroll_premium: 2,
        profile: 3,
        change_name: 4,
        premium_exchange_rate: 5,
        clan_creation: 6,
    };
    round_trip(service_prices(&service), Decoded::ServicePrices(service));
    round_trip(
        reputations(&[(1, 100), (4, 35250)], 4).unwrap(),
        Decoded::Reputations {
            reputations: vec![(1, 100), (4, 35250)],
            quest_active_faction: 4,
        },
    );
    round_trip(
        base_info(9, "Alice"),
        Decoded::BaseInfo {
            ranked_players: 9,
            nickname: "Alice".into(),
        },
    );
    round_trip(
        player_elo(3, 1500, 1490),
        Decoded::PlayerElo {
            place: 3,
            rating: 1500,
            rating_before: 1490,
        },
    );
    round_trip(
        elo_list(
            0,
            &[Ranked {
                name: "Bob".into(),
                rating: 1500,
                rating_before: 1520,
            }],
        )
        .unwrap(),
        Decoded::EloList(vec![("Bob".into(), 1, 1500, -20)]),
    );
    let quest = QuestRecord {
        instance_id: 1,
        quest_id: 10101,
        games: 0,
        minutes_left: 1440,
        active: true,
    };
    round_trip(quests(&[quest]), Decoded::Quests(vec![quest]));
    round_trip(
        services(Some(24)),
        Decoded::Services {
            premium_hours: Some(24),
        },
    );
    round_trip(
        services(None),
        Decoded::Services {
            premium_hours: None,
        },
    );
    round_trip(unlocked_factions(0x0F), Decoded::UnlockedFactions(0x0F));
    round_trip(
        tutorial(false, 0, 0),
        Decoded::Tutorial {
            show: false,
            a: 0,
            b: 0,
        },
    );
    round_trip(
        online_stats(1, 2, 1440),
        Decoded::OnlineStats {
            players_online: 1,
            games_online: 2,
            minutes_to_maintenance: 1440,
        },
    );
    round_trip(
        permitted_autobuy(true),
        Decoded::Permitted(Permitted::Autobuy(true)),
    );
    round_trip(
        permitted_active_kit(2),
        Decoded::Permitted(Permitted::ActiveKit(2)),
    );
    round_trip(
        permitted_nickname_check("Al", Some(NameRefusal::Pattern)),
        Decoded::Permitted(Permitted::NicknameCheck {
            nickname: "Al".into(),
            refusal: Some(NameRefusal::Pattern),
        }),
    );
    round_trip(
        permitted_offer_bought(),
        Decoded::Permitted(Permitted::OfferBought),
    );
    round_trip(
        permitted_shop_event(5).unwrap(),
        Decoded::Permitted(Permitted::NoEffect {
            op: 0x47,
            event: Some(5),
        }),
    );
    round_trip(
        permitted_upgrade_attached(1, 2, 0, 9, false),
        Decoded::Permitted(Permitted::UpgradeAttached {
            host_item_id: 1,
            upgrade_item_id: 2,
            slot: 0,
            value: 9,
            detach_first: false,
        }),
    );
}

/// The answers to upgrades, rerolls and crafting (`83 46 01..09`), as the
/// client's handler 0x275520 takes them.
#[test]
fn item_actions_change_the_inventory() {
    let mut client = Client::signed_in();
    // Two pistols, the first with modifications 2000 and 2010, and a
    // second item to attach (the model takes any item as an upgrade).
    let mut first = pistol(1);
    first.modifications = vec![
        Modification {
            id: 2000,
            state: false,
        },
        Modification {
            id: 2010,
            state: false,
        },
    ];
    client
        .receive(&inventory(&[first, pistol(2), pistol(3)]).unwrap())
        .unwrap();

    // The reroll window needs its item.
    assert_eq!(
        client.receive(&permitted_reroll_window(1, 200, 1, Some((2000, 60)))),
        Ok(Decoded::Permitted(Permitted::RerollWindow {
            item_id: 1,
            a: 200,
            b: 1,
            more: Some((2000, 60)),
        }))
    );
    assert_eq!(
        client.receive(&permitted_reroll_window(9, 200, 1, None)),
        Err(ModelError::NotInInventory(9))
    );
    // The three choices are looked up; the item's timer is set.
    assert_eq!(
        client.receive(&permitted_reroll_result(1, [2020, 2030, 7], 60)),
        Err(ModelError::UnknownModification(7))
    );
    assert_eq!(
        client.receive(&permitted_reroll_result(1, [2020, 2030, 3000], 60)),
        Err(ModelError::UnknownModification(3000))
    );
    client
        .receive(&permitted_reroll_result(1, [2020, 2030, 2040], 21_600))
        .unwrap();
    assert_eq!(client.inventory()[0].reroll_time, 21_600);
    // The chosen one replaces the old one where it was.
    client
        .receive(&permitted_modifier_chosen(1, 2000, 2030, [220, 2, 21_600]))
        .unwrap();
    let ids = |client: &Client| -> Vec<u32> {
        client.inventory()[0]
            .modifications
            .iter()
            .map(|modification| modification.id)
            .collect()
    };
    assert_eq!(ids(&client), [2030, 2010]);
    assert_eq!(
        client.receive(&permitted_modifier_chosen(9, 2000, 2030, [0; 3])),
        Err(ModelError::NotInInventory(9))
    );

    // An upgrade goes on with its dict id; one already in the slot comes
    // off. One upgrade may be on several items (patch 30): `detach_first`
    // takes it off the host itself (another slot of it), and `83 46 02`
    // off the host it names; the other items keep it.
    client
        .receive(&permitted_upgrade_attached(1, 3, 3, 0, false))
        .unwrap();
    assert_eq!(
        client.inventory()[0].upgrades,
        [Upgrade {
            id: 3,
            a: PISTOL.into(),
            value: 0,
            param: 3
        }]
    );
    client
        .receive(&permitted_upgrade_attached(2, 3, 0, 5, false))
        .unwrap();
    assert_eq!(client.inventory()[0].upgrades.len(), 1);
    assert_eq!(client.inventory()[1].upgrades[0].param, 0);
    assert_eq!(client.inventory()[1].upgrades[0].value, 5);
    client
        .receive(&permitted_upgrade_attached(2, 3, 1, 6, true))
        .unwrap();
    assert_eq!(client.inventory()[0].upgrades[0].param, 3);
    assert_eq!(client.inventory()[1].upgrades.len(), 1);
    assert_eq!(client.inventory()[1].upgrades[0].param, 1);
    client.receive(&permitted_upgrade_detached(3, 2)).unwrap();
    assert!(client.inventory()[1].upgrades.is_empty());
    assert_eq!(client.inventory()[0].upgrades.len(), 1);
    client.receive(&permitted_upgrade_detached(3, 1)).unwrap();
    assert!(client.inventory()[0].upgrades.is_empty());

    // Crafting: the donor goes, and a modification comes or goes.
    let crafted = |crafted| permitted_crafted(1, crafted).unwrap();
    assert_eq!(
        client.receive(&crafted(Crafted::Added {
            consumed_item_id: 2,
            modification: 7,
        })),
        Err(ModelError::UnknownModification(7))
    );
    assert_eq!(
        client.receive(&crafted(Crafted::Nothing {
            consumed_item_id: 9
        })),
        Err(ModelError::NotInInventory(9))
    );
    client
        .receive(&crafted(Crafted::Added {
            consumed_item_id: 2,
            modification: 2040,
        }))
        .unwrap();
    assert_eq!(ids(&client), [2030, 2010, 2040]);
    assert_eq!(client.inventory().len(), 2);
    client
        .receive(&crafted(Crafted::Removed {
            mode: 2,
            consumed_item_id: 3,
            modification: 2010,
        }))
        .unwrap();
    assert_eq!(ids(&client), [2030, 2040]);
    assert_eq!(client.inventory().len(), 1);
    // A refused craft: the item and nothing else.
    assert_eq!(
        client.receive(&crafted(Crafted::Failed)),
        Ok(Decoded::Permitted(Permitted::Crafted {
            item_id: 1,
            crafted: Crafted::Failed
        }))
    );
    assert_eq!(
        client.receive_raw(0x83, &h("46 09 FE 01000000 03000000")),
        Err(ModelError::Trailing {
            id: ServerId::Permitted,
            extra: 4
        })
    );
    assert_eq!(client.inventory().len(), 1);
}

#[test]
fn a_reply_with_a_body_the_client_does_not_read_is_caught() {
    let mut client = Client::signed_in();
    // `83 48` has no handler: a body after it is the 0.21d habit.
    assert_eq!(
        client.receive_raw(0x83, &h("48 00")),
        Err(ModelError::Trailing {
            id: ServerId::Permitted,
            extra: 1
        })
    );
    // The 0.21d sold answer ended with a profile list.
    client.receive(&inventory(&[ammo(2, 50)]).unwrap()).unwrap();
    assert_eq!(
        client.receive_raw(0x83, &h("47 01 02000000 01000000 00000000")),
        Err(ModelError::Trailing {
            id: ServerId::Permitted,
            extra: 4
        })
    );
    // A clan name check has no sixth reason.
    assert!(client.receive_raw(0x83, &h("49 08 'W' 00 05")).is_err());
    assert!(client.receive_raw(0x83, &h("49 03 'W' 00 05")).is_ok());
}

#[test]
fn chat_friends_and_squads_read_back() {
    let mut client = Client::signed_in();
    assert_eq!(
        client.receive(&chat_text(sender::ACCOUNT, 7, "Bob", "SR", 1, "hi")),
        Ok(Decoded::ChatText {
            sender_type: 5,
            sender_id: 7,
            sender_name: "Bob".into(),
            sender_clan_tag: Some("SR".into()),
            channel: 1,
            body: "hi".into(),
        })
    );
    assert_eq!(
        client.receive(&match_stats_notice(5)),
        Ok(Decoded::ChatText {
            sender_type: 8,
            sender_id: 0,
            sender_name: String::new(),
            sender_clan_tag: None,
            channel: 2,
            body: "#mstats [5]".into(),
        })
    );
    // The 0.21d layout (no tag for a player) is misread.
    assert!(
        client
            .receive_raw(0xA2, &h("05 07000000 'Bob' 01 'hi'"))
            .is_err()
    );
    // A tag of 16 bytes overruns its buffer.
    assert!(matches!(
        client.receive_raw(0xA2, &h("05 07000000 'Bob' 'ABCDEFGHIJKLMNOP' 01 'hi'")),
        Err(ModelError::Overflow { .. })
    ));

    let friends = vec![
        Friend {
            account_id: 2,
            name: "Bob".into(),
            clan_tag: String::new(),
            status: 0,
        },
        Friend {
            account_id: 3,
            name: "Eve".into(),
            clan_tag: "X".into(),
            status: 2,
        },
    ];
    // Statuses before the list: the client holds no friends.
    assert!(
        client
            .receive(&friend_statuses(&[(2, 1)]).unwrap())
            .is_err()
    );
    assert_eq!(
        client.receive(&friend_list(&friends).unwrap()),
        Ok(Decoded::Friendship(FriendshipPush::Friends(friends)))
    );
    assert_eq!(
        client.receive(&friend_statuses(&[(3, 0), (2, 1)]).unwrap()),
        Ok(Decoded::Friendship(FriendshipPush::Statuses(vec![
            (3, 0),
            (2, 1)
        ])))
    );
    // One status for two friends is dropped; an unknown id stops it.
    assert!(
        client
            .receive(&friend_statuses(&[(2, 1)]).unwrap())
            .is_err()
    );
    assert!(
        client
            .receive(&friend_statuses(&[(2, 1), (9, 1)]).unwrap())
            .is_err()
    );
    // The 0.21d `u8 result` answer is read as a list and runs off the body.
    assert!(client.receive_raw(0xA3, &h("00 34")).is_err());
    assert_eq!(
        client.receive(&found_players(&[(4, "Dan".into(), "T".into())]).unwrap()),
        Ok(Decoded::Friendship(FriendshipPush::Found {
            action: 6,
            players: vec![(4, "Dan".into(), "T".into())]
        }))
    );
    // An ignored player's line is dropped after its sender id.
    assert_eq!(
        client.receive(&ignore_list(&[(7, "Bob".into())]).unwrap()),
        Ok(Decoded::Friendship(FriendshipPush::Ignored(vec![(
            7,
            "Bob".into()
        )])))
    );
    assert_eq!(
        client.receive(&chat_text(sender::ACCOUNT, 7, "Bob", "SR", 1, "hi")),
        Ok(Decoded::ChatTextIgnored { sender_id: 7 })
    );

    // Important messages wait for their answer; a repeat is dropped.
    let invite = important_message(
        sender::ACCOUNT,
        7,
        "Bob",
        "",
        important::SQUAD_INVITE,
        "invite",
        11,
    );
    assert!(matches!(
        client.receive(&invite),
        Ok(Decoded::Important {
            duplicate: false,
            message_id: 11,
            ..
        })
    ));
    assert!(matches!(
        client.receive(&invite),
        Ok(Decoded::Important {
            duplicate: true,
            ..
        })
    ));
    assert_eq!(client.pending_important(), [11]);
    assert!(client.answer_important(11));
    // Answering an id it does not hold is the client's fatal assert.
    assert!(!client.answer_important(11));

    let members = vec![SquadMember {
        account_id: 1,
        kit_id: 1,
        kit_revision: 2,
        name: "Alice".into(),
        online: true,
        ready: false,
    }];
    assert_eq!(
        client.receive(&squad_info(9, &members, 0, true, 4, SquadPicks { own: Some(LobbyPick::Room), commander: Some(LobbyPick::Basement) }).unwrap()),
        Ok(Decoded::SquadInfo {
            squad_id: 9,
            members,
            commander_index: 0,
            i_am_commander: true,
            revision: 4,
            picks: [1, 2],
        })
    );
    assert!(
        client
            .receive(&squad_info(0, &[], 0, false, 0, SquadPicks::default()).unwrap())
            .is_ok()
    );
    // Four members overwrite the squad's own fields.
    let member = "01000000 01000000 01000000 'A' 01 00";
    let four = format!("09000000 04 00 {member} {member} {member} {member} 00 00000000 00 00");
    assert!(matches!(
        client.receive_raw(0xA1, &h(&four)),
        Err(ModelError::Overflow {
            what: "squad members",
            ..
        })
    ));
}

#[test]
fn the_small_pushes_read_back() {
    let mut client = Client::signed_in();
    assert_eq!(
        client.receive(&clan_team_limits(2, 5)),
        Ok(Decoded::ClanTeamLimits([0, 2, 5, 0]))
    );
    assert_eq!(
        client.receive(&video_link(true, "http://x")),
        Ok(Decoded::VideoLink {
            show: true,
            url: "http://x".into()
        })
    );
    assert_eq!(
        client.receive(&admin_text("a", "b")),
        Ok(Decoded::AdminText("a".into(), "b".into()))
    );
    assert_eq!(
        client.receive(&admin_text_clear()),
        Ok(Decoded::AdminTextClear)
    );
    assert_eq!(
        client.receive(&premium_store(3600, [1, 2, 3, 4], 0.5)),
        Ok(Decoded::PremiumStore {
            seconds: 3600,
            values: [1, 2, 3, 4],
            factor: 0.5
        })
    );
    assert_eq!(
        client.receive(&premium_discount(0.1)),
        Ok(Decoded::PremiumDiscount(0.1))
    );
    assert_eq!(client.receive(&signed_out()), Ok(Decoded::SignedOut));
    let tables = craft_tables(&[0.5; 12], &[7; 16], &[9; 16]);
    assert_eq!(
        client.receive(&tables),
        Ok(Decoded::CraftTables {
            chances: [0.5; 12],
            prices_general: [7; 16],
            prices_premium: [9; 16]
        })
    );
    // A video link of 246 units overruns the stack buffer.
    let mut long = vec![1, 246, 0];
    long.extend(std::iter::repeat_n([b'x', 0], 246).flatten());
    assert!(matches!(
        client.receive_raw(0x9E, &long),
        Err(ModelError::Overflow { .. })
    ));
}

#[test]
fn clan_info_reads_back() {
    let mut client = Client::signed_in();
    let rank = ClanRank {
        name: "Chief".into(),
        permissions: 3,
        is_default: true,
    };
    let infos = [
        ClanInfo::None,
        ClanInfo::Registered {
            clan_id: 5,
            name: "Wolves".into(),
            tag: "WLF".into(),
            ranks: vec![rank.clone()],
            members: vec![ClanMember {
                account_id: 1,
                name: "Alice".into(),
                rank: 0,
                level: 3,
                online: true,
            }],
            unknown: 7,
            flag: true,
            level: 2,
            progress: [1, 2, 3, 4],
        },
        ClanInfo::Registering {
            clan_id: 5,
            name: "Wolves".into(),
            tag: "WLF".into(),
            ranks: Vec::new(),
            members: vec![ClanApplicant {
                account_id: 1,
                name: "Alice".into(),
                rank: 0,
                status: 1,
            }],
            unknown: 0,
            flag: true,
            paid: false,
            money: 9,
            reg_state: 2,
        },
        ClanInfo::DailyMessage {
            clan_id: 5,
            text: "\u{43F}\u{440}\u{438}\u{432}\u{456}\u{442}".into(),
        },
        ClanInfo::Members {
            clan_id: 5,
            members: Vec::new(),
            unknown: 1,
            flag: false,
        },
        ClanInfo::Ranks {
            clan_id: 5,
            ranks: vec![rank],
        },
        ClanInfo::ClanTeam {
            clan_id: 5,
            members: vec![ClanTeamMember {
                account_id: 1,
                name: "Alice".into(),
                role: 1,
                ready: false,
            }],
            seat: Some(ClanTeamSeat {
                in_clan_team: true,
                i_am_commander: false,
                commander_ready: true,
                clan_team_id: 3,
            }),
        },
        ClanInfo::Startup {
            clan_id: 5,
            name: "Wolves".into(),
            tag: "WLF".into(),
        },
        ClanInfo::Disbanded { show: true },
    ];
    for info in infos {
        assert_eq!(
            client.receive(&clan_info(&info).unwrap()),
            Ok(Decoded::ClanInfo(info))
        );
    }
    // Eleven ranks.
    let eleven = format!("03 05000000 0B {}", "'r' 00 00 ".repeat(11));
    assert!(matches!(
        client.receive_raw(0x9A, &h(&eleven)),
        Err(ModelError::Overflow {
            what: "clan ranks",
            ..
        })
    ));
}

#[test]
fn results_read_back() {
    let mut client = Client::signed_in();
    let stats = MatchStats {
        match_id: 1,
        unknown: 0,
        winner_team: 0,
        team_scores: [5, 2],
        team_params: [1, 2],
        game_mode: 4,
        best_player: TitleHolder {
            name: "Alice".into(),
            clan_tag: "SR".into(),
        },
        best_player_score: 100,
        best_shooter: TitleHolder::default(),
        shooter_accuracy_percent: 0,
        shooter_shots: 0,
        best_support: TitleHolder::default(),
        support_value: 0,
        best_explorer: TitleHolder::default(),
        explorer_artefacts: 0,
        explorer_items: 0,
        rows: vec![ResultRow {
            name: "Alice".into(),
            clan_tag: String::new(),
            player_id: 1,
            score: 100,
            kills: 5,
            deaths: 1,
            team: 0,
            level: 2,
        }],
    };
    let mut full = FullResult::empty(1);
    full.components[1] = ScoreComponent {
        name_key: "st_score".into(),
        show_amount: false,
        columns: [(0.0, 0, 0, 0), (2.5, 10, -3, 4), (0.0, 0, 0, 0)],
    };
    full.premium.columns[2].money = 9;
    full.dropped = Some(ItemDescr::plain(5, PISTOL, 900_000));
    full.drop_chances = [0.1, 0.2, 0.3, 0.4];
    assert_eq!(
        client.receive(&match_stats(&stats, &full).unwrap()),
        Ok(Decoded::MatchStats {
            stats: stats.clone(),
            own: full.clone()
        })
    );
    let player = AccountBody {
        team: 1,
        name: "Bob".into(),
        clan_tag: String::new(),
        protection: 0,
        kits: vec![Kit::empty(4)],
        items: Vec::new(),
    };
    assert_eq!(
        client.receive(&player_details(&FullResult::empty(2), &player).unwrap()),
        Ok(Decoded::PlayerDetails {
            discarded: FullResult::empty(2),
            player
        })
    );
    assert_eq!(
        client.receive(&daily_bonus(&[1, 0], &ammo(3, 10), false, true).unwrap()),
        Ok(Decoded::DailyBonus {
            progress: vec![1, 0],
            reward: ammo(3, 10),
            completed: false,
            show_window: true
        })
    );

    // 21 rows, a team past 1, a five-entry list: each corrupts the client.
    let header = "01000000 00000000 00 00 00 00000000 00000000 00
                  00 00 0000  00 00 00 0000  00 00 0000  00 00 0000 0000";
    let row = |team: &str| format!("'A' 00 01000000 0000 0000 0000 {team} 0000 ");
    let rows21 = format!("{header} 15 {}", row("00").repeat(21));
    assert!(matches!(
        client.receive_raw(0x97, &h(&rows21)),
        Err(ModelError::Overflow {
            what: "result rows",
            ..
        })
    ));
    let team2 = format!("{header} 01 {}", row("02"));
    assert!(matches!(
        client.receive_raw(0x97, &h(&team2)),
        Err(ModelError::Bad(ServerId::MatchStats, _))
    ));
    let empty_component = "00 00 00000000 00000000 00000000";
    let full_with_list = |list: &str| {
        format!(
            "{header} 00
             01000000 {empty_component} {empty_component} {empty_component}
             {} 0000 0000 00000000 0000 00 0000 0000 0000
             {list}
             00000000 0000 00 00000000 00000000 00000000 00000000",
            "00".repeat(99)
        )
    };
    assert!(client.receive_raw(0x97, &h(&full_with_list("00"))).is_ok());
    let five = format!("05 {}", "01000000 ".repeat(5));
    assert!(matches!(
        client.receive_raw(0x97, &h(&full_with_list(&five))),
        Err(ModelError::Overflow { .. })
    ));
    // Eight bonus days overwrite the reward.
    assert!(matches!(
        client.receive_raw(
            0x99,
            &h("08 0000000000000000 01 01000000 3200 01000000 00 00 00 00")
        ),
        Err(ModelError::Overflow {
            what: "daily bonus days",
            ..
        })
    ));
    // The message that crashed the real client on 2026-10-12, byte for
    // byte: the item's id before its stack flag. The client reads the flag
    // first (0x2773CE), so it took the 50 cartridges (dict 0xD7) for an
    // item that is no stack, read two more u32 than the message has and
    // took its counts of modifications and upgrades from the bytes after
    // the message's end (5 and 3 that time; 0x229AC4 then looked up a
    // modification nobody sent).
    assert!(
        client
            .receive_raw(0x99, &h("01 00  00000000 01 D700 32000000 00 00  00 00"))
            .is_err()
    );
    // The same bonus in the client's order is fine.
    assert!(
        client
            .receive_raw(0x99, &h("01 00  01 00000000 D700 32000000 00 00  00 00"))
            .is_ok()
    );
    // A game that connects again holds what it held and reads the same
    // way: 0x80 first, and the same message is as fatal as it was.
    client.reconnect();
    assert!(matches!(
        client.receive_raw(0x99, &h("01 00  01 00000000 D700 32000000 00 00  00 00")),
        Err(ModelError::NotSignedIn(_))
    ));
    assert!(client.receive_raw(0x80, &h("00 00")).is_ok());
    assert!(
        client
            .receive_raw(0x99, &h("01 00  00000000 01 D700 32000000 00 00  00 00"))
            .is_err()
    );
    assert!(
        client
            .receive_raw(0x99, &h("01 00  01 00000000 D700 32000000 00 00  00 00"))
            .is_ok()
    );
    // A message that ends before the client stops reading is never fine.
    assert!(
        client
            .receive_raw(0x99, &h("01 00  00 09000000 1200 40420F00 00000000"))
            .is_err()
    );
}

#[test]
fn a_whole_sequence_stops_at_the_first_bad_message() {
    let items = [pistol(1)];
    let good = [
        signed_in(0, false),
        base_info(1, "Alice"),
        inventory(&items).unwrap(),
        own_account(&account(items.to_vec())).unwrap(),
    ];
    let mut client = Client::new();
    assert_eq!(client.receive_all(&good).unwrap().len(), 4);

    // The account before the inventory.
    let bad = [
        signed_in(0, false),
        own_account(&account(items.to_vec())).unwrap(),
        inventory(&items).unwrap(),
    ];
    let mut client = Client::new();
    assert_eq!(client.receive_all(&bad), Err(ModelError::NotInInventory(1)));
    assert_eq!(client.received(), 1);
}
