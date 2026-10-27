use survarium::v026g::match_msgs::snapshot::PlayerStatistics;

use super::*;

/// Places 0 and 1 on team 0, 2 and 3 on team 1.
const TEAMS: [u8; 4] = [0, 0, 1, 1];

/// Every place's (kills, deaths), on kit 0.
fn statistics(places: [(u16, u16); 4]) -> Statistics {
    Statistics {
        players: places
            .iter()
            .map(|&(kills, deaths)| {
                let mut player = PlayerStatistics::default();
                player.events[event::KILL][0] = kills;
                player.events[event::DEATH][0] = deaths;
                player
            })
            .collect(),
        ..Statistics::default()
    }
}

/// The snapshots in turn, each paired with the one before.
fn pairs(snapshots: &[[(u16, u16); 4]]) -> Pairs {
    let mut pairs = Pairs::default();
    let mut before = None;
    for &places in snapshots {
        let after = statistics(places);
        pairs.observe(before.as_ref(), &after, &TEAMS);
        before = Some(after);
    }
    pairs
}

#[test]
fn killing_the_same_enemy_pays_ten_kills() {
    // Place 0 kills place 2 fifteen times, one a snapshot, and place 3
    // twice.
    let mut snapshots: Vec<[(u16, u16); 4]> =
        (1..=15).map(|n| [(n, 0), (0, 0), (0, n), (0, 0)]).collect();
    snapshots.push([(17, 0), (0, 0), (0, 15), (0, 2)]);
    let pairs = pairs(&snapshots);
    let mut played = statistics([(17, 0), (0, 0), (0, 15), (0, 2)]);
    played.players[0].events[event::HEADSHOT_KILL][0] = 4;
    let cuts = cap(&mut played, &pairs);
    assert_eq!(
        cuts,
        [Cut {
            place: 0,
            counted: 17,
            paid: 12
        }]
    );
    assert_eq!(played.players[0].total(event::KILL), 12);
    // The headshots in the same proportion, 4 x 12 / 17.
    assert_eq!(played.players[0].total(event::HEADSHOT_KILL), 2);
    // The deaths are as they were.
    assert_eq!(played.players[2].total(event::DEATH), 15);
}

#[test]
fn kills_that_come_together_are_not_paired() {
    // Twelve snapshots, each with places 0 and 1 both killing place 2 once:
    // who killed whom is unknown, so nothing is cut.
    let snapshots: Vec<[(u16, u16); 4]> = (1..=12)
        .map(|n| [(n, 0), (n, 0), (0, 2 * n), (0, 0)])
        .collect();
    let pairs = pairs(&snapshots);
    let mut played = statistics(snapshots[11]);
    assert_eq!(cap(&mut played, &pairs), []);
}

#[test]
fn a_teammate_dying_with_a_kill_is_not_paired() {
    // Each time place 0 kills place 2, place 1 dies too (to a grenade of
    // place 3, say).
    let snapshots: Vec<[(u16, u16); 4]> =
        (1..=12).map(|n| [(n, 0), (0, n), (0, n), (0, 0)]).collect();
    let pairs = pairs(&snapshots);
    let mut played = statistics(snapshots[11]);
    assert_eq!(cap(&mut played, &pairs), []);
}

#[test]
fn ten_kills_of_each_enemy_are_paid_whole() {
    let snapshots: Vec<[(u16, u16); 4]> = (1..=10)
        .flat_map(|n| {
            [
                [(2 * n - 1, 0), (0, 0), (0, n), (0, n - 1)],
                [(2 * n, 0), (0, 0), (0, n), (0, n)],
            ]
        })
        .collect();
    let pairs = pairs(&snapshots);
    let mut played = statistics(snapshots[19]);
    assert_eq!(cap(&mut played, &pairs), []);
    assert_eq!(played.players[0].total(event::KILL), 20);
}
