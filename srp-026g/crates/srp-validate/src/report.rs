//! A client report: what a player's game tells the match server about
//! itself (client patches 125 and 126, `docs/notes/memory-report.md`,
//! `docs/notes/module-report.md`). One
//! carrier for every kind: an 0xE0 chunk of its own impossible prefix
//! ([`MAGIC`]), plain and resent until acked like the load receipts, a
//! report of several parts in as many carriers. The server logs each part
//! once and does nothing else with it.
//!
//! ```text
//! magic[8]  u32 match_id  u8 kind  u8 part  u8 parts  body (the kind's)
//!   kind 1 memory: crate::memory
//!   kind 2 modules: crate::modules
//! ```

use crate::{memory, modules};

/// An E0 chunk can never start so (offset 0xFFFF is past any total), and
/// the receipts' ([`crate::recovery::MAGIC`]) and the resource
/// acknowledgement have other bytes 6 and 7.
pub const MAGIC: [u8; 8] = [255, 255, 255, 255, 255, 255, 3, 0];
const HEADER: usize = MAGIC.len() + 4 + 3;
/// A carrier's bytes at most: one E0 chunk's data.
pub const MAX_SIZE: usize = 480;
const _: () = assert!(HEADER + memory::MAX_BODY <= MAX_SIZE);
const _: () = assert!(HEADER + modules::MAX_BODY <= MAX_SIZE);

/// When in a match a report was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Moment {
    /// The connection's first world is in: the level, its bakes and the
    /// players' textures are loaded.
    Loaded,
    /// The match is closing on the game (close_current_match), its world
    /// still there: the match's own peak.
    Ended,
}

impl Moment {
    pub fn word(self) -> &'static str {
        match self {
            Moment::Loaded => "loaded",
            Moment::Ended => "ended",
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Moment::Loaded => 0,
            Moment::Ended => 1,
        }
    }

    pub(crate) fn of(byte: u8) -> Option<Moment> {
        match byte {
            0 => Some(Moment::Loaded),
            1 => Some(Moment::Ended),
            _ => None,
        }
    }
}

/// The bytes of the carrier that starts `bytes`, more of a datagram
/// possibly after it: what the transport needs to find the next message
/// of a joined datagram (`udp-transport`'s `carrier_len`). `None`: no
/// carrier of a known kind starts there, or it is cut short. Only its
/// length: [`Carrier::parse`] reads the rest.
pub fn carrier_len(bytes: &[u8]) -> Option<usize> {
    let (header, body) = bytes.split_at_checked(HEADER)?;
    if header.get(..MAGIC.len())? != MAGIC {
        return None;
    }
    let len = match header.get(MAGIC.len().checked_add(4)?)? {
        1 => memory::body_len(body)?,
        2 => modules::body_len(body)?,
        _ => return None,
    };
    let len = HEADER.checked_add(len)?;
    (bytes.len() >= len).then_some(len)
}

/// A whole report, as the client gathers it.
pub enum Report {
    Memory(memory::Report),
    Modules(modules::Report),
}

impl Report {
    fn kind(&self) -> u8 {
        match self {
            Report::Memory(_) => 1,
            Report::Modules(_) => 2,
        }
    }

    /// How many carriers it goes in.
    pub fn parts(&self) -> u8 {
        match self {
            Report::Memory(report) => report.parts(),
            Report::Modules(report) => report.parts(),
        }
    }

    /// Carrier `part` (0-based) of match `match_id`, encoded, and its length.
    pub fn part(&self, match_id: u32, part: u8) -> Option<([u8; MAX_SIZE], usize)> {
        let mut bytes = [0; MAX_SIZE];
        let (header, body) = bytes.split_at_mut(HEADER);
        let (magic, header) = header.split_at_mut(MAGIC.len());
        magic.copy_from_slice(&MAGIC);
        let (id, header) = header.split_at_mut(4);
        id.copy_from_slice(&match_id.to_le_bytes());
        header.copy_from_slice(&[self.kind(), part, self.parts()]);
        let len = match self {
            Report::Memory(report) => report.put_part(part, body)?,
            Report::Modules(report) => report.put_part(part, body)?,
        };
        Some((bytes, HEADER.checked_add(len)?))
    }
}

/// A carrier's body as the server reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    Memory(memory::Part),
    Modules(modules::Part),
}

/// One carrier as the server reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carrier {
    pub match_id: u32,
    pub part: u8,
    pub parts: u8,
    pub body: Body,
}

impl Carrier {
    /// Exactly a carrier: the magic, a kind, `part < parts` within the
    /// kind's most, and its body.
    pub fn parse(bytes: &[u8]) -> Option<Carrier> {
        if carrier_len(bytes)? != bytes.len() {
            return None;
        }
        let (header, body) = bytes.split_at_checked(HEADER)?;
        let (magic, header) = header.split_at_checked(MAGIC.len())?;
        if magic != MAGIC {
            return None;
        }
        let (id, header) = header.split_at_checked(4)?;
        let &[kind, part, parts] = header else { return None };
        if part >= parts {
            return None;
        }
        let full = part.checked_add(1)? < parts;
        let body = match kind {
            1 if parts <= memory::MAX_PARTS => Body::Memory(memory::Part::parse(body, part == 0, full)?),
            2 if parts <= modules::MAX_PARTS => Body::Modules(modules::Part::parse(body, full)?),
            _ => return None,
        };
        Some(Carrier { match_id: u32::from_le_bytes(id.try_into().ok()?), part, parts, body })
    }

    /// What tells one part of a connection's reports from another (a
    /// copy, resent until acked, has the same): its kind, the report's
    /// moment, the part.
    pub fn key(&self) -> (u8, u8, u8) {
        match self.body {
            Body::Memory(part) => (1, part.moment.byte(), self.part),
            Body::Modules(part) => (2, part.moment.byte(), self.part),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Arena, Moment, Name, Process, Usage};

    fn report(arenas: u32, moment: Moment) -> Report {
        let mut report = memory::Report::new(moment, Process { free: 9, largest_free: 3, video_budget: 1 << 33 });
        for n in 0..arenas {
            report.add(Arena { name: Name::of_c(b"survarium"), size: n, usage: Usage::Heap { used: n, largest_free: 1 } });
        }
        Report::Memory(report)
    }

    #[test]
    fn carriers_round_trip_and_fit_a_chunk() {
        let report = report(17, Moment::Loaded);
        assert_eq!(report.parts(), 2);
        let mut keys = std::vec::Vec::new();
        for part in 0..report.parts() {
            let (bytes, len) = report.part(0x8123_4567, part).unwrap();
            assert!(len <= MAX_SIZE);
            let carrier = Carrier::parse(&bytes[..len]).unwrap();
            assert_eq!((carrier.match_id, carrier.part, carrier.parts), (0x8123_4567, part, 2));
            keys.push(carrier.key());
            let mut longer = bytes[..len].to_vec();
            longer.push(0);
            assert_eq!(Carrier::parse(&longer), None);
        }
        assert_eq!(keys, [(1, 0, 0), (1, 0, 1)]);
        // its length from its start alone, whatever follows it; none cut short
        for part in 0..report.parts() {
            let (bytes, len) = report.part(1, part).unwrap();
            let mut joined = bytes[..len].to_vec();
            joined.extend([0xe1, 0xff, 0xff]);
            assert_eq!(carrier_len(&joined), Some(len));
            assert!((0..len).all(|cut| carrier_len(&bytes[..cut]).is_none()));
        }
        let ended = super::tests::report(0, Moment::Ended);
        let (bytes, len) = ended.part(1, 0).unwrap();
        assert_eq!(Carrier::parse(&bytes[..len]).unwrap().key(), (1, 1, 0));
        assert_eq!(report.part(1, 2), None);
    }

    #[test]
    fn a_carrier_is_no_chunk_receipt_or_other_kind() {
        let report = report(3, Moment::Loaded);
        let (bytes, len) = report.part(7, 0).unwrap();
        let bytes = &bytes[..len];
        for at in [0, 5, 6, 7, 12, 13, 14] {
            let mut bad = bytes.to_vec();
            bad[at] ^= 0x80;
            assert_eq!(Carrier::parse(&bad), None, "byte {at}");
        }
        let mut more_parts = bytes.to_vec();
        more_parts[14] = memory::MAX_PARTS + 1;
        assert_eq!(Carrier::parse(&more_parts), None);
        assert_eq!(crate::recovery::Receipt::parse(bytes), None);
        assert_ne!(MAGIC, crate::recovery::MAGIC);
        // an older server takes it for a chunk whose offset is past its total
        let offset = u16::from_le_bytes([bytes[4], bytes[5]]);
        let total = u16::from_le_bytes([bytes[6], bytes[7]]);
        assert!(offset >= total);
    }
}
