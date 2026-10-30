//! A strict reader of the server's lobby messages that follows the 0.26g
//! client's own reading rules, for tests: give it every message a server
//! sends, in order, and it reports the first one the real client would
//! crash on, misread or silently drop.
//!
//! What it holds the server to (`lobby.md` §3 H1..H11, `results.md` §6,
//! `social.md`):
//! - only ids the client accepts, and only 0x80 before the sign-in is
//!   answered;
//! - every body read to its last byte with the client's layout: nothing
//!   missing, nothing left;
//! - the client's array sizes: 3 kits, 69 account items, 8 modifications
//!   and 3 upgrades an item, 3 squad members, 20 result rows, 7 bonus days,
//!   every string within its buffer;
//! - an item's layout by `Dictionary::is_stack` (a stack has a count and no
//!   times), and every `dict_id` one the client knows;
//! - the inventory (0x89) holding every item the account (0x87) lists, a
//!   kit slot's index inside the account's item table;
//! - a non-zero search step in 0x85, LZMA2 payloads of the exact size.
//!
//! It keeps what the client keeps where a later message depends on it: the
//! inventory (changed by 0x89, purchases, sales, uses, repairs and drops),
//! the friend count (0xA3 09) and the important messages waiting for an
//! answer.

use std::fmt;
use std::num::NonZeroU16;

use super::super::dictionary::Dictionary;
use super::super::lzma2;
use super::super::modifications::Modifications;
use super::super::prices::PriceItem;
use super::super::skills::MODIFIERS;
use super::push::{
    AccountBody, Bought, ClientState, Crafted, FULL_CONDITION, ItemDescr, Kit, KitSlot,
    Modification, NameRefusal, OwnAccount, QuestRecord, Repaired, ServicePrices, ShopRecord,
    ShopRecordItem, SpentMoney, Upgrade,
};
use super::requests::id as client_id;
use super::results::{
    BonusBlock, BonusColumn, FullResult, MatchStats, ResultRow, ScoreComponent, TitleHolder,
};
use super::social::{
    ClanApplicant, ClanInfo, ClanMember, ClanRank, ClanTeamMember, ClanTeamSeat, Friend,
    SquadMember, friendship, sender,
};
use super::tutorial_model::Tutorial;
use super::{FORBIDDEN_SERVER_IDS, ServerId, ServerMessage, limits};

/// What the client would do with a message instead of reading it well.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// An id outside 0x80..=0xAA, or one whose table entry is NULL: a
    /// dropped datagram or a jump to nothing.
    BadId(u8),
    /// Before 0x80 the client logs and ignores anything else.
    NotSignedIn(ServerId),
    /// The body ends before the client stops reading: it reads past it.
    Short(ServerId),
    /// Bytes the client does not read: the layout is not the client's.
    Trailing { id: ServerId, extra: usize },
    /// A string or a count past the client's buffer.
    Overflow {
        id: ServerId,
        what: &'static str,
        len: usize,
        most: usize,
    },
    /// A `dict_id` the client's dictionary lacks: NULL is dereferenced.
    UnknownItem(u16),
    /// An item of the account's table that the inventory lacks: 0x89 must
    /// come before 0x87.
    NotInInventory(u32),
    /// A kit slot whose index is past the account's item table.
    BadItemIndex { index: u16, items: usize },
    /// A modification (state clear) or a defect (state set) that the
    /// client's table of that kind lacks: the row not found is read
    /// (0x229AC4).
    UnknownModification(u32),
    /// The search step of 0x85 is 0: a divide by zero.
    ZeroSearchStep,
    /// An LZMA2 payload that does not unpack to the size the count gives.
    BadPacked(ServerId),
    /// Something else the client's code can not take.
    Bad(ServerId, &'static str),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ModelError {}

/// The answer to an action, by its op and event.
#[derive(Clone, Debug, PartialEq)]
pub enum Permitted {
    /// An op (and for 0x46, 0x47 and 0x49 an event) the client has no
    /// handler for: nothing is read.
    NoEffect {
        op: u8,
        event: Option<u8>,
    },
    /// `83 30`.
    ClanPlay,
    /// `83 4B`.
    QuestFaction,
    UpgradeAttached {
        host_item_id: u32,
        upgrade_item_id: u32,
        slot: u8,
        value: u64,
        detach_first: bool,
    },
    UpgradeDetached {
        upgrade_item_id: u32,
        host_item_id: u32,
    },
    ItemUsed {
        item_id: u32,
    },
    RerollWindow {
        item_id: u32,
        a: u32,
        b: u32,
        more: Option<(u32, u32)>,
    },
    RerollResult {
        item_id: u32,
        choices: [u32; 3],
        reroll_time: u32,
    },
    ModifierChosen {
        item_id: u32,
        old_modification: u32,
        new_modification: u32,
        rest: [u32; 3],
    },
    Crafted {
        item_id: u32,
        crafted: Crafted,
    },
    Bought(Vec<Bought>),
    Sold {
        item_id: u32,
        amount: u32,
    },
    OfferBought,
    Repaired {
        item_id: u32,
        repaired: Repaired,
    },
    /// `83 49 01` with auto-repair off: the autobuy box.
    Autobuy(bool),
    /// `83 49 01` with auto-repair on.
    AutoRepair {
        autobuy: bool,
    },
    ActiveKit(u32),
    NicknameCheck {
        nickname: String,
        refusal: Option<NameRefusal>,
    },
    NicknameChanged {
        ticket_item_id: u32,
    },
    ClanNameReserved,
    ClanNameCheck {
        name: String,
        refusal: Option<NameRefusal>,
    },
    ClanTagCheck {
        tag: String,
        refusal: Option<NameRefusal>,
    },
    ClanInviteSent {
        target_name: String,
    },
    ClanInvite(Option<(u32, String, String)>),
    RankUp(bool),
    ClanTeamInvite {
        extra: u32,
        target_name: String,
        text: String,
    },
}

/// What 0xA3 carried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FriendshipPush {
    Friends(Vec<Friend>),
    Ignored(Vec<(u32, String)>),
    /// `None`: the count was not the friend count the client holds, so it
    /// read nothing (a strict error is returned instead).
    Statuses(Vec<(u32, u8)>),
    /// Sub-type 6, and any other but 7, 8 and 9.
    Found {
        action: u8,
        players: Vec<(u32, String, String)>,
    },
}

/// A message as the client read it.
#[derive(Clone, Debug, PartialEq)]
pub enum Decoded {
    SignedIn {
        client_state: u8,
        flag: bool,
    },
    ConnectToMatch {
        host: String,
        port: u16,
        match_id: u32,
        team: u8,
        match_session: u32,
    },
    Permitted(Permitted),
    Denied {
        op: u8,
        text: String,
    },
    ClientState {
        state: ClientState,
        status_text: Option<String>,
    },
    Account(OwnAccount),
    SquadMemberAccount {
        body: AccountBody,
        revision: u32,
    },
    Inventory {
        items: Vec<ItemDescr>,
        /// The money spent after a match: the three lists that end the
        /// message.
        spent: SpentMoney,
    },
    Prices(Vec<PriceItem>),
    Money {
        generic: u32,
        premium: u32,
    },
    Skills {
        level: u8,
        exp_total: u32,
        exp_level_start: u32,
        exp_next_level: u32,
        skills: Vec<(u16, u8)>,
    },
    ServicePrices(ServicePrices),
    Reputations {
        reputations: Vec<(u8, u16)>,
        quest_active_faction: u8,
    },
    BaseInfo {
        ranked_players: u32,
        nickname: String,
    },
    PlayerElo {
        place: u32,
        rating: u16,
        rating_before: u16,
    },
    /// `(name, place, rating, change)`.
    EloList(Vec<(String, u32, u16, i16)>),
    Quests(Vec<QuestRecord>),
    PlayerDetails {
        discarded: FullResult,
        player: AccountBody,
    },
    Services {
        premium_hours: Option<u32>,
    },
    Shop(Vec<ShopRecord>),
    UnlockedFactions(u8),
    MatchStats {
        stats: MatchStats,
        own: FullResult,
    },
    Drop {
        item: ItemDescr,
        card_item_id: u32,
        card_count: u32,
        rarity: u8,
    },
    DailyBonus {
        progress: Vec<u8>,
        reward: ItemDescr,
        completed: bool,
        show_window: bool,
    },
    ClanInfo(ClanInfo),
    CraftTables {
        chances: [f32; 12],
        prices_general: [u16; 16],
        prices_premium: [u16; 16],
    },
    ClanTeamLimits([u8; 4]),
    Tutorial {
        show: bool,
        a: u32,
        b: u32,
    },
    VideoLink {
        show: bool,
        url: String,
    },
    OnlineStats {
        players_online: u32,
        games_online: u32,
        minutes_to_maintenance: u32,
    },
    SquadInfo {
        squad_id: u32,
        members: Vec<SquadMember>,
        commander_index: u8,
        i_am_commander: bool,
        revision: u32,
        /// srp's last byte: the commander's quest faction (0 none).
        commander_faction: u8,
    },
    ChatText {
        sender_type: u8,
        sender_id: u32,
        sender_name: String,
        /// Only read for a sender of type 5.
        sender_clan_tag: Option<String>,
        channel: u8,
        body: String,
    },
    /// A chat line from an ignored player: the client stops after the
    /// sender's id.
    ChatTextIgnored {
        sender_id: u32,
    },
    Friendship(FriendshipPush),
    Important {
        sender_type: u8,
        sender_id: u32,
        sender_name: String,
        sender_clan_tag: String,
        kind: u8,
        text: String,
        message_id: u32,
        /// The client already held this id and dropped the message.
        duplicate: bool,
    },
    AdminText(String, String),
    AdminTextClear,
    PremiumStore {
        seconds: u32,
        values: [u32; 4],
        factor: f32,
    },
    PremiumDiscount(f32),
    SignedOut,
}

struct Reader<'a> {
    id: ServerId,
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], ModelError> {
        if self.rest.len() < len {
            return Err(ModelError::Short(self.id));
        }
        let (head, tail) = self.rest.split_at(len);
        self.rest = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, ModelError> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, ModelError> {
        Ok(self.u8()? != 0)
    }
    /// The autobuy and auto-repair boxes: the lobby ticks them for 0..=3
    /// only (`LobbyMenu.set_autobuy` compares the number with 1, 2 and 3),
    /// so any other value would show both off.
    fn auto_state(&mut self) -> Result<u8, ModelError> {
        match self.u8()? {
            state @ 0..=3 => Ok(state),
            _ => Err(self.bad("an autobuy state past 3")),
        }
    }
    fn u16(&mut self) -> Result<u16, ModelError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn u32(&mut self) -> Result<u32, ModelError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    fn u64(&mut self) -> Result<u64, ModelError> {
        let low = self.u32()?;
        let high = self.u32()?;
        Ok(u64::from(high) << 32 | u64::from(low))
    }
    fn f32(&mut self) -> Result<f32, ModelError> {
        Ok(f32::from_bits(self.u32()?))
    }
    /// A string into a buffer that holds `most` bytes and the NUL.
    fn str(&mut self, what: &'static str, most: usize) -> Result<String, ModelError> {
        let len = usize::from(self.u8()?);
        let bytes = self.take(len)?;
        if len > most {
            return Err(ModelError::Overflow {
                id: self.id,
                what,
                len,
                most,
            });
        }
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
    fn wide(&mut self, what: &'static str, most: usize) -> Result<String, ModelError> {
        let count = usize::from(self.u16()?);
        let bytes = self.take(count * 2)?;
        if count > most {
            return Err(ModelError::Overflow {
                id: self.id,
                what,
                len: count,
                most,
            });
        }
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }
    fn count(&self, what: &'static str, len: usize, most: usize) -> Result<(), ModelError> {
        if len > most {
            return Err(ModelError::Overflow {
                id: self.id,
                what,
                len,
                most,
            });
        }
        Ok(())
    }
    fn bad(&self, what: &'static str) -> ModelError {
        ModelError::Bad(self.id, what)
    }
}

/// The model of one client's lobby side.
pub struct Client {
    dictionary: &'static Dictionary,
    signed_in: bool,
    /// The inventory vector (lobby_client+0x1EA978), as last sent and
    /// changed since.
    inventory: Vec<ItemDescr>,
    /// The friends the client holds (0xA3 07).
    friends: Vec<u32>,
    ignored: Vec<u32>,
    /// The ids of important messages not answered yet.
    pending_important: Vec<u32>,
    /// It holds the shop's array (0x95; lobby_client+0x1EE5F4), so its
    /// next sign-in does not ask for it.
    shop_held: bool,
    /// How many messages were read.
    received: usize,
    /// The lobby tutorial's state machine (`tutorial_model`).
    tutorial: Tutorial,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// A client that has just connected: nothing received.
    pub fn new() -> Self {
        Self {
            dictionary: Dictionary::shared(),
            signed_in: false,
            inventory: Vec::new(),
            friends: Vec::new(),
            ignored: Vec::new(),
            pending_important: Vec::new(),
            shop_held: false,
            received: 0,
            tutorial: Tutorial::default(),
        }
    }

    /// A client already past 0x80, for tests of single messages.
    pub fn signed_in() -> Self {
        Self {
            signed_in: true,
            ..Self::new()
        }
    }

    pub fn is_signed_in(&self) -> bool {
        self.signed_in
    }

    /// The game connects to the lobby again with the session it has
    /// (`lobby_client::connect` 0x26F650, from "LOBBY: try reconnect"
    /// 0x27B293 and from the first connect alike: there is one path). The
    /// link is new, so 0x80 must come first again; the game object is the
    /// same, so **everything it holds stays**: the inventory, the friends,
    /// the messages it has to answer, the tutorial's state, the shop. It
    /// reads the sign-in's pushes with the same readers as the first time
    /// (`transport.md` §6.1).
    pub fn reconnect(&mut self) {
        self.signed_in = false;
    }

    /// The last byte of the sign-in this client would send (`2E`): `FF`
    /// while it holds no shop array (0x26F6D0).
    pub fn needs_shop_items(&self) -> bool {
        !self.shop_held
    }

    /// The inventory as the client holds it now.
    pub fn inventory(&self) -> &[ItemDescr] {
        &self.inventory
    }

    pub fn received(&self) -> usize {
        self.received
    }

    /// The lobby tutorial: its state and what the player can drag.
    pub fn tutorial(&self) -> &Tutorial {
        &self.tutorial
    }

    /// The tutorial, for the player's steps.
    pub fn tutorial_mut(&mut self) -> &mut Tutorial {
        &mut self.tutorial
    }

    /// The important messages the client still has to answer (`50`).
    pub fn pending_important(&self) -> &[u32] {
        &self.pending_important
    }

    /// The client answered an important message (`50 <id> <answer>`); it
    /// asserts if it does not hold the id.
    pub fn answer_important(&mut self, message_id: u32) -> bool {
        let before = self.pending_important.len();
        self.pending_important.retain(|&id| id != message_id);
        before != self.pending_important.len()
    }

    /// Reads a message given as raw bytes, as a datagram would carry it.
    pub fn receive_raw(&mut self, id: u8, body: &[u8]) -> Result<Decoded, ModelError> {
        if FORBIDDEN_SERVER_IDS.contains(&id) {
            return Err(ModelError::BadId(id));
        }
        let id = ServerId::from_u8(id).ok_or(ModelError::BadId(id))?;
        self.read(id, body)
    }

    /// Reads a message.
    pub fn receive(&mut self, message: &ServerMessage) -> Result<Decoded, ModelError> {
        self.read(message.id(), message.body())
    }

    /// Reads messages in order, stopping at the first the client would not
    /// take.
    pub fn receive_all<'a>(
        &mut self,
        messages: impl IntoIterator<Item = &'a ServerMessage>,
    ) -> Result<Vec<Decoded>, ModelError> {
        messages
            .into_iter()
            .map(|message| self.receive(message))
            .collect()
    }

    fn read(&mut self, id: ServerId, body: &[u8]) -> Result<Decoded, ModelError> {
        if !self.signed_in && id != ServerId::SignedIn {
            return Err(ModelError::NotSignedIn(id));
        }
        let mut reader = Reader { id, rest: body };
        let decoded = self.read_body(&mut reader)?;
        if !reader.rest.is_empty() {
            return Err(ModelError::Trailing {
                id,
                extra: reader.rest.len(),
            });
        }
        self.tutorial.received(&decoded)?;
        self.apply(&decoded);
        self.received += 1;
        Ok(decoded)
    }

    fn find(&self, item_id: u32) -> Option<&ItemDescr> {
        self.inventory.iter().find(|item| item.item_id == item_id)
    }

    /// What the client changes in what it holds.
    fn apply(&mut self, decoded: &Decoded) {
        match decoded {
            Decoded::SignedIn { .. } => self.signed_in = true,
            Decoded::Shop(_) => self.shop_held = true,
            Decoded::Inventory { items, .. } => self.inventory = items.clone(),
            Decoded::Permitted(Permitted::Bought(items)) => {
                for bought in items {
                    let is_stack = self.dictionary.is_stack(bought.dict_id);
                    let value = if is_stack {
                        bought.value
                    } else {
                        FULL_CONDITION
                    };
                    match self
                        .inventory
                        .iter_mut()
                        .find(|item| item.item_id == bought.item_id)
                    {
                        Some(item) if is_stack => {
                            item.condition_or_stack = item.condition_or_stack.saturating_add(value);
                        }
                        Some(_) => {}
                        None => self.inventory.push(ItemDescr::plain(
                            bought.item_id,
                            bought.dict_id,
                            value,
                        )),
                    }
                }
            }
            Decoded::Permitted(Permitted::Sold { item_id, amount }) => {
                let dictionary = self.dictionary;
                self.take_from(*item_id, |item| {
                    dictionary.is_stack(item.dict_id).then_some(*amount)
                });
            }
            Decoded::Permitted(Permitted::ItemUsed { item_id }) if *item_id != 0 => {
                self.take_from(*item_id, |_| Some(1));
            }
            Decoded::Permitted(Permitted::UpgradeAttached {
                host_item_id,
                upgrade_item_id,
                slot,
                value,
                detach_first,
            }) => {
                if *detach_first {
                    self.detach(*upgrade_item_id, *host_item_id);
                }
                // Both must be found, or nothing is done (0x27565B).
                let upgrade_dict = self.find(*upgrade_item_id).map(|item| item.dict_id);
                let host = self
                    .inventory
                    .iter_mut()
                    .find(|item| item.item_id == *host_item_id);
                if let (Some(host), Some(dict_id)) = (host, upgrade_dict) {
                    host.upgrades.retain(|upgrade| upgrade.param != *slot);
                    host.upgrades.push(Upgrade {
                        id: *upgrade_item_id,
                        a: dict_id.into(),
                        value: *value,
                        param: *slot,
                    });
                }
            }
            Decoded::Permitted(Permitted::UpgradeDetached {
                upgrade_item_id,
                host_item_id,
            }) => self.detach(*upgrade_item_id, *host_item_id),
            Decoded::Permitted(Permitted::RerollResult {
                item_id,
                reroll_time,
                ..
            }) => {
                if let Some(item) = self
                    .inventory
                    .iter_mut()
                    .find(|item| item.item_id == *item_id)
                {
                    item.reroll_time = *reroll_time;
                }
            }
            Decoded::Permitted(Permitted::ModifierChosen {
                item_id,
                old_modification,
                new_modification,
                ..
            }) => {
                let held = self
                    .inventory
                    .iter_mut()
                    .find(|item| item.item_id == *item_id)
                    .and_then(|item| {
                        item.modifications
                            .iter_mut()
                            .find(|modification| modification.id == *old_modification)
                    });
                if let Some(held) = held {
                    held.id = *new_modification;
                }
            }
            Decoded::Permitted(Permitted::Crafted { item_id, crafted }) => {
                let item = self
                    .inventory
                    .iter_mut()
                    .find(|item| item.item_id == *item_id);
                let consumed = match (*crafted, item) {
                    (Crafted::Nothing { consumed_item_id }, _) => consumed_item_id,
                    (
                        Crafted::Added {
                            consumed_item_id,
                            modification,
                        },
                        item,
                    ) => {
                        if let Some(item) = item {
                            item.modifications.push(Modification {
                                id: modification,
                                state: false,
                            });
                        }
                        consumed_item_id
                    }
                    (
                        Crafted::Removed {
                            consumed_item_id,
                            modification,
                            ..
                        },
                        item,
                    ) => {
                        if let Some(item) = item {
                            item.modifications
                                .retain(|held| held.id != modification || held.state);
                        }
                        consumed_item_id
                    }
                    (Crafted::Failed, _) => 0,
                };
                self.inventory.retain(|item| item.item_id != consumed);
            }
            Decoded::Permitted(Permitted::NicknameChanged { ticket_item_id })
                if *ticket_item_id != 0 =>
            {
                self.take_from(*ticket_item_id, |_| Some(1));
            }
            Decoded::Permitted(Permitted::Repaired { item_id, repaired }) => {
                if let Some(item) = self
                    .inventory
                    .iter_mut()
                    .find(|item| item.item_id == *item_id)
                {
                    item.condition_or_stack = FULL_CONDITION;
                    item.repair_time = match repaired {
                        Repaired::Timed { repair_time } => *repair_time,
                        _ => 0,
                    };
                }
                if let Repaired::WithKit { kit_item_id, used } = repaired {
                    let used = u32::from(*used);
                    self.take_from(*kit_item_id, |_| Some(used));
                }
            }
            Decoded::Drop {
                item,
                card_item_id,
                card_count,
                ..
            } => {
                match self
                    .inventory
                    .iter_mut()
                    .find(|owned| owned.item_id == item.item_id)
                {
                    Some(owned) => {
                        owned.condition_or_stack = owned
                            .condition_or_stack
                            .saturating_add(item.condition_or_stack);
                    }
                    // Dict ids 0xD6 and 0xD7 are never added (`results.md` §3).
                    None if !matches!(item.dict_id, 0xD6 | 0xD7) => {
                        self.inventory.push(item.clone());
                    }
                    None => {}
                }
                if let Some(card) = self
                    .inventory
                    .iter_mut()
                    .find(|owned| owned.item_id == *card_item_id)
                {
                    card.condition_or_stack = *card_count;
                }
            }
            Decoded::Friendship(FriendshipPush::Friends(friends)) => {
                self.friends = friends.iter().map(|friend| friend.account_id).collect();
            }
            Decoded::Friendship(FriendshipPush::Ignored(ignored)) => {
                self.ignored = ignored.iter().map(|&(id, _)| id).collect();
            }
            Decoded::Important {
                message_id,
                duplicate: false,
                ..
            } => self.pending_important.push(*message_id),
            _ => {}
        }
    }

    /// The upgrade with this item id comes off the host (0x275220 with
    /// patch 30's host, 0x27534D); other items wearing it keep it. A host
    /// of 0 is the stock client's search: the first item holding it.
    fn detach(&mut self, upgrade_item_id: u32, host_item_id: u32) {
        let host = self.inventory.iter_mut().find(|item| {
            if host_item_id == 0 {
                item.upgrades
                    .iter()
                    .any(|upgrade| upgrade.id == upgrade_item_id)
            } else {
                item.item_id == host_item_id
            }
        });
        if let Some(host) = host {
            host.upgrades
                .retain(|upgrade| upgrade.id != upgrade_item_id);
        }
    }

    /// Takes `amount(item)` off an item's count and drops it at 0; `None`
    /// drops it whole.
    fn take_from(&mut self, item_id: u32, amount: impl Fn(&ItemDescr) -> Option<u32>) {
        let Some(index) = self
            .inventory
            .iter()
            .position(|item| item.item_id == item_id)
        else {
            return;
        };
        let item = &mut self.inventory[index];
        match amount(item) {
            Some(amount) if item.condition_or_stack > amount => {
                item.condition_or_stack -= amount;
            }
            _ => {
                self.inventory.remove(index);
            }
        }
    }

    /// What follows `item_id` and `dict_id` (0x1F13D0).
    fn read_item_tail(
        &self,
        r: &mut Reader<'_>,
        item_id: u32,
        dict_id: u16,
        is_stack: bool,
        lobby_form: bool,
    ) -> Result<ItemDescr, ModelError> {
        let condition_or_stack = r.u32()?;
        let (mut repair_time, mut reroll_time) = (0, 0);
        if !is_stack {
            if lobby_form {
                repair_time = r.u32()?;
            }
            reroll_time = r.u32()?;
        }
        let count = usize::from(r.u8()?);
        r.count("item modifications", count, limits::MODIFICATIONS)?;
        let mut modifications = Vec::with_capacity(count);
        for _ in 0..count {
            let modification = Modification {
                id: r.u32()?,
                state: r.bool()?,
            };
            if !Modifications::shared().is_known(modification.id, modification.state) {
                return Err(ModelError::UnknownModification(modification.id));
            }
            modifications.push(modification);
        }
        let count = usize::from(r.u8()?);
        r.count("item upgrades", count, limits::UPGRADES)?;
        let mut upgrades = Vec::with_capacity(count);
        for _ in 0..count {
            upgrades.push(Upgrade {
                id: r.u32()?,
                a: r.u32()?,
                value: r.u64()?,
                param: r.u8()?,
            });
        }
        Ok(ItemDescr {
            item_id,
            dict_id,
            condition_or_stack,
            repair_time,
            reroll_time,
            modifications,
            upgrades,
        })
    }

    /// `inventory_item_descr` (0x1F1350). `None` for the 6-byte empty
    /// entry (dict id 0).
    fn read_item(
        &self,
        r: &mut Reader<'_>,
        lobby_form: bool,
    ) -> Result<Option<ItemDescr>, ModelError> {
        let item_id = r.u32()?;
        let dict_id = r.u16()?;
        if dict_id == 0 {
            return Ok(None);
        }
        let entry = self
            .dictionary
            .item(dict_id)
            .ok_or(ModelError::UnknownItem(dict_id))?;
        self.read_item_tail(r, item_id, dict_id, entry.is_stack, lobby_form)
            .map(Some)
    }

    /// An item where the UI dereferences its dictionary entry: an empty
    /// entry is as bad as an unknown one.
    fn read_real_item(&self, r: &mut Reader<'_>) -> Result<ItemDescr, ModelError> {
        self.read_item(r, true)?.ok_or(ModelError::UnknownItem(0))
    }

    /// The account body (0x1F1070, 0x1F1120).
    fn read_account_body(&self, r: &mut Reader<'_>) -> Result<AccountBody, ModelError> {
        let team = r.u8()?;
        let name = r.str("account name", limits::NAME)?;
        let clan_tag = r.str("account clan tag", limits::ACCOUNT_CLAN_TAG)?;
        let protection = r.u32()?;
        let kit_count = usize::from(r.u8()?);
        let item_count = usize::from(r.u8()?);
        r.count("kits", kit_count, limits::KITS)?;
        r.count("account items", item_count, limits::ACCOUNT_ITEMS)?;
        let mut kits = Vec::with_capacity(kit_count);
        for _ in 0..kit_count {
            let state = r.u8()?;
            let id = r.u32()?;
            let name = r.str("kit name", limits::NAME)?;
            let mask = r.u32()?;
            if mask >> limits::KIT_SLOTS != 0 {
                // The reader loops over 23 bits only: the items of higher
                // bits would not be read.
                return Err(r.bad("a slot mask bit past slot 22"));
            }
            let mut slots = [None; limits::KIT_SLOTS];
            for (index, slot) in slots.iter_mut().enumerate() {
                if mask & 1 << index != 0 {
                    let slot_item = KitSlot {
                        item_index: r.u16()?,
                        count: r.u16()?,
                    };
                    if usize::from(slot_item.item_index) >= item_count {
                        return Err(ModelError::BadItemIndex {
                            index: slot_item.item_index,
                            items: item_count,
                        });
                    }
                    *slot = Some(slot_item);
                }
            }
            // The mask: bits 64..=127 first.
            let high = r.u64()?;
            let low = r.u64()?;
            let mask = u128::from(high) << 64 | u128::from(low);
            let mut modifiers = [0.0; MODIFIERS];
            // The reader tests bits 0..=67 only; a higher bit has no float.
            for (index, modifier) in modifiers.iter_mut().enumerate() {
                if mask & 1 << index != 0 {
                    *modifier = r.f32()?;
                }
            }
            kits.push(Kit {
                state,
                id,
                name,
                slots,
                modifiers,
            });
        }
        let mut items = Vec::with_capacity(item_count);
        for _ in 0..item_count {
            items.push(self.read_real_item(r)?);
        }
        Ok(AccountBody {
            team,
            name,
            clan_tag,
            protection,
            kits,
            items,
        })
    }

    fn read_name_check(
        &self,
        r: &mut Reader<'_>,
        reasons: u8,
    ) -> Result<(String, Option<NameRefusal>), ModelError> {
        let name = r.str("checked name", limits::NAME)?;
        let ok = r.bool()?;
        if ok {
            return Ok((name, None));
        }
        let refusal = match r.u8()? {
            0 => NameRefusal::Short,
            1 => NameRefusal::FewLetters,
            2 => NameRefusal::Taken,
            3 => NameRefusal::Space,
            4 => NameRefusal::WrongSymbol,
            5 if reasons > 5 => NameRefusal::Pattern,
            _ => return Err(r.bad("a refusal reason past the client's texts")),
        };
        Ok((name, Some(refusal)))
    }

    fn read_permitted(&self, r: &mut Reader<'_>) -> Result<Permitted, ModelError> {
        let op = r.u8()?;
        Ok(match op {
            client_id::CLAN_PLAY => Permitted::ClanPlay,
            client_id::QUEST => Permitted::QuestFaction,
            client_id::INVENTORY => {
                let event = r.u8()?;
                match event {
                    1 => Permitted::UpgradeAttached {
                        host_item_id: r.u32()?,
                        upgrade_item_id: r.u32()?,
                        slot: r.u8()?,
                        value: r.u64()?,
                        detach_first: r.bool()?,
                    },
                    2 => Permitted::UpgradeDetached {
                        upgrade_item_id: r.u32()?,
                        host_item_id: r.u32()?,
                    },
                    4 => Permitted::ItemUsed { item_id: r.u32()? },
                    6 => {
                        let (item_id, a, b) = (r.u32()?, r.u32()?, r.u32()?);
                        let more = if r.bool()? {
                            Some((r.u32()?, r.u32()?))
                        } else {
                            None
                        };
                        // The window lists the item's modifications with
                        // no check that it was found (0x29F58B).
                        if self.find(item_id).is_none() {
                            return Err(ModelError::NotInInventory(item_id));
                        }
                        Permitted::RerollWindow {
                            item_id,
                            a,
                            b,
                            more,
                        }
                    }
                    7 => {
                        let item_id = r.u32()?;
                        let choices = [r.u32()?, r.u32()?, r.u32()?];
                        // Each is looked up as a modification (0x29F717).
                        for choice in choices {
                            if !Modifications::shared().is_known(choice, false) {
                                return Err(ModelError::UnknownModification(choice));
                            }
                        }
                        Permitted::RerollResult {
                            item_id,
                            choices,
                            reroll_time: r.u32()?,
                        }
                    }
                    8 => {
                        let item_id = r.u32()?;
                        let (old_modification, new_modification) = (r.u32()?, r.u32()?);
                        let rest = [r.u32()?, r.u32()?, r.u32()?];
                        // The window is filled again from the item.
                        if self.find(item_id).is_none() {
                            return Err(ModelError::NotInInventory(item_id));
                        }
                        Permitted::ModifierChosen {
                            item_id,
                            old_modification,
                            new_modification,
                            rest,
                        }
                    }
                    9 => {
                        let mode = r.u8()?;
                        let item_id = r.u32()?;
                        // The item's dictionary entry is used with no check.
                        if self.find(item_id).is_none() {
                            return Err(ModelError::NotInInventory(item_id));
                        }
                        // Mode 0xFE ends here (0x275A31).
                        let consumed_item_id = if mode != 0xFE { r.u32()? } else { 0 };
                        let modification = if !matches!(mode, 0 | 0xFE) {
                            r.u32()?
                        } else {
                            0
                        };
                        // The window shows the consumed item's dictionary
                        // entry as the crafted one's, and the modification
                        // from its table.
                        if mode != 0xFE && self.find(consumed_item_id).is_none() {
                            return Err(ModelError::NotInInventory(consumed_item_id));
                        }
                        if !matches!(mode, 0 | 0xFE)
                            && !Modifications::shared().is_known(modification, false)
                        {
                            return Err(ModelError::UnknownModification(modification));
                        }
                        let crafted = match mode {
                            0 => Crafted::Nothing { consumed_item_id },
                            1 => Crafted::Added {
                                consumed_item_id,
                                modification,
                            },
                            0xFE => Crafted::Failed,
                            mode => Crafted::Removed {
                                mode,
                                consumed_item_id,
                                modification,
                            },
                        };
                        Permitted::Crafted { item_id, crafted }
                    }
                    event => Permitted::NoEffect {
                        op,
                        event: Some(event),
                    },
                }
            }
            client_id::SHOP => {
                let event = r.u8()?;
                match event {
                    0 => {
                        let count = r.u8()?;
                        let mut items = Vec::with_capacity(count.into());
                        for _ in 0..count {
                            let bought = Bought {
                                dict_id: r.u16()?,
                                item_id: r.u32()?,
                                value: r.u32()?,
                            };
                            if self.dictionary.item(bought.dict_id).is_none() {
                                return Err(ModelError::UnknownItem(bought.dict_id));
                            }
                            items.push(bought);
                        }
                        Permitted::Bought(items)
                    }
                    1 => {
                        let (item_id, amount) = (r.u32()?, r.u32()?);
                        // The sold item's dictionary entry is used with no
                        // check; an item the client lacks is a server bug.
                        if self.find(item_id).is_none() {
                            return Err(ModelError::NotInInventory(item_id));
                        }
                        Permitted::Sold { item_id, amount }
                    }
                    2 => Permitted::OfferBought,
                    3 => {
                        let item_id = r.u32()?;
                        // Nothing more is read for an item it lacks.
                        if self.find(item_id).is_none() {
                            return Err(ModelError::NotInInventory(item_id));
                        }
                        let repair_time = r.u32()?;
                        let repaired = if repair_time != 0 {
                            Repaired::Timed { repair_time }
                        } else {
                            let kit_item_id = r.u32()?;
                            if kit_item_id == 0 {
                                Repaired::AtOnce
                            } else if self.find(kit_item_id).is_some() {
                                Repaired::WithKit {
                                    kit_item_id,
                                    used: r.u8()?,
                                }
                            } else {
                                return Err(ModelError::NotInInventory(kit_item_id));
                            }
                        };
                        Permitted::Repaired { item_id, repaired }
                    }
                    event => Permitted::NoEffect {
                        op,
                        event: Some(event),
                    },
                }
            }
            client_id::PROFILE => {
                let event = r.u8()?;
                match event {
                    0x01 => match r.auto_state()? {
                        state if state & 2 == 0 => Permitted::Autobuy(state & 1 != 0),
                        state => Permitted::AutoRepair {
                            autobuy: state & 1 != 0,
                        },
                    },
                    0x02 => Permitted::ActiveKit(r.u32()?),
                    0x03 => {
                        let (nickname, refusal) = self.read_name_check(r, 6)?;
                        Permitted::NicknameCheck { nickname, refusal }
                    }
                    0x04 => Permitted::NicknameChanged {
                        ticket_item_id: r.u32()?,
                    },
                    0x06 => Permitted::ClanNameReserved,
                    0x08 => {
                        let (name, refusal) = self.read_name_check(r, 5)?;
                        Permitted::ClanNameCheck { name, refusal }
                    }
                    0x09 => {
                        let (tag, refusal) = self.read_name_check(r, 5)?;
                        Permitted::ClanTagCheck { tag, refusal }
                    }
                    0x0B => Permitted::ClanInviteSent {
                        target_name: r.str("invited name", limits::NAME)?,
                    },
                    0x0F => {
                        if r.u8()? == 1 {
                            Permitted::ClanInvite(None)
                        } else {
                            Permitted::ClanInvite(Some((
                                r.u32()?,
                                r.str("invited name", limits::NAME)?,
                                r.str("invite text", limits::TEXT)?,
                            )))
                        }
                    }
                    0x12 => Permitted::RankUp(r.bool()?),
                    0x19 => Permitted::ClanTeamInvite {
                        extra: r.u32()?,
                        target_name: r.str("invited name", limits::NAME)?,
                        text: r.str("invite text", limits::TEXT)?,
                    },
                    event => Permitted::NoEffect {
                        op,
                        event: Some(event),
                    },
                }
            }
            op => Permitted::NoEffect { op, event: None },
        })
    }

    fn read_title(&self, r: &mut Reader<'_>) -> Result<TitleHolder, ModelError> {
        Ok(TitleHolder {
            name: r.str("title name", limits::NAME)?,
            clan_tag: r.str("title clan tag", 14)?,
        })
    }

    fn read_match_stats(&self, r: &mut Reader<'_>) -> Result<MatchStats, ModelError> {
        let match_id = r.u32()?;
        let unknown = r.u32()?;
        let winner_team = r.u8()?;
        let team_scores = [r.u8()?, r.u8()?];
        let team_params = [r.u32()?, r.u32()?];
        let game_mode = r.u8()?;
        let best_player = self.read_title(r)?;
        let best_player_score = r.u16()?;
        let best_shooter = self.read_title(r)?;
        let shooter_accuracy_percent = r.u8()?;
        let shooter_shots = r.u16()?;
        let best_support = self.read_title(r)?;
        let support_value = r.u16()?;
        let best_explorer = self.read_title(r)?;
        let explorer_artefacts = r.u16()?;
        let explorer_items = r.u16()?;
        let count = usize::from(r.u8()?);
        r.count("result rows", count, limits::RESULT_ROWS)?;
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            let row = ResultRow {
                name: r.str("row name", limits::NAME)?,
                clan_tag: r.str("row clan tag", limits::CLAN_TAG)?,
                player_id: r.u32()?,
                score: r.u16()?,
                kills: r.u16()?,
                deaths: r.u16()?,
                team: r.u8()?,
                level: r.u16()?,
            };
            if row.team > 1 {
                return Err(r.bad("a result row's team past 1"));
            }
            rows.push(row);
        }
        Ok(MatchStats {
            match_id,
            unknown,
            winner_team,
            team_scores,
            team_params,
            game_mode,
            best_player,
            best_player_score,
            best_shooter,
            shooter_accuracy_percent,
            shooter_shots,
            best_support,
            support_value,
            best_explorer,
            explorer_artefacts,
            explorer_items,
            rows,
        })
    }

    fn read_bonus(&self, r: &mut Reader<'_>) -> Result<BonusBlock, ModelError> {
        let mut block = BonusBlock::default();
        for column in &mut block.columns {
            *column = BonusColumn {
                money: r.u32()?,
                reputation: r.u16()?,
                experience: r.u16()?,
                bonus_money: r.u8()?,
                bonus_reputation: r.u8()?,
                bonus_experience: r.u8()?,
            };
        }
        Ok(block)
    }

    fn read_full(&self, r: &mut Reader<'_>) -> Result<FullResult, ModelError> {
        let player_id = r.u32()?;
        let mut components: [ScoreComponent; 3] = Default::default();
        for component in &mut components {
            component.name_key = r.str("score component name", limits::NAME)?;
            component.show_amount = r.bool()?;
            for column in &mut component.columns {
                let amount = r.f32()?;
                // The other three are on the wire only for an amount that
                // is not 0.0 (NaN counts as not 0).
                *column = if amount != 0.0 || amount.is_nan() {
                    (amount, r.u32()?, r.u16()? as i16, r.u16()? as i16)
                } else {
                    (amount, 0, 0, 0)
                };
            }
        }
        let daily = self.read_bonus(r)?;
        let premium_items = self.read_bonus(r)?;
        let premium = self.read_bonus(r)?;
        let elo_before = r.u16()?;
        let elo_after = r.u16()?;
        let quest_money = r.u32()?;
        let quest_reputation = r.u16()?;
        let faction = r.u8()?;
        let play_seconds = [r.u16()?, r.u16()?, r.u16()?];
        let list = usize::from(r.u8()?);
        // Room for 4; the 5th lands on the count itself.
        r.count("the unread list of a full result", list, 4)?;
        r.take(list * 4)?;
        let dropped = self.read_item(r, false)?;
        let drop_rarity = r.u8()?;
        let drop_chances = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        Ok(FullResult {
            player_id,
            components,
            daily,
            premium_items,
            premium,
            elo_before,
            elo_after,
            quest_money,
            quest_reputation,
            faction,
            play_seconds,
            dropped,
            drop_rarity,
            drop_chances,
        })
    }

    fn read_ranks(&self, r: &mut Reader<'_>) -> Result<Vec<ClanRank>, ModelError> {
        let count = usize::from(r.u8()?);
        r.count("clan ranks", count, limits::CLAN_RANKS)?;
        (0..count)
            .map(|_| {
                Ok(ClanRank {
                    name: r.str("rank name", limits::RANK_NAME)?,
                    permissions: r.u8()?,
                    is_default: r.u8()? == 1,
                })
            })
            .collect()
    }

    fn read_members(&self, r: &mut Reader<'_>) -> Result<Vec<ClanMember>, ModelError> {
        let count = r.u32()? as usize;
        r.count("clan members", count, limits::CLAN_MEMBERS)?;
        (0..count)
            .map(|_| {
                Ok(ClanMember {
                    account_id: r.u32()?,
                    name: r.str("member name", limits::NAME)?,
                    rank: r.u8()?,
                    level: r.u8()?,
                    online: r.u8()? == 1,
                })
            })
            .collect()
    }

    fn read_clan_info(&self, r: &mut Reader<'_>) -> Result<ClanInfo, ModelError> {
        let kind = r.u8()?;
        if kind == 6 {
            return Ok(ClanInfo::Disbanded { show: r.bool()? });
        }
        if kind > 6 {
            return Err(r.bad("a clan message type past 6"));
        }
        let clan_id = r.u32()?;
        if clan_id == 0 {
            // Nothing else is read, whatever the type.
            return if kind == 0 {
                Ok(ClanInfo::None)
            } else {
                Err(r.bad("clan id 0 in a clan update"))
            };
        }
        Ok(match kind {
            0 => {
                let name = r.str("clan name", limits::NAME)?;
                // A stack buffer of 80.
                let tag = r.str("clan tag", 79)?;
                let registered = r.u8()? == 1;
                let ranks = self.read_ranks(r)?;
                if registered {
                    let members = self.read_members(r)?;
                    ClanInfo::Registered {
                        clan_id,
                        name,
                        tag,
                        ranks,
                        members,
                        unknown: r.u16()?,
                        flag: r.u8()? == 1,
                        level: r.u8()?,
                        progress: [r.u32()?, r.u32()?, r.u32()?, r.u32()?],
                    }
                } else {
                    let count = r.u32()? as usize;
                    r.count("clan members", count, limits::CLAN_MEMBERS)?;
                    let members = (0..count)
                        .map(|_| {
                            Ok(ClanApplicant {
                                account_id: r.u32()?,
                                name: r.str("member name", limits::NAME)?,
                                rank: r.u8()?,
                                status: r.u8()?,
                            })
                        })
                        .collect::<Result<_, ModelError>>()?;
                    ClanInfo::Registering {
                        clan_id,
                        name,
                        tag,
                        ranks,
                        members,
                        unknown: r.u16()?,
                        flag: r.u8()? == 1,
                        paid: r.u8()? == 1,
                        money: r.u32()?,
                        reg_state: r.u8()?,
                    }
                }
            }
            1 => ClanInfo::DailyMessage {
                clan_id,
                text: r.wide("daily message", limits::WIDE_TEXT)?,
            },
            2 => ClanInfo::Members {
                clan_id,
                members: self.read_members(r)?,
                unknown: r.u16()?,
                flag: r.u8()? == 1,
            },
            3 => ClanInfo::Ranks {
                clan_id,
                ranks: self.read_ranks(r)?,
            },
            4 => {
                let count = usize::from(r.u8()?);
                r.count("clan team members", count, limits::CLAN_TEAM_MEMBERS)?;
                let members = (0..count)
                    .map(|_| {
                        Ok(ClanTeamMember {
                            account_id: r.u32()?,
                            name: r.str("member name", limits::NAME)?,
                            role: r.u8()?,
                            ready: r.bool()?,
                        })
                    })
                    .collect::<Result<_, ModelError>>()?;
                let seat = if r.u8()? != 0 {
                    Some(ClanTeamSeat {
                        in_clan_team: r.bool()?,
                        i_am_commander: r.bool()?,
                        commander_ready: r.bool()?,
                        clan_team_id: r.u32()?,
                    })
                } else {
                    None
                };
                ClanInfo::ClanTeam {
                    clan_id,
                    members,
                    seat,
                }
            }
            _ => {
                if r.u8()? == 0 {
                    return Err(r.bad("a clan start-up info with its flag clear"));
                }
                ClanInfo::Startup {
                    clan_id,
                    name: r.str("clan name", limits::NAME)?,
                    tag: r.str("clan tag", limits::CLAN_TAG)?,
                }
            }
        })
    }

    fn read_body(&self, r: &mut Reader<'_>) -> Result<Decoded, ModelError> {
        Ok(match r.id {
            ServerId::SignedIn => Decoded::SignedIn {
                client_state: r.u8()?,
                flag: r.bool()?,
            },
            ServerId::ConnectToMatch => Decoded::ConnectToMatch {
                host: r.str("match host", limits::HOST)?,
                port: r.u16()?,
                match_id: r.u32()?,
                team: r.u8()?,
                match_session: r.u32()?,
            },
            ServerId::Permitted => Decoded::Permitted(self.read_permitted(r)?),
            ServerId::Denied => {
                let op = r.u8()?;
                r.u8()?;
                Decoded::Denied {
                    op,
                    text: r.str("denial text", limits::TEXT)?,
                }
            }
            ServerId::ClientState => {
                let state = match r.u8()? {
                    0 => ClientState::Lobby,
                    1 => {
                        let order_id = r.u32()?;
                        let place = r.u32()?;
                        let queue_total = r.u32()?;
                        let current_wait_seconds = r.u32()?;
                        let average_wait_seconds = r.u32()?;
                        let search_step =
                            NonZeroU16::new(r.u16()?).ok_or(ModelError::ZeroSearchStep)?;
                        let mut search_properties = [0; limits::SEARCH_PROPERTIES];
                        for property in &mut search_properties {
                            *property = r.u16()?;
                        }
                        ClientState::InQueue {
                            order_id,
                            place,
                            queue_total,
                            current_wait_seconds,
                            average_wait_seconds,
                            search_step,
                            search_properties,
                        }
                    }
                    2 => ClientState::InMatch {
                        order_id: r.u32()?,
                        match_id: r.u32()?,
                        team: r.u8()?,
                    },
                    _ => return Err(r.bad("a client state with no layout")),
                };
                // The text is read only if bytes remain.
                let status_text = if r.rest.is_empty() {
                    None
                } else {
                    Some(r.str("status text", limits::STATUS_TEXT)?)
                };
                Decoded::ClientState { state, status_text }
            }
            ServerId::Account => {
                let body = self.read_account_body(r)?;
                let mut revisions = Vec::with_capacity(body.kits.len());
                for _ in 0..body.kits.len() {
                    revisions.push(r.u32()?);
                }
                let active_kit = r.u32()?;
                let auto_state = r.auto_state()?;
                let account = OwnAccount {
                    body,
                    revisions,
                    active_kit,
                    autobuy: auto_state & 1 != 0,
                    auto_repair: auto_state & 2 != 0,
                    level: r.u8()?,
                    exp_total: r.u32()?,
                    exp_level_start: r.u32()?,
                    exp_next_level: r.u32()?,
                };
                // 0x270320: every item is copied from the inventory by id;
                // a miss is a NULL source.
                for item in &account.body.items {
                    if self.find(item.item_id).is_none() {
                        return Err(ModelError::NotInInventory(item.item_id));
                    }
                }
                if !account.body.kits.is_empty()
                    && !account
                        .body
                        .kits
                        .iter()
                        .any(|kit| kit.id == account.active_kit)
                {
                    return Err(r.bad("an active kit id that is none of the kits"));
                }
                Decoded::Account(account)
            }
            ServerId::SquadMemberAccount => Decoded::SquadMemberAccount {
                body: self.read_account_body(r)?,
                revision: r.u32()?,
            },
            ServerId::Inventory => {
                let count = r.u32()? as usize;
                // Each item is at least 12 bytes: a count past the body is
                // a read past it.
                if count > r.rest.len() {
                    return Err(ModelError::Short(r.id));
                }
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.read_real_item(r)?);
                }
                // 0x26EF40: three lists of {u32, u16}, each into a stack
                // buffer; a count past the buffer is a fatal assert.
                let mut spent = SpentMoney::default();
                let limits = spent.lists().map(|(_, _, most)| most);
                for (list, most) in [
                    &mut spent.autobuy_ammo,
                    &mut spent.autobuy_consumables,
                    &mut spent.repair,
                ]
                .into_iter()
                .zip(limits)
                {
                    let count = usize::from(r.u8()?);
                    if count > most {
                        return Err(r.bad("more spent-money rows than the client's buffer"));
                    }
                    for _ in 0..count {
                        let value = r.u32()?;
                        list.push((value, r.u16()?));
                    }
                }
                Decoded::Inventory { items, spent }
            }
            ServerId::Prices => {
                let count = usize::from(r.u16()?);
                if count == 0 {
                    return Ok(Decoded::Prices(Vec::new()));
                }
                let packed = r.u32()? as usize;
                let stream = r.take(packed)?;
                let unpacked = lzma2::unstore(stream).ok_or(ModelError::BadPacked(r.id))?;
                if unpacked.len() != count * PriceItem::SIZE {
                    return Err(ModelError::BadPacked(r.id));
                }
                let records = unpacked
                    .as_chunks::<{ PriceItem::SIZE }>()
                    .0
                    .iter()
                    .map(|record| PriceItem {
                        cost: u32::from_le_bytes([record[0], record[1], record[2], record[3]]),
                        dict_id: u16::from_le_bytes([record[4], record[5]]),
                        level: record[6],
                        faction_id: record[7],
                    })
                    .collect();
                Decoded::Prices(records)
            }
            ServerId::Money => Decoded::Money {
                generic: r.u32()?,
                premium: r.u32()?,
            },
            ServerId::Skills => {
                let level = r.u8()?;
                let exp_total = r.u32()?;
                let exp_level_start = r.u32()?;
                let exp_next_level = r.u32()?;
                let count = r.u8()?;
                let mut skills = Vec::with_capacity(count.into());
                for _ in 0..count {
                    skills.push((r.u16()?, r.u8()?));
                }
                Decoded::Skills {
                    level,
                    exp_total,
                    exp_level_start,
                    exp_next_level,
                    skills,
                }
            }
            ServerId::ServicePrices => Decoded::ServicePrices(ServicePrices {
                reroll: r.u32()?,
                reroll_premium: r.u32()?,
                profile: r.u32()?,
                change_name: r.u32()?,
                premium_exchange_rate: r.u32()?,
                clan_creation: r.u32()?,
            }),
            ServerId::Reputations => {
                let count = r.u8()?;
                if count == 0xFF {
                    return Err(r.bad("a reputation count the client takes for none received"));
                }
                let mut reputations = Vec::with_capacity(count.into());
                for _ in 0..count {
                    let faction = r.u32()?;
                    r.u16()?;
                    reputations.push((faction as u8, r.u16()?));
                }
                Decoded::Reputations {
                    reputations,
                    quest_active_faction: r.u8()?,
                }
            }
            ServerId::BaseInfo => Decoded::BaseInfo {
                ranked_players: r.u32()?,
                nickname: r.str("nickname", limits::NAME)?,
            },
            ServerId::PlayerElo => Decoded::PlayerElo {
                place: r.u32()?,
                rating: r.u16()?,
                rating_before: r.u16()?,
            },
            ServerId::EloList => {
                let count = r.u8()?;
                let mut rows = Vec::with_capacity(count.into());
                for _ in 0..count {
                    rows.push((
                        r.str("ranking name", limits::NAME)?,
                        r.u32()?,
                        r.u16()?,
                        r.u16()? as i16,
                    ));
                }
                Decoded::EloList(rows)
            }
            ServerId::Quests => {
                let count = r.u32()? as usize;
                if count > r.rest.len() / 20 {
                    return Err(ModelError::Short(r.id));
                }
                let mut records = Vec::with_capacity(count);
                for _ in 0..count {
                    let record = QuestRecord {
                        instance_id: r.u32()?,
                        quest_id: r.u32()?,
                        games: r.u32()?,
                        minutes_left: r.u32()?,
                        active: r.u8()? == 1,
                    };
                    r.take(3)?;
                    records.push(record);
                }
                Decoded::Quests(records)
            }
            ServerId::PlayerDetails => Decoded::PlayerDetails {
                discarded: self.read_full(r)?,
                player: self.read_account_body(r)?,
            },
            ServerId::Services => {
                let count = r.u8()?;
                let mut premium_hours = None;
                for _ in 0..count {
                    let kind = r.u8()?;
                    r.u8()?;
                    let a = r.u32()?;
                    r.u32()?;
                    if kind == 0 {
                        premium_hours = Some(a);
                    }
                }
                Decoded::Services { premium_hours }
            }
            ServerId::Shop => {
                let count = r.u32()? as usize;
                let packed = r.u32()? as usize;
                let stream = r.take(packed)?;
                let unpacked = lzma2::unstore(stream).ok_or(ModelError::BadPacked(r.id))?;
                if unpacked.len() != count * ShopRecord::SIZE {
                    return Err(ModelError::BadPacked(r.id));
                }
                let mut records = Vec::with_capacity(count);
                for record in unpacked.as_chunks::<{ ShopRecord::SIZE }>().0 {
                    let at = |offset: usize| {
                        u32::from_le_bytes([
                            record[offset],
                            record[offset + 1],
                            record[offset + 2],
                            record[offset + 3],
                        ])
                    };
                    // The count is the byte at +0x14.
                    let items = usize::from(record[0x14]);
                    r.count("shop record items", items, limits::SHOP_RECORD_ITEMS)?;
                    // 0x2CAC10: the record's texts and icon are looked up
                    // in `shop_bundles_dict` by the u32 at +0x04, and the
                    // result is used with no check.
                    let bundle = at(0x04);
                    if !self
                        .dictionary
                        .bundles
                        .iter()
                        .any(|entry| u32::from(entry.id) == bundle)
                    {
                        return Err(r.bad("a shop record of no bundle of the dictionary"));
                    }
                    // 0x228046 / 0x228416: kind 0 is premium time, 1 a
                    // bundle; a record of another kind is shown nowhere.
                    if at(0x0C) > 1 {
                        return Err(
                            r.bad("a shop record that is neither premium time nor a bundle")
                        );
                    }
                    // 0x228A00: an item whose u32 at +0x20 is 0 is listed
                    // as a bundle's content, and the lobby makes an item of
                    // its dict id (`InventoryList.setupBundleData`).
                    for index in 0..items {
                        let base = 0x18 + index * 0x24;
                        let dict_id = at(base);
                        let known = u16::try_from(dict_id)
                            .is_ok_and(|dict_id| self.dictionary.item(dict_id).is_some());
                        if at(0x0C) == 1 && at(base + 0x20) == 0 && !known {
                            return Err(r.bad("a bundle item the dictionary lacks"));
                        }
                    }
                    records.push(ShopRecord {
                        id: at(0x00),
                        ui_key: at(0x04),
                        kind: at(0x0C),
                        value: at(0x10),
                        items: (0..items)
                            .map(|index| {
                                let base = 0x18 + index * 0x24;
                                ShopRecordItem {
                                    dict_id: at(base),
                                    amount: at(base + 0x0C),
                                    a: at(base + 0x14),
                                    b: at(base + 0x18),
                                    kind: at(base + 0x20),
                                }
                            })
                            .collect(),
                        money_type: at(0xCC),
                    });
                }
                Decoded::Shop(records)
            }
            ServerId::UnlockedFactions => Decoded::UnlockedFactions(r.u8()?),
            ServerId::MatchStats => Decoded::MatchStats {
                stats: self.read_match_stats(r)?,
                own: self.read_full(r)?,
            },
            ServerId::Drop => Decoded::Drop {
                item: self.read_real_item(r)?,
                card_item_id: r.u32()?,
                card_count: r.u32()?,
                rarity: r.u8()?,
            },
            ServerId::DailyBonus => {
                let days = usize::from(r.u8()?);
                let progress = r.take(days)?.to_vec();
                r.count("daily bonus days", days, limits::BONUS_DAYS)?;
                // 0x2773CE: the stack flag first, then the item's id
                // and its dictionary id.
                let is_stack = r.bool()?;
                let item_id = r.u32()?;
                let dict_id = r.u16()?;
                let entry = self
                    .dictionary
                    .item(dict_id)
                    .ok_or(ModelError::UnknownItem(dict_id))?;
                if entry.is_stack != is_stack {
                    return Err(r.bad("a daily bonus item whose stack flag is not its item's"));
                }
                let reward = self.read_item_tail(r, item_id, dict_id, is_stack, true)?;
                Decoded::DailyBonus {
                    progress,
                    reward,
                    completed: r.bool()?,
                    show_window: r.bool()?,
                }
            }
            ServerId::ClanInfo => Decoded::ClanInfo(self.read_clan_info(r)?),
            ServerId::CraftTables => {
                let mut chances = [0.0; 12];
                for chance in &mut chances {
                    *chance = r.f32()?;
                }
                let mut prices_general = [0; 16];
                for price in &mut prices_general {
                    *price = r.u16()?;
                }
                let mut prices_premium = [0; 16];
                for price in &mut prices_premium {
                    *price = r.u16()?;
                }
                Decoded::CraftTables {
                    chances,
                    prices_general,
                    prices_premium,
                }
            }
            ServerId::ClanTeamLimits => {
                Decoded::ClanTeamLimits([r.u8()?, r.u8()?, r.u8()?, r.u8()?])
            }
            ServerId::Tutorial => Decoded::Tutorial {
                show: r.bool()?,
                a: r.u32()?,
                b: r.u32()?,
            },
            ServerId::VideoLink => Decoded::VideoLink {
                show: r.bool()?,
                url: r.wide("video link", limits::WIDE_TEXT)?,
            },
            ServerId::OnlineStats => Decoded::OnlineStats {
                players_online: r.u32()?,
                games_online: r.u32()?,
                minutes_to_maintenance: r.u32()?,
            },
            ServerId::SquadInfo => {
                let squad_id = r.u32()?;
                if squad_id == 0 {
                    return Ok(Decoded::SquadInfo {
                        squad_id,
                        members: Vec::new(),
                        commander_index: 0,
                        i_am_commander: false,
                        revision: 0,
                        commander_faction: 0,
                    });
                }
                let count = usize::from(r.u8()?);
                r.count("squad members", count, limits::SQUAD_MEMBERS)?;
                let commander_index = r.u8()?;
                let mut members = Vec::with_capacity(count);
                for _ in 0..count {
                    members.push(SquadMember {
                        account_id: r.u32()?,
                        kit_id: r.u32()?,
                        kit_revision: r.u32()?,
                        name: r.str("squad member name", limits::NAME)?,
                        online: r.bool()?,
                        ready: r.bool()?,
                    });
                }
                if count > 0 && usize::from(commander_index) >= count {
                    return Err(r.bad("a commander index past the members"));
                }
                Decoded::SquadInfo {
                    squad_id,
                    members,
                    commander_index,
                    i_am_commander: r.bool()?,
                    revision: r.u32()?,
                    commander_faction: r.u8()?,
                }
            }
            ServerId::ChatText => {
                let sender_type = r.u8()?;
                let sender_id = r.u32()?;
                if sender_type == sender::ACCOUNT && self.ignored.contains(&sender_id) {
                    // Nothing more is read.
                    r.rest = &[];
                    return Ok(Decoded::ChatTextIgnored { sender_id });
                }
                let sender_name = r.str("chat sender name", limits::NAME)?;
                let sender_clan_tag = if sender_type == sender::ACCOUNT {
                    Some(r.str("chat sender clan tag", limits::CLAN_TAG)?)
                } else {
                    None
                };
                Decoded::ChatText {
                    sender_type,
                    sender_id,
                    sender_name,
                    sender_clan_tag,
                    channel: r.u8()?,
                    body: r.str("chat body", limits::TEXT)?,
                }
            }
            ServerId::Friendship => {
                let action = r.u8()?;
                let count = usize::from(r.u16()?);
                Decoded::Friendship(match action {
                    friendship::FRIEND_LIST => {
                        let mut friends = Vec::with_capacity(count.min(1024));
                        for _ in 0..count {
                            friends.push(Friend {
                                account_id: r.u32()?,
                                name: r.str("friend name", limits::NAME)?,
                                clan_tag: r.str("friend clan tag", limits::CLAN_TAG)?,
                                status: r.u8()?,
                            });
                        }
                        FriendshipPush::Friends(friends)
                    }
                    friendship::IGNORE_LIST => {
                        let mut ignored = Vec::with_capacity(count.min(1024));
                        for _ in 0..count {
                            ignored.push((r.u32()?, r.str("ignored name", limits::NAME)?));
                        }
                        FriendshipPush::Ignored(ignored)
                    }
                    friendship::FRIEND_STATUSES => {
                        if count != self.friends.len() {
                            return Err(r.bad(
                                "friend statuses for another number of friends than the client holds",
                            ));
                        }
                        let mut statuses = Vec::with_capacity(count);
                        for _ in 0..count {
                            let (account_id, status) = (r.u32()?, r.u8()?);
                            if !self.friends.contains(&account_id) {
                                return Err(r.bad("a status for an account that is no friend"));
                            }
                            statuses.push((account_id, status));
                        }
                        FriendshipPush::Statuses(statuses)
                    }
                    action => {
                        let mut players = Vec::with_capacity(count.min(1024));
                        for _ in 0..count {
                            players.push((
                                r.u32()?,
                                r.str("found name", limits::NAME)?,
                                r.str("found clan tag", limits::CLAN_TAG)?,
                            ));
                        }
                        FriendshipPush::Found { action, players }
                    }
                })
            }
            ServerId::Important => {
                let sender_type = r.u8()?;
                let sender_id = r.u32()?;
                let sender_name = r.str("important sender name", limits::NAME)?;
                let sender_clan_tag = r.str("important sender clan tag", limits::CLAN_TAG)?;
                let kind = r.u8()?;
                let text = r.str("important text", limits::TEXT)?;
                let message_id = r.u32()?;
                Decoded::Important {
                    sender_type,
                    sender_id,
                    sender_name,
                    sender_clan_tag,
                    kind,
                    text,
                    message_id,
                    duplicate: self.pending_important.contains(&message_id),
                }
            }
            ServerId::AdminText => Decoded::AdminText(
                r.str("admin text", limits::NAME)?,
                r.str("admin text", limits::NAME)?,
            ),
            ServerId::AdminTextClear => {
                r.u8()?;
                Decoded::AdminTextClear
            }
            ServerId::PremiumStore => {
                let seconds = r.u32()?;
                let values = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
                let factor = r.f32()?;
                // 0x28A547: every premium offer is priced
                // `value * (1 - factor)`.
                if !(0.0..1.0).contains(&factor) {
                    return Err(r.bad("a premium offer whose discount is no share under 1"));
                }
                Decoded::PremiumStore {
                    seconds,
                    values,
                    factor,
                }
            }
            ServerId::PremiumDiscount => {
                let discount = r.f32()?;
                // `LobbyMenu.show_premium_discount` divides by
                // `1 - discount` for the old price.
                if !(0.0..1.0).contains(&discount) {
                    return Err(r.bad("a premium discount that is no share under 1"));
                }
                Decoded::PremiumDiscount(discount)
            }
            ServerId::SignedOut => Decoded::SignedOut,
        })
    }
}
