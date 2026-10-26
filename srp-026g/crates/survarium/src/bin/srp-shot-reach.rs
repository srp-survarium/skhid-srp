//! `srp-shot-reach`: how far each weapon's shot is heard, from the game's sound files
//! (`survarium::v026g::shot_reach`), for the bots' table (`shot_reach.tsv`).
//!
//! ```text
//! srp-shot-reach <archives> [--check <shot_reach.tsv>]
//!
//! <archives>: --dir <game folder>
//!           | --db <resources.db> [--patch <resources.db.patch_<n>>]...
//! ```
//!
//! Without `--check` it prints the table, `name TAB metres` a line sorted by name. With it, it
//! compares the archives' weapons with the file and says, one a line, `missing <name> <metres>`
//! (a weapon the file lacks), `differs <name> <file's> <archives'>`, `extra <name>` (a row of a
//! weapon the archives do not have) and `silent <name>` (a weapon whose sound chain gives no
//! reach); exit status 1 if any weapon is missing or differs. The patches are laid over
//! `resources.db` in ascending n, as the game mounts them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use resources_db::Archive;
use resources_db::overlay::Overlay;
use resources_db::server::{archives_in, patch_number};
use survarium::v026g::shot_reach::{self, config_path};

const USAGE: &str = "usage: srp-shot-reach (--dir <game folder> | --db <resources.db> [--patch <file>]...) [--check <shot_reach.tsv>]";

fn main() -> ExitCode {
    match run() {
        Ok(ok) => {
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let mut args = std::env::args().skip(1);
    let (mut dir, mut db, mut patches, mut check) = (None, None, Vec::new(), None);
    while let Some(arg) = args.next() {
        let mut value = |what: &str| args.next().ok_or_else(|| format!("{what} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--dir" => dir = Some(PathBuf::from(value("--dir")?)),
            "--db" => db = Some(PathBuf::from(value("--db")?)),
            "--patch" => patches.push(PathBuf::from(value("--patch")?)),
            "--check" => check = Some(PathBuf::from(value("--check")?)),
            _ => return Err(format!("{arg}?\n{USAGE}")),
        }
    }
    let (db, mut patches) = match (dir, db) {
        (Some(dir), None) if patches.is_empty() => archives_in(&dir).map_err(|e| e.to_string())?,
        (None, Some(db)) => (db, patches),
        _ => return Err(USAGE.to_owned()),
    };
    patches.sort_by_key(|p| p.file_name().and_then(|n| n.to_str()).and_then(patch_number));
    let mut archives = Vec::new();
    let mut names = Vec::new();
    for path in std::iter::once(&db).chain(&patches) {
        let at = |cause: &dyn std::fmt::Display| format!("{}: {cause}", path.display());
        let file = std::fs::File::open(path).map_err(|e| at(&e))?;
        archives.push(Archive::new(file).map_err(|e| at(&e))?);
        names.push(path.display().to_string());
    }
    let mut overlay = Overlay::new(archives, &names).map_err(|e| e.to_string())?;
    let stems: Vec<String> = overlay
        .files_under("gameplay/items/weapons/")
        .filter_map(|(path, _, _)| {
            let stem = path.strip_prefix("gameplay/items/weapons/")?.strip_suffix(".options")?;
            (stem.starts_with("wpn_") && !stem.contains('/')).then(|| stem.to_owned())
        })
        .collect();
    let mut weapons: Vec<(String, Option<f32>)> = Vec::new();
    for stem in stems {
        let bytes = overlay
            .read(&config_path(&stem))
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{stem}: listed and not read"))?;
        let mut read = |path: &str| overlay.read(path).map_err(|e| format!("{path}: {e}"));
        let reach = shot_reach::reach(&bytes, &mut read).map_err(|e| format!("{stem}: {e}"))?;
        weapons.push((stem, reach));
    }
    let Some(check) = check else {
        print!("{}", shot_reach::rows(&weapons));
        return Ok(true);
    };
    let text = std::fs::read_to_string(&check).map_err(|e| format!("{}: {e}", check.display()))?;
    let mut have: BTreeMap<&str, &str> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (name, metres) = line.split_once('\t').ok_or_else(|| format!("{line}?"))?;
        have.insert(name, metres);
    }
    let mut ok = true;
    for (name, reach) in &weapons {
        match (reach, have.get(name.as_str())) {
            (None, _) => println!("silent {name}"),
            (Some(r), None) => {
                println!("missing {name} {r}");
                ok = false;
            }
            (Some(r), Some(file)) if file.parse::<f32>().ok() != Some(*r) => {
                println!("differs {name} {file} {r}");
                ok = false;
            }
            _ => {}
        }
    }
    for name in have.keys() {
        if !weapons.iter().any(|(w, _)| w == name) {
            println!("extra {name}");
        }
    }
    Ok(ok)
}
