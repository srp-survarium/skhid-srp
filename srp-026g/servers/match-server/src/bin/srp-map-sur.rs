//! A replay of a map with nobody in it (`sur_convert::of_map`): the game
//! started with `-match_replay=<it>` goes from its start straight into the
//! map, with no server, sign-in or lobby (`scripts/capture`).
//!
//! ```text
//! srp-map-sur <maps_dict id> <out.sur> [--minutes <n>]
//! ```
//!
//! The map's options and mode config are the game data's (`SRP_GAME_DATA`).

use std::process::ExitCode;

use match_server::v026g::sur_convert;
use survarium::v026g::maps::{MapId, MatchMap};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let minutes = args
        .iter()
        .position(|arg| arg == "--minutes")
        .map(|at| args.get(at + 1).and_then(|v| v.parse::<u32>().ok()));
    let plain: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|&(i, arg)| !arg.starts_with("--") && (i == 0 || args[i - 1] != "--minutes"))
        .map(|(_, arg)| arg)
        .collect();
    let (Some(minutes), [id, out]) = (minutes.unwrap_or(Some(30)), plain.as_slice()) else {
        eprintln!("usage: srp-map-sur <maps_dict id> <out.sur> [--minutes <n>]");
        return ExitCode::from(2);
    };
    if let Err(error) = survarium::v026g::game_data::init() {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    let Some(map) = id.parse().ok().map(MapId::new).and_then(MatchMap::of) else {
        eprintln!("map {id}: no playable maps_dict row of that id in the game data");
        return ExitCode::FAILURE;
    };
    match sur_convert::of_map(map, minutes.saturating_mul(60_000)) {
        Ok(file) => match std::fs::write(out, &file) {
            Ok(()) => {
                println!("{out}: map {id} ({}, {}), {} bytes", map.map().project, map.map().config, file.len());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{out}: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("map {id}: {error}");
            ExitCode::FAILURE
        }
    }
}
