//! Reliable-UDP transport of the MATCH connection (sans IO), a variant of `udp_transport::Transport`.
//!
//! Why a copy: `udp_transport::Transport` and `udp_transport::wire` call `class_of(direction, id)` of the lobby
//! table, so they reject every match id (0xC9..0xE5) as "invalid message type" and cannot express fixed-length
//! ids, lifetimes or the cfg resend of DF/E0. Header codec (`ack`) and cyclic seq window (`seq`) of
//! udp-transport are reused unchanged; only the message layer and the send/resend rules differ:
//!
//! * the table is [`crate::table`] (lengths: fixed ids have no length prefix in a multi datagram, E4 is by content);
//! * every message has a lifetime and resend interval from the table: reliable ids are resent (with a NEW seq)
//!   until acked or until their lifetime ends (DE: 1900 ms / 100 ms, DF: 2 x send_period); lifetime 0 = once;
//! * plain ids that are resent (C9, CA, CB, CE, DC, DF, ...) arrive more than once at the peer: the application
//!   must deduplicate them (the transport only drops duplicate DATAGRAMS and duplicate unique x ids);
//! * keepalive FC after 100 ms idle (match connection, PROVEN value; lobby 500 ms).
//!
//! Everything else (header, 9-bit seqs, tail acks, ack at once, parts of 440 bytes) is as implemented in
//! udp-transport.

use crate::table::{self, Direction, Info, Len, Life};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant};
use udp_transport::ack::{self, Header};
use udp_transport::seq::{self, Received, Recorded};
use udp_transport::wire::{Data, Datagram, Message, ParseError, Part, MAX_DATAGRAM, PART};
use udp_transport::{Class, Role};

const MULTIPART: u8 = 0xFF;

fn is_low_level(t: u8) -> bool {
    (0xFA..=0xFD).contains(&t)
}
const FA: u8 = 0xFA;
const FB: u8 = 0xFB;
const FC: u8 = 0xFC;
const FD: u8 = 0xFD;

/// Parse one datagram travelling in `direction` with the match table.
pub fn parse(direction: Direction, d: &[u8]) -> Result<Datagram, ParseError> {
    let (header, end) = ack::decode(d)?;
    let area = &d[3..end];
    let multi = header.multi;
    let mut messages = Vec::new();
    let mut i = 0;
    while i < area.len() {
        let t = area[i];
        i += 1;
        if is_low_level(t) {
            messages.push(Message::LowLevel(t));
            continue;
        }
        let prefixed = t == MULTIPART;
        let id = if prefixed {
            let id = *area.get(i).ok_or(ParseError::Truncated)?;
            i += 1;
            id
        } else {
            t
        };
        let (class, len) = table::spec(direction, id).ok_or(ParseError::InvalidType(id))?;
        let x = if class.has_x() {
            let x = *area.get(i).ok_or(ParseError::Truncated)?;
            i += 1;
            Some(x)
        } else {
            None
        };
        let part = if prefixed {
            let index = *area.get(i).ok_or(ParseError::Truncated)?;
            i += 1;
            let count = if index == 0 {
                let c = *area.get(i).ok_or(ParseError::Truncated)?;
                i += 1;
                Some(c)
            } else {
                None
            };
            Some(Part { index, count })
        } else {
            None
        };
        let payload = if !multi {
            // single datagram: the rest. (The client insists that a fixed length equals the rest; we are lenient.)
            let p = &area[i..];
            i = area.len();
            p
        } else {
            let n = match (len, prefixed) {
                (Len::Fixed(n), false) => n,
                (Len::E4, false) => usize::from(*area.get(i + 1).ok_or(ParseError::Truncated)?) + 2,
                _ => {
                    let mut n = usize::from(*area.get(i).ok_or(ParseError::Truncated)?);
                    i += 1;
                    if n == 0xFF {
                        let b = area.get(i..i + 2).ok_or(ParseError::Truncated)?;
                        n = usize::from(u16::from_le_bytes([b[0], b[1]]));
                        i += 2;
                    }
                    n
                }
            };
            let p = area.get(i..i + n).ok_or(ParseError::Truncated)?;
            i += n;
            p
        };
        messages.push(Message::Data(Data { id, x, part, payload: payload.to_vec() }));
    }
    Ok(Datagram { header, messages })
}

/// The message area of `messages`.
pub fn encode_area(direction: Direction, multi: bool, messages: &[Message]) -> Result<Vec<u8>, ParseError> {
    let mut out = Vec::new();
    for (k, m) in messages.iter().enumerate() {
        match m {
            Message::LowLevel(t) => {
                if !is_low_level(*t) {
                    return Err(ParseError::InvalidType(*t));
                }
                out.push(*t);
            }
            Message::Data(d) => {
                let (class, len) = table::spec(direction, d.id).ok_or(ParseError::InvalidType(d.id))?;
                if d.x.is_some() != class.has_x() {
                    return Err(ParseError::Unencodable("x byte does not match the id's class"));
                }
                if d.part.is_some() {
                    out.push(MULTIPART);
                }
                out.push(d.id);
                out.extend(d.x);
                if let Some(p) = d.part {
                    out.push(p.index);
                    if p.index == 0 {
                        out.push(p.count.ok_or(ParseError::Unencodable("part 0 needs a count"))?);
                    }
                }
                if multi {
                    let implicit = d.part.is_none() && !matches!(len, Len::Var);
                    if !implicit {
                        if d.payload.len() < 0xFF {
                            out.push(d.payload.len() as u8);
                        } else {
                            let n = u16::try_from(d.payload.len())
                                .map_err(|_| ParseError::Unencodable("payload over 65535 bytes"))?;
                            out.push(0xFF);
                            out.extend(n.to_le_bytes());
                        }
                    }
                } else if k + 1 != messages.len() {
                    return Err(ParseError::Unencodable("data message not last in a single datagram"));
                }
                out.extend(&d.payload);
            }
        }
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// FC keepalive after this long without a send (match connection: 100 ms, PROVEN 0x200123D25).
    pub idle: Duration,
    /// A received datagram is acked by a keepalive this long after the last send if no message carried the ack.
    pub ack_delay: Duration,
    pub tail: usize,
    pub part: usize,
    /// `is_gone` after this long without a peer datagram (server side: generous; the client's own value is 2000 ms).
    pub timeout: Duration,
    /// The most datagrams `poll` returns at once.
    pub burst: usize,
    /// No new (first-send) datagram while this many reliable datagrams are unacked (the ack mask is 256 wide).
    pub window: usize,
    /// `cfg` value of the table: match options `send_period`.
    pub send_period: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idle: Duration::from_millis(100),
            ack_delay: Duration::from_millis(20),
            tail: 4,
            part: PART,
            timeout: Duration::from_secs(10),
            burst: 64,
            window: 160,
            send_period: Duration::from_millis(100),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Message { id: u8, x: Option<u8>, payload: Vec<u8> },
    Disconnected,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    UnknownId(u8),
    TooLarge(usize),
}

/// Handle of a tracked message, see [`MatchTransport::send_tracked`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MsgKey(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Not all parts sent or acked yet.
    InFlight,
    /// Every part was acked (the client parsed it; the handler may still have dropped it by state).
    Acked,
    /// A lifetime ended before every part was acked.
    Expired,
}

struct Outgoing {
    key: u64,
    msg: Option<u64>,
    info: Info,
    body: Vec<u8>,
    seqs: Vec<u16>,
    first_sent: Instant,
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

struct Tracked {
    parts_left: usize,
    expired: bool,
}

pub struct MatchTransport {
    role: Role,
    config: Config,
    next_seq: u16,
    received: Received,
    unique_out: u32,
    order_out: u32,
    pending: VecDeque<(Vec<u8>, Info, Option<u64>)>,
    unacked: Vec<Outgoing>,
    seq_key: HashMap<u16, u64>,
    next_key: u64,
    next_msg: u64,
    tracked: HashMap<u64, Tracked>,
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

impl MatchTransport {
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
            next_msg: 0,
            tracked: HashMap::new(),
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
    pub fn resent(&self) -> u32 {
        self.resent
    }
    pub fn rejected(&self) -> u32 {
        self.rejected
    }
    pub fn unacked(&self) -> usize {
        self.unacked.len()
    }
    pub fn config(&self) -> &Config {
        &self.config
    }
    /// Change `cfg` (send_period) once the match options are known.
    pub fn set_send_period(&mut self, p: Duration) {
        self.config.send_period = p;
    }

    pub fn is_gone(&self, now: Instant) -> bool {
        self.gone || now.saturating_duration_since(self.last_received) >= self.config.timeout
    }

    /// Ask the peer to disconnect (low-level FA).
    pub fn disconnect(&mut self) {
        self.low_level_out.push_back(FA);
    }

    fn send_dir(&self) -> Direction {
        match self.role {
            Role::Server => Direction::ToClient,
            Role::Client => Direction::ToServer,
        }
    }
    fn recv_dir(&self) -> Direction {
        match self.role {
            Role::Server => Direction::ToServer,
            Role::Client => Direction::ToClient,
        }
    }

    pub fn send(&mut self, id: u8, payload: &[u8]) -> Result<Option<u8>, SendError> {
        self.send_inner(id, payload, false).map(|(x, _)| x)
    }

    /// Like [`send`](Self::send) and returns a key to ask [`delivery`](Self::delivery) later.
    pub fn send_tracked(&mut self, id: u8, payload: &[u8]) -> Result<MsgKey, SendError> {
        self.send_inner(id, payload, true).map(|(_, k)| k.expect("tracked"))
    }

    fn send_inner(&mut self, id: u8, payload: &[u8], track: bool) -> Result<(Option<u8>, Option<MsgKey>), SendError> {
        let dir = self.send_dir();
        let info = table::info(dir, id, self.config.send_period).ok_or(SendError::UnknownId(id))?;
        let head = 1 + usize::from(info.class.has_x());
        let split = head + payload.len() > self.config.part + 20;
        let chunks: Vec<&[u8]> = if split { payload.chunks(self.config.part).collect() } else { vec![payload] };
        if chunks.len() > 255 {
            return Err(SendError::TooLarge(payload.len()));
        }
        let x = match info.class {
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
        let msg = track.then(|| {
            let m = self.next_msg;
            self.next_msg += 1;
            self.tracked.insert(m, Tracked { parts_left: chunks.len(), expired: false });
            m
        });
        let count = chunks.len();
        for (i, c) in chunks.into_iter().enumerate() {
            let part = split.then_some(Part { index: i as u8, count: (i == 0).then_some(count as u8) });
            let d = Data { id, x, part, payload: c.to_vec() };
            let body = encode_area(dir, false, &[Message::Data(d)]).expect("table checked");
            self.pending.push_back((body, info, msg));
        }
        // a message that is never tracked as reliable (lifetime Once) counts as acked at once
        if let (Some(m), Life::Once) = (msg, info.life) {
            self.tracked.remove(&m);
        }
        Ok((x, msg.map(MsgKey)))
    }

    pub fn delivery(&self, key: MsgKey) -> Delivery {
        match self.tracked.get(&key.0) {
            None => Delivery::Acked,
            Some(t) if t.expired => Delivery::Expired,
            Some(t) if t.parts_left == 0 => Delivery::Acked,
            Some(_) => Delivery::InFlight,
        }
    }

    pub fn receive(&mut self, datagram: &[u8], now: Instant) -> Vec<Event> {
        let mut events = Vec::new();
        let dg = match parse(self.recv_dir(), datagram) {
            Ok(dg) => dg,
            Err(e) => {
                log::debug!("match datagram did not parse ({e}); dropped");
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
                Message::LowLevel(t) => match t {
                    FA => {
                        if self.low_level_out.len() < 8 {
                            self.low_level_out.push_back(FB);
                        }
                        if !self.gone {
                            self.gone = true;
                            events.push(Event::Disconnected);
                        }
                    }
                    FB | FD => {
                        if !self.gone {
                            self.gone = true;
                            events.push(Event::Disconnected);
                        }
                    }
                    _ => {}
                },
                Message::Data(d) => self.data(d, &mut events),
            }
        }
        events
    }

    fn data(&mut self, d: Data, events: &mut Vec<Event>) {
        let Some((class, _)) = table::spec(self.recv_dir(), d.id) else { return };
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
                if !a.count.is_some_and(|c| a.parts.len() >= usize::from(c)) {
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
            }
        }
    }

    fn finish(&mut self, o: &Outgoing, acked: bool) {
        for q in &o.seqs {
            if self.seq_key.get(q) == Some(&o.key) {
                self.seq_key.remove(q);
            }
        }
        if let Some(m) = o.msg {
            if let Some(t) = self.tracked.get_mut(&m) {
                if acked {
                    t.parts_left = t.parts_left.saturating_sub(1);
                } else {
                    t.expired = true;
                }
            }
        }
    }

    fn ack_seq(&mut self, s: u16) {
        if let Some(key) = self.seq_key.remove(&s) {
            if let Some(at) = self.unacked.iter().position(|o| o.key == key) {
                let o = self.unacked.remove(at);
                self.finish(&o, true);
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
        let d = ack::encode(&h, body).expect("tail <= 35");
        debug_assert!(d.len() <= MAX_DATAGRAM + 40);
        (s, d)
    }

    /// The datagrams to send now: low-level, due resends, new messages, a keepalive when idle or an ack is due.
    pub fn poll(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(t) = self.low_level_out.pop_front() {
            out.push(self.datagram(&[t], now).1);
        }
        // lifetimes first
        let mut i = 0;
        while i < self.unacked.len() {
            let expired = match self.unacked[i].info.life {
                Life::Until(d) => now.saturating_duration_since(self.unacked[i].first_sent) >= d,
                _ => false,
            };
            if expired {
                let o = self.unacked.remove(i);
                self.finish(&o, false);
            } else {
                i += 1;
            }
        }
        let due: Vec<usize> = (0..self.unacked.len())
            .filter(|i| now.saturating_duration_since(self.unacked[*i].last_sent) >= self.unacked[*i].info.resend)
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
        while out.len() < self.config.burst && self.unacked.len() < self.config.window {
            let Some((body, info, msg)) = self.pending.pop_front() else { break };
            let (s, d) = self.datagram(&body, now);
            if info.life != Life::Once {
                let key = self.next_key;
                self.next_key += 1;
                self.seq_key.insert(s, key);
                self.unacked.push(Outgoing { key, msg, info, body, seqs: vec![s], first_sent: now, last_sent: now });
            }
            out.push(d);
        }
        if out.is_empty() && !self.gone {
            let quiet = |pace: Duration| self.last_sent.is_none_or(|t| now.saturating_duration_since(t) >= pace);
            if (self.ack_due && quiet(self.config.ack_delay))
                || (self.received.newest().is_some() && quiet(self.config.idle))
            {
                out.push(self.datagram(&[FC], now).1);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        let s = s.as_str();
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    /// The client's first datagram: `00 FF 04 | C9 order session dbg 00`; the table length 10 gives
    /// 3 + 1 + 10 = 14 bytes.
    #[test]
    fn c9_datagram_parses() {
        let d = hex("00ff04c9 78563412 efbeadde 00 00");
        let dg = parse(Direction::ToServer, &d).unwrap();
        assert_eq!(dg.header.seq, 0);
        assert_eq!(dg.header.r, 0x1FF);
        let Message::Data(m) = &dg.messages[0] else { panic!() };
        assert_eq!((m.id, m.x, m.payload.len()), (0xC9, None, 10));
        assert_eq!(&m.payload[..4], &[0x78, 0x56, 0x34, 0x12]);
    }

    /// `S->C <hdr> | DC D4 04 D4 04` for port 1236.
    #[test]
    fn dc_datagram_round_trips() {
        let d = hex("0001 04 dc d404d404".replace(' ', "").as_str());
        let dg = parse(Direction::ToClient, &d).unwrap();
        let Message::Data(m) = &dg.messages[0] else { panic!() };
        assert_eq!((m.id, m.payload.clone()), (0xDC, hex("d404d404")));
        assert_eq!(ack::encode(&dg.header, &encode_area(Direction::ToClient, false, &dg.messages).unwrap()).unwrap(), d);
    }

    #[test]
    fn multi_datagram_with_fixed_and_var_ids() {
        // C->S multi: CA (fixed 4), CC (fixed 6), CD (var, len prefix), CE (fixed 0), FC
        let area = hex("ca01000000 cc 05 000000 00 08 cd 03 aabbcc ce fc".replace(' ', "").as_str());
        let h = Header::new(1, 0x1FF, true, &Default::default(), 0);
        let d = ack::encode(&h, &area).unwrap();
        let dg = parse(Direction::ToServer, &d).unwrap();
        let ids: Vec<u8> = dg
            .messages
            .iter()
            .map(|m| match m {
                Message::Data(d) => d.id,
                Message::LowLevel(t) => *t,
            })
            .collect();
        assert_eq!(ids, vec![0xCA, 0xCC, 0xCD, 0xCE, 0xFC]);
        // and the encoder writes the same bytes
        assert_eq!(encode_area(Direction::ToServer, true, &dg.messages).unwrap(), area);
    }

    #[test]
    fn e4_length_by_content() {
        // S->C multi: E4 x=0 kind=1 n=2 'ab', then FC
        let area = hex("e4 00 01 02 6162 fc".replace(' ', "").as_str());
        let h = Header::new(1, 0x1FF, true, &Default::default(), 0);
        let d = ack::encode(&h, &area).unwrap();
        let dg = parse(Direction::ToClient, &d).unwrap();
        assert_eq!(dg.messages.len(), 2);
        let Message::Data(m) = &dg.messages[0] else { panic!() };
        assert_eq!(m.payload, hex("01026162"));
    }

    #[test]
    fn lobby_ids_are_invalid_here() {
        let h = Header::new(0, 0x1FF, false, &Default::default(), 0);
        let d = ack::encode(&h, &[0x2E, 0, 1]).unwrap();
        assert!(matches!(parse(Direction::ToServer, &d), Err(ParseError::InvalidType(0x2E))));
    }

    fn pair() -> (MatchTransport, MatchTransport, Instant) {
        let t0 = Instant::now();
        (MatchTransport::new(Role::Client, t0), MatchTransport::new(Role::Server, t0), t0)
    }

    fn pump(from: &mut MatchTransport, to: &mut MatchTransport, now: Instant) -> Vec<Event> {
        let mut ev = Vec::new();
        for d in from.poll(now) {
            ev.extend(to.receive(&d, now));
        }
        ev
    }

    #[test]
    fn send_receive_and_tracking() {
        let (mut c, mut s, t0) = pair();
        c.send(0xC9, &[0; 10]).unwrap();
        let ev = pump(&mut c, &mut s, t0);
        assert_eq!(ev, vec![Event::Message { id: 0xC9, x: None, payload: vec![0; 10] }]);
        let k = s.send_tracked(0xDC, &[0xD4, 4, 0xD4, 4]).unwrap();
        assert_eq!(s.delivery(k), Delivery::InFlight);
        let ev = pump(&mut s, &mut c, t0);
        assert_eq!(ev, vec![Event::Message { id: 0xDC, x: None, payload: vec![0xD4, 4, 0xD4, 4] }]);
        // the client's ack
        pump(&mut c, &mut s, t0 + Duration::from_millis(30));
        assert_eq!(s.delivery(k), Delivery::Acked);
        // unique ids count from 0, E2 has its own (ordered) counter
        assert_eq!(s.send(0xDD, &[1]).unwrap(), Some(0));
        assert_eq!(s.send(0xDE, &[1]).unwrap(), Some(1));
        assert_eq!(s.send(0xE2, &[0; 8]).unwrap(), Some(0));
    }

    #[test]
    fn lifetime_ends_resends_and_marks_expired() {
        let (mut c, mut s, t0) = pair();
        c.send(0xC9, &[0; 10]).unwrap();
        pump(&mut c, &mut s, t0);
        let k = s.send_tracked(0xDE, &[7; 10]).unwrap();
        let first = s.poll(t0);
        assert_eq!(first.len(), 1);
        // lost: resent after 100 ms, and again; gone after 1900 ms
        assert_eq!(s.poll(t0 + Duration::from_millis(100)).iter().filter(|d| d[3] == 0xDE).count(), 1);
        assert_eq!(s.poll(t0 + Duration::from_millis(200)).iter().filter(|d| d[3] == 0xDE).count(), 1);
        assert!(s.resent() >= 2);
        let late = s.poll(t0 + Duration::from_millis(2000));
        assert!(late.iter().all(|d| d[3] != 0xDE));
        assert_eq!(s.delivery(k), Delivery::Expired);
        assert_eq!(s.unacked(), 0);
    }

    #[test]
    fn plain_once_is_not_kept() {
        let (mut c, mut s, t0) = pair();
        c.send(0xC9, &[0; 10]).unwrap();
        pump(&mut c, &mut s, t0);
        s.send(0xE5, &[1, 2, 3]).unwrap();
        assert_eq!(s.poll(t0).len(), 1);
        assert_eq!(s.unacked(), 0);
    }

    #[test]
    fn multipart_de_round_trip() {
        let (mut c, mut s, t0) = pair();
        c.send(0xC9, &[0; 10]).unwrap();
        pump(&mut c, &mut s, t0);
        let payload: Vec<u8> = (0..5000u32).map(|i| (i * 7) as u8).collect();
        s.send(0xDE, &payload).unwrap();
        let ds = s.poll(t0);
        assert!(ds.len() > 10);
        let mut ev = Vec::new();
        for d in ds.iter().rev() {
            ev.extend(c.receive(d, t0));
        }
        assert_eq!(ev, vec![Event::Message { id: 0xDE, x: Some(0), payload }]);
    }

    #[test]
    fn keepalive_after_100_ms() {
        let (mut c, mut s, t0) = pair();
        c.send(0xC9, &[0; 10]).unwrap();
        pump(&mut c, &mut s, t0);
        let _ = s.poll(t0 + Duration::from_millis(30)); // ack keepalive
        assert!(s.poll(t0 + Duration::from_millis(60)).is_empty());
        let ka = s.poll(t0 + Duration::from_millis(160));
        assert_eq!(ka.len(), 1);
        assert_eq!(ka[0][3], 0xFC);
    }
}
