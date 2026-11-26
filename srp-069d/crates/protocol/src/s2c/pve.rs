//! PvE messages of the server (docs/notes/pve.md section 1.2, 4.4, 5). Every layout is PROVEN from the 0.69d exe (the
//! reader addresses are quoted per type); the only unproven parts are named in the doc comments.
//!
//! `C2` and `99` are not modelled by [`S2c`] directly: each type here has a `message()` that gives the raw message, and a
//! `decode` for tests and for a client-side reader.

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

use super::{Denied, DeniedBody, S2c};

/// Squad `type` byte of `AE` (`Squad::x`): 0 normal squad, 1 PvE squad. PROVEN: reader 0x20018DC00, UI 0x1C4891 (`type`),
/// AS `PVEManager.setSquadType`; `type == 1` opens the PvE window on its own.
pub const SQUAD_TYPE_PVE: u8 = 1;

/// Message id of the PvE settings / top 100 messages (unique class, the udp-transport table).
pub const PVE_PUSH: u8 = 0xC2;
/// Message id of the PvE result.
pub const PVE_RESULT: u8 = 0x99;
/// `84 x 5F` codes (reader 0x20018F4BF -> `on_pve_action_denied` 0x2001CBBD0). PROVEN.
pub mod deny {
    /// `st_profile_check_failed_not_enough_money`; the text carries a loop variable (OPEN which), so the server does not use it.
    pub const PROFILE_NOT_ENOUGH_MONEY: u8 = 3;
    /// `st_not_enougth_money`.
    pub const NOT_ENOUGH_MONEY: u8 = 4;
    /// `st_not_all_team_members_ready`.
    pub const NOT_ALL_READY: u8 = 5;
    /// `st_not_enough_match_servers`.
    pub const NO_MATCH_SERVER: u8 = 6;
    /// Any other code: the generic `st_pve_action_denied` ("Unable to start the team mission").
    pub const GENERIC: u8 = 0;
}

/// `C2 00 u32 map id, u32 difficulty`: the squad commander changed the mission settings. PROVEN: inline 0x20018FB55 ->
/// 0x2001CB940 (`on_pve_settings_changed_by_commander`, text `st_pve_settings_changed_by_commander`); Flash then calls
/// `accept_pve_commander_settings` (local, no message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PveSettingsChanged {
    pub map: u32,
    pub difficulty: u32,
}

impl PveSettingsChanged {
    pub fn message(&self) -> S2c {
        S2c::Raw { id: PVE_PUSH, payload: [vec![0u8], self.to_bytes().unwrap_or_default()].concat() }
    }

    /// Decode the whole `C2` payload (sub byte first).
    pub fn from_push(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        if r.u8()? != 0 {
            return Err(Error::Invalid("C2 sub other than 0"));
        }
        let m = Self::decode(&mut r)?;
        r.finish()?;
        Ok(m)
    }
}

impl Encode for PveSettingsChanged {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.map).u32(self.difficulty);
    }
}

impl Decode for PveSettingsChanged {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(PveSettingsChanged { map: r.u32()?, difficulty: r.u32()? })
    }
}

/// One squad of the top 100 list: `u8 k (<= 3), k x { str128 nick, str32 clan abbreviation, u8 score }, u16 play_time`
/// (seconds). PROVEN: reader 0x181320 (0x208 bytes in memory), UI 0x1CBE80 (`position`, `players[{name, abbr, score}]`,
/// `play_time` as `h:mm:ss`). The lobby movie's PVETop100Screen shows the players' `score` under its "chests_num"
/// column: the lobby sends the run's chests found there (seen in a real run; OPEN for a squad: one player's byte or a
/// sum).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopEntry {
    pub players: Vec<TopPlayer>,
    pub play_time: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopPlayer {
    pub name: String,
    pub clan: String,
    pub score: u8,
}

impl Encode for TopEntry {
    fn encode(&self, w: &mut Writer) {
        if self.players.len() > 3 {
            w.fail(Error::Limit("C2 03 players per entry (3)"));
        }
        w.count_u8(self.players.len(), "C2 03 players");
        for p in &self.players {
            w.str_max(&p.name, 128).str_max(&p.clan, 32).u8(p.score);
        }
        w.u16(self.play_time);
    }
}

impl Decode for TopEntry {
    fn decode(r: &mut Reader) -> Result<Self> {
        let k = r.u8()? as usize;
        if k > 3 {
            return Err(Error::Limit("C2 03 players per entry (3)"));
        }
        let mut players = Vec::with_capacity(k);
        for _ in 0..k {
            players.push(TopPlayer { name: r.str()?, clan: r.str()?, score: r.u8()? });
        }
        Ok(TopEntry { players, play_time: r.u16()? })
    }
}

/// `C2 03 u8 n, n x entry, u32 own index (FFFFFFFF = none), [own != FFFFFFFF: one more entry, the player's own squad]`.
/// PROVEN: reader 0x200197740 (frame 0xCE00). The index is the 0-based position of the own squad in the list (UI
/// `is_player`; LIKELY: the exe only compares it with FFFFFFFF and with the loop index); when the own squad is not in the
/// first `n` the index is the real place and the extra entry shows the squad (LIKELY).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PveTop100 {
    pub entries: Vec<TopEntry>,
    /// `(index, own entry)`; None = the player has no result.
    pub own: Option<(u32, TopEntry)>,
}

/// The most entries the message carries (the exe keeps 100).
pub const TOP_MAX: usize = 100;

impl PveTop100 {
    pub fn message(&self) -> Result<S2c> {
        let mut w = Writer::new();
        w.u8(3);
        self.encode(&mut w);
        Ok(S2c::Raw { id: PVE_PUSH, payload: w.finish()? })
    }

    pub fn from_push(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        if r.u8()? != 3 {
            return Err(Error::Invalid("C2 sub other than 3"));
        }
        let m = Self::decode(&mut r)?;
        r.finish()?;
        Ok(m)
    }
}

impl Encode for PveTop100 {
    fn encode(&self, w: &mut Writer) {
        if self.entries.len() > TOP_MAX {
            w.fail(Error::Limit("C2 03 entries (100)"));
        }
        w.count_u8(self.entries.len(), "C2 03 entries");
        for e in &self.entries {
            e.encode(w);
        }
        match &self.own {
            Some((i, e)) => {
                if *i == u32::MAX {
                    w.fail(Error::Invalid("C2 03 own index FFFFFFFF means none"));
                }
                w.u32(*i);
                e.encode(w);
            }
            None => {
                w.u32(u32::MAX);
            }
        }
    }
}

impl Decode for PveTop100 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u8()? as usize;
        if n > TOP_MAX {
            return Err(Error::Limit("C2 03 entries (100)"));
        }
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            entries.push(TopEntry::decode(r)?);
        }
        let i = r.u32()?;
        let own = if i == u32::MAX { None } else { Some((i, TopEntry::decode(r)?)) };
        Ok(PveTop100 { entries, own })
    }
}

/// `84 x 5F u8 1, u8 code`: a PvE action was denied. PROVEN reader 0x20018F4BF (codes in [`deny`]); it also clears the
/// "Play waiting" flag `lobby_menu+0xEBBD4`, so the Play button works again. The first byte after the request id must be 1
/// (anything else reads nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PveDenied {
    pub code: u8,
}

impl PveDenied {
    pub fn message(&self) -> S2c {
        S2c::Denied(Denied { request: crate::ids::c2s::PVE, body: DeniedBody::Pve { code: self.code } })
    }
}

/// `85 04 u32 order, u32 search, u32`: the PvE search runs (queue state 4). PROVEN reader 0x200186280: the body is that
/// of state 1 (order -> L+0x6B0D18, `search` -> L+0x6B0D10, the last word -> L+0x6B0D0C, read by nothing). The status
/// case calls `root.set_pve_match_making(queue_state == 4)` (0x200175639) and then, for state 1 / 4, 0x2001C16F0 with
/// `search` (0x2001756B9), the only reader of that word:
/// * 0 = "match found": `root.set_mm_sync_state(st_mm_match_found)` and the found flag (menu+0xEBBE8) (0x2001C1736..);
/// * not 0 = the search runs: `root.set_game_search_enabled(true)` and the search range (0x2001C17E2..).
///
/// The search must run first: `set_game_search_enabled` changes the lobby movie only when the value changes
/// (`LobbyGameSearch.searchType`, LobbyMenu.set_game_search_enabled), and only that change at the `85 00` calls
/// `matchMaking.hideGameSearch(false)`. The level load sets `MatchMaking.hide` from `pveMatchMakingActive` (true here:
/// LobbyMenu.start_loading -> onHidePVEWindow -> hideGameSearch), and while it stays set every layout hides the mode window
/// (match settings): with `search` 0 the lobby lost Play and the mode choice after a PvE match.
/// Sent [`PveSearching::SEARCH_RUNNING`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PveSearching {
    pub order: u32,
}

impl PveSearching {
    /// The `search` word: any value but 0 means "the search runs"; 100 is the value the exe itself passes to the same
    /// function for a running state (0x200175717 on `85 00`, 0x2001E85EC on Play).
    pub const SEARCH_RUNNING: u32 = 100;

    pub fn message(&self) -> S2c {
        let mut p = vec![4u8];
        p.extend_from_slice(&self.order.to_le_bytes());
        p.extend_from_slice(&Self::SEARCH_RUNNING.to_le_bytes());
        p.extend_from_slice(&[0; 4]);
        S2c::Raw { id: crate::ids::s2c::QUEUE_STATE, payload: p }
    }

    pub fn from_payload(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        if r.u8()? != 4 {
            return Err(Error::Invalid("85 state other than 4"));
        }
        let order = r.u32()?;
        r.skip(8)?;
        r.finish()?;
        Ok(PveSearching { order })
    }
}

/// One player record of `99`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PveResultPlayer {
    pub name: String,
    pub clan: String,
    /// Public account id.
    pub account: u64,
    /// Signed in the exe (`movsx`, 0x20013EDE9).
    pub score: i16,
    pub kills: u16,
    pub deaths: u16,
    pub sort: u8,
    pub premium: u8,
}

/// `99` PvE result (ordered). PROVEN field order (blocks 0x20018ECDD, 0x180D10 team, 0x180980 / 0x180DC3 players):
/// `u32 match id (compared with the running match), u32 (not displayed), u32 match time s, u8 (unused, +0xD), u8 success
/// (non-zero = victory, +0x10), u8 chests found, u8 chests total, u8 n players (+0xC), n x { str128 name, str32 clan,
/// u64 public id, u16 score, u16 kills, u16 deaths, u8 sort, u8 premium }, u8 index of the receiving player`. At most 20
/// records are prepared by the client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PveResult {
    pub match_id: u32,
    pub time_secs: u32,
    pub chests_found: u8,
    pub chests_total: u8,
    pub success: bool,
    pub players: Vec<PveResultPlayer>,
    /// Index of the receiving player in `players`.
    pub me: u8,
}

impl PveResult {
    pub fn message(&self) -> Result<S2c> {
        Ok(S2c::Raw { id: PVE_RESULT, payload: self.to_bytes()? })
    }
}

impl Encode for PveResult {
    fn encode(&self, w: &mut Writer) {
        if self.players.len() > 20 {
            w.fail(Error::Limit("99 players (20)"));
        }
        w.u32(self.match_id).u32(0).u32(self.time_secs);
        // wire order (reader 0x200180D67..0x200180DB7): unused, success, found, total, n
        w.u8(0).u8(self.success as u8).u8(self.chests_found).u8(self.chests_total);
        w.count_u8(self.players.len(), "99 players");
        for p in &self.players {
            w.str_max(&p.name, 128).str_max(&p.clan, 32).u64(p.account);
            w.u16(p.score as u16).u16(p.kills).u16(p.deaths).u8(p.sort).u8(p.premium);
        }
        w.u8(self.me);
    }
}

impl Decode for PveResult {
    fn decode(r: &mut Reader) -> Result<Self> {
        let match_id = r.u32()?;
        r.u32()?;
        let time_secs = r.u32()?;
        r.u8()?;
        let success = r.u8()? != 0;
        let (chests_found, chests_total) = (r.u8()?, r.u8()?);
        let n = r.u8()? as usize;
        if n > 20 {
            return Err(Error::Limit("99 players (20)"));
        }
        let mut players = Vec::with_capacity(n);
        for _ in 0..n {
            players.push(PveResultPlayer {
                name: r.str()?,
                clan: r.str()?,
                account: r.u64()?,
                score: r.u16()? as i16,
                kills: r.u16()?,
                deaths: r.u16()?,
                sort: r.u8()?,
                premium: r.u8()?,
            });
        }
        let me = r.u8()?;
        Ok(PveResult { match_id, time_secs, chests_found, chests_total, success, players, me })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_changed_bytes() {
        let m = PveSettingsChanged { map: 15, difficulty: 2 };
        let S2c::Raw { id, payload } = m.message() else { panic!() };
        assert_eq!(id, 0xC2);
        assert_eq!(payload, [0, 15, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(PveSettingsChanged::from_push(&payload).unwrap(), m);
        assert!(PveSettingsChanged::from_push(&[1, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn top_100_bytes_and_round_trip() {
        let one = TopEntry { players: vec![TopPlayer { name: "ab".into(), clan: "C".into(), score: 7 }], play_time: 0x0102 };
        let t = PveTop100 { entries: vec![one.clone()], own: None };
        let S2c::Raw { payload, .. } = t.message().unwrap() else { panic!() };
        // sub 3, n 1, entry (k 1, str "ab", str "C", score 7, time LE), own index FFFFFFFF
        assert_eq!(payload, [3, 1, 1, 2, b'a', b'b', 1, b'C', 7, 2, 1, 0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(PveTop100::from_push(&payload).unwrap(), t);
        let with_own = PveTop100 { entries: vec![one.clone()], own: Some((0, one)) };
        let S2c::Raw { payload, .. } = with_own.message().unwrap() else { panic!() };
        assert_eq!(&payload[11..15], &[0, 0, 0, 0]);
        assert_eq!(PveTop100::from_push(&payload).unwrap(), with_own);
        // an empty list is legal (the UI shows st_pve_top_100_empty)
        let S2c::Raw { payload, .. } = PveTop100::default().message().unwrap() else { panic!() };
        assert_eq!(payload, [3, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn top_100_limits() {
        let e = TopEntry { players: vec![TopPlayer::default(); 4], play_time: 0 };
        assert!(PveTop100 { entries: vec![e], own: None }.message().is_err());
        assert!(PveTop100 { entries: vec![TopEntry::default(); 101], own: None }.message().is_err());
    }

    #[test]
    fn denied_and_searching_bytes() {
        let m = PveDenied { code: deny::NOT_ALL_READY }.message();
        assert_eq!(m.id(), 0x84);
        assert_eq!(m.encode_payload().unwrap(), [0x5F, 1, 5]);
        assert_eq!(S2c::decode(0x84, &[0x5F, 1, 5]).unwrap(), m);
        let s = PveSearching { order: 9 }.message();
        let S2c::Raw { id, payload } = &s else { panic!() };
        assert_eq!(*id, 0x85);
        // the second word is not 0: a running search, never "match found" (see PveSearching)
        assert_eq!(payload, &[4, 9, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(PveSearching::from_payload(payload).unwrap().order, 9);
    }

    #[test]
    fn result_field_order() {
        let r = PveResult {
            match_id: 0x0A0B0C0D,
            time_secs: 600,
            chests_found: 4,
            chests_total: 10,
            success: true,
            players: vec![PveResultPlayer { name: "n".into(), clan: "".into(), account: 5, score: 300, kills: 2, deaths: 1, sort: 0, premium: 1 }],
            me: 0,
        };
        let b = r.to_bytes().unwrap();
        assert_eq!(&b[..4], &[0x0D, 0x0C, 0x0B, 0x0A]);
        assert_eq!(&b[8..12], &600u32.to_le_bytes());
        // unused, success, found, total, n (reader 0x200180D67..0x200180DB7)
        assert_eq!(&b[12..17], &[0, 1, 4, 10, 1]);
        assert_eq!(PveResult::from_bytes(&b).unwrap(), r);
        assert_eq!(r.message().unwrap().id(), 0x99);
        let many = PveResult { players: vec![PveResultPlayer::default(); 21], ..r };
        assert!(many.to_bytes().is_err());
    }
}
