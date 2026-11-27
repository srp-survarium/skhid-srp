//! Integration tests: a hub with scripted Client-role transports connected by an in-memory "network" under a virtual
//! clock (no sockets, no sleeping).

mod common;

use common::*;
use match_server::match_msgs::*;
use match_server::{EndReason, EndRule, Hub, HubConfig, LeaveReason, MatchConfig, MatchSpec, MatchTicket, PlayerSpec, PveEndRule, WorldStore};
use protocol::Decode;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

struct Net {
    hub: Hub,
    clients: Vec<(SocketAddr, Client)>,
    now: Instant,
}

fn player(i: u64, session: Option<u32>) -> PlayerSpec {
    PlayerSpec { account_id: 100 + i, name: format!("p{i}"), team: (i % 2) as u8, session, bot: false }
}

fn spec(n: u64, session: Option<u32>) -> MatchSpec {
    MatchSpec {
        end_rule: None,
        pve_end: None,
        ai_ignore_players: false,
        roster: None,
        project: None,
        match_id: Some(SEED),
        map: MAP,
        mode: MODE,
        options: test_options(),
        players: (0..n).map(|i| player(i, session)).collect(),
        level_checksum: None,
    }
}

fn hub(mc: MatchConfig) -> Hub {
    let mut worlds = WorldStore::new();
    worlds.insert(test_world());
    Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds)
}

fn addr(i: u16) -> SocketAddr {
    format!("127.0.0.1:{}", 5000 + i).parse().unwrap()
}

impl Net {
    fn new(mc: MatchConfig) -> Self {
        Net { hub: hub(mc), clients: vec![], now: Instant::now() }
    }

    fn add_client(&mut self, idx: u64, order_id: u32, session: u32) {
        let mut c = Client::new(idx, order_id, session, self.now);
        c.connect();
        self.clients.push((addr(idx as u16), c));
    }

    fn create(&mut self, n: u64, session: Option<u32>) -> MatchTicket {
        let t = self.hub.create_match(&spec(n, session), self.now).unwrap();
        assert_eq!(t.port, 1236);
        t
    }

    fn step(&mut self, ms: u64) {
        self.now += Duration::from_millis(ms);
        let now = self.now;
        for (a, c) in self.clients.iter_mut() {
            for d in c.poll(now) {
                self.hub.on_datagram(*a, &d, now);
            }
        }
        for (to, d) in self.hub.poll(now) {
            if let Some((_, c)) = self.clients.iter_mut().find(|(a, _)| *a == to) {
                c.receive(&d, now);
            }
        }
    }

    fn run(&mut self, ms: u64) {
        for _ in 0..ms / 5 {
            self.step(5);
        }
    }
}

fn two_player_match(mc: MatchConfig) -> (Net, MatchTicket) {
    let mut net = Net::new(mc);
    let t = net.create(2, None);
    for p in &t.players {
        net.add_client(u64::from(p.slot), p.order_id, 9000 + u32::from(p.slot));
    }
    (net, t)
}

/// A two-player match with a short time limit (the relay mirrors the clients' timelimit rule).
fn timed_match(rule: EndRule) -> Net {
    let mut net = Net::new(MatchConfig::default());
    let mut s = spec(2, None);
    s.end_rule = Some(rule);
    let t = net.hub.create_match(&s, net.now).unwrap();
    for p in &t.players {
        net.add_client(u64::from(p.slot), p.order_id, 9000 + u32::from(p.slot));
    }
    net
}

/// A client never ends an online match by itself; the relay closes it at the clients' own end: countdown from the
/// enter DF, match_time, then 3 s (state 5), with every recipient's bit in the leave mask (close_current_match(3)).
#[test]
fn the_relay_closes_the_match_at_the_time_limit() {
    let rule = EndRule { countdown_ms: 200, match_ms: 1000, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: true, draw_overtime_ms: 0 };
    let mut net = timed_match(rule);
    net.run(6000);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].reason, EndReason::TimeLimit);
    for (_, c) in &net.clients {
        let enter = c.dfs.iter().find(|d| d.enter_mask != 0).unwrap().time;
        let fin: Vec<u32> = c.dfs.iter().filter(|d| d.leave_mask & (1 << c.idx) != 0).map(|d| d.time).collect();
        assert!(!fin.is_empty() && c.closed, "client {} closed by its own leave bit", c.idx);
        assert!(fin[0] >= enter + 200 + 1000 + 3000 && fin[0] < enter + 200 + 1000 + 3000 + 100, "{fin:?} enter {enter}");
    }
}

/// A leave that empties a team cuts the clients' remaining time to advantage_time (on_leave 0x200365D00): the relay
/// closes the others then, not at the full time limit.
#[test]
fn a_leave_that_empties_a_team_cuts_to_the_advantage_time() {
    let rule = EndRule { countdown_ms: 200, match_ms: 60_000, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: false, draw_overtime_ms: 0 };
    let mut net = timed_match(rule);
    net.run(1500);
    net.clients[1].1.leave();
    net.run(5000);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].reason, EndReason::TimeLimit, "the advantage end, long before the 60 s limit");
    assert_eq!(ended[0].winner, None, "the timelimit rule alone names no winner");
    assert!(net.clients[0].1.closed);
}

/// TDM: a leave that leaves a team with nobody ends the clients' match 3 s later and the other team wins (tdm check
/// 0x200366DB0); the relay closes the rest then and reports that winner (no client message carries one online).
#[test]
fn an_empty_tdm_team_ends_the_match_and_the_other_team_wins() {
    let rule = EndRule { countdown_ms: 200, match_ms: 60_000, advantage_ms: 30_000, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: true, draw_overtime_ms: 0 };
    let mut net = timed_match(rule);
    net.run(1500);
    net.clients[1].1.leave();
    net.run(4500);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1, "closed 3 s after the leave, not after the 30 s advantage");
    assert_eq!(ended[0].winner, Some(0));
    assert!(net.clients[0].1.closed);
}

/// The six PvE projects share (map 15, mode 7): a match names its project and gets that project's stored world (a hub
/// without a world host); an unknown project is refused; a PvE (fixed-roster) world refuses a roster of another record
/// count with a clear error. The match id is the options' (every match its own): two matches of one world run at once,
/// one id twice is refused, and an id that differs from the options' is refused.
#[test]
fn pve_worlds_are_kept_per_project() {
    let mut worlds = WorldStore::new();
    worlds.insert(test_world());
    for (k, p) in ["pve_01_v1_maze1", "pve_01_v2_maze2"].iter().enumerate() {
        let mut w = test_world().with_project(p).unwrap();
        (w.map, w.mode, w.match_id, w.records) = (15, 7, 0x5EED_0000 + k as u32, 18);
        worlds.insert(w);
    }
    let mut hub = Hub::new(HubConfig { port: 1236, ..Default::default() }, worlds);
    assert!(hub.can_create(15, 7, Some("pve_01_v2_maze2")));
    assert!(!hub.can_create(15, 7, Some("pve_01_v3_maze1")));
    assert!(hub.can_create(15, 7, None), "some PvE world exists");
    assert!(hub.can_create(MAP, MODE, None));
    let pve = |project: &str, roster: Option<&str>, id: u32| MatchSpec {
        end_rule: None,
        pve_end: None,
        ai_ignore_players: false,
        match_id: None,
        map: 15,
        mode: 7,
        options: options_with_id(id),
        players: vec![player(0, None)],
        level_checksum: None,
        roster: roster.map(|r| match_server::RosterObjects::from_spec(r).unwrap()),
        project: Some(project.into()),
    };
    let now = Instant::now();
    let a = hub.new_match_id();
    let b = hub.new_match_id();
    assert_ne!(a, b);
    assert!((0x1000_0000..0x8000_0000).contains(&a) && (0x1000_0000..0x8000_0000).contains(&b));
    assert_eq!(hub.create_match(&pve("pve_01_v2_maze2", None, a), now).unwrap().match_id, a);
    assert_eq!(hub.create_match(&pve("pve_01_v2_maze2", None, b), now).unwrap().match_id, b, "a second match of the same world at once");
    assert_eq!(hub.create_match(&pve("pve_01_v2_maze2", None, a), now).unwrap_err(), match_server::CreateError::IdInUse(a));
    let mut wrong = pve("pve_01_v2_maze2", None, 0x2222_0000);
    wrong.match_id = Some(0x2222_0001);
    assert_eq!(hub.create_match(&wrong, now).unwrap_err(), match_server::CreateError::OptionsMatchId(0x2222_0001, 0x2222_0000));
    assert_eq!(hub.running_matches(), 2);
    assert!(matches!(hub.create_match(&pve("pve_01_v3_maze1", None, 0x3333_0000), now), Err(match_server::CreateError::NoProjectWorld(_))));
    let e = hub.create_match(&pve("pve_01_v1_maze1", Some("MG5|MG5"), 0x4444_0000), now).unwrap_err().to_string();
    assert!(e.contains("serves only its capture's roster of 18 records"), "{e}");
}

/// A PvE mission: one human and server-owned bot slots. Only the human gets a ticket; the
/// match starts when he is ready; the bots enter in the same DF as the human, with no start profile and no input; the
/// match time runs from the enter; the human's leave ends the match (bots do not keep it running) and the result lists
/// only him.
#[test]
fn server_owned_bot_slots_enter_with_the_human_and_stay_idle() {
    let mut net = Net::new(MatchConfig::default());
    let mut s = spec(1, None);
    s.players.extend((1..=3).map(|k| PlayerSpec::bot(&format!("pve_bot_{k}"), 1)));
    let t = net.hub.create_match(&s, net.now).unwrap();
    assert_eq!(t.players.len(), 1, "a ticket for the human only");
    assert_eq!(t.players[0].slot, 0);
    assert!(net.hub.played(SEED).is_none(), "no match time before the enter");
    net.add_client(0, t.players[0].order_id, 9000);
    net.run(3000);
    let c = &net.clients[0].1;
    assert_eq!(c.dd.as_ref().unwrap().local_player_index, 0);
    let at = c.dfs.iter().position(|d| d.enter_mask != 0).expect("an enter");
    assert_eq!(c.dfs[at].enter_mask, 0b1111, "the human and the three bots enter in one DF");
    assert_eq!(c.dfs[at].profile.iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![0], "a start profile for the human only");
    assert!(c.dfs.iter().all(|d| d.input_mask == 0 && d.late.is_empty()), "nobody sends inputs for the bots");
    assert!(c.entered);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.phase, st.entered, st.bots, st.ready), ("running", 1, 3, 1));
    let played = net.hub.played(SEED).expect("match time after the enter");
    assert!(played > Duration::from_millis(1000) && played < Duration::from_millis(3000), "{played:?}");
    net.clients[0].1.leave();
    net.run(300);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].reason, EndReason::AllLeft, "the bots do not keep the match running");
    assert_eq!(ended[0].players.iter().map(|p| p.slot).collect::<Vec<_>>(), vec![0], "the result lists the human only");
    // bots before a human, or bots alone, are refused
    let mut bad = spec(1, None);
    bad.players.insert(0, PlayerSpec::bot("pve_bot_1", 1));
    assert!(matches!(hub(MatchConfig::default()).create_match(&bad, net.now), Err(match_server::CreateError::PlayerCount)));
    let mut alone = spec(0, None);
    alone.players.push(PlayerSpec::bot("pve_bot_1", 1));
    assert!(matches!(hub(MatchConfig::default()).create_match(&alone, net.now), Err(match_server::CreateError::PlayerCount)));
}

/// A PvE mission of `humans` human slots (team 0; the first `connected` of them connect) and 3 server-owned bots, with
/// the mission script's failure ends mirrored by `rule` (short times: the real ones are `PveEndRule::PVE_LUA`). The relay
/// closes at its mirrored end (no hand-over to a lobby: `pve_handover` None, the exact-step tests below).
fn pve_match(humans: u64, connected: usize, rule: PveEndRule, mc: MatchConfig) -> Net {
    pve_match_with(humans, connected, rule, MatchConfig { pve_handover: None, ..mc }).0
}

/// [`pve_match`] with the config as given, and the match id.
fn pve_match_with(humans: u64, connected: usize, rule: PveEndRule, mc: MatchConfig) -> (Net, u32) {
    let mut net = Net::new(mc);
    let mut s = spec(humans, None);
    for p in &mut s.players {
        p.team = 0;
    }
    s.players.extend((1..=3).map(|k| PlayerSpec::bot(&format!("pve_bot_{k}"), 1)));
    s.pve_end = Some(rule);
    let t = net.hub.create_match(&s, net.now).unwrap();
    for p in t.players.iter().take(connected) {
        net.add_client(u64::from(p.slot), p.order_id, 9000 + u32::from(p.slot));
    }
    (net, t.match_id)
}

fn enter_df(c: &Client) -> u32 {
    c.dfs.iter().find(|d| d.enter_mask != 0).expect("an enter").time
}

/// Time of the first DF carrying the client's own leave bit (the close).
fn fin_df(c: &Client) -> Option<u32> {
    c.dfs.iter().find(|d| d.leave_mask & (1 << c.idx) != 0).map(|d| d.time)
}

/// Time of the DF carrying the leave of `slot`.
fn leave_df(c: &Client, slot: u32) -> u32 {
    c.dfs.iter().find(|d| d.leave_mask & (1 << slot) != 0).expect("the leave").time
}

/// Every human entered: `all_players_ready` starts the mission `start_all` after the enter DF, `start_match_instantly`
/// ends it `match` later with action 26 {success true, winner_team 1} (state 5): the relay closes the client with the
/// first DF after that step and reports a failure (winner 1). The server's backstop does not cut a PvE mission short.
#[test]
fn a_pve_mission_ends_at_the_scripts_mission_timeout_as_a_failure() {
    let rule = PveEndRule { cancel_ms: 60_000, cancel2_ms: 30_000, start_all_ms: 500, start_two_of_three_ms: 800, match_ms: 2000 };
    let mc = MatchConfig { max_duration: Duration::from_millis(1000), ..MatchConfig::default() };
    let mut net = pve_match(1, 1, rule, mc);
    net.run(6000);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::PveTimeout, Some(1)));
    let c = &net.clients[0].1;
    assert_eq!(fin_df(c), Some(enter_df(c) + 500 + 2000 + TICK));
    assert!(c.closed);
}

/// 2 of a 3-human mission entered, nobody left: `2_from_3_players_ready` starts it `start_two_of_three` after the enter.
#[test]
fn two_of_three_humans_start_after_the_longer_wait() {
    let rule = PveEndRule { cancel_ms: 60_000, cancel2_ms: 30_000, start_all_ms: 500, start_two_of_three_ms: 800, match_ms: 2000 };
    let mc = MatchConfig { ready_timeout: Duration::from_millis(300), ..MatchConfig::default() };
    let mut net = pve_match(3, 2, rule, mc);
    net.run(6000);
    let ended = net.hub.take_ended();
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::PveTimeout, Some(1)));
    for (_, c) in &net.clients {
        assert_eq!(fin_df(c), Some(enter_df(c) + 800 + 2000 + TICK));
    }
}

/// No start (1 of 2 humans entered): `wait_before_cancel`, started by `match` in the first step after the DE
/// (world time + tick), cancels the mission `cancel` later (action 26 {false, 1}: state 4).
#[test]
fn without_a_start_the_script_cancels_the_mission() {
    let rule = PveEndRule { cancel_ms: 3000, cancel2_ms: 30_000, start_all_ms: 500, start_two_of_three_ms: 800, match_ms: 60_000 };
    let mc = MatchConfig { ready_timeout: Duration::from_millis(300), ..MatchConfig::default() };
    let mut net = pve_match(2, 1, rule, mc);
    net.run(5000);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::PveCancel, Some(1)));
    let c = &net.clients[0].1;
    assert!(c.entered);
    assert_eq!(fin_df(c), Some(T0 + TICK + 3000 + TICK));
}

/// The hand-over (default `pve_handover`): the mirrored cancel does not close the client. Its end goes to the lobby as an
/// outcome (state 4, winner 1, no counters, the mission time from the enter), the DFs go on; the lobby's end after its
/// result closes the client with DF code 2 (the client keeps its ended world: the result screen gets the pointer) and
/// the result is the script's failure (winner 1).
#[test]
fn a_mirrored_failure_end_goes_to_the_lobby_and_its_end_keeps_the_world() {
    let rule = PveEndRule { cancel_ms: 3000, cancel2_ms: 30_000, start_all_ms: 500, start_two_of_three_ms: 800, match_ms: 60_000 };
    let mc = MatchConfig { ready_timeout: Duration::from_millis(300), ..MatchConfig::default() };
    let (mut net, _) = pve_match_with(2, 1, rule, mc);
    net.run(5000);
    assert!(net.hub.take_ended().is_empty(), "no close at the mirrored end");
    let out = net.hub.take_outcomes();
    assert_eq!(out.len(), 1);
    let o = &out[0];
    let c = &net.clients[0].1;
    assert_eq!((o.state, o.winner_team, o.pve_success, o.players.len()), (4, 1, false, 0));
    assert_eq!(o.t_end, T0 + TICK + 3000);
    assert_eq!(o.mission_ms, o.t_end - enter_df(c));
    assert!(!c.closed && fin_df(c).is_none());
    let last = c.dfs.last().unwrap().time;
    assert!(last > o.t_end + 1500, "the DFs go on after the end ({last})");
    net.run(1000);
    assert!(net.hub.take_outcomes().is_empty(), "once");
    assert!(net.hub.end_match_after_result(o.match_id, net.now));
    net.run(200);
    let ended = net.hub.take_ended();
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::Requested, Some(1)));
    let c = &net.clients[0].1;
    assert!(c.closed);
    let fins: Vec<u8> = c.dfs.iter().filter(|d| d.leave_mask & (1 << c.idx) != 0).map(|d| d.code).collect();
    assert!(!fins.is_empty() && fins.iter().all(|&k| k == 2), "closing DFs with code 2: {fins:?}");
    assert!(c.dfs.iter().filter(|d| d.leave_mask & (1 << c.idx) == 0).all(|d| d.code == 0xFF), "the others plain");
}

/// The hand-over's backstop: no lobby end within `pve_handover` of the mirrored end -> the relay closes (code 2, the
/// world ended) with the mirrored reason.
#[test]
fn a_handed_over_failure_end_closes_at_the_backstop() {
    let rule = PveEndRule { cancel_ms: 3000, cancel2_ms: 30_000, start_all_ms: 500, start_two_of_three_ms: 800, match_ms: 60_000 };
    let mc = MatchConfig { ready_timeout: Duration::from_millis(300), pve_handover: Some(Duration::from_secs(2)), ..MatchConfig::default() };
    let (mut net, _) = pve_match_with(2, 1, rule, mc);
    net.run(4500);
    assert_eq!(net.hub.take_outcomes().len(), 1);
    assert!(net.hub.take_ended().is_empty());
    net.run(2000);
    let ended = net.hub.take_ended();
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::PveCancel, Some(1)));
    let c = &net.clients[0].1;
    let fin = c.dfs.iter().find(|d| d.leave_mask & (1 << c.idx) != 0).expect("closed");
    assert_eq!(fin.code, 2);
}

/// A lobby end of a PvE match without any result (no script end) closes with the plain code: nothing would show the
/// result screen or its button.
#[test]
fn a_pve_end_without_the_scripts_end_closes_plainly() {
    let rule = PveEndRule { cancel_ms: 60_000, cancel2_ms: 30_000, start_all_ms: 500, start_two_of_three_ms: 800, match_ms: 60_000 };
    let (mut net, id) = pve_match_with(1, 1, rule, MatchConfig::default());
    net.run(1500);
    assert!(net.hub.end_match(id, net.now));
    net.run(200);
    let c = &net.clients[0].1;
    let fin = c.dfs.iter().find(|d| d.leave_mask & (1 << c.idx) != 0).expect("closed");
    assert_eq!(fin.code, 0xFF);
}

/// A leave before the start that leaves one human (`has_left_match.players_count + 1 >= human_players_count`):
/// `cancel_match_instantly` ends the mission in the leave's own step; the other human is closed with the next DF.
#[test]
fn a_leave_before_the_start_that_leaves_one_human_cancels_at_once() {
    let rule = PveEndRule { cancel_ms: 60_000, cancel2_ms: 30_000, start_all_ms: 3000, start_two_of_three_ms: 800, match_ms: 60_000 };
    let mut net = pve_match(2, 2, rule, MatchConfig::default());
    net.run(1500);
    assert!(net.clients.iter().all(|(_, c)| c.entered));
    net.clients[1].1.leave();
    net.run(1000);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::PveCancel, Some(1)));
    let c = &net.clients[0].1;
    let tl = leave_df(c, 1);
    assert!(tl <= enter_df(c) + 3000, "before the start");
    assert_eq!(fin_df(c), Some(tl + TICK));
}

/// 3-human mission, 2 entered, 1 of them left before the start: `on_player_left_before_match_started` (index 16)
/// enables `wait_before_cancel2` (index 14), which runs from the next step and cancels `cancel2` later; it disables
/// `2_from_3_players_ready`, so no start and no mission timeout.
#[test]
fn two_of_three_and_a_leave_before_the_start_cancel_after_the_second_wait() {
    let rule = PveEndRule { cancel_ms: 60_000, cancel2_ms: 1000, start_all_ms: 500, start_two_of_three_ms: 3000, match_ms: 1000 };
    let mc = MatchConfig { ready_timeout: Duration::from_millis(300), ..MatchConfig::default() };
    let mut net = pve_match(3, 2, rule, mc);
    net.run(1500);
    assert!(net.clients.iter().all(|(_, c)| c.entered));
    net.clients[1].1.leave();
    net.run(2500);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::PveCancel, Some(1)));
    let c = &net.clients[0].1;
    assert_eq!(fin_df(c), Some(leave_df(c, 1) + TICK + 1000 + TICK));
}

/// After the start a leave ends nothing (the others may still be alive: only the world knows); when every human left,
/// the script's own end is a failure too (`on_player_killed_no_resurrect`, nobody can resurrect): AllLeft, winner 1.
#[test]
fn after_the_start_only_every_human_leaving_ends_the_mission() {
    let rule = PveEndRule { cancel_ms: 60_000, cancel2_ms: 30_000, start_all_ms: 200, start_two_of_three_ms: 800, match_ms: 60_000 };
    let mut net = pve_match(2, 2, rule, MatchConfig::default());
    net.run(1500);
    net.clients[1].1.leave();
    net.run(1000);
    assert!(net.hub.take_ended().is_empty(), "one human still in the mission");
    assert!(!net.clients[0].1.closed);
    net.clients[0].1.leave();
    net.run(500);
    let ended = net.hub.take_ended();
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::AllLeft, Some(1)));
}

#[test]
fn two_clients_connect_get_de_and_see_each_others_inputs_in_order() {
    let (mut net, ticket) = two_player_match(MatchConfig::default());
    assert_eq!(ticket.match_id, SEED);
    net.run(3000);

    for (_, c) in &net.clients {
        // DC: min = max = server port
        assert_eq!(c.dc, Some(Ports { min: 1236, max: 1236 }));
        // DD: own index, the provided level checksum, the options blob
        let dd = c.dd.as_ref().expect("DD");
        assert_eq!(dd.local_player_index as u64, c.idx);
        assert_eq!(dd.level_checksum, CHECKSUM);
        assert_eq!(dd.options().unwrap(), test_options());
        // DE: the cached world, time = the world's time, exactly once
        let de = c.de.as_ref().expect("DE");
        assert_eq!(c.de_count, 1);
        assert_eq!(de.time, T0);
        assert_eq!(de.unpack().unwrap(), test_world().body);
        // DF: one per tick from T0 + tick without gaps; nobody enters until both clients answered a DF with a CD (both
        // synced), then one DF enters both with their start profiles and the enter bits repeat for `enter_repeats` DFs
        assert_eq!(c.dfs[0].time, T0 + TICK);
        assert_eq!(c.dfs[0].enter_mask, 0, "no enter before every client is synced");
        let at = c.dfs.iter().position(|d| d.enter_mask != 0).expect("an enter");
        let first = &c.dfs[at];
        assert_eq!(first.enter_mask, 0b11, "both players enter in the same DF");
        assert!(c.dfs[..at].iter().all(|d| d.profile.is_empty()), "no profile before the enter");
        let repeats = c.dfs[at..].iter().take_while(|d| d.enter_mask == 0b11).count();
        assert_eq!(repeats, MatchConfig::default().enter_repeats as usize);
        assert!(c.dfs.last().is_some_and(|d| d.enter_mask == 0), "the enter bits stop after the repeats");
        assert_eq!(first.code, 0xFF);
        let mut profiles: Vec<u8> = first.profile.iter().map(|(p, _)| *p).collect();
        profiles.sort();
        assert_eq!(profiles, vec![0, 1]);
        assert!(c.dfs.len() > 30);
        for w in c.dfs.windows(2) {
            assert_eq!(w[1].time, w[0].time + TICK, "DF stream has a gap");
        }
        assert!(c.entered && !c.closed);
        // normal operation: nothing late
        assert!(c.dfs.iter().all(|d| d.late.is_empty() && d.leave_mask == 0));
    }

    let (a, b) = (&net.clients[0].1, &net.clients[1].1);
    assert!(a.sent.len() > 30 && b.sent.len() > 30);
    // each client's inputs, relayed to the other in the order sent, never back to the sender (the client asserts on an
    // input of its own player arriving where its own unmoved input is: change_input 0x2001B4038)
    for (viewer, name, own, player, sender) in [(a, "A", 0u8, 1u8, b), (b, "B", 1u8, 0u8, a)] {
        assert!(viewer.seen(own).is_empty(), "{name} got its own inputs back");
        let seen = viewer.seen(player);
        assert!(seen.len() + 8 >= sender.sent.len(), "{name} lost inputs of {player}: {} of {}", seen.len(), sender.sent.len());
        assert_eq!(&seen[..], &sender.sent[..seen.len()], "{name} sees player {player} out of order");
    }
    // and both views agree
    let (va, vb) = (a.seen(1), b.seen(1));
    let n = va.len().min(vb.len());
    assert_eq!(va[..n], vb[..n]);

    // confirmations: only a client's own CD stamps; b = the server's match clock when the CD arrived (the client's servo
    // needs the real receive time): close to the stamp in this lockstep test, never before the world time
    for (c, other) in [(a, b), (b, a)] {
        let conf: Vec<(u32, u32)> = c.dfs.iter().flat_map(|d| d.conf.clone()).collect();
        let stamps: std::collections::BTreeSet<u32> = c.sent.iter().map(|(s, _)| *s).chain(c.pre_sent.iter().copied()).collect();
        assert!(conf.len() + 8 >= stamps.len());
        assert!(conf.iter().all(|(x, y)| x.abs_diff(*y) < 1000), "{conf:?}");
        assert!(conf.iter().all(|(x, _)| stamps.contains(x)));
        assert!(conf.len() <= stamps.len(), "confirmations of the other client's CDs leaked in");
        let _ = other;
    }

    // the match is running with both players entered
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.phase, st.entered, st.ready, st.de_sent), ("running", 2, 2, 2));
    assert_eq!(st.late_dropped, 0);

    // both leave (CE): the match ends AllLeft without a winner
    net.clients[0].1.leave();
    net.clients[1].1.leave();
    net.run(300);
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    let e = &ended[0];
    assert_eq!((e.match_id, e.reason, e.winner), (SEED, EndReason::AllLeft, None));
    assert!(e.players.iter().all(|p| p.entered && p.leave == LeaveReason::Menu), "CE = the menu leave");
    // after the drain time the match is gone
    net.run(2500);
    assert_eq!(net.hub.running_matches(), 0);
}

#[test]
fn leave_is_announced_to_the_others() {
    let (mut net, _) = two_player_match(MatchConfig::default());
    net.run(1000);
    net.clients[1].1.leave();
    net.run(500);
    let a = &net.clients[0].1;
    let leave_dfs: Vec<_> = a.dfs.iter().filter(|d| d.leave_mask != 0).collect();
    assert_eq!(leave_dfs.len(), 1);
    assert_eq!(leave_dfs[0].leave_mask, 0b10);
    // the leaver gets no more DFs after its CE, the stream to A continues
    assert!(net.hub.stats(SEED).unwrap().left == 1);
    let t_last = a.dfs.last().unwrap().time;
    net.run(500);
    assert!(net.clients[0].1.dfs.last().unwrap().time > t_last);
    // A leaves too: the match ends
    net.clients[0].1.leave();
    net.run(300);
    assert_eq!(net.hub.take_ended()[0].reason, EndReason::AllLeft);
}

#[test]
fn late_inputs_duplicates_and_cc() {
    let (mut net, _) = two_player_match(MatchConfig::default());
    net.run(1500);
    net.clients[0].1.send_inputs = false;
    net.clients[1].1.send_inputs = false;
    // a short pause: the correction below (150 ms before the stalled clock) must stay within 255 ms of the DF that carries
    // it (dt is a u8 of milliseconds); with relay_delay 50 a 200 ms pause would make it too old, which the relay drops
    net.run(100);

    let st0 = net.hub.stats(SEED).unwrap();
    // B changes an input after it was sent: CD{time X, late (dt 150 ms, input)} -> relayed as n_late with the right dt (milliseconds, 0x20032ED00)
    let x = net.clients[1].1.next_cd;
    let input = PlayerInput { rot: [1, 2], actions: 0xAB };
    net.clients[1].1.send_cd(&ClientUpdate { time: x, late: vec![(3 * TICK as u8, input)], ..Default::default() });
    // A sends the same CD time twice (the real client resends plain messages): relayed once
    let y = net.clients[0].1.next_cd;
    let cd = ClientUpdate { time: y, now: vec![PlayerInput { rot: [5, 6], actions: 0xCD }], ..Default::default() };
    net.clients[0].1.send_cd(&cd);
    net.clients[0].1.send_cd(&cd);
    net.run(600);
    let a = &net.clients[0].1;
    let seen_b = a.seen(1);
    // a CD late entry is a precise-time input: it goes in the late list of the DF the CD landed in, with the CD's own
    // dt (never floored to the tick grid or put in the input mask); the sender's confirmation moves it there too
    let df = a.dfs.iter().find(|d| d.late.iter().any(|(_, p)| *p == 1)).expect("a DF with a late entry of B");
    let k = df.late.iter().position(|(_, p)| *p == 1).unwrap();
    assert_eq!(df.late[k].0, 3 * TICK as u8, "the CD's own dt");
    assert!(df.time >= x);
    let hit: Vec<_> = seen_b.iter().filter(|(_, act)| *act == 0xAB).collect();
    assert_eq!(hit, vec![&(df.time - 3 * TICK, 0xAB)]);
    assert_eq!(df.inputs.len(), df.late.len() + df.input_mask.count_ones() as usize);
    assert_eq!(df.inputs[k].actions, 0xAB); // late inputs come first, in the order of the late list
    // A's CD: relayed once, to B only, at its own time when that DF had not gone out yet, else as a normal input of the
    // next DF (where its confirmation goes too: the client moves its input there, 0x2001B2650)
    assert!(a.seen(0).is_empty(), "A never gets its own input back");
    let seen_a: Vec<_> = net.clients[1].1.seen(0).into_iter().filter(|(_, act)| *act == 0xCD).collect();
    assert_eq!(seen_a.len(), 1, "{seen_a:?}");
    assert!(seen_a[0].0 >= y);
    let conf_df = a.dfs.iter().find(|d| d.conf.iter().any(|(s, _)| *s == y)).expect("A's confirmation");
    assert_eq!(conf_df.time, seen_a[0].0, "the confirmation rides in the DF the input got");
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.late_relayed - st0.late_relayed, st.cd_duplicates - st0.cd_duplicates), (1, 1)); // B's changed input; A's duplicate

    // CC: answered with a DE, at most once per cc_gap
    net.clients[0].1.send_cc();
    net.run(100);
    net.clients[0].1.send_cc();
    net.run(100);
    assert_eq!(net.clients[0].1.de_count, 2);
    let st1 = net.hub.stats(SEED).unwrap();
    assert_eq!((st1.cc_received - st.cc_received, st1.de_sent - st.de_sent), (2, 1));

    // two inputs of one player that land in one DF are merged as the client merges them (rotation words added, action
    // bits OR'ed), never replaced (replacing loses inputs)
    let z = net.clients[1].1.next_cd + 10 * TICK;
    let i1 = PlayerInput { rot: [1, 2], actions: 0x100 };
    let i2 = PlayerInput { rot: [10, 20], actions: 0x200 };
    net.clients[1].1.send_cd(&ClientUpdate { time: z, now: vec![i1], ..Default::default() });
    net.clients[1].1.send_cd(&ClientUpdate { time: z - TICK, now: vec![i2], ..Default::default() });
    net.run(800);
    let a = &net.clients[0].1;
    // B's two CDs land in one DF (the second, older one never in an earlier DF than the first: a player's CDs land in
    // non-decreasing DFs); A sees one merged input there and neither of the two alone
    let seen: Vec<u64> = a.seen(1).into_iter().map(|(_, act)| act).filter(|act| *act & 0x300 != 0).collect();
    assert_eq!(seen, vec![0x300], "both inputs of B, merged in one DF");
    let df = a.dfs.iter().find(|d| d.input_mask & 0b10 != 0 && d.inputs[d.late.len()].actions == 0x300).unwrap();
    assert_eq!(df.inputs[df.late.len()].rot, [11, 22]);
    assert!(df.time >= z);
    let _ = st1;
}

#[test]
fn a_df_stays_inside_the_clients_4096_byte_copy() {
    use protocol::Encode;
    let (mut net, _) = two_player_match(MatchConfig::default());
    net.run(1000);
    let before = net.clients[0].1.dfs.len();
    // 2 x 60 profile inputs of 12 entries (about 11 KB): more than one DF can carry
    let pi = ProfileInput { head: [1, 2, 3, 4], entries: (0..12).map(|k| (k, 1, 2, 3)).collect() };
    for c in 0..2 {
        let x = net.clients[c].1.next_cd;
        net.clients[c].1.send_cd(&ClientUpdate { time: x, profile: vec![pi.clone(); 60], ..Default::default() });
    }
    net.run(1000);
    let dfs = &net.clients[0].1.dfs[before..];
    assert!(dfs.iter().all(|d| d.to_bytes().unwrap().len() < 0x1000), "every DF payload fits the client's 0x1000-byte copy");
    // a player has at most one profile entry per time (the newer wins, as the client keeps one): A gets B's one, never
    // its own back
    assert_eq!(dfs.iter().map(|d| d.profile.len()).sum::<usize>(), 1);
}

#[test]
fn lobby_ends_the_match_and_the_winner_is_passed_on() {
    let (mut net, ticket) = two_player_match(MatchConfig::default());
    net.run(1000);
    assert!(net.hub.set_winner(ticket.match_id, 1));
    assert!(net.hub.end_match(ticket.match_id, net.now));
    net.run(300);
    for (_, c) in &net.clients {
        assert!(c.closed, "the final DF carries the local player's leave bit");
        assert_eq!(c.dfs.last().unwrap().leave_mask, 0b11);
    }
    let ended = net.hub.take_ended();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].reason, ended[0].winner), (EndReason::Requested, Some(1)));
    assert!(ended[0].players.iter().all(|p| p.leave == LeaveReason::MatchEnded && p.entered));
    assert!(ended[0].duration >= Duration::from_millis(800));
    assert!(!net.hub.end_match(ticket.match_id + 1, net.now));
}

#[test]
fn nobody_comes_ready_timeout() {
    let mut net = Net::new(MatchConfig::default());
    net.create(2, None);
    net.run(59_000);
    assert!(net.hub.take_ended().is_empty());
    net.run(2000);
    let e = net.hub.take_ended();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].reason, EndReason::ReadyTimeout);
    assert!(e[0].players.iter().all(|p| !p.entered && p.leave == LeaveReason::NeverEntered));
}

#[test]
fn unknown_order_and_wrong_session_are_refused() {
    let mut net = Net::new(MatchConfig::default());
    let t = net.create(1, Some(777));
    net.add_client(0, 0x1111_1111, 777); // unknown order id
    net.add_client(1, t.players[0].order_id, 778); // right id, wrong session
    net.run(800);
    assert!(net.clients[0].1.dc.is_none());
    assert!(net.clients[1].1.dc.is_none());
    assert_eq!(net.hub.stats(SEED).unwrap().ready, 0);
    // the right session gets in
    net.add_client(2, t.players[0].order_id, 777);
    net.run(800);
    assert!(net.clients[2].1.dc.is_some());
    assert!(net.clients[2].1.dd.is_some());
}

#[test]
fn start_with_the_ready_players_and_refuse_a_late_joiner() {
    let mc = MatchConfig { ready_timeout: Duration::from_millis(500), ..Default::default() };
    let mut net = Net::new(mc);
    let t = net.create(3, None);
    for p in &t.players[..2] {
        net.add_client(u64::from(p.slot), p.order_id, 1);
    }
    net.run(1500);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.phase, st.entered), ("running", 2));
    // the third player connects after the start
    net.add_client(2, t.players[2].order_id, 1);
    net.run(1500);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.entered, st.late_refused), (2, 1));
    let c = &net.clients[2].1;
    assert!(c.dd.is_some(), "it still gets DC and DD");
    assert!(c.de.is_none() && c.dfs.is_empty(), "but no world and no DF");
    // the DFs of the others never name player 2
    assert!(net.clients[0].1.dfs.iter().all(|d| d.enter_mask & 0b100 == 0 && d.input_mask & 0b100 == 0));
}

#[test]
fn a_silent_client_times_out_and_the_rest_continue() {
    let (mut net, _) = two_player_match(MatchConfig::default());
    net.run(1000);
    // B goes silent (no datagrams reach the hub)
    let b_addr = net.clients[1].0;
    net.clients[1].0 = addr(999);
    net.run(6000);
    let _ = b_addr;
    let a = &net.clients[0].1;
    assert!(a.dfs.iter().any(|d| d.leave_mask == 0b10));
    assert_eq!(net.hub.stats(SEED).unwrap().left, 1);
    net.clients[0].1.leave();
    net.run(300);
    let e = net.hub.take_ended();
    assert_eq!(e[0].players[1].leave, LeaveReason::TimedOut);
    assert_eq!(e[0].players[0].leave, LeaveReason::Menu);
}

/// The input recorder writes the header (match id, world time, records, options, world item), one DF record per tick
/// with every player's inputs (the full tick, before the no-echo filtering) and the end marker.
#[test]
fn the_recorder_writes_the_full_input_stream() {
    let dir = std::env::temp_dir().join(format!("srprec_test_{}", std::process::id()));
    let mc = MatchConfig { record_dir: Some(dir.clone()), ..MatchConfig::default() };
    let rule = EndRule { countdown_ms: 200, match_ms: 1000, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: true, draw_overtime_ms: 0 };
    let mut net = Net::new(mc);
    let mut s = spec(2, None);
    s.end_rule = Some(rule);
    let t = net.hub.create_match(&s, net.now).unwrap();
    for p in &t.players {
        net.add_client(u64::from(p.slot), p.order_id, 9000 + u32::from(p.slot));
    }
    net.run(6000);
    assert_eq!(net.hub.take_ended().len(), 1);
    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let b = std::fs::read(&files[0]).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(&b[..8], b"SRPREC01");
    let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    assert_eq!(u32_at(8), t.match_id);
    let n = usize::from(b[17]);
    assert_eq!(n, 2);
    let mut p = 18 + 2 * n;
    let opts = u32_at(p) as usize;
    assert_eq!(&b[p + 4..p + 4 + opts], &s.options[..]);
    p += 4 + opts;
    p += 4 + u32_at(p) as usize; // the world item
    let (mut dfs, mut times, mut enter_seen) = (0, Vec::new(), false);
    loop {
        match b[p] {
            1 => {
                let len = u32_at(p + 5) as usize;
                let df = GameUpdate::from_bytes(&b[p + 9..p + 9 + len]).unwrap();
                assert_eq!(df.time, u32_at(p + 1));
                assert!(df.conf.is_empty(), "confirmations are per recipient: not recorded");
                enter_seen |= df.enter_mask == 0b11;
                times.push(df.time);
                dfs += 1;
                p += 9 + len;
            }
            0xFF => {
                assert_eq!(p + 5, b.len(), "the end marker closes the file");
                break;
            }
            k => panic!("record kind {k}"),
        }
    }
    assert!(dfs > 50 && enter_seen, "{dfs} DFs, enter of both players");
    assert!(times.windows(2).all(|w| w[1] == w[0] + 50), "one record per tick, in order");
}
