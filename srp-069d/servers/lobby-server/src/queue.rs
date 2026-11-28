//! The PvP queue (`2F` / `31`) and the estimated waiting time (`83 2F`).
//!
//! PROVEN client side (docs/notes/progression.md section 3): `2F u32 server, u8 type (2 = rated),
//! u32 game-mode mask, u8` starts a search; the client then waits for `85`: state 1 / 4 = `u32 order, u32, u32` (the
//! search is running, the Play button turns into "leave", cancel = `31 u32 order`), state 0 = idle again. `83 2F u32 a,
//! u32 b` carries the estimate: `b` goes to `root.set_mm_estimated_time` (seconds; 0 hides the label, LIKELY).
//!
//! State kept here (real, per lobby): one [`Ticket`] per account or squad (accounts, mode mask, rated flag, the unix
//! second it joined), plus two logs the estimate reads: the recent joins and the recent match starts with the time the
//! players waited. Nothing is a constant.
//!
//! Estimate (documented formula, `PvpQueue::estimate`), for a ticket with mask M:
//! 1. when matches were started for overlapping masks, the mean waiting time of the last [`RECENT_STARTS`] of them
//!    (what players really waited);
//! 2. otherwise, from the queue: `players_needed = min_players` of the PvP rows of the game data minus the players
//!    waiting with an overlapping mask, divided by the arrival rate of those players over the last [`WINDOW_SECS`]
//!    seconds (`(joined players - the first join) / span`, the span at least [`MIN_SPAN_SECS`]); no estimate (0) while
//!    fewer than two players ever arrived (no rate) or nobody is missing.
//!
//! Starting a match: with a match server hook that serves rows of the data, a full group becomes
//! an online match (`online::queue_start`, every member gets `82`). Otherwise, when enough players wait, the group is
//! taken out of the queue and every member gets `85 00` (refused): the client's idle state. The log says
//! so. `online_queue` (a test-only override) pins every queue match to one row.
//!
//! The `2F` mode mask (docs/notes/online.md section 7): bit `i` = mode id `i` (the lobby movie's mode check boxes, one per
//! bit 0..6, and the three bits 7..9 that have no check box and are always set). [`modes_of_mask`] turns it into mode ids,
//! [`PvpQueue::group_for_kinds`] forms a group whose tickets share at least one playable mode, [`PvpQueue::choose`] gives
//! its mode (ticked; ranked: the 10-minute slot) and map (rotation, CHOICE) among the pool's rows the match server serves,
//! and [`PvpQueue::pick`] draws the variation and weather among that map's rows. Under the world host's limits
//! ([`crate::host_room`]) [`PvpQueue::pick_in`] keeps the same rotation over the rows the host has room for: every served
//! row while a new map can load, else the loaded maps that take one more match, else none (the group stays queued).

use std::collections::{BTreeMap, VecDeque};

use accounts::AccountId;
use protocol::s2c::*;

use crate::dispatch::Ctx;
use crate::host_room::Scope;
use crate::peer::{Cross, Peer};

/// How far back the arrival rate looks.
pub const WINDOW_SECS: i64 = 600;
/// A rate over a shorter span than this is not trusted (two joins in the same second would give an infinite rate).
pub const MIN_SPAN_SECS: i64 = 60;
/// Matches counted in the mean waiting time.
pub const RECENT_STARTS: usize = 10;

/// One search: a single player or a whole squad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// order id the client keeps (`85` state 1) and names in `31`
    pub order: u32,
    pub accounts: Vec<AccountId>,
    /// game-mode mask of `2F`
    pub mask: u32,
    pub rated: bool,
    /// unix second the search began
    pub since: i64,
}

fn overlap(a: u32, b: u32) -> bool {
    a == 0 || b == 0 || a & b != 0
}

/// Mode bits of the `2F` mask the client can send: ten, bit `i` = mode id `i` (the exe masks the saved mask with 0x3FF,
/// 0x2001782F5).
pub const MASK_BITS: u32 = 10;
/// The "event" check box (paintball, bit 4): while it is selected the client sends this bit ALONE (0x200185F37..0x200185F49).
pub const EVENT_BIT: u32 = 1 << 4;
/// Bits that have no check box in the lobby movie (7, 8, 9): the exe never clears them, so every client's mask carries them.
pub const NO_BUTTON_BITS: u32 = 0x380;

/// The mode ids a `2F` mask selects, ascending; a mask of 0 selects every mode (nothing is known about the player).
pub fn modes_of_mask(mask: u32) -> Vec<u32> {
    (0..MASK_BITS).filter(|i| mask == 0 || mask & (1 << i) != 0).collect()
}

/// The mask two searches can share: wildcard (0) adapts to the other, otherwise the AND (0 = nothing in common).
fn narrow(a: u32, b: u32) -> Option<u32> {
    match (a, b) {
        (0, x) | (x, 0) => Some(x),
        (x, y) if x & y != 0 => Some(x & y),
        _ => None,
    }
}

/// The modes a queue search can ask for: the lobby movie's check boxes, bits 0..6 (`_modeButtons`, online.md 7.1). The
/// bits 7 (pve), 8 (dom) and 9 (ah) have no check box: the exe never clears them, so they say nothing about the player,
/// and artefact hunt was "only in custom matches" in 0.69d (OFFICIAL, online.md "Web evidence"). A mask of 0 (nothing
/// stated) means any of these.
pub const QUEUE_MODE_BITS: u32 = 0x7F;

/// Ranked matches: the mode is not the players' choice but a fixed loop of 10-minute slots from 00:00 UTC, Battery ->
/// TDM -> Research -> Search and Destroy (OFFICIAL 2020-07-05 "Mode rotation logic in Leagues", still published for the
/// 2021-09 season). Mode ids: br 0, tdm 1, ctp ("Research") 2, sad 5.
pub const RANKED_ROTATION: [u32; 4] = [0, 1, 2, SAD_MODE];
/// Search and Destroy's mode id (the exe's mode table 0x200987060, index 5 `sad`).
pub const SAD_MODE: u32 = 5;
/// Length of one ranked mode slot.
pub const RANKED_SLOT_SECS: i64 = 600;

/// The ranked map list, map id -> modes, "all locations use default variations" (OFFICIAL season posts 2020-03 and
/// 2021-09; map ids from the client's `st_level_NN_name`): Rudnya (br,
/// ctp), School (tdm, br, ctp, sad), Vostok Radar Station (tdm, br, ctp, sad), Chemical Plant (br, ctp), Tarakanovsky Fort
/// (tdm, br, ctp), Vector Laboratory (tdm, br, ctp), Cologne Bridge (tdm, br), London (tdm, br, ctp), Abandoned base (sad;
/// map id 12: its mode file `sad_abandoned_base` and the client's custom-match list `st_sad_abandoned_base_name` id 12).
/// No Mamayev Kurgan, CNPP Cooling Tower or Duga in ranked.
pub const RANKED_MAPS: [(u32, &[u32]); 9] =
    [(1, &[0, 2]), (2, &[1, 0, 2, 5]), (3, &[1, 0, 2, 5]), (4, &[0, 2]), (5, &[1, 0, 2]), (7, &[1, 0, 2]), (8, &[1, 0]), (9, &[1, 0, 2]), (12, &[5])];

/// (map, mode) pairs out of the pool in every queue kind: "disabled in all game modes, including ranked matches: Rudnya,
/// Battery Retrieval; ... Cologne Bridge, Research" (OFFICIAL 2021-11-17, never re-enabled). The same post's School (v2)
/// Research, London (v3) Battery and London (v3) Research are not applied: London has no fourth variation in the data,
/// so the post's numbering is not the projects' `_vN` (OPEN).
/// Arms Race (gun race, mode 3; docs/notes/gun-race.md) in the unrated pool, hence the `AC` mask bit 3 (login.rs
/// `play_gate`): a match with the race kits runs to arena.lua's end in the world host.
/// Never ranked (not in [`RANKED_MAPS`]) and never bot-filled (arena.lua spawns humans only: online.rs `NO_BOT_MODES`).
pub const GUNRACE_OPEN: bool = true;

pub const DISABLED: [(u32, u32); 2] = [(1, 0), (8, 2)];

/// The ranked slot mode at `now_unix` ([`RANKED_ROTATION`]).
pub fn ranked_mode(now_unix: i64) -> u32 {
    ranked_order(now_unix)[0]
}

/// The ranked modes in fall-through order at `now_unix`: the slot's mode, then the modes of the following slots
/// (rotation order, wrapping: in the Search and Destroy slot Battery -> TDM -> Research). CHOICE: a slot whose mode cannot
/// form a match (no ranked row served, or the group does not split into its teams: `online::queue_start`) falls
/// through to the next mode of this list that can, instead of the search waiting silently until the next slot. With
/// every ranked mode served the slot's own mode is always first and the order is never used.
pub fn ranked_order(now_unix: i64) -> [u32; RANKED_ROTATION.len()] {
    let n = RANKED_ROTATION.len();
    let slot = ((now_unix.rem_euclid(86_400) / RANKED_SLOT_SECS) % n as i64) as usize;
    std::array::from_fn(|k| RANKED_ROTATION[(slot + k) % n])
}

/// The ranked ("default") variation of a map in a mode: 0 for the classic modes ("all locations use default
/// variations", [`RANKED_MAPS`]). Search and Destroy's variation ids start at 5: the client's custom-match list labels
/// id 5 `st_variation_0_name` on Vostok (3) and Abandoned base (12) (PROVEN), so 5 there.
/// School (2) lists only id 7 (`st_variation_1_name`; its ids 5 and 6 ship but the client does not offer them), and
/// School is on the official ranked S&D list: CHOICE, School's ranked S&D layout is 7, the one the client offers.
pub fn default_variation(map: u32, mode: u32) -> u32 {
    match (map, mode) {
        (2, SAD_MODE) => 7,
        (_, SAD_MODE) => 5,
        _ => 0,
    }
}

/// A row of the data a queue match can be played on: map, mode, variation (the layout, `_vN`) and weather (1 day, 2 rain,
/// 3 evening); together they name one level project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pick {
    pub map: u32,
    pub mode: u32,
    /// the row's variation id, the third number of `5B 01` / `resolve_map`
    pub variation: u32,
    /// the row's weather id (1 day, 2 rain, 3 evening)
    pub weather: u32,
}

impl Pick {
    pub fn of(r: &game_data::ModeRow) -> Pick {
        Pick { map: r.map, mode: r.mode, variation: r.variation, weather: r.weather }
    }

    /// In the pool of a queue kind: not [`DISABLED`]; ranked only on [`RANKED_MAPS`] at the default variation
    /// ([`default_variation`]: 0, Search and Destroy 5 / School 7).
    pub fn in_pool(&self, rated: bool) -> bool {
        if DISABLED.contains(&(self.map, self.mode)) {
            return false;
        }
        if self.mode == game_data::gunrace::MODE && !GUNRACE_OPEN {
            return false;
        }
        !rated || (self.variation == default_variation(self.map, self.mode) && RANKED_MAPS.iter().any(|(m, modes)| *m == self.map && modes.contains(&self.mode)))
    }
}

/// What the waiting searches form ([`PvpQueue::group_in`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Forms {
    /// a group and the mode mask its tickets share
    Group(Vec<Ticket>, u32),
    /// nobody can play together yet: keep searching
    Wait,
    /// a group would form on the served rows, but the world host has room for none of its rows now: it stays queued
    /// (its shared mask, the key of the hold's log line)
    Held(u32),
}

#[derive(Debug, Default)]
pub struct PvpQueue {
    next_order: u32,
    tickets: Vec<Ticket>,
    /// (unix second, mask, rated, players) of every join inside the window
    joins: VecDeque<(i64, u32, bool, u32)>,
    /// (mask, rated, seconds waited) of the last started matches, newest last
    starts: VecDeque<(u32, bool, i64)>,
    /// rotation memory: the sequence number of the last match started on a mode / a map (0 = never)
    seq: u64,
    mode_seq: BTreeMap<u32, u64>,
    map_seq: BTreeMap<u32, u64>,
    /// the same per row (the variant drawn among loaded maps: [`pick_in`](Self::pick_in))
    row_seq: BTreeMap<Pick, u64>,
    /// the ranked mode slot ([`RANKED_SLOT_SECS`]) [`ranked_slot_changed`](Self::ranked_slot_changed) saw last
    ranked_slot: i64,
}

impl PvpQueue {
    /// Put `accounts` into the queue; None when one of them already waits or the list is empty.
    pub fn enqueue(&mut self, accounts: Vec<AccountId>, mask: u32, rated: bool, now: i64) -> Option<Ticket> {
        if accounts.is_empty() || accounts.iter().any(|a| self.ticket_of(*a).is_some()) {
            return None;
        }
        self.next_order += 1;
        let t = Ticket { order: self.next_order, accounts, mask, rated, since: now };
        self.joins.push_back((now, mask, rated, t.accounts.len() as u32));
        while self.joins.front().is_some_and(|j| j.0 < now - WINDOW_SECS) {
            self.joins.pop_front();
        }
        self.tickets.push(t.clone());
        Some(t)
    }

    pub fn ticket_of(&self, a: AccountId) -> Option<&Ticket> {
        self.tickets.iter().find(|t| t.accounts.contains(&a))
    }

    /// Take the search of `a` (and of everyone in its ticket) out of the queue.
    pub fn remove_account(&mut self, a: AccountId) -> Option<Ticket> {
        let i = self.tickets.iter().position(|t| t.accounts.contains(&a))?;
        Some(self.tickets.remove(i))
    }

    /// Players waiting whose search overlaps `mask` (same rated kind).
    /// 1-based position of ticket `order` among the waiting tickets of its kind (rated or not, overlapping modes), oldest
    /// first; 0 when it is not queued.
    pub fn place_of(&self, order: u32) -> u32 {
        let Some(me) = self.tickets.iter().find(|t| t.order == order) else { return 0 };
        self.tickets.iter().filter(|t| t.rated == me.rated && overlap(t.mask, me.mask)).position(|t| t.order == order).map_or(0, |i| i as u32 + 1)
    }

    pub fn waiting(&self, mask: u32, rated: bool) -> u32 {
        self.tickets.iter().filter(|t| t.rated == rated && overlap(t.mask, mask)).map(|t| t.accounts.len() as u32).sum()
    }

    /// The unix second the oldest waiting search of this kind began.
    pub fn oldest_since(&self, rated: bool) -> Option<i64> {
        self.tickets.iter().filter(|t| t.rated == rated).map(|t| t.since).min()
    }

    pub fn len(&self) -> usize {
        self.tickets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tickets.is_empty()
    }

    /// The tickets that would form a match with the oldest waiting ticket: every ticket overlapping it, oldest first,
    /// when they hold at least `needed` players. Empty otherwise.
    pub fn group(&self, needed: u32) -> Vec<Ticket> {
        self.group_for(needed, |_| true).map(|g| g.0).unwrap_or_default()
    }

    /// Like [`group`](Self::group) but the whole group must share a mode mask that `playable` accepts, and every ticket
    /// can be the seed (oldest first): the first seed whose group holds at least `needed` players wins. A ticket joins
    /// when the mask it shares with the group so far is non-empty and still playable (the shared mask only narrows).
    /// Returns the tickets and the shared mask (0 = nobody stated a mask).
    pub fn group_for(&self, needed: u32, playable: impl Fn(u32) -> bool) -> Option<(Vec<Ticket>, u32)> {
        self.group_where(needed, |_| true, playable)
    }

    /// [`group_for`](Self::group_for) over the unrated searches only: a bot-filled match never takes a ranked search
    /// (CHOICE); a ranked ticket neither seeds nor joins (a group shares its seed's kind).
    pub fn group_for_unrated(&self, needed: u32, playable: impl Fn(u32) -> bool) -> Option<(Vec<Ticket>, u32)> {
        self.group_where(needed, |t| !t.rated, playable)
    }

    fn group_where(&self, needed: u32, seed_ok: impl Fn(&Ticket) -> bool, playable: impl Fn(u32) -> bool) -> Option<(Vec<Ticket>, u32)> {
        self.group_kinds(needed, seed_ok, |m, _| playable(m))
    }

    fn group_kinds(&self, needed: u32, seed_ok: impl Fn(&Ticket) -> bool, playable: impl Fn(u32, bool) -> bool) -> Option<(Vec<Ticket>, u32)> {
        if needed == 0 {
            return None;
        }
        for seed in self.tickets.iter().filter(|t| seed_ok(t)) {
            let mut mask = seed.mask;
            let playable = |m: u32| playable(m, seed.rated);
            if !playable(mask) {
                continue;
            }
            let mut g = vec![seed.clone()];
            for t in self.tickets.iter().filter(|t| t.order != seed.order && t.rated == seed.rated) {
                // a ranked match's mode is the slot's ([`PvpQueue::choose`]): the masks need not overlap
                let shared = if seed.rated { Some(mask) } else { narrow(mask, t.mask) };
                if let Some(m) = shared.filter(|m| playable(*m)) {
                    mask = m;
                    g.push(t.clone());
                }
            }
            if g.iter().map(|t| t.accounts.len() as u32).sum::<u32>() >= needed {
                return Some((g, mask));
            }
        }
        None
    }

    /// Like [`group_for`](Self::group_for), but `playable` also gets the seed's kind (rated or not): a ranked group's
    /// mode is the slot's, not the players' mask ([`pick`](Self::pick)).
    pub fn group_for_kinds(&self, needed: u32, playable: impl Fn(u32, bool) -> bool) -> Option<(Vec<Ticket>, u32)> {
        self.group_kinds(needed, |_| true, playable)
    }

    /// The mode and map of a group with shared `mask` and kind `rated` among the `rows` the match server can serve
    /// (the caller's list), before the variation / weather draw of [`pick`](Self::pick):
    /// * the pool: [`Pick::in_pool`] (the disabled pairs; ranked: its map list at the default variation);
    /// * the mode: ranked = the slot's ([`ranked_mode`], the mask is not asked; when the pool has no ranked row of it,
    ///   the next mode of [`ranked_order`] that has one, CHOICE); otherwise one the players ticked
    ///   ([`QUEUE_MODE_BITS`] of the shared mask), the one whose last queue match is the oldest (never played first, then
    ///   the lowest id). CHOICE: the original's choice among several ticked modes is not known;
    /// * the map on it: the one whose last queue match is the oldest (then the lowest id). CHOICE: how the original picked
    ///   the map inside a mode is NOT FOUND (no source found); the rotation spreads the maps.
    pub fn choose(&self, mask: u32, rated: bool, rows: &[Pick], now_unix: i64) -> Option<(u32, u32)> {
        self.choose_with(mask, rated, rows, now_unix, |_| 0)
    }

    /// [`choose`](Self::choose) with the ties of the rotation (modes or maps whose last queue match is equally old, e.g.
    /// every one after a server start) broken by `tie(n)` (in `0..n`) instead of the lowest id. CHOICE: the rotation
    /// lives in memory, so without a random tie the first match after every start would be the lowest mode and map.
    pub fn choose_with(&self, mask: u32, rated: bool, rows: &[Pick], now_unix: i64, mut tie: impl FnMut(u32) -> u32) -> Option<(u32, u32)> {
        let seq_of = |m: &BTreeMap<u32, u64>, k: u32| m.get(&k).copied().unwrap_or(0);
        // the oldest of `keys` by `seqs`, ties drawn by `tie` among them in id order
        let mut oldest = |keys: Vec<u32>, seqs: &BTreeMap<u32, u64>| -> Option<u32> {
            let min = keys.iter().map(|k| seq_of(seqs, *k)).min()?;
            let mut ties: Vec<u32> = keys.into_iter().filter(|k| seq_of(seqs, *k) == min).collect();
            ties.sort_unstable();
            ties.dedup();
            let i = tie(ties.len() as u32) as usize;
            ties.get(i.min(ties.len() - 1)).copied()
        };
        let pool: Vec<&Pick> = rows.iter().filter(|r| r.in_pool(rated)).collect();
        let mode = if rated {
            // the slot's mode, else the next rotation mode with a ranked row ([`ranked_order`], CHOICE)
            ranked_order(now_unix).into_iter().find(|m| pool.iter().any(|r| r.mode == *m))?
        } else {
            let wanted = if mask == 0 { QUEUE_MODE_BITS } else { mask & QUEUE_MODE_BITS };
            oldest(pool.iter().map(|r| r.mode).filter(|m| *m < 32 && wanted & (1 << m) != 0).collect(), &self.mode_seq)?
        };
        let map = oldest(pool.iter().filter(|r| r.mode == mode).map(|r| r.map).collect(), &self.map_seq)?;
        Some((mode, map))
    }

    /// The match of a group: [`choose`](Self::choose)'s mode and map, then one of their rows in the pool drawn uniformly by
    /// `draw(n)` (in `0..n`): the variation "is selected" per match (OFFICIAL 0.46; random per a fan summary), and the
    /// weather with it (every weather of a variation is a row of the data; nothing weights them: CHOICE).
    pub fn pick(&self, mask: u32, rated: bool, rows: &[Pick], now_unix: i64, draw: impl FnOnce(u32) -> u32) -> Option<Pick> {
        self.pick_with(mask, rated, rows, now_unix, |_| 0, draw)
    }

    /// [`pick`](Self::pick) with the rotation's ties broken by `draw` too ([`choose_with`](Self::choose_with)): the
    /// queue's matches use it with a random draw, which then also draws the row.
    pub fn pick_random(&self, mask: u32, rated: bool, rows: &[Pick], now_unix: i64, mut draw: impl FnMut(u32) -> u32) -> Option<Pick> {
        let (mode, map) = self.choose_with(mask, rated, rows, now_unix, &mut draw)?;
        self.pick_row(mode, map, rated, rows, draw)
    }

    fn pick_with(&self, mask: u32, rated: bool, rows: &[Pick], now_unix: i64, tie: impl FnMut(u32) -> u32, draw: impl FnOnce(u32) -> u32) -> Option<Pick> {
        let (mode, map) = self.choose_with(mask, rated, rows, now_unix, tie)?;
        self.pick_row(mode, map, rated, rows, draw)
    }

    /// One of the pool's rows of `mode` on `map`, drawn by `draw(n)`.
    fn pick_row(&self, mode: u32, map: u32, rated: bool, rows: &[Pick], draw: impl FnOnce(u32) -> u32) -> Option<Pick> {
        let mut cands: Vec<Pick> = rows.iter().filter(|r| r.mode == mode && r.map == map && r.in_pool(rated)).copied().collect();
        cands.sort_unstable();
        cands.dedup();
        let i = draw(cands.len() as u32) as usize;
        cands.get(i.min(cands.len().saturating_sub(1))).copied()
    }

    /// True once per new ranked mode slot while a ranked search waits: its group may have become playable (the caller
    /// runs the queue start again).
    pub fn ranked_slot_changed(&mut self, now_unix: i64) -> bool {
        let slot = now_unix.div_euclid(RANKED_SLOT_SECS);
        if slot == self.ranked_slot || !self.tickets.iter().any(|t| t.rated) {
            return false;
        }
        self.ranked_slot = slot;
        true
    }

    /// Remember that a match started on `pick` (feeds [`choose`](Self::choose)).
    pub fn note_pick(&mut self, pick: Pick) {
        self.seq += 1;
        self.mode_seq.insert(pick.mode, self.seq);
        self.map_seq.insert(pick.map, self.seq);
        self.row_seq.insert(pick, self.seq);
    }

    /// The sequence number of the last queue match on `pick`'s map and on the row itself (0 = never), for the log.
    pub fn last_start(&self, pick: &Pick) -> (u64, u64) {
        (self.map_seq.get(&pick.map).copied().unwrap_or(0), self.row_seq.get(pick).copied().unwrap_or(0))
    }

    /// [`choose_with`](Self::choose_with) within the world host's room ([`Scope`]):
    /// * [`Scope::Rotation`]: the rotation over its rows, unchanged; the idle maps the next load evicts (`avoid`) only
    ///   when no other row fits the group;
    /// * [`Scope::Loaded`]: the same rotation over the loaded rows only (the mode, then the map whose last queue match is
    ///   the oldest, a never started one first): round robin over them, never the same one twice while another waits.
    ///   The `avoid` rows (idle maps, the drained map) are left out while another row fits the group, and taken when none
    ///   does;
    /// * [`Scope::Held`] gives none.
    ///
    /// Ranked takes its slot mode from every served row as always (the fall-through to the next slot's mode only when
    /// the slot's mode has no served row at all) and then only the scope's rows of that mode: when every map of the slot
    /// mode is full or not loaded, none (the group is held, it does not move to another mode).
    pub fn choose_in(&self, mask: u32, rated: bool, scope: &Scope, now_unix: i64, mut tie: impl FnMut(u32) -> u32) -> Option<(u32, u32)> {
        let over = |served: &[Pick], rows: &[Pick], tie: &mut dyn FnMut(u32) -> u32| -> Option<(u32, u32)> {
            if !rated {
                return self.choose_with(mask, false, rows, now_unix, tie);
            }
            let (mode, _) = self.choose_with(mask, true, served, now_unix, |_| 0)?;
            let of_mode: Vec<Pick> = rows.iter().filter(|r| r.mode == mode).copied().collect();
            self.choose_with(mask, true, &of_mode, now_unix, tie)
        };
        let (served, rows, avoid) = match scope {
            Scope::Rotation { served, rows, avoid } => (served, rows, avoid),
            Scope::Loaded { served, open, avoid } => (served, open, avoid),
            Scope::Held => return None,
        };
        if avoid.is_empty() {
            return over(served, rows, &mut tie);
        }
        let others: Vec<Pick> = rows.iter().filter(|r| !avoid.contains(r)).copied().collect();
        over(served, &others, &mut tie).or_else(|| over(served, rows, &mut tie))
    }

    /// The match of a group within the world host's room: [`choose_in`](Self::choose_in)'s mode and map, then of their
    /// rows the ones not avoided when there are any, and of those [`Scope::Rotation`]: one drawn by `draw`, as
    /// [`pick_random`](Self::pick_random) always did; [`Scope::Loaded`]: the one started the longest ago (ties drawn by
    /// `draw`), so the loaded variants take turns too.
    pub fn pick_in(&self, mask: u32, rated: bool, scope: &Scope, now_unix: i64, mut draw: impl FnMut(u32) -> u32) -> Option<Pick> {
        let (mode, map) = self.choose_in(mask, rated, scope, now_unix, &mut draw)?;
        let preferred = |rows: &[Pick], avoid: &[Pick]| -> Vec<Pick> {
            let all: Vec<Pick> = rows.iter().filter(|r| r.mode == mode && r.map == map && r.in_pool(rated)).copied().collect();
            let not_avoided: Vec<Pick> = all.iter().filter(|r| !avoid.contains(r)).copied().collect();
            if not_avoided.is_empty() { all } else { not_avoided }
        };
        let mut cands = match scope {
            Scope::Rotation { rows, avoid, .. } => return self.pick_row(mode, map, rated, &preferred(rows, avoid), draw),
            Scope::Loaded { open, avoid, .. } => preferred(open, avoid),
            Scope::Held => return None,
        };
        let seq = |r: &Pick| self.row_seq.get(r).copied().unwrap_or(0);
        let min = cands.iter().map(seq).min()?;
        cands.retain(|r| seq(r) == min);
        cands.sort_unstable();
        cands.dedup();
        let i = draw(cands.len() as u32) as usize;
        cands.get(i.min(cands.len() - 1)).copied()
    }

    /// What the waiting searches form within the world host's room ([`Scope`]): a group (as
    /// [`group_for_kinds`](Self::group_for_kinds) / [`group_for_unrated`](Self::group_for_unrated) with
    /// [`choose_in`](Self::choose_in) as the playable test); else, when the scope left rows out and a group would form on
    /// every `served` row, [`Forms::Held`] (the room holds it); else [`Forms::Wait`] (nobody can play together yet).
    pub fn group_in(&self, needed: u32, unrated_only: bool, served: &[Pick], scope: &Scope, now_unix: i64) -> Forms {
        let group = |playable: &dyn Fn(u32, bool) -> bool| if unrated_only { self.group_for_unrated(needed, |m| playable(m, false)) } else { self.group_for_kinds(needed, playable) };
        if let Some((g, mask)) = group(&|m: u32, rated: bool| self.choose_in(m, rated, scope, now_unix, |_| 0).is_some()) {
            return Forms::Group(g, mask);
        }
        let narrowed = scope.candidates().len() < served.len();
        match group(&|m: u32, rated: bool| self.choose(m, rated, served, now_unix).is_some()) {
            Some((_, mask)) if narrowed => Forms::Held(mask),
            _ => Forms::Wait,
        }
    }

    /// A match was started for these tickets (the hook of the match server): their waiting time feeds the estimate.
    pub fn record_start(&mut self, group: &[Ticket], now: i64) {
        for t in group {
            self.starts.push_back((t.mask, t.rated, (now - t.since).max(0)));
            if let Some(i) = self.tickets.iter().position(|x| x.order == t.order) {
                self.tickets.remove(i);
            }
        }
        while self.starts.len() > RECENT_STARTS {
            self.starts.pop_front();
        }
    }

    /// Estimated seconds a search with `mask` still takes (0 = no estimate). See the module docs for the formula.
    pub fn estimate(&self, mask: u32, rated: bool, needed: u32, now: i64) -> u32 {
        let waits: Vec<i64> = self.starts.iter().filter(|s| s.1 == rated && overlap(s.0, mask)).map(|s| s.2).collect();
        if !waits.is_empty() {
            return (waits.iter().sum::<i64>() / waits.len() as i64).clamp(0, u32::MAX as i64) as u32;
        }
        let waiting = self.waiting(mask, rated);
        if needed == 0 || waiting >= needed {
            return 0;
        }
        let arrivals: Vec<&(i64, u32, bool, u32)> =
            self.joins.iter().filter(|j| j.0 >= now - WINDOW_SECS && j.2 == rated && overlap(j.1, mask)).collect();
        let players: u32 = arrivals.iter().map(|j| j.3).sum();
        let (Some(first), Some(last)) = (arrivals.first(), arrivals.last()) else { return 0 };
        if players < 2 {
            return 0;
        }
        // the first arrival only starts the clock: the rate is the players that came after it per second of the span
        let span = (last.0 - first.0).max(MIN_SPAN_SECS);
        let after_first = players - first.3;
        if after_first == 0 {
            return 0;
        }
        let missing = (needed - waiting) as f64;
        (missing * span as f64 / after_first as f64).ceil().min(u32::MAX as f64) as u32
    }
}

/// Smallest `min_players` of a PvP row of the game data (the estimate does not know which row a ticket will get).
pub fn players_needed(gd: &game_data::GameData) -> u32 {
    gd.matches.modes().iter().map(|r| r.min_players).filter(|n| *n > 0).min().unwrap_or(0)
}

fn unix_now() -> i64 {
    crate::premium::unix_now()
}

/// `85 01 u32 order, u32 place, u32 total`: searching. PROVEN (0x2001C16F0): a place of 0 shows "match
/// found" at once and latches the search UI off, so the place is the ticket's 1-based position among the tickets it can
/// play with (never 0); the total (stored only, +0x6B0D0C) is the number of players waiting for the same kind.
fn searching(q: &PvpQueue, t: &Ticket) -> S2c {
    let mut p = vec![1u8];
    p.extend_from_slice(&t.order.to_le_bytes());
    p.extend_from_slice(&q.place_of(t.order).max(1).to_le_bytes());
    p.extend_from_slice(&q.waiting(t.mask, t.rated).max(1).to_le_bytes());
    S2c::Raw { id: 0x85, payload: p }
}

fn idle() -> S2c {
    S2c::QueueState(QueueState { state: 0 })
}

/// What the client gets after `2F` / `31` for its own search: `85 01 ..` while the account waits, else `85 00`.
pub fn state_of(cx: &Ctx, account: AccountId) -> S2c {
    match cx.registry.queue.ticket_of(account) {
        Some(t) => searching(&cx.registry.queue, t),
        None => idle(),
    }
}

/// The text of a ranked search refused during an event (the generic `84` text channel shows it; no client key says it).
pub const RANKED_OFF_DURING_EVENT: &str = "st_locked_by_server";

/// Put a ticket into the queue and answer its members (`peer` is the pressing one): `83 2F`, then `85 01`; when the
/// queue now holds enough players the group is refused (no match server). Returns false when someone already waits.
pub fn join(cx: &mut Ctx, peer: &mut Peer, accounts: Vec<AccountId>, mask: u32, rated: bool) -> bool {
    let now = unix_now();
    let me = peer.account;
    // one busy rule for every way into a search or a match (busy.rs); the local match is the pressing peer's own
    if accounts.iter().any(|a| crate::busy::reason(cx, *a, Some(*a) == me && peer.local_match.is_some()).is_some()) {
        return false;
    }
    // the leaver penalty's queue block (online::penalize_leaver)
    if let Some(left) = accounts.iter().filter_map(|a| cx.lobby.store.load_profile(*a).ok()).map(|p| p.penalty.blocked_until - now).max().filter(|s| *s > 0) {
        log::info!("{}: 2F refused: queue block for {left} s more", peer.addr);
        peer.push(&S2c::Denied(crate::online::queue_block_notice(u32::try_from(left).unwrap_or(u32::MAX))));
        return false;
    }
    // "Ranked matches are unavailable during the event" (OFFICIAL 0.69b)
    if rated && cx.lobby.cfg.event_at(now).is_some_and(|s| s.active(now)) {
        log::info!("{}: 2F rated refused: the event is running", peer.addr);
        peer.push(&S2c::Denied(Denied { request: 0x2F, body: DeniedBody::Text(RANKED_OFF_DURING_EVENT.into()) }));
        return false;
    }
    let Some(t) = cx.registry.queue.enqueue(accounts, mask, rated, now) else { return false };
    let needed = players_needed(&cx.lobby.data);
    let eta = cx.registry.queue.estimate(mask, rated, needed, now);
    log::info!(
        "{}: 2F queue: order {} mask {mask:#x} rated {rated} players {:?}; waiting {}/{needed}, estimate {eta} s",
        peer.addr,
        t.order,
        t.accounts,
        cx.registry.queue.waiting(mask, rated)
    );
    peer.reply(Reply::Queue { a: t.order, eta });
    for m in &t.accounts {
        if Some(*m) == me {
            peer.push(&searching(&cx.registry.queue, &t));
        } else {
            cx.fx.push(Cross { to: *m, msg: searching(&cx.registry.queue, &t) });
        }
    }
    start_if_full(cx, peer, needed, now);
    true
}

/// When enough players wait: start an online match (online.rs), keep them searching while worlds
/// exist but nobody they can play with is there, else take them out and refuse.
fn start_if_full(cx: &mut Ctx, peer: &mut Peer, needed: u32, now: i64) {
    match crate::online::queue_start(cx, now) {
        crate::online::QueueStart::Started | crate::online::QueueStart::Wait => return,
        crate::online::QueueStart::Fallback => {}
    }
    let group = cx.registry.queue.group(needed);
    if group.is_empty() {
        return;
    }
    let players: Vec<AccountId> = group.iter().flat_map(|t| t.accounts.iter().copied()).collect();
    log::info!(
        "{}: {} players wait for a match (needed {needed}): starting needs the match server, the search is refused: {players:?}",
        peer.addr,
        players.len()
    );
    for t in &group {
        cx.registry.queue.remove_account(t.accounts[0]);
    }
    let _ = now;
    for a in players {
        if Some(a) == peer.account {
            peer.push(&idle());
        } else {
            cx.fx.push(Cross { to: a, msg: idle() });
        }
    }
}

/// `2F` of a player that is in no squad.
pub fn press(cx: &mut Ctx, peer: &mut Peer, arg: u8, mask: u32) {
    let Some(me) = peer.account else { return };
    if cx.registry.queue.ticket_of(me).is_some() {
        // pressed twice: show the running search again
        let s = state_of(cx, me);
        peer.push(&s);
        return;
    }
    if !join(cx, peer, vec![me], mask, arg == 2) {
        peer.push(&idle());
    }
}

/// `31` of a player (or a squad member): the whole ticket leaves the queue, every member gets `85 00`.
pub fn cancel(cx: &mut Ctx, peer: &mut Peer) {
    let Some(me) = peer.account else { return };
    match cx.registry.queue.remove_account(me) {
        Some(t) => {
            log::info!("{}: 31 queue cancelled (order {}), {} players leave", peer.addr, t.order, t.accounts.len());
            for a in t.accounts {
                if a != me {
                    cx.fx.push(Cross { to: a, msg: idle() });
                }
            }
        }
        None => log::info!("{}: 31 with no search running", peer.addr),
    }
    peer.push(&idle());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_remove_and_waiting() {
        let mut q = PvpQueue::default();
        let a = q.enqueue(vec![1], 0x380, false, 1000).unwrap();
        assert_eq!(a.order, 1);
        assert!(q.enqueue(vec![1, 2], 0x380, false, 1001).is_none(), "an account queues once");
        let b = q.enqueue(vec![2, 3], 0x100, false, 1002).unwrap();
        assert_eq!(b.order, 2);
        assert_eq!(q.waiting(0x380, false), 3);
        assert_eq!(q.waiting(0x040, false), 0);
        assert_eq!(q.waiting(0x380, true), 0, "rated and unrated do not mix");
        assert_eq!(q.remove_account(3).unwrap().order, 2);
        assert_eq!(q.waiting(0x380, false), 1);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn estimate_from_the_arrival_rate() {
        let mut q = PvpQueue::default();
        q.enqueue(vec![1], 0x380, false, 1000).unwrap();
        assert_eq!(q.estimate(0x380, false, 12, 1000), 0, "one arrival: no rate");
        // 5 more players arrive within 100 s: 5 after the first over a 100 s span
        for (i, t) in [1020, 1040, 1060, 1080, 1100].into_iter().enumerate() {
            q.enqueue(vec![10 + i as i64], 0x380, false, t).unwrap();
        }
        assert_eq!(q.waiting(0x380, false), 6);
        // 6 missing at 5 players / 100 s = 120 s
        assert_eq!(q.estimate(0x380, false, 12, 1100), 120);
        assert_eq!(q.estimate(0x380, false, 6, 1100), 0, "nobody is missing");
        // a very short span is stretched to 60 s: 2 players in the same second
        let mut q2 = PvpQueue::default();
        q2.enqueue(vec![1], 0, false, 500).unwrap();
        q2.enqueue(vec![2], 0, false, 500).unwrap();
        assert_eq!(q2.estimate(0, false, 4, 500), 120, "2 missing at 1 player / 60 s");
        // arrivals outside the window do not count
        assert_eq!(q.estimate(0x380, false, 12, 1100 + WINDOW_SECS + 1), 0);
    }

    #[test]
    fn started_matches_give_the_mean_wait() {
        let mut q = PvpQueue::default();
        let a = q.enqueue(vec![1], 0x380, false, 1000).unwrap();
        let b = q.enqueue(vec![2], 0x380, false, 1010).unwrap();
        q.record_start(&[a, b], 1100);
        assert!(q.is_empty());
        // waited 100 and 90 s: mean 95 s, for an overlapping and for any mask, not for rated
        assert_eq!(q.estimate(0x080, false, 12, 1200), 95);
        assert_eq!(q.estimate(0, false, 12, 1200), 95);
        assert_eq!(q.estimate(0x080, true, 12, 1200), 0);
        assert_eq!(q.estimate(0x400, false, 12, 1200), 0);
    }

    #[test]
    fn the_mask_is_one_bit_per_mode_id() {
        assert_eq!(modes_of_mask(0x382), vec![1, 7, 8, 9]);
        assert_eq!(modes_of_mask(0x380), vec![7, 8, 9], "the client's mask with no check box ticked");
        assert_eq!(modes_of_mask(EVENT_BIT), vec![4]);
        assert_eq!(modes_of_mask(0).len(), MASK_BITS as usize, "no mask = any mode");
        assert_eq!(modes_of_mask(0xFFFF_FC00), Vec::<u32>::new(), "bits above 9 are not modes");
    }

    #[test]
    fn a_group_shares_a_playable_mode_and_every_ticket_can_seed() {
        let mut q = PvpQueue::default();
        q.enqueue(vec![1], 0x384, false, 10).unwrap(); // ctp (2) + the no-button bits
        q.enqueue(vec![2], 0x382, false, 11).unwrap(); // tdm (1) + the no-button bits
        q.enqueue(vec![3], 0x382, false, 12).unwrap();
        // every mode playable: the first two share only 0x380
        let (g, m) = q.group_for(2, |_| true).unwrap();
        assert_eq!((g.iter().map(|t| t.order).collect::<Vec<_>>(), m), (vec![1, 2, 3], 0x380));
        // only tdm and ctp have a world: the shared 0x380 is not playable, so ticket 1 can take neither of them; the seed 2 takes 3
        let playable = |m: u32| m & 0x006 != 0;
        let (g, m) = q.group_for(2, playable).unwrap();
        assert_eq!((g.iter().map(|t| t.order).collect::<Vec<_>>(), m), (vec![2, 3], 0x382));
        // a wildcard ticket (mask 0) joins anyone and keeps the narrowed mask
        q.enqueue(vec![4], 0, false, 13).unwrap();
        let (g, m) = q.group_for(3, playable).unwrap();
        assert_eq!((g.iter().map(|t| t.order).collect::<Vec<_>>(), m), (vec![2, 3, 4], 0x382));
        assert!(q.group_for(5, |_| true).is_none());
    }

    #[test]
    fn pick_prefers_ticked_modes_and_rotates_maps() {
        let r = |map, mode, variation, weather| Pick { map, mode, variation, weather };
        let mut q = PvpQueue::default();
        // tdm on maps 1 (variations 0 and 1, day and rain), 2 and 4, ctp on map 2, ah on map 3, br on map 1 (disabled)
        let rows = [r(1, 1, 1, 1), r(1, 1, 0, 1), r(1, 1, 0, 2), r(2, 1, 0, 1), r(4, 1, 0, 1), r(2, 2, 0, 1), r(3, 9, 0, 1), r(1, 0, 0, 1)];
        let t = 0;
        // a ticked tdm: the lowest map first; the draw picks among its three rows (sorted: v0 day, v0 rain, v1 day)
        assert_eq!(q.pick(0x382, false, &rows, t, |n| n - 1).unwrap(), r(1, 1, 1, 1));
        let p = q.pick(0x382, false, &rows, t, |n| {
            assert_eq!(n, 3, "variations and weathers of the map are the candidates");
            1
        });
        assert_eq!(p, Some(r(1, 1, 0, 2)), "the weather comes from the row");
        q.note_pick(p.unwrap());
        // same mask again: the same mode (the only ticked one), the next map that was played the longest ago
        let p = q.pick(0x382, false, &rows, t, |_| 0).unwrap();
        assert_eq!((p.map, p.mode), (2, 1));
        q.note_pick(p);
        assert_eq!(q.pick(0x382, false, &rows, t, |_| 0).unwrap().map, 4);
        // tdm and ctp ticked: the mode that was played longer ago (ctp, never played) comes first
        assert_eq!(q.pick(0x386, false, &rows, t, |_| 0).unwrap().mode, 2);
        // nothing ticked (only the always-set bits): the no-button bits (ah 9) are no queue mode
        assert_eq!(q.pick(0x380, false, &rows, t, |_| 0), None);
        // Rudnya battery is out of the pool (OFFICIAL 2021-11-17)
        assert_eq!(q.pick(0x381, false, &rows, t, |_| 0), None);
        // the event bit alone has no row; a 0 mask takes any queue mode
        assert_eq!(q.pick(EVENT_BIT, false, &rows, t, |_| 0), None);
        assert!(q.pick(0, false, &rows, t, |_| 0).is_some());
    }

    #[test]
    fn the_queue_draws_among_equally_old_modes_and_maps() {
        let r = |map, mode, variation, weather| Pick { map, mode, variation, weather };
        // tdm on maps 1, 2 and 4, ctp on map 2 and 3
        let rows = [r(1, 1, 0, 1), r(2, 1, 0, 1), r(4, 1, 0, 1), r(2, 2, 0, 1), r(3, 2, 0, 1)];
        let q = PvpQueue::default();
        // a fresh queue (every server start): nothing played, every ticked mode and map ties
        assert_eq!(q.pick_random(0x386, false, &rows, 0, |_| 0), Some(r(1, 1, 0, 1)));
        assert_eq!(q.pick_random(0x386, false, &rows, 0, |n| n - 1), Some(r(3, 2, 0, 1)));
        // the draws in order: mode (of 2 ticked), map (of that mode's), row (of 1); each sees its own count
        let scripted = |answers: [u32; 3]| {
            let mut asked = vec![];
            let p = q.pick_random(0x386, false, &rows, 0, |n| {
                asked.push(n);
                answers[asked.len() - 1]
            });
            (p, asked)
        };
        assert_eq!(scripted([0, 2, 0]), (Some(r(4, 1, 0, 1)), vec![2, 3, 1]));
        assert_eq!(scripted([1, 0, 0]), (Some(r(2, 2, 0, 1)), vec![2, 2, 1]));
        // the rotation still comes first: a played mode waits behind the never played one, whatever the draw
        let mut q = PvpQueue::default();
        q.note_pick(r(1, 1, 0, 1));
        assert_eq!(q.pick_random(0x386, false, &rows, 0, |_| 0).unwrap().mode, 2);
        // the deterministic pick keeps the lowest id on ties
        assert_eq!(PvpQueue::default().pick(0x386, false, &rows, 0, |n| n - 1), Some(r(1, 1, 0, 1)));
    }

    #[test]
    fn ranked_plays_the_slot_mode_on_its_map_list() {
        let r = |map, mode, variation, weather| Pick { map, mode, variation, weather };
        assert_eq!([0, 599, 600, 1200, 1800, 2400, 86_400 + 600].map(ranked_mode), [0, 0, 1, 2, 5, 0, 1]);
        let q = PvpQueue::default();
        // tdm on Rudnya (not a ranked tdm map) and School v0 / v1, br on Tarakanovsky Fort, ctp on Mamayev Kurgan (not ranked)
        let rows = [r(1, 1, 0, 1), r(2, 1, 0, 1), r(2, 1, 1, 1), r(5, 0, 0, 1), r(6, 2, 0, 1)];
        // 00:10 UTC = the tdm slot: School at the default variation, whatever the players ticked
        assert_eq!(q.pick(0x381, true, &rows, 600, |n| n - 1), Some(r(2, 1, 0, 1)));
        // 00:00 = battery
        assert_eq!(q.pick(0x382, true, &rows, 0, |_| 0), Some(r(5, 0, 0, 1)));
        // 00:20 = research: only Mamayev Kurgan serves ctp, not on the ranked list; 00:30 = S&D: no row. Both fall through
        // to the next rotation mode with a ranked row (S&D, then battery: ranked_order, CHOICE)
        assert_eq!(q.pick(0x384, true, &rows, 1200, |_| 0), Some(r(5, 0, 0, 1)));
        assert_eq!(q.pick(0x384, true, &rows, 1800, |_| 0), Some(r(5, 0, 0, 1)));
        // no ranked row of any rotation mode: nothing
        assert_eq!(q.pick(0x384, true, &[r(1, 1, 0, 1), r(6, 2, 0, 1)], 1800, |_| 0), None);
        // ranked tickets group even with disjoint masks
        let mut qq = PvpQueue::default();
        qq.enqueue(vec![1], 0x002, true, 10).unwrap();
        qq.enqueue(vec![2], 0x001, true, 11).unwrap();
        let (g, _) = qq.group_for_kinds(2, |m, rated| qq.choose(m, rated, &rows, 600).is_some()).unwrap();
        assert_eq!(g.len(), 2);
        // the lobby tick retries the ranked queue once per new slot while ranked searches wait
        assert!(qq.ranked_slot_changed(1800));
        assert!(!qq.ranked_slot_changed(2399), "same slot");
        assert!(qq.ranked_slot_changed(2400));
        qq.remove_account(1);
        qq.remove_account(2);
        assert!(!qq.ranked_slot_changed(3000), "no ranked search waits");
    }

    #[test]
    fn search_and_destroy_in_both_pools_and_the_ranked_fall_through() {
        let r = |map, mode, variation, weather| Pick { map, mode, variation, weather };
        assert_eq!([0, 600, 1200, 1800].map(ranked_order), [[0, 1, 2, 5], [1, 2, 5, 0], [2, 5, 0, 1], [5, 0, 1, 2]]);
        // the data's four S&D layouts (School 7, Vostok 5 / 6, Abandoned base 5), tdm on School v0, br on Vostok v0
        let sad = [r(2, 5, 7, 1), r(3, 5, 5, 1), r(3, 5, 6, 1), r(12, 5, 5, 1)];
        let rows: Vec<Pick> = sad.iter().copied().chain([r(2, 1, 0, 1), r(3, 0, 0, 1)]).collect();
        // ranked: default variations only (5; School 7); Vostok's second layout is unrated only
        assert_eq!(sad.map(|p| p.in_pool(true)), [true, true, false, true]);
        assert!(sad.iter().all(|p| p.in_pool(false)));
        assert!(!r(2, 5, 5, 1).in_pool(true), "School's unlisted id 5 is not its ranked layout");
        assert!(RANKED_MAPS.iter().any(|(m, modes)| *m == 12 && modes.contains(&SAD_MODE)));
        let mut q = PvpQueue::default();
        // 00:30 = the S&D slot: an S&D match on a ranked map, whatever was ticked; the map rotates
        let p = q.pick(0x382, true, &rows, 1800, |_| 0).unwrap();
        assert_eq!((p.mode, p.variation), (5, 7), "School first (lowest map), its layout 7");
        q.note_pick(p);
        let p = q.pick(0x382, true, &rows, 1800, |n| n - 1).unwrap();
        assert_eq!(p, r(3, 5, 5, 1), "Vostok's ranked layout is 5 only (one candidate)");
        q.note_pick(p);
        assert_eq!(q.pick(0x382, true, &rows, 1800, |_| 0).unwrap(), r(12, 5, 5, 1));
        // unrated: the S&D box (bit 5) plays S&D, both Vostok layouts drawn
        let q = PvpQueue::default();
        let p = q.pick(0x3A0, false, &rows, 0, |_| 0).unwrap();
        assert_eq!(p.mode, 5);
        let mut seen = vec![];
        for k in 0..2 {
            let mut qq = PvpQueue::default();
            qq.note_pick(r(2, 5, 7, 1));
            qq.note_pick(r(12, 5, 5, 1));
            seen.push(qq.pick(0x020, false, &rows, 0, |_| k).unwrap());
        }
        assert_eq!(seen, vec![r(3, 5, 5, 1), r(3, 5, 6, 1)]);
        // no S&D row served: the S&D slot falls through to battery (the next slot), never waits with no row
        let no_sad: Vec<Pick> = rows.iter().filter(|p| p.mode != 5).copied().collect();
        assert_eq!(q.choose(0, true, &no_sad, 1800), Some((0, 3)));
        assert_eq!(q.choose(0x020, false, &no_sad, 1800), None, "an S&D-only unrated search has no row");
    }

    /// The world host's room for the narrowing tests: no new map can load, these projects loaded, busy and with room, no
    /// drain named.
    fn loaded_scope(rows: &[(Pick, String)], open: &[&str]) -> Scope {
        use crate::host_room::{tests::{map_aged, room}, MatchRoom, NewMaps};
        let maps: Vec<_> = open.iter().map(|p| map_aged(p, 3, MatchRoom::Within, 100)).collect();
        crate::host_room::scope(rows, Some(&room(NewMaps::NoRoom, maps)))
    }

    fn named(rows: &[Pick]) -> Vec<(Pick, String)> {
        rows.iter().map(|r| (*r, format!("m{}_{}_v{}", r.map, r.mode, r.variation))).collect()
    }

    #[test]
    fn narrowed_picks_take_turns_a_fresh_map_once() {
        let r = |map, mode| Pick { map, mode, variation: 0, weather: 1 };
        // tdm on A (map 1), B (map 2), C (map 3) and D (map 4, not loaded)
        let rows = [r(1, 1), r(2, 1), r(3, 1), r(4, 1)];
        let named = named(&rows);
        let mut q = PvpQueue::default();
        // A started six times, B five, interleaved, A last; C just loaded, never started
        for _ in 0..5 {
            q.note_pick(r(1, 1));
            q.note_pick(r(2, 1));
        }
        q.note_pick(r(1, 1));
        let scope = loaded_scope(&named, &["m1_1_v0", "m2_1_v0", "m3_1_v0"]);
        let mut seen = vec![];
        for _ in 0..6 {
            let p = q.pick_in(0x382, false, &scope, 0, |_| 0).unwrap();
            q.note_pick(p);
            seen.push(p.map);
        }
        // C once, then the older of A / B (B: A was started last), then the other; then round again; never D
        assert_eq!(seen, [3, 2, 1, 3, 2, 1]);
        assert!(seen.windows(2).all(|w| w[0] != w[1]), "never the same map twice in a row");
    }

    #[test]
    fn narrowed_picks_spread_over_loaded_modes_and_variants() {
        let r = |map, mode, variation| Pick { map, mode, variation, weather: 1 };
        // loaded: tdm on map 1 (variants 0 and 1), ctp on map 2; served too: tdm on map 3, ctp on map 4
        let rows = [r(1, 1, 0), r(1, 1, 1), r(2, 2, 0), r(3, 1, 0), r(4, 2, 0)];
        let named = named(&rows);
        let scope = loaded_scope(&named, &["m1_1_v0", "m1_1_v1", "m2_2_v0"]);
        let mut q = PvpQueue::default();
        let mut seen = vec![];
        for _ in 0..6 {
            let p = q.pick_in(0x386, false, &scope, 0, |_| 0).unwrap();
            q.note_pick(p);
            seen.push(p);
        }
        // the modes alternate (tdm and ctp ticked), tdm's two loaded variants take turns, maps 3 and 4 never
        assert_eq!(seen, [r(1, 1, 0), r(2, 2, 0), r(1, 1, 1), r(2, 2, 0), r(1, 1, 0), r(2, 2, 0)]);
        // a group that ticked tdm only stays on tdm's loaded map; one that ticked only S&D has nothing loaded
        assert_eq!(q.pick_in(0x382, false, &scope, 0, |_| 0).map(|p| p.map), Some(1));
        assert_eq!(q.pick_in(0x3A0, false, &scope, 0, |_| 0), None);
        // room comes back: the next pick is the rotation's next map over every served row (map 3, never started)
        let free = crate::host_room::scope(&named, Some(&crate::host_room::tests::room(crate::host_room::tests::can_load(1), vec![])));
        assert_eq!(q.pick_in(0x382, false, &free, 0, |_| 0), Some(r(3, 1, 0)));
        assert_eq!(q.pick_in(0x382, false, &crate::host_room::scope(&named, None), 0, |_| 0), Some(r(3, 1, 0)), "no status: the same");
    }

    #[test]
    fn the_oldest_loaded_map_drains_until_it_idles_then_the_rotation_moves_on() {
        use crate::host_room::tests::{evicting, map_aged, room, room_draining};
        use crate::host_room::{MatchRoom, NewMaps};
        let r = |map| Pick { map, mode: 1, variation: 0, weather: 1 };
        // tdm on A (map 1, loaded the longest), B, C (loaded later) and D (not loaded)
        let rows = [r(1), r(2), r(3), r(4)];
        let named = named(&rows);
        let busy = |a: MatchRoom, b: MatchRoom, c: MatchRoom| vec![map_aged("m1_1_v0", 2, a, 900), map_aged("m2_1_v0", 2, b, 500), map_aged("m3_1_v0", 2, c, 100)];
        let w = MatchRoom::Within;
        // no map can load; the world host drains A
        let scope = crate::host_room::scope(&named, Some(&room_draining(NewMaps::NoRoom, busy(w, w, w), "m1_1_v0")));
        assert_eq!(scope, Scope::Loaded { served: rows.to_vec(), open: rows[..3].to_vec(), avoid: vec![r(1)] });
        let mut q = PvpQueue::default();
        let mut seen = vec![];
        for _ in 0..4 {
            let p = q.pick_in(0x382, false, &scope, 0, |_| 0).unwrap();
            q.note_pick(p);
            seen.push(p.map);
        }
        assert_eq!(seen, [2, 3, 2, 3], "A gets no new match while B or C fits");
        // B and C full: A takes the group (a group is never held for the drain)
        let only_a = crate::host_room::scope(&named, Some(&room_draining(NewMaps::NoRoom, busy(w, MatchRoom::Full, MatchRoom::Full), "m1_1_v0")));
        assert_eq!(q.pick_in(0x382, false, &only_a, 0, |_| 0), Some(r(1)));
        // A's matches ended: it is idle and the world host can load a new map by evicting it. A is still a row (idle with
        // room) and its map was never started by the queue, like D's: the rotation would tie them and take A, the lower
        // id, and A would never go. A is avoided: the rotation's next map is D, whose load evicts A
        let idle_a = vec![map_aged("m1_1_v0", 0, w, 900), map_aged("m2_1_v0", 2, w, 500), map_aged("m3_1_v0", 2, w, 100)];
        let back = crate::host_room::scope(&named, Some(&room(evicting(1), idle_a)));
        assert_eq!(back, Scope::Rotation { served: rows.to_vec(), rows: rows.to_vec(), avoid: vec![r(1)] });
        assert_eq!(q.pick_in(0x382, false, &back, 0, |_| 0), Some(r(4)));
        let unavoided = Scope::Rotation { served: rows.to_vec(), rows: rows.to_vec(), avoid: vec![] };
        assert_eq!(q.pick_in(0x382, false, &unavoided, 0, |_| 0), Some(r(1)), "what the rotation alone would pick");
    }

    #[test]
    fn the_idle_map_a_load_evicts_is_taken_only_when_nothing_else_fits() {
        use crate::host_room::tests::{evicting, map, room};
        use crate::host_room::MatchRoom;
        let r = |map, mode| Pick { map, mode, variation: 0, weather: 1 };
        // X: tdm on map 1, loaded and idle (the next load evicts it); Y: tdm on map 2, not loaded; Z: ctp on map 2, loaded
        // and busy
        let (x, y, z) = (r(1, 1), r(2, 1), r(2, 2));
        let named = named(&[x, y, z]);
        let mut q = PvpQueue::default();
        // X's map was started long ago, map 2 recently (in ctp)
        q.note_pick(x);
        q.note_pick(z);
        let st = room(evicting(1), vec![map("m1_1_v0", 0, MatchRoom::Within), map("m2_2_v0", 2, MatchRoom::Within)]);
        let scope = crate::host_room::scope(&named, Some(&st));
        assert_eq!(scope, Scope::Rotation { served: vec![x, y, z], rows: vec![x, y, z], avoid: vec![x] });
        // a tdm group: Y (it loads, X is evicted for it), although X's map was started longer ago
        assert_eq!(q.pick_in(0x382, false, &scope, 0, |_| 0), Some(y));
        // without a status the rotation's oldest map is X's
        assert_eq!(q.pick_in(0x382, false, &crate::host_room::scope(&named, None), 0, |_| 0), Some(x));
        // a tdm group with only X to play (Y not served): X, never held
        let only_x = crate::host_room::scope(&self::named(&[x, z]), Some(&st));
        assert_eq!(q.pick_in(0x382, false, &only_x, 0, |_| 0), Some(x));
    }

    #[test]
    fn narrowed_ranked_keeps_its_slot_mode() {
        let r = |map, mode| Pick { map, mode, variation: 0, weather: 1 };
        // ranked rows: br on Tarakanovsky Fort (5), tdm on School (2) and Vostok (3)
        let rows = [r(5, 0), r(2, 1), r(3, 1)];
        let named = named(&rows);
        let q = PvpQueue::default();
        // 00:10 = the tdm slot: only Vostok's tdm is loaded -> Vostok
        let scope = loaded_scope(&named, &["m3_1_v0", "m5_0_v0"]);
        assert_eq!(q.pick_in(0x381, true, &scope, 600, |_| 0), Some(r(3, 1)));
        // only the battery map loaded: the tdm slot does not fall through to it, the group waits
        let scope = loaded_scope(&named, &["m5_0_v0"]);
        assert_eq!(q.choose_in(0x381, true, &scope, 600, |_| 0), None);
        assert_eq!(q.pick_in(0x381, true, &scope, 0, |_| 0), Some(r(5, 0)), "00:00, the battery slot: the loaded map");
        assert_eq!(q.pick_in(0x381, true, &Scope::Held, 0, |_| 0), None);
        // a map can load, but both tdm maps are loaded and full: the tdm slot holds, it does not move to battery
        use crate::host_room::tests::{can_load, map, room};
        use crate::host_room::MatchRoom;
        let st = room(can_load(1), vec![map("m2_1_v0", 8, MatchRoom::Full), map("m3_1_v0", 8, MatchRoom::Full)]);
        let rot = crate::host_room::scope(&named, Some(&st));
        assert_eq!(rot, Scope::Rotation { served: rows.to_vec(), rows: vec![r(5, 0)], avoid: vec![] });
        assert_eq!(q.choose_in(0x381, true, &rot, 600, |_| 0), None);
        // ... only when the slot's mode has no served row at all does it fall through (as without a status)
        let no_tdm = [r(5, 0)];
        assert_eq!(q.choose_in(0x381, true, &crate::host_room::scope(&self::named(&no_tdm), Some(&st)), 600, |_| 0), Some((0, 5)));
    }

    #[test]
    fn a_group_is_held_only_when_the_room_left_its_rows_out() {
        let r = |map, mode| Pick { map, mode, variation: 0, weather: 1 };
        // tdm on maps 1 and 2, ctp on map 3
        let rows = [r(1, 1), r(2, 1), r(3, 2)];
        let named = named(&rows);
        let mut q = PvpQueue::default();
        q.enqueue(vec![1], 0x382, false, 10).unwrap(); // tdm
        q.enqueue(vec![2], 0x382, false, 11).unwrap();
        // only ctp loaded with room: the tdm pair would form on every row, the room holds it (key: its mask)
        let ctp_only = loaded_scope(&named, &["m3_2_v0"]);
        assert_eq!(q.group_in(2, false, &rows, &ctp_only, 0), Forms::Held(0x382));
        // a tdm map loaded with room: the group forms
        let tdm = loaded_scope(&named, &["m2_1_v0"]);
        assert!(matches!(q.group_in(2, false, &rows, &tdm, 0), Forms::Group(g, 0x382) if g.len() == 2));
        // too few players: wait, whatever the room
        assert_eq!(q.group_in(3, false, &rows, &ctp_only, 0), Forms::Wait);
        // nothing left out (no status): never held
        assert!(matches!(q.group_in(2, false, &rows, &crate::host_room::scope(&named, None), 0), Forms::Group(..)));
        assert_eq!(q.group_in(2, false, &rows, &Scope::Held, 0), Forms::Held(0x382));
        // the bot fill's unrated-only grouping, one player enough
        assert_eq!(q.group_in(1, true, &rows, &ctp_only, 0), Forms::Held(0x382));
        // players who share no served mode wait (ctp is loaded, they ticked S&D)
        let mut s = PvpQueue::default();
        s.enqueue(vec![3], 0x3A0, false, 10).unwrap();
        assert_eq!(s.group_in(1, false, &rows, &ctp_only, 0), Forms::Wait);
    }

    #[test]
    fn a_full_group_forms_from_overlapping_tickets() {
        let mut q = PvpQueue::default();
        q.enqueue(vec![1, 2, 3], 0x380, false, 10).unwrap();
        q.enqueue(vec![4, 5], 0x100, false, 11).unwrap();
        q.enqueue(vec![6], 0x040, false, 12).unwrap();
        assert!(q.group(7).is_empty());
        let g = q.group(5);
        assert_eq!(g.iter().map(|t| t.order).collect::<Vec<_>>(), vec![1, 2]);
        assert!(q.group(0).is_empty(), "no data, no start");
    }
}
