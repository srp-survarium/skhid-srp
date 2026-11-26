//! The sans-IO lobby: peers by UDP address, datagrams in, datagrams out.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use udp_transport::Event;

use crate::dispatch::{handle, Ctx};
use crate::login;
use crate::peer::{Peer, Phase, Registry};
use crate::Lobby;

pub struct Server {
    pub lobby: Lobby,
    pub peers: HashMap<SocketAddr, Peer>,
    pub registry: Registry,
    /// When the sessions of the connected accounts were last marked used (sessions.rs).
    last_touch: Option<Instant>,
}

/// How often the sessions of connected accounts are marked used: a connection longer than the session time to live
/// keeps its session, so the client's reconnect after a drop or a server restart is still accepted.
const SESSION_TOUCH_EVERY: Duration = Duration::from_secs(60);

/// A connection without a 2E this long after its first datagram is closed (`FD`, login::refuse). The client sends the
/// 2E as soon as its lobby connection starts (seen with the real client: within 2 ms).
const SIGN_IN_DEADLINE: Duration = Duration::from_secs(15);

impl Server {
    pub fn new(lobby: Lobby) -> Server {
        Server { lobby, peers: HashMap::new(), registry: Registry::default(), last_touch: None }
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Process one datagram from `addr`.
    pub fn receive(&mut self, addr: SocketAddr, datagram: &[u8], now: Instant) {
        if !self.peers.contains_key(&addr) {
            if self.peers.len() >= self.lobby.cfg.max_peers {
                log::warn!("{addr}: too many peers, datagram dropped");
                return;
            }
            self.peers.insert(addr, Peer::new(addr, now));
        }
        let Server { lobby, peers, registry, .. } = &mut *self;
        let mut cx = Ctx { lobby, registry, now, fx: vec![], kick: vec![] };
        let peer = peers.get_mut(&addr).unwrap();
        // a connection that is over (its FB / FD not sent yet: poll) only lets the transport answer a repeated FA
        let over = peer.transport.is_gone(now);
        let refused = peer.phase == Phase::Refused;
        for ev in peer.transport.receive(datagram, now) {
            match ev {
                Event::Message { id, payload, .. } if !over => handle(&mut cx, peer, id, &payload),
                // dropped by the next poll, after the FB that confirms the client's FA went out
                Event::Message { .. } | Event::Disconnected => {}
            }
        }
        // a refused peer that still talks gets another FD (the first may have been lost)
        if refused && !over && !peer.transport.is_gone(now) {
            peer.transport.instant_disconnect();
        }
        // the loading messages go out once the client acked the 80
        if peer.phase == Phase::AwaitAck && peer.transport.unacked() == 0 {
            login::push_login(&mut cx, peer);
        }
        // level-up / rank-up notices of the changes this datagram made (after the handler's own pushes)
        crate::notices::flush(&mut cx);
        let (fx, kick) = (std::mem::take(&mut cx.fx), std::mem::take(&mut cx.kick));
        for c in fx {
            if let Some(a) = self.registry.addr_of(c.to) {
                if let Some(p) = self.peers.get_mut(&a) {
                    p.push(&c.msg);
                }
            }
        }
        for a in kick {
            if a != addr {
                self.drop_peer(a);
            }
        }
    }

    fn drop_peer(&mut self, addr: SocketAddr) {
        if self.peers.remove(&addr).is_some() {
            log::info!("{addr}: peer dropped");
        }
        let account = self.registry.account_at(addr);
        // the session's time to live runs from the end of the connection (the client reconnects with it)
        if let Some(s) = account.and_then(|id| self.registry.session_of(id)) {
            self.lobby.sessions.touch(&[s], crate::sessions::unix_now());
        }
        crate::squads::on_disconnect(&self.lobby, &mut self.registry, &mut self.peers, addr);
        self.registry.remove_addr(addr);
        // the friends see the account offline (a replaced connection keeps the account online: no entry above)
        if let Some(id) = account {
            self.lobby.sessions.mark_offline(id);
            let mut cx = Ctx { lobby: &self.lobby, registry: &mut self.registry, now: Instant::now(), fx: vec![], kick: vec![] };
            crate::social::on_gone(&mut cx, id);
            let fx = std::mem::take(&mut cx.fx);
            for c in fx {
                if let Some(a) = self.registry.addr_of(c.to) {
                    if let Some(p) = self.peers.get_mut(&a) {
                        p.push(&c.msg);
                    }
                }
            }
        }
    }

    /// Online matches that ended on the match server: results, rewards, `85 00` (online.rs).
    fn online_tick(&mut self, now: Instant) {
        let mut cx = Ctx { lobby: &self.lobby, registry: &mut self.registry, now, fx: vec![], kick: vec![] };
        if self.lobby.online.is_some() {
            crate::online::tick(&mut cx);
        }
        // changes made outside a handler (the match server's thread, tests) become notices here too
        crate::notices::flush(&mut cx);
        let fx = std::mem::take(&mut cx.fx);
        for c in fx {
            if let Some(a) = self.registry.addr_of(c.to) {
                if let Some(p) = self.peers.get_mut(&a) {
                    p.push(&c.msg);
                }
            }
        }
    }

    /// The datagrams to send now (every peer), and the end of peers that went silent.
    pub fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        self.online_tick(now);
        if self.last_touch.is_none_or(|t| now.saturating_duration_since(t) >= SESSION_TOUCH_EVERY) {
            self.last_touch = Some(now);
            self.lobby.sessions.touch(&self.registry.live_sessions(), crate::sessions::unix_now());
        }
        let mut out = Vec::new();
        let mut gone = Vec::new();
        for (addr, p) in self.peers.iter_mut() {
            if p.transport.is_gone(now) {
                // what the closed connection still owes the client: the FB of its FA (the client waits for it before
                // its connection ends; without it the client cannot get back to the login screen)
                out.extend(p.transport.poll_low_level(now).into_iter().map(|d| (*addr, d)));
                gone.push(*addr);
                continue;
            }
            if p.phase == Phase::New && now.saturating_duration_since(p.since) >= SIGN_IN_DEADLINE {
                // e.g. the old connection of a client after a quick server restart: its keepalives get acks, its
                // requests no answers; the FD makes it run its own reconnect with the 2E
                log::info!("{addr}: no 2E within {} s: refused (FD)", SIGN_IN_DEADLINE.as_secs());
                login::refuse(p);
            }
            let ds = p.transport.poll(now);
            if p.phase == Phase::SignedPending && !ds.is_empty() {
                p.phase = Phase::AwaitAck;
            }
            out.extend(ds.into_iter().map(|d| (*addr, d)));
        }
        for a in gone {
            self.drop_peer(a);
        }
        out
    }
}
