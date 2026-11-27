//! Skill rules and the modifiers learned skills give (compared with reference values of an earlier
//! implementation when they are in the test tables folder).

use std::collections::BTreeMap;

use game_data::skills::{branch_of, spent, ResetPayment, SkillError, Skills};
use game_data::GameData;
use serde_json::Value;

fn gd() -> GameData {
    game_data::testing::tables()
}

fn set(v: &[(u16, u8)]) -> BTreeMap<u16, u8> {
    v.iter().copied().collect()
}

#[test]
fn tree_has_69_skills_in_5_branches_with_tier_rows() {
    game_data::require_tables!();
    let g = gd();
    assert_eq!(g.skills.len(), 69);
    for r in g.skills.rows() {
        assert!((1..=5).contains(&branch_of(r.id)), "{}", r.id);
        assert!((1..=5).contains(&r.levels), "{}", r.id);
        // the tier row digit (id / 10 % 10) names the column group; branch_points is 5 x (row - 1)
        assert_eq!(r.branch_points, 5 * ((r.id / 10 % 10) as u32 - 1), "skill {}", r.id);
        // every level has a row of modifiers in the match tree
    }
    assert_eq!(g.skills.row(543).unwrap().levels, 3);
    assert_eq!(g.skills.row(513).unwrap().levels, 5);
}

#[test]
fn apply_rules() {
    game_data::require_tables!();
    let g = gd();
    let s = &g.skills;
    let none = BTreeMap::new();
    // a plain learn
    assert_eq!(s.apply(&none, &[(112, 3), (113, 2)], 5).unwrap(), set(&[(112, 3), (113, 2)]));
    // zero entries vanish
    assert_eq!(s.apply(&none, &[(112, 0), (113, 1)], 5).unwrap(), set(&[(113, 1)]));
    // unknown skill, duplicate, too many points, over budget
    assert_eq!(s.apply(&none, &[(999, 1)], 5), Err(SkillError::Unknown(999)));
    assert_eq!(s.apply(&none, &[(112, 1), (112, 2)], 5), Err(SkillError::Duplicate(112)));
    assert_eq!(s.apply(&none, &[(543, 4)], 50), Err(SkillError::TooManyPoints { id: 543, points: 4, max: 3 }));
    assert_eq!(s.apply(&none, &[(112, 3), (113, 3)], 5), Err(SkillError::OverBudget { spent: 6, available: 5 }));
    // points are only added
    let cur = set(&[(112, 3)]);
    assert_eq!(s.apply(&cur, &[(112, 2)], 5), Err(SkillError::Decrease(112)));
    assert_eq!(s.apply(&cur, &[], 5), Err(SkillError::Decrease(112)));
    assert_eq!(s.apply(&cur, &[(112, 3), (113, 1)], 5).unwrap(), set(&[(112, 3), (113, 1)]));
    // a tier-2 skill (121, 5 points in the branch) needs 5 points in other skills of branch 1
    assert_eq!(s.apply(&none, &[(121, 1)], 50), Err(SkillError::Locked { id: 121, need: 5, have: 0 }));
    assert_eq!(s.apply(&none, &[(112, 4), (121, 1)], 50), Err(SkillError::Locked { id: 121, need: 5, have: 4 }));
    assert!(s.apply(&none, &[(112, 5), (121, 1)], 50).is_ok());
    // another branch's points do not count
    assert!(s.apply(&none, &[(212, 5), (121, 1)], 50).is_err());
}

#[test]
fn reset_rules() {
    game_data::require_tables!();
    let g = gd();
    let s = &g.skills;
    let cur = set(&[(112, 5), (121, 1), (212, 2)]);
    // wire branch = id / 100 - 1: 0 = 1xx, 1 = 2xx; one branch per reset, no "all"
    assert_eq!(s.reset_points(&cur, 0).unwrap(), 6);
    assert_eq!(s.reset_points(&cur, 1).unwrap(), 2);
    assert_eq!(s.reset_points(&cur, 2).unwrap(), 0);
    assert_eq!(s.reset_points(&cur, 4).unwrap(), 0);
    assert_eq!(s.reset_points(&cur, 5), Err(SkillError::NoBranch(5)));
    assert_eq!(s.without_branch(&cur, 0), set(&[(212, 2)]));
    assert_eq!(s.without_branch(&cur, 1), set(&[(112, 5), (121, 1)]));
    assert_eq!(spent(&cur), 8);
    // price per point, payment byte
    assert_eq!(s.reset_cost(6, ResetPayment::Silver), 6 * s.prices.reset_silver_per_point as u64);
    assert_eq!(s.reset_cost(6, ResetPayment::Gold), 6 * s.prices.reset_gold_per_point as u64);
    assert_eq!(Skills::payment(0), Ok(ResetPayment::Silver));
    assert_eq!(Skills::payment(1), Ok(ResetPayment::Gold));
    assert_eq!(Skills::payment(2), Err(SkillError::NoPayment(2)));
}

#[test]
fn modifiers_equal_the_reference() {
    game_data::require_tables!();
    let Some(text) = game_data::testing::reference("skills_expected_069d.json") else {
        eprintln!("skipped modifiers_equal_the_reference: no skills_expected_069d.json in the test tables folder");
        return;
    };
    let g = gd();
    let sets: Vec<Value> = serde_json::from_str(&text).unwrap();
    assert!(sets.len() >= 5);
    for e in &sets {
        let learned: BTreeMap<u16, u8> = e["skills"].as_object().unwrap().iter().map(|(k, v)| (k.parse().unwrap(), v.as_u64().unwrap() as u8)).collect();
        let want: BTreeMap<u32, f64> = e["mods"].as_object().unwrap().iter().map(|(k, v)| (k.parse().unwrap(), v.as_f64().unwrap())).collect();
        let got = g.matches.skill_modifiers(&Skills::match_keys(&learned));
        assert_eq!(got.keys().collect::<Vec<_>>(), want.keys().collect::<Vec<_>>(), "{}", e["name"]);
        for (k, v) in &want {
            assert!((got[k] - v).abs() < 1e-6, "{} modifier {k}: {} vs {v}", e["name"], got[k]);
        }
    }
}
