//! Keeps the matches of match recordings (`SRP_RECORD_DIR`) in srp's match
//! history with their events (`match_server::v026g::backfill`), for the
//! matches played before the history was: the recordings still kept.
//!
//! ```text
//! srp-recording-to-history --data <server-game-data.db> <srp.db> <file.srprec | dir>...
//! ```
//!
//! `--data` is the server's game data the matches were played with (their
//! maps, and the items of the kits).
//!
//! A match kept already (live, or by an earlier run) is left as it is: its
//! replay id is its key, so running it again changes nothing. Each match is
//! one transaction. The file is srp's own (`SRP_DATA_DIR/srp.db`), brought
//! to the current schema if it is older: run it with srp stopped, or it
//! waits for srp's writes. Each file's outcome is printed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use match_server::v026g::backfill;
use session::v026g::persist::{GameRepository, SqliteGameRepository};

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let [flag, data, db, inputs @ ..] = args.as_slice() else {
        return usage();
    };
    if flag != Path::new("--data") || inputs.is_empty() {
        return usage();
    }
    if let Err(error) = survarium::v026g::game_data::init_from(data) {
        eprintln!("{}: {error}", data.display());
        return ExitCode::FAILURE;
    }
    let mut repository = match SqliteGameRepository::open(db) {
        Ok(repository) => repository,
        Err(error) => {
            eprintln!("{}: {error}", db.display());
            return ExitCode::FAILURE;
        }
    };
    let mut files = Vec::new();
    for input in inputs {
        match recordings(input) {
            Ok(found) => files.extend(found),
            Err(error) => {
                eprintln!("{}: {error}", input.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let (mut kept, mut there, mut skipped) = (0, 0, 0);
    for file in &files {
        let name = file.display();
        let record = match backfill::match_of_recording(file) {
            Ok(record) => record,
            Err(why) => {
                println!("{name}: skipped, {why}");
                skipped += 1;
                continue;
            }
        };
        match repository.record_match(&record) {
            Ok(true) => {
                println!(
                    "{name}: kept, {} players, {} events",
                    record.players.len(),
                    record.events.len()
                );
                kept += 1;
            }
            Ok(false) => {
                println!("{name}: kept already");
                there += 1;
            }
            Err(error) => {
                eprintln!("{name}: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    println!(
        "{} recordings: {kept} kept, {there} kept already, {skipped} skipped",
        files.len()
    );
    ExitCode::SUCCESS
}

/// `input` if it is a recording, else the recordings in it, oldest first.
fn recordings(input: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !input.is_dir() {
        return Ok(vec![input.to_owned()]);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(input)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "srprec"));
    // The names begin with the UTC time the match was made.
    files.sort();
    Ok(files)
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: srp-recording-to-history --data <server-game-data.db> <srp.db> <file.srprec | dir>..."
    );
    ExitCode::from(2)
}
