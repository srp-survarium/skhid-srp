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
use updater_logic::rendezvous::{self, Exchange, Outcome, Ours};
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
/// How long the rendezvous is asked for a newer torrent.
const DISCOVER_MS: u64 = 20_000;

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
    /// The newest signed torrent the rendezvous gave, newer than this
    /// updater's: its revision and bytes.
    pub found: Option<(u64, Arc<Vec<u8>>)>,
}

/// What an engine runs: the torrent's swarm (its files' roles), the
/// rendezvous (the newest torrent this end has, the least revision it
/// takes from others), or both.
pub struct Job {
    files: Vec<(Torrent, Vec<Role>)>,
    meeting: Option<Meeting>,
    meeting_trackers: Vec<String>,
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
    fn start(job: Job, listener: Option<TcpListener>, port: u16, limit: Option<u64>) -> Self {
        let (commands, receiver) = mpsc::channel();
        let now = win::ticks();
        let status = Arc::new(Mutex::new(Status {
            phase: Phase::Checking { done: 0, total: 0 },
            peers: 0,
            started: now,
            progressed: now,
            found: None,
        }));
        let shared = status.clone();
        let thread = std::thread::Builder::new()
            .name("torrent".into())
            .spawn(move || {
                let outcome = run(job, listener, port, limit, &receiver, &shared);
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

/// A torrent signed by the release: what it is, its revision, its bytes.
pub struct Signed {
    pub torrent: Torrent,
    pub revision: u64,
    bytes: Arc<Vec<u8>>,
}

/// The revision's torrent as this run knows it.
enum Offered {
    /// Not read yet (or read before the overlay changed the folder).
    Unread,
    Read(Arc<Signed>),
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

    fn start(&mut self, job: Job) -> bool {
        if job.files.is_empty() && job.meeting.is_none() {
            return false;
        }
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
        *engine = Some(Engine::start(job, socket, bound, limit));
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
/// The newest one the rendezvous gave ([`discover`]), beside it.
const FOUND: &str = "updater_skhid.found.torrent";

/// One torrent beside the updater, when it is the release's.
fn signed(layout: &Layout, name: &str) -> Result<Signed, String> {
    let path = format!("{}\\{name}", layout.exe_dir);
    let bytes = win::read_file(&path, MAX_TORRENT).map_err(|code| {
        if win::is_missing(code) {
            format!("no {name}")
        } else {
            format!("{name}: {}", win::error_text(code))
        }
    })?;
    let revision = crate::signed::check(&bytes, layout).map_err(|why| format!("{name}: {why}"))?;
    let torrent = torrent::parse(&bytes)
        .map_err(|error| format!("{name} is not one this updater reads ({error:?})"))?;
    log!("torrent: {name}: signed by the release, revision {revision}");
    Ok(Signed {
        torrent,
        revision,
        bytes: Arc::new(bytes),
    })
}

/// Where the revision's torrent comes from, and why it is trusted: the one
/// place that decides it. Trusted by its signature alone
/// (`crate::signed`: the release's key, a revision not older than the
/// game's), whoever handed it over: the ZIP's file beside the updater, or
/// the newest one the rendezvous gave. The highest revision of them.
fn trusted(layout: &Layout) -> Result<Signed, String> {
    let zip = signed(layout, FILE);
    let found = signed(layout, FOUND);
    match (zip, found) {
        (Ok(zip), Ok(found)) => Ok(if found.revision > zip.revision { found } else { zip }),
        (Ok(one), Err(_)) | (Err(_), Ok(one)) => Ok(one),
        (Err(why), Err(_)) => Err(why),
    }
}

/// The revision's torrent, when the launcher allows torrent and there is
/// one this updater trusts ([`trusted`]); read once a run.
pub fn offered(session: &mut Session, layout: &Layout) -> Option<Arc<Signed>> {
    if !session.torrents.allowed() {
        return None;
    }
    match &session.torrents.offered {
        Offered::Read(torrent) => return Some(torrent.clone()),
        Offered::None => return None,
        Offered::Unread => {}
    }
    match trusted(layout) {
        Ok(signed) => {
            let torrent = &signed.torrent;
            log!(
                "torrent: revision {}: {} files, {} pieces of {} bytes, {} tracker(s)",
                signed.revision,
                torrent.files.len(),
                torrent.piece_count(),
                torrent.piece_length,
                torrent.trackers.len()
            );
            crate::signed::taken(layout, signed.revision);
            let torrent = signed;
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
pub fn fetch(session: &mut Session, signed: &Signed, files: &[(usize, String)]) -> Fetched {
    let torrent = &signed.torrent;
    let mut roles = alloc::vec![Role::Skip; torrent.files.len()];
    for (index, part) in files {
        roles[*index] = Role::Fetch(part.clone());
    }
    if !session.torrents.start(job(Some(signed), Some(roles), None)) {
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
    if let Some((revision, bytes)) = session.torrents.engine().and_then(|engine| engine.status().found) {
        keep(layout, revision, &bytes);
    }
    if session.torrents.sharing() || !session.torrents.allowed() {
        return;
    }
    let signed = offered(session, layout);
    let roles: Option<Vec<Role>> = signed.as_ref().map(|signed| signed.torrent
        .files
        .iter()
        .map(|file| {
            let place = layout.place(&file.path);
            match win::file_size(&place) {
                Ok(size) if size == file.size && size > 0 => Role::Share(place),
                _ => Role::Skip,
            }
        })
        .collect());
    if let (Some(signed), Some(roles)) = (&signed, &roles) {
        let count = roles.iter().filter(|role| matches!(role, Role::Share(_))).count();
        log!("torrent: sharing {count} of its {} files", signed.torrent.files.len());
    }
    // The rendezvous too: others find the newest torrent from this one
    // (none without one: nothing to give). And the bootstrap torrent's two
    // files, from the copies kept of them.
    let mut job = match &signed {
        Some(signed) => job(Some(signed), roles, Some(layout)),
        None => Job {
            files: Vec::new(),
            meeting: None,
            meeting_trackers: Vec::new(),
        },
    };
    job.files.extend(bootstrap(layout));
    session.torrents.start(job);
}

/// The bootstrap torrent (`Skhid-get.bat`'s: the updater and 7zr.exe at one
/// version), a file of the ZIP beside the updater.
const BOOTSTRAP: &str = "skhid-bootstrap.torrent";
/// Where its two files are kept, beside the updater: the updater replaces
/// itself at every release, and the bootstrap's bytes are shared all the
/// same.
const KEPT: &str = "updater_skhid.bootstrap";

/// Whether the file at `path` is file `file` of `torrent`, every piece.
fn whole(torrent: &Torrent, file: usize, path: &str) -> bool {
    if win::file_size(path).ok() != Some(torrent.files[file].size) {
        return false;
    }
    let mut storage = Storage {
        roles: (0..torrent.files.len())
            .map(|index| if index == file { Role::Share(path.into()) } else { Role::Skip })
            .collect(),
        files: BTreeMap::new(),
    };
    torrent
        .file_pieces(file)
        .all(|piece| storage.read(torrent, piece).is_some_and(|data| torrent.piece_ok(piece, &data)))
}

/// The bootstrap torrent and its files' roles: each from its kept copy,
/// made from the file in its place while that is the bootstrap's. Taken
/// only as the checked ZIP installed it (it is not signed: the script that
/// fetches it pins it); `None` without it.
fn bootstrap(layout: &Layout) -> Option<(Torrent, Vec<Role>)> {
    let name = format!("game/binaries/x86/{BOOTSTRAP}");
    let installed = crate::read_installed(layout)?;
    let entry = installed.files.iter().find(|file| file.name == name)?;
    let bytes = win::read_file(&layout.place(&name), MAX_TORRENT).ok()?;
    if updater_logic::sha256::digest(&bytes) != entry.sha256 {
        return None;
    }
    let torrent = torrent::parse(&bytes).ok()?;
    let kept = format!("{}\\{KEPT}", layout.exe_dir);
    let roles: Vec<Role> = (0..torrent.files.len())
        .map(|file| {
            let file_name = torrent.files[file].path.rsplit('/').next().unwrap_or_default();
            let copy = format!("{kept}\\{file_name}");
            if win::file_size(&copy).ok() == Some(torrent.files[file].size) {
                return Role::Share(copy);
            }
            let live = layout.place(&torrent.files[file].path);
            if !whole(&torrent, file, &live) {
                return Role::Skip;
            }
            let staged = format!("{copy}.skhid-new");
            let copied = win::create_folders(&kept)
                .map_err(|code| win::error_text(code))
                .and_then(|()| std::fs::copy(&live, &staged).map(|_| ()).map_err(|error| format!("{error}")))
                .and_then(|()| win::rename(&staged, &copy, true).map_err(|code| win::error_text(code)));
            match copied {
                Ok(()) => {
                    log!("torrent: the bootstrap's {file_name} kept in {KEPT}");
                    Role::Share(copy)
                }
                Err(why) => {
                    log!("torrent: the bootstrap's {file_name} cannot be kept: {why}");
                    Role::Skip
                }
            }
        })
        .collect();
    let count = roles.iter().filter(|role| matches!(role, Role::Share(_))).count();
    if count == 0 {
        return None;
    }
    log!("torrent: sharing the bootstrap's {count} of {} files", torrent.files.len());
    Some((torrent, roles))
}

/// The rendezvous's trackers when no torrent names any: the release's
/// (`release/build_torrent.py`).
const TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "http://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://exodus.desync.com:6969/announce",
];

/// An engine's job: the torrent's swarm with `roles` (when given), and the
/// rendezvous when this updater has the release's key. `layout`: the
/// least revision taken from the rendezvous is the game's (else none is
/// looked for: only given).
fn job(signed: Option<&Signed>, roles: Option<Vec<Role>>, layout: Option<&Layout>) -> Job {
    let meeting = crate::signed::key().map(|key| Meeting {
        hash: rendezvous::info_hash(&key),
        key,
        ours: signed.map(|signed| (signed.revision, signed.bytes.clone())),
        floor: match (signed, layout) {
            (Some(signed), _) => signed.revision + 1,
            (None, Some(layout)) => crate::signed::installed(layout).max(1),
            (None, None) => u64::MAX,
        },
    });
    let meeting_trackers = match signed {
        Some(signed) if !signed.torrent.trackers.is_empty() => signed.torrent.trackers.clone(),
        _ => TRACKERS.iter().map(|text| String::from(*text)).collect(),
    };
    Job {
        files: signed
            .zip(roles)
            .map(|(signed, roles)| (signed.torrent.clone(), roles))
            .into_iter()
            .collect(),
        meeting,
        meeting_trackers,
    }
}

/// Looks for a newer signed torrent than this updater has, from the
/// rendezvous, for a while: when the update server cannot be reached (a
/// blocked network, our servers down, a bare folder). One that comes is
/// kept beside the updater ([`FOUND`]) and is the run's torrent from then.
pub fn discover(session: &mut Session, layout: &Layout) {
    if !session.torrents.allowed() || crate::signed::key().is_none() {
        return;
    }
    let signed = offered(session, layout);
    log!(
        "torrent: the rendezvous asked for a torrent newer than {}",
        signed.as_ref().map_or("none".into(), |signed| format!("revision {}", signed.revision))
    );
    session.torrents.start(job(signed.as_deref(), None, Some(layout)));
    let until = win::ticks() + DISCOVER_MS;
    let mut found = None;
    while win::ticks() < until && session.pump() {
        found = session.torrents.engine().and_then(|engine| engine.status().found);
        if found.is_some() {
            break;
        }
        win::sleep(100);
    }
    session.torrents.stop();
    let Some((revision, bytes)) = found else {
        log!("torrent: the rendezvous gave none newer");
        return;
    };
    keep(layout, revision, &bytes);
    session.torrents.forget();
}

/// A newer torrent the rendezvous gave, kept beside the updater
/// ([`FOUND`]), once.
fn keep(layout: &Layout, revision: u64, bytes: &[u8]) {
    let path = format!("{}\\{FOUND}", layout.exe_dir);
    if win::read_file(&path, MAX_TORRENT).is_ok_and(|kept| kept == bytes) {
        return;
    }
    let staged = format!("{path}.skhid-new");
    match win::write_file(&staged, bytes).and_then(|()| win::rename(&staged, &path, true)) {
        Ok(()) => log!("torrent: revision {revision} from the rendezvous kept as {FOUND}"),
        Err(code) => log!("torrent: {FOUND} cannot be written: {}", win::error_text(code)),
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
    Connected(TcpStream, Which, Peer),
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

/// A tracker, asked for one of the engine's swarms.
/// A tracker, asked for one of the engine's swarms.
struct Tracker {
    address: Address,
    swarm: Which,
    next: u64,
    asking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Which {
    /// The swarm of one of the engine's torrents (its index).
    Files(usize),
    /// The rendezvous (`updater_logic::rendezvous`).
    Rendezvous,
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


/// What a connection is to the engine.
enum Link {
    /// Taken, its handshake not here yet: which swarm it is of is not
    /// known. Its bytes so far.
    Unrouted(Vec<u8>),
    /// Of the swarm of the engine's torrent of this index.
    Files(usize),
    Rendezvous(Exchange),
    /// Done: closed once what it has to send is sent.
    Closing,
}

struct Connection {
    stream: TcpStream,
    out: Vec<u8>,
    link: Link,
    ip: [u8; 4],
    since: u64,
}

fn ip_of(address: SocketAddr) -> Option<[u8; 4]> {
    match address {
        SocketAddr::V4(v4) => Some(v4.ip().octets()),
        SocketAddr::V6(_) => None,
    }
}

/// A torrent's swarm in the engine: the rules, the files, what the rules
/// asked for and is not done yet.
struct Files {
    swarm: Swarm,
    storage: Storage,
    fetching: bool,
    pending: Vec<Action>,
}

/// The rendezvous in the engine.
struct Meeting {
    hash: [u8; 20],
    key: [u8; 32],
    ours: Option<(u64, Arc<Vec<u8>>)>,
    floor: u64,
}

impl Meeting {
    fn ours(&self) -> Option<Ours<'_>> {
        self.ours.as_ref().map(|(revision, bytes)| Ours {
            revision: *revision,
            torrent: bytes,
        })
    }
}

/// A rendezvous connection may stay this long.
const MEETING_MS: u64 = 30_000;

/// The engine's thread: until the update's thread drops its [`Engine`].
fn run(
    job: Job,
    mut listener: Option<TcpListener>,
    mut port: u16,
    limit: Option<u64>,
    commands: &mpsc::Receiver<Command>,
    status: &Mutex<Status>,
) -> Result<(), String> {
    let peer_id = peer_id();
    let mut trackers: Vec<Tracker> = Vec::new();
    let mut add_trackers = |list: &[String], swarm: Which| {
        for text in list {
            if let Some(address) = tracker::address(text) {
                trackers.push(Tracker {
                    address,
                    swarm,
                    next: 0,
                    asking: false,
                });
            }
        }
    };
    let mut swarms: Vec<Files> = Vec::new();
    for (index, (torrent, roles)) in job.files.into_iter().enumerate() {
        add_trackers(&torrent.trackers, Which::Files(index));
        let fetching = roles.iter().any(|role| matches!(role, Role::Fetch(_)));
        let mut storage = Storage {
            roles,
            files: BTreeMap::new(),
        };
        let Some(start) = check_parts(&torrent, &mut storage, status, commands) else {
            return Ok(());
        };
        let mut swarm = Swarm::new(torrent, peer_id, &start, win::ticks());
        let pending = swarm.step(Event::UploadLimit(limit));
        swarms.push(Files {
            swarm,
            storage,
            fetching,
            pending,
        });
    }
    let meeting = job.meeting;
    if meeting.is_some() {
        add_trackers(&job.meeting_trackers, Which::Rendezvous);
    }
    let now = win::ticks();
    {
        let mut status = status.lock().expect("status");
        status.started = now;
        status.progressed = now;
        status.phase = Phase::Running { done: 0, total: 0 };
    }
    let (found_sender, found) = mpsc::channel::<Found>();
    let mut connections: BTreeMap<PeerKey, Connection> = BTreeMap::new();
    let mut next_key: PeerKey = 0;
    // Peers the trackers named, of which swarm, and when each was tried.
    let mut known: BTreeMap<(Which, Peer), u64> = BTreeMap::new();
    let mut connecting = 0_usize;
    let mut last_tick = 0;
    let mut best: Option<(u64, Arc<Vec<u8>>)> = None;
    let mut buffer = alloc::vec![0_u8; 64 * 1024];
    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Listen(socket, bound)) => {
                    listener = socket;
                    port = bound;
                    for tracker in &mut trackers {
                        tracker.next = 0;
                    }
                }
                Ok(Command::Limit(limit)) => {
                    for files in &mut swarms {
                        let more = files.swarm.step(Event::UploadLimit(limit));
                        files.pending.extend(more);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let now = win::ticks();
        let mut busy = false;
        let room = |swarms: &[Files], connections: &BTreeMap<PeerKey, Connection>, ip: [u8; 4]| {
            connections.len() < MAX_PEERS + 8 && swarms.iter().all(|files| files.swarm.may_connect(ip))
        };
        // Peers that connect: which swarm, their handshake says.
        while let Some(socket) = &listener {
            match socket.accept() {
                Ok((stream, address)) => {
                    busy = true;
                    match ip_of(address).filter(|&ip| room(&swarms, &connections, ip)) {
                        Some(ip) if stream.set_nonblocking(true).is_ok() => {
                            next_key += 1;
                            connections.insert(next_key, Connection {
                                stream,
                                out: Vec::new(),
                                link: Link::Unrouted(Vec::new()),
                                ip,
                                since: now,
                            });
                        }
                        _ => drop(stream),
                    }
                }
                Err(_) => break,
            }
        }
        while let Ok(item) = found.try_recv() {
            busy = true;
            match item {
                Found::Tracker(index, answer) => {
                    let tracker = &mut trackers[index];
                    tracker.asking = false;
                    match answer {
                        Ok(answer) => {
                            log!(
                                "torrent: tracker {} ({:?}): {} peer(s), again in {} s",
                                index + 1,
                                tracker.swarm,
                                answer.peers.len(),
                                answer.interval_s
                            );
                            tracker.next = now + answer.interval_s * 1000;
                            for peer in answer.peers {
                                if known.len() < 500 {
                                    known.entry((tracker.swarm, peer)).or_insert(0);
                                }
                            }
                        }
                        Err(why) => {
                            log!("torrent: tracker {} ({:?}): {why}", index + 1, tracker.swarm);
                            tracker.next = now + TRACKER_RETRY_MS;
                        }
                    }
                }
                Found::Connected(stream, which, peer) => {
                    connecting -= 1;
                    if !room(&swarms, &connections, peer.ip) || stream.set_nonblocking(true).is_err() {
                        continue;
                    }
                    next_key += 1;
                    let mut connection = Connection {
                        stream,
                        out: Vec::new(),
                        link: Link::Closing,
                        ip: peer.ip,
                        since: now,
                    };
                    match which {
                        Which::Files(index) => {
                            let files = &mut swarms[index];
                            connection.link = Link::Files(index);
                            let more = files.swarm.step(Event::Connected {
                                peer: next_key,
                                ip: peer.ip,
                            });
                            files.pending.extend(more);
                        }
                        Which::Rendezvous => {
                            let Some(meeting) = &meeting else { continue };
                            let (exchange, hello) =
                                Exchange::start(&meeting.hash, &peer_id, meeting.ours());
                            connection.link = Link::Rendezvous(exchange);
                            connection.out = hello;
                        }
                    }
                    connections.insert(next_key, connection);
                }
                Found::NotConnected => connecting -= 1,
            }
        }
        // Trackers due.
        for (index, tracker) in trackers.iter_mut().enumerate() {
            if tracker.asking || tracker.next > now {
                continue;
            }
            let (info_hash, left, uploaded, downloaded) = match (tracker.swarm, &meeting) {
                (Which::Files(index), _) => {
                    let swarm = &swarms[index].swarm;
                    let (done, wanted) = swarm.progress();
                    (swarm.torrent().v1, wanted - done, swarm.uploaded, swarm.downloaded)
                }
                (Which::Rendezvous, Some(meeting)) => (meeting.hash, 0, 0, 0),
                (Which::Rendezvous, None) => continue,
            };
            tracker.asking = true;
            let address = tracker.address.clone();
            let sender = found_sender.clone();
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
                        left,
                        event: tracker::Event::None,
                    };
                    let _ = sender.send(Found::Tracker(index, announce(address, ask, transaction)));
                });
        }
        // Peers to connect to: of a torrent while it fetches or shares, of
        // the rendezvous while a newer torrent is looked for.
        let candidates: Vec<(Which, Peer)> = known
            .iter()
            .filter(|((which, peer), tried)| {
                (**tried == 0 || now.saturating_sub(**tried) > RETRY_PEER_MS)
                    && !(peer.ip[0] == 127 && peer.port == port)
                    && match which {
                        Which::Files(index) => {
                            let swarm = &swarms[*index].swarm;
                            swarm.may_connect(peer.ip) && (!swarm.complete() || swarm.sharing())
                        }
                        Which::Rendezvous => best.is_none(),
                    }
            })
            .map(|(key, _)| *key)
            .collect();
        for (which, peer) in candidates {
            if connecting >= CONNECTING || connections.len() + connecting >= MAX_PEERS {
                break;
            }
            known.insert((which, peer), now.max(1));
            connecting += 1;
            let sender = found_sender.clone();
            let spawned = std::thread::Builder::new()
                .stack_size(64 * 1024)
                .spawn(move || {
                    let address = SocketAddr::V4(SocketAddrV4::new(peer.ip.into(), peer.port));
                    let _ = sender.send(
                        match TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
                            Ok(stream) => Found::Connected(stream, which, peer),
                            Err(_) => Found::NotConnected,
                        },
                    );
                });
            if spawned.is_err() {
                connecting -= 1;
            }
        }
        // What the peers sent.
        let keys: Vec<PeerKey> = connections.keys().copied().collect();
        for key in keys {
            let mut gone = false;
            for _ in 0..4 {
                let Some(connection) = connections.get_mut(&key) else { break };
                let read = match connection.stream.read(&mut buffer) {
                    Ok(0) => {
                        gone = true;
                        break;
                    }
                    Ok(read) => read,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => {
                        gone = true;
                        break;
                    }
                };
                busy = true;
                let bytes = &buffer[..read];
                // A peer that connected: its handshake routes it.
                if let Link::Unrouted(head) = &mut connection.link {
                    head.extend_from_slice(bytes);
                    if head.len() < 48 {
                        continue;
                    }
                    let head = core::mem::take(head);
                    let hash = &head[28..48];
                    if let Some(index) = swarms.iter().position(|files| hash == files.swarm.torrent().v1) {
                        let files = &mut swarms[index];
                        connection.link = Link::Files(index);
                        let mut more = files.swarm.step(Event::Connected {
                            peer: key,
                            ip: connection.ip,
                        });
                        more.extend(files.swarm.step(Event::Received(key, &head)));
                        files.pending.extend(more);
                    } else if let Some(meeting) = meeting.as_ref().filter(|meeting| hash == meeting.hash) {
                        let (mut exchange, hello) =
                            Exchange::start(&meeting.hash, &peer_id, meeting.ours());
                        connection.out.extend(hello);
                        let outcome = exchange.received(&head, &meeting.hash, meeting.ours());
                        connection.link = Link::Rendezvous(exchange);
                        met(connection, outcome, meeting, &mut best);
                    } else {
                        connection.link = Link::Closing;
                    }
                    continue;
                }
                match &mut connection.link {
                    Link::Files(index) => {
                        let files = &mut swarms[*index];
                        let more = files.swarm.step(Event::Received(key, bytes));
                        files.pending.extend(more);
                    }
                    Link::Rendezvous(exchange) => {
                        if let Some(meeting) = &meeting {
                            let outcome = exchange.received(bytes, &meeting.hash, meeting.ours());
                            met(connection, outcome, meeting, &mut best);
                        }
                    }
                    Link::Unrouted(_) | Link::Closing => {}
                }
            }
            if gone {
                close(&mut connections, &mut swarms, key);
            }
            for files in &mut swarms {
                apply(files, &mut connections)?;
            }
        }
        if now >= last_tick + 100 {
            last_tick = now;
            for files in &mut swarms {
                let more = files.swarm.step(Event::Tick(now));
                files.pending.extend(more);
            }
            // A connection that does not route or meet in time goes.
            let late: Vec<PeerKey> = connections
                .iter()
                .filter(|(_, connection)| {
                    matches!(connection.link, Link::Unrouted(_) | Link::Rendezvous(_) | Link::Closing)
                        && now.saturating_sub(connection.since) > MEETING_MS
                })
                .map(|(key, _)| *key)
                .collect();
            for key in late {
                close(&mut connections, &mut swarms, key);
            }
        }
        for files in &mut swarms {
            apply(files, &mut connections)?;
        }
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
            if connection.out.len() > MAX_OUT + rendezvous::MAX_TORRENT
                || (matches!(connection.link, Link::Closing) && connection.out.is_empty())
            {
                closed.push(*key);
            }
        }
        for key in closed {
            close(&mut connections, &mut swarms, key);
        }
        {
            let mut status = status.lock().expect("status");
            status.peers = connections.len();
            status.found = best.clone();
            // A fetch's job has one torrent: its progress is the engine's.
            if let Some(files) = swarms.first() {
                let (done, total) = files.swarm.progress();
                status.progressed = files.swarm.progressed;
                status.phase = if files.fetching && files.swarm.complete() {
                    Phase::Complete
                } else {
                    Phase::Running { done, total }
                };
            }
        }
        if !busy {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// What a rendezvous connection's bytes led to: a torrent it sent, kept
/// when it is the release's and newer than any so far.
fn met(
    connection: &mut Connection,
    outcome: Outcome,
    meeting: &Meeting,
    best: &mut Option<(u64, Arc<Vec<u8>>)>,
) {
    match outcome {
        Outcome::Going(send) => connection.out.extend(send),
        Outcome::Done { send, torrent } => {
            connection.out.extend(send);
            connection.link = Link::Closing;
            let Some(torrent) = torrent else { return };
            match updater_logic::signed::verify(&torrent, &meeting.key) {
                Ok(revision)
                    if u64::from(revision) >= meeting.floor
                        && best.as_ref().is_none_or(|(best, _)| u64::from(revision) > *best)
                        && torrent::parse(&torrent).is_ok() =>
                {
                    log!("torrent: the rendezvous gave a signed torrent of revision {revision}");
                    *best = Some((u64::from(revision), Arc::new(torrent)));
                }
                Ok(revision) => log!("torrent: the rendezvous gave revision {revision}: not taken"),
                Err(error) => log!("torrent: the rendezvous gave a torrent: {error:?}: not taken"),
            }
        }
        Outcome::Refused => connection.link = Link::Closing,
    }
    if let Link::Closing = connection.link {
        let _ = connection.stream.flush();
    }
}

fn close(connections: &mut BTreeMap<PeerKey, Connection>, swarms: &mut [Files], key: PeerKey) {
    if let Some(connection) = connections.remove(&key)
        && let Link::Files(index) = connection.link
    {
        let files = &mut swarms[index];
        let more = files.swarm.step(Event::Closed(key));
        files.pending.extend(more);
    }
}

/// Does what a swarm asked for, and what that brings (a piece read is
/// given back at once).
fn apply(files: &mut Files, connections: &mut BTreeMap<PeerKey, Connection>) -> Result<(), String> {
    while !files.pending.is_empty() {
        for action in core::mem::take(&mut files.pending) {
            match action {
                Action::Send(key, bytes) => {
                    if let Some(connection) = connections.get_mut(&key) {
                        connection.out.extend_from_slice(&bytes);
                    }
                }
                Action::Close(key) => {
                    if let Some(connection) = connections.get_mut(&key) {
                        connection.link = Link::Closing;
                    }
                }
                Action::Write(piece, data) => {
                    files.storage.write(files.swarm.torrent(), piece, &data)?;
                }
                Action::Load(piece) => {
                    let data = files.storage.read(files.swarm.torrent(), piece);
                    let more = files.swarm.step(Event::Loaded(piece, data));
                    files.pending.extend(more);
                }
            }
        }
    }
    Ok(())
}
