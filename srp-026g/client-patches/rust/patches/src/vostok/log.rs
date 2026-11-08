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
//! kernel32 writes it, through the exe's imports (`raw`); a test never
//! calls it. The game's threads all log (hooks run on the main, logic and
//! resource threads): a line goes out whole, one at a time ([`WRITING`]),
//! so two lines never race for the file's end.

use core::sync::atomic::{AtomicU32, Ordering};

use super::{Lock, Thread};
use crate::raw;

/// A part of a line.
#[derive(Clone, Copy, Debug)]
pub enum Piece<'a> {
    Text(&'a str),
    /// `0x` and 8 hex digits.
    Hex(u32),
    Dec(u32),
}

/// The longest line; a longer one is cut (a bake's name alone is up to 260).
const MAX: usize = 512;

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
    parts(&[pieces]);
}

/// Appends a line of parts (each some pieces) to `srp_patches.log`.
pub fn parts(parts: &[&[Piece]]) {
    let line = text_of(parts);
    write(&line.bytes[..line.len]);
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

/// Lines of one kind under [`Refusals`]' limit: at most one a second, each
/// counting the ones since the last that were not logged.
pub struct Limited {
    state: Lock<Refusals>,
    /// What the count after a line names ("refused", "postponed").
    what: &'static str,
}

impl Limited {
    pub const fn new(what: &'static str) -> Limited {
        Limited { state: Lock::new(Refusals::Quiet), what }
    }

    /// The line, unless one went out less than a second ago.
    pub fn line(&self, thread: &Thread, pieces: &[Piece]) {
        self.parts(thread, &[pieces]);
    }

    /// A line of parts (each some pieces), unless one went out less than a
    /// second ago.
    pub fn parts(&self, thread: &Thread, parts: &[&[Piece]]) {
        let now = super::files::tick_count();
        let logged = self.state.with(thread, |state| {
            let (next, logged) = state.refuse(now);
            *state = next;
            logged
        });
        if let Some(unlogged) = logged {
            let more: &[Piece] = if unlogged == 0 {
                &[]
            } else {
                &[Piece::Text("; "), Piece::Dec(unlogged), Piece::Text(" more "), Piece::Text(self.what),
                  Piece::Text(" since the last line")]
            };
            let mut all: [&[Piece]; 8] = [&[]; 8];
            let n = parts.len().min(all.len() - 1);
            all[..n].copy_from_slice(&parts[..n]);
            all[n] = more;
            let line = text_of(&all[..=n]);
            write(&line.bytes[..line.len]);
        }
    }
}

/// The patches' refusals (a packet, a world, an effect the game was given).
static REFUSALS: Limited = Limited::new("refused");

/// A refusal's line to `srp_patches.log`, under [`Refusals`]' limit.
pub fn refusal(thread: &Thread, pieces: &[Piece]) {
    REFUSALS.line(thread, pieces);
}

/// The thread writing a line (its id), 0 when none is. Not a [`Lock`]: the
/// panic handler logs too, and a panic inside a write (on the thread that
/// holds it) must still get its line out, not panic again.
static WRITING: AtomicU32 = AtomicU32::new(0);

/// `bytes` appended to the log, the only write at that moment.
fn write(bytes: &[u8]) {
    let me = super::sync::thread_id();
    let mut owner = WRITING.compare_exchange(0, me, Ordering::Acquire, Ordering::Relaxed);
    while let Err(holder) = owner {
        if holder == me {
            // the panic handler, inside this thread's own write
            return append(bytes);
        }
        core::hint::spin_loop();
        owner = WRITING.compare_exchange(0, me, Ordering::Acquire, Ordering::Relaxed);
    }
    append(bytes);
    WRITING.store(0, Ordering::Release);
}

/// `bytes` appended to `srp_patches.log`: opened, one WriteFile, closed.
fn append(bytes: &[u8]) {
    use core::ffi::c_void;
    const FILE_APPEND_DATA: u32 = 4;
    const SHARE_READ_WRITE: u32 = 3;
    const OPEN_ALWAYS: u32 = 4;
    const NORMAL: u32 = 0x80;
    let null = core::ptr::null_mut::<c_void>();
    // SAFETY: kernel32's CreateFileA, WriteFile and CloseHandle through the
    // exe's import slots, which the loader filled before the game ran; the
    // arguments are what each takes (a NUL-terminated name, our bytes).
    unsafe {
        let file = (raw::engine_create_file_a)(
            c"srp_patches.log".as_ptr().cast(),
            FILE_APPEND_DATA,
            SHARE_READ_WRITE,
            null,
            OPEN_ALWAYS,
            NORMAL,
            null,
        );
        if file as isize == -1 {
            return;
        }
        let mut written = 0;
        (raw::engine_write_file)(file, bytes.as_ptr().cast(), bytes.len() as u32, &mut written, null);
        (raw::engine_close_handle)(file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_of_pieces() {
        let text = |pieces: &[Piece]| text_of(&[pieces]);
        let line = text(&[Piece::Text("at "), Piece::Hex(0xA42D000), Piece::Text(" line "), Piece::Dec(1204)]);
        assert_eq!(&line.bytes[..line.len], b"srp: at 0x0a42d000 line 1204\r\n");
        let long = text(&[Piece::Text(&"x".repeat(1000))]);
        assert_eq!(long.len, MAX);
        assert_eq!(&long.bytes[MAX - 2..], b"\r\n");
        assert_eq!(&text(&[Piece::Dec(0)]).bytes[5..6], b"0");
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
}
