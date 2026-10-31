//! The players' files against the dump of 0.26g's `resources.db`
//! (`tools/re-kit/v026g/data/items/gameplay_items_modifications_*.txt`,
//! `..._craft_craft.options.txt`, `..._reroll_reroll.options.txt` and the
//! item configs beside them).

use super::*;
use crate::v026g::dictionary::Dictionary;
use crate::v026g::scopes::SCOPE_SLOT;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

#[test]
fn the_three_files_parse_to_what_the_dump_shows() {
    let data = Modifications::shared();
    assert_eq!(data.modifications.len(), 158);
    assert_eq!(data.defects.len(), 20);
    assert_eq!(data.defect_slots.len(), 3);
    // Sorted by id, each once.
    assert!(
        data.modifications
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
    );
    assert!(data.defects.windows(2).all(|pair| pair[0].id < pair[1].id));

    // 2000: the first strength of the barrel's quality, 1 % more damage.
    let barrel = data.modification(2000).unwrap();
    assert_eq!(barrel.kind, 1);
    assert_eq!(barrel.drop_weight, 10.0);
    assert_eq!(barrel.name, "st_modify_barrel_quality");
    assert_eq!(
        barrel.modifiers,
        [Modifier {
            path: "parameters/bullet_damage".into(),
            value: 0.01,
            kind: 2
        }]
    );
    assert_eq!(barrel.lobby_info, [("bullet_damage".to_owned(), 0.01)]);
    // The five strengths of a type weigh 10, 8, 6.4, 5.12 and 4.096.
    let weights: Vec<f32> = (2000..=2004)
        .map(|id| data.modification(id).unwrap().drop_weight)
        .collect();
    assert_eq!(weights, [10.0, 8.0, 6.4, 5.12, 4.096]);
    // Armour: 76 changes (19 body parts, 4 kinds of hit), each adding 0.02.
    let armour = data.modification(1000).unwrap();
    assert_eq!(armour.kind, 13);
    assert_eq!(armour.modifiers.len(), 76);
    assert!(
        armour
            .modifiers
            .iter()
            .all(|m| m.kind == 1 && m.value == 0.02)
    );
    assert_eq!(
        armour.modifiers[0].path,
        "hit_params/body/hit_types/ranged/armor"
    );
    // The last type, 40: the Fringe Settlers' reputation.
    let edge = data.modification(1280).unwrap();
    assert_eq!((edge.kind, edge.drop_weight), (40, 0.0));
    // 40 types, 1..=40.
    let mut kinds: Vec<u8> = data.modifications.iter().map(|m| m.kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(kinds, (1..=40).collect::<Vec<u8>>());

    let defect = data.defect(3000).unwrap();
    assert_eq!(defect.name, "st_defect_barrel_deformation");
    assert_eq!(
        (
            defect.kind,
            defect.fix_cost_general,
            defect.fix_cost_gold,
            defect.fix_time_general
        ),
        (1, 100, 50, 360)
    );
    assert_eq!(defect.required_modifications, None);
    assert_eq!(defect.modifiers[0].value, -0.1);
    // The three defects that take away what a modification gave.
    let required: Vec<(u32, Option<(u32, u32)>)> = data
        .defects
        .iter()
        .filter(|defect| defect.required_modifications.is_some())
        .map(|defect| (defect.id, defect.required_modifications))
        .collect();
    assert_eq!(
        required,
        [
            (4030, Some((1030, 1039))),
            (4040, Some((1040, 1049))),
            (4050, Some((1050, 1059)))
        ]
    );
    let mut kinds: Vec<u8> = data.defects.iter().map(|d| d.kind).collect();
    kinds.sort_unstable();
    assert_eq!(kinds, (1..=20).collect::<Vec<u8>>());
    // Every defect costs and takes the same.
    assert!(data.defects.iter().all(|d| {
        (
            d.fix_cost_general,
            d.fix_cost_gold,
            d.fix_time_general,
            d.drop_weight,
        ) == (100, 50, 360, 10.0)
    }));

    assert_eq!(
        data.defect_slots,
        [
            DefectSlot {
                condition: (0.9, 0.0),
                probability: (0.1, 0.5)
            },
            DefectSlot {
                condition: (0.5, 0.0),
                probability: (0.3, 0.7)
            },
            DefectSlot {
                condition: (0.1, 0.0),
                probability: (0.5, 1.0)
            },
        ]
    );

    #[rustfmt::skip]
    assert_eq!(
        data.craft.price_general,
        [400, 400, 800, 1600, 2400, 3400, 4400, 5400, 6600, 8000, 10000, 12000, 14000, 16000, 18000, 20000]
    );
    assert_eq!(
        data.craft.price_premium,
        [2, 2, 4, 8, 12, 17, 22, 27, 33, 40, 50, 60, 70, 80, 90, 100]
    );
    assert_eq!(
        data.craft.chances_general,
        [(50.0, 0.0), (25.0, 25.0), (12.5, 67.5)]
    );
    assert_eq!(
        data.craft.chances_premium,
        [(100.0, 0.0), (50.0, 0.0), (25.0, 25.0)]
    );
    #[rustfmt::skip]
    assert_eq!(
        data.reroll.price_general,
        [200, 200, 400, 400, 600, 800, 1000, 1200, 1600, 2000, 2400, 2800, 3200, 3800, 4400, 5000]
    );
    assert_eq!(
        data.reroll.price_premium,
        [1, 1, 2, 2, 3, 4, 5, 6, 8, 10, 12, 14, 16, 19, 22, 25]
    );
}

#[test]
fn the_client_is_asked_only_for_rows_it_has() {
    let data = Modifications::shared();
    assert!(data.is_known(2000, false));
    assert!(data.is_known(3000, true));
    // A defect's id is no modification, and the other way round.
    assert!(!data.is_known(3000, false));
    assert!(!data.is_known(2000, true));
    assert!(!data.is_known(0, false));
    assert!(!data.is_known(0x0A0B_0C0D, true));
    // Every default modification of every item is a row.
    for item in crate::v026g::item_params::all() {
        for &id in item.default_modifications {
            assert!(data.is_known(id, false), "{} has {id}", item.dict_id);
        }
    }
}

#[test]
fn the_item_masks_are_the_configs() {
    // ak_74m: "1111101111110..." and "11111010...": character j is type
    // j + 1.
    let rifle = item_mods(dict("ak_74m")).unwrap();
    assert_eq!(rifle.modifications, 0b1_1111_1011_1110);
    assert_eq!(rifle.defects, 0b1011_1110);
    assert!(!rifle.camo && !rifle.decal);
    // A bolt-action rifle chambers a round (6) and has no rate of fire
    // (4); an automatic one the other way round.
    let bolt = item_mods(dict("mosin_nagant")).unwrap();
    assert_eq!(bolt.modifications & 0b101_0000, 0b100_0000);
    assert_eq!(rifle.modifications & 0b101_0000, 0b001_0000);
    // A vest: armour and the like, and it takes a camouflage and decals.
    let vest = item_mods(dict("edge_torso_2")).unwrap();
    assert_eq!(vest.modifications, 0x1_31E7_E000);
    assert!(vest.camo && vest.decal);
    // Nothing for ammo or a camouflage itself.
    assert_eq!(item_mods(dict("ammo_7.62x25")), None);
    assert_eq!(item_mods(dict("camo_city")), None);
    // 47 items take cosmetics: the vests and the trousers.
    assert_eq!(all_item_mods().iter().filter(|item| item.camo).count(), 47);
    assert!(all_item_mods().iter().all(|item| item.camo == item.decal));
    assert!(
        all_item_mods()
            .windows(2)
            .all(|pair| pair[0].dict_id < pair[1].dict_id)
    );
    // No item names a type that has no modification or defect.
    for item in all_item_mods() {
        assert_eq!(item.modifications >> 41, 0);
        assert_eq!(item.defects >> 21, 0);
        assert_eq!(item.modifications & 1, 0);
    }
}

#[test]
fn cosmetics_go_into_their_slots() {
    let vest = dict("edge_torso_2");
    let camo = dict("camo_city");
    let decal = Dictionary::shared()
        .items
        .iter()
        .find(|item| item.category == 26)
        .unwrap()
        .dict_id;
    assert_eq!(upgrade_slots(vest, camo), [CAMO_SLOT]);
    assert_eq!(upgrade_slots(vest, decal), DECAL_SLOTS);
    // Not on a rifle, and a rifle is no upgrade.
    assert!(upgrade_slots(dict("ak_74m"), camo).is_empty());
    assert!(upgrade_slots(vest, dict("ak_74m")).is_empty());
    assert!(upgrade_slots(vest, 9999).is_empty());
}

#[test]
fn a_weapon_wears_its_camouflage_beside_its_scope() {
    for kind in [
        "wpn_aslt",
        "wpn_smg",
        "wpn_sniper",
        "wpn_shotgun",
        "wpn_pistol",
    ] {
        assert_eq!(camo_slot(kind), WEAPON_CAMO_SLOT);
    }
    for kind in ["arm_tors", "arm_legs", "arm_head", ""] {
        assert_eq!(camo_slot(kind), CAMO_SLOT);
    }
    assert_ne!(WEAPON_CAMO_SLOT, SCOPE_SLOT);
    assert!(!DECAL_SLOTS.contains(&WEAPON_CAMO_SLOT));
}

#[test]
fn a_weapon_wears_two_decals_beside_its_camouflage_and_scope() {
    // A weapon config with `camo_available` and `decal_available` (the data
    // patches of weapon-camo-1 and weapon-decals-1).
    let rifle = ItemMods {
        dict_id: 1,
        modifications: 0,
        defects: 0,
        camo: true,
        decal: true,
    };
    assert_eq!(cosmetic_slots(&rifle, "wpn_aslt", "drp_decl"), [0, 1]);
    assert_eq!(cosmetic_slots(&rifle, "wpn_pistol", "prm_decl"), [0, 1]);
    assert_eq!(
        cosmetic_slots(&rifle, "wpn_aslt", "drp_camo"),
        [WEAPON_CAMO_SLOT]
    );
    // The window's four slots are apart, logical slots of the weapon kit,
    // and go to the client in at most three records (super::weapon_kit).
    let mut slots = vec![SCOPE_SLOT, WEAPON_CAMO_SLOT];
    slots.extend(WEAPON_DECAL_SLOTS);
    slots.sort_unstable();
    assert_eq!(slots, [0, 1, 2, 3]);
    assert!(
        slots
            .iter()
            .all(|slot| crate::v026g::weapon_kit::WEAPON_SLOTS.contains(slot))
    );
    // Armour as before; nothing without the config's flags.
    let vest = ItemMods {
        dict_id: 2,
        ..rifle
    };
    assert_eq!(cosmetic_slots(&vest, "arm_tors", "drp_decl"), DECAL_SLOTS);
    assert_eq!(cosmetic_slots(&vest, "arm_tors", "drp_camo"), [CAMO_SLOT]);
    let plain = ItemMods {
        camo: false,
        decal: false,
        ..rifle
    };
    assert!(cosmetic_slots(&plain, "wpn_aslt", "drp_decl").is_empty());
    assert!(cosmetic_slots(&plain, "wpn_aslt", "drp_camo").is_empty());
    assert!(cosmetic_slots(&rifle, "wpn_aslt", "wpn_aslt").is_empty());
    assert!(is_weapon_kind("wpn_smg") && !is_weapon_kind("arm_tors") && !is_weapon_kind(""));
}

#[test]
fn a_scope_goes_on_a_weapon_it_has_a_variant_for() {
    use crate::v026g::scopes::fixtures::{self, AK_74N_PSO_1, COBRA, LONE_SCOPE, PSO_1, SVD_COBRA};
    let fits = fixtures::fits();
    let (ak, ak_premium, svd) = (dict("ak_74n"), dict("ak_74n_premium"), dict("svd"));
    let (vest, camo) = (dict("edge_torso_2"), dict("camo_city"));
    for weapon in [ak, ak_premium] {
        assert_eq!(upgrade_slots_in(fits, weapon, PSO_1), [SCOPE_SLOT]);
        assert_eq!(record_dict_in(fits, weapon, PSO_1), AK_74N_PSO_1);
    }
    assert_eq!(upgrade_slots_in(fits, svd, COBRA), [SCOPE_SLOT]);
    assert_eq!(record_dict_in(fits, svd, COBRA), SVD_COBRA);
    // No variant for the pair, a scope no variant names, a variant or
    // 0.26g's own PSO-1 as the item, a vest as the host: nothing.
    for (host, upgrade) in [
        (svd, PSO_1),
        (dict("ak_74m"), PSO_1),
        (ak, LONE_SCOPE),
        (ak, AK_74N_PSO_1),
        (ak, 304),
        (vest, PSO_1),
    ] {
        assert!(
            upgrade_slots_in(fits, host, upgrade).is_empty(),
            "{host} {upgrade}"
        );
    }
    // Cosmetics as before; their record is their own id.
    assert_eq!(upgrade_slots_in(fits, vest, camo), [CAMO_SLOT]);
    assert_eq!(record_dict_in(fits, vest, camo), camo);
    assert_eq!(record_slots_in(fits, vest, camo), [CAMO_SLOT]);
    // A record holds the variant made for its weapon, nothing else.
    assert_eq!(record_slots_in(fits, ak, AK_74N_PSO_1), [SCOPE_SLOT]);
    assert!(record_slots_in(fits, svd, AK_74N_PSO_1).is_empty());
    assert!(record_slots_in(fits, ak, PSO_1).is_empty());
    assert!(record_slots_in(fits, vest, SVD_COBRA).is_empty());
    // The players' data before the patch: no scope goes on anything.
    assert!(upgrade_slots(svd, 304).is_empty());
    assert!(upgrade_slots(dict("rem_700"), 69).is_empty());
}

#[test]
fn a_roll_gives_what_the_item_takes() {
    let data = Modifications::shared();
    let rifle = dict("ak_74m");
    let mut rng = Rng::new(7);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..2000 {
        let rolled = data.roll_for_rarity(rifle, 3, &mut rng);
        assert_eq!(rolled.len(), 3);
        let kinds: Vec<u8> = rolled
            .iter()
            .map(|&id| data.modification(id).unwrap().kind)
            .collect();
        // Three types, each the rifle's.
        assert!(kinds[0] != kinds[1] && kinds[1] != kinds[2] && kinds[0] != kinds[2]);
        for (index, &id) in rolled.iter().enumerate() {
            let mut others = rolled.clone();
            others.remove(index);
            assert!(data.fits(rifle, &others, id));
        }
        seen.extend(kinds);
    }
    // Types 1..=5, 7..=9 and 12: 11 has no drop weight, 6 is a bolt
    // action's, 10 (reliability) never rolls (2026-10-23).
    assert_eq!(
        seen.into_iter().collect::<Vec<u8>>(),
        [1, 2, 3, 4, 5, 7, 8, 9, 12]
    );
    assert!(data.roll_for_rarity(rifle, 0, &mut rng).is_empty());
    // An item that takes nothing gets nothing.
    assert!(
        data.roll_for_rarity(dict("camo_city"), 3, &mut rng)
            .is_empty()
    );
    // A full item takes no more.
    assert_eq!(data.roll(rifle, &[2000, 2010, 2020], &mut rng), None);
    // A type held is not rolled again: the trousers' ammo bags are a
    // default.
    let trousers = dict("bm_legs_4");
    assert!(!data.fits(trousers, &[], 1051));
    assert!(!data.fits(rifle, &[2001], 2000));
    assert!(data.fits(rifle, &[2001], 2010));
    assert!(!data.fits(rifle, &[], 1000));
    // The stronger a modification, the rarer.
    let mut counts = [0_u32; 5];
    for _ in 0..20_000 {
        let id = data.roll(rifle, &[], &mut rng).unwrap();
        counts[(id % 10) as usize] += 1;
    }
    assert!(
        counts.windows(2).all(|pair| pair[0] > pair[1]),
        "{counts:?}"
    );
}

#[test]
fn a_reroll_offers_three_other_modifications() {
    let data = Modifications::shared();
    let rifle = dict("ak_74m");
    let mut rng = Rng::new(3);
    let mut same_type = false;
    for _ in 0..500 {
        let choices = data
            .reroll_choices(rifle, &[2000, 2010], 2000, &mut rng)
            .unwrap();
        let kinds: Vec<u8> = choices
            .iter()
            .map(|&id| data.modification(id).unwrap().kind)
            .collect();
        assert!(kinds[0] != kinds[1] && kinds[1] != kinds[2] && kinds[0] != kinds[2]);
        for &choice in &choices {
            // Not what it replaces, not the type the item keeps, and it
            // fits in its place.
            assert_ne!(choice, 2000);
            assert!(data.fits(rifle, &[2010], choice));
        }
        same_type |= kinds.contains(&1);
    }
    // Another strength of the replaced type may be offered.
    assert!(same_type);
    assert_eq!(
        data.reroll_choices(dict("camo_city"), &[], 2000, &mut rng),
        None
    );
}

#[test]
fn a_reroll_offers_three_on_an_item_of_few_types() {
    // A backpack takes three types that roll in the data (18, 24, 32; as
    // Skhid's Marauder backpack and Bandit balaclava take three), and 32 is
    // reliability, which never rolls (the owner's rule of 2026-10-23): with
    // one of its own, the replaced one's type and the other are left, two
    // types, so another strength fills the offer; with two, only the
    // replaced one's type.
    let data = Modifications::shared();
    let backpack = dict("scav_back_1");
    let mut rng = Rng::new(5);
    let kind = |id: u32| data.modification(id).unwrap().kind;
    let mut kinds_seen = std::collections::BTreeSet::new();
    for _ in 0..300 {
        let choices = data
            .reroll_choices(backpack, &[1062], 1062, &mut rng)
            .expect("an offer");
        assert!(choices[0] != choices[1] && choices[1] != choices[2] && choices[0] != choices[2]);
        for &choice in &choices {
            assert_ne!(choice, 1062);
            assert_ne!(kind(choice), kind(1201), "reliability");
            assert!(data.fits(backpack, &[], choice));
            kinds_seen.insert(kind(choice));
        }
        // Both types left are offered before a second strength of one.
        let kinds: std::collections::BTreeSet<u8> = choices.iter().map(|&id| kind(id)).collect();
        assert_eq!(kinds.len(), 2);
    }
    assert_eq!(kinds_seen, [kind(1062), kind(1120)].into());
    for held in [&[1062, 1120][..], &[1062, 1120, 1201]] {
        let choices = data
            .reroll_choices(backpack, held, 1062, &mut rng)
            .expect("an offer");
        assert!(
            choices
                .iter()
                .all(|&id| kind(id) == kind(1062) && id != 1062)
        );
    }
}

#[test]
fn defects_come_with_wear() {
    let data = Modifications::shared();
    // No defect at all over 90 %.
    assert_eq!(data.defect_chance(1.0, 0), 0.0);
    assert_eq!(data.defect_chance(0.9, 0), 0.0);
    // The first slot: 10 % just under 90 %, 50 % at nothing.
    assert!((data.defect_chance(0.8999, 0) - 0.1).abs() < 1e-3);
    assert!((data.defect_chance(0.45, 0) - 0.3).abs() < 1e-5);
    assert!((data.defect_chance(0.0, 0) - 0.5).abs() < 1e-6);
    // The second opens under 50 %, the third under 10 %; there is no
    // fourth.
    assert_eq!(data.defect_chance(0.6, 1), 0.0);
    assert!((data.defect_chance(0.25, 1) - 0.5).abs() < 1e-5);
    assert_eq!(data.defect_chance(0.2, 2), 0.0);
    assert!((data.defect_chance(0.0, 2) - 1.0).abs() < 1e-6);
    assert_eq!(data.defect_chance(0.0, 3), 0.0);

    let rifle = dict("ak_74m");
    let ids = |list: Vec<&Defect>| list.iter().map(|d| d.id).collect::<Vec<u32>>();
    // The rifle's mask: types 1..=5 and 7.
    assert_eq!(
        ids(data.defect_candidates(rifle, &[], &[])),
        [3000, 3010, 3020, 3030, 3040, 3060]
    );
    assert_eq!(
        ids(data.defect_candidates(rifle, &[], &[3000, 3060])),
        [3010, 3020, 3030, 3040]
    );
    // Trousers with ammo bags (a default modification 1050) may lose one;
    // those without may not.
    let with_bags = dict("bm_legs_4");
    assert!(ids(data.defect_candidates(with_bags, &[], &[])).contains(&4050));
    let plain = dict("scav_legs_1");
    assert!(item_mods(plain).unwrap().defects & (1 << 12) != 0);
    assert!(!ids(data.defect_candidates(plain, &[], &[])).contains(&4050));

    let mut rng = Rng::new(11);
    // A whole item never breaks; one at nothing does half the time.
    assert!((0..500).all(|_| { data.roll_defect(rifle, &[], &[], 0.95, &mut rng).is_none() }));
    let broke = (0..4000)
        .filter(|_| data.roll_defect(rifle, &[], &[], 0.0, &mut rng).is_some())
        .count();
    assert!((1800..2200).contains(&broke), "{broke}");
    // At nothing the third slot always fills, with a defect it has not.
    let third = data
        .roll_defect(rifle, &[], &[3000, 3010], 0.0, &mut rng)
        .unwrap();
    assert!([3020, 3030, 3040, 3060].contains(&third));
}

#[test]
fn craft_and_reroll_go_by_the_tables() {
    let data = Modifications::shared();
    assert_eq!(data.craft.price(0, false), 400);
    assert_eq!(data.craft.price(10, false), 10_000);
    assert_eq!(data.craft.price(10, true), 50);
    // Level 16 (the highest an item has) costs the table's last.
    assert_eq!(data.craft.price(16, false), 20_000);
    assert_eq!(data.craft.chances(0, false), Some((50.0, 0.0)));
    assert_eq!(data.craft.chances(2, true), Some((25.0, 25.0)));
    assert_eq!(data.craft.chances(3, false), None);
    use CraftOutcome::{Failure, Neutral, Success};
    assert_eq!(data.craft.outcome_at(0, false, 49.9), Some(Success));
    assert_eq!(data.craft.outcome_at(0, false, 50.0), Some(Neutral));
    assert_eq!(data.craft.outcome_at(0, true, 99.9), Some(Success));
    assert_eq!(data.craft.outcome_at(2, false, 12.4), Some(Success));
    assert_eq!(data.craft.outcome_at(2, false, 12.5), Some(Failure));
    assert_eq!(data.craft.outcome_at(2, false, 79.9), Some(Failure));
    assert_eq!(data.craft.outcome_at(2, false, 80.0), Some(Neutral));
    assert_eq!(data.craft.outcome_at(3, false, 0.0), None);
    // The lobby's rows: silver success and failure, gold success and
    // failure, for a difference of 0, 1 and 2.
    let (chances, general, premium) = data.craft.tables();
    #[rustfmt::skip]
    assert_eq!(
        chances,
        [50.0, 0.0, 100.0, 0.0,  25.0, 25.0, 50.0, 0.0,  12.5, 67.5, 25.0, 25.0]
    );
    assert_eq!((general[2], premium[2]), (800, 4));

    assert_eq!(data.reroll.price(0, false), 200);
    assert_eq!(data.reroll.price(10, false), 2400);
    assert_eq!(data.reroll.price(10, true), 12);
    assert_eq!(data.reroll.price(16, true), 25);
    // No level changes for nothing (the owner's rule of 2026-10-25: the
    // same price every time, `docs/notes/flat-reroll.md`).
    for level in 0..=u8::MAX {
        assert!(data.reroll.price(level, false) > 0);
        assert!(data.reroll.price(level, true) > 0);
    }
}

#[test]
fn reliability_slows_wear_and_premium_gear_pays_more() {
    let data = Modifications::shared();
    let rifle = dict("ak_74m");
    assert_eq!(data.wear_factor(rifle, &[], "degrade_per_match"), 1.0);
    // 2090: 5 % less wear; 2094: 25 %.
    let factor = data.wear_factor(rifle, &[(2090, false)], "degrade_per_match");
    assert!((factor - 0.95).abs() < 1e-6);
    let factor = data.wear_factor(rifle, &[(2094, false)], "degrade_per_use");
    assert!((factor - 0.75).abs() < 1e-6);
    // Another modification changes nothing of it, and a defect's id is
    // not read as a modification's.
    assert_eq!(
        data.wear_factor(rifle, &[(2000, false)], "degrade_per_match"),
        1.0
    );
    assert_eq!(
        data.wear_factor(rifle, &[(2090, true)], "degrade_per_match"),
        1.0
    );
    // The golden rifle's 2094 default is gone (`super::super::golden`).
    let premium = dict("ak_74n_premium");
    assert_eq!(data.wear_factor(premium, &[], "degrade_per_death"), 1.0);

    // Its 1230 (10 % more silver) is a modification of its own: it pays
    // as one, and without it nothing.
    let bonuses = data.bonuses(premium, &[(1230, false)]);
    assert!((bonuses.money - 0.1).abs() < 1e-6);
    assert_eq!(bonuses.experience, 0.0);
    assert_eq!(data.bonuses(premium, &[]), Bonuses::default());
    // Premium trousers: experience and the Black Market's reputation, as
    // modifications of their own (`super::super::golden`); without them,
    // nothing.
    let trousers = dict("bm_legs_4_premium");
    assert_eq!(data.bonuses(trousers, &[]), Bonuses::default());
    let bonuses = data.bonuses(trousers, &[(1240, false), (1260, false)]);
    assert!((bonuses.experience - 0.05).abs() < 1e-6);
    assert_eq!(bonuses.money, 0.0);
    assert!((bonuses.reputation[1] - 0.05).abs() < 1e-6);
    assert_eq!(bonuses.reputation[0], 0.0);
    let bonuses = data.bonuses(
        dict("scav_torso_4_premium"),
        &[(1240, false), (1250, false)],
    );
    assert!((bonuses.reputation[0] - 0.05).abs() < 1e-6);
    assert_eq!(data.bonuses(rifle, &[(2000, false)]), Bonuses::default());
}

/// The lobby colours a bonus at the best value it rolls gold and shows its
/// range in the reroll window (`ModificationRolls.ROLLS`,
/// `docs/notes/max-bonus.md`): for each property the values the
/// modifications that roll give it (a drop weight, a type some item takes),
/// weakest first, as `ui_desc/props_list` has them. The movie gets no
/// modification id, only `{prop_id, prop_value}`, so the table is by property;
/// the types that share one (weight, aim time, reliability, recoil, reload,
/// anomaly armour) roll the same values.
#[test]
fn the_lobby_knows_the_values_each_bonus_rolls() {
    use std::collections::BTreeMap;

    let data = Modifications::shared();
    let taken = all_item_mods()
        .iter()
        .fold(0u64, |mask, item| mask | item.modifications);
    let root = ConfigValue::parse(super::super::game_data::file(MODIFICATIONS_PATH)).unwrap();
    // Property -> type -> (drop weight, value).
    let mut by_type: BTreeMap<u32, BTreeMap<u8, Vec<(f32, f64)>>> = BTreeMap::new();
    for row in root.req("modifications").unwrap().children() {
        let id = number(row.req("id").unwrap()).unwrap();
        let modification = data.modification(id).unwrap();
        if modification.drop_weight <= 0.0 || taken & (1 << modification.kind) == 0 {
            continue;
        }
        let props = row
            .req("ui_desc")
            .and_then(|desc| desc.req("props_list"))
            .unwrap()
            .children();
        assert_eq!(props.len(), 1, "modification {id} shows one property");
        let prop_id = u32::try_from(props[0].i32_at("prop_id").unwrap()).unwrap();
        let value = props[0].req("prop_value").unwrap();
        let value = value
            .as_str()
            .and_then(|text| text.parse::<f64>().ok())
            .or_else(|| value.as_f32().map(f64::from))
            .unwrap_or_else(|| panic!("modification {id}: a prop_value"));
        by_type
            .entry(prop_id)
            .or_default()
            .entry(modification.kind)
            .or_default()
            .push((modification.drop_weight, value));
    }
    let mut expected: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    for (prop_id, kinds) in by_type {
        for (kind, mut rows) in kinds {
            // The most common first: the weakest.
            rows.sort_by(|a, b| b.0.total_cmp(&a.0));
            let values: Vec<f64> = rows.into_iter().map(|(_, value)| value).collect();
            assert!(
                values.windows(2).all(|pair| pair[0].abs() < pair[1].abs()),
                "property {prop_id}, type {kind}: the rarer, the stronger: {values:?}"
            );
            match expected.get(&prop_id) {
                Some(other) => assert_eq!(
                    other, &values,
                    "property {prop_id}: type {kind} rolls other values"
                ),
                None => {
                    expected.insert(prop_id, values);
                }
            }
        }
    }
    assert!(expected.len() > 10, "{expected:?}");
    // The owner's example: +5 % damage is the best, +4 % is not.
    assert_eq!(expected[&1], [0.01, 0.02, 0.03, 0.04, 0.05]);

    let source =
        include_str!("../../../../../flash/lobby_menu/src/survarium/item/ModificationRolls.as");
    let start = source.find("ROLLS:Object = {").expect("the lobby's ROLLS");
    let rows = &source[start + "ROLLS:Object = {".len()..];
    let rows = &rows[..rows.find("};").expect("the end of ROLLS")];
    // `<prop id>:[<values>], // <name>` a line.
    let lobby: BTreeMap<u32, Vec<f64>> = rows
        .lines()
        .map(|line| line.split("//").next().unwrap().trim())
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (id, values) = line.split_once(':').expect("<prop id>:[values]");
            let values = values.trim().trim_end_matches(',');
            let values = values.trim_start_matches('[').trim_end_matches(']');
            (
                id.trim().parse().unwrap(),
                values
                    .split(',')
                    .map(|v| v.trim().parse().unwrap())
                    .collect(),
            )
        })
        .collect();
    let same = lobby.len() == expected.len()
        && lobby.iter().zip(&expected).all(|((a, x), (b, y))| {
            a == b && x.len() == y.len() && x.iter().zip(y).all(|(x, y)| (x - y).abs() < 1e-9)
        });
    // An object literal: no comma after the last row.
    let table: Vec<String> = expected
        .iter()
        .map(|(id, values)| {
            let values: Vec<String> = values.iter().map(f64::to_string).collect();
            format!("         {id}:[{}]", values.join(","))
        })
        .collect();
    let table = table.join(",\n");
    assert!(
        same,
        "ModificationRolls.ROLLS should be, by this data:\n{table}"
    );
}

/// The lobby shows the premium bonuses (the golden weapons' silver, the
/// premium armour's experience and reputation) in gold as the most there
/// is (`ModificationRolls.isPremium`, by `ItemFactory.BONUS_UI`'s property
/// and value; docs/notes/max-bonus.md). True while no modification gives
/// that property more, and none rolls it (`ROLLS` would decide then).
#[test]
fn the_premium_bonuses_are_the_most_their_property_gives() {
    use std::collections::BTreeMap;

    let source = include_str!("../../../../../flash/lobby_menu/src/survarium/item/ItemFactory.as");
    let start = source
        .find("BONUS_UI:Array = [")
        .expect("the lobby's BONUS_UI");
    let rows = &source[start..];
    let rows = &rows[rows.find('\n').unwrap() + 1..rows.find("\n      ];").unwrap()];
    // `[<id>,<icon>,<prop id>,"<value>"], // <name>` a line.
    let premium: BTreeMap<u32, f64> = rows
        .lines()
        .map(|line| {
            let row = line.split("//").next().unwrap().trim();
            let row = row.trim_end_matches(',').trim_start_matches('[');
            let fields: Vec<&str> = row.trim_end_matches(']').split(',').collect();
            let [_, _, prop_id, value] = fields[..] else {
                panic!("a BONUS_UI row: {line}")
            };
            (
                prop_id.trim().parse().unwrap(),
                value.trim().trim_matches('"').parse().unwrap(),
            )
        })
        .collect();
    // Silver, experience and the four factions' reputation.
    assert_eq!(premium.len(), 6, "{premium:?}");

    let data = Modifications::shared();
    let taken = all_item_mods()
        .iter()
        .fold(0u64, |mask, item| mask | item.modifications);
    let root = ConfigValue::parse(super::super::game_data::file(MODIFICATIONS_PATH)).unwrap();
    let mut seen = 0;
    for row in root.req("modifications").unwrap().children() {
        let id = number(row.req("id").unwrap()).unwrap();
        let Some(props) = row
            .req("ui_desc")
            .and_then(|desc| desc.req("props_list"))
            .ok()
        else {
            continue;
        };
        for prop in props.children() {
            let prop_id = u32::try_from(prop.i32_at("prop_id").unwrap()).unwrap();
            let Some(&best) = premium.get(&prop_id) else {
                continue;
            };
            seen += 1;
            let value = prop.req("prop_value").unwrap();
            let value = value
                .as_str()
                .and_then(|text| text.parse::<f64>().ok())
                .or_else(|| value.as_f32().map(f64::from))
                .unwrap_or_else(|| panic!("modification {id}: a prop_value"));
            assert!(
                value.abs() <= best.abs() + 1e-9,
                "modification {id} gives property {prop_id} {value}, more than the premium {best}"
            );
            let modification = data.modification(id).unwrap();
            assert!(
                modification.drop_weight <= 0.0 || taken & (1 << modification.kind) == 0,
                "modification {id} rolls property {prop_id}: ModificationRolls.ROLLS decides it"
            );
        }
    }
    assert!(seen >= premium.len(), "{seen}");
}

/// The ids of the "reliability" modifications ("Надежность"): those that
/// change nothing but wear.
fn reliability_ids(data: &Modifications) -> Vec<u32> {
    data.modifications
        .iter()
        .filter(|row| row.only_changes_wear())
        .map(|row| row.id)
        .collect()
}

/// The owner's rule of 2026-10-23 (docs/notes/no-reliability-roll.md):
/// which modifications are reliability, by the data.
#[test]
fn reliability_is_the_ten_rows_that_only_change_wear() {
    let data = Modifications::shared();
    let ids = reliability_ids(data);
    let expected: Vec<u32> = (1200..=1204).chain(2090..=2094).collect();
    assert_eq!(ids, expected);
    for &id in &ids {
        let row = data.modification(id).unwrap();
        assert_eq!(row.name, "st_modify_reliability");
        assert!(row.drop_weight > 0.0 && !row.rolls(), "{id}");
        assert_eq!(row.lobby_info.len(), 1);
        assert_eq!(row.lobby_info[0].0, "reliability");
    }
    // The lobby names a modification by its property: reliability's (123,
    // `st_multiplicative_reliability_modifier`, "Reliability" /
    // "Надійність" / "Надежность") is shown by these ten and no other.
    let root = ConfigValue::parse(super::super::game_data::file(MODIFICATIONS_PATH)).unwrap();
    let mut showing = Vec::new();
    for row in root.req("modifications").unwrap().children() {
        let id = number(row.req("id").unwrap()).unwrap();
        let props = row
            .req("ui_desc")
            .and_then(|desc| desc.req("props_list"))
            .unwrap()
            .children();
        if props
            .iter()
            .any(|prop| prop.i32_at("prop_id").unwrap() == 123)
        {
            showing.push(id);
        }
    }
    showing.sort_unstable();
    assert_eq!(showing, expected);
    // Only these change wear, and every other row with a drop weight rolls.
    for row in &data.modifications {
        let wear = row
            .modifiers
            .iter()
            .any(|modifier| WEAR_PARAMETERS.contains(&modifier.path.as_str()));
        assert_eq!(wear, ids.contains(&row.id), "{}", row.id);
        if !ids.contains(&row.id) {
            assert_eq!(row.rolls(), row.drop_weight > 0.0, "{}", row.id);
        }
    }
}

/// No roll of any item gives reliability: a drop's, an upgrade's (both
/// [`Modifications::roll`]) and a reroll's offer, whatever the item holds,
/// reliability included.
#[test]
fn no_roll_gives_reliability() {
    let data = Modifications::shared();
    let reliability = reliability_ids(data);
    let mut rng = Rng::new(23);
    let (mut weapons, mut gear) = (0, 0);
    for item in all_item_mods() {
        if item.modifications == 0 {
            continue;
        }
        let dict_id = item.dict_id;
        let kind = item_params(dict_id).map_or("", |params| params.kind);
        if is_weapon_kind(kind) {
            weapons += 1;
        } else {
            gear += 1;
        }
        // The reliability this item could hold, as one from before.
        let own: Vec<u32> = reliability
            .iter()
            .copied()
            .filter(|&id| data.fits(dict_id, &[], id))
            .take(1)
            .collect();
        for held in [&[][..], &own] {
            assert!(
                data.candidates(dict_id, held)
                    .iter()
                    .all(|row| !reliability.contains(&row.id)),
                "{dict_id}"
            );
        }
        for _ in 0..20 {
            let rolled = data.roll_for_rarity(dict_id, 3, &mut rng);
            assert!(
                rolled.iter().all(|id| !reliability.contains(id)),
                "{dict_id}"
            );
            let mut held = own.clone();
            if let Some(id) = data.roll(dict_id, &held, &mut rng) {
                assert!(!reliability.contains(&id), "{dict_id}");
                held.push(id);
            }
            for &replaced in &held {
                if let Some(choices) = data.reroll_choices(dict_id, &held, replaced, &mut rng) {
                    for choice in choices {
                        assert!(!reliability.contains(&choice), "{dict_id}");
                        assert_ne!(choice, replaced);
                        let others: Vec<u32> =
                            held.iter().copied().filter(|&id| id != replaced).collect();
                        assert!(data.fits(dict_id, &others, choice), "{dict_id}");
                    }
                }
            }
        }
    }
    assert!(
        weapons >= 50 && gear >= 100,
        "{weapons} weapons, {gear} gear"
    );

    // A rifle's reliability rerolls into another, valid modification.
    let rifle = dict("ak_74m");
    for _ in 0..200 {
        let choices = data
            .reroll_choices(rifle, &[2090, 2000], 2090, &mut rng)
            .expect("an offer");
        for choice in choices {
            assert!(!reliability.contains(&choice));
            assert!(data.fits(rifle, &[2000], choice));
        }
    }
}

/// The stock data, before the edit lists of release/content.tsv: the items
/// for which reliability was one of three types that roll (backpacks, masks
/// and respirators, oxygen packs) have two left, so they reach two
/// modifications of their own by rolls, and none is left to offer for the
/// reliability of a full one. No item that took a roll before has none now.
/// On live, `third-mod-1.json` gives them a third type
/// ([`every_item_rolls_three_types_with_the_edit_lists`]).
#[test]
fn items_of_few_types_roll_two() {
    let data = Modifications::shared();
    let reliability = reliability_ids(data);
    let mut rng = Rng::new(4);
    let mut two = Vec::new();
    for item in all_item_mods() {
        let dict_id = item.dict_id;
        let rolled_before = data
            .modifications
            .iter()
            .any(|row| row.drop_weight > 0.0 && item.modifications & (1 << row.kind) != 0);
        let pool = data.candidates(dict_id, &[]);
        assert_eq!(rolled_before, !pool.is_empty(), "{dict_id}: an empty pool");
        let kinds: std::collections::BTreeSet<u8> = pool.iter().map(|row| row.kind).collect();
        if kinds.len() < MAX_RANK && !kinds.is_empty() {
            assert_eq!(kinds.len(), 2, "{dict_id}");
            two.push(dict_id);
            assert_eq!(data.roll_for_rarity(dict_id, 3, &mut rng).len(), 2);
        }
    }
    let names: Vec<&str> = two
        .iter()
        .map(|&dict_id| item_params(dict_id).unwrap().kind)
        .collect();
    assert_eq!(two.len(), 19, "{names:?}");
    assert!(
        names
            .iter()
            .all(|kind| ["arm_back", "arm_mask", "arm_oxy"].contains(kind)),
        "{names:?}"
    );
    assert!(two.contains(&dict("bandit_026_mask")));

    // A full backpack: its reliability has nothing to turn into, its
    // others another strength of their own type.
    let backpack = dict("scav_back_1");
    let full = [1062, 1120, 1201];
    assert_eq!(data.reroll_choices(backpack, &full, 1201, &mut rng), None);
    assert!(data.candidates(backpack, &[1062, 1120]).is_empty());
    assert_eq!(data.roll(backpack, &[1062, 1120], &mut rng), None);
    let choices = data
        .reroll_choices(backpack, &full, 1120, &mut rng)
        .unwrap();
    for choice in choices {
        assert!(!reliability.contains(&choice));
        assert_eq!(data.modification(choice).unwrap().kind, 24);
    }
}

/// The types a roll may give an item with this `available_modifications_mask`
/// (as bits) that holds nothing: those with a modification that rolls
/// ([`Modification::rolls`]), less the types of its defaults.
fn rollable_types(data: &Modifications, mask: u64, defaults: &[u32]) -> u32 {
    let rolling = data
        .modifications
        .iter()
        .filter(|row| row.rolls())
        .fold(0_u64, |kinds, row| kinds | 1 << row.kind);
    (mask & rolling & !data.kinds_of(defaults.iter().copied())).count_ones()
}

/// The masks the manifest's edit lists set, in its order: by config, the
/// first `from` and the last `to` of each, and the configs of `third-mod-1.json`
/// with their `from`.
#[allow(clippy::type_complexity)]
fn edited_masks() -> (
    std::collections::BTreeMap<String, (String, String)>,
    Vec<(String, String)>,
) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let manifest = std::fs::read_to_string(root.join("release/content.tsv")).unwrap();
    let mut chain = std::collections::BTreeMap::<String, (String, String)>::new();
    let mut third = Vec::new();
    for line in manifest.lines().filter(|line| line.starts_with("edit\t")) {
        let list = line.split('\t').nth(1).unwrap();
        let path = root.join("srp-026g/scripts/content-edit").join(list);
        let text = std::fs::read_to_string(&path).unwrap();
        let edits: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entries = edits.get("edit_text").and_then(|value| value.as_array());
        for edit in entries.into_iter().flatten() {
            if edit["path"] != "parameters/available_modifications_mask" {
                continue;
            }
            let text = |key: &str| edit[key].as_str().unwrap().to_owned();
            let (file, from, to) = (text("file"), text("from"), text("to"));
            match chain.get_mut(&file) {
                Some(link) => {
                    assert_eq!(
                        link.1, from,
                        "{list}: {file} starts where the last edit left it"
                    );
                    link.1 = to;
                }
                None => {
                    chain.insert(file.clone(), (from.clone(), to));
                }
            }
            if list == "third-mod-1.json" {
                third.push((file, from));
            }
        }
    }
    (chain, third)
}

/// The owner's rule of 2026-10-23 (docs/notes/no-reliability-roll.md): with
/// reliability out, every item that rolls has at least three types once the
/// manifest's edit lists are on (`third-mod-1.json`: masks Armor, oxygen
/// packs Weight limit and Energy costs, the Pilgrim container Energy costs),
/// so an upgrade is never refused for nothing to roll and a full item's
/// reliability always has another type to turn into. The stock test archive
/// with the edit lists' masks laid on in order (the live masks are that
/// chain's end); the masks of items the packs add (1812-1816, 1823, not in
/// the archive) are in it as their edit's `from`.
#[test]
fn every_item_rolls_three_types_with_the_edit_lists() {
    let data = Modifications::shared();
    let (chain, third) = edited_masks();
    let bits = |mask: &str| mask_bits(mask, "available_modifications_mask").unwrap();
    let dictionary = Dictionary::shared();
    let defaults_of = |file: &str| {
        dictionary
            .items
            .iter()
            .find(|entry| entry.cfg_name == file)
            .and_then(|entry| item_params(entry.dict_id))
            .map_or(&[][..], |params| params.default_modifications)
    };
    let mut in_archive = std::collections::BTreeSet::new();
    let mut rolling = 0;
    for entry in &dictionary.items {
        let Some(item) = item_mods(entry.dict_id) else {
            continue;
        };
        let mask = match chain.get(&entry.cfg_name) {
            Some((from, to)) => {
                assert_eq!(
                    item.modifications,
                    bits(from),
                    "{}: the chain's start",
                    entry.name
                );
                in_archive.insert(entry.cfg_name.clone());
                bits(to)
            }
            None => item.modifications,
        };
        let types = rollable_types(data, mask, defaults_of(&entry.cfg_name));
        assert!(
            types == 0 || types >= 3,
            "{} rolls {types} types",
            entry.name
        );
        rolling += usize::from(types > 0);
    }
    assert!(rolling > 150, "{rolling}");
    for (file, (_, to)) in chain.iter().filter(|(file, _)| !in_archive.contains(*file)) {
        assert!(file.starts_with("gameplay/items/armour/mask/"), "{file}");
        assert!(rollable_types(data, bits(to), &[]) >= 3, "{file}");
    }

    // third-mod-1.json: the 20 that reliability left with two, each with
    // three or more now.
    let (mut masks, mut oxygen, mut pilgrim) = (0, 0, 0);
    for (file, from) in &third {
        let defaults = defaults_of(file);
        assert_eq!(rollable_types(data, bits(from), defaults), 2, "{file}");
        assert_ne!(bits(from) & (1 << 32), 0, "{file} took reliability");
        let to = bits(&chain[file].1);
        let added = to & !bits(from);
        if file.starts_with("gameplay/items/armour/mask/") {
            masks += 1;
            assert_eq!(added, 1 << 13, "{file}: Armor");
        } else if file.ends_with("/edge_back_6.options") {
            pilgrim += 1;
            assert_eq!(added, 1 << 28, "{file}: Energy costs");
        } else {
            oxygen += 1;
            assert_eq!(
                added,
                1 << 24 | 1 << 28,
                "{file}: Weight limit, Energy costs"
            );
        }
    }
    assert_eq!((masks, oxygen, pilgrim), (15, 4, 1));
}

/// [`every_item_rolls_three_types_with_the_edit_lists`] on a server game
/// data file as `release/build-content` makes it (`SRP_TEST_GAME_DATA`):
/// every item that rolls there has three types or more.
#[test]
#[ignore = "needs SRP_TEST_GAME_DATA: a server-game-data.db made by release/build-content"]
fn every_item_of_the_built_game_data_rolls_three_types() {
    use super::super::game_data::GameData;
    let path = std::env::var_os("SRP_TEST_GAME_DATA").expect("SRP_TEST_GAME_DATA");
    let data = GameData::open(std::path::Path::new(&path)).unwrap();
    let file = |path: &str| data.file(path).unwrap_or_else(|| panic!("no {path}"));
    let dictionary = Dictionary::parse(file(super::super::dictionary::PATH)).unwrap();
    let table = Modifications::parse(
        file(MODIFICATIONS_PATH),
        file(CRAFT_PATH),
        file(REROLL_PATH),
    )
    .unwrap();
    let params = super::super::item_params::derive(&data, &dictionary).unwrap();
    let mut short = Vec::new();
    let mut rolling = 0;
    for item in derive_item_mods(&data, &dictionary).unwrap() {
        let params = params.iter().find(|row| row.dict_id == item.dict_id);
        let defaults = params.map_or(&[][..], |params| params.default_modifications);
        let types = rollable_types(&table, item.modifications, defaults);
        rolling += usize::from(types > 0);
        if types > 0 && types < 3 {
            let name = &dictionary.item(item.dict_id).unwrap().name;
            short.push(format!("{name}: {types}"));
        }
    }
    assert!(rolling > 150, "{rolling}");
    assert!(short.is_empty(), "fewer than three types: {short:?}");
}
