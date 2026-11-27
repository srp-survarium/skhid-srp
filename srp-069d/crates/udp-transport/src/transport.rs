//! One reliable-UDP peer, sans IO: feed it datagrams with [`Transport::receive`], send what
//! [`Transport::poll`] returns. Behaviour = the earlier reference implementation that worked with the real
//! client, plus the client's class table:
//!
//! * our seq is a 9-bit counter, every send and every resend takes a NEW seq (a resent seq is a
//!   duplicate for the client and is dropped);
//! * R = newest peer seq (cyclic), `0x1FF` until the first peer datagram; the ack stream covers the
//!   peer seqs in the 256 window behind R, `tail` bytes wide;
//! * unique and ordered messages are kept until a datagram they went out in is acked, and resent
//!   after [`Config::resend`] (1 s: the client acks up to 0.5 s late); plain messages go once;
//! * messages are numbered per class: unique ids and order ids are separate counters (low 8 bits on
//!   the wire), plain ids carry no x byte;
//! * a message whose single datagram would exceed ~460 bytes goes in 440-byte parts
//!   (`FF id x part [count]`), each part its own single datagram;
//! * every datagram is acked at once (the client drops the connection on "too old acks" after ~0.4 s).

use crate::ack::Header;
use crate::seq::{self, Received, Recorded};
use crate::table::{class_of, low_level, Class, Direction};
use crate::wire::{self, Data, Message, PART};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Which end this transport is. The client role exists for scripted-client tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Server,
    Client,
}

impl Role {
    fn recv_direction(self) -> Direction {
        match self {
            Role::Server => Direction::ToServer,
            Role::Client => Direction::ToClient,
        }
    }

    fn send_direction(self) -> Direction {
        match self {
            Role::Server => Direction::ToClient,
            Role::Client => Direction::ToServer,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// A keepalive (FC) goes when nothing was sent for this long.
    pub idle: Duration,
    /// A received datagram is acked by a keepalive this long after the last send, if no message
    /// carried the ack sooner.
    pub ack_delay: Duration,
    /// Resend interval of unique/ordered messages.
    pub resend: Duration,
    /// Tail bytes after ack0 (7 ack bits each, plus 4 in the final byte).
    pub tail: usize,
    /// Payload bytes per part.
    pub part: usize,
    /// `is_gone` after this long without a peer datagram.
    pub timeout: Duration,
    /// The most datagrams `poll` returns at once.
    pub burst: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idle: Duration::from_millis(250),
            ack_delay: Duration::from_millis(20),
            resend: Duration::from_millis(1000),
            tail: 4,
            part: PART,
            timeout: Duration::from_secs(10),
            burst: 48,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// A message of the peer, parts reassembled, unique duplicates dropped, ordered ones in order.
    Message { id: u8, x: Option<u8>, payload: Vec<u8> },
    /// The peer left (low-level FA / FB / FD).
    Disconnected,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    /// Not an id the peer's table accepts.
    UnknownId(u8),
    /// More than 255 parts.
    TooLarge(usize),
}

struct Outgoing {
    key: u64,
    /// Message area of its single-format datagram.
    body: Vec<u8>,
    seqs: Vec<u16>,
    last_sent: Instant,
}

struct Assembly {
    count: Option<u8>,
    parts: BTreeMap<u8, Vec<u8>>,
}

const MAX_ASSEMBLIES: usize = 16;
const UNIQUE_WINDOW: usize = 128;
const ORDER_WINDOW: u8 = 128;
const SEQS_KEPT_PER_MESSAGE: usize = 16;

pub struct Transport {
    role: Role,
    config: Config,
    next_seq: u16,
    received: Received,
    unique_out: u32,
    order_out: u32,
    /// Bodies waiting for their first send; the flag = reliable.
    pending: VecDeque<(Vec<u8>, bool)>,
    unacked: Vec<Outgoing>,
    seq_key: HashMap<u16, u64>,
    next_key: u64,
    low_level_out: VecDeque<u8>,
    ack_due: bool,
    last_sent: Option<Instant>,
    last_received: Instant,
    unique_seen: VecDeque<u8>,
    ordered_next: u8,
    ordered_wait: BTreeMap<u8, (u8, Vec<u8>)>,
    assemblies: Vec<((u8, Option<u8>), Assembly)>,
    gone: bool,
    resent: u32,
    rejected: u32,
}

impl Transport {
    pub fn new(role: Role, now: Instant) -> Self {
        Self::with_config(role, Config::default(), now)
    }

    pub fn with_config(role: Role, config: Config, now: Instant) -> Self {
        Self {
            role,
            config,
            next_seq: 0,
            received: Received::default(),
            unique_out: 0,
            order_out: 0,
            pending: VecDeque::new(),
            unacked: Vec::new(),
            seq_key: HashMap::new(),
            next_key: 0,
            low_level_out: VecDeque::new(),
            ack_due: false,
            last_sent: None,
            last_received: now,
            unique_seen: VecDeque::new(),
            ordered_next: 0,
            ordered_wait: BTreeMap::new(),
            assemblies: Vec::new(),
            gone: false,
            resent: 0,
            rejected: 0,
        }
    }

    pub fn role(&self) -> Role {
        self.role
    }

    /// Datagrams sent again.
    pub fn resent(&self) -> u32 {
        self.resent
    }

    /// Datagrams dropped because they did not parse (not acked, as the client does).
    pub fn rejected(&self) -> u32 {
        self.rejected
    }

    /// Reliable messages (parts) not yet acked.
    pub fn unacked(&self) -> usize {
        self.unacked.len()
    }

    /// Next unique id / order id to be assigned (full counts; the wire carries the low 8 bits).
    pub fn counters(&self) -> (u32, u32) {
        (self.unique_out, self.order_out)
    }

    pub fn is_gone(&self, now: Instant) -> bool {
        self.gone || now.saturating_duration_since(self.last_received) >= self.config.timeout
    }

    /// Queue a message. Takes the next unique or order id by the id's class. Returns the x byte
    /// that went out (`None` for plain ids).
    pub fn send(&mut self, id: u8, payload: &[u8]) -> Result<Option<u8>, SendError> {
        let dir = self.role.send_direction();
        let class = class_of(dir, id).ok_or(SendError::UnknownId(id))?;
        let head = 1 + usize::from(class.has_x());
        let split = head + payload.len() > self.config.part + 20;
        let parts = if split {
            let n = payload.len().div_ceil(self.config.part);
            if n > 255 {
                return Err(SendError::TooLarge(payload.len()));
            }
            n
        } else {
            1
        };
        let x = match class {
            Class::Unique => {
                let x = self.unique_out as u8;
                self.unique_out += 1;
                Some(x)
            }
            Class::Ordered => {
                let x = self.order_out as u8;
                self.order_out += 1;
                Some(x)
            }
            Class::Plain => None,
        };
        let reliable = class != Class::Plain;
        let datas = if split {
            wire::split(dir, id, x, payload, self.config.part).expect("checked above")
        } else {
            vec![Data { id, x, part: None, payload: payload.to_vec() }]
        };
        debug_assert_eq!(datas.len(), parts);
        for d in datas {
            let body = wire::encode_area(dir, false, &[Message::Data(d)]).expect("class checked");
            self.pending.push_back((body, reliable));
        }
        Ok(x)
    }

    /// Process one datagram of the peer.
    pub fn receive(&mut self, datagram: &[u8], now: Instant) -> Vec<Event> {
        let mut events = Vec::new();
        let dg = match wire::parse(self.role.recv_direction(), datagram) {
            Ok(dg) => dg,
            Err(e) => {
                // dropped whole and not acked: the peer sends its messages again
                log::debug!("datagram did not parse ({e}); dropped");
                self.rejected += 1;
                return events;
            }
        };
        self.last_received = now;
        for s in dg.header.acked() {
            self.ack_seq(s);
        }
        match self.received.record(dg.header.seq) {
            Recorded::New => self.ack_due = true,
            Recorded::Duplicate => {
                self.ack_due = true;
                return events;
            }
            Recorded::TooOld => return events,
        }
        for m in dg.messages {
            match m {
                Message::LowLevel(t) => self.low_level(t, &mut events),
                Message::Data(d) => self.data(d, &mut events),
            }
        }
        events
    }

    fn low_level(&mut self, t: u8, events: &mut Vec<Event>) {
        match t {
            low_level::INITIATE_DISCONNECTION => {
                if self.low_level_out.len() < 8 {
                    self.low_level_out.push_back(low_level::CONFIRM_DISCONNECTION);
                }
                if !self.gone {
                    self.gone = true;
                    events.push(Event::Disconnected);
                }
            }
            low_level::CONFIRM_DISCONNECTION | low_level::INSTANT_DISCONNECT => {
                if !self.gone {
                    self.gone = true;
                    events.push(Event::Disconnected);
                }
            }
            _ => {}
        }
    }

    fn data(&mut self, d: Data, events: &mut Vec<Event>) {
        let Some(class) = class_of(self.role.recv_direction(), d.id) else { return };
        if class == Class::Unique {
            if let Some(x) = d.x {
                if self.unique_seen.contains(&x) {
                    return;
                }
            }
        }
        let (id, x) = (d.id, d.x);
        let payload = match d.part {
            None => d.payload,
            Some(part) => {
                let key = (id, x);
                let at = match self.assemblies.iter().position(|(k, _)| *k == key) {
                    Some(at) => at,
                    None => {
                        if self.assemblies.len() == MAX_ASSEMBLIES {
                            self.assemblies.remove(0);
                        }
                        self.assemblies.push((key, Assembly { count: None, parts: BTreeMap::new() }));
                        self.assemblies.len() - 1
                    }
                };
                let a = &mut self.assemblies[at].1;
                if part.index == 0 {
                    a.count = part.count;
                }
                a.parts.entry(part.index).or_insert(d.payload);
                let done = a.count.is_some_and(|c| a.parts.len() >= usize::from(c));
                if !done {
                    return;
                }
                let (_, a) = self.assemblies.remove(at);
                a.parts.into_values().flatten().collect()
            }
        };
        match class {
            Class::Plain => events.push(Event::Message { id, x, payload }),
            Class::Unique => {
                if let Some(x) = x {
                    if self.unique_seen.len() == UNIQUE_WINDOW {
                        self.unique_seen.pop_front();
                    }
                    self.unique_seen.push_back(x);
                }
                events.push(Event::Message { id, x, payload });
            }
            Class::Ordered => {
                let x = x.unwrap_or(0);
                let ahead = x.wrapping_sub(self.ordered_next);
                if ahead == 0 {
                    events.push(Event::Message { id, x: Some(x), payload });
                    self.ordered_next = self.ordered_next.wrapping_add(1);
                    while let Some((id, payload)) = self.ordered_wait.remove(&self.ordered_next) {
                        events.push(Event::Message { id, x: Some(self.ordered_next), payload });
                        self.ordered_next = self.ordered_next.wrapping_add(1);
                    }
                } else if ahead < ORDER_WINDOW {
                    self.ordered_wait.entry(x).or_insert((id, payload));
                }
                // else: old or duplicate, dropped
            }
        }
    }

    fn ack_seq(&mut self, s: u16) {
        if let Some(key) = self.seq_key.remove(&s) {
            if let Some(at) = self.unacked.iter().position(|o| o.key == key) {
                let o = self.unacked.remove(at);
                for q in o.seqs {
                    if self.seq_key.get(&q) == Some(&key) {
                        self.seq_key.remove(&q);
                    }
                }
            }
        }
    }

    fn datagram(&mut self, body: &[u8], now: Instant) -> (u16, Vec<u8>) {
        let s = self.next_seq;
        let r = self.received.newest().unwrap_or(seq::NONE);
        let h = Header::new(s, r, false, self.received.set(), self.config.tail);
        self.next_seq = seq::next(s);
        self.last_sent = Some(now);
        self.ack_due = false;
        (s, crate::ack::encode(&h, body).expect("tail <= 35"))
    }

    /// Queue `FD` (instant disconnect): the 0.69d client ends the connection at once when it reads one
    /// (`udp_match_connection::process_low_level_message` 0x200751B80, case FD -> 0x2007509B0 with event 4), whatever
    /// its state. The transport itself stays as it is.
    pub fn instant_disconnect(&mut self) {
        if self.low_level_out.len() < 8 {
            self.low_level_out.push_back(low_level::INSTANT_DISCONNECT);
        }
    }

    /// Queue `FA` (initiate disconnection); the peer answers `FB` (tests play the client's side with it).
    pub fn initiate_disconnect(&mut self) {
        if self.low_level_out.len() < 8 {
            self.low_level_out.push_back(low_level::INITIATE_DISCONNECTION);
        }
    }

    /// Only the queued low-level datagrams (`FB` confirming the peer's `FA`, `FD`): what a connection that is over
    /// still owes the peer. The 0.69d client that sent `FA` waits for the `FB` (case FB with its own `FA` pending ->
    /// 0x2007509B0 event 2) before the connection is closed.
    pub fn poll_low_level(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(t) = self.low_level_out.pop_front() {
            out.push(self.datagram(&[t], now).1);
        }
        out
    }

    /// The datagrams to send now: low-level, resends that are due, new messages, and a keepalive
    /// when nothing else went out and an ack is due or the line is idle.
    pub fn poll(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(t) = self.low_level_out.pop_front() {
            out.push(self.datagram(&[t], now).1);
        }
        // resends, oldest first
        let due: Vec<usize> = (0..self.unacked.len())
            .filter(|i| now.saturating_duration_since(self.unacked[*i].last_sent) >= self.config.resend)
            .collect();
        for i in due {
            if out.len() >= self.config.burst {
                break;
            }
            let body = self.unacked[i].body.clone();
            let key = self.unacked[i].key;
            let (s, d) = self.datagram(&body, now);
            let o = &mut self.unacked[i];
            o.last_sent = now;
            o.seqs.push(s);
            if o.seqs.len() > SEQS_KEPT_PER_MESSAGE {
                let old = o.seqs.remove(0);
                if self.seq_key.get(&old) == Some(&key) {
                    self.seq_key.remove(&old);
                }
            }
            self.seq_key.insert(s, key);
            self.resent += 1;
            out.push(d);
        }
        while out.len() < self.config.burst {
            let Some((body, reliable)) = self.pending.pop_front() else { break };
            let (s, d) = self.datagram(&body, now);
            if reliable {
                let key = self.next_key;
                self.next_key += 1;
                self.seq_key.insert(s, key);
                self.unacked.push(Outgoing { key, body, seqs: vec![s], last_sent: now });
            }
            out.push(d);
        }
        if out.is_empty() && !self.gone {
            let quiet = |pace: Duration| {
                self.last_sent.is_none_or(|t| now.saturating_duration_since(t) >= pace)
            };
            if (self.ack_due && quiet(self.config.ack_delay)) || (self.received.newest().is_some() && quiet(self.config.idle)) {
                out.push(self.datagram(&[low_level::CONTINUOUS_FLOW], now).1);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Transport, Transport, Instant) {
        let t0 = Instant::now();
        (Transport::new(Role::Client, t0), Transport::new(Role::Server, t0), t0)
    }

    /// Deliver everything `from` has to say to `to`; returns the events.
    fn pump(from: &mut Transport, to: &mut Transport, now: Instant) -> Vec<Event> {
        let mut ev = Vec::new();
        for d in from.poll(now) {
            ev.extend(to.receive(&d, now));
        }
        ev
    }

    #[test]
    fn sign_in_and_reply() {
        let (mut c, mut s, t0) = pair();
        assert_eq!(c.send(0x2E, &[1, 2, 3]).unwrap(), Some(0));
        let ev = pump(&mut c, &mut s, t0);
        assert_eq!(ev, vec![Event::Message { id: 0x2E, x: Some(0), payload: vec![1, 2, 3] }]);
        // server answers 80 (unique x=0) and 8F, 8B (ordered x=0, x=1)
        assert_eq!(s.send(0x80, &[0; 23]).unwrap(), Some(0));
        assert_eq!(s.send(0x8F, &[9]).unwrap(), Some(0));
        assert_eq!(s.send(0x8B, &[8]).unwrap(), Some(1));
        let ev = pump(&mut s, &mut c, t0);
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[2], Event::Message { id: 0x8B, x: Some(1), payload: vec![8] });
        assert_eq!(s.counters(), (1, 2));
    }

    #[test]
    fn plain_has_no_counter_and_goes_once() {
        let (mut c, mut s, t0) = pair();
        c.send(0x2E, &[]).unwrap();
        pump(&mut c, &mut s, t0);
        assert_eq!(s.send(0xAC, &[0x3c, 0, 0, 0, 0, 0, 0, 0]).unwrap(), None);
        assert_eq!(s.send(0x9E, &[1]).unwrap(), Some(0)); // unique
        assert_eq!(s.send(0x83, &[1]).unwrap(), Some(0)); // ordered, own counter
        assert_eq!(s.counters(), (1, 1));
        let first = s.poll(t0);
        assert_eq!(first.len(), 3);
        assert_eq!(first[0][3], 0xAC);
        assert_eq!(first[0][4], 0x3c); // no x byte between id and payload
        // the plain one is not kept; the two reliable ones are
        assert_eq!(s.unacked(), 2);
    }

    #[test]
    fn multipart_round_trip() {
        let (mut c, mut s, t0) = pair();
        c.send(0x2E, &[]).unwrap();
        pump(&mut c, &mut s, t0);
        let payload: Vec<u8> = (0..960u32).map(|i| (i * 7) as u8).collect();
        s.send(0x87, &payload).unwrap();
        let ds = s.poll(t0);
        assert_eq!(ds.len(), 3);
        assert!(ds.iter().all(|d| d.len() <= wire::MAX_DATAGRAM));
        let mut ev = Vec::new();
        // parts may arrive in any order
        for d in ds.iter().rev() {
            ev.extend(c.receive(d, t0));
        }
        assert_eq!(ev, vec![Event::Message { id: 0x87, x: Some(0), payload }]);
    }

    #[test]
    fn resend_until_acked_with_new_seqs() {
        let (mut c, mut s, t0) = pair();
        c.send(0x2E, &[]).unwrap();
        pump(&mut c, &mut s, t0);
        s.send(0x85, &[0]).unwrap();
        let first = s.poll(t0);
        assert_eq!(first.len(), 1);
        // nothing within the resend interval except keepalives
        let t1 = t0 + Duration::from_millis(500);
        assert!(s.poll(t1).iter().all(|d| d[3] == 0xFC));
        // lost; after 1 s it goes again under a new seq
        let t2 = t0 + Duration::from_millis(1100);
        let again = s.poll(t2);
        let resent: Vec<_> = again.iter().filter(|d| d[3] == 0x85).collect();
        assert_eq!(resent.len(), 1);
        assert_ne!(first[0][0], resent[0][0]);
        assert_eq!(s.resent(), 1);
        // the client gets only the resend: delivered once
        let ev = c.receive(resent[0], t2);
        assert_eq!(ev.len(), 1);
        // the client's ack (a keepalive) clears it
        for d in c.poll(t2 + Duration::from_millis(30)) {
            s.receive(&d, t2);
        }
        assert_eq!(s.unacked(), 0);
        assert!(s.poll(t2 + Duration::from_millis(3000)).iter().all(|d| d[3] == 0xFC));
    }

    #[test]
    fn late_ack_of_first_copy_counts() {
        let (mut c, mut s, t0) = pair();
        c.send(0x2E, &[]).unwrap();
        pump(&mut c, &mut s, t0);
        s.send(0x85, &[0]).unwrap();
        let first = s.poll(t0);
        let t2 = t0 + Duration::from_millis(1100);
        let _lost_resend = s.poll(t2);
        // only the FIRST copy reaches the client; its ack arrives after the resend went out
        c.receive(&first[0], t2);
        for d in c.poll(t2 + Duration::from_millis(30)) {
            s.receive(&d, t2);
        }
        assert_eq!(s.unacked(), 0);
    }

    #[test]
    fn unique_duplicates_dropped_and_ordered_in_order() {
        let t0 = Instant::now();
        let mut s = Transport::new(Role::Server, t0);
        let mut c = Transport::new(Role::Client, t0);
        // client sends two copies of the same unique message under different seqs
        c.send(0x32, &[2, 0]).unwrap();
        let d1 = c.poll(t0);
        let d2 = {
            // force a second copy
            let t = t0 + Duration::from_millis(1100);
            c.poll(t)
        };
        let mut ev = Vec::new();
        for d in d1.iter().chain(d2.iter()) {
            ev.extend(s.receive(d, t0));
        }
        let n = ev.iter().filter(|e| matches!(e, Event::Message { id: 0x32, .. })).count();
        assert_eq!(n, 1);
        // ordered S->C arriving out of order is delivered in order
        let mut srv = Transport::new(Role::Server, t0);
        srv.send(0x83, &[10]).unwrap();
        srv.send(0x84, &[11]).unwrap();
        srv.send(0x85, &[12]).unwrap();
        let mut cl = Transport::new(Role::Client, t0);
        // the server needs a client datagram first for a normal R; here R = 0x1FF is fine
        let ds = srv.poll(t0);
        let mut ev = Vec::new();
        ev.extend(cl.receive(&ds[2], t0));
        ev.extend(cl.receive(&ds[1], t0));
        assert!(ev.is_empty());
        ev.extend(cl.receive(&ds[0], t0));
        let ids: Vec<u8> = ev.iter().map(|e| match e { Event::Message { id, .. } => *id, _ => 0 }).collect();
        assert_eq!(ids, vec![0x83, 0x84, 0x85]);
    }

    #[test]
    fn seq_wraps_at_512_and_keeps_working() {
        let (mut c, mut s, t0) = pair();
        c.send(0x2E, &[]).unwrap();
        pump(&mut c, &mut s, t0);
        let mut got = 0;
        for i in 0..700u32 {
            let t = t0 + Duration::from_millis(30 * u64::from(i));
            s.send(0x83, &[i as u8]).unwrap();
            for d in s.poll(t) {
                got += c.receive(&d, t).iter().filter(|e| matches!(e, Event::Message { .. })).count();
            }
            for d in c.poll(t + Duration::from_millis(25)) {
                s.receive(&d, t);
            }
        }
        assert_eq!(got, 700);
        assert_eq!(s.unacked(), 0);
        assert_eq!(s.rejected(), 0);
        assert_eq!(c.rejected(), 0);
    }

    #[test]
    fn errors_and_garbage() {
        let (_, mut s, t0) = pair();
        assert_eq!(s.send(0x2E, &[]), Err(SendError::UnknownId(0x2E)));
        assert_eq!(s.send(0x80, &vec![0; 441 * 256]), Err(SendError::TooLarge(441 * 256)));
        assert!(s.receive(&[1, 2], t0).is_empty());
        assert_eq!(s.rejected(), 1);
    }

    #[test]
    fn disconnect() {
        let (mut c, mut s, t0) = pair();
        let mut dg = Vec::new();
        c.low_level_out.push_back(low_level::INITIATE_DISCONNECTION);
        dg.extend(c.poll(t0));
        let ev = s.receive(&dg[0], t0);
        assert_eq!(ev, vec![Event::Disconnected]);
        assert!(s.is_gone(t0));
        assert_eq!(s.poll(t0).len(), 1); // FB
    }

    #[test]
    fn fb_and_fd_from_poll_low_level() {
        let (mut c, mut s, t0) = pair();
        c.low_level_out.push_back(low_level::INITIATE_DISCONNECTION);
        let dg = c.poll(t0);
        assert_eq!(s.receive(&dg[0], t0), vec![Event::Disconnected]);
        let fb = s.poll_low_level(t0);
        assert_eq!(fb.len(), 1);
        // the client side sees the FB as the end of its own disconnection
        assert_eq!(c.receive(&fb[0], t0), vec![Event::Disconnected]);
        assert!(s.poll_low_level(t0).is_empty());

        let (mut c, mut s, t0) = pair();
        s.instant_disconnect();
        let fd = s.poll_low_level(t0);
        assert_eq!(fd.len(), 1);
        assert_eq!(c.receive(&fd[0], t0), vec![Event::Disconnected]);
        assert!(!s.is_gone(t0), "FD does not end the sender's transport");
    }
}
