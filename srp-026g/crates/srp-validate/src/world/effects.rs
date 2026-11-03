//! The effects section (world apply 0x2D2C1A..0x2D2C93,
//! world_validation.md section 8), held to a template.
//!
//! The section is three parts (PROVEN, 0x2D2C44 / 0x2D2C54 / 0x2D2C73):
//! `[core+0x19030]` vtable +0x20 (4 bytes, H), `game_effect_player`
//! 0x2DDDF0 (u32 time, u32 count E, E effects), `[[core+0x19034]+4]` vtable
//! +0x0C (8 bytes). An effect's root emitter and its time calculator are
//! each a raw pointer when a flag byte says so (0x2E5C30; writer
//! 0x2EFD00 for the calculator), else an index into a table with no bound
//! (0xAA2E48, 0xAA2E74). So a world can name any address.
//!
//! Every effects section of 22,486 worlds (the live sample and the kept
//! ones) is one of two byte forms; the check allows those only (strict:
//! anything else is refused and can be loosened):
//!
//! ```text
//! E = 0, 20 bytes:  H | time | 00 00 00 00 | 00 00 80 3F 00 00 00 00
//! E = 1, 87 bytes:  as TEMPLATE_ONE; free: H (0), time (4), t (18), s (28);
//!                   off 58 is H again, off 75 is H - 1
//! ```
//!
//! With the template the reader takes the path of the worlds every game
//! loaded: the root emitter indexed (index 0), the effect's own calculator,
//! the same emitter's bytes. PROVEN for the paths (0x2E5C30,
//! 0x2DDF28..0x2DDF3C); LIKELY that the free fields are no index (they span
//! the whole range across matches).

use super::Section;
use crate::{Part, Reason, Refusal};

/// A part of the template: bytes it must have, or the u32 H (offset 0)
/// less `less`.
enum Field {
    Bytes(&'static [u8], &'static str),
    Opening { less: u32 },
}

/// No effect: 20 bytes. Fields by offset; the others are free.
const TEMPLATE_NONE: [(usize, Field); 2] = [
    (8, Field::Bytes(&[0, 0, 0, 0], "the effect count")),
    (
        12,
        Field::Bytes(&[0, 0, 0x80, 0x3F, 0, 0, 0, 0], "the closing part"),
    ),
];

/// One effect: 87 bytes.
const TEMPLATE_ONE: [(usize, Field); 15] = [
    (8, Field::Bytes(&[1, 0, 0, 0], "the effect count")),
    (12, Field::Bytes(&[0], "the root emitter's pointer flag")),
    (13, Field::Bytes(&[0, 0, 0, 0], "the root emitter's index")),
    (17, Field::Bytes(&[1], "the effect's +0x44")),
    (22, Field::Bytes(&[0, 1], "the two bytes of 0x2DE100")),
    (
        24,
        Field::Bytes(&[0xCD, 0xCC, 0xCC, 0x3D], "the effect's 0.1"),
    ),
    (32, Field::Bytes(&[0; 20], "the effect's other five values")),
    (52, Field::Bytes(&[0], "the effect's bool")),
    (53, Field::Bytes(&[0], "the time calculator's pointer flag")),
    (54, Field::Bytes(&[0, 0, 0, 0], "the time calculator")),
    (58, Field::Opening { less: 0 }),
    (
        62,
        Field::Bytes(&[0xFF, 0, 0, 0, 0, 0, 0, 0, 0], "the emitter's bytes"),
    ),
    (
        71,
        Field::Bytes(&[0xCD, 0xCC, 0xCC, 0x3D], "the emitter's 0.1"),
    ),
    (75, Field::Opening { less: 1 }),
    (
        79,
        Field::Bytes(&[0, 0, 0x80, 0x3F, 0, 0, 0, 0], "the closing part"),
    ),
];

/// The two lengths the template has.
const NONE_LEN: usize = 20;
const ONE_LEN: usize = 87;

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..)
        .and_then(<[u8]>::first_chunk::<4>)
        .map(|bytes| u32::from_le_bytes(*bytes))
}

/// The effects section is one of the template's two forms.
pub fn effects(section: &Section) -> Result<(), Refusal> {
    let bytes = section.bytes;
    let refuse =
        |at: usize, reason| Refusal::new(Part::Effects, section.at.saturating_add(at), reason);
    let (template, effects): (&[(usize, Field)], u32) = match bytes.len() {
        NONE_LEN => (&TEMPLATE_NONE, 0),
        ONE_LEN => (&TEMPLATE_ONE, 1),
        len => return Err(refuse(0, Reason::EffectsLength { len })),
    };
    let count = u32_at(bytes, 8).unwrap_or(0);
    if count != effects {
        return Err(refuse(
            8,
            Reason::EffectsCount {
                count,
                len: bytes.len(),
            },
        ));
    }
    let opening = u32_at(bytes, 0).unwrap_or(0);
    for (at, field) in template {
        match *field {
            Field::Bytes(wanted, what) => {
                let found = bytes.get(*at..).unwrap_or_default();
                if let Some((index, (&found, &wanted))) = found
                    .iter()
                    .zip(wanted)
                    .enumerate()
                    .find(|(_, (found, wanted))| found != wanted)
                {
                    let byte = at.saturating_add(index);
                    return Err(refuse(
                        byte,
                        Reason::EffectsByte {
                            at: byte,
                            what,
                            found,
                            wanted,
                        },
                    ));
                }
            }
            Field::Opening { less } => {
                let wanted = opening.wrapping_sub(less);
                let found = u32_at(bytes, *at).unwrap_or(0);
                if found != wanted {
                    return Err(refuse(
                        *at,
                        Reason::EffectsEmitterId {
                            at: *at,
                            found,
                            wanted,
                        },
                    ));
                }
            }
        }
    }
    Ok(())
}
