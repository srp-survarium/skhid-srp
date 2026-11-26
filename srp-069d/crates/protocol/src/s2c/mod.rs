//! Server to client messages. [`S2c`] is the sum of every message; `S2c::decode(id, payload)` and
//! `S2c::encode()` / [`S2c::to_wire`] give the payload after `id x` (AC: after `id`).

pub mod details;
pub mod hero;
pub mod hunt;
pub mod items;
pub mod lobby;
pub mod online;
pub mod pve;
pub mod achievements;
pub mod quests;
pub mod clans;
pub mod league;
pub mod rooms;
pub mod reply;
pub mod results;
pub mod social;

pub use clans::*;
pub use details::PlayerDetails93;
pub use items::*;
pub use league::*;
pub use rooms::*;
pub use lobby::*;
pub use online::ConnectToMatch;
pub use pve::{PveDenied, PveResult, PveResultPlayer, PveSearching, PveSettingsChanged, PveTop100, TopEntry, TopPlayer, SQUAD_TYPE_PVE};
pub use reply::*;
pub use results::*;
pub use social::*;

use crate::ids::{s2c as id, s2c_class, MsgClass};
use crate::wire::{Decode, Encode, Reader, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum S2c {
    SignedIn(SignedIn),
    /// `83 x <request id> ...`
    Reply(Reply),
    /// `84 x <request id> ...`
    Denied(Denied),
    QueueState(QueueState),
    Account(Account87),
    MemberProfile(MemberProfile88),
    Inventory(Inventory89),
    Prices(Prices8A),
    Money(Money),
    Level(Level8C),
    ServicePrices(ServicePrices8D),
    Factions(Factions8E),
    Name(NameMsg),
    SpecificsOpened(SpecificsOpened),
    Factions92(Factions92),
    Notices(Notices94),
    Shop(Shop95),
    FactionMask(FactionMask),
    MatchResult(Box<MatchResult>),
    Clan(NoClan9A),
    CraftChances(CraftChances),
    Flags9C(Flags9C),
    TutorialGate(TutorialGate),
    ServerInfo(ServerInfo9E),
    League(League9F),
    LimitedAccessOff(LimitedAccessOff),
    BoosterPopup(BoosterPopup),
    ImportantMessage(ImportantMessage),
    PlayGate(PlayGate),
    Squad(SquadUpdate),
    Chat(Chat),
    Messaging(Messaging),
    SystemMessage(SystemMessage),
    AdminText(AdminText),
    Workshop(Workshop),
    NoticeB4(NoticeB4),
    DiscountTimer(DiscountTimer),
    DiscountB6(DiscountB6),
    EventIds(EventIds),
    PlayingOrderTip(PlayingOrderTip),
    ItemCounts(ItemCounts90),
    Presets(PresetsBB),
    /// An id this crate does not model (the payload is kept as is).
    Raw { id: u8, payload: Vec<u8> },
}

macro_rules! dispatch {
    ($self:ident, $m:ident => $body:expr, $raw:ident => $rawbody:expr) => {
        match $self {
            S2c::SignedIn($m) => (id::SIGNED_IN, $body),
            S2c::Reply($m) => (id::REPLY, $body),
            S2c::Denied($m) => (id::DENY, $body),
            S2c::QueueState($m) => (id::QUEUE_STATE, $body),
            S2c::Account($m) => (id::ACCOUNT, $body),
            S2c::MemberProfile($m) => (id::MEMBER_PROFILE, $body),
            S2c::Inventory($m) => (id::INVENTORY, $body),
            S2c::Prices($m) => (id::PRICES, $body),
            S2c::Money($m) => (id::MONEY, $body),
            S2c::Level($m) => (id::LEVEL, $body),
            S2c::ServicePrices($m) => (id::SERVICE_PRICES, $body),
            S2c::Factions($m) => (id::FACTIONS, $body),
            S2c::Name($m) => (id::NAME, $body),
            S2c::SpecificsOpened($m) => (id::SPECIFICS_OPENED, $body),
            S2c::Factions92($m) => (id::FACTIONS_92, $body),
            S2c::Notices($m) => (id::NOTICES, $body),
            S2c::Shop($m) => (id::SHOP, $body),
            S2c::FactionMask($m) => (id::FACTION_MASK, $body),
            S2c::MatchResult($m) => (id::MATCH_RESULT, $body),
            S2c::Clan($m) => (id::CLAN, $body),
            S2c::CraftChances($m) => (id::CRAFT_CHANCES, $body),
            S2c::Flags9C($m) => (id::FLAGS_9C, $body),
            S2c::TutorialGate($m) => (id::TUTORIAL_GATE, $body),
            S2c::ServerInfo($m) => (id::SERVER_INFO, $body),
            S2c::League($m) => (id::LEAGUE, $body),
            S2c::LimitedAccessOff($m) => (0xA4, $body),
            S2c::BoosterPopup($m) => (id::BOOSTER_POPUP, $body),
            S2c::ImportantMessage($m) => (id::IMPORTANT_MESSAGE, $body),
            S2c::PlayGate($m) => (id::PLAY_GATE, $body),
            S2c::Squad($m) => (id::SQUAD, $body),
            S2c::Chat($m) => (id::CHAT, $body),
            S2c::Messaging($m) => (id::MESSAGING, $body),
            S2c::SystemMessage($m) => (id::SYSTEM_MESSAGE, $body),
            S2c::AdminText($m) => (id::ADMIN_TEXT, $body),
            S2c::Workshop($m) => (id::WORKSHOP, $body),
            S2c::NoticeB4($m) => (id::NOTICE_B4, $body),
            S2c::DiscountTimer($m) => (id::DISCOUNT_TIMER, $body),
            S2c::DiscountB6($m) => (id::DISCOUNT_B6, $body),
            S2c::EventIds($m) => (id::EVENT_IDS, $body),
            S2c::PlayingOrderTip($m) => (id::PLAYING_ORDER_TIP, $body),
            S2c::ItemCounts($m) => (id::ITEM_COUNTS, $body),
            S2c::Presets($m) => (id::PRESETS, $body),
            S2c::Raw { id: $raw, .. } => (*$raw, $rawbody),
        }
    };
}

impl S2c {
    /// Message id.
    pub fn id(&self) -> u8 {
        match self {
            S2c::Raw { id, .. } => *id,
            other => dispatch!(other, _m => (), raw => ()).0,
        }
    }

    /// Delivery class (which counter the `x` byte takes).
    pub fn class(&self) -> MsgClass {
        s2c_class(self.id())
    }

    /// Payload after `id x` (after `id` for plain messages).
    pub fn encode_payload(&self) -> Result<Vec<u8>> {
        match self {
            S2c::Raw { payload, .. } => Ok(payload.clone()),
            S2c::SignedIn(m) => m.to_bytes(),
            S2c::Reply(m) => m.to_bytes(),
            S2c::Denied(m) => m.to_bytes(),
            S2c::QueueState(m) => m.to_bytes(),
            S2c::Account(m) => m.to_bytes(),
            S2c::MemberProfile(m) => m.to_bytes(),
            S2c::Inventory(m) => m.to_bytes(),
            S2c::Prices(m) => m.to_bytes(),
            S2c::Money(m) => m.to_bytes(),
            S2c::Level(m) => m.to_bytes(),
            S2c::ServicePrices(m) => m.to_bytes(),
            S2c::Factions(m) => m.to_bytes(),
            S2c::Name(m) => m.to_bytes(),
            S2c::SpecificsOpened(m) => m.to_bytes(),
            S2c::Factions92(m) => m.to_bytes(),
            S2c::Notices(m) => m.to_bytes(),
            S2c::Shop(m) => m.to_bytes(),
            S2c::FactionMask(m) => m.to_bytes(),
            S2c::MatchResult(m) => m.to_bytes(),
            S2c::Clan(m) => m.to_bytes(),
            S2c::CraftChances(m) => m.to_bytes(),
            S2c::Flags9C(m) => m.to_bytes(),
            S2c::TutorialGate(m) => m.to_bytes(),
            S2c::ServerInfo(m) => m.to_bytes(),
            S2c::League(m) => m.to_bytes(),
            S2c::LimitedAccessOff(m) => m.to_bytes(),
            S2c::BoosterPopup(m) => m.to_bytes(),
            S2c::ImportantMessage(m) => m.to_bytes(),
            S2c::PlayGate(m) => m.to_bytes(),
            S2c::Squad(m) => m.to_bytes(),
            S2c::Chat(m) => m.to_bytes(),
            S2c::Messaging(m) => m.to_bytes(),
            S2c::SystemMessage(m) => m.to_bytes(),
            S2c::AdminText(m) => m.to_bytes(),
            S2c::Workshop(m) => m.to_bytes(),
            S2c::NoticeB4(m) => m.to_bytes(),
            S2c::DiscountTimer(m) => m.to_bytes(),
            S2c::DiscountB6(m) => m.to_bytes(),
            S2c::EventIds(m) => m.to_bytes(),
            S2c::PlayingOrderTip(m) => m.to_bytes(),
            S2c::ItemCounts(m) => m.to_bytes(),
            S2c::Presets(m) => m.to_bytes(),
        }
    }

    /// Full message as the transport carries it: `id x payload`, or `id payload` for the plain class.
    /// `x` is the counter value of the message's class (ignored for plain messages).
    pub fn to_wire(&self, x: u8) -> Result<Vec<u8>> {
        let mut out = vec![self.id()];
        if self.class() != MsgClass::Plain {
            out.push(x);
        }
        out.extend(self.encode_payload()?);
        Ok(out)
    }

    /// Decode the payload of message `mid` (bytes after `id x`). Ids this crate does not model give
    /// [`S2c::Raw`]; a modelled id with a bad payload is an error.
    pub fn decode(mid: u8, payload: &[u8]) -> Result<S2c> {
        fn d<T: Decode>(p: &[u8]) -> Result<T> {
            T::from_bytes(p)
        }
        Ok(match mid {
            id::SIGNED_IN => S2c::SignedIn(d(payload)?),
            id::REPLY => S2c::Reply(d(payload)?),
            id::DENY => S2c::Denied(d(payload)?),
            // state 1 / 4 / 2 carry more words (queue.rs): only the one-byte form is modelled
            id::QUEUE_STATE if payload.len() != 1 => S2c::Raw { id: mid, payload: payload.to_vec() },
            id::QUEUE_STATE => S2c::QueueState(d(payload)?),
            id::ACCOUNT => S2c::Account(d(payload)?),
            id::MEMBER_PROFILE => S2c::MemberProfile(d(payload)?),
            id::INVENTORY => S2c::Inventory(d(payload)?),
            id::PRICES => S2c::Prices(d(payload)?),
            id::MONEY => S2c::Money(d(payload)?),
            id::LEVEL => S2c::Level(d(payload)?),
            id::SERVICE_PRICES => S2c::ServicePrices(d(payload)?),
            id::FACTIONS => S2c::Factions(d(payload)?),
            id::NAME => S2c::Name(d(payload)?),
            id::SPECIFICS_OPENED => S2c::SpecificsOpened(d(payload)?),
            id::FACTIONS_92 => S2c::Factions92(d(payload)?),
            id::NOTICES => S2c::Notices(d(payload)?),
            id::SHOP => S2c::Shop(d(payload)?),
            id::FACTION_MASK => S2c::FactionMask(d(payload)?),
            id::MATCH_RESULT => S2c::MatchResult(Box::new(d(payload)?)),
            id::CLAN => match d::<NoClan9A>(payload) {
                Ok(m) => S2c::Clan(m),
                // every other 9A form (own clan, members, ranks, ...) is read with `Clan9A::from_bytes`
                Err(_) => S2c::Raw { id: mid, payload: payload.to_vec() },
            },
            id::CRAFT_CHANCES => S2c::CraftChances(d(payload)?),
            id::FLAGS_9C => S2c::Flags9C(d(payload)?),
            id::TUTORIAL_GATE => S2c::TutorialGate(d(payload)?),
            id::SERVER_INFO => S2c::ServerInfo(d(payload)?),
            id::LEAGUE => S2c::League(d(payload)?),
            0xA4 => S2c::LimitedAccessOff(d(payload)?),
            id::BOOSTER_POPUP => S2c::BoosterPopup(d(payload)?),
            id::IMPORTANT_MESSAGE => S2c::ImportantMessage(d(payload)?),
            id::PLAY_GATE => S2c::PlayGate(d(payload)?),
            id::SQUAD => S2c::Squad(d(payload)?),
            id::CHAT => S2c::Chat(d(payload)?),
            id::MESSAGING => S2c::Messaging(d(payload)?),
            id::SYSTEM_MESSAGE => S2c::SystemMessage(d(payload)?),
            id::ADMIN_TEXT => S2c::AdminText(d(payload)?),
            id::WORKSHOP => S2c::Workshop(d(payload)?),
            id::NOTICE_B4 => S2c::NoticeB4(d(payload)?),
            id::DISCOUNT_TIMER => S2c::DiscountTimer(d(payload)?),
            id::DISCOUNT_B6 => S2c::DiscountB6(d(payload)?),
            id::EVENT_IDS => S2c::EventIds(d(payload)?),
            id::PLAYING_ORDER_TIP => S2c::PlayingOrderTip(d(payload)?),
            id::ITEM_COUNTS => S2c::ItemCounts(d(payload)?),
            id::PRESETS => S2c::Presets(d(payload)?),
            _ => S2c::Raw { id: mid, payload: payload.to_vec() },
        })
    }
}

/// Helper for tests and transports: split `id x payload` / `id payload` the way the class says.
pub fn split_wire(msg: &[u8]) -> Result<(u8, Option<u8>, &[u8])> {
    let mut r = Reader::new(msg);
    let mid = r.u8()?;
    if s2c_class(mid) == MsgClass::Plain {
        Ok((mid, None, &msg[1..]))
    } else {
        let x = r.u8()?;
        Ok((mid, Some(x), &msg[2..]))
    }
}
