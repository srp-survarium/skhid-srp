//! A scripted match client for the tests: the Client-role [`MatchTransport`] plus the handshake and the per-tick CD
//! stream a real client sends (as far as the client code shows it).
#![allow(dead_code)]

use match_server::match_msgs::*;
use match_server::mtransport::{Config, Event, MatchTransport};
use match_server::table::id;
use match_server::world::World;
use protocol::{Decode, Encode};
use std::time::Instant;
use udp_transport::Role;

pub const MAP: u32 = 3;
pub const MODE: u32 = 1;
pub const SEED: u32 = 0x1234_ABCD;
pub const CHECKSUM: u32 = 0xDEAD_C0DE;
pub const T0: u32 = 10_000;
pub const TICK: u32 = 50;

pub fn test_world() -> World {
    let mut item = T0.to_le_bytes().to_vec();
    item.extend((0..3000u32).map(|i| (i * 31 % 251) as u8));
    World::from_item(MAP, MODE, SEED, CHECKSUM, TICK as u8, 100, item).unwrap()
}

/// The start world the fake world host builds (protocol 4): its own time, bytes and checksum, so a test can tell it
/// from the stored [`test_world`].
pub const HOST_T0: u32 = 850;
pub const HOST_CHECKSUM: u32 = 0x255F_73E4;

pub fn host_world() -> World {
    let mut item = HOST_T0.to_le_bytes().to_vec();
    item.extend((0..2000u32).map(|i| (i * 17 % 241) as u8));
    World::from_item(MAP, MODE, SEED, HOST_CHECKSUM, TICK as u8, 100, item).unwrap()
}

pub fn test_options() -> Vec<u8> {
    options_with_id(SEED)
}

/// A 600-byte options blob with the online header the relay reads (`u16 send_period 100, u16 max_delay, u8 tick 50,
/// u8 n, u32 match_id`) and filler after it.
pub fn options_with_id(match_id: u32) -> Vec<u8> {
    let mut o: Vec<u8> = (0..600u32).map(|i| (i * 7 % 256) as u8).collect();
    o[..2].copy_from_slice(&100u16.to_le_bytes());
    o[2..4].copy_from_slice(&500u16.to_le_bytes());
    o[4] = TICK as u8;
    o[6..10].copy_from_slice(&match_id.to_le_bytes());
    o
}

pub struct Client {
    pub t: MatchTransport,
    pub order_id: u32,
    pub session: u32,
    pub dc: Option<Ports>,
    pub dd: Option<StaticInfo>,
    pub de: Option<WorldMsg>,
    pub de_count: u32,
    pub dfs: Vec<GameUpdate>,
    pub de_at: Option<Instant>,
    pub my_index: Option<u8>,
    pub entered: bool,
    pub closed: bool,
    pub next_cd: u32,
    /// (stamp, actions) of every CD input sent.
    pub sent: Vec<(u32, u64)>,
    /// stamps of the CDs sent before the own enter (no inputs)
    pub pre_sent: Vec<u32>,
    pub counter: u64,
    pub send_inputs: bool,
    /// Duplicate every CD datagram payload once (the real client resends plain messages).
    pub idx: u64,
    /// E0 hashes received
    pub e0s: Vec<Hash>,
    /// the world time of the first DE (the clock runs from it): T0 for the relay's start world, the DE's own time for a
    /// rejoining client ([`Client::rejoin_synced`])
    pub base: u32,
}

impl Client {
    pub fn new(idx: u64, order_id: u32, session: u32, now: Instant) -> Self {
        let mut cfg = Config::default();
        cfg.send_period = std::time::Duration::from_millis(100);
        Self {
            t: MatchTransport::with_config(Role::Client, cfg, now),
            order_id,
            session,
            dc: None,
            dd: None,
            de: None,
            de_count: 0,
            dfs: vec![],
            de_at: None,
            my_index: None,
            entered: false,
            closed: false,
            next_cd: 0,
            sent: vec![],
            pre_sent: vec![],
            counter: 0,
            send_inputs: true,
            idx,
            e0s: vec![],
            base: T0,
        }
    }

    pub fn connect(&mut self) {
        let c9 = Connect { order_id: self.order_id, session: self.session, debug: 0, reserved: 0 };
        self.t.send(id::C9_CONNECT, &c9.to_bytes().unwrap()).unwrap();
    }

    /// The menu "leave match": CE (the only sender of CE, PROVEN).
    pub fn leave(&mut self) {
        self.t.send(id::CE_LEAVE, &[]).unwrap();
    }

    /// The normal end of a match: the client sends no CE (`close_current_match(3)`, PROVEN); it closes the
    /// connection (here the transport's low-level disconnect; whether the real client sends one or goes silent is OPEN).
    pub fn end_normally(&mut self) {
        self.t.disconnect();
    }

    pub fn send_cc(&mut self) {
        let cc = QueryWorld { tick: 1, players: 0, global: 0xFF, reason: 2 };
        self.t.send(id::CC_QUERY_DE, &cc.to_bytes().unwrap()).unwrap();
    }

    /// A CC for the tick whose low byte is `tick_lo` (g0 / g1 differ) with the given reason bits.
    pub fn send_cc_at(&mut self, tick_lo: u8, reason: u8) {
        let cc = QueryWorld { tick: tick_lo, players: 0, global: 0xFF, reason };
        self.t.send(id::CC_QUERY_DE, &cc.to_bytes().unwrap()).unwrap();
    }

    pub fn send_cd(&mut self, cd: &ClientUpdate) {
        self.t.send(id::CD_CLIENT_UPDATE, &cd.to_bytes().unwrap()).unwrap();
    }

    pub fn clock(&self, now: Instant) -> Option<u32> {
        self.de_at.map(|a| self.base + now.saturating_duration_since(a).as_millis() as u32)
    }

    pub fn receive(&mut self, d: &[u8], now: Instant) {
        for ev in self.t.receive(d, now) {
            let Event::Message { id: mid, payload, .. } = ev else { continue };
            match mid {
                id::DC_PORTS => {
                    if self.dc.is_none() {
                        self.dc = Some(Ports::from_bytes(&payload).unwrap());
                        // the client probes the port range with CA
                        let ca = PortProbe { order_id: self.order_id };
                        self.t.send(id::CA_PORT_PROBE, &ca.to_bytes().unwrap()).unwrap();
                    }
                }
                id::DD_STATIC_INFO => {
                    let dd = StaticInfo::from_bytes(&payload).unwrap();
                    self.my_index = Some(dd.local_player_index);
                    self.dd = Some(dd);
                    // level "loaded": CB with the first profile input
                    let cb = Ready { flag: 0, profile: ProfileInput { head: [0, 0, 1, self.idx as u8], entries: vec![] } };
                    self.t.send(id::CB_READY, &cb.to_bytes().unwrap()).unwrap();
                }
                id::DE_WORLD => {
                    let de = WorldMsg::from_bytes(&payload).unwrap();
                    self.de_count += 1;
                    if self.de_at.is_none() {
                        self.de_at = Some(now);
                    }
                    self.de = Some(de);
                }
                id::DF_GAME_UPDATE => {
                    if self.de_at.is_none() {
                        continue; // synced flag not set: the real client drops DF before DE
                    }
                    let df = GameUpdate::from_bytes(&payload).unwrap();
                    if self.dfs.iter().any(|d| d.time == df.time) {
                        continue; // the client's repeat filter
                    }
                    if !self.entered && self.send_inputs {
                        // synced, not entered yet: the real client still answers the DF with a CD stamped at its time
                        let cd = ClientUpdate { time: df.time, ..Default::default() };
                        let _ = self.t.send(id::CD_CLIENT_UPDATE, &cd.to_bytes().unwrap());
                        self.pre_sent.push(df.time);
                    }
                    if let Some(me) = self.my_index {
                        if !self.entered && df.enter_mask & (1 << me) != 0 {
                            self.entered = true;
                            let c = self.clock(now).unwrap();
                            self.next_cd = self.base + (c - self.base) / TICK * TICK;
                        }
                        if df.leave_mask & (1 << me) != 0 {
                            self.closed = true; // close_current_match(3)
                        }
                    }
                    self.dfs.push(df);
                }
                id::E0_HASH => self.e0s.push(Hash::from_bytes(&payload).unwrap()),
                _ => {}
            }
        }
    }

    /// The CD stream: one per tick from the tick of the enter on, one distinct input each.
    pub fn drive(&mut self, now: Instant) {
        if !self.entered || self.closed || !self.send_inputs {
            return;
        }
        let c = self.clock(now).unwrap();
        while self.next_cd <= c {
            self.counter += 1;
            let actions = ((self.idx + 1) << 32) | self.counter;
            let cd = ClientUpdate {
                time: self.next_cd,
                now: vec![PlayerInput { rot: [self.counter as u16, 7], actions }],
                ..Default::default()
            };
            self.sent.push((self.next_cd, actions));
            self.send_cd(&cd);
            self.next_cd += TICK;
        }
    }

    /// A rejoining client after its DE (the running world, the local player already in its presence mask): its clock runs
    /// from the DE's time and its CDs start at once (the client's CD loop needs only the DE and the mask, 0x2001B13D5).
    pub fn rejoin_synced(&mut self) {
        let de = self.de.as_ref().expect("the rejoin DE");
        self.base = de.time;
        self.entered = true;
        self.next_cd = de.time + TICK;
    }

    pub fn poll(&mut self, now: Instant) -> Vec<Vec<u8>> {
        self.drive(now);
        self.t.poll(now)
    }

    /// Inputs of `player` seen in this client's DFs, in DF order: (stamp, actions). Late entries come first within a DF.
    pub fn seen(&self, player: u8) -> Vec<(u32, u64)> {
        let mut out = vec![];
        for df in &self.dfs {
            let mut k = 0;
            for (dt, p) in &df.late {
                if *p == player {
                    out.push((df.time - u32::from(*dt), df.inputs[k].actions));
                }
                k += 1;
            }
            for bit in 0..20u8 {
                if df.input_mask & (1 << bit) != 0 {
                    if bit == player {
                        out.push((df.time, df.inputs[k].actions));
                    }
                    k += 1;
                }
            }
        }
        out
    }
}
