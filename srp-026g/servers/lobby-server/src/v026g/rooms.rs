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
use std::time::{Duration, Instant};

use session::matchmaking_policy::{self as policy, Closed, Worth};
use session::v026g::presets::{self, Offer};
use session::v026g::room_settings::{self, RoomSettings};
use session::v026g::rooms::{self as model, Door, Due, QueueChoice, Refused, Spec, Stage};
use session::v026g::{matchmaking, quests, rules};
use session::{LockExt, QueuePool};
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

/// A free member of a room, ready for its match.
struct Ready {
    player: session::Player,
    strength: i32,
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

    /// The account's squad mate with its equipment level, when the mate
    /// comes along with the account's Play, Join or queue now: free in the
    /// lobby as [`Self::room_fit_of`] says (in a room too, while it plays no
    /// match), its game told its room, its kit able to play.
    pub(super) fn room_mate(&self, account_id: u32) -> Option<(u32, u8)> {
        let mate = self.store.squads().mate_of(account_id)?;
        if self.room_unseen.contains_key(&mate) || self.room_fit_of(mate).is_err() {
            return None;
        }
        let account = self.accounts.get(mate)?;
        let account = account.locked();
        let unix = quests::now();
        rules::kits_can_play(&account, unix)
            .then(|| (mate, matchmaking::gear_level(&account, unix)))
    }

    /// The account's mate that its Play takes along: [`Self::room_mate`]
    /// in no room (one in a room is joined there instead), and the
    /// equipment level Play picks a room by: the higher of the two, so the
    /// room's max level lets both in.
    fn play_party(&self, account_id: u32, gear: u8) -> (Option<(u32, u8)>, u8) {
        let mate = self
            .room_mate(account_id)
            .filter(|(mate, _)| self.rooms.key_of(*mate).is_none());
        let party_gear = mate.map_or(gear, |(_, mate_gear)| gear.max(mate_gear));
        (mate, party_gear)
    }

    /// The squad info's "ready" for a member (`social.md` §2.2, the game's
    /// Play gating): it is free in the lobby, in no room and no queue, its
    /// kit able to play, so that its commander's Play takes it along. The
    /// game lets a commander press Play while one of the squad is ready (it
    /// is, itself, when Play could take it), and a soldier while its
    /// commander is not: a free commander leads the pair, a busy one (in a
    /// room, a match, away) leaves its soldier's Play to it.
    pub(super) fn squad_ready(&self, account_id: u32) -> bool {
        self.rooms.key_of(account_id).is_none()
            && self.rooms.queue_place(account_id).is_none()
            && !self.room_unseen.contains_key(&account_id)
            && self.room_fit_of(account_id).is_ok()
            && self
                .accounts
                .get(account_id)
                .is_some_and(|account| rules::kits_can_play(&account.locked(), quests::now()))
    }

    /// The account and its mate wait in room `key`'s queue together (the
    /// mate when it comes along: in no room, the room lets it in). The
    /// account's place, or why not.
    fn room_enqueue_party(
        &mut self,
        who: &Who,
        gear: u8,
        mate: Option<(u32, u8)>,
        key: u32,
        choice: QueueChoice,
        now: Instant,
    ) -> Result<usize, Refused> {
        let place = match choice {
            QueueChoice::Chosen => self.rooms.enqueue(who.account_id, gear, key, now),
            QueueChoice::Play => self.rooms.enqueue_play(who.account_id, gear, key, now),
        }?;
        if let Some((mate, mate_gear)) = mate.filter(|(mate, _)| self.rooms.key_of(*mate).is_none())
        {
            let queued = match choice {
                QueueChoice::Chosen => self.rooms.enqueue(mate, mate_gear, key, now),
                QueueChoice::Play => self.rooms.enqueue_play(mate, mate_gear, key, now),
            };
            if let Err(refused) = queued {
                log::info!(
                    "{}: its squad mate stays out of room {key}'s queue ({refused:?})",
                    who.name
                );
            }
        }
        Ok(place)
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
            // A full room's queue (the owner's of 2026-10-17): from the lobby
            // only, one at a time; a room with a place is joined at once.
            // A squad mate free in the lobby waits with it (`room_mate`).
            RoomRequest::Queue { key } => {
                let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
                let mate = self.room_mate(who.account_id);
                let free = self.rooms.get(key).is_some_and(|room| {
                    let comes = mate.is_some_and(|(mate, _)| !room.members.contains(&mate));
                    room.free_places() > usize::from(comes)
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
                        match self.room_enqueue_party(
                            who,
                            gear,
                            mate,
                            key,
                            QueueChoice::Chosen,
                            now,
                        ) {
                            Ok(place) => {
                                // Queue stays lit while queued: a press again
                                // is logged only when the place moved.
                                if before != Some((key, place)) {
                                    log::info!(
                                        "{}: waits in room {key}'s queue, place {place}",
                                        who.name
                                    );
                                }
                                RoomCode::None
                            }
                            Err(refused) => code_of(refused),
                        }
                    }
                };
                self.send_room_state(who, code, now);
            }
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
            // Its squad mate waiting in the same queue leaves it too: they
            // came together.
            RoomRequest::Unqueue => {
                if let Some(key) = self.rooms.dequeue(who.account_id) {
                    log::info!("{}: left room {key}'s queue", who.name);
                    if let Some(mate) = self.store.squads().mate_of(who.account_id)
                        && self
                            .rooms
                            .queue_place(mate)
                            .is_some_and(|(queued, _)| queued == key)
                    {
                        self.rooms.dequeue(mate);
                    }
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

    /// Join the room with that key, the squad mate along (`room_mate`); a
    /// pair with too few places, both in no room, waits in its queue
    /// together. The state answers it.
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
                    if mate.is_some_and(|(mate, _)| {
                        self.rooms.key_of(mate).is_none()
                            && self.rooms.key_of(who.account_id).is_none()
                    }) =>
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
    /// then the join of the room with `key` (0: Play). A preset that could
    /// not be chosen joins nothing (code 14).
    fn room_preset_join(&mut self, who: &Who, preset: u8, key: u32, now: Instant) {
        match self.preset_choose(who, preset) {
            Ok(_) => {
                self.send_presets(who, PresetDone::Chosen, preset);
                if key == 0 {
                    self.room_play_pressed(who, now);
                } else {
                    self.room_join(who, key, now);
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
            self.room_create_public(who, PublicBracket::of(party_gear), now)
                .map(|key| {
                    if let Some((mate, mate_gear)) = mate
                        && let Err(refused) = self.rooms.join(mate, mate_gear, key, now)
                    {
                        log::info!(
                            "{}: its squad mate stays out of room {key} ({refused:?})",
                            who.name
                        );
                    }
                })
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
        for (account_id, key) in self.rooms.play_queue() {
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
                let waiting: Vec<_> = self
                    .rooms
                    .play_queue()
                    .into_iter()
                    .filter_map(|(account_id, _)| {
                        let who = self.who_is(account_id)?;
                        if self.room_unseen.contains_key(&account_id)
                            || self.room_enter_fit(&who).is_err()
                        {
                            return None;
                        }
                        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
                        // A pair by the higher gear of the two (`play_party`).
                        let (_, party_gear) = self.play_party(account_id, gear);
                        (PublicBracket::of(party_gear) == bracket).then_some((who, gear))
                    })
                    .collect();
                if waiting.len() < SCALE_PLAYERS {
                    break;
                }
                let Ok(key) = self.room_create_public(&waiting[0].0, bracket, now) else {
                    // Capacity or map configuration failure: keep everyone
                    // queued, including the player who would make the room.
                    break;
                };
                for (who, gear) in waiting.iter().skip(1).take(usize::from(QUICK_SIZE) * 2 - 1) {
                    // One that cannot join now waits in its queue.
                    let _ = self.rooms.join(who.account_id, *gear, key, now);
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
            squad: marks.get(&account_id).copied().unwrap_or(0),
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
        if rule.room == 0 {
            return Err(Some(JoinHold::PlayerLimit));
        }
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
            // One whose kit can not play now keeps its place, passed over
            // (it may switch back while it waits); the next goes in. A pair
            // waiting together goes in together, at two places.
            loop {
                let party = {
                    let accounts = &self.accounts;
                    let online = &self.by_account;
                    let unseen = &self.room_unseen;
                    let squads = self.store.squads();
                    let unix = quests::now();
                    let can_play = |account_id: u32| {
                        online.contains_key(&account_id)
                            && !unseen.contains_key(&account_id)
                            && accounts.get(account_id).is_some_and(|account| {
                                rules::kits_can_play(&account.locked(), unix)
                            })
                    };
                    let mate_of = |account_id: u32| squads.mate_of(account_id);
                    self.rooms.next_ready_in_queue(key, &can_play, &mate_of)
                };
                if party.is_empty() {
                    break;
                }
                // Play's bracket is the pair's: the higher gear of the two.
                let unix = quests::now();
                let gear_of = |account_id: u32| {
                    self.accounts
                        .get(account_id)
                        .map(|account| matchmaking::gear_level(&account.locked(), unix))
                };
                let party_gear = party
                    .iter()
                    .filter_map(|(account_id, _)| gear_of(*account_id))
                    .max()
                    .unwrap_or(0);
                let mut beside = None;
                for (account_id, choice) in party {
                    // Beside its mate: the one let in with it, or one in the
                    // room already.
                    let mate = self.store.squads().mate_of(account_id).filter(|mate| {
                        self.rooms
                            .get(key)
                            .is_some_and(|room| room.members.contains(mate))
                    });
                    let fits = self.room_fit_of(account_id).is_ok()
                        && gear_of(account_id).is_some_and(|gear| {
                            self.rooms.get(key).is_some_and(|room| {
                                room.lets_in(gear)
                                    && (choice == QueueChoice::Chosen
                                        || PublicBracket::of(party_gear).contains(room))
                            })
                        });
                    if fits && self.rooms.admit(account_id, key, beside.or(mate)) {
                        log::info!("Account {account_id}: from room {key}'s queue into it");
                        beside = Some(account_id);
                        continue;
                    }
                    if choice == QueueChoice::Play
                        && let Some(who) = self.who_is(account_id)
                    {
                        self.room_quick(&who, now);
                    }
                    log::info!(
                        "Account {account_id}: out of room {key}'s queue (may not come now)"
                    );
                }
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

    /// The member as its room's match takes it: free in the lobby, its
    /// active kit one Play would take, its game of the pool.
    fn room_ready(&self, account_id: u32) -> Option<Ready> {
        // Its game started anew and does not know of the room yet.
        if self.room_unseen.contains_key(&account_id) {
            return None;
        }
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
        self.room_unseen_gone(now);
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
        let sides = match &arranged {
            Some(arranged) => arranged.clone(),
            None => model::sides(&parties, size, player_value),
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
        for (party, _) in parties.iter().filter(|(party, _)| party.len() > 1) {
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
                None => vec![party.clone()],
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
        // A private room's member goes back to the team its owner put it on.
        let arranged = room.arranged_side(account_id);
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
        // A squad goes in on one side (the owner's of 2026-10-31): beside its
        // mate in this match, or it waits; the first of a pair waiting
        // together goes in on a side with places for both, and the other
        // follows at once.
        let mate = self.store.squads().mate_of(account_id);
        let mate_side = mate
            .filter(|mate| room.in_match.contains(mate))
            .and_then(|mate| {
                self.store
                    .matchmaking()
                    .formed_match(match_id)
                    .and_then(|formed| {
                        formed
                            .seats
                            .iter()
                            .rev()
                            .find(|seat| seat.player.account_id == mate)
                            .map(|seat| seat.team)
                    })
            });
        let mate_waits = mate.filter(|mate| {
            mate_side.is_none()
                && room.members.contains(mate)
                && !room.in_match.contains(mate)
                && self.room_ready(*mate).is_some()
        });
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
        let placement = match (arranged, mate_side, mate_waits) {
            (Some(side), _, _) => policy::place_joiner_on(sides, worth, side, &rule),
            (None, Some(side), _) => policy::place_joiner_beside(sides, worth, side, &rule),
            (None, None, Some(_)) => {
                let pair = Worth {
                    players: 2,
                    ..worth
                };
                policy::place_joiner(sides, pair, previous, &rule)
                    .and_then(|placement| placement.assignments.first().map(|(_, side)| *side))
                    .and_then(|side| policy::place_joiner_beside(sides, worth, side, &rule))
            }
            (None, None, None) => policy::place_joiner(sides, worth, previous, &rule),
        };
        let Some(placement) = placement else {
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
        self.rooms.seated_late(key, account_id, team);
        log::info!(
            "Room {key}: {} went into its match {ticket:#x} late, team {team}",
            self.name_of(account_id).unwrap_or_default()
        );
        self.room_send_to_match(ready.player, now);
        // Its mate waited with it: in now, on this side.
        if let Some(mate) = mate_waits {
            self.room_join_at.insert(mate, now);
            self.room_late_join(mate, now);
        }
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
