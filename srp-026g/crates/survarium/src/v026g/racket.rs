//! Racket (`docs/notes/ffa.md`, "Twists"; the owner, 2026-11-14: "replaces all sniper rifles and
//! all silent weapons with ak-12"): a free-for-all's twist ([`super::twist::Twist::Racket`]). Every
//! sniper rifle and every silenced gun of a player's kits is an AK-12 for the match, and the ammo
//! its kit's guns can no longer fire is AK-12 ammo in the same ammo bags. The server owns what a
//! match spawns (the static info's profiles, which every game cooks each life from), so the
//! change reaches every game, late joiners and replays; the accounts keep their own guns.

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::item_params::{self, ItemClass};
use super::loadouts::{AMMO_SLOTS, bot_gear::category};
use super::match_msgs::static_info::{FULL_CONDITION, MatchItem, MatchProfile};
use super::twist::STORE_LIVES;

/// The gun every replaced one becomes, by its dictionary name.
pub const GUN: &str = "ak_12";

/// The shot the other players hear of a silenced gun ([`item_params::ItemParams::shot_heard`]):
/// the AS Val's muffled one, which every silenced gun of the game but the sniper rifles fires
/// (the Val, the MP7 with its silencer and its Legend, the ported Kriss Vector). The SR-3M, the
/// 9A-91 and the MSBS fire loud shots of their own.
pub const SILENCED_SHOT: &str = "weapons/val_m";

/// The profile slots of the two weapons (`srp_validate::world::WEAPON_SLOTS`).
const WEAPON_SLOTS: [usize; 2] = [7, 10];

/// The AK-12 and the one ammo the data pairs with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Arms {
    gun: u16,
    ammo: u16,
}

impl Arms {
    /// The data's AK-12 and its ammo: exactly one ammo the dictionary pairs with it, which every
    /// ammo slot takes.
    fn of(dictionary: &Dictionary) -> Result<Self, String> {
        let gun = dictionary
            .item_named(GUN)
            .ok_or_else(|| format!("the dictionary has no `{GUN}`"))?
            .dict_id;
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
        Ok(Self { gun, ammo })
    }
}

fn is_ammo(dict_id: u16) -> bool {
    item_params::item_params(dict_id).is_some_and(|params| params.class == ItemClass::Ammunition)
}

/// The ammo Racket gives: the AK-12's.
pub fn ammo() -> u16 {
    Arms::of(Dictionary::shared())
        .expect("Racket's arms are in the data (racket::check at start)")
        .ammo
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

/// The ammo bags `count` rounds of `dict_id` take: a bag share (`required_bags`, at least one)
/// for each clip begun.
fn bags_of(dict_id: u16, count: u32) -> u32 {
    let required = item_params::item_params(dict_id).map_or(1, |params| params.required_bags.max(1));
    count.div_ceil(item_params::shop_unit(dict_id)) * required
}

/// The rounds of `dict_id` that `bags` ammo bags hold: its whole clips.
fn rounds_in(dict_id: u16, bags: u32) -> u32 {
    let required = item_params::item_params(dict_id).map_or(1, |params| params.required_bags.max(1));
    bags / required * item_params::shop_unit(dict_id)
}

/// `profile` armed for Racket:
///
/// - every gun of its table that Racket [`replaces`] is a whole AK-12 with no modification and no
///   upgrade (the gun's own scope, camo and bonuses stay its own), of the same instance id, in
///   every kit and slot that held it;
/// - in each kit that held one, the ammo of its ammo slots that none of its guns fires now leaves
///   those slots, and its ammo bags (a share for each clip begun) are AK-12 ammo, whole clips, in
///   the first slot it left. A kit with no such ammo has its AK-12's magazine alone;
/// - that AK-12 ammo is one new stack every such kit holds, whose store lasts [`STORE_LIVES`] of
///   the most rounds a kit takes;
/// - an item no kit holds any more (the ammo that left) leaves the table;
/// - nothing else of the kits changes: armour, pockets, artefacts, skills.
pub fn arm(profile: &mut MatchProfile, dictionary: &Dictionary) {
    let arms = Arms::of(dictionary).expect("Racket's arms are in the data (racket::check at start)");
    let replaced: Vec<usize> = (0..profile.items.len())
        .filter(|&index| replaces(dictionary, profile.items[index].dict_id))
        .collect();
    if replaced.is_empty() {
        return;
    }
    for &index in &replaced {
        let item = &mut profile.items[index];
        *item = MatchItem {
            instance_id: item.instance_id,
            dict_id: arms.gun,
            condition_or_stack: FULL_CONDITION,
            ..MatchItem::default()
        };
    }
    let next_id = profile.items.iter().map(|item| item.instance_id).max().unwrap_or(0);
    let items = &mut profile.items;
    let mut stack: Option<(u16, u32)> = None;
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
        let mut bags = 0_u32;
        let mut first = None;
        for slot in AMMO_SLOTS.map(usize::from) {
            let Some((index, count)) = kit.slots[slot] else {
                continue;
            };
            let Some(ammo) = items.get(usize::from(index)).map(|item| item.dict_id) else {
                continue;
            };
            if guns.iter().any(|&gun| dictionary.compatible(gun, ammo)) {
                continue;
            }
            bags = bags.saturating_add(bags_of(ammo, u32::from(count)));
            first.get_or_insert(slot);
            kit.slots[slot] = None;
        }
        let rounds = rounds_in(arms.ammo, bags);
        let Some(slot) = first.filter(|_| rounds > 0) else {
            continue;
        };
        let rounds = rounds.min(u32::from(u16::MAX));
        let (at, most) = stack.get_or_insert_with(|| {
            items.push(MatchItem {
                instance_id: next_id.wrapping_add(1),
                dict_id: arms.ammo,
                ..MatchItem::default()
            });
            (items.len() as u16 - 1, 0)
        });
        *most = (*most).max(rounds);
        kit.slots[slot] = Some((*at, rounds as u16));
    }
    if let Some((at, most)) = stack {
        items[usize::from(at)].condition_or_stack = most.saturating_mul(STORE_LIVES);
    }
    profile.drop_unheld();
}

/// The game data has Racket's AK-12 and its one ammo, which every ammo slot takes; srp refuses to
/// start otherwise.
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    Arms::of(Dictionary::shared())
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
        assert_eq!(
            Arms::of(dictionary),
            Ok(Arms { gun: id("ak_12"), ammo: id("ammo_6.5x39") })
        );
    }

    #[test]
    fn a_sniper_kit_gets_an_ak_12_and_its_bags_of_ammo() {
        let dictionary = Dictionary::shared();
        let mut sniper = profile(&[&[
            (2, "ren_torso_6", 1),
            (7, "svd", 1),
            (8, "ammo_7.62x54_7n1", 40),
            (10, "val", 1),
            (11, "ammo_sp6", 60),
            (13, "rgd-5", 2),
        ]]);
        let before = sniper.clone();
        arm(&mut sniper, dictionary);
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
        // Both guns' ammo is one stack of AK-12 ammo in the first slot it left, the bags of
        // both: whole clips of the AK-12's.
        let bags = bags_of(id("ammo_7.62x54_7n1"), 40) + bags_of(id("ammo_sp6"), 60);
        let rounds = rounds_in(id("ammo_6.5x39"), bags);
        assert!(rounds > 0);
        assert_eq!(named(&sniper, 0, 8), Some(("ammo_6.5x39", rounds as u16)));
        assert_eq!(named(&sniper, 0, 11), None);
        // The rest of the kit as it was.
        for slot in [2, 13] {
            assert_eq!(named(&sniper, 0, slot), named(&before, 0, slot));
        }
        let (at, _) = sniper.kits[0].slots[8].unwrap();
        let new = &sniper.items[usize::from(at)];
        assert_eq!(new.instance_id, before.items.iter().map(|i| i.instance_id).max().unwrap() + 1);
        assert_eq!(new.condition_or_stack, rounds * STORE_LIVES);
        // The ammo that left is out of the table: torso, two AK-12s' entries, grenades, the stack.
        assert_eq!(sniper.items.len(), before.items.len() - 1);
    }

    #[test]
    fn a_loud_kit_and_a_loud_guns_ammo_stay() {
        let dictionary = Dictionary::shared();
        // An AK-74U kit: nothing to do.
        let mut loud = profile(&[&[(7, "ak_74u", 1), (8, "ammo_5.45x39_7n10", 60)]]);
        let before = loud.clone();
        arm(&mut loud, dictionary);
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
        arm(&mut mixed, dictionary);
        assert_eq!(named(&mixed, 0, 10), named(&before, 0, 10));
        assert_eq!(named(&mixed, 0, 11), named(&before, 0, 11));
        assert_eq!(named(&mixed, 0, 8).map(|(name, _)| name), Some("ammo_6.5x39"));
        // The other kit untouched.
        for slot in 0..mixed.kits[1].slots.len() {
            assert_eq!(named(&mixed, 1, slot), named(&before, 1, slot));
        }
    }

    #[test]
    fn a_gun_two_kits_share_is_replaced_in_both() {
        let dictionary = Dictionary::shared();
        let mut shared = profile(&[&[(7, "vss", 1), (8, "ammo_sp5", 20)]]);
        // A second kit holding the same table entries, as an account's body sends one.
        let second = MatchKit::new(2, 2, String::new(), shared.kits[0].slots, [0.0; MODIFIERS]);
        shared.kits.push(second);
        arm(&mut shared, dictionary);
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
