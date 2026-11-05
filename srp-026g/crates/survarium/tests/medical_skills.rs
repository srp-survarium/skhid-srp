//! Cross-check the content-built tree and the native 68-float wire payload.
use survarium::v026g::skills::{SkillError, SkillsTree};

#[test]
#[ignore = "requires SRP_MEDICAL_SKILLS_CONFIG from release/build-content"]
fn medical_tree_progression_and_native_payload() {
    let path = std::env::var("SRP_MEDICAL_SKILLS_CONFIG").expect("built skills_tree.options");
    let tree = SkillsTree::parse(&std::fs::read(path).unwrap()).unwrap();
    // physical-skills-1.json: the two unlearnable melee skills out, Packrat
    // in column 4 of its row, no two physical skills in one place.
    assert!(tree.skill(206).is_none() && tree.skill(210).is_none());
    let packrat = tree.skill(209).unwrap();
    assert_eq!(
        (packrat.column, packrat.row, packrat.branch_points),
        (4, 2, 5)
    );
    let mut physical = std::collections::HashSet::new();
    for skill in tree.skills.iter().filter(|skill| skill.branch() == 2) {
        assert!(physical.insert((skill.column, skill.row)), "{}", skill.id);
    }
    let ids = [301, 302, 311, 312, 321, 322, 331, 332, 341, 342];
    let shifts = [0, 3, 6, 9, 12, 15, 16, 17, 20, 21];
    let ranks = [5, 5, 5, 5, 5, 1, 1, 5, 1, 5];
    let requirements = [0, 0, 5, 5, 10, 20, 20, 20, 25, 10];
    assert_eq!(
        (
            tree.skill(342).unwrap().column,
            tree.skill(342).unwrap().row
        ),
        (3, 3)
    );
    assert_eq!(
        (
            tree.skill(322).unwrap().column,
            tree.skill(322).unwrap().row
        ),
        (4, 4)
    );
    let ten_points = [(301, 5), (302, 5), (342, 1)];
    tree.validate(100, &ten_points).unwrap();
    assert!(matches!(
        tree.validate(100, &[(301, 5), (302, 4), (342, 1)]),
        Err(SkillError::Locked { .. })
    ));
    assert!(matches!(
        tree.validate(100, &[(301, 5), (302, 5), (322, 1)]),
        Err(SkillError::Locked { .. })
    ));
    tree.validate(100, &[(301, 5), (302, 5), (311, 5), (312, 5), (322, 1)])
        .unwrap();
    let layout = [
        (301, 2, 1),
        (302, 3, 1),
        (311, 1, 2),
        (312, 4, 2),
        (321, 2, 3),
        (342, 3, 3),
        (331, 2, 4),
        (332, 3, 4),
        (322, 4, 4),
        (341, 2, 5),
    ];
    for (id, column, row) in layout {
        let skill = tree.skill(id).unwrap();
        assert_eq!((skill.column, skill.row), (column, row), "skill {id}");
    }
    let mut positions = std::collections::HashSet::new();
    for skill in tree.skills.iter().filter(|skill| skill.branch() == 3) {
        assert!(
            positions.insert((skill.column, skill.row)),
            "overlapping skill {}",
            skill.id
        );
    }
    let mut spent = Vec::new();
    for ((id, rank), required) in ids.into_iter().zip(ranks).zip(requirements) {
        let skill = tree.skill(id).unwrap();
        assert_eq!((skill.levels_amount, skill.branch_points), (rank, required));
        assert!(skill.unknown_modifiers.is_empty());
        if required > 0 {
            assert!(matches!(
                tree.validate(100, &[(id, 1)]),
                Err(SkillError::Locked { .. })
            ));
        }
        for n in 1..=rank {
            let mut preview = spent.clone();
            preview.push((id, n));
            tree.validate(100, &preview).unwrap();
        }
        spent.push((id, rank));
    }
    assert_eq!(spent.iter().map(|(_, n)| u32::from(*n)).sum::<u32>(), 38);
    let modifiers = tree.modifiers(&spent);
    let encoded: u32 = ranks
        .into_iter()
        .zip(shifts)
        .map(|(rank, shift)| u32::from(rank) << shift)
        .sum();
    assert_eq!(modifiers[43], encoded as f32);
    assert_eq!(modifiers[43] as u32, encoded);
    assert_eq!(modifiers[33], 0.0); // activation is filtered by native medicine kind
    assert!((modifiers[57] - 0.25).abs() < 1e-6);
    assert_eq!(modifiers[15], 1.0);
    assert_eq!(modifiers[64], 1.0);
    for old in [111, 113] {
        assert!(tree.skill(old).unwrap().antiskills.contains(&331));
        assert!(tree.skill(331).unwrap().antiskills.contains(&old));
    }
}
