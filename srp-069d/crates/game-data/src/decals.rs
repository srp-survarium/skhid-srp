//! Camouflage and decals (0.69d): the items (`camo_decal` of `game.options`, volume 75), the decal sets and the armour
//! that takes them. Data file `data/decals_069d.json`, made by `crates/data-extract`. See `docs/notes/decals.md`.
//!
//! * dictionary category 25 = camouflage, 26 = decal (PROVEN `camo_decal` rows; client `ItemFactory`: category 25 shows as
//!   "camo", everything else of the editor as "decal").
//! * only torsos (category 3) and legs (category 5) carry `camo_available` / `decal_available` (PROVEN, 65 + 65 rows).
//! * an item record holds at most 3 camo / decal entries (PROVEN: client vector inline cap, 0x2001263D0).
//! * the dictionary type prefix says where an item comes from: `stk_` shop (silver), `prm_` premium, `drp_` drop,
//!   `unl_` unlocked by an achievement (LIKELY meaning of the prefixes; costs are PROVEN data).

use std::collections::BTreeMap;

use serde::Deserialize;

pub const FILE: &str = "decals_069d.json";
/// Camo / decal entries one item record takes (client limit).
pub const MAX_PER_ITEM: usize = 3;
/// Slot byte of the camouflage on a piece (PROVEN: lobby movie `DecorateControl.configUI` sets `camoSlot.index = 3`;
/// the slot has no "put in" (placement) button).
pub const CAMO_SLOT: u8 = 3;
/// Slot bytes of the two decals of a piece (PROVEN: `decalSlot1.index = 0`, `decalSlot2.index = 1`; the exe sends
/// `decal_available` to the movie as 2 and `camo_available` as a flag, 0x2002484A0, so a piece with the flag has two
/// decal slots and one camouflage slot; the movie lists only decals for a decal slot, only camouflage for the camo slot).
pub const DECAL_SLOTS: [u8; 2] = [0, 1];
pub const CAT_CAMO: u16 = 25;
pub const CAT_DECAL: u16 = 26;

#[derive(Debug, Clone, Deserialize)]
pub struct DecalItem {
    pub name: String,
    pub category: u16,
    #[serde(rename = "type")]
    pub kind: String,
    pub cost: u32,
    /// dictionary currency: 0 silver, 1 gold
    pub currency: u8,
    pub camo_decal_type: u8,
    /// `default_modifications` (group, grade)
    pub mods: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetTier {
    pub needed: u32,
    pub mods: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DecalSet {
    pub items: Vec<u16>,
    pub tiers: Vec<SetTier>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Accepts {
    pub camo: bool,
    pub decal: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct DecalData {
    items: BTreeMap<String, DecalItem>,
    pub sets: BTreeMap<String, DecalSet>,
    armour: BTreeMap<String, Accepts>,
}

impl DecalData {
    pub fn parse(json: &str) -> Result<DecalData, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn item(&self, dict: u16) -> Option<&DecalItem> {
        self.items.get(&dict.to_string())
    }

    pub fn is_decal_item(&self, dict: u16) -> bool {
        self.item(dict).is_some()
    }

    /// May a decal / camo item `decal` be attached to the armour piece `target` (torso or legs with the matching flag)?
    pub fn fits(&self, target: u16, decal: u16) -> bool {
        let (Some(a), Some(d)) = (self.armour.get(&target.to_string()), self.item(decal)) else { return false };
        if d.category == CAT_CAMO {
            a.camo
        } else {
            a.decal
        }
    }

    /// Is `slot` a slot the editor offers for the item `decal`: camouflage only in [`CAMO_SLOT`], decals only in
    /// [`DECAL_SLOTS`] (PROVEN, `DecorateControl`).
    pub fn slot_fits(&self, decal: u16, slot: u8) -> bool {
        match self.item(decal) {
            Some(d) if d.category == CAT_CAMO => slot == CAMO_SLOT,
            Some(_) => DECAL_SLOTS.contains(&slot),
            None => false,
        }
    }

    /// Price of buying the item in the shop-less way the dictionary gives: `stk_` and `prm_` items with a cost.
    /// Returns (price, gold). `drp_` (drop) and `unl_` (achievement) items are not sold. LIKELY.
    pub fn dictionary_price(&self, dict: u16) -> Option<(u32, bool)> {
        let d = self.item(dict)?;
        let sold = d.kind.starts_with("prm_") || d.kind.starts_with("stk_");
        (sold && d.cost > 0).then_some((d.cost, d.currency == 1))
    }

    /// The modifications (group, grade) a set of worn decals gives: each item's own `default_modifications` plus the
    /// modifiers of every set tier whose `items_needed` is reached by the worn items of that set. A decal dict id
    /// worn twice counts once for a set (distinct items).
    pub fn worn_mods(&self, worn: &[u16]) -> Vec<(u32, u32)> {
        let mut out = vec![];
        for d in worn {
            if let Some(i) = self.item(*d) {
                out.extend(i.mods.iter().copied());
            }
        }
        for s in self.sets.values() {
            let have = s.items.iter().filter(|i| worn.contains(i)).count() as u32;
            for t in s.tiers.iter().filter(|t| have >= t.needed) {
                out.extend(t.mods.iter().copied());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::GameData;

    fn gd() -> GameData {
        crate::testing::tables()
    }

    #[test]
    fn data_shape() {
        crate::require_tables!();
        let g = gd();
        let d = g.decals.item(718).unwrap();
        assert_eq!((d.category, d.kind.as_str(), d.currency, d.cost), (26, "prm_decl", 1, 50));
        assert_eq!(d.mods, vec![(124, 6)]);
        assert_eq!(g.decals.sets.len(), 11);
        // every item exists in the dictionary with the same category
        for (k, _) in g.decals.sets.iter() {
            assert!(k.starts_with("decal_set_"));
        }
    }

    #[test]
    fn only_torsos_and_legs_take_decals() {
        crate::require_tables!();
        let g = gd();
        let torso = g.items.iter().find(|i| i.category == 3).unwrap().dict_id;
        let legs = g.items.iter().find(|i| i.category == 5).unwrap().dict_id;
        let boots = g.items.iter().find(|i| i.category == 7).unwrap().dict_id;
        assert!(g.decals.fits(torso, 718));
        assert!(g.decals.fits(legs, 154), "camo on legs");
        assert!(!g.decals.fits(boots, 718));
        assert!(!g.decals.fits(718, 718));
    }

    #[test]
    fn set_bonus_needs_the_whole_tier() {
        crate::require_tables!();
        let g = gd();
        let (name, s) = g.decals.sets.iter().next().unwrap();
        let one = g.decals.worn_mods(&s.items[..1]);
        let all = g.decals.worn_mods(&s.items);
        assert!(all.len() > one.len(), "{name}: the tier adds modifiers");
        assert!(g.decals.dictionary_price(718).is_some());
    }
}
