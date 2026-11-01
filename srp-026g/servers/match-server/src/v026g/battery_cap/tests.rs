use std::path::Path;

use survarium::v026g::maps::mode;
use survarium::v026g::match_msgs::MAX_PLAYERS;
use survarium::v026g::match_msgs::snapshot::{self, KITS, PlayerStatistics};
use survarium::v026g::match_msgs::static_info::SCORE_WEIGHTS;

use super::*;

/// The batteries of the battery maps (`victory_items_count`).
const BATTERIES: usize = 10;

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
fn a_battery_put_in_again_and_again_pays_ten_put_ins() {
    // Live, 2026-10-20: one player took a battery out of its own container
    // and put it back 233 times; 30 points from kills.
    let mut played = statistics(&[
        &[
            (event::PUT_VICTORY_ITEM_IN_CONTAINER, 0, 200),
            (event::PUT_VICTORY_ITEM_IN_CONTAINER, 1, 33),
            (event::KILL, 0, 3),
        ],
        &[],
    ]);
    let cuts = cap(&mut played, BATTERIES);
    assert_eq!(
        cuts,
        [Cut {
            place: 0,
            kind: "put in",
            counted: 233,
            paid: 10
        }]
    );
    let farmer = &played.players[0];
    assert_eq!(farmer.total(event::PUT_VICTORY_ITEM_IN_CONTAINER), 10);
    // Shared by what each kit counted, each keeping at least 1.
    assert_eq!(
        farmer.events[event::PUT_VICTORY_ITEM_IN_CONTAINER],
        [8, 2, 0]
    );
    assert_eq!(farmer.total(event::KILL), 3);
}

#[test]
fn batteries_stolen_back_and_forth_pay_ten_steals_each() {
    // Live, match 0x517f92b3: two players stole two batteries from each
    // other's base 18 and 16 times (in hundredths of a delivery).
    let mut played = statistics(&[
        &[
            (event::STEAL_VICTORY_ITEM, 0, 1800),
            (event::STOLEN_AND_DELIVERED, 0, 18),
            (event::PUT_VICTORY_ITEM_IN_CONTAINER, 0, 23),
        ],
        &[
            (event::STEAL_VICTORY_ITEM, 0, 1600),
            (event::STOLEN_AND_DELIVERED, 0, 16),
            (event::PUT_VICTORY_ITEM_IN_CONTAINER, 0, 18),
        ],
    ]);
    let cuts = cap(&mut played, BATTERIES);
    assert_eq!(cuts.len(), 6);
    for player in &played.players {
        assert_eq!(player.total(event::STEAL_VICTORY_ITEM), 1000);
        assert_eq!(player.total(event::STOLEN_AND_DELIVERED), 10);
        assert_eq!(player.total(event::PUT_VICTORY_ITEM_IN_CONTAINER), 10);
    }
    assert!(cuts.contains(&Cut {
        place: 1,
        kind: "stolen delivery share",
        counted: 1600,
        paid: 1000
    }));
}

#[test]
fn an_honest_battery_match_under_the_cap_is_paid_as_counted() {
    let events: &[(usize, usize, u16)] = &[
        (event::PUT_VICTORY_ITEM_IN_CONTAINER, 0, 6),
        (event::PUT_VICTORY_ITEM_IN_CONTAINER, 2, 4),
        (event::BRING_VICTORY_ITEM, 0, 350),
        (event::STEAL_VICTORY_ITEM, 1, 240),
        (event::RECOVER_VICTORY_ITEM, 0, 1000),
        (event::SOLO_DELIVERY, 0, 3),
        (event::STOLEN_AND_DELIVERED, 0, 2),
        (event::KILL, 0, 25),
    ];
    let mut played = statistics(&[events, &[(event::KILL, 0, 400)]]);
    let before = played.clone();
    assert!(cap(&mut played, BATTERIES).is_empty());
    assert_eq!(played, before);
}

#[test]
fn an_active_honest_carriers_eleventh_delivery_is_not_paid() {
    // The rule's known catch (the owner accepted it, 2026-10-20): a
    // carrier who delivers 11 batteries in one match is paid for 10, each
    // with its share and 200 m of progress (exe patch 92).
    let mut played = statistics(&[&[
        (event::PUT_VICTORY_ITEM_IN_CONTAINER, 0, 11),
        (event::BRING_VICTORY_ITEM, 0, 3300),
        (event::SOLO_DELIVERY, 0, 11),
    ]]);
    let cuts = cap(&mut played, BATTERIES);
    assert_eq!(cuts.len(), 3);
    let carrier = &played.players[0];
    assert_eq!(carrier.total(event::PUT_VICTORY_ITEM_IN_CONTAINER), 10);
    assert_eq!(carrier.total(event::BRING_VICTORY_ITEM), 3000);
    assert_eq!(carrier.total(event::SOLO_DELIVERY), 10);
}

#[test]
fn a_carriers_progress_on_the_ten_batteries_is_paid_as_counted() {
    // Ten fresh batteries brought 75 m each: ten shares and 7 points of
    // progress each (exe patch 92), under the cap.
    let mut played = statistics(&[&[(event::BRING_VICTORY_ITEM, 0, 10 * (100 + 70))]]);
    assert!(cap(&mut played, BATTERIES).is_empty());
    assert_eq!(played.players[0].total(event::BRING_VICTORY_ITEM), 1700);
}

#[test]
fn batteries_found_over_the_maps_count_pay_the_maps_count() {
    let mut played = statistics(&[&[
        (event::VICTORY_ITEM_FOUND, 0, 20),
        (event::VICTORY_ITEM_FOUND, 2, 7),
    ]]);
    let cuts = cap(&mut played, BATTERIES);
    assert_eq!(
        cuts,
        [Cut {
            place: 0,
            kind: "found",
            counted: 27,
            paid: 10
        }]
    );
    assert_eq!(played.players[0].total(event::VICTORY_ITEM_FOUND), 10);
}

#[test]
fn a_kit_with_only_farmed_events_is_still_a_kit_used() {
    // Kit 1 did nothing in the match but put a battery in once, kit 2 only
    // stole a little; kit 0 farmed both. `rewards` counts a kit used (its
    // wear, its part of the pay) when any of its counters is above 0.
    let mut played = statistics(&[&[
        (event::PUT_VICTORY_ITEM_IN_CONTAINER, 0, 500),
        (event::KILL, 0, 4),
        (event::PUT_VICTORY_ITEM_IN_CONTAINER, 1, 1),
        (event::STEAL_VICTORY_ITEM, 0, 5000),
        (event::STEAL_VICTORY_ITEM, 2, 40),
    ]]);
    let cuts = cap(&mut played, BATTERIES);
    assert_eq!(cuts.len(), 2);
    let farmer = &played.players[0];
    assert_eq!(
        farmer.events[event::PUT_VICTORY_ITEM_IN_CONTAINER],
        [9, 1, 0]
    );
    assert_eq!(farmer.total(event::STEAL_VICTORY_ITEM), 1000);
    assert!(farmer.events[event::STEAL_VICTORY_ITEM][2] >= 1);
    for kit in 0..KITS {
        let used = farmer.events.iter().any(|counts| counts[kit] > 0);
        assert!(used, "kit {kit}");
    }
}

#[test]
fn a_cut_pays_the_cap_exactly_and_keeps_every_kit_that_counted() {
    for counts in [
        [233, 0, 0],
        [200, 33, 0],
        [1, 1, 1000],
        [11, 0, 1],
        [4, 4, 4],
        [u16::MAX; KITS],
    ] {
        let counted: u32 = counts.iter().map(|&count| u32::from(count)).sum();
        let used = counts.iter().filter(|&&count| count > 0).count() as u32;
        for paid in [1, 2, 3, 10, 1000] {
            if counted <= paid {
                continue;
            }
            let mut now = counts;
            cut(&mut now, paid);
            let total: u32 = now.iter().map(|&count| u32::from(count)).sum();
            assert_eq!(total, paid, "{counts:?} to {paid}: {now:?}");
            for (&now_kit, was) in now.iter().zip(counts) {
                assert!(now_kit <= was, "{counts:?} to {paid}: {now:?}");
                if was > 0 && paid >= used {
                    assert!(now_kit >= 1, "{counts:?} to {paid}: {now:?}");
                }
            }
        }
    }
}

#[test]
fn a_map_without_batteries_cuts_nothing() {
    let mut played = statistics(&[&[(event::PUT_VICTORY_ITEM_IN_CONTAINER, 0, 50)]]);
    assert!(cap(&mut played, 0).is_empty());
    assert_eq!(
        played.players[0].total(event::PUT_VICTORY_ITEM_IN_CONTAINER),
        50
    );
    // Research (mode 5, no batteries: `Game::batteries` is 0) counts its
    // captures in two of the battery events' places: thirty whole ones stay.
    let mut research = statistics(&[&[
        (event::RESEARCH_CAPTURE, 0, 3_000),
        (event::RESEARCH_NEUTRALISE, 1, 1_200),
    ]]);
    assert!(cap(&mut research, 0).is_empty());
    assert_eq!(research.players[0].total(event::RESEARCH_CAPTURE), 3_000);
    assert_eq!(research.players[0].total(event::RESEARCH_NEUTRALISE), 1_200);
}

/// The last world of a directory of saved worlds (`SRP_SNAPSHOT_DIR`'s
/// `match026g_<id>_<ms>_p<n>_<kind>_r<run>.bin`) through the cap on a map
/// of 10 batteries: each place's battery totals and score before and after.
/// `SRP_BATTERY_WORLDS=<dir> cargo test -p match-server -- --ignored
/// --nocapture battery_cap_of_saved_worlds`.
#[test]
#[ignore = "reads saved worlds named by SRP_BATTERY_WORLDS"]
fn battery_cap_of_saved_worlds() {
    let dir = std::env::var("SRP_BATTERY_WORLDS").expect("SRP_BATTERY_WORLDS");
    let (time, path) = std::fs::read_dir(Path::new(&dir))
        .expect("the directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.to_owned();
            let time: u32 = name.split('_').nth(2)?.parse().ok()?;
            (!name.contains("_rejected_")).then_some((time, path))
        })
        .max()
        .expect("a world");
    let body = std::fs::read(path).expect("a world");
    let mut statistics = (1..=MAX_PLAYERS)
        .filter_map(|places| snapshot::decode(time, &body, places, mode::BATTERIES, BATTERIES))
        .filter_map(|decoded| decoded.statistics)
        .find(|statistics| statistics.victory_items.len() == BATTERIES)
        .expect("statistics");
    let before = statistics.clone();
    let cuts = cap(&mut statistics, BATTERIES);
    for (place, (was, now)) in before.players.iter().zip(&statistics.players).enumerate() {
        let totals: Vec<_> = KINDS
            .iter()
            .map(|&(event, _, _)| (event, was.total(event), now.total(event)))
            .filter(|&(_, was, _)| was > 0)
            .collect();
        if !totals.is_empty() {
            println!(
                "place {place}: score {} -> {}, (event, counted, paid) {totals:?}",
                snapshot::score(was, &SCORE_WEIGHTS),
                snapshot::score(now, &SCORE_WEIGHTS)
            );
        }
    }
    println!("at {time} ms: {cuts:?}");
}
