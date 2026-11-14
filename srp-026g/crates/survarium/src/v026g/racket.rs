//! Racket (`docs/notes/ffa.md`, "Twists"; the owner, 2026-11-14: "replaces all sniper rifles and
//! all silent weapons with ak-12"): a free-for-all's twist ([`super::twist::Twist::Racket`]). Every
//! sniper rifle and every silenced gun of a player's kits is an AK-12 for the match, the EOTech 552
//! sight on it ([`SIGHT`]), with the same amount of AK-12 ammo in every such kit. The server owns what a
//! match spawns (the static info's profiles, which every game cooks each life from), so the
//! change reaches every game, late joiners and replays; the accounts keep their own guns.

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::item_params::{self, ItemClass};
use super::loadouts::{AMMO_SLOTS, bot_gear::category};
use super::match_msgs::static_info::{FULL_CONDITION, MatchItem, MatchProfile, Upgrade};
use super::scopes::{SCOPE_SLOT, ScopeFits};
use super::twist::STORE_LIVES;

/// The gun every replaced one becomes, by its dictionary name.
pub const GUN: &str = "ak_12";

/// The sight every replaced gun's AK-12 wears, by its dictionary name: the shop's EOTech 552 (the
/// owner, 2026-11-14: "also for ak-12, people should have eotech collimator set on replaced
/// weapons"), on the AK-12 by the variant the game data makes for it (`fit_ak_12_eotech_552`).
pub const SIGHT: &str = "shop_scope_eotech_552";

/// The shot the other players hear of a silenced gun ([`item_params::ItemParams::shot_heard`]):
/// the AS Val's muffled one, which every silenced gun of the game but the sniper rifles fires
/// (the Val, the MP7 with its silencer and its Legend, the ported Kriss Vector). The SR-3M, the
/// 9A-91 and the MSBS fire loud shots of their own.
pub const SILENCED_SHOT: &str = "weapons/val_m";

/// The profile slots of the two weapons (`srp_validate::world::WEAPON_SLOTS`).
const WEAPON_SLOTS: [usize; 2] = [7, 10];

/// The AK-12 and the one ammo the data pairs with it, read from the game data once
/// ([`Arms::shared`]): what [`arm`] gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arms {
    gun: u16,
    /// The ammo Racket gives (`twist::Twist::gives`).
    pub ammo: u16,
    /// The rounds of AK-12 ammo a kit gets: the dictionary's `optimal_count` of the AK-12's
    /// category.
    rounds: u16,
    /// The variant that shows [`SIGHT`] on the AK-12 (`ScopeFits::variant`), when the game data
    /// has one; without it the AK-12 goes bare (the owner, 2026-11-14: the sight is behaviour, not
    /// correctness, so nothing refuses data without it).
    sight: Option<u16>,
}

impl Arms {
    /// The data's AK-12 and its ammo: exactly one ammo the dictionary pairs with it, which every
    /// ammo slot takes, and its category's amount; and its [`SIGHT`]'s variant, if the data has it.
    fn of(dictionary: &Dictionary, fits: &ScopeFits) -> Result<Self, String> {
        let item = dictionary
            .item_named(GUN)
            .ok_or_else(|| format!("the dictionary has no `{GUN}`"))?;
        let gun = item.dict_id;
        let rounds = dictionary
            .categories
            .iter()
            .find(|category| category.id == item.category)
            .and_then(|category| u16::try_from(category.optimal_count).ok())
            .filter(|&count| count > 0)
            .ok_or_else(|| format!("category {} of `{GUN}` has no ammo amount", item.category))?;
        let ammo: Vec<u16> = dictionary
            .items
            .iter()
            .map(|item| item.dict_id)
            .filter(|&id| is_ammo(id) && dictionary.compatible(gun, id))
            .collect();
        let &[ammo] = ammo.as_slice() else {
            return Err(format!("`{GUN}` fires {} kinds of ammo, not one: {ammo:?}", ammo.len()));
        };
        if let Some(slot) = AMMO_SLOTS.iter().find(|&&slot| !dictionary.fits(ammo, slot)) {
            return Err(format!("`{GUN}`'s ammo {ammo} does not fit ammo slot {slot}"));
        }
        let sight = dictionary
            .item_named(SIGHT)
            .and_then(|scope| fits.variant(gun, scope.dict_id));
        Ok(Self {
            gun,
            ammo,
            rounds,
            sight,
        })
    }

    /// The arms of the server's game data ([`check`] refused data without them at start).
    pub fn shared() -> Self {
        static SHARED: std::sync::OnceLock<Arms> = std::sync::OnceLock::new();
        *SHARED.get_or_init(|| {
            Arms::of(Dictionary::shared(), ScopeFits::shared())
                .expect("Racket's arms are in the data (racket::check)")
        })
    }
}

fn is_ammo(dict_id: u16) -> bool {
    item_params::item_params(dict_id).is_some_and(|params| params.class == ItemClass::Ammunition)
}

/// Whether Racket replaces the gun `dict_id`: a sniper rifle (its golden and premium copies are
/// of their plain gun's category), or a gun whose shot is [`SILENCED_SHOT`].
pub fn replaces(dictionary: &Dictionary, dict_id: u16) -> bool {
    dictionary
        .item(dict_id)
        .is_some_and(|item| item.category == category::SNIPER_RIFLES)
        || item_params::item_params(dict_id).and_then(|params| params.shot_heard)
            == Some(SILENCED_SHOT)
}

/// `profile` armed for Racket:
///
/// - every gun of its table that Racket [`replaces`] is a whole AK-12 wearing the [`SIGHT`] and no
///   other modification or upgrade (the gun's own scope, camo and bonuses stay its own), of the
///   same instance id, in every kit and slot that held it;
/// - in each kit that held one, the ammo that none of its guns fires now leaves its slots, and
///   the kit has the AK-12's category's amount (`optimal_count`) of AK-12 ammo, in the first slot
///   that ammo left or else its first empty ammo slot (a kit already holding AK-12 ammo keeps
///   its own; one with no ammo slot free has its AK-12's magazine alone);
/// - that AK-12 ammo is one new stack every such kit holds, its store the amount
///   [`STORE_LIVES`] times;
/// - an item no kit holds any more (the ammo that left) leaves the table;
/// - nothing else of the kits changes: armour, pockets, artefacts, skills.
pub fn arm(profile: &mut MatchProfile, arms: &Arms) {
    let dictionary = Dictionary::shared();
    let replaced: Vec<usize> = (0..profile.items.len())
        .filter(|&index| replaces(dictionary, profile.items[index].dict_id))
        .collect();
    if replaced.is_empty() {
        return;
    }
    let next_id = profile.items.iter().map(|item| item.instance_id).max().unwrap_or(0);
    let sight = arms
        .sight
        .map_or_else(Vec::new, |variant| sight_records(variant, next_id.wrapping_add(2)));
    for &index in &replaced {
        let item = &mut profile.items[index];
        *item = MatchItem {
            instance_id: item.instance_id,
            dict_id: arms.gun,
            condition_or_stack: FULL_CONDITION,
            upgrades: sight.clone(),
            ..MatchItem::default()
        };
    }
    let items = &mut profile.items;
    let mut stack: Option<u16> = None;
    for kit in &mut profile.kits {
        let held = |slot: usize| kit.slots[slot].map(|(index, _)| usize::from(index));
        if !WEAPON_SLOTS
            .into_iter()
            .any(|slot| held(slot).is_some_and(|index| replaced.contains(&index)))
        {
            continue;
        }
        let guns: Vec<u16> = WEAPON_SLOTS
            .into_iter()
            .filter_map(held)
            .filter_map(|index| items.get(index).map(|item| item.dict_id))
            .collect();
        let mut first = None;
        for slot in AMMO_SLOTS.map(usize::from) {
            let Some((index, _)) = kit.slots[slot] else {
                continue;
            };
            let Some(ammo) = items.get(usize::from(index)).map(|item| item.dict_id) else {
                continue;
            };
            if guns.iter().any(|&gun| dictionary.compatible(gun, ammo)) {
                continue;
            }
            first.get_or_insert(slot);
            kit.slots[slot] = None;
        }
        let fired = |slot: u8| {
            kit.slots[usize::from(slot)]
                .and_then(|(index, _)| items.get(usize::from(index)))
                .is_some_and(|item| dictionary.compatible(arms.gun, item.dict_id))
        };
        if AMMO_SLOTS.into_iter().any(fired) {
            continue;
        }
        let empty = AMMO_SLOTS
            .map(usize::from)
            .into_iter()
            .find(|&slot| kit.slots[slot].is_none());
        let Some(slot) = first.or(empty) else {
            continue;
        };
        let at = *stack.get_or_insert_with(|| {
            items.push(MatchItem {
                instance_id: next_id.wrapping_add(1),
                dict_id: arms.ammo,
                condition_or_stack: u32::from(arms.rounds).saturating_mul(STORE_LIVES),
                ..MatchItem::default()
            });
            items.len() as u16 - 1
        });
        kit.slots[slot] = Some((at, arms.rounds));
    }
    profile.drop_unheld();
}

/// The records that put the sight `variant` on a gun: a scope's ([`SCOPE_SLOT`]), packed as every
/// weapon's records are (`weapon_kit::pack`), as an owned scope's reach a match. `id` is the scope
/// item's: no item of the profile, as an owned scope is in no kit.
fn sight_records(variant: u16, id: u32) -> Vec<Upgrade> {
    let scope = super::lobby::Upgrade {
        id,
        a: u32::from(variant),
        value: 0,
        param: SCOPE_SLOT,
    };
    super::weapon_kit::pack(&[scope])
        .into_iter()
        .map(|record| Upgrade {
            id: record.id,
            unknown_4: record.a,
            unknown_8: record.value,
            param: record.param,
        })
        .collect()
}

/// The game data has Racket's AK-12 and its one ammo, which every ammo slot takes; srp refuses to
/// start otherwise. (Its sight is not checked: without it the AK-12 goes bare.)
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    Arms::of(Dictionary::shared(), ScopeFits::shared())
        .map(|_| ())
        .map_err(|cause| GameDataError(format!("{}: Racket: {cause}", data.origin)))
}

#[cfg(test)]
mod tests {
    use super::super::loadouts::Loadout;
    use super::super::match_msgs::static_info::MatchKit;
    use super::super::skills::MODIFIERS;
    use super::*;

    fn id(name: &str) -> u16 {
        Dictionary::shared()
            .item_named(name)
            .unwrap_or_else(|| panic!("{name}"))
            .dict_id
    }

    fn named(profile: &MatchProfile, kit: usize, slot: usize) -> Option<(&str, u16)> {
        let (index, count) = profile.kits[kit].slots[slot]?;
        let item = &profile.items[usize::from(index)];
        Some((Dictionary::shared().item(item.dict_id)?.name.as_str(), count))
    }

    fn profile(kits: &[&[(u8, &str, u32)]]) -> MatchProfile {
        let dictionary = Dictionary::shared();
        let loadouts: Vec<Loadout> = kits
            .iter()
            .map(|kit| {
                let names: Vec<(u8, &str)> = kit.iter().map(|&(slot, name, _)| (slot, name)).collect();
                let mut loadout = Loadout::from_names(dictionary, names.iter().copied()).expect("a loadout");
                for (item, &(_, _, count)) in loadout.items.iter_mut().zip(kit.iter()) {
                    item.count = count;
                }
                loadout
            })
            .collect();
        let loadouts: Vec<&Loadout> = loadouts.iter().collect();
        MatchProfile::from_loadouts("p", 4, &loadouts, [0.0; MODIFIERS], dictionary, 100)
    }

    #[test]
    fn it_replaces_the_sniper_rifles_and_the_silenced_guns_alone() {
        let dictionary = Dictionary::shared();
        let replaced = |name: &str| replaces(dictionary, id(name));
        for name in ["svd", "mosin_nagant", "vss", "vssk", "vsk_94", "rem_700", "val"] {
            assert!(replaced(name), "{name}");
        }
        for name in ["ak_74u", "ak_12", "sr3m", "spas_12", "rpk", "magnum"] {
            assert!(!replaced(name), "{name}");
        }
        // The shot the data gives the Val, and a loud gun's.
        let heard = |name: &str| item_params::item_params(id(name)).and_then(|p| p.shot_heard);
        assert_eq!(heard("val"), Some(SILENCED_SHOT));
        assert_eq!(heard("ak_12"), Some("weapons/ak_12_m"));
        assert_eq!(heard("ammo_6.5x39"), None);
        let arms = Arms::of(dictionary, ScopeFits::shared()).unwrap();
        assert_eq!((arms.gun, arms.ammo), (id("ak_12"), id("ammo_6.5x39")));
        assert!(arms.rounds > 0);
    }

    #[test]
    fn a_sniper_kit_gets_an_ak_12_and_its_ammo() {
        let mut sniper = profile(&[&[
            (2, "ren_torso_6", 1),
            (7, "svd", 1),
            (8, "ammo_7.62x54_7n1", 40),
            (10, "val", 1),
            (11, "ammo_sp6", 60),
            (13, "rgd-5", 2),
        ]]);
        let before = sniper.clone();
        arm(&mut sniper, &Arms::shared());
        assert_eq!(named(&sniper, 0, 7).map(|(name, _)| name), Some("ak_12"));
        assert_eq!(named(&sniper, 0, 10).map(|(name, _)| name), Some("ak_12"));
        // The guns keep their instance ids, whole and bare.
        for slot in [7, 10] {
            let (index, _) = sniper.kits[0].slots[slot].unwrap();
            let (was, _) = before.kits[0].slots[slot].unwrap();
            let (gun, old) = (&sniper.items[usize::from(index)], &before.items[usize::from(was)]);
            assert_eq!((gun.instance_id, gun.condition_or_stack), (old.instance_id, FULL_CONDITION));
            assert!(gun.modifications.is_empty() && gun.upgrades.is_empty());
        }
        // Both guns' ammo leaves; the AK-12 ammo's amount is in the first slot it left.
        let rounds = Arms::shared().rounds;
        assert_eq!(named(&sniper, 0, 8), Some(("ammo_6.5x39", rounds)));
        assert_eq!(named(&sniper, 0, 11), None);
        // The rest of the kit as it was.
        for slot in [2, 13] {
            assert_eq!(named(&sniper, 0, slot), named(&before, 0, slot));
        }
        let (at, _) = sniper.kits[0].slots[8].unwrap();
        let new = &sniper.items[usize::from(at)];
        assert_eq!(new.instance_id, before.items.iter().map(|i| i.instance_id).max().unwrap() + 1);
        assert_eq!(new.condition_or_stack, u32::from(rounds) * STORE_LIVES);
        // The ammo that left is out of the table: torso, two AK-12s' entries, grenades, the stack.
        assert_eq!(sniper.items.len(), before.items.len() - 1);
    }

    #[test]
    fn a_loud_kit_and_a_loud_guns_ammo_stay() {
        // An AK-74U kit: nothing to do.
        let mut loud = profile(&[&[(7, "ak_74u", 1), (8, "ammo_5.45x39_7n10", 60)]]);
        let before = loud.clone();
        arm(&mut loud, &Arms::shared());
        assert_eq!(loud, before);
        // A shotgun beside a sniper rifle: the shotgun's shells stay, the rifle's rounds go.
        let mut mixed = profile(&[
            &[
                (7, "svd", 1),
                (8, "ammo_7.62x54_7n1", 20),
                (10, "spas_12", 1),
                (11, "ammo_12mm_buck", 16),
            ],
            &[(7, "ak_74u", 1), (8, "ammo_5.45x39_7n10", 60)],
        ]);
        let before = mixed.clone();
        arm(&mut mixed, &Arms::shared());
        assert_eq!(named(&mixed, 0, 10), named(&before, 0, 10));
        assert_eq!(named(&mixed, 0, 11), named(&before, 0, 11));
        assert_eq!(named(&mixed, 0, 8).map(|(name, _)| name), Some("ammo_6.5x39"));
        // The other kit untouched.
        for slot in 0..mixed.kits[1].slots.len() {
            assert_eq!(named(&mixed, 1, slot), named(&before, 1, slot));
        }
    }

    /// A Val beside an SR-3M on one SP-6: the SR-3M keeps it, and the AK-12 still has its own
    /// ammo, the category's amount, in the first empty ammo slot.
    #[test]
    fn an_ak_12_whose_ammo_a_loud_gun_keeps_has_its_own() {
        let mut kit = profile(&[&[(7, "val", 1), (8, "ammo_sp6", 60), (10, "sr3m", 1)]]);
        let sp6 = kit.kits[0].slots[8];
        kit.kits[0].slots[11] = sp6;
        arm(&mut kit, &Arms::shared());
        let arms = Arms::shared();
        assert_eq!(named(&kit, 0, 7).map(|(name, _)| name), Some("ak_12"));
        assert_eq!(named(&kit, 0, 8), Some(("ammo_sp6", 60)));
        assert_eq!(named(&kit, 0, 11), Some(("ammo_sp6", 60)));
        assert_eq!(named(&kit, 0, 9), Some(("ammo_6.5x39", arms.rounds)));
        // A sniper rifle with no ammo at all: the same amount.
        let mut dry = profile(&[&[(7, "svd", 1)]]);
        arm(&mut dry, &Arms::shared());
        assert_eq!(named(&dry, 0, 8), Some(("ammo_6.5x39", arms.rounds)));
    }

    #[test]
    fn a_gun_two_kits_share_is_replaced_in_both() {
        let mut shared = profile(&[&[(7, "vss", 1), (8, "ammo_sp5", 20)]]);
        // A second kit holding the same table entries, as an account's body sends one.
        let second = MatchKit::new(2, 2, String::new(), shared.kits[0].slots, [0.0; MODIFIERS]);
        shared.kits.push(second);
        arm(&mut shared, &Arms::shared());
        for kit in 0..2 {
            assert_eq!(named(&shared, kit, 7).map(|(name, _)| name), Some("ak_12"));
            assert_eq!(named(&shared, kit, 8).map(|(name, _)| name), Some("ammo_6.5x39"));
        }
        // One new stack both kits hold, lasting every life; the SP-5 no kit holds now leaves:
        // the AK-12 (the VSS's entry) and its ammo.
        let (first, rounds) = shared.kits[0].slots[8].unwrap();
        assert_eq!(shared.kits[1].slots[8], Some((first, rounds)));
        let stack = &shared.items[usize::from(first)];
        assert_eq!(stack.condition_or_stack, u32::from(rounds) * STORE_LIVES);
        assert_eq!(shared.items.len(), 2);
    }
}
