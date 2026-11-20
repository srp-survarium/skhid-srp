//! Cross-check the content-built tree and the native 68-float wire payload.
use survarium::v026g::skills::{SkillError, SkillsTree};

/// `(id, ranks, branch_points, column, row, modifier, bit shift)`: the
/// modifier carrying the packed ranks (43 the tree's first ten, 45 the
/// combo skills) and the rank field's first bit (medical-skills.md).
#[rustfmt::skip]
const TREE: [(u16, u8, u32, u8, u8, usize, u32); 14] = [
    (342, 1, 0, 2, 1, 43, 21), (302, 2, 0, 3, 1, 43, 3),
    (301, 3, 3, 1, 2, 43, 0), (311, 4, 3, 2, 2, 43, 6), (312, 3, 3, 3, 2, 43, 9), (314, 3, 3, 4, 2, 45, 12),
    (303, 2, 7, 1, 3, 45, 0), (321, 3, 7, 2, 3, 43, 12), (324, 3, 7, 4, 3, 45, 10),
    (323, 4, 12, 1, 4, 45, 4), (332, 5, 12, 2, 4, 43, 17), (333, 3, 12, 4, 4, 45, 7),
    (331, 1, 17, 2, 5, 43, 16), (322, 1, 17, 3, 5, 43, 15),
];

#[test]
#[ignore = "requires SRP_MEDICAL_SKILLS_CONFIG from release/build-content"]
fn medical_tree_progression_and_native_payload() {
    let path = std::env::var("SRP_MEDICAL_SKILLS_CONFIG").expect("built skills_tree.options");
    let tree = SkillsTree::parse(&std::fs::read(path).unwrap()).unwrap();
    // physical-skills-1.json: the two unlearnable melee skills out, Packrat
    // in column 4 of its row, no two physical skills in one place.
    assert!(tree.skill(206).is_none() && tree.skill(210).is_none());
    let packrat = tree.skill(202).unwrap();
    assert_eq!(
        (packrat.column, packrat.row, packrat.branch_points),
        (4, 3, 10)
    );
    let athlete = tree.skill(209).unwrap();
    assert_eq!((athlete.column, athlete.row), (3, 2));
    let mut physical = std::collections::HashSet::new();
    for skill in tree.skills.iter().filter(|skill| skill.branch() == 2) {
        assert!(physical.insert((skill.column, skill.row)), "{}", skill.id);
    }
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
    assert!(tree.skill(313).is_none(), "Take Cover left the tree");
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
    // Each tier needs its threshold in the rows above: 0/3/7/12/17.
    // Each row first locked one point short, then open with that point.
    let locked_then_open = |short: &[(u16, u8)], open: &[(u16, u8)], skill: u16| {
        assert!(
            matches!(tree.validate(100, short), Err(SkillError::Locked { skill: s, .. }) if s == skill),
            "{skill} with {short:?}"
        );
        tree.validate(100, open).unwrap();
    };
    locked_then_open(&[(302, 2), (301, 1)], &[(342, 1), (302, 2), (301, 1)], 301);
    let row_one = [(342, 1), (302, 2)];
    let with = |more: &[(u16, u8)]| [&row_one[..], more].concat();
    locked_then_open(&with(&[(311, 3), (303, 1)]), &with(&[(311, 4), (303, 1)]), 303);
    locked_then_open(
        &with(&[(311, 4), (301, 3), (312, 1), (323, 1)]),
        &with(&[(311, 4), (301, 3), (312, 2), (323, 1)]),
        323,
    );
    locked_then_open(
        &with(&[(311, 4), (301, 3), (312, 3), (303, 2), (323, 1), (331, 1)]),
        &with(&[(311, 4), (301, 3), (312, 3), (303, 2), (323, 2), (331, 1)]),
        331,
    );
    // A full build: every skill at its top rank but one of the tier-5 pair
    // (Unflinching and Concentration go together).
    let mut spent = Vec::new();
    for &(id, ranks, ..) in &TREE {
        if id == 322 {
            continue;
        }
        for n in 1..=ranks {
            let mut preview = spent.clone();
            preview.push((id, n));
            tree.validate(100, &preview).unwrap();
        }
        spent.push((id, ranks));
    }
    assert_eq!(spent.iter().map(|(_, n)| u32::from(*n)).sum::<u32>(), 37);
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
    assert!(tree.skill(323).unwrap().antiskills.is_empty());
    assert!(tree.skill(333).unwrap().antiskills.is_empty());
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
    // Numb Legs 2, Unflinching 4 << 4, Concentration 3 << 7, Second Wind 3 << 10, Sharp Eye 3 << 12.
    assert_eq!(modifiers[45], 15810.0);
    assert_eq!(modifiers[33], 0.0); // activation is filtered by native medicine kind
    assert!((modifiers[57] - 0.24).abs() < 1e-6);
    assert_eq!(modifiers[15], 1.0);
    assert_eq!(modifiers[64], 1.0);
}
