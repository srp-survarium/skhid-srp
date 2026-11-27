//! Online match end to end without sockets: lobby clients queue, the lobby asks the REAL match hub (`srp::online::Online`)
//! for a match and sends `82`, scripted match clients connect to the hub with what `82` gave them (the order id and the
//! login session), play, leave; the lobby clients report the result (`53` events 5 / 6) and are paid.

#[path = "../../lobby-server/tests/support/mod.rs"]
mod support;
#[path = "../../match-server/tests/common/mod.rs"]
mod mclient;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use game_data::GameData;
use lobby_server::LobbyConfig;
use match_server::world::World;
use match_server::{HubConfig, WorldStore};
use protocol::c2s::{Event53, Request};
use protocol::s2c::*;
use srp::online::Online;
use support::*;

fn pump_all(e: &mut Env, clients: &mut [Client]) {
    for _ in 0..40 {
        let mut refs: Vec<&mut Client> = clients.iter_mut().collect();
        e.step(&mut refs, Duration::from_millis(30));
    }
}

#[test]
fn queue_to_82_to_match_to_rewards() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let row = gd.matches.modes().iter().filter(|r| r.min_players >= 2 && r.variation == 0 && r.mode != game_data::gunrace::MODE).min_by_key(|r| (r.min_players, r.map, r.mode)).unwrap().clone();
    let n = row.min_players as usize;
    queue_to_rewards(row, n);
}

/// The same path for the gun race (docs/notes/gun-race.md), three humans: every seat its own team (the `82` side), the
/// options carry the gun race project and level, `+0x1081` = the equipment level, nine profiles per record; the one
/// client reporting a win (its team = the world's winner byte) is the race winner and everybody is paid.
#[test]
fn a_gun_race_queue_match_goes_through_the_real_hub() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    queue_to_rewards(gd.gunrace.row.clone(), 3);
}

fn queue_to_rewards(row: game_data::ModeRow, n: usize) {
    let gun = row.mode == game_data::gunrace::MODE;

    // the world cache: a test item (a real capture comes from the world host): the 22-byte header, G1 with no roster
    // objects and 100 level bytes (2 objects; the hub puts each match's roster objects in front), then opaque bytes
    let mut item = mclient::T0.to_le_bytes().to_vec();
    item.extend([0u8; 18]);
    item.extend(104u16.to_le_bytes());
    item.extend(2u32.to_le_bytes());
    item.extend((0..100u32).map(|i| (i * 7 % 251) as u8));
    // G2 empty, G3 = 42 opaque bytes, the capture's one kd_stats record (5 zero bytes), 3000 opaque bytes, the team
    // counts 0 / 0 (one record) and 15 more bytes; G4 empty
    item.extend([0, 0]);
    item.extend((42u16 + 5 + 3000 + 16).to_le_bytes());
    item.extend((0..42u32).map(|i| (i * 13 % 251) as u8 | 1));
    item.extend([0u8; 5]);
    item.extend((0..3000u32).map(|i| (i * 31 % 251) as u8));
    item.extend([0u8, 0]);
    item.extend([3u8; 14]);
    item.extend([0, 0]);
    let world = World::from_item(row.map, row.mode, mclient::SEED, mclient::CHECKSUM, 50, 100, item)
        .and_then(|w| w.with_capture_roster(&match_server::RosterObjects::from_spec("").unwrap()))
        .and_then(|w| w.with_record_block(42))
        .and_then(|w| w.with_team_counts(16))
        .unwrap();
    let mut worlds = WorldStore::new();
    worlds.insert(world);
    let online = Arc::new(Online::new(HubConfig { public_host: "match.test".into(), port: 1236, ..Default::default() }, worlds));

    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig {
        achievement_rewards: false,
        daily_quests: false,
        treasure_hunt: false,
        online_queue: Some((row.map, row.mode, row.weather)),
        online_report_grace: Duration::from_secs(5),
        online_min_players: n as u32,
        ..LobbyConfig::default()
    };
    let mut e = Env::with(&dir.path().join("e2e.sqlite"), cfg, Arc::new(lobby_server::MinimalMatches));
    e.server.lobby.online = Some(online.clone());
    let mut lobby: Vec<Client> = vec![];
    for i in 0..n {
        let mut c = e.client(&format!("player{i}"));
        c.send(Request::SignIn { session: c.session, a: 0xFF, b: 0xFF });
        for _ in 0..300 {
            let mut refs: Vec<&mut Client> = lobby.iter_mut().collect();
            refs.push(&mut c);
            e.step(&mut refs, Duration::from_millis(30));
            if c.inbox.iter().any(|(id, _)| *id == 0xB8) {
                break;
            }
        }
        lobby.push(c);
        pump_all(&mut e, &mut lobby);
    }
    for c in lobby.iter_mut() {
        c.take();
    }
    let before: Vec<u32> = lobby.iter().map(|c| e.store.load_profile(c.account).unwrap().silver).collect();

    for i in 0..n {
        lobby[i].send(Request::Queue { global: 0, arg: 0, mode: 0x380, flag: 0 });
        pump_all(&mut e, &mut lobby);
    }
    let mut tickets: Vec<ConnectToMatch> = vec![];
    for c in lobby.iter_mut() {
        let m = c.take();
        let t = m
            .iter()
            .find_map(|x| match x {
                S2c::Raw { id: 0x82, payload } => <ConnectToMatch as protocol::Decode>::from_bytes(payload).ok(),
                _ => None,
            })
            .expect("82 from the lobby");
        tickets.push(t);
    }
    // the match's own id (Hub::new_match_id), the same in every ticket
    let id = tickets[0].match_id;
    assert!(tickets.iter().all(|t| t.host == "match.test" && t.port == 1236 && t.match_id == id));
    assert_ne!(id, mclient::SEED, "not the stored world's id: every match its own");
    assert_eq!(online.hub().lock().unwrap().running_matches(), 1);

    // scripted match clients, one per lobby client, with exactly what 82 and the login gave them
    let mut now = Instant::now() + Duration::from_millis(5);
    let mut mc: Vec<(SocketAddr, mclient::Client)> = vec![];
    for (i, c) in lobby.iter().enumerate() {
        let mut m = mclient::Client::new(i as u64, tickets[i].order_id, c.session, now);
        m.connect();
        mc.push((format!("127.0.0.1:{}", 7000 + i).parse().unwrap(), m));
    }
    let hub = online.hub();
    let mut steps = 0;
    let tick = |now: &mut Instant, mc: &mut Vec<(SocketAddr, mclient::Client)>| {
        *now += Duration::from_millis(20);
        for (addr, m) in mc.iter_mut() {
            for d in m.poll(*now) {
                hub.lock().unwrap().on_datagram(*addr, &d, *now);
            }
        }
        let out = hub.lock().unwrap().poll(*now);
        for (to, d) in out {
            if let Some((_, m)) = mc.iter_mut().find(|(a, _)| *a == to) {
                m.receive(&d, *now);
            }
        }
    };
    while !mc.iter().all(|(_, m)| m.entered) {
        tick(&mut now, &mut mc);
        steps += 1;
        assert!(steps < 6000, "the clients did not all enter: {:?}", mc.iter().map(|(_, m)| (m.dd.is_some(), m.de.is_some(), m.entered)).collect::<Vec<_>>());
    }
    // what DD carried: the online options of this roster (single_player 0, the world's seed and tick, the humans' names)
    let opts = mc[0].1.dd.as_ref().unwrap().options().unwrap();
    assert_eq!((opts[4], opts[5]), (50, n as u8), "tick_ms and player count");
    assert_eq!(u32::from_le_bytes([opts[6], opts[7], opts[8], opts[9]]), id, "the options carry the match's id (the world's seed)");
    assert_eq!(opts[11], 0, "single_player = 0");
    for i in 0..n {
        let name = format!("player{i}");
        assert!(opts.windows(name.len()).any(|w| w == name.as_bytes()), "{name} in the roster");
    }
    assert_eq!(mc[0].1.dd.as_ref().unwrap().level_checksum, mclient::CHECKSUM);
    if gun {
        let mut sides: Vec<u8> = tickets.iter().map(|t| t.side).collect();
        sides.sort();
        assert_eq!(sides, (0..n as u8).collect::<Vec<_>>(), "each seat its own team");
        assert!(opts.windows(16).any(|w| w == b"arena_01/default"), "the gun race level");
        assert!((1..=5).contains(&opts[14]), "+0x1081 = the equipment level");
    }
    // everybody's inputs reach everybody for a while
    for _ in 0..150 {
        tick(&mut now, &mut mc);
    }
    let me0 = mc[0].1.my_index.unwrap();
    let seen_by_last = mc[n - 1].1.seen(me0);
    assert!(!seen_by_last.is_empty(), "inputs of player 0 are relayed to the last player");

    // the match ends: every client closes its connection (no CE: that is the menu leave)
    for (_, m) in mc.iter_mut() {
        m.end_normally();
    }
    for _ in 0..300 {
        tick(&mut now, &mut mc);
    }

    // the lobby clients report (team 0 won) and the lobby pays after the match server's end
    let teams: Vec<u8> = tickets.iter().map(|t| t.side).collect();
    for i in 0..n {
        lobby[i].send(Request::Events(vec![Event53 { id: if teams[i] == 0 { 5 } else { 6 }, x: 0, value: tickets[i].match_id, time: 0 }]));
    }
    pump_all(&mut e, &mut lobby);
    for _ in 0..8 {
        pump_all(&mut e, &mut lobby);
        // the hub's end results are taken by the lobby's tick
        let _ = hub.lock().unwrap().poll(now);
    }
    for (i, c) in lobby.iter_mut().enumerate() {
        let m = c.take();
        assert!(m.contains(&S2c::QueueState(QueueState { state: 0 })), "client {i} gets 85 00");
        let now_silver = e.store.load_profile(c.account).unwrap().silver;
        // no statistics: only the data's drop_after_combat silver roll of the online match level
        let gd = game_data::testing::tables();
        let row = gd.drops.money.iter().find(|r| r.match_level == lobby_server::online::ONLINE_MATCH_LEVEL).unwrap();
        let got = now_silver - before[i];
        // a match of a few seconds: the roll scaled by short_match_factor (duration / 900 s x 3), never above the roll
        assert!(got <= row.max, "client {i} (team {}) paid at most the silver roll: {got}", teams[i]);
        assert!(m.iter().any(|x| x.id() == 0x97), "client {i} gets the results screen (97): paid, not a leaver");
    }
    assert!(e.server.registry.online.is_empty());
}

/// The captured world of pve_01_v1_maze2 (difficulty 0): the full roster (one human, the 17 bots).
fn pve_worlds(gd: &GameData) -> (String, WorldStore) {
    let project = gd.pve.project(0, 2).unwrap();
    // the capture of pve_01_v1_maze2: the full roster (one human, the 17 bots) at the head of G1, 30 level bytes, an
    // opaque G3 (its per-record blocks are not located: the world is fixed to 18 records)
    let me = [local_match::OnlinePlayer { team: 0, human: local_match::HumanLoadout::default_loadout(), hero: None, pve_budget: (0, 0) }];
    let cap = match_server::RosterObjects::from_spec(&local_match::pve_roster_spec(gd, &me, 0).unwrap()).unwrap();
    assert_eq!(cap.records, 18);
    let mut g1 = (cap.count() + 3).to_le_bytes().to_vec();
    g1.extend(cap.bytes());
    g1.extend((0..30u32).map(|i| (i * 7 % 251) as u8));
    let mut item = mclient::T0.to_le_bytes().to_vec();
    item.extend([0u8; 18]);
    item.extend((g1.len() as u16).to_le_bytes());
    item.extend(g1);
    item.extend([0, 0]);
    item.extend(500u16.to_le_bytes());
    item.extend((0..500u32).map(|i| (i * 31 % 251) as u8));
    item.extend([0, 0]);
    let world = World::from_item(15, 7, mclient::SEED, mclient::CHECKSUM, 50, 100, item)
        .and_then(|w| w.with_capture_roster(&cap))
        .and_then(|w| w.with_project(&project))
        .unwrap();
    assert!(world.fixed_roster);
    let mut worlds = WorldStore::new();
    worlds.insert(world);
    (project, worlds)
}

/// An online PvE mission end to end (docs/notes/pve.md): a one-man PvE squad (both seats locked) presses
/// Play; the lobby asks the REAL hub (`Online::create_pve`), which takes the captured world of the difficulty's project
/// (the other maze when only one is captured), builds the options of the human and the 17 script bots and adds the bots
/// as server-owned slots; the scripted match client gets DD with 18 records, enters in the same DF as the 17 bots, which
/// send nothing; his "won" report brings `99` and the drops `83 45 0B`, then the lobby ends the match (closing DFs with
/// code 2: the result screen) and sends `85 00` after his `5F 04`.
#[test]
fn online_pve_mission_with_idle_bots_to_99_and_85() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let (project, worlds) = pve_worlds(&gd);
    let online = Arc::new(Online::new(HubConfig { public_host: "match.test".into(), port: 1236, ..Default::default() }, worlds));

    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, online_report_grace: Duration::from_secs(5), ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("pve.sqlite"), cfg, Arc::new(lobby_server::MinimalMatches));
    e.server.lobby.online = Some(online.clone());
    let mut c = e.client("ranger");
    e.sign_in(&mut c);
    e.store
        .update_profile(c.account, |p| {
            p.add_item(game_data::pve::KEY_PREMIUM, 1);
            Ok::<(), ()>(())
        })
        .unwrap();
    let mut lobby = vec![c];
    lobby[0].send(Request::Squad(protocol::c2s::Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut e, &mut lobby);
    lobby[0].send(Request::Pve(protocol::c2s::Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut e, &mut lobby);
    let t = lobby[0]
        .take()
        .iter()
        .find_map(|x| match x {
            S2c::Raw { id: 0x82, payload } => <ConnectToMatch as protocol::Decode>::from_bytes(payload).ok(),
            _ => None,
        })
        .expect("82 from the lobby");
    assert_eq!(t.side, 0);
    let id = t.match_id;

    let mut now = Instant::now() + Duration::from_millis(5);
    let mut m = mclient::Client::new(0, t.order_id, lobby[0].session, now);
    m.connect();
    let addr: SocketAddr = "127.0.0.1:7100".parse().unwrap();
    let mut mc = vec![(addr, m)];
    let hub = online.hub();
    let tick = |now: &mut Instant, mc: &mut Vec<(SocketAddr, mclient::Client)>| {
        *now += Duration::from_millis(20);
        for (addr, m) in mc.iter_mut() {
            for d in m.poll(*now) {
                hub.lock().unwrap().on_datagram(*addr, &d, *now);
            }
        }
        let out = hub.lock().unwrap().poll(*now);
        for (to, d) in out {
            if let Some((_, m)) = mc.iter_mut().find(|(a, _)| *a == to) {
                m.receive(&d, *now);
            }
        }
    };
    let mut steps = 0;
    while !mc[0].1.entered {
        tick(&mut now, &mut mc);
        steps += 1;
        assert!(steps < 6000, "the client did not enter");
    }
    for _ in 0..100 {
        tick(&mut now, &mut mc);
    }
    // DD: 18 records, the PvE project, difficulty 0, single_player 0
    let opts = mc[0].1.dd.as_ref().unwrap().options().unwrap();
    assert_eq!((opts[5], opts[11], opts[15]), (18, 0, 0));
    assert!(opts.windows(project.len()).any(|w| w == project.as_bytes()));
    assert!(opts.windows(10).any(|w| w == b"pve_bot_17"));
    // the human and the 17 bots entered in one DF; no input of a bot ever
    let c = &mc[0].1;
    let at = c.dfs.iter().position(|d| d.enter_mask != 0).unwrap();
    assert_eq!(c.dfs[at].enter_mask, (1 << 18) - 1);
    assert!(c.dfs.iter().all(|d| d.input_mask & !1 == 0 && d.late.iter().all(|(_, p)| *p == 0)), "bots stay idle");
    let st = hub.lock().unwrap().stats(id).unwrap();
    assert_eq!((st.entered, st.bots), (1, 17));

    // the mission is won: the report brings 99 and the drops, then the match ends and 85 00 follows
    lobby[0].send(Request::Events(vec![Event53 { id: 5, x: 0, value: id, time: 0 }]));
    pump_all(&mut e, &mut lobby);
    let got = lobby[0].take();
    let r = got
        .iter()
        .find_map(|x| match x {
            S2c::Raw { id: 0x99, payload } => <PveResult as protocol::Decode>::from_bytes(payload).ok(),
            _ => None,
        })
        .expect("99");
    assert!(r.success && r.match_id == id && r.players.len() == 1 && r.time_secs <= 5, "{r:?}");
    assert!(got.iter().any(|x| matches!(x, S2c::Reply(Reply::Inventory(Inventory45::PveDrops { .. })))), "83 45 0B");
    for _ in 0..8 {
        pump_all(&mut e, &mut lobby);
        tick(&mut now, &mut mc);
    }
    let fins: Vec<u8> = mc[0].1.dfs.iter().filter(|d| d.leave_mask & 1 != 0).map(|d| d.code).collect();
    assert!(!fins.is_empty(), "the final DF closes the client's match");
    assert!(fins.iter().all(|&k| k == 2), "after the result the closing DFs keep the world (code 2, the result screen): {fins:?}");
    for _ in 0..8 {
        pump_all(&mut e, &mut lobby);
        for _ in 0..50 {
            tick(&mut now, &mut mc);
        }
    }
    assert!(!lobby[0].take().contains(&S2c::QueueState(QueueState { state: 0 })), "no 85 00 while the result screen is open");
    // "Return to lobby" on the result screen: 5F 04
    lobby[0].send(Request::Pve(protocol::c2s::Pve5F::CloseStats));
    pump_all(&mut e, &mut lobby);
    assert!(lobby[0].take().contains(&S2c::QueueState(QueueState { state: 0 })), "85 00");
    assert!(e.server.registry.online.is_empty());
}

/// Lobby client `who` presses Play in his one-man PvE squad (difficulty 0); returns what he got.
fn pve_play(e: &mut Env, lobby: &mut [Client], who: usize) -> Vec<S2c> {
    lobby[who].send(Request::Squad(protocol::c2s::Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(e, lobby);
    lobby[who].take();
    lobby[who].send(Request::Pve(protocol::c2s::Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(e, lobby);
    lobby[who].take()
}

fn connect_of(m: &[S2c]) -> Option<ConnectToMatch> {
    m.iter().find_map(|x| match x {
        S2c::Raw { id: 0x82, payload } => <ConnectToMatch as protocol::Decode>::from_bytes(payload).ok(),
        _ => None,
    })
}

/// With the REAL hub: two missions of one maze, one after the other, each with its own match id (not the captured
/// world's id): both are stored as runs and both get `99` and the end screen's
/// `83 45 0B`. While the first match is still on its result screen in the lobby, a second squad's Play for the same maze
/// starts its own match (the same map twice at once); that match ends before anybody entered, and its
/// key is given back.
#[test]
fn two_pve_missions_of_one_world_are_two_runs_with_their_own_end_screens() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let (_, worlds) = pve_worlds(&gd);
    let online = Arc::new(Online::new(HubConfig { public_host: "match.test".into(), port: 1236, ..Default::default() }, worlds));
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, online_report_grace: Duration::from_secs(5), ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("pve.sqlite"), cfg, Arc::new(lobby_server::MinimalMatches));
    e.server.lobby.online = Some(online.clone());
    let mut lobby = vec![];
    for name in ["ranger", "scout"] {
        let mut c = e.client(name);
        e.sign_in(&mut c);
        e.store
            .update_profile(c.account, |p| {
                p.add_item(game_data::pve::KEY_PREMIUM, 2);
                Ok::<(), ()>(())
            })
            .unwrap();
        lobby.push(c);
    }
    let hub = online.hub();
    let mut now = Instant::now() + Duration::from_millis(5);
    let mut port = 7200u16;
    let mut runs = vec![];
    for round in 0..2 {
        let t = connect_of(&pve_play(&mut e, &mut lobby, 0)).expect("82");
        let id = t.match_id;
        assert!(!runs.iter().any(|&r: &i64| r == i64::from(id)), "round {round}: a new id");
        port += 1;
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        let mut m = mclient::Client::new(0, t.order_id, lobby[0].session, now);
        m.connect();
        let mut steps = 0;
        let step = |now: &mut Instant, m: &mut mclient::Client| {
            *now += Duration::from_millis(20);
            for d in m.poll(*now) {
                hub.lock().unwrap().on_datagram(addr, &d, *now);
            }
            let out = hub.lock().unwrap().poll(*now);
            for (to, d) in out {
                if to == addr {
                    m.receive(&d, *now);
                }
            }
        };
        while !m.entered {
            step(&mut now, &mut m);
            steps += 1;
            assert!(steps < 6000, "round {round}: the client did not enter");
        }
        // round 0 wins, round 1 fails (the client's "lost" report)
        lobby[0].send(Request::Events(vec![Event53 { id: if round == 0 { 5 } else { 6 }, x: 0, value: id, time: 0 }]));
        pump_all(&mut e, &mut lobby);
        let got = lobby[0].take();
        let r = got
            .iter()
            .find_map(|x| match x {
                S2c::Raw { id: 0x99, payload } => <PveResult as protocol::Decode>::from_bytes(payload).ok(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("round {round}: 99"));
        assert_eq!((r.match_id, r.success), (id, round == 0), "round {round}");
        assert!(got.iter().any(|x| matches!(x, S2c::Reply(Reply::Inventory(Inventory45::PveDrops { .. })))), "round {round}: 83 45 0B");
        runs.push(e.store.pve_latest_run(lobby[0].account).unwrap().unwrap());
        if round == 0 {
            // the second player's Play for the same maze while the first one's result screen is up: its own match
            let scout = lobby[1].account;
            let keys = |e: &Env, _k: usize| e.store.load_profile(scout).unwrap().items.iter().filter(|i| i.dict_id == game_data::pve::KEY_PREMIUM).map(|i| i.count).sum::<u32>();
            let t1 = connect_of(&pve_play(&mut e, &mut lobby, 1)).expect("82: a second match of the same maze");
            assert_ne!(t1.match_id, id, "its own id");
            assert_eq!(hub.lock().unwrap().running_matches(), 2, "two matches of one maze at once");
            assert_eq!(keys(&e, 1), 1, "the key paid the entry");
            // nobody connects to it: it ends before anybody entered, and the key comes back
            assert!(hub.lock().unwrap().end_match(t1.match_id, now));
            for _ in 0..8 {
                let _ = hub.lock().unwrap().poll(now);
                pump_all(&mut e, &mut lobby);
            }
            assert_eq!(keys(&e, 1), 2, "nobody entered: the key is given back");
            lobby[1].take();
        }
        // the match closes (code 2), the player leaves the screen: 85 00, the world is free again
        for _ in 0..8 {
            pump_all(&mut e, &mut lobby);
            for _ in 0..50 {
                step(&mut now, &mut m);
            }
        }
        lobby[0].send(Request::Pve(protocol::c2s::Pve5F::CloseStats));
        pump_all(&mut e, &mut lobby);
        assert!(lobby[0].take().contains(&S2c::QueueState(QueueState { state: 0 })), "round {round}: 85 00");
        assert!(e.server.registry.online.is_empty());
        for _ in 0..200 {
            step(&mut now, &mut m);
        }
    }
    assert_eq!(runs.len(), 2);
    assert_ne!(runs[0], runs[1], "two runs, their own keys (their own match ids)");
    assert_eq!(e.store.pve_result_ids_of(lobby[0].account).unwrap().len(), 2);
    let (second, _) = e.store.pve_result(runs[1]).unwrap().unwrap();
    assert_eq!((i64::from(second.match_id), second.success), (runs[1], false));
}

/// Paid respawns (docs/notes/pve.md): with a world host configured, the human's options
/// record carries his spendable access keys after the entry fee (+0x2D0) and his gold balance (+0x2D4); without one the
/// budget stays 0 (the spends could not be charged). The respawn price (+0x2F8) comes from the data either way.
#[test]
fn a_pve_respawn_budget_only_with_a_world_host() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    for host in [false, true] {
        let (_, worlds) = pve_worlds(&gd);
        let mut hc = HubConfig { public_host: "match.test".into(), port: 1236, ..Default::default() };
        if host {
            // a host that is never reachable: the budget depends on the switch, the link itself goes off
            let none: match_server::Connector = Arc::new(|| Err(std::io::Error::other("no world host in this test")));
            hc.match_config.world_host = Some(match_server::HostConfig::with_connector("none", none));
        }
        let online = Arc::new(Online::new(hc, worlds));
        let dir = tempfile::tempdir().unwrap();
        let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() };
        let mut e = Env::with(&dir.path().join("pve.sqlite"), cfg, Arc::new(lobby_server::MinimalMatches));
        e.server.lobby.online = Some(online.clone());
        let mut c = e.client("ranger");
        e.sign_in(&mut c);
        e.store
            .update_profile(c.account, |p| {
                p.add_item(game_data::pve::KEY_PREMIUM, 2);
                p.add_item(game_data::pve::KEY_FREE, 1);
                p.gold = 500;
                Ok::<(), ()>(())
            })
            .unwrap();
        let mut lobby = vec![c];
        lobby[0].send(Request::Squad(protocol::c2s::Squad49::PveState { state: 0x30, zero: 0xFF }));
        pump_all(&mut e, &mut lobby);
        lobby[0].send(Request::Pve(protocol::c2s::Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
        pump_all(&mut e, &mut lobby);
        let t = lobby[0]
            .take()
            .iter()
            .find_map(|x| match x {
                S2c::Raw { id: 0x82, payload } => <ConnectToMatch as protocol::Decode>::from_bytes(payload).ok(),
                _ => None,
            })
            .expect("82");
        let mut now = Instant::now();
        let mut m = mclient::Client::new(0, t.order_id, lobby[0].session, now);
        m.connect();
        let addr: SocketAddr = "127.0.0.1:7100".parse().unwrap();
        let hub = online.hub();
        for _ in 0..500 {
            if m.dd.is_some() {
                break;
            }
            now += Duration::from_millis(20);
            for d in m.poll(now) {
                hub.lock().unwrap().on_datagram(addr, &d, now);
            }
            let out = hub.lock().unwrap().poll(now);
            for (_, d) in out {
                m.receive(&d, now);
            }
        }
        let opts = m.dd.as_ref().expect("DD").options().unwrap();
        // the human record: name, clan, id, games, u32, ai id, 3 bytes, then keys / gold / price
        let h = opts.windows(7).position(|w| w == b"\x06ranger").expect("the human record");
        let b = h + 7 + 1 + 8 + 8 + 4 + 3;
        let keys = u32::from_le_bytes(opts[b..b + 4].try_into().unwrap());
        let gold = u32::from_le_bytes(opts[b + 4..b + 8].try_into().unwrap());
        let price = u16::from_le_bytes([opts[b + 8], opts[b + 9]]);
        let fee_left = e.store.load_profile(lobby[0].account).unwrap();
        let held: u32 = fee_left.items.iter().filter(|i| [game_data::pve::KEY_FREE, game_data::pve::KEY_PREMIUM].contains(&i.dict_id)).map(|i| i.count).sum();
        assert_eq!(held, 2, "the entry fee took one key (the free one first)");
        if host {
            // 500 gold minus the two locked seats' block-slot price: the balance after every start charge
            assert_eq!((keys, gold), (2, fee_left.gold), "the keys left after the fee and the gold balance");
            assert!(gold > 0 && gold < 500);
        } else {
            assert_eq!((keys, gold), (0, 0), "no world host: no budget");
        }
        assert_eq!(price as u32, gd.pve.prices.respawn_price);
    }
}

/// A stored test world of `row` (the 22-byte header, G1 without roster objects, opaque G3, the team counts), as
/// `queue_to_82_to_match_to_rewards` builds it.
fn stored_world(row: &game_data::matches::ModeRow) -> World {
    let mut item = mclient::T0.to_le_bytes().to_vec();
    item.extend([0u8; 18]);
    item.extend(104u16.to_le_bytes());
    item.extend(2u32.to_le_bytes());
    item.extend((0..100u32).map(|i| (i * 7 % 251) as u8));
    item.extend([0, 0]);
    item.extend((42u16 + 5 + 3000 + 16).to_le_bytes());
    item.extend((0..42u32).map(|i| (i * 13 % 251) as u8 | 1));
    item.extend([0u8; 5]);
    item.extend((0..3000u32).map(|i| (i * 31 % 251) as u8));
    item.extend([0u8, 0]);
    item.extend([3u8; 14]);
    item.extend([0, 0]);
    World::from_item(row.map, row.mode, mclient::SEED, mclient::CHECKSUM, 50, 100, item)
        .and_then(|w| w.with_capture_roster(&match_server::RosterObjects::from_spec("").unwrap()))
        .and_then(|w| w.with_record_block(42))
        .and_then(|w| w.with_team_counts(16))
        .unwrap()
}

/// PvP bots through the REAL hub: a lone searcher whose bot wait passed gets a tdm match with 11 AI players when the match
/// server has a world host (it always drives the AI players); the DD options carry 12 records, the AI players after the
/// human with distinct ai ids below 36 and the synthetic account ids; without a world host he keeps searching.
#[test]
fn a_lone_searcher_gets_a_match_with_ai_players_from_the_hub() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let row = gd.matches.modes().iter().filter(|r| r.project.ends_with("_tdm") && r.min_players >= 12).min_by_key(|r| (r.map, r.variation)).unwrap().clone();
    for bots in [true, false] {
        let mut worlds = WorldStore::new();
        worlds.insert(stored_world(&row));
        let mut hc = HubConfig { public_host: "match.test".into(), port: 1236, ..Default::default() };
        // a host that is never reachable: the stored world is used, the link goes off; only its presence matters here
        if bots {
            let none: match_server::Connector = Arc::new(|| Err(std::io::Error::other("no world host in this test")));
            hc.match_config.world_host = Some(match_server::HostConfig::with_connector("none", none));
        }
        let online = Arc::new(Online::new(hc, worlds));
        let dir = tempfile::tempdir().unwrap();
        let cfg = LobbyConfig {
            achievement_rewards: false,
            daily_quests: false,
            treasure_hunt: false,
                online_queue: Some((row.map, row.mode, row.variation)),
            online_min_players: 2,
            online_bot_wait: Some(Duration::ZERO),
            ..LobbyConfig::default()
        };
        let mut e = Env::with(&dir.path().join("b4.sqlite"), cfg, Arc::new(lobby_server::MinimalMatches));
        e.server.lobby.online = Some(online.clone());
        let mut c = e.client("solo");
        e.sign_in(&mut c);
        let mut lobby = vec![c];
        lobby[0].take();
        lobby[0].send(Request::Queue { global: 0, arg: 0, mode: 0x380, flag: 0 });
        pump_all(&mut e, &mut lobby);
        let t = connect_of(&lobby[0].take());
        if !bots {
            assert!(t.is_none(), "no bot-driving host: still searching");
            assert_eq!(e.server.registry.queue.len(), 1);
            continue;
        }
        let t = t.expect("82 for the bot-filled match");
        assert_eq!(t.side, 0);
        let mut now = Instant::now();
        let mut m = mclient::Client::new(0, t.order_id, lobby[0].session, now);
        m.connect();
        let addr: SocketAddr = "127.0.0.1:7101".parse().unwrap();
        let hub = online.hub();
        for _ in 0..500 {
            if m.dd.is_some() {
                break;
            }
            now += Duration::from_millis(20);
            for d in m.poll(now) {
                hub.lock().unwrap().on_datagram(addr, &d, now);
            }
            let out = hub.lock().unwrap().poll(now);
            for (_, d) in out {
                m.receive(&d, now);
            }
        }
        let opts = m.dd.as_ref().expect("DD").options().unwrap();
        assert_eq!(opts[5], 12, "1 human + 11 AI players");
        let mut ais = std::collections::BTreeSet::new();
        for k in 0..11u64 {
            let id = ((0xB07u64 << 32) | (k + 1)).to_le_bytes();
            let at = opts.windows(8).position(|w| w == id).unwrap_or_else(|| panic!("AI record {k}"));
            let ai = u32::from_le_bytes(opts[at + 16..at + 20].try_into().unwrap());
            assert!(ai < gd.matches.ai_players().len() as u32);
            ais.insert(ai);
        }
        assert_eq!(ais.len(), 11, "each AI player once");
        assert!(e.server.registry.queue.is_empty());
    }
}

/// The world host names the projects it serves (its answer to the project-list query, `world_host::fetch_projects`):
/// the hub serves a PvP row only when its level project is one of them (variation and weather included),
/// refuses a match on any other row before the world host is asked, and the sign-in `AC` offers the check-box modes of
/// the served rows. Without a host, a stored world serves only its capture's row (variation 0, day).
#[test]
fn the_world_hosts_projects_decide_what_the_queue_offers() {
    game_data::require_tables!();
    let projects: Vec<String> = ["level_01_day_tdm", "level_05_day_br", "level_05_day_slr_vn", "level_05_day_ah", "pve_01_v1_maze1"].map(String::from).to_vec();
    let gd = game_data::testing::tables();
    let row = |p: &str| gd.matches.modes().iter().find(|r| r.project == p).unwrap_or_else(|| panic!("row {p}")).clone();
    let mut hc = HubConfig { public_host: "match.test".into(), port: 1236, ..Default::default() };
    let none: match_server::Connector = Arc::new(|| Err(std::io::Error::other("no world host in this test")));
    hc.match_config.world_host = Some(match_server::HostConfig::with_connector("none", none));
    let online = Arc::new(Online::new(hc, WorldStore::new()).with_host_projects(projects));
    use lobby_server::OnlineMatches;
    assert!(online.serves(&gd, &row("level_05_day_br")));
    assert!(online.serves(&gd, &row("level_05_day_slr_vn")));
    assert!(!online.serves(&gd, &row("level_03_rain_slr_vn")), "a rain project the host does not list");
    assert!(!online.serves(&gd, &row("level_02_day_tdm")));
    let e = online.create_row(&gd, &row("level_02_day_tdm"), 0, &[], None).unwrap_err();
    assert!(e.contains("does not serve level_02_day_tdm"), "{e}");

    // the sign-in AC: tdm, battery, slaughter (ah has no check box)
    let dir = tempfile::tempdir().unwrap();
    let lc = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("gate.sqlite"), lc, Arc::new(lobby_server::MinimalMatches));
    e.server.lobby.online = Some(online.clone());
    let mut c = e.client("gate");
    let gate = e.sign_in(&mut c).iter().find_map(|x| if let S2c::PlayGate(g) = x { Some(g.mode_mask) } else { None });
    assert_eq!(gate, Some(0b100_0011));

    // no host: a stored world of level_02 tdm serves its day row of variation 0 only
    let r = row("level_02_day_tdm");
    let mut worlds = WorldStore::new();
    worlds.insert(stored_world(&r));
    let stored = Online::new(HubConfig { public_host: "match.test".into(), port: 1236, ..Default::default() }, worlds);
    assert!(stored.serves(&gd, &r));
    assert!(!stored.serves(&gd, &row("level_02_day_tdm_v1")));
    assert!(!stored.serves(&gd, &row("level_01_day_tdm")));
}
