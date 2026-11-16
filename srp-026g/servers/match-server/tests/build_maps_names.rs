//! `srp-build-maps --files` names the files a live server reads its maps from: the server's own store
//! (`match_server::v026g::bot_maps`) after `survarium::v026g::game_data::init`, as `srp` runs it. A grid's key holds the
//! fastest bot's walk cap, which the game data's bot profiles set: a pre-build that read no game data named its grids and
//! tactics by the test archive's cap (1.126 against 1.166 of the rev56 data at 7586d390f), files no server reads.
//!
//! It needs the real data, so it is ignored by default:
//!
//! ```text
//! SRP_GAME_DATA=<server-game-data.db> SRP_MAPDATA_DIR=<the bots' mapdata dir> \
//!     cargo test -p match-server --test build_maps_names -- --ignored
//! ```
//!
//! `SRP_BUILD_MAPS_TEST_PROJECTS` (space-separated, e.g. `level_01/day_2 level_06/day_2`) narrows the maps; unset, every
//! `.mapdata` of the folder.

use std::path::{Path, PathBuf};
use std::process::Command;

use match_server::v026g::bot_maps;
use match_server::v026g::bots::maps;
use match_server::v026g::bots::tuning::LoadedTuning;

fn env_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} is unset (see the file's doc)"))
}

fn projects() -> Vec<String> {
    std::env::var("SRP_BUILD_MAPS_TEST_PROJECTS")
        .map(|list| list.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The bin's `--files` lines, `(project, [grid, sight, tactics])`, with `SRP_GAME_DATA` as given (`None`: removed), and
/// its stderr.
fn bin_files(dir: &Path, projects: &[String], data: Option<&Path>) -> (Vec<(String, [String; 3])>, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_srp-build-maps"));
    command.arg("--files").arg(dir).args(projects);
    match data {
        Some(data) => command.env("SRP_GAME_DATA", data),
        None => command.env_remove("SRP_GAME_DATA"),
    };
    let out = command.output().expect("srp-build-maps runs");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "srp-build-maps --files failed: {stderr}");
    let lines = String::from_utf8(out.stdout)
        .expect("utf-8")
        .lines()
        .map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            let [project, grid, sight, tactics] = parts[..] else { panic!("not 4 columns: {line}") };
            (project.to_owned(), [grid.to_owned(), sight.to_owned(), tactics.to_owned()])
        })
        .collect();
    (lines, stderr)
}

#[test]
#[ignore = "needs SRP_GAME_DATA (a server-game-data.db) and SRP_MAPDATA_DIR (the bots' .mapdata folder)"]
fn files_are_a_post_init_servers_names() {
    let data = env_path("SRP_GAME_DATA");
    let dir = env_path("SRP_MAPDATA_DIR");
    let projects = projects();
    let (named, log) = bin_files(&dir, &projects, Some(&data));
    assert!(!named.is_empty(), "no maps named in {}", dir.display());
    assert!(!log.contains("WARNING"), "the run with game data warned: {log}");
    assert!(log.contains("walk cap the grids fly"), "no walk cap in the start log: {log}");

    // The server: its game data first (`srp`'s main), then the tuning and the store.
    survarium::v026g::game_data::init_from(&data).expect("the game data reads and checks");
    let tuning = LoadedTuning::from_env().expect("the tuning").tuning().clone();
    let store = bot_maps(dir.clone(), maps::KEEP, maps::AT_ONCE, None, &tuning, maps::AtLoad::Refuse);
    for (project, files) in &named {
        let server = store.built_file_names(project).expect("the server names the map's files");
        assert_eq!(files, &server, "{project}: srp-build-maps --files against a post-init server");
    }
    eprintln!("{} maps: the bin's names are the post-init server's", named.len());

    // Without game data the run still names files, and says in its log that no live server reads them.
    let (_, unset) = bin_files(&dir, &named[..1].iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(), None);
    assert!(unset.contains("WARNING: SRP_GAME_DATA is unset"), "no warning without game data: {unset}");
}
