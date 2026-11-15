//! `skhid_torrent.dll` (libtorrent-rasterbar 2.0 behind the C API of
//! `client/updater/torrent-dll/skt.h`): its one owner. The DLL is loaded
//! from beside the updater at run time, once; no DLL, or one that does not
//! load (Windows 7, a file an antivirus took) or lacks a function: no
//! torrent, HTTPS as without it. Every raw call is here; what comes back is
//! parsed once into the types below.

use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{CStr, c_char};
use core::ptr::NonNull;
use std::ffi::CString;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};

use updater_logic::newest::Item;

use crate::win;

/// Its name, beside the updater.
pub const DLL: &str = "skhid_torrent.dll";

#[repr(C)]
struct RawConfig {
    listen_port: u16,
    upload_limit: i32,
    dht_bootstrap: *const c_char,
    loopback: i32,
}

#[repr(C)]
struct RawFile {
    name: *const c_char,
    path: *const c_char,
    fetch: i32,
    trusted: i32,
}

#[repr(C)]
struct RawEvent {
    kind: i32,
    torrent: i32,
    a: i64,
    text: [u8; 160],
    sig: [u8; 64],
    len: i32,
    data: [u8; 1000],
}

#[repr(C)]
struct RawStatus {
    checking: i32,
    progress_ppm: i32,
    wanted: i64,
    wanted_done: i64,
    peers: i32,
    uploaded: i64,
    error: [u8; 160],
}

enum RawSession {}
type S = *mut RawSession;

/// The DLL's functions (skt.h), each by its name.
macro_rules! library {
    ($($field:ident = $name:literal: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        struct Library {
            $($field: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
        }

        fn functions(module: HMODULE) -> Result<Library, String> {
            Ok(Library {
                $($field: {
                    let address = unsafe { GetProcAddress(module, concat!($name, "\0").as_ptr()) }
                        .ok_or_else(|| alloc::format!("{DLL} has no {}", $name))?;
                    // SAFETY: the DLL exports it with skt.h's signature,
                    // which is the field's.
                    unsafe {
                        core::mem::transmute::<unsafe extern "system" fn() -> isize, unsafe extern "C" fn($($arg),*) $(-> $ret)?>(address)
                    }
                },)*
            })
        }
    };
}

library! {
    version = "skt_version": fn() -> *const c_char;
    session_new = "skt_session_new": fn(*const RawConfig, *mut c_char, i32) -> S;
    session_free = "skt_session_free": fn(S);
    listen = "skt_listen": fn(S, u16);
    set_upload_limit = "skt_set_upload_limit": fn(S, i32);
    ban = "skt_ban": fn(S, *const c_char) -> i32;
    add = "skt_add": fn(S, *const u8, usize, *const c_char, *const RawFile, usize, *const *const c_char, usize, *mut c_char, i32) -> i32;
    add_magnet = "skt_add_magnet": fn(S, *const u8, *const c_char, *const *const c_char, usize, *mut c_char, i32) -> i32;
    info = "skt_info": fn(S, i32, *mut u8, i32) -> i32;
    status_of = "skt_status_of": fn(S, i32, *mut RawStatus) -> i32;
    file_progress = "skt_file_progress": fn(S, i32, *mut i64, usize) -> i32;
    remove = "skt_remove": fn(S, i32) -> i32;
    poll = "skt_poll": fn(S, *mut RawEvent, i32, i32) -> i32;
    dht_get = "skt_dht_get": fn(S, *const u8, *const u8, usize);
    dht_put = "skt_dht_put": fn(S, *const u8, *const u8, i64, *const u8, usize, *const u8, usize) -> i32;
}

fn load(dir: &str) -> Result<Library, String> {
    let path = alloc::format!("{dir}\\{DLL}");
    if !win::exists(&path) {
        return Err(alloc::format!("no {DLL}"));
    }
    let wide = win::wide(&path);
    // Its own folder and the system's only: nothing of the current
    // folder's or the PATH's.
    let module = unsafe {
        LoadLibraryExW(wide.as_ptr(), core::ptr::null_mut(), LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32)
    };
    if module.is_null() {
        return Err(alloc::format!("{DLL} cannot be loaded: {}", win::error_text(win::last_error())));
    }
    functions(module)
}

static LIBRARY: OnceLock<Result<Library, String>> = OnceLock::new();

fn library(dir: &str) -> Result<&'static Library, String> {
    LIBRARY.get_or_init(|| load(dir)).as_ref().map_err(String::clone)
}

fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// A text for the DLL: without a zero byte inside (none of ours has one).
fn c(text: &str) -> CString {
    CString::new(text.replace('\0', "")).expect("no zero byte")
}

/// A torrent of the session (the id `add` gave).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TorrentId(i32);

/// How a session is made.
pub struct Config<'a> {
    /// `None`: the system picks one.
    pub port: Option<u16>,
    /// Bytes a second, every peer's.
    pub upload_limit: u64,
    pub dht_bootstrap: &'a str,
    /// Tests: 127.0.0.1 only.
    pub loopback: bool,
}

/// What this run does with a file of a torrent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Fetched into its path; what is there checked first.
    Fetch,
    /// Shared from its path, never written. `trusted`: checked before and
    /// not changed since (`updater_logic::shared`): not hashed again.
    Share { trusted: bool },
}

pub struct FileSpec<'a> {
    /// Its path in the torrent.
    pub name: &'a str,
    /// Where it is.
    pub path: &'a str,
    pub role: Role,
}

#[derive(Debug, Clone)]
pub enum Event {
    Listening(u16),
    ListenFailed { port: u16, why: String },
    HashFailed { piece: i64 },
    /// A peer that sent a piece that failed: banned in its torrent.
    Banned { ip: String },
    /// A torrent's files checked.
    Checked(TorrentId),
    /// A magnet's info came ([`Session::info`]).
    Metadata(TorrentId),
    Error { text: String },
    DhtItem(Item),
    DhtPut { nodes: i64 },
}

#[derive(Debug, Clone, Default)]
pub struct Status {
    /// Its files being checked: how much of them, in millionths.
    pub checking: Option<u32>,
    pub wanted: u64,
    pub wanted_done: u64,
    pub peers: u32,
    pub uploaded: u64,
    pub error: Option<String>,
}

/// A libtorrent session: stopped (and waited for) when dropped.
pub struct Session {
    library: &'static Library,
    raw: NonNull<RawSession>,
}

impl Session {
    /// A session, when the DLL beside the updater (`dir`) loads.
    pub fn new(dir: &str, config: &Config) -> Result<Self, String> {
        let library = library(dir)?;
        let bootstrap = c(config.dht_bootstrap);
        let raw = RawConfig {
            listen_port: config.port.unwrap_or(0),
            upload_limit: i32::try_from(config.upload_limit).unwrap_or(i32::MAX),
            dht_bootstrap: bootstrap.as_ptr(),
            loopback: i32::from(config.loopback),
        };
        let mut error = [0_u8; 160];
        let session = unsafe { (library.session_new)(&raw, error.as_mut_ptr().cast(), 160) };
        let raw = NonNull::new(session).ok_or_else(|| alloc::format!("{DLL}: {}", text(&error)))?;
        Ok(Self { library, raw })
    }

    pub fn version(&self) -> String {
        let version = unsafe { (self.library.version)() };
        if version.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(version) }.to_string_lossy().into_owned()
    }

    fn s(&self) -> S {
        self.raw.as_ptr()
    }

    /// Listens on another port; a [`Event::Listening`] or
    /// [`Event::ListenFailed`] follows.
    pub fn listen(&mut self, port: Option<u16>) {
        unsafe { (self.library.listen)(self.s(), port.unwrap_or(0)) }
    }

    pub fn set_upload_limit(&mut self, bytes_per_second: u64) {
        unsafe { (self.library.set_upload_limit)(self.s(), i32::try_from(bytes_per_second).unwrap_or(i32::MAX)) }
    }

    /// No connection to or from `ip` for the session.
    pub fn ban(&mut self, ip: &str) {
        let ip = c(ip);
        unsafe { (self.library.ban)(self.s(), ip.as_ptr()) };
    }

    /// A torrent (its bytes) with `files`, the others neither fetched nor
    /// shared; `nowhere`: a folder where nothing of it is; `trackers`: the
    /// only ones asked.
    pub fn add(&mut self, torrent: &[u8], nowhere: &str, files: &[FileSpec], trackers: &[String]) -> Result<TorrentId, String> {
        let names: Vec<(CString, CString)> = files.iter().map(|file| (c(file.name), c(file.path))).collect();
        let raw: Vec<RawFile> = files
            .iter()
            .zip(&names)
            .map(|(file, (name, path))| RawFile {
                name: name.as_ptr(),
                path: path.as_ptr(),
                fetch: i32::from(file.role == Role::Fetch),
                trusted: i32::from(file.role == Role::Share { trusted: true }),
            })
            .collect();
        let trackers: Vec<CString> = trackers.iter().map(|tracker| c(tracker)).collect();
        let tracker_pointers: Vec<*const c_char> = trackers.iter().map(|tracker| tracker.as_ptr()).collect();
        let nowhere = c(nowhere);
        let mut error = [0_u8; 160];
        let id = unsafe {
            (self.library.add)(
                self.s(),
                torrent.as_ptr(),
                torrent.len(),
                nowhere.as_ptr(),
                raw.as_ptr(),
                raw.len(),
                tracker_pointers.as_ptr(),
                tracker_pointers.len(),
                error.as_mut_ptr().cast(),
                160,
            )
        };
        if id < 0 { Err(text(&error)) } else { Ok(TorrentId(id)) }
    }

    /// The torrent of v2 info hash `v2`, for its info only (from its peers:
    /// [`Event::Metadata`], then [`Session::info`]).
    pub fn add_magnet(&mut self, v2: &[u8; 32], nowhere: &str, trackers: &[String]) -> Result<TorrentId, String> {
        let trackers: Vec<CString> = trackers.iter().map(|tracker| c(tracker)).collect();
        let tracker_pointers: Vec<*const c_char> = trackers.iter().map(|tracker| tracker.as_ptr()).collect();
        let nowhere = c(nowhere);
        let mut error = [0_u8; 160];
        let id = unsafe {
            (self.library.add_magnet)(
                self.s(),
                v2.as_ptr(),
                nowhere.as_ptr(),
                tracker_pointers.as_ptr(),
                tracker_pointers.len(),
                error.as_mut_ptr().cast(),
                160,
            )
        };
        if id < 0 { Err(text(&error)) } else { Ok(TorrentId(id)) }
    }

    /// A magnet's info, once it came.
    pub fn info(&mut self, torrent: TorrentId) -> Option<Vec<u8>> {
        let n = unsafe { (self.library.info)(self.s(), torrent.0, core::ptr::null_mut(), 0) };
        let length = usize::try_from(n.checked_neg()?).ok().filter(|&n| n > 0)?;
        let mut out = alloc::vec![0_u8; length];
        let got = unsafe { (self.library.info)(self.s(), torrent.0, out.as_mut_ptr(), n.checked_neg()?) };
        (usize::try_from(got).ok() == Some(length)).then_some(out)
    }

    pub fn status(&mut self, torrent: TorrentId) -> Option<Status> {
        let mut raw: RawStatus = unsafe { core::mem::zeroed() };
        if unsafe { (self.library.status_of)(self.s(), torrent.0, &mut raw) } != 0 {
            return None;
        }
        let error = text(&raw.error);
        Some(Status {
            checking: (raw.checking != 0).then_some(u32::try_from(raw.progress_ppm).unwrap_or(0)),
            wanted: u64::try_from(raw.wanted).unwrap_or(0),
            wanted_done: u64::try_from(raw.wanted_done).unwrap_or(0),
            peers: u32::try_from(raw.peers).unwrap_or(0),
            uploaded: u64::try_from(raw.uploaded).unwrap_or(0),
            error: (!error.is_empty()).then_some(error),
        })
    }

    /// The bytes checked of each file `add` was given, in its order.
    pub fn file_progress(&mut self, torrent: TorrentId, files: usize) -> Option<Vec<u64>> {
        let mut out = alloc::vec![0_i64; files];
        if unsafe { (self.library.file_progress)(self.s(), torrent.0, out.as_mut_ptr(), files) } != 0 {
            return None;
        }
        Some(out.into_iter().map(|done| u64::try_from(done).unwrap_or(0)).collect())
    }

    /// The torrent goes (its files stay).
    pub fn remove(&mut self, torrent: TorrentId) {
        unsafe { (self.library.remove)(self.s(), torrent.0) };
    }

    /// The events since the last look, waiting up to `wait_ms` for one.
    pub fn poll(&mut self, wait_ms: u32) -> Vec<Event> {
        let mut raw: Vec<RawEvent> = (0..16).map(|_| unsafe { core::mem::zeroed() }).collect();
        let mut events = Vec::new();
        let mut wait = i32::try_from(wait_ms).unwrap_or(i32::MAX);
        loop {
            let n = unsafe { (self.library.poll)(self.s(), raw.as_mut_ptr(), 16, wait) };
            let Ok(n) = usize::try_from(n) else { break };
            events.extend(raw[..n].iter().filter_map(parse_event));
            if n < raw.len() {
                break;
            }
            wait = 0;
        }
        events
    }

    /// Asks the DHT for the item of `key` and `salt` ([`Event::DhtItem`]s).
    pub fn dht_get(&mut self, key: &[u8; 32], salt: &[u8]) {
        unsafe { (self.library.dht_get)(self.s(), key.as_ptr(), salt.as_ptr(), salt.len()) }
    }

    /// Puts a signed item, as it is ([`Event::DhtPut`] once stored).
    pub fn dht_put(&mut self, key: &[u8; 32], salt: &[u8], item: &Item) {
        unsafe {
            (self.library.dht_put)(
                self.s(),
                key.as_ptr(),
                item.signature.as_ptr(),
                item.seq,
                salt.as_ptr(),
                salt.len(),
                item.value.as_ptr(),
                item.value.len(),
            )
        };
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { (self.library.session_free)(self.s()) }
    }
}

fn torrent_of(id: i32) -> Option<TorrentId> {
    (id >= 0).then_some(TorrentId(id))
}

fn parse_event(raw: &RawEvent) -> Option<Event> {
    let port = || u16::try_from(raw.a).unwrap_or(0);
    Some(match raw.kind {
        1 => Event::Listening(port()),
        2 => Event::ListenFailed { port: port(), why: text(&raw.text) },
        3 => Event::HashFailed { piece: raw.a },
        4 => Event::Banned { ip: text(&raw.text) },
        5 => Event::Checked(torrent_of(raw.torrent)?),
        7 => Event::Metadata(torrent_of(raw.torrent)?),
        8 => Event::Error { text: text(&raw.text) },
        9 => Event::DhtItem(Item {
            seq: raw.a,
            signature: raw.sig,
            value: raw.data[..usize::try_from(raw.len).ok().filter(|&n| n <= raw.data.len())?].to_vec(),
        }),
        10 => Event::DhtPut { nodes: raw.a },
        _ => return None,
    })
}

