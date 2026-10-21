//! One client's connection: the server's half of the 0.26g client's
//! `udp_match_connection` (`docs/notes/transport.md`), for a
//! match or for the lobby.
//!
//! One `Transport` per client. It owns no socket: `receive` takes a datagram
//! from the client and `poll` returns the datagrams to send it, so a server
//! keeps one UDP socket and a map of peers.
//!
//! - Every datagram is `seq rseq ack0 <messages> [tail acks]` (§2). The
//!   server's seq starts at 0 (the client's "nothing yet" is 0xFF). `rseq`
//!   is the newest client seq received, so nothing is sent before the
//!   client's first datagram: the client drops a datagram that echoes a seq
//!   it has not sent, and one without a message. A pure ack or keep-alive is
//!   the low-level message `FC`.
//! - A lone message goes in the single format, several in the multi format;
//!   only variable-length ids get a length there (§3.1). At most 252
//!   messages and 508 bytes.
//! - Unique ids take the next unique id, ordered ids the next order id: two
//!   separate one-byte counters from 0 (client `enqueue_impl` 0x3FCE30). A
//!   message too large for a datagram goes as `FF` parts sharing one id
//!   (0x3FD060).
//! - The client's windows are 128 wide. A unique or ordered message waits
//!   here while its id would be 100 or more past the oldest one not yet
//!   acked: past 127 the client takes a resent unique id for a new one, and
//!   drops an ordered message as "not newer than the last delivered" while
//!   still acking its datagram, which stops the ordered channel for good
//!   (0x3F483C, 0x3F4A7C). An id counts as unacked for half a second more,
//!   while a late copy of it may still be on its way.
//! - Reliable messages are resent under new seqs until a seq they went out
//!   under is acked, or until their lifetime ends (§4). One skipped over by
//!   the client's acks goes again at once.
//! - The parts of a split message leave a few at a time (`Config::bulk_burst`,
//!   `bulk_pace`), first sends and resends alike: thirty full datagrams in
//!   one millisecond are more than some lines carry (live, 2026-10-13: a
//!   client that reported 314 of 325 such datagrams missing and every small
//!   one around them received). The client takes parts in any order and
//!   with no time limit (0x3F4DA0). What was queued behind a split message
//!   stays behind it until each part has gone once; what was queued before
//!   it, and anything once its parts only go again, does not wait.
//! - Short-lived messages (the table's 250 ms ids: relays, roster changes,
//!   drop orders) wait up to `Config::gather` after the last datagram with
//!   messages, so that what comes meanwhile shares one datagram; anything
//!   else that is queued takes them along at once (§11.1). So do those ids
//!   kept until acked (`send_kept`: a resync's replay, the relays to a
//!   resynced game): a burst of them is packed, never one per datagram.
//!   Only what is not of those ids (E5, E6, E7, a world's parts) flushes
//!   the queue at once.
//! - Two messages sent joined (`send_joined`) go in one datagram, in their
//!   order: a 0xE7 that ends a clock correction ahead of a 0xE5.
//! - A client that took a late datagram of ours from the lap before as its
//!   newest acks that seq from then on, which reads here as one we have
//!   not sent yet. Our seq then skips ahead to it, and the client takes the
//!   next one as newer by one (§11.2).
//! - From the client: plain messages are not deduplicated by the transport
//!   (the client's own has none either), so a resent D8, D9, DA, DE or E0
//!   reaches the caller again and the caller must dedup them. Unique ones
//!   (every lobby request) are delivered once, through a 128-wide window.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::ack;
use crate::packet_log::ConnectionLog;
use crate::seq::{Recorded, Window, is_newer};
use crate::table::{Class, Direction, Length, Lifetime, Table, mask_bytes, player_inputs_len};
use crate::wire::{self, HEADER, MAX_DATAGRAM, MAX_MESSAGES, MULTIPART, Part, low_level};

/// Tail ack bytes always left room for when a message is sized: a datagram
/// with a full message still acks 34 client seqs.
const TAIL_RESERVE: usize = 4;
/// The most a message's header and payload may take in a datagram.
const MAX_BODY: usize = MAX_DATAGRAM - HEADER - TAIL_RESERVE;
/// Data in part 0 (`FF id x 0 count`) and in a later part (`FF id x part`).
/// The client's own parts are 500 and 501 bytes less its tail acks
/// (0x3FD2B5, 0x3FD388); it accepts any size.
const FIRST_PART_DATA: usize = MAX_BODY - 5;
const PART_DATA: usize = MAX_BODY - 4;
/// The client stops reading acks 127 seqs back; data waits when this many
/// datagrams are past the newest acked one.
const MAX_AHEAD: u64 = 100;
/// How far a unique or order id may run ahead of the oldest unacked one.
const CHANNEL_WINDOW: u64 = 100;
/// The datagrams a message went in that are remembered, the newest: an ack
/// of any of them acks it. The client keeps the last 8 for its own messages
/// (pkt+0x5C ring), but here a message sent again at the table's 50 ms
/// over a round trip of 400 ms and more was never acked with 8: the acks
/// that came named copies already forgotten (a resync's replay to a game
/// 500 ms away was redone for ever). 32 cover 1.6 s of such copies.
const SEQS_PER_MESSAGE: usize = 32;
/// A message whose datagram is unacked while one sent this many later was
/// acked is taken as lost and goes again at once.
const FAST_RESEND_GAP: u64 = 3;
/// How many sent datagrams are remembered, to time their acks and to learn
/// which of our acks the client has seen.
const SENT_KEPT: usize = 256;
/// Client seqs this far behind the ones the client is known to have had
/// acked are still reported: the client ignores the acks of a datagram whose
/// `rseq` it has already seen (§2.4, `r == A`), so a late arrival's ack has
/// to ride along with a newer `rseq`.
const ACK_MARGIN: u64 = 14;
/// The keep-alive pace towards a client that acks nothing (it is loading).
const SLOW_IDLE: Duration = Duration::from_secs(1);
/// The pace of `FA` while the server disconnects (the client's own idle).
const CLOSING_PACE: Duration = Duration::from_millis(100);
/// The most half-received split messages kept per channel.
const MAX_ASSEMBLIES: usize = 8;
/// How many lost serials are remembered for `is_lost`: a stall of a few
/// seconds can end the lives of a few hundred relays before their sender
/// looks.
const LOST_KEPT: usize = 4096;

/// The server's timers for one connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// A keep-alive goes when nothing was sent for this long.
    pub idle: Duration,
    /// A client datagram is acked this long after the last send at the
    /// latest (by a keep-alive, if no message carries the ack sooner).
    pub ack_delay: Duration,
    /// How long a peer that sends nothing stays connected (`is_gone`).
    pub timeout: Duration,
    /// How long a disconnect waits for the client's confirm.
    pub closing: Duration,
    /// The most datagrams with messages sent at once, and
    pub burst: u32,
    /// the time that earns one more. The client's seq window is 128 wide:
    /// it takes a datagram that arrives 128 or more seqs late (a network
    /// duplicate, say) as new and delivers its plain messages again
    /// (§2.3 step 6). At 48 + one a millisecond, 128 seqs take 80 ms.
    pub pace: Duration,
    /// How long after its ack (or its one send, or its expiry) a unique or
    /// order id still holds the channel window back: a copy of it resent
    /// before the ack came, or duplicated by the network, may arrive that
    /// much later, and must then still be inside the client's window.
    pub guard: Duration,
    /// The most parts of split messages sent at once, and
    pub bulk_burst: u32,
    /// the time that earns one more: a part fills a datagram, so this is
    /// the rate of the large ones. `u32::MAX` parts at once is no limit.
    pub bulk_pace: Duration,
    /// How long a short-lived message (one the table gives up after a
    /// lifetime, sent without an id: the match's relays, roster changes
    /// and drop orders) may wait after the last datagram with messages for
    /// more to share its datagram. Anything else queued, a full datagram's
    /// worth, or an ack due takes it along at once. Zero: every message
    /// goes at once.
    pub gather: Duration,
}

impl Config {
    /// The defaults: the client gives up after 5 s without a datagram on
    /// both kinds of connection (0x27A127, 0x26F0ED) and sends a keep-alive
    /// after 100 ms (match) or 500 ms (lobby) of silence.
    pub fn for_table(table: Table) -> Self {
        Self {
            idle: match table {
                Table::Match => Duration::from_millis(100),
                Table::Lobby => Duration::from_millis(250),
            },
            ack_delay: Duration::from_millis(20),
            timeout: Duration::from_secs(10),
            closing: Duration::from_secs(1),
            burst: 48,
            pace: Duration::from_millis(1),
            guard: Duration::from_millis(500),
            // A match's large messages are the static info and the world:
            // 4 parts at once and one more every 3 ms is 1.35 Mbit/s, so a
            // world of 11 parts is out in 21 ms and one of 31 in 81 ms
            // (to the 5 ms of the server's turn).
            // The lobby's sign-in is left as it was: its messages live
            // until they are acked, so a thin line only makes it slow.
            bulk_burst: match table {
                Table::Match => 4,
                Table::Lobby => u32::MAX,
            },
            bulk_pace: Duration::from_millis(3),
            // A match's relays went one per datagram, 100 to 220 datagrams
            // a second to each client (live, 2026-10-14): the seq lapped
            // every 1.2 to 2.5 s and the client's acks named fewer of them.
            // Some 15 ms is a fraction of the 250 ms a relay has before its
            // tick is final. The lobby has no short-lived ids.
            gather: match table {
                Table::Match => Duration::from_millis(15),
                Table::Lobby => Duration::ZERO,
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// A client message without its id and x byte. A split message arrives
    /// whole.
    Message { id: u8, payload: Vec<u8> },
    /// The client left, or confirmed the server's disconnect.
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Connected,
    /// The server disconnects and waits for the confirm.
    Closing {
        since: Instant,
    },
    Closed,
}

/// Why `send` refused a message.
#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    /// Not an id the client reads on this connection (outside the table's
    /// range), or one whose table entry is null (lobby 0x86, 0xA7).
    UnknownId(u8),
    /// An id that must never reach the client (lobby 0x81, 0xA0).
    Forbidden(u8),
    /// The payload does not have the size the client expects for the id.
    BadLength {
        id: u8,
        expected: usize,
        got: usize,
    },
    /// ED/EE before `set_roster_size`, or with fewer than two players set.
    BadPlayerInputs(u8),
    /// Too large for one datagram and not splittable, or more than 255
    /// parts.
    TooLarge(usize),
    NotConnected,
}

/// The channel a message's x byte counts on, with the full count (the wire
/// byte is its low 8 bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    None,
    Unique(u64),
    Ordered(u64),
}

struct Outgoing {
    /// Which `send` it came from: the parts of one message share it.
    serial: u64,
    /// `id [x]`, or `FF id x part [count]`.
    head: Vec<u8>,
    payload: Vec<u8>,
    /// A variable-length id: length-prefixed in a multi datagram.
    variable: bool,
    channel: Channel,
    lifetime: Lifetime,
    resend: Duration,
    /// Set at the first send of a message with a limited lifetime; or when
    /// it is queued with a deadline (`send_until`), and then moved on to
    /// the table's lifetime from its first send if that is later.
    expires: Option<Instant>,
    /// It may wait for others to share its datagram (`Config::gather`).
    gathers: bool,
    /// It goes in one datagram with the message queued right after it
    /// ([`Transport::send_joined`]).
    joined: bool,
    /// The datagrams it went out in (their running numbers), newest last.
    sent_in: VecDeque<u64>,
}

impl Outgoing {
    /// A part of a split message: it fills a datagram.
    fn is_part(&self) -> bool {
        self.head.first() == Some(&MULTIPART)
    }

    fn single_len(&self) -> usize {
        self.head.len() + self.payload.len()
    }

    fn multi_len(&self) -> usize {
        let prefix = if self.variable {
            wire::prefix_len(self.payload.len())
        } else {
            0
        };
        self.single_len() + prefix
    }

    fn write(&self, body: &mut Vec<u8>, multi: bool) {
        body.extend(&self.head);
        if multi && self.variable {
            wire::push_prefix(body, self.payload.len());
        }
        body.extend(&self.payload);
    }
}

/// A datagram sent, remembered until it falls out of the ring.
struct Sent {
    number: u64,
    at: Instant,
    /// Whether its ack already gave a round trip.
    timed: bool,
    /// The client seq (running number) up to which this datagram's acks
    /// were complete; `None` if its tail had to be cut short.
    floor: Option<u64>,
}

/// A split message being received.
struct Assembly {
    id: u8,
    count: Option<u8>,
    parts: BTreeMap<u8, Vec<u8>>,
}

impl Assembly {
    /// Add a part; the whole payload once every part is there (the client
    /// compares the number of parts with part 0's count, 0x3F4D64).
    fn add(&mut self, part: Part, data: &[u8]) -> Option<Vec<u8>> {
        if part.index == 0 {
            self.count = part.count;
        }
        self.parts
            .entry(part.index)
            .or_insert_with(|| data.to_vec());
        let count = usize::from(self.count?);
        (self.parts.len() >= count).then(|| self.parts.values().flatten().copied().collect())
    }
}

pub struct Transport {
    table: Table,
    config: Config,
    /// Datagrams sent so far; the next one's seq is its low byte.
    sent_total: u64,
    /// The client seqs received.
    received: Window,
    /// The running number of the newest client seq (it counts the wraps).
    received_number: u64,
    /// The client is known to have every ack up to this client seq number.
    ack_floor: Option<u64>,
    /// A client datagram arrived since the last send.
    ack_due: bool,
    /// The newest server datagram the client acked.
    newest_acked: Option<u64>,
    next_unique: u64,
    next_order: u64,
    next_serial: u64,
    pending: VecDeque<Outgoing>,
    unacked: Vec<(Outgoing, Instant)>,
    /// How many entries of `pending` and `unacked` each serial has (the
    /// parts of a split message one each): [`Self::is_delivered`] in O(1),
    /// not a walk of the queue (a resync's gate and `check_in_flight` ask
    /// it for hundreds of serials a poll).
    outstanding: HashMap<u64, u32>,
    low_level: VecDeque<u8>,
    last_sent: Option<Instant>,
    /// When the last datagram with messages went (`Config::gather`).
    last_data: Option<Instant>,
    last_received: Instant,
    state: State,
    /// The ED/EE player mask size, from the roster.
    mask_bytes: usize,
    /// Serials of messages dropped at the end of their lifetime, the
    /// newest [`LOST_KEPT`].
    lost: BTreeSet<u64>,
    /// How often our seq skipped ahead to the one the client acks.
    skips: u32,
    /// Client unique ids delivered.
    unique_in: Window,
    unique_parts: BTreeMap<u8, Assembly>,
    /// The last client order id delivered (0xFF before the first).
    order_in: u8,
    order_waiting: BTreeMap<u8, (u8, Vec<u8>)>,
    order_parts: BTreeMap<u8, Assembly>,
    /// Messages sent again, after their resend time or as lost.
    resent: u32,
    /// Client datagrams that did not parse.
    rejected: u32,
    sent: VecDeque<Sent>,
    /// The round trips measured since `take_round_trips`, in ms.
    rtt_sum: u64,
    rtt_count: u32,
    rtt_max: u32,
    /// The latest round trip measured, in ms.
    rtt_last: Option<u32>,
    /// Datagrams with messages that may go now, and when that was counted.
    budget: u32,
    budget_at: Instant,
    /// The same for the parts of split messages.
    bulk_budget: u32,
    bulk_at: Instant,
    /// Unique and order ids done with (acked, sent once, expired), and
    /// when: they hold the channel windows back for `guard`.
    retired: VecDeque<(Channel, Instant)>,
    log: Option<ConnectionLog>,
}

impl Transport {
    pub fn new(table: Table, now: Instant) -> Self {
        Self::with_config(table, Config::for_table(table), now)
    }

    pub fn with_config(table: Table, config: Config, now: Instant) -> Self {
        Self {
            table,
            config,
            sent_total: 0,
            received: Window::default(),
            received_number: 0,
            ack_floor: None,
            ack_due: false,
            newest_acked: None,
            next_unique: 0,
            next_order: 0,
            next_serial: 0,
            pending: VecDeque::new(),
            unacked: Vec::new(),
            outstanding: HashMap::new(),
            low_level: VecDeque::new(),
            last_sent: None,
            last_data: None,
            last_received: now,
            state: State::Connected,
            mask_bytes: 0,
            lost: BTreeSet::new(),
            skips: 0,
            unique_in: Window::default(),
            unique_parts: BTreeMap::new(),
            order_in: 0xFF,
            order_waiting: BTreeMap::new(),
            order_parts: BTreeMap::new(),
            resent: 0,
            rejected: 0,
            sent: VecDeque::new(),
            rtt_sum: 0,
            rtt_count: 0,
            rtt_max: 0,
            rtt_last: None,
            budget: config.burst,
            budget_at: now,
            bulk_budget: config.bulk_burst,
            bulk_at: now,
            retired: VecDeque::new(),
            log: None,
        }
    }

    pub fn table(&self) -> Table {
        self.table
    }

    /// Write every datagram of this connection to `log` from now on.
    pub fn set_log(&mut self, log: ConnectionLog) {
        self.log = Some(log);
    }

    /// The match's roster size, once the client knows it (it learns it from
    /// the static info, E4). It gives the player mask size of ED/EE, which
    /// cannot be sent before.
    pub fn set_roster_size(&mut self, players: u8) {
        self.mask_bytes = mask_bytes(players);
    }

    /// How many messages went again: a count of lost or late datagrams.
    pub fn resent(&self) -> u32 {
        self.resent
    }

    /// How many client datagrams were dropped because they did not parse.
    pub fn rejected(&self) -> u32 {
        self.rejected
    }

    /// How often our seq skipped ahead to the one the client acks: each
    /// time, the client had taken a late datagram of ours from the lap
    /// before as its newest (§11.2).
    pub fn skips(&self) -> u32 {
        self.skips
    }

    /// The round trips measured since the last call, as the mean and the
    /// longest in ms: from a datagram's send to the ack that names it, the
    /// client's own delay in acking included. `None` if there were none.
    pub fn take_round_trips(&mut self) -> Option<(u32, u32)> {
        let count = std::mem::take(&mut self.rtt_count);
        let sum = std::mem::take(&mut self.rtt_sum);
        let max = std::mem::take(&mut self.rtt_max);
        (count > 0).then(|| ((sum / u64::from(count)) as u32, max))
    }

    /// The latest round trip measured (see [`Self::take_round_trips`]), in ms.
    pub fn last_round_trip(&self) -> Option<u32> {
        self.rtt_last
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// The client sent nothing for the configured timeout, or a disconnect
    /// ended.
    pub fn is_gone(&self, now: Instant) -> bool {
        self.is_gone_after(now, self.config.timeout)
    }

    /// The same, for a client allowed to be silent for `timeout`: one that is
    /// loading its level may send and ack nothing until it is done.
    pub fn is_gone_after(&self, now: Instant, timeout: Duration) -> bool {
        self.state == State::Closed || now.duration_since(self.last_received) >= timeout
    }

    /// Reliable messages not yet acked, and messages not yet sent.
    pub fn backlog(&self) -> usize {
        self.pending.len() + self.unacked.len()
    }

    /// Handle one datagram from the client.
    pub fn receive(&mut self, datagram: &[u8], now: Instant) -> Vec<Event> {
        if let Some(log) = &mut self.log {
            log.datagram(Direction::ToServer, datagram, 0);
        }
        let mut events = Vec::new();
        let parsed = match wire::parse(self.table, Direction::ToServer, datagram, 0) {
            Ok(parsed) => parsed,
            Err(error) => {
                // As the client does with one of ours: dropped whole, its
                // seq not acked, so the client sends its messages again.
                log::debug!("A client datagram did not parse ({error:?}); dropped");
                self.rejected += 1;
                return events;
            }
        };
        self.last_received = now;
        self.skip_to_echo(parsed.seq, parsed.rseq);
        self.acknowledge(parsed.rseq, parsed.acks.bits, parsed.acks.count, now);
        let before = self.received.newest();
        match self.received.record(parsed.seq) {
            Recorded::New => {
                if let (Some(before), Some(newest)) = (before, self.received.newest()) {
                    self.received_number += u64::from(newest.wrapping_sub(before));
                }
                self.ack_due = true;
            }
            Recorded::Duplicate => {
                // Our ack of it may have been lost: ack again.
                self.ack_due = true;
                return events;
            }
            Recorded::Outside => return events,
        }
        for message in in_send_order(&parsed.messages) {
            if message.low_level {
                self.low_level_message(message.id, &mut events);
                continue;
            }
            if self.state == State::Closed {
                continue;
            }
            let (id, payload) = (message.id, message.payload);
            match (message.class, message.x, message.part) {
                (Class::Plain, _, _) => events.push(Event::Message {
                    id,
                    payload: payload.to_vec(),
                }),
                (Class::Unique, Some(x), None) => {
                    if self.unique_in.record(x) != Recorded::Duplicate {
                        events.push(Event::Message {
                            id,
                            payload: payload.to_vec(),
                        });
                    }
                }
                (Class::Unique, Some(x), Some(part)) => {
                    if self.unique_in.contains(x) {
                        continue;
                    }
                    let whole = add_part(&mut self.unique_parts, x, id, part, payload);
                    if let Some(payload) = whole {
                        self.unique_in.record(x);
                        events.push(Event::Message { id, payload });
                    }
                }
                (Class::Ordered, Some(order), part) => {
                    if !is_newer(order, self.order_in) || self.order_waiting.contains_key(&order) {
                        continue;
                    }
                    let whole = match part {
                        None => Some(payload.to_vec()),
                        Some(part) => add_part(&mut self.order_parts, order, id, part, payload),
                    };
                    if let Some(payload) = whole {
                        self.order_waiting.insert(order, (id, payload));
                        self.deliver_in_order(&mut events);
                    }
                }
                // The parser gives every unique and ordered message its x.
                (Class::Unique | Class::Ordered, None, _) => {}
            }
        }
        events
    }

    /// Deliver the waiting ordered messages that follow the last delivered
    /// one (the client's loop at 0x3F51F7).
    fn deliver_in_order(&mut self, events: &mut Vec<Event>) {
        loop {
            let next = self.order_in.wrapping_add(1);
            let Some((id, payload)) = self.order_waiting.remove(&next) else {
                break;
            };
            self.order_in = next;
            events.push(Event::Message { id, payload });
        }
    }

    /// The client echoes a seq of ours 1 to 128 past our newest: one of
    /// the lap before, which reached it so late that it took it for newer
    /// than everything (§2.3 step 6: 1 to 127 ahead of its newest, or 128
    /// ahead and the lower number; §11.2). It acks that seq from then on,
    /// which would
    /// read here as a seq not sent yet, so no ack would count and data
    /// would stop at `MAX_AHEAD` (live, 2026-10-14: 1.56 s). Our seq skips
    /// ahead to it instead: the client takes the next one as newer by one
    /// (nothing in it checks for gaps), and its acks name our datagrams by
    /// the same numbers again, since its window still holds the ones it
    /// had before the late one, at the distance the skip gives them.
    ///
    /// Only for the client's newest datagram (a late one carries an old
    /// `rseq`), only for a seq we did send a lap before, and only when that
    /// one is older than the newest it acked: a client that has had
    /// nothing from us for 128 of our datagrams echoes its true newest,
    /// and its window then holds older datagrams than the numbers a skip
    /// would give them.
    fn skip_to_echo(&mut self, seq: u8, rseq: u8) {
        if self.sent_total == 0
            || self
                .received
                .newest()
                .is_some_and(|newest| !is_newer(seq, newest))
        {
            return;
        }
        let ahead = u64::from(rseq.wrapping_sub(self.sent_total.wrapping_sub(1) as u8));
        if !(1..=128).contains(&ahead) || self.sent_total + ahead < 257 {
            return;
        }
        let echoed = self.sent_total - 1 + ahead - 256;
        if self.newest_acked.is_none_or(|acked| echoed >= acked) {
            return;
        }
        self.sent_total += ahead;
        self.skips += 1;
        log::debug!(
            "The client echoes seq {rseq}, of our datagram {echoed}: our seq skips {ahead} ahead"
        );
        if let Some(log) = &mut self.log {
            log.note(&format!("seq skips {ahead} ahead to {rseq}"));
        }
    }

    /// The client's acks: `rseq` and every older seq its bits name, as
    /// datagram numbers.
    fn acknowledge(&mut self, rseq: u8, bits: u128, count: u32, now: Instant) {
        if self.sent_total == 0 {
            return;
        }
        // An ack for a seq not sent (0xFF before any, or a stray value).
        let behind = u64::from((self.sent_total.wrapping_sub(1) as u8).wrapping_sub(rseq));
        if behind >= 128 || behind >= self.sent_total {
            return;
        }
        let newest = self.sent_total - 1 - behind;
        let acked = |number: u64| {
            let back = newest.wrapping_sub(number);
            number <= newest && back < u64::from(count) && bits >> (127 - back) & 1 != 0
        };
        // The ack's own seq is the datagram the client had just received:
        // the older ones named by its bits were acked later than they came.
        for sent in &mut self.sent {
            if !acked(sent.number) {
                continue;
            }
            if sent.number == newest && !sent.timed {
                sent.timed = true;
                let ms = now.saturating_duration_since(sent.at).as_millis() as u32;
                self.rtt_sum += u64::from(ms);
                self.rtt_count += 1;
                self.rtt_max = self.rtt_max.max(ms);
                self.rtt_last = Some(ms);
            }
            if let Some(floor) = sent.floor.take()
                && self.ack_floor.is_none_or(|known| floor > known)
            {
                self.ack_floor = Some(floor);
            }
        }
        let retired = &mut self.retired;
        let outstanding = &mut self.outstanding;
        let mut waits = |message: &Outgoing| {
            let waits = !message.sent_in.iter().any(|number| acked(*number));
            if !waits {
                if message.channel != Channel::None {
                    retired.push_back((message.channel, now));
                }
                settle(outstanding, message.serial);
            }
            waits
        };
        self.unacked.retain(|(message, _)| waits(message));
        // One waiting to go again is acked just the same.
        self.pending.retain(|message| waits(message));
        if self.newest_acked.is_none_or(|known| newest > known) {
            self.newest_acked = Some(newest);
        }
        // What this ack reports missing (inside its window, not set) while
        // a datagram sent some later was received was probably lost: send
        // it again now. Bits beyond the window say nothing. The later one
        // is one we sent: after a skip (`skip_to_echo`) `newest` is a
        // number no datagram had.
        let Some(received) = self
            .sent
            .iter()
            .rev()
            .map(|sent| sent.number)
            .find(|number| acked(*number))
        else {
            return;
        };
        for (message, at) in &mut self.unacked {
            let skipped = message.sent_in.back().is_some_and(|sent| {
                *sent + FAST_RESEND_GAP <= received && newest - *sent < u64::from(count)
            });
            if skipped && *at > now {
                *at = now;
            }
        }
    }

    fn low_level_message(&mut self, id: u8, events: &mut Vec<Event>) {
        match id {
            low_level::INITIATE_DISCONNECTION => {
                // The client asks until it hears the confirm.
                if self.low_level.len() < 8 {
                    self.low_level.push_back(low_level::CONFIRM_DISCONNECTION);
                }
                if self.state != State::Closed {
                    self.close();
                    events.push(Event::Disconnected);
                }
            }
            low_level::CONFIRM_DISCONNECTION => {
                if matches!(self.state, State::Closing { .. }) {
                    self.close();
                    events.push(Event::Disconnected);
                }
            }
            low_level::CONTINUOUS_FLOW => {}
            low_level::INSTANT_DISCONNECT => {
                if self.state != State::Closed {
                    self.close();
                    events.push(Event::Disconnected);
                }
            }
            _ => log::debug!("A low-level message {id:#04x}; ignored"),
        }
    }

    fn close(&mut self) {
        self.state = State::Closed;
        self.pending.clear();
        self.unacked.clear();
        self.outstanding.clear();
        if let Some(log) = &mut self.log {
            log.note("closed");
        }
    }

    /// Queue a server message. A unique id takes the next unique id, an
    /// ordered one the next order id; one too large for a datagram goes in
    /// parts. Returns its serial for [`Self::is_delivered`].
    ///
    /// Refused, never put on the wire: an id the client's table does not
    /// have or that crashes it, and a payload whose size the client would
    /// not accept (it drops the whole datagram, or loses its place in a
    /// multi datagram).
    pub fn send(&mut self, id: u8, payload: &[u8]) -> Result<u64, SendError> {
        self.send_for(id, payload, None, None, None)
    }

    /// As [`Self::send`], but a message the table would give up after its
    /// lifetime is resent until it is acked: for what must not be lost,
    /// however late (the replay of a sync). The lifetime is the sender's
    /// own matter; the client takes a late copy as any other. A send-once
    /// id is still sent once.
    pub fn send_kept(&mut self, id: u8, payload: &[u8]) -> Result<u64, SendError> {
        self.send_for(id, payload, Some(Lifetime::Forever), None, None)
    }

    /// As [`Self::send_kept`], sent again every `resend` while it is not
    /// acked, not at the table's pace: for a client that is in no hurry
    /// for it and acks slowly (one that waits for its world sends a pure
    /// ack every 100 ms and nothing else).
    pub fn send_kept_every(
        &mut self,
        id: u8,
        payload: &[u8],
        resend: Duration,
    ) -> Result<u64, SendError> {
        self.send_for(id, payload, Some(Lifetime::Forever), Some(resend), None)
    }

    /// As [`Self::send`], but a message the table would give up after its
    /// lifetime is given up after `lifetime` instead. A message with no
    /// lifetime of that kind (sent once, or kept until acked) goes as the
    /// table has it.
    pub fn send_lasting(
        &mut self,
        id: u8,
        payload: &[u8],
        lifetime: Duration,
    ) -> Result<u64, SendError> {
        self.send_for(id, payload, Some(Lifetime::For(lifetime)), None, None)
    }

    /// As [`Self::send`], but a message the table would give up after its
    /// lifetime is given up at `deadline` if that is later, and if it has
    /// not gone out by then, at `deadline` in any case. Until then it goes
    /// again at the table's pace. For what is of use to the client only
    /// until a moment (a relay, until the game's history has passed its
    /// tick): one held back by a stall is lost at that moment rather than
    /// sent late, and one that goes in time lives at least the table's
    /// lifetime from its first send, however near the deadline was. A
    /// message with no lifetime of that kind goes as the table has it.
    pub fn send_until(
        &mut self,
        id: u8,
        payload: &[u8],
        deadline: Instant,
    ) -> Result<u64, SendError> {
        self.send_for(id, payload, None, None, Some(deadline))
    }

    /// As [`Self::send`] (the table's lifetime), sent again every `resend`
    /// while it is not acked, not at the table's pace: for the parts of a
    /// world a game that plays waits for (0xE8 goes again every 500 ms by
    /// the table, which a burst of lost parts makes seconds).
    pub fn send_every(
        &mut self,
        id: u8,
        payload: &[u8],
        resend: Duration,
    ) -> Result<u64, SendError> {
        self.send_for(id, payload, None, Some(resend), None)
    }

    /// A unique id the table sends once (0xE7), sent again every `resend`
    /// until acked: the client takes a unique id once and drops a copy of
    /// it while it is in its 128-wide window (transport.md: 0x3F47AA), so
    /// only one copy is acted on. The caller sends no newer one of the kind
    /// before this is acked, or the late first copy of this one could
    /// arrive after it.
    pub fn send_unique_kept(
        &mut self,
        id: u8,
        payload: &[u8],
        resend: Duration,
    ) -> Result<u64, SendError> {
        let info = self
            .table
            .info(Direction::ToClient, id)
            .ok_or(SendError::UnknownId(id))?;
        if info.class != Class::Unique {
            return self.send_for(id, payload, None, Some(resend), None);
        }
        let serial = self.send_for(id, payload, None, Some(resend), None)?;
        for message in self
            .pending
            .iter_mut()
            .filter(|message| message.serial == serial)
        {
            message.lifetime = Lifetime::Forever;
        }
        Ok(serial)
    }

    /// Stop sending a message that is not acked yet (one kept by
    /// [`Self::send_unique_kept`] that something newer makes wrong). A copy
    /// on its way may still arrive. It counts as delivered from then on,
    /// never as lost.
    pub fn forget(&mut self, serial: u64, now: Instant) {
        let mut channels = Vec::new();
        self.pending.retain(|message| {
            let keep = message.serial != serial;
            if !keep {
                channels.push(message.channel);
            }
            keep
        });
        self.unacked.retain(|(message, _)| {
            let keep = message.serial != serial;
            if !keep {
                channels.push(message.channel);
            }
            keep
        });
        channels.dedup();
        for channel in channels {
            if channel != Channel::None {
                self.retired.push_back((channel, now));
            }
        }
        self.outstanding.remove(&serial);
    }

    /// Two whole messages that go in one datagram, `first` ahead of
    /// `second`: the client handles the messages of a datagram in order,
    /// and a datagram lost loses both. For a 0xE7 that ends a clock
    /// correction ahead of a 0xE5 (match.md section 29): alone after the
    /// 0xE5 it would be wrong. Neither may be split; if either cannot be
    /// sent, neither is.
    pub fn send_joined(
        &mut self,
        first: (u8, &[u8]),
        second: (u8, &[u8]),
    ) -> Result<(u64, u64), SendError> {
        for (id, payload) in [first, second] {
            if 2 + payload.len() > MAX_BODY {
                return Err(SendError::TooLarge(payload.len()));
            }
            if self.table.info(Direction::ToClient, id).is_none() {
                return Err(SendError::UnknownId(id));
            }
        }
        let a = self.send_for(first.0, first.1, None, None, None)?;
        let b = match self.send_for(second.0, second.1, None, None, None) {
            Ok(serial) => serial,
            Err(error) => {
                self.pending.retain(|message| message.serial != a);
                self.outstanding.remove(&a);
                return Err(error);
            }
        };
        if let Some(message) = self
            .pending
            .iter_mut()
            .rev()
            .find(|message| message.serial == a)
        {
            message.joined = true;
        }
        Ok((a, b))
    }

    fn send_for(
        &mut self,
        id: u8,
        payload: &[u8],
        kept: Option<Lifetime>,
        resend: Option<Duration>,
        deadline: Option<Instant>,
    ) -> Result<u64, SendError> {
        if self.table.is_forbidden(id) {
            return Err(SendError::Forbidden(id));
        }
        let info = self
            .table
            .info(Direction::ToClient, id)
            .ok_or(SendError::UnknownId(id))?;
        if self.state != State::Connected {
            return Err(SendError::NotConnected);
        }
        let bad_length = |expected| SendError::BadLength {
            id,
            expected,
            got: payload.len(),
        };
        let variable = match info.length {
            Length::Fixed(len) => {
                if payload.len() != len {
                    return Err(bad_length(len));
                }
                false
            }
            Length::PlayerInputs { head } => {
                if self.mask_bytes == 0 {
                    return Err(SendError::BadPlayerInputs(id));
                }
                match player_inputs_len(head, self.mask_bytes, payload) {
                    Some(len) if len == payload.len() => {}
                    Some(len) => return Err(bad_length(len)),
                    None => return Err(SendError::BadPlayerInputs(id)),
                }
                false
            }
            Length::Variable { first_min, .. } => {
                if payload.len() < first_min {
                    return Err(bad_length(first_min));
                }
                true
            }
        };
        let splittable = variable && info.class != Class::Plain;
        let x_len = usize::from(info.class != Class::Plain);
        let whole = 1 + x_len + payload.len() <= MAX_BODY;
        let parts = 1 + payload
            .len()
            .saturating_sub(FIRST_PART_DATA)
            .div_ceil(PART_DATA);
        if !whole && (!splittable || parts > 255) {
            return Err(SendError::TooLarge(payload.len()));
        }
        let channel = match info.class {
            Class::Plain => Channel::None,
            Class::Unique => {
                self.next_unique += 1;
                Channel::Unique(self.next_unique - 1)
            }
            Class::Ordered => {
                self.next_order += 1;
                Channel::Ordered(self.next_order - 1)
            }
        };
        let x = match channel {
            Channel::None => None,
            Channel::Unique(count) | Channel::Ordered(count) => Some(count as u8),
        };
        let serial = self.next_serial;
        self.next_serial += 1;
        let lifetime = match (info.lifetime, kept) {
            (Lifetime::For(_), Some(kept)) => kept,
            (lifetime, _) => lifetime,
        };
        let outgoing = |head: Vec<u8>, payload: &[u8]| Outgoing {
            serial,
            head,
            payload: payload.to_vec(),
            variable,
            channel,
            lifetime,
            resend: resend.unwrap_or(info.resend),
            expires: match info.lifetime {
                Lifetime::For(_) => deadline,
                Lifetime::Once | Lifetime::Forever => None,
            },
            // The ids with a short lifetime by the table, kept until acked
            // or not (`send_kept`: a resync's replay and the relays to a
            // resynced game): a burst of them is packed (`flush_due`).
            gathers: info.class == Class::Plain && matches!(info.lifetime, Lifetime::For(_)),
            joined: false,
            sent_in: VecDeque::new(),
        };
        self.outstanding
            .insert(serial, if whole { 1 } else { parts as u32 });
        if whole {
            let mut head = vec![id];
            head.extend(x);
            self.pending.push_back(outgoing(head, payload));
            return Ok(serial);
        }
        let x = x.expect("a splittable id is unique or ordered");
        let (first, mut rest) = payload.split_at(FIRST_PART_DATA);
        self.pending
            .push_back(outgoing(vec![MULTIPART, id, x, 0, parts as u8], first));
        for index in 1..parts {
            let (data, tail) = rest.split_at(rest.len().min(PART_DATA));
            rest = tail;
            self.pending
                .push_back(outgoing(vec![MULTIPART, id, x, index as u8], data));
        }
        Ok(serial)
    }

    /// Whether nothing of a message sent is waiting any more: the client
    /// acked every part of it. A send-once message counts once it went out
    /// (its arrival cannot be known). False for a message dropped at the end
    /// of its lifetime (see [`Self::is_lost`]).
    pub fn is_delivered(&self, serial: u64) -> bool {
        serial < self.next_serial
            && !self.lost.contains(&serial)
            && !self.outstanding.contains_key(&serial)
    }

    /// The walk of the queue that `outstanding` stands for agrees with it.
    #[cfg(test)]
    pub(crate) fn outstanding_agrees(&self) -> bool {
        let mut counted: HashMap<u64, u32> = HashMap::new();
        for message in self
            .pending
            .iter()
            .chain(self.unacked.iter().map(|(message, _)| message))
        {
            *counted.entry(message.serial).or_default() += 1;
        }
        counted == self.outstanding
    }

    /// Whether a message was dropped unacked at the end of its lifetime
    /// (the short-lived ids, and the snapshot E8 after 1.75 s). The last
    /// 4096 are remembered.
    pub fn is_lost(&self, serial: u64) -> bool {
        self.lost.contains(&serial)
    }

    /// Disconnect the client: it confirms (`FB`) and leaves within 500 ms.
    pub fn disconnect(&mut self, now: Instant) {
        if self.state == State::Connected {
            self.pending.clear();
            self.unacked.clear();
            self.outstanding.clear();
            self.state = State::Closing { since: now };
        }
    }

    /// Disconnect the client at once with `FD`, which it obeys in any state
    /// without the 500 ms handshake. `FD` is sent once and may be lost, so
    /// the ordinary disconnect follows it.
    pub fn kick(&mut self, now: Instant) {
        if self.state == State::Connected {
            self.low_level.push_back(low_level::INSTANT_DISCONNECT);
            self.disconnect(now);
        }
    }

    /// The datagrams to send now: low-level messages, due resends and new
    /// messages, and a keep-alive when a client datagram needs its ack or
    /// nothing went for a while.
    pub fn poll(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut datagrams = Vec::new();
        // `rseq` must be a seq the client sent: nothing before its first.
        if self.received.newest().is_none() {
            return datagrams;
        }
        let quiet_for = |pace: Duration, last_sent: Option<Instant>| {
            last_sent.is_none_or(|sent| now.saturating_duration_since(sent) >= pace)
        };
        if let State::Closing { since } = self.state {
            if now.saturating_duration_since(since) >= self.config.closing {
                self.close();
            } else if self.low_level.is_empty() && quiet_for(CLOSING_PACE, self.last_sent) {
                self.low_level.push_back(low_level::INITIATE_DISCONNECTION);
            }
        }
        while let Some(id) = self.low_level.pop_front() {
            datagrams.push(self.datagram(false, &[id], now));
        }

        if self.state == State::Connected {
            self.expire(now);
            // Resends first, oldest first.
            let (due, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.unacked)
                .into_iter()
                .partition(|(_, at)| *at <= now);
            self.unacked = waiting;
            for (message, _) in due.into_iter().rev() {
                self.pending.push_front(message);
            }
            let bases = self.channel_bases(now);
            self.refill(now);
            let flush = self.flush_due(now);
            while flush && self.ahead() < MAX_AHEAD && self.budget > 0 {
                let Some(messages) = self.take_messages(bases) else {
                    break;
                };
                let multi = messages.len() > 1;
                let mut body = Vec::with_capacity(MAX_DATAGRAM);
                for message in &messages {
                    message.write(&mut body, multi);
                }
                datagrams.push(self.datagram(multi, &body, now));
                self.budget -= 1;
                self.last_data = Some(now);
                let number = self.sent_total - 1;
                for mut message in messages {
                    if !message.sent_in.is_empty() {
                        self.resent += 1;
                    }
                    let lifetime = match message.lifetime {
                        Lifetime::Once => {
                            if message.channel != Channel::None {
                                self.retired.push_back((message.channel, now));
                            }
                            settle(&mut self.outstanding, message.serial);
                            continue;
                        }
                        Lifetime::For(lifetime) => Some(lifetime),
                        Lifetime::Forever => None,
                    };
                    // At its first send: the table's lifetime from now, or
                    // its deadline (`send_until`) if that is later.
                    if message.sent_in.is_empty()
                        && let Some(lifetime) = lifetime
                    {
                        let end = now + lifetime;
                        message.expires = Some(message.expires.map_or(end, |at| at.max(end)));
                    }
                    if message.sent_in.len() == SEQS_PER_MESSAGE {
                        message.sent_in.pop_front();
                    }
                    message.sent_in.push_back(number);
                    let at = now + message.resend;
                    self.unacked.push((message, at));
                }
            }
        }

        if datagrams.is_empty() && self.state == State::Connected {
            let idle = if self.ahead() >= MAX_AHEAD {
                SLOW_IDLE.max(self.config.idle)
            } else {
                self.config.idle
            };
            let ack_now = self.ack_due && quiet_for(self.config.ack_delay, self.last_sent);
            if ack_now || quiet_for(idle, self.last_sent) {
                datagrams.push(self.datagram(false, &[low_level::CONTINUOUS_FLOW], now));
            }
        }
        if !datagrams.is_empty() {
            self.last_sent = Some(now);
            self.ack_due = false;
        }
        if let Some(log) = &mut self.log {
            for datagram in &datagrams {
                log.datagram(Direction::ToClient, datagram, self.mask_bytes);
            }
        }
        datagrams
    }

    /// Whether the queued messages go now (`Config::gather`). Short-lived
    /// ones wait until `gather` has passed since the last datagram with
    /// messages, so that the relays that come meanwhile share a datagram.
    /// They go sooner, with everything queued, when:
    /// - anything else is queued: a clock message or a ping (E5, E6, E7)
    ///   must not wait, or the round trip and clock samples taken with it
    ///   would be off by the wait (a relay kept until acked is not such:
    ///   it waits to be packed as any relay);
    /// - a part of a split message may go;
    /// - the queue fills a datagram;
    /// - a client datagram waits for its ack for `ack_delay`: the ack
    ///   rides on the messages, not on a bare `FC` just before them.
    fn flush_due(&self, now: Instant) -> bool {
        let gather = self.config.gather;
        if self
            .last_data
            .is_none_or(|at| now.saturating_duration_since(at) >= gather)
        {
            return true;
        }
        if self.ack_due
            && self
                .last_sent
                .is_none_or(|at| now.saturating_duration_since(at) >= self.config.ack_delay)
        {
            return true;
        }
        let room = MAX_DATAGRAM - HEADER - self.tail_wanted().0;
        let mut used = 0;
        for (index, message) in self.pending.iter().enumerate() {
            if message.is_part() {
                if self.bulk_budget > 0 {
                    return true;
                }
                continue;
            }
            if !message.gathers {
                return true;
            }
            used += message.multi_len();
            if used >= room || index + 1 >= MAX_MESSAGES {
                return true;
            }
        }
        false
    }

    /// Earn the datagrams of the time since the last count.
    fn refill(&mut self, now: Instant) {
        let pace = self.config.pace.as_micros().max(1);
        let earned = now.saturating_duration_since(self.budget_at).as_micros() / pace;
        if earned > 0 {
            self.budget =
                (u128::from(self.budget) + earned).min(u128::from(self.config.burst)) as u32;
            self.budget_at = now;
        }
        let pace = self.config.bulk_pace.as_micros().max(1);
        let earned = now.saturating_duration_since(self.bulk_at).as_micros() / pace;
        if earned > 0 {
            let budget = u128::from(self.bulk_budget) + earned;
            if budget >= u128::from(self.config.bulk_burst) {
                self.bulk_budget = self.config.bulk_burst;
                self.bulk_at = now;
            } else {
                // The rest of the time counts on: a server that looks
                // every 5 ms sends 5 parts in 15 ms, not 3.
                self.bulk_budget = budget as u32;
                self.bulk_at += Duration::from_micros((earned * pace) as u64);
            }
        }
    }

    /// Drop the messages whose lifetime ended (the client's 0x3FE470:
    /// silently, no disconnect).
    fn expire(&mut self, now: Instant) {
        let mut lost = Vec::new();
        let retired = &mut self.retired;
        let mut alive = |message: &Outgoing| {
            let alive = message.expires.is_none_or(|expires| now < expires);
            if !alive {
                lost.push(message.serial);
                if message.channel != Channel::None {
                    retired.push_back((message.channel, now));
                }
            }
            alive
        };
        self.unacked.retain(|(message, _)| alive(message));
        self.pending.retain(|message| alive(message));
        // The parts of a split message leave one after another, so they
        // end one after another: with the first, the rest is of no use to
        // the client and goes too.
        if !lost.is_empty() {
            self.unacked
                .retain(|(message, _)| !lost.contains(&message.serial));
            self.pending
                .retain(|message| !lost.contains(&message.serial));
            for serial in &lost {
                self.outstanding.remove(serial);
            }
        }
        self.lost.extend(lost);
        while self.lost.len() > LOST_KEPT {
            self.lost.pop_first();
        }
    }

    /// The oldest unique id and order id not yet acked (or not yet sent),
    /// or done with less than `guard` ago.
    fn channel_bases(&mut self, now: Instant) -> (Option<u64>, Option<u64>) {
        let guard = self.config.guard;
        while self
            .retired
            .front()
            .is_some_and(|(_, at)| now.saturating_duration_since(*at) >= guard)
        {
            self.retired.pop_front();
        }
        let mut unique: Option<u64> = None;
        let mut ordered: Option<u64> = None;
        let channels = self
            .pending
            .iter()
            .chain(self.unacked.iter().map(|(message, _)| message))
            .map(|message| message.channel)
            .chain(self.retired.iter().map(|(channel, _)| *channel));
        for channel in channels {
            let (base, count) = match channel {
                Channel::None => continue,
                Channel::Unique(count) => (&mut unique, count),
                Channel::Ordered(count) => (&mut ordered, count),
            };
            *base = Some(base.map_or(count, |base| base.min(count)));
        }
        (unique, ordered)
    }

    /// Take the messages of the next datagram off the queue, in order: the
    /// first that may go, then as many more as fit with it. `None` when
    /// nothing may go. A part of a split message waits for its turn
    /// (`bulk_burst`). On its first way out it holds back what was queued
    /// behind it, so a message's order is the order it had when all its
    /// parts left at once; a part that goes again holds nothing back.
    fn take_messages(&mut self, bases: (Option<u64>, Option<u64>)) -> Option<Vec<Outgoing>> {
        let may_go = |channel: Channel| {
            let (base, count) = match channel {
                Channel::None => return true,
                Channel::Unique(count) => (bases.0, count),
                Channel::Ordered(count) => (bases.1, count),
            };
            base.is_none_or(|base| count - base < CHANNEL_WINDOW)
        };
        let room = MAX_DATAGRAM - HEADER - self.tail_wanted().0;
        let mut taken = Vec::new();
        let mut used = 0;
        let mut bulk = self.bulk_budget;
        // The second of a joined pair goes with the first, whatever else.
        let mut with_first = false;
        for (index, message) in self.pending.iter().enumerate() {
            if std::mem::take(&mut with_first) {
                used += message.multi_len();
                taken.push(index);
                continue;
            }
            let next = message
                .joined
                .then(|| self.pending.get(index + 1))
                .flatten();
            if !may_go(message.channel) || next.is_some_and(|next| !may_go(next.channel)) {
                continue;
            }
            if message.is_part() && bulk == 0 {
                // A part that has not gone yet keeps its place: what its
                // sender queued behind the message is meant to come after
                // it (the discards right behind a world, `send_sync`).
                if message.sent_in.is_empty() {
                    break;
                }
                continue;
            }
            let cost = message.multi_len();
            let pair = next.map_or(0, Outgoing::multi_len);
            let count = 1 + usize::from(next.is_some());
            if !taken.is_empty()
                && (used + cost + pair > room || taken.len() + count > MAX_MESSAGES)
            {
                break;
            }
            used += cost;
            taken.push(index);
            with_first = next.is_some();
            if message.is_part() {
                bulk -= 1;
            }
        }
        if taken.is_empty() {
            return None;
        }
        self.bulk_budget = bulk;
        let mut messages: Vec<Outgoing> = taken
            .iter()
            .rev()
            .map(|index| self.pending.remove(*index).expect("an index just seen"))
            .collect();
        messages.reverse();
        Some(messages)
    }

    /// Datagrams sent past the newest the client acked.
    fn ahead(&self) -> u64 {
        match self.newest_acked {
            Some(acked) => self.sent_total - 1 - acked,
            None => self.sent_total,
        }
    }

    /// How many tail ack bytes the next datagram should carry, and the
    /// client seq number its acks are then complete up to.
    ///
    /// Every client seq received past the ack floor (what the client is
    /// known to have had acked) must be reported, plus a margin; bits
    /// beyond the oldest received one carry nothing and are left out. The
    /// client treats bits outside the window sent as unknown (§2.4), and
    /// the received set only grows, so a short window never contradicts an
    /// earlier longer one.
    fn tail_wanted(&self) -> (usize, u64) {
        let depth = match self.ack_floor {
            Some(floor) => (self.received_number - floor + ACK_MARGIN).min(125),
            None => 125,
        } as u32;
        // b(k) is bit 127 - k: the deepest set bit within the depth.
        let wanted = self.received.bits() >> (127 - depth) << (127 - depth) & !(1 << 127);
        let deepest = if wanted == 0 {
            0
        } else {
            127 - wanted.trailing_zeros() as usize
        };
        (ack::tail_for_depth(deepest), self.received_number)
    }

    /// A datagram under the next seq, with the current acks. The body is
    /// never empty: a header-only datagram is one the client drops.
    fn datagram(&mut self, multi: bool, body: &[u8], now: Instant) -> Vec<u8> {
        debug_assert!(!body.is_empty());
        let rseq = self
            .received
            .newest()
            .expect("nothing is sent before the client's first datagram");
        let (wanted, floor) = self.tail_wanted();
        let tail = wanted.min(MAX_DATAGRAM - HEADER - body.len());
        let (ack0, tail_bytes) = ack::encode(self.received.bits(), tail, multi);
        let number = self.sent_total;
        self.sent_total += 1;
        let mut datagram = Vec::with_capacity(HEADER + body.len() + tail);
        datagram.extend([number as u8, rseq, ack0]);
        datagram.extend(body);
        datagram.extend(tail_bytes);
        debug_assert!(datagram.len() <= MAX_DATAGRAM);
        if self.sent.len() == SENT_KEPT {
            self.sent.pop_front();
        }
        self.sent.push_back(Sent {
            number,
            at: now,
            timed: false,
            floor: (tail == wanted).then_some(floor),
        });
        datagram
    }
}

/// Add a part to the split message `x` of a channel; the whole payload once
/// it is complete. A new message past the limit pushes out the one furthest
/// behind it.
/// One entry of `serial` left the queue for good (acked, or sent once).
fn settle(outstanding: &mut HashMap<u64, u32>, serial: u64) {
    if let Some(count) = outstanding.get_mut(&serial) {
        *count -= 1;
        if *count == 0 {
            outstanding.remove(&serial);
        }
    }
}

/// The messages of a client datagram in the order the client sent them.
/// The client's send queue is sorted by size and taken from its back
/// (0x3FE580), so a datagram holds a tick's messages largest first, not in
/// the order they were sent: the lobby movie's move of an upgrade to
/// another slot, `drop_upgrade_item` then `relocate_upgrade_item` in one
/// frame, came as `46 01` (unique id 0x22) before `46 02` (0x21), and done
/// in that order took the upgrade off (live, 2026-10-21). A unique id is
/// the message's place in the send order (the counter at
/// `construct_packet`): the unique messages are taken by it, the nearest
/// way round from the first one's (the window is 128 wide), each other
/// message keeping its place.
fn in_send_order<'m, 'a>(messages: &'m [wire::Message<'a>]) -> Vec<&'m wire::Message<'a>> {
    let unique = |message: &wire::Message<'_>| !message.low_level && message.class == Class::Unique;
    let mut order: Vec<&wire::Message<'a>> = messages.iter().collect();
    let Some(first) = messages.iter().find(|m| unique(m)).and_then(|m| m.x) else {
        return order;
    };
    let mut sorted: Vec<&wire::Message<'a>> = messages.iter().filter(|m| unique(m)).collect();
    sorted.sort_by_key(|m| m.x.map_or(0, |x| x.wrapping_sub(first) as i8));
    let mut sorted = sorted.into_iter();
    for slot in &mut order {
        if unique(slot) {
            *slot = sorted.next().expect("as many as were taken out");
        }
    }
    order
}

fn add_part(
    assemblies: &mut BTreeMap<u8, Assembly>,
    x: u8,
    id: u8,
    part: Part,
    data: &[u8],
) -> Option<Vec<u8>> {
    if !assemblies.contains_key(&x) && assemblies.len() == MAX_ASSEMBLIES {
        let oldest = *assemblies
            .keys()
            .max_by_key(|key| x.wrapping_sub(**key))
            .expect("the map is full");
        assemblies.remove(&oldest);
    }
    let assembly = assemblies.entry(x).or_insert_with(|| Assembly {
        id,
        count: None,
        parts: BTreeMap::new(),
    });
    if assembly.id != id {
        return None;
    }
    let whole = assembly.add(part, data)?;
    assemblies.remove(&x);
    Some(whole)
}
