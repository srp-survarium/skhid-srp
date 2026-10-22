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
//! - **Moderation** is the other channels': the same allowance
//!   (`social::CHAT_BURST`), an ignored sender's lines not sent (nor in the
//!   last lines), the game's own length limit and its client's censor.
//! - **The room's own lines** ("Alice joined the room", "Alice left the
//!   room"): an account line of id 0 with no name, in English; the movie
//!   puts them in the player's language. Said on the tick after the change,
//!   whatever made it (a create, a join, Play, the queue, a switch, a late
//!   join, a leave, a kick, a match it could not play, its game gone);
//!   no join for a player alone in its room, nothing for a room that is gone.
//! - **Kept** with the room only ([`session::v026g::rooms::Room::chat`]): a
//!   room gone, its chat is gone.

use std::collections::HashMap;
use std::net::SocketAddr;

use session::v026g::rooms::ChatLine;
use survarium::v026g::lobby::{self, ServerMessage, channel, sender};

use super::core::{Core, Who};

/// The room's own lines (the movie knows them by these words).
const JOINED: &str = "joined the room";
const LEFT: &str = "left the room";

fn message(line: &ChatLine) -> ServerMessage {
    lobby::chat_text(
        sender::ACCOUNT,
        line.from,
        &line.name,
        &line.tag,
        channel::ROOM,
        &line.body,
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
        let sent = message(&line);
        let from = line.from;
        self.rooms.say(key, line);
        let mut reached = 0;
        for (addr, account_id) in self.room_listeners(key) {
            if account_id == except || (from != 0 && self.ignores(account_id, from)) {
                continue;
            }
            self.send(addr, sent.clone());
            reached += 1;
        }
        log::debug!("Room {key}: a chat line reached {reached} member(s)");
    }

    /// `4C` on [`channel::ROOM`]: a member's line, to its room. One in no
    /// room reaches nobody.
    pub(super) fn room_say(&mut self, who: &Who, body: &str) {
        let Some(key) = self.rooms.key_of(who.account_id) else {
            log::info!("{}: a room chat line, but in no room", who.name);
            return;
        };
        let line = ChatLine {
            from: who.account_id,
            name: who.name.clone(),
            tag: self.clan_tag_of(who.account_id),
            body: body.to_owned(),
        };
        self.room_line(key, line, who.account_id);
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
        let Some(room) = key.and_then(|key| self.rooms.get(key)) else {
            return;
        };
        let lines: Vec<ServerMessage> = room
            .chat
            .iter()
            .filter(|line| line.from == 0 || !self.ignores(who.account_id, line.from))
            .map(message)
            .collect();
        for line in lines {
            self.send(who.addr, line);
        }
    }

    /// Each tick: who came into a room and who went since the last, said in
    /// its room; a member that left its room hears it no more.
    pub(super) fn room_chat_tick(&mut self) {
        let now: HashMap<u32, u32> = self
            .rooms
            .all()
            .flat_map(|room| room.members.iter().map(move |&member| (member, room.key)))
            .collect();
        let before = std::mem::replace(&mut self.room_seen, now.clone());
        let mut left: Vec<(u32, u32)> = before
            .iter()
            .filter(|&(account, key)| now.get(account) != Some(key))
            .map(|(&account, &key)| (account, key))
            .collect();
        let mut came: Vec<(u32, u32)> = now
            .iter()
            .filter(|&(account, key)| before.get(account) != Some(key))
            .map(|(&account, &key)| (account, key))
            .collect();
        left.sort_unstable();
        came.sort_unstable();
        for addr in self.players() {
            if let Some(player) = self.player_mut(addr)
                && player.room_chat.is_some()
                && player.room_chat != now.get(&player.account_id).copied()
            {
                player.room_chat = None;
            }
        }
        for (account, key, what) in left
            .into_iter()
            .map(|(account, key)| (account, key, LEFT))
            .chain(
                came.into_iter()
                    .map(|(account, key)| (account, key, JOINED)),
            )
        {
            // A room that is gone hears nothing; a player alone in its room
            // is not told it came.
            let Some(room) = self.rooms.get(key) else {
                continue;
            };
            if what == JOINED && room.members.len() < 2 {
                continue;
            }
            let Some(name) = self.name_of(account) else {
                continue;
            };
            let line = ChatLine {
                from: 0,
                name: String::new(),
                tag: String::new(),
                body: format!("{name} {what}"),
            };
            self.room_line(key, line, 0);
        }
    }
}
