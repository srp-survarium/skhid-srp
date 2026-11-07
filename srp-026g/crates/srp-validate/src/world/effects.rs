//! The effects section (world apply 0x2D2C1A..0x2D2C93,
//! world_validation.md section 8) and the effect record it shares with a
//! player's state ([`Effect`]).
//!
//! The section is three parts (PROVEN, 0x2D2C44 / 0x2D2C54 / 0x2D2C73):
//! `[core+0x19030]` vtable +0x20 (4 bytes, the opening id H),
//! `game_effect_player` 0x2DDDF0 (u32 time, u32 count E, E effects),
//! `[[core+0x19034]+4]` vtable +0x0C (8 bytes, the closing part).
//!
//! An effect (0x2DDE50..0x2DDF5D, its fields stored at 0x2DDFE4..0x2DE017):
//!
//! ```text
//! off  size  field
//!   0  1+4   root emitter: a pointer flag, then an index (0x2E5C30)
//!   5  1     +0x44 (== 1: the emitter's vtable +0x1C is called, 0x2DE025)
//!   6  4     its start time
//!  10  2     the two bytes 0x2DE100 is given
//!  12  4     f32 the emitter's vtable +0x38 is given
//!  16  4     u32 the emitter's vtable +0x38 is given
//!  20  4     +0x1C u32
//!  24  4     +0x20 f32
//!  28  4     +0x24 f32
//!  32  4     +0x34 u32
//!  36  4     +0x38 f32
//!  40  1     +0x3C bool
//!  41  1+4   time calculator: a pointer flag, then an index (0 its own)
//!  46  21    the root emitter's own bytes (its vtable +0x20): an id, 9
//!            bytes, an f32, the id of the next
//! ```
//!
//! A root emitter or a time calculator with its pointer flag set is read as
//! a raw pointer, and an index names an entry of a table with no bound
//! (0xAA2E48, 0xAA2E74): a world could name any address. A calculator other
//! than the effect's own reads bytes of its own (its vtable +8), so the
//! effect is 67 bytes only in the one form every game writes: the root
//! emitter indexed, its own calculator. [`Effect::read`] takes that form
//! and no other.
//!
//! Every effects section of the kept and the live worlds is one of two
//! forms, which [`effects`] holds the section to, field by field (strict:
//! anything else is refused and can be loosened):
//!
//! - no effect, 20 bytes: H, a time, E = 0, the closing part;
//! - one effect, 87 bytes: its root emitter index 0, +0x44 1, the two bytes
//!   0 and 1, the f32 0.1, +0x1C and +0x34 0, +0x3C false, its emitter's
//!   id H, bytes FF and eight 0, f32 0.1, next id H - 1; free: H, the
//!   time, its start time, the u32 at 16, and the three floats (0.05 in
//!   live worlds), which are numbers by [`Finite`].
//!
//! PROVEN for the reader's paths (0x2E5C30, 0x2DDF28..0x2DDF3C); LIKELY
//! that the free fields are no index (they span the whole range across
//! matches; none is read as one in 0x2DDDF0).

use super::Section;
use crate::reader::{Reader, Short};
use crate::{Finite, Part, Reason, Refusal};

/// An effect's length in its one form.
const EFFECT_LEN: usize = 67;

/// An effect as a world saves it, in its one 67-byte form (the module's
/// table), borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect<'a> {
    bytes: &'a [u8; EFFECT_LEN],
}

/// Why bytes are no [`Effect`]: where (in the message) and what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectFault {
    Short(Short),
    /// The root emitter's pointer flag is set: a raw address.
    RawRootEmitter { at: usize, flag: u8 },
    /// The time calculator is not the effect's own (a raw address, or a
    /// table entry that reads more bytes).
    NotOwnCalculator { at: usize, flag: u8, index: u32 },
    /// A float is no number.
    NotNumber { at: usize, field: &'static str },
}

impl core::fmt::Display for EffectFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            EffectFault::Short(short) => {
                write!(f, "an effect cut short at {}", short.at)
            }
            EffectFault::RawRootEmitter { at, flag } => write!(
                f,
                "an effect's root emitter at {at} is a raw address (flag {flag:#04x})"
            ),
            EffectFault::NotOwnCalculator { at, flag, index } => write!(
                f,
                "an effect's time calculator at {at} is not its own (flag {flag:#04x}, index {index})"
            ),
            EffectFault::NotNumber { at, field } => {
                write!(f, "an effect's {field} at {at} is no number")
            }
        }
    }
}

impl EffectFault {
    /// Where in the message it is.
    pub fn at(self) -> usize {
        match self {
            EffectFault::Short(short) => short.at,
            EffectFault::RawRootEmitter { at, .. }
            | EffectFault::NotOwnCalculator { at, .. }
            | EffectFault::NotNumber { at, .. } => at,
        }
    }
}

impl<'a> Effect<'a> {
    /// The record's length.
    pub const LEN: usize = EFFECT_LEN;
    const ROOT_FLAG: usize = 0;
    const ROOT_INDEX: usize = 1;
    const FIELD_44: usize = 5;
    const START_MS: usize = 6;
    const PLAY_ARGS: usize = 10;
    const EMITTER_F32: usize = 12;
    const EMITTER_U32: usize = 16;
    const FIELD_1C: usize = 20;
    const FIELD_20: usize = 24;
    const FIELD_24: usize = 28;
    const FIELD_34: usize = 32;
    const FIELD_38: usize = 36;
    const FIELD_3C: usize = 40;
    const CALCULATOR_FLAG: usize = 41;
    const CALCULATOR_INDEX: usize = 42;
    const EMITTER_ID: usize = 46;
    const EMITTER_STATE: usize = 50;
    const EMITTER_STATE_F32: usize = 59;
    const EMITTER_NEXT_ID: usize = 63;
    /// The floats, by offset and name, which [`Effect::read`] holds to be
    /// numbers.
    const FLOATS: [(usize, &'static str); 5] = [
        (Self::EMITTER_F32, "the emitter's f32"),
        (Self::FIELD_20, "the effect's +0x20"),
        (Self::FIELD_24, "the effect's +0x24"),
        (Self::FIELD_38, "the effect's +0x38"),
        (Self::EMITTER_STATE_F32, "the emitter's state f32"),
    ];

    /// The next effect of `reader`, in its one form, or why not.
    pub fn read(reader: &mut Reader<'a>) -> Result<Self, EffectFault> {
        let at = reader.at();
        let bytes = reader.take(Self::LEN).map_err(EffectFault::Short)?;
        let bytes: &'a [u8; EFFECT_LEN] = bytes.try_into().map_err(|_| {
            EffectFault::Short(Short {
                at,
                wanted: Self::LEN,
            })
        })?;
        let effect = Self { bytes };
        let field = |offset: usize| at.saturating_add(offset);
        let flag = effect.byte(Self::ROOT_FLAG);
        if flag != 0 {
            return Err(EffectFault::RawRootEmitter {
                at: field(Self::ROOT_FLAG),
                flag,
            });
        }
        let (flag, index) = (
            effect.byte(Self::CALCULATOR_FLAG),
            effect.word(Self::CALCULATOR_INDEX),
        );
        if flag != 0 || index != 0 {
            return Err(EffectFault::NotOwnCalculator {
                at: field(Self::CALCULATOR_FLAG),
                flag,
                index,
            });
        }
        for (offset, name) in Self::FLOATS {
            if Finite::of_bits(effect.word(offset)).is_none() {
                return Err(EffectFault::NotNumber {
                    at: field(offset),
                    field: name,
                });
            }
        }
        Ok(effect)
    }

    fn byte(&self, at: usize) -> u8 {
        self.bytes.get(at).copied().unwrap_or(0)
    }

    fn word(&self, at: usize) -> u32 {
        self.bytes
            .get(at..)
            .and_then(<[u8]>::first_chunk::<4>)
            .map_or(0, |word| u32::from_le_bytes(*word))
    }

    /// A float [`Effect::read`] held to be a number.
    fn float(&self, at: usize) -> Finite {
        Finite::of_bits(self.word(at)).unwrap_or(Finite::ZERO)
    }

    /// The root emitter's index (its table entry; the pointer flag is 0).
    pub fn root_emitter(&self) -> u32 {
        self.word(Self::ROOT_INDEX)
    }

    /// The effect's +0x44.
    pub fn field_44(&self) -> u8 {
        self.byte(Self::FIELD_44)
    }

    /// When it started (LIKELY: the section's time in live worlds).
    pub fn start_ms(&self) -> u32 {
        self.word(Self::START_MS)
    }

    /// The two bytes 0x2DE100 is given.
    pub fn play_args(&self) -> [u8; 2] {
        [
            self.byte(Self::PLAY_ARGS),
            self.byte(Self::PLAY_ARGS.saturating_add(1)),
        ]
    }

    /// The f32 the root emitter's vtable +0x38 is given.
    pub fn emitter_f32(&self) -> Finite {
        self.float(Self::EMITTER_F32)
    }

    /// The u32 the root emitter's vtable +0x38 is given.
    pub fn emitter_u32(&self) -> u32 {
        self.word(Self::EMITTER_U32)
    }

    pub fn field_1c(&self) -> u32 {
        self.word(Self::FIELD_1C)
    }

    pub fn field_20(&self) -> Finite {
        self.float(Self::FIELD_20)
    }

    pub fn field_24(&self) -> Finite {
        self.float(Self::FIELD_24)
    }

    pub fn field_34(&self) -> u32 {
        self.word(Self::FIELD_34)
    }

    pub fn field_38(&self) -> Finite {
        self.float(Self::FIELD_38)
    }

    /// The bool at +0x3C (any byte but 0 is true, `setne` at 0x2DDF04).
    pub fn field_3c(&self) -> bool {
        self.byte(Self::FIELD_3C) != 0
    }

    /// The root emitter's id.
    pub fn emitter_id(&self) -> u32 {
        self.word(Self::EMITTER_ID)
    }

    /// The root emitter's nine state bytes.
    pub fn emitter_state(&self) -> [u8; 9] {
        let mut state = [0; 9];
        if let Some(bytes) = self
            .bytes
            .get(Self::EMITTER_STATE..Self::EMITTER_STATE_F32)
        {
            state.copy_from_slice(bytes);
        }
        state
    }

    /// The root emitter's state f32.
    pub fn emitter_state_f32(&self) -> Finite {
        self.float(Self::EMITTER_STATE_F32)
    }

    /// The id of the emitter after the root.
    pub fn emitter_next_id(&self) -> u32 {
        self.word(Self::EMITTER_NEXT_ID)
    }
}

/// The effects section, borrowed from the world: its opening id, its time,
/// its effect if it has one, its closing part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effects<'a> {
    opening: u32,
    time: u32,
    effect: Option<Effect<'a>>,
    closing: &'a [u8; CLOSING_LEN],
}

/// The closing part's length.
const CLOSING_LEN: usize = 8;
/// Where the effect starts in the section.
const EFFECT_AT: usize = 12;

impl<'a> Effects<'a> {
    /// The section, read to its end with at most one effect (the most any
    /// world has), or why not.
    pub fn read(section: &Section<'a>) -> Result<Self, Refusal> {
        let len = section.bytes.len();
        let refuse = |at: usize, reason| Refusal::new(Part::Effects, at, reason);
        let wrong_length = || refuse(section.at, Reason::EffectsLength { len });
        let mut reader = Reader::new(section.bytes, section.at);
        let opening = reader.u32().map_err(|_| wrong_length())?;
        let time = reader.u32().map_err(|_| wrong_length())?;
        let count_at = reader.at();
        let count = reader.u32().map_err(|_| wrong_length())?;
        let effect = match count {
            0 => None,
            1 => Some(Effect::read(&mut reader).map_err(|fault| match fault {
                EffectFault::Short(_) => wrong_length(),
                fault => refuse(fault.at(), Reason::EffectsEffect(fault)),
            })?),
            count => return Err(refuse(count_at, Reason::EffectsCount { count, len })),
        };
        let closing = reader
            .take(CLOSING_LEN)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(wrong_length)?;
        if !reader.is_done() {
            return Err(wrong_length());
        }
        Ok(Self {
            opening,
            time,
            effect,
            closing,
        })
    }

    /// The id the section opens with (H).
    pub fn opening(&self) -> u32 {
        self.opening
    }

    pub fn time(&self) -> u32 {
        self.time
    }

    pub fn effect(&self) -> Option<Effect<'a>> {
        self.effect
    }

    /// The closing part (`[[core+0x19034]+4]` vtable +0x0C).
    pub fn closing(&self) -> [u8; CLOSING_LEN] {
        *self.closing
    }
}

/// The closing part of every world: f32 1.0 and a u32 0.
const CLOSING: [u8; CLOSING_LEN] = [0, 0, 0x80, 0x3F, 0, 0, 0, 0];
/// The f32 0.1 an effect and its emitter save.
const POINT_ONE: u32 = 0x3DCC_CCCD;
/// The emitter's state bytes in every world.
const EMITTER_STATE: [u8; 9] = [0xFF, 0, 0, 0, 0, 0, 0, 0, 0];

/// The effects section is one of the two forms every world has (the
/// module doc).
pub fn effects(section: &Section) -> Result<(), Refusal> {
    let read = Effects::read(section)?;
    let refuse = |offset: usize, reason| {
        Err(Refusal::new(
            Part::Effects,
            section.at.saturating_add(offset),
            reason,
        ))
    };
    let closing_at = match read.effect() {
        None => EFFECT_AT,
        Some(effect) => {
            let of = |offset: usize| EFFECT_AT.saturating_add(offset);
            let opening = read.opening();
            let next = opening.wrapping_sub(1);
            let fields: [(usize, &'static str, u32, u32); 10] = [
                (
                    Effect::ROOT_INDEX,
                    "the root emitter's index",
                    effect.root_emitter(),
                    0,
                ),
                (
                    Effect::FIELD_44,
                    "the effect's +0x44",
                    effect.field_44().into(),
                    1,
                ),
                (
                    Effect::PLAY_ARGS,
                    "the two bytes of 0x2DE100",
                    u16::from_le_bytes(effect.play_args()).into(),
                    0x0100,
                ),
                (
                    Effect::EMITTER_F32,
                    "the effect's 0.1",
                    effect.emitter_f32().bits(),
                    POINT_ONE,
                ),
                (Effect::FIELD_1C, "the effect's +0x1C", effect.field_1c(), 0),
                (Effect::FIELD_34, "the effect's +0x34", effect.field_34(), 0),
                (
                    Effect::FIELD_3C,
                    "the effect's bool",
                    effect.field_3c().into(),
                    0,
                ),
                (
                    Effect::EMITTER_ID,
                    "the emitter's id (the opening id)",
                    effect.emitter_id(),
                    opening,
                ),
                (
                    Effect::EMITTER_STATE_F32,
                    "the emitter's 0.1",
                    effect.emitter_state_f32().bits(),
                    POINT_ONE,
                ),
                (
                    Effect::EMITTER_NEXT_ID,
                    "the next emitter's id (the opening id less 1)",
                    effect.emitter_next_id(),
                    next,
                ),
            ];
            for (offset, field, found, wanted) in fields {
                if found != wanted {
                    let at = of(offset);
                    return refuse(
                        at,
                        Reason::EffectsField {
                            at,
                            field,
                            found,
                            wanted,
                        },
                    );
                }
            }
            if effect.emitter_state() != EMITTER_STATE {
                let at = of(Effect::EMITTER_STATE);
                return refuse(
                    at,
                    Reason::EffectsBytes {
                        at,
                        what: "the emitter's state",
                    },
                );
            }
            of(Effect::LEN)
        }
    };
    if read.closing() != CLOSING {
        return refuse(
            closing_at,
            Reason::EffectsBytes {
                at: closing_at,
                what: "the closing part",
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A live world's one effect (2026-11-06, at 40 600 ms), its three
    /// floats 0.05.
    const LIVE: [u8; 87] = [
        0x5e, 0x93, 0xe5, 0xef, 0x98, 0x9e, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x98, 0x9e, 0, 0,
        0, 1, 0xcd, 0xcc, 0xcc, 0x3d, 0x79, 0x09, 0x3f, 0x12, 0, 0, 0, 0, 0xcd, 0xcc, 0x4c, 0x3d,
        0xcd, 0xcc, 0x4c, 0x3d, 0, 0, 0, 0, 0xcd, 0xcc, 0x4c, 0x3d, 0, 0, 0, 0, 0, 0, 0x5e, 0x93,
        0xe5, 0xef, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0xcd, 0xcc, 0xcc, 0x3d, 0x5d, 0x93, 0xe5, 0xef, 0,
        0, 0x80, 0x3f, 0, 0, 0, 0,
    ];
    /// No effect.
    const NONE: [u8; 20] = [
        1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0x80, 0x3f, 0, 0, 0, 0,
    ];

    fn section(bytes: &[u8]) -> Section<'_> {
        Section { at: 0, bytes }
    }

    fn reason(bytes: &[u8]) -> Option<Reason> {
        effects(&section(bytes)).err().map(|refusal| refusal.reason)
    }

    #[test]
    fn every_field_reads_by_name() {
        let read = Effects::read(&section(&LIVE)).unwrap();
        assert_eq!((read.opening(), read.time()), (0xEFE5_935E, 40_600));
        assert_eq!(read.closing(), CLOSING);
        let effect = read.effect().unwrap();
        assert_eq!(effect.root_emitter(), 0);
        assert_eq!(effect.field_44(), 1);
        assert_eq!(effect.start_ms(), 40_600);
        assert_eq!(effect.play_args(), [0, 1]);
        assert_eq!(effect.emitter_f32().bits(), POINT_ONE);
        assert_eq!(effect.emitter_u32(), 0x123F_0979);
        assert_eq!(effect.field_1c(), 0);
        assert_eq!(effect.field_20().get(), 0.05);
        assert_eq!(effect.field_24().get(), 0.05);
        assert_eq!(effect.field_34(), 0);
        assert_eq!(effect.field_38().get(), 0.05);
        assert!(!effect.field_3c());
        assert_eq!(effect.emitter_id(), 0xEFE5_935E);
        assert_eq!(effect.emitter_state(), EMITTER_STATE);
        assert_eq!(effect.emitter_state_f32().bits(), POINT_ONE);
        assert_eq!(effect.emitter_next_id(), 0xEFE5_935D);
        assert_eq!(reason(&LIVE), None);
        assert_eq!(reason(&NONE), None);
    }

    #[test]
    fn an_effects_floats_take_any_number_and_its_u32s_are_held() {
        let mut nan = LIVE;
        nan[40..44].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(
            reason(&nan),
            Some(Reason::EffectsEffect(EffectFault::NotNumber { at: 40, .. }))
        ));
        let mut set = LIVE;
        set[44] = 1;
        assert!(matches!(
            reason(&set),
            Some(Reason::EffectsField {
                at: 44,
                found: 1,
                wanted: 0,
                ..
            })
        ));
    }

    #[test]
    fn only_the_form_a_game_writes_is_an_effect() {
        let mut raw = LIVE;
        raw[12] = 1;
        assert!(matches!(
            reason(&raw),
            Some(Reason::EffectsEffect(EffectFault::RawRootEmitter {
                at: 12,
                flag: 1
            }))
        ));
        let mut calculator = LIVE;
        calculator[54] = 2;
        assert!(matches!(
            reason(&calculator),
            Some(Reason::EffectsEffect(EffectFault::NotOwnCalculator {
                index: 2,
                ..
            }))
        ));
        let mut two = LIVE;
        two[8] = 2;
        assert!(matches!(
            reason(&two),
            Some(Reason::EffectsCount { count: 2, .. })
        ));
        assert!(matches!(
            reason(&LIVE[..86]),
            Some(Reason::EffectsLength { len: 86 })
        ));
        let mut closing = NONE;
        closing[19] = 1;
        assert!(matches!(
            reason(&closing),
            Some(Reason::EffectsBytes { at: 12, .. })
        ));
    }
}
