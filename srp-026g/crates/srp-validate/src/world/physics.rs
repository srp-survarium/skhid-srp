//! The physics section: the contact manifolds of the bodies that move in
//! the physics world (exe patch 121, `docs/notes/physics_contacts.md`),
//! read as typed views ([`Contacts`]) and written by [`Contacts::encode`].
//!
//! The game's own physics world writes nothing here and reads nothing
//! (`bullet_physics_world`'s vtable +40h is `ret 4`, 0x75AF0, and +44h
//! `ret 8`, 0x15A4B0, called by `game_world_core::serialize` 0x2D26D9 and
//! world apply 0x2D2C18), so a stock world's section is empty. But Bullet
//! keeps the contact points of each touching pair from step to step in a
//! persistent manifold (`btCollisionDispatcher` +0Ch/+14h, getNewManifold
//! 0x8F400), and no world carried them: a game that loaded a world (a
//! rollback, a resync, a joiner) stepped a falling or settling battery or
//! grenade from contacts of another moment than a game that stepped
//! straight through, and their worlds parted (the live split of match
//! 0x54417103 at 135.45 s: seven CRCs among fifteen games, the worlds
//! differing in one battery's transform alone). Patch 121 writes each such
//! manifold here, and every load puts them back.
//!
//! An empty section: no manifold of a moving body has points (and every
//! stock world). Otherwise:
//!
//! ```text
//! u8  version          1
//! u8  count            1..=MAX_CONTACTS, ascending by (body 0, body 1)
//! count times:
//!   key body 0         13 bytes: u8 kind, 3 x u32 (BodyKey)
//!   key body 1         13 bytes
//!   u8  body 0 first   1 when the near callback (0x8F860) passes body 0
//!                      first (the pair's +0Ch), else 0
//!   u8  points         1..=4 (m_cachedPoints)
//!   points x 145 bytes the btManifoldPoint's fields but its user pointer
//!                      and its three constraint rows, each vector's x y z
//!                      (POINT_BYTES; the order is the patch's
//!                      `vostok::physics_contacts::POINT_FIELDS`)
//! ```
//!
//! A body is named by what the world itself sets the same in every game
//! ([`BodyKey`]): a moving body by its origin (the world's battery or grenade
//! puts the body there), a fixed one by its broadphase box and filter (the
//! map's). The bits are the floats', -0 as 0.

use super::Section;
use crate::reader::{Reader, Short};
use crate::{Part, Reason, Refusal};

/// The format's version byte.
pub const VERSION: u8 = 1;
/// The most manifolds a world holds (the patch's pending records).
pub const MAX_CONTACTS: usize = 32;
/// The most points a manifold has (`MANIFOLD_CACHE_SIZE`).
pub const MAX_POINTS: usize = 4;
/// A point's bytes.
pub const POINT_BYTES: usize = 145;
/// A body key's bytes.
pub const KEY_BYTES: usize = 13;
/// A record's bytes before its points.
pub const HEAD_BYTES: usize = 2 * KEY_BYTES + 2;

/// A body of a manifold, by what every game that holds the same world has
/// the same. The order (the kind, then the words) is the records' and the
/// solver's (`patches::physics_contacts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BodyKey {
    /// A battery's or a grenade's body (filter group bit 800h): its origin's
    /// bits (the world's transform of it).
    Moving([u32; 3]),
    /// A fixed body (m_collisionFlags bit 1): its broadphase box's FNV-1a
    /// (64 bits, the box's six floats' bits) and its group | mask << 16.
    Fixed([u32; 3]),
    /// Any other (a kinematic one): its origin's bits.
    Other([u32; 3]),
}

impl BodyKey {
    fn kind(self) -> u8 {
        match self {
            BodyKey::Moving(_) => 1,
            BodyKey::Fixed(_) => 2,
            BodyKey::Other(_) => 3,
        }
    }

    fn words(self) -> [u32; 3] {
        match self {
            BodyKey::Moving(w) | BodyKey::Fixed(w) | BodyKey::Other(w) => w,
        }
    }

    /// Its 13 bytes.
    pub fn encode(self) -> [u8; KEY_BYTES] {
        let mut out = [0u8; KEY_BYTES];
        let [a, b, c] = self.words();
        for (slot, byte) in out.iter_mut().zip(
            [self.kind()]
                .into_iter()
                .chain(a.to_le_bytes())
                .chain(b.to_le_bytes())
                .chain(c.to_le_bytes()),
        ) {
            *slot = byte;
        }
        out
    }

    fn read(reader: &mut Reader<'_>) -> Result<BodyKey, ContactsFault> {
        let at = reader.at();
        let kind = reader.u8()?;
        let words = [reader.u32()?, reader.u32()?, reader.u32()?];
        match kind {
            1 => Ok(BodyKey::Moving(words)),
            2 => Ok(BodyKey::Fixed(words)),
            3 => Ok(BodyKey::Other(words)),
            value => Err(ContactsFault::Value { at, field: "body kind", value }),
        }
    }
}

/// A float's bits as a key takes them: -0 as 0.
pub fn key_bits(bits: u32) -> u32 {
    if bits == 0x8000_0000 { 0 } else { bits }
}

/// FNV-1a (64 bits) of words, a fixed body's box.
pub fn fnv1a(words: &[u32]) -> u64 {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    for word in words {
        for byte in word.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    hash
}

/// The number of points of a manifold, 1..=4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointCount(u8);

impl PointCount {
    pub fn new(points: usize) -> Option<PointCount> {
        u8::try_from(points).ok().filter(|n| (1..=MAX_POINTS as u8).contains(n)).map(PointCount)
    }

    pub fn get(self) -> usize {
        usize::from(self.0)
    }
}

/// A record's head: its bodies in the manifold's order, which of them the
/// near callback passes first, its points' count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactHead {
    pub body0: BodyKey,
    pub body1: BodyKey,
    pub body0_first: bool,
    pub points: PointCount,
}

impl ContactHead {
    /// Its bytes.
    pub fn encode(self) -> [u8; HEAD_BYTES] {
        let mut out = [0u8; HEAD_BYTES];
        for (slot, byte) in out.iter_mut().zip(
            self.body0
                .encode()
                .into_iter()
                .chain(self.body1.encode())
                .chain([u8::from(self.body0_first), self.points.0]),
        ) {
            *slot = byte;
        }
        out
    }

    /// The order of the records (and of the solver's manifolds).
    pub fn order(&self) -> (BodyKey, BodyKey) {
        (self.body0, self.body1)
    }
}

/// A manifold of the section: its head and its points' bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contact<'a> {
    pub head: ContactHead,
    /// `head.points` times [`POINT_BYTES`].
    pub points: &'a [u8],
}

impl<'a> Contact<'a> {
    /// Point `i`'s bytes.
    pub fn point(&self, i: usize) -> Option<&'a [u8]> {
        let start = i.checked_mul(POINT_BYTES)?;
        self.points.get(start..start.checked_add(POINT_BYTES)?)
    }
}

/// The section as the patch writes it: no record, or the version, the count
/// and the records, each in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contacts<'a> {
    count: usize,
    records: &'a [u8],
    base: usize,
}

/// What is not as the patch writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactsFault {
    Short(Short),
    /// A byte is none of the values its field takes.
    Value { at: usize, field: &'static str, value: u8 },
    /// A record is not after the one before it (the writer sorts them, and
    /// a pair has one record).
    Order { at: usize, record: usize },
    /// Bytes after the last record.
    Trailing { at: usize },
}

impl From<Short> for ContactsFault {
    fn from(short: Short) -> Self {
        ContactsFault::Short(short)
    }
}

impl core::fmt::Display for ContactsFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            ContactsFault::Short(short) => write!(f, "cut short at {}", short.at),
            ContactsFault::Value { at, field, value } => write!(f, "{field} {value} at {at}"),
            ContactsFault::Order { at, record } => {
                write!(f, "record {record} at {at} is not after the one before")
            }
            ContactsFault::Trailing { at } => write!(f, "bytes after the last record at {at}"),
        }
    }
}

impl<'a> Contacts<'a> {
    /// The section's records, each checked; `base` is where `bytes` start in
    /// the body.
    pub fn parse(bytes: &'a [u8], base: usize) -> Result<Contacts<'a>, ContactsFault> {
        if bytes.is_empty() {
            return Ok(Contacts { count: 0, records: bytes, base });
        }
        let mut reader = Reader::new(bytes, base);
        let at = reader.at();
        let version = reader.u8()?;
        if version != VERSION {
            return Err(ContactsFault::Value { at, field: "version", value: version });
        }
        let at = reader.at();
        let count = reader.u8()?;
        if count == 0 || usize::from(count) > MAX_CONTACTS {
            return Err(ContactsFault::Value { at, field: "count", value: count });
        }
        let start = reader;
        let mut last: Option<(BodyKey, BodyKey)> = None;
        for record in 0..usize::from(count) {
            let at = reader.at();
            let contact = read_contact(&mut reader)?;
            if last.is_some_and(|before| before >= contact.head.order()) {
                return Err(ContactsFault::Order { at, record });
            }
            last = Some(contact.head.order());
        }
        if !reader.is_done() {
            return Err(ContactsFault::Trailing { at: reader.at() });
        }
        Ok(Contacts { count: usize::from(count), records: reader.since(&start), base: start.at() })
    }

    /// How many records.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The records, in order.
    pub fn iter(&self) -> impl Iterator<Item = Contact<'a>> + 'a {
        let mut reader = Reader::new(self.records, self.base);
        (0..self.count).map_while(move |_| read_contact(&mut reader).ok())
    }

    /// The section's bytes for `records` (heads with their points' bytes),
    /// handed to `out` piece by piece: nothing for none. The records must be
    /// in order, one a pair, at most [`MAX_CONTACTS`] (the writer's; the
    /// parse refuses anything else).
    pub fn encode<'r>(
        records: impl ExactSizeIterator<Item = (ContactHead, &'r [u8])>,
        mut out: impl FnMut(&[u8]),
    ) {
        let count = records.len();
        let Ok(count) = u8::try_from(count) else { return };
        if count == 0 {
            return;
        }
        out(&[VERSION, count]);
        for (head, points) in records {
            out(&head.encode());
            out(points);
        }
    }
}

fn read_contact<'a>(reader: &mut Reader<'a>) -> Result<Contact<'a>, ContactsFault> {
    let body0 = BodyKey::read(reader)?;
    let body1 = BodyKey::read(reader)?;
    let at = reader.at();
    let body0_first = match reader.u8()? {
        0 => false,
        1 => true,
        value => return Err(ContactsFault::Value { at, field: "body 0 first", value }),
    };
    let at = reader.at();
    let n = reader.u8()?;
    let points = PointCount::new(usize::from(n))
        .ok_or(ContactsFault::Value { at, field: "points", value: n })?;
    let bytes = reader.take(points.get().saturating_mul(POINT_BYTES))?;
    Ok(Contact { head: ContactHead { body0, body1, body0_first, points }, points: bytes })
}

/// The physics section holds what patch 121 writes ([`Contacts::parse`]).
pub fn physics(section: &Section) -> Result<(), Refusal> {
    Contacts::parse(section.bytes, section.at)
        .map(|_| ())
        .map_err(|fault| Refusal::new(Part::Physics, fault_at(fault, section.at), Reason::PhysicsContacts { fault }))
}

fn fault_at(fault: ContactsFault, base: usize) -> usize {
    match fault {
        ContactsFault::Short(short) => short.at,
        ContactsFault::Value { at, .. } | ContactsFault::Order { at, .. } | ContactsFault::Trailing { at } => at,
    }
    .max(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn head(body0: BodyKey, body1: BodyKey, n: usize) -> ContactHead {
        ContactHead { body0, body1, body0_first: false, points: PointCount::new(n).unwrap() }
    }

    fn encoded(records: &[(ContactHead, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        Contacts::encode(records.iter().map(|(h, p)| (*h, p.as_slice())), |b| out.extend_from_slice(b));
        out
    }

    #[test]
    fn round_trip() {
        let battery = BodyKey::Moving([0x3f80_0000, 2, 3]);
        let level = BodyKey::Fixed([7, 8, 0x0010_0008]);
        let records = [
            (head(battery, level, 2), (0..2 * POINT_BYTES).map(|i| i as u8).collect::<Vec<u8>>()),
            (head(BodyKey::Moving([0x3f80_0000, 2, 4]), level, 1), std::vec![9u8; POINT_BYTES]),
        ];
        let bytes = encoded(&records);
        assert_eq!(bytes.len(), 2 + 2 * HEAD_BYTES + 3 * POINT_BYTES);
        let parsed = Contacts::parse(&bytes, 100).unwrap();
        assert_eq!(parsed.len(), 2);
        let got: Vec<_> = parsed.iter().collect();
        assert_eq!(got[0].head, records[0].0);
        assert_eq!(got[0].points, records[0].1.as_slice());
        assert_eq!(got[1].point(0), Some(&[9u8; POINT_BYTES][..]));
        assert_eq!(got[1].point(1), None);
    }

    #[test]
    fn empty_is_none() {
        assert!(Contacts::parse(&[], 0).unwrap().is_empty());
        assert!(encoded(&[]).is_empty());
    }

    #[test]
    fn refusals() {
        let battery = BodyKey::Moving([1, 2, 3]);
        let level = BodyKey::Fixed([7, 8, 9]);
        let good = encoded(&[(head(battery, level, 1), std::vec![0u8; POINT_BYTES])]);
        let mut bad = good.clone();
        bad[0] = 2;
        assert!(matches!(Contacts::parse(&bad, 0), Err(ContactsFault::Value { field: "version", .. })));
        let mut bad = good.clone();
        bad[1] = 0;
        assert!(matches!(Contacts::parse(&bad, 0), Err(ContactsFault::Value { field: "count", .. })));
        let mut bad = good.clone();
        bad[2] = 4;
        assert!(matches!(Contacts::parse(&bad, 0), Err(ContactsFault::Value { field: "body kind", .. })));
        let mut bad = good.clone();
        bad[2 + 2 * KEY_BYTES] = 2;
        assert!(matches!(Contacts::parse(&bad, 0), Err(ContactsFault::Value { field: "body 0 first", .. })));
        for n in [0u8, 5] {
            let mut bad = good.clone();
            bad[3 + 2 * KEY_BYTES] = n;
            assert!(Contacts::parse(&bad, 0).is_err());
        }
        let mut bad = good.clone();
        bad.push(0);
        assert!(matches!(Contacts::parse(&bad, 0), Err(ContactsFault::Trailing { .. })));
        assert!(matches!(Contacts::parse(&good[..good.len() - 1], 0), Err(ContactsFault::Short(_))));
        // out of order, and a pair twice
        let twice = encoded(&[
            (head(battery, level, 1), std::vec![0u8; POINT_BYTES]),
            (head(battery, level, 1), std::vec![0u8; POINT_BYTES]),
        ]);
        assert!(matches!(Contacts::parse(&twice, 0), Err(ContactsFault::Order { record: 1, .. })));
        let reversed = encoded(&[
            (head(BodyKey::Moving([1, 2, 4]), level, 1), std::vec![0u8; POINT_BYTES]),
            (head(battery, level, 1), std::vec![0u8; POINT_BYTES]),
        ]);
        assert!(Contacts::parse(&reversed, 0).is_err());
    }

    #[test]
    fn key_order_is_kind_then_words() {
        assert!(BodyKey::Moving([9, 9, 9]) < BodyKey::Fixed([0, 0, 0]));
        assert!(BodyKey::Fixed([1, 0, 0]) < BodyKey::Fixed([1, 0, 1]));
        assert!(BodyKey::Fixed([0, 0, 0]) < BodyKey::Other([0, 0, 0]));
        assert_eq!(key_bits(0x8000_0000), 0);
        assert_eq!(key_bits(0x3f80_0000), 0x3f80_0000);
    }
}
