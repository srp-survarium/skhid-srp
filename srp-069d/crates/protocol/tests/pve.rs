//! PvE requests (docs/notes/pve.md section 1.1): the byte layouts of the exe builders, round trips, and the squad /
//! invite requests with their PvE meaning.

use protocol::c2s::{Pve5F, Request, Squad49};
use protocol::s2c::{Squad, SquadUpdate, SQUAD_TYPE_PVE};
use protocol::{Decode, Encode};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn check(id: u8, payload: &str, want: Request) {
    let p = hex(payload);
    let got = Request::decode(id, &p).unwrap_or_else(|e| panic!("{id:#04x} {payload}: {e}"));
    assert_eq!(got, want, "{payload}");
    assert_eq!(got.encode().unwrap(), (id, p), "{payload}");
}

#[test]
fn pve_requests_5f() {
    // 00 set_pve_match_settings(map 15, difficulty 2)
    check(0x5F, "000f00000002000000", Request::Pve(Pve5F::Settings { map: 15, difficulty: 2 }));
    // 01 commander Play, 02 solo Play
    check(0x5F, "010f00000001000000", Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 1 }));
    check(0x5F, "020f00000000000000", Request::Pve(Pve5F::Start { solo: true, map: 15, difficulty: 0 }));
    // 03 top 100, 04 close stats, 05 unlock rewards, 06 reroll rewards
    check(0x5F, "03", Request::Pve(Pve5F::Top100));
    check(0x5F, "04", Request::Pve(Pve5F::CloseStats));
    check(0x5F, "05", Request::Pve(Pve5F::UnlockRewards));
    check(0x5F, "06", Request::Pve(Pve5F::RerollRewards));
}

#[test]
fn pve_requests_reject_bad_forms() {
    assert!(Request::decode(0x5F, &hex("07")).is_err());
    assert!(Request::decode(0x5F, &hex("0100")).is_err(), "truncated Play");
    assert!(Request::decode(0x5F, &[]).is_err());
}

#[test]
fn squad_and_invite_requests() {
    // 49 04 = leave_squad (decline of the commander settings), 49 05 u8 state, u8 0
    check(0x49, "04", Request::Squad(Squad49::Leave));
    check(0x49, "050300", Request::Squad(Squad49::PveState { state: 3, zero: 0 }));
    // 4D 17: invite to a PvE squad (u64 id, u8 0x17, str)
    check(0x4D, "0700000000000000170161", Request::People { id: 7, kind: 0x17, text: "a".into() });
    check(0x4D, "0700000000000000180161", Request::People { id: 7, kind: 0x18, text: "a".into() });
}

#[test]
fn ae_carries_the_squad_type() {
    let s = SquadUpdate { squad: Some(Squad { id: 1, kind: SQUAD_TYPE_PVE, ..Default::default() }) };
    let b = s.to_bytes().unwrap();
    // u32 id, u8 count, u8 commander, u8 type
    assert_eq!(&b[..7], &[1, 0, 0, 0, 0, 0, 1]);
    assert_eq!(SquadUpdate::from_bytes(&b).unwrap(), s);
}

#[test]
fn pve_drops_83_45_0b_layout() {
    use protocol::items::ItemRecord;
    use protocol::s2c::{Inventory45, Reply, S2c};
    // reader 0x200193DF0: u8 n, n x item record (u32 uid, u32 +4, u16 +8, u16 dict, then for dict != 0 the tail read by
    // 0x2001263D0: u32 value, u8 mods, u8 n2, u8 n3, u8 n4), u32 cost, u8 flag
    let rec = ItemRecord { uid: 7, dict: 925, value: 40, ..Default::default() };
    let msg = S2c::Reply(Reply::Inventory(Inventory45::PveDrops { items: vec![rec], cost: 75, flag: true }));
    let bytes = msg.encode_payload().unwrap();
    let want = ["450b", "01", "07000000", "00000000", "0000", "9d03", "28000000", "00000000", "4b000000", "01"].concat();
    assert_eq!(bytes, hex(&want));
    assert_eq!(S2c::decode(0x83, &bytes).unwrap(), msg);
    // no drops (a lost run): n 0, cost 0, no locked chests
    let empty = S2c::Reply(Reply::Inventory(Inventory45::PveDrops { items: vec![], cost: 0, flag: false }));
    let bytes = empty.encode_payload().unwrap();
    assert_eq!(bytes, hex("450b000000000000"));
    assert_eq!(S2c::decode(0x83, &bytes).unwrap(), empty);
}
