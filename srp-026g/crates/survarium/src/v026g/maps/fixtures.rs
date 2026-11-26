//! Game data made up for tests: the test data with a map laid over it as
//! a patch adds one ([`with_map`]): a row in `maps_dict` of a new
//! dictionary, its game mode config, its project. Nothing of the map is
//! known to the code beforehand, so a table derived from it shows what the
//! server makes of a map it has never seen.

use binary_config_parser::tree::ConfigValue;

use super::super::dictionary::{self, Dictionary};
use super::super::game_data::{self, GameData};
use super::{GAME_MODES_DIR, MapId, MapMode, RuleMode};

/// A map a patch adds.
#[derive(Clone, Debug, PartialEq)]
pub struct AddedMap {
    /// Its `maps_dict` id.
    pub id: MapId,
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
    /// `mode.srp_build`, as `modes.tsv`'s `build` writes it.
    pub srp_build: Option<&'static str>,
}

impl AddedMap {
    /// Team Deathmatch on a level 12 in the day, the only mode of its map:
    /// 17 respawn points.
    pub fn level_12_tdm() -> Self {
        Self {
            id: MapId::new(200),
            config: "level_12_day_4.options",
            level: 12,
            weather: super::weather::DAY,
            project: "level_12/day_4",
            mode: RuleMode::TeamDeathmatch.number(),
            respawn_points: 17,
            containers: Vec::new(),
            srp_build: None,
        }
    }

    /// Research on a level 12 in the day, as the Research pack adds a map
    /// (`research_mode.md`): 14 respawn points and no containers.
    pub fn level_12_research() -> Self {
        Self {
            id: MapId::new(202),
            config: "level_12_day_5.options",
            project: "level_12/day_5",
            mode: RuleMode::Research.number(),
            respawn_points: 14,
            ..Self::level_12_tdm()
        }
    }

    /// Artifact Hunt on a level 12 in the day, as the Artifact Hunt pack
    /// adds a map (`artefact_hunt.md`, "Data"): 16 respawn points, the rule's
    /// config ([`artefact_hunt_mode`]), an id over a byte.
    pub fn level_12_artefact_hunt() -> Self {
        Self {
            id: MapId::new(330),
            config: "artefact_hunt/level_12_day_ah.options",
            project: "level_12/day_ah",
            mode: RuleMode::ArtefactHunt.number(),
            respawn_points: 16,
            ..Self::level_12_tdm()
        }
    }
}

/// The parts of an Artifact Hunt config the rule reads, 0.46c2's values:
/// two spawners and the two bases 60 m apart.
pub fn artefact_hunt_mode() -> Vec<(String, ConfigValue)> {
    let int = |key: &str, value: i32| (key.to_owned(), ConfigValue::Integer(value));
    let at = |position: [f32; 3]| ConfigValue::Named(vec![("position".into(), ConfigValue::Float3(position))]);
    let base = |team: i32, position: [f32; 3]| {
        let box_shape = ConfigValue::Named(vec![
            ("type".into(), ConfigValue::Integer(1)),
            ("position".into(), ConfigValue::Float3(position)),
            ("axis_x".into(), ConfigValue::Float3([1.0, 0.0, 0.0])),
            ("axis_y".into(), ConfigValue::Float3([0.0, 1.0, 0.0])),
            ("axis_z".into(), ConfigValue::Float3([0.0, 0.0, 1.0])),
            ("half".into(), ConfigValue::Float3([0.6, 0.6, 0.6])),
        ]);
        ConfigValue::Named(vec![
            ("team".into(), ConfigValue::Integer(team)),
            ("position".into(), ConfigValue::Float3(position)),
            ("shapes".into(), ConfigValue::Indexed(vec![box_shape])),
        ])
    };
    vec![
        (
            "parameters".into(),
            ConfigValue::Named(vec![
                int("match_time", 900),
                int("wait_time", 5),
                int("countdown_time", 20),
                ("wait_player_percent".into(), ConfigValue::Float(0.6)),
                int("respawn_time", 15),
                int("time_needed_to_continue_match", 90),
                int("draw_additional_time", 180),
                int("time_before_new_round", 3),
                int("round_countdown_time", 5),
                int("time_needed_to_respawn", 7),
                int("vi_spawn_time", 60),
                int("vi_spawn_variation", 10),
                int("vi_not_picked_ttl", 120),
                int("vi_dropped_ttl", 60),
                int("score", 5),
            ]),
        ),
        (
            "artefact_hunt".into(),
            ConfigValue::Named(vec![
                (
                    "spawners".into(),
                    ConfigValue::Indexed(vec![at([0.0, 1.0, 0.0]), at([5.0, 1.0, -5.0])]),
                ),
                (
                    "bases".into(),
                    ConfigValue::Indexed(vec![
                        base(1, [30.0, 0.0, 0.0]),
                        base(0, [-30.0, 0.0, 0.0]),
                    ]),
                ),
                (
                    "item".into(),
                    ConfigValue::Named(vec![
                        ("drop_lift".into(), ConfigValue::Float(0.13)),
                        ("usable_radius".into(), ConfigValue::Float(0.3)),
                    ]),
                ),
            ]),
        ),
    ]
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
fn stock_config(game_mode: RuleMode) -> ConfigValue {
    let stock = super::all()
        .iter()
        .find(|map| (map.level, map.weather, map.mode) == (1, super::weather::DAY, game_mode.into()))
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
    with_maps(std::slice::from_ref(map))
}

/// The test data with every map of `maps` added, as one patch adds them ([`with_map`]).
pub fn with_maps(maps: &[AddedMap]) -> GameData {
    let stock = game_data::shared();
    let mut dictionary_config = ConfigValue::parse(stock.file(dictionary::PATH).unwrap()).unwrap();
    let mut files = Vec::new();
    for map in maps {
        match entry(&mut dictionary_config, "maps_dict") {
            ConfigValue::Indexed(rows) => rows.push(ConfigValue::Named(vec![
                ("id".into(), ConfigValue::Integer(map.id.get().into())),
                (
                    "cfg_mode_name".into(),
                    ConfigValue::String(map.config.into()),
                ),
            ])),
            _ => panic!("maps_dict is not a list"),
        }
        files.extend(map_files(map));
    }
    files.push((
        dictionary::PATH.to_owned(),
        dictionary_config.to_bytes().unwrap(),
    ));
    stock.laid_over("the test data and maps of a patch", files)
}

/// `map`'s game mode config and its project, by path.
fn map_files(map: &AddedMap) -> [(String, Vec<u8>); 2] {
    let mut config = stock_config(match RuleMode::of_config(map.mode) {
        Some(RuleMode::Research | RuleMode::FreezeTag | RuleMode::ArtefactHunt) => RuleMode::TeamDeathmatch,
        Some(other) => other,
        None => RuleMode::Batteries,
    });
    let root = entry(&mut config, "mode");
    *entry(root, "name") = ConfigValue::String(map.config.trim_end_matches(".options").into());
    *entry(root, "mode") = ConfigValue::Integer(map.mode.into());
    if map.mode == RuleMode::FreezeTag.number() {
        *entry(root, "rules") = ConfigValue::Indexed([93, 91, 94].into_iter().map(ConfigValue::Integer).collect());
    }
    if map.mode == RuleMode::Research.number() {
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
    if map.mode == RuleMode::ArtefactHunt.number() {
        *entry(root, "rules") = ConfigValue::Indexed(
            [93, 91, 481, 94]
                .into_iter()
                .map(ConfigValue::Integer)
                .collect(),
        );
        for (key, value) in artefact_hunt_mode() {
            *entry_or_new(root, &key) = value;
        }
    }
    let making = entry(root, "match_making_params");
    *entry(making, "map_id") = ConfigValue::Integer(map.level.into());
    *entry(making, "map_weather_id") = ConfigValue::Integer(map.weather.into());
    *entry(making, "map_name") = ConfigValue::String(map.project.into());
    let ui = entry(root, "ui");
    *entry(ui, "map_name") = ConfigValue::String(format!("st_level_{}_name", map.level));
    if let Some(tag) = map.srp_build {
        *entry_or_new(root, "srp_build") = ConfigValue::String(tag.into());
    }

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
    [
        (
            format!("{GAME_MODES_DIR}{}", map.config),
            config.to_bytes().unwrap(),
        ),
        (
            format!("projects/{}/client_project", map.project),
            project.to_bytes().unwrap(),
        ),
    ]
}

/// The table of `data` ([`super::derive`]), with its own dictionary.
pub fn table_of(data: &GameData) -> Result<Vec<MapMode>, game_data::GameDataError> {
    let dictionary = Dictionary::parse(data.file(dictionary::PATH).unwrap()).unwrap();
    super::derive(data, &dictionary)
}
