//! A scripted 0.26g lobby client: the client's half of the transport, as
//! far as a lobby link needs it, feeding every message it takes into the
//! strict reader of `survarium::v026g::lobby::client_model`. The tests
//! drive it, and so does the test bot (`srp-bot`).
//!
//! It follows the client's wire rules (transport.md): seqs from 0 and
//! `rseq` 0xFF before anything came; one message a datagram; every request
//! under the next unique id; a datagram seen before is dropped; a unique
//! message is delivered once; an ordered one that is not newer than the
//! last delivered is **dropped although its datagram is acked** (the
//! hazard of §4.1), and one whose predecessor is missing waits; split
//! messages are joined by their id; `FC` when idle; `FD` and `FA` end the
//! link. Anything the real client would drop, log as an error or crash on
//! is kept in `faults`: a correct server leaves it empty.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use survarium::v026g::lobby::client_model::{Client, Decoded};
use udp_transport::seq::{Recorded, Window};
use udp_transport::{Class, Direction, Table, ack, wire};

/// The client sends a keep-alive after this long with nothing to send. (The
/// real one waits 500 ms; sooner is allowed, and keeps the tests short.)
const IDLE: Duration = Duration::from_millis(40);
/// The real client's wait before a keep-alive: for a scripted client that
/// stays for hours ([`TestClient::idle`]).
pub const CLIENT_IDLE: Duration = Duration::from_millis(500);
/// A lobby request is sent again after a second unacked (transport.md
/// §4.1).
const RESEND: Duration = Duration::from_millis(1000);

/// The client's compare at 0x210000: `b` is not newer than `a`.
fn not_newer(a: u8, b: u8) -> bool {
    if a < b {
        u32::from(a) + 128 <= u32::from(b)
    } else {
        u32::from(b) + 128 > u32::from(a)
    }
}

struct Outgoing {
    /// `id x body`.
    bytes: Vec<u8>,
    /// The seqs it went out under.
    seqs: Vec<u8>,
    sent_at: Option<Instant>,
}

/// A message of a datagram, copied out of it.
struct Taken {
    id: u8,
    low_level: bool,
    class: Class,
    x: Option<u8>,
    part: Option<wire::Part>,
    payload: Vec<u8>,
}

#[derive(Default)]
struct Assembly {
    count: Option<u8>,
    parts: BTreeMap<u8, Vec<u8>>,
}

pub struct TestClient {
    next_seq: u8,
    sent_any: bool,
    received: Window,
    next_unique: u8,
    unique_in: Window,
    order_last: u8,
    order_waiting: BTreeMap<u8, (u8, Vec<u8>)>,
    assemblies: HashMap<(bool, u8), Assembly>,
    outgoing: Vec<Outgoing>,
    low_level: Vec<u8>,
    last_send: Option<Instant>,
    /// A keep-alive (`FC`) goes after this long with nothing to send.
    pub idle: Duration,
    /// The strict reader of the lobby's messages.
    pub model: Client,
    /// Every message taken, in the order the client's game code got it.
    pub decoded: Vec<Decoded>,
    /// The ids of the same messages.
    pub ids: Vec<u8>,
    /// Every 0x9F taken: the players and matches online and the minutes to
    /// maintenance.
    pub online: Vec<(u32, u32, u32)>,
    /// What a correct server never causes.
    pub faults: Vec<String>,
    /// The server disconnected the client (`FD` or `FA`).
    pub disconnected: bool,
    /// Drop this share of the server's datagrams (0..=100), by a fixed
    /// pattern.
    pub loss_percent: u32,
    loss_state: u32,
    pub dropped: u32,
    /// Ordered messages dropped as "not newer": with a server that keeps
    /// the window, only late copies of delivered ones.
    pub order_drops: u32,
}

impl TestClient {
    pub fn new() -> Self {
        Self {
            next_seq: 0,
            sent_any: false,
            received: Window::default(),
            next_unique: 0,
            unique_in: Window::default(),
            order_last: 0xFF,
            order_waiting: BTreeMap::new(),
            assemblies: HashMap::new(),
            outgoing: Vec::new(),
            low_level: Vec::new(),
            last_send: None,
            idle: IDLE,
            model: Client::new(),
            decoded: Vec::new(),
            ids: Vec::new(),
            online: Vec::new(),
            faults: Vec::new(),
            disconnected: false,
            loss_percent: 0,
            loss_state: 0x2545_F491,
            dropped: 0,
            order_drops: 0,
        }
    }

    /// The same game on a new link (`Client::reconnect`): the transport
    /// starts anew, what the game holds stays. What it took before is kept
    /// in `decoded` and `faults`.
    pub fn reconnected(self) -> Self {
        let mut model = self.model;
        model.reconnect();
        Self {
            model,
            decoded: self.decoded,
            ids: self.ids,
            faults: self.faults,
            ..Self::new()
        }
    }

    /// `2E` as the game itself would send it: it asks for the shop only
    /// while it holds none.
    pub fn sign_in_as_held(&mut self, session_id: u32) {
        let need_shop_items = self.model.needs_shop_items();
        self.sign_in(session_id, need_shop_items);
    }

    /// Ordered messages that wait for one before them.
    pub fn waiting_in_order(&self) -> usize {
        self.order_waiting.len()
    }

    // ------------------------------------------------------------- sending

    /// Queue a request: its id and what follows the transport's x byte.
    pub fn send(&mut self, id: u8, body: &[u8]) {
        let mut bytes = vec![id, self.next_unique];
        self.next_unique = self.next_unique.wrapping_add(1);
        bytes.extend_from_slice(body);
        self.outgoing.push(Outgoing {
            bytes,
            seqs: Vec::new(),
            sent_at: None,
        });
    }

    /// `2E`: the connect payload.
    pub fn sign_in(&mut self, session_id: u32, need_shop_items: bool) {
        let mut body = session_id.to_le_bytes().to_vec();
        body.push(if need_shop_items { 0xFF } else { 0 });
        self.send(0x2E, &body);
    }

    /// Leave as the client does: `FA` until the confirm.
    pub fn leave(&mut self) {
        self.low_level.push(0xFA);
    }

    fn datagram(&mut self, body: &[u8]) -> Vec<u8> {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.sent_any = true;
        let tail = ack::MAX_TAIL.min(wire::MAX_DATAGRAM - wire::HEADER - body.len());
        let (ack0, tail) = ack::encode(self.received.bits() | 1 << 127, tail, false);
        let mut datagram = vec![seq, self.received.newest().unwrap_or(0xFF), ack0];
        datagram.extend_from_slice(body);
        datagram.extend(tail);
        datagram
    }

    /// The datagrams to send now.
    pub fn poll(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut datagrams = Vec::new();
        for id in std::mem::take(&mut self.low_level) {
            datagrams.push(self.datagram(&[id]));
        }
        if self.disconnected {
            return datagrams;
        }
        for index in 0..self.outgoing.len() {
            let due = self.outgoing[index]
                .sent_at
                .is_none_or(|at| now.saturating_duration_since(at) >= RESEND);
            if !due {
                continue;
            }
            let bytes = self.outgoing[index].bytes.clone();
            let seq = self.next_seq;
            datagrams.push(self.datagram(&bytes));
            let message = &mut self.outgoing[index];
            message.seqs.push(seq);
            message.sent_at = Some(now);
        }
        let quiet = self
            .last_send
            .is_none_or(|at| now.saturating_duration_since(at) >= self.idle);
        // Nothing is sent before the connect payload.
        if datagrams.is_empty() && quiet && self.sent_any {
            datagrams.push(self.datagram(&[0xFC]));
        }
        if !datagrams.is_empty() {
            self.last_send = Some(now);
        }
        datagrams
    }

    // ----------------------------------------------------------- receiving

    fn lose(&mut self) -> bool {
        if self.loss_percent == 0 {
            return false;
        }
        // xorshift: the same pattern every run.
        self.loss_state ^= self.loss_state << 13;
        self.loss_state ^= self.loss_state >> 17;
        self.loss_state ^= self.loss_state << 5;
        self.loss_state % 100 < self.loss_percent
    }

    /// One datagram from the server.
    pub fn receive(&mut self, datagram: &[u8]) {
        if self.disconnected {
            return;
        }
        if self.lose() {
            self.dropped += 1;
            return;
        }
        if datagram.len() > wire::MAX_DATAGRAM {
            self.faults
                .push(format!("a datagram of {} bytes", datagram.len()));
            return;
        }
        let parsed = match wire::parse(Table::Lobby, Direction::ToClient, datagram, 0) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.faults
                    .push(format!("a datagram the client drops: {error:?}"));
                return;
            }
        };
        // An echo of a seq never sent: dropped whole (0x3F1C40).
        let last_sent = self.next_seq.wrapping_sub(1);
        let ahead = parsed.rseq.wrapping_sub(last_sent);
        if !self.sent_any || (ahead != 0 && ahead < 128) {
            self.faults
                .push(format!("rseq {} was never sent", parsed.rseq));
            return;
        }
        // The acks: every request that went out under an acked seq.
        let (rseq, bits, count) = (parsed.rseq, parsed.acks.bits, parsed.acks.count);
        self.outgoing.retain(|message| {
            !message.seqs.iter().any(|seq| {
                let back = u32::from(rseq.wrapping_sub(*seq));
                back < count && bits >> (127 - back) & 1 != 0
            })
        });
        if self.received.record(parsed.seq) != Recorded::New {
            return;
        }
        let messages: Vec<Taken> = parsed
            .messages
            .iter()
            .map(|message| Taken {
                id: message.id,
                low_level: message.low_level,
                class: message.class,
                x: message.x,
                part: message.part,
                payload: message.payload.to_vec(),
            })
            .collect();
        for Taken {
            id,
            low_level,
            class,
            x,
            part,
            payload,
        } in messages
        {
            if low_level {
                match id {
                    0xFC => {}
                    0xFA => {
                        self.low_level.push(0xFB);
                        self.disconnected = true;
                    }
                    0xFB | 0xFD => self.disconnected = true,
                    other => self
                        .faults
                        .push(format!("CRASH: low-level id {other:#04x}")),
                }
                continue;
            }
            match (class, x) {
                (Class::Plain, _) => self.deliver(id, &payload),
                (Class::Unique, Some(x)) => {
                    if self.unique_in.contains(x) {
                        continue;
                    }
                    let Some(whole) = self.join(false, x, part, payload) else {
                        continue;
                    };
                    self.unique_in.record(x);
                    self.deliver(id, &whole);
                }
                (Class::Ordered, Some(x)) => {
                    if not_newer(self.order_last, x) {
                        self.order_drops += 1;
                        continue;
                    }
                    if self.order_waiting.contains_key(&x) {
                        continue;
                    }
                    let Some(whole) = self.join(true, x, part, payload) else {
                        continue;
                    };
                    self.order_waiting.insert(x, (id, whole));
                    loop {
                        let next = self.order_last.wrapping_add(1);
                        let Some((id, payload)) = self.order_waiting.remove(&next) else {
                            break;
                        };
                        self.order_last = next;
                        self.deliver(id, &payload);
                    }
                }
                (_, None) => self.faults.push(format!("id {id:#04x} without its x byte")),
            }
        }
    }

    /// A whole message: itself, or a split one once every part is there.
    fn join(
        &mut self,
        ordered: bool,
        x: u8,
        part: Option<wire::Part>,
        payload: Vec<u8>,
    ) -> Option<Vec<u8>> {
        let Some(part) = part else {
            return Some(payload);
        };
        let assembly = self.assemblies.entry((ordered, x)).or_default();
        if part.index == 0 {
            assembly.count = part.count;
        }
        assembly.parts.entry(part.index).or_insert(payload);
        let count = usize::from(assembly.count?);
        if assembly.parts.len() < count {
            return None;
        }
        let assembly = self.assemblies.remove(&(ordered, x))?;
        Some(assembly.parts.into_values().flatten().collect())
    }

    fn deliver(&mut self, id: u8, payload: &[u8]) {
        match self.model.receive_raw(id, payload) {
            // The online counts are repeated every few seconds: kept apart,
            // so the rest reads as what a request brought.
            Ok(Decoded::OnlineStats {
                players_online,
                games_online,
                minutes_to_maintenance,
            }) => self
                .online
                .push((players_online, games_online, minutes_to_maintenance)),
            Ok(decoded) => {
                self.ids.push(id);
                self.decoded.push(decoded);
            }
            Err(error) => self.faults.push(format!(
                "message {id:#04X} the client can't take: {error:?}"
            )),
        }
    }
}

/// The client's requests: `(id, body)` as `lobby_requests.md` lays them
/// out.
pub mod request {
    fn string(out: &mut Vec<u8>, text: &str) {
        out.push(text.len() as u8);
        out.extend_from_slice(text.as_bytes());
    }

    pub fn play(kit_id: u32) -> (u8, Vec<u8>) {
        (0x2F, kit_id.to_le_bytes().to_vec())
    }

    pub fn leave(order_id: u32, left_match: bool) -> (u8, Vec<u8>) {
        let mut body = order_id.to_le_bytes().to_vec();
        body.push(if left_match { 0xFF } else { 0 });
        (0x31, body)
    }

    /// `46 00`: one move of an item.
    pub fn move_item(
        kit_id: u32,
        item_id: u32,
        dict_id: u16,
        source: u8,
        target: u8,
        count: u32,
    ) -> (u8, Vec<u8>) {
        let mut body = vec![0];
        body.extend(kit_id.to_le_bytes());
        body.push(1);
        body.extend(item_id.to_le_bytes());
        body.extend(dict_id.to_le_bytes());
        body.extend([source, target]);
        body.extend(count.to_le_bytes());
        body.extend(0_u32.to_le_bytes());
        (0x46, body)
    }

    pub fn buy(dict_id: u16, count: u32, faction: u8, slot: u8, kit_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![0];
        body.extend(dict_id.to_le_bytes());
        body.extend(count.to_le_bytes());
        body.extend([faction, 0, slot]);
        body.extend(kit_id.to_le_bytes());
        (0x47, body)
    }

    pub fn sell(item_id: u32, amount: u32) -> (u8, Vec<u8>) {
        let mut body = vec![1];
        body.extend(item_id.to_le_bytes());
        body.extend(amount.to_le_bytes());
        (0x47, body)
    }

    /// `47 03`, paid in silver (0) or gold (1).
    pub fn repair(item_id: u32, kind: u8) -> (u8, Vec<u8>) {
        let mut body = vec![3];
        body.extend(item_id.to_le_bytes());
        body.push(kind);
        (0x47, body)
    }

    pub fn exchange(gold: u32) -> (u8, Vec<u8>) {
        let mut body = vec![4];
        body.extend(gold.to_le_bytes());
        (0x47, body)
    }

    pub fn skills(skills: &[(u16, u8)]) -> (u8, Vec<u8>) {
        let mut body = vec![0, skills.len() as u8];
        for (id, points) in skills {
            body.extend(id.to_le_bytes());
            body.push(*points);
        }
        (0x48, body)
    }

    pub fn reset_skills(branch: u8, payment: u8) -> (u8, Vec<u8>) {
        (0x48, vec![1, branch, payment])
    }

    pub fn autobuy(on: bool) -> (u8, Vec<u8>) {
        (0x49, vec![1, u8::from(on)])
    }

    pub fn active_kit(kit_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![2];
        body.extend(kit_id.to_le_bytes());
        (0x49, body)
    }

    /// `49 <sub> <string>`: a nickname or clan name check, an invite.
    pub fn profile_text(sub: u8, text: &str) -> (u8, Vec<u8>) {
        let mut body = vec![sub];
        string(&mut body, text);
        (0x49, body)
    }

    pub fn add_squad_member(name: &str) -> (u8, Vec<u8>) {
        let mut body = vec![2];
        string(&mut body, name);
        (0x4A, body)
    }

    pub fn leave_squad() -> (u8, Vec<u8>) {
        (0x4A, vec![4])
    }

    pub fn quest_faction(faction: u8) -> (u8, Vec<u8>) {
        (0x4B, vec![0, faction])
    }

    pub fn chat(channel_id: u32, receiver: &str, channel: u8, body_text: &str) -> (u8, Vec<u8>) {
        let mut body = channel_id.to_le_bytes().to_vec();
        string(&mut body, receiver);
        body.push(channel);
        string(&mut body, body_text);
        (0x4C, body)
    }

    /// `4D`: type 0 a friend request, 2 a squad invite (`invite`), 3 its
    /// acceptance (`membership accepted`).
    pub fn important(target: &str, kind: u8, text: &str) -> (u8, Vec<u8>) {
        let mut body = 0_u32.to_le_bytes().to_vec();
        string(&mut body, target);
        body.push(kind);
        string(&mut body, text);
        (0x4D, body)
    }

    pub fn remove_friend(account_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![2];
        body.extend(account_id.to_le_bytes());
        (0x4E, body)
    }

    pub fn ignore(account_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![3];
        body.extend(account_id.to_le_bytes());
        (0x4E, body)
    }

    pub fn chat_mute(
        name: &str,
        mode: Option<survarium::v026g::lobby::ChatMuteMode>,
    ) -> (u8, Vec<u8>) {
        let mut body = vec![5];
        string(&mut body, name);
        body.extend([
            survarium::v026g::lobby::CHAT_MUTE_ACTION,
            mode.map_or(0, |mode| mode as u8),
        ]);
        (0x49, body)
    }

    pub fn find_players(name: &str) -> (u8, Vec<u8>) {
        let mut body = vec![6];
        string(&mut body, name);
        (0x4E, body)
    }

    /// `4F`: the general channel in the client's language room, the rest
    /// as the client starts with them.
    pub fn subscribe() -> (u8, Vec<u8>) {
        let ids: [u32; 10] = [0, 2, u32::MAX, 0, 1, 0, 0, 0, 0, 0];
        let mut body: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
        body.push(0);
        (0x4F, body)
    }

    pub fn answer(message_id: u32, answer: u8) -> (u8, Vec<u8>) {
        let mut body = message_id.to_le_bytes().to_vec();
        body.push(answer);
        (0x50, body)
    }

    /// A query with no argument: `id 00`.
    pub fn query(id: u8) -> (u8, Vec<u8>) {
        (id, vec![0])
    }

    pub fn squad_member(account_id: u32, kit_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![0];
        body.extend(account_id.to_le_bytes());
        body.extend(kit_id.to_le_bytes());
        (0x34, body)
    }

    /// `41 00 <match>`: the results screen again.
    pub fn match_stats(match_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![0];
        body.extend(match_id.to_le_bytes());
        (0x41, body)
    }

    /// `3D 00 <player> <match>`: a row of the results screen.
    pub fn player_details(player_id: u32, match_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![0];
        body.extend(player_id.to_le_bytes());
        body.extend(match_id.to_le_bytes());
        (0x3D, body)
    }

    /// `46 03 <match> <card item>` (the client's order).
    pub fn use_drop_card(card_item_id: u32, match_id: u32) -> (u8, Vec<u8>) {
        let mut body = vec![3];
        body.extend(match_id.to_le_bytes());
        body.extend(card_item_id.to_le_bytes());
        (0x46, body)
    }

    /// `47 02 <offer> <x> <f32 0.0>`: an offer; from the results screen
    /// `x` is the match.
    pub fn buy_offer(offer_id: u32, x: u32) -> (u8, Vec<u8>) {
        let mut body = vec![2];
        body.extend(offer_id.to_le_bytes());
        body.extend(x.to_le_bytes());
        body.extend(0.0_f32.to_le_bytes());
        (0x47, body)
    }

    pub fn ranking(start: u32, count: u8) -> (u8, Vec<u8>) {
        let mut body = vec![0];
        body.extend(start.to_le_bytes());
        body.push(count);
        (0x3B, body)
    }
}
