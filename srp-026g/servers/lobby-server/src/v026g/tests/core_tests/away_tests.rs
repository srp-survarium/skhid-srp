//! A client that goes silent (`v026g::away`): when its link goes, how long
//! its session waits, and how it comes back.

use super::*;

impl World {
    /// The client's logic thread is held up for `time`: it sends nothing
    /// (its network thread still takes what the lobby sends).
    pub(super) fn silent_for(&mut self, index: usize, time: Duration) {
        let rounds = time.as_millis() / ROUND.as_millis();
        for _ in 0..rounds {
            self.now += ROUND;
            for (at, (addr, client)) in self.clients.iter_mut().enumerate() {
                if at == index {
                    continue;
                }
                for datagram in client.poll(self.now) {
                    self.core.datagram(*addr, &datagram, self.now);
                }
            }
            for (to, datagram) in self.core.step(self.now) {
                if let Some((_, client)) = self.clients.iter_mut().find(|(addr, _)| *addr == to) {
                    client.receive(&datagram);
                }
            }
        }
    }

    /// The client gave up on its link and connects again with its session,
    /// from a new port, as a new connection.
    fn connect_again(&mut self, index: usize, port: u16) {
        let session_id = self.ids[index].1;
        self.clients[index] = (
            format!("127.0.0.1:{port}").parse().unwrap(),
            TestClient::new(),
        );
        self.client(index).sign_in(session_id, false);
        self.settle();
    }

    /// The match (the queue's id) the lobby has the client in.
    fn in_match(&self, index: usize) -> Option<u32> {
        self.core
            .player(self.clients[index].0)
            .and_then(|player| player.in_match)
    }

    pub(super) fn has_link(&self, index: usize) -> bool {
        self.core.player(self.clients[index].0).is_some()
    }
}

#[test]
fn a_client_that_only_sends_keep_alives_is_never_dropped() {
    // Out of focus, or idle in the lobby: the client's link still sends
    // its `FC` (transport.md §6.1).
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.advance(Duration::from_secs(300));
    assert!(world.has_link(alice));
    assert!(world.core.away.is_empty());
    assert!(!world.store.has_ended(world.ids[alice].1));
    world.assert_clean();
}

#[test]
fn a_client_silent_for_less_than_the_timeout_keeps_its_link() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.ask(alice, request::play(1));
    world.take(alice);
    world.silent_for(alice, Duration::from_secs(25));
    assert!(world.has_link(alice));
    // And it is still in its room, and still served.
    assert!(world.core.rooms.of(world.ids[alice].0).is_some());
    world.advance(Duration::from_secs(3));
    world.ask(alice, request::autobuy(true));
    assert!(
        world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::Permitted(Permitted::Autobuy(true))))
    );
    assert_eq!(world.client(alice).faults, Vec::<String>::new());
    assert_eq!(world.client(alice).waiting_in_order(), 0);
}

#[test]
fn a_silent_client_is_out_of_the_lobby_first_and_its_session_ends_after_the_wait() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.ask(alice, request::play(1));
    let (account_id, session_id) = world.ids[alice];
    world.silent_for(alice, Duration::from_secs(31));
    // Step 1: no link, not in the queue, not online for the others.
    assert!(!world.has_link(alice));
    assert!(!world.core.by_account.contains_key(&account_id));
    assert!(world.store.matchmaking().busy_sessions().is_empty());
    assert_eq!(world.store.players_online(), 1);
    assert_eq!(world.client(bob).online.last(), Some(&(1, 0, 1440)));
    // Step 2: the session waits.
    assert_eq!(world.core.away[&session_id].in_match, None);
    assert!(!world.store.has_ended(session_id));
    world.silent_for(alice, Duration::from_secs(58));
    assert!(!world.store.has_ended(session_id));
    // Step 3: a minute after the link went.
    world.silent_for(alice, Duration::from_secs(3));
    assert!(world.store.has_ended(session_id));
    assert!(world.store.hold(session_id).is_none());
    assert!(world.core.away.is_empty());
    // A sign-in with it is refused now.
    world.connect_again(alice, 49001);
    assert!(!world.client(alice).model.is_signed_in());
    assert!(world.client(alice).disconnected);
}

#[test]
fn a_silent_client_that_connects_again_in_time_is_taken_back() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let session_id = world.ids[alice].1;
    world.silent_for(alice, Duration::from_secs(45));
    assert!(!world.has_link(alice));
    // Its own link gave up meanwhile; it connects again with its session.
    world.connect_again(alice, 49002);
    assert!(world.client(alice).model.is_signed_in());
    assert!(world.has_link(alice));
    assert!(world.core.away.is_empty());
    // The wait is over for good: the session lives on with the link.
    world.advance(Duration::from_secs(120));
    assert!(!world.store.has_ended(session_id));
    assert!(world.has_link(alice));
    assert_eq!(world.store.players_online(), 1);
    // And the lobby serves it.
    world.ask(alice, request::play(1));
    assert_eq!(denial(&world.take(alice), 0x2F), None);
    assert!(world.core.rooms.of(world.ids[alice].0).is_some());
    world.assert_clean();
}

#[test]
fn a_client_in_a_match_may_be_silent_for_longer() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, alice_team) = world.match_of(alice, bob);
    let seated = world.in_match(alice);
    assert!(seated.is_some());
    // Loading its level: 100 s without a word, well past the lobby's 30.
    world.silent_for(alice, Duration::from_secs(100));
    assert!(world.has_link(alice));
    assert_eq!(world.in_match(alice), seated);
    assert!(world.core.away.is_empty());
    // It talks again on the same link, and the match's end reaches it
    // there: back in the lobby, then the results.
    world.advance(Duration::from_secs(5));
    world.take(alice);
    world.end_match(match_id, alice_team);
    world.settle();
    let decoded = world.take(alice);
    assert_eq!(queue_state(&decoded), Some(ClientState::Lobby));
    assert_eq!(screens(&decoded).len(), 1);
    assert_eq!(world.in_match(alice), None);
    world.assert_clean();
}

#[test]
fn a_link_lost_in_a_match_comes_back_into_the_match() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let session_id = world.ids[alice].1;
    let (match_id, alice_team) = world.match_of(alice, bob);
    let seated = world.in_match(alice);
    // Two minutes of silence cost the link, but not the session, however
    // long the match lasts.
    world.silent_for(alice, Duration::from_secs(121));
    assert!(!world.has_link(alice));
    assert_eq!(world.core.away[&session_id].in_match, seated);
    world.silent_for(alice, Duration::from_secs(600));
    assert!(!world.store.has_ended(session_id));
    // The client connects again in the middle of the match: the lobby has
    // it in the match again, and does not send it there a second time.
    world.connect_again(alice, 49003);
    assert!(world.client(alice).model.is_signed_in());
    assert_eq!(world.in_match(alice), seated);
    world.advance(Duration::from_secs(3));
    assert!(sent_to_match(&world.take(alice)).is_none());
    assert!(world.host.reseated.lock().unwrap().is_empty());
    // The end of the match reaches it as it does a link that never broke.
    world.end_match(match_id, alice_team);
    world.settle();
    let decoded = world.take(alice);
    assert_eq!(queue_state(&decoded), Some(ClientState::Lobby));
    assert_eq!(screens(&decoded).len(), 1);
    assert_eq!(world.in_match(alice), None);
    world.assert_clean();
}

#[test]
fn a_client_away_through_its_match_comes_back_to_the_lobby_after_it() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let (alice_session, carol_session) = (world.ids[alice].1, world.ids[carol].1);
    // Carol and Dave are of a level far from Alice's: Carol makes a room of
    // her own, and Dave's Play takes it, the room nearest his level.
    for player in [carol, dave] {
        super::pvb_tests::with_gear(&mut world, player, 6);
    }
    let (first, alice_team) = world.match_of(alice, bob);
    super::rooms_tests::create(
        &mut world,
        carol,
        survarium::v026g::lobby::rooms::RoomSpec {
            max_level: 10,
            ..super::rooms_tests::spec(8)
        },
        None,
    );
    let (second, carol_team) = world.match_of(carol, dave);
    world.silent_for(alice, Duration::from_secs(121));
    // Carol's game is gone for good.
    let gone = world.clients[carol].0;
    world.clients[carol].0 = "127.0.0.1:1".parse().unwrap();
    world.advance(Duration::from_secs(121));
    assert!(world.core.player(gone).is_none());
    assert_eq!(world.core.away.len(), 2);

    // The matches end. The wait starts now: Alice connects again within
    // it, with no sign-in at the login server, and is shown the results.
    world.end_match(first, alice_team);
    world.end_match(second, carol_team);
    world.silent_for(alice, Duration::from_secs(40));
    assert!(!world.store.has_ended(alice_session));
    world.connect_again(alice, 49004);
    assert!(world.client(alice).model.is_signed_in());
    assert_eq!(world.in_match(alice), None);
    world.advance(Duration::from_secs(2));
    assert_eq!(screens(&world.take(alice)).len(), 1);
    // Carol never does: her session ends a minute after her match.
    world.advance(Duration::from_secs(25));
    assert!(world.store.has_ended(carol_session) || world.store.hold(carol_session).is_none());
    assert!(world.core.away.is_empty());
    assert!(!world.store.has_ended(alice_session));
}

#[test]
fn an_account_that_signs_in_anew_ends_the_session_that_waited() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let (account_id, old_session) = world.ids[alice];
    world.silent_for(alice, Duration::from_secs(31));
    assert!(world.core.away.contains_key(&old_session));
    // The player started the game again and signed in: a new session.
    let new_session = world.new_session(account_id);
    let again = world.connect_session((account_id, new_session));
    assert!(world.client(again).model.is_signed_in());
    assert!(world.core.away.is_empty());
    assert!(world.store.hold(old_session).is_none());
    assert!(!world.store.has_ended(new_session));
}

#[test]
fn with_no_wait_a_silent_client_loses_its_session_with_its_link() {
    let mut world = World::new();
    world.core.settings.session_grace = Duration::ZERO;
    let alice = world.connect("Alice");
    let session_id = world.ids[alice].1;
    world.silent_for(alice, Duration::from_secs(29));
    assert!(!world.store.has_ended(session_id));
    world.silent_for(alice, Duration::from_secs(2));
    assert!(world.store.has_ended(session_id));
    assert!(world.core.away.is_empty());
}

#[test]
fn a_drop_query_is_not_answered() {
    // `42 00 <u32>`: sent only after a chat line of the statistics
    // service on channel 4, which srp never sends; nothing in the client
    // waits for an answer (results.md §5.3).
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.advance(Duration::from_secs(6));
    world.take(alice);
    world.ask(alice, (0x42, vec![0, 7, 0, 0, 0]));
    assert!(world.take(alice).is_empty());
    // The link is as it was.
    world.ask(alice, request::autobuy(true));
    assert!(
        world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::Permitted(Permitted::Autobuy(true))))
    );
    world.assert_clean();
}
