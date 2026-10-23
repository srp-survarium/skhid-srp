//! Every sentence the updater shows a player. Each says what went wrong
//! and, where there is one, what to do about it. English only (but the
//! box of a file blocked or held, `updater_logic::file_error`); the
//! launcher's own states and its progress bar are in the launcher's
//! language. A message goes to the launcher as at most 255 ASCII characters,
//! so the sentences are short and name files by their last parts.

use alloc::format;
use alloc::string::String;

use updater_logic::url::Url;
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_DISK_FULL, ERROR_HANDLE_DISK_FULL, ERROR_LOCK_VIOLATION,
    ERROR_SHARING_VIOLATION, ERROR_WRITE_PROTECT,
};
use windows_sys::Win32::Networking::WinHttp::{
    ERROR_WINHTTP_CANNOT_CONNECT, ERROR_WINHTTP_CONNECTION_ERROR, ERROR_WINHTTP_NAME_NOT_RESOLVED,
    ERROR_WINHTTP_SECURE_CERT_CN_INVALID, ERROR_WINHTTP_SECURE_CERT_DATE_INVALID,
    ERROR_WINHTTP_SECURE_CERT_REV_FAILED, ERROR_WINHTTP_SECURE_CERT_REVOKED,
    ERROR_WINHTTP_SECURE_CHANNEL_ERROR, ERROR_WINHTTP_SECURE_FAILURE,
    ERROR_WINHTTP_SECURE_INVALID_CA, ERROR_WINHTTP_SECURE_INVALID_CERT, ERROR_WINHTTP_TIMEOUT,
};

use crate::http::HttpError;
use crate::install::InstallError;
use crate::win::{self, Code};

/// The title of the updater's own message boxes.
pub const TITLE: &str = "Skhid updater";

/// The last two parts of a path: enough to know the file, short enough for
/// the launcher's line.
fn short(path: &str) -> &str {
    let mut cut = path.len();
    for _ in 0..2 {
        match path[..cut].rfind(['\\', '/']) {
            Some(at) => cut = at,
            None => return path,
        }
    }
    &path[cut + 1..]
}

fn megabytes(bytes: u64) -> u64 {
    bytes.div_ceil(1 << 20)
}

pub fn usage() -> String {
    "The Skhid updater is started by launcher_skhid.exe. To update by hand, every patch file \
     hashed again, start it as: updater_skhid.exe check (to see whether the game's files are \
     updated: updater_skhid.exe ready)"
        .into()
}

pub fn wrong_folder(path: &str) -> String {
    format!(
        "The updater must be in the game's folder game\\binaries\\x86, beside survarium.exe. \
         It is here:\n{path}"
    )
}

pub fn bad_address(address: &str, source: &str) -> String {
    format!(
        "The update server address \"{address}\" ({source}) is not valid. It must be \
         https://host. Delete updater_skhid.url to use the default server."
    )
}

pub fn newer_format(server: &Url) -> String {
    format!(
        "{} offers the client files in a newer form than this updater reads. Download the \
         current files from the server's page and copy them over the game.",
        server.host
    )
}

pub fn archive_too_large(server: &Url) -> String {
    format!(
        "{} offers client files larger than this updater takes. Tell the server's operator.",
        server.host
    )
}

pub fn not_the_server(server: &Url, detail: &str) -> String {
    format!(
        "The answer from {} is not the update server's ({detail}). A proxy or a Wi-Fi sign-in \
         page may stand in between. Try again later.",
        server.host
    )
}

pub fn nothing_offered(server: &Url) -> String {
    format!(
        "{} offers no client files now. Try again later.",
        server.host
    )
}

pub fn busy(server: &Url) -> String {
    format!(
        "{} is busy with other downloads. Try again in a minute.",
        server.host
    )
}

pub fn checksum_mismatch(server: &Url) -> String {
    format!(
        "The files downloaded from {} are damaged (the checksum is wrong). Nothing was \
         installed. Try again.",
        server.host
    )
}

pub fn bad_archive(detail: &str) -> String {
    format!(
        "The downloaded archive cannot be used ({detail}). Nothing was installed. Tell the server's operator."
    )
}

/// The server could not be asked, but the installed files are whole.
pub fn offline(reason: &str) -> String {
    format!("No update check: {reason} The installed files are complete, so you can play.")
}

/// The server could not be asked, and nothing was ever installed by this
/// updater (the files were copied by hand).
pub fn offline_unchecked(reason: &str) -> String {
    format!("No update check: {reason} The files could not be checked; you can try to play.")
}

/// The server could not be asked, and a file it installed is gone or changed.
pub fn offline_damaged(reason: &str, name: &str) -> String {
    format!(
        "{} is missing or damaged and cannot be repaired now: {reason}",
        short(name)
    )
}

/// The server could not be asked, and a file it installed cannot be read
/// (Windows' error code).
pub fn offline_unreadable(reason: &str, name: &str, code: Code) -> String {
    format!(
        "{} cannot be read ({}, error {code}) and cannot be repaired now: {reason}",
        short(name),
        win::error_text(code)
    )
}

pub fn restart_failed(reason: &str) -> String {
    format!(
        "The launcher was updated but could not be started again ({reason}). Start \
         launcher_skhid.exe yourself."
    )
}

pub fn repairing(name: &str) -> String {
    format!("repairing {name}")
}

pub fn http(server: &Url, path: &str, error: &HttpError) -> String {
    let host = &server.host;
    match error {
        HttpError::Network(step, code) => match *code {
            ERROR_WINHTTP_NAME_NOT_RESOLVED => format!(
                "The address {host} was not found. Check the internet connection and try again."
            ),
            ERROR_WINHTTP_CANNOT_CONNECT => format!(
                "Cannot connect to {host}. The server may be down, or a firewall blocks \
                 updater_skhid.exe. Try again later."
            ),
            ERROR_WINHTTP_TIMEOUT => format!(
                "{host} did not answer in time. Check the internet connection and try again."
            ),
            ERROR_WINHTTP_CONNECTION_ERROR => {
                format!("The connection to {host} broke off. Try again.")
            }
            ERROR_WINHTTP_SECURE_FAILURE
            | ERROR_WINHTTP_SECURE_CHANNEL_ERROR
            | ERROR_WINHTTP_SECURE_CERT_DATE_INVALID
            | ERROR_WINHTTP_SECURE_CERT_CN_INVALID
            | ERROR_WINHTTP_SECURE_INVALID_CA
            | ERROR_WINHTTP_SECURE_INVALID_CERT
            | ERROR_WINHTTP_SECURE_CERT_REVOKED
            | ERROR_WINHTTP_SECURE_CERT_REV_FAILED => format!(
                "No secure connection to {host} (error {code}). Check this PC's date and time, \
                 and that no program intercepts HTTPS."
            ),
            code => format!("Network error {code} ({step}) while asking {host}. Try again later."),
        },
        HttpError::Status(status) => {
            format!("{host} answered {path} with error {status}. Try again later.")
        }
        HttpError::TooLarge => format!("The answer of {host} to {path} is too large."),
        HttpError::Cut { got, expected } => {
            format!("The download from {host} broke off ({got} of {expected} bytes). Try again.")
        }
        HttpError::Stopped => "Stopped.".into(),
        HttpError::Html => format!(
            "{host} answered {path} with a web page, not the file (a sign-in or virus-scan page)."
        ),
        HttpError::Redirects => format!(
            "{host} redirected {path} more than {} times.",
            updater_logic::url::MAX_REDIRECTS
        ),
        HttpError::BadRedirect(location) => {
            format!("{host} redirected {path} to an address the updater does not take: {location}")
        }
    }
}

/// What to do about a file error, by its Windows code.
fn advice(code: Code) -> &'static str {
    match code {
        ERROR_ACCESS_DENIED | ERROR_WRITE_PROTECT => {
            " Move the game out of Program Files or start the launcher as administrator; \
             an antivirus can block it too."
        }
        ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION => {
            " Another program holds the file: close the game and try again."
        }
        ERROR_DISK_FULL | ERROR_HANDLE_DISK_FULL => " The disk is full.",
        _ => "",
    }
}

pub fn install(error: &InstallError) -> String {
    match error {
        InstallError::Folder { path, code } => format!(
            "Cannot make the folder {}: {} (error {code}).{}",
            short(path),
            win::error_text(*code),
            advice(*code)
        ),
        InstallError::DiskFull { path, needed, free } => format!(
            "Not enough free disk space for the update in {}: {} MB needed, {} MB free.",
            short(path),
            megabytes(*needed),
            megabytes(*free)
        ),
        InstallError::Write { path, code } => format!(
            "Cannot write {}: {} (error {code}).{}",
            short(path),
            win::error_text(*code),
            advice(*code)
        ),
        InstallError::ReadBack { path } => format!(
            "{} did not read back as it was written. An antivirus may have changed it, or the \
             disk is failing. Nothing was installed.",
            short(path)
        ),
        InstallError::Replace { path, code } => format!(
            "Cannot replace {}: {} (error {code}). Nothing was changed.{}",
            short(path),
            win::error_text(*code),
            advice(*code)
        ),
        InstallError::Stopped => "Stopped.".into(),
    }
}

/// Said after each reason that PLAY stays off for.
const PLAY_OFF: &str = " PLAY stays off until this is done.";

pub fn disk_full(folder: &str, needed: u64, free: u64) -> String {
    format!(
        "Not enough free disk space to update the game's files in {}: {} MB needed, {} MB free. \
         Free some space, then start the launcher again.{PLAY_OFF}",
        folder,
        megabytes(needed),
        megabytes(free)
    )
}

/// The patch step (game\resources.db.patch_<n>, the server's list) could
/// not finish. PLAY stays off.
pub fn patches_game_running() -> String {
    format!(
        "The game is running, so Skhid could not update the game's patches \
         (game\\resources.db.patch_*). Close the game, then start the launcher \
         again.{PLAY_OFF}"
    )
}

/// A patch file another program than the game holds (the game is told by
/// resources.db): an antivirus that scans it, most often.
pub fn patches_held(name: &str) -> String {
    format!(
        "Another program holds game\\{name}, so Skhid could not update the game's patches. \
         An antivirus may be scanning it: wait a minute, then start the launcher again.{PLAY_OFF}"
    )
}

pub fn patches_failed() -> String {
    format!(
        "Skhid could not finish updating the game's patches (game\\resources.db.patch_*); \
         the reason is in game\\binaries\\x86\\updater_skhid.log. Start the launcher again, \
         or send that file to the server's operator.{PLAY_OFF}"
    )
}

pub fn patches_unreachable(reason: &str) -> String {
    format!("The list of the game's patches could not be fetched. {reason}{PLAY_OFF}")
}

pub fn patches_not_offered(server: &Url) -> String {
    format!(
        "{} offers no list of the game's patches. Tell the server's operator.{PLAY_OFF}",
        server.host
    )
}

pub fn patches_bad_list(server: &Url, detail: &str) -> String {
    format!(
        "The list of the game's patches from {} cannot be used ({detail}). Tell the server's \
         operator.{PLAY_OFF}",
        server.host
    )
}

pub fn patches_download(name: &str, reason: &str) -> String {
    format!("game\\{name} could not be downloaded. {reason}{PLAY_OFF}")
}

/// None of a patch's `count` addresses gave its file; `first`: what the
/// first one's failure was.
pub fn patches_no_address(name: &str, count: usize, first: &str) -> String {
    format!(
        "game\\{name} could not be downloaded from any of its {count} addresses (each is in \
         updater_skhid.log). The first: {first}{PLAY_OFF}"
    )
}

pub fn patches_wrong(server: &Url, name: &str) -> String {
    format!(
        "{} sent game\\{name} with another checksum than its list says. Tell the server's \
         operator.{PLAY_OFF}",
        server.host
    )
}

/// The server could not be asked (`reason`, the overlay's words) and the
/// patches were never checked in place, or are not any more.
pub fn patches_offline(reason: &str) -> String {
    format!("The game's patches could not be checked: {reason}{PLAY_OFF}")
}

/// `updater_skhid.exe ready` found the patches not checked in place.
pub fn patches_not_ready() -> String {
    "Skhid has not finished updating the game's files (game\\resources.db.patch_*), so the \
     game is not started. Start Skhid.bat (the launcher): it finishes the update, or says \
     what is in the way. Details: game\\binaries\\x86\\updater_skhid.log."
        .into()
}

/// The server lets in only games of revision `need` and up, and the game
/// exe the update installed says `have`: the server's update is not offered
/// yet (it is being put in place). The launcher stays open and looks again.
pub fn behind(have: u8, need: u8) -> String {
    format!(
        "The server lets in game revision {need} and up; this game is revision {have}, and \
         the update to it is not offered yet (the server is being updated). Click OK: the \
         launcher keeps looking and starts anew with the update.{PLAY_OFF}"
    )
}

/// `check` by hand replaced this updater, and the new one could not be run
/// to finish the update.
pub fn replaced_not_run(reason: &str) -> String {
    format!(
        "updater_skhid.exe was updated, but the new one could not be started to finish the \
         update ({reason}). Start it again."
    )
}
