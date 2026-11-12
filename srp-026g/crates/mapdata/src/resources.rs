//! Read access to the game's `resources.db` (only the dump step needs it;
//! the match server reads the files [`crate::format`] writes).

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use binary_config_parser::{ConfigError, ConfigValue};
use resources_db::Archive;
use resources_db::overlay::Overlay;

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
    /// The archives laid over one another as the game mounts them.
    overlay: Overlay<BufReader<File>>,
    /// Every file seen, by its [`normalize`]d path: its path in the archive.
    paths: HashMap<String, String>,
}

impl Resources {
    /// Opens the archive and indexes its files by path.
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::open_layers(&[path.to_path_buf()])
    }

    /// The archives mounted in order, as the game mounts `resources.db` and
    /// its `resources.db.patch_<n>`: a file of a later one wins, folders merge
    /// (`resources_db::overlay`).
    ///
    /// Many collision meshes are **hard links** in the index: a node that
    /// points at another node, a file or a whole folder (0.26g's archive has
    /// 58 links to folders, e.g. a model's `collision` folder that is another
    /// model's). The game reads the target's files under the link's own path;
    /// so does this reader (`resources_db`), so a linked folder's meshes and
    /// primitives are there like any other.
    pub fn open_layers(paths: &[PathBuf]) -> Result<Self, Error> {
        let mut archives = Vec::with_capacity(paths.len());
        let mut names = Vec::with_capacity(paths.len());
        for path in paths {
            let at = |error: &dyn fmt::Display| Error(format!("{}: {error}", path.display()));
            let file = File::open(path).map_err(|error| at(&error))?;
            archives.push(Archive::new(BufReader::new(file)).map_err(|error| at(&error))?);
            names.push(path.display().to_string());
        }
        let overlay = Overlay::new(archives, &names).map_err(|error| Error(error.to_string()))?;
        let paths = overlay
            .files_under("")
            .map(|(path, _, _)| (normalize(path), path.to_owned()))
            .collect();
        Ok(Self { overlay, paths })
    }

    /// Every file's path (normalized), in no order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.paths.keys().map(String::as_str)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.paths.contains_key(&normalize(path))
    }

    /// File bytes by engine path; a leading `resources/` is optional.
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        let stored = self
            .paths
            .get(&normalize(path))
            .ok_or_else(|| Error(format!("missing resource `{path}`")))?;
        self.overlay
            .read(stored)
            .map_err(|error| Error(format!("{path}: {error}")))?
            .ok_or_else(|| Error(format!("missing resource `{path}`")))
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
