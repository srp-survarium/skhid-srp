//! One connected peer (UDP address): its transport, the account it signed in as and the per-connection state.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::time::Instant;

use accounts::AccountId;
use protocol::s2c::{Reply, S2c};
use udp_transport::{Role, Transport};

/// Where a peer is in the sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No 2E yet.
    New,
    /// 80 queued, not on the wire yet.
    SignedPending,
    /// 80 sent, waiting for the client's ack (the loading messages go out only after it).
    AwaitAck,
    /// Login pushes sent.
    Live,
    /// The 2E was refused (unknown / expired session, banned account) or never came: every datagram of the peer is
    /// answered with `FD` (the client closes the connection), nothing else is handled. See `login::refuse`.
    Refused,
}

/// A local match in progress (set by 5B 01 / the tutorial 5B 02, ended by the 53 flush).
#[derive(Debug, Clone, Copy)]
pub struct LocalMatchState {
    pub started: Instant,
    pub map: u32,
    pub mode: u32,
    /// The shooting range (`5B 03`): ends with no reward and no results.
    pub range: bool,
}

/// A reroll list the client shows (45 05 -> pick 45 06).
#[derive(Debug, Clone)]
pub struct RerollOffer {
    /// the modifier being rerolled
    pub old: (u16, u16),
    /// the three rows in wire order (= the order of the client's list)
    pub rows: Vec<(u16, u16)>,
    /// paid with gold: every row is free to pick; paid with spare parts: rows after the first are "premium"
    pub gold: bool,
}

pub struct Peer {
    pub addr: SocketAddr,
    pub transport: Transport,
    pub phase: Phase,
    pub account: Option<AccountId>,
    pub name: String,
    /// The lobby-load `5B 02` was answered: a later one starts the tutorial.
    pub handshake_5b: bool,
    pub local_match: Option<LocalMatchState>,
    /// Upgrade offers shown to the client (45 07 -> pick 45 08), by item uid.
    pub craft_offer: HashMap<u32, Vec<(u16, u16)>>,
    /// Reroll offers (45 05 -> pick 45 06): old pair and the three rows.
    pub reroll_offer: HashMap<u32, RerollOffer>,
    /// Unix second the daily quest set ends at (0 = not loaded yet); see `quests::tick`.
    pub quest_expires: i64,
    /// `57`: the game-server mask the client selected (bit = 9E server id); `u32::MAX` until it says (the client's own
    /// default when the 9E list is non-empty and nothing was saved).
    pub server_mask: u32,
    /// `56`: the last ping results (server id, ms) the client measured against the 9E list.
    pub server_pings: Vec<(u8, u16)>,
    /// Rejoin: the loading messages went out to an account that sits in a running online match; its first request
    /// afterwards checks for a kept seat and sends the `82` (online::offer_rejoin_after_login).
    pub rejoin_check: bool,
    /// The peer's first datagram (the sign-in deadline counts from it).
    pub since: Instant,
}

impl Peer {
    pub fn new(addr: SocketAddr, now: Instant) -> Peer {
        Peer {
            addr,
            since: now,
            transport: Transport::new(Role::Server, now),
            phase: Phase::New,
            account: None,
            name: String::new(),
            handshake_5b: false,
            local_match: None,
            craft_offer: HashMap::new(),
            reroll_offer: HashMap::new(),
            quest_expires: 0,
            server_mask: u32::MAX,
            server_pings: Vec::new(),
            rejoin_check: false,
        }
    }

    /// Queue a message; the transport takes the `x` counter of the id's class.
    pub fn push(&mut self, m: &S2c) {
        match m.encode_payload() {
            Ok(payload) => {
                if let Err(e) = self.transport.send(m.id(), &payload) {
                    log::warn!("{}: send {:#04x} failed: {e:?}", self.addr, m.id());
                }
            }
            Err(e) => log::error!("{}: encode {:#04x} failed: {e}", self.addr, m.id()),
        }
    }

    pub fn reply(&mut self, r: Reply) {
        self.push(&S2c::Reply(r));
    }

    /// `83 x <request> <data>` with raw data after the request id.
    pub fn reply_raw(&mut self, request: u8, data: &[u8]) {
        self.reply(Reply::Raw { request, data: data.to_vec() });
    }
}

/// An effect on another account's peer (delivered by the server after the handler returned).
#[derive(Debug, Clone)]
pub struct Cross {
    pub to: AccountId,
    pub msg: S2c,
}

/// Which peer an account is on. The social relay of the next wave finds other players through it.
#[derive(Default)]
pub struct Registry {
    by_account: BTreeMap<AccountId, SocketAddr>,
    /// Squads (squads.rs).
    pub squads: crate::squads::Squads,
    /// B0 presence byte of online accounts (absent = 0, in the lobby); see `social::ST_*`.
    presence: BTreeMap<AccountId, u8>,
    /// Custom match rooms (rooms.rs).
    pub rooms: crate::rooms::Rooms,
    /// `account -> (match context id, team)` of players inside an online match (chat channels 5..7).
    match_ctx: BTreeMap<AccountId, (u32, u8)>,
    /// The PvP queue (queue.rs).
    pub queue: crate::queue::PvpQueue,
    /// The PvE queue of 3 (pve.rs).
    pub pve: crate::pve::PveQueue,
    /// Online matches in progress (online.rs).
    pub online: crate::online::OnlineBook,
    /// The login session each online account signed in with (C9 of an online match carries it).
    sessions: BTreeMap<AccountId, u32>,
    /// The client language index of each account (`32` after the 80 and `5C 00` carry it, 0x2000915E0: 1 russian ..
    /// 10 turkish); kept after a disconnect (an online match ends while the player is away).
    languages: BTreeMap<AccountId, u8>,
}

impl Registry {
    pub fn set_language(&mut self, account: AccountId, language: u8) {
        self.languages.insert(account, language);
    }

    /// The client language of an account; 0 (other: English texts) when it never said.
    pub fn language(&self, account: AccountId) -> u8 {
        self.languages.get(&account).copied().unwrap_or(0)
    }

    /// Returns the address the account was on before (a new sign-in replaces the old connection).
    pub fn insert(&mut self, account: AccountId, addr: SocketAddr) -> Option<SocketAddr> {
        self.by_account.insert(account, addr).filter(|old| *old != addr)
    }

    pub fn remove_addr(&mut self, addr: SocketAddr) {
        self.by_account.retain(|_, a| *a != addr);
        let on = &self.by_account;
        self.presence.retain(|a, _| on.contains_key(a));
        self.match_ctx.retain(|a, _| on.contains_key(a));
        self.sessions.retain(|a, _| on.contains_key(a));
    }

    pub fn set_session(&mut self, account: AccountId, session: u32) {
        self.sessions.insert(account, session);
    }

    pub fn session_of(&self, account: AccountId) -> Option<u32> {
        self.sessions.get(&account).copied()
    }

    /// The sessions of every connected account.
    pub fn live_sessions(&self) -> Vec<u32> {
        self.sessions.values().copied().collect()
    }

    /// The player joined an online match: chat channels 5 (all) and 6 / 7 (own team) reach the same context.
    pub fn set_match_context(&mut self, account: AccountId, context: u32, team: u8) {
        self.match_ctx.insert(account, (context, team));
    }

    pub fn clear_match_context(&mut self, account: AccountId) {
        self.match_ctx.remove(&account);
    }

    pub fn match_context(&self, account: AccountId) -> Option<(u32, u8)> {
        self.match_ctx.get(&account).copied()
    }

    /// Everybody in a match context with their team.
    pub fn match_members(&self, context: u32) -> Vec<(AccountId, u8)> {
        self.match_ctx.iter().filter(|(_, (c, _))| *c == context).map(|(a, (_, t))| (*a, *t)).collect()
    }

    pub fn set_status(&mut self, account: AccountId, status: u8) {
        self.presence.insert(account, status);
    }

    /// Presence byte of an online account.
    pub fn status(&self, account: AccountId) -> u8 {
        self.presence.get(&account).copied().unwrap_or(0)
    }

    /// Account a peer address is signed in as.
    pub fn account_at(&self, addr: SocketAddr) -> Option<AccountId> {
        self.by_account.iter().find(|(_, a)| **a == addr).map(|(id, _)| *id)
    }

    pub fn addr_of(&self, account: AccountId) -> Option<SocketAddr> {
        self.by_account.get(&account).copied()
    }

    pub fn is_online(&self, account: AccountId) -> bool {
        self.by_account.contains_key(&account)
    }

    pub fn online(&self) -> impl Iterator<Item = (AccountId, SocketAddr)> + '_ {
        self.by_account.iter().map(|(a, s)| (*a, *s))
    }

    pub fn len(&self) -> usize {
        self.by_account.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_account.is_empty()
    }
}
