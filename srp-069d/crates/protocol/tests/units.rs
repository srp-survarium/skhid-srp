//! Unit tests for the primitives and for rules the golden vectors do not reach.

use protocol::items::{AccountBody, ItemRecord, ModuleElem, Profile};
use protocol::lzma2;
use protocol::s2c::{Account87, Inventory89, MatchResult, ResultPlayer, S2c, SquadUpdate};
use protocol::{Decode, Encode, Error, MsgClass, Reader, Writer};

#[test]
fn strings_use_ff_form_from_255_bytes() {
    for n in [0usize, 1, 254, 255, 256, 1000] {
        let s = "a".repeat(n);
        let mut w = Writer::new();
        w.str(&s);
        let b = w.finish().unwrap();
        if n < 255 {
            assert_eq!(b[0] as usize, n);
            assert_eq!(b.len(), n + 1);
        } else {
            assert_eq!(&b[..3], &[0xFF, n as u8, (n >> 8) as u8]);
            assert_eq!(b.len(), n + 3);
        }
        assert_eq!(Reader::new(&b).str().unwrap(), s);
    }
}

#[test]
fn reader_never_panics_on_short_input() {
    let mut r = Reader::new(&[1, 2, 3]);
    assert!(matches!(r.u32(), Err(Error::Truncated { .. })));
    assert!(matches!(Reader::new(&[5, b'a']).str(), Err(Error::Truncated { .. })));
    // hostile count must not allocate
    assert!(Reader::new(&[0u8; 4]).check_count(usize::MAX / 2, 8).is_err());
}

#[test]
fn lzma2_round_trip_over_chunk_boundary() {
    let raw: Vec<u8> = (0..150_000u32).map(|i| (i * 7 + i / 255) as u8).collect();
    let blob = lzma2::compress(&raw);
    assert_eq!(lzma2::decompress(&blob).unwrap(), raw);
    assert_eq!(lzma2::compress(&[]), vec![0]);
    assert_eq!(lzma2::decompress(&[0]).unwrap(), Vec::<u8>::new());
}

#[test]
fn inventory_89_round_trip_with_all_lists() {
    let it = ItemRecord {
        uid: 9,
        attached_to: 0,
        bound: 3,
        dict: 107,
        value: 107,
        mods: vec![(201, 1), (203, 2)],
        n2: vec![(1, 0.5)],
        decals: vec![],
        modules: vec![ModuleElem { dict: 0x6ec, uid: 42, f24: 0, slot: "picatinny_scope_medium".into(), mods: vec![(1, 2)] }],
    };
    let m = Inventory89 { max_items: 150, items: vec![it.clone(), ItemRecord { uid: 10, dict: 31, value: 31, ..Default::default() }] };
    let b = m.to_bytes().unwrap();
    assert_eq!(Inventory89::from_bytes(&b).unwrap(), m);
    assert_eq!(&b[..8], &[150, 0, 0, 0, 2, 0, 0, 0]);
}

#[test]
fn item_limits_are_enforced() {
    let mut it = ItemRecord { uid: 1, dict: 5, value: 5, ..Default::default() };
    it.mods = vec![(1, 1); 9];
    assert!(matches!(it.to_bytes(), Err(Error::Limit(_))));
    it.mods.clear();
    it.modules = vec![ModuleElem { dict: 1, uid: 1, f24: 0, slot: "x".repeat(33), mods: vec![] }];
    assert!(it.to_bytes().is_err());
}

#[test]
fn short_record_with_dict_zero_is_twelve_bytes() {
    let it = ItemRecord { uid: 1, ..Default::default() };
    assert_eq!(it.to_bytes().unwrap().len(), 12);
}

#[test]
fn profile_modifier_mask_round_trip() {
    let mut p = Profile::default();
    p.slots.insert(7, (0, 0));
    p.slots.insert(8, (1, 100));
    p.mods.insert(0, 1.5);
    p.mods.insert(137, -2.0);
    p.voice = "scheme".into();
    let b = p.to_bytes().unwrap();
    assert_eq!(Profile::from_bytes(&b).unwrap(), p);
    p.mods.insert(138, 0.0);
    assert!(p.to_bytes().is_err());
}

#[test]
fn account_87_needs_one_version_per_profile() {
    let body = AccountBody { name: "test".into(), profiles: vec![Profile::default()], ..Default::default() };
    let ok = Account87 { body: body.clone(), profile_state: vec![(0, 0)], current_profile: 1, ..Default::default() };
    assert_eq!(Account87::from_bytes(&ok.to_bytes().unwrap()).unwrap(), ok);
    let bad = Account87 { body, profile_state: vec![], ..Default::default() };
    assert!(bad.to_bytes().is_err());
}

#[test]
fn ai_players_are_not_modelled_yet() {
    let body = AccountBody { ai_id: 3, ..Default::default() };
    assert!(matches!(body.to_bytes(), Err(Error::Unsupported(_))));
}

#[test]
fn match_result_zero_one_player_matches_gen_97_size() {
    // gen_97.match_end(1, 5, 1, 0, 0, 0, [player("test", 0, 0, 0, 0, 0)]) is 1348 bytes (1344 with an empty name)
    let m = MatchResult { players: vec![ResultPlayer::new("test", 0, 0, 0, 0, 0)], ..Default::default() };
    let b = m.to_bytes().unwrap();
    assert_eq!(MatchResult::from_bytes(&b).unwrap(), m);
    assert_eq!(b.len(), 1348);
}

#[test]
fn the_97_place_counts_higher_scores_across_both_teams_and_ties_share() {
    let mut rows: Vec<ResultPlayer> =
        [(30, 0), (50, 1), (30, 1), (10, 0), (50, 0)].iter().map(|&(s, t)| ResultPlayer::new("p", 0, s, 0, 0, t)).collect();
    protocol::s2c::set_places(&mut rows);
    assert_eq!(rows.iter().map(|r| r.flag).collect::<Vec<_>>(), vec![2, 0, 2, 4, 0]);
    // no scores yet (online matches without a stats source): everyone shares place 0
    let mut zero = vec![ResultPlayer::new("a", 0, 0, 0, 0, 0), ResultPlayer::new("b", 0, 0, 0, 0, 1)];
    protocol::s2c::set_places(&mut zero);
    assert!(zero.iter().all(|r| r.flag == 0));
}

#[test]
fn client_bytes_of_62_21_are_signed_deltas_gold_first() {
    use protocol::s2c::Custom62;
    // the client's +1 silver click: 62 21 00 01; -1 gold: 62 21 FF 00 (0x200282980)
    assert_eq!(Custom62::decode(&[0x21, 0x00, 0x01]).unwrap(), Custom62::Tokens { gold: 0, silver: 1 });
    assert_eq!(Custom62::decode(&[0x21, 0xFF, 0x00]).unwrap(), Custom62::Tokens { gold: -1, silver: 0 });
}

#[test]
fn squad_zero_is_four_bytes() {
    assert_eq!(SquadUpdate::default().to_bytes().unwrap(), vec![0, 0, 0, 0]);
}

#[test]
fn wire_framing_follows_the_class() {
    use protocol::s2c::{PlayGate, QueueState};
    let m = S2c::QueueState(QueueState { state: 0 });
    assert_eq!(m.class(), MsgClass::Ordered);
    assert_eq!(m.to_wire(9).unwrap(), vec![0x85, 9, 0]);
    let g = S2c::PlayGate(PlayGate { value: 60, mode_mask: 0x10, waits: vec![(3, 7)] });
    assert_eq!(g.class(), MsgClass::Plain);
    assert_eq!(g.to_wire(9).unwrap(), [0xAC, 60, 0, 0, 0, 0x10, 0, 0, 0, 1, 3, 7, 0]);
    assert_eq!(S2c::decode(0xAC, &g.to_wire(9).unwrap()[1..]).unwrap(), g);
    let wire = m.to_wire(9).unwrap();
    let (id, x, p) = protocol::s2c::split_wire(&wire).unwrap();
    assert_eq!((id, x, p), (0x85, Some(9), &[0u8][..]));
}

#[test]
fn unknown_server_ids_stay_raw() {
    assert_eq!(S2c::decode(0xC3, &[1, 2]).unwrap(), S2c::Raw { id: 0xC3, payload: vec![1, 2] });
}

#[test]
fn trailing_bytes_are_rejected() {
    assert!(matches!(protocol::s2c::QueueState::from_bytes(&[1, 2]), Err(Error::Trailing(1))));
}

#[test]
fn item_and_preset_messages_round_trip() {
    use protocol::c2s::{Presets58, Request, Roulette5A};
    use protocol::s2c::{Account48, ItemCounts90, PlayingOrderTip, PresetEntry, PresetsBB, Reply, Shop46};
    // 90: a layout seen in the exe reader (u8 n, n x u32 uid + u8 amount)
    let m = S2c::ItemCounts(ItemCounts90 { entries: vec![(100_001, 1), (100_007, 3)] });
    assert_eq!(m.encode_payload().unwrap(), vec![2, 0xA1, 0x86, 1, 0, 1, 0xA7, 0x86, 1, 0, 3]);
    assert_eq!(S2c::decode(0x90, &m.encode_payload().unwrap()).unwrap(), m);
    let m = S2c::PlayingOrderTip(PlayingOrderTip { kind: 2, arg: 9 });
    assert_eq!(S2c::decode(0x86, &m.encode_payload().unwrap()).unwrap(), m);
    assert!(S2c::PlayingOrderTip(PlayingOrderTip { kind: 4, arg: 0 }).encode_payload().is_err());
    // BB
    let m = S2c::Presets(PresetsBB { presets: vec![PresetEntry { id: 7, flag: 0, name: "Rush".into() }] });
    assert_eq!(m.encode_payload().unwrap(), vec![1, 7, 0, 0, 0, 0, 4, b'R', b'u', b's', b'h']);
    assert_eq!(S2c::decode(0xBB, &m.encode_payload().unwrap()).unwrap(), m);
    assert_eq!(S2c::decode(0xBB, &[0]).unwrap(), S2c::Presets(PresetsBB::default()));
    // 83 48 02 and 83 46 06 / 07
    for r in [
        Reply::Account(Account48::ProfileSet { profile: 0x1234 }),
        Reply::Shop(Shop46::Chest { item: 100_005, chest_dict: 1115, rewards: vec![ItemRecord { uid: 100_006, dict: 31, value: 31, ..Default::default() }] }),
        Reply::Shop(Shop46::Search { a: 1, items: vec![] }),
    ] {
        let b = S2c::Reply(r.clone()).encode_payload().unwrap();
        assert_eq!(S2c::decode(0x83, &b).unwrap(), S2c::Reply(r));
    }
    assert_eq!(S2c::Reply(Reply::Account(Account48::ProfileSet { profile: 5 })).encode_payload().unwrap(), vec![0x48, 2, 5, 0, 0, 0]);
    // 58 and 5A requests
    for (mid, p, want) in [
        (0x58u8, vec![0, 3, b'a', b'b', b'c', 9, 0, 0, 0], Request::Presets(Presets58::Create { name: "abc".into(), profile: 9 })),
        (0x58, vec![1, 5, 0, 0, 0], Request::Presets(Presets58::Remove { preset: 5 })),
        (0x58, vec![2, 5, 0, 0, 0, 1, b'x'], Request::Presets(Presets58::Rename { preset: 5, name: "x".into() })),
        (0x58, vec![3, 9, 0, 0, 0, 5, 0, 0, 0], Request::Presets(Presets58::Load { profile: 9, preset: 5 })),
        (0x58, vec![4, 9, 0, 0, 0, 5, 0, 0, 0], Request::Presets(Presets58::Save { profile: 9, preset: 5 })),
        (0x5A, vec![2, 0xFF], Request::Roulette(Roulette5A::Sub2 { a: 0xFF, hunt: None })),
        (0x5A, vec![2, 0, 7, 0, 0, 0], Request::Roulette(Roulette5A::Sub2 { a: 0, hunt: Some(7) })),
        (0x5A, vec![3, 1, 2], Request::Roulette(Roulette5A::Sub3 { a: 1, b: 2 })),
    ] {
        assert_eq!(Request::decode(mid, &p).unwrap(), want);
    }
    assert!(Request::decode(0x58, &[9]).is_err());
}
