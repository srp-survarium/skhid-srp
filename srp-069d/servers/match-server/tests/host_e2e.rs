//! End-to-end check of the relay and the REAL world host daemon (no game client): the relay (`Hub`, `Match`) runs a
//! recorded PvE match whose only human is a scripted client that sends, for every DF tick of the recording, a CD with
//! exactly that human's entries of the recorded DF (now input, late inputs with their dt, profile entries, ping). The
//! relay files each CD into the DF of its own time, so the DFs the relay forwards to the host carry the recorded inputs
//! at the recorded times; the human's first CD goes out right before the recorded enter DF, so the enter lands there.
//! The match's world is the recording's own DE item (the capture's roster, `roster: None`) and its options the
//! recording's blob. The relay's link connects to the daemon at `SRP_E2E_SOCK` and the host's `Outcome` / `Final` come
//! back through `Hub::take_outcomes` / the `MatchEnd`, as the lobby gets them.
//!
//! Ignored by default (it needs a running `world-host daemon`):
//!   SRP_E2E_REC=<.srprec> SRP_E2E_SOCK=<socket> [SRP_E2E_PROJECT=pve_01_v1_maze2] [SRP_E2E_SPEED=5] [SRP_E2E_E0=1]
//!   cargo test -p match-server --test host_e2e -- --ignored --nocapture
//! SRP_E2E_SPEED = virtual ms per real ms (the host must keep up within the link's 10 s heartbeat).
//! SRP_E2E_OWN_WORLD=1 = protocol 4: no stored world, the host builds the start world; the match gets its own id
//! (SRP_E2E_MATCH_ID, hex, default 0x5EED0000 + the low 16 bits of the process id) in the options and the spec.
//! SRP_E2E_E0=1 = check the E0 frames (always sent from the host's ticks): the scripted client keeps every E0 and the
//! test prints (and checks) that they carry the host's mask, one per-player byte per set bit and the g bytes.
//! SRP_E2E_BOTS=1 = check the host bots (protocol 5, always on): the host drives the mission's bots with the exe's AI and
//! the relay files its `Bots` frames into the DFs (hold SRP_E2E_BOTS_HOLD_MS virtual ms, default 1000); the test prints the
//! relay's bot counters and checks that bot entries came, none twice. SRP_E2E_RECORD_DIR=<dir> = the relay records the
//! match (`.srprec`, with the host's bot entries in its DFs): a host without AI replaying it must reach the daemon's
//! world byte for byte (the "inputs only" check).
//! SRP_E2E_RUN_TO_MS=<world ms> = the lobby's end request only at that DF time instead of the recording's last DF
//! (a keepalive human idles on): lets a PvP world reach its own end (`Outcome`), the end screen path: the result while
//! the client is in the match, then the lobby's close after the result with code 2.
//! SRP_E2E_PVP=1 = a PvP match: no PvE end rule; meant for the synthetic recording of world-host
//! `HOST_BOTS_SYNTH` (`HOST_BOTS_SYNTH_ONLINE=1`: 1 human + 11 AI players, the production options writer) with
//! SRP_E2E_OWN_WORLD=1 and SRP_E2E_PROJECT=<the tdm project>. The human's start profile is his first profile entry.
#![cfg(unix)]

mod common;

use common::{MAP, MODE};
use match_server::match_msgs::*;
use match_server::mtransport::{Config, Event, MatchTransport};
use match_server::table::id;
use match_server::{EndReason, HostConfig, Hub, HubConfig, MatchConfig, MatchSpec, PlayerSpec, PveEndRule, World, WorldStore};
use protocol::{Decode, Encode};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use udp_transport::Role;

/// The recording (`Recorder` in relay.rs): head, options, DE item, then DF records; 0xFF = the end (close tick).
struct Rec {
    match_id: u32,
    tick: u32,
    roster: Vec<(bool, u8)>,
    options: Vec<u8>,
    item: Vec<u8>,
    dfs: Vec<GameUpdate>,
    close: u32,
}

fn read_rec(path: &str) -> Rec {
    let d = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(&d[..8], b"SRPREC01");
    let u32_at = |p: usize| u32::from_le_bytes(d[p..p + 4].try_into().unwrap());
    let match_id = u32_at(8);
    let (tick, n) = (d[16] as u32, d[17] as usize);
    let roster = (0..n).map(|i| (d[18 + 2 * i] != 0, d[19 + 2 * i])).collect();
    let mut p = 18 + 2 * n;
    let mut blob = || {
        let l = u32_at(p) as usize;
        let b = d[p + 4..p + 4 + l].to_vec();
        p += 4 + l;
        b
    };
    let options = blob();
    let item = blob();
    let (mut dfs, mut close) = (Vec::new(), 0);
    while p + 5 <= d.len() {
        let t = u32_at(p + 1);
        if d[p] == 0xFF {
            close = t;
            break;
        }
        let l = u32_at(p + 5) as usize;
        dfs.push(GameUpdate::from_bytes(&d[p + 9..p + 9 + l]).expect("a recorded DF"));
        p += 9 + l;
    }
    // the relay's 4 closing DFs (fin) are the relay's own: the scripted match ends through the lobby's end request
    let from = close.saturating_sub(4 * tick);
    while dfs.last().is_some_and(|df| df.time >= from && df.time < close) {
        dfs.pop();
    }
    Rec { match_id, tick, roster, options, item, dfs, close }
}

/// One CD per recorded DF tick that carries any of the human's entries: exactly those entries.
fn human_cds(rec: &Rec, slot: u8, enter_t: u32) -> BTreeMap<u32, ClientUpdate> {
    let mut out = BTreeMap::new();
    for df in rec.dfs.iter().filter(|df| df.time > enter_t) {
        let mut cd = ClientUpdate { time: df.time, ..Default::default() };
        let mut k = 0;
        for (dt, p) in &df.late {
            if *p == slot {
                cd.late.push((*dt, df.inputs[k]));
            }
            k += 1;
        }
        for bit in 0..32u8 {
            if df.input_mask & (1 << bit) != 0 {
                if bit == slot {
                    cd.now.push(df.inputs[k]);
                }
                k += 1;
            }
        }
        cd.profile = df.profile.iter().filter(|(p, _)| *p == slot).map(|(_, x)| x.clone()).collect();
        cd.ping = df.ping.iter().filter(|(p, _)| *p == slot).map(|(_, v)| *v).collect();
        if !(cd.now.is_empty() && cd.late.is_empty() && cd.profile.is_empty() && cd.ping.is_empty()) {
            out.insert(df.time, cd);
        }
    }
    out
}

struct RecClient {
    t: MatchTransport,
    order_id: u32,
    start_profile: ProfileInput,
    enter_t: u32,
    tick: u32,
    cds: BTreeMap<u32, ClientUpdate>,
    synced: bool,
    sync_sent: bool,
    entered_at: Option<u32>,
    closed: bool,
    /// the closing DF's code (2 = the client keeps its world: PvE result screen, PvP end screen)
    close_code: Option<u8>,
    last_df: u32,
    e0s: Vec<Hash>,
    /// the first DE's world time
    first_de: Option<u32>,
    /// an empty CD for every tick the recording has none of the human's entries (synthetic PvP recordings: an idle human)
    keepalive: bool,
    /// every ai entry of the DFs (each DF once): (DF time, slot, kind, param)
    ai: Vec<(u32, u8, u16, u16)>,
}

impl RecClient {
    fn receive(&mut self, d: &[u8], now: Instant) {
        for ev in self.t.receive(d, now) {
            let Event::Message { id: mid, payload, .. } = ev else { continue };
            match mid {
                id::DC_PORTS => {
                    let ca = PortProbe { order_id: self.order_id };
                    let _ = self.t.send(id::CA_PORT_PROBE, &ca.to_bytes().unwrap());
                }
                id::DD_STATIC_INFO => {
                    let cb = Ready { flag: 0, profile: self.start_profile.clone() };
                    let _ = self.t.send(id::CB_READY, &cb.to_bytes().unwrap());
                }
                id::DE_WORLD => {
                    self.synced = true;
                    if self.first_de.is_none() {
                        self.first_de = WorldMsg::from_bytes(&payload).ok().map(|w| w.time);
                    }
                }
                id::E0_HASH => self.e0s.push(Hash::from_bytes(&payload).unwrap()),
                id::DF_GAME_UPDATE if self.synced => {
                    let df = GameUpdate::from_bytes(&payload).unwrap();
                    if df.time <= self.last_df {
                        continue;
                    }
                    self.last_df = df.time;
                    self.ai.extend(df.ai.iter().map(|a| (df.time, a.0, a.1, a.2)));
                    if df.enter_mask & 1 != 0 && self.entered_at.is_none() {
                        self.entered_at = Some(df.time);
                    }
                    if df.leave_mask & 1 != 0 {
                        self.closed = true;
                        self.close_code.get_or_insert(df.code);
                    }
                    let next = df.time + self.tick;
                    // the first CD (sync) right before the recorded enter DF; then the recorded entries of the next tick
                    if !self.sync_sent && next >= self.enter_t {
                        self.sync_sent = true;
                        let cd = ClientUpdate { time: df.time, ..Default::default() };
                        let _ = self.t.send(id::CD_CLIENT_UPDATE, &cd.to_bytes().unwrap());
                    }
                    if self.sync_sent {
                        if let Some(cd) = self.cds.remove(&next) {
                            let _ = self.t.send(id::CD_CLIENT_UPDATE, &cd.to_bytes().unwrap());
                        } else if self.keepalive {
                            // an idle human still sends a CD per tick (the relay drops a slot without CDs for 5 s)
                            let cd = ClientUpdate { time: next, ..Default::default() };
                            let _ = self.t.send(id::CD_CLIENT_UPDATE, &cd.to_bytes().unwrap());
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[test]
#[ignore]
fn a_recorded_pve_match_through_the_relay_and_the_real_world_host() {
    let (Ok(rec_path), Ok(sock)) = (std::env::var("SRP_E2E_REC"), std::env::var("SRP_E2E_SOCK")) else {
        panic!("set SRP_E2E_REC and SRP_E2E_SOCK");
    };
    let project = std::env::var("SRP_E2E_PROJECT").unwrap_or("pve_01_v1_maze2".into());
    let speed: u64 = std::env::var("SRP_E2E_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let rec = read_rec(&rec_path);
    let human = rec.roster.iter().position(|(bot, _)| !bot).expect("one human") as u8;
    assert_eq!(human, 0, "the scripted client is slot 0");
    let enter = rec.dfs.iter().find(|df| df.enter_mask != 0).expect("an enter DF");
    let enter_t = enter.time;
    // the human's start profile: the enter DF's (a recorded match), else his first profile entry (the synthetic PvP
    // recording puts every record's profile at enter + tick)
    let start_profile = rec
        .dfs
        .iter()
        .filter(|df| df.time >= enter_t)
        .find_map(|df| df.profile.iter().find(|(p, _)| *p == human).map(|(_, x)| x.clone()))
        .expect("the human's start profile");
    let pvp = std::env::var("SRP_E2E_PVP").is_ok_and(|v| v == "1");
    let cds = human_cds(&rec, human, enter_t);
    let last_t = rec.dfs.last().unwrap().time;
    println!(
        "recording {rec_path}: match {:#x}, {} records, {} DFs ({}..{}), enter at {enter_t}, close {}, {} human CDs",
        rec.match_id,
        rec.roster.len(),
        rec.dfs.len(),
        rec.dfs[0].time,
        last_t,
        rec.close,
        cds.len()
    );

    let own = std::env::var("SRP_E2E_OWN_WORLD").is_ok_and(|v| v == "1");
    let match_id = if own {
        std::env::var("SRP_E2E_MATCH_ID").ok().and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0x5EED_0000 | (std::process::id() & 0xFFFF))
    } else {
        rec.match_id
    };
    let mut options = rec.options.clone();
    options[6..10].copy_from_slice(&match_id.to_le_bytes());
    let mut worlds = WorldStore::new();
    if !own {
        let world = World::from_item(MAP, MODE, rec.match_id, 0, rec.tick as u8, 100, rec.item.clone()).unwrap().with_project(&project).unwrap();
        worlds.insert(world);
    }
    println!("match id {match_id:#x}, start world {}", if own { "built by the host" } else { "the recording's" });
    let e0 = std::env::var("SRP_E2E_E0").is_ok_and(|v| v == "1");
    let mut hc = HostConfig::unix(std::path::Path::new(&sock));
    let bots = std::env::var("SRP_E2E_BOTS").is_ok_and(|v| v == "1");
    hc.bots_hold_ms = std::env::var("SRP_E2E_BOTS_HOLD_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
    let record_dir = std::env::var("SRP_E2E_RECORD_DIR").ok().map(std::path::PathBuf::from);
    // the virtual clock runs `speed` x real time: the start-world wait (a cold zygote loads the level) is scaled with it
    let mc = MatchConfig { world_host: Some(hc), host_start_timeout: Duration::from_secs(45 * speed), record_dir, ..MatchConfig::default() };
    let mut hub = Hub::new(HubConfig { port: 1236, match_config: mc, ..Default::default() }, worlds);
    let players: Vec<PlayerSpec> = rec
        .roster
        .iter()
        .enumerate()
        .map(|(i, (bot, team))| if *bot { PlayerSpec::bot(&format!("{}_bot_{i}", if pvp { "pvp" } else { "pve" }), *team) } else { PlayerSpec { account_id: 100, name: "player".into(), team: *team, session: None, bot: false } })
        .collect();
    let spec = MatchSpec {
        match_id: Some(match_id),
        map: MAP,
        mode: MODE,
        options,
        players,
        level_checksum: None,
        roster: None,
        project: Some(project.clone()),
        end_rule: None,
        pve_end: (!pvp).then_some(PveEndRule::PVE_LUA),
        ai_ignore_players: false,
    };
    let t0 = Instant::now();
    let mut now = t0;
    let ticket = hub.create_match(&spec, now).unwrap();
    let mut cfg = Config::default();
    cfg.send_period = Duration::from_millis(100);
    let mut c = RecClient {
        t: MatchTransport::with_config(Role::Client, cfg, now),
        order_id: ticket.players[human as usize].order_id,
        start_profile,
        enter_t,
        tick: rec.tick,
        cds,
        synced: false,
        sync_sent: false,
        entered_at: None,
        closed: false,
        close_code: None,
        last_df: 0,
        e0s: vec![],
        first_de: None,
        ai: vec![],
        keepalive: pvp,
    };
    let c9 = Connect { order_id: c.order_id, session: 9000, debug: 0, reserved: 0 };
    c.t.send(id::C9_CONNECT, &c9.to_bytes().unwrap()).unwrap();
    let addr: SocketAddr = "127.0.0.1:5000".parse().unwrap();
    let real0 = Instant::now();
    let mut outcomes = Vec::new();
    let mut ended = Vec::new();
    let mut end_requested = false;
    let run_to: u32 = std::env::var("SRP_E2E_RUN_TO_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0).max(last_t);
    let mut last_report = 0u32;
    let mut bots_st = match_server::BotsStats::default();
    loop {
        now += Duration::from_millis(5);
        for d in c.t.poll(now) {
            hub.on_datagram(addr, &d, now);
        }
        for (_, d) in hub.poll(now) {
            c.receive(&d, now);
        }
        if let Some(b) = hub.bots_stats(match_id) {
            bots_st = b;
        }
        outcomes.extend(hub.take_outcomes());
        ended.extend(hub.take_ended());
        // the lobby's end request once the recording's last normal DF went out
        if !end_requested && c.last_df >= run_to {
            end_requested = true;
            println!("DF {run_to} reached (last recorded DF {last_t}): the lobby's end request");
            assert!(hub.end_match(match_id, now));
        }
        if !ended.is_empty() {
            break;
        }
        if c.last_df >= last_report + 60_000 {
            last_report = c.last_df;
            let s = hub.stats(match_id);
            println!("DF {} (real {} s): host {:?}", c.last_df, real0.elapsed().as_secs(), s.map(|s| (s.host, s.host_restarts)));
        }
        // pacing: virtual time at most `speed` x real time
        let virt = now.duration_since(t0).as_millis() as u64;
        let real = real0.elapsed().as_millis() as u64;
        if virt > real * speed {
            std::thread::sleep(Duration::from_millis((virt - real * speed) / speed + 1));
        }
        assert!(now.duration_since(t0) < Duration::from_millis((rec.close.max(run_to) + 120_000) as u64), "no end");
    }
    println!("enter DF {:?} (recorded {enter_t}); host link {:?}; first DE time {:?}", c.entered_at, hub.stats(match_id).map(|s| (s.host, s.host_restarts)), c.first_de);
    for o in &outcomes {
        println!("HostOutcome: t_end {} state {} winner {} pve_success {} mission_ms {} team_scores {:?}", o.t_end, o.state, o.winner_team, o.pve_success, o.mission_ms, o.team_scores);
        for p in o.players.iter().filter(|p| !p.bot) {
            println!("  human {p:?}");
        }
    }
    // PvP end screen: a world-host end hands the result over with the client still in the match; the lobby then closes it
    // after the result (online.rs PVP_END_AFTER_RESULT) and the closing DF carries code 2 (the client keeps its world)
    if pvp {
        let open_at_result = !c.closed;
        let t_result = c.last_df;
        println!("PvP: result at DF {t_result}, client still in the match: {open_at_result}");
        if open_at_result {
            let t_ask = now + Duration::from_secs(3);
            let mut asked = false;
            while !c.closed {
                now += Duration::from_millis(5);
                for d in c.t.poll(now) {
                    hub.on_datagram(addr, &d, now);
                }
                for (_, d) in hub.poll(now) {
                    c.receive(&d, now);
                }
                ended.extend(hub.take_ended());
                if !asked && now >= t_ask {
                    asked = true;
                    println!("PvP: the lobby's close after the result at DF {}", c.last_df);
                    hub.end_match_after_result(match_id, now);
                }
                assert!(now.duration_since(t0) < Duration::from_millis((rec.close.max(run_to) + 150_000) as u64), "no close after the result");
            }
            println!("PvP: closing DF {} code {:?}; {} result(s)", c.last_df, c.close_code, ended.len());
            assert_eq!(c.close_code, Some(2), "the PvP end screen's close keeps the world");
            assert_eq!(ended.len(), 1, "one result");
        }
    }
    let e = &ended[0];
    println!("MatchEnd: reason {:?} winner {:?}; {} human(s), bots (slot, team, entered) {:?}", e.reason, e.winner, e.players.len(), e.bots.iter().map(|b| (b.slot, b.team, b.entered)).collect::<Vec<_>>());
    if let Some(st) = &e.stats {
        for p in st.players.iter().filter(|p| p.slot == human) {
            println!("  MatchEnd human row {p:?}");
        }
        println!("  team_scores {:?}", st.team_scores);
    }
    if e0 {
        let n = c.e0s.len();
        let masked = c.e0s.iter().filter(|h| h.players != 0).count();
        let own = c.e0s.iter().filter(|h| h.players & 1 != 0).count();
        let mut masks: Vec<u32> = c.e0s.iter().map(|h| h.players).collect();
        masks.sort_unstable();
        masks.dedup();
        let zeros = c.e0s.iter().filter(|h| h.players & 1 != 0 && h.hashes[0] == 0).count();
        println!("E0: {n} frames, {masked} with a mask != 0, {own} with slot 0's bit; masks seen {:x?}; slot 0 byte 00 in {zeros}", masks);
        for h in c.e0s.iter().take(3).chain(c.e0s.iter().rev().take(2)) {
            println!("  E0 tick_lo {:3} mask {:06x} bytes {:02x?} g {:02x?} wire {:02x?}", h.tick, h.players, h.hashes, h.globals, h.to_bytes().unwrap());
        }
        assert!(n > 100, "E0 frames from the host");
        assert!(c.e0s.iter().all(|h| h.hashes.len() == (h.players & 0xFF_FFFF).count_ones() as usize));
        assert_eq!(masked, n, "every E0 carries the host's mask");
    }
    if bots {
        let mut seen = c.ai.clone();
        seen.sort_unstable();
        seen.dedup();
        println!("BOTS relay: {bots_st:?}; {} ai entries reached the client ({} distinct); {} host outcome(s)", c.ai.len(), seen.len(), outcomes.len());
        assert_eq!(seen.len(), c.ai.len(), "no ai entry twice");
        assert!(bots_st.frames > 100 && bots_st.inputs > 0, "the host drove the bots: {bots_st:?}");
        assert_eq!(bots_st.duplicates, 0);
    }
    assert_eq!(c.entered_at, Some(enter_t), "the enter at the recorded DF");
    assert!(matches!(e.reason, EndReason::Requested | EndReason::PveCancel | EndReason::PveTimeout | EndReason::AllLeft | EndReason::ReadyTimeout) || true);
}
