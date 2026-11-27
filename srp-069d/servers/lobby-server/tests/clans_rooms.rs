//! Clans (48 07..17, 9A, 4D kind 7, clan chat), chat channels 8 / 9 and custom match rooms (62 xx, 98, A1).

mod support;

use std::time::Duration;

use accounts::public_id;
use protocol::c2s::{Account48, Request, Squad49};
use protocol::s2c::*;
use protocol::Decode;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

fn sign_in_with(e: &mut Env, c: &mut Client, others: &mut [&mut Client]) {
    c.send(Request::SignIn { session: c.session, a: 0xFF, b: 0xFF });
    for _ in 0..300 {
        let mut all: Vec<&mut Client> = vec![&mut *c];
        all.extend(others.iter_mut().map(|o| &mut **o));
        e.step(&mut all, Duration::from_millis(30));
        if c.inbox.iter().any(|(id, _)| *id == 0xB8) {
            break;
        }
    }
    let mut all: Vec<&mut Client> = vec![&mut *c];
    all.extend(others.iter_mut().map(|o| &mut **o));
    e.pump(&mut all);
    c.take();
}

fn three() -> (tempfile::TempDir, Env, Client, Client, Client) {
    let (d, mut e) = env();
    let (mut a, mut b, mut c) = (e.client("alice"), e.client("bob"), e.client("carol"));
    sign_in_with(&mut e, &mut a, &mut []);
    sign_in_with(&mut e, &mut b, &mut [&mut a]);
    sign_in_with(&mut e, &mut c, &mut [&mut a, &mut b]);
    a.take();
    b.take();
    (d, e, a, b, c)
}

fn pump3(e: &mut Env, a: &mut Client, b: &mut Client, c: &mut Client) {
    e.pump(&mut [a, b, c]);
}

/// The 9A messages of a batch.
fn clan_msgs(m: &[S2c]) -> Vec<Clan9A> {
    m.iter()
        .filter_map(|x| match x {
            S2c::Raw { id: 0x9A, payload } => Some(Clan9A::from_bytes(payload).expect("9A decodes")),
            S2c::Clan(_) => Some(Clan9A::NoClan),
            _ => None,
        })
        .collect()
}

fn lines(m: &[S2c]) -> Vec<ChatLine> {
    m.iter().filter_map(|x| if let S2c::Chat(c) = x { Some(c.lines.clone()) } else { None }).flatten().collect()
}

fn rooms98(m: &[S2c]) -> Vec<Room98> {
    m.iter()
        .filter_map(|x| match x {
            S2c::Raw { id: 0x98, payload } => Some(Room98::from_bytes(payload).expect("98 decodes")),
            _ => None,
        })
        .collect()
}

fn denied_texts(m: &[S2c]) -> Vec<String> {
    m.iter()
        .filter_map(|x| match x {
            S2c::Denied(Denied { body: DeniedBody::Text(t), .. }) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// The `83 48` replies of a batch as `sub, data...` (what follows the request id).
fn r48(m: &[S2c]) -> Vec<Vec<u8>> {
    m.iter()
        .filter(|x| matches!(x, S2c::Reply(_)))
        .map(|x| x.encode_payload().unwrap())
        .filter(|p| p.first() == Some(&0x48))
        .map(|p| p[1..].to_vec())
        .collect()
}

fn acct(r: Account48) -> Request {
    Request::Account(r)
}

fn found_clan(e: &mut Env, a: &mut Client, b: &mut Client, c: &mut Client) {
    a.send(acct(Account48::ClanNameCheck { name: "Iron Wolves".into() }));
    a.send(acct(Account48::ClanAbbrCheck { abbr: "x".into() }));
    pump3(e, a, b, c);
    let m = a.take();
    // 83 48 09 "Iron Wolves" ok, 83 48 0A "x" invalid (short = reason 0)
    let replies = r48(&m);
    assert_eq!(replies.len(), 2);
    let data = &replies[0];
    assert_eq!(data[0], 9);
    assert_eq!(&data[2..2 + data[1] as usize], b"Iron Wolves");
    assert_eq!(*data.last().unwrap(), 1);
    let data = &replies[1];
    assert_eq!(data[0], 0x0A);
    assert_eq!(&data[data.len() - 2..], &[0, 0]);
    // creating without a reservation is refused with a text
    a.send(acct(Account48::CreateClan));
    pump3(e, a, b, c);
    assert_eq!(denied_texts(&a.take()), ["st_cant_create_clan"]);
    // reserve, pay, create
    a.send(acct(Account48::ReserveClanName { name: "Iron Wolves".into(), abbr: "IW".into() }));
    pump3(e, a, b, c);
    // the reservation, then the registration form (the creation window's payment step: P = 1, nothing collected yet)
    let form = |m: &[S2c]| clan_msgs(m).into_iter().find_map(|x| if let Clan9A::Registration { supporters_ok, paid, collected, name_state, members, .. } = x { Some((supporters_ok, paid, collected, name_state, members.len())) } else { None });
    let m = a.take();
    assert!(matches!(clan_msgs(&m).first(), Some(Clan9A::Reservation { name, abbr, .. }) if name == "Iron Wolves" && abbr == "IW"));
    assert_eq!(form(&m), Some((true, false, 0, 3, 1)));
    a.send(acct(Account48::CreateClan));
    pump3(e, a, b, c);
    assert_eq!(denied_texts(&a.take()), ["st_cant_create_clan"]);
    let before = e.store.load_profile(a.account).unwrap().silver;
    let cost = lobby_server::clans::CREATION_COST_SILVER;
    // a partial deposit (the window's stepper): taken, shown as progress, not paid yet
    a.send(acct(Account48::BuyClanRegistration { amount: 10_000 }));
    pump3(e, a, b, c);
    let m = a.take();
    assert!(m.iter().any(|x| matches!(x, S2c::Money(_))));
    assert_eq!(form(&m), Some((true, false, 10_000, 3, 1)));
    assert_eq!(e.store.load_profile(a.account).unwrap().silver, before - 10_000);
    // more than is left: only the rest is taken; the form says paid
    a.send(acct(Account48::BuyClanRegistration { amount: cost }));
    pump3(e, a, b, c);
    let m = a.take();
    assert_eq!(form(&m), Some((true, true, cost, 3, 1)));
    assert_eq!(e.store.load_profile(a.account).unwrap().silver, before - cost);
    // nothing more to take
    a.send(acct(Account48::BuyClanRegistration { amount: 1 }));
    pump3(e, a, b, c);
    assert_eq!(denied_texts(&a.take()), ["st_cant_buy_clan_registration"]);
    assert_eq!(e.store.load_profile(a.account).unwrap().silver, before - cost);
    a.send(acct(Account48::CreateClan));
    pump3(e, a, b, c);
    let cm = clan_msgs(&a.take());
    let Some(Clan9A::Full { name, abbr, ranks, members, me_index, commander, .. }) = cm.last().cloned() else { panic!("{cm:?}") };
    assert_eq!((name.as_str(), abbr.as_str()), ("Iron Wolves", "IW"));
    assert_eq!(ranks.len(), 4, "the four standard ranks");
    assert_eq!(members.len(), 1);
    assert_eq!((me_index, commander, members[0].online), (0, true, true));
    assert_eq!(members[0].id, public_id(a.account));
}

/// The client's own request bytes: `48 13` toward commander only asks, `48 12 member 01` is the confirmed
/// hand-over, `48 14 member` the confirmed kick, `48 0E` from the commander disbands.
#[test]
fn hand_over_asks_first_then_the_confirmation_hands_over_kick_and_disband() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    found_clan(&mut e, &mut a, &mut b, &mut c);
    let cid = e.store.clan_id_of(a.account).unwrap().unwrap();
    for x in [b.account, c.account] {
        e.store.clan_invite(a.account, x).unwrap();
        e.store.clan_accept(x, cid).unwrap();
    }
    let rank = |e: &Env, x| e.store.clan_of(x).unwrap().unwrap().member(x).unwrap().rank;
    assert_eq!(rank(&e, b.account), 3, "joined at soldier");
    let id_b = public_id(b.account);
    // soldier -> sergeant -> deputy: plain moves, the reply says "no question" (13 00)
    for want in [2, 1] {
        a.send(acct(Account48::RankUp { member: id_b }));
        pump3(&mut e, &mut a, &mut b, &mut c);
        assert_eq!(r48(&a.take()).last().map(|d| d.as_slice()), Some(&[0x13u8, 0][..]));
        assert_eq!(rank(&e, b.account), want);
    }
    // deputy -> commander: nothing changes, the client is told to ask (13 01)
    a.send(acct(Account48::RankUp { member: id_b }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(r48(&a.take()).last().map(|d| d.as_slice()), Some(&[0x13u8, 1][..]));
    assert_eq!((rank(&e, a.account), rank(&e, b.account)), (0, 1), "no hand-over before the answer");
    // the client's "Yes": 48 12 <member> 01
    a.send(acct(Account48::RankDown { member: id_b, a: 1 }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!((rank(&e, a.account), rank(&e, b.account)), (1, 0), "command handed over");
    // the confirmed kick: 48 14 <member>
    b.send(acct(Account48::Kick { member: public_id(c.account) }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(e.store.clan_of(c.account).unwrap().is_none());
    assert!(clan_msgs(&c.take()).iter().any(|m| matches!(m, Clan9A::NoClan)));
    // the commander confirms "disband" (48 0E): everybody is out
    b.send(acct(Account48::LeaveClan));
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(e.store.clan_of(a.account).unwrap().is_none() && e.store.clan_of(b.account).unwrap().is_none());
    // the client's bytes decode as intended
    assert_eq!(Request::decode(0x48, &[0x0E]).unwrap(), acct(Account48::LeaveClan));
    assert_eq!(Request::decode(0x48, &[0x0F, 0]).unwrap(), acct(Account48::ClanWindowOpened { a: 0 }));
}

#[test]
fn clan_create_invite_accept_chat_ranks_and_leave() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    found_clan(&mut e, &mut a, &mut b, &mut c);
    let (id_a, id_b) = (public_id(a.account), public_id(b.account));
    // the name is taken for everybody else now
    c.send(acct(Account48::ClanNameCheck { name: "iron wolves".into() }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    let m = r48(&c.take());
    let data = m.last().expect("reply");
    assert_eq!(&data[data.len() - 2..], &[0, rules_exists()]);

    // invite: 83 48 10 asks the client for 4D kind 7, which delivers B1 type 7
    a.send(acct(Account48::ClanInvite { player: id_b }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    let m = r48(&a.take());
    let data = m.last().expect("reply");
    assert_eq!(data[0], 0x10);
    assert_eq!(data[1], 0);
    assert_eq!(u64::from_le_bytes(data[6..14].try_into().unwrap()), id_b);
    assert!(b.take().is_empty(), "nothing reaches the target before the client's 4D 7");
    a.send(Request::People { id: id_b, kind: 7, text: "Iron Wolves".into() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let m = b.take();
    let inv = m.iter().find_map(|x| if let S2c::SystemMessage(s) = x { Some(s.clone()) } else { None }).expect("invite message");
    assert_eq!((inv.kind, inv.sender, inv.name.as_str(), inv.clan.as_str(), inv.text.as_str()), (7, id_a, "alice", "IW", "Iron Wolves"));
    // a forged 4D 7 without a stored invitation delivers nothing
    a.send(Request::People { id: public_id(c.account), kind: 7, text: "x".into() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(c.take().is_empty());
    // bob without invite permission cannot invite carol; bob accepts
    b.send(acct(Account48::ImportantMessageRead { id: id_a, answer: 0 })); // 0 = accept
    pump3(&mut e, &mut a, &mut b, &mut c);
    for who in [a.take(), b.take()] {
        let Some(Clan9A::Full { members, .. }) = clan_msgs(&who).last().cloned() else { panic!() };
        assert_eq!(members.len(), 2);
    }
    assert_eq!(e.store.clan_abbr_of(b.account), "IW");
    b.send(acct(Account48::ClanInvite { player: public_id(c.account) }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(denied_texts(&b.take()), ["st_dont_have_permission"]);

    // clan chat: channel 3 with the clan id reaches the other member only, carries the tag
    let clan_id = e.store.clan_id_of(a.account).unwrap().unwrap();
    b.send(Request::Chat { channel: 3, target: u64::from(clan_id), text: "for the pack".into() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(lines(&b.take()).is_empty());
    assert!(lines(&c.take()).is_empty());
    let l = lines(&a.take());
    assert_eq!((l.len(), l[0].channel, l[0].text.as_str(), l[0].clan.as_str(), l[0].sender), (1, 3, "for the pack", "IW", id_b));
    // a wrong clan id reaches nobody
    b.send(Request::Chat { channel: 3, target: u64::from(clan_id) + 5, text: "stale".into() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(lines(&a.take()).is_empty());

    // permissions: bob may not change ranks; alice grants the invite bit (permission 0 = invite) to rank 3 (soldier, bob's)
    b.send(acct(Account48::CreateRank { name: "Scout".into() }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(denied_texts(&b.take()), ["st_dont_have_permission"]);
    a.send(acct(Account48::ClanPermission { rank: 3, value: 1, permission: 0 }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    let Some(Clan9A::Full { ranks, .. }) = clan_msgs(&b.take()).last().cloned() else { panic!() };
    assert_eq!(ranks[3].perms, 16);
    a.take();
    // now bob can invite carol
    b.send(acct(Account48::ClanInvite { player: public_id(c.account) }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(denied_texts(&b.take()).is_empty());
    // daily message: alice may (commander); it reaches both
    a.send(acct(Account48::DailyMessage { text: "Raid at dusk".into() }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    for who in [a.take(), b.take()] {
        assert!(clan_msgs(&who).iter().any(|m| matches!(m, Clan9A::Daily { text, .. } if text == "Raid at dusk")));
    }
    // opening the clan window (48 0F 00) changes nothing
    a.send(acct(Account48::ClanWindowOpened { a: 0 }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    a.take();
    assert!(e.store.clan_of(a.account).unwrap().is_some());
    // bob confirms "leave" (48 0E) and sees "no clan", alice a shorter list
    b.send(acct(Account48::LeaveClan));
    pump3(&mut e, &mut a, &mut b, &mut c);
    a.take();
    assert!(clan_msgs(&b.take()).iter().any(|m| matches!(m, Clan9A::NoClan)));
    assert_eq!(e.store.clan_abbr_of(b.account), "");
    // login push: a fresh sign-in of a clan member carries 9A with the clan (checked on the store side)
    assert!(e.store.clan_of(a.account).unwrap().is_some());
}

fn rules_exists() -> u8 {
    accounts::clans::R_EXISTS
}

#[test]
fn clan_pushes_at_sign_in_and_presence() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut a = e.client("alice");
    // a clan made directly in the store; the account signs in and gets 9A with it, and no clan for others
    let id = a.account;
    e.store.reserve_clan(id, "Night Watch", "NW").unwrap();
    e.store.mark_clan_registration_paid(id).unwrap();
    e.store.create_clan(id).unwrap();
    let mut b = e.client("bob");
    c_sign(&mut e, &mut b);
    let m = e.sign_in(&mut a);
    let cm = clan_msgs(&m);
    assert!(matches!(cm.as_slice(), [Clan9A::Full { name, .. }] if name == "Night Watch"), "{cm:?}");
    let _ = b;
}

fn c_sign(e: &mut Env, b: &mut Client) {
    let m = e.sign_in(b);
    assert!(matches!(clan_msgs(&m).as_slice(), [Clan9A::NoClan]), "{:?}", clan_msgs(&m));
}

#[test]
fn squad_chat_channel_8_reaches_squad_mates_only() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    let (id_a, id_b) = (public_id(a.account), public_id(b.account));
    a.send(Request::People { id: id_b, kind: 2, text: String::new() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    b.take();
    b.send(Request::People { id: id_a, kind: 3, text: String::new() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    a.send(Request::Squad(Squad49::Accepted { player: id_b }));
    pump3(&mut e, &mut a, &mut b, &mut c);
    a.take();
    b.take();
    c.take();
    // the squad id the AE push carries is the context id the clients send back
    let sid = e.server.registry.squads.squad_of(a.account).expect("squad").id;
    a.send(Request::Chat { channel: 8, target: u64::from(sid), text: "squad up".into() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(lines(&a.take()).is_empty());
    assert!(lines(&c.take()).is_empty());
    let l = lines(&b.take());
    assert_eq!((l.len(), l[0].channel, l[0].text.as_str()), (1, 8, "squad up"));
    // channel 2 is server only: a client line on it goes nowhere
    a.send(Request::Chat { channel: 2, target: 0, text: "fake system".into() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(lines(&b.take()).is_empty() && lines(&c.take()).is_empty());
}

#[test]
fn custom_match_room_lifecycle_and_channel_9() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    let (id_a, id_b) = (public_id(a.account), public_id(b.account));
    // a clan match needs a clan
    a.send(Request::Other { id: 0x62, payload: Custom62::Create { kind: 1 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(denied_texts(&a.take()), ["st_no_clan_info"]);
    a.send(Request::Other { id: 0x62, payload: Custom62::Create { kind: 0 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let r = rooms98(&a.take());
    let room = r.last().cloned().expect("98 after create");
    assert_ne!(room.room_id, 0);
    assert_eq!(room.members.len(), 1);
    assert!(room.members[0].commander);
    // the 98 team byte is the client's role: 0 team A, 1 team B, 2 unassigned (PROVEN)
    assert_eq!((room.members[0].team, room.members[0].id), (0, id_a));
    // the map / mode / weather are a row of the game data
    assert!(e.server.lobby.data.matches.modes().iter().any(|x| x.map == u32::from(room.map) && x.mode == u32::from(room.mode) && x.variation == u32::from(room.weather)));

    // invite bob (B1 type 8), he accepts: both see two members
    a.send(Request::Other { id: 0x62, payload: Custom62::Invite { target: id_b }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let inv = b.take().into_iter().find_map(|x| if let S2c::SystemMessage(s) = x { Some(s) } else { None }).expect("invite");
    assert_eq!((inv.kind, inv.sender), (8, id_a));
    // carol was not invited: her answer does nothing
    c.send(Request::Other { id: 0x62, payload: Custom62::Answer { inviter: id_a, accept: 0 }.encode() }); // 0 = accept
    b.send(Request::Other { id: 0x62, payload: Custom62::Answer { inviter: id_a, accept: 0 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(rooms98(&c.take()).is_empty());
    let (ra, rb) = (rooms98(&a.take()), rooms98(&b.take()));
    assert_eq!(ra.last().unwrap().members.len(), 2);
    assert_eq!(rb.last().unwrap().room_id, room.room_id);
    assert_eq!(rb.last().unwrap().members[1].team, 2);

    // bob joins team B: the wire byte is the role itself (the exe sends the button value - 1: A 0, B 1, ? 2)
    b.send(Request::Other { id: 0x62, payload: Custom62::MemberStatus { player: id_b, status: 1 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(rooms98(&a.take()).last().unwrap().members[1].team, 1);
    // the host's "?" puts himself in the reserve, "A" back on team A; a member cannot move the host, role 3 is no role
    a.send(Request::Other { id: 0x62, payload: Custom62::MemberStatus { player: id_a, status: 2 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(rooms98(&b.take()).last().unwrap().members[0].team, 2);
    a.send(Request::Other { id: 0x62, payload: Custom62::MemberStatus { player: id_a, status: 0 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(rooms98(&b.take()).last().unwrap().members[0].team, 0);
    b.send(Request::Other { id: 0x62, payload: Custom62::MemberStatus { player: id_a, status: 1 }.encode() });
    b.send(Request::Other { id: 0x62, payload: Custom62::MemberStatus { player: id_b, status: 3 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let m = b.take();
    assert_eq!(denied_texts(&m), ["st_cant_change_status_custom_match_error"; 2]);
    assert_eq!(rooms98(&m).last().map(|r| (r.members[0].team, r.members[1].team)), Some((0, 1)));
    a.take();
    let bad = Custom62::Parameters { map: 250, weather: 1, mode: 1, region: 0, voice: 1, ai_ignore: 0 };
    a.send(Request::Other { id: 0x62, payload: bad.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(denied_texts(&a.take()), ["st_cant_change_parameter_custom_match_error"]);
    let row = e.server.lobby.data.matches.modes().iter().find(|x| x.variation == 0 && (x.map as u8, x.mode as u8, x.variation as u8) != (room.map, room.mode, room.weather)).cloned();
    if let Some(row) = row {
        let ok = Custom62::Parameters { map: row.map as u8, weather: row.variation as u8, mode: row.mode as u8, region: 0, voice: 1, ai_ignore: 1 };
        b.send(Request::Other { id: 0x62, payload: ok.encode() });
        pump3(&mut e, &mut a, &mut b, &mut c);
        assert_ne!(rooms98(&a.take()).last().map(|r| (r.voice, r.ai_ignore)), Some((1, 1)), "a member is not the host");
        // the region is a 9E server id: this lobby lists no server, so only id 0 exists
        let far = Custom62::Parameters { map: row.map as u8, weather: row.variation as u8, mode: row.mode as u8, region: 3, voice: 1, ai_ignore: 1 };
        a.send(Request::Other { id: 0x62, payload: far.encode() });
        pump3(&mut e, &mut a, &mut b, &mut c);
        assert_eq!(denied_texts(&a.take()), ["st_cant_change_parameter_custom_match_error"]);
        b.take();
        a.send(Request::Other { id: 0x62, payload: ok.encode() });
        pump3(&mut e, &mut a, &mut b, &mut c);
        let r = rooms98(&b.take());
        let r = r.last().unwrap();
        assert_eq!((r.map, r.mode, r.weather, r.region, r.voice, r.ai_ignore), (row.map as u8, row.mode as u8, row.variation as u8, 0, 1, 1));
    }
    a.take();
    // the room's "weather" byte is the variation id (the client's custom_match_data lists st_variation_N_name, id N):
    // School TDM variation 4 is a row, Rudnya TDM has no variation 3
    let v4 = Custom62::Parameters { map: 2, weather: 4, mode: 1, region: 0, voice: 1, ai_ignore: 0 };
    a.send(Request::Other { id: 0x62, payload: v4.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let m = a.take();
    assert!(denied_texts(&m).is_empty(), "{m:?}");
    assert_eq!(rooms98(&m).last().map(|r| (r.map, r.mode, r.weather)), Some((2, 1, 4)));
    let v3 = Custom62::Parameters { map: 1, weather: 3, mode: 1, region: 0, voice: 1, ai_ignore: 0 };
    a.send(Request::Other { id: 0x62, payload: v3.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(denied_texts(&a.take()), ["st_cant_change_parameter_custom_match_error"]);
    b.take();

    // chat channel 9 with the room id: bob gets it, carol and the sender do not
    a.send(Request::Chat { channel: 9, target: u64::from(room.room_id), text: "ready?".into() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(lines(&a.take()).is_empty() && lines(&c.take()).is_empty());
    let l = lines(&b.take());
    assert_eq!((l.len(), l[0].channel, l[0].text.as_str()), (1, 9, "ready?"));

    // Start is refused honestly with a text, and no match starts
    a.send(Request::Other { id: 0x62, payload: Custom62::Start.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let m = a.take();
    assert_eq!(denied_texts(&m), ["st_cant_start_custom_match_error"]);

    // the host leaves: the room closes; he gets A0, bob A1 "aborted by the commander", and nobody an empty 98 (the
    // client would show an empty room)
    a.send(Request::Other { id: 0x62, payload: Custom62::Leave { player: id_a }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let (ma, mb) = (a.take(), b.take());
    assert_eq!(aborted(&mb), ["st_custom_match_aborted_by_commander"]);
    assert!(ma.iter().any(|x| matches!(x, S2c::Raw { id: 0xA0, payload } if payload.is_empty())), "{ma:?}");
    assert!(rooms98(&ma).is_empty() && rooms98(&mb).is_empty());
    assert!(e.server.registry.rooms.is_empty());
    // a leave without a room (the client still shows one) is answered with A0
    b.send(Request::Other { id: 0x62, payload: Custom62::Leave { player: id_b }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert!(b.take().iter().any(|x| matches!(x, S2c::Raw { id: 0xA0, .. })));
}

/// `A1` reasons in a batch of messages.
fn aborted(m: &[S2c]) -> Vec<String> {
    m.iter()
        .filter_map(|x| match x {
            S2c::Raw { id: 0xA1, payload } => protocol::wire::Reader::new(payload).str().ok(),
            _ => None,
        })
        .collect()
}

#[test]
fn a_member_leaves_with_a0_and_the_host_removes_one_with_a1() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    let (id_a, id_b, id_c) = (public_id(a.account), public_id(b.account), public_id(c.account));
    a.send(Request::Other { id: 0x62, payload: Custom62::Create { kind: 0 }.encode() });
    a.send(Request::Other { id: 0x62, payload: Custom62::Invite { target: id_b }.encode() });
    a.send(Request::Other { id: 0x62, payload: Custom62::Invite { target: id_c }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    b.send(Request::Other { id: 0x62, payload: Custom62::Answer { inviter: id_a, accept: 0 }.encode() });
    c.send(Request::Other { id: 0x62, payload: Custom62::Answer { inviter: id_a, accept: 0 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(rooms98(&a.take()).last().unwrap().members.len(), 3);
    b.take();
    c.take();
    // bob leaves himself (the mini panel's X): A0 for him, the others see two members
    b.send(Request::Other { id: 0x62, payload: Custom62::Leave { player: id_b }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    let mb = b.take();
    assert!(mb.iter().any(|x| matches!(x, S2c::Raw { id: 0xA0, .. })) && rooms98(&mb).is_empty(), "{mb:?}");
    assert_eq!(rooms98(&a.take()).last().unwrap().members.len(), 2);
    // carol cannot remove the host; the host removes carol: A1 "removed", the room stays
    c.take();
    c.send(Request::Other { id: 0x62, payload: Custom62::Leave { player: id_a }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(denied_texts(&c.take()), ["st_cant_kick_player_from_custom_match"]);
    a.send(Request::Other { id: 0x62, payload: Custom62::Leave { player: id_c }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(aborted(&c.take()), ["st_custom_match_kicked_from_match"]);
    assert_eq!(rooms98(&a.take()).last().unwrap().members.len(), 1);
    assert_eq!(e.server.registry.rooms.len(), 1);
}

#[test]
fn a_dropped_connection_leaves_the_room() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    let id_b = public_id(b.account);
    a.send(Request::Other { id: 0x62, payload: Custom62::Create { kind: 0 }.encode() });
    a.send(Request::Other { id: 0x62, payload: Custom62::Invite { target: id_b }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    b.send(Request::Other { id: 0x62, payload: Custom62::Answer { inviter: public_id(a.account), accept: 0 }.encode() });
    pump3(&mut e, &mut a, &mut b, &mut c);
    assert_eq!(e.server.registry.rooms.len(), 1);
    // bob goes silent (10 s timeout); alice and carol keep talking
    for _ in 0..60 {
        e.step(&mut [&mut a, &mut c], Duration::from_millis(250));
    }
    let r = rooms98(&a.take());
    assert_eq!(r.last().unwrap().members.len(), 1);
}

/// Clan experience from members' match results (game_data::clan: CHOICE table): the window bar is the stored experience.
#[test]
fn match_results_give_the_clan_experience_and_level_up_pushes_the_window() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    found_clan(&mut e, &mut a, &mut b, &mut c);
    let clan = e.store.clan_of(a.account).unwrap().unwrap();
    let mut facts = game_data::MatchFacts::local(4, 1, true, true, 1);
    let give = |e: &mut Env, who: accounts::AccountId, f: &game_data::MatchFacts| {
        let lobby_server::Server { lobby, registry, .. } = &mut e.server;
        let mut cx = lobby_server::dispatch::Ctx { lobby, registry, now: std::time::Instant::now(), fx: vec![], kick: vec![] };
        lobby_server::stats::record_match(&mut cx, who, f);
        // only the clan window pushes (a match also answers achievements and quests)
        cx.fx.into_iter().filter(|c| matches!(&c.msg, S2c::Raw { id: 0x9A, .. })).collect::<Vec<_>>()
    };
    // a local training match (no match type) pays nothing; neither does a non-member
    assert!(give(&mut e, a.account, &facts).is_empty());
    facts.match_type = Some(1);
    assert!(give(&mut e, c.account, &facts).is_empty(), "carol is in no clan");
    assert_eq!(e.store.load_clan(clan.id).unwrap().unwrap().xp, 0);
    // 49 wins = 980 experience: still level 1; the 50th win crosses 1000
    for _ in 0..49 {
        give(&mut e, a.account, &facts);
    }
    assert_eq!(e.store.load_clan(clan.id).unwrap().unwrap().xp, 980);
    let fx = give(&mut e, a.account, &facts);
    let to_a: Vec<S2c> = fx.into_iter().filter(|x| x.to == a.account).map(|x| x.msg).collect();
    let cm = clan_msgs(&to_a);
    let Some(Clan9A::Full { level, min, max, current, .. }) = cm.last().cloned() else { panic!("{cm:?}") };
    assert_eq!((level, min, max, current), (2, 0, 2000, 0), "1000 experience: level 2, bar restarts");
    // a loss pays 10, a leaver nothing
    facts.won = false;
    give(&mut e, a.account, &facts);
    facts.finished = false;
    give(&mut e, a.account, &facts);
    assert_eq!(e.store.load_clan(clan.id).unwrap().unwrap().xp, 1010);
}
