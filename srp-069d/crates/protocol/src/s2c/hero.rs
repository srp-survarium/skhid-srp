//! Hero of the day (`A6`, `A7`, `A9`) and the store's action data (`BC`). Layouts PROVEN from the exe (docs/notes/
//! docs/notes/hero-of-the-day.md, docs/notes/actions.md); each is pushed as an id the crate does not otherwise model (`S2c::Raw`).
//!
//! * `A6` (reader 0x200188890): `u8 show`; show: `u8 n` (n <= 9), n x `{u8 min_level, u8 max_level, u8 max_player_level,
//!   u8 m (<= 3), m x u8 bonus percent}`. UI 0x2001CA550 builds `root.set_hero_of_the_day_data`; `show` is
//!   `root.show_hero_of_the_day` (the lobby's "Heroes of the Day" entry and its booster slot).
//! * `A7` (reader 0x2001889F0, UI 0x2001CA8F0): `u8 groups` (<= 2), `u8 skip`; skip == 0: `u32 id` (the client caches it
//!   at +0x7A3968 and sends it back in `48 2B`), 2 groups x 3 nominations x 3 places x `{u32 score, str128 name,
//!   str32 abbr}` (score 0 = empty place); then `u8 own`; own != 0: 2 x 3 x `u32` (the player's own value per group and
//!   nomination), `u8`, `u8` (kept at +0x7A4638 / +0x7A4639, copied into chat lines).
//! * `A9` (reader inline 0x20018E8FB, UI 0x2001CAC40): `u8 kind` (1 friends, 2 clan), `u8 flag` (0 = nothing), 3
//!   nominations x 3 places x `{f64 public id, u32 score}`, `u32` (kept at +0x7A4618 / +0x7A461C, sent back in `48 2C`).
//! * `BC` (reader 0x200196540): sub 0 `u32 tab, u32 len, len bytes (binary config), u8 more, [u32 a, u32 b]`; sub 1
//!   `u32 data id, u32 chest item id (0xFFFFFFFF = none), u8 result` (`root.set_buy_action_result`).

use crate::s2c::S2c;
use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

pub const HERO_CONFIG: u8 = 0xA6;
pub const HERO_DATA: u8 = 0xA7;
pub const HERO_FRIENDS: u8 = 0xA9;
pub const STORE_ACTIONS: u8 = 0xBC;

/// One competitive group of `A6`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroGroupInfo {
    pub min_level: u8,
    pub max_level: u8,
    pub max_player_level: u8,
    /// bonus percent per place, best first (at most 3)
    pub bonus: Vec<u8>,
}

/// `A6`: `None` hides the entry (flag 0).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroConfig(pub Option<Vec<HeroGroupInfo>>);

impl Encode for HeroConfig {
    fn encode(&self, w: &mut Writer) {
        match &self.0 {
            None => {
                w.u8(0);
            }
            Some(groups) => {
                if groups.len() > 9 {
                    w.fail(Error::Limit("A6 groups (9)"));
                }
                w.u8(1).count_u8(groups.len(), "A6 groups");
                for g in groups {
                    if g.bonus.len() > 3 {
                        w.fail(Error::Limit("A6 bonuses (3)"));
                    }
                    w.u8(g.min_level).u8(g.max_level).u8(g.max_player_level).count_u8(g.bonus.len(), "A6 bonuses");
                    for b in &g.bonus {
                        w.u8(*b);
                    }
                }
            }
        }
    }
}
impl Decode for HeroConfig {
    fn decode(r: &mut Reader) -> Result<Self> {
        if r.u8()? == 0 {
            return Ok(HeroConfig(None));
        }
        let n = r.u8()? as usize;
        let mut v = Vec::new();
        for _ in 0..n {
            let mut g = HeroGroupInfo { min_level: r.u8()?, max_level: r.u8()?, max_player_level: r.u8()?, bonus: vec![] };
            let m = r.u8()? as usize;
            for _ in 0..m {
                g.bonus.push(r.u8()?);
            }
            v.push(g);
        }
        Ok(HeroConfig(Some(v)))
    }
}
impl HeroConfig {
    pub fn push(&self) -> S2c {
        S2c::Raw { id: HERO_CONFIG, payload: self.to_bytes().unwrap_or_default() }
    }
}

/// One place of a nomination in `A7`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroPlace {
    pub score: u32,
    pub name: String,
    pub abbr: String,
}

/// The ranking part of `A7` (present when the client's cached id differs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroRanking {
    pub id: u32,
    /// `[group][nomination][place]`
    pub places: [[[HeroPlace; 3]; 3]; 2],
}

impl Default for HeroRanking {
    fn default() -> Self {
        HeroRanking { id: 0, places: Default::default() }
    }
}

/// The player's own part of `A7`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroOwn {
    /// `[group][nomination]`
    pub values: [[u32; 3]; 2],
    pub a: u8,
    pub b: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroData7 {
    /// groups the window shows (the UI loops this many times; at most 2)
    pub groups: u8,
    pub ranking: Option<HeroRanking>,
    pub own: Option<HeroOwn>,
}

impl Encode for HeroData7 {
    fn encode(&self, w: &mut Writer) {
        if self.groups > 2 {
            w.fail(Error::Limit("A7 groups (2)"));
        }
        w.u8(self.groups);
        match &self.ranking {
            None => {
                w.u8(1);
            }
            Some(rk) => {
                w.u8(0).u32(rk.id);
                for g in &rk.places {
                    for n in g {
                        for p in n {
                            w.u32(p.score).str_max(&p.name, 128).str_max(&p.abbr, 32);
                        }
                    }
                }
            }
        }
        match &self.own {
            None => {
                w.u8(0);
            }
            Some(o) => {
                w.u8(1);
                for g in &o.values {
                    for v in g {
                        w.u32(*v);
                    }
                }
                w.u8(o.a).u8(o.b);
            }
        }
    }
}
impl Decode for HeroData7 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let groups = r.u8()?;
        let mut m = HeroData7 { groups, ..Default::default() };
        if r.u8()? == 0 {
            let mut rk = HeroRanking { id: r.u32()?, ..Default::default() };
            for g in 0..2 {
                for n in 0..3 {
                    for p in 0..3 {
                        rk.places[g][n][p] = HeroPlace { score: r.u32()?, name: r.str()?, abbr: r.str()? };
                    }
                }
            }
            m.ranking = Some(rk);
        }
        if r.u8()? != 0 {
            let mut o = HeroOwn::default();
            for g in 0..2 {
                for n in 0..3 {
                    o.values[g][n] = r.u32()?;
                }
            }
            o.a = r.u8()?;
            o.b = r.u8()?;
            m.own = Some(o);
        }
        Ok(m)
    }
}
impl HeroData7 {
    pub fn push(&self) -> S2c {
        S2c::Raw { id: HERO_DATA, payload: self.to_bytes().unwrap_or_default() }
    }
}

/// `A9`: the friends (kind 1) or clan (kind 2) ranking; `None` = flag 0 (nothing).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeroFriends {
    pub kind: u8,
    /// `[nomination][place]` = (public id, score); score 0 = empty
    pub rows: Option<([[(f64, u32); 3]; 3], u32)>,
}

impl Encode for HeroFriends {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.kind);
        match &self.rows {
            None => {
                w.u8(0);
            }
            Some((rows, id)) => {
                w.u8(1);
                for n in rows {
                    for (pid, score) in n {
                        w.f64(*pid).u32(*score);
                    }
                }
                w.u32(*id);
            }
        }
    }
}
impl Decode for HeroFriends {
    fn decode(r: &mut Reader) -> Result<Self> {
        let kind = r.u8()?;
        if r.u8()? == 0 {
            return Ok(HeroFriends { kind, rows: None });
        }
        let mut rows = [[(0.0, 0); 3]; 3];
        for n in &mut rows {
            for p in n.iter_mut() {
                *p = (r.f64()?, r.u32()?);
            }
        }
        Ok(HeroFriends { kind, rows: Some((rows, r.u32()?)) })
    }
}
impl HeroFriends {
    pub fn push(&self) -> S2c {
        S2c::Raw { id: HERO_FRIENDS, payload: self.to_bytes().unwrap_or_default() }
    }
}

/// `BC`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreActions {
    /// sub 0: the content of one store tab (a binary config blob); `more` = `(a, b)` of the tail
    Tab { tab: u32, blob: Vec<u8>, more: Option<(u32, u32)> },
    /// sub 1: the answer of a buy
    Bought { data_id: u32, chest_item: u32, ok: bool },
}

impl Encode for StoreActions {
    fn encode(&self, w: &mut Writer) {
        match self {
            StoreActions::Tab { tab, blob, more } => {
                w.u8(0).u32(*tab).count_u32(blob.len()).bytes(blob);
                match more {
                    None => {
                        w.u8(0);
                    }
                    Some((a, b)) => {
                        w.u8(1).u32(*a).u32(*b);
                    }
                }
            }
            StoreActions::Bought { data_id, chest_item, ok } => {
                w.u8(1).u32(*data_id).u32(*chest_item).u8(u8::from(*ok));
            }
        }
    }
}
impl Decode for StoreActions {
    fn decode(r: &mut Reader) -> Result<Self> {
        match r.u8()? {
            0 => {
                let tab = r.u32()?;
                let n = r.u32()? as usize;
                let blob = r.bytes(n)?.to_vec();
                let more = if r.u8()? != 0 { Some((r.u32()?, r.u32()?)) } else { None };
                Ok(StoreActions::Tab { tab, blob, more })
            }
            1 => Ok(StoreActions::Bought { data_id: r.u32()?, chest_item: r.u32()?, ok: r.u8()? != 0 }),
            _ => Err(Error::Invalid("BC sub")),
        }
    }
}
impl StoreActions {
    pub fn push(&self) -> S2c {
        S2c::Raw { id: STORE_ACTIONS, payload: self.to_bytes().unwrap_or_default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a6_layout() {
        let m = HeroConfig(Some(vec![HeroGroupInfo { min_level: 3, max_level: 5, max_player_level: 100, bonus: vec![25, 20, 15] }]));
        assert_eq!(m.to_bytes().unwrap(), [1, 1, 3, 5, 100, 3, 25, 20, 15]);
        assert_eq!(HeroConfig::from_bytes(&m.to_bytes().unwrap()).unwrap(), m);
        assert_eq!(HeroConfig(None).to_bytes().unwrap(), [0]);
    }

    #[test]
    fn a7_round_trip_and_sizes() {
        let mut rk = HeroRanking { id: 7, ..Default::default() };
        rk.places[0][1][2] = HeroPlace { score: 9, name: "ab".into(), abbr: "X".into() };
        let m = HeroData7 { groups: 1, ranking: Some(rk), own: Some(HeroOwn { values: [[1, 2, 3], [0; 3]], a: 0, b: 0 }) };
        let b = m.to_bytes().unwrap();
        // 1 + 1 + 4 + 18 x (4 + 1 + 1) + extra 3 bytes of the filled place + 1 + 24 + 2
        assert_eq!(b.len(), 2 + 4 + 18 * 6 + 3 + 1 + 24 + 2);
        assert_eq!(HeroData7::from_bytes(&b).unwrap(), m);
        // the smallest message of the exe: no data, no own part
        assert_eq!(HeroData7 { groups: 0, ranking: None, own: None }.to_bytes().unwrap(), [0, 1, 0]);
    }

    #[test]
    fn a9_and_bc_round_trip() {
        let mut rows = [[(0.0, 0); 3]; 3];
        rows[1][0] = (f64::from_bits(0x5000_0000_0000_0007), 12);
        let m = HeroFriends { kind: 2, rows: Some((rows, 99)) };
        let b = m.to_bytes().unwrap();
        assert_eq!(b.len(), 2 + 9 * 12 + 4);
        assert_eq!(HeroFriends::from_bytes(&b).unwrap(), m);
        let t = StoreActions::Tab { tab: 3, blob: vec![1, 2, 3], more: None };
        assert_eq!(t.to_bytes().unwrap(), [0, 3, 0, 0, 0, 3, 0, 0, 0, 1, 2, 3, 0]);
        let k = StoreActions::Bought { data_id: 5, chest_item: u32::MAX, ok: false };
        assert_eq!(StoreActions::from_bytes(&k.to_bytes().unwrap()).unwrap(), k);
    }
}
