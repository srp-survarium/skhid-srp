//! game-data (0.69d): the client's game tables and the lobby rules that read them.
//!
//! The rules follow the client (exe and data) where it decides them; the notes in `docs/notes` say where a rule comes
//! from. See README.md.
//!
//! The tables are not shipped: `srp setup` builds them from the operator's game install (crate `data-extract`) into a
//! data folder, and [`GameData::from_dir`] loads them. Tests load them with [`testing`] and skip when there are none.
//!
//! ```no_run
//! let gd = game_data::GameData::from_dir("data".as_ref()).unwrap();
//! let price = gd.shop.price(3).unwrap(); // AK-74M
//! assert!(price.for_sale);
//! ```

pub mod achievements;
pub mod badges;
pub mod chests;
pub mod clan;
pub mod daily_quests;
pub mod decals;
pub mod events;
pub mod skins;
pub mod facts;
pub mod faction_battle;
pub mod fines;
pub mod game_modes;
pub mod gunrace;
pub mod craft;
pub mod hero;
pub mod hunt;
pub mod items;
pub mod league;
pub mod levels;
pub mod matches;
pub mod merge;
pub mod module_drops;
pub mod modules;
pub mod perks;
pub mod pockets;
pub mod pve;
pub mod progression;
pub mod result_texts;
pub mod rewards;
pub mod shop;
pub mod skills;
pub mod testing;
pub mod texts;
pub mod vouchers;
mod tsv;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Deserialize;

pub use achievements::{Achievement, Achievements};
pub use chests::ChestData;
pub use daily_quests::DailyQuests;
pub use decals::DecalData;
pub use skins::SkinData;
pub use facts::{KillFact, MatchFacts};
pub use faction_battle::FactionBattleData;
pub use hero::HeroData;
pub use hunt::HuntData;
pub use craft::{CraftData, ItemParams, ModGroup, Roll, SplitMix};
pub use items::{Item, ItemDb};
pub use league::{LeagueData, Schedule as LeagueSchedule, Season as LeagueSeason};
pub use levels::{LevelFields, Levels};
pub use matches::{AiPlayer, BotKit, MatchData, ModeRow};
pub use modules::ModuleData;
pub use progression::Progression;
pub use pve::PveData;
pub use rewards::Drops;
pub use vouchers::Vouchers;
pub use shop::{Price, ShopTable, Wallet};
pub use skills::{ResetPayment, SkillError, SkillPrices, SkillRow, Skills};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("reading {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("parsing {what}: {source}")]
    Json { what: &'static str, source: serde_json::Error },
    #[error("no game tables in {dir} ({missing} of the files missing, e.g. {first}): run `srp setup` (it builds them from your game install)")]
    NoTables { dir: String, missing: usize, first: &'static str },
}

/// File names inside the data directory.
pub const ITEMS_FILE: &str = "items_069d.tsv";
pub const SHOP_FILE: &str = "shop_069d.tsv";
pub const TABLES_FILE: &str = "game_tables_069d.json";
pub const BOTS_FILE: &str = "bot_match_069d.json";
pub const NAMES_FILE: &str = "bot_names_en.json";

/// Every file of a data folder (what `srp setup` writes and [`GameData::from_dir`] reads).
pub const DATA_FILES: [&str; 32] = [
    "items_069d.tsv", "shop_069d.tsv", "game_tables_069d.json", "bot_match_069d.json", "bot_names_en.json", "tutorial_069d.json",
    "achievements_069d.json", "badges_069d.json", "daily_quests_069d.json", "skills_069d.json", "skins_069d.json",
    "weapon_hands_069d.json", "decals_069d.json", "module_drops_069d.json", "chests_069d.json", "drops_069d.json",
    "weekly_drop_069d.json", "faction_battle_069d.json", "event_drops_069d.json", "hero_of_the_day_069d.json",
    "event_exchange_069d.json", "league_069d.json", "progression_069d.json", "vouchers_069d.json", "booster_costs_069d.json",
    "game_modes_069d.json", "gunrace_069d.json", "pve_069d.json", "pve_bots_069d.json", "store_texts_069d.json",
    "event_categories_069d.json", "bot_names_069d.json",
];

/// The files of [`DATA_FILES`] that `dir` lacks.
pub fn missing_files(dir: &Path) -> Vec<&'static str> {
    DATA_FILES.iter().copied().filter(|f| !dir.join(f).is_file()).collect()
}

#[derive(Debug, Deserialize)]
struct RawTables {
    items: HashMap<String, ItemParams>,
    mod_groups: HashMap<String, ModGroup>,
    weapon_slots: HashMap<String, Vec<Option<String>>>,
    compatible_ammo: HashMap<String, Vec<u32>>,
    module_links: Vec<(String, String)>,
    xp_table: Vec<u32>,
    level_rewards: Vec<(u32, u32)>,
    item_categories: Vec<(u32, u32, u32)>,
    slot_categories: Vec<Vec<u16>>,
    exchange_currency_rates: Vec<Vec<u32>>,
    buy_specific_cost: Vec<Vec<u32>>,
    buy_preset_cost: Vec<Vec<u32>>,
    match_levels_factors: Vec<LevelFactor>,
    /// weapon cfg name -> (slot id, module cfg name): `weapons.*.default_modules` of game.options
    #[serde(default)]
    weapon_default_modules: HashMap<String, Vec<(String, String)>>,
    /// module cfg name -> where it attaches and what its attach_params blocks add / disable / override
    #[serde(default)]
    module_attach: HashMap<String, modules::RawAttach>,
    /// weapon cfg name -> its slot ids with `accept_module` true
    #[serde(default)]
    weapon_slots_accepting: HashMap<String, Vec<String>>,
    /// (module dict id, `detach_price_in_gold`) of `modules_drop_options.modules_additional_infos`, non-zero rows
    #[serde(default)]
    module_detach_price: Vec<(u16, u32)>,
    /// (module dict id, `premium_modifiers`, `override_modifiers`) of every `modules_additional_infos` row
    #[serde(default)]
    module_mods: Vec<(u16, Vec<(u16, u16)>, Vec<(u16, u16)>)>,
}

/// `match_levels_factors` row: multipliers of match rewards by match level.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct LevelFactor {
    pub reputation_factor: f64,
    pub experience_factor: f64,
    pub silver_factor: f64,
}

/// All tables. Build once (`from_dir`) and share it (`Arc<GameData>`); it is read only.
#[derive(Debug)]
pub struct GameData {
    pub items: ItemDb,
    pub shop: ShopTable,
    pub craft: CraftData,
    pub modules: ModuleData,
    pub levels: Levels,
    pub matches: MatchData,
    pub skills: Skills,
    pub achievements: Achievements,
    /// badges (in-match medals) and their rewards (data/badges_069d.json)
    pub badges: badges::Badges,
    /// daily quests (data/daily_quests_069d.json)
    pub daily_quests: DailyQuests,
    /// post-match drops (`drop_after_combat`, data/drops_069d.json)
    pub drops: Drops,
    /// new-player kit, faction levels, event rewards (`progression_069d.json`)
    pub progression: Progression,
    /// camouflage / decal items, sets, armour that takes them (`decals_069d.json`)
    pub decals: DecalData,
    /// weapon skins (skins_069d.json)
    pub skins: SkinData,
    /// vouchers and boosters (premium time, booster modifiers)
    pub vouchers: Vouchers,
    /// chest contents and drop tables (`chests_069d.json`)
    pub chests: ChestData,
    /// rarity and drop modifier tables of new modules (`module_drops_069d.json`)
    pub module_drops: module_drops::ModuleDrops,
    /// treasure hunt (item search roulette) pages (`weekly_drop_069d.json`)
    pub hunt: HuntData,
    /// faction battle reward tiers (`faction_battle_069d.json`)
    pub faction_battle: FactionBattleData,
    /// ranked leagues: tables, rating rule, league shop (`league_069d.json`)
    pub league: LeagueData,
    /// hero of the day: competitive groups, bonuses, periods (`hero_of_the_day_069d.json`)
    pub hero: HeroData,
    /// event currency exchange rows (`event_exchange_069d.json`)
    pub exchange: hero::ExchangeData,
    /// event currency drops, worn-item bonus (`event_drops_069d.json`, docs/notes/events.md)
    pub events: events::EventData,
    /// PvE missions, prices, keys and chests (`pve_069d.json`)
    pub pve: PveData,
    /// game-mode timing per level project (`game_modes_069d.json`): the online match end
    pub game_modes: game_modes::GameModes,
    /// Arms Race (gun race, mode 3): its row (also in `matches`) and the race kits (`gunrace_069d.json`)
    pub gunrace: gunrace::GunRaceData,
    /// the client's own texts of the keys shown in fields the client does not localize (`store_texts_069d.json`)
    pub texts: texts::ClientTexts,
    /// `events.options` `ui_category` per event: the result window's three reward rows (`event_categories_069d.json`)
    pub event_categories: result_texts::EventCategories,
    /// the client's bot name lists with their texts per language: bot rows of the result window (`bot_names_069d.json`)
    pub bot_names: result_texts::BotNames,
    /// row `slot` = categories allowed in that profile slot (0..=22)
    pub slot_categories: Vec<Vec<u16>>,
    /// (category id, is_stackable, minimum count)
    pub item_categories: Vec<(u32, bool, u32)>,
    /// `exchange_currency_rates`: silver per gold (15 entries, all 250)
    pub exchange_rates: Vec<u32>,
    /// `buy_specific_cost` rows (price of using a non-owned item in a pocket, by tier)
    pub buy_specific_cost: Vec<Vec<u32>>,
    pub buy_preset_cost: Vec<Vec<u32>>,
    pub match_level_factors: Vec<LevelFactor>,
    /// one-handed weapons (`double_handed` false; data/weapon_hands_069d.json), see [`perks`]
    pub one_handed: std::collections::BTreeSet<u16>,
}

impl GameData {
    /// The level projects a world host serves, `(project, level path)` in data order: every PvP mode row (the data holds
    /// only the rows whose level project ships in the client; `project` = the row's project, the level = its `map_name`,
    /// e.g. `level_01_day_tdm` / `level_01/day_tdm`), each project once, then the PvE projects (`pve_01_v1_maze2` ->
    /// `pve_01/v1_maze2`, the level folder of the `game_modes/pve_01_*` maps). The world host daemon serves exactly
    /// these when its config names no project.
    pub fn host_projects(&self) -> Vec<(String, String)> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for r in self.matches.modes() {
            if seen.insert(r.project.clone()) {
                out.push((r.project.clone(), r.map_name.clone()));
            }
        }
        for p in &self.pve.projects {
            // `pve_01_v1_maze2`: the level folder `pve_01`, the map `v1_maze2`
            let level = p.match_indices('_').nth(1).map(|(i, _)| format!("{}/{}", &p[..i], &p[i + 1..]));
            if let Some(level) = level {
                if seen.insert(p.clone()) {
                    out.push((p.clone(), level));
                }
            }
        }
        out
    }

    /// Load every table from a data folder (written by `srp setup` / `srp extract`).
    pub fn from_dir(dir: &Path) -> Result<GameData, Error> {
        let missing = missing_files(dir);
        if !missing.is_empty() {
            return Err(Error::NoTables { dir: dir.display().to_string(), missing: missing.len(), first: missing[0] });
        }
        GameData::from_reader(&|name: &str| {
            let p = dir.join(name);
            std::fs::read_to_string(&p).map_err(|source| Error::Io { path: p.display().to_string(), source })
        })
    }

    /// Load every table through `read(file name)`.
    pub fn from_reader(read: &dyn Fn(&str) -> Result<String, Error>) -> Result<GameData, Error> {
        let (items_tsv, shop_tsv, tables_json, bots_json, names_json) = (read(ITEMS_FILE)?, read(SHOP_FILE)?, read(TABLES_FILE)?, read(BOTS_FILE)?, read(NAMES_FILE)?);
        let (items_tsv, shop_tsv, tables_json, bots_json, names_json) = (&*items_tsv, &*shop_tsv, &*tables_json, &*bots_json, &*names_json);
        let pve_bots = read("pve_bots_069d.json")?;
        let items = ItemDb::parse(items_tsv);
        let mut shop = ShopTable::build(&items, shop_tsv);
        let league = LeagueData::parse(&read("league_069d.json")?).map_err(|source| Error::Json { what: league::FILE, source })?;
        shop.apply_league(&league.shop);
        let t: RawTables = serde_json::from_str(tables_json).map_err(|source| Error::Json { what: TABLES_FILE, source })?;
        let bots: matches::RawBotData = serde_json::from_str(bots_json).map_err(|source| Error::Json { what: BOTS_FILE, source })?;
        let names: matches::RawNames = serde_json::from_str(names_json).map_err(|source| Error::Json { what: NAMES_FILE, source })?;
        let params: HashMap<u16, ItemParams> = t.items.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
        let groups: BTreeMap<u32, ModGroup> = t.mod_groups.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
        let mut modules = ModuleData::build(&items, &t.weapon_slots, &t.compatible_ammo, &t.module_links);
        modules.set_defaults(&items, &t.weapon_default_modules);
        modules.set_attach(&items, &t.module_attach, &t.weapon_slots_accepting, &t.module_detach_price);
        modules.set_fixed_mods(&t.module_mods);
        let gunrace = gunrace::GunRaceData::parse(&read("gunrace_069d.json")?).map_err(|source| Error::Json { what: gunrace::FILE, source })?;
        let mut matches = MatchData::new(
            bots,
            names,
            serde_json::from_str(&read("tutorial_069d.json")?).map_err(|source| Error::Json { what: matches::TUTORIAL_FILE, source })?,
        );
        // the gun race row comes from its own mode file (gunrace_069d.json), not from the mode row table
        matches.add_row(gunrace.row.clone());
        Ok(GameData {
            shop,
            craft: CraftData::new(params, groups),
            modules,
            levels: Levels::new(t.xp_table, t.level_rewards),
            matches,
            gunrace,
            skills: Skills::new(serde_json::from_str(&read("skills_069d.json")?).map_err(|source| Error::Json { what: skills::SKILLS_FILE, source })?),
            achievements: Achievements::new(
                serde_json::from_str(&read("achievements_069d.json")?)
                    .map_err(|source| Error::Json { what: achievements::ACHIEVEMENTS_FILE, source })?,
            ),
            badges: badges::Badges::parse(&read("badges_069d.json")?).map_err(|source| Error::Json { what: badges::FILE, source })?,
            daily_quests: DailyQuests::parse(&read("daily_quests_069d.json")?).map_err(|source| Error::Json { what: daily_quests::DAILY_QUESTS_FILE, source })?,
            drops: serde_json::from_str(&read("drops_069d.json")?).map_err(|source| Error::Json { what: "drops_069d.json", source })?,
            decals: DecalData::parse(&read("decals_069d.json")?).map_err(|source| Error::Json { what: decals::FILE, source })?,
            skins: SkinData::parse(&read("skins_069d.json")?).map_err(|source| Error::Json { what: "skins_069d.json", source })?,
            progression: Progression::parse(&read("progression_069d.json")?).map_err(|source| Error::Json { what: progression::FILE, source })?,
            vouchers: Vouchers::new(serde_json::from_str(&read("vouchers_069d.json")?).map_err(|source| Error::Json { what: vouchers::FILE, source })?, serde_json::from_str(&read("booster_costs_069d.json")?).map_err(|source| Error::Json { what: vouchers::COSTS_FILE, source })?),
            chests: ChestData::parse(&read("chests_069d.json")?).map_err(|source| Error::Json { what: "chests_069d.json", source })?,
            module_drops: module_drops::ModuleDrops::parse(&read("module_drops_069d.json")?).map_err(|source| Error::Json { what: "module_drops_069d.json", source })?,
            hunt: HuntData::parse(&read("weekly_drop_069d.json")?).map_err(|source| Error::Json { what: hunt::FILE, source })?,
            faction_battle: FactionBattleData::parse(&read("faction_battle_069d.json")?).map_err(|source| Error::Json { what: "faction_battle_069d.json", source })?,
            league,
            hero: HeroData::parse(&read("hero_of_the_day_069d.json")?).map_err(|source| Error::Json { what: hero::FILE, source })?,
            exchange: hero::ExchangeData::parse(&read("event_exchange_069d.json")?).map_err(|source| Error::Json { what: hero::EXCHANGE_FILE, source })?,
            events: events::EventData::parse(&read("event_drops_069d.json")?).map_err(|source| Error::Json { what: events::FILE, source })?,
            pve: PveData::parse(&read("pve_069d.json")?)
                .and_then(|p| p.with_bots(&pve_bots))
                .map_err(|source| Error::Json { what: pve::FILE, source })?,
            game_modes: game_modes::GameModes::parse(&read("game_modes_069d.json")?).map_err(|source| Error::Json { what: game_modes::FILE, source })?,
            texts: texts::ClientTexts::parse(&read("store_texts_069d.json")?).map_err(|source| Error::Json { what: texts::FILE, source })?,
            event_categories: result_texts::EventCategories::parse(&read("event_categories_069d.json")?)
                .map_err(|source| Error::Json { what: result_texts::CATEGORIES_FILE, source })?,
            bot_names: result_texts::BotNames::parse(&read("bot_names_069d.json")?).map_err(|source| Error::Json { what: result_texts::BOT_NAMES_FILE, source })?,
            slot_categories: t.slot_categories,
            item_categories: t.item_categories.into_iter().map(|(c, s, m)| (c, s == 1, m)).collect(),
            exchange_rates: t.exchange_currency_rates.into_iter().next().unwrap_or_default(),
            buy_specific_cost: t.buy_specific_cost,
            buy_preset_cost: t.buy_preset_cost,
            match_level_factors: t.match_levels_factors,
            one_handed: perks::one_handed_set(
                serde_json::from_str(&read("weapon_hands_069d.json")?).map_err(|source| Error::Json { what: perks::WEAPON_HANDS_FILE, source })?,
            ),
            items,
        })
    }

    /// Does profile slot `slot` take items of this dict id (category row of `profile_slots_restrictions`)?
    pub fn slot_allows(&self, slot: u8, did: u16) -> bool {
        pockets::slot_allows(&self.slot_categories, slot, self.items.category(did))
    }
}

#[cfg(test)]
mod tests {
    use super::GameData;

    #[test]
    fn a_folder_without_tables_says_run_setup() {
        let e = GameData::from_dir(std::path::Path::new("/no/such/dir")).unwrap_err().to_string();
        assert!(e.contains("srp setup") && e.contains("32 of the files missing"), "{e}");
    }

    /// The world host's projects come from the data: the 107 PvP rows whose level ships in the client, the four Search and
    /// Destroy layouts (`sad_abandoned_base` keeps its folder-less level name: the exe loads it),
    /// the gun race's `arena_01` (level `arena_01/default`), then the six PvE mazes.
    #[test]
    fn the_host_projects_are_the_shipped_pvp_rows_and_the_pve_mazes() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        let p = gd.host_projects();
        assert_eq!(p.len(), 107 + 4 + 1 + 6);
        assert!(p.contains(&("sad_abandoned_base".to_string(), "sad_abandoned_base".to_string())));
        assert!(p.contains(&("level_02_day_sad_v3".to_string(), "level_02/day_sad_v3".to_string())));
        assert!(p.contains(&("level_01_day_ah".to_string(), "level_01/day_ah".to_string())));
        assert!(p.contains(&("pve_01_v1_maze2".to_string(), "pve_01/v1_maze2".to_string())));
        assert!(p.contains(&("pve_01_v3_maze1".to_string(), "pve_01/v3_maze1".to_string())));
        assert!(p.contains(&("arena_01".to_string(), "arena_01/default".to_string())));
        assert!(p.iter().all(|(n, l)| *n == l.replace('/', "_") || format!("{n}/default") == *l));
    }
}
