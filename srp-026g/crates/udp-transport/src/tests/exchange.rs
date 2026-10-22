//! Server ↔ client-model exchanges over a link that loses, duplicates and
//! reorders datagrams (seeded, so every run is the same).
//!
//! What must hold whatever the link does:
//! - the model never rejects a server datagram (`faults` stays empty), and
//!   every datagram passes the client's stateless checks as it is emitted;
//! - every reliable unique message is delivered exactly once, every ordered
//!   one exactly once and in the order sent, every reliable plain one at
//!   least once; send-once and short-lived ones at most as often as sent
//!   (plain short-lived ones: only ever a payload that was sent);
//! - the same for the client's messages on the server side.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::client_model::{ClientModel, ModelState};
use crate::wire::MAX_DATAGRAM;
use crate::{Class, Direction, Event, Lifetime, State, Table, Transport};

/// xorshift64*.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, per_mille: u64) -> bool {
        self.below(1000) < per_mille
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next() as u8).collect()
    }
}

/// What the link does to a datagram.
#[derive(Clone, Copy)]
struct Link {
    /// Chances in a thousand.
    loss: u64,
    duplicate: u64,
    /// The longest delay in 5 ms steps; each datagram gets its own, so
    /// later ones overtake earlier ones.
    delay: u64,
}

const CLEAN: Link = Link {
    loss: 0,
    duplicate: 0,
    delay: 0,
};
const POOR: Link = Link {
    loss: 50,
    duplicate: 50,
    delay: 8,
};
const BAD: Link = Link {
    loss: 200,
    duplicate: 100,
    delay: 12,
};

const STEP: Duration = Duration::from_millis(5);

struct Sim {
    table: Table,
    server: Transport,
    client: ClientModel,
    rng: Rng,
    link: Link,
    start: Instant,
    step: u64,
    to_client: Vec<(u64, Vec<u8>)>,
    to_server: Vec<(u64, Vec<u8>)>,
    /// What the server's caller received.
    events: Vec<(u8, Vec<u8>)>,
    emitted: usize,
}

impl Sim {
    fn new(table: Table, link: Link, seed: u64) -> Self {
        let start = Instant::now();
        Self {
            table,
            server: Transport::new(table, start),
            client: ClientModel::new(table),
            rng: Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1),
            link,
            start,
            step: 0,
            to_client: Vec::new(),
            to_server: Vec::new(),
            events: Vec::new(),
            emitted: 0,
        }
    }

    fn now(&self) -> Instant {
        self.start + STEP * self.step as u32
    }

    /// One 5 ms step: deliver what is due, then both sides send.
    fn step(&mut self) {
        let now = self.now();
        let step = self.step;
        let due = |queue: &mut Vec<(u64, Vec<u8>)>| -> Vec<Vec<u8>> {
            let (due, later): (Vec<_>, Vec<_>) = std::mem::take(queue)
                .into_iter()
                .partition(|(at, _)| *at <= step);
            *queue = later;
            due.into_iter().map(|(_, datagram)| datagram).collect()
        };
        for datagram in due(&mut self.to_client) {
            self.client.receive(&datagram, now);
        }
        for datagram in due(&mut self.to_server) {
            for event in self.server.receive(&datagram, now) {
                if let Event::Message { id, payload } = event {
                    self.events.push((id, payload));
                }
            }
        }
        for datagram in self.server.poll(now) {
            // The stateless rules, on every datagram as it leaves.
            assert!(datagram.len() <= MAX_DATAGRAM);
            if let Err(fault) = ClientModel::check(self.table, self.client.mask_bytes, &datagram) {
                panic!("the server emitted a datagram the client drops: {fault}: {datagram:02X?}");
            }
            self.emitted += 1;
            Self::carry(
                &mut self.rng,
                self.link,
                step,
                &mut self.to_client,
                datagram,
            );
        }
        for datagram in self.client.poll(now) {
            Self::carry(
                &mut self.rng,
                self.link,
                step,
                &mut self.to_server,
                datagram,
            );
        }
        assert!(
            self.client.faults.is_empty(),
            "step {step}: {:?}",
            self.client.faults
        );
        self.step += 1;
    }

    fn carry(
        rng: &mut Rng,
        link: Link,
        step: u64,
        queue: &mut Vec<(u64, Vec<u8>)>,
        datagram: Vec<u8>,
    ) {
        if rng.chance(link.loss) {
            return;
        }
        let copies = 1 + u64::from(rng.chance(link.duplicate));
        for _ in 0..copies {
            let delay = if link.delay == 0 {
                0
            } else {
                rng.below(link.delay + 1)
            };
            queue.push((step + 1 + delay, datagram.clone()));
        }
    }

    /// Step until both sides have nothing left to deliver.
    fn settle(&mut self, limit: u64) {
        let end = self.step + limit;
        while self.server.backlog() > 0 || self.client.backlog() > 0 {
            assert!(self.step < end, "not settled after {limit} steps");
            self.step();
        }
        // Let what is in flight arrive.
        for _ in 0..40 {
            self.step();
        }
    }
}

type Sent = Vec<(u8, Vec<u8>)>;

fn count(messages: &[(u8, Vec<u8>)]) -> BTreeMap<(u8, Vec<u8>), usize> {
    let mut counts = BTreeMap::new();
    for message in messages {
        *counts.entry(message.clone()).or_insert(0) += 1;
    }
    counts
}

/// Check what arrived against what was sent, by each id's class and
/// lifetime in the table.
fn check_delivery(table: Table, direction: Direction, sent: &Sent, delivered: &Sent) {
    let (sent_counts, delivered_counts) = (count(sent), count(delivered));
    for message in delivered_counts.keys() {
        assert!(
            sent_counts.contains_key(message),
            "delivered but never sent: {:#04x} ({} bytes)",
            message.0,
            message.1.len()
        );
    }
    for (message, sent) in &sent_counts {
        let info = table.info(direction, message.0).unwrap();
        let got = delivered_counts.get(message).copied().unwrap_or(0);
        let what = format!("{:#04x} ({} bytes)", message.0, message.1.len());
        match (info.class, info.lifetime) {
            (Class::Unique | Class::Ordered, Lifetime::Forever) => {
                assert_eq!(got, *sent, "exactly once: {what}")
            }
            (Class::Unique | Class::Ordered, _) => assert!(got <= *sent, "at most once: {what}"),
            (Class::Plain, Lifetime::Forever) => assert!(got >= *sent, "at least once: {what}"),
            (Class::Plain, Lifetime::Once) => assert!(got <= *sent, "at most once: {what}"),
            (Class::Plain, Lifetime::For(_)) => {}
        }
    }
    // Ordered ids arrive in the order sent.
    let ordered = |messages: &Sent| -> Sent {
        messages
            .iter()
            .filter(|message| table.info(direction, message.0).unwrap().class == Class::Ordered)
            .cloned()
            .collect()
    };
    let (sent, delivered) = (ordered(sent), ordered(delivered));
    assert_eq!(sent.len(), delivered.len());
    for (index, (sent, delivered)) in sent.iter().zip(&delivered).enumerate() {
        assert!(
            sent == delivered,
            "ordered message {index}: sent {:#04x} ({} bytes), got {:#04x} ({} bytes)",
            sent.0,
            sent.1.len(),
            delivered.0,
            delivered.1.len()
        );
    }
}

/// A payload of `len` bytes that starts with a counter where it fits, so
/// equal sizes stay distinct.
fn payload(rng: &mut Rng, counter: &mut u32, len: usize) -> Vec<u8> {
    *counter += 1;
    let mut payload = rng.bytes(len);
    for (byte, tag) in payload.iter_mut().zip(counter.to_le_bytes()) {
        *byte = tag;
    }
    payload
}

// ------------------------------------------------------------- the lobby

const LOBBY_SIZES: [usize; 20] = [
    0, 1, 2, 5, 40, 120, 253, 254, 255, 256, 498, 499, 500, 501, 502, 996, 1500, 5000, 12000, 30000,
];

fn lobby_exchange(link: Link, seed: u64, join: bool) {
    let mut sim = Sim::new(Table::Lobby, link, seed);
    sim.client.join = join;
    let mut script = Rng(seed ^ 0xABCD_EF01_2345_6789);
    let mut counter = 0u32;
    let mut to_client: Sent = Vec::new();
    let mut to_server: Sent = Vec::new();

    // The sign-in, then the server's pushes as in lobby.md: the inventory
    // (large) before the account (large), with unique ids in between.
    let sign_in = payload(&mut script, &mut counter, 5);
    sim.client.send(0x2E, &sign_in);
    to_server.push((0x2E, sign_in));
    while sim.events.is_empty() {
        sim.step();
        assert!(sim.step < 4000, "the sign-in never arrived");
    }
    let ordered_ids = [
        0x83, 0x84, 0x85, 0x87, 0x89, 0x8B, 0x92, 0x99, 0x9B, 0xA3, 0xA9,
    ];
    let unique_ids = [0x80, 0x82, 0x9A, 0xA1, 0xA2, 0xA4, 0xAA];
    let mut push = |sim: &mut Sim, script: &mut Rng, counter: &mut u32, id: u8, len: usize| {
        let payload = payload(script, counter, len);
        sim.server.send(id, &payload).unwrap();
        to_client.push((id, payload));
    };
    push(&mut sim, &mut script, &mut counter, 0x80, 12);
    push(&mut sim, &mut script, &mut counter, 0x85, 3);
    push(&mut sim, &mut script, &mut counter, 0x89, 30000);
    push(&mut sim, &mut script, &mut counter, 0x87, 2800);
    push(&mut sim, &mut script, &mut counter, 0x8B, 8);

    // A stream of mixed pushes and requests.
    for _ in 0..600 {
        for _ in 0..script.below(3) {
            let len = LOBBY_SIZES[script.below(14) as usize];
            match script.below(10) {
                0..=5 => {
                    let id = ordered_ids[script.below(ordered_ids.len() as u64) as usize];
                    push(&mut sim, &mut script, &mut counter, id, len);
                }
                6..=8 => {
                    let id = unique_ids[script.below(unique_ids.len() as u64) as usize];
                    push(&mut sim, &mut script, &mut counter, id, len);
                }
                _ => push(&mut sim, &mut script, &mut counter, 0x9F, len.min(400)),
            }
        }
        if script.chance(8) {
            let len = LOBBY_SIZES[script.below(LOBBY_SIZES.len() as u64) as usize];
            push(&mut sim, &mut script, &mut counter, 0x89, len);
        }
        // (The model has no limit of its own on unique ids in flight, so its
        // requests come slowly enough to stay inside the server's window.)
        if script.chance(60) {
            let id = 0x2F + script.below(0x22) as u8;
            let len = LOBBY_SIZES[script.below(16) as usize];
            let payload = payload(&mut script, &mut counter, len);
            sim.client.send(id, &payload);
            to_server.push((id, payload));
        }
        sim.step();
    }
    // A burst that wraps both one-byte counters while nothing is acked yet.
    for index in 0..700 {
        let id = if index % 3 == 0 { 0xA2 } else { 0x8C };
        push(&mut sim, &mut script, &mut counter, id, 6);
    }
    // More than 256 requests, to wrap the client's counter too.
    for _ in 0..300 {
        let payload = payload(&mut script, &mut counter, 9);
        sim.client.send(0x4C, &payload);
        to_server.push((0x4C, payload));
        for _ in 0..10 {
            sim.step();
        }
    }
    sim.settle(200_000);

    assert_eq!(sim.client.state, ModelState::Connected);
    assert_eq!(sim.server.state(), State::Connected);
    check_delivery(
        Table::Lobby,
        Direction::ToClient,
        &to_client,
        &sim.client.delivered,
    );
    check_delivery(Table::Lobby, Direction::ToServer, &to_server, &sim.events);
    assert!(sim.emitted > 300);
}

#[test]
fn a_lobby_exchange_on_a_clean_link() {
    lobby_exchange(CLEAN, 1, false);
}

#[test]
fn a_lobby_exchange_survives_loss_duplication_and_reordering() {
    for seed in 1..=4 {
        lobby_exchange(POOR, seed, false);
    }
    for seed in 5..=7 {
        lobby_exchange(BAD, seed, false);
    }
}

#[test]
fn a_lobby_exchange_with_a_client_that_joins_its_messages() {
    lobby_exchange(CLEAN, 8, true);
    lobby_exchange(POOR, 9, true);
    lobby_exchange(BAD, 10, true);
}

// ------------------------------------------------------------- the match

/// A valid payload for a server match id.
fn match_payload(rng: &mut Rng, counter: &mut u32, id: u8) -> Vec<u8> {
    let inputs = |rng: &mut Rng, head: usize| {
        // 20 players: 3 mask bytes, 2 to 20 bits set.
        let players = 2 + rng.below(19) as usize;
        let mut mask = 0u32;
        while (mask.count_ones() as usize) < players {
            mask |= 1 << rng.below(20);
        }
        let mut payload = rng.bytes(head);
        payload.extend(&mask.to_le_bytes()[..3]);
        payload.extend(rng.bytes(8 * players));
        payload
    };
    let len = match id {
        0xE3 | 0xE6 | 0xF0 => 4,
        0xE5 => 7,
        0xE7 | 0xE9 | 0xEA | 0xEF | 0xF2 | 0xF4 => 2,
        0xEB => 10,
        0xEC => 11,
        0xF1 | 0xF5 => 1,
        0xF3 => 3,
        0xED => return inputs(rng, 1),
        0xEE => return inputs(rng, 2),
        0xE4 => [1, 300, 2000, 40000][rng.below(4) as usize],
        0xE8 => [3, 700, 9000][rng.below(3) as usize],
        0xF6 => [1, 254, 255, 600][rng.below(4) as usize],
        _ => unreachable!(),
    };
    if len >= 4 {
        payload(rng, counter, len)
    } else {
        rng.bytes(len)
    }
}

fn match_exchange(link: Link, seed: u64, join: bool) {
    let mut sim = Sim::new(Table::Match, link, seed);
    sim.client.join = join;
    sim.client.mask_bytes = 3;
    sim.server.set_roster_size(20);
    let mut script = Rng(seed ^ 0x1357_9BDF_0246_8ACE);
    let mut counter = 0u32;
    let mut to_client: Sent = Vec::new();
    let mut to_server: Sent = Vec::new();
    let mut snapshots = Vec::new();

    let mut request = |sim: &mut Sim, id: u8, payload: Vec<u8>| {
        sim.client.send(id, &payload);
        to_server.push((id, payload));
    };
    request(&mut sim, 0xD8, vec![0x78, 0x56, 0x34, 0x12, 0x00]);
    while sim.events.is_empty() {
        sim.step();
        assert!(sim.step < 4000, "the connect never arrived");
    }
    let frequent = [
        0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xE5, 0xE6, 0xE7,
    ];
    let reliable = [0xE3, 0xF3, 0xF4, 0xF5, 0xF6, 0xE4];
    let client_inputs = [(0xDC, 9), (0xDD, 10), (0xDF, 5), (0xDB, 6), (0xE1, 0)];
    for round in 0..1500u32 {
        for _ in 0..script.below(4) {
            let id = frequent[script.below(frequent.len() as u64) as usize];
            let payload = match_payload(&mut script, &mut counter, id);
            sim.server.send(id, &payload).unwrap();
            to_client.push((id, payload));
        }
        if script.chance(60) {
            let id = reliable[script.below(reliable.len() as u64) as usize];
            let payload = match_payload(&mut script, &mut counter, id);
            sim.server.send(id, &payload).unwrap();
            to_client.push((id, payload));
        }
        if round % 400 == 100 {
            let payload = match_payload(&mut script, &mut counter, 0xE8);
            snapshots.push((sim.server.send(0xE8, &payload).unwrap(), payload.clone()));
            to_client.push((0xE8, payload));
        }
        // The client acks a message through the last 8 one-byte seqs it
        // went under, so the model sends at a rate a real client does
        // (some 60 datagrams a second, not hundreds).
        if script.chance(300) {
            let (id, len) = client_inputs[script.below(client_inputs.len() as u64) as usize];
            let payload = payload(&mut script, &mut counter, len);
            request(&mut sim, id, payload);
        }
        if script.chance(40) {
            let id = [0xD9, 0xDA, 0xDE, 0xE0][script.below(4) as usize];
            let len = match id {
                0xD9 => 4,
                0xDE => 2,
                _ => 0,
            };
            let payload = script.bytes(len);
            request(&mut sim, id, payload);
        }
        sim.step();
    }
    sim.settle(100_000);

    assert_eq!(sim.client.state, ModelState::Connected);
    check_delivery(
        Table::Match,
        Direction::ToClient,
        &to_client,
        &sim.client.delivered,
    );
    check_delivery(Table::Match, Direction::ToServer, &to_server, &sim.events);
    // What the server reports as delivered did arrive, and nothing is
    // both delivered and lost.
    for (serial, payload) in snapshots {
        let arrived = sim.client.delivered.contains(&(0xE8, payload));
        assert!(!(sim.server.is_delivered(serial) && sim.server.is_lost(serial)));
        assert!(sim.server.is_delivered(serial) || sim.server.is_lost(serial));
        if sim.server.is_delivered(serial) {
            assert!(arrived, "a snapshot reported delivered did not arrive");
        }
    }
}

#[test]
fn a_match_exchange_on_a_clean_link() {
    match_exchange(CLEAN, 11, false);
}

#[test]
fn a_match_exchange_survives_loss_duplication_and_reordering() {
    for seed in 12..=15 {
        match_exchange(POOR, seed, false);
    }
    for seed in 16..=18 {
        match_exchange(BAD, seed, false);
    }
}

#[test]
fn a_match_exchange_with_a_client_that_joins_its_messages() {
    match_exchange(CLEAN, 19, true);
    match_exchange(BAD, 20, true);
}

// ------------------------------------------- a late datagram, a lap back

/// A match link driven by hand, 20 ms a step: one relay a step, so that
/// each step is one server datagram, numbered by `emitted`.
struct Pair {
    server: Transport,
    model: ClientModel,
    start: Instant,
    step: u32,
    emitted: usize,
    relays: Vec<(u64, Vec<u8>)>,
}

impl Pair {
    fn new() -> Self {
        let start = Instant::now();
        let mut server = Transport::new(Table::Match, start);
        server.set_roster_size(20);
        let mut model = ClientModel::new(Table::Match);
        model.mask_bytes = 3;
        model.send(0xD8, &[1, 2, 3, 4, 0]);
        for datagram in model.poll(start) {
            server.receive(&datagram, start);
        }
        Self {
            server,
            model,
            start,
            step: 0,
            emitted: 0,
            relays: Vec::new(),
        }
    }

    fn now(&self) -> Instant {
        self.start + Duration::from_millis(20) * self.step
    }

    /// The next step's server datagrams, with their numbers, not yet
    /// delivered; with a relay queued first if `relay`.
    fn server_step(&mut self, relay: bool) -> Vec<(usize, Vec<u8>)> {
        self.step += 1;
        let now = self.now();
        if relay {
            let mut payload = self.step.to_le_bytes().to_vec();
            payload.extend([0xEB; 6]);
            let serial = self.server.send(0xEB, &payload).unwrap();
            self.relays.push((serial, payload));
        }
        self.server
            .poll(now)
            .into_iter()
            .map(|datagram| {
                self.emitted += 1;
                (self.emitted - 1, datagram)
            })
            .collect()
    }

    fn deliver_to_model(&mut self, datagram: &[u8]) {
        let now = self.now();
        self.model.receive(datagram, now);
        assert!(self.model.faults.is_empty(), "{:?}", self.model.faults);
    }

    fn model_step(&mut self) -> Vec<Vec<u8>> {
        let now = self.now();
        self.model.poll(now)
    }

    fn deliver_to_server(&mut self, datagram: &[u8]) {
        let now = self.now();
        self.server.receive(datagram, now);
    }

    /// Steps with everything delivered until nothing is left to send.
    fn settle(&mut self) {
        for _ in 0..2000 {
            for (_, datagram) in self.server_step(false) {
                self.deliver_to_model(&datagram);
            }
            for datagram in self.model_step() {
                self.deliver_to_server(&datagram);
            }
            if self.server.backlog() == 0 && self.step.is_multiple_of(50) {
                return;
            }
        }
        panic!("not settled");
    }

    /// Every relay reached the game or is reported lost, and none is
    /// reported delivered that the game lacks.
    fn check(&self) {
        let mut lost = 0;
        for (serial, payload) in &self.relays {
            let arrived = self.model.delivered.contains(&(0xEB, payload.clone()));
            assert!(
                arrived || self.server.is_lost(*serial),
                "relay {serial} vanished"
            );
            assert!(
                !self.server.is_delivered(*serial) || arrived,
                "relay {serial} reported delivered, never arrived"
            );
            lost += usize::from(self.server.is_lost(*serial));
        }
        assert!(lost < self.relays.len() / 2, "{lost} relays lost");
    }
}

/// One server datagram (`held`) reaches the game after `later` others.
fn late_datagram(held: usize, later: usize) -> Pair {
    let mut pair = Pair::new();
    let mut stash = None;
    while pair.emitted < held + later + 300 {
        for (number, datagram) in pair.server_step(true) {
            if number == held {
                stash = Some(datagram);
                continue;
            }
            pair.deliver_to_model(&datagram);
            if number == held + later {
                pair.deliver_to_model(&stash.take().expect("held back"));
            }
        }
        for datagram in pair.model_step() {
            pair.deliver_to_server(&datagram);
        }
    }
    pair.settle();
    pair.check();
    pair
}

#[test]
fn a_datagram_a_lap_late_makes_our_seq_skip_to_the_games() {
    // 129 to 255 later, the game takes it for its newest (1 to 127 ahead
    // of its newest); 128 later, when it is the lower number (seq 44
    // against 172). (Far over 200 later, ours under that seq go out
    // before its echo comes back: no skip, see the transport note.)
    for later in [129, 160, 200, 128] {
        let pair = late_datagram(300, later);
        assert_eq!(pair.server.skips(), 1, "{later} later");
    }
    // 128 later as the higher number (144 against 16): delivered, not
    // recorded; the game's newest stays, and so does our seq.
    let pair = late_datagram(400, 128);
    assert_eq!(pair.server.skips(), 0);
    // Under a lap: an older datagram the game records as such.
    let pair = late_datagram(300, 100);
    assert_eq!(pair.server.skips(), 0);
}

#[test]
fn a_late_game_datagram_does_not_make_our_seq_skip() {
    // A game datagram that arrives after 140 of ours echoes a seq that
    // reads as one a lap ahead; but it is not the game's newest.
    let mut pair = Pair::new();
    let mut stash = None;
    let mut model_sent = 0;
    while pair.emitted < 600 {
        for (_, datagram) in pair.server_step(true) {
            pair.deliver_to_model(&datagram);
        }
        for datagram in pair.model_step() {
            model_sent += 1;
            if model_sent == 60 {
                stash = Some((pair.emitted, datagram));
                continue;
            }
            pair.deliver_to_server(&datagram);
        }
        let emitted = pair.emitted;
        if let Some((_, datagram)) = stash.take_if(|(at, _)| emitted >= *at + 140) {
            pair.deliver_to_server(&datagram);
        }
    }
    pair.settle();
    pair.check();
    assert_eq!(pair.server.skips(), 0);
}

#[test]
fn a_game_that_heard_nothing_for_a_lap_does_not_make_our_seq_skip() {
    // Nothing of ours reaches the game while it goes on sending: its
    // datagrams echo its true newest, which our seq leaves 128 and more
    // behind (data stops at 100, its datagrams' acks go on). Skipping to
    // it would read its window of old datagrams as acks of the newer
    // ones under the same seqs. (Driven by hand: the model of the client
    // could not send so long unheard, nor wait so long.)
    let (mut server, start) = super::connect_match();
    let at = |step: u32| start + Duration::from_millis(20) * step;
    let mut game_seq = 0u8;
    let mut echoed = 0;
    for step in 1..=300u32 {
        server.send(0xF4, &[step as u8, 1]).unwrap();
        let datagrams = server.poll(at(step));
        assert_eq!(datagrams.len(), 1);
        echoed = datagrams[0][0];
        game_seq = game_seq.wrapping_add(1);
        server.receive(&super::flow(game_seq, echoed, &[]), at(step));
    }
    let mut unheard = Vec::new();
    let mut emitted = 0;
    let mut step = 300;
    while emitted < 140 {
        step += 1;
        unheard.push(server.send(0xF4, &[step as u8, 2]).unwrap());
        emitted += server.poll(at(step)).len();
        game_seq = game_seq.wrapping_add(1);
        server.receive(&super::flow(game_seq, echoed, &[]), at(step));
    }
    assert_eq!(server.skips(), 0);
    assert!(unheard.iter().all(|serial| !server.is_delivered(*serial)));
}

// ------------------------------------------------- order and split joins

/// Feed the server's datagrams to a fresh model in the order given by
/// `order` and return what it delivered.
fn deliver_in(datagrams: &[Vec<u8>], order: impl Iterator<Item = usize>) -> Sent {
    let now = Instant::now();
    let mut model = ClientModel::new(Table::Lobby);
    // The model has sent its sign-in: seq 0.
    model.send(0x2E, &[1, 2, 3, 4, 1]);
    model.poll(now);
    for index in order {
        model.receive(&datagrams[index], now);
    }
    assert!(model.faults.is_empty(), "{:?}", model.faults);
    model.delivered
}

#[test]
fn an_ordered_split_message_is_delivered_in_order_between_single_ones() {
    let (mut transport, now) = super::connect_lobby();
    let large: Vec<u8> = (0..2000u32).map(|i| (i % 251) as u8).collect();
    let larger: Vec<u8> = (0..1300u32).map(|i| (i % 13) as u8).collect();
    let sent: Sent = vec![
        (0x85, vec![1]),
        (0x89, large),
        (0x8B, vec![2, 2]),
        (0x87, larger),
        (0x8C, vec![3; 300]),
        (0x8D, vec![]),
    ];
    for (id, payload) in &sent {
        transport.send(*id, payload).unwrap();
    }
    // A unique id in the middle is not ordered against them.
    transport.send(0x82, &[9]).unwrap();
    let datagrams = transport.poll(now);
    assert!(datagrams.len() >= 8);
    let ordered = |delivered: Sent| -> Sent {
        delivered
            .into_iter()
            .filter(|message| message.0 != 0x82)
            .collect()
    };

    let count = datagrams.len();
    // In order, in reverse, and from the middle outwards.
    assert_eq!(ordered(deliver_in(&datagrams, 0..count)), sent);
    assert_eq!(ordered(deliver_in(&datagrams, (0..count).rev())), sent);
    let middle = (0..count).map(|i| (i * 5 + 3) % count);
    if count % 5 != 0 {
        assert_eq!(ordered(deliver_in(&datagrams, middle)), sent);
    }
    // Every datagram twice.
    let twice = (0..count).rev().chain(0..count);
    assert_eq!(ordered(deliver_in(&datagrams, twice)), sent);
    // Without the first single message nothing ordered is delivered.
    let held = deliver_in(&datagrams, 1..count);
    assert_eq!(ordered(held), vec![]);
}

#[test]
fn a_unique_split_message_is_joined_whatever_the_order() {
    let (mut transport, now) = connect_match_with_roster();
    let info: Vec<u8> = (0..3000u32).map(|i| (i % 241) as u8).collect();
    transport.send(0xE5, &[1; 7]).unwrap();
    transport.send(0xE4, &info).unwrap();
    transport.send(0xE6, &[2; 4]).unwrap();
    // Seven parts: they leave a few at a time.
    let datagrams: Vec<Vec<u8>> = (0..4)
        .flat_map(|step| transport.poll(now + Duration::from_millis(3) * step))
        .collect();
    for order in [
        (0..datagrams.len()).collect::<Vec<_>>(),
        (0..datagrams.len()).rev().collect(),
    ] {
        let mut model = ClientModel::new(Table::Match);
        model.send(0xD8, &[1, 2, 3, 4, 0]);
        model.poll(now);
        for index in order.iter().chain(&order) {
            model.receive(&datagrams[*index], now);
        }
        assert!(model.faults.is_empty(), "{:?}", model.faults);
        let whole: Vec<&(u8, Vec<u8>)> = model
            .delivered
            .iter()
            .filter(|message| message.0 == 0xE4)
            .collect();
        assert_eq!(whole, [&(0xE4, info.clone())]);
        assert_eq!(model.delivered.len(), 3);
    }
}

fn connect_match_with_roster() -> (Transport, Instant) {
    let (mut transport, now) = super::connect_match();
    transport.set_roster_size(20);
    (transport, now)
}

// ------------------------------------------------- the disconnect, both ways

#[test]
fn the_model_and_the_server_disconnect_each_other() {
    // The server disconnects: the model confirms and is gone in 500 ms.
    let mut sim = Sim::new(Table::Lobby, CLEAN, 30);
    sim.client.send(0x2E, &[1, 2, 3, 4, 1]);
    for _ in 0..20 {
        sim.step();
    }
    let now = sim.now();
    sim.server.disconnect(now);
    for _ in 0..120 {
        sim.step();
    }
    assert_eq!(sim.client.state, ModelState::Gone);
    assert_eq!(sim.server.state(), State::Closed);

    // The client leaves: the server confirms.
    let mut sim = Sim::new(Table::Match, POOR, 31);
    sim.client.send(0xD8, &[1, 2, 3, 4, 0]);
    for _ in 0..40 {
        sim.step();
    }
    sim.client.leave();
    for _ in 0..400 {
        sim.step();
    }
    assert_eq!(sim.client.state, ModelState::Gone);
    assert_eq!(sim.server.state(), State::Closed);

    // A kick: gone at once.
    let mut sim = Sim::new(Table::Match, CLEAN, 32);
    sim.client.send(0xD8, &[1, 2, 3, 4, 0]);
    for _ in 0..20 {
        sim.step();
    }
    let now = sim.now();
    sim.server.kick(now);
    for _ in 0..3 {
        sim.step();
    }
    assert_eq!(sim.client.state, ModelState::Gone);
}

// ------------------------------------------------- the model is strict

/// A model that has sent seqs 0..=`last`.
fn model_after(last: u8) -> (ClientModel, Instant) {
    let now = Instant::now();
    let mut model = ClientModel::new(Table::Match);
    model.mask_bytes = 3;
    for _ in 0..=last {
        model.send(0xE1, &[]);
    }
    assert_eq!(model.poll(now).len(), usize::from(last) + 1);
    (model, now)
}

#[test]
fn the_model_drops_what_the_client_drops() {
    let cases: [(&[u8], &str); 12] = [
        (&[0, 0], "shorter than its header"),
        (&[0, 0, 0], "empty message area"),
        (&[0, 0, 0x80, 0x00], "overlaps"),
        (&[0, 5, 0, 0xFC], "newer than the last seq sent"),
        (&[0, 0, 0, 0xE2], "invalid message type"),
        (&[0, 0, 0, 0xE3, 1, 2, 3], "extra space"),
        (&[0, 0, 0, 0xE3, 1, 2, 3, 4, 5], "extra space"),
        (&[0, 0, 0, 0xFC, 0xFC], "non-empty low level"),
        (&[0, 0, 0, 0xFE], "CRASH"),
        (
            &[0, 0, 1, 0xF6, 0, 0xFF, 2, 0, 9, 9],
            "one byte must be used",
        ),
        (
            &[0, 0, 0, 0xED, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            "inputs too short",
        ),
        (&[0, 0, 0, 0xE4, 0], "length getter failed"),
    ];
    for (datagram, fault) in cases {
        let (mut model, now) = model_after(3);
        model.receive(datagram, now);
        assert_eq!(model.faults.len(), 1, "{datagram:02X?}");
        assert!(
            model.faults[0].contains(fault),
            "{:?} for {datagram:02X?}",
            model.faults
        );
        assert!(model.delivered.is_empty());
    }
    // The lobby's null entries, and an id outside its range.
    for (id, fault) in [
        (0xA7, "CRASH"),
        (0x86, "CRASH"),
        (0x7F, "invalid"),
        (0xAB, "invalid"),
    ] {
        let mut model = ClientModel::new(Table::Lobby);
        model.send(0x2E, &[0; 5]);
        model.poll(Instant::now());
        model.receive(&[0, 0, 0, id, 0, 1], Instant::now());
        assert!(model.faults[0].contains(fault), "{:?}", model.faults);
    }
    // Acks that take back a seq acked before.
    let (mut model, now) = model_after(4);
    model.receive(&[0, 3, 0x40, 0xFC], now); // acks 3 and 2
    assert!(model.faults.is_empty());
    model.receive(&[1, 4, 0x00, 0xFC], now); // acks 4, but not 3
    assert_eq!(model.faults, ["inconsistency in acknowledgements bits"]);
    // The same datagram twice is a duplicate, not a fault.
    let (mut model, now) = model_after(0);
    let ports = [0, 0, 0, 0xE3, 1, 2, 3, 4];
    model.receive(&ports, now);
    model.receive(&ports, now);
    model.receive(&[1, 0, 0, 0xF5, 9], now);
    model.receive(&ports, now);
    assert!(model.faults.is_empty());
    assert_eq!(model.duplicates, 2);
    assert_eq!(model.delivered, [(0xE3, vec![1, 2, 3, 4]), (0xF5, vec![9])]);
}

#[test]
fn the_model_loses_an_order_id_sent_too_far_ahead() {
    // What the server's channel window prevents. The client drops an
    // ordered message that is not within its half range of the last one
    // delivered (0x210000 from 0x3F4837, inline at 0x3F4A7C) but still acks
    // its datagram, so the sender never sends it again and the channel
    // stops there for good.
    let now = Instant::now();
    let mut model = ClientModel::new(Table::Lobby);
    model.send(0x2E, &[0; 5]);
    model.poll(now);
    // Nothing delivered yet (last = 0xFF): order id 128 is 129 ahead.
    model.receive(&[0, 0, 0, 0x85, 128, 0xEE], now);
    model.receive(&[1, 0, 0, 0xFF, 0x89, 128, 0, 1, 0xEE], now);
    for order in 0..=127u8 {
        model.receive(&[order + 2, 0, 0, 0x85, order, order], now);
    }
    assert!(model.faults.is_empty());
    assert_eq!(model.delivered.len(), 128);
    assert_eq!(model.delivered[127], (0x85, vec![127]));
    // 127 ahead is held and delivered in its turn.
    let mut model = ClientModel::new(Table::Lobby);
    model.send(0x2E, &[0; 5]);
    model.poll(now);
    model.receive(&[0, 0, 0, 0x85, 126, 0xEE], now);
    for order in 0..=125u8 {
        model.receive(&[order + 1, 0, 0, 0x85, order, order], now);
    }
    assert_eq!(model.delivered.len(), 127);
    assert_eq!(model.delivered[126], (0x85, vec![0xEE]));
}
