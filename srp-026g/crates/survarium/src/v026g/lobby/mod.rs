//! The 0.26g lobby messages (`docs/notes/lobby.md`,
//! `lobby_requests.md`, `social.md`, `results.md`, `transport.md` §4.1).
//!
//! The lobby runs on the UDP transport the matches use, with its own table
//! of message classes. On the wire every lobby message is
//!
//! ```text
//! id  [x]  body
//! ```
//!
//! - **`x`** is one byte the transport owns, not the message: the *unique
//!   id* of a unique message, or the *order id* of a message that is unique
//!   and ordered ([`Delivery`]). Only 0x9F has none. Each is a counter of
//!   the connection (from 0, one for the unique channel and one for the
//!   ordered channel), so the writers here give `id` and `body` only and
//!   the transport (`crates/udp-transport`) fills `x` in. [`ServerMessage::wire`]
//!   shows the three together, for logs and tests.
//! - Every lobby message is **variable length**: the transport carries the
//!   length (the rest of a single datagram, or a prefix in a multi one).
//!   So a reader here gets exactly its body, and a body's last field may be
//!   "the rest".
//!
//! What is here:
//! - [`ServerId`]: the server→client ids the client takes. The ids it
//!   crashes on (0x81, 0x86, 0xA0, 0xA7; anything outside 0x80..=0xAA) have
//!   no value, so a [`ServerMessage`] can not carry one.
//! - the writers, one for each message: [`push`] (account, inventory,
//!   economy, acks), [`social`] (chat, friends, squad, clan, the small
//!   pushes) and [`results`] (the results screen, drops, the daily bonus);
//! - [`requests`]: the parser of every client→server message (0x2E..=0x50);
//! - [`client_model`]: a strict reader of the server's messages that
//!   follows the client's own reading rules and limits, for tests of
//!   whatever sends them.
//!
//! The client checks nothing it reads, so each writer keeps the limits of
//! the notes itself: a string is cut to the client's buffer, and a count
//! past the client's array is an error ([`WireError`]), never sent.

use std::fmt;

pub mod client_model;
pub mod client_rules;
pub mod lobby_pick;
pub mod presets;
pub mod push;
pub mod requests;
pub mod results;
pub mod rooms;
pub mod social;
pub mod tutorial_model;

#[cfg(test)]
mod tests;

pub use push::*;
pub use requests::{ClientMessage, ParseError, parse};
pub use results::*;
pub use social::*;
pub use lobby_pick::LobbyPick;

/// The first and last id the client's lobby table takes from the server
/// (`transport.md` §4.1: getter+4/+5 of the table object at 0xA07FA8).
pub const FIRST_SERVER_ID: u8 = 0x80;
pub const LAST_SERVER_ID: u8 = 0xAA;
/// The first and last id the client sends (getter+6/+7).
pub const FIRST_CLIENT_ID: u8 = 0x2E;
pub const LAST_CLIENT_ID: u8 = 0x50;

/// The ids inside 0x80..=0xAA that must never be sent:
/// - 0x81, 0xA0: the handler's jump table entry is NULL (`lobby.md` H1);
/// - 0x86, 0xA7: the transport's class table has no entry, so even asking
///   for the id's class is a jump to 0 (`transport.md` §4.1).
pub const FORBIDDEN_SERVER_IDS: [u8; 4] = [0x81, 0x86, 0xA0, 0xA7];

/// How the transport delivers a lobby message, which decides its `x` byte
/// (`transport.md` §4.1, the class byte per id at 0x26E2F0; PROVEN).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Reliable and unique (flags 0x20): `x` is the unique id. Not ordered
    /// against anything.
    Unique,
    /// Reliable, unique and ordered (flags 0x60): `x` is the order id, from
    /// 0, and the client holds a message until its predecessor came.
    Ordered,
    /// Sent once, no `x` byte (flags 0x1F): only 0x9F.
    Plain,
}

/// The server→client lobby ids the client accepts (`lobby.md` §1.3).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ServerId {
    SignedIn = 0x80,
    ConnectToMatch = 0x82,
    Permitted = 0x83,
    Denied = 0x84,
    ClientState = 0x85,
    Account = 0x87,
    SquadMemberAccount = 0x88,
    Inventory = 0x89,
    Prices = 0x8A,
    Money = 0x8B,
    Skills = 0x8C,
    ServicePrices = 0x8D,
    Reputations = 0x8E,
    BaseInfo = 0x8F,
    PlayerElo = 0x90,
    EloList = 0x91,
    Quests = 0x92,
    PlayerDetails = 0x93,
    Services = 0x94,
    Shop = 0x95,
    UnlockedFactions = 0x96,
    MatchStats = 0x97,
    Drop = 0x98,
    DailyBonus = 0x99,
    ClanInfo = 0x9A,
    CraftTables = 0x9B,
    ClanTeamLimits = 0x9C,
    Tutorial = 0x9D,
    VideoLink = 0x9E,
    OnlineStats = 0x9F,
    SquadInfo = 0xA1,
    ChatText = 0xA2,
    Friendship = 0xA3,
    Important = 0xA4,
    AdminText = 0xA5,
    AdminTextClear = 0xA6,
    PremiumStore = 0xA8,
    PremiumDiscount = 0xA9,
    SignedOut = 0xAA,
}

impl ServerId {
    /// Every id, in order.
    pub const ALL: [ServerId; 39] = [
        Self::SignedIn,
        Self::ConnectToMatch,
        Self::Permitted,
        Self::Denied,
        Self::ClientState,
        Self::Account,
        Self::SquadMemberAccount,
        Self::Inventory,
        Self::Prices,
        Self::Money,
        Self::Skills,
        Self::ServicePrices,
        Self::Reputations,
        Self::BaseInfo,
        Self::PlayerElo,
        Self::EloList,
        Self::Quests,
        Self::PlayerDetails,
        Self::Services,
        Self::Shop,
        Self::UnlockedFactions,
        Self::MatchStats,
        Self::Drop,
        Self::DailyBonus,
        Self::ClanInfo,
        Self::CraftTables,
        Self::ClanTeamLimits,
        Self::Tutorial,
        Self::VideoLink,
        Self::OnlineStats,
        Self::SquadInfo,
        Self::ChatText,
        Self::Friendship,
        Self::Important,
        Self::AdminText,
        Self::AdminTextClear,
        Self::PremiumStore,
        Self::PremiumDiscount,
        Self::SignedOut,
    ];

    /// The id for a byte, if the client accepts it.
    pub fn from_u8(id: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|&known| known as u8 == id)
    }

    /// The id's class in the client's lobby table.
    pub fn delivery(self) -> Delivery {
        match self {
            Self::SignedIn
            | Self::ConnectToMatch
            | Self::ClanInfo
            | Self::SquadInfo
            | Self::ChatText
            | Self::Important
            | Self::SignedOut => Delivery::Unique,
            Self::OnlineStats => Delivery::Plain,
            _ => Delivery::Ordered,
        }
    }
}

/// One server→client lobby message: an id the client accepts and its body
/// (what follows the transport's `x` byte).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerMessage {
    id: ServerId,
    body: Vec<u8>,
}

impl ServerMessage {
    /// A message from a body made elsewhere (a relay, a test). The id is
    /// still one the client accepts.
    pub fn from_parts(id: ServerId, body: Vec<u8>) -> Self {
        Self { id, body }
    }

    pub fn id(&self) -> ServerId {
        self.id
    }

    /// The id byte, as the transport's `send(id, payload)` takes it.
    pub fn id_byte(&self) -> u8 {
        self.id as u8
    }

    /// The payload: everything after the `x` byte.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }

    pub fn delivery(&self) -> Delivery {
        self.id.delivery()
    }

    /// The message as it lies in a datagram, without a length prefix:
    /// `id, x, body`, or `id, body` for the one id with no `x` byte. `x` is
    /// the transport's counter value; it is ignored for a [`Delivery::Plain`]
    /// id.
    pub fn wire(&self, x: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.body.len() + 2);
        out.push(self.id as u8);
        if self.delivery() != Delivery::Plain {
            out.push(x);
        }
        out.extend_from_slice(&self.body);
        out
    }
}

/// Why a writer refused: the message would overrun one of the client's
/// arrays or make it dereference nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireError {
    /// More entries than the client's array holds.
    TooMany {
        what: &'static str,
        count: usize,
        most: usize,
    },
    /// An item whose `dict_id` the client's dictionary lacks: its reader
    /// misparses the item and its UI dereferences NULL (`lobby.md` H5).
    UnknownItem(u16),
    /// A kit slot naming an item index past the account's item table.
    BadItemIndex { index: u16, items: usize },
    /// An item's modification (state clear) or defect (state set) that the
    /// client's table lacks: it reads the row it did not find (`lobby.md`
    /// H6).
    UnknownModification { id: u32, state: bool },
    /// A kit slot past the 23 a kit has.
    BadSlot(u8),
    /// A value the client's code can not take (a team past 1, an id out of
    /// a table, a count that must match another).
    BadValue(&'static str),
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany { what, count, most } => {
                write!(f, "{count} {what}: the client holds {most}")
            }
            Self::UnknownItem(dict_id) => write!(f, "no item with dict id {dict_id}"),
            Self::BadItemIndex { index, items } => {
                write!(f, "item index {index} in a table of {items}")
            }
            Self::UnknownModification { id, state: false } => {
                write!(f, "no modification {id}")
            }
            Self::UnknownModification { id, state: true } => write!(f, "no defect {id}"),
            Self::BadSlot(slot) => write!(f, "no kit slot {slot}"),
            Self::BadValue(what) => write!(f, "bad value: {what}"),
        }
    }
}

impl std::error::Error for WireError {}

fn at_most(what: &'static str, count: usize, most: usize) -> Result<(), WireError> {
    if count > most {
        return Err(WireError::TooMany { what, count, most });
    }
    Ok(())
}

/// The client's limits (`lobby.md` §2 and H4, H6, H8; `social.md`;
/// `results.md` §6). Each is the most that is safe to send.
pub mod limits {
    /// Kits ("profiles") of an account: a 4th overwrites the item table.
    pub const KITS: usize = 3;
    /// The account's item table: 69 = 3 kits of 23 slots.
    pub const ACCOUNT_ITEMS: usize = 69;
    /// The three lists that end the inventory (0x89): the client's stack
    /// buffers (LIKELY 24, 18 and 27 rows, from the frame offsets at
    /// 0x2704BD..0x27052D); one row more is the fatal "buffer overflow".
    pub const SPENT_AUTOBUY_AMMO: usize = 24;
    pub const SPENT_AUTOBUY_CONSUMABLES: usize = 18;
    pub const SPENT_REPAIR: usize = 27;
    /// Slots of a kit (`profile_slot_id` 0..=22).
    pub const KIT_SLOTS: usize = 23;
    /// Modifications and upgrades of an item (fatal asserts past them).
    pub const MODIFICATIONS: usize = 8;
    pub const UPGRADES: usize = 3;
    /// Names: 64-byte buffers.
    pub const NAME: usize = 63;
    /// The clan abbreviation of an account body (buffer +0x48..+0x56).
    pub const ACCOUNT_CLAN_TAG: usize = 14;
    /// The clan abbreviation in chat, friends and results (16-byte buffers).
    pub const CLAN_TAG: usize = 15;
    /// The 0x85 status text (128-byte buffer).
    pub const STATUS_TEXT: usize = 127;
    /// The 0x82 host (64-byte stack buffer).
    pub const HOST: usize = 63;
    /// Chat and important message bodies, and a denial's text (256-byte
    /// buffers).
    pub const TEXT: usize = 255;
    /// The 0x9E link and a clan's daily message, in UTF-16 units.
    pub const WIDE_TEXT: usize = 245;
    /// Squad members (`read_squad_info`: 3 slots of 0x50).
    pub const SQUAD_MEMBERS: usize = 3;
    /// Rows of the results screen.
    pub const RESULT_ROWS: usize = 20;
    /// Days of the daily bonus.
    pub const BONUS_DAYS: usize = 7;
    /// Clan ranks, members and clan-team members.
    pub const CLAN_RANKS: usize = 10;
    pub const CLAN_MEMBERS: usize = 105;
    pub const CLAN_TEAM_MEMBERS: usize = 100;
    /// A clan rank's name (32-byte buffer).
    pub const RANK_NAME: usize = 31;
    /// Items of a shop record.
    pub const SHOP_RECORD_ITEMS: usize = 5;
    /// The search properties of 0x85.
    pub const SEARCH_PROPERTIES: usize = 20;
}

/// Builds a body. Strings are `u8 len` and the bytes, cut to the client's
/// buffer at a character boundary.
#[derive(Default)]
pub(crate) struct Writer(Vec<u8>);

impl Writer {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }
    pub(crate) fn u8(&mut self, value: u8) -> &mut Self {
        self.0.push(value);
        self
    }
    /// A wire boolean: the client tests `!= 0`, and some fields `== 1`.
    pub(crate) fn bool(&mut self, value: bool) -> &mut Self {
        self.u8(value.into())
    }
    pub(crate) fn u16(&mut self, value: u16) -> &mut Self {
        self.0.extend(value.to_le_bytes());
        self
    }
    pub(crate) fn u32(&mut self, value: u32) -> &mut Self {
        self.0.extend(value.to_le_bytes());
        self
    }
    pub(crate) fn u64(&mut self, value: u64) -> &mut Self {
        self.0.extend(value.to_le_bytes());
        self
    }
    pub(crate) fn f32(&mut self, value: f32) -> &mut Self {
        self.0.extend(value.to_le_bytes());
        self
    }
    pub(crate) fn bytes(&mut self, value: &[u8]) -> &mut Self {
        self.0.extend_from_slice(value);
        self
    }
    pub(crate) fn str(&mut self, value: &str, max_len: usize) -> &mut Self {
        let cut = cut(value, max_len);
        self.u8(cut.len() as u8);
        self.0.extend(cut.as_bytes());
        self
    }
    /// `u16 count` and the UTF-16LE units, at most `max_units`.
    pub(crate) fn wide(&mut self, value: &str, max_units: usize) -> &mut Self {
        let mut units: Vec<u16> = Vec::new();
        for character in value.chars() {
            let mut pair = [0; 2];
            let encoded = character.encode_utf16(&mut pair);
            if units.len() + encoded.len() > max_units {
                break;
            }
            units.extend_from_slice(encoded);
        }
        self.u16(units.len() as u16);
        for unit in units {
            self.u16(unit);
        }
        self
    }
    pub(crate) fn done(&mut self, id: ServerId) -> ServerMessage {
        ServerMessage {
            id,
            body: std::mem::take(&mut self.0),
        }
    }
}

/// `value` cut to at most `max_len` bytes (and 255, the length byte), never
/// inside a character.
pub fn cut(value: &str, max_len: usize) -> &str {
    let mut end = value.len().min(max_len).min(255);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}
