//! BitTorrent, when the launcher's "Allow torrent" is on
//! (`srp-026g/docs/notes/torrent-downloads.md`): the revision's torrent
//! (signed by the release: [`trusted`]) as an extra source of the original game's
//! files and of missing patches, and the sharing of every file of it this
//! folder has, while the updater runs.
//!
//! The rules are `updater_logic::swarm`'s (one state, one transition); here
//! are the sockets and the files. One thread, the engine's, owns the swarm,
//! the listening socket, the peers' connections (non-blocking) and the
//! files; short threads ask the trackers and make outgoing connections and
//! hand back what they got. The update's thread starts and stops the engine
//! ([`Torrents`]) and reads its [`Status`].
//!
//! Only peers from the torrent's trackers (HTTP and UDP); IPv4; no DHT,
//! PEX, UPnP, uTP or encryption.

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::collections::BTreeMap;
use std::hash::{BuildHasher, RandomState};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::net::{
    Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, ToSocketAddrs, UdpSocket,
};
use std::sync::{Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use updater_logic::pipe::{FromLauncher, ToLauncher, Torrenting};
use updater_logic::swarm::{Action, Event, MAX_PEERS, PeerKey, Start, Swarm};
use updater_logic::torrent::{self, MAX_TORRENT, Torrent};
use updater_logic::tracker::{self, Address, Announce, Ask, Peer};
use updater_logic::url;

use crate::http::{self, HttpError};
use crate::install::Layout;
use crate::log::log;
use crate::{Session, win};

/// Outgoing connections being made at a time.
const CONNECTING: usize = 8;
/// A peer address is not tried again within this.
const RETRY_PEER_MS: u64 = 300_000;
/// A tracker that did not answer is asked again after this.
const TRACKER_RETRY_MS: u64 = 120_000;
/// Bytes waiting to go to one peer past which it is closed: it does not
/// read.
const MAX_OUT: usize = 4 << 20;
/// A download by torrent is left (for HTTPS) when no peer is connected
/// this long after its start, or no piece came for this long.
const NO_PEER_MS: u64 = 20_000;
const STALL_MS: u64 = 60_000;

/// What each file of the torrent is here, for one run of the engine.
#[derive(Debug, Clone)]
pub enum Role {
    Skip,
    /// Fetched into this file (a part beside its place); pieces already in
    /// it are checked first.
    Fetch(String),
    /// Shared from this file, which has the torrent's size: each piece is
    /// checked when read, before a byte of it is sent.
    Share(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Pieces already in the parts being checked: bytes of all.
    Checking { done: u64, total: u64 },
    /// Fetching (the wanted bytes checked, of all) or sharing.
    Running { done: u64, total: u64 },
    /// Every wanted piece is here, checked, and written.
    Complete,
    /// The engine stopped on this (a file it cannot write).
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Status {
    pub phase: Phase,
    /// Connections open now.
    pub peers: usize,
    /// When the engine started fetching, and when a piece last came.
    pub started: u64,
    pub progressed: u64,
}

enum Command {
    Listen(Option<TcpListener>, u16),
    Limit(Option<u64>),
}

/// The engine's thread: stopped and waited for when dropped.
pub struct Engine {
    commands: Option<mpsc::Sender<Command>>,
    status: Arc<Mutex<Status>>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    fn start(
        torrent: Torrent,
        roles: Vec<Role>,
        listener: Option<TcpListener>,
        port: u16,
        limit: Option<u64>,
    ) -> Self {
        let (commands, receiver) = mpsc::channel();
        let now = win::ticks();
        let status = Arc::new(Mutex::new(Status {
            phase: Phase::Checking { done: 0, total: 0 },
            peers: 0,
            started: now,
            progressed: now,
        }));
        let shared = status.clone();
        let thread = std::thread::Builder::new()
            .name("torrent".into())
            .spawn(move || {
                let fetching = roles.iter().any(|role| matches!(role, Role::Fetch(_)));
                let outcome = run(torrent, roles, fetching, listener, port, limit, &receiver, &shared);
                if let Err(why) = outcome {
                    log!("torrent: stopped: {why}");
                    shared.lock().expect("status").phase = Phase::Failed(why);
                }
            })
            .ok();
        Self {
            commands: Some(commands),
            status,
            thread,
        }
    }

    pub fn status(&self) -> Status {
        self.status.lock().expect("status").clone()
    }

    fn send(&self, command: Command) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(command);
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.commands = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The revision's torrent as this run knows it.
enum Offered {
    /// Not read yet (or read before the overlay changed the folder).
    Unread,
    Read(Arc<Torrent>),
    /// None, or none this updater takes: HTTPS only this run.
    None,
}

/// The launcher's torrent settings and what runs of them: the session's
/// one owner of the listening socket and of the engine.
pub struct Torrents {
    mode: Mode,
    limit: Option<u64>,
    offered: Offered,
}

enum Mode {
    Off,
    On {
        port: Option<u16>,
        /// `None`: the port is busy (outgoing connections only).
        listener: Option<TcpListener>,
        engine: Option<Engine>,
    },
}

fn listen(port: Option<u16>) -> Option<TcpListener> {
    match TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port.unwrap_or(0))) {
        Ok(listener) => {
            let _ = listener.set_nonblocking(true);
            Some(listener)
        }
        Err(error) => {
            log!("torrent: port {port:?} cannot be listened on: {error}");
            None
        }
    }
}

fn bound_port(listener: &Option<TcpListener>, port: Option<u16>) -> u16 {
    listener
        .as_ref()
        .and_then(|listener| listener.local_addr().ok())
        .map_or(port.unwrap_or(6881), |address| address.port())
}

impl Torrents {
    pub fn new(settings: Torrenting) -> Self {
        let mode = match settings {
            Torrenting::Off => Mode::Off,
            Torrenting::On { port } => {
                let listener = listen(port);
                log!(
                    "torrent: allowed, port {}",
                    listener.as_ref().map_or("busy".into(), |_| format!(
                        "{}",
                        bound_port(&listener, port)
                    ))
                );
                Mode::On {
                    port,
                    listener,
                    engine: None,
                }
            }
        };
        Self {
            mode,
            limit: None,
            offered: Offered::Unread,
        }
    }

    /// The folder changed (the overlay): the torrent is read again.
    pub fn forget(&mut self) {
        self.offered = Offered::Unread;
    }

    /// One of the launcher's torrent messages; what to answer it.
    pub fn on_message(&mut self, message: FromLauncher) -> Option<ToLauncher<'static>> {
        match message {
            FromLauncher::AllowTorrent(true) => {
                if let Mode::Off = self.mode {
                    log!("torrent: allowed by the launcher");
                    self.mode = Mode::On {
                        port: None,
                        listener: listen(None),
                        engine: None,
                    };
                }
                None
            }
            FromLauncher::AllowTorrent(false) => {
                log!("torrent: not allowed by the launcher: stopped");
                self.mode = Mode::Off;
                None
            }
            FromLauncher::Port(number) => {
                let wanted = updater_logic::pipe::port_number(&format!("{number}"));
                let Mode::On {
                    port,
                    listener,
                    engine,
                } = &mut self.mode
                else {
                    // Off: nothing listens, so any port is "ok".
                    return Some(ToLauncher::PortOk);
                };
                // The old socket goes first: the same port again is free.
                *listener = None;
                *listener = listen(wanted);
                *port = wanted;
                let bound = bound_port(listener, wanted);
                if let Some(engine) = engine {
                    engine.send(Command::Listen(
                        listener.as_ref().and_then(|l| l.try_clone().ok()),
                        bound,
                    ));
                }
                log!("torrent: port {number} asked for: {}", if listener.is_some() { "listening" } else { "busy" });
                Some(if listener.is_some() {
                    ToLauncher::PortOk
                } else {
                    ToLauncher::PortBusy
                })
            }
            FromLauncher::UploadLimit(limit) => {
                log!("torrent: upload limit {limit:?} bytes a second");
                self.limit = limit;
                if let Mode::On {
                    engine: Some(engine),
                    ..
                } = &self.mode
                {
                    engine.send(Command::Limit(limit));
                }
                None
            }
            _ => None,
        }
    }

    pub fn allowed(&self) -> bool {
        matches!(self.mode, Mode::On { .. })
    }

    fn start(&mut self, torrent: &Torrent, roles: Vec<Role>) -> bool {
        let limit = self.limit;
        let Mode::On {
            port,
            listener,
            engine,
        } = &mut self.mode
        else {
            return false;
        };
        *engine = None;
        let bound = bound_port(listener, *port);
        let socket = listener.as_ref().and_then(|l| l.try_clone().ok());
        *engine = Some(Engine::start(torrent.clone(), roles, socket, bound, limit));
        true
    }

    fn engine(&self) -> Option<&Engine> {
        match &self.mode {
            Mode::On { engine, .. } => engine.as_ref(),
            Mode::Off => None,
        }
    }

    /// Stops the engine (its files closed once this returns).
    pub fn stop(&mut self) {
        if let Mode::On { engine, .. } = &mut self.mode {
            *engine = None;
        }
    }

    pub fn sharing(&self) -> bool {
        self.engine().is_some()
    }
}

/// The torrent beside the updater, a file of the players' ZIP.
const FILE: &str = "skhid.torrent";

/// Where the revision's torrent comes from, and why it is trusted: the one
/// place that decides it. The file beside the updater (the ZIP brings it),
/// taken only when its signature is the release's and its revision is not
/// older than the game's (`updater_logic::signed`).
fn trusted(layout: &Layout) -> Result<Torrent, String> {
    let path = format!("{}\\{FILE}", layout.exe_dir);
    let bytes = win::read_file(&path, MAX_TORRENT).map_err(|code| {
        if win::is_missing(code) {
            format!("no {FILE}")
        } else {
            format!("{FILE}: {}", win::error_text(code))
        }
    })?;
    let revision = crate::signed::check(&bytes, layout).map_err(|why| format!("{FILE}: {why}"))?;
    log!("torrent: {FILE}: signed by the release, revision {revision}");
    torrent::parse(&bytes).map_err(|error| format!("{FILE} is not one this updater reads ({error:?})"))
}

/// The revision's torrent, when the launcher allows torrent and there is
/// one this updater trusts ([`trusted`]); read once a run.
pub fn offered(session: &mut Session, layout: &Layout) -> Option<Arc<Torrent>> {
    if !session.torrents.allowed() {
        return None;
    }
    match &session.torrents.offered {
        Offered::Read(torrent) => return Some(torrent.clone()),
        Offered::None => return None,
        Offered::Unread => {}
    }
    match trusted(layout) {
        Ok(torrent) => {
            log!(
                "torrent: {FILE}: {} files, {} pieces of {} bytes, {} tracker(s)",
                torrent.files.len(),
                torrent.piece_count(),
                torrent.piece_length,
                torrent.trackers.len()
            );
            let torrent = Arc::new(torrent);
            session.torrents.offered = Offered::Read(torrent.clone());
            Some(torrent)
        }
        Err(why) => {
            log!("torrent: {why}: HTTPS only this run");
            session.torrents.offered = Offered::None;
            None
        }
    }
}

/// How a download by torrent ended.
pub enum Fetched {
    /// Every file is in its part, every piece checked.
    Done,
    /// Not (no peer, it stalled, torrent turned off, a file): the reason.
    Not(String),
    Stopped,
}

/// Fetches `files` (the torrent's file, its part) by torrent: the launcher
/// shows "checking" (pieces in the parts already) and "downloading" with
/// its bar. Left for HTTPS when no peer comes or the pieces stop coming.
pub fn fetch(session: &mut Session, torrent: &Torrent, files: &[(usize, String)]) -> Fetched {
    let mut roles = alloc::vec![Role::Skip; torrent.files.len()];
    for (index, part) in files {
        roles[*index] = Role::Fetch(part.clone());
    }
    if !session.torrents.start(torrent, roles) {
        return Fetched::Not("torrent is not allowed".into());
    }
    let mut shown = None;
    let outcome = loop {
        if !session.pump() {
            break Fetched::Stopped;
        }
        let Some(status) = session.torrents.engine().map(Engine::status) else {
            break Fetched::Not("torrent was turned off".into());
        };
        let now = win::ticks();
        match status.phase {
            Phase::Checking { done, total } => {
                if shown != Some(0) {
                    shown = Some(0);
                    session.begin_progress();
                    session.state(updater_logic::pipe::state::TORRENT_CHECKING);
                }
                session.progress(done, total);
            }
            Phase::Running { done, total } => {
                if shown != Some(1) {
                    shown = Some(1);
                    session.begin_progress();
                    session.state(updater_logic::pipe::state::DOWNLOADING);
                }
                session.progress(done, total);
                if status.peers == 0 && now.saturating_sub(status.started) > NO_PEER_MS {
                    break Fetched::Not(format!("no peer in {} s", NO_PEER_MS / 1000));
                }
                if now.saturating_sub(status.progressed) > STALL_MS {
                    break Fetched::Not(format!("no piece for {} s", STALL_MS / 1000));
                }
            }
            Phase::Complete => break Fetched::Done,
            Phase::Failed(why) => break Fetched::Not(why),
        }
        win::sleep(100);
    };
    session.torrents.stop();
    outcome
}

/// Shares every file of the torrent this folder has with the torrent's
/// size, however it came (the torrent, HTTPS, the ZIP): while the launcher
/// stays open. Nothing when torrent is not allowed or there is no torrent,
/// or it runs already.
pub fn share(session: &mut Session, layout: &Layout) {
    if session.torrents.sharing() || !session.torrents.allowed() {
        return;
    }
    let Some(torrent) = offered(session, layout) else {
        return;
    };
    let roles: Vec<Role> = torrent
        .files
        .iter()
        .map(|file| {
            let place = layout.place(&file.path);
            match win::file_size(&place) {
                Ok(size) if size == file.size && size > 0 => Role::Share(place),
                _ => Role::Skip,
            }
        })
        .collect();
    let count = roles.iter().filter(|role| matches!(role, Role::Share(_))).count();
    log!("torrent: sharing {count} of its {} files", torrent.files.len());
    if count > 0 {
        session.torrents.start(&torrent, roles);
    }
}

/// A file of the torrent by its path.
pub fn file_index(torrent: &Torrent, path: &str) -> Option<usize> {
    torrent.files.iter().position(|file| file.path == path)
}

// The engine.

/// What the short threads hand back.
enum Found {
    Tracker(usize, Result<Announce, String>),
    Connected(TcpStream, Peer),
    NotConnected,
}

struct Storage {
    roles: Vec<Role>,
    files: BTreeMap<usize, std::fs::File>,
}

impl Storage {
    fn open(&mut self, file: usize) -> std::io::Result<&mut std::fs::File> {
        if !self.files.contains_key(&file) {
            let opened = match &self.roles[file] {
                Role::Fetch(path) => std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(path)?,
                Role::Share(path) => std::fs::File::open(path)?,
                Role::Skip => return Err(ErrorKind::NotFound.into()),
            };
            self.files.insert(file, opened);
        }
        Ok(self.files.get_mut(&file).expect("opened"))
    }

    /// A piece's bytes as on the wire (the file's, then pad zeros).
    fn read(&mut self, torrent: &Torrent, piece: u32) -> Option<Vec<u8>> {
        let (file, of_file) = torrent.piece_file(piece);
        let at = u64::from(piece) * u64::from(torrent.piece_length) - torrent.files[file].offset;
        let handle = self.open(file).ok()?;
        let mut data = alloc::vec![0_u8; torrent.piece_size(piece) as usize];
        handle.seek(SeekFrom::Start(at)).ok()?;
        handle.read_exact(&mut data[..of_file as usize]).ok()?;
        Some(data)
    }

    fn write(&mut self, torrent: &Torrent, piece: u32, data: &[u8]) -> Result<(), String> {
        let (file, of_file) = torrent.piece_file(piece);
        let at = u64::from(piece) * u64::from(torrent.piece_length) - torrent.files[file].offset;
        let Role::Fetch(path) = &self.roles[file] else {
            return Ok(());
        };
        let path = path.clone();
        let handle = self
            .open(file)
            .map_err(|error| format!("cannot open {path}: {error}"))?;
        handle
            .seek(SeekFrom::Start(at))
            .and_then(|_| handle.write_all(&data[..of_file as usize]))
            .map_err(|error| format!("cannot write {path}: {error}"))
    }
}

fn random_u64() -> u64 {
    RandomState::new().hash_one(win::ticks())
}

fn peer_id() -> [u8; 20] {
    let mut id = *b"-SK0001-000000000000";
    for (at, byte) in id[8..].iter_mut().enumerate() {
        *byte = b'a' + (random_u64().rotate_left(at as u32) % 26) as u8;
    }
    id
}

/// What the parts hold already: each wanted piece checked, kept or not.
fn check_parts(
    torrent: &Torrent,
    storage: &mut Storage,
    status: &Mutex<Status>,
    commands: &mpsc::Receiver<Command>,
) -> Option<Vec<Start>> {
    let mut start = alloc::vec![Start::Absent; torrent.piece_count() as usize];
    let total: u64 = storage
        .roles
        .iter()
        .enumerate()
        .filter(|(_, role)| matches!(role, Role::Fetch(_)))
        .map(|(file, _)| torrent.files[file].size)
        .sum();
    let mut done = 0;
    for file in 0..torrent.files.len() {
        let role = storage.roles[file].clone();
        let pieces = torrent.file_pieces(file);
        match role {
            Role::Skip => {}
            Role::Share(_) => pieces.for_each(|piece| start[piece as usize] = Start::Offered),
            Role::Fetch(path) => {
                let size = win::file_size(&path).unwrap_or(0);
                for piece in pieces {
                    if let Err(mpsc::TryRecvError::Disconnected) = commands.try_recv() {
                        return None;
                    }
                    let end = (u64::from(piece) + 1) * u64::from(torrent.piece_length)
                        - torrent.files[file].offset;
                    let have = size >= end.min(torrent.files[file].size)
                        && storage
                            .read(torrent, piece)
                            .is_some_and(|data| torrent.piece_ok(piece, &data));
                    start[piece as usize] = if have { Start::Checked } else { Start::Missing };
                    done += u64::from(torrent.piece_file(piece).1);
                    status.lock().expect("status").phase = Phase::Checking { done, total };
                }
            }
        }
    }
    Some(start)
}

struct Tracker {
    address: Address,
    next: u64,
    asking: bool,
}

/// One announce, on its own thread.
fn announce(address: Address, ask: Ask, transaction: u32) -> Result<Announce, String> {
    match address {
        Address::Http(base) => {
            let text = tracker::http_url(&base, &ask);
            let url = url::patch(&text).ok_or("not an address")?;
            let mut body = Vec::new();
            http::get_file(
                &url,
                0,
                tracker::MAX_ANSWER as u64,
                http::Slow::Wait,
                &mut |_| {},
                &mut |_, chunk| {
                    body.extend_from_slice(chunk);
                    Ok(())
                },
                &mut |_, _| true,
            )
            .map_err(|error: HttpError| format!("{error:?}"))?;
            tracker::parse_http(&body).map_err(|error| format!("{error:?}"))
        }
        Address::Udp { host, port } => {
            let target = (host.as_str(), port)
                .to_socket_addrs()
                .map_err(|error| format!("{error}"))?
                .find(SocketAddr::is_ipv4)
                .ok_or("no IPv4 address")?;
            let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
                .map_err(|error| format!("{error}"))?;
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|error| format!("{error}"))?;
            let mut buffer = [0_u8; 2048];
            let mut exchange = |request: &[u8]| -> Result<Vec<u8>, String> {
                for _ in 0..2 {
                    socket.send_to(request, target).map_err(|error| format!("{error}"))?;
                    if let Ok((read, from)) = socket.recv_from(&mut buffer)
                        && from == target
                    {
                        return Ok(buffer[..read].to_vec());
                    }
                }
                Err("no answer".into())
            };
            let connection =
                tracker::udp_connected(&exchange(&tracker::udp_connect(transaction))?, transaction)
                    .map_err(|error| format!("{error:?}"))?;
            let request = tracker::udp_announce(connection, transaction, transaction ^ 0x5A5A, &ask);
            tracker::udp_announced(&exchange(&request)?, transaction)
                .map_err(|error| format!("{error:?}"))
        }
    }
}

struct Connection {
    stream: TcpStream,
    out: Vec<u8>,
}

fn ip_of(address: SocketAddr) -> Option<[u8; 4]> {
    match address {
        SocketAddr::V4(v4) => Some(v4.ip().octets()),
        SocketAddr::V6(_) => None,
    }
}

/// The engine's thread: until the update's thread drops its [`Engine`].
fn run(
    torrent: Torrent,
    roles: Vec<Role>,
    fetching: bool,
    mut listener: Option<TcpListener>,
    mut port: u16,
    limit: Option<u64>,
    commands: &mpsc::Receiver<Command>,
    status: &Mutex<Status>,
) -> Result<(), String> {
    let mut storage = Storage {
        roles,
        files: BTreeMap::new(),
    };
    let Some(start) = check_parts(&torrent, &mut storage, status, commands) else {
        return Ok(());
    };
    let peer_id = peer_id();
    let mut trackers: Vec<Tracker> = torrent
        .trackers
        .iter()
        .filter_map(|text| tracker::address(text))
        .map(|address| Tracker {
            address,
            next: 0,
            asking: false,
        })
        .collect();
    let info_hash = torrent.v1;
    let now = win::ticks();
    let mut swarm = Swarm::new(torrent, peer_id, &start, now);
    {
        let mut status = status.lock().expect("status");
        status.started = now;
        status.progressed = now;
    }
    let (found_sender, found) = mpsc::channel::<Found>();
    let mut connections: BTreeMap<PeerKey, Connection> = BTreeMap::new();
    let mut next_key: PeerKey = 0;
    // Peers the trackers named, and when each was last tried.
    let mut known: BTreeMap<Peer, u64> = BTreeMap::new();
    let mut connecting = 0_usize;
    let mut last_tick = 0;
    let mut actions = swarm.step(Event::UploadLimit(limit));
    loop {
        // The update's thread: gone, or a new port or limit.
        loop {
            match commands.try_recv() {
                Ok(Command::Listen(socket, bound)) => {
                    listener = socket;
                    port = bound;
                    for tracker in &mut trackers {
                        tracker.next = 0;
                    }
                }
                Ok(Command::Limit(limit)) => actions.extend(swarm.step(Event::UploadLimit(limit))),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let now = win::ticks();
        let mut busy = false;
        // Peers that connect.
        while let Some(socket) = &listener {
            match socket.accept() {
                Ok((stream, address)) => {
                    busy = true;
                    match ip_of(address).filter(|&ip| swarm.may_connect(ip)) {
                        Some(ip) if stream.set_nonblocking(true).is_ok() => {
                            next_key += 1;
                            connections.insert(next_key, Connection { stream, out: Vec::new() });
                            actions.extend(swarm.step(Event::Connected { peer: next_key, ip }));
                        }
                        _ => drop(stream),
                    }
                }
                Err(_) => break,
            }
        }
        // What the short threads got.
        while let Ok(item) = found.try_recv() {
            busy = true;
            match item {
                Found::Tracker(index, answer) => {
                    let tracker = &mut trackers[index];
                    tracker.asking = false;
                    match answer {
                        Ok(answer) => {
                            log!(
                                "torrent: tracker {}: {} peer(s), again in {} s",
                                index + 1,
                                answer.peers.len(),
                                answer.interval_s
                            );
                            tracker.next = now + answer.interval_s * 1000;
                            for peer in answer.peers {
                                if known.len() < 500 {
                                    known.entry(peer).or_insert(0);
                                }
                            }
                        }
                        Err(why) => {
                            log!("torrent: tracker {}: {why}", index + 1);
                            tracker.next = now + TRACKER_RETRY_MS;
                        }
                    }
                }
                Found::Connected(stream, peer) => {
                    connecting -= 1;
                    if swarm.may_connect(peer.ip) && stream.set_nonblocking(true).is_ok() {
                        next_key += 1;
                        connections.insert(next_key, Connection { stream, out: Vec::new() });
                        actions.extend(swarm.step(Event::Connected {
                            peer: next_key,
                            ip: peer.ip,
                        }));
                    }
                }
                Found::NotConnected => connecting -= 1,
            }
        }
        // Trackers due.
        let (done, wanted) = swarm.progress();
        for (index, tracker) in trackers.iter_mut().enumerate() {
            if tracker.asking || tracker.next > now {
                continue;
            }
            tracker.asking = true;
            let ask_address = tracker.address.clone();
            let sender = found_sender.clone();
            let (uploaded, downloaded) = (swarm.uploaded, swarm.downloaded);
            let transaction = random_u64() as u32;
            let _ = std::thread::Builder::new()
                .stack_size(256 * 1024)
                .spawn(move || {
                    let ask = Ask {
                        info_hash: &info_hash,
                        peer_id: &peer_id,
                        port,
                        uploaded,
                        downloaded,
                        left: wanted - done,
                        event: tracker::Event::None,
                    };
                    let _ = sender.send(Found::Tracker(index, announce(ask_address, ask, transaction)));
                });
        }
        // Peers to connect to.
        if !swarm.complete() || swarm.sharing() {
            let candidates: Vec<Peer> = known
                .iter()
                .filter(|(peer, tried)| {
                    (**tried == 0 || now.saturating_sub(**tried) > RETRY_PEER_MS)
                        && swarm.may_connect(peer.ip)
                        && !(peer.ip[0] == 127 && peer.port == port)
                })
                .map(|(peer, _)| *peer)
                .collect();
            for peer in candidates {
                if connecting >= CONNECTING || swarm.peer_count() + connecting >= MAX_PEERS {
                    break;
                }
                known.insert(peer, now.max(1));
                connecting += 1;
                let sender = found_sender.clone();
                let spawned = std::thread::Builder::new()
                    .stack_size(64 * 1024)
                    .spawn(move || {
                        let address = SocketAddr::V4(SocketAddrV4::new(peer.ip.into(), peer.port));
                        let _ = sender.send(
                            match TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
                                Ok(stream) => Found::Connected(stream, peer),
                                Err(_) => Found::NotConnected,
                            },
                        );
                    });
                if spawned.is_err() {
                    connecting -= 1;
                }
            }
        }
        // What the peers sent.
        let keys: Vec<PeerKey> = connections.keys().copied().collect();
        let mut buffer = alloc::vec![0_u8; 64 * 1024];
        for key in keys {
            let mut gone = false;
            for _ in 0..4 {
                let Some(connection) = connections.get_mut(&key) else { break };
                match connection.stream.read(&mut buffer) {
                    Ok(0) => gone = true,
                    Ok(read) => {
                        busy = true;
                        actions.extend(swarm.step(Event::Received(key, &buffer[..read])));
                        apply(&mut swarm, &mut storage, &mut connections, &mut actions)?;
                        continue;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                    Err(_) => gone = true,
                }
                break;
            }
            if gone {
                connections.remove(&key);
                actions.extend(swarm.step(Event::Closed(key)));
            }
        }
        if now >= last_tick + 100 {
            last_tick = now;
            actions.extend(swarm.step(Event::Tick(now)));
        }
        apply(&mut swarm, &mut storage, &mut connections, &mut actions)?;
        // What goes to the peers.
        let mut closed = Vec::new();
        for (key, connection) in &mut connections {
            while !connection.out.is_empty() {
                match connection.stream.write(&connection.out) {
                    Ok(0) => {
                        closed.push(*key);
                        break;
                    }
                    Ok(written) => {
                        busy = true;
                        connection.out.drain(..written);
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => {
                        closed.push(*key);
                        break;
                    }
                }
            }
            if connection.out.len() > MAX_OUT {
                closed.push(*key);
            }
        }
        for key in closed {
            connections.remove(&key);
            actions.extend(swarm.step(Event::Closed(key)));
        }
        {
            let (done, total) = swarm.progress();
            let mut status = status.lock().expect("status");
            status.peers = swarm.peer_count();
            status.progressed = swarm.progressed;
            status.phase = if fetching && swarm.complete() {
                Phase::Complete
            } else {
                Phase::Running { done, total }
            };
        }
        if !busy {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Does what the swarm asked for, and what that brings (a piece read is
/// given back at once).
fn apply(
    swarm: &mut Swarm,
    storage: &mut Storage,
    connections: &mut BTreeMap<PeerKey, Connection>,
    actions: &mut Vec<Action>,
) -> Result<(), String> {
    while !actions.is_empty() {
        for action in core::mem::take(actions) {
            match action {
                Action::Send(key, bytes) => {
                    if let Some(connection) = connections.get_mut(&key) {
                        connection.out.extend_from_slice(&bytes);
                    }
                }
                Action::Close(key) => {
                    connections.remove(&key);
                }
                Action::Write(piece, data) => storage.write(swarm.torrent(), piece, &data)?,
                Action::Load(piece) => {
                    let data = storage.read(swarm.torrent(), piece);
                    actions.extend(swarm.step(Event::Loaded(piece, data)));
                }
            }
        }
    }
    Ok(())
}
