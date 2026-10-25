//! The host side of the client check's drift test (`client_world_check.md`
//! section 8): for every kept world under the directories given, the context
//! the game's payload reads (20 places; the mode, batteries and respawn
//! points of the match; no profiles, artefacts, objects or inventory) and
//! the **host** build's verdict, for the world and for mutated copies of it.
//!
//! `check_client_verify_026g.py` runs this, then the arena's i686 payload
//! (`srp_world_check`) under Unicorn with a world core whose rule list says
//! the same match, on the same bytes, and asserts the two builds agree.
//!
//! ```text
//! cargo run --release -p srp-validate --example drift_dump -- DIR...
//! ```
//!
//! One line a world: `<path>\t<time>\t<mode>\t<batteries>\t<respawn>\t<verdicts>`,
//! the verdicts `0` (read) or `1` (refused), the world's first, then one for
//! each of [`MUTATIONS`] (the byte at `len * k / MUTATIONS` flipped). The
//! match is the first at 20 places that lets the world pass; a world that
//! passes under none reports mode 2, no batteries, no respawn points and its
//! refusal (the payload reads the same match, so the two still agree).
//! A kept world is the body after the time (`match026g_<match>_<time>_...`):
//! the game's reader has the time first, so the payload gets both.

use std::path::{Path, PathBuf};

use srp_validate::world::{self, Context};
use srp_validate::{MAX_PLAYERS, mode};

/// The mutated copies of each world.
pub const MUTATIONS: usize = 16;
const MAX_RESPAWN: usize = 127;
const MAX_BATTERIES: usize = 16;

fn main() {
    let dirs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    assert!(!dirs.is_empty(), "usage: drift_dump DIR...");
    let mut files = Vec::new();
    for dir in &dirs {
        collect(dir, &mut files);
    }
    files.sort();
    for path in &files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let Some(time) = name.split('_').nth(2).and_then(|t| t.parse::<u32>().ok()) else {
            continue;
        };
        let body = std::fs::read(path).expect("a world file reads");
        let (game_mode, batteries, respawn) = choose(&body, time);
        let context = context(time, game_mode, batteries, respawn);
        let verdict = |body: &[u8]| u8::from(world::validate(body, &context).is_err());
        let mut verdicts = vec![verdict(&body)];
        for k in 0..MUTATIONS {
            let mut mutated = body.clone();
            if let Some(byte) = mutated.get_mut(body.len() * k / MUTATIONS) {
                *byte ^= 0xFF;
            }
            verdicts.push(verdict(&mutated));
        }
        let verdicts: String = verdicts.iter().map(u8::to_string).collect();
        println!(
            "{}\t{time}\t{game_mode}\t{batteries}\t{respawn}\t{verdicts}",
            path.display()
        );
    }
}

/// The context the payload's `Match::context` builds.
fn context(time: u32, game_mode: u8, batteries: usize, respawn: usize) -> Context<'static> {
    Context {
        time,
        places: MAX_PLAYERS,
        game_mode,
        batteries,
        respawn_points: Some(respawn),
        profiles: None,
        artefacts: None,
        objects: None,
        inventory: None,
    }
}

/// The first `(game_mode, batteries, respawn)` at 20 places that lets the
/// world pass; mode 2 with none of either when none does.
fn choose(body: &[u8], time: u32) -> (u8, usize, usize) {
    let modes = [
        mode::BATTERIES,
        mode::TIMED_BATTERIES,
        mode::TEAM_DEATHMATCH,
        mode::RESEARCH,
    ];
    let Ok(frame) = world::frame(body, MAX_PLAYERS) else {
        return (mode::BATTERIES, 0, 0);
    };
    for game_mode in modes {
        let most = if mode::WITH_BATTERIES.contains(&game_mode) {
            MAX_BATTERIES
        } else {
            0
        };
        for batteries in 0..=most {
            if world::statistics(&frame.statistics(), &context(time, game_mode, batteries, 0))
                .is_err()
            {
                continue;
            }
            for respawn in 0..=MAX_RESPAWN {
                let context = context(time, game_mode, batteries, respawn);
                if world::rules(&frame.rules(), &context).is_ok()
                    && world::validate(body, &context).is_ok()
                {
                    return (game_mode, batteries, respawn);
                }
            }
        }
    }
    (mode::BATTERIES, 0, 0)
}

/// Every `match026g_*.bin` under `dir`, following no symbolic link (a Wine
/// prefix's `dosdevices/z:` is `/`).
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
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
