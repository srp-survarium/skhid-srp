//! The Colt Python twist of a free-for-all (`docs/notes/ffa-cowboy.md`): every kit's guns become
//! one Colt Python (dictionary item `magnum`) in weapon slot 7, with its ammunition, in the match
//! profile the static info sends ([`arm`]). Every game cooks each life from those profiles, so the
//! change reaches every client, a late joiner and a replay alike.

use srp_validate::world::{AMMO_SLOTS, WEAPON_SLOTS};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::item_params::{self, ItemClass};
use super::match_msgs::static_info::{FULL_CONDITION, MatchItem, MatchKit, MatchProfile};
use super::twist::STORE_LIVES;

/// The dictionary name of the Colt Python.
pub const GUN_NAME: &str = "magnum";

/// The weapon slot the Python goes into (weapon 1).
const GUN_SLOT: u8 = 7;
/// The ammo slot its rounds go into (weapon 1's first).
const AMMO_SLOT: u8 = 8;

/// The Python and its ammunition, read from the game data once ([`Revolver::of`]): what [`arm`]
/// puts in every kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revolver {
    /// [`GUN_NAME`]'s dict id.
    gun: u16,
    /// The first ammunition of the dictionary the gun takes.
    pub ammo: u16,
    /// A kit's rounds of it: the dictionary's `optimal_count` of the gun's category.
    rounds: u16,
}

impl Revolver {
    /// The Python of `dictionary`, or what the data lacks for it: the item, its states, its slot,
    /// an ammunition it takes that fits [`AMMO_SLOT`], or its category's amount.
    pub fn of(dictionary: &Dictionary) -> Result<Self, String> {
        let gun = dictionary
            .item_named(GUN_NAME)
            .ok_or_else(|| format!("no item `{GUN_NAME}`"))?;
        if gun.is_stack || item_params::item_params(gun.dict_id).is_none_or(|p| p.gun.is_none()) {
            return Err(format!("`{GUN_NAME}` is no gun"));
        }
        if !dictionary.fits(gun.dict_id, GUN_SLOT) {
            return Err(format!("`{GUN_NAME}` does not fit slot {GUN_SLOT}"));
        }
        let ammo = dictionary
            .items
            .iter()
            .find(|item| {
                item.is_stack
                    && dictionary.compatible(gun.dict_id, item.dict_id)
                    && dictionary.fits(item.dict_id, AMMO_SLOT)
                    && item_params::item_params(item.dict_id)
                        .is_some_and(|params| params.class == ItemClass::Ammunition)
            })
            .ok_or_else(|| format!("no ammunition of `{GUN_NAME}` fits slot {AMMO_SLOT}"))?;
        let rounds = dictionary
            .categories
            .iter()
            .find(|category| category.id == gun.category)
            .map(|category| category.optimal_count)
            .and_then(|count| u16::try_from(count).ok())
            .filter(|&count| count > 0)
            .ok_or_else(|| {
                format!(
                    "category {} of `{GUN_NAME}` has no ammunition amount",
                    gun.category
                )
            })?;
        Ok(Self {
            gun: gun.dict_id,
            ammo: ammo.dict_id,
            rounds,
        })
    }

    /// The Python of the server's game data ([`check`] refused data without one at start).
    pub fn shared() -> Self {
        static SHARED: std::sync::OnceLock<Revolver> = std::sync::OnceLock::new();
        *SHARED.get_or_init(|| {
            Revolver::of(Dictionary::shared())
                .expect("the game data has the Colt Python (cowboy::check)")
        })
    }
}

/// The game data has the Colt Python and its ammunition ([`Revolver::of`]); srp refuses to start
/// otherwise.
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    Revolver::of(Dictionary::shared())
        .map(drop)
        .map_err(|why| GameDataError(format!("{}: the Colt Python: {why}", data.origin)))
}

/// `profile` with the Python as the only gun of every kit, loaded or not (a kit's state stays):
///
/// - slot 7 holds the Python, slot 10 nothing;
/// - slot 8 holds `revolver`'s rounds of its ammunition, the other ammo slots (`AMMO_SLOTS`:
///   9, 11, 12, 19..=22) nothing;
/// - every other slot keeps what it held, the pockets packed as they were.
///
/// The Python and its ammunition are one new item each, whole and with no modifications or
/// upgrades, held by every kit, their ids unused by the profile's items before. An item no kit
/// holds any more (a gun or ammunition taken out) leaves the table.
pub fn arm(profile: &mut MatchProfile, revolver: &Revolver) {
    let used: Vec<u32> = profile.items.iter().map(|item| item.instance_id).collect();
    let mut fresh = (1..=u32::MAX).filter(|id| !used.contains(id));
    let mut fresh_id = || {
        fresh
            .next()
            .expect("a profile has fewer than u32::MAX items")
    };
    let gun = MatchItem {
        instance_id: fresh_id(),
        dict_id: revolver.gun,
        condition_or_stack: FULL_CONDITION,
        ..MatchItem::default()
    };
    let ammo = MatchItem {
        instance_id: fresh_id(),
        dict_id: revolver.ammo,
        // Every life's rounds: a spawn takes a slot's count out of the stack's store
        // (`snapshot_rules.md` §7.3), so one kit's worth would last one life.
        condition_or_stack: u32::from(revolver.rounds).saturating_mul(STORE_LIVES),
        ..MatchItem::default()
    };
    let (gun_at, ammo_at) = (profile.items.len() as u16, profile.items.len() as u16 + 1);
    profile.items.extend([gun, ammo]);
    for kit in &mut profile.kits {
        let mut slots = kit.slots;
        for &slot in WEAPON_SLOTS.iter().chain(&AMMO_SLOTS) {
            slots[usize::from(slot)] = None;
        }
        slots[usize::from(GUN_SLOT)] = Some((gun_at, 1));
        slots[usize::from(AMMO_SLOT)] = Some((ammo_at, revolver.rounds));
        let name = std::mem::take(&mut kit.name);
        *kit = MatchKit::new(kit.state, kit.id, name, slots, kit.modifiers);
    }
    profile.drop_unheld();
}

#[cfg(test)]
mod tests {
    use super::super::dictionary::PROFILE_SLOTS;
    use super::super::loadouts::{self, BotRole, POCKET_SLOTS};
    use super::super::match_msgs::static_info::{self, KIT_NOT_LOADED, KIT_USABLE};
    use super::super::skills::MODIFIERS;
    use super::*;
    use srp_validate::world::{Ammo, Body};

    type Worn = [Option<(u16, u16)>; PROFILE_SLOTS];
    /// A kit to make: loaded or not, and what it holds by `(slot, item name, count)`.
    type Kit<'a> = (bool, &'a [(u8, &'a str, u16)]);

    fn dict(name: &str) -> u16 {
        Dictionary::shared()
            .item_named(name)
            .unwrap_or_else(|| panic!("no item `{name}`"))
            .dict_id
    }

    /// A profile as the lobby makes one (`account_body`): each item once however many kits (and
    /// slots) hold it, its id `100 + its index`, with its default modifications.
    fn profile(kits: &[Kit]) -> MatchProfile {
        let dictionary = Dictionary::shared();
        let mut items: Vec<MatchItem> = Vec::new();
        let kits = kits
            .iter()
            .enumerate()
            .map(|(n, &(loaded, held))| {
                let mut slots = [None; PROFILE_SLOTS];
                for &(slot, name, count) in held {
                    let dict_id = dict(name);
                    assert!(dictionary.fits(dict_id, slot), "{name} in slot {slot}");
                    let index = items
                        .iter()
                        .position(|item| item.dict_id == dict_id)
                        .unwrap_or_else(|| {
                            let stack = dictionary.is_stack(dict_id);
                            items.push(MatchItem {
                                instance_id: 100 + items.len() as u32,
                                dict_id,
                                condition_or_stack: if stack { 500 } else { FULL_CONDITION },
                                modifications: loadouts::default_modifications(dict_id),
                                ..MatchItem::default()
                            });
                            items.len() - 1
                        });
                    slots[usize::from(slot)] = Some((index as u16, count));
                }
                let state = if loaded { KIT_USABLE } else { KIT_NOT_LOADED };
                MatchKit::new(state, n as u32 + 1, String::new(), slots, [0.0; MODIFIERS])
            })
            .collect();
        MatchProfile {
            team: 0,
            name: "cowboy".to_owned(),
            clan_tag: String::new(),
            protection: static_info::NO_PROTECTION,
            kits,
            items,
            squad: 0,
        }
    }

    /// Each kit's slots as `(dict id, count)`.
    fn worn(profile: &MatchProfile) -> Vec<Worn> {
        let dict_id = |index: u16| profile.items[usize::from(index)].dict_id;
        profile
            .kits
            .iter()
            .map(|kit| {
                kit.slots
                    .map(|held| held.map(|(index, count)| (dict_id(index), count)))
            })
            .collect()
    }

    /// `before`'s kits as [`arm`] makes them: the guns and the ammunition out, the Python and its
    /// rounds in.
    fn armed(before: &MatchProfile, revolver: &Revolver) -> Vec<Worn> {
        worn(before)
            .into_iter()
            .map(|mut slots| {
                for &slot in WEAPON_SLOTS.iter().chain(&AMMO_SLOTS) {
                    slots[usize::from(slot)] = None;
                }
                slots[7] = Some((revolver.gun, 1));
                slots[8] = Some((revolver.ammo, revolver.rounds));
                slots
            })
            .collect()
    }

    /// [`arm`] of `before` by its rules: the kits as [`armed`], their states, ids, names and
    /// modifiers kept, every item held, the ids unique, the two new items whole and bare, a
    /// profile a match is sent (`check_sent`: `check_profile`, the pockets packed), and each
    /// loaded kit's Python finding its rounds as the game cooks it.
    fn assert_armed(before: &MatchProfile) -> MatchProfile {
        let revolver = Revolver::shared();
        let mut after = before.clone();
        arm(&mut after, &revolver);
        assert_eq!(worn(&after), armed(before, &revolver));
        for (kit, was) in after.kits.iter().zip(&before.kits) {
            assert_eq!(
                (kit.state, kit.id, &kit.name, kit.modifiers),
                (was.state, was.id, &was.name, was.modifiers)
            );
        }
        for (index, item) in after.items.iter().enumerate() {
            let held = after
                .kits
                .iter()
                .flat_map(|kit| kit.slots.iter().flatten())
                .any(|&(at, _)| usize::from(at) == index);
            assert!(held, "item {index} ({}) is held by no kit", item.dict_id);
        }
        let mut ids: Vec<u32> = after.items.iter().map(|item| item.instance_id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), after.items.len(), "the ids are unique");
        let new: Vec<&MatchItem> = after
            .items
            .iter()
            .filter(|item| {
                before
                    .items
                    .iter()
                    .all(|was| was.instance_id != item.instance_id)
            })
            .collect();
        assert_eq!(new.len(), 2);
        for item in new {
            assert!(item.modifications.is_empty() && item.upgrades.is_empty());
            let whole = match item.dict_id == revolver.gun {
                true => FULL_CONDITION,
                // Every life's rounds (`STORE_LIVES`).
                false => u32::from(revolver.rounds) * STORE_LIVES,
            };
            assert_eq!(item.condition_or_stack, whole);
        }
        static_info::check_sent(0, &after, Dictionary::shared()).unwrap();
        for (n, kit) in after.kits.iter().enumerate() {
            if kit.state != KIT_USABLE {
                continue;
            }
            let inventory = after.inventory(n, Dictionary::shared()).unwrap();
            let gun = inventory
                .iter()
                .find(|item| item.slot == 7)
                .expect("a gun in slot 7");
            let one_ammo = matches!(&gun.body, Body::Weapon(gun) if gun.ammo == Ammo::Kit(1));
            assert!(one_ammo, "kit {}: {:?}", kit.id, gun.body);
        }
        after
    }

    #[test]
    fn the_test_data_has_the_python() {
        check(super::super::game_data::shared()).unwrap();
        let revolver = Revolver::shared();
        // The Colt Python, its .357 rounds, and the pistols' optimal 40: two clips of 20.
        assert_eq!(revolver.gun, dict("magnum"));
        assert_eq!(revolver.ammo, dict("ammo_.357m"));
        assert_eq!(revolver.rounds, 40);
        assert_eq!(item_params::shop_unit(revolver.ammo), 20);
    }

    #[test]
    fn a_sniper_kit_gets_the_python_and_keeps_the_rest() {
        let before = profile(&[(
            true,
            &[
                (7, "svd", 1),
                (8, "ammo_7.62x54_7n1", 20),
                (9, "ammo_7.62x54r_old", 10),
                (13, "medkit", 2),
                (14, "rgd-5", 1),
                (3, "bm_back_1", 1),
            ],
        )]);
        let after = assert_armed(&before);
        // The rifle and its two ammunitions leave; the medkits, the grenade, the backpack, the
        // Python and its rounds stay.
        assert_eq!(after.items.len(), 5);
        assert!(after.items.iter().all(|item| item.dict_id != dict("svd")));
    }

    #[test]
    fn a_kit_of_two_guns_gets_one_python() {
        let before = profile(&[
            (
                true,
                &[
                    (7, "ak_74n", 1),
                    (8, "ammo_5.45x39_7n6m", 90),
                    (10, "tt_33", 1),
                    (11, "ammo_7.62x25", 16),
                    (20, "ammo_7.62x25", 8),
                ],
            ),
            // Both guns one item.
            (
                true,
                &[
                    (7, "tt_33", 1),
                    (10, "tt_33", 1),
                    (8, "ammo_7.62x25", 24),
                    (13, "medkit", 1),
                ],
            ),
        ]);
        let after = assert_armed(&before);
        assert_eq!(after.items.len(), 3);
    }

    #[test]
    fn kits_sharing_ammunition_with_another_kit() {
        // Two kits of one ammunition item, and a kit that is not loaded holding it too: that one
        // is armed as well (and stays not loaded), so the item leaves the table.
        let before = profile(&[
            (
                true,
                &[
                    (7, "ak_74n", 1),
                    (8, "ammo_5.45x39_7n6m", 60),
                    (9, "ammo_5.45x39_7n6m", 30),
                ],
            ),
            (
                true,
                &[
                    (7, "ak_74m", 1),
                    (8, "ammo_5.45x39_7n6m", 120),
                    (13, "medkit", 3),
                ],
            ),
            (false, &[(7, "ak_74n", 1), (11, "ammo_5.45x39_7n6m", 30)]),
        ]);
        assert_eq!(before.items.len(), 4);
        let after = assert_armed(&before);
        assert_eq!(after.kits[2].state, KIT_NOT_LOADED);
        assert_eq!(after.items.len(), 3);
    }

    #[test]
    fn an_empty_kit_gets_the_python_too() {
        let mut before = profile(&[(true, &[(14, "medkit", 1)])]);
        before.kits.push(MatchKit::empty(2, ""));
        let after = assert_armed(&before);
        // The medkit is in the first pocket, the kit's packing, before and after.
        assert_eq!(
            worn(&after)[0][usize::from(POCKET_SLOTS[0])],
            Some((dict("medkit"), 1))
        );
        assert_eq!(after.items.len(), 3);
    }

    #[test]
    fn a_bots_profile_is_armed_alike() {
        for role in BotRole::ALL {
            let kits = loadouts::bot_kits(role);
            let before = MatchProfile::from_loadouts(
                "bot",
                1,
                &kits,
                [0.0; MODIFIERS],
                Dictionary::shared(),
                1,
            );
            assert_eq!(assert_armed(&before).kits.len(), 2);
        }
    }

    #[test]
    fn an_index_past_the_table_stays_past_it() {
        let mut profile = profile(&[(true, &[(7, "svd", 1), (13, "medkit", 1)])]);
        profile.kits[0].slots[14] = Some((40, 1));
        arm(&mut profile, &Revolver::shared());
        let (index, _) = profile.kits[0].slots[14].unwrap();
        assert!(usize::from(index) >= profile.items.len());
    }
}
