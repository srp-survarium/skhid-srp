//! Bounded executable collection. Only a hash is used as a file name. Each
//! peer owns one transfer, with one window in memory; Drop removes partial
//! files and releases the reservation. Completed files are SHA-256 verified,
//! private, never executed, and shared across matches by their hash. The
//! release's own executable ([`Released`]) is never asked for. The game's
//! code blocks ([`Upload::Code`]) are kept the same way in a store of their
//! own ([`Kind::Code`]), one block after another on a connection ([`Blocks`]).
use openssl::sha::Sha256;
use srp_validate::executable::{Answer, Identity, Upload, CHUNK, WINDOW};
use std::{collections::BTreeMap, fs::{self, File, OpenOptions}, io::{self, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock}, time::{Duration, Instant}};

const MAX_ACTIVE: usize = 16;
/// Code blocks offered on one connection at most (the game offers 32).
const MAX_BLOCKS: u8 = 32;

/// What a store keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Games' executable files: `client-executables/<sha256>.exe`.
    Executable,
    /// Code allocations of games that are no module, as the game read them
    /// from its memory: `client-code/<sha256>.bin`.
    Code,
}
impl Kind {
    fn folder(self) -> &'static str { match self { Kind::Executable => "client-executables", Kind::Code => "client-code" } }
    fn suffix(self) -> &'static str { match self { Kind::Executable => ".exe", Kind::Code => ".bin" } }
    /// Bytes kept at most, partial files counted.
    fn limit(self) -> u64 { match self { Kind::Executable => 256 << 20, Kind::Code => 512 << 20 } }
    fn max_files(self) -> usize { match self { Kind::Executable => 1024, Kind::Code => 16384 } }
}
/// `SRP_RELEASED_EXE_SHA256`: the SHA-256 of the game executable in the
/// players' download this server was released with, which the release writes
/// beside `SRP_MIN_PATCH_REVISION` and the match server reads once at its
/// start. A game that offers it is answered as one whose file is stored:
/// nothing is asked for or written. Unset (a local server): every unknown
/// executable is collected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Released([u8; 32]);
impl Released {
    pub fn parse(value: Option<&str>) -> Option<Self> {
        let value = value.map(str::trim).filter(|value| !value.is_empty())?;
        let digit = |digit: u8| char::from(digit).to_digit(16);
        let sha256 = (value.len() == 64).then(|| value.as_bytes().chunks(2).map(|pair| Some((digit(pair[0])? * 16 + digit(pair[1])?) as u8)).collect::<Option<Vec<u8>>>())
            .flatten().and_then(|bytes| <[u8; 32]>::try_from(bytes).ok());
        match sha256 {
            // The release prints this line from the new server's log (release/lib.sh, check_server).
            Some(sha256) => log::info!("Executables: the release's own is sha256 {}; a game that reports it is not asked for its file", Self(sha256)),
            None => log::error!("SRP_RELEASED_EXE_SHA256 is `{value}`; it must be the 64 hex digits of a SHA-256. Every unknown executable is collected"),
        }
        sha256.map(Self)
    }
}
impl std::fmt::Display for Released {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}
struct Catalog {
    kind: Kind,
    root: PathBuf,
    stored: BTreeMap<String, u64>,
    used: u64,
    reserved: u64,
    active: usize,
    serial: u64,
}
impl Catalog {
    fn open(kind: Kind, root: &Path) -> io::Result<Self> {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
        match builder.create(root) { Ok(()) => {}, Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}, Err(e) => return Err(e) }
        if !fs::symlink_metadata(root)?.file_type().is_dir() { return Err(io::Error::other("not a directory")); }
        let mut catalog = Self { kind, root: root.to_owned(), stored: BTreeMap::new(), used: 0, reserved: 0, active: 0, serial: 0 };
        for (n, entry) in fs::read_dir(root)?.enumerate() {
            if n >= kind.max_files() { return Err(io::Error::other(format!("{} has too many files", kind.folder()))); }
            let entry = entry?;
            if !entry.file_type()?.is_file() { return Err(io::Error::other("unexpected entry in executable store")); }
            let size = entry.metadata()?.len();
            catalog.used = catalog.used.saturating_add(size); // includes crash-left partial files
            let name = entry.file_name();
            if let Some(hash) = name.to_str().and_then(|name| name.strip_suffix(kind.suffix()))
                && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                catalog.stored.insert(hash.to_owned(), size);
            }
        }
        Ok(catalog)
    }
}
fn catalog(kind: Kind) -> Option<Arc<Mutex<Catalog>>> {
    static STORES: [OnceLock<Option<Arc<Mutex<Catalog>>>>; 2] = [OnceLock::new(), OnceLock::new()];
    STORES[kind as usize].get_or_init(|| {
        #[cfg(not(test))]
        let root = std::env::var_os("SRP_DATA_DIR").map_or_else(|| PathBuf::from("."), PathBuf::from).join(kind.folder());
        #[cfg(test)]
        let root = std::env::temp_dir().join(format!("skhid-{}-integration-{}", kind.folder(), std::process::id()));
        match Catalog::open(kind, &root) {
            Ok(catalog) => Some(Arc::new(Mutex::new(catalog))),
            Err(error) => { log::error!("Collection {} unavailable: {error}", kind.folder()); None }
        }
    }).clone()
}
pub(super) struct Pending {
    catalog: Arc<Mutex<Catalog>>,
    kind: Kind,
    path: PathBuf,
    file: Option<File>,
    identity: Identity,
    hash: Sha256,
    offset: u32,
    window: [u8; WINDOW as usize],
    received: u16,
    started: Instant,
}
impl Pending {
    fn new(catalog: Arc<Mutex<Catalog>>, identity: Identity, now: Instant) -> io::Result<Self> {
        let mut store = catalog.lock().map_err(|_| io::Error::other("store poisoned"))?;
        if store.active >= MAX_ACTIVE || store.stored.len() + store.active >= store.kind.max_files()
            || store.used.saturating_add(store.reserved).saturating_add(u64::from(identity.size())) > store.kind.limit() {
            return Err(io::Error::other(format!("{} is full", store.kind.folder())));
        }
        store.serial += 1;
        let path = store.root.join(format!("{}.{}.part", std::process::id(), store.serial));
        let mut options = OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        let file = options.open(&path)?;
        store.reserved += u64::from(identity.size()); store.active += 1;
        let kind = store.kind;
        drop(store);
        Ok(Self { catalog, kind, path, file: Some(file), identity, hash: Sha256::new(), offset: 0, window: [0; WINDOW as usize], received: 0, started: now })
    }
    fn data(&mut self, offset: u32, bytes: &[u8]) -> io::Result<Option<Answer>> {
        let end = self.identity.window_end(self.offset).ok_or_else(|| io::Error::other("finished"))?;
        if offset < self.offset { return Ok(Some(Answer::Need(self.offset))); }
        if offset >= end || self.identity.chunk_len(offset) != Some(bytes.len()) { return Ok(None); }
        let at = (offset - self.offset) as usize;
        let bit = 1u16 << (at / CHUNK);
        if self.received & bit != 0 {
            if self.window[at..at+bytes.len()] != *bytes { return Err(io::Error::other("conflicting duplicate")); }
            return Ok(None);
        }
        self.window[at..at+bytes.len()].copy_from_slice(bytes);
        self.received |= bit;
        let len = (end - self.offset) as usize;
        let count = len.div_ceil(CHUNK);
        let complete = (u32::from(self.received) + 1) == (1u32 << count);
        if !complete { return Ok(None); }
        if self.kind == Kind::Executable && self.offset == 0 && !self.window.starts_with(b"MZ") { return Err(io::Error::other("not an executable")); }
        self.file.as_mut().ok_or_else(|| io::Error::other("closed"))?.write_all(&self.window[..len])?;
        self.hash.update(&self.window[..len]);
        self.offset = end; self.received = 0;
        if end < self.identity.size() { return Ok(Some(Answer::Need(end))); }
        if self.hash.clone().finish() != self.identity.sha256 { return Err(io::Error::other("SHA-256 mismatch")); }
        let file = self.file.take().ok_or_else(|| io::Error::other("closed"))?;
        drop(file);
        let mut store = self.catalog.lock().map_err(|_| io::Error::other("store poisoned"))?;
        let hash = self.identity.to_string();
        let destination = store.root.join(format!("{hash}{}", store.kind.suffix()));
        // Atomic publication without overwriting another peer's completed copy.
        match fs::hard_link(&self.path, destination) {
            Ok(()) => { store.used += u64::from(self.identity.size()); store.stored.insert(hash, u64::from(self.identity.size())); }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && store.stored.get(&hash) == Some(&u64::from(self.identity.size())) => {},
            Err(e) => return Err(e),
        }
        Ok(Some(Answer::Stored))
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        drop(self.file.take());
        let removed = fs::remove_file(&self.path);
        if let Ok(mut store) = self.catalog.lock() {
            store.active = store.active.saturating_sub(1);
            store.reserved = store.reserved.saturating_sub(u64::from(self.identity.size()));
            if removed.is_err() { store.used = store.used.saturating_add(u64::from(self.identity.size())); }
        }
    }
}
#[derive(Default)]
pub(super) enum Receiver {
    #[default]
    New,
    Receiving(Box<Pending>),
    Finished { identity: Identity, answer: Answer },
}
impl Receiver {
    pub fn identity(&self) -> Option<Identity> {
        match self { Self::New => None, Self::Receiving(p) => Some(p.identity), Self::Finished { identity, .. } => Some(*identity) }
    }
    pub fn cancel(&mut self) {
        if let Self::Receiving(p) = self { *self = Self::Finished { identity: p.identity, answer: Answer::Refused }; }
    }
    pub fn expire(&mut self, now: Instant) {
        if let Self::Receiving(p) = self && now.saturating_duration_since(p.started) > Duration::from_secs(600) {
            *self = Self::Finished { identity: p.identity, answer: Answer::Refused };
        }
    }
    pub fn receive(&mut self, upload: Upload<'_>, now: Instant, released: Option<Released>) -> Option<([u8; 32], Answer)> {
        self.with_store(upload, now, released, || catalog(Kind::Executable))
    }
    fn with_store(&mut self, upload: Upload<'_>, now: Instant, released: Option<Released>, store: impl FnOnce() -> Option<Arc<Mutex<Catalog>>>) -> Option<([u8; 32], Answer)> {
        self.expire(now);
        match upload {
            Upload::Offer(identity) | Upload::Code(identity) => {
                if matches!(upload, Upload::Offer(_)) && matches!(self, Self::New) && released == Some(Released(identity.sha256)) {
                    // The release's own: as a stored one, and the store is not touched.
                    *self = Self::Finished { identity, answer: Answer::Stored };
                }
                if matches!(self, Self::New) {
                    let result = store().and_then(|store| {
                        let known = store.lock().ok()?.stored.get(&identity.to_string()).copied();
                        if let Some(size) = known {
                            return Some(Self::Finished { identity, answer: if size == u64::from(identity.size()) { Answer::Stored } else { Answer::Refused } });
                        }
                        Pending::new(store, identity, now).ok().map(|p| Self::Receiving(Box::new(p)))
                    });
                    *self = result.unwrap_or(Self::Finished { identity, answer: Answer::Refused });
                }
                if self.identity() != Some(identity) { return None; } // one file per connection
                let answer = match self { Self::Receiving(p) => Answer::Need(p.offset), Self::Finished { answer, .. } => *answer, Self::New => return None };
                Some((identity.sha256, answer))
            }
            Upload::Data { sha256, offset, bytes } => {
                let identity = self.identity().filter(|i| i.sha256 == sha256)?;
                let Self::Receiving(pending) = self else { return None };
                let answer = match pending.data(offset, bytes) {
                    Ok(answer) => answer,
                    Err(error) => { log::warn!("Upload {identity}: {error}"); Some(Answer::Refused) }
                };
                if let Some(answer @ (Answer::Stored | Answer::Refused)) = answer {
                    *self = Self::Finished { identity, answer };
                }
                answer.map(|answer| (sha256, answer))
            }
        }
    }
}

/// A connection's code blocks, one at a time: each offer of another block
/// starts its own transfer (the one before, if not whole, is dropped), up to
/// [`MAX_BLOCKS`].
#[derive(Default)]
pub(super) struct Blocks { current: Receiver, offered: u8 }
impl Blocks {
    /// A code block's offer that starts its transfer (to log once).
    pub fn starts(&self, upload: &Upload<'_>) -> bool {
        match upload {
            Upload::Code(identity) => self.current.identity() != Some(*identity) && self.offered < MAX_BLOCKS,
            _ => false,
        }
    }
    pub fn holds(&self, sha256: [u8; 32]) -> bool { self.current.identity().is_some_and(|i| i.sha256 == sha256) }
    pub fn cancel(&mut self) { self.current.cancel(); }
    pub fn expire(&mut self, now: Instant) { self.current.expire(now); }
    pub fn receive(&mut self, upload: Upload<'_>, now: Instant) -> Option<([u8; 32], Answer)> {
        self.with_store(upload, now, || catalog(Kind::Code))
    }
    fn with_store(&mut self, upload: Upload<'_>, now: Instant, store: impl FnOnce() -> Option<Arc<Mutex<Catalog>>>) -> Option<([u8; 32], Answer)> {
        match upload {
            Upload::Code(identity) if self.starts(&upload) => {
                self.current = Receiver::New;
                self.offered += 1;
                self.current.with_store(Upload::Code(identity), now, None, store)
            }
            Upload::Code(identity) if self.current.identity() != Some(identity) => Some((identity.sha256, Answer::Refused)),
            Upload::Code(_) | Upload::Data { .. } => self.current.with_store(upload, now, None, store),
            Upload::Offer(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    struct Store { path: PathBuf, catalog: Arc<Mutex<Catalog>> }
    impl Store {
        fn new() -> Self {
            Self::of(Kind::Executable)
        }
        fn of(kind: Kind) -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!("skhid-store-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            Self { catalog: Arc::new(Mutex::new(Catalog::open(kind, &path).unwrap())), path }
        }
    }
    impl Drop for Store { fn drop(&mut self) { fs::remove_dir_all(&self.path).unwrap(); } }
    fn identity(bytes: &[u8]) -> Identity { Identity::new(bytes.len() as u32, openssl::sha::sha256(bytes)).unwrap() }
    #[test]
    fn reordered_duplicates_verified_and_deduplicated_across_matches() {
        let store = Store::new(); let now = Instant::now();
        let mut bytes = vec![5; 4 * 1024 * 1024 + 17]; bytes[..2].copy_from_slice(b"MZ");
        let identity = identity(&bytes); let mut receiver = Receiver::New;
        assert_eq!(receiver.with_store(Upload::Offer(identity), now, None, || Some(store.catalog.clone())), Some((identity.sha256, Answer::Need(0))));
        for window in (0..bytes.len()).step_by(WINDOW as usize) {
            let end = bytes.len().min(window + WINDOW as usize);
            for offset in (window..end).step_by(CHUNK).rev() {
                let data = Upload::Data { sha256: identity.sha256, offset: offset as u32, bytes: &bytes[offset..end.min(offset+CHUNK)] };
                receiver.with_store(data, now, None, || None);
                receiver.with_store(data, now, None, || None);
            }
        }
        assert!(matches!(receiver, Receiver::Finished { answer: Answer::Stored, .. }));
        assert_eq!(fs::read(store.path.join(format!("{identity}.exe"))).unwrap(), bytes);
        assert_eq!(fs::read_dir(&store.path).unwrap().count(), 1);
        let reopened = Arc::new(Mutex::new(Catalog::open(Kind::Executable, &store.path).unwrap()));
        let mut next = Receiver::New;
        assert_eq!(next.with_store(Upload::Offer(identity), now, None, || Some(reopened.clone())), Some((identity.sha256, Answer::Stored)));
        assert_eq!(store.catalog.lock().unwrap().reserved, 0);
    }
    #[test]
    fn code_blocks_are_kept_one_after_another_and_at_most_max_blocks() {
        let store = Store::of(Kind::Code); let now = Instant::now();
        let mut blocks = Blocks::default();
        let catalog = || Some(store.catalog.clone());
        let (a, b) = (vec![0xcc; 5000], vec![0x90; 100]);
        let (ia, ib) = (identity(&a), identity(&b));
        let send = |blocks: &mut Blocks, bytes: &[u8], id: Identity| {
            for offset in (0..bytes.len()).step_by(CHUNK) {
                let end = bytes.len().min(offset + CHUNK);
                blocks.with_store(Upload::Data { sha256: id.sha256, offset: offset as u32, bytes: &bytes[offset..end] }, now, catalog);
            }
        };
        // a first block, no MZ: stored as .bin
        assert!(blocks.starts(&Upload::Code(ia)));
        assert_eq!(blocks.with_store(Upload::Code(ia), now, catalog), Some((ia.sha256, Answer::Need(0))));
        assert!(!blocks.starts(&Upload::Code(ia)), "a repeated offer is the same transfer");
        send(&mut blocks, &a, ia);
        assert_eq!(blocks.with_store(Upload::Code(ia), now, catalog), Some((ia.sha256, Answer::Stored)));
        assert_eq!(fs::read(store.path.join(format!("{ia}.bin"))).unwrap(), a);
        // the next one, half sent, then replaced by a third: its partial file goes
        assert_eq!(blocks.with_store(Upload::Code(ib), now, catalog), Some((ib.sha256, Answer::Need(0))));
        assert_eq!(blocks.with_store(Upload::Code(ia), now, catalog), Some((ia.sha256, Answer::Stored)));
        assert_eq!(fs::read_dir(&store.path).unwrap().count(), 1);
        // an executable's offer is not a block's
        assert_eq!(blocks.with_store(Upload::Offer(ib), now, catalog), None);
        // after MAX_BLOCKS offers, another block is refused
        while blocks.offered < MAX_BLOCKS { blocks.offered += 1; }
        assert!(!blocks.starts(&Upload::Code(ib)));
        assert_eq!(blocks.with_store(Upload::Code(ib), now, catalog), Some((ib.sha256, Answer::Refused)));
        assert_eq!(store.catalog.lock().unwrap().reserved, 0);
    }
    #[test]
    fn corrupt_or_interrupted_upload_is_removed_and_reservation_released() {
        for corrupt in [false, true] {
            let store = Store::new(); let now = Instant::now(); let bytes = b"MZ a file";
            let identity = identity(bytes); let mut receiver = Receiver::New;
            receiver.with_store(Upload::Offer(identity), now, None, || Some(store.catalog.clone()));
            if corrupt {
                let answer = receiver.with_store(Upload::Data { sha256: identity.sha256, offset: 0, bytes: b"MZ a fake" }, now, None, || None);
                assert_eq!(answer, Some((identity.sha256, Answer::Refused)));
            }
            drop(receiver);
            assert_eq!(fs::read_dir(&store.path).unwrap().count(), 0);
            assert_eq!(store.catalog.lock().unwrap().reserved, 0);
        }
    }
    #[test]
    fn quota_and_timeout_refuse_without_leaving_partial_files() {
        let store = Store::new(); let now = Instant::now(); let identity = identity(b"MZfile");
        store.catalog.lock().unwrap().used = Kind::Executable.limit();
        let mut receiver = Receiver::New;
        assert_eq!(receiver.with_store(Upload::Offer(identity), now, None, || Some(store.catalog.clone())), Some((identity.sha256, Answer::Refused)));
        store.catalog.lock().unwrap().used = 0;
        let mut receiver = Receiver::New;
        receiver.with_store(Upload::Offer(identity), now, None, || Some(store.catalog.clone()));
        assert_eq!(receiver.with_store(Upload::Offer(identity), now+Duration::from_secs(601), None, || None), Some((identity.sha256, Answer::Refused)));
        assert_eq!(fs::read_dir(&store.path).unwrap().count(), 0);
    }
    #[test]
    fn the_releases_own_executable_is_not_asked_for_and_any_other_is() {
        let store = Store::new(); let now = Instant::now();
        let (own, other) = (identity(b"MZ the release's"), identity(b"MZ another"));
        let hex = own.to_string();
        let released = Released::parse(Some(&hex));
        assert_eq!(released, Some(Released(own.sha256)));
        assert_eq!(Released::parse(Some(&hex.to_uppercase())), released);
        let (signed, long) = (format!("+{}", &hex[1..]), format!("{hex}0"));
        for unread in [None, Some(""), Some("0"), Some(&hex[1..]), Some(signed.as_str()), Some(long.as_str())] {
            assert_eq!(Released::parse(unread), None, "{unread:?}");
        }
        // No store is opened for it, nothing is reserved or written, and a repeated offer is answered the same.
        let mut receiver = Receiver::New;
        for _ in 0..2 {
            assert_eq!(receiver.with_store(Upload::Offer(own), now, released, || panic!("the store is not asked")), Some((own.sha256, Answer::Stored)));
        }
        assert_eq!(receiver.with_store(Upload::Data { sha256: own.sha256, offset: 0, bytes: b"MZ the release's" }, now, released, || None), None);
        assert_eq!(fs::read_dir(&store.path).unwrap().count(), 0);
        // Any other executable is collected as without the setting.
        for released in [released, None] {
            let mut receiver = Receiver::New;
            assert_eq!(receiver.with_store(Upload::Offer(other), now, released, || Some(store.catalog.clone())), Some((other.sha256, Answer::Need(0))));
            assert_eq!(store.catalog.lock().unwrap().active, 1);
        }
        // Without the setting the release's own is collected too.
        let mut receiver = Receiver::New;
        assert_eq!(receiver.with_store(Upload::Offer(own), now, None, || Some(store.catalog.clone())), Some((own.sha256, Answer::Need(0))));
    }
}
