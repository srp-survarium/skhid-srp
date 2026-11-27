//! The treasure hunt ("item search", the weekly drop roulette): what the client's `weekly_drop_configs` say and how a
//! page is rolled from them.
//!
//! Data: `data/weekly_drop_069d.json` (`crates/data-extract` from `gameplay/drops/drops.options`).
//!
//! PROVEN (exe + AS3, docs/notes/items.md section 3a):
//! * a config has `drop_by_level[level]` = a list of pages, a page = `{items, roulette_prob}`; the client indexes
//!   `drop_by_level[current level][page index]` (0x20024E5EF..0x20024E63B) and reads per item `dict_id`, `min`, `prob`,
//!   `rare_level`, `item_quality` (0x20024E75F..0x20024E9F6);
//! * `drop_cards_to_open` / `premium_money_to_open` / `max_free_drop_cards(_in_period)` / `can_change_drop_level` are
//!   read as shown (0x20024E33F, 0x20024EB6D, 0x200188D93 uses the constant 2 for the free cards).
//!
//! Rules the data and the client do not state (CHOICE, noted at each place): a page is picked by its `roulette_prob`
//! (they add up to 1 per level), an item of a page by its `prob` among the items not yet found, a `dict_id` list is
//! one concrete id picked uniformly when the page is made, the amount is uniform in `[min, max]`.

use std::collections::HashMap;

use serde::Deserialize;

use crate::craft::Roll;

pub const FILE: &str = "weekly_drop_069d.json";

/// Config used by the server (`weekly_drop`); the other config in the file is the 2018 New Year event.
pub const DEFAULT_CONFIG: &str = "weekly_drop";

/// Free drop card item (`drop_card_free`) and paid / bonus drop card item (`drop_card`).
pub const FREE_CARD: u16 = 1132;
pub const PAID_CARD: u16 = 153;

#[derive(Debug, Clone, Deserialize)]
pub struct PageItem {
    /// one id, or the list the page item is picked from when the page is made
    pub ids: Vec<u16>,
    pub min: u32,
    pub max: u32,
    pub prob: f64,
    pub rare_level: u8,
    pub item_quality: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Page {
    pub roulette_prob: f64,
    pub items: Vec<PageItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HuntConfig {
    pub max_free_drop_cards_in_period: u8,
    pub max_free_drop_cards: u8,
    pub can_change_drop_level: bool,
    pub drop_cards_to_open: u32,
    pub premium_money_to_open: u32,
    /// index = roulette level; level 0 is empty
    pub levels: Vec<Vec<Page>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RareLevel {
    pub text: String,
    pub color: u32,
}

#[derive(Debug, Deserialize)]
struct Raw {
    configs: HashMap<String, HuntConfig>,
    rare_levels: Vec<RareLevel>,
}

#[derive(Debug)]
pub struct HuntData {
    configs: HashMap<String, HuntConfig>,
    pub rare_levels: Vec<RareLevel>,
}

impl HuntData {
    pub fn parse(json: &str) -> Result<HuntData, serde_json::Error> {
        let raw: Raw = serde_json::from_str(json)?;
        Ok(HuntData { configs: raw.configs, rare_levels: raw.rare_levels })
    }

    pub fn config(&self, name: &str) -> Option<&HuntConfig> {
        self.configs.get(name)
    }

    pub fn default_config(&self) -> Option<&HuntConfig> {
        self.config(DEFAULT_CONFIG)
    }
}

impl HuntConfig {
    /// Lowest roulette level that has pages (1).
    pub fn min_level(&self) -> u8 {
        self.levels.iter().position(|p| !p.is_empty()).unwrap_or(0) as u8
    }

    /// Highest roulette level of the data (5).
    pub fn top_level(&self) -> u8 {
        self.levels.iter().rposition(|p| !p.is_empty()).unwrap_or(0) as u8
    }

    pub fn page_count(&self, level: u8) -> usize {
        self.levels.get(level as usize).map_or(0, |p| p.len())
    }

    /// Pick a page of `level` by `roulette_prob` (CHOICE: the probabilities of a level add up to 1, they are used
    /// as weights).
    pub fn pick_page(&self, level: u8, r: &mut dyn Roll) -> Option<u8> {
        let pages = self.levels.get(level as usize).filter(|p| !p.is_empty())?;
        let total: f64 = pages.iter().map(|p| p.roulette_prob).sum();
        let mut x = r.unit() * total;
        for (i, p) in pages.iter().enumerate() {
            if x < p.roulette_prob {
                return Some(i as u8);
            }
            x -= p.roulette_prob;
        }
        Some((pages.len() - 1) as u8)
    }

    pub fn page(&self, level: u8, page: u8) -> Option<&Page> {
        self.levels.get(level as usize)?.get(page as usize)
    }

    /// The concrete dictionary id of every item of a page (a list is a pool: CHOICE uniform).
    pub fn resolve_page(&self, level: u8, page: u8, r: &mut dyn Roll) -> Vec<u16> {
        self.resolve_page_skipping(level, page, &|_| false, r).0
    }

    /// [`Self::resolve_page`] that draws a pool id for which `skip` is true only when every id of that pool is skipped
    /// (then the plain draw, CHOICE). The second value counts the entries where a skipped id was avoided. With nothing
    /// skipped the draws are the same as [`Self::resolve_page`]'s.
    pub fn resolve_page_skipping(&self, level: u8, page: u8, skip: &dyn Fn(u16) -> bool, r: &mut dyn Roll) -> (Vec<u16>, usize) {
        let Some(p) = self.page(level, page) else { return (Vec::new(), 0) };
        let mut avoided = 0;
        let ids = p
            .items
            .iter()
            .map(|it| {
                let open: Vec<u16> = it.ids.iter().copied().filter(|d| !skip(*d)).collect();
                let pool = if open.is_empty() { &it.ids[..] } else { &open[..] };
                if !open.is_empty() && open.len() < it.ids.len() {
                    avoided += 1;
                }
                pool[r.below(pool.len() as u32) as usize]
            })
            .collect();
        (ids, avoided)
    }

    /// Pick the item found by one search among the ones not found yet, weighted by `prob`
    /// (CHOICE: the probabilities of a page add up to 0.93..1.0 and are used as weights).
    pub fn pick_item(&self, level: u8, page: u8, found: &[bool], r: &mut dyn Roll) -> Option<usize> {
        let p = self.page(level, page)?;
        let open: Vec<usize> = (0..p.items.len()).filter(|i| !found.get(*i).copied().unwrap_or(false)).collect();
        if open.is_empty() {
            return None;
        }
        let total: f64 = open.iter().map(|&i| p.items[i].prob).sum();
        if total <= 0.0 {
            return Some(open[r.below(open.len() as u32) as usize]);
        }
        let mut x = r.unit() * total;
        for &i in &open {
            if x < p.items[i].prob {
                return Some(i);
            }
            x -= p.items[i].prob;
        }
        open.last().copied()
    }

    /// Amount of a found item: uniform in `[min, max]`.
    pub fn amount(&self, level: u8, page: u8, item: usize, r: &mut dyn Roll) -> u32 {
        match self.page(level, page).and_then(|p| p.items.get(item)) {
            Some(it) if it.max > it.min => it.min + r.below(it.max - it.min + 1),
            Some(it) => it.min,
            None => 0,
        }
    }
}
