//! Custom matches (rooms), lobby side: `62 18..24`, S->C 98 room state, A0 ended, A1 aborted, and the CLAN BATTLE (custom
//! match type 1). Layouts: `protocol::s2c::rooms`; the notes and the evidence labels of every rule are in
//! `docs/notes/clan-battles.md` and `docs/notes/social.md` (custom rooms).
//!
//! Real state, real rules: rooms live in the [`Rooms`] table (memory: a room has no meaning after a restart), the host
//! controls the settings, map / mode / weather are rows of the game data (`matches::modes`), team sizes are bounded by
//! the row. The room's `weather` byte (98, `62 1E`) is the row's VARIATION id: the client's own list
//! (`singleplayer_match_maps.options` `custom_match_data`, volume 75) labels each "weather" entry `st_variation_N_name`
//! with id N (PROVEN). The `region` byte is the id of an entry of the game-server list
//! (`9E`): the window's region drop-down is filled from that list (CustomMatchWindow.setServersList, PROVEN).
//! Team numbers are the client's own roles (0 team A, 1 team B, 2 unassigned = reserve; PROVEN, the 98 `team` byte is the
//! Flash `role`, and `62 1D` carries the role itself: the exe sends the button value - 1).
//!
//! * **Leaving** (`62 1C` own id; the mini panel's X and the window's back button): the leaver gets `A0` (the client
//!   resets its room record and hides the window and the panel, PROVEN); a host who leaves closes the room and the others
//!   get `A1` `st_custom_match_aborted_by_commander`; a member the host removes gets `A1`
//!   `st_custom_match_kicked_from_match` (the game's own texts, sent by the server only: the exe never names them).
//! * **Ready** (`62 22 u8`, PROVEN the per-player toggle): stored per member, shown in the 98. The client's Play button
//!   only sends Start when every non-host member of team A or B is ready (unassigned members are not checked, PROVEN
//!   0x2001E8161) and the room's tokens are complete; the server checks the same.
//! * **Reserve**: unassigned members do not block the Start; they stay out of the match and get `A1`
//!   `st_custom_match_match_started_without_you` ("The match started when you were in reserve", a server-only text).
//! * **Bots** (training rooms only: the client hides the bot settings of a clan room and when `max_ai_count` is 0,
//!   PROVEN CustomMatchWindow.setData): offered when the match server's world host drives bots, at most
//!   [`crate::online::bot_team_cap`] per team (the `bots_per_team` setting) and never more than the team's free
//!   match slots; the Start passes them as the room's AI players (the queue's bot fill path, `online.rs`). The room's
//!   "bots ignore players" goes with them ([`BotFill::ignore_players`]): the world host sets the exe's own harmless-bots byte 0x20F43FE53, which the AI's
//!   vision and sound filters read (PROVEN 0x20032CA70 / 0x200368B50: they skip every record with `ai_id == -1`).
//! * **Clan battle**: the host needs a clan and the clan rank permission "battle commander" (LIKELY); only members of
//!   the host's clan and of ONE other clan may join (the game's own refusal texts); team A is the host's clan side, team
//!   B the other clan's (the client shows the team buttons by `clan_id`, PROVEN); no bots; Start needs two different
//!   clans on their own sides and everybody ready. A refused Start with unready members sends them the ready check
//!   `83 30` (CHOICE as the trigger; the effect is PROVEN: the client answers `62 22` ready).
//! * **Start** (`62 23`) starts an online match on the room's row through `online.rs` when online matches are on and
//!   the match server serves the row's project; otherwise it is refused with a client-shown text. Every refusal is
//!   logged. NOT done: `62 19` (join request, no caller found).

use std::collections::{BTreeMap, BTreeSet};

use accounts::{account_of_public_id, public_id, AccountId};
use protocol::c2s::Request;
use protocol::s2c::*;
use protocol::Encode;

use crate::dispatch::Ctx;
use crate::online::{BotFill, MatchKind};

/// Player (and bot) slots per team of a room on `row`: half the match size, 1..=9 (the 98 count fields take ONE digit,
/// so 10 would break the client's "min-max" character range).
fn team_cap(row: Option<&game_data::matches::ModeRow>) -> u8 {
    (row.map(crate::online::match_size).unwrap_or(20) / 2).clamp(1, 9) as u8
}
use crate::peer::{Cross, Peer};

/// The client's member roles in the 98 (PROVEN): 0 team A, 1 team B, 2 unassigned.
pub const TEAM_A: u8 = 0;
pub const TEAM_B: u8 = 1;
pub const TEAM_NONE: u8 = 2;
/// Most members of a room (the client's custom match member vector; the match limit of the game data is 20).
pub const MAX_MEMBERS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomMemberState {
    pub account: AccountId,
    pub team: u8,
    /// `62 22`: ready for the battle (the host counts as ready)
    pub ready: bool,
    /// `62 21`: the tokens this member pays for, `[silver, gold]`
    pub tokens: [u8; 2],
}

/// Price of one custom-match token in silver: 5 000 (FAN [0.6x], Steam thread 2021-04 with the moderator: "5,000 silver
/// per player per match, premium players free").
pub const TOKEN_COST_SILVER: u32 = 5000;
/// Price of one token in gold: the silver price at the exchange rate of 250 (CHOICE: no source names it).
pub const TOKEN_COST_GOLD: u32 = 20;

/// Tokens a room needs: one per team member without running premium (OFFICIAL 0.65b: premium players play free). The
/// reserve (unassigned) does not play, so it needs none (CHOICE). Any member may pay for the room's tokens.
fn tokens_required(cx: &Ctx, room: &Room, now: i64) -> u32 {
    room.members
        .iter()
        .filter(|m| m.team != TEAM_NONE && cx.lobby.store.load_profile(m.account).map_or(true, |p| p.premium_left(now) == 0))
        .count() as u32
}

/// Tokens the members pay for.
fn tokens_allocated(room: &Room) -> u32 {
    room.members.iter().map(|m| u32::from(m.tokens[0]) + u32::from(m.tokens[1])).sum()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub id: u32,
    pub host: AccountId,
    /// 0 training, 1 clan match (`custom_match_type`).
    pub kind: u8,
    pub map: u8,
    pub mode: u8,
    pub weather: u8,
    pub region: u8,
    pub voice: u8,
    pub ai_ignore: u8,
    pub ai: [u8; 2],
    pub players: [u8; 2],
    pub members: Vec<RoomMemberState>,
    pub invited: BTreeSet<AccountId>,
    /// Clan battle: the clan of team A and of team B (0 = none yet). Team A starts as the host's clan; the other clan
    /// is the first foreign clan that joins. Both are 0 in a training room.
    pub clans: [u32; 2],
}

impl Room {
    pub fn has(&self, a: AccountId) -> bool {
        self.members.iter().any(|m| m.account == a)
    }
    fn team_size(&self, team: u8) -> usize {
        self.members.iter().filter(|m| m.team == team).count()
    }
    pub fn team_of(&self, a: AccountId) -> Option<u8> {
        self.members.iter().find(|m| m.account == a).map(|m| m.team)
    }
    /// The game-data row of the room's map, mode and variation.
    fn row<'a>(&self, cx: &'a Ctx) -> Option<&'a game_data::matches::ModeRow> {
        cx.lobby.data.matches.modes().iter().find(|x| x.map == u32::from(self.map) && x.mode == u32::from(self.mode) && x.variation == u32::from(self.weather))
    }
}

/// Refusal texts: the game's own localization keys (the `st_custom_match_error_*` family) where it has one.
const T_NOT_IN_CLAN: &str = "st_custom_match_error_player_not_in_clan";
const T_CLAN_MISMATCH: &str = "st_custom_match_error_player_clan_mismatch";
const T_BUSY: &str = "st_custom_match_error_player_already_in_match";
const T_SQUAD: &str = "st_custom_match_error_player_in_squad";
const T_NOT_READY: &str = "st_custom_match_error_not_all_members_ready";
const T_TEAM_FULL: &str = "st_custom_match_error_team_already_full";
const T_TEAM_EMPTY: &str = "st_custom_match_error_team_empty";
const T_CANT_START: &str = "st_cant_start_custom_match_error";
const T_CANT_STATUS: &str = "st_cant_change_status_custom_match_error";
const T_CANT_KICK: &str = "st_cant_kick_player_from_custom_match";
const T_CANT_SWAP: &str = "st_cant_swap_teams_custom_match_error";
const T_CANT_PARAM: &str = "st_cant_change_parameter_custom_match_error";
const T_CANT_AI: &str = "st_cant_change_ai_count_custom_match_error";
const T_CANT_PLAYERS: &str = "st_cant_change_player_count_custom_match_error";
/// "The mode is unavailable": the room's map + mode is a row the match server does not serve.
const T_NOT_SERVED: &str = "st_locked_by_server";
/// `A1` reasons (the client shows them in a message box, then closes the room; 0x20018EE13).
const ABORTED_BY_HOST: &str = "st_custom_match_aborted_by_commander";
const KICKED: &str = "st_custom_match_kicked_from_match";
const STARTED_WITHOUT_YOU: &str = "st_custom_match_match_started_without_you";

#[derive(Default)]
pub struct Rooms {
    by_id: BTreeMap<u32, Room>,
    of: BTreeMap<AccountId, u32>,
    next_id: u32,
    next_msg: u32,
}

impl Rooms {
    pub fn room_of(&self, a: AccountId) -> Option<&Room> {
        self.of.get(&a).and_then(|id| self.by_id.get(id))
    }
    pub fn len(&self) -> usize {
        self.by_id.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
    fn remove_member(&mut self, a: AccountId) {
        if let Some(id) = self.of.remove(&a) {
            if let Some(r) = self.by_id.get_mut(&id) {
                r.members.retain(|m| m.account != a);
            }
        }
    }
}

/// `84 62` with a client localization key (the client localizes the text and shows it, 0x200176E42 ->
/// `root.show_system_message`).
fn denied(text: &str) -> S2c {
    S2c::Denied(Denied { request: 0x62, body: DeniedBody::Text(text.into()) })
}

/// Refuse a `62` request with a client-shown text, and log why.
fn refuse(peer: &mut Peer, what: &str, why: &str, text: &str) {
    log::info!("{}: 62 {what} refused: {why} ({text})", peer.addr);
    peer.push(&denied(text));
}

/// `A0`: the custom match is over for this client (it resets its room record and hides the room UI).
fn ended_msg() -> S2c {
    S2c::Raw { id: 0xA0, payload: Vec::new() }
}

/// `A1 str reason`: the room was closed for this client; the client shows the (localized) reason, then resets its room
/// record like `A0`.
fn aborted_msg(reason: &str) -> S2c {
    let mut w = protocol::Writer::new();
    w.str_max(reason, 2000);
    S2c::Raw { id: 0xA1, payload: w.finish().unwrap_or_default() }
}

fn push_to(cx: &mut Ctx, peer: &mut Peer, to: AccountId, msg: S2c) {
    if peer.account == Some(to) {
        peer.push(&msg);
    } else if cx.registry.is_online(to) {
        cx.fx.push(Cross { to, msg });
    }
}

/// AI players a team of this room may get: none in a clan room or without a match server whose world host drives bots
/// (the client then hides the bot settings: `max_ai_count` 0); else the team's slots, at most
/// [`crate::online::bot_team_cap`] (the team limit of a bot-filled match: `bots_per_team`, the hard cap, the
/// patrol graphs per team). None in a mode the AI cannot play ([`crate::online::NO_BOT_MODES`]: Search and Destroy).
fn bot_cap(cx: &Ctx, room: &Room) -> u8 {
    let bots = cx.lobby.online.as_ref().is_some_and(|h| h.bots_available());
    if room.kind != 0 || !bots || !crate::online::bots_play(u32::from(room.mode)) {
        return 0;
    }
    let row = room.row(cx);
    let (cap, _) = crate::online::bot_team_cap(cx.lobby.cfg.bots_per_team, row.map(crate::online::match_size).unwrap_or(20));
    team_cap(row).min(u8::try_from(cap).unwrap_or(u8::MAX))
}

/// The 98 for one room (None = the empty "not in a room" record; only for a client that never had a room: leaving is A0).
pub fn room_msg(cx: &Ctx, room: Option<&Room>) -> S2c {
    let Some(r) = room else {
        return S2c::Raw { id: 0x98, payload: Room98::default().to_bytes().unwrap_or_default() };
    };
    let store = &cx.lobby.store;
    // the client shows the clan ids and names only for a clan match (custom_match_type == 1, PROVEN 0x2001C8765)
    let clan_side = |id: u32| match (r.kind, id) {
        (1, id) if id != 0 => (id, store.load_clan(id).ok().flatten().map(|c| c.name).unwrap_or_default()),
        _ => (0, String::new()),
    };
    let (clan_a, clan_b) = (clan_side(r.clans[0]), clan_side(r.clans[1]));
    let now = crate::premium::unix_now();
    let members = r
        .members
        .iter()
        .map(|m| {
            let name = store.account(m.account).ok().flatten().map(|a| a.name).unwrap_or_default();
            let clan = store.clan_of(m.account).ok().flatten();
            RoomMember {
                name,
                abbr: clan.as_ref().map(|c| c.abbr.clone()).unwrap_or_default(),
                id: public_id(m.account),
                clan_id: clan.map(|c| c.id).unwrap_or(0),
                team: m.team,
                commander: m.account == r.host,
                // `62 22` state; the host never toggles it (his Play button is Start) and counts as ready
                ready: m.ready || m.account == r.host,
                premium: store.load_profile(m.account).is_ok_and(|p| p.premium_left(now) > 0),
                tokens_silver: m.tokens[0],
                tokens_gold: m.tokens[1],
            }
        })
        .collect();
    let cap = team_cap(r.row(cx));
    let msg = Room98 {
        members,
        room_id: r.id,
        clan_a: clan_a.0,
        clan_b: clan_b.0,
        clan_a_name: clan_a.1,
        clan_b_name: clan_b.1,
        loading_screen: String::new(),
        kind: r.kind,
        map: r.map,
        mode: r.mode,
        weather: r.weather,
        region: r.region,
        voice: r.voice,
        ai_ignore: r.ai_ignore,
        tokens_allocated: tokens_allocated(r).min(255) as u8,
        tokens_required: tokens_required(cx, r, now).min(255) as u8,
        ai_a: r.ai[0],
        ai_b: r.ai[1],
        players_a: r.players[0],
        players_b: r.players[1],
        cost_gold: TOKEN_COST_GOLD,
        cost_silver: TOKEN_COST_SILVER,
        // PER TEAM: the client makes "min-max" the allowed characters of each team's one-digit field (CustomMatchWindow
        // 1464-1472, 3471-3525). Min 1 player = what 62 PlayerCount accepts. A max of
        // 0 bots hides the bot settings (PROVEN CustomMatchWindow.setData `bShowBotsOptions`).
        min_ai: 0,
        max_ai: bot_cap(cx, r),
        min_players: 1,
        max_players: cap,
        unknown: [0, 0],
    };
    S2c::Raw { id: 0x98, payload: msg.to_bytes().unwrap_or_default() }
}

/// Why `a` cannot be in (or be invited to) a custom match now: a squad, another custom match, a queue or a running
/// online match. The texts are the game's own.
fn busy(cx: &Ctx, a: AccountId, local_match: bool) -> Option<&'static str> {
    let reg = &cx.registry;
    if reg.squads.squad_of(a).is_some_and(|q| q.members.len() > 1) {
        return Some(T_SQUAD);
    }
    crate::busy::reason(cx, a, local_match).map(|_| T_BUSY)
}

/// Clan battle entry rule (the official 0.65b text: "only players of your clan and one other clan may join clan
/// matches"; the refusals are the game's `st_custom_match_error_*` texts): the account needs a clan, and the clan must
/// be one of the room's two clans or fill the free slot.
fn clan_entry(cx: &Ctx, room: &Room, a: AccountId) -> Result<(), &'static str> {
    if room.kind != 1 {
        return Ok(());
    }
    let Some(c) = cx.lobby.store.clan_id_of(a).ok().flatten() else { return Err(T_NOT_IN_CLAN) };
    if room.clans.contains(&c) || room.clans.contains(&0) {
        Ok(())
    } else {
        Err(T_CLAN_MISMATCH)
    }
}

/// Give a clan to a free side for every clan present in the room and free the side of a clan nobody of whom is left
/// (a clan battle only). The host's clan never leaves while the room exists.
fn refresh_clans(cx: &mut Ctx, room_id: u32) {
    let Some(r) = cx.registry.rooms.by_id.get(&room_id) else { return };
    if r.kind != 1 {
        return;
    }
    let mut clans = r.clans;
    let present: Vec<u32> = r.members.iter().filter_map(|m| cx.lobby.store.clan_id_of(m.account).ok().flatten()).collect();
    for c in clans.iter_mut() {
        if *c != 0 && !present.contains(c) {
            *c = 0;
        }
    }
    for c in present {
        if c != 0 && !clans.contains(&c) {
            if let Some(slot) = clans.iter_mut().find(|x| **x == 0) {
                *slot = c;
            }
        }
    }
    if let Some(r) = cx.registry.rooms.by_id.get_mut(&room_id) {
        r.clans = clans;
    }
}

fn broadcast(cx: &mut Ctx, peer: &mut Peer, room_id: u32) {
    let Some(r) = cx.registry.rooms.by_id.get(&room_id).cloned() else { return };
    for m in &r.members {
        let msg = room_msg(cx, Some(&r));
        push_to(cx, peer, m.account, msg);
    }
}

/// Close a room: everybody leaves the table.
fn close(cx: &mut Ctx, room: &Room) {
    for m in &room.members {
        cx.registry.rooms.of.remove(&m.account);
    }
    cx.registry.rooms.by_id.remove(&room.id);
}

/// Take `a` out of its room: he gets `A0`. A host who leaves closes the room (the game's text "The custom match was
/// aborted by the commander" says so): the others get `A1` with that reason.
fn leave(cx: &mut Ctx, peer: &mut Peer, a: AccountId) {
    let Some(room) = cx.registry.rooms.room_of(a).cloned() else { return };
    if room.host == a {
        close(cx, &room);
        log::info!("{}: custom match {} closed: the host left", peer.addr, room.id);
        push_to(cx, peer, a, ended_msg());
        for o in room.members.iter().map(|m| m.account).filter(|x| *x != a) {
            push_to(cx, peer, o, aborted_msg(ABORTED_BY_HOST));
        }
    } else {
        cx.registry.rooms.remove_member(a);
        refresh_clans(cx, room.id);
        log::info!("{}: account {a} left custom match {}", peer.addr, room.id);
        push_to(cx, peer, a, ended_msg());
        broadcast(cx, peer, room.id);
    }
}

/// The connection of `a` ended.
pub fn on_gone(cx: &mut Ctx, peer: &mut Peer, a: AccountId) {
    leave(cx, peer, a);
    for r in cx.registry.rooms.by_id.values_mut() {
        r.invited.remove(&a);
    }
}

/// The row a new room starts on: the first variation-0 row the match server serves (with online matches), else the
/// first variation-0 row of the data.
fn default_row(cx: &Ctx) -> Option<game_data::matches::ModeRow> {
    let gd = &cx.lobby.data;
    let rows = gd.matches.modes();
    let fits = |r: &&game_data::matches::ModeRow| r.variation == 0 && r.map < 256 && r.mode < 256;
    let served = cx.lobby.online.as_ref().and_then(|h| rows.iter().filter(fits).find(|r| h.serves(gd, r)));
    served.or_else(|| rows.iter().find(fits)).cloned()
}

/// `62 xx` request.
pub fn handle(cx: &mut Ctx, peer: &mut Peer, payload: &[u8]) {
    let Some(me) = peer.account else { return };
    let req = match Custom62::decode(payload) {
        Ok(r) => r,
        Err(e) => {
            log::info!("{}: 62 request {:02x?} not decoded: {e}", peer.addr, payload);
            return;
        }
    };
    let in_room = cx.registry.rooms.room_of(me).cloned();
    let is_host = in_room.as_ref().is_some_and(|r| r.host == me);
    match req {
        Custom62::Create { kind } => {
            if in_room.is_some() {
                refuse(peer, "18 create", "already in a room", "st_custom_match_error_already_in_match");
                return;
            }
            if kind > 1 {
                log::info!("{}: 62 18 create: unknown kind {kind}", peer.addr);
                return;
            }
            if let Some(t) = busy(cx, me, peer.local_match.is_some()) {
                refuse(peer, "18 create", "busy", t);
                return;
            }
            let mut clan = 0;
            if kind == 1 {
                let c = cx.lobby.store.clan_of(me).ok().flatten();
                let Some(c) = c else {
                    refuse(peer, "18 create", "no clan", "st_no_clan_info");
                    return;
                };
                // the rank permission "battle commander" (LIKELY it is the right to lead clan battles: the exe has the
                // permission and no other user of it)
                if c.perms_of(me) & accounts::clans::PERM_BATTLE == 0 {
                    refuse(peer, "18 create", "no battle permission", "st_dont_have_permission");
                    return;
                }
                clan = c.id;
            }
            let Some(row) = default_row(cx) else {
                refuse(peer, "18 create", "no match row", "st_cant_create_custom_match_error");
                return;
            };
            let rooms = &mut cx.registry.rooms;
            rooms.next_id += 1;
            let id = rooms.next_id;
            let per_team = team_cap(Some(&row));
            let room = Room {
                id,
                host: me,
                kind,
                map: row.map as u8,
                mode: row.mode as u8,
                // the variation id (see the module docs): 0
                weather: row.variation as u8,
                // the first entry of the game-server list (9E id 0)
                region: 0,
                voice: 0,
                ai_ignore: 0,
                ai: [0, 0],
                players: [per_team, per_team],
                members: vec![RoomMemberState { account: me, team: TEAM_A, ready: true, tokens: [0, 0] }],
                invited: BTreeSet::new(),
                clans: [clan, 0],
            };
            rooms.of.insert(me, id);
            rooms.by_id.insert(id, room);
            log::info!("{}: custom match {id} created (kind {kind}, {})", peer.addr, row.project);
            broadcast(cx, peer, id);
        }
        Custom62::Invite { target } => {
            let (Some(room), Some(to)) = (in_room, account_of_public_id(target)) else { return };
            if !is_host {
                refuse(peer, "1A invite", "not the host", "st_dont_have_permission");
                return;
            }
            if !cx.registry.is_online(to) || room.members.len() >= MAX_MEMBERS {
                refuse(peer, "1A invite", "target offline or room full", "st_cant_send_custom_match_join_request_error");
                return;
            }
            if let Some(t) = busy(cx, to, false) {
                refuse(peer, "1A invite", "target busy", t);
                return;
            }
            if let Err(t) = clan_entry(cx, &room, to) {
                refuse(peer, "1A invite", "clan rule", t);
                return;
            }
            if let Some(r) = cx.registry.rooms.by_id.get_mut(&room.id) {
                r.invited.insert(to);
            }
            cx.registry.rooms.next_msg += 1;
            let message_id = 0x4000_0000 | cx.registry.rooms.next_msg;
            let clan = cx.lobby.store.clan_abbr_of(me);
            let msg = S2c::SystemMessage(SystemMessage { kind: 8, sender: public_id(me), name: peer.name.clone(), clan, flag: 0, text: String::new(), message_id });
            cx.fx.push(Cross { to, msg });
        }
        Custom62::Answer { inviter, accept } => {
            let Some(host) = account_of_public_id(inviter) else { return };
            let Some(room) = cx.registry.rooms.room_of(host).cloned() else {
                if accept == 0 {
                    refuse(peer, "1B answer", "the inviter has no room", "st_custom_match_error_player_no_longer_in_match");
                }
                return;
            };
            if !room.invited.contains(&me) {
                return;
            }
            if let Some(r) = cx.registry.rooms.by_id.get_mut(&room.id) {
                r.invited.remove(&me);
            }
            // 0 = accept, 1 = decline (PROVEN, protocol Custom62::Answer)
            if accept != 0 {
                return;
            }
            if in_room.is_some() || room.members.len() >= MAX_MEMBERS {
                let t = if in_room.is_some() { "st_custom_match_error_already_in_match" } else { "st_no_space_in_custom_match" };
                refuse(peer, "1B answer", "in a room or room full", t);
                return;
            }
            // the world may have changed since the invitation (a squad, a queue, a clan left or joined)
            let refused = busy(cx, me, peer.local_match.is_some()).map(str::to_owned).or_else(|| clan_entry(cx, &room, me).err().map(str::to_owned));
            if let Some(t) = refused {
                refuse(peer, "1B answer", "busy or clan rule", &t);
                return;
            }
            let rooms = &mut cx.registry.rooms;
            rooms.of.insert(me, room.id);
            if let Some(r) = rooms.by_id.get_mut(&room.id) {
                r.members.push(RoomMemberState { account: me, team: TEAM_NONE, ready: false, tokens: [0, 0] });
            }
            refresh_clans(cx, room.id);
            log::info!("{}: account {me} joined custom match {}", peer.addr, room.id);
            broadcast(cx, peer, room.id);
        }
        Custom62::Leave { player } => {
            let Some(room) = in_room else {
                // the client still shows a room the server does not hold (a restart, a lost message): A0 clears it
                log::info!("{}: 62 1C leave without a room: A0", peer.addr);
                peer.push(&ended_msg());
                return;
            };
            let target = account_of_public_id(player).unwrap_or(me);
            if target == me {
                leave(cx, peer, me);
            } else if is_host && room.has(target) {
                // the host removes a member: the same request with the member's id (the row's remove button, PROVEN)
                cx.registry.rooms.remove_member(target);
                refresh_clans(cx, room.id);
                log::info!("{}: custom match {}: the host removed account {target}", peer.addr, room.id);
                push_to(cx, peer, target, aborted_msg(KICKED));
                broadcast(cx, peer, room.id);
            } else {
                refuse(peer, "1C remove", "not the host or not a member", T_CANT_KICK);
            }
        }
        Custom62::MemberStatus { player, status: team } => {
            let Some(room) = in_room else { return };
            let target = account_of_public_id(player).unwrap_or(me);
            // the wire byte is the role itself: 0 team A, 1 team B, 2 unassigned (PROVEN, protocol Custom62::MemberStatus)
            if (target != me && !is_host) || !room.has(target) || team > TEAM_NONE {
                refuse(peer, "1D team", &format!("account {target} to role {team} not allowed"), T_CANT_STATUS);
                broadcast(cx, peer, room.id);
                return;
            }
            // a clan battle: a side is the clan of that side (the client only offers the join button of the own clan, PROVEN)
            if room.kind == 1 && team != TEAM_NONE {
                let clan = cx.lobby.store.clan_id_of(target).ok().flatten();
                if clan.is_none() || clan != Some(room.clans[usize::from(team)]) {
                    refuse(peer, "1D team", "clan side", T_CLAN_MISMATCH);
                    return;
                }
            }
            if team != TEAM_NONE && room.team_of(target) != Some(team) && room.team_size(team) >= room.players[usize::from(team)] as usize {
                refuse(peer, "1D team", "team full", T_TEAM_FULL);
                return;
            }
            if let Some(r) = cx.registry.rooms.by_id.get_mut(&room.id) {
                let host = r.host;
                if let Some(m) = r.members.iter_mut().find(|m| m.account == target) {
                    if m.team != team {
                        // a new position needs a new answer (CHOICE; the client does not say)
                        m.ready = m.account == host;
                    }
                    m.team = team;
                }
            }
            log::info!("{}: custom match {}: account {target} to role {team}", peer.addr, room.id);
            broadcast(cx, peer, room.id);
        }
        Custom62::Parameters { map, weather, mode, region, voice, ai_ignore } => {
            let Some(room) = in_room else { return };
            if !is_host {
                refuse(peer, "1E parameters", "not the host", T_CANT_PARAM);
                return;
            }
            let known = cx.lobby.data.matches.modes().iter().any(|r| r.map == u32::from(map) && r.mode == u32::from(mode) && r.variation == u32::from(weather));
            // the region is an entry id of the 9E server list (one id 0 when the list is empty)
            let servers = cx.lobby.cfg.game_servers.len().clamp(1, crate::servers::MAX_SERVERS);
            if !known || usize::from(region) >= servers {
                refuse(peer, "1E parameters", &format!("map {map} mode {mode} variation {weather} region {region} not a row / server"), T_CANT_PARAM);
                broadcast(cx, peer, room.id);
                return;
            }
            if let Some(r) = cx.registry.rooms.by_id.get_mut(&room.id) {
                (r.map, r.weather, r.mode, r.region, r.voice, r.ai_ignore) = (map, weather, mode, region, voice.min(1), ai_ignore.min(1));
            }
            // the new row may have smaller teams or no bots: keep the counts inside it
            if let Some(r) = cx.registry.rooms.by_id.get(&room.id).cloned() {
                let (cap, bots) = (team_cap(r.row(cx)), bot_cap(cx, &r));
                if let Some(r) = cx.registry.rooms.by_id.get_mut(&room.id) {
                    r.players = r.players.map(|p| p.min(cap));
                    r.ai = r.ai.map(|a| a.min(bots));
                }
            }
            broadcast(cx, peer, room.id);
        }
        Custom62::PlayerCount { team, count } | Custom62::AiCount { team, count } => {
            let Some(room) = in_room else { return };
            let is_players = matches!(req, Custom62::PlayerCount { .. });
            let (what, text) = if is_players { ("1F players", T_CANT_PLAYERS) } else { ("20 bots", T_CANT_AI) };
            if !is_host || team > 1 {
                refuse(peer, what, "not the host or no such team", text);
                return;
            }
            let cap = if is_players { team_cap(room.row(cx)) } else { bot_cap(cx, &room) };
            let low = if is_players { 1 } else { 0 };
            if count < low || count > cap {
                refuse(peer, what, &format!("{count} outside {low}..={cap}"), text);
                broadcast(cx, peer, room.id);
                return;
            }
            if is_players && room.team_size(team) > usize::from(count) {
                refuse(peer, what, &format!("{} players already on the team", room.team_size(team)), text);
                broadcast(cx, peer, room.id);
                return;
            }
            if let Some(r) = cx.registry.rooms.by_id.get_mut(&room.id) {
                if is_players {
                    r.players[usize::from(team)] = count;
                } else {
                    r.ai[usize::from(team)] = count;
                }
            }
            broadcast(cx, peer, room.id);
        }
        Custom62::SwapTeams => {
            let Some(room) = in_room else { return };
            if !is_host {
                refuse(peer, "24 swap", "not the host", T_CANT_SWAP);
                return;
            }
            if let Some(r) = cx.registry.rooms.by_id.get_mut(&room.id) {
                for m in &mut r.members {
                    m.team = match m.team {
                        TEAM_A => TEAM_B,
                        TEAM_B => TEAM_A,
                        t => t,
                    };
                }
                r.players.swap(0, 1);
                r.ai.swap(0, 1);
                r.clans.swap(0, 1);
            }
            broadcast(cx, peer, room.id);
        }
        Custom62::Start => {
            let Some(room) = in_room else {
                refuse(peer, "23 start", "not in a room", T_CANT_START);
                return;
            };
            if !is_host {
                refuse(peer, "23 start", "not the host", "st_dont_have_permission");
                return;
            }
            start(cx, peer, room);
        }
        Custom62::Ready { ready } => {
            let Some(room) = in_room else { return };
            if is_host {
                // the host's Play button is Start; he is always ready
                return;
            }
            if let Some(m) = cx.registry.rooms.by_id.get_mut(&room.id).and_then(|r| r.members.iter_mut().find(|m| m.account == me)) {
                m.ready = ready != 0;
            }
            broadcast(cx, peer, room.id);
        }
        Custom62::Tokens { gold: dg, silver: ds } => {
            let Some(room) = in_room else { return };
            // a change of the tokens the member pays for (deltas, gold first); an increase only while the room needs more
            // (the client allows adding only while allocated < required), and he must be able to pay the new total now
            let cur = room.members.iter().find(|m| m.account == me).map_or([0, 0], |m| m.tokens);
            let add = |n: u8, d: i8| (i16::from(n) + i16::from(d)).clamp(0, 255) as u8;
            let (silver, gold) = (add(cur[0], ds), add(cur[1], dg));
            let more = u32::from(silver) + u32::from(gold) > u32::from(cur[0]) + u32::from(cur[1]);
            if more && tokens_allocated(&room) >= tokens_required(cx, &room, crate::premium::unix_now()) {
                log::info!("{}: 62 21 {dg:+} gold / {ds:+} silver refused: the room's tokens are complete", peer.addr);
                broadcast(cx, peer, room.id);
                return;
            }
            let need = (u64::from(silver) * u64::from(TOKEN_COST_SILVER), u64::from(gold) * u64::from(TOKEN_COST_GOLD));
            let ok = cx.lobby.store.load_profile(me).is_ok_and(|p| u64::from(p.silver) >= need.0 && u64::from(p.gold) >= need.1);
            if !ok {
                refuse(peer, "21 tokens", "not enough money", "st_not_enough_money");
            } else if let Some(m) = cx.registry.rooms.by_id.get_mut(&room.id).and_then(|r| r.members.iter_mut().find(|m| m.account == me)) {
                m.tokens = [silver, gold];
                log::info!("{}: 62 21 pays {silver} silver / {gold} gold tokens in room {}", peer.addr, room.id);
            }
            broadcast(cx, peer, room.id);
        }
        Custom62::JoinRequest { .. } => {
            log::info!("{}: 62 request {req:?} is not served (no rules known)", peer.addr);
        }
    }
}

/// `62 23` Start: the host's room becomes an online match on the room's row when the lobby has a match server that
/// serves the row's project (online.rs); otherwise the host gets a client-shown text and the log says why. Rules (all
/// real): the teams' humans fit the player slots the host set, each team has a human or a bot, nobody is in another
/// match, the team members (not the reserve) are ready, the tokens are complete; bots only in a training room and only
/// when the world host drives them. The room closes when the match starts: the players get `A0` (after the `82`), the
/// reserve `A1` "The match started when you were in reserve" (the text exists only for this). CHOICE: the client would keep
/// its room through the match (its room record survives the `82`, the load and `close_current_match`; docs/notes/social.md
/// "Room lifetime after Start"), but every match is paid with tokens again, so the room ends here; the way back into the
/// running match is the seat rejoin of `online.rs` (82 again at a drop and after a new sign-in), which no room message
/// touches.
fn start(cx: &mut Ctx, peer: &mut Peer, room: Room) {
    let what = "23 start";
    let Some(row) = room.row(cx).cloned() else {
        refuse(peer, what, &format!("map {} mode {} variation {} is not a row", room.map, room.mode, room.weather), T_CANT_START);
        return;
    };
    let Some(hook) = cx.lobby.online.clone() else {
        refuse(peer, what, "no match server is attached", T_CANT_START);
        return;
    };
    if !hook.serves(&cx.lobby.data, &row) {
        refuse(peer, what, &format!("the match server does not serve {}", row.project), T_NOT_SERVED);
        return;
    }
    let team = |t: u8| -> Vec<AccountId> { room.members.iter().filter(|m| m.team == t).map(|m| m.account).collect() };
    let (a, b) = (team(TEAM_A), team(TEAM_B));
    let reserve = team(TEAM_NONE);
    let cap = usize::from(team_cap(Some(&row)));
    let ai = room.ai.map(usize::from);
    if ai != [0, 0] {
        let bots = usize::from(bot_cap(cx, &room));
        if room.kind != 0 || !hook.bots_available() || ai.iter().any(|n| *n > bots) {
            refuse(peer, what, &format!("bots {ai:?} not available here (at most {bots} per team)"), T_CANT_START);
            return;
        }
    }
    if a.len() > usize::from(room.players[0]) || b.len() > usize::from(room.players[1]) || a.len() + ai[0] > cap || b.len() + ai[1] > cap {
        refuse(peer, what, &format!("teams {}+{} / {}+{} bots over the slots ({:?}, match {cap} per team)", a.len(), ai[0], b.len(), ai[1], room.players), T_TEAM_FULL);
        return;
    }
    if a.len() + ai[0] == 0 || b.len() + ai[1] == 0 || a.is_empty() && b.is_empty() {
        refuse(peer, what, &format!("a team is empty ({} + {} bots, {} + {} bots)", a.len(), ai[0], b.len(), ai[1]), T_TEAM_EMPTY);
        return;
    }
    // a training room (custom_match_type 0) pays nothing (MatchKind::Training); a clan room is a clan battle
    let mut kind = MatchKind::Training;
    if room.kind == 1 {
        match clan_battle_check(cx, &room) {
            Ok(clans) => kind = MatchKind::Clan(clans),
            Err(t) => {
                refuse(peer, what, "clan battle rule", &t);
                return;
            }
        }
    }
    // every team member except the host has said "ready" (the client's Play button checks the same before it sends
    // Start, and it does not check the unassigned members: PROVEN 0x2001E8161)
    let unready: Vec<AccountId> = room.members.iter().filter(|m| m.account != room.host && m.team != TEAM_NONE && !m.ready).map(|m| m.account).collect();
    if !unready.is_empty() {
        refuse(peer, what, &format!("not ready: {unready:?}"), T_NOT_READY);
        if room.kind == 1 {
            // the ready check: the client of every unready member answers `62 22` ready by itself (PROVEN 0x20018F337)
            for to in unready {
                push_to(cx, peer, to, S2c::Reply(Reply::Ready30));
            }
        }
        return;
    }
    // the cost: allocated tokens must cover the required ones, and every payer must still have the money
    let now = crate::premium::unix_now();
    let (have, need) = (tokens_allocated(&room), tokens_required(cx, &room, now));
    if have < need {
        refuse(peer, what, &format!("tokens {have} of {need}"), T_CANT_START);
        return;
    }
    let cost = |m: &RoomMemberState| (u32::from(m.tokens[0]) * TOKEN_COST_SILVER, u32::from(m.tokens[1]) * TOKEN_COST_GOLD);
    if let Some(m) = room.members.iter().find(|m| {
        let (s, g) = cost(m);
        !cx.lobby.store.load_profile(m.account).is_ok_and(|p| p.silver >= s && p.gold >= g)
    }) {
        let why = format!("account {} cannot pay its tokens", m.account);
        refuse(peer, what, &why, "st_not_enough_money");
        return;
    }
    let humans: Vec<AccountId> = a.iter().chain(&b).copied().collect();
    let bots = (ai != [0, 0]).then(|| BotFill { need: ai, level: crate::online::bot_level(cx, &humans), ignore_players: room.ai_ignore != 0 });
    match crate::online::start_room(cx, &row, kind, [a, b], bots) {
        Ok(id) => {
            log::info!(
                "{}: custom match {} started as online match {id:#x} ({}, bots {ai:?}, bots ignore players {}, reserve {reserve:?})",
                peer.addr,
                room.id,
                row.project,
                room.ai_ignore != 0 && ai != [0, 0]
            );
            // the match exists: charge the tokens (checked above; a payer short of money now pays what he has)
            for m in room.members.iter().filter(|m| m.tokens != [0, 0]) {
                let (s, g) = cost(m);
                let money = crate::dispatch::update_account(cx, m.account, |p| {
                    let before = p.clone();
                    p.silver -= s.min(p.silver);
                    p.gold -= g.min(p.gold);
                    (crate::build::money_8b(p), crate::online::StartCharge::between(&before, p))
                });
                if let Some((money, taken)) = money {
                    // given back if nobody ever enters the match (online.rs, like the boosters)
                    crate::online::add_start_charge(cx, id, m.account, taken);
                    push_to(cx, peer, m.account, S2c::Money(money));
                }
            }
            close(cx, &room);
            for m in &room.members {
                let msg = if m.team == TEAM_NONE { aborted_msg(STARTED_WITHOUT_YOU) } else { ended_msg() };
                push_to(cx, peer, m.account, msg);
            }
        }
        Err(e) => {
            refuse(peer, what, &format!("the match server refused: {e}"), T_CANT_START);
        }
    }
}

/// The extra rules of a clan battle (all enforced from stored state, nothing canned): two different clans, team A is
/// the clan of side A and team B the clan of side B, nobody without a clan or with a foreign clan. Returns the clans of
/// team A and team B.
fn clan_battle_check(cx: &Ctx, room: &Room) -> Result<[u32; 2], String> {
    let [ca, cb] = room.clans;
    if ca == 0 || cb == 0 || ca == cb {
        return Err(T_CANT_START.into());
    }
    for m in &room.members {
        let clan = cx.lobby.store.clan_id_of(m.account).ok().flatten();
        let want = room.clans.get(usize::from(m.team)).copied();
        if clan.is_none() {
            return Err(T_NOT_IN_CLAN.into());
        }
        if m.team != TEAM_NONE && clan != want {
            return Err(T_CLAN_MISMATCH.into());
        }
    }
    Ok([ca, cb])
}

/// Used by dispatch: is this request a `62 xx`?
pub fn is_custom(req: &Request) -> Option<&[u8]> {
    match req {
        Request::Other { id: 0x62, payload } => Some(payload),
        _ => None,
    }
}

/// Members of the sender's room except the sender (chat channel 9), when `room_id` is the sender's room.
pub fn chat_audience(cx: &Ctx, me: AccountId, room_id: u64) -> Vec<AccountId> {
    match cx.registry.rooms.room_of(me) {
        Some(r) if u64::from(r.id) == room_id => r.members.iter().map(|m| m.account).filter(|a| *a != me).collect(),
        _ => vec![],
    }
}
