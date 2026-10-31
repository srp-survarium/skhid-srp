//! Every server→client writer against bytes laid out by hand.

use std::num::NonZeroU16;

use super::super::*;
use super::{assert_bytes, bytes, h};
use crate::v026g::lobby::client_model::{Client, Decoded};
use crate::v026g::prices::PriceItem;
use crate::v026g::skills::MODIFIERS;

/// The TT-33 (no stack) and its ammo (a stack): 0.26g dict ids 18 and 50.
const PISTOL: u16 = 18;
const AMMO: u16 = 50;

fn pistol(item_id: u32) -> ItemDescr {
    ItemDescr::plain(item_id, PISTOL, FULL_CONDITION)
}

fn ammo(item_id: u32, count: u32) -> ItemDescr {
    ItemDescr::plain(item_id, AMMO, count)
}

fn body_with(kits: Vec<Kit>, items: Vec<ItemDescr>) -> AccountBody {
    AccountBody {
        team: 0,
        name: "Alice".into(),
        clan_tag: String::new(),
        protection: NO_PROTECTION,
        kits,
        items,
    }
}

/// One kit: the pistol in slot 7, 50 rounds in slot 8, and two modifiers:
/// index 5 (low qword) and index 64 (high qword).
fn sample_kit() -> Kit {
    let mut kit = Kit::empty(1);
    kit.slots[7] = Some(KitSlot {
        item_index: 0,
        count: 1,
    });
    kit.slots[8] = Some(KitSlot {
        item_index: 1,
        count: 50,
    });
    kit.modifiers[5] = 1.0;
    kit.modifiers[64] = 2.0;
    kit
}

const SAMPLE_BODY: &str = "
    00                          team
    'Alice'                     name
    00                          clan abbreviation
    0A000000                    protection
    01                          kits
    02                          items
    00                          kit: state
    01000000                    kit id
    00                          kit name
    80010000                    slot mask: bits 7 and 8
    0000 0100                   slot 7: item index 0, count 1
    0100 3200                   slot 8: item index 1, count 50
    0100000000000000            modifier mask, bits 64..127: bit 64
    2000000000000000            modifier mask, bits 0..63: bit 5
    0000803F                    modifier 5 = 1.0
    00000040                    modifier 64 = 2.0
    01000000 1200 40420F00 00000000 00000000 00 00      the pistol
    02000000 3200 32000000 00 00                        the ammo: 50 rounds
";

#[test]
fn no_forbidden_id_can_be_made() {
    for id in 0..=255_u8 {
        let known = ServerId::from_u8(id);
        let allowed =
            (FIRST_SERVER_ID..=LAST_SERVER_ID).contains(&id) && !FORBIDDEN_SERVER_IDS.contains(&id);
        assert_eq!(known.is_some(), allowed, "{id:#04X}");
        if let Some(known) = known {
            assert_eq!(known as u8, id);
        }
    }
    // 0x51..=0x7F, 0x86 and 0xA7 have no transport entry; 0x81 and 0xA0 no
    // handler.
    for id in (0x51..=0x7F).chain([0x81, 0x86, 0xA0, 0xA7]) {
        assert_eq!(ServerId::from_u8(id), None, "{id:#04X}");
    }
    assert_eq!(ServerId::ALL.len(), 0xAA - 0x80 + 1 - 4);
    let mut sorted = ServerId::ALL;
    sorted.sort();
    assert_eq!(sorted, ServerId::ALL);
}

/// transport.md §4.1: the class of each id.
#[test]
fn delivery_classes_are_the_clients_table() {
    let unique = [0x80, 0x82, 0x9A, 0xA1, 0xA2, 0xA4, 0xAA];
    for id in ServerId::ALL {
        let expected = if unique.contains(&(id as u8)) {
            Delivery::Unique
        } else if id as u8 == 0x9F {
            Delivery::Plain
        } else {
            Delivery::Ordered
        };
        assert_eq!(id.delivery(), expected, "{id:?}");
    }
    // The ordered ids, as the note lists them.
    let ordered: Vec<u8> = ServerId::ALL
        .into_iter()
        .filter(|id| id.delivery() == Delivery::Ordered)
        .map(|id| id as u8)
        .collect();
    let listed: Vec<u8> = [0x83, 0x84, 0x85]
        .into_iter()
        .chain(0x87..=0x99)
        .chain(0x9B..=0x9E)
        .chain([0xA3, 0xA5, 0xA6, 0xA8, 0xA9])
        .collect();
    assert_eq!(ordered, listed);
}

#[test]
fn the_x_byte_follows_the_id() {
    // Unique: `id, unique id, body`.
    assert_eq!(signed_in(0, false).wire(0x07), h("80 07 00 00"));
    // Ordered: `id, order id, body`.
    assert_eq!(money(1, 2).wire(0x03), h("8B 03 01000000 02000000"));
    // 0x9F is plain: no x byte.
    assert_eq!(
        online_stats(1, 2, 3).wire(0x55),
        h("9F 01000000 02000000 03000000")
    );
}

#[test]
fn signed_in_is_three_bytes() {
    assert_bytes(&signed_in(0, false), "80 00 00");
    assert_bytes(&signed_in(1, true), "80 01 01");
}

#[test]
fn connect_to_match_carries_the_match_session() {
    assert_bytes(
        &connect_to_match("127.0.0.1", 1236, 0x123, 1, 5000),
        "82 '127.0.0.1' D404 23010000 01 88130000",
    );
    // The host goes into a 64-byte stack buffer.
    let long = connect_to_match(&"h".repeat(100), 1, 2, 0, 0);
    assert_eq!(long.body()[0], 63);
    assert_eq!(long.body().len(), 1 + 63 + 2 + 4 + 1 + 4);
}

#[test]
fn permitted_and_denied() {
    assert_bytes(&permitted(0x2F).unwrap(), "83 2F");
    assert_bytes(&permitted(0x48).unwrap(), "83 48");
    assert!(permitted(0x2D).is_err());
    assert!(permitted(0x51).is_err());
    assert_bytes(&permitted_quest_faction(), "83 4B");
    assert_bytes(&permitted_clan_play(), "83 30");
    // `u8 op, u8 skipped, string text`.
    assert_bytes(
        &denied(0x47, "st_cannot_buy_item"),
        "84 47 00 'st_cannot_buy_item'",
    );
    assert_eq!(denied(0x2F, &"x".repeat(300)).body().len(), 2 + 1 + 255);
}

#[test]
fn client_states() {
    assert_bytes(&client_state(&ClientState::Lobby, None), "85 00");
    let mut search_properties = [0; 20];
    search_properties[0] = 1;
    search_properties[19] = 0x1234;
    let queued = ClientState::InQueue {
        order_id: 7,
        place: 2,
        queue_total: 9,
        current_wait_seconds: 30,
        average_wait_seconds: 60,
        search_step: NonZeroU16::new(10).unwrap(),
        search_properties,
    };
    assert_bytes(
        &client_state(&queued, None),
        "85 01
         07000000 02000000 09000000 1E000000 3C000000
         0A00
         0100 0000 0000 0000 0000 0000 0000 0000 0000 0000
         0000 0000 0000 0000 0000 0000 0000 0000 0000 3412",
    );
    let playing = ClientState::InMatch {
        order_id: 7,
        match_id: 0x123,
        team: 1,
    };
    assert_bytes(&client_state(&playing, None), "85 02 07000000 23010000 01");
    // The optional text, read only if bytes remain; a 128-byte buffer.
    assert_bytes(
        &client_state(&ClientState::Lobby, Some("st_wait")),
        "85 00 'st_wait'",
    );
    let long = client_state(&ClientState::Lobby, Some(&"t".repeat(200)));
    assert_eq!(long.body().len(), 1 + 1 + 127);
}

#[test]
fn the_inventory_writes_gear_and_stacks_differently() {
    let items = [pistol(1), ammo(2, 50)];
    assert_bytes(
        &inventory(&items).unwrap(),
        "89 02000000
         01000000 1200 40420F00 00000000 00000000 00 00
         02000000 3200 32000000 00 00
         00 00 00                 nothing spent after a match",
    );
    assert_bytes(&inventory(&[]).unwrap(), "89 00000000 00 00 00");
}

/// The tail the client reads at 0x26EF40: three lists of `u32 silver,
/// u16 dict id`.
#[test]
fn the_inventory_ends_with_the_money_spent_after_a_match() {
    let spent = SpentMoney {
        autobuy_ammo: vec![(120, AMMO), (7, 51)],
        autobuy_consumables: vec![],
        repair: vec![(0x0102_0304, PISTOL)],
    };
    let message = inventory_with_spent(&[ammo(2, 50)], &spent).unwrap();
    assert_bytes(
        &message,
        "89 01000000
         02000000 3200 32000000 00 00
         02  78000000 3200  07000000 3300
         00
         01  04030201 1200",
    );
    let mut client = Client::signed_in();
    assert_eq!(
        client.receive(&message),
        Ok(Decoded::Inventory {
            items: vec![ammo(2, 50)],
            spent: spent.clone(),
        })
    );

    // One row more than a buffer of the client holds.
    for (list, most) in [(0, 24), (1, 18), (2, 27)] {
        let mut spent = SpentMoney::default();
        let rows = vec![(1, AMMO); most + 1];
        match list {
            0 => spent.autobuy_ammo = rows,
            1 => spent.autobuy_consumables = rows,
            _ => spent.repair = rows,
        }
        assert!(matches!(
            inventory_with_spent(&[], &spent),
            Err(WireError::TooMany { count, most: limit, .. }) if count == most + 1 && limit == most
        ));
    }
    // The model refuses what the old writer sent: the items and no lists.
    let short = ServerMessage::from_parts(ServerId::Inventory, vec![0, 0, 0, 0]);
    assert!(Client::signed_in().receive(&short).is_err());
}

#[test]
fn an_item_with_times_modifications_and_upgrades() {
    let item = ItemDescr {
        item_id: 0x0A0B0C0D,
        dict_id: PISTOL,
        condition_or_stack: 755_000,
        repair_time: 60,
        reroll_time: 90,
        modifications: vec![
            Modification {
                id: 3000,
                state: true,
            },
            Modification {
                id: 2000,
                state: false,
            },
        ],
        upgrades: vec![Upgrade {
            id: 0x11,
            a: 0x22,
            value: 0x0102030405060708,
            param: 3,
        }],
    };
    assert_bytes(
        &inventory(std::slice::from_ref(&item)).unwrap(),
        "89 01000000
         0D0C0B0A 1200
         38850B00                 condition 75.5 x 10000
         3C000000                 repair time
         5A000000                 reroll time
         02  B80B0000 01  D0070000 00
         01  11000000 22000000 0807060504030201 03
         00 00 00",
    );
}

#[test]
fn the_own_account() {
    let account = OwnAccount {
        body: body_with(vec![sample_kit()], vec![pistol(1), ammo(2, 50)]),
        revisions: vec![7],
        active_kit: 1,
        autobuy: true,
        auto_repair: false,
        level: 3,
        exp_total: 1000,
        exp_level_start: 500,
        exp_next_level: 1500,
    };
    let expected = format!(
        "87 {SAMPLE_BODY}
         07000000                    revision of kit 0
         01000000                    active kit id
         01                          autobuy
         03                          level
         E8030000 F4010000 DC050000  experience: total, level start, next level"
    );
    assert_bytes(&own_account(&account).unwrap(), &expected);
}

#[test]
fn an_account_with_no_kit_modifiers_has_an_empty_mask() {
    let mut kit = Kit::empty(5);
    kit.name = "Sniper".into();
    kit.state = KIT_PREMIUM;
    let account = OwnAccount {
        body: AccountBody {
            team: 1,
            name: "Bob".into(),
            clan_tag: "SRP".into(),
            protection: 3,
            kits: vec![kit],
            items: Vec::new(),
        },
        revisions: vec![1],
        active_kit: 5,
        autobuy: false,
        auto_repair: false,
        level: 0,
        exp_total: 0,
        exp_level_start: 0,
        exp_next_level: 100,
    };
    assert_bytes(
        &own_account(&account).unwrap(),
        "87 01 'Bob' 'SRP' 03000000 01 00
         02 05000000 'Sniper' 00000000
         0000000000000000 0000000000000000
         01000000 05000000 00 00 00000000 00000000 64000000",
    );
}

#[test]
fn a_squad_mates_account_ends_with_its_revision() {
    let body = body_with(vec![sample_kit()], vec![pistol(1), ammo(2, 50)]);
    let expected = format!("88 {SAMPLE_BODY} 09000000");
    assert_bytes(&squad_member_account(&body, 9).unwrap(), &expected);
}

#[test]
fn the_account_bodys_limits() {
    let ok = body_with(vec![sample_kit()], vec![pistol(1), ammo(2, 50)]);
    assert!(squad_member_account(&ok, 0).is_ok());

    // A 4th kit would overwrite the item table.
    let four = body_with((1..=4).map(Kit::empty).collect(), Vec::new());
    assert_eq!(
        squad_member_account(&four, 0),
        Err(WireError::TooMany {
            what: "kits",
            count: 4,
            most: 3
        })
    );
    // 69 items fit, 70 do not.
    let full = body_with(Vec::new(), (1..=69).map(pistol).collect());
    assert!(squad_member_account(&full, 0).is_ok());
    let over = body_with(Vec::new(), (1..=70).map(pistol).collect());
    assert!(matches!(
        squad_member_account(&over, 0),
        Err(WireError::TooMany { most: 69, .. })
    ));
    // A slot's index is into the item table.
    let mut kit = Kit::empty(1);
    kit.slots[0] = Some(KitSlot {
        item_index: 1,
        count: 1,
    });
    let stale = body_with(vec![kit], vec![pistol(1)]);
    assert_eq!(
        squad_member_account(&stale, 0),
        Err(WireError::BadItemIndex { index: 1, items: 1 })
    );
    // A dict id the client lacks.
    let unknown = body_with(Vec::new(), vec![ItemDescr::plain(1, 9999, 1)]);
    assert_eq!(
        squad_member_account(&unknown, 0),
        Err(WireError::UnknownItem(9999))
    );
    assert_eq!(
        inventory(&[ItemDescr::plain(1, 0, 1)]),
        Err(WireError::UnknownItem(0))
    );
    // 8 modifications and 3 upgrades are the most.
    let mut item = pistol(1);
    item.modifications = vec![
        Modification {
            id: 2000,
            state: false
        };
        8
    ];
    item.upgrades = vec![
        Upgrade {
            id: 1,
            a: 0,
            value: 0,
            param: 0
        };
        3
    ];
    assert!(inventory(std::slice::from_ref(&item)).is_ok());
    item.modifications.push(Modification {
        id: 2000,
        state: false,
    });
    assert!(matches!(
        inventory(std::slice::from_ref(&item)),
        Err(WireError::TooMany { most: 8, .. })
    ));
    item.modifications.pop();
    item.upgrades.push(item.upgrades[0]);
    assert!(matches!(
        inventory(std::slice::from_ref(&item)),
        Err(WireError::TooMany { most: 3, .. })
    ));
    // One revision for each kit.
    let account = OwnAccount {
        body: ok,
        revisions: Vec::new(),
        active_kit: 1,
        autobuy: false,
        auto_repair: false,
        level: 1,
        exp_total: 0,
        exp_level_start: 0,
        exp_next_level: 1,
    };
    assert!(own_account(&account).is_err());
}

#[test]
fn names_are_cut_to_the_clients_buffers() {
    let mut kit = Kit::empty(1);
    kit.name = "k".repeat(80);
    let body = AccountBody {
        team: 0,
        name: "n".repeat(80),
        clan_tag: "c".repeat(20),
        protection: 0,
        kits: vec![kit],
        items: Vec::new(),
    };
    let message = squad_member_account(&body, 0).unwrap();
    let bytes = message.body();
    assert_eq!(bytes[1], 63);
    // The clan abbreviation's buffer ends where the kit count is kept.
    assert_eq!(bytes[1 + 1 + 63], 14);
    let kit_name = 1 + 1 + 63 + 1 + 14 + 4 + 2 + 1 + 4;
    assert_eq!(bytes[kit_name], 63);
    // Never inside a character: 40 two-byte letters are cut to 31.
    assert_eq!(cut(&"я".repeat(40), 63).len(), 62);
}

#[test]
fn prices_are_packed_as_stored_lzma2() {
    let record = PriceItem {
        cost: 47500,
        dict_id: 3,
        level: 5,
        faction_id: 3,
    };
    // `u16 n`, `u32 packed size`, then the stream: one stored chunk with a
    // dictionary reset (01, size - 1 big-endian), the record, the end.
    assert_bytes(
        &prices(&[record]).unwrap(),
        "8A 0100 0C000000
         01 0007   8CB90000 0300 05 03   00",
    );
    // With no records nothing follows the count.
    assert_bytes(&prices(&[]).unwrap(), "8A 0000");
    // The real list: n x 8 bytes in one chunk.
    let list = crate::v026g::prices::price_list();
    let message = prices(list).unwrap();
    let body = message.body();
    assert_eq!(u16::from_le_bytes([body[0], body[1]]) as usize, list.len());
    let packed = u32::from_le_bytes([body[2], body[3], body[4], body[5]]) as usize;
    assert_eq!(packed, body.len() - 6);
    assert_eq!(packed, list.len() * 8 + 3 + 1);
}

#[test]
fn money_skills_and_the_small_pushes() {
    assert_bytes(&money(2000, 1000), "8B D0070000 E8030000");
    assert_bytes(
        &level_and_skills(3, 1000, 500, 1500, &[(101, 2), (209, 1)]).unwrap(),
        "8C 03 E8030000 F4010000 DC050000 02  6500 02  D100 01",
    );
    assert_bytes(
        &level_and_skills(1, 0, 0, 100, &[]).unwrap(),
        "8C 01 00000000 00000000 64000000 00",
    );
    let service = ServicePrices {
        reroll: 100,
        reroll_premium: 5,
        profile: 200,
        change_name: 50,
        premium_exchange_rate: 100,
        clan_creation: 1000,
    };
    assert_bytes(
        &service_prices(&service),
        "8D 64000000 05000000 C8000000 32000000 64000000 E8030000",
    );
    // `u8 n`, n x {u32 faction, u16, u16 points}, `u8 quest faction`.
    assert_bytes(
        &reputations(&[(1, 100), (2, 0)], 1).unwrap(),
        "8E 02  01000000 0000 6400  02000000 0000 0000  01",
    );
    assert_bytes(&reputations(&[], 0).unwrap(), "8E 00 00");
    assert_bytes(&base_info(42, "Alice"), "8F 2A000000 'Alice'");
    assert_bytes(&player_elo(7, 1500, 1480), "90 07000000 DC05 C805");
    // A row: name, place, rating, change (-10).
    assert_bytes(
        &elo_list(
            10,
            &[Ranked {
                name: "Bob".into(),
                rating: 1500,
                rating_before: 1510,
            }],
        )
        .unwrap(),
        "91 01 'Bob' 0B000000 DC05 F6FF",
    );
    // The flush: the ranking's top as a first page, then its first row
    // again up to 255 rows.
    let top = [
        Ranked {
            name: "Bob".into(),
            rating: 102,
            rating_before: 100,
        },
        Ranked {
            name: "Al".into(),
            rating: 98,
            rating_before: 100,
        },
    ];
    let flush = ranking_flush(&top).unwrap().unwrap();
    let mut expected = h("91 FF 'Bob' 01000000 6600 0200  'Al' 02000000 6200 FEFF");
    for _ in 0..253 {
        expected.extend(h("'Bob' 01000000 6600 0200"));
    }
    assert_eq!(bytes(&flush), expected);
    assert!(ranking_flush(&[]).is_none());
    assert_bytes(&unlocked_factions(0x0F), "96 0F");
    assert_bytes(&tutorial(false, 0, 0), "9D 00 00000000 00000000");
    assert_bytes(&tutorial(true, 0x22, 1), "9D 01 22000000 01000000");
    assert_bytes(&online_stats(12, 3, 1440), "9F 0C000000 03000000 A0050000");
}

/// The answer to a replay poll: one ranking row at the replay start
/// (2 200 000 000 is 0x83215600) whose name is the
/// asker's status, the match in 8 lower-case hex digits and the size.
#[test]
fn a_replay_poll_is_answered_with_one_row_at_the_replay_start() {
    assert_eq!(REPLAY_START, 2_200_000_000);
    assert_eq!(REPLAY_ASK, 0xFFFF_FF01);
    assert_bytes(
        &replay_answer("ready", 0x1F07_C8FB, 1_166_938).unwrap(),
        "91 01 'R1;ready;1f07c8fb;1166938' 00562183 0000 0000",
    );
    let mut client = Client::signed_in();
    let message = replay_answer("none", 0, 0).unwrap();
    assert_eq!(
        client.receive(&message).unwrap(),
        Decoded::EloList(vec![("R1;none;00000000;0".into(), 2_200_000_000, 0, 0)])
    );
    // The longest word with the largest size: within the 63 bytes.
    let longest = format!("R1;not_recorded;ffffffff;{}", u64::MAX);
    assert_eq!(longest.len(), 45);
    let Decoded::EloList(rows) = client
        .receive(&replay_answer("not_recorded", u32::MAX, u64::MAX).unwrap())
        .unwrap()
    else {
        panic!("not a ranking list");
    };
    assert_eq!(rows[0].0, longest);
}

/// "Player vs Bots": the status (`B1` and a row a map, from 2 050 000 000)
/// and a choice's answer (`B2`, at the start it was asked with), as the
/// client reads them; every name within the 63 bytes, every start above
/// the reserved start.
#[test]
fn player_vs_bots_rows_are_ranking_rows_at_their_starts() {
    assert_eq!(PVB_STATUS_START, 2_050_000_000);
    assert_eq!(PVB_CHOOSE_BASE, 2_051_000_000);
    const {
        assert!(RESERVED_START < PVB_STATUS_START);
        assert!(PVB_STATUS_START + PVB_MAP_ROWS as u32 + 1 < PVB_CHOOSE_BASE);
    }
    assert_eq!(pvb_choice_start(8, 61), 2_051_008_061);
    assert_eq!(pvb_choice(2_051_008_061), Some((8, 61)));
    assert_eq!(pvb_choice(2_051_010_138), Some((10, 138)));
    assert_eq!(pvb_choice(PVB_CHOOSE_BASE - 1), None);
    assert_eq!(pvb_choice(PVB_CHOOSE_LAST + 1), None);
    assert_eq!(pvb_choice(PVB_CHOOSE_BASE + 256_000), None);
    assert_eq!(pvb_choice(PVB_CHOOSE_BASE + 1_256), None);

    let widest = PvbNumbers {
        gear: u8::MAX,
        max_gear: u8::MAX,
        running: u32::MAX,
        max_matches: u32::MAX,
    };
    let status = PvbStatus {
        code: PvbCode::GearTooHigh,
        numbers: PvbNumbers {
            gear: 6,
            max_gear: 5,
            running: 1,
            max_matches: 5,
        },
        sizes: vec![6, 8, 10],
        default_size: 8,
        default_map: 61,
    };
    let maps = [
        PvbMap {
            map_id: 61,
            level: 1,
            mode: 2,
            free: 4,
        },
        PvbMap {
            map_id: 138,
            level: 8,
            mode: 4,
            free: 0,
        },
    ];
    let mut client = Client::signed_in();
    let read = |client: &mut Client, message| match client.receive(&message).unwrap() {
        Decoded::EloList(rows) => rows,
        other => panic!("not a ranking list: {other:?}"),
    };
    assert_eq!(
        read(&mut client, pvb_status(&status, &maps, 32).unwrap()),
        vec![
            ("B1;1;6;5;6.8.10;8;61;1;5".into(), 2_050_000_000, 0, 0),
            ("61;1;2;4".into(), 2_050_000_001, 0, 0),
            ("138;8;4;0".into(), 2_050_000_002, 0, 0),
        ]
    );
    // As many rows as asked for, the header always.
    assert_eq!(
        read(&mut client, pvb_status(&status, &maps, 2).unwrap()).len(),
        2
    );
    assert_eq!(
        read(&mut client, pvb_status(&status, &maps, 0).unwrap()).len(),
        1
    );
    assert!(pvb_status(&status, &[maps[0]; PVB_MAP_ROWS + 1], 32).is_err());
    let wide = PvbStatus {
        numbers: widest,
        code: PvbCode::UnknownChoice,
        default_size: u8::MAX,
        default_map: u8::MAX,
        ..status.clone()
    };
    let header = &read(&mut client, pvb_status(&wide, &[], 1).unwrap())[0].0;
    assert!(header.len() <= limits::NAME, "{header}");

    assert_eq!(
        read(&mut client, pvb_choose_ok(8, 61).unwrap()),
        vec![("B2;ok;8;61".into(), 2_051_008_061, 0, 0)]
    );
    assert_eq!(
        read(
            &mut client,
            pvb_choose_no(2_051_006_117, PvbCode::AtLimit, status.numbers).unwrap()
        ),
        vec![("B2;no;7;6;5;1;5".into(), 2_051_006_117, 0, 0)]
    );
    let no = &read(
        &mut client,
        pvb_choose_no(PVB_CHOOSE_LAST, PvbCode::UnknownChoice, widest).unwrap(),
    )[0]
    .0;
    assert!(no.len() <= limits::NAME, "{no}");
    // The codes are the contract's.
    for (code, number) in [
        (PvbCode::Allowed, 0),
        (PvbCode::GearTooHigh, 1),
        (PvbCode::InSquad, 2),
        (PvbCode::Searching, 3),
        (PvbCode::InMatch, 4),
        (PvbCode::Outdated, 5),
        (PvbCode::Off, 6),
        (PvbCode::AtLimit, 7),
        (PvbCode::Maintenance, 8),
        (PvbCode::ClanTeam, 9),
        (PvbCode::UnknownChoice, 10),
    ] {
        assert_eq!(code as u8, number);
    }
}

#[test]
fn quests_are_twenty_byte_records() {
    let record = QuestRecord {
        instance_id: 5,
        quest_id: 10101,
        games: 1,
        minutes_left: 1440,
        active: true,
    };
    let proposal = QuestRecord {
        instance_id: 6,
        quest_id: 10102,
        games: 0,
        minutes_left: 60,
        active: false,
    };
    assert_bytes(
        &quests(&[record, proposal]),
        "92 02000000
         05000000 75270000 01000000 A0050000 01 000000
         06000000 76270000 00000000 3C000000 00 000000",
    );
    assert_bytes(&quests(&[]), "92 00000000");
}

#[test]
fn services_list_only_the_premium_account() {
    assert_bytes(&services(None), "94 00");
    // `u8 type 0, u8, u32 hours, u32`.
    assert_bytes(&services(Some(48)), "94 01  00 00 30000000 00000000");
}

#[test]
fn the_shop() {
    // No records: the stream of no bytes is its end marker.
    assert_bytes(&shop(&[]).unwrap(), "95 00000000 01000000 00");

    let record = ShopRecord {
        id: 7,
        ui_key: 0x1234,
        kind: 1,
        value: 500,
        items: vec![ShopRecordItem {
            dict_id: 3,
            amount: 2,
            a: 10,
            b: 20,
            kind: 1,
        }],
        money_type: 1,
    };
    let message = shop(std::slice::from_ref(&record)).unwrap();
    let body = message.body();
    // `u32 n`, `u32 packed`: 0xD0 bytes in one stored chunk (3 + 1 more).
    assert_eq!(&body[..8], h("01000000 D4000000"));
    assert_eq!(&body[8..11], h("01 00CF"));
    assert_eq!(body.len(), 8 + 3 + 0xD0 + 1);
    assert_eq!(body[body.len() - 1], 0);
    let unpacked = &body[11..11 + 0xD0];
    let mut expected = vec![0_u8; 0xD0];
    expected[0x00] = 7;
    expected[0x04..0x06].copy_from_slice(&[0x34, 0x12]);
    expected[0x0C] = 1;
    expected[0x10..0x12].copy_from_slice(&[0xF4, 0x01]);
    expected[0x14] = 1;
    expected[0x18] = 3;
    expected[0x18 + 0x0C] = 2;
    expected[0x18 + 0x14] = 10;
    expected[0x18 + 0x18] = 20;
    expected[0x18 + 0x20] = 1;
    expected[0xCC] = 1;
    assert_eq!(unpacked, expected);

    let mut six = record;
    six.items = vec![ShopRecordItem::default(); 6];
    assert!(shop(&[six]).is_err());
}

#[test]
fn inventory_action_answers() {
    assert_bytes(
        &permitted_upgrade_attached(1, 2, 3, 0x0102030405060708, true),
        "83 46 01 01000000 02000000 03 0807060504030201 01",
    );
    assert_bytes(
        &permitted_upgrade_detached(9, 4),
        "83 46 02 09000000 04000000",
    );
    assert_bytes(&permitted_item_used(9), "83 46 04 09000000");
    assert_bytes(
        &permitted_reroll_window(1, 2, 3, None),
        "83 46 06 01000000 02000000 03000000 00",
    );
    assert_bytes(
        &permitted_reroll_window(1, 2, 3, Some((4, 5))),
        "83 46 06 01000000 02000000 03000000 01 04000000 05000000",
    );
    assert_bytes(
        &permitted_reroll_result(1, [2, 3, 4], 5),
        "83 46 07 01000000 02000000 03000000 04000000 05000000",
    );
    assert_bytes(
        &permitted_modifier_chosen(1, 2, 3, [4, 5, 6]),
        "83 46 08 01000000 02000000 03000000 04000000 05000000 06000000",
    );
    // `u8 mode, u32 item`; the consumed item unless mode 0xFE; the
    // modification unless mode 0.
    assert_bytes(
        &permitted_crafted(
            1,
            Crafted::Nothing {
                consumed_item_id: 2,
            },
        )
        .unwrap(),
        "83 46 09 00 01000000 02000000",
    );
    assert_bytes(
        &permitted_crafted(
            1,
            Crafted::Added {
                consumed_item_id: 2,
                modification: 3,
            },
        )
        .unwrap(),
        "83 46 09 01 01000000 02000000 03000000",
    );
    assert_bytes(
        &permitted_crafted(
            1,
            Crafted::Removed {
                mode: 2,
                consumed_item_id: 2,
                modification: 3,
            },
        )
        .unwrap(),
        "83 46 09 02 01000000 02000000 03000000",
    );
    // Mode 0xFE: the item and nothing more.
    assert_bytes(
        &permitted_crafted(1, Crafted::Failed).unwrap(),
        "83 46 09 FE 01000000",
    );
    assert!(
        permitted_crafted(
            1,
            Crafted::Removed {
                mode: 0xFE,
                consumed_item_id: 2,
                modification: 3
            }
        )
        .is_err()
    );
}

#[test]
fn shop_answers() {
    // `u8 n`, n x {u16 dict, u32 item id, u32 value}.
    let bought = [
        Bought {
            dict_id: AMMO,
            item_id: 2,
            value: 100,
        },
        Bought {
            dict_id: PISTOL,
            item_id: 9,
            value: FULL_CONDITION,
        },
    ];
    assert_bytes(
        &permitted_bought(&bought).unwrap(),
        "83 47 00 02  3200 02000000 64000000  1200 09000000 40420F00",
    );
    assert_eq!(
        permitted_bought(&[Bought {
            dict_id: 9999,
            item_id: 1,
            value: 1
        }]),
        Err(WireError::UnknownItem(9999))
    );
    // No profile list after the amount any more.
    assert_bytes(&permitted_sold(2, 50), "83 47 01 02000000 32000000");
    assert_bytes(&permitted_offer_bought(), "83 47 02");
    // `u32 item, u32 v`; only for v = 0 a kit id, and for a kit its count.
    assert_bytes(
        &permitted_repaired(1, Repaired::Timed { repair_time: 600 }).unwrap(),
        "83 47 03 01000000 58020000",
    );
    assert_bytes(
        &permitted_repaired(1, Repaired::AtOnce).unwrap(),
        "83 47 03 01000000 00000000 00000000",
    );
    assert_bytes(
        &permitted_repaired(
            1,
            Repaired::WithKit {
                kit_item_id: 4,
                used: 1,
            },
        )
        .unwrap(),
        "83 47 03 01000000 00000000 04000000 01",
    );
    assert!(permitted_repaired(1, Repaired::Timed { repair_time: 0 }).is_err());
    assert!(
        permitted_repaired(
            1,
            Repaired::WithKit {
                kit_item_id: 0,
                used: 1
            }
        )
        .is_err()
    );
    assert_bytes(&permitted_shop_event(4).unwrap(), "83 47 04");
    assert_bytes(&permitted_shop_event(5).unwrap(), "83 47 05");
    assert!(permitted_shop_event(0).is_err());
}

#[test]
fn kit_and_account_answers() {
    assert_bytes(&permitted_autobuy(true), "83 49 01 01");
    assert_bytes(&permitted_autobuy(false), "83 49 01 00");
    assert_bytes(&permitted_active_kit(2), "83 49 02 02000000");
    // `string, bool ok`, and a reason only when refused.
    assert_bytes(
        &permitted_nickname_check("Alice", None),
        "83 49 03 'Alice' 01",
    );
    assert_bytes(
        &permitted_nickname_check("Al", Some(NameRefusal::Short)),
        "83 49 03 'Al' 00 00",
    );
    assert_bytes(
        &permitted_nickname_check("A b", Some(NameRefusal::Pattern)),
        "83 49 03 'A b' 00 05",
    );
    assert_bytes(&permitted_nickname_changed(0), "83 49 04 00000000");
    assert_bytes(&permitted_nickname_changed(77), "83 49 04 4D000000");
}

#[test]
fn chat_lines() {
    // A player (type 5) has a clan tag after the name.
    assert_bytes(
        &chat_text(sender::ACCOUNT, 7, "Bob", "SR", channel::GENERAL, "hi"),
        "A2 05 07000000 'Bob' 'SR' 01 'hi'",
    );
    assert_bytes(
        &chat_text(sender::ACCOUNT, 7, "Bob", "", channel::SQUAD, "go"),
        "A2 05 07000000 'Bob' 00 08 'go'",
    );
    // Any other sender type has none.
    assert_bytes(
        &chat_text(sender::STATS, 0, "", "ignored", channel::CLAN, "0\x0EBob"),
        "A2 08 00000000 00 03 05 30 0E \"Bob\"",
    );
    assert_bytes(
        &match_stats_notice(291),
        "A2 08 00000000 00 02 '#mstats [291]'",
    );
    // Name 63, tag 15, body 255.
    let long = chat_text(
        sender::ACCOUNT,
        1,
        &"n".repeat(70),
        &"t".repeat(20),
        1,
        &"b".repeat(300),
    );
    assert_eq!(
        long.body().len(),
        1 + 4 + (1 + 63) + (1 + 15) + 1 + (1 + 255)
    );
}

#[test]
fn friend_lists() {
    let friends = [
        Friend {
            account_id: 2,
            name: "Bob".into(),
            clan_tag: "SR".into(),
            status: 1,
        },
        Friend {
            account_id: 3,
            name: "Eve".into(),
            clan_tag: String::new(),
            status: 2,
        },
    ];
    // `07, u16 n`, n x {u32 id, name, tag, u8 status}.
    assert_bytes(
        &friend_list(&friends).unwrap(),
        "A3 07 0200  02000000 'Bob' 'SR' 01  03000000 'Eve' 00 02",
    );
    assert_bytes(&friend_list(&[]).unwrap(), "A3 07 0000");
    // `08, u16 n`, n x {u32 id, name}; a `<muted>` player's id marked.
    assert_bytes(
        &ignore_list(&[(9, "Troll".into(), ChatMuteMode::Hidden)]).unwrap(),
        "A3 08 0100 09000000 'Troll'",
    );
    assert_bytes(
        &ignore_list(&[
            (9, "Troll".into(), ChatMuteMode::Hidden),
            (5, "Loud".into(), ChatMuteMode::Placeholder),
        ])
        .unwrap(),
        "A3 08 0200 09000000 'Troll' 05000080 'Loud'",
    );
    // `09, u16 n`, n x {u32 id, u8 status}.
    assert_bytes(
        &friend_statuses(&[(2, 0), (3, 2)]).unwrap(),
        "A3 09 0200 02000000 00 03000000 02",
    );
    // `06, u16 n`, n x {u32 id, name, tag}.
    assert_bytes(
        &found_players(&[(4, "Dan".into(), "X".into())]).unwrap(),
        "A3 06 0100 04000000 'Dan' 'X'",
    );
}

#[test]
fn important_messages() {
    assert_bytes(
        &important_message(
            sender::ACCOUNT,
            7,
            "Bob",
            "SR",
            important::SQUAD_INVITE,
            "invite",
            0x1001,
        ),
        "A4 05 07000000 'Bob' 'SR' 02 'invite' 01100000",
    );
    // The tag is always there, even for a system sender.
    assert_bytes(
        &important_message(
            sender::STATS,
            0,
            "",
            "",
            important::QUEST,
            "#+q[5][10101]",
            3,
        ),
        "A4 08 00000000 00 00 01 '#+q[5][10101]' 03000000",
    );
}

#[test]
fn squad_info_layout() {
    // No squad: only the id.
    assert_bytes(&squad_info(0, &[], 0, false, 0).unwrap(), "A1 00000000");
    let members = [
        SquadMember {
            account_id: 1,
            kit_id: 1,
            kit_revision: 4,
            name: "Alice".into(),
            online: true,
            ready: true,
        },
        SquadMember {
            account_id: 2,
            kit_id: 0x8000_0002,
            kit_revision: 9,
            name: "Bob".into(),
            online: true,
            ready: false,
        },
    ];
    assert_bytes(
        &squad_info(0x55, &members, 0, true, 3).unwrap(),
        "A1 55000000
         02                      members
         00                      commander index
         01000000 01000000 04000000 'Alice' 01 01
         02000000 02000080 09000000 'Bob' 01 00
         01                      i am the commander
         03000000                revision",
    );
    // Three slots; a commander among them.
    let four = vec![members[0].clone(); 4];
    assert!(squad_info(1, &four, 0, false, 0).is_err());
    assert!(squad_info(1, &members, 2, false, 0).is_err());
}

#[test]
fn the_other_small_pushes() {
    assert_bytes(&clan_team_limits(2, 5), "9C 00 02 05 00");
    // With the players a clan needs to be registered.
    assert_bytes(&clan_limits(5, 2, 8), "9C 05 02 08 00");
    // `bool, u16 n`, n UTF-16LE units.
    assert_bytes(&video_link(true, "hi"), "9E 01 0200 6800 6900");
    assert_bytes(&video_link(false, ""), "9E 00 0000");
    let long = video_link(true, &"u".repeat(400));
    assert_eq!(long.body().len(), 1 + 2 + 245 * 2);
    assert_bytes(&admin_text("a", "bc"), "A5 'a' 'bc'");
    assert_bytes(&admin_text_clear(), "A6 00");
    assert_bytes(
        &premium_store(7200, [1, 2, 3, 4], 0.5),
        "A8 201C0000 01000000 02000000 03000000 04000000 0000003F",
    );
    assert_bytes(&premium_discount(0.25), "A9 0000803E");
    assert_bytes(&signed_out(), "AA");
}

fn rank() -> ClanRank {
    ClanRank {
        name: "Chief".into(),
        permissions: 0x1F,
        is_default: false,
    }
}

#[test]
fn clan_info_types() {
    assert_bytes(&clan_info(&ClanInfo::None).unwrap(), "9A 00 00000000");
    let registered = ClanInfo::Registered {
        clan_id: 5,
        name: "Wolves".into(),
        tag: "WLF".into(),
        ranks: vec![rank()],
        members: vec![ClanMember {
            account_id: 1,
            name: "Alice".into(),
            rank: 0,
            level: 7,
            online: true,
        }],
        unknown: 0x0102,
        flag: true,
        level: 2,
        progress: [10, 20, 30, 40],
    };
    assert_bytes(
        &clan_info(&registered).unwrap(),
        "9A 00 05000000 'Wolves' 'WLF'
         01                              registered
         01 'Chief' 1F 00                ranks
         01000000                        members
         01000000 'Alice' 00 07 01       id, name, rank, level, online
         0201 01                         u16, flag
         02 0A000000 14000000 1E000000 28000000",
    );
    let registering = ClanInfo::Registering {
        clan_id: 5,
        name: "Wolves".into(),
        tag: "WLF".into(),
        ranks: Vec::new(),
        members: vec![ClanApplicant {
            account_id: 1,
            name: "Alice".into(),
            rank: 0,
            status: 0,
        }],
        unknown: 0,
        flag: false,
        paid: true,
        money: 500,
        reg_state: 3,
    };
    assert_bytes(
        &clan_info(&registering).unwrap(),
        "9A 00 05000000 'Wolves' 'WLF'
         00                              not registered
         00                              ranks
         01000000 01000000 'Alice' 00 00 members: id, name, rank, status
         0000 00                         u16, flag
         01 F4010000 03                  paid, money, state",
    );
    assert_bytes(
        &clan_info(&ClanInfo::DailyMessage {
            clan_id: 5,
            text: "ok".into(),
        })
        .unwrap(),
        "9A 01 05000000 0200 6F00 6B00",
    );
    assert_bytes(
        &clan_info(&ClanInfo::Members {
            clan_id: 5,
            members: Vec::new(),
            unknown: 3,
            flag: false,
        })
        .unwrap(),
        "9A 02 05000000 00000000 0300 00",
    );
    assert_bytes(
        &clan_info(&ClanInfo::Ranks {
            clan_id: 5,
            ranks: vec![rank()],
        })
        .unwrap(),
        "9A 03 05000000 01 'Chief' 1F 00",
    );
    let member = ClanTeamMember {
        account_id: 1,
        name: "Alice".into(),
        role: 0,
        ready: true,
    };
    assert_bytes(
        &clan_info(&ClanInfo::ClanTeam {
            clan_id: 5,
            members: vec![member.clone()],
            seat: Some(ClanTeamSeat {
                in_clan_team: true,
                i_am_commander: true,
                commander_ready: false,
                clan_team_id: 8,
            }),
        })
        .unwrap(),
        "9A 04 05000000 01  01000000 'Alice' 00 01  01 01 01 00 08000000",
    );
    assert_bytes(
        &clan_info(&ClanInfo::ClanTeam {
            clan_id: 5,
            members: Vec::new(),
            seat: None,
        })
        .unwrap(),
        "9A 04 05000000 00 00",
    );
    assert_bytes(
        &clan_info(&ClanInfo::Startup {
            clan_id: 5,
            name: "Wolves".into(),
            tag: "WLF".into(),
        })
        .unwrap(),
        "9A 05 05000000 01 'Wolves' 'WLF'",
    );
    assert_bytes(
        &clan_info(&ClanInfo::Disbanded { show: true }).unwrap(),
        "9A 06 01",
    );
    // Clan id 0 ends the client's read: only type 0 may carry it.
    assert!(
        clan_info(&ClanInfo::Ranks {
            clan_id: 0,
            ranks: Vec::new()
        })
        .is_err()
    );
    // 10 ranks, 105 members, 100 team members.
    assert!(
        clan_info(&ClanInfo::Ranks {
            clan_id: 1,
            ranks: vec![rank(); 11]
        })
        .is_err()
    );
    assert!(
        clan_info(&ClanInfo::ClanTeam {
            clan_id: 1,
            members: vec![member; 101],
            seat: None
        })
        .is_err()
    );
}

#[test]
fn clan_answers() {
    assert_bytes(&permitted_clan_name_reserved(), "83 49 06");
    assert_bytes(
        &permitted_clan_name_check("Wolves", None).unwrap(),
        "83 49 08 'Wolves' 01",
    );
    assert_bytes(
        &permitted_clan_tag_check("W", Some(NameRefusal::Taken)).unwrap(),
        "83 49 09 'W' 00 02",
    );
    assert!(permitted_clan_name_check("x", Some(NameRefusal::Pattern)).is_err());
    assert_bytes(&permitted_clan_invite_sent("Bob"), "83 49 0B 'Bob'");
    assert_bytes(&permitted_clan_invite(None), "83 49 0F 01");
    assert_bytes(
        &permitted_clan_invite(Some((5, "Bob", "Wolves"))),
        "83 49 0F 00 05000000 'Bob' 'Wolves'",
    );
    assert_bytes(&permitted_rank_up(true), "83 49 12 01");
    assert_bytes(
        &permitted_clan_team_invite(8, "Bob", "x"),
        "83 49 19 08000000 'Bob' 'x'",
    );
}

fn sample_stats() -> MatchStats {
    MatchStats {
        match_id: 0x123,
        unknown: 0,
        winner_team: 1,
        team_scores: [3, 5],
        team_params: [100, 150],
        game_mode: 2,
        best_player: TitleHolder {
            name: "Alice".into(),
            clan_tag: "SR".into(),
        },
        best_player_score: 900,
        best_shooter: TitleHolder {
            name: "Bob".into(),
            clan_tag: String::new(),
        },
        shooter_accuracy_percent: 45,
        shooter_shots: 300,
        best_support: TitleHolder::default(),
        support_value: 4,
        best_explorer: TitleHolder {
            name: "Eve".into(),
            clan_tag: String::new(),
        },
        explorer_artefacts: 2,
        explorer_items: 6,
        rows: vec![ResultRow {
            name: "Alice".into(),
            clan_tag: "SR".into(),
            player_id: 1,
            score: 900,
            kills: 12,
            deaths: 3,
            team: 1,
            level: 5,
        }],
    }
}

const SAMPLE_STATS: &str = "
    23010000                    match id
    00000000                    unused
    01                          winner team
    03 05                       team scores
    64000000 96000000           team params
    02                          game mode
    'Alice' 'SR' 8403           best player, score
    'Bob' 00 2D 2C01            best shooter, accuracy, shots
    00 00 0400                  best support, value
    'Eve' 00 0200 0600          best explorer, artefacts, items
    01                          rows
    'Alice' 'SR' 01000000 8403 0C00 0300 01 0500
";

fn sample_full() -> FullResult {
    let mut full = FullResult::empty(1);
    full.components[0] = ScoreComponent {
        name_key: "st_kills".into(),
        show_amount: true,
        // Only the first column has an amount: the others carry nothing.
        columns: [(12.0, 180, 60, -5), (0.0, 9, 9, 9), (0.0, 0, 0, 0)],
    };
    full.daily.columns[0] = BonusColumn {
        money: 100,
        reputation: 10,
        experience: 20,
        bonus_money: 50,
        bonus_reputation: 0,
        bonus_experience: 25,
    };
    full.elo_before = 1500;
    full.elo_after = 1512;
    full.quest_money = 1000;
    full.quest_reputation = 100;
    full.faction = 1;
    full.play_seconds = [600, 0, 0];
    full.dropped = Some(ItemDescr::plain(77, PISTOL, 800_000));
    full.drop_rarity = 1;
    full.drop_chances = [0.5, 0.25, 0.0, 0.125];
    full
}

const SAMPLE_FULL: &str = "
    01000000                                player id
    'st_kills' 01                           component 0: key, show amount
        00004041 B4000000 3C00 FBFF         column 0: 12.0, money, exp, rep
        00000000                            column 1: amount 0, nothing else
        00000000                            column 2
    00 00  00000000 00000000 00000000       component 1: empty
    00 00  00000000 00000000 00000000       component 2: empty
    64000000 0A00 1400 32 00 19             daily bonus, column 0
    00000000 0000 0000 00 00 00
    00000000 0000 0000 00 00 00
    00000000 0000 0000 00 00 00  00000000 0000 0000 00 00 00  00000000 0000 0000 00 00 00
    00000000 0000 0000 00 00 00  00000000 0000 0000 00 00 00  00000000 0000 0000 00 00 00
    DC05 E805                               elo before, after
    E8030000 6400 01                        quest money, reputation, faction
    5802 0000 0000                          play seconds
    00                                      the unread list: none
    4D000000 1200 00350C00 00000000 00 00   the drop: no repair time in this form
    01                                      rarity
    0000003F 0000803E 00000000 0000003E     drop chances
";

#[test]
fn match_stats_layout() {
    let expected = format!("97 {SAMPLE_STATS} {SAMPLE_FULL}");
    assert_bytes(
        &match_stats(&sample_stats(), &sample_full()).unwrap(),
        &expected,
    );
}

#[test]
fn a_result_with_no_drop_has_an_empty_descriptor() {
    let message = match_stats(&sample_stats(), &FullResult::empty(1)).unwrap();
    let stats_len = h(SAMPLE_STATS).len();
    // id; 3 empty components of 14 bytes; 3 bonus blocks of 33; 4 + 4 + 2
    // + 1 + 6; the list count; `u32 id, u16 dict 0`; rarity; 4 floats.
    let expected = format!(
        "01000000
         {empty} {empty} {empty}
         {zeros99}
         0000 0000 00000000 0000 00 0000 0000 0000
         00
         00000000 0000
         00
         00000000 00000000 00000000 00000000",
        empty = "00 00 00000000 00000000 00000000",
        zeros99 = "00".repeat(99),
    );
    assert_eq!(&message.body()[stats_len..], h(&expected));
}

#[test]
fn result_limits() {
    let mut stats = sample_stats();
    stats.rows = vec![stats.rows[0].clone(); 21];
    assert!(match_stats(&stats, &FullResult::empty(1)).is_err());
    stats.rows.truncate(20);
    assert!(match_stats(&stats, &FullResult::empty(1)).is_ok());
    // A team past 1 corrupts the client's stack.
    stats.rows[3].team = 2;
    assert!(match_stats(&stats, &FullResult::empty(1)).is_err());
    // A drop the client's dictionary lacks.
    let mut full = FullResult::empty(1);
    full.dropped = Some(ItemDescr::plain(1, 9999, 1));
    assert_eq!(
        match_stats(&sample_stats(), &full),
        Err(WireError::UnknownItem(9999))
    );
    // A title's clan tag has a 15-byte buffer, a row's 16.
    let mut stats = sample_stats();
    stats.best_player.clan_tag = "t".repeat(30);
    stats.rows[0].clan_tag = "t".repeat(30);
    let body = match_stats(&stats, &FullResult::empty(1))
        .unwrap()
        .into_body();
    let title_tag = 4 + 4 + 3 + 8 + 1 + (1 + 5);
    assert_eq!(body[title_tag], 14);
}

#[test]
fn player_details_are_a_result_and_an_account() {
    let player = body_with(vec![sample_kit()], vec![pistol(1), ammo(2, 50)]);
    let message = player_details(&FullResult::empty(9), &player).unwrap();
    let full_len = 4 + 3 * 14 + 99 + 17 + 1 + 6 + 1 + 16;
    assert_eq!(message.id(), ServerId::PlayerDetails);
    assert_eq!(&message.body()[..4], h("09000000"));
    assert_eq!(&message.body()[full_len..], h(SAMPLE_BODY));
}

#[test]
fn a_drop_and_the_daily_bonus() {
    // The lobby form of the descriptor, the card's id and count, a rarity.
    assert_bytes(
        &drop(&pistol(77), 5, 2, 3).unwrap(),
        "98 4D000000 1200 40420F00 00000000 00000000 00 00  05000000 02000000 03",
    );
    assert_eq!(
        drop(&ItemDescr::plain(1, 0, 1), 0, 0, 0),
        Err(WireError::UnknownItem(0))
    );
    // `u8 n`, n days; `bool is_stack, u32 id, u16 dict` (0x2773CE: the
    // flag is read first), the rest of the descriptor; completed; show
    // the window.
    assert_bytes(
        &daily_bonus(&[1, 1, 0], &ammo(9, 100), false, true).unwrap(),
        "99 03 01 01 00  01 09000000 3200 64000000 00 00  00 01",
    );
    assert_bytes(
        &daily_bonus(&[], &pistol(9), true, false).unwrap(),
        "99 00  00 09000000 1200 40420F00 00000000 00000000 00 00  01 00",
    );
    // What the first live session got in the old order (id, flag, dict):
    // 17 bytes of which the client would read 25 and more.
    assert_bytes(
        &daily_bonus(&[0], &ItemDescr::plain(0, 0xD7, 50), false, false).unwrap(),
        "99 01 00  01 00000000 D700 32000000 00 00  00 00",
    );
    assert!(daily_bonus(&[0; 8], &pistol(9), false, false).is_err());
}

#[test]
fn craft_tables_are_112_bytes() {
    let mut chances = [0.0; 12];
    chances[0] = 1.0;
    chances[11] = 0.5;
    let mut general = [0; 16];
    general[0] = 400;
    general[15] = 20000;
    let mut premium = [0; 16];
    premium[0] = 2;
    premium[15] = 100;
    let message = craft_tables(&chances, &general, &premium);
    let body = message.body();
    assert_eq!(body.len(), 112);
    assert_eq!(&body[..4], h("0000803F"));
    assert_eq!(&body[44..48], h("0000003F"));
    assert_eq!(&body[48..50], h("9001"));
    assert_eq!(&body[78..80], h("204E"));
    assert_eq!(&body[80..82], h("0200"));
    assert_eq!(&body[110..112], h("6400"));
    assert_eq!(bytes(&message)[0], 0x9B);
}

#[test]
fn a_kit_may_fill_every_slot_and_modifier() {
    let mut kit = Kit::empty(1);
    for (index, slot) in kit.slots.iter_mut().enumerate() {
        *slot = Some(KitSlot {
            item_index: index as u16,
            count: 1,
        });
    }
    kit.modifiers = [1.0; MODIFIERS];
    let body = body_with(vec![kit], (1..=23).map(pistol).collect());
    let bytes = squad_member_account(&body, 0).unwrap().into_body();
    let kit_at = 1 + 6 + 1 + 4 + 2;
    // state, id, name, the mask of 23 slots, 23 slots.
    assert_eq!(&bytes[kit_at + 6..kit_at + 10], h("FFFF7F00"));
    let mask_at = kit_at + 10 + 23 * 4;
    // Bits 64..=67, then bits 0..=63.
    assert_eq!(
        &bytes[mask_at..mask_at + 16],
        h("0F00000000000000 FFFFFFFFFFFFFFFF")
    );
    let items_at = mask_at + 16 + 68 * 4;
    assert_eq!(bytes.len(), items_at + 23 * 20 + 4);
}

// ---- the two boxes of the kit window, the premium shop's records, the
// ---- offer after a match

#[test]
fn the_autobuy_and_auto_repair_boxes_are_one_number() {
    use crate::v026g::lobby::client_model::Permitted;
    // `83 49 01 <state>`: bit 0 autobuy, bit 1 auto-repair.
    assert_bytes(&permitted_auto_state(false, false), "83 49 01 00");
    assert_bytes(&permitted_auto_state(true, false), "83 49 01 01");
    assert_bytes(&permitted_auto_state(false, true), "83 49 01 02");
    assert_bytes(&permitted_auto_state(true, true), "83 49 01 03");
    let mut client = Client::signed_in();
    assert_eq!(
        client.receive(&permitted_auto_state(true, true)).unwrap(),
        Decoded::Permitted(Permitted::AutoRepair { autobuy: true })
    );
    assert_eq!(
        client.receive(&permitted_auto_state(true, false)).unwrap(),
        Decoded::Permitted(Permitted::Autobuy(true))
    );
    // The lobby ticks its boxes for 0..=3 only: `FF` would show both off.
    assert!(client.receive_raw(0x83, &[0x49, 0x01, 0xFF]).is_err());

    // The same number in the account (`87`), after the active kit's id.
    let account = |autobuy, auto_repair| OwnAccount {
        body: AccountBody {
            team: 0,
            name: "A".into(),
            clan_tag: String::new(),
            protection: 10,
            kits: Vec::new(),
            items: Vec::new(),
        },
        revisions: Vec::new(),
        active_kit: 1,
        autobuy,
        auto_repair,
        level: 0,
        exp_total: 0,
        exp_level_start: 0,
        exp_next_level: 500,
    };
    for (autobuy, auto_repair, state) in [
        (false, false, 0),
        (true, false, 1),
        (false, true, 2),
        (true, true, 3),
    ] {
        let message = own_account(&account(autobuy, auto_repair)).unwrap();
        let body = message.body();
        // team, name, tag, protection, no kits, no items, the active kit.
        assert_eq!(body[..13], [0, 1, b'A', 0, 10, 0, 0, 0, 0, 0, 1, 0, 0]);
        assert_eq!(body[13], 0);
        assert_eq!(body[14], state);
        let Decoded::Account(read) = Client::signed_in().receive(&message).unwrap() else {
            panic!("not the account");
        };
        assert_eq!((read.autobuy, read.auto_repair), (autobuy, auto_repair));
    }
}

#[test]
fn a_shop_record_is_laid_out_as_the_client_reads_it() {
    use crate::v026g::shop_offers;
    let records = shop_offers::records(&shop_offers::DEFAULT_PREMIUM_PRICES);
    let message = shop(&records).unwrap();
    let body = message.body();
    assert_eq!(message.id_byte(), 0x95);
    assert_eq!(body[..4], 14_u32.to_le_bytes());
    let unpacked = crate::v026g::lzma2::unstore(&body[8..]).unwrap();
    assert_eq!(unpacked.len(), 14 * 0xD0);
    let u32_at = |record: usize, offset: usize| {
        let at = record * 0xD0 + offset;
        u32::from_le_bytes(unpacked[at..at + 4].try_into().unwrap())
    };
    // The first record: a year of premium (bundle 6), 8000 gold, kind 0,
    // no items.
    assert_eq!(
        [
            u32_at(0, 0x00),
            u32_at(0, 0x04),
            u32_at(0, 0x0C),
            u32_at(0, 0x10),
            u32_at(0, 0x14),
            u32_at(0, 0xCC)
        ],
        [6, 6, 0, 8_000, 0, 1]
    );
    // The ninth: the AK-74N bundle (9), kind 1, 1500 gold, two items: the
    // rifle (dict 338, 1) and its ammo (dict 7, 2000), each an item of the
    // inventory (0 at +0x20 of the item).
    assert_eq!(
        [
            u32_at(8, 0x00),
            u32_at(8, 0x04),
            u32_at(8, 0x0C),
            u32_at(8, 0x10),
            u32_at(8, 0x14),
            u32_at(8, 0xCC)
        ],
        [9, 9, 1, 1_500, 2, 1]
    );
    assert_eq!(
        [
            u32_at(8, 0x18),
            u32_at(8, 0x18 + 0x0C),
            u32_at(8, 0x18 + 0x20)
        ],
        [338, 1, 0]
    );
    assert_eq!(
        [
            u32_at(8, 0x3C),
            u32_at(8, 0x3C + 0x0C),
            u32_at(8, 0x3C + 0x20)
        ],
        [7, 2_000, 0]
    );
    let mut client = Client::signed_in();
    assert_eq!(
        client.receive(&message).unwrap(),
        Decoded::Shop(records.clone())
    );

    // What the client would not take: a record of no bundle (its texts are
    // looked up by the id with no check), a kind it shows nowhere, a
    // bundle item its dictionary lacks.
    let mut no_bundle = records[0].clone();
    no_bundle.ui_key = 99;
    let mut odd_kind = records[0].clone();
    odd_kind.kind = 2;
    let mut unknown_item = records[8].clone();
    unknown_item.items[0].dict_id = 9_999;
    for bad in [no_bundle, odd_kind, unknown_item] {
        let message = shop(&[bad]).unwrap();
        assert!(Client::signed_in().receive(&message).is_err());
    }
}

#[test]
fn the_premium_offers_of_a8_and_a9() {
    // `A8`: the premium seconds, the money, experience and reputation
    // premium would add, the seconds the offer stands, the discount.
    let message = premium_post_reward(0, 150, 60, 45, 3_600, 0.25);
    assert_bytes(
        &message,
        "A8 00000000 96000000 3C000000 2D000000 100E0000 0000803E",
    );
    assert_eq!(
        Client::signed_in().receive(&message).unwrap(),
        Decoded::PremiumStore {
            seconds: 0,
            values: [150, 60, 45, 3_600],
            factor: 0.25
        }
    );
    assert_bytes(&premium_discount(0.25), "A9 0000803E");
    // The client prices an offer at `value * (1 - discount)` and divides
    // by what is left for the old price: a whole discount is refused.
    for bad in [1.0, 1.5, -0.1, f32::NAN] {
        assert!(
            Client::signed_in().receive(&premium_discount(bad)).is_err(),
            "{bad}"
        );
        assert!(
            Client::signed_in()
                .receive(&premium_post_reward(0, 1, 1, 1, 60, bad))
                .is_err(),
            "{bad}"
        );
    }
}
