//! Anti-AFK (`docs/notes/anti-afk.md`): a player without input for three
//! minutes of play is removed from the match, as a player who leaves.

use survarium::v026g::match_msgs::players::PlayerState;
use survarium::v026g::match_msgs::rules::state;

use super::{Bench, MAP_BATTERIES, session_of, setup};
use crate::v026g::Settings;
use crate::v026g::afk::Setting;

/// Three players, all alive in every world; `bots` places are test bots'.
fn bench_of(settings: Settings, private: bool, bots: &[usize]) -> Bench {
    let mut setup = setup(3, MAP_BATTERIES);
    setup.private = private;
    let mut bench = Bench::with_setup(setup, settings, bots, &[], false);
    alive(&mut bench, [true; 3]);
    bench.join_all();
    bench
}

/// Every simulating client's worlds have the players alive or dead.
fn alive(bench: &mut Bench, alive: [bool; 3]) {
    for client in &mut bench.clients {
        client.sim.player_states = alive
            .iter()
            .map(|&alive| {
                Some(PlayerState {
                    current_index: 0,
                    target_index: 0,
                    pickup: weapon_pickup::Record::EMPTY,
                    items: vec![1_000_000],
                    alive,
                    has_to_die: false,
                    look_pitch: 0.0,
                    rotation_y: 0.0,
                    actions_mask: 0,
                    effects: 0,
                })
            })
            .collect();
    }
}

/// Time passes until play begins (the countdown done): the bench's ms then.
fn until_play(bench: &mut Bench) -> u64 {
    assert!(bench.until(40_000, |bench| bench.game().rule_state()
        == state::IN_PROCESS));
    bench.ms
}

/// `ms` pass with the players at `active` turning their view and pressing
/// keys every two seconds.
fn play(bench: &mut Bench, ms: u64, active: &[usize]) {
    let end = bench.ms + ms;
    let mut keys = 0x1;
    while bench.ms < end {
        for &place in active {
            bench.clients[place].input(0, keys);
        }
        keys ^= 0x20;
        bench.run_long(2_000.min(end - bench.ms));
    }
}

/// The player at `place` was removed: its game told it left (0xEA with
/// its own index), the others that it left, and its seat freed.
fn removed(bench: &Bench, place: usize) -> bool {
    bench.clients[place].told_to_leave
        && bench
            .clients
            .iter()
            .enumerate()
            .filter(|(other, client)| *other != place && !client.closed)
            .all(|(_, client)| client.left().contains(&(place as u8)))
        && bench.registry.seat(session_of(place)).is_none()
}

#[test]
fn a_player_without_input_for_three_minutes_of_play_is_removed() {
    let mut bench = bench_of(Settings::default(), false, &[]);
    // The countdown does not count: 20 s and more passed since the join.
    let play_at = until_play(&mut bench);
    play(&mut bench, 175_000, &[1, 2]);
    assert!(!bench.clients[0].told_to_leave);
    play(&mut bench, 15_000, &[1, 2]);
    assert!(removed(&bench, 0), "{}", bench.stages());
    assert!(bench.ms - play_at <= 190_000);
    // The others play on, the match with them.
    assert!(!bench.clients[1].told_to_leave && !bench.clients[2].told_to_leave);
    play(&mut bench, 200_000, &[1, 2]);
    assert!(!bench.clients[1].told_to_leave && !bench.clients[2].told_to_leave);
    assert!(bench.results.try_recv().is_err());
    bench.assert_no_faults();
}

#[test]
fn an_input_starts_the_three_minutes_again() {
    let mut bench = bench_of(Settings::default(), false, &[]);
    until_play(&mut bench);
    play(&mut bench, 120_000, &[1, 2]);
    // One turn of the view.
    play(&mut bench, 2_000, &[0, 1, 2]);
    play(&mut bench, 170_000, &[1, 2]);
    assert!(!bench.clients[0].told_to_leave);
    play(&mut bench, 20_000, &[1, 2]);
    assert!(removed(&bench, 0));
}

#[test]
fn time_dead_does_not_count() {
    let mut bench = bench_of(Settings::default(), false, &[]);
    until_play(&mut bench);
    // Player 0 is dead in every world from here on, and never respawns.
    alive(&mut bench, [false, true, true]);
    play(&mut bench, 400_000, &[1, 2]);
    assert!(!bench.clients[0].told_to_leave);
}

#[test]
fn time_its_game_is_not_heard_does_not_count() {
    let mut bench = bench_of(Settings::default(), false, &[]);
    let play_at = until_play(&mut bench);
    play(&mut bench, 100_000, &[1, 2]);
    // Its game shows no running simulation for two minutes (no hash
    // report, no world): a hitch, a resync, a PC that froze. The others
    // vouch for the worlds all the while.
    bench.clients[0].no_hashes = true;
    bench.clients[0].no_uploads = true;
    play(&mut bench, 120_000, &[1, 2]);
    bench.clients[0].no_hashes = false;
    bench.clients[0].no_uploads = false;
    // 100 s counted before: three minutes are reached 80 s on, not at once.
    play(&mut bench, 60_000, &[1, 2]);
    assert!(!bench.clients[0].told_to_leave);
    play(&mut bench, 40_000, &[1, 2]);
    assert!(removed(&bench, 0));
    assert!(bench.ms - play_at >= 300_000);
}

#[test]
fn a_private_rooms_match_removes_nobody() {
    let mut bench = bench_of(Settings::default(), true, &[]);
    until_play(&mut bench);
    play(&mut bench, 400_000, &[]);
    assert!(bench.clients.iter().all(|client| !client.told_to_leave));
}

#[test]
fn the_setting_off_removes_nobody() {
    let settings = Settings {
        anti_afk: Setting::Off,
        ..Settings::default()
    };
    let mut bench = bench_of(settings, false, &[]);
    until_play(&mut bench);
    play(&mut bench, 400_000, &[]);
    assert!(bench.clients.iter().all(|client| !client.told_to_leave));
}

#[test]
fn a_bot_is_never_removed() {
    // Place 2 is a test bot's: it never gives an input.
    let mut bench = bench_of(Settings::default(), false, &[2]);
    until_play(&mut bench);
    play(&mut bench, 400_000, &[0, 1]);
    assert!(bench.clients.iter().all(|client| !client.told_to_leave));
    assert!(bench.registry.seat(session_of(2)).is_some());
}

#[test]
fn a_match_against_bots_removes_nobody() {
    // The owner's of 2026-11-02: "it should also be fine to afk w/bots".
    let mut bench = Bench::vs_bots(3, MAP_BATTERIES, Settings::default(), &[1, 2]);
    alive(&mut bench, [true; 3]);
    bench.join(0);
    until_play(&mut bench);
    play(&mut bench, 400_000, &[]);
    assert!(!bench.clients[0].told_to_leave);
    assert!(bench.registry.seat(session_of(0)).is_some());
    assert!(bench.results.try_recv().is_err());
}
