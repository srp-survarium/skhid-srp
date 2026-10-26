//! Demo (branch `demo/bot-gear-levels`): the 15 gear-level kits as a
//! player's presets on the content's game data (`SRP_GAME_DATA`, with the
//! 0.29a3 weapons and the STK2 pieces): every kit is its level's own and
//! passes the lobby's Play check. Without `SRP_GAME_DATA` it checks
//! nothing; the unit test `presets::tests::
//! the_demo_puts_the_fifteen_gear_levels_on_the_presets` runs on the test
//! data.

use session::v026g::account::Account;
use session::v026g::{matchmaking, presets, rules};
use survarium::v026g::game_data;
use survarium::v026g::loadouts::{bot_carry_limit, resolved_level};

#[test]
fn every_gear_level_kit_plays_from_the_presets() {
    let Some(path) = std::env::var_os(game_data::GAME_DATA_ENV).filter(|path| !path.is_empty())
    else {
        eprintln!("SRP_GAME_DATA is unset: the content's kits are not checked");
        return;
    };
    game_data::init_from(path.as_ref()).expect("the server's game data");
    let now = 1_000;
    let mut account = Account::new("Alice");
    let given = presets::demo_gear_presets(&mut account, now).unwrap();
    println!("given {given:?}");
    for number in 1..=presets::PRESETS as u8 {
        presets::choose(&mut account, number, now).unwrap();
        for (k, kit) in account.kits.iter().enumerate() {
            let level = (number - 1) * 3 + k as u8 + 1;
            assert_eq!(resolved_level(level), level);
            rules::check_play(&account, kit.id, now)
                .unwrap_or_else(|why| panic!("level {level}: {why:?}"));
            let weight = account.kit_weight(kit);
            assert!(weight <= bot_carry_limit(), "level {level}: {weight} kg");
            println!(
                "preset {number} kit {}: gear level {level}, the lobby's {}, {weight:.1} kg",
                k + 1,
                matchmaking::kit_level(&account, kit)
            );
        }
    }
    assert!(!account.repair());
}
