//! The weapon numbers of the bots are the game data's, live: a value a
//! balance patch changes in a weapon's config reaches the number the bot
//! uses, and nothing in the bots keeps a copy of it by the weapon's name.
//!
//! Audit: `fighting-record/audit-weapon-data.md` (W2, W10, W11, the test of
//! section 4). The process's own tables (`weapons::all()`, `guns()`,
//! `kicks()`, `Arms::of_kit`) are read once from `SRP_GAME_DATA`, so a test
//! cannot swap them; it makes a patched copy of the test data
//! (`GameData::laid_over`, [`patched_item`]), derives a weapon table from it
//! (`weapons::derive`) and hands that table to the same builders the
//! process's tables are made by ([`guns_of`], [`kicks_of`], [`arms_in`]).
//! Nothing in a match changes: the process's tables are those builders over
//! `weapons::all()`.
//!
//! The tests are in two halves: the patched value reaches [`Gun`], [`Load`],
//! [`Arms`], [`Kick`] and the stand-in's draw time; and an unknown weapon
//! type still gets a distance band ([`WeaponKind::of_item`]).

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, Loadout, RusherKits, bot_kits};
use survarium::v026g::skills::MODIFIERS;
use survarium::v026g::weapons::fixtures::{ConfigValue, patched_item};
use survarium::v026g::weapons::{self, WeaponParams};

use super::standin::{DRAW_MS, draw_ms_of};
use crate::v026g::bots::guns::{
    self, Arms, Factors, Gun, Load, MG_CATEGORY, PISTOL_CATEGORY, RIFLE_CATEGORY, Reload,
    SHOTGUN_CATEGORY, SMG_CATEGORY, SNIPER_CATEGORY, Table, arms_in, gun_in, gun_with,
    gun_with_in, guns_of,
};
use crate::v026g::bots::recoil::{self, Kick, kick_in, kicks_of};
use crate::v026g::bots::reflex::WeaponKind;

/// The test data's weapon table with the AK-74U's config patched: every
/// number the bots read of a weapon, changed.
fn patched_table() -> Vec<WeaponParams> {
    let number = ConfigValue::Float;
    let data = patched_item(
        game_data::shared(),
        Dictionary::shared(),
        "ak_74u",
        &[
            (&["parameters", "reload_time"], number(2.0)),
            (&["parameters", "magazine_capacity"], number(45.0)),
            (&["parameters", "rounds_per_minute"], number(700.0)),
            (&["parameters", "aim_time"], number(0.4)),
            (&["parameters", "show_time"], number(0.5)),
            (&["parameters", "hide_time"], number(0.3)),
            (&["recoil", "min_top_recoil"], number(3.0)),
            (&["recoil", "max_top_recoil"], number(3.5)),
            (&["recoil", "recoil_power"], number(2.5)),
        ],
    );
    weapons::derive(&data, Dictionary::shared()).expect("the patched data's weapons read")
}

fn ak(guns: &[Gun]) -> &Gun {
    gun_in(guns, "ak_74u").expect("the AK-74U is in the data")
}

/// The first kit of the Low tier's support: the AK-74U's.
fn support_kit() -> &'static Loadout {
    let equipment = BotEquipment::of(BotTier::Low, RusherKits::default());
    bot_kits(equipment, BotRole::Support)[0]
}

#[test]
fn a_patched_weapon_reaches_the_gun_the_load_the_arms_the_kick_and_the_draw() {
    let stock_guns = guns::guns();
    let stock = ak(stock_guns);
    let stock_kick = kick_in(recoil::kicks(), "ak_74u");

    let table = patched_table();
    let patched = Table::of(table.clone());
    let patched_guns = patched.guns;
    let patched_kicks = kicks_of(&table);
    let gun = ak(patched_guns);

    // Gun: the magazine, the rate, the reload, the aim-in, the draw and
    // the put-away.
    assert_eq!(gun.magazine, 45);
    assert_eq!(gun.reload, Reload::Magazine { ms: 2_000 });
    assert_eq!(gun.reload_ms(0, 45), 2_000);
    assert_eq!(gun.rpm, 700);
    assert_eq!(gun.cycle_ms(), (60_000.0f32 / 700.0).ceil() as u32);
    assert_eq!((gun.aim_ms, gun.show_ms, gun.hide_ms), (400, 500, 300));
    // The stand-in's weapon coming up after a carrier drops a battery: the
    // patched `show_time` with the switch on, the old 800 ms for all without.
    assert_eq!(draw_ms_of(gun, true), 500);
    assert_eq!(draw_ms_of(gun, false), DRAW_MS);

    // Load: a life of 90 rounds loads the patched magazine.
    let load = Load::of(gun, 90);
    assert_eq!((load.rounds, load.spare), (45, 45));
    let before = Load::of(stock, 90);
    assert_eq!((before.rounds, before.spare), (stock.magazine, 90 - stock.magazine));

    // Kick: the config's degrees, the power, the rate.
    let kick = kick_in(&patched_kicks, "ak_74u");
    assert_eq!(kick.top_deg, (3.0, 3.5));
    assert_eq!((kick.power, kick.rpm), (2.5, 700));
    assert!((kick.top(0.5) - 3.25 / recoil::TOP_DEG_PER_UNIT).abs() < 1e-7);
    assert_eq!(kick.interval_s(), 60.0 / 700.0);

    // Arms: the support's first kit carries the patched gun, with the rounds
    // of its own ammo slots (the ammo is another item, not patched).
    let none = [0.0; MODIFIERS];
    let arms = arms_in(patched, Dictionary::shared(), support_kit(), &none, false)
        .expect("the kit's gun");
    assert!(std::ptr::eq(arms.primary.0, gun));
    assert_eq!(arms.primary.0.magazine, 45);
    assert_eq!(arms.primary.1, Arms::fallback().primary.1);
    assert_eq!(arms.reserve, Arms::fallback().reserve);

    // The upgrades go on top of the patched numbers, not the stock ones: the
    // reload-time mod 2044 (-15 %) of the patched 2.0 s, and the Arms with the
    // upgrades on carry that gun. The stock gun of the same mod is another.
    let fast = gun_with_in(patched, "ak_74u", &[2044], Factors::default()).unwrap();
    assert_eq!(fast.reload, Reload::Magazine { ms: 1_700 });
    let stock_fast = gun_with("ak_74u", &[2044], Factors::default()).unwrap();
    assert_ne!(stock_fast.reload, fast.reload);
    assert!(std::ptr::eq(
        fast,
        gun_with_in(patched, "ak_74u", &[2044], Factors::default()).unwrap()
    ));
    let on = arms_in(patched, Dictionary::shared(), support_kit(), &none, true).unwrap();
    assert!(std::ptr::eq(on.primary.0, gun), "no mods or factors: the table's own gun");

    // The stock is untouched: the process's tables read the process's data,
    // and the same builders over the process's weapons give the process's
    // numbers.
    assert_ne!(stock.magazine, 45);
    assert_ne!(stock.reload_ms(0, stock.magazine), 2_000);
    assert_ne!(stock.rpm, 700);
    assert_ne!(stock_kick.top_deg, (3.0, 3.5));
    assert_eq!(Arms::fallback().primary.0.item, "ak_74u");
    assert_eq!(Arms::fallback().primary.0.magazine, stock.magazine);
    assert_eq!(*Arms::fallback().primary.0, *stock);
    let rebuilt = guns_of(weapons::all());
    assert_eq!(rebuilt.as_slice(), stock_guns);
    let rebuilt: Vec<Kick> = kicks_of(weapons::all());
    assert_eq!(rebuilt.as_slice(), recoil::kicks());
    // The other weapons of the patched table are the stock ones.
    for (patched, stock) in patched_guns.iter().zip(stock_guns) {
        if patched.item != "ak_74u" {
            assert_eq!(patched, stock, "{}", patched.item);
        }
    }
}

#[test]
fn a_patched_weapon_of_a_new_type_still_has_a_distance_band() {
    let number = ConfigValue::Float;
    let data = patched_item(
        game_data::shared(),
        Dictionary::shared(),
        "ak_74u",
        &[
            (&["parameters", "type"], ConfigValue::String("wpn_new".into())),
            (&["parameters", "rounds_per_minute"], number(650.0)),
        ],
    );
    let table = weapons::derive(&data, Dictionary::shared()).unwrap();
    let w = table.iter().find(|w| w.name == "ak_74u").unwrap();
    assert_eq!(w.kind, "wpn_new");
    // Before: no kind at all. Now: its category's (8, the rifles'), and it
    // is counted as a fallback (the activity counter of the fallback).
    assert_eq!(WeaponKind::of_item(w.kind, w.category), Some(WeaponKind::Rifle));
    assert!(WeaponKind::by_fallback(w.kind, w.category));
    // The stock weapon it was is not one.
    let stock = weapons::weapon_named("ak_74u").unwrap();
    assert!(!WeaponKind::by_fallback(stock.kind, stock.category));
}

#[test]
fn a_made_up_weapon_type_takes_the_kind_of_its_category() {
    for (category, kind) in [
        (RIFLE_CATEGORY, WeaponKind::Rifle),
        (SNIPER_CATEGORY, WeaponKind::Sniper),
        (SHOTGUN_CATEGORY, WeaponKind::Shotgun),
        (SMG_CATEGORY, WeaponKind::Smg),
        (MG_CATEGORY, WeaponKind::Rifle),
        (PISTOL_CATEGORY, WeaponKind::Pistol),
    ] {
        assert_eq!(WeaponKind::of_item("wpn_new", category), Some(kind), "{category}");
        assert_eq!(WeaponKind::of_category(category), Some(kind), "{category}");
    }
    // A category that holds no weapons gives none (the kind stays unknown).
    for category in [0, 9, 12, 18, 22] {
        assert_eq!(WeaponKind::of_item("wpn_new", category), None, "{category}");
    }
    // A type the code knows is still its own, in whatever category.
    assert_eq!(WeaponKind::of_item("wpn_smg", RIFLE_CATEGORY), Some(WeaponKind::Smg));
    assert_eq!(WeaponKind::of_item("wpn_aslt", SMG_CATEGORY), Some(WeaponKind::Rifle));
    assert_eq!(WeaponKind::of_item("wpn_pstl", SMG_CATEGORY), Some(WeaponKind::Pistol));
    assert_eq!(WeaponKind::of_item("wpn_aslt", SNIPER_CATEGORY), Some(WeaponKind::Sniper));
}

/// `WeaponKind::of_item` before the fallback by category: the rule the live
/// weapons were given.
fn of_item_before(kind: &str, category: u8) -> Option<WeaponKind> {
    Some(match (category, kind) {
        (13, _) => WeaponKind::Sniper,
        (14, _) => WeaponKind::Shotgun,
        (_, "wpn_dbrl" | "wpn_stgn" | "wpn_asgn") => WeaponKind::Shotgun,
        (_, "wpn_bolt") => WeaponKind::Sniper,
        (_, "wpn_smg") => WeaponKind::Smg,
        (_, "wpn_pstl" | "wpn_rvlr" | "wpn_apst") => WeaponKind::Pistol,
        (_, "wpn_aslt") => WeaponKind::Rifle,
        _ => return None,
    })
}

/// The (category, `parameters.type`, kind) of the 83 weapons of the rev30
/// server game data (`audit-weapon-classes.tsv`), with how many have it.
const LIVE_83: [(u8, &str, WeaponKind, u32); 14] = [
    (8, "wpn_aslt", WeaponKind::Rifle, 27),
    (8, "wpn_smg", WeaponKind::Smg, 2),
    (13, "wpn_aslt", WeaponKind::Sniper, 5),
    (13, "wpn_bolt", WeaponKind::Sniper, 8),
    (14, "wpn_asgn", WeaponKind::Shotgun, 7),
    (14, "wpn_dbrl", WeaponKind::Shotgun, 5),
    (14, "wpn_stgn", WeaponKind::Shotgun, 3),
    (15, "wpn_aslt", WeaponKind::Rifle, 4),
    (15, "wpn_pstl", WeaponKind::Pistol, 1),
    (15, "wpn_smg", WeaponKind::Smg, 13),
    (16, "wpn_aslt", WeaponKind::Rifle, 3),
    (17, "wpn_apst", WeaponKind::Pistol, 1),
    (17, "wpn_pstl", WeaponKind::Pistol, 3),
    (17, "wpn_rvlr", WeaponKind::Pistol, 1),
];

#[test]
fn the_kinds_of_the_83_live_weapons_are_the_ones_they_had() {
    assert_eq!(LIVE_83.iter().map(|row| row.3).sum::<u32>(), 83);
    for (category, kind, expected, _) in LIVE_83 {
        assert_eq!(WeaponKind::of_item(kind, category), Some(expected), "{category} {kind}");
        assert_eq!(
            WeaponKind::of_item(kind, category),
            of_item_before(kind, category),
            "{category} {kind}"
        );
    }
}

/// Every weapon of the data in hand gives the same kind as before, and some
/// kind: with the test data here, and with the live data below.
fn every_weapon_keeps_its_kind(all: &[WeaponParams]) {
    for w in all {
        let kind = WeaponKind::of_item(w.kind, w.category);
        assert!(kind.is_some(), "{} ({} {})", w.name, w.category, w.kind);
        // No live weapon takes the fallback: its activity counter is 0.
        assert!(!WeaponKind::by_fallback(w.kind, w.category), "{}", w.name);
        assert_eq!(kind, of_item_before(w.kind, w.category), "{}", w.name);
    }
}

#[test]
fn the_weapons_of_the_test_data_keep_their_kinds() {
    every_weapon_keeps_its_kind(weapons::all());
}

/// `SRP_GAME_DATA=<server-game-data.db> cargo test --release --lib --
/// --ignored on_the_live_data_every_weapon`: for every weapon of the live
/// file, its gun, kick and kind exist and equal its config, its kind is the
/// one it had, and the weapons whose `show_time` is not the 0.8 s the
/// stand-in used for every weapon are listed.
#[test]
#[ignore = "needs the live game data (SRP_GAME_DATA)"]
fn on_the_live_data_every_weapon_has_its_gun_kick_and_kind_from_its_config() {
    game_data::init().expect("SRP_GAME_DATA names the server's game data");
    let all = weapons::all();
    assert!(!all.is_empty());
    every_weapon_keeps_its_kind(all);
    let mut odd = Vec::new();
    for w in all {
        let gun = guns::gun(w.name).unwrap_or_else(|| panic!("{}: no gun", w.name));
        assert_eq!(gun.magazine, w.magazine_capacity, "{}", w.name);
        assert_eq!(gun.rpm, w.rounds_per_minute, "{}", w.name);
        assert_eq!(gun.category, w.category, "{}", w.name);
        let kick = kick_in(recoil::kicks(), w.name);
        assert_eq!((kick.item, kick.rpm), (w.name, w.rounds_per_minute), "{}", w.name);
        if draw_ms_of(gun, true) != DRAW_MS {
            odd.push(format!("{} {} ms", w.name, draw_ms_of(gun, true)));
        }
    }
    println!(
        "{} weapons; draw time other than 800 ms: {}",
        all.len(),
        if odd.is_empty() { "none".to_owned() } else { odd.join(", ") }
    );
}
