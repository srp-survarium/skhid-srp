//! Rooms (the players call them servers): a player-made place in the lobby
//! where players wait together, vote on the next map and play match after
//! match. The owner's rules of 2026-10-16:
//!
//! - Public rooms are made by the lobby on demand: 8 v 8, equipment cap
//!   10 or 15, the standard pool. Players create private rooms only, with
//!   a PIN and a chosen pool. A private room has no max level ([`Door`]).
//!   A room is named after its maker and cannot be changed once made.
//!   Only a private room's owner may take a member out.
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
//!   wait begin; except who came by Play and played it: it leaves the room
//!   (the owner's of 2026-10-20, [`Rooms::match_over`]).
//! - The lobby routes Play within its equipment bracket, queues for full
//!   rooms, and opens another room when six ready Play users are waiting.
//!   [`QueueChoice`] keeps those waiters distinct from explicit room queues.
//! - A private room (a PIN; the owner's of 2026-10-26) has no team size
//!   and no players to start: it takes the most a match has (10 v 10), and
//!   nothing counts down in it. Its owner starts its match when it wants,
//!   with whoever is in, 1 v 0 too ([`Rooms::start`]): on the vote's
//!   winner then (its cards are there, and voted for, as in any room).
//!   After the match it waits for its owner again. Its owner arranges its
//!   two teams, any split up to 10 a side ([`Rooms::move_member`]), and
//!   its match plays them as they are ([`Room::arranged`]). It has no max
//!   level either (the owner's of 2026-10-26: "private room shouldn't have
//!   max level too"): whoever has its PIN comes in with any equipment, and
//!   nothing takes a member out of it for its equipment ([`Door::Pin`]).
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
/// The fewest players an open room's vote (and so its match) starts with.
pub const START_PLAYERS: usize = 2;
/// A private room's team size: the most a match has, 10 v 10 (the owner's
/// of 2026-10-26: no size to choose).
pub const PRIVATE_SIZE: u8 = (crate::MAX_PLAYERS_PER_MATCH / 2) as u8;
/// The cards of a vote.
pub const CARDS: usize = 3;
/// The pool entries one piece of a draft carries.
pub const DRAFT_CHUNK: usize = 20;
/// The keys new rooms are given first: four digits, easy to say.
const SHORT_KEYS: std::ops::RangeInclusive<u32> = 1000..=9999;
/// The largest key.
const KEY_MAX: u32 = 99_999;
/// The lines a room's chat keeps: what a member's "Room Chat" tab is
/// given when it comes, and again after its match (`social.md` §9.8).
pub const CHAT_KEPT: usize = 20;

/// What a room is made with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub size: u8,
    pub wait: u8,
    /// Who it lets in: an open room's max level, a private room's PIN.
    pub door: Door,
    /// The players an open room needs before its vote begins: 2 (1 v 1), 4
    /// or 6. A private room has none: its owner starts it ([`Room::needs`]).
    pub min_players: u8,
}

/// Who a room lets in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Door {
    /// An open room: anyone whose equipment level is within `max_level`
    /// (0: any).
    Open { max_level: u8 },
    /// A private room: whoever gives its PIN, with any equipment (the
    /// owner's of 2026-10-26: no max level).
    Pin(u16),
}

impl Door {
    /// Whether a player of equipment level `gear` comes in.
    pub fn lets_in(self, gear: u8) -> bool {
        match self {
            Door::Open { max_level } => max_level == 0 || gear <= max_level,
            Door::Pin(_) => true,
        }
    }

    /// The highest equipment level let in as the movie and a join's offer
    /// are told it: an open room's (0: any); 0 for a private room, which
    /// has none.
    pub fn max_level(self) -> u8 {
        match self {
            Door::Open { max_level } => max_level,
            Door::Pin(_) => 0,
        }
    }

    /// Whether the PIN given last (`given`) opens it: an open room's always.
    fn opens(self, given: Option<u16>) -> bool {
        match self {
            Door::Open { .. } => true,
            Door::Pin(pin) => given == Some(pin),
        }
    }
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
    /// Its chat's last lines, the oldest first, [`CHAT_KEPT`] at most:
    /// gone with the room.
    pub chat: VecDeque<ChatLine>,
    /// The members who came by Play (into it, or Play made it for them),
    /// not by a choice of this room (a join, a create, its queue): they
    /// leave it when a match of it they played is over, for their next
    /// Play to put them into the room nearest their level (the owner's of
    /// 2026-10-20). Choosing the room later makes one a chosen member.
    pub by_play: HashSet<u32>,
    /// A private room's teams as its owner arranged them: each member's
    /// side, 0 or 1 (the owner's of 2026-10-26: "the owner should be able
    /// to arrange both teams (even 1 vs 10)"). A member comes on the side
    /// with fewer; its owner moves it ([`Rooms::move_member`]); its match
    /// plays them as they are. Empty in an open room (its sides are drawn
    /// at the start, [`sides`]).
    pub sides: HashMap<u32, u8>,
    /// The members who came while its match is played: a private room's
    /// newcomers, whose late join goes to the side with fewer (their team in
    /// the arrangement is not the owner's choice yet). Cleared with the match.
    pub came_in_match: HashSet<u32>,
}

/// A line of a room's chat: a member's, or the room's own (a member came
/// or went), whose sender is account 0 with no name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatLine {
    pub from: u32,
    pub name: String,
    pub tag: String,
    pub body: String,
}

impl Room {
    /// Each side's players times two.
    pub fn capacity(&self) -> usize {
        2 * usize::from(self.spec.size)
    }

    /// The players its match needs: an open room's before its vote begins
    /// (two at least); a private room's one, its owner starting it with
    /// whoever is in ([`Rooms::start`]).
    pub fn needs(&self) -> usize {
        if self.is_open() {
            usize::from(self.spec.min_players).max(START_PLAYERS)
        } else {
            1
        }
    }

    /// The members of a private room's side `side` in its arrangement.
    pub fn side_count(&self, side: u8) -> usize {
        self.sides.values().filter(|&&each| each == side).count()
    }

    /// A private room's arrangement: the members of each side, in the
    /// room's order (what its owner's Start plays).
    pub fn arranged(&self) -> [Vec<u32>; 2] {
        let on = |side: u8| {
            self.members
                .iter()
                .copied()
                .filter(|member| self.sides.get(member).copied().unwrap_or(0) == side)
                .collect()
        };
        [on(0), on(1)]
    }

    /// A member comes: last in the list; in a private room on the side with
    /// fewer (side 0 of two alike).
    fn add_member(&mut self, account_id: u32) {
        if self.members.is_empty() {
            self.owner = account_id;
        }
        self.members.push(account_id);
        if !self.is_open() {
            let side = u8::from(self.side_count(1) < self.side_count(0));
            self.sides.insert(account_id, side);
        }
        if matches!(self.stage, Stage::InMatch { .. }) {
            self.came_in_match.insert(account_id);
        }
    }

    /// The team a private room's member goes into its match on when it
    /// goes in late (it missed the start: its game away or started anew, a
    /// late join that failed): the one its owner put it on (the owner's of
    /// 2026-10-26: "Start plays exactly that"). None in an open room, and
    /// for one who came while the match is played (it goes to the side with
    /// fewer, as in any room).
    pub fn arranged_side(&self, account_id: u32) -> Option<u8> {
        if self.is_open() || self.came_in_match.contains(&account_id) {
            return None;
        }
        self.sides.get(&account_id).copied()
    }

    pub fn is_full(&self) -> bool {
        self.members.len() >= self.capacity()
    }

    pub fn is_open(&self) -> bool {
        matches!(self.spec.door, Door::Open { .. })
    }

    /// Whether a player of equipment level `gear` is let in, and may play
    /// its matches (a private room: any).
    pub fn lets_in(&self, gear: u8) -> bool {
        self.spec.door.lets_in(gear)
    }

    /// Its max level as the movie is told it ([`Door::max_level`]).
    pub fn max_level(&self) -> u8 {
        self.spec.door.max_level()
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

/// Why an owner's Start did nothing ([`Rooms::start`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartRefused {
    /// The asker is in no room (or its room has no cards).
    NoRoom,
    /// Only its owner starts a room's match.
    NotOwner,
    /// An open room starts by its countdown, not by a Start.
    Open,
    /// Its match is being played.
    Playing,
}

/// Why an owner's move of a member did nothing ([`Rooms::move_member`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveRefused {
    NoRoom,
    NotOwner,
    /// An open room's sides are drawn at its start.
    Open,
    /// Its match is being played.
    Playing,
    /// No member at that place.
    NoMember,
    /// The other side has its [`PRIVATE_SIZE`].
    Full,
}

/// A pool being sent in pieces before its create.
#[derive(Clone, Debug)]
struct Draft {
    chunks: [u32; 8],
    seen: u8,
    at: Instant,
}

/// Whether a queued player chose this room or asked Play to find a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueChoice {
    Chosen,
    Play,
}

#[derive(Clone, Copy, Debug)]
struct Queued {
    key: u32,
    choice: QueueChoice,
    since: Instant,
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
    queued: HashMap<u32, Queued>,
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

    /// A line in room `key`'s chat, kept with its last [`CHAT_KEPT`]; none
    /// for a room that is gone.
    pub fn say(&mut self, key: u32, line: ChatLine) {
        if let Some(room) = self.rooms.get_mut(&key) {
            if room.chat.len() >= CHAT_KEPT {
                room.chat.pop_front();
            }
            room.chat.push_back(line);
        }
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
    /// owner's equipment level `gear` must be within an open room's max
    /// level. A private one (a PIN) is [`PRIVATE_SIZE`] a side, whatever `spec`
    /// says.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &mut self,
        owner: u32,
        gear: u8,
        mut spec: Spec,
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
        if let Door::Pin(_) = spec.door {
            spec.size = PRIVATE_SIZE;
        }
        if !spec.door.lets_in(gear) {
            return Err(Refused::GearTooHigh);
        }
        if self.rooms.len() >= most_rooms
            && self.of(owner).is_none_or(|room| {
                room.members.len() > 1 || (room.is_open() && self.queue_len(room.key) > 0)
            })
        {
            return Err(Refused::TooMany);
        }
        self.leave(owner);
        let key = self.new_key(below).ok_or(Refused::TooMany)?;
        let serial = self.next_serial();
        // The vote is always there: its cards from the start.
        let cards = Self::draw(&pool, below);
        let mut room = Room {
            key,
            owner,
            maker: owner,
            spec,
            pool,
            members: Vec::new(),
            stage: Stage::Waiting,
            cards,
            votes: HashMap::new(),
            last_map: 0,
            after_match: false,
            in_match: HashSet::new(),
            serial,
            kicked: HashSet::new(),
            chat: VecDeque::new(),
            by_play: HashSet::new(),
            sides: HashMap::new(),
            came_in_match: HashSet::new(),
        };
        room.add_member(owner);
        self.rooms.insert(key, room);
        self.of_account.insert(owner, key);
        self.dequeue(owner);
        Ok(key)
    }

    /// The account joins room `key`, with the PIN it gave last (within
    /// [`PIN_LIFE`]) and its equipment level `gear`; it leaves the room it
    /// was in. It chose the room: it stays after a match, as its maker does
    /// (Play's: [`Rooms::came_by_play`]). Joining the room it is in already
    /// changes only that: one that came by Play has chosen it now.
    pub fn join(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
    ) -> Result<(), Refused> {
        let pin = self.take_pin(account_id, now);
        let room = self.rooms.get_mut(&key).ok_or(Refused::Unknown)?;
        if room.members.contains(&account_id) {
            room.by_play.remove(&account_id);
            return Ok(());
        }
        if room.kicked.contains(&account_id) {
            return Err(Refused::Kicked);
        }
        if !room.spec.door.opens(pin) {
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
        room.add_member(account_id);
        room.serial = serial;
        self.of_account.insert(account_id, key);
        // In a room: no queue any more.
        self.dequeue(account_id);
        Ok(())
    }

    /// The account came into its room by Play (into an open room, or one
    /// Play made for it): it leaves the room after the first match of it
    /// that it plays ([`Rooms::match_over`]).
    pub fn came_by_play(&mut self, account_id: u32) {
        if let Some(room) = self
            .of_account
            .get(&account_id)
            .and_then(|key| self.rooms.get_mut(key))
        {
            room.by_play.insert(account_id);
        }
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

    /// The account leaves its room. An empty public room keeps its queue; an
    /// otherwise empty room is gone. An owner hands it to the next member. The
    /// room it left, if any.
    pub fn leave(&mut self, account_id: u32) -> Option<u32> {
        let key = self.of_account.remove(&account_id)?;
        let serial = self.next_serial();
        let room = self.rooms.get_mut(&key)?;
        room.members.retain(|member| *member != account_id);
        room.votes.remove(&account_id);
        room.in_match.remove(&account_id);
        room.by_play.remove(&account_id);
        room.sides.remove(&account_id);
        room.came_in_match.remove(&account_id);
        room.serial = serial;
        if room.members.is_empty() && !(room.is_open() && self.queues.contains_key(&key)) {
            self.rooms.remove(&key);
            // Its queue goes with it: nobody waits for a room that is gone.
            for waiting in self.queues.remove(&key).unwrap_or_default() {
                self.queued.remove(&waiting);
            }
        } else if room.owner == account_id
            && let Some(&owner) = room.members.first()
        {
            room.owner = owner;
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
    /// lobby only), the room lets it in (its PIN given just before, an
    /// open room's max level, not turned out). One queue at a time: a new
    /// one takes it out of the last. Its place, 1 the first.
    pub fn enqueue(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
    ) -> Result<usize, Refused> {
        self.enqueue_as(account_id, gear, key, now, QueueChoice::Chosen)
    }

    /// Play waits for space. Unlike an explicitly chosen queue, these
    /// players may move together to another room when demand grows.
    pub fn enqueue_play(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
    ) -> Result<usize, Refused> {
        self.enqueue_as(account_id, gear, key, now, QueueChoice::Play)
    }

    fn enqueue_as(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
        choice: QueueChoice,
    ) -> Result<usize, Refused> {
        let pin = self.take_pin(account_id, now);
        if self.of_account.contains_key(&account_id) {
            return Err(Refused::Bad);
        }
        let room = self.rooms.get(&key).ok_or(Refused::Unknown)?;
        if choice == QueueChoice::Play && !room.is_open() {
            return Err(Refused::Bad);
        }
        if room.kicked.contains(&account_id) {
            return Err(Refused::Kicked);
        }
        if !room.spec.door.opens(pin) {
            return Err(Refused::WrongPin);
        }
        if !room.lets_in(gear) {
            return Err(Refused::GearTooHigh);
        }
        if self
            .queued
            .get(&account_id)
            .is_none_or(|queued| queued.key != key)
        {
            let since = self
                .queued
                .get(&account_id)
                .map_or(now, |queued| queued.since);
            self.dequeue(account_id);
            self.queues.entry(key).or_default().push_back(account_id);
            self.queued
                .insert(account_id, Queued { key, choice, since });
        } else if let Some(queued) = self.queued.get_mut(&account_id) {
            queued.choice = choice;
        }
        Ok(self.queue_place(account_id).map_or(0, |(_, place)| place))
    }

    /// The account leaves the queue it waits in, if any: its room's key.
    pub fn dequeue(&mut self, account_id: u32) -> Option<u32> {
        let key = self.queued.remove(&account_id)?.key;
        if let Some(queue) = self.queues.get_mut(&key) {
            queue.retain(|waiting| *waiting != account_id);
            if queue.is_empty() {
                self.queues.remove(&key);
                if self
                    .rooms
                    .get(&key)
                    .is_some_and(|room| room.members.is_empty())
                {
                    self.rooms.remove(&key);
                }
            }
        }
        Some(key)
    }

    /// The room an account waits for and its place there, 1 the first.
    pub fn queue_place(&self, account_id: u32) -> Option<(u32, usize)> {
        let key = self.queued.get(&account_id)?.key;
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

    /// Play's waiters across all rooms, oldest first. Repeated Play does
    /// not change their position; choosing a room's queue removes them.
    pub fn play_queue(&self) -> Vec<(u32, u32)> {
        let mut waiting: Vec<_> = self
            .queued
            .iter()
            .filter(|(_, queued)| queued.choice == QueueChoice::Play)
            .map(|(&account_id, queued)| (queued.since, account_id, queued.key))
            .collect();
        waiting.sort_unstable();
        waiting
            .into_iter()
            .map(|(_, account_id, key)| (account_id, key))
            .collect()
    }

    /// The first of the room's queue, while it has a place free: out of the
    /// queue, for the lobby to put in (`admit`) if it still may come.
    pub fn next_in_queue(&mut self, key: u32) -> Option<u32> {
        self.next_ready_in_queue(key, &|_| true)
            .map(|(account_id, _)| account_id)
    }

    /// [`next_in_queue`](Self::next_in_queue), the first that `ready` takes:
    /// those before it stay in their places.
    pub fn next_ready_in_queue(
        &mut self,
        key: u32,
        ready: &dyn Fn(u32) -> bool,
    ) -> Option<(u32, QueueChoice)> {
        let room = self.rooms.get(&key)?;
        if room.is_full() {
            return None;
        }
        let queue = self.queues.get_mut(&key)?;
        let at = queue.iter().position(|&account_id| ready(account_id))?;
        let first = queue.remove(at)?;
        let choice = self.queued.remove(&first)?.choice;
        if self.queues.get(&key).is_some_and(VecDeque::is_empty) {
            self.queues.remove(&key);
        }
        Some((first, choice))
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
        room.add_member(account_id);
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
        if room.owner != owner || room.is_open() {
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

    /// The owner of a private room starts its match now, on the vote's
    /// winner then (the owner's of 2026-10-26: no countdown, its owner's
    /// Start with whoever is in). The lobby makes the match as for a
    /// [`Due::Start`] of [`Rooms::tick`], with as many members as can play
    /// (one at the least, [`Room::needs`]).
    pub fn start(
        &self,
        account_id: u32,
        below: &mut impl FnMut(usize) -> usize,
    ) -> Result<Due, StartRefused> {
        let room = self.of(account_id).ok_or(StartRefused::NoRoom)?;
        if room.owner != account_id {
            return Err(StartRefused::NotOwner);
        }
        if room.is_open() {
            return Err(StartRefused::Open);
        }
        if room.stage != Stage::Waiting {
            return Err(StartRefused::Playing);
        }
        let map_id = room.winner(below).ok_or(StartRefused::NoRoom)?;
        Ok(Due::Start {
            key: room.key,
            map_id,
        })
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
    /// stops counting for a while (changing its kit) does not. A private
    /// room counts nothing down: its owner starts it ([`Rooms::start`]).
    /// `counts`: whether a member can play the match now (in the lobby, a
    /// kit that can play, equipment within the max level).
    pub fn tick(
        &mut self,
        now: Instant,
        below: &mut impl FnMut(usize) -> usize,
        counts: impl Fn(u32) -> bool,
    ) -> Vec<Due> {
        // Admission can discard the final ineligible waiter of an empty
        // room. An empty public room with a queue survives for admission.
        self.rooms
            .retain(|key, room| !room.members.is_empty() || self.queues.contains_key(key));
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
                Stage::Waiting if enough && room.is_open() => {
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

    /// A member went into the room's running match late, on side `team`:
    /// in a private room its side in the arrangement from now on, as it
    /// plays, while that team has a place (never more than
    /// [`PRIVATE_SIZE`]); one on its own team keeps it.
    pub fn seated_late(&mut self, key: u32, account_id: u32, team: u8) {
        if let Some(room) = self.rooms.get_mut(&key) {
            room.in_match.insert(account_id);
            let team = team.min(1);
            if room
                .sides
                .get(&account_id)
                .is_some_and(|&side| side != team)
                && room.side_count(team) < usize::from(PRIVATE_SIZE)
            {
                room.sides.insert(account_id, team);
            }
        }
    }

    /// The owner of a private room moves the member at `place` of its
    /// list (itself too) to the other side (the owner's of 2026-10-26: "a
    /// button to move the player into any team"), while the room waits;
    /// never onto a side of [`PRIVATE_SIZE`] already. Its side now.
    pub fn move_member(&mut self, owner: u32, place: usize) -> Result<u8, MoveRefused> {
        let key = *self.of_account.get(&owner).ok_or(MoveRefused::NoRoom)?;
        let room = self.rooms.get_mut(&key).ok_or(MoveRefused::NoRoom)?;
        if room.owner != owner {
            return Err(MoveRefused::NotOwner);
        }
        if room.is_open() {
            return Err(MoveRefused::Open);
        }
        if room.stage != Stage::Waiting {
            return Err(MoveRefused::Playing);
        }
        let member = *room.members.get(place).ok_or(MoveRefused::NoMember)?;
        let to = 1 - room.sides.get(&member).copied().unwrap_or(0).min(1);
        if room.side_count(to) >= usize::from(PRIVATE_SIZE) {
            return Err(MoveRefused::Full);
        }
        room.sides.insert(member, to);
        Ok(to)
    }

    /// A member that went into the room's running match late was let go
    /// before it entered: it waits in the room again.
    pub fn join_failed(&mut self, key: u32, account_id: u32) {
        if let Some(room) = self.rooms.get_mut(&key) {
            room.in_match.remove(&account_id);
        }
    }

    /// The room's match (the queue's id) is over: everyone is back, and the
    /// room waits for the next one (the next tick starts its vote). Who came
    /// by Play and played it leaves the room, as by Leave (the owner's of
    /// 2026-10-20: its next Play may put it into a room of its level); one
    /// that came by Play and did not play it (it came too late to go in)
    /// stays for the next match. An empty public room keeps its queue. The room's key
    /// and who left it.
    pub fn match_over(&mut self, match_id: u32) -> Option<(u32, Vec<u32>)> {
        let room = self.rooms.values_mut().find(
            |room| matches!(room.stage, Stage::InMatch { match_id: id, .. } if id == match_id),
        )?;
        let key = room.key;
        let leaving: Vec<u32> = room
            .members
            .iter()
            .copied()
            .filter(|member| room.by_play.contains(member) && room.in_match.contains(member))
            .collect();
        // The cards and the votes for the next match stay: they were cast
        // while this one was played.
        room.stage = Stage::Waiting;
        room.in_match.clear();
        room.came_in_match.clear();
        room.after_match = true;
        for &member in &leaving {
            self.leave(member);
        }
        Some((key, leaving))
    }

    /// The room's match was called off before its countdown (fewer than
    /// the room's minimum could play it): as a match that could not be
    /// made, the room has its vote and its wait again, whole, with the
    /// votes cast meanwhile; no results to come back from.
    pub fn match_not_made(&mut self, match_id: u32) -> Option<u32> {
        let room = self.rooms.values_mut().find(
            |room| matches!(room.stage, Stage::InMatch { match_id: id, .. } if id == match_id),
        )?;
        room.stage = Stage::Waiting;
        room.in_match.clear();
        room.came_in_match.clear();
        room.after_match = false;
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
            wait: 30,
            door: pin.map_or(Door::Open { max_level: 0 }, Door::Pin),
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
        assert_eq!(rooms.match_over(77), Some((key, vec![])));
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

    /// The owner's of 2026-10-26: a private room is 10 v 10, counts nothing
    /// down, and its owner starts its match when it wants, on the vote's
    /// winner then; nobody else, never an open room, never while its match
    /// is played. After the match it waits for its owner again.
    #[test]
    fn a_private_room_starts_by_its_owner_only() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let open = room_of(&mut rooms, 1, 6, None, now);
        let private = room_of(&mut rooms, 2, 6, Some(1234), now);
        assert_eq!(rooms.get(private).unwrap().spec.size, PRIVATE_SIZE);
        assert_eq!(rooms.get(private).unwrap().capacity(), 20);
        assert_eq!(rooms.get(open).unwrap().spec.size, 6);
        assert_eq!(rooms.get(private).unwrap().needs(), 1);
        rooms.give_pin(3, 1234, now);
        rooms.join(3, 0, private, now).unwrap();
        rooms.join(4, 0, open, now).unwrap();
        // Two in each: the open room votes, the private one waits.
        assert!(rooms.tick(now, &mut first, |_| true).is_empty());
        assert!(matches!(rooms.get(open).unwrap().stage, Stage::Vote { .. }));
        assert_eq!(rooms.get(private).unwrap().stage, Stage::Waiting);
        let later = now + Duration::from_secs(600);
        let due = rooms.tick(later, &mut first, |_| true);
        assert!(matches!(due[..], [Due::Start { key, .. }] if key == open));
        assert_eq!(rooms.get(private).unwrap().stage, Stage::Waiting);

        assert_eq!(rooms.start(3, &mut first), Err(StartRefused::NotOwner));
        assert_eq!(rooms.start(1, &mut first), Err(StartRefused::Open));
        assert_eq!(rooms.start(9, &mut first), Err(StartRefused::NoRoom));
        // The member's vote decides the map.
        assert!(rooms.vote(3, 2));
        let cards = rooms.get(private).unwrap().cards.clone();
        assert_eq!(
            rooms.start(2, &mut first),
            Ok(Due::Start {
                key: private,
                map_id: cards[2]
            })
        );
        rooms.started(private, 77, cards[2], &[2, 3], later);
        assert_eq!(rooms.start(2, &mut first), Err(StartRefused::Playing));
        rooms.match_over(77);
        rooms.tick(later, &mut first, |_| true);
        rooms.tick(later + Duration::from_secs(600), &mut first, |_| true);
        assert_eq!(rooms.get(private).unwrap().stage, Stage::Waiting);
        assert!(rooms.start(2, &mut first).is_ok());
        // Its owner gone, the next member owns it and starts it.
        rooms.leave(2);
        assert!(rooms.start(3, &mut first).is_ok());
    }

    /// The owner's of 2026-10-26: a private room's owner arranges its teams,
    /// any split up to 10 a side; a newcomer comes on the side with fewer; a
    /// leaver goes out of it; the arrangement stays across matches.
    #[test]
    fn a_private_rooms_owner_arranges_its_teams() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, Some(1234), now);
        for member in 2..=4 {
            rooms.give_pin(member, 1234, now);
            rooms.join(member, 0, key, now).unwrap();
        }
        // Each on the side with fewer.
        assert_eq!(rooms.get(key).unwrap().arranged(), [vec![1, 3], vec![2, 4]]);
        assert_eq!(rooms.move_member(2, 0), Err(MoveRefused::NotOwner));
        assert_eq!(rooms.move_member(1, 9), Err(MoveRefused::NoMember));
        // The owner moves itself and member 3: 0 v 4, then 1 v 3.
        assert_eq!(rooms.move_member(1, 0), Ok(1));
        assert_eq!(rooms.move_member(1, 2), Ok(1));
        assert_eq!(
            rooms.get(key).unwrap().arranged(),
            [vec![], vec![1, 2, 3, 4]]
        );
        assert_eq!(rooms.move_member(1, 1), Ok(0));
        // Kept across a match; no move while it is played.
        rooms.started(key, 7, 61, &[1, 2, 3, 4], now);
        assert_eq!(rooms.move_member(1, 1), Err(MoveRefused::Playing));
        rooms.match_over(7);
        assert_eq!(rooms.get(key).unwrap().arranged(), [vec![2], vec![1, 3, 4]]);
        // A leaver is out of it; the next comes on the side with fewer.
        rooms.leave(3);
        rooms.give_pin(5, 1234, now);
        rooms.join(5, 0, key, now).unwrap();
        assert_eq!(rooms.get(key).unwrap().arranged(), [vec![2, 5], vec![1, 4]]);
        // An open room has none.
        let open = room_of(&mut rooms, 9, 6, None, now);
        assert_eq!(rooms.move_member(9, 0), Err(MoveRefused::Open));
        assert!(rooms.get(open).unwrap().sides.is_empty());
    }

    /// A side holds 10 at most: 10 v 1 takes no move onto the ten.
    #[test]
    fn a_private_rooms_side_holds_ten() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, Some(1234), now);
        for member in 2..=11 {
            rooms.give_pin(member, 1234, now);
            rooms.join(member, 0, key, now).unwrap();
        }
        // 1, 3, ... 11 on side 0 (6), the rest on side 1 (5): to 10 v 1.
        for place in 0..rooms.get(key).unwrap().members.len() {
            let member = rooms.get(key).unwrap().members[place];
            if rooms.get(key).unwrap().sides[&member] == 1 && member != 2 {
                assert_eq!(rooms.move_member(1, place), Ok(0));
            }
        }
        assert_eq!(rooms.get(key).unwrap().side_count(0), 10);
        assert_eq!(rooms.move_member(1, 1), Err(MoveRefused::Full));
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

    /// The owner's of 2026-10-20: who came by Play and played the match
    /// leaves the room after it; who chose the room stays; one by Play that
    /// did not play it stays for the next; choosing the room makes one a
    /// chosen member; a room all of Play's is gone after its match.
    #[test]
    fn who_came_by_play_and_played_leaves_after_the_match() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        for account in 2..=5 {
            rooms.join(account, 0, key, now).unwrap();
        }
        // 3, 4 and 5 by Play; 2 chose it; 1 made it.
        for account in 3..=5 {
            rooms.came_by_play(account);
        }
        // 4 chooses the room it is in: a chosen member now.
        rooms.join(4, 0, key, now).unwrap();
        // 5 came too late to go into the match.
        rooms.started(key, 7, POOL[0], &[1, 2, 3, 4], now);
        assert_eq!(rooms.match_over(7), Some((key, vec![3])));
        assert_eq!(rooms.get(key).unwrap().members, [1, 2, 4, 5]);
        assert_eq!(rooms.key_of(3), None);
        assert!(!rooms.get(key).unwrap().kicked.contains(&3));
        // 5 plays the next one, and leaves after it.
        rooms.started(key, 8, POOL[0], &[1, 2, 4, 5], now);
        assert_eq!(rooms.match_over(8), Some((key, vec![5])));
        // Back by a join: chosen, it stays.
        rooms.join(3, 0, key, now).unwrap();
        rooms.started(key, 9, POOL[0], &[1, 2, 3, 4], now);
        assert_eq!(rooms.match_over(9), Some((key, vec![])));

        // A room Play made, every member by Play: gone with its match.
        let quick = room_of(&mut rooms, 10, 8, None, now);
        rooms.came_by_play(10);
        rooms.join(11, 0, quick, now).unwrap();
        rooms.came_by_play(11);
        rooms.started(quick, 10, POOL[0], &[10, 11], now);
        assert_eq!(rooms.match_over(10), Some((quick, vec![10, 11])));
        assert!(rooms.get(quick).is_none());
        // A match called off is no match played: nobody leaves.
        let other = room_of(&mut rooms, 20, 8, None, now);
        rooms.join(21, 0, other, now).unwrap();
        rooms.came_by_play(21);
        rooms.started(other, 11, POOL[0], &[20, 21], now);
        assert_eq!(rooms.match_not_made(11), Some(other));
        assert_eq!(rooms.key_of(21), Some(other));
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

    #[test]
    fn an_empty_public_room_keeps_its_queue_but_cancellation_removes_both() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 8, None, now);
        rooms.enqueue_play(2, 0, key, now).unwrap();
        // Leaving this room would not free a room slot: its queue keeps it.
        assert_eq!(
            rooms.create(1, 0, spec(8, Some(42)), &[61], &[61], 1, now, &mut first),
            Err(Refused::TooMany)
        );
        assert_eq!(rooms.key_of(1), Some(key));
        rooms.leave(1);
        assert!(rooms.get(key).unwrap().members.is_empty());
        assert_eq!(rooms.queue_place(2), Some((key, 1)));
        rooms.dequeue(2);
        assert!(rooms.is_empty());
        assert!(rooms.play_queue().is_empty());
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
            door: Door::Open { max_level: 5 },
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

    /// A private room has no max level (the owner's of 2026-10-26): its
    /// PIN lets in the highest equipment and the lowest, its queue too, and
    /// its members always fit; Play never takes anyone into it.
    #[test]
    fn a_private_room_lets_in_any_equipment() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = rooms
            .create(1, 15, spec(8, Some(42)), &POOL, &POOL, 50, now, &mut first)
            .unwrap();
        let room = rooms.get(key).unwrap();
        assert!(room.lets_in(15) && room.lets_in(0));
        assert_eq!(room.max_level(), 0);
        rooms.give_pin(2, 42, now);
        assert_eq!(rooms.join(2, 15, key, now), Ok(()));
        rooms.give_pin(3, 42, now);
        assert_eq!(rooms.join(3, 0, key, now), Ok(()));
        assert_eq!(rooms.get(key).unwrap().members, [1, 2, 3]);
        rooms.give_pin(4, 42, now);
        assert_eq!(rooms.enqueue(4, 15, key, now), Ok(1));
        assert!(rooms.open_rooms(5, 0).next().is_none());
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

    /// A room's chat keeps its last [`CHAT_KEPT`] lines and goes with it:
    /// a room made later under the same key starts with none.
    #[test]
    fn a_rooms_chat_keeps_its_last_lines_and_goes_with_the_room() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        let line = |i: usize| ChatLine {
            from: 1,
            name: "Alice".to_owned(),
            tag: String::new(),
            body: format!("line {i}"),
        };
        for i in 0..CHAT_KEPT + 5 {
            rooms.say(key, line(i));
        }
        let chat = &rooms.get(key).unwrap().chat;
        assert_eq!(chat.len(), CHAT_KEPT);
        assert_eq!(chat.front(), Some(&line(5)));
        assert_eq!(chat.back(), Some(&line(CHAT_KEPT + 4)));
        // Nobody left: the room and its chat are gone; a line for it is
        // dropped.
        rooms.leave(1);
        rooms.say(key, line(0));
        assert!(rooms.get(key).is_none());
        let again = room_of(&mut rooms, 2, 6, None, now);
        assert!(rooms.get(again).unwrap().chat.is_empty());
    }
}
