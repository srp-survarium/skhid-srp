//! Chest rolls against the real extracted tables (data/chests_069d.json).

use game_data::chests::{OpenError, Reward};
use game_data::{GameData, SplitMix};

fn gd() -> GameData {
    game_data::testing::tables()
}

fn amounts(v: &[Reward]) -> Vec<(u16, u32)> {
    v.iter().map(|r| (r.dict, r.amount)).collect()
}

#[test]
fn every_dictionary_chest_has_data() {
    game_data::require_tables!();
    let g = gd();
    let cats: Vec<u16> = g.items.iter().filter(|i| i.category == 31).map(|i| i.dict_id).collect();
    assert!(cats.len() >= 47);
    for d in &cats {
        assert!(g.chests.is_chest(*d), "dictionary chest {d} has no chest data");
    }
}

#[test]
fn table_weights_add_up_to_one_hundred() {
    game_data::require_tables!();
    let g = gd();
    for d in g.chests.openable() {
        let _ = d;
    }
    for name in ["daily_quest_chest_1_drop", "pve_mission_1_chest_rank_12_drop", "vouchers_chest_drop", "rank_1_equip_chest_drop"] {
        let t = g.chests.table(name).unwrap();
        let sum: f64 = t.groups.iter().map(|x| x.weight).sum();
        assert!((sum - 100.0).abs() < 1e-3, "{name}: {sum}");
    }
}

#[test]
fn fixed_pack_gives_everything_listed() {
    game_data::require_tables!();
    let g = gd();
    // seven_days_chest_rank_1: 3 x voucher 853, 3 x booster 342, 3 x booster 339 (extraction type 2)
    let r = g.chests.open(1300, &mut SplitMix(1)).unwrap();
    assert_eq!(amounts(&r), vec![(853, 3), (342, 3), (339, 3)]);
    // rank_1_boosters_chest: 15 x premium ammo chest 1, 15 x voucher chest 1, 15 x booster chest
    let r = g.chests.open(1149, &mut SplitMix(2)).unwrap();
    assert_eq!(amounts(&r), vec![(1353, 15), (1148, 15), (1116, 15)]);
}

#[test]
fn set_chests_give_each_piece_once() {
    game_data::require_tables!();
    let g = gd();
    for seed in 0..20 {
        let mut r = amounts(&g.chests.open(2061, &mut SplitMix(seed)).unwrap());
        r.sort();
        assert_eq!(r, vec![(2055, 1), (2056, 1), (2057, 1), (2058, 1), (2059, 1), (2062, 1)]);
        let mut m: Vec<u16> = g.chests.open(2033, &mut SplitMix(seed)).unwrap().iter().map(|x| x.dict).collect();
        m.sort();
        let mut want = vec![1772, 1773, 1766, 1446, 1997, 1335, 1415, 1338, 1339, 1704];
        want.sort();
        assert_eq!(m, want, "rare_modules_chest_1: ten groups, ten draws");
    }
}

#[test]
fn single_draw_chests_follow_their_table() {
    game_data::require_tables!();
    let g = gd();
    // daily_quest_chest_1: groups weighted 10 / 14 / 14 / ...; money group 214 x 1000 has weight 14
    let t = g.chests.table("daily_quest_chest_1_drop").unwrap();
    let total: f64 = t.groups.iter().map(|x| x.weight).sum();
    let mut rng = SplitMix(7);
    let n = 20000;
    let mut silver = 0;
    for _ in 0..n {
        let r = g.chests.open(1157, &mut rng).unwrap();
        assert_eq!(r.len(), 1);
        if r[0].dict == 214 {
            assert_eq!(r[0].amount, 1000);
            silver += 1;
        }
    }
    let want = t.groups.iter().filter(|x| x.ids.as_deref() == Some(&[214][..])).map(|x| x.weight).sum::<f64>() / total;
    let got = silver as f64 / n as f64;
    assert!((got - want).abs() < 0.01, "silver share {got} vs table {want}");
    // a range: pve mission rank 11 silver is 250..750
    let mut seen = (u32::MAX, 0);
    for _ in 0..5000 {
        for x in g.chests.open(1483, &mut rng).unwrap() {
            if x.dict == 214 {
                seen = (seen.0.min(x.amount), seen.1.max(x.amount));
            }
        }
    }
    assert!(seen.0 >= 250 && seen.1 <= 750 && seen.0 < 300 && seen.1 > 700, "{seen:?}");
}

/// The premium ammo chests (tables with one group of weight 0) give one random box of their generation.
#[test]
fn premium_ammo_chests_give_one_box_of_their_generation() {
    game_data::require_tables!();
    let g = gd();
    let mut rng = SplitMix(11);
    for (chest, generation) in [(1351u16, 1usize), (1352, 2), (1353, 3)] {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..400 {
            let r = g.chests.open(chest, &mut rng).unwrap();
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].amount, 1);
            let d = r[0].dict;
            assert!(g.items.is_ammo_booster(d), "chest {chest}: dict {d}");
            // ammo_booster_K_G / _K_Gs: the generation is the name's last number
            let name = &g.items.get(d).unwrap().cfg_name;
            assert!(name.trim_end_matches('s').ends_with(&format!("_{generation}")), "chest {chest}: {name}");
            seen.insert(d);
        }
        assert_eq!(seen.len(), 8, "chest {chest}: all eight boxes of generation {generation}");
    }
}

#[test]
fn chests_the_data_cannot_open_are_refused() {
    game_data::require_tables!();
    let g = gd();
    let mut rng = SplitMix(3);
    // item-level columns not mapped
    assert!(matches!(g.chests.open(1115, &mut rng), Err(OpenError::Unresolved(_))));
    // table missing from chest_drops.options (pve mission rank 23)
    assert!(matches!(g.chests.open(1472, &mut rng), Err(OpenError::NoTable(_))));
    assert_eq!(g.chests.open(44, &mut rng), Err(OpenError::NotAChest));
    // everything else rolls
    let ok = g.chests.openable();
    assert!(ok.contains(&1157) && ok.contains(&2061) && ok.contains(&1300) && !ok.contains(&1115));
    for d in ok {
        assert!(!g.chests.open(d, &mut rng).unwrap().is_empty(), "chest {d}");
    }
}

#[test]
fn rewards_exist_in_the_dictionary() {
    game_data::require_tables!();
    let g = gd();
    let mut rng = SplitMix(11);
    for d in g.chests.openable() {
        for _ in 0..50 {
            for r in g.chests.open(d, &mut rng).unwrap() {
                assert!(g.items.get(r.dict).is_some(), "chest {d} rewards unknown dict {}", r.dict);
                assert!(r.amount >= 1);
            }
        }
    }
}
