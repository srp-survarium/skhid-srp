//! The content-built technical tree (technical-skills.md): its layout, thresholds and the packed
//! value a kit gets in modifier 47, which `technical_skills::Skills` parses.
use survarium::v026g::skills::{SkillError, SkillsTree, modifier_index};
use technical_skills::{FIELDS, LOW, Skills, TankHarness, weight};

/// `(id, ranks, branch_points, column, row)`.
#[rustfmt::skip]
const TREE: [(u16, u8, u32, u8, u8); 17] = [
    (401, 3, 0, 3, 1), (402, 3, 0, 1, 1), (403, 2, 0, 4, 1), (404, 2, 0, 2, 1),
    (411, 3, 4, 3, 2), (412, 1, 4, 2, 2), (413, 2, 4, 4, 2), (414, 2, 4, 1, 2),
    (422, 1, 8, 2, 3), (423, 1, 8, 3, 3), (431, 3, 8, 4, 3),
    (432, 1, 12, 1, 4), (433, 2, 12, 3, 4),
    (441, 3, 16, 1, 5), (442, 1, 12, 4, 4), (443, 1, 16, 2, 5), (444, 1, 16, 3, 5),
];

/// Tank Pocket and Tank Depth, row II's pair: one or the other.
const TANK_POCKET: u16 = 413;
const TANK_DEPTH: u16 = 414;

fn other(id: u16) -> Option<u16> {
    match id {
        TANK_POCKET => Some(TANK_DEPTH),
        TANK_DEPTH => Some(TANK_POCKET),
        _ => None,
    }
}

/// Every rank of the rows up to `last`, with `pair` of row II's pair.
fn build(last: u8, pair: u16) -> Vec<(u16, u8)> {
    TREE.iter()
        .filter(|&&(id, .., row)| row <= last && other(id).is_none_or(|_| id == pair))
        .map(|&(id, ranks, ..)| (id, ranks))
        .collect()
}

fn points(skills: &[(u16, u8)]) -> u32 {
    skills.iter().map(|&(_, points)| u32::from(points)).sum()
}

#[test]
#[ignore = "requires SRP_MEDICAL_SKILLS_CONFIG (the built skills_tree.options) from release/build-content"]
fn technical_tree_progression_and_modifiers() {
    let path = std::env::var("SRP_MEDICAL_SKILLS_CONFIG").expect("built skills_tree.options");
    let tree = SkillsTree::parse(&std::fs::read(path).unwrap()).unwrap();
    let ids: Vec<u16> = tree.skills.iter().filter(|s| s.branch() == 4).map(|s| s.id).collect();
    assert_eq!(ids, TREE.map(|row| row.0));
    let mut places = std::collections::HashSet::new();
    for &(id, ranks, branch_points, column, row) in &TREE {
        let skill = tree.skill(id).unwrap();
        assert_eq!(
            (skill.levels_amount, skill.branch_points, skill.column, skill.row),
            (ranks, branch_points, column, row),
            "skill {id}"
        );
        let excludes: Vec<u16> = other(id).into_iter().collect();
        assert!(skill.unknown_modifiers.is_empty() && skill.antiskills == excludes, "{id}");
        assert!(places.insert((column, row)), "two skills at {id}'s place");
        // Each skill's field in modifier 47 is the crate's.
        let field = FIELDS.iter().find(|f| f.skill == id).unwrap();
        assert_eq!(ranks, field.ranks, "{id}");
    }
    // The pair: either alone is fine, both are refused.
    for pair in [TANK_POCKET, TANK_DEPTH] {
        tree.validate(100, &build(2, pair)).unwrap();
    }
    let mut both = build(2, TANK_POCKET);
    both.push((TANK_DEPTH, 1));
    assert!(matches!(tree.validate(100, &both), Err(SkillError::Exclusive(..))));
    // A full build is 30 points; each row opens at 0/4/8/12/16.
    let all = build(5, TANK_DEPTH);
    assert_eq!(points(&all), 30);
    assert_eq!(points(&build(5, TANK_POCKET)), 30);
    tree.validate(100, &all).unwrap();
    tree.validate(30, &all).unwrap();
    assert!(matches!(tree.validate(29, &all), Err(SkillError::OverLevel { .. })));
    for row in 2..=5u8 {
        let needs = 4 * u32::from(row - 1);
        let first = TREE.iter().find(|r| r.4 == row).unwrap().0;
        let mut set = build(row - 1, TANK_POCKET);
        let mut spare = points(&set) - needs;
        for (_, points) in set.iter_mut().rev() {
            let take = spare.min(u32::from(*points)) as u8;
            *points -= take;
            spare -= u32::from(take);
        }
        let short = set.iter().rposition(|&(_, points)| points > 0).unwrap();
        set.push((first, 1));
        tree.validate(100, &set).unwrap_or_else(|e| panic!("row {row} at {needs}: {e}"));
        set[short].1 -= 1;
        assert!(
            matches!(tree.validate(100, &set), Err(SkillError::Locked { skill, .. }) if skill == first),
            "row {row} at {}",
            needs - 1
        );
    }
    // The packed value: each rank adds its digit's weight; nothing else of the 68 is touched.
    let at = modifier_index("spot_enemy_traps").unwrap();
    assert_eq!(at, technical_skills::MODIFIER);
    let m = tree.modifiers(&all);
    for (index, &value) in m.iter().enumerate() {
        if index != at {
            assert_eq!(value, 0.0, "modifier {index}");
        }
    }
    let full = Skills::parse(m[at]).unwrap();
    assert_eq!(full.pack(), m[at]);
    assert_eq!(full.battery_take_speed(), 1.3);
    assert_eq!(full.friendly_blast(), 0.5);
    assert!(matches!(full.tank_harness(), TankHarness::Depth(_)));
    let pocket = Skills::parse(tree.modifiers(&build(5, TANK_POCKET))[at]).unwrap();
    assert!(matches!(pocket.tank_harness(), TankHarness::Pocket(_)));
    assert!(full.sharp_ears() && full.scavenger() && full.oxygen_rush() && full.bear_trap());
    for (n, field) in FIELDS.iter().enumerate() {
        let one = tree.modifiers(&[(field.skill, 1)])[at];
        let want = if field.skill == 444 { -(LOW as f32) } else { weight(n) as f32 };
        assert_eq!(one, want, "{}", field.skill);
    }
    // A saved set past its levels is settled, never more.
    assert_eq!(tree.settle(100, &[(401, 9)]), vec![(401, 3)]);
}
