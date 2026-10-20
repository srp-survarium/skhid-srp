//! Rooms (the players' "servers"): the lobby's side of
//! `session::v026g::rooms`, whose rules are the owner's of 2026-10-16, and
//! of its wire (`survarium::v026g::lobby::rooms`, `lobby.md` §2.16.3).
//! Every PvP match is a room's (the owner's of 2026-10-17: the queue, the
//! "matches now" list and its JOIN went).
//!
//! srp's changed lobby movie asks with the ranking page's request at the
//! rooms' reserved starts, as "Player vs Bots" does: the
//! list of the rooms, the asker's own room, the maps a pool may hold, and
//! the actions (create, join, leave, vote, kick). Every action is answered
//! with the asker's room as it is after it ([`lobby::rooms::room_state`]).
//!
//! What the lobby does for a room:
//!
//! - **Its match.** When its countdown is over (`Rooms::tick`), the members
//!   who are in the lobby and free, each with its active kit if Play would
//!   take it, are put on two sides (`rooms::sides`: squads together, the
//!   sides as even as they can be by what their players count for) and
//!   seated in a match of teams that are not fixed
//!   (`Matchmaking::seat_sides_in_pool`), on the map the vote chose and
//!   with none of the server's bots; then "match found" and `82`
//!   as for any Play. Its rewards are any match's.
//! - **Late.** A member in the lobby while the room's match is played sees
//!   the room first and goes into it `LATE_JOIN_WAIT` later (or at once on
//!   its ask) if the match admits players (from its countdown until
//!   `LOAD_MARGIN` is left), on the side with fewer players in it; of two
//!   alike in that, the side it played on in that match before, then the
//!   one that counts for less (`matchmaking_policy::place_joiner`);
//!   otherwise it waits for the next match. While it waits it is told why it
//!   cannot go in now (`late_hold`: a joiner on its way, a game loading,
//!   the last 30 s, the end, full, a maintenance; the owner's of 2026-10-20).
//! - **After.** The match over, its players are in the lobby again and in
//!   the room, whose next vote and wait begin.
//! - **Play.** Play with nothing else chosen puts the player into an open
//!   room (the one nearest its level, however far, then the fullest), or makes
//!   one for it (8 v 8, any max level, 30 s, the whole pool), and answers
//!   with the lobby state: the movie shows the room.
//! - **Remembered.** A room made by hand (a create, not Play) is its maker's
//!   next create window, kept with the account (`room_remember`; not the
//!   PIN; the owner's of 2026-10-20); the window asks for it fitted to the
//!   pool and the maker's equipment now (`room_settings::form`).
//! - A member whose game is gone (crashed, killed, its link silent) keeps
//!   its place while its session is kept: the link's 30 s and the session's
//!   60 s, about 90 s in all (the owner's of 2026-10-18); then it leaves the
//!   room and its queue, and may join again as a new member. Its game
//!   started anew within that keeps the place; a game quit from its menu
//!   (`FA`) leaves the room at once.
//! - A member that leaves the room's match by its own word (0xE0) leaves
//!   the room (`room_left_matches`); joining it again while that match is
//!   played, it goes into it late as a new place (the owner's of
//!   2026-10-19).
//!
//! # Settings (environment, read once at start)
//!
//! | variable | default | meaning |
//! |---|---|---|
//! | `SRP_ROOMS_MAX` | 50 | the most rooms at once, 1 to 1000 |
//! | `SRP_ROOMS_EXTRA_MAPS` | (none) | playable `maps_dict` ids a creator may add to a pool besides the match pool (`SRP_MATCH_MAPS`): the experimental maps |

use std::collections::HashSet;
use std::time::{Duration, Instant};

use session::matchmaking_policy::{self as policy, Closed, Worth};
use session::v026g::room_settings::{self, RoomSettings};
use session::v026g::rooms::{self as model, Due, Refused, Spec, Stage};
use session::v026g::{matchmaking, quests, rules};
use session::{LockExt, QueuePool};
use survarium::v026g::lobby::rooms::{
    self as wire, Card, Fit, JoinHold, Member, PoolEntry, RoomCode, RoomPhase, RoomRequest,
    RoomRow, RoomSpec, RoomSwitch, RoomView,
};
use survarium::v026g::lobby::{self, ClientState};
use survarium::v026g::maps;

use super::MAINTENANCE_WINDOW;
use super::core::{Core, Who};
use super::play::{MatchPlayer, Running, match_profile};

/// `SRP_ROOMS_MAX` when unset.
pub const DEFAULT_MAX_ROOMS: usize = 50;
/// What a room Play makes is made with (any max level, the whole pool):
/// 8 a side, the owner's word of 2026-10-18 ("8vs8 not 10vs10").
const QUICK_SIZE: u8 = 8;
const QUICK_WAIT: u8 = 30;
/// How long a member who comes while its room's match is played sees the
/// room before it is put into the match, unless it asks first (the owner's
/// of 2026-10-19).
pub const LATE_JOIN_WAIT: Duration = Duration::from_secs(15);

/// The rooms' settings in the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `SRP_ROOMS_MAX` (default 50).
    pub max_rooms: usize,
    /// `SRP_ROOMS_EXTRA_MAPS`: playable ids a pool may hold besides the
    /// match pool.
    pub extra_maps: Vec<u8>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_rooms: DEFAULT_MAX_ROOMS,
            extra_maps: Vec::new(),
        }
    }
}

impl Config {
    /// The environment's.
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok();
        let max_rooms = match var("SRP_ROOMS_MAX") {
            Some(value) => parse_max(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_ROOMS_MAX is `{value}`; it must be 1 to 1000. Using {DEFAULT_MAX_ROOMS}"
                );
                DEFAULT_MAX_ROOMS
            }),
            None => DEFAULT_MAX_ROOMS,
        };
        let extra_maps = match var("SRP_ROOMS_EXTRA_MAPS") {
            Some(value) => parse_extra(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_ROOMS_EXTRA_MAPS is `{value}`; it must list playable maps_dict ids. Using \
                     none"
                );
                Vec::new()
            }),
            None => Vec::new(),
        };
        log::info!(
            "Rooms: {max_rooms} at most, {} experimental maps",
            extra_maps.len()
        );
        Self {
            max_rooms,
            extra_maps,
        }
    }
}

/// `SRP_ROOMS_MAX`: 1 to 1000.
fn parse_max(value: &str) -> Option<usize> {
    value
        .trim()
        .parse()
        .ok()
        .filter(|most| (1..=1000).contains(most))
}

/// `SRP_ROOMS_EXTRA_MAPS`: playable ids, comma-separated; empty is none.
fn parse_extra(value: &str) -> Option<Vec<u8>> {
    value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| {
            let id: u8 = id.parse().ok()?;
            maps::map_mode(id)
                .filter(|map| map.is_playable())
                .map(|_| id)
        })
        .collect()
}

/// The wire's code for a refusal of the model's.
fn code_of(refused: Refused) -> RoomCode {
    match refused {
        Refused::Unknown => RoomCode::Unknown,
        Refused::WrongPin => RoomCode::WrongPin,
        Refused::Full => RoomCode::Full,
        Refused::TooMany => RoomCode::TooMany,
        Refused::Bad => RoomCode::Bad,
        Refused::Kicked => RoomCode::Kicked,
        Refused::GearTooHigh => RoomCode::GearTooHigh,
    }
}

/// A free member of a room, ready for its match.
struct Ready {
    player: session::Player,
    strength: i32,
}

impl Core<'_> {
    /// The maps a pool may hold, in the order their bits name them: the
    /// match pool, then the experimental maps.
    pub(super) fn room_entries(&self) -> Vec<PoolEntry> {
        let pool = &self.settings.maps;
        let extra = self
            .settings
            .rooms
            .extra_maps
            .iter()
            .filter(|id| !pool.contains(id));
        pool.iter()
            .map(|id| (*id, false))
            .chain(extra.map(|id| (*id, true)))
            .filter_map(|(id, extra)| {
                let map = maps::map_mode(id)?;
                Some(PoolEntry {
                    map_id: id,
                    level: map.level,
                    weather: map.weather,
                    mode: map.mode,
                    extra,
                })
            })
            // A create names its entries by bit: no more than the bits.
            .take(wire::POOL_BITS)
            .collect()
    }

    /// Whether `who` may make or join a room now.
    fn room_fit(&self, who: &Who) -> Result<(), RoomCode> {
        self.room_fit_of(who.account_id)
    }

    /// Whether the account may be put in a room now: its game in the lobby
    /// and free (no match, no search), not in a clan team, no maintenance
    /// near.
    fn room_fit_of(&self, account_id: u32) -> Result<(), RoomCode> {
        let Some(&addr) = self.by_account.get(&account_id) else {
            return Err(RoomCode::InMatch);
        };
        let Some(player) = self.player(addr) else {
            return Err(RoomCode::InMatch);
        };
        if self.pool_for_sessions([player.session_id]) != Some(QueuePool::GrowingRoster) {
            return Err(RoomCode::Outdated);
        }
        if player.in_match.is_some()
            || player.connect.is_some()
            || player.queued_since.is_some()
            || self
                .store
                .matchmaking()
                .pool_of_account(account_id)
                .is_some()
        {
            return Err(RoomCode::InMatch);
        }
        if self.accounts.clans().team_of(account_id).is_some() {
            return Err(RoomCode::ClanTeam);
        }
        if self.maintenance_is_near() {
            return Err(RoomCode::Maintenance);
        }
        Ok(())
    }

    fn maintenance_is_near(&self) -> bool {
        self.store
            .minutes_to_maintenance()
            .is_some_and(|minutes| minutes <= MAINTENANCE_WINDOW)
    }

    /// A room request (`3B` at a rooms' start).
    pub(super) fn room_request(&mut self, who: &Who, request: RoomRequest, now: Instant) {
        match request {
            RoomRequest::List { count } => {
                let list = self.room_list(who, count, now);
                self.send_made(who.addr, list);
            }
            RoomRequest::State => self.send_room_state(who, RoomCode::None, now),
            RoomRequest::Pool { page } => {
                let entries = self.room_entries();
                self.send_made(who.addr, wire::room_pool(&entries, page));
            }
            // The create window's start: the asker's last room made by hand,
            // fitted to the pool and its equipment now.
            RoomRequest::Form => {
                let entries = self.room_entries();
                let form = {
                    let account = who.account.locked();
                    let gear = matchmaking::gear_level(&account, quests::now());
                    room_settings::form(account.room_settings.as_ref(), &entries, gear)
                };
                self.send_made(who.addr, wire::room_form(&form));
            }
            RoomRequest::Bits { chunk, bits } => {
                let entries = self.rooms.draft(who.account_id, chunk, bits, now);
                self.send_made(who.addr, wire::bits_answer(chunk, entries));
            }
            RoomRequest::Pin { pin } => {
                self.rooms.give_pin(who.account_id, pin, now);
                self.send_made(who.addr, wire::pin_answer());
            }
            // The movie's Play with rooms on: straight into a room, no
            // search shown; the state answers it.
            RoomRequest::Play => {
                // A place in a match (closed early, or signed in again):
                // back to it, as the game's own Play does (`play`).
                let player = session::Player {
                    session_id: who.session_id,
                    account_id: who.account_id,
                    profile_id: who.account.locked().active_kit,
                };
                if self.store.matchmaking().back_to_seat(player) {
                    log::info!("{} pressed Play: back to its seat", who.name);
                    if let Some(state) = self.player_mut(who.addr) {
                        state.queued_since = Some(now);
                    }
                    self.queue_status(who.addr, now);
                } else {
                    self.room_quick(who, now);
                }
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Leave => {
                if let Some(key) = self.rooms.leave(who.account_id) {
                    log::info!("{}: left room {key}", who.name);
                }
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Vote { card } => {
                if self.rooms.vote(who.account_id, card) {
                    log::debug!("{}: votes for card {card}", who.name);
                }
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Kick { place, check } => {
                // A kick made from an older list (a member came or went since)
                // does nothing: its place may be another member now.
                let fresh = self
                    .rooms
                    .of(who.account_id)
                    .is_some_and(|room| room.serial % wire::KICK_CHECKS == check);
                let kicked = if fresh {
                    self.rooms.kick(who.account_id, usize::from(place))
                } else {
                    None
                };
                if let Some(member) = kicked {
                    let name = self.name_of(member).unwrap_or_default();
                    log::info!("{}: took {name} out of its room", who.name);
                    self.room_codes.insert(member, RoomCode::Kicked);
                }
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Join { key } => {
                let code = match self.room_fit(who) {
                    Err(code) => code,
                    Ok(()) => match self.rooms.join(
                        who.account_id,
                        matchmaking::gear_level(&who.account.locked(), quests::now()),
                        key,
                        now,
                    ) {
                        Ok(()) => {
                            log::info!("{}: joined room {key}", who.name);
                            RoomCode::None
                        }
                        Err(refused) => code_of(refused),
                    },
                };
                if code != RoomCode::None {
                    log::info!("{}: join room {key}: refused ({code:?})", who.name);
                }
                // Into the room first: its match, if one is played, after
                // `LATE_JOIN_WAIT` or on the member's ask (the tick).
                self.send_room_state(who, code, now);
            }
            // A full room's queue (the owner's of 2026-10-17): from the lobby
            // only, one at a time; a room with a place is joined at once.
            RoomRequest::Queue { key } => {
                let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
                let full = self.rooms.get(key).is_some_and(model::Room::is_full);
                let code = match self.room_fit(who) {
                    Err(code) => code,
                    Ok(()) if !full => match self.rooms.join(who.account_id, gear, key, now) {
                        Ok(()) => {
                            log::info!("{}: joined room {key} (a place was free)", who.name);
                            RoomCode::None
                        }
                        Err(refused) => code_of(refused),
                    },
                    Ok(()) => match self.rooms.enqueue(who.account_id, gear, key, now) {
                        Ok(place) => {
                            log::info!("{}: waits in room {key}'s queue, place {place}", who.name);
                            RoomCode::None
                        }
                        Err(refused) => code_of(refused),
                    },
                };
                self.send_room_state(who, code, now);
            }
            RoomRequest::JoinMatch => {
                // The member does not wait: into its room's match now.
                self.room_join_at.insert(who.account_id, now);
                self.room_late_join(who.account_id, now);
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Unqueue => {
                if let Some(key) = self.rooms.dequeue(who.account_id) {
                    log::info!("{}: left room {key}'s queue", who.name);
                }
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Create(spec) => {
                let code = match self.room_fit(who) {
                    Err(code) => code,
                    Ok(()) => match self.room_create(who, spec, now) {
                        Ok(key) => {
                            self.room_remember(who, key);
                            RoomCode::None
                        }
                        Err(refused) => code_of(refused),
                    },
                };
                if code != RoomCode::None {
                    log::info!("{}: create a room: refused ({code:?})", who.name);
                }
                self.send_room_state(who, code, now);
            }
        }
    }

    fn room_create(&mut self, who: &Who, spec: RoomSpec, now: Instant) -> Result<u32, Refused> {
        let entries: Vec<u8> = self
            .room_entries()
            .iter()
            .map(|entry| entry.map_id)
            .collect();
        let default = self.settings.maps.clone();
        // A room with a PIN asks for the one its maker gave just before.
        let pin = match spec.pin {
            false => None,
            true => Some(
                self.rooms
                    .take_pin(who.account_id, now)
                    .ok_or(Refused::Bad)?,
            ),
        };
        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
        let rng = &mut self.rng;
        let key = self.rooms.create(
            who.account_id,
            gear,
            Spec {
                size: spec.size,
                max_level: spec.max_level,
                wait: spec.wait,
                pin,
                min_players: 2 * spec.min_side,
            },
            &entries,
            &default,
            self.settings.rooms.max_rooms,
            now,
            &mut |bound| rng.below(bound as u32) as usize,
        )?;
        let pool = self.rooms.get(key).map_or(0, |room| room.pool.len());
        log::info!(
            "{}: made room {key}: {} v {}, from {} v {}, max level {}, wait {} s, {}PIN, {pool} maps",
            who.name,
            spec.size,
            spec.size,
            spec.min_side,
            spec.min_side,
            spec.max_level,
            spec.wait,
            if spec.pin { "" } else { "no " }
        );
        Ok(key)
    }

    /// A room made by hand: its settings are its maker's create window's
    /// start from now on, kept with the account (the owner's of
    /// 2026-10-20). Not its PIN. A room Play makes keeps nothing.
    fn room_remember(&self, who: &Who, key: u32) {
        let Some(room) = self.rooms.get(key) else {
            return;
        };
        let spec = room.spec;
        let settings = RoomSettings::of_room(
            spec.size,
            spec.min_players / 2,
            spec.max_level,
            spec.wait,
            &room.pool,
            &self.room_entries(),
        );
        let mut account = who.account.locked();
        if account.room_settings.as_ref() != Some(&settings) {
            account.room_settings = Some(settings);
            self.accounts.mark_dirty(who.account_id);
        }
    }

    /// Play for a game whose movie has rooms: into an open room, or a
    /// new one; the lobby state, so the movie shows it. Its own room if it
    /// has one.
    pub(super) fn room_play(&mut self, who: &Who, now: Instant) {
        self.room_quick(who, now);
        self.send(who.addr, lobby::client_state(&ClientState::Lobby, None));
    }

    /// Play's room: the asker's own if it is in one; else an open room
    /// (`room_for_play`), or a new one made for it. What went wrong is
    /// kept for its next state.
    fn room_quick(&mut self, who: &Who, now: Instant) {
        // Play is another choice than waiting: out of any queue.
        self.rooms.dequeue(who.account_id);
        if self.rooms.of(who.account_id).is_none() {
            match self.room_fit(who) {
                Err(code) => {
                    log::info!("{}: Play into a room: refused ({code:?})", who.name);
                    self.room_codes.insert(who.account_id, code);
                }
                Ok(()) => {
                    let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
                    match self.room_for_play(who.account_id, gear) {
                        Some(key) if self.rooms.join(who.account_id, gear, key, now).is_ok() => {
                            log::info!("{} pressed Play: into room {key}", who.name);
                        }
                        _ => {
                            self.rooms.forget_draft(who.account_id);
                            let spec = RoomSpec {
                                size: QUICK_SIZE,
                                max_level: 0,
                                wait: QUICK_WAIT,
                                pin: false,
                                min_side: 1,
                            };
                            match self.room_create(who, spec, now) {
                                Ok(key) => {
                                    log::info!(
                                        "{} pressed Play: no open room, made room {key}",
                                        who.name
                                    );
                                }
                                Err(refused) => {
                                    log::info!("{} pressed Play: no room ({refused:?})", who.name);
                                    self.room_codes.insert(who.account_id, code_of(refused));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// The asker's room as the movie is told it.
    fn room_view(&self, who: &Who, now: Instant) -> Option<RoomView> {
        let room = self.rooms.of(who.account_id)?;
        let tally = room.tally();
        let mine = room.votes.get(&who.account_id).copied();
        let unix = quests::now();
        let members = room
            .members
            .iter()
            .map(|&account_id| {
                let gear = self.accounts.get(account_id).map_or(0, |account| {
                    matchmaking::gear_level(&account.locked(), unix)
                });
                Member {
                    gear,
                    owner: account_id == room.owner,
                    me: account_id == who.account_id,
                    in_match: room.in_match.contains(&account_id),
                    fit: self.member_fit(account_id),
                    away: !self.by_account.contains_key(&account_id),
                    name: self.name_of(account_id).unwrap_or_default(),
                }
            })
            .collect();
        let (phase, secs) = phase_of(room, now);
        let (join_in, join_hold) = match self.room_join_in(who, room, now) {
            Ok(secs) => (secs, None),
            Err(hold) => (None, Some(hold)),
        };
        Some(RoomView {
            key: room.key,
            level: self.room_level(room),
            max_level: room.spec.max_level,
            size: room.spec.size,
            wait: room.spec.wait,
            phase,
            secs,
            owner: room.owner == who.account_id,
            pool: room.pool.len() as u16,
            serial: room.serial,
            pin: room.spec.pin.is_some(),
            cards: room
                .cards
                .iter()
                .enumerate()
                .map(|(index, &map_id)| Card {
                    map_id,
                    votes: tally[index],
                    mine: mine == Some(index as u8),
                })
                .collect(),
            members,
            last_map: room.last_map,
            min_players: room.needs() as u8,
            // The vote is open until the match starts: no map is fixed
            // before (the cards show the votes).
            next_map: 0,
            switch: self.room_switch(who, room),
            join_in,
            join_hold,
        })
    }

    /// Whether a room's match being played takes a member of the room now
    /// (`room_late_join` would put one in), and why not: a maintenance
    /// near; its sides full; or what the match server says (a joiner on its
    /// way in, a game loading, its last 30 s, its end, every place taken).
    /// `Err(None)`: it takes nobody, and nobody says why.
    fn late_hold(
        &self,
        running: &Running,
        size: usize,
        now: Instant,
    ) -> Result<(), Option<JoinHold>> {
        if self.maintenance_is_near() {
            return Err(Some(JoinHold::Maintenance));
        }
        // The side a joiner would go to is full (the room's size), or the
        // match has no place left: the side does not depend on who comes.
        let live = self.host.live(running.ticket.match_id, now);
        let (sides, rule) = self.late_rule(running, live.as_ref(), size);
        let anyone = Worth {
            players: 1,
            strength: 0,
        };
        if policy::place_joiner(sides, anyone, None, &rule).is_none() {
            return Err(Some(JoinHold::Full));
        }
        if self.late_admission(running, now) {
            return Ok(());
        }
        Err(self
            .host
            .closed(running.ticket.match_id, now)
            .map(|closed| match closed {
                Closed::Joining => JoinHold::Joining,
                Closed::Loading => JoinHold::Loading,
                Closed::LastSeconds => JoinHold::LastSeconds,
                Closed::Ending => JoinHold::Ending,
                Closed::Full => JoinHold::Full,
            }))
    }

    /// The member came while its room's match is played: seconds until it
    /// goes into it (the tick puts it in), or why it cannot now (the
    /// owner's of 2026-10-20: players are told). `Ok(None)`: it is in it,
    /// cannot play as it is (its kit or gear: the movie has its own text
    /// for that, from `fit`), or the match takes nobody and nobody says
    /// why.
    fn room_join_in(
        &self,
        who: &Who,
        room: &model::Room,
        now: Instant,
    ) -> Result<Option<u32>, JoinHold> {
        let Stage::InMatch { match_id, .. } = room.stage else {
            return Ok(None);
        };
        if room.in_match.contains(&who.account_id) || self.member_fit(who.account_id) != Fit::Yes {
            return Ok(None);
        }
        let Some(running) = self.matches.get(&match_id) else {
            return Ok(None);
        };
        match self.late_hold(running, usize::from(room.spec.size), now) {
            Ok(()) => {}
            Err(Some(hold)) => return Err(hold),
            Err(None) => return Ok(None),
        }
        let at = self
            .room_join_at
            .get(&who.account_id)
            .copied()
            .unwrap_or(now + LATE_JOIN_WAIT);
        let left = at.saturating_duration_since(now);
        Ok(Some(left.as_millis().div_ceil(1000) as u32))
    }

    /// A room the member may switch to with one click (the owner's of
    /// 2026-10-19): open (no PIN, a place free, it was not turned out of
    /// it), its max level set, lower than its room's (none set counts as
    /// the highest) and not under its equipment; the lowest of them, then
    /// the fullest, then the oldest. None while it plays a match.
    fn room_switch(&self, who: &Who, room: &model::Room) -> Option<RoomSwitch> {
        // Only what a Join would take (no maintenance near, no clan team).
        self.room_fit(who).ok()?;
        let playing = room.in_match.contains(&who.account_id)
            || self
                .player(who.addr)
                .is_some_and(|player| player.in_match.is_some() || player.connect.is_some());
        if playing {
            return None;
        }
        let cap = match room.spec.max_level {
            0 => u8::MAX,
            max => max,
        };
        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
        let best = self
            .rooms
            .open_rooms(who.account_id, gear)
            .filter(|other| {
                other.key != room.key && other.spec.max_level != 0 && other.spec.max_level < cap
            })
            .min_by_key(|other| {
                (
                    other.spec.max_level,
                    std::cmp::Reverse(other.members.len()),
                    other.key,
                )
            })?;
        Some(RoomSwitch {
            key: best.key,
            max_level: best.spec.max_level,
            players: best.members.len() as u8,
            size: best.spec.size,
            name: self.name_of(best.maker).unwrap_or_default(),
        })
    }

    fn send_room_state(&mut self, who: &Who, code: RoomCode, now: Instant) {
        let view = self.room_view(who, now);
        // A code said once: why the last request or Play left it outside.
        let kept = self.room_codes.remove(&who.account_id);
        let code = match (code, kept) {
            (RoomCode::None, Some(kept)) if view.is_none() => kept,
            (code, _) => code,
        };
        let queued = self.rooms.queue_place(who.account_id);
        self.send_made(who.addr, wire::room_state(view.as_ref(), code, queued));
    }

    fn room_list(
        &self,
        who: &Who,
        count: u8,
        now: Instant,
    ) -> Result<lobby::ServerMessage, lobby::WireError> {
        let rows: Vec<RoomRow> = self
            .rooms
            .all()
            .map(|room| {
                let (phase, secs) = phase_of(room, now);
                // Before its match: the card that leads alone, or none (a
                // tie or no votes is drawn at the start, so no map is shown
                // that may not be played).
                let map_id = match room.stage {
                    Stage::InMatch { .. } => room.last_map,
                    _ => room.leading().unwrap_or(0),
                };
                RoomRow {
                    key: room.key,
                    level: self.room_level(room),
                    max_level: room.spec.max_level,
                    size: room.spec.size,
                    players: room.members.len() as u8,
                    phase,
                    secs,
                    map_id,
                    pin: room.spec.pin.is_some(),
                    name: self.name_of(room.maker).unwrap_or_default(),
                }
            })
            .collect();
        let my_key = self.rooms.key_of(who.account_id).unwrap_or(0);
        wire::room_list(&rows, rows.len(), my_key, count)
    }

    /// A room's level: the level its match would have with its members now
    /// (`matchmaking::match_level` of their levels, as a match's is made).
    /// A room has no level of its own: it follows who is in it.
    /// The room Play puts the account into (a member as any other, it stays
    /// until it leaves): of the open rooms that let it in, the one whose
    /// level is nearest the player's, however far (the owner's of
    /// 2026-10-18: too few players online to leave anyone out; only a room
    /// made by hand has a max level); then the fullest, then the oldest.
    fn room_for_play(&self, account_id: u32, gear: u8) -> Option<u32> {
        let mine = i32::from(self.level_of(account_id, quests::now()).min);
        self.rooms
            .open_rooms(account_id, gear)
            .min_by_key(|room| {
                (
                    (i32::from(self.room_level(room)) - mine).abs(),
                    std::cmp::Reverse(room.members.len()),
                    room.key,
                )
            })
            .map(|room| room.key)
    }

    fn room_level(&self, room: &model::Room) -> u8 {
        let unix = quests::now();
        let levels: Vec<u8> = room
            .members
            .iter()
            .map(|&account_id| self.level_of(account_id, unix).min)
            .collect();
        matchmaking::match_level(&levels).min(u16::from(u8::MAX)) as u8
    }

    /// The rooms with a place free take the first of their queues, who
    /// still may come: in the lobby (its game there, in no room, not in a
    /// match), its equipment within the max level. Straight into a match
    /// being played. One who may not is out of the queue.
    fn room_admit_queues(&mut self) {
        // Near a maintenance nobody may come: the queues wait, to be told
        // (code 9) when their rooms are disbanded.
        if self.maintenance_is_near() {
            return;
        }
        for key in self.rooms.with_queue_and_place() {
            while let Some(account_id) = self.rooms.next_in_queue(key) {
                let fits = self.room_fit_of(account_id).is_ok()
                    && self.accounts.get(account_id).is_some_and(|account| {
                        let gear = matchmaking::gear_level(&account.locked(), quests::now());
                        self.rooms.get(key).is_some_and(|room| room.lets_in(gear))
                    });
                if fits && self.rooms.admit(account_id, key) {
                    log::info!("Account {account_id}: from room {key}'s queue into it");
                    continue;
                }
                log::info!("Account {account_id}: out of room {key}'s queue (may not come now)");
            }
        }
    }

    /// A member of a room whose session ended.
    pub(super) fn room_gone(&mut self, account_id: u32) {
        if let Some(key) = self.rooms.leave(account_id) {
            log::info!("Account {account_id} left room {key} with its session");
        }
        if let Some(key) = self.rooms.dequeue(account_id) {
            log::info!("Account {account_id} left room {key}'s queue with its session");
        }
        self.rooms.forget_draft(account_id);
        self.rooms.forget_pin(account_id);
        self.room_codes.remove(&account_id);
    }

    /// Who left its room's match by its own word (its game's 0xE0, which
    /// the match server passes on: `Matchmaking::leave_match`) is out of
    /// the room, as by Leave: it may join again, and while the match is
    /// played it goes into it again as a new place (the owner's of
    /// 2026-10-19).
    fn room_left_matches(&mut self) {
        for (match_id, account_id) in self.store.matchmaking().take_left() {
            let left = self.rooms.of(account_id).is_some_and(|room| {
                matches!(room.stage, Stage::InMatch { match_id: id, .. } if id == match_id)
                    && room.in_match.contains(&account_id)
            });
            if left && let Some(key) = self.rooms.leave(account_id) {
                log::info!(
                    "{}: left match {match_id:#x} by its own word, and with it room {key}",
                    self.name_of(account_id).unwrap_or_default()
                );
            }
        }
    }

    /// A member that went into its room's match late and was let go before
    /// it entered (`Matchmaking::join_failed`: its game never came, left,
    /// or did not catch up) waits in the room again: it is told when it
    /// goes in, and goes in at its next turn. Live, 2026-10-19 (room 2168):
    /// one such member stayed marked in the match and waited out all of it.
    fn room_failed_joins(&mut self) {
        for (match_id, account_id) in self.store.matchmaking().take_failed_joins() {
            let Some(key) = self.rooms.of(account_id).and_then(|room| {
                (matches!(room.stage, Stage::InMatch { match_id: id, .. } if id == match_id)
                    && room.in_match.contains(&account_id))
                .then_some(room.key)
            }) else {
                continue;
            };
            self.rooms.join_failed(key, account_id);
            // Its next turn: `LATE_JOIN_WAIT` from now.
            self.room_join_at.remove(&account_id);
            log::info!(
                "{}: did not get into match {match_id:#x}; it waits in room {key} again",
                self.name_of(account_id).unwrap_or_default()
            );
        }
    }

    /// The room's match is over.
    pub(super) fn room_match_over(&mut self, match_id: u32) {
        self.room_launching.remove(&match_id);
        if let Some(key) = self.rooms.match_over(match_id) {
            log::info!("Room {key}: its match {match_id:#x} is over; back in the room");
            // A maintenance is near: the match was played out, and the room
            // ends with it.
            if self.maintenance_is_near() {
                self.room_disband(key);
            }
        }
    }

    /// The room is gone (a maintenance is near): its members are told so
    /// at their next state request.
    fn room_disband(&mut self, key: u32) {
        let members = self.rooms.disband(key);
        log::info!(
            "Room {key}: disbanded for the maintenance ({} members)",
            members.len()
        );
        for member in members {
            self.room_codes.insert(member, RoomCode::Maintenance);
        }
    }

    /// The member as its room's match takes it: free in the lobby, its
    /// active kit one Play would take, its game of the pool.
    fn room_ready(&self, account_id: u32) -> Option<Ready> {
        let addr = *self.by_account.get(&account_id)?;
        let player = self.player(addr)?;
        if player.in_match.is_some() || player.connect.is_some() || player.queued_since.is_some() {
            return None;
        }
        if self
            .store
            .matchmaking()
            .pool_of_account(account_id)
            .is_some()
        {
            return None;
        }
        let session_id = player.session_id;
        let unix = quests::now();
        if self.member_fit(account_id) != Fit::Yes {
            return None;
        }
        let kit_id = player.account.locked().active_kit;
        let level = self.level_of(account_id, unix);
        Some(Ready {
            player: session::Player {
                session_id,
                account_id,
                profile_id: kit_id,
            },
            strength: self.strength_of(account_id, level),
        })
    }

    /// The rooms' pass of the lobby's tick: matches whose countdown is
    /// over start, and members free in the lobby while their room's match
    /// is played go into it.
    pub(super) fn room_tick(&mut self, now: Instant) {
        // What the site shows of the rooms.
        self.store.set_room_counts(session::RoomCounts {
            rooms: self.rooms.len(),
            in_rooms: self.rooms.all().map(|room| room.members.len()).sum(),
            queued: self.rooms.queued_total(),
        });
        self.room_left_matches();
        self.room_failed_joins();
        if self.rooms.is_empty() {
            return;
        }
        self.room_admit_queues();
        // A maintenance is near: no room plays another match. A room not
        // playing ends now; one playing plays its match out and ends with it
        // (`room_match_over`), the owner's of 2026-10-17.
        if self.maintenance_is_near() {
            let idle: Vec<u32> = self
                .rooms
                .all()
                .filter(|room| !matches!(room.stage, Stage::InMatch { .. }))
                .map(|room| room.key)
                .collect();
            for key in idle {
                self.room_disband(key);
            }
        }
        // Who counts for a start: a member the match would take now (in the
        // lobby, free, a kit that can play, within the max level).
        let counted: HashSet<u32> = if self.maintenance_is_near() {
            HashSet::new()
        } else {
            self.rooms
                .all()
                .filter(|room| !matches!(room.stage, Stage::InMatch { .. }))
                .flat_map(|room| room.members.iter().copied())
                .filter(|&member| self.room_ready(member).is_some())
                .collect()
        };
        let rng = &mut self.rng;
        let due = self.rooms.tick(
            now,
            &mut |bound| rng.below(bound as u32) as usize,
            |member| counted.contains(&member),
        );
        for Due::Start { key, map_id } in due {
            self.room_start(key, map_id, now);
        }
        let waiting: Vec<u32> = self
            .rooms
            .all()
            .filter(|room| matches!(room.stage, Stage::InMatch { .. }))
            .flat_map(|room| {
                room.members
                    .iter()
                    .filter(|member| !room.in_match.contains(member))
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        // Each goes into the match `LATE_JOIN_WAIT` after it came (or at
        // once on its ask): the room shows first (the owner's of
        // 2026-10-19).
        self.room_join_at
            .retain(|account_id, _| waiting.contains(account_id));
        for account_id in waiting {
            let at = *self
                .room_join_at
                .entry(account_id)
                .or_insert(now + LATE_JOIN_WAIT);
            if at <= now {
                self.room_late_join(account_id, now);
            }
        }
    }

    /// Whether the member can play its room's next match as it is now: its
    /// active kit as Play takes it (a weapon and its ammo), and its
    /// equipment level within the room's max level (which holds for its
    /// matches, not only its door: equipment changes in the room).
    pub(super) fn member_fit(&self, account_id: u32) -> Fit {
        let Some(account) = self.accounts.get(account_id) else {
            return Fit::Kit;
        };
        let unix = quests::now();
        let account = account.locked();
        // The kit the match would take (`settle_active_kit`, without
        // changing the account: this is read on every answer and tick).
        let kit_id = if account
            .kit_index(account.active_kit)
            .is_some_and(|index| account.kit_usable(index, unix))
        {
            account.active_kit
        } else {
            account
                .kits
                .first()
                .map_or(account.active_kit, |kit| kit.id)
        };
        if rules::check_play(&account, kit_id, unix).is_err() {
            return Fit::Kit;
        }
        let gear = matchmaking::gear_level(&account, unix);
        if self
            .rooms
            .of(account_id)
            .is_some_and(|room| !room.lets_in(gear))
        {
            return Fit::Gear;
        }
        Fit::Yes
    }

    /// Start the room's match on `map_id`.
    fn room_start(&mut self, key: u32, map_id: u8, now: Instant) {
        let Some(room) = self.rooms.get(key) else {
            return;
        };
        let size = usize::from(room.spec.size);
        let members = room.members.clone();
        let unfit: Vec<(u32, Fit)> = members
            .iter()
            .map(|&member| (member, self.member_fit(member)))
            .filter(|(_, fit)| *fit != Fit::Yes)
            .collect();
        if self.maintenance_is_near() {
            log::info!("Room {key}: no match now, a maintenance is near");
            self.rooms.not_started(key, now);
            return;
        }
        let ready: Vec<Ready> = members
            .iter()
            .filter_map(|&account_id| self.room_ready(account_id))
            .filter(|ready| {
                self.pool_for_sessions([ready.player.session_id]) == Some(QueuePool::GrowingRoster)
            })
            .collect();
        // Squads of the room's members, each a party; any other member alone.
        let mut seen = HashSet::new();
        let mut parties: Vec<(Vec<u32>, i64)> = Vec::new();
        for each in &ready {
            if !seen.insert(each.player.account_id) {
                continue;
            }
            let mates: Vec<u32> = self
                .store
                .squads()
                .of(each.player.account_id)
                .map(|squad| squad.members)
                .unwrap_or_default()
                .into_iter()
                .filter(|mate| {
                    *mate != each.player.account_id
                        && ready.iter().any(|other| other.player.account_id == *mate)
                })
                .collect();
            let mut party = vec![each.player.account_id];
            let mut strength = i64::from(each.strength);
            for mate in mates {
                if seen.insert(mate)
                    && let Some(other) = ready.iter().find(|other| other.player.account_id == mate)
                {
                    party.push(mate);
                    strength += i64::from(other.strength);
                }
            }
            parties.push((party, strength));
        }
        let player_value = i64::from(self.settings.balance.player_value());
        let sides = model::sides(&parties, size, player_value);
        let player_of = |account_id: &u32| {
            ready
                .iter()
                .find(|each| each.player.account_id == *account_id)
                .map(|each| each.player)
        };
        let teams: [Vec<session::Player>; 2] =
            sides.map(|side| side.iter().filter_map(player_of).collect());
        if teams.iter().any(Vec::is_empty) {
            log::info!(
                "Room {key}: no match yet: {} of {} members are free with a kit Play takes",
                ready.len(),
                members.len()
            );
            self.rooms.not_started(key, now);
            return;
        }
        // Each squad of two or more is a squad in the match too.
        let mut squad_ids: std::collections::HashMap<u32, u8> = std::collections::HashMap::new();
        let mut next_squad = 0_u8;
        for (party, _) in parties.iter().filter(|(party, _)| party.len() > 1) {
            next_squad = next_squad.saturating_add(1);
            for &member in party {
                squad_ids.insert(member, next_squad);
            }
        }
        let squad_of = |account_id: u32| squad_ids.get(&account_id).copied().unwrap_or(0);
        let Some(match_id) = self.store.matchmaking().seat_sides_in_pool(
            [&teams[0], &teams[1]],
            now,
            QueuePool::GrowingRoster,
            &squad_of,
        ) else {
            log::warn!("Room {key}: its match could not be seated");
            self.rooms.not_started(key, now);
            return;
        };
        self.room_launching.insert(match_id, map_id);
        // Made at the match server before anyone is told: if it fails, its
        // players are back in the lobby (`launch`) and the room waits again,
        // nobody left marked as searching.
        if !self.launch(match_id) {
            log::warn!("Room {key}: its match could not be made; the countdown runs again");
            self.room_launching.remove(&match_id);
            self.rooms.not_started(key, now);
            return;
        }
        let seated: Vec<u32> = teams
            .iter()
            .flatten()
            .map(|player| player.account_id)
            .collect();
        log::info!(
            "Room {key}: match {match_id:#x} on map {map_id}, {} v {}",
            teams[0].len(),
            teams[1].len()
        );
        for player in teams.iter().flatten() {
            self.room_send_to_match(*player, now);
        }
        // `launch` made the match (it is in `matches` from here on).
        self.rooms.started(key, match_id, map_id, &seated, now);
        // The match started without who could not play it: out of the room,
        // told why (they may come back with a kit that can).
        for (member, fit) in unfit {
            log::info!(
                "Room {key}: account {member} taken out: {}",
                if fit == Fit::Gear {
                    "equipment over the max level"
                } else {
                    "its kit cannot play"
                }
            );
            self.rooms.leave(member);
            self.room_codes.insert(
                member,
                if fit == Fit::Gear {
                    RoomCode::GearTooHigh
                } else {
                    RoomCode::KitUnfit
                },
            );
        }
    }

    /// "Match found" and where it is, for a member seated in its room's
    /// match, as for any Play.
    fn room_send_to_match(&mut self, player: session::Player, now: Instant) {
        if let Some(account) = self.store.account(player.account_id) {
            account.locked().play = Some(player);
        }
        let order_id = self.next_order_id;
        // 0xFFFFFFFF is the client's "not queued".
        self.next_order_id = (self.next_order_id + 1) % 0x7FFF_FFFF + 1;
        let Some(addr) = self.by_account.get(&player.account_id).copied() else {
            return;
        };
        if let Some(state) = self.player_mut(addr) {
            state.queued_since = Some(now);
            state.order_id = order_id;
        }
        self.queue_status(addr, now);
    }

    /// The member goes into its room's match being played, if it is free
    /// and the match admits it now; otherwise it waits for the next one.
    fn room_late_join(&mut self, account_id: u32, now: Instant) {
        let Some(room) = self.rooms.of(account_id) else {
            return;
        };
        let Stage::InMatch { match_id, .. } = room.stage else {
            return;
        };
        let key = room.key;
        let size = usize::from(room.spec.size);
        let Some(running) = self.matches.get(&match_id) else {
            return;
        };
        if !self.late_admission(running, now) || self.maintenance_is_near() {
            return;
        }
        // One that left this match by its own word comes again as a new
        // place, its state fresh; the place it left is gone for good
        // (`room_left_matches`). A place is never taken twice.
        let Some(ready) = self.room_ready(account_id) else {
            return;
        };
        let ticket = running.ticket.match_id;
        let live = self.host.live(ticket, now);
        let (sides, rule) = self.late_rule(running, live.as_ref(), size);
        let worth = Worth {
            players: 1,
            strength: i64::from(ready.strength),
        };
        // One who played this match before (left it, or its game was gone
        // for good) goes back to its side when the sides have as many
        // players: the side of its latest place in the roster, which keeps
        // every place, given up or not.
        let previous = self
            .store
            .matchmaking()
            .formed_match(match_id)
            .and_then(|formed| {
                formed
                    .seats
                    .iter()
                    .rev()
                    .find(|seat| seat.player.account_id == account_id)
                    .map(|seat| seat.team)
            });
        let Some(placement) = policy::place_joiner(sides, worth, previous, &rule) else {
            return;
        };
        let team = placement.assignments.first().map_or(0, |(_, team)| *team);
        let unix = quests::now();
        let Some(profile) = self.accounts.get(account_id).map(|account| {
            let mut account = account.locked();
            account.settle_active_kit(unix);
            MatchPlayer {
                session_id: ready.player.session_id,
                account_id,
                profile: match_profile(&account, team, 0, unix),
                bot: self.accounts.is_bot_name(&account.name),
                driven: false,
            }
        }) else {
            return;
        };
        let first = running.seats.len();
        let host = self.host;
        let admitted =
            self.store
                .matchmaking()
                .append_player(match_id, ready.player, team, |actual, _| {
                    actual == first && host.join_running(ticket, vec![profile])
                });
        if !admitted {
            return;
        }
        self.seated_late(
            match_id,
            &[(ready.player, team, ready.strength)],
            &placement,
            rule.lead,
        );
        self.rooms.seated_late(key, account_id);
        log::info!(
            "Room {key}: {} went into its match {ticket:#x} late, team {team}",
            self.name_of(account_id).unwrap_or_default()
        );
        self.room_send_to_match(ready.player, now);
    }
}

/// The phase and its seconds: left of a countdown, played of a match.
fn phase_of(room: &model::Room, now: Instant) -> (RoomPhase, u32) {
    match room.stage {
        Stage::Waiting => (RoomPhase::Waiting, 0),
        Stage::Vote { until } => (
            RoomPhase::Vote,
            until.saturating_duration_since(now).as_secs() as u32,
        ),
        Stage::InMatch { since, .. } => (
            RoomPhase::InMatch,
            now.saturating_duration_since(since).as_secs() as u32,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_read_as_documented() {
        let config = Config::default();
        assert_eq!(config.max_rooms, 50);
        assert_eq!(parse_max(" 7 "), Some(7));
        assert_eq!(parse_max("0"), None);
        assert_eq!(parse_max("1001"), None);
        assert_eq!(parse_extra(""), Some(vec![]));
        assert_eq!(parse_extra("61, 117"), Some(vec![61, 117]));
        // 107: level_08 night, which has no project.
        assert_eq!(parse_extra("107"), None);
        assert_eq!(parse_extra("x"), None);
    }
}
