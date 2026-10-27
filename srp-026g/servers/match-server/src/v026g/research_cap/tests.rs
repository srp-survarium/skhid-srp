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
fn units_traded_back_and_forth_pay_two_hundred_score() {
    // Live, revision 35: 0 kills, score 431, all of it from units.
    // 0.3 a hundredth: 700 + 700 hundredths and 1 invader are 430.
    let mut played = statistics(&[
        &[
            (event::RESEARCH_CAPTURE, 0, 700),
            (event::RESEARCH_NEUTRALISE, 0, 700),
            (event::KILL_INVADER, 0, 1),
            (event::KILL, 0, 2),
        ],
        &[(event::RESEARCH_CAPTURE, 0, 300), (event::KILL, 0, 12)],
    ]);
    let cuts = cap(&mut played);
    assert_eq!(
        cuts,
        [Cut {
            place: 0,
            scored: 430,
            paid: 200
        }]
    );
    let farmer = &played.players[0];
    // In the same proportion, 200 / 430.
    assert_eq!(farmer.total(event::RESEARCH_CAPTURE), 325);
    assert_eq!(farmer.total(event::RESEARCH_NEUTRALISE), 325);
    assert_eq!(farmer.total(event::KILL_INVADER), 0);
    assert_eq!(farmer.total(event::KILL), 2);
    // Three captures, 90 score: as it was.
    assert_eq!(played.players[1].total(event::RESEARCH_CAPTURE), 300);
}

#[test]
fn two_hundred_score_is_paid_whole() {
    let mut played = statistics(&[&[
        (event::RESEARCH_CAPTURE, 0, 500),
        (event::RESEARCH_NEUTRALISE, 1, 100),
        (event::KILL_INVADER, 2, 2),
    ]]);
    assert_eq!(cap(&mut played), []);
    assert_eq!(played.players[0].total(event::RESEARCH_CAPTURE), 500);
}
