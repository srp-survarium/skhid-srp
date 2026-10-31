//! What a 0.26g match, a first match, a daily bonus and a drop card give:
//! the game's `gameplay/drops/drops.options` (0.26g's `resources.db`;
//! `docs/notes/data.md` §5) and each item's level, drop weight
//! and rarity ([`super::item_params`]).
//!
//! Against 0.21d (`v021d::drops`) the file was redone: no `slots_open`,
//! `drop_amount` or `drop_value`; instead four tables (`generic_drop`,
//! `new_player_drop`, `daily_bonus_drop`, `drop_card_drop`), each with
//! `item_level_probabilities[21][16]`, flat `item_category_probabilities`,
//! `rarity_points` and `item_level_independent_categories`, plus
//! `playtime_dependence` and `game_mode_params`.
//!
//! **The client has no code for any of this**: the strings `rarity_points`,
//! `item_level_probabilities`, `playtime_dependence` are not in the 0.26g
//! exe, and it only shows the item the server names. So the roll is the
//! server's, and [`roll`] is srp's reading of the data:
//!
//! 1. A category group by `drop_weight`, then one of its categories evenly
//!    [0.21d's rule]. **Category 0 is no drop** [LIKELY: no item has it, and
//!    it is 60 of the generic table's 100].
//! 2. The items of the category with `drop_weight` above 0 [LIKELY: the
//!    cosmetics that can drop are the ones of type `drp_camo` / `drp_decl`,
//!    and exactly those have a drop weight; premium copies of weapons have
//!    0].
//! 3. Unless the category is in `item_level_independent_categories`: an
//!    item level from the row of the match's level (21 rows, as the 21 match
//!    levels of `match_levels_dict`; 16 columns, item levels 0..=15), and
//!    the category's items **of that level**. Many categories have no item
//!    at a level (the new player's table rolls levels 1 and 2 over every
//!    weapon category, and the lowest machine gun is level 9): then the
//!    category and level are rolled again [srp's choice; 0.21d took the
//!    nearest level instead, which would hand a new player that machine
//!    gun]. The group is not rolled again, so each keeps its share. Only if
//!    [`TRIES`] rolls find nothing is the nearest level taken.
//! 4. One of them by `drop_weight` [0.21d's rule].
//! 5. A rarity from `rarity_points`, read as the bounds of bands on a line:
//!    `[0, 1, 1.6, 1.9, 2]` is rarity 0 half the time, 1 for 30 %, 2 for
//!    15 %, 3 for 5 % [LIKELY: it makes the new player's table
//!    (`[0, 0, 0, 0, 2]`) always give rarity 3 and the daily bonus
//!    (`[0, 0, 0, 0, 0, 2]`) always rarity 4, and money is the one item with
//!    five rarities: the daily bonus, which only has premium money, is then
//!    its fifth, 50 gold].
//! 6. A stack's count from the item's `rarity` pair for that rarity (its
//!    last pair if it has fewer), in whole clips for ammo (every ammo's pair
//!    is 2 to 5 clips); gear gets the rarity level of its `rarity` list
//!    (`[0, 1, 2, 3]`), which the lobby shows from the item's modifications
//!    (`lobby.md` §2.6).
//!
//! `playtime_dependence.drop_item_probabilities_bonus` (`{games 1: 100,
//! 2: 75, 3: 50, 4: 25, -1: 0}`) reads as a bonus, in percent, to the chance
//! of a drop for the first games of a `play_period` [LIKELY];
//! [`PlaytimeDependence::bonus_percent`] gives it and [`roll`] applies it by
//! multiplying the chance of an item [GUESS: how it was applied is not in
//! the data]. Pass 0 for the plain table. `modifications_steps` and
//! `game_mode_params` are parsed but not used: what they did is not known.
//!
//! Not carried over: 0.21d's progress bar (its `slots_open` is gone: 0.26g
//! shows one dropped item, `results.md`) and srp's own bias against
//! expensive gear (it went by `drop_value`; the level table now does that).

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::item_params::{ItemParams, item_params, shop_unit};
use super::scopes::SCOPE_CATEGORY;
pub use crate::v021d::drops::Rng;

/// Its path in the archive.
pub const PATH: &str = "gameplay/drops/drops.options";

/// The category that stands for no drop.
pub const NOTHING: u8 = 0;

/// How often a category and level are rolled again when the category has no
/// item of the level, before the nearest level is taken.
pub const TRIES: usize = 32;

/// One row of `item_category_probabilities`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryGroup {
    pub weight: u32,
    pub categories: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DropTable {
    /// By match level (21 rows): a weight for each item level (16).
    pub levels: Vec<Vec<u32>>,
    pub categories: Vec<CategoryGroup>,
    pub rarity_points: Vec<f32>,
    /// The categories whose items are not picked by level.
    pub level_independent: Vec<u8>,
}

/// `playtime_dependence`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaytimeDependence {
    pub play_period: u32,
    pub games_without_drop_bonus_coef: f32,
    pub modifications_steps: Vec<f32>,
    /// `(games, bonus)`; games -1 is every later game.
    pub drop_item_probabilities_bonus: Vec<(i32, u32)>,
}

impl PlaytimeDependence {
    /// The bonus for the `game`-th game (from 1) of a play period.
    pub fn bonus_percent(&self, game: u32) -> u32 {
        self.drop_item_probabilities_bonus
            .iter()
            .find(|&&(games, _)| i64::from(games) == i64::from(game))
            .or_else(|| {
                self.drop_item_probabilities_bonus
                    .iter()
                    .find(|&&(games, _)| games < 0)
            })
            .map_or(0, |&(_, bonus)| bonus)
    }
}

/// `game_mode_params`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameModeParams {
    pub high_threshold: f32,
    pub low_threshold: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drops {
    /// After a match.
    pub generic: DropTable,
    pub new_player: DropTable,
    pub daily_bonus: DropTable,
    /// From a drop card (the voucher `drop_card`, 153).
    pub drop_card: DropTable,
    pub playtime: PlaytimeDependence,
    pub game_mode: GameModeParams,
}

/// One dropped item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    pub dict_id: u16,
    /// Units of a stack; 1 for an item that is no stack.
    pub count: u32,
    /// The rarity rolled: for gear its rarity level (0..=3), for a stack the
    /// index of the count pair used.
    pub rarity: u8,
}

fn number(value: &ConfigValue) -> Result<u32, ConfigError> {
    value
        .as_i32()
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| ConfigError("a drop table number is not one".into()))
}

fn float(value: &ConfigValue) -> Result<f32, ConfigError> {
    value
        .as_f32()
        .ok_or_else(|| ConfigError("a drop table number is not one".into()))
}

fn list<T>(
    value: &ConfigValue,
    item: impl Fn(&ConfigValue) -> Result<T, ConfigError>,
) -> Result<Vec<T>, ConfigError> {
    value.children().into_iter().map(item).collect()
}

fn categories(value: &ConfigValue) -> Result<Vec<u8>, ConfigError> {
    list(value, |category| {
        number(category).and_then(|category| {
            u8::try_from(category).map_err(|_| ConfigError("a category is out of range".into()))
        })
    })
}

fn parse_table(table: &ConfigValue) -> Result<DropTable, ConfigError> {
    Ok(DropTable {
        levels: list(table.req("item_level_probabilities")?, |row| {
            list(row, number)
        })?,
        categories: list(table.req("item_category_probabilities")?, |group| {
            Ok(CategoryGroup {
                weight: number(group.req("drop_weight")?)?,
                categories: categories(group.req("categories")?)?,
            })
        })?,
        rarity_points: list(table.req("rarity_points")?, float)?,
        level_independent: categories(table.req("item_level_independent_categories")?)?,
    })
}

impl Drops {
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<Drops> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            Self::parse(super::game_data::file(PATH)).expect("the drop tables parse")
        })
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        let root = ConfigValue::parse(bytes)?;
        let playtime = root.req("playtime_dependence")?;
        let game_mode = root.req("game_mode_params")?;
        Ok(Self {
            generic: parse_table(root.req("generic_drop")?)?,
            new_player: parse_table(root.req("new_player_drop")?)?,
            daily_bonus: parse_table(root.req("daily_bonus_drop")?)?,
            drop_card: parse_table(root.req("drop_card_drop")?)?,
            playtime: PlaytimeDependence {
                play_period: number(playtime.req("play_period")?)?,
                games_without_drop_bonus_coef: playtime.f32_at("games_without_drop_bonus_coef")?,
                modifications_steps: list(playtime.req("modifications_steps")?, float)?,
                drop_item_probabilities_bonus: list(
                    playtime.req("drop_item_probabilities_bonus")?,
                    |row| Ok((row.i32_at("games")?, number(row.req("bonus")?)?)),
                )?,
            },
            game_mode: GameModeParams {
                high_threshold: game_mode.f32_at("high_threshold")?,
                low_threshold: game_mode.f32_at("low_threshold")?,
            },
        })
    }
}

/// A fraction in `[0, 1)`.
fn unit(rng: &mut Rng) -> f32 {
    rng.below(1 << 24) as f32 / (1 << 24) as f32
}

impl DropTable {
    /// The chance, of 1, that a roll gives an item at all.
    pub fn item_chance(&self) -> f32 {
        let total: u32 = self.categories.iter().map(|group| group.weight).sum();
        let nothing: u32 = self
            .categories
            .iter()
            .filter(|group| group.categories == [NOTHING])
            .map(|group| group.weight)
            .sum();
        if total == 0 {
            0.0
        } else {
            (total - nothing) as f32 / total as f32
        }
    }

    /// The rarity for a point `at` on the line of `rarity_points` (step 5).
    pub fn rarity_at(&self, at: f32) -> u8 {
        self.rarity_points
            .windows(2)
            .position(|band| band[0] <= at && at < band[1])
            .unwrap_or(self.rarity_points.len().saturating_sub(2)) as u8
    }

    fn roll_rarity(&self, rng: &mut Rng) -> u8 {
        let top = self.rarity_points.last().copied().unwrap_or(0.0);
        self.rarity_at(unit(rng) * top)
    }

    /// The items a roll of `category` at item level `level` picks among
    /// (steps 2 and 3): those that can drop, of that level unless the
    /// category goes by no level.
    pub fn candidates(
        &self,
        dictionary: &Dictionary,
        category: u8,
        level: u8,
    ) -> Vec<&'static ItemParams> {
        let independent = self.level_independent.contains(&category);
        droppable(dictionary, category)
            .filter(|params| independent || params.item_level == level)
            .collect()
    }

    /// As [`Self::candidates`], but the items nearest to `level` when the
    /// category has none of it.
    pub fn nearest_candidates(
        &self,
        dictionary: &Dictionary,
        category: u8,
        level: u8,
    ) -> Vec<&'static ItemParams> {
        let exact = self.candidates(dictionary, category, level);
        if !exact.is_empty() {
            return exact;
        }
        let distance = |params: &ItemParams| params.item_level.abs_diff(level);
        let Some(nearest) = droppable(dictionary, category).map(distance).min() else {
            return exact;
        };
        droppable(dictionary, category)
            .filter(|params| distance(params) == nearest)
            .collect()
    }
}

/// The items of `category` that can drop: those with a drop weight.
fn droppable(
    dictionary: &Dictionary,
    category: u8,
) -> impl Iterator<Item = &'static ItemParams> + '_ {
    dictionary
        .items
        .iter()
        // [srp] Scopes are bought, never dropped (the shop's scope items
        // and the variants, `super::scopes`), whatever a drop table says.
        .filter(move |item| item.category == category && category != SCOPE_CATEGORY)
        .filter_map(|item| item_params(item.dict_id))
        .filter(|params| params.drop_weight > 0)
}

/// What a roll that picked `params` gives, at a rarity from the table
/// (steps 5 and 6).
fn dropped(table: &DropTable, dictionary: &Dictionary, params: &ItemParams, rng: &mut Rng) -> Drop {
    let rarity = usize::from(table.roll_rarity(rng));
    if dictionary.is_stack(params.dict_id) {
        let pairs = params.stack_rarity;
        let used = rarity.min(pairs.len().saturating_sub(1));
        let (least, most) = pairs.get(used).copied().unwrap_or((1, 1));
        let step = shop_unit(params.dict_id);
        let (least, most) = ((least / step).max(1), (most / step).max(1));
        let steps = least + rng.below(most.saturating_sub(least) + 1);
        Drop {
            dict_id: params.dict_id,
            count: steps * step,
            rarity: used as u8,
        }
    } else {
        let levels = params.gear_rarity;
        let used = rarity.min(levels.len().saturating_sub(1));
        Drop {
            dict_id: params.dict_id,
            count: 1,
            rarity: levels.get(used).copied().unwrap_or(0),
        }
    }
}

/// One roll of `table` for a match of `match_level` (0..=20), with
/// `bonus_percent` more chance of an item
/// ([`PlaytimeDependence::bonus_percent`], or 0). `None` is no drop.
pub fn roll(
    table: &DropTable,
    dictionary: &Dictionary,
    match_level: u8,
    bonus_percent: u32,
    rng: &mut Rng,
) -> Option<Drop> {
    let chance = (table.item_chance() * (1.0 + bonus_percent as f32 / 100.0)).min(1.0);
    if unit(rng) >= chance {
        return None;
    }
    let groups: Vec<&CategoryGroup> = table
        .categories
        .iter()
        .filter(|group| group.categories != [NOTHING])
        .collect();
    let weights: Vec<u32> = groups.iter().map(|group| group.weight).collect();
    let row = table
        .levels
        .get(usize::from(match_level))
        .or(table.levels.last())?;
    // The group once, so that each keeps its share of the drops; then its
    // category and the level until some item has them.
    let group = groups[rng.pick(&weights)?];
    let pick = |nearest: bool, rng: &mut Rng| {
        let category = group.categories[rng.below(group.categories.len() as u32) as usize];
        let level = rng.pick(row).unwrap_or(0) as u8;
        let candidates = if nearest {
            table.nearest_candidates(dictionary, category, level)
        } else {
            table.candidates(dictionary, category, level)
        };
        let item_weights: Vec<u32> = candidates.iter().map(|p| p.drop_weight).collect();
        rng.pick(&item_weights).map(|index| candidates[index])
    };
    let params = (0..TRIES)
        .find_map(|_| pick(false, rng))
        .or_else(|| (0..TRIES).find_map(|_| pick(true, rng)))?;
    Some(dropped(table, dictionary, params, rng))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_026g_drop_tables() {
        let drops = Drops::shared();
        for table in [
            &drops.generic,
            &drops.new_player,
            &drops.daily_bonus,
            &drops.drop_card,
        ] {
            assert_eq!(table.levels.len(), 21);
            assert!(table.levels.iter().all(|row| row.len() == 16));
            assert!(
                table
                    .levels
                    .iter()
                    .all(|row| row.iter().sum::<u32>() == 100)
            );
            assert_eq!(table.level_independent, [10, 11, 24, 25, 26, 27, 28, 29]);
            let weights: u32 = table.categories.iter().map(|group| group.weight).sum();
            assert_eq!(weights, 100);
        }

        let generic = &drops.generic;
        assert_eq!(generic.levels[0][..3], [80, 20, 0]);
        assert_eq!(generic.levels[20][13..], [10, 40, 50]);
        let groups: Vec<(u32, &[u8])> = generic
            .categories
            .iter()
            .map(|group| (group.weight, &group.categories[..]))
            .collect();
        assert_eq!(
            groups,
            [
                (60, &[0][..]),
                (5, &[25, 26]),
                (5, &[27]),
                (9, &[28]),
                (1, &[29]),
                (10, &[1, 2, 3, 4, 5, 6, 7]),
                (10, &[8, 13, 14, 15, 16, 17]),
            ]
        );
        assert_eq!(generic.rarity_points, [0.0, 1.0, 1.6, 1.9, 2.0]);
        assert!((generic.item_chance() - 0.4).abs() < 1e-6);

        assert_eq!(drops.new_player.levels[7][..4], [0, 50, 50, 0]);
        assert_eq!(drops.new_player.categories.len(), 2);
        assert_eq!(drops.new_player.rarity_points, [0.0, 0.0, 0.0, 0.0, 2.0]);
        assert_eq!(drops.new_player.item_chance(), 1.0);
        assert_eq!(drops.daily_bonus.categories.len(), 1);
        assert_eq!(drops.daily_bonus.categories[0].categories, [29]);
        assert_eq!(
            drops.daily_bonus.rarity_points,
            [0.0, 0.0, 0.0, 0.0, 0.0, 2.0]
        );
        assert_eq!(drops.daily_bonus.levels[20][15], 100);
        assert_eq!(drops.drop_card.categories[0].weight, 40);
        assert_eq!(drops.drop_card.categories[0].categories, [25, 26]);
        assert_eq!(drops.drop_card.item_chance(), 1.0);

        assert_eq!(drops.playtime.play_period, 60);
        assert_eq!(drops.playtime.games_without_drop_bonus_coef, 1.0);
        assert_eq!(drops.playtime.modifications_steps, [0.0, 0.1, 0.01, 0.001]);
        assert_eq!(
            drops.playtime.drop_item_probabilities_bonus,
            [(1, 100), (2, 75), (3, 50), (4, 25), (-1, 0)]
        );
        let bonus: Vec<u32> = (1..=6).map(|g| drops.playtime.bonus_percent(g)).collect();
        assert_eq!(bonus, [100, 75, 50, 25, 0, 0]);
        assert_eq!(
            (
                drops.game_mode.high_threshold,
                drops.game_mode.low_threshold
            ),
            (0.6, 0.2)
        );
    }

    #[test]
    fn rarity_points_are_bands() {
        let drops = Drops::shared();
        let generic = &drops.generic;
        assert_eq!(generic.rarity_at(0.0), 0);
        assert_eq!(generic.rarity_at(0.99), 0);
        assert_eq!(generic.rarity_at(1.0), 1);
        assert_eq!(generic.rarity_at(1.59), 1);
        assert_eq!(generic.rarity_at(1.7), 2);
        assert_eq!(generic.rarity_at(1.95), 3);
        // The new player's and the daily bonus's tables have one band.
        for at in [0.0, 1.0, 1.99] {
            assert_eq!(drops.new_player.rarity_at(at), 3);
            assert_eq!(drops.daily_bonus.rarity_at(at), 4);
        }
        // Over many rolls: half, 30 %, 15 %, 5 %.
        let mut rng = Rng::new(5);
        let mut counts = [0u32; 4];
        for _ in 0..20_000 {
            counts[usize::from(generic.roll_rarity(&mut rng))] += 1;
        }
        let share = |index: usize| f64::from(counts[index]) / 20_000.0;
        assert!((share(0) - 0.50).abs() < 0.02, "{counts:?}");
        assert!((share(1) - 0.30).abs() < 0.02, "{counts:?}");
        assert!((share(2) - 0.15).abs() < 0.02, "{counts:?}");
        assert!((share(3) - 0.05).abs() < 0.01, "{counts:?}");
    }

    #[test]
    fn candidates_go_by_drop_weight_and_level() {
        let dictionary = Dictionary::shared();
        let generic = &Drops::shared().generic;
        // Assault rifles (category 8) near level 5: the AKS-74U.
        let rifles = generic.candidates(dictionary, 8, 5);
        assert!(rifles.iter().all(|params| params.item_level == 5));
        assert!(rifles.iter().any(|params| params.dict_id == 13));
        // No rifle of level 0: nothing, or the nearest level if asked.
        assert!(generic.candidates(dictionary, 8, 0).is_empty());
        let low = generic.nearest_candidates(dictionary, 8, 0);
        assert!(!low.is_empty());
        let level = low[0].item_level;
        assert!(level > 0 && low.iter().all(|params| params.item_level == level));
        assert_eq!(generic.nearest_candidates(dictionary, 8, 5), rifles);
        // Premium copies never drop: their weight is 0.
        for category in [8, 13, 14, 15, 16, 17] {
            for level in 0..16 {
                for params in generic.candidates(dictionary, category, level) {
                    let item = dictionary.item(params.dict_id).unwrap();
                    assert!(!item.name.ends_with("_premium"), "{}", item.name);
                }
            }
        }
        // Cosmetics are not picked by level: every one that can drop, and
        // only the `drp_` ones.
        let camouflages = generic.candidates(dictionary, 25, 9);
        assert_eq!(camouflages.len(), 20);
        assert!(camouflages.iter().all(|params| params.kind == "drp_camo"));
        let decals = generic.candidates(dictionary, 26, 0);
        assert_eq!(decals.len(), 21);
        assert!(decals.iter().all(|params| params.kind == "drp_decl"));
        // Money.
        assert_eq!(generic.candidates(dictionary, 28, 3)[0].dict_id, 214);
        assert_eq!(generic.candidates(dictionary, 29, 3)[0].dict_id, 215);
        assert!(generic.candidates(dictionary, 0, 3).is_empty());
    }

    #[test]
    fn an_item_of_drop_weight_0_is_never_dropped() {
        // An item whose config says drop_weight 0 (as the Duty
        // exoskeleton's: sold, never dropped) is in no table at any level.
        // The Bandit balaclava of the test patch drops as the stock mask it
        // is made from (its weight, 110).
        let dictionary = Dictionary::shared();
        let drops = Drops::shared();
        assert_eq!(item_params(1810).unwrap().drop_weight, 110);
        let never: Vec<u16> = dictionary
            .items
            .iter()
            .filter(|item| item_params(item.dict_id).is_some_and(|params| params.drop_weight == 0))
            .map(|item| item.dict_id)
            .collect();
        assert!(!never.is_empty());
        for dict_id in never {
            let category = dictionary.item(dict_id).unwrap().category;
            for table in [
                &drops.generic,
                &drops.new_player,
                &drops.daily_bonus,
                &drops.drop_card,
            ] {
                for level in 0..=20 {
                    assert!(
                        table
                            .nearest_candidates(dictionary, category, level)
                            .iter()
                            .all(|params| params.dict_id != dict_id),
                        "{dict_id} at level {level}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_match_drops_something_two_times_in_five() {
        let dictionary = Dictionary::shared();
        let generic = &Drops::shared().generic;
        let mut rng = Rng::new(7);
        let (mut drops, mut gear, mut money) = (0u32, 0u32, 0u32);
        for _ in 0..10_000 {
            let Some(drop) = roll(generic, dictionary, 5, 0, &mut rng) else {
                continue;
            };
            drops += 1;
            let item = dictionary.item(drop.dict_id).unwrap();
            let params = item_params(drop.dict_id).unwrap();
            assert!(params.drop_weight > 0, "{}", item.name);
            if item.is_stack {
                assert!(drop.count >= 1);
                let (least, most) = params.stack_rarity[usize::from(drop.rarity)];
                assert!(
                    (least..=most).contains(&drop.count),
                    "{} × {}",
                    item.name,
                    drop.count
                );
                assert!(drop.rarity <= 3);
            } else {
                assert_eq!(drop.count, 1);
                assert!(drop.rarity <= 3);
            }
            match item.category {
                1..=8 | 13..=17 => {
                    gear += 1;
                    // Match level 5 gives item levels 3..=6.
                    assert!((3..=6).contains(&params.item_level), "{}", item.name);
                }
                28 => {
                    money += 1;
                    assert!((1000..=10_000).contains(&drop.count), "{}", drop.count);
                }
                29 => assert!([10, 20, 30, 40].contains(&drop.count), "{}", drop.count),
                _ => {}
            }
        }
        // 40 of 100 are a drop; half of those gear, 9 of 40 money.
        assert!((3800..4200).contains(&drops), "{drops} drops");
        assert!((1800..2200).contains(&gear), "{gear} gear");
        assert!((750..1050).contains(&money), "{money} money");

        // The first game of a play period: twice the chance.
        let mut drops = 0;
        for _ in 0..10_000 {
            drops += u32::from(roll(generic, dictionary, 5, 100, &mut rng).is_some());
        }
        assert!(
            (7800..8200).contains(&drops),
            "{drops} drops with the bonus"
        );
    }

    #[test]
    fn the_other_tables_always_give() {
        let dictionary = Dictionary::shared();
        let drops = Drops::shared();
        let mut rng = Rng::new(11);
        for _ in 0..500 {
            // A new player: a weapon or armour of level 1 or 2, best rarity.
            let drop = roll(&drops.new_player, dictionary, 0, 0, &mut rng).unwrap();
            let item = dictionary.item(drop.dict_id).unwrap();
            assert!(matches!(item.category, 1..=8 | 13..=17), "{}", item.name);
            assert!(!item.is_stack);
            let level = item_params(drop.dict_id).unwrap().item_level;
            assert!((1..=2).contains(&level), "{} is level {level}", item.name);
            assert_eq!(drop.rarity, 3);

            // The daily bonus: 50 gold.
            let drop = roll(&drops.daily_bonus, dictionary, 9, 0, &mut rng).unwrap();
            assert_eq!((drop.dict_id, drop.count, drop.rarity), (215, 50, 4));

            // A drop card: a cosmetic, armour or a weapon.
            let drop = roll(&drops.drop_card, dictionary, 20, 0, &mut rng).unwrap();
            let item = dictionary.item(drop.dict_id).unwrap();
            assert!(
                matches!(item.category, 1..=8 | 13..=17 | 25 | 26),
                "{}",
                item.name
            );
        }
    }

    #[test]
    fn ammo_would_drop_in_whole_clips() {
        // No table of 0.26g has an ammo category, but the count rule holds
        // for any stack: a table that does.
        let dictionary = Dictionary::shared();
        let mut table = Drops::shared().generic.clone();
        table.categories = vec![CategoryGroup {
            weight: 1,
            categories: vec![9],
        }];
        let mut rng = Rng::new(3);
        for _ in 0..300 {
            let drop = roll(&table, dictionary, 5, 0, &mut rng).unwrap();
            let params = item_params(drop.dict_id).unwrap();
            assert_eq!(drop.count % params.clip_size, 0);
            assert!((60..=150).contains(&drop.count));
        }
    }

    /// The STK2 outfits' 14 items on game data with their packs
    /// (`SRP_GAME_DATA`: `release/build-content`'s server-game-data.db with
    /// stk2-bandit, stk2-novice and stk2-loner): no faction, sold by seller 0
    /// at no rank at its own price, as the Novice's knit cap (the owner,
    /// 2026-10-31), and dropped: each at the
    /// owner's level and in the generic table's pool of its category at that
    /// level, by its own drop weight (`armour::tests` holds it to its level's
    /// range). It makes that data the process's:
    /// run it alone.
    #[test]
    #[ignore = "needs game data with the STK2 outfits' packs (SRP_GAME_DATA); run alone"]
    fn the_stk2_outfits_drop_and_are_sold_to_everyone() {
        use super::super::{game_data, prices};
        let path = std::env::var_os(game_data::GAME_DATA_ENV).expect("SRP_GAME_DATA");
        game_data::init_from(std::path::Path::new(&path)).expect("its data, alone");
        let dictionary = Dictionary::shared();
        let generic = &Drops::shared().generic;
        // (item, the owner's level, the stock piece its config copies)
        let items: [(&str, u8, &str); 14] = [
            ("stk2_bandit_026_torso", 6, "torso/edge_torso_1"),
            ("stk2_bandit_026_legs", 6, "legs/edge_legs_1"),
            ("stk2_bandit_026_boots", 6, "boots/edge_boots_1"),
            ("stk2_novice_026_hood", 4, "helmet/scavenger_helmet_5"),
            ("stk2_novice_026_torso", 4, "torso/blackmarket_torso_5"),
            ("stk2_novice_026_torso_v2", 4, "torso/blackmarket_torso_5"),
            ("stk2_novice_026_back", 4, "back/bm_back_1"),
            ("stk2_novice_026_legs", 4, "legs/scavenger_legs_3"),
            ("stk2_novice_026_boots", 4, "boots/scavenger_boots_3"),
            ("stk2_novice_026_hand", 4, "gloves/eq046_blackmarket_gloves_5"),
            ("stk2_loner_026_torso", 6, "torso/renesanse_torso_8"),
            ("stk2_loner_026_legs", 6, "legs/renesanse_legs_8"),
            ("stk2_loner_026_boots", 6, "boots/renesanse_boots_1"),
            ("stk2_loner_026_hand", 6, "gloves/scavenger_gloves_5"),
        ];
        for (name, level, donor) in items {
            let item = dictionary.item_named(name).unwrap_or_else(|| panic!("no {name}"));
            let params = item_params(item.dict_id).unwrap();
            let cfg = format!("gameplay/items/armour/{donor}.options");
            let template = dictionary.items.iter().find(|i| i.cfg_name == cfg).unwrap();
            let template = item_params(template.dict_id).unwrap();
            assert_eq!(params.faction, 0, "{name}");
            assert_eq!(params.item_level, level, "{name}");
            // Its own drop weight (armour::tests checks it against its
            // level's), in the same pool as its template's.
            assert_eq!(params.kind, template.kind, "{name}");
            assert!(params.drop_weight > 0, "{name}");
            // Sold as the Novice's knit cap (the owner, 2026-10-31): by
            // seller 0 at no rank, at its own price.
            assert_eq!(prices::sellers_of(dictionary, item, params), [(0, 0)], "{name}");
            assert!(
                prices::price_list().iter().any(|r| r.dict_id == item.dict_id
                    && r.faction_id == 0
                    && r.level == 0
                    && r.cost == item.base_cost),
                "{name}"
            );
            assert!(!generic.level_independent.contains(&item.category), "{name}");
            let pool = generic.candidates(dictionary, item.category, level);
            assert!(pool.iter().any(|candidate| candidate.dict_id == item.dict_id), "{name}");
        }
    }
}
