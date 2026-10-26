use super::super::bot_kit;
use super::*;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

fn level(dict_id: u16) -> u8 {
    item_params::item_params(dict_id)
        .unwrap()
        .match_making_level
}

/// The first ammo of the dictionary that fits `weapon`.
fn ammo_of(weapon: u16) -> u16 {
    let dictionary = Dictionary::shared();
    dictionary
        .items
        .iter()
        .find(|item| {
            item_params::item_params(item.dict_id).is_some_and(|p| p.kind == "ammo")
                && dictionary.compatible(weapon, item.dict_id)
        })
        .unwrap_or_else(|| panic!("no ammo for #{weapon}"))
        .dict_id
}

/// Every weapon of a role's category with its role.
fn role_weapons() -> Vec<(BotRole, u16)> {
    Dictionary::shared()
        .items
        .iter()
        .filter_map(|item| Some((BotRole::of_weapon(item.dict_id)?, item.dict_id)))
        .collect()
}

#[test]
fn a_role_is_its_weapons_category() {
    let role = |name: &str| BotRole::of_weapon(dict(name));
    // The SMGs, the PPSh-41 of the kind of an assault rifle among them.
    assert_eq!(role("uzi"), Some(BotRole::Flanker));
    assert_eq!(role("ppsh"), Some(BotRole::Flanker));
    // The assault rifles, the AK-74U of the kind of an SMG among them, and
    // the machine guns.
    assert_eq!(role("ak_74u"), Some(BotRole::Support));
    assert_eq!(role("val"), Some(BotRole::Support));
    assert_eq!(role("rpk"), Some(BotRole::Support));
    assert_eq!(role("toz_34"), Some(BotRole::Rusher));
    assert_eq!(role("rem_870"), Some(BotRole::Rusher));
    // The sniper rifles, the SVD of the kind of an assault rifle among
    // them.
    assert_eq!(role("svd"), Some(BotRole::Camper));
    assert_eq!(role("mosin_nagant"), Some(BotRole::Camper));
    // No pistol, nothing that is no weapon.
    assert_eq!(role("tt_33"), None);
    assert_eq!(role("medkit"), None);
    let category = |name: &str| Dictionary::shared().item_named(name).unwrap().category;
    assert_eq!(category("tt_33"), category::PISTOLS);
    assert_eq!(category("rpd"), category::MACHINE_GUNS);
    // Each fixed kit's weapon is of its role.
    for each in BotRole::ALL {
        assert_eq!(
            BotRole::of_weapon(bot_kit(each).slot(7).unwrap().dict_id),
            Some(each)
        );
        assert_eq!(bot_kit(each).slot(8).unwrap().count, each.rounds_a_slot());
    }
    // Every role has weapons from level 0 or 1 to 15 in the data... and
    // every weapon of a role has the facts the match server fires it by.
    for (role, weapon) in role_weapons() {
        let facts = weapon_facts(weapon).unwrap_or_else(|| panic!("{role} #{weapon}"));
        assert_eq!(
            facts.magazine,
            item_params::item_params(weapon).unwrap().magazine_capacity,
            "#{weapon}"
        );
    }
}

/// Every weapon of a role with an ammo for it, bare and in armour that
/// opens pockets, makes a bot kit that passes the rules: its ammo fits it,
/// at least a magazine and the category's minimum, at most the role's, and
/// the kit is of the level of the weapon, its ammo and its armour.
#[test]
fn every_role_weapon_makes_a_kit_that_passes_the_rules() {
    let dictionary = Dictionary::shared();
    let armours: [&[(u8, u16)]; 3] = [
        &[],
        &[(3, dict("bm_back_1")), (4, dict("bm_legs_5"))],
        &[
            (0, dict("edge_helmet_3")),
            (2, dict("edge_torso_3")),
            (4, dict("edge_legs_3")),
        ],
    ];
    for (role, weapon) in role_weapons() {
        let ammo = ammo_of(weapon);
        for armour in armours {
            let kit = bot_kit_of(armour, weapon, ammo, role)
                .unwrap_or_else(|why| panic!("{role} #{weapon} in {armour:?}: {why:?}"));
            assert_eq!(kit_problems(&kit, bot_carry_limit()), Vec::<String>::new());
            assert_eq!(kit.slot(7).unwrap().dict_id, weapon);
            let rounds: u32 = BOT_AMMO_SLOTS
                .iter()
                .map(|&slot| {
                    let item = kit.slot(slot).unwrap();
                    assert_eq!(item.dict_id, ammo);
                    assert!(dictionary.compatible(weapon, item.dict_id));
                    assert!(item.count <= role.rounds_a_slot() && item.count.is_multiple_of(10));
                    item.count
                })
                .sum();
            let magazine = item_params::item_params(weapon).unwrap().magazine_capacity;
            assert!(rounds >= magazine, "#{weapon}: {rounds}");
            let wanted = armour
                .iter()
                .map(|&(_, piece)| level(piece))
                .chain([level(weapon), level(ammo)])
                .max()
                .unwrap();
            assert_eq!(kit.gear_level(), wanted, "#{weapon} in {armour:?}");
        }
    }
}

/// The pockets: as many as the armour opens, of what the kit's level lets
/// in (a medkit from level 3, bandages from 1, a grenade from 3, the radio
/// from 0), the medkit first (quick use 1) and a stack at most a pocket's
/// units.
#[test]
fn the_pockets_hold_what_the_kits_level_lets_in() {
    let name = |kit: &Loadout, slot: u8| {
        kit.slot(slot).map(|item| {
            (
                Dictionary::shared()
                    .item(item.dict_id)
                    .unwrap()
                    .name
                    .as_str(),
                item.count,
            )
        })
    };
    // The Mosin bare: level 0, one pocket of one unit, the radio.
    let mosin = dict("mosin_nagant");
    let bare = bot_kit_of(&[], mosin, dict("ammo_7.62x54r_old"), BotRole::Camper).unwrap();
    assert_eq!(bare.gear_level(), 0);
    assert_eq!(pockets(&bare), (1, 1));
    assert_eq!(name(&bare, 13), Some(("respawn_booster", 1)));
    // Scavengers' level 1 torso: bandages in the one pocket.
    let low = bot_kit_of(
        &[(2, dict("scav_torso_1"))],
        mosin,
        dict("ammo_7.62x54r_old"),
        BotRole::Camper,
    )
    .unwrap();
    assert_eq!(low.gear_level(), 1);
    assert_eq!(name(&low, 13), Some(("bandages", 1)));
    // The fixed kits' pocket gear: three pockets of two, a medkit, the
    // grenades and the radio.
    let ak = dict("ak_74u");
    let kit = bot_kit_of(
        &[(3, dict("bm_back_1")), (4, dict("bm_legs_5"))],
        ak,
        dict("ammo_5.45x39_7n6m"),
        BotRole::Support,
    )
    .unwrap();
    assert_eq!(pockets(&kit), (3, 2));
    assert_eq!(name(&kit, 13), Some(("medkit", 2)));
    assert_eq!(name(&kit, 14), Some(("rgd-5", 2)));
    assert_eq!(name(&kit, 15), Some(("respawn_booster", 1)));
    assert_eq!(kit.slot(8).unwrap().count, 200);
}

/// Heavy armour: the rounds are what the carry limit leaves, to the ten;
/// none of that much: no kit.
#[test]
fn the_carry_limit_trims_the_rounds() {
    let svd = dict("svd");
    let ak = dict("ak_12");
    let rpd = dict("rpd");
    let heavy = [
        (2, dict("ren_torso_6")),
        (4, dict("ren_legs_6")),
        (0, dict("ren_helmet_6")),
        (5, dict("ren_gloves_6")),
        (6, dict("ren_boots_6")),
        (3, dict("ren_back_5")),
    ];
    let (mut trimmed, mut refused) = (0, 0);
    for (weapon, role) in [
        (svd, BotRole::Camper),
        (ak, BotRole::Support),
        (rpd, BotRole::Support),
    ] {
        let ammo = ammo_of(weapon);
        // More and more of the heavy pieces.
        for worn in 1..=heavy.len() {
            match bot_kit_of(&heavy[..worn], weapon, ammo, role) {
                Ok(kit) => {
                    assert!(kit.weight() <= bot_carry_limit(), "{}", kit.weight());
                    let rounds = kit.slot(8).unwrap().count;
                    // Ten more a slot would be over the limit, or over
                    // the role's.
                    let more = Loadout {
                        items: kit
                            .items
                            .iter()
                            .map(|item| SlotItem {
                                count: if BOT_AMMO_SLOTS.contains(&item.slot) {
                                    item.count + 10
                                } else {
                                    item.count
                                },
                                ..*item
                            })
                            .collect(),
                    };
                    if rounds < role.rounds_a_slot() {
                        trimmed += 1;
                        assert!(
                            more.weight() > bot_carry_limit() - 1e-3,
                            "{}",
                            more.weight()
                        );
                    }
                }
                // Too heavy for a magazine and the minimum: no kit.
                Err(why) => {
                    refused += 1;
                    assert!(why.iter().any(|why| why.contains("rounds")), "{why:?}");
                }
            }
        }
    }
    assert!(
        trimmed > 0 && refused > 0,
        "{trimmed} trimmed, {refused} refused"
    );
}

/// A player's kit's parts: weapon 1 of a role, and weapon 2 for another
/// role; the ammo it holds most of for each; its armour, by slot; no part
/// for a pistol, a weapon with no ammo, or a second weapon of the same
/// role.
#[test]
fn a_players_kit_gives_a_part_for_each_role_it_has_a_weapon_of() {
    let item = |slot: u8, name: &str, count: u32| SlotItem {
        slot,
        dict_id: dict(name),
        count,
    };
    let kit = Loadout {
        items: vec![
            item(2, "edge_torso_3", 1),
            item(0, "edge_helmet_3", 1),
            item(7, "ak_74m", 1),
            item(8, "ammo_5.45x39_7n6m", 30),
            item(9, "ammo_5.45x39_7n10", 90),
            item(10, "svd", 1),
            item(11, "ammo_7.62x54_7n1", 20),
            item(13, "medkit", 1),
        ],
    };
    let parts = KitParts::of_kit(&kit);
    assert_eq!(
        parts,
        [
            KitParts {
                role: BotRole::Support,
                weapon: dict("ak_74m"),
                ammo: dict("ammo_5.45x39_7n10"),
                armour: vec![(0, dict("edge_helmet_3")), (2, dict("edge_torso_3"))],
            },
            KitParts {
                role: BotRole::Camper,
                weapon: dict("svd"),
                ammo: dict("ammo_7.62x54_7n1"),
                armour: vec![(0, dict("edge_helmet_3")), (2, dict("edge_torso_3"))],
            },
        ]
    );
    // A pistol as weapon 1: only weapon 2's part. A weapon with no ammo:
    // none.
    let pistol = Loadout {
        items: vec![
            item(7, "tt_33", 1),
            item(8, "ammo_7.62x25", 8),
            item(10, "ppsh", 1),
            item(11, "ammo_7.62x25", 70),
        ],
    };
    let parts = KitParts::of_kit(&pistol);
    assert_eq!(parts.len(), 1);
    assert_eq!(
        (parts[0].role, parts[0].weapon),
        (BotRole::Flanker, dict("ppsh"))
    );
    let dry = Loadout {
        items: vec![item(7, "uzi", 1), item(8, "ammo_7.62x54_7n1", 30)],
    };
    assert!(KitParts::of_kit(&dry).is_empty());
}

/// What the rules find wrong, each.
#[test]
fn the_kit_rules_say_what_is_wrong() {
    let ak = dict("ak_74u");
    let ammo = dict("ammo_5.45x39_7n6m");
    let good = bot_kit_of(&[], ak, ammo, BotRole::Support).unwrap();
    assert!(kit_problems(&good, bot_carry_limit()).is_empty());
    let with = |extra: SlotItem| {
        let mut kit = good.clone();
        kit.items.retain(|item| item.slot != extra.slot);
        kit.items.push(extra);
        kit_problems(&kit, bot_carry_limit())
    };
    // A weapon 2, a pistol as weapon 1, ammo of another weapon, a closed
    // pocket, too few rounds.
    let item = |slot: u8, name: &str, count: u32| SlotItem {
        slot,
        dict_id: dict(name),
        count,
    };
    assert_eq!(with(item(10, "svd", 1)), ["a weapon 2"]);
    assert!(with(item(7, "tt_33", 1))[0].contains("no role's category"));
    assert!(with(item(9, "ammo_7.62x54_7n1", 20))[0].contains("does not fit"));
    assert!(with(item(16, "bandages", 1))[0].contains("closed pocket"));
    let mut few = good.clone();
    for item in &mut few.items {
        if BOT_AMMO_SLOTS.contains(&item.slot) {
            item.count = 5;
        }
    }
    assert!(kit_problems(&few, bot_carry_limit())[0].contains("15 rounds"));
    // Over the limit.
    assert!(kit_problems(&good, 1.0)[0].contains("over 1 kg"));
    // The carry limit: a player's and skill 209's five ranks.
    assert_eq!(
        bot_carry_limit(),
        item_params::player().max_carried_weight + 10.0
    );
    assert_eq!(
        super::super::super::skills::MODIFIER_NAMES[MAX_CARRIED_WEIGHT_MODIFIER],
        "max_carried_weight"
    );
    assert_eq!(
        bot_modifiers()
            .iter()
            .filter(|&&value| value != 0.0)
            .count(),
        1
    );
}
