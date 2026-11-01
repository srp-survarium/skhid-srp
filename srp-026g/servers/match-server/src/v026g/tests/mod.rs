//! Tests of the 0.26g match server: the real `Server` and `Game`, driven
//! by scripted clients ([`client::Client`]).
//!
//! - [`Bench`]: in memory, on a clock the test moves (a 15-minute match
//!   takes a moment);
//! - [`Net`]: over localhost UDP sockets, with the server's own loop in a
//!   thread.

use super::test_client as client;

mod afk;
mod bots;
mod driven;
mod game;
mod late_join;
mod late_join_edge_cases;
mod resync_gate;
mod sockets;
mod start;
mod units;

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::SessionStore;
use session::persist::MemoryRepository;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::match_msgs::static_info::{MatchItem, MatchKit, NO_PROTECTION};

use super::game::Game;
use super::server::{Server, serve};
use super::{MatchPlayer, MatchProfile, MatchRegistry, MatchResult, MatchSetup, Settings};
use client::Client;

/// Modes 2 (batteries) and 4 (Team Deathmatch) on level_01.
pub const MAP_BATTERIES: u8 = 61;
pub const MAP_TDM: u8 = 117;

pub fn session_of(place: usize) -> u32 {
    0x1000 + place as u32
}

fn profile(name: &str, team: u8) -> MatchProfile {
    let mut kit = MatchKit::empty(1, "");
    kit.slots[7] = Some((0, 1));
    MatchProfile {
        team,
        name: name.into(),
        clan_tag: String::new(),
        protection: NO_PROTECTION,
        kits: vec![kit],
        items: vec![MatchItem {
            instance_id: 1,
            dict_id: Dictionary::shared()
                .item_named("tt_33")
                .expect("the pistol of the 0.26g dictionary")
                .dict_id,
            condition_or_stack: 1_000_000,
            ..MatchItem::default()
        }],
        squad: 0,
    }
}

pub fn setup(players: usize, map_id: u8) -> MatchSetup {
    MatchSetup {
        map_id,
        match_level: 3,
        players: (0..players)
            .map(|place| MatchPlayer {
                bot: false,
                driven: false,
                session_id: session_of(place),
                account_id: 100 + place as u32,
                profile: profile(&format!("player{place}"), (place % 2) as u8),
            })
            .collect(),
        score_weights: None,
        vs_bots: false,
        private: false,
        room_minimum: None,
        blackout: false,
    }
}

fn store() -> Arc<SessionStore> {
    Arc::new(
        SessionStore::with_repository(
            Duration::from_secs(60),
            Box::new(MemoryRepository::default()),
        )
        .expect("a store in memory"),
    )
}

/// The server and its clients in memory, on a clock the test moves.
pub struct Bench {
    pub server: Server,
    pub registry: Arc<MatchRegistry>,
    pub results: Receiver<MatchResult>,
    pub clients: Vec<Client>,
    pub match_id: u32,
    /// Each client's address; a client that comes back has a new one.
    addrs: Vec<SocketAddr>,
    start: Instant,
    pub ms: u64,
    /// Each place's one-way delay, both ways, in ms (none if not given).
    pub one_way: Vec<u64>,
    /// Datagrams on a delayed way: when they arrive, the place, whether to
    /// the server, the bytes.
    in_air: Vec<(u64, usize, bool, Vec<u8>)>,
    /// The wall-clock time spent in the server's `poll`.
    pub server_busy: Duration,
}

impl Bench {
    /// A match of `players` made, nobody connected yet. All clients are
    /// patched unless a test says otherwise.
    pub fn new(players: usize, map_id: u8, settings: Settings) -> Self {
        Self::with_bots(players, map_id, settings, &[])
    }

    /// As [`Bench::new`], with the places of `bots` marked as test bots'
    /// (`MatchPlayer::bot`) and given clients that simulate nothing
    /// (`Client::idle`, what `srp-bot` runs).
    pub fn with_bots(players: usize, map_id: u8, settings: Settings, bots: &[usize]) -> Self {
        Self::with_dialect(players, map_id, settings, bots, &[], false)
    }

    /// With the server's own bots ([`MatchPlayer::driven`]) at `driven`.
    pub fn with_driven(players: usize, map_id: u8, settings: Settings, driven: &[usize]) -> Self {
        Self::with_dialect(players, map_id, settings, &[], driven, false)
    }

    pub fn growing(players: usize, map_id: u8, settings: Settings) -> Self {
        Self::with_dialect(players, map_id, settings, &[], &[], true)
    }

    /// A match of growing rosters with the server's own bots at `driven`.
    pub fn growing_with_driven(
        players: usize,
        map_id: u8,
        settings: Settings,
        driven: &[usize],
    ) -> Self {
        Self::with_dialect(players, map_id, settings, &[], driven, true)
    }

    /// A room's match (growing rosters) of places on `teams`, the room
    /// needing `minimum` players (`MatchSetup::room_minimum`).
    pub fn room(teams: &[u8], map_id: u8, settings: Settings, minimum: u8) -> Self {
        let mut setup = setup(teams.len(), map_id);
        for (player, &team) in setup.players.iter_mut().zip(teams) {
            player.profile.team = team;
        }
        setup.room_minimum = Some(minimum);
        Self::with_setup(setup, settings, &[], &[], true)
    }

    /// A match of one player against the server's own bots at `driven`
    /// ("Player vs Bots": `MatchSetup::vs_bots`), of growing rosters.
    pub fn vs_bots(players: usize, map_id: u8, settings: Settings, driven: &[usize]) -> Self {
        let mut setup = setup(players, map_id);
        setup.vs_bots = true;
        Self::with_setup(setup, settings, &[], driven, true)
    }

    fn with_dialect(
        players: usize,
        map_id: u8,
        settings: Settings,
        bots: &[usize],
        driven: &[usize],
        growing: bool,
    ) -> Self {
        Self::with_setup(setup(players, map_id), settings, bots, driven, growing)
    }

    fn with_setup(
        mut setup: MatchSetup,
        settings: Settings,
        bots: &[usize],
        driven: &[usize],
        growing: bool,
    ) -> Self {
        log_to_stderr();
        let (players, map_id) = (setup.players.len(), setup.map_id);
        let start = Instant::now();
        let registry = Arc::new(MatchRegistry::new("127.0.0.1", 1236));
        for place in bots {
            setup.players[*place].bot = true;
        }
        for place in driven {
            let player = &mut setup.players[*place];
            (player.driven, player.session_id, player.account_id) = (true, 0, 0);
        }
        let ticket = registry
            .create_with_dialect(setup, start, growing)
            .expect("a match the clients can hold");
        let (sink, results) = channel();
        let server = Server::new(
            1236,
            registry.clone(),
            store(),
            Box::new(sink),
            settings,
            start,
        );
        Self {
            server,
            registry,
            results,
            clients: (0..players)
                .map(|place| {
                    if growing {
                        Client::growing(session_of(place), players, map_id)
                    } else if bots.contains(&place) {
                        Client::idle(session_of(place))
                    } else {
                        Client::new(session_of(place), true, players, map_id)
                    }
                })
                .collect(),
            match_id: ticket.match_id,
            addrs: (0..players)
                .map(|place| SocketAddr::from(([127, 0, 0, 1], 40_000 + place as u16)))
                .collect(),
            start,
            ms: 0,
            one_way: Vec::new(),
            in_air: Vec::new(),
            server_busy: Duration::ZERO,
        }
    }

    pub fn now(&self) -> Instant {
        self.start + Duration::from_millis(self.ms)
    }

    /// The player's game was started anew: a fresh client for its place,
    /// from another port.
    pub fn restart(&mut self, place: usize) {
        let old = &self.clients[place];
        let (players, map_id) = (old.sim.players, old.sim.map_id);
        self.clients[place] = if old.model.growing_roster {
            Client::growing(old.session, players, map_id)
        } else {
            Client::new(old.session, true, players, map_id)
        };
        let port = self
            .addrs
            .iter()
            .map(SocketAddr::port)
            .max()
            .unwrap_or(40_000)
            + 100;
        self.addrs[place] = SocketAddr::from(([127, 0, 0, 1], port));
    }

    /// `ms` pass: the clients send, the server answers. A place with a
    /// one-way delay (`one_way`) has its datagrams arrive that much later
    /// either way.
    pub fn step(&mut self, ms: u64) {
        self.ms += ms;
        let now = self.now();
        self.land(true, now);
        for (place, client) in self.clients.iter_mut().enumerate() {
            let delay = self.one_way.get(place).copied().unwrap_or(0);
            for datagram in client.poll(self.ms) {
                if delay == 0 {
                    self.server.receive(self.addrs[place], &datagram, now);
                } else {
                    self.in_air.push((self.ms + delay, place, true, datagram));
                }
            }
        }
        let busy = Instant::now();
        let out = self.server.poll(now);
        self.server_busy += busy.elapsed();
        for (addr, datagram) in out {
            // A client that is gone hears nothing.
            if let Some(place) = self.addrs.iter().position(|each| *each == addr) {
                let delay = self.one_way.get(place).copied().unwrap_or(0);
                if delay == 0 {
                    self.clients[place].receive(&datagram);
                } else {
                    self.in_air.push((self.ms + delay, place, false, datagram));
                }
            }
        }
        self.land(false, now);
    }

    /// The delayed datagrams due by now, to the server or to the clients.
    fn land(&mut self, to_server: bool, now: Instant) {
        let ms = self.ms;
        let (due, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.in_air)
            .into_iter()
            .partition(|(at, _, up, _)| *at <= ms && *up == to_server);
        self.in_air = rest;
        for (_, place, _, datagram) in due {
            if to_server {
                self.server.receive(self.addrs[place], &datagram, now);
            } else {
                self.clients[place].receive(&datagram);
            }
        }
    }

    /// Time passes in steps of 5 ms.
    pub fn run(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(5) {
            self.step(5);
        }
    }

    /// A long stretch, in steps of 25 ms.
    pub fn run_long(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(25) {
            self.step(25);
        }
    }

    /// Time passes until `done`, at most `limit_ms`: whether it came.
    pub fn until(&mut self, limit_ms: u64, done: impl Fn(&Bench) -> bool) -> bool {
        let end = self.ms + limit_ms;
        while self.ms < end {
            if done(self) {
                return true;
            }
            self.step(5);
        }
        done(self)
    }

    pub fn game(&self) -> &Game {
        self.server
            .games
            .get(&self.match_id)
            .expect("the match is running")
    }

    pub fn stages(&self) -> String {
        self.server
            .games
            .get(&self.match_id)
            .map_or(String::new(), Game::stages)
    }

    pub fn clock(&self) -> u32 {
        self.game().match_clock(self.now())
    }

    /// The client connects, loads and is in the simulation.
    pub fn join(&mut self, place: usize) {
        self.clients[place].connect();
        assert!(
            self.until(1_000, |bench| bench.clients[place].model.options.is_some()),
            "player {place} got no static info"
        );
        self.clients[place].ready();
        assert!(
            self.until(30_000, |bench| {
                bench.stages().as_bytes().get(place) == Some(&b'L')
                    && bench.clients[place].model.is_synced()
            }),
            "player {place} did not get in: {}",
            self.stages()
        );
    }

    /// Every client joins, one after another.
    pub fn join_all(&mut self) {
        for place in 0..self.clients.len() {
            self.join(place);
        }
    }

    /// No client was sent anything it would drop or choke on.
    pub fn assert_no_faults(&self) {
        for (place, client) in self.clients.iter().enumerate() {
            assert!(
                client.faults.is_empty(),
                "player {place}: {:?}",
                client.faults
            );
        }
    }
}

/// The server's own loop on a localhost socket, and clients with sockets.
pub struct Net {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<std::io::Result<()>>>,
    server: SocketAddr,
    pub registry: Arc<MatchRegistry>,
    pub results: Receiver<MatchResult>,
    pub clients: Vec<(Client, UdpSocket)>,
    pub match_id: u32,
    start: Instant,
}

/// The socket tests run one at a time: each wants its real time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

impl Net {
    pub fn new(players: usize, map_id: u8, settings: Settings) -> Self {
        log_to_stderr();
        let socket = UdpSocket::bind("127.0.0.1:0").expect("a free port");
        let server = socket.local_addr().expect("its address");
        let registry = Arc::new(MatchRegistry::new("127.0.0.1", server.port()));
        let ticket = registry
            .create(setup(players, map_id))
            .expect("a match the clients can hold");
        assert_eq!(ticket.address(), server.to_string());
        let (sink, results) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (registry, stop) = (registry.clone(), stop.clone());
            std::thread::spawn(move || {
                serve(
                    &socket,
                    server.port(),
                    registry,
                    store(),
                    Box::new(sink),
                    settings,
                    &stop,
                )
            })
        };
        let clients = (0..players)
            .map(|place| {
                let socket = UdpSocket::bind("127.0.0.1:0").expect("a free port");
                socket.set_nonblocking(true).expect("a socket");
                (
                    Client::new(session_of(place), true, players, map_id),
                    socket,
                )
            })
            .collect();
        Self {
            stop,
            thread: Some(thread),
            server,
            registry,
            results,
            clients,
            match_id: ticket.match_id,
            start: Instant::now(),
        }
    }

    pub fn client(&mut self, place: usize) -> &mut Client {
        &mut self.clients[place].0
    }

    /// One pass: every client sends what it has and takes what came.
    pub fn step(&mut self) {
        let ms = self.start.elapsed().as_millis() as u64;
        let mut buffer = [0; 2048];
        for (client, socket) in &mut self.clients {
            for datagram in client.poll(ms) {
                let _ = socket.send_to(&datagram, self.server);
            }
            while let Ok((len, from)) = socket.recv_from(&mut buffer) {
                if from == self.server {
                    client.receive(&buffer[..len]);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    /// Passes until `done`, at most `limit`: whether it came. Never hangs.
    pub fn until(&mut self, limit: Duration, done: impl Fn(&Net) -> bool) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if done(self) {
                return true;
            }
            self.step();
        }
        done(self)
    }

    pub fn run(&mut self, time: Duration) {
        let deadline = Instant::now() + time;
        while Instant::now() < deadline {
            self.step();
        }
    }

    /// The client connects, loads and is in the simulation.
    pub fn join(&mut self, place: usize) {
        self.client(place).connect();
        assert!(
            self.until(Duration::from_secs(3), |net| {
                net.clients[place].0.model.options.is_some()
            }),
            "player {place} got no static info"
        );
        self.client(place).ready();
        assert!(
            self.until(Duration::from_secs(5), |net| {
                let client = &net.clients[place].0;
                client.model.is_synced() && client.entered().contains(&(place as u8))
            }),
            "player {place} did not get in"
        );
    }

    pub fn assert_no_faults(&self) {
        for (place, (client, _)) in self.clients.iter().enumerate() {
            assert!(
                client.faults.is_empty(),
                "player {place}: {:?}",
                client.faults
            );
        }
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Short waits for the socket tests, which run on the real clock.
pub fn quick() -> Settings {
    let mut settings = Settings::default();
    let timing = &mut settings.timing;
    timing.client_timeout = Duration::from_millis(1_500);
    timing.refresh = Duration::from_millis(400);
    timing.hash_settle_ms = 700;
    timing.divergence_confirm_ms = 200;
    timing.divergence_confirm_max = Duration::from_millis(500);
    timing.end_delay_ms = 300;
    timing.linger = Duration::from_millis(300);
    timing.final_wait = Duration::from_millis(500);
    settings
}

pub fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The server's log on stderr, when `SRP_TEST_LOG` is set: for finding out
/// why a test fails.
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

pub fn log_to_stderr() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if std::env::var_os("SRP_TEST_LOG").is_some() && log::set_logger(&StderrLog).is_ok() {
            log::set_max_level(log::LevelFilter::Debug);
        }
    });
}
