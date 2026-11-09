//! The content-built artefact tree (artefact-skills.md): its layout, thresholds and what a kit
//! gets in its 68 modifiers.
use survarium::v026g::skills::{SkillError, SkillsTree, modifier_index};

/// `(id, ranks, branch_points, column, row)`.
#[rustfmt::skip]
const TREE: [(u16, u8, u32, u8, u8); 12] = [
    (501, 4, 0, 2, 1), (502, 5, 0, 3, 1),
    (512, 5, 5, 3, 2), (513, 1, 5, 4, 2),
    (521, 1, 10, 2, 3), (522, 1, 10, 3, 3), (523, 4, 10, 1, 3),
    (531, 1, 15, 1, 4), (532, 1, 15, 2, 4), (533, 1, 15, 3, 4),
    (541, 1, 20, 2, 5), (542, 1, 20, 3, 5),
];

/// Keeper of the Artifacts and Forest Protection, the row-III pair: one or the other.
const KEEPER: u16 = 521;
const FOREST: u16 = 523;
/// Resonance and Cascade, the row-V pair: one or the other.
const RESONANCE: u16 = 541;
const CASCADE: u16 = 542;

/// The other skill of an either/or pair.
fn other(id: u16) -> Option<u16> {
    match id {
        KEEPER => Some(FOREST),
        FOREST => Some(KEEPER),
        RESONANCE => Some(CASCADE),
        CASCADE => Some(RESONANCE),
        _ => None,
    }
}

/// Every rank of the rows up to `last` with `row3` of the row-III pair and `row5` of the row-V
/// pair.
fn build(last: u8, row3: u16, row5: u16) -> Vec<(u16, u8)> {
    TREE.iter()
        .filter(|&&(id, .., row)| row <= last && other(id).is_none_or(|_| id == row3 || id == row5))
        .map(|&(id, ranks, ..)| (id, ranks))
        .collect()
}

fn points(skills: &[(u16, u8)]) -> u32 {
    skills.iter().map(|&(_, points)| u32::from(points)).sum()
}

fn locked(tree: &SkillsTree, skills: &[(u16, u8)], skill: u16) -> bool {
    matches!(tree.validate(100, skills), Err(SkillError::Locked { skill: s, .. }) if s == skill)
}

fn exclusive(tree: &SkillsTree, skills: &[(u16, u8)]) -> bool {
    matches!(tree.validate(100, skills), Err(SkillError::Exclusive(..)))
}

#[test]
#[ignore = "requires SRP_MEDICAL_SKILLS_CONFIG (the built skills_tree.options) from release/build-content"]
fn artefact_tree_progression_and_modifiers() {
    let path = std::env::var("SRP_MEDICAL_SKILLS_CONFIG").expect("built skills_tree.options");
    let tree = SkillsTree::parse(&std::fs::read(path).unwrap()).unwrap();
    let ids: Vec<u16> = tree.skills.iter().filter(|s| s.branch() == 5).map(|s| s.id).collect();
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
    }
    // Each pair: either alone is fine, both are refused.
    tree.validate(100, &[(501, 4), (502, 1), (512, 5), (KEEPER, 1)]).unwrap();
    tree.validate(100, &[(501, 4), (502, 1), (512, 5), (FOREST, 4)]).unwrap();
    assert!(exclusive(&tree, &[(501, 4), (502, 1), (512, 5), (KEEPER, 1), (FOREST, 1)]));
    for row3 in [KEEPER, FOREST] {
        let mut both = build(5, row3, RESONANCE);
        both.push((CASCADE, 1));
        assert!(exclusive(&tree, &both), "Resonance and Cascade with {row3}");
        let mut both = build(5, row3, CASCADE);
        both.push((other(row3).unwrap(), 1));
        assert!(exclusive(&tree, &both), "Keeper and Forest Protection with Cascade");
    }
    // The stock rows, 0/5/10/15/20, on both paths: row I holds 9; with Keeper rows I-III hold
    // 17 and rows I-IV 20 (row V needs every point, a full build is 21), with Forest Protection
    // rows I-IV hold 23 (a full build is 24).
    for (row3, rows_i_iv, full) in [(KEEPER, 20, 21), (FOREST, 23, 24)] {
        assert_eq!(points(&build(4, row3, RESONANCE)), rows_i_iv, "{row3}");
        for row5 in [RESONANCE, CASCADE] {
            let all = build(5, row3, row5);
            assert_eq!(points(&all), full, "{row3} {row5}");
            tree.validate(100, &all).unwrap();
            tree.validate(full as u8, &all).unwrap();
            assert!(matches!(tree.validate(full as u8 - 1, &all), Err(SkillError::OverLevel { .. })));
        }
        // Each row opens at its threshold: exactly that many points in the rows above it (the
        // spare ones taken from the nearest row first), and a point less locks it.
        for row in 2..=5u8 {
            let needs = 5 * u32::from(row - 1);
            let first = build(row, row3, RESONANCE).last().unwrap().0;
            let mut set = build(row - 1, row3, RESONANCE);
            let mut spare = points(&set) - needs;
            for (_, points) in set.iter_mut().rev() {
                let take = spare.min(u32::from(*points)) as u8;
                *points -= take;
                spare -= u32::from(take);
            }
            assert_eq!(points(&set), needs, "{row3} row {row}");
            let short = set.iter().rposition(|&(_, points)| points > 0).unwrap();
            set.push((first, 1));
            tree.validate(100, &set).unwrap_or_else(|e| panic!("{row3}: row {row} at {needs}: {e}"));
            set[short].1 -= 1;
            assert!(locked(&tree, &set, first), "{row3}: row {row} at {}", needs - 1);
        }
    }
    // The modifiers of a full build: the data skills' native ones and the packed integer.
    let m = tree.modifiers(&build(5, FOREST, RESONANCE));
    let at = |name: &str| m[modifier_index(name).unwrap()];
    assert!((at("artefact_search_time") + 0.24).abs() < 1e-6);
    assert!((at("incoming_weapon_damage_in_anomaly") + 0.2).abs() < 1e-6);
    // Artifact Container: an artefact slot for a pocket.
    assert_eq!((at("artefact_slots"), at("pocket_slots")), (1.0, -1.0));
    assert!((at("artefact_duration") - 0.25).abs() < 1e-6);
    // Resistance 5, then bits 4..7 (Resonance, Spring II, Onyx II, Larkspur II) and 12 (Artifact
    // Sight); with Cascade, bit 8 instead of bit 4; with Keeper, bit 3 and no modifier 41.
    let power = modifier_index("artefact_power").unwrap();
    let damage = modifier_index("incoming_weapon_damage_in_anomaly").unwrap();
    assert_eq!(at("artefact_power"), 4341.0);
    assert_eq!(tree.modifiers(&build(5, FOREST, CASCADE))[power], 4581.0);
    let keeper = tree.modifiers(&build(5, KEEPER, RESONANCE));
    assert_eq!((keeper[power], keeper[damage]), (4349.0, 0.0));
    assert_eq!(tree.modifiers(&build(5, KEEPER, CASCADE))[power], 4589.0);
    #[rustfmt::skip]
    let bits = [(502, 1.0), (KEEPER, 8.0), (RESONANCE, 16.0), (531, 32.0), (532, 64.0), (533, 128.0), (CASCADE, 256.0), (513, 4096.0)];
    for (id, bits) in bits {
        assert_eq!(tree.modifiers(&[(id, 1)])[power], bits, "{id}");
    }
    // Knowledge of Artifacts is the active duration alone (no bit).
    assert_eq!(tree.modifiers(&[(512, 5)])[power], 0.0);
    // Cascade is also the retained Medic (modifier 43 bit 20): a Larkspur's activation heals the
    // teammates within 5 m. No other skill of the tree sets 43.
    let armored = modifier_index("armored_trap").unwrap();
    assert_eq!(tree.modifiers(&[(CASCADE, 1)])[armored], 1_048_576.0);
    assert_eq!(tree.modifiers(&build(5, FOREST, CASCADE))[armored], 1_048_576.0);
    assert_eq!(at("armored_trap"), 0.0);
    let only_data = tree.modifiers(&[(501, 4), (512, 5), (522, 1), (FOREST, 4)]);
    assert_eq!(only_data[power], 0.0);
    // A saved set past its levels or below its rows is settled (refunded), never more.
    assert_eq!(tree.settle(100, &[(501, 9)]), vec![(501, 4)]);
    assert_eq!(tree.settle(100, &[(501, 2), (531, 1)]), vec![(501, 2)]);
}
