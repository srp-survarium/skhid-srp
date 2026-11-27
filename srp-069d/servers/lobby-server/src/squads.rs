//! Squads: registry, invites, AE pushes, 49 xx, 33 -> 88, ready flags, squad queue hook.
//!
//! Chain: A `4D{B,2}` -> B gets a B1 kind 2 popup; B accepts -> `4D{A,3}` -> A gets a B1 kind 3 (B's id in the sender
//! field) -> A's client sends `49 02 {B}` by itself -> B joins A's squad (a squad exists from the first join). PvE
//! (kinds 0x17 / 0x18) takes the same road. After every change an AE goes to every member (own "I am commander"
//! byte) and to the accounts that left (squad id 0).
//!
//! Rules: at most 3 members, the commander is `members[0]` (the commander leaving hands over to the next member),
//! only the commander invites / kicks / disbands, a squad of one dissolves, a disconnect is a leave. One exception: the
//! one-man PvE squad of a player who locked seats in the PvE window ([`Squads::set_pve_state`]); it dissolves when both
//! locks are cleared, on a leave or disconnect, and on a `2F` (the player then searches PvP alone).
//! Squad type: the commander's `5F 00` (PvE window) or seat lock `49 05` turns a normal squad into a PvE squad, his PvP
//! `2F` turns it back ([`SquadState::set_kind`], CHOICE; the client allows the PvE window to any commander, PROVEN).
//! Play: a member's 2F = ready, 31 = not ready (the client sends 31 when its own record is ready); the commander's 2F
//! with every other member ready = the squad's queue request ([`squad_queue`] hook, answered like single player);
//! rated (2F arg 2) is refused for squads (the client switches to mode 0 itself).
//!
//! UNVERIFIED (nothing was run against a client): AE record state bits 1..7 (only bit 0 = ready is sent), the AE
//! trailing u32 (0) and `x` byte (0), items level (0), the profile id of a mate (`MATE_PROFILE_BASE + account`, must
//! differ from the local profile id 1 so the client picks squad_member_1/2), 85 00 after a member's 2F, the B1 text.
//! AE delivery class: the client's table says unique (0xAE, table.rs, PROVEN) and `udp_transport` picks the class
//! from that table, so "ordered" cannot be sent; no switch exists. `protocol::ids::s2c_class(0xAE)` says Ordered but
//! is not used by `Peer::push`.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;

use accounts::{account_of_public_id, public_id, AccountId, Store};
use protocol::c2s::{Request, Squad49};
use protocol::s2c::*;

use crate::dispatch::Ctx;
use crate::peer::{Cross, Peer, Registry};

pub const MAX_MEMBERS: usize = 3;
/// Profile id of a squad mate in AE and 88 (UNVERIFIED, see the module docs).
pub const MATE_PROFILE_BASE: u32 = 1000;

pub fn mate_profile_id(account: AccountId) -> u32 {
    MATE_PROFILE_BASE.wrapping_add(account as u32)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub account: AccountId,
    pub ready: bool,
    /// PvE squads: the other bits of the AE `state` byte (bits 1..3 "I pay for slot bit - 1", bits 4 / 5 on the
    /// commander: locked slot 0 / 1). Never touches bit 0 (`ready`). See pve.rs.
    pub pve: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SquadState {
    pub id: u32,
    /// `members[0]` is the commander.
    pub members: Vec<Member>,
    /// Squad type of AE: 0 normal, 1 PvE ([`protocol::s2c::SQUAD_TYPE_PVE`]); decided by the invite that made the squad
    /// (4D 2 / 4D 0x17).
    pub kind: u8,
    /// PvE squads: the mission settings the commander sent (`5F 00`): (map id, difficulty).
    pub settings: Option<(u32, u32)>,
}

impl SquadState {
    pub fn is_pve(&self) -> bool {
        self.kind == SQUAD_TYPE_PVE
    }

    /// Slots the commander locked (bits 4 / 5 of his state).
    pub fn locked_slots(&self) -> u8 {
        (self.members[0].pve & PVE_LOCK_MASK).count_ones() as u8
    }

    /// The AE `state` byte of member `i`.
    pub fn state_byte(&self, i: usize) -> u8 {
        let m = &self.members[i];
        // bit 0 = ready; the commander's is never set
        ((i != 0 && m.ready) as u8) | m.pve
    }

    /// Switch the AE squad type (0 normal, 1 PvE). A change starts the squad's mode afresh: the PvE settings, the pays-for /
    /// lock bits and every ready flag are cleared. True when the type changed.
    ///
    /// Why a squad changes type: the client lets ANY squad commander use the PvE window (the `5F 00` gate
    /// 0x2001EBF79 / 0x2001EBF9C ignores the AE type byte, stored by 0x20018DDAC). What the original server did is
    /// NOT FOUND; converting the squad is a CHOICE: it is the only model in which the members of a main-lobby squad get
    /// the PvE seats view (a type-0 member sees only "Awaiting other players").
    fn set_kind(&mut self, kind: u8) -> bool {
        if self.kind == kind {
            return false;
        }
        self.kind = kind;
        self.settings = None;
        for m in &mut self.members {
            m.pve = 0;
            m.ready = false;
        }
        true
    }

    /// A membership change shifts the slots: pays-for / lock bits and ready flags of a PvE squad start again.
    fn reset_pve(&mut self) {
        if self.is_pve() {
            for m in &mut self.members {
                m.pve = 0;
                m.ready = false;
            }
        }
    }
}

/// AE state bits of a PvE squad record (docs/notes/pve.md 1.1): pays-for bits 1..3, lock bits 4 / 5.
pub const PVE_PAYS_MASK: u8 = 0b0000_1110;
pub const PVE_LOCK_MASK: u8 = 0b0011_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invite {
    /// 4D 2 sent, the target did not answer yet.
    Pending,
    /// 4D 3 came back, waiting for the inviter client's `49 02`.
    Accepted,
}

#[derive(Default)]
pub struct Squads {
    by_id: BTreeMap<u32, SquadState>,
    of: BTreeMap<AccountId, u32>,
    invites: BTreeMap<(AccountId, AccountId), (Invite, u8)>,
    next_id: u32,
    next_msg: u32,
}

impl Squads {
    pub fn squad_of(&self, a: AccountId) -> Option<&SquadState> {
        self.of.get(&a).and_then(|id| self.by_id.get(id))
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn invite_state(&self, inviter: AccountId, target: AccountId) -> Option<Invite> {
        self.invites.get(&(inviter, target)).map(|x| x.0)
    }

    fn msg_id(&mut self) -> u32 {
        self.next_msg += 1;
        self.next_msg
    }

    /// Whether `inviter` may invite `target` now.
    pub fn can_invite(&self, inviter: AccountId, target: AccountId) -> Result<(), &'static str> {
        if inviter == target {
            return Err("self");
        }
        if self.of.contains_key(&target) {
            return Err("target is in a squad");
        }
        if let Some(s) = self.squad_of(inviter) {
            if s.members[0].account != inviter {
                return Err("inviter is not the commander");
            }
            if s.members.len() >= MAX_MEMBERS {
                return Err("squad is full");
            }
        }
        Ok(())
    }

    pub fn record_invite(&mut self, inviter: AccountId, target: AccountId, kind: u8) {
        self.invites.insert((inviter, target), (Invite::Pending, kind));
    }

    /// The target accepted: `Some(kind of the invite)` when the invite exists.
    pub fn accept_invite(&mut self, inviter: AccountId, target: AccountId) -> Option<u8> {
        let e = self.invites.get_mut(&(inviter, target))?;
        e.0 = Invite::Accepted;
        Some(e.1)
    }

    /// `49 02`: the accepted invite of `joiner` makes him a member of the inviter's squad (created when needed).
    /// Returns the accounts whose AE changed.
    pub fn join(&mut self, inviter: AccountId, joiner: AccountId) -> Result<Vec<AccountId>, &'static str> {
        if self.invite_state(inviter, joiner) != Some(Invite::Accepted) {
            return Err("no accepted invite");
        }
        self.can_invite(inviter, joiner)?;
        // a PvE invite (4D 0x17) makes / joins a PvE squad, any other invite a normal one; the kinds never mix
        let kind = (self.invites.get(&(inviter, joiner)).map(|x| x.1) == Some(0x17)) as u8;
        if self.squad_of(inviter).is_some_and(|s| s.kind != kind) {
            return Err("the invite and the squad are of different types");
        }
        self.invites.remove(&(inviter, joiner));
        let id = match self.of.get(&inviter) {
            Some(id) => *id,
            None => {
                self.next_id += 1;
                let id = self.next_id;
                let state = SquadState { id, members: vec![Member { account: inviter, ready: false, pve: 0 }], kind, settings: None };
                self.by_id.insert(id, state);
                self.of.insert(inviter, id);
                id
            }
        };
        let s = self.by_id.get_mut(&id).unwrap();
        s.members.push(Member { account: joiner, ready: false, pve: 0 });
        s.reset_pve();
        self.of.insert(joiner, id);
        Ok(s.members.iter().map(|m| m.account).collect())
    }

    /// Take `a` out of his squad (leave, kick, disconnect). Returns the accounts whose AE changed (the leaver first).
    pub fn remove(&mut self, a: AccountId) -> Vec<AccountId> {
        let Some(id) = self.of.remove(&a) else { return vec![] };
        let s = self.by_id.get_mut(&id).unwrap();
        s.members.retain(|m| m.account != a);
        let mut out = vec![a];
        if s.members.len() < 2 {
            for m in s.members.drain(..) {
                self.of.remove(&m.account);
                out.push(m.account);
            }
            self.by_id.remove(&id);
        } else {
            // a new commander starts not ready (his own state must be 0)
            s.members[0].ready = false;
            s.members[0].pve = 0;
            s.reset_pve();
            out.extend(s.members.iter().map(|m| m.account));
        }
        out
    }

    /// `49 01` by the commander: everyone leaves.
    pub fn disband(&mut self, by: AccountId) -> Result<Vec<AccountId>, &'static str> {
        let id = *self.of.get(&by).ok_or("not in a squad")?;
        let s = self.by_id.get(&id).unwrap();
        if s.members[0].account != by {
            return Err("not the commander");
        }
        let all: Vec<AccountId> = s.members.iter().map(|m| m.account).collect();
        for a in &all {
            self.of.remove(a);
        }
        self.by_id.remove(&id);
        Ok(all)
    }

    /// `49 03` by the commander.
    pub fn kick(&mut self, by: AccountId, target: AccountId) -> Result<Vec<AccountId>, &'static str> {
        let s = self.squad_of(by).ok_or("not in a squad")?;
        if s.members[0].account != by {
            return Err("not the commander");
        }
        if by == target || !s.members.iter().any(|m| m.account == target) {
            return Err("target is not another member");
        }
        Ok(self.remove(target))
    }

    /// Ready flag of a member (the commander's state stays 0). Returns the squad's accounts.
    pub fn set_ready(&mut self, a: AccountId, ready: bool) -> Option<Vec<AccountId>> {
        let id = *self.of.get(&a)?;
        let s = self.by_id.get_mut(&id).unwrap();
        if s.members[0].account != a {
            s.members.iter_mut().find(|m| m.account == a)?.ready = ready;
        }
        Some(s.members.iter().map(|m| m.account).collect())
    }

    /// `49 05` of a PvE squad: the sender's new `state` byte, validated. A member's bit 0 is his ready flag, the commander's
    /// bit 0 is ignored (his Play is `5F 01`); bits 1..3 are kept only for slots that exist and are not the sender's own;
    /// bits 4 / 5 only from the commander and at most as many locks as free slots (3 - members). Returns the squad.
    ///
    /// A player in no squad who locks seats in the PvE window gets a one-man PvE squad (type 1, he is its commander, his
    /// record carries the locks). The client draws a lock only from the commander record of an AE
    /// (`SquadInfo.getLockedSlots`, bits 0x10 / 0x20), an AE with squad id 0 has no records (0x20018DD6D), and its PvE Play
    /// counts members + locks >= 3 over the records (0x2001B9EB3): with no squad a lock snaps back at once (seen with
    /// the real client). The PvE window handles a squad of one itself. CHOICE (no capture of the original server): the
    /// squad dissolves again when both locks are cleared (with no lock a squad of one cannot start: 1 < 3).
    pub fn set_pve_state(&mut self, a: AccountId, state: u8) -> Result<Vec<AccountId>, &'static str> {
        let Some(&id) = self.of.get(&a) else {
            let locks = state & PVE_LOCK_MASK;
            if locks == 0 {
                return Err("not in a squad");
            }
            self.next_id += 1;
            let id = self.next_id;
            self.by_id.insert(id, SquadState { id, members: vec![Member { account: a, ready: false, pve: locks }], kind: SQUAD_TYPE_PVE, settings: None });
            self.of.insert(a, id);
            return Ok(vec![a]);
        };
        let s = self.by_id.get_mut(&id).unwrap();
        if !s.is_pve() {
            // a normal squad: only the commander's seat lock turns it into a PvE squad (the window's 5F 00 normally comes
            // first, see set_pve_settings); a member's ready / pays-for before the commander opened PvE means nothing
            if s.members[0].account != a || state & PVE_LOCK_MASK == 0 {
                return Err("not a PvE squad");
            }
            s.set_kind(SQUAD_TYPE_PVE);
            log::info!("49 05 lock by the commander: the normal squad {} becomes a PvE squad", s.id);
        }
        let idx = s.members.iter().position(|m| m.account == a).unwrap();
        let n = s.members.len();
        let mut bits = 0u8;
        for slot in 0..n {
            if slot != idx && state & (2 << slot) != 0 {
                bits |= 2 << slot;
            }
        }
        if idx == 0 {
            // a lock only for an empty seat: the window fills seats player1.. with the other n - 1 members in record order
            // (PVEMatchSettingsWindow 3740-3881), seat player(k+1) has lock bit 0x10 << k (4458-4491), so it is empty when
            // k + 1 >= n. n = 2 allows only 0x20 (0x10 would be drawn on the member's seat).
            let mut locks = 0u8;
            for (k, bit) in [0x10u8, 0x20].into_iter().enumerate() {
                if state & bit != 0 && k + 1 >= n {
                    locks |= bit;
                }
            }
            s.members[0].pve = bits | locks;
        } else {
            s.members[idx].ready = state & 1 != 0;
            s.members[idx].pve = bits;
        }
        let out: Vec<AccountId> = s.members.iter().map(|m| m.account).collect();
        // a one-man PvE squad with no lock left is gone (the AE that follows has squad id 0)
        if n == 1 && s.members[0].pve & PVE_LOCK_MASK == 0 {
            self.by_id.remove(&id);
            self.of.remove(&a);
        }
        Ok(out)
    }

    /// Switch the type of `a`'s squad ([`SquadState::set_kind`]). The squad's members when the type changed.
    pub fn set_kind_of(&mut self, a: AccountId, kind: u8) -> Option<Vec<AccountId>> {
        let id = *self.of.get(&a)?;
        let s = self.by_id.get_mut(&id)?;
        s.set_kind(kind).then(|| s.members.iter().map(|m| m.account).collect())
    }

    /// The one-man PvE squad of `a` (seats locked in the PvE window), if that is his squad.
    pub fn solo_pve(&self, a: AccountId) -> bool {
        self.squad_of(a).is_some_and(|s| s.is_pve() && s.members.len() == 1)
    }

    /// `5F 00` by the commander: store the settings; the members must accept again (ready cleared). The commander of a
    /// normal squad opened the PvE window: the squad becomes a PvE squad ([`SquadState::set_kind`]; the AE that follows
    /// carries type 1, the members get `C2 00`). Returns the squad's members (commander first).
    pub fn set_pve_settings(&mut self, by: AccountId, map: u32, difficulty: u32) -> Result<Vec<AccountId>, &'static str> {
        let id = *self.of.get(&by).ok_or("not in a squad")?;
        let s = self.by_id.get_mut(&id).unwrap();
        if s.members[0].account != by {
            return Err("not the commander");
        }
        if s.set_kind(SQUAD_TYPE_PVE) {
            log::info!("5F 00: the normal squad {} becomes a PvE squad", s.id);
        }
        s.settings = Some((map, difficulty));
        for m in &mut s.members {
            m.ready = false;
        }
        Ok(s.members.iter().map(|m| m.account).collect())
    }

    /// Forget the invites of an account (disconnect).
    pub fn drop_invites(&mut self, a: AccountId) {
        self.invites.retain(|(i, t), _| *i != a && *t != a);
    }
}

/// The AE of `acct`: his squad with his own commander byte, or squad id 0.
pub fn ae_for(store: &Store, gd: &game_data::GameData, reg: &Registry, acct: AccountId) -> S2c {
    let Some(s) = reg.squads.squad_of(acct) else { return S2c::Squad(SquadUpdate { squad: None }) };
    let members = s
        .members
        .iter()
        .enumerate()
        .map(|(i, m)| SquadMember {
            account_id: public_id(m.account),
            profile_id: mate_profile_id(m.account),
            // the same content version as the 88 below (a cache key: a change makes the client ask 33 again)
            profile_version: match (store.load_profile(m.account), store.account(m.account)) {
                (Ok(p), Ok(Some(a))) => crate::build::wire_version(gd, &p, &a.name),
                _ => 0,
            },
            // the member's equipment level, as the client computes its own (items::equipment_level)
            items_level: store
                .load_profile(m.account)
                .map(|p| u32::from(crate::items::equipment_level(gd, &p, crate::premium::unix_now())))
                .unwrap_or(0),
            name: store.account(m.account).ok().flatten().map(|a| a.name).unwrap_or_default(),
            online: reg.is_online(m.account) as u8,
            state: s.state_byte(i),
        })
        .collect();
    S2c::Squad(SquadUpdate {
        squad: Some(Squad { id: s.id, commander: 0, kind: s.kind, members, i_am_commander: (s.members[0].account == acct) as u8, tail: 0 }),
    })
}

/// Queue an AE for each of `accounts` (delivered after the handler returned).
pub(crate) fn notify(cx: &mut Ctx, accounts: Vec<AccountId>) {
    for to in accounts {
        let msg = ae_for(&cx.lobby.store, &cx.lobby.data, cx.registry, to);
        cx.fx.push(Cross { to, msg });
    }
}

/// Called from the server when a peer is gone: a disconnect is a leave; the others get their AE directly.
pub fn on_disconnect(lobby: &crate::Lobby, registry: &mut Registry, peers: &mut HashMap<SocketAddr, Peer>, addr: SocketAddr) {
    let Some(acct) = registry.online().find(|(_, a)| *a == addr).map(|(a, _)| a) else { return };
    registry.squads.drop_invites(acct);
    if let Some(t) = registry.queue.remove_account(acct) {
        log::info!("{addr}: account {acct} disconnected: its search (order {}) ends", t.order);
        for to in t.accounts.into_iter().filter(|a| *a != acct) {
            if let Some(p) = registry.addr_of(to).and_then(|a| peers.get_mut(&a)) {
                p.push(&S2c::QueueState(QueueState { state: 0 }));
            }
        }
    }
    if let Some(t) = registry.pve.remove_account(acct) {
        log::info!("{addr}: account {acct} disconnected: its PvE search (order {}) ends", t.order);
        for to in t.accounts.into_iter().filter(|a| *a != acct) {
            if let Some(p) = registry.addr_of(to).and_then(|a| peers.get_mut(&a)) {
                p.push(&S2c::QueueState(QueueState { state: 0 }));
            }
        }
    }
    let changed = registry.squads.remove(acct);
    if !changed.is_empty() {
        log::info!("{addr}: account {acct} disconnected: left the squad");
    }
    for to in changed.into_iter().filter(|a| *a != acct) {
        if let Some(a) = registry.addr_of(to) {
            let msg = ae_for(&lobby.store, &lobby.data, registry, to);
            if let Some(p) = peers.get_mut(&a) {
                p.push(&msg);
            }
        }
    }
}

/// 4D kinds 2, 3, 0x17, 0x18.
pub fn people(cx: &mut Ctx, peer: &mut Peer, id: u64, kind: u8, text: &str) {
    let Some(me) = peer.account else { return };
    let Some(other) = account_of_public_id(id) else { return };
    match kind {
        2 | 0x17 => {
            if !cx.registry.is_online(other) {
                log::info!("{}: squad invite to {other}: offline, refused", peer.addr);
                return;
            }
            if let Err(why) = cx.registry.squads.can_invite(me, other) {
                log::info!("{}: squad invite to {other} refused: {why}", peer.addr);
                return;
            }
            if cx.registry.squads.squad_of(me).is_some_and(|s| s.is_pve() != (kind == 0x17)) {
                log::info!("{}: squad invite to {other} refused: the squad is of the other type", peer.addr);
                return;
            }
            cx.registry.squads.record_invite(me, other, kind);
            let message_id = cx.registry.squads.msg_id();
            let text: String = text.chars().take(200).collect();
            log::info!("{}: squad invite {me} -> {other} (B1 kind {kind})", peer.addr);
            let msg = S2c::SystemMessage(SystemMessage { kind, sender: public_id(me), name: peer.name.clone(), text, message_id, ..Default::default() });
            cx.fx.push(Cross { to: other, msg });
        }
        3 | 0x18 => {
            // `other` is the inviter
            let Some(ikind) = cx.registry.squads.accept_invite(other, me) else {
                log::info!("{}: accept of an invite from {other} that does not exist", peer.addr);
                return;
            };
            let k = if ikind == 0x17 { 0x18 } else { 3 };
            let message_id = cx.registry.squads.msg_id();
            log::info!("{}: squad invite of {other} accepted (B1 kind {k} to the inviter)", peer.addr);
            let msg = S2c::SystemMessage(SystemMessage { kind: k, sender: public_id(me), name: peer.name.clone(), message_id, ..Default::default() });
            cx.fx.push(Cross { to: other, msg });
        }
        _ => {}
    }
}

/// 49 xx.
pub fn squad_request(cx: &mut Ctx, peer: &mut Peer, r: &Squad49) {
    let Some(me) = peer.account else { return };
    let res = match r {
        Squad49::Accepted { player } => match account_of_public_id(*player) {
            Some(j) if cx.registry.is_online(j) => cx.registry.squads.join(me, j),
            _ => Err("joiner unknown or offline"),
        },
        Squad49::Disband => cx.registry.squads.disband(me),
        Squad49::Kick { player } => match account_of_public_id(*player) {
            Some(t) => cx.registry.squads.kick(me, t),
            None => Err("bad id"),
        },
        Squad49::Leave => Ok(cx.registry.squads.remove(me)),
        // 49 05: the member's own state of a PvE squad (not while the squad searches: the search owns the slots)
        Squad49::PveState { .. } if cx.registry.pve.ticket_of(me).is_some() || cx.registry.queue.ticket_of(me).is_some() => Err("the squad is searching"),
        Squad49::PveState { state, .. } => cx.registry.squads.set_pve_state(me, *state),
    };
    // a search of a squad ends when its members change (a joiner is not in the ticket: CHOICE, docs/notes/social.md)
    if matches!(r, Squad49::Disband | Squad49::Kick { .. } | Squad49::Leave | Squad49::Accepted { .. }) {
        if let Ok(changed) = &res {
            crate::pve::drop_tickets_of(cx, changed);
            // the PvP search of the squad too: its ticket names the old members (85 00 to each of them)
            for a in changed {
                if let Some(t) = cx.registry.queue.remove_account(*a) {
                    log::info!("PvP search {} ends: the squad changed", t.order);
                    for m in t.accounts {
                        cx.fx.push(Cross { to: m, msg: S2c::QueueState(QueueState { state: 0 }) });
                    }
                }
            }
        }
    }
    // a joiner of a PvE squad with settings gets them (the window asks him to accept)
    if let (Squad49::Accepted { player }, Ok(_)) = (r, &res) {
        if let (Some(j), Some((map, difficulty))) = (account_of_public_id(*player), cx.registry.squads.squad_of(me).and_then(|s| s.settings)) {
            cx.fx.push(Cross { to: j, msg: PveSettingsChanged { map, difficulty }.message() });
        }
    }
    match res {
        // nothing changed (no squad): the sender still gets his state (AE with squad id 0)
        Ok(changed) if changed.is_empty() => notify(cx, vec![me]),
        Ok(changed) => {
            log::info!("{}: 49 {r:?}: AE to {} accounts", peer.addr, changed.len());
            notify(cx, changed);
        }
        Err(why) => {
            log::info!("{}: 49 {r:?} refused: {why}", peer.addr);
            notify(cx, vec![me]);
        }
    }
}

/// 2F / 31 of a squad member. Returns false when the peer is in no squad (the caller answers as for a single player).
pub fn queue(cx: &mut Ctx, peer: &mut Peer, req: &Request) -> bool {
    let Some(me) = peer.account else { return false };
    // a one-man PvE squad only holds the PvE window's seat locks: a PvP Play leaves it and searches alone (CHOICE; the
    // client offers PvP Play with it, and refusing would leave the player stuck until he unlocks the seats)
    if cx.registry.squads.solo_pve(me) && cx.registry.pve.ticket_of(me).is_none() {
        let ch = cx.registry.squads.remove(me);
        log::info!("{}: 2F / 31 in a one-man PvE squad: the squad ends, the player searches alone", peer.addr);
        notify(cx, ch);
        return false;
    }
    let Some(s) = cx.registry.squads.squad_of(me) else { return false };
    // the commander of a PvE squad presses PvP Play: the squad becomes a normal squad again (the reverse of the 5F 00
    // conversion, CHOICE; without it a leader who only looked into the PvE window could never queue PvP again). The ready
    // flags were just cleared, so this first 2F ends in "not every member is ready" and the members press Play as usual.
    if s.is_pve() && s.members[0].account == me && s.members.len() > 1 && cx.registry.pve.ticket_of(me).is_none() {
        let id = s.id;
        if let Some(ch) = cx.registry.squads.set_kind_of(me, 0) {
            log::info!("{}: 2F: the PvE squad {id} becomes a normal squad", peer.addr);
            notify(cx, ch);
        }
    }
    let Some(s) = cx.registry.squads.squad_of(me) else { return false };
    if s.is_pve() {
        // a PvE squad plays through the PvE window (5F), never through 2F / 31
        log::info!("{}: 2F / 31 in a PvE squad: no search ({:?})", peer.addr, req);
        peer.push(&S2c::QueueState(QueueState { state: 0 }));
        return true;
    }
    let commander = s.members[0].account == me;
    let all_ready = s.members[1..].iter().all(|m| m.ready);
    let members: Vec<AccountId> = s.members.iter().map(|m| m.account).collect();
    // the answer to the pressing peer (85 and 83 2F) was already pushed by the queue
    let mut joined = false;
    match req {
        Request::Queue { arg, .. } if *arg == 2 => {
            log::info!("{}: 2F rated refused for a squad", peer.addr);
        }
        Request::Queue { arg, mode, .. } if commander => {
            if all_ready {
                joined = squad_queue(cx, peer, &members, *mode, *arg == 2);
                for m in &members[1..] {
                    cx.registry.squads.set_ready(*m, false);
                }
                notify(cx, members);
            } else {
                log::info!("{}: squad queue refused: not every member is ready", peer.addr);
            }
        }
        Request::Queue { .. } => {
            let ch = cx.registry.squads.set_ready(me, true).unwrap_or_default();
            log::info!("{}: squad member ready", peer.addr);
            notify(cx, ch);
        }
        _ => {
            if let Some(ch) = cx.registry.squads.set_ready(me, false) {
                log::info!("{}: squad member not ready / queue cancelled", peer.addr);
                notify(cx, ch);
            }
            if cx.registry.queue.ticket_of(me).is_some() {
                // the squad's search ends for everybody (85 00 to each member)
                crate::queue::cancel(cx, peer);
                joined = true;
            }
        }
    }
    // the client waits for 85 after every 2F / 31 (flag EBBD4)
    if !joined {
        let st = crate::queue::state_of(cx, me);
        peer.push(&st);
    }
    true
}

/// The commander queues the whole squad (every other member ready): one ticket for all members (queue.rs). Online
/// squad matches need the match server: when the queue holds enough players the search is refused (`85 00`). Returns
/// true when the queue already answered the pressing peer; false = the caller answers (`85 00` without the queue).
pub fn squad_queue(cx: &mut Ctx, peer: &mut Peer, members: &[AccountId], mask: u32, rated: bool) -> bool {
    if crate::queue::join(cx, peer, members.to_vec(), mask, rated) {
        return true;
    }
    log::info!("{}: squad queue of {} members {members:?} (not queued)", peer.addr, members.len());
    false
}

/// 33 `u8, u64 member, u32 profile id` -> 88 with the mate's account body (only for a member of the own squad).
pub fn member_profile(cx: &mut Ctx, peer: &mut Peer, member: u64) {
    let Some(me) = peer.account else { return };
    let Some(m) = account_of_public_id(member) else { return };
    let ok = cx.registry.squads.squad_of(me).is_some_and(|s| s.members.iter().any(|x| x.account == m));
    if !ok {
        log::info!("{}: 33 for {m}: not a squad mate, ignored", peer.addr);
        return;
    }
    let (Ok(p), Ok(Some(acc))) = (cx.lobby.store.load_profile(m), cx.lobby.store.account(m)) else { return };
    let mut body = crate::build::account_body_current(&cx.lobby.data, &p, &acc.name);
    let pid = mate_profile_id(m);
    if let Some(wp) = body.profiles.first_mut() {
        wp.id = pid;
    }
    // the trailing u32 is the profile version (the same value AE sends as `profile_version`)
    let version = crate::build::wire_version(&cx.lobby.data, &p, &acc.name);
    peer.push(&S2c::MemberProfile(MemberProfile88 { body, version }));
}
