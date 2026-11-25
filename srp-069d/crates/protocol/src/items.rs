//! Item records, profiles and the account body shared by S->C 87, 88, 89, `83 45 0F`, `83 46 00` and the
//! match options (reader 0x2001263D0 record, 0x200125710 profile, 0x200125460 account).

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;
use std::collections::BTreeMap;

/// One element of the record's module list (n4). PROVEN (0x2001263D0).
/// On the wire: `u16 module dict, u32 module uid, u32 0, str slot id, u8 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleElem {
    /// Dictionary id of the installed module (+0x28).
    pub dict: u16,
    /// Item uid of the module (+0x20).
    pub uid: u32,
    /// Stored at +0x24, meaning unknown (sent as 0).
    pub f24: u32,
    /// Slot id on the weapon, at most 32 bytes (`weapon slots[].id` of the game options).
    pub slot: String,
    /// Module modifiers (group, grade), at most 4.
    pub mods: Vec<(u16, u16)>,
}

/// Decal / camouflage attachment (n3), 15 bytes. PROVEN layout, meaning LIKELY (Flash `camo_and_decals {id, param}`).
#[derive(Debug, Clone, PartialEq)]
pub struct Decal {
    pub id: u32,
    pub param: u16,
    pub x: f64,
    pub y: u8,
}

/// Item record, identical in 89, 87, `83 45 0F` and `83 46 00`. PROVEN (0x2001263D0).
///
/// `dict == 0` is the short form: only the first 12 bytes are written (the reader stops there; the item is
/// unusable and dict 0 crashes the UI fill, so only for tests). Limits: n1 <= 8, n2 <= 6, n3 <= 3, n4 <= 20.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemRecord {
    /// Instance id (+0), unique, >= 1; key of the 87/89 merge and of every 45/83 45 request.
    pub uid: u32,
    /// +4: for a module the uid of the weapon it is installed on (Flash `is_attached_to`), else 0. PROVEN.
    pub attached_to: u32,
    /// +8: "is_bounded_to", dict id a module is bound to, 0 = unbound (LIKELY meaning).
    pub bound: u16,
    /// Dictionary id (+0xA); must exist in the client's dictionary (unknown = lobby crash).
    pub dict: u16,
    /// +0xC: stacks = count; every other item = its original dict id; weapons = base item dict id
    /// (PROVEN by a client trace).
    pub value: u32,
    /// Modifiers (group, grade), at most 8 (Flash "modifiers"; workshop upgrades).
    pub mods: Vec<(u16, u16)>,
    /// n2: (index, value) pairs, meaning unknown, at most 6. Sent empty.
    pub n2: Vec<(u8, f32)>,
    /// n3: decals, at most 3.
    pub decals: Vec<Decal>,
    /// n4: installed modules, at most 20.
    pub modules: Vec<ModuleElem>,
}

impl Encode for ItemRecord {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.uid).u32(self.attached_to).u16(self.bound).u16(self.dict);
        if self.dict == 0 {
            return;
        }
        w.u32(self.value);
        if self.mods.len() > 8 {
            w.fail(Error::Limit("item modifiers (8)"));
        }
        w.count_u8(self.mods.len(), "item modifiers");
        for &(g, r) in &self.mods {
            w.u16(g).u16(r);
        }
        if self.n2.len() > 6 {
            w.fail(Error::Limit("item n2 (6)"));
        }
        w.count_u8(self.n2.len(), "item n2");
        for &(i, v) in &self.n2 {
            w.u8(i).f32(v);
        }
        if self.decals.len() > 3 {
            w.fail(Error::Limit("item decals (3)"));
        }
        w.count_u8(self.decals.len(), "item decals");
        for d in &self.decals {
            w.u32(d.id).u16(d.param).f64(d.x).u8(d.y);
        }
        if self.modules.len() > 20 {
            w.fail(Error::Limit("item modules (20)"));
        }
        w.count_u8(self.modules.len(), "item modules");
        for m in &self.modules {
            w.u16(m.dict).u32(m.uid).u32(m.f24);
            if m.slot.len() > 32 {
                w.fail(Error::Limit("module slot id (32)"));
            }
            w.str(&m.slot);
            if m.mods.len() > 4 {
                w.fail(Error::Limit("module modifiers (4)"));
            }
            w.count_u8(m.mods.len(), "module modifiers");
            for &(g, r) in &m.mods {
                w.u16(g).u16(r);
            }
        }
    }
}

impl Decode for ItemRecord {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut it = ItemRecord { uid: r.u32()?, attached_to: r.u32()?, bound: r.u16()?, dict: r.u16()?, ..Default::default() };
        if it.dict == 0 {
            return Ok(it);
        }
        it.value = r.u32()?;
        let n1 = r.u8()? as usize;
        if n1 > 8 {
            return Err(Error::Limit("item modifiers (8)"));
        }
        for _ in 0..n1 {
            it.mods.push((r.u16()?, r.u16()?));
        }
        let n2 = r.u8()? as usize;
        if n2 > 6 {
            return Err(Error::Limit("item n2 (6)"));
        }
        for _ in 0..n2 {
            it.n2.push((r.u8()?, r.f32()?));
        }
        let n3 = r.u8()? as usize;
        if n3 > 3 {
            return Err(Error::Limit("item decals (3)"));
        }
        for _ in 0..n3 {
            it.decals.push(Decal { id: r.u32()?, param: r.u16()?, x: r.f64()?, y: r.u8()? });
        }
        let n4 = r.u8()? as usize;
        if n4 > 20 {
            return Err(Error::Limit("item modules (20)"));
        }
        for _ in 0..n4 {
            let mut m = ModuleElem { dict: r.u16()?, uid: r.u32()?, f24: r.u32()?, slot: r.str()?, mods: vec![] };
            let k = r.u8()? as usize;
            if k > 4 {
                return Err(Error::Limit("module modifiers (4)"));
            }
            for _ in 0..k {
                m.mods.push((r.u16()?, r.u16()?));
            }
            it.modules.push(m);
        }
        Ok(it)
    }
}

/// `u8 n, n x item record` (the item list of `83 45 0F` after its leading byte, and of `83 46 00`).
pub fn write_records(w: &mut Writer, recs: &[ItemRecord]) {
    w.count_u8(recs.len(), "item records");
    for it in recs {
        it.encode(w);
    }
}

pub fn read_records(r: &mut Reader) -> Result<Vec<ItemRecord>> {
    let n = r.u8()? as usize;
    r.check_count(n, 12)?;
    (0..n).map(|_| ItemRecord::decode(r)).collect()
}

/// Equipment profile (loadout). PROVEN (0x200125710):
/// `u8 kind, u8 flag, u32 id, str128 name, u32 slot mask, per set bit (u16 index, u16 count), 18-byte modifier
/// mask + one f32 per set bit, str32 voice`.
///
/// `index` is the position in the item list OF THE SAME MESSAGE (87's own table), not in 89. The second `u16`
/// is the rounds loaded for ammo slots 8/9/11/12 and the count of quick slots 13..20.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// First byte; 2 = skipped by the slot check; sent as 0.
    pub kind: u8,
    /// Second byte (bool); 1 = profile available, runs the slot validation.
    pub flag: u8,
    pub id: u32,
    pub name: String,
    /// Slot number (0..22) -> (item index, count).
    pub slots: BTreeMap<u8, (u16, u16)>,
    /// Modifier id (0..137) -> value; bit i of the 18-byte mask. Empty for the lobby.
    pub mods: BTreeMap<u16, f32>,
    /// str32: AI sound scheme in matches, empty in the lobby.
    pub voice: String,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { kind: 0, flag: 1, id: 1, name: "p".into(), slots: BTreeMap::new(), mods: BTreeMap::new(), voice: String::new() }
    }
}

impl Encode for Profile {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.kind).u8(self.flag).u32(self.id).str_max(&self.name, 128);
        let mut mask = 0u32;
        for &s in self.slots.keys() {
            if s > 22 {
                w.fail(Error::Invalid("profile slot above 22"));
            } else {
                mask |= 1 << s;
            }
        }
        w.u32(mask);
        for &(i, c) in self.slots.values() {
            w.u16(i).u16(c);
        }
        let mut bits = [0u8; 18];
        for &m in self.mods.keys() {
            if m >= 138 {
                w.fail(Error::Invalid("modifier id above 137"));
            } else {
                bits[(m >> 3) as usize] |= 1 << (m & 7);
            }
        }
        w.bytes(&bits);
        for v in self.mods.values() {
            w.f32(*v);
        }
        w.str_max(&self.voice, 32);
    }
}

impl Decode for Profile {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut p = Profile { kind: r.u8()?, flag: r.u8()?, id: r.u32()?, name: r.str()?, ..Default::default() };
        let mask = r.u32()?;
        for s in 0..23u8 {
            if mask & (1 << s) != 0 {
                p.slots.insert(s, (r.u16()?, r.u16()?));
            }
        }
        let bits: [u8; 18] = r.array()?;
        for m in 0..138u16 {
            if bits[(m >> 3) as usize] & (1 << (m & 7)) != 0 {
                p.mods.insert(m, r.f32()?);
            }
        }
        p.voice = r.str()?;
        Ok(p)
    }
}

/// Account body (reader 0x200125460) without the AI block: the part of 87 (and 88) up to and including the
/// profile and item lists. PROVEN field order (names after the client offsets).
/// A player record with `ai_id != 0xFFFFFFFF` is followed by an AI block (reader 0x200125B40): not modelled here,
/// that belongs to the match-options builder; it is refused with [`Error::Unsupported`].
#[derive(Debug, Clone, PartialEq)]
pub struct AccountBody {
    /// First byte: team in match records, 0 in 87.
    pub team: u8,
    pub name: String,
    /// str32 clan abbreviation.
    pub clan: String,
    /// 8 bytes; the public account id in match records.
    pub account_id: u64,
    pub f2d8: u32,
    pub f2dc: u32,
    /// +0x2E0: AI id, `0xFFFFFFFF` = human.
    pub ai_id: u32,
    pub b2f5: u8,
    pub b2f6: u8,
    pub b2ef: u8,
    pub f2d0: u32,
    pub f2d4: u32,
    pub f2f8: u16,
    pub profiles: Vec<Profile>,
    pub items: Vec<ItemRecord>,
}

impl Default for AccountBody {
    fn default() -> Self {
        AccountBody {
            team: 0,
            name: String::new(),
            clan: String::new(),
            account_id: 0,
            f2d8: 0,
            f2dc: 0,
            ai_id: 0xFFFF_FFFF,
            b2f5: 0,
            b2f6: 0,
            b2ef: 0,
            f2d0: 0,
            f2d4: 0,
            f2f8: 0,
            profiles: vec![],
            items: vec![],
        }
    }
}

impl Encode for AccountBody {
    fn encode(&self, w: &mut Writer) {
        if self.ai_id != 0xFFFF_FFFF {
            w.fail(Error::Unsupported("AI block after a player record"));
        }
        w.u8(self.team).str_max(&self.name, 128).str_max(&self.clan, 32).u64(self.account_id);
        w.u32(self.f2d8).u32(self.f2dc).u32(self.ai_id);
        w.u8(self.b2f5).u8(self.b2f6).u8(self.b2ef);
        w.u32(self.f2d0).u32(self.f2d4).u16(self.f2f8);
        // the client stores the profiles and items of a body in a fixed area with no bounds check (0x2001256E3: profile
        // 0x2A08 bytes, item 0x7C0 bytes; 9 profiles + 189 items = 0x73488 bytes): never more
        if self.profiles.len() * 0x2A08 + self.items.len() * 0x7C0 > 0x73488 {
            w.fail(Error::Limit("account body larger than the client's area (9 profiles + 189 items)"));
        }
        w.count_u8(self.profiles.len(), "profiles");
        w.count_u8(self.items.len(), "account items");
        for p in &self.profiles {
            p.encode(w);
        }
        for it in &self.items {
            it.encode(w);
        }
    }
}

impl Decode for AccountBody {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut a = AccountBody {
            team: r.u8()?,
            name: r.str()?,
            clan: r.str()?,
            account_id: r.u64()?,
            f2d8: r.u32()?,
            f2dc: r.u32()?,
            ai_id: r.u32()?,
            b2f5: r.u8()?,
            b2f6: r.u8()?,
            b2ef: r.u8()?,
            f2d0: r.u32()?,
            f2d4: r.u32()?,
            f2f8: r.u16()?,
            ..Default::default()
        };
        if a.ai_id != 0xFFFF_FFFF {
            return Err(Error::Unsupported("AI block after a player record"));
        }
        let np = r.u8()? as usize;
        let ni = r.u8()? as usize;
        for _ in 0..np {
            a.profiles.push(Profile::decode(r)?);
        }
        for _ in 0..ni {
            a.items.push(ItemRecord::decode(r)?);
        }
        Ok(a)
    }
}

/// The five level fields shared by 87 (tail) and 8C. PROVEN order (fill_char_info 0x2001C1328):
/// `u8 level (+0x72501D), u8 skill points (+0x72501C), u32 experience (+0x725010), u32 previous level threshold
/// (+0x725018), u32 next level threshold (+0x725014)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevelFields {
    pub level: u8,
    pub points: u8,
    pub exp: u32,
    pub prev: u32,
    pub next: u32,
}

impl Encode for LevelFields {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.level).u8(self.points).u32(self.exp).u32(self.prev).u32(self.next);
    }
}

impl Decode for LevelFields {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(LevelFields { level: r.u8()?, points: r.u8()?, exp: r.u32()?, prev: r.u32()?, next: r.u32()? })
    }
}
