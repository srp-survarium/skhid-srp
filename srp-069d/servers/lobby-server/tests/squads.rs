//! Squads with scripted clients: invite / accept / AE for 2 and 3 members, the 4th refused, leave / kick / disband,
//! commander handover, ready toggles, 33 -> 88, disconnect cleanup.

mod support;

use accounts::public_id;
use protocol::c2s::{Request, Squad49};
use protocol::s2c::*;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

fn up(e: &mut Env, name: &str) -> Client {
    let mut c = e.client(name);
    e.sign_in(&mut c);
    c
}

fn pid(c: &Client) -> u64 {
    public_id(c.account)
}

fn last_ae(msgs: &[S2c]) -> Option<SquadUpdate> {
    msgs.iter().rev().find_map(|m| if let S2c::Squad(s) = m { Some(s.clone()) } else { None })
}

fn count_ae(msgs: &[S2c]) -> usize {
    msgs.iter().filter(|m| matches!(m, S2c::Squad(_))).count()
}

/// `who` invites `target`; the target accepts; the inviter's client sends 49 02 (as the real client does on B1 kind 3).
fn join(e: &mut Env, who: &mut Client, target: &mut Client, rest: &mut [&mut Client]) {
    let (pw, pt) = (pid(who), pid(target));
    let mut all: Vec<&mut Client> = vec![&mut *who, &mut *target];
    all.extend(rest.iter_mut().map(|c| &mut **c));
    all[0].send(Request::People { id: pt, kind: 2, text: String::new() });
    e.pump(&mut all);
    let inv = all[1].take();
    let m = inv.iter().find_map(|m| if let S2c::SystemMessage(s) = m { Some(s.clone()) } else { None }).expect("B1 invite");
    assert_eq!((m.kind, m.sender), (2, pw));
    all[1].send(Request::People { id: pw, kind: 3, text: String::new() });
    e.pump(&mut all);
    let acc = all[0].take();
    let m = acc.iter().find_map(|m| if let S2c::SystemMessage(s) = m { Some(s.clone()) } else { None }).expect("B1 accept");
    assert_eq!((m.kind, m.sender), (3, pt));
    all[0].send(Request::Squad(Squad49::Accepted { player: m.sender }));
    e.pump(&mut all);
}

fn names(s: &Squad) -> Vec<String> {
    s.members.iter().map(|m| m.name.clone()).collect()
}

#[test]
fn invite_accept_two_and_three() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b, mut c) = (up(&mut e, "alpha"), up(&mut e, "bravo"), up(&mut e, "charlie"));
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    let sa = last_ae(&a.take()).unwrap().squad.unwrap();
    let sb = last_ae(&b.take()).unwrap().squad.unwrap();
    assert_eq!(sa.id, sb.id);
    assert_ne!(sa.id, 0);
    assert_eq!(names(&sa), ["alpha", "bravo"]);
    assert_eq!((sa.commander, sa.i_am_commander, sb.i_am_commander), (0, 1, 0));
    assert_eq!(sa.members[0].account_id, pid(&a));
    assert_eq!(sa.members[1].state, 0);
    assert!(sa.members.iter().all(|m| m.online == 1));
    assert_ne!(sa.members[0].profile_id, sa.members[1].profile_id);
    // third member
    join(&mut e, &mut a, &mut c, &mut [&mut b]);
    let sc = last_ae(&c.take()).unwrap().squad.unwrap();
    assert_eq!(names(&sc), ["alpha", "bravo", "charlie"]);
    assert_eq!(sc.i_am_commander, 0);
    let sb = last_ae(&b.take()).unwrap().squad.unwrap();
    assert_eq!(sb.members.len(), 3);
    assert_eq!(last_ae(&a.take()).unwrap().squad.unwrap().i_am_commander, 1);
}

#[test]
fn fourth_member_refused_and_other_refusals() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b, mut c, mut d) = (up(&mut e, "alpha"), up(&mut e, "bravo"), up(&mut e, "charlie"), up(&mut e, "delta"));
    join(&mut e, &mut a, &mut b, &mut [&mut c, &mut d]);
    join(&mut e, &mut a, &mut c, &mut [&mut b, &mut d]);
    for x in [&mut a, &mut b, &mut c, &mut d] {
        x.take();
    }
    a.send(Request::People { id: pid(&d), kind: 2, text: String::new() });
    e.pump(&mut [&mut a, &mut b, &mut c, &mut d]);
    assert!(!d.ids().contains(&0xB1), "no invite for a full squad");
    // a non-commander cannot invite
    b.send(Request::People { id: pid(&d), kind: 2, text: String::new() });
    // an offline target gets nothing and nothing crashes
    a.send(Request::People { id: public_id(9999), kind: 2, text: String::new() });
    // a forged 49 02 without an invite changes nothing
    a.send(Request::Squad(Squad49::Accepted { player: pid(&d) }));
    e.pump(&mut [&mut a, &mut b, &mut c, &mut d]);
    assert!(!d.ids().contains(&0xB1));
    assert_eq!(last_ae(&a.take()).unwrap().squad.unwrap().members.len(), 3);
    assert!(last_ae(&d.take()).is_none());
    // a member of a squad cannot be invited by someone else
    d.send(Request::People { id: pid(&b), kind: 2, text: String::new() });
    e.pump(&mut [&mut a, &mut b, &mut c, &mut d]);
    assert!(!b.ids().contains(&0xB1));
}

#[test]
fn leave_kick_disband_and_handover() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b, mut c) = (up(&mut e, "alpha"), up(&mut e, "bravo"), up(&mut e, "charlie"));
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    join(&mut e, &mut a, &mut c, &mut [&mut b]);
    for x in [&mut a, &mut b, &mut c] {
        x.take();
    }
    // a member leaves: he gets squad 0, the others the shorter squad
    b.send(Request::Squad(Squad49::Leave));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&b.take()).unwrap().squad, None);
    assert_eq!(names(&last_ae(&a.take()).unwrap().squad.unwrap()), ["alpha", "charlie"]);
    assert_eq!(names(&last_ae(&c.take()).unwrap().squad.unwrap()), ["alpha", "charlie"]);
    // the commander leaves: the next member commands, a squad of one dissolves
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    for x in [&mut a, &mut b, &mut c] {
        x.take();
    }
    a.send(Request::Squad(Squad49::Leave));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&a.take()).unwrap().squad, None);
    let sc = last_ae(&c.take()).unwrap().squad.unwrap();
    assert_eq!(names(&sc), ["charlie", "bravo"]);
    assert_eq!(sc.i_am_commander, 1);
    assert_eq!(last_ae(&b.take()).unwrap().squad.unwrap().i_am_commander, 0);
    // a member cannot kick; the commander can
    b.send(Request::Squad(Squad49::Kick { player: pid(&c) }));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&c.take()), None, "a refused kick tells the others nothing");
    assert_eq!(last_ae(&b.take()).unwrap().squad.unwrap().members.len(), 2);
    c.send(Request::Squad(Squad49::Kick { player: pid(&b) }));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&b.take()).unwrap().squad, None);
    assert_eq!(last_ae(&c.take()).unwrap().squad, None, "a squad of one dissolves");
    // disband by the commander
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    join(&mut e, &mut a, &mut c, &mut [&mut b]);
    for x in [&mut a, &mut b, &mut c] {
        x.take();
    }
    b.send(Request::Squad(Squad49::Disband));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(count_ae(&a.take()) + count_ae(&c.take()), 0, "not the commander: nobody else is told");
    a.send(Request::Squad(Squad49::Disband));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    for x in [&mut a, &mut b, &mut c] {
        assert_eq!(last_ae(&x.take()).unwrap().squad, None);
    }
}

#[test]
fn ready_toggles_and_squad_queue() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b, mut c) = (up(&mut e, "alpha"), up(&mut e, "bravo"), up(&mut e, "charlie"));
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    join(&mut e, &mut a, &mut c, &mut [&mut b]);
    for x in [&mut a, &mut b, &mut c] {
        x.take();
    }
    let q = |mode| Request::Queue { global: u32::MAX, arg: mode, mode: 0x380, flag: 0 };
    b.send(q(0));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    let mb = b.take();
    assert!(mb.iter().any(|m| matches!(m, S2c::QueueState(QueueState { state: 0 }))));
    let sa = last_ae(&a.take()).unwrap().squad.unwrap();
    assert_eq!(sa.members.iter().map(|m| m.state).collect::<Vec<_>>(), [0, 1, 0]);
    assert_eq!(last_ae(&mb).unwrap().squad.unwrap().members[1].state, 1);
    // the commander with a member not ready: refused, nothing changes
    a.send(q(0));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(a.take().iter().any(|m| matches!(m, S2c::QueueState(_))));
    assert_eq!(last_ae(&b.take()), None);
    // unready (31) and ready again
    b.send(Request::QueueCancel { order: 0 });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&a.take()).unwrap().squad.unwrap().members[1].state, 0);
    b.send(q(0));
    c.send(q(0));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&a.take()).unwrap().squad.unwrap().members.iter().map(|m| m.state).collect::<Vec<_>>(), [0, 1, 1]);
    // rated is refused for a member
    c.send(Request::QueueCancel { order: 0 });
    c.send(q(2));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&a.take()).unwrap().squad.unwrap().members[2].state, 0);
    // all ready: the commander's 2F is the squad queue (one ticket for the 3 players: 83 2F and 85 01 for each member,
    // too few players for a match so they keep waiting), members are set back to not ready
    c.send(q(0));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    a.take();
    b.take();
    c.take();
    a.send(q(0));
    e.pump(&mut [&mut a, &mut b, &mut c]);
    let ma = a.take();
    let searching = |m: &S2c| matches!(m, S2c::Raw { id: 0x85, payload } if payload[0] == 1);
    assert!(ma.iter().any(|m| matches!(m, S2c::Reply(Reply::Queue { .. }))));
    assert!(ma.iter().any(searching));
    assert!(b.take().iter().any(searching) && c.take().iter().any(searching), "every member searches");
    // a member leaving the search ends it for all (85 00 to each)
    b.send(Request::QueueCancel { order: 0 });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(a.take().iter().any(|m| matches!(m, S2c::QueueState(QueueState { state: 0 }))));
    assert_eq!(last_ae(&ma).unwrap().squad.unwrap().members.iter().map(|m| m.state).collect::<Vec<_>>(), [0, 0, 0]);
}

#[test]
fn leaving_the_squad_ends_its_pvp_search() {
    game_data::require_tables!();
    // the queue ticket of a squad names its members: a member who leaves must not stay in the old squad's search
    let (_d, mut e) = env();
    let (mut a, mut b) = (up(&mut e, "alpha"), up(&mut e, "bravo"));
    join(&mut e, &mut a, &mut b, &mut []);
    let q = Request::Queue { global: u32::MAX, arg: 0, mode: 0x380, flag: 0 };
    b.send(q.clone());
    e.pump(&mut [&mut a, &mut b]);
    a.send(q);
    e.pump(&mut [&mut a, &mut b]);
    assert!(e.server.registry.queue.ticket_of(b.account).is_some(), "the squad searches");
    a.take();
    b.take();
    b.send(Request::Squad(Squad49::Leave));
    e.pump(&mut [&mut a, &mut b]);
    assert!(e.server.registry.queue.is_empty(), "the old squad's search ended");
    assert!(a.take().iter().any(|m| matches!(m, S2c::QueueState(QueueState { state: 0 }))), "the commander's Play comes back");
    assert!(b.take().iter().any(|m| matches!(m, S2c::QueueState(QueueState { state: 0 }))));
}

#[test]
fn member_profile_33_gives_88() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b, mut c) = (up(&mut e, "alpha"), up(&mut e, "bravo"), up(&mut e, "charlie"));
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    let pb = last_ae(&a.take()).unwrap().squad.unwrap().members[1].profile_id;
    b.take();
    a.send(Request::MemberProfile { a: 0, member: pid(&b), profile: pb });
    // not a mate: ignored
    a.send(Request::MemberProfile { a: 0, member: pid(&c), profile: 1 });
    e.pump(&mut [&mut a, &mut b, &mut c]);
    let m = a.take();
    let all88: Vec<_> = m.iter().filter_map(|x| if let S2c::MemberProfile(p) = x { Some(p) } else { None }).collect();
    assert_eq!(all88.len(), 1);
    // the trailing u32 is the mate's profile version (the AE profile_version), not the profile id
    let pv = lobby_server::build::wire_version(&e.server.lobby.data, &e.store.load_profile(b.account).unwrap(), "bravo");
    assert_eq!(all88[0].version, pv);
    assert_eq!(all88[0].body.name, "bravo");
    assert_eq!(all88[0].body.profiles[0].id, pb);
}

#[test]
fn disconnect_cleans_up() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let (mut a, mut b, mut c) = (up(&mut e, "alpha"), up(&mut e, "bravo"), up(&mut e, "charlie"));
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    join(&mut e, &mut a, &mut c, &mut [&mut b]);
    for x in [&mut a, &mut b, &mut c] {
        x.take();
    }
    // c goes silent
    e.wait(&mut [&mut a, &mut b], 120);
    let sa = last_ae(&a.take()).expect("AE after the disconnect").squad.unwrap();
    assert_eq!(names(&sa), ["alpha", "bravo"]);
    // the commander goes silent: the last member's squad dissolves
    e.wait(&mut [&mut b], 120);
    assert_eq!(last_ae(&b.take()).unwrap().squad, None);
}

#[test]
fn a_member_joining_during_the_search_ends_it() {
    game_data::require_tables!();
    // the ticket names the members it was made for: a joiner is not in it, so the search ends (CHOICE, notes/social.md)
    let (_d, mut e) = env();
    let (mut a, mut b, mut c) = (up(&mut e, "alpha"), up(&mut e, "bravo"), up(&mut e, "charlie"));
    join(&mut e, &mut a, &mut b, &mut [&mut c]);
    let q = Request::Queue { global: u32::MAX, arg: 0, mode: 0x380, flag: 0 };
    b.send(q.clone());
    e.pump(&mut [&mut a, &mut b, &mut c]);
    a.send(q);
    e.pump(&mut [&mut a, &mut b, &mut c]);
    assert!(e.server.registry.queue.ticket_of(b.account).is_some(), "the squad searches");
    a.take();
    b.take();
    join(&mut e, &mut a, &mut c, &mut [&mut b]);
    assert!(e.server.registry.queue.is_empty(), "the search ended when the third member joined");
    assert!(a.take().iter().any(|m| matches!(m, S2c::QueueState(QueueState { state: 0 }))), "the commander's Play comes back");
    assert!(b.take().iter().any(|m| matches!(m, S2c::QueueState(QueueState { state: 0 }))));
}
