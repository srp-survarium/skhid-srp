//! Item rewards after a match: spare parts and chests (docs/notes/workshop.md section 3).
//!
//! Source: `gameplay/drops/drops.options` -> `drop_after_combat` (copy in data/drops_069d.json, made by
//! crates/data-extract), PROVEN data:
//! * dict 925 `details` (the spare parts item, category 30): `min..max` per match level 1..5, `premium_factor` 0.5;
//! * dict 1872 `random_module_chest`: one per won match (`win_needed`);
//! * dict 2176..2180 `rare_weapon_level_N`: chance per won match by match level (`ignore_loss_if_premium` = a premium
//!   account also gets it after a loss).
//!
//! Module drops are the chest: the table has no direct module row, a module comes out of `random_module_chest`
//! (opened with 46 06, chests are not part of this crate).
//!
//! Meaning choices (not in any client or data source): the amount is uniform in `min..=max`; a premium account gets
//! `amount * (1 + premium_factor)` (LIKELY: the AS premium popup says "+50 %" for details, and the event currency rows
//! carry factor 0.25 = the official "+25 %"). Leavers get nothing (official 2022-01-27), also a match without a
//! won/lost marker. The official short-match multiplier (0.62b) is NOT in the 0.69d data or exe and is not applied.

use serde::Deserialize;

use crate::craft::Roll;

/// Match level of the local matches: the options blob the server builds says level 1 (options.rs, +0x1081).
pub const LOCAL_MATCH_LEVEL: u32 = 1;

/// Does a local match (Training with bots, tutorial map) pay match rewards? NO (PROVEN): the lobby movie's tooltip of the
/// "Training with bots" checkbox (`MatchMakingSettings`, l10n `st_match_settings_bots_hint`) says "Training mode where you
/// fight against bots (computer controlled opponents). Rewards in this mode are not awarded." Nothing is paid for it: no
/// experience, silver, reputation, spare parts, chests, and the `played_first_game` bonus stays unspent. The tutorial has
/// its own awards (`awards::tutorial_*`; CHOICE that its match pays no match reward on top).
pub const LOCAL_MATCH_PAYS: bool = false;

/// The spare parts item (`details`): a currency-like row, the lobby credits it to `PlayerInfo.craftParts` (8B).
pub const DETAILS_DICT: u16 = 925;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct DetailsRow {
    pub match_level: u32,
    pub min: u32,
    pub max: u32,
    pub premium_factor: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct WinItem {
    pub win_needed: bool,
    pub ignore_loss_if_premium: bool,
    pub dict_id: u16,
    pub min: u32,
    pub max: u32,
    pub prob: f64,
    /// absent = every match level
    #[serde(default)]
    pub match_level: Option<u32>,
}

/// `drop_after_combat` silver row (dict 214 `generic_money`) of a match level.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MoneyRow {
    pub match_level: u32,
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Default, Deserialize)]
pub struct Drops {
    pub max_drop_items_count: usize,
    #[serde(default)]
    pub money: Vec<MoneyRow>,
    pub details: Vec<DetailsRow>,
    pub win_items: Vec<WinItem>,
}

/// What a finished match drops for the human.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchDrops {
    /// spare parts (the `details` row)
    pub parts: u32,
    /// (dict id, amount) of the chests
    pub items: Vec<(u16, u32)>,
    /// event currencies 951 / 1138 / 1139 (docs/notes/events.md); the roll above never fills them: the lobby adds them
    /// while an event runs (`lobby-server/src/events.rs`)
    pub event: [u32; 3],
    /// the part of `parts` that the premium factor and the worn items' bonus added (shown apart on the victory screen)
    pub parts_bonus: u32,
    /// the part of `event` that the premium factor and the worn winter hats added
    pub event_bonus: [u32; 3],
    /// one-time items `(claim name, dict id, amount)`: given only when the profile has not recorded the claim yet (the
    /// claim is recorded in the same transaction), e.g. the event's first-win item
    pub once: Vec<(String, u16, u32)>,
}

impl Drops {
    /// Silver of the `drop_after_combat` money row of the match level (uniform in `min..=max`); 0 without a row.
    pub fn base_silver(&self, match_level: u32, rng: &mut dyn Roll) -> u32 {
        match self.money.iter().find(|r| r.match_level == match_level) {
            Some(r) => r.min + rng.below(r.max - r.min + 1),
            None => 0,
        }
    }

    /// Roll the drops of one match. `finished` = a won/lost marker arrived (a leaver gets nothing).
    pub fn roll(&self, match_level: u32, won: bool, finished: bool, premium: bool, rng: &mut dyn Roll) -> MatchDrops {
        self.roll_with_parts_bonus(match_level, won, finished, premium, 0, rng)
    }

    /// [`Drops::roll`] with the worn items' spare-parts bonus (`details_bonus_modifier`, `parts_percent` in percent)
    /// added to the premium factor: parts = round(base * (1 + premium_factor + parts_percent / 100)) (LIKELY additive,
    /// like the other reward bonuses).
    pub fn roll_with_parts_bonus(&self, match_level: u32, won: bool, finished: bool, premium: bool, parts_percent: u32, rng: &mut dyn Roll) -> MatchDrops {
        let mut out = MatchDrops::default();
        if !finished {
            return out;
        }
        if let Some(r) = self.details.iter().find(|r| r.match_level == match_level) {
            let base = r.min + rng.below(r.max - r.min + 1);
            let factor = 1.0 + if premium { r.premium_factor } else { 0.0 } + f64::from(parts_percent) / 100.0;
            out.parts = if factor == 1.0 { base } else { (f64::from(base) * factor).round() as u32 };
            out.parts_bonus = out.parts.saturating_sub(base);
        }
        for w in &self.win_items {
            if w.match_level.is_some_and(|l| l != match_level) {
                continue;
            }
            if w.win_needed && !won && !(premium && w.ignore_loss_if_premium) {
                continue;
            }
            if w.prob < 1.0 && rng.unit() >= w.prob {
                continue;
            }
            let n = w.min + rng.below(w.max - w.min + 1);
            if n > 0 && out.items.len() < self.max_drop_items_count {
                out.items.push((w.dict_id, n));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::SplitMix;

    fn drops() -> Drops {
        crate::testing::tables().drops
    }

    #[test]
    fn leaver_gets_nothing() {
        crate::require_tables!();
        let d = drops();
        assert_eq!(d.roll(1, true, false, true, &mut SplitMix(1)), MatchDrops::default());
    }

    #[test]
    fn details_range_and_premium_bonus() {
        crate::require_tables!();
        let d = drops();
        for seed in 0..200 {
            let a = d.roll(1, false, true, false, &mut SplitMix(seed));
            assert!((3..=9).contains(&a.parts), "{}", a.parts);
            assert!(a.items.is_empty(), "a lost match drops no chest");
            let b = d.roll(1, false, true, true, &mut SplitMix(seed));
            assert_eq!(b.parts, (a.parts as f64 * 1.5).round() as u32);
        }
    }

    #[test]
    fn parts_bonus_adds_to_the_premium_factor() {
        crate::require_tables!();
        let d = drops();
        for seed in 0..100 {
            let base = d.roll(3, false, true, false, &mut SplitMix(seed)).parts as f64;
            let plain = d.roll_with_parts_bonus(3, false, true, false, 25, &mut SplitMix(seed)).parts;
            assert_eq!(plain, (base * 1.25).round() as u32);
            let both = d.roll_with_parts_bonus(3, false, true, true, 25, &mut SplitMix(seed)).parts;
            assert_eq!(both, (base * 1.75).round() as u32, "premium +50 % and the bonus +25 % add");
        }
    }

    #[test]
    fn won_match_drops_the_module_chest() {
        crate::require_tables!();
        let d = drops();
        let r = d.roll(1, true, true, false, &mut SplitMix(7));
        assert!(r.items.contains(&(1872, 1)));
        assert!(r.items.iter().all(|(id, _)| *id == 1872 || (2176..=2180).contains(id)));
    }
}
