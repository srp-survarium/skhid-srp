//! The lobby-facing API and the datagram router (sans IO): many matches behind one UDP port.
//!
//! Lobby side:
//! * [`Hub::new_match_id`]: a fresh match id (the world's seed) for the `match_options` blob; every match has its own,
//!   so any number of matches of one map run at once. [`Hub::can_create`]: whether a
//!   start world can be had (a world host builds it; without one, a stored world).
//! * [`Hub::create_match`]: players / accounts / map / mode / options -> [`MatchTicket`] with the address and port and
//!   one `order_id` per player. The lobby sends each player `0x82 {host, port, match_id, side, order_id}`; the player's
//!   client then sends C9 `{order_id, session}` to that address (PROVEN).
//! * [`Hub::set_winner`], [`Hub::end_match`]: the lobby feeds the winner it learned from the clients' `53` events and
//!   may end a match.
//! * [`Hub::take_ended`]: the match-end callback payloads ([`MatchEnd`]).
//!
//! Transport side: [`Hub::on_datagram`] / [`Hub::poll`]. An unknown source address gets a scratch transport; when its
//! datagrams carry a C9 with a known `order_id` (and matching session, if the lobby gave one) the transport is handed to
//! that player's slot and the address is bound to it.

use crate::match_msgs::Connect;
use crate::mtransport::{Event, MatchTransport};
use crate::relay::{options_header, CreateError, EndReason, Match, MatchConfig, MatchEnd, MatchSpec, MatchStats};
use crate::table::id;
use crate::world::WorldStore;
use protocol::Decode;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use udp_transport::Role;

#[derive(Clone, Debug)]
pub struct HubConfig {
    /// Address the lobby gives to clients (host name or IP).
    pub public_host: String,
    /// The UDP port the match server listens on (put in DC and the ticket); set by [`crate::MatchServer`] after the bind.
    pub port: u16,
    pub match_config: MatchConfig,
    /// A scratch transport of an unknown address lives this long without a valid C9.
    pub unknown_ttl: Duration,
    pub max_unknown: usize,
    /// Every instant the hub is given is mapped onto a match clock that runs at the wall clock's rate
    /// ([`crate::clock::Discipline`]): a VM whose monotonic clock runs slow (WSL: 6-7 %) would otherwise drag
    /// the clients' worlds into slow motion. Off by default (tests drive the hub with virtual instants); the server turns
    /// it on.
    pub discipline_clock: bool,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            public_host: "127.0.0.1".into(),
            port: 0,
            match_config: MatchConfig::default(),
            unknown_ttl: Duration::from_secs(10),
            max_unknown: 256,
            discipline_clock: false,
        }
    }
}

/// What the lobby needs to put the player into the match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerTicket {
    pub slot: u8,
    pub account_id: u64,
    /// The per-player token: goes to the client as the last u32 of `0x82` and comes back in C9. Random, unique.
    pub order_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchTicket {
    pub match_id: u32,
    pub host: String,
    pub port: u16,
    pub players: Vec<PlayerTicket>,
}

pub struct Hub {
    cfg: HubConfig,
    /// given start worlds for a hub without a world host (tests, a host-less setup); the host builds them otherwise
    worlds: WorldStore,
    matches: HashMap<u32, Match>,
    /// every match id this hub handed out or created (a lobby keys per-match rows by it: never twice)
    used_ids: std::collections::HashSet<u32>,
    orders: HashMap<u32, (u32, u8)>,
    addrs: HashMap<SocketAddr, (u32, u8)>,
    unknown: HashMap<SocketAddr, (MatchTransport, Instant)>,
    /// the match clock's discipline ([`HubConfig::discipline_clock`])
    clock: Option<crate::clock::Discipline>,
    ended: Vec<MatchEnd>,
}

fn random_u32() -> u32 {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u8(0x5A);
    h.finish() as u32
}

impl Hub {
    pub fn new(cfg: HubConfig, worlds: WorldStore) -> Self {
        Self {
            clock: cfg.discipline_clock.then(crate::clock::Discipline::new),
            cfg,
            worlds,
            matches: HashMap::new(),
            used_ids: Default::default(),
            orders: HashMap::new(),
            addrs: HashMap::new(),
            unknown: HashMap::new(),
            ended: Vec::new(),
        }
    }

    pub fn set_port(&mut self, port: u16) {
        self.cfg.port = port;
    }

    pub fn config(&self) -> &HubConfig {
        &self.cfg
    }

    pub fn worlds_mut(&mut self) -> &mut WorldStore {
        &mut self.worlds
    }

    /// A fresh match id for a new match's options: random in 0x10000000..0x7FFFFFFF (positive as an i32 where a lobby
    /// stores it), never one this hub handed out before or one that is running.
    pub fn new_match_id(&mut self) -> u32 {
        loop {
            let id = 0x1000_0000 | (random_u32() & 0x6FFF_FFFF);
            if id < 0x8000_0000 && !self.matches.contains_key(&id) && self.used_ids.insert(id) {
                return id;
            }
        }
    }

    /// Can a match of (map, mode) / this project get a start world: a stored one, or a world host builds every project the
    /// lobby names (a project the host does not serve ends the match with `NoWorld`).
    pub fn can_create(&self, map: u32, mode: u32, project: Option<&str>) -> bool {
        if self.cfg.match_config.world_host.is_some() {
            return true;
        }
        match project {
            Some(p) => self.worlds.get_project(map, mode, p).is_some(),
            None => self.worlds.get(map, mode).is_some(),
        }
    }

    /// Does a match of (map, mode) / this project need the world host to build its start world (no stored world, a host
    /// configured)?
    pub fn host_builds(&self, map: u32, mode: u32, project: Option<&str>) -> bool {
        let stored = match project {
            Some(p) => self.worlds.get_project(map, mode, p).is_some(),
            None => self.worlds.get(map, mode).is_some(),
        };
        !stored && self.cfg.match_config.world_host.is_some()
    }

    /// The match clock for a monotonic instant read now ([`HubConfig::discipline_clock`]); the instant itself when off.
    fn clock(&mut self, now: Instant) -> Instant {
        match self.clock.as_mut() {
            Some(c) => c.map(now, std::time::SystemTime::now()),
            None => now,
        }
    }

    /// The wall clock's rate against the monotonic clock and the rate applied to the match clock (None when off).
    pub fn clock_rate(&self) -> Option<(Option<f64>, f64)> {
        self.clock.as_ref().map(|c| (c.estimate(), c.rate()))
    }

    pub fn create_match(&mut self, spec: &MatchSpec, now: Instant) -> Result<MatchTicket, CreateError> {
        let now = self.clock(now);
        let (opt_id, _, _) = options_header(&spec.options).ok_or(CreateError::Options)?;
        let match_id = spec.match_id.unwrap_or(opt_id);
        if match_id != opt_id {
            return Err(CreateError::OptionsMatchId(match_id, opt_id));
        }
        if self.matches.contains_key(&match_id) {
            return Err(CreateError::IdInUse(match_id));
        }
        // the start world: a stored one when the hub has it (tests, a host-less setup; under this match's id), else the
        // world host builds it (protocol 4; production srp stores none)
        let stored = match &spec.project {
            Some(p) => self.worlds.get_project(spec.map, spec.mode, p),
            None => self.worlds.get(spec.map, spec.mode),
        };
        let given = match stored {
            Some(w) => Some(w),
            None if self.cfg.match_config.world_host.is_some() => None,
            None => {
                return Err(match &spec.project {
                    Some(p) => CreateError::NoProjectWorld(p.clone()),
                    None => CreateError::NoWorld(spec.map, spec.mode),
                })
            }
        };
        // one ticket per human; a server-owned bot slot has none (order 0 is never handed out)
        let mut order_ids: Vec<u32> = Vec::new();
        for p in &spec.players {
            if p.bot {
                order_ids.push(0);
                continue;
            }
            loop {
                let o = random_u32();
                if o != 0 && o != u32::MAX && !self.orders.contains_key(&o) && !order_ids.contains(&o) {
                    order_ids.push(o);
                    break;
                }
            }
        }
        let given = match given {
            Some(world) => {
                let mut w = match &spec.roster {
                    Some(r) => {
                        let w = world.with_roster(r).map_err(|e| CreateError::World(e.to_string()))?;
                        log::info!("match {match_id:#x}: world G1 with {} roster objects ({} bytes; the stored world had {})", w.roster_objects, w.roster_bytes, world.roster_objects);
                        w
                    }
                    None => (*world).clone(),
                };
                w.match_id = match_id;
                Some(std::sync::Arc::new(w))
            }
            None => None,
        };
        let m = Match::new(spec, given, match_id, &order_ids, self.cfg.match_config.clone(), now)?;
        self.used_ids.insert(match_id);
        let players = spec
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.bot)
            .map(|(i, p)| PlayerTicket { slot: i as u8, account_id: p.account_id, order_id: order_ids[i] })
            .collect();
        for (i, o) in order_ids.iter().enumerate() {
            if !spec.players[i].bot {
                self.orders.insert(*o, (match_id, i as u8));
            }
        }
        self.matches.insert(match_id, m);
        Ok(MatchTicket { match_id, host: self.cfg.public_host.clone(), port: self.cfg.port, players })
    }

    pub fn set_winner(&mut self, match_id: u32, team: u8) -> bool {
        self.matches.get_mut(&match_id).map(|m| m.set_winner(team)).is_some()
    }

    /// Real per-player statistics of a match (kills, deaths, score) from a source that observed the world.
    pub fn set_stats(&mut self, match_id: u32, stats: crate::relay::FinalStats) -> bool {
        self.matches.get_mut(&match_id).map(|m| m.set_stats(stats)).is_some()
    }

    /// End a running match now; its result appears in [`take_ended`](Self::take_ended).
    pub fn end_match(&mut self, match_id: u32, now: Instant) -> bool {
        let now = self.clock(now);
        match self.matches.get_mut(&match_id) {
            Some(m) => {
                m.end(EndReason::Requested, now);
                true
            }
            None => false,
        }
    }

    /// End a PvE match whose result (`99`, `83 45 0B`) the lobby sent: the mission script ended the clients' worlds, so
    /// the closing DFs carry code 2 (the client keeps its world and HUD; the result screen gets the pointer,
    /// [`Match::end_after_result`]). Its result appears in [`take_ended`](Self::take_ended).
    /// The match still runs its clients although its result went out (the PvP end screen): the lobby keeps their `85 00`
    /// until [`has_match`](Self::has_match) is false ([`Match::holds_clients`]).
    pub fn holds_clients(&self, match_id: u32) -> bool {
        self.matches.get(&match_id).is_some_and(|m| m.holds_clients())
    }

    /// The match is still here (running, or ended and draining its connections).
    pub fn has_match(&self, match_id: u32) -> bool {
        self.matches.contains_key(&match_id)
    }

    pub fn end_match_after_result(&mut self, match_id: u32, now: Instant) -> bool {
        let now = self.clock(now);
        match self.matches.get_mut(&match_id) {
            Some(m) => {
                m.end_after_result(now);
                true
            }
            None => false,
        }
    }

    pub fn stats(&self, match_id: u32) -> Option<MatchStats> {
        self.matches.get(&match_id).map(|m| m.stats())
    }

    /// The host bots' counters of a running match (protocol 5; all zero when the host does not drive the bots).
    pub fn bots_stats(&self, match_id: u32) -> Option<crate::relay::BotsStats> {
        self.matches.get(&match_id).map(|m| m.bots_stats())
    }

    /// Match time of a running match since its enter DF (the mission clock), None before the enter or when unknown.
    pub fn played(&self, match_id: u32) -> Option<Duration> {
        self.matches.get(&match_id).and_then(|m| m.played())
    }

    pub fn running_matches(&self) -> usize {
        self.matches.len()
    }

    /// Results of matches that ended since the last call (the match-end callback).
    pub fn take_ended(&mut self) -> Vec<MatchEnd> {
        std::mem::take(&mut self.ended)
    }

    /// The world host's outcomes since the last call (the end of the mission as the world
    /// saw it, while the match still runs).
    pub fn take_outcomes(&mut self) -> Vec<crate::relay::HostOutcome> {
        self.matches.values_mut().flat_map(|m| m.take_outcomes()).collect()
    }

    /// Does this match have a world host in use (its outcome can still come)?
    pub fn host_active(&self, match_id: u32) -> bool {
        self.matches.get(&match_id).is_some_and(|m| m.host_active())
    }

    /// Players who left a running match since the last call: (match id, account, why).
    /// Rejoin: humans dropped with their slot kept, since the last call: (match, account, how).
    pub fn take_dropped(&mut self) -> Vec<(u32, u64, crate::relay::LeaveReason)> {
        let mut out = Vec::new();
        for (id, m) in self.matches.iter_mut() {
            for (account, why) in m.take_dropped() {
                out.push((*id, account, why));
            }
        }
        out
    }

    /// Rejoin: dropped humans whose client is back in the match, since the last call: (match, account).
    pub fn take_rejoined(&mut self) -> Vec<(u32, u64)> {
        let mut out = Vec::new();
        for (id, m) in self.matches.iter_mut() {
            for account in m.take_rejoined() {
                out.push((*id, account));
            }
        }
        out
    }

    /// Rejoin: may the account's dropped slot still come back (the lobby then sends its `82`)? `session` = the login
    /// session after a new sign-in (C9 carries it).
    pub fn allow_rejoin(&mut self, match_id: u32, account: u64, session: Option<u32>) -> bool {
        self.matches.get_mut(&match_id).is_some_and(|m| m.allow_rejoin(account, session))
    }

    /// Rejoin declined: the account's dropped slot leaves for good.
    pub fn leave_dropped(&mut self, match_id: u32, account: u64) -> bool {
        self.matches.get_mut(&match_id).is_some_and(|m| m.leave_dropped(account))
    }

    pub fn take_left(&mut self) -> Vec<(u32, u64, crate::relay::LeaveReason)> {
        let mut out = Vec::new();
        for (id, m) in self.matches.iter_mut() {
            for (account, why) in m.take_left() {
                out.push((*id, account, why));
            }
        }
        out
    }

    /// One datagram from `from`.
    pub fn on_datagram(&mut self, from: SocketAddr, d: &[u8], now: Instant) {
        let now = self.clock(now);
        if let Some((mid, slot)) = self.addrs.get(&from).copied() {
            match self.matches.get_mut(&mid) {
                // a dropped slot (rejoin) has no connection: its old address may bring its new C9 (below)
                Some(m) if !m.has_conn(slot) => {
                    self.addrs.remove(&from);
                }
                Some(m) => {
                    m.on_datagram(slot, d, now);
                    return;
                }
                None => return,
            }
        }
        if !self.unknown.contains_key(&from) {
            if self.unknown.len() >= self.cfg.max_unknown {
                return;
            }
            let mut tc = self.cfg.match_config.transport;
            tc.send_period = Duration::from_millis(100);
            self.unknown.insert(from, (MatchTransport::with_config(Role::Server, tc, now), now));
        }
        let (t, _) = self.unknown.get_mut(&from).expect("just inserted");
        let events = t.receive(d, now);
        let c9 = events.iter().find_map(|e| match e {
            Event::Message { id: id::C9_CONNECT, payload, .. } => Connect::from_bytes(payload).ok(),
            _ => None,
        });
        let Some(c9) = c9 else { return };
        let Some(&(mid, slot)) = self.orders.get(&c9.order_id) else {
            log::warn!("C9 from {from} with an unknown order id");
            self.unknown.remove(&from);
            return;
        };
        let (t, _) = self.unknown.remove(&from).expect("present");
        let Some(m) = self.matches.get_mut(&mid) else { return };
        if m.attach(slot, from, t, &c9, now) {
            // a reconnect from a new address replaces the old binding
            self.addrs.retain(|_, v| *v != (mid, slot));
            self.addrs.insert(from, (mid, slot));
            m.send_ports(slot, self.cfg.port);
            m.on_events(slot, events, now);
        }
    }

    /// Run all timers; returns the datagrams to send.
    pub fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        let now = self.clock(now);
        let mut out = Vec::new();
        let mut done = Vec::new();
        for (mid, m) in self.matches.iter_mut() {
            out.extend(m.poll(now));
            if let Some(r) = m.take_result() {
                self.ended.push(r);
            }
            if m.is_done(now) {
                done.push(*mid);
            }
        }
        for mid in done {
            if let Some(m) = self.matches.remove(&mid) {
                for o in m.order_ids() {
                    self.orders.remove(&o);
                }
                let _ = m.account_ids();
            }
            self.addrs.retain(|_, v| v.0 != mid);
        }
        let ttl = self.cfg.unknown_ttl;
        self.unknown.retain(|_, (_, since)| now.saturating_duration_since(*since) < ttl);
        out
    }
}
