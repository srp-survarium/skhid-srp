//! The 0.26g match server's socket: one UDP port for every match. A
//! datagram goes to the match of the client that sent it; a new client is
//! placed by the session in its 0xD8, through the registry the lobby made
//! the match in. (0.21d: `crate::server`.)

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use session::SessionStore;
use session::v026g::replays::Replays;
use survarium::v026g::match_msgs::{ClientMessage, id, parse_client};
use udp_transport::{Config, Event, PacketLog, Table, Transport};

use super::Settings;
use super::game::Game;
use super::record::{Desk, Recorder};
use super::registry::{MatchEnd, MatchRegistry, MatchResult, PlayerResult, ResultsSink};

/// How long a receive waits before the matches are advanced anyway.
const RECEIVE_WAIT: Duration = Duration::from_millis(5);
const HOUSEKEEPING: Duration = Duration::from_secs(1);
/// How often the registry is told how each match stands
/// (`MatchRegistry::live_matches`).
const LIVE_EVERY: Duration = Duration::from_secs(1);
/// Larger than any client datagram (508 bytes).
const RECEIVE_BUFFER: usize = 2048;
/// How many stray addresses are remembered before the list starts over.
const STRAYS_KEPT: usize = 256;
/// An address that sent this many datagrams without connecting (0xD8), over
/// [`STRAY_PATIENCE`] or longer, is told to disconnect: a client of a match
/// that ended with a restart keeps sending, and what answers it (acks,
/// keep-alives) tells it the connection is fine.
const STRAY_LIMIT: u32 = 3;
/// How long an address is heard without its 0xD8 before it is told to
/// disconnect. A client that connects sends its 0xD8 in its first datagram
/// and again every 500 ms until it is acked, with no end (transport.md
/// section 4: D8, lifetime none, resend 500), and a keep-alive every 100 ms
/// in between (section 6). So one whose first datagram was lost on the way
/// has sent three datagrams with no 0xD8 after 300 ms; told to disconnect
/// then (0xFD: the client drops the connection at once, 0x3FDEED), it never
/// sent its second. Seen live on 2026-10-12: the player got into its match
/// 32 s later, through a new sign-in. Six of its 0xD8 fit in this.
const STRAY_PATIENCE: Duration = Duration::from_secs(3);

/// An address that sends without being placed or connecting.
struct Stray {
    since: Instant,
    datagrams: u32,
    /// Told to disconnect: once.
    told: bool,
}

/// The 0.26g match server: runs every match the lobby makes in `registry`
/// on the configured UDP port (`SRP_BIND_HOST`, `SRP_MATCH_PORT`), and
/// hands each finished one to `results`. Returns only on a socket error.
///
/// `store` is the sessions' store: a client seen in a match keeps its
/// session alive while its lobby connection is closed.
pub fn run(
    registry: Arc<MatchRegistry>,
    store: Arc<SessionStore>,
    results: impl ResultsSink + 'static,
) -> io::Result<()> {
    let config = vostok::config::get();
    let socket = UdpSocket::bind(config.match_server.bind_addr())?;
    let settings = Settings::from_env();
    let switch = |on: bool| if on { "on" } else { "off" };
    log::info!(
        "Match server (0.26g) on {}; late inputs: {:?}, clock correction: {}, initial world \
         check: {}, relays packed for {} ms",
        config.match_server.bind_addr(),
        settings.late_inputs,
        switch(settings.clock_correction),
        switch(settings.check_initial_world),
        settings.pack.as_millis()
    );
    serve(
        &socket,
        config.match_server.port,
        registry,
        store,
        Box::new(results),
        settings,
        &AtomicBool::new(false),
    )
}

/// [`run`] on a socket already bound, until `stop` is set: for a caller
/// that picks the address itself (`servers/srp`, and its tests on a free
/// port). The clients are told the port of `registry` ([`MatchRegistry::
/// port`]). The settings are the environment's, as [`run`]'s.
pub fn run_on(
    socket: &UdpSocket,
    registry: Arc<MatchRegistry>,
    store: Arc<SessionStore>,
    results: impl ResultsSink + 'static,
    stop: &AtomicBool,
) -> io::Result<()> {
    run_on_with_replays(socket, registry, store, results, stop, None)
}

/// [`run_on`] whose recordings' writer tells `replays` of every file it
/// keeps, drops or deletes (`SRP_REPLAY_DIR`, `super::replay`): srp's, one
/// for the process, which its lobby and web page share.
pub fn run_on_with_replays(
    socket: &UdpSocket,
    registry: Arc<MatchRegistry>,
    store: Arc<SessionStore>,
    results: impl ResultsSink + 'static,
    stop: &AtomicBool,
    replays: Option<Arc<Replays>>,
) -> io::Result<()> {
    let mut settings = Settings::from_env();
    if let Some(record) = settings.record.as_mut() {
        record.replays = replays.map(Desk);
    }
    let switch = |on: bool| if on { "on" } else { "off" };
    log::info!(
        "Match server (0.26g) on UDP {}; late inputs: {:?}, clock correction: {}, initial world \
         check: {}, relays packed for {} ms",
        socket.local_addr()?,
        settings.late_inputs,
        switch(settings.clock_correction),
        switch(settings.check_initial_world),
        settings.pack.as_millis()
    );
    let port = registry.port();
    serve(
        socket,
        port,
        registry,
        store,
        Box::new(results),
        settings,
        stop,
    )
}

/// The loop of [`run`] on a socket already bound, until `stop` is set.
/// `port` is the one the clients are told (0xE3).
pub(crate) fn serve(
    socket: &UdpSocket,
    port: u16,
    registry: Arc<MatchRegistry>,
    store: Arc<SessionStore>,
    results: Box<dyn ResultsSink>,
    settings: Settings,
    stop: &AtomicBool,
) -> io::Result<()> {
    socket.set_read_timeout(Some(RECEIVE_WAIT))?;
    let mut server = Server::new(port, registry, store, results, settings, Instant::now());
    server.packet_log = PacketLog::from_env();
    let mut buffer = [0; RECEIVE_BUFFER];
    while !stop.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buffer) {
            Ok((len, addr)) => server.receive(addr, &buffer[..len], Instant::now()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            // Windows reports a datagram to a closed port on the next receive.
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            Err(error) => return Err(error),
        }
        for (addr, datagram) in server.poll(Instant::now()) {
            if let Err(error) = socket.send_to(&datagram, addr) {
                log::debug!("Can't send to {addr}: {error}");
            }
        }
    }
    Ok(())
}

pub(crate) struct Server {
    port: u16,
    registry: Arc<MatchRegistry>,
    store: Arc<SessionStore>,
    results: Box<dyn ResultsSink>,
    settings: Settings,
    packet_log: Option<PacketLog>,
    /// `SRP_RECORD_DIR`: the match recordings' writer.
    recorder: Option<Recorder>,
    pub(crate) games: HashMap<u32, Game>,
    /// Clients not yet placed: their 0xD8 hasn't come.
    pending: HashMap<SocketAddr, Transport>,
    /// Each placed client's match and roster place.
    routes: HashMap<SocketAddr, (u32, usize)>,
    /// Addresses already logged as sending without being placed.
    strays_logged: HashSet<SocketAddr>,
    /// The addresses that send without being placed or connecting, while
    /// each has a transport in `pending`.
    stray_datagrams: HashMap<SocketAddr, Stray>,
    next_housekeeping: Instant,
    next_live: Instant,
    /// For tests: the patch revision of a session the store does not hold.
    #[cfg(test)]
    pub(crate) revisions: HashMap<u32, u8>,
}

impl Server {
    pub fn new(
        port: u16,
        registry: Arc<MatchRegistry>,
        store: Arc<SessionStore>,
        results: Box<dyn ResultsSink>,
        settings: Settings,
        now: Instant,
    ) -> Self {
        let recorder = settings.record.clone().and_then(|record| {
            let (dir, min, max) = (
                record.dir.clone(),
                record.min_players,
                record.max_bytes >> 20,
            );
            match Recorder::start(record) {
                Ok(recorder) => {
                    log::info!(
                        "Match recordings in {}: kept for a match {min} players or more have \
                         entered (late joiners counted), {max} MB at most together",
                        dir.display()
                    );
                    Some(recorder)
                }
                Err(error) => {
                    log::error!("Match recordings: can't use {}: {error}", dir.display());
                    None
                }
            }
        });
        Self {
            port,
            registry,
            store,
            results,
            settings,
            packet_log: None,
            recorder,
            games: HashMap::new(),
            pending: HashMap::new(),
            routes: HashMap::new(),
            strays_logged: HashSet::new(),
            stray_datagrams: HashMap::new(),
            next_housekeeping: now,
            next_live: now,
            #[cfg(test)]
            revisions: HashMap::new(),
        }
    }

    pub fn receive(&mut self, addr: SocketAddr, datagram: &[u8], now: Instant) {
        if let Some(&(match_id, index)) = self.routes.get(&addr) {
            // A game that left its place and was given a new one (it left
            // by its own word and came again, the owner's of 2026-10-19)
            // may connect from the address it had: its new 0xD8 is no
            // datagram of the place it left.
            if let Some(game) = self.games.get_mut(&match_id)
                && game.peer_index(addr) == Some(index)
                && !game
                    .gone_session(index)
                    .and_then(|session_id| self.registry.seat(session_id))
                    .is_some_and(|seat| {
                        (seat.match_id, usize::from(seat.place)) != (match_id, index)
                    })
            {
                game.receive(index, datagram, now);
                for (session_id, account_id) in game.take_quits() {
                    // The player left by its own word (0xE0), not by a
                    // link or a game that broke: it has given its place
                    // up. Its session can be put in another match, and the
                    // queue no longer sends it back to this one when it
                    // presses Play. (Until 2026-10-12 it did, for as long
                    // as the match ran: live, a player who left a match of
                    // two was sent back to it three times.) The place
                    // stays in the roster, and the lobby takes the player
                    // out of its room; joining it again while the match is
                    // played puts it in as a new place (the owner's of
                    // 2026-10-19), and the result counts its latest place
                    // only.
                    self.registry.release(session_id);
                    self.store.matchmaking().leave_match(account_id);
                    log::info!(
                        "Match {match_id:#x}: player {index} left by its own word (0xE0): its \
                         place is given up (and its room left, if it was in one)"
                    );
                }
                return;
            }
            self.routes.remove(&addr);
        }
        let packet_log = &self.packet_log;
        let config = Config {
            gather: self.settings.pack,
            ..Config::for_table(Table::Match)
        };
        let transport = self.pending.entry(addr).or_insert_with(|| {
            let mut transport = Transport::with_config(Table::Match, config, now);
            if let Some(Ok(log)) = packet_log
                .as_ref()
                .map(|log| log.connection(Table::Match, addr))
            {
                transport.set_log(log);
            }
            transport
        });
        for event in transport.receive(datagram, now) {
            if let Event::Message {
                id: id::CONNECT,
                payload,
            } = event
                && let Ok(ClientMessage::Connect { session, debug }) =
                    parse_client(id::CONNECT, &payload)
            {
                self.stray_datagrams.remove(&addr);
                self.connect(addr, session, debug, now);
                // The datagram with its 0xD8, for the recording.
                if let Some(&(match_id, index)) = self.routes.get(&addr)
                    && let Some(game) = self.games.get_mut(&match_id)
                {
                    game.record_in(index, datagram, now);
                }
                return;
            }
        }
        if self.stray_datagrams.len() >= STRAYS_KEPT {
            self.stray_datagrams.clear();
        }
        let stray = self.stray_datagrams.entry(addr).or_insert(Stray {
            since: now,
            datagrams: 0,
            told: false,
        });
        stray.datagrams += 1;
        let heard = now.saturating_duration_since(stray.since);
        if !stray.told && stray.datagrams >= STRAY_LIMIT && heard >= STRAY_PATIENCE {
            // Answered until now, as any client is; now it hears that this
            // is not its match.
            stray.told = true;
            transport.disconnect(now);
            log::info!(
                "{addr} sent {} datagrams in {} ms without connecting and is not in a match here \
                 (its match ended?); told to disconnect",
                stray.datagrams,
                heard.as_millis()
            );
        }
        self.log_stray(addr, datagram.len());
    }

    /// A datagram from an address that is not placed and did not connect.
    /// If the same host is placed under another port, its client's port
    /// changed after the first datagram (a NAT), and nothing it sends gets
    /// through.
    fn log_stray(&mut self, addr: SocketAddr, len: usize) {
        let placed = self.routes.keys().find(|other| other.ip() == addr.ip());
        let Some(placed) = placed.copied() else {
            return;
        };
        if self.strays_logged.len() >= STRAYS_KEPT {
            self.strays_logged.clear();
        }
        if self.strays_logged.insert(addr) {
            log::info!(
                "{addr} sent {len} bytes but is not a placed client; its host is placed as \
                 {placed}: the client's port may have changed"
            );
        }
    }

    /// A client's 0xD8: put it in its place in its match.
    fn connect(&mut self, addr: SocketAddr, session_id: u32, debug: u8, now: Instant) {
        let Some(mut transport) = self.pending.remove(&addr) else {
            return;
        };
        let Some(seat) = self.registry.seat(session_id) else {
            log::warn!("{addr} connected for session {session_id:#x}, which has no match seat");
            transport.disconnect(now);
            self.pending.insert(addr, transport);
            return;
        };
        if !self.games.contains_key(&seat.match_id) {
            let Some(formed) = self.registry.formed(seat.match_id) else {
                log::error!(
                    "Match {:#x} is not in the registry; {addr} is turned away",
                    seat.match_id
                );
                transport.disconnect(now);
                self.pending.insert(addr, transport);
                return;
            };
            log::info!(
                "Match {:#x}: map {} ({}), mode {}, {} players",
                formed.match_id,
                formed.map.id,
                formed.map.name,
                formed.map.mode,
                formed.places.len()
            );
            let mut game = Game::new(formed, self.port, self.settings.clone(), now);
            if let Some(recorder) = &self.recorder {
                game.start_recording(recorder, now);
            }
            self.games.insert(seat.match_id, game);
        }
        let growing = survarium::v026g::match_msgs::late_join::has_connect_capability(debug);
        if (growing && debug == 0x81) || (!growing && debug != 0) {
            log::info!(
                "{addr} (session {session_id:#x}) runs with -debug_hash_mismatch: a stock client \
                 like it would read an upload request (0xF6) as a world to compare"
            );
        }
        let index = usize::from(seat.place);
        let revision = self.revision_of(session_id);
        let Some(game) = self.games.get_mut(&seat.match_id) else {
            return;
        };
        if game.growing_roster() != growing {
            transport.disconnect(now);
            self.pending.insert(addr, transport);
            return;
        }
        if let Some(formed) = self.registry.formed(seat.match_id) {
            game.sync_roster(formed, now);
        }
        game.attach(index, addr, session_id, revision, transport, now);
        self.routes.insert(addr, (seat.match_id, index));
        self.store.seen_in_match(session_id);
    }

    /// The patch revision the game of this session signed in with; 0 for a
    /// session the store does not know. How fast a game's clock takes a
    /// correction goes by it (`match_msgs::clock`), so a game that comes
    /// back to its place after an update is asked anew.
    fn revision_of(&self, session_id: u32) -> u8 {
        #[cfg(test)]
        if let Some(revision) = self.revisions.get(&session_id) {
            return *revision;
        }
        self.store.client_revision(session_id).unwrap_or(0)
    }

    pub fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut datagrams = Vec::new();
        let mut results = Vec::new();
        let tell_live = now >= self.next_live;
        if tell_live {
            self.next_live = now + LIVE_EVERY;
        }
        for game in self.games.values_mut() {
            if let Some(formed) = self.registry.formed(game.id) {
                game.sync_roster(formed, now);
            }
            datagrams.extend(game.poll(now));
            for (place, account_id) in game.take_rejected_joins() {
                if self.registry.release_place(game.id, place) {
                    self.store.matchmaking().join_failed(account_id);
                }
            }
            self.registry
                .set_admitting(game.id, game.roster_size(), game.admission(now));
            if tell_live {
                self.registry.set_live(game.id, game.live_now(now));
            }
            results.extend(game.take_result());
        }
        // Before the clients hear the end: back in the lobby, they ask.
        for result in results {
            // And may press Play again while this match lingers.
            self.registry.free_seats(result.match_id);
            self.results.match_ended(result);
        }
        self.pending.retain(|addr, transport| {
            datagrams.extend(
                transport
                    .poll(now)
                    .into_iter()
                    .map(|datagram| (*addr, datagram)),
            );
            !transport.is_gone(now)
        });
        let pending = &self.pending;
        self.stray_datagrams
            .retain(|addr, _| pending.contains_key(addr));
        if now >= self.next_housekeeping {
            self.next_housekeeping = now + HOUSEKEEPING.min(self.settings.timing.linger);
            self.housekeeping(now);
        }
        datagrams
    }

    /// Finished matches free their seats; the clients in a match count as
    /// seen; a made match nobody reached is given up.
    fn housekeeping(&mut self, now: Instant) {
        let over: Vec<u32> = self
            .games
            .iter()
            .filter(|(_, game)| game.is_over(now))
            .map(|(id, _)| *id)
            .collect();
        for match_id in over {
            if let Some(mut game) = self.games.remove(&match_id)
                && let Some(result) = game.close(now)
            {
                // It did not end by itself: everyone left.
                self.results.match_ended(result);
            }
            self.routes.retain(|_, (id, _)| *id != match_id);
            self.registry.end(match_id);
            log::info!("Match {match_id:#x} is over");
        }
        for game in self.games.values() {
            for session_id in game.sessions() {
                self.store.seen_in_match(session_id);
            }
        }
        for (match_id, formed_at) in self.registry.matches() {
            if !self.games.contains_key(&match_id)
                && now.saturating_duration_since(formed_at) >= self.settings.timing.unstarted
                && let Some(formed) = self.registry.end(match_id)
            {
                log::info!("Match {match_id:#x}: no client came; given up");
                self.results.match_ended(MatchResult {
                    match_id,
                    map_id: formed.map.id,
                    game_mode: formed.map.mode,
                    match_level: formed.match_level,
                    end: MatchEnd::NoShow,
                    winner: None,
                    team_points: [0, 0],
                    duration_ms: 0,
                    played_ms: None,
                    trusted_snapshot_ms: None,
                    trusted_by: 0,
                    vs_bots: formed.vs_bots,
                    players: formed
                        .places
                        .iter()
                        .enumerate()
                        .map(|(index, place)| PlayerResult {
                            place: index as u8,
                            session_id: place.session_id,
                            account_id: place.account_id,
                            name: place.name.clone(),
                            team: place.team,
                            driven: place.driven,
                            entered: false,
                            present_at_end: false,
                            left: false,
                            time_in_match_ms: 0,
                            statistics: None,
                            score: 0,
                            kills: 0,
                            deaths: 0,
                            longest_killstreak: 0,
                            kit_changes: 0,
                            supplies: None,
                        })
                        .collect(),
                });
            }
        }
    }
}
