//! Vouchers and boosters of the dictionary (categories 27 and 23), data/vouchers_069d.json (made by
//! crates/data-extract from `game.options`). PROVEN data fields: `voucher_type` (0 rename, 1 drop
//! card, 2 repair, 3 premium access, 4 service booster, 5 PvE ticket: the client's `voucher_type` key, AS
//! `PremiumPopup` shows type 3 as premium time) and `ttl_in_hours` of the premium vouchers (24 / 48 / 72 / 168).

use serde::Deserialize;

pub const FILE: &str = "vouchers_069d.json";
/// `voucher_type` of the premium account vouchers (`premium_access_voucher_*`).
pub const PREMIUM_ACCESS: u32 = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct VoucherRow {
    pub dict_id: u16,
    pub cfg_name: String,
    pub category: u32,
    #[serde(default)]
    pub voucher_type: Option<u32>,
    #[serde(default)]
    pub ttl_in_hours: Option<u32>,
    /// boosters: (modifier group, grade), e.g. (124, 2) = experience bonus grade 2 (+50 %)
    #[serde(default)]
    pub default_modifications: Vec<(u32, u32)>,
    /// dictionary `cost` and `currency` (1 = gold, 0 = silver)
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub currency: u32,
    /// `use_variable_costs`: true for the per-match boosters 339..344 (priced by `booster_use_costs`, PROVEN: the client
    /// reads the key at 0x2001C5274), false for 853 / 857 / 861 (their own `cost` in gold, per match as well)
    #[serde(default)]
    pub use_variable_costs: bool,
    /// service boosters (`voucher_type` 4): the service id = the 94 record type the client matches (24 for 1088)
    #[serde(default)]
    pub service_id: Option<u32>,
    /// service boosters: how many matches one grant lasts (`service_duration_matches`, 3 for 1088)
    #[serde(default)]
    pub service_duration_matches: Option<u32>,
    /// resolved `default_modifications`: the modifier name and value of the group/grade (data `modifications`)
    #[serde(default)]
    pub modifiers: Vec<BoosterModifier>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoosterModifier {
    pub name: String,
    pub value: f64,
}

/// `gameplay/items/boosters/booster_parameters.options` (volume 00): `booster_use_costs[match level row][nth booster]`.
pub const COSTS_FILE: &str = "booster_costs_069d.json";

#[derive(Debug, Default, Clone, Deserialize)]
pub struct BoosterCosts {
    pub booster_use_costs: Vec<Vec<u32>>,
    /// silver of a match by the player's equipment level (index = the level, 0 = nothing worn .. 5): the "repairs" group
    /// of the client's match participation cost (0x200333210, type 2 at 0x2001BFBC0); 0 x 6 in the 0.69d data
    #[serde(default)]
    pub equipment_use_costs: Vec<u32>,
}

/// Booster slots of the client per group: the lobby panel (`survarium.boosters.BoosterPanel`) has three silver slots
/// (`slot_1..3`) and three gold slots (`slot_gold_1..3`; `getBoostersSelectionArray` lists only the boosters whose
/// `isPremium` matches the slot), and `booster_use_costs` has three columns (the column index is checked against 3 at 0x2001C559D).
pub const MAX_MATCH_BOOSTERS: usize = 3;

/// `new_player_booster` ("Newbie's blessing", `st_new_player_booster_name`): a service booster (`service_id` 24, 3 matches,
/// modifier `new_player_damage_modifier` -0.15 against anomaly-type hits). Nothing in the client or the data grants it;
/// CHOICE: every new account gets it for its first online matches (docs/notes/store.md).
pub const NEW_PLAYER_BOOSTER: u16 = 1088;

#[derive(Debug, Default)]
pub struct Vouchers {
    rows: Vec<VoucherRow>,
    costs: BoosterCosts,
}

impl Vouchers {
    pub(crate) fn new(rows: Vec<VoucherRow>, costs: BoosterCosts) -> Vouchers {
        Vouchers { rows, costs }
    }

    /// Dictionary category of the boosters.
    pub const BOOSTER_CATEGORY: u32 = 23;

    /// A per-match booster: every category 23 row. 339..344 (`use_variable_costs`: silver by `booster_use_costs`) and
    /// 853 / 857 / 861 (their own price, 5 gold). The `_24h_` in the last three's config names is not what the client
    /// does: their descriptions say "for one match" (`st_experience_booster_24h_description` = "Increases experience gain
    /// for one match", the silver and reputation ones alike), and the client counts them into the match participation
    /// cost (`booster_participation_costs` 0x2001BF460 -> "Match costs", `root.set_match_participation_cost` 0x2001BFBC0).
    pub fn is_match_booster(&self, dict: u16) -> bool {
        self.get(dict).is_some_and(|r| r.category == Self::BOOSTER_CATEGORY)
    }

    /// A booster of the panel's gold slots (dictionary `currency` 1: 853 / 857 / 861); the others use the silver slots.
    pub fn is_gold_booster(&self, dict: u16) -> bool {
        self.get(dict).is_some_and(|r| r.category == Self::BOOSTER_CATEGORY && r.currency == 1)
    }

    /// A booster priced by `booster_use_costs` (339..344) rather than by its own price.
    pub fn has_variable_cost(&self, dict: u16) -> bool {
        self.get(dict).is_some_and(|r| r.category == Self::BOOSTER_CATEGORY && r.use_variable_costs)
    }

    /// Silver for the `nth` (0..3) booster at booster level `level`: `booster_use_costs[level][nth]`, the level itself as
    /// the row index like the client (0x2001BF65C: the level byte is the row, no offset), past the table its last row.
    pub fn match_cost(&self, level: u32, nth: usize) -> Option<u32> {
        let last = self.costs.booster_use_costs.len().checked_sub(1)?;
        self.costs.booster_use_costs.get((level as usize).min(last))?.get(nth).copied()
    }

    /// Silver a match costs a player of equipment level `level` (`equipment_use_costs[level]`). The client indexes the
    /// table by the level itself (0x200333286: the level of 0x2003330C0, no offset, no clamp); a level past the table
    /// takes its last value (CHOICE: the client would read past the array). 0 without a table.
    pub fn equipment_use_cost(&self, level: u8) -> u32 {
        let t = &self.costs.equipment_use_costs;
        t.get(usize::from(level)).or(t.last()).copied().unwrap_or(0)
    }

    /// Replace the `equipment_use_costs` table (tests: the 0.69d table is all 0).
    pub fn set_equipment_use_costs(&mut self, t: Vec<u32>) {
        self.costs.equipment_use_costs = t;
    }

    /// Value of a modifier of a booster item (0.5 = +50 %).
    pub fn modifier(&self, dict: u16, name: &str) -> f64 {
        self.get(dict).map(|r| r.modifiers.iter().filter(|m| m.name == name).map(|m| m.value).sum()).unwrap_or(0.0)
    }

    /// The service boosters (rows with a `service_id`; in 0.69d only `new_player_booster` 1088).
    pub fn service_boosters(&self) -> impl Iterator<Item = &VoucherRow> {
        self.rows.iter().filter(|r| r.service_id.is_some())
    }

    pub fn get(&self, dict: u16) -> Option<&VoucherRow> {
        self.rows.iter().find(|r| r.dict_id == dict)
    }

    /// Hours of premium account a voucher gives (premium access vouchers only).
    pub fn premium_hours(&self, dict: u16) -> Option<u32> {
        let r = self.get(dict)?;
        (r.voucher_type == Some(PREMIUM_ACCESS)).then_some(r.ttl_in_hours).flatten().filter(|h| *h > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{BoosterCosts, Vouchers, NEW_PLAYER_BOOSTER};

    #[test]
    fn premium_vouchers_have_their_ttl() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        assert_eq!(gd.vouchers.premium_hours(213), Some(24));
        assert_eq!(gd.vouchers.premium_hours(980), Some(48));
        assert_eq!(gd.vouchers.premium_hours(975), Some(72));
        assert_eq!(gd.vouchers.premium_hours(1021), Some(168));
        assert_eq!(gd.vouchers.premium_hours(151), None); // repair voucher
        assert_eq!(gd.vouchers.premium_hours(853), None); // experience booster
        assert!(gd.vouchers.is_match_booster(853) && gd.vouchers.is_match_booster(340) && !gd.vouchers.is_match_booster(213));
        assert!(gd.vouchers.is_gold_booster(853) && gd.vouchers.is_gold_booster(861) && !gd.vouchers.is_gold_booster(339));
        assert!(gd.vouchers.has_variable_cost(344) && !gd.vouchers.has_variable_cost(857) && !gd.vouchers.has_variable_cost(1088));
        assert_eq!(gd.vouchers.modifier(853, "experience_bonus_modifier"), 0.5);
        assert_eq!(gd.vouchers.modifier(857, "money_bonus_modifier"), 0.5);
        assert_eq!(gd.vouchers.modifier(861, "reputation_bonus_modifier"), 0.5);
        assert_eq!(gd.vouchers.match_cost(1, 2), Some(750));
        assert_eq!(gd.vouchers.match_cost(9, 0), Some(1500)); // clamped to the last row
        // the level is the row (rows 0 and 1 are equal): level 2 = [500, 1000, 1500], 3 = [750, ..], 5 = [1500, ..]
        assert_eq!([0, 1, 2, 3, 4, 5].map(|l| gd.vouchers.match_cost(l, 0)), [250, 250, 500, 750, 1000, 1500].map(Some));
        assert_eq!(gd.vouchers.match_cost(3, 2), Some(2250));
        // equipment_use_costs: 0 at every level in the 0.69d data; another table is read by the level itself
        assert!((0..=6).all(|l| gd.vouchers.equipment_use_cost(l) == 0));
        let v = Vouchers::new(vec![], BoosterCosts { booster_use_costs: vec![], equipment_use_costs: vec![0, 0, 1000, 2000, 5000, 8000] });
        assert_eq!([0, 1, 2, 5, 9].map(|l| v.equipment_use_cost(l)), [0, 0, 1000, 8000, 8000]);
        assert_eq!(Vouchers::default().equipment_use_cost(3), 0);
        let s: Vec<_> = gd.vouchers.service_boosters().map(|r| (r.dict_id, r.service_id, r.service_duration_matches, r.default_modifications.clone())).collect();
        assert_eq!(s, vec![(NEW_PLAYER_BOOSTER, Some(24), Some(3), vec![(140, 0)])]);
        assert_eq!(gd.vouchers.modifier(NEW_PLAYER_BOOSTER, "new_player_damage_modifier"), -0.15);
    }
}
