//! The host build's verdicts on kept worlds (`world_frame_check.md`):
//!
//! ```text
//! cargo run --release -p world-frame --example verdicts -- DIR...
//! ```
//!
//! Every `match026g_<match>_<time>_*.bin` under the directories (the
//! server's `SRP_SNAPSHOT_DIR` dumps, a body each; no symbolic link is
//! followed), checked as a game checks it: its time, then the body, at all
//! 20 slots. One line a world on stdout, `<path>\t<time>\t<verdicts>`: `0`
//! (loads) or `1` (refused), the world's first, then one for each of its
//! [`MUTATIONS`] mutated copies (the body's byte at `len * k / MUTATIONS`
//! XOR 0xFF). `check_world_check_026g.py --drift` runs the arena's i686
//! build on the same bytes and asserts the two agree. On stderr: each
//! directory's worlds, the refused ones with their reasons, the largest
//! section and state and the highest slot.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use world_frame::{Frame, PLAYER_STATE_CAP, Places, Section};

/// The mutated copies of each world.
const MUTATIONS: usize = 16;

#[derive(Default)]
struct Seen {
    worlds: usize,
    refused: BTreeMap<String, usize>,
    largest: [usize; 6],
    state: usize,
    slot: Option<u8>,
}

fn main() {
    let dirs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    assert!(!dirs.is_empty(), "usage: verdicts DIR...");
    for dir in &dirs {
        let mut files = Vec::new();
        collect(dir, &mut files);
        files.sort();
        let mut seen = Seen::default();
        for path in &files {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let Some(time) = name.split('_').nth(2).and_then(|t| t.parse::<u32>().ok()) else {
                continue;
            };
            let body = std::fs::read(path).expect("a world file reads");
            let verdict = |body: &[u8]| {
                let mut state = time.to_le_bytes().to_vec();
                state.extend_from_slice(body);
                u8::from(Frame::parse_state(&state, Places::ALL).is_err())
            };
            let mut verdicts = vec![verdict(&body)];
            for k in 0..MUTATIONS {
                let mut mutated = body.clone();
                mutated[body.len() * k / MUTATIONS] ^= 0xFF;
                verdicts.push(verdict(&mutated));
            }
            let line: String = verdicts.iter().map(u8::to_string).collect();
            println!("{}\t{time}\t{line}", path.display());
            note(&mut seen, &body);
        }
        eprintln!(
            "{}: {} worlds, {} refused {:?}; largest bullets {:#x}, statistics {:#x}, objects {:#x}, \
             physics {:#x}, rules {:#x}, effects {:#x} (caps {:x?}), a state {:#x} (cap {:#x}), \
             highest slot {:?}",
            dir.display(),
            seen.worlds,
            seen.refused.values().sum::<usize>(),
            seen.refused,
            seen.largest[0],
            seen.largest[1],
            seen.largest[2],
            seen.largest[3],
            seen.largest[4],
            seen.largest[5],
            Section::ALL.map(Section::cap),
            seen.state,
            PLAYER_STATE_CAP,
            seen.slot,
        );
    }
}

/// What the world adds to its directory's numbers.
fn note(seen: &mut Seen, body: &[u8]) {
    seen.worlds += 1;
    match Frame::parse(body, Places::ALL) {
        Ok(frame) => {
            for (largest, section) in seen.largest.iter_mut().zip(Section::ALL) {
                *largest = (*largest).max(frame.section(section).bytes.len());
            }
            seen.slot = seen.slot.max(frame.slots().iter().map(|slot| slot.index()).last());
            // The states: walk the blocks again, by their prefixes.
            let mut at = 4;
            for _ in frame.slots().iter() {
                let len = usize::from(u16::from_le_bytes([body[at], body[at + 1]]));
                seen.state = seen.state.max(len);
                at += 2 + len;
                let animated = body[at] != 0;
                at += 1;
                if animated {
                    at += world_frame::animation_len(&body[at..]).expect("a frame that parsed");
                }
            }
        }
        Err(refusal) => {
            // The reason without its offset.
            let text = refusal.to_string();
            let reason = text.split_once(": ").map_or(text.as_str(), |(_, why)| why);
            *seen
                .refused
                .entry(format!("{}: {reason}", refusal.part()))
                .or_default() += 1;
        }
    }
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
