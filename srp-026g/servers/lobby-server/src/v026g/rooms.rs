//! Rooms (the players' "servers"): the lobby's side of
//! `session::v026g::rooms`, whose rules are the owner's of 2026-10-16, and
//! of its wire (`survarium::v026g::lobby::rooms`, `lobby.md` §2.16.3).
//! Every PvP match is a room's (the owner's of 2026-10-17: the queue, the
//! "matches now" list and its JOIN went).
//!
//! srp's changed lobby movie asks with the ranking page's request at the
//! rooms' reserved starts, as "Player vs Bots" does: the
//! list of the rooms, the asker's own room, the maps a pool may hold, and
//! the actions (create, join, leave, vote, kick, start). Every action is answered
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
//!   as for any Play. Its rewards are any match's. A private room (a PIN)
//!   counts nothing down: its owner's Start makes the match at once with
//!   whoever is in, 1 v 0 too (`room_start_pressed`; the owner's of
//!   2026-10-26).
//! - **Late.** A member in the lobby while the room's match is played sees
//!   the room first and goes into it `LATE_JOIN_WAIT` later (or at once on
//!   its ask) if the match admits players (from its countdown until
//!   `LOAD_MARGIN` is left), on the side with fewer players in it; of two
//!   alike in that, the side it played on in that match before, then the
//!   one that counts for less (`matchmaking_policy::place_joiner`);
//!   otherwise it waits for the next match. While it waits it is told why it
//!   cannot go in now (`late_hold`: a joiner on its way, a game loading,
//!   the last 30 s, the end, full, a maintenance; the owner's of 2026-10-20).
//!   A member whose place in the match is kept (its game started anew, or
//!   back from it) goes back to it the same way, as Play does, whatever
//!   the match takes now (the owner's of 2026-11-01).
//! - **After.** The match over, its players are in the lobby again and in
//!   the room, whose next vote and wait begin: every member alike, however
//!   it came in (Play or a join; the owner's of 2026-10-31).
//! - **Play.** Public rooms start empty and grow with demand: the first
//!   Play creates an 8 v 8 room in its equipment bracket (up to 10 or 15).
//!   Full rooms queue newcomers; six ready Play waiters in the same bracket
//!   open another room. Explicit room queues stay chosen.
//!   Only private rooms may be created by a client.
//! - **Squads** (two at most; the owner's of 2026-10-31,
//!   `docs/notes/squads.md`): Play, Join and a queue take the mate along
//!   when it is free in the lobby ([`Core::room_mate`]), into the same room
//!   or queue, or neither moves; Play of one whose mate is in a room goes
//!   into that room (its queue if full). A queue lets a pair in at two
//!   places. The sides keep a squad together; late, the second goes on its
//!   mate's side. The room's lists mark each pair (`Member::squad`).
//! - **Remembered.** A room made by hand (a create, not Play) is its maker's
//!   next create window, kept with the account (`room_remember`; not the
//!   PIN; the owner's of 2026-10-20); the movie asks for it fitted to the
//!   pool and the maker's equipment now (`room_settings::form`) once, as
//!   its lobby comes up, so the window opens filled (the owner's of
//!   2026-10-25).
//! - A member whose game is gone (crashed, killed, its link silent) keeps
//!   its place while its session is kept: the link's 30 s and the session's
//!   60 s, about 90 s in all (the owner's of 2026-10-18); then it leaves the
//!   room and its queue, and may join again as a new member. Its game
//!   started anew within that keeps the place; a game quit from its menu
//!   (`FA`) leaves the room at once.
//! - A game started anew (a new session) is not seated in a match, nor
//!   sent "match found", until it has been told its room (`room_unseen`:
//!   its movie asks the state, Play, Join match); one not told within
//!   about 90 s loses the place. Live, 2026-10-20: a game that did not know
//!   it was in a room was pulled into its matches.
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

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use session::matchmaking_policy::{self as policy, Closed, Worth};
use session::v026g::presets::{self, Offer};
use session::v026g::room_settings::{self, RoomSettings};
use session::v026g::rooms::{self as model, Door, Due, QueueChoice, Refused, Spec, Stage, Waiting};
use session::v026g::{matchmaking, quests, rules};
use session::{LockExt, QueuePool, Strength};
use survarium::v026g::lobby::presets::PresetDone;
use survarium::v026g::lobby::rooms::{
    self as wire, Card, Fit, FriendRoom, JoinHold, ListMember, Member, OfferKind, OfferRoom,
    PoolEntry, RoomCode, RoomOffer, RoomPhase, RoomRequest, RoomRow, RoomSpec, RoomSwitch,
    RoomView,
};
use survarium::v026g::lobby::{self, ClientState};
use survarium::v026g::maps;

use super::MAINTENANCE_WINDOW;
use super::core::{Core, Who};
use super::play::{MatchPlayer, Running, match_profile};

/// `SRP_ROOMS_MAX` when unset.
pub const DEFAULT_MAX_ROOMS: usize = 50;
/// What a room Play makes is made with (its bracket's cap, the standard pool):
/// 8 a side, the owner's word of 2026-10-18 ("8vs8 not 10vs10").
const QUICK_SIZE: u8 = 8;
const QUICK_WAIT: u8 = 30;
/// Six ready Play users waiting in one equipment bracket open another room.
const SCALE_PLAYERS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PublicBracket {
    UpTo10,
    UpTo15,
}

impl PublicBracket {
    fn of(gear: u8) -> Self {
        if gear <= 10 {
            Self::UpTo10
        } else {
            Self::UpTo15
        }
    }

    fn max_level(self) -> u8 {
        match self {
            Self::UpTo10 => 10,
            Self::UpTo15 => 15,
        }
    }

    fn other(self) -> Self {
        match self {
            Self::UpTo10 => Self::UpTo15,
            Self::UpTo15 => Self::UpTo10,
        }
    }

    fn contains(self, room: &model::Room) -> bool {
        room.is_open() && room.max_level() == self.max_level()
    }
}
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
            extra_maps.len(),
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

/// A free member of a room, ready for its match, and the gear level it
/// would bring (`matchmaking::gear_level`): a match's equipment level is
/// the highest of its players' (`docs/notes/kd-balance.md`).
struct Ready {
    player: session::Player,
    gear: u8,
}

impl Core<'_> {
    /// The maps a pool may hold, in the order their bits name them: the
    /// match pool, then the experimental maps, then
    /// Blackout's: each plain battery map of those once more, as mode 6 by
    /// its lobby id (its tile the same map and weather).
    pub(super) fn room_entries(&self) -> Vec<PoolEntry> {
        let pool = &self.settings.maps;
        let extra = self
            .settings
            .rooms
            .extra_maps
            .iter()
            .filter(|id| !pool.contains(id));
        let entries = pool
            .iter()
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
            .collect::<Vec<_>>();
        let blackout: Vec<PoolEntry> = entries
            .iter()
            .filter(|entry| entry.mode == maps::BLACKOUT_RULES)
            .filter_map(|entry| {
                Some(PoolEntry {
                    map_id: maps::blackout_id(entry.map_id)?,
                    mode: maps::mode::BLACKOUT,
                    ..*entry
                })
            })
            .collect();
        entries
            .into_iter()
            .chain(blackout)
            // A create names its entries by bit: no more than the bits.
            .take(wire::POOL_BITS)
            .collect()
    }

    /// The mate a squad's action takes along (the owner's of 2026-10-31:
    /// only its commander acts for the squad: Play, Join, Queue, Switch),
    /// with its equipment level: the commander's mate while it is free in
    /// the lobby as [`Self::room_fit_of`] says (in a room too, while it plays
    /// no match), its game told its room, its kit able to play, and waiting
    /// in no queue but its commander's. `None` for anyone but a commander.
    /// The one rule of who comes along: Play, Join, Queue, the queue's turn,
    /// the scale-out and the late join all ask it.
    pub(super) fn room_mate(&self, account_id: u32) -> Option<(u32, u8)> {
        let squad = self.store.squads().of(account_id)?;
        if squad.commander() != account_id {
            return None;
        }
        let mate = squad.mate_of(account_id)?;
        if self.room_unseen.contains_key(&mate)
            || self.room_fit_of(mate).is_err()
            || self
                .rooms
                .waiting_of(mate)
                .is_some_and(|waiting| waiting.lead() != account_id)
        {
            return None;
        }
        let account = self.accounts.get(mate)?;
        let account = account.locked();
        let unix = quests::now();
        rules::kits_can_play(&account, unix)
            .then(|| (mate, matchmaking::gear_level(&account, unix)))
    }

    /// A squad member that is not its commander: it goes into no room on
    /// its own (no Play, Join, Queue, room made or switch; code 17), only
    /// with its commander's (the owner's of 2026-10-31). Leaving its room,
    /// its queue or the squad stays its own.
    pub(super) fn squad_soldier(&self, account_id: u32) -> bool {
        self.store
            .squads()
            .of(account_id)
            .is_some_and(|squad| squad.commander() != account_id)
    }

    /// The mate a commander's Play takes ([`Self::room_mate`]), and the
    /// equipment level Play picks a room by: the higher of the two, so the
    /// room's max level lets both in.
    fn play_party(&self, account_id: u32, gear: u8) -> (Option<(u32, u8)>, u8) {
        let mate = self.room_mate(account_id);
        let party_gear = mate.map_or(gear, |(_, mate_gear)| gear.max(mate_gear));
        (mate, party_gear)
    }

    /// The squad info's "ready" for a member (`social.md` §2.2, the game's
    /// Play gating): the commander always (its Play is the squad's: the
    /// game greys a soldier's Play while its commander is ready); a
    /// soldier while its commander's actions take it ([`Self::room_mate`]).
    pub(super) fn squad_ready(&self, account_id: u32) -> bool {
        let Some(squad) = self.store.squads().of(account_id) else {
            return false;
        };
        let commander = squad.commander();
        commander == account_id
            || self
                .room_mate(commander)
                .is_some_and(|(mate, _)| mate == account_id)
    }

    /// The commander and its mate (`room_mate`: the one a Join with places
    /// would take, from another room too) wait in room `key`'s queue as one
    /// entry, its place. The place, or why not.
    fn room_enqueue_party(
        &mut self,
        who: &Who,
        gear: u8,
        mate: Option<(u32, u8)>,
        key: u32,
        choice: QueueChoice,
        now: Instant,
    ) -> Result<usize, Refused> {
        self.rooms
            .enqueue_party(who.account_id, gear, mate, key, now, choice)
    }

    /// Whether `who` may make or join a room now.
    /// A squad's soldier may not: its commander acts for it.
    fn room_fit(&self, who: &Who) -> Result<(), RoomCode> {
        if self.squad_soldier(who.account_id) {
            return Err(RoomCode::SquadLeader);
        }
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

    /// [`room_fit`], and the kit a match would take can play
    /// ([`rules::kits_can_play`]): Play, Join (a PIN's too), a queue and a
    /// friend's room refuse it at the door (the owner's of 2026-10-23); the
    /// movie greys them by the same rule. Making a room by hand does not ask
    /// it (the member check warns, `member_fit`).
    fn room_enter_fit(&self, who: &Who) -> Result<(), RoomCode> {
        self.room_fit(who)?;
        if !rules::kits_can_play(&who.account.locked(), quests::now()) {
            return Err(RoomCode::KitCantPlay);
        }
        Ok(())
    }

    /// The maintenance rule ([`MAINTENANCE_WINDOW`]): no room made, no
    /// match started, no entry; the presets list's reasons say it too.
    pub(super) fn maintenance_is_near(&self) -> bool {
        self.store
            .minutes_to_maintenance()
            .is_some_and(|minutes| minutes <= MAINTENANCE_WINDOW)
    }

    /// A room request (`3B` at a rooms' start).
    pub(super) fn room_request(&mut self, who: &Who, request: RoomRequest, now: Instant) {
        // Every request but the list's, the create window's steps, a room's
        // members and the friends' rooms is answered with the asker's room:
        // its game knows it from then on.
        if !matches!(
            request,
            RoomRequest::List { .. }
                | RoomRequest::Pool { .. }
                | RoomRequest::Form
                | RoomRequest::Bits { .. }
                | RoomRequest::Pin { .. }
                | RoomRequest::Members { .. }
                | RoomRequest::Friends
                | RoomRequest::Offer { .. }
        ) {
            self.room_shown(who.account_id);
        }
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
            // The movie put up its "Room Chat" tab: the room's last lines,
            // and its new ones from now on (`room_chat`).
            RoomRequest::Chat => {
                self.room_chat_listen(who);
                self.send_room_state(who, RoomCode::None, now);
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
            // A friend's right-click menu: which friends' rooms it may offer.
            RoomRequest::Friends => {
                let friends = self.friend_rooms(who);
                self.send_made(who.addr, wire::friend_rooms(&friends));
            }
            RoomRequest::Bits { chunk, bits } => {
                let entries = self.rooms.draft(who.account_id, chunk, bits, now);
                self.send_made(who.addr, wire::bits_answer(chunk, entries));
            }
            RoomRequest::Pin { pin } => {
                self.rooms.give_pin(who.account_id, pin, now);
                self.send_made(who.addr, wire::pin_answer());
            }
            RoomRequest::Play => self.room_play_pressed(who, now),
            RoomRequest::Start => {
                self.room_start_pressed(who, now);
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Move { place, check } => {
                // A move made from an older list does nothing, as a kick's.
                let fresh = self
                    .rooms
                    .of(who.account_id)
                    .is_some_and(|room| room.serial % wire::KICK_CHECKS == check);
                let moved = if fresh {
                    self.rooms.move_member(who.account_id, usize::from(place))
                } else {
                    Err(model::MoveRefused::NoMember)
                };
                match moved {
                    Ok(side) => log::info!(
                        "{}: moved the member at place {place} of its room to team {side}",
                        who.name
                    ),
                    Err(refused) => {
                        log::info!("{}: move in a room: refused ({refused:?})", who.name)
                    }
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
            RoomRequest::Join { key } => self.room_join(who, key, now),
            RoomRequest::Offer { key } => self.room_offer(who, key),
            RoomRequest::PresetJoin { preset, key } => self.room_preset_join(who, preset, key, now),
            RoomRequest::Queue { key } => self.room_queue(who, key, now),
            RoomRequest::JoinMatch => {
                // The member does not wait: into its room's match now.
                self.room_join_at.insert(who.account_id, now);
                self.room_late_join(who.account_id, now);
                self.send_room_state(who, RoomCode::None, now);
            }
            // Who is in a room the asker looks at in the list: anyone may
            // ask, of any room (a PIN keeps the door, not the names).
            RoomRequest::Members { key } => {
                let members = self.room_members(who, key);
                self.send_made(who.addr, wire::room_members(key, &members));
            }
            // A commander takes its pair's place out whole; its mate leaves
            // it alone (`Rooms::dequeue`).
            RoomRequest::Unqueue => {
                if let Some(key) = self.rooms.dequeue(who.account_id) {
                    log::info!("{}: left room {key}'s queue", who.name);
                }
                self.send_room_state(who, RoomCode::None, now);
            }
            RoomRequest::Create(spec) => {
                let code = match self.room_fit(who).and(if spec.pin {
                    Ok(())
                } else {
                    Err(RoomCode::PublicCreate)
                }) {
                    Err(code) => code,
                    Ok(()) => match self.room_create(who, spec, now) {
                        Ok(key) => {
                            self.room_remember(who, key, spec);
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

    /// The movie's Play with rooms on: straight into a room, no search
    /// shown; the state answers it.
    fn room_play_pressed(&mut self, who: &Who, now: Instant) {
        // A place in a match (closed early, or signed in again): back to it,
        // as the game's own Play does (`play`).
        if self.back_to_seat(who.addr, now) {
            log::info!("{} pressed Play: back to its seat", who.name);
        } else {
            self.room_quick(who, now);
        }
        self.send_room_state(who, RoomCode::None, now);
    }

    /// The owner's Start in its private room (the owner's of 2026-10-26):
    /// its match now, on the vote's winner, with whoever can play
    /// (`room_start`; one is enough). Anyone else's, an open room's, or one
    /// while its match is played does nothing; nor while a maintenance is
    /// near (`room_start`).
    fn room_start_pressed(&mut self, who: &Who, now: Instant) {
        let rng = &mut self.rng;
        match self.rooms.start(who.account_id, &mut |bound| {
            rng.below(bound as u32) as usize
        }) {
            Ok(Due::Start { key, map_id }) => {
                log::info!("{}: started its room {key}'s match", who.name);
                self.room_start(key, map_id, now);
            }
            Err(refused) => log::info!("{}: Start in a room: refused ({refused:?})", who.name),
        }
    }

    /// A full room's queue (the owner's of 2026-10-17): from the lobby
    /// only, one at a time; a room with a place is joined at once
    /// ([`Self::room_join`]). A squad mate free in the lobby waits with it
    /// (`room_mate`). The state answers it.
    fn room_queue(&mut self, who: &Who, key: u32, now: Instant) {
        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
        let mate = self.room_mate(who.account_id);
        let free = self.rooms.get(key).is_some_and(|room| {
            let comes = mate.is_some_and(|(mate, _)| !room.members.contains(&mate));
            let beside = self
                .store
                .squads()
                .mate_of(who.account_id)
                .filter(|mate| room.members.contains(mate));
            room.has_places(1 + usize::from(comes), beside)
        });
        if free && self.room_enter_fit(who).is_ok() {
            // Places for all: a join (`room_join` answers it).
            self.room_join(who, key, now);
            return;
        }
        let code = match self.room_enter_fit(who) {
            Err(code) => code,
            Ok(()) => {
                let before = self.rooms.queue_place(who.account_id);
                match self.room_enqueue_party(who, gear, mate, key, QueueChoice::Chosen, now) {
                    Ok(place) => {
                        // Queue stays lit while queued: a press again
                        // is logged only when the place moved.
                        if before != Some((key, place)) {
                            log::info!("{}: waits in room {key}'s queue, place {place}", who.name);
                        }
                        RoomCode::None
                    }
                    Err(refused) => code_of(refused),
                }
            }
        };
        self.send_room_state(who, code, now);
    }

    /// Join the room with that key, a commander's mate along (`room_mate`);
    /// a commander in no room whose squad finds too few places on one side
    /// (two for the pair, or one on its mate's team with the mate in it)
    /// waits in its queue, with its mate. The state answers it.
    fn room_join(&mut self, who: &Who, key: u32, now: Instant) {
        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
        let mate = self.room_mate(who.account_id);
        let code = match self.room_enter_fit(who) {
            Err(code) => code,
            Ok(()) => match self.rooms.join_with(who.account_id, gear, mate, key, now) {
                Ok(()) => {
                    log::info!("{}: joined room {key}", who.name);
                    RoomCode::None
                }
                Err(Refused::Full)
                    if self.rooms.key_of(who.account_id).is_none()
                        && self.store.squads().of(who.account_id).is_some() =>
                {
                    match self.room_enqueue_party(who, gear, mate, key, QueueChoice::Chosen, now) {
                        Ok(place) => {
                            log::info!(
                                "{}: room {key} has no places for its squad: both wait in its \
                                 queue, place {place}",
                                who.name
                            );
                            RoomCode::None
                        }
                        Err(refused) => code_of(refused),
                    }
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

    /// Whether joining the room with `key` (0: the room Play would take
    /// now) offers one of the asker's kit presets (`presets.md`): the one
    /// with the highest gear level the room's max level allows. Nothing for
    /// a room of any max level or a private one (unless the kits cannot
    /// play: `presets::offer`), for no room, or while the asker may join
    /// none (the join then says why). Play that would queue is offered,
    /// instead, the Join offer of a room of the other bracket with a place
    /// ([`Self::room_instead_of_queue`], the owner's of 2026-10-30).
    fn room_offer(&mut self, who: &Who, key: u32) {
        let unix = quests::now();
        let (gear, levels, kits_play, active) = {
            let account = who.account.locked();
            (
                matchmaking::gear_level(&account, unix),
                presets::levels(&account, unix),
                rules::kits_can_play(&account, unix),
                account.active_preset,
            )
        };
        // Play picks a room for the squad's gear and places (`play_party`).
        let (mate, party_gear) = self.play_party(who.account_id, gear);
        let places = 1 + usize::from(mate.is_some());
        let (target, about) = match key {
            0 if self.rooms.of(who.account_id).is_none() => {
                match self.room_for_play(who.account_id, party_gear, places) {
                    Some(key) => (Some(key), OfferRoom::Entered),
                    // A pair's queue offers no room of the other bracket:
                    // a preset changes the asker's gear, not its mate's.
                    None if mate.is_some() => (None, OfferRoom::Entered),
                    None => match self.room_instead_of_queue(
                        who.account_id,
                        gear,
                        &levels,
                        kits_play,
                        active,
                    ) {
                        Some(key) => (Some(key), OfferRoom::InsteadOfQueue),
                        None => (None, OfferRoom::Entered),
                    },
                }
            }
            0 => (None, OfferRoom::Entered),
            key => (Some(key), OfferRoom::Entered),
        };
        let room = match self.room_fit(who) {
            Ok(()) => target.and_then(|key| self.rooms.get(key)),
            Err(_) => None,
        };
        // A join refused at the door (the join then says why) is about no
        // room Play goes to instead.
        let about = room.map_or(OfferRoom::Entered, |_| about);
        let max_level = room.map_or(0, model::Room::max_level);
        let offered = match room {
            Some(_) => presets::offer(gear, max_level, &levels, kits_play, active),
            None => Offer::None,
        };
        let (kind, preset, level) = match offered {
            Offer::None => (OfferKind::None, 0, 0),
            Offer::Higher { preset, level } => (OfferKind::Higher, preset, level),
            Offer::Fits { preset, level } => (OfferKind::Fits, preset, level),
            Offer::Unfit { preset, level } => (OfferKind::Unfit, preset, level),
        };
        if kind != OfferKind::None {
            log::info!(
                "{}: room {} (max level {max_level}) offers preset {preset} (level {level}; gear {gear}){}",
                who.name,
                target.unwrap_or(0),
                match about {
                    OfferRoom::Entered => "",
                    OfferRoom::InsteadOfQueue => ", instead of Play's queue",
                }
            );
        }
        let answer = wire::room_offer(
            key,
            &RoomOffer {
                key: target.unwrap_or(0),
                kind,
                preset,
                level,
                gear,
                max_level,
                about,
            },
        );
        self.send_made(who.addr, answer);
    }

    /// "Switch and enter": preset `preset` the active one (its kits put on),
    /// then the room with `key` as its Queue takes it (a join with places,
    /// else its queue: a Join's or a full room's Queue's offer), or Play
    /// (`key` 0). A preset that could not be chosen joins nothing (code 14).
    fn room_preset_join(&mut self, who: &Who, preset: u8, key: u32, now: Instant) {
        // A soldier's join is its commander's: its loadout stays.
        if self.squad_soldier(who.account_id) {
            self.send_room_state(who, RoomCode::SquadLeader, now);
            return;
        }
        match self.preset_choose(who, preset) {
            Ok(_) => {
                self.send_presets(who, PresetDone::Chosen, preset);
                if key == 0 {
                    self.room_play_pressed(who, now);
                } else {
                    self.room_queue(who, key, now);
                }
            }
            Err(_) => self.send_room_state(who, RoomCode::PresetRefused, now),
        }
    }

    fn room_create(&mut self, who: &Who, spec: RoomSpec, now: Instant) -> Result<u32, Refused> {
        let entries: Vec<u8> = self
            .room_entries()
            .iter()
            .map(|entry| entry.map_id)
            .collect();
        // The whole pool but the experimental maps: the match pool and
        // its Blackout twins (the owner's of 2026-10-25: Blackout
        // is part of the standard pool).
        let default: Vec<u8> = self
            .room_entries()
            .iter()
            .filter(|entry| !entry.extra)
            .map(|entry| entry.map_id)
            .collect();
        // A room with a PIN asks for the one its maker gave just before; it
        // has no max level (the owner's of 2026-10-26), the window's is
        // only kept for the next open room (`room_remember`).
        let door = match spec.pin {
            false => Door::Open {
                max_level: spec.max_level,
            },
            true => Door::Pin(
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
                wait: spec.wait,
                door,
                min_players: 2 * spec.min_side,
            },
            &entries,
            &default,
            self.settings.rooms.max_rooms,
            now,
            &mut |bound| rng.below(bound as u32) as usize,
        )?;
        let pool = self.rooms.get(key).map_or(0, |room| room.pool.len());
        if spec.pin {
            log::info!(
                "{}: made room {key}: private (PIN), {} v {}, started by its owner, any \
                 equipment, {pool} maps",
                who.name,
                model::PRIVATE_SIZE,
                model::PRIVATE_SIZE,
            );
        } else {
            log::info!(
                "{}: made room {key}: {} v {}, from {} v {}, max level {}, wait {} s, no PIN, \
                 {pool} maps",
                who.name,
                spec.size,
                spec.size,
                spec.min_side,
                spec.min_side,
                spec.max_level,
                spec.wait
            );
        }
        // A commander's mate comes in with it (`room_mate`), no PIN typed:
        // a room made by hand, by Play or by the scale-out alike (the
        // owner's of 2026-10-31).
        if let Some(mate) = self.room_mate(who.account_id)
            && let Err(refused) = self.rooms.bring_mate(who.account_id, mate, key)
        {
            log::info!(
                "{}: its squad mate stays out of room {key} ({refused:?})",
                who.name
            );
        }
        Ok(key)
    }

    /// A room made by hand: its settings are its maker's create window's
    /// start from now on, kept with the account (the owner's of
    /// 2026-10-20). Not its PIN. A room Play makes keeps nothing. The
    /// settings as the window sent them (`spec`): a private room's size and
    /// players to start are not its own (it has none), but the window's,
    /// kept for its next open room.
    fn room_remember(&self, who: &Who, key: u32, spec: RoomSpec) {
        let Some(room) = self.rooms.get(key) else {
            return;
        };
        let settings = RoomSettings::of_room(
            spec.size,
            spec.min_side,
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
        self.room_shown(who.account_id);
        self.room_quick(who, now);
        self.send(who.addr, lobby::client_state(&ClientState::Lobby, None));
    }

    /// Play stays in its equipment bracket. With no room it creates the
    /// first one; full rooms queue new players until six are ready together.
    /// A squad goes together: into the mate's room when it is in one (its
    /// queue when that is full), else the mate free in the lobby comes along
    /// (`play_party`): a room with places for both, by the higher gear of
    /// the two, or the queue together, or a new room.
    fn room_quick(&mut self, who: &Who, now: Instant) {
        if self.rooms.of(who.account_id).is_some() {
            return;
        }
        if let Err(code) = self.room_enter_fit(who) {
            self.room_codes.insert(who.account_id, code);
            return;
        }
        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
        if self.room_quick_to_mate(who, gear, now) {
            self.room_codes.remove(&who.account_id);
            return;
        }
        let (mate, party_gear) = self.play_party(who.account_id, gear);
        let places = 1 + usize::from(mate.is_some());
        let result = if let Some(key) = self.room_for_play(who.account_id, party_gear, places) {
            self.rooms.join_with(who.account_id, gear, mate, key, now)
        } else if let Some(key) = self.play_queue(who.account_id, party_gear) {
            self.room_enqueue_party(who, gear, mate, key, QueueChoice::Play, now)
                .map(|_| ())
        } else {
            // Its mate comes in with it (`room_create`).
            self.room_create_public(who, PublicBracket::of(party_gear), now)
                .map(|_| ())
        };
        match result {
            Ok(()) => {
                self.room_codes.remove(&who.account_id);
            }
            Err(refused) => {
                self.room_codes.insert(who.account_id, code_of(refused));
            }
        }
    }

    /// Play of one whose squad mate is in a room: into that room beside it,
    /// or its queue when the room is full. False when that room does not
    /// take it (its PIN, its max level, turned out of it): Play goes on as
    /// for anyone.
    fn room_quick_to_mate(&mut self, who: &Who, gear: u8, now: Instant) -> bool {
        let Some(mate) = self.store.squads().mate_of(who.account_id) else {
            return false;
        };
        let Some(key) = self.rooms.key_of(mate) else {
            return false;
        };
        match self
            .rooms
            .join_with(who.account_id, gear, Some((mate, gear)), key, now)
        {
            Ok(()) => {
                log::info!("{}: Play: into its squad mate's room {key}", who.name);
                true
            }
            Err(Refused::Full) => match self.rooms.enqueue(who.account_id, gear, key, now) {
                Ok(place) => {
                    log::info!(
                        "{}: Play: its squad mate's room {key} is full; waits in its queue, \
                         place {place}",
                        who.name
                    );
                    true
                }
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    /// The queue Play puts the account in when no room of its bracket takes
    /// it now: the one it waits in, if of its bracket, else a shortest.
    /// None: Play makes the bracket's first room.
    fn play_queue(&self, account_id: u32, gear: u8) -> Option<u32> {
        self.rooms
            .queue_place(account_id)
            .map(|(key, _)| key)
            .filter(|key| {
                self.rooms
                    .get(*key)
                    .is_some_and(|room| PublicBracket::of(gear).contains(room))
            })
            .or_else(|| self.room_queue_for_play(gear))
    }

    /// For Play that would queue (no room of its bracket takes it now; the
    /// owner's of 2026-10-30, "exactly the same recommendation when I join a
    /// room with lower/higher gear"): the room of the other bracket with a
    /// place whose Join would offer a preset (`presets::offer`, Higher or
    /// Fits) of that bracket, so that "Switch and enter" (the preset chosen,
    /// then Play by the new gear) goes there; the room Play would take with
    /// that gear. None when Play makes a room, or when Join would offer
    /// nothing (a gear-10 player with no higher preset still queues: Play
    /// keeps to its bracket).
    fn room_instead_of_queue(
        &self,
        account_id: u32,
        gear: u8,
        levels: &[Option<u8>; presets::PRESETS],
        kits_play: bool,
        active: u8,
    ) -> Option<u32> {
        self.play_queue(account_id, gear)?;
        let other = PublicBracket::of(gear).other();
        let level = match presets::offer(gear, other.max_level(), levels, kits_play, active) {
            Offer::Higher { level, .. } | Offer::Fits { level, .. } => level,
            Offer::None | Offer::Unfit { .. } => return None,
        };
        if PublicBracket::of(level) != other {
            return None;
        }
        self.room_in_bracket(account_id, level, other, 1)
    }

    /// Queue in the same bracket. Counts for expansion are across all
    /// these queues, not per room.
    fn room_queue_for_play(&self, gear: u8) -> Option<u32> {
        self.rooms
            .all()
            .filter(|room| PublicBracket::of(gear).contains(room))
            .min_by_key(|room| (self.rooms.queue_len(room.key), room.key))
            .map(|room| room.key)
    }

    fn room_create_public(
        &mut self,
        who: &Who,
        bracket: PublicBracket,
        now: Instant,
    ) -> Result<u32, Refused> {
        // A previous visit to the private-room form must not change Play's
        // map pool or remembered settings.
        self.rooms.forget_draft(who.account_id);
        let key = self.room_create(
            who,
            RoomSpec {
                size: QUICK_SIZE,
                max_level: bracket.max_level(),
                wait: QUICK_WAIT,
                pin: false,
                min_side: 1,
            },
            now,
        )?;
        Ok(key)
    }

    /// Only connected, ready Play users count toward demand. A changed
    /// loadout is routed again before counting, and explicit queues stay put.
    fn room_scale_queues(&mut self, now: Instant) {
        if self.maintenance_is_near() {
            return;
        }
        for (waiting, key) in self.rooms.play_queue() {
            let account_id = waiting.lead();
            let Some(who) = self.who_is(account_id) else {
                continue;
            };
            if self.room_unseen.contains_key(&account_id) || self.room_enter_fit(&who).is_err() {
                continue;
            }
            let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
            let (mate, gear) = self.play_party(account_id, gear);
            let same_bracket = self
                .rooms
                .get(key)
                .is_some_and(|room| PublicBracket::of(gear).contains(room));
            let places = 1 + usize::from(mate.is_some());
            if !same_bracket || self.room_for_play(account_id, gear, places).is_some() {
                self.room_quick(&who, now);
            }
        }
        // Routing changed equipment can put players into another room's
        // queue. Fill its free places before counting remaining demand.
        self.room_admit_queues(now);
        for bracket in [PublicBracket::UpTo10, PublicBracket::UpTo15] {
            loop {
                // Each entry's lead (one alone, or a commander with the mate
                // its Play takes, `play_party`), by the pair's gear.
                let waiting: Vec<_> = self
                    .rooms
                    .play_queue()
                    .into_iter()
                    .filter_map(|(waiting, _)| {
                        let account_id = waiting.lead();
                        let who = self.who_is(account_id)?;
                        if self.room_unseen.contains_key(&account_id)
                            || self.room_enter_fit(&who).is_err()
                        {
                            return None;
                        }
                        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
                        let (mate, party_gear) = self.play_party(account_id, gear);
                        (PublicBracket::of(party_gear) == bracket).then_some((who, gear, mate))
                    })
                    .collect();
                let players: usize = waiting
                    .iter()
                    .map(|(_, _, mate)| 1 + usize::from(mate.is_some()))
                    .sum();
                if players < SCALE_PLAYERS {
                    break;
                }
                let Ok(key) = self.room_create_public(&waiting[0].0, bracket, now) else {
                    // Capacity or map configuration failure: keep everyone
                    // queued, including the player who would make the room.
                    break;
                };
                // In the queue's order, a pair together or neither (one place
                // left for it: both stay queued); its maker's mate came with
                // it (`room_create`).
                for (who, gear, mate) in waiting.iter().skip(1) {
                    let _ = self.rooms.join_with(who.account_id, *gear, *mate, key, now);
                }
                log::info!(
                    "Room {key}: opened for queued Play users, max level {}",
                    bracket.max_level()
                );
            }
        }
    }

    /// A member of a room as the asker is told it.
    /// Each squad of `room` whose two members are both in it, numbered 1,
    /// 2, ... in the list's order: its mark (`Member::squad`).
    fn room_squad_marks(&self, room: &model::Room) -> HashMap<u32, u8> {
        let squads = self.store.squads();
        let mut marks = HashMap::new();
        let mut next = 0_u8;
        for &member in &room.members {
            if marks.contains_key(&member) {
                continue;
            }
            if let Some(mate) = squads
                .mate_of(member)
                .filter(|mate| room.members.contains(mate))
            {
                next = next.saturating_add(1).min(wire::SQUAD_MARKS);
                marks.insert(member, next);
                marks.insert(mate, next);
            }
        }
        marks
    }

    fn room_member(
        &self,
        who: &Who,
        room: &model::Room,
        account_id: u32,
        marks: &HashMap<u32, u8>,
        unix: u64,
    ) -> Member {
        let gear = self.accounts.get(account_id).map_or(0, |account| {
            matchmaking::gear_level(&account.locked(), unix)
        });
        Member {
            gear,
            owner: account_id == room.owner,
            me: account_id == who.account_id,
            in_match: room.in_match.contains(&account_id),
            fit: self.member_fit(account_id),
            away: !self.by_account.contains_key(&account_id)
                || self.room_unseen.contains_key(&account_id),
            squad: marks.get(&account_id).copied(),
            name: self.name_of(account_id).unwrap_or_default(),
        }
    }

    /// `3D 00 <GEAR_BY_ACCOUNT> <account>`: the kits of a member of a room
    /// now, as the list's side panel shows them to anyone (`S1;G`, a PIN
    /// room's too). Any other account (in no room, or none at all) gets no
    /// answer: account ids cannot be walked for anyone's kits.
    pub(super) fn listed_member_gear(&mut self, who: &Who, account_id: u32) {
        if self
            .rooms
            .all()
            .any(|room| room.members.contains(&account_id))
        {
            self.send_gear(who, account_id, account_id);
        } else {
            log::info!(
                "{} asked for the kits of account {account_id}: in no room",
                who.name
            );
        }
    }

    /// `3D 00 <GEAR_IN_ROOM> <place + 20 × check>`: the kits of the member
    /// at that place of the asker's room, if the check is its serial's now
    /// (a member came or went since its list was drawn: no answer).
    pub(super) fn room_member_gear(&mut self, who: &Who, value: u32) {
        let member = wire::member_gear_place(value).and_then(|(place, check)| {
            let room = self.rooms.of(who.account_id)?;
            (room.serial % wire::KICK_CHECKS == check)
                .then(|| room.members.get(usize::from(place)).copied())
                .flatten()
        });
        match member {
            Some(account_id) => self.send_gear(who, account_id, account_id),
            None => log::info!(
                "{} asked for the kits of a member of its room: not in it now",
                who.name
            ),
        }
    }

    /// Who is in the room of that key, for anyone looking at the list (the
    /// owner's of 2026-10-20), each with its side while it plays the room's
    /// match: the team of its latest place in the match's roster. Nobody: the
    /// room is gone.
    fn room_members(&self, who: &Who, key: u32) -> Vec<ListMember> {
        let Some(room) = self.rooms.get(key) else {
            return Vec::new();
        };
        let formed = match room.stage {
            Stage::InMatch { match_id, .. } => self.store.matchmaking().formed_match(match_id),
            _ => None,
        };
        let unix = quests::now();
        let marks = self.room_squad_marks(room);
        room.members
            .iter()
            .map(|&account_id| {
                let member = self.room_member(who, room, account_id, &marks, unix);
                let side = formed
                    .as_ref()
                    .filter(|_| member.in_match)
                    .and_then(|formed| {
                        formed
                            .seats
                            .iter()
                            .rev()
                            .find(|seat| seat.player.account_id == account_id)
                    })
                    .map(|seat| seat.team);
                ListMember {
                    member,
                    side,
                    account_id,
                }
            })
            .collect()
    }

    /// The asker's room as the movie is told it.
    fn room_view(&self, who: &Who, now: Instant) -> Option<RoomView> {
        let room = self.rooms.of(who.account_id)?;
        let tally = room.tally();
        let mine = room.votes.get(&who.account_id).copied();
        let unix = quests::now();
        let marks = self.room_squad_marks(room);
        let members = room
            .members
            .iter()
            .map(|&account_id| self.room_member(who, room, account_id, &marks, unix))
            .collect();
        let (phase, secs) = phase_of(room, now);
        let (join_in, join_hold) = match self.room_join_in(who, room, now) {
            Ok(secs) => (secs, None),
            Err(hold) => (None, Some(hold)),
        };
        Some(RoomView {
            key: room.key,
            level: self.room_level(room),
            max_level: room.max_level(),
            size: room.spec.size,
            wait: room.spec.wait,
            phase,
            secs,
            owner: room.owner == who.account_id,
            pool: room.pool.len() as u16,
            serial: room.serial,
            pin: !room.is_open(),
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
            // A private room's teams as its owner arranged them.
            teams: (!room.is_open()).then(|| {
                room.members
                    .iter()
                    .map(|member| room.sides.get(member).copied().unwrap_or(0))
                    .collect()
            }),
        })
    }

    /// Whether a room's match being played takes a member of the room now
    /// (`room_late_join` would put one in), and why not: a maintenance
    /// near; its sides full; or what the match server says (a joiner on its
    /// way in, a game loading, its last 30 s, its end, every place taken).
    /// `Err(None)`: it takes nobody, and nobody says why.
    fn late_hold(
        &self,
        match_id: u32,
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
        let (sides, rule) = self.late_rule(match_id, running, live.as_ref(), size);
        if rule.room == 0 {
            return Err(Some(JoinHold::PlayerLimit));
        }
        let anyone = Worth {
            players: 1,
            strength: Strength::ZERO,
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
        let Some(running) = self.matches.get(&match_id) else {
            return Ok(None);
        };
        let left = || {
            let at = self
                .room_join_at
                .get(&who.account_id)
                .copied()
                .unwrap_or(now + LATE_JOIN_WAIT);
            at.saturating_duration_since(now).as_millis().div_ceil(1000) as u32
        };
        // Its place in the match is kept (its game started anew, or came
        // back from it): free in the lobby, it goes back to it as Play
        // takes it, whatever the match takes now (`room_late_join`).
        if room.in_match.contains(&who.account_id) {
            return Ok(self.room_free(who.account_id).map(|_| left()));
        }
        if self.member_fit(who.account_id) != Fit::Yes {
            return Ok(None);
        }
        match self.late_hold(match_id, running, usize::from(room.spec.size), now) {
            Ok(()) => {}
            Err(Some(hold)) => return Err(hold),
            Err(None) => return Ok(None),
        }
        // A squad's side has no place for it (beside its mate, or two
        // together): it waits for the next match, told the match is full.
        if let Some(ready) = self.room_ready(who.account_id) {
            let late = self.late_placement(who.account_id, room, match_id, running, &ready, now);
            if late.squad && late.placement.is_none() {
                return Err(JoinHold::Full);
            }
        }
        Ok(Some(left()))
    }

    /// A room the member may switch to with one click (the owner's of
    /// 2026-10-19) out of an open room: open (no PIN, a place free, it was
    /// not turned out of it), its max level set, lower than its room's
    /// (none set counts as the highest) and not under its equipment; the
    /// lowest of them, then the fullest, then the oldest. None while it
    /// plays a match, and none out of a private room (no max level).
    fn room_switch(&self, who: &Who, room: &model::Room) -> Option<RoomSwitch> {
        // Only what a Join would take (no maintenance near, no clan team, a
        // kit that can play).
        self.room_enter_fit(who).ok()?;
        let playing = room.in_match.contains(&who.account_id)
            || self
                .player(who.addr)
                .is_some_and(|player| player.in_match.is_some() || player.connect.is_some());
        if playing {
            return None;
        }
        if !PublicBracket::UpTo15.contains(room) {
            return None;
        }
        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
        // Places for the squad mate a Join takes along.
        let places = 1 + usize::from(self.room_mate(who.account_id).is_some());
        let best = self
            .rooms
            .open_rooms(who.account_id, gear, places)
            .filter(|other| PublicBracket::UpTo10.contains(other))
            .min_by_key(|other| {
                (
                    other.max_level(),
                    std::cmp::Reverse(other.members.len()),
                    other.key,
                )
            })?;
        Some(RoomSwitch {
            key: best.key,
            max_level: best.max_level(),
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

    /// The asker's friends who are members of a room (not waiting in a
    /// queue), but of the asker's own room: a friend's menu offers "Join
    /// room" for these (the owner's of 2026-10-20). Any friend may see its
    /// friend's room; friendship is both ways.
    fn friend_rooms(&self, who: &Who) -> Vec<FriendRoom> {
        let friends = self
            .store
            .account(who.account_id)
            .map(|account| account.locked().friends.clone())
            .unwrap_or_default();
        let mine = self.rooms.key_of(who.account_id);
        friends
            .into_iter()
            .filter_map(|account_id| {
                let room = self.rooms.of(account_id)?;
                (Some(room.key) != mine).then(|| FriendRoom {
                    account_id,
                    key: room.key,
                    players: room.members.len() as u8,
                    size: room.spec.size,
                    pin: !room.is_open(),
                })
            })
            .take(wire::FRIEND_ROWS)
            .collect()
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
                    max_level: room.max_level(),
                    size: room.spec.size,
                    players: room.members.len() as u8,
                    phase,
                    secs,
                    map_id,
                    pin: !room.is_open(),
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
    /// The room Play puts the account into (until the end of the first
    /// match of it that it plays, unless it chooses the room): of the open
    /// rooms in its equipment bracket, the one whose level is nearest the
    /// player's, then the fullest, then its key.
    fn room_for_play(&self, account_id: u32, gear: u8, places: usize) -> Option<u32> {
        self.room_in_bracket(account_id, gear, PublicBracket::of(gear), places)
    }

    /// The room Play would take for `gear` in `bracket`
    /// ([`Self::room_for_play`]).
    fn room_in_bracket(
        &self,
        account_id: u32,
        gear: u8,
        bracket: PublicBracket,
        places: usize,
    ) -> Option<u32> {
        let mine = i32::from(self.level_of(account_id, quests::now()).min);
        self.rooms
            .open_rooms(account_id, gear, places)
            .filter(|room| bracket.contains(room))
            .filter(|room| self.rooms.queue_len(room.key) == 0)
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
    fn room_admit_queues(&mut self, now: Instant) {
        // Near a maintenance nobody may come: the queues wait, to be told
        // (code 9) when their rooms are disbanded.
        if self.maintenance_is_near() {
            return;
        }
        for key in self.rooms.with_queue_and_place() {
            while let Some(room) = self.rooms.get(key) {
                // Who of each entry would go now, the one rule: its lead
                // (one alone, or the commander) while it may (online, its
                // game told, a kit that plays; else it keeps its place,
                // passed over: it may switch back while it waits), with its
                // mate while the commander's actions take it (`room_mate`)
                // and the room lets it in; else the commander alone, its
                // mate staying where it is (squads.md). Then whether it may
                // come in (in the lobby, its max level, Play's bracket the
                // pair's): one that may not leaves the queue.
                let unix = quests::now();
                let gear_of = |account_id: u32| {
                    self.accounts.get(account_id).map_or(0, |account| {
                        matchmaking::gear_level(&account.locked(), unix)
                    })
                };
                let can_play = |account_id: u32| {
                    self.by_account.contains_key(&account_id)
                        && !self.room_unseen.contains_key(&account_id)
                        && self
                            .accounts
                            .get(account_id)
                            .is_some_and(|account| rules::kits_can_play(&account.locked(), unix))
                };
                let squads = self.store.squads();
                let goes: HashMap<u32, (Waiting, Option<u32>)> = self
                    .rooms
                    .queue_of(key)
                    .into_iter()
                    .filter(|waiting| can_play(waiting.lead()))
                    .map(|waiting| {
                        let lead = waiting.lead();
                        let going = match waiting {
                            Waiting::Pair { mate, .. }
                                if self.room_mate(lead).is_some_and(|(free, gear)| {
                                    free == mate && room.lets_in(gear)
                                }) =>
                            {
                                waiting
                            }
                            _ => Waiting::One(lead),
                        };
                        let beside = squads
                            .mate_of(lead)
                            .filter(|mate| room.members.contains(mate));
                        (lead, (going, beside))
                    })
                    .collect();
                let Some((going, choice)) = self
                    .rooms
                    .take_from_queue(key, &|waiting| goes.get(&waiting.lead()).copied())
                else {
                    break;
                };
                let accounts = going.accounts();
                let lead = going.lead();
                let party_gear = accounts
                    .iter()
                    .map(|&account| gear_of(account))
                    .max()
                    .unwrap_or(0);
                let fits = self.room_fit_of(lead).is_ok()
                    && self.rooms.get(key).is_some_and(|room| {
                        room.lets_in(gear_of(lead))
                            && (choice == QueueChoice::Chosen
                                || PublicBracket::of(party_gear).contains(room))
                    });
                let beside = goes.get(&lead).and_then(|(_, beside)| *beside);
                if fits && self.rooms.admit_party(&accounts, key, beside) {
                    for account_id in accounts {
                        log::info!("Account {account_id}: from room {key}'s queue into it");
                    }
                    continue;
                }
                // Out of the queue; Play's lead goes where Play takes it
                // (its mate along, `room_mate`).
                if choice == QueueChoice::Play
                    && let Some(who) = self.who_is(lead)
                {
                    self.room_quick(&who, now);
                }
                log::info!("Account {lead}: out of room {key}'s queue (may not come now)");
            }
        }
    }

    /// The account signed in from a game started anew (closed or crashed,
    /// not from its menu). Its room and its queue keep it, but that game
    /// may not know of them (live, 2026-10-20: one in a room it did not
    /// know of was pulled into its matches twice): it is seated in no
    /// match until the room is shown in it (`room_shown`), and loses its
    /// place if that is not within the 90 s a lost game has.
    pub(super) fn room_restarted(&mut self, account_id: u32, now: Instant) {
        if self.rooms.of(account_id).is_none() && self.rooms.queue_place(account_id).is_none() {
            return;
        }
        let wait = self.settings.link_timeout + self.settings.session_grace;
        log::info!(
            "Account {account_id}: signed in from a game started anew; in no match of its room              until its game is told the room"
        );
        self.room_unseen.insert(account_id, now + wait);
    }

    /// The account's game was told its room (a request answered with its
    /// state, or Play): the room plays with it again. One that comes while
    /// its room's match is played sees the room first (`LATE_JOIN_WAIT`).
    fn room_shown(&mut self, account_id: u32) {
        if self.room_unseen.remove(&account_id).is_some() {
            self.room_join_at.remove(&account_id);
            log::info!("Account {account_id}: its game started anew was told its room");
        }
    }

    /// Who signed in anew and was not told its room in time is out of it.
    fn room_unseen_gone(&mut self, now: Instant) {
        let late: Vec<u32> = self
            .room_unseen
            .iter()
            .filter(|(_, until)| **until <= now)
            .map(|(account_id, _)| *account_id)
            .collect();
        for account_id in late {
            self.room_unseen.remove(&account_id);
            if self.rooms.of(account_id).is_some() || self.rooms.queue_place(account_id).is_some() {
                log::info!(
                    "Account {account_id}: its game started anew was never told its room; out of it"
                );
                self.room_gone(account_id);
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
        self.room_unseen.remove(&account_id);
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

    /// The room's match is over: its players are back in the room, every
    /// member alike, however it came in (`Rooms::match_over`, the owner's of
    /// 2026-10-31); their results are sent after this (`end_running`).
    pub(super) fn room_match_over(&mut self, match_id: u32) {
        self.room_launching.remove(&match_id);
        if let Some(key) = self.rooms.match_over(match_id) {
            log::info!("Room {key}: its match {match_id:#x} is over; back in the room");
            if self.maintenance_is_near() {
                // A maintenance is near: the match was played out, and the
                // room ends with it.
                self.room_disband(key);
            }
        }
    }

    /// The room's match was called off before its countdown (fewer than
    /// its minimum could play it, `match.md` section 34): the room goes on
    /// as for a match that could not be made, its vote and wait whole.
    pub(super) fn room_match_not_made(&mut self, match_id: u32) {
        self.room_launching.remove(&match_id);
        if let Some(key) = self.rooms.match_not_made(match_id) {
            log::info!(
                "Room {key}: its match {match_id:#x} was called off before its countdown; the \
                 room votes and waits again"
            );
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

    /// The member's link if it is free in the lobby: its game told its
    /// room, in no match, not on its way to one.
    fn room_free(&self, account_id: u32) -> Option<SocketAddr> {
        // Its game started anew and does not know of the room yet.
        if self.room_unseen.contains_key(&account_id) {
            return None;
        }
        let addr = *self.by_account.get(&account_id)?;
        let player = self.player(addr)?;
        (player.in_match.is_none() && player.connect.is_none() && player.queued_since.is_none())
            .then_some(addr)
    }

    /// The member as its room's match takes it as a new place: free in the
    /// lobby, with no place kept in a match, its active kit one Play would
    /// take, its game of the pool.
    fn room_ready(&self, account_id: u32) -> Option<Ready> {
        let player = self.player(self.room_free(account_id)?)?;
        if self
            .store
            .matchmaking()
            .pool_of_account(account_id)
            .is_some()
        {
            return None;
        }
        let session_id = player.session_id;
        // Its game plays in matches that take players (a member's game
        // started anew may be an older one): counted for the room's vote
        // as it is seated, or a match is made with fewer than the room's
        // minimum and called off at once (`room_start`).
        if self.pool_for_sessions([session_id]) != Some(QueuePool::GrowingRoster) {
            return None;
        }
        let unix = quests::now();
        if self.member_fit(account_id) != Fit::Yes {
            return None;
        }
        let (kit_id, gear) = {
            let account = player.account.locked();
            (account.active_kit, matchmaking::gear_level(&account, unix))
        };
        Some(Ready {
            player: session::Player {
                session_id,
                account_id,
                profile_id: kit_id,
            },
            gear,
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
        self.room_unseen_gone(now);
        // A queued pair whose squad broke, or that another commands now,
        // waits as two at its place, the commander first (nobody loses its
        // place; each is its own from then on).
        let squads = self.store.squads();
        self.rooms.split_pairs(&|commander, mate| {
            squads.of(commander).is_some_and(|squad| {
                squad.commander() == commander && squad.mate_of(commander) == Some(mate)
            })
        });
        // A squad's "ready" follows its members' rooms, queues and matches
        // (`squad_ready`): the infos are compared and sent when they differ.
        if !self.store.squads().is_empty() {
            self.squads_dirty = true;
        }
        if self.rooms.is_empty() {
            return;
        }
        self.room_admit_queues(now);
        self.room_scale_queues(now);
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
            let squads = self.store.squads();
            self.rooms
                .all()
                .filter(|room| !matches!(room.stage, Stage::InMatch { .. }))
                .flat_map(|room| {
                    let ready: Vec<u32> = room
                        .members
                        .iter()
                        .copied()
                        .filter(|&member| self.room_ready(member).is_some())
                        .collect();
                    // A squad alone is no match (the owner's of 2026-10-31:
                    // a pair is never played against itself): its room
                    // waits for a third.
                    let pair_alone = room.is_open()
                        && ready.len() == 2
                        && squads.mate_of(ready[0]) == Some(ready[1]);
                    if pair_alone { Vec::new() } else { ready }
                })
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
                    .filter(|&&member| {
                        !room.in_match.contains(&member) || self.room_free(member).is_some()
                    })
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        // Each goes into the match `LATE_JOIN_WAIT` after it came (or at
        // once on its ask), back to its place if the match keeps one: the
        // room shows first (the owner's of 2026-10-19).
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
    /// equipment level within an open room's max level (which holds for its
    /// matches, not only its door: equipment changes in the room; a private
    /// room has none).
    pub(super) fn member_fit(&self, account_id: u32) -> Fit {
        let Some(account) = self.accounts.get(account_id) else {
            return Fit::Kit;
        };
        let unix = quests::now();
        let account = account.locked();
        // The kit the match would take (`settle_active_kit`, without
        // changing the account: this is read on every answer and tick).
        if !rules::kits_can_play(&account, unix) {
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
        let room_minimum = u8::try_from(room.needs()).unwrap_or(u8::MAX);
        // A PIN is the room's for its whole life: its every match pays
        // nothing (the owner's of 2026-10-20).
        let private = !room.is_open();
        // A private room plays its owner's arrangement as it is (the
        // owner's of 2026-10-26): no sides drawn.
        let arranged = private.then(|| room.arranged());
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
            .collect();
        // The match's equipment level: the highest gear level of the
        // players placed (the owner's of 2026-11-01); each counts for its
        // K/D and rating there.
        let match_gear = ready.iter().map(|each| each.gear).max().unwrap_or(0);
        let strength = |account_id: u32| self.strength_of(account_id, match_gear, false);
        // Squads of the room's members, each a party; any other member alone.
        let mut seen = HashSet::new();
        let mut parties: Vec<model::Party> = Vec::new();
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
            let mut party = vec![(each.player.account_id, strength(each.player.account_id))];
            for mate in mates {
                if seen.insert(mate) && ready.iter().any(|other| other.player.account_id == mate) {
                    party.push((mate, strength(mate)));
                }
            }
            parties.push(model::Party::new(party));
        }
        // A squad alone plays no match against itself (the owner's of
        // 2026-10-31): the room waits for a third.
        if arranged.is_none() && parties.len() == 1 && parties[0].members().len() > 1 {
            log::info!("Room {key}: no match yet: only a squad can play");
            self.rooms.not_started(key, now);
            return;
        }
        // Drawn among the splits near the most even one (the owner's "a bit
        // of diffusion"), by a seed of this start.
        let seed = self.rng.below(u32::MAX);
        let player_value = self.settings.balance.player_value();
        let sides = match &arranged {
            Some(arranged) => arranged.clone(),
            None => model::sides(&parties, size, player_value, seed),
        };
        let player_of = |account_id: &u32| {
            ready
                .iter()
                .find(|each| each.player.account_id == *account_id)
                .map(|each| each.player)
        };
        let teams: [Vec<session::Player>; 2] = sides
            .each_ref()
            .map(|side| side.iter().filter_map(player_of).collect());
        // Never fewer than the room's minimum: the match server would call
        // such a match off at once (`MatchSetup::room_minimum`), and the room
        // make it again. A side empty only in a match of one's minimum (a
        // private room's 1 v 0).
        let seated = teams.iter().map(Vec::len).sum::<usize>();
        if seated < usize::from(room_minimum)
            || (room_minimum > 1 && teams.iter().any(Vec::is_empty))
        {
            log::info!(
                "Room {key}: no match yet: {} of {} members are free with a kit Play takes, \
                 {seated} seated, the room needs {room_minimum}",
                ready.len(),
                members.len()
            );
            self.rooms.not_started(key, now);
            return;
        }
        // Each squad of two or more is a squad in the match too.
        let mut squad_ids: std::collections::HashMap<u32, u8> = std::collections::HashMap::new();
        let mut next_squad = 0_u8;
        for party in parties
            .iter()
            .map(model::Party::members)
            .filter(|party| party.len() > 1)
        {
            // In a private room a squad may be on both teams: a squad in the
            // match is its members on one team, two at least.
            let groups: Vec<Vec<u32>> = match arranged {
                Some(_) => sides
                    .iter()
                    .map(|side| {
                        party
                            .iter()
                            .copied()
                            .filter(|member| side.contains(member))
                            .collect()
                    })
                    .collect(),
                None => vec![party.to_vec()],
            };
            for group in groups.iter().filter(|group| group.len() > 1) {
                next_squad = next_squad.saturating_add(1);
                for &member in group {
                    squad_ids.insert(member, next_squad);
                }
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
        self.room_launching
            .insert(match_id, (map_id, room_minimum, private));
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

    /// The member goes into its room's match being played, if it is free:
    /// back to its place if the match keeps one (its game started anew, or
    /// came back from it), as Play does (`back_to_seat`); else as a new
    /// place if the match admits it now, or it waits for the next one.
    fn room_late_join(&mut self, account_id: u32, now: Instant) {
        let Some(room) = self.rooms.of(account_id) else {
            return;
        };
        let Stage::InMatch { match_id, .. } = room.stage else {
            return;
        };
        let key = room.key;
        let Some(ticket) = self
            .matches
            .get(&match_id)
            .map(|running| running.ticket.match_id)
        else {
            return;
        };
        if room.in_match.contains(&account_id) {
            if let Some(addr) = self.room_free(account_id)
                && self.back_to_seat(addr, now)
            {
                self.room_join_at.remove(&account_id);
                log::info!(
                    "Room {key}: {} went back to its place in its match {ticket:#x}",
                    self.name_of(account_id).unwrap_or_default()
                );
            }
            return;
        }
        let running = &self.matches[&match_id];
        if !self.late_admission(running, now) || self.maintenance_is_near() {
            return;
        }
        // One that left this match by its own word comes again as a new
        // place, its state fresh; the place it left is gone for good
        // (`room_left_matches`). A place is never taken twice.
        let Some(ready) = self.room_ready(account_id) else {
            return;
        };
        // A soldier whose commander waits in the room to go in goes with
        // it, by its commander's turn (`late_placement`).
        if self.squad_soldier(account_id)
            && self.store.squads().of(account_id).is_some_and(|squad| {
                let commander = squad.commander();
                room.members.contains(&commander)
                    && !room.in_match.contains(&commander)
                    && self.room_ready(commander).is_some()
            })
        {
            return;
        }
        let Late {
            placement,
            lead,
            party,
            ..
        } = self.late_placement(account_id, room, match_id, running, &ready, now);
        let Some(placement) = placement else {
            return;
        };
        let team = placement.assignments.first().map_or(0, |(_, team)| *team);
        let unix = quests::now();
        let profiles: Option<Vec<MatchPlayer>> = party
            .iter()
            .map(|each| {
                let account = self.accounts.get(each.player.account_id)?;
                let mut account = account.locked();
                account.settle_active_kit(unix);
                Some(MatchPlayer {
                    session_id: each.player.session_id,
                    account_id: each.player.account_id,
                    profile: match_profile(&account, team, 0, unix),
                    bot: self.accounts.is_bot_name(&account.name),
                    driven: false,
                })
            })
            .collect();
        let Some(profiles) = profiles else {
            return;
        };
        // A squad goes in together: its places reserved at once, or none.
        let players: Vec<(session::Player, u8)> =
            party.iter().map(|each| (each.player, team)).collect();
        let first = running.seats.len();
        let host = self.host;
        let admitted = self
            .store
            .matchmaking()
            .append_players(match_id, &players, |actual, _| {
                actual == first && host.join_running(ticket, profiles)
            });
        if !admitted {
            return;
        }
        let joined: Vec<(session::Player, u8, Strength)> = party
            .iter()
            .map(|each| (each.player, team, each.strength))
            .collect();
        self.seated_late(match_id, &joined, &placement, lead);
        for each in &party {
            let account_id = each.player.account_id;
            self.rooms.seated_late(key, account_id, team);
            self.room_join_at.remove(&account_id);
            log::info!(
                "Room {key}: {} went into its match {ticket:#x} late, team {team}",
                self.name_of(account_id).unwrap_or_default()
            );
            self.room_send_to_match(each.player, now);
        }
    }

    /// Where the room's match being played (`running`, `match_id`) would
    /// take the member (`ready`) now (`room_late_join`): a private room's
    /// member back on the team its owner put it on; a squad on one side
    /// (the owner's of 2026-10-31): beside its mate in the match, or
    /// nowhere (it waits for the next match); with its mate waiting to go
    /// in too, both together on a side with places for both, or neither.
    /// A side's places are its players in the match and those whose place
    /// is kept (`late_rule`).
    fn late_placement(
        &self,
        account_id: u32,
        room: &model::Room,
        match_id: u32,
        running: &Running,
        ready: &Ready,
        now: Instant,
    ) -> Late {
        let live = self.host.live(running.ticket.match_id, now);
        let (sides, rule) = self.late_rule(
            match_id,
            running,
            live.as_ref(),
            usize::from(room.spec.size),
        );
        let formed = self.store.matchmaking().formed_match(match_id);
        let side_in_match = |account: u32| {
            formed.as_ref().and_then(|formed| {
                formed
                    .seats
                    .iter()
                    .rev()
                    .find(|seat| seat.player.account_id == account)
                    .map(|seat| seat.team)
            })
        };
        let mate = self.store.squads().mate_of(account_id);
        let mate_side = mate
            .filter(|mate| room.in_match.contains(mate))
            .and_then(side_in_match);
        // A commander takes its mate waiting in the room with it
        // (`room_mate`): both in on one side, or neither.
        let waiting_mate = self
            .room_mate(account_id)
            .map(|(mate, _)| mate)
            .filter(|mate| {
                mate_side.is_none() && room.members.contains(mate) && !room.in_match.contains(mate)
            })
            .and_then(|mate| self.room_ready(mate));
        let going: Vec<(session::Player, u8)> = std::iter::once((ready.player, ready.gear))
            .chain(waiting_mate.map(|mate| (mate.player, mate.gear)))
            .collect();
        // Each counts for its K/D and rating at the match's equipment level,
        // which a joiner of higher gear raises (the owner's of 2026-11-01);
        // a squad's members 1.3 times.
        let squad = mate_side.is_some() || going.len() > 1;
        let match_gear = going.iter().map(|(_, gear)| *gear).fold(
            self.results
                .match_gear(running.ticket.match_id)
                .unwrap_or(0),
            u8::max,
        );
        let party: Vec<Joiner> = going
            .into_iter()
            .map(|(player, _)| Joiner {
                player,
                strength: self.strength_of(player.account_id, match_gear, squad),
            })
            .collect();
        let worth = Worth {
            players: party.len(),
            strength: party.iter().map(|each| each.strength).sum(),
        };
        // One who played this match before (left it, or its game was gone
        // for good) goes back to its side when the sides have as many
        // players: the side of its latest place in the roster, which keeps
        // every place, given up or not.
        let previous = side_in_match(account_id);
        let placement = match (room.arranged_side(account_id), mate_side) {
            (Some(side), _) => policy::place_joiner_on(sides, worth, side, &rule),
            (None, Some(side)) => policy::place_joiner_beside(sides, worth, side, &rule),
            (None, None) => policy::place_joiner(sides, worth, previous, &rule),
        };
        Late {
            placement,
            lead: rule.lead,
            squad,
            party,
        }
    }
}

/// One who goes into a match being played, and what it counts for there.
struct Joiner {
    player: session::Player,
    strength: Strength,
}

/// [`Core::late_placement`]'s answer.
struct Late {
    /// The side, or none now.
    placement: Option<policy::LatePlacement>,
    lead: [Strength; 2],
    /// Who goes in: the member, and its squad mate waiting with it.
    party: Vec<Joiner>,
    /// Whether its squad decided the side (its mate in the match, or
    /// going in with it).
    squad: bool,
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
    fn public_brackets_include_level_ten_in_the_lower_one() {
        assert_eq!(PublicBracket::of(0), PublicBracket::UpTo10);
        assert_eq!(PublicBracket::of(10), PublicBracket::UpTo10);
        assert_eq!(PublicBracket::of(11), PublicBracket::UpTo15);
        assert_eq!(PublicBracket::of(15), PublicBracket::UpTo15);
    }

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
