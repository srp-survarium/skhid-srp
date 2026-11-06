//! A room's own chat (the owner's of 2026-10-20: "each room has its own
//! chat", "and you should be able to type in one"; `social.md` §9.8,
//! `rooms.md`).
//!
//! - **The channel** is srp's own, [`channel::ROOM`] (10): the game's
//!   client sends a line typed in a tab that sends on 10 with channel id 0
//!   and shows a received one in the tabs that list 10. The tab is srp's
//!   lobby movie's ("Room Chat", right of the general chat), up while its
//!   player is in a room.
//! - **Who hears it:** the room's members, each once its movie said its tab
//!   is up ([`RoomRequest::Chat`](survarium::v026g::lobby::rooms::RoomRequest),
//!   which also gives it the room's last lines,
//!   [`rooms::CHAT_KEPT`](session::v026g::rooms::CHAT_KEPT) at most), and as
//!   long as it is a member of that room. Not a room's queue. A game that
//!   never asks (no such tab) is sent nothing.
//! - **A member in a match** is not sent the room's lines as they are said
//!   (its game's match chat is its own). Its movie asks again after the
//!   match (`SrpRooms.afterMatch`), and its tab shows the room's last lines
//!   then, those said during the match among them.
//! - **At the match's start** (the owner's of 2026-11-06: "when the match
//!   starts last messages can be shown too from room chat"): the hud asks
//!   ([`HudRequest::RoomHistory`], `RoomHistory.as`) once its match chat is
//!   up, and a player of its room's match is given the room's last
//!   [`MATCH_HISTORY`] lines on [`channel::ROOM`], then "done"; it shows
//!   them in every match tab. Once a match: a retry, or a reconnect, is
//!   answered "done" only.
//! - **The match's all-chat** (the owner's of 2026-11-06: "show history
//!   across the previous match of messages to all"): a line a player of
//!   its room's match says on [`channel::MATCH`] is kept in the room's chat
//!   too ([`Said::InMatch`], kept apart from the room's own lines), so the
//!   tab shows it after the match. Team lines are not kept, nor another
//!   match's (one against bots). Members in the lobby (one who came during
//!   the match) hear it as it is said.
//! - **Moderation** is the other channels': the same allowance
//!   (`social::CHAT_BURST`), an ignored sender's lines not sent (nor in the
//!   last lines), the game's own length limit and its client's censor.
//! - **Who came and went** is not said (the owner's of 2026-11-06).
//! - **Kept** with the room only ([`session::v026g::rooms::Room::chat`]): a
//!   room gone, its chat is gone.
//!
//! [`HudRequest::RoomHistory`]: super::social::HudRequest

use std::net::SocketAddr;

use session::v026g::rooms::{CHAT_KEPT, ChatLine, Said, Stage};
use survarium::v026g::lobby::{self, ServerMessage, channel, sender};

use super::core::{Core, Who};
use super::social::Nonce;

/// How many of the room's last lines a match's chat is given: as many as
/// the hud's log shows.
const MATCH_HISTORY: usize = 10;

fn message(line: &ChatLine, body: &str) -> ServerMessage {
    lobby::chat_text(
        sender::ACCOUNT,
        line.from,
        &line.name,
        &line.tag,
        channel::ROOM,
        body,
    )
}

impl Core<'_> {
    /// The links of room `key`'s members whose tab is up, with their
    /// accounts; none in a match.
    fn room_listeners(&self, key: u32) -> Vec<(SocketAddr, u32)> {
        self.players()
            .into_iter()
            .filter_map(|addr| {
                let player = self.player(addr)?;
                (player.room_chat == Some(key)
                    && player.in_match.is_none()
                    && self.rooms.key_of(player.account_id) == Some(key))
                .then_some((addr, player.account_id))
            })
            .collect()
    }

    /// The key of the room whose match `match_id` is, when `account_id` is
    /// one of its members playing it; none for another match (one against
    /// bots, started from a room's vote).
    fn room_of_match(&self, account_id: u32, match_id: u32) -> Option<u32> {
        self.rooms
            .of(account_id)
            .filter(|room| {
                matches!(room.stage, Stage::InMatch { match_id: id, .. } if id == match_id)
                    && room.in_match.contains(&account_id)
            })
            .map(|room| room.key)
    }

    /// A line in room `key`'s chat: kept, and sent to its members but
    /// `except` (the sender: its game showed its own line) and those who
    /// ignore its sender.
    fn room_line(&mut self, key: u32, line: ChatLine, except: u32) {
        let from = line.from;
        self.rooms.say(key, line.clone());
        let mut reached = 0;
        for (addr, account_id) in self.room_listeners(key) {
            if account_id == except {
                continue;
            }
            let Some(body) = self.chat_body(account_id, from, &line.body) else {
                continue;
            };
            self.send(addr, message(&line, body));
            reached += 1;
        }
        log::debug!("Room {key}: a chat line reached {reached} member(s)");
    }

    /// A line of `who` in room `key`'s chat, said where `said` is.
    fn say_in_room(&mut self, key: u32, said: Said, who: &Who, body: &str) {
        let line = ChatLine {
            said,
            from: who.account_id,
            name: who.name.clone(),
            tag: self.clan_tag_of(who.account_id),
            body: body.to_owned(),
        };
        self.room_line(key, line, who.account_id);
    }

    /// `4C` on [`channel::ROOM`]: a member's line, to its room. One in no
    /// room reaches nobody.
    pub(super) fn room_say(&mut self, who: &Who, body: &str) {
        let Some(key) = self.rooms.key_of(who.account_id) else {
            log::info!("{}: a room chat line, but in no room", who.name);
            return;
        };
        self.say_in_room(key, Said::InRoom, who, body);
    }

    /// `4C` on [`channel::MATCH`] from a player in match `match_id`: its
    /// all-chat line, kept in its room's chat too when it is the room's
    /// match.
    pub(super) fn room_keep_match_line(&mut self, who: &Who, match_id: u32, body: &str) {
        if let Some(key) = self.room_of_match(who.account_id, match_id) {
            self.say_in_room(key, Said::InMatch, who, body);
        }
    }

    /// The movie's tab is up ([`RoomRequest::Chat`]): the member hears its
    /// room from now on, and gets its last lines (but an ignored sender's).
    /// Out of a room it hears none; in a match it is given no line (its
    /// movie asks again after it).
    ///
    /// [`RoomRequest::Chat`]: survarium::v026g::lobby::rooms::RoomRequest
    pub(super) fn room_chat_listen(&mut self, who: &Who) {
        let key = self.rooms.key_of(who.account_id);
        let Some(player) = self.player_mut(who.addr) else {
            return;
        };
        player.room_chat = key;
        if player.in_match.is_some() {
            return;
        }
        let lines = key.map_or_else(Vec::new, |key| {
            self.room_history(key, who.account_id, CHAT_KEPT)
        });
        for line in lines {
            self.send(who.addr, line);
        }
    }

    /// The hud's ask ([`HudRequest::RoomHistory`]) from a player in match
    /// `match_id`: a player of its room's match is given the room's last
    /// [`MATCH_HISTORY`] lines (but an ignored sender's), once a match;
    /// every ask is answered "done" after them, so the hud stops asking.
    ///
    /// [`HudRequest::RoomHistory`]: super::social::HudRequest
    pub(super) fn room_history_to_match(&mut self, who: &Who, match_id: u32, nonce: Nonce<'_>) {
        let first = self
            .matches
            .get_mut(&match_id)
            .is_some_and(|running| running.room_histories.insert(who.account_id));
        if first && let Some(key) = self.room_of_match(who.account_id, match_id) {
            for line in self.room_history(key, who.account_id, MATCH_HISTORY) {
                self.send(who.addr, line);
            }
        }
        self.hud_answer(who, &format!("#room-chat:{}:done", nonce.as_str()));
    }

    /// Room `key`'s last `at_most` lines as `account_id` is sent them (an
    /// ignored sender's left out), the oldest first; none for a room that
    /// is gone.
    fn room_history(&self, key: u32, account_id: u32, at_most: usize) -> Vec<ServerMessage> {
        let Some(room) = self.rooms.get(key) else {
            return Vec::new();
        };
        let mut lines: Vec<ServerMessage> = room
            .chat
            .iter()
            .rev()
            .filter_map(|line| {
                self.chat_body(account_id, line.from, &line.body)
                    .map(|body| message(line, body))
            })
            .take(at_most)
            .collect();
        lines.reverse();
        lines
    }

    /// Each tick: a member that left its room hears it no more.
    pub(super) fn room_chat_tick(&mut self) {
        for addr in self.players() {
            let Some(player) = self.player(addr) else {
                continue;
            };
            let left = player
                .room_chat
                .is_some_and(|key| self.rooms.key_of(player.account_id) != Some(key));
            if left && let Some(player) = self.player_mut(addr) {
                player.room_chat = None;
            }
        }
    }
}
