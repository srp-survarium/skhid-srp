//! Scripted clients for the lobby tests: a client-role `Transport` per client, a virtual clock, no sockets.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use accounts::{AccountId, HashCost, Store};
use lobby_server::{
    Lobby, LobbyConfig, LocalMatches, MatchCtx, MatchOutcome, MinimalMatches, Server, SessionTable,
};
use protocol::c2s::Request;
use protocol::s2c::{Reply, S2c};
use udp_transport::{Event, Role, Transport};

pub struct Client {
    pub addr: SocketAddr,
    pub t: Transport,
    pub account: AccountId,
    pub session: u32,
    pub inbox: Vec<(u8, Vec<u8>)>,
    /// The server ended the connection (FB / FD reached the client).
    pub closed: bool,
    /// Low-level ids the server sent (FA / FB / FD; keepalives FC left out), in order.
    pub low: Vec<u8>,
}

impl Client {
    pub fn send(&mut self, r: Request) {
        let (id, pl) = r.encode().expect("encode request");
        self.t.send(id, &pl).expect("client send");
    }

    pub fn send_raw(&mut self, id: u8, pl: &[u8]) {
        self.t.send(id, pl).expect("client send");
    }

    /// Everything received since the last call, decoded.
    pub fn take(&mut self) -> Vec<S2c> {
        std::mem::take(&mut self.inbox)
            .into_iter()
            .map(|(id, pl)| match S2c::decode(id, &pl) {
                Ok(m) => m,
                // the protocol crate does not decode `83 5B 02 <size 0>` and `83 60 00` (short answers the client accepts)
                Err(_) if id == 0x83 && !pl.is_empty() => S2c::Reply(Reply::Raw { request: pl[0], data: pl[1..].to_vec() }),
                // the recorder's fake results payload
                Err(_) if id == 0x97 => S2c::Raw { id, payload: pl },
                Err(e) => panic!("decode {id:#04x} {pl:02x?}: {e}"),
            })
            .collect()
    }

    pub fn ids(&self) -> Vec<u8> {
        self.inbox.iter().map(|(i, _)| *i).collect()
    }
}

pub struct Env {
    pub store: Arc<Store>,
    pub sessions: Arc<SessionTable>,
    pub server: Server,
    pub now: Instant,
    next_port: u16,
}

pub fn build_server(store: &Arc<Store>, sessions: &Arc<SessionTable>, cfg: LobbyConfig, matches: Arc<dyn LocalMatches>) -> Server {
    let data = Arc::new(game_data::testing::tables());
    let lobby = Lobby::new(cfg, store.clone(), data, sessions.clone(), matches);
    lobby.seed(12345);
    Server::new(lobby)
}

pub fn open_store(path: &Path) -> Arc<Store> {
    let s = Store::open(path).unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    Arc::new(s)
}

impl Env {
    pub fn new(db: &Path) -> Env {
        // the reference vectors carry neither achievement rewards nor a login calendar
        Env::with(db, LobbyConfig { achievement_rewards: false, login_calendar: None, ..LobbyConfig::default() }, Arc::new(MinimalMatches))
    }

    pub fn with(db: &Path, cfg: LobbyConfig, matches: Arc<dyn LocalMatches>) -> Env {
        let store = open_store(db);
        // as the srp binary (open_parts): sessions in the database
        let sessions = Arc::new(SessionTable::persistent(store.clone()));
        let server = build_server(&store, &sessions, cfg, matches);
        Env { store, sessions, server, now: Instant::now(), next_port: 40000 }
    }

    /// Restart: a new server over the same database (peers are gone; the sessions stay in the database).
    pub fn restart(&mut self, db: &Path, cfg: LobbyConfig, matches: Arc<dyn LocalMatches>) {
        self.store = open_store(db);
        self.sessions = Arc::new(SessionTable::persistent(self.store.clone()));
        self.server = build_server(&self.store, &self.sessions, cfg, matches);
    }

    /// The client's own lobby reconnect: a new UDP connection (new port, new transport) that sends the 2E of the
    /// session it already holds (no new sign-in at the login server).
    pub fn reconnect(&mut self, old: &Client) -> Client {
        self.next_port += 1;
        let addr: SocketAddr = format!("127.0.0.1:{}", self.next_port).parse().unwrap();
        Client { addr, t: Transport::new(Role::Client, self.now), account: old.account, session: old.session, inbox: vec![], closed: false, low: vec![] }
    }

    fn new_client(&mut self, account: AccountId) -> Client {
        self.next_port += 1;
        let addr: SocketAddr = format!("127.0.0.1:{}", self.next_port).parse().unwrap();
        Client {
            addr,
            t: Transport::new(Role::Client, self.now),
            account,
            session: self.sessions.grant(account),
            inbox: vec![],
            closed: false,
            low: vec![],
        }
    }

    /// Create an account (starter profile) and a client holding a session for it.
    pub fn client(&mut self, name: &str) -> Client {
        let id = self.store.create_account(name, "pw").unwrap();
        self.new_client(id)
    }

    /// A client for an existing account.
    pub fn client_of(&mut self, name: &str) -> Client {
        let id = self.store.find_account(name).unwrap().unwrap().id;
        self.new_client(id)
    }

    /// One round: clients send what they have, the server answers, clients receive.
    pub fn step(&mut self, clients: &mut [&mut Client], dt: Duration) {
        self.now += dt;
        for c in clients.iter_mut() {
            for d in c.t.poll(self.now) {
                self.server.receive(c.addr, &d, self.now);
            }
        }
        for (addr, d) in self.server.poll(self.now) {
            if let Some(c) = clients.iter_mut().find(|c| c.addr == addr) {
                if let Ok(dg) = udp_transport::wire::parse(udp_transport::Direction::ToClient, &d) {
                    for m in dg.messages {
                        if let udp_transport::Message::LowLevel(t) = m {
                            if t != 0xFC {
                                c.low.push(t);
                            }
                        }
                    }
                }
                for ev in c.t.receive(&d, self.now) {
                    match ev {
                        Event::Message { id, payload, .. } => c.inbox.push((id, payload)),
                        Event::Disconnected => c.closed = true,
                    }
                }
            }
        }
    }

    pub fn pump(&mut self, clients: &mut [&mut Client]) {
        for _ in 0..40 {
            self.step(clients, Duration::from_millis(30));
        }
    }

    /// Advance the virtual clock (keepalives keep the peers).
    pub fn wait(&mut self, clients: &mut [&mut Client], secs: u64) {
        for _ in 0..secs * 4 {
            self.step(clients, Duration::from_millis(250));
        }
    }

    /// 2E and the whole login sequence; the inbox is emptied afterwards (the pushes are returned).
    pub fn sign_in(&mut self, c: &mut Client) -> Vec<S2c> {
        c.send(Request::SignIn { session: c.session, a: 0xFF, b: 0xFF });
        for _ in 0..300 {
            self.step(&mut [&mut *c], Duration::from_millis(30));
            if c.inbox.iter().any(|(id, _)| *id == 0xB8) {
                break;
            }
        }
        self.pump(&mut [&mut *c]);
        c.take()
    }

    /// The full lobby-load handshake (5B 02) after the sign-in.
    pub fn handshake(&mut self, c: &mut Client) -> Vec<S2c> {
        c.send(Request::Play(protocol::c2s::Play5B::Load));
        self.pump(&mut [&mut *c]);
        c.take()
    }
}

/// A local match hook that records its calls.
#[derive(Default)]
pub struct Recorder {
    pub calls: Mutex<Vec<String>>,
}

impl LocalMatches for Recorder {
    fn training(&self, ctx: &MatchCtx, map: u32, mode: u32, weather: u32) -> Vec<u8> {
        self.calls.lock().unwrap().push(format!("training {} {map} {mode} {weather} items {}", ctx.account_name, ctx.profile.items.len()));
        vec![1, 2, 3, 4]
    }

    fn tutorial(&self, ctx: &MatchCtx) -> Vec<u8> {
        self.calls.lock().unwrap().push(format!("tutorial {}", ctx.account_name));
        vec![9; 5]
    }

    fn shooting_range(&self, ctx: &MatchCtx, weapon: &protocol::items::ItemRecord, profile: u8) -> Vec<u8> {
        self.calls.lock().unwrap().push(format!("range {} dict {} uid {} profile {profile}", ctx.account_name, weapon.dict, weapon.uid));
        vec![3; 6]
    }

    fn finished(&self, ctx: &MatchCtx, map: u32, mode: u32, won: bool, finished: bool, events: &[u16]) -> MatchOutcome {
        self.calls.lock().unwrap().push(format!("finished {} {map} {mode} won {won} fin {finished} {events:?}", ctx.account_name));
        MatchOutcome { results97: Some(vec![7, 7]), exp: 500, silver: 1000, reputation: 50, first_game: false }
    }
}

/// The reference vectors number items 1, 2, 3 ...; the Rust lobby starts at `UID_BASE`: shift every item uid of a message.
pub fn lift(mut m: S2c) -> S2c {
    use protocol::items::ItemRecord;
    use protocol::s2c::{Inventory45, Shop46};
    const D: u32 = accounts::UID_BASE - 1;
    fn rec(r: &mut ItemRecord) {
        r.uid += D;
        if r.attached_to != 0 {
            r.attached_to += D;
        }
        for x in &mut r.modules {
            x.uid += D;
        }
    }
    match &mut m {
        S2c::Inventory(i) => i.items.iter_mut().for_each(rec),
        S2c::Account(a) => a.body.items.iter_mut().for_each(rec),
        S2c::Reply(Reply::Shop(Shop46::Buy { items, .. })) => items.iter_mut().for_each(rec),
        S2c::Reply(Reply::Shop(Shop46::Sell { item, .. })) => *item += D,
        S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) => items.iter_mut().for_each(rec),
        _ => {}
    }
    m
}

pub fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

/// A named vector of `crates/protocol/tests/vectors/<file>`: (id, payload).
pub fn vector(file: &str, name: &str) -> (u8, Vec<u8>) {
    let p = format!("{}/../../crates/protocol/tests/vectors/{file}", env!("CARGO_MANIFEST_DIR"));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    let e = v.as_array().unwrap().iter().find(|e| e["name"] == name).unwrap_or_else(|| panic!("no vector {name}"));
    (e["id"].as_u64().unwrap() as u8, unhex(e["payload"].as_str().unwrap()))
}
