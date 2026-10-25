//! The bullets section (`bullet_manager::deserialize` 0x321C50,
//! world_validation.md section 7): a 16-byte header, then N records of 68
//! bytes (`bullet::bullet` 0x32BBC0; writer `bullet::serialize` 0x32BEC0),
//! each with one more byte, the pellet, when its ammo's `buck_shot` is over
//! one. Nothing says where a record ends: the section is read to its exact
//! end.
//!
//! ```text
//!  off size  field
//!   0   4    u32 id
//!   4  24    6 x f32: position, direction
//!  28   8    2 x f32
//!  36   4    u32
//!  40  16    4 x f32
//!  56   2    2 x u8
//!  58   4    f32
//!  62   1    i8
//!  63   1    u8 owner        a place; [owner+0x10C] read at once (0x32BDA1)
//!  64   1    u8 second id    looked up like the owner; null is a crash later
//!  65   1    u8 weapon slot  [[owner+0x10C] + slot*4 + 0x10C], no bound
//!  66   1    u8 ammo index   the weapon's ammo list, no bound; == its length:
//!                            the weapon's melee params (0x2E2750)
//!  67   1    u8 material     0 none, else a table with a default: safe
//!  68   1    u8 pellet       only when the ammo's buck_shot > 1 (0x2E2467)
//! ```

use super::{Context, Frame, Section};
use crate::reader::Reader;
use crate::{Part, Reason, Refusal, finite};

/// The header: the next bullet id, two u32 the reader does not index with,
/// N (0x321C5A..0x321C8E).
const HEADER: usize = 16;
/// A record without its pellet byte.
const RECORD: usize = 68;
/// The most records the section's cap holds (0x2400 bytes): fewer than the
/// manager's pool of 0xC0 bullets (0x322730).
const MAX_RECORDS: usize = (0x2400 - HEADER) / RECORD;

/// The two weapon slots of a kit (table 0x928778): only they get an ammo
/// list.
pub const WEAPON_SLOTS: [u8; 2] = [7, 10];
/// A kit's ammo slots, in the order its weapons' ammo lists take them
/// (table 0x928780).
pub const AMMO_SLOTS: [u8; 8] = [8, 9, 11, 12, 19, 20, 21, 22];

/// The bullets read to their exact end, every record's owner a place with
/// a block in this world and its second id the owner, its slot a weapon
/// slot, its floats numbers. With the profiles, the owner's current kit (its
/// head's `current_index`) has a weapon in the slot, the ammo index is at
/// most that weapon's ammo count, and a record has its pellet byte exactly
/// when that ammo is buckshot.
///
/// Without profiles (a tool reading kept worlds with a guessed match) the
/// weapon, the ammo and the buckshot are not known: the records are framed
/// every way the pellet bytes can fall, and the world passes when one way
/// has every record pass the other checks.
pub fn bullets(frame: &Frame, body: &[u8], context: &Context) -> Result<(), Refusal> {
    let section = frame.bullets();
    let refuse = |offset: usize, reason| Refusal::new(Part::Bullets, offset, reason);
    let mut header = Reader::new(section.bytes, section.at);
    let count = (|| {
        header.skip(HEADER - 4)?;
        header.u32()
    })();
    let len = section.bytes.len();
    let unread = |records: u32| {
        refuse(
            section.at,
            Reason::BulletsUnread {
                len,
                bullets: records,
            },
        )
    };
    let Ok(count) = count else {
        return Err(unread(0));
    };
    let records = count as usize;
    if records > MAX_RECORDS
        || records
            .checked_mul(RECORD)
            .and_then(|bytes| bytes.checked_add(HEADER))
            .is_none_or(|least| least > len)
    {
        return Err(unread(count));
    }
    match context.profiles {
        Some(_) => framed(&section, frame, body, context, records)
            .map_err(|fault| fault.unwrap_or(unread(count))),
        None => guessed(&section, frame, records).map_err(|fault| fault.unwrap_or(unread(count))),
    }
}

/// A record's checks that need no profile: its fields the checks with a
/// profile look at, or the fault.
struct Fields {
    owner: u8,
    slot: u8,
    ammo: u8,
}

fn fields(record: &[u8; RECORD], number: usize, frame: &Frame) -> Result<Fields, Reason> {
    let [.., owner, second, slot, ammo, _material] = *record;
    if !frame.has_block(owner) {
        return Err(Reason::BulletOwner {
            bullet: number,
            owner,
        });
    }
    if second != owner {
        return Err(Reason::BulletSecond {
            bullet: number,
            second,
            owner,
        });
    }
    if !WEAPON_SLOTS.contains(&slot) {
        return Err(Reason::BulletSlot {
            bullet: number,
            slot,
        });
    }
    // The f32s: 4..28, 28, 32, 40..56, 58 (36 is a u32, 56 and 57 bytes).
    let numbers = [4, 8, 12, 16, 20, 24, 28, 32, 40, 44, 48, 52, 58]
        .into_iter()
        .all(|at| {
            record
                .get(at..)
                .and_then(<[u8]>::first_chunk::<4>)
                .is_some_and(|bytes| finite(u32::from_le_bytes(*bytes)))
        });
    if !numbers {
        return Err(Reason::BulletNotNumber { bullet: number });
    }
    Ok(Fields { owner, slot, ammo })
}

/// The record at `at` in the section (an offset of the section's own).
fn record_at<'a>(section: &Section<'a>, at: usize) -> Option<&'a [u8; RECORD]> {
    section
        .bytes
        .get(at..)
        .and_then(|rest| rest.first_chunk::<RECORD>())
}

/// With the profiles: each record's length is known from its ammo.
fn framed(
    section: &Section,
    frame: &Frame,
    body: &[u8],
    context: &Context,
    records: usize,
) -> Result<(), Option<Refusal>> {
    let profiles = context.profiles.unwrap_or_default();
    let mut at = HEADER;
    for number in 0..records {
        let record = record_at(section, at).ok_or(None)?;
        let offset = section.at.saturating_add(at);
        let refuse = |reason| Some(Refusal::new(Part::Bullets, offset, reason));
        let fields = fields(record, number, frame).map_err(refuse)?;
        // The owner's current kit: its block's first byte, which the head's
        // check found usable.
        let kit = super::Blocks::of(body)
            .find(|block| block.place == fields.owner)
            .and_then(|block| block.state.first().copied())
            .unwrap_or(0);
        let weapon = profiles
            .get(usize::from(fields.owner))
            .and_then(|profile| profile.weapons.get(usize::from(kit)))
            .and_then(|weapons| {
                let index = WEAPON_SLOTS.iter().position(|slot| *slot == fields.slot)?;
                weapons.get(index)
            })
            .filter(|weapon| weapon.present);
        let Some(weapon) = weapon else {
            return Err(refuse(Reason::BulletNoWeapon {
                bullet: number,
                slot: fields.slot,
                kit,
            }));
        };
        if fields.ammo > weapon.ammo {
            return Err(refuse(Reason::BulletAmmo {
                bullet: number,
                ammo: fields.ammo,
                count: weapon.ammo,
            }));
        }
        let pellet = usize::from(weapon.buckshot(fields.ammo));
        at = at.saturating_add(RECORD).saturating_add(pellet);
    }
    if at == section.bytes.len() {
        Ok(())
    } else {
        Err(None)
    }
}

/// The pellet bytes so far a framing may have read: a set of 0..192.
#[derive(Clone, Copy, Default)]
struct Reach([u64; 3]);

impl Reach {
    fn has(&self, pellets: usize) -> bool {
        self.0
            .get(pellets / 64)
            .is_some_and(|word| word.checked_shr((pellets % 64) as u32).unwrap_or(0) & 1 != 0)
    }

    fn add(&mut self, pellets: usize) {
        if let Some(word) = self.0.get_mut(pellets / 64) {
            *word |= 1u64.checked_shl((pellets % 64) as u32).unwrap_or(0);
        }
    }

    fn is_empty(&self) -> bool {
        self.0 == [0; 3]
    }
}

/// Without profiles: every way the pellet bytes can fall, record by
/// record; the fault reported is that of the record that no way gets past,
/// read with the fewest pellets before it.
fn guessed(section: &Section, frame: &Frame, records: usize) -> Result<(), Option<Refusal>> {
    let mut reach = Reach::default();
    reach.add(0);
    for number in 0..records {
        let start = number.saturating_mul(RECORD).saturating_add(HEADER);
        let mut next = Reach::default();
        let mut fault = None;
        for pellets in 0..=number {
            if !reach.has(pellets) {
                continue;
            }
            let at = start.saturating_add(pellets);
            let Some(record) = record_at(section, at) else {
                continue;
            };
            match fields(record, number, frame) {
                Ok(_) => {
                    next.add(pellets);
                    next.add(pellets.saturating_add(1));
                }
                Err(reason) => {
                    if fault.is_none() {
                        fault = Some(Refusal::new(
                            Part::Bullets,
                            section.at.saturating_add(at),
                            reason,
                        ));
                    }
                }
            }
        }
        if next.is_empty() {
            return Err(fault);
        }
        reach = next;
    }
    let end = records.saturating_mul(RECORD).saturating_add(HEADER);
    let pellets = section.bytes.len().checked_sub(end).ok_or(None)?;
    if reach.has(pellets) {
        Ok(())
    } else {
        Err(None)
    }
}
