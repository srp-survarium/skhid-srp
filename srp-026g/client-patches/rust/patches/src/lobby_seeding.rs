//! Patch 150: the game shares its files by BitTorrent on the login screen
//! and in the lobby, never in a match, as its developers made it, while its
//! own `enable_torrent_seeding` (a line of user.cfg, which the launcher's
//! "Allow torrent" box writes) is on (`srp-026g/docs/notes/torrent-downloads.md`,
//! "Sharing from the game").
//!
//! The game's own triggers stay: switch_to_login and switch_to_lobby call
//! start_seeding while the setting is on (its default now off,
//! patch_026g.py LOBBY_SEEDING_DEFAULT), its command's callback starts or
//! stops it as it changes, switch_to_game_world calls stop_seeding (the
//! pipe's disconnect: the updater ends), a replay never either.
//! start_seeding posts start_seeding_impl, whose pipe server and message
//! handler stay the game's; this patch:
//!
//! - names its pipe `\\.\pipe\skhid_seed-<process id>` (`seeding.accept`;
//!   the game's own name is the narrow product name read as wide: garbled,
//!   and the launcher's updater pipe's prefix);
//! - starts `updater_skhid.exe seed <pipe>` beside the exe, with no window,
//!   where the game asked Windows' "Survarium Update Service" to start its
//!   updater (which it never did: a registry key it could not open), and
//!   returns (`seeding.updater`).

#![forbid(unsafe_code)]

use crate::vostok::log::{self, Piece::Text};
use crate::vostok::seeding;
use crate::vostok::{Context, Flow, LobbySeedingOnUpdaterExit, NoExit};

/// The program started, beside the exe.
pub const PROGRAM: &str = "updater_skhid.exe";
const PIPE_PREFIX: &[u8] = b"\\\\.\\pipe\\skhid_seed-";

/// The longest pipe name: the prefix and a u32's ten digits.
const NAME_MAX: usize = PIPE_PREFIX.len() + 10;

/// The pipe's name for a process: `\\.\pipe\skhid_seed-<id>`, ASCII. One
/// game, one name: it never meets the launcher's updater pipe nor another
/// game's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipeName {
    bytes: [u8; NAME_MAX],
    len: usize,
}

impl PipeName {
    pub fn of(process: u32) -> Self {
        let mut bytes = [0; NAME_MAX];
        bytes[..PIPE_PREFIX.len()].copy_from_slice(PIPE_PREFIX);
        let mut digits = [0u8; 10];
        let mut count = 0;
        let mut rest = process;
        loop {
            digits[count] = b'0' + (rest % 10) as u8;
            count += 1;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        for (at, digit) in digits[..count].iter().rev().enumerate() {
            bytes[PIPE_PREFIX.len() + at] = *digit;
        }
        Self { bytes, len: PIPE_PREFIX.len() + count }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// The updater's arguments: `seed <pipe>`.
fn arguments(pipe: &PipeName) -> ([u8; 5 + NAME_MAX], usize) {
    let mut out = [0; 5 + NAME_MAX];
    out[..5].copy_from_slice(b"seed ");
    out[5..5 + pipe.len].copy_from_slice(pipe.as_bytes());
    (out, 5 + pipe.len)
}

/// Before its pipe's accept (`seeding.accept`): our name, in the place of
/// the game's.
pub fn on_accept(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let name = PipeName::of(seeding::process_id());
    if seeding::accept_as(ctx, name.as_bytes()) { Flow::Skip } else { Flow::Continue }
}

/// In the place of the service's start (`seeding.updater`): the updater
/// beside the exe, then start_seeding_impl's return.
pub fn on_updater(_ctx: &mut Context<'_>) -> Flow<LobbySeedingOnUpdaterExit> {
    let pipe = PipeName::of(seeding::process_id());
    let (words, len) = arguments(&pipe);
    let pipe_text = core::str::from_utf8(pipe.as_bytes()).unwrap_or("");
    match seeding::start_beside(PROGRAM, &words[..len]) {
        Ok(()) => log::line(&[Text("lobby_seeding: "), Text(PROGRAM), Text(" seed "), Text(pipe_text), Text(" started")]),
        Err(code) => log::line(&[
            Text("lobby_seeding: "),
            Text(PROGRAM),
            Text(" not started, error "),
            log::Piece::Dec(code),
        ]),
    }
    Flow::Exit(LobbySeedingOnUpdaterExit::Started)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pipe_is_named_by_the_process() {
        assert_eq!(PipeName::of(0).as_bytes(), br"\\.\pipe\skhid_seed-0");
        assert_eq!(PipeName::of(1234).as_bytes(), br"\\.\pipe\skhid_seed-1234");
        assert_eq!(PipeName::of(9876).as_bytes(), br"\\.\pipe\skhid_seed-9876");
        assert_eq!(PipeName::of(u32::MAX).as_bytes().len(), NAME_MAX);
        let (words, len) = arguments(&PipeName::of(56));
        assert_eq!(&words[..len], br"seed \\.\pipe\skhid_seed-56");
    }
}
