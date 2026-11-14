//! The release's key, and whether a torrent is the release's: its
//! signature by that key (`updater_logic::signed`), and a revision not
//! lower than the highest this updater has taken, nor than the game's (no
//! rollback: a torrent of an older release, signed then, is never taken
//! again).

use alloc::format;
use alloc::string::String;

use updater_logic::signed::{self, SignedError};

use crate::gate;
use crate::install::Layout;
use crate::log::log;
use crate::win;

/// The release's public key, 64 hex digits: `release/signing.pub`, which
/// `build.rs` reads, or `SKHID_SIGNING_KEY` of the build (the tests sign
/// with a throwaway key). A build without either takes no torrent.
const KEY: Option<&str> = option_env!("SKHID_SIGNING_KEY");
/// The highest revision taken, beside the updater.
const TAKEN: &str = "updater_skhid.torrent.revision";

/// The release's public key; `None` in a build without one.
pub fn key() -> Option<[u8; 32]> {
    KEY.and_then(signed::public_key)
}

/// The revision the installed game exe says (0 when it cannot be told).
pub fn game_number(layout: &Layout) -> u64 {
    gate::game_revision(layout)
        .ok()
        .and_then(|revision| revision.number())
        .map_or(0, u64::from)
}

/// The least revision taken: the highest taken before, the game's.
pub fn installed(layout: &Layout) -> u64 {
    let game = game_number(layout);
    let taken = win::read_file(&format!("{}\\{TAKEN}", layout.exe_dir), 16)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);
    game.max(taken)
}

/// Notes that a torrent of `revision` was taken.
pub fn taken(layout: &Layout, revision: u64) {
    if revision > installed(layout) {
        let path = format!("{}\\{TAKEN}", layout.exe_dir);
        if let Err(code) = win::write_file(&path, format!("{revision}\r\n").as_bytes()) {
            log!("torrent: {TAKEN} cannot be written: {}", win::error_text(code));
        }
    }
}

/// The revision of `torrent` when it is the release's and not older than
/// [`installed`].
pub fn check(torrent: &[u8], layout: &Layout) -> Result<u64, String> {
    let key = key().ok_or("this updater has no key to check a torrent with")?;
    let revision = signed::verify(torrent, &key).map_err(|error| match error {
        SignedError::Unsigned => String::from("not signed"),
        SignedError::Wrong => String::from("not signed by the release's key"),
    })?;
    let revision = u64::from(revision);
    match installed(layout) {
        least if revision < least => Err(format!(
            "of revision {revision}, older than the {least} this folder has"
        )),
        _ => Ok(revision),
    }
}
