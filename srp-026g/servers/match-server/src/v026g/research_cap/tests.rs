use survarium::v026g::match_msgs::snapshot::PlayerStatistics;

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

#[test]
fn objective_trading_with_no_combat_still_pays_only_two_hundred_points() {
    let mut played = statistics(&[&[
        (event::RESEARCH_CAPTURE, 0, 2100),
        (event::RESEARCH_NEUTRALISE, 1, 2000),
    ]]);
    assert_eq!(
        cap(&mut played),
        [Cut {
            place: 0,
            scored: 1230,
            paid: 200
        }]
    );
    let p = &played.players[0];
    assert!(
        (p.total(event::RESEARCH_CAPTURE) + p.total(event::RESEARCH_NEUTRALISE)) as f64 * 0.3
            <= 200.0
    );
}

#[test]
fn historical_contested_research_players_keep_their_pay() {
    // Match 7964b894: these players lost 46%, 43%, 12% under the flat cap.
    let mut played = statistics(&[
        &[
            (event::RESEARCH_CAPTURE, 0, 769),
            (event::RESEARCH_NEUTRALISE, 0, 400),
            (event::KILL, 0, 12),
            (event::KILL_INVADER, 0, 5),
        ],
        &[
            (event::RESEARCH_CAPTURE, 0, 588),
            (event::RESEARCH_NEUTRALISE, 0, 579),
            (event::KILL, 0, 9),
            (event::KILL_INVADER, 0, 3),
        ],
        &[
            (event::RESEARCH_CAPTURE, 0, 312),
            (event::RESEARCH_NEUTRALISE, 0, 382),
            (event::KILL, 0, 2),
            (event::KILL_INVADER, 0, 1),
        ],
    ]);
    let original = played.clone();
    assert!(cap(&mut played).is_empty());
    assert_eq!(played, original);
}

#[test]
fn assists_expand_the_allowance_and_invader_kills_are_paid_whole() {
    let mut played = statistics(&[&[
        (event::RESEARCH_CAPTURE, 0, 1000),
        (event::RESEARCH_NEUTRALISE, 1, 1000),
        (event::KILL_INVADER, 2, 20),
        (event::KILL, 1, 20),
        (event::ASSIST, 2, 5),
    ]]);
    assert!(cap(&mut played).is_empty()); // 600 objectives within 1100.
    played.players[0].events[event::RESEARCH_CAPTURE][0] = 3000;
    assert_eq!(cap(&mut played)[0].paid, 1100);
    assert_eq!(played.players[0].total(event::KILL_INVADER), 20);
    assert_eq!(played.players[0].total(event::ASSIST), 5);
}

#[test]
fn a_few_kills_do_not_exempt_large_objective_farms() {
    let mut played = statistics(&[&[
        (event::RESEARCH_CAPTURE, 0, 2100),
        (event::RESEARCH_NEUTRALISE, 0, 2000),
        (event::KILL, 0, 12),
    ]]);
    assert_eq!(cap(&mut played)[0].paid, 680);
}

#[test]
fn mixed_combat_objective_play_loses_at_most_a_small_objective_share() {
    // Match 64b3770: 5/10 with four assists, 499.2 objective points.
    let mut played = statistics(&[&[
        (event::RESEARCH_CAPTURE, 0, 1664),
        (event::KILL, 0, 5),
        (event::ASSIST, 0, 4),
    ]]);
    assert_eq!(cap(&mut played)[0].paid, 480);
    assert_eq!(played.players[0].total(event::RESEARCH_CAPTURE), 1600);
}
