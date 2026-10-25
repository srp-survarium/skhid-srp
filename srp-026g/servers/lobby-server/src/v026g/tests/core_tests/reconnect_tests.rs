//! The same game connecting again with the session it has: after the
//! server came up again (its sessions restored), and after its link went
//! silent. The game is the same object (`Client::reconnect`,
//! `transport.md` §6.1): it holds all it held, and reads the sign-in's
//! pushes a second time with the same readers.
//!
//! The first live 0.26g client crashed on exactly this on 2026-10-12, half
//! a second after it connected to a restarted server: the daily bonus
//! (`99`) had its item's id before its stack flag, the client read past the
//! message's end, and the second time the bytes there were not zeros.

use survarium::v026g::lobby::client_rules::RankingPage;

use super::*;

impl World {
    /// The server stops and comes up again: a new lobby over the sessions
    /// and accounts that were kept (live: restored from the database). It
    /// knows no link and nobody who was away.
    fn restart(&mut self) {
        self.core = Core::new(
            self.store,
            self.accounts,
            self.results,
            self.host,
            self.inbox,
            Settings::local("127.0.0.1:0"),
            self.now,
        );
    }

    /// The game of `index` connects again, from a new port, with what it
    /// holds, and signs in as the game does: asking for the shop only if
    /// it has none.
    fn game_connects_again(&mut self, index: usize, port: u16) {
        let session_id = self.ids[index].1;
        let old = std::mem::replace(&mut self.clients[index].1, TestClient::new());
        let mut client = old.reconnected();
        client.decoded.clear();
        client.ids.clear();
        client.sign_in_as_held(session_id);
        self.clients[index] = (format!("127.0.0.1:{port}").parse().unwrap(), client);
        self.settle();
    }

    /// The last message with this id the lobby pushed to a client, with
    /// its id byte.
    fn last_pushed(&self, index: usize, id: u8) -> Vec<u8> {
        let message = self.core.links[&self.clients[index].0]
            .pushed
            .iter()
            .rev()
            .find(|message| message.id_byte() == id)
            .expect("the message was pushed");
        let mut bytes = vec![message.id_byte()];
        bytes.extend_from_slice(message.body());
        bytes
    }
}

/// A game's ranking page, kept by the game's own script's rules
/// (`client_rules::RankingPage`), fed with what the lobby sends.
struct Ranking {
    page: RankingPage<(String, u32)>,
    /// What the last `8F` stored: a `9D` hands it to the page.
    count: usize,
}

impl Ranking {
    fn new() -> Self {
        Self {
            page: RankingPage::new(),
            count: 0,
        }
    }

    /// What the game took, in order.
    fn take(&mut self, decoded: &[Decoded]) {
        for message in decoded {
            match message {
                Decoded::BaseInfo { ranked_players, .. } => self.count = *ranked_players as usize,
                Decoded::Tutorial { .. } => self.page.set_total(self.count),
                Decoded::EloList(rows) => {
                    let rows: Vec<(String, u32)> =
                        rows.iter().map(|row| (row.0.clone(), row.1)).collect();
                    self.page.fill(&rows);
                }
                _ => {}
            }
        }
    }

    /// The names on the page's lines; an empty line is "".
    fn lines(&self) -> Vec<&str> {
        self.page
            .shown()
            .iter()
            .map(|row| row.as_ref().map_or("", |row| row.0.as_str()))
            .collect()
    }
}

impl World {
    /// The player opens the ranking page: its request, if it makes one, is
    /// sent and answered.
    fn open_ranking(&mut self, index: usize, ranking: &mut Ranking) {
        if let Some((start, count)) = ranking.page.open() {
            self.ask(index, request::ranking(start, count));
        }
        let decoded = self.take(index);
        ranking.take(&decoded);
    }
}

/// Seen live on 2026-10-12, twice: a ranking request that got no answer
/// (srp was started again) left the page shifted for as long as the game
/// ran, and a page opened after somebody registered asked for the old
/// number of rows.
#[test]
fn the_ranking_page_is_right_after_a_restart_and_a_new_player() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let mut ranking = Ranking::new();
    let decoded = world.take(alice);
    ranking.take(&decoded);
    // Alice was told of Bob and Carol as they registered.
    assert_eq!(ranking.count, 3);
    world.open_ranking(alice, &mut ranking);
    assert_eq!(ranking.lines(), ["Alice", "Bob", "Carol"]);

    // She opens the page as srp goes down: the request is never answered.
    assert_eq!(ranking.page.open(), Some((0, 3)));
    assert_eq!(ranking.page.waits_for(), 3);
    world.restart();
    for index in [alice, bob, carol] {
        world.game_connects_again(index, 49340 + index as u16);
    }
    // The sign-in's flush: the three rows she waited for, then place 1 up
    // to every row a `91` holds. The page waits for nothing any more.
    let decoded = world.take(alice);
    let flush = decoded
        .iter()
        .find_map(|message| match message {
            Decoded::EloList(rows) => Some(rows.clone()),
            _ => None,
        })
        .expect("the flush");
    assert_eq!(flush.len(), 255);
    let places: Vec<(&str, u32)> = flush.iter().map(|row| (row.0.as_str(), row.1)).collect();
    assert_eq!(places[..3], [("Alice", 1), ("Bob", 2), ("Carol", 3)]);
    assert!(places[3..].iter().all(|row| *row == ("Alice", 1)));
    ranking.take(&decoded);
    assert_eq!(ranking.page.waits_for(), 0);
    // The page was open: the flush answered it.
    assert_eq!(ranking.lines(), ["Alice", "Bob", "Carol"]);

    // Dave registers. Alice, in the lobby, is told the new count at once:
    // `8F`, then the `9D` that hands it to the page, and nothing else.
    world.take(alice);
    let dave = world.connect("Dave");
    let decoded = world.take(alice);
    let told: Vec<&Decoded> = decoded
        .iter()
        .filter(|message| matches!(message, Decoded::BaseInfo { .. } | Decoded::Tutorial { .. }))
        .collect();
    assert!(matches!(
        told[..],
        [
            Decoded::BaseInfo {
                ranked_players: 4,
                ..
            },
            Decoded::Tutorial { show: false, .. }
        ]
    ));
    assert!(
        !decoded
            .iter()
            .any(|message| matches!(message, Decoded::EloList(_)))
    );
    ranking.take(&decoded);
    // So the page asks for four rows the first time it is opened.
    world.open_ranking(alice, &mut ranking);
    assert_eq!(ranking.lines(), ["Alice", "Bob", "Carol", "Dave"]);
    world.open_ranking(alice, &mut ranking);
    assert_eq!(ranking.lines(), ["Alice", "Bob", "Carol", "Dave"]);
    assert_eq!(ranking.page.waits_for(), 0);

    // The count is said once.
    world.advance(Duration::from_secs(5));
    assert!(
        !world
            .take(alice)
            .iter()
            .any(|message| matches!(message, Decoded::BaseInfo { .. }))
    );
    let _ = dave;
    world.assert_clean();
}

/// The sign-in's ordered pushes for a game that holds the shop.
const SECOND_SIGN_IN: [u8; 21] = [
    0x8F, 0x90, 0x8B, 0x96, 0x8E, 0x8D, 0x94, 0x8A, 0x8E, 0x89, 0x87, 0x8C, 0x92, 0x9C, 0x9D, 0x9B,
    0x85, 0x91, 0xA3, 0xA3, 0x99,
];

fn ordered(ids: &[u8]) -> Vec<u8> {
    ids.iter()
        .copied()
        .filter(|id| !matches!(id, 0x80 | 0xA1 | 0x9F))
        .collect()
}

#[test]
fn the_daily_bonus_is_in_the_clients_order_byte_for_byte() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    // One day, nothing won; a stack (the flag FIRST), item id 0, dict
    // 0xD7, 50 of it; no modifications, no upgrades; not given; the
    // welcome window. 17 bytes, and the client reads 17.
    assert_eq!(
        world.last_pushed(alice, 0x99),
        [
            0x99, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0xD7, 0x00, 0x32, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01
        ]
    );
    world.assert_clean();
}

#[test]
fn a_game_that_connects_again_after_a_restart_takes_the_whole_sign_in_again() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let held = world.client(alice).model.inventory().to_vec();
    assert!(!world.client(alice).model.needs_shop_items());
    world.take(alice);

    world.restart();
    assert!(world.core.links.is_empty());
    // The game's own link gave up; five seconds later it tries again.
    world.game_connects_again(alice, 49334);
    let client = world.client(alice);
    assert!(client.model.is_signed_in());
    // The whole sign-in again, 0x80 first, without the shop it holds.
    assert_eq!(client.ids[0], 0x80);
    assert_eq!(ordered(&client.ids), SECOND_SIGN_IN);
    assert!(client.ids.contains(&0xA1));
    // What it holds is what it held.
    assert_eq!(client.model.inventory(), held);
    // The bonus of the same day: the same bytes but for the window, which
    // is not opened a second time.
    assert_eq!(
        world.last_pushed(alice, 0x99),
        [
            0x99, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0xD7, 0x00, 0x32, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00
        ]
    );
    let decoded = world.take(alice);
    assert_eq!(queue_state(&decoded), Some(ClientState::Lobby));
    // And it is served as before.
    world.ask(alice, request::query(0x43));
    assert!(matches!(
        world.take(alice)[..],
        [Decoded::DailyBonus { .. }]
    ));
    // Bob's game does the same a little later.
    world.game_connects_again(bob, 49335);
    assert!(world.client(bob).model.is_signed_in());
    assert_eq!(ordered(&world.client(bob).ids), SECOND_SIGN_IN);
    assert_eq!(world.store.players_online(), 2);
    world.assert_clean();
}

#[test]
fn a_game_whose_link_went_silent_connects_again_with_all_it_holds() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let session_id = world.ids[alice].1;
    let held = world.client(alice).model.inventory().to_vec();
    world.take(alice);
    // Its link goes after 30 s of silence; the session waits a minute.
    world.silent_for(alice, Duration::from_secs(45));
    assert!(!world.has_link(alice));
    world.game_connects_again(alice, 49336);
    let client = world.client(alice);
    assert!(client.model.is_signed_in());
    assert_eq!(client.ids[0], 0x80);
    assert_eq!(ordered(&client.ids), SECOND_SIGN_IN);
    assert_eq!(client.model.inventory(), held);
    assert!(!world.store.has_ended(session_id));
    world.assert_clean();

    // And while the server's link still stands (the game gave up after
    // its own 5 s): the old link is retired, the new one signed in.
    world.game_connects_again(alice, 49337);
    let client = world.client(alice);
    assert!(client.model.is_signed_in());
    assert_eq!(ordered(&client.ids), SECOND_SIGN_IN);
    assert_eq!(client.model.inventory(), held);
    world.assert_clean();
}
