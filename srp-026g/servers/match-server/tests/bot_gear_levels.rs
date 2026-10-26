//! Demo (branch `demo/bot-gear-levels`): every kit of the bots' 15 gear
//! levels has its arms on the content's game data (`SRP_GAME_DATA`, with
//! the 0.29a3 weapons): the weapon's facts from its config, its magazine
//! the game's. Without `SRP_GAME_DATA` it checks nothing (the unit test
//! `bots::guns::tests` runs on the test data).

use match_server::v026g::bots::guns::Arms;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::item_params::item_params;
use survarium::v026g::loadouts::{BOT_AMMO_SLOTS, BotRole, GEAR_LEVELS, bot_kits};

#[test]
fn every_gear_level_weapon_has_its_arms() {
    let Some(path) = std::env::var_os(game_data::GAME_DATA_ENV).filter(|path| !path.is_empty())
    else {
        eprintln!("SRP_GAME_DATA is unset: the content's weapons are not checked");
        return;
    };
    game_data::init_from(path.as_ref()).expect("the server's game data");
    let dictionary = Dictionary::shared();
    for level in 1..=GEAR_LEVELS {
        for role in BotRole::ALL {
            for kit in bot_kits(role, level) {
                let weapon = kit.slot(7).unwrap();
                let name = &dictionary.item(weapon.dict_id).unwrap().name;
                let rounds = BOT_AMMO_SLOTS
                    .iter()
                    .filter_map(|&slot| kit.slot(slot))
                    .map(|item| item.count)
                    .sum();
                let arms = Arms::of_kit(name, rounds)
                    .unwrap_or_else(|| panic!("level {level} {role}: {name}"));
                let gun = arms.primary.0;
                assert_eq!(
                    gun.magazine,
                    item_params(weapon.dict_id).unwrap().magazine_capacity,
                    "{name}"
                );
                println!("level {level} {role}: {gun:?} x{rounds}");
            }
        }
    }
}
