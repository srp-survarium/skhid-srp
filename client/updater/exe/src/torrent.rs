//! BitTorrent, when the launcher allows it: the release-signed torrent as an
//! extra source of the game's files and missing patches, and sharing of what
//! this folder has. Engine: libtorrent in `skhid_torrent.dll`
//! (`crate::libtorrent`); without it, HTTPS only. [`Torrents`] owns the
//! session. Spec: `srp-026g/docs/notes/torrent-downloads.md`.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use updater_logic::checked::Identity;
use updater_logic::newest::{self, Item, Newest};
use updater_logic::pipe::{FromLauncher, ToLauncher, Torrenting};
use updater_logic::shared::{self, Record};
use updater_logic::torrent::{self, MAX_TORRENT, Torrent};
use updater_logic::upload;

use crate::install::Layout;
use crate::libtorrent::{self, Event, FileSpec, Role, TorrentId};
use crate::log::log;
use crate::{Session, win};

/// A torrent download falls back to HTTPS with no peer this long after its
/// check, or no piece for this long.
const NO_PEER_MS: u64 = 20_000;
const STALL_MS: u64 = 60_000;
/// Discovery: how long the DHT is asked, then the swarm for the info.
const NEWEST_MS: u64 = 20_000;
const METADATA_MS: u64 = 60_000;
/// The DHT is asked again this often; after an item, a higher one is
/// waited for this long.
const ASK_AGAIN_MS: u64 = 5_000;
const HIGHER_MS: u64 = 3_000;
/// DHT nodes forget an item in about two hours.
const PUT_AGAIN_MS: u64 = 30 * 60_000;
const PUT_RETRY_MS: u64 = 30_000;
const DHT_BOOTSTRAP: &str =
    "dht.libtorrent.org:25401,router.bittorrent.com:6881,router.utorrent.com:6881,dht.transmissionbt.com:6881";

/// In `game\torrent`: the check record ([`updater_logic::shared`]), and an
/// empty folder for the files a session neither fetches nor shares, so none
/// of them is read.
const CHECKED: &str = "checked";
const NOWHERE: &str = "none";

/// Where an earlier build kept its torrent state, beside the updater.
const OLD_FILES: [&str; 4] = [
    "updater_skhid.torrent.checked",
    "updater_skhid.port",
    "updater_skhid.found.torrent",
    "updater_skhid.torrent.revision",
];
const OLD_FOLDERS: [&str; 2] = ["updater_skhid.torrent-none", "updater_skhid.bootstrap"];

/// Built for `tests/test_torrent.py`: loopback peers only.
fn test_build() -> bool {
    option_env!("SKHID_TEST_TRACKERS").is_some()
}

fn dht_bootstrap() -> &'static str {
    match option_env!("SKHID_TEST_DHT") {
        Some(nodes) => nodes,
        None if test_build() => "127.0.0.1:1", // no DHT
        None => DHT_BOOTSTRAP,
    }
}

/// Same as TRACKERS in `release/build_torrent.py`.
const TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "http://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://exodus.desync.com:6969/announce",
];

/// Never a torrent's own: those are outside the signature, so whoever
/// handed it over could name trackers that list only itself.
fn trackers() -> Vec<String> {
    match option_env!("SKHID_TEST_TRACKERS") {
        Some(list) => list.split_whitespace().map(String::from).collect(),
        None => TRACKERS.iter().map(|text| String::from(*text)).collect(),
    }
}

/// A torrent signed by the release; `bytes` are canonical, as signed.
pub struct Signed {
    pub torrent: Torrent,
    pub revision: u64,
    bytes: Vec<u8>,
}

/// The revision's torrent as this run knows it.
enum Offered {
    /// Not read yet, or stale since the overlay changed the folder.
    Unread,
    Read(Arc<Signed>),
    /// None this updater takes: HTTPS only this run.
    None,
}

/// Counts the launcher turning torrent on (`1A`), so an HTTPS download can
/// see it happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn(u32);

/// The launcher's torrent settings and the session that runs them.
pub struct Torrents {
    mode: Mode,
    /// The updater's folder: the DLL.
    dir: String,
    /// `game\torrent`: the record, the empty folder.
    state: String,
    /// Path of [`PORT_FILE`].
    kept_port: String,
    /// Bytes a second, all sharing together.
    rate: u64,
    offered: Offered,
    /// Banned for the whole run, across sessions.
    banned: Vec<String>,
    /// v2 info hashes of this run's torrents: the record keeps only theirs.
    known: Vec<[u8; 32]>,
    /// Uploaded by this run's torrents already removed.
    sent: u64,
    turn: Turn,
}

enum Mode {
    /// `port`: the launcher's, for when it allows torrent.
    Off { port: Option<u16> },
    On { port: Option<u16>, engine: Box<Engine> },
    /// For good: another updater takes the port.
    Closed,
}

struct Engine {
    session: libtorrent::Session,
    listen: Listen,
    job: Job,
}

enum Listen {
    /// `answer`: the launcher waits for it (`21`).
    Asked { want: Want, answer: bool },
    Listening(u16),
    Busy(Option<u16>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    /// The launcher's; if busy, reported busy.
    Named(u16),
    /// From [`PORT_FILE`]; if busy, any.
    Kept(u16),
    /// The one picked is kept.
    Any,
}

impl Want {
    fn port(self) -> Option<u16> {
        match self {
            Self::Named(port) | Self::Kept(port) => Some(port),
            Self::Any => None,
        }
    }
}

/// The port picked the first time, used when the launcher names none, so
/// peers that knew it find it again.
const PORT_FILE: &str = "port";

fn kept_port_number(kept: &str) -> Option<u16> {
    let text = win::read_file(kept, 16).ok()?;
    updater_logic::pipe::port_number(core::str::from_utf8(&text).ok()?.trim())
}

/// What the session does: one thing at a time.
enum Job {
    Idle,
    Fetching(Added),
    /// Hashes every piece of files, nothing more.
    Checking(Added),
    /// The newest release, from the DHT and its swarm.
    Discovering(Discovery),
    /// The folder's files of the run's torrent, the bootstrap's kept copies,
    /// and the signed item on the DHT.
    Sharing {
        swarm: Option<Added>,
        bootstrap: Option<Added>,
        put: Option<Put>,
    },
}

impl Job {
    fn added(&mut self) -> Vec<&mut Added> {
        match self {
            Job::Fetching(added) | Job::Checking(added) => alloc::vec![added],
            Job::Sharing { swarm, bootstrap, .. } => swarm.iter_mut().chain(bootstrap.iter_mut()).collect(),
            Job::Idle | Job::Discovering(_) => Vec::new(),
        }
    }
}

struct Added {
    id: TorrentId,
    v2: [u8; 32],
    piece_length: u32,
    files: Vec<Placed>,
    check: Check,
}

/// Read from the torrent's status, not alerts: an alert can be dropped, and
/// a failed check posts none.
enum Check {
    /// `moved`: when `ppm` last changed.
    Running { ppm: u32, moved: u64 },
    Done,
    /// An unreadable file, or no progress for [`CHECK_STALL_MS`].
    Failed,
}

const CHECK_STALL_MS: u64 = 60_000;

/// `identity`: as when added, to see a shared file change during its check.
struct Placed {
    path: String,
    size: u64,
    role: Role,
    identity: Option<Identity>,
}

/// The signed item this updater puts on the DHT.
struct Put {
    item: Item,
    state: PutState,
}

enum PutState {
    /// Not put yet in this sharing.
    Due,
    /// Stored on no node yet (the DHT may not have been found): retried
    /// after [`PUT_RETRY_MS`].
    Sent(u64),
    /// Put again after [`PUT_AGAIN_MS`].
    Stored(u64),
}

enum Discovery {
    /// `best`: the highest verified item of `floor` or higher, and when it
    /// came.
    Asking {
        floor: u64,
        asked: u64,
        best: Option<(Newest, Item, u64)>,
    },
    /// Its torrent's info from the swarm.
    Fetching { newest: Newest, item: Item, id: TorrentId },
    /// The signed torrent, rebuilt from the item and the info.
    Found { revision: u64, bytes: Vec<u8> },
}

/// `copies`: (file index, kept copy).
struct Bootstrap {
    bytes: Vec<u8>,
    torrent: Torrent,
    copies: Vec<(usize, String)>,
}

enum Progress {
    Checking { done: u64, total: u64 },
    Running { done: u64, total: u64, peers: u32 },
    Failed(String),
}

impl Engine {
    /// The launcher's answer, when it waits for one.
    fn listening(&mut self, port: u16, kept_port: &str) -> Option<ToLauncher<'static>> {
        let answer = matches!(self.listen, Listen::Asked { answer: true, .. });
        if !matches!(self.listen, Listen::Listening(at) if at == port) {
            log!("torrent: listening on port {port}");
        }
        if let Listen::Asked { want: Want::Any, .. } = self.listen {
            match win::write_file(kept_port, format!("{port}\r\n").as_bytes()) {
                Ok(()) => log!("torrent: port {port} picked, kept in torrent\\{PORT_FILE}"),
                Err(code) => log!("torrent: port {port} picked; torrent\\{PORT_FILE}: {}", win::error_text(code)),
            }
        }
        self.listen = Listen::Listening(port);
        answer.then_some(ToLauncher::PortOk)
    }

    fn listen_failed(&mut self, port: u16, why: &str) -> Option<ToLauncher<'static>> {
        log!("torrent: port {port} cannot be listened on ({why})");
        // Each interface reports its own failure: one for a port no longer asked
        // (the kept one, after the switch to any) is old news.
        if let Listen::Asked { want, .. } = self.listen
            && want.port() != Some(port)
        {
            return None;
        }
        let (asked, answer) = match self.listen {
            // The kept port is taken: any, and keep that one.
            Listen::Asked { want: Want::Kept(_), answer } => {
                self.session.listen(None);
                self.listen = Listen::Asked { want: Want::Any, answer };
                return None;
            }
            Listen::Asked { want, answer } => (want.port(), answer),
            Listen::Listening(port) => (Some(port), false),
            Listen::Busy(port) => (port, false),
        };
        self.listen = Listen::Busy(asked);
        answer.then_some(ToLauncher::PortBusy)
    }

    /// Advances each running check from its status; a done one is recorded.
    fn follow_checks(&mut self, dir: &str, known: &[[u8; 32]]) {
        let now = win::ticks();
        let session = &mut self.session;
        for added in self.job.added() {
            let Check::Running { ppm, moved } = added.check else { continue };
            added.check = match session.status(added.id) {
                Some(libtorrent::Status { error: Some(error), .. }) => {
                    log!("torrent: its files cannot be checked: {error}");
                    Check::Failed
                }
                Some(libtorrent::Status { checking: Some(at), .. }) if at != ppm => {
                    Check::Running { ppm: at, moved: now }
                }
                Some(libtorrent::Status { checking: Some(_), .. }) if now.saturating_sub(moved) < CHECK_STALL_MS => continue,
                Some(libtorrent::Status { checking: Some(_), .. }) | None => {
                    log!("torrent: the check of its files stopped moving");
                    Check::Failed
                }
                Some(_) => {
                    if let Some(done) = session.file_progress(added.id, added.files.len()) {
                        record_checked(added, &done, dir, known);
                    }
                    Check::Done
                }
            };
        }
    }

    /// Rebuilds the signed torrent from the discovered item and the swarm's
    /// info.
    fn metadata(&mut self, id: TorrentId) {
        let Job::Discovering(Discovery::Fetching { newest, item, id: of }) = &self.job else { return };
        if *of != id {
            return;
        }
        let (newest, signature) = (*newest, item.signature);
        let made = self
            .session
            .info(id)
            .and_then(|info| newest::torrent(&info, &newest, &signature))
            .filter(|bytes| torrent::parse(bytes).is_ok());
        let Some(bytes) = made else {
            log!("torrent: revision {}'s info from its swarm is not one this updater reads", newest.revision);
            return;
        };
        log!("torrent: revision {}'s info came from its swarm", newest.revision);
        self.session.remove(id);
        self.job = Job::Discovering(Discovery::Found { revision: u64::from(newest.revision), bytes });
    }

    /// Kept when the release signed it, of the floor or higher, and the
    /// highest yet.
    fn dht_item(&mut self, item: Item) {
        let Job::Discovering(Discovery::Asking { floor, best, .. }) = &mut self.job else { return };
        let Some(key) = crate::signed::key() else { return };
        let Some(newest) = newest::verify(&item, &key) else {
            log!("torrent: a DHT item not signed by the release: ignored");
            return;
        };
        if u64::from(newest.revision) >= *floor && best.as_ref().is_none_or(|(had, _, _)| had.revision < newest.revision) {
            log!("torrent: the DHT names revision {}, signed by the release", newest.revision);
            *best = Some((newest, item, win::ticks()));
        }
    }

    fn dht_stored(&mut self, nodes: i64) {
        if let Job::Sharing { put: Some(put), .. } = &mut self.job
            && nodes > 0
        {
            put.state = PutState::Stored(win::ticks());
        }
        log!("torrent: the newest release's item stored on {nodes} DHT nodes");
    }
}

impl Torrents {
    pub fn new(settings: Torrenting, layout: &Layout) -> Self {
        let (port, on) = match settings {
            Torrenting::Off { port } => (port, false),
            Torrenting::On { port } => (port, true),
        };
        let _ = win::create_folders(&layout.torrent_dir);
        // An earlier build's state, beside the updater, goes.
        for name in OLD_FILES {
            let _ = win::delete(&format!("{}\\{name}", layout.exe_dir));
        }
        for name in OLD_FOLDERS {
            let _ = std::fs::remove_dir_all(format!("{}\\{name}", layout.exe_dir));
        }
        let mut torrents = Self {
            mode: Mode::Off { port },
            dir: layout.exe_dir.clone(),
            state: layout.torrent_dir.clone(),
            kept_port: format!("{}\\{PORT_FILE}", layout.torrent_dir),
            rate: upload::CAP,
            offered: Offered::Unread,
            banned: Vec::new(),
            known: Vec::new(),
            sent: 0,
            turn: Turn(0),
        };
        if on {
            torrents.turn_on(port);
        }
        torrents
    }

    /// The launcher's port; else the kept one; else any (then kept).
    fn want(&self, port: Option<u16>) -> Want {
        match port {
            Some(port) => Want::Named(port),
            None => kept_port_number(&self.kept_port).map_or(Want::Any, Want::Kept),
        }
    }

    /// `false`: the DLL did not load or make a session; torrent stays off.
    fn turn_on(&mut self, port: Option<u16>) -> bool {
        let want = self.want(port);
        let config = libtorrent::Config {
            port: want.port(),
            upload_limit: self.rate,
            dht_bootstrap: dht_bootstrap(),
            loopback: test_build(),
        };
        match libtorrent::Session::new(&self.dir, &config) {
            Ok(mut session) => {
                log!("torrent: allowed (libtorrent {}), port {want:?}", session.version());
                for ip in &self.banned {
                    session.ban(ip);
                }
                self.mode = Mode::On {
                    port,
                    engine: Box::new(Engine {
                        session,
                        listen: Listen::Asked { want, answer: false },
                        job: Job::Idle,
                    }),
                };
                true
            }
            Err(why) => {
                log!("torrent: {why}: HTTPS only");
                self.mode = Mode::Off { port };
                false
            }
        }
    }

    pub fn turn(&self) -> Turn {
        self.turn
    }

    /// After the overlay changed the folder: the torrent is read again.
    pub fn forget(&mut self) {
        self.offered = Offered::Unread;
    }

    fn engine(&mut self) -> Option<&mut Engine> {
        match &mut self.mode {
            Mode::On { engine, .. } => Some(engine),
            Mode::Off { .. } | Mode::Closed => None,
        }
    }

    /// The answer due now; a port asked while on is answered by [`service`]
    /// once listened on.
    pub fn on_message(&mut self, message: FromLauncher) -> Option<ToLauncher<'static>> {
        match message {
            FromLauncher::AllowTorrent(true) => {
                if let Mode::Off { port } = self.mode {
                    log!("torrent: allowed by the launcher");
                    if self.turn_on(port) {
                        self.turn = Turn(self.turn.0.wrapping_add(1));
                    }
                }
                None
            }
            FromLauncher::AllowTorrent(false) => {
                if let Mode::On { port, .. } = self.mode {
                    log!("torrent: not allowed by the launcher: stopped");
                    self.stop();
                    self.mode = Mode::Off { port };
                }
                None
            }
            FromLauncher::Port(number) => {
                let wanted = updater_logic::pipe::port_number(&format!("{number}"));
                let want = self.want(wanted);
                match &mut self.mode {
                    Mode::On { port, engine } => {
                        *port = wanted;
                        // The same port again: libtorrent says nothing.
                        match engine.listen {
                            Listen::Listening(at) if want.port().is_none_or(|wanted| wanted == at) => {
                                return Some(ToLauncher::PortOk);
                            }
                            Listen::Busy(at) if want.port().is_some() && want.port() == at => {
                                return Some(ToLauncher::PortBusy);
                            }
                            _ => {}
                        }
                        engine.session.listen(want.port());
                        engine.listen = Listen::Asked { want, answer: true };
                        None
                    }
                    // Nothing listens until torrent is allowed.
                    Mode::Off { port } => {
                        *port = wanted;
                        Some(ToLauncher::PortOk)
                    }
                    Mode::Closed => None,
                }
            }
            FromLauncher::UploadLimit(limit) => {
                // The cap is fixed: the launcher's limit may only lower it.
                self.rate = upload::rate(limit);
                log!("torrent: upload limit {limit:?} bytes a second asked: {} allowed", self.rate);
                let rate = self.rate;
                if let Some(engine) = self.engine() {
                    engine.session.set_upload_limit(rate);
                }
                None
            }
            _ => None,
        }
    }

    pub fn allowed(&self) -> bool {
        matches!(self.mode, Mode::On { .. })
    }

    pub fn sharing(&self) -> bool {
        matches!(&self.mode, Mode::On { engine, .. } if matches!(engine.job, Job::Sharing { .. }))
    }

    /// Stops what the session does (its port stays).
    pub fn stop(&mut self) {
        self.set_job(Job::Idle);
    }

    /// Lets go of the port for good, before another updater listens on it.
    pub fn close(&mut self) {
        self.stop();
        self.mode = Mode::Closed;
    }

    /// The old job's torrents are removed, their upload counted.
    fn set_job(&mut self, job: Job) {
        let Mode::On { engine, .. } = &mut self.mode else { return };
        let old = core::mem::replace(&mut engine.job, job);
        let mut gone: Vec<TorrentId> = Vec::new();
        match old {
            Job::Idle | Job::Discovering(Discovery::Asking { .. } | Discovery::Found { .. }) => {}
            Job::Fetching(added) | Job::Checking(added) => gone.push(added.id),
            Job::Discovering(Discovery::Fetching { id, .. }) => gone.push(id),
            Job::Sharing { swarm, bootstrap, .. } => gone.extend(swarm.iter().chain(&bootstrap).map(|added| added.id)),
        }
        for id in gone {
            self.sent += engine.session.status(id).map_or(0, |status| status.uploaded);
            engine.session.remove(id);
        }
    }

    /// Handles the session's events; returns the port answer if one is due.
    /// Called at every look at the pipe.
    pub fn service(&mut self) -> Option<ToLauncher<'static>> {
        let events = self.engine().map(|engine| engine.session.poll(0))?;
        let mut answer = None;
        for event in events {
            answer = self.on(event).or(answer);
        }
        if let Mode::On { engine, .. } = &mut self.mode {
            engine.follow_checks(&self.state, &self.known);
        }
        self.put_due();
        answer
    }

    fn on(&mut self, event: Event) -> Option<ToLauncher<'static>> {
        let Mode::On { engine, .. } = &mut self.mode else { return None };
        match event {
            Event::Listening(port) => return engine.listening(port, &self.kept_port),
            Event::ListenFailed { port, why } => return engine.listen_failed(port, &why),
            Event::HashFailed { piece } => log!("torrent: piece {piece} failed its hash: fetched again"),
            Event::Banned { ip } => {
                if !self.banned.contains(&ip) {
                    log!("torrent: {ip} sent a piece that failed: banned for this run");
                    engine.session.ban(&ip);
                    self.banned.push(ip);
                }
            }
            Event::Metadata(id) => engine.metadata(id),
            Event::Error { text } => log!("torrent: {text}"),
            Event::DhtItem(item) => engine.dht_item(item),
            Event::DhtPut { nodes } => engine.dht_stored(nodes),
        }
        None
    }

    fn put_due(&mut self) {
        let Some(key) = crate::signed::key() else { return };
        let Some(engine) = self.engine() else { return };
        let Job::Sharing { put: Some(put), .. } = &mut engine.job else { return };
        let now = win::ticks();
        let due = match put.state {
            PutState::Due => true,
            PutState::Sent(at) => now.saturating_sub(at) >= PUT_RETRY_MS,
            PutState::Stored(at) => now.saturating_sub(at) >= PUT_AGAIN_MS,
        };
        if !due {
            return;
        }
        put.state = PutState::Sent(now);
        log!("torrent: the item of revision {} put on the DHT", put.item.seq);
        engine.session.dht_put(&key, newest::SALT, &put.item);
    }

    /// The run's torrent ([`trusted`]) while torrent is allowed; read once a
    /// run.
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
                    torrent.pieces,
                    torrent.piece_length
                );
                crate::signed::taken(layout, signed.revision);
                // Before a bootstrap check writes the record, which drops
                // unknown torrents.
                self.know(signed.torrent.v2);
                let torrent = Arc::new(signed);
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

    /// The record keeps the checks of known torrents only.
    fn know(&mut self, v2: [u8; 32]) {
        if !self.known.contains(&v2) {
            self.known.push(v2);
        }
    }

    /// `files`: (file index, path, role).
    fn add(&mut self, bytes: &[u8], torrent: &Torrent, files: Vec<(usize, String, Role)>) -> Result<Added, String> {
        let nowhere = format!("{}\\{NOWHERE}", self.state);
        self.know(torrent.v2);
        let Some(engine) = self.engine() else {
            return Err("torrent is not allowed".into());
        };
        let placed: Vec<Placed> = files
            .iter()
            .map(|(index, path, role)| Placed {
                path: path.clone(),
                size: torrent.files[*index].size,
                role: *role,
                identity: win::identity(path).ok(),
            })
            .collect();
        let specs: Vec<FileSpec> = files
            .iter()
            .map(|(index, path, role)| FileSpec {
                name: &torrent.files[*index].path,
                path,
                role: *role,
            })
            .collect();
        let id = engine.session.add(bytes, &nowhere, &specs, &trackers())?;
        Ok(Added {
            id,
            v2: torrent.v2,
            piece_length: torrent.piece_length,
            files: placed,
            check: Check::Running { ppm: 0, moved: win::ticks() },
        })
    }

    /// Trusted (not hashed again) when the record says it was checked as it
    /// is.
    fn shared_roles(&self, torrent: &Torrent, files: &[(usize, String)]) -> Vec<(usize, String, Role)> {
        let record = read_record(&self.state);
        files
            .iter()
            .map(|(index, path)| {
                let trusted = win::identity(path).is_ok_and(|identity| record.trusted(&torrent.v2, path, &identity));
                (*index, path.clone(), Role::Share { trusted })
            })
            .collect()
    }

    /// [`Torrents::checked_files`] gives the result.
    fn start_check(&mut self, bytes: &[u8], torrent: &Torrent, files: &[(usize, String)]) -> bool {
        let roles = self.shared_roles(torrent, files);
        self.stop();
        match self.add(bytes, torrent, roles) {
            Ok(added) => {
                self.set_job(Job::Checking(added));
                true
            }
            Err(why) => {
                log!("torrent: cannot check its files: {why}");
                false
            }
        }
    }

    /// Whether each file is whole; `Ok(None)`: still running; `Err`: not
    /// checking any more (torrent turned off, the check failed).
    fn checked_files(&mut self) -> Result<Option<Vec<bool>>, ()> {
        let Some(engine) = self.engine() else { return Err(()) };
        let Job::Checking(added) = &engine.job else { return Err(()) };
        match added.check {
            Check::Running { .. } => return Ok(None),
            Check::Failed => {
                self.stop();
                return Err(());
            }
            Check::Done => {}
        }
        let (id, count) = (added.id, added.files.len());
        let sizes: Vec<u64> = added.files.iter().map(|file| file.size).collect();
        let whole = engine
            .session
            .file_progress(id, count)
            .map(|done| done.iter().zip(&sizes).map(|(done, size)| done == size).collect());
        self.stop();
        whole.map(Some).ok_or(())
    }

    /// The whole ones of `files`, as [`whole`] but waiting on `alive`
    /// instead of the pipe: for the bootstrap's few small files.
    fn whole_now(
        &mut self,
        bytes: &[u8],
        torrent: &Torrent,
        files: Vec<(usize, String)>,
        alive: &mut dyn FnMut() -> bool,
    ) -> Option<Vec<(usize, String)>> {
        if files.is_empty() {
            return Some(files);
        }
        if !self.start_check(bytes, torrent, &files) {
            return None;
        }
        let whole = loop {
            self.service();
            match self.checked_files() {
                Ok(Some(whole)) => break whole,
                Ok(None) if alive() => win::sleep(50),
                Ok(None) => {
                    self.stop();
                    return None;
                }
                Err(()) => return None,
            }
        };
        Some(files.into_iter().zip(whole).filter(|(_, whole)| *whole).map(|(file, _)| file).collect())
    }

    /// `files`: (file index, part).
    fn start_fetch(&mut self, signed: &Signed, files: &[(usize, String)]) -> Result<(), String> {
        self.stop();
        for (index, part) in files {
            // A longer part (left by another torrent's file of that name) is
            // cut, so a finished part is the file byte for byte.
            let size = signed.torrent.files[*index].size;
            if win::file_size(part).is_ok_and(|had| had > size) {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(part)
                    .and_then(|file| file.set_len(size))
                    .map_err(|error| format!("cannot cut {part} to its size: {error}"))?;
            }
        }
        let roles = files.iter().map(|(index, part)| (*index, part.clone(), Role::Fetch)).collect();
        let added = self.add(&signed.bytes, &signed.torrent, roles)?;
        self.set_job(Job::Fetching(added));
        Ok(())
    }

    /// The fetch at a look; `None`: not fetching any more.
    fn fetching(&mut self) -> Option<Progress> {
        let engine = self.engine()?;
        let Job::Fetching(added) = &engine.job else { return None };
        let total: u64 = added.files.iter().map(|file| file.size).sum();
        let (id, check) = (added.id, &added.check);
        if let Check::Failed = check {
            return Some(Progress::Failed("the parts could not be checked".into()));
        }
        let checked = matches!(check, Check::Done);
        let count = added.files.len();
        let status = engine.session.status(id)?;
        // Whole pieces that passed their hash only: the status counts blocks
        // not yet hashed, which a lying peer can send.
        let hashed: u64 = engine.session.file_progress(id, count)?.iter().sum();
        Some(match status {
            libtorrent::Status { error: Some(error), .. } => Progress::Failed(error),
            libtorrent::Status { checking: Some(ppm), .. } => Progress::Checking {
                done: (u128::from(total) * u128::from(ppm) / 1_000_000) as u64,
                total,
            },
            _ if !checked => Progress::Checking { done: 0, total },
            status => Progress::Running { done: hashed, total, peers: status.peers },
        })
    }

    /// Shares every file of the run's torrent present at its size, the
    /// bootstrap's kept copies, and the signed item on the DHT. `alive`: the
    /// launcher or the game is still there.
    pub fn share(&mut self, layout: &Layout, alive: &mut dyn FnMut() -> bool) {
        if self.sharing() || !self.allowed() {
            return;
        }
        let signed = self.offered(layout);
        let bootstrap = self.bootstrap(layout, alive);
        let mut swarm = None;
        let mut put = None;
        if let Some(signed) = &signed {
            let files: Vec<(usize, String)> = signed
                .torrent
                .files
                .iter()
                .enumerate()
                .map(|(index, file)| (index, layout.place(&file.path)))
                .filter(|(index, place)| {
                    let size = signed.torrent.files[*index].size;
                    size > 0 && win::file_size(place).ok() == Some(size)
                })
                .collect();
            let roles = self.shared_roles(&signed.torrent, &files);
            let trusted = roles.iter().filter(|(_, _, role)| *role == Role::Share { trusted: true }).count();
            log!(
                "torrent: sharing {} of its {} files ({trusted} checked before)",
                files.len(),
                signed.torrent.files.len()
            );
            swarm = self
                .add(&signed.bytes, &signed.torrent, roles)
                .map_err(|why| log!("torrent: cannot share it: {why}"))
                .ok();
            put = crate::signed::key()
                .and_then(|key| newest::of_torrent(&signed.bytes).filter(|item| newest::verify(item, &key).is_some()))
                .map(|item| Put { item, state: PutState::Due });
        }
        let bootstrap = bootstrap.and_then(|Bootstrap { bytes, torrent, copies: files }| {
            log!("torrent: sharing the bootstrap's {} of {} files", files.len(), torrent.files.len());
            let roles = self.shared_roles(&torrent, &files);
            self.add(&bytes, &torrent, roles)
                .map_err(|why| log!("torrent: cannot share the bootstrap: {why}"))
                .ok()
        });
        if swarm.is_none() && bootstrap.is_none() {
            return;
        }
        self.set_job(Job::Sharing { swarm, bootstrap, put });
        self.put_due();
    }

    /// Copies of `Skhid-get.bat`'s files are kept, made while the installed
    /// file still matches, because the updater replaces itself every release.
    fn bootstrap(&mut self, layout: &Layout, alive: &mut dyn FnMut() -> bool) -> Option<Bootstrap> {
        let (bytes, torrent) = installed_bootstrap(layout)?;
        let kept = format!("{}\\{KEPT}", layout.torrent_dir);
        let copy = |file: usize| {
            let name = torrent.files[file].path.rsplit('/').next().unwrap_or_default();
            format!("{kept}\\{name}")
        };
        // A copy of the right size can be anything: every piece checked.
        let copies = (0..torrent.files.len()).map(|file| (file, copy(file))).filter(|(_, path)| win::exists(path)).collect();
        let mut ready = self.whole_now(&bytes, &torrent, copies, alive)?;
        let lives = (0..torrent.files.len())
            .filter(|file| !ready.iter().any(|(ready, _)| ready == file))
            .map(|file| (file, layout.place(&torrent.files[file].path)))
            .filter(|(file, path)| win::file_size(path).ok() == Some(torrent.files[*file].size))
            .collect();
        for (file, live) in self.whole_now(&bytes, &torrent, lives, alive)? {
            let name = &torrent.files[file].path;
            match keep_copy(&live, &copy(file), &kept) {
                Ok(()) => {
                    log!("torrent: the bootstrap's {name} kept in torrent\\{KEPT}");
                    ready.push((file, copy(file)));
                }
                Err(why) => log!("torrent: the bootstrap's {name} cannot be kept: {why}"),
            }
        }
        ready.sort();
        (!ready.is_empty()).then_some(Bootstrap { bytes, torrent, copies: ready })
    }

    fn sharing_now(&mut self) -> Sharing {
        let rate = self.rate;
        let mut sent = self.sent;
        let mut peers = 0;
        if let Some(engine) = self.engine()
            && let Job::Sharing { swarm, bootstrap, .. } = &engine.job
        {
            let ids: Vec<TorrentId> = swarm.iter().chain(bootstrap).map(|added| added.id).collect();
            for id in ids {
                if let Some(status) = engine.session.status(id) {
                    sent += status.uploaded;
                    peers += status.peers as usize;
                }
            }
        }
        Sharing { sent, rate, peers }
    }

    fn start_discovery(&mut self, floor: u64) {
        self.stop();
        self.set_job(Job::Discovering(Discovery::Asking { floor, asked: 0, best: None }));
    }

    /// `true` while it goes on; `until`: the current step's deadline.
    fn discovering(&mut self, until: &mut u64) -> bool {
        let Some(key) = crate::signed::key() else { return false };
        let nowhere = format!("{}\\{NOWHERE}", self.state);
        let Some(engine) = self.engine() else { return false };
        let now = win::ticks();
        let Job::Discovering(discovery) = &mut engine.job else { return false };
        match discovery {
            Discovery::Asking { best: Some((newest, item, at)), .. } if now.saturating_sub(*at) >= HIGHER_MS || now >= *until => {
                let (newest, item) = (*newest, item.clone());
                match engine.session.add_magnet(&newest.v2, &nowhere, &trackers()) {
                    Ok(id) => {
                        log!("torrent: revision {}'s info asked of its swarm", newest.revision);
                        *discovery = Discovery::Fetching { newest, item, id };
                        *until = now + METADATA_MS;
                        true
                    }
                    Err(why) => {
                        log!("torrent: revision {}'s info cannot be asked for: {why}", newest.revision);
                        false
                    }
                }
            }
            Discovery::Asking { .. } if now >= *until => {
                log!("torrent: the DHT named no newer release in {} s", NEWEST_MS / 1000);
                false
            }
            Discovery::Asking { asked, .. } => {
                if *asked == 0 || now.saturating_sub(*asked) >= ASK_AGAIN_MS {
                    *asked = now;
                    engine.session.dht_get(&key, newest::SALT);
                }
                true
            }
            Discovery::Fetching { newest, .. } if now >= *until => {
                log!("torrent: revision {}'s info did not come from its swarm in {} s", newest.revision, METADATA_MS / 1000);
                false
            }
            Discovery::Fetching { .. } => true,
            Discovery::Found { .. } => false,
        }
    }

    fn discovered(&mut self) -> Option<(u64, Vec<u8>)> {
        let engine = self.engine()?;
        let Job::Discovering(Discovery::Found { revision, bytes }) = &mut engine.job else { return None };
        Some((*revision, core::mem::take(bytes)))
    }
}

/// Records the shared files found whole and unchanged. Fetched parts are not
/// recorded: checked against their pin and renamed, they are checked again.
fn record_checked(added: &Added, done: &[u64], dir: &str, known: &[[u8; 32]]) {
    if added.files.iter().all(|file| file.role == Role::Fetch) {
        let pieces = |size: u64| size.div_ceil(u64::from(added.piece_length));
        let wanted: u64 = added.files.iter().map(|file| pieces(file.size)).sum();
        let here: u64 = added
            .files
            .iter()
            .zip(done)
            .map(|(file, done)| if *done == file.size { pieces(file.size) } else { done / u64::from(added.piece_length) })
            .sum();
        log!("torrent: {here} of the {wanted} pieces to fetch are in the parts already, checked");
        return;
    }
    let checked: Vec<(String, Option<Identity>)> = added
        .files
        .iter()
        .zip(done)
        .filter(|(file, _)| matches!(file.role, Role::Share { .. }))
        .map(|(file, done)| {
            // Whole, and not changed while it was checked.
            let now = win::identity(&file.path).ok().filter(|now| *done == file.size && Some(*now) == file.identity);
            (file.path.clone(), now)
        })
        .collect();
    let whole = checked.iter().filter(|(_, identity)| identity.is_some()).count();
    log!("torrent: {whole} of its {} files checked whole", added.files.len());
    let mut record = read_record(dir);
    record.set(&added.v2, checked, known);
    let path = format!("{dir}\\{CHECKED}");
    let staged = format!("{path}.skhid-new");
    if let Err(code) = win::write_file(&staged, &shared::write_record(&record)).and_then(|()| win::rename(&staged, &path, true)) {
        log!("torrent: torrent\\{CHECKED} cannot be written: {}", win::error_text(code));
    }
}

fn read_record(dir: &str) -> Record {
    win::read_file(&format!("{dir}\\{CHECKED}"), 4 << 20)
        .ok()
        .and_then(|bytes| shared::parse_record(&bytes))
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy)]
pub struct Sharing {
    /// Bytes sent in the run, by every torrent.
    pub sent: u64,
    /// The run's upload cap, bytes a second.
    pub rate: u64,
    pub peers: usize,
}

/// Why [`share_while`] shared nothing.
#[derive(Debug)]
pub enum NotShared {
    /// Likely another updater that shares.
    PortBusy,
    /// No trusted torrent, no bootstrap, or no engine.
    Nothing,
}

/// A taken port is tried again this long: the updater sharing on it may be
/// closing.
const PORT_WAIT_MS: u64 = 10_000;
/// Sharing is started again this often, should it have stopped.
const RESHARE_MS: u64 = 10_000;

/// For `crate::seed`: shares on `port` until `go_on` says stop (asked ten
/// times a second); fetches and installs nothing.
pub fn share_while(layout: &Layout, port: Option<u16>, go_on: &mut dyn FnMut(&Sharing) -> bool) -> Result<(), NotShared> {
    // The launcher's updater's kept port, waited for: never a second one.
    let port = port.or_else(|| kept_port_number(&format!("{}\\{PORT_FILE}", layout.torrent_dir)));
    let until = win::ticks() + PORT_WAIT_MS;
    let mut torrents = loop {
        let torrents = Torrents::new(Torrenting::On { port }, layout);
        if !torrents.allowed() {
            return Err(NotShared::Nothing);
        }
        let mut torrents = torrents;
        let listening = loop {
            torrents.service();
            match torrents.engine().map(|engine| &engine.listen) {
                Some(Listen::Listening(_)) => break true,
                Some(Listen::Asked { .. }) if win::ticks() < until => win::sleep(50),
                Some(Listen::Asked { .. } | Listen::Busy(_)) | None => break false,
            }
        };
        if listening {
            break torrents;
        }
        drop(torrents);
        if win::ticks() >= until {
            return Err(NotShared::PortBusy);
        }
        win::sleep(1000);
    };
    let share = |torrents: &mut Torrents, go_on: &mut dyn FnMut(&Sharing) -> bool| {
        let before = torrents.sharing_now();
        torrents.share(layout, &mut || go_on(&before));
    };
    share(&mut torrents, go_on);
    if !torrents.sharing() {
        return Err(NotShared::Nothing);
    }
    let mut shared = win::ticks();
    loop {
        torrents.service();
        if !go_on(&torrents.sharing_now()) {
            break;
        }
        if win::ticks().saturating_sub(shared) >= RESHARE_MS {
            shared = win::ticks();
            share(&mut torrents, go_on);
        }
        win::sleep(100);
    }
    torrents.close();
    Ok(())
}

/// From the players' ZIP, in `game\torrent`.
const FILE: &str = "skhid.torrent";
/// The newest one the DHT named ([`discover`]).
const FOUND: &str = "found.torrent";

/// `name` in `game\torrent`, when the release signed it.
fn signed(layout: &Layout, name: &str) -> Result<Signed, String> {
    let path = format!("{}\\{name}", layout.torrent_dir);
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
    Ok(Signed { torrent, revision, bytes })
}

/// The run's torrent: the higher revision of the ZIP's and the DHT's, each
/// trusted by its signature alone, whoever handed it over.
fn trusted(layout: &Layout) -> Result<Signed, String> {
    let zip = signed(layout, FILE);
    let found = signed(layout, FOUND);
    match (zip, found) {
        (Ok(zip), Ok(found)) => Ok(if found.revision > zip.revision { found } else { zip }),
        (Ok(one), Err(_)) | (Err(_), Ok(one)) => Ok(one),
        (Err(why), Err(_)) => Err(why),
    }
}

pub fn offered(session: &mut Session, layout: &Layout) -> Option<Arc<Signed>> {
    session.torrents.offered(layout)
}

/// Whether an HTTPS download of `path`, started at `since`, hands over to
/// torrent: it was turned on since and the run's torrent has the file. The
/// bytes HTTPS got stay; torrent checks them first.
pub fn takes_over(session: &mut Session, layout: &Layout, since: Turn, path: &str) -> bool {
    !session.stopped
        && session.torrents.turn() != since
        && offered(session, layout).is_some_and(|offered| file_index(&offered.torrent, path).is_some())
}

pub enum Fetched {
    /// Every file is in its part, every piece checked.
    Done,
    /// No peer, a stall, torrent turned off, a file error: why.
    Not(String),
    Stopped,
}

/// `files`: (file index, part). Gives up, for HTTPS, when no peer comes or
/// pieces stop coming.
pub fn fetch(session: &mut Session, signed: &Signed, files: &[(usize, String)]) -> Fetched {
    if let Err(why) = session.torrents.start_fetch(signed, files) {
        return Fetched::Not(why);
    }
    let mut shown = None;
    // From the check's end: (started, last progress, bytes then).
    let mut running: Option<(u64, u64, u64)> = None;
    let outcome = loop {
        if !session.pump() {
            break Fetched::Stopped;
        }
        let Some(progress) = session.torrents.fetching() else {
            break Fetched::Not("torrent was turned off".into());
        };
        let now = win::ticks();
        match progress {
            Progress::Checking { done, total } => {
                if shown != Some(0) {
                    shown = Some(0);
                    session.begin_progress();
                    session.state(updater_logic::pipe::state::TORRENT_CHECKING);
                }
                session.progress(done, total);
            }
            Progress::Running { done, total, peers } => {
                if shown != Some(1) {
                    shown = Some(1);
                    session.begin_progress();
                    session.state(updater_logic::pipe::state::DOWNLOADING);
                }
                session.progress(done, total);
                if done >= total {
                    break Fetched::Done;
                }
                let (started, progressed, had) = running.get_or_insert((now, now, done));
                if done > *had {
                    *had = done;
                    *progressed = now;
                }
                if peers == 0 && now.saturating_sub(*started) > NO_PEER_MS {
                    break Fetched::Not(format!("no peer in {} s", NO_PEER_MS / 1000));
                }
                if now.saturating_sub(*progressed) > STALL_MS {
                    break Fetched::Not(format!("no piece for {} s", STALL_MS / 1000));
                }
            }
            Progress::Failed(why) => break Fetched::Not(why),
        }
        win::sleep(100);
    };
    session.torrents.stop();
    outcome
}

/// Whether each of `files` (index, path) is whole, every piece hashed unless
/// the record says it was checked as it is now. `None`: torrent off or
/// stopped.
pub fn whole(session: &mut Session, signed: &Signed, files: &[(usize, String)]) -> Option<Vec<bool>> {
    if files.is_empty() {
        return Some(Vec::new());
    }
    if !session.torrents.start_check(&signed.bytes, &signed.torrent, files) {
        return None;
    }
    loop {
        if !session.pump() {
            session.torrents.stop();
            return None;
        }
        match session.torrents.checked_files() {
            Ok(Some(whole)) => return Some(whole),
            Ok(None) => win::sleep(100),
            Err(()) => return None,
        }
    }
}

/// From the ZIP, in `game\torrent`.
const BOOTSTRAP: &str = "skhid-bootstrap.torrent";

/// Unsigned, so taken only as the checked ZIP installed it; the script that
/// fetches its files pins them.
fn installed_bootstrap(layout: &Layout) -> Option<(Vec<u8>, Torrent)> {
    let name = format!("game/torrent/{BOOTSTRAP}");
    let installed = crate::read_installed(layout)?;
    let entry = installed.files.iter().find(|file| file.name == name)?;
    let bytes = win::read_file(&layout.place(&name), MAX_TORRENT).ok()?;
    if updater_logic::sha256::digest(&bytes) != entry.sha256 {
        return None;
    }
    let torrent = torrent::parse(&bytes).ok()?;
    Some((bytes, torrent))
}

fn keep_copy(live: &str, copy: &str, kept: &str) -> Result<(), String> {
    let staged = format!("{copy}.skhid-new");
    win::create_folders(kept).map_err(win::error_text)?;
    std::fs::copy(live, &staged).map_err(|error| format!("{error}"))?;
    win::rename(&staged, copy, true).map_err(win::error_text)
}

/// In `game\torrent`: the bootstrap's kept files.
const KEPT: &str = "bootstrap";

/// For when the update server cannot be reached: a newer signed torrent
/// from the DHT item (`updater_logic::newest`) and its swarm, kept as
/// [`FOUND`] for [`trusted`].
pub fn discover(session: &mut Session, layout: &Layout) {
    if !session.torrents.allowed() || crate::signed::key().is_none() {
        return;
    }
    let signed = offered(session, layout);
    let floor = match &signed {
        Some(signed) => signed.revision + 1,
        None => crate::signed::installed(layout).max(1),
    };
    log!("torrent: the DHT asked for a release of revision {floor} or newer");
    session.torrents.start_discovery(floor);
    let mut until = win::ticks() + NEWEST_MS;
    while session.pump() && session.torrents.discovering(&mut until) {
        win::sleep(100);
    }
    let found = session.torrents.discovered();
    session.torrents.stop();
    let Some((revision, bytes)) = found else { return };
    keep(layout, revision, &bytes);
    session.torrents.forget();
}

fn keep(layout: &Layout, revision: u64, bytes: &[u8]) {
    let path = format!("{}\\{FOUND}", layout.torrent_dir);
    if win::read_file(&path, MAX_TORRENT).is_ok_and(|kept| kept == bytes) {
        return;
    }
    let staged = format!("{path}.skhid-new");
    match win::write_file(&staged, bytes).and_then(|()| win::rename(&staged, &path, true)) {
        Ok(()) => log!("torrent: revision {revision} from the DHT and its swarm kept as {FOUND}"),
        Err(code) => log!("torrent: {FOUND} cannot be written: {}", win::error_text(code)),
    }
}

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

/// Once the server answers, its archive is installed: the Skhid files
/// fetched by torrent while it could not be reached go.
pub fn remove_skhid_parts(layout: &Layout) {
    let Ok(signed) = trusted(layout) else { return };
    for index in skhid_files(&signed.torrent) {
        let part = skhid_part(layout, &signed.torrent.files[index].path);
        if win::exists(&part) && win::delete(&part).is_ok() {
            log!("removed the left-over torrent download: {part}");
        }
    }
}
