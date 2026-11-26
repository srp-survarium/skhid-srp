//! The world host link of one match.
//!
//! The world host (`srp-world-host`, a separate daemon) runs the 0.69d exe's own world code on the match's DF stream and
//! tells the relay what only a world knows: the end of a PvE mission (the extraction, every human dead), the winner, the
//! mission time and the per-player counters ([`world_host_proto`]). Without a host, or with a host that failed, lags or
//! diverged, the match runs as a plain relay: the link is `Off` and nothing in the relay depends on it.
//!
//! **IO.** The hub mutex never touches a socket. Per connection a writer thread connects, writes the frames of the
//! match's [`FrameLog`] from its own cursor (the log is append-only; the hub only appends under a short lock) and starts a
//! reader thread that decodes the host's frames into a bounded channel. [`HostLink::poll`] (inside `Match::poll`, after
//! that poll's DFs went out) only drains that channel and compares counters.
//!
//! **Recovery.** The log keeps `Create` and every DF of the match in memory. On EOF, a `Fault` after
//! `Ready`, a bad frame, a queue of more than `max_queue` unwritten frames while live, or no frame from the host for
//! `heartbeat_ms` of DFs, the link drops the connection and opens a new one (the zygote forks a new child), which gets
//! the whole log from the start and catches up. At most `max_restarts`; then `Off`. `Diverged` is never retried. No socket
//! (connect fails) or a `Fault` before `Ready` (the host refused the `Create`) is `Off` at once. A late `Ready` is not
//! fatal for a match with a given world: it never waits for the host.
//!
//! **Start world (protocol 4).** A `Create` with an empty world asks the host to build the start world; its `Ready`
//! brings it ([`Note::Start`]) and the match waits for it (relay.rs `on_start_world`; a link that goes `Off` before it
//! ends the match, `EndReason::NoWorld`). The log's `Create` is then rewritten with that world, so a restarted child
//! loads exactly the bytes the clients loaded.
//!
//! **End.** The fin DFs (every recipient's leave bit) are never appended; `Close{t_last}` names the
//! last normal DF. The host's `Outcome` is latched once. `Final` (after `Close`) ends the connection.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use world_host_proto as proto;

/// A connected stream to the host (a unix socket in production, one end of a socket pair in tests).
pub trait HostStream: Read + Write + Send {
    fn try_clone_stream(&self) -> io::Result<Box<dyn HostStream>>;
    /// Shut both directions down (unblocks a reader or writer on another clone).
    fn shutdown_both(&self);
}

#[cfg(unix)]
impl HostStream for std::os::unix::net::UnixStream {
    fn try_clone_stream(&self) -> io::Result<Box<dyn HostStream>> {
        Ok(Box::new(self.try_clone()?))
    }

    fn shutdown_both(&self) {
        let _ = self.shutdown(std::net::Shutdown::Both);
    }
}

/// Opens one new connection to the host (one per match, and one more per restart).
pub type Connector = Arc<dyn Fn() -> io::Result<Box<dyn HostStream>> + Send + Sync>;

/// The world host link: its connector and limits.
#[derive(Clone)]
pub struct HostConfig {
    /// for logs: the socket path
    pub label: String,
    pub connector: Connector,
    /// No frame from the host for this many ms of DFs (the `Stats` heartbeat comes every 5 s of world time): restart.
    pub heartbeat_ms: u32,
    /// More unwritten frames than this while live: the host lags, restart.
    pub max_queue: usize,
    /// Restarts (catch-up from the log) per match; then `Off`.
    pub max_restarts: u32,
    /// The relay keeps its `MatchEnd` back this long for the host's `Final` (inside the 2 s drain).
    pub final_wait: Duration,
    /// A mirrored PvE failure end (cancel, 45-minute end) with a live host waits this long for the host's Outcome
    /// before the relay closes the clients (1 s: the host reads tick `c` as final after DF `c + 300`, which leaves the
    /// relay at `c + 350`). See `Match::mirror_hold`.
    pub mirror_wait: Duration,
    // Always on (no switch): phase 2 E0 = the host's per-tick hashes go out as E0 (its mask, per-player bytes and g bytes)
    // while the link is live, a CD's entries stay in one DF; phase 2 resync = a client's confirmed CC is answered with the
    // host's DE of its newest final tick (item + animation-history tail), to that client only (`Match::on_host_de`).
    /// The resync breaker: at most this many host DEs per client within `resync_window`; the next CC switches resync
    /// off for that client for `resync_cooldown` (rule 4). A CC with reason 1 (a DE did not
    /// land) trips it at once.
    pub resync_max: u32,
    pub resync_window: Duration,
    /// A tripped breaker switches resync off for that client this long, then on again with an empty window (not for the
    /// rest of the match).
    pub resync_cooldown: Duration,
    /// At most this many host DEs per client per match (2 scripted in a Normal / Hard PvE mission + network causes).
    pub resync_match_max: u32,
    /// A DE is asked only for a second CC above the first within this long (rule 2: stale world state repeats a CC every
    /// 500 ms of world time; a hash taken by the 500 ms timeout sends a single one, 0x2001AE449).
    pub resync_confirm: Duration,
    /// No DE at a time T in [S - this, S + tick) of a scripted step S that adds to an empty modifier record (rule 3,
    /// `Match::resync_steps`): the clients' load skips such a record, so a DE before S makes them re-apply the step.
    pub resync_step_guard_ms: u32,
    /// A `WantDe` without its `De` (or refusal) for this long is dropped.
    pub de_wait: Duration,
    /// The host reads a tick as final this far behind its newest stepped DF (its `final_lag_ms`): a DE of a
    /// time above the CC's tick needs DFs up to `tick + final_lag_ms + tick`.
    pub final_lag_ms: u32,
    /// The in-memory frame log (about 1 MB per 10 minutes of PvE) may not pass this; a link that would pass it goes
    /// `Off` (64 MiB: about 10 hours of PvE, far past any match's backstop).
    pub max_log_bytes: usize,
    // Host bots are always on (protocol 5): a match with bot slots asks the host to drive them
    // (`Create.drive_bots`); the host's `Bots` frames go into the DFs.
    /// The relay holds a DF at most this long past its time for the `Bots` of the previous step (30 ms): ai entries
    /// cannot go late.
    pub bots_hold_ms: u32,
}

impl std::fmt::Debug for HostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostConfig")
            .field("label", &self.label)
            .field("heartbeat_ms", &self.heartbeat_ms)
            .field("max_queue", &self.max_queue)
            .field("max_restarts", &self.max_restarts)
            .field("final_wait", &self.final_wait)
            .field("mirror_wait", &self.mirror_wait)
            .field("resync_max", &self.resync_max)
            .field("resync_window", &self.resync_window)
            .field("resync_cooldown", &self.resync_cooldown)
            .field("resync_match_max", &self.resync_match_max)
            .field("resync_confirm", &self.resync_confirm)
            .field("resync_step_guard_ms", &self.resync_step_guard_ms)
            .field("de_wait", &self.de_wait)
            .field("final_lag_ms", &self.final_lag_ms)
            .field("max_log_bytes", &self.max_log_bytes)
            .finish()
    }
}

impl HostConfig {
    /// The default limits over any connector.
    pub fn with_connector(label: &str, connector: Connector) -> Self {
        HostConfig {
            label: label.to_string(),
            connector,
            heartbeat_ms: 10_000,
            max_queue: 1024,
            max_restarts: 2,
            final_wait: Duration::from_millis(1500),
            mirror_wait: Duration::from_secs(1),
            resync_max: 3,
            resync_window: Duration::from_secs(60),
            resync_cooldown: Duration::from_secs(120),
            resync_match_max: 12,
            resync_confirm: Duration::from_millis(1500),
            resync_step_guard_ms: 500,
            de_wait: Duration::from_secs(2),
            final_lag_ms: 300,
            max_log_bytes: 64 << 20,
            bots_hold_ms: 30,
        }
    }

    /// The host daemon's unix socket (`world_host.socket` of the srp config).
    #[cfg(unix)]
    pub fn unix(path: &std::path::Path) -> Self {
        let p = path.to_path_buf();
        let connector: Connector = Arc::new(move || {
            let s = std::os::unix::net::UnixStream::connect(&p)?;
            Ok(Box::new(s) as Box<dyn HostStream>)
        });
        Self::with_connector(&path.display().to_string(), connector)
    }
}

/// Ask the world host which level projects it serves (`LIST_PROJECTS` -> `Projects` on a connection of its own; the
/// daemon takes them from the game data or its config's `[project]` sections). A daemon that does not know the query
/// answers `Fault` (an error here); no answer within `timeout` is an error too.
pub fn query_projects(h: &HostConfig, timeout: Duration) -> io::Result<Vec<String>> {
    let mut s = (h.connector)()?;
    proto::write_frame(&mut *s, proto::ty::LIST_PROJECTS, &[])?;
    let mut rd = s.try_clone_stream()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(proto::read_frame(&mut *rd));
    });
    let got = rx.recv_timeout(timeout);
    s.shutdown_both();
    match got {
        Ok(Ok(Some(f))) if f.ty == proto::ty::PROJECTS => Ok(proto::decode_projects(&f)?),
        Ok(Ok(Some(f))) => match proto::FromHost::decode(&f) {
            Ok(proto::FromHost::Fault { reason }) => Err(io::Error::other(format!("the world host refused the project list: {reason}"))),
            _ => Err(io::Error::other(format!("the world host answered the project list with frame {:#04x}", f.ty))),
        },
        Ok(Ok(None)) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the world host closed without a project list")),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "the world host did not answer the project list")),
    }
}

/// The link's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    /// first connection, no `Ready` yet
    Starting,
    /// `Ready` seen and every frame of the restart's backlog written
    Live,
    /// a restarted connection replays the log
    CatchingUp,
    /// no host: a plain relay
    Off,
    /// the host saw an input the clients treat differently: not used for anything
    Diverged,
    /// `Close` was sent and the connection is over (`Final` or not)
    Closed,
}

impl LinkState {
    pub fn name(self) -> &'static str {
        match self {
            LinkState::Starting => "starting",
            LinkState::Live => "live",
            LinkState::CatchingUp => "catching-up",
            LinkState::Off => "off",
            LinkState::Diverged => "diverged",
            LinkState::Closed => "closed",
        }
    }
}

/// What the match learns from one [`HostLink::poll`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Note {
    /// the host's world ended (state 4 / 5), latched once
    Outcome(proto::Outcome),
    /// the host's last word at `Close.t_last` (or an unsolicited one)
    Final(proto::Outcome),
    /// phase 2: the E0 bytes of one item (the relay sends E0 from it when E0 is on)
    Tick(proto::Tick),
    /// phase 2 resync: the host's DE for a `WantDe`
    De(proto::De),
    /// phase 2 resync: the host refused a `WantDe` (its `Fault "WantDe <id>: ..."`; the link stays up)
    DeRefused { request_id: u32, reason: String },
    /// protocol 4: the start world the host built (`Ready.start`) and its collision checksum, once
    Start { checksum: u32, item: Vec<u8> },
    /// protocol 5: the host's bot entries of one stepped DF, for DF `t_for` (only with `Create.drive_bots`)
    Bots(proto::bots::Bots),
}

/// Append-only frames of one match, shared with the writer threads.
struct FrameLog {
    inner: Mutex<LogInner>,
    cv: Condvar,
}

struct LogInner {
    frames: Vec<Arc<[u8]>>,
    /// the link is gone: writers exit
    done: bool,
}

impl FrameLog {
    fn len(&self) -> usize {
        self.inner.lock().unwrap().frames.len()
    }
}

enum Event {
    Connected,
    ConnectFailed(String),
    Msg(proto::FromHost),
    Bad(String),
    Eof,
    WriteFailed(String),
}

struct Conn {
    cancel: Arc<AtomicBool>,
    written: Arc<AtomicUsize>,
    /// a clone of the stream for the shutdown (filled by the writer after the connect)
    stream: Arc<Mutex<Option<Box<dyn HostStream>>>>,
    ready: bool,
    /// the relay's DF time when the last frame came from this connection (or when it was opened)
    heard_at: u32,
    /// log length when the connection was opened (a restart is live once its backlog is written)
    mark: usize,
    /// frames written at the last check, and the relay's DF time when that count last moved
    last_written: usize,
    progress_at: u32,
}

impl Conn {
    fn stop(&self, log: &FrameLog) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(s) = self.stream.lock().unwrap().take() {
            s.shutdown_both();
        }
        log.cv.notify_all();
    }
}

/// One match's link to the world host (sans socket IO; see the module docs).
pub(crate) struct HostLink {
    cfg: HostConfig,
    match_id: u32,
    state: LinkState,
    log: Arc<FrameLog>,
    log_bytes: usize,
    conn: Option<Conn>,
    epoch: u32,
    restarts: u32,
    tx: SyncSender<(u32, Event)>,
    rx: Receiver<(u32, Event)>,
    outcome: Option<proto::Outcome>,
    stats: Option<proto::Stats>,
    final_: Option<proto::Outcome>,
    /// version 3: the deaths of the `Kills` frames, by (t, victim), duplicates of a restarted child dropped
    kills: std::collections::BTreeMap<(u32, u8), proto::KillRecord>,
    /// a `Kills` frame arrived (the host's log is valid; an empty frame = no death)
    kills_seen: bool,
    /// version 6: the `Used` frame (what each human used), the last one before `Final`
    used: Option<proto::Used>,
    /// `Close.t_last`, once sent
    closed_at: Option<u32>,
    /// the relay's last normal DF
    last_df: u32,
    /// protocol 4: the `Create` without a world, until the first `Ready` brings the start world
    pending_create: Option<proto::Create>,
    /// protocol 5: the host drives the match's bots (`Create.drive_bots`)
    drive_bots: bool,
    /// the host reported `Diverged` while it drives bots: the link stays up for the bots' inputs, but its outcome, E0,
    /// resync DEs and final values are not used
    diverged: bool,
}

impl HostLink {
    /// Queue `Create` and open the first connection. `t0` = the world time of the DE (the DF clock's start).
    pub(crate) fn new(cfg: HostConfig, create: &proto::Create, t0: u32) -> Result<Self, proto::Error> {
        let frame = proto::ToHost::Create(create.clone()).frame()?;
        let (tx, rx) = std::sync::mpsc::sync_channel(4096);
        let log = Arc::new(FrameLog { inner: Mutex::new(LogInner { frames: vec![Arc::from(frame.as_slice())], done: false }), cv: Condvar::new() });
        let mut link = HostLink {
            cfg,
            match_id: create.match_id,
            state: LinkState::Starting,
            log,
            log_bytes: frame.len(),
            conn: None,
            epoch: 0,
            restarts: 0,
            tx,
            rx,
            outcome: None,
            stats: None,
            final_: None,
            kills: Default::default(),
            kills_seen: false,
            used: None,
            closed_at: None,
            last_df: t0,
            pending_create: create.world.is_empty().then(|| create.clone()),
            drive_bots: create.drive_bots,
            diverged: false,
        };
        log::info!("match {:#x}: world host {} attached ({} bytes Create, {} records)", link.match_id, link.cfg.label, frame.len(), create.records.len());
        link.open();
        Ok(link)
    }

    pub(crate) fn state(&self) -> LinkState {
        self.state
    }

    pub(crate) fn e0_enabled(&self) -> bool {
        !self.diverged
    }

    /// The host drives the match's bots (protocol 5).
    pub(crate) fn drives_bots(&self) -> bool {
        self.drive_bots
    }

    /// The host reported `Diverged` (with bots the link stays up; see `diverged`).
    pub(crate) fn diverged(&self) -> bool {
        self.diverged || self.state == LinkState::Diverged
    }

    /// Resync answers CCs (not after a divergence).
    pub(crate) fn resync_enabled(&self) -> bool {
        !self.diverged
    }

    pub(crate) fn cfg(&self) -> &HostConfig {
        &self.cfg
    }

    /// Phase 2 resync: ask for the DE of the host's newest final tick. Only while live.
    pub(crate) fn want_de(&mut self, request_id: u32) -> bool {
        if self.state != LinkState::Live || self.closed_at.is_some() {
            return false;
        }
        match (proto::ToHost::WantDe { request_id }).frame() {
            Ok(f) => {
                self.append(f);
                self.state == LinkState::Live
            }
            Err(e) => {
                log::error!("match {:#x}: WantDe not encodable: {e}", self.match_id);
                false
            }
        }
    }

    pub(crate) fn mirror_wait(&self) -> Duration {
        self.cfg.mirror_wait
    }

    pub(crate) fn final_wait(&self) -> Duration {
        self.cfg.final_wait
    }

    pub(crate) fn restarts(&self) -> u32 {
        self.restarts
    }

    /// The host is still in use (its outcome and final values count).
    pub(crate) fn active(&self) -> bool {
        matches!(self.state, LinkState::Starting | LinkState::Live | LinkState::CatchingUp)
    }

    pub(crate) fn outcome(&self) -> Option<&proto::Outcome> {
        if self.diverged { None } else { self.outcome.as_ref() }
    }

    pub(crate) fn final_outcome(&self) -> Option<&proto::Outcome> {
        if self.state == LinkState::Diverged || self.diverged { None } else { self.final_.as_ref() }
    }

    /// The deaths the host reported (`Kills`), in time order; None = no `Kills` frame came (no valid log).
    pub(crate) fn kills(&self) -> Option<Vec<proto::KillRecord>> {
        self.kills_seen.then(|| self.kills.values().copied().collect())
    }

    /// What each human used (`Used`, version 6), only together with a valid `Final` (a diverged host's world is not
    /// the clients'); None = no frame (an older host, or no Final).
    pub(crate) fn used(&self) -> Option<&proto::Used> {
        self.final_outcome().and(self.used.as_ref())
    }

    pub(crate) fn last_stats(&self) -> Option<&proto::Stats> {
        if self.state == LinkState::Diverged { None } else { self.stats.as_ref() }
    }

    /// `Close` was sent and nothing more will come (Final, EOF, failure, or the link is not used).
    /// `Close` went out (the relay's end, or a PvP end screen's early Close).
    pub(crate) fn closed(&self) -> bool {
        self.closed_at.is_some()
    }

    pub(crate) fn settled(&self) -> bool {
        !self.active() || self.final_.is_some()
    }

    fn append(&mut self, frame: Vec<u8>) {
        if self.log_bytes + frame.len() > self.cfg.max_log_bytes {
            // the writers read from the log, so a log that stops growing is a link that stops: memory stays bounded
            self.set_off(&format!("the frame log would pass {} bytes", self.cfg.max_log_bytes));
            return;
        }
        self.log_bytes += frame.len();
        let mut g = self.log.inner.lock().unwrap();
        g.frames.push(Arc::from(frame.as_slice()));
        drop(g);
        self.log.cv.notify_all();
    }

    /// One normal DF (never a fin DF), the recorder's unfiltered bytes.
    pub(crate) fn df(&mut self, t: u32, bytes: &[u8]) {
        self.last_df = t;
        // nothing after Close (a PvP end screen keeps the clients' DFs going after the host's Close)
        if !self.active() || self.closed_at.is_some() {
            return;
        }
        match (proto::ToHost::Df(proto::Df { t, bytes: bytes.to_vec() })).frame() {
            Ok(f) => self.append(f),
            Err(e) => self.fail(&format!("DF {t} not encodable: {e}")),
        }
    }

    /// The relay ends the match: `Close{t_last}`, the last normal DF.
    pub(crate) fn close(&mut self, t_last: u32) {
        if !self.active() || self.closed_at.is_some() {
            return;
        }
        self.closed_at = Some(t_last);
        match (proto::ToHost::Close { t_last }).frame() {
            Ok(f) => self.append(f),
            Err(e) => self.fail(&format!("Close not encodable: {e}")),
        }
        log::info!("match {:#x}: Close {t_last} to the world host ({})", self.match_id, self.state.name());
    }

    fn open(&mut self) {
        self.epoch += 1;
        let epoch = self.epoch;
        let cancel = Arc::new(AtomicBool::new(false));
        let written = Arc::new(AtomicUsize::new(0));
        let stream: Arc<Mutex<Option<Box<dyn HostStream>>>> = Arc::new(Mutex::new(None));
        let mark = self.log.len();
        let w = Writer { epoch, connector: self.cfg.connector.clone(), log: self.log.clone(), written: written.clone(), cancel: cancel.clone(), slot: stream.clone(), tx: self.tx.clone(), match_id: self.match_id };
        let spawned = std::thread::Builder::new().name(format!("host-w-{:08x}-{epoch}", self.match_id)).spawn(move || w.run());
        if let Err(e) = spawned {
            log::error!("match {:#x}: no world host writer thread: {e}", self.match_id);
            self.state = LinkState::Off;
            self.release();
            return;
        }
        self.conn = Some(Conn { cancel, written, stream, ready: false, heard_at: self.last_df, mark, last_written: 0, progress_at: self.last_df });
    }

    fn stop_conn(&mut self) {
        if let Some(c) = self.conn.take() {
            c.stop(&self.log);
        }
    }

    /// Off / Diverged / Closed: the connection ends and the log is freed.
    fn release(&mut self) {
        self.stop_conn();
        let mut g = self.log.inner.lock().unwrap();
        g.done = true;
        g.frames = Vec::new();
        drop(g);
        self.log.cv.notify_all();
        self.log_bytes = 0;
    }

    /// The relay cannot feed the host any more (a DF it could not encode): `Off`.
    pub(crate) fn give_up(&mut self, why: &str) {
        if self.active() {
            self.set_off(why);
        }
    }

    fn set_off(&mut self, why: &str) {
        log::warn!("match {:#x}: world host off ({why}): the match goes on without it", self.match_id);
        self.state = LinkState::Off;
        self.release();
    }

    /// A connection failed: catch-up restart or `Off`.
    fn fail(&mut self, why: &str) {
        if !self.active() {
            return;
        }
        if self.closed_at.is_some() {
            log::warn!("match {:#x}: world host gone after Close ({why}): no Final", self.match_id);
            self.state = LinkState::Closed;
            self.release();
            return;
        }
        if self.restarts >= self.cfg.max_restarts {
            self.set_off(&format!("{why}; {} restart(s) used", self.restarts));
            return;
        }
        self.stop_conn();
        self.restarts += 1;
        self.state = LinkState::CatchingUp;
        // a new child replays the log: the old WantDe frames would make it build DEs nobody waits for
        {
            let mut g = self.log.inner.lock().unwrap();
            let before = g.frames.len();
            g.frames.retain(|f| f.get(4) != Some(&proto::ty::WANT_DE));
            let dropped = before - g.frames.len();
            if dropped > 0 {
                self.log_bytes = g.frames.iter().map(|f| f.len()).sum();
                log::info!("match {:#x}: {dropped} WantDe frame(s) left out of the restart's replay", self.match_id);
            }
        }
        log::warn!(
            "match {:#x}: world host restart {}/{} ({why}): a new child gets Create and {} DF(s) from the start",
            self.match_id,
            self.restarts,
            self.cfg.max_restarts,
            self.log.len().saturating_sub(1)
        );
        self.open();
    }

    /// Drain the host's frames and check liveness and lag. Never blocks.
    pub(crate) fn poll(&mut self) -> Vec<Note> {
        let mut notes = Vec::new();
        for _ in 0..512 {
            let (epoch, ev) = match self.rx.try_recv() {
                Ok(x) => x,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            };
            if epoch != self.epoch || self.conn.is_none() || !self.active() {
                continue; // an old connection's last words
            }
            self.on_event(ev, &mut notes);
        }
        if !self.active() || self.closed_at.is_some() {
            return notes;
        }
        let last_df = self.last_df;
        let len = self.log.len();
        let Some(c) = self.conn.as_mut() else { return notes };
        let written = c.written.load(Ordering::SeqCst);
        let queued = len.saturating_sub(written);
        if written != c.last_written {
            c.last_written = written;
            c.progress_at = last_df;
        }
        let (ready, mark, heard_at, progress_at) = (c.ready, c.mark, c.heard_at, c.progress_at);
        if self.state == LinkState::CatchingUp && ready && written >= mark {
            log::info!("match {:#x}: world host caught up (restart {})", self.match_id, self.restarts);
            self.state = LinkState::Live;
        }
        let silent = last_df.saturating_sub(heard_at);
        let stuck = last_df.saturating_sub(progress_at);
        if silent >= self.cfg.heartbeat_ms {
            self.fail(&format!("no frame for {silent} ms of DFs"));
        } else if matches!(self.state, LinkState::Live | LinkState::Starting) && queued > self.cfg.max_queue {
            self.fail(&format!("{queued} frames unwritten: lagging"));
        } else if queued > 0 && stuck >= self.cfg.heartbeat_ms {
            // a catching-up child that sends its heartbeat but reads nothing (the queue rule waits for Live)
            self.fail(&format!("no frame taken for {stuck} ms of DFs ({queued} waiting)"));
        }
        notes
    }

    fn on_event(&mut self, ev: Event, notes: &mut Vec<Note>) {
        if let Event::Msg(_) = &ev {
            if let Some(c) = self.conn.as_mut() {
                c.heard_at = self.last_df;
            }
        }
        match ev {
            Event::Connected => log::info!("match {:#x}: world host connection {} open", self.match_id, self.epoch),
            Event::ConnectFailed(e) => self.set_off(&format!("no connection to {}: {e}", self.cfg.label)),
            Event::Eof => self.fail("the host closed the connection"),
            Event::WriteFailed(e) => self.fail(&format!("write: {e}")),
            Event::Bad(e) => self.fail(&format!("bad frame: {e}")),
            Event::Msg(m) => self.on_msg(m, notes),
        }
    }

    fn on_msg(&mut self, m: proto::FromHost, notes: &mut Vec<Note>) {
        use proto::FromHost as F;
        match m {
            F::Ready(r) => {
                if r.match_id != self.match_id {
                    self.fail(&format!("Ready for match {:#x}", r.match_id));
                    return;
                }
                log::info!(
                    "match {:#x}: world host ready (world time {}, collision checksum {:#x}, created in {} ms{})",
                    self.match_id,
                    r.world_time,
                    r.collision_checksum,
                    r.create_ms,
                    r.start.as_ref().map_or(String::new(), |b| format!(", start world {} bytes", b.len()))
                );
                if let Some(mut c) = self.pending_create.take() {
                    let Some(item) = r.start.filter(|b| !b.is_empty()) else {
                        self.set_off("Ready without the start world it was asked to build");
                        return;
                    };
                    // a restarted child gets this world in its Create: it loads the bytes the clients load
                    c.world = item.clone();
                    match proto::ToHost::Create(c).frame() {
                        Ok(f) => {
                            let mut g = self.log.inner.lock().unwrap();
                            if let Some(first) = g.frames.first_mut() {
                                self.log_bytes = self.log_bytes - first.len() + f.len();
                                *first = Arc::from(f.as_slice());
                            }
                        }
                        Err(e) => {
                            self.set_off(&format!("Create with the start world not encodable: {e}"));
                            return;
                        }
                    }
                    notes.push(Note::Start { checksum: r.collision_checksum, item });
                }
                if let Some(c) = self.conn.as_mut() {
                    c.ready = true;
                }
                if self.state == LinkState::Starting {
                    self.state = LinkState::Live;
                }
            }
            F::Stats(s) => self.stats = Some(s),
            // a diverged host's world is not the clients': nothing of it is used (its bots' inputs still are)
            F::Outcome(_) | F::Tick(_) | F::De(_) if self.diverged => {}
            F::Outcome(o) => {
                if self.outcome.is_some() {
                    // a restarted child reports the same end again: latched once
                    if self.outcome.as_ref() != Some(&o) {
                        log::warn!("match {:#x}: world host Outcome again, different: {o:?} (kept {:?})", self.match_id, self.outcome);
                    }
                    return;
                }
                if !matches!(o.state, 4 | 5) {
                    log::warn!("match {:#x}: world host Outcome with state {} ignored", self.match_id, o.state);
                    return;
                }
                log::info!(
                    "match {:#x}: world host Outcome at {}: state {}, winner team {}, PvE success {}, mission {} ms, {} player row(s)",
                    self.match_id,
                    o.t_end,
                    o.state,
                    o.winner_team,
                    o.pve_success,
                    o.mission_ms,
                    o.players.len()
                );
                self.outcome = Some(o.clone());
                notes.push(Note::Outcome(o));
            }
            F::Final(o) => {
                log::info!("match {:#x}: world host Final at {}: state {}, winner team {}", self.match_id, o.t_end, o.state, o.winner_team);
                if !self.diverged && self.closed_at.is_none() && self.outcome.is_none() && matches!(o.state, 4 | 5) {
                    // an unsolicited Final whose world ended: it is the end
                    self.outcome = Some(o.clone());
                    notes.push(Note::Outcome(o.clone()));
                }
                self.final_ = Some(o.clone());
                if !self.diverged {
                    notes.push(Note::Final(o));
                }
                self.state = LinkState::Closed;
                self.release();
            }
            F::Diverged(d) if self.drive_bots => {
                if !self.diverged {
                    log::error!(
                        "match {:#x}: WORLD HOST DIVERGED at {} (re-timed {}, merged {}): it keeps driving the bots, but its outcome, E0 and resync are not used for this match",
                        self.match_id,
                        d.t,
                        d.retimed,
                        d.merged
                    );
                }
                self.diverged = true;
            }
            F::Diverged(d) => {
                log::error!(
                    "match {:#x}: WORLD HOST DIVERGED at {} (re-timed {}, merged {}): its outcome, E0 and resync are not used for this match",
                    self.match_id,
                    d.t,
                    d.retimed,
                    d.merged
                );
                self.state = LinkState::Diverged;
                self.release();
            }
            F::Fault { reason } => {
                let ready = self.conn.as_ref().is_some_and(|c| c.ready);
                // the host refuses one WantDe with `Fault "WantDe <id>: <why>"` and goes on (world-host session.rs): not a
                // failure of the link
                if let Some(id) = reason.strip_prefix("WantDe ").and_then(|r| r.split(':').next()).and_then(|n| n.trim().parse::<u32>().ok()) {
                    if ready {
                        notes.push(Note::DeRefused { request_id: id, reason });
                        return;
                    }
                }
                if ready {
                    self.fail(&format!("Fault: {reason}"));
                } else {
                    self.set_off(&format!("the host refused the match: {reason}"));
                }
            }
            F::Tick(tk) => notes.push(Note::Tick(tk)),
            F::De(d) => notes.push(Note::De(d)),
            F::Bots(b) => {
                if self.drive_bots {
                    notes.push(Note::Bots(b));
                } else {
                    log::warn!("match {:#x}: world host Bots for {} without drive_bots: ignored", self.match_id, b.t_for);
                }
            }
            F::Used(u) => {
                log::info!("match {:#x}: world host Used at {}: {} player row(s)", self.match_id, u.t, u.players.len());
                self.used = Some(u);
            }
            F::Kills(k) => {
                self.kills_seen = true;
                for x in k.records {
                    if x.t > k.t_upto || usize::from(x.victim) >= proto::MAX_PLAYERS {
                        log::warn!("match {:#x}: world host kill record {x:?} outside its frame (t_upto {}): dropped", self.match_id, k.t_upto);
                        continue;
                    }
                    self.kills.insert((x.t, x.victim), x);
                }
            }
        }
    }
}

impl Drop for HostLink {
    fn drop(&mut self) {
        self.release();
    }
}

struct Writer {
    epoch: u32,
    connector: Connector,
    log: Arc<FrameLog>,
    written: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    slot: Arc<Mutex<Option<Box<dyn HostStream>>>>,
    tx: SyncSender<(u32, Event)>,
    match_id: u32,
}

impl Writer {
    fn run(self) {
        let mut stream = match (self.connector)() {
            Ok(s) => s,
            Err(e) => {
                let _ = self.tx.send((self.epoch, Event::ConnectFailed(e.to_string())));
                return;
            }
        };
        let (reader, shut) = match (stream.try_clone_stream(), stream.try_clone_stream()) {
            (Ok(r), Ok(s)) => (r, s),
            (Err(e), _) | (_, Err(e)) => {
                let _ = self.tx.send((self.epoch, Event::ConnectFailed(format!("clone: {e}"))));
                return;
            }
        };
        *self.slot.lock().unwrap() = Some(shut);
        if self.cancel.load(Ordering::SeqCst) {
            stream.shutdown_both();
            return;
        }
        let _ = self.tx.send((self.epoch, Event::Connected));
        let (epoch, tx, cancel) = (self.epoch, self.tx.clone(), self.cancel.clone());
        let spawned = std::thread::Builder::new().name(format!("host-r-{:08x}-{epoch}", self.match_id)).spawn(move || read_loop(epoch, reader, tx, cancel));
        if spawned.is_err() {
            let _ = self.tx.send((self.epoch, Event::WriteFailed("no reader thread".into())));
            stream.shutdown_both();
            return;
        }
        let mut cur = 0usize;
        loop {
            let batch: Vec<Arc<[u8]>> = {
                let mut g = self.log.inner.lock().unwrap();
                while g.frames.len() <= cur && !g.done && !self.cancel.load(Ordering::SeqCst) {
                    g = self.log.cv.wait_timeout(g, Duration::from_millis(100)).unwrap().0;
                }
                if g.done || self.cancel.load(Ordering::SeqCst) {
                    break;
                }
                let end = g.frames.len().min(cur + 256);
                g.frames[cur..end].to_vec()
            };
            let mut buf = Vec::with_capacity(batch.iter().map(|f| f.len()).sum());
            for f in &batch {
                buf.extend_from_slice(f);
            }
            if let Err(e) = stream.write_all(&buf).and_then(|_| stream.flush()) {
                if !self.cancel.load(Ordering::SeqCst) {
                    let _ = self.tx.send((self.epoch, Event::WriteFailed(e.to_string())));
                }
                break;
            }
            cur += batch.len();
            self.written.store(cur, Ordering::SeqCst);
        }
        stream.shutdown_both();
    }
}

/// The kill log of a match when it agrees with the world's own event rows of `Final`:
/// for every slot with a detail row, its records as killer of another player = rows 0 (`kill`) + 4 (`team_kill`), and
/// its records as killer of himself = row 3 (`suicide`) (the kill handler 0x20033DCF0 adds exactly these per death).
/// None (logged) when there are no detail rows or a count differs: per-kill facts are never invented.
pub(crate) fn checked_kills(match_id: u32, kills: Option<Vec<proto::KillRecord>>, f: &proto::Outcome) -> Option<Vec<proto::KillRecord>> {
    let kills = kills?;
    if f.details.is_empty() {
        return None;
    }
    for d in &f.details {
        let others = kills.iter().filter(|k| k.killer == d.slot && k.victim != d.slot).count();
        let selves = kills.iter().filter(|k| k.killer == d.slot && k.victim == d.slot).count();
        let want_others = usize::from(d.events[0]) + usize::from(d.events[4]);
        let want_selves = usize::from(d.events[3]);
        if others != want_others || selves != want_selves {
            log::warn!(
                "match {match_id:#x}: kill log of slot {} has {others} kill(s) / {selves} suicide(s), the world's rows say {want_others} / {want_selves}: no kill log for this match",
                d.slot
            );
            return None;
        }
    }
    if let Some(k) = kills.iter().find(|k| !f.details.iter().any(|d| d.slot == k.victim) || (k.killer != 0xFF && !f.details.iter().any(|d| d.slot == k.killer))) {
        log::warn!("match {match_id:#x}: kill record {k:?} names a slot without a detail row: no kill log for this match");
        return None;
    }
    Some(kills)
}

fn read_loop(epoch: u32, mut r: Box<dyn HostStream>, tx: SyncSender<(u32, Event)>, cancel: Arc<AtomicBool>) {
    loop {
        let ev = match proto::read_frame(&mut *r) {
            Ok(Some(f)) => match proto::FromHost::decode(&f) {
                Ok(m) => Event::Msg(m),
                Err(e) => Event::Bad(format!("type {:#04x}: {e}", f.ty)),
            },
            Ok(None) => Event::Eof,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => Event::Bad(e.to_string()),
            Err(_) => Event::Eof,
        };
        let last = !matches!(ev, Event::Msg(_));
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        if tx.send((epoch, ev)).is_err() || last {
            return;
        }
    }
}
