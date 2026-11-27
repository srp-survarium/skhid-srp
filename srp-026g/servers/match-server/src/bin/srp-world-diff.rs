//! What differs between two saved worlds of one tick
//! (`match_server::v026g::world_diff`), as the desync capture writes it
//! beside the two worlds it asked for (`docs/notes/desync_capture.md`).
//!
//! ```text
//! srp-world-diff [--data <server-game-data.db> --recording <rec.srprec>] <a.bin> <b.bin>
//! ```
//!
//! The worlds are `SRP_SNAPSHOT_DIR` dumps (a body each, from the mask
//! on). With the match's recording (and the server game data its kits are
//! read with) every place is read to the end of its state, items included,
//! and the game objects by the map's list; without, each place to its
//! inventory header and the sections that need no match. Prints the
//! differences as JSON, the first one also on stderr. Exit 0 when the
//! worlds are equal, 1 when they differ, 2 for a usage error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use match_server::v026g::{world_check, world_diff, world_validation};

fn usage(why: &str) -> ExitCode {
    eprintln!("srp-world-diff: {why}");
    eprintln!(
        "usage: srp-world-diff [--data <server-game-data.db> --recording <rec>] <a.bin> <b.bin>"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut data = None;
    let mut recording = None;
    let mut worlds: Vec<PathBuf> = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" => data = args.next().map(PathBuf::from),
            "--recording" => recording = args.next().map(PathBuf::from),
            _ if arg.starts_with("--") => return usage(&format!("unknown option {arg}")),
            _ => worlds.push(arg.into()),
        }
    }
    let [a_path, b_path] = worlds.as_slice() else {
        return usage("two worlds are needed");
    };
    if let Some(data) = &data
        && let Err(error) = survarium::v026g::game_data::init_from(data)
    {
        return usage(&format!("{}: {error}", data.display()));
    }
    let read =
        |path: &Path| std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()));
    let (a, b) = match (read(a_path), read(b_path)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(error), _) | (_, Err(error)) => return usage(&error),
    };
    let name = |path: &Path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    };
    let time = name(a_path)
        .and_then(|name| world_check::dump_of_name(&name))
        .map_or(0, |(_, time, _)| time);
    let diff = match &recording {
        Some(path) => {
            let recording = match world_check::read_recording(path) {
                Ok(recording) => recording,
                Err(error) => return usage(&format!("{}: {error}", path.display())),
            };
            let Some(roster) = recording.roster() else {
                return usage("the recording has no match options this game data reads (--data)");
            };
            world_validation::with_contexts(time, recording.places(), roster, |contexts| {
                world_diff::diff(&a, &b, contexts)
            })
        }
        None => world_diff::diff(&a, &b, &[]),
    };
    match serde_json::to_string_pretty(&diff) {
        Ok(json) => println!("{json}"),
        Err(error) => return usage(&error.to_string()),
    }
    match diff.first_field() {
        Some(first) => {
            eprintln!("first difference: {first}");
            ExitCode::from(1)
        }
        None => {
            eprintln!("the worlds are equal");
            ExitCode::SUCCESS
        }
    }
}
