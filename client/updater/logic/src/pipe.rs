//! The messages between the 0.26g launcher and its updater, as the launcher
//! speaks them on its named pipe (`client-patches/launcher/README.md`, "The
//! pipe"; read from `survarium_launcher.exe` and `survarium_updater.exe` of
//! build 2777).
//!
//! A message on the wire is a 16-bit length (little-endian) and that many
//! bytes; the first of them is the message's id. Numbers are little-endian.
//! A narrow string is a length byte and its bytes; a wide string is a 16-bit
//! count of UTF-16 units and the units.

use alloc::vec::Vec;

/// The updater's states the launcher knows (`02 <state>`). Only the ones
/// this updater reports are named.
pub mod state {
    /// Stopped by an error; the launcher shows its error state.
    pub const ERROR: u8 = 0x01;
    /// Asking the server which version it offers.
    pub const CHECKING: u8 = 0x04;
    /// The question failed; it is asked again.
    pub const CHECK_FAILED: u8 = 0x05;
    pub const DOWNLOADING: u8 = 0x09;
    /// The download failed; it is tried again.
    pub const DOWNLOAD_FAILED: u8 = 0x0A;
    pub const DOWNLOADED: u8 = 0x0C;
    /// A command line was handed to the launcher to run (after `03`).
    pub const COMMAND_LINE: u8 = 0x0D;
    /// Nothing to do: the game may be started.
    pub const LATEST: u8 = 0x0E;
    /// The launcher asked to stop, and the updater has.
    pub const ABORTED: u8 = 0x0F;
    pub const INSTALLING: u8 = 0x10;
    pub const INSTALLED: u8 = 0x11;
    /// The original's `copy` action is done.
    pub const COPIED: u8 = 0x13;
    /// "Server maintenance...": the launcher turns PLAY off (and Enter with
    /// it) until another updater says [`LATEST`] (`on_updater_state_changed`
    /// 0x1612B, `srp-026g/docs/notes/launcher.md` 5.4). Said when the server
    /// stops answering while the launcher stays open.
    pub const MAINTENANCE: u8 = 0x16;
}

/// What an install step is, for [`ToLauncher::InstallInfo`] (ids 06..0D).
pub mod step {
    pub const EXTRACT: u8 = 1;
    pub const MOVE: u8 = 5;
    pub const INFO: u8 = 7;
}

/// The kinds of [`ToLauncher::DiskSpace`].
pub mod disk {
    pub const FULL: u8 = 0;
    pub const CANNOT_WRITE: u8 = 3;
}

/// A message of the updater.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToLauncher<'a> {
    /// `01`: bytes done and in all, bytes a second, seconds left
    /// (`u64::MAX` for unknown).
    Progress {
        done: u64,
        total: u64,
        speed: u64,
        eta: u64,
    },
    /// `02`.
    State(u8),
    /// `03`: a command line for the launcher to run.
    CommandLine(&'a str),
    /// `04`: the disk is full or cannot be written: the drive or folder,
    /// the bytes free, the kind ([`disk`]).
    DiskSpace { path: &'a str, free: u64, kind: u8 },
    /// `05`: an error in words.
    ErrorText(&'a str),
    /// `06 + step`: what the install is doing.
    InstallInfo { step: u8, text: &'a str },
    /// `0E`: the launcher is to answer [`FromLauncher::Yes`] or
    /// [`FromLauncher::No`].
    Question,
    /// `0F`.
    CannotOpenFile,
    /// `11`.
    CannotMoveFile,
    /// `16`.
    CannotVerifyHash,
    /// `17`.
    HashMismatch,
}

fn narrow(out: &mut Vec<u8>, text: &str) {
    // The launcher shows these with its own (ANSI) font: ASCII only, and at
    // most what the length byte can say.
    let bytes: Vec<u8> = text
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c as u8
            } else {
                b'?'
            }
        })
        .take(255)
        .collect();
    out.push(bytes.len() as u8);
    out.extend_from_slice(&bytes);
}

fn wide(out: &mut Vec<u8>, text: &str) {
    let units: Vec<u16> = text.encode_utf16().take(2000).collect();
    out.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
}

/// The message as it goes on the pipe: length, id, fields.
pub fn frame(message: ToLauncher) -> Vec<u8> {
    let mut out = alloc::vec![0_u8, 0];
    match message {
        ToLauncher::Progress {
            done,
            total,
            speed,
            eta,
        } => {
            out.push(0x01);
            for value in [done, total, speed, eta] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        ToLauncher::State(state) => out.extend_from_slice(&[0x02, state]),
        ToLauncher::CommandLine(text) => {
            out.push(0x03);
            wide(&mut out, text);
        }
        ToLauncher::DiskSpace { path, free, kind } => {
            out.push(0x04);
            wide(&mut out, path);
            out.extend_from_slice(&free.to_le_bytes());
            out.push(kind);
        }
        ToLauncher::ErrorText(text) => {
            out.push(0x05);
            narrow(&mut out, text);
        }
        ToLauncher::InstallInfo { step, text } => {
            out.push(0x06 + step.min(7));
            narrow(&mut out, text);
        }
        ToLauncher::Question => out.push(0x0E),
        ToLauncher::CannotOpenFile => out.push(0x0F),
        ToLauncher::CannotMoveFile => out.push(0x11),
        ToLauncher::CannotVerifyHash => out.push(0x16),
        ToLauncher::HashMismatch => out.push(0x17),
    }
    let length = (out.len() - 2) as u16;
    out[..2].copy_from_slice(&length.to_le_bytes());
    out
}

/// A message of the launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FromLauncher {
    /// `1A`: torrent (0) or direct (1) download. Nothing to this updater.
    DownloadType(u8),
    /// `1B`: stop.
    Abort,
    /// `1C` and `1D`: the answer to [`ToLauncher::Question`].
    Yes,
    No,
    /// `21`: the torrent port. Nothing to this updater.
    Port(u16),
    /// Any other id.
    Other(u8),
}

/// Takes the bytes of the pipe as they come and gives the whole messages.
#[derive(Default)]
pub struct Reader {
    buffer: Vec<u8>,
}

impl Reader {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole message, if all of it is here.
    pub fn message(&mut self) -> Option<FromLauncher> {
        loop {
            let length = u16::from_le_bytes([*self.buffer.first()?, *self.buffer.get(1)?]) as usize;
            if self.buffer.len() < 2 + length {
                return None;
            }
            let payload: Vec<u8> = self.buffer.drain(..2 + length).skip(2).collect();
            // A message of no bytes has no id: nothing to tell.
            let Some(&id) = payload.first() else { continue };
            return Some(match id {
                0x1A => FromLauncher::DownloadType(payload.get(1).copied().unwrap_or(0)),
                0x1B => FromLauncher::Abort,
                0x1C => FromLauncher::Yes,
                0x1D => FromLauncher::No,
                0x21 if payload.len() >= 3 => {
                    FromLauncher::Port(u16::from_le_bytes([payload[1], payload[2]]))
                }
                other => FromLauncher::Other(other),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_length_id_fields() {
        assert_eq!(frame(ToLauncher::State(state::LATEST)), [2, 0, 0x02, 0x0E]);
        assert_eq!(frame(ToLauncher::Question), [1, 0, 0x0E]);
        assert_eq!(frame(ToLauncher::CannotMoveFile), [1, 0, 0x11]);
        let progress = frame(ToLauncher::Progress {
            done: 1,
            total: 0x0102_0304,
            speed: 3,
            eta: u64::MAX,
        });
        assert_eq!(progress.len(), 2 + 1 + 32);
        assert_eq!(&progress[..3], [33, 0, 0x01]);
        assert_eq!(&progress[3..11], [1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&progress[11..19], [4, 3, 2, 1, 0, 0, 0, 0]);
        assert_eq!(&progress[27..], [0xFF; 8]);
        assert_eq!(
            frame(ToLauncher::ErrorText("No.")),
            [5, 0, 0x05, 3, b'N', b'o', b'.']
        );
        assert_eq!(
            frame(ToLauncher::InstallInfo {
                step: step::EXTRACT,
                text: "a"
            }),
            [3, 0, 0x07, 1, b'a']
        );
        assert_eq!(
            frame(ToLauncher::CommandLine("\"a\"")),
            [9, 0, 0x03, 3, 0, b'"', 0, b'a', 0, b'"', 0]
        );
        assert_eq!(
            frame(ToLauncher::DiskSpace {
                path: "C",
                free: 5,
                kind: disk::CANNOT_WRITE
            }),
            [14, 0, 0x04, 1, 0, b'C', 0, 5, 0, 0, 0, 0, 0, 0, 0, 3]
        );
    }

    #[test]
    fn an_error_text_is_ascii_and_short_enough_for_its_length_byte() {
        let long = "x".repeat(400);
        let message = frame(ToLauncher::ErrorText(&long));
        assert_eq!(message.len(), 2 + 1 + 1 + 255);
        assert_eq!(message[3], 255);
        let message = frame(ToLauncher::ErrorText("C:\\Ігри\\a\tb"));
        assert_eq!(&message[4..], b"C:\\????\\a?b");
    }

    #[test]
    fn the_launchers_messages_are_read_however_the_bytes_arrive() {
        let bytes = [
            1, 0, 0x1B, // abort
            2, 0, 0x1A, 1, // direct download
            0, 0, // nothing
            1, 0, 0x1C, // yes
            1, 0, 0x1D, // no
            3, 0, 0x21, 0x39, 0x30, // port 12345
            4, 0, 0x99, 1, 2, 3, // unknown
        ];
        let expected = [
            FromLauncher::Abort,
            FromLauncher::DownloadType(1),
            FromLauncher::Yes,
            FromLauncher::No,
            FromLauncher::Port(12345),
            FromLauncher::Other(0x99),
        ];
        let mut reader = Reader::default();
        reader.push(&bytes);
        let all: Vec<FromLauncher> = core::iter::from_fn(|| reader.message()).collect();
        assert_eq!(all, expected);
        // Byte by byte.
        let mut reader = Reader::default();
        let mut got = Vec::new();
        for byte in bytes {
            reader.push(&[byte]);
            while let Some(message) = reader.message() {
                got.push(message);
            }
        }
        assert_eq!(got, expected);
        assert_eq!(reader.message(), None);
    }
}
