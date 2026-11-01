//! The lobby's core with clients in memory. The clock is the test's: each
//! round is 10 ms, and the transports, the rooms and the timeouts all take
//! the time they are given.

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use session::v026g::rewards::{MatchEnd, MatchResult, PlayerResult, Results, Terms};
use session::v026g::{Account, GameAccounts, premium, rules};
use session::{LockExt, SessionStore};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::drops::Rng;
use survarium::v026g::lobby::client_model::{Decoded, FriendshipPush, Permitted};
use survarium::v026g::lobby::requests::ClientMessage;
use survarium::v026g::lobby::requests::{BUY_PREMIUM, INVENTORY_SLOT};
use survarium::v026g::lobby::{ClientState, FULL_CONDITION, NameRefusal, channel};
use survarium::v026g::maps;
use survarium::v026g::match_msgs::late_join::PATCH_REVISION;
use survarium::v026g::match_msgs::snapshot::{PlayerStatistics, event};
use survarium::v026g::prices;
use survarium::v026g::shop_offers;

use super::client::{TestClient, request};
use super::{TestHost, denial, find, sent_to_match, signed_in};
use crate::v026g::core::Core;
use crate::v026g::{Inbox, MatchFinished, Settings};

const ROUND: Duration = Duration::from_millis(10);
/// From Play by two players to their room's match: the vote, the wait of
/// the room Play makes (30 s), and a little more.
const PLAY_TO_MATCH: Duration = Duration::from_secs(47);

struct World {
    store: &'static SessionStore,
    accounts: &'static GameAccounts,
    results: &'static Results,
    host: &'static TestHost,
    inbox: &'static Inbox,
    core: Core<'static>,
    clients: Vec<(SocketAddr, TestClient)>,
    /// `(account id, session id)` of each client.
    ids: Vec<(u32, u32)>,
    now: Instant,
}

impl World {
    fn new() -> Self {
        Self::with_settings(Settings::local("127.0.0.1:0"))
    }

    fn with_settings(settings: Settings) -> Self {
        let store: &'static SessionStore = Box::leak(Box::new(SessionStore::new()));
        let accounts: &'static GameAccounts = Box::leak(Box::new(GameAccounts::new()));
        // The tests are written against the stock first kit (the TT-33) and
        // no premium: a new account has neither starter kit nor starter
        // premium unless a test turns them on.
        accounts.set_starter_kit(false);
        let results: &'static Results = Box::leak(Box::new(Results::new()));
        let host: &'static TestHost = Box::leak(Box::new(TestHost::default()));
        let inbox: &'static Inbox = Box::leak(Box::new(Inbox::default()));
        let now = Instant::now();
        let core = Core::new(store, accounts, results, host, inbox, settings, now);
        Self {
            store,
            accounts,
            results,
            host,
            inbox,
            core,
            clients: Vec::new(),
            ids: Vec::new(),
            now,
        }
    }

    /// One round: every client's datagrams to the lobby, the lobby's back.
    fn round(&mut self) {
        self.now += ROUND;
        for (addr, client) in &mut self.clients {
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

    /// Long enough for a request, its answers and their acks.
    fn settle(&mut self) {
        for _ in 0..40 {
            self.round();
        }
    }

    fn advance(&mut self, time: Duration) {
        let rounds = time.as_millis() / ROUND.as_millis();
        for _ in 0..rounds {
            self.round();
        }
    }

    /// A new account signs in with a game of the newest patch; returns
    /// the client's index.
    fn connect(&mut self, name: &str) -> usize {
        self.connect_revision(name, PATCH_REVISION)
    }

    /// The same with a game of patch `revision`.
    fn connect_revision(&mut self, name: &str, revision: u8) -> usize {
        let (account, _) = signed_in(self.store, name);
        let session = self.store.create_session_with_revision(account, revision);
        self.connect_session((account, session))
    }

    /// Another session of the account, as a sign-in of a game of the newest
    /// patch makes it.
    fn new_session(&self, account_id: u32) -> u32 {
        self.store
            .create_session_with_revision(account_id, PATCH_REVISION)
    }

    fn connect_session(&mut self, ids: (u32, u32)) -> usize {
        let index = self.clients.len();
        let addr: SocketAddr = format!("127.0.0.1:{}", 40_000 + index).parse().unwrap();
        let mut client = TestClient::new();
        client.sign_in(ids.1, true);
        self.clients.push((addr, client));
        self.ids.push(ids);
        // Past the next tick: the others' online counts have followed.
        self.advance(Duration::from_millis(1500));
        index
    }

    fn client(&mut self, index: usize) -> &mut TestClient {
        &mut self.clients[index].1
    }

    /// Send a request and let it be answered.
    fn ask(&mut self, index: usize, (id, body): (u8, Vec<u8>)) {
        self.client(index).send(id, &body);
        self.settle();
    }

    /// What the client took since the last call.
    fn take(&mut self, index: usize) -> Vec<Decoded> {
        let client = self.client(index);
        client.ids.clear();
        std::mem::take(&mut client.decoded)
    }

    /// The client's account. (The guard must not be held across a round:
    /// the lobby locks the account too.)
    fn account(&self, index: usize) -> std::sync::MutexGuard<'static, Account> {
        let account = self
            .accounts
            .get(self.ids[index].0)
            .expect("a signed-in account");
        let account: &'static std::sync::Arc<std::sync::Mutex<Account>> =
            Box::leak(Box::new(account));
        account.locked()
    }

    /// Change the account behind the lobby's back and tell it, as the
    /// results of a match will.
    fn change(&mut self, index: usize, change: impl FnOnce(&mut Account)) {
        change(&mut self.account(index));
        self.inbox.account_changed(self.ids[index].0);
        self.settle();
        self.take(index);
    }

    /// No client was sent anything it would drop, misread or crash on, and
    /// nothing is stuck behind a missing ordered message.
    fn assert_clean(&self) {
        for (addr, client) in &self.clients {
            assert_eq!(client.faults, Vec::<String>::new(), "client {addr}");
            assert_eq!(client.waiting_in_order(), 0, "client {addr}");
            assert_eq!(client.order_drops, 0, "client {addr}");
        }
    }
}

fn queue_state(decoded: &[Decoded]) -> Option<ClientState> {
    decoded.iter().rev().find_map(|message| match message {
        Decoded::ClientState { state, .. } => Some(*state),
        _ => None,
    })
}

/// The `91`s the lobby answers a `3B` with `start` at once, as the client
/// reads them: the rows of each, as `(name, place)`.
fn answered(world: &mut World, index: usize, start: u32, count: u8) -> Vec<Vec<(String, u32)>> {
    let addr = world.clients[index].0;
    let before = world.core.links[&addr].pushed.len();
    let now = world.now;
    world
        .core
        .handle(addr, ClientMessage::QueryRanking { start, count }, now);
    let mut client = survarium::v026g::lobby::client_model::Client::signed_in();
    world.core.links[&addr].pushed[before..]
        .iter()
        .map(|message| match client.receive(message).unwrap() {
            Decoded::EloList(rows) => rows.into_iter().map(|row| (row.0, row.1)).collect(),
            other => panic!("not a ranking list: {other:?}"),
        })
        .collect()
}

/// srp's own requests (`3B` from `lobby::RESERVED_START` on) that it does
/// not know get no answer: the start itself, the matches now and the JOIN
/// that went with the queue, past the replay poll.
#[test]
fn a_reserved_start_the_server_does_not_know_gets_no_answer() {
    use survarium::v026g::lobby::rooms::ROOMS_LIST;
    use survarium::v026g::lobby::{REPLAY_START, RESERVED_START};

    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    for start in [
        RESERVED_START,
        RESERVED_START + 1,
        2_100_000_000,
        2_100_000_001,
        REPLAY_START + 1,
        3_000_000_000,
        u32::MAX,
    ] {
        for count in [0, 1, 32, 255] {
            world.ask(alice, request::ranking(start, count));
            assert_eq!(world.take(alice), [], "start {start}, count {count}");
        }
    }
    // The link is as good as before.
    assert_eq!(answered(&mut world, alice, ROOMS_LIST, 32).len(), 1);
    world.assert_clean();
}

#[test]
fn the_ranking_page_is_answered_as_ever_and_srps_own_requests_touch_nothing_of_it() {
    use survarium::v026g::lobby::RESERVED_START;
    use survarium::v026g::lobby::rooms::{ROOM_STATE, ROOMS_LIST};

    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.account(bob).rating += 100;
    let addr = world.clients[alice].0;
    let pushed = |world: &World| world.core.links[&addr].pushed.clone();
    let ranked_sent = |world: &World| world.core.player(addr).unwrap().ranked_sent;

    // The ranking's first page, and a page further down.
    let first_page = |world: &mut World| {
        world.take(alice);
        world.ask(alice, request::ranking(0, 10));
        world.take(alice)
    };
    let before = first_page(&mut world);
    assert!(matches!(
        &before[..],
        [
            Decoded::BaseInfo { ranked_players: 2, .. },
            Decoded::Tutorial { show: false, .. },
            Decoded::PlayerElo { place: 2, .. },
            Decoded::EloList(rows)
        ] if rows.len() == 2 && rows[0].0 == "Bob" && rows[0].1 == 1 && rows[1].0 == "Alice"
    ));
    world.ask(alice, request::ranking(1, 1));
    let second = world.take(alice);
    assert!(matches!(
        &second[..],
        [Decoded::EloList(rows)] if rows.len() == 1 && rows[0].0 == "Alice" && rows[0].1 == 2
    ));

    // srp's own requests, asked with the count the page has of the ranking
    // made wrong on purpose: the first page of the ranking would put it
    // right and send the count, the account's own row and the rows. They
    // do none of that: one `91` each that is answered, and the count is as
    // it was.
    world.core.player_mut(addr).unwrap().ranked_sent = 77;
    let sent = pushed(&world);
    let now = world.now;
    for (start, count) in [
        (ROOMS_LIST, 32),
        (ROOM_STATE, 1),
        (RESERVED_START, 32),
        (u32::MAX, 255),
    ] {
        world
            .core
            .handle(addr, ClientMessage::QueryRanking { start, count }, now);
    }
    assert_eq!(ranked_sent(&world), 77);
    let answers = pushed(&world)[sent.len()..].to_vec();
    assert_eq!(
        answers
            .iter()
            .map(|message| message.id_byte())
            .collect::<Vec<_>>(),
        [0x91, 0x91]
    );
    // The last start of all that is a ranking page's is one: no row is
    // that far down, and the answer is the empty list it always was.
    world.core.handle(
        addr,
        ClientMessage::QueryRanking {
            start: RESERVED_START - 1,
            count: 32,
        },
        now,
    );
    assert_eq!(ranked_sent(&world), 77);
    let last = pushed(&world).pop().unwrap();
    assert_eq!((last.id_byte(), last.body()), (0x91, &[0_u8][..]));
    // And the first page does put the count right.
    world.core.handle(
        addr,
        ClientMessage::QueryRanking {
            start: 0,
            count: 10,
        },
        now,
    );
    assert_eq!(ranked_sent(&world), 2);
    world.settle();

    // The page after it all: what it was before.
    assert_eq!(first_page(&mut world), before);
    world.ask(alice, request::ranking(1, 1));
    assert_eq!(world.take(alice), second);
    world.assert_clean();
}

#[test]
fn a_sign_in_is_answered_with_the_whole_account_in_the_clients_order() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let client = world.client(alice);
    assert!(client.model.is_signed_in());
    // 0x80 first; then the ordered pushes in the order of `lobby.md` §3,
    // the inventory before the account. The lists and the squad (which the
    // client never asks for) follow; 0x9F and 0xA1 are not ordered against
    // the rest.
    assert_eq!(client.ids[0], 0x80);
    let ordered: Vec<u8> = client
        .ids
        .iter()
        .copied()
        .filter(|id| !matches!(id, 0x80 | 0xA1))
        .collect();
    assert_eq!(
        ordered,
        [
            0x8F, 0x90, 0x8B, 0x96, 0x8E, 0x8D, 0x94, 0x8A, 0x8E, 0x95, 0x89, 0x87, 0x8C, 0x92,
            0x9C, 0x9D, 0x9B, 0x85, 0x91, 0xA3, 0xA3, 0x99
        ]
    );
    assert!(client.ids.contains(&0xA1));
    // The three free weapons and every ammo (`tutorial::starter_ammo`).
    let ammo = session::v026g::tutorial::starter_ammo(Dictionary::shared()).len();
    assert_eq!(client.model.inventory().len(), 3 + ammo);
    // Play is enabled: more than half an hour to maintenance. The counts
    // came with the pushes and, as 0x9F is sent once and may be lost,
    // again on the next pass.
    assert!(client.online.len() >= 2);
    assert!(client.online.iter().all(|counts| *counts == (1, 0, 1440)));
    let decoded = world.take(alice);
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Account(own) if own.body.name == "Alice" && own.body.kits.len() == 3
    )));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Friendship(FriendshipPush::Friends(friends)) if friends.is_empty()
    )));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Friendship(FriendshipPush::Ignored(ignored)) if ignored.is_empty()
    )));
    assert!(
        decoded
            .iter()
            .any(|message| matches!(message, Decoded::SquadInfo { squad_id: 0, .. }))
    );
    assert_eq!(queue_state(&decoded), Some(ClientState::Lobby));
    assert_eq!(world.store.players_online(), 1);
    world.assert_clean();
}

#[test]
fn nothing_but_the_sign_in_answer_goes_before_it_is_acked() {
    let mut world = World::new();
    let (_, session) = signed_in(world.store, "Alice");
    let addr: SocketAddr = "127.0.0.1:40000".parse().unwrap();
    let mut client = TestClient::new();
    client.sign_in(session, true);
    let now = world.now;
    for datagram in client.poll(now) {
        world.core.datagram(addr, &datagram, now);
    }
    // The first datagrams carry the 0x80 alone, however often they go: a
    // push that overtook it would be thrown away by the client's sign-in
    // handler.
    for _ in 0..5 {
        world.now += ROUND;
        for (_, datagram) in world.core.step(world.now) {
            let parsed = udp_transport::wire::parse(
                udp_transport::Table::Lobby,
                udp_transport::Direction::ToClient,
                &datagram,
                0,
            )
            .unwrap();
            for message in &parsed.messages {
                assert!(
                    message.low_level || message.id == 0x80,
                    "{:#04X}",
                    message.id
                );
            }
        }
    }
    // Once the client acks it the rest comes.
    world.clients.push((addr, client));
    world.ids.push((0, session));
    world.settle();
    assert!(world.client(0).ids.contains(&0x87));
    world.assert_clean();
}

#[test]
fn an_unknown_or_ended_session_is_refused() {
    let mut world = World::new();
    let unknown = world.connect_session((0, 0xBAD));
    assert!(world.client(unknown).disconnected);
    assert!(world.client(unknown).decoded.is_empty());

    let ids = signed_in(world.store, "Alice");
    assert!(world.store.end_session(ids.1));
    let ended = world.connect_session(ids);
    assert!(world.client(ended).disconnected);
    assert!(world.client(ended).decoded.is_empty());
    assert_eq!(world.store.players_online(), 0);
    // Both links are forgotten once their disconnect is done.
    world.advance(Duration::from_secs(3));
    assert!(world.core.links.is_empty());
    world.assert_clean();
}

#[test]
fn a_datagram_that_is_no_sign_in_makes_no_link() {
    let mut world = World::new();
    let addr: SocketAddr = "127.0.0.1:40000".parse().unwrap();
    let mut client = TestClient::new();
    let (id, body) = request::play(1);
    client.send(id, &body);
    for datagram in client.poll(world.now) {
        world.core.datagram(addr, &datagram, world.now);
    }
    world.core.datagram(addr, &[1, 2, 3, 4, 5, 6, 7], world.now);
    world.core.datagram(addr, &[], world.now);
    assert!(world.core.links.is_empty());
    assert!(world.core.step(world.now).is_empty());
}

/// Gear is sold by the factions, each item from a reputation level on.
fn known_to_every_seller(account: &mut Account) {
    for faction in 1..=4 {
        account.add_reputation(faction, 40_000);
    }
}

/// A shop record the account can buy now: a stack or not.
fn buyable(account: &Account, stack: bool) -> &'static prices::PriceItem {
    prices::price_list()
        .iter()
        .find(|record| {
            Dictionary::shared().is_stack(record.dict_id) == stack
                && rules::buy(
                    &mut account.clone(),
                    record.dict_id,
                    1,
                    record.faction_id,
                    INVENTORY_SLOT,
                    1,
                    1_000,
                )
                .is_ok()
        })
        .expect("the shop sells something")
}

#[test]
fn buying_selling_and_moving_go_round_with_their_pushes() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, |account| {
        account.generic_money = 5_000_000;
        known_to_every_seller(account);
    });
    let record = buyable(&world.account(alice), false);
    let before = world.client(alice).model.inventory().len();

    // Buy: `83 47 00`, then the inventory, the account and the money.
    world.ask(
        alice,
        request::buy(record.dict_id, 1, record.faction_id, INVENTORY_SLOT, 1),
    );
    assert_eq!(world.client(alice).ids, [0x83, 0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    let Decoded::Permitted(Permitted::Bought(bought)) = &decoded[0] else {
        panic!("{:?}", decoded[0]);
    };
    assert_eq!((bought.len(), bought[0].dict_id), (1, record.dict_id));
    let item_id = bought[0].item_id;
    assert!(matches!(
        decoded[3],
        Decoded::Money { generic, .. } if generic == 5_000_000 - record.cost
    ));
    assert_eq!(world.client(alice).model.inventory().len(), before + 1);
    assert!(world.account(alice).item(item_id).is_some());

    // Move the pistol off the kit: no `83`, the pushes are the answer.
    world.ask(alice, request::move_item(1, 1, 18, 7, INVENTORY_SLOT, 1));
    assert_eq!(world.client(alice).ids, [0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    let Decoded::Account(own) = &decoded[1] else {
        panic!("{:?}", decoded[1]);
    };
    assert_eq!(own.body.kits[0].slots[7], None);
    assert_eq!(world.account(alice).kits[0].slots[7], None);

    // Sell what was bought: `83 47 01` and the pushes.
    world.ask(alice, request::sell(item_id, 1));
    assert_eq!(world.client(alice).ids, [0x83, 0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::Sold { item_id: sold, amount: 1 }) if sold == item_id
    ));
    assert_eq!(world.client(alice).model.inventory().len(), before);
    assert!(world.account(alice).item(item_id).is_none());
    world.assert_clean();
}

#[test]
fn a_worn_item_is_repaired_and_gold_is_exchanged() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, |account| {
        account.generic_money = 1_000_000;
        rules::wear(account, 1, 0.5);
    });
    assert!(world.account(alice).item(1).unwrap().condition_or_stack < FULL_CONDITION);

    world.ask(alice, request::repair(1, 0));
    assert_eq!(world.client(alice).ids, [0x83, 0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::Repaired { item_id: 1, .. })
    ));
    assert_eq!(
        world.account(alice).item(1).unwrap().condition_or_stack,
        FULL_CONDITION
    );
    // Nothing left to repair: the game's own text for it.
    world.ask(alice, request::repair(1, 0));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_repair_item")
    );

    // A new account has no gold: this one is given some.
    world.change(alice, |account| account.premium_money = 1_000);
    let gold = world.account(alice).premium_money;
    world.ask(alice, request::exchange(2));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::NoEffect {
            op: 0x47,
            event: Some(4)
        })
    ));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Money { premium, .. } if *premium == gold - 2
    )));

    // srp's exchange window buys gold back: the same answer.
    let silver = world.account(alice).generic_money;
    world.ask(alice, request::exchange(BUY_PREMIUM + 3));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::NoEffect {
            op: 0x47,
            event: Some(4)
        })
    ));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Money { premium, generic } if *premium == gold + 1 && *generic == silver - 300
    )));
    // More gold than the silver pays for: the game's own text for it.
    world.ask(alice, request::exchange(BUY_PREMIUM + 1_000_000));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_not_enougth_money")
    );
    assert_eq!(world.account(alice).premium_money, gold + 1);
    world.assert_clean();
}

#[test]
fn skills_are_answered_with_the_skills_and_the_kits_that_carry_them() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, |account| {
        account.add_experience(50_000);
    });

    world.ask(alice, request::skills(&[(101, 1)]));
    // No `83 48`: the client has no handler for it.
    assert_eq!(world.client(alice).ids, [0x8C, 0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    assert!(matches!(
        &decoded[0],
        Decoded::Skills { skills, .. } if *skills == [(101, 1)]
    ));
    let Decoded::Account(own) = &decoded[2] else {
        panic!("{:?}", decoded[2]);
    };
    assert!(own.body.kits[0].modifiers.iter().any(|value| *value != 0.0));

    // Taking a point back is refused, and the skills are sent as they are.
    world.ask(alice, request::skills(&[]));
    assert_eq!(world.client(alice).ids, [0x84, 0x8C]);
    let decoded = world.take(alice);
    assert_eq!(
        denial(&decoded, 0x48).as_deref(),
        Some("st_player_skills_check_failed")
    );

    // A reset of the branch, in silver.
    let silver = world.account(alice).generic_money;
    world.ask(alice, request::reset_skills(0, 0));
    let decoded = world.take(alice);
    assert!(matches!(
        &decoded[0],
        Decoded::Skills { skills, .. } if skills.is_empty()
    ));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Money { generic, .. } if *generic == silver - rules::REROLL_COST
    )));
    world.assert_clean();
}

#[test]
fn a_refusal_carries_the_games_own_text() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.change(alice, known_to_every_seller);

    // Not enough silver for the dearest thing the shop lists.
    let dear = prices::price_list()
        .iter()
        .max_by_key(|record| record.cost)
        .unwrap();
    assert!(dear.cost > world.account(alice).generic_money);
    world.ask(
        alice,
        request::buy(dear.dict_id, 1, dear.faction_id, INVENTORY_SLOT, 1),
    );
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_not_enougth_money")
    );
    // An item it does not own.
    world.ask(alice, request::sell(999, 1));
    assert!(denial(&world.take(alice), 0x47).is_some());
    // A move of an item that is not in the slot: refused, and the client's
    // window is put right with the inventory and the account.
    world.ask(alice, request::move_item(1, 1, 18, 3, INVENTORY_SLOT, 1));
    assert_eq!(world.client(alice).ids, [0x84, 0x89, 0x87, 0x8B]);
    assert_eq!(
        denial(&world.take(alice), 0x46).as_deref(),
        Some("st_cannot_relocate_item")
    );
    // A kit that is none of its own made the active one.
    world.ask(alice, request::active_kit(77));
    assert!(denial(&world.take(alice), 0x49).is_some());
    // Play without a weapon.
    world.ask(alice, request::move_item(1, 1, 18, 7, INVENTORY_SLOT, 1));
    world.take(alice);
    world.ask(alice, request::play(1));
    assert_eq!(
        denial(&world.take(alice), 0x2F).as_deref(),
        Some("st_profile_check_failed_weapon")
    );
    // Play with a kit that is none of its own.
    world.ask(alice, request::play(77));
    assert_eq!(
        denial(&world.take(alice), 0x2F).as_deref(),
        Some("st_profile_check_failed")
    );
    assert!(world.store.matchmaking().busy_sessions().is_empty());
    world.assert_clean();
}

#[test]
fn what_srp_does_not_have_is_refused_never_ignored() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    let bytes = |parts: &[&[u8]]| -> Vec<u8> { parts.concat() };
    let u32s = |sub: u8, values: &[u32], tail: &[u8]| -> Vec<u8> {
        let mut body = vec![sub];
        for value in values {
            body.extend(value.to_le_bytes());
        }
        body.extend_from_slice(tail);
        body
    };
    #[rustfmt::skip]
    let refused: Vec<(u8, Vec<u8>, &str)> = vec![
        // Clan-team Play.
        (0x30, 1_u32.to_le_bytes().to_vec(), "st_must_be_in_clan_team"),
        // A cosmetic the account has not, attached (`items_tests` has the
        // ones it has, and a detach of a cosmetic that is gone, which is
        // off).
        (0x46, u32s(1, &[1, 5, 6], &[0, 0, 0, 0, 0, 0, 0, 0, 0]), "st_cannot_upgrade_item"),
        // A detach cut short before its upgrade is no request. (One
        // without its host, a game without patch 30, is read: host 0,
        // `items_tests`.)
        (0x46, u32s(2, &[1], &[]), "st_cannot_relocate_item"),
        // A drop card, an item used.
        (0x46, u32s(3, &[5, 6], &[]), "st_cannot_use_drop_card"),
        (0x46, u32s(4, &[5], &[]), "st_error_while_using_premium_access_voucher"),
        // The reroll window of no item, a reroll of a modification the
        // pistol has not, a choice with nothing offered. (A craft that
        // can not be is answered `83 46 09 FE`: `items_tests`.)
        (0x46, u32s(6, &[99], &[]), "st_cannot_select_rerolled_modification"),
        (0x46, u32s(7, &[1, 2], &[0xFF]), "st_cannot_select_rerolled_modification"),
        (0x46, u32s(8, &[1, 2, 3], &[]), "st_cannot_select_rerolled_modification"),
        // A shop offer, premium with bonuses and with a discount.
        // (An offer that is none; no discount was offered.)
        (0x47, u32s(2, &[99, 0, 0], &[]), "st_cannot_buy_item"),
        (0x47, u32s(5, &[1, 0, 0, 0, 0], &[1]), "st_cannot_buy_item"),
        (0x47, u32s(6, &[1, 0], &[]), "st_cannot_buy_item"),
        // A repair in gold of an item with no defects.
        (0x47, u32s(3, &[1], &[1]), "st_cannot_repair_item"),
        // A nickname change, a comment about a player.
        // (A name too short.)
        (0x49, bytes(&[&[4, 2], b"Al", &[0, 0, 0, 0]]), "st_change_name_error"),
        (0x49, bytes(&[&[5, 3], b"Bob", &[1, 1]]), "st_error_during_storing_user_comment"),
        // Clans: reserve a name, create, invite, a clan team.
        (0x49, bytes(&[&[6, 4], b"Clan", &[2], b"CL"]), "st_cant_reserve_clan"),
        (0x49, vec![7], "st_cant_create_clan"),
        (0x49, bytes(&[&[0x0F, 3], b"Bob"]), "st_cant_invite_player_to_clan"),
        (0x49, vec![0x17], "st_cant_create_clan_team"),
        (0x49, vec![0x21, 1], "st_cant_change_ready_state"),
    ];
    for (op, body, text) in refused {
        world.client(alice).send(op, &body);
        world.settle();
        let decoded = world.take(alice);
        assert_eq!(
            denial(&decoded, op).as_deref(),
            Some(text),
            "{op:#04X} {body:02X?}: {decoded:?}"
        );
    }

    // Questions are answered: a name check by the name, the clan window
    // with "no clan", the video link with none.
    world.ask(alice, request::profile_text(3, "Al"));
    assert!(matches!(
        &world.take(alice)[0],
        Decoded::Permitted(Permitted::NicknameCheck { nickname, refusal: Some(NameRefusal::Short) })
            if nickname == "Al"
    ));
    world.ask(alice, request::profile_text(3, "Stalker"));
    assert!(matches!(
        &world.take(alice)[0],
        Decoded::Permitted(Permitted::NicknameCheck { refusal: None, .. })
    ));
    // (A clan's name may have blanks, but not at its ends.)
    world.ask(alice, request::profile_text(8, "Free Stalkers "));
    assert!(matches!(
        &world.take(alice)[0],
        Decoded::Permitted(Permitted::ClanNameCheck {
            refusal: Some(NameRefusal::Space),
            ..
        })
    ));
    world.ask(alice, (0x49, vec![0x0E, 0]));
    assert!(matches!(&world.take(alice)[0], Decoded::ClanInfo(_)));
    world.ask(alice, (0x45, vec![2]));
    assert!(matches!(
        &world.take(alice)[0],
        Decoded::VideoLink { show: false, .. }
    ));
    // A request cut short is refused too when the client waits for it.
    world.ask(alice, (0x47, vec![0, 1]));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_buy_item")
    );
    world.assert_clean();
}

#[test]
fn the_queries_are_answered() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.account(bob).rating += 100;

    world.ask(alice, request::query(0x37));
    assert!(matches!(world.take(alice)[..], [Decoded::Skills { .. }]));
    world.ask(alice, request::query(0x39));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Reputations { .. }]
    ));
    world.ask(alice, request::query(0x3C));
    assert!(matches!(world.take(alice)[..], [Decoded::Quests(_)]));
    world.ask(alice, request::query(0x3E));
    assert!(matches!(world.take(alice)[..], [Decoded::Services { .. }]));
    world.ask(alice, request::query(0x40));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::UnlockedFactions(0x0F)]
    ));
    // The ranking's first page: the count, the account's own row, the rows.
    world.ask(alice, request::ranking(0, 10));
    let decoded = world.take(alice);
    assert!(matches!(
        &decoded[..],
        [
            Decoded::BaseInfo { ranked_players: 2, .. },
            // What makes the client's page take the new count.
            Decoded::Tutorial { show: false, .. },
            Decoded::PlayerElo { place: 2, .. },
            Decoded::EloList(rows)
        ] if rows.len() == 2 && rows[0].0 == "Bob" && rows[1].0 == "Alice"
    ));
    // The echo after 0x80 asks for nothing; nor does a tutorial step
    // (there is no tutorial).
    world.ask(alice, (0x32, vec![0, 0]));
    world.ask(alice, (0x4B, vec![2]));
    world.ask(alice, (0x49, vec![0x24, 0]));
    world.ask(alice, (0x4B, vec![1, 2]));
    assert!(world.take(alice).is_empty());

    // The autobuy box and the active kit have their own `83 49`.
    world.ask(alice, request::autobuy(true));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Permitted(Permitted::Autobuy(true))]
    ));
    world.ask(alice, request::active_kit(2));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Permitted(Permitted::ActiveKit(2))]
    ));
    assert_eq!(world.account(alice).active_kit, 2);
    world.assert_clean();
}

#[test]
fn choosing_a_quest_faction_gives_its_quests() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    world.ask(alice, request::quest_faction(2));
    let decoded = world.take(alice);
    // The faction byte travels in 0x8E; `83 4B` makes the client ask for
    // its quests and services.
    assert!(matches!(
        decoded[0],
        Decoded::Reputations {
            quest_active_faction: 2,
            ..
        }
    ));
    assert!(matches!(
        decoded[1],
        Decoded::Permitted(Permitted::QuestFaction)
    ));
    // Each new quest is announced in the game's own way (`#+q`).
    let announced = decoded
        .iter()
        .filter(|message| matches!(message, Decoded::Important { kind: 1, text, .. } if text.starts_with("#+q")))
        .count();
    assert_eq!(announced, world.account(alice).quests.len());
    assert!(announced > 0);
    world.ask(alice, request::query(0x3C));
    assert!(matches!(
        &world.take(alice)[..],
        [Decoded::Quests(quests)] if quests.len() == announced
    ));
    world.assert_clean();
}

#[test]
fn two_players_meet_in_a_room_and_go_to_one_match() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);

    // Alone: a room is made for her, and it waits, however long. Her
    // client is told it is in the lobby, where the movie shows the room.
    world.ask(alice, request::play(1));
    assert_eq!(queue_state(&world.take(alice)), Some(ClientState::Lobby));
    let room = world
        .core
        .rooms
        .of(world.ids[alice].0)
        .expect("Alice's room")
        .key;
    world.advance(Duration::from_secs(150));
    assert!(sent_to_match(&world.take(alice)).is_none());
    assert!(world.host.made.lock().unwrap().is_empty());

    // The second player goes into it, and its vote and wait begin.
    world.take(bob);
    world.ask(bob, request::play(1));
    assert_eq!(
        world.core.rooms.of(world.ids[bob].0).map(|room| room.key),
        Some(room)
    );
    world.advance(PLAY_TO_MATCH - Duration::from_secs(4));
    assert!(sent_to_match(&world.take(alice)).is_none());
    world.advance(Duration::from_secs(4));
    let (to_alice, to_bob) = (world.take(alice), world.take(bob));
    let (alice_match, alice_team) = sent_to_match(&to_alice).expect("Alice's match");
    let (bob_match, bob_team) = sent_to_match(&to_bob).expect("Bob's match");
    assert_eq!(alice_match, bob_match);
    assert_ne!(alice_team, bob_team);
    // "Match found" (place 0) came first, and nothing after the 0x82 took
    // the match away again.
    for decoded in [&to_alice, &to_bob] {
        let connect = decoded
            .iter()
            .position(|message| matches!(message, Decoded::ConnectToMatch { .. }))
            .unwrap();
        assert!(decoded[..connect].iter().any(|message| matches!(
            message,
            Decoded::ClientState {
                state: ClientState::InQueue { place: 0, .. },
                ..
            }
        )));
        assert!(
            !decoded[connect..]
                .iter()
                .any(|message| matches!(message, Decoded::ClientState { .. }))
        );
    }

    // The match server was asked once, for a roster the clients can hold.
    let made = world.host.made.lock().unwrap().clone();
    assert_eq!(made.len(), 1);
    let (match_id, setup) = &made[0];
    assert_eq!(*match_id, alice_match);
    assert!(maps::map_mode(setup.map_id).unwrap().is_playable());
    assert_eq!(setup.players.len(), 2);
    assert_eq!(setup.players[0].session_id, world.ids[alice].1);
    assert_eq!(setup.players[0].account_id, world.ids[alice].0);
    let profile = &setup.players[0].profile;
    assert_eq!((profile.name.as_str(), profile.team), ("Alice", alice_team));
    // Its real kits and items: the starter pistol in slot 7 of the first.
    assert_eq!(profile.kits.len(), 3);
    assert_eq!(profile.items.len(), 2);
    let (pistol, count) = profile.kits[0].slots[7].unwrap();
    assert_eq!((profile.items[usize::from(pistol)].dict_id, count), (18, 1));
    drop(made);

    // The online counts show the match.
    world.advance(Duration::from_secs(2));
    assert_eq!(world.client(alice).online.last(), Some(&(2, 1, 1440)));
    world.assert_clean();
}

#[test]
fn the_match_profile_carries_the_skills_modifiers() {
    let mut account = Account::new("Alice");
    account.add_experience(50_000);
    rules::set_skills(&mut account, &[(101, 1)]).unwrap();
    let profile = crate::v026g::match_profile(&account, 1, 3, 1_000);
    assert_eq!((profile.team, profile.squad), (1, 3));
    assert_eq!(
        profile.kits[0].modifiers,
        survarium::v026g::skills::SkillsTree::shared().modifiers(&account.skills)
    );
    assert!(profile.kits[0].modifiers.iter().any(|value| *value != 0.0));
    // Only the kit with a weapon is loaded in the match: the second and
    // the third are empty.
    assert_eq!(
        profile.kits.iter().map(|kit| kit.state).collect::<Vec<_>>(),
        [0, 2, 2]
    );
}

/// A kit against the covered-slots rule (one saved before it: the Zubr
/// UM-4 helmet, which has its mask built in, over a mask) is never loaded
/// in a match: what the match gets is what the lobby's rule lets be worn.
#[test]
fn a_kit_with_a_covered_slot_filled_is_not_loaded_in_a_match() {
    let mut account = Account::new("Alice");
    let zubr = survarium::v026g::dictionary::Dictionary::shared()
        .items
        .iter()
        .find(|item| item.cfg_name.ends_with("/renesanse_helmet_6.options"))
        .unwrap()
        .dict_id;
    let helmet = account.add_item(zubr, 100);
    let mask = account.add_item(1810, 100);
    let slot = |item_id| Some(session::v026g::KitSlot { item_id, count: 1 });
    account.kits[0].slots[0] = slot(helmet);
    assert_eq!(
        crate::v026g::match_profile(&account, 0, 0, 1_000).kits[0].state,
        0
    );
    account.kits[0].slots[1] = slot(mask);
    assert_eq!(
        crate::v026g::match_profile(&account, 0, 0, 1_000).kits[0].state,
        2
    );
    account.uncover_kits();
    assert_eq!(
        crate::v026g::match_profile(&account, 0, 0, 1_000).kits[0].state,
        0
    );
}

/// A spawn with a kit that has no weapon ends every game of the match
/// (`match_profile`): such a kit is never loaded in one, whoever may use
/// it in the lobby.
#[test]
fn a_kit_without_a_weapon_is_not_loaded_in_a_match() {
    let states = |account: &Account| {
        crate::v026g::match_profile(account, 0, 0, 1_000)
            .kits
            .iter()
            .map(|kit| kit.state)
            .collect::<Vec<_>>()
    };
    let mut account = Account::new("Alice");
    assert_eq!(states(&account), [0, 2, 2]);
    // The second kit gets the first one's weapon and ammunition: loaded.
    account.kits[1].slots = account.kits[0].slots;
    assert_eq!(states(&account), [0, 0, 2]);
    // The third kit is the account's to use in the lobby without premium,
    // and is still not loaded while it is empty.
    assert!(!account.is_premium(1_000));
    assert_eq!(account.kit_state(2, 1_000), 0);
    assert_eq!(states(&account), [0, 0, 2]);
    account.kits[2].slots = account.kits[0].slots;
    assert_eq!(states(&account), [0, 0, 0]);
    // The weapon taken out of a kit: not loaded any more, though the lobby
    // lets the player have such a kit.
    account.kits[1].slots[7] = None;
    account.kits[1].slots[10] = None;
    assert_eq!(account.kit_state(1, 1_000), 0);
    assert_eq!(states(&account), [0, 2, 0]);
}

/// The admin page plans a maintenance in the store; every client in the
/// lobby gets the minutes left in its next online counts (`9F`), and a day
/// again once it is cancelled. Under 60 the client shows the notice, at 30
/// or under it locks Play (`update_play_button_state` 0x21FD40).
#[test]
fn a_planned_maintenance_reaches_the_clients_in_the_lobby() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let minutes = |world: &mut World| {
        world.advance(Duration::from_secs(6));
        world.client(alice).online.last().map(|counts| counts.2)
    };
    assert_eq!(minutes(&mut world), Some(1440));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    world.store.set_maintenance(Some(now + 45 * 60));
    assert_eq!(minutes(&mut world), Some(45));
    world.store.set_maintenance(Some(now + 20 * 60));
    assert_eq!(minutes(&mut world), Some(20));
    world.store.set_maintenance(None);
    assert_eq!(minutes(&mut world), Some(1440));
}

/// Who leaves its room before its match starts is in no match: the room
/// has one player left, and waits.
#[test]
fn leaving_the_room_before_its_match_makes_no_match() {
    use survarium::v026g::lobby::rooms::ROOM_LEAVE;

    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(5));
    answered(&mut world, bob, ROOM_LEAVE, 1);
    assert!(world.core.rooms.of(world.ids[bob].0).is_none());
    world.advance(PLAY_TO_MATCH);
    for client in [alice, bob] {
        assert!(sent_to_match(&world.take(client)).is_none());
    }
    assert!(world.core.rooms.of(world.ids[alice].0).is_some());
    assert!(world.host.made.lock().unwrap().is_empty());
    world.assert_clean();
}

#[test]
fn a_finished_match_brings_its_players_back_to_the_lobby() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(PLAY_TO_MATCH);
    let (match_id, _) = sent_to_match(&world.take(alice)).expect("a match");
    assert!(sent_to_match(&world.take(bob)).is_some());
    assert_eq!(world.store.match_count(), 1);

    // Bob leaves the match early: his client is in the lobby again, but
    // his seat is kept, and Play sends him back (0.21d's rule).
    world.ask(bob, request::leave(1, true));
    assert_eq!(queue_state(&world.take(bob)), Some(ClientState::Lobby));
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(1));
    assert_eq!(
        sent_to_match(&world.take(bob)).map(|sent| sent.0),
        Some(match_id)
    );
    assert_eq!(world.host.made.lock().unwrap().len(), 1);

    // The match server says the match is over.
    world.host.open.lock().unwrap().clear();
    world.inbox.match_finished(MatchFinished {
        match_id,
        not_made: false,
    });
    world.settle();
    for client in [alice, bob] {
        assert_eq!(queue_state(&world.take(client)), Some(ClientState::Lobby));
    }
    assert_eq!(world.store.match_count(), 0);
    assert!(world.core.matches.is_empty());

    // Both came by Play and are in their room still (the owner's of
    // 2026-10-31): its next vote makes the next match.
    assert_eq!(world.core.rooms.len(), 1);
    world.advance(PLAY_TO_MATCH + Duration::from_secs(15));
    let again = sent_to_match(&world.take(alice)).expect("a second match").0;
    assert_ne!(again, match_id);

    // A match that ends without a word is noticed too: the lobby asks the
    // match server about its matches.
    world.host.open.lock().unwrap().clear();
    world.advance(Duration::from_secs(2));
    assert_eq!(queue_state(&world.take(alice)), Some(ClientState::Lobby));
    assert_eq!(world.store.match_count(), 0);
    world.assert_clean();
}

#[test]
fn a_match_the_match_server_cannot_make_sends_everyone_back() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.host.fail.store(true, Ordering::Relaxed);
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(PLAY_TO_MATCH);
    for client in [alice, bob] {
        let decoded = world.take(client);
        assert_eq!(queue_state(&decoded), Some(ClientState::Lobby));
        assert!(sent_to_match(&decoded).is_none());
    }
    assert_eq!(world.store.match_count(), 0);
    assert!(world.store.matchmaking().busy_sessions().is_empty());
    // And they can try again.
    world.host.fail.store(false, Ordering::Relaxed);
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(PLAY_TO_MATCH);
    assert!(sent_to_match(&world.take(alice)).is_some());
    world.assert_clean();
}

#[test]
fn chat_reaches_who_it_is_for() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    for client in [alice, bob, carol] {
        world.ask(client, request::subscribe());
        world.take(client);
    }
    let lines = |decoded: &[Decoded]| -> Vec<(String, u8, String)> {
        decoded
            .iter()
            .filter_map(|message| match message {
                Decoded::ChatText {
                    sender_type: 5,
                    sender_name,
                    sender_clan_tag: Some(_),
                    channel,
                    body,
                    ..
                } => Some((sender_name.clone(), *channel, body.clone())),
                _ => None,
            })
            .collect()
    };

    // The general channel: everyone but the sender (its client echoes).
    world.ask(alice, request::chat(2, "", channel::GENERAL, "hello"));
    let hello = vec![("Alice".to_owned(), channel::GENERAL, "hello".to_owned())];
    assert_eq!(lines(&world.take(bob)), hello);
    assert_eq!(lines(&world.take(carol)), hello);
    assert!(lines(&world.take(alice)).is_empty());

    // A private line: the named player only.
    world.ask(alice, request::chat(0, "bob", channel::PRIVATE, "psst"));
    assert_eq!(
        lines(&world.take(bob)),
        [("Alice".to_owned(), channel::PRIVATE, "psst".to_owned())]
    );
    assert!(lines(&world.take(carol)).is_empty());

    // Not in a match or a squad: those channels reach nobody, whatever id
    // the client names.
    world.ask(alice, request::chat(7, "", channel::MATCH, "anyone?"));
    world.ask(alice, request::chat(7, "", channel::SQUAD, "anyone?"));
    assert!(lines(&world.take(bob)).is_empty());

    // Bob ignores Alice: her lines no longer reach him.
    let alice_id = world.ids[alice].0;
    world.ask(bob, request::ignore(alice_id));
    assert!(matches!(
        &world.take(bob)[..],
        [Decoded::Friendship(FriendshipPush::Ignored(ignored))]
            if ignored[..] == [(alice_id, "Alice".to_owned())]
    ));
    world.ask(
        alice,
        request::chat(2, "", channel::GENERAL, "still there?"),
    );
    assert!(lines(&world.take(bob)).is_empty());
    assert_eq!(lines(&world.take(carol)).len(), 1);

    // A flood is cut: ten lines at once, then one a second.
    for _ in 0..30 {
        let (id, body) = request::chat(2, "", channel::GENERAL, "spam");
        world.client(carol).send(id, &body);
    }
    world.settle();
    assert_eq!(lines(&world.take(alice)).len(), 10);
    world.advance(Duration::from_secs(3));
    world.ask(carol, request::chat(2, "", channel::GENERAL, "sorry"));
    assert_eq!(lines(&world.take(alice)).len(), 1);
    world.assert_clean();
}

#[test]
fn player_menu_mute_by_name_and_unmute_filter_incoming_chat() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    for client in [alice, bob, carol] {
        world.ask(client, request::subscribe());
        world.take(client);
    }
    let lines = |decoded: Vec<Decoded>| {
        decoded
            .into_iter()
            .filter(|message| matches!(message, Decoded::ChatText { .. }))
            .count()
    };
    // add_ignore_name is what the room/ranking menu sends: action 4, string.
    world.ask(bob, (0x4E, vec![4, 5, b'A', b'l', b'i', b'c', b'e']));
    let alice_id = world.ids[alice].0;
    assert!(matches!(
        &world.take(bob)[..],
        [Decoded::Friendship(FriendshipPush::Ignored(ignored))]
            if ignored[..] == [(alice_id, "Alice".to_owned())]
    ));
    world.ask(alice, request::chat(2, "", channel::GENERAL, "muted"));
    world.ask(
        alice,
        request::chat(0, "Bob", channel::PRIVATE, "also muted"),
    );
    assert_eq!(lines(world.take(bob)), 0);
    assert_eq!(lines(world.take(carol)), 1);
    world.ask(
        carol,
        request::chat(2, "", channel::GENERAL, "still visible"),
    );
    assert_eq!(lines(world.take(bob)), 1);

    // remove_ignored takes the account id from the pushed list, not the row id.
    let mut unmute = vec![5];
    unmute.extend(alice_id.to_le_bytes());
    world.ask(bob, (0x4E, unmute));
    assert!(matches!(
        &world.take(bob)[..],
        [Decoded::Friendship(FriendshipPush::Ignored(ignored))] if ignored.is_empty()
    ));
    world.ask(alice, request::chat(2, "", channel::GENERAL, "unmuted"));
    world.ask(
        alice,
        request::chat(0, "Bob", channel::PRIVATE, "private too"),
    );
    assert_eq!(lines(world.take(bob)), 2);
    world.assert_clean();
}

#[test]
fn players_in_a_match_talk_on_its_channels() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(PLAY_TO_MATCH);
    let (match_id, alice_team) = sent_to_match(&world.take(alice)).unwrap();
    world.take(bob);
    world.take(carol);
    let said = |decoded: &[Decoded]| {
        decoded
            .iter()
            .filter(|message| matches!(message, Decoded::ChatText { .. }))
            .count()
    };
    world.ask(alice, request::chat(match_id, "", channel::MATCH, "gl"));
    assert_eq!(said(&world.take(bob)), 1);
    assert_eq!(said(&world.take(carol)), 0);
    // The team channel: Bob is on the other team.
    let team_channel = channel::TEAM_1 + alice_team;
    world.ask(alice, request::chat(match_id, "", team_channel, "go left"));
    assert_eq!(said(&world.take(bob)), 0);
    world.assert_clean();
}

#[test]
fn a_friend_request_makes_friends_and_their_statuses_follow() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);
    world.take(alice);
    world.take(bob);

    // Alice finds Bob and asks.
    world.ask(alice, request::find_players("bo"));
    assert!(matches!(
        &world.take(alice)[..],
        [Decoded::Friendship(FriendshipPush::Found { action: 6, players })]
            if players.len() == 1 && players[0].0 == bob_id && players[0].1 == "Bob"
    ));
    world.ask(alice, request::important("Bob", 0, ""));
    let decoded = world.take(bob);
    let [
        Decoded::Important {
            sender_type: 5,
            sender_id,
            sender_name,
            kind: 0,
            message_id,
            ..
        },
    ] = &decoded[..]
    else {
        panic!("{decoded:?}");
    };
    assert_eq!((*sender_id, sender_name.as_str()), (alice_id, "Alice"));
    // Asking again while it waits sends nothing more.
    world.ask(alice, request::important("Bob", 0, ""));
    assert!(world.take(bob).is_empty());

    // Bob accepts: both get their friend list, with the other in the lobby.
    let message_id = *message_id;
    assert!(world.client(bob).model.answer_important(message_id));
    world.ask(bob, request::answer(message_id, 0));
    let friends = |decoded: &[Decoded]| -> Option<Vec<(u32, String, u8)>> {
        find(decoded, |message| match message {
            Decoded::Friendship(FriendshipPush::Friends(friends)) => Some(
                friends
                    .iter()
                    .map(|friend| (friend.account_id, friend.name.clone(), friend.status))
                    .collect(),
            ),
            _ => None,
        })
    };
    assert_eq!(
        friends(&world.take(alice)),
        Some(vec![(bob_id, "Bob".to_owned(), 0)])
    );
    assert_eq!(
        friends(&world.take(bob)),
        Some(vec![(alice_id, "Alice".to_owned(), 0)])
    );
    assert_eq!(
        world.store.account(alice_id).unwrap().locked().friends,
        [bob_id]
    );

    // Bob goes into a match: Alice's client is told his status, with
    // exactly the friends it holds.
    let carol = world.connect("Carol");
    world.ask(bob, request::play(1));
    world.ask(carol, request::play(1));
    world.advance(PLAY_TO_MATCH);
    let statuses = |decoded: &[Decoded]| -> Vec<Vec<(u32, u8)>> {
        decoded
            .iter()
            .filter_map(|message| match message {
                Decoded::Friendship(FriendshipPush::Statuses(statuses)) => Some(statuses.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(statuses(&world.take(alice)), [vec![(bob_id, 1)]]);

    // Bob's game goes away: offline.
    world.clients[bob].1.leave();
    world.advance(Duration::from_secs(3));
    assert_eq!(statuses(&world.take(alice)), [vec![(bob_id, 2)]]);
    assert!(world.store.has_ended(world.ids[bob].1));

    // He signs in again and has his friend, kept with the account.
    let bob_again = world.connect_session((bob_id, world.new_session(bob_id)));
    assert_eq!(
        friends(&world.take(bob_again)),
        Some(vec![(alice_id, "Alice".to_owned(), 0)])
    );
    world.advance(Duration::from_secs(2));
    assert_eq!(statuses(&world.take(alice)), [vec![(bob_id, 0)]]);

    // Removing a friend removes both ways.
    world.ask(alice, request::remove_friend(bob_id));
    assert_eq!(friends(&world.take(alice)), Some(Vec::new()));
    assert_eq!(friends(&world.take(bob_again)), Some(Vec::new()));
    assert!(
        world
            .store
            .account(bob_id)
            .unwrap()
            .locked()
            .friends
            .is_empty()
    );
    world.assert_clean();
}

#[test]
fn a_friend_request_waits_for_a_player_who_is_away() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob_ids = signed_in(world.store, "Bob");
    world.ask(alice, request::important("Bob", 0, ""));
    let bob = world.connect_session(bob_ids);
    assert!(world.take(bob).iter().any(|message| matches!(
        message,
        Decoded::Important { kind: 0, sender_name, .. } if sender_name == "Alice"
    )));
    world.assert_clean();
}

/// A friend request is sent within the sender's allowance: five at once,
/// then one more every ten seconds (the Tab list's menu keeps the same on
/// the client, exe patch 86); one over it is dropped.
#[test]
fn friend_requests_come_five_at_once_then_one_every_ten_seconds() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let names = ["Bob", "Carol", "Dave", "Erin", "Frank", "Grace"];
    let others: Vec<usize> = names.iter().map(|name| world.connect(name)).collect();
    world.take(alice);
    for &other in &others {
        world.take(other);
    }
    let asked = |world: &mut World, other: usize| {
        world.take(other).iter().any(|message| {
            matches!(message, Decoded::Important { kind: 0, sender_name, .. } if sender_name == "Alice")
        })
    };
    for (name, &other) in names.iter().zip(&others).take(5) {
        world.ask(alice, request::important(name, 0, ""));
        assert!(asked(&mut world, other), "{name}");
    }
    let grace = others[5];
    world.ask(alice, request::important("Grace", 0, ""));
    assert!(!asked(&mut world, grace), "the sixth at once");
    // (the asks themselves took a little of the test's time)
    world.advance(Duration::from_secs(5));
    world.ask(alice, request::important("Grace", 0, ""));
    assert!(!asked(&mut world, grace), "five seconds after");
    world.advance(Duration::from_secs(5));
    world.ask(alice, request::important("Grace", 0, ""));
    assert!(asked(&mut world, grace), "ten seconds after");
    world.assert_clean();
}

/// Alice invites Bob into her squad, as the clients do it.
fn form_squad(world: &mut World, alice: usize, bob: usize) {
    world.take(alice);
    world.take(bob);
    world.ask(alice, request::important("Bob", 2, "invite"));
    let invite = find(&world.take(bob), |message| match message {
        Decoded::Important {
            kind: 2,
            message_id,
            sender_name,
            ..
        } if sender_name == "Alice" => Some(*message_id),
        _ => None,
    })
    .expect("the invite");
    // Bob's client: "membership accepted" to the inviter, then the answer
    // (0x28C698, then 0x28C7E1).
    world.ask(bob, request::important("Alice", 3, "membership accepted"));
    world.ask(bob, request::answer(invite, 0));
    let accepted = find(&world.take(alice), |message| match message {
        Decoded::Important {
            kind: 3,
            message_id,
            sender_name,
            ..
        } if sender_name == "Bob" => Some(*message_id),
        _ => None,
    })
    .expect("the acceptance");
    // Alice's client answers it by itself and adds the player.
    world.ask(alice, request::answer(accepted, 0));
    world.ask(alice, request::add_squad_member("Bob"));
}

/// The ids of the messages of `kind` from `sender` among `messages`.
fn importants(messages: &[Decoded], kind: u8, sender: &str) -> Vec<u32> {
    messages
        .iter()
        .filter_map(|message| match message {
            Decoded::Important {
                kind: got,
                message_id,
                sender_name,
                ..
            } if *got == kind && sender_name == sender => Some(*message_id),
            _ => None,
        })
        .collect()
}

/// Live on 2026-10-12: every first accept of a squad invite was lost (12
/// of 12), because the acceptance was passed on only after the answer and
/// a client sends it before. Either order forms the squad, once.
#[test]
fn an_accepted_squad_invite_reaches_the_inviter_in_either_order() {
    for answer_first in [false, true] {
        let mut world = World::new();
        let alice = world.connect("Alice");
        let bob = world.connect("Bob");
        world.take(alice);
        world.take(bob);
        world.ask(alice, request::important("Bob", 2, "invite"));
        let invite = importants(&world.take(bob), 2, "Alice")[0];
        if answer_first {
            world.ask(bob, request::answer(invite, 0));
            assert!(importants(&world.take(alice), 3, "Bob").is_empty());
            world.ask(bob, request::important("Alice", 3, "membership accepted"));
        } else {
            world.ask(bob, request::important("Alice", 3, "membership accepted"));
            assert!(importants(&world.take(alice), 3, "Bob").is_empty());
            world.ask(bob, request::answer(invite, 0));
        }
        let accepted = importants(&world.take(alice), 3, "Bob");
        assert_eq!(accepted.len(), 1, "answer first: {answer_first}");
        world.ask(alice, request::answer(accepted[0], 0));
        world.ask(alice, request::add_squad_member("Bob"));
        let squad = world
            .store
            .squads()
            .of(world.ids[alice].0)
            .expect("a squad");
        assert_eq!(squad.members, [world.ids[alice].0, world.ids[bob].0]);
    }
}

/// An invite that is declined tells the inviter nothing, whatever the
/// client said before the answer.
#[test]
fn a_declined_squad_invite_is_not_an_acceptance() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.take(bob);
    world.ask(alice, request::important("Bob", 2, "invite"));
    let invite = importants(&world.take(bob), 2, "Alice")[0];
    world.ask(bob, request::important("Alice", 3, "membership accepted"));
    world.ask(bob, request::answer(invite, 1));
    assert!(importants(&world.take(alice), 3, "Bob").is_empty());
    world.ask(alice, request::add_squad_member("Bob"));
    assert!(world.store.squads().of(world.ids[alice].0).is_none());
}

/// An invite whose player's game was started again is lost with that game:
/// the next invite of the pair is the same one sent again, under its id,
/// and can be answered. (It was dropped, for as long as srp ran.)
#[test]
fn a_squad_invite_that_waits_is_sent_again() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);
    world.take(alice);
    world.take(bob);
    world.ask(alice, request::important("Bob", 2, "invite"));
    let invite = importants(&world.take(bob), 2, "Alice")[0];
    // Bob's game is started again: a new session, and no invite in it.
    let bob_again = world.connect_session((bob_id, world.new_session(bob_id)));
    assert!(importants(&world.take(bob_again), 2, "Alice").is_empty());
    world.ask(alice, request::important("Bob", 2, "invite"));
    assert_eq!(importants(&world.take(bob_again), 2, "Alice"), [invite]);
    world.ask(
        bob_again,
        request::important("Alice", 3, "membership accepted"),
    );
    world.ask(bob_again, request::answer(invite, 0));
    assert_eq!(importants(&world.take(alice), 3, "Bob").len(), 1);
    world.ask(alice, request::add_squad_member("Bob"));
    let squad = world.store.squads().of(alice_id).expect("a squad");
    assert_eq!(squad.members, [alice_id, bob_id]);
}

#[test]
fn a_squad_info_carries_the_lobby_picks() {
    use survarium::v026g::lobby::lobby_pick::LOBBY_PICK_BASE;
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let picks = |decoded: &[Decoded]| {
        decoded.iter().rev().find_map(|message| match message {
            Decoded::SquadInfo { squad_id, picks, .. } => Some((*squad_id != 0, *picks)),
            _ => None,
        })
    };
    let pick = |world: &mut World, who, id: u32| world.ask(who, request::ranking(LOBBY_PICK_BASE + id, 1));
    // Out of a squad: the own pick alone, sent back at once; nothing answered else.
    world.take(alice);
    pick(&mut world, alice, 4);
    assert_eq!(picks(&world.take(alice)), Some((false, [4, 0])));
    pick(&mut world, bob, 3);
    form_squad(&mut world, alice, bob);
    // In it: each its own and the commander's (Alice's).
    assert_eq!(picks(&world.take(alice)), Some((true, [4, 4])));
    assert_eq!(picks(&world.take(bob)), Some((true, [3, 4])));
    // The commander's change reaches every member; a member's own changes its own byte only.
    pick(&mut world, alice, 2);
    assert_eq!(picks(&world.take(bob)), Some((true, [3, 2])));
    world.take(alice);
    pick(&mut world, bob, 1);
    assert_eq!(picks(&world.take(bob)), Some((true, [1, 2])));
    assert_eq!(picks(&world.take(alice)), None, "the commander's squad info is as it was");
    // A start past the picks is no pick.
    pick(&mut world, bob, 5);
    assert_eq!(picks(&world.take(bob)), None);
}

#[test]
fn a_squad_of_two_forms_shows_each_mate_and_plays_together() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);

    // "Membership accepted" without an invite is not passed on.
    world.take(alice);
    world.ask(bob, request::important("Alice", 3, "membership accepted"));
    assert!(world.take(alice).is_empty());
    world.ask(alice, request::add_squad_member("Bob"));
    assert!(world.store.squads().of(alice_id).is_none());

    form_squad(&mut world, alice, bob);
    let squad = |decoded: &[Decoded]| {
        decoded.iter().rev().find_map(|message| match message {
            Decoded::SquadInfo {
                squad_id,
                members,
                commander_index,
                i_am_commander,
                ..
            } => Some((
                *squad_id,
                members.clone(),
                *commander_index,
                *i_am_commander,
            )),
            _ => None,
        })
    };
    let (id, members, commander, alice_commands) = squad(&world.take(alice)).expect("the squad");
    assert_ne!(id, 0);
    assert_eq!((commander, alice_commands), (0, true));
    assert_eq!(
        members
            .iter()
            .map(|member| (
                member.account_id,
                member.name.as_str(),
                member.online,
                member.ready
            ))
            .collect::<Vec<_>>(),
        // Both free in the lobby (no room, a kit that plays): ready, so that
        // the commander's Play takes the other along.
        [
            (alice_id, "Alice", true, true),
            (bob_id, "Bob", true, true)
        ]
    );
    // Her own entry has her own kit's id; Bob's an id that is none of
    // hers, so her client asks for him.
    assert_eq!(members[0].kit_id, 1);
    let bob_kit = members[1].kit_id;
    assert!(![1, 2, 3].contains(&bob_kit));
    let (_, members, _, bob_commands) = squad(&world.take(bob)).expect("the squad");
    assert!(!bob_commands);
    assert_eq!(members[1].kit_id, 1);

    // The mate's character: one kit, filed under the id of 0xA1 and with
    // its revision, or the client would ask again for ever (0x227C59).
    world.ask(alice, request::squad_member(bob_id, bob_kit));
    let decoded = world.take(alice);
    let [Decoded::SquadMemberAccount { body, revision }] = &decoded[..] else {
        panic!("{decoded:?}");
    };
    assert_eq!(body.name, "Bob");
    assert_eq!(body.kits.len(), 1);
    assert_eq!(body.kits[0].id, bob_kit);
    let listed = world.store.squads().of(alice_id).unwrap();
    assert_eq!(listed.members, [alice_id, bob_id]);
    // Somebody outside the squad is not shown.
    world.ask(carol, request::squad_member(bob_id, bob_kit));
    assert!(
        world
            .take(carol)
            .iter()
            .all(|message| !matches!(message, Decoded::SquadMemberAccount { .. }))
    );
    // Bob wears something else: the revision of 0xA1 moves, and the
    // answer to the client's next question carries the same one.
    world.ask(bob, request::move_item(1, 1, 18, 7, INVENTORY_SLOT, 1));
    world.ask(bob, request::move_item(1, 1, 18, INVENTORY_SLOT, 7, 1));
    let (_, members, _, _) = squad(&world.take(alice)).expect("the squad again");
    assert_ne!(members[1].kit_revision, *revision);
    world.ask(alice, request::squad_member(bob_id, bob_kit));
    assert!(matches!(
        &world.take(alice)[..],
        [Decoded::SquadMemberAccount { revision, .. }] if *revision == members[1].kit_revision
    ));

    // The squad channel.
    world.ask(alice, request::chat(id, "", channel::SQUAD, "ready?"));
    assert!(world.take(bob).iter().any(|message| matches!(
        message,
        Decoded::ChatText { channel: channel::SQUAD, body, .. } if body == "ready?"
    )));

    // Bob's Play makes a room and takes Alice along (Alice's Play then
    // finds her in it), and Carol's goes into it too: the one open room.
    // When its match starts the squad is on one team, Carol on the other.
    for client in [bob, alice, carol] {
        world.ask(client, request::play(1));
    }
    let room = world.core.rooms.of(bob_id).expect("Bob's room").key;
    for client in [alice, carol] {
        assert_eq!(
            world
                .core
                .rooms
                .of(world.ids[client].0)
                .map(|room| room.key),
            Some(room)
        );
    }
    world.advance(PLAY_TO_MATCH);
    let (alice_match, alice_team) = sent_to_match(&world.take(alice)).expect("Alice's match");
    let (bob_match, bob_team) = sent_to_match(&world.take(bob)).expect("Bob's match");
    let (carol_match, carol_team) = sent_to_match(&world.take(carol)).expect("Carol's match");
    assert_eq!((alice_match, alice_team), (bob_match, bob_team));
    assert_eq!(carol_match, alice_match);
    assert_ne!(carol_team, alice_team);
    let made = world.host.made.lock().unwrap().clone();
    let squads: Vec<(u32, u8)> = made[0]
        .1
        .players
        .iter()
        .map(|player| (player.account_id, player.profile.squad))
        .collect();
    let squad_of = |account_id: u32| squads.iter().find(|(id, _)| *id == account_id).unwrap().1;
    assert_ne!(squad_of(alice_id), 0);
    assert_eq!(squad_of(alice_id), squad_of(bob_id));
    assert_eq!(squad_of(world.ids[carol].0), 0);
    world.assert_clean();
}

#[test]
fn leaving_a_squad_tells_everyone() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    form_squad(&mut world, alice, bob);
    world.take(alice);
    world.take(bob);

    world.ask(bob, request::leave_squad());
    for client in [alice, bob] {
        let decoded = world.take(client);
        assert!(
            decoded
                .iter()
                .any(|message| matches!(message, Decoded::SquadInfo { squad_id: 0, .. })),
            "{decoded:?}"
        );
    }
    assert!(world.store.squads().of(world.ids[alice].0).is_none());
    world.assert_clean();
}

#[test]
fn a_second_sign_in_of_an_account_signs_the_first_out() {
    let mut world = World::new();
    let first = world.connect("Alice");
    let account_id = world.ids[first].0;
    world.take(first);
    let second = world.connect_session((account_id, world.new_session(account_id)));
    // The old game is told (`AA`: "the user is already in the game") and
    // disconnected; its session is over.
    let decoded = world.take(first);
    assert!(matches!(decoded[..], [Decoded::SignedOut]), "{decoded:?}");
    world.advance(Duration::from_secs(2));
    assert!(world.client(first).disconnected);
    assert!(world.store.has_ended(world.ids[first].1));
    assert!(world.client(second).model.is_signed_in());
    assert!(!world.store.has_ended(world.ids[second].1));
    assert_eq!(world.store.players_online(), 1);
    assert_eq!(world.core.links.len(), 1);
    world.assert_clean();
}

#[test]
fn a_session_ended_elsewhere_signs_its_client_out() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    // A ban ends the session in the store.
    world.store.ban_account(
        world.ids[alice].0,
        session::Ban {
            until: None,
            reason: "test".to_owned(),
        },
    );
    world.advance(Duration::from_secs(3));
    assert!(matches!(world.take(alice)[..], [Decoded::SignedOut]));
    assert!(world.client(alice).disconnected);
    assert!(world.core.links.is_empty());
    assert_eq!(world.store.players_online(), 0);
}

#[test]
fn a_silent_client_loses_its_link_and_its_session() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.ask(alice, request::play(1));
    let (alice_session, bob_session) = (world.ids[alice].1, world.ids[bob].1);
    // Alice's game is gone without a word (Alt+F4, a lost connection).
    let (_, gone) = world.clients.remove(alice);
    drop(gone);
    world.advance(Duration::from_secs(29));
    assert_eq!(world.core.links.len(), 2);
    assert!(!world.store.has_ended(alice_session));
    // The link goes after 30 s, and its player is out of the queue; the
    // session waits a minute more for the client to connect again
    // (`away_tests`).
    world.advance(Duration::from_secs(2));
    assert_eq!(world.core.links.len(), 1);
    assert!(world.store.matchmaking().busy_sessions().is_empty());
    assert_eq!(world.store.players_online(), 1);
    assert!(!world.store.has_ended(alice_session));
    world.advance(Duration::from_secs(61));
    assert!(world.store.has_ended(alice_session));
    assert!(world.store.hold(alice_session).is_none());
    assert_eq!(world.core.links.len(), 1);
    assert_eq!(world.store.players_online(), 1);
    // Bob, who kept talking, is still there, and sees one player online.
    assert!(!world.store.has_ended(bob_session));
    assert_eq!(world.clients[0].1.online.last(), Some(&(1, 0, 1440)));
    world.assert_clean();
}

#[test]
fn a_client_that_leaves_ends_its_session_at_once() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.client(alice).leave();
    world.settle();
    assert!(world.client(alice).disconnected);
    assert!(world.store.has_ended(world.ids[alice].1));
    assert!(world.core.links.is_empty());
}

/// The admin page's moderation (`session::v026g::moderation`): what it
/// takes is gone from the open game's inventory and kits at once, and the
/// game is sent nothing it would drop or crash on.
#[test]
fn an_item_the_admin_takes_leaves_the_open_game() {
    use session::v026g::moderation::{Pick, plan_take, take};
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    let (pistol, ammo) = {
        let account = world.account(alice);
        (account.inventory[0].item_id, account.inventory[1].item_id)
    };
    assert!(world.account(alice).kits[0].slot_of(pistol).is_some());
    world.change(alice, |account| {
        let plan = plan_take(
            account,
            &[
                Pick {
                    item_id: pistol,
                    units: None,
                },
                Pick {
                    item_id: ammo,
                    units: Some(1),
                },
            ],
        )
        .unwrap();
        take(account, &plan);
    });
    let seen = world.clients[alice].1.model.inventory();
    assert!(seen.iter().all(|item| item.item_id != pistol));
    assert_eq!(seen.len(), world.account(alice).inventory.len());
    assert_eq!(world.account(alice).kits[0].slot_of(pistol), None);
    world.assert_clean();
}

#[test]
fn other_servers_reach_a_client_through_the_inbox() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    let account_id = world.ids[alice].0;
    // A change made elsewhere: the client gets the account again.
    world.account(alice).generic_money = 123_456;
    world.inbox.account_changed(account_id);
    world.settle();
    assert_eq!(
        world.client(alice).ids,
        [0x89, 0x87, 0x8B, 0x8C, 0x8E, 0x92]
    );
    assert!(world.take(alice).iter().any(|message| matches!(
        message,
        Decoded::Money {
            generic: 123_456,
            ..
        }
    )));
    // Messages for an account: sent in order if it is in the lobby.
    world.inbox.push(
        account_id,
        vec![
            survarium::v026g::lobby::money(1, 2),
            survarium::v026g::lobby::match_stats_notice(77),
        ],
    );
    world
        .inbox
        .push(9_999, vec![survarium::v026g::lobby::money(3, 4)]);
    world.settle();
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Money {
            generic: 1,
            premium: 2
        }
    ));
    assert!(matches!(
        &decoded[1],
        Decoded::ChatText { sender_type: 8, body, .. } if body == "#mstats [77]"
    ));
    // The balance changed with no word to the lobby (the web page): the
    // client is told within a second.
    world.account(alice).premium_money = 5;
    world.advance(Duration::from_secs(2));
    assert!(
        world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::Money { premium: 5, .. }))
    );
    world.assert_clean();
}

#[test]
fn hundreds_of_ordered_pushes_through_a_lossy_link_all_arrive_in_order() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    // A third of the server's datagrams never arrive. The client drops an
    // ordered message that is 128 or more past the one it waits for while
    // still acking its datagram, which would stop the channel for good
    // (transport.md §4.1): the transport must hold the later ones back.
    world.client(alice).loss_percent = 33;
    let pushes: Vec<_> = (0..400)
        .map(|index| survarium::v026g::lobby::money(index, 0))
        .collect();
    world.inbox.push(world.ids[alice].0, pushes);
    world.advance(Duration::from_secs(40));
    let client = world.client(alice);
    assert!(client.dropped > 100, "{} datagrams lost", client.dropped);
    assert_eq!(client.faults, Vec::<String>::new());
    assert_eq!(client.waiting_in_order(), 0);
    let amounts: Vec<u32> = world
        .take(alice)
        .iter()
        .filter_map(|message| match message {
            Decoded::Money {
                generic,
                premium: 0,
            } => Some(*generic),
            _ => None,
        })
        .collect();
    assert_eq!(amounts, (0..400).collect::<Vec<u32>>());
    // And the link still works.
    world.client(alice).loss_percent = 0;
    world.ask(alice, request::autobuy(true));
    assert!(
        world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::Permitted(Permitted::Autobuy(true))))
    );
}

#[test]
fn the_online_counts_are_repeated_for_a_client_that_missed_them() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    // 0x9F is sent once, never again by the transport; until one arrives
    // the client's Play is off. Nothing changes here, yet they keep coming.
    let before = world.client(alice).online.len();
    world.advance(Duration::from_secs(16));
    let after = world.client(alice).online.len();
    assert!(
        (before + 3..=before + 4).contains(&after),
        "{before} then {after}"
    );
    // Nothing else is repeated.
    world.take(alice);
    world.advance(Duration::from_secs(6));
    assert!(world.take(alice).is_empty());
    world.assert_clean();
}

#[test]
fn a_client_whose_link_broke_connects_again_with_its_session() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let ids = world.ids[alice];
    // The client gave up on its link (5 s without a datagram) and connects
    // again, from a new port, while the server's link still stands.
    world.clients[alice].1 = TestClient::new();
    world.clients[alice].0 = "127.0.0.1:49999".parse().unwrap();
    world.client(alice).sign_in(ids.1, false);
    world.settle();
    assert!(world.client(alice).model.is_signed_in());
    // It has the shop already: no 0x95 this time.
    assert!(!world.client(alice).ids.contains(&0x95));
    assert!(!world.store.has_ended(ids.1));
    world.advance(Duration::from_secs(3));
    assert_eq!(world.core.links.len(), 2);
    assert_eq!(world.store.players_online(), 2);

    // In a match, a link that goes silent does not cost the session: the
    // client comes back to the lobby with it.
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(PLAY_TO_MATCH);
    assert!(sent_to_match(&world.take(alice)).is_some());
    let silent = std::mem::replace(&mut world.clients[alice].1, TestClient::new());
    drop(silent);
    world.clients[alice].0 = "127.0.0.1:49998".parse().unwrap();
    // (A player in a match has two minutes: `away_tests`.)
    world.advance(Duration::from_secs(125));
    assert_eq!(world.core.links.len(), 1);
    assert!(!world.store.has_ended(ids.1));
    world.client(alice).sign_in(ids.1, false);
    world.settle();
    assert!(world.client(alice).model.is_signed_in());
    world.assert_clean();
}

// ------------------------------------------------- results and rewards

impl World {
    /// The match server's result for a match the lobby made, settled as
    /// `servers/srp` does it: team `winner` brought the batteries home,
    /// everyone was in to the end, the first player made three kills.
    fn end_match(&mut self, match_id: u32, winner: u8) {
        let setup = self
            .host
            .made
            .lock()
            .unwrap()
            .iter()
            .find(|(id, _)| *id == match_id)
            .map(|(_, setup)| setup.clone())
            .expect("a match the lobby made");
        let players = setup
            .players
            .iter()
            .enumerate()
            .map(|(place, player)| {
                let mut statistics = PlayerStatistics::default();
                if place == 0 {
                    statistics.events[event::KILL][0] = 3;
                }
                PlayerResult {
                    place: place as u8,
                    session_id: player.session_id,
                    account_id: player.account_id,
                    name: player.profile.name.clone(),
                    team: player.profile.team,
                    driven: player.driven,
                    entered: true,
                    present_at_end: true,
                    left: false,
                    time_in_match_ms: 600_000,
                    score: if place == 0 { 300 } else { 0 },
                    kills: if place == 0 { 3 } else { 0 },
                    deaths: 0,
                    longest_killstreak: 0,
                    kit_changes: 0,
                    statistics: Some(statistics),
                    pay_statistics: None,
                    supplies: None,
                }
            })
            .collect();
        let result = MatchResult {
            match_id,
            map_id: setup.map_id,
            game_mode: maps::map_mode(setup.map_id).unwrap().mode,
            match_level: setup.match_level,
            end: MatchEnd::ModeRule,
            winner: Some(winner),
            team_points: if winner == 0 { [3, 1] } else { [1, 3] },
            duration_ms: 700_000,
            played_ms: Some(600_000),
            trusted_snapshot_ms: Some(690_000),
            trusted_by: 2,
            players,
            vs_bots: setup.vs_bots,
            private: setup.private,
            blackout: setup.blackout,
        };
        let terms = Terms::at(session::v026g::quests::now());
        let report = self
            .results
            .settle(&result, self.accounts, &terms, &mut Rng::new(7));
        assert!(report.is_some());
        self.accounts.flush();
        for player in &result.players {
            self.inbox.account_changed(player.account_id);
        }
        self.host.open.lock().unwrap().remove(&match_id);
        self.inbox.match_finished(MatchFinished {
            match_id,
            not_made: false,
        });
    }

    /// The match server called a room's match off before its countdown
    /// (`MatchEnd::NotMade`), settled as `servers/srp` does it: nobody
    /// played, nothing is settled.
    fn call_off(&mut self, match_id: u32) {
        let setup = self
            .host
            .made
            .lock()
            .unwrap()
            .iter()
            .find(|(id, _)| *id == match_id)
            .map(|(_, setup)| setup.clone())
            .expect("a match the lobby made");
        let players = setup
            .players
            .iter()
            .enumerate()
            .map(|(place, player)| PlayerResult {
                place: place as u8,
                session_id: player.session_id,
                account_id: player.account_id,
                name: player.profile.name.clone(),
                team: player.profile.team,
                driven: player.driven,
                entered: place == 0,
                present_at_end: place == 0,
                left: false,
                time_in_match_ms: 20_000,
                score: 0,
                kills: 0,
                deaths: 0,
                longest_killstreak: 0,
                kit_changes: 0,
                statistics: None,
                pay_statistics: None,
                supplies: None,
            })
            .collect();
        let result = MatchResult {
            match_id,
            map_id: setup.map_id,
            game_mode: maps::map_mode(setup.map_id).unwrap().mode,
            match_level: setup.match_level,
            end: MatchEnd::NotMade,
            winner: None,
            team_points: [0, 0],
            duration_ms: 20_000,
            played_ms: None,
            trusted_snapshot_ms: None,
            trusted_by: 0,
            players,
            vs_bots: setup.vs_bots,
            private: setup.private,
            blackout: setup.blackout,
        };
        let terms = Terms::at(session::v026g::quests::now());
        let report = self
            .results
            .settle(&result, self.accounts, &terms, &mut Rng::new(7));
        assert!(report.is_none(), "nothing of a match called off is settled");
        self.host.open.lock().unwrap().remove(&match_id);
        self.inbox.match_finished(MatchFinished {
            match_id,
            not_made: true,
        });
    }

    /// Two players in a match: both press Play, which puts them into one
    /// room, and its match starts (after a match of the room, its vote is
    /// longer). `(match id, the first one's team)`.
    fn match_of(&mut self, first: usize, second: usize) -> (u32, u8) {
        self.ask(first, request::play(1));
        self.ask(second, request::play(1));
        for _ in 0..PLAY_TO_MATCH.as_secs() * 2 {
            if sent_to_match(&self.client(first).decoded).is_some() {
                break;
            }
            self.advance(Duration::from_secs(1));
        }
        self.settle();
        let sent = sent_to_match(&self.take(first)).expect("a match");
        assert_eq!(
            sent_to_match(&self.take(second)).map(|(id, _)| id),
            Some(sent.0)
        );
        sent
    }
}

/// The results screens taken: `(match id, the player's id, rows)`.
fn screens(decoded: &[Decoded]) -> Vec<(u32, u32, usize)> {
    decoded
        .iter()
        .filter_map(|message| match message {
            Decoded::MatchStats { stats, own } => {
                Some((stats.match_id, own.player_id, stats.rows.len()))
            }
            _ => None,
        })
        .collect()
}

fn position(decoded: &[Decoded], pick: impl Fn(&Decoded) -> bool) -> Option<usize> {
    decoded.iter().position(pick)
}

#[test]
fn a_finished_match_pays_its_players_and_shows_them_the_results() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let (match_id, alice_team) = world.match_of(alice, bob);
    let before = world.account(alice).clone();

    world.end_match(match_id, alice_team);
    world.settle();
    for (place, client) in [alice, bob].into_iter().enumerate() {
        let decoded = world.take(client);
        // Back in the lobby; then what the match changed of the account
        // (the inventory before the account), and only then the screen,
        // which reads its bars from what the client holds.
        let lobby = position(&decoded, |message| {
            matches!(
                message,
                Decoded::ClientState {
                    state: ClientState::Lobby,
                    ..
                }
            )
        })
        .expect("85 00");
        let screen = position(&decoded, |message| {
            matches!(message, Decoded::MatchStats { .. })
        })
        .expect("the results screen");
        assert!(lobby < screen, "{decoded:?}");
        let between = &decoded[lobby..screen];
        let at = |pick: fn(&Decoded) -> bool| position(between, pick).expect("a push");
        let inventory = at(|message| matches!(message, Decoded::Inventory { .. }));
        let account = at(|message| matches!(message, Decoded::Account(_)));
        assert!(inventory < account);
        at(|message| matches!(message, Decoded::Money { .. }));
        at(|message| matches!(message, Decoded::Skills { .. }));
        at(|message| matches!(message, Decoded::Quests(_)));
        at(|message| matches!(message, Decoded::PlayerElo { .. }));
        // Once, for this match, for this player (its roster place + 1).
        assert_eq!(screens(&decoded), [(match_id, place as u32 + 1, 2)]);
        // The money the client was last told is what the account has.
        let told = decoded.iter().rev().find_map(|message| match message {
            Decoded::Money { generic, premium } => Some((*generic, *premium)),
            _ => None,
        });
        let account = world.account(client);
        assert_eq!(told, Some((account.generic_money, account.premium_money)));
        assert_eq!(
            world.clients[client].1.model.inventory().len(),
            account.inventory.len()
        );
    }
    // Alice won with three kills: paid, and rated up.
    {
        let after = world.account(alice);
        assert!(after.generic_money > before.generic_money);
        assert!(after.exp_total > before.exp_total);
        assert!(after.rating > before.rating);
    }
    // Shown once.
    world.advance(Duration::from_secs(3));
    assert!(screens(&world.take(alice)).is_empty());
    assert!(screens(&world.take(bob)).is_empty());

    // `41 00 <match>`: the screen again. `3D 00 <player> <match>`: the
    // kits of a row.
    world.ask(alice, request::match_stats(match_id));
    let decoded = world.take(alice);
    assert_eq!(screens(&decoded), [(match_id, 1, 2)]);
    world.ask(alice, request::player_details(2, match_id));
    let decoded = world.take(alice);
    assert!(
        matches!(
            &decoded[..],
            [Decoded::PlayerDetails { player, .. }]
                if player.name == "Bob" && player.kits.len() == 3
        ),
        "{decoded:?}"
    );
    // Not a row, not a match kept, not a player of the match: no answer.
    world.ask(alice, request::player_details(3, match_id));
    world.ask(alice, request::match_stats(match_id + 1));
    assert!(world.take(alice).is_empty());
    world.take(carol);
    world.ask(carol, request::match_stats(match_id));
    world.ask(carol, request::player_details(1, match_id));
    assert!(world.take(carol).is_empty());

    // The screen's "buy premium" (`47 02 <offer> <match>`): three days
    // (offer 1) for 600 gold, and the premium half the screen promised.
    let half = world
        .results
        .find(match_id)
        .unwrap()
        .player(world.ids[alice].0)
        .unwrap()
        .reward
        .premium_bonus;
    // A new account has no gold: this one is given some.
    world.change(alice, |account| account.premium_money = 1_000);
    let paid = world.account(alice).clone();
    world.ask(alice, request::buy_offer(1, match_id));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::OfferBought)
    ));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Services {
            premium_hours: Some(72)
        }
    )));
    {
        let account = world.account(alice);
        assert_eq!(account.premium_money, paid.premium_money - 200);
        assert!(account.generic_money >= paid.generic_money + half.0);
        assert_eq!(account.exp_total, paid.exp_total + half.1);
    }
    // Once: a second purchase adds time and no second half.
    let paid = world.account(alice).clone();
    world.change(alice, |account| account.premium_money += 600);
    world.ask(alice, request::buy_offer(1, match_id));
    assert!(world.take(alice).iter().any(|message| matches!(
        message,
        Decoded::Services {
            premium_hours: Some(144)
        }
    )));
    assert_eq!(world.account(alice).exp_total, paid.exp_total);
    world.assert_clean();
}

#[test]
fn a_game_started_anew_is_shown_no_results_screen_when_its_old_match_ends() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, alice_team) = world.match_of(alice, bob);
    let alice_account = world.ids[alice].0;

    // Alice's game is gone in the match; she starts it again and signs in
    // while the match still runs. This game has loaded no match.
    world.client(alice).leave();
    world.settle();
    let session = world.new_session(alice_account);
    let again = world.connect_session((alice_account, session));
    assert!(screens(&world.take(again)).is_empty());
    assert!(!world.core.sent_to_match.contains(&session));

    // The match ends while she is in the lobby: Bob, who sat through it,
    // gets the screen; her new game gets none, and nothing waits for a
    // later moment.
    world.end_match(match_id, alice_team);
    world.advance(Duration::from_secs(3));
    assert_eq!(screens(&world.take(bob)).len(), 1);
    assert!(screens(&world.take(again)).is_empty());
    assert!(!world.results.has_unseen(alice_account));
    world.advance(Duration::from_secs(3));
    assert!(screens(&world.take(again)).is_empty());

    // Her next match is one her game has loaded: its screen comes.
    let (next, team) = world.match_of(again, bob);
    assert!(world.core.sent_to_match.contains(&session));
    world.end_match(next, team);
    world.settle();
    assert_eq!(screens(&world.take(again)).len(), 1);
}

#[test]
fn the_results_wait_for_a_player_who_left_the_match_or_the_game() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, alice_team) = world.match_of(alice, bob);
    let alice_account = world.ids[alice].0;

    // Bob closed the match early and sits in the lobby; Alice's game is
    // gone altogether.
    world.ask(bob, request::leave(1, true));
    assert!(screens(&world.take(bob)).is_empty());
    world.client(alice).leave();
    world.settle();
    assert!(world.client(alice).disconnected);

    world.end_match(match_id, alice_team);
    world.advance(Duration::from_secs(2));
    // Bob is shown it on the lobby's next pass, after his account.
    let decoded = world.take(bob);
    assert_eq!(screens(&decoded), [(match_id, 2, 2)]);
    let screen = position(&decoded, |message| {
        matches!(message, Decoded::MatchStats { .. })
    })
    .unwrap();
    assert!(
        decoded[..screen]
            .iter()
            .any(|message| matches!(message, Decoded::Account(_)))
    );

    // Alice signs in again: the account as the match left it comes with
    // the sign-in, and no screen. Her game may have been started anew, and
    // a game that has loaded no match dies of a results screen (it
    // translates the map's name of its last match: a null pointer).
    let paid = world.account(alice).generic_money;
    assert!(world.results.has_unseen(alice_account));
    let session = world.new_session(alice_account);
    let again = world.connect_session((alice_account, session));
    let decoded = world.take(again);
    assert!(screens(&decoded).is_empty());
    assert!(
        decoded
            .iter()
            .any(|message| matches!(message, Decoded::Account(_)))
    );
    // The results are not kept for a later moment either: they were hers
    // to see once.
    assert!(!world.results.has_unseen(alice_account));
    assert!(
        decoded
            .iter()
            .any(|message| matches!(message, Decoded::Money { generic, .. } if *generic == paid))
    );
    world.advance(Duration::from_secs(3));
    assert!(screens(&world.take(again)).is_empty());
    assert!(screens(&world.take(bob)).is_empty());
    world.clients.remove(alice);
    world.assert_clean();
}

/// A drop card's answer: `98` with what it gave and the cards left, then
/// the inventory and the account.
fn assert_card_used(world: &mut World, player: usize, card: u32, left: u32) {
    let decoded = world.take(player);
    let dropped = position(&decoded, |message| {
        matches!(
            message,
            Decoded::Drop { card_item_id, card_count, .. }
                if *card_item_id == card && *card_count == left
        )
    })
    .unwrap_or_else(|| panic!("{decoded:?}"));
    let inventory = position(&decoded, |message| {
        matches!(message, Decoded::Inventory { .. })
    })
    .unwrap();
    let account = position(&decoded, |message| matches!(message, Decoded::Account(_))).unwrap();
    assert!(dropped < inventory && inventory < account);
    assert_eq!(
        world.clients[player].1.model.inventory().len(),
        world.account(player).inventory.len()
    );
}

/// The drop-card loop of 2026-10-18..20: a card was taken on any results
/// screen kept, as often as asked, so cards bought with silver turned
/// into more silver. One card for each player that finished a match now
/// (`drop_cards.md`).
#[test]
fn a_drop_card_is_used_once_on_the_screen_of_a_match_played() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let card_dict = Dictionary::shared()
        .item_named("drop_card")
        .unwrap()
        .dict_id;
    for player in [alice, bob, carol] {
        world.change(player, |account| {
            account.add_item(card_dict, 2);
        });
    }
    let card_of = |world: &World, player: usize| {
        world
            .account(player)
            .inventory
            .iter()
            .find(|item| item.dict_id == card_dict)
            .unwrap()
            .item_id
    };
    let (card, bob_card, carol_card) = (
        card_of(&world, alice),
        card_of(&world, bob),
        card_of(&world, carol),
    );
    let pistol = world.account(alice).kits[0].slots[7].unwrap().item_id;
    let refused = |world: &mut World, player: usize, request: (u8, Vec<u8>)| {
        let before = world.account(player).clone();
        world.ask(player, request);
        assert_eq!(
            denial(&world.take(player), 0x46).as_deref(),
            Some("st_cannot_use_drop_card")
        );
        assert_eq!(*world.account(player), before, "nothing changed");
    };

    let (first, team) = world.match_of(alice, bob);
    world.end_match(first, team);
    world.advance(Duration::from_secs(3));
    for player in [alice, bob, carol] {
        world.take(player);
    }

    // Not a card, not an item, a match not kept: the game's own refusal.
    for not_a_card in [pistol, 999] {
        refused(&mut world, alice, request::use_drop_card(not_a_card, first));
    }
    refused(&mut world, alice, request::use_drop_card(card, 0x77));
    // Her card on her match's screen.
    assert!(world.account(alice).drop_cards.is_empty());
    world.ask(alice, request::use_drop_card(card, first));
    assert_card_used(&mut world, alice, card, 1);
    assert_eq!(world.account(alice).drop_cards, [first]);
    // A second card for the same match: refused, and the card stays.
    refused(&mut world, alice, request::use_drop_card(card, first));
    // Carol was not in it.
    refused(&mut world, carol, request::use_drop_card(carol_card, first));
    assert!(world.account(carol).drop_cards.is_empty());
    // Bob's own card is his, once; the arguments the other way round are
    // still read.
    world.ask(bob, request::use_drop_card(first, bob_card));
    assert_card_used(&mut world, bob, bob_card, 1);
    refused(&mut world, bob, request::use_drop_card(bob_card, first));

    // Her next match takes her next card, once.
    let (second, team) = world.match_of(alice, bob);
    world.end_match(second, team);
    world.advance(Duration::from_secs(3));
    world.take(alice);
    world.ask(alice, request::use_drop_card(card, second));
    assert_card_used(&mut world, alice, card, 0);
    refused(&mut world, alice, request::use_drop_card(card, first));
    // Used, and no card left either.
    refused(&mut world, alice, request::use_drop_card(card, second));
    world.assert_clean();
}

#[test]
fn a_player_who_signs_in_again_goes_back_to_its_match_under_its_new_session() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, _) = world.match_of(alice, bob);
    let (account, old_session) = world.ids[bob];
    assert!(world.host.reseated.lock().unwrap().is_empty());

    // Bob's game was started anew: a new sign-in, a new session. The old
    // link is signed out; Play sends the new one to the match, and the
    // match server is told whose place it is now.
    let session = world.new_session(account);
    assert_ne!(session, old_session);
    let again = world.connect_session((account, session));
    world.take(again);
    world.ask(again, request::play(1));
    world.advance(Duration::from_secs(1));
    assert_eq!(
        sent_to_match(&world.take(again)).map(|sent| sent.0),
        Some(match_id)
    );
    assert_eq!(
        world.host.reseated.lock().unwrap()[..],
        [(match_id, account, session)]
    );
    // Told once; nobody else was moved.
    world.advance(Duration::from_secs(3));
    assert_eq!(world.host.reseated.lock().unwrap().len(), 1);
    assert_eq!(world.host.made.lock().unwrap().len(), 1);
    world.clients.remove(bob);
    world.assert_clean();
}

#[test]
fn the_operators_text_reaches_everyone_and_who_signs_in_later() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.take(bob);
    // Longer than the client's 64-byte buffers: cut, never refused.
    let long = "x".repeat(100);
    world.inbox.set_admin_text("Restart at 20:00", &long);
    world.settle();
    for index in [alice, bob] {
        let decoded = world.take(index);
        assert!(
            decoded.iter().any(|message| matches!(
                message,
                Decoded::AdminText(first, second)
                    if first == "Restart at 20:00" && second.len() == 63
            )),
            "{decoded:?}"
        );
    }
    // It stands: a client signing in now is shown it.
    let eve = world.connect("Eve");
    assert!(world.take(eve).iter().any(
        |message| matches!(message, Decoded::AdminText(first, _) if first == "Restart at 20:00")
    ));
    world.inbox.clear_admin_text();
    world.settle();
    for index in [alice, bob, eve] {
        assert!(
            world
                .take(index)
                .iter()
                .any(|message| matches!(message, Decoded::AdminTextClear))
        );
    }
    assert_eq!(world.inbox.admin_text(), None);
    let late = world.connect("Late");
    assert!(
        !world
            .take(late)
            .iter()
            .any(|message| matches!(message, Decoded::AdminText(..)))
    );
    // Any messages, to everyone at once.
    world
        .inbox
        .broadcast(vec![survarium::v026g::lobby::match_stats_notice(5)]);
    world.settle();
    for index in [alice, bob, eve, late] {
        assert!(world.take(index).iter().any(|message| matches!(
            message,
            Decoded::ChatText { sender_type: 8, body, .. } if body == "#mstats [5]"
        )));
    }
    world.assert_clean();
}

mod admin_mute_tests;
mod away_tests;
mod balance_tests;
mod chat_mute_tests;
mod clan_tests;
mod feedback_tests;
mod gear_tests;
mod items_tests;
mod patch_check_tests;
mod presets_tests;
mod pvb_tests;
mod reconnect_tests;
mod replay_tests;
mod room_chat_tests;
mod rooms_tests;

// ---- the premium shop, items for gold, the daily bonus, the tutorial, a
// ---- nickname change (`shop`)

/// The last balance the client was told.
fn told_money(decoded: &[Decoded]) -> Option<(u32, u32)> {
    decoded.iter().rev().find_map(|message| match message {
        Decoded::Money { generic, premium } => Some((*generic, *premium)),
        _ => None,
    })
}

fn told_premium(decoded: &[Decoded]) -> Option<Option<u32>> {
    decoded.iter().rev().find_map(|message| match message {
        Decoded::Services { premium_hours } => Some(*premium_hours),
        _ => None,
    })
}

#[test]
fn a_sign_in_brings_the_shop_no_tutorial_and_the_daily_bonus() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let decoded = world.take(alice);
    // The shop: seven premium times and seven bundles.
    let records = decoded
        .iter()
        .find_map(|message| match message {
            Decoded::Shop(records) => Some(records.clone()),
            _ => None,
        })
        .expect("the shop");
    assert_eq!(records, premium::Settings::default().shop_records());
    assert_eq!(records.len(), 14);
    // The price list has the premium items too.
    let prices_sent = decoded
        .iter()
        .find_map(|message| match message {
            Decoded::Prices(prices) => Some(prices.len()),
            _ => None,
        })
        .expect("the prices");
    assert_eq!(prices_sent, shop_offers::price_list().len());
    assert!(prices_sent > prices::price_list().len());
    // The rename's price: the voucher's 250 gold.
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::ServicePrices(prices) if prices.change_name == 250
    )));
    // No tutorial: every step done.
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Tutorial {
            show: true,
            a: 0x3F,
            b: 1
        }
    )));
    // The daily bonus: day 1 of 7, nothing won, 50 gold on the seventh;
    // the welcome window is opened.
    let bonus = decoded
        .iter()
        .find_map(|message| match message {
            Decoded::DailyBonus {
                progress,
                reward,
                completed,
                show_window,
            } => Some((
                progress.clone(),
                reward.dict_id,
                reward.condition_or_stack,
                *completed,
                *show_window,
            )),
            _ => None,
        })
        .expect("the daily bonus");
    assert_eq!(bonus, (vec![0], 215, 50, false, true));
    // No discount is set: none is offered.
    assert!(
        !decoded
            .iter()
            .any(|message| matches!(message, Decoded::PremiumDiscount(_)))
    );
    // `43 00`: the bonus again.
    world.ask(alice, request::query(0x43));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::DailyBonus { .. }]
    ));
    world.assert_clean();

    // The same account, the same day, again: the window is not opened a
    // second time.
    let ids = world.ids[alice];
    world.store.end_session(ids.1);
    world.advance(Duration::from_secs(2));
    let session = world.new_session(ids.0);
    let again = world.connect_session((ids.0, session));
    let decoded = world.take(again);
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::DailyBonus {
            show_window: false,
            ..
        }
    )));
}

/// An account without premium: the third kit comes as one it may use, takes
/// a move and goes to Play.
#[test]
fn the_third_kit_is_filled_and_played_without_premium() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let decoded = world.take(alice);
    let Some(Decoded::Account(own)) = decoded
        .iter()
        .find(|message| matches!(message, Decoded::Account(_)))
    else {
        panic!("{decoded:?}");
    };
    let states: Vec<u8> = own.body.kits.iter().map(|kit| kit.state).collect();
    assert_eq!(states, [survarium::v026g::lobby::KIT_READY; 3]);
    let now = session::v026g::quests::now();
    assert!(!world.account(alice).is_premium(now));
    // The first kit's gear in the third, then its pistol out and back in
    // by the client's own moves.
    world.change(alice, |account| {
        account.kits[2].slots = account.kits[0].slots;
    });
    let pistol = world.account(alice).kits[2].slots[7].expect("a pistol");
    let item = pistol.item_id;
    let dict = world.account(alice).item(item).expect("owned").dict_id;
    let out = request::move_item(3, item, dict, 7, INVENTORY_SLOT, 1);
    world.ask(alice, out);
    assert_eq!(denial(&world.take(alice), 0x46), None);
    assert_eq!(world.account(alice).kits[2].slots[7], None);
    let back = request::move_item(3, item, dict, INVENTORY_SLOT, 7, 1);
    world.ask(alice, back);
    assert_eq!(denial(&world.take(alice), 0x46), None);
    assert_eq!(world.account(alice).kits[2].slots[7], Some(pistol));
    // Play with it: into a room.
    world.ask(alice, request::play(3));
    assert_eq!(denial(&world.take(alice), 0x2F), None);
    assert!(world.core.rooms.of(world.ids[alice].0).is_some());
    world.assert_clean();
}

#[test]
fn premium_time_is_bought_for_gold() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    // A new account has no gold: this one is given some.
    world.change(alice, |account| account.premium_money = 1_000);
    let gold = world.account(alice).premium_money;
    assert!(gold >= 100);
    // The third kit is every account's, premium or not.
    world.ask(alice, request::active_kit(3));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Permitted(Permitted::ActiveKit(3))]
    ));

    // A day (offer 7) for 100 gold.
    world.ask(alice, request::buy_offer(7, 0));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::OfferBought)
    ));
    assert_eq!(told_premium(&decoded), Some(Some(24)));
    assert_eq!(told_money(&decoded).unwrap().1, gold - 100);
    // The services say so when asked.
    world.ask(alice, request::query(0x3E));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Services {
            premium_hours: Some(24)
        }]
    ));

    // No such offer, and not the gold for a year: the game's texts.
    world.ask(alice, request::buy_offer(99, 0));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_buy_item")
    );
    world.ask(alice, request::buy_offer(6, 0));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cant_buy_premium_message")
    );
    assert_eq!(world.account(alice).premium_money, gold - 100);
    world.assert_clean();
}

#[test]
fn a_bundle_and_a_premium_item_are_bought_for_gold() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    world.change(alice, |account| account.premium_money = 2_000);
    let dictionary = Dictionary::shared();
    let id = |name: &str| dictionary.item_named(name).unwrap().dict_id;
    let silver = world.account(alice).generic_money;

    // The AK-74N bundle (offer 9): the rifle and 2000 rounds, 1500 gold,
    // onto the 1000 a new account has.
    world.ask(alice, request::buy_offer(9, 0));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::OfferBought)
    ));
    assert_eq!(told_money(&decoded), Some((silver, 500)));
    let owned: Vec<(u16, u32)> = world.clients[alice]
        .1
        .model
        .inventory()
        .iter()
        .map(|item| (item.dict_id, item.condition_or_stack))
        .collect();
    assert!(owned.contains(&(id("ak_74n_premium"), FULL_CONDITION)));
    assert!(owned.contains(&(id("ammo_5.45x39_7n6m"), 3_000)));

    // Two repair kits from the shop's list: 50 gold each, silver untouched.
    world.ask(
        alice,
        request::buy(id("repair_voucher"), 2, 0, INVENTORY_SLOT, 1),
    );
    let decoded = world.take(alice);
    assert!(matches!(
        &decoded[0],
        Decoded::Permitted(Permitted::Bought(bought))
            if bought.len() == 1 && bought[0].dict_id == id("repair_voucher") && bought[0].value == 2
    ));
    assert_eq!(told_money(&decoded), Some((silver, 400)));
    // Not the gold for the AK-74N (1500): the game's text for it.
    world.ask(alice, request::buy(id("ak_74n"), 1, 0, INVENTORY_SLOT, 1));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cant_buy_premium_message")
    );
    assert_eq!(world.account(alice).premium_money, 400);
    world.assert_clean();
}

/// The owner's choice of 2026-10-18 (`prices::NOT_SOLD`): the Russian and
/// Belarusian flags and coats of arms, the flag of the USSR and the red
/// star are in no price record the lobby is
/// sent and a buy of one is refused, for gold as for silver; one already
/// owned is still sold back. The other flags are sold as before.
#[test]
fn the_excluded_flags_and_emblems_are_not_sold() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let decoded = world.take(alice);
    let dictionary = Dictionary::shared();
    let id = |name: &str| dictionary.item_named(name).unwrap().dict_id;
    let excluded = [
        "decals_4_1",
        "decals_4_2",
        "decals_4_4",
        "decals_5_1",
        "decals_5_2",
        "decals_5_4",
    ]
    .map(id);
    let sent: Vec<u16> = decoded
        .iter()
        .find_map(|message| match message {
            Decoded::Prices(prices) => Some(prices.iter().map(|price| price.dict_id).collect()),
            _ => None,
        })
        .expect("the prices");
    assert!(excluded.iter().all(|dict_id| !sent.contains(dict_id)));
    // The flag of Ukraine and the coat of arms of Lithuania still are.
    assert!(sent.contains(&id("decals_4_3")) && sent.contains(&id("decals_5_7")));

    world.change(alice, |account| {
        account.premium_money = 10_000;
        account.generic_money = 1_000_000;
    });
    let before = world.account(alice).clone();
    for dict_id in excluded {
        world.ask(alice, request::buy(dict_id, 1, 0, INVENTORY_SLOT, 1));
        assert_eq!(
            denial(&world.take(alice), 0x47).as_deref(),
            Some("st_cannot_buy_item"),
            "dict {dict_id}"
        );
    }
    assert_eq!(*world.account(alice), before);
    // Another flag: 300 gold.
    world.ask(
        alice,
        request::buy(id("decals_4_3"), 1, 0, INVENTORY_SLOT, 1),
    );
    let decoded = world.take(alice);
    assert!(matches!(
        &decoded[0],
        Decoded::Permitted(Permitted::Bought(bought)) if bought[0].dict_id == id("decals_4_3")
    ));
    assert_eq!(world.account(alice).premium_money, 9_700);

    // One owned from before is sold back like any decal.
    let mut item_id = 0;
    world.change(alice, |account| {
        item_id = account.add_item(id("decals_4_2"), FULL_CONDITION)
    });
    world.take(alice);
    let silver = world.account(alice).generic_money;
    world.ask(alice, request::sell(item_id, 1));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::Sold { item_id: sold, amount: 1 }) if sold == item_id
    ));
    assert!(world.account(alice).item(item_id).is_none());
    assert!(world.account(alice).generic_money > silver);
    world.assert_clean();
}

#[test]
fn the_premium_voucher_is_used_from_the_inventory() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    let voucher = Dictionary::shared()
        .item_named(premium::PREMIUM_VOUCHER)
        .unwrap()
        .dict_id;
    let mut item_id = 0;
    world.change(alice, |account| item_id = account.add_item(voucher, 2));

    world.ask(
        alice,
        (0x46, [vec![4], item_id.to_le_bytes().to_vec()].concat()),
    );
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::ItemUsed { item_id: used }) if used == item_id
    ));
    assert_eq!(told_premium(&decoded), Some(Some(24)));
    // One left, in the client's inventory too.
    let left = |world: &World| {
        world.clients[alice]
            .1
            .model
            .inventory()
            .iter()
            .find(|item| item.item_id == item_id)
            .map(|item| item.condition_or_stack)
    };
    assert_eq!(left(&world), Some(1));
    world.ask(
        alice,
        (0x46, [vec![4], item_id.to_le_bytes().to_vec()].concat()),
    );
    assert_eq!(told_premium(&world.take(alice)), Some(Some(48)));
    assert_eq!(left(&world), None);
    // Used up; and the pistol is no voucher.
    for item in [item_id, 1] {
        world.ask(
            alice,
            (0x46, [vec![4], item.to_le_bytes().to_vec()].concat()),
        );
        assert_eq!(
            denial(&world.take(alice), 0x46).as_deref(),
            Some("st_error_while_using_premium_access_voucher")
        );
    }
    world.assert_clean();
}

/// `47 05 <offer> <discount> <money> <experience> <reputation> <faction>`.
fn accept_with_bonuses(offer_id: u32, discount: f32) -> (u8, Vec<u8>) {
    let mut body = vec![5];
    body.extend(offer_id.to_le_bytes());
    body.extend(discount.to_le_bytes());
    for value in [0_u32, 0, 0] {
        body.extend(value.to_le_bytes());
    }
    body.push(0);
    (0x47, body)
}

/// `47 06 <offer> <discount>`.
fn accept_with_discount(offer_id: u32, discount: f32) -> (u8, Vec<u8>) {
    let mut body = vec![6];
    body.extend(offer_id.to_le_bytes());
    body.extend(discount.to_le_bytes());
    (0x47, body)
}

#[test]
fn a_discount_the_operator_set_is_offered_and_taken() {
    let mut world = World::new();
    world.core.shop.discount_percent = 25;
    let alice = world.connect("Alice");
    let decoded = world.take(alice);
    // `A9`: the discount, for an account without premium.
    assert!(
        decoded.iter().any(
            |message| matches!(message, Decoded::PremiumDiscount(discount) if *discount == 0.25)
        )
    );
    // A new account has no gold: this one is given some.
    world.change(alice, |account| account.premium_money = 1_000);
    let gold = world.account(alice).premium_money;
    // Closing the window (`-1`) is answered with nothing.
    world.ask(alice, accept_with_discount(u32::MAX, -1.0));
    assert!(world.take(alice).is_empty());
    // Another discount than the one offered is refused.
    world.ask(alice, accept_with_discount(7, 0.9));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_buy_item")
    );
    // A day at a quarter off: 75 gold.
    world.ask(alice, accept_with_discount(7, 0.25));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::OfferBought)
    ));
    assert_eq!(told_premium(&decoded), Some(Some(24)));
    assert_eq!(told_money(&decoded).unwrap().1, gold - 75);
    // With premium there is no discount to take.
    world.ask(alice, accept_with_discount(7, 0.25));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_buy_item")
    );
    // Nor is one offered at the next sign-in.
    let ids = world.ids[alice];
    world.store.end_session(ids.1);
    world.advance(Duration::from_secs(2));
    let session = world.new_session(ids.0);
    let again = world.connect_session((ids.0, session));
    assert!(
        !world
            .take(again)
            .iter()
            .any(|message| matches!(message, Decoded::PremiumDiscount(_)))
    );
    world.assert_clean();
}

#[test]
fn the_offer_after_a_match_gives_the_premium_half() {
    let mut world = World::new();
    world.core.shop.discount_percent = 20;
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, alice_team) = world.match_of(alice, bob);
    world.end_match(match_id, alice_team);
    world.settle();
    let decoded = world.take(alice);
    world.take(bob);
    // `A8` after the screen: the premium half of the match, an hour to
    // take it, a fifth off.
    let half = world
        .results
        .find(match_id)
        .unwrap()
        .player(world.ids[alice].0)
        .unwrap()
        .reward
        .premium_bonus;
    assert!(half.0 > 0 && half.1 > 0);
    let screen = position(&decoded, |message| {
        matches!(message, Decoded::MatchStats { .. })
    })
    .expect("the results screen");
    let offer = position(&decoded, |message| {
        matches!(message, Decoded::PremiumStore { .. })
    })
    .expect("the offer");
    assert!(screen < offer);
    assert!(matches!(
        decoded[offer],
        Decoded::PremiumStore { seconds: 0, values, factor }
            if values == [half.0, half.1, half.2, 3600] && factor == 0.2
    ));
    // The daily bonus as the match left it: Alice's first victory today.
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::DailyBonus { progress, show_window: false, .. } if progress == &[1]
    )));

    // A wrong discount is refused; the offer stands.
    world.ask(alice, accept_with_bonuses(7, 0.5));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_buy_item")
    );
    // A new account has no gold: this one is given some.
    world.change(alice, |account| account.premium_money = 1_000);
    let before = world.account(alice).clone();
    world.ask(alice, accept_with_bonuses(7, 0.2));
    let decoded = world.take(alice);
    // `83 47 05`: the client shows "premium activated" with the rewards.
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::NoEffect {
            op: 0x47,
            event: Some(5)
        })
    ));
    assert_eq!(told_premium(&decoded), Some(Some(24)));
    let after = world.account(alice).clone();
    // A day at a fifth off: 80 gold; the half's money and experience.
    assert_eq!(after.premium_money, before.premium_money - 80);
    assert!(after.generic_money >= before.generic_money + half.0);
    assert_eq!(after.exp_total, before.exp_total + half.1);
    assert_eq!(
        told_money(&decoded),
        Some((after.generic_money, after.premium_money))
    );
    assert!(
        decoded
            .iter()
            .any(|message| matches!(message, Decoded::Skills { .. }))
    );
    // Once: the offer is gone with its purchase.
    world.ask(alice, accept_with_bonuses(7, 0.2));
    assert_eq!(
        denial(&world.take(alice), 0x47).as_deref(),
        Some("st_cannot_buy_item")
    );
    assert_eq!(world.account(alice).exp_total, after.exp_total);

    // Bob's offer ran out: refused.
    let bob_id = world.ids[bob].0;
    world.core.premium_offers.get_mut(&bob_id).unwrap().1 = 0;
    world.ask(bob, accept_with_bonuses(7, 0.2));
    assert_eq!(
        denial(&world.take(bob), 0x47).as_deref(),
        Some("st_cannot_buy_item")
    );
    world.assert_clean();
}

#[test]
fn a_nickname_is_checked_and_changed() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.take(bob);
    let check = |world: &mut World, name: &str| {
        world.ask(alice, request::profile_text(3, name));
        match world.take(alice).remove(0) {
            Decoded::Permitted(Permitted::NicknameCheck { refusal, .. }) => refusal,
            other => panic!("not a nickname check: {other:?}"),
        }
    };
    // Another account's name, in any case, is taken; one's own is not.
    assert_eq!(check(&mut world, "bob"), Some(NameRefusal::Taken));
    assert_eq!(check(&mut world, "Alice"), None);
    assert_eq!(check(&mut world, "Alisa"), None);
    assert_eq!(check(&mut world, "A B"), Some(NameRefusal::Space));

    let rename = |name: &str, voucher: u32| {
        let (id, mut body) = request::profile_text(4, name);
        body.extend(voucher.to_le_bytes());
        (id, body)
    };
    // A new account has no gold: this one is given some.
    world.change(alice, |account| account.premium_money = 1_000);
    let gold = world.account(alice).premium_money;
    // A taken or bad name: the game's text, and nothing is paid.
    for name in ["Bob", "Al"] {
        world.ask(alice, rename(name, 0));
        assert_eq!(
            denial(&world.take(alice), 0x49).as_deref(),
            Some("st_change_name_error")
        );
    }
    assert_eq!(world.account(alice).premium_money, gold);

    // In gold: 250.
    world.ask(alice, rename("Alisa", 0));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::NicknameChanged { ticket_item_id: 0 })
    ));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::BaseInfo { nickname, .. } if nickname == "Alisa"
    )));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Account(own) if own.body.name == "Alisa"
    )));
    assert_eq!(told_money(&decoded).unwrap().1, gold - 250);
    assert_eq!(world.account(alice).name, "Alisa");
    // The store's name follows: others find the account by it, and the
    // old name is free.
    let alice_id = world.ids[alice].0;
    assert_eq!(
        world.store.account(alice_id).unwrap().locked().name,
        "Alisa"
    );
    world.ask(bob, request::find_players("Alis"));
    assert!(matches!(
        &world.take(bob)[..],
        [Decoded::Friendship(FriendshipPush::Found { players, .. })]
            if players.len() == 1 && players[0].1 == "Alisa"
    ));
    world.ask(bob, request::profile_text(3, "Alisa"));
    assert!(matches!(
        &world.take(bob)[0],
        Decoded::Permitted(Permitted::NicknameCheck {
            refusal: Some(NameRefusal::Taken),
            ..
        })
    ));
    // Alice's login is still hers alone: Bob may not take the name she
    // signs in with.
    world.ask(bob, request::profile_text(3, "Alice"));
    assert!(matches!(
        &world.take(bob)[0],
        Decoded::Permitted(Permitted::NicknameCheck {
            refusal: Some(NameRefusal::Taken),
            ..
        })
    ));
    // And she signs in as before.
    assert_eq!(world.store.account_id_by_name("Alice"), Some(alice_id));

    // With a rename voucher: one is taken, no gold.
    let voucher = Dictionary::shared()
        .item_named(premium::RENAME_VOUCHER)
        .unwrap()
        .dict_id;
    let mut item_id = 0;
    world.change(alice, |account| item_id = account.add_item(voucher, 1));
    world.ask(alice, rename("Alina", item_id));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::NicknameChanged { ticket_item_id }) if ticket_item_id == item_id
    ));
    assert_eq!(told_money(&decoded).unwrap().1, gold - 250);
    assert!(world.account(alice).item(item_id).is_none());
    assert_eq!(world.account(alice).name, "Alina");
    world.assert_clean();
}

/// No tutorial (`session::v026g::tutorial`): a new account is told every
/// step is done, so its client runs none of it, and starts with 15 000
/// silver, 150 gold, the steps' experience, the ammo and the starter kit
/// (`tutorial::tests`); a step a client sends anyway is not answered.
#[test]
fn a_new_account_has_no_tutorial_and_what_it_paid() {
    use survarium::v026g::lobby::tutorial_model::State;
    let mut world = World::new();
    world.accounts.set_starter_kit(true);
    let alice = world.connect("Alice");
    // The start is what the client is first told: no window for it.
    let decoded = world.take(alice);
    assert_eq!(told_money(&decoded), Some((15_000, 150)));
    // A month of premium: 720 hours left.
    assert_eq!(told_premium(&decoded), Some(Some(720)));
    // Its inventory, the respirator's and the gloves' modifications too.
    let held: Vec<(u16, Vec<u32>)> = world
        .client(alice)
        .model
        .inventory()
        .iter()
        .filter(|item| !item.modifications.is_empty())
        .map(|item| {
            let ids = item.modifications.iter().map(|m| m.id).collect();
            (item.dict_id, ids)
        })
        .collect();
    assert_eq!(held, [(43, vec![1183]), (42, vec![1133])]);
    assert_eq!(
        world.client(alice).model.inventory().len(),
        world.account(alice).inventory.len()
    );
    assert_eq!(world.client(alice).model.tutorial().state(), State::Default);
    let account = world.account(alice);
    assert_eq!(
        (account.generic_money, account.premium_money),
        (15_000, 150)
    );
    assert_eq!(account.exp_total, 250);
    // The Scavengers' quests, never neutral, at their level 5.
    assert_eq!(account.quest_faction, 1);
    assert_eq!(account.reputation_level(1), 5);
    for dict_id in session::v026g::tutorial::starter_ammo(Dictionary::shared()) {
        let owned: u32 = account
            .inventory
            .iter()
            .filter(|item| item.dict_id == dict_id)
            .map(|item| item.condition_or_stack)
            .sum();
        assert_eq!(owned, session::v026g::tutorial::SIGNUP_ROUNDS, "{dict_id}");
    }
    drop(account);
    // Given once: the next sign-in gives nothing more.
    let money = world.account(alice).generic_money;
    let ids = world.ids[alice];
    world.store.end_session(ids.1);
    world.advance(Duration::from_secs(2));
    let session = world.new_session(ids.0);
    let again = world.connect_session((ids.0, session));
    world.take(again);
    assert_eq!(world.account(again).generic_money, money);
    // The tutorial's steps and its "choose a weapon": nothing.
    for step in [(0x4B, vec![1, 1]), (0x4B, vec![2]), (0x46, vec![0x0A])] {
        world.ask(again, step);
        assert!(world.take(again).is_empty());
    }
    assert_eq!(world.account(again).generic_money, money);
    world.assert_clean();
}

#[test]
fn the_two_boxes_of_the_kit_window_are_kept() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.take(alice);
    // The lobby's number: 2 is auto-repair alone, 3 both, 0 neither.
    world.ask(alice, (0x49, vec![1, 2]));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Permitted(Permitted::AutoRepair { autobuy: false })]
    ));
    assert!(world.account(alice).extras.auto_repair && !world.account(alice).autobuy);
    world.ask(alice, (0x49, vec![1, 3]));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Permitted(Permitted::AutoRepair { autobuy: true })]
    ));
    assert!(world.account(alice).extras.auto_repair && world.account(alice).autobuy);
    // The account says so when it is sent again.
    world.change(alice, |_| {});
    world.ask(alice, request::active_kit(2));
    world.take(alice);
    world.ask(alice, request::move_item(2, 99, 1, 100, 7, 1));
    assert!(world.take(alice).iter().any(|message| matches!(
        message,
        Decoded::Account(own) if own.autobuy && own.auto_repair
    )));
    world.ask(alice, (0x49, vec![1, 0]));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::Permitted(Permitted::Autobuy(false))]
    ));
    assert!(!world.account(alice).extras.auto_repair && !world.account(alice).autobuy);
}
