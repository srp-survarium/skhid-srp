//! What keeps PLAY from starting a game the server refuses.
//!
//! The login server lets in a game whose exe says a patch revision of
//! `SRP_MIN_PATCH_REVISION` or more, and answers any other with the game's
//! own "You're using an outdated version of the game." The server says that
//! number in `/api/client-files` (`min-revision`), so:
//!
//! - [`behind`]: after the overlay, the revision the installed game exe says
//!   (`updater_logic::revision`) is compared with it. Lower: the server takes
//!   a newer game than the archive it offers (a release half put in place):
//!   PLAY stays off, with words that say so.
//! - [`watch`]: while the launcher stays open after the update, the server is
//!   asked again every few seconds. When it offers another archive or
//!   another least revision than this run acted on, or answers again after
//!   it stopped answering, the launcher is started anew, and its updater
//!   updates. While a server that answered at the start does not answer,
//!   PLAY is off ("Server maintenance..."): a release stops the server, and
//!   the game the launcher would start may be one the server refuses once it
//!   is back.

use alloc::format;
use alloc::string::String;
use alloc::vec;

use updater_logic::manifest::parse_manifest;
use updater_logic::pipe::state;
use updater_logic::revision::{HEADERS_LEN, Revision, SITE_LEN, revision_of, site_offset};
use updater_logic::sha256::to_hex;
use updater_logic::url::Url;

use crate::http::{self, HttpError};
use crate::install::Layout;
use crate::log::log;
use crate::win::{self, File};
use crate::{Session, restart_launcher, text};

/// The game exe the launcher starts, beside the updater.
pub const GAME_EXE: &str = "survarium_skhid.exe";
/// How often the server is asked while the launcher stays open.
const WATCH_MS: u32 = 10_000;
/// How many answers in a row the server must miss before PLAY is turned off.
const MISSES: u32 = 2;

/// What the server said that the update acted on.
pub struct Seen {
    pub server: Url,
    /// The archive the game's Skhid files are from: the one the server
    /// offered, or (the server could not be asked) the one installed last.
    pub archive: Option<[u8; 32]>,
    /// The least revision the server lets in, if it said one.
    pub min_revision: Option<u8>,
    /// The server answered: PLAY goes off while it does not.
    pub online: bool,
}

fn read_exactly(file: &File, buffer: &mut [u8]) -> Result<(), String> {
    let mut done = 0;
    while done < buffer.len() {
        match file.read(&mut buffer[done..]) {
            Ok(0) => return Err("the file ends before it".into()),
            Ok(read) => done += read,
            Err(code) => return Err(win::error_text(code)),
        }
    }
    Ok(())
}

/// The revision the installed game exe says in its sign-in.
fn game_revision(layout: &Layout) -> Result<Revision, String> {
    let path = format!("{}\\{GAME_EXE}", layout.exe_dir);
    let file = File::shared(&path).map_err(win::error_text)?;
    let size = file.size().map_err(win::error_text)?;
    let mut headers = vec![0_u8; HEADERS_LEN.min(size as usize)];
    read_exactly(&file, &mut headers)?;
    let Some(at) = site_offset(&headers) else {
        return Ok(Revision::Unknown);
    };
    file.seek(at).map_err(win::error_text)?;
    let mut site = [0_u8; SITE_LEN];
    read_exactly(&file, &mut site)?;
    Ok(revision_of(&site))
}

/// Whether the installed game is older than the server lets in: the words
/// for the player if so. What cannot be told (no least revision, an exe
/// that cannot be read or is not one this updater knows) is logged and
/// lets the update go on: the login server still decides.
pub fn behind(layout: &Layout, min_revision: Option<u8>) -> Option<String> {
    let need = min_revision.filter(|&need| need > 0)?;
    match game_revision(layout) {
        Ok(revision) => match revision.number() {
            Some(have) if have < need => {
                log!(
                    "{GAME_EXE} says revision {have}; the server lets in {need} and up: PLAY stays off"
                );
                Some(text::behind(have, need))
            }
            Some(have) => {
                log!("{GAME_EXE} says revision {have}; the server lets in {need} and up");
                None
            }
            None => {
                log!(
                    "{GAME_EXE}: its revision cannot be read (not an exe this updater knows); the \
                     server lets in {need} and up"
                );
                None
            }
        },
        Err(why) => {
            log!(
                "{GAME_EXE}: its revision cannot be read ({why}); the server lets in {need} and up"
            );
            None
        }
    }
}

fn hex(sha256: &Option<[u8; 32]>) -> String {
    match sha256 {
        Some(sha256) => String::from_utf8_lossy(&to_hex(sha256)).into(),
        None => "none".into(),
    }
}

/// After the update's last message, until the launcher closes (PLAY starts
/// the game and closes it too): the server asked every [`WATCH_MS`]; the
/// launcher started anew when the update must run again (above). The pipe
/// is closed at the end.
pub fn watch(session: &mut Session, layout: &Layout, product: &str, seen: &Seen) {
    log!(
        "watching {}: archive {}, least revision {:?}",
        seen.server.host,
        hex(&seen.archive),
        seen.min_revision
    );
    let mut missed = 0;
    let mut down = false;
    while session.pipe.is_some() && session.pause(WATCH_MS) {
        let result = http::get(&seen.server, "/api/client-files", 4096, &mut |_, _| {
            session.pump()
        });
        let why = match result.as_ref().map(|body| parse_manifest(body)) {
            Ok(Ok(now)) => {
                let again = if down {
                    Some("the server answers again".into())
                } else if Some(now.sha256) != seen.archive {
                    Some(format!(
                        "the server offers archive {}",
                        hex(&Some(now.sha256))
                    ))
                } else if now.min_revision != seen.min_revision {
                    Some(format!(
                        "the server lets in revision {:?} and up",
                        now.min_revision
                    ))
                } else {
                    None
                };
                if let Some(again) = again {
                    log!("{again}: the update runs again");
                    restart_launcher(session, layout, product);
                    return;
                }
                missed = 0;
                continue;
            }
            Err(HttpError::Stopped) => break,
            // Busy, or reading a new archive: no news.
            Err(HttpError::Status(503)) => continue,
            Err(error) => format!("{error:?}"),
            Ok(Err(error)) => format!("not the server's answer ({error:?})"),
        };
        missed += 1;
        log!("watch: /api/client-files: {why} ({missed} in a row)");
        if seen.online && !down && missed >= MISSES {
            log!("the server does not answer: PLAY off until it does");
            session.state(state::MAINTENANCE);
            down = true;
        }
    }
    session.leave();
}
