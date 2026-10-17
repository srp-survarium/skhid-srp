//! The links: one per client address, from its sign-in (`2E`, the
//! transport's connect payload) to its end.
//!
//! - A sign-in is taken only for a session the store holds
//!   ([`SessionStore::hold`]); the link keeps the hold while it lasts. One
//!   that is refused is disconnected (`FD`, then the transport's own
//!   disconnect): until its 0x80 the client reads nothing else
//!   (`lobby.md` H2). So is one whose game is older than the server takes
//!   (`patch_check`), and its session ends.
//! - The pushes of a sign-in wait until the client has acked its 0x80. The
//!   client's sign-in handler ignores every other message (`lobby.md` H2),
//!   and 0x80 is not ordered against the pushes (transport.md §4.1), so a
//!   push that overtook a lost 0x80 would be thrown away.
//! - A link ends when the client leaves (`FA`), sends nothing for
//!   [`Settings::link_timeout`], its session ends elsewhere (a ban: it is
//!   sent `AA` first), or its account signs in again (`AA` to the old
//!   link). The session ends with its link, as 0.26g has no sign-out
//!   (`login_chat.md` §4.6), but for a link that went silent (below); the
//!   store keeps an ended session that is seated in a match until the
//!   match is over.
//! - Two links do not end the session. A client that connects again with
//!   the session of a link that still stands (its own link broke: it gives
//!   up after 5 s without a datagram, sooner than the server) takes the
//!   link's place. And the session of a link that goes silent waits for
//!   its client to connect again, through the match of a player who was in
//!   one: `away` has the rule.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::v026g::replays::Replays;
use session::v026g::rewards::Results;
use session::v026g::wire::{self, SignInInfo};
use session::v026g::{Account, GameAccounts, bonus, premium, quests, tutorial};
use session::{LockExt, SessionHold, SessionStore};
use survarium::v026g::drops::Rng;
use survarium::v026g::lobby::requests::{self, ClientMessage, ParseError};
use survarium::v026g::lobby::{self, MINUTES_TO_MAINTENANCE, ServerMessage, WireError};
use udp_transport::{Direction, Event, PacketLog, State, Table, Transport};

use super::play::{MatchHost, Running};
use super::social::Request;
use super::{Inbox, Notice, Settings};

/// The most links kept at once, and the most of them not signed in (being
/// refused or closed): datagrams from more addresses are dropped.
const MAX_LINKS: usize = 4096;
const MAX_CLOSING: usize = 256;
/// The online counts go to each client again this often, changed or not,
/// and once more on the first pass after its sign-in.
const ONLINE_STATS_EVERY: Duration = Duration::from_secs(5);
/// How long a link being signed out waits for its `AA` to be acked.
const SIGN_OUT_WAIT: Duration = Duration::from_secs(1);

/// A signed-in client.
pub(super) struct Player<'a> {
    pub session_id: u32,
    pub account_id: u32,
    /// The name the lobby shows for the account.
    pub name: String,
    pub account: Arc<Mutex<Account>>,
    /// Keeps the session seen while the link lasts.
    _hold: SessionHold<'a>,
    /// The address the client reached srp at, if it could be found: what a
    /// client on this machine's network is told for the match server too.
    pub reached: Option<IpAddr>,
    /// The chat channels' ids (`4F`); all 0 until the client subscribes.
    pub channel_ids: [u32; 10],
    /// The game's language, as `4F` and `45` give it (0 English, 1
    /// Russian, 2 Ukrainian, 3 German, 4 Polish); `None` before either.
    pub language: Option<u8>,
    /// When it pressed Play, while it waits.
    pub queued_since: Option<Instant>,
    /// When its join was last said in the general chat, and whether that
    /// join is still to be answered by a leave line (`queue_chat`).
    pub queue_line: Option<Instant>,
    pub queue_announced: bool,
    /// The order id of its Play (0x85), which `31` gives back.
    pub order_id: u32,
    /// The match (the queue's id) it was sent to, while it lasts.
    pub in_match: Option<u32>,
    /// A 0x82 waiting for the client state sent before it (by its serial)
    /// to be acked (see `play`).
    pub connect: Option<(Option<u64>, ServerMessage)>,
    /// The squad info (0xA1 body) the client has, and its send.
    pub squad_sent: Option<Vec<u8>>,
    pub squad_serial: Option<u64>,
    /// The balance the client has.
    pub money_sent: (u32, u32),
    /// The count of ranked players the client's ranking page has.
    pub ranked_sent: u32,
    /// The online counts the client was last sent, and when (`None`: only
    /// with the sign-in's pushes).
    pub online_sent: (u32, u32, u32),
    pub online_at: Option<Instant>,
    /// The friends of the last `A3 07`, with the statuses the client has.
    pub friends_sent: Vec<(u32, u8)>,
    /// Chat lines it may still send now, and when that was counted.
    pub chat_allowance: f32,
    pub chat_at: Instant,
    /// The name it was last told is no player's (`st_player_not_found`),
    /// and when: not said again for each line (`social`).
    pub not_found: Option<(String, Instant)>,
}

pub(super) enum Stage<'a> {
    Player(Box<Player<'a>>),
    /// Signed out by the server: its `AA` is on its way.
    Leaving {
        serial: Option<u64>,
        until: Instant,
    },
    /// The transport is disconnecting; the link goes when it is done.
    Closing,
}

pub(super) struct Link<'a> {
    pub transport: Transport,
    pub stage: Stage<'a>,
    /// The serial of the 0x80, and what waits for its ack.
    pub held: Option<(u64, Vec<ServerMessage>)>,
    /// Everything pushed to the link, for the tests.
    #[cfg(test)]
    pub pushed: Vec<ServerMessage>,
}

impl Link<'_> {
    fn write(&mut self, message: &ServerMessage) -> Option<u64> {
        match self.transport.send(message.id_byte(), message.body()) {
            Ok(serial) => Some(serial),
            Err(error) => {
                // A writer's limit was missed, or the link is closing.
                log::error!(
                    "Lobby message {:#04X} ({} bytes) was not sent: {error:?}",
                    message.id_byte(),
                    message.body().len()
                );
                None
            }
        }
    }

    /// Queue a message: behind the 0x80's ack if that is still out.
    fn push(&mut self, message: ServerMessage) -> Option<u64> {
        #[cfg(test)]
        self.pushed.push(message.clone());
        if let Some((_, held)) = &mut self.held {
            held.push(message);
            return None;
        }
        self.write(&message)
    }
}

/// Who sent a request: what a handler needs of the player, copied out so
/// the handler may change the links.
#[derive(Clone)]
pub(super) struct Who {
    pub addr: SocketAddr,
    pub session_id: u32,
    pub account_id: u32,
    pub name: String,
    pub account: Arc<Mutex<Account>>,
}

pub(super) struct Core<'a> {
    pub store: &'a SessionStore,
    pub accounts: &'a GameAccounts,
    /// The finished matches: their results screens and who has not seen
    /// its own (`play`).
    pub results: &'a Results,
    pub host: &'a dyn MatchHost,
    inbox: &'a Inbox,
    pub settings: Settings,
    pub packet_log: Option<PacketLog>,
    pub links: HashMap<SocketAddr, Link<'a>>,
    /// The link of each account in the lobby.
    pub by_account: HashMap<u32, SocketAddr>,
    /// Friend requests and squad invites waiting for an answer, by message
    /// id (`social`).
    pub requests: BTreeMap<u32, Request>,
    pub next_message_id: u32,
    pub next_order_id: u32,
    /// The matches made and not over, by the queue's match id (`play`).
    pub matches: HashMap<u32, Running>,
    pub rng: Rng,
    /// The premium shop's settings (`session::v026g::premium`): the
    /// environment's.
    pub shop: premium::Settings,
    /// The offer made with a match's results (`A8`), by account: the match
    /// and when the offer ends, Unix seconds (`shop`).
    pub premium_offers: HashMap<u32, (u32, u64)>,
    next_tick: Instant,
    /// The ranked players, and the count of accounts they were counted at.
    ranked: (usize, u32),
    /// A squad changed, or a member's link or kit did: the squad infos are
    /// compared with what the clients have on the next pass.
    pub squads_dirty: bool,
    /// The clan teams waiting for a match and the invites under way
    /// (`clans`).
    pub clan: super::clans::ClanState,
    /// The sessions whose link went silent, kept for their clients to
    /// connect again (`away`).
    pub away: HashMap<u32, super::away::Away>,
    /// The sessions the lobby has sent into a match. Such a session's game
    /// has loaded a match and can show a results screen; any other would
    /// die of one (`show_results`). A lobby started anew knows of none, so
    /// a game that outlived it is shown no screen until its next match.
    pub sent_to_match: HashSet<u32>,
    /// The places held for the players whose JOIN was granted, by account:
    /// their Play puts them into that match (`join`).
    pub holds: HashMap<u32, super::join::Hold>,
    /// The listing keys of the matches, which JOIN names a match by
    /// (`join`).
    pub listing: super::join::Listing,
    /// The choices of a match against bots held for the players' Play, by
    /// account, kept a while after they ran out (`pvb`).
    pub pvb_holds: HashMap<u32, super::pvb::Hold>,
    /// The match against bots being made, by the queue's match id: what
    /// it was chosen with (`pvb`, `play::launch`).
    pub pvb_launching: HashMap<u32, super::pvb::Choice>,
    /// "Watch replay" (`replay`): the process's replays, if srp gave them.
    pub replays: Option<&'a Replays>,
}

impl<'a> Core<'a> {
    pub fn new(
        store: &'a SessionStore,
        accounts: &'a GameAccounts,
        results: &'a Results,
        host: &'a dyn MatchHost,
        inbox: &'a Inbox,
        settings: Settings,
        now: Instant,
    ) -> Self {
        // An important message's id must be new to the client that gets it
        // (it ignores one it still holds, `social.md` §1.4), also after a
        // restart of srp: the ids start at the time.
        let next_message_id = (quests::now() as u32 & 0x0FFF_FFFF) << 3 | 1;
        // The squads are the store's, and so is their size (`SRP_SQUAD_SIZE`).
        store.squads().set_size(settings.squad_size);
        Self {
            store,
            accounts,
            results,
            host,
            inbox,
            next_tick: now + settings.tick,
            settings,
            packet_log: None,
            links: HashMap::new(),
            by_account: HashMap::new(),
            requests: BTreeMap::new(),
            next_message_id,
            next_order_id: 1,
            matches: HashMap::new(),
            rng: Rng::from_time(),
            shop: *premium::settings(),
            premium_offers: HashMap::new(),
            ranked: (usize::MAX, 0),
            squads_dirty: false,
            clan: Default::default(),
            away: HashMap::new(),
            sent_to_match: HashSet::new(),
            holds: HashMap::new(),
            // A key is not one a match of the lobby before a restart had,
            // which a window still open may name: the keys start at the
            // time.
            listing: super::join::Listing::new(quests::now() as u32),
            pvb_holds: HashMap::new(),
            pvb_launching: HashMap::new(),
            replays: None,
        }
    }

    // ------------------------------------------------------------ lookups

    pub fn player(&self, addr: SocketAddr) -> Option<&Player<'a>> {
        match &self.links.get(&addr)?.stage {
            Stage::Player(player) => Some(player),
            _ => None,
        }
    }

    pub fn player_mut(&mut self, addr: SocketAddr) -> Option<&mut Player<'a>> {
        match &mut self.links.get_mut(&addr)?.stage {
            Stage::Player(player) => Some(player),
            _ => None,
        }
    }

    pub fn who(&self, addr: SocketAddr) -> Option<Who> {
        let player = self.player(addr)?;
        Some(Who {
            addr,
            session_id: player.session_id,
            account_id: player.account_id,
            name: player.name.clone(),
            account: player.account.clone(),
        })
    }

    /// The player of an account in the lobby.
    pub fn who_is(&self, account_id: u32) -> Option<Who> {
        self.who(*self.by_account.get(&account_id)?)
    }

    /// The addresses of the signed-in players.
    pub fn players(&self) -> Vec<SocketAddr> {
        self.links
            .iter()
            .filter(|(_, link)| matches!(link.stage, Stage::Player(_)))
            .map(|(addr, _)| *addr)
            .collect()
    }

    /// The name the lobby shows for an account, in the lobby or not.
    pub fn name_of(&self, account_id: u32) -> Option<String> {
        if let Some(account) = self.accounts.get(account_id) {
            return Some(account.locked().name.clone());
        }
        Some(self.store.account(account_id)?.locked().name.clone())
    }

    /// The account's clan tag (none: srp has no clans yet).
    pub fn clan_tag_of(&self, account_id: u32) -> String {
        self.accounts
            .get(account_id)
            .map(|account| account.locked().clan_tag.clone())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------ sending

    /// Queue a message for a link.
    pub fn send(&mut self, to: SocketAddr, message: ServerMessage) -> Option<u64> {
        self.links.get_mut(&to)?.push(message)
    }

    pub fn send_all(&mut self, to: SocketAddr, messages: impl IntoIterator<Item = ServerMessage>) {
        for message in messages {
            self.send(to, message);
        }
    }

    /// Queue a message a writer made; a writer's refusal is a bug of the
    /// caller's and is logged.
    pub fn send_made(&mut self, to: SocketAddr, message: Result<ServerMessage, WireError>) {
        match message {
            Ok(message) => {
                self.send(to, message);
            }
            Err(error) => log::error!("A lobby message could not be made: {error}"),
        }
    }

    /// `84 <op> 00 <key>`: refused, with the game's own text for it.
    pub fn deny(&mut self, to: SocketAddr, op: u8, key: &str) {
        self.send(to, lobby::denied(op, key));
    }

    /// What an invite into a full squad says (`SRP_SQUAD_FULL_TEXT`, or a
    /// sentence from the squads' size).
    pub fn squad_full_text(&self) -> String {
        self.settings
            .squad_full_text
            .clone()
            .unwrap_or_else(|| super::squad_full_text_for(self.store.squads().size()))
    }

    /// The inventory, the account and the money, in the order the client
    /// needs: it never asks for them (`lobby_requests.md` §3). The account
    /// is marked for the saver.
    pub fn push_account(&mut self, who: &Who) {
        let (push, balance) = {
            let account = who.account.locked();
            (
                wire::after_change(&account, quests::now()),
                (account.generic_money, account.premium_money),
            )
        };
        self.accounts.mark_dirty(who.account_id);
        match push {
            Ok(push) => {
                self.send_all(who.addr, push);
                if let Some(player) = self.player_mut(who.addr) {
                    player.money_sent = balance;
                }
            }
            Err(error) => log::error!("{}'s account could not be sent: {error}", who.name),
        }
    }

    // ----------------------------------------------------------- datagrams

    /// One datagram from `from`.
    pub fn datagram(&mut self, from: SocketAddr, bytes: &[u8], now: Instant) {
        let events = match self.links.get_mut(&from) {
            Some(link) => link.transport.receive(bytes, now),
            None => {
                let Some(events) = self.first_datagram(from, bytes, now) else {
                    return;
                };
                events
            }
        };
        for event in events {
            match event {
                Event::Message { id, payload } => {
                    if self.player(from).is_none() {
                        continue;
                    }
                    match requests::parse(id, &payload) {
                        Ok(message) => self.handle(from, message, now),
                        Err(ParseError::Trailing { message, extra }) => {
                            log::warn!("Lobby message {id:#04X} has {extra} bytes too many");
                            self.handle(from, *message, now);
                        }
                        Err(error) => {
                            // The bytes too (at most 64), to see what a
                            // client sent that the reader does not take.
                            let shown = &payload[..payload.len().min(64)];
                            let hex: String = shown.iter().map(|b| format!("{b:02x}")).collect();
                            log::warn!(
                                "A lobby message was not read: {error} ({} bytes: {hex})",
                                payload.len()
                            );
                            self.unreadable(from, id);
                        }
                    }
                }
                Event::Disconnected => {
                    // The transport still owes the client its confirm: the
                    // link goes on the next pass.
                    self.retire(from, "its client left", true);
                    break;
                }
            }
        }
    }

    /// A datagram from an address with no link: a link is made only for a
    /// sign-in. Returns what else the datagram carried.
    fn first_datagram(
        &mut self,
        from: SocketAddr,
        bytes: &[u8],
        now: Instant,
    ) -> Option<Vec<Event>> {
        if self.links.len() >= MAX_LINKS {
            return None;
        }
        let mut transport = Transport::new(Table::Lobby, now);
        let mut events = transport.receive(bytes, now).into_iter();
        let Some(Event::Message { id, payload }) = events.next() else {
            return None;
        };
        let Ok(ClientMessage::SignIn {
            session_id,
            need_shop_items,
        }) = requests::parse(id, &payload)
        else {
            return None;
        };
        self.sign_in(from, transport, session_id, need_shop_items, now);
        Some(events.collect())
    }

    /// A request that could not be read. The client waits for the answer
    /// of an inventory, shop or skills request before it shows the result,
    /// so those are refused with the game's text for the kind.
    fn unreadable(&mut self, from: SocketAddr, id: u8) {
        let key = match id {
            requests::id::INVENTORY => "st_cannot_relocate_item",
            requests::id::SHOP => "st_cannot_buy_item",
            requests::id::SKILLS => "st_player_skills_check_failed",
            _ => return,
        };
        self.deny(from, id, key);
    }

    // -------------------------------------------------------------- sign-in

    /// A sign-in refused before anything of it was taken: `FD`, and the
    /// link closes. The client counts it a failed try (transport.md §6.1).
    fn refuse_sign_in(&mut self, from: SocketAddr, mut transport: Transport, now: Instant) {
        let closing = self
            .links
            .values()
            .filter(|link| !matches!(link.stage, Stage::Player(_)))
            .count();
        if closing < MAX_CLOSING {
            transport.kick(now);
            self.links.insert(
                from,
                Link {
                    transport,
                    stage: Stage::Closing,
                    held: None,
                    #[cfg(test)]
                    pushed: Vec::new(),
                },
            );
        }
    }

    fn sign_in(
        &mut self,
        from: SocketAddr,
        mut transport: Transport,
        session_id: u32,
        need_shop_items: bool,
        now: Instant,
    ) {
        // A session from before a restart, of a game older than the server
        // now takes: it must not get the pushes, which name items its game
        // may lack. It ends, so the game's tries fail and it goes to its
        // login screen, where its sign-in is refused (`patch_check`).
        if let Some((name, revision)) = self.outdated_session(session_id) {
            log::info!(
                "{name}: a lobby sign-in from {from} of a game of patch revision {revision}, the \
                 server takes {} and later; refused, and its session ended",
                self.settings.min_patch_revision
            );
            self.store.end_session(session_id);
            self.refuse_sign_in(from, transport, now);
            return;
        }
        let held = self.store.hold(session_id);
        let account = held
            .as_ref()
            .and_then(|_| self.store.account_for_session(session_id));
        let (Some(hold), Some(account)) = (held, account) else {
            log::warn!("A lobby sign-in from {from} with an unknown or ended session; refused");
            self.refuse_sign_in(from, transport, now);
            return;
        };
        let (account_id, login_name) = {
            let account = account.locked();
            (account.id, account.name.clone())
        };

        // The account's other link: another game signed in with it (the
        // old one is told so, `AA`), or this game connected again.
        let mut still_in_match = None;
        if let Some(old) = self.by_account.get(&account_id).copied() {
            let same_session = self
                .player(old)
                .is_some_and(|player| player.session_id == session_id);
            if same_session {
                still_in_match = self.player(old).and_then(|player| player.in_match);
                log::info!("{login_name} connected to the lobby again from {from}");
                self.retire(old, "its client connected again", false);
                if let Some(link) = self.links.get_mut(&old) {
                    link.transport.kick(now);
                }
            } else {
                self.sign_out(old, "its account signed in again", now);
            }
        }

        // A client that connects again is in the match it was in, if that
        // still runs: its link went silent (`away`), or it gave up on a
        // link that still stood.
        let in_match = self
            .back_from_away(session_id, account_id)
            .or(still_in_match)
            .filter(|id| self.matches.contains_key(id));

        let game = self.accounts.get_or_create(account_id, &login_name);
        let unix = quests::now();
        // A test bot (`SRP_BOT_ACCOUNTS`) starts no run of bonus days.
        let bot = self.accounts.is_bot(account_id);
        if bot {
            log::info!("{login_name} is a test bot (SRP_BOT_ACCOUNTS)");
        }
        let (name, notices) = {
            let mut game = game.locked();
            game.settle_active_kit(unix);
            // The account is seen today: its run of days for the daily
            // bonus is brought up to the day before the pushes.
            if !bot {
                bonus::visit(&mut game, unix, &mut self.rng);
            }
            let notices = quests::refresh(&mut game, unix, &mut self.rng);
            (game.name.clone(), notices)
        };
        self.accounts.mark_dirty(account_id);
        // With no account locked: the ranking locks each in turn.
        let ranking = self.accounts.ranking();
        let place = ranking
            .iter()
            .position(|row| row.0 == account_id)
            .map_or(0, |index| index as u32 + 1);
        let online = self.online_counts();
        let (push, balance) = {
            let game = game.locked();
            let info = SignInInfo {
                now: unix,
                need_shop_items,
                ranked_players: ranking.len() as u32,
                place,
                players_online: online.0,
                games_online: online.1,
                minutes_to_maintenance: online.2,
            };
            (
                wire::sign_in_push(&game, &info),
                (game.generic_money, game.premium_money),
            )
        };
        let mut push = match push {
            Ok(push) => push.into_iter(),
            Err(error) => {
                // Not an account a client can hold: it must not be half
                // signed in.
                log::error!("{name}'s sign-in could not be answered: {error}; refused");
                self.refuse_sign_in(from, transport, now);
                return;
            }
        };

        if let Some(log) = &self.packet_log {
            match log.connection(Table::Lobby, from) {
                // The sign-in's body is the session id: all a connection
                // shows to prove whose it is. A `9E` may carry a replay's
                // download token (`replay`): never logged whole.
                Ok(file) => transport.set_log(file.with_redaction(Box::new(|direction, id, _| {
                    (direction == Direction::ToServer && id == requests::id::SIGN_IN)
                        || (direction == Direction::ToClient
                            && id == lobby::ServerId::VideoLink as u8)
                }))),
                Err(error) => log::warn!("No packet log for {from}: {error}"),
            }
        }
        let player = Player {
            session_id,
            account_id,
            name: name.clone(),
            account: game,
            _hold: hold,
            reached: reached_address(from),
            channel_ids: [0; 10],
            language: None,
            queued_since: None,
            queue_line: None,
            queue_announced: false,
            order_id: 0,
            in_match,
            connect: None,
            squad_sent: None,
            squad_serial: None,
            money_sent: balance,
            ranked_sent: ranking.len() as u32,
            online_sent: online,
            online_at: None,
            friends_sent: Vec::new(),
            chat_allowance: super::social::CHAT_BURST,
            chat_at: now,
            not_found: None,
        };
        let mut link = Link {
            transport,
            stage: Stage::Player(Box::new(player)),
            held: None,
            #[cfg(test)]
            pushed: Vec::new(),
        };
        // The 0x80 first and alone; the rest once it is acked.
        if let Some(signed_in) = push.next() {
            #[cfg(test)]
            link.pushed.push(signed_in.clone());
            let serial = link.write(&signed_in);
            link.held = serial.map(|serial| (serial, Vec::new()));
        }
        self.links.insert(from, link);
        self.by_account.insert(account_id, from);
        self.send_all(from, push);
        // The ranking page of a game that was running before this sign-in
        // may still wait for rows that will never come (`ranking_flush`).
        let top: Vec<lobby::Ranked> = ranking
            .iter()
            .take(lobby::RANKING_PAGE_ROWS)
            .map(|row| lobby::Ranked {
                name: row.1.clone(),
                rating: row.2,
                rating_before: row.3,
            })
            .collect();
        if let Some(flush) = lobby::ranking_flush(&top) {
            self.send_made(from, flush);
        }
        log::info!("{name} (account {account_id}) signed in to the lobby from {from}");

        // What needs other accounts: the friend and ignore lists, which the
        // client never asks for, friend requests that waited, the squad.
        if let Some(who) = self.who(from) {
            self.push_friends(&who);
            self.push_ignored(&who);
            self.waiting_requests(&who);
            self.notify(&who, &notices);
            // Its clan, and the clan invites that waited.
            self.clan_sign_in(&who);
            // The daily bonus and a premium discount (`shop`).
            self.sign_in_extras(&who);
        }
        // A match that ended while the account was away: its screen. The
        // sign-in's pushes carried the account as the match left it.
        self.show_results(from, true);
        // The operator's text that stands (`Inbox::set_admin_text`).
        if let Some((first, second)) = self.inbox.admin_text() {
            self.send(from, lobby::admin_text(&first, &second));
        }
        self.squads_dirty = true;
    }

    /// The players and matches online, as the lobby's status bar shows
    /// them, and the minutes to maintenance (a day when none is set).
    pub fn online_counts(&self) -> (u32, u32, u32) {
        (
            self.store.players_online() as u32,
            self.matches.len() as u32,
            self.store
                .minutes_to_maintenance()
                .unwrap_or(MINUTES_TO_MAINTENANCE),
        )
    }

    // ------------------------------------------------------------- the end

    /// The player of a link is no more: out of the queue, not ready in its
    /// squad, and (unless its game goes on with another link) its session
    /// ended. The link itself stays for the transport to finish.
    pub fn retire(&mut self, addr: SocketAddr, why: &str, end_session: bool) {
        let Some(link) = self.links.get_mut(&addr) else {
            return;
        };
        let Stage::Player(player) = std::mem::replace(&mut link.stage, Stage::Closing) else {
            return;
        };
        link.held = None;
        if self.by_account.get(&player.account_id) == Some(&addr) {
            self.by_account.remove(&player.account_id);
        }
        // A place held for its JOIN is free again.
        self.holds.remove(&player.account_id);
        self.leave_queue_said(player.session_id, &player.name, player.queue_announced);
        self.store.squads().set_ready(player.account_id, false);
        // Nor in its clan team, which it leaves with its session.
        self.clan_link_gone(player.account_id, end_session);
        if let Some(account) = self.store.account(player.account_id) {
            account.locked().play = None;
        }
        if end_session {
            // It leaves its squad with it; a seat in a match is kept by the
            // store until the match is over.
            self.store.end_session(player.session_id);
        }
        self.squads_dirty = true;
        log::info!(
            "{} (account {}) left the lobby: {why}",
            player.name,
            player.account_id
        );
    }

    /// Sign a player out from this side: `AA` (the client goes to its login
    /// screen with `st_login_menu_status_sign_in_already_online`), then the
    /// disconnect.
    pub fn sign_out(&mut self, addr: SocketAddr, why: &str, now: Instant) {
        self.retire(addr, why, true);
        if let Some(link) = self.links.get_mut(&addr) {
            let serial = link.push(lobby::signed_out());
            link.stage = Stage::Leaving {
                serial,
                until: now + SIGN_OUT_WAIT,
            };
        }
    }

    /// The lobby stops. That is not the players' doing: their sessions
    /// stay, and a client that connects again to a lobby started anew is
    /// taken with the session it has (the store keeps sessions across a
    /// restart of srp).
    pub fn shutdown(&mut self) {
        for addr in self.players() {
            self.retire(addr, "the lobby stopped", false);
        }
        self.links.clear();
    }

    // ------------------------------------------------------------ the loop

    /// What is due at `now`; returns the datagrams to send.
    pub fn step(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        for notice in self.inbox.take() {
            self.notice(notice, now);
        }
        if now >= self.next_tick {
            self.next_tick = now + self.settings.tick;
            self.tick(now);
        }
        if self.squads_dirty {
            self.refresh_squads();
        }

        let (timeout, match_timeout) =
            (self.settings.link_timeout, self.settings.match_link_timeout);
        let mut datagrams = Vec::new();
        let mut gone = Vec::new();
        let mut silent = Vec::new();
        for (addr, link) in &mut self.links {
            if let Some((serial, _)) = &link.held
                && link.transport.is_delivered(*serial)
            {
                let (_, held) = link.held.take().expect("just seen");
                for message in &held {
                    link.write(message);
                }
            }
            let mut connect = None;
            let mut close = false;
            match &mut link.stage {
                Stage::Player(player) => {
                    // The ordered channel delivers in order: once the state
                    // sent before the 0x82 is acked, so is every earlier one.
                    let ready = link.held.is_none()
                        && player.connect.as_ref().is_some_and(|(after, _)| {
                            after.is_none_or(|serial| link.transport.is_delivered(serial))
                        });
                    if ready {
                        connect = player.connect.take().map(|(_, message)| message);
                    }
                }
                Stage::Leaving { serial, until } => {
                    let delivered = serial.is_none_or(|serial| link.transport.is_delivered(serial));
                    close = delivered || now >= *until;
                }
                Stage::Closing => {}
            }
            if let Some(connect) = connect {
                #[cfg(test)]
                link.pushed.push(connect.clone());
                link.write(&connect);
            }
            if close {
                link.transport.disconnect(now);
                link.stage = Stage::Closing;
            }
            datagrams.extend(
                link.transport
                    .poll(now)
                    .into_iter()
                    .map(|datagram| (*addr, datagram)),
            );
            if link.transport.state() == State::Closed {
                gone.push((*addr, "its link closed", true));
            } else {
                // A player in a match that runs has longer: its client may
                // be loading its level (`away`).
                let in_match = matches!(
                    &link.stage,
                    Stage::Player(player)
                        if player.in_match.is_some_and(|id| self.matches.contains_key(&id))
                );
                let timeout = if in_match { match_timeout } else { timeout };
                if link.transport.is_gone_after(now, timeout) {
                    silent.push(*addr);
                }
            }
        }
        for addr in silent {
            self.link_went_silent(addr, now);
        }
        for (addr, why, end_session) in gone {
            self.retire(addr, why, end_session);
            self.links.remove(&addr);
        }
        datagrams
    }

    /// The pass made every [`Settings::tick`].
    fn tick(&mut self, now: Instant) {
        // Sessions ended elsewhere (a ban, the admin's kick).
        let ended: Vec<SocketAddr> = self
            .links
            .iter()
            .filter_map(|(addr, link)| match &link.stage {
                Stage::Player(player) if self.store.has_ended(player.session_id) => Some(*addr),
                _ => None,
            })
            .collect();
        for addr in ended {
            self.sign_out(addr, "its session ended", now);
        }

        self.check_matches(now);
        self.holds.retain(|_, hold| hold.until > now);
        self.log_predictions(now);
        self.collect_away(now);
        self.clan_matches(now);
        let online = self.online_counts();
        let accounts = self.accounts.len();
        if accounts != self.ranked.0 {
            self.ranked = (accounts, self.accounts.ranking().len() as u32);
        }
        let ranked = self.ranked.1;
        for addr in self.players() {
            self.queue_status(addr, now);
            let Some(player) = self.player(addr) else {
                continue;
            };
            let balance = {
                let account = player.account.locked();
                (account.generic_money, account.premium_money)
            };
            // Somebody registered: the ranking page asks for rows by the
            // count it has, so it is told the new one before it is opened
            // (`8F` stores it, the `9D` after it hands it to the page;
            // with `show` off a `9D` does nothing else, 0x271340). Not in
            // a match: the client has its lobby again when it is back.
            if player.in_match.is_none() && player.ranked_sent != ranked {
                let count = lobby::base_info(ranked, &player.name);
                let recount = tutorial::message(&player.account.locked(), false);
                self.send(addr, count);
                self.send(addr, recount);
                if let Some(player) = self.player_mut(addr) {
                    player.ranked_sent = ranked;
                }
            }
            let Some(player) = self.player(addr) else {
                continue;
            };
            let (money_sent, online_sent) = (player.money_sent, player.online_sent);
            // 0x9F is the one message sent once and never again
            // (transport.md §4.1), and until the client has one its Play
            // is off (it takes the minutes to maintenance as 0,
            // `social.md` §4): it is repeated.
            let stats_due = player
                .online_at
                .is_none_or(|at| now.saturating_duration_since(at) >= ONLINE_STATS_EVERY);
            // The web page, or a match's rewards, changed the balance.
            if balance != money_sent {
                self.send(addr, lobby::money(balance.0, balance.1));
            }
            if online != online_sent || stats_due {
                self.send(addr, lobby::online_stats(online.0, online.1, online.2));
            }
            if let Some(player) = self.player_mut(addr) {
                player.money_sent = balance;
                if online != online_sent || stats_due {
                    player.online_sent = online;
                    player.online_at = Some(now);
                }
            }
            self.friend_statuses(addr);
            // A repair that is over (`items`).
            if let Some(who) = self.who(addr) {
                self.item_timers(&who);
            }
            // A match that ended after its player had left it.
            let in_lobby = self
                .player(addr)
                .is_some_and(|player| player.in_match.is_none() && player.queued_since.is_none());
            if in_lobby {
                self.show_results(addr, false);
            }
        }
        self.squads_dirty = true;
    }

    fn notice(&mut self, notice: Notice, now: Instant) {
        match notice {
            Notice::MatchFinished(finished) => self.match_finished(finished.match_id, now),
            Notice::Push {
                account_id,
                messages,
            } => {
                if let Some(addr) = self.by_account.get(&account_id).copied() {
                    self.send_all(addr, messages);
                }
            }
            Notice::Broadcast { messages } => {
                for addr in self.players() {
                    self.send_all(addr, messages.clone());
                }
            }
            Notice::AccountChanged { account_id } => {
                let Some(who) = self.who_is(account_id) else {
                    return;
                };
                self.push_account(&who);
                let (skills, reputations, quests) = {
                    let account = who.account.locked();
                    (
                        wire::level_and_skills(&account),
                        wire::reputations(&account),
                        wire::quests(&account, quests::now()),
                    )
                };
                self.send_made(who.addr, skills);
                self.send_made(who.addr, reputations);
                self.send(who.addr, quests);
            }
        }
    }
}

/// The local address datagrams to `peer` leave from: the one a client on
/// this machine or its network reached srp at. (A UDP socket bound to every
/// interface does not say which one a datagram came in on.)
fn reached_address(peer: SocketAddr) -> Option<IpAddr> {
    let probe = UdpSocket::bind(match peer {
        SocketAddr::V4(_) => "0.0.0.0:0",
        SocketAddr::V6(_) => "[::]:0",
    })
    .ok()?;
    probe.connect(peer).ok()?;
    Some(probe.local_addr().ok()?.ip())
}
