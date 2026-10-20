//! The matches and their seats, and 0.21d's queue.
//!
//! A 0.26g match is seated at once by the lobby: a room's two sides
//! ([`Matchmaking::seat_sides_in_pool`], its members who come later on
//! [`Matchmaking::append_player`]), a clan match's two teams
//! ([`Matchmaking::seat_teams_in_pool`]), or one player against the
//! server's bots ([`Matchmaking::seat_solo`]). 0.26g has no queue: its
//! queue, the level search and the late joins it scored went with the
//! rooms (the owner's of 2026-10-17).
//!
//! 0.21d matchmaking. A 0.21d match has a fixed roster: every client is sent
//! the whole player list (0x51) before it loads, and a player not on it can
//! never join (`resources/notes/v021d/match_start_spec.md`). So players wait
//! in the queue for a gathering window, and then everyone waiting goes to a
//! new match together, up to its 20 places.
//! - A match needs two parties: two players alone, or a squad and at least
//!   one player who is not of it (alone, or of another squad). The window
//!   starts when the second party joins; a player or a squad alone waits
//!   for as long as it takes, and if the queue drops back to one party,
//!   the window starts over.
//! - A squad queues as one group and plays on one team, so a squad alone
//!   would have nobody against it: it is one party, whatever its size.
//!   (Until 2026-10-12 the count was of players, and two squadmates alone
//!   got a match with nobody in it but them.)
//! - A seat is kept for the whole match, so a player whose game or
//!   connection broke can come back to it, also from a new sign-in. A
//!   player who left the match by its own will (its client says so on the
//!   match link: [`Matchmaking::give_up_seat`]) has given the seat up:
//!   Play queues it for another match. (Until 2026-10-12 Play sent it back
//!   to the match it had just left, for as long as that match ran.) A seat
//!   given up is a place of the roster still, and the account may be
//!   seated in the same match again as a new place
//!   ([`Matchmaking::append_player`]; the owner's of 2026-10-19): its
//!   seats are told apart by their places, never by the account alone.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::LockExt;

/// `player_profile[20]` in the client's `match_options`.
pub const MAX_PLAYERS_PER_MATCH: usize = 20;
/// The first match id handed out.
const FIRST_MATCH_ID: u32 = 0x123;
/// With this many waiting (5 v 5), a match starts once nobody has joined the
/// queue for [`DEFAULT_QUIET_PERIOD`], without waiting out the whole window.
pub const DEFAULT_FILL_PLAYERS: usize = 10;
pub const DEFAULT_QUIET_PERIOD: Duration = Duration::from_secs(30);
/// How long the queue gathers players by default.
pub const DEFAULT_GATHER_WINDOW: Duration = Duration::from_secs(60);

/// A player who pressed Play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    pub session_id: u32,
    pub account_id: u32,
    /// The profile it plays with.
    pub profile_id: u32,
}

/// A roster place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub player: Player,
    /// 0 or 1.
    pub team: u8,
    /// Players of one squad share a non-zero id within the match.
    pub squad: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormedMatch {
    pub id: u32,
    pub pool: QueuePool,
    pub fixed_teams: bool,
    /// The roster: a seat's index is the player's index in the match.
    pub seats: Vec<Seat>,
}

/// Where a player who pressed Play stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueState {
    /// `place` counts from 1 over the players waiting.
    Waiting {
        place: usize,
        total: usize,
        since: Instant,
    },
    Placed {
        match_id: u32,
        team: u8,
        index: usize,
    },
}

/// A player's gear level (0.26g: `match_maker_defines.items_levels_modifiers`'
/// `item_level_min` of its row; `crate::v026g::matchmaking`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Level {
    pub min: u8,
}

/// One level, as a player's strength counts it: strengths are whole
/// numbers, so a part of a level can be told.
pub const STRENGTH_SCALE: i32 = 100;

/// Clients from different pools never share a match: a 0.26g game that
/// takes late joiners (`GrowingRoster`, patch revision 5 on) or not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum QueuePool {
    #[default]
    Standard,
    GrowingRoster,
}

/// A party that waits in 0.21d's queue.
#[derive(Clone, Debug, PartialEq, Eq)]
struct QueuedGroup {
    players: Vec<Player>,
    squad: bool,
    since: Instant,
}

impl QueuedGroup {
    /// How many parties it is: a squad is one (its players came together
    /// and play on one team), every other player one of its own.
    fn parties(&self) -> usize {
        if self.squad { 1 } else { self.players.len() }
    }
}

struct Inner {
    gather: Duration,
    fill_players: usize,
    quiet: Duration,
    /// When someone last pressed Play.
    last_joined: Option<Instant>,
    /// When the queue got its second party (a player alone or a squad:
    /// [`QueuedGroup::parties`]); `None` while it has fewer.
    gathering_since: Option<Instant>,
    next_match_id: u32,
    queue: Vec<QueuedGroup>,
    matches: HashMap<u32, Vec<Seat>>,
    match_pools: HashMap<u32, QueuePool>,
    fixed_teams: HashSet<u32>,
    formed_at: HashMap<u32, Instant>,
    /// The seated sessions a lobby connection has sent to their match. A
    /// seat is sent by whichever connection the session has, also one made
    /// after the one that pressed Play dropped.
    told: HashSet<u32>,
    /// The seats given up ([`Matchmaking::give_up_seat`]), by match and
    /// roster place. The place stays in the roster, which is fixed.
    given_up: HashSet<(u32, usize)>,
    /// `(match, account)`: who left its match by its own word
    /// ([`Matchmaking::leave_match`]) and the lobby was not told yet
    /// ([`Matchmaking::take_left`]).
    left: Vec<(u32, u32)>,
    /// `(match, account)`: late joiners the match server let go before
    /// they entered ([`Matchmaking::join_failed`]), the lobby not told yet
    /// ([`Matchmaking::take_failed_joins`]).
    failed_joins: Vec<(u32, u32)>,
}

pub struct Matchmaking {
    inner: Mutex<Inner>,
}

impl Matchmaking {
    pub fn new(gather: Duration) -> Self {
        Self {
            inner: Mutex::new(Inner {
                gather,
                fill_players: fill_from_env(),
                quiet: quiet_from_env(),
                last_joined: None,
                gathering_since: None,
                next_match_id: FIRST_MATCH_ID,
                queue: Vec::new(),
                matches: HashMap::new(),
                match_pools: HashMap::new(),
                fixed_teams: HashSet::new(),
                formed_at: HashMap::new(),
                told: HashSet::new(),
                given_up: HashSet::new(),
                left: Vec::new(),
                failed_joins: Vec::new(),
            }),
        }
    }

    /// Players press Play: one alone, or a whole squad (`squad`) once all
    /// its members are ready. A player with a seat in a match being played
    /// goes back to it instead, under its new session if it signed in again.
    /// A squad is one party, and a match needs two: a squad alone in the
    /// queue waits for somebody who is not of it.
    pub fn queue(&self, players: &[Player], squad: bool, now: Instant) {
        let mut inner = self.inner.locked();
        let waiting: Vec<Player> = players
            .iter()
            .filter(|player| !inner.back_to_seat(**player))
            .copied()
            .collect();
        if waiting.is_empty() {
            return;
        }
        let sessions: HashSet<u32> = waiting.iter().map(|player| player.session_id).collect();
        let accounts: HashSet<u32> = waiting.iter().map(|player| player.account_id).collect();
        inner.queue.retain(|group| {
            !group.players.iter().any(|player| {
                sessions.contains(&player.session_id) || accounts.contains(&player.account_id)
            })
        });
        let joined = waiting.len();
        inner.queue.push(QueuedGroup {
            players: waiting,
            squad,
            since: now,
        });
        inner.last_joined = Some(now);
        inner.track(now);
        log::info!(
            "Queue: {joined} joined{}; {} waiting",
            if squad { " as a squad" } else { "" },
            inner.waiting()
        );
    }

    /// The player has a seat in a match being played: it goes back to it,
    /// under its new session if it signed in again, and is told where the
    /// match is once more. `false`: it has none.
    pub fn back_to_seat(&self, player: Player) -> bool {
        self.inner.locked().back_to_seat(player)
    }

    /// Seat one player who is not in the queue straight into a match being
    /// played, on `team`: a room's member who comes while its match is
    /// played. A match of growing rosters with no fixed teams and a place
    /// left; nobody with a place anywhere. One whose place in it was given
    /// up comes as a new place (the old one stays given up). `reserve`
    /// runs under the lock (the match server's reservation) and must not
    /// call matchmaking again: the seat is kept only if it says yes.
    pub fn append_player(
        &self,
        match_id: u32,
        player: Player,
        team: u8,
        reserve: impl FnOnce(usize, &[Seat]) -> bool,
    ) -> bool {
        let mut inner = self.inner.locked();
        let queued = inner.queue.iter().any(|group| {
            group.players.iter().any(|waiting| {
                waiting.account_id == player.account_id || waiting.session_id == player.session_id
            })
        });
        if queued {
            return false;
        }
        let Some((first, seats)) = inner.late_seat(match_id, player, team) else {
            return false;
        };
        if !reserve(first, &seats) {
            return false;
        }
        inner
            .matches
            .get_mut(&match_id)
            .expect("match checked under lock")
            .extend(seats);
        true
    }

    /// A player leaves the queue, and its squad with it.
    pub fn leave_queue(&self, session_id: u32) {
        let mut inner = self.inner.locked();
        let before = inner.waiting();
        inner.queue.retain(|group| {
            !group
                .players
                .iter()
                .any(|player| player.session_id == session_id)
        });
        inner.track(Instant::now());
        let after = inner.waiting();
        if after != before {
            log::info!(
                "Queue: {} left (session {session_id:#x}); {after} waiting",
                before - after
            );
        }
    }

    /// Where a session stands now; `None` if it is neither waiting nor
    /// seated. Starts the matches whose players have gathered (0.21d).
    pub fn poll(&self, session_id: u32, now: Instant) -> Option<QueueState> {
        let mut inner = self.inner.locked();
        inner.form(now);
        if let Some((match_id, index, seat)) = inner.seat_of_session(session_id) {
            return Some(QueueState::Placed {
                match_id,
                team: seat.team,
                index,
            });
        }
        let total = inner.queue.iter().map(|group| group.players.len()).sum();
        let mut before = 0;
        for group in &inner.queue {
            if group
                .players
                .iter()
                .any(|player| player.session_id == session_id)
            {
                return Some(QueueState::Waiting {
                    place: before + 1,
                    total,
                    since: group.since,
                });
            }
            before += group.players.len();
        }
        None
    }

    /// Players waiting for matchmaking, counting every squad member separately.
    /// Reading this count does not form matches or advance the queue.
    pub fn queued_players(&self) -> usize {
        self.inner.locked().waiting()
    }

    /// Seat two given teams in a new match at once, each on its own side
    /// (a clan match: who waits for it is the lobby's to keep). `None`,
    /// and nothing changed, when a team is empty, the two do not fit a
    /// match, or one of the players has a seat in a match being played.
    /// The players leave the queue.
    pub fn seat_teams(&self, teams: [&[Player]; 2], now: Instant) -> Option<u32> {
        self.seat_teams_in_pool(teams, now, QueuePool::Standard)
    }

    pub fn seat_teams_in_pool(
        &self,
        teams: [&[Player]; 2],
        now: Instant,
        pool: QueuePool,
    ) -> Option<u32> {
        self.seat_given_sides(teams, now, pool, true, &|_| 0)
    }

    /// Seat the two sides of a room's match at once, as
    /// [`Self::seat_teams_in_pool`] does, but with teams that are not fixed:
    /// the match takes late joiners (the room's members who come while it
    /// is played; `session::v026g::rooms`). `squad_of` gives each account
    /// its squad's id in the match (0: none).
    pub fn seat_sides_in_pool(
        &self,
        teams: [&[Player]; 2],
        now: Instant,
        pool: QueuePool,
        squad_of: &dyn Fn(u32) -> u8,
    ) -> Option<u32> {
        self.seat_given_sides(teams, now, pool, false, squad_of)
    }

    fn seat_given_sides(
        &self,
        teams: [&[Player]; 2],
        now: Instant,
        pool: QueuePool,
        fixed: bool,
        squad_of: &dyn Fn(u32) -> u8,
    ) -> Option<u32> {
        let mut inner = self.inner.locked();
        let players = || teams.iter().flat_map(|team| team.iter());
        if teams.iter().any(|team| team.is_empty())
            || players().count() > MAX_PLAYERS_PER_MATCH
            || players().any(|player| inner.seat_of_account_mut(player.account_id).is_some())
        {
            return None;
        }
        let accounts: HashSet<u32> = players().map(|player| player.account_id).collect();
        inner.queue.retain(|group| {
            !group
                .players
                .iter()
                .any(|player| accounts.contains(&player.account_id))
        });
        inner.track(now);
        let id = inner.next_match_id;
        inner.next_match_id += 1;
        let seats: Vec<Seat> = teams
            .iter()
            .enumerate()
            .flat_map(|(team, players)| {
                players.iter().map(move |player| Seat {
                    player: *player,
                    team: team as u8,
                    squad: squad_of(player.account_id),
                })
            })
            .collect();
        log::info!(
            "Match {id:#x} formed for two teams of {} and {} players",
            teams[0].len(),
            teams[1].len()
        );
        inner.matches.insert(id, seats);
        inner.match_pools.insert(id, pool);
        if fixed {
            inner.fixed_teams.insert(id);
        }
        inner.formed_at.insert(id, now);
        Some(id)
    }

    /// Seat one player alone in a new match at once, on `team`, its teams
    /// fixed: a match against the server's own bots ("Player vs Bots"),
    /// whose places the match server adds after it.
    /// No rule of two parties, of levels or of a gather window; and as a
    /// match of fixed teams it never takes a late joiner (`late_seats`).
    /// `None`, and nothing changed, when the player has a seat in a match
    /// being played or `team` is over 1. The player leaves the queue.
    pub fn seat_solo(
        &self,
        player: Player,
        team: u8,
        now: Instant,
        pool: QueuePool,
    ) -> Option<u32> {
        let mut inner = self.inner.locked();
        if team > 1 || inner.seat_of_account_mut(player.account_id).is_some() {
            return None;
        }
        inner.queue.retain(|group| {
            !group
                .players
                .iter()
                .any(|queued| queued.account_id == player.account_id)
        });
        inner.track(now);
        let id = inner.next_match_id;
        inner.next_match_id += 1;
        log::info!("Match {id:#x} formed for one player against the server's bots");
        inner.matches.insert(
            id,
            vec![Seat {
                player,
                team,
                squad: 0,
            }],
        );
        inner.match_pools.insert(id, pool);
        inner.fixed_teams.insert(id);
        inner.formed_at.insert(id, now);
        Some(id)
    }

    /// The account's player left its match by its own will: the seat is
    /// given up, and Play queues the player for another match instead of
    /// sending it back. Its place stays in the roster (nobody else can
    /// take it). Returns the match, or `None` if the account has no seat.
    pub fn give_up_seat(&self, account_id: u32) -> Option<u32> {
        self.inner.locked().give_up_seat(account_id)
    }

    /// [`Self::give_up_seat`] for a player that left its match by its own
    /// word (its client's 0xE0): the lobby hears of it once
    /// ([`Self::take_left`]) and takes it out of its room (the owner's of
    /// 2026-10-19).
    pub fn leave_match(&self, account_id: u32) -> Option<u32> {
        let mut inner = self.inner.locked();
        let match_id = inner.give_up_seat(account_id)?;
        inner.left.push((match_id, account_id));
        Some(match_id)
    }

    /// `(match, account)` of every [`Self::leave_match`] since the last
    /// call, in order.
    pub fn take_left(&self) -> Vec<(u32, u32)> {
        std::mem::take(&mut self.inner.locked().left)
    }

    /// [`Self::give_up_seat`] for a late joiner the match server let go
    /// before it entered (its game never came, left, or did not catch up):
    /// the lobby hears of it once ([`Self::take_failed_joins`]), and a
    /// room's member waits in its room again, to go in at its next turn.
    pub fn join_failed(&self, account_id: u32) -> Option<u32> {
        let mut inner = self.inner.locked();
        let match_id = inner.give_up_seat(account_id)?;
        inner.failed_joins.push((match_id, account_id));
        Some(match_id)
    }

    /// `(match, account)` of every [`Self::join_failed`] since the last
    /// call, in order.
    pub fn take_failed_joins(&self) -> Vec<(u32, u32)> {
        std::mem::take(&mut self.inner.locked().failed_joins)
    }

    /// A lobby connection sent the session to its match.
    pub fn mark_told(&self, session_id: u32) {
        self.inner.locked().told.insert(session_id);
    }

    /// Whether the session was sent to the match it is seated in.
    pub fn is_told(&self, session_id: u32) -> bool {
        self.inner.locked().told.contains(&session_id)
    }

    /// Compatibility of an account's retained seat, if it has one.
    pub fn pool_of_account(&self, account_id: u32) -> Option<QueuePool> {
        let inner = self.inner.locked();
        let (id, _, _) = inner.live_seat_of_account(account_id)?;
        inner.match_pools.get(&id).copied()
    }

    /// The match and roster index of a session's seat.
    pub fn seat(&self, session_id: u32) -> Option<(u32, usize)> {
        let inner = self.inner.locked();
        inner
            .seat_of_session(session_id)
            .map(|(match_id, index, _)| (match_id, index))
    }

    /// A match being played, as it was formed.
    pub fn formed_match(&self, match_id: u32) -> Option<FormedMatch> {
        let inner = self.inner.locked();
        let seats = inner.matches.get(&match_id)?.clone();
        Some(FormedMatch {
            id: match_id,
            pool: inner
                .match_pools
                .get(&match_id)
                .copied()
                .unwrap_or_default(),
            fixed_teams: inner.fixed_teams.contains(&match_id),
            seats,
        })
    }

    /// The match is over: its seats are free. A session that gave its seat
    /// up and sits in another match keeps being told of that one: clearing
    /// it sent the game to the match it was already playing again (it
    /// reloaded it, or started a match nobody simulated).
    pub fn end_match(&self, match_id: u32) {
        let mut inner = self.inner.locked();
        if let Some(seats) = inner.matches.remove(&match_id) {
            for (index, seat) in seats.into_iter().enumerate() {
                let session_id = seat.player.session_id;
                if inner.given_up.contains(&(match_id, index))
                    || inner.seat_of_session(session_id).is_some()
                {
                    continue;
                }
                inner.told.remove(&session_id);
            }
        }
        inner.given_up.retain(|(id, _)| *id != match_id);
        inner.left.retain(|(id, _)| *id != match_id);
        inner.formed_at.remove(&match_id);
        inner.match_pools.remove(&match_id);
        inner.fixed_teams.remove(&match_id);
    }

    /// The matches being played, with when each formed.
    pub fn matches(&self) -> Vec<(u32, Instant)> {
        let inner = self.inner.locked();
        inner.formed_at.iter().map(|(id, at)| (*id, *at)).collect()
    }

    pub fn match_count(&self) -> usize {
        self.inner.locked().matches.len()
    }

    /// The sessions waiting or seated: they must not end for being idle.
    pub fn busy_sessions(&self) -> HashSet<u32> {
        let inner = self.inner.locked();
        let waiting = inner.queue.iter().flat_map(|group| &group.players);
        let seated = inner.matches.values().flatten().map(|seat| &seat.player);
        waiting
            .chain(seated)
            .map(|player| player.session_id)
            .collect()
    }
}

impl Inner {
    /// The players waiting.
    fn waiting(&self) -> usize {
        self.queue.iter().map(|group| group.players.len()).sum()
    }

    /// The roster place `player` would take on `team` in a match being
    /// played, and its index; `None` when the match takes nobody: it is no
    /// match of growing rosters, its teams are fixed, `team` is over 1, the
    /// player has a place in a match (one it gave up aside: it comes again
    /// as a new place), or there is no room.
    fn late_seat(&self, match_id: u32, player: Player, team: u8) -> Option<(usize, Vec<Seat>)> {
        if team > 1
            || self.fixed_teams.contains(&match_id)
            || self.match_pools.get(&match_id) != Some(&QueuePool::GrowingRoster)
            || self.live_seat_of_account(player.account_id).is_some()
        {
            return None;
        }
        let roster = self.matches.get(&match_id)?;
        let first = roster.len();
        if first >= MAX_PLAYERS_PER_MATCH {
            return None;
        }
        let seat = Seat {
            player,
            team,
            squad: 0,
        };
        Some((first, vec![seat]))
    }

    /// The account's seat, unless it was given up.
    fn seat_of_account_mut(&mut self, account_id: u32) -> Option<&mut Seat> {
        let (match_id, index) = self
            .live_seat_of_account(account_id)
            .map(|(match_id, index, _)| (match_id, index))?;
        self.matches.get_mut(&match_id)?.get_mut(index)
    }

    /// [`Matchmaking::give_up_seat`].
    fn give_up_seat(&mut self, account_id: u32) -> Option<u32> {
        let (match_id, index, session_id) = {
            let (match_id, index, seat) = self.live_seat_of_account(account_id)?;
            (match_id, index, seat.player.session_id)
        };
        self.given_up.insert((match_id, index));
        self.told.remove(&session_id);
        Some(match_id)
    }

    /// [`Matchmaking::back_to_seat`].
    fn back_to_seat(&mut self, player: Player) -> bool {
        let Some(seat) = self.seat_of_account_mut(player.account_id) else {
            return false;
        };
        let before = std::mem::replace(&mut seat.player.session_id, player.session_id);
        self.told.remove(&before);
        true
    }

    /// [`Self::seat_of_account_mut`], with the match and the roster index:
    /// of the account's places, the one not given up (it has one at most).
    fn live_seat_of_account(&self, account_id: u32) -> Option<(u32, usize, &Seat)> {
        self.matches.iter().find_map(|(match_id, seats)| {
            let index = (0..seats.len()).rev().find(|&index| {
                seats[index].player.account_id == account_id
                    && !self.given_up.contains(&(*match_id, index))
            })?;
            Some((*match_id, index, &seats[index]))
        })
    }

    /// The session's seat, unless it was given up.
    fn seat_of_session(&self, session_id: u32) -> Option<(u32, usize, &Seat)> {
        self.matches.iter().find_map(|(match_id, seats)| {
            let index = (0..seats.len()).rev().find(|&index| {
                seats[index].player.session_id == session_id
                    && !self.given_up.contains(&(*match_id, index))
            })?;
            Some((*match_id, index, &seats[index]))
        })
    }

    /// The gathering window starts with the queue's second party and ends
    /// when it drops below two.
    fn track(&mut self, now: Instant) {
        let parties: usize = self.queue.iter().map(QueuedGroup::parties).sum();
        if parties < 2 {
            self.gathering_since = None;
        } else if self.gathering_since.is_none() {
            self.gathering_since = Some(now);
        }
    }

    /// Start a match for those waiting once the queue has had two parties for
    /// the gathering window, or at once when a match is full. Groups go in
    /// the order they came, as many as fit.
    fn form(&mut self, now: Instant) {
        loop {
            self.track(now);
            let Some(since) = self.gathering_since else {
                return;
            };
            let total: usize = self.queue.iter().map(|group| group.players.len()).sum();
            let settled = total >= self.fill_players
                && self
                    .last_joined
                    .is_none_or(|joined| now.saturating_duration_since(joined) >= self.quiet);
            if now.saturating_duration_since(since) < self.gather
                && total < MAX_PLAYERS_PER_MATCH
                && !settled
            {
                return;
            }
            let mut chosen = Vec::new();
            let mut count = 0;
            for (index, group) in self.queue.iter().enumerate() {
                let size = group.players.len();
                if count + size <= MAX_PLAYERS_PER_MATCH {
                    count += size;
                    chosen.push(index);
                }
            }
            // Each side needs somebody: a squad alone is one side.
            let parties: usize = chosen
                .iter()
                .map(|&index| self.queue[index].parties())
                .sum();
            if parties < 2 {
                return;
            }
            let mut taken: Vec<QueuedGroup> = chosen
                .iter()
                .rev()
                .map(|&index| self.queue.remove(index))
                .collect();
            taken.reverse();
            let id = self.next_match_id;
            self.next_match_id += 1;
            let seats = seat(taken);
            log::info!(
                "Match {id:#x} formed for {} players: {:?}",
                seats.len(),
                seats
                    .iter()
                    .map(|seat| (seat.player.session_id, seat.team))
                    .collect::<Vec<_>>()
            );
            self.matches.insert(id, seats);
            self.match_pools.insert(id, QueuePool::Standard);
            self.formed_at.insert(id, now);
            // Whoever is left waits a new window.
            self.gathering_since = None;
        }
    }
}

/// [`seat`]'s teams, by group: the largest first, each on the team with
/// fewer players.
fn seat_teams_of(groups: &[QueuedGroup]) -> Vec<u8> {
    let mut by_size: Vec<usize> = (0..groups.len()).collect();
    by_size.sort_by_key(|&index| std::cmp::Reverse(groups[index].players.len()));
    let mut team_of = vec![0; groups.len()];
    let mut sizes = [0; 2];
    for index in by_size {
        let team = usize::from(sizes[1] < sizes[0]);
        team_of[index] = team as u8;
        sizes[team] += groups[index].players.len();
    }
    team_of
}

/// Teams of the groups: the largest first, each on the team with fewer
/// players. Seats keep the order the groups came in.
fn seat(groups: Vec<QueuedGroup>) -> Vec<Seat> {
    let team_of = seat_teams_of(&groups);
    seats_of(groups, &team_of)
}

/// The seats of groups on the teams given, in the order the groups came.
fn seats_of(groups: Vec<QueuedGroup>, team_of: &[u8]) -> Vec<Seat> {
    let mut seats = Vec::new();
    let mut next_squad = 0;
    for (group, &team) in groups.into_iter().zip(team_of) {
        let squad = if group.squad && group.players.len() > 1 {
            next_squad += 1;
            next_squad
        } else {
            0
        };
        seats.extend(group.players.into_iter().map(|player| Seat {
            player,
            team,
            squad,
        }));
    }
    seats
}

/// `SRP_MATCH_FILL_PLAYERS` (default 10) and `SRP_MATCH_QUIET_SECONDS`
/// (default 30): with that many waiting and nobody joining for that long, the
/// match starts before the window ends. A bad value falls back to the default.
pub fn fill_from_env() -> usize {
    std::env::var("SRP_MATCH_FILL_PLAYERS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(DEFAULT_FILL_PLAYERS)
}

pub fn quiet_from_env() -> Duration {
    std::env::var("SRP_MATCH_QUIET_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .map_or(DEFAULT_QUIET_PERIOD, Duration::from_secs)
}

/// `SRP_MATCH_GATHER_SECONDS`: how long the queue gathers players before a
/// match starts (default 30; 0 starts one at once). Anything else is an
/// error naming the variable.
pub fn gather_window_from_env() -> Result<Duration, String> {
    gather_window(std::env::var("SRP_MATCH_GATHER_SECONDS").ok().as_deref())
}

fn gather_window(value: Option<&str>) -> Result<Duration, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_GATHER_WINDOW);
    };
    match value.trim().parse::<u16>() {
        Ok(seconds) => Ok(Duration::from_secs(seconds.into())),
        _ => Err(format!(
            "SRP_MATCH_GATHER_SECONDS is `{value}`; it must be a number of seconds"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: u32) -> Player {
        Player {
            session_id: id,
            account_id: 1000 + id,
            profile_id: 1,
        }
    }

    #[test]
    fn queued_players_counts_members_without_forming_matches() {
        let mm = Matchmaking::new(Duration::ZERO);
        let now = Instant::now();
        assert_eq!(mm.queued_players(), 0);
        mm.queue(&[player(1), player(2)], true, now);
        mm.queue(&[player(3)], false, now);
        assert_eq!(mm.queued_players(), 3);
        assert_eq!(mm.queued_players(), 3);
        assert_eq!(mm.match_count(), 0);
        mm.leave_queue(3);
        assert_eq!(mm.queued_players(), 2);
        mm.queue(&[player(3)], false, now);
        let Some(QueueState::Placed { match_id, .. }) = mm.poll(3, now) else {
            panic!("match should form");
        };
        assert_eq!(mm.queued_players(), 0);
        assert_eq!(mm.match_count(), 1);
        mm.end_match(match_id);
        assert_eq!(mm.queued_players(), 0);
    }

    /// A room's member who comes while its match is played is seated in
    /// it: only once the host reserved the place, never twice, never one
    /// who waits in the queue or has a place anywhere.
    #[test]
    fn a_player_out_of_the_queue_is_seated_into_a_match_being_played() {
        let mm = Matchmaking::new(Duration::ZERO);
        let now = Instant::now();
        let id = mm
            .seat_sides_in_pool(
                [&[player(1)], &[player(2)]],
                now,
                QueuePool::GrowingRoster,
                &|_| 0,
            )
            .expect("seated");
        // The host refuses: nothing changes.
        assert!(!mm.append_player(id, player(3), 1, |_, _| false));
        assert_eq!(mm.formed_match(id).unwrap().seats.len(), 2);
        assert_eq!(mm.seat(3), None);
        // A team that is none, a match that is none.
        assert!(!mm.append_player(id, player(3), 2, |_, _| panic!("team 2")));
        assert!(!mm.append_player(id + 1, player(3), 0, |_, _| panic!("no match")));
        assert!(mm.append_player(id, player(3), 1, |first, seats| {
            assert_eq!(first, 2);
            assert_eq!(
                seats,
                [Seat {
                    player: player(3),
                    team: 1,
                    squad: 0
                }]
            );
            true
        }));
        assert_eq!(mm.seat(3), Some((id, 2)));
        // Seated already; and one who waits in the queue.
        assert!(!mm.append_player(id, player(3), 0, |_, _| panic!("twice")));
        mm.queue(&[player(4)], false, now);
        assert!(!mm.append_player(id, player(4), 0, |_, _| panic!("queued")));
        // A place given up by the player's own word is still a place of
        // the match: one who comes again is a new place (the owner's of
        // 2026-10-19), live, its old one given up; the lobby hears of the
        // leave once.
        assert_eq!(mm.leave_match(player(3).account_id), Some(id));
        assert_eq!(mm.take_left(), [(id, player(3).account_id)]);
        assert_eq!(mm.take_left(), []);
        assert_eq!(mm.seat(3), None);
        assert_eq!(mm.pool_of_account(player(3).account_id), None);
        assert!(mm.append_player(id, player(3), 0, |first, _| {
            assert_eq!(first, 3);
            true
        }));
        assert_eq!(mm.formed_match(id).unwrap().seats.len(), 4);
        assert_eq!(mm.seat(3), Some((id, 3)));
        assert_eq!(
            mm.pool_of_account(player(3).account_id),
            Some(QueuePool::GrowingRoster)
        );
        // Back to its seat goes to the new place, never the old one.
        let again = Player {
            session_id: 33,
            ..player(3)
        };
        assert!(mm.back_to_seat(again));
        assert_eq!(mm.seat(33), Some((id, 3)));
        assert_eq!(mm.formed_match(id).unwrap().seats[2].player, player(3));
        // Seated again: not a third place while it is.
        assert!(!mm.append_player(id, again, 0, |_, _| panic!("seated")));
        // Its new place given up, and only that one: none is left.
        assert_eq!(mm.give_up_seat(player(3).account_id), Some(id));
        assert_eq!(mm.seat(33), None);
        assert_eq!(mm.give_up_seat(player(3).account_id), None);
        assert_eq!(mm.take_left(), []);
        // A late joiner the match server let go: its seat given up, and the
        // lobby hears of it once (not as a leave).
        assert!(mm.append_player(id, player(3), 0, |first, _| first == 4));
        assert_eq!(mm.join_failed(player(3).account_id), Some(id));
        assert_eq!(mm.seat(3), None);
        assert_eq!(mm.join_failed(player(3).account_id), None);
        assert_eq!(mm.take_failed_joins(), [(id, player(3).account_id)]);
        assert_eq!(mm.take_failed_joins(), []);
        assert_eq!(mm.take_left(), []);
        // A match of fixed teams, or of the standard queue, takes nobody
        // late.
        let clan = mm
            .seat_teams_in_pool([&[player(8)], &[player(9)]], now, QueuePool::GrowingRoster)
            .expect("seated");
        assert!(!mm.append_player(clan, player(7), 0, |_, _| panic!("fixed")));
        mm.queue(&[player(5), player(6)], false, now);
        let Some(QueueState::Placed { match_id, .. }) = mm.poll(5, now) else {
            panic!("a standard match");
        };
        assert!(!mm.append_player(match_id, player(7), 0, |_, _| panic!("standard")));
    }

    fn after(start: Instant, seconds: u64) -> Instant {
        start + Duration::from_secs(seconds)
    }

    #[test]
    fn a_seat_is_told_once_until_play_or_the_end_of_its_match() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::ZERO);
        matchmaking.queue(&[player(1)], false, start);
        matchmaking.queue(&[player(2)], false, start);
        let Some(QueueState::Placed { match_id, .. }) = matchmaking.poll(1, start) else {
            panic!("not placed");
        };
        assert!(!matchmaking.is_told(1));
        matchmaking.mark_told(1);
        assert!(matchmaking.is_told(1));
        // Play again (back to its match, from a new sign-in): to be sent again.
        matchmaking.queue(
            &[Player {
                session_id: 7,
                ..player(1)
            }],
            false,
            start,
        );
        assert!(!matchmaking.is_told(1));
        assert!(!matchmaking.is_told(7));
        matchmaking.mark_told(7);
        matchmaking.mark_told(2);
        matchmaking.end_match(match_id);
        assert!(!matchmaking.is_told(7));
        assert!(!matchmaking.is_told(2));
    }

    #[test]
    fn players_gather_for_the_window_then_start_together() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(10));
        matchmaking.queue(&[player(1)], false, start);
        matchmaking.queue(&[player(2)], false, after(start, 5));
        assert_eq!(
            matchmaking.poll(2, after(start, 14)),
            Some(QueueState::Waiting {
                place: 2,
                total: 2,
                since: after(start, 5)
            })
        );
        assert_eq!(matchmaking.match_count(), 0);

        let Some(QueueState::Placed {
            match_id,
            team,
            index,
        }) = matchmaking.poll(2, after(start, 15))
        else {
            panic!("the window ended");
        };
        assert_eq!((match_id, team, index), (FIRST_MATCH_ID, 1, 1));
        assert_eq!(
            matchmaking.poll(1, after(start, 15)),
            Some(QueueState::Placed {
                match_id,
                team: 0,
                index: 0
            })
        );
        assert_eq!(matchmaking.seat(2), Some((match_id, 1)));
        assert_eq!(matchmaking.formed_match(match_id).unwrap().seats.len(), 2);
    }

    #[test]
    fn one_player_alone_waits_and_the_window_starts_with_the_second() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(60));
        matchmaking.queue(&[player(1)], false, start);
        // Long past the window: still waiting alone.
        assert!(matches!(
            matchmaking.poll(1, after(start, 600)),
            Some(QueueState::Waiting { .. })
        ));
        matchmaking.queue(&[player(2)], false, after(start, 700));
        assert!(matches!(
            matchmaking.poll(1, after(start, 759)),
            Some(QueueState::Waiting { total: 2, .. })
        ));
        assert!(matches!(
            matchmaking.poll(1, after(start, 760)),
            Some(QueueState::Placed { .. })
        ));
    }

    #[test]
    fn the_window_starts_over_when_the_queue_is_down_to_one() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(60));
        matchmaking.queue(&[player(1)], false, start);
        matchmaking.queue(&[player(2)], false, after(start, 10));
        matchmaking.leave_queue(2);
        matchmaking.queue(&[player(3)], false, after(start, 50));
        // 60 s after player 2 joined, but only 20 s after the queue had two again.
        assert!(matches!(
            matchmaking.poll(1, after(start, 70)),
            Some(QueueState::Waiting { .. })
        ));
        assert!(matches!(
            matchmaking.poll(1, after(start, 110)),
            Some(QueueState::Placed { .. })
        ));
    }

    #[test]
    fn ten_waiting_start_once_nobody_joins_for_a_while() {
        let start = Instant::now();
        let mm = Matchmaking::new(DEFAULT_GATHER_WINDOW);
        for id in 1..=10 {
            mm.queue(&[player(id)], false, after(start, id.into()));
        }
        // Someone joined at 10 s: the quiet period (30 s) runs from there.
        assert!(
            mm.poll(1, after(start, 39))
                .is_some_and(|s| matches!(s, QueueState::Waiting { .. }))
        );
        assert!(matches!(
            mm.poll(1, after(start, 40)),
            Some(QueueState::Placed { .. })
        ));
        assert_eq!(mm.match_count(), 1);
    }

    #[test]
    fn nine_waiting_still_wait_the_whole_window() {
        let start = Instant::now();
        let mm = Matchmaking::new(DEFAULT_GATHER_WINDOW);
        for id in 1..=9 {
            mm.queue(&[player(id)], false, start);
        }
        assert!(matches!(
            mm.poll(1, after(start, 59)),
            Some(QueueState::Waiting { .. })
        ));
        assert!(matches!(
            mm.poll(1, after(start, 60)),
            Some(QueueState::Placed { .. })
        ));
    }

    #[test]
    fn thirty_five_players_make_a_full_match_then_one_when_settled_and_the_rest_wait() {
        let start = Instant::now();
        let mm = Matchmaking::new(DEFAULT_GATHER_WINDOW);
        for id in 1..=35 {
            mm.queue(&[player(id)], false, start);
        }
        // 20 at once, 15 left: they have been quiet since the start.
        mm.poll(1, start);
        assert_eq!(mm.match_count(), 1);
        mm.poll(1, after(start, 30));
        assert_eq!(mm.match_count(), 2);
        assert!(matches!(
            mm.poll(35, after(start, 30)),
            Some(QueueState::Placed { .. })
        ));
    }

    #[test]
    fn a_full_match_starts_at_once_and_the_rest_wait() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(60));
        for id in 0..21 {
            matchmaking.queue(&[player(id)], false, start);
        }
        assert!(matches!(
            matchmaking.poll(0, start),
            Some(QueueState::Placed { .. })
        ));
        assert_eq!(
            matchmaking.poll(20, start),
            Some(QueueState::Waiting {
                place: 1,
                total: 1,
                since: start
            })
        );
    }

    #[test]
    fn a_squad_plays_on_one_team() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::ZERO);
        matchmaking.queue(&[player(1)], false, start);
        matchmaking.queue(&[player(2), player(3), player(4)], true, start);
        matchmaking.queue(&[player(5)], false, start);
        matchmaking.poll(1, start);
        let seats = matchmaking.formed_match(FIRST_MATCH_ID).unwrap().seats;
        let teams: Vec<_> = seats.iter().map(|seat| (seat.team, seat.squad)).collect();
        assert_eq!(teams, [(1, 0), (0, 1), (0, 1), (0, 1), (1, 0)]);
    }

    #[test]
    fn two_given_teams_are_seated_at_once_each_on_its_side() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(60));
        // One of them also waits in the queue, with another player.
        matchmaking.queue(&[player(2)], false, start);
        matchmaking.queue(&[player(9)], false, start);
        assert_eq!(matchmaking.seat_teams([&[player(1)], &[]], start), None);
        let id = matchmaking
            .seat_teams([&[player(1), player(2)], &[player(3)]], start)
            .unwrap();
        let seats = matchmaking.formed_match(id).unwrap().seats;
        let teams: Vec<_> = seats
            .iter()
            .map(|seat| (seat.player.session_id, seat.team, seat.squad))
            .collect();
        assert_eq!(teams, [(1, 0, 0), (2, 0, 0), (3, 1, 0)]);
        assert_eq!(
            matchmaking.poll(3, start),
            Some(QueueState::Placed {
                match_id: id,
                team: 1,
                index: 2
            })
        );
        // The queue lost the seated player and is one player again.
        assert_eq!(
            matchmaking.poll(9, after(start, 600)),
            Some(QueueState::Waiting {
                place: 1,
                total: 1,
                since: start
            })
        );
        // A seated player is in no second match.
        assert_eq!(
            matchmaking.seat_teams([&[player(3)], &[player(4)]], start),
            None
        );
        matchmaking.end_match(id);
        assert!(
            matchmaking
                .seat_teams([&[player(3)], &[player(4)]], start)
                .is_some()
        );
    }

    #[test]
    fn a_squad_alone_waits_for_somebody_who_is_not_of_it() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(60));
        matchmaking.queue(&[player(1), player(2)], true, start);
        // Two players, but both of one side: long past the window, no match.
        assert_eq!(
            matchmaking.poll(1, after(start, 600)),
            Some(QueueState::Waiting {
                place: 1,
                total: 2,
                since: start
            })
        );
        assert_eq!(matchmaking.match_count(), 0);
        // One player alone is enough, and the window starts with it.
        matchmaking.queue(&[player(3)], false, after(start, 600));
        assert!(matches!(
            matchmaking.poll(1, after(start, 659)),
            Some(QueueState::Waiting { total: 3, .. })
        ));
        assert!(matches!(
            matchmaking.poll(1, after(start, 660)),
            Some(QueueState::Placed { .. })
        ));
        let seats = matchmaking.formed_match(FIRST_MATCH_ID).unwrap().seats;
        let team = |session: u32| {
            seats
                .iter()
                .find(|seat| seat.player.session_id == session)
                .unwrap()
                .team
        };
        assert_eq!(team(1), team(2));
        assert_ne!(team(1), team(3));
    }

    #[test]
    fn a_squad_is_left_waiting_when_the_other_side_leaves() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(60));
        matchmaking.queue(&[player(1), player(2), player(3)], true, start);
        matchmaking.queue(&[player(4)], false, after(start, 10));
        matchmaking.leave_queue(4);
        assert!(matches!(
            matchmaking.poll(1, after(start, 300)),
            Some(QueueState::Waiting { total: 3, .. })
        ));
        // Another squad is somebody too; the window starts over with it.
        matchmaking.queue(&[player(5), player(6)], true, after(start, 300));
        assert!(matches!(
            matchmaking.poll(1, after(start, 359)),
            Some(QueueState::Waiting { total: 5, .. })
        ));
        assert!(matches!(
            matchmaking.poll(5, after(start, 360)),
            Some(QueueState::Placed { .. })
        ));
        assert_eq!(matchmaking.match_count(), 1);
    }

    #[test]
    fn leaving_takes_the_whole_squad_out() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::from_secs(10));
        matchmaking.queue(&[player(1), player(2)], true, start);
        matchmaking.leave_queue(2);
        assert_eq!(matchmaking.poll(1, after(start, 20)), None);
        assert!(matchmaking.busy_sessions().is_empty());
    }

    /// A player who left its match by its own will is not sent back to
    /// it: Play queues it, and it can be seated in another match while the
    /// one it left still runs.
    #[test]
    fn a_player_who_left_its_match_queues_for_another() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::ZERO);
        matchmaking.queue(&[player(1), player(2)], false, start);
        matchmaking.poll(1, start);
        matchmaking.mark_told(2);

        // Without a word of its own, Play sends it back.
        matchmaking.queue(&[player(2)], false, after(start, 10));
        assert_eq!(matchmaking.seat(2), Some((FIRST_MATCH_ID, 1)));

        assert_eq!(
            matchmaking.give_up_seat(player(2).account_id),
            Some(FIRST_MATCH_ID)
        );
        assert_eq!(matchmaking.seat(2), None);
        assert!(!matchmaking.is_told(2));
        assert_eq!(matchmaking.give_up_seat(player(2).account_id), None);
        // The roster is as it was formed, and the other keeps its seat.
        assert_eq!(
            matchmaking
                .formed_match(FIRST_MATCH_ID)
                .unwrap()
                .seats
                .len(),
            2
        );
        assert_eq!(matchmaking.seat(1), Some((FIRST_MATCH_ID, 0)));

        // Play: it waits for a second player, as anybody.
        matchmaking.queue(&[player(2)], false, after(start, 20));
        assert!(matches!(
            matchmaking.poll(2, after(start, 20)),
            Some(QueueState::Waiting {
                place: 1,
                total: 1,
                ..
            })
        ));
        matchmaking.queue(&[player(3)], false, after(start, 30));
        let second = match matchmaking.poll(2, after(start, 30)) {
            Some(QueueState::Placed { match_id, .. }) => match_id,
            other => panic!("not seated in a new match: {other:?}"),
        };
        assert_ne!(second, FIRST_MATCH_ID);
        assert_eq!(matchmaking.match_count(), 2);
        matchmaking.mark_told(2);
        matchmaking.mark_told(3);

        // The match it left ends: nothing of it is left, and the new seat
        // stands. It was sent to the new match and is not sent again (a
        // second "goes to match" made its game load that match once more).
        matchmaking.end_match(FIRST_MATCH_ID);
        assert_eq!(matchmaking.seat(2).map(|(id, _)| id), Some(second));
        assert!(matchmaking.is_told(2));
        assert!(matchmaking.is_told(3));
        // In the new match it has a seat to give up like any other.
        assert_eq!(matchmaking.give_up_seat(player(2).account_id), Some(second));
    }

    #[test]
    fn a_seated_player_goes_back_to_its_match_from_a_new_sign_in() {
        let start = Instant::now();
        let matchmaking = Matchmaking::new(Duration::ZERO);
        matchmaking.queue(&[player(1), player(2)], false, start);
        matchmaking.poll(1, start);
        let again = Player {
            session_id: 77,
            ..player(2)
        };
        matchmaking.queue(&[again], false, after(start, 30));
        assert_eq!(matchmaking.seat(77), Some((FIRST_MATCH_ID, 1)));
        assert_eq!(matchmaking.seat(2), None);

        matchmaking.end_match(FIRST_MATCH_ID);
        assert_eq!(matchmaking.poll(77, after(start, 31)), None);
        assert_eq!(matchmaking.match_count(), 0);
    }

    /// A player alone against the server's bots: seated at once, out of the
    /// queue, the teams fixed; nobody joins it late.
    #[test]
    fn a_solo_match_is_seated_at_once_and_takes_no_late_joiner() {
        let mm = Matchmaking::new(Duration::from_secs(60));
        let start = Instant::now();
        mm.queue(&[player(1)], false, start);
        let id = mm
            .seat_solo(player(1), 0, start, QueuePool::GrowingRoster)
            .expect("seated");
        assert_eq!(mm.queued_players(), 0);
        assert_eq!(
            mm.poll(1, start),
            Some(QueueState::Placed {
                match_id: id,
                team: 0,
                index: 0
            })
        );
        let formed = mm.formed_match(id).unwrap();
        assert!(formed.fixed_teams);
        assert_eq!(formed.pool, QueuePool::GrowingRoster);
        assert_eq!(formed.seats.len(), 1);
        // Seated already: not twice; a team over 1 is none.
        assert_eq!(
            mm.seat_solo(player(1), 0, start, QueuePool::GrowingRoster),
            None
        );
        assert_eq!(
            mm.seat_solo(player(2), 2, start, QueuePool::GrowingRoster),
            None
        );
        // No late joiner.
        assert!(!mm.append_player(id, player(2), 1, |_, _| true));
        assert_eq!(mm.formed_match(id).unwrap().seats.len(), 1);
    }

    #[test]
    fn the_gathering_window_is_read_from_the_environment() {
        assert_eq!(gather_window(None), Ok(DEFAULT_GATHER_WINDOW));
        assert_eq!(gather_window(Some(" 0 ")), Ok(Duration::ZERO));
        assert_eq!(gather_window(Some("20")), Ok(Duration::from_secs(20)));
        let error = gather_window(Some("soon")).unwrap_err();
        assert!(error.contains("SRP_MATCH_GATHER_SECONDS"), "{error}");
    }
}
