//! Runs the server's world check (`survarium::v026g::match_msgs::world`)
//! over saved worlds: every `match026g_*.bin` under the directories given,
//! and prints one line per refused world and a count per dump kind.
//!
//! ```text
//! cargo run -p match-server --example world_check -- [--log FILE]... DIR...
//! ```
//!
//! A world's time is in its file name (`match026g_<id>_<ms>_p<n>_<kind>_r<run>.bin`).
//! What the match was (map, mode, players) comes from a server log given
//! with `--log` (the match server's "Match 0x.. made: map N (.., mode M),
//! P players" line). Without one, the places, the mode and the batteries
//! are found as the first that let the statistics and the rules read
//! (respawn points and profiles unknown: those checks are skipped); the
//! line then says "guessed".

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use survarium::v026g::maps::{self, mode};
use survarium::v026g::match_msgs::MAX_PLAYERS;
use survarium::v026g::match_msgs::world::{self, Context, Refusal};

struct Known {
    places: usize,
    game_mode: u8,
    batteries: usize,
    respawn_points: Option<usize>,
}

fn main() {
    let mut logs = Vec::new();
    let mut dirs = Vec::new();
    let mut verbose = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "-v" {
            verbose = true;
        } else if arg == "--log" {
            logs.push(PathBuf::from(args.next().expect("--log FILE")));
        } else {
            dirs.push(PathBuf::from(arg));
        }
    }
    assert!(
        !dirs.is_empty(),
        "usage: world_check [--log FILE]... DIR..."
    );
    let known = read_logs(&logs);
    let mut files = Vec::new();
    for dir in &dirs {
        collect(dir, &mut files);
    }
    files.sort();
    // kind -> (passed, refused)
    let mut kinds: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut guessed = 0;
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some((id, time, kind)) = parse_name(&name) else {
            continue;
        };
        let body = std::fs::read(path).unwrap();
        let entry = kinds.entry(kind.clone()).or_default();
        let result = match known.get(&id) {
            // The roster the log names, or the 20 places of a growing
            // roster (the server then sends the clients a roster of 20:
            // `snapshot_players`), which the log does not say.
            Some(match_) => [match_.places, MAX_PLAYERS]
                .into_iter()
                .map(|places| {
                    world::validate(
                        &body,
                        &Context {
                            time,
                            places,
                            game_mode: match_.game_mode,
                            batteries: match_.batteries,
                            respawn_points: match_.respawn_points,
                            profiles: None,
                        },
                    )
                })
                // Either passing is a pass; else the log's roster's
                // refusal, the one that says what is wrong.
                .reduce(|logged, full| if full.is_ok() { full } else { logged })
                .expect("two rosters"),
            None => {
                guessed += 1;
                guess(&body, time).map(|found| {
                    if verbose {
                        println!("PASS {} as {found}", path.display());
                    }
                })
            }
        };
        match result {
            Ok(()) => entry.0 += 1,
            Err(refusal) => {
                entry.1 += 1;
                *reasons.entry(class(&refusal)).or_default() += 1;
                println!("REFUSED {}: {refusal}", path.display());
            }
        }
    }
    println!();
    println!(
        "{} worlds ({guessed} with a guessed match)",
        files_counted(&kinds)
    );
    for (kind, (passed, refused)) in &kinds {
        println!("  {kind:<14} {passed:>6} pass {refused:>6} refused");
    }
    if !reasons.is_empty() {
        println!("refusals by reason:");
        for (reason, count) in &reasons {
            println!("  {count:>6}  {reason}");
        }
    }
}

fn files_counted(kinds: &BTreeMap<String, (usize, usize)>) -> usize {
    kinds.values().map(|(a, b)| a + b).sum()
}

/// A refusal without its numbers, to count alike ones together.
fn class(refusal: &Refusal) -> String {
    let words: String = refusal
        .reason
        .split_whitespace()
        .filter(|word| !word.chars().any(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ");
    let section = refusal
        .section
        .trim_end_matches(|c: char| c.is_ascii_digit());
    format!("{section}: {words}")
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Never through a link: a Wine prefix's `dosdevices/z:` is `/`.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect(&path, files);
        } else if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("match026g_"))
            && path.extension().is_some_and(|ext| ext == "bin")
        {
            files.push(path);
        }
    }
}

/// `match026g_<id hex>_<ms>_p<n>_<kind>_r<run>.bin` -> (id, ms, kind).
fn parse_name(name: &str) -> Option<(u32, u32, String)> {
    let stem = name.strip_prefix("match026g_")?.strip_suffix(".bin")?;
    let mut parts = stem.split('_');
    let id = u32::from_str_radix(parts.next()?, 16).ok()?;
    let time = parts.next()?.parse().ok()?;
    let _place = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    // The kind may itself hold '_'; the run stamp is last.
    let kind = rest[..rest.len().saturating_sub(1)].join("_");
    Some((id, time, kind))
}

/// The matches the logs made: "Match 0x<id> made: map <n> (<name>, mode
/// <m>), <p> players".
fn read_logs(logs: &[PathBuf]) -> HashMap<u32, Known> {
    let mut known = HashMap::new();
    for log in logs {
        let Ok(text) = std::fs::read_to_string(log) else {
            eprintln!("can't read {}", log.display());
            continue;
        };
        for line in text.lines() {
            let Some(at) = line.find("Match 0x") else {
                continue;
            };
            let rest = &line[at + "Match 0x".len()..];
            let Some((id, rest)) = rest.split_once(" made: map ") else {
                continue;
            };
            let Ok(id) = u32::from_str_radix(id, 16) else {
                continue;
            };
            let Some((map, rest)) = rest.split_once(' ') else {
                continue;
            };
            let Some(players) = rest
                .rsplit_once(", ")
                .and_then(|(_, tail)| tail.strip_suffix(" players"))
                .and_then(|n| n.parse::<usize>().ok())
            else {
                continue;
            };
            let Some(map) = map.parse::<u8>().ok().and_then(maps::map_mode) else {
                continue;
            };
            known.insert(
                id,
                Known {
                    places: players,
                    game_mode: map.mode,
                    batteries: map.victory_items_count as usize,
                    respawn_points: Some(map.respawn_points),
                },
            );
        }
    }
    known
}

/// The check with the first match that lets the statistics and rules
/// read, and that match in words; with none, the refusal of the guess
/// that got furthest.
fn guess(body: &[u8], time: u32) -> Result<String, Refusal> {
    let mut best: Option<(usize, Refusal)> = None;
    for game_mode in [
        mode::BATTERIES,
        mode::TIMED_BATTERIES,
        mode::TEAM_DEATHMATCH,
    ] {
        let batteries: Vec<usize> = if game_mode == mode::TEAM_DEATHMATCH {
            vec![0]
        } else {
            (1..=16).collect()
        };
        for places in 1..=20 {
            for &batteries in &batteries {
                let context = Context {
                    time,
                    places,
                    game_mode,
                    batteries,
                    respawn_points: None,
                    profiles: None,
                };
                match world::validate(body, &context) {
                    Ok(()) => {
                        return Ok(format!(
                            "{places} places, mode {game_mode}, {batteries} batteries (guessed)"
                        ));
                    }
                    Err(refusal) => {
                        let depth = match refusal.section.as_str() {
                            "frame" => 0,
                            section if section.starts_with("player") => 1,
                            "statistics" => 2,
                            _ => 3,
                        };
                        if best.as_ref().is_none_or(|(held, _)| depth > *held) {
                            best = Some((depth, refusal));
                        }
                    }
                }
            }
        }
    }
    Err(best.expect("at least one guess").1)
}
