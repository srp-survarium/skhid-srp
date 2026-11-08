//! The game's files srp takes its 0.26g data from: those of the archives
//! the players' game mounts, read once at start.
//!
//! The players' game mounts the stock `resources.db` and every
//! `resources.db.patch_<n>` beside it over it, n ascending, a later file
//! over an earlier one (client patch revision 14). `srp` reads the server's
//! game data (`SRP_GAME_DATA`): the files of exactly those archives it
//! needs, laid over one another as the game lays them, which
//! `srp-game-data build` makes ([`resources_db::server`]), each checked
//! against its archive's file table. It names what it was made of
//! ([`Provenance`]): `resources.db`, and each patch with its sha256 and the
//! URL the players' updater downloads it from. The web page hands that list
//! to the players' updater (`GET /patches`, [`Provenance::offered`]), so the
//! server offers exactly the patches it runs on: the dictionary, every
//! item's and map's config and the other `gameplay/` files are the players'
//! own, and the server cannot know an item, or a value of one, the players'
//! game does not. A missing or wrong file stops `srp` at start ([`init`]).
//!
//! The tables are derived from these files when first used, as the game
//! reads them: the dictionary ([`super::dictionary`]), each item's
//! parameters and the slots it covers ([`super::item_params`]), what it may
//! be modified with and the modifications ([`super::modifications`]), the
//! game modes and each map's respawn points and battery containers, read in
//! its project ([`super::maps`]), the names of the maps and modes in the
//! game's localizations ([`super::localization`]), drops, quests, skills,
//! awards and the test profiles. What the game's files do not say stays in
//! code: the bundles.
//!
//! Without [`init`] (tests, tools), the data is that of
//! `resources/game-026g/server-game-data.db`: 0.26g's `resources.db` with
//! the test patches 14 to 16 over it, as `srp-game-data build` makes it.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use binary_config_parser::tree::ConfigValue;
use resources_db::Archive;
pub use resources_db::server::Provenance;
use resources_db::server::{GAMEPLAY, LOCALIZATIONS, PROJECT_MARKER, PROJECTS};

/// The server's game data (`srp-game-data build`).
pub const GAME_DATA_ENV: &str = "SRP_GAME_DATA";

/// Where the test data is, for the log and the messages.
const TEST_ORIGIN: &str = "resources/game-026g/server-game-data.db";
/// The game data of tests and tools (`resources/game-026g/README.md`).
const TEST_ARCHIVE: &[u8] = include_bytes!("../../../../resources/game-026g/server-game-data.db");

/// How the data is made, for the messages.
const HOW_TO_MAKE: &str = "make it with srp-game-data build --dir <the game folder with \
                           resources.db and its patches> --patch-url <where the players \
                           download the patches> --out server-game-data.db \
                           (crates/resources-db/README.md)";

/// Why the game's files cannot be had.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameDataError(pub String);

impl fmt::Display for GameDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the game's files: {}", self.0)
    }
}

impl std::error::Error for GameDataError {}

fn error(message: impl Into<String>) -> GameDataError {
    GameDataError(message.into())
}

/// The game's files srp reads, and what they were made of.
#[derive(Debug)]
pub struct GameData {
    /// Where they were read, for the log.
    pub origin: String,
    /// The archives they were made of: `resources.db` and the patches the
    /// players' updater is told to fetch ([`Provenance::offered`]).
    pub provenance: Provenance,
    /// Every file under `gameplay/` and the localizations
    /// ([`LOCALIZATIONS`]), by its path in the archive.
    files: BTreeMap<String, Vec<u8>>,
    /// The projects the archive has (`projects/<project>/client_project`),
    /// each its `client_project` cut to its `server_objects` and the client
    /// objects a world saves ([`resources_db::server::objects_of`]).
    projects: BTreeMap<String, Vec<u8>>,
}

impl GameData {
    /// The server's game data in `archive`: every file checked against its
    /// table and the provenance read ([`resources_db::server::read`]), then
    /// the `gameplay/` files, the localizations and the projects taken.
    pub fn from_archive<R: Read + Seek>(
        archive: &mut Archive<R>,
        origin: String,
    ) -> Result<Self, GameDataError> {
        let data = resources_db::server::read(archive).map_err(|cause| error(cause.to_string()))?;
        let paths: Vec<String> = archive
            .files_under("")
            .map(|(path, _)| path)
            .filter(|path| path.starts_with(GAMEPLAY) || LOCALIZATIONS.contains(path))
            .map(str::to_owned)
            .collect();
        let mut files = BTreeMap::new();
        for path in paths {
            let bytes = read(archive, &path)?;
            files.insert(path, bytes);
        }
        let project_paths: Vec<(String, String)> = archive
            .files_under(PROJECTS)
            .filter_map(|(path, _)| {
                let project = path.strip_prefix(PROJECTS)?.strip_suffix(PROJECT_MARKER)?;
                Some((project.to_owned(), path.to_owned()))
            })
            .collect();
        let mut projects = BTreeMap::new();
        for (project, path) in project_paths {
            projects.insert(project, read(archive, &path)?);
        }
        Ok(Self {
            origin,
            provenance: data.provenance,
            files,
            projects,
        })
    }

    /// The data of tests and tools ([`TEST_ARCHIVE`]), with srp's own
    /// configs and keys made from the repository's tables
    /// ([`with_srp_tables`]), as a release's content patch has them.
    pub fn test_archive() -> Self {
        let mut archive =
            Archive::new(Cursor::new(TEST_ARCHIVE)).expect("the test archive is an archive");
        let mut data =
            Self::from_archive(&mut archive, TEST_ORIGIN.into()).expect("the test archive reads");
        with_srp_tables(&mut data.files);
        with_pocket_units(&mut data.files);
        with_blackout_maps(&mut data.files);
        with_faces(&mut data.files);
        data
    }

    /// The server's game data in the file `data`.
    pub fn open(data: &Path) -> Result<Self, GameDataError> {
        let at = |cause: &dyn fmt::Display| {
            error(format!(
                "{}: {cause}; {GAME_DATA_ENV} must name the server's game data ({HOW_TO_MAKE})",
                data.display()
            ))
        };
        let file = std::fs::File::open(data).map_err(|cause| at(&cause))?;
        let mut archive = Archive::new(file).map_err(|cause| at(&cause))?;
        let origin = data.display().to_string();
        Self::from_archive(&mut archive, origin).map_err(|cause| at(&cause.0))
    }

    /// The bytes of the file at `path`, if the archive has it.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// Whether the archive has the project `project` (a game mode's
    /// `match_making_params.map_name`, e.g. `level_01/day_2`).
    pub fn has_project(&self, project: &str) -> bool {
        self.projects.contains_key(project)
    }

    /// What the data has of the project `project`: its `client_project`
    /// cut to its `server_objects`, a binary config.
    pub fn project(&self, project: &str) -> Option<&[u8]> {
        self.projects.get(project).map(Vec::as_slice)
    }

    /// This data with `files` over it, as a patch lays its files: a file
    /// of `gameplay/` or a localization by its path, a project as
    /// `projects/<project>/client_project` (what the data keeps of one:
    /// [`resources_db::server::objects_of`]). Tests make up data so.
    #[cfg(any(test, feature = "fixtures"))]
    pub fn laid_over(&self, origin: &str, files: Vec<(String, Vec<u8>)>) -> Self {
        let mut data = Self {
            origin: origin.to_owned(),
            provenance: self.provenance.clone(),
            files: self.files.clone(),
            projects: self.projects.clone(),
        };
        for (path, bytes) in files {
            let project = path
                .strip_prefix(PROJECTS)
                .and_then(|path| path.strip_suffix(PROJECT_MARKER));
            match project {
                Some(project) => data.projects.insert(project.to_owned(), bytes),
                None => data.files.insert(path, bytes),
            };
        }
        data
    }

    /// How many files there are: of `gameplay/`, and the localizations.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// What it was made of, in one line for the log: `resources.db` and
    /// each patch by its key and the start of its sha256.
    pub fn made_of(&self) -> String {
        let db = &self.provenance.db;
        let mut line = format!("{} {}", db.name, &db.sha256[..12]);
        for (key, sha256, _) in self.provenance.offered() {
            line.push_str(&format!(" + {key} {}", &sha256[..12]));
        }
        line
    }
}

/// The file at `path` of `archive`, which lists it.
fn read<R: Read + Seek>(archive: &mut Archive<R>, path: &str) -> Result<Vec<u8>, GameDataError> {
    archive
        .read(path)
        .map_err(|cause| error(format!("{path}: {cause}")))
        .map(|bytes| bytes.expect("a listed file"))
}

/// srp's own configs, each made by a key/value table of
/// `scripts/content-edit` (`content_edit.py` STATS) into the content patch:
/// the test data's are the repository's.
const SRP_TABLES: &[(&str, &str)] = &[
    (
        super::lobby::rooms::RULES_PATH,
        include_str!("../../../../scripts/content-edit/rooms.tsv"),
    ),
    (
        super::new_account::PATH,
        include_str!("../../../../scripts/content-edit/new-account.tsv"),
    ),
];

/// The dictionary's entries as `scripts/content-edit/items.tsv` writes them
/// into the content patch: their price and srp's keys, `srp_like` and
/// `srp_start`.
const ITEMS_TABLE: &str = include_str!("../../../../scripts/content-edit/items.tsv");

/// The grenades' and the trap's stats; srp reads one of its columns, the
/// `pocket_units` the content patch adds to their configs.
const THROWABLES_TABLE: &str = include_str!("../../../../scripts/content-edit/throwables.tsv");

/// srp's shop rules, which name items ([`super::prices::SHOP_PATH`]).
const SHOP_TABLE: &str = include_str!("../../../../scripts/content-edit/shop.tsv");

/// Blackout's maps as the content patch makes them of the live configs
/// (`docs/notes/blackout.md`): the test data's are this edit list laid over
/// its own battery configs.
const BLACKOUT_EDITS: &str = include_str!("../../../../scripts/content-edit/blackout-mode-1.json");

/// `files` with [`BLACKOUT_EDITS`]: for each `maps_dict` entry it appends
/// whose battery config `files` has, a copy of that config at the entry's
/// name, with the list's strings replaced and its values added, and the
/// entry.
fn with_blackout_maps(files: &mut BTreeMap<String, Vec<u8>>) {
    fn text(value: &serde_json::Value) -> String {
        value.as_str().expect("a text").to_owned()
    }
    fn list<'a>(edits: &'a serde_json::Value, edit: &str) -> &'a [serde_json::Value] {
        edits[edit].as_array().map_or(&[], Vec::as_slice)
    }
    fn replace(value: &mut ConfigValue, replaced: &[(String, String)]) {
        match value {
            ConfigValue::String(text) => {
                if let Some((_, to)) = replaced.iter().find(|(from, _)| from == text) {
                    *text = to.clone();
                }
            }
            ConfigValue::Named(entries) => {
                for (_, value) in entries {
                    replace(value, replaced);
                }
            }
            ConfigValue::Indexed(items) => {
                for value in items {
                    replace(value, replaced);
                }
            }
            _ => {}
        }
    }
    let edits: serde_json::Value =
        serde_json::from_str(BLACKOUT_EDITS).expect("the edit list reads");
    let replaced: Vec<(String, String)> = list(&edits, "replace_string")
        .iter()
        .map(|each| (text(&each["from"]), text(&each["to"])))
        .collect();
    let added: Vec<(String, String, String)> = list(&edits, "add_value")
        .iter()
        .map(|each| (text(&each["table"]), text(&each["key"]), text(&each["value"])))
        .collect();
    let path = super::dictionary::PATH;
    let mut dictionary = ConfigValue::parse(&files[path]).expect("the dictionary parses");
    let ConfigValue::Named(sections) = &mut dictionary else {
        panic!("the dictionary is a named table")
    };
    let Some((_, ConfigValue::Indexed(maps))) =
        sections.iter_mut().find(|(name, _)| name == "maps_dict")
    else {
        panic!("the dictionary has maps_dict")
    };
    for entry in list(&edits, "append_entry") {
        let name = text(&entry["value"]["cfg_mode_name"]);
        let base = name.trim_start_matches("blackout/");
        let Some(bytes) = files.get(&format!("{}{base}", super::maps::GAME_MODES_DIR)) else {
            continue;
        };
        let mut config = ConfigValue::parse(bytes).expect("a battery config");
        replace(&mut config, &replaced);
        let ConfigValue::Named(tables) = &mut config else {
            panic!("{base} is a named table")
        };
        for (table, key, value) in &added {
            let Some((_, ConfigValue::Named(entries))) =
                tables.iter_mut().find(|(name, _)| name == table)
            else {
                panic!("{base} has no table {table}")
            };
            entries.push((key.clone(), ConfigValue::String(value.clone())));
        }
        let config = config.to_bytes().expect("a config");
        files.insert(format!("{}{name}", super::maps::GAME_MODES_DIR), config);
        let id = entry["value"]["id"].as_i64().expect("an id");
        maps.push(ConfigValue::Named(vec![
            ("cfg_mode_name".into(), ConfigValue::String(name)),
            ("id".into(), ConfigValue::Integer(id as i32)),
        ]));
    }
    files.insert(path.to_owned(), dictionary.to_bytes().expect("a config"));
}

/// The faces the test data has: [`TEST_FACES`] entries of `body_parts.heads`,
/// each today's `body_parts.head` (until the faces pack ships, its form:
/// `face-choice.md`), so tests choose and send faces other than 0.
fn with_faces(files: &mut BTreeMap<String, Vec<u8>>) {
    let path = super::faces::PATH;
    let mut config = ConfigValue::parse(&files[path]).expect("the body parts parse");
    let ConfigValue::Named(sections) = &mut config else {
        panic!("{path} is a named table")
    };
    let Some((_, ConfigValue::Named(parts))) =
        sections.iter_mut().find(|(name, _)| name == "body_parts")
    else {
        panic!("{path} has body_parts")
    };
    let head = parts
        .iter()
        .find(|(name, _)| name == "head")
        .map(|(_, head)| head.clone())
        .expect("today's head");
    parts.push((
        "heads".into(),
        ConfigValue::Indexed(vec![head; TEST_FACES]),
    ));
    files.insert(path.to_owned(), config.to_bytes().expect("a config"));
}

/// How many faces [`with_faces`] gives the test data.
pub const TEST_FACES: usize = 3;

/// `files` with `parameters.pocket_units` where [`THROWABLES_TABLE`] has
/// one: a float in the config of the row's name (`<name>.options` under
/// `gameplay/items/`), as `content_edit.py` adds a key of an `add` column.
fn with_pocket_units(files: &mut BTreeMap<String, Vec<u8>>) {
    let mut lines = THROWABLES_TABLE
        .lines()
        .filter(|line| !line.trim().is_empty());
    let head: Vec<&str> = lines.next().expect("a header").split('\t').collect();
    let column = head
        .iter()
        .position(|c| *c == "pocket_units")
        .expect("pocket_units");
    for line in lines {
        let cells: Vec<&str> = line.split('\t').collect();
        let Some(units) = cells.get(column).filter(|cell| !cell.is_empty()) else {
            continue;
        };
        let file = format!("/{}.options", cells[0]);
        let path = files
            .keys()
            .find(|path| path.starts_with("gameplay/items/") && path.ends_with(&file))
            .expect("the row's config")
            .clone();
        let mut config = ConfigValue::parse(&files[&path]).expect("the config parses");
        let ConfigValue::Named(sections) = &mut config else {
            panic!("{path} is a named table")
        };
        let Some((_, ConfigValue::Named(parameters))) =
            sections.iter_mut().find(|(name, _)| name == "parameters")
        else {
            panic!("{path} has parameters")
        };
        let value = ConfigValue::Float(units.parse().expect("a number"));
        parameters.push(("pocket_units".into(), value));
        files.insert(path, config.to_bytes().expect("a config"));
    }
}

/// `files` with srp's own configs ([`SRP_TABLES`], [`SHOP_TABLE`]) and the
/// dictionary's entries as [`ITEMS_TABLE`] has them, for the items the
/// dictionary has.
fn with_srp_tables(files: &mut BTreeMap<String, Vec<u8>>) {
    for (path, table) in SRP_TABLES {
        files.insert((*path).to_owned(), key_value_config(table));
    }
    let mut lines = ITEMS_TABLE
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
    let head: Vec<&str> = lines.next().expect("a header").split('\t').collect();
    let column = |name| head.iter().position(|c| *c == name).expect(name);
    let (cost, like, start) = (column("base_cost"), column("like"), column("start"));
    let rows: BTreeMap<i32, Vec<&str>> = lines
        .map(|line| {
            let cells: Vec<&str> = line.split('\t').collect();
            (cells[0].parse().expect("a dict id"), cells)
        })
        .collect();
    let path = super::dictionary::PATH;
    let mut dictionary = ConfigValue::parse(&files[path]).expect("the dictionary parses");
    let ConfigValue::Named(sections) = &mut dictionary else {
        panic!("the dictionary is a named table")
    };
    let Some((_, ConfigValue::Indexed(entries))) =
        sections.iter_mut().find(|(name, _)| name == "items_dict")
    else {
        panic!("the dictionary has items_dict")
    };
    let mut names = Vec::new();
    for entry in entries {
        let id = entry.get("dict_id").and_then(ConfigValue::as_i32);
        names.extend(
            entry
                .get("name")
                .and_then(ConfigValue::as_str)
                .map(str::to_owned),
        );
        let (Some(row), ConfigValue::Named(keys)) = (id.and_then(|id| rows.get(&id)), entry) else {
            continue;
        };
        for (_, value) in keys.iter_mut().filter(|(key, _)| key == "base_cost") {
            *value = ConfigValue::Integer(row[cost].parse().expect("a price"));
        }
        if !row[like].is_empty() {
            keys.push(("srp_like".into(), ConfigValue::String(row[like].into())));
        }
        if !row[start].is_empty() {
            let ids = row[start].split(' ').map(|id| id.parse().expect("an id"));
            let ids = ids.map(ConfigValue::Integer).collect();
            keys.push(("srp_start".into(), ConfigValue::Indexed(ids)));
        }
    }
    files.insert(path.to_owned(), dictionary.to_bytes().expect("a config"));
    let shop: String = SHOP_TABLE
        .lines()
        .filter(|line| {
            line.split_once('/')
                .is_none_or(|(item, _)| names.iter().any(|n| n == item))
        })
        .map(|line| format!("{line}\n"))
        .collect();
    files.insert(super::prices::SHOP_PATH.to_owned(), key_value_config(&shop));
}

/// A key/value table's config, as `content_edit.py`'s `new_config` makes it:
/// in a key, `/` goes into a named table and `[i]` to an item of an indexed
/// one; a cell is a whole number, a number with a point, true/false or a text.
pub(super) fn key_value_config(table: &str) -> Vec<u8> {
    enum Node {
        Leaf(ConfigValue),
        Table(Vec<(String, Node)>),
    }
    fn into_value(node: Node) -> ConfigValue {
        match node {
            Node::Leaf(value) => value,
            Node::Table(entries) if entries.iter().all(|(step, _)| step.starts_with('[')) => {
                ConfigValue::Indexed(
                    entries
                        .into_iter()
                        .map(|(_, node)| into_value(node))
                        .collect(),
                )
            }
            Node::Table(entries) => ConfigValue::Named(
                entries
                    .into_iter()
                    .map(|(step, node)| (step, into_value(node)))
                    .collect(),
            ),
        }
    }
    let mut root = Vec::new();
    let rows = table
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
    for row in rows.skip(1) {
        let (key, cell) = row
            .split_once('\t')
            .expect("a key/value table's row: key, value");
        let value = match cell {
            "true" | "false" => ConfigValue::Boolean(cell == "true"),
            _ => cell.parse().map(ConfigValue::Integer).unwrap_or_else(|_| {
                cell.parse()
                    .map_or_else(|_| ConfigValue::String(cell.to_owned()), ConfigValue::Float)
            }),
        };
        // `pvb/sizes[0]`: pvb, sizes, [0]
        let steps: Vec<String> = key
            .split('/')
            .flat_map(|part| {
                let (name, items) = part.split_once('[').unwrap_or((part, ""));
                let items = items
                    .split('[')
                    .filter(|i| !i.is_empty())
                    .map(|i| format!("[{i}"));
                std::iter::once(name.to_owned())
                    .filter(|n| !n.is_empty())
                    .chain(items)
            })
            .collect();
        let (last, path) = steps.split_last().expect("a key");
        let mut table = &mut root;
        for step in path {
            let at = match table.iter().position(|(name, _)| name == step) {
                Some(at) => at,
                None => {
                    table.push((step.clone(), Node::Table(Vec::new())));
                    table.len() - 1
                }
            };
            let Node::Table(inner) = &mut table[at].1 else {
                panic!("{key}: under a value")
            };
            table = inner;
        }
        table.push((last.clone(), Node::Leaf(value)));
    }
    into_value(Node::Table(root)).to_bytes().expect("a config")
}

static SHARED: OnceLock<GameData> = OnceLock::new();

/// The game's files of this process: those [`init`] read, else those of
/// the test archive.
pub fn shared() -> &'static GameData {
    SHARED.get_or_init(GameData::test_archive)
}

/// The file at `path` of [`shared`]. Panics without it: [`init`] checks
/// every file the tables read first.
pub fn file(path: &str) -> &'static [u8] {
    shared()
        .file(path)
        .unwrap_or_else(|| panic!("{}: no {path}", shared().origin))
}

/// The game's files `srp` runs with: the file `SRP_GAME_DATA` names.
pub fn from_env() -> Result<GameData, GameDataError> {
    let data = std::env::var_os(GAME_DATA_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            error(format!(
                "{GAME_DATA_ENV} is unset: it names the server's game data, the files srp reads \
                 of resources.db and the patches the players' game mounts \
                 (server-game-data.db, in the download folder); {HOW_TO_MAKE}"
            ))
        })?;
    GameData::open(&data)
}

/// Reads the game's files ([`from_env`]), makes them this process's, and
/// derives every table from them and checks it ([`super::tables::check`]),
/// and that the golden weapons and the premium armour have their patches
/// ([`super::golden::check`]).
/// `srp` calls it before anything else: whatever is wrong stops it here.
/// The files are the process's before the check, so that a table one
/// parser takes from another ([`super::item_params::all`] and the like) is
/// built from them and no other.
pub fn init() -> Result<&'static GameData, GameDataError> {
    init_with(from_env()?)
}

/// [`init`] with the server's game data in the file `data`, not
/// `SRP_GAME_DATA`: what `srp-check-game-data` runs on data a release
/// builds, so that data `srp` would refuse is refused before it ships.
pub fn init_from(data: &Path) -> Result<&'static GameData, GameDataError> {
    init_with(GameData::open(data)?)
}

fn init_with(data: GameData) -> Result<&'static GameData, GameDataError> {
    if SHARED.set(data).is_err() {
        return Err(error(
            "a table was used before the game's files were read; srp reads them first",
        ));
    }
    let data = shared();
    super::tables::check(data)?;
    super::new_account::check_data(data)?;
    super::golden::check(data)?;
    super::keeper::check(data)?;
    log::info!(
        "game data: {} ({} gameplay and text files, {} projects; made of {}; {} patch(es) offered at \
         /patches)",
        data.origin,
        data.len(),
        data.projects.len(),
        data.made_of(),
        data.provenance.patches.len()
    );
    // The scope pairs and why any variant was left out, in the log at start.
    super::scopes::ScopeFits::shared();
    Ok(data)
}

/// A config of the game's files, parsed.
pub fn config(data: &GameData, path: &str) -> Result<Option<ConfigValue>, GameDataError> {
    data.file(path)
        .map(|bytes| {
            ConfigValue::parse(bytes).map_err(|cause| error(format!("{path}: {}", cause.0)))
        })
        .transpose()
}

#[cfg(test)]
mod tests;
