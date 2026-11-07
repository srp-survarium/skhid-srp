//! Checks real worlds as the match server checks every upload before it
//! relays it (`match_server::v026g::world_check`): the uploads of match
//! recordings with each match's own roster, and kept world dumps of the
//! matches whose recordings are given with them.
//!
//! ```text
//! srp-world-check --data <server-game-data.db> [--save <dir>] [--revision <n>] <file or folder>...
//! ```
//!
//! A folder stands for the `*.srprec` and `match026g_*.bin` files in it.
//! `--data`: the server's game data the check reads maps and items from (a
//! release's own). `--revision <n>`: only recordings that games of revision
//! `n` played (the others are listed as skipped). `--save <dir>`: each
//! refused world written there under the server's dump name, to check again
//! later or read. Exit 0 when every world passed and at least one was
//! checked; 1 when a world was refused, or could not be checked, or none
//! was; 2 for a usage error.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use match_server::v026g::world_check::{self, Recording, Unchecked};

/// Refusals printed in full, for each file.
const SHOWN: usize = 3;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut data = None;
    let mut save = None;
    let mut revision = None;
    let mut inputs = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" => data = args.next(),
            "--save" => save = args.next().map(PathBuf::from),
            "--revision" => match args.next().and_then(|n| n.parse::<u64>().ok()) {
                Some(n) => revision = Some(n),
                None => return usage("--revision takes a number"),
            },
            "-h" | "--help" => return usage(""),
            other if other.starts_with("--") => return usage(&format!("unknown option {other}")),
            _ => inputs.push(PathBuf::from(arg)),
        }
    }
    let Some(data) = data else {
        return usage(
            "--data <server-game-data.db> is needed: the check reads the maps and items from it",
        );
    };
    if inputs.is_empty() {
        return usage("no recording or dump given");
    }
    if let Err(error) = survarium::v026g::game_data::init_from(Path::new(&data)) {
        eprintln!("{data}: {error}");
        return ExitCode::FAILURE;
    }
    let (mut recordings, mut dumps) = (Vec::new(), Vec::new());
    for input in &inputs {
        if let Err(error) = files_of(input, &mut recordings, &mut dumps) {
            eprintln!("{}: {error}", input.display());
            return ExitCode::FAILURE;
        }
    }
    if let Some(dir) = &save
        && let Err(error) = std::fs::create_dir_all(dir)
    {
        eprintln!("{}: {error}", dir.display());
        return ExitCode::FAILURE;
    }

    let mut tally = Tally::default();
    // The dumps by match, checked with their match's recording.
    let mut dumps_of: HashMap<u32, Vec<(PathBuf, u32)>> = HashMap::new();
    for dump in dumps {
        let name = dump
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        match world_check::dump_of_name(name) {
            Some((id, time, _)) => dumps_of.entry(id).or_default().push((dump, time)),
            None => {
                println!(
                    "{}: not a dump's name (match026g_<id>_<ms>_p<place>_..bin)",
                    dump.display()
                );
                tally.unchecked += 1;
            }
        }
    }
    for path in &recordings {
        let name = path.display();
        let recording = match world_check::read_recording(path) {
            Ok(recording) => recording,
            Err(error) => {
                println!("{name}: UNREAD: {error}");
                tally.unread += 1;
                continue;
            }
        };
        if let Some(wanted) = revision
            && !recording.revisions.contains(&wanted)
        {
            println!(
                "{name}: skipped: played by games of revision {:?}, not {wanted}",
                recording.revisions
            );
            tally.skipped += 1;
            if let Some(id) = recording.match_id {
                dumps_of.remove(&id);
            }
            continue;
        }
        let mut file = FileTally::default();
        for upload in &recording.uploads {
            let refused = file.note(&recording, &upload.body, upload.time, || {
                format!("p{} t{}", upload.place, upload.time)
            });
            if let Some(dir) = save.as_ref().filter(|_| refused) {
                let id = recording.match_id.unwrap_or(0);
                let out = dir.join(format!(
                    "match026g_{id:x}_{}_p{}_refused.bin",
                    upload.time, upload.place
                ));
                if let Err(error) = std::fs::write(&out, &upload.body) {
                    eprintln!("{}: {error}", out.display());
                }
            }
        }
        for (dump, time) in recording
            .match_id
            .and_then(|id| dumps_of.remove(&id))
            .unwrap_or_default()
        {
            match std::fs::read(&dump) {
                Ok(body) => {
                    file.note(&recording, &body, time, || dump.display().to_string());
                }
                Err(error) => {
                    println!("{}: UNREAD: {error}", dump.display());
                    tally.unread += 1;
                }
            }
        }
        let map = recording.options.as_ref().map_or("no options".into(), |o| {
            format!(
                "map {} ({}), {} players",
                o.map.map().id,
                o.map.map().name,
                o.players.len()
            )
        });
        println!(
            "{name}: {} worlds, {} passed, {} refused, {} not checked; {map}; revisions {:?}{}",
            file.checked + file.unchecked,
            file.checked - file.refused,
            file.refused,
            file.unchecked,
            recording.revisions,
            if recording.unparsed_options > 0 {
                format!(
                    "; {} 0xE4s did not read (parts of resends that share a unique id, or an item this data lacks)",
                    recording.unparsed_options
                )
            } else {
                String::new()
            }
        );
        for line in &file.shown {
            println!("  REFUSED {line}");
        }
        tally.add(file);
    }
    for (id, dumps) in dumps_of {
        println!(
            "match {id:x}: {} dumps not checked: no recording of the match was given",
            dumps.len()
        );
        tally.unchecked += dumps.len();
    }
    println!("--- refusals by kind");
    for (kind, n) in &tally.kinds {
        println!("{n}\t{kind}");
    }
    let passed = tally.checked - tally.refused;
    println!(
        "{} worlds checked: {passed} passed, {} refused; {} not checked, {} files unread, {} recordings skipped",
        tally.checked, tally.refused, tally.unchecked, tally.unread, tally.skipped
    );
    if tally.refused > 0 || tally.unchecked > 0 || tally.unread > 0 {
        println!(
            "FAILED: the match server would refuse worlds games wrote, or could not check them"
        );
        ExitCode::FAILURE
    } else if tally.checked == 0 {
        println!("FAILED: no world was checked");
        ExitCode::FAILURE
    } else {
        println!("ok: the match server takes every world");
        ExitCode::SUCCESS
    }
}

fn usage(why: &str) -> ExitCode {
    if !why.is_empty() {
        eprintln!("{why}");
    }
    eprintln!(
        "usage: srp-world-check --data <server-game-data.db> [--save <dir>] [--revision <n>] <recording, dump or folder>..."
    );
    ExitCode::from(2)
}

/// The recordings and dumps of `input`: itself, or a folder's (not its
/// subfolders'), in name order.
fn files_of(
    input: &Path,
    recordings: &mut Vec<PathBuf>,
    dumps: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    let mut files = if input.is_dir() {
        std::fs::read_dir(input)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![input.to_path_buf()]
    };
    files.sort();
    for file in files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.ends_with(".srprec") {
            recordings.push(file);
        } else if name.starts_with("match026g_") && name.ends_with(".bin") {
            dumps.push(file);
        } else if !input.is_dir() {
            return Err(std::io::Error::other(
                "neither a recording (.srprec) nor a dump (match026g_*.bin)",
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct FileTally {
    checked: usize,
    refused: usize,
    unchecked: usize,
    kinds: BTreeMap<String, usize>,
    shown: Vec<String>,
}

impl FileTally {
    /// Checks one world of `recording`'s match; whether it was refused.
    fn note(
        &mut self,
        recording: &Recording,
        body: &[u8],
        time: u32,
        what: impl Fn() -> String,
    ) -> bool {
        match recording.check(body, time) {
            Ok(()) => self.checked += 1,
            Err(Unchecked::Refused(refused)) => {
                self.checked += 1;
                self.refused += 1;
                let refusal = refused.refusal();
                *self
                    .kinds
                    .entry(format!("{}: {}", refusal.part, refusal.reason))
                    .or_default() += 1;
                if self.shown.len() < SHOWN {
                    self.shown.push(format!("{}: {refused}", what()));
                }
                return true;
            }
            Err(Unchecked::NoOptions) => self.unchecked += 1,
        }
        false
    }
}

#[derive(Default)]
struct Tally {
    checked: usize,
    refused: usize,
    unchecked: usize,
    unread: usize,
    skipped: usize,
    kinds: BTreeMap<String, usize>,
}

impl Tally {
    fn add(&mut self, file: FileTally) {
        self.checked += file.checked;
        self.refused += file.refused;
        self.unchecked += file.unchecked;
        for (kind, n) in file.kinds {
            *self.kinds.entry(kind).or_default() += n;
        }
    }
}
