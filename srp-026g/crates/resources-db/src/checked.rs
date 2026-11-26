// SPDX-License-Identifier: GPL-3.0-or-later
//! The files a match asks the players' games to hash ([`CHECKED`]), as the
//! updater checks its patch files: a game whose archive is not the
//! release's (a damaged or an older `resources.db` or patch, a
//! `resources.db.patch_<n>` of no release) shows in the match server's log.
//!
//! A match checks files of its map's project: its `client_project`, and
//! what its `vegetation` client objects' `layers` name
//! (`survarium::object_vegetation::load` 0x294D90, `grass_cook` 0x12A020):
//! each layer's `filename`, a `.veg` file under `projects/<map>/`, and its
//! `model_def` names, models whose `settings` name their materials
//! (`material_settings/<surface>/material_name`), whose textures
//! (`<stage>/effect/texture_<what>/value`) are `textures/<name>.dds` with
//! their `.options`. Every match checks [`ALWAYS`] too (`startup.cfg`).
//!
//! The data names each file once with its size and sha256 ([`Sum`]), and
//! each project's paths ([`Checked::paths_of`]), as text ([`Checked::text`],
//! [`FORMAT`]).

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek};

use binary_config_parser::tree::ConfigValue;
use sha2::{Digest, Sha256};

use crate::overlay::Overlay;
use crate::{Error, crc32};

/// Where the server's game data keeps the checked files.
pub const CHECKED: &str = "srp/checked";
/// The first line of [`CHECKED`].
pub const FORMAT: &str = "skhid checked files 1";
/// Checked in every match, whatever its project.
pub const ALWAYS: [&str; 1] = ["startup.cfg"];

const PROJECTS: &str = "projects/";
const CLIENT_PROJECT: &str = "client_project";

/// A file's size and the sha256 of its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sum {
    pub size: u32,
    pub sha256: [u8; 32],
}

impl Sum {
    pub fn of(bytes: &[u8]) -> Result<Self, String> {
        let size = u32::try_from(bytes.len()).map_err(|_| "over 4 GiB".to_owned())?;
        Ok(Self {
            size,
            sha256: Sha256::digest(bytes).into(),
        })
    }
}

/// The files of an archive, as the game mounts them.
pub trait Files {
    fn has(&self, path: &str) -> bool;
    /// Every file whose path starts with `prefix`, in path order.
    fn under(&self, prefix: &str) -> Vec<String>;
    /// The bytes at `path`, checked against the archive's table.
    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, Error>;
}

impl<R: Read + Seek> Files for Overlay<R> {
    fn has(&self, path: &str) -> bool {
        self.entry(path).is_some()
    }

    fn under(&self, prefix: &str) -> Vec<String> {
        self.files_under(prefix)
            .map(|(path, _, _)| path.to_owned())
            .collect()
    }

    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let Some((_, entry)) = self.entry(path) else {
            return Ok(None);
        };
        let entry = *entry;
        let bytes = Overlay::read(self, path)?;
        if let Some(bytes) = &bytes
            && (bytes.len() != entry.size as usize || crc32(bytes) != entry.hash)
        {
            return Err(Error::Content(format!(
                "{path}: its bytes are not those its table says (size {}, CRC-32 {:08x})",
                entry.size, entry.hash
            )));
        }
        Ok(bytes)
    }
}

/// What the server's game data says a match checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checked {
    /// Each checked file once, by its path.
    pub files: BTreeMap<String, Sum>,
    /// By project (`level_01/day_2`), the paths a match of it checks
    /// besides [`Checked::always`].
    pub projects: BTreeMap<String, BTreeSet<String>>,
    /// The paths every match checks ([`ALWAYS`] as the archive has them).
    pub always: BTreeSet<String>,
}

impl Checked {
    /// The checked files of every project in `files`.
    pub fn of(files: &mut impl Files) -> Result<Self, Error> {
        let mut checked = Self::default();
        let projects: Vec<String> = files
            .under(PROJECTS)
            .into_iter()
            .filter_map(|path| {
                let project = path.strip_prefix(PROJECTS)?.strip_suffix(CLIENT_PROJECT)?;
                Some(project.strip_suffix('/')?.to_owned())
            })
            .collect();
        let mut closure = Closure::default();
        for project in projects {
            let paths = closure.of_project(files, &project)?;
            checked.projects.insert(project, paths);
        }
        checked.always = ALWAYS
            .into_iter()
            .filter(|path| files.has(path))
            .map(str::to_owned)
            .collect();
        let every: BTreeSet<String> = checked
            .projects
            .values()
            .flatten()
            .chain(&checked.always)
            .cloned()
            .collect();
        for path in every {
            let bytes = files
                .read(&path)?
                .ok_or_else(|| Error::Content(format!("{path}: listed and not read")))?;
            let sum = Sum::of(&bytes).map_err(|why| Error::Content(format!("{path}: {why}")))?;
            checked.files.insert(path, sum);
        }
        Ok(checked)
    }

    /// The paths a match of `project` checks, [`Checked::always`] first; an
    /// unknown project checks those alone.
    pub fn paths_of<'a>(&'a self, project: &str) -> impl Iterator<Item = &'a str> + 'a {
        let own = self.projects.get(project).into_iter().flatten();
        self.always
            .iter()
            .chain(own.filter(|path| !self.always.contains(*path)))
            .map(String::as_str)
    }

    /// One line a file, then one a project's path, then one an always
    /// checked path; fields apart by a tab each (a path has none), `→` here:
    ///
    /// ```text
    /// skhid checked files 1
    /// file→<path>→<size>→<sha256>
    /// project→<project>→<path>
    /// always→<path>
    /// ```
    pub fn text(&self) -> String {
        let mut text = format!("{FORMAT}\n");
        for (path, sum) in &self.files {
            text += &format!("file\t{path}\t{}\t{}\n", sum.size, hex(&sum.sha256));
        }
        for (project, paths) in &self.projects {
            for path in paths {
                text += &format!("project\t{project}\t{path}\n");
            }
        }
        for path in &self.always {
            text += &format!("always\t{path}\n");
        }
        text
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let bad = |line: &str, why: &str| Error::Content(format!("{CHECKED}: {line:?}: {why}"));
        let mut lines = text.lines();
        if lines.next() != Some(FORMAT) {
            return Err(Error::Content(format!("{CHECKED}: not {FORMAT:?}")));
        }
        let mut checked = Self::default();
        for line in lines {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields[..] {
                ["file", path, size, sha256] => {
                    let size = size.parse().map_err(|_| bad(line, "not a size"))?;
                    let sha256 = unhex(sha256).ok_or_else(|| bad(line, "not a sha256"))?;
                    if checked.files.insert(path.to_owned(), Sum { size, sha256 }).is_some() {
                        return Err(bad(line, "a second line of the file"));
                    }
                }
                ["project", project, path] => {
                    if !checked.files.contains_key(path) {
                        return Err(bad(line, "a path of no file line"));
                    }
                    checked
                        .projects
                        .entry(project.to_owned())
                        .or_default()
                        .insert(path.to_owned());
                }
                ["always", path] => {
                    if !checked.files.contains_key(path) {
                        return Err(bad(line, "a path of no file line"));
                    }
                    checked.always.insert(path.to_owned());
                }
                _ => return Err(bad(line, "not a line of the format")),
            }
        }
        Ok(checked)
    }
}

/// The checked paths of projects, each model and material followed once.
#[derive(Default)]
struct Closure {
    models: BTreeMap<String, BTreeSet<String>>,
    materials: BTreeMap<String, BTreeSet<String>>,
}

impl Closure {
    /// `project`'s `client_project` and what its vegetation names: each layer's
    /// file and its models'. A file a layer names that the archive lacks is
    /// left out: the game finds nothing there either.
    fn of_project(&mut self, files: &mut impl Files, project: &str) -> Result<BTreeSet<String>, Error> {
        let path = format!("{PROJECTS}{project}/{CLIENT_PROJECT}");
        let bytes = files
            .read(&path)?
            .ok_or_else(|| Error::Content(format!("{path}: listed and not read")))?;
        let config = ConfigValue::parse(&bytes)
            .map_err(|cause| Error::Content(format!("{path}: {cause}")))?;
        let map = project.split('/').next().unwrap_or(project);
        let mut paths = BTreeSet::from([path]);
        let objects = config.get("client_objects").map(ConfigValue::children).unwrap_or_default();
        let vegetation = objects.into_iter().filter(|object| {
            object.get("game_object_type").and_then(ConfigValue::as_str) == Some("vegetation")
        });
        for object in vegetation {
            for layer in object.get("layers").map(ConfigValue::children).unwrap_or_default() {
                if let Some(name) = layer.get("filename").and_then(ConfigValue::as_str)
                    && !name.is_empty()
                {
                    let veg = format!("{PROJECTS}{map}/{name}");
                    if files.has(&veg) {
                        paths.insert(veg);
                    }
                }
                let models = layer.get("model_def").map(ConfigValue::children).unwrap_or_default();
                for model in models {
                    if let Some(name) = model.get("name").and_then(ConfigValue::as_str)
                        && !name.is_empty()
                    {
                        paths.extend(self.of_model(files, name)?);
                    }
                }
            }
        }
        Ok(paths)
    }

    /// Every file of `models/<name>.model/`, and its materials'.
    fn of_model(&mut self, files: &mut impl Files, name: &str) -> Result<BTreeSet<String>, Error> {
        if let Some(paths) = self.models.get(name) {
            return Ok(paths.clone());
        }
        let folder = format!("models/{name}.model/");
        let mut paths: BTreeSet<String> = files.under(&folder).into_iter().collect();
        let settings = format!("{folder}settings");
        // A settings file that is no config is checked, its materials not.
        let materials: Vec<String> = match files.read(&settings)? {
            Some(bytes) => ConfigValue::parse(&bytes)
                .map(|config| {
                    config
                        .get("material_settings")
                        .map(ConfigValue::children)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|surface| surface.get("material_name")?.as_str())
                        .filter(|material| !material.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            None => Vec::new(),
        };
        for material in materials {
            paths.extend(self.of_material(files, &material)?);
        }
        self.models.insert(name.to_owned(), paths.clone());
        Ok(paths)
    }

    /// `material_instances/<name>.material` and its textures, each its
    /// `.dds` and `.options` that the archive has.
    fn of_material(&mut self, files: &mut impl Files, name: &str) -> Result<BTreeSet<String>, Error> {
        if let Some(paths) = self.materials.get(name) {
            return Ok(paths.clone());
        }
        let path = format!("material_instances/{name}.material");
        let mut paths = BTreeSet::new();
        if let Some(bytes) = files.read(&path)? {
            paths.insert(path);
            let mut textures = BTreeSet::new();
            // A material that is no config is checked, its textures not.
            if let Ok(config) = ConfigValue::parse(&bytes) {
                textures_in(&config, &mut textures);
            }
            for texture in textures {
                for extension in [".dds", ".options"] {
                    let file = format!("textures/{texture}{extension}");
                    if files.has(&file) {
                        paths.insert(file);
                    }
                }
            }
        }
        self.materials.insert(name.to_owned(), paths.clone());
        Ok(paths)
    }
}

/// The names of every `texture_<what>` parameter's `value` under `config`.
fn textures_in(config: &ConfigValue, textures: &mut BTreeSet<String>) {
    for (name, value) in config.entries() {
        if name.starts_with("texture")
            && let Some(texture) = value.get("value").and_then(ConfigValue::as_str)
            && !texture.is_empty()
        {
            textures.insert(texture.to_owned());
        }
        textures_in(value, textures);
    }
    if let ConfigValue::Indexed(values) = config {
        for value in values {
            textures_in(value, textures);
        }
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut out = [0; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests;
