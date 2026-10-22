//! Game data made up for tests: the test data with a map laid over it as
//! a patch adds one ([`with_map`]): a row in `maps_dict` of a new
//! dictionary, its game mode config, its project. Nothing of the map is
//! known to the code beforehand, so a table derived from it shows what the
//! server makes of a map it has never seen.

use binary_config_parser::tree::ConfigValue;

use super::super::dictionary::{self, Dictionary};
use super::super::game_data::{self, GameData};
use super::{GAME_MODES_DIR, MapMode, mode};

/// A map a patch adds.
#[derive(Clone, Debug, PartialEq)]
pub struct AddedMap {
    /// Its `maps_dict` id.
    pub id: u8,
    /// `cfg_mode_name`.
    pub config: &'static str,
    /// `match_making_params.map_id`, `map_weather_id`, `map_name`.
    pub level: u8,
    pub weather: u8,
    pub project: &'static str,
    /// `mode.mode`.
    pub mode: u8,
    /// The entries of its project's `server_objects.respawn_points`.
    pub respawn_points: usize,
    /// Its project's battery containers, as `(id, team)`.
    pub containers: Vec<(i32, i32)>,
}

impl AddedMap {
    /// Team Deathmatch on a level 12 in the day, the only mode of its map:
    /// 17 respawn points.
    pub fn level_12_tdm() -> Self {
        Self {
            id: 200,
            config: "level_12_day_4.options",
            level: 12,
            weather: super::weather::DAY,
            project: "level_12/day_4",
            mode: mode::TEAM_DEATHMATCH,
            respawn_points: 17,
            containers: Vec::new(),
        }
    }

    /// Research on a level 12 in the day, as the Research pack adds a map
    /// (`research_mode.md`): 14 respawn points and no containers.
    pub fn level_12_research() -> Self {
        Self {
            id: 202,
            config: "level_12_day_5.options",
            project: "level_12/day_5",
            mode: mode::RESEARCH,
            respawn_points: 14,
            ..Self::level_12_tdm()
        }
    }
}

/// `value`'s entry `key`, a named table's, made if it has none.
fn entry_or_new<'a>(value: &'a mut ConfigValue, key: &str) -> &'a mut ConfigValue {
    let ConfigValue::Named(entries) = value else {
        panic!("`{key}` of a value that is not a named table");
    };
    let at = match entries.iter().position(|(name, _)| name == key) {
        Some(at) => at,
        None => {
            entries.push((key.to_owned(), ConfigValue::Integer(0)));
            entries.len() - 1
        }
    };
    &mut entries[at].1
}

/// `value`'s entry `key`, a named table's.
fn entry<'a>(value: &'a mut ConfigValue, key: &str) -> &'a mut ConfigValue {
    match value {
        ConfigValue::Named(entries) => {
            &mut entries
                .iter_mut()
                .find(|(name, _)| name == key)
                .unwrap_or_else(|| panic!("no `{key}`"))
                .1
        }
        _ => panic!("`{key}` of a value that is not a named table"),
    }
}

/// The stock config whose mode is `game_mode`, on level_01 in the day.
fn stock_config(game_mode: u8) -> ConfigValue {
    let stock = super::all()
        .iter()
        .find(|map| (map.level, map.weather, map.mode) == (1, super::weather::DAY, game_mode))
        .expect("a stock config of the mode");
    let bytes = game_data::shared()
        .file(&format!("{GAME_MODES_DIR}{}", stock.config))
        .expect("its file");
    ConfigValue::parse(bytes).unwrap()
}

/// The test data ([`game_data::shared`]) with `map` added as a patch adds
/// one. Its config is level_01's of its mode with the map's numbers and
/// names (Research, which the test data lacks: Team Deathmatch's with the
/// rules and the goal of the pack's configs); its localization key is
/// `st_level_<level>_name`, which no localization has.
pub fn with_map(map: &AddedMap) -> GameData {
    let stock = game_data::shared();

    let mut config = stock_config(match map.mode {
        mode::RESEARCH => mode::TEAM_DEATHMATCH,
        other if mode::ALL.contains(&other) => other,
        _ => mode::BATTERIES,
    });
    let root = entry(&mut config, "mode");
    *entry(root, "name") = ConfigValue::String(map.config.trim_end_matches(".options").into());
    *entry(root, "mode") = ConfigValue::Integer(map.mode.into());
    if map.mode == mode::RESEARCH {
        *entry(root, "rules") = ConfigValue::Indexed(
            [93, 91, 480, 94]
                .into_iter()
                .map(ConfigValue::Integer)
                .collect(),
        );
        let parameters = entry(root, "parameters");
        *entry_or_new(parameters, "tickets_count") = ConfigValue::Integer(0);
        *entry_or_new(parameters, "resources_count") = ConfigValue::Integer(100);
    }
    let making = entry(root, "match_making_params");
    *entry(making, "map_id") = ConfigValue::Integer(map.level.into());
    *entry(making, "map_weather_id") = ConfigValue::Integer(map.weather.into());
    *entry(making, "map_name") = ConfigValue::String(map.project.into());
    let ui = entry(root, "ui");
    *entry(ui, "map_name") = ConfigValue::String(format!("st_level_{}_name", map.level));

    let point = |team: i32| {
        ConfigValue::Named(vec![
            ("team".into(), ConfigValue::Integer(team)),
            (
                "position".into(),
                ConfigValue::Float3([0.0, 0.0, team as f32]),
            ),
        ])
    };
    let mut objects = vec![(
        "respawn_points".to_owned(),
        ConfigValue::Indexed(
            (0..map.respawn_points)
                .map(|n| point(n as i32 % 2))
                .collect(),
        ),
    )];
    if !map.containers.is_empty() {
        let containers = map
            .containers
            .iter()
            .map(|&(id, team)| {
                ConfigValue::Named(vec![
                    ("id".into(), ConfigValue::Integer(id)),
                    ("team".into(), ConfigValue::Integer(team)),
                ])
            })
            .collect();
        objects.push((
            "victory_items_containers".to_owned(),
            ConfigValue::Indexed(containers),
        ));
    }
    let project = ConfigValue::Named(vec![(
        resources_db::server::SERVER_OBJECTS.to_owned(),
        ConfigValue::Named(objects),
    )]);

    let mut dictionary_config = ConfigValue::parse(stock.file(dictionary::PATH).unwrap()).unwrap();
    match entry(&mut dictionary_config, "maps_dict") {
        ConfigValue::Indexed(rows) => rows.push(ConfigValue::Named(vec![
            ("id".into(), ConfigValue::Integer(map.id.into())),
            (
                "cfg_mode_name".into(),
                ConfigValue::String(map.config.into()),
            ),
        ])),
        _ => panic!("maps_dict is not a list"),
    }

    stock.laid_over(
        "the test data and a map of a patch",
        vec![
            (
                dictionary::PATH.to_owned(),
                dictionary_config.to_bytes().unwrap(),
            ),
            (
                format!("{GAME_MODES_DIR}{}", map.config),
                config.to_bytes().unwrap(),
            ),
            (
                format!("projects/{}/client_project", map.project),
                project.to_bytes().unwrap(),
            ),
        ],
    )
}

/// The table of `data` ([`super::derive`]), with its own dictionary.
pub fn table_of(data: &GameData) -> Result<Vec<MapMode>, game_data::GameDataError> {
    let dictionary = Dictionary::parse(data.file(dictionary::PATH).unwrap()).unwrap();
    super::derive(data, &dictionary)
}
