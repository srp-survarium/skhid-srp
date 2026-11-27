//! Module merge (45 10): rarity of a module, which donors the client lets through, the next rarity and the gold
//! price. Sources (docs/notes/workshop.md section 2):
//!
//! * rarity = grade of the modifier group `module_rarity_modifier` (312, `modules_drop_options`) in the module's
//!   record: the C++ reader of the item record copies the byte at +2 of the (group, grade) pair whose group is 312
//!   into the Flash field `moduleRarity` (0x200249A00..0x200249A2A, PROVEN); 0 when the record has no such pair.
//! * enum `ModuleRarityEnums`: 0 grey, 1 green, 2 blue, 3 orange (the purple "max quality" of the 0.69 notes), 4 gold,
//!   5 legendary (AS, PROVEN).
//! * donors: `ModuleUpgradeInventory.filterByRarity` -> `checkDonorFilter` (AS, PROVEN): with no module chosen any
//!   module passes; else a donor needs the same rarity, and for an orange or gold module an orange, gold or legendary
//!   donor (the 0.69 note: max-quality modules can be used to upgrade others). A module installed on a weapon
//!   (`attachedToInstanceId` > 0) is listed only with the panel's "show installed" box ticked; being bound to a weapon
//!   does not exclude it.
//! * next rarity: `ModuleUpgradePanel.getNextRarity` (AS, PROVEN): orange and gold -> legendary, legendary stays,
//!   else +1. `MAX_DONORS` = 3 (`ModuleUpgradeWindow`, AS).
//! * gold price: [`gold_price`], the number the client shows on the gold button (PROVEN, docs/notes/workshop.md
//!   section 1): `premium_to_craft[min(rarity - 1, len - 1)]` of the module's dictionary row, so grey 0 (index -1 reads
//!   `undefined`, shown as 0), green 10, blue 25, orange and gold 50. The "free" button shows no price and the client
//!   checks none; `details_to_craft` is 0 for every module, so it costs nothing.
//!
//! * success chance: [`success_chance`] = [`crate::craft::CRAFT_CHANCE`], the pair the client shows (push 9B ->
//!   `ModuleUpgradePanel.setCraftChances([free, pay])`, PROVEN) and the server rolls. Values: fan wiki "Workshop"
//!   (snapshot 2022-05-27, row "Same Rarity": 50 % / 100 %, failure 0 % / 0 %). The panel knows only two numbers, so a
//!   per-combination chance cannot be shown; the wiki rows "Blue + White or Green + Blue" (25 % / 50 %, failure
//!   25 % / 0 %) and "Green + White" (12 % / 25 %, failure 67 % / 25 %) need a donor of another rarity, which
//!   `filterByRarity` never lets through in 0.69d: NOT APPLICABLE. LIKELY those rows (and the "lose your modification"
//!   failure) describe the old pre-0.43 item upgrade the page text talks about, not the 0.69d module merge.
//! * a missed roll: the client knows two outcomes only, `setCraftResult(item, success)` with the labels
//!   `craft_result_success_label` / `craft_result_neutral_label` (AS, PROVEN); reply result 0 = neutral, no modifier
//!   list. There is no "module lost" result code. CHOICE (no source): the donors and the gold are consumed, the module
//!   stays unchanged (the wiki's same-rarity failure chance is 0 %, so this is the "nothing happened" case).
//!
//! * the module's drop modifiers after a merge: drawn again from the table of the new rarity
//!   ([`crate::module_drops::ModuleDrops::merged`]); the client's panel shows those bounds as the next grade's
//!   values before the merge (0x2001BCF60, PROVEN). Other pairs (override) are kept.

use crate::craft::CraftData;

/// Modifier group that stores a module's rarity as its grade.
pub const RARITY_GROUP: u16 = 312;
pub const RARITY_ORANGE: u16 = 3;
pub const RARITY_GOLD: u16 = 4;
pub const RARITY_LEGENDARY: u16 = 5;
/// Donors of one merge (`ModuleUpgradeWindow.MAX_DONORS`).
pub const MAX_DONORS: usize = 3;

/// Rarity of a module record (0 = grey when the record has no rarity pair).
pub fn rarity(mods: &[(u16, u16)]) -> u16 {
    mods.iter().find(|(g, _)| *g == RARITY_GROUP).map(|(_, r)| *r).unwrap_or(0)
}

/// Rarity after a successful merge (`getNextRarity`).
pub fn next_rarity(r: u16) -> u16 {
    match r {
        RARITY_ORANGE | RARITY_GOLD => RARITY_LEGENDARY,
        RARITY_LEGENDARY => RARITY_LEGENDARY,
        r => r + 1,
    }
}

/// Success chance in percent of a merge paid with gold (`true`) or the free button (`false`).
pub fn success_chance(gold: bool) -> f32 {
    if gold { crate::craft::CRAFT_CHANCE.1 } else { crate::craft::CRAFT_CHANCE.0 }
}

/// A module of the highest rarity cannot be merged further (`showMaxUpgradeWarning`).
pub fn can_merge(r: u16) -> bool {
    r < RARITY_LEGENDARY
}

/// May a module of rarity `donor` be a donor for a module of rarity `module` (`checkDonorFilter`)?
pub fn donor_fits(module: u16, donor: u16) -> bool {
    if donor == module {
        return true;
    }
    matches!(module, RARITY_ORANGE | RARITY_GOLD) && matches!(donor, RARITY_ORANGE | RARITY_GOLD | RARITY_LEGENDARY)
}

/// The module's modifiers with the rarity pair set to `r` (other pairs unchanged, the rarity pair stays first).
pub fn with_rarity(mods: &[(u16, u16)], r: u16) -> Vec<(u16, u16)> {
    let mut out: Vec<(u16, u16)> = mods.iter().copied().filter(|(g, _)| *g != RARITY_GROUP).collect();
    out.insert(0, (RARITY_GROUP, r));
    out
}

/// Gold price of merging a module of rarity `r`, exactly what the gold button shows (PROVEN chain): the C++
/// builder 0x20028CA90 (Flash callback `workshop_item_opened`) copies the module's own dictionary `premium_to_craft`
/// unchanged into `root.fill_item_craft_data` (modules = categories 39, 41..53; the 94 notice kinds 0x3A / 0x3B / 0x3C
/// would replace it by 20/50/100, 10/25/50, 1/1/1, and the server sends none of them), `WorkshopWindow` hands it to
/// `ModuleUpgradePanel.setCraftPremiumPrice`, and `updateCraftPrice` shows
/// `craftPremiumPrices[Math.min(moduleRarity - 1, length - 1)]`. A grey module (rarity 0) reads index -1, `undefined`,
/// which the `uint` field turns into 0: the client shows 0 and lets the click through, so it costs 0. None when the
/// dictionary row has no `premium_to_craft` or only zeros, i.e. the item has no merge price at all (the merge is
/// refused; every module has 10 / 25 / 50).
pub fn gold_price(craft: &CraftData, dict: u16, r: u16) -> Option<u32> {
    let row = &craft.params(dict)?.premium_to_craft;
    if row.iter().all(|p| *p == 0) {
        return None;
    }
    if r == 0 {
        return Some(0);
    }
    Some(row[(r as usize - 1).min(row.len() - 1)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rarity_rules() {
        assert_eq!(rarity(&[]), 0);
        assert_eq!(rarity(&[(300, 4), (312, 2)]), 2);
        assert_eq!(next_rarity(0), 1);
        assert_eq!(next_rarity(2), 3);
        assert_eq!(next_rarity(3), 5);
        assert_eq!(next_rarity(4), 5);
        assert_eq!(next_rarity(5), 5);
        assert!(can_merge(4) && !can_merge(5));
        assert!(donor_fits(1, 1) && !donor_fits(1, 2));
        assert!(donor_fits(3, 4) && donor_fits(4, 3) && !donor_fits(3, 2));
        // a legendary donor is let through for an orange or gold module only (`checkDonorFilter`)
        assert!(donor_fits(3, 5) && donor_fits(4, 5) && !donor_fits(2, 5) && !donor_fits(1, 5));
    }

    #[test]
    fn chances_are_the_wiki_same_rarity_row() {
        assert_eq!((success_chance(false), success_chance(true)), (50.0, 100.0));
    }

    #[test]
    fn rarity_pair_replaced_in_front() {
        assert_eq!(with_rarity(&[(300, 4)], 1), vec![(312, 1), (300, 4)]);
        assert_eq!(with_rarity(&[(300, 4), (312, 1)], 2), vec![(312, 2), (300, 4)]);
    }

    #[test]
    fn gold_price_by_rarity() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        let m = gd.items.iter().find(|i| gd.items.is_module(i.dict_id)).unwrap().dict_id;
        // the gold button: grey 0 (index -1 = undefined = 0), green 10, blue 25, orange / gold 50
        assert_eq!(gold_price(&gd.craft, m, 0), Some(0));
        assert_eq!(gold_price(&gd.craft, m, 1), Some(10));
        assert_eq!(gold_price(&gd.craft, m, 2), Some(25));
        assert_eq!(gold_price(&gd.craft, m, 3), Some(50));
        assert_eq!(gold_price(&gd.craft, m, 4), Some(50));
        assert_eq!(gold_price(&gd.craft, 925, 0), None); // the spare-parts item has no merge price
    }

    /// `ModuleUpgradePanel.updateCraftPrice` on the client's own dictionary row, for every module and every rarity
    /// that can be merged: AS3 `Array[i]` with i < 0 is `undefined`, which a `uint` field stores as 0.
    #[test]
    fn gold_price_is_the_panel_formula_on_every_module() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        let mut n = 0;
        for it in gd.items.iter().filter(|i| gd.items.is_module(i.dict_id)) {
            let p = gd.craft.params(it.dict_id).unwrap();
            let row = &p.premium_to_craft;
            assert_eq!(row, &vec![10, 25, 50], "dict {}", it.dict_id);
            for r in 0..RARITY_LEGENDARY {
                let i = (r as i64 - 1).min(row.len() as i64 - 1);
                let shown = if i < 0 { 0 } else { row[i as usize] };
                assert_eq!(gold_price(&gd.craft, it.dict_id, r), Some(shown), "dict {} rarity {r}", it.dict_id);
            }
            // the free button: the panel shows and checks no price, and the data has none either
            assert!(p.details_to_craft.iter().all(|v| *v == 0), "dict {}", it.dict_id);
            n += 1;
        }
        assert_eq!(n, 416);
    }
}
