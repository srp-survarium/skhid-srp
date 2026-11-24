//! `83 x <request id> ...` request answers and `84 x <request id> ...` denials.
//! Dispatcher 0x20018F264 (PROVEN). Payloads here start at the request id byte (right after `83 x`).
//! Unknown request ids or subs the client ignores are kept as [`Reply::Raw`].

use super::league::LeagueReply;
use crate::items::{read_records, write_records, ItemRecord};
use crate::lzma2;
use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// `83 45 <sub>` inventory action results (`process_succeeded_inventory_action`, 0x2001901B0, PROVEN order;
/// meaning LIKELY unless noted).
#[derive(Debug, Clone, PartialEq)]
pub enum Inventory45 {
    /// Sub 1 camouflage / decal attached: `u32 item, u32 id (the decal item's uid), u8 slot, f64 x`. The client looks both
    /// items up in its inventory, erases an entry of `item` with the same slot, and adds `{id, dict of the decal item,
    /// x, slot}` (PROVEN reader 0x200190238).
    Camo { item: u32, id: u32, x: u8, value: f64 },
    /// Sub 2 camouflage / decal removed: `u32 item, u32 id, u8 slot`; the client erases the entry of the item whose
    /// `id` and slot match (PROVEN reader 0x200190422).
    CamoDrop { item: u32, id: u32, slot: u8 },
    /// Sub 3 item used: `u32 item` (count - 1, 0 = skip).
    Used { item: u32 },
    /// Sub 5 reroll options: `u32 item, 3 x (u16 group, u16 grade)`. PROVEN reader, 
    RerollOptions { item: u32, options: [(u16, u16); 3] },
    /// Sub 6 reroll result: `u32 item, u16 a, u16 b, u16 c, u16 d`: replaces (a,b) by (c,d); no append (0x190A9D).
    RerollDone { item: u32, old: (u16, u16), new: (u16, u16) },
    /// Sub 7 craft (workshop upgrade) offer: `u8 result, u32 item, [result not 0 and not 2: u32 n, n x (u16 group,
    /// u16 grade)]`. Result 1 = choose screen (list SORTED by (group, grade) = the order shown, PROVEN docs/notes/workshop.md),
    /// 0 = failed roll (state FAIL, item unchanged), 2 = cannot craft. PROVEN reader 0x200190DAB..0x200190EF6.
    Craft { result: u8, item: u32, offer: Vec<(u16, u16)> },
    /// Sub 8 craft pick applied: `u32 item, u16 a, u16 b, u16 c, u16 d`: replaces (a,b) by (c,d) or appends (c,d)
    /// when (a,b) is absent (e.g. (0,0) as old). PROVEN reader 0x190F51.
    CraftDone { item: u32, old: (u16, u16), new: (u16, u16) },
    /// Sub 0xF item list change: `u8 x, u8 n, n x item record`; known uid = overwritten, unknown = added, record
    /// count (+0xC) 0 = removed. PROVEN reader 0x1910FD.
    Records { x: u8, items: Vec<ItemRecord> },
    /// Sub 0x0B PvE end-screen drops (`set_dropped_pve_items`): `u8 n, n x item record, u32 cost, u8 flag`. PROVEN reader
    /// 0x200193DF0 (docs/notes/pve.md "End screen"): the records are read like the other item records (0x2001263D0) and
    /// only shown, `cost` = the price of the next reroll (HUD `rerollCost`; the client also shows it as affordable when
    /// `cost <= its premium money`), `flag` = the run still has locked chests (HUD `hasLockedChests`, shows the "open
    /// all" button). A server push to the in-match HUD movie: the client never sends `45 0B`.
    PveDrops { items: Vec<ItemRecord>, cost: u32, flag: bool },
    /// Sub 0x0A match drops (reader 0x200194130, PROVEN): `u8 flag`; flag 0: `u8 n, n x {item record, u32 amount,
    /// u32 bonus}`. The amount overwrites the record's +0xC, then the record is added to the inventory (0x200193CB0, which
    /// skips the money / event currency ids 214 / 215 / 925 / 951 / 1138 / 1139 / 1356). With flag 0 the client also calls
    /// `root.fill_match_drop_items(items, match id of the last match options)` (0x200179AE0): one entry per record whose
    /// `bound` u16 is 0, shown count = amount - bonus, `bonus_amount` = bonus; the victory screen shows them in its extra
    /// rewards panel (`fillVictoryExtraRewards`). Flag 1 = the drop was refused for inactivity
    /// (`st_reject_drop_for_inactivity`), nothing follows.
    MatchDrops { flag: u8, items: Vec<MatchDrop> },
    /// Sub 0x10 module upgrade: `u8 result, u32 module uid, [result not 0/2: u8 n (<= 4), n x (u16, u16)]`.
    /// Result 2 = refused ("cannot_craft_item"). PROVEN reader 0x190C33.
    ModuleUpgrade { result: u8, uid: u32, mods: Vec<(u16, u16)> },
    /// Sub 0x11 / 0x12 skin change / preview: an item record.
    Skin { sub: u8, item: ItemRecord },
    /// Any other sub (0, 2, 4, 9, 0xC..0xE and > 0x12 are ignored or not decoded by the client).
    Other { sub: u8, data: Vec<u8> },
}

/// One entry of [`Inventory45::MatchDrops`]: the item, the amount received (stacks: the count; for other items the
/// record's own +0xC value, the screen ignores it and shows the condition 100) and the bonus part of it (premium, worn
/// items), which the screen shows separately.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchDrop {
    pub item: ItemRecord,
    pub amount: u32,
    pub bonus: u32,
}

/// `83 46 <sub>` shop results (`0x200192DF0`, PROVEN).
#[derive(Debug, Clone, PartialEq)]
pub enum Shop46 {
    /// Sub 0 (answer to 46 00 buy): `u8 flag, u8 n, n x item record`. Flag 0 runs the pending callback (unlocks the
    /// UI). Smallest `00 00 00` (refused: `flag 0, n 0`); bought: `00 00 01 <record>`.
    Buy { flag: u8, items: Vec<ItemRecord> },
    /// Sub 1 (answer to 46 01 sell): `u32 item id, u32 amount`; stacks lose the amount, other items are removed.
    Sell { item: u32, amount: u32 },
    /// Sub 6 chest opened: `u32 chest item uid, u16 chest dict id, u8 n, n x item record`. The client first removes
    /// one unit of the chest item (count - 1, record erased at 0; a non-stack item is erased), then adds the n
    /// records (same uid = count added, new uid = new item) and shows them (reader 0x2001930D8..0x200193626,
    /// `0x248F50(chest dict id, rewards)`). PROVEN reads; that the records are the rewards is LIKELY.
    Chest { item: u32, chest_dict: u16, rewards: Vec<ItemRecord> },
    /// Sub 7 item search result: `u8 a, u8 n, n x item record`; records are merged into the inventory like sub 6
    /// and handed to `root.set_items_search_result` (reader 0x200192E55..0x200193043). PROVEN reads, meaning of
    /// `a` unknown.
    Search { a: u8, items: Vec<ItemRecord> },
    /// Any other sub.
    Other { sub: u8, data: Vec<u8> },
}

/// `83 48 <sub>` account answers (0x200192060).
#[derive(Debug, Clone, PartialEq)]
pub enum Account48 {
    /// Sub 4 nickname validation: `str128 name, u8 flag (1 = nickname valid, LIKELY)`; flag 0: `u8 reason, u32 n,
    /// n x str128` (suggestions).
    Nickname { name: String, valid: u8, reason: u8, suggestions: Vec<String> },
    /// Sub 5 nickname change result: `u8 ok`; ok != 0: `u32 voucher uid, u8 face`. PROVEN reader 0x200192B68: ok = 0 only
    /// ends the wait (`root.set_account_face` with the old face); else the inventory row with that uid loses one
    /// (the row is removed at 0) and the face byte is stored.
    NicknameChanged { ok: u8, uid: u32, face: u8 },
    /// Sub 6 complaint / compliment added: `u8` (which value is which: LIKELY the request's own kind byte).
    Comment { kind: u8 },
    /// Sub 2 active profile: `u32 profile id`. The reader finds the profile record with that id (+0x29F8), makes it
    /// the active one and rebuilds the character (0x200192CDD, then 0x1BEC60). PROVEN.
    ProfileSet { profile: u32 },
    Other { sub: u8, data: Vec<u8> },
}

/// `83 55` achievements (0x200194C90).
#[derive(Debug, Clone, PartialEq)]
pub enum Achievements55 {
    /// Sub 3: `u8 n, n x {u8, u16}`; sent with n = 0.
    List { entries: Vec<(u8, u16)> },
    Other { sub: u8, data: Vec<u8> },
}

/// `83 5B` play / lobby-load answer. PROVEN layout: `u8 sub, u32 packed size, raw LZMA2`.
/// The unpacked blob is the account/match options structure read by 0x2001250E0 (smallest valid blob is 118 bytes);
/// building it from lobby data is the later "options builder" step, this type only carries the blob.
/// Any 5B answer (even sub 1 or 3) ends the Play-button block.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayBlob {
    pub sub: u8,
    pub blob: Vec<u8>,
}

impl PlayBlob {
    /// Smallest valid unpacked blob, 118 zero bytes (n = 0 account bodies, two empty strings): 
    pub fn minimal_blob() -> Vec<u8> {
        vec![0u8; 118]
    }
}

/// Reply body after `83 x`.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    /// 2F: `u32 a, u32 b`; b -> `root.set_mm_estimated_time`. Smallest 8 zero bytes. PROVEN.
    Queue { a: u32, eta: u32 },
    /// 30: nothing read (the clan match ready check: the client answers `62 22` ready by itself, PROVEN 0x20018F337).
    Ready30,
    Inventory(Inventory45),
    Shop(Shop46),
    Account(Account48),
    /// 4B league answers.
    League(LeagueReply),
    Achievements(Achievements55),
    /// 5B: see [`PlayBlob`].
    Play(PlayBlob),
    /// 60 sub 1: `u32 weapon uid` ("all ok, close workshop"). PROVEN reader 0x20018FF60.
    WorkshopDone { uid: u32 },
    /// Any other request id or undecoded sub: the bytes after the request id.
    Raw { request: u8, data: Vec<u8> },
}

fn pair(r: &mut Reader) -> Result<(u16, u16)> {
    Ok((r.u16()?, r.u16()?))
}

fn rest_vec(r: &mut Reader) -> Vec<u8> {
    r.rest().to_vec()
}

impl Encode for Reply {
    fn encode(&self, w: &mut Writer) {
        match self {
            Reply::Queue { a, eta } => {
                w.u8(0x2F).u32(*a).u32(*eta);
            }
            Reply::Ready30 => {
                w.u8(0x30);
            }
            Reply::Inventory(i) => {
                w.u8(0x45);
                match i {
                    Inventory45::Camo { item, id, x, value } => {
                        w.u8(1).u32(*item).u32(*id).u8(*x).f64(*value);
                    }
                    Inventory45::CamoDrop { item, id, slot } => {
                        w.u8(2).u32(*item).u32(*id).u8(*slot);
                    }
                    Inventory45::Used { item } => {
                        w.u8(3).u32(*item);
                    }
                    Inventory45::RerollOptions { item, options } => {
                        w.u8(5).u32(*item);
                        for &(g, r) in options {
                            w.u16(g).u16(r);
                        }
                    }
                    Inventory45::RerollDone { item, old, new } => {
                        w.u8(6).u32(*item).u16(old.0).u16(old.1).u16(new.0).u16(new.1);
                    }
                    Inventory45::Craft { result, item, offer } => {
                        w.u8(7).u8(*result).u32(*item);
                        if *result != 0 && *result != 2 {
                            w.count_u32(offer.len());
                            for &(g, r) in offer {
                                w.u16(g).u16(r);
                            }
                        }
                    }
                    Inventory45::CraftDone { item, old, new } => {
                        w.u8(8).u32(*item).u16(old.0).u16(old.1).u16(new.0).u16(new.1);
                    }
                    Inventory45::Records { x, items } => {
                        w.u8(0x0F).u8(*x);
                        write_records(w, items);
                    }
                    Inventory45::PveDrops { items, cost, flag } => {
                        w.u8(0x0B);
                        write_records(w, items);
                        w.u32(*cost).u8(*flag as u8);
                    }
                    Inventory45::MatchDrops { flag, items } => {
                        w.u8(0x0A).u8(*flag);
                        if *flag == 0 {
                            w.count_u8(items.len(), "match drops");
                            for d in items {
                                d.item.encode(w);
                                w.u32(d.amount).u32(d.bonus);
                            }
                        }
                    }
                    Inventory45::ModuleUpgrade { result, uid, mods } => {
                        w.u8(0x10).u8(*result).u32(*uid);
                        if *result != 0 && *result != 2 {
                            if mods.len() > 4 {
                                w.fail(Error::Limit("module upgrade modifiers (4)"));
                            }
                            w.count_u8(mods.len(), "module upgrade modifiers");
                            for &(g, r) in mods {
                                w.u16(g).u16(r);
                            }
                        }
                    }
                    Inventory45::Skin { sub, item } => {
                        w.u8(*sub);
                        item.encode(w);
                    }
                    Inventory45::Other { sub, data } => {
                        w.u8(*sub).bytes(data);
                    }
                }
            }
            Reply::Shop(s) => {
                w.u8(0x46);
                match s {
                    Shop46::Buy { flag, items } => {
                        w.u8(0).u8(*flag);
                        write_records(w, items);
                    }
                    Shop46::Sell { item, amount } => {
                        w.u8(1).u32(*item).u32(*amount);
                    }
                    Shop46::Chest { item, chest_dict, rewards } => {
                        w.u8(6).u32(*item).u16(*chest_dict);
                        write_records(w, rewards);
                    }
                    Shop46::Search { a, items } => {
                        w.u8(7).u8(*a);
                        write_records(w, items);
                    }
                    Shop46::Other { sub, data } => {
                        w.u8(*sub).bytes(data);
                    }
                }
            }
            Reply::Account(a) => {
                w.u8(0x48);
                match a {
                    Account48::Nickname { name, valid, reason, suggestions } => {
                        w.u8(4).str_max(name, 128).u8(*valid);
                        if *valid == 0 {
                            w.u8(*reason).count_u32(suggestions.len());
                            for s in suggestions {
                                w.str_max(s, 128);
                            }
                        }
                    }
                    Account48::NicknameChanged { ok, uid, face } => {
                        w.u8(5).u8(*ok);
                        if *ok != 0 {
                            w.u32(*uid).u8(*face);
                        }
                    }
                    Account48::Comment { kind } => {
                        w.u8(6).u8(*kind);
                    }
                    Account48::ProfileSet { profile } => {
                        w.u8(2).u32(*profile);
                    }
                    Account48::Other { sub, data } => {
                        w.u8(*sub).bytes(data);
                    }
                }
            }
            Reply::League(l) => {
                w.u8(0x4B);
                l.encode(w);
            }
            Reply::Achievements(a) => {
                w.u8(0x55);
                match a {
                    Achievements55::List { entries } => {
                        w.u8(3).count_u8(entries.len(), "achievements");
                        for &(a, b) in entries {
                            w.u8(a).u16(b);
                        }
                    }
                    Achievements55::Other { sub, data } => {
                        w.u8(*sub).bytes(data);
                    }
                }
            }
            Reply::Play(p) => {
                w.u8(0x5B).u8(p.sub);
                lzma2::write_packed(w, &p.blob);
            }
            Reply::WorkshopDone { uid } => {
                w.u8(0x60).u8(1).u32(*uid);
            }
            Reply::Raw { request, data } => {
                w.u8(*request).bytes(data);
            }
        }
    }
}

impl Decode for Reply {
    fn decode(r: &mut Reader) -> Result<Self> {
        let req = r.u8()?;
        Ok(match req {
            0x2F => Reply::Queue { a: r.u32()?, eta: r.u32()? },
            0x30 => Reply::Ready30,
            0x45 => {
                let sub = r.u8()?;
                Reply::Inventory(match sub {
                    1 => Inventory45::Camo { item: r.u32()?, id: r.u32()?, x: r.u8()?, value: r.f64()? },
                    2 => Inventory45::CamoDrop { item: r.u32()?, id: r.u32()?, slot: r.u8()? },
                    3 => Inventory45::Used { item: r.u32()? },
                    5 => Inventory45::RerollOptions { item: r.u32()?, options: [pair(r)?, pair(r)?, pair(r)?] },
                    6 => Inventory45::RerollDone { item: r.u32()?, old: pair(r)?, new: pair(r)? },
                    7 => {
                        let result = r.u8()?;
                        let item = r.u32()?;
                        let mut offer = Vec::new();
                        if result != 0 && result != 2 {
                            let n = r.u32()? as usize;
                            r.check_count(n, 4)?;
                            for _ in 0..n {
                                offer.push(pair(r)?);
                            }
                        }
                        Inventory45::Craft { result, item, offer }
                    }
                    8 => Inventory45::CraftDone { item: r.u32()?, old: pair(r)?, new: pair(r)? },
                    0x0B => Inventory45::PveDrops { items: read_records(r)?, cost: r.u32()?, flag: r.u8()? != 0 },
                    0x0A => {
                        let flag = r.u8()?;
                        let mut items = Vec::new();
                        if flag == 0 {
                            let n = r.u8()? as usize;
                            r.check_count(n, 20)?;
                            for _ in 0..n {
                                items.push(MatchDrop { item: ItemRecord::decode(r)?, amount: r.u32()?, bonus: r.u32()? });
                            }
                        }
                        Inventory45::MatchDrops { flag, items }
                    }
                    0x0F => Inventory45::Records { x: r.u8()?, items: read_records(r)? },
                    0x10 => {
                        let result = r.u8()?;
                        let uid = r.u32()?;
                        let mut mods = Vec::new();
                        if result != 0 && result != 2 {
                            let n = r.u8()? as usize;
                            if n > 4 {
                                return Err(Error::Limit("module upgrade modifiers (4)"));
                            }
                            for _ in 0..n {
                                mods.push(pair(r)?);
                            }
                        }
                        Inventory45::ModuleUpgrade { result, uid, mods }
                    }
                    0x11 | 0x12 => Inventory45::Skin { sub, item: ItemRecord::decode(r)? },
                    _ => Inventory45::Other { sub, data: rest_vec(r) },
                })
            }
            0x46 => {
                let sub = r.u8()?;
                Reply::Shop(match sub {
                    0 => Shop46::Buy { flag: r.u8()?, items: read_records(r)? },
                    1 => Shop46::Sell { item: r.u32()?, amount: r.u32()? },
                    6 => Shop46::Chest { item: r.u32()?, chest_dict: r.u16()?, rewards: read_records(r)? },
                    7 => Shop46::Search { a: r.u8()?, items: read_records(r)? },
                    _ => Shop46::Other { sub, data: rest_vec(r) },
                })
            }
            0x48 => {
                let sub = r.u8()?;
                Reply::Account(match sub {
                    4 => {
                        let name = r.str()?;
                        let valid = r.u8()?;
                        let (mut reason, mut suggestions) = (0, Vec::new());
                        if valid == 0 {
                            reason = r.u8()?;
                            let n = r.u32()? as usize;
                            r.check_count(n, 1)?;
                            for _ in 0..n {
                                suggestions.push(r.str()?);
                            }
                        }
                        Account48::Nickname { name, valid, reason, suggestions }
                    }
                    5 => {
                        let ok = r.u8()?;
                        if ok != 0 {
                            Account48::NicknameChanged { ok, uid: r.u32()?, face: r.u8()? }
                        } else {
                            Account48::NicknameChanged { ok, uid: 0, face: 0 }
                        }
                    }
                    6 => Account48::Comment { kind: r.u8()? },
                    2 => Account48::ProfileSet { profile: r.u32()? },
                    _ => Account48::Other { sub, data: rest_vec(r) },
                })
            }
            0x4B => Reply::League(LeagueReply::decode(r)?),
            0x55 => {
                let sub = r.u8()?;
                Reply::Achievements(match sub {
                    3 => {
                        let n = r.u8()? as usize;
                        Achievements55::List { entries: (0..n).map(|_| Ok((r.u8()?, r.u16()?))).collect::<Result<_>>()? }
                    }
                    _ => Achievements55::Other { sub, data: rest_vec(r) },
                })
            }
            0x5B => {
                let sub = r.u8()?;
                Reply::Play(PlayBlob { sub, blob: lzma2::read_packed(r)? })
            }
            0x60 => {
                if r.u8()? != 1 {
                    return Err(Error::Unsupported("83 60 sub other than 1"));
                }
                Reply::WorkshopDone { uid: r.u32()? }
            }
            _ => Reply::Raw { request: req, data: rest_vec(r) },
        })
    }
}

/// `84 x <request id> u8 code ...` operation denied. PROVEN (reader 0x20018FE10): code 0 or 2: `u8, u32`;
/// code 1: `str512`; any other code hits an "unintended default case" assert. Smallest
/// `84 x <id> 00 00 00 00 00 00`. Special ids: 0x5B clears the Play block; 0x5F -> `process_pve_action_denied`
/// (own layout `u8 1, u8 code`, [`DeniedBody::Pve`], see `s2c::pve`).
#[derive(Debug, Clone, PartialEq)]
pub struct Denied {
    pub request: u8,
    pub body: DeniedBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeniedBody {
    /// Code 0 or 2: `u8 a, u32 b`.
    Numeric { code: u8, a: u8, b: u32 },
    /// Code 1: error text (cap 512).
    Text(String),
    /// Request 0x5F only: `u8 1, u8 code` (PROVEN reader 0x20018F4BF).
    Pve { code: u8 },
}

impl Denied {
    pub fn numeric(request: u8, code: u8, a: u8, b: u32) -> Self {
        Denied { request, body: DeniedBody::Numeric { code, a, b } }
    }
}

impl Encode for Denied {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.request);
        match &self.body {
            DeniedBody::Numeric { code, a, b } => {
                if *code != 0 && *code != 2 {
                    w.fail(Error::Invalid("84 numeric code must be 0 or 2"));
                }
                w.u8(*code).u8(*a).u32(*b);
            }
            DeniedBody::Text(t) => {
                w.u8(1).str_max(t, 511);
            }
            DeniedBody::Pve { code } => {
                if self.request != 0x5F {
                    w.fail(Error::Invalid("84 Pve body is only for request 0x5F"));
                }
                w.u8(1).u8(*code);
            }
        }
    }
}

impl Decode for Denied {
    fn decode(r: &mut Reader) -> Result<Self> {
        let request = r.u8()?;
        if request == 0x5F {
            // `u8 1, u8 code`: the first byte is a fixed 1 (the PvE reader), not the generic code
            if r.u8()? != 1 {
                return Err(Error::Invalid("84 5F: first byte must be 1"));
            }
            return Ok(Denied { request, body: DeniedBody::Pve { code: r.u8()? } });
        }
        let body = match r.u8()? {
            code @ (0 | 2) => DeniedBody::Numeric { code, a: r.u8()?, b: r.u32()? },
            1 => DeniedBody::Text(r.str()?),
            _ => return Err(Error::Invalid("84 code other than 0, 1, 2")),
        };
        Ok(Denied { request, body })
    }
}
