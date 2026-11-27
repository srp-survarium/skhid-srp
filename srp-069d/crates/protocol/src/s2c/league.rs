//! League data: push 0x9F and the answers `83 4B` (static analysis; only the empty answers were seen
//! working). The league window waits for these; with S = 0 the UI shows "no league".

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// Row of the friends / clan league lists: `u16, u8, u8, f64` (16 bytes in the client). PROVEN layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LeagueEntry {
    pub a: u16,
    pub b: u8,
    pub c: u8,
    pub value: f64,
}

/// Season history element: `u8, u16, u8` (6 bytes in the client, at most 64). PROVEN layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SeasonEntry {
    pub a: u8,
    pub b: u16,
    pub c: u8,
}

fn write_entries(w: &mut Writer, v: &[LeagueEntry]) {
    w.count_u8(v.len(), "league entries");
    for e in v {
        w.u16(e.a).u8(e.b).u8(e.c).f64(e.value);
    }
}
fn read_entries(r: &mut Reader) -> Result<Vec<LeagueEntry>> {
    let n = r.u8()? as usize;
    (0..n).map(|_| Ok(LeagueEntry { a: r.u16()?, b: r.u8()?, c: r.u8()?, value: r.f64()? })).collect()
}
fn write_history(w: &mut Writer, v: &[SeasonEntry]) {
    if v.len() > 64 {
        w.fail(Error::Limit("season history (64)"));
    }
    w.count_u8(v.len(), "season history");
    for e in v {
        w.u8(e.a).u16(e.b).u8(e.c);
    }
}
fn read_history(r: &mut Reader) -> Result<Vec<SeasonEntry>> {
    let n = r.u8()? as usize;
    if n > 64 {
        return Err(Error::Limit("season history (64)"));
    }
    (0..n).map(|_| Ok(SeasonEntry { a: r.u8()?, b: r.u16()?, c: r.u8()? })).collect()
}

/// Reward group of the league info: `u32 id, u8 m (<= 5), m x {u16 dict, u32 amount, u8}`. The item dict ids must
/// exist in the client dictionary (LIKELY assert otherwise).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RewardGroup {
    pub id: u32,
    pub items: Vec<(u16, u32, u8)>,
}

/// The player's own league state (present when S != 0).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LeagueState {
    /// League points (`set_league_progress`, 9th argument).
    pub id: u32,
    pub w: u16,
    /// Current level (also stored at L+0x7A394A).
    pub lv: u8,
    pub lv2: u8,
    /// Season history (9F only).
    pub history: Vec<SeasonEntry>,
    pub friends: Vec<LeagueEntry>,
    pub clan: Vec<LeagueEntry>,
    /// 5 x `{u8, u8}` pushed as u16 values (9F only).
    pub pairs: [(u8, u8); 5],
}

/// 0x9F type 0, full league info. PROVEN layout (reader 0x200188EE0, PROVEN by code path):
/// `u8 type 0, u16 N1, N1 x RewardGroup, u8 K, K x {u32, u32, u8 c (1..number of leagues in the client config)},
/// u8 x, u8 A (0xFF = none), u8 B, [A != 0xFF: u32 val], u16 S (0 = no league), u8 flag, [S != 0: state]`.
/// The minimal no-league payload is `00 00 00 00 00 FF 00 00 00 00` (10 bytes).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LeagueInfo {
    pub rewards: Vec<RewardGroup>,
    pub levels: Vec<(u32, u32, u8)>,
    pub x: u8,
    pub a: u8,
    pub b: u8,
    pub val: u32,
    pub s: u16,
    pub flag: u8,
    pub state: Option<LeagueState>,
}

impl LeagueInfo {
    /// The 10-byte "no league" answer.
    pub fn none() -> Self {
        LeagueInfo { a: 0xFF, ..Default::default() }
    }
}

/// One entry of the 9F type 3 notice: `u16, u32, u8 pc (<= 4), u32, u16, pc x {u16, u16}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NoticeEntry {
    pub a: u16,
    pub b: u32,
    pub c: u32,
    pub d: u16,
    pub pairs: Vec<(u16, u16)>,
}

/// 0x9F type 3, "league up / season end" notice: `u8 state, u8 league_up, u8 show_rewards, u8 n, n x entry`.
/// `03 00 00 00 00` is harmless (the client only answers with 4B 02). State > 1 does item lookups: valid ids only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LeagueNotice {
    pub state: u8,
    pub league_up: u8,
    pub show_rewards: u8,
    pub entries: Vec<NoticeEntry>,
}

/// 0x9F league push (ordered, status "do nothing", not a loading gate).
#[derive(Debug, Clone, PartialEq)]
pub enum League9F {
    Info(LeagueInfo),
    Notice(LeagueNotice),
}

impl Encode for League9F {
    fn encode(&self, w: &mut Writer) {
        match self {
            League9F::Info(i) => {
                w.u8(0).count_u16(i.rewards.len(), "league rewards");
                for g in &i.rewards {
                    if g.items.len() > 5 {
                        w.fail(Error::Limit("reward group items (5)"));
                    }
                    w.u32(g.id).count_u8(g.items.len(), "reward items");
                    for &(d, a, c) in &g.items {
                        w.u16(d).u32(a).u8(c);
                    }
                }
                w.count_u8(i.levels.len(), "league levels");
                for &(a, b, c) in &i.levels {
                    w.u32(a).u32(b).u8(c);
                }
                w.u8(i.x).u8(i.a).u8(i.b);
                if i.a != 0xFF {
                    w.u32(i.val);
                }
                w.u16(i.s).u8(i.flag);
                match (&i.state, i.s) {
                    (Some(st), s) if s != 0 => {
                        w.u32(st.id).u16(st.w).u8(st.lv).u8(st.lv2);
                        write_history(w, &st.history);
                        write_entries(w, &st.friends);
                        write_entries(w, &st.clan);
                        for &(a, b) in &st.pairs {
                            w.u8(a).u8(b);
                        }
                    }
                    (None, 0) => {}
                    _ => w.fail(Error::Invalid("9F: state present exactly when S != 0")),
                }
            }
            League9F::Notice(n) => {
                w.u8(3).u8(n.state).u8(n.league_up).u8(n.show_rewards).count_u8(n.entries.len(), "league notice");
                for e in &n.entries {
                    if e.pairs.len() > 4 {
                        w.fail(Error::Limit("notice pairs (4)"));
                    }
                    w.u16(e.a).u32(e.b).count_u8(e.pairs.len(), "notice pairs").u32(e.c).u16(e.d);
                    for &(g, r) in &e.pairs {
                        w.u16(g).u16(r);
                    }
                }
            }
        }
    }
}

impl Decode for League9F {
    fn decode(r: &mut Reader) -> Result<Self> {
        match r.u8()? {
            0 => {
                let mut i = LeagueInfo::default();
                let n1 = r.u16()? as usize;
                for _ in 0..n1 {
                    let id = r.u32()?;
                    let m = r.u8()? as usize;
                    if m > 5 {
                        return Err(Error::Limit("reward group items (5)"));
                    }
                    let items = (0..m).map(|_| Ok((r.u16()?, r.u32()?, r.u8()?))).collect::<Result<_>>()?;
                    i.rewards.push(RewardGroup { id, items });
                }
                let k = r.u8()? as usize;
                for _ in 0..k {
                    i.levels.push((r.u32()?, r.u32()?, r.u8()?));
                }
                i.x = r.u8()?;
                i.a = r.u8()?;
                i.b = r.u8()?;
                if i.a != 0xFF {
                    i.val = r.u32()?;
                }
                i.s = r.u16()?;
                i.flag = r.u8()?;
                if i.s != 0 {
                    let mut st = LeagueState { id: r.u32()?, w: r.u16()?, lv: r.u8()?, lv2: r.u8()?, ..Default::default() };
                    st.history = read_history(r)?;
                    st.friends = read_entries(r)?;
                    st.clan = read_entries(r)?;
                    for p in &mut st.pairs {
                        *p = (r.u8()?, r.u8()?);
                    }
                    i.state = Some(st);
                }
                Ok(League9F::Info(i))
            }
            3 => {
                let mut n = LeagueNotice { state: r.u8()?, league_up: r.u8()?, show_rewards: r.u8()?, entries: vec![] };
                let cnt = r.u8()? as usize;
                for _ in 0..cnt {
                    let a = r.u16()?;
                    let b = r.u32()?;
                    let pc = r.u8()? as usize;
                    if pc > 4 {
                        return Err(Error::Limit("notice pairs (4)"));
                    }
                    let c = r.u32()?;
                    let d = r.u16()?;
                    let pairs = (0..pc).map(|_| Ok((r.u16()?, r.u16()?))).collect::<Result<_>>()?;
                    n.entries.push(NoticeEntry { a, b, c, d, pairs });
                }
                Ok(League9F::Notice(n))
            }
            _ => Err(Error::Unsupported("9F type other than 0 and 3")),
        }
    }
}

/// `83 4B` answers (after the request id 4B). PROVEN reader 0x200191950; field meaning LIKELY.
#[derive(Debug, Clone, PartialEq)]
pub enum LeagueReply {
    /// Kind 1, answer to `4B 01`: `u16 S2` (0 = nothing more), else `u32 id, u16 w, u8 lv, u8 lv2, friends, clan`.
    /// Keep S2 equal to the S of the 9F push (else the client sends 4B 02 after every kind-1 reply).
    Window { s2: u16, state: Option<LeagueState> },
    /// Kind 4, answer to `4B 04 u64`: `u64 public id` (echo; PROVEN 0x20027EB90 prints it with `%llu` into the Flash
    /// object's `public_account_id`), `u8 n (<= 64), n x {u8 month, u16 year, u8 league}`. The window compares the echo
    /// with the viewer's own id: an empty own history shows `st_season_league_history_not_found`, another player's
    /// `st_season_league_player_history_not_found` (AS `LeagueHistoryWindow.setLeagueHistoryData`).
    History { public_id: u64, entries: Vec<SeasonEntry> },
    /// Kind 5, answer to `4B 05`: `u8 n (<= 150), n x {u16, str128 name, str32 clan}`, then the local player's
    /// `{u16, str128, str32}` and a `u32`.
    Top { rows: Vec<(u16, String, String)>, me: (u16, String, String), tail: u32 },
}

impl Encode for LeagueReply {
    fn encode(&self, w: &mut Writer) {
        match self {
            LeagueReply::Window { s2, state } => {
                w.u8(1).u16(*s2);
                match (state, *s2) {
                    (Some(st), s) if s != 0 => {
                        w.u32(st.id).u16(st.w).u8(st.lv).u8(st.lv2);
                        write_entries(w, &st.friends);
                        write_entries(w, &st.clan);
                    }
                    (None, 0) => {}
                    _ => w.fail(Error::Invalid("4B: state present exactly when S2 != 0")),
                }
            }
            LeagueReply::History { public_id, entries } => {
                w.u8(4).u64(*public_id);
                write_history(w, entries);
            }
            LeagueReply::Top { rows, me, tail } => {
                if rows.len() > 150 {
                    w.fail(Error::Limit("league top rows (150)"));
                }
                w.u8(5).count_u8(rows.len(), "league top rows");
                for (a, n, c) in rows {
                    w.u16(*a).str_max(n, 128).str_max(c, 32);
                }
                w.u16(me.0).str_max(&me.1, 128).str_max(&me.2, 32).u32(*tail);
            }
        }
    }
}

impl Decode for LeagueReply {
    fn decode(r: &mut Reader) -> Result<Self> {
        match r.u8()? {
            1 => {
                let s2 = r.u16()?;
                let state = if s2 != 0 {
                    let mut st = LeagueState { id: r.u32()?, w: r.u16()?, lv: r.u8()?, lv2: r.u8()?, ..Default::default() };
                    st.friends = read_entries(r)?;
                    st.clan = read_entries(r)?;
                    Some(st)
                } else {
                    None
                };
                Ok(LeagueReply::Window { s2, state })
            }
            4 => Ok(LeagueReply::History { public_id: r.u64()?, entries: read_history(r)? }),
            5 => {
                let n = r.u8()? as usize;
                if n > 150 {
                    return Err(Error::Limit("league top rows (150)"));
                }
                let rows = (0..n).map(|_| Ok((r.u16()?, r.str()?, r.str()?))).collect::<Result<_>>()?;
                let me = (r.u16()?, r.str()?, r.str()?);
                Ok(LeagueReply::Top { rows, me, tail: r.u32()? })
            }
            _ => Err(Error::Unsupported("4B reply kind other than 1, 4, 5")),
        }
    }
}
