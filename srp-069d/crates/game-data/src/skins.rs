//! Weapon skins (0.69d). Data file `data/skins_069d.json`, made by `crates/data-extract`. Rules and sources:
//! `docs/notes/weapon-skins.md`.
//!
//! PROVEN (game data): 40 weapon rows carry `skins_to_unlock` (a list of dictionary ids); each is a premium / event
//! variant of a weapon (`base_item_dict_id`) and the list names that base weapon. A skin is itself a weapon
//! dictionary item; applying it changes the dict id of the player's weapon record (client log "changing item %d skin
//! from %d to %d", 0x2001905D4).

use std::collections::BTreeMap;

/// Skin item dict id -> the weapon dict ids it can be put on.
#[derive(Debug, Clone, Default)]
pub struct SkinData {
    unlocks: BTreeMap<u16, Vec<u16>>,
}

impl SkinData {
    pub fn parse(json: &str) -> Result<SkinData, serde_json::Error> {
        let raw: BTreeMap<String, Vec<u16>> = serde_json::from_str(json)?;
        let mut unlocks = BTreeMap::new();
        for (k, v) in raw {
            if let Ok(id) = k.parse::<u16>() {
                unlocks.insert(id, v);
            }
        }
        Ok(SkinData { unlocks })
    }

    /// Whether `dict` is a skin item.
    pub fn is_skin(&self, dict: u16) -> bool {
        self.unlocks.contains_key(&dict)
    }

    /// The weapons a skin item can be applied to.
    pub fn targets(&self, skin: u16) -> &[u16] {
        self.unlocks.get(&skin).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Skin items that can be put on weapon `target` (a base weapon dict id), in dict order.
    pub fn skins_for(&self, target: u16) -> Vec<u16> {
        self.unlocks.iter().filter(|(_, t)| t.contains(&target)).map(|(s, _)| *s).collect()
    }

    pub fn len(&self) -> usize {
        self.unlocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.unlocks.is_empty()
    }
}
