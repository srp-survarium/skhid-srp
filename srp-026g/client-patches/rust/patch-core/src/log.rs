//! A line to `srp_patches.log` in the game's folder (its working folder):
//! what a patch wants the owner to see after the fact, and the panic
//! handler's last word before the crash. No `core::fmt` (its code would
//! take kilobytes of the arena): a line is [`Piece`]s.
//!
//! ```
//! use patch_core::log::{line, Piece::*};
//! line(&[Text("suicide_key: handler "), Hex(0x2000_0000)]);
//! ```
//!
//! On the host (`cargo test`) a line goes nowhere.

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
    let mut line = Line { bytes: [0; MAX], len: 0 };
    line.piece(Piece::Text("srp: "));
    pieces.iter().for_each(|&p| line.piece(p));
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

#[cfg(target_arch = "x86")]
fn write(bytes: &[u8]) {
    use crate::{Import, Va};
    use core::ffi::c_void;
    type Handle = *mut c_void;
    // kernel32's, by the exe's IAT slots (bake_cache's too)
    const CREATE_FILE: Import<
        unsafe extern "stdcall" fn(*const u8, u32, u32, *mut c_void, u32, u32, Handle) -> Handle,
    > = Import::at(Va(0x870048));
    const WRITE_FILE: Import<unsafe extern "stdcall" fn(Handle, *const u8, u32, *mut u32, *mut c_void) -> i32> =
        Import::at(Va(0x870108));
    const CLOSE_HANDLE: Import<unsafe extern "stdcall" fn(Handle) -> i32> = Import::at(Va(0x870040));
    const FILE_APPEND_DATA: u32 = 4;
    const SHARE_READ_WRITE: u32 = 3;
    const OPEN_ALWAYS: u32 = 4;
    const NORMAL: u32 = 0x80;
    let null = core::ptr::null_mut();
    unsafe {
        let file = CREATE_FILE.get()(
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
        WRITE_FILE.get()(file, bytes.as_ptr(), bytes.len() as u32, &mut written, null);
        CLOSE_HANDLE.get()(file);
    }
}

#[cfg(not(target_arch = "x86"))]
fn write(_: &[u8]) {}

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
}
