//! First installation of the original game, before applying the Skhid ZIP.
//!
//! Its archive's addresses are the server's (`GET /game`: the release's,
//! then the mirrors srp is given), or, when the server cannot be reached or
//! offers none, the ones built into this updater at release time. They are
//! tried in their order (`client/updater/README.md`, "Mirrors"): any
//! failure moves on to the next, which goes on from the bytes already on
//! disk; the pinned size and SHA-256 decide.
use crate::{
    Session,
    http::{self, HttpError},
    install::Layout,
    log::log,
    text, win,
};
use alloc::{format, string::String, vec::Vec};
use updater_logic::{
    download,
    game::{self, Pin},
    patches::{self as list, Address, Download},
    pipe::{ToLauncher, state, step},
    url::Url,
};
use windows_sys::Win32::Foundation::ERROR_INVALID_HANDLE;

pub fn present(layout: &Layout) -> bool {
    ["resources.db", "binaries\\x86\\survarium.exe"]
        .iter()
        .all(|name| {
            win::file_size(&format!("{}\\{name}", layout.game_dir)).is_ok_and(|size| size > 0)
        })
}

fn extractor_path(layout: &Layout) -> String {
    format!("{}\\{}", layout.exe_dir, game::EXTRACTOR_NAME)
}

/// The extractor beside the updater is the pinned file. It comes with the
/// Skhid archive; when it is not there or not that file, the Skhid files
/// are installed first, which brings it.
pub fn extractor_ready(layout: &Layout) -> bool {
    win::hash_file(&extractor_path(layout))
        .is_ok_and(|(size, hash)| game::EXTRACTOR.matches(size, hash))
}

/// Removes what an attempt that was cut short left in the game folder (the
/// PC switched off, the updater killed while the extractor ran): extraction
/// folders, each with up to 6.3 GB in it, and, once the game is there, the
/// 2.9 GB download. The caller holds the installation lock, so no other
/// updater is extracting here.
pub fn sweep(layout: &Layout) {
    for name in win::list_folders(&layout.game_dir, STAGE_PATTERN) {
        let stage = format!("{}\\{name}", layout.game_dir);
        clean_stage(&stage);
        if win::exists(&stage) {
            log!(
                "left over and not removed (in use, or it holds files that are not the game's): {stage}"
            );
        } else {
            log!("removed a left-over extraction folder: {stage}");
        }
    }
    let archive = archive_path(layout);
    if present(layout) && win::exists(&archive) && win::delete(&archive).is_ok() {
        log!("removed the left-over game download: {archive}");
    }
    let output = output_path(layout);
    if win::exists(&output) {
        log!("what the extractor printed in an attempt that was cut short:");
        log_output(&output);
    }
}

/// Where the extractor's messages go while it runs, beside the log.
fn output_path(layout: &Layout) -> String {
    format!("{}\\updater_skhid.7zr.txt", layout.exe_dir)
}

/// The extractor prints some twenty lines for the five files; the end of
/// more than this is kept, where the errors are.
const OUTPUT_LIMIT: usize = 16 * 1024;

/// Copies what the extractor printed into the log, line by line, and removes
/// its file. Without this the log has the extractor's exit number only, and
/// not which file it could not write or why.
fn log_output(path: &str) {
    match win::read_tail(path, OUTPUT_LIMIT) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                log!("7zr: {line}");
            }
        }
        Err(code) => log!(
            "what the extractor printed cannot be read: {}",
            win::error_text(code)
        ),
    }
    let _ = win::delete(path);
}

/// `win::staging_dir` makes `.skhid-extract-<ticks>-<n>`.
const STAGE_PATTERN: &str = ".skhid-extract-*";

fn archive_path(layout: &Layout) -> String {
    format!("{}\\.skhid-game.7z", layout.game_dir)
}

fn io_error(action: &str, code: win::Code) -> Option<String> {
    if code == windows_sys::Win32::Foundation::ERROR_CANCELLED {
        return None;
    }
    Some(format!("Cannot {action}: {}.", win::error_text(code)))
}

/// What the log calls the archive's download (`download::line`), as
/// Skhid-without-updater.bat does.
const LABEL: &str = "skhid-game";
/// What a failed write of the download is, to tell it from the network's.
const WRITE_STEP: &str = "write the download";

/// The archive's addresses built into this updater at release time
/// (`SKHID_BUILTIN_GAME`, the `/game` of the release's list, which
/// `release/build-zip` passes to the build): those of the pinned archive.
fn built_in() -> Option<Download> {
    option_env!("SKHID_BUILTIN_GAME")
        .and_then(|text| list::parse_game(text.as_bytes()).ok())
        .filter(|offered| game::ARCHIVE.matches(offered.size, offered.sha256))
}

/// Where the archive is, in the order its addresses are tried: the
/// server's `GET /game` when it offers the pinned archive (a server that
/// answers always wins), else the addresses built in.
fn addresses(session: &mut Session, server: Option<&Url>) -> Result<Vec<Address>, Option<String>> {
    let mut why = String::from("the update server cannot be reached.");
    if let Some(server) = server {
        why = match http::get(server, "/game", 64 * 1024, &mut |_, _| session.pump()) {
            Ok(body) => match list::parse_game(&body) {
                Ok(offered) if game::ARCHIVE.matches(offered.size, offered.sha256) => {
                    log!(
                        "game: {} offers {} address(es) of the archive",
                        server.host,
                        offered.addresses.len()
                    );
                    return Ok(offered.addresses);
                }
                Ok(_) => format!(
                    "{} offers another archive than the one this updater installs.",
                    server.host
                ),
                Err(error) => format!("{}'s /game cannot be used ({error:?}).", server.host),
            },
            Err(HttpError::Stopped) => return Err(None),
            Err(error) => text::http(server, "/game", &error),
        };
        log!("game: {why}");
    }
    match built_in() {
        Some(offered) => {
            log!(
                "game: the {} address(es) built into this updater are used",
                offered.addresses.len()
            );
            Ok(offered.addresses)
        }
        None => Err(Some(text::game_no_address(&why))),
    }
}

fn check(
    path: &str,
    pin: Pin,
    session: &mut Session,
) -> Result<Option<win::LockedFile>, Option<String>> {
    match win::file_size(path) {
        Ok(size) if size != pin.size => return Ok(None),
        Err(code) if win::is_missing(code) => return Ok(None),
        Err(code) => return Err(io_error("read the downloaded file size", code)),
        _ => {}
    }
    let file = match win::LockedFile::open(path) {
        Ok(file) => file,
        Err(code) if win::is_missing(code) => return Ok(None),
        Err(code) => return Err(io_error("open the downloaded file", code)),
    };
    session.begin_progress();
    session.state(state::CHECKING);
    let (size, hash) = file
        .verify(&mut |done| session.progress(done, pin.size))
        .map_err(|code| io_error("verify the downloaded file", code))?;
    Ok(pin.matches(size, hash).then_some(file))
}

/// Returns a read lock only after hashing the bytes actually on disk. The lock
/// stays alive until the extractor exits, preventing replacement after checking.
///
/// The addresses are tried in their order, each once a run; any failure
/// moves on to the next, which goes on from the bytes on disk (they are
/// the same file at every address). A download left by a run that stopped
/// is gone on from the same way. One line of the log a try and a failure.
fn fetch(
    session: &mut Session,
    addresses: &[Address],
    path: &str,
    pin: Pin,
) -> Result<win::LockedFile, Option<String>> {
    if let Some(file) = check(path, pin, session)? {
        log!("reusing verified download: {path}");
        return Ok(file);
    }
    let count = addresses.len();
    let mut failed: Vec<(String, String)> = Vec::new();
    for (at, address) in addresses.iter().enumerate() {
        if !session.pump() {
            return Err(None);
        }
        let host = address.url.host.as_str();
        // A whole file that is here failed its check: it is no part.
        let mut have = win::file_size(path).unwrap_or(0);
        if have >= pin.size {
            let _ = win::delete(path);
            have = 0;
        }
        log!(
            "{}",
            download::line(
                LABEL,
                "try",
                host,
                &format!(
                    "address {} of {count}, {}, from byte {have}",
                    at + 1,
                    address.text
                )
            )
        );
        session.begin_progress();
        session.state(state::DOWNLOADING);
        // Too slow is a failure only while another address remains.
        let slow = if at + 1 < count {
            http::Slow::Leave
        } else {
            http::Slow::Wait
        };
        let (result, received) = get(session, address, path, have, pin, slow)?;
        let why = match result {
            Ok(()) => match win::file_size(path).unwrap_or(0) {
                size if size != pin.size => format!("cut short at byte {size} of {}", pin.size),
                _ => match check(path, pin, session)? {
                    Some(file) => {
                        log!(
                            "{}",
                            download::line(
                                LABEL,
                                "ok",
                                host,
                                &format!("{} bytes, SHA-256 verified", pin.size)
                            )
                        );
                        return Ok(file);
                    }
                    None => {
                        let _ = win::delete(path);
                        format!("wrong SHA-256 (not {})", pin.sha256)
                    }
                },
            },
            Err(error) => text::reason(&error),
        };
        log!(
            "{}",
            download::line(
                LABEL,
                "failed",
                host,
                &format!("{why} ({received} bytes received)")
            )
        );
        failed.push((host.into(), why));
    }
    // Bytes got stay for the next start; no byte, no file.
    if win::file_size(path).is_ok_and(|size| size == 0) {
        let _ = win::delete(path);
    }
    Err(Some(text::game_download_failed(&download::failures(
        &failed,
    ))))
}

/// One try of one address: its bytes into `path` from byte `from` on, or
/// from 0 when it gives the whole file again. What it answered and the
/// bytes received of it; `Err` for a failure here (the disk), not of the
/// address, and when told to stop (the bytes stay for the next run).
fn get(
    session: &mut Session,
    address: &Address,
    path: &str,
    from: u64,
    pin: Pin,
    slow: http::Slow,
) -> Result<(Result<(), HttpError>, u64), Option<String>> {
    let opened = if from == 0 {
        win::File::create(path)
    } else {
        win::File::exclusive(path).and_then(|file| file.seek(from).map(|()| file))
    };
    // `None` only between the old file closed and the new one made.
    let mut file = Some(opened.map_err(|code| io_error("save the game download", code))?);
    let mut written = from;
    let mut received = 0_u64;
    let result = http::get_file(
        &address.url,
        from,
        pin.size,
        slow,
        &mut |to| log!("{LABEL}: {} redirects to {to}", address.text),
        &mut |at, chunk| {
            if at != written {
                // A 200 to a request from a byte on: the whole file again
                // (the old one closed first: it is open to this alone).
                file = None;
                file = Some(
                    win::File::create(path).map_err(|code| HttpError::Network(WRITE_STEP, code))?,
                );
                written = at;
            }
            let Some(out) = &file else {
                return Err(HttpError::Network(WRITE_STEP, ERROR_INVALID_HANDLE));
            };
            out.write(chunk)
                .map_err(|code| HttpError::Network(WRITE_STEP, code))?;
            written += chunk.len() as u64;
            received += chunk.len() as u64;
            Ok(())
        },
        &mut |done, _| session.progress(done, pin.size),
    );
    let flushed = file.as_ref().map_or(Ok(()), win::File::flush);
    drop(file);
    match result {
        Err(HttpError::Stopped) => return Err(None),
        Err(HttpError::Network(WRITE_STEP, code)) => {
            return Err(io_error("save the game download", code));
        }
        _ => {}
    }
    flushed.map_err(|code| io_error("flush the download to disk", code))?;
    if let Err(HttpError::Html(_, read)) = result {
        received = read;
    }
    Ok((result, received))
}

fn extracted(stage: &str, name: &str) -> String {
    format!("{stage}\\Survarium\\game\\{}", name.replace('/', "\\"))
}

fn clean_stage(stage: &str) {
    for (name, _) in game::FILES {
        let _ = win::delete(&extracted(stage, name));
    }
    for dir in [
        "resources\\sources\\splash_screens",
        "resources\\sources",
        "resources\\ssl",
        "resources",
        "binaries\\x86",
        "binaries",
        "",
    ] {
        win::remove_dir(&format!("{stage}\\Survarium\\game\\{dir}"));
    }
    win::remove_dir(&format!("{stage}\\Survarium"));
    win::remove_dir(stage);
}

/// Installs only missing original files. Every extraction and check finishes
/// before promotion; failed promotions undo only files added by this attempt.
///
/// `server`: the update server, when it answered this run; else the
/// archive's addresses built into this updater are used.
pub fn ensure(
    session: &mut Session,
    layout: &Layout,
    server: Option<&Url>,
) -> Result<bool, Option<String>> {
    if present(layout) {
        return Ok(false);
    }
    log!("original game files missing; downloading the pinned 0.26g archive");
    // The extractor first: without it 2.9 GB would be downloaded for nothing.
    // It is a file of the Skhid archive and is never downloaded on its own;
    // it stays locked from this check until it has run.
    let extractor_path = extractor_path(layout);
    let Some(extractor) = check(&extractor_path, game::EXTRACTOR, session)? else {
        return Err(Some(format!(
            "The original game cannot be unpacked: {} beside the updater is missing or is not the file this updater runs (7-Zip's 7zr.exe, {} bytes, SHA-256 {}). It comes with the Skhid archive: unpack that archive again. Nothing was downloaded or run.",
            game::EXTRACTOR_NAME,
            game::EXTRACTOR.size,
            game::EXTRACTOR.sha256
        )));
    };
    let archive_path = archive_path(layout);
    // Archive + unpacked files + room for the Skhid overlay; an archive that
    // is here already (kept after an extraction that failed) needs no room
    // again. This works on a 32-bit updater too: file sizes and progress are
    // u64 throughout.
    let downloaded = win::file_size(&archive_path).is_ok_and(|size| size == game::ARCHIVE.size);
    let needed = if downloaded { 0 } else { game::ARCHIVE.size }
        + game::FILES.iter().map(|(_, size)| size).sum::<u64>()
        + (256 << 20);
    let free = win::free_space(&layout.game_dir)
        .ok_or_else(|| Some("Cannot check free disk space for the game installation.".into()))?;
    if free < needed {
        return Err(Some(format!(
            "The game installation needs {needed} bytes of free space; this disk has {free}."
        )));
    }
    let addresses = addresses(session, server)?;
    let archive = fetch(session, &addresses, &archive_path, game::ARCHIVE)?;
    let stage = win::staging_dir(&layout.game_dir)
        .map_err(|code| io_error("create an extraction folder", code))?;
    let result = (|| {
        session.state(state::INSTALLING);
        session.send(ToLauncher::InstallInfo {
            step: step::EXTRACT,
            text: "Survarium 0.26g",
        });
        // -sccUTF-8: what it prints is UTF-8, so a folder name in any
        // alphabet reads right in the log.
        let mut command = format!(
            "\"{extractor_path}\" x \"{archive_path}\" -o\"{stage}\" -y -bd -bb0 -sccUTF-8"
        );
        for (name, _) in game::FILES {
            command.push_str(&format!(" \"Survarium/game/{name}\""));
        }
        let output_path = output_path(layout);
        let code = win::run_wait(
            &extractor_path,
            &command,
            &layout.game_dir,
            &output_path,
            &mut || session.pump(),
        );
        log_output(&output_path);
        let code = code.map_err(|code| io_error("extract the original game", code))?;
        log!("the extractor ended with exit {code}");
        if code != 0 {
            return Err(Some(format!(
                "The game extractor failed (exit {code}). Nothing was installed. What it \
                 printed is in updater_skhid.log beside the updater."
            )));
        }
        for (name, expected) in game::FILES {
            if win::file_size(&extracted(&stage, name)).ok() != Some(*expected) {
                return Err(Some(format!(
                    "The extracted game file {name} is missing or has the wrong size. Nothing was installed."
                )));
            }
        }
        // Do not silently accept an empty file or a directory in a required
        // destination, and do not overwrite a user's conflicting path.
        for (name, _) in game::FILES {
            let target = format!("{}\\{}", layout.game_dir, name.replace('/', "\\"));
            if win::exists(&target) && !win::file_size(&target).is_ok_and(|size| size > 0) {
                return Err(Some(format!(
                    "Cannot install {name}: its destination is empty, unreadable or a directory. Move it aside and retry."
                )));
            }
        }
        let original = check(
            &extracted(&stage, "binaries/x86/survarium.exe"),
            game::ORIGINAL_EXE,
            session,
        )?;
        if original.is_none() {
            return Err(Some("The extracted survarium.exe failed its pinned SHA-256 check. Nothing was installed.".into()));
        }
        drop(original);
        if !session.pump() {
            return Err(None);
        }
        let mut added: Vec<String> = Vec::new();
        for (name, _) in game::FILES {
            let target = format!("{}\\{}", layout.game_dir, name.replace('/', "\\"));
            // Preserve existing files, including the Skhid certificate and DLL.
            if win::exists(&target) {
                continue;
            }
            let result = win::create_folders(win::split_path(&target).0)
                .and_then(|()| win::rename(&extracted(&stage, name), &target, false));
            if let Err(code) = result {
                for path in added.iter().rev() {
                    let _ = win::delete(path);
                }
                return Err(io_error("install the original game files", code));
            }
            added.push(target);
        }
        log!("installed original game: {} files added", added.len());
        Ok(true)
    })();
    clean_stage(&stage);
    drop(extractor);
    drop(archive);
    // Retain the verified download on extraction failure for a retry; it is
    // always rehashed next time. (A download that failed stays too: the
    // next try goes on from its bytes.) Successful installs need no archive cache.
    // The extractor is a file of the Skhid archive and stays.
    if result.is_ok() {
        let _ = win::delete(&archive_path);
    }
    result
}
