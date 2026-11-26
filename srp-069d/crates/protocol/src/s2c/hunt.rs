//! Treasure hunt (item search roulette) and faction battle messages. Layouts PROVEN from the exe, meanings from the
//! lobby movie (docs/notes/items.md section 3a, docs/notes/progression.md).
//!
//! * `AA` (reader 0x200188BD0; the full-state reader is 0x200180F20 + 0x200180FA0):
//!   * sub 0 = state: `str32 config name, u32 a`; `a == 0` ends the message (no hunt). Else `u32 b, u32 c, u8 d,
//!     u32 time_left, u8 level, u8 page, u8 max_level, u8 min_level, u8 n, n x { u16 dict, u8 not_found, u8 m,
//!     m x { u16, u16 } }`. The UI builder (0x20024E260) calls `root.set_items_search_data`.
//!   * sub 1 = counts only: `str32 name, u32 a, [a != 0: u32 b, u32 c, u8 d]`; builds the `ItemSearch` object
//!     {shop_maps_count b, free_maps_count c, used_maps_count min(d, 2 - c), free_maps_max, max_free_drop_cards}
//!     and calls `root.update_items_search_data`.
//! * `84 x 46` (generic denied reader 0x20018FE10, shown by 0x176400 case 0x46 at 0x176BF8): code 0, `u8 a != 0xFF`,
//!   `u32 reason` 0 failed, 1 not enough drop cards, 2 not enough money, 3 all treasures opened; the client then calls
//!   `root.unblock_roulette`.
//! * faction battle status = the optional 40 bytes of `8E` (`u8 k != 0`, 10 x u32; reader 0x20018AC70,
//!   `root.faction_battle_status_received`, 0x2001CAFC0): `[time_left, mask, rep1, got1, rep2, got2, rep3, got3, rep4, got4]`.
//! * `83 4A 05` (0x191513): `u8 n, n x item record` -> `root.faction_battle_reward_received` (shows the dict / amount).

use crate::items::ItemRecord;
use crate::s2c::{Denied, S2c};
use crate::wire::{Decode, Encode, Reader, Result, Writer};

/// `AA` message id.
pub const WEEKLY_DROP: u8 = 0xAA;

/// One item of the page as the client lists it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HuntItem {
    pub dict: u16,
    /// the client's `owned` = this byte is 0 (0x20024E896)
    pub not_found: bool,
    /// (group, grade) pairs the window shows as a module's modifiers (0x2002477E0; the UI builder uses them for the
    /// module categories 39, 41..53 only); at most [`HUNT_ITEM_MODS`]
    pub mods: Vec<(u16, u16)>,
}

/// Pairs one page entry carries at most (the reader 0x200180FA0 fills a fixed buffer of 4 and asserts beyond it).
pub const HUNT_ITEM_MODS: usize = 4;

/// The counts of `AA` sub 0 / sub 1: `b` = paid drop cards held, `c` = free drop cards held, `d` = free cards used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HuntCounts {
    pub b: u32,
    pub c: u32,
    pub d: u8,
}

/// `AA` sub 0 body after the counts (present when `a != 0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuntPage {
    /// seconds the hunt still runs (the lobby shows it as a countdown in minutes)
    pub time_left: u32,
    pub level: u8,
    pub page: u8,
    pub max_level: u8,
    pub min_level: u8,
    pub items: Vec<HuntItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeeklyDrop {
    /// sub 0: `a` non-zero = a hunt exists; `None` page/counts = `a == 0`
    State { name: String, a: u32, counts: HuntCounts, page: Option<HuntPage> },
    /// sub 1
    Counts { name: String, a: u32, counts: HuntCounts },
}

impl Encode for WeeklyDrop {
    fn encode(&self, w: &mut Writer) {
        match self {
            WeeklyDrop::State { name, a, counts, page } => {
                w.u8(0).str_max(name, 31).u32(*a);
                if *a != 0 {
                    w.u32(counts.b).u32(counts.c).u8(counts.d);
                    let p = page.as_ref();
                    w.u32(p.map_or(0, |p| p.time_left));
                    w.u8(p.map_or(0, |p| p.level)).u8(p.map_or(0, |p| p.page)).u8(p.map_or(0, |p| p.max_level)).u8(p.map_or(0, |p| p.min_level));
                    let items = p.map_or(&[][..], |p| &p.items[..]);
                    w.count_u8(items.len(), "hunt items");
                    for it in items {
                        let mods = &it.mods[..it.mods.len().min(HUNT_ITEM_MODS)];
                        w.u16(it.dict).u8(it.not_found as u8).u8(mods.len() as u8);
                        for &(g, v) in mods {
                            w.u16(g).u16(v);
                        }
                    }
                }
            }
            WeeklyDrop::Counts { name, a, counts } => {
                w.u8(1).str_max(name, 31).u32(*a);
                if *a != 0 {
                    w.u32(counts.b).u32(counts.c).u8(counts.d);
                }
            }
        }
    }
}

impl Decode for WeeklyDrop {
    fn decode(r: &mut Reader) -> Result<Self> {
        let sub = r.u8()?;
        let name = r.str()?;
        let a = r.u32()?;
        let counts = if a != 0 { HuntCounts { b: r.u32()?, c: r.u32()?, d: r.u8()? } } else { HuntCounts::default() };
        match sub {
            0 => {
                let page = if a != 0 {
                    let time_left = r.u32()?;
                    let (level, page, max_level, min_level) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
                    let n = r.u8()? as usize;
                    let mut items = Vec::new();
                    for _ in 0..n {
                        let dict = r.u16()?;
                        let not_found = r.u8()? != 0;
                        let m = r.u8()? as usize;
                        let mut mods = Vec::with_capacity(m);
                        for _ in 0..m {
                            mods.push((r.u16()?, r.u16()?));
                        }
                        items.push(HuntItem { dict, not_found, mods });
                    }
                    Some(HuntPage { time_left, level, page, max_level, min_level, items })
                } else {
                    None
                };
                Ok(WeeklyDrop::State { name, a, counts, page })
            }
            1 => Ok(WeeklyDrop::Counts { name, a, counts }),
            _ => Err(crate::Error::Invalid("AA sub")),
        }
    }
}

impl WeeklyDrop {
    /// The message as the server pushes it.
    pub fn push(&self) -> S2c {
        S2c::Raw { id: WEEKLY_DROP, payload: self.to_bytes().unwrap_or_default() }
    }
}

/// Why a search or a level change was refused (the `84 x 46` reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDenied {
    Failed = 0,
    NotEnoughCards = 1,
    NotEnoughMoney = 2,
    AllOpened = 3,
}

impl SearchDenied {
    pub fn message(self) -> S2c {
        S2c::Denied(Denied::numeric(0x46, 0, 0, self as u32))
    }
}

/// The ten words of the faction battle status inside `8E`.
pub fn battle_status(time_left: u32, mask: u32, reputation: [u32; 4], received: [u32; 4]) -> [u32; 10] {
    let mut v = [0u32; 10];
    v[0] = time_left;
    v[1] = mask;
    for i in 0..4 {
        v[2 + 2 * i] = reputation[i];
        v[3 + 2 * i] = received[i];
    }
    v
}

/// `83 4A 05` data after the request id: `05 u8 n, n x item record`.
pub fn faction_reward_reply(records: &[ItemRecord]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(5).count_u8(records.len(), "faction battle rewards");
    for r in records {
        r.encode(&mut w);
    }
    w.finish().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip_and_layout() {
        let m = WeeklyDrop::State {
            name: "weekly_drop".into(),
            a: 7,
            counts: HuntCounts { b: 3, c: 1, d: 1 },
            page: Some(HuntPage {
                time_left: 3600,
                level: 2,
                page: 5,
                max_level: 4,
                min_level: 1,
                items: vec![HuntItem { dict: 952, not_found: true, mods: vec![] }, HuntItem { dict: 214, not_found: false, mods: vec![(312, 2), (403, 5)] }],
            }),
        };
        let b = m.to_bytes().unwrap();
        // sub, name, a, b, c, d, time, level, page, max, min, n, 2 x (u16, flag, m)
        assert_eq!(b[0], 0);
        assert_eq!(b[1], 11);
        assert_eq!(WeeklyDrop::from_bytes(&b).unwrap(), m);
        assert_eq!(b.len(), 1 + 1 + 11 + 4 + 4 + 4 + 1 + 4 + 4 + 1 + 2 * 4 + 2 * 4);
        // the second entry: dict, flag, m = 2, (312, 2), (403, 5)
        assert_eq!(&b[b.len() - 12..], &[214, 0, 0, 2, 0x38, 1, 2, 0, 0x93, 1, 5, 0]);
    }

    #[test]
    fn no_hunt_and_counts() {
        let none = WeeklyDrop::State { name: "weekly_drop".into(), a: 0, counts: HuntCounts::default(), page: None };
        assert_eq!(none.to_bytes().unwrap().len(), 1 + 1 + 11 + 4);
        assert_eq!(WeeklyDrop::from_bytes(&none.to_bytes().unwrap()).unwrap(), none);
        let c = WeeklyDrop::Counts { name: "weekly_drop".into(), a: 9, counts: HuntCounts { b: 1, c: 2, d: 0 } };
        assert_eq!(WeeklyDrop::from_bytes(&c.to_bytes().unwrap()).unwrap(), c);
    }

    #[test]
    fn denied_and_battle_words() {
        assert!(matches!(SearchDenied::AllOpened.message(), S2c::Denied(_)));
        let v = battle_status(10, 0b11000, [1, 2, 3, 4], [0, 1, 3, 7]);
        assert_eq!(v, [10, 0b11000, 1, 0, 2, 1, 3, 3, 4, 7]);
    }
}
