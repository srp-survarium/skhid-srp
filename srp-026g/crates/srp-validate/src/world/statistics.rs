//! The statistics section (`shared_statistics::serialize` 0x2F5AC0,
//! snapshot_rules.md section 3): a record for each roster place, one for
//! each battery in modes 2 and 3, the 11-byte tail. The server's own
//! reader of it, for the results, is `snapshot::Statistics`; this one only
//! walks it and looks at what a game trusts.

use super::{Context, Section, battery_records};
use crate::reader::{Reader, Short};
use crate::{KITS, MAX_PLAYERS, Part, Reason, Refusal, finite};

/// The score events a record's masks cover (`match_stats_events_dict_enum`).
const EVENTS: u32 = 28;
/// The events carried as u16 (table 0x9205E0: 6, 17, 18, 22, 23, 26); the
/// others are one byte (table 0x920588).
const WORD_EVENTS: u32 = 1 << 6 | 1 << 17 | 1 << 18 | 1 << 22 | 1 << 23 | 1 << 26;
/// The tail: the team points, the batteries, the winner, a mask
/// (0x2F5B08..0x2F5BCB).
const TAIL: usize = 11;

/// The statistics read to their exact length: one record for each place,
/// one for each battery in modes 2 and 3, the tail. Every damage value
/// and the newbie bonus is a number, and `last_hit_enemy_id` a place of the
/// roster or 0xFF.
pub fn statistics(section: &Section, context: &Context) -> Result<(), Refusal> {
    let refuse = |reason| Refusal::new(Part::Statistics, section.at, reason);
    if section.bytes.is_empty() {
        return Err(refuse(Reason::StatisticsEmpty));
    }
    let places = context.places;
    let batteries = battery_records(context);
    let unread = refuse(Reason::StatisticsUnread {
        len: section.bytes.len(),
        places,
        batteries,
    });
    if places > MAX_PLAYERS {
        return Err(unread);
    }
    let mut reader = Reader::new(section.bytes, section.at);
    // The first fault of a record; given only when all of it reads.
    let mut fault = None;
    for place in 0..places {
        let record = player(&mut reader).map_err(|_| unread)?;
        if fault.is_some() {
            continue;
        }
        if !record.numbers {
            fault = Some(Reason::DamageNotNumber { place });
        } else if usize::from(record.last_hit_enemy) >= places && record.last_hit_enemy != 0xFF {
            fault = Some(Reason::LastHitPast {
                place,
                enemy: record.last_hit_enemy,
                places,
            });
        }
    }
    for _ in 0..batteries {
        battery(&mut reader, places).map_err(|_| unread)?;
    }
    reader.skip(TAIL).map_err(|_| unread)?;
    if !reader.is_done() {
        return Err(unread);
    }
    match fault {
        Some(reason) => Err(refuse(reason)),
        None => Ok(()),
    }
}

/// What a place's record holds that a check looks at.
struct Record {
    /// Every damage value and the newbie bonus is a number.
    numbers: bool,
    last_hit_enemy: u8,
}

/// The read failed, or a mask names more than its values.
struct Unread;

impl From<Short> for Unread {
    fn from(_: Short) -> Self {
        Unread
    }
}

/// `player_shared_statistics::serialize` 0x335750.
fn player(reader: &mut Reader) -> Result<Record, Unread> {
    // Each kit's events: a mask, then a count for each set bit.
    for _ in 0..KITS {
        let mask = reader.u32()?;
        if mask.checked_shr(EVENTS).unwrap_or(0) != 0 {
            return Err(Unread);
        }
        let bytes = mask
            .count_ones()
            .saturating_add((mask & WORD_EVENTS).count_ones());
        reader.skip(bytes as usize)?;
    }
    // `received_damage[kit]`, by inflictor.
    let mut numbers = true;
    for _ in 0..KITS {
        numbers &= masked_f32(reader, MAX_PLAYERS)?;
    }
    // `last_hit_ms`, by receiver.
    let mask = reader.u32()?;
    if mask.checked_shr(MAX_PLAYERS as u32).unwrap_or(0) != 0 {
        return Err(Unread);
    }
    reader.skip((mask.count_ones() as usize).saturating_mul(4))?;
    let _last_killed_enemy_ms = reader.u32()?;
    let last_hit_enemy = reader.u8()?;
    let _killstreak = reader.u8()?;
    numbers &= finite(reader.f32_bits()?);
    let _longest_killstreak = reader.u16()?;
    Ok(Record {
        numbers,
        last_hit_enemy,
    })
}

/// A battery's status (0x334DC0): the distances carried by kit and
/// player, two floats, the owner team, the first finder.
fn battery(reader: &mut Reader, places: usize) -> Result<(), Unread> {
    for _ in 0..KITS {
        masked_f32(reader, places)?;
    }
    reader.skip(4 + 4 + 1 + 1)?;
    Ok(())
}

/// A u32 mask over the first `count` values, then the value of each set
/// bit. Whether every value is a number.
fn masked_f32(reader: &mut Reader, count: usize) -> Result<bool, Unread> {
    let mask = reader.u32()?;
    let count = count.min(MAX_PLAYERS) as u32;
    if mask.checked_shr(count).unwrap_or(0) != 0 {
        return Err(Unread);
    }
    let mut numbers = true;
    for _ in 0..mask.count_ones() {
        numbers &= finite(reader.f32_bits()?);
    }
    Ok(numbers)
}
