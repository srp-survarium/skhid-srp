//! Cross-check the content-built tree and the native 68-float wire payload.
use survarium::v026g::skills::{SkillError, SkillsTree};

/// `(id, ranks, branch_points, column, row, modifier, bit shift)`: the
/// modifier carrying the packed ranks (43 the tree's first ten, 45 the
/// combo skills) and the rank field's first bit (medical-skills.md).
#[rustfmt::skip]
const TREE: [(u16, u8, u32, u8, u8, usize, u32); 13] = [
    (301, 3, 0, 2, 1, 43, 0), (302, 2, 0, 3, 1, 43, 3),
    (311, 5, 3, 1, 2, 43, 6), (313, 2, 3, 3, 2, 45, 2), (312, 3, 3, 4, 2, 43, 9),
    (342, 2, 10, 1, 3, 43, 21), (303, 2, 10, 2, 3, 45, 0), (321, 3, 10, 4, 3, 43, 12),
    (332, 5, 15, 2, 4, 43, 17), (323, 4, 15, 3, 4, 45, 4), (333, 4, 15, 4, 4, 45, 7),
    (331, 1, 20, 2, 5, 43, 16), (322, 1, 20, 3, 5, 43, 15),
];

#[test]
#[ignore = "requires SRP_MEDICAL_SKILLS_CONFIG from release/build-content"]
fn medical_tree_progression_and_native_payload() {
    let path = std::env::var("SRP_MEDICAL_SKILLS_CONFIG").expect("built skills_tree.options");
    let tree = SkillsTree::parse(&std::fs::read(path).unwrap()).unwrap();
    let medical: Vec<u16> = tree
        .skills
        .iter()
        .filter(|skill| skill.branch() == 3)
        .map(|skill| skill.id)
        .collect();
    let mut expected: Vec<u16> = TREE.iter().map(|row| row.0).collect();
    expected.sort_unstable();
    assert_eq!(medical, expected, "Medic (341) left the tree");
    assert!(tree.skill(341).is_none());
    let mut positions = std::collections::HashSet::new();
    for &(id, ranks, branch_points, column, row, ..) in &TREE {
        let skill = tree.skill(id).unwrap();
        assert_eq!(
            (
                skill.levels_amount,
                skill.branch_points,
                skill.column,
                skill.row
            ),
            (ranks, branch_points, column, row),
            "skill {id}"
        );
        assert_eq!(skill.levels.len(), usize::from(ranks), "skill {id}");
        assert!(skill.unknown_modifiers.is_empty());
        assert!(positions.insert((column, row)), "overlapping skill {id}");
        if branch_points > 0 {
            assert!(matches!(
                tree.validate(100, &[(id, 1)]),
                Err(SkillError::Locked { .. })
            ));
        }
        assert!(matches!(
            tree.validate(100, &[(id, ranks + 1)]),
            Err(SkillError::TooManyPoints { .. })
        ));
    }
    // Each tier needs its threshold in the rows above: 0/3/10/15/20.
    let tier_one = [(301, 3), (302, 2)];
    tree.validate(100, &[(301, 3), (311, 1)]).unwrap();
    tree.validate(100, &[(301, 1), (302, 2), (311, 1)]).unwrap();
    assert!(matches!(
        tree.validate(100, &[(301, 2), (311, 1)]),
        Err(SkillError::Locked { skill: 311, .. })
    ));
    let mut nine = tier_one.to_vec();
    nine.extend([(311, 4), (303, 1)]);
    assert!(matches!(
        tree.validate(100, &nine),
        Err(SkillError::Locked { skill: 303, .. })
    ));
    let mut ten = tier_one.to_vec();
    ten.extend([(311, 5), (303, 1)]);
    tree.validate(100, &ten).unwrap();
    let mut fourteen = tier_one.to_vec();
    fourteen.extend([(311, 5), (312, 3), (313, 1), (323, 1)]);
    assert!(matches!(
        tree.validate(100, &fourteen),
        Err(SkillError::Locked { skill: 323, .. })
    ));
    let mut fifteen = tier_one.to_vec();
    fifteen.extend([(311, 5), (312, 3), (313, 2), (323, 1)]);
    tree.validate(100, &fifteen).unwrap();
    // A full build: every skill at its top rank but one of the tier-5 pair
    // and one of tier 4's Unflinching and Concentration.
    let mut spent = Vec::new();
    for &(id, ranks, ..) in &TREE {
        if id == 322 || id == 333 {
            continue;
        }
        for n in 1..=ranks {
            let mut preview = spent.clone();
            preview.push((id, n));
            tree.validate(100, &preview).unwrap();
        }
        spent.push((id, ranks));
    }
    assert_eq!(spent.iter().map(|(_, n)| u32::from(*n)).sum::<u32>(), 32);
    let mut medicine = spent.clone();
    medicine.retain(|&(id, _)| id != 331);
    medicine.push((322, 1));
    tree.validate(100, &medicine).unwrap();
    for set in [&spent, &medicine] {
        let mut both = set.clone();
        both.push(if set.contains(&(331, 1)) {
            (322, 1)
        } else {
            (331, 1)
        });
        assert!(matches!(
            tree.validate(100, &both),
            Err(SkillError::Exclusive(..))
        ));
    }
    for old in [111, 113, 322] {
        assert!(tree.skill(old).unwrap().antiskills.contains(&331));
        assert!(tree.skill(331).unwrap().antiskills.contains(&old));
    }
    // Concentration instead of Unflinching: the same 32 points; both refused.
    let concentration: Vec<(u16, u8)> = spent
        .iter()
        .map(|&(id, n)| if id == 323 { (333, n) } else { (id, n) })
        .collect();
    tree.validate(100, &concentration).unwrap();
    let mut both = spent.clone();
    both.push((333, 1));
    assert!(matches!(
        tree.validate(100, &both),
        Err(SkillError::Exclusive(..))
    ));
    assert_eq!(tree.skill(323).unwrap().antiskills, [333]);
    assert_eq!(tree.skill(333).unwrap().antiskills, [323]);
    assert_eq!(tree.modifiers(&concentration)[45], 522.0);
    // Both tier-5 unlocks' packed bits, to check the transport of each.
    let mut all = spent.clone();
    all.push((322, 1));
    let modifiers = tree.modifiers(&all);
    for slot in [43, 45] {
        let encoded: u32 = TREE
            .iter()
            .filter(|row| row.5 == slot && all.iter().any(|&(id, _)| id == row.0))
            .map(|&(_, ranks, .., shift)| u32::from(ranks) << shift)
            .sum();
        assert_eq!(modifiers[slot], encoded as f32, "modifier {slot}");
        assert_eq!(modifiers[slot] as u32, encoded);
    }
    assert_eq!(modifiers[45], 74.0);
    assert_eq!(modifiers[33], 0.0); // activation is filtered by native medicine kind
    assert!((modifiers[57] - 0.24).abs() < 1e-6);
    assert_eq!(modifiers[15], 1.0);
    assert_eq!(modifiers[64], 1.0);
}
