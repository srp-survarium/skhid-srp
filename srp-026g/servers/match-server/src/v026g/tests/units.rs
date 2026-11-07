//! The registry and the settings.

use crate::v026g::{
    CreateError, LateInputs, MatchRegistry, Seat, parse_late_inputs, parse_number,
    parse_snapshot_dir,
};

use super::{MAP_BATTERIES, session_of, setup};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::maps::{self, GameMode, MatchMap};
use survarium::v026g::match_msgs::static_info::parse_options;

#[test]
fn a_made_match_seats_its_players_until_it_is_over() {
    let registry = MatchRegistry::new("srp.example.com", 1236);
    let ticket = registry.create(setup(3, MAP_BATTERIES)).unwrap();
    assert_ne!(ticket.match_id, 0);
    assert_eq!(ticket.address(), "srp.example.com:1236");
    assert_eq!(
        (ticket.host.as_str(), ticket.port),
        ("srp.example.com", 1236)
    );
    assert!(registry.is_open(ticket.match_id));
    for place in 0..3 {
        assert_eq!(
            registry.seat(session_of(place)),
            Some(Seat {
                match_id: ticket.match_id,
                place: place as u8
            })
        );
    }
    assert_eq!(registry.seat(0xBEEF), None);
    // The static info is built with the match: its N is the roster's size.
    let formed = registry.formed(ticket.match_id).unwrap();
    assert_eq!(formed.info.roster(), 3);
    assert_eq!(formed.places[2].name, "player2");
    assert_eq!(formed.places[1].team, 1);

    // A seated session cannot be put in another match, until it gives its
    // place up or the match is over.
    assert_eq!(
        registry.create(setup(1, MAP_BATTERIES)),
        Err(CreateError::AlreadySeated {
            session_id: session_of(0),
            match_id: ticket.match_id
        })
    );
    assert!(registry.release(session_of(0)).is_some());
    let second = registry.create(setup(1, MAP_BATTERIES)).unwrap();
    assert_ne!(second.match_id, ticket.match_id);
    assert_eq!(registry.match_count(), 2);
    registry.end(ticket.match_id);
    assert!(!registry.is_open(ticket.match_id));
    assert_eq!(registry.seat(session_of(1)), None);
    assert_eq!(
        registry.seat(session_of(0)).unwrap().match_id,
        second.match_id
    );
}

#[test]
fn a_place_follows_its_account_to_a_new_session() {
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    let ticket = registry.create(setup(2, MAP_BATTERIES)).unwrap();
    let seat = Seat {
        match_id: ticket.match_id,
        place: 1,
    };
    // Account 101 (place 1) signed in again: session 0x2001 now.
    assert_eq!(registry.reseat(ticket.match_id, 101, 0x2001), Some(seat));
    assert_eq!(registry.seat(0x2001), Some(seat));
    assert_eq!(registry.seat(session_of(1)), None);
    assert_eq!(registry.seat(session_of(0)).unwrap().place, 0);
    // Again with the same session: nothing changes.
    assert_eq!(registry.reseat(ticket.match_id, 101, 0x2001), Some(seat));
    // Not an account of the match, not a match, and never a session that
    // has another place.
    assert_eq!(registry.reseat(ticket.match_id, 999, 0x2002), None);
    assert_eq!(registry.reseat(ticket.match_id ^ 1, 101, 0x2002), None);
    assert_eq!(registry.reseat(ticket.match_id, 101, session_of(0)), None);
    assert_eq!(registry.seat(0x2001), Some(seat));
    assert_eq!(registry.seat(session_of(0)).unwrap().place, 0);

    // With its result a match's players are free, while it is still open
    // for its clients to hear the end.
    registry.free_seats(ticket.match_id);
    assert!(registry.is_open(ticket.match_id));
    assert_eq!(registry.seat(0x2001), None);
    assert!(registry.create(setup(2, MAP_BATTERIES)).is_ok());
    registry.end(ticket.match_id);
    assert_eq!(registry.reseat(ticket.match_id, 101, 0x2003), None);
}

#[test]
fn what_the_clients_could_not_hold_is_refused_when_the_match_is_made() {
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    assert_eq!(
        registry.create(setup(0, MAP_BATTERIES)),
        Err(CreateError::Roster(0))
    );
    assert_eq!(
        registry.create(setup(21, MAP_BATTERIES)),
        Err(CreateError::Roster(21))
    );
    // maps_dict id 11 is a test map the client cannot load: no match names it.
    assert_eq!(MatchMap::of(11), None);
    let mut twice = setup(2, MAP_BATTERIES);
    twice.players[1].session_id = twice.players[0].session_id;
    assert_eq!(
        registry.create(twice),
        Err(CreateError::DuplicateSession(session_of(0)))
    );
    // A name longer than the client's buffer.
    let mut long = setup(2, MAP_BATTERIES);
    long.players[0].profile.name = "x".repeat(64);
    assert!(matches!(
        registry.create(long),
        Err(CreateError::StaticInfo(_))
    ));
    // Nothing of a refused match stays.
    assert_eq!(registry.match_count(), 0);
    assert_eq!(registry.seat(session_of(0)), None);
    assert!(registry.create(setup(20, MAP_BATTERIES)).is_ok());
}

#[test]
fn blackout_is_the_static_info_entry_on_its_battery_map() {
    // maps_dict 61: Rudnya's batteries (mode 2); 62: its chargers (mode 3).
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    // Rudnya's Blackout map: its battery map's twin (`blackout-mode-1.json`).
    let twin = maps::find(1, maps::weather::DAY, GameMode::Blackout).unwrap();
    let blackout = MatchMap::of(twin.id).unwrap();
    assert_eq!(blackout.mode(), GameMode::Blackout);
    for (played, expected) in [
        (blackout, true),
        (MatchMap::of(MAP_BATTERIES).unwrap(), false),
        (MatchMap::of(62).unwrap(), false),
    ] {
        let mut made = setup(2, MAP_BATTERIES);
        made.map = played;
        let ticket = registry.create(made).unwrap();
        let formed = registry.formed(ticket.match_id).unwrap();
        assert_eq!(formed.played, played);
        let options = parse_options(formed.info.options(), Dictionary::shared()).unwrap();
        assert_eq!(options.map, played);
        let entry = formed.info.options().ends_with(b"SRPX\x02\x01\x01");
        assert_eq!(entry, expected, "{played:?}");
        registry.end(ticket.match_id);
    }
}

/// Scarcity (`docs/notes/scarcity.md`): the kits the clients get, and the
/// server reads the snapshots against, are cut to Scarcity's ammo, a late
/// joiner's too; a plain match's are as the lobby gave them. And the static
/// info's variant entry is Scarcity's.
#[test]
fn a_scarcity_match_gives_the_kits_cut_to_its_ammo() {
    use survarium::v026g::match_msgs::static_info::MatchItem;
    use survarium::v026g::scarcity;
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    let twin = maps::find(1, maps::weather::DAY, GameMode::Scarcity).unwrap();
    let scarcity_map = MatchMap::of(twin.id).unwrap();
    // The TT-33 in slot 7 and 50 of its ammo in slot 8: Scarcity gives 8.
    let with_ammo = |mut player: crate::v026g::MatchPlayer| {
        let profile = &mut player.profile;
        profile.items.push(MatchItem {
            instance_id: 2,
            dict_id: Dictionary::shared().item_named("ammo_7.62x25").unwrap().dict_id,
            condition_or_stack: 200,
            ..MatchItem::default()
        });
        profile.kits[0].slots[8] = Some((1, 50));
        player
    };
    for (played, cut) in [
        (scarcity_map, true),
        (MatchMap::of(MAP_BATTERIES).unwrap(), false),
    ] {
        let mut made = setup(3, MAP_BATTERIES);
        made.map = played;
        made.players = made.players.into_iter().map(with_ammo).collect();
        let late = made.players.pop().unwrap();
        let given = made.players[0].profile.clone();
        let mut rationed = given.clone();
        scarcity::ration(&mut rationed, Dictionary::shared());
        assert_eq!(rationed.kits[0].slots[8], Some((1, 8)));
        let want = if cut { &rationed } else { &given };
        let ticket = registry.create_growing(made).unwrap();
        let formed = registry.formed(ticket.match_id).unwrap();
        let options = parse_options(formed.info.options(), Dictionary::shared()).unwrap();
        assert_eq!(formed.profiles[0].kits, want.kits, "{played:?}");
        assert_eq!(options.players[0].kits, want.kits, "{played:?}");
        assert_eq!(
            formed.info.options().ends_with(b"SRPX\x02\x01\x02"),
            cut,
            "{played:?}"
        );
        // A late joiner's kits: cut as the others'.
        registry.set_admitting(
            ticket.match_id,
            2,
            Ok(std::time::Instant::now() + std::time::Duration::from_secs(60)),
        );
        registry.join_running(ticket.match_id, late).unwrap();
        let formed = registry.formed(ticket.match_id).unwrap();
        let options = parse_options(formed.info.options(), Dictionary::shared()).unwrap();
        assert_eq!(formed.profiles[2].kits, want.kits, "{played:?}");
        assert_eq!(options.players[2].kits, want.kits, "{played:?}");
        registry.end(ticket.match_id);
    }
}

#[test]
fn the_settings_and_their_defaults() {
    assert_eq!(parse_late_inputs(None), LateInputs::Relay);
    assert_eq!(parse_late_inputs(Some("relay")), LateInputs::Relay);
    assert_eq!(parse_late_inputs(Some("discard")), LateInputs::Discard);
    assert_eq!(parse_late_inputs(Some(" Move ")), LateInputs::Move);
    assert_eq!(parse_late_inputs(Some("drop")), LateInputs::Relay);
    assert_eq!(parse_number("X", None, 10), 10);
    assert_eq!(parse_number("X", Some(" 120 "), 10), 120);
    assert_eq!(parse_number("X", Some("long"), 10), 10);
    assert_eq!(
        parse_snapshot_dir(None),
        Some(std::path::PathBuf::from("logs/snapshots"))
    );
    assert_eq!(parse_snapshot_dir(Some("off")), None);
    assert_eq!(parse_snapshot_dir(Some("")), None);
    assert_eq!(
        parse_snapshot_dir(Some("d:/worlds")),
        Some(std::path::PathBuf::from("d:/worlds"))
    );
}
