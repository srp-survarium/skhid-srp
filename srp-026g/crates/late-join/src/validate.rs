//! Build-2777 profile validation before the native, unchecked reader runs:
//! a descriptor the server sends is read once here, and the game's own
//! decoder (0x2F7C50) only ever sees one that passed.

/// What the check needs of the game's items_dictionary (live, never
/// packet-controlled).
pub trait Dictionary {
    /// The item `id`'s record kind, if the dictionary has it.
    fn item(&self, id: u16) -> Option<ItemRecord>;
    /// Whether the dictionary has the modification `id` (of its defects or
    /// its ordinary ones).
    fn has_modification(&self, id: u32, kind: Modification) -> bool;
}

/// How much state an item carries on the wire after its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemRecord {
    /// 4 bytes (the row's +22Ah set).
    Short,
    /// 12 bytes.
    Long,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modification {
    Ordinary,
    Defect,
}

/// The most bytes a descriptor may have.
pub const MAX_BYTES: usize = 32768;
const SLOTS: u32 = 23;
const MAX_ITEMS: u32 = 69;

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.bytes.split_at_checked(n)?;
        self.bytes = rest;
        Some(taken)
    }

    /// A little-endian number of `n` bytes.
    fn number(&mut self, n: usize) -> Option<u32> {
        Some(self.take(n)?.iter().rev().fold(0, |value, &b| value << 8 | u32::from(b)))
    }

    /// A string: its length (a byte, at most `most`) and its bytes.
    fn string(&mut self, most: u32) -> Option<()> {
        let length = self.number(1)?;
        if length > most {
            return None;
        }
        self.take(length as usize).map(drop)
    }
}

/// Whether `bytes` are a profile descriptor the game's reader can take, its
/// items all the dictionary's: the first build's late_join_validate_profile.
pub fn profile(bytes: &[u8], dictionary: &impl Dictionary) -> bool {
    bytes.len() <= MAX_BYTES && parse(&mut Reader { bytes }, dictionary).is_some()
}

fn parse(r: &mut Reader, dictionary: &impl Dictionary) -> Option<()> {
    if r.number(1)? > 1 {
        return None;
    }
    r.string(63)?;
    r.string(14)?;
    r.number(4)?; // protection
    let kits = r.number(1)?;
    let items = r.number(1)?;
    if kits == 0 || kits > 3 || items > MAX_ITEMS {
        return None;
    }
    let mut references = [0u16; 3 * SLOTS as usize];
    let mut referenced = 0;
    let mut usable = false;
    for _ in 0..kits {
        usable |= r.number(1)? == 0;
        r.number(4)?; // kit id
        r.string(63)?;
        let mask = r.number(4)?;
        if mask >> SLOTS != 0 {
            return None;
        }
        for slot in 0..SLOTS {
            if mask & 1 << slot == 0 {
                continue;
            }
            let index = r.number(2)?;
            if index >= items {
                return None;
            }
            *references.get_mut(referenced)? = index as u16;
            referenced += 1;
            r.number(2)?; // stack count
        }
        // Native 128-bit modifier mask: only its low 68 bits are valid.
        // Wire dwords are [bits64..95, bits96..127, bits0..31, bits32..63].
        let masks = [r.number(4)?, r.number(4)?, r.number(4)?, r.number(4)?];
        if masks[0] >> 4 != 0 || masks[1] != 0 {
            return None;
        }
        for mask in masks {
            for _ in 0..mask.count_ones() {
                if r.number(4)? & 0x7F80_0000 == 0x7F80_0000 {
                    return None; // not a finite float
                }
            }
        }
    }
    if !usable {
        return None;
    }
    for index in 0..items {
        r.number(4)?; // instance id
        let id = r.number(2)?;
        if id == 0 {
            if references[..referenced].contains(&(index as u16)) {
                return None;
            }
            continue;
        }
        let record = dictionary.item(id as u16)?;
        r.take(match record {
            ItemRecord::Short => 4,
            ItemRecord::Long => 12,
        })?;
        let modifications = r.number(1)?;
        if modifications > 8 {
            return None;
        }
        for _ in 0..modifications {
            let id = r.number(4)?;
            let kind = match r.number(1)? {
                0 => Modification::Ordinary,
                1 => Modification::Defect,
                _ => return None,
            };
            if !dictionary.has_modification(id, kind) {
                return None;
            }
        }
        let upgrades = r.number(1)?;
        if upgrades > 3 {
            return None;
        }
        r.take(upgrades as usize * 17)?;
    }
    r.bytes.is_empty().then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    struct Items;

    impl Dictionary for Items {
        fn item(&self, id: u16) -> Option<ItemRecord> {
            match id {
                7 => Some(ItemRecord::Short),
                9 => Some(ItemRecord::Long),
                _ => None,
            }
        }

        fn has_modification(&self, id: u32, kind: Modification) -> bool {
            (id, kind) == (5, Modification::Ordinary) || (id, kind) == (6, Modification::Defect)
        }
    }

    /// One kit holding item 0 in slot 0, items 7 (short) and 9 (long, one
    /// modification, one upgrade).
    fn profile_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        b.push(1); // version
        b.extend([3, b'a', b'b', b'c']); // name
        b.push(0); // second string
        b.extend(0u32.to_le_bytes()); // protection
        b.extend([1, 2]); // kits, items
        b.push(0); // usable
        b.extend(77u32.to_le_bytes()); // kit id
        b.push(0); // kit name
        b.extend(1u32.to_le_bytes()); // slot 0
        b.extend(0u16.to_le_bytes()); // item 0
        b.extend(1u16.to_le_bytes()); // stack
        b.extend(1u32.to_le_bytes()); // modifier mask: one value
        for _ in 0..3 {
            b.extend(0u32.to_le_bytes());
        }
        b.extend(1.5f32.to_le_bytes());
        // item 0
        b.extend(100u32.to_le_bytes());
        b.extend(7u16.to_le_bytes());
        b.extend([0; 4]);
        b.push(0); // modifications
        b.push(0); // upgrades
        // item 1
        b.extend(101u32.to_le_bytes());
        b.extend(9u16.to_le_bytes());
        b.extend([0; 12]);
        b.push(1);
        b.extend(5u32.to_le_bytes());
        b.push(0);
        b.push(1);
        b.extend([0; 17]);
        b
    }

    #[test]
    fn a_whole_profile_passes() {
        assert!(profile(&profile_bytes(), &Items));
    }

    #[test]
    fn every_short_read_fails() {
        let bytes = profile_bytes();
        for n in 0..bytes.len() {
            assert!(!profile(&bytes[..n], &Items), "{n} bytes");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(!profile(&longer, &Items));
    }

    #[test]
    fn refusals() {
        let bytes = profile_bytes();
        let changed = |at: usize, value: u8| {
            let mut b = bytes.clone();
            b[at] = value;
            profile(&b, &Items)
        };
        assert!(!changed(0, 2)); // version
        assert!(!changed(1, 64)); // a name past 63
        assert!(!changed(10, 4)); // four kits
        assert!(!changed(10, 0)); // none
        assert!(!changed(11, 70)); // 70 items
        assert!(!changed(12, 1)); // no usable kit
        assert!(!changed(21, 0x80)); // slot 31
        assert!(!changed(18, 3)); // slot 1 too: the bytes after are not its item
        assert!(!changed(22, 9)); // an item index past the items
        assert!(!changed(26, 0x10)); // modifier mask bits past 68
        assert!(!changed(30, 1));
        assert!(!changed(45, 0x7F)); // a NaN modifier
        assert!(!changed(50, 8)); // an item the dictionary lacks
        assert!(!changed(56, 9)); // nine modifications
        assert!(!changed(57, 4)); // four upgrades
        // the referenced item 0 empty
        let mut b = bytes.clone();
        b[50] = 0;
        b[51] = 0;
        assert!(!profile(&b, &Items));
        // item 1 empty: no kit holds it
        let mut b = bytes[..58].to_vec();
        b.extend(101u32.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        assert!(profile(&b, &Items));
        // a modification the dictionary lacks, a defect flag of 2 or of 1
        assert!(changed(77, 5));
        assert!(!changed(77, 6));
        assert!(!changed(81, 2));
        assert!(!changed(81, 1));
        assert!(!profile(&[0; MAX_BYTES + 1], &Items));
    }
}
