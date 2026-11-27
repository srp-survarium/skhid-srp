//! Chat, friends, ignore, search and presence across the connected peers (4C 4D 4E 51 in; AF, B0 out). Peers are found
//! through `Registry`; effects on other accounts go out as `Cross` messages after the handler returned.
//!
//! * Chat 4C: channel 1 (general) goes to every signed-in peer but the sender and those who ignore the sender; channel 4
//!   (private) goes to the target account (`target` = the u64 public id of the friend record, PROVEN 0x200264519);
//!   the sender never gets their own line back (the client prints it itself, PROVEN 0x2643E0). Other channels (squad, team, ...) belong to the squad wave.
//! * Friends: a request shows on the target as an invite (B0 status 4, plus a system line); answering with 4D kind 0
//!   accepts (the store makes them friends at once), 4E 01 declines / removes; every change pushes B0 05 to both.
//! * Presence: B0 07 (status of every row of the receiver's list, the client applies it only when n equals its list
//!   size) to the online friends on sign-in, sign-out, match start and match end.

use accounts::{account_of_public_id, public_id, AccountId, FriendState};
use protocol::c2s::{Friends4E, Request};
use protocol::s2c::*;

use crate::dispatch::Ctx;
use crate::peer::{Cross, Peer};

/// B0 status byte (FriendRender.as): in the lobby, in a match, offline, invite, search.
pub const ST_LOBBY: u8 = 0;
pub const ST_INGAME: u8 = 1;
pub const ST_OFFLINE: u8 = 2;
pub const ST_INVITE: u8 = 4;
pub const ST_SEARCH: u8 = 5;

fn status_of(cx: &Ctx, account: AccountId) -> u8 {
    if cx.registry.is_online(account) {
        cx.registry.status(account)
    } else {
        ST_OFFLINE
    }
}

/// B0 05 rows of an account: friends (online status or offline). Requests are not rows: a received one is the client's
/// own Requests entry ([`KIND_FRIEND_REQUEST`]; a status-4 row would show it twice, with dead buttons), a sent one is
/// not shown.
fn friend_rows(cx: &Ctx, me: AccountId) -> Vec<Person> {
    let rows = cx.lobby.store.friends(me).unwrap_or_else(|e| {
        log::warn!("friends of {me}: {e}");
        vec![]
    });
    rows.into_iter()
        .filter_map(|f| {
            let status = match f.state {
                FriendState::Friend => status_of(cx, f.id),
                FriendState::RequestReceived | FriendState::RequestSent => return None,
            };
            Some(Person { id: public_id(f.id), name: f.name, clan: String::new(), status, seconds: 0 })
        })
        .collect()
}

/// `B1` type of a friend request (PROVEN 0x2001C2918..0x2001C2C3E, movie `LobbyMenu.add_important_message`
/// FRIEND_INVITE, `FriendRender`; docs/notes/social.md): the receiver's friends window lists it under
/// "REQUESTS" as `[ABBR] name` with the client's own `st_add_to_friends_request` and Add / Cancel, the Friends button
/// counts and blinks. The client reads sender id, name, clan tag and message id (the text is ignored: empty). Its only
/// answer is `51 id answer`, 0 = Add, 1 = Cancel ([`answer_friend_request`]).
pub const KIND_FRIEND_REQUEST: u8 = 0;
/// Message ids of friend requests: this base | the requester's account id (stable: a re-delivery at sign-in is the same
/// row; the notices use 0x1000_0000..0x1FFF_FFFF).
const FRIEND_REQUEST_IDS: u32 = 0x4000_0000;

fn friend_request_msg(cx: &Ctx, from: AccountId, name: &str) -> S2c {
    S2c::SystemMessage(SystemMessage {
        kind: KIND_FRIEND_REQUEST,
        sender: public_id(from),
        name: name.into(),
        clan: cx.lobby.store.clan_abbr_of(from),
        message_id: FRIEND_REQUEST_IDS | (from as u32 & 0x3FFF_FFFF),
        ..Default::default()
    })
}

/// At sign-in: every request the account received and has not answered (the client keeps them per session only),
/// except from accounts it ignores.
pub fn pending_friend_requests(cx: &Ctx, peer: &mut Peer) {
    let Some(me) = peer.account else { return };
    for f in cx.lobby.store.friends(me).unwrap_or_default().into_iter().filter(|f| f.state == FriendState::RequestReceived) {
        if !cx.lobby.store.is_ignored(me, f.id).unwrap_or(false) {
            peer.push(&friend_request_msg(cx, f.id, &f.name));
        }
    }
}

/// `51 id answer` of a friend request ([`KIND_FRIEND_REQUEST`]): 0 = Add (the two become friends), 1 = Cancel (the request
/// is removed); both lists are pushed. False when `id` is not a friend request id (another message's read ack).
fn answer_friend_request(cx: &mut Ctx, peer: &mut Peer, me: AccountId, id: u32, answer: u8) -> bool {
    if id & 0xC000_0000 != FRIEND_REQUEST_IDS {
        return false;
    }
    let from = AccountId::from(id & 0x3FFF_FFFF);
    let pending = cx.lobby.store.friends(me).unwrap_or_default().iter().any(|f| f.id == from && f.state == FriendState::RequestReceived);
    if !pending {
        log::info!("{}: 51 friend request {id:#x}: no such pending request", peer.addr);
        return true;
    }
    let r = if answer == 0 { cx.lobby.store.friend_request(me, from).map(|s| format!("{s:?}")) } else { cx.lobby.store.friend_remove(me, from).map(|_| "removed".to_string()) };
    match r {
        Ok(s) => log::info!("{}: friend request of {from} answered {answer}: {s}", peer.addr),
        Err(e) => log::warn!("{}: friend request of {from}: {e}", peer.addr),
    }
    peer.push(&friends_msg(cx, me));
    tell(cx, from);
    true
}

pub fn friends_msg(cx: &Ctx, me: AccountId) -> S2c {
    S2c::Messaging(Messaging::Friends(friend_rows(cx, me)))
}

/// B0 07: the status of every row of the account's list (n must equal the client's list size).
pub fn status_msg(cx: &Ctx, me: AccountId) -> S2c {
    S2c::Messaging(Messaging::Status(friend_rows(cx, me).into_iter().map(|p| (p.id, p.status)).collect()))
}

/// Set the account's presence and push B0 07 to its online friends. Use `ST_OFFLINE` after the registry dropped it.
pub fn announce(cx: &mut Ctx, me: AccountId, status: u8) {
    if status != ST_OFFLINE {
        cx.registry.set_status(me, status);
    }
    let friends = cx.lobby.store.friends(me).unwrap_or_default();
    for f in friends.into_iter().filter(|f| f.state == FriendState::Friend && cx.registry.is_online(f.id)) {
        let msg = status_msg(cx, f.id);
        cx.fx.push(Cross { to: f.id, msg });
    }
}

/// The peer of an account went away (timeout, disconnect): its friends see it offline. The registry no longer has it.
pub fn on_gone(cx: &mut Ctx, account: AccountId) {
    announce(cx, account, ST_OFFLINE);
    // the room is left (a host leaving closes it) and the clan sees the member offline; there is no peer any more
    let mut gone = Peer::new(std::net::SocketAddr::from(([0, 0, 0, 0], 0)), cx.now);
    crate::rooms::on_gone(cx, &mut gone, account);
    crate::clans::presence_changed(cx, account);
}

pub fn ignored_rows(cx: &Ctx, peer: &Peer) -> Vec<(u64, String)> {
    let Some(me) = peer.account else { return vec![] };
    cx.lobby.store.ignored(me).unwrap_or_default().into_iter().map(|a| (public_id(a.id), a.name)).collect()
}

fn player_line(cx: &Ctx, me: AccountId, name: &str, channel: u8, text: &str) -> S2c {
    let clan = cx.lobby.store.clan_abbr_of(me);
    S2c::Chat(Chat {
        lines: vec![ChatLine { kind: 5, sender: public_id(me), age: 0, name: name.into(), clan, channel, text: text.into(), ..Default::default() }],
    })
}

/// After the lobby-load handshake (the UI calls of B0 05 need the lobby movie, like 91).
pub fn after_handshake(cx: &mut Ctx, peer: &mut Peer) {
    let Some(me) = peer.account else { return };
    peer.push(&friends_msg(cx, me));
    // no welcome chat line: AF lines are not localized and no client key says "chat connected"
}

fn tell(cx: &mut Ctx, to: AccountId) {
    if cx.registry.is_online(to) {
        let msg = friends_msg(cx, to);
        cx.fx.push(Cross { to, msg });
    }
}

/// 4C: relay to the other peers (see the module docs).
fn chat(cx: &mut Ctx, peer: &mut Peer, me: AccountId, channel: u8, target: u64, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let line = player_line(cx, me, &peer.name, channel, text);
    match channel {
        1 => {
            let all: Vec<AccountId> = cx.registry.online().map(|(a, _)| a).collect();
            for to in all.into_iter().filter(|a| *a != me) {
                if cx.lobby.store.is_ignored(to, me).unwrap_or(false) {
                    continue;
                }
                cx.fx.push(Cross { to, msg: line.clone() });
            }
        }
        4 => {
            // channel 4 carries the friend record's u64 public id (exe 0x200264519; -1 = nobody, never sent)
            let Some(to) = account_of_public_id(target) else { return };
            if to == me {
                return;
            }
            if !cx.registry.is_online(to) {
                // AF lines are not localized: the lobby's 84 4C text is (the game's own "Offline")
                peer.push(&S2c::Denied(Denied { request: 0x4C, body: DeniedBody::Text("st_friend_state_offline".into()) }));
            } else if !cx.lobby.store.is_ignored(to, me).unwrap_or(false) {
                cx.fx.push(Cross { to, msg: line });
            }
        }
        // clan (3), squad (8), custom match (9), match / team (5..7): the context id must be the sender's own
        3 | 5..=9 => {
            for to in crate::chat::audience(cx, me, channel, target) {
                if !cx.lobby.store.is_ignored(to, me).unwrap_or(false) {
                    cx.fx.push(Cross { to, msg: line.clone() });
                }
            }
        }
        // 2 is the system channel (server only); anything else is not a known channel
        _ => log::debug!("{}: 4C channel {channel} ignored", peer.addr),
    }
}

/// 4C chat, 4D people, 4E friend list, 51 read message. 4D squad kinds (2, 3, 0x17, 0x18) belong to the squad wave.
pub fn handle(cx: &mut Ctx, peer: &mut Peer, req: &Request) {
    let Some(me) = peer.account else { return };
    match req {
        Request::Chat { channel, target, text } => {
            log::info!("{}: 4C chat channel {channel} target {target}: {text:?}", peer.addr);
            chat(cx, peer, me, *channel, *target, text);
        }
        Request::People { id, kind, .. } => {
            log::info!("{}: 4D kind {kind} id {id:#x}", peer.addr);
            if *kind != 0 {
                return;
            }
            let Some(other) = account_of_public_id(*id) else { return };
            match cx.lobby.store.friend_request(me, other) {
                Ok(state) => {
                    log::info!("{}: friend request to {other}: {state:?}", peer.addr);
                    peer.push(&friends_msg(cx, me));
                    tell(cx, other);
                    // the receiver gets the client's own Requests entry (not from an account it ignores)
                    if state == FriendState::RequestSent && cx.registry.is_online(other) && !cx.lobby.store.is_ignored(other, me).unwrap_or(false) {
                        let msg = friend_request_msg(cx, me, &peer.name);
                        cx.fx.push(Cross { to: other, msg });
                    }
                }
                Err(e) => log::info!("{}: friend request to {other} refused: {e}", peer.addr),
            }
        }
        Request::Friends(f) => match f {
            Friends4E::Remove { id } => {
                if let Some(other) = account_of_public_id(*id) {
                    if let Err(e) = cx.lobby.store.friend_remove(me, other) {
                        log::warn!("4E remove: {e}");
                    }
                    peer.push(&friends_msg(cx, me));
                    tell(cx, other);
                }
            }
            Friends4E::AddIgnore { id } => {
                if let Some(other) = account_of_public_id(*id) {
                    if let Err(e) = cx.lobby.store.ignore_add(me, other) {
                        log::info!("4E ignore: {e}");
                    }
                }
                peer.push(&S2c::Messaging(Messaging::Ignored(ignored_rows(cx, peer))));
            }
            Friends4E::RemoveIgnored { id } => {
                if let Some(other) = account_of_public_id(*id) {
                    if let Err(e) = cx.lobby.store.ignore_remove(me, other) {
                        log::warn!("4E unignore: {e}");
                    }
                }
                peer.push(&S2c::Messaging(Messaging::Ignored(ignored_rows(cx, peer))));
            }
            Friends4E::Find { query } => {
                let rows = if query.is_empty() {
                    vec![]
                } else {
                    cx.lobby.store.search_accounts(query, 20, Some(me)).unwrap_or_default()
                };
                log::info!("{}: 4E 04 find {query:?} -> {} rows", peer.addr, rows.len());
                let people = rows
                    .into_iter()
                    .map(|a| Person { id: public_id(a.id), status: status_of(cx, a.id), name: a.name, clan: String::new(), seconds: 0 })
                    .collect();
                peer.push(&S2c::Messaging(Messaging::Found(people)));
            }
        },
        // 51 (read important message) needs no answer: the client removes the message from its own list. After the
        // level-up / rank-up notices (notices.rs) it follows with 34, which needs none either (dispatch.rs). Notices are
        // not stored, so there is nothing to mark as read.
        Request::ReadMessage { id, a } => {
            if !answer_friend_request(cx, peer, me, *id, *a) {
                log::debug!("{}: 51 message {id} read", peer.addr);
            }
        }
        _ => {}
    }
}
