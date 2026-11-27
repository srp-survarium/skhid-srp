//! Message ids and the delivery class each server message goes out in.
//!
//! The class decides which counter the `x` byte comes from:
//! a wrong class blocks the ordered channel (B0 sent as unique is dropped by the client).

/// Delivery class of a server to client message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgClass {
    /// `id x` with the connection's ordered counter.
    Ordered,
    /// `id x` with the unique counter.
    Unique,
    /// `id` only, no `x` byte (AC).
    Plain,
}

/// Server to client ids (PROVEN: accepted by the real client; LIKELY where noted).
pub mod s2c {
    pub const SIGNED_IN: u8 = 0x80; // unique
    pub const CONNECT_TO_MATCH: u8 = 0x82; // unique (udp-transport table 0x80..=0x82)
    pub const REPLY: u8 = 0x83; // ordered; `83 x <request id> ...`
    pub const DENY: u8 = 0x84; // ordered (LIKELY same class as 83)
    pub const QUEUE_STATE: u8 = 0x85;
    pub const PLAYING_ORDER_TIP: u8 = 0x86;
    pub const ACCOUNT: u8 = 0x87;
    pub const MEMBER_PROFILE: u8 = 0x88;
    pub const INVENTORY: u8 = 0x89;
    pub const PRICES: u8 = 0x8A;
    pub const MONEY: u8 = 0x8B;
    pub const LEVEL: u8 = 0x8C;
    pub const SERVICE_PRICES: u8 = 0x8D;
    pub const FACTIONS: u8 = 0x8E;
    pub const NAME: u8 = 0x8F;
    pub const SPECIFICS_OPENED: u8 = 0x91;
    pub const ITEM_COUNTS: u8 = 0x90;
    pub const FACTIONS_92: u8 = 0x92;
    pub const NOTICES: u8 = 0x94;
    pub const SHOP: u8 = 0x95;
    pub const FACTION_MASK: u8 = 0x96;
    pub const MATCH_RESULT: u8 = 0x97;
    pub const CLAN: u8 = 0x9A;
    pub const CRAFT_CHANCES: u8 = 0x9B;
    pub const FLAGS_9C: u8 = 0x9C;
    pub const TUTORIAL_GATE: u8 = 0x9D;
    pub const SERVER_INFO: u8 = 0x9E; // unique
    pub const LEAGUE: u8 = 0x9F;
    pub const BOOSTER_POPUP: u8 = 0xA5;
    pub const IMPORTANT_MESSAGE: u8 = 0xA8;
    pub const DAILY_QUESTS: u8 = 0xAB;
    pub const PLAY_GATE: u8 = 0xAC; // plain, no x byte
    pub const SQUAD: u8 = 0xAE;
    pub const CHAT: u8 = 0xAF; // unique
    pub const MESSAGING: u8 = 0xB0; // ordered
    pub const SYSTEM_MESSAGE: u8 = 0xB1; // unique
    pub const ADMIN_TEXT: u8 = 0xB2;
    pub const WORKSHOP: u8 = 0xB3;
    pub const NOTICE_B4: u8 = 0xB4;
    pub const DISCOUNT_TIMER: u8 = 0xB5;
    pub const DISCOUNT_B6: u8 = 0xB6;
    pub const EVENT_IDS: u8 = 0xB8;
    pub const PRESETS: u8 = 0xBB;
}

/// Client to server ids.
pub mod c2s {
    pub const SIGN_IN: u8 = 0x2E;
    pub const QUEUE: u8 = 0x2F;
    pub const QUEUE_CANCEL: u8 = 0x31;
    pub const INFO_32: u8 = 0x32;
    pub const MEMBER_PROFILE: u8 = 0x33;
    pub const VICTORY_DATA: u8 = 0x38;
    pub const INVENTORY: u8 = 0x45;
    pub const SHOP: u8 = 0x46;
    pub const SKILLS: u8 = 0x47;
    pub const ACCOUNT: u8 = 0x48;
    pub const SQUAD: u8 = 0x49;
    pub const QUESTS: u8 = 0x4A;
    pub const LEAGUE: u8 = 0x4B;
    pub const CHAT: u8 = 0x4C;
    pub const PEOPLE: u8 = 0x4D;
    pub const FRIENDS: u8 = 0x4E;
    pub const MESSAGING_SIGN_IN: u8 = 0x4F;
    pub const SYSTEM_INFO: u8 = 0x50;
    pub const READ_MESSAGE: u8 = 0x51;
    pub const EVENTS: u8 = 0x53;
    pub const ACHIEVEMENTS: u8 = 0x55;
    pub const PLAY: u8 = 0x5B;
    pub const PVE: u8 = 0x5F;
    pub const WORKSHOP: u8 = 0x60;
}

/// Class of a server message id (default Ordered for ids not seen on the wire: LIKELY).
pub fn s2c_class(id: u8) -> MsgClass {
    // the client's own table (the same as `udp_transport::table::class_of`, which `Peer::push` uses on the wire)
    match id {
        0x80..=0x82 | 0x9A | 0x9E | 0xAD..=0xAF | 0xB1 | 0xB7 | 0xB9..=0xBF | 0xC1 | 0xC2 => MsgClass::Unique,
        0xA2 | 0xA3 | 0xAC | 0xB4 | 0xC0 | 0xC3 => MsgClass::Plain,
        _ => MsgClass::Ordered,
    }
}
