//! The server's own loop over localhost UDP, with scripted clients on real
//! sockets and the real clock (waits shortened, every wait bounded).

use std::time::Duration;

use survarium::v026g::match_msgs::ServerMessage;
use survarium::v026g::match_msgs::snapshot::Tdm;
use survarium::v026g::match_msgs::upload::CHUNK_DATA;

use super::client::INITIAL_MS;
use super::{MAP_BATTERIES, MAP_TDM, Net, one_at_a_time, quick, session_of};
use crate::v026g::MatchEnd;
use crate::v026g::registry::static_id;

const SOON: Duration = Duration::from_secs(5);

#[test]
fn one_player_plays_a_whole_match_over_udp() {
    let _guard = one_at_a_time();
    let mut net = Net::new(1, MAP_TDM, quick());
    // Connect, the one port, the static info, read back by the client
    // model as the client reads it.
    net.client(0).connect();
    assert!(net.until(SOON, |net| net.clients[0].0.model.options.is_some()));
    {
        let port = net.registry.port();
        let match_id = net.match_id;
        let client = net.client(0);
        assert_eq!(
            client.taken[0],
            ServerMessage::Ports {
                min: port,
                max: port
            }
        );
        let options = client.model.options.clone().unwrap();
        assert_eq!(client.model.local_index, Some(0));
        assert_eq!(options.players.len(), 1);
        assert_eq!(options.map.map().id, MAP_TDM);
        assert_eq!(options.match_id, static_id(match_id));
    }

    // Its world goes up in chunks (an initial world, with nobody in it,
    // takes two); it starts from it as generation 0 and enters.
    net.client(0).ready();
    assert!(net.until(SOON, |net| net.clients[0].0.model.is_synced()));
    assert!(net.until(SOON, |net| net.clients[0].0.entered() == [0]));
    assert_eq!(net.clients[0].0.snapshots(), [(0, INITIAL_MS)]);
    assert_eq!(net.clients[0].0.model.generation.0, 0);
    let synced = net.clients[0].0.got(|message| match message {
        ServerMessage::Snapshot { body, .. } => Some(body.clone()),
        _ => None,
    });
    assert_eq!(synced[0], net.clients[0].0.sim.world(INITIAL_MS));
    assert!(synced[0].len() > CHUNK_DATA);

    // It plays: inputs go up and none is turned down; its worlds are asked
    // for, uploaded and vouched for by its own hash reports.
    for _ in 0..10 {
        net.client(0).input(50, 0x21);
        net.run(Duration::from_millis(30));
    }
    assert!(net.until(SOON, |net| net.clients[0].0.upload_requests >= 2));
    assert!(net.clients[0].0.taken.iter().all(|message| {
        !matches!(
            message,
            ServerMessage::DiscardInput { .. } | ServerMessage::InputRelay { .. }
        )
    }));
    assert!(net.results.try_recv().is_err());

    // Its team runs out of tickets with nobody alive: the mode's rule has
    // its end, and the server ends the match.
    let at = net.clients[0].0.clock() / 50 * 50;
    {
        let sim = &mut net.client(0).sim;
        // Play had begun (the real countdown is 20 s; the client's own
        // rule is the one that counts).
        let mut rule = sim.rule_at(at);
        rule.state = 3;
        rule.start = Some(INITIAL_MS);
        sim.force_rule(rule);
        sim.tdm_from = Some((
            at,
            Tdm {
                tickets: [0, 64],
                alive_players_mask: 0,
                active_players_mask: 1,
            },
        ));
    }
    assert!(net.until(SOON, |net| net.clients[0].0.closed));
    let result = net.results.recv_timeout(SOON).expect("the match's result");
    assert_eq!(result.match_id, net.match_id);
    assert_eq!(result.end, MatchEnd::ModeRule);
    assert_eq!(
        result.mode,
        survarium::v026g::maps::GameMode::TeamDeathmatch
    );
    assert_eq!(result.winner, Some(1));
    assert_eq!(result.team_points, [0, 64]);
    assert!(result.played_ms.is_some());
    assert_eq!(result.players.len(), 1);
    assert_eq!(result.players[0].session_id, session_of(0));
    assert!(result.players[0].entered && result.players[0].present_at_end);
    // A moment later its seat is free.
    assert!(net.until(SOON, |net| net.registry.seat(session_of(0)).is_none()));
    net.assert_no_faults();
}

#[test]
fn players_relay_leave_and_time_out_over_udp() {
    let _guard = one_at_a_time();
    let mut net = Net::new(3, MAP_BATTERIES, quick());
    for place in 0..3 {
        net.join(place);
    }
    assert!(net.until(SOON, |net| net.clients[0].0.entered() == [0, 1, 2]));
    // The later ones started behind the match clock; a moment for the
    // pings to bring them up.
    net.run(Duration::from_millis(2_600));

    // Inputs both ways, byte for byte.
    let first = net.client(0).input(100, 0x8000_0021);
    let second = net.client(1).input(100, 0x40);
    assert!(net.until(SOON, |net| {
        net.clients[1].0.relays_of(0) == [first]
            && net.clients[2].0.relays_of(0) == [first]
            && net.clients[0].0.relays_of(1) == [second]
            && net.clients[2].0.relays_of(1) == [second]
    }));
    assert!(net.clients[0].0.relays_of(0).is_empty());

    // One leaves (0xE0): the others see it go.
    net.client(2).leave();
    assert!(net.until(SOON, |net| {
        net.clients[0].0.left() == [2] && net.clients[1].0.left() == [2]
    }));
    // One falls silent: gone after the timeout.
    net.client(1).silent = true;
    assert!(net.until(SOON, |net| net.clients[0].0.left() == [2, 1]));
    assert!(!net.clients[0].0.closed);
    // The one left leaves too: the match is over, with who left.
    net.client(0).leave();
    assert!(net.until(SOON, |net| net.registry.match_count() == 0));
    let result = net.results.recv_timeout(SOON).expect("a result");
    assert_eq!(result.end, MatchEnd::Deserted);
    assert!(
        result
            .players
            .iter()
            .all(|player| player.entered && player.left)
    );
    net.assert_no_faults();
}

#[test]
fn a_client_out_of_step_is_resynced_over_udp() {
    let _guard = one_at_a_time();
    let mut net = Net::new(3, MAP_BATTERIES, quick());
    for place in 0..3 {
        net.join(place);
    }
    net.run(Duration::from_millis(2_600));
    assert_eq!(net.clients[2].0.model.syncs, 1);

    // The third client's world goes its own way: its hashes disagree with
    // the two others', a healthy one is asked for its world (0xF6), and
    // the third starts from it as generation 1.
    let from = net.clients[2].0.clock();
    net.client(2).sim.salt_from = Some((from, 9));
    let asked = net.clients[0].0.upload_requests + net.clients[1].0.upload_requests;
    assert!(net.until(SOON, |net| net.clients[2].0.model.syncs == 2));
    let snapshots = net.clients[2].0.snapshots();
    assert_eq!(snapshots.len(), 2);
    assert_eq!((snapshots[0].0, snapshots[1].0), (0, 1));
    assert!(snapshots[1].1 > from);
    assert_eq!(net.clients[2].0.model.generation.0, 1);
    assert!(net.clients[0].0.upload_requests + net.clients[1].0.upload_requests > asked);
    // The others were never touched, and it does not happen again.
    net.run(Duration::from_millis(1_500));
    assert_eq!(net.clients[0].0.model.syncs, 1);
    assert_eq!(net.clients[1].0.model.syncs, 1);
    assert_eq!(net.clients[2].0.model.syncs, 2);
    assert!(net.clients[0].0.left().is_empty());
    net.assert_no_faults();
}

/// A match recording (`SRP_RECORD_DIR`) of a match of three, kept from
/// three players: every place's datagrams both ways, the notes, the end.
#[test]
fn a_match_of_enough_players_is_recorded_whole_over_udp() {
    use crate::v026g::record::{self, IN, NOTE, OUT, RecordSettings};

    let _guard = one_at_a_time();
    let dir = std::env::temp_dir().join(format!("srp-record-net-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let recorded = |min_players: usize| {
        let mut settings = quick();
        settings.record = Some(RecordSettings {
            dir: dir.clone(),
            min_players,
            max_bytes: 1 << 30,
            replays: None,
        });
        let mut net = Net::new(3, MAP_BATTERIES, settings);
        for place in 0..3 {
            net.join(place);
        }
        assert!(net.until(SOON, |net| net.clients[0].0.entered() == [0, 1, 2]));
        net.run(Duration::from_millis(1_000));
        let first = net.client(0).input(100, 0x8000_0021);
        assert!(net.until(SOON, |net| net.clients[1].0.relays_of(0) == [first]));
        for place in 0..3 {
            net.client(place).leave();
        }
        assert!(net.until(SOON, |net| net.registry.match_count() == 0));
        net.assert_no_faults();
    };
    let files = || -> Vec<std::path::PathBuf> {
        std::fs::read_dir(&dir)
            .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
            .unwrap_or_default()
    };

    // Three entered, three wanted: kept.
    recorded(3);
    let mut records = Vec::new();
    for _ in 0..300 {
        if let [path] = files().as_slice()
            && path.extension().is_some_and(|ext| ext == "srprec")
            && let Ok(read) = record::read(path)
            && read
                .last()
                .is_some_and(|last| last.bytes.starts_with(b"{\"dropped\""))
        {
            records = read;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!records.is_empty(), "{:?}", files());
    let notes: Vec<serde_json::Value> = records
        .iter()
        .filter(|record| record.kind == NOTE)
        .map(|record| serde_json::from_slice(&record.bytes).expect("JSON"))
        .collect();
    assert_eq!(notes[0]["note"], "header");
    assert_eq!(notes[0]["places"].as_array().map(Vec::len), Some(3));
    let entered = notes
        .iter()
        .filter(|note| note["note"] == "entered")
        .count();
    assert_eq!(entered, 3);
    // Each place's game revision, for its replay (0: the test's sessions
    // are not the store's), and nothing of its session or address.
    let revisions: Vec<&serde_json::Value> = notes
        .iter()
        .filter(|note| note["note"] == "revision")
        .collect();
    assert_eq!(revisions.len(), 3, "{notes:?}");
    for note in revisions {
        assert_eq!(note["revision"], 0);
        let keys: Vec<&String> = note.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["note", "place", "revision"]);
    }
    assert!(notes.iter().any(|note| note["note"] == "clock"));
    assert!(notes.iter().any(|note| note["note"] == "result"));
    let closed = notes.last().expect("notes");
    assert_eq!(closed["note"], "closed");
    assert_eq!(closed["players"], 3);
    assert_eq!(closed["dropped"], 0);
    for place in 0..3u8 {
        let of = |kind| {
            records
                .iter()
                .filter(|record| record.kind == kind && record.place == place)
                .count()
        };
        assert!(
            of(IN) > 10 && of(OUT) > 10,
            "place {place}: {} in, {} out",
            of(IN),
            of(OUT)
        );
    }
    // Times go forward (a roster note takes the time it is made, a moment
    // after the loop's own).
    assert!(
        records
            .windows(2)
            .all(|pair| pair[0].at_ms <= pair[1].at_ms + 5)
    );

    // Three entered, four wanted: nothing left behind.
    std::fs::remove_dir_all(&dir).expect("the folder");
    recorded(4);
    std::thread::sleep(Duration::from_millis(300));
    assert!(files().is_empty(), "{:?}", files());
    let _ = std::fs::remove_dir_all(&dir);
}
