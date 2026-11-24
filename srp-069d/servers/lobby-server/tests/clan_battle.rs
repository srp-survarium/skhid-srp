//! Clan battles (custom match type 1) end to end with scripted clients: two clans, the room, the clan rules, the ready
//! toggle (`62 22`) and the ready check (`83 30`), Start through a fake match server, the clients' reports and the
//! clan experience. Evidence and choices: docs/notes/clan-battles.md.

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use accounts::public_id;
use game_data::GameData;
use lobby_server::{Ended, EndedPlayer, LobbyConfig, MinimalMatches, OnlineMatches, Seat, Ticket82};
use protocol::c2s::{Event53, Request};
use protocol::s2c::*;
use protocol::Decode;
use support::*;

const MATCH_ID: u32 = 0xC1A4_0001;

#[derive(Default)]
struct Fake {
    created: Mutex<Vec<Vec<(i64, u8)>>>,
    /// the options match-type byte of every create
    types: Mutex<Vec<u8>>,
    ended: Mutex<Vec<Ended>>,
}

impl OnlineMatches for Fake {
    fn available(&self, _: u32, _: u32) -> bool {
        true
    }
    fn create(&self, _: &GameData, _: u32, _: u32, _: u32, match_type: u8, seats: &[Seat]) -> Result<Ticket82, String> {
        self.created.lock().unwrap().push(seats.iter().map(|s| (s.account, s.team)).collect());
        self.types.lock().unwrap().push(match_type);
        Ok(Ticket82 { match_id: MATCH_ID, host: "match.example".into(), port: 1236, orders: (0..seats.len() as u32).map(|i| 100 + i).collect() })
    }
    fn take_ended(&self) -> Vec<Ended> {
        std::mem::take(&mut *self.ended.lock().unwrap())
    }
}

/// alice, bob (clan Iron: alice commander, bob member), carol, dave (clan Rust: carol commander), erin (clan Third),
/// frank (no clan).
const NAMES: [&str; 6] = ["alice", "bob", "carol", "dave", "erin", "frank"];
const A: usize = 0;
const B: usize = 1;
const C: usize = 2;
const D: usize = 3;
const E: usize = 4;
const F: usize = 5;

struct Rig {
    _dir: tempfile::TempDir,
    e: Env,
    fake: Arc<Fake>,
    c: Vec<Client>,
    clans: [u32; 3],
}

fn pump(r: &mut Rig) {
    for _ in 0..40 {
        let mut refs: Vec<&mut Client> = r.c.iter_mut().collect();
        r.e.step(&mut refs, Duration::from_millis(30));
    }
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig {
        achievement_rewards: false,
        daily_quests: false,
        treasure_hunt: false,
        online_report_grace: Duration::from_secs(5),
        ..LobbyConfig::default()
    };
    let mut e = Env::with(&dir.path().join("cb.sqlite"), cfg, Arc::new(MinimalMatches));
    let fake = Arc::new(Fake::default());
    e.server.lobby.online = Some(fake.clone());
    let mut c: Vec<Client> = vec![];
    for n in NAMES {
        let mut cl = e.client(n);
        cl.send(Request::SignIn { session: cl.session, a: 0xFF, b: 0xFF });
        for _ in 0..300 {
            let mut refs: Vec<&mut Client> = c.iter_mut().collect();
            refs.push(&mut cl);
            e.step(&mut refs, Duration::from_millis(30));
            if cl.inbox.iter().any(|(id, _)| *id == 0xB8) {
                break;
            }
        }
        c.push(cl);
        let mut refs: Vec<&mut Client> = c.iter_mut().collect();
        e.pump(&mut refs);
    }
    // the three clans are founded through the store (the lobby side of founding is tested in clans_rooms.rs)
    let found = |founder: usize, name: &str, abbr: &str, members: &[usize]| {
        let f = c[founder].account;
        e.store.reserve_clan(f, name, abbr).unwrap();
        e.store.mark_clan_registration_paid(f).unwrap();
        let id = e.store.create_clan(f).unwrap();
        for &m in members {
            e.store.clan_invite(f, c[m].account).unwrap();
            e.store.clan_accept(c[m].account, id).unwrap();
        }
        id
    };
    let clans = [found(A, "Iron Wolves", "IW", &[B]), found(C, "Rust Hawks", "RH", &[D]), found(E, "Third Order", "TO", &[])];
    for cl in c.iter_mut() {
        cl.take();
    }
    Rig { _dir: dir, e, fake, c, clans }
}

fn send62(r: &mut Rig, who: usize, m: Custom62) {
    r.c[who].send(Request::Other { id: 0x62, payload: m.encode() });
    pump(r);
}

fn texts(m: &[S2c]) -> Vec<String> {
    m.iter()
        .filter_map(|x| match x {
            S2c::Denied(Denied { body: DeniedBody::Text(t), .. }) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn rooms98(m: &[S2c]) -> Vec<Room98> {
    m.iter()
        .filter_map(|x| match x {
            S2c::Raw { id: 0x98, payload } => Some(Room98::from_bytes(payload).expect("98 decodes")),
            _ => None,
        })
        .collect()
}

/// The last 98 a client got (the host's view of the room).
fn view(r: &mut Rig, who: usize) -> Room98 {
    rooms98(&r.c[who].take()).pop().expect("a 98")
}

fn invite(r: &mut Rig, host: usize, target: usize, accept: bool) -> Vec<String> {
    send62(r, host, Custom62::Invite { target: public_id(r.c[target].account) });
    let mut t = texts(&r.c[host].take());
    if t.is_empty() {
        let inviter = public_id(r.c[host].account);
        send62(r, target, Custom62::Answer { inviter, accept: u8::from(!accept) } /* 0 = accept */);
        t = texts(&r.c[target].take());
    }
    t
}

/// `62 1D` with the role itself (0 team A, 1 team B, 2 reserve: the exe sends the button value - 1).
fn pick_team(r: &mut Rig, who: usize, status: u8) -> Vec<String> {
    let me = public_id(r.c[who].account);
    send62(r, who, Custom62::MemberStatus { player: me, status });
    texts(&r.c[who].take())
}

fn clan_xp(r: &Rig, i: usize) -> u32 {
    r.e.store.load_clan(r.clans[i]).unwrap().unwrap().xp
}

fn has_ready_check(m: &[S2c]) -> bool {
    m.iter().any(|x| matches!(x, S2c::Reply(Reply::Ready30)))
}

fn setup_room(r: &mut Rig) {
    // a row with room for two a side
    let row = r.e.server.lobby.data.matches.modes().iter().find(|x| x.variation == 0 && x.max_players >= 4 && x.map < 256 && x.mode < 256).cloned().expect("a row");
    send62(r, A, Custom62::Create { kind: 1 });
    send62(r, A, Custom62::Parameters { map: row.map as u8, weather: row.variation as u8, mode: row.mode as u8, region: 0, voice: 0, ai_ignore: 0 });
    send62(r, A, Custom62::PlayerCount { team: 0, count: 2 });
    send62(r, A, Custom62::PlayerCount { team: 1, count: 2 });
    for cl in r.c.iter_mut() {
        cl.take();
    }
}

#[test]
fn only_a_clan_member_with_the_battle_permission_hosts_a_clan_match() {
    game_data::require_tables!();
    let mut r = rig();
    // frank has no clan, bob is a plain member (the join rank soldier has no "battle commander" bit)
    send62(&mut r, F, Custom62::Create { kind: 1 });
    assert_eq!(texts(&r.c[F].take()), ["st_no_clan_info"]);
    send62(&mut r, B, Custom62::Create { kind: 1 });
    assert_eq!(texts(&r.c[B].take()), ["st_dont_have_permission"]);
    assert!(r.e.server.registry.rooms.is_empty());
    // the commander of the clan may; the room is a clan room: type 1, team A = his clan, team B still open
    send62(&mut r, A, Custom62::Create { kind: 1 });
    let v = view(&mut r, A);
    assert_eq!((v.kind, v.clan_a, v.clan_a_name.as_str(), v.clan_b, v.clan_b_name.as_str()), (1, r.clans[0], "Iron Wolves", 0, ""));
    assert_eq!((v.members.len(), v.members[0].team, v.members[0].commander, v.members[0].clan_id), (1, 0, true, r.clans[0]));
    // a training room shows no clan at all (the client reads the clan fields for type 1 only)
    let me = public_id(r.c[A].account);
    send62(&mut r, A, Custom62::Leave { player: me });
    r.c[A].take();
    send62(&mut r, A, Custom62::Create { kind: 0 });
    let v = view(&mut r, A);
    assert_eq!((v.kind, v.clan_a, v.clan_b), (0, 0, 0));
    // a clan rank given the permission may host (48 11 is tested in clans_rooms.rs; here the rank is set in the store)
    let me = public_id(r.c[A].account);
    send62(&mut r, A, Custom62::Leave { player: me });
    r.c[A].take();
    r.e.store.clan_set_perm(r.c[A].account, 3, 0, true).unwrap(); // rank 3 = soldier, the join rank
    send62(&mut r, B, Custom62::Create { kind: 1 });
    assert_eq!(view(&mut r, B).clan_a, r.clans[0]);
}

#[test]
fn invitations_follow_the_two_clan_rule_with_the_games_own_texts() {
    game_data::require_tables!();
    let mut r = rig();
    setup_room(&mut r);
    // no clan, then own clan, then the other clan
    assert_eq!(invite(&mut r, A, F, true), ["st_custom_match_error_player_not_in_clan"]);
    assert!(invite(&mut r, A, B, true).is_empty());
    assert!(invite(&mut r, A, C, true).is_empty());
    let v = view(&mut r, A);
    assert_eq!((v.members.len(), v.clan_a, v.clan_b, v.clan_b_name.as_str()), (3, r.clans[0], r.clans[1], "Rust Hawks"));
    // a third clan does not fit; the second clan's other member does
    assert_eq!(invite(&mut r, A, E, true), ["st_custom_match_error_player_clan_mismatch"]);
    assert!(invite(&mut r, A, D, true).is_empty());
    // somebody who is in a custom match already is busy
    let t = invite(&mut r, A, D, true);
    assert_eq!(t, ["st_custom_match_error_player_already_in_match"]);
    // when the second clan has left, the free side is open to a third clan
    for who in [C, D] {
        let me = public_id(r.c[who].account);
        send62(&mut r, who, Custom62::Leave { player: me });
    }
    r.c[A].take();
    assert!(invite(&mut r, A, E, true).is_empty());
    let v = view(&mut r, A);
    assert_eq!((v.clan_a, v.clan_b, v.clan_b_name.as_str()), (r.clans[0], r.clans[2], "Third Order"));
}

#[test]
fn a_squad_member_cannot_be_invited_and_a_room_member_cannot_squad_up_into_a_match() {
    game_data::require_tables!();
    let mut r = rig();
    // alice and bob form a squad: neither may host or be invited
    let (a, b) = (r.c[A].account, r.c[B].account);
    let sq = &mut r.e.server.registry.squads;
    sq.record_invite(a, b, 0);
    sq.accept_invite(a, b);
    sq.join(a, b).unwrap();
    send62(&mut r, A, Custom62::Create { kind: 0 });
    assert_eq!(texts(&r.c[A].take()), ["st_custom_match_error_player_in_squad"]);
    send62(&mut r, C, Custom62::Create { kind: 0 });
    r.c[C].take();
    assert_eq!(invite(&mut r, C, A, true), ["st_custom_match_error_player_in_squad"]);
}

#[test]
fn teams_are_the_clans_sides_and_ready_is_per_player() {
    game_data::require_tables!();
    let mut r = rig();
    setup_room(&mut r);
    for t in [B, C, D] {
        assert!(invite(&mut r, A, t, true).is_empty());
    }
    r.c[A].take();
    // the client offers the team buttons by clan; the server enforces the same: team A = Iron, team B = Rust
    assert_eq!(pick_team(&mut r, B, 1), ["st_custom_match_error_player_clan_mismatch"]);
    assert_eq!(pick_team(&mut r, C, 0), ["st_custom_match_error_player_clan_mismatch"]);
    assert!(pick_team(&mut r, B, 0).is_empty());
    assert!(pick_team(&mut r, C, 1).is_empty());
    assert!(pick_team(&mut r, D, 1).is_empty());
    let v = view(&mut r, A);
    let teams: Vec<u8> = v.members.iter().map(|m| m.team).collect();
    assert_eq!(teams, [0, 0, 1, 1], "98 teams are the client's roles: 0 A, 1 B");
    // ready: only the member's own toggle (62 22), shown to everybody, the host always counts
    let ready = |v: &Room98| v.members.iter().map(|m| m.ready).collect::<Vec<_>>();
    assert_eq!(ready(&v), [true, false, false, false]);
    send62(&mut r, C, Custom62::Ready { ready: 0xFF });
    let v = view(&mut r, A);
    assert_eq!(ready(&v), [true, false, true, false]);
    send62(&mut r, C, Custom62::Ready { ready: 0 });
    assert_eq!(ready(&view(&mut r, A)), [true, false, false, false]);
    // the host's own toggle and a toggle from outside the room change nothing
    send62(&mut r, A, Custom62::Ready { ready: 0 });
    send62(&mut r, F, Custom62::Ready { ready: 0xFF });
    let stored: Vec<bool> = r.e.server.registry.rooms.room_of(r.c[A].account).unwrap().members.iter().map(|m| m.ready).collect();
    assert_eq!(stored, [true, false, false, false]);
    r.c[A].take();
    // changing the side takes the ready back (CHOICE)
    send62(&mut r, D, Custom62::Ready { ready: 1 });
    r.c[A].take();
    assert!(pick_team(&mut r, D, 2).is_empty());
    assert!(pick_team(&mut r, D, 1).is_empty());
    assert_eq!(ready(&view(&mut r, A)), [true, false, false, false]);
    // no bots in a clan match
    send62(&mut r, A, Custom62::AiCount { team: 0, count: 1 });
    assert_eq!(texts(&r.c[A].take()), ["st_cant_change_ai_count_custom_match_error"]);
}

#[test]
fn a_clan_battle_starts_when_all_are_ready_and_pays_the_clan_experience() {
    game_data::require_tables!();
    let mut r = rig();
    setup_room(&mut r);
    for t in [B, C, D] {
        assert!(invite(&mut r, A, t, true).is_empty());
    }
    assert!(pick_team(&mut r, B, 0).is_empty());
    assert!(pick_team(&mut r, C, 1).is_empty());
    assert!(pick_team(&mut r, D, 1).is_empty());
    for c in r.c.iter_mut() {
        c.take();
    }

    // nobody is ready: Start is refused with the game's text and the ready check goes to the three unready members
    send62(&mut r, A, Custom62::Start);
    assert_eq!(texts(&r.c[A].take()), ["st_custom_match_error_not_all_members_ready"]);
    for who in [B, C, D] {
        assert!(has_ready_check(&r.c[who].take()), "83 30 for client {who}");
    }
    assert!(r.c[A].take().is_empty() && r.fake.created.lock().unwrap().is_empty());
    // the client answers 83 30 with 62 22 ready by itself (PROVEN); two of them did
    send62(&mut r, B, Custom62::Ready { ready: 0xFF });
    send62(&mut r, C, Custom62::Ready { ready: 0xFF });
    for c in r.c.iter_mut() {
        c.take();
    }
    send62(&mut r, A, Custom62::Start);
    assert_eq!(texts(&r.c[A].take()), ["st_custom_match_error_not_all_members_ready"]);
    for who in [B, C] {
        assert!(!has_ready_check(&r.c[who].take()), "ready players are not asked again");
    }
    assert!(has_ready_check(&r.c[D].take()));
    send62(&mut r, D, Custom62::Ready { ready: 0xFF });
    for c in r.c.iter_mut() {
        c.take();
    }
    // only the host starts
    send62(&mut r, B, Custom62::Start);
    assert!(r.fake.created.lock().unwrap().is_empty());
    // the cost: four players without premium need four tokens (5 000 silver or 20 gold each); none paid yet
    send62(&mut r, A, Custom62::Start);
    assert_eq!(texts(&r.c[A].take()), ["st_cant_start_custom_match_error"]);
    // the host pays all four in silver (anyone may pay for the others); the 98 shows it
    // the client sends one +1 silver delta per click (62 21 00 01)
    for _ in 0..4 {
        send62(&mut r, A, Custom62::Tokens { gold: 0, silver: 1 });
    }
    // a fifth click is refused: the room's tokens are complete
    send62(&mut r, A, Custom62::Tokens { gold: 0, silver: 1 });
    let v = view(&mut r, A);
    assert_eq!((v.tokens_allocated, v.tokens_required, v.cost_silver, v.cost_gold), (4, 4, 5000, 20));
    assert_eq!((v.members[0].tokens_silver, v.members[0].tokens_gold), (4, 0));
    for c in r.c.iter_mut() {
        c.take();
    }
    let host_before = r.e.store.load_profile(r.c[A].account).unwrap().silver;
    send62(&mut r, A, Custom62::Start);
    assert_eq!(r.e.store.load_profile(r.c[A].account).unwrap().silver, host_before - 4 * 5000, "charged when the match starts");
    let before: Vec<u32> = [A, B, C, D].iter().map(|&i| r.e.store.load_profile(r.c[i].account).unwrap().silver).collect();

    // one match: Iron on side 0, Rust on side 1; the room is gone
    let seats = r.fake.created.lock().unwrap().clone();
    assert_eq!(seats.len(), 1);
    let expect = vec![(r.c[A].account, 0), (r.c[B].account, 0), (r.c[C].account, 1), (r.c[D].account, 1)];
    assert_eq!(seats[0], expect);
    // a clan battle's options carry match type 6: no killcam, no hit / kill markers (official 0.65b)
    assert_eq!(*r.fake.types.lock().unwrap(), vec![protocol::s2c::online::MATCH_TYPE_CLAN]);
    assert!(r.e.server.registry.rooms.is_empty());
    let mut sides = vec![];
    for who in [A, B, C, D] {
        let m = r.c[who].take();
        let t = m
            .iter()
            .find_map(|x| match x {
                S2c::Raw { id: 0x82, payload } => ConnectToMatch::from_bytes(payload).ok(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("client {who} got no 82: {m:?}"));
        sides.push(t.side);
    }
    assert_eq!(sides, [0, 0, 1, 1]);

    // Iron won: its players report 5, Rust reports 6; the match server says it ended
    for (who, ev) in [(A, 5u16), (B, 5), (C, 6), (D, 6)] {
        r.c[who].send(Request::Events(vec![Event53 { id: ev, x: 0, value: MATCH_ID, time: 0 }]));
        pump(&mut r);
    }
    let players = expect.iter().map(|&(account, team)| EndedPlayer { account, team, entered: true, left_by_menu: false }).collect();
    r.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: None });
    pump(&mut r);

    // the clan experience: 20 per winning member, 10 per losing member (CHOICE values, game_data::clan::match_xp)
    assert_eq!(clan_xp(&r, 0), 2 * 20);
    assert_eq!(clan_xp(&r, 1), 2 * 10);
    assert_eq!(clan_xp(&r, 2), 0);
    // the rewards of a finished match are paid like any online match (the data's silver roll): LIKELY, the 0.65b post
    // says clan battles give rewards without naming them
    let gd = game_data::testing::tables();
    let row = gd.drops.money.iter().find(|x| x.match_level == lobby_server::online::ONLINE_MATCH_LEVEL).unwrap();
    for (n, &i) in [A, B, C, D].iter().enumerate() {
        let got = r.e.store.load_profile(r.c[i].account).unwrap().silver - before[n];
        assert!((row.min..=row.max).contains(&got), "client {i}: {got}");
        let m = r.c[i].take();
        let ids: Vec<u8> = m.iter().map(|x| x.id()).collect();
        // 85 00 ends the reward messages; the clan window refresh (9A) follows it
        assert!(ids.contains(&0x85) && ids.contains(&0x8B), "rewards and 85 00: {ids:02x?}");
    }
    assert!(r.e.server.registry.online.is_empty());
}

#[test]
fn start_needs_two_different_clans_on_their_own_sides() {
    game_data::require_tables!();
    let mut r = rig();
    setup_room(&mut r);
    assert!(invite(&mut r, A, B, true).is_empty());
    assert!(pick_team(&mut r, B, 0).is_empty());
    send62(&mut r, B, Custom62::Ready { ready: 1 });
    for c in r.c.iter_mut() {
        c.take();
    }
    // a team B that is empty
    send62(&mut r, A, Custom62::Start);
    assert_eq!(texts(&r.c[A].take()), ["st_custom_match_error_team_empty"]);
    // the other clan's player cannot take a side of Iron
    assert!(invite(&mut r, A, C, true).is_empty());
    assert_eq!(pick_team(&mut r, C, 0), ["st_custom_match_error_player_clan_mismatch"]);
    // swapping the sides swaps the clans with them: Rust is team A afterwards
    assert!(pick_team(&mut r, C, 1).is_empty());
    send62(&mut r, C, Custom62::Ready { ready: 1 });
    send62(&mut r, A, Custom62::SwapTeams);
    let v = view(&mut r, A);
    assert_eq!((v.clan_a, v.clan_b), (r.clans[1], r.clans[0]));
    let teams: Vec<(u8, u32)> = v.members.iter().map(|m| (m.team, m.clan_id)).collect();
    assert_eq!(teams, [(1, r.clans[0]), (1, r.clans[0]), (0, r.clans[1])]);
    // a member who left his clan since cannot play: the clan rule is checked again at Start
    r.e.store.clan_leave(r.c[B].account).unwrap();
    for c in r.c.iter_mut() {
        c.take();
    }
    send62(&mut r, A, Custom62::Start);
    assert_eq!(texts(&r.c[A].take()), ["st_custom_match_error_player_not_in_clan"]);
    assert!(r.fake.created.lock().unwrap().is_empty());
}
