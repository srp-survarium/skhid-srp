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

use std::ops::RangeInclusive;
use std::sync::OnceLock;
use std::time::Duration;

use binary_config_parser::tree::ConfigValue;

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
/// The rooms the asker's friends are members of ([`friend_rooms`]): a
/// friend's right-click menu offers "Join room" by it (the owner's of
/// 2026-10-20).
pub const ROOM_FRIENDS: u32 = ROOMS_FIRST + 40;
/// The owner of a private room (a PIN) starts its match now, with whoever
/// is in (the owner's of 2026-10-26: no countdown in a private room).
/// Nothing is done for anyone else, an open room, or a room playing its
/// match. Answered with the state.
pub const ROOM_START: u32 = ROOMS_FIRST + 41;
/// `ROOM_MOVE_BASE + place + 20 × check` ([`move_start`]): the owner of a
/// private room moves the member at that place of the state's list (itself
/// too) to the other team (the owner's of 2026-10-26: "a button to move the
/// player into any team"); `check` as a kick's. Nothing is done for anyone
/// else, an open room, a room playing its match, or onto a team of 10.
/// Answered with the state.
pub const ROOM_MOVE_BASE: u32 = ROOMS_FIRST + 20_000;
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
/// `ROOM_OFFER_BASE + key`, key 1 to [`ROOM_KEY_MAX`]: whether joining that
/// room offers one of the asker's kit presets ([`room_offer`]); key 0: the
/// room Play would take now. Asked before the join (`presets.md`, since
/// 2026-10-23; a movie before it never asks).
pub const ROOM_OFFER_BASE: u32 = ROOMS_FIRST + 400_000;
/// `ROOM_PRESET_BASE + (preset − 1) × 100 000 + key`, preset 1 to
/// [`PRESET_COUNT`], key 1 to [`ROOM_KEY_MAX`]: put that preset on the kits,
/// then join that room ("Switch and enter"); key 0: then Play. Answered with
/// the state.
pub const ROOM_PRESET_BASE: u32 = ROOMS_FIRST + 500_000;
/// An account's kit presets.
pub const PRESET_COUNT: u8 = 5;
/// Leave the queue the asker waits in. Answered with the state.
pub const ROOM_UNQUEUE: u32 = ROOMS_FIRST + 7;
/// Go into the asker's room's match being played now, without waiting for
/// [`RoomView::join_in`]. Answered with the state.
pub const ROOM_JOIN_MATCH: u32 = ROOMS_FIRST + 8;
/// What the asker's create window starts with: the settings of the last
/// room it made by hand, fitted to the pool and the rules now
/// ([`room_form`]; the owner's of 2026-10-20).
pub const ROOM_FORM: u32 = ROOMS_FIRST + 9;
/// The asker's room chat: its last lines (20, `session::v026g::rooms`) as
/// chat lines on [`super::channel::ROOM`], and from now on the room's new ones;
/// the movie asks when it puts up its "Room Chat" tab (`social.md` §9.8).
/// Answered with the state.
pub const ROOM_CHAT: u32 = ROOMS_FIRST + 30;
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
/// The most rows of [`friend_rooms`] after its header: all a `91` holds
/// (a friend list has 200 at most).
pub const FRIEND_ROWS: usize = u8::MAX as usize - 1;
/// The most bytes of a name in a row: a player's name is at most this long
/// in the game (`limits::NAME` is the buffer's).
const ROW_NAME: usize = 32;

/// The rooms' settings in the game's files, made by
/// `scripts/content-edit/rooms.tsv` (its header says each key); the lobby
/// movie reads the same file through exe patch 94.
pub const RULES_PATH: &str = "gameplay/srp/rooms.options";

/// The rooms' and Player vs Bots' settings ([`RULES_PATH`]): [`rules`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomRules {
    /// Each side's players a room may have.
    pub sizes: RangeInclusive<u8>,
    /// The players a side a room may need before its vote begins: 1 v 1 up.
    pub min_sides: RangeInclusive<u8>,
    /// The highest max level a room may have: the highest equipment level.
    pub level_most: u8,
    /// The waits before a match a room may have, seconds, ascending.
    pub waits: Vec<u8>,
    /// The create window's settings before a player has made a room, and
    /// the room Play makes.
    pub default: RoomSpecDefault,
    /// The vote's own time before each match, whatever the room's wait.
    pub vote: Duration,
    /// What the vote after a match has more: its players come back from
    /// the results screen first.
    pub after_match: Duration,
    /// How long a member who comes while its room's match is played sees
    /// the room before it is put into the match, unless it asks first.
    pub late_join_wait: Duration,
    /// This many ready Play users waiting in one equipment bracket open
    /// another room.
    pub scale_players: usize,
    /// The most rooms at once.
    pub rooms_most: usize,
    /// With no countdown this long after a match's first entry (match ms),
    /// it starts with who is in.
    pub match_start_wait_ms: u32,
    pub pvb: PvbRules,
}

/// What the create window starts with, and Play makes a room with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomSpecDefault {
    pub size: u8,
    pub min_side: u8,
    /// 0: any.
    pub max_level: u8,
    pub wait: u8,
}

/// "Player vs Bots" in the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PvbRules {
    /// The team sizes offered, each side's players.
    pub sizes: Vec<u8>,
    /// The one the window shows first.
    pub default_size: u8,
    /// The highest equipment level let in (the bots wear the player's).
    pub max_gear: u8,
}

impl RoomRules {
    /// The settings of `config` ([`RULES_PATH`]); a key missing, or a value
    /// out of what the rooms can take, is an error naming it.
    pub fn parse(config: &[u8]) -> Result<Self, String> {
        let root =
            ConfigValue::parse(config).map_err(|cause| format!("{RULES_PATH}: {}", cause.0))?;
        let at = |path: &str| -> Result<&ConfigValue, String> {
            path.split('/').try_fold(&root, |table, key| {
                table
                    .get(key)
                    .ok_or_else(|| format!("{RULES_PATH}: no `{path}`"))
            })
        };
        let int = |path: &str| -> Result<i64, String> {
            match at(path)? {
                ConfigValue::Integer(n) => Ok(i64::from(*n)),
                other => Err(format!(
                    "{RULES_PATH}: `{path}` is {other:?}, not a whole number"
                )),
            }
        };
        fn fit<T: TryFrom<i64>>(path: &str, n: i64) -> Result<T, String> {
            T::try_from(n).map_err(|_| format!("{RULES_PATH}: `{path}` is {n}, out of range"))
        }
        let byte = |path: &str| int(path).and_then(|n| fit::<u8>(path, n));
        let bytes = |path: &str| -> Result<Vec<u8>, String> {
            match at(path)? {
                ConfigValue::Indexed(items) => items
                    .iter()
                    .map(|item| match item {
                        ConfigValue::Integer(n) => fit(path, i64::from(*n)),
                        other => Err(format!("{RULES_PATH}: `{path}` holds {other:?}")),
                    })
                    .collect(),
                other => Err(format!("{RULES_PATH}: `{path}` is {other:?}, not a list")),
            }
        };
        let seconds = |path: &str| {
            int(path)
                .and_then(|n| fit::<u64>(path, n))
                .map(Duration::from_secs)
        };
        let rules = RoomRules {
            sizes: byte("size_least")?..=byte("size_most")?,
            min_sides: 1..=byte("min_side_most")?,
            level_most: byte("level_most")?,
            waits: bytes("waits")?,
            default: RoomSpecDefault {
                size: byte("default_size")?,
                min_side: byte("default_min_side")?,
                max_level: byte("default_max_level")?,
                wait: byte("default_wait")?,
            },
            vote: seconds("vote_s")?,
            after_match: seconds("after_match_s")?,
            late_join_wait: seconds("late_join_wait_s")?,
            scale_players: int("scale_players").and_then(|n| fit("scale_players", n))?,
            rooms_most: int("rooms_most").and_then(|n| fit("rooms_most", n))?,
            match_start_wait_ms: int("match_start_wait_ms")
                .and_then(|n| fit("match_start_wait_ms", n))?,
            pvb: PvbRules {
                sizes: bytes("pvb/sizes")?,
                default_size: byte("pvb/default_size")?,
                max_gear: byte("pvb/max_gear")?,
            },
        };
        rules
            .check()
            .map_err(|why| format!("{RULES_PATH}: {why}"))?;
        Ok(rules)
    }

    /// What the rooms' code relies on: the ranges not empty and within a
    /// match's sides, the defaults within them, and the largest create code
    /// within the rooms' starts.
    fn check(&self) -> Result<(), String> {
        let side_most = (MEMBERS_MAX / 2) as u8;
        let default = self.default;
        let problems = [
            (
                self.sizes.is_empty() || *self.sizes.start() == 0 || *self.sizes.end() > side_most,
                format!("sizes {:?}: 1 to {side_most} a side", self.sizes),
            ),
            (
                self.min_sides.is_empty() || self.min_sides.end() > self.sizes.start(),
                format!("min sides {:?}: 1 to the least size", self.min_sides),
            ),
            (
                self.waits.is_empty() || !self.waits.is_sorted() || self.waits.contains(&0),
                format!("waits {:?}: some seconds, ascending", self.waits),
            ),
            (
                !self.sizes.contains(&default.size)
                    || !self.min_sides.contains(&default.min_side)
                    || default.max_level > self.level_most
                    || !self.waits.contains(&default.wait),
                format!("the defaults {default:?} are not the rooms'"),
            ),
            (self.rooms_most == 0, "rooms_most 0: no room".into()),
            (
                self.pvb.sizes.is_empty()
                    || self
                        .pvb
                        .sizes
                        .iter()
                        .any(|size| !(1..=side_most).contains(size))
                    || !self.pvb.sizes.contains(&self.pvb.default_size),
                format!(
                    "pvb sizes {:?} (default {}): 1 to {side_most} a side",
                    self.pvb.sizes, self.pvb.default_size
                ),
            ),
            (
                self.pvb.max_gear > self.level_most,
                format!(
                    "pvb max gear {} over level_most {}",
                    self.pvb.max_gear, self.level_most
                ),
            ),
        ];
        if let Some((_, why)) = problems.into_iter().find(|(wrong, _)| *wrong) {
            return Err(why);
        }
        let most = RoomSpec {
            size: *self.sizes.end(),
            max_level: self.level_most,
            wait: *self.waits.last().expect("checked"),
            pin: true,
            min_side: *self.min_sides.end(),
        };
        if self.create_code(most) > ROOMS_LAST - ROOM_CREATE_BASE {
            return Err(format!("{most:?}'s create code is past the rooms' starts"));
        }
        Ok(())
    }

    /// A create's code (`create_start`): `((((size − least size) × levels
    /// + max level) × waits + wait index) × 2 + PIN) × min sides + (min side − 1)`,
    /// `levels` 0 to [`Self::level_most`], `waits` and `min sides`
    /// their counts (the lobby movie's RoomsCreate counts the same from the
    /// same file). A wait none of [`Self::waits`] is the default's.
    fn create_code(&self, spec: RoomSpec) -> u32 {
        let index = |wait| self.waits.iter().position(|&w| w == wait);
        let wait = index(spec.wait)
            .or_else(|| index(self.default.wait))
            .unwrap_or(0) as u32;
        let size =
            u32::from(spec.size.clamp(*self.sizes.start(), *self.sizes.end()) - self.sizes.start());
        let max_level = u32::from(spec.max_level.min(self.level_most));
        let min_side = u32::from(
            spec.min_side
                .clamp(*self.min_sides.start(), *self.min_sides.end())
                - self.min_sides.start(),
        );
        (((size * self.levels() + max_level) * self.waits.len() as u32 + wait) * 2
            + u32::from(spec.pin))
            * self.min_side_count()
            + min_side
    }

    /// The spec of a create's code, if it is one.
    fn create_spec(&self, code: u32) -> Option<RoomSpec> {
        let min_side = self.min_sides.start() + (code % self.min_side_count()) as u8;
        let code = code / self.min_side_count();
        let pin = code % 2 == 1;
        let rest = code / 2;
        let wait = self.waits[(rest % self.waits.len() as u32) as usize];
        let rest = rest / self.waits.len() as u32;
        let max_level = (rest % self.levels()) as u8;
        let size = rest / self.levels();
        if size > u32::from(self.sizes.end() - self.sizes.start()) {
            return None;
        }
        Some(RoomSpec {
            size: self.sizes.start() + size as u8,
            max_level,
            wait,
            pin,
            min_side,
        })
    }

    /// The max levels a room may have: 0 (any) to [`Self::level_most`].
    fn levels(&self) -> u32 {
        u32::from(self.level_most) + 1
    }

    fn min_side_count(&self) -> u32 {
        u32::from(self.min_sides.end() - self.min_sides.start()) + 1
    }
}

/// The rooms' settings of the game's files ([`super::super::game_data::shared`]),
/// read when first used; `srp` reads them at start (`tables::check`).
pub fn rules() -> &'static RoomRules {
    static RULES: OnceLock<RoomRules> = OnceLock::new();
    RULES.get_or_init(|| {
        RoomRules::parse(super::super::game_data::file(RULES_PATH))
            .unwrap_or_else(|why| panic!("{why}"))
    })
}

/// What a room is made with (the create window).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomSpec {
    /// Each side's players, one of [`RoomRules::sizes`].
    pub size: u8,
    /// The highest equipment level let in, 1 to [`RoomRules::level_most`]; 0: any. A
    /// private room has none: its window's is kept for the next open room.
    pub max_level: u8,
    /// Seconds from the second player to the match, one of [`RoomRules::waits`].
    pub wait: u8,
    /// Joining asks for a PIN: the one the creator gave with its PIN
    /// request just before ([`ROOM_PIN_BASE`]).
    pub pin: bool,
    /// The players a side it needs before its vote begins, one of
    /// [`RoomRules::min_sides`].
    pub min_side: u8,
}

/// A room request, by its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomRequest {
    List {
        count: u8,
    },
    State,
    Leave,
    Play,
    Vote {
        card: u8,
    },
    Pool {
        page: u8,
    },
    Kick {
        place: u8,
        check: u32,
    },
    Pin {
        pin: u16,
    },
    Join {
        key: u32,
    },
    Queue {
        key: u32,
    },
    Members {
        key: u32,
    },
    /// Key 0: the room Play would take.
    Offer {
        key: u32,
    },
    /// Preset 1 to [`PRESET_COUNT`]; key 0: Play.
    PresetJoin {
        preset: u8,
        key: u32,
    },
    Unqueue,
    JoinMatch,
    Form,
    Chat,
    Friends,
    /// [`ROOM_START`].
    Start,
    /// [`ROOM_MOVE_BASE`].
    Move {
        place: u8,
        check: u32,
    },
    Bits {
        chunk: u8,
        bits: u32,
    },
    Create(RoomSpec),
}

/// The start of a create (`RoomRules::create_code`), PIN 1 when joining asks for one. A room has no
/// level of its own to make: the server works it out from its players.
pub fn create_start(spec: RoomSpec) -> u32 {
    ROOM_CREATE_BASE + rules().create_code(spec)
}

/// The start of a kick of the member at `place` of a state of `serial`.
pub fn kick_start(place: u8, serial: u32) -> u32 {
    ROOM_KICK_BASE + u32::from(place) + MEMBERS_MAX as u32 * (serial % KICK_CHECKS)
}

/// The start of a move of the member at `place` of a state of `serial` to
/// the other team ([`ROOM_MOVE_BASE`]).
pub fn move_start(place: u8, serial: u32) -> u32 {
    ROOM_MOVE_BASE + u32::from(place) + MEMBERS_MAX as u32 * (serial % KICK_CHECKS)
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
        ROOM_CHAT => R::Chat,
        ROOM_FRIENDS => R::Friends,
        ROOM_START => R::Start,
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
        _ if (ROOM_MOVE_BASE..ROOM_MOVE_BASE + MEMBERS_MAX as u32 * KICK_CHECKS)
            .contains(&start) =>
        {
            R::Move {
                place: ((start - ROOM_MOVE_BASE) % MEMBERS_MAX as u32) as u8,
                check: (start - ROOM_MOVE_BASE) / MEMBERS_MAX as u32,
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
        _ if (ROOM_OFFER_BASE..=ROOM_OFFER_BASE + ROOM_KEY_MAX).contains(&start) => R::Offer {
            key: start - ROOM_OFFER_BASE,
        },
        _ if (ROOM_PRESET_BASE..ROOM_PRESET_BASE + u32::from(PRESET_COUNT) * 100_000)
            .contains(&start) =>
        {
            let offset = start - ROOM_PRESET_BASE;
            R::PresetJoin {
                preset: (offset / 100_000) as u8 + 1,
                key: offset % 100_000,
            }
        }
        _ if (ROOM_BITS_BASE..ROOM_BITS_BASE + CHUNKS * (1 << BITS_PER_CHUNK)).contains(&start) => {
            let offset = start - ROOM_BITS_BASE;
            R::Bits {
                chunk: (offset >> BITS_PER_CHUNK) as u8,
                bits: offset & ((1 << BITS_PER_CHUNK) - 1),
            }
        }
        _ if start >= ROOM_CREATE_BASE => R::Create(rules().create_spec(start - ROOM_CREATE_BASE)?),
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
    /// "Switch and enter": the preset could not be chosen (the asker is in a
    /// match or on its way to one, or a kit rule refused): nothing changed,
    /// no room joined (`presets.md`). An empty preset is no refusal: it
    /// starts empty.
    PresetRefused = 14,
    /// The kit a match would take can not play (no weapon, too little
    /// ammo): Play, Join, a queue and a friend's room refuse it at the door
    /// (`presets.md`, the owner's of 2026-10-23). The movie greys those by
    /// the same rule and says nothing of this code.
    KitCantPlay = 15,
    /// Public rooms are created by Play as demand grows, never by a client.
    PublicCreate = 16,
    /// A squad member that is not its commander goes into no room on its
    /// own: its commander's Play, Join or Queue takes it (the owner's of
    /// 2026-10-31, `squads.md`). The movie greys those by the same rule.
    SquadLeader = 17,
}

/// A room in [`room_list`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomRow {
    pub key: u32,
    /// Its level: worked out from its players (the level its match would
    /// have now).
    pub level: u8,
    /// The highest equipment level let in; 0: any, and a private room (it
    /// has none: the movie shows none).
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

/// A friend of the asker who is a member of a room, in [`friend_rooms`]:
/// what joining it from the friend's menu needs (as the list's Join: a PIN
/// asked first, Queue for a full room).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriendRoom {
    /// The friend's account, as its row of the friend list (`A3 07`) has it.
    pub account_id: u32,
    pub key: u32,
    pub players: u8,
    /// Players a side.
    pub size: u8,
    pub pin: bool,
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
    /// Its squad's mark in this room: the pairs whose two members are both
    /// in it numbered 1, 2, ... in the list's order ([`SQUAD_MARKS`] at
    /// most); `None` for one in no pair there (the owner's of 2026-10-31:
    /// squads seen in the room). The wire says none as 0.
    pub squad: Option<u8>,
    pub name: String,
}

/// The most squad marks a room shows: a room's 20 members are 10 pairs.
pub const SQUAD_MARKS: u8 = (MEMBERS_MAX / 2) as u8;

/// A squad mark as one character of the state's `S` row: `0` none, then
/// `1`..`9`, `a` (base 36).
fn squad_mark(squad: Option<u8>) -> char {
    char::from_digit(u32::from(squad_number(squad)), 36).unwrap_or('0')
}

/// A squad mark as the wire's number: 0 none.
fn squad_number(squad: Option<u8>) -> u8 {
    squad.map_or(0, |mark| mark.clamp(1, SQUAD_MARKS))
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
    /// The highest equipment level let in; 0: any, and a private room (it
    /// has none: the movie shows none).
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
    /// The players it needs before its vote begins (2, 4 or 6; 1 in a
    /// private room (a PIN): its owner starts its match with whoever is in).
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
    /// A private room's teams as its owner arranged them: each member's
    /// side (0, 1), in the order of [`Self::members`]; none for an open
    /// room (its sides are drawn at the start).
    pub teams: Option<Vec<u8>>,
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
    /// All lifetime roster places were used; leaving cannot free one.
    PlayerLimit = 7,
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
    /// One of [`RoomRules::sizes`].
    pub size: u8,
    /// One of [`RoomRules::min_sides`].
    pub min_side: u8,
    /// 0 (any) to [`RoomRules::level_most`].
    pub max_level: u8,
    /// One of [`RoomRules::waits`].
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
///   <pool>;<members>;<serial>;<pin>;<next map>;<min players>;<code>` (`code`
///   as above: why a join of another room, or a room made, left it in this
///   one), then a row a card, `C;<i>;<map_id>;
///   <votes>;<mine>`, a row a member, `P;<i>;<gear>;<owner>;<me>;
///   <in match>;<fit>;<away>;<name>` (`fit`: [`Fit`]; `away` 1: its game is
///   gone for now), in a private room a row `A;<sides>`, a digit a member
///   in the list's order, its team as the owner arranged it (0, 1;
///   [`RoomView::teams`]), a row `S;<marks>` when two members of one squad are
///   in it, a character a member in the list's order, its squad's mark
///   ([`Member::squad`]: `0` none, `1`..`9`, `a`), a row
///   `W;<key>;<max level>;<players>;<size>;<name>` when
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
        "{ROOMS_VERSION};R;{};{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
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
        room.min_players,
        code as u8
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
    if let Some(teams) = &room.teams {
        let sides: String = teams
            .iter()
            .map(|&side| if side == 0 { '0' } else { '1' })
            .collect();
        names.push(format!("A;{sides}"));
    }
    if room.members.iter().any(|member| member.squad.is_some()) {
        let marks: String = room
            .members
            .iter()
            .map(|member| squad_mark(member.squad))
            .collect();
        names.push(format!("S;{marks}"));
    }
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
/// <in match>;<fit>;<away>;<side>;<account>;<squad>` (the state's fields;
/// `side` 1 or 2, the team of its place in the room's match being played, 0
/// when it has none; `squad` its squad's mark, [`Member::squad`], 0 none),
/// and its whole name alone (up to the 63 bytes a row holds, `;`
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
            "P;{index};{};{};{};{};{};{};{};{};{}",
            member.gear,
            u8::from(member.owner),
            u8::from(member.me),
            u8::from(member.in_match),
            member.fit as u8,
            u8::from(member.away),
            seen.side.map_or(0, |team| team.min(1) + 1),
            seen.account_id,
            squad_number(member.squad)
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

/// `91` as the answer to `3B 00 <ROOM_FRIENDS> 1`, `place = ROOM_FRIENDS +
/// i`: `S1;Y;<count>`, then a row a friend of the asker who is a member of
/// a room (not one waiting in its queue; not of the asker's own room),
/// `<account id>;<key>;<players>;<size>;<pin>`. At most [`FRIEND_ROWS`].
pub fn friend_rooms(friends: &[FriendRoom]) -> Result<ServerMessage, WireError> {
    at_most("friends in rooms", friends.len(), FRIEND_ROWS)?;
    rows_at(ROOM_FRIENDS, friend_names(friends))
}

fn friend_names(friends: &[FriendRoom]) -> Vec<String> {
    let mut names = vec![format!("{ROOMS_VERSION};Y;{}", friends.len())];
    names.extend(friends.iter().map(|friend| {
        format!(
            "{};{};{};{};{}",
            friend.account_id,
            friend.key,
            friend.players,
            friend.size,
            u8::from(friend.pin)
        )
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

/// What [`room_offer`] says of a join (`presets.md`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferKind {
    /// No popup: join as it is (a room of any max level, no preset that
    /// fits higher, or no room).
    None = 0,
    /// The room allows more than the kits: a preset higher than the gear
    /// fits ("Enter anyway" or "Switch and enter").
    Higher = 1,
    /// The gear is over the room's max level and a preset fits ("Exit" or
    /// "Switch and enter").
    Fits = 2,
    /// The kit a match would take can not play, and another preset that
    /// can fits ("Exit" or "Switch and enter").
    Unfit = 3,
}

/// What the room of an offer is to the request ([`RoomOffer::about`]).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferRoom {
    /// The room the join goes into (for Play, the one it takes): "Enter
    /// anyway" or "Exit" beside "Switch and enter".
    Entered = 0,
    /// Play would queue in its own bracket; the room is of the other one,
    /// with a place for the preset offered (the owner's of 2026-10-30):
    /// "Queue" (Play as it is) beside "Switch and enter".
    InsteadOfQueue = 1,
}

/// What [`room_offer`] says of a join (or Play).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomOffer {
    /// The room (for Play, the one it would take, or the other bracket's
    /// offered instead of its queue); 0: none.
    pub key: u32,
    pub kind: OfferKind,
    /// 1 to [`PRESET_COUNT`] and its gear level; 0 and 0 for none.
    pub preset: u8,
    pub level: u8,
    /// The asker's gear now.
    pub gear: u8,
    /// The room's max level.
    pub max_level: u8,
    pub about: OfferRoom,
}

/// The answer to `3B 00 <ROOM_OFFER_BASE + asked>`, one row at that start:
/// `S1;O;<key>;<kind>;<preset>;<level>;<gear>;<max level>;<about>` (the
/// fields of [`RoomOffer`]; `about` since 2026-10-30, a movie before it
/// reads only the first eight).
pub fn room_offer(asked: u32, offer: &RoomOffer) -> Result<ServerMessage, WireError> {
    rows_at(
        ROOM_OFFER_BASE + asked.min(ROOM_KEY_MAX),
        vec![offer_name(offer)],
    )
}

fn offer_name(offer: &RoomOffer) -> String {
    format!(
        "{ROOMS_VERSION};O;{};{};{};{};{};{};{}",
        offer.key,
        offer.kind as u8,
        offer.preset,
        offer.level,
        offer.gear,
        offer.max_level,
        offer.about as u8
    )
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
        // The room chat past the pool's pages, before the kicks.
        const {
            assert!(ROOM_POOL_BASE + POOL_PAGES <= ROOM_CHAT);
            assert!(ROOM_CHAT < ROOM_KICK_BASE);
        }
        assert_eq!(room_request(ROOM_CHAT, 1), Some(RoomRequest::Chat));
        assert_eq!(room_request(ROOM_CHAT + 1, 1), None);
        // The friends' rooms: after the pool's last page, before the kicks.
        const { assert!(ROOM_POOL_BASE + POOL_PAGES <= ROOM_FRIENDS && ROOM_FRIENDS < ROOM_KICK_BASE) };
        assert_eq!(room_request(ROOM_FRIENDS, 1), Some(RoomRequest::Friends));
        assert_eq!(room_request(ROOM_FRIENDS - 1, 1), None);
        assert_eq!(ROOM_FRIENDS + 1, ROOM_START);
        assert_eq!(room_request(ROOM_START, 1), Some(RoomRequest::Start));
        assert_eq!(room_request(ROOM_START + 1, 1), None);
        // The moves: after the PINs, before the joins.
        const {
            assert!(ROOM_PIN_BASE + (PIN_MAX as u32) < ROOM_MOVE_BASE);
            assert!(ROOM_MOVE_BASE + MEMBERS_MAX as u32 * KICK_CHECKS <= ROOM_JOIN_BASE);
        }
        assert_eq!(
            room_request(move_start(19, 1000), 1),
            Some(RoomRequest::Move {
                place: 19,
                check: 1000 % KICK_CHECKS
            })
        );
        assert_eq!(
            room_request(move_start(0, 0), 1),
            Some(RoomRequest::Move { place: 0, check: 0 })
        );
        assert_eq!(room_request(move_start(19, KICK_CHECKS - 1) + 1, 1), None);
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
        let rules = rules();
        for size in rules.sizes.clone() {
            for max_level in [0, 5, rules.level_most] {
                for wait in rules.waits.iter().copied() {
                    for pin in [false, true] {
                        for min_side in rules.min_sides.clone() {
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
        // The codes of the revision-41 movie: 8 v 8, any level, 30 s, a PIN, from 1 v 1.
        let spec = RoomSpec {
            size: 8,
            max_level: 0,
            wait: 30,
            pin: true,
            min_side: 1,
        };
        assert_eq!(
            create_start(spec),
            ROOM_CREATE_BASE + ((2 * 16 * 10 + 3) * 2 + 1) * 3
        );
    }

    #[test]
    fn the_rules_name_a_missing_key_and_refuse_defaults_out_of_range() {
        use binary_config_parser::tree::ConfigValue as V;
        let config = |drop: &str, default_size: i32| {
            let mut entries: Vec<(String, V)> = [
                ("size_least", 6),
                ("size_most", 10),
                ("min_side_most", 3),
                ("level_most", 15),
                ("default_size", default_size),
                ("default_min_side", 1),
                ("default_max_level", 0),
                ("default_wait", 30),
                ("vote_s", 15),
                ("after_match_s", 10),
                ("late_join_wait_s", 15),
                ("scale_players", 6),
                ("rooms_most", 50),
                ("match_start_wait_ms", 30_000),
            ]
            .into_iter()
            .map(|(key, n)| (key.to_owned(), V::Integer(n)))
            .collect();
            entries.push((
                "waits".into(),
                V::Indexed(vec![V::Integer(15), V::Integer(30)]),
            ));
            entries.push((
                "pvb".into(),
                V::Named(vec![
                    ("sizes".into(), V::Indexed(vec![V::Integer(8)])),
                    ("default_size".into(), V::Integer(8)),
                    ("max_gear".into(), V::Integer(15)),
                ]),
            ));
            entries.retain(|(key, _)| key != drop);
            V::Named(entries).to_bytes().expect("a config")
        };
        let rules = RoomRules::parse(&config("", 8)).expect("rules");
        assert_eq!(
            (rules.sizes, rules.min_sides, rules.vote),
            (6..=10, 1..=3, Duration::from_secs(15))
        );
        assert_eq!(
            RoomRules::parse(&config("vote_s", 8)),
            Err(format!("{RULES_PATH}: no `vote_s`"))
        );
        assert!(
            RoomRules::parse(&config("", 11))
                .unwrap_err()
                .contains("the defaults")
        );
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
            squad: Some(2),
            name: "a;b".into(),
        };
        let other = Member {
            owner: false,
            in_match: false,
            fit: Fit::Yes,
            away: false,
            squad: None,
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
                "P;0;7;1;0;1;2;1;2;5;2",
                "a;b",
                "P;1;7;0;0;0;0;0;0;6;0",
                "Bob",
                "P;2;7;0;0;0;0;0;1;7;0",
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
                    squad: Some(SQUAD_MARKS),
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
    fn the_friends_rooms_say_how_to_join_each() {
        let friends = [
            FriendRoom {
                account_id: 4_000_000_001,
                key: 99_999,
                players: 20,
                size: 10,
                pin: true,
            },
            FriendRoom {
                account_id: 7,
                key: 12,
                players: 1,
                size: 8,
                pin: false,
            },
        ];
        assert_eq!(
            friend_names(&friends),
            ["S1;Y;2", "4000000001;99999;20;10;1", "7;12;1;8;0"]
        );
        assert_eq!(friend_names(&[]), ["S1;Y;0"]);
        assert!(friend_rooms(&vec![friends[1]; FRIEND_ROWS]).is_ok());
        assert!(friend_rooms(&vec![friends[1]; FRIEND_ROWS + 1]).is_err());
    }

    #[test]
    fn a_preset_offer_and_a_preset_join_have_starts_of_their_own() {
        const {
            assert!(ROOM_MEMBERS_BASE + ROOM_KEY_MAX < ROOM_OFFER_BASE);
            assert!(ROOM_OFFER_BASE + ROOM_KEY_MAX < ROOM_PRESET_BASE);
            assert!(ROOM_PRESET_BASE + PRESET_COUNT as u32 * 100_000 <= ROOM_BITS_BASE);
        }
        assert_eq!(
            room_request(ROOM_OFFER_BASE, 1),
            Some(RoomRequest::Offer { key: 0 })
        );
        assert_eq!(
            room_request(ROOM_OFFER_BASE + ROOM_KEY_MAX, 1),
            Some(RoomRequest::Offer { key: ROOM_KEY_MAX })
        );
        // The presets' joins right after.
        assert_eq!(ROOM_OFFER_BASE + ROOM_KEY_MAX + 1, ROOM_PRESET_BASE);
        assert_eq!(
            room_request(ROOM_PRESET_BASE, 1),
            Some(RoomRequest::PresetJoin { preset: 1, key: 0 })
        );
        assert_eq!(
            room_request(ROOM_PRESET_BASE + 4 * 100_000 + ROOM_KEY_MAX, 1),
            Some(RoomRequest::PresetJoin {
                preset: 5,
                key: ROOM_KEY_MAX
            })
        );
        // The pool's bits right after the fifth preset.
        assert_eq!(ROOM_PRESET_BASE + 5 * 100_000, ROOM_BITS_BASE);
        let offer = RoomOffer {
            key: 12,
            kind: OfferKind::Fits,
            preset: 3,
            level: 7,
            gear: 11,
            max_level: 8,
            about: OfferRoom::InsteadOfQueue,
        };
        assert_eq!(offer_name(&offer), "S1;O;12;2;3;7;11;8;1");
        assert!(room_offer(12, &offer).is_ok());
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
