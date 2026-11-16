//! The Pilgrim container gives its wearer Resonance (backpack-bonuses-1.json, artefact-skills.md):
//! on the game data a release builds, a kit wearing it has modifier 40's Resonance bit, set beside
//! the kit's skills, never added.
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::loadouts::{Loadout, SlotItem, give_worn_skills};
use survarium::v026g::match_msgs::static_info::MatchProfile;
use survarium::v026g::skills::{SkillsTree, modifier_index};

#[test]
#[ignore = "requires SRP_GAME_DATA: a server-game-data.db that release/build-content made with backpack-bonuses-1.json"]
fn a_pilgrim_wearer_has_resonance_beside_its_skills() {
    let path = std::env::var_os(game_data::GAME_DATA_ENV).expect("SRP_GAME_DATA");
    game_data::init_from(std::path::Path::new(&path)).unwrap();
    let dictionary = Dictionary::shared();
    let pilgrim = dictionary
        .items
        .iter()
        .find(|item| item.cfg_name == "gameplay/items/armour/back/edge_back_6.options")
        .expect("the Pilgrim container");
    let power = modifier_index("artefact_power").unwrap();
    let tree = SkillsTree::shared();
    let worn = |back: bool, skills: &[(u16, u8)]| {
        let items = back.then_some(SlotItem {
            slot: 3,
            dict_id: pilgrim.dict_id,
            count: 1,
        });
        let loadout = Loadout {
            items: items.into_iter().collect(),
        };
        let mut profile =
            MatchProfile::from_loadout("Alice", 0, &loadout, tree.modifiers(skills), dictionary, 1);
        give_worn_skills(&mut profile);
        profile.kits[0].modifiers[power]
    };
    assert_eq!(worn(true, &[]), 16.0);
    // Resonance already: nothing more (16 + 16 would be Spring II's 32).
    assert_eq!(worn(true, &[(541, 1)]), 16.0);
    // Cascade (256) and Resonance both (the owner, 2026-11-16).
    assert_eq!(worn(true, &[(542, 1)]), 272.0);
    // Resistance 5, Keeper, Spring II: the bits beside them.
    assert_eq!(worn(true, &[(502, 5), (521, 1), (531, 1)]), 61.0);
    assert_eq!(worn(false, &[]), 0.0);
    assert_eq!(worn(false, &[(542, 1)]), 256.0);
}
