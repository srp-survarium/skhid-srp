//! Builds the bots' maps ahead of a release (A197): the walk grids, sight trees and tactics the server would otherwise
//! build in the first match on each map, on the host's one core while players play.
//!
//! ```text
//! srp-build-maps <mapdata dir> <built dir> [<project> ...]
//! ```
//!
//! The projects are every `<dir>/*.mapdata` when none is named (`level_01/day_4` names `level_01_day_4.mapdata`). The
//! tuning is the server's: `SRP_BOT_TUNING` as the server reads it (unset: the code's defaults, as the compose file
//! ships). The store is the server's own (`match_server::v026g::bot_maps`): the files get the keys the server looks
//! for. Each map is built and written, then a second store (a restart) must read every part back with the same
//! digests; anything a restart would build again fails the run (exit 1). The built dir must be empty or absent, so
//! every file in it is this build's.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use match_server::v026g::bot_maps;
use match_server::v026g::bots::maps::{self, Made, Source};
use match_server::v026g::bots::tuning::LoadedTuning;

/// A map's build may take this long before the run fails (the largest take seconds).
const LIMIT: Duration = Duration::from_secs(900);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, built, projects @ ..] = args.as_slice() else {
        eprintln!("usage: srp-build-maps <mapdata dir> <built dir> [<project> ...]");
        return ExitCode::from(2);
    };
    match run(Path::new(dir), Path::new(built), projects) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("srp-build-maps: {why}");
            ExitCode::FAILURE
        }
    }
}

fn projects_in(dir: &Path) -> Result<Vec<String>, String> {
    let mut projects: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".mapdata")?;
            // `level_01_day_4` -> `level_01/day_4`: the level is the first two parts.
            let cut = stem.match_indices('_').nth(1)?.0;
            Some(format!("{}/{}", &stem[..cut], &stem[cut + 1..]))
        })
        .collect();
    projects.sort();
    Ok(projects)
}

fn word(source: Source) -> &'static str {
    match source {
        Source::Shared => "shared",
        Source::Read => "read",
        Source::Built => "built",
    }
}

fn run(dir: &Path, built: &Path, named: &[String]) -> Result<(), String> {
    if std::fs::read_dir(built).is_ok_and(|mut entries| entries.next().is_some()) {
        return Err(format!("{} is not empty: every file there must be this build's", built.display()));
    }
    let loaded = LoadedTuning::from_env()?;
    let tuning = loaded.tuning().clone();
    let projects = if named.is_empty() { projects_in(dir)? } else { named.to_vec() };
    if projects.is_empty() {
        return Err(format!("no .mapdata in {}", dir.display()));
    }
    println!("{} maps from {} into {}; {} (hash {:08x})", projects.len(), dir.display(), built.display(), loaded.data_line(), tuning.hash());
    let (dir, built_dir): (PathBuf, PathBuf) = (dir.into(), built.into());
    let store = || bot_maps(dir.clone(), maps::KEEP, maps::AT_ONCE, Some(built_dir.clone()), &tuning);
    let done = maps::prebuild(store, &projects, LIMIT)?;
    for map in &done {
        let Made { grid, sight, tactics } = map.made;
        println!(
            "{}\t{:.1} s\tgrid {}, sight tree {}, tactics {}\tdigests {:016x} {:016x} {:016x}; read back the same",
            map.project,
            map.seconds,
            word(grid),
            word(sight),
            word(tactics),
            map.digests[0],
            map.digests[1],
            map.digests[2]
        );
    }
    let mut files: Vec<(String, u64)> = std::fs::read_dir(built)
        .map_err(|e| format!("{}: {e}", built.display()))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            Some((entry.file_name().into_string().ok()?, entry.metadata().ok()?.len()))
        })
        .collect();
    files.sort();
    for (name, size) in &files {
        println!("file\t{name}\t{size}");
    }
    println!("{} files, {} MB", files.len(), files.iter().map(|(_, size)| size).sum::<u64>() >> 20);
    Ok(())
}
