//! Patch 29: "Watch replay", the lobby's link starts the updater
//! (client_patch.md section 3.27).
//!
//! The lobby's video tutorial link opened whatever text the server's 9E
//! sent (ShellExecuteA). Here only a replay link, `replay <8 hex> <32 hex>
//! <64 hex> <1..10 digits>` (lower-case hex), starts `<this exe's
//! folder>\updater_skhid.exe` with the command line `updater_skhid.exe
//! <link>` (CreateProcessW, the folder its working folder); anything else
//! does nothing. Either way the callback returns as the original did (AL
//! 1), and the link's element is not enabled.

#![forbid(unsafe_code)]

use crate::vostok::kernel32::{close, exe_path, start_process};
use crate::vostok::lobby::ServerLink;
use crate::vostok::{Context, Flow, ReplayLinkOnClickedExit};

/// A path's room (UTF-16 units), as GetModuleFileNameW is given it.
const PATH_UNITS: usize = 0x208;
/// The command line's room: a path's and the link's.
const COMMAND_UNITS: usize = PATH_UNITS + 0x100;
/// The updater's name, NUL-ended.
const UPDATER: &[u8] = b"updater_skhid.exe\0";

/// One piece of the link's pattern.
enum Piece {
    Text(&'static [u8]),
    /// So many lower-case hex digits.
    Hex(usize),
}

const PATTERN: [Piece; 7] = [Piece::Text(b"replay "), Piece::Hex(8), Piece::Text(b" "), Piece::Hex(32),
                             Piece::Text(b" "), Piece::Hex(64), Piece::Text(b" ")];
/// The most digits of the size at the end.
const MOST_DIGITS: usize = 10;

/// Whether the NUL-ended text `byte` reads is a replay link: the pattern,
/// then 1 to 10 decimal digits and the end. Reads no byte past a mismatch
/// or the NUL (a NUL matches no piece of the pattern).
fn is_replay_link(byte: impl Fn(usize) -> u8) -> bool {
    let mut at = 0;
    for piece in &PATTERN {
        match piece {
            Piece::Text(text) => {
                for &c in *text {
                    if byte(at) != c {
                        return false;
                    }
                    at += 1;
                }
            }
            Piece::Hex(count) => {
                for _ in 0..*count {
                    if !matches!(byte(at), b'0'..=b'9' | b'a'..=b'f') {
                        return false;
                    }
                    at += 1;
                }
            }
        }
    }
    let mut digits = 0;
    loop {
        match byte(at + digits) {
            0 => return digits > 0,
            b'0'..=b'9' if digits < MOST_DIGITS => digits += 1,
            _ => return false,
        }
    }
}

/// `lobby.replay_link_clicked`: the lobby client's link, checked, starts
/// the updater; then the callback's own end (AL 1).
pub fn on_clicked(ctx: &mut Context<'_>) -> Flow<ReplayLinkOnClickedExit> {
    if let Some(link) = ServerLink::at_replay_link_clicked(ctx)
        && is_replay_link(|i| link.byte(i))
    {
        start_updater(|i| link.byte(i));
    }
    Flow::Exit(ReplayLinkOnClickedExit::Done)
}

/// Starts the updater beside this exe with the link (checked: ASCII,
/// NUL-ended) on its command line.
fn start_updater(link: impl Fn(usize) -> u8) {
    let mut app = [0u16; PATH_UNITS];
    let length = exe_path(&mut app) as usize;
    if length.wrapping_sub(1) >= PATH_UNITS - 1 {
        return; // none, or cut short
    }
    let Some(slash) = app[..length].iter().rposition(|&unit| unit == u16::from(b'\\')) else {
        return; // no folder
    };
    let folder_units = slash + 1;
    if folder_units + UPDATER.len() > PATH_UNITS {
        return;
    }
    let mut folder = [0u16; PATH_UNITS];
    folder[..folder_units].copy_from_slice(&app[..folder_units]);
    for (unit, &c) in app[folder_units..].iter_mut().zip(UPDATER) {
        *unit = u16::from(c);
    }
    let mut command = [0u16; COMMAND_UNITS];
    let name = &UPDATER[..UPDATER.len() - 1];
    for (unit, &c) in command.iter_mut().zip(name) {
        *unit = u16::from(c);
    }
    command[name.len()] = u16::from(b' ');
    let mut at = 0;
    loop {
        let c = link(at);
        command[name.len() + 1 + at] = u16::from(c);
        if c == 0 {
            break;
        }
        at += 1;
    }
    if let Some(started) = start_process(&app, &mut command, &folder) {
        close(started);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(text: &str) -> impl Fn(usize) -> u8 + '_ {
        move |i| *text.as_bytes().get(i).unwrap_or(&0)
    }

    #[test]
    fn the_replay_links_alone() {
        let (id, token, sha) = ("0a1b2c3d", "0123456789abcdef".repeat(2), "fedcba9876543210".repeat(4));
        let valid = format!("replay {id} {token} {sha} 1048576");
        assert!(is_replay_link(link(&valid)));
        assert!(is_replay_link(link(&format!("replay {id} {token} {sha} 1234567890"))));
        assert!(!is_replay_link(link(&format!("replay {id} {token} {sha} 12345678901"))));
        assert!(!is_replay_link(link(&format!("replay {id} {token} {sha} "))));
        assert!(!is_replay_link(link(&format!("replay {id} {token} {sha} 12a"))));
        assert!(!is_replay_link(link(&format!("replay 0A1B2C3D {token} {sha} 1"))));
        assert!(!is_replay_link(link(&format!("replay {id} {token} {sha}"))));
        assert!(!is_replay_link(link("https://example.com")));
        assert!(!is_replay_link(link("")));
    }
}
