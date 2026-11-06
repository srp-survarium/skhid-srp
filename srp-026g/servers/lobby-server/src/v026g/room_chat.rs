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
//!   [`rooms::CHAT_KEPT`](session::v026g::rooms::CHAT_KEPT)), and as long as
//!   it is a member of that room. Not a room's queue. A game that never asks
//!   (no such tab) is sent nothing.
//! - **A member in a match** is sent nothing (its game's match chat is its
//!   own, and is not touched): no line, no last lines. Its movie asks again
//!   after the match (`SrpRooms.afterMatch`), and its tab shows the room's
//!   last lines then, those said during the match among them.
//! - **At the match's start** (the owner's of 2026-11-06: "when the match
//!   starts last messages can be shown too from room chat"): the hud asks
//!   with [`MATCH_HISTORY_ASK`] on [`channel::ROOM`] once its match chat is
//!   up, and is given the room's last [`MATCH_HISTORY`] lines on that
//!   channel; it shows them in every match tab.
//! - **The match's all-chat** (the owner's of 2026-11-06: "show history
//!   across the previous match of messages to all"): a line a member says
//!   on [`channel::MATCH`] in its match is kept in its room's chat too, so
//!   the tab shows it after the match. Team lines are not kept. Members in
//!   the lobby (one who came during the match) hear it as it is said.
//! - **Moderation** is the other channels': the same allowance
//!   (`social::CHAT_BURST`), an ignored sender's lines not sent (nor in the
//!   last lines), the game's own length limit and its client's censor.
//! - **Who came and went** is not said (the owner's of 2026-11-06).
//! - **Kept** with the room only ([`session::v026g::rooms::Room::chat`]): a
//!   room gone, its chat is gone.

use std::net::SocketAddr;

use session::v026g::rooms::ChatLine;
use survarium::v026g::lobby::{self, ServerMessage, channel, sender};

use super::core::{Core, Who};

/// The hud's ask for its room's last lines, on [`channel::ROOM`] once its
/// match chat is up (`flash/hud`, `HUD.as`).
pub(super) const MATCH_HISTORY_ASK: &str = "#room-chat";
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

    /// A line of `who` in room `key`'s chat.
    fn say_in_room(&mut self, key: u32, who: &Who, body: &str) {
        let line = ChatLine {
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
        self.say_in_room(key, who, body);
    }

    /// `4C` on [`channel::MATCH`] from a player in a match: its all-chat
    /// line, kept in its room's chat too. A player in no room: nothing.
    pub(super) fn room_keep_match_line(&mut self, who: &Who, body: &str) {
        if let Some(key) = self.rooms.key_of(who.account_id) {
            self.say_in_room(key, who, body);
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
        for line in key.map_or_else(Vec::new, |key| self.room_history(key, who.account_id)) {
            self.send(who.addr, line);
        }
    }

    /// [`MATCH_HISTORY_ASK`]: a player in a match is given its room's last
    /// [`MATCH_HISTORY`] lines (but an ignored sender's), which its hud
    /// shows in the match chat. Out of a match or of a room: nothing.
    pub(super) fn room_history_to_match(&mut self, who: &Who) {
        if self
            .player(who.addr)
            .is_none_or(|player| player.in_match.is_none())
        {
            return;
        }
        let Some(key) = self.rooms.key_of(who.account_id) else {
            return;
        };
        let lines = self.room_history(key, who.account_id);
        let skip = lines.len().saturating_sub(MATCH_HISTORY);
        for line in lines.into_iter().skip(skip) {
            self.send(who.addr, line);
        }
    }

    /// Room `key`'s kept lines as `account_id` is sent them, the oldest
    /// first; none for a room that is gone.
    fn room_history(&self, key: u32, account_id: u32) -> Vec<ServerMessage> {
        let Some(room) = self.rooms.get(key) else {
            return Vec::new();
        };
        room.chat
            .iter()
            .filter_map(|line| {
                self.chat_body(account_id, line.from, &line.body)
                    .map(|body| message(line, body))
            })
            .collect()
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
