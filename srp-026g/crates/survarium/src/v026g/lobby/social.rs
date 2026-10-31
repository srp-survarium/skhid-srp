//! Chat, friends, squads and clans on the lobby link, and the small pushes
//! (`social.md`, `login_chat.md` §5).

use super::push::{NameRefusal, write_name_check};
use super::requests::id;
use super::{ServerId, ServerMessage, WireError, Writer, at_most, limits};

/// Per-recipient rendering policy. Absence of a policy means normal delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ChatMuteMode {
    Hidden = 1,
    Placeholder = 2,
}

/// SRP extension of `comment_about_user`: kind 254, mode 0/1/2. This uses
/// the existing name + two bytes transport, never the report/praise kinds.
pub const CHAT_MUTE_ACTION: u8 = 254;
pub const MUTED_CHAT_TEXT: &str = "<muted>";

/// `message_channel_enum` (`social.md` §1.1).
pub mod channel {
    pub const NONE: u8 = 0;
    pub const GENERAL: u8 = 1;
    pub const SYSTEM: u8 = 2;
    pub const CLAN: u8 = 3;
    pub const PRIVATE: u8 = 4;
    pub const MATCH: u8 = 5;
    pub const TEAM_1: u8 = 6;
    pub const TEAM_2: u8 = 7;
    pub const SQUAD: u8 = 8;
    pub const CLAN_TEAM: u8 = 9;
    /// srp's own: a room's chat (`social.md` §9.8). Not one of the game's
    /// channels: the client sends a line typed in a tab whose
    /// `channel_to_send` is 10 with channel id 0 (0x26AD30: any channel
    /// above 9 goes out with id 0) and shows a received one in the tabs
    /// that list 10 (`add_message` 0x208AD0 passes it on as it is). Only
    /// srp's lobby movie has such a tab (the "Room Chat" tab of
    /// `SrpRooms.as`); the game's own tabs list 10 nowhere in the lobby.
    pub const ROOM: u8 = 10;
}

/// `client_type` of a chat sender (`social.md` §1.2).
pub mod sender {
    /// A player: the only type that carries a clan tag in 0xA2.
    pub const ACCOUNT: u8 = 5;
    /// The stats processor: `#mstats` lines, drop notices, clan lines.
    pub const STATS: u8 = 8;
    /// The clan service.
    pub const CLAN: u8 = 9;
}

/// `important_message_types` (`social.md` §1.4).
pub mod important {
    pub const FRIEND_REQUEST: u8 = 0;
    pub const QUEST: u8 = 1;
    pub const SQUAD_INVITE: u8 = 2;
    pub const SQUAD_INVITE_ACCEPTED: u8 = 3;
    pub const REPUTATION_LEVEL: u8 = 4;
    pub const DROP: u8 = 5;
    pub const ITEM_REPAIRED: u8 = 6;
    pub const CLAN_INVITE: u8 = 7;
    pub const CLAN_TEAM_INVITE: u8 = 10;
    pub const REFERRAL_REWARD: u8 = 13;
    pub const SKILL_POINT_REWARD: u8 = 14;
}

/// The sub-types of 0xA3 the client tells apart (`social.md` §1.3).
pub mod friendship {
    pub const FOUND_PLAYERS: u8 = 6;
    pub const FRIEND_LIST: u8 = 7;
    pub const IGNORE_LIST: u8 = 8;
    pub const FRIEND_STATUSES: u8 = 9;
}

/// `A2`: a chat line (`social.md` §1.2). The clan tag is on the wire only
/// for a sender of type [`sender::ACCOUNT`]; send players as that type.
/// A line from [`sender::STATS`] is never shown: on channel 3 it is a clan
/// system line, on channel 4 a drop notice, elsewhere `#mstats [N]`.
pub fn chat_text(
    sender_type: u8,
    sender_id: u32,
    sender_name: &str,
    sender_clan_tag: &str,
    channel: u8,
    body: &str,
) -> ServerMessage {
    let mut writer = Writer::new();
    writer
        .u8(sender_type)
        .u32(sender_id)
        .str(sender_name, limits::NAME);
    if sender_type == sender::ACCOUNT {
        writer.str(sender_clan_tag, limits::CLAN_TAG);
    }
    writer.u8(channel).str(body, limits::TEXT);
    writer.done(ServerId::ChatText)
}

/// The chat line that makes the client ask for a match's results
/// (`41 00 <match id>`): `#mstats [N]` from the stats processor. The
/// client reads N with `strtol(…, base 0)`, so it is written in decimal
/// with no leading zero.
pub fn match_stats_notice(match_id: u32) -> ServerMessage {
    chat_text(
        sender::STATS,
        0,
        "",
        "",
        channel::SYSTEM,
        &format!("#mstats [{match_id}]"),
    )
}

/// A friend as 0xA3 lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Friend {
    pub account_id: u32,
    pub name: String,
    pub clan_tag: String,
    /// Passed raw to Flash (0.21d: 0 lobby, 1 in a game, 2 offline, 3 away;
    /// not re-checked for 0.26g).
    pub status: u8,
}

/// `A3 07`: the whole friend list. The client never asks for it: push it at
/// sign-in and after every change.
pub fn friend_list(friends: &[Friend]) -> Result<ServerMessage, WireError> {
    at_most("friends", friends.len(), u16::MAX.into())?;
    let mut writer = Writer::new();
    writer.u8(friendship::FRIEND_LIST).u16(friends.len() as u16);
    for friend in friends {
        writer
            .u32(friend.account_id)
            .str(&friend.name, limits::NAME)
            .str(&friend.clan_tag, limits::CLAN_TAG)
            .u8(friend.status);
    }
    Ok(writer.done(ServerId::Friendship))
}

/// `A3 08`: the whole ignore list, `(account id, name)`.
pub fn ignore_list(ignored: &[(u32, String)]) -> Result<ServerMessage, WireError> {
    at_most("ignored players", ignored.len(), u16::MAX.into())?;
    let mut writer = Writer::new();
    writer.u8(friendship::IGNORE_LIST).u16(ignored.len() as u16);
    for (account_id, name) in ignored {
        writer.u32(*account_id).str(name, limits::NAME);
    }
    Ok(writer.done(ServerId::Friendship))
}

/// `A3 09`: the status of every friend, `(account id, status)`. The client
/// drops it unless it lists exactly as many friends as it holds, and stops
/// at an id it does not know: send the same friends, in any order, as the
/// last [`friend_list`].
pub fn friend_statuses(statuses: &[(u32, u8)]) -> Result<ServerMessage, WireError> {
    at_most("friend statuses", statuses.len(), u16::MAX.into())?;
    let mut writer = Writer::new();
    writer
        .u8(friendship::FRIEND_STATUSES)
        .u16(statuses.len() as u16);
    for &(account_id, status) in statuses {
        writer.u32(account_id).u8(status);
    }
    Ok(writer.done(ServerId::Friendship))
}

/// `A3 06`: the players a search found, `(account id, name, clan tag)`.
/// (Any sub-type but 7, 8 and 9 is read this way, so the 0.21d `u8 result`
/// answers no longer exist.)
pub fn found_players(found: &[(u32, String, String)]) -> Result<ServerMessage, WireError> {
    at_most("found players", found.len(), u16::MAX.into())?;
    let mut writer = Writer::new();
    writer.u8(friendship::FOUND_PLAYERS).u16(found.len() as u16);
    for (account_id, name, clan_tag) in found {
        writer
            .u32(*account_id)
            .str(name, limits::NAME)
            .str(clan_tag, limits::CLAN_TAG);
    }
    Ok(writer.done(ServerId::Friendship))
}

/// `A4`: an important message (`social.md` §1.4). The client keeps it until
/// it answers `50 <message_id> <answer>`; an id it already holds is
/// ignored. `sender_id` is what it sends back for a clan invite.
pub fn important_message(
    sender_type: u8,
    sender_id: u32,
    sender_name: &str,
    sender_clan_tag: &str,
    kind: u8,
    text: &str,
    message_id: u32,
) -> ServerMessage {
    Writer::new()
        .u8(sender_type)
        .u32(sender_id)
        .str(sender_name, limits::NAME)
        .str(sender_clan_tag, limits::CLAN_TAG)
        .u8(kind)
        .str(text, limits::TEXT)
        .u32(message_id)
        .done(ServerId::Important)
}

/// One member in a squad info (0xA1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SquadMember {
    /// What the client sends back to drop the member (`4A 03`).
    pub account_id: u32,
    /// The member's kit. For the receiving player's own entry one of its
    /// own kit ids; for a mate an id that is none of them, so the client
    /// asks for the mate (`34 00 <account> <kit>`).
    pub kit_id: u32,
    /// Compared with the revision of the character the client shows;
    /// another value makes it ask again.
    pub kit_revision: u32,
    pub name: String,
    pub online: bool,
    pub ready: bool,
}

/// `A1`: the player's squad (`social.md` §2.1), or none for `squad_id` 0
/// (then nothing else is sent). `commander_index` is the commander's place
/// in `members`: a soldier's Play is locked while that member is ready.
pub fn squad_info(
    squad_id: u32,
    members: &[SquadMember],
    commander_index: u8,
    i_am_commander: bool,
    revision: u32,
) -> Result<ServerMessage, WireError> {
    let mut writer = Writer::new();
    writer.u32(squad_id);
    if squad_id == 0 {
        return Ok(writer.done(ServerId::SquadInfo));
    }
    at_most("squad members", members.len(), limits::SQUAD_MEMBERS)?;
    if usize::from(commander_index) >= members.len().max(1) {
        return Err(WireError::BadValue("a commander index past the members"));
    }
    writer.u8(members.len() as u8).u8(commander_index);
    for member in members {
        writer
            .u32(member.account_id)
            .u32(member.kit_id)
            .u32(member.kit_revision)
            .str(&member.name, limits::NAME)
            .bool(member.online)
            .bool(member.ready);
    }
    writer.bool(i_am_commander).u32(revision);
    Ok(writer.done(ServerId::SquadInfo))
}

/// `9C`: the clan-team battle limits (`lobby.md` §2.20): the least and most
/// members a clan team may press Play with, and no players needed to
/// register a clan ([`clan_limits`] has that too).
pub fn clan_team_limits(min_members: u8, max_members: u8) -> ServerMessage {
    clan_limits(0, min_members, max_members)
}

/// `9C`: the numbers of the clan rules (`social.md` §5):
/// - the players a clan needs to be registered (PROVEN: the registration
///   window shows "ready n / this", 0x2A0880, and its Register button is
///   on only with that many accepted, 0x2A0F90);
/// - the least and most players (the commander and the lineup) a clan team
///   may press Play with (PROVEN: `update_play_button_state` 0x21FEE9).
///
/// The fourth byte is stored and never read by the client; sent as 0.
pub fn clan_limits(players_to_register: u8, team_min: u8, team_max: u8) -> ServerMessage {
    Writer::new()
        .u8(players_to_register)
        .u8(team_min)
        .u8(team_max)
        .u8(0)
        .done(ServerId::ClanTeamLimits)
}

/// `9E`: the video tutorial link, the answer to `45 <language>`
/// (`lobby.md` §2.22): at most 245 UTF-16 units.
pub fn video_link(show: bool, url: &str) -> ServerMessage {
    Writer::new()
        .bool(show)
        .wide(url, limits::WIDE_TEXT)
        .done(ServerId::VideoLink)
}

/// `A5`: the admin text, two lines (`social.md` §5).
pub fn admin_text(first: &str, second: &str) -> ServerMessage {
    Writer::new()
        .str(first, limits::NAME)
        .str(second, limits::NAME)
        .done(ServerId::AdminText)
}

/// `A6`: clears the admin text.
pub fn admin_text_clear() -> ServerMessage {
    Writer::new().u8(0).done(ServerId::AdminTextClear)
}

/// `A8`: the premium discount store / the offer after a match
/// (`social.md` §5): `u32 seconds` (shown in hours), four u32 and an f32
/// whose meanings are not known.
pub fn premium_store(seconds: u32, values: [u32; 4], factor: f32) -> ServerMessage {
    let mut writer = Writer::new();
    writer.u32(seconds);
    for value in values {
        writer.u32(value);
    }
    writer.f32(factor).done(ServerId::PremiumStore)
}

/// `A8`, named: the offer a player without premium gets after a match
/// (`social.md` §5; client 0x274FF8 → 0x28A400, PROVEN). The client lists
/// every premium offer of the shop (0x95, kind 0) at
/// `round(value * (1 - discount))` (`root.set_premium_discount_store`) and
/// opens `root.show_premium_post_reward(money, experience, reputation,
/// hours, time_left, discount, faction)`: "buy premium now and get these
/// too", with the faction the client holds. `premium_seconds` reaches the
/// window in whole hours; `time_left` (seconds) runs down on the premium
/// button. The answer is `47 05`. `discount` is a share, 0.0..1.0.
pub fn premium_post_reward(
    premium_seconds: u32,
    money: u32,
    experience: u32,
    reputation: u32,
    time_left: u32,
    discount: f32,
) -> ServerMessage {
    premium_store(
        premium_seconds,
        [money, experience, reputation, time_left],
        discount,
    )
}

/// `A9`: shows a premium discount.
pub fn premium_discount(discount: f32) -> ServerMessage {
    Writer::new().f32(discount).done(ServerId::PremiumDiscount)
}

/// `AA`: signs the client out to its login screen with
/// `st_login_menu_status_sign_in_already_online`: for the connection an
/// account loses when it signs in elsewhere.
pub fn signed_out() -> ServerMessage {
    Writer::new().done(ServerId::SignedOut)
}

// ---- clans (`social.md` §3) ----

/// A rank of a clan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanRank {
    pub name: String,
    /// Bit 0 battle commander, 1 daily message, 2 change rank, 3 kick,
    /// 4 invite (PROVEN: `set_clan_manage_permissions` 0x2A11A0 unpacks the
    /// bits into its five named fields in that order).
    pub permissions: u8,
    pub is_default: bool,
}

/// A member of a registered clan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanMember {
    pub account_id: u32,
    pub name: String,
    pub rank: u8,
    pub level: u8,
    pub online: bool,
}

/// A member of a clan still being registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanApplicant {
    pub account_id: u32,
    pub name: String,
    pub rank: u8,
    /// 0 accepted (counted as ready), 1 declined, 2 no answer yet
    /// (`ClanInviteRenderer.statusMap`; PROVEN in the lobby SWF).
    pub status: u8,
}

/// A member of a clan team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanTeamMember {
    pub account_id: u32,
    pub name: String,
    /// 0 the commander, 1 the lineup, 2 the reserve, 3 applications
    /// (`TournamentWindow.STATE_*`; PROVEN in the lobby SWF). A role of 0
    /// or 1 counts for a battle. The window wants the list grouped by
    /// role, the commander first.
    pub role: u8,
    pub ready: bool,
}

/// The player's own place in a clan team (0x9A type 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanTeamSeat {
    pub in_clan_team: bool,
    pub i_am_commander: bool,
    /// The receiving player's **own** ready state, whatever its name says
    /// (PROVEN: on `83 30` the commander and the soldier alike send the
    /// opposite of it, 0x276810). A soldier's Play is gated on the
    /// commander's `ready` in the member list instead.
    pub commander_ready: bool,
    pub clan_team_id: u32,
}

/// What 0x9A carries, by its first byte (`read_clan_info`).
#[derive(Clone, Debug, PartialEq)]
pub enum ClanInfo {
    /// Type 0 with clan id 0: the player has no clan.
    None,
    /// Type 0: a registered clan.
    Registered {
        clan_id: u32,
        name: String,
        tag: String,
        ranks: Vec<ClanRank>,
        members: Vec<ClanMember>,
        /// The index of the receiving player in `members` (PROVEN: the
        /// client takes its own permissions from the rank of that member,
        /// 0x2A11A0).
        unknown: u16,
        /// The receiving player commands the clan: window state 4
        /// ("manage"), else 3 (PROVEN with `ManageClanWindow.setState`).
        flag: bool,
        /// `set_clan_progress(minimum, maximum, current, players, level)`
        /// (PROVEN, 0x2A1480): the clan's level, then `players` (the most
        /// members it may have), `minimum` and `maximum` (the experience
        /// the level starts and ends at) and `current` (its experience).
        level: u8,
        progress: [u32; 4],
    },
    /// Type 0: a clan whose registration is under way.
    Registering {
        clan_id: u32,
        name: String,
        tag: String,
        ranks: Vec<ClanRank>,
        members: Vec<ClanApplicant>,
        /// The index of the receiving player in `members` (not used by the
        /// client while the clan is not registered).
        unknown: u16,
        /// The receiving player founded it: window state 1 (it invites
        /// and registers), else 2 (invited).
        flag: bool,
        /// The registration is paid in full (the client keeps it once set).
        paid: bool,
        /// The silver paid so far (the client keeps the most it saw).
        money: u32,
        /// 0, 1, 2: the name, the abbreviation or both were refused; 3 or
        /// more: none.
        reg_state: u8,
    },
    /// Type 1: the daily message, at most 245 UTF-16 units.
    DailyMessage { clan_id: u32, text: String },
    /// Type 2: the members changed.
    Members {
        clan_id: u32,
        members: Vec<ClanMember>,
        unknown: u16,
        flag: bool,
    },
    /// Type 3: the ranks changed.
    Ranks { clan_id: u32, ranks: Vec<ClanRank> },
    /// Type 4: the clan team; `seat` is `None` when the player is in none.
    ClanTeam {
        clan_id: u32,
        members: Vec<ClanTeamMember>,
        seat: Option<ClanTeamSeat>,
    },
    /// Type 5: the clan's name and abbreviation at start-up. Read with no
    /// cut, so they are cut here.
    Startup {
        clan_id: u32,
        name: String,
        tag: String,
    },
    /// Type 6: non-zero shows `st_clan_was_disbanded`.
    Disbanded { show: bool },
}

fn write_ranks(writer: &mut Writer, ranks: &[ClanRank]) -> Result<(), WireError> {
    at_most("clan ranks", ranks.len(), limits::CLAN_RANKS)?;
    writer.u8(ranks.len() as u8);
    for rank in ranks {
        writer
            .str(&rank.name, limits::RANK_NAME)
            .u8(rank.permissions)
            .bool(rank.is_default);
    }
    Ok(())
}

fn write_members(writer: &mut Writer, members: &[ClanMember]) -> Result<(), WireError> {
    at_most("clan members", members.len(), limits::CLAN_MEMBERS)?;
    writer.u32(members.len() as u32);
    for member in members {
        writer
            .u32(member.account_id)
            .str(&member.name, limits::NAME)
            .u8(member.rank)
            .u8(member.level)
            .bool(member.online);
    }
    Ok(())
}

/// `9A`: clan info (`social.md` §3.2).
pub fn clan_info(info: &ClanInfo) -> Result<ServerMessage, WireError> {
    let mut writer = Writer::new();
    let no_clan = WireError::BadValue("clan id 0 means no clan");
    match info {
        ClanInfo::None => {
            writer.u8(0).u32(0);
        }
        ClanInfo::Registered {
            clan_id,
            name,
            tag,
            ranks,
            members,
            unknown,
            flag,
            level,
            progress,
        } => {
            if *clan_id == 0 {
                return Err(no_clan);
            }
            writer
                .u8(0)
                .u32(*clan_id)
                .str(name, limits::NAME)
                .str(tag, limits::ACCOUNT_CLAN_TAG)
                .u8(1);
            write_ranks(&mut writer, ranks)?;
            write_members(&mut writer, members)?;
            writer.u16(*unknown).bool(*flag).u8(*level);
            for value in progress {
                writer.u32(*value);
            }
        }
        ClanInfo::Registering {
            clan_id,
            name,
            tag,
            ranks,
            members,
            unknown,
            flag,
            paid,
            money,
            reg_state,
        } => {
            if *clan_id == 0 {
                return Err(no_clan);
            }
            writer
                .u8(0)
                .u32(*clan_id)
                .str(name, limits::NAME)
                .str(tag, limits::ACCOUNT_CLAN_TAG)
                .u8(0);
            write_ranks(&mut writer, ranks)?;
            at_most("clan members", members.len(), limits::CLAN_MEMBERS)?;
            writer.u32(members.len() as u32);
            for member in members {
                writer
                    .u32(member.account_id)
                    .str(&member.name, limits::NAME)
                    .u8(member.rank)
                    .u8(member.status);
            }
            writer
                .u16(*unknown)
                .bool(*flag)
                .bool(*paid)
                .u32(*money)
                .u8(*reg_state);
        }
        ClanInfo::DailyMessage { clan_id, text } => {
            if *clan_id == 0 {
                return Err(no_clan);
            }
            writer.u8(1).u32(*clan_id).wide(text, limits::WIDE_TEXT);
        }
        ClanInfo::Members {
            clan_id,
            members,
            unknown,
            flag,
        } => {
            if *clan_id == 0 {
                return Err(no_clan);
            }
            writer.u8(2).u32(*clan_id);
            write_members(&mut writer, members)?;
            writer.u16(*unknown).bool(*flag);
        }
        ClanInfo::Ranks { clan_id, ranks } => {
            if *clan_id == 0 {
                return Err(no_clan);
            }
            writer.u8(3).u32(*clan_id);
            write_ranks(&mut writer, ranks)?;
        }
        ClanInfo::ClanTeam {
            clan_id,
            members,
            seat,
        } => {
            if *clan_id == 0 {
                return Err(no_clan);
            }
            at_most(
                "clan team members",
                members.len(),
                limits::CLAN_TEAM_MEMBERS,
            )?;
            writer.u8(4).u32(*clan_id).u8(members.len() as u8);
            for member in members {
                writer
                    .u32(member.account_id)
                    .str(&member.name, limits::NAME)
                    .u8(member.role)
                    .bool(member.ready);
            }
            match seat {
                Some(seat) => {
                    writer
                        .u8(1)
                        .bool(seat.in_clan_team)
                        .bool(seat.i_am_commander)
                        .bool(seat.commander_ready)
                        .u32(seat.clan_team_id);
                }
                None => {
                    writer.u8(0);
                }
            }
        }
        ClanInfo::Startup { clan_id, name, tag } => {
            if *clan_id == 0 {
                return Err(no_clan);
            }
            writer
                .u8(5)
                .u32(*clan_id)
                .u8(1)
                .str(name, limits::NAME)
                .str(tag, limits::CLAN_TAG);
        }
        ClanInfo::Disbanded { show } => {
            writer.u8(6).bool(*show);
        }
    }
    Ok(writer.done(ServerId::ClanInfo))
}

fn profile_event(event: u8) -> Writer {
    let mut writer = Writer::new();
    writer.u8(id::PROFILE).u8(event);
    writer
}

/// `83 30`: clan-team play permitted; the client answers `49 20`/`49 21`.
pub fn permitted_clan_play() -> ServerMessage {
    Writer::new().u8(id::CLAN_PLAY).done(ServerId::Permitted)
}

/// `83 49 06`: the clan's name was reserved.
pub fn permitted_clan_name_reserved() -> ServerMessage {
    profile_event(0x06).done(ServerId::Permitted)
}

/// `83 49 08`: the answer to a clan name check (`49 08`); kept only when
/// the name equals the one asked. [`NameRefusal::Pattern`] is not among its
/// texts.
pub fn permitted_clan_name_check(
    name: &str,
    refusal: Option<NameRefusal>,
) -> Result<ServerMessage, WireError> {
    clan_check(0x08, name, refusal)
}

/// `83 49 09`: the answer to a clan abbreviation check (`49 09`).
pub fn permitted_clan_tag_check(
    tag: &str,
    refusal: Option<NameRefusal>,
) -> Result<ServerMessage, WireError> {
    clan_check(0x09, tag, refusal)
}

fn clan_check(
    event: u8,
    name: &str,
    refusal: Option<NameRefusal>,
) -> Result<ServerMessage, WireError> {
    if refusal == Some(NameRefusal::Pattern) {
        return Err(WireError::BadValue("a clan check has no pattern refusal"));
    }
    let mut writer = profile_event(event);
    write_name_check(&mut writer, name, refusal);
    Ok(writer.done(ServerId::Permitted))
}

/// `83 49 0B`: shows `st_clan_invite_1 <name> st_clan_invite_2`.
pub fn permitted_clan_invite_sent(target_name: &str) -> ServerMessage {
    profile_event(0x0B)
        .str(target_name, limits::NAME)
        .done(ServerId::Permitted)
}

/// `83 49 0F`: the answer to `49 0F` (invite to the clan). `None` refuses
/// (status 1, nothing more). Otherwise the client sends the long `4D` of
/// type 7 with `extra`, the target and the text, then `49 0B <target>`.
pub fn permitted_clan_invite(invite: Option<(u32, &str, &str)>) -> ServerMessage {
    let mut writer = profile_event(0x0F);
    match invite {
        None => {
            writer.u8(1);
        }
        Some((extra, target_name, text)) => {
            writer
                .u8(0)
                .u32(extra)
                .str(target_name, limits::NAME)
                .str(text, limits::TEXT);
        }
    }
    writer.done(ServerId::Permitted)
}

/// `83 49 12`: the answer to a rank-up request; `true` opens the client's
/// confirmation.
pub fn permitted_rank_up(confirm: bool) -> ServerMessage {
    profile_event(0x12).bool(confirm).done(ServerId::Permitted)
}

/// `83 49 19`: the answer to `49 19` (invite to the clan team): the client
/// sends the long `4D` of type 10.
pub fn permitted_clan_team_invite(extra: u32, target_name: &str, text: &str) -> ServerMessage {
    profile_event(0x19)
        .u32(extra)
        .str(target_name, limits::NAME)
        .str(text, limits::TEXT)
        .done(ServerId::Permitted)
}
