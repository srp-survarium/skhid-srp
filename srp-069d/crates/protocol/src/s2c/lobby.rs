//! Server to client lobby pushes of the sign-in sequence and the account pages (docs/notes/workshop.md).
//! Every `decode` / `encode` here works on the payload after `id x` (AC: after `id`).

use crate::items::{AccountBody, ItemRecord, LevelFields};
use crate::lzma2;
use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

macro_rules! prim_msg {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident : $t:ident),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Default)]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
        impl Encode for $name {
            fn encode(&self, w: &mut Writer) { $( w.$t(self.$f); )* }
        }
        impl Decode for $name {
            fn decode(r: &mut Reader) -> Result<Self> { Ok($name { $( $f: r.$t()? ),* }) }
        }
    };
}

prim_msg! {
    /// 0x80 sign-in answer (unique). PROVEN reader 0x2001858B1: `u8 state, u8 flag, u64, u32, u8, 4 bytes skipped,
    /// u32 (0 -> -1)`. `flag` is the second byte the client echoes in request 32. The 8 bytes (one 8-byte read) are the
    /// player's **own public account id**: stored at `L+0x7A3960` (0x200185916) and compared with the u64 account id of
    /// squad records (`is_local_player` 0x2001C4C92, own record 0x20018DEE0), custom-match members (0x2001E8110) and
    /// used for the season history / client-auth URL. With 0 no squad record is ever "me" (the PvE window lists the
    /// player twice).
    SignedIn { state: u8, flag: u8, account_id: u64, a: u32, b: u8, skipped: u32, c: u32 }
}

prim_msg! {
    /// 0x85 queue / play state (ordered), the one-byte form: ONLY state 0 = idle (re-enables Play after 2F/5F, clears
    /// lobby_menu+0xEBBD4). PROVEN reader 0x200186280: state 1 = searching is `01 u32 order, u32 place
    /// (0 = "match found", 0x2001C16F0), u32 total`; state 2 = match being created is `02 u32 order, u32 match id, u8 side`
    /// (0x200186308; -1 / -1 / 3 = none, a local match); see [`match_state`].
    QueueState { state: u8 }
}

/// `85 02 u32 order, u32 match id, u8 side` (reader 0x200186308): a match is being created. A local match (training,
/// tutorial, range) has none of the three: the reader's own "none" values -1 / -1 / 3 (0x200186352), with which
/// 0x200266150 leaves the match-found record alone.
pub fn match_state(order: u32, match_id: u32, side: u8) -> crate::s2c::S2c {
    let mut p = vec![2u8];
    p.extend_from_slice(&order.to_le_bytes());
    p.extend_from_slice(&match_id.to_le_bytes());
    p.push(side);
    crate::s2c::S2c::Raw { id: 0x85, payload: p }
}

/// [`match_state`] of a local match.
pub fn local_match_state() -> crate::s2c::S2c {
    match_state(u32::MAX, u32::MAX, 3)
}

prim_msg! {
    /// 0x8B money (ordered). PROVEN order (docs/notes/workshop.md, stored +0x798690..0x7986A8): silver, gold, event currency 1,
    /// spare parts ("details"), event currency 2, event currency 3, league.
    Money { silver: u32, gold: u32, event1: u32, spare_parts: u32, event2: u32, event3: u32, league: u32 }
}

prim_msg! {
    /// 0x91 pocket "specifics opened" bit mask over the 19 pocket consumables (static table 0x200976A90). PROVEN
    /// reader; 0x7FFFF (all opened) must come only AFTER the lobby-load 5B 02 reply (before it the client crashes).
    SpecificsOpened { mask: u32 }
}

prim_msg! {
    /// 0x96 faction unlock mask (inline 0x20018ECB3, PROVEN): bit f-1 = faction f unlocked; `0F` = all.
    FactionMask { mask: u8 }
}

prim_msg! {
    /// 0x9B item craft chances (inline 0x20018EF9D, PROVEN): percent for spare-part and gold crafts
    /// (`root.setup_item_craft_chances([A, B])`; the unit being percent is LIKELY). Send only after the lobby is up.
    CraftChances { regular: f32, gold: f32 }
}

prim_msg! {
    /// 0x9D tutorial / step gate (ordered). PROVEN reader: `u8 A, u32 F, u32 G,
    /// u8 faction, u16 H`. F = 0xE2 / 0x4E2 selects the full-UI lobby step.
    TutorialGate { a: u8, flags: u32, g: u32, faction: u8, h: u16 }
}

/// 0xAC play gate (PLAIN class, no x byte). PROVEN reader 0x20018AEC0: `u32 value, u32 mode_mask, u8 n,
/// n x {u8 server id, u16 wait_count}`.
/// * `value` -> lobby_menu+0xEBB90 (0x2001C1E00), must be >= 30 (0x2001B98DB); 0 also opens the maintenance popup;
/// * `mode_mask` -> `[0x200AA2124]`, the server-enabled game modes: the mode list (0x200177ED0) shows mode i as
///   playable only with bit i set ("The mode is unavailable" otherwise);
/// * the server rows: `wait_count` of the `9E` server with that id (+0x82, 0x20018B088), shown only in the server
///   tick-box tooltip `ping: N wait: M`. The reader always reads `n`, also after an 8-byte AC.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayGate {
    pub value: u32,
    pub mode_mask: u32,
    /// (server id, wait_count)
    pub waits: Vec<(u8, u16)>,
}
impl Encode for PlayGate {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.value).u32(self.mode_mask).u8(self.waits.len().min(255) as u8);
        for &(id, wait) in self.waits.iter().take(255) {
            w.u8(id).u16(wait);
        }
    }
}
impl Decode for PlayGate {
    fn decode(r: &mut Reader) -> Result<Self> {
        let (value, mode_mask, n) = (r.u32()?, r.u32()?, r.u8()?);
        let waits = (0..n).map(|_| Ok((r.u8()?, r.u16()?))).collect::<Result<_>>()?;
        Ok(PlayGate { value, mode_mask, waits })
    }
}

/// 0x8F: `u8 flag, str name`. PROVEN to be accepted by the client (loading gate bit 0x01); meaning LIKELY.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NameMsg {
    pub flag: u8,
    pub name: String,
}
impl Encode for NameMsg {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.flag).str(&self.name);
    }
}
impl Decode for NameMsg {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(NameMsg { flag: r.u8()?, name: r.str()? })
    }
}

/// 0x9C: four flag bytes stored at lobby_client+0x7A33B0.. (inline 0x20018E5E6, PROVEN).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Flags9C(pub [u8; 4]);
impl Encode for Flags9C {
    fn encode(&self, w: &mut Writer) {
        w.bytes(&self.0);
    }
}
impl Decode for Flags9C {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Flags9C(r.array()?))
    }
}

/// 0x87 account body + profiles + items (ordered, loading bit 0x08). PROVEN:
/// [`AccountBody`], then per profile `u32 version + u8` (the 3D character rebuilds only when the version changes),
/// then `u32 current profile id`, then [`LevelFields`] (same as 8C).
/// Send 89 first so the 87/89 merge finds the items; pocket items need 91 first (pocket crash).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Account87 {
    pub body: AccountBody,
    /// One `(version, flag)` per profile.
    pub profile_state: Vec<(u32, u8)>,
    pub current_profile: u32,
    pub level: LevelFields,
}

impl Encode for Account87 {
    fn encode(&self, w: &mut Writer) {
        self.body.encode(w);
        if self.profile_state.len() != self.body.profiles.len() {
            w.fail(Error::Invalid("87: one version per profile"));
        }
        for &(v, f) in &self.profile_state {
            w.u32(v).u8(f);
        }
        w.u32(self.current_profile);
        self.level.encode(w);
    }
}

impl Decode for Account87 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let body = AccountBody::decode(r)?;
        let mut profile_state = Vec::new();
        for _ in 0..body.profiles.len() {
            profile_state.push((r.u32()?, r.u8()?));
        }
        Ok(Account87 { body, profile_state, current_profile: r.u32()?, level: LevelFields::decode(r)? })
    }
}

/// 0x88 member profile of a squad mate (reply to 33). Reader 0x200186B60: account body + `u32 profile VERSION` (PROVEN:
/// stored at account +0x300; the 3D rebuild test 0x200277E80 compares the profile id AND this version,
/// so the same profile with a new version rebuilds the mate's model; it must equal the AE `profile_version` of that mate).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemberProfile88 {
    pub body: AccountBody,
    pub version: u32,
}
impl Encode for MemberProfile88 {
    fn encode(&self, w: &mut Writer) {
        self.body.encode(w);
        w.u32(self.version);
    }
}
impl Decode for MemberProfile88 {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(MemberProfile88 { body: AccountBody::decode(r)?, version: r.u32()? })
    }
}

/// 0x89 inventory (ordered, loading bit 0x04). PROVEN (reader 0x200186D30):
/// `u32 max items, u32 n, u32 packed size, raw LZMA2 of n item records`. Encoded with stored LZMA2 chunks
/// (compressed streams differ on the wire and decode to the same records).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Inventory89 {
    pub max_items: u32,
    pub items: Vec<ItemRecord>,
}

impl Encode for Inventory89 {
    fn encode(&self, w: &mut Writer) {
        let mut raw = Writer::new();
        for it in &self.items {
            it.encode(&mut raw);
        }
        match raw.finish() {
            Ok(b) => {
                w.u32(self.max_items).count_u32(self.items.len());
                lzma2::write_packed(w, &b);
            }
            Err(e) => w.fail(e),
        }
    }
}

impl Decode for Inventory89 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let max_items = r.u32()?;
        let n = r.u32()? as usize;
        let raw = lzma2::read_packed(r)?;
        let mut rr = Reader::new(&raw);
        if n > raw.len() / 12 + 1 {
            return Err(Error::Invalid("89: item count larger than the data"));
        }
        let items = (0..n).map(|_| ItemRecord::decode(&mut rr)).collect::<Result<Vec<_>>>()?;
        rr.finish()?;
        Ok(Inventory89 { max_items, items })
    }
}

/// One 16-byte price record of 0x8A. PROVEN (0x18A810..0x18A95B):
/// `u32 a (+0x2F0), u32 b (+0x2F4), u16 dict, u16 c (+0x2F8), u8 d (+0x2FC), u8 e bool (+0x300), 2 pad`.
/// Use: a = price, b = sell base, c = gold/premium cost, d = kind (1 = gold item), e = for sale.
/// For modules (workshop shop tab) a = spare-parts price, c = gold price.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PriceRecord {
    pub a: u32,
    pub b: u32,
    pub dict: u16,
    pub c: u16,
    pub d: u8,
    pub e: u8,
}

/// 0x8A prices (ordered). PROVEN: `u8 idx` (row of the dictionary's service price vector; sent
/// as 0, an out-of-range idx crashes) followed by raw LZMA2 WITHOUT a size field; decoded size must be
/// a multiple of 16 and at most dictionary item count * 16. Smallest valid payload `00 00`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prices8A {
    pub idx: u8,
    pub records: Vec<PriceRecord>,
}

impl Encode for Prices8A {
    fn encode(&self, w: &mut Writer) {
        let mut raw = Writer::new();
        for p in &self.records {
            raw.u32(p.a).u32(p.b).u16(p.dict).u16(p.c).u8(p.d).u8(p.e).zeros(2);
        }
        match raw.finish() {
            Ok(b) => {
                w.u8(self.idx).bytes(&lzma2::compress(&b));
            }
            Err(e) => w.fail(e),
        }
    }
}

impl Decode for Prices8A {
    fn decode(r: &mut Reader) -> Result<Self> {
        let idx = r.u8()?;
        let raw = lzma2::decompress(r.rest())?;
        if raw.len() % 16 != 0 {
            return Err(Error::Invalid("8A: decoded size not a multiple of 16"));
        }
        let mut rr = Reader::new(&raw);
        let mut records = Vec::with_capacity(raw.len() / 16);
        while !rr.is_empty() {
            let p = PriceRecord { a: rr.u32()?, b: rr.u32()?, dict: rr.u16()?, c: rr.u16()?, d: rr.u8()?, e: rr.u8()? };
            rr.skip(2)?;
            records.push(p);
        }
        Ok(Prices8A { idx, records })
    }
}

/// 0x8C level and skills (ordered). PROVEN (0x200186F30): [`LevelFields`], `u8 n`, n x
/// `{u16 skill id, u8 points}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Level8C {
    pub level: LevelFields,
    pub skills: Vec<(u16, u8)>,
}
impl Encode for Level8C {
    fn encode(&self, w: &mut Writer) {
        self.level.encode(w);
        w.count_u8(self.skills.len(), "skills");
        for &(id, p) in &self.skills {
            w.u16(id).u8(p);
        }
    }
}
impl Decode for Level8C {
    fn decode(r: &mut Reader) -> Result<Self> {
        let level = LevelFields::decode(r)?;
        let n = r.u8()? as usize;
        let skills = (0..n).map(|_| Ok((r.u16()?, r.u8()?))).collect::<Result<_>>()?;
        Ok(Level8C { level, skills })
    }
}

/// 0x8D service prices (ordered). PROVEN (stored at lobby_client+0x7A3380): `u32 x6`
/// (+0/+4 reroll cost, +8 profile cost, +0xC name change, +0x10 clan creation, +0x14 quest reroll),
/// `f32 x4` sell multipliers by modifier count (sell = base * mult[min(n,3)] + 0.5; all 0 made every sell price 0),
/// `u32 +0x28` premium exchange rate (silver per gold; 250 in the data, 0 divides by zero in the exchange window),
/// `u8 flag`, `[flag: u16 +0x2C]`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ServicePrices8D {
    pub costs: [u32; 6],
    pub sell_mult: [f32; 4],
    pub exchange_rate: u32,
    /// The tail `u8 flag, u16 price`: PROVEN (0x200180830) the u16 is stored at +0x2C and the flag at +0x2E, and replaces
    /// the gold price of a PvE ticket in `play_cost_gold` (0x1BA3AA). None = flag 0 = the client's own price
    /// (`pve_default_prices.ticket_price`, 50).
    pub extra: Option<u16>,
}
impl ServicePrices8D {
    /// Override the PvE ticket gold price shown by the client.
    pub fn with_pve_ticket_price(mut self, price: u16) -> Self {
        self.extra = Some(price);
        self
    }
}
impl Encode for ServicePrices8D {
    fn encode(&self, w: &mut Writer) {
        for c in self.costs {
            w.u32(c);
        }
        for m in self.sell_mult {
            w.f32(m);
        }
        w.u32(self.exchange_rate);
        match self.extra {
            Some(v) => {
                w.u8(1).u16(v);
            }
            None => {
                w.u8(0);
            }
        }
    }
}
impl Decode for ServicePrices8D {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut s = ServicePrices8D::default();
        for c in &mut s.costs {
            *c = r.u32()?;
        }
        for m in &mut s.sell_mult {
            *m = r.f32()?;
        }
        s.exchange_rate = r.u32()?;
        if r.u8()? != 0 {
            s.extra = Some(r.u16()?);
        }
        Ok(s)
    }
}

/// Faction record of 8E / 92: `u32 faction (1..4), u32 unlocked seller level (LIKELY), u32 reputation points`.
/// The client reads record `f` as faction `f` in two places (0x2002470B8, 0x200188398), so the list starts with a
/// faction-0 record (`lobby_server::build::factions_8e`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FactionRec {
    pub faction: u32,
    pub unlocked: u32,
    pub points: u32,
}

/// 0x8E factions / reputations (ordered). PROVEN (0x20018AC70): `u8 n, n x FactionRec, u8 active
/// faction (0 = none -> empty shop), u8 k, [k != 0: 10 x u32 faction battle status]`. Needs 96 (mask) too.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Factions8E {
    pub factions: Vec<FactionRec>,
    pub active: u8,
    pub battle: Option<[u32; 10]>,
}
impl Encode for Factions8E {
    fn encode(&self, w: &mut Writer) {
        w.count_u8(self.factions.len(), "factions");
        for f in &self.factions {
            w.u32(f.faction).u32(f.unlocked).u32(f.points);
        }
        w.u8(self.active);
        match &self.battle {
            Some(b) => {
                w.u8(1);
                for v in b {
                    w.u32(*v);
                }
            }
            None => {
                w.u8(0);
            }
        }
    }
}
impl Decode for Factions8E {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u8()? as usize;
        let mut factions = Vec::new();
        for _ in 0..n {
            factions.push(FactionRec { faction: r.u32()?, unlocked: r.u32()?, points: r.u32()? });
        }
        let active = r.u8()?;
        let battle = if r.u8()? != 0 {
            let mut b = [0u32; 10];
            for v in &mut b {
                *v = r.u32()?;
            }
            Some(b)
        } else {
            None
        };
        Ok(Factions8E { factions, active, battle })
    }
}

/// 0x92: the body of 8E plus `u8 flag` (flag != 0 shows the Quests subsystem). PROVEN.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Factions92 {
    pub factions: Factions8E,
    pub quests: u8,
}
impl Encode for Factions92 {
    fn encode(&self, w: &mut Writer) {
        self.factions.encode(w);
        w.u8(self.quests);
    }
}
impl Decode for Factions92 {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Factions92 { factions: Factions8E::decode(r)?, quests: r.u8()? })
    }
}

/// One record of 0x94. PROVEN layout (0x200189F90); meaning LIKELY (effects / boosters / notices).
/// `data1` must be <= 0x800 bytes, `data2` <= 0x400 (stack overflow in the client otherwise).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Notice94 {
    pub f30: u32,
    pub f38: u32,
    pub f3c: u32,
    /// Dictionary item id for types 1..7 and 0x18 (NULL checked).
    pub f40: u32,
    pub f44: u32,
    pub kind: u8,
    pub x: u8,
    pub data1: Vec<u8>,
    pub data2: Vec<u8>,
}

/// Optional tail of 0x94, present only when some record has type 0x16.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tail94 {
    /// `u16 C` then C bytes (C <= 60).
    pub bytes: Vec<u8>,
    pub a: u32,
    pub b: u32,
    /// `u32 len` + bytes (copied up to 0x400).
    pub text: Vec<u8>,
}

/// 0x94 notices / premium store data (ordered). PROVEN layout. Send 95 first (LIKELY).
/// `u32 N, N x Notice94, u16 (+0x7A3930), u32 M, M x 3 x u32, u32 K, K x u32, [type 0x16 present: u8 present,
/// present: Tail94]`. Smallest: 14 zero bytes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Notices94 {
    pub notices: Vec<Notice94>,
    pub v930: u16,
    pub triples: Vec<[u32; 3]>,
    pub list: Vec<u32>,
    pub tail: Option<Tail94>,
}
impl Encode for Notices94 {
    fn encode(&self, w: &mut Writer) {
        w.count_u32(self.notices.len());
        for n in &self.notices {
            if n.data1.len() > 0x800 || n.data2.len() > 0x400 {
                w.fail(Error::Limit("94 record blobs (0x800 / 0x400)"));
            }
            w.u32(n.f30).u32(n.f38).u32(n.f3c).u32(n.f40).u32(n.f44);
            w.count_u32(n.data1.len()).count_u32(n.data2.len());
            w.u8(n.kind).u8(n.x).bytes(&n.data1).bytes(&n.data2);
        }
        w.u16(self.v930).count_u32(self.triples.len());
        for t in &self.triples {
            w.u32(t[0]).u32(t[1]).u32(t[2]);
        }
        w.count_u32(self.list.len());
        for v in &self.list {
            w.u32(*v);
        }
        if self.notices.iter().any(|n| n.kind == 0x16) {
            match &self.tail {
                Some(t) => {
                    if t.bytes.len() > 60 {
                        w.fail(Error::Limit("94 tail bytes (60)"));
                    }
                    w.u8(1).count_u16(t.bytes.len(), "94 tail").bytes(&t.bytes).u32(t.a).u32(t.b);
                    w.count_u32(t.text.len()).bytes(&t.text);
                }
                None => {
                    w.u8(0);
                }
            }
        } else if self.tail.is_some() {
            w.fail(Error::Invalid("94: tail without a type 0x16 record"));
        }
    }
}
impl Decode for Notices94 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u32()? as usize;
        r.check_count(n, 34)?;
        let mut m = Notices94::default();
        for _ in 0..n {
            let mut x = Notice94 { f30: r.u32()?, f38: r.u32()?, f3c: r.u32()?, f40: r.u32()?, f44: r.u32()?, ..Default::default() };
            let l1 = r.u32()? as usize;
            let l2 = r.u32()? as usize;
            if l1 > 0x800 || l2 > 0x400 {
                return Err(Error::Limit("94 record blobs (0x800 / 0x400)"));
            }
            x.kind = r.u8()?;
            x.x = r.u8()?;
            x.data1 = r.bytes(l1)?.to_vec();
            x.data2 = r.bytes(l2)?.to_vec();
            m.notices.push(x);
        }
        m.v930 = r.u16()?;
        let k = r.u32()? as usize;
        r.check_count(k, 12)?;
        for _ in 0..k {
            m.triples.push([r.u32()?, r.u32()?, r.u32()?]);
        }
        let k = r.u32()? as usize;
        r.check_count(k, 4)?;
        for _ in 0..k {
            m.list.push(r.u32()?);
        }
        if m.notices.iter().any(|n| n.kind == 0x16) && r.u8()? != 0 {
            let c = r.u16()? as usize;
            if c > 60 {
                return Err(Error::Limit("94 tail bytes (60)"));
            }
            let bytes = r.bytes(c)?.to_vec();
            let a = r.u32()?;
            let b = r.u32()?;
            let len = r.u32()? as usize;
            m.tail = Some(Tail94 { bytes, a, b, text: r.bytes(len)?.to_vec() });
        }
        Ok(m)
    }
}

/// 44-byte sub-record of a shop element (95). PROVEN (0x1806A0): ten `u32`, `u16`, `u8`, `u8`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ShopSub {
    pub vals: [u32; 10],
    pub a: u16,
    pub b: u8,
    pub c: u8,
}

/// Shop element (0x490 bytes in the client). PROVEN layout, meaning LIKELY (premium store offers):
/// `u32 +0, u32 +4 (dictionary id, used by B5/B6 loops), u8 +8, u8 +9, u32 +0xC, u8 +0x488, u8 m (<= 20), m x ShopSub`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShopElem {
    pub f0: u32,
    pub dict: u32,
    pub f8: u8,
    pub f9: u8,
    pub fc: u32,
    pub f488: u8,
    pub subs: Vec<ShopSub>,
}

/// Shop action tab of 95: `u32 +0x40, str64 name, u8 bool` (the u32 comes before the string).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShopTab {
    pub id: u32,
    pub name: String,
    pub flag: u8,
}

/// 0x95 shop (ordered). PROVEN (not LZMA2 in 0.69d): `u32 n, n x ShopElem, u8 n2, n2 x ShopTab`.
/// Hazard: B5/B6 dereference `dict` of every element, keep them valid or send n = 0.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shop95 {
    pub elems: Vec<ShopElem>,
    pub tabs: Vec<ShopTab>,
}
impl Encode for Shop95 {
    fn encode(&self, w: &mut Writer) {
        w.count_u32(self.elems.len());
        for e in &self.elems {
            if e.subs.len() > 20 {
                w.fail(Error::Limit("95 sub-records (20)"));
            }
            w.u32(e.f0).u32(e.dict).u8(e.f8).u8(e.f9).u32(e.fc).u8(e.f488);
            w.count_u8(e.subs.len(), "95 sub-records");
            for s in &e.subs {
                for v in s.vals {
                    w.u32(v);
                }
                w.u16(s.a).u8(s.b).u8(s.c);
            }
        }
        w.count_u8(self.tabs.len(), "95 tabs");
        for t in &self.tabs {
            // at most 0x3F bytes: the client's reader NULs [len] in a 0x40 field, so a 64-byte name would zero the low byte
            // of the tab id stored after it (0x20018ABFB / 0x20011DEB8)
            w.u32(t.id).str_max(&t.name, 0x3F).u8(t.flag);
        }
    }
}
impl Decode for Shop95 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u32()? as usize;
        r.check_count(n, 16)?;
        let mut s = Shop95::default();
        for _ in 0..n {
            let mut e = ShopElem { f0: r.u32()?, dict: r.u32()?, f8: r.u8()?, f9: r.u8()?, fc: r.u32()?, f488: r.u8()?, subs: vec![] };
            let m = r.u8()? as usize;
            if m > 20 {
                return Err(Error::Limit("95 sub-records (20)"));
            }
            for _ in 0..m {
                let mut vals = [0u32; 10];
                for v in &mut vals {
                    *v = r.u32()?;
                }
                e.subs.push(ShopSub { vals, a: r.u16()?, b: r.u8()?, c: r.u8()? });
            }
            s.elems.push(e);
        }
        let n2 = r.u8()? as usize;
        for _ in 0..n2 {
            s.tabs.push(ShopTab { id: r.u32()?, name: r.str()?, flag: r.u8()? });
        }
        Ok(s)
    }
}

/// One game server of 0x9E. PROVEN (docs/notes/server-list.md): `str64, str64, u16, u16, u8`. Meaning (exe
/// 0x189DA1..0x189F4A, 0x195150): `name` = the host the client pings (entry +0x00), `address` = the label Flash shows
/// (`label`, +0x40; the field names are historic), `a` / `b` = `x` / `y` of the server's marker on the window's map, `c` = the
/// server id (the bit in the `57` mask and the byte of the `56` ping results).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameServer {
    pub name: String,
    pub address: String,
    pub a: u16,
    pub b: u16,
    pub c: u8,
}

/// 0x9E game server startup info (UNIQUE class). PROVEN (0x200189A50): three long strings
/// (max 0x400; the third is the base of the BA URL), `u8 n` (<= 8, more = assert), n x [`GameServer`].
/// Smallest form: `00 00 00 00` (three empty strings, n = 0).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ServerInfo9E {
    pub s1: String,
    pub s2: String,
    pub s3: String,
    pub servers: Vec<GameServer>,
}
impl Encode for ServerInfo9E {
    fn encode(&self, w: &mut Writer) {
        w.str_max(&self.s1, 0x3FF).str_max(&self.s2, 0x3FF).str_max(&self.s3, 0x3FF);
        if self.servers.len() > 8 {
            w.fail(Error::Limit("9E servers (8)"));
        }
        w.count_u8(self.servers.len(), "9E servers");
        for s in &self.servers {
            w.str_max(&s.name, 0x3F).str_max(&s.address, 0x3F).u16(s.a).u16(s.b).u8(s.c);
        }
    }
}
impl Decode for ServerInfo9E {
    fn decode(r: &mut Reader) -> Result<Self> {
        let (s1, s2, s3) = (r.str()?, r.str()?, r.str()?);
        let n = r.u8()? as usize;
        if n > 8 {
            return Err(Error::Limit("9E servers (8)"));
        }
        let mut servers = Vec::new();
        for _ in 0..n {
            servers.push(GameServer { name: r.str()?, address: r.str()?, a: r.u16()?, b: r.u16()?, c: r.u8()? });
        }
        Ok(ServerInfo9E { s1, s2, s3, servers })
    }
}

/// 0xB3 weapon workshop data (ordered), the answer to request `60 00 u16 dict`. PROVEN (docs/notes/workshop.md, 0x200289A00):
/// `u16 dict` (must equal the opened item's dict, else the client re-sends 60 00), `u32 kills` (progress shown on
/// locked slots), `u32 matches`, `u32 n1, n1 x {str64 slot id, u8 locked}` (must list EVERY slot id of the weapon or
/// the client dereferences NULL), `u32 n2, n2 x u16` (LIKELY unlocked upgrade ids). Never send unrequested.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Workshop {
    pub dict: u16,
    pub kills: u32,
    pub matches: u32,
    pub slots: Vec<(String, u8)>,
    pub unlocked: Vec<u16>,
}
impl Encode for Workshop {
    fn encode(&self, w: &mut Writer) {
        w.u16(self.dict).u32(self.kills).u32(self.matches).count_u32(self.slots.len());
        for (s, l) in &self.slots {
            w.str_max(s, 0x3F).u8(*l);
        }
        w.count_u32(self.unlocked.len());
        for u in &self.unlocked {
            w.u16(*u);
        }
    }
}
impl Decode for Workshop {
    fn decode(r: &mut Reader) -> Result<Self> {
        let (dict, kills, matches) = (r.u16()?, r.u32()?, r.u32()?);
        let n1 = r.u32()? as usize;
        r.check_count(n1, 2)?;
        let mut slots = Vec::new();
        for _ in 0..n1 {
            slots.push((r.str()?, r.u8()?));
        }
        let n2 = r.u32()? as usize;
        r.check_count(n2, 2)?;
        let unlocked = (0..n2).map(|_| r.u16()).collect::<Result<_>>()?;
        Ok(Workshop { dict, kills, matches, slots, unlocked })
    }
}

/// 0xB8 event ids the client's 53 tracker reports (ordered). PROVEN: `u16 tag` (!= 0 ignores the
/// rest), `u16 n, n x u16` (set capacity 256, the 257th id = assert). Sent with tag 0 and {5, 6, 0x3D, 0x3E}:
/// 5/6 and 0x3D/0x3E = match won/lost, so the first 53 after a bot match tells the result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventIds {
    pub tag: u16,
    pub ids: Vec<u16>,
}
impl Encode for EventIds {
    fn encode(&self, w: &mut Writer) {
        if self.ids.len() > 256 {
            w.fail(Error::Limit("B8 ids (256)"));
        }
        w.u16(self.tag).count_u16(self.ids.len(), "B8 ids");
        for i in &self.ids {
            w.u16(*i);
        }
    }
}
impl Decode for EventIds {
    fn decode(r: &mut Reader) -> Result<Self> {
        let tag = r.u16()?;
        let n = r.u16()? as usize;
        r.check_count(n, 2)?;
        Ok(EventIds { tag, ids: (0..n).map(|_| r.u16()).collect::<Result<_>>()? })
    }
}

/// Minor pushes with a trivial layout (all PROVEN readers).
macro_rules! small_msg {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident : $t:ident),* $(,)? }) => {
        prim_msg! { $(#[$m])* $name { $($(#[$fm])* $f : $t),* } }
    };
}

small_msg! {
    /// 0xA4 (inline 0x20018F0AC): one byte, ignored; clears the "restricted / limited access" state.
    LimitedAccessOff { ignored: u8 }
}
small_msg! {
    /// 0xA8 (inline 0x20018E9BA): `u8 type, u8 group, u8 nomination/place` -> `root.add_important_message`.
    ImportantMessage { kind: u8, group: u8, place: u8 }
}
small_msg! {
    /// 0xB5 premium discount timer (inline 0x20018F730): `u32 a..e, f32 f`; loops over every 95 element
    /// (dictionary lookups without NULL check): send with an empty shop only.
    DiscountTimer { a: u32, b: u32, c: u32, d: u32, e: u32, f: f32 }
}
small_msg! {
    /// 0xB6 (inline 0x20018F6ED): `f32 + 1 byte`, same shop-element loop as B5.
    DiscountB6 { factor: f32, ignored: u8 }
}

/// 0xA5 booster buy popup (inline 0x20018F12F, PROVEN): `u8 n, n x {u16 dict, u8 currency, u32 amount}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoosterPopup {
    pub offers: Vec<(u16, u8, u32)>,
}
impl Encode for BoosterPopup {
    fn encode(&self, w: &mut Writer) {
        w.count_u8(self.offers.len(), "A5 offers");
        for &(d, c, a) in &self.offers {
            w.u16(d).u8(c).u32(a);
        }
    }
}
impl Decode for BoosterPopup {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u8()? as usize;
        Ok(BoosterPopup { offers: (0..n).map(|_| Ok((r.u16()?, r.u8()?, r.u32()?))).collect::<Result<_>>()? })
    }
}

/// 0xB2 admin text (inline 0x20018F7B4, PROVEN): `str256, str1024, u32, u32`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdminText {
    pub title: String,
    pub text: String,
    pub a: u32,
    pub b: u32,
}
impl Encode for AdminText {
    fn encode(&self, w: &mut Writer) {
        w.str_max(&self.title, 0xFF).str_max(&self.text, 0x3FF).u32(self.a).u32(self.b);
    }
}
impl Decode for AdminText {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(AdminText { title: r.str()?, text: r.str()?, a: r.u32()?, b: r.u32()? })
    }
}

/// 0x9A clan info, sub 0 only (reader 0x20018BD60, PROVEN start): `u8 0, u32 clan id`. Clan id 0 = no clan.
/// Other subs (member lists, ranks) are not modelled.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NoClan9A {
    pub clan_id: u32,
}
impl Encode for NoClan9A {
    fn encode(&self, w: &mut Writer) {
        w.u8(0).u32(self.clan_id);
    }
}
impl Decode for NoClan9A {
    fn decode(r: &mut Reader) -> Result<Self> {
        if r.u8()? != 0 {
            return Err(Error::Unsupported("9A sub other than 0"));
        }
        let clan_id = r.u32()?;
        if clan_id != 0 {
            return Err(Error::Unsupported("9A with a clan"));
        }
        Ok(NoClan9A { clan_id })
    }
}

/// 0xB4 (inline 0x20018F6CC, PROVEN): `u32, str64, u16`, all ignored by the client.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NoticeB4 {
    pub a: u32,
    pub text: String,
    pub b: u16,
}
impl Encode for NoticeB4 {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.a).str_max(&self.text, 0x3F).u16(self.b);
    }
}
impl Decode for NoticeB4 {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(NoticeB4 { a: r.u32()?, text: r.str()?, b: r.u16()? })
    }
}
