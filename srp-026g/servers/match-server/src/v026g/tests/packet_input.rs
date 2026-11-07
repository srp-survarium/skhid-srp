use super::{Bench, MAP_BATTERIES};
use crate::v026g::Settings;
use survarium::v026g::match_msgs::{PlayerInput, WireTime};

#[test]
fn eight_distinct_bad_inputs_remove_the_sender_but_retransmissions_do_not() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all(); bench.run(4_000);
    for count in 0..8 {
        let bad = PlayerInput { time: WireTime::tick(bench.clients[0].clock() + 50), rotation: [0; 4], actions: 1 << 30 };
        for _ in 0..20 { bench.clients[0].send_input(bad); }
        bench.run(100);
        if count < 7 { assert!(!bench.clients[0].disconnected); }
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
