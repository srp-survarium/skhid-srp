//! Kit presets (`presets`, `docs/notes/presets.md`): the Character panel's
//! list and its choice of the active preset, which the kits are (every kit
//! change is the active preset's), and a room join that offers a preset
//! ("Enter anyway" / "Exit" or "Switch and enter"), "Switch and enter"
//! choosing it before the join.

use session::v026g::{matchmaking, quests};
use survarium::v026g::lobby::presets::{PRESET_CHOOSE_BASE, PRESETS_FIRST};
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

/// The header of the presets' list a choice of `preset` is answered with.
fn choose(world: &mut World, index: usize, preset: u8) -> Vec<String> {
    let (lists, _) = asked(world, index, PRESET_CHOOSE_BASE + u32::from(preset));
    lists[0].clone()
}

fn gear(world: &World, index: usize) -> u8 {
    matchmaking::gear_level(&world.account(index), quests::now())
}

fn sent_account(decoded: &[Decoded]) -> bool {
    decoded
        .iter()
        .any(|message| matches!(message, Decoded::Account(_)))
}

fn kits(world: &World, index: usize) -> Vec<[Option<session::v026g::account::KitSlot>; 23]> {
    world
        .account(index)
        .kits
        .iter()
        .map(|kit| kit.slots)
        .collect()
}

#[test]
fn the_dropdown_lists_the_presets_and_chooses_the_active_one() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    // Preset 1 active: the starter kit, level 0; the others empty.
    let (lists, _) = asked(&mut world, alice, PRESETS_FIRST);
    assert_eq!(
        lists,
        [[
            "P1;L;0;0;0;1",
            "1;1;0",
            "2;0;-1",
            "3;0;-1",
            "4;0;-1",
            "5;0;-1"
        ]]
    );
    let starter = kits(&world, alice);

    // An empty preset chosen starts as the kits worn.
    let (lists, decoded) = asked(&mut world, alice, PRESET_CHOOSE_BASE + 2);
    assert_eq!(
        lists,
        [[
            "P1;L;1;2;0;2",
            "1;1;0",
            "2;1;0",
            "3;0;-1",
            "4;0;-1",
            "5;0;-1"
        ]]
    );
    assert!(sent_account(&decoded));
    assert_eq!(kits(&world, alice), starter);

    // Changed in preset 2 (as a match's results change an account): its
    // level follows, preset 1 stays as it was left.
    with_gear(&mut world, alice, 5);
    let geared = kits(&world, alice);
    let inventory = world.account(alice).inventory.clone();
    let (lists, _) = asked(&mut world, alice, PRESETS_FIRST);
    assert_eq!(lists[0][..3], ["P1;L;0;0;0;2", "1;1;0", "2;1;5"]);

    // Back to 1, then 2: each its own kits, nothing bought, sold or made.
    assert_eq!(choose(&mut world, alice, 1)[0], "P1;L;1;1;0;1");
    assert_eq!(kits(&world, alice), starter);
    assert_eq!(gear(&world, alice), 0);
    choose(&mut world, alice, 2);
    assert_eq!(kits(&world, alice), geared);
    assert_eq!(gear(&world, alice), 5);
    assert_eq!(world.account(alice).inventory, inventory);
    world.assert_clean();
}

/// A move of the lobby (`46 00`) in the active preset is kept with it: away
/// and back, the kits are as the move left them.
#[test]
fn a_move_in_the_kits_is_the_active_presets() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    choose(&mut world, alice, 2);
    let (pistol, tt33) = {
        let account = world.account(alice);
        let pistol = account.kits[0].slots[7].unwrap().item_id;
        (pistol, account.item(pistol).unwrap().dict_id)
    };
    world.ask(
        alice,
        request::move_item(1, pistol, tt33, 7, INVENTORY_SLOT, 1),
    );
    assert_eq!(world.account(alice).kits[0].slots[7], None);
    let moved = kits(&world, alice);
    choose(&mut world, alice, 1);
    assert!(world.account(alice).kits[0].slots[7].is_some());
    choose(&mut world, alice, 2);
    assert_eq!(kits(&world, alice), moved);
    world.assert_clean();
}

#[test]
fn a_sold_item_is_counted_as_an_empty_slot() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    // Preset 2 the starter too; preset 1 sells the pistol.
    choose(&mut world, alice, 2);
    choose(&mut world, alice, 1);
    let pistol = world.account(alice).kits[0].slots[7].unwrap().item_id;
    world.ask(alice, request::sell(pistol, 1));
    assert!(world.account(alice).item(pistol).is_none());
    let silver = world.account(alice).generic_money;
    let (lists, _) = asked(&mut world, alice, PRESET_CHOOSE_BASE + 2);
    // One slot empty; preset 2 now can not play (no weapon).
    assert_eq!(lists[0][..3], ["P1;L;1;2;1;2", "1;1;-1", "2;1;-1"]);
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
    // Bob: preset 1 his starter kit (level 0); he plays preset 2, level 5.
    choose(&mut world, bob, 2);
    with_gear(&mut world, bob, 5);
    assert_eq!(
        rows(&mut world, bob, ROOM_OFFER_BASE + key, 1),
        [format!("S1;O;{key};2;1;0;5;3")]
    );
    // "Exit": nothing asked; the join itself is refused as before.
    assert_eq!(
        rows(&mut world, bob, ROOM_JOIN_BASE + key, 1),
        ["S1;N;12;0;0"]
    );
    // "Switch and enter": preset 1 active, then the join.
    let (lists, decoded) = asked(&mut world, bob, ROOM_PRESET_BASE + key);
    assert!(sent_account(&decoded));
    assert_eq!(lists[0][0], "P1;L;1;1;0;1");
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
    // Carol made preset 3 of level 5, and plays preset 1, her starter.
    choose(&mut world, carol, 3);
    with_gear(&mut world, carol, 5);
    choose(&mut world, carol, 1);
    assert_eq!(gear(&world, carol), 0);
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
    // Only presets over the max: the active one, level 5.
    with_gear(&mut world, dave, 5);
    assert_eq!(
        rows(&mut world, dave, ROOM_OFFER_BASE + key, 1),
        [format!("S1;O;{key};0;0;0;5;3")]
    );
    assert_eq!(
        rows(&mut world, dave, ROOM_JOIN_BASE + key, 1),
        ["S1;N;12;0;0"]
    );
    // "Switch and enter" while its game is on its way to a match: nothing
    // changes, no join (code 14).
    let addr = world.clients[dave].0;
    world.core.player_mut(addr).unwrap().in_match = Some(1);
    let before = world.account(dave).clone();
    let (lists, decoded) = asked(&mut world, dave, ROOM_PRESET_BASE + 100_000 + key);
    assert_eq!(lists, [["S1;N;14;0;0"]]);
    assert!(!sent_account(&decoded));
    assert_eq!(*world.account(dave), before);
    world.core.player_mut(addr).unwrap().in_match = None;
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
    // Erin: preset 2 of level 5; she plays preset 1, her starter.
    choose(&mut world, erin, 2);
    with_gear(&mut world, erin, 5);
    choose(&mut world, erin, 1);
    assert_eq!(
        rows(&mut world, erin, ROOM_OFFER_BASE, 1),
        [format!("S1;O;{key};1;2;5;0;7")]
    );
    // "Switch and enter": preset 2 active, then Play, which takes that room.
    let (lists, _) = asked(&mut world, erin, ROOM_PRESET_BASE + 100_000);
    assert_eq!(lists[0][0], "P1;L;1;2;0;2");
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

/// The review's case: preset 2's kit 1 is empty and its kit 2 plays; the
/// popup offers it and "Switch and enter" plays kit 2, so the member is fit
/// for the room's match (not taken out at its start).
#[test]
fn switch_and_enter_plays_a_kit_of_the_preset_that_can_play() {
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
    // Preset 2: kit 1 empty, the starter pistol and rounds in kit 2.
    choose(&mut world, bob, 2);
    world.change(bob, |account| {
        let starter = account.kits[0].slots;
        account.kits[0].slots = [None; 23];
        account.kits[1].slots[7] = starter[7];
        account.kits[1].slots[8] = starter[8];
    });
    // Preset 1 plays kit 1, level 5.
    choose(&mut world, bob, 1);
    with_gear(&mut world, bob, 5);
    assert_eq!(world.account(bob).active_kit, 1);
    assert_eq!(
        rows(&mut world, bob, ROOM_OFFER_BASE + key, 1),
        [format!("S1;O;{key};2;2;0;5;3")]
    );
    let (lists, _) = asked(&mut world, bob, ROOM_PRESET_BASE + 100_000 + key);
    assert_eq!(lists[0][0], "P1;L;1;2;0;2");
    assert!(
        lists[1][0].starts_with(&format!("S1;R;{key};")),
        "{lists:?}"
    );
    assert_eq!(world.account(bob).active_kit, 2);
    // Bob's row in the room: fit to play (`P;<i>;<gear>;<owner>;<me>;
    // <in match>;<fit>;...`).
    let me = lists[1]
        .iter()
        .find(|row| row.starts_with("P;") && row.split(';').nth(4) == Some("1"))
        .unwrap()
        .clone();
    assert_eq!(me.split(';').nth(6), Some("0"), "{me}");
    world.assert_clean();
}
