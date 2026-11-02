//! The server's own bots (`MatchPlayer::driven`, `SRP_SERVER_BOTS`): places
//! with no client, entered and played by the match server
//! (`game/driven.rs`).

use survarium::v026g::match_msgs::rules::state;
use survarium::v026g::match_msgs::snapshot::Tdm;
use survarium::v026g::match_msgs::{PlayerInput, ServerMessage, TICK_MS};

use super::{Bench, MAP_TDM, setup};
use crate::v026g::drive::key;
use crate::v026g::{MatchEnd, MatchRegistry, Settings};

/// What a script's inputs must be: on the grid, with no bit a bot never
/// sends.
fn assert_bot_inputs(inputs: &[PlayerInput]) {
    for input in inputs {
        assert_eq!(input.time.ms, None, "on the 50 ms grid: {input:?}");
        assert_eq!(input.actions & key::NEVER, 0, "{input:?}");
    }
}

#[test]
fn the_servers_bot_enters_once_a_real_client_plays_and_its_inputs_reach_it() {
    let mut bench = Bench::with_driven(2, MAP_TDM, Settings::default(), &[1]);

    // Nobody simulates: the bot's place waits, with no client and no clock.
    bench.run(2_000);
    assert_eq!(bench.stages(), "");

    // The real client starts the match; the bot enters after it, ahead of
    // its clock, and the client hears it.
    bench.join(0);
    assert!(bench.until(2_000, |bench| bench.clients[0].entered() == [0, 1]));
    assert_eq!(bench.stages(), "L-");

    // Its script starts 2 s after its entry: the first input walks.
    assert!(bench.until(4_000, |bench| !bench.clients[0].relays_of(1).is_empty()));
    let first = bench.clients[0].relays_of(1)[0];
    assert_eq!(first.actions, key::FORWARD);
    assert_eq!(first.rotation, [0; 4]);

    // A whole run of the script: every input reaches the client, none of
    // them late (it would be moved or dropped, a fault of the client's
    // world), and the bot is no witness: one client vouches for the world.
    bench.run_long(30_000);
    let relays = bench.clients[0].relays_of(1);
    assert!(relays.len() >= 15, "{} inputs", relays.len());
    assert_bot_inputs(&relays);
    assert!(relays.iter().any(|input| input.actions == key::FIRE));
    assert!(relays.iter().any(|input| input.rotation != [0; 4]));
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert_eq!(bench.game().trusted_by(), 1);
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.clients[0].model.syncs, 1);
    let moved = bench.clients[0].got(|message| match message {
        ServerMessage::MoveInput { .. } | ServerMessage::DiscardInput { .. } => Some(()),
        _ => None,
    });
    assert!(moved.is_empty());

    // The bot's team (1) runs out of tickets: the end is the real
    // client's world, and the bot's row says what it is.
    let at = bench.clock() / TICK_MS * TICK_MS + 100;
    bench.clients[0].sim.tdm_from = Some((
        at,
        Tdm {
            tickets: [58, 0],
            alive_players_mask: 0b01,
            active_players_mask: 0b11,
        },
    ));
    assert!(bench.until(12_000, |bench| bench.clients[0].told_to_leave));
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::ModeRule);
    assert_eq!(result.winner, Some(0));
    let bot = &result.players[1];
    assert!(bot.driven && bot.entered && !bot.left);
    assert_eq!((bot.session_id, bot.account_id), (0, 0));
    assert!(!result.players[0].driven);
    bench.assert_no_faults();
}

#[test]
fn a_client_that_comes_later_gets_the_bots_entry_and_inputs_from_the_history() {
    let mut bench = Bench::with_driven(3, MAP_TDM, Settings::default(), &[2]);
    bench.join(0);
    bench.until(2_000, |bench| bench.clients[0].entered() == [0, 2]);
    assert_eq!(bench.clients[0].entered(), [0, 2], "{}", bench.stages());
    // The bot has walked for a while before the second player comes.
    bench.run_long(6_000);
    assert!(!bench.clients[0].relays_of(2).is_empty());

    bench.join(1);
    // The bot's entry is in the world it starts from; its own comes after.
    assert!(bench.until(2_000, |bench| bench.clients[1].entered() == [1]));
    assert_eq!(bench.clients[0].entered(), [0, 2, 1]);
    bench.run_long(20_000);
    // From its start on it gets what the first client gets.
    let first = bench.clients[0].relays_of(2);
    let second = bench.clients[1].relays_of(2);
    assert!(!second.is_empty());
    assert!(first.ends_with(&second[second.len().saturating_sub(5)..]));
    assert_bot_inputs(&second);
    assert_eq!(bench.game().trusted_by(), 2);
    assert_eq!((bench.game().resyncs(0), bench.game().resyncs(1)), (0, 0));
    bench.assert_no_faults();
}

#[test]
fn a_match_left_to_the_servers_bot_is_over() {
    let mut bench = Bench::with_driven(2, MAP_TDM, Settings::default(), &[1]);
    bench.join(0);
    assert!(bench.until(2_000, |bench| bench.clients[0].entered() == [0, 1]));
    bench.run(3_000);
    // As in a match of real players only: the last one gone, it is over.
    bench.clients[0].leave();
    assert!(bench.until(5_000, |bench| bench.results.try_recv().is_ok()));
    assert!(bench.until(10_000, |bench| bench.registry.match_count() == 0));
    bench.assert_no_faults();
}

/// A match of one player against the server's bots ("Player vs Bots") goes
/// on while its player uploads the worlds it is asked for; one that stops
/// uploading for `Timing::pvb_stall` ends it, as one everyone left, and
/// the result says it was a match against bots.
#[test]
fn a_match_against_bots_whose_player_stops_uploading_is_over() {
    let mut settings = Settings::default();
    settings.timing.pvb_stall = std::time::Duration::from_secs(12);
    let mut bench = Bench::vs_bots(4, MAP_TDM, settings, &[1, 2, 3]);
    bench.join(0);
    assert!(bench.until(3_000, |bench| bench.clients[0].entered().len() == 4));
    // Its uploads keep it going well past the stall time.
    bench.run_long(30_000);
    assert!(bench.results.try_recv().is_err());
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);

    // Its game stops answering: over within the stall time and a request.
    bench.clients[0].no_uploads = true;
    let mut result = None;
    for _ in 0..25 {
        bench.run_long(1_000);
        if let Ok(over) = bench.results.try_recv() {
            result = Some(over);
            break;
        }
    }
    let result = result.expect("the match is over");
    assert_eq!(result.end, MatchEnd::Deserted);
    assert!(result.vs_bots);
    assert!(bench.until(10_000, |bench| bench.clients[0].told_to_leave));
}

/// The stall rule is only for a match against bots: a match with the
/// server's bots that is not one goes on.
#[test]
fn a_match_with_bots_for_testing_has_no_stall_rule() {
    let mut settings = Settings::default();
    settings.timing.pvb_stall = std::time::Duration::from_secs(12);
    let mut bench = Bench::growing_with_driven(4, MAP_TDM, settings, &[1, 2, 3]);
    bench.join(0);
    assert!(bench.until(3_000, |bench| bench.clients[0].entered().len() == 4));
    bench.run_long(5_000);
    bench.clients[0].no_uploads = true;
    bench.run_long(30_000);
    assert!(bench.results.try_recv().is_err());
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
}

#[test]
fn a_match_only_the_servers_bots_are_in_is_never_started_and_is_given_up() {
    // No client ever comes: the match is never run, and given up as one
    // nobody came to.
    let mut bench = Bench::with_driven(2, MAP_TDM, Settings::default(), &[0, 1]);
    assert!(bench.until(150_000, |bench| bench.results.try_recv().is_ok()));
    assert_eq!(bench.stages(), "");
    assert_eq!(bench.registry.match_count(), 0);
}

#[test]
fn the_servers_bots_have_no_seats_and_share_session_zero() {
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    let mut setup = setup(4, MAP_TDM);
    for place in [2, 3] {
        let player = &mut setup.players[place];
        (player.driven, player.session_id, player.account_id) = (true, 0, 0);
    }
    registry
        .create(setup)
        .expect("two bots of session 0 are no clash");
    assert!(registry.seat(0).is_none());
    // Nor can a bot be put into a match being played.
    let mut late = super::setup(1, MAP_TDM).players.remove(0);
    late.driven = true;
    assert!(registry.join_running(1, late).is_err());
}

/// A 60 x 60 m floor for the map of [`MAP_TDM`] with two stations to walk
/// to, written where `SRP_MAPDATA_DIR` would point; the store with its
/// walk grid built.
fn walking_map() -> std::sync::Arc<crate::v026g::bots::maps::Maps> {
    use crate::v026g::bots::maps::{MapState, Maps};
    use mapdata::{Library, MapData, Placement, Vec3, WalkerBehaviour};

    let project = survarium::v026g::maps::map_mode(MAP_TDM).unwrap().project;
    let floor = Library {
        name: "floor".into(),
        vertices: vec![
            Vec3::new(-30.0, 0.0, -30.0),
            Vec3::new(30.0, 0.0, -30.0),
            Vec3::new(30.0, 0.0, 30.0),
            Vec3::new(-30.0, 0.0, 30.0),
        ],
        triangles: vec![[0, 2, 1], [0, 3, 2]],
        walker: vec![WalkerBehaviour::Walk; 2],
        primitives: 0,
    };
    let map = MapData {
        project: project.into(),
        levelmap_min: Vec3::new(-30.0, 0.0, -30.0),
        levelmap_max: Vec3::new(30.0, 1.0, 30.0),
        stations: vec![Vec3::new(20.0, 0.0, 20.0), Vec3::new(20.0, 0.0, -20.0)],
        libraries: vec![floor],
        placements: vec![Placement {
            library: 0,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            cgroup: 10,
            cmask: 20,
        }],
        ..MapData::default()
    };
    let dir = std::env::temp_dir().join(format!("srp-walking-map-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    map.write(&dir.join(mapdata::format::file_name(project)))
        .unwrap();
    let maps = Maps::new(Some(dir));
    let started = std::time::Instant::now();
    while !matches!(maps.get(project), MapState::Ready(_)) {
        assert!(started.elapsed().as_secs() < 20, "{:?}", maps.get(project));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    maps
}

#[test]
fn with_the_map_the_servers_bot_sees_its_body_in_the_worlds_and_walks_smoothly() {
    use survarium::v026g::match_msgs::motion::Tail;
    use survarium::v026g::match_msgs::players::PlayerState;

    let settings = Settings {
        maps: Some(walking_map()),
        ..Settings::default()
    };
    let mut bench = Bench::with_driven(2, MAP_TDM, settings, &[1]);
    // The real client's worlds have the bot's body standing at (-20, 0, 0)
    // (the physics frame mirrors z), facing +z.
    let sim = &mut bench.clients[0].sim;
    sim.player_states[1] = Some(PlayerState {
        current_index: 0,
        target_index: 0,
        pickup: weapon_pickup::Record::EMPTY,
        items: vec![1, 2, 3],
        alive: true,
        has_to_die: false,
        look_pitch: 0.0,
        rotation_y: 0.0,
        actions_mask: 0,
        effects: 0,
    });
    sim.player_tails[1] = Some(Tail {
        time_ms: 0,
        speed: 0.0,
        health: 1.0,
        inserted_time_ms: 100,
        physics_position: Some([-20.0, 0.9, 0.0]),
        crouching: false,
        sprinting: false,
        recent_hits: Vec::new(),
    });
    bench.join(0);
    assert!(bench.until(2_000, |bench| bench.clients[0].entered() == [0, 1]));
    // The bots' tactics are built on a thread of their own, in real time.
    let started = std::time::Instant::now();
    while !bench.game().bots_have_map() {
        assert!(started.elapsed().as_secs() < 20, "no tactics");
        std::thread::sleep(std::time::Duration::from_millis(2));
        bench.step(5);
    }

    // Once a world shows its body and play has begun (after the 20 s
    // countdown) it heads for a station: forward, and turns that ease in,
    // none more than the top turning speed allows.
    assert!(bench.until(30_000, |bench| {
        bench.clients[0]
            .relays_of(1)
            .iter()
            .any(|input| input.actions & key::FORWARD != 0)
    }));
    bench.run_long(3_000);
    let relays = bench.clients[0].relays_of(1);
    assert_bot_inputs(&relays);
    let yaw = |input: &PlayerInput| {
        crate::v026g::drive::half_value(u16::from_le_bytes([input.rotation[0], input.rotation[1]]))
    };
    assert!(
        relays.iter().filter(|input| yaw(input) != 0.0).count() >= 3,
        "{relays:?}"
    );
    assert!(
        relays
            .iter()
            .all(|input| yaw(input).abs() <= 5.0 * 0.05 + 1e-3)
    );

    // The worlds keep showing the body where it stood: it is stuck, and
    // steps back to get out.
    assert!(bench.until(20_000, |bench| {
        bench.clients[0]
            .relays_of(1)
            .iter()
            .any(|input| input.actions & key::BACK != 0)
    }));
    // While it walks the client is asked for a world every 2 s (5 s
    // without bots).
    let asked = |bench: &Bench| {
        bench.clients[0]
            .got(|message| match message {
                ServerMessage::UploadRequest => Some(()),
                _ => None,
            })
            .len()
    };
    let before = asked(&bench);
    bench.run_long(10_000);
    let during = asked(&bench) - before;
    assert!((4..=7).contains(&during), "{during} requests in 10 s");
    assert_eq!(bench.game().resyncs(0), 0);
    bench.assert_no_faults();
}

#[test]
fn a_solo_humans_bot_match_keeps_accepted_actions_for_replay() {
    use crate::v026g::{record, sur_convert};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let dir = std::env::temp_dir().join(format!(
        "srp-solo-pve-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let settings = Settings {
        record: Some(record::RecordSettings {
            dir: dir.clone(),
            min_players: 6,
            max_bytes: 1 << 20,
            replays: None,
        }),
        ..Settings::default()
    };
    let mut bench = Bench::vs_bots(2, MAP_TDM, settings, &[1]);
    bench.join(0);
    assert!(bench.until(2_000, |bench| bench.clients[0].entered() == [0, 1]));
    bench.run(4_000);
    let accepted = bench.clients[0].input(100, 0x8000_0021);
    bench.run(300);
    bench.clients[0].profile_input(1);
    bench.clients[0].input(3_000, 0x1234); // Refused: too far in the future.
    bench.run(300);
    bench.assert_no_faults();
    drop(bench);
    let mut path = None;
    for _ in 0..200 {
        path = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.extension().is_some_and(|e| e == "srprec")
                    && record::read(p).is_ok_and(|r| {
                        r.last().is_some_and(|r| {
                            r.kind == record::NOTE && r.bytes.starts_with(b"{\"dropped\"")
                        })
                    })
            });
        if path.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let path = path.expect("solo PvE recording was kept and closed");
    let records = record::read(&path).unwrap();
    let human: Vec<_> = records
        .iter()
        .filter(|r| r.kind == record::ACCEPTED_INPUT)
        .collect();
    assert_eq!(human.len(), 1, "only the accepted input is saved");
    assert_eq!(human[0].place, 0);
    assert_eq!(&human[0].bytes[4..8], &accepted.rotation);
    assert_eq!(&human[0].bytes[8..], &accepted.actions.to_le_bytes());
    assert!(
        records
            .iter()
            .any(|r| r.kind == record::ACCEPTED_PROFILE && r.place == 0)
    );
    let notes = record::read_notes(&path).unwrap();
    let (file, report) =
        sur_convert::convert_file(&path, &notes, b"config", sur_convert::Settings::default())
            .unwrap();
    assert!(report.inputs > 1, "human and bot inputs are present");
    let chunks = crate::v026g::sur::chunks(&file).unwrap();
    let events = crate::v026g::sur::read_body(
        &survarium::v026g::lzma2::unpack(chunks[2].1, 1 << 20).unwrap(),
    )
    .unwrap();
    assert!(events.iter().any(|r| {
        r.event
            .inputs
            .iter()
            .any(|(place, input)| *place == 0 && input.actions == 0x21)
    }));
    std::fs::remove_dir_all(dir).unwrap();
}
