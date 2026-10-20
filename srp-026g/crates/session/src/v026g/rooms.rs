//! Rooms (the players call them servers): a player-made place in the lobby
//! where players wait together, vote on the next map and play match after
//! match. The owner's rules of 2026-10-16:
//!
//! - A room is made by a player with a team size (6 v 6 to 10 v 10), a
//!   max level (the highest equipment
//!   level let in, 0 for any, never under its maker's own), a wait (15 s to a minute), an optional
//!   PIN and a pool of maps. It is named after its maker and cannot be
//!   changed once made. Its owner may take a member out.
//! - Once two players are in it who can play (a kit with a weapon and its
//!   ammo, equipment within the max level; one who cannot is marked red in
//!   the room, counts for nothing, and is taken out of the room when the
//!   match starts without it), the vote runs for 15 s ([`VOTE`]) and the
//!   room's own wait together, always whole, a full room too: its players
//!   may still be changing their equipment (the owner's of 2026-10-17). The
//!   vote stays open until the match starts, and its winner then is the
//!   match's map (the owner's of 2026-10-19: no time between the vote and
//!   the start in which the map is fixed). The vote is always there: three
//!   cards drawn from the pool when the room is made, and the next three as
//!   soon as a match starts (its players vote during it); a member may
//!   change its vote. Each card is a map with its weather and mode
//!   (a `maps_dict` id); the card with the most votes is played, a tie and no
//!   vote at all are drawn at random.
//! - There are no teams in the room: the sides are drawn when the match
//!   starts (squads together, the sides as even as they can be).
//! - After the match everyone is back in the room, and the next vote and
//!   wait begin.
//! - Play with nothing else chosen puts a player into an open room (no
//!   PIN, a place free; [`Rooms::open_rooms`]), or makes a new one.
//!
//! This module keeps the rooms and their rules. The lobby asks it, does
//! what it says (sends players to a match, tells the clients) and tells it
//! what came of that; nothing here sends or locks anything else.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// How long a pool sent in pieces before a create waits for that create.
pub const DRAFT_LIFE: Duration = Duration::from_secs(120);
/// How long a PIN given before a JOIN waits for that JOIN.
pub const PIN_LIFE: Duration = Duration::from_secs(60);
/// The fewest players a vote (and so a match) starts with.
pub const START_PLAYERS: usize = 2;
/// The cards of a vote.
pub const CARDS: usize = 3;
/// The pool entries one piece of a draft carries.
pub const DRAFT_CHUNK: usize = 20;
/// The keys new rooms are given first: four digits, easy to say.
const SHORT_KEYS: std::ops::RangeInclusive<u32> = 1000..=9999;
/// The largest key.
const KEY_MAX: u32 = 99_999;

/// What a room is made with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub size: u8,
    /// The highest equipment level let in; 0: any.
    pub max_level: u8,
    pub wait: u8,
    pub pin: Option<u16>,
    /// The players it needs before its vote begins: 2 (1 v 1), 4 or 6.
    pub min_players: u8,
}

/// The vote's own time before each match: always this long, whatever the
/// room's wait (the owner's of 2026-10-17).
pub const VOTE: Duration = Duration::from_secs(15);
/// What the vote after a match has more: its players come back from the
/// results screen first (the owner's of 2026-10-17).
pub const AFTER_MATCH: Duration = Duration::from_secs(10);

/// Where a room is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Fewer than its `min_players` who can play: nothing runs.
    Waiting,
    /// The vote on the cards runs until `until`, when the match starts on
    /// its winner: [`VOTE`] and the room's own wait together (the owner's
    /// of 2026-10-19: the vote goes on until the match starts, its time
    /// added to the wait).
    Vote { until: Instant },
    /// Its match (the queue's id) is being played since `since`.
    InMatch { match_id: u32, since: Instant },
}

/// Why a request about rooms was not done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Unknown,
    WrongPin,
    Full,
    TooMany,
    Bad,
    Kicked,
    /// The player's equipment level is over the room's max level.
    GearTooHigh,
}

/// A room.
#[derive(Clone, Debug)]
pub struct Room {
    pub key: u32,
    pub owner: u32,
    /// Who made it: the room is named after it, whoever owns it now.
    pub maker: u32,
    pub spec: Spec,
    /// The `maps_dict` ids the vote's cards are drawn from.
    pub pool: Vec<u8>,
    /// Its members' accounts, in the order they came.
    pub members: Vec<u32>,
    pub stage: Stage,
    /// The cards of the vote now; empty while it waits.
    pub cards: Vec<u8>,
    /// Each voter's card.
    pub votes: HashMap<u32, u8>,
    /// The map of its last match; 0: none yet.
    pub last_map: u8,
    /// Its match just ended: the next vote is [`AFTER_MATCH`] longer.
    pub after_match: bool,
    /// The members in its match now.
    pub in_match: HashSet<u32>,
    /// Changes whenever a member comes or goes.
    pub serial: u32,
    /// Who its owner took out: they may not come back.
    pub kicked: HashSet<u32>,
}

impl Room {
    /// Each side's players times two.
    pub fn capacity(&self) -> usize {
        2 * usize::from(self.spec.size)
    }

    /// The players it needs before its vote begins (two at least).
    pub fn needs(&self) -> usize {
        usize::from(self.spec.min_players).max(START_PLAYERS)
    }

    pub fn is_full(&self) -> bool {
        self.members.len() >= self.capacity()
    }

    pub fn is_open(&self) -> bool {
        self.spec.pin.is_none()
    }

    /// Whether a player of equipment level `gear` is let in.
    pub fn lets_in(&self, gear: u8) -> bool {
        self.spec.max_level == 0 || gear <= self.spec.max_level
    }

    /// The votes each card has.
    pub fn tally(&self) -> Vec<u8> {
        let mut tally = vec![0_u8; self.cards.len()];
        for card in self.votes.values() {
            if let Some(count) = tally.get_mut(usize::from(*card)) {
                *count = count.saturating_add(1);
            }
        }
        tally
    }

    /// The card that leads the vote alone: what the room list shows before
    /// the start. `None` on a tie or with no votes (the start then draws
    /// one at random, [`Room::winner`]).
    pub fn leading(&self) -> Option<u8> {
        let tally = self.tally();
        let most = *tally.iter().max()?;
        if most == 0 || tally.iter().filter(|&&votes| votes == most).count() > 1 {
            return None;
        }
        let index = tally.iter().position(|&votes| votes == most)?;
        self.cards.get(index).copied()
    }

    /// The map the vote chose: the card with the most votes; of several
    /// alike (or none voted for) one at random. `None` with no cards.
    pub fn winner(&self, below: &mut impl FnMut(usize) -> usize) -> Option<u8> {
        let tally = self.tally();
        let most = *tally.iter().max()?;
        let tied: Vec<usize> = (0..tally.len()).filter(|&i| tally[i] == most).collect();
        let pick = tied[below(tied.len()).min(tied.len() - 1)];
        self.cards.get(pick).copied()
    }
}

/// What the lobby must do now ([`Rooms::tick`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    /// The room's vote and wait are over: its match is to start on `map_id`,
    /// the vote's winner then.
    Start { key: u32, map_id: u8 },
}

/// A pool being sent in pieces before its create.
#[derive(Clone, Debug)]
struct Draft {
    chunks: [u32; 8],
    seen: u8,
    at: Instant,
}

/// The rooms of the lobby.
#[derive(Debug, Default)]
pub struct Rooms {
    rooms: BTreeMap<u32, Room>,
    of_account: HashMap<u32, u32>,
    /// A full room's queue, by its key: who waits for a place, the first
    /// first (the owner's of 2026-10-17: "Queue").
    queues: HashMap<u32, VecDeque<u32>>,
    /// The room an account waits for: one at a time, from the lobby only.
    queued: HashMap<u32, u32>,
    drafts: HashMap<u32, Draft>,
    pins: HashMap<u32, (u16, Instant)>,
    serial: u32,
}

impl Rooms {
    pub fn len(&self) -> usize {
        self.rooms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }

    pub fn get(&self, key: u32) -> Option<&Room> {
        self.rooms.get(&key)
    }

    pub fn all(&self) -> impl Iterator<Item = &Room> {
        self.rooms.values()
    }

    /// The account's room.
    pub fn of(&self, account_id: u32) -> Option<&Room> {
        self.rooms.get(self.of_account.get(&account_id)?)
    }

    pub fn key_of(&self, account_id: u32) -> Option<u32> {
        self.of_account.get(&account_id).copied()
    }

    fn next_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1);
        self.serial
    }

    /// A piece of the pool of the account's next room: entries `chunk ×
    /// 20` to `chunk × 20 + 19`, each in by its bit. The entries in so far.
    pub fn draft(&mut self, account_id: u32, chunk: u8, bits: u32, now: Instant) -> u32 {
        let draft = self
            .drafts
            .entry(account_id)
            .and_modify(|draft| {
                if now.saturating_duration_since(draft.at) > DRAFT_LIFE {
                    draft.chunks = [0; 8];
                    draft.seen = 0;
                }
            })
            .or_insert(Draft {
                chunks: [0; 8],
                seen: 0,
                at: now,
            });
        if let Some(slot) = draft.chunks.get_mut(usize::from(chunk)) {
            *slot = bits & ((1 << DRAFT_CHUNK) - 1);
            draft.seen |= 1 << chunk;
        }
        draft.at = now;
        draft.chunks.iter().map(|chunk| chunk.count_ones()).sum()
    }

    /// The pool the account's draft names among `entries` (the maps a pool
    /// may hold, in their order), taken up; `None` when it sent none lately.
    fn take_draft(&mut self, account_id: u32, entries: &[u8], now: Instant) -> Option<Vec<u8>> {
        let draft = self.drafts.remove(&account_id)?;
        if now.saturating_duration_since(draft.at) > DRAFT_LIFE || draft.seen == 0 {
            return None;
        }
        Some(
            entries
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    draft
                        .chunks
                        .get(index / DRAFT_CHUNK)
                        .is_some_and(|chunk| chunk >> (index % DRAFT_CHUNK) & 1 == 1)
                })
                .map(|(_, &map_id)| map_id)
                .collect(),
        )
    }

    /// The PIN the account's next JOIN gives.
    pub fn give_pin(&mut self, account_id: u32, pin: u16, now: Instant) {
        self.pins.insert(account_id, (pin, now));
    }

    /// The PIN the account gave last, within [`PIN_LIFE`], taken up.
    pub fn take_pin(&mut self, account_id: u32, now: Instant) -> Option<u16> {
        let (pin, at) = self.pins.remove(&account_id)?;
        (now.saturating_duration_since(at) <= PIN_LIFE).then_some(pin)
    }

    /// A key no room has: four digits first, at random.
    fn new_key(&self, below: &mut impl FnMut(usize) -> usize) -> Option<u32> {
        let short = SHORT_KEYS.end() - SHORT_KEYS.start() + 1;
        for _ in 0..64 {
            let key = SHORT_KEYS.start() + below(short as usize) as u32;
            if !self.rooms.contains_key(&key) {
                return Some(key);
            }
        }
        (1..=KEY_MAX).find(|key| !self.rooms.contains_key(key))
    }

    /// The account makes a room: it leaves the one it is in, and owns the
    /// new one. `entries` are the maps a pool may hold; the room's pool is
    /// the account's draft of them, or the `default` pool with none. The
    /// owner's equipment level `gear` must be within the room's max level.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &mut self,
        owner: u32,
        gear: u8,
        spec: Spec,
        entries: &[u8],
        default: &[u8],
        most_rooms: usize,
        now: Instant,
        below: &mut impl FnMut(usize) -> usize,
    ) -> Result<u32, Refused> {
        let pool = self
            .take_draft(owner, entries, now)
            .unwrap_or_else(|| default.to_vec());
        if pool.is_empty() || spec.size == 0 {
            return Err(Refused::Bad);
        }
        if spec.max_level != 0 && gear > spec.max_level {
            return Err(Refused::GearTooHigh);
        }
        if self.rooms.len() >= most_rooms
            && self.of(owner).is_none_or(|room| room.members.len() > 1)
        {
            return Err(Refused::TooMany);
        }
        self.leave(owner);
        let key = self.new_key(below).ok_or(Refused::TooMany)?;
        let serial = self.next_serial();
        // The vote is always there: its cards from the start.
        let cards = Self::draw(&pool, below);
        self.rooms.insert(
            key,
            Room {
                key,
                owner,
                maker: owner,
                spec,
                pool,
                members: vec![owner],
                stage: Stage::Waiting,
                cards,
                votes: HashMap::new(),
                last_map: 0,
                after_match: false,
                in_match: HashSet::new(),
                serial,
                kicked: HashSet::new(),
            },
        );
        self.of_account.insert(owner, key);
        self.dequeue(owner);
        Ok(key)
    }

    /// The account joins room `key`, with the PIN it gave last (within
    /// [`PIN_LIFE`]) and its equipment level `gear`; it leaves the room it
    /// was in. Joining the room it is in already is no change.
    pub fn join(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
    ) -> Result<(), Refused> {
        let pin = self.take_pin(account_id, now);
        let room = self.rooms.get(&key).ok_or(Refused::Unknown)?;
        if room.members.contains(&account_id) {
            return Ok(());
        }
        if room.kicked.contains(&account_id) {
            return Err(Refused::Kicked);
        }
        if room.spec.pin.is_some_and(|wanted| pin != Some(wanted)) {
            return Err(Refused::WrongPin);
        }
        if !room.lets_in(gear) {
            return Err(Refused::GearTooHigh);
        }
        if room.is_full() {
            return Err(Refused::Full);
        }
        self.leave(account_id);
        let serial = self.next_serial();
        let Some(room) = self.rooms.get_mut(&key) else {
            return Err(Refused::Unknown);
        };
        room.members.push(account_id);
        room.serial = serial;
        self.of_account.insert(account_id, key);
        // In a room: no queue any more.
        self.dequeue(account_id);
        Ok(())
    }

    /// The rooms Play may put the account into: no PIN, a place free, its
    /// max level not under the account's equipment level `gear`, not one
    /// that turned it out.
    pub fn open_rooms(&self, account_id: u32, gear: u8) -> impl Iterator<Item = &Room> {
        self.rooms.values().filter(move |room| {
            room.is_open()
                && !room.is_full()
                && room.lets_in(gear)
                && !room.kicked.contains(&account_id)
        })
    }

    /// The account leaves its room. A room nobody is left in is gone; an
    /// owner who leaves hands the room to the member who came first. The
    /// room it left, if any.
    pub fn leave(&mut self, account_id: u32) -> Option<u32> {
        let key = self.of_account.remove(&account_id)?;
        let serial = self.next_serial();
        let room = self.rooms.get_mut(&key)?;
        room.members.retain(|member| *member != account_id);
        room.votes.remove(&account_id);
        room.in_match.remove(&account_id);
        room.serial = serial;
        if room.members.is_empty() {
            self.rooms.remove(&key);
            // Its queue goes with it: nobody waits for a room that is gone.
            for waiting in self.queues.remove(&key).unwrap_or_default() {
                self.queued.remove(&waiting);
            }
        } else if room.owner == account_id {
            room.owner = room.members[0];
        }
        Some(key)
    }

    /// The room is gone, all its members out of it (a maintenance): who
    /// they were.
    pub fn disband(&mut self, key: u32) -> Vec<u32> {
        let Some(room) = self.rooms.remove(&key) else {
            return Vec::new();
        };
        for member in &room.members {
            self.of_account.remove(member);
        }
        let mut gone = room.members;
        for waiting in self.queues.remove(&key).unwrap_or_default() {
            self.queued.remove(&waiting);
            gone.push(waiting);
        }
        gone
    }

    /// The account waits in the full room's queue: it is in no room (the
    /// lobby only), the room lets it in (its PIN given just before, its
    /// max level, not turned out). One queue at a time: a new one takes it
    /// out of the last. Its place, 1 the first.
    pub fn enqueue(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
    ) -> Result<usize, Refused> {
        let pin = self.take_pin(account_id, now);
        if self.of_account.contains_key(&account_id) {
            return Err(Refused::Bad);
        }
        let room = self.rooms.get(&key).ok_or(Refused::Unknown)?;
        if room.kicked.contains(&account_id) {
            return Err(Refused::Kicked);
        }
        if room.spec.pin.is_some_and(|wanted| pin != Some(wanted)) {
            return Err(Refused::WrongPin);
        }
        if !room.lets_in(gear) {
            return Err(Refused::GearTooHigh);
        }
        if self.queued.get(&account_id) != Some(&key) {
            self.dequeue(account_id);
            self.queues.entry(key).or_default().push_back(account_id);
            self.queued.insert(account_id, key);
        }
        Ok(self.queue_place(account_id).map_or(0, |(_, place)| place))
    }

    /// The account leaves the queue it waits in, if any: its room's key.
    pub fn dequeue(&mut self, account_id: u32) -> Option<u32> {
        let key = self.queued.remove(&account_id)?;
        if let Some(queue) = self.queues.get_mut(&key) {
            queue.retain(|waiting| *waiting != account_id);
            if queue.is_empty() {
                self.queues.remove(&key);
            }
        }
        Some(key)
    }

    /// The room an account waits for and its place there, 1 the first.
    pub fn queue_place(&self, account_id: u32) -> Option<(u32, usize)> {
        let key = *self.queued.get(&account_id)?;
        let place = self
            .queues
            .get(&key)?
            .iter()
            .position(|w| *w == account_id)?;
        Some((key, place + 1))
    }

    /// How many wait in the room's queue.
    pub fn queue_len(&self, key: u32) -> usize {
        self.queues.get(&key).map_or(0, VecDeque::len)
    }

    /// How many wait in all the queues.
    pub fn queued_total(&self) -> usize {
        self.queued.len()
    }

    /// The first of the room's queue, while it has a place free: out of the
    /// queue, for the lobby to put in (`admit`) if it still may come.
    pub fn next_in_queue(&mut self, key: u32) -> Option<u32> {
        let room = self.rooms.get(&key)?;
        if room.is_full() {
            return None;
        }
        let first = self.queues.get_mut(&key)?.pop_front()?;
        self.queued.remove(&first);
        if self.queues.get(&key).is_some_and(VecDeque::is_empty) {
            self.queues.remove(&key);
        }
        Some(first)
    }

    /// The rooms with a queue and a place free.
    pub fn with_queue_and_place(&self) -> Vec<u32> {
        self.queues
            .keys()
            .copied()
            .filter(|key| self.rooms.get(key).is_some_and(|room| !room.is_full()))
            .collect()
    }

    /// The first of a queue goes in: its checks were made when it queued
    /// (PIN, turned out) and by the lobby now (in the lobby, its level).
    pub fn admit(&mut self, account_id: u32, key: u32) -> bool {
        if self.of_account.contains_key(&account_id) {
            return false;
        }
        let serial = self.next_serial();
        let Some(room) = self.rooms.get_mut(&key) else {
            return false;
        };
        if room.is_full() {
            return false;
        }
        room.members.push(account_id);
        room.serial = serial;
        self.of_account.insert(account_id, key);
        true
    }

    /// The owner takes the member at `place` of its room's list out; it
    /// may not come back. The member's account.
    pub fn kick(&mut self, owner: u32, place: usize) -> Option<u32> {
        let key = *self.of_account.get(&owner)?;
        let room = self.rooms.get(&key)?;
        // Only a private room (with a PIN) turns anyone out: an open one is
        // everybody's (the owner's of 2026-10-19).
        if room.owner != owner || room.spec.pin.is_none() {
            return None;
        }
        let member = *room.members.get(place)?;
        if member == owner {
            return None;
        }
        self.leave(member);
        if let Some(room) = self.rooms.get_mut(&key) {
            room.kicked.insert(member);
        }
        Some(member)
    }

    /// The account's vote: card 0 to 2 of its room's vote now.
    pub fn vote(&mut self, account_id: u32, card: u8) -> bool {
        let Some(key) = self.of_account.get(&account_id) else {
            return false;
        };
        let Some(room) = self.rooms.get_mut(key) else {
            return false;
        };
        // Open until the match starts (the next cards come with it).
        if usize::from(card) >= room.cards.len() {
            return false;
        }
        room.votes.insert(account_id, card);
        true
    }

    /// Three cards from the pool, all different while the pool has as many.
    fn draw(pool: &[u8], below: &mut impl FnMut(usize) -> usize) -> Vec<u8> {
        let mut left: Vec<u8> = pool.to_vec();
        left.sort_unstable();
        left.dedup();
        let mut cards = Vec::new();
        while cards.len() < CARDS && !left.is_empty() {
            let pick = below(left.len()).min(left.len() - 1);
            cards.push(left.swap_remove(pick));
        }
        cards
    }

    /// The pass of the lobby's tick. Two members who count begin the vote,
    /// which runs [`VOTE`] and the room's own wait together; when it is
    /// over, with two who count, the room is due to start its match on the
    /// vote's winner then ([`Due::Start`]), which the lobby answers with
    /// [`Rooms::started`] or [`Rooms::not_started`]. Fewer than two members stop it; a member who
    /// stops counting for a while (changing its kit) does not.
    /// `counts`: whether a member can play the match now (in the lobby, a
    /// kit that can play, equipment within the max level).
    pub fn tick(
        &mut self,
        now: Instant,
        below: &mut impl FnMut(usize) -> usize,
        counts: impl Fn(u32) -> bool,
    ) -> Vec<Due> {
        let mut due = Vec::new();
        for room in self.rooms.values_mut() {
            let enough = room
                .members
                .iter()
                .filter(|&&member| counts(member))
                .count()
                >= room.needs();
            // The vote is always there: a match that started took its cards,
            // and the next three are drawn at once, for the match after it.
            if room.cards.is_empty() {
                room.cards = Self::draw(&room.pool, below);
                room.votes.clear();
            }
            let present = room.members.len() >= room.needs();
            match room.stage {
                Stage::Waiting if enough => {
                    let extra = if room.after_match {
                        AFTER_MATCH
                    } else {
                        Duration::ZERO
                    };
                    room.after_match = false;
                    room.stage = Stage::Vote {
                        until: now + VOTE + extra + Duration::from_secs(room.spec.wait.into()),
                    };
                }
                Stage::Vote { .. } if !present => {
                    room.stage = Stage::Waiting;
                }
                // The whole time, a full room too; the map is the vote's
                // winner at its end.
                Stage::Vote { until } if now >= until => match room.winner(below) {
                    Some(map_id) if enough => due.push(Due::Start {
                        key: room.key,
                        map_id,
                    }),
                    _ => room.stage = Stage::Waiting,
                },
                _ => {}
            }
        }
        due
    }

    /// The room's match was made: `seated` went to it.
    pub fn started(&mut self, key: u32, match_id: u32, map_id: u8, seated: &[u32], now: Instant) {
        if let Some(room) = self.rooms.get_mut(&key) {
            room.stage = Stage::InMatch {
                match_id,
                since: now,
            };
            room.last_map = map_id;
            room.in_match = seated.iter().copied().collect();
            room.cards.clear();
            room.votes.clear();
        }
    }

    /// The room's match could not be made: its vote and wait run again,
    /// whole ([`VOTE`] and the wait: the 30 s at the least hold), its votes
    /// kept.
    pub fn not_started(&mut self, key: u32, now: Instant) {
        if let Some(room) = self.rooms.get_mut(&key) {
            room.stage = match room.stage {
                Stage::Vote { .. } => Stage::Vote {
                    until: now + VOTE + Duration::from_secs(room.spec.wait.into()),
                },
                _ => Stage::Waiting,
            };
        }
    }

    /// The PIN the account gave is dropped (its session ended).
    pub fn forget_pin(&mut self, account_id: u32) {
        self.pins.remove(&account_id);
    }

    /// The account's draft of a pool is dropped (Play's room has the whole
    /// pool).
    pub fn forget_draft(&mut self, account_id: u32) {
        self.drafts.remove(&account_id);
    }

    /// A member went into the room's running match late.
    pub fn seated_late(&mut self, key: u32, account_id: u32) {
        if let Some(room) = self.rooms.get_mut(&key) {
            room.in_match.insert(account_id);
        }
    }

    /// A member that went into the room's running match late was let go
    /// before it entered: it waits in the room again.
    pub fn join_failed(&mut self, key: u32, account_id: u32) {
        if let Some(room) = self.rooms.get_mut(&key) {
            room.in_match.remove(&account_id);
        }
    }

    /// The room's match (the queue's id) is over: everyone is back, and the
    /// room waits for the next one (the next tick starts its vote).
    pub fn match_over(&mut self, match_id: u32) -> Option<u32> {
        let room = self.rooms.values_mut().find(
            |room| matches!(room.stage, Stage::InMatch { match_id: id, .. } if id == match_id),
        )?;
        // The cards and the votes for the next match stay: they were cast
        // while this one was played.
        room.stage = Stage::Waiting;
        room.in_match.clear();
        room.after_match = true;
        Some(room.key)
    }
}

/// The sides of a match of a room: `parties` (a squad, or one player, each
/// with what it counts for beside being there) on two sides of at most
/// `size`, a party too large for a side split into parts that fit. Of the
/// ways to put the parties whole, the one that fits the sizes, then whose
/// sides are the fewest players apart past one, then that count the most
/// alike (`player_value` a player, and the strengths: so with an odd count
/// the stronger players are the side of fewer). The largest party (the
/// first of those) is on side 0. When no way fits, the larger parties go
/// first, each to the side with fewer players, split where it must be.
pub fn sides(parties: &[(Vec<u32>, i64)], size: usize, player_value: i64) -> [Vec<u32>; 2] {
    let parties: Vec<(Vec<u32>, i64)> = parties
        .iter()
        .flat_map(|(members, strength)| {
            let each = strength / members.len().max(1) as i64;
            members
                .chunks(size.max(1))
                .map(move |chunk| (chunk.to_vec(), each * chunk.len() as i64))
        })
        .collect();
    let count = parties.len();
    // What a way is worth, the less the better. Bit `i` of `mask`: party
    // `i` is on side 1.
    let score = |mask: u32| {
        let mut players = [0_i64; 2];
        let mut strength = [0_i64; 2];
        for (index, (members, each)) in parties.iter().enumerate() {
            let side = (mask >> index & 1) as usize;
            players[side] += members.len() as i64;
            strength[side] += each;
        }
        let over: i64 = players
            .iter()
            .map(|&side| (side - size as i64).max(0))
            .sum();
        let apart = (players[0] - players[1]).abs();
        let worth = |side: usize| player_value * players[side] + strength[side];
        (over, (apart - 1).max(0), (worth(0) - worth(1)).abs())
    };
    let fixed = (0..count).max_by_key(|&index| (parties[index].0.len(), std::cmp::Reverse(index)));
    // A room has at most 20 members, so at most 20 parties.
    let best = (0..1_u32 << count.min(crate::MAX_PLAYERS_PER_MATCH))
        .filter(|mask| fixed.is_none_or(|fixed| mask >> fixed & 1 == 0))
        .min_by_key(|&mask| score(mask))
        .filter(|&mask| score(mask).0 == 0);
    let [mut a, mut b] = match best {
        Some(mask) => {
            let mut sides: [Vec<u32>; 2] = Default::default();
            for (index, (members, _)) in parties.into_iter().enumerate() {
                sides[(mask >> index & 1) as usize].extend(members);
            }
            sides
        }
        None => fill(parties, size),
    };
    // A room of one squad (or of players all on one side by size): half of
    // it plays on the other side, or there is no match.
    if b.is_empty() && a.len() >= 2 {
        b = a.split_off(a.len().div_ceil(2));
    } else if a.is_empty() && b.len() >= 2 {
        a = b.split_off(b.len().div_ceil(2));
    }
    [a, b]
}

/// [`sides`] when the parties do not fit whole: the larger parties first,
/// each where it fits on the side with fewer players (of two alike, the
/// one that counts for less); one too large for either side has its
/// members go one by one to the side with fewer players, while there is
/// room.
fn fill(mut parties: Vec<(Vec<u32>, i64)>, size: usize) -> [Vec<u32>; 2] {
    parties.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(b.1.cmp(&a.1)));
    let mut sides: [(Vec<u32>, i64); 2] = Default::default();
    for (members, strength) in parties {
        let fits = |side: &(Vec<u32>, i64)| side.0.len() + members.len() <= size;
        let order = if (sides[0].0.len(), sides[0].1) <= (sides[1].0.len(), sides[1].1) {
            [0, 1]
        } else {
            [1, 0]
        };
        let Some(side) = order.into_iter().find(|&side| fits(&sides[side])) else {
            let each = strength / members.len().max(1) as i64;
            for member in members {
                let side = usize::from(sides[1].0.len() < sides[0].0.len());
                if sides[side].0.len() < size {
                    sides[side].0.push(member);
                    sides[side].1 += each;
                }
            }
            continue;
        };
        sides[side].0.extend(members);
        sides[side].1 += strength;
    }
    let [(a, _), (b, _)] = sides;
    [a, b]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(size: u8, pin: Option<u16>) -> Spec {
        Spec {
            size,
            max_level: 0,
            wait: 30,
            pin,
            min_players: 2,
        }
    }

    /// Always the first choice.
    fn first(_: usize) -> usize {
        0
    }

    const POOL: [u8; 5] = [61, 63, 65, 117, 118];

    fn room_of(rooms: &mut Rooms, owner: u32, size: u8, pin: Option<u16>, now: Instant) -> u32 {
        rooms
            .create(owner, 0, spec(size, pin), &POOL, &POOL, 50, now, &mut first)
            .unwrap()
    }

    #[test]
    fn two_players_begin_the_vote_then_the_wait_then_the_match_on_its_map() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        assert!(SHORT_KEYS.contains(&key));
        // One player: it waits.
        assert!(rooms.tick(now, &mut first, |_| true).is_empty());
        assert_eq!(rooms.get(key).unwrap().stage, Stage::Waiting);
        rooms.join(2, 0, key, now).unwrap();
        assert!(rooms.tick(now, &mut first, |_| true).is_empty());
        let room = rooms.get(key).unwrap();
        // The vote and the room's own wait (30 s) together.
        let wait = Duration::from_secs(30);
        assert_eq!(
            room.stage,
            Stage::Vote {
                until: now + VOTE + wait
            }
        );
        assert_eq!(room.cards.len(), 3);
        let cards = room.cards.clone();
        assert!(rooms.vote(1, 0));
        // A vote may change.
        assert!(rooms.vote(1, 2));
        assert!(rooms.vote(2, 2));
        assert!(!rooms.vote(2, 3));
        // Past its 15 s the vote is still open (the owner's of 2026-10-19):
        // both change to card 0.
        let voted = now + VOTE;
        assert!(rooms.tick(voted, &mut first, |_| true).is_empty());
        assert!(matches!(rooms.get(key).unwrap().stage, Stage::Vote { .. }));
        assert!(rooms.vote(1, 0));
        assert!(rooms.vote(2, 0));
        assert!(
            rooms
                .tick(voted + wait - Duration::from_secs(1), &mut first, |_| true)
                .is_empty()
        );
        // The time is up: the match starts on the winner then.
        assert_eq!(
            rooms.tick(voted + wait, &mut first, |_| true),
            vec![Due::Start {
                key,
                map_id: cards[0]
            }]
        );
        rooms.started(key, 77, cards[0], &[1, 2], now);
        assert!(matches!(
            rooms.get(key).unwrap().stage,
            Stage::InMatch { match_id: 77, .. }
        ));
        // The next cards, and their vote while the match is played.
        rooms.tick(voted + wait, &mut first, |_| true);
        assert!(rooms.vote(2, 1));
        assert_eq!(rooms.match_over(77), Some(key));
        let room = rooms.get(key).unwrap();
        assert_eq!((room.stage, room.last_map), (Stage::Waiting, cards[0]));
        // The next vote begins, its votes kept, 10 s longer after a match.
        let back = voted + wait * 2;
        rooms.tick(back, &mut first, |_| true);
        let room = rooms.get(key).unwrap();
        assert_eq!(
            room.stage,
            Stage::Vote {
                until: back + VOTE + AFTER_MATCH + wait
            }
        );
        assert_eq!(room.votes.get(&2), Some(&1));
    }

    #[test]
    fn a_member_who_cannot_play_counts_for_no_start_but_changing_kit_stops_nothing() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        rooms.join(2, 0, key, now).unwrap();
        rooms.tick(now, &mut first, |member| member != 2);
        assert_eq!(rooms.get(key).unwrap().stage, Stage::Waiting);
        rooms.tick(now, &mut first, |_| true);
        assert!(matches!(rooms.get(key).unwrap().stage, Stage::Vote { .. }));
        // Its kit is being changed: the vote and the wait run on.
        rooms.tick(now + VOTE, &mut first, |member| member != 2);
        let Stage::Vote { until } = rooms.get(key).unwrap().stage else {
            panic!("no vote");
        };
        // Still not fit when the wait is over: no match; the room waits.
        assert!(
            rooms
                .tick(until, &mut first, |member| member != 2)
                .is_empty()
        );
        assert_eq!(rooms.get(key).unwrap().stage, Stage::Waiting);
    }

    #[test]
    fn a_member_unfit_for_a_while_in_the_vote_stops_nothing_unfit_at_its_end_no_match() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        rooms.join(2, 0, key, now).unwrap();
        rooms.tick(now, &mut first, |_| true);
        let Stage::Vote { until } = rooms.get(key).unwrap().stage else {
            panic!("no vote");
        };
        // Changing its kit in the middle of the vote: it runs on.
        let middle = now + VOTE;
        assert!(
            rooms
                .tick(middle, &mut first, |member| member != 2)
                .is_empty()
        );
        assert_eq!(rooms.get(key).unwrap().stage, Stage::Vote { until });
        // Fit again at its end: the match.
        let due = rooms.tick(until, &mut first, |_| true);
        assert!(matches!(due.as_slice(), [Due::Start { key: k, .. }] if *k == key));

        // Another room: unfit at the vote's end, no match; it waits.
        let other = room_of(&mut rooms, 3, 6, None, now);
        rooms.join(4, 0, other, now).unwrap();
        rooms.tick(now, &mut first, |_| true);
        let Stage::Vote { until } = rooms.get(other).unwrap().stage else {
            panic!("no vote");
        };
        assert!(
            rooms
                .tick(until, &mut first, |member| member != 4)
                .iter()
                .all(|due| !matches!(due, Due::Start { key: k, .. } if *k == other))
        );
        assert_eq!(rooms.get(other).unwrap().stage, Stage::Waiting);
    }

    #[test]
    fn a_room_that_could_not_start_votes_and_waits_again() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        for account in 2..=12 {
            rooms.join(account, 0, key, now).unwrap();
        }
        rooms.tick(now, &mut first, |_| true);
        let cards = rooms.get(key).unwrap().cards.clone();
        assert!(rooms.vote(2, 1));
        let wait = Duration::from_secs(rooms.get(key).unwrap().spec.wait.into());
        let due = now + VOTE + wait;
        let first_due = rooms.tick(due, &mut first, |_| true);
        assert_eq!(
            first_due,
            vec![Due::Start {
                key,
                map_id: cards[1]
            }]
        );
        // Not made: the vote and the wait again, whole (the 30 s at the
        // least hold), its votes kept.
        rooms.not_started(key, due);
        let room = rooms.get(key).unwrap();
        assert_eq!(
            room.stage,
            Stage::Vote {
                until: due + VOTE + wait
            }
        );
        assert_eq!(room.votes.get(&2), Some(&1));
        assert!(rooms.tick(due + VOTE, &mut first, |_| true).is_empty());
        // A vote changed meanwhile changes the map.
        assert!(rooms.vote(2, 2));
        assert_eq!(
            rooms.tick(due + VOTE + wait, &mut first, |_| true),
            vec![Due::Start {
                key,
                map_id: cards[2]
            }]
        );
        // The maker names it, whoever owns it later.
        rooms.leave(1);
        let room = rooms.get(key).unwrap();
        assert_eq!((room.owner, room.maker), (2, 1));
    }

    #[test]
    fn a_full_room_waits_its_whole_countdown_and_a_lone_player_stops_it() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        rooms.join(2, 0, key, now).unwrap();
        rooms.tick(now, &mut first, |_| true);
        rooms.leave(2);
        rooms.tick(now, &mut first, |_| true);
        assert_eq!(rooms.get(key).unwrap().stage, Stage::Waiting);
        for account in 2..=12 {
            rooms.join(account, 0, key, now).unwrap();
        }
        assert_eq!(rooms.join(13, 0, key, now), Err(Refused::Full));
        rooms.tick(now, &mut first, |_| true);
        rooms.tick(now + VOTE, &mut first, |_| true);
        // Full, but its players may still be changing their equipment.
        assert!(
            rooms
                .tick(now + VOTE + Duration::from_secs(1), &mut first, |_| true)
                .is_empty()
        );
        let wait = Duration::from_secs(rooms.get(key).unwrap().spec.wait.into());
        assert_eq!(rooms.tick(now + VOTE + wait, &mut first, |_| true).len(), 1);
    }

    #[test]
    fn a_room_of_2_v_2_waits_for_four() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = rooms
            .create(
                1,
                0,
                Spec {
                    min_players: 4,
                    ..spec(6, None)
                },
                &POOL,
                &POOL,
                50,
                now,
                &mut first,
            )
            .unwrap();
        for account in 2..=3 {
            rooms.join(account, 0, key, now).unwrap();
        }
        rooms.tick(now, &mut first, |_| true);
        assert_eq!(rooms.get(key).unwrap().stage, Stage::Waiting);
        rooms.join(4, 0, key, now).unwrap();
        rooms.tick(now, &mut first, |_| true);
        assert!(matches!(rooms.get(key).unwrap().stage, Stage::Vote { .. }));
        // One leaves: three are not enough.
        rooms.leave(4);
        rooms.tick(now, &mut first, |_| true);
        assert_eq!(rooms.get(key).unwrap().stage, Stage::Waiting);
    }

    #[test]
    fn a_room_of_one_squad_plays_it_on_both_sides() {
        let [a, b] = sides(&[(vec![1, 2, 3, 4], 40)], 6, 10);
        assert_eq!((a.len(), b.len()), (2, 2));
        let [a, b] = sides(&[(vec![1, 2], 20)], 8, 10);
        assert_eq!((a.len(), b.len()), (1, 1));
    }

    #[test]
    fn a_full_rooms_queue_takes_its_first_in_order_when_a_place_frees() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        for account in 2..=12 {
            rooms.join(account, 0, key, now).unwrap();
        }
        // In a room: no queue (the lobby only).
        let other = room_of(&mut rooms, 20, 6, None, now);
        assert_eq!(rooms.enqueue(20, 0, key, now), Err(Refused::Bad));
        assert_eq!(rooms.enqueue(30, 0, key, now), Ok(1));
        assert_eq!(rooms.enqueue(31, 0, key, now), Ok(2));
        assert_eq!(rooms.enqueue(32, 0, key, now), Ok(3));
        // Again: the same place.
        assert_eq!(rooms.enqueue(31, 0, key, now), Ok(2));
        assert_eq!(rooms.queue_place(32), Some((key, 3)));
        // Full: nobody comes yet.
        assert_eq!(rooms.next_in_queue(key), None);
        // One queue at a time: 31 waits for the other room now.
        rooms.leave(20);
        assert!(rooms.get(other).is_none());
        let other = room_of(&mut rooms, 21, 6, None, now);
        assert_eq!(rooms.enqueue(31, 0, other, now), Ok(1));
        assert_eq!(rooms.queue_place(32), Some((key, 2)));
        assert_eq!(rooms.queued_total(), 3);
        // A place frees: the first goes in.
        rooms.leave(5);
        assert!(rooms.with_queue_and_place().contains(&key));
        assert_eq!(rooms.next_in_queue(key), Some(30));
        assert!(rooms.admit(30, key));
        assert_eq!(rooms.next_in_queue(key), None);
        assert_eq!(rooms.queue_place(32), Some((key, 1)));
        // Joining a room takes one out of its queue.
        rooms.join(31, 0, other, now).unwrap();
        assert_eq!(rooms.queue_place(31), None);
        // A room disbanded lets its queue go too.
        let gone = rooms.disband(key);
        assert!(gone.contains(&32));
        assert_eq!(rooms.queue_place(32), None);
        assert_eq!(rooms.queued_total(), 0);
    }

    #[test]
    fn a_queue_asks_for_the_pin_and_the_max_level() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, Some(42), now);
        assert_eq!(rooms.enqueue(2, 0, key, now), Err(Refused::WrongPin));
        rooms.give_pin(2, 42, now);
        assert_eq!(rooms.enqueue(2, 0, key, now), Ok(1));
        assert_eq!(rooms.dequeue(2), Some(key));
        assert_eq!(rooms.dequeue(2), None);
    }

    /// An open room (no PIN) is everybody's: its owner turns nobody out.
    #[test]
    fn an_open_room_has_no_kick() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 8, None, now);
        rooms.join(2, 0, key, now).unwrap();
        assert_eq!(rooms.kick(1, 1), None);
        assert_eq!(rooms.get(key).unwrap().members, [1, 2]);
    }

    #[test]
    fn a_pin_a_kick_and_an_owner_who_leaves() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 8, Some(42), now);
        assert_eq!(rooms.join(2, 0, key, now), Err(Refused::WrongPin));
        rooms.give_pin(2, 41, now);
        assert_eq!(rooms.join(2, 0, key, now), Err(Refused::WrongPin));
        rooms.give_pin(2, 42, now);
        assert_eq!(rooms.join(2, 0, key, now), Ok(()));
        // A PIN is good for one JOIN within its life.
        rooms.give_pin(3, 42, now);
        assert_eq!(
            rooms.join(3, 0, key, now + PIN_LIFE + Duration::from_secs(1)),
            Err(Refused::WrongPin)
        );
        rooms.give_pin(3, 42, now);
        rooms.join(3, 0, key, now).unwrap();
        // Only the owner kicks, and never itself.
        assert_eq!(rooms.kick(2, 2), None);
        assert_eq!(rooms.kick(1, 0), None);
        assert_eq!(rooms.kick(1, 2), Some(3));
        rooms.give_pin(3, 42, now);
        assert_eq!(rooms.join(3, 0, key, now), Err(Refused::Kicked));
        // The owner leaves: the first to have come owns it.
        rooms.leave(1);
        assert_eq!(rooms.get(key).unwrap().owner, 2);
        rooms.leave(2);
        assert!(rooms.get(key).is_none());
        assert!(rooms.is_empty());
    }

    #[test]
    fn a_draft_names_the_pool_and_lasts_a_while() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let entries: Vec<u8> = (0..25).collect();
        assert_eq!(rooms.draft(1, 0, 0b101, now), 2);
        assert_eq!(rooms.draft(1, 1, 0b1, now), 3);
        let key = rooms
            .create(1, 0, spec(8, None), &entries, &[99], 50, now, &mut first)
            .unwrap();
        assert_eq!(rooms.get(key).unwrap().pool, vec![0, 2, 20]);
        // Too old: the default.
        rooms.draft(2, 0, 0b1, now);
        let key = rooms
            .create(
                2,
                0,
                spec(8, None),
                &entries,
                &[99],
                50,
                now + DRAFT_LIFE + Duration::from_secs(1),
                &mut first,
            )
            .unwrap();
        assert_eq!(rooms.get(key).unwrap().pool, vec![99]);
        // Nothing at all is no room.
        rooms.draft(3, 0, 0, now);
        assert_eq!(
            rooms.create(3, 0, spec(8, None), &entries, &[99], 50, now, &mut first),
            Err(Refused::Bad)
        );
    }

    #[test]
    fn the_rooms_are_capped_but_the_owner_of_one_alone_may_make_another() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let mut keys = 0..;
        let mut next = |_: usize| keys.next().unwrap();
        rooms
            .create(1, 0, spec(8, None), &POOL, &POOL, 1, now, &mut next)
            .unwrap();
        assert_eq!(
            rooms.create(2, 0, spec(8, None), &POOL, &POOL, 1, now, &mut next),
            Err(Refused::TooMany)
        );
        rooms
            .create(1, 0, spec(6, None), &POOL, &POOL, 1, now, &mut next)
            .unwrap();
        assert_eq!(rooms.len(), 1);
    }

    #[test]
    fn a_max_level_keeps_higher_gear_out_and_0_lets_anyone_in() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let capped = Spec {
            max_level: 5,
            ..spec(8, None)
        };
        // Not under its maker's own equipment level.
        assert_eq!(
            rooms.create(1, 6, capped, &POOL, &POOL, 50, now, &mut first),
            Err(Refused::GearTooHigh)
        );
        let key = rooms
            .create(1, 5, capped, &POOL, &POOL, 50, now, &mut first)
            .unwrap();
        assert_eq!(rooms.join(2, 6, key, now), Err(Refused::GearTooHigh));
        assert!(rooms.open_rooms(2, 6).next().is_none());
        assert_eq!(rooms.join(2, 5, key, now), Ok(()));
        let any = room_of(&mut rooms, 3, 8, None, now);
        assert_eq!(rooms.join(4, 15, any, now), Ok(()));
    }

    #[test]
    fn a_tie_is_drawn_and_no_vote_is_a_draw_of_all() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        rooms.join(2, 0, key, now).unwrap();
        rooms.tick(now, &mut first, |_| true);
        let cards = rooms.get(key).unwrap().cards.clone();
        let mut last = |bound: usize| bound - 1;
        assert_eq!(rooms.get(key).unwrap().winner(&mut last), Some(cards[2]));
        rooms.vote(1, 0);
        rooms.vote(2, 1);
        assert_eq!(rooms.get(key).unwrap().winner(&mut last), Some(cards[1]));
        assert_eq!(rooms.get(key).unwrap().winner(&mut first), Some(cards[0]));
    }

    #[test]
    fn the_sides_keep_squads_together_and_even() {
        let parties = vec![
            (vec![1, 2, 3], 30),
            (vec![4], 20),
            (vec![5], 5),
            (vec![6], 10),
            (vec![7], 10),
        ];
        let [a, b] = sides(&parties, 6, 10);
        assert!(a.len().abs_diff(b.len()) <= 1);
        let together = |side: &Vec<u32>| [1, 2, 3].iter().all(|m| side.contains(m));
        assert!(together(&a) || together(&b));
        assert_eq!(a.len() + b.len(), 7);
        // A squad larger than a side is split.
        let [a, b] = sides(&[((1..=8).collect(), 80)], 6, 10);
        assert_eq!((a.len(), b.len()), (6, 2));
        // Two squads of 5 and one of 2 in a 6 v 6: the 2 are split, not left out.
        let [a, b] = sides(
            &[
                ((1..=5).collect(), 50),
                ((6..=10).collect(), 50),
                (vec![11, 12], 20),
            ],
            6,
            10,
        );
        assert_eq!((a.len(), b.len()), (6, 6));
    }

    #[test]
    fn the_sides_count_alike_and_an_odd_count_puts_the_stronger_on_the_side_of_fewer() {
        // Five alone, two of them worth six more each, a player 12: the
        // two strong ones are the two.
        let parties: Vec<(Vec<u32>, i64)> = [0, 0, 6, 0, 6]
            .into_iter()
            .enumerate()
            .map(|(index, strength)| (vec![index as u32 + 1], strength))
            .collect();
        let [a, b] = sides(&parties, 8, 12);
        let (mut fewer, more) = if a.len() < b.len() { (a, b) } else { (b, a) };
        fewer.sort_unstable();
        assert_eq!((fewer, more.len()), (vec![3, 5], 3));
        // Four alone: two a side, the strong ones apart.
        let [a, b] = sides(&parties[1..], 8, 12);
        assert_eq!((a.len(), b.len()), (2, 2));
        assert!(a.contains(&3) != a.contains(&5));
    }
}
