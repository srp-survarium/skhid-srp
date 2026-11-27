//! The client dictionary (data/items_069d.tsv, made from `gameplay/game.options`): one row per dict id,
//! and the category rules the lobby applies.

use std::collections::{BTreeMap, HashMap};

use crate::tsv::Table;

/// helmet .. boots: slot = category - 1 (categories 56/57 are the premium helmet/mask rows of slots 0/1)
pub const ARMOUR_CATS: [u16; 7] = [1, 2, 3, 4, 5, 6, 7];
/// rows with a base item id (rifles, SMG, bolt, shotguns, pistols, carbines)
pub const WEAPON_CATS: [u16; 8] = [8, 13, 14, 15, 16, 17, 36, 40];
pub const AMMO_CATS: [u16; 5] = [9, 18, 19, 20, 21];
/// premium ammo boxes (`ammo_booster`): 37 for the primary weapon (profile slot 11), 38 for the pistol (slot 12)
pub const AMMO_BOOSTER_CATS: [u16; 2] = [37, 38];
/// stackable categories of the quick slots: 10 drug, 11 mine, 24 grenade, 34 tactical, 54 resupply, 55 trap
pub const CONSUMABLE_CATS: [u16; 6] = [10, 11, 24, 34, 54, 55];
/// what may sit in a pocket (quick slot): the categories with a pocket tab (`pocket_slots_restrictions`, `pockets::GROUPS`).
/// `profile_slots_restrictions` also allows 23 (premium consumables / vouchers), but no pocket tab has it: the lobby
/// movie's `getTabByCategory(23)` returns null and the slots selector crashes on it (LIKELY, same path as the pocket
/// crash), so 23 is refused.
pub const QUICK_CATS: [u16; 6] = [10, 11, 24, 34, 54, 55];
/// weapon modules (workshop attachments)
pub const MODULE_CATS: std::ops::RangeInclusive<u16> = 41..=53;

#[derive(Debug, Clone)]
pub struct Item {
    pub dict_id: u16,
    pub cfg_name: String,
    pub section: String,
    pub category: u16,
    /// item kind string (`wpn_aslt`, `arm_tors`, ...), empty for some items
    pub kind: String,
    pub item_level: u32,
    /// dictionary faction id (1 Scavengers, 2 Black Market, 3 Renaissance Army, 4 Fringe, 0 none)
    pub faction: u32,
    pub match_making_level: u32,
    /// raw `cost` cell (camo, decals, modules, ammo, grenades only)
    pub cost: String,
    /// raw `currency` cell (0 silver, 4 spare parts for modules)
    pub currency: String,
    /// `base_item_dict_id` (weapons)
    pub base_item: Option<u32>,
    pub weight: f64,
    pub is_stackable: bool,
    /// `data.type` of the item config (None when the item has no `data` block). The match's inventory cook 0x2002D8320
    /// picks the in-match item class by it: 9 = grenade set, 2, 3, 13, 14, 15, 16 = placeable sets, others generic.
    pub data_type: Option<u32>,
    /// `parameters.stack_size` (default 1 as in the exe, 0x2002D841A): units per slot count.
    pub stack_size: u32,
}

impl Item {
    /// The dictionary's own silver price: a non-zero `cost` with currency "" or "0".
    pub fn own_silver_cost(&self) -> Option<u32> {
        if !self.cost.is_empty() && self.cost != "0" && (self.currency.is_empty() || self.currency == "0") {
            crate::tsv::int_of_float(&self.cost).map(|v| v as u32)
        } else {
            None
        }
    }

    /// `cost` as read for modules: the number truncated to an integer, 0 when empty.
    pub fn cost_int(&self) -> u32 {
        if self.cost.is_empty() {
            0
        } else {
            crate::tsv::int_of_float(&self.cost).unwrap_or(0) as u32
        }
    }

    /// `max(1, item_level)`
    pub fn level_min1(&self) -> u32 {
        self.item_level.max(1)
    }

    /// premium (gold) weapon rows end with `_premium`
    pub fn is_premium(&self) -> bool {
        self.cfg_name.ends_with("_premium")
    }
}

#[derive(Debug, Default)]
pub struct ItemDb {
    by_id: BTreeMap<u16, Item>,
    by_cfg: HashMap<String, u16>,
}

fn num<T: std::str::FromStr + Default>(s: &str) -> T {
    s.trim().parse().unwrap_or_default()
}

impl ItemDb {
    pub fn parse(tsv: &str) -> ItemDb {
        let t = Table::parse(tsv);
        let mut db = ItemDb::default();
        for r in &t.rows {
            let g = |n: &str| t.get(r, n);
            let base = g("base_item_dict_id");
            let item = Item {
                dict_id: num(g("dict_id")),
                cfg_name: g("cfg_name").to_string(),
                section: g("section").to_string(),
                category: num(g("category_id")),
                kind: g("type").to_string(),
                item_level: num(g("item_level")),
                faction: num(g("faction")),
                match_making_level: num(g("match_making_level")),
                cost: g("cost").to_string(),
                currency: g("currency").to_string(),
                base_item: if base.is_empty() { None } else { Some(num(base)) },
                weight: num(g("weight")),
                is_stackable: g("is_stackable") == "1",
                data_type: g("data_type").trim().parse().ok(),
                stack_size: g("stack_size").trim().parse().unwrap_or(1),
            };
            db.by_cfg.insert(item.cfg_name.clone(), item.dict_id);
            db.by_id.insert(item.dict_id, item);
        }
        db
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, did: u16) -> Option<&Item> {
        self.by_id.get(&did)
    }

    pub fn contains(&self, did: u16) -> bool {
        self.by_id.contains_key(&did)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        self.by_id.values()
    }

    /// dict id of a config name (`mosin_nagant`); the last row wins if a name repeats
    pub fn by_cfg_name(&self, name: &str) -> Option<u16> {
        self.by_cfg.get(name).copied()
    }

    /// category id, 0 when the item is unknown or has none
    pub fn category(&self, did: u16) -> u16 {
        self.get(did).map(|i| i.category).unwrap_or(0)
    }

    pub fn is_stackable(&self, did: u16) -> bool {
        self.get(did).map(|i| i.is_stackable).unwrap_or(false)
    }

    pub fn is_armour(&self, did: u16) -> bool {
        ARMOUR_CATS.contains(&self.category(did))
    }

    pub fn is_weapon(&self, did: u16) -> bool {
        WEAPON_CATS.contains(&self.category(did))
    }

    pub fn is_ammo(&self, did: u16) -> bool {
        AMMO_CATS.contains(&self.category(did))
    }

    pub fn is_consumable(&self, did: u16) -> bool {
        CONSUMABLE_CATS.contains(&self.category(did))
    }

    /// a premium ammo box (`ammo_booster`, categories 37 / 38)
    pub fn is_ammo_booster(&self, did: u16) -> bool {
        AMMO_BOOSTER_CATS.contains(&self.category(did))
    }

    /// The box category a weapon takes: the lobby movie's `ItemFactory.getItemAmmoBoostersCategory` (db 75 ABC):
    /// no weapon (`isWeapon` = super category 2 = categories 8, 13..17, 36) -> none, category 17 (pistols) -> 38,
    /// every other weapon -> 37. The box slot of a weapon slot (7 -> 11, 10 -> 12) takes only this category; the
    /// movie's selector lists only it and `LocalPaperDoll.updateBoosterSlots` sends any other box back to slot 100.
    pub fn ammo_booster_category(&self, weapon: u16) -> Option<u16> {
        match self.get(weapon)?.category {
            17 => Some(38),
            8 | 13..=16 | 36 => Some(37),
            _ => None,
        }
    }

    /// may sit in a pocket (quick slot): the categories with a pocket tab
    pub fn is_quick_item(&self, did: u16) -> bool {
        QUICK_CATS.contains(&self.category(did))
    }

    /// a workshop module: a dictionary item of categories 41..=53
    pub fn is_module(&self, did: u16) -> bool {
        self.contains(did) && MODULE_CATS.contains(&self.category(did))
    }

    /// The u32 at record +0xC of 87/89 item records: stacks = count; every other item = its ORIGINAL
    /// dict id (weapons: `base_item_dict_id`). Unknown dict ids give their own id.
    pub fn amount_field(&self, did: u16, count: u32) -> u32 {
        match self.get(did) {
            Some(r) if r.is_stackable => count,
            Some(Item { base_item: Some(b), .. }) => *b,
            _ => did as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    /// `getItemAmmoBoostersCategory` of the lobby movie: pistols (17) take 38, the other weapons 37, nothing else a box
    #[test]
    fn ammo_booster_category_follows_the_lobby_movie() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        let it = &gd.items;
        // Mosin (107), TT-33 (18, pistol), TOZ-66 (17, sawn-off, category 17), TOZ-34 (16, shotgun, category 14)
        for (weapon, want) in [(107u16, Some(37u16)), (18, Some(38)), (17, Some(38)), (16, Some(37))] {
            assert_eq!(it.ammo_booster_category(weapon), want, "weapon {weapon} category {}", it.category(weapon));
        }
        // ammo, armour, a box itself, an unknown id: no box
        for d in [104u16, 31, 1281, 0xFFFF] {
            assert_eq!(it.ammo_booster_category(d), None, "dict {d}");
        }
    }
}
