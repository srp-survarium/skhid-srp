//! Saloon, the cowboy twist of a free-for-all (`docs/notes/ffa-cowboy.md`): every kit's guns
//! become a Winchester 1894 (dictionary item `winchester_1894`) in weapon slot 7 and a Colt Python
//! (`magnum`) in slot 10, or a Colt Python alone in slot 7, by the match's [`Variation`], with
//! their ammunition, in the match profile the static info sends ([`arm`]). Every game cooks each
//! life from those profiles, so the change reaches every client, a late joiner and a replay alike.

use srp_validate::world::{AMMO_SLOTS, WEAPON_SLOTS};
use weapon_pickup::CarryRule;

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::item_params::{self, ItemClass};
use super::match_msgs::static_info::{FULL_CONDITION, MatchItem, MatchKit, MatchProfile};
use super::skills::slot_modifier::{DISABLE_SECOND_WEAPON, USE_TWO_DOUBLE_HANDED_WEAPONS};
use super::twist::STORE_LIVES;

/// The dictionary name of the Colt Python.
pub const COLT: &str = "magnum";
/// The dictionary name of the Winchester 1894.
pub const WINCHESTER: &str = "winchester_1894";

/// Weapon 1's slot and the ammo slot its rounds go into (its first).
const FIRST: (u8, u8) = (7, 8);
/// Weapon 2's slot and the ammo slot its rounds go into (its first).
const SECOND: (u8, u8) = (10, 11);

/// The guns of a Saloon match (the owner, 2026-11-14: "saloon should have two variations with
/// winchesters and colts or just with colts"), drawn half and half with its card
/// (`twist::SHARES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variation {
    /// The Winchester in weapon 1's slot and the Colt in weapon 2's; a kit that carries one weapon
    /// (`disable_second_weapon`: Deep Pockets, Supply Pocket) the Winchester alone. A clip for
    /// every ammo bag of the kit, as Racket gives its AK-12's, the Winchester's first.
    WinchesterAndColt,
    /// The Colt alone in weapon 1's slot, with its category's rounds.
    Colt,
}

/// A gun Saloon gives and its ammunition, read from the game data ([`Gun::of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Gun {
    gun: u16,
    /// The first ammunition of the dictionary the gun takes that fits its ammo slots.
    ammo: u16,
    /// A clip of that ammo: its rounds (`clip_size`) and the ammo bags it takes (`required_bags`).
    clip: u16,
    clip_bags: u32,
}

impl Gun {
    /// The gun `name` of `dictionary`, to go in each of `slots` (a weapon slot and its ammo slot),
    /// or what the data lacks for it: the item, its states, its slots, an ammunition it takes that
    /// fits the ammo slots, that ammunition's clip.
    fn of(dictionary: &Dictionary, name: &str, slots: &[(u8, u8)]) -> Result<Self, String> {
        let gun = dictionary
            .item_named(name)
            .ok_or_else(|| format!("no item `{name}`"))?;
        if gun.is_stack || item_params::item_params(gun.dict_id).is_none_or(|p| p.gun.is_none()) {
            return Err(format!("`{name}` is no gun"));
        }
        if let Some(&(slot, _)) = slots
            .iter()
            .find(|&&(slot, _)| !dictionary.fits(gun.dict_id, slot))
        {
            return Err(format!("`{name}` does not fit slot {slot}"));
        }
        let ammo = dictionary
            .items
            .iter()
            .find(|item| {
                item.is_stack
                    && dictionary.compatible(gun.dict_id, item.dict_id)
                    && slots
                        .iter()
                        .all(|&(_, slot)| dictionary.fits(item.dict_id, slot))
                    && item_params::item_params(item.dict_id)
                        .is_some_and(|params| params.class == ItemClass::Ammunition)
            })
            .ok_or_else(|| format!("no ammunition of `{name}` fits its ammo slots"))?;
        let (clip, clip_bags) = item_params::item_params(ammo.dict_id)
            .and_then(|params| Some((u16::try_from(params.clip_size).ok()?, params.required_bags)))
            .filter(|&(clip, bags)| clip > 0 && bags > 0)
            .ok_or_else(|| {
                format!(
                    "`{name}`'s ammo {} has no clip size or ammo bags",
                    ammo.dict_id
                )
            })?;
        Ok(Self {
            gun: gun.dict_id,
            ammo: ammo.dict_id,
            clip,
            clip_bags,
        })
    }

    /// Its rounds for `bags` ammo bags: a clip for each clip's bags.
    fn rounds(&self, bags: u32) -> u16 {
        u16::try_from(bags / self.clip_bags)
            .unwrap_or(u16::MAX)
            .saturating_mul(self.clip)
    }
}

/// Saloon's guns and their ammunition, read from the game data once ([`Guns::shared`]): what
/// [`arm`] gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guns {
    /// [`WINCHESTER`], in weapon 1's slot.
    winchester: Gun,
    /// [`COLT`], in weapon 1's slot alone or in weapon 2's beside the Winchester.
    colt: Gun,
    /// The Colt's rounds when it is alone: the dictionary's `optimal_count` of its category.
    colt_alone: u16,
}

impl Guns {
    /// The guns of `dictionary`, or what the data lacks for one of them ([`Gun::of`]), or the
    /// Colt's category's amount.
    pub fn of(dictionary: &Dictionary) -> Result<Self, String> {
        let winchester = Gun::of(dictionary, WINCHESTER, &[FIRST])
            .map_err(|why| format!("the Winchester 1894: {why}"))?;
        let colt = Gun::of(dictionary, COLT, &[FIRST, SECOND])
            .map_err(|why| format!("the Colt Python: {why}"))?;
        let category = dictionary.item(colt.gun).map(|item| item.category);
        let colt_alone = dictionary
            .categories
            .iter()
            .find(|entry| Some(entry.id) == category)
            .map(|entry| entry.optimal_count)
            .and_then(|count| u16::try_from(count).ok())
            .filter(|&count| count > 0)
            .ok_or_else(|| {
                format!("the Colt Python: category {category:?} has no ammunition amount")
            })?;
        Ok(Self {
            winchester,
            colt,
            colt_alone,
        })
    }

    /// The guns of the server's game data ([`check`] refused data without them at start).
    pub fn shared() -> Self {
        static SHARED: std::sync::OnceLock<Guns> = std::sync::OnceLock::new();
        *SHARED.get_or_init(|| {
            Guns::of(Dictionary::shared()).expect("the game data has Saloon's guns (cowboy::check)")
        })
    }

    /// The ammunition of the guns: the Winchester's and the Colt's.
    pub fn ammo(&self) -> [u16; 2] {
        [self.winchester.ammo, self.colt.ammo]
    }
}

/// The game data has the Winchester 1894, the Colt Python and their ammunition ([`Guns::of`]);
/// srp refuses to start otherwise.
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    Guns::of(Dictionary::shared())
        .map(drop)
        .map_err(|why| GameDataError(format!("{}: Saloon: {why}", data.origin)))
}

/// `profile` with `variation`'s guns as the only guns of every kit, loaded or not (a kit's state
/// stays):
///
/// - [`Variation::Colt`]: slot 7 holds the Colt and slot 8 its category's rounds;
/// - [`Variation::WinchesterAndColt`]: slot 7 holds the Winchester and slot 10 the Colt, or
///   nothing in a kit that carries one weapon; the kit's ammo bags as the lobby counts them (the
///   player's, its worn items' and its skills'; `racket::free_bags`) are a clip each, the
///   Winchester taking the first and every other one, the Colt the rest: the Winchester's rounds
///   in slot 8, the Colt's in slot 11, none for a gun with no clip;
/// - the other weapon and ammo slots (`WEAPON_SLOTS`, `AMMO_SLOTS`: 9, 12, 19..=22) nothing;
/// - every other slot keeps what it held, the pockets packed as they were.
///
/// Each gun and each ammunition given is one new item, whole and with no modifications or
/// upgrades, held by every kit that has it, their ids unused by the profile's items before; an
/// ammunition's store is the largest kit's rounds of it [`STORE_LIVES`] times. An item no kit holds
/// any more (a gun or ammunition taken out) leaves the table.
pub fn arm(profile: &mut MatchProfile, guns: &Guns, variation: Variation) {
    let used: Vec<u32> = profile.items.iter().map(|item| item.instance_id).collect();
    let mut fresh = (1..=u32::MAX).filter(|id| !used.contains(id));
    let base = profile.items.len() as u16;
    // The Colt and its ammunition first: alone, they take the lowest ids.
    for gun in [guns.colt, guns.winchester] {
        for (dict_id, condition_or_stack) in [(gun.gun, FULL_CONDITION), (gun.ammo, 0)] {
            profile.items.push(MatchItem {
                instance_id: fresh
                    .next()
                    .expect("a profile has fewer than u32::MAX items"),
                dict_id,
                condition_or_stack,
                ..MatchItem::default()
            });
        }
    }
    let colt = (base, base + 1);
    let winchester = (base + 2, base + 3);
    let items = &mut profile.items;
    for kit in &mut profile.kits {
        for &slot in WEAPON_SLOTS.iter().chain(&AMMO_SLOTS) {
            kit.slots[usize::from(slot)] = None;
        }
        // Each gun given: its slots, its item and its ammunition's, its rounds.
        let given = match variation {
            Variation::Colt => vec![(FIRST, colt, guns.colt_alone)],
            Variation::WinchesterAndColt => {
                let bags = super::racket::free_bags(kit, items);
                let carry = CarryRule::from_modifiers(
                    kit.modifiers[DISABLE_SECOND_WEAPON],
                    kit.modifiers[USE_TWO_DOUBLE_HANDED_WEAPONS],
                );
                match carry {
                    CarryRule::OneWeapon => vec![(FIRST, winchester, guns.winchester.rounds(bags))],
                    CarryRule::OneDoubleHanded | CarryRule::TwoDoubleHanded => vec![
                        (FIRST, winchester, guns.winchester.rounds(bags.div_ceil(2))),
                        (SECOND, colt, guns.colt.rounds(bags / 2)),
                    ],
                }
            }
        };
        let mut slots = kit.slots;
        for ((gun_slot, ammo_slot), (gun_at, ammo_at), rounds) in given {
            slots[usize::from(gun_slot)] = Some((gun_at, 1));
            if rounds == 0 {
                continue;
            }
            slots[usize::from(ammo_slot)] = Some((ammo_at, rounds));
            // Every life's rounds: a spawn takes a slot's count out of the stack's store
            // (`snapshot_rules.md` §7.3), so one kit's worth would last one life.
            let store = &mut items[usize::from(ammo_at)].condition_or_stack;
            *store = (*store).max(u32::from(rounds).saturating_mul(STORE_LIVES));
        }
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

    /// `before`'s kits as [`arm`] makes them of [`Variation::Colt`]: the guns and the ammunition
    /// out, the Python and its rounds in.
    fn armed(before: &MatchProfile, guns: &Guns) -> Vec<Worn> {
        worn(before)
            .into_iter()
            .map(|mut slots| {
                for &slot in WEAPON_SLOTS.iter().chain(&AMMO_SLOTS) {
                    slots[usize::from(slot)] = None;
                }
                slots[7] = Some((guns.colt.gun, 1));
                slots[8] = Some((guns.colt.ammo, guns.colt_alone));
                slots
            })
            .collect()
    }

    /// [`arm`] of `before` with the Colt alone by its rules: the kits as [`armed`], their states, ids, names and
    /// modifiers kept, every item held, the ids unique, the two new items whole and bare, a
    /// profile a match is sent (`check_sent`: `check_profile`, the pockets packed), and each
    /// loaded kit's Python finding its rounds as the game cooks it.
    fn assert_armed(before: &MatchProfile) -> MatchProfile {
        let guns = Guns::shared();
        let mut after = before.clone();
        arm(&mut after, &guns, Variation::Colt);
        assert_eq!(worn(&after), armed(before, &guns));
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
            let whole = match item.dict_id == guns.colt.gun {
                true => FULL_CONDITION,
                // Every life's rounds (`STORE_LIVES`).
                false => u32::from(guns.colt_alone) * STORE_LIVES,
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
    fn the_test_data_has_the_guns() {
        check(super::super::game_data::shared()).unwrap();
        let guns = Guns::shared();
        // The Colt Python, its .357 rounds, and the pistols' optimal 40: two clips of 20.
        assert_eq!(guns.colt.gun, dict("magnum"));
        assert_eq!(guns.colt.ammo, dict("ammo_.357m"));
        assert_eq!(guns.colt_alone, 40);
        assert_eq!(item_params::shop_unit(guns.colt.ammo), 20);
        // The Winchester and its .30-30: clips of 20, one bag each, as the .357's.
        assert_eq!(guns.winchester.gun, dict("winchester_1894"));
        assert_eq!(guns.winchester.ammo, dict("ammo_.30-30"));
        assert_eq!((guns.winchester.clip, guns.winchester.clip_bags), (20, 1));
        assert_eq!((guns.colt.clip, guns.colt.clip_bags), (20, 1));
    }

    /// The Winchester and the Colt: a kit of the player's 3 ammo bags has two clips for the
    /// Winchester and one for the Colt; a kit that carries one weapon the Winchester alone, with
    /// a clip for each bag.
    #[test]
    fn the_winchester_and_the_colt_share_the_bags() {
        let mut before = profile(&[
            (
                true,
                &[
                    (7, "svd", 1),
                    (8, "ammo_7.62x54_7n1", 20),
                    (10, "tt_33", 1),
                    (11, "ammo_7.62x25", 16),
                    (13, "medkit", 2),
                ],
            ),
            (true, &[(7, "ak_74n", 1), (8, "ammo_5.45x39_7n6m", 90)]),
        ]);
        before.kits[1].modifiers[DISABLE_SECOND_WEAPON] = 1.0;
        let guns = Guns::shared();
        let mut after = before.clone();
        arm(&mut after, &guns, Variation::WinchesterAndColt);
        let (winchester, colt) = (guns.winchester, guns.colt);
        let worn = worn(&after);
        let held = |kit: usize, slot: usize| worn[kit][slot];
        assert_eq!(held(0, 7), Some((winchester.gun, 1)));
        assert_eq!(held(0, 8), Some((winchester.ammo, 40)));
        assert_eq!(held(0, 10), Some((colt.gun, 1)));
        assert_eq!(held(0, 11), Some((colt.ammo, 20)));
        assert_eq!(
            held(0, usize::from(POCKET_SLOTS[0])),
            Some((dict("medkit"), 2))
        );
        assert_eq!(held(1, 7), Some((winchester.gun, 1)));
        assert_eq!(held(1, 8), Some((winchester.ammo, 60)));
        assert_eq!((held(1, 10), held(1, 11)), (None, None));
        for slot in [9, 12, 19, 20, 21, 22] {
            assert_eq!((held(0, slot), held(1, slot)), (None, None), "slot {slot}");
        }
        // The two guns, their two stacks (each the largest kit's rounds a thousand times) and
        // the medkit.
        let store = |dict_id| {
            let item = after
                .items
                .iter()
                .find(|item| item.dict_id == dict_id)
                .unwrap();
            item.condition_or_stack
        };
        assert_eq!(after.items.len(), 5);
        assert_eq!(store(winchester.ammo), 60 * STORE_LIVES);
        assert_eq!(store(colt.ammo), 20 * STORE_LIVES);
        static_info::check_sent(0, &after, Dictionary::shared()).unwrap();
        let inventory = after.inventory(0, Dictionary::shared()).unwrap();
        for slot in [7, 10] {
            let gun = inventory
                .iter()
                .find(|item| item.slot == slot)
                .expect("a gun");
            let one_ammo = matches!(&gun.body, Body::Weapon(gun) if gun.ammo == Ammo::Kit(1));
            assert!(one_ammo, "slot {slot}: {:?}", gun.body);
        }
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
        arm(&mut profile, &Guns::shared(), Variation::Colt);
        let (index, _) = profile.kits[0].slots[14].unwrap();
        assert!(usize::from(index) >= profile.items.len());
    }
}
