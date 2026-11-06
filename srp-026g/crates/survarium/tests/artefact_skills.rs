//! The content-built artefact tree (artefact-skills.md): its layout, thresholds and what a kit
//! gets in its 68 modifiers.
use survarium::v026g::skills::{SkillError, SkillsTree, modifier_index};

/// `(id, ranks, branch_points, column, row)`.
#[rustfmt::skip]
const TREE: [(u16, u8, u32, u8, u8); 11] = [
    (501, 5, 0, 2, 1), (502, 5, 0, 3, 1),
    (511, 4, 5, 2, 2), (512, 5, 5, 3, 2),
    (521, 1, 10, 2, 3), (522, 1, 10, 3, 3),
    (531, 1, 20, 1, 4), (532, 1, 20, 2, 4), (533, 1, 20, 3, 4),
    (541, 1, 24, 2, 5), (542, 1, 24, 3, 5),
];

/// Resonance and Cascade, the row-V pair: one or the other.
const RESONANCE: u16 = 541;
const CASCADE: u16 = 542;

/// Every rank of rows I..IV and one of the row-V pair.
fn full(pick: u16) -> Vec<(u16, u8)> {
    TREE.iter().filter(|&&(id, ..)| id < 540 || id == pick).map(|&(id, ranks, ..)| (id, ranks)).collect()
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
        let excludes: &[u16] = match id {
            RESONANCE => &[CASCADE],
            CASCADE => &[RESONANCE],
            _ => &[],
        };
        assert!(skill.unknown_modifiers.is_empty() && skill.antiskills == excludes, "{id}");
        assert!(places.insert((column, row)), "two skills at {id}'s place");
    }
    // 25 points, every row reached; a full build is valid at 25, not at 24, with either of the
    // row-V pair, never with both.
    for pick in [RESONANCE, CASCADE] {
        tree.validate(100, &full(pick)).unwrap();
        tree.validate(25, &full(pick)).unwrap();
        assert!(matches!(tree.validate(24, &full(pick)), Err(SkillError::OverLevel { .. })));
    }
    let mut both = full(RESONANCE);
    both.push((CASCADE, 1));
    assert!(matches!(tree.validate(100, &both), Err(SkillError::Exclusive(..))));
    // Row II at 5, row III at 10, row IV at 20 (rows I-III hold 21), row V at 24 (all of I-IV).
    tree.validate(100, &[(501, 5), (511, 1)]).unwrap();
    assert!(matches!(tree.validate(100, &[(501, 4), (511, 1)]), Err(SkillError::Locked { skill: 511, .. })));
    tree.validate(100, &[(501, 5), (502, 3), (512, 2), (521, 1)]).unwrap();
    assert!(matches!(
        tree.validate(100, &[(501, 5), (502, 3), (512, 1), (521, 1)]),
        Err(SkillError::Locked { skill: 521, .. })
    ));
    let three: Vec<(u16, u8)> = full(RESONANCE).into_iter().filter(|&(id, _)| id < 530).collect();
    let mut four = three.clone();
    four.push((531, 1));
    tree.validate(100, &four).unwrap();
    let mut short = four.clone();
    short[0].1 = 3; // Quick Hands 3: rows I-III hold 19
    assert!(matches!(tree.validate(100, &short), Err(SkillError::Locked { skill: 531, .. })));
    let mut nearly = four.clone();
    nearly[0].1 = 4; // rows I-III hold 20, still row IV
    tree.validate(100, &nearly).unwrap();
    // Row V needs every point of rows I-IV: one short of any is locked, for either of the pair.
    for pick in [RESONANCE, CASCADE] {
        let build = full(pick);
        for at in (0..build.len()).filter(|&at| build[at].0 < 540) {
            let mut less = build.clone();
            less[at].1 -= 1;
            assert!(
                matches!(tree.validate(100, &less), Err(SkillError::Locked { skill, .. }) if skill == pick),
                "{} one short",
                less[at].0
            );
        }
    }
    // The modifiers of a full build: the data skills' native ones and the packed integer.
    let m = tree.modifiers(&full(RESONANCE));
    let at = |name: &str| m[modifier_index(name).unwrap()];
    assert!((at("artefact_search_time") + 0.25).abs() < 1e-6);
    assert!((at("incoming_weapon_damage_in_anomaly") + 0.2).abs() < 1e-6);
    // Artifact Container: an artefact slot for a pocket.
    assert_eq!((at("artefact_slots"), at("pocket_slots")), (1.0, -1.0));
    assert!((at("artefact_duration") - 0.25).abs() < 1e-6);
    // Resistance 5, then bits 3..7 (Keeper, Resonance, Spring II, Onyx II, Larkspur II); with
    // Cascade, bit 8 instead of bit 4.
    let power = modifier_index("artefact_power").unwrap();
    assert_eq!(at("artefact_power"), 253.0);
    assert_eq!(tree.modifiers(&full(CASCADE))[power], 493.0);
    for (id, bits) in [(502, 1.0), (521, 8.0), (RESONANCE, 16.0), (531, 32.0), (532, 64.0), (533, 128.0), (CASCADE, 256.0)] {
        assert_eq!(tree.modifiers(&[(id, 1)])[power], bits, "{id}");
    }
    // Cascade is also the retained Medic (modifier 43 bit 20): a Larkspur's activation heals the
    // teammates within 5 m. No other skill of the tree sets 43.
    let armored = modifier_index("armored_trap").unwrap();
    assert_eq!(tree.modifiers(&[(CASCADE, 1)])[armored], 1_048_576.0);
    assert_eq!(tree.modifiers(&full(CASCADE))[armored], 1_048_576.0);
    assert_eq!(at("armored_trap"), 0.0);
    let only_data = tree.modifiers(&[(501, 5), (511, 4), (512, 5), (522, 1)]);
    assert_eq!(only_data[power], 0.0);
    // A saved set past its levels or below its rows is settled (refunded), never more.
    assert_eq!(tree.settle(100, &[(501, 9)]), vec![(501, 5)]);
    assert_eq!(tree.settle(100, &[(501, 2), (531, 1)]), vec![(501, 2)]);
}
