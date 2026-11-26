//! Squad, chat, friends and system messages. Chat travels on the lobby UDP
//! connection in 0.69d: AF and B1 are UNIQUE class, B0 and AE ordered.

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// Squad member record of AE. PROVEN (reader 0x20018DC00):
/// `u64 account id, u32 profile id, u32 profile version, u32 items level (> 5 -> 0), str128 name, u8 online,
/// u8 state (bit 0 = ready)`. The client's own record is the one whose account id equals L+0x7A3960.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SquadMember {
    pub account_id: u64,
    pub profile_id: u32,
    pub profile_version: u32,
    pub items_level: u32,
    pub name: String,
    pub online: u8,
    pub state: u8,
}

/// Squad of an AE message with a non-zero squad id.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Squad {
    pub id: u32,
    /// Index of the commander in `members`.
    pub commander: u8,
    /// Squad type, stored at L+0x79889A: 0 normal, 1 PvE ([`SQUAD_TYPE_PVE`](super::SQUAD_TYPE_PVE)). PROVEN: reader
    /// 0x20018DC00, UI 0x1C4891, AS `PVEManager.setSquadType`; type 1 opens the PvE window.
    pub kind: u8,
    pub members: Vec<SquadMember>,
    /// "I am the commander" (L+0x7988A0); the Play button tests it.
    pub i_am_commander: u8,
    /// Stored at L+0x798890, meaning unknown (sent as 0).
    pub tail: u32,
}

/// 0xAE squad update (ordered). PROVEN reader: `u32 squad id` (0 = no squad, nothing follows), else
/// `u8 count (<= 3), u8 commander index, u8 squad type, members, u8 i_am_commander, u32`. Push to every member after each
/// squad change, each with its own `i_am_commander`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SquadUpdate {
    pub squad: Option<Squad>,
}
impl Encode for SquadUpdate {
    fn encode(&self, w: &mut Writer) {
        let Some(s) = &self.squad else {
            w.u32(0);
            return;
        };
        if s.id == 0 {
            w.fail(Error::Invalid("squad id 0 means no squad"));
        }
        if s.members.len() > 3 {
            w.fail(Error::Limit("squad members (3)"));
        }
        w.u32(s.id).count_u8(s.members.len(), "squad members").u8(s.commander).u8(s.kind);
        for m in &s.members {
            w.u64(m.account_id).u32(m.profile_id).u32(m.profile_version).u32(m.items_level);
            w.str_max(&m.name, 128).u8(m.online).u8(m.state);
        }
        w.u8(s.i_am_commander).u32(s.tail);
    }
}
impl Decode for SquadUpdate {
    fn decode(r: &mut Reader) -> Result<Self> {
        let id = r.u32()?;
        if id == 0 {
            return Ok(SquadUpdate { squad: None });
        }
        let n = r.u8()? as usize;
        if n > 3 {
            return Err(Error::Limit("squad members (3)"));
        }
        let mut s = Squad { id, commander: r.u8()?, kind: r.u8()?, ..Default::default() };
        for _ in 0..n {
            s.members.push(SquadMember {
                account_id: r.u64()?,
                profile_id: r.u32()?,
                profile_version: r.u32()?,
                items_level: r.u32()?,
                name: r.str()?,
                online: r.u8()?,
                state: r.u8()?,
            });
        }
        s.i_am_commander = r.u8()?;
        s.tail = r.u32()?;
        Ok(SquadUpdate { squad: Some(s) })
    }
}

/// One chat line of AF. PROVEN (0x200264A40): `u8 kind, u64 sender id (0 = system),
/// u32 age in seconds, str128 name, [kind 5: str32 clan abbreviation], u8 channel, str256 text,
/// [sender != 0: 3 x u8 (nomination, hero place, league top)]`.
/// Kind 5 = player chat (dropped by the client when the sender is on the ignore list); 9 and 0xA are anti-cheat
/// notices; the other kinds go straight to the printer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChatLine {
    pub kind: u8,
    pub sender: u64,
    pub age: u32,
    pub name: String,
    /// Only written / read for kind 5.
    pub clan: String,
    /// 1 = general, 4 = private (others to confirm, LIKELY).
    pub channel: u8,
    pub text: String,
    /// Only written / read when `sender != 0`.
    pub flags: [u8; 3],
}
impl Encode for ChatLine {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.kind).u64(self.sender).u32(self.age).str_max(&self.name, 128);
        if self.kind == 5 {
            w.str_max(&self.clan, 32);
        }
        w.u8(self.channel).str_max(&self.text, 256);
        if self.sender != 0 {
            w.bytes(&self.flags);
        }
    }
}
impl Decode for ChatLine {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut l = ChatLine { kind: r.u8()?, sender: r.u64()?, age: r.u32()?, name: r.str()?, ..Default::default() };
        if l.kind == 5 {
            l.clan = r.str()?;
        }
        l.channel = r.u8()?;
        l.text = r.str()?;
        if l.sender != 0 {
            l.flags = r.array()?;
        }
        Ok(l)
    }
}

/// 0xAF chat (UNIQUE class): `u8 n, n x ChatLine`. PROVEN layout.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chat {
    pub lines: Vec<ChatLine>,
}
impl Encode for Chat {
    fn encode(&self, w: &mut Writer) {
        w.count_u8(self.lines.len(), "chat lines");
        for l in &self.lines {
            l.encode(w);
        }
    }
}
impl Decode for Chat {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u8()? as usize;
        Ok(Chat { lines: (0..n).map(|_| ChatLine::decode(r)).collect::<Result<_>>()? })
    }
}

/// Friend list / search row. Status byte (FriendRender.as): 0 lobby, 1 in game, 2 offline, 4 invite, 5 search.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Person {
    pub id: u64,
    pub name: String,
    pub clan: String,
    pub status: u8,
    /// Seconds in the status (B0 05 only).
    pub seconds: u32,
}

/// 0xB0 messaging (ORDERED). PROVEN (0x200263E5D). Sub 6 is part of the loading gate (bit
/// 0x40): the login sequence must contain `B0 06 00 00`. Sub 7 is applied only when `n` equals the friend count.
#[derive(Debug, Clone, PartialEq)]
pub enum Messaging {
    /// Sub 4 player search result: `u16 n, n x {u64 id, str128 name, str32 abbr, u8 status}` (`seconds` unused).
    Found(Vec<Person>),
    /// Sub 5 friend list, replaces the whole list: `u16 n, n x {u64 id, str128 name, str32 abbr, u8 status, u32 seconds}`.
    Friends(Vec<Person>),
    /// Sub 6 ignore list: `u16 n, n x {u64 id, str128 name}`.
    Ignored(Vec<(u64, String)>),
    /// Sub 7 status update: `u16 n, n x {u64 id, u8 status}`.
    Status(Vec<(u64, u8)>),
}
impl Encode for Messaging {
    fn encode(&self, w: &mut Writer) {
        match self {
            Messaging::Found(v) => {
                w.u8(4).count_u16(v.len(), "B0 rows");
                for p in v {
                    w.u64(p.id).str_max(&p.name, 128).str_max(&p.clan, 32).u8(p.status);
                }
            }
            Messaging::Friends(v) => {
                w.u8(5).count_u16(v.len(), "B0 rows");
                for p in v {
                    w.u64(p.id).str_max(&p.name, 128).str_max(&p.clan, 32).u8(p.status).u32(p.seconds);
                }
            }
            Messaging::Ignored(v) => {
                w.u8(6).count_u16(v.len(), "B0 rows");
                for (id, n) in v {
                    w.u64(*id).str_max(n, 128);
                }
            }
            Messaging::Status(v) => {
                w.u8(7).count_u16(v.len(), "B0 rows");
                for &(id, s) in v {
                    w.u64(id).u8(s);
                }
            }
        }
    }
}
impl Decode for Messaging {
    fn decode(r: &mut Reader) -> Result<Self> {
        let sub = r.u8()?;
        let n = r.u16()? as usize;
        r.check_count(n, 9)?;
        Ok(match sub {
            4 => Messaging::Found(
                (0..n)
                    .map(|_| Ok(Person { id: r.u64()?, name: r.str()?, clan: r.str()?, status: r.u8()?, seconds: 0 }))
                    .collect::<Result<_>>()?,
            ),
            5 => Messaging::Friends(
                (0..n)
                    .map(|_| Ok(Person { id: r.u64()?, name: r.str()?, clan: r.str()?, status: r.u8()?, seconds: r.u32()? }))
                    .collect::<Result<_>>()?,
            ),
            6 => Messaging::Ignored((0..n).map(|_| Ok((r.u64()?, r.str()?))).collect::<Result<_>>()?),
            7 => Messaging::Status((0..n).map(|_| Ok((r.u64()?, r.u8()?))).collect::<Result<_>>()?),
            _ => return Err(Error::Unsupported("B0 sub other than 4..7")),
        })
    }
}

/// 0xB1 system / important message (UNIQUE class). PROVEN (reader 0x200265550, consumer call 0x2002658B0 passes the
/// record's +0xB8 byte as the type): `u8 flag, u64 sender id, str128 name, str32 abbr, u8 TYPE, str256 text, u32 message
/// id` (de-duplicated by message id). The TYPE is the byte AFTER the clan tag (the
/// 0.26g layout has the same order: sender type first, kind after the tag); the first byte is read into +0xB0 and used by
/// none of the three consumers (0.26g: the sender type, 5 = player; 0 here). Types: 2 = squad invite (accept -> 4D kind 3), 0x17 = PvE squad invite (accept -> 4D kind 0x18),
/// 7/8 = clan / custom match invites (LIKELY).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemMessage {
    pub kind: u8,
    pub sender: u64,
    pub name: String,
    pub clan: String,
    pub flag: u8,
    pub text: String,
    pub message_id: u32,
}
impl Encode for SystemMessage {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.flag).u64(self.sender).str_max(&self.name, 128).str_max(&self.clan, 32).u8(self.kind);
        w.str_max(&self.text, 256).u32(self.message_id);
    }
}
impl Decode for SystemMessage {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(SystemMessage {
            flag: r.u8()?,
            sender: r.u64()?,
            name: r.str()?,
            clan: r.str()?,
            kind: r.u8()?,
            text: r.str()?,
            message_id: r.u32()?,
        })
    }
}
