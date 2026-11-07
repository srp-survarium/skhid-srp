use super::{Bench, MAP_BATTERIES};
use crate::v026g::game::input_safety::STRIKE_LIMIT;
use crate::v026g::Settings;
use survarium::v026g::match_msgs::{PlayerInput, WireTime};

#[test]
fn distinct_bad_inputs_up_to_the_limit_remove_the_sender_but_retransmissions_do_not() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all(); bench.run(4_000);
    for count in 0..STRIKE_LIMIT {
        let bad = PlayerInput { time: WireTime::tick(bench.clients[0].clock() + 50), rotation: [0; 4], actions: 1 << 30 };
        for _ in 0..20 { bench.clients[0].send_input(bad); }
        bench.run(100);
        if count + 1 < STRIKE_LIMIT { assert!(!bench.clients[0].disconnected); }
        for other in [1, 2] {
            assert!(!bench.clients[other].closed && !bench.clients[other].disconnected);
            assert!(bench.clients[other].relays_of(0).is_empty());
        }
    }
    bench.run(500);
    assert!(bench.clients[0].disconnected);
    assert_eq!(bench.stages(), "gLL");
    bench.assert_no_faults();
}

/// Live, 2026-11-02 (match 0x30e6dd21, player 10): its world went wrong,
/// and while the server found it out of step and resynced it, it sent 11
/// inputs with NaN rotations (`[00 fe 00 fe]`, actions 0). Discarded, they
/// count for nothing: it loads its world and plays on.
#[test]
fn nan_inputs_of_a_world_out_of_step_are_discarded_and_its_player_is_resynced_not_removed() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run(7_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(bench.until(10_000, |bench| bench.game().out_of_step(2)));
    assert_eq!(bench.game().resyncs(2), 0);
    // One every tick until its resync's world is loaded.
    let mut sent = 0;
    while bench.clients[2].model.syncs < 2 && sent < 200 {
        let nan = nan_input(bench.clients[2].clock() + 50);
        bench.clients[2].send_input(nan);
        sent += 1;
        bench.run(50);
    }
    assert_eq!(bench.clients[2].model.syncs, 2);
    assert_eq!(bench.game().resyncs(2), 1);
    assert!(sent >= 11, "{sent}");
    assert!(bench.until(2_000, |bench| bench.stages() == "LLL"));
    assert!(!bench.clients[2].disconnected && !bench.clients[2].closed);
    assert_eq!(bench.game().inputs_refused(2), sent);
    assert!(!bench.game().out_of_step(2));
    for other in [0, 1] {
        assert!(bench.clients[other].relays_of(2).is_empty());
    }
    // It plays on.
    let good = bench.clients[2].input(50, 0x21);
    bench.run(200);
    for other in [0, 1] {
        assert_eq!(bench.clients[other].relays_of(2), [good]);
    }
    bench.assert_no_faults();
}

/// A game in step that sends the same NaN rotations is removed as before.
#[test]
fn nan_inputs_of_a_world_in_step_remove_their_sender() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run(7_000);
    for count in 0..STRIKE_LIMIT {
        assert!(!bench.game().out_of_step(0));
        let nan = nan_input(bench.clients[0].clock() + 50);
        bench.clients[0].send_input(nan);
        bench.run(50);
        if count + 1 < STRIKE_LIMIT {
            assert!(!bench.clients[0].disconnected);
        }
    }
    bench.run(500);
    assert!(bench.clients[0].disconnected);
    assert_eq!(bench.stages(), "gLL");
    for other in [1, 2] {
        assert!(bench.clients[other].relays_of(0).is_empty());
    }
    bench.assert_no_faults();
}

fn nan_input(time: u32) -> PlayerInput {
    PlayerInput { time: WireTime::tick(time), rotation: [0x00, 0xFE, 0x00, 0xFE], actions: 0 }
}
