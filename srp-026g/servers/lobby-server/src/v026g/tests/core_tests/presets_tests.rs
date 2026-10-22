//! Kit presets (`presets`, `docs/notes/presets.md`): the Character panel's
//! list, save and apply, and a room join that offers a preset ("Enter
//! anyway" / "Exit" or "Switch and enter"), with "Switch and enter" putting
//! it on before the join.

use session::v026g::{matchmaking, quests};
use survarium::v026g::lobby::presets::{PRESET_APPLY_BASE, PRESET_SAVE_BASE, PRESETS_FIRST};
use survarium::v026g::lobby::rooms::{
    ROOM_JOIN_BASE, ROOM_OFFER_BASE, ROOM_PRESET_BASE, ROOM_STATE, RoomSpec,
};

use super::pvb_tests::with_gear;
use super::rooms_tests::{create, rows, spec};
use super::*;

/// What a `3B` at `start` is answered with, through the client: the names
/// of the rows of each `91`, and every message.
fn asked(world: &mut World, index: usize, start: u32) -> (Vec<Vec<String>>, Vec<Decoded>) {
    // Earlier answers (`rows` hands its request to the lobby directly)
    // reach the client first.
    world.settle();
    world.take(index);
    world.ask(index, request::ranking(start, 1));
    let decoded = world.take(index);
    let lists = decoded
        .iter()
        .filter_map(|message| match message {
            Decoded::EloList(rows) => Some(rows.iter().map(|row| row.0.clone()).collect()),
            _ => None,
        })
        .collect();
    (lists, decoded)
}

fn gear(world: &World, index: usize) -> u8 {
    matchmaking::gear_level(&world.account(index), quests::now())
}

/// The ammo `with_gear` puts in the first kit's slot 9 out again: back to
/// the starter's gear.
fn starter_gear(world: &mut World, index: usize) {
    world.change(index, |account| account.kits[0].slots[9] = None);
    assert_eq!(gear(world, index), 0);
}

fn sent_account(decoded: &[Decoded]) -> bool {
    decoded
        .iter()
        .any(|message| matches!(message, Decoded::Account(_)))
}

#[test]
fn presets_are_listed_saved_and_put_on() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let (lists, _) = asked(&mut world, alice, PRESETS_FIRST);
    assert_eq!(
        lists,
        [[
            "P1;L;0;0;0",
            "1;0;-1",
            "2;0;-1",
            "3;0;-1",
            "4;0;-1",
            "5;0;-1"
        ]]
    );

    // The starter kit into preset 1, a level 5 kit into preset 2.
    let (lists, _) = asked(&mut world, alice, PRESET_SAVE_BASE + 1);
    assert_eq!(lists[0][..2], ["P1;L;1;1;0", "1;1;0"]);
    let starter = world.account(alice).kits.clone();
    with_gear(&mut world, alice, 5);
    let geared = world.account(alice).kits.clone();
    let (lists, _) = asked(&mut world, alice, PRESET_SAVE_BASE + 2);
    assert_eq!(
        lists,
        [["P1;L;1;2;0", "1;1;0", "2;1;5", "3;0;-1", "4;0;-1", "5;0;-1"]]
    );

    // Preset 1 on: the kits as saved, the client sent its account.
    let inventory = world.account(alice).inventory.clone();
    let (lists, decoded) = asked(&mut world, alice, PRESET_APPLY_BASE + 1);
    assert_eq!(lists[0][0], "P1;L;2;1;0");
    assert!(sent_account(&decoded));
    assert_eq!(gear(&world, alice), 0);
    let slots: Vec<_> = world
        .account(alice)
        .kits
        .iter()
        .map(|kit| kit.slots)
        .collect();
    assert_eq!(
        slots,
        starter.iter().map(|kit| kit.slots).collect::<Vec<_>>()
    );
    // Nothing bought, sold or made.
    assert_eq!(world.account(alice).inventory, inventory);
    // And back from the dropdown.
    asked(&mut world, alice, PRESET_APPLY_BASE + 2);
    assert_eq!(gear(&world, alice), 5);
    let slots: Vec<_> = world
        .account(alice)
        .kits
        .iter()
        .map(|kit| kit.slots)
        .collect();
    assert_eq!(
        slots,
        geared.iter().map(|kit| kit.slots).collect::<Vec<_>>()
    );

    // An empty preset changes nothing.
    let (lists, decoded) = asked(&mut world, alice, PRESET_APPLY_BASE + 4);
    assert_eq!(lists[0][0], "P1;L;3;4;0");
    assert!(!sent_account(&decoded));
    assert_eq!(gear(&world, alice), 5);
    world.assert_clean();
}

#[test]
fn a_sold_item_is_counted_as_an_empty_slot() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    asked(&mut world, alice, PRESET_SAVE_BASE + 1);
    let pistol = world.account(alice).kits[0].slots[7].unwrap().item_id;
    world.ask(alice, request::sell(pistol, 1));
    assert!(world.account(alice).item(pistol).is_none());
    let silver = world.account(alice).generic_money;
    let (lists, _) = asked(&mut world, alice, PRESET_APPLY_BASE + 1);
    // One slot empty; the preset now can not play (no weapon).
    assert_eq!(lists[0][..2], ["P1;L;2;1;1", "1;1;-1"]);
    assert_eq!(world.account(alice).kits[0].slots[7], None);
    assert_eq!(world.account(alice).generic_money, silver);
}

#[test]
fn gear_over_the_max_level_offers_the_highest_preset_that_fits() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 3,
            ..spec(8)
        },
        None,
    );
    // Bob: preset 1 his starter kit (level 0), preset 2 of level 5, and
    // he plays the level 5 kits.
    asked(&mut world, bob, PRESET_SAVE_BASE + 1);
    with_gear(&mut world, bob, 5);
    asked(&mut world, bob, PRESET_SAVE_BASE + 2);
    assert_eq!(
        rows(&mut world, bob, ROOM_OFFER_BASE + key, 1),
        [format!("S1;O;{key};2;1;0;5;3")]
    );
    // "Exit": nothing asked; the join itself is refused as before.
    assert_eq!(
        rows(&mut world, bob, ROOM_JOIN_BASE + key, 1),
        ["S1;N;12;0;0"]
    );
    // "Switch and enter": preset 1 on, then the join.
    let (lists, decoded) = asked(&mut world, bob, ROOM_PRESET_BASE + key);
    assert!(sent_account(&decoded));
    assert_eq!(lists[0][0], "P1;L;2;1;0");
    let state = &lists[1];
    assert!(state[0].starts_with(&format!("S1;R;{key};")), "{state:?}");
    assert_eq!(gear(&world, bob), 0);
    world.assert_clean();
}

#[test]
fn a_room_allowing_more_than_the_kits_offers_a_higher_preset() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let carol = world.connect("Carol");
    let frank = world.connect("Frank");
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 7,
            ..spec(8)
        },
        None,
    );
    let open = create(&mut world, frank, spec(8), None);
    // Carol saved a level 5 preset, and plays her starter kit now.
    with_gear(&mut world, carol, 5);
    asked(&mut world, carol, PRESET_SAVE_BASE + 3);
    starter_gear(&mut world, carol);
    assert_eq!(
        rows(&mut world, carol, ROOM_OFFER_BASE + key, 1),
        [format!("S1;O;{key};1;3;5;0;7")]
    );
    // A room of any max level offers nothing.
    assert_eq!(
        rows(&mut world, carol, ROOM_OFFER_BASE + open, 1),
        [format!("S1;O;{open};0;0;0;0;0")]
    );
    // "Enter anyway": the plain join, the kits as they are.
    let state = rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    assert!(state[0].starts_with(&format!("S1;R;{key};")), "{state:?}");
    assert_eq!(gear(&world, carol), 0);
}

#[test]
fn no_preset_that_fits_keeps_the_refusal() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let dave = world.connect("Dave");
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 3,
            ..spec(8)
        },
        None,
    );
    with_gear(&mut world, dave, 5);
    // Only a preset over the max.
    asked(&mut world, dave, PRESET_SAVE_BASE + 1);
    assert_eq!(
        rows(&mut world, dave, ROOM_OFFER_BASE + key, 1),
        [format!("S1;O;{key};0;0;0;5;3")]
    );
    assert_eq!(
        rows(&mut world, dave, ROOM_JOIN_BASE + key, 1),
        ["S1;N;12;0;0"]
    );
    // "Switch and enter" with an empty preset: nothing changes, no join.
    let kits = world.account(dave).kits.clone();
    let (lists, decoded) = asked(&mut world, dave, ROOM_PRESET_BASE + 100_000 + key);
    assert_eq!(lists, [["S1;N;14;0;0"]]);
    assert!(!sent_account(&decoded));
    assert_eq!(world.account(dave).kits, kits);
    assert_eq!(rows(&mut world, dave, ROOM_STATE, 1), ["S1;N;0;0;0"]);
}

#[test]
fn play_offers_a_preset_for_the_room_it_would_take() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let erin = world.connect("Erin");
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 7,
            ..spec(8)
        },
        None,
    );
    with_gear(&mut world, erin, 5);
    asked(&mut world, erin, PRESET_SAVE_BASE + 1);
    starter_gear(&mut world, erin);
    assert_eq!(
        rows(&mut world, erin, ROOM_OFFER_BASE, 1),
        [format!("S1;O;{key};1;1;5;0;7")]
    );
    // "Switch and enter": preset 1 on, then Play, which takes that room.
    let (lists, _) = asked(&mut world, erin, ROOM_PRESET_BASE);
    assert_eq!(lists[0][0], "P1;L;2;1;0");
    assert!(
        lists[1][0].starts_with(&format!("S1;R;{key};")),
        "{lists:?}"
    );
    assert_eq!(gear(&world, erin), 5);
    // In a room: Play offers nothing.
    assert_eq!(
        rows(&mut world, erin, ROOM_OFFER_BASE, 1),
        ["S1;O;0;0;0;0;5;0"]
    );
}
