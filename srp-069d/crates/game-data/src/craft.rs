//! Workshop crafting rules (docs/notes/workshop.md): item-aware modifier pools, rank costs, reroll
//! costs, maximum rank, exchange. Data: `items[dict id]` parameters and `mod_groups` of game.options.
//!
//! Rules from the client data: a modifier group is allowed on an item when its `type` is in the item's
//! `available_modifications_mask` (LIKELY, server rule); one group per type; types already on the item
//! are excluded; grade by `drop_weight`. Rank = `min(3, base_rarity_level + modifiers on the item)`;
//! rank 3 is never craftable. Costs are `details_to_craft` / `premium_to_craft` indexed by rank.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Deserialize;

/// Success chance in percent paid with spare parts (the "free" button) / gold: push 9B, shown by `CraftItem` and by
/// `ModuleUpgradePanel` (one pair for both windows: PROVEN, docs/notes/workshop.md section 1) and rolled by the
/// server. Values: fan wiki "Workshop", row "Same Rarity", snapshot 2022-05-27: 50 % with the ordinary currency
/// (the "Silver" column), 100 % with gold. NOT in the client or the game data (the push is the only carrier);
/// whether the 2022 table matches 0.69d is LIKELY, not checked. The outcome of a missed roll: result 0 = "neutral"
/// (`craft_result_neutral_label`), see `crate::merge`.
pub const CRAFT_CHANCE: (f32, f32) = (50.0, 100.0);
/// Offered rows per craft or reroll (the UI list size).
pub const OFFER_ROWS: usize = 3;
/// Highest rank: an item at rank 3 cannot be upgraded further.
pub const MAX_RANK: u32 = 3;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ItemParams {
    #[serde(default)]
    pub base_rarity_level: u32,
    /// indices of the set flags of `available_modifications_mask` (23 slot classes)
    #[serde(default)]
    pub mask: Vec<u32>,
    #[serde(default)]
    pub details_to_craft: Vec<u32>,
    #[serde(default)]
    pub premium_to_craft: Vec<u32>,
    #[serde(default)]
    pub details_to_reroll: u32,
    #[serde(default)]
    pub premium_to_reroll: u32,
    /// pocket group -> cells per unit (consumables)
    #[serde(default)]
    pub pocket_contains: BTreeMap<String, u32>,
    /// (modification group, value), e.g. (173, 1) = one pocket slot
    #[serde(default)]
    pub default_modifications: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModGroup {
    /// slot class (0..22)
    #[serde(rename = "type")]
    pub kind: u32,
    /// (grade, drop weight)
    pub grades: Vec<(u32, f64)>,
}

/// Source of randomness for the pickers (the server passes a real generator, tests a fixed one).
pub trait Roll {
    /// uniform in `0..n` (n > 0)
    fn below(&mut self, n: u32) -> u32;
    /// uniform in `[0, 1)`
    fn unit(&mut self) -> f64;
}

/// Small deterministic generator (SplitMix64) for tests and seeds.
pub struct SplitMix(pub u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl Roll for SplitMix {
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n.max(1) as u64) as u32
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Debug, Default)]
pub struct CraftData {
    items: HashMap<u16, ItemParams>,
    groups: BTreeMap<u32, ModGroup>,
}

/// What the upgrade costs and whether it is allowed (45 07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftQuote {
    pub rank: u32,
    pub price: u32,
    /// rank < 3, a price exists and the offer list is not empty is checked by the caller with `choices`
    pub allowed: bool,
}

impl CraftData {
    pub(crate) fn new(items: HashMap<u16, ItemParams>, groups: BTreeMap<u32, ModGroup>) -> CraftData {
        CraftData { items, groups }
    }

    pub fn params(&self, did: u16) -> Option<&ItemParams> {
        self.items.get(&did)
    }

    pub fn group(&self, group: u32) -> Option<&ModGroup> {
        self.groups.get(&group)
    }

    pub fn groups(&self) -> impl Iterator<Item = (u32, &ModGroup)> {
        self.groups.iter().map(|(g, m)| (*g, m))
    }

    /// Client rank: `min(3, base_rarity_level + modifiers on the record)`; 3 = not craftable.
    pub fn rank(&self, did: u16, n_mods: usize) -> u32 {
        let base = self.params(did).map(|p| p.base_rarity_level).unwrap_or(0);
        (base as usize + n_mods).min(MAX_RANK as usize) as u32
    }

    /// Upgrades still possible on a fresh item (3 for rank 0, 0 for rank-3 items).
    pub fn max_upgrades(&self, did: u16) -> u32 {
        MAX_RANK - self.rank(did, 0)
    }

    /// Price of the next upgrade: `details_to_craft` (spare parts) or `premium_to_craft` (gold) by rank;
    /// 0 at rank 3 or when the item has no cost row. The same as `CraftItem` shows (PROVEN, docs/notes/workshop.md
    /// section 1): `root.fill_item_craft_data` (0x20028CA90) sends the item's own `details_to_craft` unchanged and its
    /// `premium_to_craft` as `trunc(p * factor + 0.5)`, factor 1.0 unless a 94 notice of kind 0x24 / 0x25 / 0x26 sets
    /// 0.5 / 0.7 / 1.5 (the server sends none), and `CraftItem` indexes both by `ItemData.rank`.
    pub fn craft_price(&self, did: u16, n_mods: usize, gold: bool) -> u32 {
        let rank = self.rank(did, n_mods);
        if rank >= MAX_RANK {
            return 0;
        }
        let Some(p) = self.params(did) else { return 0 };
        let row = if gold { &p.premium_to_craft } else { &p.details_to_craft };
        if row.is_empty() {
            return 0;
        }
        row[(rank as usize).min(row.len() - 1)]
    }

    pub fn craft_quote(&self, did: u16, n_mods: usize, gold: bool) -> CraftQuote {
        let rank = self.rank(did, n_mods);
        let price = self.craft_price(did, n_mods, gold);
        CraftQuote { rank, price, allowed: rank < MAX_RANK && price > 0 }
    }

    /// Reroll cost (45 05): `premium_to_reroll` gold or `details_to_reroll` spare parts.
    pub fn reroll_price(&self, did: u16, gold: bool) -> u32 {
        self.params(did)
            .map(|p| if gold { p.premium_to_reroll } else { p.details_to_reroll })
            .unwrap_or(0)
    }

    /// Gold price of a "premium" row of a spare-parts reroll (45 06). The client shows `premium_cost` =
    /// `premium_to_reroll * f + 0.5` truncated (PROVEN 0x20028CF76..0x20028CFD8), f = lobby +0x7A52E4 = 1.0 unless a
    /// push 94 record of type 0x24..0x26 sets a sale factor; this server sends no such record, so the price is
    /// `premium_to_reroll`.
    pub fn reroll_premium_price(&self, did: u16) -> u32 {
        self.reroll_price(did, true)
    }

    /// Slot classes (types) an item accepts.
    pub fn mask(&self, did: u16) -> BTreeSet<u32> {
        self.params(did).map(|p| p.mask.iter().copied().collect()).unwrap_or_default()
    }

    /// Candidate groups per slot class: types in the item's mask, minus the types of its current modifiers
    /// (the pair `keep` being rerolled does not count as held). Groups sorted.
    pub fn candidates(&self, did: u16, mods: &[(u16, u16)], keep: Option<(u16, u16)>) -> BTreeMap<u32, Vec<u32>> {
        let mask = self.mask(did);
        let used: BTreeSet<u32> = mods
            .iter()
            .filter(|m| Some(**m) != keep)
            .filter_map(|(g, _)| self.groups.get(&(*g as u32)).map(|e| e.kind))
            .collect();
        let mut by_type: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (g, e) in &self.groups {
            if mask.contains(&e.kind) && !used.contains(&e.kind) {
                by_type.entry(e.kind).or_default().push(*g);
            }
        }
        by_type
    }

    /// Up to 3 offered (group, grade) pairs, sorted: 3 random types, one random group each, grade by weight.
    pub fn choices(&self, did: u16, mods: &[(u16, u16)], keep: Option<(u16, u16)>, rng: &mut dyn Roll) -> Vec<(u16, u16)> {
        let by_type = self.candidates(did, mods, keep);
        let mut types: Vec<u32> = by_type.keys().copied().collect();
        // partial Fisher-Yates: the first `take` entries are a random sample
        let take = OFFER_ROWS.min(types.len());
        for i in 0..take {
            let j = i + rng.below((types.len() - i) as u32) as usize;
            types.swap(i, j);
        }
        let mut out = Vec::new();
        for t in &types[..take] {
            let gs = &by_type[t];
            let grp = gs[rng.below(gs.len() as u32) as usize];
            let e = &self.groups[&grp];
            let total: f64 = e.grades.iter().map(|g| g.1).sum();
            let mut r = rng.unit() * total;
            let mut grade = e.grades.last().map(|g| g.0).unwrap_or(0);
            for (gr, w) in &e.grades {
                if r < *w {
                    grade = *gr;
                    break;
                }
                r -= *w;
            }
            out.push((grp as u16, grade as u16));
        }
        out.sort_unstable();
        out
    }

    /// `n` modifiers for a new item, one slot type each: every step offers [`Self::choices`] (3 random allowed types
    /// not on the item yet, a random group of each, grade by `drop_weight`) and takes one of them at random. Fewer
    /// when the item allows fewer types. Used for the hidden modifiers of a treasure-hunt item (`item_quality`,
    /// CHOICE: the workshop rules, the original server's roll is in no file).
    pub fn roll_mods(&self, did: u16, n: usize, rng: &mut dyn Roll) -> Vec<(u16, u16)> {
        let mut mods = Vec::new();
        for _ in 0..n {
            let offer = self.choices(did, &mods, None, rng);
            if offer.is_empty() {
                break;
            }
            mods.push(offer[rng.below(offer.len() as u32) as usize]);
        }
        mods
    }

    /// Reroll list (45 05): the 3 rows, padded with the last row when fewer exist; the old pair when none.
    pub fn reroll_choices(&self, did: u16, mods: &[(u16, u16)], old: (u16, u16), rng: &mut dyn Roll) -> Vec<(u16, u16)> {
        let mut offer = self.choices(did, mods, Some(old), rng);
        while !offer.is_empty() && offer.len() < OFFER_ROWS {
            offer.push(*offer.last().unwrap());
        }
        if offer.is_empty() {
            offer = vec![old; OFFER_ROWS];
        }
        offer
    }

    /// Does a craft roll succeed? `chance` in percent (CRAFT_CHANCE).
    pub fn roll_success(chance: f32, rng: &mut dyn Roll) -> bool {
        rng.unit() * 100.0 < chance as f64
    }
}

/// Silver per gold in the exchange window (`exchange_currency_rates`, all 250) and the 8D `+0x28` rate.
pub const EXCHANGE_RATE: u32 = 250;

/// 46 03: exchange `gold_units` (clamped to the gold held); returns (gold spent, silver gained).
pub fn exchange(gold_units: u32, gold_held: u32) -> (u32, u64) {
    let n = gold_units.min(gold_held);
    (n, n as u64 * EXCHANGE_RATE as u64)
}
