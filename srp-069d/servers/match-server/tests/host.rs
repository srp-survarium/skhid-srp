//! The world host link of a match against a fake host: an in-process
//! thread on one end of a unix socket pair per connection, speaking `world-host-proto`. The match runs on the virtual
//! clock of the relay tests; the fake host runs in real time, so the tests step the clock in small steps with short real
//! pauses and wait for conditions.
#![cfg(unix)]

mod common;

use common::*;
use match_server::match_msgs::{GameUpdate, PlayerInput};
use match_server::{
    Connector, EndReason, EndRule, HostConfig, HostOutcome, HostStream, Hub, HubConfig, MatchConfig, MatchEnd, MatchSpec, PlayerSpec, PveEndRule,
    WorldStore,
};
use protocol::Decode;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use world_host_proto::{self as proto, FromHost, ToHost};

/// What the fake host does on one connection.
#[derive(Clone, Debug)]
struct Plan {
    /// answer Create with Ready
    ready: bool,
    /// answer Create with Fault (the zygote refuses the match)
    refuse: bool,
    /// Stats every n DFs (0 = never)
    stats_every: u32,
    /// drop the connection after n DFs
    crash_after: Option<u32>,
    fault_after: Option<u32>,
    diverge_after: Option<u32>,
    /// after n DFs: Outcome {state 5, winner}
    outcome_after: Option<(u32, u8)>,
    /// the world ends `ms` after the enter DF: Outcome {state 5, winner} once the DF 300 ms later was stepped
    outcome_after_enter: Option<(u32, u8)>,
    /// answer Close with Final (state, winner of the world at t_last when no Outcome was sent)
    final_: Option<(u8, u8)>,
    /// after Ready the host stops reading: the relay's writes block (a hung child)
    stall_after_ready: bool,
    /// phase 2: after stepping DF t, a Tick for item t - tick (the client's rule)
    ticks: bool,
    /// phase 2 resync, the answer to WantDe: 0 none, 1 a De of the newest final tick (last DF - 300) with the entered
    /// players' presence mask at +0x11, 2 a refusal (`Fault "WantDe <id>: ..."`), 3 a De with a wrong presence mask
    de: u8,
    /// the answers to the first WantDe frames in order (values as `de`); later ones use `de`
    de_script: Vec<u8>,
    /// version 3: before Final a `Kills` frame with the deaths [`details`] counts (true) or one too few (false)
    kills: Option<bool>,
    /// protocol 5: a `Bots` frame after every stepped DF ([`fake_bots`]), sent whether or not Create asked for it
    bots: bool,
    /// the first `Bots` with an ai entry goes out only after the next DF (before that DF's own `Bots`)
    bots_late_first_ai: bool,
}

impl Plan {
    fn normal() -> Self {
        Plan {
            ready: true,
            refuse: false,
            stats_every: 20,
            crash_after: None,
            fault_after: None,
            diverge_after: None,
            outcome_after: None,
            outcome_after_enter: None,
            final_: Some((3, 3)),
            stall_after_ready: false,
            ticks: false,
            de: 1,
            de_script: vec![],
            kills: None,
            bots: false,
            bots_late_first_ai: false,
        }
    }

    fn silent() -> Self {
        Plan { ready: false, stats_every: 0, final_: None, ..Plan::normal() }
    }
}

/// The fake host's bot entries after stepping DF `t` (protocol 5), for DF `t + tick`: once slot 1 entered, slot 1's
/// input (rot[0] = that DF's tick number, rot[1] 0x1234, actions 0x40) and on every 4th tick an ai entry of slot 2
/// (kind 1, param = the tick number). Before the enter the frame is empty (the AI has no players yet).
fn fake_bots(t: u32, present: u32) -> proto::bots::Bots {
    let t_for = t + TICK;
    let n = (t_for / TICK) as u16;
    let (mut inputs, mut ai) = (vec![], vec![]);
    if present & 0b10 != 0 {
        let mut b = [0u8; 11];
        b[..2].copy_from_slice(&n.to_le_bytes());
        b[2..4].copy_from_slice(&0x1234u16.to_le_bytes());
        b[10] = 0x40;
        inputs.push((1u8, b));
        if n % 4 == 0 {
            ai.push((2u8, 1u16, n));
        }
    }
    proto::bots::Bots { t_for, inputs, ai, offgrid: 0 }
}

/// The fake host's E0 globals of item `t`: g0 = the tick number's low byte, g1 = t / 100, g2 = 0x77.
fn g_of(t: u32) -> [u8; 3] {
    [(t / TICK) as u8, (t / 100) as u8, 0x77]
}

/// The human (slot 0) and the bot rows of a host frame; `k` tells frames apart.
fn rows(k: u16) -> Vec<proto::PlayerStats> {
    vec![
        proto::PlayerStats { slot: 0, kills: 40 + k, deaths: 1, score: 590 + k, chests: 2, keys_spent: 1, gold_spent: 50 },
        proto::PlayerStats { slot: 1, kills: 0, deaths: 3, score: 0, chests: 0, ..Default::default() },
        // a row for a slot the match does not have: dropped by the relay
        proto::PlayerStats { slot: 19, kills: 9, deaths: 9, score: 9, chests: 9, ..Default::default() },
    ]
}

/// Version-3 detail rows: slot 0 killed slots 1 and 2 (event row 0 = 2), slot 1 killed himself (rows 2 and 3), plus a
/// row for a slot the match lacks; `badges` adds slot 0's best-player medal (badge 23) and a kill badge.
fn details(badges: bool) -> Vec<proto::PlayerDetail> {
    let mut d0 = proto::PlayerDetail { slot: 0, best_streak: 2, ..Default::default() };
    d0.events[0] = 2;
    let mut d1 = proto::PlayerDetail { slot: 1, ..Default::default() };
    d1.events[2] = 2;
    d1.events[3] = 1;
    let mut d2 = proto::PlayerDetail { slot: 2, ..Default::default() };
    d2.events[2] = 1;
    if badges {
        d0.badges[23] = [1, 0, 0];
        d0.badges[5] = [0, 1, 0];
    }
    vec![d0, d1, d2, proto::PlayerDetail { slot: 3, ..Default::default() }, proto::PlayerDetail { slot: 19, ..Default::default() }]
}

/// The deaths of [`details`]: 0 kills 1 and 2, 1 kills himself; `all = false` leaves one out.
fn kill_log(t: u32, all: bool) -> proto::Kills {
    let k = |t, killer, victim| proto::KillRecord { t, killer, victim, weapon_dict: 66, weapon_category: 13, ..Default::default() };
    let mut records = vec![k(t - 200, 0, 1), k(t - 100, 1, 1), k(t - 50, 0, 2)];
    if !all {
        records.pop();
    }
    proto::Kills { t_upto: t, records }
}

#[derive(Default)]
struct Fake {
    plans: Vec<Plan>,
    no_socket: bool,
    conns: AtomicU32,
    /// every frame the host got: (connection, message)
    got: Mutex<Vec<(u32, ToHost)>>,
    /// `t_for` of the `Bots` frames held back by `bots_late_first_ai`
    held: Mutex<Vec<u32>>,
}

impl Fake {
    fn new(plans: Vec<Plan>) -> Arc<Fake> {
        Arc::new(Fake { plans, ..Default::default() })
    }

    fn got(&self, conn: u32) -> Vec<ToHost> {
        self.got.lock().unwrap().iter().filter(|(k, _)| *k == conn).map(|(_, m)| m.clone()).collect()
    }

    fn dfs(&self, conn: u32) -> Vec<proto::Df> {
        self.got(conn).into_iter().filter_map(|m| if let ToHost::Df(d) = m { Some(d) } else { None }).collect()
    }

    fn connector(self: &Arc<Self>) -> Connector {
        let fake = self.clone();
        Arc::new(move || {
            if fake.no_socket {
                return Err(io::Error::new(io::ErrorKind::NotFound, "no socket"));
            }
            let k = fake.conns.fetch_add(1, Ordering::SeqCst);
            let plan = fake.plans[(k as usize).min(fake.plans.len() - 1)].clone();
            let (relay, host) = UnixStream::pair()?;
            let gate = Arc::new(AtomicBool::new(false));
            let shut = Arc::new(AtomicBool::new(false));
            let f = fake.clone();
            let g = gate.clone();
            std::thread::spawn(move || run_fake(&f, k, host, plan, &g));
            Ok(Box::new(Gated { inner: relay, gate, shut }) as Box<dyn HostStream>)
        })
    }
}

/// The relay's end of the pair; its writes block while `gate` is set (a host that stopped reading).
struct Gated {
    inner: UnixStream,
    gate: Arc<AtomicBool>,
    shut: Arc<AtomicBool>,
}

impl Read for Gated {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.inner.read(b)
    }
}

impl Write for Gated {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        while self.gate.load(Ordering::SeqCst) {
            if self.shut.load(Ordering::SeqCst) {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        self.inner.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl HostStream for Gated {
    fn try_clone_stream(&self) -> io::Result<Box<dyn HostStream>> {
        Ok(Box::new(Gated { inner: self.inner.try_clone()?, gate: self.gate.clone(), shut: self.shut.clone() }))
    }
    fn shutdown_both(&self) {
        self.shut.store(true, Ordering::SeqCst);
        let _ = self.inner.shutdown(std::net::Shutdown::Both);
    }
}

fn run_fake(fake: &Fake, k: u32, s: UnixStream, plan: Plan, gate: &AtomicBool) {
    let mut r = s.try_clone().unwrap();
    let mut w = s;
    fn put(w: &mut UnixStream, m: FromHost) {
        let _ = w.write_all(&m.frame().unwrap());
    }
    let (mut dfs, mut outcome, mut enter): (u32, Option<proto::Outcome>, Option<u32>) = (0, None, None);
    let (mut last_t, mut present) = (0u32, 0u32);
    let mut wants = 0usize;
    let mut held: Option<proto::bots::Bots> = None;
    let mut held_once = false;
    loop {
        let f = match proto::read_frame(&mut r) {
            Ok(Some(f)) => f,
            _ => return,
        };
        let m = ToHost::decode(&f).expect("a frame the relay sends decodes");
        fake.got.lock().unwrap().push((k, m.clone()));
        match m {
            ToHost::Create(c) => {
                let match_id = c.match_id;
                if plan.refuse {
                    put(&mut w, FromHost::Fault { reason: "max_matches".into() });
                    return;
                }
                if plan.ready {
                    // protocol 4: an empty world asks the host to build it; the fake's start world is `host_world()`
                    let start = c.world.is_empty().then(|| host_world().item().unwrap());
                    let world_time = start.as_ref().map_or(T0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                    put(&mut w, FromHost::Ready(proto::Ready { match_id, world_time, collision_checksum: HOST_CHECKSUM, create_ms: 800, start }));
                }
                if plan.stall_after_ready {
                    // the relay's writes block from now on, while a heartbeat keeps coming (only the queue shows it)
                    gate.store(true, Ordering::SeqCst);
                    let mut hb = w.try_clone().unwrap();
                    std::thread::spawn(move || {
                        let s = FromHost::Stats(proto::Stats { t: 0, players: vec![], team_score: [0, 0] }).frame().unwrap();
                        while hb.write_all(&s).is_ok() {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                    });
                }
            }
            ToHost::Df(d) => {
                dfs += 1;
                last_t = d.t;
                if let Ok(g) = GameUpdate::from_bytes(&d.bytes) {
                    present = (present | g.enter_mask) & !g.leave_mask;
                }
                if plan.crash_after == Some(dfs) {
                    return;
                }
                if plan.fault_after == Some(dfs) {
                    put(&mut w, FromHost::Fault { reason: "world step failed".into() });
                    return;
                }
                if plan.diverge_after == Some(dfs) {
                    put(&mut w, FromHost::Diverged(proto::Diverged { t: d.t, retimed: 1, merged: 0 }));
                }
                if plan.bots {
                    if let Some(b) = held.take() {
                        put(&mut w, FromHost::Bots(b));
                    }
                    let b = fake_bots(d.t, present);
                    if plan.bots_late_first_ai && !held_once && !b.ai.is_empty() {
                        held_once = true;
                        fake.held.lock().unwrap().push(b.t_for);
                        held = Some(b);
                    } else {
                        put(&mut w, FromHost::Bots(b));
                    }
                }
                if plan.ticks && d.t >= T0 + 2 * TICK {
                    let it = d.t - TICK;
                    // slots 0, 1 and 19 (the last mask byte's bit 3): one byte each, slot 1's byte moves with the tick
                    put(&mut w, FromHost::Tick(proto::Tick { t: it, mask: [0b11, 0, 0b1000], hashes: vec![0x5A, (it / TICK) as u8, 0x13], g: g_of(it) }));
                }
                if plan.stats_every > 0 && dfs % plan.stats_every == 0 {
                    put(&mut w, FromHost::Stats(proto::Stats { t: d.t, players: rows(1), team_score: [0, 0] }));
                }
                // the world's own end `ms` after the enter DF, read as final once DF t_end + 300 was stepped
                if enter.is_none() && GameUpdate::from_bytes(&d.bytes).is_ok_and(|g| g.enter_mask != 0) {
                    enter = Some(d.t);
                }
                if let (Some((ms, winner)), Some(e), None) = (plan.outcome_after_enter, enter, outcome.as_ref()) {
                    if d.t >= e + ms + 300 {
                        let o = proto::Outcome { t_end: e + ms, state: 5, winner_team: winner, pve_success: winner == 0, mission_ms: ms, players: rows(0), team_score: [7, 9], details: details(false) };
                        put(&mut w, FromHost::Outcome(o.clone()));
                        outcome = Some(o);
                    }
                }
                if let Some((n, winner)) = plan.outcome_after {
                    if dfs == n {
                        let o = proto::Outcome { t_end: d.t, state: 5, winner_team: winner, pve_success: winner == 0, mission_ms: 61_500, players: rows(0), team_score: [1, 0], details: details(false) };
                        put(&mut w, FromHost::Outcome(o.clone()));
                        outcome = Some(o);
                    }
                }
            }
            ToHost::Close { t_last } => {
                if let Some((state, winner)) = plan.final_ {
                    let base = outcome.clone().unwrap_or(proto::Outcome { t_end: t_last, state, winner_team: winner, pve_success: false, mission_ms: 0, players: vec![], team_score: [0, 0], details: vec![] });
                    if let Some(all) = plan.kills {
                        put(&mut w, FromHost::Kills(kill_log(t_last, all)));
                    }
                    put(&mut w, FromHost::Final(proto::Outcome { t_end: t_last, players: rows(5), details: details(true), ..base }));
                    return;
                }
                // a host that ignores Close: it stays connected until the relay drops it
            }
            ToHost::WantDe { request_id } => match {
                wants += 1;
                plan.de_script.get(wants - 1).copied().unwrap_or(plan.de)
            } {
                de @ (1 | 3) => {
                    // the newest final tick's item + tail: time at +0, the presence mask at +0x11, 600 bytes in all
                    let time = last_t - 300;
                    let mut item = vec![0xA5u8; 600];
                    item[..4].copy_from_slice(&time.to_le_bytes());
                    let m = if de == 3 { present | 0x100 } else { present };
                    item[0x11..0x15].copy_from_slice(&m.to_le_bytes());
                    put(&mut w, FromHost::De(proto::De { request_id, time, item }));
                }
                2 => put(&mut w, FromHost::Fault { reason: format!("WantDe {request_id}: no history item of time {}", last_t - 300) }),
                _ => {}
            },
        }
    }
}

struct Net {
    hub: Hub,
    client: (SocketAddr, Client),
    /// more clients (slots 1..) for PvP tests
    others: Vec<(SocketAddr, Client)>,
    now: Instant,
}

/// One human (slot 0, team 0) and 3 server-owned bots, PvE, with the given mirror rule and host. No hand-over of the
/// relay's own failure ends (`pve_handover` None: they close after `mirror_wait`); [`pve_handover`] has it.
fn pve(rule: PveEndRule, fake: &Arc<Fake>, tune: impl FnOnce(&mut HostConfig)) -> Net {
    pve_handover(rule, fake, tune, None)
}

fn pve_handover(rule: PveEndRule, fake: &Arc<Fake>, tune: impl FnOnce(&mut HostConfig), handover: Option<Duration>) -> Net {
    let mut h = HostConfig::with_connector("fake-host", fake.connector());
    tune(&mut h);
    let mc = MatchConfig { world_host: Some(h), pve_handover: handover, ..MatchConfig::default() };
    let mut worlds = WorldStore::new();
    worlds.insert(test_world().with_project("pve_01_v1_maze2").unwrap());
    let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds);
    let mut players = vec![PlayerSpec { account_id: 100, name: "p0".into(), team: 0, session: None, bot: false }];
    players.extend((1..=3).map(|k| PlayerSpec::bot(&format!("pve_bot_{k}"), 1)));
    let spec = MatchSpec {
        match_id: Some(SEED),
        map: MAP,
        mode: MODE,
        options: test_options(),
        players,
        level_checksum: None,
        roster: None,
        project: Some("pve_01_v1_maze2".into()),
        end_rule: None,
        pve_end: Some(rule),
        ai_ignore_players: false,
    };
    let now = Instant::now();
    let t = hub.create_match(&spec, now).unwrap();
    let mut c = Client::new(0, t.players[0].order_id, 9000, now);
    c.connect();
    Net { hub, client: ("127.0.0.1:5000".parse().unwrap(), c), others: vec![], now }
}

/// A mission whose script ends lie far away: only the host or the lobby ends it.
fn long() -> PveEndRule {
    PveEndRule { cancel_ms: 600_000, cancel2_ms: 300_000, start_all_ms: 500, start_two_of_three_ms: 800, match_ms: 3_600_000 }
}

impl Net {
    fn step(&mut self) {
        self.step_filtered(&mut |_| false);
    }

    /// [`Net::step`] that loses every datagram of the relay to the first client for which `drop` says so.
    fn step_filtered(&mut self, drop: &mut dyn FnMut(&[u8]) -> bool) {
        self.now += Duration::from_millis(5);
        let now = self.now;
        for (a, c) in std::iter::once(&mut self.client).chain(self.others.iter_mut()) {
            for d in c.poll(now) {
                self.hub.on_datagram(*a, &d, now);
            }
        }
        let first = self.client.0;
        for (to, d) in self.hub.poll(now) {
            if to == first && drop(&d) {
                continue;
            }
            if let Some((_, c)) = std::iter::once(&mut self.client).chain(self.others.iter_mut()).find(|(a, _)| *a == to) {
                c.receive(&d, now);
            }
        }
    }

    /// `ms` of virtual time, with a short real pause per DF so the host thread keeps up.
    fn run(&mut self, ms: u64) {
        for i in 0..ms / 5 {
            self.step();
            if i % 10 == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    /// Step until `f` holds (at most `ms` of virtual time).
    fn until(&mut self, ms: u64, mut f: impl FnMut(&mut Net) -> bool) -> bool {
        for _ in 0..ms / 5 {
            if f(self) {
                return true;
            }
            self.step();
            std::thread::sleep(Duration::from_micros(300));
        }
        f(self)
    }

    fn host(&self) -> (&'static str, u32) {
        let s = self.hub.stats(SEED).unwrap();
        (s.host, s.host_restarts)
    }

    fn ended(&mut self, ms: u64) -> MatchEnd {
        let mut got = vec![];
        assert!(self.until(ms, |n| {
            got.extend(n.hub.take_ended());
            !got.is_empty()
        }));
        got.remove(0)
    }

    fn outcome(&mut self, ms: u64) -> HostOutcome {
        let mut got = vec![];
        assert!(self.until(ms, |n| {
            got.extend(n.hub.take_outcomes());
            !got.is_empty()
        }), "no host outcome");
        assert_eq!(got.len(), 1);
        got.remove(0)
    }
}

fn fin(c: &Client) -> Option<u32> {
    c.dfs.iter().find(|d| d.leave_mask & 1 != 0).map(|d| d.time)
}

/// The whole phase-1 path: Create (the .srprec head), one Df per normal DF with the recorder's bytes, the host's
/// Outcome reported to the lobby while the clients stay in the match (winner and counters from the host), the lobby's
/// end, Close at the last normal DF (no fin DF reaches the host), Final, the MatchEnd with the Final's counters.
#[test]
fn create_dfs_outcome_close_final() {
    let fake = Fake::new(vec![Plan { outcome_after: Some((80, 0)), ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |_| {});
    let o = net.outcome(8000);
    assert_eq!((o.match_id, o.state, o.winner_team, o.pve_success, o.mission_ms), (SEED, 5, 0, true, 61_500));
    let human = o.players.iter().find(|p| p.slot == 0).unwrap();
    assert_eq!((human.account_id, human.bot, human.kills, human.deaths, human.score, human.chests), (100, false, 40, 1, 590, 2));
    assert_eq!((human.keys_spent, human.gold_spent), (1, 50), "the paid respawns the world charged (protocol 2)");
    assert!(o.players.iter().any(|p| p.slot == 1 && p.bot && p.account_id == 0));
    assert!(o.players.iter().all(|p| p.slot < 4), "a row of a slot the match lacks is dropped");
    assert_eq!(net.host().0, "live");
    // the relay does not close the client: the lobby owns the end sequence
    net.run(1000);
    assert!(net.hub.take_ended().is_empty());
    assert!(!net.client.1.closed && fin(&net.client.1).is_none());

    assert!(net.hub.end_match(SEED, net.now));
    let e = net.ended(3000);
    assert_eq!((e.reason, e.winner), (EndReason::Requested, Some(0)), "the host's winner");
    assert!(e.bots.is_empty(), "a PvE mission's script bots stay out of the lobby's roster (PvP only)");
    let st = e.stats.expect("the host's counters");
    let p0 = st.players.iter().find(|p| p.slot == 0).unwrap();
    assert_eq!((p0.kills, p0.score, p0.chests), (45, 595, 2), "the Final's counters");
    assert_eq!(st.team_scores, [1, 0]);
    // version 3: the Outcome's detail rows (no badges) went to the lobby; the result has the Final's rows with badges
    assert_eq!(o.details.len(), 4, "the row of a slot the match lacks is dropped");
    assert_eq!(o.details[0].badge_mask(), 0);
    assert_eq!(st.details.len(), 4);
    assert_eq!(st.details[0].badges[23], [1, 0, 0], "the Final's badges");
    assert_eq!((st.details[0].events[0], st.details[0].best_streak), (2, 2));
    assert_eq!(st.kills, None, "no Kills frame: no kill log");

    // what the host got
    let got = fake.got(0);
    let ToHost::Create(c) = &got[0] else { panic!("Create first: {:?}", got[0]) };
    assert_eq!((c.version, c.match_id, c.kind, c.project.as_str(), c.map, c.mode, c.tick_ms), (proto::PROTO_VERSION, SEED, proto::KIND_PVE, "pve_01_v1_maze2", MAP, MODE, TICK as u8));
    assert_eq!(c.records.iter().map(|r| (r.bot, r.team)).collect::<Vec<_>>(), vec![(false, 0), (true, 1), (true, 1), (true, 1)]);
    assert_eq!(c.options, test_options());
    assert_eq!(c.world, test_world().item().unwrap(), "the DE item");
    let dfs = fake.dfs(0);
    assert!(dfs.len() > 80);
    assert!(dfs.windows(2).all(|w| w[1].t == w[0].t + TICK), "one Df per tick, in order");
    assert_eq!(dfs[0].t, T0 + TICK);
    let ToHost::Close { t_last } = got.last().unwrap() else { panic!("Close last") };
    assert_eq!(*t_last, dfs.last().unwrap().t, "Close names the last normal DF");
    // no fin DF reached the host: every Df decodes, none has the human's leave bit; the client's close came after
    for d in &dfs {
        let df = GameUpdate::from_bytes(&d.bytes).unwrap();
        assert_eq!(df.time, d.t);
        assert_eq!(df.leave_mask, 0);
        assert!(df.conf.is_empty(), "the unfiltered bytes: no confirmations");
    }
    assert_eq!(fin(&net.client.1), Some(t_last + TICK));
    assert!(dfs.iter().any(|d| GameUpdate::from_bytes(&d.bytes).unwrap().enter_mask == 0b1111), "the enter DF");
    assert_eq!(net.host().0, "closed");
}

/// Version 3: the `Kills` frame before `Final` becomes the result's kill log when it agrees with the Final's event rows
/// (kills + team kills per killer, suicides); one death short and there is no kill log at all.
#[test]
fn the_kill_log_is_kept_only_when_it_matches_the_event_rows() {
    let rule = PveEndRule { match_ms: 3000, ..long() };
    for (all, kept) in [(true, true), (false, false)] {
        let fake = Fake::new(vec![Plan { final_: Some((5, 1)), kills: Some(all), ..Plan::normal() }]);
        let mut net = pve(rule, &fake, |_| {});
        let e = net.ended(10_000);
        let st = e.stats.expect("the Final's counters");
        match st.kills {
            Some(k) => {
                assert!(kept);
                assert_eq!(k.iter().map(|x| (x.killer, x.victim)).collect::<Vec<_>>(), vec![(0, 1), (1, 1), (0, 2)], "time order");
                assert!(k.iter().all(|x| x.weapon_category == 13));
            }
            None => assert!(!kept, "a complete log is kept"),
        }
    }
}

/// The relay's own end (its PvE mirror: the script's 45-minute end) with a live host: Close, the MatchEnd kept back
/// until Final, with the Final's counters and the relay's winner 1. Then a host that ignores Close: the MatchEnd
/// comes after `final_wait` with the last Stats.
#[test]
fn the_relays_own_end_waits_for_final_then_for_final_wait() {
    let rule = PveEndRule { match_ms: 3000, ..long() };
    let fake = Fake::new(vec![Plan { final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pve(rule, &fake, |_| {});
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::PveTimeout, Some(1)));
    assert_eq!(e.stats.expect("the Final's counters").players[0].kills, 45);
    assert!(net.client.1.closed);

    let fake = Fake::new(vec![Plan { final_: None, ..Plan::normal() }]);
    let mut net = pve(rule, &fake, |h| h.final_wait = Duration::from_millis(400));
    assert!(net.until(10_000, |n| n.hub.stats(SEED).unwrap().phase == "ended"));
    let at = net.now;
    let e = net.ended(2000);
    let waited = net.now - at;
    assert!(waited >= Duration::from_millis(390), "held for final_wait: {waited:?}");
    assert_eq!(e.winner, Some(1));
    assert_eq!(e.stats.expect("the last Stats").players[0].kills, 41);
}

/// With a live host, the relay's mirrored PvE failure end (here the script's
/// mission timeout) does not close the client at once. The DFs go on until the host's Outcome (read as final 300 ms after
/// the end step) arrives; then the lobby owns the end, so the `99` of the failure reaches a client still in the match.
/// Without an Outcome the relay closes after `mirror_wait` (1 s), or, with the hand-over, gives its own end to the lobby.
/// The lobby's end after the result closes with DF code 2. "Every human left" closes at once (nobody sees a screen).
#[test]
fn a_mirrored_failure_end_waits_for_the_hosts_outcome() {
    let rule = PveEndRule { match_ms: 3000, ..long() };
    let enter = |c: &Client| c.dfs.iter().find(|d| d.enter_mask != 0).expect("the enter").time;

    // the host's Outcome comes inside the wait: the relay does not close, the lobby ends the match later
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((3500, 1)), final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pve(rule, &fake, |_| {});
    let o = net.outcome(10_000);
    let c_end = enter(&net.client.1) + 3500;
    assert_eq!((o.t_end, o.winner_team, o.pve_success), (c_end, 1, false));
    assert!(fin(&net.client.1).is_none() && !net.client.1.closed, "the client is still in the match when the Outcome comes");
    net.run(3000);
    assert!(net.hub.take_ended().is_empty(), "the lobby owns the end once the host's Outcome came");
    assert!(!net.client.1.closed);
    assert!(net.hub.end_match_after_result(SEED, net.now));
    let e = net.ended(3000);
    assert_eq!((e.reason, e.winner), (EndReason::Requested, Some(1)));
    assert!(fin(&net.client.1).unwrap() > c_end + 3000);
    let codes: Vec<u8> = net.client.1.dfs.iter().filter(|d| d.leave_mask & 1 != 0).map(|d| d.code).collect();
    assert!(codes.iter().all(|&k| k == 2), "the closing DFs keep the world (code 2): {codes:?}");

    // no Outcome, no hand-over: the relay closes mirror_wait after the end step instead of the very next DF
    let fake = Fake::new(vec![Plan { final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pve(rule, &fake, |_| {});
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::PveTimeout, Some(1)));
    let late = fin(&net.client.1).unwrap() - (enter(&net.client.1) + 3500);
    assert!((950..=1300).contains(&late), "closed {late} ms after the end step");

    // no Outcome, with the hand-over (the default): after mirror_wait the relay's own end goes to the lobby (state 5,
    // winner 1, no counters); the lobby's end after its result closes with code 2 and winner 1, the host's Final kept
    // only for the counters
    let fake = Fake::new(vec![Plan { final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pve_handover(rule, &fake, |_| {}, Some(Duration::from_secs(10)));
    let o = net.outcome(10_000);
    let c_end = enter(&net.client.1) + 3500;
    assert_eq!((o.t_end, o.state, o.winner_team, o.pve_success, o.players.len()), (c_end, 5, 1, false, 0));
    assert!(!net.client.1.closed);
    net.run(2000);
    assert!(net.hub.take_ended().is_empty() && net.hub.take_outcomes().is_empty());
    assert!(net.hub.end_match_after_result(SEED, net.now));
    let e = net.ended(3000);
    assert_eq!((e.reason, e.winner), (EndReason::Requested, Some(1)));
    assert_eq!(e.stats.expect("the Final's counters").players[0].kills, 45);
    let codes: Vec<u8> = net.client.1.dfs.iter().filter(|d| d.leave_mask & 1 != 0).map(|d| d.code).collect();
    assert!(!codes.is_empty() && codes.iter().all(|&k| k == 2), "{codes:?}");

    // every human left: closed at once (no wait)
    let fake = Fake::new(vec![Plan::normal()]);
    let mut net = pve(long(), &fake, |_| {});
    net.run(2000);
    assert_eq!(net.host().0, "live");
    net.client.1.leave();
    let e = net.ended(400);
    assert_eq!(e.reason, EndReason::AllLeft);
}

/// A child that crashes mid-match: a new connection gets Create and every DF from the start, catches up and goes on;
/// the outcome still comes. The client never notices.
#[test]
fn a_crashed_host_is_restarted_and_catches_up() {
    let fake = Fake::new(vec![Plan { crash_after: Some(40), ..Plan::normal() }, Plan { outcome_after: Some((90, 1)), ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |_| {});
    let o = net.outcome(10_000);
    assert_eq!((o.winner_team, o.pve_success), (1, false));
    assert_eq!(net.host(), ("live", 1));
    let (first, second) = (fake.dfs(0), fake.dfs(1));
    assert_eq!(first.len(), 40);
    assert!(matches!(fake.got(1)[0], ToHost::Create(_)));
    assert_eq!(second[0].t, first[0].t, "the restart replays from the first DF");
    assert_eq!(&second[..40], &first[..], "the same bytes again");
    assert!(!net.client.1.closed);
}

/// A host that stops reading (its queue passes max_queue) and a host that never answers (no Ready, no frame for
/// heartbeat_ms of DFs): each is restarted max_restarts times, then the link is off and the match is a plain relay:
/// its end is not held.
#[test]
fn lagging_or_silent_hosts_are_restarted_then_dropped() {
    let fake = Fake::new(vec![Plan { stall_after_ready: true, ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |h| {
        // live: the queue rule; catching up after a restart: no write progress for heartbeat_ms of DFs
        h.max_queue = 20;
        h.heartbeat_ms = 3000;
    });
    assert!(net.until(20_000, |n| n.host().0 == "off"), "{:?}", net.host());
    assert_eq!(net.host().1, 2);
    assert_eq!(fake.conns.load(Ordering::SeqCst), 3);

    let fake = Fake::new(vec![Plan::silent()]);
    let mut net = pve(long(), &fake, |h| h.heartbeat_ms = 1000);
    assert!(net.until(20_000, |n| n.host().0 == "off"), "{:?}", net.host());
    assert_eq!((net.host().1, fake.conns.load(Ordering::SeqCst)), (2, 3));
    let dfs_before = net.client.1.dfs.len();
    net.run(500);
    assert!(net.client.1.dfs.len() > dfs_before, "the DFs never stopped");
    assert!(net.hub.end_match(SEED, net.now));
    net.step();
    assert_eq!(net.hub.take_ended().len(), 1, "an Off link holds nothing back");
}

/// Fault: before Ready it is a refused Create (off at once, no retry); after Ready a restart.
#[test]
fn a_fault_before_ready_is_off_after_ready_a_restart() {
    let fake = Fake::new(vec![Plan { refuse: true, ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |_| {});
    assert!(net.until(5000, |n| n.host().0 == "off"));
    net.run(500);
    assert_eq!((net.host().1, fake.conns.load(Ordering::SeqCst)), (0, 1));

    let fake = Fake::new(vec![Plan { fault_after: Some(30), ..Plan::normal() }, Plan { outcome_after: Some((60, 0)), ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |_| {});
    let o = net.outcome(10_000);
    assert!(o.pve_success);
    assert_eq!(net.host(), ("live", 1));

    // no socket at all: off, the match is a plain relay
    let fake = Arc::new(Fake { plans: vec![Plan::normal()], no_socket: true, ..Default::default() });
    let mut net = pve(long(), &fake, |_| {});
    assert!(net.until(5000, |n| n.host().0 == "off"));
    net.run(3000);
    assert!(net.client.1.entered);
}

/// Diverged: the host is not used any more (no retry); an Outcome it would send later never reaches the lobby, and the
/// match end is not held. A match without bot slots (a host that drives bots stays attached for their inputs:
/// `a_diverged_host_keeps_driving_the_bots_without_its_outcome`).
#[test]
fn a_diverged_host_is_not_used() {
    let fake = Fake::new(vec![Plan { diverge_after: Some(30), outcome_after: Some((60, 0)), ..Plan::normal() }]);
    let mut net = pvp(None, &fake);
    assert!(net.until(8000, |n| n.host().0 == "diverged"));
    net.run(3000);
    assert!(net.hub.take_outcomes().is_empty());
    assert_eq!((net.host().1, fake.conns.load(Ordering::SeqCst)), (0, 1));
    assert!(net.hub.end_match(SEED, net.now));
    net.step();
    let e = net.hub.take_ended();
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].winner, e[0].stats.clone()), (None, None), "nothing of the diverged host");
}

/// A PvP match (one human of team 0, the harness has one client) with the host and an optional time-limit mirror.
fn pvp(end_rule: Option<EndRule>, fake: &Arc<Fake>) -> Net {
    pvp_screen(end_rule, fake, MatchConfig::default().pvp_end_screen)
}

/// [`pvp`] with the PvP end screen's lobby wait (`MatchConfig::pvp_end_screen`).
fn pvp_screen(end_rule: Option<EndRule>, fake: &Arc<Fake>, screen: Duration) -> Net {
    let mc = MatchConfig { world_host: Some(HostConfig::with_connector("fake-host", fake.connector())), pvp_end_screen: screen, ..MatchConfig::default() };
    let mut worlds = WorldStore::new();
    worlds.insert(test_world());
    let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds);
    let players = vec![PlayerSpec { account_id: 100, name: "p0".into(), team: 0, session: None, bot: false }];
    let spec = MatchSpec { match_id: Some(SEED), map: MAP, mode: MODE, options: test_options(), players, level_checksum: None, roster: None, project: None, end_rule, pve_end: None, ai_ignore_players: false };
    let now = Instant::now();
    let t = hub.create_match(&spec, now).unwrap();
    let mut c = Client::new(0, t.players[0].order_id, 9000, now);
    c.connect();
    Net { hub, client: ("127.0.0.1:5000".parse().unwrap(), c), others: vec![], now }
}

/// Phase 3: a PvP match gets the host too (kind PvP). An end the relay does not mirror (tickets, deliveries, points,
/// rounds) comes from the host's Outcome. The PvP end screen: the host gets its Close first
/// (no DF after it), the MatchEnd (the world's winner, the Final's counters, team scores and badge rows) goes to the lobby
/// while the client is still in the match, and the lobby's end after its 97 closes the client with code 2.
#[test]
fn a_pvp_world_end_gives_the_result_first_and_the_lobby_closes_with_code_2() {
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((2000, 1)), final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pvp(None, &fake);
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::HostEnd, Some(1)));
    let st = e.stats.expect("the Final's counters");
    assert_eq!((st.players[0].kills, st.players[0].score, st.team_scores), (45, 595, [7, 9]));
    assert!(!st.details.is_empty() && st.details.iter().any(|d| d.badges.iter().any(|b| b.iter().any(|&n| n > 0))), "the Final's badge rows");
    // the client is still in the match: no closing DF yet, the DFs go on
    let c = &net.client.1;
    let enter = c.dfs.iter().find(|d| d.enter_mask != 0).unwrap().time;
    assert_eq!(fin(c), None, "not closed before the lobby's result");
    let got = fake.got(0);
    let close = got.iter().position(|m| matches!(m, ToHost::Close { .. })).expect("Close to the host");
    assert!(got[close + 1..].iter().all(|m| !matches!(m, ToHost::Df(_))), "no DF to the host after its Close");
    let ToHost::Close { t_last } = got[close] else { unreachable!() };
    assert!(t_last >= enter + 2300, "Close after the host read the end tick as final: {t_last} vs {}", enter + 2000);
    let before = c.dfs.len();
    net.run(300);
    assert!(net.client.1.dfs.len() > before, "the clients' DFs go on while the lobby sends its result");
    assert!(net.hub.take_ended().is_empty(), "one result");
    assert!(net.hub.holds_clients(SEED), "the lobby keeps the 85 00 back");
    // the lobby's end after its 83 45 0A + 97: code 2
    assert!(net.hub.end_match_after_result(SEED, net.now));
    net.run(200);
    assert!(!net.hub.holds_clients(SEED) && net.hub.has_match(SEED), "closed, the connections drain");
    let codes: Vec<u8> = net.client.1.dfs.iter().filter(|d| d.leave_mask & 1 != 0).map(|d| d.code).collect();
    assert!(!codes.is_empty() && codes.iter().all(|&c| c == 2), "the closing DFs keep the world: {codes:?}");
    assert!(net.client.1.closed);
    net.run(3000);
    assert!(net.hub.take_ended().is_empty(), "no second result after the close");
    assert!(net.hub.has_match(SEED), "a code-2 end drains at least FIN_KEEPS_WORLD_DRAIN");
    net.run(3500);
    assert!(!net.hub.has_match(SEED), "then the lobby's 85 00 may go");
    let ToHost::Create(cr) = &fake.got(0)[0] else { panic!("Create") };
    assert_eq!((cr.kind, cr.project.as_str()), (proto::KIND_PVP, ""));
}

/// The PvP end screen's backstop: a lobby that does not end the match after the early result -> the relay closes it
/// itself (code 2) `pvp_end_screen` later.
#[test]
fn a_pvp_end_screen_closes_by_itself_without_the_lobby() {
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((2000, 1)), final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pvp_screen(None, &fake, Duration::from_millis(800));
    let e = net.ended(10_000);
    assert_eq!(e.reason, EndReason::HostEnd);
    assert_eq!(fin(&net.client.1), None);
    assert!(net.until(3000, |n| fin(&n.client.1).is_some()), "the backstop closes");
    let codes: Vec<u8> = net.client.1.dfs.iter().filter(|d| d.leave_mask & 1 != 0).map(|d| d.code).collect();
    assert!(codes.iter().all(|&c| c == 2), "{codes:?}");
    net.run(3000);
    assert!(net.hub.take_ended().is_empty(), "no second result");

}

/// Phase 3, the empty-team end: slot 1 (team 1) leaves; the tdm rule recounts the present
/// players at the leave (0x200366A60: team 1 empty -> winner byte 0) and ends 3 s later (0x200366DB0); the
/// relay mirrors that end with winner 0. The host's Final can carry a later winner byte (in a replay of a recording the
/// fin DFs make slot 0 leave too: both teams empty -> 2, 0x200366B57). The relay's mirror winner is kept and the
/// disagreement logged; a host Outcome latched at the end tick would win.
#[test]
fn a_pvp_empty_team_end_keeps_the_mirrors_winner_over_a_later_final() {
    let rule = EndRule { countdown_ms: 200, match_ms: 600_000, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: true, draw_overtime_ms: 0 };
    let fake = Fake::new(vec![Plan { final_: Some((5, 2)), ..Plan::normal() }]);
    let mc = MatchConfig { world_host: Some(HostConfig::with_connector("fake-host", fake.connector())), ..MatchConfig::default() };
    let mut worlds = WorldStore::new();
    worlds.insert(test_world());
    let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds);
    let players = (0..2u64).map(|i| PlayerSpec { account_id: 100 + i, name: format!("p{i}"), team: i as u8, session: None, bot: false }).collect();
    let spec = MatchSpec { match_id: Some(SEED), map: MAP, mode: MODE, options: test_options(), players, level_checksum: None, roster: None, project: None, end_rule: Some(rule), pve_end: None, ai_ignore_players: false };
    let now = Instant::now();
    let t = hub.create_match(&spec, now).unwrap();
    let mut cs: Vec<Client> = t.players.iter().map(|p| Client::new(u64::from(p.slot), p.order_id, 9000 + u32::from(p.slot), now)).collect();
    for c in cs.iter_mut() {
        c.connect();
    }
    let c1 = cs.pop().unwrap();
    let c0 = cs.pop().unwrap();
    let mut net = Net { hub, client: ("127.0.0.1:5000".parse().unwrap(), c0), others: vec![("127.0.0.1:5001".parse().unwrap(), c1)], now };
    net.run(2000);
    assert!(net.client.1.entered && net.others[0].1.entered);
    net.others[0].1.leave();
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::TimeLimit, Some(0)), "team 1 has nobody left: team 0 wins, not the later Final's 2");
    assert_eq!(e.stats.expect("the Final's counters").players[0].kills, 45);
}

/// Phase 3: the relay's time-limit mirror closes first (the host reads the end tick 300 ms later); the MatchEnd waits for
/// the Final and takes the world's winner (the mirror knows none for a time end) and counters.
#[test]
fn a_pvp_time_limit_end_takes_the_hosts_winner_from_final() {
    let rule = EndRule { countdown_ms: 200, match_ms: 1500, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: false, draw_overtime_ms: 0 };
    let fake = Fake::new(vec![Plan { final_: Some((5, 2)), ..Plan::normal() }]);
    let mut net = pvp(Some(rule), &fake);
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::TimeLimit, Some(2)), "the world's draw byte");
    assert_eq!(e.stats.expect("the Final's counters").players[0].kills, 45);
}

/// The PvP end screen at a time-limit end: the mirror's close at T_end + 3 s waits for the host's Outcome (it reads the
/// end tick as final 300 ms later); the end then goes the end screen's way: the result first, the client still in the
/// match, the lobby's end with code 2.
#[test]
fn a_pvp_time_limit_end_waits_for_the_hosts_outcome_for_the_end_screen() {
    // mirror close at enter + 200 + 1500 + 3000 = enter + 4700; the host's Outcome for that tick
    let rule = EndRule { countdown_ms: 200, match_ms: 1500, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: false, draw_overtime_ms: 0 };
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((4700, 0)), final_: Some((5, 0)), ..Plan::normal() }]);
    let mut net = pvp(Some(rule), &fake);
    let e = net.ended(20_000);
    assert_eq!((e.reason, e.winner), (EndReason::HostEnd, Some(0)));
    assert_eq!(fin(&net.client.1), None, "still in the match for the result");
    assert!(net.hub.end_match_after_result(SEED, net.now));
    net.run(200);
    let codes: Vec<u8> = net.client.1.dfs.iter().filter(|d| d.leave_mask & 1 != 0).map(|d| d.code).collect();
    assert!(!codes.is_empty() && codes.iter().all(|&c| c == 2), "{codes:?}");
}

/// A PvP match of two humans (team 0 / 1, both connected) with the host; `tune` sets the host switches.
fn pvp2(fake: &Arc<Fake>, tune: impl FnOnce(&mut HostConfig)) -> Net {
    pvp2_cfg(fake, tune, |_| {})
}

fn pvp2_cfg(fake: &Arc<Fake>, tune: impl FnOnce(&mut HostConfig), tune_m: impl FnOnce(&mut MatchConfig)) -> Net {
    let mut h = HostConfig::with_connector("fake-host", fake.connector());
    tune(&mut h);
    let mut mc = MatchConfig { world_host: Some(h), ..MatchConfig::default() };
    tune_m(&mut mc);
    let mut worlds = WorldStore::new();
    worlds.insert(test_world());
    let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds);
    let players = (0..2u64).map(|i| PlayerSpec { account_id: 100 + i, name: format!("p{i}"), team: i as u8, session: None, bot: false }).collect();
    let spec = MatchSpec { match_id: Some(SEED), map: MAP, mode: MODE, options: test_options(), players, level_checksum: None, roster: None, project: None, end_rule: None, pve_end: None, ai_ignore_players: false };
    let now = Instant::now();
    let t = hub.create_match(&spec, now).unwrap();
    let mut cs: Vec<Client> = t.players.iter().map(|p| Client::new(u64::from(p.slot), p.order_id, 9000 + u32::from(p.slot), now)).collect();
    for c in cs.iter_mut() {
        c.connect();
    }
    let c1 = cs.pop().unwrap();
    let c0 = cs.pop().unwrap();
    Net { hub, client: ("127.0.0.1:5000".parse().unwrap(), c0), others: vec![("127.0.0.1:5001".parse().unwrap(), c1)], now }
}

fn enter_of(c: &Client) -> u32 {
    c.dfs.iter().find(|d| d.enter_mask != 0).expect("the enter").time
}

/// Phase 2 E0 (always on): the host's Tick of item t (sent after it stepped DF t + tick) goes to every entered
/// client at once as E0 with the host's mask, its per-player bytes of that tick (the respawn screen's per-player time,
/// 0x2001AE3B0) and its g0 / g1 / g2, from the enter DF + 500 ms on. Nothing while the link
/// is not live (here: a host that never sends Ready).
#[test]
fn e0_carries_the_hosts_mask_and_bytes_only_while_live() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(4000);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!(st.host, "live");
    for c in std::iter::once(&net.client.1).chain(net.others.iter().map(|(_, c)| c)) {
        assert!(c.e0s.len() > 20, "E0 every tick: {}", c.e0s.len());
        // mask bits 0, 1 and 19 (wire bytes 03 00 08), one byte per set bit in bit order, of the same tick
        assert!(c.e0s.iter().all(|e| e.players == 0x08_0003 && e.hashes == vec![0x5A, e.tick, 0x13]), "the host's mask and bytes: {:?}", c.e0s.first());
        let wire = <match_server::match_msgs::Hash as protocol::Encode>::to_bytes(&c.e0s[0]).unwrap();
        assert_eq!(wire[1..4], [0x03, 0x00, 0x08], "mask bytes in the reader's order (0x2001AE040)");
        assert!(c.e0s.iter().all(|e| e.globals[0] == e.tick && e.globals[2] == 0x77), "the host's g bytes of the same tick");
        let first = ((enter_of(c) + 500) / TICK) as u8;
        assert_eq!(c.e0s[0].tick, first, "from the enter DF + 500 ms");
    }
    assert!(st.e0_sent > 40);

    let fake = Fake::new(vec![Plan { ticks: true, ready: false, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(3000);
    assert_eq!(net.hub.stats(SEED).unwrap().host, "starting");
    assert!(net.client.1.e0s.is_empty(), "no E0 before the link is live");
}

/// Phase 2 CD-together rule: two CDs of the same time with 40 precise-time entries, a ping and a profile each (80 precise
/// entries > the client's 60): instead of splitting entries off, the whole bundle of the higher slot, its confirmation
/// included, moves to the next DF; each bundle arrives in one DF with its sender's confirmation.
#[test]
fn with_e0_a_cd_keeps_its_entries_and_its_confirmation_in_one_df() {
    use match_server::match_msgs::{ClientUpdate, PlayerInput, ProfileInput};
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    assert!(net.client.1.entered && net.others[0].1.entered);
    let x = net.client.1.next_cd.max(net.others[0].1.next_cd) + 4 * TICK;
    for c in std::iter::once(&mut net.client.1).chain(net.others.iter_mut().map(|(_, c)| c)) {
        c.send_inputs = false;
        let me = c.idx as u16;
        let cd = ClientUpdate {
            time: x,
            now: vec![PlayerInput { rot: [1, 2], actions: 3 }],
            late: (0..40u8).map(|dt| (dt, PlayerInput { rot: [dt.into(), me], actions: 1 })).collect(),
            profile: vec![ProfileInput { head: [9, 9, 9, me as u8], entries: vec![] }],
            ping: vec![70 + me],
        };
        c.send_cd(&cd);
    }
    net.run(1500);
    // the DF that carries the other slot's precise entries, from each client's view
    let bundle_df = |c: &Client, other: u8| -> (u32, usize, bool, bool) {
        let d = c.dfs.iter().find(|d| d.late.iter().any(|(_, p)| *p == other)).expect("the other slot's precise entries");
        let n = d.late.iter().filter(|(_, p)| *p == other).count();
        (d.time, n, d.ping.iter().any(|(p, v)| *p == other && *v == 70 + u16::from(other)), d.profile.iter().any(|(p, pi)| *p == other && pi.head[0] == 9))
    };
    let conf_df = |c: &Client| c.dfs.iter().find(|d| d.conf.iter().any(|(a, _)| *a == x)).expect("the confirmation").time;
    let (d1, n1, ping1, prof1) = bundle_df(&net.client.1, 1);
    let (d0, n0, ping0, prof0) = bundle_df(&net.others[0].1, 0);
    assert_eq!((n0, n1), (40, 40), "every precise entry of a CD in one DF");
    assert!(ping0 && ping1 && prof0 && prof1, "the ping and the profile with them");
    assert_ne!(d0, d1, "one bundle moved (80 precise entries do not fit the client's 60)");
    assert_eq!(conf_df(&net.others[0].1), d1, "slot 1's confirmation in the DF with its entries");
    assert_eq!(conf_df(&net.client.1), d0, "slot 0's confirmation in the DF with its entries");
    assert!(net.hub.stats(SEED).unwrap().df_deferred >= 1);
    // no entry reaches back to a tick the clients already hashed (they hash t after DF t + tick): every late dt < 2 ticks
    for c in std::iter::once(&net.client.1).chain(net.others.iter().map(|(_, c)| c)) {
        assert!(c.dfs.iter().all(|d| d.late.iter().all(|(dt, _)| u32::from(*dt) < 2 * TICK)), "late dt < 2 ticks");
    }
}

fn newest(c: &Client) -> u32 {
    c.dfs.iter().map(|d| d.time).max().expect("a DF")
}

fn lo(t: u32) -> u8 {
    ((t / TICK) & 0xFF) as u8
}

/// The time of an E0's `tick_lo` nearest to `around` (as the client rebuilds it).
fn near(tick_lo: u8, around: u32) -> u32 {
    let b = around / TICK;
    let d = tick_lo.wrapping_sub((b & 0xFF) as u8) as i8;
    (i64::from(b) + i64::from(d)) as u32 * TICK
}

fn cl(net: &mut Net, other: Option<usize>) -> &mut Client {
    match other {
        None => &mut net.client.1,
        Some(k) => &mut net.others[k].1,
    }
}

/// The two CCs of a lasting mismatch (resync rule 2): two ticks below the newest
/// DF, 300 ms later again two below the then newest (above the first); then `ms` of run.
fn cc_pair(net: &mut Net, other: Option<usize>, reason: u8, ms: u64) {
    let t = newest(cl(net, other)) - 2 * TICK;
    cl(net, other).send_cc_at(lo(t), reason);
    net.run(300);
    let t = newest(cl(net, other)) - 2 * TICK;
    cl(net, other).send_cc_at(lo(t), reason);
    net.run(ms);
}

/// A relay datagram to a client that carries part `index` of a DE.
fn carries_de_part(d: &[u8], index: u8) -> bool {
    use match_server::table::{id, Direction};
    use udp_transport::wire::Message;
    match_server::mtransport::parse(Direction::ToClient, d)
        .is_ok_and(|dg| dg.messages.iter().any(|m| matches!(m, Message::Data(x) if x.id == id::DE_WORLD && x.part.is_some_and(|p| p.index == index))))
}

fn want_des(fake: &Fake) -> usize {
    fake.got(0).iter().filter(|m| matches!(m, ToHost::WantDe { .. })).count()
}

/// Phase 2 resync (always on): a client's confirmed mismatch (two CCs) waits until the host's
/// newest final tick lies above its tick, then one WantDe; the host's DE (item + tail, its time and presence mask
/// checked) goes to that client only. Afterwards no E0 and no CC at or below the floor (the DE time, or the newest DF
/// when the client acked the DE) for it; a mismatch above it gets the next DE.
#[test]
fn resync_answers_a_cc_with_the_hosts_de_to_that_client_only() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    assert!(net.client.1.entered && net.others[0].1.entered);
    let (des0, des1) = (net.client.1.de_count, net.others[0].1.de_count);
    let t_cc = newest(&net.client.1) - 2 * TICK;
    net.client.1.send_cc_at(lo(t_cc - TICK), 4);
    net.run(300);
    net.client.1.send_cc_at(lo(t_cc), 4);
    net.run(1500);
    assert_eq!(net.client.1.de_count, des0 + 1, "one DE for the CCs");
    assert_eq!(net.others[0].1.de_count, des1, "nothing to the other client");
    let de = net.client.1.de.clone().unwrap();
    assert!(de.time > t_cc && de.time % TICK == 0, "a DE above the CC's tick ({} vs {t_cc})", de.time);
    let body = de.unpack().unwrap();
    assert_eq!((body.len(), &body[..4]), (600, &de.time.to_le_bytes()[..]), "the host's item + tail as it sent them");
    assert_eq!(want_des(&fake), 1);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.resync_asked, st.resync_sent, st.resync_refused, st.resync_acked, st.resync_unconfirmed), (1, 1, 0, 1, 1));
    assert!(st.e0_held >= 1, "E0 at or below the floor held back for the resynced client");
    // E0 still goes to the other client every tick
    assert!(net.others[0].1.e0s.len() > net.client.1.e0s.len());

    // a CC at or below the floor: its hashes predate the load, ignored
    net.client.1.send_cc_at(lo(de.time), 4);
    net.run(1000);
    assert_eq!((net.client.1.de_count, want_des(&fake)), (des0 + 1, 1));
    assert_eq!(net.hub.stats(SEED).unwrap().resync_ignored, 1);

    // a newer mismatch gets the next DE; reason 8 (the masks differ, always with a mask-0 E0) is a real mismatch too
    // (the reason is one value, not bit flags: CC sender 0x2001AE98F)
    cc_pair(&mut net, None, 8, 1500);
    assert_eq!((net.client.1.de_count, want_des(&fake)), (des0 + 2, 2));
}

/// Resync rule 2: one CC (a hash the client took by its 500 ms timeout, which the next equal compare clears) gets no
/// DE; a second CC above the first within `resync_confirm` (stale world state repeats a CC every 500 ms) does. A second
/// CC later than `resync_confirm` counts as a new first one.
#[test]
fn a_single_cc_gets_no_resync_de_a_second_one_above_it_does() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    let des = net.client.1.de_count;
    let t = newest(&net.client.1) - 2 * TICK;
    net.client.1.send_cc_at(lo(t), 4);
    net.run(2500);
    assert_eq!((net.client.1.de_count, want_des(&fake)), (des, 0), "one CC: no DE");
    // 2.5 s later: too late to confirm the first, a new first one
    let t = newest(&net.client.1) - 2 * TICK;
    net.client.1.send_cc_at(lo(t), 4);
    net.run(300);
    assert_eq!(want_des(&fake), 0);
    // a CC at the first's own tick (a repeat) confirms nothing
    net.client.1.send_cc_at(lo(t), 4);
    net.run(300);
    assert_eq!(want_des(&fake), 0);
    let t = newest(&net.client.1) - 2 * TICK;
    net.client.1.send_cc_at(lo(t), 4);
    net.run(1500);
    assert_eq!((net.client.1.de_count, want_des(&fake)), (des + 1, 1), "the confirming CC: one DE");
    assert_eq!(net.hub.stats(SEED).unwrap().resync_unconfirmed, 3);
}

/// Resync rule 1: a lost DE part. The DFs the relay sends after the DE reach the client before the part's resend
/// completes the DE (the client hashes them in its old world and never again, 0x2001AFD05). The floor is the newest DF
/// when the client acked every part, and from the send to the ack the client gets no E0: every E0 it gets afterwards
/// lies above the newest DF it had when the DE landed. (A floor at the send would let the E0 of the overtaking DFs'
/// ticks meet the client's old hashes: a mismatch, a CC and another DE.)
#[test]
fn a_lost_de_part_moves_the_resync_floor_to_the_ack() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    let des = net.client.1.de_count;
    let t = newest(&net.client.1) - 2 * TICK;
    net.client.1.send_cc_at(lo(t), 4);
    net.run(300);
    let t = newest(&net.client.1) - 2 * TICK;
    net.client.1.send_cc_at(lo(t), 4);
    let (mut dropped, mut at_send, mut landed) = (0, None, None);
    for _ in 0..600 {
        net.step_filtered(&mut |d| {
            let lose = dropped == 0 && carries_de_part(d, 1);
            dropped += u32::from(lose);
            lose
        });
        if at_send.is_none() && net.hub.stats(SEED).unwrap().resync_sent == 1 {
            at_send = Some((net.client.1.e0s.len(), newest(&net.client.1)));
        }
        if net.client.1.de_count > des {
            landed = Some(newest(&net.client.1));
            break;
        }
        std::thread::sleep(Duration::from_micros(300));
    }
    assert_eq!(dropped, 1, "a DE part was lost");
    let ((n0, df_at_send), landed) = (at_send.expect("the DE went out"), landed.expect("the DE landed after the resend"));
    assert!(landed > df_at_send, "DFs overtook the resent part ({landed} vs {df_at_send})");
    net.run(1500);
    let after: Vec<u32> = net.client.1.e0s[n0..].iter().map(|h| near(h.tick, landed)).collect();
    assert!(!after.is_empty(), "E0 again after the ack");
    assert!(after.iter().all(|&t| t > landed), "no E0 at or below the newest DF the client had when the DE landed ({landed}): {after:?}");
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.resync_sent, st.resync_acked, st.resync_expired), (1, 1, 0));
    assert!(st.e0_held >= 1);
    // a CC above the new floor within 2 s of the ack is logged and counted as a re-trigger (it still needs a second CC)
    let t = newest(&net.client.1) - 2 * TICK;
    net.client.1.send_cc_at(lo(t), 4);
    net.run(100);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.resync_retriggers, st.resync_asked), (1, 1));
}

/// Resync rule 3: no DE whose time lies in [S - 500, S + tick) of a scripted step S that adds to an empty modifier
/// record; here S = the relay's own PvE mission start (`start_match_instantly`, FF -1: the enter DF + `start_all_ms`).
/// A mismatch just before S gets its DE only once the host's final tick passed S.
#[test]
fn no_resync_de_inside_the_mission_start_guard() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let rule = PveEndRule { start_all_ms: 2000, ..long() };
    let mut net = pve(rule, &fake, |_| {});
    assert!(net.until(4000, |n| n.client.1.entered));
    net.run(200);
    let start = enter_of(&net.client.1) + 2000;
    assert!(net.until(5000, |n| newest(&n.client.1) >= start - 400));
    let des = net.client.1.de_count;
    net.client.1.send_cc_at(lo(start - 500), 4);
    net.run(100);
    net.client.1.send_cc_at(lo(start - 450), 4);
    assert!(net.until(4000, |n| n.client.1.de_count > des), "a DE after the step");
    let de = net.client.1.de.clone().unwrap();
    assert!(de.time >= start + TICK, "DE at {} inside the guard [{}, {})", de.time, start - 500, start + TICK);
    assert!(net.hub.stats(SEED).unwrap().resync_deferred >= 1);
    assert_eq!(want_des(&fake), 1, "no WantDe while the DE's time lay in the guard");
}

/// Resync rule 4: at most `resync_max` host DEs per client within `resync_window`; the next CC trips the breaker for
/// `resync_cooldown` (its CCs are ignored), then resync is on again. At `resync_match_max` DEs in the match it stays off
/// for that client. A CC with reason 1 after a host DE (the DE did not land) trips it at once.
#[test]
fn the_resync_breaker_cools_down_and_the_match_budget_ends_it() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |h| {
        h.resync_max = 1;
        h.resync_cooldown = Duration::from_secs(3);
        h.resync_match_max = 3;
    });
    net.run(2000);
    let des = net.client.1.de_count;
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 1);
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 1, "a CC after resync_max DEs in the window trips the breaker");
    assert_eq!(net.hub.stats(SEED).unwrap().resync_tripped, 1);
    // the other client is not affected
    let d1 = net.others[0].1.de_count;
    cc_pair(&mut net, Some(0), 4, 1500);
    assert_eq!(net.others[0].1.de_count, d1 + 1);
    // the cooldown is over: on again with an empty window
    net.run(1500);
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 2, "on again after the cooldown");
    cc_pair(&mut net, None, 4, 1500);
    net.run(3000);
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 3);
    // the match budget (3) is used up: off for the rest of the match
    net.run(3500);
    cc_pair(&mut net, None, 4, 1500);
    net.run(3500);
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 3, "off for the rest of the match");
    assert_eq!(net.hub.stats(SEED).unwrap().resync_tripped, 3, "two cooldowns and the match budget");

    // reason 1 after a DE: tripped at once
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    let des = net.client.1.de_count;
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 1);
    cc_pair(&mut net, None, 1, 1500);
    assert_eq!(net.client.1.de_count, des + 1, "a DE that did not land: no second one");
    assert_eq!(net.hub.stats(SEED).unwrap().resync_tripped, 1);
}

/// Resync rule 4: only host DEs that fail in a row count. Refused, refused, delivered, refused, refused, delivered:
/// never three in a row, so resync stays on (four failures in all switched it off before).
#[test]
fn only_failed_host_des_in_a_row_switch_resync_off() {
    let fake = Fake::new(vec![Plan { ticks: true, de_script: vec![2, 2, 1, 2, 2, 1], ..Plan::normal() }]);
    let mut net = pvp2(&fake, |h| {
        h.resync_max = 5;
    });
    net.run(2000);
    let des = net.client.1.de_count;
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 1, "two refusals, then the third WantDe's DE");
    cc_pair(&mut net, None, 4, 1500);
    assert_eq!(net.client.1.de_count, des + 2, "two more refusals do not switch resync off");
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.resync_refused, st.resync_sent, st.resync_acked), (4, 2, 2));
    assert_eq!(want_des(&fake), 6);
}

/// A refused WantDe (the host's `Fault "WantDe <id>: ..."`) is not a link failure: the host stays live, no restart, the
/// client gets nothing. A De the relay's checks refuse (a presence mask with a slot that never entered) goes to nobody.
/// Three failed DEs in a row switch resync off for the match. Without E0 the switch does nothing (no WantDe).
#[test]
fn refused_or_bad_host_des_reach_no_client_and_never_restart_the_host() {
    for de in [2u8, 3] {
        let fake = Fake::new(vec![Plan { ticks: true, de, ..Plan::normal() }]);
        let mut net = pvp2(&fake, |_| {});
        net.run(2000);
        let des = net.client.1.de_count;
        for _ in 0..4 {
            cc_pair(&mut net, None, 4, 1500);
        }
        assert_eq!(net.client.1.de_count, des, "no DE (plan {de})");
        let st = net.hub.stats(SEED).unwrap();
        assert_eq!((st.host, st.host_restarts), ("live", 0), "plan {de}");
        assert_eq!((st.resync_refused, st.resync_sent), (3, 0), "plan {de}: three failures in a row, then resync off for the match");
        assert_eq!(want_des(&fake), 3);
    }

}

/// Protocol 4: a hub with no stored world; the world host builds every start world. `ids` = one PvE match per id (each
/// with its own client: the first is `client`, the rest `others`), all of project `pve_01_v1_maze2`.
fn host_built(fake: &Arc<Fake>, ids: &[u32], rule: PveEndRule, tune: impl FnOnce(&mut MatchConfig)) -> (Net, Vec<Result<match_server::MatchTicket, match_server::CreateError>>) {
    let mut mc = MatchConfig { world_host: Some(HostConfig::with_connector("fake-host", fake.connector())), pve_handover: None, ..MatchConfig::default() };
    tune(&mut mc);
    let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, WorldStore::new());
    let now = Instant::now();
    let mut tickets = vec![];
    let mut clients = vec![];
    for (k, id) in ids.iter().enumerate() {
        let mut players = vec![PlayerSpec { account_id: 100 + k as u64, name: format!("p{k}"), team: 0, session: None, bot: false }];
        players.extend((1..=3).map(|b| PlayerSpec::bot(&format!("pve_bot_{b}"), 1)));
        let spec = MatchSpec {
            match_id: Some(*id),
            map: MAP,
            mode: MODE,
            options: options_with_id(*id),
            players,
            level_checksum: None,
            roster: None,
            project: Some("pve_01_v1_maze2".into()),
            end_rule: None,
            pve_end: Some(rule),
            ai_ignore_players: false,
        };
        let t = hub.create_match(&spec, now);
        if let Ok(t) = &t {
            let mut c = Client::new(k as u64, t.players[0].order_id, 9000 + k as u32, now);
            c.connect();
            clients.push((format!("127.0.0.1:{}", 5000 + k).parse().unwrap(), c));
        }
        tickets.push(t);
    }
    let client = if clients.is_empty() { ("127.0.0.1:5999".parse().unwrap(), Client::new(9, 1, 1, now)) } else { clients.remove(0) };
    (Net { hub, client, others: clients, now }, tickets)
}

/// Protocol 4: the Create has no world; the host's Ready brings the start world, and the client's DD (checksum) and DE
/// (time, item) and the DF clock come from it; the match then runs with the host as usual. A restarted child gets a
/// Create that carries that very world (it loads the bytes the clients loaded).
#[test]
fn the_world_host_builds_the_start_world() {
    let fake = Fake::new(vec![Plan { crash_after: Some(30), ..Plan::normal() }, Plan { outcome_after: Some((60, 0)), ..Plan::normal() }]);
    let (mut net, t) = host_built(&fake, &[SEED], long(), |_| {});
    assert_eq!(t[0].as_ref().unwrap().match_id, SEED);
    assert!(net.until(8000, |n| n.client.1.entered), "the client entered");
    let c = &net.client.1;
    assert_eq!(c.dd.as_ref().unwrap().level_checksum, HOST_CHECKSUM, "DD carries the host's collision checksum");
    let de = c.de.as_ref().expect("the first DE");
    assert_eq!(de.time, HOST_T0, "the DE is the host's start world");
    assert_eq!(c.dfs.first().unwrap().time, HOST_T0 + TICK, "the DF clock starts at the host's world time");
    let ToHost::Create(c0) = &fake.got(0)[0] else { panic!("Create first") };
    assert_eq!((c0.version, c0.world.len(), c0.project.as_str()), (proto::PROTO_VERSION, 0, "pve_01_v1_maze2"), "protocol 4: an empty world");
    let o = net.outcome(10_000);
    assert!(o.pve_success);
    assert_eq!(net.host(), ("live", 1), "restarted once");
    let ToHost::Create(c1) = &fake.got(1)[0] else { panic!("Create first on the restart") };
    assert_eq!(c1.world, host_world().item().unwrap(), "the restart's Create carries the start world");
    assert_eq!(fake.dfs(1)[0].t, HOST_T0 + TICK, "and every DF from the first");
}

/// The world host must deliver the start world: a refused Create, no socket and no Ready within `host_start_timeout`
/// each end the match before anybody entered (`NoWorld`): no DD, no DE, never a hang.
#[test]
fn no_start_world_ends_the_match_before_anybody_entered() {
    for (what, fake) in [
        ("refused", Fake::new(vec![Plan { refuse: true, ..Plan::normal() }])),
        ("no socket", Arc::new(Fake { plans: vec![Plan::normal()], no_socket: true, ..Default::default() })),
        ("no Ready", Fake::new(vec![Plan::silent()])),
    ] {
        let (mut net, t) = host_built(&fake, &[SEED], long(), |mc| mc.host_start_timeout = Duration::from_secs(3));
        assert!(t[0].is_ok(), "{what}: the match is created");
        let e = net.ended(8000);
        assert_eq!(e.reason, EndReason::NoWorld, "{what}");
        assert!(e.players.iter().all(|p| !p.entered), "{what}: nobody entered");
        assert!(net.client.1.dd.is_none() && net.client.1.de.is_none(), "{what}: no DD, no DE");
    }
    // a hub without host and without a stored world refuses at once
    let mut hub = Hub::new(HubConfig { port: 1236, ..Default::default() }, WorldStore::new());
    let spec = MatchSpec {
        match_id: None,
        map: MAP,
        mode: MODE,
        options: test_options(),
        players: vec![PlayerSpec { account_id: 1, name: "p".into(), team: 0, session: None, bot: false }],
        level_checksum: None,
        roster: None,
        project: Some("pve_01_v1_maze2".into()),
        end_rule: None,
        pve_end: None,
        ai_ignore_players: false,
    };
    assert!(hub.create_match(&spec, Instant::now()).is_err());
}

/// Two matches of one map at the same time, each with its own id and its own host-built start world.
#[test]
fn two_matches_of_one_map_run_at_once() {
    let fake = Fake::new(vec![Plan::normal(), Plan::normal()]);
    let (a, b) = (0x1A2B_0001, 0x1A2B_0002);
    let (mut net, t) = host_built(&fake, &[a, b], long(), |_| {});
    assert_eq!(t.iter().map(|t| t.as_ref().unwrap().match_id).collect::<Vec<_>>(), vec![a, b]);
    assert!(net.until(8000, |n| n.client.1.entered && n.others[0].1.entered), "both clients entered");
    assert_eq!(net.hub.running_matches(), 2);
    let ids: Vec<u32> = (0..2).map(|k| if let ToHost::Create(c) = &fake.got(k)[0] { c.match_id } else { 0 }).collect();
    assert!(ids.contains(&a) && ids.contains(&b), "one Create per match: {ids:x?}");
    for id in [a, b] {
        assert_eq!(net.hub.stats(id).unwrap().host, "live");
    }
}

/// Resync rule 3 with a host-built PvE start world (the host's first step at 850): pve.lua `match` already ran in the
/// start world, the relay's mission start (`start_match_instantly`, the enter DF + `start_all_ms`) is still a guarded
/// step: a mismatch just before it gets its DE only once the host's final tick passed it.
#[test]
fn the_mission_start_guard_holds_with_a_host_built_start_world() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let rule = PveEndRule { start_all_ms: 2000, ..long() };
    let (mut net, _) = host_built(&fake, &[SEED], rule, |_| {});
    assert!(net.until(4000, |n| n.client.1.entered));
    assert_eq!(net.client.1.de.as_ref().unwrap().time, HOST_T0, "the host's start world");
    net.run(200);
    let start = enter_of(&net.client.1) + 2000;
    assert!(net.until(5000, |n| newest(&n.client.1) >= start - 400));
    let des = net.client.1.de_count;
    net.client.1.send_cc_at(lo(start - 500), 4);
    net.run(100);
    net.client.1.send_cc_at(lo(start - 450), 4);
    assert!(net.until(4000, |n| n.client.1.de_count > des), "a DE after the step");
    let de = net.client.1.de.clone().unwrap();
    assert!(de.time >= start + TICK, "DE at {} inside the guard [{}, {})", de.time, start - 500, start + TICK);
    assert!(net.hub.stats(SEED).unwrap().resync_deferred >= 1);
    assert_eq!(want_des(&fake), 1, "no WantDe while the DE's time lay in the guard");
}

/// Slot `slot`'s input of this tick in DF `d` (not its late entries).
fn now_input(d: &GameUpdate, slot: u8) -> Option<PlayerInput> {
    if d.input_mask >> slot & 1 == 0 {
        return None;
    }
    let k = (d.input_mask & ((1u32 << slot) - 1)).count_ones() as usize;
    Some(d.inputs[d.late.len() + k])
}

/// Slot `slot`'s late entries in DF `d`: (their own time, input).
fn late_inputs(d: &GameUpdate, slot: u8) -> Vec<(u32, PlayerInput)> {
    d.late.iter().enumerate().filter(|(_, (_, p))| *p == slot).map(|(j, (dt, _))| (d.time - u32::from(*dt), d.inputs[j])).collect()
}

/// The client's DFs by time (a resent DF once).
fn by_time(c: &Client) -> std::collections::BTreeMap<u32, GameUpdate> {
    c.dfs.iter().map(|d| (d.time, d.clone())).collect()
}

fn first_enter(c: &Client, slot: u8) -> u32 {
    c.dfs.iter().find(|d| d.enter_mask >> slot & 1 != 0).map(|d| d.time).expect("the bots entered")
}

/// [`Net::run`] with a real pause every step: the relay holds DFs for the fake host's `Bots`, which answers in real time.
fn run_paced(net: &mut Net, ms: u64) {
    for _ in 0..ms / 5 {
        net.step();
        std::thread::sleep(Duration::from_micros(300));
    }
}

fn the_bot_input(t: u32) -> PlayerInput {
    PlayerInput { rot: [(t / TICK) as u16, 0x1234], actions: 0x40 }
}

/// Protocol 7: `MatchSpec.ai_ignore_players` (a custom room's "bots ignore players") is the `Create`'s flag, next to
/// `drive_bots`; a spec without it sends false.
#[test]
fn create_carries_ai_ignore_players() {
    for on in [true, false] {
        let fake = Fake::new(vec![Plan::normal()]);
        let h = HostConfig::with_connector("fake-host", fake.connector());
        let mc = MatchConfig { world_host: Some(h), pve_handover: None, ..MatchConfig::default() };
        let mut worlds = WorldStore::new();
        worlds.insert(test_world().with_project("pve_01_v1_maze2").unwrap());
        let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds);
        let mut players = vec![PlayerSpec { account_id: 100, name: "p0".into(), team: 0, session: None, bot: false }];
        players.push(PlayerSpec::bot("bot_1", 1));
        let spec = MatchSpec {
            match_id: Some(SEED),
            map: MAP,
            mode: MODE,
            options: test_options(),
            players,
            level_checksum: None,
            roster: None,
            project: Some("pve_01_v1_maze2".into()),
            end_rule: None,
            pve_end: None,
            ai_ignore_players: on,
        };
        hub.create_match(&spec, Instant::now()).unwrap();
        let t0 = Instant::now();
        while fake.got(0).is_empty() && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let ToHost::Create(c) = &fake.got(0)[0] else { panic!("Create first") };
        assert_eq!((c.version, c.drive_bots, c.ai_ignore_players), (proto::PROTO_VERSION, true, on));
    }
}

/// Protocol 5: with `bots` on, Create asks the host to drive the bots; every bot input of a
/// `Bots` frame is in its own DF `t_for` (DF t waits for the `Bots` of the step before), every ai entry once in its DF,
/// nothing late or lost.
#[test]
fn host_bots_go_into_their_own_df() {
    let fake = Fake::new(vec![Plan { bots: true, ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |h| {
        h.bots_hold_ms = 5000;
    });
    assert!(net.until(5000, |n| n.client.1.entered));
    run_paced(&mut net, 6000);
    let ToHost::Create(c) = &fake.got(0)[0] else { panic!("Create first") };
    assert!(c.drive_bots && c.version >= 5);
    let st = net.hub.bots_stats(SEED).unwrap();
    assert!(st.frames > 60 && st.inputs > 50, "{st:?}");
    assert_eq!((st.late_inputs, st.ai_lost, st.hold_timeouts, st.duplicates), (0, 0, 0, 0), "{st:?}");
    let e = first_enter(&net.client.1, 1);
    let dfs = by_time(&net.client.1);
    let last = *dfs.keys().last().unwrap();
    let (mut inputs, mut ai) = (0u32, 0u32);
    for (&t, d) in dfs.range(e + TICK..last) {
        assert_eq!(now_input(d, 1), Some(the_bot_input(t)), "DF {t}: slot 1's host input");
        assert!(late_inputs(d, 1).is_empty(), "DF {t}: no late bot entry");
        let want: Vec<(u8, u16, u16)> = if (t / TICK) % 4 == 0 { vec![(2, 1, (t / TICK) as u16)] } else { vec![] };
        assert_eq!(d.ai, want, "DF {t}: the host's ai entries");
        assert_eq!((now_input(d, 2), now_input(d, 3)), (None, None));
        inputs += 1;
        ai += d.ai.len() as u32;
    }
    assert!(inputs > 50);
    assert!(ai > 10);
    assert_eq!(st.ai, dfs.values().map(|d| d.ai.len() as u32).sum::<u32>(), "every filed ai entry reached the client");
}

/// A `Bots` frame that comes after its DF: the DF waits `bots_hold_ms`, then goes without it; the frame's input goes
/// out late with its own time (dt under 2 ticks), its ai entry is lost (ai entries carry no dt) and never reaches a client.
#[test]
fn a_late_bots_frame_holds_then_its_input_goes_late_and_its_ai_is_lost() {
    let fake = Fake::new(vec![Plan { bots: true, bots_late_first_ai: true, ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |h| {
        h.bots_hold_ms = 30;
    });
    assert!(net.until(5000, |n| n.client.1.entered));
    run_paced(&mut net, 4000);
    let held = fake.held.lock().unwrap().clone();
    assert_eq!(held.len(), 1);
    let t_late = held[0];
    let st = net.hub.bots_stats(SEED).unwrap();
    assert!(st.hold_timeouts >= 1 && st.late_inputs >= 1 && st.ai_lost >= 1, "{st:?}");
    let dfs = by_time(&net.client.1);
    assert_eq!(now_input(&dfs[&t_late], 1), None, "DF {t_late} went out without the bot");
    let late: Vec<(u32, u32, PlayerInput)> = dfs.values().flat_map(|d| late_inputs(d, 1).into_iter().map(|(at, i)| (d.time, at, i))).collect();
    assert!(late.iter().any(|(_, at, _)| *at == t_late), "the held input went out late: {late:?}");
    for (t, at, i) in &late {
        assert_eq!(*i, the_bot_input(*at), "a late bot entry keeps its own time");
        assert!(t - at < 2 * TICK, "DF {t}: dt {} reaches back 2 ticks", t - at);
    }
    assert!(dfs.values().all(|d| !d.ai.contains(&(2, 1, (t_late / TICK) as u16))), "the late ai entry reached no client");
    let n_ai: usize = dfs.values().map(|d| d.ai.len()).sum();
    assert!(n_ai > 0 && n_ai as u32 <= st.ai);
}

/// A restarted host catches up from the first DF and sends its `Bots` again: frames for times already filed are
/// dropped (no bot input or ai entry twice); the bot inputs go on after the catch-up.
#[test]
fn a_restarted_hosts_bots_are_not_filed_twice() {
    let fake = Fake::new(vec![Plan { bots: true, crash_after: Some(80), ..Plan::normal() }, Plan { bots: true, ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |h| {
        h.bots_hold_ms = 2000;
    });
    assert!(net.until(5000, |n| n.client.1.entered));
    assert!(net.until(20_000, |n| n.host() == ("live", 1) && fake.dfs(1).len() > 140));
    run_paced(&mut net, 2000);
    let st = net.hub.bots_stats(SEED).unwrap();
    assert!(st.duplicates >= 60, "the replayed frames: {st:?}");
    let dfs = by_time(&net.client.1);
    let mut seen = std::collections::HashSet::new();
    for d in dfs.values() {
        for a in &d.ai {
            assert!(seen.insert(*a), "ai entry {a:?} twice");
        }
        for (at, i) in late_inputs(d, 1) {
            assert_eq!(i, the_bot_input(at));
        }
        if let Some(i) = now_input(d, 1) {
            assert_eq!(i, the_bot_input(d.time));
        }
    }
    let last = *dfs.keys().last().unwrap();
    let tail: Vec<&GameUpdate> = dfs.range(last - 20 * TICK..last).map(|(_, d)| d).collect();
    assert!(tail.iter().all(|d| now_input(d, 1).is_some()), "the bot inputs go on after the catch-up");
}

/// The host stops for good while driving the bots: one neutral input per bot slot in one DF (the clients repeat a
/// slot's last input), then no bot input at all.
#[test]
fn a_host_gone_for_good_leaves_one_neutral_input_per_bot() {
    let fake = Fake::new(vec![Plan { bots: true, crash_after: Some(80), ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |h| {
        h.bots_hold_ms = 2000;
        h.max_restarts = 0;
    });
    assert!(net.until(5000, |n| n.client.1.entered));
    assert!(net.until(20_000, |n| n.host().0 == "off"), "{:?}", net.host());
    net.run(2000);
    let dfs = by_time(&net.client.1);
    for slot in 1..=3u8 {
        let neutral: Vec<u32> = dfs.values().filter(|d| now_input(d, slot) == Some(PlayerInput::default())).map(|d| d.time).collect();
        assert_eq!(neutral.len(), 1, "slot {slot}: {neutral:?}");
        assert!(dfs.range(neutral[0] + 1..).all(|(_, d)| now_input(d, slot).is_none() && late_inputs(d, slot).is_empty()), "slot {slot}: nothing after it");
    }
}

/// `Diverged` from a host that drives the bots: the link stays (the clients need the bots), but its outcome is never
/// used; the bot inputs go on.
#[test]
fn a_diverged_host_keeps_driving_the_bots_without_its_outcome() {
    let fake = Fake::new(vec![Plan { bots: true, diverge_after: Some(60), outcome_after: Some((90, 0)), ..Plan::normal() }]);
    let mut net = pve(long(), &fake, |h| {
        h.bots_hold_ms = 2000;
    });
    assert!(net.until(5000, |n| n.client.1.entered));
    assert!(net.until(20_000, |_| fake.dfs(0).len() > 130));
    assert!(net.hub.take_outcomes().is_empty(), "no outcome of a diverged host");
    assert_eq!(net.host(), ("live", 0));
    assert_eq!(fake.conns.load(Ordering::SeqCst), 1);
    let st = net.hub.bots_stats(SEED).unwrap();
    assert!(st.frames > 120, "{st:?}");
    let dfs = by_time(&net.client.1);
    let after = fake.dfs(0)[100].t;
    assert!(dfs.range(after..after + 20 * TICK).all(|(t, d)| now_input(d, 1) == Some(the_bot_input(*t))), "the bot inputs go on");
    assert!(net.hub.end_match(SEED, net.now));
    net.step();
    let e = net.hub.take_ended();
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].winner, e[0].stats.clone()), (None, None), "nothing of the diverged host");
}

/// One human (team 0) and 11 AI players (5 on team 0, 6 on team 1) of a PvP match, the host driving them.
fn pvp_bots(fake: &Arc<Fake>, tune: impl FnOnce(&mut HostConfig)) -> Net {
    let mut h = HostConfig::with_connector("fake-host", fake.connector());
    tune(&mut h);
    let mc = MatchConfig { world_host: Some(h), ..MatchConfig::default() };
    let mut worlds = WorldStore::new();
    worlds.insert(test_world());
    let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds);
    let mut players = vec![PlayerSpec { account_id: 100, name: "p0".into(), team: 0, session: None, bot: false }];
    players.extend((0..11u8).map(|k| PlayerSpec::bot(&format!("bot {k}"), if k < 5 { 0 } else { 1 })));
    let spec = MatchSpec { match_id: Some(SEED), map: MAP, mode: MODE, options: test_options(), players, level_checksum: None, roster: None, project: None, end_rule: None, pve_end: None, ai_ignore_players: false };
    let now = Instant::now();
    let t = hub.create_match(&spec, now).unwrap();
    assert_eq!(t.players.len(), 1, "one ticket: the human");
    let mut c = Client::new(0, t.players[0].order_id, 9000, now);
    c.connect();
    Net { hub, client: ("127.0.0.1:5000".parse().unwrap(), c), others: vec![], now }
}

/// A PvP match with AI players asks the host to drive them (Create kind PvP,
/// drive_bots); every bot enters in the human's enter DF and gets its spawn profile `{0, 1, 1, 0}` with no entries in
/// DF enter + tick (no script spawns them); the host's bot inputs go into their DFs; the MatchEnd lists the bots with
/// their teams (the lobby tells enemy kills from team kills with them) and the humans alone as players.
#[test]
fn pvp_ai_players_enter_spawn_and_follow_the_hosts_inputs() {
    let fake = Fake::new(vec![Plan { bots: true, final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pvp_bots(&fake, |h| {
        h.bots_hold_ms = 5000;
    });
    assert!(net.until(5000, |n| n.client.1.entered));
    run_paced(&mut net, 3000);
    let ToHost::Create(c) = &fake.got(0)[0] else { panic!("Create first") };
    assert_eq!((c.kind, c.drive_bots), (proto::KIND_PVP, true));
    assert_eq!(c.records.iter().filter(|r| r.bot).count(), 11);
    let e = first_enter(&net.client.1, 1);
    let dfs = by_time(&net.client.1);
    assert_eq!(dfs[&e].enter_mask & 0xFFF, 0xFFF, "the human and the 11 bots enter together");
    assert!(dfs[&e].profile.iter().any(|(p, _)| *p == 0), "the human's start profile in the enter DF");
    let spawn: Vec<(u8, [u8; 4], usize)> = dfs[&(e + TICK)].profile.iter().map(|(p, x)| (*p, x.head, x.entries.len())).collect();
    assert_eq!(spawn, (1..=11u8).map(|p| (p, match_server::BOT_SPAWN_PROFILE, 0)).collect::<Vec<_>>(), "one spawn profile per bot in DF enter + tick");
    assert!(dfs.iter().filter(|(t, _)| **t != e + TICK).all(|(_, d)| d.profile.iter().all(|(p, _)| *p == 0)), "no other bot profile");
    let last = *dfs.keys().last().unwrap();
    assert!(dfs.range(e + TICK..last).all(|(t, d)| now_input(d, 1) == Some(the_bot_input(*t))), "the host's inputs for bot slot 1");
    let st = net.hub.bots_stats(SEED).unwrap();
    assert!(st.frames > 40 && st.inputs > 40 && st.late_inputs == 0, "{st:?}");
    assert!(net.hub.end_match(SEED, net.now));
    let end = net.ended(5000);
    assert_eq!(end.players.len(), 1);
    assert_eq!(end.bots.len(), 11);
    assert!(end.bots.iter().all(|b| b.entered && b.account_id == 0));
    assert_eq!(end.bots.iter().map(|b| (b.slot, b.team)).collect::<Vec<_>>(), (1..=11u8).map(|k| (k, if k <= 5 { 0 } else { 1 })).collect::<Vec<_>>());
}

/// artefact hunt's draw time: the time-limit mirror closes at T_end + 3 s, but on a draw the world plays on for
/// `draw_overtime_ms`. With a live host the relay waits for the host's Outcome and ends from it (winner and all); without
/// an overtime the mirror closes at T_end + 3 s; a host that sends no Outcome is waited for at most the draw time + 5 s.
#[test]
fn an_ah_draw_time_waits_for_the_hosts_outcome() {
    // mirror: enter + 200 countdown + 1500 match + 3000 = enter + 4700; the host's world ends in its draw time at enter
    // + 6000 (Outcome after DF + 300)
    let rule = EndRule { countdown_ms: 200, match_ms: 1500, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: false, draw_overtime_ms: 2000 };
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((6000, 1)), final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pvp(Some(rule), &fake);
    let e = net.ended(20_000);
    assert_eq!((e.reason, e.winner), (EndReason::HostEnd, Some(1)), "the host's end after the mirror's time end");
    // the end screen: the lobby ends it after its result
    assert!(net.hub.end_match_after_result(SEED, net.now));
    net.run(200);
    let enter = enter_of(&net.client.1);
    let closed = fin(&net.client.1).expect("closed");
    assert!(closed > enter + 6000, "closed after the world's own end ({closed} vs enter {enter})");

    // no overtime: the mirror's close at its time end waits `mirror_wait` (1 s) for the host's Outcome (the end screen),
    // then closes before the host's later Outcome
    let rule0 = EndRule { draw_overtime_ms: 0, ..rule };
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((6000, 1)), final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pvp(Some(rule0), &fake);
    let e = net.ended(20_000);
    assert_eq!(e.reason, EndReason::TimeLimit);
    let (enter, closed) = (enter_of(&net.client.1), fin(&net.client.1).expect("closed"));
    assert!(closed > enter + 4700 && closed <= enter + 4700 + 1000 + 100, "the mirror's close at enter + 4700 + mirror_wait ({closed} vs enter {enter})");

    // a host without an Outcome: held at most draw time + 5 s past the mirror's close
    let fake = Fake::new(vec![Plan { final_: Some((5, 2)), ..Plan::normal() }]);
    let mut net = pvp(Some(rule), &fake);
    let e = net.ended(30_000);
    assert_eq!(e.reason, EndReason::TimeLimit);
    let (enter, closed) = (enter_of(&net.client.1), fin(&net.client.1).expect("closed"));
    assert!(closed > enter + 4700 + 2000 && closed <= enter + 4700 + 7000 + 1000 + 100, "{closed} vs enter {enter}");
}

/// A PvP world that ended normally with no team winning (state 5, winner byte 3: artefact hunt after its draw time, a
/// tied battery retrieval or slaughter) reaches the lobby as a draw (2), from the Outcome and from a Final alike; a team
/// win stays the team.
#[test]
fn a_world_end_without_a_winner_is_a_draw_for_the_lobby() {
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((2000, 3)), final_: Some((5, 3)), ..Plan::normal() }]);
    let mut net = pvp(None, &fake);
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::HostEnd, Some(2)));
    // the relay's time-limit mirror first, then the host's Final with state 5 / winner 3
    let rule = EndRule { countdown_ms: 200, match_ms: 1500, advantage_ms: 300, wait_ratio: 0.6, leave_ratio: 0.51, team_empty_end: false, draw_overtime_ms: 0 };
    let fake = Fake::new(vec![Plan { final_: Some((5, 3)), ..Plan::normal() }]);
    let mut net = pvp(Some(rule), &fake);
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::TimeLimit, Some(2)));
    assert_eq!((match_server::pvp_winner(5, 3), match_server::pvp_winner(4, 3), match_server::pvp_winner(5, 1), match_server::pvp_winner(5, 2)), (2, 3, 1, 2));
    // the gun race (all versus all, docs/notes/gun-race.md): state 5 carries arena.lua's winner as it is (a record index
    // 0..7 or 9, the time end), never the team draw mapping; any other state is no result
    let g = match_server::GUNRACE_MODE;
    assert_eq!((match_server::mode_winner(g, 5, 3), match_server::mode_winner(g, 5, 2), match_server::mode_winner(g, 5, 9)), (3, 2, 9));
    assert_eq!((match_server::mode_winner(g, 4, 1), match_server::mode_winner(g, 3, 3)), (match_server::GUNRACE_NO_RESULT, match_server::GUNRACE_NO_RESULT));
    assert_eq!(match_server::mode_winner(1, 5, 3), 2, "other modes: pvp_winner");
}

/// Rejoin: a player whose client goes silent (a frozen game) is
/// dropped with his slot kept: no leave bit to anybody, one neutral input of his in the next DF, nothing more to his old
/// address, the match goes on. His client connects again (new address, same order id: the lobby's `82`), loads (DD, CB)
/// and gets the world host's DE of the running world, then every DF after it from the history (none with his own entries),
/// then the live DFs; his inputs reach the other player again.
#[test]
fn a_dropped_player_rejoins_with_the_hosts_de_and_the_dfs_after_it() {
    let fake = Fake::new(vec![Plan::normal()]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    assert!(net.client.1.entered && net.others[0].1.entered);
    // the game freezes: no CDs (its transport still acks)
    net.client.1.send_inputs = false;
    let frozen_at = newest(&net.client.1);
    net.run(6000);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.dropped, st.left, st.phase), (1, 0, "running"), "dropped, not left; the match goes on");
    assert_eq!(net.hub.take_dropped(), vec![(SEED, 100, match_server::LeaveReason::TimedOut)]);
    assert!(net.hub.take_left().is_empty());
    let old_dfs = net.client.1.dfs.len();
    let other = &net.others[0].1;
    assert!(other.dfs.iter().all(|d| d.leave_mask == 0), "no leave bit for the dropped slot");
    let neutral: Vec<u32> = other.dfs.iter().filter(|d| now_input(d, 0).is_some_and(|i| i == PlayerInput::default())).map(|d| d.time).collect();
    assert_eq!(neutral.len(), 1, "one neutral input of the dropped slot");
    assert!(neutral[0] > frozen_at);
    let drop_df = neutral[0];
    net.run(1000);
    assert_eq!(net.client.1.dfs.len(), old_dfs, "nothing more to the old connection");

    // the client comes back: a new connection with the same order id (the lobby's 82)
    let order = net.client.1.order_id;
    let mut c = Client::new(0, order, 9000, net.now);
    c.connect();
    net.client = ("127.0.0.1:5002".parse().unwrap(), c);
    assert!(net.until(5000, |n| n.client.1.de.is_some()), "the rejoin DE");
    net.client.1.rejoin_synced();
    let de_t = net.client.1.de.as_ref().unwrap().time;
    assert!(de_t > drop_df, "the DE lies after the drop's neutral input ({de_t} vs {drop_df})");
    assert!(net.until(3000, |n| n.hub.stats(SEED).unwrap().rejoined == 1), "the slot plays again");
    assert_eq!(net.hub.take_rejoined(), vec![(SEED, 100)]);
    assert_eq!(net.client.1.dd.as_ref().map(|d| d.local_player_index), Some(0));
    net.run(2000);
    let c = &net.client.1;
    assert_eq!(c.de_count, 1);
    // every DF from the DE on, in order, none twice, none with the receiver's own entries
    let times: Vec<u32> = c.dfs.iter().map(|d| d.time).collect();
    assert_eq!(times[0], de_t + TICK, "the history starts right after the DE");
    assert!(times.windows(2).all(|w| w[1] == w[0] + TICK), "no DF missing after the DE");
    assert!(c.dfs.iter().all(|d| d.input_mask & 1 == 0 && d.late.iter().all(|(_, p)| *p != 0)), "never its own entries");
    assert!(c.dfs.iter().all(|d| d.leave_mask == 0));
    // its inputs are relayed again
    let mine: Vec<u64> = c.sent.iter().map(|(_, a)| *a).collect();
    let seen: Vec<u64> = net.others[0].1.seen(0).iter().map(|(_, a)| *a).collect();
    assert!(mine.len() > 10 && mine[..10].iter().all(|a| seen.contains(a)), "the rejoined client's inputs reach the other player");
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.dropped, st.rejoined, st.left, st.rejoin_retries), (1, 1, 0, 0));
    assert_eq!(want_des(&fake), 1);
    // the menu leave is never a drop
    net.client.1.leave();
    net.run(500);
    assert_eq!(net.hub.take_left(), vec![(SEED, 100, match_server::LeaveReason::Menu)]);
    assert!(net.hub.take_dropped().is_empty());
}

/// Rejoin: a dropped player who does not come back is in the match result with his drop as the leave reason (the lobby
/// pays him nothing when the winner is known); the lobby can take him out for good (he declined); when every human has
/// been dropped for `rejoin_grace` the match ends (AllLeft).
#[test]
fn dropped_players_leave_at_the_end_by_decline_or_after_the_grace() {
    // the end while dropped
    let fake = Fake::new(vec![Plan::normal()]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    net.client.1.send_inputs = false;
    net.run(6000);
    assert_eq!(net.hub.stats(SEED).unwrap().dropped, 1);
    net.hub.end_match(SEED, net.now);
    let e = net.ended(3000);
    let p0 = e.players.iter().find(|p| p.slot == 0).unwrap();
    let p1 = e.players.iter().find(|p| p.slot == 1).unwrap();
    assert_eq!((p0.leave, p1.leave), (match_server::LeaveReason::TimedOut, match_server::LeaveReason::MatchEnded));

    // declined: a leave bit, his drop as the reason
    let fake = Fake::new(vec![Plan::normal()]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    net.client.1.send_inputs = false;
    net.run(6000);
    assert!(net.hub.allow_rejoin(SEED, 100, None), "a dropped slot may come back");
    assert!(!net.hub.allow_rejoin(SEED, 101, None), "a connected one is not dropped");
    assert!(net.hub.leave_dropped(SEED, 100));
    net.run(300);
    assert_eq!(net.hub.take_left(), vec![(SEED, 100, match_server::LeaveReason::TimedOut)]);
    assert!(net.others[0].1.dfs.iter().any(|d| d.leave_mask == 1), "the leave bit goes out");
    assert!(!net.hub.allow_rejoin(SEED, 100, None), "gone for good");

    // every human dropped: the grace, then AllLeft
    let fake = Fake::new(vec![Plan::normal()]);
    let mut net = pvp2_cfg(&fake, |_| {}, |m| m.rejoin_grace = Duration::from_secs(3));
    net.run(2000);
    net.client.1.send_inputs = false;
    net.others[0].1.send_inputs = false;
    net.run(6000);
    assert_eq!(net.hub.stats(SEED).unwrap().dropped, 2);
    assert_eq!(net.hub.stats(SEED).unwrap().phase, "running", "within the grace the match goes on");
    let e = net.ended(5000);
    assert_eq!(e.reason, EndReason::AllLeft);
    assert!(e.players.iter().all(|p| p.leave == match_server::LeaveReason::TimedOut));
}

/// PvP end screen with rejoin: a player dropped before the world's end stays dropped through the end screen (no rejoin
/// once the host is closed, no leave event racing the result): the result carries his drop as his leave (the lobby's zero
/// 97), the player still in the match gets the code-2 close. A client that goes away after the world's end (its own
/// victory-screen close) leaves no leave event and is never dropped (it finished the match).
#[test]
fn a_dropped_player_stays_dropped_through_the_pvp_end_screen() {
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((9000, 1)), final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    net.client.1.send_inputs = false;
    net.run(6000);
    assert_eq!(net.hub.stats(SEED).unwrap().dropped, 1);
    assert_eq!(net.hub.take_dropped(), vec![(SEED, 100, match_server::LeaveReason::TimedOut)]);
    let e = net.ended(10_000);
    assert_eq!((e.reason, e.winner), (EndReason::HostEnd, Some(1)));
    let p0 = e.players.iter().find(|p| p.slot == 0).unwrap();
    let p1 = e.players.iter().find(|p| p.slot == 1).unwrap();
    assert_eq!((p0.leave, p1.leave), (match_server::LeaveReason::TimedOut, match_server::LeaveReason::MatchEnded));
    assert!(!net.hub.allow_rejoin(SEED, 100, None), "no rejoin after the world's end");
    // the dropped client comes back during the end screen: no DE (the host is closed)
    let order = net.client.1.order_id;
    let mut c = Client::new(0, order, 9000, net.now);
    c.connect();
    net.client = ("127.0.0.1:5002".parse().unwrap(), c);
    net.run(2000);
    assert!(net.client.1.de.is_none(), "no rejoin DE from a closed host");
    assert!(net.hub.take_left().is_empty() && net.hub.take_dropped().is_empty(), "no leave or drop event after the result");
    assert_eq!(net.hub.stats(SEED).unwrap().rejoined, 0);
    // the lobby's close after its 97: code 2 to the player in the match
    assert!(net.hub.end_match_after_result(SEED, net.now));
    net.run(300);
    let codes: Vec<u8> = net.others[0].1.dfs.iter().filter(|d| d.leave_mask & 2 != 0).map(|d| d.code).collect();
    assert!(!codes.is_empty() && codes.iter().all(|&c| c == 2), "{codes:?}");
    assert!(net.hub.take_ended().is_empty(), "one result");

    // a client that goes silent after the world's end finished the match: no drop, no leave event
    let fake = Fake::new(vec![Plan { outcome_after_enter: Some((2000, 1)), final_: Some((5, 1)), ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    let _ = net.ended(10_000);
    net.client.1.send_inputs = false;
    net.run(6000);
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!(st.dropped, 0, "never dropped after the world's end");
    assert!(net.hub.take_left().is_empty() && net.hub.take_dropped().is_empty());
}

/// Without a world host there is no running world to give back: a silent player leaves.
#[test]
fn without_rejoin_a_silent_player_leaves() {
    let fake = Fake::new(vec![Plan::normal()]);
    let mut net = pvp2_cfg(&fake, |_| {}, |m| m.rejoin = false);
    net.run(2000);
    net.client.1.send_inputs = false;
    net.run(6000);
    assert_eq!(net.hub.take_left(), vec![(SEED, 100, match_server::LeaveReason::TimedOut)]);
    assert_eq!(net.hub.stats(SEED).unwrap().dropped, 0);
}

/// Rejoin with E0 on (the live test server's switches): a rejoining client gets no E0 before its DE (it has no world), and
/// afterwards only E0 above the newest DF at its DE's ack (its own hashes start after the load).
#[test]
fn a_rejoining_client_gets_no_e0_before_its_de() {
    let fake = Fake::new(vec![Plan { ticks: true, ..Plan::normal() }]);
    let mut net = pvp2(&fake, |_| {});
    net.run(2000);
    net.client.1.send_inputs = false;
    net.run(6000);
    assert_eq!(net.hub.stats(SEED).unwrap().dropped, 1);
    let order = net.client.1.order_id;
    let mut c = Client::new(0, order, 9000, net.now);
    c.connect();
    net.client = ("127.0.0.1:5002".parse().unwrap(), c);
    assert!(net.until(5000, |n| n.client.1.de.is_some()), "the rejoin DE");
    assert!(net.client.1.e0s.is_empty(), "no E0 before the DE");
    net.client.1.rejoin_synced();
    assert!(net.until(3000, |n| n.hub.stats(SEED).unwrap().rejoined == 1));
    let first_df = net.client.1.dfs.first().map(|d| d.time).unwrap();
    let floor = net.client.1.dfs.last().map(|d| d.time).unwrap();
    net.run(2000);
    let c = &net.client.1;
    assert!(!c.e0s.is_empty(), "E0 again after the rejoin");
    assert!(c.e0s.iter().all(|e| near(e.tick, floor) > floor), "only E0 above the newest DF at the ack ({first_df}..{floor})");
    let st = net.hub.stats(SEED).unwrap();
    assert_eq!((st.resync_sent, st.resync_tripped), (1, 0), "the rejoin DE, no breaker");
}
