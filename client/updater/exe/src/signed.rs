//! Whether a torrent is the release's: signed by its key, and of a revision
//! no lower than any taken before, the game's or this updater's own, so an
//! older release's torrent, validly signed then, is never taken again.

use alloc::format;
use alloc::string::String;

use updater_logic::signed::{self, SignedError};

use crate::gate;
use crate::install::Layout;
use crate::log::log;
use crate::win;

/// 64 hex digits from `release/signing.pub` or `SKHID_TEST_PUBLIC_KEY`
/// (`build.rs`); a build without either takes no torrent.
const KEY: Option<&str> = option_env!("SKHID_BUILT_PUBLIC_KEY");
/// Beside the updater: the highest revision taken.
const TAKEN: &str = "updater_skhid.torrent.revision";

pub fn key() -> Option<[u8; 32]> {
    KEY.and_then(signed::public_key)
}

/// 0 when the installed exe's revision cannot be told.
pub fn game_number(layout: &Layout) -> u64 {
    gate::game_revision(layout)
        .ok()
        .and_then(|revision| revision.number())
        .map_or(0, u64::from)
}

/// `SKHID_REVISION`, set by `release/build-zip`; 0 for a local build.
fn built() -> u64 {
    option_env!("SKHID_REVISION")
        .and_then(|text| text.parse().ok())
        .unwrap_or(0)
}

/// The least revision taken: the highest of the one taken before, the
/// game's and this updater's own.
pub fn installed(layout: &Layout) -> u64 {
    let game = game_number(layout).max(built());
    let taken = win::read_file(&format!("{}\\{TAKEN}", layout.exe_dir), 16)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);
    game.max(taken)
}

pub fn taken(layout: &Layout, revision: u64) {
    if revision > installed(layout) {
        let path = format!("{}\\{TAKEN}", layout.exe_dir);
        if let Err(code) = win::write_file(&path, format!("{revision}\r\n").as_bytes()) {
            log!("torrent: {TAKEN} cannot be written: {}", win::error_text(code));
        }
    }
}

/// `torrent`'s revision, when the release signed it and it is not older
/// than [`installed`].
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
