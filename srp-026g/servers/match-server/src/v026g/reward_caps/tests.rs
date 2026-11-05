use super::*;
use survarium::v026g::match_msgs::snapshot::{PlayerStatistics, event};

fn feeder() -> (Statistics, kill_cap::Pairs) {
    let mut played = Statistics {
        players: vec![PlayerStatistics::default(); 2],
        ..Statistics::default()
    };
    let mut pairs = kill_cap::Pairs::default();
    for count in 1..=23 {
        let before = played.clone();
        played.players[0].events[event::KILL][0] = count;
        played.players[1].events[event::DEATH][0] = count;
        pairs.observe(Some(&before), &played, &[0, 1]);
    }
    played.players[0].events[event::RESEARCH_CAPTURE][0] = 2100;
    played.players[0].events[event::RESEARCH_NEUTRALISE][0] = 2000;
    (played, pairs)
}

#[test]
fn unpaid_feeding_kills_do_not_buy_objective_allowance_and_originals_survive() {
    let (played, pairs) = feeder();
    let payment = Payment::of(&played, &pairs, mode::RESEARCH, false);
    assert_eq!(payment.kills[0].paid, 10);
    assert_eq!(payment.objectives[0].paid, 600); // 200 + 40 * 10, not 1120.
    assert_eq!(played.players[0].total(event::KILL), 23);
    assert_eq!(played.players[0].total(event::RESEARCH_CAPTURE), 2100);
}

#[test]
fn bot_matches_keep_their_existing_payment_counters() {
    let (played, pairs) = feeder();
    let payment = Payment::of(&played, &pairs, mode::RESEARCH, true);
    assert!(payment.kills.is_empty());
    assert!(payment.objectives.is_empty());
    assert_eq!(payment.statistics, played);
}

#[test]
fn objectives_in_other_modes_are_not_reinterpreted_as_research() {
    let (played, pairs) = feeder();
    let payment = Payment::of(&played, &pairs, mode::BATTERIES, false);
    assert_eq!(payment.kills[0].paid, 10);
    assert!(payment.objectives.is_empty());
    assert_eq!(
        payment.statistics.players[0].total(event::RESEARCH_CAPTURE),
        2100
    );
}
