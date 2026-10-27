//! One weapon class from the game data (`FireTuning::weapon_class_by_data`,
//! audit W5): a weapon is of its shop category's class, except an SMG type in
//! the rifle category (the AK-74U and its copy), and a server bot's kind
//! follows the weapon it holds. With the switch off every choice is the
//! category's, as before.

use std::sync::Arc;

use mapdata::Vec3;
use survarium::v026g::weapons::{self, WeaponParams};

use super::super::guns::{self, AutoClass, Gun, Handling, WeaponClass};
use super::super::reflex::{CombatTuning, FireTuning, WeaponKind};
use super::super::trace::Trace;
use super::super::tuning::Tuning;
use super::super::Bots;

/// A weapon row of the given family: the AK-74U's numbers with the type, the
/// category, the fire mode and the bolt changed.
fn row(kind: &'static str, category: u8, auto: bool, bolt: bool) -> WeaponParams {
    WeaponParams {
        kind,
        category,
        first_fire_mode: if auto { -1 } else { 1 },
        has_chamber: bolt,
        chamber_a_round_time: if bolt { 1.0 } else { 0.0 },
        ..*weapons::weapon_named("ak_74u").unwrap()
    }
}

fn on() -> FireTuning {
    FireTuning {
        weapon_class_by_data: true,
        ..FireTuning::default()
    }
}

/// `Gun::class` before the data's class: the category, the sniper category
/// split by its bolt.
fn class_before(gun: &Gun) -> &'static str {
    match (gun.category, gun.handling) {
        (15, _) => "smg",
        (16, _) => "mg",
        (17, _) => "pistol",
        (14, _) => "shotgun",
        (13, Handling::Bolt) => "bolt",
        (13, _) => "marksman",
        _ => "rifle",
    }
}

/// The families of the 83 weapons of the live data (rev30 and rev34):
/// (category, `parameters.type`, automatic, bolt action, the class by category,
/// the class by data, how many weapons). Only the SMG type in the rifle category
/// differs.
const FAMILIES: [(u8, &str, bool, bool, &str, &str, u32); 16] = [
    (8, "wpn_aslt", true, false, "rifle", "rifle", 22),
    (8, "wpn_aslt", false, false, "rifle", "rifle", 5),
    (8, "wpn_smg", true, false, "rifle", "smg", 2), // the AK-74U and its premium copy
    (13, "wpn_aslt", true, false, "marksman", "marksman", 4),
    (13, "wpn_aslt", false, false, "marksman", "marksman", 1),
    (13, "wpn_bolt", false, true, "bolt", "bolt", 8),
    (14, "wpn_asgn", false, false, "shotgun", "shotgun", 7),
    (14, "wpn_dbrl", false, false, "shotgun", "shotgun", 5),
    (14, "wpn_stgn", false, true, "shotgun", "shotgun", 3),
    (15, "wpn_aslt", true, false, "smg", "smg", 4), // mp7 x3, ppsh: the category wins
    (15, "wpn_pstl", true, false, "smg", "smg", 1), // aek_919k: the category wins
    (15, "wpn_smg", true, false, "smg", "smg", 13),
    (16, "wpn_aslt", true, false, "mg", "mg", 3),
    (17, "wpn_apst", true, false, "pistol", "pistol", 1),
    (17, "wpn_pstl", false, false, "pistol", "pistol", 3),
    (17, "wpn_rvlr", false, false, "pistol", "pistol", 1),
];

#[test]
fn the_83_live_weapons_have_the_class_the_data_says_and_the_one_their_category_gave() {
    assert_eq!(FAMILIES.iter().map(|f| f.6).sum::<u32>(), 83);
    for (category, kind, auto, bolt, before, by_data, _) in FAMILIES {
        let gun = Gun::of(&row(kind, category, auto, bolt));
        assert_eq!(gun.class(), before, "{category} {kind}");
        assert_eq!(gun.by_category.name(), before, "{category} {kind}");
        assert_eq!(gun.class.name(), by_data, "{category} {kind}");
        assert_eq!(class_before(&gun), before, "{category} {kind}");
    }
    // What changes: one family, 2 of the 83 weapons.
    let changed: u32 = FAMILIES
        .iter()
        .filter(|f| f.4 != f.5)
        .map(|f| f.6)
        .sum();
    assert_eq!(changed, 2);
}

#[test]
fn with_the_switch_off_every_weapon_goes_by_its_category_as_before() {
    let fire = FireTuning::default();
    assert!(!fire.weapon_class_by_data);
    for gun in guns::guns() {
        assert_eq!(gun.class(), class_before(gun), "{}", gun.item);
        assert_eq!(fire.class_of(gun), gun.by_category, "{}", gun.item);
        // The bursts' table is the category 15's.
        if let Handling::Burst(legacy) = gun.handling {
            let smg = gun.category == 15;
            assert_eq!(legacy == AutoClass::Smg, smg, "{}", gun.item);
            assert_eq!(fire.auto_class(gun, legacy), legacy, "{}", gun.item);
        }
        // The crouch and side-step shares are the old ones.
        let marksman = gun.handling == Handling::Semi && matches!(gun.category, 8 | 13);
        let crouch = match gun.handling {
            Handling::Bolt => 0.5,
            Handling::Burst(_) if gun.category == 16 => fire.crouch_machine_gun,
            Handling::Burst(_) => fire.crouch_auto,
            _ if marksman => fire.crouch_marksman,
            _ => fire.crouch_other,
        };
        assert_eq!(fire.crouch_chance(gun, 0.5), crouch, "{}", gun.item);
        let strafe = match gun.handling {
            Handling::Bolt => fire.strafe_bolt,
            _ if marksman => fire.strafe_marksman,
            _ => 1.0,
        };
        assert_eq!(fire.strafe_chance(gun), strafe, "{}", gun.item);
    }
}

#[test]
fn with_the_switch_on_the_ak_74u_is_an_smg() {
    let ak = guns::gun("ak_74u").unwrap();
    assert_eq!(ak.by_category, WeaponClass::Rifle);
    assert_eq!(ak.class, WeaponClass::Smg);
    let fire = on();
    assert_eq!(fire.class_of(ak), WeaponClass::Smg);
    // The SMG's bursts' table, the SMG's band, the SMG's range floor.
    assert_eq!(fire.auto_class(ak, AutoClass::Rifle), AutoClass::Smg);
    assert_eq!(WeaponKind::of_class(ak.class), WeaponKind::Smg);
    let off = CombatTuning::default();
    let by_data = CombatTuning {
        fire,
        ..CombatTuning::default()
    };
    assert_eq!(off.fire_range_m(ak, 200.0), off.range_floor_rifle_m);
    assert_eq!(by_data.fire_range_m(ak, 200.0), by_data.range_floor_smg_m);
    assert!(by_data.range_floor_smg_m < off.range_floor_rifle_m);
}

#[test]
fn with_the_switch_on_the_other_families_move_as_the_data_says() {
    let fire = on();
    let gun = |kind, category, auto, bolt| Gun::of(&row(kind, category, auto, bolt));
    // An MP7 or a PPSh (category 15, type of an assault rifle) and the AEK-919K
    // (category 15, an automatic pistol): the shop's category wins, SMGs.
    let mp7 = gun("wpn_aslt", 15, true, false);
    assert_eq!(fire.class_of(&mp7), WeaponClass::Smg);
    assert_eq!(fire.auto_class(&mp7, AutoClass::Smg), AutoClass::Smg);
    assert_eq!(fire.class_of(&gun("wpn_pstl", 15, true, false)), WeaponClass::Smg);
    // The one exception: an SMG type in the rifle category is an SMG; in any
    // other category the category wins.
    assert_eq!(fire.class_of(&gun("wpn_smg", 8, true, false)), WeaponClass::Smg);
    assert_eq!(fire.class_of(&gun("wpn_smg", 13, false, false)), WeaponClass::Marksman);
    assert_eq!(fire.class_of(&gun("wpn_smg", 14, false, false)), WeaponClass::Shotgun);
    assert_eq!(fire.class_of(&gun("wpn_smg", 17, true, false)), WeaponClass::Pistol);
    assert_eq!(fire.class_of(&gun("wpn_aslt", 17, true, false)), WeaponClass::Pistol);
    // A one-round weapon of the rifle type is a marksman's for the crouch and
    // the side-step with the switch on, as with it off.
    let sks = gun("wpn_aslt", 8, false, false);
    assert_eq!(fire.crouch_chance(&sks, 0.5), fire.crouch_marksman);
    assert_eq!(FireTuning::default().crouch_chance(&sks, 0.5), fire.crouch_marksman);
    // The kinds of the five classes' neighbours.
    for (class, kind) in [
        (WeaponClass::Pistol, WeaponKind::Pistol),
        (WeaponClass::Smg, WeaponKind::Smg),
        (WeaponClass::Rifle, WeaponKind::Rifle),
        (WeaponClass::MachineGun, WeaponKind::Rifle),
        (WeaponClass::Shotgun, WeaponKind::Shotgun),
        (WeaponClass::Marksman, WeaponKind::Sniper),
        (WeaponClass::Bolt, WeaponKind::Sniper),
    ] {
        assert_eq!(WeaponKind::of_class(class), kind);
    }
}

#[test]
fn a_weapon_of_a_type_nobody_knows_takes_the_class_of_its_category() {
    for (category, class) in [
        (8, WeaponClass::Rifle),
        (13, WeaponClass::Marksman),
        (14, WeaponClass::Shotgun),
        (15, WeaponClass::Smg),
        (16, WeaponClass::MachineGun),
        (17, WeaponClass::Pistol),
        // A category that holds no weapons: a rifle, as `Gun::class` always said.
        (9, WeaponClass::Rifle),
    ] {
        let gun = Gun::of(&row("wpn_new", category, true, false));
        assert_eq!(gun.class, class, "{category}");
        assert_eq!(gun.class, gun.by_category, "{category}");
    }
    // A new bolt action of the sniper category is a bolt.
    assert_eq!(Gun::of(&row("wpn_new", 13, false, true)).class, WeaponClass::Bolt);
}

fn bots_with(fire_on: bool, roles: &[Option<&str>]) -> (Bots, Arc<Tuning>) {
    let mut tuning = Tuning::default();
    tuning.aim.combat.fire.weapon_class_by_data = fire_on;
    let tuning = Arc::new(tuning);
    let teams: Vec<u8> = (0..roles.len()).map(|place| (place % 2) as u8).collect();
    let driven = vec![true; roles.len()];
    let bots = Bots::new(
        0x5a5,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &driven,
        roles,
        Arc::clone(&tuning),
        Trace::none(0),
    );
    (bots, tuning)
}

/// The flanker has the Vityaz, then the AK-74U, then the MP-153: with the
/// switch on its kind follows the kit it wears (an SMG, an SMG, a shotgun);
/// off, it is the flanker's role's, an SMG, whatever it holds.
#[test]
fn the_kind_follows_the_held_weapon_with_the_switch_on_and_the_role_with_it_off() {
    for (fire_on, third) in [(true, WeaponKind::Shotgun), (false, WeaponKind::Smg)] {
        let (mut bots, tuning) = bots_with(fire_on, &[Some("flanker")]);
        let bot = bots.bots[0].as_mut().unwrap();
        let held = |bot: &super::super::Bot| (bot.reflex.held_gun().item, bot.weapon(&[], &tuning));
        assert_eq!(held(bot), ("vityaz", Some(WeaponKind::Smg)));
        // The kit changes at the next life, as `a_bot_with_its_first_kit_empty_picks_the_fullest`.
        bot.kit_chosen = 2;
        bot.new_life(0.5, None, Vec3::ZERO);
        assert_eq!(held(bot), ("mp_153", Some(third)), "{fire_on}");
    }
    // The support's AK-74U: a rifle by its role, an SMG by the data.
    let (bots, tuning) = bots_with(true, &[Some("support")]);
    let bot = bots.bots[0].as_ref().unwrap();
    assert_eq!(bot.reflex.held_gun().item, "ak_74u");
    assert_eq!(bot.weapon(&[], &tuning), Some(WeaponKind::Smg));
    let (bots, tuning) = bots_with(false, &[Some("support")]);
    assert_eq!(bots.bots[0].as_ref().unwrap().weapon(&[], &tuning), Some(WeaponKind::Rifle));
}

/// The activity counter: the bots whose class or band the data's class
/// changes.
#[test]
fn the_counts_say_which_bots_the_data_changes() {
    let (bots, _) = bots_with(false, &[Some("support"), Some("flanker"), Some("camper")]);
    let counts = bots.class_counts();
    assert_eq!(counts.bots, 3);
    // Only the support's AK-74U (the rifle band and tables, then the SMG's).
    assert_eq!((counts.differ, counts.burst, counts.floor), (1, 1, 1));
    // The band: the support's rifle to an SMG; the flanker's Vityaz and the
    // camper's Mosin stay.
    assert_eq!(counts.band, 1);
    // The same with the switch on: the count is of what the data would change.
    let (bots, _) = bots_with(true, &[Some("support"), Some("flanker"), Some("camper")]);
    assert_eq!(bots.class_counts(), counts);
}

/// What the type alone says (the family of its `parameters.type`), where it
/// says one: for the table of disagreeing weapons below.
fn family_by_type(w: &WeaponParams) -> Option<WeaponClass> {
    let bolt = !w.automatic() && w.has_chamber && w.chamber_a_round_time > 0.0;
    Some(match w.kind {
        "wpn_smg" => WeaponClass::Smg,
        "wpn_pstl" | "wpn_rvlr" | "wpn_apst" => WeaponClass::Pistol,
        "wpn_dbrl" | "wpn_stgn" | "wpn_asgn" => WeaponClass::Shotgun,
        "wpn_bolt" => WeaponClass::Bolt,
        "wpn_aslt" if w.category == 13 && bolt => WeaponClass::Bolt,
        "wpn_aslt" if w.category == 13 => WeaponClass::Marksman,
        "wpn_aslt" if w.category == 16 => WeaponClass::MachineGun,
        "wpn_aslt" => WeaponClass::Rifle,
        _ => return None,
    })
}

/// `SRP_GAME_DATA=<server-game-data.db> cargo test --release --lib --
/// --ignored --nocapture --exact v026g::bots::tests::weapon_class::on_the_live_data_the_weapons_where_type_and_category_disagree`:
/// every weapon whose `parameters.type` says another family than its category
/// (id, name, type, category, the category's class, the class the bots get),
/// and that the class differs from the category's for exactly the SMG types of
/// the rifle category.
#[test]
#[ignore = "needs the live game data (SRP_GAME_DATA)"]
fn on_the_live_data_the_weapons_where_type_and_category_disagree() {
    survarium::v026g::game_data::init().expect("SRP_GAME_DATA names the server's game data");
    let mut lines = Vec::new();
    let mut moved = Vec::new();
    for w in weapons::all() {
        let by_category = guns::gun(w.name).unwrap().by_category;
        let class = WeaponClass::of(w);
        assert_eq!(guns::gun(w.name).unwrap().class, class, "{}", w.name);
        if family_by_type(w).is_some_and(|family| family != by_category) {
            lines.push(format!(
                "{:>5} {:<26} {:<9} {:>2}  {:<8} -> {}",
                w.dict_id,
                w.name,
                w.kind,
                w.category,
                by_category.name(),
                class.name()
            ));
        }
        if class != by_category {
            moved.push(w.name);
            assert_eq!((w.kind, w.category), ("wpn_smg", 8), "{}", w.name);
        }
    }
    println!("{} disagreeing weapons of {}:
{}", lines.len(), weapons::all().len(), lines.join("
"));
    moved.sort_unstable();
    assert_eq!(moved, ["ak_74u", "ak_74u_premium"]);
    for gun in guns::guns() {
        assert_eq!(gun.class(), class_before(gun), "{}", gun.item);
    }
}
