//! The effects section (world apply 0x2D2C1A..0x2D2C93,
//! world_validation.md section 8) and the effect record it shares with a
//! player's state ([`Effect`]).
//!
//! The section is three parts (PROVEN, 0x2D2C44 / 0x2D2C54 / 0x2D2C73):
//! `[core+0x19030]` vtable +0x20 (4 bytes, the opening id H),
//! `game_effect_player` 0x2DDDF0 (u32 time, u32 count E, E effects),
//! `[[core+0x19034]+4]` vtable +0x0C (8 bytes, the closing part).
//!
//! An effect is a `game_effect_node` as `game_effect_player::serialize`
//! 0x2DDAD0 writes it (read back at 0x2DDE50..0x2DDF5D, stored at
//! 0x2DDFE4..0x2DE017). The writer passes each field's expression as text
//! (`.\game_effect_player.cpp`, 0xA10C4C..0xA10DD4), so these names are
//! the game's own (PROVEN):
//!
//! ```text
//! off  size  field                                  node
//!   0  1+4   node->effect: a pointer flag, then
//!            the root emitter's index (0x2E5BB0 / 0x2E5C30)
//!   5  1     u8( node->activation_status )          +0x44
//!   6  4     node->state.start_time_in_ms           +0x18
//!  10  1     u8( node->playback_type )              +0x28
//!  11  1     u8( node->serialization_mode )         +0x2C
//!  12  4     node->effect->length()                 f32
//!  16  4     node->effect->creation_seed()          u32
//!  20  4     node->state.current_interval_id        +0x1C
//!  24  4     node->state.current_interval_time      +0x20 f32
//!  28  4     node->state.current_time               +0x24 f32
//!  32  4     node->previous_time.interval_id        +0x34
//!  36  4     node->previous_time.interval_time      +0x38 f32
//!  40  1     node->previous_time.effect_ended       +0x3C bool
//!  41  1+4   node->time_calculator: a pointer flag,
//!            then an index (0x2EFD00; 0 its own)    +0x30
//!  46  4     the root's id                          (unknown)
//!  50  1     its child 0 is there                   bool
//!  51  4     m_effects[0]->creation_seed()          u32
//!  55  4     m_effects[0]->start_time()             f32
//!  59  4     m_effects[0]->length()                 f32
//!  63  4     m_seeds[0]                             u32
//! ```
//!
//! Bytes 46..67 are the root effect's own: the u32 the effect's `+0x10`
//! object writes (its vtable +0x1C, 0x2DDD7B; what it is was not traced:
//! the section's opening id H in every world), then the effect's own
//! serialize (vtable +0x24, 0x2DDD86), which these bytes fit as
//! `composite_game_effect::serialize` 0x334310 with one child whose own
//! serialize writes nothing: the child's presence (a bool of 1), its
//! creation seed, start time and length, then `m_seeds[0]` (its strings
//! 0xA19CB4..0xA19D14). That it is a composite with one such child is
//! LIKELY (the bytes fit it; the root's class was not traced).
//!
//! The domains, each read once by [`Effect::read`]:
//!
//! - `activation_status`: 0 (the node's constructor 0x2DD7D8), 1 (activated,
//!   0x2DE0D9), 2 (stopped, 0x2DDA09, 0x2DE37D) (PROVEN: the only stores);
//! - `playback_type`: 0 `remove_at_end` or 1 `manual_remove` (0.21d's
//!   `game_effect::playback_enum`, LIKELY the same in 0.26g);
//! - `serialization_mode`: 1 or 2, the only modes the writer writes (a node
//!   of mode 1 goes to the world's writer, of mode 2 to the client's,
//!   0x2DDB83..0x2DDB9D; others are skipped) (PROVEN);
//! - `effect_ended` and the child's presence: 0x00 or 0xFF, the bool
//!   writer's (0x2DDD5E, 0x3343A3; any byte but 0 reads as true, `setne`
//!   0x2DDF04) (PROVEN); the child is there (a 0 would end the root's bytes
//!   at 51: another form);
//! - the root emitter indexed (its flag 0) and the effect's own time
//!   calculator (flag and index 0): a flag set is read as a raw pointer,
//!   and an index names an entry of a table with no bound (0xAA2E48,
//!   0xAA2E74), and a calculator other than the effect's own reads bytes of
//!   its own (its vtable +8), so the effect is 67 bytes only in this form
//!   (PROVEN for the reader's paths, 0x2E5C30, 0x2DDF28..0x2DDF3C);
//! - every float a number ([`Finite`]).
//!
//! Open in a player's effect: the root emitter's index and
//! `current_interval_id` (an index into the root's intervals, whose count
//! is not in the context; the section holds both to 0), the times, seeds
//! and ids.
//!
//! Every effects section of the kept and the live worlds is one of two
//! forms, which [`effects`] holds the section to, field by field (strict:
//! anything else is refused and can be loosened):
//!
//! - no effect, 20 bytes: H, a time, E = 0, the closing part;
//! - one effect, 87 bytes: its root emitter index 0, activation 1 (active),
//!   playback 0 and serialization mode 1, length 0.1, both interval ids 0,
//!   not ended, its root's id H, the child there with seed 0, start 0 and
//!   length 0.1, `m_seeds[0]` H - 1; free: H, the time, its start time, its
//!   creation seed, and the three interval times (0.05 in live worlds).

use super::Section;
use crate::reader::{Reader, Short};
use crate::{Finite, Part, Reason, Refusal, wire_bool};

/// An effect's length in its one form.
const EFFECT_LEN: usize = 67;

/// `node->activation_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationStatus {
    /// 0: made, not activated yet.
    Created,
    /// 1.
    Active,
    /// 2.
    Stopped,
}

/// `node->playback_type` (`game_effect::playback_enum`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackType {
    RemoveAtEnd,
    ManualRemove,
}

/// `node->serialization_mode`: which writer saves the node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerializationMode {
    /// 1: the world's (`serializable_effects_count`).
    World,
    /// 2: the client's (`client_serializable_effects_count`).
    Client,
}

/// An effect as a world saves it, in its one 67-byte form (the module's
/// table), every field read into its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    activation_status: ActivationStatus,
    start_time_ms: u32,
    playback_type: PlaybackType,
    serialization_mode: SerializationMode,
    length: Finite,
    creation_seed: u32,
    current_interval_id: u32,
    current_interval_time: Finite,
    current_time: Finite,
    previous_interval_id: u32,
    previous_interval_time: Finite,
    effect_ended: bool,
    root_id: u32,
    child_creation_seed: u32,
    child_start_time: Finite,
    child_length: Finite,
    child_seed: u32,
}

/// Why bytes are no [`Effect`]: where (in the message) and what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectFault {
    Short(Short),
    /// The root emitter's pointer flag is set: a raw address.
    RawRootEmitter {
        at: usize,
        flag: u8,
    },
    /// The root emitter's index is not 0: an entry of a table with no
    /// bound (0xAA2E48), and every world has 0.
    RootIndex {
        at: usize,
        index: u32,
    },
    /// The time calculator is not the effect's own (a raw address, or a
    /// table entry that reads more bytes).
    NotOwnCalculator {
        at: usize,
        flag: u8,
        index: u32,
    },
    /// A float is no number.
    NotNumber {
        at: usize,
        field: &'static str,
    },
    /// A byte that is none of its field's values.
    Domain {
        at: usize,
        field: &'static str,
        byte: u8,
    },
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
            EffectFault::RootIndex { at, index } => write!(
                f,
                "an effect's root emitter index at {at} is {index}, not 0 (a table with no bound)"
            ),
            EffectFault::NotOwnCalculator { at, flag, index } => write!(
                f,
                "an effect's time calculator at {at} is not its own (flag {flag:#04x}, index {index})"
            ),
            EffectFault::NotNumber { at, field } => {
                write!(f, "an effect's {field} at {at} is no number")
            }
            EffectFault::Domain { at, field, byte } => {
                write!(
                    f,
                    "an effect's {field} at {at} is {byte:#04x}, none of its values"
                )
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
            | EffectFault::RootIndex { at, .. }
            | EffectFault::NotOwnCalculator { at, .. }
            | EffectFault::NotNumber { at, .. }
            | EffectFault::Domain { at, .. } => at,
        }
    }
}

impl Effect {
    /// The record's length.
    pub const LEN: usize = EFFECT_LEN;
    const ROOT_INDEX: usize = 1;
    const ACTIVATION_STATUS: usize = 5;
    const PLAYBACK_TYPE: usize = 10;
    const SERIALIZATION_MODE: usize = 11;
    const LENGTH: usize = 12;
    const CURRENT_INTERVAL_ID: usize = 20;
    const CURRENT_INTERVAL_TIME: usize = 24;
    const CURRENT_TIME: usize = 28;
    const PREVIOUS_INTERVAL_ID: usize = 32;
    const PREVIOUS_INTERVAL_TIME: usize = 36;
    const EFFECT_ENDED: usize = 40;
    const CALCULATOR_FLAG: usize = 41;
    const ROOT_ID: usize = 46;
    const CHILD_PRESENT: usize = 50;
    const CHILD_CREATION_SEED: usize = 51;
    const CHILD_START_TIME: usize = 55;
    const CHILD_LENGTH: usize = 59;
    const CHILD_SEED: usize = 63;

    /// The next effect of `reader`, in its one form with every field in
    /// its domain, or why not.
    pub fn read(reader: &mut Reader<'_>) -> Result<Self, EffectFault> {
        let at = reader.at();
        let bytes = reader.take(Self::LEN).map_err(EffectFault::Short)?;
        let field = |offset: usize| at.saturating_add(offset);
        // The 67 bytes are there: every offset below is in them.
        let byte = |offset: usize| bytes.get(offset).copied().unwrap_or(0);
        let word = |offset: usize| {
            bytes
                .get(offset..)
                .and_then(<[u8]>::first_chunk::<4>)
                .map_or(0, |word| u32::from_le_bytes(*word))
        };
        let flag = byte(0);
        if flag != 0 {
            return Err(EffectFault::RawRootEmitter { at: field(0), flag });
        }
        let index = word(Self::ROOT_INDEX);
        if index != 0 {
            return Err(EffectFault::RootIndex {
                at: field(Self::ROOT_INDEX),
                index,
            });
        }
        let (flag, index) = (byte(Self::CALCULATOR_FLAG), word(Self::CALCULATOR_FLAG + 1));
        if flag != 0 || index != 0 {
            return Err(EffectFault::NotOwnCalculator {
                at: field(Self::CALCULATOR_FLAG),
                flag,
                index,
            });
        }
        let domain = |offset: usize, name: &'static str| EffectFault::Domain {
            at: field(offset),
            field: name,
            byte: byte(offset),
        };
        let number = |offset: usize, name: &'static str| {
            Finite::of_bits(word(offset)).ok_or(EffectFault::NotNumber {
                at: field(offset),
                field: name,
            })
        };
        let activation_status = match byte(Self::ACTIVATION_STATUS) {
            0 => ActivationStatus::Created,
            1 => ActivationStatus::Active,
            2 => ActivationStatus::Stopped,
            _ => return Err(domain(Self::ACTIVATION_STATUS, "activation_status")),
        };
        let playback_type = match byte(Self::PLAYBACK_TYPE) {
            0 => PlaybackType::RemoveAtEnd,
            1 => PlaybackType::ManualRemove,
            _ => return Err(domain(Self::PLAYBACK_TYPE, "playback_type")),
        };
        let serialization_mode = match byte(Self::SERIALIZATION_MODE) {
            1 => SerializationMode::World,
            2 => SerializationMode::Client,
            _ => return Err(domain(Self::SERIALIZATION_MODE, "serialization_mode")),
        };
        let effect_ended = wire_bool(byte(Self::EFFECT_ENDED))
            .ok_or_else(|| domain(Self::EFFECT_ENDED, "previous_time.effect_ended"))?;
        if wire_bool(byte(Self::CHILD_PRESENT)) != Some(true) {
            return Err(domain(Self::CHILD_PRESENT, "the root's child"));
        }
        Ok(Self {
            activation_status,
            start_time_ms: word(6),
            playback_type,
            serialization_mode,
            length: number(Self::LENGTH, "length()")?,
            creation_seed: word(16),
            current_interval_id: word(Self::CURRENT_INTERVAL_ID),
            current_interval_time: number(Self::CURRENT_INTERVAL_TIME, "current_interval_time")?,
            current_time: number(Self::CURRENT_TIME, "current_time")?,
            previous_interval_id: word(Self::PREVIOUS_INTERVAL_ID),
            previous_interval_time: number(
                Self::PREVIOUS_INTERVAL_TIME,
                "previous_time.interval_time",
            )?,
            effect_ended,
            root_id: word(Self::ROOT_ID),
            child_creation_seed: word(Self::CHILD_CREATION_SEED),
            child_start_time: number(Self::CHILD_START_TIME, "the child's start_time()")?,
            child_length: number(Self::CHILD_LENGTH, "the child's length()")?,
            child_seed: word(Self::CHILD_SEED),
        })
    }

    /// `node->activation_status`.
    pub fn activation_status(&self) -> ActivationStatus {
        self.activation_status
    }

    /// `node->state.start_time_in_ms` (LIKELY: the section's time in live
    /// worlds).
    pub fn start_time_ms(&self) -> u32 {
        self.start_time_ms
    }

    pub fn playback_type(&self) -> PlaybackType {
        self.playback_type
    }

    pub fn serialization_mode(&self) -> SerializationMode {
        self.serialization_mode
    }

    /// `node->effect->length()`.
    pub fn length(&self) -> Finite {
        self.length
    }

    /// `node->effect->creation_seed()`.
    pub fn creation_seed(&self) -> u32 {
        self.creation_seed
    }

    /// `node->state.current_interval_id`: an index into the root's
    /// intervals.
    pub fn current_interval_id(&self) -> u32 {
        self.current_interval_id
    }

    pub fn current_interval_time(&self) -> Finite {
        self.current_interval_time
    }

    pub fn current_time(&self) -> Finite {
        self.current_time
    }

    /// `node->previous_time.interval_id`.
    pub fn previous_interval_id(&self) -> u32 {
        self.previous_interval_id
    }

    /// `node->previous_time.interval_time`.
    pub fn previous_interval_time(&self) -> Finite {
        self.previous_interval_time
    }

    /// `node->previous_time.effect_ended`.
    pub fn effect_ended(&self) -> bool {
        self.effect_ended
    }

    /// The root's id: the u32 its `+0x10` object writes (its meaning is
    /// not traced; the section's opening id in every world).
    pub fn root_id(&self) -> u32 {
        self.root_id
    }

    /// `m_effects[0]->creation_seed()` of the root.
    pub fn child_creation_seed(&self) -> u32 {
        self.child_creation_seed
    }

    /// `m_effects[0]->start_time()`.
    pub fn child_start_time(&self) -> Finite {
        self.child_start_time
    }

    /// `m_effects[0]->length()`.
    pub fn child_length(&self) -> Finite {
        self.child_length
    }

    /// `m_seeds[0]` of the root.
    pub fn child_seed(&self) -> u32 {
        self.child_seed
    }
}

/// The effects section: its opening id, its time, its effect if it has
/// one, its closing part (borrowed from the world).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effects<'a> {
    opening: u32,
    time: u32,
    effect: Option<Effect>,
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

    /// `game_effect_player`'s `m_current_time_in_ms`.
    pub fn time(&self) -> u32 {
        self.time
    }

    pub fn effect(&self) -> Option<Effect> {
        self.effect
    }

    /// The closing part (`[[core+0x19034]+4]` vtable +0x0C).
    pub fn closing(&self) -> [u8; CLOSING_LEN] {
        *self.closing
    }
}

/// The closing part of every world: f32 1.0 and a u32 0.
const CLOSING: [u8; CLOSING_LEN] = [0, 0, 0x80, 0x3F, 0, 0, 0, 0];
/// The f32 0.1 an effect and its root's child save as their length.
const POINT_ONE: u32 = 0x3DCC_CCCD;

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
            let fields: [(usize, &'static str, u32, u32); 12] = [
                (
                    Effect::ACTIVATION_STATUS,
                    "activation_status (active)",
                    (effect.activation_status() == ActivationStatus::Active).into(),
                    1,
                ),
                (
                    Effect::PLAYBACK_TYPE,
                    "playback_type (remove_at_end)",
                    (effect.playback_type() == PlaybackType::RemoveAtEnd).into(),
                    1,
                ),
                (
                    Effect::SERIALIZATION_MODE,
                    "serialization_mode (the world's)",
                    (effect.serialization_mode() == SerializationMode::World).into(),
                    1,
                ),
                (
                    Effect::LENGTH,
                    "length() 0.1",
                    effect.length().bits(),
                    POINT_ONE,
                ),
                (
                    Effect::CURRENT_INTERVAL_ID,
                    "current_interval_id",
                    effect.current_interval_id(),
                    0,
                ),
                (
                    Effect::PREVIOUS_INTERVAL_ID,
                    "previous_time.interval_id",
                    effect.previous_interval_id(),
                    0,
                ),
                (
                    Effect::EFFECT_ENDED,
                    "previous_time.effect_ended",
                    effect.effect_ended().into(),
                    0,
                ),
                (
                    Effect::ROOT_ID,
                    "the root's id (the opening id)",
                    effect.root_id(),
                    opening,
                ),
                (
                    Effect::CHILD_CREATION_SEED,
                    "the child's creation_seed()",
                    effect.child_creation_seed(),
                    0,
                ),
                (
                    Effect::CHILD_START_TIME,
                    "the child's start_time()",
                    effect.child_start_time().bits(),
                    0,
                ),
                (
                    Effect::CHILD_LENGTH,
                    "the child's length() 0.1",
                    effect.child_length().bits(),
                    POINT_ONE,
                ),
                (
                    Effect::CHILD_SEED,
                    "m_seeds[0] (the opening id less 1)",
                    effect.child_seed(),
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
        0xe5, 0xef, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0xcd, 0xcc, 0xcc, 0x3d, 0x5d, 0x93, 0xe5, 0xef,
        0, 0, 0x80, 0x3f, 0, 0, 0, 0,
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
        assert_eq!(effect.activation_status(), ActivationStatus::Active);
        assert_eq!(effect.start_time_ms(), 40_600);
        assert_eq!(effect.playback_type(), PlaybackType::RemoveAtEnd);
        assert_eq!(effect.serialization_mode(), SerializationMode::World);
        assert_eq!(effect.length().bits(), POINT_ONE);
        assert_eq!(effect.creation_seed(), 0x123F_0979);
        assert_eq!(effect.current_interval_id(), 0);
        assert_eq!(effect.current_interval_time().get(), 0.05);
        assert_eq!(effect.current_time().get(), 0.05);
        assert_eq!(effect.previous_interval_id(), 0);
        assert_eq!(effect.previous_interval_time().get(), 0.05);
        assert!(!effect.effect_ended());
        assert_eq!(effect.root_id(), 0xEFE5_935E);
        assert_eq!(effect.child_creation_seed(), 0);
        assert_eq!(effect.child_start_time(), Finite::ZERO);
        assert_eq!(effect.child_length().bits(), POINT_ONE);
        assert_eq!(effect.child_seed(), 0xEFE5_935D);
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
    fn a_byte_out_of_its_domain_is_no_effect() {
        // activation_status 3, playback_type 2, serialization_mode 0 and 3,
        // effect_ended 0x01, the child's presence 0x00.
        for (offset, byte) in [(5, 3), (10, 2), (11, 0), (11, 3), (40, 1), (50, 0)] {
            let mut bad = LIVE;
            bad[EFFECT_AT + offset] = byte;
            assert!(
                matches!(
                    reason(&bad),
                    Some(Reason::EffectsEffect(EffectFault::Domain { .. }))
                ),
                "{offset}"
            );
        }
        // A stopped effect is an effect, not the section's.
        let mut stopped = LIVE;
        stopped[EFFECT_AT + 5] = 2;
        assert!(matches!(
            reason(&stopped),
            Some(Reason::EffectsField { at: 17, .. })
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
