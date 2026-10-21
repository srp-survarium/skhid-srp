//! Every message the 0.26g client sends on the lobby link, 0x2E..=0x50
//! (`lobby_requests.md`, `social.md` §1.5 and §3.3, `login_chat.md` §5).
//!
//! [`parse`] takes the id and the body the transport delivered (after its
//! `x` byte). The client writes booleans as `00`/`FF`; "raw" bytes are a
//! Flash number's low byte. A query's leading `00` is skipped here.
//! Strings are `u8 len` and bytes; the client cuts nothing but its own
//! nicknames, and its length byte wraps at 256, so a text is whatever the
//! length byte says. Bytes that are no UTF-8 are replaced.

use std::fmt;

/// The client's lobby message ids.
pub mod id {
    pub const SIGN_IN: u8 = 0x2E;
    pub const PLAY: u8 = 0x2F;
    pub const CLAN_PLAY: u8 = 0x30;
    pub const LEAVE: u8 = 0x31;
    pub const AFTER_SIGN_IN: u8 = 0x32;
    pub const QUERY_SQUAD_MEMBER: u8 = 0x34;
    pub const QUERY_SKILLS: u8 = 0x37;
    pub const QUERY_REPUTATIONS: u8 = 0x39;
    pub const QUERY_RANKING: u8 = 0x3B;
    pub const QUERY_QUESTS: u8 = 0x3C;
    pub const QUERY_PLAYER_DETAILS: u8 = 0x3D;
    pub const QUERY_SERVICES: u8 = 0x3E;
    pub const QUERY_UNLOCKED_FACTIONS: u8 = 0x40;
    pub const QUERY_MATCH_STATS: u8 = 0x41;
    pub const QUERY_DROP: u8 = 0x42;
    pub const QUERY_DAILY_BONUS: u8 = 0x43;
    pub const QUERY_VIDEO_LINK: u8 = 0x45;
    pub const INVENTORY: u8 = 0x46;
    pub const SHOP: u8 = 0x47;
    pub const SKILLS: u8 = 0x48;
    pub const PROFILE: u8 = 0x49;
    pub const SQUAD: u8 = 0x4A;
    pub const QUEST: u8 = 0x4B;
    pub const CHAT_TEXT: u8 = 0x4C;
    pub const IMPORTANT: u8 = 0x4D;
    pub const FRIENDSHIP: u8 = 0x4E;
    pub const SUBSCRIPTION: u8 = 0x4F;
    pub const IMPORTANT_READ: u8 = 0x50;

    /// The ids in 0x2E..=0x50 the client never makes
    /// (`lobby_requests.md` §3).
    pub const NEVER_SENT: [u8; 7] = [0x33, 0x35, 0x36, 0x38, 0x3A, 0x3F, 0x44];
}

/// The slot id that means "the inventory" in a move and a purchase.
pub const INVENTORY_SLOT: u8 = 100;

/// Added to the amount of `47 04` by srp's exchange window when it buys gold
/// with silver (`MessageMoneyExchange.BUY_GOLD`). The stock window sends only gold
/// for silver, and no account has this much gold.
pub const BUY_PREMIUM: u32 = 0x4000_0000;

/// One entry of `46 00`: `relocate_item_descr`, 16 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Relocation {
    pub item_id: u32,
    pub dict_id: u16,
    pub source_slot: u8,
    pub target_slot: u8,
    pub count: u32,
    pub count_in_inventory: u32,
}

/// How a repair is paid (`47 03`; `MessageRepair.onMessageAccept`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairPayment {
    /// Kind 0: silver.
    Silver,
    /// Kind 1: gold.
    Gold,
    /// Kind 2: a repair kit (voucher), by its item id.
    Kit { kit_item_id: u32 },
    /// Another kind byte.
    Other(u8),
}

/// A clan request: `49 06..=23` (`social.md` §3.3). The layouts are PROVEN,
/// the meanings LIKELY from the Flash callback names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClanRequest {
    /// `06`.
    ReserveName { name: String, tag: String },
    /// `07`.
    Create,
    /// `08`.
    CheckName { name: String },
    /// `09`.
    CheckTag { tag: String },
    /// `0A`: 0 accepts, 1 declines.
    AnswerInvite { inviter_id: u32, answer: u8 },
    /// `0B`: sent by the client itself after `83 49 0F`.
    InviteSent { target_name: String },
    /// `0C`.
    BuyRegistration { value: u32 },
    /// `0D`: leave, disband or cancel the registration (the server decides
    /// by the player's role).
    LeaveOrDisband,
    /// `0E`: asks for a 0x9A of this type (0 or 4).
    QueryInfo { what: u8 },
    /// `0F`.
    Invite { target_name: String },
    /// `10 <rank> <value> <permission>` (PROVEN: the sender 0x273710
    /// writes its three arguments in that order, and the five
    /// `update_*_permission` callbacks push the constant last): the rank's
    /// index, the checkbox (0 or 1), and 0 invite, 1 kick, 2 change rank,
    /// 3 daily message, 4 battle commander.
    SetPermission { rank: u8, permission: u8, value: u8 },
    /// `11`.
    ChangeRank { account_id: u32, up: bool },
    /// `12`.
    AskRankUp { account_id: u32 },
    /// `13`.
    Kick { account_id: u32 },
    /// `14`.
    CreateRank { name: String },
    /// `15`.
    RemoveRank { rank: u8 },
    /// `16`: `u16 count` and UTF-16LE units (the writer 0x26E210; PROVEN).
    SetDailyMessage { text: String },
    /// `17`.
    CreateTeam,
    /// `18`.
    AskToJoinTeam,
    /// `19`.
    InviteToTeam { target_name: String },
    /// `1A`.
    AnswerTeamInvite { inviter_id: u32, answer: u8 },
    /// `1B`.
    LeaveTeam,
    /// `1C`.
    DisbandTeam,
    /// `1D`.
    SetTeamMemberStatus { member_name: String, status: u8 },
    /// `1E`.
    OustCommander,
    /// `1F`.
    RemoveTeamMember { member_name: String },
    /// `20`: a team soldier's ready toggle (after `83 30`).
    SoldierReady { ready: bool },
    /// `21`: the team commander's ready toggle (after `83 30`).
    CommanderReady { ready: bool },
    /// `22`.
    SetTeamCommander { member_name: String },
    /// `23`.
    MatchClosed,
}

/// `4E`: a friendship action (`social.md` §1.5). Adding a friend goes
/// through `4D` type 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FriendshipAction {
    /// `02`.
    RemoveFriend { account_id: u32 },
    /// `03`.
    Ignore { account_id: u32 },
    /// `04`.
    IgnoreByName { name: String },
    /// `05`.
    StopIgnoring { account_id: u32 },
    /// `06`.
    FindPlayers { name: String },
}

/// A message of the client.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientMessage {
    /// `2E`: the connect payload.
    SignIn {
        session_id: u32,
        /// The client has no shop records yet (wants 0x95).
        need_shop_items: bool,
    },
    /// `2F`: Play with the active kit.
    Play { kit_id: u32 },
    /// `30`: clan-team Play.
    ClanPlay { kit_id: u32 },
    /// `31`: leave the queue (`left_match` false) or a match left (true).
    Leave { order_id: u32, left_match: bool },
    /// `32 00 <bool>`: sent right after 0x80, echoing its flag. What it
    /// asks for is not known: accept it.
    AfterSignIn { flag: bool },

    /// `34`: a squad mate's account (→ 0x88).
    QuerySquadMember { account_id: u32, kit_id: u32 },
    /// `37` (→ 0x8C).
    QuerySkills,
    /// `39` (→ 0x8E).
    QueryReputations,
    /// `3B`: a page of the ranking (→ 0x91).
    QueryRanking { start: u32, count: u8 },
    /// `3C` (→ 0x92).
    QueryQuests,
    /// `3D`: a player of the results screen (→ 0x93).
    QueryPlayerDetails { player_id: u32, match_id: u32 },
    /// `3E` (→ 0x94).
    QueryServices,
    /// `40` (→ 0x96).
    QueryUnlockedFactions,
    /// `41` (→ 0x97).
    QueryMatchStats { match_id: u32 },
    /// `42` (→ 0x98, LIKELY).
    QueryDrop { drop_id: u32 },
    /// `43` (→ 0x99, LIKELY).
    QueryDailyBonus,
    /// `45`: the UI language, 0 english, 1 russian, 2 ukrainian, 3 german,
    /// 4 polish (→ 0x9E, LIKELY). No leading `00`.
    QueryVideoLink { language: u8 },

    /// `46 00`: move items between a kit's slots and the inventory.
    MoveItems { kit_id: u32, moves: Vec<Relocation> },
    /// `46 01`: put a cosmetic on an item: `a0` the host item, `a1` the
    /// cosmetic's item id, `a2` the slot (PROVEN, `items.md` §3).
    AttachUpgrade {
        kit_id: u32,
        a0: u32,
        a1: u32,
        a2: u8,
    },
    /// `46 02`: take a cosmetic off an item: `upgrade_item_id` off
    /// `host_item_id`. The host is patch 30's (`client-patches`, 0x29AAD5:
    /// the upgrade window's item, which Flash's `drop_upgrade_item` passes
    /// first). A client without patch 30 (every released revision up to
    /// 16) sends the kit and the upgrade alone (0x2708C0): `host_item_id`
    /// is then 0, and the stock client takes the upgrade off the first item
    /// of its inventory that holds it (0x275220 → find_if 0x277CF0).
    DetachUpgrade {
        kit_id: u32,
        upgrade_item_id: u32,
        host_item_id: u32,
    },
    /// `46 03`: the match's id, then the drop card's item id (PROVEN:
    /// Flash `use_drop_card(match id, voucher id)`, written in that order
    /// by 0x288D00 -> 0x270AE0).
    UseDropCard { a0: u32, a1: u32 },
    /// `46 04`.
    UseItem { item_id: u32 },
    /// `46 05`: the decal editor's value for a cosmetic on an item:
    /// `item_id` the host, `a` the cosmetic's item id (PROVEN, `items.md`
    /// §3).
    ApplyDecal {
        kit_id: u32,
        item_id: u32,
        a: u32,
        value: u64,
    },
    /// `46 06`.
    OpenReroll { item_id: u32 },
    /// `46 07`: change a modification; `flag`: paid in gold.
    RerollModifier {
        item_id: u32,
        modification_id: u32,
        flag: bool,
    },
    /// `46 08`: take one of the three offered. `new_modification_id` is
    /// its place among them (0..=2), not an id (`items.md` §3).
    ChooseModifier {
        item_id: u32,
        old_modification_id: u32,
        new_modification_id: u32,
    },
    /// `46 09`: upgrade the item `a0` with the donor item `a1`; `flag`:
    /// paid in gold (PROVEN, `items.md` §3).
    Craft { a0: u32, a1: u32, flag: bool },
    /// `46 0A`: the tutorial entered its "choose a weapon" step
    /// (0x2A4678) and waits for the inventory (`89`): `tutorial_model`.
    /// (No weapon is chosen yet, whatever the name says.)
    TutorialWeaponChosen,

    /// `47 00`: buy `count` steps of an item (a clip of ammo, else one
    /// item) from `faction`.
    Buy {
        dict_id: u16,
        count: u32,
        faction: u8,
        target_slot: u8,
        kit_id: u32,
    },
    /// `47 01`: sell `amount` steps of an item.
    Sell { item_id: u32, amount: u32 },
    /// `47 02`: buy a shop offer; `x` is 0 except from the results screen.
    BuyOffer { offer_id: u32, x: u32, factor: f32 },
    /// `47 03`.
    Repair {
        item_id: u32,
        payment: RepairPayment,
    },
    /// `47 04`: gold for silver (the stock window).
    ExchangePremium { amount: u32 },
    /// `47 04` with [`BUY_PREMIUM`] added: silver for so much gold. srp's
    /// own: its exchange window (`client-patches/exchange`) sends it; the
    /// stock client has no way to.
    BuyPremium { gold: u32 },
    /// `47 05`.
    AcceptPremiumWithBonuses {
        a0: u32,
        a1: f32,
        a2: u32,
        a3: u32,
        a4: u32,
        faction: u8,
    },
    /// `47 06`.
    AcceptPremiumWithDiscount { a0: u32, a1: f32 },

    /// `48 00`: the account's whole set of skills, `(skill id, points)`.
    SetSkills { skills: Vec<(u16, u8)> },
    /// `48 01`: reset the skills of a branch (the window's tab: 0 for
    /// skills 101.., 1 for 201..), paid in silver (0) or gold (1)
    /// (`MessageAskReroll`: `reroll_ok_clicked(branch, payment)`; PROVEN in
    /// the UI).
    ResetSkills { branch: u8, payment: u8 },

    /// `49 00`.
    RenameKit { kit_id: u32, name: String },
    /// `49 01`: the account's two boxes, as the lobby's number
    /// (`LocalPlayerProfile.getAutoBuyState`; PROVEN): bit 0 is autobuy
    /// (`on`), bit 1 auto-repair.
    SetAutobuy { on: bool, repair: bool },
    /// `49 02`.
    SetActiveKit { kit_id: u32 },
    /// `49 03`.
    CheckNickname { nickname: String },
    /// `49 04`: the u32 is LIKELY the rename ticket's item id.
    ChangeNickname {
        nickname: String,
        ticket_item_id: u32,
    },
    /// `49 05`: GUESS a report or praise of a player.
    CommentAboutUser { nickname: String, a: u8, b: u8 },
    /// `49 06..=23`.
    Clan(ClanRequest),
    /// `49 24`.
    HideVideoLink,

    /// `4A 01`.
    DisbandSquad,
    /// `4A 02`: add the player who accepted the invite.
    AddSquadMember { name: String },
    /// `4A 03`.
    DropSquadMember { account_id: u32 },
    /// `4A 04`.
    LeaveSquad,

    /// `4B 00`: take quests from this faction (answer `83 4B`).
    ChooseQuestFaction { faction: u8 },
    /// `4B 01`: LIKELY a tutorial step done (1..=4).
    TutorialStep { step: u8 },
    /// `4B 02`.
    SkipTutorial,

    /// `4C`: a chat line.
    ChatText {
        channel_id: u32,
        receiver: String,
        channel: u8,
        body: String,
    },
    /// `4D`: an important message for another player (a friend request, a
    /// squad invite or its acceptance; with `extra` a clan or clan-team
    /// invite). Its first u32 is always 0.
    Important {
        target_name: String,
        kind: u8,
        text: String,
        extra: Option<u32>,
    },
    /// `4E`.
    Friendship(FriendshipAction),
    /// `4F`: the chat channels' ids by channel number, and the language
    /// (0 english, 1 russian, 2 ukrainian, 3 german, 4 polish).
    Subscribe {
        channel_ids: [u32; 10],
        language: u8,
    },
    /// `50`: the answer to an important message: 0 accept, 1 decline.
    ImportantRead { message_id: u32, answer: u8 },
}

/// Why a client message was not read.
#[derive(Clone, Debug, PartialEq)]
pub enum ParseError {
    /// An id the client never sends (or none of the lobby's).
    UnknownId(u8),
    /// A sub-type the id does not have.
    UnknownSubType { id: u8, sub: u8 },
    /// The body ends inside a field.
    Truncated { id: u8 },
    /// The message was read whole and bytes are left. Nothing in the notes
    /// has such bytes, so it is reported; the message is given for a
    /// caller that would rather carry on.
    Trailing {
        message: Box<ClientMessage>,
        extra: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "unknown lobby message {id:#04X}"),
            Self::UnknownSubType { id, sub } => {
                write!(f, "lobby message {id:#04X}: unknown sub-type {sub:#04X}")
            }
            Self::Truncated { id } => write!(f, "lobby message {id:#04X} is cut short"),
            Self::Trailing { extra, .. } => write!(f, "{extra} bytes after a lobby message"),
        }
    }
}

impl std::error::Error for ParseError {}

struct Reader<'a> {
    id: u8,
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        if self.rest.len() < len {
            return Err(ParseError::Truncated { id: self.id });
        }
        let (head, tail) = self.rest.split_at(len);
        self.rest = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, ParseError> {
        Ok(self.u8()? != 0)
    }
    fn u16(&mut self) -> Result<u16, ParseError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn u32(&mut self) -> Result<u32, ParseError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    fn u64(&mut self) -> Result<u64, ParseError> {
        let low = self.u32()?;
        let high = self.u32()?;
        Ok(u64::from(high) << 32 | u64::from(low))
    }
    fn f32(&mut self) -> Result<f32, ParseError> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn str(&mut self) -> Result<String, ParseError> {
        let len = self.u8()?;
        Ok(String::from_utf8_lossy(self.take(len.into())?).into_owned())
    }
    /// `u16 count` and UTF-16LE units.
    fn wide(&mut self) -> Result<String, ParseError> {
        let count = self.u16()?;
        let bytes = self.take(usize::from(count) * 2)?;
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }
    /// The `00` every query starts with. Its value is not checked: the
    /// client always writes 0.
    fn query_zero(&mut self) -> Result<(), ParseError> {
        self.u8().map(drop)
    }
    fn unknown_sub(&self, sub: u8) -> ParseError {
        ParseError::UnknownSubType { id: self.id, sub }
    }
}

/// Reads one client message from its id and body.
pub fn parse(id: u8, body: &[u8]) -> Result<ClientMessage, ParseError> {
    let mut reader = Reader { id, rest: body };
    let message = parse_body(&mut reader)?;
    if !reader.rest.is_empty() {
        return Err(ParseError::Trailing {
            message: Box::new(message),
            extra: reader.rest.len(),
        });
    }
    Ok(message)
}

fn parse_body(r: &mut Reader<'_>) -> Result<ClientMessage, ParseError> {
    use ClientMessage as M;
    Ok(match r.id {
        id::SIGN_IN => M::SignIn {
            session_id: r.u32()?,
            need_shop_items: r.bool()?,
        },
        id::PLAY => M::Play { kit_id: r.u32()? },
        id::CLAN_PLAY => M::ClanPlay { kit_id: r.u32()? },
        id::LEAVE => M::Leave {
            order_id: r.u32()?,
            left_match: r.bool()?,
        },
        id::AFTER_SIGN_IN => {
            r.query_zero()?;
            M::AfterSignIn { flag: r.bool()? }
        }
        id::QUERY_SQUAD_MEMBER => {
            r.query_zero()?;
            M::QuerySquadMember {
                account_id: r.u32()?,
                kit_id: r.u32()?,
            }
        }
        id::QUERY_SKILLS => r.query_zero().map(|()| M::QuerySkills)?,
        id::QUERY_REPUTATIONS => r.query_zero().map(|()| M::QueryReputations)?,
        id::QUERY_RANKING => {
            r.query_zero()?;
            M::QueryRanking {
                start: r.u32()?,
                count: r.u8()?,
            }
        }
        id::QUERY_QUESTS => r.query_zero().map(|()| M::QueryQuests)?,
        id::QUERY_PLAYER_DETAILS => {
            r.query_zero()?;
            M::QueryPlayerDetails {
                player_id: r.u32()?,
                match_id: r.u32()?,
            }
        }
        id::QUERY_SERVICES => r.query_zero().map(|()| M::QueryServices)?,
        id::QUERY_UNLOCKED_FACTIONS => r.query_zero().map(|()| M::QueryUnlockedFactions)?,
        id::QUERY_MATCH_STATS => {
            r.query_zero()?;
            M::QueryMatchStats { match_id: r.u32()? }
        }
        id::QUERY_DROP => {
            r.query_zero()?;
            M::QueryDrop { drop_id: r.u32()? }
        }
        id::QUERY_DAILY_BONUS => r.query_zero().map(|()| M::QueryDailyBonus)?,
        id::QUERY_VIDEO_LINK => M::QueryVideoLink { language: r.u8()? },
        id::INVENTORY => parse_inventory(r)?,
        id::SHOP => parse_shop(r)?,
        id::SKILLS => match r.u8()? {
            0 => {
                let count = r.u8()?;
                let mut skills = Vec::with_capacity(count.into());
                for _ in 0..count {
                    skills.push((r.u16()?, r.u8()?));
                }
                M::SetSkills { skills }
            }
            1 => M::ResetSkills {
                branch: r.u8()?,
                payment: r.u8()?,
            },
            sub => return Err(r.unknown_sub(sub)),
        },
        id::PROFILE => parse_profile(r)?,
        id::SQUAD => match r.u8()? {
            1 => M::DisbandSquad,
            2 => M::AddSquadMember { name: r.str()? },
            3 => M::DropSquadMember {
                account_id: r.u32()?,
            },
            4 => M::LeaveSquad,
            sub => return Err(r.unknown_sub(sub)),
        },
        id::QUEST => match r.u8()? {
            0 => M::ChooseQuestFaction { faction: r.u8()? },
            1 => M::TutorialStep { step: r.u8()? },
            2 => M::SkipTutorial,
            sub => return Err(r.unknown_sub(sub)),
        },
        id::CHAT_TEXT => M::ChatText {
            channel_id: r.u32()?,
            receiver: r.str()?,
            channel: r.u8()?,
            body: r.str()?,
        },
        id::IMPORTANT => {
            // Always 0.
            r.u32()?;
            let target_name = r.str()?;
            let kind = r.u8()?;
            let text = r.str()?;
            // The long form (clan and clan-team invites) has one more u32.
            let extra = if r.rest.is_empty() {
                None
            } else {
                Some(r.u32()?)
            };
            M::Important {
                target_name,
                kind,
                text,
                extra,
            }
        }
        id::FRIENDSHIP => M::Friendship(match r.u8()? {
            2 => FriendshipAction::RemoveFriend {
                account_id: r.u32()?,
            },
            3 => FriendshipAction::Ignore {
                account_id: r.u32()?,
            },
            4 => FriendshipAction::IgnoreByName { name: r.str()? },
            5 => FriendshipAction::StopIgnoring {
                account_id: r.u32()?,
            },
            6 => FriendshipAction::FindPlayers { name: r.str()? },
            sub => return Err(r.unknown_sub(sub)),
        }),
        id::SUBSCRIPTION => {
            let mut channel_ids = [0; 10];
            for channel_id in &mut channel_ids {
                *channel_id = r.u32()?;
            }
            M::Subscribe {
                channel_ids,
                language: r.u8()?,
            }
        }
        id::IMPORTANT_READ => M::ImportantRead {
            message_id: r.u32()?,
            answer: r.u8()?,
        },
        other => return Err(ParseError::UnknownId(other)),
    })
}

fn parse_inventory(r: &mut Reader<'_>) -> Result<ClientMessage, ParseError> {
    use ClientMessage as M;
    Ok(match r.u8()? {
        0x00 => {
            let kit_id = r.u32()?;
            let count = r.u8()?;
            let mut moves = Vec::with_capacity(count.into());
            for _ in 0..count {
                moves.push(Relocation {
                    item_id: r.u32()?,
                    dict_id: r.u16()?,
                    source_slot: r.u8()?,
                    target_slot: r.u8()?,
                    count: r.u32()?,
                    count_in_inventory: r.u32()?,
                });
            }
            M::MoveItems { kit_id, moves }
        }
        0x01 => {
            let message = M::AttachUpgrade {
                kit_id: r.u32()?,
                a0: r.u32()?,
                a1: r.u32()?,
                a2: r.u8()?,
            };
            // Eight bytes the client always zeroes.
            r.u64()?;
            message
        }
        0x02 => M::DetachUpgrade {
            kit_id: r.u32()?,
            upgrade_item_id: r.u32()?,
            // A stock client ends here (no patch 30).
            host_item_id: if r.rest.is_empty() { 0 } else { r.u32()? },
        },
        0x03 => M::UseDropCard {
            a0: r.u32()?,
            a1: r.u32()?,
        },
        0x04 => M::UseItem { item_id: r.u32()? },
        0x05 => M::ApplyDecal {
            kit_id: r.u32()?,
            item_id: r.u32()?,
            a: r.u32()?,
            value: r.u64()?,
        },
        0x06 => M::OpenReroll { item_id: r.u32()? },
        0x07 => M::RerollModifier {
            item_id: r.u32()?,
            modification_id: r.u32()?,
            flag: r.bool()?,
        },
        0x08 => M::ChooseModifier {
            item_id: r.u32()?,
            old_modification_id: r.u32()?,
            new_modification_id: r.u32()?,
        },
        0x09 => M::Craft {
            a0: r.u32()?,
            a1: r.u32()?,
            flag: r.bool()?,
        },
        0x0A => M::TutorialWeaponChosen,
        sub => return Err(r.unknown_sub(sub)),
    })
}

fn parse_shop(r: &mut Reader<'_>) -> Result<ClientMessage, ParseError> {
    use ClientMessage as M;
    Ok(match r.u8()? {
        0x00 => {
            let dict_id = r.u16()?;
            let count = r.u32()?;
            let faction = r.u8()?;
            // Always 0.
            r.u8()?;
            M::Buy {
                dict_id,
                count,
                faction,
                target_slot: r.u8()?,
                kit_id: r.u32()?,
            }
        }
        0x01 => M::Sell {
            item_id: r.u32()?,
            amount: r.u32()?,
        },
        0x02 => M::BuyOffer {
            offer_id: r.u32()?,
            x: r.u32()?,
            factor: r.f32()?,
        },
        0x03 => {
            let item_id = r.u32()?;
            let payment = match r.u8()? {
                0 => RepairPayment::Silver,
                1 => RepairPayment::Gold,
                // Only this kind carries the kit's id.
                2 => RepairPayment::Kit {
                    kit_item_id: r.u32()?,
                },
                other => RepairPayment::Other(other),
            };
            M::Repair { item_id, payment }
        }
        0x04 => match r.u32()? {
            amount if amount >= BUY_PREMIUM => M::BuyPremium {
                gold: amount - BUY_PREMIUM,
            },
            amount => M::ExchangePremium { amount },
        },
        0x05 => M::AcceptPremiumWithBonuses {
            a0: r.u32()?,
            a1: r.f32()?,
            a2: r.u32()?,
            a3: r.u32()?,
            a4: r.u32()?,
            faction: r.u8()?,
        },
        0x06 => M::AcceptPremiumWithDiscount {
            a0: r.u32()?,
            a1: r.f32()?,
        },
        sub => return Err(r.unknown_sub(sub)),
    })
}

fn parse_profile(r: &mut Reader<'_>) -> Result<ClientMessage, ParseError> {
    use ClanRequest as C;
    use ClientMessage as M;
    let clan = match r.u8()? {
        0x00 => {
            return Ok(M::RenameKit {
                kit_id: r.u32()?,
                name: r.str()?,
            });
        }
        0x01 => {
            let state = r.u8()?;
            return Ok(M::SetAutobuy {
                on: state & 1 != 0,
                repair: state & 2 != 0,
            });
        }
        0x02 => return Ok(M::SetActiveKit { kit_id: r.u32()? }),
        0x03 => {
            return Ok(M::CheckNickname { nickname: r.str()? });
        }
        0x04 => {
            return Ok(M::ChangeNickname {
                nickname: r.str()?,
                ticket_item_id: r.u32()?,
            });
        }
        0x05 => {
            return Ok(M::CommentAboutUser {
                nickname: r.str()?,
                a: r.u8()?,
                b: r.u8()?,
            });
        }
        0x24 => {
            // Always 0.
            r.u8()?;
            return Ok(M::HideVideoLink);
        }
        0x06 => C::ReserveName {
            name: r.str()?,
            tag: r.str()?,
        },
        0x07 => C::Create,
        0x08 => C::CheckName { name: r.str()? },
        0x09 => C::CheckTag { tag: r.str()? },
        0x0A => C::AnswerInvite {
            inviter_id: r.u32()?,
            answer: r.u8()?,
        },
        0x0B => C::InviteSent {
            target_name: r.str()?,
        },
        0x0C => C::BuyRegistration { value: r.u32()? },
        0x0D => C::LeaveOrDisband,
        0x0E => C::QueryInfo { what: r.u8()? },
        0x0F => C::Invite {
            target_name: r.str()?,
        },
        0x10 => C::SetPermission {
            rank: r.u8()?,
            value: r.u8()?,
            permission: r.u8()?,
        },
        0x11 => C::ChangeRank {
            account_id: r.u32()?,
            up: r.bool()?,
        },
        0x12 => C::AskRankUp {
            account_id: r.u32()?,
        },
        0x13 => C::Kick {
            account_id: r.u32()?,
        },
        0x14 => C::CreateRank { name: r.str()? },
        0x15 => C::RemoveRank { rank: r.u8()? },
        0x16 => C::SetDailyMessage { text: r.wide()? },
        0x17 => C::CreateTeam,
        0x18 => C::AskToJoinTeam,
        0x19 => C::InviteToTeam {
            target_name: r.str()?,
        },
        0x1A => C::AnswerTeamInvite {
            inviter_id: r.u32()?,
            answer: r.u8()?,
        },
        0x1B => C::LeaveTeam,
        0x1C => C::DisbandTeam,
        0x1D => C::SetTeamMemberStatus {
            member_name: r.str()?,
            status: r.u8()?,
        },
        0x1E => C::OustCommander,
        0x1F => C::RemoveTeamMember {
            member_name: r.str()?,
        },
        0x20 => C::SoldierReady { ready: r.bool()? },
        0x21 => C::CommanderReady { ready: r.bool()? },
        0x22 => C::SetTeamCommander {
            member_name: r.str()?,
        },
        0x23 => C::MatchClosed,
        sub => return Err(r.unknown_sub(sub)),
    };
    Ok(M::Clan(clan))
}
