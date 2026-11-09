//! A newcomer's profile as the server sends it in a proposal, parsed before
//! the game's own unchecked reader (player_profile::deserialize_static
//! 0x1F1070) may see it (before the Rust port
//! `late_join_validate_026g.c`). The game's reader writes a profile body of
//! fixed room (0x45FC bytes: three kits, 69 items inline) and looks every
//! item up in the dictionary without a check: a profile that is longer, has
//! more kits, items, modifications or upgrades than that room, names an item
//! or a modification the dictionary does not have, or a slot of an empty
//! item, writes over memory or crashes the game. [`CheckedProfile`] is the
//! only way to the reader: made here alone, from bytes that are none of
//! those.
//!
//! After the profile comes srp's block when the newcomer's face is not 0
//! (face-choice.md: `SRPX 03 01 <face>`, the faces entry of its one place):
//! read here with the profile, once, into the newcomer's [`Face`] for patch
//! 123; anything else after the profile refuses the proposal.

use super::runtime::{KitMask, MAX_PROFILE};
use crate::faces::{self, Face, Name};

/// The kits of a profile body.
const KITS: u32 = 3;
/// A kit's slots (the reader's mask loop, 0x17).
const SLOTS: u32 = 23;
/// The items of a profile body.
const ITEMS: u32 = 69;
/// The texts' room, their zero aside: the name's 64 bytes, the clan's 15.
const NAME: u32 = 63;
const CLAN: u32 = 14;
const KIT_NAME: u32 = 63;
/// An item's modifications and upgrades held inline.
const MODIFICATIONS: u32 = 8;
const UPGRADES: u32 = 3;
/// An upgrade's bytes: u32 id, u32, u64, u8.
const UPGRADE: usize = 17;

/// What an item record carries after its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemRecord {
    /// A stack (the dictionary row's +22Ah set): its count, 4 bytes.
    Stack,
    /// Its condition, repair and reroll times: 12 bytes.
    Single,
}

impl ItemRecord {
    fn bytes(self) -> usize {
        match self {
            ItemRecord::Stack => 4,
            ItemRecord::Single => 12,
        }
    }
}

/// Which of the dictionary's two tables a modification is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modification {
    Ordinary,
    Defect,
}

/// What the parse needs of the game's items_dictionary: the match's own,
/// live (never anything the packet says).
pub trait Dictionary {
    /// The record the item `id` has, if the dictionary has the item.
    fn item(&self, id: u16) -> Option<ItemRecord>;
    /// Whether the dictionary has the modification `id` in that table.
    fn has_modification(&self, id: u32, kind: Modification) -> bool;
}

/// A proposal's body the game's reader can take: the profile, which its
/// reader takes whole, and the kits its players are loaded for (those whose
/// first byte is 0: the reader keeps it as the kit's +758h, which 0 makes
/// usable); its player's name and srp's face of it, from the block after.
#[derive(Clone, Copy, Debug)]
pub struct CheckedProfile<'a> {
    /// The whole body: the profile, then srp's block if any (what a
    /// retransmission of the proposal repeats byte for byte).
    body: &'a [u8],
    /// The body's front: the profile alone.
    profile: &'a [u8],
    usable: KitMask,
    /// What a face is known by; none for a name patch 123 cannot keep (empty,
    /// or with a zero in it), whose face no one asks for.
    name: Option<Name>,
    face: Face,
}

impl<'a> CheckedProfile<'a> {
    /// `body`, when it is a profile of `dictionary`'s items that fits a
    /// profile body, with a usable kit, then nothing or srp's block with the
    /// newcomer's face ([`faces::proposal_face`]).
    pub fn parse(body: &'a [u8], dictionary: &impl Dictionary) -> Option<CheckedProfile<'a>> {
        if body.len() > MAX_PROFILE {
            return None;
        }
        let mut r = Reader(body);
        let (name, usable) = profile(&mut r, dictionary)?;
        let block = r.0;
        let face = faces::proposal_face(block)?;
        let profile = &body[..body.len() - block.len()];
        Some(CheckedProfile { body, profile, usable, name: Name::of_bytes(name), face })
    }

    /// The whole body: what the proposal's storage keeps and compares.
    pub fn body(&self) -> &'a [u8] {
        self.body
    }

    /// The profile alone: what the game's reader takes, to its end.
    pub fn profile(&self) -> &'a [u8] {
        self.profile
    }

    pub fn usable(&self) -> KitMask {
        self.usable
    }

    pub fn name(&self) -> Option<Name> {
        self.name
    }

    /// srp's face of the newcomer: its block's, face 0 without one.
    pub fn face(&self) -> Face {
        self.face
    }
}

/// Little-endian fields from the front; a read past the end is none.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(taken)
    }

    /// A number of `n` bytes.
    fn number(&mut self, n: usize) -> Option<u32> {
        Some(self.take(n)?.iter().rev().fold(0, |value, &b| value << 8 | u32::from(b)))
    }

    /// A text: its length (a byte, at most `most`) and its bytes.
    fn text(&mut self, most: u32) -> Option<&'a [u8]> {
        let length = self.number(1)?;
        (length <= most).then_some(())?;
        self.take(length as usize)
    }
}

/// The profile at the reader's front: its player's name and its usable kits.
fn profile<'a>(r: &mut Reader<'a>, dictionary: &impl Dictionary) -> Option<(&'a [u8], KitMask)> {
    // the team: 0 or 1
    (r.number(1)? <= 1).then_some(())?;
    let name = r.text(NAME)?;
    r.text(CLAN)?;
    r.number(4)?; // protection
    let kits = r.number(1)?;
    let items = r.number(1)?;
    (1..=KITS).contains(&kits).then_some(())?;
    (items <= ITEMS).then_some(())?;
    // The items a kit's slot names: none of them may be empty.
    let mut slotted = 0u128;
    let mut usable = 0u8;
    for kit in 0..kits {
        if r.number(1)? == 0 {
            usable |= 1 << kit;
        }
        r.number(4)?; // kit id
        r.text(KIT_NAME)?;
        let mask = r.number(4)?;
        (mask >> SLOTS == 0).then_some(())?;
        for _ in (0..SLOTS).filter(|slot| mask & 1 << slot != 0) {
            let index = r.number(2)?;
            (index < items).then_some(())?;
            slotted |= 1 << index;
            r.number(2)?; // stack count
        }
        // The kit's 128-bit modifier mask, of which only the low 68 bits
        // name a modifier; the wire's dwords are bits 64..95, 96..127, 0..31
        // and 32..63. A value for each bit set, a finite float.
        let masks = [r.number(4)?, r.number(4)?, r.number(4)?, r.number(4)?];
        (masks[0] >> 4 == 0 && masks[1] == 0).then_some(())?;
        for _ in 0..masks.iter().map(|m| m.count_ones()).sum::<u32>() {
            let value = r.number(4)?;
            (value & EXPONENT != EXPONENT).then_some(())?;
        }
    }
    let usable = KitMask::parse(usable)?;
    for index in 0..items {
        r.number(4)?; // instance id
        let id = r.number(2)? as u16;
        if id == 0 {
            // an empty item: nothing after it, and no slot names it
            (slotted & 1 << index == 0).then_some(())?;
            continue;
        }
        r.take(dictionary.item(id)?.bytes())?;
        let modifications = r.number(1)?;
        (modifications <= MODIFICATIONS).then_some(())?;
        for _ in 0..modifications {
            let id = r.number(4)?;
            let kind = match r.number(1)? {
                0 => Modification::Ordinary,
                1 => Modification::Defect,
                _ => return None,
            };
            dictionary.has_modification(id, kind).then_some(())?;
        }
        let upgrades = r.number(1)?;
        (upgrades <= UPGRADES).then_some(())?;
        r.take(upgrades as usize * UPGRADE)?;
    }
    Some((name, usable))
}

/// An f32's exponent bits: all set is an infinity or no number.
const EXPONENT: u32 = 0xFF << 23;

#[cfg(test)]
mod tests {
    use super::*;

    /// check_late_join_026g.py's dictionary: items 101 (single) and 102 (a
    /// stack), modification 1230, defect 7.
    struct Fixture;

    impl Dictionary for Fixture {
        fn item(&self, id: u16) -> Option<ItemRecord> {
            match id {
                101 => Some(ItemRecord::Single),
                102 => Some(ItemRecord::Stack),
                _ => None,
            }
        }

        fn has_modification(&self, id: u32, kind: Modification) -> bool {
            matches!((id, kind), (1230, Modification::Ordinary) | (7, Modification::Defect))
        }
    }

    fn u32s(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// The check's two-kit profile: both item layouts, both tables, an
    /// upgrade; (bytes, the kits' offsets, the items' offsets).
    fn wire() -> (Vec<u8>, Vec<usize>, Vec<usize>) {
        let mut w = b"\x01\x03new\x02SR".to_vec();
        w.extend(10u32.to_le_bytes());
        w.extend(b"\x02\x02");
        let mut kits = vec![];
        for kit in 0..2u32 {
            kits.push(w.len());
            w.push(0);
            w.extend((kit + 1).to_le_bytes());
            w.extend(b"\x01K");
            w.extend(3u32.to_le_bytes());
            w.extend([0, 0, 1, 0, 1, 0, 9, 0]);
            w.extend(u32s(&[8, 0, 0, 0]));
            w.extend(1.25f32.to_le_bytes());
        }
        let mut items = vec![];
        for index in 0..2u32 {
            items.push(w.len());
            w.extend((index + 1).to_le_bytes());
            w.extend((101 + index as u16).to_le_bytes());
            w.extend(99u32.to_le_bytes());
            if index == 0 {
                w.extend(u32s(&[2, 3]));
            }
            w.push(2);
            w.extend(1230u32.to_le_bytes());
            w.push(0);
            w.extend(7u32.to_le_bytes());
            w.push(1);
            w.push(1);
            w.extend(u32s(&[31, 32]));
            w.extend(33u64.to_le_bytes());
            w.push(3);
        }
        (w, kits, items)
    }

    fn checks(bytes: &[u8]) -> bool {
        CheckedProfile::parse(bytes, &Fixture).is_some()
    }

    #[test]
    fn the_checks_profile_and_its_breakages() {
        let (w, kits, items) = wire();
        assert_eq!(CheckedProfile::parse(&w, &Fixture).map(|p| p.usable().bits()), Some(0b11));
        for length in 0..w.len() {
            assert!(!checks(&w[..length]), "cut at {length}");
        }
        let mut longer = w.clone();
        longer.push(0);
        assert!(!checks(&longer));
        let breakages: Vec<(usize, Vec<u8>)> = vec![
            (0, vec![2]), (1, vec![0x40]), (5, vec![0x0F]), (12, vec![4]), (13, vec![0x46]),
            (kits[0] + 7, (1u32 << 23).to_le_bytes().to_vec()), (kits[0] + 11, vec![2, 0]),
            (kits[0] + 19, 16u32.to_le_bytes().to_vec()), (kits[0] + 23, 1u32.to_le_bytes().to_vec()),
            (kits[0] + 35, 0x7F80_0000u32.to_le_bytes().to_vec()), (items[0] + 4, vec![0xFF, 0xFF]),
            (items[0] + 18, vec![9]), (items[0] + 19, 9999u32.to_le_bytes().to_vec()),
            (items[0] + 23, vec![2]), (items[0] + 29, vec![4]),
        ];
        for (at, new) in breakages {
            let mut broken = w.clone();
            broken[at..at + new.len()].copy_from_slice(&new);
            assert!(!checks(&broken), "broken at {at}");
        }
        // no usable kit
        let mut unusable = w.clone();
        for &at in &kits {
            unusable[at] = 2;
        }
        assert!(!checks(&unusable));
        // one usable kit of two
        let mut one = w.clone();
        one[kits[0]] = 5;
        assert_eq!(CheckedProfile::parse(&one, &Fixture).map(|p| p.usable().bits()), Some(0b10));
        // a slot naming an empty item
        let mut empty = w[..items[0] + 4].to_vec();
        empty.extend([0, 0]);
        empty.extend(&w[items[1]..]);
        assert!(!checks(&empty));
    }

    #[test]
    fn srps_block_after_the_profile_is_the_newcomers_face() {
        let (w, _, _) = wire();
        let plain = CheckedProfile::parse(&w, &Fixture).unwrap();
        assert_eq!((plain.body(), plain.profile(), plain.face()), (&w[..], &w[..], Face::FIRST));
        assert!(plain.name() == Name::of_bytes(b"new"));
        let faced = [&w[..], b"SRPX\x03\x01\x05"].concat();
        let checked = CheckedProfile::parse(&faced, &Fixture).unwrap();
        assert_eq!((checked.body(), checked.profile()), (&faced[..], &w[..]));
        assert_eq!((checked.face(), checked.usable().bits()), (Face::of_byte(5), 0b11));
        // A block srp does not send: the proposal refused, as a profile with
        // bytes after it is.
        for other in [&b"SRPX"[..], b"SRPX\x03\x01", b"SRPX\x03\x02\x01\x01", b"SRPX\x02\x01\x01",
                      b"SRPX\x03\x01\x01\x00", b"SRPX\x03\x01\x01\x03\x01\x01", b"SRPY\x03\x01\x01", b"\x00"] {
            assert!(!checks(&[&w[..], other].concat()), "{other:?}");
        }
        // A name no face is known by (a zero in it): the profile taken, no name.
        let mut unnamed = w.clone();
        unnamed[3] = 0;
        assert!(CheckedProfile::parse(&unnamed, &Fixture).unwrap().name().is_none());
    }

    #[test]
    fn the_largest_profile() {
        let mut w = vec![0, 63];
        w.extend([b'N'; 63]);
        w.push(14);
        w.extend([b'C'; 14]);
        w.extend(10u32.to_le_bytes());
        w.extend([3, 69]);
        for kit in 0..3u16 {
            w.push(0);
            w.extend(u32::from(kit + 1).to_le_bytes());
            w.push(63);
            w.extend([b'K'; 63]);
            w.extend(((1u32 << 23) - 1).to_le_bytes());
            for slot in 0..23 {
                w.extend((kit * 23 + slot).to_le_bytes());
                w.extend(1u16.to_le_bytes());
            }
            w.extend(u32s(&[15, 0, u32::MAX, u32::MAX]));
            for _ in 0..68 {
                w.extend(1.0f32.to_le_bytes());
            }
        }
        for index in 0..69u32 {
            w.extend((index + 1).to_le_bytes());
            w.extend((101 + (index % 2) as u16).to_le_bytes());
            w.extend(99u32.to_le_bytes());
            if index % 2 == 0 {
                w.extend(u32s(&[2, 3]));
            }
            w.push(8);
            for _ in 0..4 {
                w.extend(1230u32.to_le_bytes());
                w.push(0);
                w.extend(7u32.to_le_bytes());
                w.push(1);
            }
            w.push(3);
            for _ in 0..3 {
                w.extend(u32s(&[31, 32]));
                w.extend(33u64.to_le_bytes());
                w.push(3);
            }
        }
        assert_eq!(CheckedProfile::parse(&w, &Fixture).map(|p| p.usable().bits()), Some(0b111));
        for cut in [1, 17, 52] {
            assert!(!checks(&w[..w.len() - cut]));
        }
        let mut over = vec![0u8; MAX_PROFILE + 1];
        over[..w.len()].copy_from_slice(&w);
        assert!(!checks(&over));
    }
}
