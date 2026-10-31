//! The chat ("messaging") server. The stock client opens a second
//! connection to the lobby port for it; the first byte tells the two
//! protocols apart (see `handle_connection`). Layouts are in
//! `../srp-021d/docs/notes/lobby_spec.md` §1.5 and §5.
//!
//! It relays typed lines between the connected players: the general channel
//! to everyone, whatever their client's language, private (`/name text`) to
//! that player,
//! and the match, team and squad channels to the players subscribed to the
//! same one. It keeps each account's friend and ignore lists (in memory,
//! with the account), finds players by name, tells which friends are in the
//! chat, and carries friend requests, which wait for the other player's
//! answer even while they are away.

mod message;

use message::{
    ClientMessage, FriendStatus, FriendshipRequest, ListedAccount, NAME_CAPACITY, ServerMessage,
};

use crate::UnknownSessions;
use session::{LockExt, SessionStore};
use vostok::network_client::TcpClient;
use vostok::network_packet::TcpPacket;
use vostok::serde::Serialize;

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
pub enum messaging_server_message_types_enum {
    messaging_server_connection_successful = 0xC8,
    messaging_server_message               = 0xC9,
    messaging_server_sign_in_info          = 0xCA,
    messaging_friendship_status            = 0xCB,
    messaging_important_message            = 0xCC,
}

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
pub enum messaging_client_message_types_enum {
    messaging_client_message         = 0xC1,
    messaging_important_message      = 0xC2,
    messaging_client_sign_in_info    = 0xC3,
    messaging_friendship_action      = 0xC4,
    messaging_client_subscription    = 0xC5,
    messaging_important_message_read = 0xC6,
}

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
pub enum messaging_client_type_enum {
    unknown_client_type                = 0x0,
    login_server_client_type           = 0x1,
    lobby_server_client_type           = 0x2,
    match_server_client_type           = 0x3,
    message_server_client_type         = 0x4,
    account_client_type                = 0x5,
    administrative_client_type         = 0x6,
    match_maker_server_client_type     = 0x7,
    stats_processor_server_client_type = 0x8,
}

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
pub enum messaging_message_channel_enum {
    server_message_channel = 0x0,
    player_general_channel = 0x1,
    player_system_channel  = 0x2,
    player_clan_channel    = 0x3,
    player_private_channel = 0x4,
    player_match_channel   = 0x5,
    player_team1_channel   = 0x6,
    player_team2_channel   = 0x7,
    player_squad_channel   = 0x8,
    max_channel_num        = 0x9,
}

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum friendship_actions_enum {
    add_friend            = 0x0,
    add_friend_by_name    = 0x1,
    remove_friend         = 0x2,
    add_ignorable         = 0x3,
    add_ignorable_by_name = 0x4,
    remove_ignorable      = 0x5,
    find_players          = 0x6,
    query_friend_list     = 0x7,
    query_ignore_list     = 0x8,
    update_friends_status = 0x9,
}

#[repr(u8)]
#[rustfmt::skip]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
pub enum important_message_types {
    friendship                = 0x0,
    quest                     = 0x1,
    squad_invite              = 0x2,
    squad_invite_accepted     = 0x3,
    reputation_level_unlocked = 0x4,
    drop                      = 0x5,
}

/// The answer to an important message that accepts it.
const ACCEPT: u8 = 0;

/// Who a chat line goes to. The client sends the id of the channel it
/// writes to: its language for the general channel, its match for the match
/// and team channels (the team picking the channel), its squad for the
/// squad channel; it sends nothing for a channel it has no id for.
///
/// The general channel is one for everyone who subscribed to it, whatever
/// the id. The id is the client's language: 2 when it runs with
/// `-localization=english`, 1 with any other (`messaging_client` sign-in
/// 0x1B3407, from 1 in its constructor), so matched by it the english
/// clients and the russian and ukrainian ones each had a chat of their own.
/// The client shows a line by its channel byte alone, and its names and
/// lines are Scaleform's UTF-8 in every localization, so any client shows
/// any line.
fn receives(channel: u8, channel_id: u32, receiver: &[u8], member: &Member) -> bool {
    use messaging_message_channel_enum as channels;

    let set = |id: u32| id != 0 && id != u32::MAX;
    match channel {
        _ if channel == channels::player_private_channel as u8 => {
            member.name.eq_ignore_ascii_case(receiver)
        }
        _ if channel == channels::player_general_channel as u8 => {
            set(channel_id) && set(member.channel_ids[usize::from(channel)])
        }
        _ if (channels::player_match_channel as u8..=channels::player_squad_channel as u8)
            .contains(&channel) =>
        {
            set(channel_id) && member.channel_ids[usize::from(channel)] == channel_id
        }
        // Clan lines: the client does not send them, and srp has no clans.
        _ => false,
    }
}

/// The client's localization behind its general channel id, for the log.
fn language_name(language: u32) -> &'static str {
    match language {
        1 => "not english: russian, ukrainian or another",
        2 => "english",
        _ => "unknown",
    }
}

/// Everyone signed in to the chat, shared by the connection threads.
#[derive(Default)]
pub struct ChatRoom {
    next_member_id: AtomicU64,
    members: Mutex<HashMap<u64, Member>>,
    next_message_id: AtomicU32,
    /// Friend requests and squad invites waiting for an answer, by message id.
    requests: Mutex<HashMap<u32, Request>>,
}

struct Member {
    account_id: u32,
    name: Vec<u8>,
    /// The channel ids the client subscribed to, by channel (all 0 until
    /// it does).
    channel_ids: [u32; 9],
    writer: Arc<Mutex<TcpStream>>,
}

/// An important message from one player to another.
struct Request {
    from: u32,
    from_name: Vec<u8>,
    to: u32,
    kind: important_message_types,
    text: Vec<u8>,
    /// A squad invite whose "membership accepted" came before its answer,
    /// as a client sends them: the text, passed on to the inviter when the
    /// answer accepts.
    accepted_text: Option<Vec<u8>>,
}

impl Request {
    fn message(&self, message_id: u32) -> ServerMessage {
        ServerMessage::Important {
            sender_type: messaging_client_type_enum::account_client_type as u8,
            sender_account_id: self.from,
            sender_name: self.from_name.clone(),
            message_type: self.kind as u8,
            text: self.text.clone(),
            message_id,
        }
    }
}

/// Removes a member from the room however its connection ends.
struct Membership<'a> {
    room: &'a ChatRoom,
    id: u64,
}

impl Drop for Membership<'_> {
    fn drop(&mut self) {
        self.room.members.locked().remove(&self.id);
    }
}

impl ChatRoom {
    fn join(&self, member: Member) -> Membership<'_> {
        let id = self.next_member_id.fetch_add(1, Ordering::Relaxed);
        self.members.locked().insert(id, member);
        Membership { room: self, id }
    }

    fn subscribe(&self, id: u64, channel_ids: [u32; 9]) {
        if let Some(member) = self.members.locked().get_mut(&id) {
            member.channel_ids = channel_ids;
        }
    }

    /// An important message from the server (a quest result, a reputation
    /// level): the client acts on its text and answers it at once.
    pub fn notify(&self, account_id: u32, kind: u8, text: &str) {
        let message_id = self.next_message_id.fetch_add(1, Ordering::Relaxed) + 1;
        let message = ServerMessage::Important {
            sender_type: messaging_client_type_enum::lobby_server_client_type as u8,
            sender_account_id: 0,
            sender_name: Vec::new(),
            message_type: kind,
            text: text.as_bytes().to_vec(),
            message_id,
        };
        self.tell_account(account_id, &message);
    }

    /// Whether the account has a client in the chat.
    pub fn is_online(&self, account_id: u32) -> bool {
        self.members
            .locked()
            .values()
            .any(|member| member.account_id == account_id)
    }

    /// Send a message to every client of an account in the chat.
    fn tell_account(&self, account_id: u32, message: &ServerMessage) {
        let writers: Vec<_> = self
            .members
            .locked()
            .values()
            .filter(|member| member.account_id == account_id)
            .map(|member| member.writer.clone())
            .collect();
        for writer in &writers {
            if let Err(error) = send(writer, message.clone()) {
                log::debug!("Chat message not delivered: {error}");
            }
        }
    }

    /// Send `request.to` a request that waits for its answer: now if it is in
    /// the chat, and a friend request also when it joins. One request of a
    /// kind per pair waits at once.
    fn request(&self, request: Request) {
        let mut requests = self.requests.locked();
        if requests.values().any(|waiting| {
            waiting.from == request.from && waiting.to == request.to && waiting.kind == request.kind
        }) {
            return;
        }
        let message_id = self.next_message_id.fetch_add(1, Ordering::Relaxed) + 1;
        let message = request.message(message_id);
        let to = request.to;
        requests.insert(message_id, request);
        drop(requests);
        self.tell_account(to, &message);
    }

    /// The friend requests waiting for `account_id`'s answer. A squad invite
    /// is only for a player in the chat when it is sent.
    fn requests_for(&self, account_id: u32) -> Vec<ServerMessage> {
        let mut waiting: Vec<_> = self
            .requests
            .locked()
            .iter()
            .filter(|(_, request)| {
                request.to == account_id && request.kind == important_message_types::friendship
            })
            .map(|(id, request)| request.message(*id))
            .collect();
        waiting.sort_by_key(|message| match message {
            ServerMessage::Important { message_id, .. } => *message_id,
            _ => 0,
        });
        waiting
    }

    /// The invited player's "membership accepted" came while its invite
    /// still waits for the answer: kept with the invite. `false` if no
    /// invite of `inviter` waits for `invited`.
    fn accepted_before_answer(&self, inviter: u32, invited: u32, text: &[u8]) -> bool {
        let mut requests = self.requests.locked();
        let invite = requests.values_mut().find(|waiting| {
            waiting.from == inviter
                && waiting.to == invited
                && waiting.kind == important_message_types::squad_invite
        });
        match invite {
            Some(invite) => {
                invite.accepted_text = Some(text.to_vec());
                true
            }
            None => false,
        }
    }

    /// Takes the request `message_id` if it was sent to `account_id`.
    fn take_request(&self, message_id: u32, account_id: u32) -> Option<Request> {
        let mut requests = self.requests.locked();
        if requests.get(&message_id)?.to != account_id {
            return None;
        }
        requests.remove(&message_id)
    }

    /// Send a typed line to everyone it is for; returns how many got it.
    fn relay(
        &self,
        from: u64,
        channel_id: u32,
        receiver: &[u8],
        channel: u8,
        body: &[u8],
    ) -> usize {
        let (message, writers) = {
            let members = self.members.locked();
            let Some(sender) = members.get(&from) else {
                return 0;
            };
            let message = ServerMessage::Text {
                sender_type: messaging_client_type_enum::account_client_type as u8,
                sender_account_id: sender.account_id,
                sender_name: sender.name.clone(),
                channel,
                body: body.to_vec(),
            };
            let writers: Vec<_> = members
                .iter()
                .filter(|(id, member)| {
                    **id != from && receives(channel, channel_id, receiver, member)
                })
                .map(|(_, member)| member.writer.clone())
                .collect();
            (message, writers)
        };
        for writer in &writers {
            // A recipient that is gone is noticed and removed by its own thread.
            if let Err(error) = send(writer, message.clone()) {
                log::debug!("Chat line not delivered: {error}");
            }
        }
        writers.len()
    }
}

fn send(writer: &Mutex<TcpStream>, message: ServerMessage) -> std::io::Result<()> {
    let mut packet = TcpPacket::new();
    message.serialize(&mut packet);
    writer.locked().write_all(packet.get_buffer())
}

/// The most accounts on a friend or ignore list. The client keeps any
/// number; this keeps a list within one frame.
const MAX_LISTED: usize = 200;
/// The most players a search finds.
const MAX_FOUND: usize = 50;

/// The account named `name`, ignoring ASCII case.
fn account_named(store: &SessionStore, name: &[u8], except: u32) -> Option<u32> {
    store
        .find_accounts(name, except, usize::MAX)
        .into_iter()
        .find(|(_, found)| found.as_bytes().eq_ignore_ascii_case(name))
        .map(|(id, _)| id)
}

/// Make two accounts each other's friends, if both lists have room.
fn befriend(store: &SessionStore, a: u32, b: u32) -> bool {
    let (Some(first), Some(second)) = (store.account(a), store.account(b)) else {
        return false;
    };
    let room = |friends: &[u32], other| friends.contains(&other) || friends.len() < MAX_LISTED;
    let (mut first, mut second) = (first.locked(), second.locked());
    if a == b || !room(&first.friends, b) || !room(&second.friends, a) {
        return false;
    }
    if !first.friends.contains(&b) {
        first.friends.push(b);
    }
    if !second.friends.contains(&a) {
        second.friends.push(a);
    }
    drop((first, second));
    store.mark_dirty(a);
    store.mark_dirty(b);
    true
}

/// The answer to a friendship request from `account_id`'s client
/// (`messaging_client::on_packet_received` 0x19AD5A). After an accepted
/// change (`'4'`) the client asks for the list again; anything else it only
/// logs. Friendship is both ways: removing a friend removes it from both
/// lists. Ignoring is one-sided.
fn friendship_reply(
    store: &SessionStore,
    room: &ChatRoom,
    account_id: u32,
    request: &FriendshipRequest,
) -> ServerMessage {
    let action = request.action();
    let Some(account) = store.account(account_id) else {
        // A standalone lobby's default account has no lists.
        return match request {
            FriendshipRequest::QueryFriendList => ServerMessage::FriendList(Vec::new()),
            FriendshipRequest::QueryIgnoreList => ServerMessage::IgnoreList(Vec::new()),
            FriendshipRequest::FindPlayers { .. } => ServerMessage::FoundPlayers(Vec::new()),
            FriendshipRequest::UpdateFriendsStatus => ServerMessage::FriendStatuses(Vec::new()),
            _ => ServerMessage::FriendshipResult {
                action,
                accepted: false,
            },
        };
    };
    let listed = |ids: &[u32]| -> Vec<ListedAccount> {
        ids.iter()
            .filter_map(|id| {
                let name = store.account(*id)?.locked().name.clone().into_bytes();
                Some(ListedAccount {
                    account_id: *id,
                    name,
                    status: if room.is_online(*id) {
                        FriendStatus::InLobby
                    } else {
                        FriendStatus::Offline
                    },
                })
            })
            .collect()
    };
    let ignore = |target: Option<u32>| {
        let mut account = account.locked();
        let accepted = target.is_some_and(|target| {
            target != account_id
                && store.account(target).is_some()
                && (account.ignored.contains(&target) || account.ignored.len() < MAX_LISTED)
        });
        if let Some(target) = target.filter(|target| accepted && !account.ignored.contains(target))
        {
            account
                .ignored
                .set(target, Some(session::chat_mutes::ChatMuteMode::Hidden));
            store.mark_dirty(account_id);
        }
        ServerMessage::FriendshipResult { action, accepted }
    };
    match request {
        FriendshipRequest::RemoveFriend { account_id: target } => {
            account.locked().friends.retain(|id| id != target);
            if let Some(other) = store.account(*target).filter(|_| *target != account_id) {
                other.locked().friends.retain(|id| *id != account_id);
            }
            store.mark_dirty(account_id);
            store.mark_dirty(*target);
            ServerMessage::FriendshipResult {
                action,
                accepted: true,
            }
        }
        FriendshipRequest::AddIgnored { account_id: target } => ignore(Some(*target)),
        FriendshipRequest::AddIgnoredByName { name } => {
            ignore(account_named(store, name, account_id))
        }
        FriendshipRequest::RemoveIgnored { account_id: target } => {
            account.locked().ignored.set(*target, None);
            store.mark_dirty(account_id);
            ServerMessage::FriendshipResult {
                action,
                accepted: true,
            }
        }
        FriendshipRequest::FindPlayers { name } => ServerMessage::FoundPlayers(
            store
                .find_accounts(name, account_id, MAX_FOUND)
                .into_iter()
                .map(|(found, name)| ListedAccount {
                    account_id: found,
                    name: name.into_bytes(),
                    status: FriendStatus::Offline,
                })
                .collect(),
        ),
        FriendshipRequest::QueryFriendList => {
            let friends = account.locked().friends.clone();
            ServerMessage::FriendList(listed(&friends))
        }
        FriendshipRequest::QueryIgnoreList => {
            let ignored: Vec<u32> = account.locked().ignored.iter().copied().collect();
            ServerMessage::IgnoreList(listed(&ignored))
        }
        FriendshipRequest::UpdateFriendsStatus => {
            let friends = account.locked().friends.clone();
            ServerMessage::FriendStatuses(listed(&friends))
        }
    }
}

/// Handle an important message from `account_id`'s client:
/// - A friend request (`add_to_friend_list`) goes to the named player, who
///   answers it with 0xC6. A request that can't be sent is answered as a
///   refused `add_friend_by_name`, which the client only logs.
/// - A squad invite goes to the named player if it is in the chat. When it
///   accepts (0xC6), its client tells the inviter with a "membership
///   accepted" message, and the inviter's client asks the lobby to add it
///   (`2A 02`). The client has no answer for a refused invite.
fn important_message(
    store: &SessionStore,
    room: &ChatRoom,
    (account_id, name): (u32, &[u8]),
    target_name: &[u8],
    message_type: u8,
    text: Vec<u8>,
) -> Option<ServerMessage> {
    use important_message_types as types;

    let kind = match message_type {
        _ if message_type == types::friendship as u8 => types::friendship,
        _ if message_type == types::squad_invite as u8 => types::squad_invite,
        _ if message_type == types::squad_invite_accepted as u8 => types::squad_invite_accepted,
        _ => {
            log::info!("Important message of type {message_type} from a client; dropped");
            return None;
        }
    };
    if kind != types::friendship {
        squad_message(store, room, (account_id, name), target_name, kind, text);
        return None;
    }
    let refused = ServerMessage::FriendshipResult {
        action: friendship_actions_enum::add_friend_by_name,
        accepted: false,
    };
    let Some(account) = store.account(account_id) else {
        return Some(refused);
    };
    let Some(target) = account_named(store, target_name, account_id) else {
        return Some(refused);
    };
    let already = account.locked().friends.contains(&target);
    let ignored_by_target = store
        .account(target)
        .is_some_and(|target| target.locked().ignored.contains(&account_id));
    if already || ignored_by_target {
        return Some(refused);
    }
    room.request(Request {
        from: account_id,
        from_name: name.to_vec(),
        to: target,
        kind,
        text,
        accepted_text: None,
    });
    None
}

/// A squad invite, or the invited player's "membership accepted" to the
/// inviter. Either goes only to a player in the chat who doesn't ignore the
/// sender. Being accepted is only passed on for an invite that was accepted.
///
/// The client that accepts sends "membership accepted" first and its
/// answer to the invite after it (`lobby_menu::scaleform_callback`:
/// `send_important_message` at 0x171C3A, `read_important_message` at
/// 0x171C55; PROVEN), so the first usually finds the invite still waiting:
/// it is kept with the invite and passed on when the answer accepts.
/// (Until 2026-10-12 it was dropped then, and every first accept was lost.)
fn squad_message(
    store: &SessionStore,
    room: &ChatRoom,
    (account_id, name): (u32, &[u8]),
    target_name: &[u8],
    kind: important_message_types,
    text: Vec<u8>,
) {
    let target = account_named(store, target_name, account_id).filter(|target| {
        room.is_online(*target)
            && !store
                .account(*target)
                .is_some_and(|target| target.locked().ignored.contains(&account_id))
    });
    let Some(target) = target else {
        log::info!(
            "{kind:?} from {} not sent: {} is not in the chat",
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(target_name)
        );
        return;
    };
    let request = Request {
        from: account_id,
        from_name: name.to_vec(),
        to: target,
        kind,
        text,
        accepted_text: None,
    };
    if kind == important_message_types::squad_invite {
        room.request(request);
    } else if store.squads().has_accepted(target, account_id) {
        // The inviter's client answers it by itself and adds the player.
        let message_id = room.next_message_id.fetch_add(1, Ordering::Relaxed) + 1;
        room.tell_account(target, &request.message(message_id));
    } else if room.accepted_before_answer(target, account_id, &request.text) {
        // Its answer to the invite is what it sends next.
    } else {
        log::info!(
            "{} said it joined {}'s squad without an invite; not passed on",
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(target_name)
        );
    }
}

/// Serve one chat connection until the client leaves. `reader` has already
/// buffered the first message; `writer` is the same socket.
pub fn run(
    store: &SessionStore,
    room: &ChatRoom,
    mut reader: TcpClient,
    writer: TcpStream,
    unknown: UnknownSessions,
) {
    let session_id = match reader.read::<ClientMessage>() {
        Ok(ClientMessage::SignInInfo { session_id, .. }) => session_id,
        Ok(message) => {
            log::warn!("Chat connection opened with {message:?} instead of a sign-in; closed it");
            return;
        }
        Err(error) if error.is_disconnect() => {
            log::info!("Chat connection closed before its sign-in");
            return;
        }
        Err(error) => {
            log::warn!("Unreadable chat sign-in ({error}); closed the connection");
            return;
        }
    };

    // The name the lobby shows for the account. The connection holds its
    // session while it lasts. The chat client knows no refusal: it logs any
    // answer but its sign-in (0xCA) and keeps waiting, and on a closed
    // connection reconnects. So a session the login server didn't open, or
    // one that has ended, is just closed; the lobby, which refuses it too,
    // sends the client back to the login screen. A lobby without a login
    // server uses the lobby's default account instead.
    let hold = store.hold(session_id);
    let account = hold
        .as_ref()
        .and_then(|_| store.account_for_session(session_id));
    let (account_id, mut name) = match account {
        Some(account) => {
            let account = account.locked();
            (account.id, account.name.clone().into_bytes())
        }
        None if unknown == UnknownSessions::Refuse => {
            log::warn!("Chat session {session_id:#x} is unknown or has ended; closed it");
            return;
        }
        None => {
            log::warn!("Unknown chat session {session_id:#x}; using a default account");
            (1, b"dummy_name".to_vec())
        }
    };
    if let Some(hold) = &hold {
        hold.attach(&writer);
    }
    name.truncate(NAME_CAPACITY - 1);
    let display_name = String::from_utf8_lossy(&name).into_owned();

    let writer = Arc::new(Mutex::new(writer));
    let sign_in = ServerMessage::SignInInfo {
        local_name: name.clone(),
    };
    if let Err(error) = send(&writer, sign_in) {
        log::info!("Session {session_id:#x} left the chat before signing in: {error}");
        return;
    }
    let membership = room.join(Member {
        account_id,
        name: name.clone(),
        channel_ids: [0; 9],
        writer: writer.clone(),
    });
    log::info!("Session {session_id:#x} joined the chat as {display_name}");
    // The general channel id of the last subscription, for the log.
    let mut language = 0;
    for request in room.requests_for(account_id) {
        if let Err(error) = send(&writer, request) {
            log::debug!("Friend request not delivered: {error}");
        }
    }

    loop {
        let message = match reader.read::<ClientMessage>() {
            Ok(message) => message,
            Err(error) if error.is_disconnect() => {
                log::info!("Session {session_id:#x} left the chat ({error})");
                return;
            }
            Err(error) => {
                log::warn!(
                    "Unreadable chat message from session {session_id:#x} ({error}); \
                     closed the connection"
                );
                return;
            }
        };
        match message {
            ClientMessage::SignInInfo { .. } => {
                log::warn!("Session {session_id:#x} signed in to the chat again; ignored");
            }
            ClientMessage::Subscription { channel_ids } => {
                log::debug!("Session {session_id:#x} chat channels: {channel_ids:x?}");
                // It comes again with every match or squad change.
                let general = messaging_message_channel_enum::player_general_channel as usize;
                if channel_ids[general] != language {
                    language = channel_ids[general];
                    log::info!(
                        "{display_name}'s client is on chat language {language} ({}); \
                         the general chat is one for all",
                        language_name(language)
                    );
                }
                room.subscribe(membership.id, channel_ids);
            }
            ClientMessage::Friendship(request) => {
                log::debug!("Session {session_id:#x} friendship request: {request:?}");
                let reply = friendship_reply(store, room, account_id, &request);
                if let Err(error) = send(&writer, reply) {
                    log::debug!("Friendship reply not delivered: {error}");
                }
            }
            ClientMessage::Text {
                channel_id,
                receiver,
                channel,
                body,
            } => {
                let recipients = room.relay(membership.id, channel_id, &receiver, channel, &body);
                log::info!(
                    "Chat from {display_name} on channel {channel} reached {recipients} player(s)"
                );
                log::debug!("Chat line: {}", String::from_utf8_lossy(&body));
            }
            ClientMessage::Important {
                target_name,
                message_type,
                text,
            } => {
                log::info!(
                    "{display_name} sent an important message of type {message_type} to {}",
                    String::from_utf8_lossy(&target_name)
                );
                let reply = important_message(
                    store,
                    room,
                    (account_id, &name),
                    &target_name,
                    message_type,
                    text,
                );
                if let Some(reply) = reply
                    && let Err(error) = send(&writer, reply)
                {
                    log::debug!("Friend request refusal not delivered: {error}");
                }
            }
            ClientMessage::ImportantAnswer { message_id, answer } => {
                // The client answers its own server notices at once; only
                // friend requests and squad invites are kept here.
                let Some(request) = room.take_request(message_id, account_id) else {
                    continue;
                };
                if answer != ACCEPT {
                    log::info!("{display_name} declined a {:?}", request.kind);
                    continue;
                }
                if request.kind == important_message_types::squad_invite {
                    log::info!("{display_name} accepted a squad invite");
                    store.squads().invite_accepted(request.from, account_id);
                    // Its "membership accepted" came first, as a client
                    // sends it: the inviter gets it now.
                    if let Some(text) = request.accepted_text {
                        let accepted = Request {
                            from: account_id,
                            from_name: name.clone(),
                            to: request.from,
                            kind: important_message_types::squad_invite_accepted,
                            text,
                            accepted_text: None,
                        };
                        let message_id = room.next_message_id.fetch_add(1, Ordering::Relaxed) + 1;
                        room.tell_account(request.from, &accepted.message(message_id));
                    }
                    continue;
                }
                let accepted = befriend(store, request.from, account_id);
                log::info!("{display_name} accepted a friend request (made: {accepted})");
                let result = ServerMessage::FriendshipResult {
                    action: friendship_actions_enum::add_friend,
                    accepted,
                };
                room.tell_account(request.from, &result);
                room.tell_account(account_id, &result);
            }
            ClientMessage::Unknown { message_type } => {
                log::warn!("Unknown chat message type {message_type:#x}; skipped");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::time::{Duration, Instant};

    /// The language id the english client subscribes to.
    const ENGLISH: u32 = 2;
    const RUSSIAN: u32 = 1;

    /// A chat server on a free local port, serving every connection with `run`.
    struct Server {
        store: Arc<SessionStore>,
        room: Arc<ChatRoom>,
        addr: std::net::SocketAddr,
    }

    impl Server {
        fn start() -> Self {
            let store = Arc::new(SessionStore::new());
            let room = Arc::new(ChatRoom::default());
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let (thread_store, thread_room) = (store.clone(), room.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let stream = stream.unwrap();
                    let (store, room) = (thread_store.clone(), thread_room.clone());
                    std::thread::spawn(move || {
                        let reader = TcpClient::new(stream.try_clone().unwrap());
                        run(&store, &room, reader, stream, UnknownSessions::Refuse);
                    });
                }
            });
            Self { store, room, addr }
        }

        fn account_id(&self, email: &str) -> u32 {
            self.store
                .get_or_create_account(IpAddr::V4(Ipv4Addr::LOCALHOST), email)
        }

        /// Sign in as the account named `email`, the way the client does.
        fn sign_in(&self, email: &str) -> Client {
            let account_id = self.account_id(email);
            let session_id = self.store.create_session(account_id);
            let mut stream = TcpStream::connect(self.addr).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut sign_in = vec![6, 0xC3];
            sign_in.extend(session_id.to_le_bytes());
            sign_in.push(5);
            stream.write_all(&sign_in).unwrap();
            let mut client = Client { stream, account_id };
            // The account name: the part of the email before the `@`.
            let name = email.split('@').next().unwrap();
            let mut expected = vec![0xCA, name.len() as u8];
            expected.extend(name.as_bytes());
            assert_eq!(client.frame(), expected);
            client
        }

        fn wait_for_members(&self, count: usize) {
            let deadline = Instant::now() + Duration::from_secs(2);
            while self.room.members.locked().len() != count {
                assert!(
                    Instant::now() < deadline,
                    "the room never had {count} members"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    struct Client {
        stream: TcpStream,
        account_id: u32,
    }

    impl Client {
        fn send(&mut self, frame: &[u8]) {
            let mut bytes = vec![frame.len() as u8];
            bytes.extend(frame);
            self.stream.write_all(&bytes).unwrap();
        }

        fn say(&mut self, channel: u8, channel_id: u32, receiver: &str, body: &str) {
            let mut frame = vec![0xC1];
            frame.extend(channel_id.to_le_bytes());
            frame.push(receiver.len() as u8);
            frame.extend(receiver.as_bytes());
            frame.push(channel);
            frame.push(body.len() as u8);
            frame.extend(body.as_bytes());
            self.send(&frame);
        }

        /// The subscription the client sends after signing in, in a match
        /// on team 1 if `match_id` isn't 0 (spec §5.4).
        fn subscribe(&mut self, language: u32, match_id: u32) {
            let mut frame = vec![0xC5];
            for channel_id in [0, language, u32::MAX, 0, 1, match_id, match_id, 0, 0] {
                frame.extend(channel_id.to_le_bytes());
            }
            self.send(&frame);
        }

        fn ask_friendship(&mut self, name: &str) {
            let mut frame = vec![0xC2, 0, 0, 0, 0, name.len() as u8];
            frame.extend(name.as_bytes());
            frame.extend([0, 0]);
            self.send(&frame);
        }

        fn answer(&mut self, message_id: u32, answer: u8) {
            let mut frame = vec![0xC6];
            frame.extend(message_id.to_le_bytes());
            frame.push(answer);
            self.send(&frame);
        }

        fn frame(&mut self) -> Vec<u8> {
            let mut len = [0_u8];
            self.stream.read_exact(&mut len).unwrap();
            let mut frame = vec![0; len[0] as usize];
            self.stream.read_exact(&mut frame).unwrap();
            frame
        }

        /// The next frame, if one arrives within a short wait.
        fn maybe_frame(&mut self) -> Option<Vec<u8>> {
            self.stream
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let mut len = [0_u8];
            let frame = match self.stream.read_exact(&mut len) {
                Ok(()) => {
                    let mut frame = vec![0; len[0] as usize];
                    self.stream.read_exact(&mut frame).unwrap();
                    Some(frame)
                }
                Err(_) => None,
            };
            self.stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            frame
        }
    }

    /// What a client receives for a line from `sender`.
    fn line_from(sender: &Client, name: &str, channel: u8, body: &str) -> Vec<u8> {
        let mut frame = vec![0xC9, 5];
        frame.extend(sender.account_id.to_le_bytes());
        frame.push(name.len() as u8);
        frame.extend(name.as_bytes());
        frame.push(channel);
        frame.push(body.len() as u8);
        frame.extend(body.as_bytes());
        frame
    }

    /// A friend request as the client receives it.
    fn friend_request(from: u32, name: &str, message_id: u32) -> Vec<u8> {
        let mut frame = vec![0xCC, 5];
        frame.extend(from.to_le_bytes());
        frame.push(name.len() as u8);
        frame.extend(name.as_bytes());
        frame.extend([0, 0]);
        frame.extend(message_id.to_le_bytes());
        frame
    }

    /// An important message as the client sends it.
    fn important(kind: u8, name: &str, text: &str) -> Vec<u8> {
        let mut frame = vec![0xC2, 0, 0, 0, 0, name.len() as u8];
        frame.extend(name.as_bytes());
        frame.push(kind);
        frame.push(text.len() as u8);
        frame.extend(text.as_bytes());
        frame
    }

    /// An important message as the client receives it.
    fn important_from(kind: u8, from: u32, name: &str, text: &str, message_id: u32) -> Vec<u8> {
        let mut frame = vec![0xCC, 5];
        frame.extend(from.to_le_bytes());
        frame.push(name.len() as u8);
        frame.extend(name.as_bytes());
        frame.push(kind);
        frame.push(text.len() as u8);
        frame.extend(text.as_bytes());
        frame.extend(message_id.to_le_bytes());
        frame
    }

    fn message_id(request: &[u8]) -> u32 {
        u32::from_le_bytes(request[request.len() - 4..].try_into().unwrap())
    }

    fn with_id(action: u8, account_id: u32) -> Vec<u8> {
        let mut frame = vec![0xC4, action];
        frame.extend(account_id.to_le_bytes());
        frame
    }

    /// A list entry: id, name, and the status byte if `status` is given.
    fn entry(account_id: u32, name: &str, status: Option<u8>) -> Vec<u8> {
        let mut bytes = account_id.to_le_bytes().to_vec();
        bytes.push(name.len() as u8);
        bytes.extend(name.as_bytes());
        bytes.extend(status);
        bytes
    }

    fn list(action: u8, entries: &[Vec<u8>]) -> Vec<u8> {
        let mut frame = vec![0xCB, action];
        frame.extend((entries.len() as u16).to_le_bytes());
        frame.extend(entries.concat());
        frame
    }

    const IN_LOBBY: u8 = 0;
    const OFFLINE: u8 = 2;

    #[test]
    fn signs_in_with_the_account_name_and_starts_with_empty_lists() {
        let server = Server::start();
        let mut client = server.sign_in("tester@test.invalid");

        client.send(&[0xC4, 7]);
        assert_eq!(client.frame(), [0xCB, 7, 0, 0]);
        client.send(&[0xC4, 8]);
        assert_eq!(client.frame(), [0xCB, 8, 0, 0]);
        client.send(&[0xC4, 9]);
        assert_eq!(client.frame(), [0xCB, 9, 0, 0]);
    }

    #[test]
    fn an_accepted_friend_request_befriends_both() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut bob = server.sign_in("bob");
        server.wait_for_members(2);

        alice.ask_friendship("BOB");
        let request = bob.frame();
        assert_eq!(
            request,
            friend_request(alice.account_id, "alice", message_id(&request))
        );
        bob.answer(message_id(&request), 0);
        assert_eq!(alice.frame(), [0xCB, 0, b'4']);
        assert_eq!(bob.frame(), [0xCB, 0, b'4']);

        alice.send(&[0xC4, 7]);
        assert_eq!(
            alice.frame(),
            list(7, &[entry(bob.account_id, "bob", Some(IN_LOBBY))])
        );
        bob.send(&[0xC4, 7]);
        assert_eq!(
            bob.frame(),
            list(7, &[entry(alice.account_id, "alice", Some(IN_LOBBY))])
        );

        // An answer counts once.
        bob.answer(message_id(&request), 0);
        assert_eq!(alice.maybe_frame(), None);

        drop(bob);
        server.wait_for_members(1);
        alice.send(&[0xC4, 9]);
        let mut statuses = vec![0xCB, 9, 1, 0];
        statuses.extend(server.account_id("bob").to_le_bytes());
        statuses.push(OFFLINE);
        assert_eq!(alice.frame(), statuses);

        // Removing a friend ends the friendship both ways.
        alice.send(&with_id(2, server.account_id("bob")));
        assert_eq!(alice.frame(), [0xCB, 2, b'4']);
        let mut bob = server.sign_in("bob");
        bob.send(&[0xC4, 7]);
        assert_eq!(bob.frame(), [0xCB, 7, 0, 0]);
    }

    #[test]
    fn an_accepted_squad_invite_lets_the_inviter_add_the_player() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut bob = server.sign_in("bob");
        server.wait_for_members(2);

        alice.send(&important(2, "Bob", "join"));
        let invite = bob.frame();
        assert_eq!(
            invite,
            important_from(2, alice.account_id, "alice", "join", message_id(&invite))
        );
        // The client's order: "membership accepted", then the answer to
        // the invite. The inviter hears of it once the answer accepts.
        bob.send(&important(3, "alice", "membership accepted"));
        assert_eq!(alice.maybe_frame(), None);

        bob.answer(message_id(&invite), 0);
        let accepted = alice.frame();
        assert_eq!(
            accepted,
            important_from(
                3,
                bob.account_id,
                "bob",
                "membership accepted",
                message_id(&accepted)
            )
        );
        assert!(
            server
                .store
                .squads()
                .has_accepted(alice.account_id, bob.account_id)
        );
    }

    /// The other order (the answer first) is taken too, and "membership
    /// accepted" with no invite at all goes nowhere.
    #[test]
    fn an_acceptance_after_the_answer_or_without_an_invite() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut bob = server.sign_in("bob");
        server.wait_for_members(2);

        bob.send(&important(3, "alice", "membership accepted"));
        assert_eq!(alice.maybe_frame(), None);

        alice.send(&important(2, "Bob", "join"));
        let invite = bob.frame();
        bob.answer(message_id(&invite), 0);
        assert_eq!(alice.maybe_frame(), None);
        bob.send(&important(3, "alice", "membership accepted"));
        let accepted = alice.frame();
        assert_eq!(
            accepted,
            important_from(
                3,
                bob.account_id,
                "bob",
                "membership accepted",
                message_id(&accepted)
            )
        );
        assert_eq!(alice.maybe_frame(), None);

        // A declined invite is no acceptance, whatever came before.
        let mut carol = server.sign_in("carol");
        server.wait_for_members(3);
        alice.send(&important(2, "carol", "join"));
        let invite = carol.frame();
        carol.send(&important(3, "alice", "membership accepted"));
        carol.answer(message_id(&invite), 1);
        assert_eq!(alice.maybe_frame(), None);
        assert!(
            !server
                .store
                .squads()
                .has_accepted(alice.account_id, carol.account_id)
        );
    }

    #[test]
    fn a_squad_invite_is_only_for_a_player_in_the_chat() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        server.account_id("bob");
        alice.send(&important(2, "bob", "join"));
        // Let the invite be handled before bob arrives.
        alice.send(&[0xC4, 7]);
        assert_eq!(alice.frame(), [0xCB, 7, 0, 0]);
        let mut bob = server.sign_in("bob");
        assert_eq!(bob.maybe_frame(), None);
    }

    #[test]
    fn a_friend_request_waits_for_its_player_and_can_be_declined() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        server.account_id("bob");

        alice.ask_friendship("bob");
        alice.ask_friendship("bob");
        let mut bob = server.sign_in("bob");
        let request = bob.frame();
        assert_eq!(
            request,
            friend_request(alice.account_id, "alice", message_id(&request))
        );
        assert_eq!(bob.maybe_frame(), None, "one request per pair");

        bob.answer(message_id(&request), 1);
        assert_eq!(alice.maybe_frame(), None);
        alice.send(&[0xC4, 7]);
        assert_eq!(alice.frame(), [0xCB, 7, 0, 0]);
    }

    #[test]
    fn friend_requests_to_nobody_or_oneself_are_refused() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        alice.ask_friendship("nobody");
        assert_eq!(alice.frame(), [0xCB, 1, 0]);
        alice.ask_friendship("alice");
        assert_eq!(alice.frame(), [0xCB, 1, 0]);
    }

    #[test]
    fn an_ignoring_player_gets_no_friend_requests_from_them() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut bob = server.sign_in("bob");
        bob.send(&with_id(3, alice.account_id));
        assert_eq!(bob.frame(), [0xCB, 3, b'4']);

        alice.ask_friendship("bob");
        assert_eq!(alice.frame(), [0xCB, 1, 0]);
        assert_eq!(bob.maybe_frame(), None);
    }

    #[test]
    fn the_ignore_list_keeps_names_without_statuses() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let bob = server.sign_in("bob");
        let mut by_name = vec![0xC4, 4, 3];
        by_name.extend(b"BOB");
        alice.send(&by_name);
        assert_eq!(alice.frame(), [0xCB, 4, b'4']);
        alice.send(&[0xC4, 8]);
        assert_eq!(
            alice.frame(),
            list(8, &[entry(bob.account_id, "bob", None)])
        );
        alice.send(&with_id(3, bob.account_id));
        assert_eq!(alice.frame(), [0xCB, 3, b'4']);
        alice.send(&[0xC4, 8]);
        assert_eq!(
            alice.frame(),
            list(8, &[entry(bob.account_id, "bob", None)]),
            "listed once"
        );
        alice.send(&with_id(5, bob.account_id));
        assert_eq!(alice.frame(), [0xCB, 5, b'4']);
        alice.send(&[0xC4, 8]);
        assert_eq!(alice.frame(), [0xCB, 8, 0, 0]);
        alice.send(&with_id(3, alice.account_id));
        assert_eq!(alice.frame(), [0xCB, 3, 0]);
    }

    #[test]
    fn players_are_found_by_part_of_their_name_but_not_oneself() {
        let server = Server::start();
        let mut anna = server.sign_in("anna");
        let hannah = server.sign_in("Hannah");
        server.sign_in("bob");
        let mut search = vec![0xC4, 6, 3];
        search.extend(b"ANN");
        anna.send(&search);
        assert_eq!(
            anna.frame(),
            list(6, &[entry(hannah.account_id, "Hannah", None)])
        );
    }

    #[test]
    fn a_closed_connection_leaves_the_room() {
        let server = Server::start();
        let client = server.sign_in("leaver@test.invalid");
        server.wait_for_members(1);

        drop(client);

        server.wait_for_members(0);
    }

    #[test]
    fn general_lines_reach_everyone_else_whatever_the_language() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut bob = server.sign_in("bob");
        let mut carol = server.sign_in("carol");
        let mut dmitri = server.sign_in("dmitri");
        let mut late = server.sign_in("late");
        alice.subscribe(ENGLISH, 0);
        bob.subscribe(ENGLISH, 0);
        carol.subscribe(RUSSIAN, 0);
        dmitri.subscribe(RUSSIAN, 0x123);
        server.wait_for_members(5);
        // Subscriptions are handled in order with the lines that follow,
        // but the others' come from other connections.
        std::thread::sleep(Duration::from_millis(100));

        alice.say(1, ENGLISH, "", "hi all");
        let expected = line_from(&alice, "alice", 1, "hi all");
        assert_eq!(bob.frame(), expected);
        assert_eq!(carol.frame(), expected);
        assert_eq!(dmitri.frame(), expected, "in a match too");
        assert_eq!(alice.maybe_frame(), None, "the client shows its own line");
        assert_eq!(late.maybe_frame(), None, "not subscribed yet");

        // Both ways, and the UTF-8 the clients send passes as it is.
        carol.say(1, RUSSIAN, "", "привіт усім");
        let expected = line_from(&carol, "carol", 1, "привіт усім");
        assert_eq!(alice.frame(), expected);
        assert_eq!(bob.frame(), expected);
        assert_eq!(dmitri.frame(), expected);
        assert_eq!(carol.maybe_frame(), None);
    }

    #[test]
    fn a_cyrillic_name_reaches_an_english_client_as_it_is() {
        let server = Server::start();
        let mut cyrillic = server.sign_in("Гравець");
        let mut alice = server.sign_in("alice");
        cyrillic.subscribe(RUSSIAN, 0);
        alice.subscribe(ENGLISH, 0);
        server.wait_for_members(2);
        std::thread::sleep(Duration::from_millis(100));

        cyrillic.say(1, RUSSIAN, "", "gg");
        assert_eq!(alice.frame(), line_from(&cyrillic, "Гравець", 1, "gg"));
    }

    #[test]
    fn only_the_general_channel_is_shared_across_languages() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut dmitri = server.sign_in("dmitri");
        alice.subscribe(ENGLISH, 0x123);
        dmitri.subscribe(RUSSIAN, 0x456);
        server.wait_for_members(2);
        std::thread::sleep(Duration::from_millis(100));

        // Another match, and a language id on the match channel.
        alice.say(5, 0x123, "", "gg");
        alice.say(5, ENGLISH, "", "gg");
        // A general line without an id: the client never sends one.
        alice.say(1, 0, "", "nobody");
        assert_eq!(dmitri.maybe_frame(), None);
    }

    #[test]
    fn private_lines_reach_only_the_named_player() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut bob = server.sign_in("bob");
        let mut carol = server.sign_in("carol");
        server.wait_for_members(3);

        alice.say(4, 0, "Bob", "psst");

        assert_eq!(bob.frame(), line_from(&alice, "alice", 4, "psst"));
        assert_eq!(carol.maybe_frame(), None);
    }

    #[test]
    fn match_and_team_lines_reach_only_the_same_match_and_team() {
        let server = Server::start();
        let mut alice = server.sign_in("alice");
        let mut bob = server.sign_in("bob");
        let mut carol = server.sign_in("carol");
        alice.subscribe(ENGLISH, 0x123);
        bob.subscribe(ENGLISH, 0x123);
        carol.subscribe(ENGLISH, 0x456);
        server.wait_for_members(3);
        std::thread::sleep(Duration::from_millis(100));

        alice.say(5, 0x123, "", "gg");
        assert_eq!(bob.frame(), line_from(&alice, "alice", 5, "gg"));
        alice.say(6, 0x123, "", "push");
        assert_eq!(bob.frame(), line_from(&alice, "alice", 6, "push"));
        // Nobody is on the second team.
        alice.say(7, 0x123, "", "wrong team");
        assert_eq!(bob.maybe_frame(), None);
        assert_eq!(carol.maybe_frame(), None);
    }

    /// Friend and ignore changes mark the accounts they change.
    #[test]
    fn friend_and_ignore_changes_are_marked_for_saving() {
        use session::persist::MemoryRepository;
        let repository = MemoryRepository::default();
        let store = SessionStore::with_repository(Duration::ZERO, Box::new(repository)).unwrap();
        let room = ChatRoom::default();
        let a = store.sign_in("alice", b"test-password-1").unwrap();
        let b = store.sign_in("bob", b"test-password-2").unwrap();
        let changed = |requests: &[FriendshipRequest], marked: &[u32]| {
            store.flush();
            for request in requests {
                friendship_reply(&store, &room, a, request);
            }
            for account in marked {
                assert!(store.is_dirty(*account), "{requests:?}: {account} unmarked");
            }
            assert_eq!(store.flush(), marked.len(), "{requests:?}");
        };
        store.flush();
        assert!(befriend(&store, a, b));
        assert!(store.is_dirty(a) && store.is_dirty(b));
        changed(
            &[FriendshipRequest::RemoveFriend { account_id: b }],
            &[a, b],
        );
        changed(&[FriendshipRequest::AddIgnored { account_id: b }], &[a]);
        changed(&[FriendshipRequest::RemoveIgnored { account_id: b }], &[a]);
        changed(
            &[FriendshipRequest::AddIgnoredByName {
                name: b"BOB".to_vec(),
            }],
            &[a],
        );
        assert_eq!(
            store
                .account(a)
                .unwrap()
                .locked()
                .ignored
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [b]
        );
    }
}
