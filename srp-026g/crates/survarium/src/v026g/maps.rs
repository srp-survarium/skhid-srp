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
use srp_validate::world::{Anomaly, Artefacts, Group, Held, Map, State};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::item_params::ItemClass;

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

/// Where `maps_dict`'s `cfg_mode_name` is.
pub const GAME_MODES_DIR: &str = "gameplay/game_modes/";

/// `mode.mode` of a config srp plays: the rules a match runs, what the
/// client and the simulation know it by. A config of another mode (0, the
/// engine's own `loopback_client` and `gather_server_resources`) has no
/// row ([`derive`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum RuleMode {
    /// Gather batteries ("Protective device"), rule class 88.
    Batteries = 2,
    /// Batteries that appear over time, with charging stations; rule 89.
    TimedBatteries = 3,
    /// Team Deathmatch with tickets; rule 90.
    TeamDeathmatch = 4,
    /// Research: three units to capture, the first team to 100 points
    /// wins; rule 480, exe patch 53 (`docs/notes/research_mode.md`).
    Research = 5,
    /// Free for all: each player his own side, the first to the kill target
    /// wins (`docs/notes/ffa.md`). No rule of its own: its configs are Team
    /// Deathmatch's without the tdm rule, on TDM's projects, whose team-2
    /// respawn points (content edit ffa-1) the free-for-all exe patch spawns
    /// each player at.
    FreeForAll = 8,
}

impl RuleMode {
    /// The modes a match can be.
    pub const ALL: [Self; 5] = [
        Self::Batteries,
        Self::TimedBatteries,
        Self::TeamDeathmatch,
        Self::Research,
        Self::FreeForAll,
    ];

    /// The mode of a config's `mode.mode`, if srp plays it.
    pub fn of_config(mode: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|rules| rules.number() == mode)
    }

    /// Its `mode.mode`: what a message to the client says.
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// Whether its maps have the teams' battery containers (2 and 3).
    pub const fn has_batteries(self) -> bool {
        matches!(self, Self::Batteries | Self::TimedBatteries)
    }

    /// The rule classes a map of it plays (`mode.rules`), in the order its
    /// world saves them: timelimit (93), respawn (91), its own rule (a
    /// free-for-all has none), kd stats (94). srp reads a world's rules section in that order alone
    /// (`srp_validate::world::rules`, snapshot_rules.md section 2), so a
    /// playable map with another list stops the data ([`MapMode::of`]).
    pub const fn rule_classes(self) -> &'static [u16] {
        match self {
            Self::Batteries => &[93, 91, 88, 94],
            Self::TimedBatteries => &[93, 91, 89, 94],
            Self::TeamDeathmatch => &[93, 91, 90, 94],
            Self::Research => &[93, 91, 480, 94],
            Self::FreeForAll => &[93, 91, 94],
        }
    }

    /// A playable map's `rules`: its [`rule_classes`](Self::rule_classes),
    /// else an error naming them.
    pub fn check_rules(self, rules: &[u16]) -> Result<(), ConfigError> {
        if rules == self.rule_classes() {
            return Ok(());
        }
        Err(ConfigError(format!(
            "its `rules` are {rules:?}: srp reads a mode {self} world's rules as {:?} \
             (srp_validate::world::rules), and would refuse every world of it",
            self.rule_classes()
        )))
    }
}

/// Its `mode.mode`, as the logs have always written it.
impl std::fmt::Display for RuleMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.number().fmt(f)
    }
}

/// The rules a Blackout match plays: the plain battery mode (no chargers).
pub const BLACKOUT_RULES: RuleMode = RuleMode::Batteries;

/// The mode a match is played in, as its map's config says: its rules
/// (`mode.mode`, [`RuleMode`]), or a variant of srp's of them
/// (`mode.srp_variant`, [`VARIANTS`]). A map is one mode: the lobby, the
/// match server and the results know it by the map's `maps_dict` id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameMode {
    Batteries,
    TimedBatteries,
    TeamDeathmatch,
    Research,
    FreeForAll,
    /// "Protective device: blackout" (`docs/notes/blackout.md`): the rules
    /// of [`BLACKOUT_RULES`], the static info's Blackout entry (exe patch
    /// 65) and its own pay.
    Blackout,
}

/// srp's variants of a config's rules, by `mode.srp_variant`, and the
/// rules each plays.
pub const VARIANTS: [(&str, GameMode); 1] = [("blackout", GameMode::Blackout)];

impl GameMode {
    /// A config's mode: `None` for rules srp does not play; an error for a
    /// variant srp does not know or one on other rules than its own.
    pub fn of_config(rules: u8, variant: Option<&str>) -> Result<Option<Self>, String> {
        let Some(variant) = variant else {
            return Ok(RuleMode::of_config(rules).map(Self::from));
        };
        match VARIANTS.iter().find(|(name, _)| *name == variant) {
            Some(&(_, mode)) if mode.rules().number() == rules => Ok(Some(mode)),
            Some(&(_, mode)) => Err(format!(
                "`srp_variant` {variant} plays mode {}, not {rules}",
                mode.rules()
            )),
            None => Err(format!("`srp_variant` {variant} is none of srp's")),
        }
    }

    /// The rules it plays.
    pub const fn rules(self) -> RuleMode {
        match self {
            Self::Batteries => RuleMode::Batteries,
            Self::TimedBatteries => RuleMode::TimedBatteries,
            Self::TeamDeathmatch => RuleMode::TeamDeathmatch,
            Self::Research => RuleMode::Research,
            Self::FreeForAll => RuleMode::FreeForAll,
            Self::Blackout => BLACKOUT_RULES,
        }
    }

    /// Its number in the lobby (the rooms' pool, cards and rows, the room
    /// settings' modes left out, `SRP_MATCH_MODES`): its rules', 6 for
    /// Blackout.
    pub const fn lobby_number(self) -> u8 {
        match self {
            Self::Blackout => 6,
            other => other.rules().number(),
        }
    }

    /// The mode of a lobby number.
    pub fn of_lobby_number(number: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.lobby_number() == number)
    }

    /// Every mode.
    pub const ALL: [Self; 6] = [
        Self::Batteries,
        Self::TimedBatteries,
        Self::TeamDeathmatch,
        Self::Research,
        Self::FreeForAll,
        Self::Blackout,
    ];
}

/// A config's rules with no variant.
impl From<RuleMode> for GameMode {
    fn from(rules: RuleMode) -> Self {
        match rules {
            RuleMode::Batteries => Self::Batteries,
            RuleMode::TimedBatteries => Self::TimedBatteries,
            RuleMode::TeamDeathmatch => Self::TeamDeathmatch,
            RuleMode::Research => Self::Research,
            RuleMode::FreeForAll => Self::FreeForAll,
        }
    }
}

/// A map a match can be played on ([`MapMode::is_playable`]): made only
/// by [`Self::of`], so whatever holds one need not check it again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchMap(&'static MapMode);

impl MatchMap {
    /// A `maps_dict` id, if a match can be played on it.
    pub fn of(map_id: u8) -> Option<Self> {
        map_mode(map_id).filter(|map| map.is_playable()).map(Self)
    }

    /// The map played: what the client loads.
    pub fn map(self) -> &'static MapMode {
        self.0
    }

    pub fn mode(self) -> GameMode {
        self.0.mode
    }
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
    /// `mode.mode` and srp's `mode.srp_variant` ([`GameMode::of_config`]).
    pub mode: GameMode,
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
    /// A free-for-all: the kills that win (`kills_to_win`, 30).
    pub kills_to_win: u32,
    /// `match_making_params.max_players`: the most a match of it takes. Only
    /// a free-for-all's is kept to (10; `docs/notes/ffa.md`).
    pub max_players: u32,
    /// How many respawn points the project has: every entry of its
    /// `server_objects.respawn_points`, whatever its team or layer, is
    /// loaded (`player_respawn_rule_cook` 0x2C6910), so this is how many
    /// pairs of bytes the respawn rule writes into a snapshot
    /// (snapshot_rules.md §2.3). PROVEN (code and data). 0 without a
    /// project.
    pub respawn_points: usize,
    /// Of those, the team-2 points: a free-for-all's (its exe patch spawns
    /// its players there and nowhere else).
    pub solo_respawn_points: usize,
    /// Modes 2 and 3: whose the two battery containers are, by container
    /// id: `victory_items_containers[..].team` of the project by its `id`.
    /// PROVEN (data). A battery's container id in a snapshot is this id,
    /// not the team. `None` in modes 4 and 5 and without a project.
    pub container_teams: Option<[u8; 2]>,
    /// The map's objects a world saves, in the game's order
    /// ([`world_objects`]): what the server reads an uploaded world's game
    /// objects with. None without a project.
    pub world_objects: &'static Map<'static>,
}

impl MapMode {
    /// Whether a match can be played on it: on a map whose project the
    /// game data has. Its config is there and of a mode srp plays, or it
    /// would have no row.
    pub fn is_playable(&self) -> bool {
        self.has_project
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
/// server data's cut of it: [`resources_db::server::objects_of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectObjects {
    /// [`MapMode::respawn_points`].
    pub respawn_points: usize,
    /// [`MapMode::solo_respawn_points`].
    pub solo_respawn_points: usize,
    /// The battery containers' teams by container id, if the project has
    /// containers ([`MapMode::container_teams`]).
    pub container_teams: Option<[u8; 2]>,
    /// [`MapMode::world_objects`].
    pub world_objects: &'static Map<'static>,
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
    pub fn from_project(
        project: &ConfigValue,
        with_batteries: bool,
        data: &GameData,
        dictionary: &Dictionary,
    ) -> Result<Self, ConfigError> {
        let objects = project.get("server_objects");
        let points = objects
            .and_then(|objects| objects.get("respawn_points"))
            .map(ConfigValue::children)
            .unwrap_or_default();
        let respawn_points = points.len();
        let solo_respawn_points = points
            .iter()
            .filter(|point| point.get("team").and_then(ConfigValue::as_i32) == Some(2))
            .count();
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
            solo_respawn_points,
            container_teams,
            world_objects: world_objects(project, data, dictionary)?,
        })
    }
}

/// A project of no objects a world saves.
pub static NO_WORLD_OBJECTS: Map<'static> = Map {
    damage_zones: 0,
    resupply_zones: 0,
    anomalies: &[],
};

/// The objects a world of the project saves after its places' grenades and
/// traps, in the game's order (`srp_validate::world::objects`): its
/// `client_objects`' damage zones that no anomaly's group names, its
/// resupply zones and its anomalies, each kind in the project's order
/// (`simple_game_project::insert` 0x285750). An anomaly with
/// `artefacts_enabled` has its `artefact_containers` and `artefacts[].count`
/// artefacts; its `states` are in the game's order: the order of
/// `generic_anomaly_core::resolve_links` 0x33C590, an insertion sort with
/// `state_prio` 0x33C400 ([`state_before`]; the sort the game takes for at
/// most 16 states, and a project has at most 7). An artefact saves its
/// amount as a stack's u16 (`inventory_item::deserialize` 0x2FFF10 by the
/// dictionary's `is_stack`, as every map's are), and an active one what its
/// item's core saves (`data.type`, [`ItemClass::Artefact`]): one of no stack
/// or of no artefact's class stops the data. Leaked: the rows live as long as
/// the process.
pub fn world_objects(
    project: &ConfigValue,
    data: &GameData,
    dictionary: &Dictionary,
) -> Result<&'static Map<'static>, ConfigError> {
    let objects = project
        .get(resources_db::server::CLIENT_OBJECTS)
        .map(ConfigValue::children)
        .unwrap_or_default();
    let of = |kind: &'static str| {
        objects.iter().copied().filter(move |object| {
            object.get("game_object_type").and_then(ConfigValue::as_str) == Some(kind)
        })
    };
    let flag = |table: &ConfigValue, key: &str| -> Result<bool, ConfigError> {
        match table.get(key) {
            None => Ok(false),
            Some(value) => value
                .as_bool()
                .ok_or_else(|| ConfigError(format!("`{key}` is not a bool"))),
        }
    };
    let mut driven = std::collections::BTreeSet::new();
    let mut anomalies = Vec::new();
    for anomaly in of("generic_anomaly_core") {
        let name = anomaly.get("name").and_then(ConfigValue::as_str).unwrap_or("");
        let mut states = Vec::new();
        for state in anomaly.get("states").map(ConfigValue::children).unwrap_or_default() {
            let mut groups = Vec::new();
            for group in state.get("groups").map(ConfigValue::children).unwrap_or_default() {
                let zones = group.get("zones").map(ConfigValue::children).unwrap_or_default();
                for zone in &zones {
                    let zone = zone.as_str().ok_or_else(|| {
                        ConfigError(format!("anomaly {name}: a group's zone is not a name"))
                    })?;
                    driven.insert(zone);
                }
                let zones = zones.len();
                groups.push(if flag(group, "keep_all_zones_active")? {
                    Group::KeepAll { zones }
                } else {
                    Group::Switched { zones }
                });
            }
            let order = StateOrder {
                select_priority: int(Some(state), "select_priority")?,
                energy_threshold: int(Some(state), "energy_threshold")?,
                shoot_trigger: flag(state, "shoot_trigger")?,
                zone_activity_trigger: flag(state, "zone_activity_trigger")?,
            };
            states.push((order, State { groups: Box::leak(groups.into_boxed_slice()) }));
        }
        if states.len() > MAX_SORTED_STATES {
            return Err(ConfigError(format!(
                "anomaly {name} has {} states: the game sorts more than {MAX_SORTED_STATES} \
                 otherwise, which srp does not know",
                states.len()
            )));
        }
        insertion_sort(&mut states);
        let artefacts = if flag(anomaly, "artefacts_enabled")? {
            let mut artefacts = Vec::new();
            for artefact in anomaly.get("artefacts").map(ConfigValue::children).unwrap_or_default() {
                let dict_id: u16 = int(Some(artefact), "dict_id")?;
                let item = dictionary.item(dict_id).filter(|item| item.is_stack).ok_or_else(|| {
                    ConfigError(format!(
                        "anomaly {name}: artefact {dict_id} is no stack of the dictionary"
                    ))
                })?;
                let config = super::game_data::config(data, &item.cfg_name)
                    .map_err(|cause| ConfigError(cause.0))?
                    .ok_or_else(|| {
                        ConfigError(format!("anomaly {name}: artefact {dict_id} has no config"))
                    })?;
                let ItemClass::Artefact(core) = ItemClass::from_config(&config)? else {
                    return Err(ConfigError(format!(
                        "anomaly {name}: item {dict_id} is no artefact"
                    )));
                };
                let count = int::<usize>(Some(artefact), "count")?;
                artefacts.extend(std::iter::repeat_n(core, count));
            }
            Some(Artefacts {
                containers: anomaly
                    .get("artefact_containers")
                    .map_or(0, |containers| containers.children().len()),
                artefacts: Box::leak(artefacts.into_boxed_slice()),
            })
        } else {
            None
        };
        let states: Vec<State> = states.into_iter().map(|(_, state)| state).collect();
        anomalies.push(Anomaly {
            artefacts,
            states: Box::leak(states.into_boxed_slice()),
        });
    }
    let damage_zones = of("damage_zone")
        .filter(|zone| {
            let name = zone.get("full_name").and_then(ConfigValue::as_str).unwrap_or("");
            !driven.contains(name)
        })
        .count();
    if damage_zones == 0 && anomalies.is_empty() && of("resupply_zone").next().is_none() {
        return Ok(&NO_WORLD_OBJECTS);
    }
    Ok(Box::leak(Box::new(Map {
        damage_zones,
        resupply_zones: of("resupply_zone").count(),
        anomalies: Box::leak(anomalies.into_boxed_slice()),
    })))
}

/// A world's game objects when nothing in them was touched, as a game
/// saves them at its start: each of `held` (the places' grenades and
/// traps, `srp_validate::world::Objects`) one 0 byte (`grenade_core::
/// serialize` 0x303DA0, `booby_trap_core::serialize` 0x3052E0), then the
/// map's: a damage zone's 5 bytes, a resupply zone with no user, an
/// anomaly with nothing pending, no energy, no state, and its containers
/// empty and its artefacts free. For tests and scripted games.
pub fn untouched_objects(held: impl IntoIterator<Item = Held>, map: &Map) -> Vec<u8> {
    let mut bytes: Vec<u8> = held.into_iter().map(|_| 0).collect();
    bytes.extend(std::iter::repeat_n(0, map.damage_zones * 5 + map.resupply_zones));
    for anomaly in map.anomalies {
        if anomaly.artefacts.is_some() {
            bytes.push(0);
        }
        bytes.extend(0f32.to_le_bytes());
        bytes.push(0xFF);
        bytes.extend([0; 5]);
        if let Some(artefacts) = anomaly.artefacts {
            for _ in 0..artefacts.containers {
                bytes.extend([0, 0xFF, 0, 0, 0, 0]);
            }
            for _ in artefacts.artefacts {
                bytes.extend([0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            }
        }
    }
    bytes
}

/// The game sorts an anomaly's states by insertion up to this many
/// (STLport's threshold of 0x40 bytes of four-byte pointers, 0x33C6C1).
const MAX_SORTED_STATES: usize = 16;

/// What `state_prio` 0x33C400 compares of a state.
#[derive(Clone, Copy, Debug)]
struct StateOrder {
    select_priority: i32,
    energy_threshold: i32,
    shoot_trigger: bool,
    zone_activity_trigger: bool,
}

/// `state_prio(a, b)` 0x33C400: whether `a` goes before `b`. The higher
/// priority first, then the higher energy threshold; at a tie of both,
/// `a`'s shoot trigger when the two differ, else `a`'s zone activity
/// trigger (so not a strict order: kept exactly).
fn state_before(a: &StateOrder, b: &StateOrder) -> bool {
    if a.select_priority != b.select_priority {
        a.select_priority > b.select_priority
    } else if a.energy_threshold != b.energy_threshold {
        a.energy_threshold > b.energy_threshold
    } else if a.shoot_trigger != b.shoot_trigger {
        a.shoot_trigger
    } else {
        a.zone_activity_trigger
    }
}

/// STLport's `__insertion_sort` with [`state_before`]: an element before
/// the first goes to the front, any other moves down while it goes before
/// the one ahead of it (`__unguarded_linear_insert`).
fn insertion_sort<T>(states: &mut [(StateOrder, T)]) {
    for next in 1..states.len() {
        if state_before(&states[next].0, &states[0].0) {
            states[..=next].rotate_right(1);
        } else {
            let mut at = next;
            // It stops at 1 at the latest: it does not go before the first.
            while state_before(&states[at].0, &states[at - 1].0) {
                states.swap(at, at - 1);
                at -= 1;
            }
        }
    }
}

impl MapMode {
    /// The row of a `maps_dict` id's config (`gameplay/game_modes/`
    /// `cfg_mode_name`) and what the server reads of its project, if the
    /// data has the project ([`ProjectObjects::from_project`]). A playable
    /// id ([`MapMode::is_playable`]) must have a respawn point, one of
    /// modes 2 and 3 the two teams' battery containers, and one of mode 5
    /// the points that win; a config of a mode srp does not play has none
    /// (`None`). The strings are leaked: the rows live as long as the
    /// process.
    pub fn from_config(
        id: u8,
        cfg_mode_name: &str,
        config: &ConfigValue,
        project: Option<&ConfigValue>,
        data: &GameData,
        dictionary: &Dictionary,
    ) -> Result<Option<Self>, ConfigError> {
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
        let variant = Some(text(Some(mode), "srp_variant")?).filter(|variant| !variant.is_empty());
        let Some(game_mode) =
            GameMode::of_config(int(Some(mode), "mode")?, variant).map_err(ConfigError)?
        else {
            return Ok(None);
        };
        let rule_mode = game_mode.rules();
        let objects = project
            .map(|project| {
                ProjectObjects::from_project(project, rule_mode.has_batteries(), data, dictionary)
            })
            .transpose()?
            .unwrap_or(ProjectObjects {
                respawn_points: 0,
                solo_respawn_points: 0,
                container_teams: None,
                world_objects: &NO_WORLD_OBJECTS,
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
            kills_to_win: int(p, "kills_to_win")?,
            max_players: int(making, "max_players")?,
            respawn_points: objects.respawn_points,
            solo_respawn_points: objects.solo_respawn_points,
            container_teams: objects
                .container_teams
                .filter(|_| rule_mode.has_batteries()),
            world_objects: objects.world_objects,
        };
        if row.is_playable() {
            if row.respawn_points == 0 {
                return Err(ConfigError(format!(
                    "its project {} has no respawn points",
                    row.project
                )));
            }
            if rule_mode.has_batteries() && row.container_teams.is_none() {
                return Err(ConfigError(format!(
                    "its project {} has no battery containers, and it is mode {}",
                    row.project, rule_mode
                )));
            }
            if rule_mode == RuleMode::Research && row.resources_count == 0 {
                return Err(ConfigError(
                    "it is mode 5 and has no `resources_count`".to_owned(),
                ));
            }
            if rule_mode == RuleMode::FreeForAll
                && (row.kills_to_win == 0 || row.solo_respawn_points == 0 || row.max_players == 0)
            {
                return Err(ConfigError(format!(
                    "it is a free-for-all with {} kills to win, {} players at most and {} team-2 \
                     respawn points in {}",
                    row.kills_to_win,
                    row.max_players,
                    row.solo_respawn_points,
                    row.project
                )));
            }
            rule_mode.check_rules(row.rules)?;
        }
        Ok(Some(row))
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
/// ([`MapMode::from_config`]). An id whose config the game lacks, or whose
/// config is of a mode srp does not play, is left out, and so never sent
/// (114 and 143..=151 of 0.26g; 141 and 142, mode 0).
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
        let row = MapMode::from_config(
            entry.id,
            &entry.cfg_mode_name,
            &config,
            project.as_ref(),
            data,
            dictionary,
        )
            .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?;
        rows.extend(row);
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
pub fn playable_in(mode: GameMode) -> impl Iterator<Item = &'static MapMode> {
    playable().filter(move |map| map.mode == mode)
}

/// A free-for-all's side of roster place `place`: a team of its own, 4 +
/// the place (`docs/notes/ffa.md`). The game reads team 2 as "neutral" and 3
/// as "none", and the free-for-all exe patch lets a team of 2 or more respawn
/// and resupply.
pub fn free_for_all_team(place: usize) -> u8 {
    srp_validate::own_side(place)
}

/// The most players a match on `map` takes, the server's bots among them: a
/// free-for-all's config's `max_players` (10); any other mode's none of its
/// own.
pub fn player_cap(map: &MapMode) -> Option<usize> {
    (map.mode == GameMode::FreeForAll).then_some(map.max_players as usize)
}

/// The id for a level, weather and mode, if it can be played.
pub fn find(level: u8, weather: u8, mode: GameMode) -> Option<&'static MapMode> {
    playable().find(|map| (map.level, map.weather, map.mode) == (level, weather, mode))
}

#[cfg(test)]
mod tests {
    use super::super::dictionary::Dictionary;
    use srp_validate::world::Artefact;
    use super::*;

    fn ids(mode: GameMode) -> Vec<u8> {
        playable_in(mode).map(|map| map.id).collect()
    }

    #[test]
    fn blackout_is_a_mode_of_its_own_maps() {
        // The test data's 24 battery maps, each once more as Blackout
        // (`scripts/content-edit/blackout-mode-1.json`: ids from 219).
        let blackout: Vec<&MapMode> = playable_in(GameMode::Blackout).collect();
        assert_eq!(blackout.len(), playable_in(GameMode::Batteries).count());
        let ids: Vec<u8> = blackout.iter().map(|map| map.id).collect();
        assert_eq!(ids, (219..=242).collect::<Vec<u8>>());
        for map in blackout {
            let twin = find(map.level, map.weather, GameMode::Batteries).unwrap();
            assert_eq!(map.config, format!("blackout/{}", twin.config));
            assert_eq!((map.project, map.rules), (twin.project, twin.rules));
            assert_eq!(map.mode.rules(), BLACKOUT_RULES);
            assert_eq!(map.mode.lobby_number(), 6);
            assert_eq!(map.mode_name_key, "st_srp_mode_blackout");
            assert_eq!(map.container_teams, twin.container_teams);
            assert_eq!(MatchMap::of(map.id).unwrap().mode(), GameMode::Blackout);
        }
        assert_eq!(MatchMap::of(11), None, "a test map the client cannot load");
    }

    #[test]
    fn a_configs_mode_is_its_rules_or_a_variant_of_them() {
        assert_eq!(GameMode::of_config(4, None), Ok(Some(GameMode::TeamDeathmatch)));
        assert_eq!(GameMode::of_config(2, Some("blackout")), Ok(Some(GameMode::Blackout)));
        // Rules srp does not play: no mode.
        assert_eq!(GameMode::of_config(0, None), Ok(None));
        // A variant of other rules (played or not), or one srp does not know:
        // refused.
        assert!(GameMode::of_config(0, Some("blackout")).is_err());
        assert!(GameMode::of_config(4, Some("blackout")).is_err());
        assert!(GameMode::of_config(2, Some("dark")).is_err());
        for mode in GameMode::ALL {
            assert_eq!(GameMode::of_lobby_number(mode.lobby_number()), Some(mode));
        }
    }

    #[test]
    fn the_whitelist_is_that_of_the_notes() {
        // modes_maps.md §8.
        let mode_2: Vec<u8> = (61..=105).step_by(2).chain([109]).collect();
        let mode_3: Vec<u8> = (62..=106).step_by(2).chain([110]).collect();
        let mode_4: Vec<u8> = (117..=140).collect();
        assert_eq!(ids(GameMode::Batteries), mode_2);
        assert_eq!(ids(GameMode::TimedBatteries), mode_3);
        assert_eq!(ids(GameMode::TeamDeathmatch), mode_4);
        // And Blackout's 24, one for each battery map, and a free-for-all
        // for each Team Deathmatch map (`ffa_ids`).
        assert_eq!(ids(GameMode::FreeForAll), ffa_ids());
        assert_eq!(playable().count(), 72 + 24 + 24);
        for bad in [11, 45, 107, 108, 114, 115].into_iter().chain(141..=151) {
            assert!(playable().all(|map| map.id != bad), "{bad}");
        }
        // 114 and 143..=151 have no config at all; 141 and 142 are of mode 0.
        for missing in [114, 141, 142].into_iter().chain(143..=151) {
            assert!(map_mode(missing).is_none(), "{missing}");
        }
        // 15 ids a level: 3 weathers × 3 modes, and in each weather Blackout
        // and a free-for-all.
        for level in 1..=8 {
            assert_eq!(playable().filter(|map| map.level == level).count(), 15);
            for mode in [GameMode::Blackout, GameMode::FreeForAll] {
                let of_mode = playable_in(mode).filter(|map| map.level == level).count();
                assert_eq!(of_mode, 3, "level {level}, {mode:?}");
            }
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
        // The ids without a row: the ten whose config the db lacks, and
        // 141 and 142, of mode 0.
        let missing: Vec<u8> = dictionary
            .maps
            .iter()
            .map(|entry| entry.id)
            .filter(|&id| map_mode(id).is_none())
            .collect();
        assert_eq!(missing, [114, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151]);
        // A playable id's config name spells its level, weather and mode,
        // and its project is `level_0X/<weather>_<mode>` (Blackout's: its
        // battery map's, `blackout_is_a_mode_of_its_own_maps`; a
        // free-for-all's: its own copy of its Team Deathmatch map's,
        // `the_free_for_alls_play_on_their_own_projects`).
        for map in playable()
            .filter(|map| !matches!(map.mode, GameMode::Blackout | GameMode::FreeForAll))
        {
            let weather = match map.weather {
                weather::DAY => "day",
                weather::RAIN => "rain",
                weather::EVENING => "evn",
                weather::PRERAIN => "prerain",
                other => panic!("{} has weather {other}", map.id),
            };
            let config = format!("level_0{}_{weather}_{}.options", map.level, map.mode.rules());
            assert_eq!(map.config, config);
            let long = if weather == "evn" { "evening" } else { weather };
            let project = format!("level_0{}/{long}_{}", map.level, map.mode.rules());
            assert_eq!(map.project, project);
            assert!(map.project.len() <= 31);
        }
    }

    #[test]
    fn spot_checks() {
        let map = map_mode(61).unwrap();
        assert_eq!((map.level, map.weather, map.mode), (1, weather::DAY, GameMode::Batteries));
        assert_eq!(map.project, "level_01/day_2");
        assert_eq!(map.rules, [93, 91, 88, 94]);
        let map = map_mode(109).unwrap();
        assert_eq!((map.level, map.weather, map.mode), (8, weather::PRERAIN, GameMode::Batteries));
        assert_eq!(map.loading_screen_id, 25);
        assert_eq!(find(8, weather::PRERAIN, GameMode::TeamDeathmatch).unwrap().id, 140);
        assert_eq!(find(1, weather::EVENING, GameMode::TimedBatteries).unwrap().id, 64);
        assert_eq!(find(4, weather::RAIN, GameMode::TeamDeathmatch).unwrap().id, 128);
        // Level 8 has no rain, and its night has no project.
        assert!(find(8, weather::RAIN, GameMode::Batteries).is_none());
        assert!(find(8, weather::NIGHT, GameMode::Batteries).is_none());
        assert!(!map_mode(107).unwrap().has_project);
        // Loading screens, static_info.md §4.
        assert_eq!(find(5, weather::EVENING, GameMode::Batteries).unwrap().loading_screen_id, 14);
        assert_eq!(find(7, weather::DAY, GameMode::TeamDeathmatch).unwrap().loading_screen_id, 18);
    }

    /// A map whose rules srp would read its worlds otherwise than its game
    /// writes them stops the data, by name.
    #[test]
    fn a_mode_with_rules_other_than_the_world_check_reads_stops_the_data() {
        for mode in RuleMode::ALL {
            assert!(mode.check_rules(mode.rule_classes()).is_ok());
        }
        let wrong = RuleMode::TeamDeathmatch.check_rules(&[93, 91, 90, 94, 500]).unwrap_err();
        assert!(wrong.0.contains("[93, 91, 90, 94, 500]"), "{}", wrong.0);
        assert!(RuleMode::Research.check_rules(&[93, 91, 90, 94]).is_err());
    }

    #[test]
    fn every_playable_mode_has_the_same_timings() {
        for map in playable() {
            assert_eq!(map.match_time, 900, "{}", map.id);
            assert_eq!(map.wait_time, 5, "{}", map.id);
            assert_eq!(map.countdown_time, 20, "{}", map.id);
            assert_eq!(map.wait_player_percent, 0.8, "{}", map.id);
            assert_eq!(map.respawn_time, 8, "{}", map.id);
            let (rules, icon): (&[u16], u8) = match map.mode.rules() {
                RuleMode::Batteries => (&[93, 91, 88, 94], 0),
                RuleMode::TimedBatteries => (&[93, 91, 89, 94], 0),
                RuleMode::TeamDeathmatch => (&[93, 91, 90, 94], 2),
                RuleMode::FreeForAll => (&[93, 91, 94], 2),
                RuleMode::Research => panic!("the test data has no Research"),
            };
            assert_eq!(map.rules, rules, "{}", map.id);
            assert_eq!(map.icon, icon, "{}", map.id);
            match map.mode {
                // A free-for-all's config is its Team Deathmatch's: the
                // tickets stay, with no rule to count them.
                GameMode::TeamDeathmatch | GameMode::FreeForAll => {
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
            if map.mode == GameMode::TimedBatteries {
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

    /// The free-for-all points content edit ffa-1 adds to each level's Team
    /// Deathmatch projects (`scripts/ffa-mode/checked.tsv`: 0.31a2's that
    /// stand on 0.26g's ground).
    const FFA_POINTS: [usize; 9] = [28, 25, 24, 30, 24, 25, 29, 23, 22];

    /// Every playable row of `table` of a level of 1 to 9 has what srp
    /// had by hand; `free_for_all`: its Team Deathmatch projects have the
    /// free-for-all's points too.
    fn as_before(table: &[MapMode], free_for_all: bool) {
        for map in table.iter().filter(|map| map.is_playable()) {
            let level = usize::from(map.level);
            assert!((1..=9).contains(&level), "{}", map.id);
            let added = if free_for_all { FFA_POINTS[level - 1] } else { 0 };
            let (points, teams) = match map.mode {
                // Team Deathmatch's project as it was; a free-for-all's
                // own copy of it holds the free-for-all points.
                GameMode::TeamDeathmatch => (TDM_BEFORE[level - 1], None),
                GameMode::FreeForAll => (TDM_BEFORE[level - 1] + added, None),
                _ => (GATHER_BEFORE[level - 1], Some(teams_before(map.level))),
            };
            assert_eq!(map.respawn_points, points, "{} ({})", map.id, map.project);
            assert_eq!(map.container_teams, teams, "{} ({})", map.id, map.project);
        }
    }

    #[test]
    fn the_projects_give_what_srp_had_by_hand() {
        // The test data: 0.26g's maps, levels 1 to 8, all 72, Blackout's 24
        // and the free-for-alls' 24.
        assert_eq!(playable().count(), 72 + 24 + 24);
        as_before(all(), true);
        // Spot checks: level_04's Team Deathmatch has 75 points of its own
        // (39 of layer `tdm`), level_02's 31 (one of team 2), and the
        // free-for-all's own copy of each those and its own.
        let level_04 = find(4, weather::RAIN, GameMode::TeamDeathmatch).unwrap();
        assert_eq!((level_04.respawn_points, level_04.solo_respawn_points), (75, 0));
        let level_04 = find(4, weather::RAIN, GameMode::FreeForAll).unwrap();
        assert_eq!((level_04.respawn_points, level_04.solo_respawn_points), (75 + 30, 30));
        let level_02 = find(2, weather::DAY, GameMode::TeamDeathmatch).unwrap();
        assert_eq!((level_02.respawn_points, level_02.solo_respawn_points), (31, 1));
        let level_02 = find(2, weather::DAY, GameMode::FreeForAll).unwrap();
        assert_eq!((level_02.respawn_points, level_02.solo_respawn_points), (31 + 25, 1 + 25));
        assert_eq!(
            find(7, weather::DAY, GameMode::TimedBatteries).unwrap().container_teams,
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
            (12, GameMode::TeamDeathmatch, "level_12/day_4")
        );
        // Its respawn points are its project's, and it has no containers.
        assert_eq!(map.respawn_points, 17);
        assert_eq!(map.container_teams, None);
        // It is played in mode 4 only.
        let playable_in = |game_mode: GameMode| -> Vec<u8> {
            table
                .iter()
                .filter(|map| map.is_playable() && map.mode == game_mode)
                .map(|map| map.id)
                .collect()
        };
        assert!(playable_in(GameMode::TeamDeathmatch).contains(&added.id));
        assert_eq!(playable_in(GameMode::TeamDeathmatch).len(), 25);
        for game_mode in [GameMode::Batteries, GameMode::TimedBatteries] {
            assert!(!playable_in(game_mode).contains(&added.id));
            assert_eq!(playable_in(game_mode), ids(game_mode));
        }
        assert_eq!(table.iter().filter(|map| map.is_playable()).count(), 73 + 24 + 24);
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
            (12, GameMode::Research, "level_12/day_5")
        );
        assert_eq!(map.rules, [93, 91, 480, 94]);
        assert_eq!((map.respawn_points, map.container_teams), (14, None));
        assert_eq!((map.resources_count, map.tickets_count), (100, 0));
        assert!(!map.mode.rules().has_batteries());
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
            mode: RuleMode::Batteries.number(),
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
        // A config of a mode srp does not play has no row, whatever its
        // project.
        let other = fixtures::AddedMap {
            mode: 6,
            respawn_points: 0,
            ..fixtures::AddedMap::level_12_tdm()
        };
        let table = fixtures::table_of(&fixtures::with_map(&other)).unwrap();
        assert!(table.iter().all(|map| map.id != 200));
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

    /// The free-for-alls' ids (content edit ffa-1) the test data has: levels
    /// 1 to 8 in their three weathers (London's, 163, is a patch's).
    fn ffa_ids() -> Vec<u8> {
        (46..=60).chain(154..=162).collect()
    }

    /// Each of `table`'s `count` free-for-alls is playable on its own copy
    /// of its level's Team Deathmatch project (`<weather>_4` -> `<weather>_8`,
    /// content edit ffa-projects-1), which holds its team-2 points, with the
    /// owner's kill target and the most players.
    fn free_for_alls_play_on_their_own_projects(table: &[MapMode], count: usize) {
        let ffa: Vec<&MapMode> = table
            .iter()
            .filter(|map| map.mode == GameMode::FreeForAll)
            .collect();
        assert_eq!(ffa.len(), count);
        for map in ffa {
            assert_eq!(map.mode.rules().number(), 8);
            assert_eq!(map.mode.lobby_number(), 8);
            assert!(map.config.starts_with("ffa/"), "{}", map.config);
            assert!(map.is_playable(), "{}", map.config);
            // The STALKER maps' Team Deathmatch is not offered (no dictionary
            // row: stalker-tdm-hold-1): nothing to compare with.
            let Some(tdm) = table.iter().find(|other| {
                other.mode == GameMode::TeamDeathmatch
                    && (other.level, other.weather) == (map.level, map.weather)
            }) else {
                continue;
            };
            assert_eq!(
                map.config,
                format!("ffa/{}8.options", tdm.config.strip_suffix("4.options").unwrap())
            );
            let copy = format!("{}_8", tdm.project.strip_suffix("_4").unwrap());
            assert_eq!(map.project, copy);
            // The copy has TDM's points and its own team-2 points; TDM's
            // project has none of them.
            assert_eq!(
                map.respawn_points - map.solo_respawn_points,
                tdm.respawn_points - tdm.solo_respawn_points
            );
            assert!(tdm.solo_respawn_points < map.solo_respawn_points, "{}", tdm.project);
            assert!(map.solo_respawn_points >= 20, "{} {}", map.config, map.solo_respawn_points);
            assert_eq!((map.kills_to_win, map.match_time), (30, tdm.match_time));
            assert_eq!(player_cap(map), Some(10));
            assert_eq!(map.rules, &[93, 91, 94]);
            assert_eq!(map.mode_name_key, "st_mode_08_name");
        }
    }

    /// The test data's, made of content edit ffa-1 as the content patch
    /// makes them (`game_data::test_archive`).
    #[test]
    fn the_free_for_alls_play_on_their_own_projects() {
        free_for_alls_play_on_their_own_projects(all(), 24);
    }

    /// Built data with both FFA edit lists, including London and all five ports.
    #[test]
    #[ignore = "needs SRP_TEST_GAME_DATA from build-content with both FFA edit lists"]
    fn the_released_free_for_alls_play_on_their_own_projects() {
        let data = GameData::open(&env_path("SRP_TEST_GAME_DATA")).unwrap();
        let maps = fixtures::table_of(&data).unwrap();
        free_for_alls_play_on_their_own_projects(&maps, 33);
        let ports: Vec<_> = maps.iter()
            .filter(|m| m.mode == GameMode::FreeForAll && m.level >= 10)
            .map(|m| (m.id, m.level, m.solo_respawn_points))
            .collect();
        assert_eq!(ports, [
            (42, 15, 24), (43, 16, 24), (44, 17, 24),
            (164, 10, 22), (165, 11, 22), (167, 12, 22), (168, 13, 22), (169, 14, 22)
        ]);
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
                map.mode.lobby_number(),
                map.project,
                map.respawn_points,
                map.container_teams
            );
        }
        // The stock maps are as in the test data, and as before.
        for map in table.iter().filter(|map| stock.contains(&map.id)) {
            assert_eq!(Some(map), map_mode(map.id), "{}", map.id);
        }
        as_before(&table, false);
        let london: Vec<&&MapMode> = played.iter().filter(|map| map.level == 9).collect();
        assert_eq!(london.len(), 3);
        for map in london {
            assert_eq!(map.respawn_points, 20, "{}", map.id);
            if map.mode != GameMode::TeamDeathmatch {
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

    #[test]
    fn a_maps_world_objects_are_its_lone_damage_zones_resupplies_and_anomalies() {
        // level_01/day_2: its two damage zones are its anomalies' own.
        let map = map_mode(61).unwrap();
        assert_eq!(map.project, "level_01/day_2");
        let objects = map.world_objects;
        assert_eq!((objects.damage_zones, objects.resupply_zones), (0, 2));
        let artefacts: Vec<_> = objects
            .anomalies
            .iter()
            .map(|a| a.artefacts.map(|a| (a.containers, a.artefacts.len())))
            .collect();
        assert_eq!(artefacts, [Some((11, 10)), Some((5, 10))]);
        assert_eq!(objects.artefacts(), 20);
        // Each of its item's core (an active one saves its own bytes).
        let cores: Vec<_> = objects
            .anomalies
            .iter()
            .map(|a| a.artefacts.unwrap().artefacts)
            .collect();
        assert_eq!(cores, [[Artefact::Lifebone; 10], [Artefact::Spring; 10]]);
        // level_02/evening_2: three lone zones, and the fruit anomaly's
        // seven states.
        let map = map_mode(69).unwrap();
        assert_eq!(map.project, "level_02/evening_2");
        assert_eq!(map.world_objects.damage_zones, 3);
        let states: Vec<usize> = map.world_objects.anomalies.iter().map(|a| a.states.len()).collect();
        assert_eq!(states, [1, 7]);
        // Every map's objects read, and an untouched world of them reads
        // to its end.
        for map in all().iter().filter(|map| map.has_project) {
            let objects = srp_validate::world::Objects {
                held: &[Held::Grenade, Held::Trap],
                map: map.world_objects,
            };
            let bytes = untouched_objects(objects.held.iter().copied(), map.world_objects);
            let context = srp_validate::world::Context {
                time: 0,
                places: 1,
                game_mode: 2,
                batteries: 0,
                respawn_points: None,
                profiles: None,
                artefacts: None,
                objects: Some(&objects),
                inventory: None,
            };
            let section = srp_validate::world::Section { at: 0, bytes: &bytes };
            assert_eq!(srp_validate::world::objects(&section, &context), Ok(()), "{}", map.project);
        }
    }

    #[test]
    fn an_anomalys_states_are_in_the_games_order() {
        let order = |select_priority, energy_threshold, shoot_trigger, zone_activity_trigger| StateOrder {
            select_priority,
            energy_threshold,
            shoot_trigger,
            zone_activity_trigger,
        };
        // The higher priority first, then the higher threshold (the fruit
        // anomaly's seven states are by threshold, the lowest first).
        let mut states: Vec<(StateOrder, u32)> = [0, 20, 400, 1000]
            .into_iter()
            .map(|threshold| (order(1, threshold, false, false), threshold as u32))
            .collect();
        states.push((order(2, 0, false, false), 2000));
        insertion_sort(&mut states);
        let sorted: Vec<u32> = states.iter().map(|(_, tag)| *tag).collect();
        assert_eq!(sorted, [2000, 1000, 400, 20, 0]);
        // At a tie the comparer is not a strict order: as the game, a state
        // with its zone trigger goes before every one it ties with.
        let mut states = vec![
            (order(0, 0, false, false), 1),
            (order(0, 0, false, true), 2),
            (order(0, 0, false, false), 3),
            (order(0, 0, false, true), 4),
        ];
        insertion_sort(&mut states);
        let sorted: Vec<u32> = states.iter().map(|(_, tag)| *tag).collect();
        assert_eq!(sorted, [4, 2, 1, 3]);
    }
}
