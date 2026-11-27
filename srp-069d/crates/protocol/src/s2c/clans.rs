//! S->C 0x9A clan info (reader 0x20018BD60, jump table RVA 0x18CCC8; layouts read from the reader;
//! docs/notes/social.md). Sub 0 has two forms, chosen by the byte after the clan tag
//! (0x20018BE2B): 1 = the registered clan ([`Clan9A::Full`]), anything else = the receiver's OWN clan in
//! registration ([`Clan9A::Registration`]).
//!
//! * sub 0 full clan: `u32 clan id` (0 = no clan, nothing else follows), `str100 name`, `str32 abbr`, `u8 own = 1`,
//!   `u8 nr <= 10, nr x {str32 rank name, u8 permission mask, u8 default flag}`, `u32 K <= 107`,
//!   `K x member`, `u16 index of the receiving player in the member list` (PROVEN use: 0x280572 finds the local
//!   player's rank through it), `u8 A` (1 = commander: window state 4, else 3), `u8 level`, `u32 players`,
//!   `u32 min, u32 max, u32 current` (the five values of `root.set_clan_progress(min, max, current, players, level)`).
//! * member: `u64 id, str128 name, u8 rank index, u8 default-rank flag, u8 online` (own-clan layout).
//! * sub 1: `u32 clan id`, `str512` daily message (shown in chat as a clan system line).
//! * sub 2: `u32 clan id`, `u32 K`, K members, `u16 own index`, `u8 A` (members refresh).
//! * sub 3: `u32 clan id`, `u8 nr`, ranks (ranks refresh).
//! * sub 4: `u8 n` (n != 0: notice "the clan was disbanded").
//! * sub 5: `u32 clan id`, `u8 flag (0 = nothing)`, `str100 name`, `str32 abbr` (a reservation).

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rank9A {
    pub name: String,
    /// bit 4 invite, bit 3 kick, bit 2 change rank, bit 1 daily message, bit 0 battle commander.
    pub perms: u8,
    pub default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Member9A {
    pub id: u64,
    pub name: String,
    pub rank: u8,
    /// the member's level, shown in the member row (PROVEN: reader 0x20018C033 -> +0xD, consumer
    /// 0x20027FD88 'level'; the row's `default_rank` comes from the rank table's own flag, 0x20027FD5E)
    pub level: u8,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clan9A {
    /// sub 0 with id 0: the player is in no clan.
    NoClan,
    Full {
        clan_id: u32,
        name: String,
        abbr: String,
        ranks: Vec<Rank9A>,
        members: Vec<Member9A>,
        me_index: u16,
        commander: bool,
        level: u8,
        players: u32,
        min: u32,
        max: u32,
        current: u32,
    },
    Daily { clan_id: u32, text: String },
    Members { clan_id: u32, members: Vec<Member9A>, me_index: u16, commander: bool },
    Ranks { clan_id: u32, ranks: Vec<Rank9A> },
    Disbanded,
    Reservation { clan_id: u32, name: String, abbr: String },
    /// Sub 0, registration form (reader 0x20018BDD7..0x20018C16E): `u32 id (!= 0), str100 name, str32 abbr, u8 0, u8 nr
    /// ranks + ranks, u32 K, K x {u64 id, str128 name, u8 rank, u8 status (0 = ready)}, u16 own index, u8 commander, u8
    /// supporters_ok (P, sticky: window state 2 = payment), u8 paid (Q, sticky), u32 collected silver (only grows), u8
    /// name_state (S: 3 = name and abbreviation fine; 0 / 1 / 2 = name / abbreviation / both invalid)`.
    Registration {
        clan_id: u32,
        name: String,
        abbr: String,
        ranks: Vec<Rank9A>,
        members: Vec<(u64, String, u8, u8)>,
        me_index: u16,
        commander: bool,
        supporters_ok: bool,
        paid: bool,
        collected: u32,
        name_state: u8,
    },
}

fn put_ranks(w: &mut Writer, ranks: &[Rank9A]) {
    w.count_u8(ranks.len().min(10), "clan ranks");
    for r in ranks.iter().take(10) {
        w.str_max(&r.name, 32).u8(r.perms).u8(u8::from(r.default));
    }
}

fn put_members(w: &mut Writer, members: &[Member9A]) {
    w.u32(members.len().min(107) as u32);
    for m in members.iter().take(107) {
        w.u64(m.id).str_max(&m.name, 128).u8(m.rank).u8(m.level).u8(u8::from(m.online));
    }
}

fn get_ranks(r: &mut Reader) -> Result<Vec<Rank9A>> {
    let n = r.u8()? as usize;
    if n > 10 {
        return Err(Error::Limit("9A ranks (10)"));
    }
    (0..n).map(|_| Ok(Rank9A { name: r.str()?, perms: r.u8()?, default: r.u8()? == 1 })).collect()
}

fn get_members(r: &mut Reader) -> Result<Vec<Member9A>> {
    let k = r.u32()? as usize;
    if k > 107 {
        return Err(Error::Limit("9A members (107)"));
    }
    (0..k).map(|_| Ok(Member9A { id: r.u64()?, name: r.str()?, rank: r.u8()?, level: r.u8()?, online: r.u8()? == 1 })).collect()
}

impl Encode for Clan9A {
    fn encode(&self, w: &mut Writer) {
        match self {
            Clan9A::NoClan => {
                w.u8(0).u32(0);
            }
            Clan9A::Full { clan_id, name, abbr, ranks, members, me_index, commander, level, players, min, max, current } => {
                w.u8(0).u32(*clan_id).str_max(name, 100).str_max(abbr, 32).u8(1);
                put_ranks(w, ranks);
                put_members(w, members);
                w.u16(*me_index).u8(u8::from(*commander)).u8(*level).u32(*players).u32(*min).u32(*max).u32(*current);
            }
            Clan9A::Daily { clan_id, text } => {
                w.u8(1).u32(*clan_id).str_max(text, 500);
            }
            Clan9A::Members { clan_id, members, me_index, commander } => {
                w.u8(2).u32(*clan_id);
                put_members(w, members);
                w.u16(*me_index).u8(u8::from(*commander));
            }
            Clan9A::Ranks { clan_id, ranks } => {
                w.u8(3).u32(*clan_id);
                put_ranks(w, ranks);
            }
            Clan9A::Disbanded => {
                w.u8(4).u8(1);
            }
            Clan9A::Registration { clan_id, name, abbr, ranks, members, me_index, commander, supporters_ok, paid, collected, name_state } => {
                w.u8(0).u32(*clan_id).str_max(name, 100).str_max(abbr, 32).u8(0);
                put_ranks(w, ranks);
                w.u32(members.len().min(107) as u32);
                for (id, n, rank, status) in members.iter().take(107) {
                    w.u64(*id).str_max(n, 128).u8(*rank).u8(*status);
                }
                w.u16(*me_index).u8(u8::from(*commander)).u8(u8::from(*supporters_ok)).u8(u8::from(*paid)).u32(*collected).u8(*name_state);
            }
            Clan9A::Reservation { clan_id, name, abbr } => {
                w.u8(5).u32(*clan_id).u8(1).str_max(name, 100).str_max(abbr, 32);
            }
        }
    }
}

impl Decode for Clan9A {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(match r.u8()? {
            0 => {
                let clan_id = r.u32()?;
                if clan_id == 0 {
                    return Ok(Clan9A::NoClan);
                }
                let (name, abbr) = (r.str()?, r.str()?);
                if r.u8()? != 1 {
                    let ranks = get_ranks(r)?;
                    let k = r.u32()? as usize;
                    if k > 107 {
                        return Err(Error::Limit("9A members (107)"));
                    }
                    let members = (0..k).map(|_| Ok((r.u64()?, r.str()?, r.u8()?, r.u8()?))).collect::<Result<Vec<_>>>()?;
                    return Ok(Clan9A::Registration {
                        clan_id,
                        name,
                        abbr,
                        ranks,
                        members,
                        me_index: r.u16()?,
                        commander: r.u8()? != 0,
                        supporters_ok: r.u8()? != 0,
                        paid: r.u8()? != 0,
                        collected: r.u32()?,
                        name_state: r.u8()?,
                    });
                }
                let ranks = get_ranks(r)?;
                let members = get_members(r)?;
                Clan9A::Full {
                    clan_id,
                    name,
                    abbr,
                    ranks,
                    members,
                    me_index: r.u16()?,
                    commander: r.u8()? == 1,
                    level: r.u8()?,
                    players: r.u32()?,
                    min: r.u32()?,
                    max: r.u32()?,
                    current: r.u32()?,
                }
            }
            1 => Clan9A::Daily { clan_id: r.u32()?, text: r.str()? },
            2 => {
                let clan_id = r.u32()?;
                let members = get_members(r)?;
                Clan9A::Members { clan_id, members, me_index: r.u16()?, commander: r.u8()? == 1 }
            }
            3 => Clan9A::Ranks { clan_id: r.u32()?, ranks: get_ranks(r)? },
            4 => {
                r.u8()?;
                Clan9A::Disbanded
            }
            5 => {
                let clan_id = r.u32()?;
                if r.u8()? == 0 {
                    return Err(Error::Unsupported("9A sub 5 without data"));
                }
                Clan9A::Reservation { clan_id, name: r.str()?, abbr: r.str()? }
            }
            _ => return Err(Error::Unsupported("9A sub")),
        })
    }
}
