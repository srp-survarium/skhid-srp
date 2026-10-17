//! The lobby's core with clients in memory. The clock is the test's: each
//! round is 10 ms, and the transports, the queue and the timeouts all take
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
use survarium::v026g::lobby::requests::{BUY_PREMIUM, INVENTORY_SLOT};
use survarium::v026g::lobby::{ClientState, FULL_CONDITION, NameRefusal, channel};
use survarium::v026g::maps;
use survarium::v026g::match_msgs::snapshot::{PlayerStatistics, event};
use survarium::v026g::prices;
use survarium::v026g::shop_offers;

use super::client::{TestClient, request};
use super::{TestHost, denial, find, sent_to_match, signed_in};
use crate::v026g::core::Core;
use crate::v026g::{Inbox, MatchFinished, Settings};

mod late_join;
mod making_matches;

const ROUND: Duration = Duration::from_millis(10);

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
    /// A lobby whose queue gathers players for `gather`.
    fn new(gather: Duration) -> Self {
        Self::with_settings(gather, Settings::local("127.0.0.1:0"))
    }

    /// The same, with `settings`.
    fn with_settings(gather: Duration, settings: Settings) -> Self {
        let store: &'static SessionStore = Box::leak(Box::new(SessionStore::with_start_rules(
            gather,
            2,
            Duration::ZERO,
            Duration::from_secs(3600),
        )));
        let accounts: &'static GameAccounts = Box::leak(Box::new(GameAccounts::new()));
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

    /// A new account signs in; returns the client's index.
    fn connect(&mut self, name: &str) -> usize {
        let ids = signed_in(self.store, name);
        self.connect_session(ids)
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

#[test]
fn a_sign_in_is_answered_with_the_whole_account_in_the_clients_order() {
    let mut world = World::new(Duration::from_secs(60));
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
            0x8F, 0x90, 0x8B, 0x96, 0x8E, 0x8D, 0x94, 0x8A, 0x95, 0x89, 0x87, 0x8C, 0x92, 0x9C,
            0x9D, 0x9B, 0x85, 0x91, 0xA3, 0xA3, 0x99
        ]
    );
    assert!(client.ids.contains(&0xA1));
    // The three free weapons and their two kinds of ammo.
    assert_eq!(client.model.inventory().len(), 5);
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    world.change(alice, |account| {
        account.generic_money = 5_000_000;
        known_to_every_seller(account);
    });
    let record = buyable(&world.account(alice), false);

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
    assert_eq!(world.client(alice).model.inventory().len(), 6);
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
    assert_eq!(world.client(alice).model.inventory().len(), 5);
    assert!(world.account(alice).item(item_id).is_none());
    world.assert_clean();
}

#[test]
fn a_worn_item_is_repaired_and_gold_is_exchanged() {
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    // The echo after 0x80 and the tutorial's own reports ask for nothing.
    world.ask(alice, (0x32, vec![0, 0]));
    world.ask(alice, (0x4B, vec![2]));
    world.ask(alice, (0x49, vec![0x24, 0]));
    assert!(world.take(alice).is_empty());
    assert!(world.account(alice).tutorial_done);
    // A step of the tutorial pays its award (the notes about items: 25
    // gold), so the account follows.
    world.ask(alice, (0x4B, vec![1, 2]));
    assert!(
        world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::Money { .. }))
    );
    assert_eq!(world.account(alice).tutorial_step, 2);

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
    let mut world = World::new(Duration::from_secs(60));
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
fn two_players_gather_for_the_window_and_go_to_one_match() {
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);

    // Alone in the queue: waiting, however long.
    world.ask(alice, request::play(1));
    let decoded = world.take(alice);
    let Some(ClientState::InQueue {
        place,
        queue_total,
        order_id,
        ..
    }) = queue_state(&decoded)
    else {
        panic!("{decoded:?}");
    };
    assert_eq!((place, queue_total), (1, 1));
    assert_ne!(order_id, u32::MAX);
    world.advance(Duration::from_secs(150));
    let decoded = world.take(alice);
    // Its queue screen is kept up to date, the wait counted by the server.
    assert!(decoded.len() > 100);
    assert!(matches!(
        queue_state(&decoded),
        Some(ClientState::InQueue { place: 1, queue_total: 1, current_wait_seconds, .. })
            if current_wait_seconds >= 149
    ));
    assert!(sent_to_match(&decoded).is_none());
    assert!(world.host.made.lock().unwrap().is_empty());

    // The second player starts the window.
    world.take(bob);
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(58));
    assert!(sent_to_match(&world.take(alice)).is_none());
    assert!(matches!(
        queue_state(&world.take(bob)),
        Some(ClientState::InQueue {
            place: 2,
            queue_total: 2,
            ..
        })
    ));
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
    let mut world = World::new(Duration::ZERO);
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

#[test]
fn leaving_the_queue_goes_back_to_the_lobby_and_no_match_is_made() {
    let mut world = World::new(Duration::from_secs(10));
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(5));
    world.take(bob);
    world.ask(bob, request::leave(1, false));
    assert_eq!(queue_state(&world.take(bob)), Some(ClientState::Lobby));
    // Alice is alone again: the window is off.
    world.take(alice);
    world.advance(Duration::from_secs(30));
    let decoded = world.take(alice);
    assert!(matches!(
        queue_state(&decoded),
        Some(ClientState::InQueue {
            place: 1,
            queue_total: 1,
            ..
        })
    ));
    assert!(sent_to_match(&decoded).is_none());
    assert!(world.take(bob).iter().all(|message| !matches!(
        message,
        Decoded::ClientState { .. } | Decoded::ConnectToMatch { .. }
    )));
    assert!(world.host.made.lock().unwrap().is_empty());
    world.assert_clean();
}

#[test]
fn a_finished_match_brings_its_players_back_to_the_lobby() {
    let mut world = World::new(Duration::ZERO);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(2));
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
    world.inbox.match_finished(MatchFinished { match_id });
    world.settle();
    for client in [alice, bob] {
        assert_eq!(queue_state(&world.take(client)), Some(ClientState::Lobby));
    }
    assert_eq!(world.store.match_count(), 0);
    assert!(world.core.matches.is_empty());

    // A new Play is a new match.
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(2));
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
    let mut world = World::new(Duration::ZERO);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.host.fail.store(true, Ordering::Relaxed);
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(2));
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
    world.advance(Duration::from_secs(2));
    assert!(sent_to_match(&world.take(alice)).is_some());
    world.assert_clean();
}

#[test]
fn chat_reaches_who_it_is_for() {
    let mut world = World::new(Duration::from_secs(60));
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
fn players_in_a_match_talk_on_its_channels() {
    let mut world = World::new(Duration::ZERO);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(2));
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
    let mut world = World::new(Duration::ZERO);
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
    world.advance(Duration::from_secs(3));
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
    let bob_again = world.connect_session((bob_id, world.store.create_session(bob_id)));
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
    let mut world = World::new(Duration::ZERO);
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
        let mut world = World::new(Duration::ZERO);
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
    let mut world = World::new(Duration::ZERO);
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
    let mut world = World::new(Duration::ZERO);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);
    world.take(alice);
    world.take(bob);
    world.ask(alice, request::important("Bob", 2, "invite"));
    let invite = importants(&world.take(bob), 2, "Alice")[0];
    // Bob's game is started again: a new session, and no invite in it.
    let bob_again = world.connect_session((bob_id, world.store.create_session(bob_id)));
    assert!(importants(&world.take(bob_again), 2, "Alice").is_empty());
    world.ask(alice, request::important("Bob", 2, "invite"));
    assert_eq!(importants(&world.take(bob_again), 2, "Alice"), [invite]);
    world.ask(bob_again, request::important("Alice", 3, "membership accepted"));
    world.ask(bob_again, request::answer(invite, 0));
    assert_eq!(importants(&world.take(alice), 3, "Bob").len(), 1);
    world.ask(alice, request::add_squad_member("Bob"));
    let squad = world.store.squads().of(alice_id).expect("a squad");
    assert_eq!(squad.members, [alice_id, bob_id]);
}

#[test]
fn a_squad_of_two_forms_shows_each_mate_and_queues_together() {
    let mut world = World::new(Duration::ZERO);
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
        [
            (alice_id, "Alice", true, false),
            (bob_id, "Bob", true, false)
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

    // Bob readies: he waits for the squad, nobody is in the queue.
    world.ask(bob, request::play(1));
    assert!(matches!(
        queue_state(&world.take(bob)),
        Some(ClientState::InQueue { .. })
    ));
    let (_, members, _, _) = squad(&world.take(alice)).expect("Bob is ready");
    assert!(members[1].ready && !members[0].ready);
    assert!(world.store.matchmaking().busy_sessions().is_empty());
    // Carol queues alone meanwhile.
    world.ask(carol, request::play(1));
    world.advance(Duration::from_secs(2));
    assert!(sent_to_match(&world.take(carol)).is_none());

    // The commander presses Play: the squad queues as one, and with Carol
    // there are enough for a match. The squad is on one team.
    world.ask(alice, request::play(1));
    world.advance(Duration::from_secs(2));
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
fn leaving_a_squad_tells_everyone_and_takes_them_out_of_the_queue() {
    let mut world = World::new(Duration::from_secs(600));
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    form_squad(&mut world, alice, bob);
    world.ask(bob, request::play(1));
    world.ask(alice, request::play(1));
    assert_eq!(world.store.matchmaking().busy_sessions().len(), 2);
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
        assert_eq!(queue_state(&decoded), Some(ClientState::Lobby));
    }
    assert!(world.store.matchmaking().busy_sessions().is_empty());
    assert!(world.store.squads().of(world.ids[alice].0).is_none());
    world.assert_clean();
}

#[test]
fn a_second_sign_in_of_an_account_signs_the_first_out() {
    let mut world = World::new(Duration::from_secs(60));
    let first = world.connect("Alice");
    let account_id = world.ids[first].0;
    world.take(first);
    let second = world.connect_session((account_id, world.store.create_session(account_id)));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    world.client(alice).leave();
    world.settle();
    assert!(world.client(alice).disconnected);
    assert!(world.store.has_ended(world.ids[alice].1));
    assert!(world.core.links.is_empty());
}

#[test]
fn other_servers_reach_a_client_through_the_inbox() {
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::ZERO);
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
    world.advance(Duration::from_secs(2));
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
        self.inbox.match_finished(MatchFinished { match_id });
    }

    /// Two players in a match: `(match id, the first one's team)`.
    fn match_of(&mut self, first: usize, second: usize) -> (u32, u8) {
        self.ask(first, request::play(1));
        self.ask(second, request::play(1));
        self.advance(Duration::from_secs(2));
        let sent = sent_to_match(&self.take(first)).expect("a match");
        assert!(sent_to_match(&self.take(second)).is_some());
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
    let mut world = World::new(Duration::ZERO);
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
    assert!(
        world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::Services { premium_hours: Some(144) }))
    );
    assert_eq!(world.account(alice).exp_total, paid.exp_total);
    world.assert_clean();
}

#[test]
fn a_game_started_anew_is_shown_no_results_screen_when_its_old_match_ends() {
    let mut world = World::new(Duration::ZERO);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, alice_team) = world.match_of(alice, bob);
    let alice_account = world.ids[alice].0;

    // Alice's game is gone in the match; she starts it again and signs in
    // while the match still runs. This game has loaded no match.
    world.client(alice).leave();
    world.settle();
    let session = world.store.create_session(alice_account);
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
    let mut world = World::new(Duration::ZERO);
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
    let session = world.store.create_session(alice_account);
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

#[test]
fn a_drop_card_is_used_from_the_results_screen() {
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    let card_dict = Dictionary::shared()
        .item_named("drop_card")
        .unwrap()
        .dict_id;
    world.change(alice, |account| {
        account.add_item(card_dict, 2);
    });
    let card = world
        .account(alice)
        .inventory
        .iter()
        .find(|item| item.dict_id == card_dict)
        .unwrap()
        .item_id;
    let pistol = world.account(alice).kits[0].slots[7].unwrap().item_id;

    // Not a card, not an item: the game's own refusal.
    for not_a_card in [pistol, 999] {
        world.ask(alice, request::use_drop_card(not_a_card, 0));
        assert_eq!(
            denial(&world.take(alice), 0x46).as_deref(),
            Some("st_cannot_use_drop_card")
        );
    }
    // A card: `98` with what it gave and the cards left, then the
    // inventory and the account. The second time with the arguments the
    // other way round (their order is a guess).
    for (left, (a, b)) in [(1, (card, 0x77)), (0, (0x77, card))] {
        world.ask(alice, request::use_drop_card(a, b));
        let decoded = world.take(alice);
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
            world.clients[alice].1.model.inventory().len(),
            world.account(alice).inventory.len()
        );
    }
    // No card left.
    world.ask(alice, request::use_drop_card(card, 0x77));
    assert_eq!(
        denial(&world.take(alice), 0x46).as_deref(),
        Some("st_cannot_use_drop_card")
    );
    world.assert_clean();
}

#[test]
fn a_player_who_signs_in_again_goes_back_to_its_match_under_its_new_session() {
    let mut world = World::new(Duration::ZERO);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, _) = world.match_of(alice, bob);
    let (account, old_session) = world.ids[bob];
    assert!(world.host.reseated.lock().unwrap().is_empty());

    // Bob's game was started anew: a new sign-in, a new session. The old
    // link is signed out; Play sends the new one to the match, and the
    // match server is told whose place it is now.
    let session = world.store.create_session(account);
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
    let mut world = World::new(Duration::from_secs(60));
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

mod away_tests;
mod balance_tests;
mod reconnect_tests;
mod items_tests;
mod clan_tests;
mod join_tests;
mod feedback_tests;
mod matches_now_tests;
mod patch_check_tests;
mod pvb_tests;
mod queue_chat_tests;
mod replay_tests;
mod squad_size_tests;

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
fn a_sign_in_brings_the_shop_the_tutorial_and_the_daily_bonus() {
    let mut world = World::new(Duration::from_secs(60));
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
    // The tutorial runs for a new account, from the inventory step.
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Tutorial {
            show: true,
            a: 1,
            b: 0
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
    let session = world.store.create_session(ids.0);
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
    let mut world = World::new(Duration::from_secs(60));
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
    // Play with it: in the queue.
    world.ask(alice, request::play(3));
    assert!(matches!(
        queue_state(&world.take(alice)),
        Some(ClientState::InQueue { .. })
    ));
    world.assert_clean();
}

#[test]
fn premium_time_is_bought_for_gold() {
    let mut world = World::new(Duration::from_secs(60));
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
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    world.take(alice);
    world.change(alice, |account| account.premium_money = 2_000);
    let dictionary = Dictionary::shared();
    let id = |name: &str| dictionary.item_named(name).unwrap().dict_id;
    let silver = world.account(alice).generic_money;

    // The AK-74N bundle (offer 9): the rifle and 2000 rounds, 1500 gold.
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
    assert!(owned.contains(&(id("ammo_5.45x39_7n6m"), 2_000)));

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

#[test]
fn the_premium_voucher_is_used_from_the_inventory() {
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    world.take(alice);
    let voucher = Dictionary::shared()
        .item_named(premium::PREMIUM_VOUCHER)
        .unwrap()
        .dict_id;
    let mut item_id = 0;
    world.change(alice, |account| item_id = account.add_item(voucher, 2));

    world.ask(alice, (0x46, [vec![4], item_id.to_le_bytes().to_vec()].concat()));
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
    world.ask(alice, (0x46, [vec![4], item_id.to_le_bytes().to_vec()].concat()));
    assert_eq!(told_premium(&world.take(alice)), Some(Some(48)));
    assert_eq!(left(&world), None);
    // Used up; and the pistol is no voucher.
    for item in [item_id, 1] {
        world.ask(alice, (0x46, [vec![4], item.to_le_bytes().to_vec()].concat()));
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
    let mut world = World::new(Duration::from_secs(60));
    world.core.shop.discount_percent = 25;
    let alice = world.connect("Alice");
    let decoded = world.take(alice);
    // `A9`: the discount, for an account without premium.
    assert!(decoded.iter().any(
        |message| matches!(message, Decoded::PremiumDiscount(discount) if *discount == 0.25)
    ));
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
    let session = world.store.create_session(ids.0);
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
    let mut world = World::new(Duration::ZERO);
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
    let mut world = World::new(Duration::from_secs(60));
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
    assert_eq!(world.store.account(alice_id).unwrap().locked().name, "Alisa");
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

#[test]
fn the_tutorials_steps_pay_their_awards_once() {
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    world.take(alice);
    let (silver, gold) = {
        let account = world.account(alice);
        (account.generic_money, account.premium_money)
    };
    // The notes about items (step 2): 25 gold.
    world.ask(alice, (0x4B, vec![1, 2]));
    assert_eq!(told_money(&world.take(alice)), Some((silver, gold + 25)));
    // Again: nothing.
    world.ask(alice, (0x4B, vec![1, 2]));
    assert!(world.take(alice).is_empty());
    // The tutorial itself (step 1): 1000 silver and 250 experience.
    world.ask(alice, (0x4B, vec![1, 1]));
    let decoded = world.take(alice);
    assert_eq!(told_money(&decoded), Some((silver + 1000, gold + 25)));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Skills { exp_total: 250, .. }
    )));
    // A step that is none asks for nothing.
    world.ask(alice, (0x4B, vec![1, 9]));
    assert!(world.take(alice).is_empty());
    {
        let account = world.account(alice);
        assert_eq!(account.extras.tutorial_flags, 0b111);
        assert!(account.tutorial_done);
    }
    // The next sign-in tells the client the tutorial is over.
    let ids = world.ids[alice];
    world.store.end_session(ids.1);
    world.advance(Duration::from_secs(2));
    let session = world.store.create_session(ids.0);
    let again = world.connect_session((ids.0, session));
    assert!(world.take(again).iter().any(|message| matches!(
        message,
        Decoded::Tutorial {
            show: true,
            a: 0b111,
            b: 1
        }
    )));
    world.assert_clean();
}

/// The first live session (2026-10-12) stopped here: the client's tutorial
/// sent `49 02` and `46 0A`, srp answered `83` alone, and no weapon could
/// be dragged. The client highlights the two weapons when the inventory
/// comes again (`tutorial_model`).
#[test]
fn the_tutorials_weapon_step_is_answered_with_the_inventory() {
    use survarium::v026g::lobby::tutorial_model::{CHOICE, State, Stuck, WEAPON_SLOT};
    let mut world = World::new(Duration::from_secs(60));
    let alice = world.connect("Alice");
    world.take(alice);
    // The sign-in's `9D` started the tutorial at the inventory tab.
    assert_eq!(
        world.client(alice).model.tutorial().state(),
        State::ClickInventory
    );
    let kit = world.account(alice).kits[0].id;
    // The player opens the inventory: the client's two requests.
    let requests = world
        .client(alice)
        .model
        .tutorial_mut()
        .inventory_opened(kit)
        .unwrap();
    assert_eq!(requests[1], (0x46, vec![0x0A]));
    // `49 02` alone: its `83`, and still nothing to drag.
    world.ask(alice, requests[0].clone());
    assert_eq!(world.client(alice).ids, [0x83]);
    world.take(alice);
    assert_eq!(
        world
            .client(alice)
            .model
            .tutorial_mut()
            .drag_started(CHOICE[0]),
        Err(Stuck::NotHighlighted(CHOICE[0]))
    );
    // `46 0A`: the inventory, the account and the money, in that order.
    world.ask(alice, requests[1].clone());
    assert_eq!(world.client(alice).ids, [0x89, 0x87, 0x8B]);
    let decoded = world.take(alice);
    let Decoded::Inventory { items, .. } = &decoded[0] else {
        panic!("not the inventory: {:?}", decoded[0]);
    };
    // Nothing was given: a new account owns the three starting weapons and
    // their two kinds of ammo.
    let mut dicts: Vec<u16> = items.iter().map(|item| item.dict_id).collect();
    dicts.sort_unstable();
    assert_eq!(dicts, [18, 50, 104, 106, 107]);
    // Both weapons can be dragged now, and dropped on weapon slot 1.
    let model = &mut world.client(alice).model;
    assert_eq!(model.tutorial().highlighted(), CHOICE);
    model.tutorial_mut().drag_started(CHOICE[1]).unwrap();
    model.tutorial_mut().dropped_on_weapon_slot(CHOICE[1]).unwrap();
    assert_eq!(model.tutorial().state(), State::ClickAmmo);
    // The drop's own request (`46 00`): the Mosin into weapon slot 1.
    let mosin = world
        .account(alice)
        .inventory
        .iter()
        .find(|item| item.dict_id == CHOICE[1])
        .unwrap()
        .item_id;
    world.ask(
        alice,
        request::move_item(kit, mosin, CHOICE[1], INVENTORY_SLOT, WEAPON_SLOT, 1),
    );
    world.take(alice);
    {
        let account = world.account(alice);
        let slot = account.kits[0].slots[usize::from(WEAPON_SLOT)].unwrap();
        assert_eq!(slot.item_id, mosin);
    }

    // An account in the tutorial that lost one of the two gets it back.
    let bob = world.connect("Bob");
    world.take(bob);
    world.change(bob, |account| {
        account.inventory.retain(|item| item.dict_id != CHOICE[0]);
    });
    let kit = world.account(bob).kits[0].id;
    world
        .client(bob)
        .model
        .tutorial_mut()
        .inventory_opened(kit)
        .unwrap();
    world.ask(bob, (0x46, vec![0x0A]));
    world.take(bob);
    assert_eq!(world.client(bob).model.tutorial().highlighted(), CHOICE);
    assert_eq!(
        world
            .account(bob)
            .inventory
            .iter()
            .filter(|item| item.dict_id == CHOICE[0])
            .count(),
        1
    );
    world.assert_clean();
}

#[test]
fn the_two_boxes_of_the_kit_window_are_kept() {
    let mut world = World::new(Duration::from_secs(60));
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
