//! The maps and game modes of 0.26g (`docs/notes/modes_maps.md`,
//! `static_info.md` §4).
//!
//! A 0.26g match names its map, weather and mode by one byte: an id of
//! `maps_dict` in the client's own `gameplay/db_static_dictionaries`, which
//! names a file under `gameplay/game_modes/`. That file holds the mode, its
//! rules and every timing, the names the client shows, and the project the
//! client loads. The server sends only the id, so its own clock and scores
//! must use the same values: the table here is derived from the players'
//! files when first used ([`all`], [`super::game_data`]), and so is what
//! the server needs of each project: how many respawn points it has and
//! whose its battery containers are (its `server_objects`).
//!
//! An id can be played ([`MapMode::is_playable`]) when the game data has
//! its config, the config is of a mode srp plays (2 to 5) and the data
//! has its project. No level or id is known here beforehand: a map a patch
//! adds is played as soon as the server runs on the data with it. Of
//! 0.26g's 89 ids, 72 can be played (modes 2, 3 and 4 of its maps in their
//! weathers); with London (`level_09`, patch 14 of skhid) 75; with the
//! Research pack (mode 5, `research_mode.md`) 25 more, among them 145..=149.
//! The others must never be sent:
//! - 114 and 143..=151 name a config the client does not have ("Wrong map
//!   config. Map id = %u", 0x2C9D18);
//! - 107 and 108 (level_08 at night) and the test maps 11, 45 and 115 name a
//!   project that `resources.db` does not have;
//! - 141 (`loopback_client`) and 142 (`gather_server_resources`) are mode 0.

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

/// Where `maps_dict`'s `cfg_mode_name` is.
pub const GAME_MODES_DIR: &str = "gameplay/game_modes/";

/// `mode.mode` of the configs.
pub mod mode {
    /// Gather batteries ("Protective device"), rule class 88.
    pub const BATTERIES: u8 = 2;
    /// Batteries that appear over time, with charging stations; rule 89.
    pub const TIMED_BATTERIES: u8 = 3;
    /// Team Deathmatch with tickets; rule 90.
    pub const TEAM_DEATHMATCH: u8 = 4;
    /// Research: three units to capture, the first team to 100 points
    /// wins; rule 480, exe patch 53 (`docs/notes/research_mode.md`).
    pub const RESEARCH: u8 = 5;
    /// The modes a match can be.
    pub const ALL: [u8; 4] = [BATTERIES, TIMED_BATTERIES, TEAM_DEATHMATCH, RESEARCH];
    /// The modes whose maps have the teams' battery containers.
    pub const WITH_BATTERIES: [u8; 2] = [BATTERIES, TIMED_BATTERIES];
    /// Blackout ("Protective device: blackout", `docs/notes/blackout.md`): a mode of
    /// the lobby's only. A match of it is one of [`super::BLACKOUT_RULES`]
    /// with the static info's Blackout flag; no config says 6.
    pub const BLACKOUT: u8 = 6;
}

/// The rules a Blackout match plays: the plain battery mode (no chargers).
pub const BLACKOUT_RULES: u8 = mode::BATTERIES;

/// Blackout's maps, `(the lobby's id, the maps_dict id played)`: one for
/// each playable map of [`BLACKOUT_RULES`] in `rules_ids`, by id, each named
/// by an id `maps_dict` (`dictionary_ids`) does not use, from 255 down (0
/// never). The lobby's rooms pool, cards and rows carry the lobby's id; a
/// match is made on the id played, with the flag.
pub fn blackout_pairs_of(dictionary_ids: &[u8], rules_ids: &[u8]) -> Vec<(u8, u8)> {
    let mut free = (1..=u8::MAX).rev().filter(|id| !dictionary_ids.contains(id));
    let mut played: Vec<u8> = rules_ids.to_vec();
    played.sort_unstable();
    played.dedup();
    played
        .into_iter()
        .map_while(|id| free.next().map(|lobby| (lobby, id)))
        .collect()
}

/// [`blackout_pairs_of`] the game data's dictionary and playable maps.
pub fn blackout_pairs() -> &'static [(u8, u8)] {
    static SHARED: std::sync::OnceLock<Vec<(u8, u8)>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let dictionary: Vec<u8> = Dictionary::shared().maps.iter().map(|map| map.id).collect();
        let rules: Vec<u8> = playable_in(BLACKOUT_RULES).map(|map| map.id).collect();
        blackout_pairs_of(&dictionary, &rules)
    })
}

/// The `maps_dict` id a lobby's Blackout id plays, if it is one.
pub fn blackout_played(id: u8) -> Option<u8> {
    blackout_pairs()
        .iter()
        .find(|(lobby, _)| *lobby == id)
        .map(|&(_, played)| played)
}

/// The lobby's Blackout id of a `maps_dict` id of [`BLACKOUT_RULES`].
pub fn blackout_id(map_id: u8) -> Option<u8> {
    blackout_pairs()
        .iter()
        .find(|(_, played)| *played == map_id)
        .map(|&(lobby, _)| lobby)
}

/// `match_making_params.map_weather_id`.
pub mod weather {
    pub const DAY: u8 = 1;
    pub const RAIN: u8 = 2;
    pub const EVENING: u8 = 3;
    pub const NIGHT: u8 = 4;
    pub const PRERAIN: u8 = 5;
}

/// One `maps_dict` id and its `gameplay/game_modes/` config, with what the
/// server reads of its project. A parameter the config lacks is 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapMode {
    /// The `maps_dict` id: the match options' `u8 map_id`.
    pub id: u8,
    /// `cfg_mode_name`, the file under `gameplay/game_modes/`.
    pub config: &'static str,
    /// `mode.name`.
    pub name: &'static str,
    /// `mode.mode` ([`mode`]).
    pub mode: u8,
    /// `match_making_params.map_id`: the map, whatever the weather and mode
    /// (`level_0N` is N; the test maps are 88). A number of the data's.
    pub level: u8,
    /// `match_making_params.map_weather_id` ([`weather`]).
    pub weather: u8,
    /// `match_making_params.map_name`: the project under `projects/`.
    pub project: &'static str,
    /// Whether the game data has that project's `client_project`.
    pub has_project: bool,
    /// `ui.loading_screen_id` and `ui.icon`.
    pub loading_screen_id: u8,
    pub icon: u8,
    /// `ui.map_name` and `ui.mode_name`: the keys of the map's and the
    /// mode's names in the game's localizations (`st_level_01_name`,
    /// `st_mode_04_name`; [`super::localization`]).
    pub map_name_key: &'static str,
    pub mode_name_key: &'static str,
    /// `rules`: the rule resource classes the client loads (Research's
    /// own is 480, over a byte).
    pub rules: &'static [u16],
    /// Seconds a match lasts.
    pub match_time: u32,
    pub wait_time: u32,
    /// Seconds of countdown before the match starts.
    pub countdown_time: u32,
    /// The share of the roster that must be in before the countdown.
    pub wait_player_percent: f32,
    /// Seconds from a death to the respawn.
    pub respawn_time: u32,
    /// Modes 2 and 3: the batteries that win.
    pub victory_items_count: u32,
    /// Mode 3: seconds between batteries, give or take the variation, and
    /// the charging stations in play.
    pub vi_spawn_time: u32,
    pub vi_spawn_variation: u32,
    pub spawn_points_count: u32,
    /// Mode 4: a team's tickets.
    pub tickets_count: u32,
    /// Mode 5: the points that win (`resources_count`, 100).
    pub resources_count: u32,
    /// How many respawn points the project has: every entry of its
    /// `server_objects.respawn_points`, whatever its team or layer, is
    /// loaded (`player_respawn_rule_cook` 0x2C6910), so this is how many
    /// pairs of bytes the respawn rule writes into a snapshot
    /// (snapshot_rules.md §2.3). PROVEN (code and data). 0 without a
    /// project.
    pub respawn_points: usize,
    /// Modes 2 and 3: whose the two battery containers are, by container
    /// id: `victory_items_containers[..].team` of the project by its `id`.
    /// PROVEN (data). A battery's container id in a snapshot is this id,
    /// not the team. `None` in modes 4 and 5 and without a project.
    pub container_teams: Option<[u8; 2]>,
}

impl MapMode {
    /// Whether a match can be played on it: a mode srp plays ([`mode::ALL`]),
    /// on a map whose project the game data has. Its config is there, or
    /// it would have no row.
    pub fn is_playable(&self) -> bool {
        self.has_project && mode::ALL.contains(&self.mode)
    }
}

/// A number of a game mode config; a missing one is 0.
fn int<T: TryFrom<i32>>(table: Option<&ConfigValue>, key: &str) -> Result<T, ConfigError> {
    let number = match table.and_then(|table| table.get(key)) {
        None => 0,
        Some(value) => value
            .as_i32()
            .ok_or_else(|| ConfigError(format!("`{key}` is not a number")))?,
    };
    T::try_from(number).map_err(|_| ConfigError(format!("`{key}` = {number} is out of range")))
}

fn leak_str(text: &str) -> &'static str {
    Box::leak(text.to_owned().into_boxed_str())
}

/// What the server reads of a project (its `client_project`, or the
/// server data's cut of it: [`resources_db::server::server_objects_of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectObjects {
    /// [`MapMode::respawn_points`].
    pub respawn_points: usize,
    /// The battery containers' teams by container id, if the project has
    /// containers ([`MapMode::container_teams`]).
    pub container_teams: Option<[u8; 2]>,
}

impl ProjectObjects {
    /// The respawn points and the battery containers of the project
    /// `project`, a parsed `client_project`. Containers there must be two,
    /// ids 0 and 1, of teams 0 and 1, one each: the gather rules and the
    /// scores know no other.
    /// `with_batteries`: the containers are read (and must be one of each
    /// team) only for a mode that has batteries; a Team Deathmatch or
    /// Research project copied from a batteries one may keep containers it
    /// never uses.
    pub fn from_project(project: &ConfigValue, with_batteries: bool) -> Result<Self, ConfigError> {
        let objects = project.get("server_objects");
        let respawn_points = objects
            .and_then(|objects| objects.get("respawn_points"))
            .map_or(0, |points| points.children().len());
        let containers = objects
            .filter(|_| with_batteries)
            .and_then(|objects| objects.get("victory_items_containers"))
            .map(ConfigValue::children)
            .unwrap_or_default();
        let container_teams = if containers.is_empty() {
            None
        } else {
            let mut teams: [Option<u8>; 2] = [None, None];
            for container in &containers {
                let id = container.i32_at("id")?;
                let team = container.i32_at("team")?;
                let place = usize::try_from(id)
                    .ok()
                    .filter(|&id| id < 2)
                    .ok_or_else(|| ConfigError(format!("a battery container has id {id}")))?;
                if !(0..2).contains(&team) {
                    return Err(ConfigError(format!(
                        "battery container {id} is of team {team}"
                    )));
                }
                if teams[place].replace(team as u8).is_some() {
                    return Err(ConfigError(format!("two battery containers have id {id}")));
                }
            }
            match teams {
                [Some(first), Some(second)] if first != second => Some([first, second]),
                _ => {
                    return Err(ConfigError(format!(
                        "the battery containers are not one of each team: {teams:?}"
                    )));
                }
            }
        };
        Ok(Self {
            respawn_points,
            container_teams,
        })
    }
}

impl MapMode {
    /// The row of a `maps_dict` id's config (`gameplay/game_modes/`
    /// `cfg_mode_name`) and what the server reads of its project, if the
    /// data has the project ([`ProjectObjects::from_project`]). A playable
    /// id ([`MapMode::is_playable`]) must have a respawn point, one of
    /// modes 2 and 3 the two teams' battery containers, and one of mode 5
    /// the points that win. The strings are leaked: the rows live as long
    /// as the process.
    pub fn from_config(
        id: u8,
        cfg_mode_name: &str,
        config: &ConfigValue,
        project: Option<&ConfigValue>,
    ) -> Result<Self, ConfigError> {
        let mode = config.req("mode")?;
        let p = mode.get("parameters");
        let making = mode.get("match_making_params");
        let ui = mode.get("ui");
        let text = |table: Option<&ConfigValue>, key: &str| -> Result<&'static str, ConfigError> {
            match table.and_then(|table| table.get(key)) {
                None => Ok(""),
                Some(value) => value
                    .as_str()
                    .map(leak_str)
                    .ok_or_else(|| ConfigError(format!("`{key}` is not a string"))),
            }
        };
        let rules: Vec<u16> = match mode.get("rules") {
            None => Vec::new(),
            Some(list) => list
                .children()
                .into_iter()
                .map(|rule| {
                    rule.as_i32()
                        .and_then(|rule| u16::try_from(rule).ok())
                        .ok_or_else(|| ConfigError("a `rules` entry is not a class id".into()))
                })
                .collect::<Result<_, _>>()?,
        };
        let wait_player_percent = match p.and_then(|p| p.get("wait_player_percent")) {
            None => 0.0,
            Some(value) => value
                .as_f32()
                .ok_or_else(|| ConfigError("`wait_player_percent` is not a number".into()))?,
        };
        let game_mode: u8 = int(Some(mode), "mode")?;
        let objects = project
            .map(|project| {
                ProjectObjects::from_project(project, mode::WITH_BATTERIES.contains(&game_mode))
            })
            .transpose()?
            .unwrap_or(ProjectObjects {
                respawn_points: 0,
                container_teams: None,
            });
        let row = Self {
            id,
            config: leak_str(cfg_mode_name),
            name: text(Some(mode), "name")?,
            mode: game_mode,
            level: int(making, "map_id")?,
            weather: int(making, "map_weather_id")?,
            project: text(making, "map_name")?,
            has_project: project.is_some(),
            loading_screen_id: int(ui, "loading_screen_id")?,
            icon: int(ui, "icon")?,
            map_name_key: text(ui, "map_name")?,
            mode_name_key: text(ui, "mode_name")?,
            rules: if rules.is_empty() {
                &[]
            } else {
                Box::leak(rules.into_boxed_slice())
            },
            match_time: int(p, "match_time")?,
            wait_time: int(p, "wait_time")?,
            countdown_time: int(p, "countdown_time")?,
            wait_player_percent,
            respawn_time: int(p, "respawn_time")?,
            victory_items_count: int(p, "victory_items_count")?,
            vi_spawn_time: int(p, "vi_spawn_time")?,
            vi_spawn_variation: int(p, "vi_spawn_variation")?,
            spawn_points_count: int(p, "spawn_points_count")?,
            tickets_count: int(p, "tickets_count")?,
            resources_count: int(p, "resources_count")?,
            respawn_points: objects.respawn_points,
            container_teams: objects
                .container_teams
                .filter(|_| mode::WITH_BATTERIES.contains(&game_mode)),
        };
        if row.is_playable() {
            if row.respawn_points == 0 {
                return Err(ConfigError(format!(
                    "its project {} has no respawn points",
                    row.project
                )));
            }
            if mode::WITH_BATTERIES.contains(&row.mode) && row.container_teams.is_none() {
                return Err(ConfigError(format!(
                    "its project {} has no battery containers, and it is mode {}",
                    row.project, row.mode
                )));
            }
            if row.mode == mode::RESEARCH && row.resources_count == 0 {
                return Err(ConfigError(
                    "it is mode 5 and has no `resources_count`".to_owned(),
                ));
            }
        }
        Ok(row)
    }
}

/// Every `maps_dict` id whose config the players have, by id ([`derive`]).
pub fn all() -> &'static [MapMode] {
    static SHARED: std::sync::OnceLock<Vec<MapMode>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        derive(super::game_data::shared(), Dictionary::shared()).expect("the game modes read")
    })
}

/// A row for each `maps_dict` id of `dictionary` whose config `data` has,
/// by id, with what the server reads of its project if `data` has it
/// ([`MapMode::from_config`]). An id whose config the game lacks is left
/// out, and so never sent (114 and 143..=151 of 0.26g).
pub fn derive(data: &GameData, dictionary: &Dictionary) -> Result<Vec<MapMode>, GameDataError> {
    let mut rows = Vec::new();
    for entry in &dictionary.maps {
        let path = format!("{GAME_MODES_DIR}{}", entry.cfg_mode_name);
        let Some(config) = super::game_data::config(data, &path)? else {
            continue;
        };
        let project_name = config
            .get("mode")
            .and_then(|mode| mode.get("match_making_params"))
            .and_then(|making| making.get("map_name"))
            .and_then(ConfigValue::as_str)
            .unwrap_or("");
        let project = data
            .project(project_name)
            .map(|bytes| {
                ConfigValue::parse(bytes).map_err(|cause| {
                    GameDataError(format!(
                        "projects/{project_name}/client_project: {}",
                        cause.0
                    ))
                })
            })
            .transpose()?;
        let row = MapMode::from_config(entry.id, &entry.cfg_mode_name, &config, project.as_ref())
            .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?;
        rows.push(row);
    }
    rows.sort_by_key(|row| row.id);
    if let Some(pair) = rows.windows(2).find(|w| w[0].id == w[1].id) {
        return Err(GameDataError(format!(
            "map id {} is in maps_dict twice",
            pair[0].id
        )));
    }
    Ok(rows)
}

/// The config of a `maps_dict` id, if the client has one.
pub fn map_mode(id: u8) -> Option<&'static MapMode> {
    let all = all();
    all.binary_search_by_key(&id, |map| map.id)
        .ok()
        .map(|index| &all[index])
}

/// Every id a match may use, by id.
pub fn playable() -> impl Iterator<Item = &'static MapMode> {
    all().iter().filter(|map| map.is_playable())
}

/// The ids a match of `mode` may use.
pub fn playable_in(mode: u8) -> impl Iterator<Item = &'static MapMode> {
    playable().filter(move |map| map.mode == mode)
}

/// The id for a level, weather and mode, if it can be played.
pub fn find(level: u8, weather: u8, mode: u8) -> Option<&'static MapMode> {
    playable().find(|map| (map.level, map.weather, map.mode) == (level, weather, mode))
}

#[cfg(test)]
mod tests {
    use super::super::dictionary::Dictionary;
    use super::*;

    fn ids(mode: u8) -> Vec<u8> {
        playable_in(mode).map(|map| map.id).collect()
    }

    #[test]
    fn blackout_names_its_maps_by_free_ids_from_the_top() {
        let pairs = blackout_pairs_of(&[1, 61, 63, 254, 252], &[63, 61, 61]);
        assert_eq!(pairs, vec![(255, 61), (253, 63)]);
        // No free id left: no more Blackout maps.
        let all: Vec<u8> = (1..=254).collect();
        assert_eq!(blackout_pairs_of(&all, &[61, 63]), vec![(255, 61)]);
        // The data's: every plain battery map has one, none of them a maps_dict id.
        let pairs = blackout_pairs();
        assert_eq!(pairs.len(), playable_in(BLACKOUT_RULES).count());
        for &(lobby, played) in pairs {
            assert!(map_mode(lobby).is_none());
            assert_eq!(map_mode(played).map(|map| map.mode), Some(BLACKOUT_RULES));
            assert_eq!(blackout_played(lobby), Some(played));
            assert_eq!(blackout_id(played), Some(lobby));
        }
        assert_eq!(blackout_played(61), None);
    }

    #[test]
    fn the_whitelist_is_that_of_the_notes() {
        // modes_maps.md §8.
        let mode_2: Vec<u8> = (61..=105).step_by(2).chain([109]).collect();
        let mode_3: Vec<u8> = (62..=106).step_by(2).chain([110]).collect();
        let mode_4: Vec<u8> = (117..=140).collect();
        assert_eq!(ids(mode::BATTERIES), mode_2);
        assert_eq!(ids(mode::TIMED_BATTERIES), mode_3);
        assert_eq!(ids(mode::TEAM_DEATHMATCH), mode_4);
        assert_eq!(playable().count(), 72);
        for bad in [11, 45, 107, 108, 114, 115].into_iter().chain(141..=151) {
            assert!(playable().all(|map| map.id != bad), "{bad}");
        }
        // 114 and 143..=151 have no config at all.
        for missing in [114].into_iter().chain(143..=151) {
            assert!(map_mode(missing).is_none(), "{missing}");
        }
        // 9 ids a level: 3 weathers × 3 modes.
        for level in 1..=8 {
            assert_eq!(playable().filter(|map| map.level == level).count(), 9);
        }
    }

    #[test]
    fn the_table_is_the_dictionarys_maps_dict() {
        let dictionary = Dictionary::shared();
        assert!(all().windows(2).all(|w| w[0].id < w[1].id));
        // Every row is a maps_dict id with that config name.
        for map in all() {
            let entry = dictionary.maps.iter().find(|entry| entry.id == map.id);
            assert_eq!(entry.unwrap().cfg_mode_name, map.config, "{}", map.id);
        }
        // The ids without a row are the ten whose config the db lacks.
        let missing: Vec<u8> = dictionary
            .maps
            .iter()
            .map(|entry| entry.id)
            .filter(|&id| map_mode(id).is_none())
            .collect();
        assert_eq!(missing, [114, 143, 144, 145, 146, 147, 148, 149, 150, 151]);
        // A playable id's config name spells its level, weather and mode,
        // and its project is `level_0X/<weather>_<mode>`.
        for map in playable() {
            let weather = match map.weather {
                weather::DAY => "day",
                weather::RAIN => "rain",
                weather::EVENING => "evn",
                weather::PRERAIN => "prerain",
                other => panic!("{} has weather {other}", map.id),
            };
            let config = format!("level_0{}_{weather}_{}.options", map.level, map.mode);
            assert_eq!(map.config, config);
            let long = if weather == "evn" { "evening" } else { weather };
            let project = format!("level_0{}/{long}_{}", map.level, map.mode);
            assert_eq!(map.project, project);
            assert!(map.project.len() <= 31);
        }
    }

    #[test]
    fn spot_checks() {
        let map = map_mode(61).unwrap();
        assert_eq!((map.level, map.weather, map.mode), (1, weather::DAY, 2));
        assert_eq!(map.project, "level_01/day_2");
        assert_eq!(map.rules, [93, 91, 88, 94]);
        let map = map_mode(109).unwrap();
        assert_eq!((map.level, map.weather, map.mode), (8, weather::PRERAIN, 2));
        assert_eq!(map.loading_screen_id, 25);
        assert_eq!(find(8, weather::PRERAIN, 4).unwrap().id, 140);
        assert_eq!(find(1, weather::EVENING, 3).unwrap().id, 64);
        assert_eq!(find(4, weather::RAIN, 4).unwrap().id, 128);
        // Level 8 has no rain, and its night has no project.
        assert!(find(8, weather::RAIN, 2).is_none());
        assert!(find(8, weather::NIGHT, 2).is_none());
        assert!(!map_mode(107).unwrap().has_project);
        // Loading screens, static_info.md §4.
        assert_eq!(find(5, weather::EVENING, 2).unwrap().loading_screen_id, 14);
        assert_eq!(find(7, weather::DAY, 4).unwrap().loading_screen_id, 18);
    }

    #[test]
    fn every_playable_mode_has_the_same_timings() {
        for map in playable() {
            assert_eq!(map.match_time, 900, "{}", map.id);
            assert_eq!(map.wait_time, 5, "{}", map.id);
            assert_eq!(map.countdown_time, 20, "{}", map.id);
            assert_eq!(map.wait_player_percent, 0.8, "{}", map.id);
            assert_eq!(map.respawn_time, 8, "{}", map.id);
            let (rule, icon) = match map.mode {
                mode::BATTERIES => (88, 0),
                mode::TIMED_BATTERIES => (89, 0),
                _ => (90, 2),
            };
            assert_eq!(map.rules, [93, 91, rule, 94], "{}", map.id);
            assert_eq!(map.icon, icon, "{}", map.id);
            match map.mode {
                mode::TEAM_DEATHMATCH => {
                    // 150 tickets on level_05 and level_08, else 100.
                    let tickets = if matches!(map.level, 5 | 8) { 150 } else { 100 };
                    assert_eq!(map.tickets_count, tickets, "{}", map.id);
                    assert_eq!(map.victory_items_count, 0);
                }
                _ => {
                    assert_eq!(map.victory_items_count, 10, "{}", map.id);
                    assert_eq!(map.tickets_count, 0);
                }
            }
            if map.mode == mode::TIMED_BATTERIES {
                assert_eq!((map.vi_spawn_time, map.vi_spawn_variation), (60, 10));
                let stations = if map.level == 8 { 2 } else { 3 };
                assert_eq!(map.spawn_points_count, stations, "{}", map.id);
            } else {
                assert_eq!((map.vi_spawn_time, map.spawn_points_count), (0, 0));
            }
        }
    }

    /// What srp had by hand before it read the projects, levels 1 to 9:
    /// the respawn points of modes 2 and 3 and of mode 4, and the battery
    /// containers' teams by id.
    const GATHER_BEFORE: [usize; 9] = [39, 30, 28, 36, 29, 30, 31, 26, 20];
    const TDM_BEFORE: [usize; 9] = [43, 31, 22, 75, 29, 30, 31, 26, 20];
    fn teams_before(level: u8) -> [u8; 2] {
        if matches!(level, 2 | 7) {
            [0, 1]
        } else {
            [1, 0]
        }
    }

    /// Every playable row of `table` of a level of 1 to 9 has what srp
    /// had by hand.
    fn as_before(table: &[MapMode]) {
        for map in table.iter().filter(|map| map.is_playable()) {
            let level = usize::from(map.level);
            assert!((1..=9).contains(&level), "{}", map.id);
            let (points, teams) = match map.mode {
                mode::TEAM_DEATHMATCH => (TDM_BEFORE[level - 1], None),
                _ => (GATHER_BEFORE[level - 1], Some(teams_before(map.level))),
            };
            assert_eq!(map.respawn_points, points, "{} ({})", map.id, map.project);
            assert_eq!(map.container_teams, teams, "{} ({})", map.id, map.project);
        }
    }

    #[test]
    fn the_projects_give_what_srp_had_by_hand() {
        // The test data: 0.26g's maps, levels 1 to 8, all 72.
        assert_eq!(playable().count(), 72);
        as_before(all());
        // Spot checks: level_04's Team Deathmatch has 75 points (39 of
        // layer `tdm`), level_02's 31 (one of team 2).
        assert_eq!(find(4, weather::RAIN, 4).unwrap().respawn_points, 75);
        assert_eq!(find(2, weather::DAY, 4).unwrap().respawn_points, 31);
        assert_eq!(
            find(7, weather::DAY, 3).unwrap().container_teams,
            Some([0, 1])
        );
        // Without a project, nothing: level_08 at night.
        let night = map_mode(107).unwrap();
        assert_eq!((night.respawn_points, night.container_teams), (0, None));
        // The names are the localizations' keys.
        let map = map_mode(61).unwrap();
        assert_eq!(
            (map.map_name_key, map.mode_name_key),
            ("st_level_01_name", "st_mode_02_name")
        );
    }

    #[test]
    fn a_map_of_a_patch_is_played_from_its_data() {
        // Team Deathmatch on a level 12, a map no code names.
        let added = fixtures::AddedMap::level_12_tdm();
        let data = fixtures::with_map(&added);
        let table = fixtures::table_of(&data).unwrap();
        // The stock rows are as they were; the map is one more.
        assert_eq!(table.len(), all().len() + 1);
        let map = *table.iter().find(|map| map.id == added.id).unwrap();
        assert_eq!(
            table.iter().filter(|row| row.id != added.id).count(),
            all().len()
        );
        assert!(
            table
                .iter()
                .filter(|row| row.id != added.id)
                .eq(all().iter())
        );
        assert!(map.is_playable());
        assert_eq!(
            (map.level, map.mode, map.project),
            (12, 4, "level_12/day_4")
        );
        // Its respawn points are its project's, and it has no containers.
        assert_eq!(map.respawn_points, 17);
        assert_eq!(map.container_teams, None);
        // It is played in mode 4 only.
        let playable_in = |game_mode: u8| -> Vec<u8> {
            table
                .iter()
                .filter(|map| map.is_playable() && map.mode == game_mode)
                .map(|map| map.id)
                .collect()
        };
        assert!(playable_in(mode::TEAM_DEATHMATCH).contains(&added.id));
        assert_eq!(playable_in(mode::TEAM_DEATHMATCH).len(), 25);
        for game_mode in mode::WITH_BATTERIES {
            assert!(!playable_in(game_mode).contains(&added.id));
            assert_eq!(playable_in(game_mode), ids(game_mode));
        }
        assert_eq!(table.iter().filter(|map| map.is_playable()).count(), 73);
    }

    #[test]
    fn a_research_map_is_played_from_its_data() {
        // Research on a level 12, as the pack adds its 25 maps: rule 480,
        // its project's respawn points, no containers.
        let added = fixtures::AddedMap::level_12_research();
        let table = fixtures::table_of(&fixtures::with_map(&added)).unwrap();
        let map = *table.iter().find(|map| map.id == added.id).unwrap();
        assert!(map.is_playable());
        assert_eq!(
            (map.level, map.mode, map.project),
            (12, mode::RESEARCH, "level_12/day_5")
        );
        assert_eq!(map.rules, [93, 91, 480, 94]);
        assert_eq!((map.respawn_points, map.container_teams), (14, None));
        assert_eq!((map.resources_count, map.tickets_count), (100, 0));
        assert!(!mode::WITH_BATTERIES.contains(&map.mode));
        // The stock maps are as they were.
        assert!(
            table
                .iter()
                .filter(|row| row.id != added.id)
                .eq(all().iter())
        );
        // A stray container of a project copied from a batteries one is
        // not read.
        let stray = fixtures::AddedMap {
            containers: vec![(3, 1)],
            ..added.clone()
        };
        let table = fixtures::table_of(&fixtures::with_map(&stray)).unwrap();
        let map = table.iter().find(|map| map.id == added.id).unwrap();
        assert!(map.is_playable());
        assert_eq!(map.container_teams, None);
        // Without a respawn point it stops the server, as every mode.
        let bare = fixtures::AddedMap {
            respawn_points: 0,
            ..added
        };
        let message = fixtures::table_of(&fixtures::with_map(&bare))
            .unwrap_err()
            .to_string();
        assert!(message.contains("no respawn points"), "{message}");
    }

    #[test]
    fn a_map_whose_project_the_server_cannot_play_stops_it() {
        // Batteries on a map without its two containers, or with two of
        // one team; a map without a respawn point.
        let batteries = |containers: Vec<(i32, i32)>, respawn_points| fixtures::AddedMap {
            id: 201,
            config: "level_12_day_2.options",
            project: "level_12/day_2",
            mode: mode::BATTERIES,
            containers,
            respawn_points,
            ..fixtures::AddedMap::level_12_tdm()
        };
        for (map, why) in [
            (batteries(Vec::new(), 10), "no battery containers"),
            (batteries(vec![(0, 1), (1, 1)], 10), "not one of each team"),
            (
                batteries(vec![(0, 1), (0, 0)], 10),
                "two battery containers have id 0",
            ),
            (batteries(vec![(0, 1), (2, 0)], 10), "id 2"),
            (batteries(vec![(0, 1), (1, 0)], 0), "no respawn points"),
        ] {
            let message = fixtures::table_of(&fixtures::with_map(&map))
                .unwrap_err()
                .to_string();
            assert!(message.contains("level_12_day_2.options"), "{message}");
            assert!(message.contains(why), "{message}");
        }
        // With them: played, its containers' teams by id.
        let table =
            fixtures::table_of(&fixtures::with_map(&batteries(vec![(1, 0), (0, 1)], 12))).unwrap();
        let map = table.iter().find(|map| map.id == 201).unwrap();
        assert!(map.is_playable());
        assert_eq!(
            (map.respawn_points, map.container_teams),
            (12, Some([1, 0]))
        );
        // Team Deathmatch reads no containers: a project copied from a
        // batteries one with a stray container (one, of an odd id) is played.
        let tdm = fixtures::AddedMap {
            containers: vec![(3, 1)],
            ..fixtures::AddedMap::level_12_tdm()
        };
        let table = fixtures::table_of(&fixtures::with_map(&tdm)).unwrap();
        let map = table.iter().find(|map| map.id == 200).unwrap();
        assert!(map.is_playable());
        assert_eq!(map.container_teams, None);
        // A mode srp does not play is in the table, not played, whatever
        // its project.
        let other = fixtures::AddedMap {
            mode: 6,
            respawn_points: 0,
            ..fixtures::AddedMap::level_12_tdm()
        };
        let table = fixtures::table_of(&fixtures::with_map(&other)).unwrap();
        assert!(
            !table
                .iter()
                .find(|map| map.id == 200)
                .unwrap()
                .is_playable()
        );
    }

    fn env_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")))
    }

    /// The server data of 0.26g's archive (`SRP_TEST_RESOURCES_DB`) with
    /// skhid's patches (`SRP_TEST_SKHID_PATCHES`, by spaces) and then
    /// `more` over it, checked as srp checks it at start.
    fn live_data(more: &[std::path::PathBuf]) -> GameData {
        use resources_db::server::{PatchFile, build, url_in};
        let db = env_path("SRP_TEST_RESOURCES_DB");
        let patches: Vec<PatchFile> = std::env::var("SRP_TEST_SKHID_PATCHES")
            .expect("SRP_TEST_SKHID_PATCHES")
            .split_whitespace()
            .map(std::path::PathBuf::from)
            .chain(more.iter().cloned())
            .map(|path| {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                PatchFile {
                    urls: vec![url_in("http://203.0.113.5/patches", &name)],
                    path,
                }
            })
            .collect();
        let built = build(&db, &patches, None).unwrap();
        let mut archive = resources_db::Archive::new(std::io::Cursor::new(built.bytes)).unwrap();
        let data = GameData::from_archive(&mut archive, "live".into()).unwrap();
        super::super::tables::check(&data).unwrap();
        data
    }

    /// The live game data: 0.26g's archive (`SRP_TEST_RESOURCES_DB`) with
    /// skhid's patches (`SRP_TEST_SKHID_PATCHES`, by spaces: patch 12 and
    /// patch 14, London) over it. The maps of 0.26g, levels 1 to 8, have
    /// what srp had by hand; London is played too (ids 181 to 183) with
    /// its project's values, and named by its patch.
    #[test]
    #[ignore = "needs 0.26g's resources.db and skhid's patches"]
    fn the_live_data_gives_what_srp_had_by_hand_and_london() {
        let data = live_data(&[]);
        let table = fixtures::table_of(&data).unwrap();
        let played: Vec<&MapMode> = table.iter().filter(|map| map.is_playable()).collect();
        let stock: Vec<u8> = playable().map(|map| map.id).collect();
        let mut expected = stock.clone();
        expected.extend([181, 182, 183]);
        assert_eq!(
            played.iter().map(|map| map.id).collect::<Vec<_>>(),
            expected
        );
        for map in &played {
            eprintln!(
                "{:3} level {} weather {} mode {} {:24} respawn points {:2} containers {:?}",
                map.id,
                map.level,
                map.weather,
                map.mode,
                map.project,
                map.respawn_points,
                map.container_teams
            );
        }
        // The stock maps are as in the test data, and as before.
        for map in table.iter().filter(|map| stock.contains(&map.id)) {
            assert_eq!(Some(map), map_mode(map.id), "{}", map.id);
        }
        as_before(&table);
        let london: Vec<&&MapMode> = played.iter().filter(|map| map.level == 9).collect();
        assert_eq!(london.len(), 3);
        for map in london {
            assert_eq!(map.respawn_points, 20, "{}", map.id);
            if map.mode != mode::TEAM_DEATHMATCH {
                assert_eq!(map.container_teams, Some([1, 0]), "{}", map.id);
            }
            let names = |language| {
                super::super::localization::of(&data, language)
                    .unwrap()
                    .unwrap()
                    .get(map.map_name_key)
                    .map(str::to_owned)
            };
            use super::super::localization::Language;
            assert_eq!(names(Language::English).as_deref(), Some("London"));
            assert_eq!(names(Language::Ukrainian).as_deref(), Some("Лондон"));
        }
    }

    /// Revision 17's balance change: the SVD (items_dict 63, the only item
    /// of `wpn_svd.options`) does 10 damage points less, 110 to 100: the
    /// game stores 1.1 and 1.0 (`parameters.bullet_damage`; the lobby's
    /// damage line, `ui_desc.props_list` prop 1, says 110 and 100). Every
    /// other weapon and ammo config is as the live data has it (the SVD's
    /// 7.62x54 ammo is shared and unchanged). srp reads no damage: the game
    /// computes it, and the server data carries the file.
    #[test]
    #[ignore = "needs 0.26g's resources.db, skhid's patches and revision 17's patch"]
    fn revision_17_svd_does_10_damage_points_less() {
        use super::super::dictionary::{self, Dictionary};
        let before = live_data(&[]);
        let after = live_data(&[env_path("SRP_TEST_PATCH_17")]);
        let dictionary = Dictionary::parse(after.file(dictionary::PATH).unwrap()).unwrap();
        let svd: Vec<_> = dictionary
            .items
            .iter()
            .filter(|item| item.cfg_name == "gameplay/items/weapons/wpn_svd.options")
            .collect();
        assert_eq!(
            svd.iter()
                .map(|item| (item.dict_id, item.name.as_str()))
                .collect::<Vec<_>>(),
            [(63, "svd")]
        );
        let damage = |data: &GameData| {
            let config =
                ConfigValue::parse(data.file("gameplay/items/weapons/wpn_svd.options").unwrap())
                    .unwrap();
            let shown = config
                .req("ui_desc")
                .unwrap()
                .req("props_list")
                .unwrap()
                .children()
                .into_iter()
                .find(|prop| prop.i32_at("prop_id").unwrap() == 1)
                .unwrap()
                .str_at("prop_value")
                .unwrap()
                .to_owned();
            (
                config
                    .req("parameters")
                    .unwrap()
                    .f32_at("bullet_damage")
                    .unwrap(),
                shown,
            )
        };
        assert_eq!(damage(&before), (1.1, "110".to_owned()));
        assert_eq!(damage(&after), (1.0, "100".to_owned()));
        for item in &dictionary.items {
            if item.cfg_name.starts_with("gameplay/items/weapons/") && item.dict_id != 63 {
                assert_eq!(
                    before.file(&item.cfg_name),
                    after.file(&item.cfg_name),
                    "{}",
                    item.cfg_name
                );
            }
        }
    }

    /// Weapon balance 1 (`scripts/content-edit/weapon-balance-1.json`, the
    /// patch at `SRP_TEST_PATCH_WEAPON_BALANCE_1` over the live data): each
    /// listed weapon's `parameters` value and the lobby card's text of it
    /// (`ui_desc.props_list`: prop 1 damage x100, 3 armour piercing x100, 4
    /// rate of fire) change as listed, and nothing else; the SVD's magazine
    /// holds 5 rounds, still an int, and its description says so in every
    /// language; every weapon card has a magazine line (prop 134, a new line
    /// of `ui_properties` named in every language) after its rate of fire;
    /// the items' misspelt
    /// `stamina_regenation_speed_modifier` is spelt as the exe reads it, with
    /// its value, in every item config and in the modifications' paths. srp
    /// reads only the magazine (the bots' guns); the game computes the rest,
    /// and the server data carries the files.
    #[test]
    #[ignore = "needs 0.26g's resources.db, skhid's patches and the weapon balance 1 patch"]
    fn weapon_balance_1_changes_only_its_values() {
        use super::super::dictionary::{self, Dictionary};
        const OLD: &str = "stamina_regenation_speed_modifier";
        const NEW: &str = "stamina_regeneration_speed_modifier";
        // (weapon, key, live, new); the card's text is the value x100, or
        // the rate of fire itself.
        let changes: &[(&str, &str, f32, f32)] = &[
            ("kriss_vector", "bullet_pierce", 0.45, 0.25),
            ("kriss_vector", "bullet_damage", 0.45, 0.42),
            ("fn_p90", "bullet_pierce", 0.55, 0.30),
            ("mp7", "bullet_pierce", 0.55, 0.30),
            ("mp7", "bullet_damage", 0.35, 0.40),
            ("mp7_silencer", "bullet_pierce", 0.55, 0.30),
            ("mp7_silencer", "bullet_damage", 0.35, 0.40),
            ("mp7_silencer_special", "bullet_pierce", 0.55, 0.30),
            ("mp7_silencer_special", "bullet_damage", 0.35, 0.40),
            ("hk416", "bullet_pierce", 0.45, 0.34),
            ("m4a1", "bullet_pierce", 0.42, 0.32),
            ("msbs_carbine", "bullet_pierce", 0.42, 0.32),
            ("fn_fnc", "bullet_pierce", 0.40, 0.30),
            ("fn_fnc", "bullet_damage", 0.45, 0.50),
            ("pkp_pecheneg", "bullet_pierce", 0.60, 0.45),
            ("vz58", "bullet_pierce", 0.30, 0.40),
            ("valmet", "bullet_pierce", 0.30, 0.40),
            ("soc_vepr", "bullet_pierce", 0.30, 0.40),
            ("kedr", "bullet_pierce", 0.10, 0.20),
            ("aek_919k", "bullet_pierce", 0.10, 0.20),
            ("sr2m", "rounds_per_minute", 900.0, 800.0),
            ("sr3m", "rounds_per_minute", 900.0, 800.0),
            ("sr3m", "bullet_damage", 0.55, 0.45),
            ("sr3m_premium", "rounds_per_minute", 900.0, 800.0),
            ("sr3m_premium", "bullet_damage", 0.55, 0.45),
            ("val", "bullet_damage", 0.55, 0.50),
            ("a9_91", "bullet_damage", 0.55, 0.50),
            ("vss", "bullet_damage", 0.55, 0.50),
            ("vss_premium", "bullet_damage", 0.55, 0.50),
            ("vsk_94", "bullet_damage", 0.55, 0.50),
            ("vsk_94_premium", "bullet_damage", 0.55, 0.50),
            ("pp2000", "rounds_per_minute", 750.0, 680.0),
            ("mosin_nagant", "bullet_damage", 1.0, 1.1),
            ("mosin_nagant", "bullet_pierce", 0.35, 0.40),
            ("toz_122", "bullet_damage", 1.1, 1.2),
            ("svd", "magazine_capacity", 10.0, 5.0),
        ];
        let prop = |key: &str| match key {
            "bullet_damage" => Some(1),
            "bullet_pierce" => Some(3),
            "rounds_per_minute" => Some(4),
            _ => None,
        };
        let shown = |key: &str, value: f32| {
            if key == "rounds_per_minute" {
                format!("{value}")
            } else {
                format!("{}", (value * 100.0).round())
            }
        };
        let before = live_data(&[]);
        let after = live_data(&[env_path("SRP_TEST_PATCH_WEAPON_BALANCE_1")]);
        let dictionary = Dictionary::parse(after.file(dictionary::PATH).unwrap()).unwrap();
        let mut renamed = 0;
        let mut magazines = 0;
        let mut seen = std::collections::BTreeSet::new();
        for item in &dictionary.items {
            let path = item.cfg_name.as_str();
            if !path.starts_with("gameplay/items/") || !seen.insert(path) {
                continue;
            }
            let (Some(old), Some(new)) = (before.file(path), after.file(path)) else {
                continue;
            };
            let old = ConfigValue::parse(old).unwrap();
            let new = ConfigValue::parse(new).unwrap();
            let name = path.rsplit('/').next().unwrap();
            let weapon = name
                .strip_prefix("wpn_")
                .and_then(|n| n.strip_suffix(".options"));
            let mine: Vec<_> = changes
                .iter()
                .filter(|change| Some(change.0) == weapon)
                .collect();
            for (key, value) in old.entries() {
                let mut value = value.clone();
                let mut text = None;
                if key == "parameters" {
                    let mut entries = value.entries().to_vec();
                    for (k, v) in &mut entries {
                        if k == OLD {
                            *k = NEW.into();
                            renamed += 1;
                        }
                        if let Some(change) = mine.iter().find(|change| change.1 == k.as_str()) {
                            assert_eq!(v.as_f32(), Some(change.2), "{path} {k}");
                            *v = ConfigValue::Float(change.3);
                        }
                    }
                    value = ConfigValue::Named(entries);
                    let got = new.req("parameters").unwrap();
                    for (k, v) in value.entries() {
                        let want = mine.iter().find(|change| change.1 == k.as_str());
                        let have = got.get(k).unwrap_or_else(|| panic!("{path} {k}"));
                        match want {
                            Some(change) => assert_eq!(have.as_f32(), Some(change.3), "{path} {k}"),
                            None => assert_eq!(have, v, "{path} {k}"),
                        }
                    }
                    assert_eq!(got.entries().len(), value.entries().len(), "{path}");
                    continue;
                }
                // A weapon card gets a magazine line (prop 134) after its
                // rate of fire, with the magazine the weapon has now.
                let magazine = new
                    .get("parameters")
                    .and_then(|p| p.get("magazine_capacity"))
                    .and_then(ConfigValue::as_f32);
                let lines = value.get("props_list").map(ConfigValue::children);
                let rate = lines.as_ref().and_then(|lines| {
                    lines
                        .iter()
                        .position(|p| p.i32_at("prop_id").ok() == Some(4))
                });
                if key == "ui_desc"
                    && let (Some(magazine), Some(rate)) = (magazine, rate)
                {
                    let mut entries = value.entries().to_vec();
                    for (k, v) in &mut entries {
                        if k == "props_list" {
                            let mut lines: Vec<ConfigValue> =
                                v.children().into_iter().cloned().collect();
                            lines.insert(
                                rate + 1,
                                ConfigValue::Named(vec![
                                    (
                                        "prop_value".into(),
                                        ConfigValue::String(format!("{magazine}")),
                                    ),
                                    ("prop_id".into(), ConfigValue::Integer(134)),
                                ]),
                            );
                            *v = ConfigValue::Indexed(lines);
                        }
                    }
                    value = ConfigValue::Named(entries);
                    magazines += 1;
                }
                if key == "ui_desc" && !mine.is_empty() {
                    let props = new.req("ui_desc").unwrap().req("props_list").unwrap();
                    text = Some(props.clone());
                    for change in mine.iter().filter(|change| prop(change.1).is_some()) {
                        let entry = props
                            .children()
                            .into_iter()
                            .find(|p| Some(p.i32_at("prop_id").unwrap()) == prop(change.1))
                            .unwrap();
                        assert_eq!(
                            entry.str_at("prop_value").unwrap(),
                            shown(change.1, change.3),
                            "{path} {}",
                            change.1
                        );
                    }
                }
                let got = new.get(key).unwrap_or_else(|| panic!("{path} {key}"));
                if let Some(props) = text {
                    // The card's other lines are the live ones.
                    let old_props = value.req("props_list").unwrap().children();
                    assert_eq!(old_props.len(), props.children().len(), "{path}");
                    for (o, n) in old_props.iter().zip(props.children()) {
                        let id = o.i32_at("prop_id").unwrap();
                        if !mine.iter().any(|change| prop(change.1) == Some(id)) {
                            assert_eq!(*o, n, "{path} prop {id}");
                        }
                    }
                } else {
                    assert_eq!(got, &value, "{path} {key}");
                }
            }
        }
        assert!(renamed > 200, "{renamed} items had the misspelt key");
        assert_eq!(magazines, 83, "weapon cards with a magazine line");
        // The card lines' table has the magazine line: the bullets-in-pack
        // line's keys (comparable, more is better), its own id, name and top.
        let lines = |data: &GameData| {
            let file = data
                .file("gameplay/static_game_parameters.options")
                .unwrap();
            let config = ConfigValue::parse(file).unwrap();
            config.req("ui_properties").unwrap().clone()
        };
        let (old_lines, new_lines) = (lines(&before), lines(&after));
        let (old_lines, new_lines) = (old_lines.children(), new_lines.children());
        assert_eq!(new_lines.len(), old_lines.len() + 1);
        assert!(old_lines.iter().zip(&new_lines).all(|(o, n)| o == n));
        let like = old_lines
            .iter()
            .find(|line| line.i32_at("prop_id").ok() == Some(16))
            .unwrap();
        let want: Vec<(String, ConfigValue)> = like
            .entries()
            .iter()
            .map(|(k, v)| {
                let v = match k.as_str() {
                    "prop_id" => ConfigValue::Integer(134),
                    "prop_name" => ConfigValue::String("st_inv_magazine_size".into()),
                    "min_value" => ConfigValue::String("0".into()),
                    "max_value" => ConfigValue::String("100".into()),
                    _ => v.clone(),
                };
                (k.clone(), v)
            })
            .collect();
        assert_eq!(new_lines.last().unwrap().entries(), want.as_slice());
        // The modifications and the defect that change it name the key as
        // the exe reads it.
        let walk = |config: &ConfigValue| -> Vec<String> {
            fn strings(value: &ConfigValue, out: &mut Vec<String>) {
                match value {
                    ConfigValue::String(s) => out.push(s.clone()),
                    _ => value.children().into_iter().for_each(|c| strings(c, out)),
                }
            }
            let mut out = Vec::new();
            strings(config, &mut out);
            out
        };
        let path = "gameplay/items/modifications/modifications.options";
        let old = walk(&ConfigValue::parse(before.file(path).unwrap()).unwrap());
        let new = walk(&ConfigValue::parse(after.file(path).unwrap()).unwrap());
        assert_eq!(old.iter().filter(|s| *s == OLD).count(), 6);
        assert_eq!(new.iter().filter(|s| *s == OLD).count(), 0);
        assert_eq!(new.iter().filter(|s| *s == NEW).count(), 6);
        let fixed: Vec<String> = old
            .iter()
            .map(|s| if s == OLD { NEW.into() } else { s.clone() })
            .collect();
        assert_eq!(fixed, new);
        // The SVD's magazine stays an int, as the bots' guns read it.
        let svd = ConfigValue::parse(
            after
                .file("gameplay/items/weapons/wpn_svd.options")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            svd.req("parameters").unwrap().get("magazine_capacity"),
            Some(&ConfigValue::Integer(5))
        );
        // Its description, in each language, says 5 rounds where it said 10;
        // every other text is the live one. The server data has no texts:
        // they are read in the patches (the live one is the newest patch's).
        // The texts are UTF-8 and the reader decodes Windows-1251, so the
        // words found are ASCII ones (the text compares the same either way).
        let read_in = |archive: &std::path::Path, path: &str| {
            let file = std::io::BufReader::new(std::fs::File::open(archive).unwrap());
            resources_db::Archive::new(file)
                .unwrap()
                .read(path)
                .unwrap()
        };
        let live: Vec<std::path::PathBuf> = std::iter::once(env_path("SRP_TEST_RESOURCES_DB"))
            .chain(
                std::env::var("SRP_TEST_SKHID_PATCHES")
                    .unwrap()
                    .split_whitespace()
                    .map(std::path::PathBuf::from),
            )
            .collect();
        // The card's magazine line is named in each language.
        for (language, from, to, label) in [
            (
                "english",
                "supply: 10 rounds",
                "supply: 5 rounds",
                "Magazine",
            ),
            ("german", "r 10 Patronen", "r 5 Patronen", "Magazin"),
            ("polish", "w 10-nabojowy", "w 5-nabojowy", "Magazynek"),
            ("russian", " 10 ", " 5 ", "Магазин"),
            ("original", " 10 ", " 5 ", "Магазин"),
            ("ukrainian", " 10 ", " 5 ", "Магазин"),
        ] {
            let path = format!("localization/{language}/localization");
            let old = live.iter().rev().find_map(|db| read_in(db, &path)).unwrap();
            let new = read_in(&env_path("SRP_TEST_PATCH_WEAPON_BALANCE_1"), &path).unwrap();
            let old = ConfigValue::parse(&old).unwrap();
            let new = ConfigValue::parse(&new).unwrap();
            let (old, new) = (old.req("strings").unwrap(), new.req("strings").unwrap());
            assert_eq!(old.entries().len() + 1, new.entries().len(), "{path}");
            let label = encoding_rs::WINDOWS_1251.decode(label.as_bytes()).0;
            assert_eq!(
                new.get("st_inv_magazine_size"),
                Some(&ConfigValue::String(label.into_owned())),
                "{path}"
            );
            for (key, value) in old.entries() {
                let got = new.get(key).unwrap_or_else(|| panic!("{path} {key}"));
                if key == "st_svd_description" {
                    let ConfigValue::String(text) = value else {
                        panic!("{path} {key}")
                    };
                    assert_eq!(text.matches(from).count(), 1, "{path}");
                    assert_eq!(got, &ConfigValue::String(text.replace(from, to)), "{path}");
                } else {
                    assert_eq!(got, value, "{path} {key}");
                }
            }
        }
    }
}
