//! Local match options data: maps/modes rows, the AI player roster with kits,
//! skills -> profile modifiers, bot display names. The byte layout of the options is the protocol
//! crate's job; this module only picks rows, bots and items.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

use crate::items::{ItemDb, WEAPON_CATS};

/// The tutorial's kits and script bots (made by crates/data-extract).
pub const TUTORIAL_FILE: &str = "tutorial_069d.json";
/// at most 20 players per match
pub const MAX_PLAYERS: usize = 20;
/// bots are picked whose `match_making_level` is nearest to this
pub const DEFAULT_BOT_LEVEL: u32 = 5;

/// profile slot of each kit key (armour = category - 1, 7/10 weapons, 8/9 ammo, 13.. pockets)
pub const KIT_SLOTS: [(&str, u8); 14] = [
    ("helmet_slot", 0),
    ("mask_slot", 1),
    ("torso_slot", 2),
    ("back_slot", 3),
    ("pants_slot", 4),
    ("gloves_slot", 5),
    ("boots_slot", 6),
    ("weapon1_slot", 7),
    ("ammo_slot_1", 8),
    ("ammo_slot_2", 9),
    ("weapon2_slot", 10),
    ("pocket_slot1", 13),
    ("pocket_slot2", 14),
    ("pocket_slot3", 15),
];
/// slots whose second u16 is a count / rounds
pub const COUNTED_SLOTS: [u8; 5] = [8, 9, 13, 14, 15];

/// A row of `gameplay/game_modes/*.options` match_making_params.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModeRow {
    pub map: u32,
    pub mode: u32,
    pub variation: u32,
    /// 1 day, 2 rain, 3 evening
    pub weather: u32,
    pub project: String,
    pub map_name: String,
    pub min_players: u32,
    pub max_players: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pid {
    pub integral_gain: f64,
    pub proportional_gain: f64,
    pub max_output: f64,
    pub mass: f64,
    pub derivational_gain: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Noise {
    pub deviation: f64,
    pub max_value: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vision {
    pub near_plane_distance: f64,
    pub vertical_fov: f64,
    pub far_plane_distance: f64,
    pub velocity_factor: f64,
    pub visibility_inertia: f64,
    pub increase_speed: f64,
    pub aspect_ratio: f64,
    pub decrease_speed: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hearing {
    pub shot_sound_hearing_radius: f64,
    pub walk_sound_hearing_radius: f64,
    pub crouch_sound_hearing_radius: f64,
    pub sprint_sound_hearing_radius: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FireQueue {
    pub interval: [u32; 2],
    pub distance: f64,
    pub length: [u32; 2],
}

#[derive(Debug, Clone, Deserialize)]
pub struct KitEntry {
    pub name: String,
    #[serde(default)]
    pub amount_per_spawn: u32,
    /// modules installed on a kit weapon (`{slot, module}`, module = cfg name)
    #[serde(default)]
    pub modules_to_equip: Vec<KitModule>,
}

/// One module of a kit weapon (`modules_to_equip` entry of `ai.profiles` / `ai_players.options`).
#[derive(Debug, Clone, Deserialize)]
pub struct KitModule {
    pub slot: String,
    pub module: String,
}

/// One kit of an AI player: slot key -> item (`helmet_slot`, `weapon1_slot`, ...).
#[derive(Debug, Clone, Deserialize)]
pub struct AiProfile {
    #[serde(default)]
    pub voices: Option<String>,
    #[serde(flatten)]
    pub slots: BTreeMap<String, serde_json::Value>,
}

impl AiProfile {
    pub fn entry(&self, key: &str) -> Option<KitEntry> {
        self.slots.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// A copy of the kit with slot `key` (`weapon1_slot`, `ammo_slot_1`, ...) holding item `name` (cfg name): the
    /// entry's amount is kept, its modules are dropped (they belong to the old item). Offline tests only (a bot with a
    /// weapon outside the data kits).
    pub fn with_item(&self, key: &str, name: &str) -> AiProfile {
        let mut p = self.clone();
        let e = p.slots.entry(key.to_string()).or_insert_with(|| serde_json::json!({}));
        if let Some(o) = e.as_object_mut() {
            o.insert("name".into(), serde_json::Value::String(name.to_string()));
            o.remove("modules_to_equip");
        }
        p
    }
}

/// One entry of `gameplay/players/ai_players.options` `players` (index = ai id).
#[derive(Debug, Clone, Deserialize)]
pub struct AiPlayer {
    pub fire_delay_after_enemy_change_in_ms: u32,
    pub hit_reaction_delay_in_ms: u32,
    pub sound_scheme: String,
    pub forgive_headshots: bool,
    pub heading_pid_controller_parameters: Pid,
    pub pitch_pid_controller_parameters: Pid,
    /// kits; absent in a script's `ai_params` (the tutorial bots' kits are named by the script entry)
    #[serde(default)]
    pub profiles: Vec<AiProfile>,
    pub aim_distance: f64,
    pub tick_time_in_ms: u32,
    pub enemy_detection_crouch_capsule_radius: f64,
    pub enemy_detection_stand_capsule_radius: f64,
    pub vision_reaction_delay_in_ms: u32,
    pub vision_sensor_parameters: Vision,
    pub hearing_sensor_parameters: Hearing,
    pub enemy_death_reaction_delay_in_ms: u32,
    pub hit_chance: u32,
    pub heading_noise_parameters: Noise,
    pub pitch_noise_parameters: Noise,
    pub last_sensed_enemy_cull_down_time_in_ms: u32,
    pub heading_value_too_large_for_fire_after: f64,
    pub can_kill_series_before_shot_time_in_ms: u32,
    pub sound_reaction_delay_in_ms: u32,
    /// "skill_543" -> learned points
    #[serde(default)]
    pub skills: BTreeMap<String, u32>,
    /// absent in a script's `ai_params` (tutorial bots): 0
    #[serde(default)]
    pub match_making_level: u32,
    pub hit_damage_multiplier: f64,
    #[serde(default)]
    pub limit_in_match: u32,
    pub fire_queue_distance_parameters: Vec<FireQueue>,
    pub shoot_after_aim_delay_in_ms: u32,
    #[serde(default)]
    pub difficulty: u32,
}

/// One bot of the tutorial script (`mode.scripting.profiles` of `gameplay/game_modes/tutorial.options`): the record
/// name must be the script's name (the scene binds records with `ai_id != FFFFFFFF` by name, exe 0x2003C9464),
/// `head` = face, `kit` = the named `ai.profiles` kit, `ai` = `ai_params`.
#[derive(Debug, Clone, Deserialize)]
pub struct TutorialBot {
    pub name: String,
    pub team: u8,
    pub head: u8,
    pub kit: AiProfile,
    pub ai: AiPlayer,
}

/// The tutorial's players (`data/tutorial_069d.json`): the human's kits `tutorial_player_0..3` (profile index = kit
/// number: the script spawns index 3 and switches to 0, 1, 2) and the nine script bots.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TutorialData {
    pub human: Vec<AiProfile>,
    pub bots: Vec<TutorialBot>,
}

/// Items, slots and counts of a bot kit, as the match options carry them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotKit {
    /// (instance id, dict id, u32 at +0xC)
    pub items: Vec<(u32, u16, u32)>,
    /// profile slot -> instance id
    pub slots: BTreeMap<u8, u32>,
    /// profile slot -> count / rounds (counted slots only)
    pub counts: BTreeMap<u8, u32>,
    /// installed modules: weapon instance id -> (slot name, module instance id), from the kit's `modules_to_equip`;
    /// the module items follow the slot items in `items`
    pub attach: Vec<(u32, Vec<(String, u32)>)>,
}

/// (map, mode, variation, weather, project, map name, min players, max players)
type RawModeRow = (u32, u32, u32, u32, String, String, u32, u32);

#[derive(Debug, Deserialize)]
pub(crate) struct RawBotData {
    pub mode_rows: Vec<RawModeRow>,
    pub ai_players: Vec<AiPlayer>,
    pub skill_tree: HashMap<String, Vec<Vec<(String, f64)>>>,
    pub mod_names: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawNames {
    #[serde(default)]
    pub first: Vec<Option<String>>,
    #[serde(default)]
    pub second: Vec<Option<String>>,
}

#[derive(Debug, Default)]
pub struct MatchData {
    modes: Vec<ModeRow>,
    ai: Vec<AiPlayer>,
    skill_tree: HashMap<u32, Vec<Vec<(String, f64)>>>,
    mod_names: Vec<String>,
    first_names: Vec<Option<String>>,
    second_names: Vec<Option<String>>,
    tutorial: TutorialData,
}

impl MatchData {
    pub(crate) fn new(raw: RawBotData, names: RawNames, tutorial: TutorialData) -> MatchData {
        MatchData {
            modes: raw
                .mode_rows
                .into_iter()
                .map(|(map, mode, variation, weather, project, map_name, min_players, max_players)| ModeRow {
                    map,
                    mode,
                    variation,
                    weather,
                    project,
                    map_name,
                    min_players,
                    max_players,
                })
                .collect(),
            ai: raw.ai_players,
            skill_tree: raw.skill_tree.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect(),
            mod_names: raw.mod_names,
            first_names: names.first,
            second_names: names.second,
            tutorial,
        }
    }

    /// Add a row that is not in the mode row table (the gun race row of its own mode file); a row already there is not
    /// added twice.
    pub(crate) fn add_row(&mut self, row: ModeRow) {
        if !self.modes.contains(&row) {
            self.modes.push(row);
        }
    }

    /// The tutorial's human kits and script bots.
    pub fn tutorial(&self) -> &TutorialData {
        &self.tutorial
    }

    /// Drop every kit's `modules_to_equip`: only for byte comparisons with the earlier reference implementation,
    /// which ignored them.
    pub fn clear_kit_modules(&mut self) {
        for ai in &mut self.ai {
            for p in &mut ai.profiles {
                for v in p.slots.values_mut() {
                    if let Some(o) = v.as_object_mut() {
                        o.remove("modules_to_equip");
                    }
                }
            }
        }
    }

    pub fn modes(&self) -> &[ModeRow] {
        &self.modes
    }

    /// Row for the lobby's `5B 01` (map, mode, weather). Rows of the requested variation (the weather id)
    /// first, else variation 0; the day row (weather 1) is preferred, then the lowest weather.
    pub fn resolve_map(&self, map: u32, mode: u32, weather_id: u32) -> Option<&ModeRow> {
        let mut rows: Vec<&ModeRow> =
            self.modes.iter().filter(|r| r.map == map && r.mode == mode && r.variation == weather_id).collect();
        if rows.is_empty() {
            rows = self.modes.iter().filter(|r| r.map == map && r.mode == mode && r.variation == 0).collect();
        }
        // stable sort by (weather != 1, weather)
        rows.sort_by_key(|r| (r.weather != 1, r.weather));
        rows.first().copied()
    }

    pub fn ai_players(&self) -> &[AiPlayer] {
        &self.ai
    }

    /// Indices of `count` bots: nearest `match_making_level` first, then higher difficulty, then index
    /// (each AI player at most once: limit_in_match is 1).
    pub fn pick_bots(&self, count: usize, level: u32) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.ai.len()).collect();
        order.sort_by_key(|&i| {
            (
                (self.ai[i].match_making_level as i64 - level as i64).abs(),
                -(self.ai[i].difficulty as i64),
                i,
            )
        });
        order.truncate(count);
        order
    }

    /// Items/slots/counts of a kit (an AI player's `profiles[0]`, a tutorial kit). Item ids count up from 1 in slot
    /// order; the modules of `modules_to_equip` follow as items of their own (count field as a lobby item's), installed
    /// on their weapon in the listed order (a module whose name or slot is unknown is skipped).
    pub fn kit(&self, items: &ItemDb, profile: &AiProfile) -> BotKit {
        let mut out = BotKit { items: Vec::new(), slots: BTreeMap::new(), counts: BTreeMap::new(), attach: Vec::new() };
        let mut keys = KIT_SLOTS;
        keys.sort_by_key(|k| k.1);
        let mut modules: Vec<(u32, Vec<KitModule>)> = Vec::new();
        for (key, slot) in keys {
            let Some(e) = profile.entry(key) else { continue };
            let Some(did) = items.by_cfg_name(&e.name) else { continue };
            let it = items.get(did).expect("dictionary row");
            let amount = e.amount_per_spawn;
            let base = it.base_item.filter(|b| *b != 0);
            let c = match base {
                Some(b) if WEAPON_CATS.contains(&it.category) => b,
                _ if amount != 0 => amount,
                _ => 100,
            };
            let iid = out.items.len() as u32 + 1;
            out.items.push((iid, did, c));
            out.slots.insert(slot, iid);
            if COUNTED_SLOTS.contains(&slot) {
                out.counts.insert(slot, amount);
            }
            if !e.modules_to_equip.is_empty() {
                modules.push((iid, e.modules_to_equip));
            }
        }
        for (weapon, list) in modules {
            let mut installed = Vec::new();
            for m in list {
                let Some(did) = items.by_cfg_name(&m.module) else { continue };
                if m.slot.is_empty() || installed.iter().any(|(s, _): &(String, u32)| *s == m.slot) {
                    continue;
                }
                let iid = out.items.len() as u32 + 1;
                out.items.push((iid, did, items.amount_field(did, 1)));
                installed.push((m.slot, iid));
            }
            if !installed.is_empty() {
                out.attach.push((weapon, installed));
            }
        }
        out
    }

    /// Modifier id -> summed value of the learned skill levels (`{"skill_543": 3}`).
    /// The profile carries the SUM per modifier id (name table index).
    pub fn skill_modifiers(&self, skills: &BTreeMap<String, u32>) -> BTreeMap<u32, f64> {
        let mut mods = BTreeMap::new();
        for (sid, pts) in skills {
            let Ok(sid) = sid.trim_start_matches("skill_").parse::<u32>() else { continue };
            let Some(levels) = self.skill_tree.get(&sid) else { continue };
            for lvl in levels.iter().take(*pts as usize) {
                for (name, val) in lvl {
                    if let Some(i) = self.mod_names.iter().position(|n| n == name) {
                        *mods.entry(i as u32).or_insert(0.0) += val;
                    }
                }
            }
        }
        mods
    }

    /// Does level `level` (0-based) of skill `id` carry modifier `name` (`skills_tree.options` `modifiers[level]`)?
    pub fn skill_level_has(&self, id: u16, level: usize, name: &str) -> bool {
        self.skill_tree.get(&u32::from(id)).and_then(|l| l.get(level)).is_some_and(|m| m.iter().any(|(n, _)| n == name))
    }

    pub fn modifier_names(&self) -> &[String] {
        &self.mod_names
    }

    /// Bot display name: "<first> <second>" of the two name lists, `fallback` when both are empty.
    pub fn bot_name(&self, first: usize, second: usize, fallback: &str) -> String {
        let f = self.first_names.get(first).and_then(|s| s.as_deref()).unwrap_or("");
        let s = self.second_names.get(second).and_then(|s| s.as_deref()).unwrap_or("");
        let n = format!("{f} {s}");
        let n = n.trim();
        if n.is_empty() { fallback.to_string() } else { n.to_string() }
    }

    /// Name list indices of bot number `k` (0-based): (k % 31, (k * 7 + 3) % 30).
    pub fn bot_name_indices(k: usize) -> (usize, usize) {
        (k % 31, (k * 7 + 3) % 30)
    }

    /// Team of bot number `k`: alternating, the first bot is on team 1 (the human is on team 0).
    pub fn bot_team(k: usize) -> u8 {
        ((k + 1) % 2) as u8
    }
}

#[cfg(test)]
mod tests {

    /// The tutorial data: four human kits, nine script bots on team 1 with their kits and `ai_params` (tutorial.options
    /// vol 75); every kit item resolves in the dictionary.
    #[test]
    fn tutorial_kits_and_script_bots() {
        crate::require_tables!();
        let g = crate::testing::tables();
        let t = g.matches.tutorial();
        assert_eq!(t.human.len(), 4);
        let names: Vec<&str> = t.bots.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names[0], "tutorial_bot1_battery");
        assert_eq!(names[8], "tutorial_bot2_ctp");
        assert_eq!(names.len(), 9);
        assert!(t.bots.iter().all(|b| b.team == 1));
        assert_eq!(t.bots.iter().map(|b| b.head).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5, 6, 7, 0]);
        // script AI: no match making level in ai_params, the data's damage multiplier
        assert!(t.bots.iter().all(|b| b.ai.match_making_level == 0 && (b.ai.hit_damage_multiplier - 0.3).abs() < 1e-6));
        for kit in t.human.iter().chain(t.bots.iter().map(|b| &b.kit)) {
            let k = g.matches.kit(&g.items, kit);
            let listed = kit.slots.values().filter(|v| v.get("name").is_some()).count();
            assert_eq!(k.slots.len(), listed, "every kit item is in the dictionary");
        }
    }

    /// `modules_to_equip` becomes module items installed on the kit weapon (training bots 1 and 3 carry the Mosin scope and
    /// the PPSh mag + collimator); clearing them gives the plain slot items.
    #[test]
    fn kit_modules_are_installed_items() {
        crate::require_tables!();
        let mut g = crate::testing::tables();
        let ai = g.matches.ai_players()[3].profiles[0].clone();
        let k = g.matches.kit(&g.items, &ai);
        assert_eq!(k.attach.len(), 1);
        let (weapon, list) = &k.attach[0];
        assert_eq!(k.slots[&7], *weapon);
        let dict = |iid: u32| k.items.iter().find(|i| i.0 == iid).unwrap().1;
        let slots: Vec<(&str, u16)> = list.iter().map(|(s, m)| (s.as_str(), dict(*m))).collect();
        assert_eq!(slots, [("mag", g.items.by_cfg_name("ppsh_762x25_30_round_mag").unwrap()), ("scope_picatinnyrail", g.items.by_cfg_name("eotech_552_collimator").unwrap())]);
        let n = k.items.len();
        g.matches.clear_kit_modules();
        let ai = g.matches.ai_players()[3].profiles[0].clone();
        let plain = g.matches.kit(&g.items, &ai);
        assert!(plain.attach.is_empty());
        assert_eq!(plain.items.len(), n - 2);
        assert_eq!(plain.items[..], k.items[..n - 2]);
    }
}
