//! Read access to the game's `resources.db` (only the dump step needs it;
//! the match server reads the files [`crate::format`] writes).

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use binary_config_parser::{ConfigError, ConfigValue};
use vostok_resources_db::{Archive, FileEntry, NodeKind};

/// The variable the tests and `mapdata-dump` read the archive's path from.
pub const PATH_VARIABLE: &str = "SRP_RESOURCES_DB";

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<ConfigError> for Error {
    fn from(error: ConfigError) -> Self {
        Self(error.to_string())
    }
}

pub struct Resources {
    archives: Vec<Archive>,
    /// By path: the archive that gives it (the last layer that has it) and
    /// its entry there.
    files: HashMap<String, (usize, FileEntry)>,
    /// Index entries that are hard links whose target is not a file (a link
    /// to a link); none in the shipped 0.26g archive.
    pub unresolved_links: usize,
}

impl Resources {
    /// Opens the archive and indexes its files by path.
    ///
    /// Many collision meshes (walker meshes above all) are **hard links** in
    /// the index: a node that points at another node's payload. The
    /// `resources-db` listing resolves them to the target's payload under the
    /// link's own path, so they are kept here as files like any other; a
    /// loader that took only plain file nodes would lose those meshes.
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::open_layers(&[path.to_path_buf()])
    }

    /// The archives mounted in order, as the game mounts `resources.db` and
    /// its `resources.db.patch_<n>`: a file of a later one wins.
    pub fn open_layers(paths: &[PathBuf]) -> Result<Self, Error> {
        let mut archives = Vec::with_capacity(paths.len());
        let mut files = HashMap::new();
        let mut unresolved_links = 0;
        for (layer, path) in paths.iter().enumerate() {
            let archive = Archive::open(path)
                .map_err(|error| Error(format!("{}: {error}", path.display())))?;
            // The unpacker indexes slices directly; a damaged file must not
            // take the caller down, so treat a panic while listing as a load
            // error.
            let entries = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| archive.list()))
                .map_err(|_| Error(format!("{}: unreadable file index", path.display())))?;
            for entry in entries {
                match entry.kind {
                    NodeKind::File { .. } => {
                        files.insert(
                            normalize(&String::from_utf8_lossy(&entry.path)),
                            (layer, entry),
                        );
                    }
                    NodeKind::HardLink => unresolved_links += 1,
                    _ => {}
                }
            }
            archives.push(archive);
        }
        Ok(Self {
            archives,
            files,
            unresolved_links,
        })
    }

    /// Every file's path (normalized), in no order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.files.contains_key(&normalize(path))
    }

    /// File bytes by engine path; a leading `resources/` is optional.
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        let (layer, entry) = self
            .files
            .get(&normalize(path))
            .ok_or_else(|| Error(format!("missing resource `{path}`")))?;
        self.archives[*layer]
            .read_file(entry)
            .map_err(|error| Error(format!("{path}: {error}")))
    }

    pub fn config(&mut self, path: &str) -> Result<ConfigValue, Error> {
        let bytes = self.read(path)?;
        ConfigValue::parse(&bytes).map_err(|error| Error(format!("{path}: {error}")))
    }
}

/// Engine paths as the archive stores them: no `resources/` prefix, forward
/// slashes, lower case.
pub fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    path.strip_prefix("resources/").unwrap_or(&path).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_engine_paths() {
        assert_eq!(
            normalize("resources/models\\Level/01/terrain.model/walker_collision"),
            "models/level/01/terrain.model/walker_collision"
        );
    }
}
