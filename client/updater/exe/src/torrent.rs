//! BitTorrent, when the launcher's "Allow torrent" is on
//! (`srp-026g/docs/notes/torrent-downloads.md`): the revision's torrent
//! (signed by the release: [`trusted`]) as an extra source of the original
//! game's files and of missing patches, and the sharing of every file of it
//! this folder has, while the updater runs.
//!
//! The engine is libtorrent, in `skhid_torrent.dll` beside the updater
//! (`crate::libtorrent`, its one owner): no DLL, or one that does not load,
//! and torrent stays off, HTTPS as without it. [`Torrents`] is the one owner
//! of the session: the launcher's settings, the port, the upload cap, and
//! what the session does ([`Job`]: one at a time), driven by its events
//! ([`Torrents::service`], from the update's every look at the pipe).
//!
//! Peers come from this updater's own trackers ([`trackers`]: never a
//! torrent's, which are outside its signature) and the DHT; no LSD, PEX,
//! UPnP, NAT-PMP or uTP. A peer that sent a piece that failed its hash is
//! banned for the updater's run.

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

/// A download by torrent is left (for HTTPS) when no peer is connected
/// this long after its check, or no piece came for this long.
const NO_PEER_MS: u64 = 20_000;
const STALL_MS: u64 = 60_000;
/// How long the DHT is asked for the newest release's item, and how long
/// its torrent's info is then waited for from its swarm.
const NEWEST_MS: u64 = 20_000;
const METADATA_MS: u64 = 60_000;
/// The DHT is asked again this often meanwhile; once an item came, a higher
/// one is waited for this long.
const ASK_AGAIN_MS: u64 = 5_000;
const HIGHER_MS: u64 = 3_000;
/// The signed item is put again this often while it shares (DHT nodes
/// forget an item in about two hours).
const PUT_AGAIN_MS: u64 = 30 * 60_000;
/// The DHT's way in.
const DHT_BOOTSTRAP: &str =
    "dht.libtorrent.org:25401,router.bittorrent.com:6881,router.utorrent.com:6881,dht.transmissionbt.com:6881";

/// Beside the updater: what was checked ([`updater_logic::shared`]), and a
/// folder where nothing of a torrent is (the files a session neither
/// fetches nor shares are named there, so that nothing is read of them).
const CHECKED: &str = "updater_skhid.torrent.checked";
const NOWHERE: &str = "updater_skhid.torrent-none";

/// A test build (`SKHID_TEST_TRACKERS`): 127.0.0.1 only, its DHT
/// (`SKHID_TEST_DHT`, else none to be found).
fn test_build() -> bool {
    option_env!("SKHID_TEST_TRACKERS").is_some()
}

fn dht_bootstrap() -> &'static str {
    match option_env!("SKHID_TEST_DHT") {
        Some(nodes) => nodes,
        None if test_build() => "127.0.0.1:1",
        None => DHT_BOOTSTRAP,
    }
}

/// The release's trackers (`release/build_torrent.py` TRACKERS).
const TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "http://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://exodus.desync.com:6969/announce",
];

/// The trackers of every torrent: the release's, never a torrent's (its
/// trackers are outside the signature: whoever handed it over could name
/// its own, and be the only one to give peers). A test build has its own
/// (`SKHID_TEST_TRACKERS`, space-separated; `tests/test_torrent.py`).
fn trackers() -> Vec<String> {
    match option_env!("SKHID_TEST_TRACKERS") {
        Some(list) => list.split_whitespace().map(String::from).collect(),
        None => TRACKERS.iter().map(|text| String::from(*text)).collect(),
    }
}

/// A torrent signed by the release: what it is, its revision, its bytes
/// (`torrent::canonical`: what the signature covers, and the piece layers).
pub struct Signed {
    pub torrent: Torrent,
    pub revision: u64,
    bytes: Vec<u8>,
}

/// The revision's torrent as this run knows it.
enum Offered {
    /// Not read yet (or read before the overlay changed the folder).
    Unread,
    Read(Arc<Signed>),
    /// None, or none this updater takes: HTTPS only this run.
    None,
}

/// The times the launcher turned torrent on in this run (`1A`): what a
/// download by HTTPS or the watch compares, to see it happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn(u32);

/// The launcher's torrent settings and the session that runs them.
pub struct Torrents {
    mode: Mode,
    /// Beside the updater: the DLL, the record.
    dir: String,
    /// [`PORT_FILE`] beside the updater.
    kept_port: String,
    /// Bytes a second, all the sharing together (`updater_logic::upload`).
    rate: u64,
    offered: Offered,
    /// The addresses banned in this run, for every session of it.
    banned: Vec<String>,
    /// The torrents of this run (their v2 info hashes): the record keeps
    /// theirs.
    known: Vec<[u8; 32]>,
    /// Bytes sent by the torrents of this run that are gone.
    sent: u64,
    turn: Turn,
}

enum Mode {
    /// `port`: the launcher's, kept for when it allows torrent.
    Off { port: Option<u16> },
    On { port: Option<u16>, engine: Box<Engine> },
    /// For good: another updater runs now, on the same port.
    Closed,
}

/// The libtorrent session and what it does.
struct Engine {
    session: libtorrent::Session,
    listen: Listen,
    job: Job,
}

/// The session's port.
enum Listen {
    /// Asked for; `answer`: the launcher waits for the answer (`21`).
    Asked { want: Want, answer: bool },
    Listening(u16),
    Busy(Option<u16>),
}

/// The port asked of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    /// The launcher's (busy: busy).
    Named(u16),
    /// The one kept beside the updater ([`PORT_FILE`]; busy: any).
    Kept(u16),
    /// Any: the one the system picks is kept.
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

/// The port listened on when the launcher names none, beside the updater:
/// the one picked the first time, so that peers that knew it find it again.
const PORT_FILE: &str = "updater_skhid.port";

/// The port kept in `kept` ([`PORT_FILE`]), if it holds one.
fn kept_port_number(kept: &str) -> Option<u16> {
    let text = win::read_file(kept, 16).ok()?;
    updater_logic::pipe::port_number(core::str::from_utf8(&text).ok()?.trim())
}

/// What the session does: one thing at a time.
enum Job {
    Idle,
    /// Files into their parts.
    Fetching(Added),
    /// Whether files are a torrent's, every piece (nothing after).
    Checking(Added),
    /// The newest release, from the DHT and its swarm.
    Discovering(Discovery),
    /// The folder's files of the revision's torrent, the bootstrap's kept
    /// copies, and the signed item on the DHT.
    Sharing {
        swarm: Option<Added>,
        bootstrap: Option<Added>,
        put: Option<Put>,
    },
}

/// A torrent added to the session.
struct Added {
    id: TorrentId,
    v2: [u8; 32],
    piece_length: u32,
    files: Vec<Placed>,
    checked: bool,
}

/// A file of an added torrent: where it is, what it is to the session, and
/// (shared) what Windows said of it when it was added.
struct Placed {
    path: String,
    size: u64,
    role: Role,
    identity: Option<Identity>,
}

/// The signed item this updater puts on the DHT, and when it last did.
struct Put {
    item: Item,
    at: Option<u64>,
}

enum Discovery {
    /// Asking the DHT; the best item that verified, newer than `floor`.
    Asking {
        floor: u64,
        asked: u64,
        best: Option<(Newest, Item, u64)>,
    },
    /// Its torrent's info from the swarm.
    Fetching { newest: Newest, item: Item, id: TorrentId },
    /// The signed torrent, made again.
    Found { revision: u64, bytes: Vec<u8> },
}

/// The bootstrap torrent to share, and its kept copies (its file, the copy).
struct Bootstrap {
    bytes: Vec<u8>,
    torrent: Torrent,
    copies: Vec<(usize, String)>,
}

/// What a fetch looks like at a look ([`fetch`]).
enum Progress {
    Checking { done: u64, total: u64 },
    Running { done: u64, total: u64, peers: u32 },
    Failed(String),
}

impl Torrents {
    /// The launcher's settings, for the updater of `layout`.
    pub fn new(settings: Torrenting, layout: &Layout) -> Self {
        let mut torrents = Self {
            mode: Mode::Off { port: None },
            dir: layout.exe_dir.clone(),
            kept_port: format!("{}\\{PORT_FILE}", layout.exe_dir),
            rate: upload::CAP,
            offered: Offered::Unread,
            banned: Vec::new(),
            known: Vec::new(),
            sent: 0,
            turn: Turn(0),
        };
        match settings {
            Torrenting::Off { port } => torrents.mode = Mode::Off { port },
            Torrenting::On { port } => {
                torrents.mode = Mode::Off { port };
                torrents.turn_on(port);
            }
        }
        torrents
    }

    /// What to listen on: the launcher's port when it names one; else the
    /// one kept; else any (then kept).
    fn want(&self, port: Option<u16>) -> Want {
        match port {
            Some(port) => Want::Named(port),
            None => kept_port_number(&self.kept_port).map_or(Want::Any, Want::Kept),
        }
    }

    /// On, when the DLL loads and makes a session; else it stays off.
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

    /// The folder changed (the overlay): the torrent is read again.
    pub fn forget(&mut self) {
        self.offered = Offered::Unread;
    }

    fn engine(&mut self) -> Option<&mut Engine> {
        match &mut self.mode {
            Mode::On { engine, .. } => Some(engine),
            Mode::Off { .. } | Mode::Closed => None,
        }
    }

    /// One of the launcher's torrent messages; what to answer it now (a
    /// port asked while on is answered once it is listened on: [`service`]).
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
                        // Listened on already: libtorrent says nothing again.
                        if let Listen::Listening(at) = engine.listen
                            && want.port().is_none_or(|wanted| wanted == at)
                        {
                            return Some(ToLauncher::PortOk);
                        }
                        engine.session.listen(want.port());
                        engine.listen = Listen::Asked { want, answer: true };
                        None
                    }
                    // Off: nothing listens, so any port is "ok"; it is the
                    // one listened on once torrent is allowed.
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

    /// Stops the session and lets go of the port, for good: before another
    /// updater runs, which listens on the same port.
    pub fn close(&mut self) {
        self.stop();
        self.mode = Mode::Closed;
    }

    /// The job from now on; the torrents of the one before go.
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

    /// What the session said since the last look: the port's answer for
    /// the launcher, if one is due. Called at every look at the pipe.
    pub fn service(&mut self) -> Option<ToLauncher<'static>> {
        let events = self.engine().map(|engine| engine.session.poll(0))?;
        let mut answer = None;
        for event in events {
            answer = self.on(event).or(answer);
        }
        self.put_due();
        answer
    }

    /// One event of the session.
    fn on(&mut self, event: Event) -> Option<ToLauncher<'static>> {
        let Mode::On { engine, .. } = &mut self.mode else { return None };
        match event {
            Event::Listening(port) => {
                let answer = matches!(engine.listen, Listen::Asked { answer: true, .. });
                if !matches!(engine.listen, Listen::Listening(at) if at == port) {
                    log!("torrent: listening on port {port}");
                }
                if let Listen::Asked { want: Want::Any, .. } = engine.listen {
                    match win::write_file(&self.kept_port, format!("{port}\r\n").as_bytes()) {
                        Ok(()) => log!("torrent: port {port} picked, kept in {PORT_FILE}"),
                        Err(code) => log!("torrent: port {port} picked; {PORT_FILE}: {}", win::error_text(code)),
                    }
                }
                engine.listen = Listen::Listening(port);
                return answer.then_some(ToLauncher::PortOk);
            }
            Event::ListenFailed { port, why } => {
                log!("torrent: port {port} cannot be listened on ({why})");
                let (asked, answer) = match engine.listen {
                    // The kept one is another program's now: any, kept.
                    Listen::Asked { want: Want::Kept(_), answer } => {
                        engine.session.listen(None);
                        engine.listen = Listen::Asked { want: Want::Any, answer };
                        return None;
                    }
                    Listen::Asked { want, answer } => (want.port(), answer),
                    Listen::Listening(port) => (Some(port), false),
                    Listen::Busy(port) => (port, false),
                };
                engine.listen = Listen::Busy(asked);
                return answer.then_some(ToLauncher::PortBusy);
            }
            Event::HashFailed { piece } => log!("torrent: piece {piece} failed its hash: fetched again"),
            Event::Banned { ip } => {
                if !self.banned.contains(&ip) {
                    log!("torrent: {ip} sent a piece that failed: banned for this run");
                    engine.session.ban(&ip);
                    self.banned.push(ip);
                }
            }
            Event::Checked(id) => {
                let dir = self.dir.clone();
                let known = self.known.clone();
                let Engine { session, job, .. } = &mut **engine;
                let added = match job {
                    Job::Fetching(added) | Job::Checking(added) => Some(added),
                    Job::Sharing { swarm, bootstrap, .. } => swarm.iter_mut().chain(bootstrap.iter_mut()).find(|added| added.id == id),
                    _ => None,
                };
                if let Some(added) = added.filter(|added| added.id == id) {
                    added.checked = true;
                    checked(session, added, &dir, &known);
                }
            }
            Event::DhtPut { nodes: 0 } => {}
            Event::Metadata(id) => {
                if let Job::Discovering(Discovery::Fetching { newest, item, id: of }) = &engine.job
                    && *of == id
                {
                    let (newest, item) = (*newest, item.clone());
                    let made = engine
                        .session
                        .info(id)
                        .and_then(|info| newest::torrent(&info, &newest, &item.signature))
                        .filter(|bytes| torrent::parse(bytes).is_ok());
                    match made {
                        Some(bytes) => {
                            log!("torrent: revision {}'s info came from its swarm", newest.revision);
                            engine.session.remove(id);
                            engine.job = Job::Discovering(Discovery::Found {
                                revision: u64::from(newest.revision),
                                bytes,
                            });
                        }
                        None => log!("torrent: revision {}'s info from its swarm is not one this updater reads", newest.revision),
                    }
                }
            }
            Event::Error { text } => log!("torrent: {text}"),
            Event::DhtItem(item) => {
                if let Job::Discovering(Discovery::Asking { floor, best, .. }) = &mut engine.job
                    && let Some(key) = crate::signed::key()
                {
                    match newest::verify(&item, &key) {
                        Some(newest) if u64::from(newest.revision) >= *floor => {
                            if best.as_ref().is_none_or(|(had, _, _)| had.revision < newest.revision) {
                                log!("torrent: the DHT names revision {}, signed by the release", newest.revision);
                                *best = Some((newest, item, win::ticks()));
                            }
                        }
                        Some(_) => {}
                        None => log!("torrent: a DHT item not signed by the release: ignored"),
                    }
                }
            }
            Event::DhtPut { nodes } => log!("torrent: the newest release's item stored on {nodes} DHT nodes"),
        }
        None
    }

    /// The signed item put on the DHT again, when it is time.
    fn put_due(&mut self) {
        let Some(key) = crate::signed::key() else { return };
        let Some(engine) = self.engine() else { return };
        let Job::Sharing { put: Some(put), .. } = &mut engine.job else { return };
        let now = win::ticks();
        if put.at.is_some_and(|at| now.saturating_sub(at) < PUT_AGAIN_MS) {
            return;
        }
        put.at = Some(now);
        log!("torrent: the item of revision {} put on the DHT", put.item.seq);
        engine.session.dht_put(&key, newest::SALT, &put.item);
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
                    torrent.pieces,
                    torrent.piece_length
                );
                crate::signed::taken(layout, signed.revision);
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

    /// Adds `bytes` (a torrent) with `files` (its file, where, how).
    fn add(&mut self, bytes: &[u8], torrent: &Torrent, files: Vec<(usize, String, Role)>) -> Result<Added, String> {
        let nowhere = format!("{}\\{NOWHERE}", self.dir);
        if !self.known.contains(&torrent.v2) {
            self.known.push(torrent.v2);
        }
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
            checked: false,
        })
    }

    /// `files` (the torrent's file, where it is) as shared: each trusted
    /// when the record says it was checked as it is.
    fn shared_roles(&self, torrent: &Torrent, files: &[(usize, String)]) -> Vec<(usize, String, Role)> {
        let record = read_record(&self.dir);
        files
            .iter()
            .map(|(index, path)| {
                let trusted = win::identity(path).is_ok_and(|identity| record.trusted(&torrent.v2, path, &identity));
                (*index, path.clone(), Role::Share { trusted })
            })
            .collect()
    }

    /// Starts checking `files` of `signed`: [`Torrents::checked_files`]
    /// says when it is done.
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

    /// The check's answer once done: whether each file is whole. `Err`:
    /// not checking any more (torrent turned off).
    fn checked_files(&mut self) -> Result<Option<Vec<bool>>, ()> {
        let Some(engine) = self.engine() else { return Err(()) };
        let Job::Checking(added) = &engine.job else { return Err(()) };
        if !added.checked {
            return Ok(None);
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

    /// [`whole`] without the launcher's pipe: the bootstrap's few small
    /// files.
    fn whole_now(&mut self, bytes: &[u8], torrent: &Torrent, files: &[(usize, String)]) -> Option<Vec<bool>> {
        if !self.start_check(bytes, torrent, files) {
            return None;
        }
        loop {
            self.service();
            match self.checked_files() {
                Ok(Some(whole)) => return Some(whole),
                Ok(None) => win::sleep(50),
                Err(()) => return None,
            }
        }
    }

    /// Starts fetching `files` (the torrent's file, its part).
    fn start_fetch(&mut self, signed: &Signed, files: &[(usize, String)]) -> Result<(), String> {
        self.stop();
        for (index, part) in files {
            // A part longer than the file (left by another torrent's file of
            // that name): what is past its size goes, so that a finished
            // part is the file, byte for byte.
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
        let (id, checked) = (added.id, added.checked);
        let status = engine.session.status(id)?;
        Some(match status {
            libtorrent::Status { error: Some(error), .. } => Progress::Failed(error),
            libtorrent::Status { checking: Some(ppm), .. } => Progress::Checking {
                done: (u128::from(total) * u128::from(ppm) / 1_000_000) as u64,
                total,
            },
            _ if !checked => Progress::Checking { done: 0, total },
            status => Progress::Running {
                done: status.wanted_done,
                total: status.wanted,
                peers: status.peers,
            },
        })
    }

    /// Shares every file of the torrent this folder has with the torrent's
    /// size, however it came (the torrent, HTTPS, the ZIP), and the
    /// bootstrap's kept copies, and puts the release's signed item on the
    /// DHT: while the launcher stays open. Nothing when torrent is not
    /// allowed, or it shares already.
    pub fn share(&mut self, layout: &Layout) {
        if self.sharing() || !self.allowed() {
            return;
        }
        let signed = self.offered(layout);
        let bootstrap = self.bootstrap(layout);
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
                .map(|item| Put { item, at: None });
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

    /// The bootstrap torrent (`Skhid-get.bat`'s: the updater and 7zr.exe at
    /// one version, a file of the ZIP beside the updater) and its files'
    /// kept copies to share: each made from the file in its place while
    /// that is the bootstrap's. Taken only as the checked ZIP installed it
    /// (it is not signed: the script that fetches it pins it).
    fn bootstrap(&mut self, layout: &Layout) -> Option<Bootstrap> {
        let name = format!("game/binaries/x86/{BOOTSTRAP}");
        let installed = crate::read_installed(layout)?;
        let entry = installed.files.iter().find(|file| file.name == name)?;
        let bytes = win::read_file(&layout.place(&name), MAX_TORRENT).ok()?;
        if updater_logic::sha256::digest(&bytes) != entry.sha256 {
            return None;
        }
        let torrent = torrent::parse(&bytes).ok()?;
        let kept = format!("{}\\{KEPT}", layout.exe_dir);
        let copy = |file: usize| {
            let name = torrent.files[file].path.rsplit('/').next().unwrap_or_default();
            format!("{kept}\\{name}")
        };
        let all: Vec<usize> = (0..torrent.files.len()).collect();
        // A copy is shared once every piece of it is checked (a copy of the
        // right size can be anything); else it is made again.
        let copies: Vec<(usize, String)> = all.iter().map(|&file| (file, copy(file))).filter(|(_, path)| win::exists(path)).collect();
        let whole = if copies.is_empty() { Vec::new() } else { self.whole_now(&bytes, &torrent, &copies)? };
        let mut ready: Vec<(usize, String)> = copies.into_iter().zip(whole).filter(|(_, whole)| *whole).map(|(copy, _)| copy).collect();
        let lives: Vec<(usize, String)> = all
            .iter()
            .filter(|file| !ready.iter().any(|(ready, _)| ready == *file))
            .map(|&file| (file, layout.place(&torrent.files[file].path)))
            .filter(|(file, path)| win::file_size(path).ok() == Some(torrent.files[*file].size))
            .collect();
        let whole = if lives.is_empty() { Vec::new() } else { self.whole_now(&bytes, &torrent, &lives)? };
        for ((file, live), _) in lives.into_iter().zip(whole).filter(|(_, whole)| *whole) {
            let copy = copy(file);
            let staged = format!("{copy}.skhid-new");
            let copied = win::create_folders(&kept)
                .map_err(win::error_text)
                .and_then(|()| std::fs::copy(&live, &staged).map(|_| ()).map_err(|error| format!("{error}")))
                .and_then(|()| win::rename(&staged, &copy, true).map_err(win::error_text));
            match copied {
                Ok(()) => {
                    log!("torrent: the bootstrap's {} kept in {KEPT}", torrent.files[file].path);
                    ready.push((file, copy));
                }
                Err(why) => log!("torrent: the bootstrap's {} cannot be kept: {why}", torrent.files[file].path),
            }
        }
        ready.sort();
        (!ready.is_empty()).then_some(Bootstrap { bytes, torrent, copies: ready })
    }

    /// What the sharing is now.
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

    /// Starts asking the DHT for an item newer than `floor`.
    fn start_discovery(&mut self, floor: u64) {
        self.stop();
        self.set_job(Job::Discovering(Discovery::Asking { floor, asked: 0, best: None }));
    }

    /// The discovery's next step at a look; `true` while it goes on.
    fn discovering(&mut self, until: &mut u64) -> bool {
        let Some(key) = crate::signed::key() else { return false };
        let nowhere = format!("{}\\{NOWHERE}", self.dir);
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

    /// The torrent the discovery made, when it did.
    fn discovered(&mut self) -> Option<(u64, Vec<u8>)> {
        let engine = self.engine()?;
        let Job::Discovering(Discovery::Found { revision, bytes }) = &mut engine.job else { return None };
        Some((*revision, core::mem::take(bytes)))
    }
}

/// The files of `added` it found whole, every piece, as Windows says they
/// still are: the record's (shared ones; a fetched part is checked
/// against its pin, then renamed: it is checked again where it is).
fn checked(session: &mut libtorrent::Session, added: &Added, dir: &str, known: &[[u8; 32]]) {
    let Some(done) = session.file_progress(added.id, added.files.len()) else { return };
    if added.files.iter().all(|file| file.role == Role::Fetch) {
        let pieces = |size: u64| size.div_ceil(u64::from(added.piece_length));
        let wanted: u64 = added.files.iter().map(|file| pieces(file.size)).sum();
        let here: u64 = added
            .files
            .iter()
            .zip(&done)
            .map(|(file, done)| if *done == file.size { pieces(file.size) } else { done / u64::from(added.piece_length) })
            .sum();
        log!("torrent: {here} of the {wanted} pieces to fetch are in the parts already, checked");
        return;
    }
    let whole: Vec<(String, Identity)> = added
        .files
        .iter()
        .zip(&done)
        .filter(|(file, done)| matches!(file.role, Role::Share { .. }) && **done == file.size)
        .filter_map(|(file, _)| {
            let now = win::identity(&file.path).ok()?;
            (Some(now) == file.identity).then(|| (file.path.clone(), now))
        })
        .collect();
    log!("torrent: {} of its {} files checked whole", whole.len(), added.files.len());
    let mut record = read_record(dir);
    record.set(&added.v2, whole, known);
    let path = format!("{dir}\\{CHECKED}");
    let staged = format!("{path}.skhid-new");
    if let Err(code) = win::write_file(&staged, &shared::write_record(&record)).and_then(|()| win::rename(&staged, &path, true)) {
        log!("torrent: {CHECKED} cannot be written: {}", win::error_text(code));
    }
}

fn read_record(dir: &str) -> Record {
    win::read_file(&format!("{dir}\\{CHECKED}"), 4 << 20)
        .ok()
        .and_then(|bytes| shared::parse_record(&bytes))
        .unwrap_or_default()
}

/// What the sharing is at a look ([`share_while`]).
#[derive(Debug, Clone, Copy)]
pub struct Sharing {
    /// Bytes sent in the run, by every torrent.
    pub sent: u64,
    /// What the run's cap lets go, bytes a second.
    pub rate: u64,
    /// Peers connected.
    pub peers: usize,
}

/// Why [`share_while`] shared nothing.
#[derive(Debug)]
pub enum NotShared {
    /// The port is another program's (another updater that shares).
    PortBusy,
    /// No torrent this updater takes, no bootstrap, or no engine: nothing
    /// to share.
    Nothing,
}

/// How long a port another program holds is tried again: an updater that
/// shared on it may be closing.
const PORT_WAIT_MS: u64 = 10_000;
/// How often the sharing is looked at again: the session's job started
/// again should it have stopped.
const RESHARE_MS: u64 = 10_000;

/// The game's sharing (`crate::seed`), in one call: every file of the
/// folder's torrent, as [`Torrents::share`] shares it while the launcher is
/// open, on `port`, within the run's cap, until `go_on` says stop (asked
/// ten times a second, with what the sharing is). Nothing is fetched or
/// installed.
pub fn share_while(layout: &Layout, port: Option<u16>, go_on: &mut dyn FnMut(&Sharing) -> bool) -> Result<(), NotShared> {
    // The port the launcher's updater shares on too, when one is kept: the
    // same, waited for, never a second one beside it.
    let port = port.or_else(|| kept_port_number(&format!("{}\\{PORT_FILE}", layout.exe_dir)));
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
                Some(Listen::Busy(_)) | None => break false,
                Some(Listen::Asked { .. }) => win::sleep(50),
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
    torrents.share(layout);
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
            torrents.share(layout);
        }
        win::sleep(100);
    }
    torrents.close();
    Ok(())
}

/// The torrent beside the updater, a file of the players' ZIP.
const FILE: &str = "skhid.torrent";
/// The newest one the DHT named ([`discover`]), beside it.
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
    Ok(Signed { torrent, revision, bytes })
}

/// Where the revision's torrent comes from, and why it is trusted: the one
/// place that decides it. Trusted by its signature alone
/// (`crate::signed`: the release's key, a revision not older than the
/// game's), whoever handed it over: the ZIP's file beside the updater, or
/// the newest one the DHT named. The highest revision of them.
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
        && offered(session, layout).is_some_and(|offered| file_index(&offered.torrent, path).is_some())
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
    if let Err(why) = session.torrents.start_fetch(signed, files) {
        return Fetched::Not(why);
    }
    let mut shown = None;
    // Since the check: when, and when the bytes last grew.
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

/// Whether each of `files` (the torrent's file, where it is) is that file
/// of the torrent, every piece (libtorrent checks them; a file the record
/// says was checked as it is now is not read again). `None`: not checked
/// (torrent off or stopped).
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

/// The bootstrap torrent, a file of the ZIP beside the updater.
const BOOTSTRAP: &str = "skhid-bootstrap.torrent";
/// Where its files are kept, beside the updater: the updater replaces
/// itself at every release, and the bootstrap's bytes are shared all the
/// same.
const KEPT: &str = "updater_skhid.bootstrap";

/// Looks for a newer signed torrent than this updater has, on the DHT (the
/// release's item, `updater_logic::newest`) and then its swarm: when the
/// update server cannot be reached (a blocked network, our servers down, a
/// bare folder). The torrent made again from the item and the swarm's info
/// is kept beside the updater ([`FOUND`]) and is the run's torrent from
/// then (once [`trusted`] takes it, by its signature).
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

/// A newer torrent the DHT named, kept beside the updater ([`FOUND`]).
fn keep(layout: &Layout, revision: u64, bytes: &[u8]) {
    let path = format!("{}\\{FOUND}", layout.exe_dir);
    if win::read_file(&path, MAX_TORRENT).is_ok_and(|kept| kept == bytes) {
        return;
    }
    let staged = format!("{path}.skhid-new");
    match win::write_file(&staged, bytes).and_then(|()| win::rename(&staged, &path, true)) {
        Ok(()) => log!("torrent: revision {revision} from the DHT and its swarm kept as {FOUND}"),
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
