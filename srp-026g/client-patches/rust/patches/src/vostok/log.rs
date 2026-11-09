//! A line to `srp_patches.log` in the game's folder (its working folder):
//! what a patch wants the owner to see after the fact, and the panic
//! handler's last word before the crash. No `core::fmt` (its code would
//! take kilobytes of the arena): a line is [`Piece`]s.
//!
//! ```
//! use crate::vostok::log::{line, Piece::*};
//! line(&[Text("suicide_key: handler "), Hex(0x2000_0000)]);
//! ```
//!
//! The file is opened once, at the game's first line, and kept open
//! (appends only; others may read, write, rename or delete it): a line is
//! one WriteFile on the calling thread, no longer an open, a write and a
//! close (under Wine the open and the close are each a server call, the
//! open a path look-up too, and the game's thread waited for them line by
//! line). Nothing is held back in memory: a line is in the file (the
//! system's cache) when `line` returns, so the crash reporter, which reads
//! the file's tail after a crash, sees every line up to the crash.
//! - A line the same as the one before it is counted, not written: the next
//!   other line, or every [`Repeats::SAY_EVERY`]th repeat, says how many came.
//! - Past [`LogFile::ROTATE_AT`] bytes (looked at every
//!   [`LogFile::LOOK_EVERY`] bytes this game writes) the file becomes
//!   `srp_patches.old.log` (the one before it replaced) and a new one begins.
//!
//! The patches log on several of the game's threads: the state is atomic,
//! and two threads' lines at once are each whole (an append each).
//!
//! kernel32 writes it, through the exe's imports (`raw`); a test never
//! calls it.

use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use super::files::{Find, FindData, Path};
use super::{GameCell, Thread};
use crate::raw;

/// A part of a line.
#[derive(Clone, Copy, Debug)]
pub enum Piece<'a> {
    Text(&'a str),
    /// `0x` and 8 hex digits.
    Hex(u32),
    Dec(u32),
}

/// The longest line; a longer one is cut.
const MAX: usize = 200;

struct Line {
    bytes: [u8; MAX],
    len: usize,
}

impl Line {
    fn push(&mut self, byte: u8) {
        if let Some(slot) = self.bytes.get_mut(self.len) {
            *slot = byte;
            self.len += 1;
        }
    }

    fn piece(&mut self, piece: Piece) {
        match piece {
            Piece::Text(text) => text.bytes().for_each(|b| self.push(b)),
            Piece::Hex(value) => {
                self.push(b'0');
                self.push(b'x');
                for shift in (0..8).rev() {
                    self.push(b"0123456789abcdef"[(value >> (4 * shift) & 0xF) as usize]);
                }
            }
            Piece::Dec(value) => {
                let mut digits = [0u8; 10];
                let mut n = 0;
                let mut v = value;
                loop {
                    digits[n] = b'0' + (v % 10) as u8;
                    n += 1;
                    v /= 10;
                    if v == 0 {
                        break;
                    }
                }
                digits[..n].iter().rev().for_each(|&d| self.push(d));
            }
        }
    }

    fn text(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// The line `srp: <pieces>` and a line break, as text.
fn text(pieces: &[Piece]) -> Line {
    text_of(&[pieces])
}

/// The line `srp: ` and each part's pieces, and a line break, as text.
fn text_of(parts: &[&[Piece]]) -> Line {
    let mut line = Line { bytes: [0; MAX], len: 0 };
    line.piece(Piece::Text("srp: "));
    parts.iter().flat_map(|part| part.iter()).for_each(|&p| line.piece(p));
    line.len = line.len.min(MAX - 2);
    line.push(b'\r');
    line.push(b'\n');
    line
}

/// Appends a line to `srp_patches.log`.
pub fn line(pieces: &[Piece]) {
    say(&text(pieces));
}

/// Appends a line of each part's pieces in turn.
pub fn line_of(parts: &[&[Piece]]) {
    say(&text_of(parts));
}

/// The refusals' lines: at most one a second, each counting the refusals
/// since the one before it that got none, so a flood cannot fill the disk
/// and no refusal goes unseen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusals {
    Quiet,
    /// A line at `at` (GetTickCount's ms); `unlogged` refusals after it.
    Logged { at: u32, unlogged: u32 },
}

impl Refusals {
    const EVERY_MS: u32 = 1000;

    /// A refusal at `now`: the next state, and `Some(n)` when it is logged,
    /// `n` being the refusals since the last line that were not.
    fn refuse(self, now: u32) -> (Self, Option<u32>) {
        match self {
            Self::Logged { at, unlogged } if now.wrapping_sub(at) < Self::EVERY_MS => {
                (Self::Logged { at, unlogged: unlogged.saturating_add(1) }, None)
            }
            Self::Logged { unlogged, .. } => (Self::Logged { at: now, unlogged: 0 }, Some(unlogged)),
            Self::Quiet => (Self::Logged { at: now, unlogged: 0 }, Some(0)),
        }
    }
}

static REFUSALS: GameCell<Refusals> = GameCell::new(Refusals::Quiet);

/// A refusal's line to `srp_patches.log`, under [`Refusals`]' limit.
pub fn refusal(thread: &Thread, pieces: &[Piece]) {
    let now = super::files::tick_count();
    let (next, logged) = REFUSALS.get(thread).refuse(now);
    REFUSALS.set(thread, next);
    if let Some(unlogged) = logged {
        let more: &[Piece] = if unlogged == 0 {
            &[]
        } else {
            &[Piece::Text("; "), Piece::Dec(unlogged), Piece::Text(" more refused since the last line")]
        };
        say(&text_of(&[pieces, more]));
    }
}

// ---- the same line again ------------------------------------------------------------------------

/// FNV-1a of a line's bytes: what tells a line from the one before it.
fn hash(bytes: &[u8]) -> u32 {
    // Its offset basis 811C9DC5h and prime 1000193h, in halves (the layer
    // check takes a literal past FFFFh for a game address).
    const BASIS: u32 = (0x811C << 16) | 0x9DC5;
    const PRIME: u32 = (1 << 24) | 0x193;
    bytes.iter().fold(BASIS, |h, &b| (h ^ u32::from(b)).wrapping_mul(PRIME))
}

/// What [`Repeats::see`] makes of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seen {
    /// Another line than the one before: written, after a line saying how
    /// many repeats of the one before went unwritten (when any did).
    New { repeated: u32 },
    /// The line before again: not written; `Some(n)` says the `n` repeats
    /// so far now (and counts again from none).
    Again { say: Option<u32> },
}

/// The last line's hash (`| 1`: 0 is "no line yet") and how many times it
/// came again since it was written.
struct Repeats {
    last: AtomicU32,
    count: AtomicU32,
}

impl Repeats {
    /// A run of one line is said every this many repeats, so a crash in the
    /// middle of one still shows it in the log.
    const SAY_EVERY: u32 = 1000;

    const fn new() -> Repeats {
        Repeats { last: AtomicU32::new(0), count: AtomicU32::new(0) }
    }

    fn see(&self, hash: u32) -> Seen {
        let key = hash | 1;
        if self.last.swap(key, Ordering::AcqRel) != key {
            return Seen::New { repeated: self.count.swap(0, Ordering::AcqRel) };
        }
        let n = self.count.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        let say = n >= Self::SAY_EVERY
            && self.count.compare_exchange(n, 0, Ordering::AcqRel, Ordering::Relaxed).is_ok();
        Seen::Again { say: say.then_some(n) }
    }
}

static REPEATS: Repeats = Repeats::new();

/// `srp: (the line above: N more times)`.
fn repeated(n: u32) -> Line {
    text(&[Piece::Text("(the line above: "), Piece::Dec(n), Piece::Text(" more times)")])
}

/// A line to the file, unless it is the line before it again.
fn say(line: &Line) {
    match REPEATS.see(hash(line.text())) {
        Seen::New { repeated: 0 } => FILE.append(line.text()),
        Seen::New { repeated: n } => {
            FILE.append(repeated(n).text());
            FILE.append(line.text());
        }
        Seen::Again { say: Some(n) } => FILE.append(repeated(n).text()),
        Seen::Again { say: None } => {}
    }
}

// ---- the file ---------------------------------------------------------------------------------

const NAME: &[u8] = b"srp_patches.log\0";
const OLD_NAME: &[u8] = b"srp_patches.old.log\0";

/// Whether `n` bytes written after `before` reach the next multiple of
/// `every` (the byte count wraps at 4 GiB).
fn crosses(before: u32, n: u32, every: u32) -> bool {
    before / every != before.wrapping_add(n) / every
}

/// The file this game writes: its handle once open, the bytes this game
/// wrote, and whether a rename is under way.
struct LogFile {
    /// CreateFileA's handle; 0 before the first line (a file's handle is
    /// never 0) and when the open failed (the next line opens again).
    handle: AtomicUsize,
    written: AtomicU32,
    rotating: AtomicBool,
}

static FILE: LogFile = LogFile::new();

impl LogFile {
    /// The file's size past which it is renamed (the crash reporter takes
    /// its last 2 MiB).
    const ROTATE_AT: u32 = 8 << 20;
    /// The size is looked at each time this game has written this much more.
    const LOOK_EVERY: u32 = 256 << 10;

    const fn new() -> LogFile {
        LogFile { handle: AtomicUsize::new(0), written: AtomicU32::new(0), rotating: AtomicBool::new(false) }
    }

    /// The open file: opened at the first line. Of two threads opening it
    /// at once, the first to store its handle keeps it; the other closes
    /// its own.
    fn handle(&self) -> Option<*mut c_void> {
        match self.handle.load(Ordering::Acquire) {
            0 => {
                let opened = open()?;
                match self.handle.compare_exchange(0, opened as usize, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => Some(opened),
                    Err(theirs) => {
                        close(opened);
                        Some(theirs as *mut c_void)
                    }
                }
            }
            open => Some(open as *mut c_void),
        }
    }

    fn append(&self, bytes: &[u8]) {
        let Some(file) = self.handle() else { return };
        write(file, bytes);
        let n = bytes.len() as u32;
        let before = self.written.fetch_add(n, Ordering::Relaxed);
        if crosses(before, n, Self::LOOK_EVERY) {
            self.look();
        }
    }

    /// Past ROTATE_AT (another game may write it too): the file renamed and
    /// a new one opened in its place. The handle before is kept, never
    /// closed: a thread may be writing through it now, and a closed
    /// handle's number may be another file's by then (one handle a rename,
    /// a few a game). A rename refused (a reader without delete sharing) is
    /// tried again at the next look.
    fn look(&self) {
        if size().is_none_or(|size| size < Self::ROTATE_AT) || self.rotating.swap(true, Ordering::Acquire) {
            return;
        }
        if rename() {
            self.handle.store(open().map_or(0, |file| file as usize), Ordering::Release);
        }
        self.rotating.store(false, Ordering::Release);
    }
}

/// The file opened for appends (made when it is not there).
fn open() -> Option<*mut c_void> {
    const FILE_APPEND_DATA: u32 = 4;
    const SHARE_ALL: u32 = 7;
    const OPEN_ALWAYS: u32 = 4;
    const NORMAL: u32 = 0x80;
    let null = core::ptr::null_mut::<c_void>();
    // SAFETY: kernel32's CreateFileA through the exe's import slot, which the
    // loader filled before the game ran; a NUL-terminated name.
    let file = unsafe {
        (raw::engine_create_file_a)(NAME.as_ptr(), FILE_APPEND_DATA, SHARE_ALL, null, OPEN_ALWAYS, NORMAL, null)
    };
    (file as isize != -1 && !file.is_null()).then_some(file)
}

fn write(file: *mut c_void, bytes: &[u8]) {
    let mut written = 0;
    // SAFETY: WriteFile through the exe's import slot on a handle CreateFileA
    // gave (never closed while kept), our bytes.
    unsafe {
        (raw::engine_write_file)(file, bytes.as_ptr().cast(), bytes.len() as u32, &mut written, core::ptr::null_mut());
    }
}

fn close(file: *mut c_void) {
    // SAFETY: a handle CreateFileA gave that nothing else holds.
    unsafe {
        (raw::engine_close_handle)(file);
    }
}

/// The file's size on disk (FindFirstFileA's record of it).
fn size() -> Option<u32> {
    let mut data = FindData::new();
    let _found = Find::first(Path::new(NAME)?, &mut data)?;
    data.size()
}

/// The file renamed srp_patches.old.log, that one replaced: whether it was.
fn rename() -> bool {
    const MOVEFILE_REPLACE_EXISTING: u32 = 1;
    // SAFETY: kernel32 is loaded in every process; GetModuleHandleA and
    // GetProcAddress through the exe's import slots, NUL-terminated names;
    // MoveFileExA is that export (the library's dynamic row).
    unsafe {
        let kernel32 = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
        if kernel32.is_null() {
            return false;
        }
        let Some(move_file) = NonNull::new((raw::engine_get_proc_address)(kernel32, c"MoveFileExA".as_ptr().cast())) else {
            return false;
        };
        raw::engine_move_file_ex_a(move_file, NAME.as_ptr(), OLD_NAME.as_ptr(), MOVEFILE_REPLACE_EXISTING) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_of_pieces() {
        let line = text(&[Piece::Text("at "), Piece::Hex(0xA42D000), Piece::Text(" line "), Piece::Dec(1204)]);
        assert_eq!(line.text(), b"srp: at 0x0a42d000 line 1204\r\n");
        let long = text(&[Piece::Text(&"x".repeat(500))]);
        assert_eq!(long.len, MAX);
        assert_eq!(&long.bytes[MAX - 2..], b"\r\n");
        assert_eq!(&text(&[Piece::Dec(0)]).bytes[5..6], b"0");
        assert_eq!(repeated(37).text(), b"srp: (the line above: 37 more times)\r\n");
    }

    #[test]
    fn a_refusal_line_a_second_counts_the_ones_between() {
        let (mut state, logged) = Refusals::Quiet.refuse(u32::MAX - 100);
        assert_eq!(logged, Some(0));
        for ms in [0, 200, 800] {
            let (next, logged) = state.refuse(ms);
            assert_eq!(logged, None);
            state = next;
        }
        assert_eq!(state.refuse(900).1, Some(3));
    }

    #[test]
    fn a_repeated_line_is_counted_and_said_before_the_next() {
        let repeats = Repeats::new();
        let (a, b) = (hash(b"srp: a\r\n"), hash(b"srp: b\r\n"));
        assert_ne!(a, b);
        assert_eq!(repeats.see(a), Seen::New { repeated: 0 });
        assert_eq!(repeats.see(a), Seen::Again { say: None });
        assert_eq!(repeats.see(a), Seen::Again { say: None });
        assert_eq!(repeats.see(b), Seen::New { repeated: 2 });
        assert_eq!(repeats.see(a), Seen::New { repeated: 0 });
        // A hash of 0 is a line too, not "none yet".
        let zero = Repeats::new();
        assert_eq!(zero.see(0), Seen::New { repeated: 0 });
    }

    #[test]
    fn a_long_run_is_said_every_so_often() {
        let repeats = Repeats::new();
        repeats.see(7);
        for _ in 1..Repeats::SAY_EVERY {
            assert_eq!(repeats.see(7), Seen::Again { say: None });
        }
        assert_eq!(repeats.see(7), Seen::Again { say: Some(Repeats::SAY_EVERY) });
        assert_eq!(repeats.see(7), Seen::Again { say: None });
        assert_eq!(repeats.see(8), Seen::New { repeated: 1 });
    }

    #[test]
    fn the_size_is_looked_at_once_a_step() {
        let every = LogFile::LOOK_EVERY;
        assert!(!crosses(0, 200, every));
        assert!(crosses(every - 100, 200, every));
        assert!(!crosses(every, 200, every));
        assert!(crosses(u32::MAX - 10, 200, every));
    }
}
