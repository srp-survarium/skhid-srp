//! A replay of a map with nobody in it (`sur_convert::of_map`): the game
//! started with `-match_replay=<it>` goes from its start straight into the
//! map, with no server, sign-in or lobby (`scripts/capture`).
//!
//! ```text
//! srp-map-sur <maps_dict id> <out.sur> [--minutes <n>]
//!             [--hold <gun dict id>[:<optic dict id>[:<rounds dict id>]] [--enter <ms>] [--aim <ms>] [--move <ms>]
//!              [--turn <yaw degrees>[:<pitch degrees>]]]
//! ```
//!
//! With `--hold` (diagnostics) its one place enters with that gun at
//! `--enter` (1000 ms) and aims it from `--aim` (30000 ms) to the end;
//! from `--move` it strafes left and right by turns while aiming; `--turn`
//! turns it once, a second after it enters (whole degrees, right and up positive).
//! The map's options and mode config are the game data's (`SRP_GAME_DATA`).

use std::process::ExitCode;

use match_server::v026g::sur_convert::{self, Holder};
use survarium::v026g::maps::{MapId, MatchMap};

const USAGE: &str = "usage: srp-map-sur <maps_dict id> <out.sur> [--minutes <n>] \
                     [--hold <gun>[:<optic>[:<rounds>]] [--enter <ms>] [--aim <ms>] [--move <ms>] [--turn <yaw>[:<pitch>]]]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut plain = Vec::new();
    let mut options = std::collections::HashMap::new();
    let mut words = args.iter();
    while let Some(arg) = words.next() {
        match arg.strip_prefix("--") {
            Some(name) => {
                let Some(value) = words.next() else {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                };
                options.insert(name.to_owned(), value.clone());
            }
            None => plain.push(arg.clone()),
        }
    }
    let number = |name: &str, default: u32| options.get(name).map_or(Some(default), |v| v.parse::<u32>().ok());
    let hold = options.get("hold").map(|spec| {
        let ids: Option<Vec<u16>> = spec.split(':').map(|id| id.parse().ok()).collect();
        let ids = ids.filter(|ids| (1..=3).contains(&ids.len()))?;
        Some(Holder {
            weapon: ids[0],
            scope: ids.get(1).copied(),
            ammo: ids.get(2).copied(),
            enter_ms: number("enter", 1_000)?,
            aim_ms: number("aim", 30_000)?,
            move_ms: match options.get("move") {
                Some(v) => Some(v.parse::<u32>().ok()?),
                None => None,
            },
            turn: match options.get("turn") {
                Some(v) => {
                    let mut parts = v.split(':').map(|d| d.parse::<i16>().ok());
                    let yaw = parts.next().flatten()?;
                    let pitch = parts.next().map_or(Some(0), |p| p)?;
                    Some(parts.next().is_none().then_some((yaw, pitch))?)
                }
                None => None,
            },
        })
    });
    let known = options.keys().all(|k| ["minutes", "hold", "enter", "aim", "move", "turn"].contains(&k.as_str()));
    let (true, Some(minutes), [id, out]) = (known, number("minutes", 30), plain.as_slice()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let holder = match hold {
        None => None,
        Some(Some(holder)) => Some(holder),
        Some(None) => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(error) = survarium::v026g::game_data::init() {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    let Some(map) = id.parse().ok().map(MapId::new).and_then(MatchMap::of) else {
        eprintln!("map {id}: no playable maps_dict row of that id in the game data");
        return ExitCode::FAILURE;
    };
    match sur_convert::of_map(map, minutes.saturating_mul(60_000), holder.as_ref()) {
        Ok(file) => match std::fs::write(out, &file) {
            Ok(()) => {
                println!("{out}: map {id} ({}, {}), {} bytes", map.map().project, map.map().config, file.len());
                if let Some(holder) = holder {
                    let name = |id: u16| survarium::v026g::dictionary::Dictionary::shared().item(id).map_or("?".to_owned(), |item| item.name.to_string());
                    println!("holding {} ({}) with {} ({})", name(holder.weapon), holder.weapon, holder.scope.map_or("no optic".to_owned(), name), holder.scope.unwrap_or(0));
                }
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
