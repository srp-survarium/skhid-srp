//! Chests: what a chest item gives when it is opened (`46 06`).
//!
//! Data: `data/chests_069d.json`, made by `crates/data-extract` from the client's own files:
//!
//! * the chest items (`gameplay/game.options` of volume 75, section `ammo_grenades_etc`, category 31) carry
//!   `extraction_type`, `drop_type` (the key of a drop table), `items_count` and `items` (PROVEN data);
//! * the drop tables are `gameplay/drops/chest_drops.options` (volume 67): per table a list of groups
//!   `{drop_weight, categories, items_dict_ids, rarity [min, max] amounts}`; the group weights of every table add up
//!   to 100 (PROVEN by the data).
//!
//! The client never rolls a chest (it only shows the server's `83 46 06` answer, docs/notes/items.md section 3), and the original
//! server is not available, so the roll below is built from the data and every rule that the data does not state is
//! marked CHOICE:
//!
//! * extraction type 2 with a fixed `items` list and no `drop_type`: every listed item with its amount (LIKELY: the
//!   list carries real amounts, e.g. the seven-day chests "3 x voucher 853, 3 x booster 342, 3 x booster 339", and
//!   there is no table to roll);
//! * extraction type 3 with a table: `items_count` groups are drawn by their `drop_weight`, without putting a drawn
//!   group back (LIKELY: `event_chest_drop` has `items_count` 6 and six groups, one per armour piece of a set, and the
//!   rare-module chests 10 / 10; drawing with replacement could give a set piece twice). With `items_count` missing, 1;
//! * the item of a group is picked uniformly among its `items_dict_ids` (CHOICE: the per-item `drop_weight` of the
//!   dictionary is 110..160 style sort data and 0 for the modules, not a probability), the amount uniformly in the
//!   group's `[min, max]` (CHOICE; the four rarity buckets of every group carry the same range, so `rarity_points`
//!   cannot change the outcome and is not stored);
//! * a group without `items_dict_ids` (the four `rank_N_equip_chest` tables: "weapon / armour of level 10..8") needs
//!   the mapping of the 11 item-level columns to the dictionary's item levels 1..5, which no file states: OPEN, those
//!   chests are refused ([`OpenError::Unresolved`]), as are chests whose table is missing from `chest_drops.options`
//!   (pve mission rank 2x..5x);
//! * a table whose group weights are all 0 (the three premium ammo chests: one group, weight 0, ids = the eight boxes of
//!   the chest's generation): every group counts the same (CHOICE: the chest item's own `items` list names the same
//!   eight boxes x 1 with `items_count` 1, so the chest gives one random box of its generation).

use std::collections::HashMap;

use serde::Deserialize;

use crate::craft::Roll;

/// Chest items are dictionary category 31.
pub const CHEST_CATEGORY: u16 = 31;

#[derive(Debug, Clone, Deserialize)]
struct RawChest {
    name: String,
    extraction: Option<u32>,
    drop_type: Option<String>,
    items_count: Option<u32>,
    items: Vec<(u16, u32)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    pub weight: f64,
    pub categories: Vec<u16>,
    pub ids: Option<Vec<u16>>,
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DropTable {
    pub levels: Vec<f64>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
struct RawChests {
    chests: HashMap<String, RawChest>,
    tables: HashMap<String, DropTable>,
}

/// One result of a chest: `amount` units of dictionary item `dict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reward {
    pub dict: u16,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenError {
    #[error("not a chest")]
    NotAChest,
    #[error("drop table {0:?} is not in chest_drops.options")]
    NoTable(String),
    #[error("chest has no drop table and no fixed items")]
    NoContent,
    #[error("table {0:?}: groups without item ids (item-level columns are not mapped)")]
    Unresolved(String),
    #[error("unknown extraction type {0}")]
    Extraction(u32),
}

#[derive(Debug)]
pub struct ChestData {
    chests: HashMap<u16, RawChest>,
    tables: HashMap<String, DropTable>,
}

impl ChestData {
    pub fn parse(json: &str) -> Result<ChestData, serde_json::Error> {
        let raw: RawChests = serde_json::from_str(json)?;
        let chests = raw.chests.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
        Ok(ChestData { chests, tables: raw.tables })
    }

    pub fn is_chest(&self, dict: u16) -> bool {
        self.chests.contains_key(&dict)
    }

    pub fn name(&self, dict: u16) -> Option<&str> {
        self.chests.get(&dict).map(|c| c.name.as_str())
    }

    pub fn table(&self, name: &str) -> Option<&DropTable> {
        self.tables.get(name)
    }

    /// Chests this server can open (a roll is possible), by dictionary id.
    pub fn openable(&self) -> Vec<u16> {
        let mut v: Vec<u16> = self.chests.keys().copied().filter(|d| self.check(*d).is_ok()).collect();
        v.sort_unstable();
        v
    }

    /// Can chest `dict` be rolled now (a drop table or fixed items exist)?
    pub fn can_open(&self, dict: u16) -> bool {
        self.check(dict).is_ok()
    }

    fn check(&self, dict: u16) -> Result<(), OpenError> {
        let c = self.chests.get(&dict).ok_or(OpenError::NotAChest)?;
        match (c.extraction, &c.drop_type) {
            (Some(2), _) if !c.items.is_empty() => Ok(()),
            (Some(3), Some(t)) => {
                let tab = self.tables.get(t).ok_or_else(|| OpenError::NoTable(t.clone()))?;
                if tab.groups.iter().any(|g| g.ids.as_ref().map_or(true, |i| i.is_empty())) {
                    return Err(OpenError::Unresolved(t.clone()));
                }
                Ok(())
            }
            (Some(e), _) if e != 2 && e != 3 => Err(OpenError::Extraction(e)),
            _ => Err(OpenError::NoContent),
        }
    }

    /// Roll one opening of chest `dict`. The rewards are merged per dictionary id.
    pub fn open(&self, dict: u16, rng: &mut dyn Roll) -> Result<Vec<Reward>, OpenError> {
        self.check(dict)?;
        let c = &self.chests[&dict];
        let mut out: Vec<Reward> = Vec::new();
        let mut add = |r: Reward| match out.iter_mut().find(|x| x.dict == r.dict) {
            Some(x) => x.amount += r.amount,
            None => out.push(r),
        };
        if c.extraction == Some(2) {
            for &(d, a) in &c.items {
                add(Reward { dict: d, amount: a });
            }
            return Ok(out);
        }
        let tab = &self.tables[c.drop_type.as_ref().unwrap()];
        // all weights 0: every group the same (module docs)
        let flat = tab.groups.iter().all(|g| g.weight <= 0.0);
        let weight = |g: &Group| if flat { 1.0 } else { g.weight };
        let mut open: Vec<&Group> = tab.groups.iter().filter(|g| weight(g) > 0.0).collect();
        let draws = c.items_count.unwrap_or(1).max(1) as usize;
        for _ in 0..draws.min(open.len()) {
            let total: f64 = open.iter().map(|g| weight(g)).sum();
            let mut x = rng.unit() * total;
            let mut pick = open.len() - 1;
            for (i, g) in open.iter().enumerate() {
                if x < weight(g) {
                    pick = i;
                    break;
                }
                x -= weight(g);
            }
            let g = open.remove(pick);
            let ids = g.ids.as_ref().unwrap();
            let id = ids[rng.below(ids.len() as u32) as usize];
            let span = g.max.saturating_sub(g.min) + 1;
            add(Reward { dict: id, amount: g.min + rng.below(span) });
        }
        Ok(out)
    }
}
