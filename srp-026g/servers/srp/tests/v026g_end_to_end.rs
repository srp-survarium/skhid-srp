//! The whole 0.26g stack of srp, in-process and for real: the login server
//! (TLS), the lobby and the match server on free localhost ports, a
//! database file in a temporary folder, and two scripted 0.26g clients (the
//! lobby's and the match server's own test clients) doing what two games
//! do from the login screen to the results screen and a second sign-in.
//!
//! Nothing is mocked: the servers are the ones `srp` starts
//! (`srp::v026g::Stack`), with the timings it runs with. Every message a
//! client takes goes through the strict client models
//! (`survarium::v026g::lobby::client_model`, `match_msgs::client_model`):
//! what a real client would drop, misread or crash on is a fault, and the
//! tests end with none.
//!
//! What stands in for a game: the simulation (`test_client::Sim` gives every
//! client in step the same well-formed world for the same tick, written
//! with the snapshot writer), and two things that make the match's end
//! come within seconds, not by luck:
//! - one shortcut past the 20 s countdown: the clients' rule is set to "in
//!   play", as the match server's own socket test does;
//! - the clients report the hash of every tick (0xDF), where a real client
//!   reports one every 100 ms. The server trusts an uploaded world only
//!   when two clients reported its tick, and two clients on a 100 ms beat
//!   need not name the same ticks: with every tick reported, the first
//!   world uploaded after the end is the one that ends the match.
//!
//! The last two tests are a match of one such client and the test bot
//! (`srp::bot`, run as the `srp-bot` binary runs it): with the bot's
//! account named in `SRP_BOT_ACCOUNTS` the match is played to its results,
//! and without it nothing about bots is on.
//!
//! Every wait has a deadline; set `SRP_TEST_LOG` to see the servers' log.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lobby_server::v026g::test_client::{TestClient, request};
use match_server::v026g::static_id;
use match_server::v026g::test_client::{Client as MatchClient, INITIAL_MS};
use session::v026g::{GameAccounts, SqliteGameRepository, rules};
use session::{LockExt, SessionStore};
use srp::bot::SignIn;
use srp::v026g::{Options, Stack};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::client_model::{Decoded, Permitted};
use survarium::v026g::lobby::client_rules::{NO_STATIC_ID, connects_to_match};
use survarium::v026g::lobby::requests::INVENTORY_SLOT;
use survarium::v026g::lobby::{ClientState, FULL_CONDITION};
use survarium::v026g::match_msgs::snapshot::{self, Tdm};
use survarium::v026g::match_msgs::{ServerMessage, TICK_MS};
use survarium::v026g::prices;

/// Team Deathmatch on level_01 (`maps_dict` id 117).
const MAP: u8 = 117;
/// The longest any one step may take.
const SOON: Duration = Duration::from_secs(10);
/// From Play by two players to their room's match: its vote (15 s), the
/// wait of the room Play makes (30 s), and a little more.
const ROOM_MATCH: Duration = Duration::from_secs(55);
/// The match's end: the servers run with their real timings (a world is
/// asked for every 5 s and checked against 2.5 s of hash reports, and the
/// end is told 3 s after the rule had it).
const MATCH_END: Duration = Duration::from_secs(25);

// ------------------------------------------------------------- the servers

/// A fresh folder under the build's temporary one.
fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "srp-e2e-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The servers' log on stderr when `SRP_TEST_LOG` is set, and the match
/// server's snapshot folder moved out of the repository.
fn setup() {
    struct StderrLog;
    impl log::Log for StderrLog {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            eprintln!("[{}] {}", record.level(), record.args());
        }
        fn flush(&self) {}
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Before any server thread: the match server would keep the
        // uploaded worlds under `logs/snapshots` of the crate.
        // SAFETY: the tests of this file all come through here first, and
        // nothing else reads the environment yet.
        unsafe { std::env::set_var("SRP_SNAPSHOT_DIR", "off") };
        if std::env::var_os("SRP_TEST_LOG").is_some() && log::set_logger(&StderrLog).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
    });
}

/// srp's servers over a database in `dir`, as `main` starts them, on free
/// ports.
fn start(dir: &Path) -> Stack {
    start_with_bots(dir, &[])
}

/// As [`start`], with `SRP_BOT_ACCOUNTS` naming `bots`.
fn start_with_bots(dir: &Path, bots: &[&str]) -> Stack {
    start_with(dir, bots, |_| {})
}

/// As [`start_with_bots`], the options changed by `change` last.
fn start_with(dir: &Path, bots: &[&str], change: impl FnOnce(&mut Options)) -> Stack {
    setup();
    let store = SessionStore::with_repository(
        session::DEFAULT_GATHER_WINDOW,
        Box::new(session::persist::open_in(dir).expect("the database")),
    )
    .expect("the accounts");
    let accounts = GameAccounts::with_repository(Box::new(
        SqliteGameRepository::open_in(dir).expect("the 0.26g tables"),
    ))
    .expect("the 0.26g accounts");
    let certs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../certs");
    let mut options = Options::local(&certs);
    // One map, so the scripted simulation knows its mode
    // (`SRP_MATCH_MAPS`).
    options.lobby.maps = vec![MAP];
    options.bots = session::v026g::bots::parse(&bots.join(","));
    change(&mut options);
    Stack::start(Arc::new(store), Arc::new(accounts), options).expect("the servers")
}

// ------------------------------------------------------------- the clients

/// Sign in as the 0.26g client does with a typed password (the test
/// bot's sign-in, `srp::bot::sign_in`): the sign-in in the clear, the TLS
/// handshake, the digest and the password.
fn sign_in(login: SocketAddr, user_name: &str, password: &str) -> SignIn {
    srp::bot::sign_in(login, user_name, password).expect("the login server")
}

/// The session and the lobby's address of a `08` reply.
fn accepted(sign_in: &SignIn) -> (u32, SocketAddr) {
    assert_eq!(sign_in.answer, 0x0B, "the user name was not taken");
    assert_eq!(sign_in.reply.len(), 11, "{:02X?}", sign_in.reply);
    assert_eq!(
        sign_in.reply[0], 0x08,
        "not signed in: {:02X?}",
        sign_in.reply
    );
    sign_in
        .accepted()
        .expect("a session and the lobby's address")
}

/// One game: its lobby link, and its match link while it is in a match.
struct Game {
    name: &'static str,
    revision: u8,
    session: u32,
    lobby_addr: SocketAddr,
    lobby_socket: UdpSocket,
    lobby: TestClient,
    in_match: Option<(SocketAddr, UdpSocket, MatchClient)>,
    /// The `Match id` of the last static info it took: a game keeps it
    /// until it is started again, whatever became of the match.
    static_id: u32,
    /// The last tick whose hash it reported.
    hashed: u32,
    started: Instant,
}

fn socket() -> UdpSocket {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a free port");
    socket.set_nonblocking(true).unwrap();
    socket
}

impl Game {
    /// Signed in at the login server as a game of the newest patch (the
    /// late join's: only such a game plays in rooms); its lobby link is
    /// started.
    fn signed_in(stack: &Stack, name: &'static str, password: &str) -> Self {
        let revision = survarium::v026g::match_msgs::late_join::PATCH_REVISION;
        let reply =
            srp::bot::sign_in_with_revision(stack.login_addr, name, password, revision).unwrap();
        let (session, lobby_addr) = accepted(&reply);
        assert_eq!(
            lobby_addr, stack.lobby_addr,
            "the lobby the client is sent to"
        );
        let mut lobby = TestClient::new();
        lobby.sign_in(session, true);
        Self {
            name,
            revision,
            session,
            lobby_addr,
            lobby_socket: socket(),
            lobby,
            in_match: None,
            static_id: NO_STATIC_ID,
            hashed: 0,
            started: Instant::now(),
        }
    }

    /// Connect to the match server (0xD8) with the session the lobby's `82`
    /// gave, as the game does: it never sends its login session there. And
    /// only for an `82` the game acts on: one that names the id of the last
    /// static info it took is dropped by it.
    fn connect_to_match(&mut self, server: SocketAddr) {
        self.connect_to_match_of(server, 2);
    }

    /// [`Game::connect_to_match`] of a match of `players` places.
    fn connect_to_match_of(&mut self, server: SocketAddr, players: usize) {
        let (match_id, session) = self
            .find(|message| match message {
                Decoded::ConnectToMatch {
                    match_id,
                    match_session,
                    ..
                } => Some((*match_id, *match_session)),
                _ => None,
            })
            .expect("sent to a match");
        assert_eq!(session, self.session, "the session the match seats by");
        assert!(
            connects_to_match(self.static_id, match_id),
            "{}: a game holding the static info {:#x} does nothing on an 82 for {match_id:#x}",
            self.name,
            self.static_id
        );
        let mut client = if survarium::v026g::match_msgs::late_join::spoken_by(self.revision) {
            MatchClient::growing(session, players, MAP)
        } else {
            MatchClient::new(session, true, players, MAP)
        };
        // Every tick is reported here (see the top of the file).
        client.no_hashes = true;
        client.connect();
        self.hashed = 0;
        self.in_match = Some((server, socket(), client));
    }

    /// Send what is due on both links and take what has come.
    fn pump(&mut self) {
        let mut buffer = [0_u8; 2048];
        for datagram in self.lobby.poll(Instant::now()) {
            let _ = self.lobby_socket.send_to(&datagram, self.lobby_addr);
        }
        while let Ok((len, from)) = self.lobby_socket.recv_from(&mut buffer) {
            if from == self.lobby_addr {
                self.lobby.receive(&buffer[..len]);
            }
        }
        if let Some((server, socket, client)) = &mut self.in_match {
            let ms = self.started.elapsed().as_millis() as u64;
            // The hash of each world that has become final, as 0xDF: never
            // of the world a snapshot gave, nor of one before it.
            if let Some(horizon) = client.horizon().filter(|_| !client.closed) {
                let resync = client.model.last_resync.unwrap_or(0);
                let from = (self.hashed.max(resync) + TICK_MS).max(horizon.saturating_sub(500));
                for time in (from..=horizon).step_by(TICK_MS as usize) {
                    let crc = snapshot::crc(&client.sim.world(time));
                    client.report(time, crc);
                    self.hashed = time;
                }
            }
            for datagram in client.poll(ms) {
                let _ = socket.send_to(&datagram, *server);
            }
            while let Ok((len, from)) = socket.recv_from(&mut buffer) {
                if from == *server {
                    client.receive(&buffer[..len]);
                }
            }
            if let Some(options) = &client.model.options {
                self.static_id = options.match_id;
            }
        }
    }

    fn ask(&mut self, (id, body): (u8, Vec<u8>)) {
        self.lobby.send(id, &body);
    }

    /// What the lobby link took since the last call.
    fn take(&mut self) -> Vec<Decoded> {
        self.lobby.ids.clear();
        std::mem::take(&mut self.lobby.decoded)
    }

    fn has(&self, pick: impl Fn(&Decoded) -> bool) -> bool {
        self.lobby.decoded.iter().any(pick)
    }

    fn find<T>(&self, pick: impl Fn(&Decoded) -> Option<T>) -> Option<T> {
        self.lobby.decoded.iter().rev().find_map(pick)
    }

    fn of_match(&self) -> &MatchClient {
        &self.in_match.as_ref().expect("in a match").2
    }

    fn of_match_mut(&mut self) -> &mut MatchClient {
        &mut self.in_match.as_mut().expect("in a match").2
    }

    /// Nothing a real client would have dropped, misread or crashed on.
    fn assert_clean(&self) {
        assert_eq!(self.lobby.faults, Vec::<String>::new(), "{}", self.name);
        assert_eq!(self.lobby.waiting_in_order(), 0, "{}", self.name);
        assert_eq!(self.lobby.order_drops, 0, "{}", self.name);
        if let Some((_, _, client)) = &self.in_match {
            assert_eq!(client.faults, Vec::<String>::new(), "{}", self.name);
        }
    }
}

/// Run the games until `done`, or fail with `what` after `limit`.
fn until(
    games: &mut [&mut Game],
    limit: Duration,
    what: &str,
    done: impl Fn(&[&mut Game]) -> bool,
) {
    let deadline = Instant::now() + limit;
    loop {
        for game in games.iter_mut() {
            game.pump();
        }
        if done(games) {
            return;
        }
        if Instant::now() >= deadline {
            for game in games.iter() {
                eprintln!(
                    "{}: lobby faults {:?}, last lobby messages {:02X?}, match faults {:?}",
                    game.name,
                    game.lobby.faults,
                    game.lobby.ids.iter().rev().take(12).collect::<Vec<_>>(),
                    game.in_match.as_ref().map(|(_, _, client)| &client.faults)
                );
            }
            panic!("timed out waiting for {what}");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn run_for(games: &mut [&mut Game], time: Duration) {
    let end = Instant::now() + time;
    while Instant::now() < end {
        for game in games.iter_mut() {
            game.pump();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The whole sign-in push has come: the squad info is its last message.
fn in_lobby(game: &Game) -> bool {
    let ids = &game.lobby.ids;
    ids.contains(&0x85) && ids.contains(&0xA1) && ids.iter().filter(|id| **id == 0xA3).count() == 2
}

/// The balance the client was last told: `(silver, gold)`.
fn money_told(game: &Game) -> (u32, u32) {
    game.find(|message| match message {
        Decoded::Money { generic, premium } => Some((*generic, *premium)),
        _ => None,
    })
    .expect("the money")
}

/// The account as the client was last told: `(silver, gold, experience)`.
fn told(game: &Game) -> (u32, u32, u32) {
    let money = money_told(game);
    let experience = game
        .find(|message| match message {
            Decoded::Skills { exp_total, .. } => Some(*exp_total),
            _ => None,
        })
        .expect("the level");
    (money.0, money.1, experience)
}

fn queue_state(game: &Game) -> Option<ClientState> {
    game.find(|message| match message {
        Decoded::ClientState { state, .. } => Some(*state),
        _ => None,
    })
}

fn account_id(stack: &Stack, name: &str) -> u32 {
    stack.store.account_id_by_name(name).expect("an account")
}

/// `(silver, gold, experience)` of the account in the server's memory.
fn held(stack: &Stack, name: &str) -> (u32, u32, u32) {
    let account = stack
        .accounts
        .get(account_id(stack, name))
        .expect("a 0.26g account");
    let account = account.locked();
    (
        account.generic_money,
        account.premium_money,
        account.exp_total,
    )
}

// ------------------------------------------------------------------ tests

#[test]
fn two_new_players_sign_in_shop_play_a_match_and_are_paid() {
    let dir = temp_dir("match");
    let stack = start(&dir);

    // ---- 1. Two new names sign in over TLS: the accounts are made, each
    // answer has a session and the lobby's UDP address.
    let mut alice = Game::signed_in(&stack, "E2eAlice", "alice-password-1");
    let mut bob = Game::signed_in(&stack, "E2eBob", "bob-password-2");
    assert_ne!(alice.session, bob.session);
    assert_eq!(stack.store.registered_accounts(), 2);

    // ---- 2. The lobby link (UDP): 0x80, then the whole account, every
    // message read by the client model.
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the sign-in pushes",
        |games| games.iter().all(|game| in_lobby(game)),
    );
    for game in [&alice, &bob] {
        assert!(game.lobby.model.is_signed_in());
        assert_eq!(game.lobby.ids[0], 0x80, "{}", game.name);
        // The starter: the three free weapons, every ammo and the starter
        // kit (`session::v026g::tutorial`), as the server holds it.
        let owned = stack
            .accounts
            .get(account_id(&stack, game.name))
            .expect("a 0.26g account")
            .locked()
            .inventory
            .len();
        let ammo = session::v026g::tutorial::starter_ammo(Dictionary::shared()).len();
        assert!(owned > 3 + ammo, "{owned}");
        assert_eq!(game.lobby.model.inventory().len(), owned);
        assert!(game.has(|message| matches!(
            message,
            Decoded::Account(own) if own.body.name == game.name && own.body.kits.len() == 3
        )));
        assert_eq!(queue_state(game), Some(ClientState::Lobby));
        game.assert_clean();
    }
    let start_money = told(&alice);
    assert_eq!(start_money, held(&stack, "E2eAlice"));
    // A new account's start (there is no tutorial).
    assert_eq!(
        (start_money.0, start_money.1),
        (
            session::v026g::tutorial::SIGNUP_SILVER,
            session::v026g::tutorial::SIGNUP_GOLD
        )
    );
    assert_eq!(start_money.2, session::v026g::tutorial::experience());

    // ---- 3. Alice buys something the shop sells her now: `83 47 00`, the
    // inventory, the account and the money.
    let record = {
        let account = stack.accounts.get(account_id(&stack, "E2eAlice")).unwrap();
        let account = account.locked().clone();
        prices::price_list()
            .iter()
            .find(|record| {
                record.cost > 0
                    && rules::buy(
                        &mut account.clone(),
                        record.dict_id,
                        1,
                        record.faction_id,
                        INVENTORY_SLOT,
                        1,
                        session::v026g::quests::now(),
                    )
                    .is_ok()
            })
            .expect("something a new account can buy")
    };
    alice.take();
    alice.ask(request::buy(
        record.dict_id,
        1,
        record.faction_id,
        INVENTORY_SLOT,
        1,
    ));
    until(&mut [&mut alice, &mut bob], SOON, "the purchase", |games| {
        games[0].has(|message| matches!(message, Decoded::Money { .. }))
            && games[0].has(|message| matches!(message, Decoded::Account(_)))
    });
    let bought = alice
        .find(|message| match message {
            Decoded::Permitted(Permitted::Bought(items)) => Some(items.clone()),
            _ => None,
        })
        .expect("83 47 00");
    assert_eq!((bought.len(), bought[0].dict_id), (1, record.dict_id));
    let after_purchase = held(&stack, "E2eAlice");
    assert_eq!(after_purchase.0, start_money.0 - record.cost);
    assert_eq!(money_told(&alice), (after_purchase.0, after_purchase.1));
    {
        let account = stack.accounts.get(account_id(&stack, "E2eAlice")).unwrap();
        let account = account.locked();
        let item = account.item(bought[0].item_id).expect("the item bought");
        assert_eq!(item.dict_id, record.dict_id);
        if !Dictionary::shared().is_stack(record.dict_id) {
            assert_eq!(item.condition_or_stack, FULL_CONDITION);
        }
        assert_eq!(alice.lobby.model.inventory().len(), account.inventory.len());
    }
    alice.assert_clean();

    // ---- 4. Both press Play: one room has them, its vote and its wait
    // run, the lobby makes the match at the match server, and each is told
    // "match found" and then where to connect (0x82).
    alice.take();
    bob.take();
    alice.ask(request::play(1));
    bob.ask(request::play(1));
    until(
        &mut [&mut alice, &mut bob],
        ROOM_MATCH,
        "connect_to_match",
        |games| {
            games
                .iter()
                .all(|game| game.has(|message| matches!(message, Decoded::ConnectToMatch { .. })))
        },
    );
    let mut tickets = Vec::new();
    for game in [&alice, &bob] {
        let decoded = &game.lobby.decoded;
        let connect = decoded
            .iter()
            .position(|message| matches!(message, Decoded::ConnectToMatch { .. }))
            .unwrap();
        // "Match found" (place 0) came before it.
        assert!(decoded[..connect].iter().any(|message| matches!(
            message,
            Decoded::ClientState {
                state: ClientState::InQueue { place: 0, .. },
                ..
            }
        )));
        let Decoded::ConnectToMatch {
            host,
            port,
            match_id,
            team,
            ..
        } = &decoded[connect]
        else {
            unreachable!()
        };
        assert_eq!(
            (host.as_str(), *port),
            ("127.0.0.1", stack.match_addr.port()),
            "the match server the client is sent to"
        );
        tickets.push((*match_id, *team));
    }
    let match_id = tickets[0].0;
    assert_eq!(tickets[1].0, match_id);
    assert_ne!(tickets[0].1, tickets[1].1, "one player a team");
    assert!(stack.registry.is_open(match_id));
    assert_eq!(stack.registry.match_count(), 1);

    // ---- 5. The match link: 0xD8 with the session, the port, the static
    // info, read back by the match client model.
    alice.connect_to_match(stack.match_addr);
    bob.connect_to_match(stack.match_addr);
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the static info",
        |games| {
            games
                .iter()
                .all(|game| game.of_match().model.options.is_some())
        },
    );
    let mut places = Vec::new();
    for (game, ticket) in [&alice, &bob].into_iter().zip(&tickets) {
        let client = game.of_match();
        let port = stack.match_addr.port();
        assert_eq!(
            client.taken[0],
            ServerMessage::Ports {
                min: port,
                max: port
            }
        );
        let options = client.model.options.as_ref().unwrap();
        // Not the lobby's match id: `registry::static_id`.
        assert_eq!(
            (options.map_id, options.match_id),
            (MAP, static_id(match_id))
        );
        assert_eq!(options.players.len(), 2);
        let place = usize::from(client.model.local_index.expect("its own place"));
        // Its own profile, as the lobby had it: name, team, three kits.
        let own = &options.players[place];
        assert_eq!((own.name.as_str(), own.team), (game.name, ticket.1));
        assert_eq!(own.kits.len(), 3);
        places.push(place as u8);
    }
    assert_ne!(places[0], places[1]);
    // Alice's profile carries what she bought.
    {
        let options = alice.of_match().model.options.as_ref().unwrap();
        let profile = &options.players[usize::from(places[0])];
        let account = stack.accounts.get(account_id(&stack, "E2eAlice")).unwrap();
        // The items a kit wears; the purchase is in the inventory only.
        assert!(profile.items.len() <= account.locked().inventory.len());
        assert!(!profile.items.is_empty());
    }

    // ---- 6. Both load; each uploads its initial world (the patched
    // client's 0xE0 chunks); the server starts both from it (0xE5, 0xE8)
    // and puts them in (0xE9).
    alice.of_match_mut().ready();
    bob.of_match_mut().ready();
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "both in the match",
        |games| {
            games.iter().all(|game| {
                let client = game.of_match();
                let mut entered = client.entered();
                entered.sort_unstable();
                client.model.is_synced() && entered == [0, 1]
            })
        },
    );
    for game in [&alice, &bob] {
        let client = game.of_match();
        // The world it started from is the initial one, byte for byte.
        let synced = client.got(|message| match message {
            ServerMessage::Snapshot { body, .. } => Some(body.clone()),
            _ => None,
        });
        assert_eq!(synced[0], client.sim.world(INITIAL_MS), "{}", game.name);
        assert_eq!(client.model.generation.0, 0);
    }
    // The later one started behind the match clock: a moment for the pings
    // to bring it up.
    run_for(&mut [&mut alice, &mut bob], Duration::from_millis(2_600));

    // ---- 7. Inputs both ways, byte for byte, and none back to its sender.
    let from_alice = alice.of_match_mut().input(100, 0x8000_0021);
    let from_bob = bob.of_match_mut().input(100, 0x40);
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the inputs relayed",
        |games| {
            games[1].of_match().relays_of(places[0]) == [from_alice]
                && games[0].of_match().relays_of(places[1]) == [from_bob]
        },
    );
    assert!(alice.of_match().relays_of(places[0]).is_empty());
    assert!(bob.of_match().relays_of(places[1]).is_empty());

    // ---- 8. The worlds agree: hash reports go up for every tick, worlds
    // are asked for (0xF6) and uploaded, and nobody is resynced.
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "an upload request",
        |games| {
            games
                .iter()
                .any(|game| game.of_match().upload_requests >= 1 && game.of_match().uploads >= 2)
        },
    );
    for game in [&alice, &bob] {
        assert_eq!(game.of_match().model.syncs, 1, "{} was resynced", game.name);
        assert!(!game.of_match().closed);
        game.assert_clean();
    }
    assert!(stack.results.find(match_id).is_none());
    let before_match = (held(&stack, "E2eAlice"), held(&stack, "E2eBob"));
    // What the launcher's page lists (`web_server::MatchCounter` over the
    // registry): this match, a player a team, its countdown running, and
    // the standing of the world the two vouch for.
    let page = stack.web_game();
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the page's word on the match",
        |_| {
            let live = page.matches.live_matches();
            live.len() == 1 && live[0].standing.is_some()
        },
    );
    let live = page.matches.live_matches().remove(0);
    assert_eq!((live.map_id, live.players), (MAP, [1, 1]));
    assert_eq!(live.phase, web_server::LivePhase::Countdown);
    assert_eq!(live.played, None);
    assert!(
        live.time_left
            .is_some_and(|left| left > Duration::from_secs(900))
    );
    // ---- 9. The match ends by its mode's rule: Bob's team has no tickets
    // and nobody alive. (The rule is set to "in play" on both clients at
    // once: the shortcut past the 20 s countdown.) The server reads it
    // from a world both clients vouch for and tells each it is over
    // (0xEA with its own index).
    let at = alice.of_match().clock().max(bob.of_match().clock()) / 50 * 50 + 150;
    let mut rule = alice.of_match().sim.rule_at(at);
    rule.state = 3;
    rule.start = Some(INITIAL_MS);
    let mut tickets_left = [64, 64];
    tickets_left[usize::from(tickets[1].1)] = 0;
    for game in [&mut alice, &mut bob] {
        let sim = &mut game.of_match_mut().sim;
        sim.force_rule(rule);
        sim.tdm_from = Some((
            at,
            Tdm {
                tickets: tickets_left,
                alive_players_mask: 1 << places[0],
                active_players_mask: 0b11,
            },
        ));
    }
    until(
        &mut [&mut alice, &mut bob],
        MATCH_END,
        "the match's end",
        |games| games.iter().all(|game| game.of_match().told_to_leave),
    );
    for game in [&alice, &bob] {
        game.assert_clean();
    }

    // ---- 10. The rewards are settled before the clients hear the end;
    // back in the lobby (`85 00`) each gets what the match changed of its
    // account and the results screen (0x97). Bob's client says it is back
    // (`31 <order> FF`); Alice's waits for the lobby.
    bob.ask(request::leave(1, true));
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the results screens",
        |games| {
            games
                .iter()
                .all(|game| game.has(|message| matches!(message, Decoded::MatchStats { .. })))
        },
    );
    let report = stack.results.find(match_id).expect("the match's report");
    assert_eq!(report.players.len(), 2);
    let winner_team = tickets[0].1;
    for ((game, place), before) in [&alice, &bob]
        .into_iter()
        .zip(&places)
        .zip([before_match.0, before_match.1])
    {
        let decoded = &game.lobby.decoded;
        let screens: Vec<_> = decoded
            .iter()
            .filter_map(|message| match message {
                Decoded::MatchStats { stats, own } => Some((stats, own)),
                _ => None,
            })
            .collect();
        assert_eq!(screens.len(), 1, "{}: one results screen", game.name);
        let (stats, own) = screens[0];
        assert_eq!(stats.match_id, match_id);
        assert_eq!(stats.winner_team, winner_team);
        assert_eq!(stats.rows.len(), 2);
        assert_eq!(own.player_id, u32::from(*place) + 1);
        // In the lobby state first; the account (after its inventory)
        // before the screen, which reads its bars from it.
        let screen = decoded
            .iter()
            .position(|message| matches!(message, Decoded::MatchStats { .. }))
            .unwrap();
        let lobby = decoded
            .iter()
            .position(|message| {
                matches!(
                    message,
                    Decoded::ClientState {
                        state: ClientState::Lobby,
                        ..
                    }
                )
            })
            .expect("85 00");
        assert!(lobby < screen, "{}", game.name);
        let inventory = decoded[..screen]
            .iter()
            .rposition(|message| matches!(message, Decoded::Inventory { .. }))
            .expect("the inventory");
        let account = decoded[..screen]
            .iter()
            .rposition(|message| matches!(message, Decoded::Account(_)))
            .expect("the account");
        assert!(lobby < inventory && inventory < account, "{}", game.name);
        // Paid: experience and silver for staying to the end, more for the
        // win; the client was told what the account now has.
        let now = held(&stack, game.name);
        assert!(
            now.0 > before.0,
            "{}: silver {before:?} -> {now:?}",
            game.name
        );
        assert!(now.2 > before.2, "{}: experience", game.name);
        assert_eq!(told(game), now, "{}", game.name);
        game.assert_clean();
    }
    let (alice_paid, bob_paid) = (held(&stack, "E2eAlice"), held(&stack, "E2eBob"));
    assert!(
        alice_paid.2 - before_match.0.2 > bob_paid.2 - before_match.1.2,
        "the winner earns more"
    );
    assert!(report.players[usize::from(places[0])].reward.won);
    assert!(!report.players[usize::from(places[1])].reward.won);

    // The lobby has freed the match; a moment later the match server has
    // too, and the match links are closed.
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the match closed",
        |_| !stack.registry.is_open(match_id) && stack.store.match_count() == 0,
    );
    assert_eq!(stack.registry.seat(alice.session), None);
    assert!(page.matches.live_matches().is_empty());
    alice.in_match = None;
    bob.in_match = None;

    // The screen again on request (`41 00`), and a row's kits (`3D 00`).
    alice.take();
    alice.ask(request::match_stats(match_id));
    alice.ask(request::player_details(u32::from(places[1]) + 1, match_id));
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the screen again",
        |games| {
            games[0].has(|message| matches!(message, Decoded::MatchStats { .. }))
            && games[0].has(|message| {
                matches!(message, Decoded::PlayerDetails { player, .. } if player.name == "E2eBob")
            })
        },
    );
    alice.assert_clean();

    // ---- 11. Both games close (`FA`): the sessions end with their links.
    let old_sessions = (alice.session, bob.session);
    alice.lobby.leave();
    bob.lobby.leave();
    until(&mut [&mut alice, &mut bob], SOON, "the sign-out", |games| {
        games.iter().all(|game| game.lobby.disconnected)
    });
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the sessions' end",
        |_| stack.store.has_ended(old_sessions.0) && stack.store.has_ended(old_sessions.1),
    );

    // ---- 12. Alice signs in again: a new session, and the account as the
    // purchase and the match left it. No results screen a second time.
    let mut again = Game::signed_in(&stack, "E2eAlice", "alice-password-1");
    assert_ne!(again.session, old_sessions.0);
    until(&mut [&mut again], SOON, "the second sign-in", |games| {
        in_lobby(games[0])
    });
    assert_eq!(told(&again), alice_paid);
    assert_eq!(
        again.lobby.model.inventory().len(),
        alice.lobby.model.inventory().len()
    );
    run_for(&mut [&mut again], Duration::from_millis(1_500));
    assert!(!again.has(|message| matches!(message, Decoded::MatchStats { .. })));
    again.assert_clean();
    assert_eq!(stack.store.registered_accounts(), 2);

    // ---- 13. The servers stop; the database file has it all.
    let alice_id = account_id(&stack, "E2eAlice");
    let bob_id = account_id(&stack, "E2eBob");
    stack.stop();
    let kept = GameAccounts::with_repository(Box::new(
        SqliteGameRepository::open_in(&dir).expect("the database again"),
    ))
    .expect("the accounts again");
    for (id, paid) in [(alice_id, alice_paid), (bob_id, bob_paid)] {
        let account = kept.get(id).expect("the account in the file");
        let account = account.locked();
        assert_eq!(
            (
                account.generic_money,
                account.premium_money,
                account.exp_total
            ),
            paid
        );
    }
    assert!(
        kept.get(alice_id)
            .unwrap()
            .locked()
            .item(bought[0].item_id)
            .is_some()
    );
    let store = SessionStore::with_repository(
        Duration::from_secs(60),
        Box::new(session::persist::open_in(&dir).expect("the database again")),
    )
    .expect("the accounts again");
    assert_eq!(store.registered_accounts(), 2);
    assert_eq!(store.account_id_by_name("e2ealice"), Some(alice_id));
}

/// Seen live on 2026-10-12: a player's match connection broke, its game
/// went back to the lobby still running, it pressed Play, was sent to its
/// match again (`82`) and did nothing, because the static info it held
/// carried the match's own id.
#[test]
fn a_player_whose_match_link_broke_goes_back_in_without_a_restart() {
    let dir = temp_dir("back");
    let stack = start(&dir);
    let mut alice = Game::signed_in(&stack, "E2eBackAlice", "alice-password-1");
    let mut bob = Game::signed_in(&stack, "E2eBackBob", "bob-password-2");
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the sign-in pushes",
        |games| games.iter().all(|game| in_lobby(game)),
    );
    alice.take();
    bob.take();
    alice.ask(request::play(1));
    bob.ask(request::play(1));
    let sent_to = |game: &Game| {
        game.find(|message| match message {
            Decoded::ConnectToMatch { match_id, team, .. } => Some((*match_id, *team)),
            _ => None,
        })
    };
    until(
        &mut [&mut alice, &mut bob],
        ROOM_MATCH,
        "connect_to_match",
        |games| games.iter().all(|game| sent_to(game).is_some()),
    );
    let (match_id, team) = sent_to(&alice).unwrap();
    alice.connect_to_match(stack.match_addr);
    bob.connect_to_match(stack.match_addr);
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the static info",
        |games| {
            games
                .iter()
                .all(|game| game.of_match().model.options.is_some())
        },
    );
    alice.of_match_mut().ready();
    bob.of_match_mut().ready();
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "both in the match",
        |games| {
            games.iter().all(|game| {
                let client = game.of_match();
                let mut entered = client.entered();
                entered.sort_unstable();
                client.model.is_synced() && entered == [0, 1]
            })
        },
    );
    let place = alice.of_match().model.local_index.unwrap();
    run_for(&mut [&mut alice, &mut bob], Duration::from_millis(2_600));

    // Alice's game loses the match server and is back in its lobby, still
    // running: it keeps the static info's id. Bob plays on.
    assert_eq!(alice.static_id, static_id(match_id));
    alice.in_match = None;
    run_for(&mut [&mut alice, &mut bob], Duration::from_millis(1_000));

    // She presses Play: her match is still on, and she is sent to it, by
    // the same id, on the same team.
    alice.take();
    alice.ask(request::play(1));
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "connect_to_match again",
        |games| sent_to(&*games[0]).is_some(),
    );
    assert_eq!(sent_to(&alice), Some((match_id, team)));
    assert_eq!(stack.registry.match_count(), 1);

    // Her game acts on it (`connect_to_match` checks that), takes the
    // static info again and is started from a snapshot of the match.
    alice.connect_to_match(stack.match_addr);
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the static info again",
        |games| games[0].of_match().model.options.is_some(),
    );
    assert_eq!(alice.of_match().model.local_index, Some(place));
    alice.of_match_mut().ready();
    until(
        &mut [&mut alice, &mut bob],
        Duration::from_secs(20),
        "back in the match",
        |games| games[0].of_match().model.is_synced(),
    );
    run_for(&mut [&mut alice, &mut bob], Duration::from_millis(3_000));
    for game in [&alice, &bob] {
        assert!(!game.of_match().closed, "{}", game.name);
        game.assert_clean();
    }
    assert_eq!(bob.of_match().model.syncs, 1, "Bob was resynced");
    assert!(stack.registry.is_open(match_id));
}

/// Seen live on 2026-10-12: a player left a match of two by the menu,
/// pressed Play and was sent back to the match it had just left. The game
/// says that it leaves (0xE0 on the match link, then `31 .. 01` to the
/// lobby); Play then leaves it in its room, for the room's next match.
#[test]
fn a_player_who_left_its_match_is_not_sent_back_to_it() {
    let dir = temp_dir("quit");
    let stack = start(&dir);
    let mut alice = Game::signed_in(&stack, "E2eQuitAlice", "alice-password-1");
    let mut bob = Game::signed_in(&stack, "E2eQuitBob", "bob-password-2");
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the sign-in pushes",
        |games| games.iter().all(|game| in_lobby(game)),
    );
    alice.take();
    bob.take();
    alice.ask(request::play(1));
    bob.ask(request::play(1));
    let sent_to = |game: &Game| {
        game.find(|message| match message {
            Decoded::ConnectToMatch { match_id, .. } => Some(*match_id),
            _ => None,
        })
    };
    until(
        &mut [&mut alice, &mut bob],
        ROOM_MATCH,
        "connect_to_match",
        |games| games.iter().all(|game| sent_to(game).is_some()),
    );
    let match_id = sent_to(&alice).unwrap();
    alice.connect_to_match(stack.match_addr);
    bob.connect_to_match(stack.match_addr);
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the static info",
        |games| {
            games
                .iter()
                .all(|game| game.of_match().model.options.is_some())
        },
    );
    alice.of_match_mut().ready();
    bob.of_match_mut().ready();
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "both in the match",
        |games| {
            games.iter().all(|game| {
                let client = game.of_match();
                client.model.is_synced() && client.entered().len() == 2
            })
        },
    );
    run_for(&mut [&mut alice, &mut bob], Duration::from_millis(1_000));

    // Alice leaves by the menu: 0xE0 to the match server, then the lobby
    // is told that she is back from her match. Bob plays on.
    alice.of_match_mut().leave();
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "Bob sees her go",
        |games| !games[1].of_match().left().is_empty(),
    );
    alice.in_match = None;
    alice.take();
    alice.ask(request::leave(u32::MAX, true));
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the lobby state",
        |games| queue_state(games[0]) == Some(ClientState::Lobby),
    );

    // She presses Play: she is in her room, which waits for its match to
    // end; she is not sent back. Her match goes on without her.
    alice.take();
    alice.ask(request::play(1));
    until(
        &mut [&mut alice, &mut bob],
        SOON,
        "the lobby state",
        |games| queue_state(games[0]) == Some(ClientState::Lobby),
    );
    run_for(&mut [&mut alice, &mut bob], Duration::from_millis(2_500));
    assert!(!alice.has(|message| matches!(message, Decoded::ConnectToMatch { .. })));
    assert!(stack.registry.is_open(match_id));
    assert_eq!(stack.registry.match_count(), 1);
    assert!(!bob.of_match().closed);
    for game in [&alice, &bob] {
        game.assert_clean();
    }
    stack.stop();
}

#[test]
fn a_wrong_password_is_refused_and_a_player_alone_waits_in_a_room() {
    let dir = temp_dir("solo");
    let stack = start(&dir);

    // The first sign-in of a name registers it with its password.
    let mut carol = Game::signed_in(&stack, "E2eCarol", "carol-password-1");
    // Another password for the name: the client's "invalid user name or
    // password" (0x0A) after the handshake, and no session.
    let wrong = sign_in(stack.login_addr, "E2eCarol", "not-her-password");
    assert_eq!(wrong.answer, 0x0B);
    assert_eq!(wrong.reply, [0x0A]);
    assert_eq!(stack.store.registered_accounts(), 1);
    // Another build's client is turned away before the handshake (0x22),
    // and a user name no account can have gets 0x0A at once.
    {
        let mut stream = TcpStream::connect(stack.login_addr).unwrap();
        stream.set_read_timeout(Some(SOON)).unwrap();
        let mut message = vec![0x01, 5];
        message.extend(b"Carol");
        let mut version = [0_u8; 12];
        version[..5].copy_from_slice(b"0.21d");
        message.extend(version);
        message.push(0);
        message.extend(u32::MAX.to_le_bytes());
        stream.write_all(&message).unwrap();
        let mut answer = Vec::new();
        let _ = stream.read_to_end(&mut answer);
        assert_eq!(answer, [0x22]);
    }
    assert_eq!(sign_in(stack.login_addr, "", "x").answer, 0x0A);
    // The right password still signs in (the wrong one locked nothing).
    let second = sign_in(stack.login_addr, "E2eCarol", "carol-password-1");
    let (second_session, _) = accepted(&second);
    assert_ne!(second_session, carol.session);

    // A lobby sign-in with a session nobody was given is disconnected.
    {
        let mut stranger = Game {
            revision: 0,
            name: "stranger",
            session: 0x0BAD_5E55,
            lobby_addr: stack.lobby_addr,
            lobby_socket: socket(),
            lobby: TestClient::new(),
            in_match: None,
            static_id: NO_STATIC_ID,
            hashed: 0,
            started: Instant::now(),
        };
        stranger.lobby.sign_in(stranger.session, true);
        until(&mut [&mut stranger], SOON, "the refusal", |games| {
            games[0].lobby.disconnected
        });
        assert!(stranger.lobby.decoded.is_empty());
    }

    // Carol, alone, presses Play: a room is made for her, her client is
    // told it is in the lobby (where the movie shows the room), and no
    // match is made.
    until(&mut [&mut carol], SOON, "the sign-in pushes", |games| {
        in_lobby(games[0])
    });
    carol.take();
    carol.ask(request::play(1));
    until(&mut [&mut carol], SOON, "the lobby state", |games| {
        queue_state(games[0]) == Some(ClientState::Lobby)
    });
    run_for(&mut [&mut carol], Duration::from_millis(3_500));
    assert!(!carol.has(|message| matches!(message, Decoded::ConnectToMatch { .. })));
    assert_eq!(stack.registry.match_count(), 0);
    assert_eq!(stack.store.match_count(), 0);
    carol.assert_clean();
    stack.stop();
}

// ------------------------------------------------------------ the test bot

/// The test bot as the `srp-bot` binary runs it (`srp::bot::run`), on a
/// thread, with its printed lines kept.
struct BotRun {
    lines: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl BotRun {
    fn start(stack: &Stack, name: &str, once: bool) -> Self {
        Self::start_with(stack, name, once, false)
    }

    /// With `drive`: the bot sends the inputs of its script (`srp::drive`).
    fn start_with(stack: &Stack, name: &str, once: bool, drive: bool) -> Self {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let config = srp::bot::Config {
            server: stack.login_addr.to_string(),
            name: name.to_owned(),
            password: "the-bots-password".to_owned(),
            once,
            kit: None,
            drive,
            revision: survarium::v026g::match_msgs::late_join::PATCH_REVISION,
        };
        let print: srp::bot::Print = {
            let lines = lines.clone();
            Arc::new(move |line: &str| {
                if std::env::var_os("SRP_TEST_LOG").is_some() {
                    eprintln!("[BOT] {line}");
                }
                lines.lock().unwrap().push(line.to_owned());
            })
        };
        let thread = {
            let stop = stop.clone();
            std::thread::spawn(move || srp::bot::run(vec![config], print, &stop))
        };
        Self {
            lines,
            stop,
            thread: Some(thread),
        }
    }

    /// Whether a line with `text` was printed, and where.
    fn said(&self, text: &str) -> Option<usize> {
        self.lines
            .lock()
            .unwrap()
            .iter()
            .position(|line| line.contains(text))
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    /// Whether the bot's loop ended by itself.
    fn finished(&self) -> bool {
        self.thread
            .as_ref()
            .is_none_or(|thread| thread.is_finished())
    }
}

impl Drop for BotRun {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// One real-like client (the scripted one that uploads its world and
/// reports its hashes) and the bot, from two sign-ins to both in the
/// match: the client, the match id, and the places and teams of the two
/// (the client's first).
fn solo_and_bot_into_a_match(stack: &Stack, bot: &BotRun) -> (Game, u32, [u8; 2], [u8; 2]) {
    let mut solo = Game::signed_in(stack, "E2eSolo", "solo-password-1");
    until(&mut [&mut solo], SOON, "the sign-in pushes", |games| {
        in_lobby(games[0])
    });
    // The bot registers itself, signs in at the lobby, presses Play and
    // waits in the room that makes for it: a match needs two.
    until(&mut [&mut solo], SOON, "the bot in its room", |_| {
        bot.said(": 1 in it").is_some()
    });
    assert_eq!(stack.registry.match_count(), 0);
    assert_eq!(stack.store.registered_accounts(), 2);

    // The player presses Play: into the bot's room, which has its two.
    solo.take();
    solo.ask(request::play(1));
    until(&mut [&mut solo], ROOM_MATCH, "connect_to_match", |games| {
        games[0].has(|message| matches!(message, Decoded::ConnectToMatch { .. }))
    });
    let (match_id, team) = solo
        .find(|message| match message {
            Decoded::ConnectToMatch { match_id, team, .. } => Some((*match_id, *team)),
            _ => None,
        })
        .unwrap();
    assert!(stack.registry.is_open(match_id));

    // The player's game loads, uploads its initial world and is started
    // from it; the bot, there long before, is started once the player is
    // in, and enters: the roster of two is complete for the player.
    solo.connect_to_match(stack.match_addr);
    until(&mut [&mut solo], SOON, "the static info", |games| {
        games[0].of_match().model.options.is_some()
    });
    solo.of_match_mut().ready();
    until(&mut [&mut solo], SOON, "both in the match", |games| {
        let client = games[0].of_match();
        let mut entered = client.entered();
        entered.sort_unstable();
        client.model.is_synced() && entered == [0, 1]
    });
    let client = solo.of_match();
    let options = client.model.options.as_ref().unwrap();
    assert_eq!(options.players.len(), 2, "the roster is the two of them");
    let place = client.model.local_index.unwrap();
    let bot_place = 1 - place;
    assert_eq!(options.players[usize::from(place)].name, "E2eSolo");
    let bot_profile = &options.players[usize::from(bot_place)];
    assert_eq!(bot_profile.name, "bot-1");
    // The bot's player as the client's world needs it: on the other team,
    // with the starter's kits and items (the client spawns it with its
    // first kit, no input needed).
    assert_eq!(bot_profile.team, 1 - team);
    assert_eq!(bot_profile.kits.len(), 3);
    assert!(!bot_profile.items.is_empty());
    until(&mut [&mut solo], SOON, "the bot's own entry", |_| {
        bot.said("is in the match").is_some()
    });
    (solo, match_id, [place, bot_place], [team, 1 - team])
}

/// The bot's team has no tickets and nobody alive, in the one world there
/// is: the player's.
fn end_by_the_modes_rule(solo: &mut Game, places: [u8; 2], teams: [u8; 2]) {
    let at = solo.of_match().clock() / 50 * 50 + 150;
    let mut rule = solo.of_match().sim.rule_at(at);
    rule.state = 3;
    rule.start = Some(INITIAL_MS);
    let mut tickets_left = [64, 64];
    tickets_left[usize::from(teams[1])] = 0;
    let sim = &mut solo.of_match_mut().sim;
    sim.force_rule(rule);
    sim.tdm_from = Some((
        at,
        Tdm {
            tickets: tickets_left,
            alive_players_mask: 1 << places[0],
            active_players_mask: 0b11,
        },
    ));
}

#[test]
fn one_player_and_the_test_bot_play_a_match_to_the_results() {
    let dir = temp_dir("bot");
    // `SRP_BOT_ACCOUNTS=bot-1`.
    let stack = start_with_bots(&dir, &["bot-1"]);
    let bot = BotRun::start(&stack, "bot-1", true);
    let (mut solo, match_id, places, teams) = solo_and_bot_into_a_match(&stack, &bot);
    let bot_id = account_id(&stack, "bot-1");
    assert!(stack.accounts.is_bot(bot_id));
    assert!(!stack.accounts.is_bot(account_id(&stack, "E2eSolo")));

    // The match runs: the player's world is asked for and uploaded, its
    // hashes go up, and nobody is resynced; the bot gives neither.
    run_for(&mut [&mut solo], Duration::from_millis(2_600));
    solo.of_match_mut().input(100, 0x8000_0021);
    until(&mut [&mut solo], SOON, "an upload request", |games| {
        games[0].of_match().upload_requests >= 1 && games[0].of_match().uploads >= 2
    });
    assert_eq!(solo.of_match().model.syncs, 1, "the player was resynced");
    assert!(solo.of_match().left().is_empty());
    assert!(stack.results.find(match_id).is_none());
    let before = (held(&stack, "E2eSolo"), held(&stack, "bot-1"));
    solo.assert_clean();

    // The end by the mode's rule, read from the player's world on its
    // word alone (one voucher): both are told.
    end_by_the_modes_rule(&mut solo, places, teams);
    until(&mut [&mut solo], MATCH_END, "the match's end", |games| {
        games[0].of_match().told_to_leave
    });
    until(&mut [&mut solo], SOON, "the results screen", |games| {
        games[0].has(|message| matches!(message, Decoded::MatchStats { .. }))
    });
    let report = stack.results.find(match_id).expect("the match's report");
    assert_eq!(report.players.len(), 2);
    let stats = solo
        .find(|message| match message {
            Decoded::MatchStats { stats, .. } => Some(stats.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!((stats.match_id, stats.winner_team), (match_id, teams[0]));
    assert_eq!(stats.rows.len(), 2);
    assert!(stats.rows.iter().any(|row| row.name == "bot-1"));
    // The player is paid as the winner of any match.
    let paid = held(&stack, "E2eSolo");
    assert!(paid.0 > before.0.0 && paid.2 > before.0.2);
    assert_eq!(told(&solo), paid);
    assert!(report.players[usize::from(places[0])].reward.won);
    // The bot's account is settled as any account (it stayed to the end
    // and lost), without statistics of its own breaking anything; it gets
    // no daily bonus, nobody's rating moved, and it is not in the ranking.
    let bot_report = &report.players[usize::from(places[1])];
    assert!(!bot_report.reward.won && !bot_report.reward.daily_bonus);
    assert!(held(&stack, "bot-1").2 > before.1.2, "the bot's experience");
    for player in &report.players {
        assert_eq!(player.reward.rating_after, player.reward.rating_before);
    }
    {
        let account = stack.accounts.get(bot_id).unwrap();
        assert_eq!(account.lock().unwrap().extras.bonus, Default::default());
    }
    let ranking = stack.accounts.ranking();
    assert_eq!(ranking.len(), 1);
    assert_eq!(ranking[0].1, "E2eSolo");
    solo.assert_clean();

    // The bot took every step, in order, and with `once` it stops after
    // its results.
    until(&mut [&mut solo], SOON, "the bot's end", |_| bot.finished());
    let steps = [
        "signed in at the login server",
        "in the lobby",
        "pressed Play",
        "in room",
        "a match is found",
        "goes to match",
        "took the static info",
        "took the world",
        "has the time",
        "is in the match",
        "the match is over",
        "the results of match",
        "one match done",
    ];
    let lines = bot.lines();
    let mut last = 0;
    for step in steps {
        let at = bot
            .said(step)
            .unwrap_or_else(|| panic!("the bot never said `{step}`: {lines:#?}"));
        assert!(at >= last, "`{step}` out of order: {lines:#?}");
        last = at;
    }
    assert!(
        !lines.iter().any(|line| line.contains("WARNING")),
        "{lines:#?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains("the-bots-password")),
        "the password was printed"
    );
    until(&mut [&mut solo], SOON, "the match closed", |_| {
        !stack.registry.is_open(match_id) && stack.store.match_count() == 0
    });
    drop(bot);
    stack.stop();
}

#[test]
fn a_driving_bot_s_inputs_reach_the_player_s_game_in_time() {
    use srp::drive::key;
    let dir = temp_dir("bot-drive");
    let stack = start_with_bots(&dir, &["bot-1"]);
    let bot = BotRun::start_with(&stack, "bot-1", true, true);
    let (mut solo, _match_id, places, _teams) = solo_and_bot_into_a_match(&stack, &bot);
    let bot_place = places[1];
    assert!(bot.said("drives: the script starts").is_some());

    // 2 s after its entry it walks forward; 3 s later it turns a quarter
    // and lets go; 1 s later it walks again.
    until(
        &mut [&mut solo],
        Duration::from_secs(15),
        "the bot's turn",
        |games| {
            games[0]
                .of_match()
                .relays_of(bot_place)
                .iter()
                .any(|input| input.rotation != [0; 4])
        },
    );
    run_for(&mut [&mut solo], Duration::from_millis(1_300));
    let relays = solo.of_match().relays_of(bot_place);
    let actions: Vec<u32> = relays.iter().map(|input| input.actions).collect();
    assert_eq!(actions[..3], [key::FORWARD, 0, key::FORWARD], "{relays:?}");
    // The turn: +pi/2 as a half float, yaw first, with the keys let go.
    assert_eq!(relays[1].rotation, [0x48, 0x3E, 0x00, 0x00]);
    assert_eq!(relays[0].rotation, [0; 4]);
    // 3 s apart, on the 50 ms grid of ticks.
    let ticks = |input: &survarium::v026g::match_msgs::PlayerInput| input.time.tick;
    assert_eq!(ticks(&relays[1]).wrapping_sub(ticks(&relays[0])), 60);
    assert!(relays.iter().all(|input| input.time.ms.is_none()));

    // Nothing of it was dropped or moved, and the player's game took
    // every relay as a client would (in time, in order).
    let lines = bot.lines();
    assert!(
        lines.iter().any(|line| line.contains("input for")),
        "{lines:#?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains("WARNING")),
        "{lines:#?}"
    );
    solo.assert_clean();
    drop(bot);
    stack.stop();
}

#[test]
fn without_the_setting_the_bot_is_a_player_as_any_other() {
    let dir = temp_dir("nobot");
    // No `SRP_BOT_ACCOUNTS`: what the servers do by default.
    let stack = start(&dir);
    let bot = BotRun::start(&stack, "bot-1", false);
    let (mut solo, match_id, places, teams) = solo_and_bot_into_a_match(&stack, &bot);
    let bot_id = account_id(&stack, "bot-1");
    // Nothing knows a bot: its account is in the ranking with the others,
    // and its place in the match is a witness's.
    assert!(stack.accounts.bot_names().is_empty());
    assert!(!stack.accounts.is_bot(bot_id));
    assert_eq!(stack.accounts.ranking().len(), 2);

    // So the rule for two players holds: a world needs two of them, the
    // bot says nothing, and what the one player claims alone is not
    // believed. The match does not end by it.
    run_for(&mut [&mut solo], Duration::from_millis(2_600));
    end_by_the_modes_rule(&mut solo, places, teams);
    run_for(&mut [&mut solo], Duration::from_secs(14));
    assert!(!solo.of_match().told_to_leave);
    assert!(stack.results.find(match_id).is_none());
    assert!(stack.registry.is_open(match_id));
    assert_eq!(solo.of_match().model.syncs, 1);
    assert!(bot.said("the match is over").is_none());
    solo.assert_clean();
    drop(bot);
    stack.stop();
}

#[test]
fn a_new_login_joins_a_running_match_through_the_lobby_and_receives_results() {
    let dir = temp_dir("late-join");
    let stack = start(&dir);
    let mut alice = Game::signed_in(&stack, "LateAlice", "test-password-1");
    let mut bob = Game::signed_in(&stack, "LateBob", "test-password-2");
    let mut charlie = Game::signed_in(&stack, "LateCharlie", "test-password-3");
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        SOON,
        "login pushes",
        |games| games.iter().all(|g| in_lobby(g)),
    );
    let ticket = |game: &Game| {
        game.find(|message| match message {
            Decoded::ConnectToMatch { match_id, .. } => Some(*match_id),
            _ => None,
        })
    };
    alice.ask(request::play(1));
    bob.ask(request::play(1));
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        ROOM_MATCH,
        "initial match",
        |games| games[..2].iter().all(|g| ticket(g).is_some()),
    );
    let match_id = ticket(&alice).unwrap();
    assert_eq!(ticket(&bob), Some(match_id));
    alice.connect_to_match(stack.match_addr);
    bob.connect_to_match(stack.match_addr);
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        SOON,
        "initial profiles",
        |games| {
            games[..2]
                .iter()
                .all(|g| g.of_match().model.options.is_some())
        },
    );
    alice.of_match_mut().ready();
    bob.of_match_mut().ready();
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        SOON,
        "initial entry",
        |games| games[..2].iter().all(|g| g.of_match().entered().len() == 2),
    );
    run_for(
        &mut [&mut alice, &mut bob, &mut charlie],
        Duration::from_secs(22),
    );
    let syncs = [alice.of_match().model.syncs, bob.of_match().model.syncs];
    charlie.ask(request::play(1));
    // Into the room first: the match 15 s later (`RoomRules::late_join_wait`).
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        SOON + Duration::from_secs(15),
        "newcomer ticket",
        |games| ticket(games[2]).is_some(),
    );
    assert_eq!(ticket(&charlie), Some(match_id));
    assert_eq!(stack.registry.match_count(), 1);
    charlie.connect_to_match(stack.match_addr);
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        SOON,
        "newcomer profiles",
        |games| games[2].of_match().model.options.is_some(),
    );
    assert_eq!(charlie.of_match().model.local_index, Some(2));
    charlie.of_match_mut().ready();
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        SOON,
        "newcomer caught up and entered",
        |games| games.iter().all(|g| g.of_match().entered().contains(&2)),
    );
    assert_eq!(
        [alice.of_match().model.syncs, bob.of_match().model.syncs],
        syncs
    );
    let input = charlie.of_match_mut().input(100, 0x21);
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        SOON,
        "newcomer input",
        |games| {
            games[..2]
                .iter()
                .all(|g| g.of_match().relays_of(2).contains(&input))
        },
    );
    let at = alice.of_match().clock() / 50 * 50 + 100;
    let options = charlie.of_match().model.options.as_ref().unwrap();
    let alive_players_mask = options
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.team == 1)
        .fold(0, |mask, (place, _)| mask | (1 << place));
    for game in [&mut alice, &mut bob, &mut charlie] {
        game.of_match_mut().sim.tdm_from = Some((
            at,
            Tdm {
                tickets: [0, 100],
                alive_players_mask,
                active_players_mask: 0b111,
            },
        ));
    }
    until(
        &mut [&mut alice, &mut bob, &mut charlie],
        MATCH_END,
        "match results",
        |games| {
            games
                .iter()
                .all(|g| g.has(|m| matches!(m, Decoded::MatchStats { .. })))
        },
    );
    let report = stack.results.find(match_id).unwrap();
    assert_eq!(report.players.len(), 3);
    for game in [&alice, &bob, &charlie] {
        game.assert_clean();
    }
    stack.stop();
}

/// The rows of the last `91` whose first row is at `place`: an answer to
/// one of srp's own requests (`lobby::RESERVED_START` on), never a
/// ranking page.
fn reserved_rows(game: &Game, place: u32) -> Option<Vec<(String, u32, u16, i16)>> {
    game.find(|message| match message {
        Decoded::EloList(rows) if rows.first().is_some_and(|row| row.1 == place) => {
            Some(rows.clone())
        }
        _ => None,
    })
}

#[test]
fn a_player_against_the_servers_bots_plays_a_match_to_the_results() {
    use survarium::v026g::lobby::{PVB_STATUS_START, pvb_choice_start};
    let dir = temp_dir("pvb");
    let stack = start_with(&dir, &[], |options| {
        options.lobby.pvb.enabled = true;
    });
    let mut solo = Game::signed_in(&stack, "E2ePvb", "pvb-password-1");
    until(&mut [&mut solo], SOON, "the sign-in pushes", |games| {
        in_lobby(games[0])
    });
    solo.take();
    solo.ask(request::ranking(PVB_STATUS_START, 32));
    until(&mut [&mut solo], SOON, "the status", |games| {
        reserved_rows(games[0], PVB_STATUS_START).is_some()
    });
    let rows = reserved_rows(&solo, PVB_STATUS_START).unwrap();
    // The starter kits' highest level (`tutorial`; 4, the third's MP-153):
    // every level plays (15, the default).
    let gear = {
        let account = stack.accounts.get(account_id(&stack, "E2ePvb")).unwrap();
        let account = account.locked();
        let level = |kit| session::v026g::matchmaking::kit_level(&account, kit);
        account.kits.iter().map(level).max().unwrap()
    };
    assert_eq!(gear, 4);
    assert_eq!(rows[0].0, format!("B1;0;{gear};15;6.8.10;8;61;0;5"));
    assert_eq!(rows.len(), 17);
    assert!(rows.iter().any(|row| row.0 == "117;1;4;5"), "{rows:?}");

    let start = pvb_choice_start(6, MAP);
    solo.ask(request::ranking(start, 1));
    until(&mut [&mut solo], SOON, "the choice", |games| {
        reserved_rows(games[0], start).is_some()
    });
    assert_eq!(reserved_rows(&solo, start).unwrap()[0].0, "B2;ok;6;117");
    solo.ask(request::play(1));
    until(&mut [&mut solo], SOON, "connect_to_match", |games| {
        games[0].has(|message| matches!(message, Decoded::ConnectToMatch { .. }))
    });
    let (match_id, team) = solo
        .find(|message| match message {
            Decoded::ConnectToMatch { match_id, team, .. } => Some((*match_id, *team)),
            _ => None,
        })
        .unwrap();
    assert_eq!(team, 0);
    assert_eq!(stack.registry.match_count(), 1);

    solo.connect_to_match_of(stack.match_addr, 12);
    until(&mut [&mut solo], SOON, "the static info", |games| {
        games[0].of_match().model.options.is_some()
    });
    let options = solo.of_match().model.options.clone().unwrap();
    assert_eq!(options.players.len(), 12);
    assert_eq!(options.players[0].name, "E2ePvb");
    let on = |team: u8| {
        options
            .players
            .iter()
            .filter(|player| player.team == team)
            .count()
    };
    assert_eq!((on(0), on(1)), (6, 6));
    assert!(options.players[1..].iter().all(|bot| bot.clan_tag == "BOT"));
    solo.of_match_mut().ready();
    until(&mut [&mut solo], SOON, "everyone in the match", |games| {
        let client = games[0].of_match();
        client.model.is_synced() && client.entered().len() == 12
    });
    let place = solo.of_match().model.local_index.unwrap();
    assert_eq!(place, 0);
    run_for(&mut [&mut solo], Duration::from_millis(2_600));
    until(&mut [&mut solo], SOON, "an upload", |games| {
        games[0].of_match().uploads >= 2
    });
    let before = held(&stack, "E2ePvb");
    solo.assert_clean();

    // Team 1, the bots', has no tickets left in the player's world.
    let at = solo.of_match().clock() / 50 * 50 + 150;
    let mut rule = solo.of_match().sim.rule_at(at);
    rule.state = 3;
    rule.start = Some(INITIAL_MS);
    let sim = &mut solo.of_match_mut().sim;
    sim.force_rule(rule);
    sim.tdm_from = Some((
        at,
        Tdm {
            tickets: [64, 0],
            alive_players_mask: 1,
            active_players_mask: (1 << 12) - 1,
        },
    ));
    until(&mut [&mut solo], MATCH_END, "the match's end", |games| {
        games[0].of_match().told_to_leave
    });
    until(&mut [&mut solo], SOON, "the results screen", |games| {
        games[0].has(|message| matches!(message, Decoded::MatchStats { .. }))
    });
    let report = stack.results.find(match_id).expect("the match's report");
    assert_eq!(report.players.len(), 12);
    let player = &report.players[0];
    assert!(player.reward.won && !player.reward.daily_bonus);
    assert_eq!(player.reward.quest_money, 0);
    assert_eq!(player.reward.rating_after, player.reward.rating_before);
    let paid = held(&stack, "E2ePvb");
    assert!(paid.0 > before.0 && paid.2 > before.2);
    assert_eq!(told(&solo), paid);
    solo.assert_clean();
    until(&mut [&mut solo], SOON, "the match closed", |_| {
        !stack.registry.is_open(match_id) && stack.store.match_count() == 0
    });
    stack.stop();
}
