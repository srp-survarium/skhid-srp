//! What a player counts for when the sides of a match are made: its K/D
//! at the match's equipment level (`session::v026g::level_stats`,
//! `docs/notes/kd-balance.md`).

use super::*;
use session::Strength;
use session::v026g::balance::Weights;
use session::v026g::level_stats::{self, KillsDeaths, LevelStats};
use session::v026g::matchmaking;

/// Five players press Play, which puts them into one room, and get its
/// match; the side of each, by client.
fn sides_of_five(world: &mut World, players: &[usize]) -> Vec<u8> {
    for &player in players {
        world.ask(player, request::play(1));
    }
    world.advance(PLAY_TO_MATCH);
    players
        .iter()
        .map(|&player| sent_to_match(&world.take(player)).expect("a match").1)
        .collect()
}

fn five(world: &mut World) -> Vec<usize> {
    ["Alice", "Bob", "Carol", "Dave", "Erin"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect()
}

/// The same kills and deaths at every equipment level.
fn everywhere(row: KillsDeaths) -> LevelStats {
    LevelStats::from_rows((0..=15).map(|level| (level, row)))
}

/// The owner's of 2026-11-05: the two strongest go one against another,
/// even where together on the side of fewer would be the most even split.
#[test]
fn the_two_strongest_players_go_one_against_another() {
    let settings = Settings::local("127.0.0.1:0");
    assert_eq!(settings.balance, Weights::default());

    let mut world = World::new();
    let players = five(&mut world);
    // Carol and Erin kill two for each death, over many: each counts for
    // 2, the others (new) for 1, and a player for 1 more. Together on the
    // side of fewer they would be 2 + 4 against 3 + 3; apart they are.
    for strong in [players[2], players[4]] {
        world.change(strong, |account| {
            account.level_stats = everywhere(KillsDeaths {
                kills: 2000,
                deaths: 1000,
            });
        });
    }
    let sides = sides_of_five(&mut world, &players);
    assert_ne!(sides[2], sides[4], "{sides:?}");
    world.assert_clean();
}

/// The owner's of 2026-11-01: the match's equipment level is the highest
/// gear level of its players, and each counts for its K/D there; a new
/// player for 1.
#[test]
fn each_counts_for_its_strength_at_the_highest_gear_level_of_the_match() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    // Alice has played much at her own gear level, and nothing elsewhere.
    let gear = matchmaking::gear_level(&world.account(alice), session::v026g::quests::now());
    world.change(alice, |account| {
        account.level_stats = LevelStats::from_rows([(
            gear,
            KillsDeaths {
                kills: 300,
                deaths: 100,
            },
        )]);
    });
    let (ticket, _) = world.match_of(alice, bob);
    assert_eq!(world.results.match_gear(ticket), Some(gear));
    let running = world
        .core
        .matches
        .values()
        .find(|running| running.ticket.match_id == ticket)
        .unwrap();
    let [of_alice, of_bob] = [alice, bob].map(|player| {
        let place = running
            .seats
            .iter()
            .position(|(session_id, _)| *session_id == world.ids[player].1)
            .unwrap();
        running.strengths[place]
    });
    assert_eq!(of_alice, world.account(alice).level_stats.kd_at(gear));
    assert!(of_alice > Strength::from_milli(2800), "{of_alice:?}");
    assert_eq!(of_bob, level_stats::DEFAULT_KD);
}
