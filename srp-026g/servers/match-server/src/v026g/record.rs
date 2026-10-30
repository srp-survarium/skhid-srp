//! Match recordings (`SRP_RECORD_DIR`): every datagram of a match, in and
//! out, in one file per match, kept when enough players played in it
//! (`SRP_RECORD_MIN_PLAYERS`, late joiners counted; one human for PvE), for learning from
//! later (player bots). Off unless the folder is set.
//!
//! A file holds what the server and the clients said to each other, so
//! anything the match server saw can be rebuilt from it offline: the inputs
//! each player made and where they went, the hash reports, the uploaded
//! worlds, the worlds sent to joiners, the roster changes. Nothing is
//! decoded here; the datagrams are the transport's own (`udp-transport`),
//! as `SRP_PACKET_LOG` has them, with no address.
//!
//! The file, `rec026g_<UTC start>_<match id>.srprec`:
//!
//! ```text
//! "SRPREC01"                         8 bytes, once
//! block*:  u32 LE packed length, u32 LE raw length, packed bytes
//! ```
//!
//! A block's bytes are a raw LZMA2 stream (`survarium::v026g::lzma2::
//! packed`: dictionary 32 KiB; Python: `lzma.decompress(data,
//! lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2, "dict_size":
//! 0x8000}])`). Unpacked, a block is records:
//!
//! ```text
//! u8 kind (1 in, 2 out, 3 note, 4 accepted input, 5 accepted kit, 6 accepted pickup),
//! u8 place (0xFF: none),
//! u32 LE ms since the recording began, u32 LE length, the bytes
//! ```
//!
//! A note is one JSON object: the first is the header (the match, the map,
//! the places with their accounts), then `places` when late joiners get
//! places, `entered` / `left` for roster changes, `clock` when the match
//! clock starts, `result` at the end, and `closed`. A block is written at
//! least every [`BLOCK_EVERY`], so a server that stops loses that much at
//! most.
//!
//! A file is written under `.part` from the match's first connection; it
//! gets its name once [`Recording::entered`] has seen the minimum of
//! players, and a `.part` is deleted when its match ends (and at start, for
//! one a stopped server left). Kept files over `SRP_RECORD_MAX_MB` together
//! are deleted oldest first. The writing and the packing are done on a
//! thread of their own; the match loop only hands the bytes over, and drops
//! them (counted) if that thread is behind by [`QUEUE`] items.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use session::v026g::replays::Replays;
use survarium::v026g::lzma2;
use survarium::v026g::match_msgs::PlayerInput;
use survarium::v026g::match_msgs::pickup::PickupInput;

/// The file's first bytes.
pub const MAGIC: &[u8; 8] = b"SRPREC01";
/// A record's kind.
pub const IN: u8 = 1;
pub const OUT: u8 = 2;
pub const NOTE: u8 = 3;
/// Accepted human action in PvE: full match time (u32 LE), rotation (4
/// bytes), actions (u32 LE). Recorded after validation and late placement.
pub const ACCEPTED_INPUT: u8 = 4;
/// Accepted human kit choice in PvE: full match time (u32 LE), kit (u8).
pub const ACCEPTED_PROFILE: u8 = 5;
/// Accepted targeted input in PvE: accepted-input bytes, then donor place
/// (u8) and donor spawn time (u32 LE). The target follows retiming/discard.
pub const ACCEPTED_PICKUP: u8 = 6;
/// The place of a record that is no player's.
pub const NO_PLACE: u8 = 0xFF;
/// A block is packed and written once it holds this much ...
const BLOCK_BYTES: usize = 1 << 20;
/// ... or is this old.
const BLOCK_EVERY: Duration = Duration::from_secs(20);
/// Items the writing thread may be behind before new ones are dropped.
const QUEUE: usize = 200_000;
const PART: &str = "part";
const EXTENSION: &str = "srprec";

/// `SRP_RECORD_*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecordSettings {
    pub dir: PathBuf,
    /// A match is kept once this many players (test bots not counted) have
    /// entered it, at its start or later. PvE needs only one human.
    pub min_players: usize,
    /// Kept files over this many bytes together: the oldest go.
    pub max_bytes: u64,
    /// "Watch replay" (`SRP_REPLAY_DIR`): told of every file kept (its
    /// replay is made when first asked for, [`super::replay`]), closed
    /// under the minimum, or deleted by the cap (its replay goes too). Set
    /// by [`super::run_on_with_replays`]; `None` from the environment alone.
    pub replays: Option<Desk>,
}

/// The replays of the srp process, as the writer tells them of its files.
#[derive(Clone)]
pub(crate) struct Desk(pub Arc<Replays>);

impl std::fmt::Debug for Desk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Replays")
    }
}

impl PartialEq for Desk {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Desk {}

/// The match of a kept recording's file name, `rec026g_<UTC
/// start>_<id:08x>.srprec`.
pub fn match_of_file(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_prefix("rec026g_")?;
    let (_, id) = stem.rsplit_once('_')?;
    session::v026g::replays::match_of_name(id, &format!(".{EXTENSION}"))
}

impl RecordSettings {
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| std::env::var(name).ok();
        let dir = var("SRP_RECORD_DIR")?;
        let dir = dir.trim();
        if dir.is_empty() || dir.eq_ignore_ascii_case("off") {
            return None;
        }
        Some(Self {
            dir: dir.into(),
            min_players: super::parse_number(
                "SRP_RECORD_MIN_PLAYERS",
                var("SRP_RECORD_MIN_PLAYERS").as_deref(),
                6,
            )
            .max(1) as usize,
            max_bytes: u64::from(super::parse_number(
                "SRP_RECORD_MAX_MB",
                var("SRP_RECORD_MAX_MB").as_deref(),
                5_000,
            )) << 20,
            replays: None,
        })
    }

    fn tell(&self, event: impl FnOnce(&Replays)) {
        if let Some(Desk(replays)) = &self.replays {
            event(replays);
        }
    }
}

enum Job {
    Open { match_id: u32, name: String },
    Record { match_id: u32, bytes: Vec<u8> },
    Keep { match_id: u32 },
    Close { match_id: u32 },
}

/// The writing thread's door: one for the match server, shared by its
/// matches.
#[derive(Clone)]
pub(crate) struct Recorder {
    jobs: SyncSender<Job>,
    min_players: usize,
}

impl Recorder {
    /// Starts the writing thread, after deleting what a stopped server left
    /// under `.part`.
    pub fn start(settings: RecordSettings) -> io::Result<Self> {
        fs::create_dir_all(&settings.dir)?;
        for entry in fs::read_dir(&settings.dir)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == PART) {
                log::info!(
                    "Recordings: {} was a match of fewer than {} players when the server \
                     stopped; deleted",
                    path.display(),
                    settings.min_players
                );
                let _ = fs::remove_file(&path);
            }
        }
        let (jobs, queue) = mpsc::sync_channel(QUEUE);
        let min_players = settings.min_players;
        std::thread::Builder::new()
            .name("srp-record".into())
            .spawn(move || Writer::new(settings).run(queue))?;
        Ok(Self { jobs, min_players })
    }

    /// The recording of a new match: its header note goes first.
    pub fn open(
        &self,
        match_id: u32,
        vs_bots: bool,
        header: serde_json::Value,
        now: Instant,
    ) -> Recording {
        let started = SystemTime::now();
        let name = format!("rec026g_{}_{match_id:08x}", utc_stamp(started));
        let mut recording = Recording {
            recorder: self.clone(),
            min_players: if vs_bots { 1 } else { self.min_players },
            match_id,
            began: now,
            entered: HashSet::new(),
            kept: false,
            dropped: 0,
        };
        recording.send(Job::Open { match_id, name });
        let mut header = header;
        if let Some(object) = header.as_object_mut() {
            object.insert("note".into(), "header".into());
            object.insert("format".into(), 1.into());
            object.insert("began_unix_ms".into(), unix_ms(started).into());
            object.insert("min_players".into(), recording.min_players.into());
            object.insert("srp".into(), env!("CARGO_PKG_VERSION").into());
        }
        recording.note(&header, now);
        recording
    }
}

/// One match's recording. Dropped, it is closed: kept if it reached the
/// minimum, else deleted.
pub(crate) struct Recording {
    recorder: Recorder,
    min_players: usize,
    match_id: u32,
    began: Instant,
    /// The places that entered, test bots not counted.
    entered: HashSet<usize>,
    kept: bool,
    dropped: u64,
}

impl Recording {
    fn at(&self, now: Instant) -> u32 {
        now.saturating_duration_since(self.began).as_millis() as u32
    }

    fn send(&mut self, job: Job) {
        match self.recorder.jobs.try_send(job) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                if self.dropped == 0 {
                    log::warn!(
                        "Match {:#x}: the recording's writer is behind; records are dropped",
                        self.match_id
                    );
                }
                self.dropped += 1;
            }
            Err(TrySendError::Disconnected(_)) => self.dropped += 1,
        }
    }

    fn record(&mut self, kind: u8, place: u8, bytes: &[u8], now: Instant) {
        let mut record = Vec::with_capacity(10 + bytes.len());
        record.push(kind);
        record.push(place);
        record.extend_from_slice(&self.at(now).to_le_bytes());
        record.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        record.extend_from_slice(bytes);
        self.send(Job::Record {
            match_id: self.match_id,
            bytes: record,
        });
    }

    /// A datagram the client at `place` sent.
    pub fn datagram_in(&mut self, place: usize, datagram: &[u8], now: Instant) {
        self.record(IN, place_byte(place), datagram, now);
    }

    /// A datagram sent to the client at `place`.
    pub fn datagram_out(&mut self, place: usize, datagram: &[u8], now: Instant) {
        self.record(OUT, place_byte(place), datagram, now);
    }

    /// A human input accepted at its final time, even when there is no
    /// other client to receive a relay (a solo bot match).
    pub fn accepted_input(&mut self, place: usize, time: u32, input: &PlayerInput, now: Instant) {
        let mut bytes = [0; 12];
        bytes[..4].copy_from_slice(&time.to_le_bytes());
        bytes[4..8].copy_from_slice(&input.rotation);
        bytes[8..].copy_from_slice(&input.actions.to_le_bytes());
        self.record(ACCEPTED_INPUT, place_byte(place), &bytes, now);
    }

    pub fn accepted_pickup(&mut self, place: usize, time: u32, input: &PickupInput, now: Instant) {
        let mut bytes = [0; 17];
        bytes[..4].copy_from_slice(&time.to_le_bytes());
        bytes[4..8].copy_from_slice(&input.input.rotation);
        bytes[8..12].copy_from_slice(&input.input.actions.to_le_bytes());
        bytes[12] = input.target.0.place.get();
        bytes[13..].copy_from_slice(&input.target.0.spawned.to_le_bytes());
        self.record(ACCEPTED_PICKUP, place_byte(place), &bytes, now);
    }

    pub fn accepted_profile(&mut self, place: usize, time: u32, kit: u8, now: Instant) {
        let mut bytes = [0; 5];
        bytes[..4].copy_from_slice(&time.to_le_bytes());
        bytes[4] = kit;
        self.record(ACCEPTED_PROFILE, place_byte(place), &bytes, now);
    }

    /// A JSON note.
    pub fn note(&mut self, note: &serde_json::Value, now: Instant) {
        self.record(NOTE, NO_PLACE, note.to_string().as_bytes(), now);
    }

    /// A player (not a test bot) entered the match: the file is kept once
    /// the minimum have.
    pub fn entered(&mut self, place: usize) {
        self.entered.insert(place);
        if !self.kept && self.entered.len() >= self.min_players {
            self.kept = true;
            log::info!(
                "Match {:#x}: {} players have entered; its recording is kept",
                self.match_id,
                self.entered.len()
            );
            self.send(Job::Keep {
                match_id: self.match_id,
            });
        }
    }

    #[cfg(test)]
    pub fn players(&self) -> usize {
        self.entered.len()
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        let dropped = self.dropped;
        let players = self.entered.len();
        if dropped > 0 {
            log::warn!(
                "Match {:#x}: its recording lost {dropped} records",
                self.match_id
            );
        }
        let note = serde_json::json!({
            "note": "closed",
            "players": players,
            "kept": self.kept,
            "dropped": dropped,
        });
        self.note(&note, Instant::now());
        self.send(Job::Close {
            match_id: self.match_id,
        });
    }
}

fn place_byte(place: usize) -> u8 {
    u8::try_from(place).unwrap_or(NO_PLACE)
}

/// One open file.
struct Open {
    name: String,
    path: PathBuf,
    file: Option<BufWriter<File>>,
    block: Vec<u8>,
    block_since: Instant,
    kept: bool,
}

struct Writer {
    settings: RecordSettings,
    open: HashMap<u32, Open>,
}

impl Writer {
    fn new(settings: RecordSettings) -> Self {
        Self {
            settings,
            open: HashMap::new(),
        }
    }

    fn run(mut self, queue: Receiver<Job>) {
        loop {
            match queue.recv_timeout(Duration::from_secs(1)) {
                Ok(job) => self.handle(job),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    let ids: Vec<u32> = self.open.keys().copied().collect();
                    for match_id in ids {
                        self.close(match_id);
                    }
                    return;
                }
            }
            let now = Instant::now();
            for open in self.open.values_mut() {
                if !open.block.is_empty() && now.duration_since(open.block_since) >= BLOCK_EVERY {
                    write_block(open);
                }
            }
        }
    }

    fn handle(&mut self, job: Job) {
        match job {
            Job::Open { match_id, name } => {
                let path = self.settings.dir.join(format!("{name}.{EXTENSION}.{PART}"));
                let file = File::create(&path)
                    .and_then(|file| {
                        let mut file = BufWriter::new(file);
                        file.write_all(MAGIC)?;
                        Ok(file)
                    })
                    .map_err(|error| {
                        log::warn!("Recordings: can't write {}: {error}", path.display())
                    })
                    .ok();
                self.open.insert(
                    match_id,
                    Open {
                        name,
                        path,
                        file,
                        block: Vec::new(),
                        block_since: Instant::now(),
                        kept: false,
                    },
                );
            }
            Job::Record { match_id, bytes } => {
                if let Some(open) = self.open.get_mut(&match_id) {
                    if open.block.is_empty() {
                        open.block_since = Instant::now();
                    }
                    open.block.extend_from_slice(&bytes);
                    if open.block.len() >= BLOCK_BYTES {
                        write_block(open);
                    }
                }
            }
            Job::Keep { match_id } => {
                if let Some(open) = self.open.get_mut(&match_id) {
                    let kept = self.settings.dir.join(format!("{}.{EXTENSION}", open.name));
                    // The file stays open: it is renamed under its writer
                    // (Rust opens files on Windows with FILE_SHARE_DELETE).
                    match fs::rename(&open.path, &kept) {
                        Ok(()) => {
                            open.path = kept;
                            open.kept = true;
                        }
                        Err(error) => {
                            log::warn!("Recordings: can't keep {}: {error}", open.path.display())
                        }
                    }
                }
            }
            Job::Close { match_id } => self.close(match_id),
        }
    }

    fn close(&mut self, match_id: u32) {
        let Some(mut open) = self.open.remove(&match_id) else {
            return;
        };
        if !open.kept {
            drop(open.file.take());
            let _ = fs::remove_file(&open.path);
            self.settings
                .tell(|replays| replays.recording_not_kept(match_id));
            return;
        }
        write_block(&mut open);
        let written = match open.file.take() {
            Some(mut file) => match file.flush() {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("Recordings: can't write {}: {error}", open.path.display());
                    false
                }
            },
            // A write failed earlier: the file is cut short.
            None => false,
        };
        let size = fs::metadata(&open.path).map_or(0, |meta| meta.len());
        log::info!(
            "Match {match_id:#x}: recording {} kept ({size} bytes)",
            open.path.display()
        );
        // Closed and whole: its replay can be made from it now. (A file cut
        // short is converted only if someone asks: what it has may do.)
        if written {
            self.settings
                .tell(|replays| replays.recording_kept(match_id));
        }
        self.prune();
    }

    /// The oldest kept files go until the rest fit `max_bytes`.
    fn prune(&self) {
        let open: HashSet<&Path> = self.open.values().map(|open| open.path.as_path()).collect();
        let Ok(entries) = fs::read_dir(&self.settings.dir) else {
            return;
        };
        let mut kept: Vec<(String, PathBuf, u64)> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == EXTENSION))
            .filter(|path| !open.contains(path.as_path()))
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                let size = fs::metadata(&path).ok()?.len();
                Some((name, path, size))
            })
            .collect();
        // The names begin with the UTC start: oldest first.
        kept.sort();
        let mut total: u64 = kept.iter().map(|(_, _, size)| size).sum();
        for (_, path, size) in kept {
            if total <= self.settings.max_bytes {
                break;
            }
            log::info!(
                "Recordings: over {} MB together; {} deleted",
                self.settings.max_bytes >> 20,
                path.display()
            );
            if fs::remove_file(&path).is_ok() {
                total -= size;
                if let Some(match_id) = match_of_file(&path) {
                    self.settings
                        .tell(|replays| replays.recording_deleted(match_id));
                }
            }
        }
    }
}

fn write_block(open: &mut Open) {
    if open.block.is_empty() {
        return;
    }
    let raw = std::mem::take(&mut open.block);
    let Some(file) = open.file.as_mut() else {
        return;
    };
    let packed = lzma2::packed(&raw);
    let result = file
        .write_all(&(packed.len() as u32).to_le_bytes())
        .and_then(|()| file.write_all(&(raw.len() as u32).to_le_bytes()))
        .and_then(|()| file.write_all(&packed))
        .and_then(|()| file.flush());
    if let Err(error) = result {
        log::warn!("Recordings: can't write {}: {error}", open.path.display());
        open.file = None;
    }
}

/// One record of a file ([`read`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub kind: u8,
    pub place: u8,
    pub at_ms: u32,
    pub bytes: Vec<u8>,
}

/// One record of a file as [`read_each`] hands it over: its bytes are
/// borrowed from the block being read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordRef<'a> {
    pub kind: u8,
    pub place: u8,
    pub at_ms: u32,
    pub bytes: &'a [u8],
}

impl RecordRef<'_> {
    pub fn to_record(self) -> Record {
        Record {
            kind: self.kind,
            place: self.place,
            at_ms: self.at_ms,
            bytes: self.bytes.to_vec(),
        }
    }
}

impl Record {
    pub fn borrowed(&self) -> RecordRef<'_> {
        RecordRef {
            kind: self.kind,
            place: self.place,
            at_ms: self.at_ms,
            bytes: &self.bytes,
        }
    }
}

/// A block's raw length is never more than this: the writer packs a block
/// once it holds [`BLOCK_BYTES`], and a record is one datagram or note
/// past that. A larger length is not a block of ours (it is not unpacked).
const MAX_BLOCK_RAW: usize = 64 << 20;

/// Every record of a recording file, in order, one block in memory at a
/// time: what the server's own replay conversion reads with, so a match
/// of a million datagrams is never held whole (`replay`). A file cut
/// short (its server stopped) gives what its whole blocks hold.
pub fn read_each(path: &Path, mut each: impl FnMut(RecordRef<'_>)) -> io::Result<()> {
    read_while(path, |record| {
        each(record);
        true
    })
}

/// [`read_each`] until `each` says `false`: the blocks after that record's
/// are not read or unpacked.
fn read_while(path: &Path, mut each: impl FnMut(RecordRef<'_>) -> bool) -> io::Result<()> {
    let mut file = io::BufReader::new(File::open(path)?);
    let mut magic = [0u8; 8];
    if io::Read::read_exact(&mut file, &mut magic).is_err() || magic != *MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a match recording",
        ));
    }
    let mut packed = Vec::new();
    loop {
        let mut head = [0u8; 8];
        match io::Read::read_exact(&mut file, &mut head) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        }
        let packed_len = u32::from_le_bytes(head[0..4].try_into().expect("4 bytes")) as usize;
        let raw_len = u32::from_le_bytes(head[4..8].try_into().expect("4 bytes")) as usize;
        if raw_len > MAX_BLOCK_RAW || packed_len > MAX_BLOCK_RAW {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a block is larger than any the writer makes",
            ));
        }
        packed.resize(packed_len, 0);
        match io::Read::read_exact(&mut file, &mut packed) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        }
        let Some(raw) = lzma2::unpack(&packed, raw_len) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a block does not unpack",
            ));
        };
        let mut block = raw.as_slice();
        while block.len() >= 10 {
            let len = u32::from_le_bytes(block[6..10].try_into().expect("4 bytes")) as usize;
            let Some(bytes) = block.get(10..10 + len) else {
                break;
            };
            let more = each(RecordRef {
                kind: block[0],
                place: block[1],
                at_ms: u32::from_le_bytes(block[2..6].try_into().expect("4 bytes")),
                bytes,
            });
            if !more {
                return Ok(());
            }
            block = &block[10 + len..];
        }
    }
    Ok(())
}

/// Every record of a recording file, in order. A file cut short (its
/// server stopped) gives what its whole blocks hold.
pub fn read(path: &Path) -> io::Result<Vec<Record>> {
    let mut records = Vec::new();
    read_each(path, |record| records.push(record.to_record()))?;
    Ok(records)
}

/// The notes of a recording file alone (its header, the places, the roster
/// changes, the clock, the result): a few hundred small records.
pub fn read_notes(path: &Path) -> io::Result<Vec<Record>> {
    let mut notes = Vec::new();
    read_each(path, |record| {
        if record.kind == NOTE {
            notes.push(record.to_record());
        }
    })?;
    Ok(notes)
}

/// The first note of a recording file, its header (the writer puts it
/// first, [`Recorder::open`]), so from the first block alone: what the
/// replay conversion checks (the map, so the mode's file) before it unpacks
/// the whole file. Reading stops at the first note; `None` for a file of
/// none.
pub fn read_header(path: &Path) -> io::Result<Option<Record>> {
    let mut header = None;
    read_while(path, |record| {
        if record.kind == NOTE {
            header = Some(record.to_record());
        }
        header.is_none()
    })?;
    Ok(header)
}

fn unix_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

/// `YYYYMMDD-HHMMSS` in UTC.
fn utc_stamp(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(dir: &Path, min_players: usize) -> RecordSettings {
        RecordSettings {
            dir: dir.to_path_buf(),
            min_players,
            max_bytes: 1 << 30,
            replays: None,
        }
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Waits for the writing thread.
    fn settle(dir: &Path, done: impl Fn(&[String]) -> bool) -> Vec<String> {
        for _ in 0..200 {
            let names = files(dir);
            if done(&names) {
                return names;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        files(dir)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "srp-record-{name}-{}-{}",
            std::process::id(),
            unix_ms(SystemTime::now())
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_bot_match_is_kept_after_one_human_but_empty_and_small_pvp_matches_are_not() {
        use session::v026g::replays::{Known, ReplaySettings, Status};
        let root = temp_dir("solo-bots");
        let dir = root.join("recordings");
        let replays = Arc::new(Replays::new(
            Some(
                ReplaySettings {
                    replay_dir: root.join("replays"),
                    record_dir: dir.clone(),
                    modes_dir: root.join("modes"),
                    max_recording_bytes: 1 << 20,
                    max_replay_bytes: 1 << 20,
                }
                .checked()
                .unwrap(),
            ),
            6,
        ));
        let mut settings = settings(&dir, 6);
        settings.replays = Some(Desk(replays.clone()));
        let recorder = Recorder::start(settings).unwrap();
        let now = Instant::now();
        let empty = recorder.open(1, true, serde_json::json!({}), now);
        drop(empty);
        let mut pvp = recorder.open(2, false, serde_json::json!({}), now);
        pvp.entered(0);
        drop(pvp);
        let mut pve = recorder.open(3, true, serde_json::json!({}), now);
        pve.entered(0);
        drop(pve);
        let names = settle(&dir, |names| {
            names.len() == 1
                && names[0].ends_with("_00000003.srprec")
                && read_header(&dir.join(&names[0])).is_ok_and(|h| h.is_some())
        });
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].ends_with("_00000003.srprec"));
        let header = read_header(&dir.join(&names[0])).unwrap().unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header.bytes).unwrap();
        assert_eq!(header["min_players"], 1);
        assert!(
            replays.jobs().is_empty(),
            "conversion waits for a watch request"
        );
        assert_eq!(
            replays.ask(
                1,
                3,
                Some(14),
                Some(Known {
                    played: true,
                    places: 1
                }),
                now + Duration::from_secs(10)
            ),
            Status::Preparing
        );
        assert_eq!(replays.jobs(), [3]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn utc_stamps() {
        assert_eq!(utc_stamp(UNIX_EPOCH), "19700101-000000");
        let time = UNIX_EPOCH + Duration::from_secs(1_791_945_296);
        assert_eq!(utc_stamp(time), "20261014-023456");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400 + 3_661);
        assert_eq!(utc_stamp(leap), "20000229-010101");
    }

    #[test]
    fn solo_pve_recording_preserves_pickup_target_and_full_accepted_time() {
        use survarium::v026g::match_msgs::WireTime;
        use survarium::v026g::match_msgs::pickup::{DropId, Life, Place};
        let dir = temp_dir("pickup");
        let recorder = Recorder::start(settings(&dir, 6)).unwrap();
        let now = Instant::now();
        let mut recording = recorder.open(0xAE, true, serde_json::json!({}), now);
        recording.entered(0);
        let input = PickupInput {
            input: PlayerInput {
                time: WireTime::precise(123457),
                rotation: [1, 2, 3, 4],
                actions: 0x8000_0020,
            },
            target: DropId(Life {
                place: Place::parse(19).unwrap(),
                spawned: 0xfedc_ba98,
            }),
        };
        recording.accepted_pickup(0, 123457, &input, now + Duration::from_millis(9));
        drop(recording);
        let names = settle(&dir, |names| {
            names.len() == 1 && names[0].ends_with(".srprec")
        });
        assert_eq!(names.len(), 1);
        let path = dir.join(&names[0]);
        let mut records = Vec::new();
        for _ in 0..200 {
            records = read(&path).unwrap_or_default();
            if records.len() == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(records.len(), 3);
        let mut expected = 123457u32.to_le_bytes().to_vec();
        expected.extend([1, 2, 3, 4]);
        expected.extend(0x8000_0020u32.to_le_bytes());
        expected.push(19);
        expected.extend(0xfedc_ba98u32.to_le_bytes());
        assert_eq!(
            records[1],
            Record {
                kind: ACCEPTED_PICKUP,
                place: 0,
                at_ms: 9,
                bytes: expected
            }
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_match_that_reaches_the_minimum_is_kept_with_every_record() {
        let dir = temp_dir("kept");
        let recorder = Recorder::start(settings(&dir, 2)).unwrap();
        let now = Instant::now();
        let mut recording =
            recorder.open(0xAB, false, serde_json::json!({"match_id": "0xab"}), now);
        recording.datagram_in(0, &[1, 2, 3], now);
        recording.entered(0);
        // A late joiner makes the second.
        recording.datagram_out(1, &[9; 600], now + Duration::from_millis(1_500));
        recording.entered(1);
        recording.entered(1);
        assert_eq!(recording.players(), 2);
        drop(recording);
        let names = settle(&dir, |names| {
            names.len() == 1 && names[0].ends_with(".srprec")
        });
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].starts_with("rec026g_") && names[0].ends_with("_000000ab.srprec"));
        // The file is closed once its size is logged: wait for the end note.
        let path = dir.join(&names[0]);
        let mut records = Vec::new();
        for _ in 0..200 {
            records = read(&path).unwrap_or_default();
            if records.len() == 4 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(records.len(), 4, "{records:?}");
        assert_eq!(records[0].kind, NOTE);
        let header: serde_json::Value = serde_json::from_slice(&records[0].bytes).unwrap();
        assert_eq!(header["note"], "header");
        assert_eq!(header["match_id"], "0xab");
        assert_eq!(header["min_players"], 2);
        assert_eq!(
            records[1],
            Record {
                kind: IN,
                place: 0,
                at_ms: 0,
                bytes: vec![1, 2, 3]
            }
        );
        assert_eq!(records[2].kind, OUT);
        assert_eq!(records[2].place, 1);
        assert_eq!(records[2].at_ms, 1_500);
        assert_eq!(records[2].bytes, vec![9; 600]);
        let closed: serde_json::Value = serde_json::from_slice(&records[3].bytes).unwrap();
        assert_eq!(closed["note"], "closed");
        assert_eq!(closed["players"], 2);
        assert_eq!(closed["kept"], true);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_match_under_the_minimum_leaves_no_file() {
        let dir = temp_dir("small");
        let recorder = Recorder::start(settings(&dir, 6)).unwrap();
        let now = Instant::now();
        let mut recording = recorder.open(0xCD, false, serde_json::json!({}), now);
        for place in 0..5 {
            recording.entered(place);
            recording.datagram_in(place, &[place as u8], now);
        }
        assert!(
            settle(&dir, |names| names.len() == 1)[0].ends_with(".srprec.part"),
            "written under .part until kept"
        );
        drop(recording);
        assert!(settle(&dir, <[String]>::is_empty).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_part_left_by_a_stopped_server_is_deleted_at_start() {
        let dir = temp_dir("left");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("rec026g_20261014-000000_00000001.srprec.part"),
            b"x",
        )
        .unwrap();
        fs::write(dir.join("rec026g_20261014-000000_00000002.srprec"), b"y").unwrap();
        let _recorder = Recorder::start(settings(&dir, 6)).unwrap();
        assert_eq!(files(&dir), ["rec026g_20261014-000000_00000002.srprec"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_oldest_kept_files_go_over_the_cap() {
        let dir = temp_dir("cap");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("rec026g_20261001-000000_00000001.srprec"),
            [0; 600],
        )
        .unwrap();
        fs::write(
            dir.join("rec026g_20261002-000000_00000002.srprec"),
            [0; 600],
        )
        .unwrap();
        let mut settings = settings(&dir, 1);
        // The new file is small: only it fits with neither old one.
        settings.max_bytes = 700;
        let recorder = Recorder::start(settings).unwrap();
        let mut recording = recorder.open(3, false, serde_json::json!({}), Instant::now());
        recording.entered(0);
        drop(recording);
        let names = settle(&dir, |names| names.len() == 1);
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].ends_with("_00000003.srprec"), "{names:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_replays_hear_of_each_file_kept_dropped_or_deleted() {
        use session::v026g::replays::{Known, ReplaySettings, Status};
        let root = temp_dir("desk");
        let dir = root.join("recordings");
        fs::create_dir_all(&dir).unwrap();
        // An old file the cap deletes, with its replay.
        fs::write(
            dir.join("rec026g_20261001-000000_00000001.srprec"),
            [0; 600],
        )
        .unwrap();
        let replays = Arc::new(Replays::new(
            Some(
                ReplaySettings {
                    replay_dir: root.join("replays"),
                    record_dir: dir.clone(),
                    modes_dir: root.join("modes"),
                    max_recording_bytes: 1 << 20,
                    max_replay_bytes: 1 << 20,
                }
                .checked()
                .unwrap(),
            ),
            2,
        ));
        let old_replay = replays.sur_path(1).unwrap();
        fs::write(&old_replay, b"x").unwrap();
        let mut settings = settings(&dir, 2);
        settings.max_bytes = 700;
        settings.replays = Some(Desk(replays.clone()));
        let recorder = Recorder::start(settings).unwrap();
        let now = Instant::now();
        let mut kept = recorder.open(0xAB, false, serde_json::json!({}), now);
        kept.entered(0);
        kept.entered(1);
        drop(kept);
        let mut small = recorder.open(0xCD, false, serde_json::json!({}), now);
        small.entered(0);
        drop(small);
        // The writer works on its own thread: wait for both closes.
        let mut waited = false;
        for _ in 0..300 {
            let names = files(&dir);
            let parts = names.iter().any(|name| name.ends_with(".part"));
            let kept = names.iter().any(|name| name.ends_with("_000000ab.srprec"));
            if !old_replay.exists() && kept && !parts {
                waited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(waited, "{:?}", files(&dir));
        // Kept, but converted only when someone asks for it.
        assert!(replays.jobs().is_empty());
        let known = Some(Known {
            played: true,
            places: 10,
        });
        let later = now + Duration::from_secs(10);
        assert_eq!(
            replays.ask(1, 0xCD, Some(14), known, later),
            Status::NotRecorded
        );
        assert_eq!(replays.ask(2, 1, Some(14), known, later), Status::Gone);
        assert_eq!(
            replays.ask(3, 0xAB, Some(14), known, later),
            Status::Preparing
        );
        assert_eq!(replays.jobs(), [0xAB]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_recordings_match_is_read_from_its_name_only() {
        let at = |name: &str| match_of_file(Path::new(name));
        assert_eq!(
            at("rec026g_20261014-215125_1f07c8fb.srprec"),
            Some(0x1F07_C8FB)
        );
        assert_eq!(at("/x/rec026g_20261014-215125_00000001.srprec"), Some(1));
        assert_eq!(at("rec026g_20261014-215125_1f07c8fb.srprec.part"), None);
        assert_eq!(at("rec026g_20261014-215125_1F07C8FB.srprec"), None);
        assert_eq!(at("x_20261014-215125_1f07c8fb.srprec"), None);
        assert_eq!(at("rec026g_1f07c8fb.srprec"), None);
    }

    #[test]
    fn many_blocks_read_back() {
        let dir = temp_dir("blocks");
        let recorder = Recorder::start(settings(&dir, 1)).unwrap();
        let now = Instant::now();
        let mut recording = recorder.open(4, false, serde_json::json!({}), now);
        recording.entered(0);
        // 3 MB of datagrams: three blocks or more.
        let datagram: Vec<u8> = (0..500u32).map(|i| (i * 7 % 251) as u8).collect();
        for i in 0..6_000u32 {
            recording.datagram_in(
                (i % 20) as usize,
                &datagram,
                now + Duration::from_millis(u64::from(i)),
            );
            // Paced as a match is: all at once, a busy machine's writer
            // falls behind and records are dropped by design.
            if i % 200 == 199 {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        drop(recording);
        let names = settle(&dir, |names| names.len() == 1);
        let path = dir.join(&names[0]);
        let mut records = Vec::new();
        for _ in 0..500 {
            records = read(&path).unwrap_or_default();
            if records.len() == 6_002 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(records.len(), 6_002);
        assert_eq!(records[6_000].place, 19);
        assert_eq!(records[6_000].at_ms, 5_999);
        assert_eq!(records[6_000].bytes, datagram);
        let _ = fs::remove_dir_all(&dir);
    }
}
