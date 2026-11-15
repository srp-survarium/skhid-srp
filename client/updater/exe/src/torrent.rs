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
//! Only peers from this updater's own trackers ([`trackers`]: never a
//! torrent's, which are outside its signature), HTTP and UDP; IPv4; no DHT,
//! PEX, UPnP, uTP or encryption. Peers are strangers, here too: so many
//! connections an address in all ([`PER_ADDRESS`]), a handshake in time,
//! one set of banned addresses for the updater's run, the torrent given
//! to so many at the rendezvous.

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
use updater_logic::swarm::{Action, Event, HANDSHAKE_MS, MAX_PEERS, PER_ADDRESS, PeerKey, Start, Swarm};
use updater_logic::torrent::{self, MAX_TORRENT, Torrent};
use updater_logic::rendezvous::{self, Exchange, Outcome, Ours};
use updater_logic::tracker::{self, Address, Announce, Ask, Peer};
use updater_logic::upload::Allowance;
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
/// The peers the trackers named that are kept (to connect to), and of one
/// address in a swarm.
const MAX_KNOWN: usize = 500;
/// An HTTP tracker's whole answer comes in this time.
const TRACKER_MS: u64 = 20_000;
/// The times this end's torrent is given to one address at the rendezvous
/// in a run (each a few hundred KB, within the run's allowance too).
const GIVES_PER_ADDRESS: u32 = 2;

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
    /// Peers past their handshake that have a piece wanted.
    pub peers: usize,
    /// Peers connected, of every swarm.
    pub connected: usize,
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
}

/// The addresses banned in this updater's run (a piece that failed, of one
/// sender), whichever engine banned them: every engine starts from them.
type Banned = Arc<Mutex<Vec<[u8; 4]>>>;

/// The run's one upload allowance (`updater_logic::upload`): every swarm and
/// the rendezvous of every engine of the run take from it.
type Upload = Arc<Mutex<Allowance>>;

enum Command {
    Listen(Option<TcpListener>, u16),
}

/// The engine's thread: stopped and waited for when dropped.
pub struct Engine {
    commands: Option<mpsc::Sender<Command>>,
    status: Arc<Mutex<Status>>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    fn start(job: Job, listener: Option<TcpListener>, port: u16, upload: Upload, banned: Banned) -> Self {
        let (commands, receiver) = mpsc::channel();
        let now = win::ticks();
        let status = Arc::new(Mutex::new(Status {
            phase: Phase::Checking { done: 0, total: 0 },
            peers: 0,
            connected: 0,
            started: now,
            progressed: now,
            found: None,
        }));
        let shared = status.clone();
        let thread = std::thread::Builder::new()
            .name("torrent".into())
            .spawn(move || {
                let outcome = run(job, listener, port, &upload, &receiver, &shared, &banned);
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

/// A torrent signed by the release: what it is, its revision, its bytes
/// (`torrent::canonical`: what the signature covers, and the piece layers).
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
    upload: Upload,
    offered: Offered,
    banned: Banned,
    turn: Turn,
}

/// The times the launcher turned torrent on in this run (`1A`): what a
/// download by HTTPS or the watch compares, to see it happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn(u32);

enum Mode {
    /// `port`: the launcher's, kept for when it allows torrent.
    Off { port: Option<u16> },
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
            Torrenting::Off { port } => Mode::Off { port },
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
        Self::of(mode)
    }

    fn of(mode: Mode) -> Self {
        Self {
            mode,
            upload: Arc::new(Mutex::new(Allowance::new(win::ticks()))),
            offered: Offered::Unread,
            banned: Banned::default(),
            turn: Turn(0),
        }
    }

    pub fn turn(&self) -> Turn {
        self.turn
    }

    /// The folder changed (the overlay): the torrent is read again.
    pub fn forget(&mut self) {
        self.offered = Offered::Unread;
    }

    /// One of the launcher's torrent messages; what to answer it.
    pub fn on_message(&mut self, message: FromLauncher) -> Option<ToLauncher<'static>> {
        match message {
            FromLauncher::AllowTorrent(true) => {
                if let Mode::Off { port } = self.mode {
                    log!("torrent: allowed by the launcher");
                    self.mode = Mode::On {
                        port,
                        listener: listen(port),
                        engine: None,
                    };
                    self.turn = Turn(self.turn.0.wrapping_add(1));
                }
                None
            }
            FromLauncher::AllowTorrent(false) => {
                log!("torrent: not allowed by the launcher: stopped");
                let port = match &self.mode {
                    Mode::Off { port } | Mode::On { port, .. } => *port,
                };
                self.mode = Mode::Off { port };
                None
            }
            FromLauncher::Port(number) => {
                let wanted = updater_logic::pipe::port_number(&format!("{number}"));
                let (port, listener, engine) = match &mut self.mode {
                    Mode::On {
                        port,
                        listener,
                        engine,
                    } => (port, listener, engine),
                    Mode::Off { port } => {
                        // Off: nothing listens, so any port is "ok"; it is
                        // the one listened on once torrent is allowed.
                        *port = wanted;
                        return Some(ToLauncher::PortOk);
                    }
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
                // The cap is fixed: the launcher's limit may only lower it.
                let mut upload = self.upload.lock().expect("upload");
                upload.lower(limit);
                log!("torrent: upload limit {limit:?} bytes a second asked: {} allowed", upload.rate());
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
        let upload = self.upload.clone();
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
        *engine = Some(Engine::start(job, socket, bound, upload, self.banned.clone()));
        true
    }

    fn engine(&self) -> Option<&Engine> {
        match &self.mode {
            Mode::On { engine, .. } => engine.as_ref(),
            Mode::Off { .. } => None,
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

    /// Stops the engine and closes the listening socket, for good: before
    /// another updater runs, which listens on the same port.
    pub fn close(&mut self) {
        if let Mode::On { engine, listener, .. } = &mut self.mode {
            *engine = None;
            *listener = None;
        }
    }

    /// The revision's torrent, when the launcher allows torrent and there is
    /// one this updater trusts ([`trusted`]); read once a run.
    pub fn offered(&mut self, layout: &Layout) -> Option<Arc<Signed>> {
        if !self.allowed() {
            return None;
        }
        match &self.offered {
            Offered::Read(torrent) => return Some(torrent.clone()),
            Offered::None => return None,
            Offered::Unread => {}
        }
        match trusted(layout) {
            Ok(signed) => {
                let torrent = &signed.torrent;
                log!(
                    "torrent: revision {}: {} files, {} pieces of {} bytes",
                    signed.revision,
                    torrent.files.len(),
                    torrent.piece_count(),
                    torrent.piece_length
                );
                crate::signed::taken(layout, signed.revision);
                let torrent = signed;
                let torrent = Arc::new(torrent);
                self.offered = Offered::Read(torrent.clone());
                Some(torrent)
            }
            Err(why) => {
                log!("torrent: {why}: HTTPS only this run");
                self.offered = Offered::None;
                None
            }
        }
    }

    /// Shares every file of the torrent this folder has with the torrent's
    /// size, however it came (the torrent, HTTPS, the ZIP): while the launcher
    /// stays open. Nothing when torrent is not allowed or there is no torrent,
    /// or it runs already.
    pub fn share(&mut self, layout: &Layout) {
        if let Some((revision, bytes)) = self.engine().and_then(|engine| engine.status().found) {
            keep(layout, revision, &bytes);
        }
        if self.sharing() || !self.allowed() {
            return;
        }
        let signed = self.offered(layout);
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
            },
        };
        job.files.extend(bootstrap(layout));
        self.start(job);
    }

    /// What the sharing is now.
    fn sharing_now(&self) -> Sharing {
        let upload = self.upload.lock().expect("upload");
        Sharing {
            sent: upload.sent(),
            rate: upload.rate(),
            peers: self.engine().map_or(0, |engine| engine.status().connected),
        }
    }
}

/// What the sharing is at a look ([`share_while`]).
#[derive(Debug, Clone, Copy)]
pub struct Sharing {
    /// Bytes sent in the run, by every swarm and the rendezvous.
    pub sent: u64,
    /// What the run's allowance lets go, bytes a second.
    pub rate: u64,
    /// Peers connected.
    pub peers: usize,
}

/// Why [`share_while`] shared nothing.
#[derive(Debug)]
pub enum NotShared {
    /// The port is another program's (another updater that shares).
    PortBusy,
    /// No torrent this updater takes, no bootstrap: nothing to share.
    Nothing,
}

/// How long a port another program holds is tried again: an updater that
/// shared on it may be closing.
const PORT_WAIT_MS: u64 = 10_000;
/// How often the sharing is looked at again: a torrent the rendezvous gave
/// kept, the engine started again should it have stopped.
const RESHARE_MS: u64 = 10_000;

/// The game's sharing (`crate::seed`), in one call: every file of the
/// folder's torrent, as [`Torrents::share`] shares it while the launcher is
/// open, on `port`, within the run's cap, until `go_on` says stop (asked
/// ten times a second, with what the sharing is). Nothing is fetched or
/// installed.
pub fn share_while(layout: &Layout, port: Option<u16>, go_on: &mut dyn FnMut(&Sharing) -> bool) -> Result<(), NotShared> {
    let until = win::ticks() + PORT_WAIT_MS;
    let listener = loop {
        if let Some(listener) = listen(port) {
            break listener;
        }
        if win::ticks() >= until {
            return Err(NotShared::PortBusy);
        }
        win::sleep(1000);
    };
    log!("torrent: sharing on port {}", listener.local_addr().map_or(0, |address| address.port()));
    let mut torrents = Torrents::of(Mode::On {
        port,
        listener: Some(listener),
        engine: None,
    });
    torrents.share(layout);
    if !torrents.sharing() {
        return Err(NotShared::Nothing);
    }
    let mut shared = win::ticks();
    while go_on(&torrents.sharing_now()) {
        if win::ticks().saturating_sub(shared) >= RESHARE_MS {
            shared = win::ticks();
            torrents.share(layout);
        }
        win::sleep(100);
    }
    torrents.close();
    Ok(())
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
    let bytes = torrent::canonical(&bytes).ok_or_else(|| format!("{name} is not bencoded"))?;
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

/// [`Torrents::offered`], of the session's.
pub fn offered(session: &mut Session, layout: &Layout) -> Option<Arc<Signed>> {
    session.torrents.offered(layout)
}

/// Whether a download by HTTPS of `path` (a path of the torrent) leaves
/// the rest to the torrent: the launcher turned torrent on since `since`
/// (the download's start), the run goes on, and the run's torrent has the
/// file. What HTTPS got stays: the torrent checks its pieces first.
pub fn takes_over(session: &mut Session, layout: &Layout, since: Turn, path: &str) -> bool {
    !session.stopped
        && session.torrents.turn() != since
        && offered(session, layout)
            .is_some_and(|offered| file_index(&offered.torrent, path).is_some())
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


/// The bootstrap torrent (`Skhid-get.bat`'s: the updater and 7zr.exe at one
/// version), a file of the ZIP beside the updater.
const BOOTSTRAP: &str = "skhid-bootstrap.torrent";
/// Where its two files are kept, beside the updater: the updater replaces
/// itself at every release, and the bootstrap's bytes are shared all the
/// same.
const KEPT: &str = "updater_skhid.bootstrap";

/// Whether the file at `path` is file `file` of `torrent`, every piece.
pub fn whole(torrent: &Torrent, file: usize, path: &str) -> bool {
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
            // A copy is shared once every piece of it is checked (a copy of
            // the right size can be anything); else it is made again.
            if whole(&torrent, file, &copy) {
                return Role::Share(copy);
            }
            let live = layout.place(&torrent.files[file].path);
            if !whole(&torrent, file, &live) {
                return Role::Skip;
            }
            let staged = format!("{copy}.skhid-new");
            let copied = win::create_folders(&kept)
                .map_err(win::error_text)
                .and_then(|()| std::fs::copy(&live, &staged).map(|_| ()).map_err(|error| format!("{error}")))
                .and_then(|()| win::rename(&staged, &copy, true).map_err(win::error_text));
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

/// The release's trackers (`release/build_torrent.py` TRACKERS).
const TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "http://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://exodus.desync.com:6969/announce",
];

/// The trackers of every swarm and of the rendezvous: the release's, never
/// a torrent's (its trackers are outside the signature: whoever handed it
/// over could name its own, and be the only one to give peers). A test
/// build has its own (`SKHID_TEST_TRACKERS`, space-separated;
/// `tests/test_torrent.py`); `release/build-zip` builds none.
fn trackers() -> Vec<String> {
    match option_env!("SKHID_TEST_TRACKERS") {
        Some(list) => list.split_whitespace().map(String::from).collect(),
        None => TRACKERS.iter().map(|text| String::from(*text)).collect(),
    }
}

/// An engine's job: the torrent's swarm with `roles` (when given), and the
/// rendezvous when this updater has the release's key. `layout`: the
/// least revision taken from the rendezvous is what this folder has
/// (`signed::installed`: the game's, the highest taken, this updater's
/// own; else none is looked for: only given).
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
    Job {
        files: signed
            .zip(roles)
            .map(|(signed, roles)| (signed.torrent.clone(), roles))
            .into_iter()
            .collect(),
        meeting,
    }
}

/// Looks for a newer signed torrent than this updater has, from the
/// rendezvous, for a while: when the update server cannot be reached (a
/// blocked network, our servers down, a bare folder). The whole while:
/// the newest that came is kept beside the updater ([`FOUND`]) and is the
/// run's torrent from then.
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
        found = session.torrents.engine().and_then(|engine| engine.status().found).or(found);
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

/// The Skhid files of a torrent: every file but the original game's and
/// the patches.
pub fn skhid_files(torrent: &Torrent) -> Vec<usize> {
    (0..torrent.files.len())
        .filter(|&index| {
            let file = &torrent.files[index];
            let in_game = file.path.strip_prefix("game/");
            let patch = in_game.and_then(updater_logic::patches::file_number).is_some();
            let original = in_game.is_some_and(|name| {
                updater_logic::game::FILES
                    .iter()
                    .any(|(path, pin)| *path == name && pin.size == file.size)
            });
            !patch && !original
        })
        .collect()
}

/// Where a Skhid file is fetched by torrent, beside its place.
pub fn skhid_part(layout: &Layout, path: &str) -> String {
    format!("{}.skhid-torrent", layout.place(path))
}

/// What a download of the Skhid files by torrent left (the server could
/// not be asked then): removed once the server answers, whose archive is
/// installed from then on.
pub fn remove_skhid_parts(layout: &Layout) {
    let Ok(signed) = trusted(layout) else { return };
    for index in skhid_files(&signed.torrent) {
        let part = skhid_part(layout, &signed.torrent.files[index].path);
        if win::exists(&part) && win::delete(&part).is_ok() {
            log!("removed the left-over torrent download: {part}");
        }
    }
}

// The engine.

/// What the short threads hand back.
enum Found {
    Tracker(usize, Result<Announce, String>),
    Connected(TcpStream, Which, Peer),
    NotConnected(Which, Peer),
}

struct Storage {
    roles: Vec<Role>,
    files: BTreeMap<usize, std::fs::File>,
}

impl Storage {
    /// File `file` of the torrent, of `size` bytes.
    fn open(&mut self, file: usize, size: u64) -> std::io::Result<&mut std::fs::File> {
        if !self.files.contains_key(&file) {
            let opened = match &self.roles[file] {
                Role::Fetch(path) => {
                    let part = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(path)?;
                    // A part longer than the file (left by another
                    // torrent's file of that name): what is past its size
                    // goes, so a finished part is the file, byte for byte.
                    if part.metadata()?.len() > size {
                        part.set_len(size)?;
                    }
                    part
                }
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
        let handle = self.open(file, torrent.files[file].size).ok()?;
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
            .open(file, torrent.files[file].size)
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
/// The commands that come meanwhile go to `queued`; `None`: the engine is
/// stopped.
fn check_parts(
    torrent: &Torrent,
    storage: &mut Storage,
    status: &Mutex<Status>,
    commands: &mpsc::Receiver<Command>,
    queued: &mut Vec<Command>,
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
                    match commands.try_recv() {
                        Ok(command) => queued.push(command),
                        Err(mpsc::TryRecvError::Disconnected) => return None,
                        Err(mpsc::TryRecvError::Empty) => {}
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
    let wanted = start
        .iter()
        .filter(|start| matches!(start, Start::Checked | Start::Missing))
        .count();
    if wanted > 0 {
        let here = start
            .iter()
            .filter(|start| matches!(start, Start::Checked))
            .count();
        log!("torrent: {here} of the {wanted} pieces to fetch are in the parts already, checked");
    }
    Some(start)
}

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
            // The whole answer in its time: a tracker that trickles holds
            // this thread no longer.
            let until = win::ticks() + TRACKER_MS;
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
                &mut |_, _| win::ticks() < until,
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
    /// This end's torrent for a connection with `ip`: given while that
    /// address had it fewer than [`GIVES_PER_ADDRESS`] times and the upload
    /// limit has room (`room`).
    fn ours(&self, given: &BTreeMap<[u8; 4], u32>, ip: [u8; 4], room: bool) -> Option<Ours<'_>> {
        self.ours.as_ref().map(|(revision, bytes)| Ours {
            revision: *revision,
            torrent: bytes,
            give: room && given.get(&ip).copied().unwrap_or(0) < GIVES_PER_ADDRESS,
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
    upload: &Mutex<Allowance>,
    commands: &mpsc::Receiver<Command>,
    status: &Mutex<Status>,
    shared_bans: &Mutex<Vec<[u8; 4]>>,
) -> Result<(), String> {
    let peer_id = peer_id();
    let mut banned: Vec<[u8; 4]> = shared_bans.lock().expect("bans").clone();
    let mut trackers: Vec<Tracker> = Vec::new();
    let list = self::trackers();
    let mut add_trackers = |swarm: Which| {
        for text in &list {
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
    // Commands that came while the parts were checked: done first.
    let mut queued: Vec<Command> = Vec::new();
    let mut swarms: Vec<Files> = Vec::new();
    for (index, (torrent, roles)) in job.files.into_iter().enumerate() {
        add_trackers(Which::Files(index));
        let fetching = roles.iter().any(|role| matches!(role, Role::Fetch(_)));
        let mut storage = Storage {
            roles,
            files: BTreeMap::new(),
        };
        let Some(start) = check_parts(&torrent, &mut storage, status, commands, &mut queued) else {
            return Ok(());
        };
        let swarm = Swarm::new(torrent, peer_id, &start, banned.clone(), win::ticks());
        swarms.push(Files {
            swarm,
            storage,
            fetching,
            pending: Vec::new(),
        });
    }
    let meeting = job.meeting;
    if meeting.is_some() {
        add_trackers(Which::Rendezvous);
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
    // Outgoing connections being made, by address.
    let mut connecting: BTreeMap<[u8; 4], usize> = BTreeMap::new();
    // The times this end's torrent was given to each address.
    let mut given: BTreeMap<[u8; 4], u32> = BTreeMap::new();
    let mut last_tick = 0;
    let mut best: Option<(u64, Arc<Vec<u8>>)> = None;
    let mut buffer = alloc::vec![0_u8; 64 * 1024];
    loop {
        let mut incoming = core::mem::take(&mut queued);
        loop {
            match commands.try_recv() {
                Ok(command) => incoming.push(command),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        for command in incoming {
            match command {
                Command::Listen(socket, bound) => {
                    listener = socket;
                    port = bound;
                    for tracker in &mut trackers {
                        tracker.next = 0;
                    }
                }
            }
        }
        let now = win::ticks();
        let mut busy = false;
        // The run's allowance, this engine's for this turn of the loop.
        let mut up = upload.lock().expect("upload");
        up.tick(now);
        // Room for one more connection with `ip`: not banned, and fewer
        // than PER_ADDRESS with it in all (taken, made, being made, of any
        // swarm or of none yet).
        let room = |connections: &BTreeMap<PeerKey, Connection>,
                    connecting: &BTreeMap<[u8; 4], usize>,
                    banned: &[[u8; 4]],
                    ip: [u8; 4]| {
            connections.len() < MAX_PEERS + 8
                && !banned.contains(&ip)
                && connections.values().filter(|connection| connection.ip == ip).count()
                    + connecting.get(&ip).copied().unwrap_or(0)
                    < PER_ADDRESS
        };
        // Peers that connect: which swarm, their handshake says.
        while let Some(socket) = &listener {
            match socket.accept() {
                Ok((stream, address)) => {
                    busy = true;
                    match ip_of(address).filter(|&ip| room(&connections, &connecting, &banned, ip)) {
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
                                learn(&mut known, tracker.swarm, peer);
                            }
                        }
                        Err(why) => {
                            log!("torrent: tracker {} ({:?}): {why}", index + 1, tracker.swarm);
                            tracker.next = now + TRACKER_RETRY_MS;
                        }
                    }
                }
                Found::Connected(stream, which, peer) => {
                    connected(&mut connecting, peer.ip);
                    if !room(&connections, &connecting, &banned, peer.ip) || stream.set_nonblocking(true).is_err() {
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
                            }, &mut up);
                            files.pending.extend(more);
                        }
                        Which::Rendezvous => {
                            let Some(meeting) = &meeting else { continue };
                            let ours = meeting.ours(&given, peer.ip, false);
                            let (exchange, hello) = Exchange::start(&meeting.hash, &peer_id, ours);
                            connection.link = Link::Rendezvous(exchange);
                            connection.out = hello;
                        }
                    }
                    connections.insert(next_key, connection);
                }
                Found::NotConnected(which, peer) => {
                    connected(&mut connecting, peer.ip);
                    // Not tried again: room for others the trackers name.
                    known.remove(&(which, peer));
                }
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
        // the rendezvous while it runs (the newest torrent is looked for
        // the whole time).
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
                        Which::Rendezvous => true,
                    }
            })
            .map(|(key, _)| *key)
            .collect();
        for (which, peer) in candidates {
            let pending: usize = connecting.values().sum();
            if pending >= CONNECTING || connections.len() + pending >= MAX_PEERS {
                break;
            }
            if !room(&connections, &connecting, &banned, peer.ip) {
                continue;
            }
            known.insert((which, peer), now.max(1));
            *connecting.entry(peer.ip).or_insert(0) += 1;
            let sender = found_sender.clone();
            let spawned = std::thread::Builder::new()
                .stack_size(64 * 1024)
                .spawn(move || {
                    let address = SocketAddr::V4(SocketAddrV4::new(peer.ip.into(), peer.port));
                    let _ = sender.send(
                        match TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
                            Ok(stream) => Found::Connected(stream, which, peer),
                            Err(_) => Found::NotConnected(which, peer),
                        },
                    );
                });
            if spawned.is_err() {
                connected(&mut connecting, peer.ip);
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
                let ip = connection.ip;
                // This end's torrent may go to the rendezvous peer: within
                // the upload limit (the first swarm's), so many an address.
                let room_up = up.clear();
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
                        }, &mut up);
                        more.extend(files.swarm.step(Event::Received(key, &head), &mut up));
                        files.pending.extend(more);
                    } else if let Some(meeting) = meeting.as_ref().filter(|meeting| hash == meeting.hash) {
                        let ours = meeting.ours(&given, ip, room_up);
                        let (mut exchange, hello) = Exchange::start(&meeting.hash, &peer_id, ours);
                        connection.out.extend(hello);
                        let outcome = exchange.received(&head, &meeting.hash, ours);
                        connection.link = Link::Rendezvous(exchange);
                        if let Some(bytes) = met(connection, outcome, meeting, &mut best) {
                            gave(&mut given, &mut up, ip, bytes);
                        }
                    } else {
                        connection.link = Link::Closing;
                    }
                    continue;
                }
                match &mut connection.link {
                    Link::Files(index) => {
                        let files = &mut swarms[*index];
                        let more = files.swarm.step(Event::Received(key, bytes), &mut up);
                        files.pending.extend(more);
                    }
                    Link::Rendezvous(exchange) => {
                        if let Some(meeting) = &meeting {
                            let ours = meeting.ours(&given, ip, room_up);
                            let outcome = exchange.received(bytes, &meeting.hash, ours);
                            if let Some(bytes) = met(connection, outcome, meeting, &mut best) {
                                gave(&mut given, &mut up, ip, bytes);
                            }
                        }
                    }
                    Link::Unrouted(_) | Link::Closing => {}
                }
            }
            if gone {
                close(&mut connections, &mut swarms, &mut up, key);
            }
            for files in &mut swarms {
                apply(files, &mut connections, &mut up)?;
            }
        }
        if now >= last_tick + 100 {
            last_tick = now;
            for files in &mut swarms {
                let more = files.swarm.step(Event::Tick(now), &mut up);
                files.pending.extend(more);
            }
            // A connection that does not route in a handshake's time, or
            // meet in a meeting's, goes.
            let late: Vec<PeerKey> = connections
                .iter()
                .filter(|(_, connection)| {
                    let age = now.saturating_sub(connection.since);
                    match connection.link {
                        Link::Unrouted(_) => age > HANDSHAKE_MS,
                        Link::Rendezvous(_) | Link::Closing => age > MEETING_MS,
                        Link::Files(_) => false,
                    }
                })
                .map(|(key, _)| *key)
                .collect();
            for key in late {
                close(&mut connections, &mut swarms, &mut up, key);
            }
        }
        for files in &mut swarms {
            apply(files, &mut connections, &mut up)?;
        }
        // The addresses a swarm banned: banned for every swarm and engine
        // of this run.
        for files in &swarms {
            for ip in files.swarm.banned() {
                if !banned.contains(ip) {
                    banned.push(*ip);
                    let mut shared = shared_bans.lock().expect("bans");
                    if !shared.contains(ip) {
                        shared.push(*ip);
                    }
                }
            }
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
                || banned.contains(&connection.ip)
            {
                closed.push(*key);
            }
        }
        for key in closed {
            close(&mut connections, &mut swarms, &mut up, key);
        }
        {
            let mut status = status.lock().expect("status");
            status.peers = swarms.iter().map(|files| files.swarm.useful_peers()).sum();
            status.connected = swarms.iter().map(|files| files.swarm.peer_count()).sum();
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
        drop(up);
        if !busy {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// A peer a tracker named, kept to be connected to: [`PER_ADDRESS`] of an
/// address in a swarm at most, [`MAX_KNOWN`] in all (the one tried longest
/// ago makes room; one never tried yet stays).
fn learn(known: &mut BTreeMap<(Which, Peer), u64>, which: Which, peer: Peer) {
    if known.contains_key(&(which, peer)) {
        return;
    }
    let same = known.keys().filter(|(other, known)| *other == which && known.ip == peer.ip).count();
    if same >= PER_ADDRESS {
        return;
    }
    if known.len() >= MAX_KNOWN {
        let oldest = known
            .iter()
            .filter(|(_, tried)| **tried > 0)
            .min_by_key(|(_, tried)| **tried)
            .map(|(key, _)| *key);
        match oldest {
            Some(key) => {
                known.remove(&key);
            }
            None => return,
        }
    }
    known.insert((which, peer), 0);
}

/// An outgoing connection with `ip` is made, or not: no longer being made.
fn connected(connecting: &mut BTreeMap<[u8; 4], usize>, ip: [u8; 4]) {
    if let Some(count) = connecting.get_mut(&ip) {
        *count -= 1;
        if *count == 0 {
            connecting.remove(&ip);
        }
    }
}

/// This end's torrent (`bytes` of it) went to `ip` at the rendezvous: once
/// more for that address, and out of the run's allowance.
fn gave(given: &mut BTreeMap<[u8; 4], u32>, up: &mut Allowance, ip: [u8; 4], bytes: usize) {
    *given.entry(ip).or_insert(0) += 1;
    up.spend(bytes as u64);
}

/// What a rendezvous connection's bytes led to: a torrent it sent, kept
/// when it is the release's and newer than any so far (as
/// `torrent::canonical` has it). The bytes of this end's torrent when it
/// went to the peer.
fn met(
    connection: &mut Connection,
    outcome: Outcome,
    meeting: &Meeting,
    best: &mut Option<(u64, Arc<Vec<u8>>)>,
) -> Option<usize> {
    let mut gave = None;
    match outcome {
        Outcome::Going(send) => connection.out.extend(send),
        Outcome::Done { send, torrent, gave: given } => {
            if given {
                gave = Some(send.len());
            }
            connection.out.extend(send);
            connection.link = Link::Closing;
            if let Some(torrent) = torrent.and_then(|torrent| torrent::canonical(&torrent)) {
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
        }
        Outcome::Refused => connection.link = Link::Closing,
    }
    if let Link::Closing = connection.link {
        let _ = connection.stream.flush();
    }
    gave
}

fn close(connections: &mut BTreeMap<PeerKey, Connection>, swarms: &mut [Files], up: &mut Allowance, key: PeerKey) {
    if let Some(connection) = connections.remove(&key)
        && let Link::Files(index) = connection.link
    {
        let files = &mut swarms[index];
        let more = files.swarm.step(Event::Closed(key), up);
        files.pending.extend(more);
    }
}

/// Does what a swarm asked for, and what that brings (a piece read is
/// given back at once).
fn apply(files: &mut Files, connections: &mut BTreeMap<PeerKey, Connection>, up: &mut Allowance) -> Result<(), String> {
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
                    let more = files.swarm.step(Event::Loaded(piece, data), up);
                    files.pending.extend(more);
                }
            }
        }
    }
    Ok(())
}
