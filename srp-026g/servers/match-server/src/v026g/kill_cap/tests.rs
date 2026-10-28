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

#[test]
fn an_active_opponent_in_a_small_match_is_not_a_passive_feeder() {
    // Even six enemy kills by the victim raise the allowance to 22, so
    // all eighteen kills pay. The audited 18/8 case's opponent had 19.
    let snapshots: Vec<_> = (1..=18).map(|n| [(n, 0), (0, 0), (0, n), (0, 0)]).collect();
    let pairs = pairs(&snapshots);
    let mut played = statistics([(18, 8), (0, 0), (6, 18), (0, 0)]);
    assert!(cap(&mut played, &pairs).is_empty());
}

#[test]
fn victim_activity_raises_an_allowance_but_does_not_remove_it() {
    let snapshots: Vec<_> = (1..=40).map(|n| [(n, 0), (0, 0), (0, n), (0, 0)]).collect();
    let pairs = pairs(&snapshots);
    let mut played = statistics([(40, 0), (0, 0), (3, 40), (0, 0)]);
    assert_eq!(cap(&mut played, &pairs)[0].paid, 16);
}

#[test]
fn allowances_use_original_activity_independently_of_roster_order() {
    let pairs = Pairs(HashMap::from([((0, 2), 50), ((2, 1), 20)]));
    let mut played = statistics([(50, 0), (0, 20), (20, 50), (0, 0)]);
    let cuts = cap(&mut played, &pairs);
    assert_eq!(
        cuts,
        [Cut {
            place: 2,
            counted: 20,
            paid: 10
        }]
    );
    // Reverse roster order; the victim now gets its own pay cut first.
    let pairs = Pairs(HashMap::from([((2, 0), 50), ((0, 1), 20)]));
    let mut played = statistics([(20, 50), (0, 20), (50, 0), (0, 0)]);
    assert_eq!(
        cap(&mut played, &pairs),
        [Cut {
            place: 0,
            counted: 20,
            paid: 10
        }]
    );
}

#[test]
fn unknown_teams_are_not_evidence_of_enemy_kills() {
    let mut played = statistics([(20, 0), (0, 0), (0, 20), (0, 0)]);
    for teams in [&[0, 0, 255, 1][..], &[255, 0, 1, 1], &[0, 0]] {
        let mut pairs = Pairs::default();
        pairs.observe(None, &played, teams);
        assert!(cap(&mut played, &pairs).is_empty());
    }
}
