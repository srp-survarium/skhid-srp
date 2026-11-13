//! Production encoders are an independent compatibility oracle for the guard.
use srp_validate::messages::{self, Dictionary as GuardDictionary};
use survarium::v026g::{dictionary::Dictionary, maps, match_msgs::static_info as info};

struct Data(&'static Dictionary);
impl GuardDictionary for Data {
    fn item(&self, id: u16) -> Option<bool> {
        self.0.item(id).map(|item| item.is_stack)
    }
    fn map(&self, id: u8) -> bool {
        maps::map_mode(id).is_some_and(|map| map.is_playable())
    }
}

fn options(blackout: bool) -> info::MatchOptions {
    let mut kit = info::MatchKit::empty(1, "kit");
    kit.slots[7] = Some((0, 1));
    info::MatchOptions {
        players: vec![info::MatchProfile {
            team: 0,
            name: "alice".into(),
            clan_tag: String::new(),
            protection: info::NO_PROTECTION,
            kits: vec![kit],
            items: vec![info::MatchItem {
                instance_id: 1,
                dict_id: Dictionary::shared().item_named("tt_33").unwrap().dict_id,
                condition_or_stack: 100,
                ..info::MatchItem::default()
            }],
            squad: 0,
        }],
        // Blackout is its maps' mode (#1243): the options carry its SRPX entry.
        map: if blackout {
            maps::playable()
                .find(|map| map.mode == maps::GameMode::Blackout)
                .and_then(|map| maps::MatchMap::of(map.id))
                .expect("a Blackout map")
        } else {
            maps::MatchMap::of(61).unwrap()
        },
        match_id: 0x1234_5678,
        match_level: 3,
        score_weights: info::SCORE_WEIGHTS,
    }
}

/// The options of every playable map, as the server writes them, pass the
/// clients' check: an entry of srp's block a map or a mode adds
/// (`static_info::options_bytes`) fails here until the check knows it.
#[test]
fn the_options_of_every_map_are_accepted() {
    let data = Data(Dictionary::shared());
    let mut modes = Vec::new();
    for map in maps::playable() {
        let options = options_on(maps::MatchMap::of(map.id).unwrap());
        let bytes = info::options_bytes(&options, data.0).unwrap();
        assert_eq!(messages::options(&bytes, &data), Ok(1), "map {}", map.id);
        if !modes.contains(&map.mode) {
            modes.push(map.mode);
        }
    }
    // The test archive's: Research's maps come with a pack.
    assert!(modes.contains(&maps::GameMode::Blackout), "{modes:?}");
    assert!(modes.contains(&maps::GameMode::FreeForAll), "{modes:?}");
    assert!(modes.len() >= 5, "{modes:?}");
}

/// The options of a match on `map`: its one player on team 0, or in a
/// free-for-all on his own side.
fn options_on(map: maps::MatchMap) -> info::MatchOptions {
    let mut options = info::MatchOptions { map, ..options(false) };
    if map.mode() == maps::GameMode::FreeForAll {
        options.players[0].team = maps::free_for_all_team(0);
    }
    options
}

/// A free-for-all's options: each place on its own side, and srp's entry
/// with the kills that win. Neither goes without the other, at the server
/// or at the clients' check.
#[test]
fn a_free_for_alls_own_sides_and_kill_target_go_together() {
    let data = Data(Dictionary::shared());
    let map = maps::playable()
        .find(|map| map.mode == maps::GameMode::FreeForAll)
        .and_then(|map| maps::MatchMap::of(map.id))
        .expect("a free-for-all map");
    let options = options_on(map);
    assert_eq!(options.players[0].team, 4);
    let bytes = info::options_bytes(&options, data.0).unwrap();
    let entry = b"SRPX\x03\x02\x1e\x00";
    assert!(bytes.ends_with(entry), "30 kills win");
    assert_eq!(messages::options(&bytes, &data), Ok(1));
    assert_eq!(info::parse_options(&bytes, data.0).unwrap(), options);
    // The server writes no free-for-all with a team of the two, and no
    // other mode with a side of one's own.
    let mut teams = options.clone();
    teams.players[0].team = 0;
    assert_eq!(
        info::options_bytes(&teams, data.0),
        Err(info::StaticInfoError::Team { player: 0, team: 0 })
    );
    let mut own = options_on(maps::MatchMap::of(61).unwrap());
    own.players[0].team = 4;
    assert_eq!(
        info::options_bytes(&own, data.0),
        Err(info::StaticInfoError::Team { player: 0, team: 4 })
    );
    // The clients' check: own sides without the entry, with Blackout's, or
    // with no kills to win; the entry with the two teams.
    let native = &bytes[..bytes.len() - entry.len()];
    for suffix in [
        &b""[..],
        b"SRPX\x02\x01\x01",
        b"SRPX\x03\x02\x00\x00",
        b"SRPX\x03\x01\x1e",
        b"SRPX\x03\x02\x1e\x00\x02\x01\x01",
    ] {
        let bad = [native, suffix].concat();
        assert!(messages::options(&bad, &data).is_err(), "{suffix:?}");
        assert!(info::parse_options(&bad, data.0).is_err(), "{suffix:?}");
    }
    let plain = info::options_bytes(&options_on(maps::MatchMap::of(61).unwrap()), data.0).unwrap();
    let bad = [&plain[..], entry].concat();
    assert!(messages::options(&bad, &data).is_err());
    assert!(info::parse_options(&bad, data.0).is_err());
}

#[test]
fn current_server_options_and_blackout_are_accepted() {
    let data = Data(Dictionary::shared());
    for blackout in [false, true] {
        let bytes = info::options_bytes(&options(blackout), data.0).unwrap();
        assert_eq!(messages::options(&bytes, &data), Ok(1));
        let mut payload = vec![0];
        payload.extend(survarium::v026g::lzma2::stored(&bytes));
        assert_eq!(messages::check(messages::id::STATIC_INFO, &payload), Ok(()));
        // Every truncation of the mandatory native options is rejected.
        let native_len = bytes.len() - if blackout { 7 } else { 0 };
        for len in 0..native_len {
            assert!(
                messages::options(&bytes[..len], &data).is_err(),
                "prefix {len}"
            );
        }
        if blackout {
            for suffix in [
                &b"SRPX\x02\x01\x00"[..],
                b"SRPX\x09\x00",
                b"SRPY\x02\x01\x01",
            ] {
                let mut bad = bytes[..native_len].to_vec();
                bad.extend(suffix);
                assert!(messages::options(&bad, &data).is_err());
            }
        }
    }
}

#[test]
fn shipped_command_marker_is_allowed_but_unused_bit_and_nan_are_not() {
    for id in [messages::id::INPUT_RELAY, messages::id::INPUT_RELAY_PRECISE] {
        let mut payload = vec![0; messages::fixed_length(id).unwrap()];
        let actions = payload.len() - 4;
        payload[actions..].copy_from_slice(&((1u32 << 29) | (3 << 14)).to_le_bytes());
        assert_eq!(messages::check(id, &payload), Ok(()));
        payload[actions..].copy_from_slice(&(1u32 << 30).to_le_bytes());
        assert!(messages::check(id, &payload).is_err());
        payload[actions..].fill(0);
        payload[actions - 4..actions - 2].copy_from_slice(&0x7c00u16.to_le_bytes());
        assert!(messages::check(id, &payload).is_err());
    }
}

#[test]
fn current_emitted_messages_and_late_join_headers_are_accepted() {
    use survarium::v026g::match_msgs::{PlayerInput, ServerMessage as M, WireTime, late_join};
    let tick = WireTime::tick(5000);
    let precise = WireTime::precise(5049);
    let mut messages = vec![
        M::Ports { min: 1236, max: 1236 },
        M::SetTime { generation: 7, echo: 21, match_time: 5000 },
        M::Ping { a: 8, b: 9 }, M::ClockOffset(-50),
        M::PlayerEntered { tick: 3, player: 19 },
        M::PlayerLeft { tick: 4, player: 19 },
        M::ProfileInputRelay { tick: 3, player: 19, value: 2 },
        M::MoveProfileInput { from_tick: 3, to_tick: 4 },
        M::DiscardProfileInput { tick: 3 }, M::UploadRequest,
    ];
    for time in [tick, precise] {
        messages.extend([
            M::InputRelay { player: 19, input: PlayerInput {
                time, rotation: [0; 4], actions: 1 << 29,
            }},
            M::MoveInput { from: tick, to: time },
            M::DiscardInput { time },
        ]);
    }
    for packed in [false, true] {
        messages.push(M::Snapshot { generation: 7, time: 5000, body: vec![0; 4000], packed });
    }
    for message in messages {
        let payload = message.payload();
        assert_eq!(messages::check(message.id(), &payload), Ok(()), "{message:?}");
        if messages::fixed_length(message.id()).is_some() {
            for len in 0..payload.len() {
                assert!(messages::check(message.id(), &payload[..len]).is_err());
            }
            let mut over = payload.clone(); over.push(0);
            assert!(messages::check(message.id(), &over).is_err());
        }
    }
    let prepare = late_join::Prepare {
        proposal: late_join::Proposal { match_id: 42, serial: 1, place: 19 },
        profile: options(false).players.remove(0),
    }.encode(Dictionary::shared()).unwrap();
    assert_eq!(messages::check(messages::id::STATIC_INFO, &prepare), Ok(()));
    for len in 0..14 {
        assert!(messages::check(messages::id::STATIC_INFO, &prepare[..len]).is_err());
    }
    let mut bad = prepare; bad[8..12].fill(0);
    assert!(messages::check(messages::id::STATIC_INFO, &bad).is_err());
}

/// A spectator's 0xE4 (local place 0xFF, #1285) and the 0xEA that ends its
/// match pass the clients' check: patch 114 discarded both.
#[test]
fn a_spectators_static_info_and_farewell_are_accepted() {
    use survarium::v026g::match_msgs::ServerMessage as M;
    let info = info::StaticInfo::new(&options(false), Dictionary::shared()).unwrap();
    let e4 = info.spectator_message();
    assert_eq!(messages::check(e4.id(), &e4.payload()), Ok(()));
    let farewell = M::PlayerLeft { tick: 4, player: info::NO_LOCAL_PLACE };
    assert_eq!(messages::check(farewell.id(), &farewell.payload()), Ok(()));
}
