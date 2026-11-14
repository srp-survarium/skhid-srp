//! The release's key, and whether a torrent is the release's: its
//! signature by that key (`updater_logic::signed`), and a revision not
//! older than the game installed here (a torrent of an older release,
//! signed then, is never taken back).

use alloc::format;
use alloc::string::String;

use updater_logic::signed::{self, SignedError};

use crate::gate;
use crate::install::Layout;

/// The release's public key: `release/signing.pub`, or `SKHID_SIGNING_KEY`
/// (64 hex digits) for a build of the tests, which sign with a throwaway
/// key.
const KEY: Option<&str> = option_env!("SKHID_SIGNING_KEY");

/// The release's public key; `None` in a build without one.
pub fn key() -> Option<[u8; 32]> {
    KEY.and_then(signed::public_key)
}

/// The revision of the game installed here (0 when it cannot be told).
pub fn installed(layout: &Layout) -> u64 {
    gate::game_revision(layout)
        .ok()
        .and_then(|revision| revision.number())
        .map_or(0, u64::from)
}

/// `Ok` when `torrent` is the release's, of this revision or a later one.
pub fn check(torrent: &[u8], layout: &Layout) -> Result<u64, String> {
    let key = key().ok_or("this updater has no key to check a torrent with")?;
    let revision = signed::verify(torrent, &key).map_err(|error| match error {
        SignedError::Unsigned => String::from("not signed"),
        SignedError::Wrong => String::from("not signed by the release's key"),
    })?;
    match installed(layout) {
        installed if revision < installed => Err(format!(
            "of revision {revision}, older than the game's {installed}"
        )),
        _ => Ok(revision),
    }
}
