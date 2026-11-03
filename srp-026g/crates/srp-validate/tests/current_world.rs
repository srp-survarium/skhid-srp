use srp_validate::{
    mode,
    world::{self, Context, Section},
};
use survarium::v026g::match_msgs::snapshot as native;

fn context(places: usize, game_mode: u8, batteries: usize) -> Context<'static> {
    Context {
        time: 5000,
        places,
        game_mode,
        batteries,
        respawn_points: Some(3),
        profiles: None,
        artefacts: None,
        objects: None,
        inventory: None,
    }
}

#[test]
fn current_statistics_include_battery_progress_and_sparse_held_steps() {
    for places in [1, 20] {
        for game_mode in [
            mode::BATTERIES,
            mode::TIMED_BATTERIES,
            mode::TEAM_DEATHMATCH,
            mode::RESEARCH,
        ] {
            let batteries = if mode::WITH_BATTERIES.contains(&game_mode) {
                2
            } else {
                0
            };
            let context = context(places, game_mode, batteries);
            let stats = native::Statistics {
                players: vec![native::PlayerStatistics::default(); places],
                victory_items: vec![
                    native::VictoryItemStatus {
                        progress: [5, 9],
                        held: vec![
                            native::HeldSteps { cell: 0, steps: 4 },
                            native::HeldSteps { cell: 2, steps: 7 }
                        ],
                        ..native::VictoryItemStatus::default()
                    };
                    batteries
                ],
                tail: native::StatisticsTail::default(),
            };
            let bytes = stats.encode();
            assert_eq!(
                world::statistics(
                    &Section {
                        at: 0,
                        bytes: &bytes
                    },
                    &context
                ),
                Ok(())
            );
            for len in 0..bytes.len() {
                assert!(
                    world::statistics(
                        &Section {
                            at: 0,
                            bytes: &bytes[..len]
                        },
                        &context
                    )
                    .is_err()
                );
            }
            if batteries != 0 {
                for held in [
                    vec![native::HeldSteps { cell: 60, steps: 1 }],
                    vec![native::HeldSteps { cell: 0, steps: 0 }],
                    vec![native::HeldSteps { cell: 1, steps: 1 }; 2],
                ] {
                    let mut bad = stats.clone();
                    bad.victory_items[0].held = held;
                    assert!(
                        world::statistics(
                            &Section {
                                at: 0,
                                bytes: &bad.encode()
                            },
                            &context
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn current_tdm_rules_and_time_are_checked() {
    let context = context(2, mode::TEAM_DEATHMATCH, 0);
    let rules = native::Rules {
        timelimit: native::Timelimit {
            start: Some(0),
            current: 5000,
            players_mask: 3,
            state: 3,
        },
        respawn: Some(native::Respawn {
            times: vec![],
            seed: 42,
            points: vec![[0, 0]; 3],
        }),
        gather: None,
        tdm: Some(native::Tdm {
            tickets: [97, 100],
            alive_players_mask: 3,
            active_players_mask: 3,
        }),
        research: None,
        kills_deaths: vec![
            native::KillsDeaths {
                kills: 0,
                deaths: 0,
                online: true
            };
            2
        ],
        team_respawn: [true, true],
    };
    let bytes = rules.to_bytes();
    assert_eq!(
        world::rules(
            &Section {
                at: 0,
                bytes: &bytes
            },
            &context
        ),
        Ok(())
    );
    let mut wrong = context;
    wrong.time += 50;
    assert!(
        world::rules(
            &Section {
                at: 0,
                bytes: &bytes
            },
            &wrong
        )
        .is_err()
    );
}

#[test]
fn semantic_views_reuse_the_shipped_frame_and_keep_player_offsets() {
    let mut body = 1u32.to_le_bytes().to_vec();
    body.extend([2, 0, 0, 1, 0]); // state length, kit bytes, no animation
    body.extend([0; 12]);
    let frame = world::frame(&body, 1).unwrap();
    let block = frame.blocks().next().unwrap();
    assert_eq!((block.place, block.at, block.state), (0, 6, &[0, 1][..]));
    assert_eq!(frame.native().slots().mask(), 1);
    body.push(0);
    assert!(world::frame(&body, 1).is_err());
}
