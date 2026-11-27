//! Pockets (quick slots with consumables).
//!
//! Quick slot ids 13..=20 exist in the 87 table, the lobby draws 13..=17. Group limits come from
//! `gameplay/pocket_slots_restrictions.options` (volume 00); which group an item belongs to and its cells
//! per unit are `parameters.pocket_contains` of the item. How many slots and cells are open comes from the
//! equipped armour: its `default_modifications` carry (173, n) = n pocket slots and (174, n) = n cells in total
//! (plus crafted modifications of the same groups; LIKELY, the client sums modifiers 62 and 63).

use crate::craft::CraftData;

pub const QUICK_SLOT_FIRST: u8 = 13;
/// the last slot the lobby draws
pub const QUICK_SLOT_DRAWN_LAST: u8 = 17;
/// the last slot of the 87 table
pub const QUICK_SLOT_LAST: u8 = 20;
/// first and last pocket slot as a range of the 87 table
pub const QUICK_SLOTS: std::ops::RangeInclusive<u8> = QUICK_SLOT_FIRST..=QUICK_SLOT_LAST;

/// modification group: number of pocket slots an armour opens
pub const MOD_POCKET_SLOTS: u32 = 173;
/// modification group: total cells an armour opens
pub const MOD_POCKET_DEPTH: u32 = 174;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PocketGroup {
    /// name as in `pocket_contains`
    pub name: &'static str,
    /// item categories that belong to it
    pub categories: &'static [u16],
    /// most cells the group may use in all pockets together
    pub max_used_cells: u32,
}

pub const GROUPS: [PocketGroup; 4] = [
    PocketGroup { name: "drug", categories: &[10], max_used_cells: 8 },
    PocketGroup { name: "grenade", categories: &[24, 11], max_used_cells: 2 },
    PocketGroup { name: "generic", categories: &[34, 55], max_used_cells: 4 },
    PocketGroup { name: "resupply", categories: &[54], max_used_cells: 1 },
];

pub fn group_by_name(name: &str) -> Option<&'static PocketGroup> {
    GROUPS.iter().find(|g| g.name == name)
}

pub fn group_of_category(cat: u16) -> Option<&'static PocketGroup> {
    GROUPS.iter().find(|g| g.categories.contains(&cat))
}

/// Open pockets of a profile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PocketCapacity {
    pub slots: u32,
    pub cells: u32,
}

/// One filled pocket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub dict_id: u16,
    pub count: u32,
}

impl CraftData {
    /// Group and cells per unit of a consumable (`pocket_contains`), None for items without one.
    pub fn pocket_group(&self, did: u16) -> Option<(&'static PocketGroup, u32)> {
        let p = self.params(did)?;
        let (name, cells) = p.pocket_contains.iter().next()?;
        Some((group_by_name(name)?, *cells))
    }

    /// Pockets open for armour pieces (dict ids) with the extra (group, value) modifications of their records.
    pub fn pocket_capacity(&self, armour: &[u16], extra_mods: &[(u16, u16)]) -> PocketCapacity {
        let mut cap = PocketCapacity::default();
        let defaults = armour.iter().filter_map(|d| self.params(*d)).flat_map(|p| p.default_modifications.iter().copied());
        let crafted = extra_mods.iter().map(|&(g, v)| (g as u32, v as u32));
        for (g, v) in defaults.chain(crafted) {
            match g {
                MOD_POCKET_SLOTS => cap.slots += v,
                MOD_POCKET_DEPTH => cap.cells += v,
                _ => {}
            }
        }
        cap
    }

    /// May `add` (one more pocket holding `count` units of an item) be placed next to `placed`?
    /// Rules of the client popup: a free slot, total cells within the depth, group cells within `max_used_cells`.
    pub fn pocket_fits(&self, cap: PocketCapacity, placed: &[Placed], add: Placed) -> bool {
        let Some((group, cells)) = self.pocket_group(add.dict_id) else { return false };
        if placed.len() as u32 + 1 > cap.slots {
            return false;
        }
        let mut total = add.count * cells;
        let mut in_group = total;
        for p in placed {
            if let Some((g, c)) = self.pocket_group(p.dict_id) {
                total += p.count * c;
                if g.name == group.name {
                    in_group += p.count * c;
                }
            }
        }
        total <= cap.cells && in_group <= group.max_used_cells
    }
}

/// Slot restrictions: categories allowed in profile slot `slot` (row `slot` of `profile_slots_restrictions`).
pub fn slot_allows(slot_categories: &[Vec<u16>], slot: u8, category: u16) -> bool {
    slot_categories.get(slot as usize).is_some_and(|v| v.contains(&category))
}
