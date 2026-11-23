use survarium::v026g::match_msgs::snapshot::PlayerStatistics;
use survarium::v026g::match_msgs::static_info::score_weights;
use survarium::v026g::maps::RuleMode;

use super::*;

/// One place per entry, each with its (event, kit, count).
fn statistics(places: &[&[(usize, usize, u16)]]) -> Statistics {
    Statistics {
        players: places
            .iter()
            .map(|events| {
                let mut player = PlayerStatistics::default();
                for &(event, kit, count) in *events {
                    player.events[event][kit] = count;
                }
                player
            })
            .collect(),
        ..Statistics::default()
    }
}

const PUT: usize = event::PUT_VICTORY_ITEM_IN_CONTAINER;
const BRING: usize = event::BRING_VICTORY_ITEM;

fn score(player: &PlayerStatistics) -> u32 {
    survarium::v026g::match_msgs::snapshot::score(player, &score_weights(RuleMode::ArtefactHunt))
}

#[test]
fn two_lone_deliveries_or_a_team_s_share_pay_whole() {
    // Two deliveries carried alone (120), and a teammate's shares of five.
    let mut played = statistics(&[
        &[(PUT, 0, 2), (BRING, 0, 200)],
        &[(BRING, 1, 250), (event::KILL, 1, 1)],
    ]);
    let original = played.clone();
    assert!(cap(&mut played, 5).is_empty());
    assert_eq!(played, original);
    assert_eq!(score(&played.players[0]), 120);
}

#[test]
fn deliveries_with_no_fighting_pay_two_at_most() {
    // A team fed five deliveries, one player carried them all alone: 300.
    let mut played = statistics(&[&[(PUT, 0, 5), (BRING, 0, 500)]]);
    assert_eq!(cap(&mut played, 5), [Cut { place: 0, scored: 300, paid: 120 }]);
    assert!(score(&played.players[0]) <= 120);
    // Kills and assists widen it: 120 + 2 x 40 + 20 = 220.
    let mut fought = statistics(&[&[(PUT, 0, 5), (BRING, 0, 500), (event::KILL, 0, 2), (event::ASSIST, 0, 1)]]);
    assert_eq!(cap(&mut fought, 5)[0].paid, 220);
    // Enough of it: nothing cut.
    let mut more = statistics(&[&[(PUT, 0, 5), (BRING, 0, 500), (event::KILL, 0, 5)]]);
    assert!(cap(&mut more, 5).is_empty());
}

#[test]
fn no_more_than_the_goal_s_deliveries_are_paid_whatever_the_fighting() {
    // A team delivers `goal` times a match at most, and a delivery's shares
    // add up to 100: more is no match's, cut before the allowance.
    let mut played = statistics(&[&[(PUT, 0, 9), (BRING, 2, 1400), (event::KILL, 0, 30)]]);
    let cuts = cap(&mut played, 5);
    let player = &played.players[0];
    assert_eq!((player.total(PUT), player.total(BRING)), (5, 500));
    assert_eq!(cuts, [Cut { place: 0, scored: 790, paid: 300 }]);
    // Kills and the other events are never cut here.
    assert_eq!(player.total(event::KILL), 30);
}

#[test]
fn passing_the_artefact_back_and_forth_counts_one_delivery_s_shares() {
    // The rule shares a delivery's 100 hundredths among its team's carriers
    // by their parts of the way, however often they passed it: two players of
    // a team that delivered once score 50 between them, and nothing is cut.
    let mut played = statistics(&[&[(PUT, 0, 1), (BRING, 0, 60)], &[(BRING, 0, 40)]]);
    assert!(cap(&mut played, 5).is_empty());
    let total: u32 = played.players.iter().map(score).sum();
    assert_eq!(total, 10 + 50);
}
