//! Exercise the real registry, game and transport. Scripted clients model
//! background resource completion independently of transport delivery; native
//! resource loading still needs its separate executable integration test.

use super::*;
use crate::v026g::JoinError;
use survarium::v026g::match_msgs::late_join::{Prepare, Prepared, Status};
use survarium::v026g::match_msgs::{ServerMessage, id, rules::state};

fn running() -> Bench {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.game().admitting());
    bench
}

#[test]
fn admission_closes_at_two_minutes_remaining_and_stale_publications_expire() {
    let bench = running();
    let deadline = bench.game().admission_deadline(bench.now()).unwrap();
    assert!(
        bench
            .game()
            .admission_deadline(deadline - Duration::from_millis(1))
            .is_some()
    );
    assert!(bench.game().admission_deadline(deadline).is_none());
    assert!(
        bench
            .game()
            .admission_deadline(deadline + Duration::from_millis(1))
            .is_none()
    );
    assert_eq!(
        bench
            .registry
            .admission_remaining(bench.match_id, deadline - Duration::from_millis(1)),
        Some(Duration::from_millis(120_001))
    );
    assert!(
        bench
            .registry
            .admission_remaining(bench.match_id, deadline)
            .is_none()
    );
    // No game-loop update is needed to reject a reservation after its deadline.
    bench
        .registry
        .set_admitting(bench.match_id, 2, Some(Instant::now()));
    assert_eq!(
        bench.registry.join_running(bench.match_id, player(2)),
        Err(JoinError::Unavailable)
    );
    assert_eq!(
        bench.registry.formed(bench.match_id).unwrap().places.len(),
        2
    );
}

fn player(place: usize) -> MatchPlayer {
    MatchPlayer {
        session_id: session_of(place),
        account_id: 100 + place as u32,
        profile: profile(&format!("new-player-{place}"), (place % 2) as u8),
        bot: false,
        driven: false,
    }
}

fn reserve(bench: &mut Bench) -> usize {
    let place = bench.clients.len();
    let ticket = bench
        .registry
        .join_running(bench.match_id, player(place))
        .unwrap();
    assert_eq!(ticket.match_id, bench.match_id);
    let map_id = bench.clients[0].sim.map_id;
    bench
        .clients
        .push(Client::growing(session_of(place), place + 1, map_id));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 40_000 + place as u16)));
    place
}

#[test]
fn a_genuinely_new_player_waits_for_resources_then_gets_a_post_barrier_snapshot() {
    let mut bench = running();
    let syncs = [bench.clients[0].model.syncs, bench.clients[1].model.syncs];
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    let prepare = bench.clients[1].prepares[0].clone();
    assert_eq!(prepare.profile.name, "new-player-2");
    assert_eq!(prepare.proposal.place, 2);

    // E4's transport acknowledgement alone must never admit the newcomer.
    // Existing players still exchange movement/fire inputs during the wait.
    let clock_before = bench.clock();
    for _ in 0..60 {
        bench.clients[0].input(50, 0x21);
        bench.run(50);
    }
    assert!(bench.clock() >= clock_before + 3_000);
    assert!(bench.clients[1].relays_of(0).len() >= 50);
    assert!(bench.clients[place].snapshots().is_empty());
    assert!(!bench.clients[0].entered().contains(&2));
    assert_eq!(&bench.stages()[..2], "LL");

    // Neither a readiness message for another proposal nor Busy passes.
    let mut stale = Prepared {
        proposal: prepare.proposal,
        status: Status::Installed,
    };
    stale.proposal.serial += 100;
    bench.clients[1].send(id::LEAVE, &stale.encode().unwrap(), true);
    bench.clients[1].prepared(&prepare, Status::Busy);
    bench.run(1_200);
    assert!(bench.clients[place].snapshots().is_empty());

    let barrier_clock = bench.clock();
    bench.clients[1].prepared(&prepare, Status::Installed);
    assert!(bench.until(8_000, |b| b.stages() == "LLL"));
    let snapshots = bench.clients[place].snapshots();
    assert_eq!(snapshots.len(), 1);
    assert!(
        snapshots[0].1 >= barrier_clock,
        "{snapshots:?}, barrier {barrier_clock}"
    );
    assert!(bench.until(500, |b| b.clients.iter().all(|c| c.entered().contains(&2))));
    assert_eq!(
        [bench.clients[0].model.syncs, bench.clients[1].model.syncs],
        syncs
    );
    bench.run(5_000);
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    assert_eq!(bench.game().resyncs(2), 0);
    // Right from its first report: its world was never asked for.
    assert_eq!(bench.clients[place].upload_requests, 0);
    bench.assert_no_faults();
}

#[test]
fn a_roster_grows_across_both_mask_boundaries_and_stops_at_twenty() {
    let mut bench = running();
    for expected in 2..20 {
        assert!(bench.until(2_000, |b| b.game().admitting()));
        let place = reserve(&mut bench);
        assert_eq!(place, expected);
        bench.join(place);
        assert!(
            bench.until(8_000, |b| b
                .clients
                .iter()
                .all(|c| c.entered().contains(&(place as u8)))),
            "place {place}"
        );
        bench.clients[place].input(50, 0x21);
        bench.run(100);
        assert!(!bench.clients[0].relays_of(place as u8).is_empty());
        assert_eq!(bench.clients[0].model.syncs, 1);
        assert_eq!(bench.game().resyncs(0), 0);
        bench.assert_no_faults();
    }
    assert!(!bench.game().admitting());
    assert_eq!(
        bench.registry.join_running(bench.match_id, player(20)),
        Err(JoinError::Full)
    );
}

#[test]
fn tdm_newcomer_reloads_its_reset_tickets_without_reloading_existing_players() {
    use survarium::v026g::match_msgs::snapshot::Tdm;
    let mut bench = Bench::growing(2, MAP_TDM, Settings::default());
    bench.join_all();
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.game().admitting());
    let standing = Some((
        bench.clock() / 50 * 50 + 100,
        Tdm {
            tickets: [88, 78],
            alive_players_mask: 0b11,
            active_players_mask: 0b11,
        },
    ));
    for client in &mut bench.clients {
        client.sim.tdm_from = standing;
    }
    bench.run_long(6_000);
    let place = reserve(&mut bench);
    bench.clients[place].sim.tdm_from = standing;
    bench.join(place);
    assert!(bench.until(8_000, |b| {
        b.clients
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.clients[1].model.syncs, 1);
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    assert_eq!(bench.game().reloads(place), 1);
    assert_eq!(bench.clients[place].model.syncs, 2);
    bench.run_long(5_000);
    assert_eq!(bench.game().resyncs(place), 0);
    bench.assert_no_faults();
}

#[test]
fn growing_dialect_requires_an_explicit_client_capability() {
    let mut bench = Bench::growing(1, MAP_BATTERIES, Settings::default());
    bench.clients[0] = Client::new(session_of(0), true, 1, MAP_BATTERIES);
    bench.clients[0].connect();
    assert!(bench.until(1_000, |b| b.clients[0].disconnected));
    assert!(bench.clients[0].model.options.is_none());
    let mut bench = Bench::new(1, MAP_BATTERIES, Settings::default());
    bench.clients[0] = Client::growing(session_of(0), 1, MAP_BATTERIES);
    bench.clients[0].connect();
    assert!(bench.until(1_000, |b| b.clients[0].disconnected));
}

#[test]
fn stock_debug_flag_is_not_a_growing_roster_capability() {
    use survarium::v026g::match_msgs::late_join::has_connect_capability;
    for flag in 0..=u8::MAX {
        assert_eq!(has_connect_capability(flag), matches!(flag, 0x80 | 0x81));
    }
    for growing in [false, true] {
        let mut bench = if growing {
            Bench::growing(1, MAP_BATTERIES, Settings::default())
        } else {
            Bench::new(1, MAP_BATTERIES, Settings::default())
        };
        let mut payload = session_of(0).to_le_bytes().to_vec();
        payload.push(0xff);
        bench.clients[0].send(id::CONNECT, &payload, true);
        if growing {
            assert!(bench.until(1_000, |b| b.clients[0].disconnected));
        } else {
            assert!(bench.until(1_000, |b| b.clients[0].model.options.is_some()));
            assert!(!bench.clients[0].disconnected);
        }
    }
}

#[test]
fn failed_preparation_disconnects_only_the_newcomer() {
    let mut bench = running();
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    let prepare = bench.clients[1].prepares[0].clone();
    bench.clients[1].prepared(&prepare, Status::Failed);
    assert!(bench.until(1_000, |b| b.clients[place].disconnected));
    bench.run(2_000);
    assert_eq!(&bench.stages()[..2], "LL");
    assert!(bench.clients[0].snapshots().len() == 1);
    assert!(!bench.clients[0].entered().contains(&2));
    // A partially installed descriptor is retained, never recycled for a
    // different account under the same slot.
    assert_eq!(
        bench
            .registry
            .formed(bench.match_id)
            .unwrap()
            .profiles
            .len(),
        3
    );
    assert!(bench.registry.seat(session_of(place)).is_none());
    bench.assert_no_faults();
}

#[test]
fn an_unverified_recent_snapshot_does_not_release_a_new_player() {
    let mut bench = running();
    bench.clients[1].no_hashes = true;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    bench.run(8_000);
    assert!(bench.clients[place].snapshots().is_empty());
    assert_eq!(&bench.stages()[..2], "LL");
    bench.clients[1].no_hashes = false;
    assert!(bench.until(8_000, |b| b.stages() == "LLL"));
    bench.assert_no_faults();
}

#[test]
fn snapshot_delivery_is_not_enough_the_newcomer_must_demonstrate_catchup() {
    let mut bench = running();
    let place = reserve(&mut bench);
    bench.clients[place].no_hashes = true;
    bench.join(place);
    bench.run(2_000);
    assert_eq!(bench.clients[place].snapshots().len(), 1);
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    let relays = bench.clients[0].relays_of(place as u8).len();
    bench.clients[place].input(50, 0x21);
    bench.run(100);
    assert_eq!(bench.clients[0].relays_of(place as u8).len(), relays);
    // Its wrong world must not count as a vote against the existing world.
    bench.clients[place].no_hashes = false;
    bench.clients[place].sim.salt_from = Some((0, 7));
    bench.run(1_000);
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    // Wrong for a second, it is started once more, and that world (the
    // scripted game's salt goes with a load) is right.
    assert!(bench.until(8_000, |b| {
        b.clients
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    assert_eq!(bench.clients[place].snapshots().len(), 2);
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    bench.assert_no_faults();
}

#[test]
fn a_resource_timeout_frees_the_newcomer_without_reloading_existing_players() {
    let mut bench = running();
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    bench.run_long(61_000);
    assert!(bench.clients[place].disconnected);
    assert!(bench.registry.seat(session_of(place)).is_none());
    assert_eq!(&bench.stages()[..2], "LL");
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.clients[1].model.syncs, 1);
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    bench.assert_no_faults();
}

#[test]
fn readiness_from_an_old_connection_cannot_ready_its_replacement() {
    let mut bench = running();
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    let old = bench.clients[1].prepares[0].proposal;
    bench.clients[1] = Client::growing(session_of(1), 3, MAP_BATTERIES);
    bench.addrs[1] = SocketAddr::from(([127, 0, 0, 1], 41_001));
    bench.clients[1].connect();
    assert!(bench.until(1_000, |b| b.clients[1].model.options.is_some()));
    let ack = Prepared {
        proposal: old,
        status: Status::Installed,
    }
    .encode()
    .unwrap();
    bench.clients[1].send(id::LEAVE, &ack, true);
    bench.run(2_000);
    // The replacement loads the place with its map (its static info has
    // it): it is asked to install nothing, and the stale ack changed
    // nothing. The newcomer is prepared without waiting for its load.
    assert!(bench.clients[1].prepares.is_empty());
    assert_eq!(bench.clients[place].snapshots().len(), 1);
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.clients[1].ready();
    assert!(bench.until(10_000, |b| b.stages() == "LLL"));
    assert!(bench.clients[1].prepares.is_empty());
    bench.assert_no_faults();
}

#[test]
fn registry_admission_is_atomic_and_rejects_invalid_profiles() {
    let mut bench = running();
    let before = bench.registry.formed(bench.match_id).unwrap();
    let mut bad = player(2);
    bad.profile.name = "x".repeat(64);
    assert!(matches!(
        bench.registry.join_running(bench.match_id, bad),
        Err(JoinError::StaticInfo(_))
    ));
    assert!(Arc::ptr_eq(
        &before,
        &bench.registry.formed(bench.match_id).unwrap()
    ));
    assert!(bench.registry.seat(session_of(2)).is_none());
    reserve(&mut bench);
    // Even a stale game-loop observation cannot admit a second player.
    bench.registry.set_admitting(
        bench.match_id,
        2,
        Some(bench.now() + Duration::from_secs(600)),
    );
    assert_eq!(
        bench.registry.join_running(bench.match_id, player(3)),
        Err(JoinError::Unavailable)
    );
    assert_eq!(before.profiles.len(), 2);
    assert_eq!(
        bench
            .registry
            .formed(bench.match_id)
            .unwrap()
            .profiles
            .len(),
        3
    );
}

#[test]
fn admission_stays_closed_before_play_and_after_match_end() {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    assert_eq!(
        bench.registry.join_running(bench.match_id, player(2)),
        Err(JoinError::Unavailable)
    );
    bench.join_all();
    assert_ne!(bench.game().rule_state(), state::IN_PROCESS);
    assert_eq!(
        bench.registry.join_running(bench.match_id, player(2)),
        Err(JoinError::Unavailable)
    );
    bench.run_long(130_000);
    assert!(bench.game().admitting());
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
    }
    assert!(bench.until(12_000, |b| b.clients.iter().all(|c| c.closed)));
    assert!(
        bench
            .registry
            .join_running(bench.match_id, player(2))
            .is_err()
    );
    let result = bench.results.try_recv().unwrap();
    assert_eq!(result.end, crate::v026g::MatchEnd::ModeRule);
    assert_eq!(result.team_points, [10, 3]);
    bench.assert_no_faults();
}

#[test]
fn departure_during_preparation_does_not_leave_a_resource_barrier() {
    let mut bench = running();
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    bench.clients[1].leave();
    assert!(bench.until(10_000, |b| b.clients[0].entered().contains(&(place as u8))));
    assert!(bench.clients[0].left().contains(&1));
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.run(2_000);
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(place), 0);
    bench.assert_no_faults();
}

#[test]
fn departed_newcomers_disappear_and_the_next_join_uses_a_new_slot() {
    let mut bench = running();
    let place = reserve(&mut bench);
    bench.join(place);
    assert!(bench.until(2_000, |b| b.clients[0].entered().contains(&(place as u8))));
    bench.clients[place].leave();
    assert!(bench.until(2_000, |b| {
        b.clients[..2]
            .iter()
            .all(|c| c.left().contains(&(place as u8)))
    }));
    assert!(bench.until(2_000, |b| b.game().admitting()));
    let next = reserve(&mut bench);
    assert_eq!(next, place + 1);
    bench.join(next);
    assert!(bench.until(2_000, |b| b.clients[0].entered().contains(&(next as u8))));
    bench.run(2_000);
    for index in [0, 1, next] {
        let mask = bench.clients[index].sim.rule_at(bench.clock()).mask;
        assert_eq!(mask & (1 << place), 0);
        assert_ne!(mask & (1 << next), 0);
        assert_eq!(bench.game().resyncs(index), 0);
    }
    bench.assert_no_faults();
}

#[test]
fn a_newcomer_can_reconnect_during_catchup_without_duplicate_entry() {
    let mut bench = running();
    let place = reserve(&mut bench);
    bench.clients[place].no_hashes = true;
    bench.join(place);
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    bench.clients[place] = Client::growing(session_of(place), place + 1, MAP_BATTERIES);
    bench.addrs[place].set_port(42_000 + place as u16);
    bench.join(place);
    assert!(bench.until(2_000, |b| b.clients[0].entered().contains(&(place as u8))));
    bench.run(2_000);
    for client in &bench.clients {
        assert_eq!(
            client
                .taken
                .iter()
                .filter(|message| matches!(message,
                    ServerMessage::PlayerEntered { player, .. } if *player == place as u8
                ))
                .count(),
            1
        );
    }
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.assert_no_faults();
}

#[test]
fn lost_preparations_and_snapshot_packets_are_retried_without_reloading_peers() {
    let mut bench = running();
    for client in &mut bench.clients {
        let mut packets = 0;
        client.lose = Some(Box::new(move |_| {
            packets += 1;
            packets % 4 == 0
        }));
    }
    let place = reserve(&mut bench);
    let mut packets = 0;
    bench.clients[place].lose = Some(Box::new(move |_| {
        packets += 1;
        packets % 3 == 0
    }));
    bench.join(place);
    assert!(bench.until(10_000, |b| {
        b.clients
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.clients[1].model.syncs, 1);
    bench.assert_no_faults();
}

#[test]
fn failed_catchup_times_out_without_entering_or_voting_against_existing_players() {
    let mut bench = running();
    let place = reserve(&mut bench);
    bench.clients[place].no_hashes = true;
    bench.join(place);
    // Its world has objects the others' have not, also after a load.
    bench.clients[place].sim.late_objects_from = Some((0, 7));
    bench.clients[place].no_hashes = false;
    // Started once more, once only, with 30 s more to catch up in.
    bench.run_long(61_000);
    assert!(!bench.clients[place].disconnected);
    assert_eq!(bench.clients[place].snapshots().len(), 2);
    assert_eq!(bench.clients[place].upload_requests, 1);
    bench.run_long(30_000);
    assert_eq!(bench.clients[place].snapshots().len(), 2);
    assert!(bench.clients[place].disconnected);
    assert!(bench.registry.seat(session_of(place)).is_none());
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    for index in 0..2 {
        assert_eq!(bench.clients[index].model.syncs, 1);
        assert_eq!(bench.game().resyncs(index), 0);
    }
    bench.assert_no_faults();
}

#[test]
fn a_newcomer_wrong_from_its_world_on_is_saved_and_started_once_more() {
    let dir = std::env::current_exe()
        .unwrap()
        .with_file_name(format!("joiner-worlds-{}", std::process::id()));
    let settings = Settings {
        snapshot_dir: Some(dir.clone()),
        ..Settings::default()
    };
    let mut bench = Bench::growing(2, MAP_BATTERIES, settings);
    bench.join_all();
    bench.run_long(25_000);
    let place = reserve(&mut bench);
    bench.clients[place].no_hashes = true;
    bench.join(place);
    let uploads = bench.clients[place].uploads;
    // Its world is not the others' from its first report on.
    bench.clients[place].sim.salt_from = Some((0, 7));
    bench.clients[place].no_hashes = false;
    assert!(bench.until(5_000, |b| b.clients[place].upload_requests == 1));
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    assert_eq!(bench.clients[place].snapshots().len(), 1);
    // Its world comes, and is saved, not counted: no report of it is kept
    // to judge the others by.
    assert!(bench.until(1_000, |b| b.clients[place].uploads == uploads + 1));
    bench.run(50);
    assert_eq!(bench.game().hash_reports_of(place), 0);
    // Started once more from a world from then on (the scripted game's
    // salt goes with a load), it is right and enters.
    assert!(bench.until(10_000, |b| {
        b.clients
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    assert_eq!(bench.clients[place].snapshots().len(), 2);
    assert_eq!(bench.clients[place].upload_requests, 1);
    for index in 0..2 {
        assert_eq!(bench.clients[index].model.syncs, 1);
        assert_eq!(bench.game().resyncs(index), 0);
    }
    let saved: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(&format!("_p{place}_joiner_")))
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(saved.len(), 1, "{saved:?}");
    bench.assert_no_faults();
}

#[test]
fn joins_preserve_living_dead_and_respawned_player_snapshots() {
    use survarium::v026g::match_msgs::players::PlayerState;
    let mut bench = running();
    for alive in [true, false, true] {
        let head = PlayerState {
            current_index: 0,
            target_index: 0,
            items: vec![990_000],
            alive,
            has_to_die: false,
            look_pitch: 0.25,
            rotation_y: 1.5,
            actions_mask: 0,
            effects: 0,
        };
        for client in &mut bench.clients {
            client.sim.player_states[0] = Some(head.clone());
        }
        assert!(bench.until(2_000, |b| b.game().admitting()));
        let place = reserve(&mut bench);
        bench.clients[place].sim.player_states[0] = Some(head.clone());
        bench.join(place);
        assert!(bench.until(2_000, |b| {
            b.clients
                .iter()
                .all(|c| c.entered().contains(&(place as u8)))
        }));
        let body = bench.clients[place]
            .taken
            .iter()
            .find_map(|message| match message {
                ServerMessage::Snapshot { body, .. } => Some(body),
                _ => None,
            })
            .unwrap();
        let blocks = survarium::v026g::match_msgs::players::blocks(body).unwrap();
        assert_eq!(blocks[0].place, 0);
        assert_eq!(PlayerState::read(blocks[0].state, 1), Some(head));
        bench.run(2_000);
        let time = bench.clock() / 50 * 50;
        assert_eq!(
            bench.clients[0].sim.world(time),
            bench.clients[place].sim.world(time)
        );
        assert_eq!(bench.clients[0].model.syncs, 1);
        for index in 0..bench.clients.len() {
            assert_eq!(bench.game().resyncs(index), 0);
        }
        bench.assert_no_faults();
    }
}

#[test]
fn ending_a_match_while_resources_load_never_enters_the_newcomer() {
    let mut bench = running();
    bench.run_long(110_000);
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
    }
    assert!(bench.until(12_000, |b| b.clients[..2].iter().all(|c| c.closed)));
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    assert!(bench.clients[place].snapshots().is_empty());
    assert!(
        bench
            .registry
            .join_running(bench.match_id, player(3))
            .is_err()
    );
    let result = bench.results.try_recv().unwrap();
    assert_eq!(result.end, crate::v026g::MatchEnd::ModeRule);
    assert!(!result.players[place].present_at_end);
    bench.assert_no_faults();
}

#[test]
fn duplicate_accounts_and_sessions_cannot_reserve_a_second_place() {
    let bench = running();
    for duplicate_session in [false, true] {
        let mut newcomer = player(2);
        if duplicate_session {
            newcomer.session_id = session_of(0);
        } else {
            newcomer.account_id = 100;
        }
        assert_eq!(
            bench.registry.join_running(bench.match_id, newcomer),
            Err(JoinError::AlreadyPlaying)
        );
        assert_eq!(
            bench.registry.formed(bench.match_id).unwrap().places.len(),
            2
        );
        assert!(bench.registry.seat(session_of(2)).is_none());
    }
}

#[test]
fn a_decided_match_closes_admission_before_the_results_delay() {
    let settings = Settings {
        timing: crate::v026g::Timing {
            end_delay_ms: 30_000,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bench = Bench::growing(2, MAP_BATTERIES, settings);
    bench.join_all();
    bench.run_long(130_000);
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
    }
    bench.run_long(10_000);
    assert!(bench.clients.iter().all(|client| !client.closed));
    assert_eq!(
        bench.registry.join_running(bench.match_id, player(2)),
        Err(JoinError::Unavailable)
    );
    bench.assert_no_faults();
}

#[test]
fn repeated_failure_acknowledgements_cannot_reject_the_same_account_again() {
    let mut bench = running();
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    let prepare = bench.clients[1].prepares[0].clone();
    bench.clients[1].prepared(&prepare, Status::Failed);
    assert!(bench.until(1_000, |b| b.clients[place].disconnected));
    assert!(bench.registry.seat(session_of(place)).is_none());
    bench.clients[1].prepared(&prepare, Status::Failed);
    let now = bench.now();
    for datagram in bench.clients[1].poll(bench.ms) {
        bench.server.receive(bench.addrs[1], &datagram, now);
    }
    assert!(
        bench
            .server
            .games
            .get_mut(&bench.match_id)
            .unwrap()
            .take_rejected_joins()
            .is_empty()
    );
    bench.assert_no_faults();
}

fn reserve_pair(bench: &mut Bench) {
    bench
        .registry
        .join_running_group(bench.match_id, vec![player(2), player(3)])
        .unwrap();
    for place in 2..4 {
        bench
            .clients
            .push(Client::growing(session_of(place), 4, MAP_BATTERIES));
        bench
            .addrs
            .push(SocketAddr::from(([127, 0, 0, 1], 40_000 + place as u16)));
        bench.clients[place].connect();
    }
    assert!(bench.until(1_000, |b| {
        b.clients[2..].iter().all(|c| c.model.options.is_some())
    }));
    for client in &mut bench.clients[2..] {
        client.ready();
    }
}

#[test]
fn a_reserved_group_enters_on_one_tick_after_every_member_catches_up() {
    let mut bench = running();
    reserve_pair(&mut bench);
    bench.clients[3].no_hashes = true;
    assert!(bench.until(10_000, |b| b.stages() == "LLLL"));
    bench.run(2_000);
    assert!(!bench.clients[0].entered().contains(&2));
    assert!(!bench.clients[0].entered().contains(&3));
    bench.clients[3].no_hashes = false;
    assert!(bench.until(3_000, |b| {
        b.clients.iter().all(|c| c.entered().contains(&3))
    }));
    for client in &bench.clients {
        let entries: Vec<_> = client
            .taken
            .iter()
            .filter_map(|message| match message {
                ServerMessage::PlayerEntered { tick, player } if *player >= 2 => {
                    Some((*tick, *player))
                }
                _ => None,
            })
            .collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, entries[1].0);
    }
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.assert_no_faults();
}

/// A group of newcomers reserved together (as the lobby's late placement
/// does, `join_running_group`): their clients, not connected.
fn reserve_group(bench: &mut Bench, count: usize) -> Vec<usize> {
    let first = bench.clients.len();
    let places: Vec<usize> = (first..first + count).collect();
    bench
        .registry
        .join_running_group(
            bench.match_id,
            places.iter().map(|place| player(*place)).collect(),
        )
        .unwrap();
    for place in &places {
        bench.clients.push(Client::growing(
            session_of(*place),
            first + count,
            MAP_BATTERIES,
        ));
        bench
            .addrs
            .push(SocketAddr::from(([127, 0, 0, 1], 40_000 + *place as u16)));
    }
    places
}

#[test]
fn a_group_member_whose_game_never_comes_is_left_out_and_the_rest_enter() {
    let mut bench = running();
    let places = reserve_group(&mut bench, 2);
    // Only the first comes; it catches up at once, and waits for the other.
    bench.join(places[0]);
    bench.run_long(15_000);
    assert!(!bench.clients[0].entered().contains(&(places[0] as u8)));
    // 20 s after the reservation the other is let go: the first enters
    // (2 v 1 then, as the sides may be).
    assert!(bench.until(8_000, |b| {
        b.clients[..2]
            .iter()
            .all(|c| c.entered().contains(&(places[0] as u8)))
    }));
    assert!(!bench.clients[0].entered().contains(&(places[1] as u8)));
    assert!(bench.registry.seat(session_of(places[1])).is_none());
    assert!(bench.registry.seat(session_of(places[0])).is_some());
    bench.run(3_000);
    for index in 0..2 {
        assert_eq!(bench.clients[index].model.syncs, 1);
        assert_eq!(bench.game().resyncs(index), 0);
    }
    bench.assert_no_faults();
}

#[test]
fn a_member_started_once_more_gives_its_caught_up_group_its_extra_time() {
    let mut bench = running();
    let reserved = bench.ms;
    let places = reserve_group(&mut bench, 2);
    bench.join(places[0]);
    // The second's world is not the others' from its first report on: it
    // is started once more, with 30 s more to catch up in.
    bench.clients[places[1]].no_hashes = true;
    bench.join(places[1]);
    bench.clients[places[1]].sim.salt_from = Some((0, 7));
    bench.clients[places[1]].no_hashes = false;
    assert!(bench.until(10_000, |b| b.clients[places[1]].upload_requests == 1));
    // From then on it reports nothing for a while.
    bench.clients[places[1]].no_hashes = true;
    assert!(bench.until(5_000, |b| b.clients[places[1]].snapshots().len() == 2));
    // The first caught up long ago; it waits past its own 60 s, for as
    // long as the second may take.
    while bench.ms < reserved + 65_000 {
        bench.step(25);
    }
    for place in &places {
        assert!(!bench.clients[*place].disconnected, "{place}");
        assert!(bench.registry.seat(session_of(*place)).is_some(), "{place}");
    }
    assert!(!bench.clients[0].entered().contains(&(places[0] as u8)));
    // The second catches up: both enter.
    bench.clients[places[1]].no_hashes = false;
    assert!(bench.until(10_000, |b| {
        b.clients[..2].iter().all(|c| {
            places
                .iter()
                .all(|place| c.entered().contains(&(*place as u8)))
        })
    }));
    bench.assert_no_faults();
}

#[test]
fn a_group_member_that_leaves_is_left_out_and_the_rest_enter() {
    let mut bench = running();
    let places = reserve_group(&mut bench, 2);
    bench.clients[places[1]].no_hashes = true;
    for place in &places {
        bench.join(*place);
    }
    // The second leaves before it is in; the first waits for it a while.
    bench.clients[places[1]].leave();
    bench.run_long(10_000);
    assert!(!bench.clients[0].entered().contains(&(places[0] as u8)));
    assert!(bench.until(15_000, |b| {
        b.clients[..2]
            .iter()
            .all(|c| c.entered().contains(&(places[0] as u8)))
    }));
    assert!(!bench.clients[0].entered().contains(&(places[1] as u8)));
    bench.assert_no_faults();
}

#[test]
fn a_part_of_a_group_that_would_unbalance_the_sides_does_not_enter_alone() {
    // Three in play, 2 v 1; the group is one for each side.
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(25_000);
    assert!(bench.game().admitting());
    let places = reserve_group(&mut bench, 2);
    assert_eq!(places, [3, 4]);
    // The one for the side that has fewer never comes: the other alone
    // would make it 3 v 1.
    bench.join(places[1]);
    bench.run_long(30_000);
    assert!(!bench.clients[0].entered().contains(&(places[1] as u8)));
    // So it waits out its time.
    bench.run_long(31_000);
    assert!(bench.clients[places[1]].disconnected);
    assert!(!bench.clients[0].entered().contains(&(places[1] as u8)));
    bench.assert_no_faults();
}

#[test]
fn group_reservation_validates_every_member_before_changing_any_seat() {
    let bench = running();
    let before = bench.registry.formed(bench.match_id).unwrap();
    let mut bad = player(3);
    bad.profile.name = "x".repeat(64);
    assert!(matches!(
        bench
            .registry
            .join_running_group(bench.match_id, vec![player(2), bad]),
        Err(JoinError::StaticInfo(_))
    ));
    assert_eq!(
        bench
            .registry
            .join_running_group(bench.match_id, vec![player(2), player(2)]),
        Err(JoinError::AlreadyPlaying)
    );
    assert_eq!(
        bench.registry.join_running_group(bench.match_id, vec![]),
        Err(JoinError::EmptyGroup)
    );
    assert!(Arc::ptr_eq(
        &before,
        &bench.registry.formed(bench.match_id).unwrap()
    ));
    assert!(bench.registry.seat(session_of(2)).is_none());
    assert!(bench.registry.seat(session_of(3)).is_none());
}

#[test]
fn a_group_member_failing_catchup_is_left_out_and_the_rest_get_their_entry_check() {
    let mut bench = running();
    reserve_pair(&mut bench);
    bench.clients[3].no_hashes = true;
    bench.run_long(61_000);
    // The member that never caught up is let go at the group's deadline ...
    assert!(bench.clients[3].disconnected);
    assert!(bench.registry.seat(session_of(3)).is_none());
    assert!(!bench.clients[0].entered().contains(&3));
    // ... and the one that did is not let go with it unchecked: the group
    // is smaller, and the sides 2 v 1 are no further apart than one
    // player, so it enters (`entry_keeps_balance`).
    assert!(!bench.clients[2].disconnected);
    assert!(bench.clients[0].entered().contains(&2));
    assert_eq!(&bench.stages()[..3], "LLL");
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.assert_no_faults();
}

#[test]
fn a_cancelled_admission_cannot_later_remove_the_accounts_new_match_seat() {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    let shared = store();
    let (sink, _) = channel();
    bench.server = Server::new(
        1236,
        bench.registry.clone(),
        shared.clone(),
        Box::new(sink),
        Settings::default(),
        bench.now(),
    );
    bench.join_all();
    bench.run_long(25_000);
    let place = reserve(&mut bench);
    bench.clients[place].no_hashes = true;
    bench.join(place);
    bench.clients[place].leave();
    bench.run(100);
    assert!(bench.registry.seat(session_of(place)).is_none());
    let newcomer = session::Player {
        session_id: session_of(place),
        account_id: 102,
        profile_id: 1,
    };
    let other = session::Player {
        session_id: 0x5000,
        account_id: 500,
        profile_id: 1,
    };
    let new_match = shared
        .matchmaking()
        .seat_teams([&[newcomer], &[other]], bench.now())
        .unwrap();
    bench.run_long(61_000);
    assert_eq!(
        shared.matchmaking().seat(newcomer.session_id),
        Some((new_match, 0))
    );
}

/// Three games in a match being played: a tick two of them name alike has
/// a majority.
fn running_three() -> Bench {
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.game().admitting());
    bench
}

/// A newcomer is reserved and loading; game 1 has not installed its place
/// yet. Returns the place and what game 1 was asked to install.
fn reserve_with_one_game_still_installing(bench: &mut Bench) -> (usize, Prepare) {
    bench.clients[1].auto_prepare = false;
    let place = reserve(bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    let prepare = bench.clients[1].prepares[0].clone();
    (place, prepare)
}

#[test]
fn a_roster_change_while_a_newcomers_world_is_on_its_way_reaches_it_after_the_world() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use udp_transport::wire;
    use udp_transport::{Direction, Table};
    let mut bench = running_three();
    let place = reserve(&mut bench);
    // The newcomer's world is held up on its way.
    let holding = Arc::new(AtomicBool::new(true));
    let held = holding.clone();
    bench.clients[place].lose = Some(Box::new(move |datagram| {
        held.load(Ordering::Relaxed)
            && wire::parse(Table::Match, Direction::ToClient, datagram, 20).is_ok_and(|parsed| {
                parsed
                    .messages
                    .iter()
                    .any(|message| message.id == id::SNAPSHOT)
            })
    }));
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(bench.until(8_000, |b| b.stages().ends_with('s')));
    // A player leaves meanwhile. The newcomer gets it after its world: the
    // client's 0xEA handler crashes before its load (live, 2026-10-14).
    bench.clients[2].leave();
    bench.run(300);
    holding.store(false, Ordering::Relaxed);
    assert!(bench.until(8_000, |b| {
        b.clients[..2]
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    let taken = &bench.clients[place].taken;
    let left = taken
        .iter()
        .position(|message| matches!(message, ServerMessage::PlayerLeft { player: 2, .. }))
        .expect("the leave");
    let world = taken
        .iter()
        .position(|message| matches!(message, ServerMessage::Snapshot { .. }))
        .unwrap();
    assert!(left > world, "{left} {world}");
    for index in 0..2 {
        assert_eq!(bench.game().resyncs(index), 0);
    }
    assert_eq!(bench.clients[place].snapshots().len(), 1);
    bench.assert_no_faults();
}

#[test]
fn games_that_install_a_place_seconds_apart_are_not_resynced_for_it() {
    let mut bench = running_three();
    // The newcomer's kits have six grenades. A game's world has a byte for
    // each from the tick it has the place; the newcomer's own from its
    // start.
    for client in &mut bench.clients {
        client.late_objects = 6;
    }
    let (place, prepare) = reserve_with_one_game_still_installing(&mut bench);
    bench.clients[place].sim.late_objects_from = Some((0, 6));
    assert!(bench.until(1_000, |b| {
        b.clients[0].sim.late_objects_from.is_some() && b.clients[2].sim.late_objects_from.is_some()
    }));
    // Game 1 loads the newcomer's kits five seconds longer than the other
    // two: all that time its reports are the minority's, and it is right.
    bench.run(5_000);
    assert!(bench.clients[1].sim.late_objects_from.is_none());
    assert_eq!(bench.game().resyncs(1), 0);
    assert_eq!(&bench.stages()[..3], "LLL");
    bench.clients[1].prepared(&prepare, Status::Installed);
    assert!(bench.until(8_000, |b| b.stages() == "LLLL"));
    assert!(bench.until(500, |b| {
        b.clients
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    // The reports of those seconds are judged at no later time either.
    bench.run(6_000);
    for index in 0..4 {
        assert_eq!(bench.game().resyncs(index), 0, "player {index}");
    }
    bench.assert_no_faults();
}

#[test]
fn a_game_out_of_step_while_a_place_is_installed_is_resynced_afterwards() {
    let mut bench = running_three();
    let (_, prepare) = reserve_with_one_game_still_installing(&mut bench);
    // Game 2's world goes its own way while game 1 still loads the place.
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    bench.run(5_000);
    assert_eq!(bench.game().resyncs(2), 0);
    // Every game has the place: reports are judged as ever from here.
    bench.clients[1].prepared(&prepare, Status::Installed);
    assert!(bench.until(10_000, |b| b.game().resyncs(2) == 1));
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    assert!(bench.until(8_000, |b| b.stages() == "LLLL"));
    bench.assert_no_faults();
}

// A game still loading holds up no other place (live, 2026-10-16/17: of 48
// late joins let go after their 60 s, 12 had loaded and were never
// prepared, or never started, because another game of the match was still
// on its loading screen: a member of their own group, or a player back in
// the match after a time-out).

#[test]
fn a_group_member_stuck_loading_holds_up_no_other_member() {
    let mut bench = running();
    let reserved = bench.ms;
    let places = reserve_group(&mut bench, 2);
    // The first member's game connects and stays on its loading screen: it
    // never says it has loaded (as Adokir's game, 2026-10-17).
    bench.clients[places[0]].connect();
    assert!(bench.until(1_000, |b| b.clients[places[0]].model.options.is_some()));
    // The second loads: it is prepared and started from a world at once,
    // and waits for the first.
    bench.clients[places[1]].connect();
    assert!(bench.until(1_000, |b| b.clients[places[1]].model.options.is_some()));
    bench.clients[places[1]].ready();
    assert!(
        bench.until(5_000, |b| b.clients[places[1]].snapshots().len() == 1),
        "a game still loading held up the preparation of another place: {}",
        bench.stages()
    );
    bench.run_long(20_000);
    assert!(!bench.clients[0].entered().contains(&(places[1] as u8)));
    assert!(!bench.clients[places[1]].disconnected);
    // At its 60 s the game on its loading screen is told that its place is
    // gone (FD), and the second enters without it (1 v 2).
    while bench.ms < reserved + 61_000 {
        bench.step(25);
    }
    assert!(bench.clients[places[0]].disconnected);
    assert!(bench.registry.seat(session_of(places[0])).is_none());
    assert!(bench.until(3_000, |b| {
        b.clients[..2]
            .iter()
            .all(|c| c.entered().contains(&(places[1] as u8)))
    }));
    assert!(!bench.clients[places[1]].disconnected);
    assert!(!bench.clients[0].entered().contains(&(places[0] as u8)));
    for index in 0..2 {
        assert_eq!(bench.clients[index].model.syncs, 1);
        assert_eq!(bench.game().resyncs(index), 0);
    }
    bench.assert_no_faults();
}

#[test]
fn a_player_back_in_and_loading_holds_up_no_newcomer() {
    let mut bench = running_three();
    let place = reserve(&mut bench);
    // Player 2's game comes back (a new game, after a crash): its static
    // info has the new place, and it stays on its loading screen a while.
    bench.clients[2] = Client::growing(session_of(2), place + 1, MAP_BATTERIES);
    bench.addrs[2] = SocketAddr::from(([127, 0, 0, 1], 41_002));
    bench.clients[2].connect();
    assert!(bench.until(1_000, |b| b.clients[2].model.options.is_some()));
    // The newcomer is prepared, catches up and enters meanwhile.
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
    assert!(
        bench.until(10_000, |b| {
            b.clients[..2]
                .iter()
                .all(|c| c.entered().contains(&(place as u8)))
        }),
        "a game back in and loading held up the newcomer: {}",
        bench.stages()
    );
    // The game back in was never asked to install a place it loads.
    assert!(bench.clients[2].prepares.is_empty());
    // It loads at last, and plays with the newcomer in its world.
    bench.clients[2].ready();
    assert!(bench.until(10_000, |b| b.stages() == "LLLL"));
    bench.run(3_000);
    for index in 0..4 {
        assert_eq!(bench.game().resyncs(index), 0, "player {index}");
    }
    bench.assert_no_faults();
}

#[test]
fn a_player_resynced_while_a_newcomer_loads_keeps_the_place_installed() {
    // The first guess at the time-outs of 2026-10-17 (a resync of a
    // player of the match while a newcomer loads loses that game's
    // installation of the place): it does not. A resync gives the game a
    // world; what it installed stays, and so does the server's count of it.
    let mut bench = running_three();
    let place = reserve(&mut bench);
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    // Every game of the match installs the place at once ...
    assert!(bench.until(2_000, |b| {
        b.clients[..3].iter().all(|c| !c.prepares.is_empty())
    }));
    // ... and game 2 goes out of step and is resynced while the newcomer
    // still loads.
    bench.run(1_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(bench.until(10_000, |b| b.game().resyncs(2) == 1));
    assert!(bench.until(5_000, |b| &b.stages()[..3] == "LLL"));
    bench.run(1_000);
    bench.clients[place].ready();
    assert!(bench.until(10_000, |b| {
        b.clients[..3]
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    assert!(bench.clients[2].prepares.len() == 1);
    bench.assert_no_faults();
}
