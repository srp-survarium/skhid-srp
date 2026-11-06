//! The content-built artefact tree (artefact-skills.md): its layout, thresholds and what a kit
//! gets in its 68 modifiers.
use survarium::v026g::skills::{SkillError, SkillsTree, modifier_index};

/// `(id, ranks, branch_points, column, row)`.
#[rustfmt::skip]
const TREE: [(u16, u8, u32, u8, u8); 11] = [
    (501, 5, 0, 2, 1), (502, 5, 0, 3, 1),
    (511, 4, 5, 2, 2), (512, 5, 5, 3, 2),
    (521, 1, 10, 2, 3), (522, 1, 10, 3, 3),
    (531, 1, 20, 1, 4), (532, 1, 20, 2, 4), (533, 1, 20, 3, 4), (534, 1, 20, 4, 4),
    (541, 1, 25, 3, 5),
];

fn full() -> Vec<(u16, u8)> {
    TREE.iter().map(|&(id, ranks, ..)| (id, ranks)).collect()
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
        assert!(skill.unknown_modifiers.is_empty() && skill.antiskills.is_empty(), "{id}");
        assert!(places.insert((column, row)), "two skills at {id}'s place");
    }
    // 26 points, every row reached; a full build is valid at 26, not at 25.
    tree.validate(100, &full()).unwrap();
    tree.validate(26, &full()).unwrap();
    assert!(matches!(tree.validate(25, &full()), Err(SkillError::OverLevel { .. })));
    // Tier 2 at 5, tier 3 at 10, tier 4 at 20 (tiers 1-3 hold 21), tier 5 at 25 (all of 1-4).
    tree.validate(100, &[(501, 5), (511, 1)]).unwrap();
    assert!(matches!(tree.validate(100, &[(501, 4), (511, 1)]), Err(SkillError::Locked { skill: 511, .. })));
    tree.validate(100, &[(501, 5), (502, 3), (512, 2), (521, 1)]).unwrap();
    assert!(matches!(
        tree.validate(100, &[(501, 5), (502, 3), (512, 1), (521, 1)]),
        Err(SkillError::Locked { skill: 521, .. })
    ));
    let three: Vec<(u16, u8)> = full().into_iter().filter(|&(id, _)| id < 530).collect();
    let mut four = three.clone();
    four.push((531, 1));
    tree.validate(100, &four).unwrap();
    let mut short = four.clone();
    short[0].1 = 3; // Concentration 3: tiers 1-3 hold 19
    assert!(matches!(tree.validate(100, &short), Err(SkillError::Locked { skill: 531, .. })));
    let mut nearly = four.clone();
    nearly[0].1 = 4; // tiers 1-3 hold 20, still tier 4
    tree.validate(100, &nearly).unwrap();
    // Tier 5 needs every point of tiers 1-4: one short of any is locked.
    let cascade: Vec<(u16, u8)> = full();
    for at in (0..cascade.len()).filter(|&at| cascade[at].0 < 540) {
        let mut less = cascade.clone();
        less[at].1 -= 1;
        assert!(
            matches!(tree.validate(100, &less), Err(SkillError::Locked { skill: 541, .. })),
            "{} one short",
            less[at].0
        );
    }
    // The modifiers of a full build: the data skills' native ones and the packed integer.
    let m = tree.modifiers(&full());
    let at = |name: &str| m[modifier_index(name).unwrap()];
    assert!((at("artefact_search_time") + 0.25).abs() < 1e-6);
    assert!((at("incoming_weapon_damage_in_anomaly") + 0.2).abs() < 1e-6);
    // Artifact Container: an artefact slot for a pocket.
    assert_eq!((at("artefact_slots"), at("pocket_slots")), (1.0, -1.0));
    assert!((at("artefact_duration") - 0.25).abs() < 1e-6);
    // Resistance 5, then bits 3..8 (Keeper, Resonance, Spring II, Onyx II, Larkspur II, Cascade).
    assert_eq!(at("artefact_power"), 509.0);
    for (id, bits) in [(502, 1.0), (521, 8.0), (522, 16.0), (531, 32.0), (532, 64.0), (533, 128.0), (541, 256.0)] {
        assert_eq!(tree.modifiers(&[(id, 1)])[modifier_index("artefact_power").unwrap()], bits, "{id}");
    }
    // Resonance Cascade is also the retained Medic (modifier 43 bit 20): a Larkspur's activation
    // heals the teammates within 5 m. No other skill of the tree sets 43.
    let armored = modifier_index("armored_trap").unwrap();
    assert_eq!(tree.modifiers(&[(541, 1)])[armored], 1_048_576.0);
    assert_eq!(at("armored_trap"), 1_048_576.0);
    let only_data = tree.modifiers(&[(501, 5), (511, 4), (512, 5), (534, 1)]);
    assert_eq!(only_data[modifier_index("artefact_power").unwrap()], 0.0);
    // A saved set past its levels or below its rows is settled (refunded), never more.
    assert_eq!(tree.settle(100, &[(501, 9)]), vec![(501, 5)]);
    assert_eq!(tree.settle(100, &[(501, 2), (531, 1)]), vec![(501, 2)]);
}
