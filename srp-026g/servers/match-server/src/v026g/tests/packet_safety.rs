//! Recipient-local recovery through the real transport and match state machine.
use super::{Bench, MAP_BATTERIES};
use crate::v026g::Settings;
use crate::v026g::scripted::WorldReceiptMode;
use srp_validate::recovery::{Outcome, Receipt};
use survarium::v026g::match_msgs::id;

#[test]
fn rejecting_the_first_world_recovers_without_entering_or_kicking_a_victim() {
    for mode in [WorldReceiptMode::RejectNext, WorldReceiptMode::DropNext] {
        let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
        bench.clients[1].world_receipts = mode;
        bench.join_all();
        assert!(bench.until(10_000, |b| b.stages() == "LLL"), "{}", bench.stages());
        assert_eq!(bench.clients[1].worlds_rejected, 1);
        // A refusal the server heard of is counted (and logged); a lost receipt is retried.
        let heard = u32::from(mode == WorldReceiptMode::RejectNext);
        assert_eq!(bench.game().loads_refused(1), heard);
        assert!(bench.clients[1].model.last_resync.is_some());
        assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
        bench.assert_no_faults();
    }
}

#[test]
fn receipts_cannot_accuse_other_players_or_restart_a_settled_generation() {
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.clients[0].world_receipts = WorldReceiptMode::Accept;
    bench.join_all();
    assert!(bench.until(10_000, |b| b.stages() == "LLL"));
    let generations = (0..3).map(|p| bench.game().generation(p)).collect::<Vec<_>>();
    for generation in [generations[0].unwrap(), generations[0].unwrap().wrapping_sub(1), 128] {
        let receipt = Receipt { match_id: bench.clients[0].model.options.as_ref().unwrap().match_id, generation, outcome: Outcome::Rejected };
        for _ in 0..100 { bench.clients[0].send(id::LEAVE, &receipt.encode(), true); }
    }
    bench.run(2_000);
    assert_eq!((0..3).map(|p| bench.game().generation(p)).collect::<Vec<_>>(), generations);
    assert_eq!(bench.stages(), "LLL");
    // Ignored (and logged), never counted as a refused load.
    assert_eq!(bench.game().loads_refused(0), 0);
    assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
    bench.assert_no_faults();
}


#[test]
fn rejecting_a_resync_world_retries_only_that_recipient_and_then_play_continues() {
    for mode in [WorldReceiptMode::RejectNext, WorldReceiptMode::DropNext] {
        let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
        bench.clients[2].world_receipts = WorldReceiptMode::Accept;
        bench.join_all(); bench.run(7_000);
        let before = bench.clients[2].model.syncs;
        let other_generations = [bench.game().generation(0), bench.game().generation(1)];
        let from = bench.clients[2].clock();
        bench.clients[2].sim.salt_from = Some((from, 7));
        bench.clients[2].world_receipts = mode;
        assert!(bench.until(15_000, |b| b.clients[2].worlds_rejected == 1));
        assert!(bench.until(10_000, |b| b.stages() == "LLL" && b.clients[2].model.syncs > before));
        assert_eq!([bench.game().generation(0), bench.game().generation(1)], other_generations);
        assert!(bench.clients[0].left().is_empty());
        assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
        bench.run(4_000);
        assert_eq!(bench.stages(), "LLL");
        bench.assert_no_faults();
    }
}

#[test]
fn repeated_rejection_has_bounded_retries_and_cannot_kick_other_clients() {
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.clients[2].world_receipts = WorldReceiptMode::Accept;
    bench.join_all(); bench.run(7_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    bench.clients[2].world_receipts = WorldReceiptMode::RejectAll;
    assert!(bench.until(15_000, |b| b.clients[2].worlds_rejected > 0));
    let before = bench.clients[2].worlds_rejected;
    bench.run(5_000);
    assert!(bench.clients[2].worlds_rejected - before <= 6);
    for client in &bench.clients[..2] { assert!(!client.closed && !client.disconnected); }
    bench.clients[2].world_receipts = WorldReceiptMode::Accept;
    assert!(bench.until(10_000, |b| b.stages() == "LLL"));
    bench.assert_no_faults();
}

#[test]
fn corrupted_handshake_data_is_retried_on_only_its_own_connection() {
    for mode in [WorldReceiptMode::RejectOptionsNext, WorldReceiptMode::RejectPortsNext] {
        let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
        bench.clients[1].world_receipts = mode;
        bench.join_all();
        assert!(bench.until(10_000, |b| b.stages() == "LLL"));
        assert_eq!(bench.clients[1].world_receipts, WorldReceiptMode::Accept);
        assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
        bench.assert_no_faults();
    }
}
