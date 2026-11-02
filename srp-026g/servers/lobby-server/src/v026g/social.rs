//! Chat, friends, important messages and squads (`social.md`,
//! `login_chat.md` §5), which 0.26g carries on the lobby link (`4C..=50`
//! in, `A1..=A4` out).
//!
//! - **Chat.** A line goes to the players it is for: the general channel
//!   to everyone who subscribed to it, whatever their client's language
//!   (as 0.21d's: the client shows a line by its channel byte alone);
//!   private to the named player; match and team to the players the lobby
//!   sent to the same match (and team); squad to the squad; srp's room
//!   channel (10) to the room's members (`room_chat`). Who that is
//!   the server knows itself, so the channel id a client sends is not
//!   trusted. Hidden senders produce no line; placeholder mutes receive
//!   only the sender's name and `<muted>`, never the original body. A
//!   player the admin muted (`admin-chat-mute.md`) reaches nobody on any
//!   channel, and is sent the mute's notice on that channel instead. A
//!   player may send [`CHAT_BURST`] lines at once and one a second after
//!   [choice: 0.21d had no limit; one flooding client must not fill the
//!   others' links].
//! - **ONE GENERAL ROOM FOR ALL LANGUAGES: THE OWNER'S RULE (2026-10-12),
//!   as on 0.21d. Do not "fix" it.** A client names a language room in
//!   `4F` (slot 1: 1 for a Russian or Ukrainian client, 2 for any other)
//!   and a language byte; srp stores the ids and uses neither to choose
//!   who gets a general line: English, Ukrainian and Russian players talk
//!   in one room. The client shows a general line by its channel byte
//!   alone (0xA2 carries no room id), so nothing on its side splits them
//!   either (`social.md` §9.6; the test
//!   `the_general_channel_is_one_room_for_every_language`).
//! - **What the sender is told** (`social.md` §9). The client has one way
//!   to show a text of the server's at any time: `84 49 00 <key>`, a
//!   message box with the localized key (0x21DD6F). `84` of the chat and
//!   friends ops themselves (`4C`..`50`) is only logged by the client and
//!   `84 4A` is dropped (0x21DEA8). The game's localization has one text
//!   for these cases, `st_player_not_found` ("Player not found."), which
//!   nothing in the client refers to: a key the server sent. So a private
//!   line, a friend request, a squad invite or an "ignore by name" for a
//!   name that is no player (or, for a line or an invite, no player in the
//!   lobby) is answered with it. There is no text for "ignores you",
//!   "already a friend" or any squad refusal: those stay silent. A flood
//!   is told by the client itself (its own spam filter, 0x2083F0, is
//!   stricter than the limit here: see [`CHAT_BURST`]).
//! - **Friends.** The lists are the store's (`session::Account::friends`,
//!   `ignored`: kept in `srp.db` as for 0.21d). The client never asks for
//!   them: they are pushed at sign-in and after each change (`A3 07`,
//!   `A3 08`), and the friends' statuses when one changes (`A3 09`, with
//!   exactly the friends of the last `A3 07`). `A3 08` lists every muted
//!   player; a `<muted>` one's id is marked (`lobby::SHOWN_MUTED`), so the
//!   client's own filter passes its `<muted>` lines (chat-mute.md). The old `u8 result`
//!   answers are gone: the client would read them as a search list. A
//!   player may send [`FRIEND_REQUEST_BURST`] friend requests at once and
//!   one every [`FRIEND_REQUEST_EVERY`] after (the client keeps the same
//!   allowance: exe patch 86's Tab list menu, `docs/notes/tab-cursor-friends.md`).
//! - **Squads** are the store's (`session::Squads`), of two at most
//!   (`docs/notes/squads.md`), formed as in 0.21d:
//!   invite (`4D` type 2); the invited client's "membership accepted"
//!   back (`4D` type 3) and then its answer (`50`), in that order
//!   (`important_message_read` 0x28C5E0: the `4D` sender at 0x28C698, the
//!   `50` sender at 0x28C7E1; PROVEN); then the inviter's client adds the
//!   player (`4A 02`). The acceptance is passed on once both have come,
//!   whichever came first.
//!   Every change is answered with the squad info (0xA1) to each member.
//!   A squad mate's character is shown from 0x88, which the client asks
//!   for by the kit id of 0xA1 and files under the id of the **first kit
//!   in the 0x88** (0x222FE0, 0x227C59): so a mate is sent as one kit, the
//!   one it plays, under an id of its own.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use session::v026g::{Account, quests, wire};
use session::{LockExt, Squad, SquadError};
use survarium::v026g::lobby::requests::{ClientMessage, FriendshipAction, id};
use survarium::v026g::lobby::{
    self, Friend, KIT_READY, LobbyPick, ServerMessage, SquadMember, SquadPicks, WireError, channel, important, sender,
};

use super::core::{Core, Who};

/// Chat lines a player may send at once, and how many more a second.
///
/// A line over the limit is dropped and the sender is told nothing, because
/// the game has nothing to tell it with: its "The spam filter has blocked
/// your message" line (`st_spam_filter_block_notification`) is printed by
/// the client's **own** filter (0x2083F0), never on the server's word. That
/// filter runs for the general, match and team channels and lets at most
/// two lines through in any two seconds (the third mutes the player for 10
/// or 5 s), which never empties this allowance: a stock client is told by
/// its own filter before the server would drop a line (the test
/// `a_stock_clients_own_spam_filter_is_stricter_than_the_servers_limit`).
/// On the private, squad and clan channels the client has no filter and no
/// notice; there the limit only stops a script.
pub(super) const CHAT_BURST: f32 = 10.0;
const CHAT_PER_SECOND: f32 = 1.0;
/// Friend requests a player may send at once, and the time that earns one
/// more [choice, docs/notes/tab-cursor-friends.md: a few at once in the
/// lobby's search, no stream of requests]. Exe patch 86 (`tab_cursor.rs`,
/// the Tab list's menu) keeps the same allowance on the client.
pub(super) const FRIEND_REQUEST_BURST: u32 = 5;
pub(super) const FRIEND_REQUEST_EVERY: Duration = Duration::from_secs(10);

/// What a player may still send of friend requests, and when that was
/// counted: [`FRIEND_REQUEST_BURST`] at first, one more each
/// [`FRIEND_REQUEST_EVERY`], never more than the burst.
#[derive(Clone, Copy, Debug)]
pub(super) struct FriendRequestAllowance {
    saved: Duration,
    at: Instant,
}

impl FriendRequestAllowance {
    pub(super) fn new(now: Instant) -> Self {
        Self { saved: FRIEND_REQUEST_EVERY * FRIEND_REQUEST_BURST, at: now }
    }

    /// One request now, if the allowance has one.
    pub(super) fn take(&mut self, now: Instant) -> bool {
        let earned = now.saturating_duration_since(self.at);
        self.saved = (self.saved + earned).min(FRIEND_REQUEST_EVERY * FRIEND_REQUEST_BURST);
        self.at = now;
        if self.saved < FRIEND_REQUEST_EVERY {
            return false;
        }
        self.saved -= FRIEND_REQUEST_EVERY;
        true
    }
}

/// `st_player_not_found`, "Player not found.": in every language of the
/// 0.26g localization and referred to by nothing in the client (exe or
/// lobby SWF), so a text the server sent. [LIKELY for a name the messaging
/// server did not know; which message carried it is not in the client:
/// `84 49` is the one that shows a key at any time with no side effect.]
const PLAYER_NOT_FOUND: &str = "st_player_not_found";
/// The same name is not reported again within this [choice: every private
/// line to a player who left would open a window of its own].
const NOT_FOUND_REPEAT: Duration = Duration::from_secs(10);
/// The most accounts on a friend or ignore list (0.21d's limit).
const MAX_LISTED: usize = 200;
/// The most players a search finds (0.21d's limit).
const MAX_FOUND: usize = 50;
/// The most requests kept waiting for an answer; past it the oldest goes.
const MAX_REQUESTS: usize = 10_000;
/// `messaging_client_type_enum::lobby_server_client_type`: the sender of
/// the server's own notices, as 0.21d's.
const LOBBY_SENDER: u8 = 2;

/// A friend's status as 0xA3 carries it (`st_friend_state_lobby`,
/// `_game`, `_offline`; LIKELY 0.21d's numbers, passed raw to Flash).
pub(super) mod status {
    pub const LOBBY: u8 = 0;
    pub const IN_GAME: u8 = 1;
    pub const OFFLINE: u8 = 2;
}

/// A friend request or a squad invite waiting for the other player's
/// answer (`50 <message id> <answer>`).
pub(super) struct Request {
    from: u32,
    from_name: String,
    to: u32,
    kind: u8,
    text: String,
    /// A squad invite whose "membership accepted" (`4D` type 3) came
    /// before its answer (`50`), as a client sends them: the text, passed
    /// on to the inviter when the answer accepts.
    accepted_text: Option<String>,
}

/// The id a squad mate's kit is sent under (0xA1 and 0x88): none of the
/// viewer's own kit ids, and another for each mate.
fn mate_kit_id(account_id: u32) -> u32 {
    0x4000_0000 | (account_id & 0x3FFF_FFFF)
}

/// The revision of the kit a mate plays: another whenever the client must
/// ask for the mate again (the kit changed, or another kit is played).
fn mate_revision(account: &Account) -> u32 {
    let index = account.kit_index(account.active_kit).unwrap_or(0);
    account
        .kits
        .get(index)
        .map_or(0, |kit| (kit.revision << 2) | (index as u32 & 3))
}

impl Core<'_> {
    /// The account named `name`, ignoring ASCII case, online or not.
    pub(super) fn account_named(&self, name: &str, except: u32) -> Option<u32> {
        self.store
            .find_accounts(name.as_bytes(), except, usize::MAX)
            .into_iter()
            .find(|(_, found)| found.eq_ignore_ascii_case(name))
            .map(|(id, _)| id)
    }

    pub(super) fn ignores(&self, account_id: u32, sender: u32) -> bool {
        self.store
            .account(account_id)
            .is_some_and(|account| account.locked().ignored.contains(&sender))
    }

    /// Apply the recipient's policy before constructing a packet. Placeholder
    /// recipients never receive the original text, on any chat channel.
    pub(super) fn chat_body<'a>(
        &self,
        recipient: u32,
        sender: u32,
        body: &'a str,
    ) -> Option<&'a str> {
        let mode = self
            .store
            .account(recipient)
            .and_then(|account| account.locked().ignored.mode(sender));
        match mode {
            None => Some(body),
            Some(lobby::ChatMuteMode::Hidden) => None,
            Some(lobby::ChatMuteMode::Placeholder) => Some(lobby::MUTED_CHAT_TEXT),
        }
    }

    pub(super) fn set_chat_mute(
        &mut self,
        who: &Who,
        name: &str,
        mode: Option<lobby::ChatMuteMode>,
        now: Instant,
    ) {
        if name == who.name {
            return;
        }
        let Some(target) = self.account_named(name, who.account_id) else {
            // From a match (the Tab list's menu: a bot's place has no
            // account) the lobby's message box would come up only back in
            // the lobby, for a player long gone: none.
            if self.player(who.addr).is_some_and(|player| player.in_match.is_none()) {
                self.player_not_found(who, name, now);
            }
            return;
        };
        if let Some(account) = self.store.account(who.account_id) {
            let mut account = account.locked();
            if mode.is_none()
                || account.ignored.contains(&target)
                || account.ignored.len() < MAX_LISTED
            {
                account.ignored.set(target, mode);
                self.store.mark_dirty(who.account_id);
            }
        }
        // Both menus offer the choices by the mode this list gives them.
        self.push_ignored(who);
    }

    /// Tell the player that `name` is no player it can reach: the game's
    /// "Player not found." in its message box (`84 49 00
    /// st_player_not_found`; the client shows `84 49` whatever it was
    /// doing, 0x21DD6F). Not for the player's own name (the game has no
    /// text for that), and once per name in [`NOT_FOUND_REPEAT`].
    fn player_not_found(&mut self, who: &Who, name: &str, now: Instant) {
        if who.name.eq_ignore_ascii_case(name) {
            return;
        }
        let Some(player) = self.player_mut(who.addr) else {
            return;
        };
        let told = player.not_found.as_ref().is_some_and(|(last, at)| {
            last.eq_ignore_ascii_case(name) && now.saturating_duration_since(*at) < NOT_FOUND_REPEAT
        });
        if told {
            return;
        }
        player.not_found = Some((name.to_owned(), now));
        self.deny(who.addr, id::PROFILE, PLAYER_NOT_FOUND);
    }

    fn next_message_id(&mut self) -> u32 {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.wrapping_add(1).max(1);
        id
    }

    // ---------------------------------------------------------------- chat

    /// `4C`: a typed line. The client has shown it to its own player.
    ///
    /// A private line for a name that is no player in the lobby (unknown,
    /// or offline) is answered with the game's "Player not found."
    /// ([`Core::player_not_found`]); the game has no "offline" text. A
    /// player who ignores the sender is not given away: nothing is said.
    pub(super) fn chat(
        &mut self,
        who: &Who,
        receiver: &str,
        channel: u8,
        body: &str,
        now: Instant,
    ) {
        let Some(player) = self.player_mut(who.addr) else {
            return;
        };
        let earned = now.saturating_duration_since(player.chat_at).as_secs_f32() * CHAT_PER_SECOND;
        player.chat_allowance = (player.chat_allowance + earned).min(CHAT_BURST);
        player.chat_at = now;
        if player.chat_allowance < 1.0 {
            log::info!("{} is flooding the chat; a line was dropped", who.name);
            return;
        }
        player.chat_allowance -= 1.0;
        let (in_match, session_id) = (player.in_match, player.session_id);
        // The admin's chat mute: on every channel, nobody gets the line; the
        // sender gets the mute's notice instead, said by itself.
        if let Some(ban) = self.store.chat_ban(who.account_id) {
            log::info!(
                "{} is muted: a line on channel {channel} was dropped",
                who.name
            );
            let tag = self.clan_tag_of(who.account_id);
            self.send(
                who.addr,
                lobby::chat_text(
                    sender::ACCOUNT,
                    who.account_id,
                    &who.name,
                    &tag,
                    channel,
                    &ban.chat_notice(),
                ),
            );
            return;
        }
        // A room's own channel: its members (`room_chat`).
        if channel == channel::ROOM {
            self.room_say(who, body);
            return;
        }

        let subscribed = |id: u32| id != 0 && id != u32::MAX;
        let squad = self.store.squads().of(who.account_id);
        let clan_mates = self.clan_chat_members(who.account_id, channel);
        let team = in_match.and_then(|id| self.team_in_match(id, session_id));
        let recipients: Vec<(SocketAddr, u32)> = self
            .players()
            .into_iter()
            .filter_map(|addr| {
                let other = self.player(addr)?;
                if other.account_id == who.account_id {
                    return None;
                }
                let same_match = in_match.is_some() && other.in_match == in_match;
                let receives = match channel {
                    channel::PRIVATE => other.name.eq_ignore_ascii_case(receiver),
                    channel::GENERAL => subscribed(other.channel_ids[usize::from(channel)]),
                    channel::MATCH => same_match,
                    channel::TEAM_1 | channel::TEAM_2 => {
                        same_match
                            && team.is_some()
                            && in_match.and_then(|id| self.team_in_match(id, other.session_id))
                                == team
                    }
                    channel::SQUAD => squad
                        .as_ref()
                        .is_some_and(|squad| squad.members.contains(&other.account_id)),
                    // The clan (3) and the clan team (9).
                    channel::CLAN | channel::CLAN_TEAM => clan_mates
                        .as_ref()
                        .is_some_and(|mates| mates.contains(&other.account_id)),
                    _ => false,
                };
                receives.then_some((addr, other.account_id))
            })
            .collect();
        if channel == channel::PRIVATE && recipients.is_empty() {
            log::info!(
                "{}: no player {receiver} in the lobby for a private line",
                who.name
            );
            self.player_not_found(who, receiver, now);
            return;
        }
        let tag = self.clan_tag_of(who.account_id);
        let mut reached = 0;
        for (addr, account_id) in recipients {
            let Some(body) = self.chat_body(account_id, who.account_id, body) else {
                continue;
            };
            self.send(
                addr,
                lobby::chat_text(
                    sender::ACCOUNT,
                    who.account_id,
                    &who.name,
                    &tag,
                    channel,
                    body,
                ),
            );
            reached += 1;
        }
        log::info!(
            "Chat from {} on channel {channel} reached {reached} player(s)",
            who.name
        );
    }

    // ------------------------------------------------- important messages

    /// The server's own notices (quests, reputation levels): the client
    /// acts on the text and answers at once.
    pub(super) fn notify(&mut self, who: &Who, notices: &[quests::Notice]) {
        for (kind, text) in notices {
            let message_id = self.next_message_id();
            self.send(
                who.addr,
                lobby::important_message(LOBBY_SENDER, 0, "", "", *kind, text, message_id),
            );
        }
    }

    fn request_message(&self, message_id: u32, request: &Request) -> ServerMessage {
        lobby::important_message(
            sender::ACCOUNT,
            request.from,
            &request.from_name,
            &self.clan_tag_of(request.from),
            request.kind,
            &request.text,
            message_id,
        )
    }

    /// Send `request.to` a request that waits for its answer: now if it is
    /// in the lobby, and a friend request also when it signs in. One
    /// request of a kind per pair waits at once.
    ///
    /// A squad invite that waits already is sent again under the id it
    /// has: a client that still holds the message skips an id it knows
    /// (0x26BCDA), and one whose game was started again since has lost the
    /// message and could never answer it, so that every later invite of
    /// the pair went nowhere for as long as srp ran.
    fn request(&mut self, request: Request) {
        let waiting = self.requests.iter().find(|(_, waiting)| {
            (waiting.from, waiting.to, waiting.kind) == (request.from, request.to, request.kind)
        });
        if let Some((&message_id, waiting)) = waiting {
            if request.kind == important::SQUAD_INVITE
                && let Some(addr) = self.by_account.get(&request.to).copied()
            {
                let message = self.request_message(message_id, waiting);
                self.send(addr, message);
            }
            return;
        }
        if self.requests.len() >= MAX_REQUESTS {
            self.requests.pop_first();
        }
        let message_id = self.next_message_id();
        if let Some(addr) = self.by_account.get(&request.to).copied() {
            let message = self.request_message(message_id, &request);
            self.send(addr, message);
        }
        self.requests.insert(message_id, request);
    }

    /// The friend requests that waited for a player who signs in. A squad
    /// invite is only for a player in the lobby when it is sent.
    pub(super) fn waiting_requests(&mut self, who: &Who) {
        let waiting: Vec<ServerMessage> = self
            .requests
            .iter()
            .filter(|(_, request)| {
                request.to == who.account_id && request.kind == important::FRIEND_REQUEST
            })
            .map(|(id, request)| self.request_message(*id, request))
            .collect();
        self.send_all(who.addr, waiting);
    }

    /// `4D`: an important message for another player.
    /// - A friend request goes to the named player, who answers it with
    ///   `50` (now, or when it signs in), within the sender's
    ///   [`FriendRequestAllowance`] (one over it is dropped). A name that
    ///   is no account is answered with "Player not found."
    ///   ([`Core::player_not_found`]), except to a player in a match.
    ///   One to a friend, or to a player who ignores the sender, is
    ///   dropped in silence: the game has no text for either, and an
    ///   ignore is not given away.
    /// - A squad invite goes to the named player if it is in the lobby and
    ///   does not ignore the sender; a name that is no player in the lobby
    ///   gets "Player not found.". When the player accepts, its client
    ///   tells the inviter ("membership accepted", type 3), which is passed
    ///   on only for an invite that was accepted; the inviter's client then
    ///   adds the player (`4A 02`).
    pub(super) fn important(
        &mut self,
        who: &Who,
        target_name: &str,
        kind: u8,
        text: &str,
        now: Instant,
    ) {
        let target = self.account_named(target_name, who.account_id);
        let request = |to| Request {
            from: who.account_id,
            from_name: who.name.clone(),
            to,
            kind,
            text: text.to_owned(),
            accepted_text: None,
        };
        match kind {
            important::FRIEND_REQUEST => {
                let Some(player) = self.player_mut(who.addr) else {
                    return;
                };
                if !player.friend_requests.take(now) {
                    log::info!("{}: too many friend requests; one to {target_name} dropped", who.name);
                    return;
                }
                let in_match = player.in_match.is_some();
                let Some(target) = target else {
                    log::info!("{}: no player {target_name} to befriend", who.name);
                    // From a match (the Tab list's menu: a bot's place has no
                    // account) the lobby's message box would come up only
                    // back in the lobby, for a player long gone: none.
                    if !in_match {
                        self.player_not_found(who, target_name, now);
                    }
                    return;
                };
                let already = self
                    .store
                    .account(who.account_id)
                    .is_some_and(|account| account.locked().friends.contains(&target));
                if already || self.ignores(target, who.account_id) {
                    return;
                }
                self.request(request(target));
            }
            important::SQUAD_INVITE | important::SQUAD_INVITE_ACCEPTED => {
                let target = target.filter(|target| self.by_account.contains_key(target));
                let Some(target) = target else {
                    log::info!(
                        "{}: squad message {kind} not sent: {target_name} is not in the lobby",
                        who.name
                    );
                    // The invite is the player's own act; the acceptance
                    // (type 3) is its client's.
                    if kind == important::SQUAD_INVITE {
                        self.player_not_found(who, target_name, now);
                    }
                    return;
                };
                if self.ignores(target, who.account_id) {
                    log::info!(
                        "{}: squad message {kind} not sent: {target_name} ignores it",
                        who.name
                    );
                    return;
                }
                // A squad of two takes nobody more (the owner's of
                // 2026-10-31): the invite goes nowhere. The movie offers
                // none then; the game has no text for it (§9.2).
                if kind == important::SQUAD_INVITE && self.store.squads().is_full(who.account_id) {
                    log::info!(
                        "{}: squad invite to {target_name} not sent: its squad is full",
                        who.name
                    );
                } else if kind == important::SQUAD_INVITE {
                    self.request(request(target));
                } else if self.store.squads().has_accepted(target, who.account_id) {
                    // The inviter's client answers it by itself and adds
                    // the player.
                    let message_id = self.next_message_id();
                    let message = self.request_message(message_id, &request(target));
                    if let Some(addr) = self.by_account.get(&target).copied() {
                        self.send(addr, message);
                    }
                } else if let Some(invite) = self.requests.values_mut().find(|waiting| {
                    (waiting.from, waiting.to, waiting.kind)
                        == (target, who.account_id, important::SQUAD_INVITE)
                }) {
                    // The client's order: its answer to the invite (`50`)
                    // is what it sends next (`important_read`).
                    invite.accepted_text = Some(text.to_owned());
                } else {
                    log::info!(
                        "{} said it joined {target_name}'s squad without an invite; not passed on",
                        who.name
                    );
                }
            }
            // Clan and clan-team invites follow an `83 49 0F` / `83 49 19`
            // srp never sends.
            _ => log::info!("{}: an important message of type {kind}; dropped", who.name),
        }
    }

    /// `50 <message id> <answer>`: 0 accepts. The client answers the
    /// server's own notices at once; only requests are kept here.
    pub(super) fn important_read(&mut self, who: &Who, message_id: u32, answer: u8) {
        if self
            .requests
            .get(&message_id)
            .is_none_or(|request| request.to != who.account_id)
        {
            return;
        }
        let Some(request) = self.requests.remove(&message_id) else {
            return;
        };
        if answer != 0 {
            log::info!("{} declined a request of type {}", who.name, request.kind);
            return;
        }
        if request.kind == important::SQUAD_INVITE {
            log::info!("{} accepted {}'s squad invite", who.name, request.from_name);
            self.store
                .squads()
                .invite_accepted(request.from, who.account_id);
            // Its "membership accepted" came first, as a client sends it:
            // the inviter gets it now, and its client adds the player.
            if let Some(text) = request.accepted_text {
                let accepted = Request {
                    from: who.account_id,
                    from_name: who.name.clone(),
                    to: request.from,
                    kind: important::SQUAD_INVITE_ACCEPTED,
                    text,
                    accepted_text: None,
                };
                let message_id = self.next_message_id();
                let message = self.request_message(message_id, &accepted);
                if let Some(addr) = self.by_account.get(&request.from).copied() {
                    self.send(addr, message);
                }
            }
            return;
        }
        let made = self.befriend(request.from, who.account_id);
        log::info!(
            "{} accepted {}'s friend request (made: {made})",
            who.name,
            request.from_name
        );
        for account_id in [request.from, who.account_id] {
            if let Some(friend) = self.who_is(account_id) {
                self.push_friends(&friend);
            }
        }
    }

    // ------------------------------------------------------------- friends

    /// Make two accounts each other's friends, if both lists have room.
    fn befriend(&self, a: u32, b: u32) -> bool {
        let (Some(first), Some(second)) = (self.store.account(a), self.store.account(b)) else {
            return false;
        };
        if a == b {
            return false;
        }
        let room = |friends: &[u32], other| friends.contains(&other) || friends.len() < MAX_LISTED;
        // One at a time: no two accounts are locked together.
        let first_fits = room(&first.locked().friends, b);
        let second_fits = room(&second.locked().friends, a);
        if !first_fits || !second_fits {
            return false;
        }
        for (account, friend) in [(&first, b), (&second, a)] {
            let mut account = account.locked();
            if !account.friends.contains(&friend) {
                account.friends.push(friend);
            }
        }
        self.store.mark_dirty(a);
        self.store.mark_dirty(b);
        true
    }

    /// What a friend's client shows of an account.
    fn status_of(&self, account_id: u32) -> u8 {
        match self
            .by_account
            .get(&account_id)
            .and_then(|addr| self.player(*addr))
        {
            Some(player) if player.in_match.is_some() => status::IN_GAME,
            Some(_) => status::LOBBY,
            None => status::OFFLINE,
        }
    }

    /// `A3 07`: the whole friend list.
    pub(super) fn push_friends(&mut self, who: &Who) {
        let ids = self
            .store
            .account(who.account_id)
            .map(|account| account.locked().friends.clone())
            .unwrap_or_default();
        let friends: Vec<Friend> = ids
            .iter()
            .filter_map(|&account_id| {
                Some(Friend {
                    account_id,
                    name: self.name_of(account_id)?,
                    clan_tag: self.clan_tag_of(account_id),
                    status: self.status_of(account_id),
                })
            })
            .collect();
        let sent = friends
            .iter()
            .map(|friend| (friend.account_id, friend.status))
            .collect();
        self.send_made(who.addr, lobby::friend_list(&friends));
        if let Some(player) = self.player_mut(who.addr) {
            player.friends_sent = sent;
        }
    }

    /// `A3 08`: every muted player, a `<muted>` one's id marked
    /// (`lobby::SHOWN_MUTED`) so that its lines pass the client's filter.
    pub(super) fn push_ignored(&mut self, who: &Who) {
        let ids = self
            .store
            .account(who.account_id)
            .map(|account| account.locked().ignored.clone())
            .unwrap_or_default();
        let ignored: Vec<(u32, String, lobby::ChatMuteMode)> = ids
            .entries()
            .filter_map(|(account_id, mode)| Some((account_id, self.name_of(account_id)?, mode)))
            .collect();
        self.send_made(who.addr, lobby::ignore_list(&ignored));
    }

    /// `A3 09` when a friend's status changed: exactly the friends the
    /// client holds, or it drops the message.
    pub(super) fn friend_statuses(&mut self, addr: SocketAddr) {
        let Some(player) = self.player(addr) else {
            return;
        };
        if player.friends_sent.is_empty() {
            return;
        }
        let now: Vec<(u32, u8)> = player
            .friends_sent
            .iter()
            .map(|&(account_id, _)| (account_id, self.status_of(account_id)))
            .collect();
        if now == player.friends_sent {
            return;
        }
        self.send_made(addr, lobby::friend_statuses(&now));
        if let Some(player) = self.player_mut(addr) {
            player.friends_sent = now;
        }
    }

    /// `4E`: a friendship action. Friendship is both ways: removing a
    /// friend removes it from both lists. Ignoring is one-sided. Each is
    /// answered with the list as it is now; a typed name that is no
    /// account also with "Player not found.".
    pub(super) fn friendship(&mut self, who: &Who, action: &FriendshipAction, now: Instant) {
        let me = who.account_id;
        let Some(account) = self.store.account(me) else {
            return;
        };
        let ignore = |target: Option<u32>| {
            let Some(target) = target.filter(|target| *target != me) else {
                return;
            };
            if self.store.account(target).is_none() {
                return;
            }
            let mut account = account.locked();
            if account.ignored.contains(&target) || account.ignored.len() < MAX_LISTED {
                account
                    .ignored
                    .set(target, Some(lobby::ChatMuteMode::Hidden));
                self.store.mark_dirty(me);
            }
        };
        match action {
            FriendshipAction::RemoveFriend { account_id: target } => {
                account.locked().friends.retain(|id| id != target);
                if let Some(other) = self.store.account(*target).filter(|_| *target != me) {
                    other.locked().friends.retain(|id| *id != me);
                }
                self.store.mark_dirty(me);
                self.store.mark_dirty(*target);
                self.push_friends(who);
                if let Some(other) = self.who_is(*target) {
                    self.push_friends(&other);
                }
            }
            FriendshipAction::Ignore { account_id: target } => {
                ignore(Some(*target));
                self.push_ignored(who);
            }
            FriendshipAction::IgnoreByName { name } => {
                let target = self.account_named(name, me);
                ignore(target);
                self.push_ignored(who);
                if target.is_none() {
                    self.player_not_found(who, name, now);
                }
            }
            FriendshipAction::StopIgnoring { account_id: target } => {
                account.locked().ignored.set(*target, None);
                self.store.mark_dirty(me);
                self.push_ignored(who);
            }
            FriendshipAction::FindPlayers { name } => {
                let found: Vec<(u32, String, String)> = self
                    .store
                    .find_accounts(name.as_bytes(), me, MAX_FOUND)
                    .into_iter()
                    .map(|(account_id, login_name)| {
                        let name = self.name_of(account_id).unwrap_or(login_name);
                        (account_id, name, self.clan_tag_of(account_id))
                    })
                    .collect();
                self.send_made(who.addr, lobby::found_players(&found));
            }
        }
    }

    // -------------------------------------------------------------- squads

    /// `4A 01..=04`. The client has no answer for them: the squad info
    /// (0xA1) that follows a change is it, and a refused change is only
    /// logged.
    ///
    /// A refusal can not be shown [PROVEN, `social.md` §9.4]: `84 4A`
    /// returns at once in the client (0x21DEA8, as 0.21d's `35 2A`), `83`
    /// has no case for `4A`, and the game's localization has no text for
    /// any squad refusal (not the commander, full, not invited, in a
    /// squad): its squad keys are labels and the two invite requests. So a
    /// refusal means the client's picture of the squad was old: it is sent
    /// the squad as it is.
    pub(super) fn squad_action(&mut self, who: &Who, message: &ClientMessage) {
        let squads = self.store.squads();
        let me = who.account_id;
        let result = match message {
            ClientMessage::DisbandSquad => squads.disband(me),
            ClientMessage::AddSquadMember { name } => match self.account_named(name, me) {
                Some(member) => squads.add(me, member).map(drop),
                None => Err(SquadError::NotInvited),
            },
            ClientMessage::DropSquadMember { account_id } => squads.drop_member(me, *account_id),
            ClientMessage::LeaveSquad => squads.leave(me),
            _ => return,
        };
        match result {
            Ok(()) => log::info!("{}: squad change {message:?}", who.name),
            Err(error) => {
                log::info!("{}: squad change {message:?} refused: {error:?}", who.name);
                // Its client asked for a change to a squad the server does
                // not have as it shows it: it gets the squad as it is.
                if let Some(player) = self.player_mut(who.addr) {
                    player.squad_sent = None;
                }
            }
        }
        self.squads_dirty = true;
    }

    /// `34 00 <account> <kit>`: a squad mate's character (0x88).
    pub(super) fn squad_member(&mut self, who: &Who, mate: u32) {
        let mates = self
            .store
            .squads()
            .of(who.account_id)
            .is_some_and(|squad| mate != who.account_id && squad.members.contains(&mate));
        let Some(account) = self.accounts.get(mate).filter(|_| mates) else {
            return;
        };
        let message = {
            let account = account.locked();
            let mut body = wire::account_body(&account, quests::now());
            let index = account.kit_index(account.active_kit).unwrap_or(0);
            // The kit it plays, alone and under the id of 0xA1: the client
            // files the mate under its first kit's id.
            let kit = (index < body.kits.len()).then(|| body.kits.swap_remove(index));
            body.kits = kit
                .map(|mut kit| {
                    kit.id = mate_kit_id(mate);
                    kit.state = KIT_READY;
                    kit
                })
                .into_iter()
                .collect();
            lobby::squad_member_account(&body, mate_revision(&account))
        };
        self.send_made(who.addr, message);
    }

    /// The lobby pick of a signed-in account (none offline or not picked).
    fn lobby_pick_of(&self, account_id: u32) -> Option<LobbyPick> {
        self.player(*self.by_account.get(&account_id)?)?.lobby_pick
    }

    /// `3B` at a lobby pick's start: kept; every squad is sent again where it
    /// changed (the picker's own `0xA1`, its squad's when it commands one).
    pub(super) fn lobby_pick(&mut self, who: &Who, pick: LobbyPick) {
        if let Some(player) = self.player_mut(who.addr)
            && player.lobby_pick != Some(pick)
        {
            player.lobby_pick = Some(pick);
            self.squads_dirty = true;
        }
    }

    /// The squad info for one member's client.
    fn squad_message(&self, me: u32, squad: Option<&Squad>) -> Result<ServerMessage, WireError> {
        let own = self.lobby_pick_of(me);
        let Some(squad) = squad else {
            return lobby::squad_info(0, &[], 0, false, 0, SquadPicks { own, commander: None });
        };
        let members: Vec<SquadMember> = squad
            .members
            .iter()
            .map(|&member| {
                let shown = self.accounts.get(member).map(|account| {
                    let account = account.locked();
                    let kit = if member == me {
                        // Its own entry carries its own kit's id.
                        account
                            .active()
                            .map_or((0, 0), |kit| (kit.id, kit.revision))
                    } else {
                        (mate_kit_id(member), mate_revision(&account))
                    };
                    (account.name.clone(), kit)
                });
                let (name, (kit_id, kit_revision)) = shown.unwrap_or_else(|| {
                    (
                        self.name_of(member).unwrap_or_default(),
                        (mate_kit_id(member), 0),
                    )
                });
                SquadMember {
                    account_id: member,
                    kit_id,
                    kit_revision,
                    name,
                    online: self.by_account.contains_key(&member),
                    ready: self.squad_ready(member),
                }
            })
            .collect();
        // The commander is the first member; its pick is every member's lobby.
        let commander = self.lobby_pick_of(squad.commander());
        lobby::squad_info(
            squad.id,
            &members,
            0,
            squad.commander() == me,
            squad.revision,
            SquadPicks { own, commander },
        )
    }

    /// Send each client its squad when it differs from what it has.
    ///
    /// 0xA1 is not ordered against other 0xA1s (transport.md §4.1): the
    /// next one for a client goes only when the last is acked.
    pub(super) fn refresh_squads(&mut self) {
        self.squads_dirty = false;
        for addr in self.players() {
            let Some(link) = self.links.get(&addr) else {
                continue;
            };
            let Some(player) = self.player(addr) else {
                continue;
            };
            let waits = player
                .squad_serial
                .is_some_and(|serial| !link.transport.is_delivered(serial));
            if waits {
                self.squads_dirty = true;
                continue;
            }
            let me = player.account_id;
            let squad = self.store.squads().of(me);
            let message = match self.squad_message(me, squad.as_ref()) {
                Ok(message) => message,
                Err(error) => {
                    log::error!("A squad info could not be made: {error}");
                    continue;
                }
            };
            let Some(player) = self.player(addr) else {
                continue;
            };
            if player.squad_sent.as_deref() != Some(message.body()) {
                let body = message.body().to_vec();
                let serial = self.send(addr, message);
                if let Some(player) = self.player_mut(addr) {
                    player.squad_sent = Some(body);
                    player.squad_serial = serial;
                }
            }
        }
    }
}
