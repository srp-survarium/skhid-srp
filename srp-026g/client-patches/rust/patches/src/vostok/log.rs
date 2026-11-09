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
//! calls it.

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
    let line = text(pieces);
    write(&line.bytes[..line.len]);
}

/// Appends a line of each part's pieces in turn.
pub fn line_of(parts: &[&[Piece]]) {
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
        let line = text_of(&[pieces, more]);
        write(&line.bytes[..line.len]);
    }
}

fn write(bytes: &[u8]) {
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
        let line = text(&[Piece::Text("at "), Piece::Hex(0xA42D000), Piece::Text(" line "), Piece::Dec(1204)]);
        assert_eq!(&line.bytes[..line.len], b"srp: at 0x0a42d000 line 1204\r\n");
        let long = text(&[Piece::Text(&"x".repeat(500))]);
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
