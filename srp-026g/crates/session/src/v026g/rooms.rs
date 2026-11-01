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
//! - A squad (two at most) comes together, by its commander alone: a join
//!   takes the mate along when it may come ([`Rooms::join_with`]), listed
//!   right after its commander (in a private room on its team); in a queue
//!   the two are one entry, the commander's place ([`Waiting::Pair`]), let
//!   in together at two places on one side, or the commander alone when
//!   its mate may not come then ([`Rooms::take_from_queue`]; the owner's of
//!   2026-10-31, `docs/notes/squads.md`).
//! - After the match everyone is back in the room, and the next vote and
//!   wait begin, however each came in (Play or a join; the owner's of
//!   2026-10-31, [`Rooms::match_over`]).
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

use survarium::v026g::drops::Rng;

use super::level_stats;
use crate::Strength;

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

    /// The places not taken.
    pub fn free_places(&self) -> usize {
        self.capacity().saturating_sub(self.members.len())
    }

    /// Whether `comers` (one, or a squad of two) find places on one side:
    /// beside their mate `beside` when it is in, on its team. An open room
    /// has no teams before its match (its sides are drawn at the start, a
    /// squad on one, [`sides`]): its free places are enough. A private
    /// room's teams are its owner's arrangement: the comers need that many
    /// places on one team, the mate's when it is in (the owner's of
    /// 2026-10-31: a pair is never put on two teams).
    pub fn has_places(&self, comers: usize, beside: Option<u32>) -> bool {
        if self.free_places() < comers {
            return false;
        }
        if self.is_open() {
            return true;
        }
        let free = |side: u8| usize::from(PRIVATE_SIZE).saturating_sub(self.side_count(side));
        match beside.and_then(|mate| self.sides.get(&mate).copied()) {
            Some(side) => free(side) >= comers,
            None => free(0).max(free(1)) >= comers,
        }
    }

    /// A member comes: last in the list, or right after its squad mate
    /// `beside` when that one is in; in a private room on its mate's side
    /// while that has a place, else on the side with fewer (side 0 of two
    /// alike).
    fn add_member(&mut self, account_id: u32, beside: Option<u32>) {
        if self.members.is_empty() {
            self.owner = account_id;
        }
        let at = beside
            .and_then(|mate| self.members.iter().position(|member| *member == mate))
            .map_or(self.members.len(), |at| at + 1);
        self.members.insert(at, account_id);
        if !self.is_open() {
            let fewer = u8::from(self.side_count(1) < self.side_count(0));
            let side = beside
                .and_then(|mate| self.sides.get(&mate).copied())
                .filter(|&side| self.side_count(side) < usize::from(PRIVATE_SIZE))
                .unwrap_or(fewer);
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

/// Who has a place in a queue: one player, or a squad's commander with its
/// mate (the owner's of 2026-10-31: only the commander queues for the
/// squad). A pair has the one place, the commander's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
    One(u32),
    Pair { commander: u32, mate: u32 },
}

impl Waiting {
    /// The one who queued: a player alone, or the commander.
    pub fn lead(self) -> u32 {
        match self {
            Self::One(account_id)
            | Self::Pair {
                commander: account_id,
                ..
            } => account_id,
        }
    }

    /// Its accounts, the lead first.
    pub fn accounts(self) -> Vec<u32> {
        match self {
            Self::One(account_id) => vec![account_id],
            Self::Pair { commander, mate } => vec![commander, mate],
        }
    }

    fn has(self, account_id: u32) -> bool {
        self.accounts().contains(&account_id)
    }
}

/// When and how a queue entry came.
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
    queues: HashMap<u32, VecDeque<Waiting>>,
    /// The room each account of an entry waits for: one at a time, from
    /// the lobby only (a pair's two the same).
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
            sides: HashMap::new(),
            came_in_match: HashSet::new(),
        };
        room.add_member(owner, None);
        self.rooms.insert(key, room);
        self.of_account.insert(owner, key);
        self.dequeue(owner);
        Ok(key)
    }

    /// The account joins room `key`, with the PIN it gave last (within
    /// [`PIN_LIFE`]) and its equipment level `gear`; it leaves the room it
    /// was in. Joining the room it is in already changes nothing.
    pub fn join(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
    ) -> Result<(), Refused> {
        self.join_with(account_id, gear, None, key, now)
    }

    /// [`Self::join`] with the account's squad mate and its equipment level
    /// (the owner's of 2026-10-31: a pair starts together). The mate comes
    /// along when the room lets it in (its max level, not turned out); one
    /// it does not let in stays where it is. Places for every one who comes,
    /// or nobody moves ([`Refused::Full`]). The PIN is the account's. Each
    /// comes right after its mate in the list (and on its team in a private
    /// room), leaving the room and the queue it was in. A mate in the room
    /// already takes no place: the account comes beside it.
    pub fn join_with(
        &mut self,
        account_id: u32,
        gear: u8,
        mate: Option<(u32, u8)>,
        key: u32,
        now: Instant,
    ) -> Result<(), Refused> {
        let given = self.pins.remove(&account_id);
        let pin = given
            .filter(|(_, at)| now.saturating_duration_since(*at) <= PIN_LIFE)
            .map(|(pin, _)| pin);
        let room = self.rooms.get(&key).ok_or(Refused::Unknown)?;
        if room.members.contains(&account_id) {
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
        let beside = mate
            .map(|(mate, _)| mate)
            .filter(|mate| room.members.contains(mate));
        let comes = mate.filter(|&(mate, gear)| {
            mate != account_id
                && !room.members.contains(&mate)
                && room.lets_in(gear)
                && !room.kicked.contains(&mate)
        });
        if !room.has_places(1 + usize::from(comes.is_some()), beside) {
            // Its PIN is still there for the queue that follows.
            if let Some(given) = given {
                self.pins.insert(account_id, given);
            }
            return Err(Refused::Full);
        }
        self.enter(account_id, key, beside);
        if let Some((mate, _)) = comes {
            self.enter(mate, key, Some(account_id));
        }
        Ok(())
    }

    /// A commander in room `key` brings its mate (with its equipment level)
    /// in beside it, on its team in a private room: no PIN asked, the
    /// commander's entry carries the pair (the owner's of 2026-10-31). The
    /// room must let the mate in (its max level, not turned out) and have a
    /// place for it on the commander's side.
    pub fn bring_mate(
        &mut self,
        commander: u32,
        (mate, gear): (u32, u8),
        key: u32,
    ) -> Result<(), Refused> {
        let room = self.rooms.get(&key).ok_or(Refused::Unknown)?;
        if !room.members.contains(&commander) {
            return Err(Refused::Bad);
        }
        if room.members.contains(&mate) {
            return Ok(());
        }
        if room.kicked.contains(&mate) {
            return Err(Refused::Kicked);
        }
        if !room.lets_in(gear) {
            return Err(Refused::GearTooHigh);
        }
        if !room.has_places(1, Some(commander)) {
            return Err(Refused::Full);
        }
        self.enter(mate, key, Some(commander));
        Ok(())
    }

    /// The account leaves its room and queue and comes into room `key`
    /// (which has a place), beside `beside`.
    fn enter(&mut self, account_id: u32, key: u32, beside: Option<u32>) {
        self.leave(account_id);
        let serial = self.next_serial();
        let Some(room) = self.rooms.get_mut(&key) else {
            return;
        };
        room.add_member(account_id, beside);
        room.serial = serial;
        self.of_account.insert(account_id, key);
        // In a room: no queue any more.
        self.dequeue(account_id);
    }

    /// The rooms Play may put the account into: no PIN, `places` free (its
    /// squad's), its max level not under the account's equipment level
    /// `gear`, not one that turned it out.
    pub fn open_rooms(
        &self,
        account_id: u32,
        gear: u8,
        places: usize,
    ) -> impl Iterator<Item = &Room> {
        self.rooms.values().filter(move |room| {
            room.is_open()
                && room.free_places() >= places.max(1)
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
        room.sides.remove(&account_id);
        room.came_in_match.remove(&account_id);
        room.serial = serial;
        if room.members.is_empty() && !(room.is_open() && self.queues.contains_key(&key)) {
            self.rooms.remove(&key);
            // Its queue goes with it: nobody waits for a room that is gone.
            for waiting in self.queues.remove(&key).unwrap_or_default() {
                for account_id in waiting.accounts() {
                    self.queued.remove(&account_id);
                }
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
            for account_id in waiting.accounts() {
                self.queued.remove(&account_id);
                gone.push(account_id);
            }
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

    /// The account (a squad's commander) and its mate (with its equipment
    /// level) wait in room `key`'s queue as one entry, the account's place
    /// ([`Waiting::Pair`]), on the account's PIN: its place, or why not
    /// (then nothing changed). The mate the room does not let in (its max
    /// level, turned out) or that is in it already stays out of it: the
    /// account waits alone. A mate idle in another room waits with it and
    /// leaves that room at their turn ([`Self::admit_party`]), as a Join
    /// with places takes it. Queuing again for the same room keeps the
    /// place.
    pub fn enqueue_party(
        &mut self,
        account_id: u32,
        gear: u8,
        mate: Option<(u32, u8)>,
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
        let entry = match mate.filter(|&(mate, mate_gear)| {
            mate != account_id
                && self.of_account.get(&mate) != Some(&key)
                && room.lets_in(mate_gear)
                && !room.kicked.contains(&mate)
        }) {
            Some((mate, _)) => Waiting::Pair {
                commander: account_id,
                mate,
            },
            None => Waiting::One(account_id),
        };
        let mine = self.queued.get(&account_id).copied();
        let since = mine.map_or(now, |queued| queued.since);
        let lead_at = |rooms: &Self| {
            rooms.queues.get(&key).and_then(|queue| {
                queue
                    .iter()
                    .position(|waiting| waiting.lead() == account_id)
            })
        };
        match lead_at(self).filter(|_| mine.is_some_and(|queued| queued.key == key)) {
            // The same room: its place kept, the entry as it is now.
            Some(at) => {
                let before = self.queues[&key][at];
                for gone in before.accounts() {
                    if !entry.has(gone) {
                        self.queued.remove(&gone);
                    }
                }
                for account in entry.accounts() {
                    if !before.has(account) {
                        self.dequeue(account);
                    }
                }
                if let Some(at) = lead_at(self)
                    && let Some(queue) = self.queues.get_mut(&key)
                {
                    queue[at] = entry;
                }
            }
            None => {
                for account in entry.accounts() {
                    self.dequeue(account);
                }
                self.queues.entry(key).or_default().push_back(entry);
            }
        }
        for account in entry.accounts() {
            self.queued.insert(account, Queued { key, choice, since });
        }
        Ok(self.queue_place(account_id).map_or(0, |(_, place)| place))
    }

    fn enqueue_as(
        &mut self,
        account_id: u32,
        gear: u8,
        key: u32,
        now: Instant,
        choice: QueueChoice,
    ) -> Result<usize, Refused> {
        self.enqueue_party(account_id, gear, None, key, now, choice)
    }

    /// The account leaves the queue it waits in, if any: its room's key. A
    /// commander takes its pair's entry out whole; its mate leaves it alone
    /// (the commander keeps the place, alone).
    pub fn dequeue(&mut self, account_id: u32) -> Option<u32> {
        let key = self.queued.remove(&account_id)?.key;
        if let Some(queue) = self.queues.get_mut(&key)
            && let Some(at) = queue.iter().position(|waiting| waiting.has(account_id))
        {
            match queue[at] {
                Waiting::Pair { commander, mate } if mate == account_id => {
                    queue[at] = Waiting::One(commander);
                }
                waiting => {
                    queue.remove(at);
                    for other in waiting.accounts() {
                        self.queued.remove(&other);
                    }
                }
            }
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

    /// Every pair entry whose squad no longer holds (`holds(commander,
    /// mate)` false: the squad broke, or another commands it) becomes two
    /// entries at its place, the commander's first, then its mate's: nobody
    /// loses its place, each waits on its own from then on.
    pub fn split_pairs(&mut self, holds: &dyn Fn(u32, u32) -> bool) {
        for queue in self.queues.values_mut() {
            let mut at = 0;
            while at < queue.len() {
                if let Waiting::Pair { commander, mate } = queue[at]
                    && !holds(commander, mate)
                {
                    queue[at] = Waiting::One(commander);
                    queue.insert(at + 1, Waiting::One(mate));
                    at += 1;
                }
                at += 1;
            }
        }
    }

    /// The room an account waits for and its place there, 1 the first.
    pub fn queue_place(&self, account_id: u32) -> Option<(u32, usize)> {
        let key = self.queued.get(&account_id)?.key;
        let place = self
            .queues
            .get(&key)?
            .iter()
            .position(|waiting| waiting.has(account_id))?;
        Some((key, place + 1))
    }

    /// The account's queue entry.
    pub fn waiting_of(&self, account_id: u32) -> Option<Waiting> {
        let key = self.queued.get(&account_id)?.key;
        self.queues
            .get(&key)?
            .iter()
            .copied()
            .find(|waiting| waiting.has(account_id))
    }

    /// The room's queue, the first first.
    pub fn queue_of(&self, key: u32) -> Vec<Waiting> {
        self.queues
            .get(&key)
            .map(|queue| queue.iter().copied().collect())
            .unwrap_or_default()
    }

    /// How many places the room's queue has (a pair's one).
    pub fn queue_len(&self, key: u32) -> usize {
        self.queues.get(&key).map_or(0, VecDeque::len)
    }

    /// How many players wait in all the queues.
    pub fn queued_total(&self) -> usize {
        self.queued.len()
    }

    /// Play's waiters across all rooms, oldest first, an entry each (a
    /// pair's one). Repeated Play does not change their position; choosing
    /// a room's queue removes them.
    pub fn play_queue(&self) -> Vec<(Waiting, u32)> {
        let mut waiting: Vec<_> = self
            .queues
            .iter()
            .flat_map(|(&key, queue)| queue.iter().map(move |&waiting| (key, waiting)))
            .filter_map(|(key, waiting)| {
                let queued = self.queued.get(&waiting.lead())?;
                (queued.choice == QueueChoice::Play).then_some((
                    queued.since,
                    waiting.lead(),
                    waiting,
                    key,
                ))
            })
            .collect();
        waiting.sort_unstable_by_key(|&(since, lead, _, _)| (since, lead));
        waiting
            .into_iter()
            .map(|(_, _, waiting, key)| (waiting, key))
            .collect()
    }

    /// The first of the room's queue, while it has a place free: out of the
    /// queue, for the lobby to put in (`admit`) if it still may come.
    pub fn next_in_queue(&mut self, key: u32) -> Option<u32> {
        self.take_from_queue(key, &|waiting| Some((waiting, None)))
            .map(|(waiting, _)| waiting.lead())
    }

    /// The first entry of the room's queue that goes in now, out of the
    /// queue: `goes` says who of an entry goes (a pair, or its commander
    /// alone when its mate may not come now; nobody: passed over, it keeps
    /// its place) and the mate in the room it goes beside; it must find
    /// that many places on one side ([`Room::has_places`]). One passed over
    /// for places keeps its place; one behind it may take a place.
    pub fn take_from_queue(
        &mut self,
        key: u32,
        goes: &dyn Fn(Waiting) -> Option<(Waiting, Option<u32>)>,
    ) -> Option<(Waiting, QueueChoice)> {
        let room = self.rooms.get(&key)?;
        let queue = self.queues.get(&key)?;
        let (at, going) = queue.iter().enumerate().find_map(|(at, &waiting)| {
            let (going, beside) = goes(waiting)?;
            room.has_places(going.accounts().len(), beside)
                .then_some((at, going))
        })?;
        let entry = queue[at];
        let choice = self.queued.get(&entry.lead())?.choice;
        if let Some(queue) = self.queues.get_mut(&key) {
            queue.remove(at);
        }
        for account_id in entry.accounts() {
            self.queued.remove(&account_id);
        }
        if self.queues.get(&key).is_some_and(VecDeque::is_empty) {
            self.queues.remove(&key);
        }
        Some((going, choice))
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
    /// Right after its squad mate `beside` when that one is in.
    pub fn admit(&mut self, account_id: u32, key: u32, beside: Option<u32>) -> bool {
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
        room.add_member(account_id, beside);
        room.serial = serial;
        self.of_account.insert(account_id, key);
        true
    }

    /// [`Self::admit`] for a party out of the queue together (a squad):
    /// all of them on places of one side ([`Room::has_places`]), each
    /// after the one before (the first beside `beside`), or none.
    pub fn admit_party(&mut self, accounts: &[u32], key: u32, beside: Option<u32>) -> bool {
        let Some(room) = self.rooms.get(&key) else {
            return false;
        };
        let Some((&lead, mates)) = accounts.split_first() else {
            return false;
        };
        if self.of_account.contains_key(&lead)
            || mates
                .iter()
                .any(|mate| self.of_account.get(mate) == Some(&key))
            || !room.has_places(accounts.len(), beside)
        {
            return false;
        }
        // A mate idle in another room leaves it, as its commander's Join
        // would take it.
        for &mate in mates {
            self.leave(mate);
        }
        let mut beside = beside;
        for &account_id in accounts {
            self.admit(account_id, key, beside);
            beside = Some(account_id);
        }
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
    /// room waits for the next one (the next tick starts its vote). The
    /// room's key.
    pub fn match_over(&mut self, match_id: u32) -> Option<u32> {
        let room = self.rooms.values_mut().find(
            |room| matches!(room.stage, Stage::InMatch { match_id: id, .. } if id == match_id),
        )?;
        // The cards and the votes for the next match stay: they were cast
        // while this one was played.
        room.stage = Stage::Waiting;
        room.in_match.clear();
        room.came_in_match.clear();
        room.after_match = true;
        Some(room.key)
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

/// One of the parties the sides of a room's match are drawn with: a player
/// alone, or a squad, which plays on one side; and what it counts for,
/// its members' K/Ds at the match's equipment level (`super::level_stats`;
/// `docs/notes/kd-balance.md`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party {
    members: Vec<u32>,
    strength: Strength,
}

impl Party {
    /// The party of `members` (each account with its K/D), in their
    /// order: one is a player alone, two or more a squad, each of whose
    /// members counts [`level_stats::SQUAD_FACTOR`] times its K/D (the owner's of
    /// 2026-11-01: "squads have their kd multiplied by 1.3").
    pub fn new(members: Vec<(u32, Strength)>) -> Self {
        let squad = members.len() > 1;
        let strength = members
            .iter()
            .map(|&(_, each)| {
                if squad {
                    level_stats::in_squad(each)
                } else {
                    each
                }
            })
            .sum();
        Self {
            members: members
                .into_iter()
                .map(|(account_id, _)| account_id)
                .collect(),
            strength,
        }
    }

    pub fn members(&self) -> &[u32] {
        &self.members
    }

    /// What it counts for, beside its players being there.
    pub fn strength(&self) -> Strength {
        self.strength
    }
}

/// How far each party's strength is shaken before the sides are made,
/// either way, in thousandths: 100 is a factor drawn from 0.9 to 1.1 (the
/// owner's of 2026-11-01: "we can add small noise", so the same players
/// do not always play together).
pub const NOISE_MILLI: u32 = 100;

/// The sides of a match of a room: `parties` on two sides of at most
/// `size`, a party too large for a side split into parts that fit (each
/// part counting its share). Each party's strength is first shaken by a
/// factor drawn by `seed` within [`NOISE_MILLI`], so the same parties and
/// seed always make the same sides. Of the ways to put the parties whole,
/// every one is tried: of those that fit the sizes and whose sides are
/// the fewest players apart past one, the one whose sides count the most
/// alike (`player` a player, and the shaken strengths: the owner's "k/d
/// equalizer", so with an odd count the stronger players are the side of
/// fewer). The largest party (the first of those) is on side 0. When no
/// way fits, the larger parties go first, each to the side with fewer
/// players, split where it must be.
pub fn sides(parties: &[Party], size: usize, player: Strength, seed: u32) -> [Vec<u32>; 2] {
    let mut rng = Rng::new(u64::from(seed) ^ 0x9E37_79B9_7F4A_7C15);
    let parties: Vec<(Vec<u32>, Strength)> = parties
        .iter()
        .flat_map(|party| {
            let factor = 1000 - NOISE_MILLI + rng.below(2 * NOISE_MILLI + 1);
            let each = party
                .strength
                .ratio(i64::from(factor), 1000 * party.members.len().max(1) as i64);
            party
                .members
                .chunks(size.max(1))
                .map(move |chunk| (chunk.to_vec(), each.times(chunk.len())))
        })
        .collect();
    let count = parties.len();
    // What a way is worth, the less the better. Bit `i` of `mask`: party
    // `i` is on side 1.
    let score = |mask: u32| {
        let mut players = [0_usize; 2];
        let mut strength = [Strength::ZERO; 2];
        for (index, (members, each)) in parties.iter().enumerate() {
            let side = (mask >> index & 1) as usize;
            players[side] += members.len();
            strength[side] += *each;
        }
        let over: usize = players.iter().map(|&side| side.saturating_sub(size)).sum();
        let apart = players[0].abs_diff(players[1]);
        let worth = |side: usize| player.times(players[side]) + strength[side];
        (over, apart.saturating_sub(1), worth(0).abs_diff(worth(1)))
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
fn fill(mut parties: Vec<(Vec<u32>, Strength)>, size: usize) -> [Vec<u32>; 2] {
    parties.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(b.1.cmp(&a.1)));
    let mut sides: [(Vec<u32>, Strength); 2] = Default::default();
    for (members, strength) in parties {
        let fits = |side: &(Vec<u32>, Strength)| side.0.len() + members.len() <= size;
        let order = if (sides[0].0.len(), sides[0].1) <= (sides[1].0.len(), sides[1].1) {
            [0, 1]
        } else {
            [1, 0]
        };
        let Some(side) = order.into_iter().find(|&side| fits(&sides[side])) else {
            let each = strength.ratio(1, members.len().max(1) as i64);
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

    /// The owner's of 2026-10-31: after its match every member is back in
    /// the room, who played it and who came too late to go in, its maker
    /// too; a room is not gone with its match.
    #[test]
    fn every_member_is_back_in_the_room_after_its_match() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        for account in 2..=5 {
            rooms.join(account, 0, key, now).unwrap();
        }
        // 5 came too late to go into the match.
        rooms.started(key, 7, POOL[0], &[1, 2, 3, 4], now);
        assert_eq!(rooms.match_over(7), Some(key));
        let room = rooms.get(key).unwrap();
        assert_eq!(room.members, [1, 2, 3, 4, 5]);
        assert_eq!(room.stage, Stage::Waiting);
        assert!(room.in_match.is_empty());
        // A room of two: both stay, the room with them.
        let pair = room_of(&mut rooms, 10, 8, None, now);
        rooms.join(11, 0, pair, now).unwrap();
        rooms.started(pair, 10, POOL[0], &[10, 11], now);
        assert_eq!(rooms.match_over(10), Some(pair));
        assert_eq!(rooms.get(pair).unwrap().members, [10, 11]);
        assert_eq!(
            (rooms.key_of(10), rooms.key_of(11)),
            (Some(pair), Some(pair))
        );
    }

    #[test]
    fn a_room_of_one_squad_plays_it_on_both_sides() {
        let [a, b] = sides(&[party(&[1, 2, 3, 4], 1000)], 6, KD, 0);
        assert_eq!((a.len(), b.len()), (2, 2));
        let [a, b] = sides(&[party(&[1, 2], 1000)], 8, KD, 0);
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
        assert!(rooms.admit(30, key, None));
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
        assert!(rooms.open_rooms(2, 6, 1).next().is_none());
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
        assert!(rooms.open_rooms(5, 0, 1).next().is_none());
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
            party(&[1, 2, 3], 1000),
            party(&[4], 2000),
            party(&[5], 500),
            party(&[6], 1000),
            party(&[7], 1000),
        ];
        let [a, b] = sides(&parties, 6, KD, 0);
        assert!(a.len().abs_diff(b.len()) <= 1);
        let together = |side: &Vec<u32>| [1, 2, 3].iter().all(|m| side.contains(m));
        assert!(together(&a) || together(&b));
        assert_eq!(a.len() + b.len(), 7);
        // A squad larger than a side is split.
        let eight: Vec<u32> = (1..=8).collect();
        let [a, b] = sides(&[party(&eight, 1000)], 6, KD, 0);
        assert_eq!((a.len(), b.len()), (6, 2));
        // Two squads of 5 and one of 2 in a 6 v 6: the 2 are split, not left out.
        let [a, b] = sides(
            &[
                party(&[1, 2, 3, 4, 5], 1000),
                party(&[6, 7, 8, 9, 10], 1000),
                party(&[11, 12], 1000),
            ],
            6,
            KD,
            0,
        );
        assert_eq!((a.len(), b.len()), (6, 6));
    }

    #[test]
    fn the_sides_count_alike_and_an_odd_count_puts_the_stronger_on_the_side_of_fewer() {
        // Five alone, two of them of K/D 1.5, the others 1, a player 1:
        // the two strong ones are the two.
        let parties: Vec<Party> = [1000, 1000, 1500, 1000, 1500]
            .into_iter()
            .enumerate()
            .map(|(index, kd)| party(&[index as u32 + 1], kd))
            .collect();
        let [a, b] = sides(&parties, 8, KD, 0);
        let (mut fewer, more) = if a.len() < b.len() { (a, b) } else { (b, a) };
        fewer.sort_unstable();
        assert_eq!((fewer, more.len()), (vec![3, 5], 3));
        // Four alone: two a side, the strong ones apart.
        let [a, b] = sides(&parties[1..], 8, KD, 0);
        assert_eq!((a.len(), b.len()), (2, 2));
        assert!(a.contains(&3) != a.contains(&5));
    }

    /// An average player's strength, one a player.
    const KD: Strength = level_stats::DEFAULT_KD;

    /// A party whose members are each of strength `milli` thousandths.
    fn party(members: &[u32], milli: i64) -> Party {
        Party::new(
            members
                .iter()
                .map(|&member| (member, Strength::from_milli(milli)))
                .collect(),
        )
    }

    /// What a side of [`sides`] counts, of these members' strengths.
    fn strength_of(side: &[u32], strengths: &[(u32, i64)]) -> i64 {
        side.iter()
            .map(|member| strengths.iter().find(|(each, _)| each == member).unwrap().1)
            .sum()
    }

    /// The owner's of 2026-11-01, "k/d equalizer": players that can be
    /// split even are, whatever the seed, when no other way is near.
    #[test]
    fn the_sides_are_as_even_as_they_can_be() {
        // 24 in all: 12 a side, the two strongest apart (every other way
        // is 6 or more apart, past twice what the noise can move: 2.4).
        let strengths = [
            (1, 7000),
            (2, 1000),
            (3, 4000),
            (4, 7000),
            (5, 1000),
            (6, 4000),
        ];
        let parties: Vec<Party> = strengths.iter().map(|&(id, s)| party(&[id], s)).collect();
        for seed in 0..50 {
            let [a, b] = sides(&parties, 8, KD, seed);
            assert_eq!((a.len(), b.len()), (3, 3));
            assert_eq!(
                (strength_of(&a, &strengths), strength_of(&b, &strengths)),
                (12000, 12000)
            );
            assert!(a.contains(&1) != a.contains(&4));
        }
        // Eight players of 1 and two of 5 in a 5 v 5: one 5 a side.
        let mut strengths: Vec<(u32, i64)> = (1..=8).map(|id| (id, 1000)).collect();
        strengths.extend([(9, 5000), (10, 5000)]);
        let parties: Vec<Party> = strengths.iter().map(|&(id, s)| party(&[id], s)).collect();
        for seed in 0..50 {
            let [a, b] = sides(&parties, 5, KD, seed);
            assert_eq!((a.len(), b.len()), (5, 5));
            assert!(a.contains(&9) != a.contains(&10));
            assert_eq!(strength_of(&a, &strengths), strength_of(&b, &strengths));
        }
    }

    /// The owner's of 2026-11-01: a squad's members count 1.3 times, and
    /// the sides are made even with that.
    #[test]
    fn a_squad_counts_one_and_three_tenths() {
        let squad = party(&[1, 2], 1000);
        assert_eq!(squad.strength(), Strength::from_milli(2600));
        assert_eq!(party(&[3], 1000).strength(), Strength::from_milli(1000));
        // A squad of two of 5 (13) and four alone (0.5, 3.5, 5, 6) in a
        // 3 v 3: the squad's third is the one of 0.5 (13.5 against 14.5).
        // Without the 1.3 (10) it would be the one of 3.5 (13.5 against
        // 11.5, where 0.5 makes 10.5 against 14.5).
        let alone = [(3, 500), (4, 3500), (5, 5000), (6, 6000)];
        let mut parties = vec![party(&[1, 2], 5000)];
        parties.extend(alone.iter().map(|&(id, s)| party(&[id], s)));
        for seed in 0..50 {
            let [a, b] = sides(&parties, 3, KD, seed);
            let mut with_squad = if a.contains(&1) { a } else { b };
            with_squad.sort_unstable();
            assert_eq!(with_squad, [1, 2, 3]);
        }
    }

    /// The owner's of 2026-11-01, "we can add small noise": players alike
    /// are put apart otherwise by each seed, so the same players do not
    /// always play together; the same seed always makes the same sides.
    #[test]
    fn the_noise_varies_the_sides_and_a_seed_always_makes_the_same() {
        let parties: Vec<Party> = (1..=8).map(|id| party(&[id], 1000)).collect();
        let drawn: HashSet<[Vec<u32>; 2]> = (0..40)
            .map(|seed| {
                let made = sides(&parties, 4, KD, seed);
                assert_eq!(made, sides(&parties, 4, KD, seed));
                assert_eq!((made[0].len(), made[1].len()), (4, 4));
                made
            })
            .collect();
        assert!(drawn.len() > 5, "{drawn:?}");
    }

    /// A squad is never split while the sizes let it be whole, whatever
    /// the strengths and the seed; and the same parties and seed always
    /// make the same sides.
    #[test]
    fn squads_are_never_split_and_a_seed_always_makes_the_same_sides() {
        let mut state = 0x2545_f491_u32;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for _ in 0..200 {
            let players = 4 + next() % 17;
            let mut parties = Vec::new();
            let mut id = 1;
            while id <= players {
                let squad = id < players && next() % 3 == 0;
                let members: Vec<(u32, Strength)> = (id..id + 1 + u32::from(squad))
                    .map(|member| (member, Strength::from_milli(i64::from(next() % 4000))))
                    .collect();
                id += members.len() as u32;
                parties.push(Party::new(members));
            }
            // A place a side more than half: five pairs are 6 v 4.
            let size = players.div_ceil(2) as usize + 1;
            let seed = next();
            let made = sides(&parties, size, KD, seed);
            assert_eq!(made, sides(&parties, size, KD, seed));
            assert_eq!(made[0].len() + made[1].len(), players as usize);
            assert!(made[0].len().abs_diff(made[1].len()) <= 2, "{made:?}");
            for party in parties.iter().filter(|party| party.members().len() > 1) {
                let on = |side: &Vec<u32>| party.members().iter().all(|m| side.contains(m));
                assert!(on(&made[0]) || on(&made[1]), "{made:?} split {party:?}");
            }
        }
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

    /// A pair joins whole or not at all; the mate comes right after the one
    /// who joined; a mate the room does not let in stays where it is.
    #[test]
    fn a_pair_joins_together_or_not_at_all() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        for account in 2..=11 {
            rooms.join(account, 0, key, now).unwrap();
        }
        // One place left: the pair is refused, nobody of it moves.
        let other = room_of(&mut rooms, 20, 6, None, now);
        rooms.join(21, 0, other, now).unwrap();
        assert_eq!(
            rooms.join_with(20, 0, Some((21, 0)), key, now),
            Err(Refused::Full)
        );
        assert_eq!(rooms.key_of(20), Some(other));
        assert_eq!(rooms.key_of(21), Some(other));
        // A place frees: both come, together in the list, out of their room.
        rooms.leave(5);
        rooms.join_with(20, 0, Some((21, 0)), key, now).unwrap();
        let members = &rooms.get(key).unwrap().members;
        assert_eq!(members.len(), 12);
        assert_eq!(&members[members.len() - 2..], &[20, 21]);
        assert!(rooms.get(other).is_none());
        // A mate in the room already: the other comes beside it.
        rooms.leave(20);
        rooms.leave(2);
        rooms.join_with(20, 0, Some((21, 0)), key, now).unwrap();
        let members = &rooms.get(key).unwrap().members;
        let at = members.iter().position(|member| *member == 21).unwrap();
        assert_eq!(members[at + 1], 20);
        // A mate over the max level stays where it is; the other comes.
        let capped = rooms
            .create(
                40,
                3,
                Spec {
                    door: Door::Open { max_level: 5 },
                    ..spec(6, None)
                },
                &POOL,
                &POOL,
                50,
                now,
                &mut first,
            )
            .unwrap();
        rooms.join_with(42, 3, Some((43, 6)), capped, now).unwrap();
        assert_eq!(rooms.key_of(42), Some(capped));
        assert_eq!(rooms.key_of(43), None);
        assert_eq!(
            rooms
                .open_rooms(50, 0, 2)
                .map(|room| room.key)
                .collect::<Vec<_>>(),
            [capped]
        );
    }

    /// In a private room the mate comes on its mate's team, while that team
    /// has a place.
    #[test]
    fn a_pair_comes_on_one_team_of_a_private_room() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, Some(1234), now);
        rooms.give_pin(2, 1234, now);
        rooms.join(2, 0, key, now).unwrap();
        // Teams 1 v 1: the pair goes on one of them, not one on each.
        rooms.give_pin(3, 1234, now);
        rooms.join_with(3, 0, Some((4, 0)), key, now).unwrap();
        let room = rooms.get(key).unwrap();
        assert_eq!(room.sides.get(&3), room.sides.get(&4));
        assert_eq!(room.arranged().map(|side| side.len()), [3, 1]);
    }

    /// A pair in a queue goes in at two free places, together; one alone
    /// behind it takes a single place, the pair keeping its places.
    #[test]
    fn a_queued_pair_waits_for_two_places() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, None, now);
        for account in 2..=12 {
            rooms.join(account, 0, key, now).unwrap();
        }
        // The commander queues for both: one place, its own.
        assert_eq!(
            rooms.enqueue_party(30, 0, Some((31, 0)), key, now, QueueChoice::Chosen),
            Ok(1)
        );
        assert_eq!(rooms.enqueue(32, 0, key, now), Ok(2));
        let pair = Waiting::Pair {
            commander: 30,
            mate: 31,
        };
        assert_eq!(rooms.waiting_of(31), Some(pair));
        assert_eq!(rooms.queue_place(31), Some((key, 1)));
        assert_eq!(rooms.queue_len(key), 2);
        let both = |waiting: Waiting| Some((waiting, None));
        // One place: not the pair, the one alone behind it.
        rooms.leave(12);
        assert_eq!(
            rooms.take_from_queue(key, &both),
            Some((Waiting::One(32), QueueChoice::Chosen))
        );
        assert!(rooms.admit(32, key, None));
        assert_eq!(rooms.queue_place(30), Some((key, 1)));
        // Two places: the pair, together.
        rooms.leave(32);
        rooms.leave(11);
        assert_eq!(
            rooms.take_from_queue(key, &both),
            Some((pair, QueueChoice::Chosen))
        );
        assert_eq!(rooms.queued_total(), 0);
        // A mate that may not come now: its commander goes alone, the
        // entry used up.
        rooms
            .enqueue_party(30, 0, Some((31, 0)), key, now, QueueChoice::Chosen)
            .unwrap();
        assert_eq!(
            rooms.take_from_queue(key, &|_| Some((Waiting::One(30), None))),
            Some((Waiting::One(30), QueueChoice::Chosen))
        );
        assert_eq!(rooms.queue_place(31), None);
        // The mate leaving the queue leaves its commander's place to it.
        rooms
            .enqueue_party(30, 0, Some((31, 0)), key, now, QueueChoice::Chosen)
            .unwrap();
        rooms.dequeue(31);
        assert_eq!(rooms.waiting_of(30), Some(Waiting::One(30)));
        // The commander leaving it takes both out.
        rooms
            .enqueue_party(30, 0, Some((31, 0)), key, now, QueueChoice::Chosen)
            .unwrap();
        rooms.dequeue(30);
        assert_eq!((rooms.queue_place(30), rooms.queue_place(31)), (None, None));
    }

    /// A private room's teams: a pair needs two places on one team, one
    /// whose mate is in needs one on its mate's team; one place on each
    /// team is no place for a pair, which waits in the queue (on its PIN)
    /// and comes in when one team has two.
    #[test]
    fn a_pair_needs_two_places_on_one_team_of_a_private_room() {
        let now = Instant::now();
        let mut rooms = Rooms::default();
        let key = room_of(&mut rooms, 1, 6, Some(1234), now);
        for account in 2..=18 {
            rooms.give_pin(account, 1234, now);
            rooms.join(account, 0, key, now).unwrap();
        }
        // 9 v 9: one place on each team.
        assert_eq!(
            rooms.get(key).unwrap().arranged().map(|side| side.len()),
            [9, 9]
        );
        rooms.give_pin(30, 1234, now);
        assert_eq!(
            rooms.join_with(30, 0, Some((31, 0)), key, now),
            Err(Refused::Full)
        );
        assert_eq!((rooms.key_of(30), rooms.key_of(31)), (None, None));
        // The PIN is still there for the queue: both wait in it.
        assert_eq!(
            rooms.enqueue_party(30, 0, Some((31, 0)), key, now, QueueChoice::Chosen),
            Ok(1)
        );
        assert_eq!(rooms.queue_place(31), Some((key, 1)));
        let both = |waiting: Waiting| Some((waiting, None));
        assert!(rooms.take_from_queue(key, &both).is_none());
        // A member of team 2 leaves: two places on it, the pair comes there.
        let team_2 = rooms.get(key).unwrap().arranged()[1][0];
        rooms.leave(team_2);
        let (party, _) = rooms.take_from_queue(key, &both).unwrap();
        assert!(rooms.admit_party(&party.accounts(), key, None));
        let room = rooms.get(key).unwrap();
        assert_eq!((room.sides[&30], room.sides[&31]), (1, 1));
        // 9 v 10: one whose mate is on the full team has no place, one
        // alone has the one left, a pair none.
        assert!(!room.has_places(1, Some(30)));
        assert!(room.has_places(1, None));
        assert!(!room.has_places(2, None));
    }
}
