//! The bots' 15 gear-level kits (demo branch `demo/bot-gear-levels`) on
//! the content's game data, where every level is its own: the server's
//! game data `SRP_GAME_DATA` names (with the 0.29a3 weapons and the STK2
//! pieces). Without it the test says so and checks nothing; the unit test
//! `loadouts::tests::every_gear_level_kit_passes_the_kit_rules` checks the
//! same on the test data.
//!
//! `SRP_GAME_DATA=<server-game-data.db> cargo test -p survarium --test
//! bot_gear_levels -- --nocapture` also prints the table.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::item_params;
use survarium::v026g::loadouts::{
    self, BotRole, GEAR_LEVELS, Loadout, bot_carry_limit, bot_kits, bot_modifiers, bot_role_of,
    kit_problems, player_gear_kit_items, resolved_level,
};
use survarium::v026g::maps;
use survarium::v026g::match_msgs::static_info::{
    MatchOptions, MatchProfile, SCORE_WEIGHTS, StaticInfo, parse_payload,
};

#[test]
fn every_gear_level_kit_is_its_own_and_passes_the_kit_rules() {
    let Some(path) = std::env::var_os(game_data::GAME_DATA_ENV).filter(|path| !path.is_empty())
    else {
        eprintln!("SRP_GAME_DATA is unset: the content's kits are not checked");
        return;
    };
    game_data::init_from(path.as_ref()).expect("the server's game data");
    let dictionary = Dictionary::shared();
    let name = |dict_id: u16| dictionary.item(dict_id).unwrap().name.as_str();
    let level_of = |dict_id: u16| item_params::item_params(dict_id).unwrap().item_level;

    let mut profiles = Vec::new();
    for level in 1..=GEAR_LEVELS {
        assert_eq!(resolved_level(level), level, "the content has every item");
        let armour = bot_kits(BotRole::Flanker, level)[0]
            .items
            .iter()
            .filter(|item| item.slot <= 6)
            .map(|item| format!("{} ({})", name(item.dict_id), level_of(item.dict_id)))
            .collect::<Vec<_>>()
            .join(", ");
        println!("level {level}: armour {armour}");
        for role in BotRole::ALL {
            let kits = bot_kits(role, level);
            for (k, kit) in kits.iter().enumerate() {
                let problems = kit_problems(kit, bot_carry_limit());
                assert!(
                    problems.is_empty(),
                    "level {level} {role} kit {k}: {problems:?}"
                );
            }
            let [first, _] = kits;
            let weapon = first.slot(7).unwrap();
            let ammo = first.slot(8).unwrap();
            let pockets: Vec<String> = first
                .items
                .iter()
                .filter(|item| loadouts::POCKET_SLOTS.contains(&item.slot))
                .map(|item| format!("{} x{}", name(item.dict_id), item.count))
                .collect();
            println!(
                "  {role:8} {} ({}), {} x{} x3, pockets [{}], {:.1} kg",
                name(weapon.dict_id),
                level_of(weapon.dict_id),
                name(ammo.dict_id),
                ammo.count,
                pockets.join(", "),
                first.weight()
            );
            let profile = MatchProfile::from_loadouts(
                &format!("Bot-{level}"),
                (profiles.len() % 2) as u8,
                &kits,
                bot_modifiers(),
                dictionary,
                1,
            );
            assert_eq!(bot_role_of(&profile), Some(role));
            profiles.push(profile);
        }
        // The player's kit of the level (the demo presets).
        let items = player_gear_kit_items(level).unwrap();
        let mut player =
            Loadout::from_names(dictionary, items.iter().map(|item| (item.slot, item.name)))
                .unwrap();
        for (slot, item) in player.items.iter_mut().zip(&items) {
            slot.count = item.count;
        }
        let problems = kit_problems(&player, bot_carry_limit());
        assert!(problems.is_empty(), "level {level} player: {problems:?}");
        println!(
            "  player   {} x{}, {:.1} kg",
            name(player.slot(7).unwrap().dict_id),
            player.slot(8).unwrap().count,
            player.weight()
        );
    }
    // A client reads every kit (twenty profiles a match: in threes).
    for chunk in profiles.chunks(20) {
        let options = MatchOptions {
            players: chunk.to_vec(),
            map_id: maps::playable().next().unwrap().id,
            match_id: 0x1234,
            match_level: 1,
            score_weights: SCORE_WEIGHTS,
        };
        let info = StaticInfo::new(&options, dictionary).expect("a client can hold it");
        let (_, read) = parse_payload(&info.payload(0).unwrap(), dictionary).unwrap();
        for (read, sent) in read.players.iter().zip(chunk) {
            assert_eq!(read.kits, sent.kits);
            assert_eq!(read.items, sent.items);
        }
    }
}
