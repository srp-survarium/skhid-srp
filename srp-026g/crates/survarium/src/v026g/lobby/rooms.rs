//! Rooms ("servers" in the players' words): srp's own wire, on the ranking
//! page's request (`3B`) at reserved starts and its reply (`91`), as
//! "Player vs Bots" is (`lobby.md` §2.16.3). Only srp's
//! changed lobby movie asks; a stock game never does.
//!
//! Every start of this module is from [`ROOMS_FIRST`] to [`ROOMS_LAST`]:
//! above "Player vs Bots", and within an `int`, which is how
//! the movie passes them. Each answer is one `91` whose first row's place is
//! the start the movie tells it by; rating and change are 0 in every row.
//! The rows' names are ASCII fields with `;` between them, the first the
//! version [`ROOMS_VERSION`] and the second the kind of the answer.

use super::push::{Ranked, elo_rows};
use super::{ServerMessage, WireError, at_most};

/// The first start of a room request: the list of the rooms.
pub const ROOMS_FIRST: u32 = 2_060_000_000;
/// `3B 00 <ROOMS_LIST> <count>`: the rooms, at most `count` of them
/// ([`room_list`]).
pub const ROOMS_LIST: u32 = ROOMS_FIRST;
/// The asker's own room ([`room_state`]).
pub const ROOM_STATE: u32 = ROOMS_FIRST + 1;
/// Leave the asker's room (the window's cross). Answered with its state.
pub const ROOM_LEAVE: u32 = ROOMS_FIRST + 2;
/// `ROOM_VOTE_BASE + card`, card 0 to 2: a vote. Answered with the state.
pub const ROOM_VOTE_BASE: u32 = ROOMS_FIRST + 3;
/// Play: into an open room, or a new one (the movie's Play button with
/// rooms on: no search, no queue). Answered with the state.
pub const ROOM_PLAY: u32 = ROOMS_FIRST + 6;
/// `ROOM_POOL_BASE + page`: the maps a room's pool may hold
/// ([`room_pool`]), [`POOL_PAGE_ROWS`] a page.
pub const ROOM_POOL_BASE: u32 = ROOMS_FIRST + 10;
/// `ROOM_KICK_BASE + place + 20 × check` ([`kick_start`]): the owner takes
/// the member at that place of the state's list out of the room; `check` is
/// that state's serial modulo [`KICK_CHECKS`], so a kick made from an older
/// list does nothing. Answered with the state.
pub const ROOM_KICK_BASE: u32 = ROOMS_FIRST + 100;
/// The values of a kick's check.
pub const KICK_CHECKS: u32 = 495;
/// `ROOM_PIN_BASE + pin`, pin 0 to 9999: the PIN the asker's next join
/// gives, or its next room asks for ([`pin_answer`]).
pub const ROOM_PIN_BASE: u32 = ROOMS_FIRST + 10_000;
/// `ROOM_JOIN_BASE + key`, key 1 to [`ROOM_KEY_MAX`]: join that room.
/// Answered with the state.
pub const ROOM_JOIN_BASE: u32 = ROOMS_FIRST + 100_000;
/// `ROOM_MEMBERS_BASE + key`, key 1 to [`ROOM_KEY_MAX`]: who is in that
/// room, for anyone looking at the list ([`room_members`]; since
/// 2026-10-20, a movie before it never asks).
pub const ROOM_MEMBERS_BASE: u32 = ROOMS_FIRST + 300_000;
/// Leave the queue the asker waits in. Answered with the state.
pub const ROOM_UNQUEUE: u32 = ROOMS_FIRST + 7;
/// Go into the asker's room's match being played now, without waiting for
/// [`RoomView::join_in`]. Answered with the state.
pub const ROOM_JOIN_MATCH: u32 = ROOMS_FIRST + 8;
/// What the asker's create window starts with: the settings of the last
/// room it made by hand, fitted to the pool and the rules now
/// ([`room_form`]; the owner's of 2026-10-20).
pub const ROOM_FORM: u32 = ROOMS_FIRST + 9;
/// `ROOM_QUEUE_BASE + key`, key 1 to [`ROOM_KEY_MAX`]: wait in that full
/// room's queue (its PIN, if it asks one, given just before), or join it if
/// a place is free. Answered with the state.
pub const ROOM_QUEUE_BASE: u32 = ROOMS_FIRST + 200_000;
/// `ROOM_BITS_BASE + chunk × 2^20 + bits`: 20 entries of the pool of the
/// room the asker creates next, entry `chunk × 20 + b` in by bit `b`
/// ([`bits_answer`]).
pub const ROOM_BITS_BASE: u32 = ROOMS_FIRST + 1_000_000;
/// `ROOM_CREATE_BASE + code` ([`create_start`]): make a room. Answered with
/// the state.
pub const ROOM_CREATE_BASE: u32 = ROOMS_FIRST + 10_000_000;
/// The last start of a room request.
pub const ROOMS_LAST: u32 = ROOMS_FIRST + 19_999_999;

/// The first field of every row's name.
pub const ROOMS_VERSION: &str = "S1";
/// The largest room key, the number the room is joined by (and its code).
pub const ROOM_KEY_MAX: u32 = 99_999;
/// The largest PIN.
pub const PIN_MAX: u16 = 9999;
/// The team sizes, each side's players.
pub const SIZES: std::ops::RangeInclusive<u8> = 6..=10;
/// The highest max level.
pub const LEVEL_MAX: u8 = 15;
/// The waits before a match, in seconds: 15 to 60 by 5.
pub const WAITS: [u8; 10] = [15, 20, 25, 30, 35, 40, 45, 50, 55, 60];
/// The players a side a room needs before its vote begins: 1 v 1 to 3 v 3
/// (the owner's of 2026-10-17).
pub const MIN_SIDES: std::ops::RangeInclusive<u8> = 1..=3;
/// The pool entries one bits request carries.
pub const BITS_PER_CHUNK: u32 = 20;
/// The most chunks: a pool of 160 entries at the most.
pub const CHUNKS: u32 = 8;
/// The most pool entries a create can name: one bit each.
pub const POOL_BITS: usize = (CHUNKS * BITS_PER_CHUNK) as usize;
/// The pool's entries a page of [`room_pool`] lists.
pub const POOL_PAGE_ROWS: usize = 30;
/// The most pages of the pool.
pub const POOL_PAGES: u32 = 16;
/// The most rooms one [`room_list`] lists, whatever the `3B` asks for.
pub const LIST_ROWS: usize = 31;
/// The most members a room has: two sides of 10.
pub const MEMBERS_MAX: usize = 20;
/// The most cards of a vote.
pub const CARDS: usize = 3;
/// The most bytes of a name in a row: a player's name is at most this long
/// in the game (`limits::NAME` is the buffer's).
const ROW_NAME: usize = 32;

/// What a room is made with (the create window).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomSpec {
    /// Each side's players, one of [`SIZES`].
    pub size: u8,
    /// The highest equipment level let in, 1 to [`LEVEL_MAX`]; 0: any.
    pub max_level: u8,
    /// Seconds from the second player to the match, one of [`WAITS`].
    pub wait: u8,
    /// Joining asks for a PIN: the one the creator gave with its PIN
    /// request just before ([`ROOM_PIN_BASE`]).
    pub pin: bool,
    /// The players a side it needs before its vote begins, one of
    /// [`MIN_SIDES`].
    pub min_side: u8,
}

/// A room request, by its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomRequest {
    List { count: u8 },
    State,
    Leave,
    Play,
    Vote { card: u8 },
    Pool { page: u8 },
    Kick { place: u8, check: u32 },
    Pin { pin: u16 },
    Join { key: u32 },
    Queue { key: u32 },
    Members { key: u32 },
    Unqueue,
    JoinMatch,
    Form,
    Bits { chunk: u8, bits: u32 },
    Create(RoomSpec),
}

/// The code of a create: `((((size − 6) × 16 + max level) × 10 + wait index)
/// × 2 + PIN) × 3 + (min side − 1)`, PIN 1 when joining asks for one. A room has no level of its
/// own to make: the server works it out from its players.
pub fn create_start(spec: RoomSpec) -> u32 {
    let wait = WAITS
        .iter()
        .position(|&wait| wait == spec.wait)
        .unwrap_or(3) as u32;
    let size = u32::from(spec.size.clamp(*SIZES.start(), *SIZES.end()) - SIZES.start());
    let max_level = u32::from(spec.max_level.min(LEVEL_MAX));
    let min_side =
        u32::from(spec.min_side.clamp(*MIN_SIDES.start(), *MIN_SIDES.end()) - MIN_SIDES.start());
    let code = (((size * 16 + max_level) * 10 + wait) * 2 + u32::from(spec.pin)) * 3 + min_side;
    ROOM_CREATE_BASE + code
}

/// The start of a kick of the member at `place` of a state of `serial`.
pub fn kick_start(place: u8, serial: u32) -> u32 {
    ROOM_KICK_BASE + u32::from(place) + MEMBERS_MAX as u32 * (serial % KICK_CHECKS)
}

/// The `match` of a `3D 00 <GEAR_IN_ROOM> <match>` for the member at `place`
/// of a room state of that `serial` (`lobby::GEAR_IN_ROOM`), and back:
/// `(place, check)`.
pub fn member_gear(place: u8, serial: u32) -> u32 {
    u32::from(place) + MEMBERS_MAX as u32 * (serial % KICK_CHECKS)
}

pub fn member_gear_place(value: u32) -> Option<(u8, u32)> {
    let check = value / MEMBERS_MAX as u32;
    (check < KICK_CHECKS).then(|| ((value % MEMBERS_MAX as u32) as u8, check))
}

/// The start of 20 entries of a pool: `chunk` 0 to [`CHUNKS`] − 1, `bits`
/// below 2^20.
pub fn bits_start(chunk: u8, bits: u32) -> u32 {
    ROOM_BITS_BASE + u32::from(chunk) * (1 << BITS_PER_CHUNK) + (bits & ((1 << BITS_PER_CHUNK) - 1))
}

/// The room request a reserved start is, if it is one.
pub fn room_request(start: u32, count: u8) -> Option<RoomRequest> {
    use RoomRequest as R;
    if !(ROOMS_FIRST..=ROOMS_LAST).contains(&start) {
        return None;
    }
    let request = match start {
        ROOMS_LIST => R::List { count },
        ROOM_STATE => R::State,
        ROOM_LEAVE => R::Leave,
        ROOM_PLAY => R::Play,
        ROOM_UNQUEUE => R::Unqueue,
        ROOM_JOIN_MATCH => R::JoinMatch,
        ROOM_FORM => R::Form,
        _ if (ROOM_VOTE_BASE..ROOM_VOTE_BASE + CARDS as u32).contains(&start) => R::Vote {
            card: (start - ROOM_VOTE_BASE) as u8,
        },
        _ if (ROOM_POOL_BASE..ROOM_POOL_BASE + POOL_PAGES).contains(&start) => R::Pool {
            page: (start - ROOM_POOL_BASE) as u8,
        },
        _ if (ROOM_KICK_BASE..ROOM_KICK_BASE + MEMBERS_MAX as u32 * KICK_CHECKS)
            .contains(&start) =>
        {
            R::Kick {
                place: ((start - ROOM_KICK_BASE) % MEMBERS_MAX as u32) as u8,
                check: (start - ROOM_KICK_BASE) / MEMBERS_MAX as u32,
            }
        }
        _ if (ROOM_PIN_BASE..=ROOM_PIN_BASE + u32::from(PIN_MAX)).contains(&start) => R::Pin {
            pin: (start - ROOM_PIN_BASE) as u16,
        },
        _ if (ROOM_JOIN_BASE + 1..=ROOM_JOIN_BASE + ROOM_KEY_MAX).contains(&start) => R::Join {
            key: start - ROOM_JOIN_BASE,
        },
        _ if (ROOM_QUEUE_BASE + 1..=ROOM_QUEUE_BASE + ROOM_KEY_MAX).contains(&start) => R::Queue {
            key: start - ROOM_QUEUE_BASE,
        },
        _ if (ROOM_MEMBERS_BASE + 1..=ROOM_MEMBERS_BASE + ROOM_KEY_MAX).contains(&start) => {
            R::Members {
                key: start - ROOM_MEMBERS_BASE,
            }
        }
        _ if (ROOM_BITS_BASE..ROOM_BITS_BASE + CHUNKS * (1 << BITS_PER_CHUNK)).contains(&start) => {
            let offset = start - ROOM_BITS_BASE;
            R::Bits {
                chunk: (offset >> BITS_PER_CHUNK) as u8,
                bits: offset & ((1 << BITS_PER_CHUNK) - 1),
            }
        }
        _ if start >= ROOM_CREATE_BASE => {
            let code = start - ROOM_CREATE_BASE;
            let min_side = MIN_SIDES.start() + (code % 3) as u8;
            let code = code / 3;
            let pin = code % 2 == 1;
            let rest = code / 2;
            let wait = WAITS.get((rest % 10) as usize).copied()?;
            let max_level = rest / 10 % 16;
            let size = rest / 160;
            if size > u32::from(SIZES.end() - SIZES.start()) {
                return None;
            }
            R::Create(RoomSpec {
                size: SIZES.start() + size as u8,
                max_level: max_level as u8,
                wait,
                pin,
                min_side,
            })
        }
        _ => return None,
    };
    Some(request)
}

/// Where a room is.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomPhase {
    /// Fewer than two who can play: nothing runs.
    Waiting = 0,
    /// Not sent since 2026-10-19 (the vote runs until the match starts);
    /// kept so the lobby movies of revision 24 and before read the wire.
    /// It was: the vote chose the map; the room's wait runs.
    Countdown = 1,
    /// Its match is being played; `secs` is how long it has been.
    InMatch = 2,
    /// The vote, open until the match starts: its 15 s and the room's wait
    /// together; `secs` is what is left.
    Vote = 3,
}

/// Why the asker is in no room, or what came of its last request: the
/// third field of a state that has no room (`S1;N;<code>`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomCode {
    /// Nothing went wrong: the asker is in no room.
    None = 0,
    // 1, "the server has no rooms now", went with `SRP_ROOMS`: rooms are
    // always on (the owner's of 2026-10-17).
    /// No room has that key.
    Unknown = 2,
    /// The room asks for a PIN, and the one given is not it (or none was).
    WrongPin = 3,
    /// The room is full.
    Full = 4,
    /// The asker is in a match, sent to one, or has a place in one.
    InMatch = 5,
    /// As many rooms are open as the server allows.
    TooMany = 6,
    /// The asker's game does not play in matches that take players.
    Outdated = 7,
    /// A room asked for with no map of the pool, or out of range.
    Bad = 8,
    /// A planned maintenance is 30 minutes away or less.
    Maintenance = 9,
    /// The owner took the asker out of this room.
    Kicked = 10,
    /// The asker is in its clan's team.
    ClanTeam = 11,
    /// The asker's equipment level is over the room's max level (at the
    /// door, or at its match's start: then it was taken out of the room).
    GearTooHigh = 12,
    /// The asker's kit could not play (no weapon, too little ammo) when its
    /// room's match started: it was taken out of the room.
    KitUnfit = 13,
}

/// A room in [`room_list`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomRow {
    pub key: u32,
    /// Its level: worked out from its players (the level its match would
    /// have now).
    pub level: u8,
    pub max_level: u8,
    pub size: u8,
    pub players: u8,
    pub phase: RoomPhase,
    pub secs: u32,
    /// The map being played, or the one ahead in the vote; 0: none.
    pub map_id: u8,
    pub pin: bool,
    /// The owner's name: the room's.
    pub name: String,
}

/// A card of the vote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub map_id: u8,
    pub votes: u8,
    /// The asker voted for it.
    pub mine: bool,
}

/// A member of the asker's room, in the order of the room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// The equipment level the game shows for it.
    pub gear: u8,
    pub owner: bool,
    pub me: bool,
    /// In the room's match now.
    pub in_match: bool,
    /// Whether it can play the room's next match as it is now: its active
    /// kit and its equipment level ([`Fit`]).
    pub fit: Fit,
    /// Its game is gone for now (its link silent): it keeps its place for
    /// about 90 s, shown grey (the owner's of 2026-10-18).
    pub away: bool,
    pub name: String,
}

/// A member's kit as its room's next match would take it. One that is not
/// [`Fit::Yes`] counts for no start and is taken out of the room when the
/// match starts without it (the owner's of 2026-10-17: a red mark in the
/// room until then).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Fit {
    #[default]
    Yes = 0,
    /// Its active kit cannot play: no weapon, or too little ammo.
    Kit = 1,
    /// Its equipment level is over the room's max level.
    Gear = 2,
}

/// The asker's room as [`room_state`] tells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomView {
    pub key: u32,
    /// Its level: worked out from its players.
    pub level: u8,
    /// The highest equipment level let in; 0: any.
    pub max_level: u8,
    pub size: u8,
    pub wait: u8,
    pub phase: RoomPhase,
    pub secs: u32,
    /// The asker owns it.
    pub owner: bool,
    /// The maps of its pool.
    pub pool: u16,
    /// Changes whenever a member comes or goes: a kick names a place of the
    /// list it was made from.
    pub serial: u32,
    pub pin: bool,
    pub cards: Vec<Card>,
    pub members: Vec<Member>,
    /// The map of its last match; 0: none yet.
    pub last_map: u8,
    /// Always 0 since 2026-10-19 (no map is fixed before the start); kept
    /// so the lobby movies of revision 24 and before read the wire.
    pub next_map: u8,
    /// The players it needs before its vote begins (2, 4 or 6).
    pub min_players: u8,
    /// A room with a lower max level the asker may join now, offered as a
    /// one-click switch.
    pub switch: Option<RoomSwitch>,
    /// The asker came while its room's match is played: seconds until it is
    /// put into it, unless it asks first ([`ROOM_JOIN_MATCH`]); none when it
    /// is in it, or the match takes nobody now.
    pub join_in: Option<u32>,
    /// The asker waits in its room while the match is played and the match
    /// takes nobody now: why (none with [`Self::join_in`]).
    pub join_hold: Option<JoinHold>,
}

/// Why a member waiting in its room cannot go into the room's match being
/// played now (the owner's of 2026-10-20: players are told why). The third
/// field of `J;-1;<why>` in [`room_state`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum JoinHold {
    /// Another player is on its way into the match: one at a time.
    Joining = 1,
    /// A game of the match is loading; joining opens when it has loaded,
    /// or within 30 s.
    Loading = 2,
    /// 30 s or less are left of it: the member plays the next match.
    LastSeconds = 3,
    /// Its side is full (the room's size a side), or the match takes no
    /// more places.
    Full = 4,
    /// A maintenance is within 30 min: nobody goes into a match.
    Maintenance = 5,
    /// The match is ending (its mode's rule, or its result).
    Ending = 6,
}

/// A room to switch to (`W` row of [`room_state`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomSwitch {
    pub key: u32,
    /// Its max level, never 0 (any).
    pub max_level: u8,
    pub players: u8,
    /// Players a side.
    pub size: u8,
    /// Its maker's name.
    pub name: String,
}

/// What the create window starts with ([`room_form`]): the settings of the
/// asker's last room made by hand, or the defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomForm {
    /// The settings are the asker's own (false: the defaults).
    pub saved: bool,
    /// One of [`SIZES`].
    pub size: u8,
    /// One of [`MIN_SIDES`].
    pub min_side: u8,
    /// 0 (any) to [`LEVEL_MAX`].
    pub max_level: u8,
    /// One of [`WAITS`].
    pub wait: u8,
    /// By the pool's entries ([`room_pool`]'s order): the entry's map and
    /// weather (its tile in the window) are in. An entry is in the pool made
    /// when its tile is in and its mode is not in [`Self::modes_off`].
    pub tiles_on: Vec<bool>,
    /// The modes left out (`maps::MapMode::mode`).
    pub modes_off: Vec<u8>,
}

/// A map a pool may hold, in [`room_pool`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolEntry {
    pub map_id: u8,
    /// Of its `maps_dict` entry (`maps::MapMode`).
    pub level: u8,
    pub weather: u8,
    pub mode: u8,
    /// One of the server's experimental maps (`SRP_ROOMS_EXTRA_MAPS`): not
    /// in a room's pool unless its creator adds it.
    pub extra: bool,
}

fn row(name: String) -> Ranked {
    Ranked {
        name,
        rating: 0,
        rating_before: 0,
    }
}

/// A name fit for a row: no `;`, at most [`ROW_NAME`] bytes; one longer is
/// cut and ends with "…" (the owner's of 2026-10-20: long names shown as
/// cut, not as other names).
fn row_name(name: &str) -> String {
    let name = name.replace(';', "_");
    if name.len() <= ROW_NAME {
        return name;
    }
    let mut cut = String::new();
    for c in name.chars() {
        if cut.len() + c.len_utf8() + '…'.len_utf8() > ROW_NAME {
            break;
        }
        cut.push(c);
    }
    cut.push('…');
    cut
}

fn rows_at(start: u32, names: Vec<String>) -> Result<ServerMessage, WireError> {
    let rows: Vec<(u32, Ranked)> = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| (start + index as u32, row(name)))
        .collect();
    let rows: Vec<(u32, &Ranked)> = rows.iter().map(|(place, row)| (*place, row)).collect();
    elo_rows(&rows)
}

/// `91` as the answer to `3B 00 <ROOMS_LIST> <count>`:
///
/// - row 0, the header, always there, `place = ROOMS_LIST`: `S1;L;<total>;
///   <my key>`, `my key` the asker's room's (0: none);
/// - row `i` from 1, `place = ROOMS_LIST + i`, a room: `<key>;<level>;<max level>;
///   <size>;<players>;<phase>;<secs>;<map_id>;<pin>;<name>`.
///
/// At most `count − 1` rooms and [`LIST_ROWS`]; `total` counts them all.
pub fn room_list(
    rooms: &[RoomRow],
    total: usize,
    my_key: u32,
    count: u8,
) -> Result<ServerMessage, WireError> {
    let most = usize::from(count).saturating_sub(1).min(LIST_ROWS);
    let mut names = vec![format!("{ROOMS_VERSION};L;{total};{my_key}")];
    names.extend(rooms.iter().take(most).map(|room| {
        format!(
            "{};{};{};{};{};{};{};{};{};{}",
            room.key,
            room.level,
            room.max_level,
            room.size,
            room.players,
            room.phase as u8,
            room.secs,
            room.map_id,
            u8::from(room.pin),
            row_name(&room.name)
        )
    }));
    rows_at(ROOMS_LIST, names)
}

/// `91` with the asker's room, the answer to its state and to every action
/// on rooms (leave, vote, kick, join, create), `place = ROOM_STATE + i`:
///
/// - in no room: one row, `S1;N;<code>;<queue key>;<place>` ([`RoomCode`];
///   the full room whose queue it waits in and its place there, 1 the
///   first; 0 and 0 when it waits in none);
/// - in one: `S1;R;<key>;<level>;<max level>;<size>;<wait>;<phase>;<secs>;<owner>;
///   <pool>;<members>;<serial>;<pin>;<next map>;<min players>`, then a row a card, `C;<i>;<map_id>;
///   <votes>;<mine>`, a row a member, `P;<i>;<gear>;<owner>;<me>;
///   <in match>;<fit>;<away>;<name>` (`fit`: [`Fit`]; `away` 1: its game is
///   gone for now), a row `W;<key>;<max level>;<players>;<size>;<name>` when
///   a room of a lower max level would take the asker now ([`RoomSwitch`]),
///   a row `J;<seconds>` when the asker waits to go into its room's match
///   being played ([`RoomView::join_in`]), or `J;-1;<why>` when it waits
///   and the match takes nobody now ([`JoinHold`]; a movie that reads only
///   the seconds takes -1 as no row), and `L;<map_id>`, the last match's
///   map (0: none).
pub fn room_state(
    room: Option<&RoomView>,
    code: RoomCode,
    queued: Option<(u32, usize)>,
) -> Result<ServerMessage, WireError> {
    let Some(room) = room else {
        let (key, place) = queued.unwrap_or((0, 0));
        return rows_at(
            ROOM_STATE,
            vec![format!("{ROOMS_VERSION};N;{};{key};{place}", code as u8)],
        );
    };
    at_most("room cards", room.cards.len(), CARDS)?;
    at_most("room members", room.members.len(), MEMBERS_MAX)?;
    let mut names = vec![format!(
        "{ROOMS_VERSION};R;{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
        room.key,
        room.level,
        room.max_level,
        room.size,
        room.wait,
        room.phase as u8,
        room.secs,
        u8::from(room.owner),
        room.pool,
        room.members.len(),
        room.serial,
        u8::from(room.pin),
        room.next_map,
        room.min_players
    )];
    names.extend(room.cards.iter().enumerate().map(|(index, card)| {
        format!(
            "C;{index};{};{};{}",
            card.map_id,
            card.votes,
            u8::from(card.mine)
        )
    }));
    names.extend(room.members.iter().enumerate().map(|(index, member)| {
        format!(
            "P;{index};{};{};{};{};{};{};{}",
            member.gear,
            u8::from(member.owner),
            u8::from(member.me),
            u8::from(member.in_match),
            member.fit as u8,
            u8::from(member.away),
            row_name(&member.name)
        )
    }));
    if let Some(secs) = room.join_in {
        names.push(format!("J;{secs}"));
    } else if let Some(why) = room.join_hold {
        names.push(format!("J;-1;{}", why as u8));
    }
    if let Some(switch) = &room.switch {
        names.push(format!(
            "W;{};{};{};{};{}",
            switch.key,
            switch.max_level,
            switch.players,
            switch.size,
            row_name(&switch.name)
        ));
    }
    names.push(format!("L;{}", room.last_map));
    rows_at(ROOM_STATE, names)
}

/// A member of a room as the Rooms list's side panel shows it
/// ([`room_members`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListMember {
    pub member: Member,
    /// The team of its place in the room's match being played, 0 or 1;
    /// `None` when it has none.
    pub side: Option<u8>,
    /// Its account: a right-click on it asks for its kits by it
    /// (`3D 00 <GEAR_BY_ACCOUNT> <account>`).
    pub account_id: u32,
}

/// `91` as the answer to `3B 00 <ROOM_MEMBERS_BASE + key> 1`, `place =
/// ROOM_MEMBERS_BASE + i`, to anyone (the room chosen in the Rooms list; a
/// room with a PIN too: names are no secret): `S1;G;<key>;<members>`, then
/// two rows a member in the order of the room: `P;<i>;<gear>;<owner>;<me>;
/// <in match>;<fit>;<away>;<side>;<account>` (the state's fields; `side` 1
/// or 2, the team of its place in the room's match being played, 0 when it
/// has none), and its whole name alone (up to the 63 bytes a row holds, `;`
/// and all: a name is never cut here). A room gone: `S1;G;<key>;0`, no
/// member.
pub fn room_members(key: u32, members: &[ListMember]) -> Result<ServerMessage, WireError> {
    at_most("room members", members.len(), MEMBERS_MAX)?;
    rows_at(ROOM_MEMBERS_BASE, members_names(key, members))
}

fn members_names(key: u32, members: &[ListMember]) -> Vec<String> {
    let mut names = vec![format!("{ROOMS_VERSION};G;{key};{}", members.len())];
    for (index, seen) in members.iter().enumerate() {
        let member = &seen.member;
        names.push(format!(
            "P;{index};{};{};{};{};{};{};{};{}",
            member.gear,
            u8::from(member.owner),
            u8::from(member.me),
            u8::from(member.in_match),
            member.fit as u8,
            u8::from(member.away),
            seen.side.map_or(0, |team| team.min(1) + 1),
            seen.account_id
        ));
        names.push(member.name.clone());
    }
    names
}

/// `91` as the answer to `3B 00 <ROOM_POOL_BASE + page> <count>`, `place =
/// that start + i`: `S1;M;<total>;<page>`, then a row an entry of that
/// page, `<index>;<map_id>;<level>;<weather>;<mode>;<extra>`. An entry's
/// index is the bit a pool names it by ([`bits_start`]).
pub fn room_pool(entries: &[PoolEntry], page: u8) -> Result<ServerMessage, WireError> {
    let first = usize::from(page) * POOL_PAGE_ROWS;
    let mut names = vec![format!("{ROOMS_VERSION};M;{};{page}", entries.len())];
    names.extend(
        entries
            .iter()
            .enumerate()
            .skip(first)
            .take(POOL_PAGE_ROWS)
            .map(|(index, entry)| {
                format!(
                    "{index};{};{};{};{};{}",
                    entry.map_id,
                    entry.level,
                    entry.weather,
                    entry.mode,
                    u8::from(entry.extra)
                )
            }),
    );
    rows_at(ROOM_POOL_BASE + u32::from(page), names)
}

/// `91` as the answer to `3B 00 <ROOM_FORM> 1`, `place = ROOM_FORM + i`:
/// `S1;F;<saved>;<size>;<min side>;<max level>;<wait>;<modes off>` (`saved`
/// 1: the asker's own settings, 0: the defaults; `modes off` the modes left
/// out, `,` between them, empty for none), then a row a chunk of 20 entries
/// of the pool, `E;<chunk>;<bits>`: entry `chunk × 20 + b`'s tile is in by
/// bit `b` ([`RoomForm::tiles_on`]), every chunk the pool has (one at least).
pub fn room_form(form: &RoomForm) -> Result<ServerMessage, WireError> {
    at_most("room form entries", form.tiles_on.len(), POOL_BITS)?;
    rows_at(ROOM_FORM, form_names(form))
}

fn form_names(form: &RoomForm) -> Vec<String> {
    let modes: Vec<String> = form.modes_off.iter().map(u8::to_string).collect();
    let mut names = vec![format!(
        "{ROOMS_VERSION};F;{};{};{};{};{};{}",
        u8::from(form.saved),
        form.size,
        form.min_side,
        form.max_level,
        form.wait,
        modes.join(",")
    )];
    let chunk = BITS_PER_CHUNK as usize;
    let chunks = form.tiles_on.len().div_ceil(chunk).max(1);
    names.extend((0..chunks).map(|index| {
        let bits = form
            .tiles_on
            .iter()
            .skip(index * chunk)
            .take(chunk)
            .enumerate()
            .filter(|&(_, &on)| on)
            .fold(0_u32, |bits, (b, _)| bits | 1 << b);
        format!("E;{index};{bits}")
    }));
    names
}

/// `91` as the answer to a bits request: `S1;B;<chunk>;<entries in>`, at
/// [`ROOM_BITS_BASE`].
pub fn bits_answer(chunk: u8, entries: u32) -> Result<ServerMessage, WireError> {
    rows_at(
        ROOM_BITS_BASE,
        vec![format!("{ROOMS_VERSION};B;{chunk};{entries}")],
    )
}

/// `91` as the answer to a PIN request: `S1;K;1`, at [`ROOM_PIN_BASE`].
pub fn pin_answer() -> Result<ServerMessage, WireError> {
    rows_at(ROOM_PIN_BASE, vec![format!("{ROOMS_VERSION};K;1")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_start_is_one_request_and_inside_an_int() {
        const {
            assert!(ROOMS_FIRST > super::super::PVB_CHOOSE_LAST);
            assert!(ROOMS_LAST <= i32::MAX as u32);
        }
        assert_eq!(
            room_request(ROOMS_LIST, 31),
            Some(RoomRequest::List { count: 31 })
        );
        assert_eq!(room_request(ROOM_STATE, 1), Some(RoomRequest::State));
        assert_eq!(room_request(ROOM_LEAVE, 1), Some(RoomRequest::Leave));
        assert_eq!(room_request(ROOM_PLAY, 1), Some(RoomRequest::Play));
        assert_eq!(
            room_request(ROOM_VOTE_BASE + 2, 1),
            Some(RoomRequest::Vote { card: 2 })
        );
        // Play and leaving a queue come right after the three cards; then
        // nothing up to the pool.
        assert_eq!(ROOM_VOTE_BASE + 3, ROOM_PLAY);
        assert_eq!(ROOM_PLAY + 1, ROOM_UNQUEUE);
        assert_eq!(ROOM_UNQUEUE + 1, ROOM_JOIN_MATCH);
        assert_eq!(
            room_request(ROOM_JOIN_MATCH, 1),
            Some(RoomRequest::JoinMatch)
        );
        // The create window's settings, then the pool.
        assert_eq!(ROOM_JOIN_MATCH + 1, ROOM_FORM);
        assert_eq!(room_request(ROOM_FORM, 1), Some(RoomRequest::Form));
        assert_eq!(ROOM_FORM + 1, ROOM_POOL_BASE);
        assert_eq!(
            room_request(ROOM_POOL_BASE + 15, 1),
            Some(RoomRequest::Pool { page: 15 })
        );
        assert_eq!(
            room_request(kick_start(19, 1000), 1),
            Some(RoomRequest::Kick {
                place: 19,
                check: 1000 % KICK_CHECKS
            })
        );
        // The kicks end where the PINs begin.
        assert_eq!(kick_start(19, KICK_CHECKS - 1) + 1, ROOM_PIN_BASE);
        assert_eq!(
            room_request(ROOM_PIN_BASE, 1),
            Some(RoomRequest::Pin { pin: 0 })
        );
        assert_eq!(
            room_request(ROOM_PIN_BASE + 9999, 1),
            Some(RoomRequest::Pin { pin: 9999 })
        );
        assert_eq!(room_request(ROOM_JOIN_BASE, 1), None);
        assert_eq!(
            room_request(ROOM_JOIN_BASE + ROOM_KEY_MAX, 1),
            Some(RoomRequest::Join { key: ROOM_KEY_MAX })
        );
        assert_eq!(room_request(ROOM_UNQUEUE, 1), Some(RoomRequest::Unqueue));
        assert_eq!(
            room_request(ROOM_QUEUE_BASE + 7, 1),
            Some(RoomRequest::Queue { key: 7 })
        );
        assert_eq!(room_request(ROOM_QUEUE_BASE, 1), None);
        // Who is in a room: after the queues, below the pool's bits.
        assert_eq!(ROOM_QUEUE_BASE + ROOM_KEY_MAX + 1, ROOM_MEMBERS_BASE);
        const { assert!(ROOM_MEMBERS_BASE + ROOM_KEY_MAX < ROOM_BITS_BASE) };
        assert_eq!(room_request(ROOM_MEMBERS_BASE, 1), None);
        assert_eq!(
            room_request(ROOM_MEMBERS_BASE + ROOM_KEY_MAX, 1),
            Some(RoomRequest::Members { key: ROOM_KEY_MAX })
        );
        assert_eq!(
            room_request(bits_start(7, 0xF_FFFF), 1),
            Some(RoomRequest::Bits {
                chunk: 7,
                bits: 0xF_FFFF
            })
        );
        assert_eq!(room_request(ROOMS_LAST, 1), None);
        assert_eq!(room_request(ROOMS_FIRST - 1, 1), None);
    }

    #[test]
    fn a_create_code_carries_its_spec_and_back() {
        for size in SIZES {
            for max_level in [0, 5, LEVEL_MAX] {
                for wait in WAITS {
                    for pin in [false, true] {
                        for min_side in MIN_SIDES {
                            let spec = RoomSpec {
                                size,
                                max_level,
                                wait,
                                pin,
                                min_side,
                            };
                            let start = create_start(spec);
                            assert!(start <= ROOMS_LAST);
                            assert_eq!(room_request(start, 1), Some(RoomRequest::Create(spec)));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_form_says_its_settings_and_its_tiles_by_chunk() {
        let mut tiles_on = vec![false; 23];
        for index in [0, 2, 19, 20, 22] {
            tiles_on[index] = true;
        }
        let form = RoomForm {
            saved: true,
            size: 9,
            min_side: 2,
            max_level: 7,
            wait: 45,
            tiles_on,
            modes_off: vec![3, 4],
        };
        assert_eq!(
            form_names(&form),
            ["S1;F;1;9;2;7;45;3,4", "E;0;524293", "E;1;5"]
        );
        assert!(room_form(&form).is_ok());
        // No pool: one chunk all the same; no mode out: an empty field.
        let form = RoomForm {
            saved: false,
            tiles_on: Vec::new(),
            modes_off: Vec::new(),
            ..form
        };
        assert_eq!(form_names(&form), ["S1;F;0;9;2;7;45;", "E;0;0"]);
        // More entries than a create can name: refused.
        let form = RoomForm {
            tiles_on: vec![true; POOL_BITS + 1],
            ..form
        };
        assert!(room_form(&form).is_err());
    }

    #[test]
    fn a_member_list_gives_each_side_account_and_whole_name() {
        let member = Member {
            gear: 7,
            owner: true,
            me: false,
            in_match: true,
            fit: Fit::Gear,
            away: true,
            name: "a;b".into(),
        };
        let other = Member {
            owner: false,
            in_match: false,
            fit: Fit::Yes,
            away: false,
            name: "Bob".into(),
            ..member.clone()
        };
        let seen = |member: &Member, side, account_id| ListMember {
            member: member.clone(),
            side,
            account_id,
        };
        assert_eq!(
            members_names(
                42,
                &[
                    seen(&member, Some(1), 5),
                    seen(&other, None, 6),
                    seen(&other, Some(0), 7)
                ]
            ),
            [
                "S1;G;42;3",
                "P;0;7;1;0;1;2;1;2;5",
                "a;b",
                "P;1;7;0;0;0;0;0;0;6",
                "Bob",
                "P;2;7;0;0;0;0;0;1;7",
                "Bob"
            ]
        );
        assert_eq!(members_names(42, &[]), ["S1;G;42;0"]);
        assert!(room_members(42, &vec![seen(&member, None, 1); MEMBERS_MAX + 1]).is_err());
    }

    /// The longest names the game takes (63 bytes: wide Latin, Cyrillic,
    /// mixed) come whole, and the widest member row fits a row.
    #[test]
    fn a_member_list_never_cuts_a_name() {
        let names = [
            "W".repeat(63),
            format!("{}W", "Ш".repeat(31)),
            format!("Ж{}", "m".repeat(61)),
        ];
        let members: Vec<ListMember> = (0..MEMBERS_MAX)
            .map(|i| ListMember {
                member: Member {
                    gear: 15,
                    owner: true,
                    me: true,
                    in_match: true,
                    fit: Fit::Gear,
                    away: true,
                    name: names[i % names.len()].clone(),
                },
                side: Some(1),
                account_id: i32::MAX as u32,
            })
            .collect();
        let rows = members_names(99_999, &members);
        assert_eq!(rows.len(), 1 + 2 * MEMBERS_MAX);
        for row in &rows {
            assert!(row.len() <= crate::v026g::lobby::limits::NAME, "{row}");
        }
        for (i, name) in rows[1..].chunks(2).map(|pair| &pair[1]).enumerate() {
            assert_eq!(name, &names[i % names.len()]);
            assert_eq!(name.len(), 63);
        }
        let message = room_members(99_999, &members).unwrap();
        let mut client = crate::v026g::lobby::client_model::Client::signed_in();
        match client.receive(&message).unwrap() {
            crate::v026g::lobby::client_model::Decoded::EloList(decoded) => {
                assert_eq!(decoded.len(), rows.len());
                for (decoded, row) in decoded.iter().zip(&rows) {
                    assert_eq!(&decoded.0, row);
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_room_members_kits_are_asked_by_place_and_check() {
        assert_eq!(
            member_gear_place(member_gear(19, 1234)),
            Some((19, 1234 % KICK_CHECKS))
        );
        assert_eq!(member_gear_place(member_gear(0, 0)), Some((0, 0)));
        assert_eq!(member_gear_place(MEMBERS_MAX as u32 * KICK_CHECKS), None);
        assert_eq!(member_gear_place(u32::MAX), None);
    }

    #[test]
    fn a_name_loses_its_separators_and_its_tail() {
        assert_eq!(row_name("a;b"), "a_b");
        // Cut, it says so.
        let cut = row_name(&"x".repeat(40));
        assert_eq!(cut, format!("{}…", "x".repeat(ROW_NAME - 3)));
        assert_eq!(cut.len(), ROW_NAME);
        assert_eq!(row_name(&"x".repeat(ROW_NAME)), "x".repeat(ROW_NAME));
        assert_eq!(row_name("Сич").len(), 6);
        let wide = row_name(&"Ш".repeat(20));
        assert_eq!(wide, format!("{}…", "Ш".repeat(14)));
        assert!(wide.len() <= ROW_NAME);
    }
}
