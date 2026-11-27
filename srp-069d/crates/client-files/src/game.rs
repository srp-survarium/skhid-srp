//! The game folder's volumes as one tree.
//!
//! The client mounts `resources.00.db`, `resources.01.db`, ... in number order and a later volume replaces a path of an
//! earlier one, so the copy that counts is the one in the highest-numbered volume that stores the path. An entry a
//! volume marks erased is skipped (the copy of an older volume stays readable).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::volume::{self, Kind, Volume};
use crate::{Error, Result};

/// One stored copy of a path.
#[derive(Debug, Clone, Copy)]
struct Copy {
    vol: u32,
    kind: Kind,
}

#[derive(Debug)]
pub struct GameFiles {
    dir: PathBuf,
    /// volume number -> its file
    volumes: BTreeMap<u32, PathBuf>,
    /// path -> its stored copies, oldest volume first
    copies: HashMap<String, Vec<Copy>>,
}

/// The volume number of a file name `resources.NN.db`.
pub fn volume_number(name: &str) -> Option<u32> {
    let n = name.strip_prefix("resources.")?.strip_suffix(".db")?;
    if n.is_empty() || !n.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

impl GameFiles {
    /// Open every `resources.NN.db` of `dir` (the folder that holds survarium's volumes, next to `binaries`).
    pub fn open(dir: &Path) -> Result<GameFiles> {
        let rd = std::fs::read_dir(dir).map_err(|source| Error::Io { path: dir.display().to_string(), source })?;
        let mut volumes = BTreeMap::new();
        for e in rd.flatten() {
            if let Some(n) = e.file_name().to_str().and_then(volume_number) {
                volumes.insert(n, e.path());
            }
        }
        if volumes.is_empty() {
            return Err(Error::format(dir.display().to_string(), "no resources.NN.db volumes in this folder"));
        }
        let mut copies: HashMap<String, Vec<Copy>> = HashMap::new();
        for (&n, p) in &volumes {
            let v = Volume::open(p)?;
            for e in v.entries {
                if matches!(e.kind, Kind::File { .. } | Kind::Packed) {
                    copies.entry(e.path).or_default().push(Copy { vol: n, kind: e.kind });
                }
            }
        }
        Ok(GameFiles { dir: dir.to_path_buf(), volumes, copies })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Volume numbers found, ascending.
    pub fn volume_numbers(&self) -> Vec<u32> {
        self.volumes.keys().copied().collect()
    }

    /// Does any volume store `path`?
    pub fn contains(&self, path: &str) -> bool {
        self.copies.contains_key(path)
    }

    /// Volumes that store `path`, ascending.
    pub fn volumes_of(&self, path: &str) -> Vec<u32> {
        self.copies.get(path).map(|c| c.iter().map(|c| c.vol).collect()).unwrap_or_default()
    }

    /// The newest volume that stores `path`.
    pub fn newest_volume(&self, path: &str) -> Option<u32> {
        self.copies.get(path).and_then(|c| c.last()).map(|c| c.vol)
    }

    /// Every stored path, sorted.
    pub fn paths(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.copies.keys().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    /// The bytes of the newest copy of `path`.
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let c = self.copies.get(path).and_then(|c| c.last()).ok_or_else(|| Error::Missing(path.to_string()))?;
        volume::read_at(&self.volumes[&c.vol], path, c.kind)
    }

    /// The bytes of `path` as volume `vol` stores it.
    pub fn read_in(&self, vol: u32, path: &str) -> Result<Vec<u8>> {
        let c = self
            .copies
            .get(path)
            .and_then(|c| c.iter().find(|c| c.vol == vol))
            .ok_or_else(|| Error::Missing(format!("{path} (volume {vol:02})")))?;
        volume::read_at(&self.volumes[&c.vol], path, c.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_volume_wins_and_erased_entries_are_skipped() {
        let dir = std::env::temp_dir().join(format!("client-files-game-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("resources.00.db"), volume::build(&[("g/a.options", b"old"), ("g/b", b"b0")], &[])).unwrap();
        std::fs::write(dir.join("resources.02.db"), volume::build(&[("g/a.options", b"new")], &["g/b"])).unwrap();
        std::fs::write(dir.join("resources.02.db.orig"), b"not a volume").unwrap();
        let g = GameFiles::open(&dir).unwrap();
        assert_eq!(g.volume_numbers(), [0, 2]);
        assert_eq!(g.read("g/a.options").unwrap(), b"new");
        assert_eq!(g.read_in(0, "g/a.options").unwrap(), b"old");
        assert_eq!(g.read("g/b").unwrap(), b"b0");
        assert_eq!(g.volumes_of("g/a.options"), [0, 2]);
        assert!(matches!(g.read("g/none"), Err(Error::Missing(_))));
        assert_eq!(g.paths(), ["g/a.options", "g/b"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn volume_numbers_need_digits() {
        assert_eq!(volume_number("resources.07.db"), Some(7));
        assert_eq!(volume_number("resources.75.db"), Some(75));
        assert_eq!(volume_number("resources.75.db.orig"), None);
        assert_eq!(volume_number("resources..db"), None);
        assert_eq!(volume_number("resources.x1.db"), None);
    }
}
