//! What the 0.26g lobby knows of an account for its matches: its level,
//! and the maps it has played (`docs/notes/matchmaking.md`).
//!
//! **The level** is the client's own (it shows it in the lobby as the
//! "equipment level" and looks for a match around it; PROVEN, 0x2CA9B0 and
//! 0x2F79A0):
//!
//! 1. a kit's level is the highest `match_making_level` of what its 23
//!    slots hold, each times its slot's `slots_weight` (all 1 in the data),
//!    rounded; a stack counts only while it has units;
//! 2. the account's is the highest of its kits that can be played (the
//!    client skips a kit of state 2, the premium kit, which srp sends for
//!    none; only a kit that passes the Play check);
//! 3. the rating adds the `modifier_value` of the last `elo_modifiers` row
//!    whose `elo_min` it reaches;
//! 4. the sum is a row of `items_levels_modifiers`: its `item_level_min`
//!    is the level.
//!
//! **The match's level** is an index into `match_levels_dict`, the reward
//! multipliers: [choice] the mean of its players' levels, rounded (the
//! client shows each player the multipliers of its own level, 0x2CAB60;
//! what the original server took for a match of mixed levels is not in the
//! client).

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params::item_params;

use super::account::{Account, Kit};
use super::rules;
use crate::Level;

/// The level of a kit: see the module, step 1.
pub fn kit_level(account: &Account, kit: &Kit) -> u8 {
    let dictionary = Dictionary::shared();
    let weights = &dictionary.match_maker.slots_weight;
    let highest = kit
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, held)| {
            let held = held.as_ref()?;
            let item = account.item(held.item_id)?;
            // A stack with nothing in the slot or left of it is no gear
            // (0x2F7A26..0x2F7A3A).
            if dictionary.is_stack(item.dict_id)
                && (held.count == 0 || item.condition_or_stack == 0)
            {
                return None;
            }
            let level = item_params(item.dict_id)?.match_making_level;
            Some(f32::from(level) * weights.get(slot).copied().unwrap_or(1.0))
        })
        .fold(0.0_f32, f32::max);
    // The client adds 0.5 and floors (0x2F7A9E).
    (highest + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// The highest level of the account's kits that can be played at `now`
/// (step 2); 0 with none.
pub fn gear_level(account: &Account, now: u64) -> u8 {
    account
        .kits
        .iter()
        .filter(|kit| rules::check_play(account, kit.id, now).is_ok())
        .map(|kit| kit_level(account, kit))
        .max()
        .unwrap_or(0)
}

/// What a rating adds to the level (step 3).
pub fn elo_modifier(rating: u16) -> u8 {
    let mut modifier = 0;
    for row in &Dictionary::shared().match_maker.elo_modifiers {
        if u32::from(rating) < row.elo_min {
            break;
        }
        modifier = row.modifier_value;
    }
    modifier.min(u32::from(u8::MAX)) as u8
}

/// The account's level (step 4).
pub fn level(account: &Account, now: u64) -> Level {
    level_of(gear_level(account, now), account.rating)
}

/// [`level`] of a gear level and a rating.
pub fn level_of(gear_level: u8, rating: u16) -> Level {
    let rows = &Dictionary::shared().match_maker.items_levels_modifiers;
    let index = usize::from(gear_level) + usize::from(elo_modifier(rating));
    // The client indexes the rows unchecked; the data's 21 take the
    // highest gear (15) with the highest rating's modifier (5).
    match rows.get(index).or(rows.last()) {
        Some(row) => Level {
            min: row.item_level_min.min(u32::from(u8::MAX)) as u8,
        },
        None => Level::default(),
    }
}

/// The level of a match of players of these levels: their mean, rounded,
/// as an index into `match_levels_dict`. 0 with no players.
pub fn match_level(levels: &[u8]) -> u16 {
    if levels.is_empty() {
        return 0;
    }
    let sum: usize = levels.iter().map(|&level| usize::from(level)).sum();
    let mean = (2 * sum + levels.len()) / (2 * levels.len());
    let last = Dictionary::shared().match_levels.len().saturating_sub(1);
    mean.min(last) as u16
}

/// How many matches the account was put in on the map `level`
/// (`maps::MapMode::level`, a number of the game data's).
pub fn map_plays(account: &Account, level: u8) -> u32 {
    account
        .map_plays
        .iter()
        .find(|(map, _)| *map == level)
        .map_or(0, |(_, plays)| *plays)
}

/// The account was put in a match on the map `level`.
pub fn count_map_play(account: &mut Account, level: u8) {
    match account.map_plays.iter_mut().find(|(map, _)| *map == level) {
        Some((_, plays)) => *plays = plays.saturating_add(1),
        None => {
            account.map_plays.push((level, 1));
            account.map_plays.sort_unstable_by_key(|(map, _)| *map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::account::KitSlot;

    /// A dictionary item by its name.
    fn dict_id(name: &str) -> u16 {
        Dictionary::shared()
            .items
            .iter()
            .find(|item| item.name == name)
            .unwrap_or_else(|| panic!("no item {name}"))
            .dict_id
    }

    fn put(account: &mut Account, kit: usize, slot: usize, name: &str, count: u32) -> u32 {
        let item_id = account.add_item(dict_id(name), count);
        account.kits[kit].slots[slot] = Some(KitSlot { item_id, count });
        item_id
    }

    #[test]
    fn a_new_account_is_level_0() {
        // The TT-33 and the Mosin carbine "have level zero" (KB 317).
        let account = Account::new("Alice");
        assert_eq!(gear_level(&account, 0), 0);
        assert_eq!(level(&account, 0), Level { min: 0 });
    }

    #[test]
    fn a_kit_is_as_high_as_its_highest_item() {
        let mut account = Account::new("Alice");
        assert_eq!(
            item_params(dict_id("ak_74m")).unwrap().match_making_level,
            10
        );
        // A second weapon of level 10 beside the starter's of level 0.
        put(&mut account, 0, 10, "ak_74m", 1);
        put(&mut account, 0, 11, "ammo_5.45x39_7n10", 30);
        assert_eq!(kit_level(&account, &account.kits[0]), 10);
        assert_eq!(gear_level(&account, 0), 10);
        assert_eq!(level(&account, 0), Level { min: 10 });
    }

    #[test]
    fn ammo_counts_while_the_slot_has_rounds() {
        let mut account = Account::new("Alice");
        let level_0 = kit_level(&account, &account.kits[0]);
        assert_eq!(level_0, 0);
        // The Mosin's second ammo slot: ammo of level 5.
        assert_eq!(
            item_params(dict_id("ammo_5.45x39_7n6m"))
                .unwrap()
                .match_making_level,
            5
        );
        let ammo = put(&mut account, 0, 9, "ammo_5.45x39_7n6m", 30);
        assert_eq!(kit_level(&account, &account.kits[0]), 5);
        // None in the slot, or none left of the stack: no gear.
        account.kits[0].slots[9].as_mut().unwrap().count = 0;
        assert_eq!(kit_level(&account, &account.kits[0]), 0);
        account.kits[0].slots[9].as_mut().unwrap().count = 30;
        account.item_mut(ammo).unwrap().condition_or_stack = 0;
        assert_eq!(kit_level(&account, &account.kits[0]), 0);
    }

    #[test]
    fn the_highest_kit_that_can_be_played_is_the_accounts() {
        let mut account = Account::new("Alice");
        // The second kit: a level 10 rifle with rounds. The first stays 0.
        put(&mut account, 1, 7, "ak_74m", 1);
        put(&mut account, 1, 8, "ammo_5.45x39_7n10", 30);
        assert_eq!(kit_level(&account, &account.kits[0]), 0);
        assert_eq!(gear_level(&account, 0), 10);
        // Without rounds it can not be played, and does not count.
        account.kits[1].slots[8] = None;
        assert!(rules::check_play(&account, account.kits[1].id, 0).is_err());
        assert_eq!(gear_level(&account, 0), 0);
        // The third kit counts without a premium account.
        put(&mut account, 2, 7, "ak_74m", 1);
        put(&mut account, 2, 8, "ammo_5.45x39_7n10", 30);
        assert_eq!(gear_level(&account, 100), 10);
    }

    #[test]
    fn the_rating_moves_the_level_up_by_the_datas_rows() {
        assert_eq!(elo_modifier(0), 0);
        assert_eq!(elo_modifier(100), 0);
        assert_eq!(elo_modifier(10_000), 0);
        assert_eq!(elo_modifier(10_001), 1);
        assert_eq!(elo_modifier(12_500), 3);
        assert_eq!(elo_modifier(14_001), 5);
        assert_eq!(elo_modifier(u16::MAX), 5);

        assert_eq!(level_of(0, 100), Level { min: 0 });
        assert_eq!(level_of(3, 11_500), Level { min: 5 });
        assert_eq!(level_of(14, 100), Level { min: 14 });
        // The rows past 15 are all of item level 15.
        assert_eq!(level_of(15, 100), Level { min: 15 });
        assert_eq!(level_of(13, 14_500), Level { min: 15 });
        assert_eq!(level_of(15, 15_000), Level { min: 15 });
        assert_eq!(level_of(200, 15_000), Level { min: 15 });
    }

    #[test]
    fn a_match_is_of_its_players_mean_level() {
        assert_eq!(match_level(&[]), 0);
        assert_eq!(match_level(&[0, 0]), 0);
        assert_eq!(match_level(&[7]), 7);
        assert_eq!(match_level(&[0, 15]), 8);
        assert_eq!(match_level(&[0, 0, 1]), 0);
        assert_eq!(match_level(&[0, 1, 1]), 1);
        assert_eq!(match_level(&[15; 16]), 15);
        // Never past the multipliers' last row.
        assert_eq!(match_level(&[200, 200]), 20);
        assert!(
            Dictionary::shared()
                .match_level(usize::from(match_level(&[15, 15])))
                .is_some()
        );
    }

    #[test]
    fn map_plays_are_counted_by_map() {
        let mut account = Account::new("Alice");
        assert_eq!(map_plays(&account, 3), 0);
        count_map_play(&mut account, 3);
        count_map_play(&mut account, 1);
        count_map_play(&mut account, 3);
        assert_eq!(account.map_plays, [(1, 1), (3, 2)]);
        assert_eq!((map_plays(&account, 3), map_plays(&account, 8)), (2, 0));
    }
}
