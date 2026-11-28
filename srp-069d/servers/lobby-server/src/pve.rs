//! PvE team missions, lobby side (docs/notes/pve.md): the PvE squad, the commander's settings, the
//! members' state, Play (solo `5F 02` / commander `5F 01`), the PvE queue of 3, the top 100 and the rules of the keys.
//! The match itself is not here: a full queue starts through the online path (`online::start_kind` with
//! [`MatchKind::Pve`]), which stays off until a captured world exists.
//!
//! Evidence labels as in the notes: PROVEN = the 0.69d exe / data, LIKELY = layout proven, meaning inferred, CHOICE = a rule
//! the server author picked because nothing proves it, OPEN = unknown, nothing invented.
//!
//! * **Squad** (PROVEN 0x20018DC00 / 0x20018DAF0): a PvE squad is a normal squad with `AE` type 1, made by the invite `4D 17`
//!   (accept `4D .. 0x18`) or by the commander of a normal squad opening the PvE window (`5F 00` / a seat lock; his PvP `2F`
//!   turns it back: squads.rs, CHOICE); the member state byte carries ready (bit 0), pays-for (bits 1..3), locked slots
//!   (bits 4 / 5 of the commander, only for an empty seat). `49 04` is leave, `49 05` is the state (squads.rs).
//! * **Settings** (PROVEN): `5F 00 map, difficulty` by the commander -> `C2 00` to the members (they must accept again: CHOICE,
//!   ready flags are cleared, the exe's accept is local only).
//! * **Play** (PROVEN message flow): `5F 02` solo, `5F 01` commander. Refusals are `84 x 5F u8 1, u8 code`: 4 not enough money,
//!   5 not all members ready, 6 no match server, anything else generic. Then `85 04 order ..` = the PvE search; `31 order`
//!   cancels (`85 00`).
//! * **Who pays what, and when** (the exe only shows the prices and the denial codes; the rest is CHOICE):
//!   - at Play nothing is charged: the server only checks that every payer *could* pay (denial code 4 otherwise);
//!   - the charge is taken when the match is formed, right before the match server is asked to create it (the players
//!     are charged for a match that starts); a failed creation refunds everything (no charge, `84 5F 6`, `85 00`);
//!   - each player pays one access key (free key 1617 first, then premium key 1384); without a key he pays the ticket price
//!     in gold (`pve_default_prices.ticket_price`, the gold price the window shows as `play_cost_gold`);
//!   - a player with the pays-for bit of a slot set pays `sponsor_price` gold for that member (who then pays nothing);
//!   - the commander pays `block_slot_price` gold per locked slot;
//!   - when the final check at the match start finds a payer short, that squad leaves the queue with code 4.
//! * **Queue of 3** (CHOICE; the exe has no queue rule, the script starts a match when 2 or 3 humans entered): tickets of
//!   the same mission and difficulty are joined, oldest first, while the players fit the smallest `want` (3 minus the
//!   squad's locked slots); the match starts when the players fill `want`. Squads stay together. The maze is drawn
//!   while the world host can load a map; else a loaded maze with room is taken, and with neither the complete group
//!   stays queued, uncharged, and is tried again every second ([`crate::host_room::pve_maze`]).
//! * **Keys** (inventory items 1384 / 1617, PROVEN data): the daily rule "2 free keys for the first two wins of a day" is
//!   implemented ([`win_key`]; LIKELY 0.69d: the client has the text `st_pve_tokens_resupply_info`, the number comes from the
//!   0.50 release notes, a UTC day is a CHOICE); the one-time "5 keys at the first login after 0.50" was a release gift for
//!   existing players: OPEN, not given.

use std::collections::BTreeMap;

use accounts::{account_of_public_id, public_id, AccountId, Profile, PveRun, Store};
use game_data::pve::{PveData, KEY_FREE, KEY_PREMIUM};
use game_data::GameData;
use protocol::c2s::Pve5F;
use protocol::s2c::pve::deny;
use protocol::s2c::*;

use crate::achievements;
use crate::dispatch::{update_account, Ctx};
use crate::online::MatchKind;
use crate::peer::{Cross, Peer};

/// Free keys of the first wins of a day (LIKELY, see the module docs).
pub const DAILY_FREE_KEYS: u32 = 2;

/// One price of the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// One access key (free, then premium) or, without a key, the ticket price in gold.
    Ticket,
    /// Plain gold (sponsor price, locked slots).
    Gold(u32),
}

/// `payer` pays `cost` for `for_whom` (the same account for a player's own ticket).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charge {
    pub payer: AccountId,
    pub for_whom: AccountId,
    pub cost: Cost,
}

/// What a PvE start costs: `members` = (account, state byte pays-for bits) in squad order (the commander first),
/// `locked` = slots the commander locked.
pub fn plan_charges(pve: &PveData, members: &[(AccountId, u8)], locked: u8) -> Vec<Charge> {
    let mut out = Vec::new();
    for (i, (acct, _)) in members.iter().enumerate() {
        // bit i + 1 of another member's state = "I pay for slot i"; the first sponsor in squad order pays
        let sponsor = members.iter().enumerate().find(|(j, (_, bits))| *j != i && bits & (2 << i) != 0).map(|(_, (a, _))| *a);
        out.push(match sponsor {
            Some(s) => Charge { payer: s, for_whom: *acct, cost: Cost::Gold(pve.prices.sponsor_price) },
            None => Charge { payer: *acct, for_whom: *acct, cost: Cost::Ticket },
        });
    }
    if locked > 0 {
        if let Some((commander, _)) = members.first() {
            out.push(Charge { payer: *commander, for_whom: *commander, cost: Cost::Gold(pve.prices.block_slot_price.saturating_mul(locked as u32)) });
        }
    }
    out
}

/// What was taken from one account: units of keys (uid, dict) and gold.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub keys: Vec<(u32, u16)>,
    pub gold: u32,
}

/// The payer cannot pay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Short;

/// The access keys a player holds (free 1617 and bought 1384).
pub fn spendable_key_count(p: &Profile) -> u32 {
    p.items.iter().filter(|i| i.dict_id == KEY_FREE || i.dict_id == KEY_PREMIUM).map(|i| i.count).sum()
}

/// The paid-respawn budget of a PvE player for his options record (`+0x2D0` keys, `+0x2D4` gold; CHOICE): every
/// spendable access key (free and bought; the entry fee is already taken when the match is created) and his whole gold
/// balance. OPEN: what the original server wrote (all keys or only bought ones, a gold cap).
pub fn respawn_budget(p: &Profile) -> (u32, u32) {
    (spendable_key_count(p), p.gold)
}

/// The access key a payment uses: the free key (1617) first, then the premium key (1384); (uid, dict) of its stack.
pub fn spendable_key(p: &Profile) -> Option<(u32, u16)> {
    [KEY_FREE, KEY_PREMIUM].into_iter().find_map(|d| p.items.iter().find(|i| i.dict_id == d && i.count > 0).map(|i| (i.uid, d)))
}

/// Take `costs` from a profile; on `Err` the profile may be half changed (callers work on a copy).
pub fn take(p: &mut Profile, pve: &PveData, costs: &[Cost]) -> Result<Receipt, Short> {
    let mut r = Receipt::default();
    for c in costs {
        match c {
            Cost::Ticket => {
                match spendable_key(p) {
                    Some((uid, dict)) => {
                        p.consume(uid, 1).ok_or(Short)?;
                        r.keys.push((uid, dict));
                    }
                    None => {
                        p.spend(accounts::Currency::Gold, pve.ticket_cost() as u64).map_err(|_| Short)?;
                        r.gold += pve.ticket_cost();
                    }
                }
            }
            Cost::Gold(n) => {
                p.spend(accounts::Currency::Gold, *n as u64).map_err(|_| Short)?;
                r.gold += *n;
            }
        }
    }
    Ok(r)
}

/// The receipts of a started PvE match go to its lobby entry (given back if nobody ever enters it).
pub(crate) fn keep_receipts(cx: &mut Ctx, id: u32, receipts: BTreeMap<AccountId, Receipt>) {
    crate::online::set_pve_receipts(cx, id, receipts);
}

/// A PvE match that ended before anybody entered it: every payer gets his keys and gold back, and his client its
/// inventory (`89`) and money (`8B`) as they are now.
pub(crate) fn refund_unplayed(cx: &mut Ctx, id: u32, receipts: &BTreeMap<AccountId, Receipt>) {
    for (a, r) in receipts {
        if update_account(cx, *a, |p| give_back(p, r)).is_none() {
            log::error!("PvE match {id:#x}: account {a}: the start's charge could not be given back ({r:?})");
            continue;
        }
        log::info!("PvE match {id:#x} ended before anybody entered: account {a} gets back {} key(s) and {} gold", r.keys.len(), r.gold);
        if let Ok(p) = cx.lobby.store.load_profile(*a) {
            cx.fx.push(Cross { to: *a, msg: S2c::Inventory(crate::build::inventory_89(&cx.lobby.data, &p)) });
            cx.fx.push(Cross { to: *a, msg: S2c::Money(crate::build::money_8b(&p)) });
        }
    }
}

/// Put a receipt back (a start that failed after the charge).
pub fn give_back(p: &mut Profile, r: &Receipt) {
    for (_, dict) in &r.keys {
        match p.items.iter_mut().find(|i| i.dict_id == *dict) {
            Some(row) => row.count += 1,
            None => {
                p.add_item(*dict, 1);
            }
        }
    }
    p.credit(accounts::Currency::Gold, r.gold as u64);
}

/// One search of the PvE queue: a player or a whole squad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub order: u32,
    /// Squad order, the commander first.
    pub accounts: Vec<AccountId>,
    pub map: u32,
    pub difficulty: u8,
    /// Players the match has for this ticket: 3 minus the locked slots.
    pub want: u8,
    pub since: i64,
    pub charges: Vec<Charge>,
}

/// The PvE queue (in the [`Registry`](crate::Registry)).
#[derive(Debug, Default)]
pub struct PveQueue {
    next_order: u32,
    tickets: Vec<Ticket>,
    /// a complete group the world host's room held: tried again from the online tick
    pub held: crate::host_room::Held,
    /// the sequence number of the last match started on a maze project (the loaded mazes take turns)
    seq: u64,
    maze_seq: BTreeMap<String, u64>,
}

impl PveQueue {
    /// None when an account of the list already waits.
    pub fn enqueue(&mut self, accounts: Vec<AccountId>, map: u32, difficulty: u8, want: u8, charges: Vec<Charge>, now: i64) -> Option<Ticket> {
        if accounts.is_empty() || accounts.iter().any(|a| self.ticket_of(*a).is_some()) {
            return None;
        }
        self.next_order += 1;
        let t = Ticket { order: self.next_order, accounts, map, difficulty, want, since: now, charges };
        self.tickets.push(t.clone());
        Some(t)
    }

    pub fn ticket_of(&self, a: AccountId) -> Option<&Ticket> {
        self.tickets.iter().find(|t| t.accounts.contains(&a))
    }

    pub fn remove_account(&mut self, a: AccountId) -> Option<Ticket> {
        let i = self.tickets.iter().position(|t| t.accounts.contains(&a))?;
        Some(self.tickets.remove(i))
    }

    pub fn len(&self) -> usize {
        self.tickets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tickets.is_empty()
    }

    /// The tickets of the first complete match, oldest leader first: tickets of the same mission and difficulty are added
    /// while the players fit the smallest `want`; complete = the players fill it. Empty when no match is complete.
    pub fn next_group(&self) -> Vec<Ticket> {
        self.next_group_without(&[])
    }

    /// [`next_group`](Self::next_group) without the tickets `skip` (the orders of groups held this pass).
    pub fn next_group_without(&self, skip: &[u32]) -> Vec<Ticket> {
        for lead in self.tickets.iter().filter(|t| !skip.contains(&t.order)) {
            let mut group = vec![lead.clone()];
            let (mut total, mut cap) = (lead.accounts.len(), lead.want as usize);
            for o in self.tickets.iter().filter(|o| o.order != lead.order && o.map == lead.map && o.difficulty == lead.difficulty && !skip.contains(&o.order)) {
                let ncap = cap.min(o.want as usize);
                if total + o.accounts.len() <= ncap {
                    total += o.accounts.len();
                    cap = ncap;
                    group.push(o.clone());
                }
                if total == cap {
                    break;
                }
            }
            if total == cap {
                return group;
            }
        }
        vec![]
    }

    /// The last start on a maze project (0 = never).
    pub fn maze_last(&self, project: &str) -> u64 {
        self.maze_seq.get(project).copied().unwrap_or(0)
    }

    /// A match started on a maze project.
    pub fn note_maze(&mut self, project: &str) {
        self.seq += 1;
        self.maze_seq.insert(project.to_string(), self.seq);
    }
}

fn idle() -> S2c {
    S2c::QueueState(QueueState { state: 0 })
}

fn push_to(cx: &mut Ctx, peer: &mut Peer, to: AccountId, msg: S2c) {
    if Some(to) == peer.account {
        peer.push(&msg);
    } else {
        cx.fx.push(Cross { to, msg });
    }
}

/// Refuse a Play: the denial (which also re-enables Play) and `85 00`.
fn refuse(peer: &mut Peer, code: u8, why: &str) {
    log::info!("{}: 5F Play refused ({code}): {why}", peer.addr);
    peer.push(&PveDenied { code }.message());
    peer.push(&idle());
}

/// A PvE match can be created now: online matches are on and the match server has a world for the mission.
pub fn available(cx: &Ctx) -> bool {
    let pve = &cx.lobby.data.pve;
    pve.missions.first().is_some_and(|m| cx.lobby.online.as_ref().is_some_and(|h| h.available(m.id, pve.mode)))
}

/// Could the payers of `charges` pay now (nothing is changed)?
fn can_pay(store: &Store, gd: &GameData, charges: &[Charge]) -> bool {
    let mut costs: BTreeMap<AccountId, Vec<Cost>> = BTreeMap::new();
    for c in charges {
        costs.entry(c.payer).or_default().push(c.cost);
    }
    costs.iter().all(|(payer, list)| store.load_profile(*payer).is_ok_and(|mut p| take(&mut p, &gd.pve, list).is_ok()))
}

/// `5F 01` / `5F 02`: Play.
fn play(cx: &mut Ctx, peer: &mut Peer, map: u32, difficulty: u32) {
    let Some(me) = peer.account else { return };
    let gd = cx.lobby.data.clone();
    if !gd.pve.valid(map, difficulty) {
        return refuse(peer, deny::GENERIC, "unknown mission or difficulty");
    }
    if let Some(t) = cx.registry.pve.ticket_of(me) {
        // pressed again while the search runs: show it again
        let msg = PveSearching { order: t.order }.message();
        peer.push(&msg);
        return;
    }
    if crate::busy::reason(cx, me, peer.local_match.is_some()).is_some() {
        return refuse(peer, deny::GENERIC, "already searching or in a match");
    }
    // the group: the server's squad (a squad of the other type or a non-commander cannot start)
    let (members, locked) = match cx.registry.squads.squad_of(me) {
        None => (vec![(me, 0u8)], 0u8),
        Some(s) if !s.is_pve() => return refuse(peer, deny::GENERIC, "in a normal squad"),
        Some(s) if s.members[0].account != me => return refuse(peer, deny::GENERIC, "not the squad commander"),
        Some(s) => {
            if let Some(u) = s.members[1..].iter().find(|m| !m.ready) {
                return refuse(peer, deny::NOT_ALL_READY, &format!("member {} is not ready", u.account));
            }
            (s.members.iter().map(|m| (m.account, m.pve)).collect(), s.locked_slots())
        }
    };
    let want = (gd.pve.mission(map).map_or(3, |m| m.players) as usize).saturating_sub(locked as usize);
    if members.len() > want || want == 0 {
        return refuse(peer, deny::GENERIC, "more players than open slots");
    }
    // no match server (online matches off, no hook, no world): refuse before anything is taken
    if !available(cx) {
        return refuse(peer, deny::NO_MATCH_SERVER, "online matches are not available");
    }
    let charges = plan_charges(&gd.pve, &members, locked);
    if !can_pay(&cx.lobby.store, &gd, &charges) {
        return refuse(peer, deny::NOT_ENOUGH_MONEY, "a payer cannot pay");
    }
    let accounts: Vec<AccountId> = members.iter().map(|m| m.0).collect();
    let now = crate::premium::unix_now();
    let Some(t) = cx.registry.pve.enqueue(accounts.clone(), map, difficulty as u8, want as u8, charges, now) else {
        return refuse(peer, deny::GENERIC, "a member already waits");
    };
    log::info!("{}: 5F Play: PvE search order {} mission {map} difficulty {difficulty}, players {:?}, want {want}", peer.addr, t.order, t.accounts);
    for a in &accounts[1..] {
        cx.registry.squads.set_ready(*a, false);
    }
    for a in &accounts {
        push_to(cx, peer, *a, PveSearching { order: t.order }.message());
    }
    if accounts.len() > 1 {
        crate::squads::notify(cx, accounts);
    }
    start_full(cx);
}

/// Start every complete match of the queue.
pub fn start_full(cx: &mut Ctx) {
    // the tickets of groups the world host's room held in this pass
    let mut held: Vec<u32> = vec![];
    loop {
        let group = cx.registry.pve.next_group_without(&held);
        if group.is_empty() {
            return;
        }
        let gd = cx.lobby.data.clone();
        // the final check: a payer who spent his keys / gold since Play leaves the queue
        let mut dropped = false;
        for t in &group {
            if !can_pay(&cx.lobby.store, &gd, &t.charges) {
                log::info!("PvE search {}: a payer cannot pay at the match start, the search ends", t.order);
                cx.registry.pve.remove_account(t.accounts[0]);
                for a in &t.accounts {
                    cx.fx.push(Cross { to: *a, msg: PveDenied { code: deny::NOT_ENOUGH_MONEY }.message() });
                    cx.fx.push(Cross { to: *a, msg: idle() });
                }
                dropped = true;
            }
        }
        if dropped {
            continue;
        }
        match start_group(cx, &group) {
            GroupStart::Started => {}
            // the world host has no room for either maze now: the group stays queued (tried again from the online
            // tick); the groups behind it may still fit
            GroupStart::Held => held.extend(group.iter().map(|t| t.order)),
            GroupStart::Failed => {
                // the match could not be created: nothing was charged; the group is refused
                for t in &group {
                    cx.registry.pve.remove_account(t.accounts[0]);
                    for a in &t.accounts {
                        cx.fx.push(Cross { to: *a, msg: PveDenied { code: deny::NO_MATCH_SERVER }.message() });
                        cx.fx.push(Cross { to: *a, msg: idle() });
                    }
                }
            }
        }
    }
}

/// What became of a complete group.
enum GroupStart {
    Started,
    /// the world host's room holds it ([`crate::host_room::pve_maze`]): nothing charged, still queued
    Held,
    /// not started, nothing charged: the group is refused
    Failed,
}

/// The maze of a group's match ([`crate::host_room::pve_maze`]): the lobby's draw while it fits the world host's room
/// (loaded with room, or a map can load), else the other maze when it fits; while no map can load, a loaded maze with
/// room (not an idle or the drained one when the other fits); None = held (logged once per difficulty and kind, tried again every
/// second).
fn maze_of(cx: &mut Ctx, difficulty: u8) -> Option<u8> {
    use crate::host_room::Maze;
    let projects = [1u8, 2].map(|m| cx.lobby.data.pve.project(difficulty, m));
    let drawn = cx.lobby.roll(|r| r.below(2)) as u8 + 1;
    let room = cx.lobby.online.as_ref().and_then(|h| h.host_room());
    let names = [projects[0].as_deref(), projects[1].as_deref()];
    let last = |k: usize| {
        names[k].map_or("not in the data".to_string(), |p| match cx.registry.pve.maze_last(p) {
            0 => format!("{p}: never started"),
            s => format!("{p}: last started #{s}"),
        })
    };
    match crate::host_room::pve_maze(names, room.as_ref(), drawn, |p| cx.registry.pve.maze_last(p)) {
        Maze::Drawn(m) => Some(m),
        Maze::Other(m) => {
            if let Some(r) = &room {
                log::info!("PvE queue: the drawn maze {drawn} of difficulty {difficulty} is loaded and takes no more match, or is the idle map the next load evicts: maze {m} instead ({})", r.describe());
            }
            Some(m)
        }
        Maze::Loaded(m) => {
            if let Some(r) = &room {
                let drained = crate::host_room::drain_map(&r.status).map_or(String::new(), |d| format!("; the world host drains {} (loaded {} s)", d.project, d.loaded_secs));
                log::info!(
                    "PvE queue: no new map can load: maze {m} of difficulty {difficulty} is loaded with room ({}; {}; {}{drained}); of two, the busy one not drained, else the one started the longest ago",
                    last(0),
                    last(1),
                    r.describe()
                );
            }
            Some(m)
        }
        Maze::Held => {
            // pve_maze holds only with a status
            if let Some(r) = &room {
                cx.registry.pve.held.hold(cx.now, "PvE queue", u32::from(difficulty), crate::host_room::HoldKind::of(r), r);
            }
            None
        }
    }
}

/// Charge, create the match, and tell the payers ([`GroupStart`]).
fn start_group(cx: &mut Ctx, group: &[Ticket]) -> GroupStart {
    let gd = cx.lobby.data.clone();
    let (map, difficulty) = (group[0].map, group[0].difficulty);
    // the maze first: a held group is not charged
    let Some(maze) = maze_of(cx, difficulty) else { return GroupStart::Held };
    let mut costs: BTreeMap<AccountId, Vec<Cost>> = BTreeMap::new();
    for c in group.iter().flat_map(|t| t.charges.iter()) {
        costs.entry(c.payer).or_default().push(c.cost);
    }
    // 1. charge (each account atomically); a failure gives everything back
    let mut receipts: BTreeMap<AccountId, Receipt> = BTreeMap::new();
    let mut ok = true;
    for (payer, list) in &costs {
        let r = update_account(cx, *payer, |p| {
            let mut c = p.clone();
            let r = take(&mut c, &gd.pve, list).ok()?;
            *p = c;
            Some(r)
        });
        match r {
            Some(Some(r)) => {
                receipts.insert(*payer, r);
            }
            _ => {
                ok = false;
                break;
            }
        }
    }
    let refund = |cx: &mut Ctx, receipts: &BTreeMap<AccountId, Receipt>| {
        for (a, r) in receipts {
            update_account(cx, *a, |p| give_back(p, r));
        }
    };
    if !ok {
        log::warn!("PvE start: a charge failed, refunding {} accounts", receipts.len());
        refund(cx, &receipts);
        return GroupStart::Failed;
    }
    // 2. the match: one team of humans (team 0), the bots are the script's
    let players: Vec<AccountId> = group.iter().flat_map(|t| t.accounts.iter().copied()).collect();
    let started = crate::online::start_kind(cx, map, gd.pve.mode, gd.pve.weather, MatchKind::Pve { difficulty, maze }, [players.clone(), vec![]]);
    match started {
        Ok(id) => {
            log::info!("PvE match {id:#x} started: mission {map} difficulty {difficulty} maze {maze}, players {players:?}");
            if let Some(project) = gd.pve.project(difficulty, maze) {
                cx.registry.pve.note_maze(&project);
            }
            cx.registry.pve.held.started();
            keep_receipts(cx, id, receipts.clone());
            for t in group {
                cx.registry.pve.remove_account(t.accounts[0]);
            }
            // 3. the payers' clients see the keys and the gold go
            for (a, r) in &receipts {
                let mut by_uid: BTreeMap<u32, u8> = BTreeMap::new();
                for (uid, _) in &r.keys {
                    *by_uid.entry(*uid).or_default() += 1;
                }
                if !by_uid.is_empty() {
                    let entries: Vec<(u32, u8)> = by_uid.into_iter().collect();
                    for chunk in entries.chunks(200) {
                        cx.fx.push(Cross { to: *a, msg: S2c::ItemCounts(ItemCounts90 { entries: chunk.to_vec() }) });
                    }
                }
                if r.gold > 0 {
                    if let Ok(p) = cx.lobby.store.load_profile(*a) {
                        cx.fx.push(Cross { to: *a, msg: S2c::Money(crate::build::money_8b(&p)) });
                    }
                }
            }
            GroupStart::Started
        }
        Err(e) => {
            log::warn!("PvE match not started: {e}; the charge is given back");
            refund(cx, &receipts);
            GroupStart::Failed
        }
    }
}

/// `5F 00`: the commander changed the mission settings.
fn settings(cx: &mut Ctx, peer: &mut Peer, map: u32, difficulty: u32) {
    let Some(me) = peer.account else { return };
    if !cx.lobby.data.pve.valid(map, difficulty) {
        log::info!("{}: 5F 00 unknown mission {map} / difficulty {difficulty}: ignored", peer.addr);
        return;
    }
    if cx.registry.pve.ticket_of(me).is_some() || cx.registry.queue.ticket_of(me).is_some() {
        log::info!("{}: 5F 00 while the squad searches: ignored", peer.addr);
        return;
    }
    match cx.registry.squads.set_pve_settings(me, map, difficulty) {
        Ok(members) => {
            log::info!("{}: 5F 00 settings mission {map} difficulty {difficulty}: C2 00 to {} members", peer.addr, members.len() - 1);
            for a in members.iter().filter(|a| **a != me) {
                cx.fx.push(Cross { to: *a, msg: PveSettingsChanged { map, difficulty }.message() });
            }
            crate::squads::notify(cx, members);
        }
        Err(why) => log::info!("{}: 5F 00 ignored: {why}", peer.addr),
    }
}

/// `5F 03`: the top 100 from the stored runs, with the player's own squad.
fn top100(cx: &mut Ctx, peer: &mut Peer) {
    let Some(me) = peer.account else { return };
    let gd = cx.lobby.data.clone();
    let store = &cx.lobby.store;
    let Some(mission) = gd.pve.missions.first().map(|m| m.id) else { return };
    let ladder = store.pve_ladder(mission, 10_000).unwrap_or_else(|e| {
        log::error!("top 100: {e}");
        vec![]
    });
    let entry = |r: &accounts::PveLadderRow| -> TopEntry {
        let players = store
            .pve_run_players(r.result_id)
            .unwrap_or_default()
            .into_iter()
            .map(|p| TopPlayer {
                name: store.account(p.account).ok().flatten().map(|a| a.name).unwrap_or_default(),
                clan: store.clan_abbr_of(p.account),
                // the list's "chests_num" column (PVETop100Screen) shows this byte: the run's chests found (the world
                // host's count; the run score here showed 255 for 5 chests with the real client). OPEN: for a squad,
                // whether the column shows one player's byte or a sum of them (the list row renderer is not in the traced
                // movie code); every player carries the squad's count
                score: r.chests_found,
            })
            .collect();
        TopEntry { players, play_time: r.time_secs.min(u16::MAX as u32) as u16 }
    };
    let entries: Vec<TopEntry> = ladder.iter().take(protocol::s2c::pve::TOP_MAX).map(entry).collect();
    // the own squad: the best ladder entry among the runs the player took part in
    let mine = store.pve_result_ids_of(me).unwrap_or_default();
    let own = ladder.iter().position(|r| mine.contains(&r.result_id)).map(|i| (i as u32, entry(&ladder[i])));
    log::info!("{}: 5F 03 top 100: {} entries, own {:?}", peer.addr, entries.len(), own.as_ref().map(|o| o.0));
    match (PveTop100 { entries, own }).message() {
        Ok(m) => peer.push(&m),
        Err(e) => log::error!("top 100 encode: {e}"),
    }
}

/// `5F xx`.
pub fn handle(cx: &mut Ctx, peer: &mut Peer, req: &Pve5F) {
    match req {
        Pve5F::Settings { map, difficulty } => settings(cx, peer, *map, *difficulty),
        Pve5F::Start { map, difficulty, .. } => play(cx, peer, *map, *difficulty),
        Pve5F::Top100 => top100(cx, peer),
        // the client closes the result screen: a mission the world host ended is closed on the match server once every
        // human did so or left (online::on_close_stats)
        Pve5F::CloseStats => {
            log::debug!("{}: 5F 04 close PvE stats", peer.addr);
            if let Some(a) = peer.account {
                crate::online::on_close_stats(cx, a);
            }
        }
        // the end screen (pve_end.rs): the answer is a fresh `83 45 0B` + `8B` (LIKELY, no reply reader exists)
        Pve5F::UnlockRewards => crate::pve_end::unlock(cx, peer),
        Pve5F::RerollRewards => crate::pve_end::reroll(cx, peer),
    }
}

/// `31`: the PvE search of this player (or his squad) ends. False = no PvE search runs, the caller goes on.
pub fn cancel_if_queued(cx: &mut Ctx, peer: &mut Peer) -> bool {
    let Some(me) = peer.account else { return false };
    let Some(t) = cx.registry.pve.remove_account(me) else { return false };
    log::info!("{}: 31 PvE search {} cancelled, {} players leave", peer.addr, t.order, t.accounts.len());
    for a in &t.accounts {
        push_to(cx, peer, *a, idle());
    }
    for a in t.accounts.iter().skip(1) {
        cx.registry.squads.set_ready(*a, false);
    }
    if t.accounts.len() > 1 {
        crate::squads::notify(cx, t.accounts.clone());
    }
    true
}

/// A squad changed (leave, kick, disband): the PvE searches of these accounts end for everybody in them.
pub fn drop_tickets_of(cx: &mut Ctx, accounts: &[AccountId]) {
    for a in accounts {
        if let Some(t) = cx.registry.pve.remove_account(*a) {
            log::info!("PvE search {} ends: the squad changed", t.order);
            for m in t.accounts {
                cx.fx.push(Cross { to: m, msg: idle() });
            }
        }
    }
}

/// The wins of a day: the first [`DAILY_FREE_KEYS`] give a free key (1617). Returns true when a key was given.
pub fn win_key(store: &Store, gd: &GameData, account: AccountId, now: i64) -> bool {
    let wins = match store.pve_note_win(account, now.div_euclid(86_400)) {
        Ok(w) => w,
        Err(e) => {
            log::error!("PvE win of account {account} not counted: {e}");
            return false;
        }
    };
    if wins > DAILY_FREE_KEYS {
        return false;
    }
    let given = store.update_profile(account, |p| {
        match p.items.iter_mut().find(|i| i.dict_id == KEY_FREE) {
            Some(row) => row.count += 1,
            None => {
                p.add_item(KEY_FREE, 1);
            }
        }
        Ok::<(), ()>(())
    });
    let _ = gd;
    given.is_ok()
}

/// What a newly stored run changed: its run key (`accounts::pve_run_key`), the accounts that got a daily key, and the
/// achievement changes per account (with the item count before them).
struct RunOutcome {
    run_key: i64,
    keys: Vec<AccountId>,
    achievements: Vec<(AccountId, Vec<achievements::Change>, usize)>,
}

/// [`record_run`]: the run is stored as a new run under its own key (a PvE match id is the captured world's id and
/// repeats for every run of a maze; the caller stores one match end once, the lobby by
/// `Running::pve_sent`), then the daily keys and the achievements of a won run.
fn record_run_once(store: &Store, gd: &GameData, rewards: bool, run: &PveRun, now: i64) -> Result<RunOutcome, accounts::Error> {
    let run_key = store.pve_add_result(run)?;
    if !run.success {
        return Ok(RunOutcome { run_key, keys: vec![], achievements: vec![] });
    }
    let keys = run.players.iter().map(|p| p.account).filter(|a| win_key(store, gd, *a, now)).collect();
    // achievements 629 / 630 ("Win N matches in team missions" with `num_stashes_found` caches, "Find all stashes in a
    // team mission"): every player of the won run, with the run's caches found
    let mut players: Vec<AccountId> = run.players.iter().map(|p| p.account).collect();
    players.sort_unstable();
    players.dedup();
    let e = achievements::Event::PveWin { stashes: u32::from(run.chests_found) };
    let ach = players
        .into_iter()
        .filter_map(|a| achievements::event_for(store, gd, rewards, a, e).map(|(ch, before)| (a, ch, before)))
        .collect();
    Ok(RunOutcome { run_key, keys, achievements: ach })
}

/// Store a finished run (a new run under its own key every call), give the free daily keys to the players of a won run and count the
/// PvE achievements (rewards on). Returns the accounts that got a key. The online path calls [`record_run_notify`] when
/// the match server reports the end of a mission.
pub fn record_run(store: &Store, gd: &GameData, run: &PveRun, now: i64) -> Result<Vec<AccountId>, accounts::Error> {
    Ok(record_run_once(store, gd, true, run, now)?.keys)
}

/// What the client of `account` gets for a daily key that was just given: the received-item popup (`83 46 00` flag 1, one
/// unit of the stack, the way the drops of a match arrive in `play::reward_messages`). The stack row itself came from
/// the store, so the record carries its real uid.
pub fn key_messages(store: &Store, gd: &GameData, account: AccountId) -> Vec<S2c> {
    let Ok(p) = store.load_profile(account) else { return vec![] };
    let Some(row) = p.items.iter().find(|i| i.dict_id == KEY_FREE).copied() else { return vec![] };
    let record = crate::build::record_value(gd, &p, &row, 1);
    vec![S2c::Reply(Reply::Shop(Shop46::Buy { flag: 1, items: vec![record] }))]
}

/// [`record_run`] for the lobby: the players who got a daily key are told at once (their messages are delivered after the
/// handler returned), and the chests of the run are stored and the end screen's `83 45 0B` is pushed to every player
/// ([`crate::pve_end::store_run_chests`]) under the run's own key. Every call is a new run (the caller sends one result
/// per match: `Running::pve_sent`). The online path calls this when the
/// match server reports the end of a mission, and must send `99` to the same players at that moment (before `85 00`).
pub fn record_run_notify(cx: &mut Ctx, run: &PveRun, now: i64) -> Result<Vec<AccountId>, accounts::Error> {
    let rw = cx.lobby.cfg.achievement_rewards;
    let o = record_run_once(&cx.lobby.store, &cx.lobby.data, rw, run, now)?;
    log::info!("PvE match {:#x}: run stored under key {:#x}", run.match_id, o.run_key);
    for &to in &o.keys {
        for msg in key_messages(&cx.lobby.store, &cx.lobby.data, to) {
            cx.fx.push(Cross { to, msg });
        }
    }
    for (to, changes, before) in &o.achievements {
        for msg in achievements::changes_messages(cx, *to, changes, *before) {
            cx.fx.push(Cross { to: *to, msg });
        }
    }
    // the end screen's message stays the last one of the run
    crate::pve_end::store_run_chests(cx, run, o.run_key, now);
    Ok(o.keys)
}

/// Public id helper for tests and logs.
pub fn public(a: AccountId) -> u64 {
    public_id(a)
}

/// The account of a public id (PvE invites use `4D 17`, handled with the other squad invites).
pub fn account_of(id: u64) -> Option<AccountId> {
    account_of_public_id(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pve() -> PveData {
        PveData::parse(&game_data::testing::read("pve_069d.json")).unwrap()
    }

    #[test]
    fn solo_and_squad_charges() {
        game_data::require_tables!();
        let d = pve();
        // solo: one ticket
        assert_eq!(plan_charges(&d, &[(1, 0)], 0), vec![Charge { payer: 1, for_whom: 1, cost: Cost::Ticket }]);
        // three members, member 3 (slot 2) is paid by member 2 (bit 3 = slot 2); the commander locked nothing
        let c = plan_charges(&d, &[(1, 0), (2, 0b1000), (3, 0)], 0);
        assert_eq!(c[0].cost, Cost::Ticket);
        assert_eq!(c[1], Charge { payer: 2, for_whom: 2, cost: Cost::Ticket });
        assert_eq!(c[2], Charge { payer: 2, for_whom: 3, cost: Cost::Gold(50) });
        // two members, one locked slot: the commander also pays one block-slot price
        let c = plan_charges(&d, &[(1, 0x10), (2, 0)], 1);
        assert_eq!(c.last().unwrap(), &Charge { payer: 1, for_whom: 1, cost: Cost::Gold(50) });
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn take_prefers_free_then_premium_key_then_gold() {
        game_data::require_tables!();
        let d = pve();
        let mut p = Profile::starter();
        p.gold = 120;
        p.add_item(KEY_PREMIUM, 1);
        p.add_item(KEY_FREE, 1);
        let r = take(&mut p, &d, &[Cost::Ticket, Cost::Ticket, Cost::Ticket]).unwrap();
        assert_eq!(r.keys.iter().map(|k| k.1).collect::<Vec<_>>(), vec![KEY_FREE, KEY_PREMIUM], "free key first");
        assert_eq!((r.gold, p.gold), (50, 70), "the third ticket costs 50 gold");
        assert!(p.items.iter().all(|i| i.dict_id != KEY_FREE && i.dict_id != KEY_PREMIUM), "used up stacks are gone");
        // gold short: Err
        assert_eq!(take(&mut p, &d, &[Cost::Ticket, Cost::Gold(50)]), Err(Short));
        // give back restores keys and gold
        let mut q = Profile::starter();
        give_back(&mut q, &r);
        assert_eq!(q.items.iter().filter(|i| i.dict_id == KEY_FREE || i.dict_id == KEY_PREMIUM).count(), 2);
        assert_eq!(q.gold, Profile::starter().gold + 50);
    }

    fn charges(a: AccountId) -> Vec<Charge> {
        vec![Charge { payer: a, for_whom: a, cost: Cost::Ticket }]
    }

    #[test]
    fn three_players_fill_one_match() {
        let mut q = PveQueue::default();
        q.enqueue(vec![1], 15, 0, 3, charges(1), 10).unwrap();
        assert!(q.next_group().is_empty());
        q.enqueue(vec![2], 15, 1, 3, charges(2), 11).unwrap();
        q.enqueue(vec![3], 15, 0, 3, charges(3), 12).unwrap();
        assert!(q.next_group().is_empty(), "another difficulty does not join");
        assert!(q.enqueue(vec![3], 15, 0, 3, charges(3), 13).is_none(), "one search per account");
        q.enqueue(vec![4], 15, 0, 3, charges(4), 14).unwrap();
        let g = q.next_group();
        assert_eq!(g.iter().flat_map(|t| t.accounts.clone()).collect::<Vec<_>>(), vec![1, 3, 4]);
        assert_eq!(q.remove_account(3).unwrap().accounts, vec![3]);
        assert!(q.next_group().is_empty());
    }

    #[test]
    fn a_held_group_lets_the_groups_behind_it_start() {
        let mut q = PveQueue::default();
        // difficulty 0: 1, 2, 3 (complete); difficulty 1: 4, 5, 6 (complete), joined later
        for (a, d, t) in [(1, 0, 10), (2, 0, 11), (3, 0, 12), (4, 1, 13), (5, 1, 14), (6, 1, 15)] {
            q.enqueue(vec![a], 15, d, 3, charges(a), t).unwrap();
        }
        let first = q.next_group();
        assert_eq!(first.iter().map(|t| t.accounts[0]).collect::<Vec<_>>(), vec![1, 2, 3]);
        // the first group is held (its mazes have no room): skipping its tickets, the next complete group comes
        let held: Vec<u32> = first.iter().map(|t| t.order).collect();
        assert_eq!(q.next_group_without(&held).iter().map(|t| t.accounts[0]).collect::<Vec<_>>(), vec![4, 5, 6]);
        // a held ticket is not taken into another group either; with both held nothing is left
        let all: Vec<u32> = (1..=6).collect();
        assert!(q.next_group_without(&all).is_empty());
        assert_eq!(q.next_group_without(&[]), first, "without skips it is next_group");
        assert_eq!(q.len(), 6, "nothing leaves the queue by being skipped");
        // the loaded mazes take turns: the last start of each project
        assert_eq!(q.maze_last("pve_01_v1_maze1"), 0);
        q.note_maze("pve_01_v1_maze1");
        q.note_maze("pve_01_v1_maze2");
        assert!(q.maze_last("pve_01_v1_maze2") > q.maze_last("pve_01_v1_maze1"));
    }

    #[test]
    fn squads_stay_together_and_locks_lower_the_size() {
        let mut q = PveQueue::default();
        // a squad of 2 with a locked slot wants 2: complete alone
        q.enqueue(vec![1, 2], 15, 2, 2, vec![], 1).unwrap();
        assert_eq!(q.next_group().len(), 1);
        q.remove_account(1).unwrap();
        // a squad of 2 (open slot) waits for one solo; two solos are not enough for it (2 + 1 = 3)
        q.enqueue(vec![1, 2], 15, 2, 3, vec![], 1).unwrap();
        assert!(q.next_group().is_empty());
        q.enqueue(vec![5], 15, 2, 3, vec![], 2).unwrap();
        let g = q.next_group();
        assert_eq!(g.len(), 2);
        // a solo that wants only 2 cannot join a squad of 2 + 1 = 3 players
        q.remove_account(5).unwrap();
        q.enqueue(vec![6], 15, 2, 2, vec![], 3).unwrap();
        assert!(q.next_group().is_empty(), "squad of 2 + solo that wants 2 -> 3 > 2");
        // the oldest tickets cannot fill, a later leader can: 6 and 7 both want 2
        q.enqueue(vec![7], 15, 2, 2, vec![], 4).unwrap();
        let g = q.next_group();
        assert_eq!(g.iter().flat_map(|t| t.accounts.clone()).collect::<Vec<_>>(), vec![6, 7]);
    }

    #[test]
    fn a_new_daily_key_comes_with_a_popup_of_one_unit() {
        game_data::require_tables!();
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("k.sqlite")).unwrap();
        store.set_hash_cost(accounts::HashCost::FAST_FOR_TESTS);
        let gd = game_data::testing::tables();
        let a = store.create_account("amy", "pw").unwrap();
        assert!(key_messages(&store, &gd, a).is_empty(), "no key row yet");
        assert!(win_key(&store, &gd, a, 20_000 * 86_400));
        assert!(win_key(&store, &gd, a, 20_000 * 86_400 + 1));
        let uid = store.load_profile(a).unwrap().items.iter().find(|i| i.dict_id == KEY_FREE).unwrap().uid;
        match &key_messages(&store, &gd, a)[..] {
            [S2c::Reply(Reply::Shop(Shop46::Buy { flag: 1, items }))] => {
                assert_eq!(items.len(), 1);
                assert_eq!((items[0].uid, items[0].dict, items[0].value), (uid, KEY_FREE, 1));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn two_free_keys_per_day() {
        game_data::require_tables!();
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("k.sqlite")).unwrap();
        store.set_hash_cost(accounts::HashCost::FAST_FOR_TESTS);
        let gd = game_data::testing::tables();
        let a = store.create_account("amy", "pw").unwrap();
        let keys = |s: &Store| s.load_profile(a).unwrap().items.iter().filter(|i| i.dict_id == KEY_FREE).map(|i| i.count).sum::<u32>();
        let day = 20_000 * 86_400 + 5;
        assert!(win_key(&store, &gd, a, day));
        assert!(win_key(&store, &gd, a, day + 100));
        assert!(!win_key(&store, &gd, a, day + 200), "the third win of the day gives nothing");
        assert_eq!(keys(&store), 2);
        assert!(win_key(&store, &gd, a, day + 86_400), "a new day");
        assert_eq!(keys(&store), 3);
    }
}
