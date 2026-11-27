//! PvE team missions (docs/notes/pve.md): the lobby-side data, from `data/pve_069d.json` (made by
//! `crates/data-extract` from the client's own files).
//!
//! * missions (`gameplay/pve_missions.options`): one mission, id 15, three difficulties, three players;
//! * the game mode (`pve_01_v*_maze*.options`): mode 7, weather 1, the six map projects;
//! * prices (`game.options` `pve_default_prices`): ticket, respawn, sponsor, block-slot, unlock, reroll prices (all gold);
//! * the access keys: dictionary 1384 (premium key) and 1617 (free key), inventory items (category 27);
//! * the reward chests `pve_mission_1_chest_rank_NM` (N 1..5 equipment tier, M 1..3 = difficulty + 1);
//! * the script's 17 bots per difficulty (`pve_bots_069d.json`, made by `crates/data-extract` from the mode
//!   file's `scripting.pve_profiles` and `gameplay/players/ai.profiles`): record name, team, face, the two kits, the AI
//!   block.
//!
//! Everything here is PROVEN data; rules the files do not state are marked CHOICE / LIKELY where they are used.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::matches::{AiPlayer, AiProfile};

pub const FILE: &str = "pve_069d.json";
/// The script's bots (`crates/data-extract`).
pub const BOTS_FILE: &str = "pve_bots_069d.json";
/// Bots of the mission script per difficulty (`pve_bot_1` .. `pve_bot_17`).
pub const BOTS: usize = 17;

/// One bot of the mission script (`scripting.pve_profiles[difficulty][group][i]` of the `pve_01_*` mode files, PROVEN
/// data). The scene binds records with `ai_id != FFFFFFFF` to the script's players by record name (0x2003C9464), so the
/// record name must be `name`. `profile` is the match profile index 0 (the forest and sawmill bandits' kit), `profile1`
/// index 1 (the army squad's kit; the script spawns with its own profile byte, docs/notes/pve.md 10.2: the index mapping
/// is LIKELY).
#[derive(Debug, Clone, Deserialize)]
pub struct PveBot {
    pub name: String,
    pub team: u8,
    pub head: u8,
    pub profile: AiProfile,
    pub profile1: AiProfile,
    /// The two kit names (`ai.profiles` keys), for logs.
    pub kit_names: [String; 2],
    /// `ai_params`: the options' AI block; the clients read only `hit_chance`, `hit_damage_multiplier` and
    /// `forgive_headshots` (hit code 0x200302F60..0x200303036), the rest is the server AI's.
    pub ai: AiPlayer,
}

/// `Pve_ticket_voucher`: the premium access key (dictionary id).
pub const KEY_PREMIUM: u16 = 1384;
/// `pve_ticket_voucher_free`: the free access key.
pub const KEY_FREE: u16 = 1617;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct RawReward {
    amount: u32,
    dict_id: u16,
}

#[derive(Debug, Clone, Deserialize)]
struct RawDifficulty {
    label: String,
    description: String,
    rewards: RawReward,
}

#[derive(Debug, Clone, Deserialize)]
struct RawMission {
    id: u32,
    name: String,
    chests_total: u32,
    closed_chest_dict_id: u16,
    min_squad_members: u32,
    difficulties: Vec<RawDifficulty>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawChest {
    name: String,
    drop_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawVoucher {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawMatchMaking {
    map_id: u32,
    map_weather_id: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct RawParameters {
    match_time: u32,
    wait_time: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct RawMode {
    mode: u32,
    match_making_params: RawMatchMaking,
    parameters: RawParameters,
    projects: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Raw {
    missions: Vec<RawMission>,
    mode: RawMode,
    prices: Prices,
    vouchers: BTreeMap<String, RawVoucher>,
    chest_items: BTreeMap<String, RawChest>,
    chests_premium_bonus_coef: u32,
    reward_reroll_info: Vec<[u32; 2]>,
}

/// `pve_default_prices` (PROVEN, `game.options` v75, parsed at 0x2002DE331). All prices are gold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Prices {
    pub ticket_price: u32,
    pub respawn_price: u32,
    pub sponsor_price: u32,
    pub block_slot_price: u32,
    pub unlock_drop_price: u32,
    pub reroll_start_price: u32,
    pub reroll_step: u32,
    pub free_chests_num: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difficulty {
    /// Text ids (`st_difficulty_low_name` ...).
    pub label: String,
    pub description: String,
    /// The preview reward of the mission window: dictionary id and amount.
    pub preview_chest: (u16, u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mission {
    /// `match_making_params.map_id` (15).
    pub id: u32,
    pub name: String,
    pub chests_total: u32,
    pub closed_chest_dict: u16,
    /// The player count of a mission (3).
    pub players: u32,
    pub difficulties: Vec<Difficulty>,
}

#[derive(Debug)]
pub struct PveData {
    pub missions: Vec<Mission>,
    pub prices: Prices,
    /// Game mode id of `game_modes/pve_01_*` (7).
    pub mode: u32,
    pub weather: u32,
    /// `match_time` seconds of the mode file (1800; the script says 45 minutes: OPEN which wins).
    pub match_time: u32,
    /// `parameters.wait_time` seconds (300).
    pub wait_time: u32,
    /// The six map projects `pve_01_v{1..3}_maze{1,2}`.
    pub projects: Vec<String>,
    pub chests_premium_bonus_coef: u32,
    /// `(start price, step)` of `pve_reward_reroll_info`.
    pub reroll: (u32, u32),
    /// `(tier N, difficulty index)` -> chest dictionary id.
    chests: BTreeMap<(u8, u8), u16>,
    /// The script's bots per difficulty index ([`PveData::with_bots`]; empty until loaded).
    bots: Vec<Vec<PveBot>>,
}

impl PveData {
    pub fn parse(json: &str) -> Result<PveData, serde_json::Error> {
        let raw: Raw = serde_json::from_str(json)?;
        let bad = |m: &str| <serde_json::Error as serde::de::Error>::custom(m.to_string());
        let missions: Vec<Mission> = raw
            .missions
            .into_iter()
            .map(|m| Mission {
                id: m.id,
                name: m.name,
                chests_total: m.chests_total,
                closed_chest_dict: m.closed_chest_dict_id,
                players: m.min_squad_members,
                difficulties: m
                    .difficulties
                    .into_iter()
                    .map(|d| Difficulty { label: d.label, description: d.description, preview_chest: (d.rewards.dict_id, d.rewards.amount) })
                    .collect(),
            })
            .collect();
        if missions.is_empty() || missions.iter().any(|m| m.difficulties.is_empty() || m.players == 0) {
            return Err(bad("pve missions without difficulties or players"));
        }
        // the keys must be the two vouchers the client counts (Flash PVE_TOKEN_PREM_ID / PVE_TOKEN_FREE_ID)
        let name_of = |id: u16| raw.vouchers.get(&id.to_string()).map(|v| v.name.as_str());
        if name_of(KEY_PREMIUM) != Some("pve_ticket_voucher") || name_of(KEY_FREE) != Some("pve_ticket_voucher_free") {
            return Err(bad("pve key vouchers 1384 / 1617 missing"));
        }
        // `pve_mission_1_chest_rank_NM`: N = equipment tier 1..5, M = difficulty + 1
        let mut chests = BTreeMap::new();
        for (id, c) in &raw.chest_items {
            // the data has one name with a leading '/' (dict 1473, `/pve_mission_1_chest_rank_31`): same chest
            let Some(nm) = c.name.trim_start_matches('/').strip_prefix("pve_mission_1_chest_rank_") else { continue };
            let b = nm.as_bytes();
            if b.len() != 2 || !b.iter().all(u8::is_ascii_digit) {
                continue;
            }
            let (n, m) = (b[0] - b'0', b[1] - b'0');
            if !(1..=5).contains(&n) || !(1..=3).contains(&m) || !c.drop_type.starts_with("pve_mission_1_chest_rank_") {
                continue;
            }
            chests.insert((n, m - 1), id.parse::<u16>().map_err(|_| bad("chest dict id"))?);
        }
        let reroll = raw.reward_reroll_info.first().map(|r| (r[0], r[1])).unwrap_or((raw.prices.reroll_start_price, raw.prices.reroll_step));
        let map_id = raw.mode.match_making_params.map_id;
        let d = PveData {
            missions,
            prices: raw.prices,
            mode: raw.mode.mode,
            weather: raw.mode.match_making_params.map_weather_id,
            match_time: raw.mode.parameters.match_time,
            wait_time: raw.mode.parameters.wait_time,
            projects: raw.mode.projects,
            chests_premium_bonus_coef: raw.chests_premium_bonus_coef,
            reroll,
            chests,
            bots: Vec::new(),
        };
        // the map id of the game mode must be a mission
        if d.mission(map_id).is_none() {
            return Err(bad("pve game mode map id is no mission"));
        }
        Ok(d)
    }

    /// Add the script's bots (`pve_bots_069d.json`: difficulty index -> the 17 bots in record order). Every difficulty
    /// of the mission needs exactly [`BOTS`] bots named `pve_bot_1..17`.
    pub fn with_bots(mut self, json: &str) -> Result<PveData, serde_json::Error> {
        let bad = |m: &str| <serde_json::Error as serde::de::Error>::custom(m.to_string());
        let raw: BTreeMap<u8, Vec<PveBot>> = serde_json::from_str(json)?;
        let n = self.missions.iter().map(|m| m.difficulties.len()).max().unwrap_or(0);
        let mut bots = Vec::with_capacity(n);
        for d in 0..n as u8 {
            let list = raw.get(&d).cloned().ok_or_else(|| bad("pve bots: a difficulty without bots"))?;
            if list.len() != BOTS || list.iter().enumerate().any(|(i, b)| b.name != format!("pve_bot_{}", i + 1)) {
                return Err(bad("pve bots: not the 17 bots pve_bot_1..17 in order"));
            }
            bots.push(list);
        }
        self.bots = bots;
        Ok(self)
    }

    /// The script's bots of a difficulty (record order), empty when unknown.
    pub fn bots(&self, difficulty: u8) -> &[PveBot] {
        self.bots.get(difficulty as usize).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn mission(&self, id: u32) -> Option<&Mission> {
        self.missions.iter().find(|m| m.id == id)
    }

    /// A (map, difficulty) the client may ask for: a known mission and a difficulty of it.
    pub fn valid(&self, map: u32, difficulty: u32) -> bool {
        self.mission(map).is_some_and(|m| (difficulty as usize) < m.difficulties.len())
    }

    /// The match options' project (map name) of a difficulty and a maze variant (1 or 2). LIKELY: `v1..v3` is the
    /// difficulty and `maze1` / `maze2` two variants of it (the six mode files differ only in this name); OPEN: which
    /// variant the original server picked (the server picks one at random).
    pub fn project(&self, difficulty: u8, maze: u8) -> Option<String> {
        let p = format!("pve_01_v{}_maze{}", difficulty as u32 + 1, maze);
        self.projects.contains(&p).then_some(p)
    }

    /// Chest dictionary id of equipment tier `tier` (1..=5) and difficulty index (0..=2): `pve_mission_1_chest_rank_NM`.
    pub fn chest(&self, tier: u8, difficulty: u8) -> Option<u16> {
        self.chests.get(&(tier, difficulty)).copied()
    }

    /// Gold price of one ticket (a key's `cost` x `play_cost` 1), the number the mission window shows as `play_cost_gold`.
    pub fn ticket_cost(&self) -> u32 {
        self.prices.ticket_price
    }

    /// Gold price of the `n`-th reroll of the rewards (`n` from 0): `reroll_start_price + n * reroll_step`
    /// (LIKELY: `pve_reward_reroll_info [[50, 25]]` = start 50, step 25).
    pub fn reroll_cost(&self, n: u32) -> u32 {
        self.reroll.0.saturating_add(self.reroll.1.saturating_mul(n))
    }

    /// Gold price of opening the `locked` remaining caches (`unlock_drop_price` each; the first `free_chests_num` are free).
    pub fn unlock_cost(&self, chests_found: u32) -> u32 {
        chests_found.saturating_sub(self.prices.free_chests_num).saturating_mul(self.prices.unlock_drop_price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> PveData {
        PveData::parse(&crate::testing::read("pve_069d.json")).unwrap()
    }

    #[test]
    fn mission_mode_and_prices() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.missions.len(), 1);
        let m = d.mission(15).unwrap();
        assert_eq!((m.players, m.chests_total, m.closed_chest_dict, m.difficulties.len()), (3, 10, 1385, 3));
        assert_eq!(m.difficulties.iter().map(|x| x.preview_chest.0).collect::<Vec<_>>(), [1483, 1468, 1469]);
        assert_eq!((d.mode, d.weather, d.match_time, d.wait_time), (7, 1, 1800, 300));
        assert_eq!(d.prices.ticket_price, 50);
        assert_eq!((d.prices.sponsor_price, d.prices.block_slot_price, d.prices.free_chests_num), (50, 50, 3));
        assert_eq!(d.reroll, (50, 25));
        assert!(d.valid(15, 0) && d.valid(15, 2));
        assert!(!d.valid(15, 3) && !d.valid(14, 0));
    }

    /// The 17 bots per difficulty: names, team 1, two kits, the difficulty's AI values (docs/notes/pve.md 10.3).
    #[test]
    fn script_bots_per_difficulty() {
        crate::require_tables!();
        let d = data().with_bots(&crate::testing::read("pve_bots_069d.json")).unwrap();
        for k in 0..3u8 {
            let b = d.bots(k);
            assert_eq!(b.len(), BOTS);
            assert_eq!(b[16].name, "pve_bot_17");
            assert!(b.iter().all(|x| x.team == 1 && !x.profile.slots.is_empty() && !x.profile1.slots.is_empty()));
        }
        assert!(d.bots(3).is_empty());
        let hit = |k: u8| (d.bots(k)[0].ai.hit_chance, d.bots(k)[0].ai.hit_damage_multiplier, d.bots(k)[0].ai.forgive_headshots);
        assert_eq!((hit(0), hit(1), hit(2)), ((50, 0.5, true), (75, 1.0, true), (100, 1.25, false)));
        assert_eq!(d.bots(0)[0].kit_names, ["1lvl_bm_assault_hevik".to_string(), "1lvl_army_karabin_gaz".to_string()]);
        assert_eq!(d.bots(0)[0].ai.vision_sensor_parameters.far_plane_distance, 40.0);
        // a short list is refused
        assert!(data().with_bots(r#"{"0": [], "1": [], "2": []}"#).is_err());
    }

    #[test]
    fn projects_chests_and_costs() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.projects.len(), 6);
        assert_eq!(d.project(0, 1).as_deref(), Some("pve_01_v1_maze1"));
        assert_eq!(d.project(2, 2).as_deref(), Some("pve_01_v3_maze2"));
        assert_eq!(d.project(3, 1), None);
        assert_eq!(d.project(0, 3), None);
        // N = 1 are the three preview dictionaries of the mission, 12 = tier 1 medium ... 15 chests in all
        assert_eq!(d.chest(1, 0), Some(1483));
        assert_eq!(d.chest(1, 1), Some(1468));
        assert_eq!(d.chest(1, 2), Some(1469));
        assert_eq!(d.chest(5, 2), Some(1481));
        assert_eq!(d.chest(6, 0), None);
        assert_eq!(d.chest(3, 0), Some(1473), "the name with a leading slash is read too");
        assert_eq!(d.ticket_cost(), 50);
        assert_eq!((d.reroll_cost(0), d.reroll_cost(1), d.reroll_cost(3)), (50, 75, 125));
        assert_eq!((d.unlock_cost(2), d.unlock_cost(3), d.unlock_cost(10)), (0, 0, 350));
    }
}
