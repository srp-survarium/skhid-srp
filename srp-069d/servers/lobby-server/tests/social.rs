//! Social relay across peers: chat (general, private, ignore), friend flow, presence (B0 07).

mod support;

use std::time::Duration;

use accounts::public_id;
use protocol::c2s::{Event53, Friends4E, Play5B, Request};
use protocol::s2c::*;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

/// Sign `c` in while the others keep stepping (so they receive what the sign-in causes).
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

fn lines(m: &[S2c]) -> Vec<ChatLine> {
    m.iter().filter_map(|x| if let S2c::Chat(c) = x { Some(c.lines.clone()) } else { None }).flatten().collect()
}

fn statuses(m: &[S2c]) -> Vec<Vec<(u64, u8)>> {
    m.iter().filter_map(|x| if let S2c::Messaging(Messaging::Status(v)) = x { Some(v.clone()) } else { None }).collect()
}

/// The friend requests (B1 type 0) among the messages.
fn requests(m: &[S2c]) -> Vec<SystemMessage> {
    m.iter().filter_map(|x| if let S2c::SystemMessage(s) = x { (s.kind == 0).then(|| s.clone()) } else { None }).collect()
}

fn friend_list(m: &[S2c]) -> Option<Vec<(u64, u8)>> {
    m.iter().rev().find_map(|x| match x {
        S2c::Messaging(Messaging::Friends(v)) => Some(v.iter().map(|p| (p.id, p.status)).collect()),
        _ => None,
    })
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

#[test]
fn chat_general_private_and_ignore() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    let id_a = public_id(a.account);
    // general: everybody but the sender
    a.send(Request::Chat { channel: 1, target: 0, text: "hi all".into() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(lines(&a.take()).is_empty());
    for m in [b.take(), c.take()] {
        let l = lines(&m);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].kind, l[0].sender, l[0].channel, l[0].name.as_str(), l[0].text.as_str()), (5, id_a, 1, "alice", "hi all"));
    }
    // bob ignores alice: carol still gets the line, bob does not
    b.send(Request::Friends(Friends4E::AddIgnore { id: id_a }));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    b.take();
    a.send(Request::Chat { channel: 1, target: 0, text: "again".into() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(lines(&b.take()).is_empty());
    assert_eq!(lines(&c.take())[0].text, "again");
    // private to carol (target = account id): only carol
    a.send(Request::Chat { channel: 4, target: accounts::public_id(c.account), text: "psst".into() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(lines(&a.take()).is_empty() && lines(&b.take()).is_empty());
    let l = lines(&c.take());
    assert_eq!((l[0].channel, l[0].text.as_str(), l[0].sender), (4, "psst", id_a));
    // private to the one who ignores alice: dropped; un-ignore lets it through
    a.send(Request::Chat { channel: 4, target: accounts::public_id(b.account), text: "blocked".into() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(lines(&b.take()).is_empty());
    b.send(Request::Friends(Friends4E::RemoveIgnored { id: id_a }));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    a.send(Request::Chat { channel: 4, target: accounts::public_id(b.account), text: "open".into() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(lines(&b.take())[0].text, "open");
    // private to an offline account: the lobby's 84 4C with the game's own "Offline" (AF lines are not localized)
    let d = e.store.create_account("dave", "pw").unwrap();
    a.send(Request::Chat { channel: 4, target: accounts::public_id(d), text: "anyone?".into() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    let m = a.take();
    assert!(lines(&m).is_empty());
    assert!(m.contains(&S2c::Denied(Denied { request: 0x4C, body: DeniedBody::Text("st_friend_state_offline".into()) })), "{m:?}");
}

#[test]
fn friend_request_accept_remove_push_both_lists() {
    game_data::require_tables!();
    let (_d, mut e, mut a, mut b, mut c) = three();
    let (id_a, id_b) = (public_id(a.account), public_id(b.account));
    a.send(Request::People { id: id_b, kind: 0, text: String::new() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    let (ma, mb) = (a.take(), b.take());
    assert_eq!(friend_list(&ma), Some(vec![]));
    // the receiver: no status-4 row, the client's own Requests entry (B1 type 0: sender, name, empty text)
    assert_eq!(friend_list(&mb), Some(vec![]));
    let r = requests(&mb);
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].kind, r[0].sender, r[0].name.as_str(), r[0].text.as_str()), (0, id_a, "alice", ""));
    assert!(lines(&mb).is_empty(), "no English chat line");
    // Add = 51 id 0: both are friends
    b.send(Request::ReadMessage { id: r[0].message_id, a: 0 });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(friend_list(&a.take()), Some(vec![(id_b, 0)]));
    assert_eq!(friend_list(&b.take()), Some(vec![(id_a, 0)]));
    // Cancel = 51 id 1: carol asks, alice declines
    let id_c = public_id(c.account);
    c.send(Request::People { id: id_a, kind: 0, text: String::new() });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    let ma = a.take();
    assert_eq!(friend_list(&ma), Some(vec![(id_b, 0)]));
    let r = requests(&ma);
    assert_eq!(r[0].sender, id_c);
    a.send(Request::ReadMessage { id: r[0].message_id, a: 1 });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(friend_list(&a.take()), Some(vec![(id_b, 0)]));
    assert_eq!(friend_list(&c.take()), Some(vec![]));
    assert!(e.store.friends(a.account).unwrap().iter().all(|f| f.id != c.account), "the request is gone");
    // a stale answer (no pending request) changes nothing
    a.send(Request::ReadMessage { id: r[0].message_id, a: 0 });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(e.store.friends(a.account).unwrap().iter().all(|f| f.id != c.account));
    // remove: both lists
    b.send(Request::Friends(Friends4E::Remove { id: id_a }));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(friend_list(&a.take()), Some(vec![]));
    assert_eq!(friend_list(&b.take()), Some(vec![]));
}

#[test]
fn a_pending_friend_request_is_delivered_again_at_sign_in() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b) = (e.client("alice"), e.client("bob"));
    e.store.friend_request(a.account, b.account).unwrap();
    let m = e.sign_in(&mut b);
    let r = requests(&m);
    assert_eq!((r.len(), r[0].sender), (1, public_id(a.account)));
    // an ignored requester's request is not shown
    e.store.ignore_add(b.account, a.account).unwrap();
    let mut b2 = e.client_of("bob");
    assert!(requests(&e.sign_in(&mut b2)).is_empty());
    let _ = &mut a;
}

#[test]
fn presence_signin_match_and_signout() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b) = (e.client("alice"), e.client("bob"));
    let id_b = public_id(b.account);
    e.store.friend_request(a.account, b.account).unwrap();
    e.store.friend_request(b.account, a.account).unwrap();
    sign_in_with(&mut e, &mut a, &mut []);
    // bob signs in: alice sees him in the lobby
    b.send(Request::SignIn { session: b.session, a: 0xFF, b: 0xFF });
    for _ in 0..300 {
        e.step(&mut [&mut a, &mut b], Duration::from_millis(30));
    }
    e.pump(&mut [&mut a, &mut b]);
    assert_eq!(statuses(&a.take()), vec![vec![(id_b, 0)]]);
    b.take();
    e.handshake(&mut b);
    // bob starts Training (85 02): in game; the match ends 31 s later: lobby again
    b.send(Request::Play(Play5B::Play { map: 1, mode: 1, weather: 0 }));
    e.pump(&mut [&mut a, &mut b]);
    assert_eq!(statuses(&a.take()), vec![vec![(id_b, 1)]]);
    e.wait(&mut [&mut a, &mut b], 31);
    b.send(Request::Events(vec![Event53 { id: 6, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut a, &mut b]);
    assert_eq!(statuses(&a.take()).last(), Some(&vec![(id_b, 0)]));
    // bob goes silent: after the timeout alice sees him offline
    e.wait(&mut [&mut a], 15);
    assert_eq!(statuses(&a.take()).last(), Some(&vec![(id_b, 2)]));
}

#[test]
fn replaced_connection_is_not_offline() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b) = (e.client("alice"), e.client("bob"));
    e.store.friend_request(a.account, b.account).unwrap();
    e.store.friend_request(b.account, a.account).unwrap();
    sign_in_with(&mut e, &mut a, &mut []);
    sign_in_with(&mut e, &mut b, &mut [&mut a]);
    a.take();
    let mut b2 = e.client_of("bob");
    sign_in_with(&mut e, &mut b2, &mut [&mut a]);
    assert!(statuses(&a.take()).iter().all(|s| s.iter().all(|(_, st)| *st != 2)));
}
