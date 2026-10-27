//! What the bots of a match run on, for the logs: the data files they read (with a SHA-256 prefix, so a file that was
//! swapped or is missing shows at once), and the environment switches that are read once (`SRP_BOT_KIT`,
//! `SRP_BOT_UPGRADES`). The tuning's switches are [`super::tuning::SwitchReport`]. Logging only: nothing here is read by a
//! decision, and it is called once per match (the server) or once per run (the bench), never per tick.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::Path;

use super::{foliage, grass, routes};

/// `sha256:` and the first 12 hex digits of the SHA-256 of `bytes`.
pub fn bytes_sha(bytes: &[u8]) -> String {
    let digest = openssl::sha::sha256(bytes);
    let mut hex = String::with_capacity(12);
    for byte in &digest[..6] {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}

/// [`bytes_sha`] of a file, or why it could not be read.
pub fn sha_prefix(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => bytes_sha(&bytes),
        Err(why) => format!("unreadable ({why})"),
    }
}

/// The tuning part of the data line for a server: the file `SRP_BOT_TUNING` names (with its hash), or the defaults.
pub fn tuning_file(var: Option<&OsStr>) -> String {
    match var.filter(|path| !path.is_empty()) {
        Some(path) => file("tuning", Some(Path::new(path)), false),
        None => "tuning=defaults".to_owned(),
    }
}

/// `name=path sha256:...`, `name=none` or `name=off`.
fn file(name: &str, path: Option<&Path>, off: bool) -> String {
    match path {
        Some(path) => format!("{name}={} {}", path.display(), sha_prefix(path)),
        None if off => format!("{name}=off"),
        None => format!("{name}=none"),
    }
}

/// The mapdata folder: its path, how many `.mapdata` files, and a digest of their names and sizes.
fn mapdata(dir: Option<&Path>) -> String {
    let Some(dir) = dir else { return "mapdata=none".to_owned() };
    let mut files: Vec<(String, u64)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            name.ends_with(".mapdata").then(|| (name, entry.metadata().map_or(0, |m| m.len())))
        })
        .collect();
    files.sort();
    let mut listing = String::new();
    for (name, size) in &files {
        let _ = writeln!(listing, "{name} {size}");
    }
    format!("mapdata={} ({} files, names+sizes {})", dir.display(), files.len(), bytes_sha(listing.as_bytes()))
}

/// The line `data files: ...` of a match: the tuning part ([`tuning_file`] for a server), the routes table, the foliage and
/// the grass files, and the mapdata folder. The arguments are the variables and the folder the bots read them from
/// (`SRP_BOT_ROUTES`, `SRP_BOT_FOLIAGE`, `SRP_BOT_GRASS`).
pub fn data_files(
    tuning: &str,
    routes_var: Option<&OsStr>,
    foliage_var: Option<&OsStr>,
    grass_var: Option<&OsStr>,
    mapdata_dir: Option<&Path>,
) -> String {
    let routes = match routes::Source::choose(routes_var, mapdata_dir) {
        routes::Source::Set(path) | routes::Source::Default(path) => file("routes", Some(&path), false),
        routes::Source::Off => file("routes", None, true),
        routes::Source::Missing => file("routes", None, false),
    };
    let foliage = match foliage::Source::choose(foliage_var, mapdata_dir) {
        foliage::Source::Set(path) | foliage::Source::Default(path) => file("foliage", Some(&path), false),
        foliage::Source::None => file("foliage", None, false),
    };
    let grass = match grass::Source::choose_file(grass_var, mapdata_dir, grass::DEFAULT_FILE) {
        grass::Source::Set(path) | grass::Source::Default(path) => file("grass", Some(&path), false),
        grass::Source::None => file("grass", None, false),
    };
    format!("data files: {tuning}; {routes}; {foliage}; {grass}; {}", mapdata(mapdata_dir))
}

/// The line `env switches: ...`: the switches the lobby and the loadouts read once from the environment. Their reading
/// (`loadouts::new_kit`, `upgrades_on`) is the one the bots use, so this is what they run on.
pub fn env_switches() -> String {
    let onoff = |on: bool| if on { "on" } else { "off" };
    format!(
        "env switches: SRP_BOT_KIT={} SRP_BOT_UPGRADES={}",
        onoff(survarium::v026g::loadouts::new_kit()),
        onoff(survarium::v026g::loadouts::upgrades_on())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line names every file the bots read, with a hash for a file that is there and a word for one that is not.
    #[test]
    fn the_data_line_names_each_file_with_a_hash_or_a_word() {
        let dir = std::env::temp_dir().join(format!("srp-switch-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("level_01_day_2.mapdata"), b"map").unwrap();
        std::fs::write(dir.join("bot-routes.json"), b"{}").unwrap();
        let line = data_files(&tuning_file(None), None, None, None, Some(&dir));
        assert!(line.starts_with("data files: tuning=defaults; routes="), "{line}");
        assert!(line.contains("bot-routes.json sha256:"), "{line}");
        assert!(line.contains("foliage=none; grass=none;"), "{line}");
        assert!(line.contains("(1 files, names+sizes sha256:"), "{line}");
        let off = data_files("tuning=defaults", Some(OsStr::new("off")), Some(OsStr::new("off")), None, Some(&dir));
        assert!(off.contains("routes=off; foliage=none"), "{off}");
        let tuning = dir.join("tuning.json");
        std::fs::write(&tuning, b"{}").unwrap();
        let with = data_files(&tuning_file(Some(tuning.as_os_str())), None, None, None, None);
        assert!(with.contains("tuning=") && with.contains("tuning.json sha256:") && with.contains("mapdata=none"), "{with}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_says_why() {
        assert!(sha_prefix(Path::new("/no/such/file")).starts_with("unreadable ("));
    }
}
