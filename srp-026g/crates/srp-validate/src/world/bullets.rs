//! Current bullet records: a 16-byte header and N fixed 69-byte records.
//! Patch 85 always saves/loads the final quiet byte and clamps stale ammo
//! ordinals after weapon pickups. Original profile ammo cannot bound a bullet
//! fired by a picked-up gun. Validate framing, owners, slots and finite floats.
use super::{Frame, WEAPON_SLOTS};
use crate::reader::Reader;
use crate::{Part, Reason, Refusal, finite};

const HEADER: usize = 16;
const RECORD: usize = 69;

pub fn bullets(frame: &Frame<'_>) -> Result<(), Refusal> {
    let section = frame.bullets();
    let unread = |count| {
        Refusal::new(
            Part::Bullets,
            section.at,
            Reason::BulletsUnread {
                len: section.bytes.len(),
                bullets: count,
            },
        )
    };
    let mut reader = Reader::new(section.bytes, section.at);
    reader.skip(12).map_err(|_| unread(0))?;
    let count = reader.u32().map_err(|_| unread(0))?;
    let wanted = (count as usize)
        .checked_mul(RECORD)
        .and_then(|bytes| bytes.checked_add(HEADER));
    let Some(wanted) = wanted.filter(|&end| end <= section.bytes.len()) else {
        return Err(unread(count));
    };
    // Shipped breakable lamps append LMP1, the authored count, and its bitset.
    // Maps without lamp state have no suffix. The native lamp loader checks
    // the count against its project; here bound the wire value and its length.
    let suffix = section.bytes.get(wanted..).unwrap_or_default();
    if !suffix.is_empty() {
        let bad = || {
            Refusal::new(
                Part::Bullets,
                section.at.saturating_add(wanted),
                Reason::LampHistory,
            )
        };
        let mut lamps = Reader::new(suffix, 0);
        if lamps.array::<4>().map_err(|_| bad())? != *b"LMP1" {
            return Err(bad());
        }
        let count = lamps.u32().map_err(|_| bad())?;
        if !(1..=256).contains(&count)
            || lamps.rest().len() != (count as usize).div_ceil(32).saturating_mul(4)
        {
            return Err(bad());
        }
    }
    for number in 0..count as usize {
        let at = reader.at();
        let bytes = reader.array::<RECORD>().map_err(|_| unread(count))?;
        let [.., owner, second, slot, _ammo, _material, _quiet] = bytes;
        let reason = if !frame.has_block(owner) {
            Some(Reason::BulletOwner {
                bullet: number,
                owner,
            })
        } else if second != owner {
            Some(Reason::BulletSecond {
                bullet: number,
                second,
                owner,
            })
        } else if !WEAPON_SLOTS.contains(&slot) {
            Some(Reason::BulletSlot {
                bullet: number,
                slot,
            })
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(Refusal::new(Part::Bullets, at, reason));
        }
        for offset in [4, 8, 12, 16, 20, 24, 28, 32, 40, 44, 48, 52, 58] {
            let number_bits = bytes.get(offset..).and_then(<[u8]>::first_chunk::<4>);
            if !number_bits.is_some_and(|value| finite(u32::from_le_bytes(*value))) {
                return Err(Refusal::new(
                    Part::Bullets,
                    at,
                    Reason::BulletNotNumber { bullet: number },
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body(records: &[u8], count: u32) -> std::vec::Vec<u8> {
        let mut body = 1u32.to_le_bytes().to_vec();
        body.extend([2, 0, 0, 0, 0]);
        body.extend(((HEADER + records.len()) as u16).to_le_bytes());
        body.extend([0; 12]);
        body.extend(count.to_le_bytes());
        body.extend(records);
        body.extend([0; 10]);
        body
    }
    fn verdict(records: &[u8], count: u32) -> Result<(), Refusal> {
        bullets(&super::super::frame(&body(records, count), 1).unwrap())
    }
    #[test]
    fn quiet_byte_is_always_present_and_old_ammo_ordinals_are_allowed() {
        let mut record = [0u8; RECORD];
        record[65] = 7;
        record[66] = 255; // native loading clamps this after a gun swap
        assert_eq!(verdict(&record, 1), Ok(()));
        assert!(verdict(&record[..68], 1).is_err());
        let pair = [record, record].concat();
        assert_eq!(verdict(&pair, 2), Ok(()));
        for length in 0..pair.len() {
            assert!(verdict(&pair[..length], 2).is_err());
        }
    }
    #[test]
    fn owner_slot_and_numbers_are_bounded() {
        let mut valid = [0u8; RECORD];
        valid[65] = 7;
        for (offset, value) in [(63, 20), (64, 1), (65, 23)] {
            let mut bad = valid;
            bad[offset] = value;
            assert!(verdict(&bad, 1).is_err());
        }
        valid[4..8].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(verdict(&valid, 1).is_err());
        assert!(verdict(&[], u32::MAX).is_err());
    }

    #[test]
    fn lamp_history_suffix_has_a_bounded_exact_length() {
        for count in [1u32, 32, 33, 256] {
            let mut suffix = b"LMP1".to_vec();
            suffix.extend(count.to_le_bytes());
            suffix.extend(std::vec![0xff; count.div_ceil(32) as usize * 4]);
            assert_eq!(verdict(&suffix, 0), Ok(()));
            for len in 1..suffix.len() {
                assert!(verdict(&suffix[..len], 0).is_err());
            }
            for wrong in [0u32, 257, u32::MAX] {
                suffix[4..8].copy_from_slice(&wrong.to_le_bytes());
                assert!(verdict(&suffix, 0).is_err());
            }
        }
    }
}
