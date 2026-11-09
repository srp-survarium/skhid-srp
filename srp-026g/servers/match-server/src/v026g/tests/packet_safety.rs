//! Recipient-local recovery through the real transport and match state machine.
use super::{Bench, MAP_BATTERIES};
use crate::v026g::Settings;
use crate::v026g::scripted::WorldReceiptMode;
use srp_validate::recovery::{Outcome, Receipt};
use survarium::v026g::match_msgs::id;

#[test]
fn rejecting_the_first_world_recovers_without_entering_or_kicking_a_victim() {
    // With its receipts' copies too (0xE0 is plain and resent until acked):
    // one refusal is answered with one newer world.
    for copies in [false, true] {
        let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
        bench.clients[1].world_receipts = WorldReceiptMode::RejectNext;
        bench.clients[1].receipt_copies = copies;
        bench.join_all();
        assert!(bench.until(10_000, |b| b.stages() == "LLL"), "{}", bench.stages());
        assert_eq!(bench.clients[1].worlds_rejected, 1);
        // The refusal is counted (and logged) once.
        assert_eq!(bench.game().loads_refused(1), 1, "copies {copies}");
        assert_eq!(bench.game().generation(1), Some(1), "copies {copies}");
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
fn a_memory_report_is_logged_once_a_part_and_is_no_leave() {
    use srp_validate::memory::{Arena, Moment, Name, Process, Report as Memory, Usage};
    use srp_validate::report::Report;
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    assert!(bench.until(10_000, |b| b.stages() == "LLL"));
    let report = |moment| {
        let mut memory = Memory::new(moment, Process { free: 0x1000_0000, largest_free: 0x800_0000, video_budget: 1 << 30 });
        for n in 0..17u32 {
            memory.add(Arena { name: Name::of_c(b"render"), size: 0x1400_0000, usage: Usage::Heap { used: n, largest_free: 0x10_0000 } });
        }
        Report::Memory(memory)
    };
    let match_id = bench.clients[0].model.options.as_ref().unwrap().match_id;
    // Each part with its copies (0xE0 is resent until acked), both moments,
    // and a report of another match, which is not this one's to log.
    for _ in 0..3 {
        for report in [report(Moment::Loaded), report(Moment::Ended)] {
            for part in 0..report.parts() {
                let (bytes, len) = report.part(match_id, part).unwrap();
                bench.clients[0].send(id::LEAVE, &bytes[..len], true);
                let (other, len) = report.part(match_id ^ 1, part).unwrap();
                bench.clients[1].send(id::LEAVE, &other[..len], true);
            }
        }
    }
    bench.run(2_000);
    assert_eq!(bench.game().report_parts_heard(0), 4);
    assert_eq!(bench.game().report_parts_heard(1), 0);
    assert_eq!(bench.stages(), "LLL");
    assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
    bench.assert_no_faults();
}

#[test]
fn rejecting_a_resync_world_retries_only_that_recipient_and_then_play_continues() {
    for copies in [false, true] {
        let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
        bench.clients[2].world_receipts = WorldReceiptMode::Accept;
        bench.clients[2].receipt_copies = copies;
        bench.join_all(); bench.run(7_000);
        let before = bench.clients[2].model.syncs;
        let other_generations = [bench.game().generation(0), bench.game().generation(1)];
        let from = bench.clients[2].clock();
        bench.clients[2].sim.salt_from = Some((from, 7));
        bench.clients[2].world_receipts = WorldReceiptMode::RejectNext;
        assert!(bench.until(15_000, |b| b.clients[2].worlds_rejected == 1));
        assert!(bench.until(10_000, |b| b.stages() == "LLL" && b.clients[2].model.syncs > before));
        assert_eq!(bench.game().loads_refused(2), 1, "copies {copies}");
        assert_eq!([bench.game().generation(0), bench.game().generation(1)], other_generations);
        assert!(bench.clients[0].left().is_empty());
        assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
        bench.run(4_000);
        assert_eq!(bench.stages(), "LLL");
        bench.assert_no_faults();
    }
}

#[test]
fn every_refused_world_is_answered_once_with_a_newer_one_and_no_other_client_is_touched() {
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.clients[2].world_receipts = WorldReceiptMode::Accept;
    bench.clients[2].receipt_copies = true;
    bench.join_all(); bench.run(7_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    bench.clients[2].world_receipts = WorldReceiptMode::RejectAll;
    assert!(bench.until(15_000, |b| b.clients[2].worlds_rejected > 0));
    bench.run(3_000);
    // At once, with no wait between: each refusal heard is answered by the
    // next generation, its copy by nothing.
    let (refused, answered) = (bench.clients[2].worlds_rejected as u32, bench.game().loads_refused(2));
    assert!(refused >= 4 && answered + 1 >= refused && answered <= refused, "{refused} refused, {answered} answered");
    for client in &bench.clients[..2] { assert!(!client.closed && !client.disconnected); }
    bench.clients[2].world_receipts = WorldReceiptMode::Accept;
    assert!(bench.until(10_000, |b| b.stages() == "LLL"));
    assert_eq!(bench.game().loads_refused(2), bench.clients[2].worlds_rejected as u32);
    bench.assert_no_faults();
}

#[test]
fn corrupted_handshake_data_is_retried_on_only_its_own_connection() {
    // Each refusal is answered once, its copies not at all: a handshake
    // refused twice is sent twice more, and the game goes on.
    for (mode, refusals) in [
        (WorldReceiptMode::RejectOptionsNext, 1),
        (WorldReceiptMode::RejectPortsNext, 1),
        (WorldReceiptMode::RejectOptionsTwice, 2),
    ] {
        for copies in [false, true] {
            let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
            bench.clients[1].world_receipts = mode;
            bench.clients[1].receipt_copies = copies;
            bench.join_all();
            assert!(bench.until(10_000, |b| b.stages() == "LLL"), "{mode:?} copies {copies}: {}", bench.stages());
            assert_eq!(bench.clients[1].world_receipts, WorldReceiptMode::Accept);
            assert_eq!(bench.clients[1].numbered_refusals, refusals);
            assert_eq!(bench.game().loads_refused(1), u32::from(refusals), "{mode:?} copies {copies}");
            assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
            bench.assert_no_faults();
        }
    }
}

#[test]
fn a_first_world_its_game_cannot_read_is_answered_with_a_newer_one() {
    // A world refused without its generation (the game could not read it)
    // is a refusal of the world on its way, told from its copies by its
    // number: the entering game is not left waiting for the world it has.
    for copies in [false, true] {
        let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
        bench.clients[1].world_receipts = WorldReceiptMode::UnreadableNext;
        bench.clients[1].receipt_copies = copies;
        bench.join_all();
        assert!(bench.until(10_000, |b| b.stages() == "LLL"), "copies {copies}: {}", bench.stages());
        assert_eq!(bench.clients[1].worlds_rejected, 1);
        assert_eq!(bench.clients[1].numbered_refusals, 1);
        assert_eq!(bench.game().loads_refused(1), 1, "copies {copies}");
        assert_eq!(bench.game().generation(1), Some(1), "copies {copies}");
        assert!(bench.clients.iter().all(|c| !c.closed && !c.disconnected));
        bench.assert_no_faults();
    }
}
